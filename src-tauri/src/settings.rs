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
}

impl Default for Settings {
    fn default() -> Self {
        Settings { auto_close: true }
    }
}

static PATH: OnceLock<PathBuf> = OnceLock::new();
/// Read by the multi's thread without a lock.
pub static AUTO_CLOSE: AtomicBool = AtomicBool::new(true);

pub fn load(app: &AppHandle) {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("settings.json");
    let s: Settings = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    AUTO_CLOSE.store(s.auto_close, Ordering::Relaxed);
    let _ = PATH.set(path);
}

fn current() -> Settings {
    Settings { auto_close: AUTO_CLOSE.load(Ordering::Relaxed) }
}

#[tauri::command]
pub fn settings_get() -> Settings {
    current()
}

#[tauri::command]
pub fn settings_set(settings: Settings) -> Result<Settings, String> {
    let path = PATH.get().ok_or("configurações ainda não carregaram")?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("não consegui salvar a configuração: {e}"))?;
    AUTO_CLOSE.store(settings.auto_close, Ordering::Relaxed);
    Ok(current())
}
