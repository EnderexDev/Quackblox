// The saved accounts: each one's login (the .ROBLOSECURITY cookie) encrypted with DPAPI in
// accounts.json in the app's data folder, so only this Windows user on this PC can use it.
// An account comes in through a Roblox login window inside the app; the login never passes
// through the page's JavaScript, only Rust reads it.
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{app_account, clients, multi, roblox, win};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    id: u64,
    name: String,
    display_name: String,
    /// base64 of the DPAPI-encrypted cookie.
    login: String,
    added_at: u64,
    last_played: Option<u64>,
    #[serde(default)]
    expired: bool,
}

/// What the window sees of an account: never the login.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: u64,
    name: String,
    display_name: String,
    last_played: Option<u64>,
    expired: bool,
}

impl From<&Saved> for Account {
    fn from(s: &Saved) -> Self {
        Account { id: s.id, name: s.name.clone(), display_name: s.display_name.clone(), last_played: s.last_played, expired: s.expired }
    }
}

pub struct Store {
    path: PathBuf,
    list: Mutex<Vec<Saved>>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Store {
    pub fn load(app: &AppHandle) -> Self {
        let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("accounts.json");
        let list = match std::fs::read_to_string(&path) {
            Err(_) => vec![],
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                // Unreadable: kept aside, so the next save never writes over the accounts in it.
                let aside = path.with_extension(format!("broken-{}.json", now()));
                eprintln!("accounts: accounts.json unreadable ({e}), moved to {}", aside.display());
                let _ = std::fs::rename(&path, &aside);
                vec![]
            }),
        };
        Store { path, list: Mutex::new(list) }
    }

    fn save(&self, list: &[Saved]) -> Result<(), String> {
        let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
        // Written beside and renamed over, so a crash mid-write never leaves half a file.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)
            .and_then(|_| std::fs::rename(&tmp, &self.path))
            .map_err(|e| tr!("não consegui salvar as contas no disco: {e}", "couldn't save the accounts to disk: {e}"))
    }

    /// Changes the list and writes it; on a failed write the list in memory goes back too, so
    /// the window never shows what the disk does not hold.
    fn update(&self, f: impl FnOnce(&mut Vec<Saved>)) -> Result<(), String> {
        let mut list = self.list.lock().unwrap();
        let before = list.clone();
        f(&mut list);
        self.save(&list).inspect_err(|_| *list = before)
    }

    fn login_of(&self, id: u64) -> Result<String, String> {
        let list = self.list.lock().unwrap();
        let saved = list.iter().find(|s| s.id == id).ok_or_else(|| tr!("essa conta não está mais salva", "that account is no longer saved"))?;
        let encrypted = B64.decode(&saved.login).map_err(|_| tr!("o login salvo está corrompido", "the saved login is corrupted"))?;
        String::from_utf8(win::unprotect(&encrypted)?).map_err(|_| tr!("o login salvo está corrompido", "the saved login is corrupted"))
    }

    fn put(&self, user: &roblox::User, cookie: &str) -> Result<(), String> {
        let login = B64.encode(win::protect(cookie.as_bytes())?);
        self.update(|list| match list.iter_mut().find(|s| s.id == user.id) {
            // The same account logged in again: a fresh login, the place in the list kept.
            Some(s) => {
                s.login = login;
                s.name = user.name.clone();
                s.display_name = user.display_name.clone();
                s.expired = false;
            }
            None => list.push(Saved {
                id: user.id,
                name: user.name.clone(),
                display_name: user.display_name.clone(),
                login,
                added_at: now(),
                last_played: None,
                expired: false,
            }),
        })
    }
}

#[tauri::command]
pub fn accounts_list(store: tauri::State<Store>) -> Vec<Account> {
    store.list.lock().unwrap().iter().map(Account::from).collect()
}

#[tauri::command]
pub fn accounts_remove(store: tauri::State<Store>, id: u64) -> Result<(), String> {
    store.update(|list| list.retain(|s| s.id != id))
}

