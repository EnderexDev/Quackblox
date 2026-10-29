// Quackblox's own settings, in settings.json in the app's data folder.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

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
    /// A Windows notification when an account drops (vigia.rs). Nothing leaves the PC.
    pub alert_windows: bool,
    /// The same notice sent to a Discord webhook too. Off until he turns it on (the roadmap's rule
    /// for anything that goes out on the network or to Discord).
    pub alert_discord: bool,
    /// The webhook's address, his to give. Never written to a log.
    pub discord_webhook: String,
    /// Each saved login asked of Roblox every 30 min, so an expired one is told as soon as it
    /// expires. Off by default, by the same rule.
    pub login_check: bool,
    /// The phone page on the home network (phone.rs). Off by default, by the same rule.
    pub phone: bool,
    /// Discord's "Playing Quackblox" with the game (discord.rs). Off by default too.
    pub rich_presence: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_close: true,
            language: "pt".into(),
            hotkeys: true,
            alert_windows: true,
            alert_discord: false,
            discord_webhook: String::new(),
            login_check: false,
            phone: false,
            rich_presence: false,
        }
    }
}

static PATH: OnceLock<PathBuf> = OnceLock::new();
/// Read by the multi's thread without a lock.
pub static AUTO_CLOSE: AtomicBool = AtomicBool::new(true);
static HOTKEYS: AtomicBool = AtomicBool::new(true);
/// Read by `tr!` wherever a text for the user is made.
static ENGLISH: AtomicBool = AtomicBool::new(false);
/// The whole set as last loaded or saved, for the settings read now and then (vigia.rs).
static CURRENT: Mutex<Option<Settings>> = Mutex::new(None);

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
    *CURRENT.lock().unwrap() = Some(s);
    let _ = PATH.set(path);
}

pub fn current() -> Settings {
    CURRENT.lock().unwrap().clone().unwrap_or_default()
}

#[tauri::command]
pub fn settings_get() -> Settings {
    current()
}

/// The window sends every setting each time: a field left out would be saved as its default.
#[tauri::command]
pub fn settings_set(app: AppHandle, mut settings: Settings) -> Result<Settings, String> {
    let path = PATH.get().ok_or_else(|| tr!("configurações ainda não carregaram", "settings have not loaded yet"))?;
    settings.discord_webhook = settings.discord_webhook.trim().to_string();
    if !settings.discord_webhook.is_empty() && !crate::vigia::is_webhook(&settings.discord_webhook) {
        return Err(tr!(
            "esse não é um endereço de webhook do Discord (começa com https://discord.com/api/webhooks/)",
            "that isn't a Discord webhook address (it starts with https://discord.com/api/webhooks/)"
        ));
    }
    if settings.alert_discord && settings.discord_webhook.is_empty() {
        return Err(tr!("cola o endereço do webhook antes de ligar o aviso no Discord", "paste the webhook address before turning on the Discord notice"));
    }
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
    *CURRENT.lock().unwrap() = Some(settings);
    crate::desktop::retranslate(&app);
    crate::phone::apply(&app);
    Ok(current())
}

/// Whether this build knows Quackblox's Discord application, so the switch shows.
#[tauri::command]
pub fn rich_presence_available() -> bool {
    crate::discord::available()
}
