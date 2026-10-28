// Quackblox's own settings, in settings.json in the app's data folder.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Close the Roblox that holds the lock, so the multi turns on by itself (his call, 28/10/26).
    pub auto_close: bool,
    /// "pt" or "en". A file from before 0.2.5 has none and stays in Portuguese.
    pub language: String,
    /// Ctrl+Shift+0 to 9 (hotkeys.rs). A file from before 0.3.0 has none and gets them on.
    pub hotkeys: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { auto_close: true, language: "pt".into(), hotkeys: true }
    }
}

static PATH: OnceLock<PathBuf> = OnceLock::new();
/// Read by the multi's thread without a lock.
pub static AUTO_CLOSE: AtomicBool = AtomicBool::new(true);
static HOTKEYS: AtomicBool = AtomicBool::new(true);
/// Read by `tr!` wherever a text for the user is made.
static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn hotkeys() -> bool {
    HOTKEYS.load(Ordering::Relaxed)
}

/// Also run by the installer and the update window, which read the language from the same file.
pub fn load(app: &AppHandle) {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("settings.json");
    let s: Settings = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    AUTO_CLOSE.store(s.auto_close, Ordering::Relaxed);
    ENGLISH.store(s.language == "en", Ordering::Relaxed);
    HOTKEYS.store(s.hotkeys, Ordering::Relaxed);
    let _ = PATH.set(path);
}

fn current() -> Settings {
    Settings { auto_close: AUTO_CLOSE.load(Ordering::Relaxed), language: if english() { "en" } else { "pt" }.into(), hotkeys: hotkeys() }
}

#[tauri::command]
pub fn settings_get() -> Settings {
    current()
}

/// The window sends every setting each time: a field left out would be saved as its default.
#[tauri::command]
pub fn settings_set(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let path = PATH.get().ok_or_else(|| tr!("configurações ainda não carregaram", "settings have not loaded yet"))?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    // Written beside, flushed to the disk and renamed over: a half-written file would bring
    // auto_close back on, and the language back to Portuguese.
    let tmp = path.with_extension("json.tmp");
    std::fs::File::create(&tmp)
        .and_then(|mut f| {
            std::io::Write::write_all(&mut f, text.as_bytes())?;
            f.sync_all()
        })
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| tr!("não consegui salvar a configuração: {e}", "couldn't save the settings: {e}"))?;
    AUTO_CLOSE.store(settings.auto_close, Ordering::Relaxed);
    ENGLISH.store(settings.language == "en", Ordering::Relaxed);
    if HOTKEYS.swap(settings.hotkeys, Ordering::Relaxed) != settings.hotkeys {
        crate::hotkeys::set(settings.hotkeys);
    }
    crate::desktop::retranslate(&app);
    Ok(current())
}