#[tauri::command]
pub fn accounts_move(store: tauri::State<Store>, id: u64, to: usize) -> Result<(), String> {
    store.update(|list| {
        if let Some(from) = list.iter().position(|s| s.id == id) {
            let item = list.remove(from);
            list.insert(to.min(list.len()), item);
        }
    })
}

/// The Roblox app's account as last read, for the title of the clients it opens.
static APP_USER: Mutex<Option<roblox::User>> = Mutex::new(None);

/// The account logged in to the Roblox app, or why there is none.
#[tauri::command]
pub async fn app_account() -> Result<roblox::User, String> {
    let cookie = app_account::cookie()?;
    let user = roblox::user_of(&cookie).await?;
    *APP_USER.lock().unwrap() = Some(user.clone());
    Ok(user)
}

/// The saved accounts' keys, in the order he gave the cards.
pub fn saved_order(app: &AppHandle) -> Vec<String> {
    app.state::<Store>().list.lock().unwrap().iter().map(|s| s.id.to_string()).collect()
}

/// Every account key: the saved ones in card order, the Roblox app's last. The app's account is
/// whatever is logged in to Roblox at the moment, so it never comes ahead of his own order.
pub fn order(app: &AppHandle) -> Vec<String> {
    let mut keys = saved_order(app);
    keys.push("app".into());
    keys
}

#[tauri::command]
pub async fn avatars(ids: Vec<u64>) -> Vec<(u64, String)> {
    roblox::headshots(&ids).await
}

#[tauri::command]
pub async fn game_info(input: String) -> Result<roblox::Game, String> {
    roblox::game(&input).await
}

const LOGIN_LABEL: &str = "login";
static LOGIN_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Opens Roblox's login page in a private window of its own. Once a login appears there it is
/// checked, saved, and the window closes; the main window hears "account-added" with the
/// account, or "account-error". A second click brings the window already open forward.
/// Async on purpose: on Windows, building a window inside a sync command deadlocks.
#[tauri::command]
pub async fn accounts_add(app: AppHandle) -> Result<(), String> {
    // Two clicks at once: the second waits here and then finds the window, instead of bumping
    // the generation (which would stop the first window's watcher) and failing on the label.
    static OPENING: Mutex<()> = Mutex::new(());
    let _guard = OPENING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(window) = app.get_webview_window(LOGIN_LABEL) {
        let _ = window.set_focus();
        return Ok(());
    }
    // One watcher per window opened: a watcher whose window was closed and replaced by a new
    // one stops, instead of watching the new window too. Bumped before the window exists, so
    // an old watcher never sees the new one.
    let generation = LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let url = "https://www.roblox.com/login".parse().map_err(|_| tr!("endereço de login inválido", "invalid login address"))?;
    let window = WebviewWindowBuilder::new(&app, LOGIN_LABEL, WebviewUrl::External(url))
        .title(tr!("Entrar numa conta do Roblox", "Sign in to a Roblox account"))
        .inner_size(520.0, 760.0)
        .incognito(true)
        .build()
        .map_err(|e| tr!("não abriu a janela de login: {e}", "the login window didn't open: {e}"))?;
    let roblox_url: tauri::Url = "https://www.roblox.com/".parse().unwrap();

    // Cookies are read off the main thread: on Windows the read deadlocks inside a command.
    std::thread::spawn(move || {
        let mut warned = false;
        loop {
            std::thread::sleep(Duration::from_millis(800));
            if LOGIN_GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            let Some(window) = app.get_webview_window(LOGIN_LABEL) else { return };
            let found = window
                .cookies_for_url(roblox_url.clone())
                .ok()
                .and_then(|cookies| cookies.into_iter().find(|c| c.name() == ".ROBLOSECURITY").map(|c| c.value().to_string()));
            let Some(cookie) = found else { continue };
            // Checked and saved while the window still holds the login: a passing network
            // failure leaves it there to try again, instead of making him log in again.
            let result = tauri::async_runtime::block_on(async {
                match roblox::user_of(&cookie).await {
                    // A disk or DPAPI failure may pass too: the login stays in the window for the retry.
                    Ok(user) => app.state::<Store>().put(&user, &cookie).map(|_| user).map_err(|e| (e, true)),
                    Err(e) => {
                        let dead = e.contains("expirou");
                        Err((e, !dead))
                    }
                }
            });
            match result {
                Err((e, true)) => {
                    if !warned {
                        warned = true;
                        let _ = app.emit("account-error", tr!("{e}. Tentando de novo...", "{e}. Trying again..."));
                    }
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
                done => {
                    // Nothing of this login stays in the window's private session for the next one.
                    let _ = window.clear_all_browsing_data();
                    let _ = window.close();
                    let _ = match done {
                        Ok(user) => app.emit("account-added", user),
                        Err((e, _)) => app.emit("account-error", e),
                    };
                    return;
                }
            }
        }
    });
    let _ = window.set_focus();
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Played {
    /// False when a Roblox opened before Quackblox still holds the lock: this client will
    /// close the one already open.
    multi: bool,
}

/// Opens a Roblox client logged in as the account ("app" for the Roblox app's own, or a saved
/// account's user id) in the game the input names.
#[tauri::command]
pub async fn play(app: AppHandle, account: String, game: String) -> Result<Played, String> {
    let (place_id, job_id) = roblox::parse_game(&game).ok_or_else(|| tr!("escolhe um jogo primeiro", "pick a game first"))?;
    let saved_id = if account == "app" { None } else { Some(account.parse::<u64>().map_err(|_| tr!("conta inválida", "invalid account"))?) };
    let cookie = match saved_id {
        None => app_account::cookie()?,
        Some(id) => app.state::<Store>().login_of(id)?,
    };
    let (ticket, rotated) = roblox::ticket(&cookie).await;
    // Roblox may have retired the old login with this answer, even a failed one: the new one is
    // saved before anything else, or the account would be lost at the next play.
    if let (Some(id), Some(new_cookie)) = (saved_id, rotated) {
        let login = B64.encode(win::protect(new_cookie.as_bytes())?);
        // Kept in memory even when the disk refuses it: the old login may be retired already,
        // and the next save writes it.
        let store = app.state::<Store>();
        let mut list = store.list.lock().unwrap();
        if let Some(s) = list.iter_mut().find(|s| s.id == id) {
            s.login = login;
        }
        if let Err(e) = store.save(&list) {
            eprintln!("accounts: renewed login kept in memory only: {e}");
        }
    }
    let ticket = match ticket {
        Ok(t) => t,
        Err(e) => {
            if let (Some(id), true) = (saved_id, e.contains("expirou")) {
                // The real reason goes back to the window even if this mark is not saved.
                let _ = app.state::<Store>().update(|list| {
                    if let Some(s) = list.iter_mut().find(|s| s.id == id) {
                        s.expired = true;
                    }
                });
            }
            return Err(e);
        }
    };
    // Off the async runtime: it waits up to 3 s on each tray player it ends.
    let handle = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || handle.state::<clients::Clients>().end_tray_players()).await;
    let process = win::open_link(&roblox::launch_link(&ticket, place_id, job_id.as_deref()))?;
    let name = match saved_id {
        Some(id) => app.state::<Store>().list.lock().unwrap().iter().find(|s| s.id == id).map(|s| s.display_name.clone()),
        None => APP_USER.lock().unwrap().as_ref().map(|u| u.display_name.clone()),
    };
    match process {
        Some(p) => app.state::<clients::Clients>().add(p, &account, name.as_deref().unwrap_or("Roblox")),
        None => eprintln!("accounts: the link opened no process of its own, the client is not tied to {account}"),
    }
    if let Some(id) = saved_id {
        // The client is already opening: a failed write here is only logged, never shown as a
        // failed play (that would invite a second click and a second client).
        let saved = app.state::<Store>().update(|list| {
            if let Some(s) = list.iter_mut().find(|s| s.id == id) {
                s.last_played = Some(now());
                s.expired = false;
            }
        });
        if let Err(e) = saved {
            eprintln!("accounts: last played not saved: {e}");
        }
    }
    Ok(Played { multi: multi::active() })
}
