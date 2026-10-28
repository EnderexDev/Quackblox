// The Roblox site logged in to a saved account, in a window of the app (phase 4). Each window has a
// browser profile of its own (its own data folder, so its own WebView2 environment, in private
// mode on top), the login put in it from here as an HttpOnly cookie: it never passes through a
// page's JavaScript, the main window's or the site's. On close the login Roblox may have renewed
// meanwhile is kept, and the profile is cleared and its folder deleted. Not for the Roblox app's
// card: its login belongs to the Roblox app, and a renewal here would sign the app out.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::webview::Cookie;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const LABEL: &str = "site-";
const COOKIE: &str = ".ROBLOSECURITY";

/// Each open window's account, data folder and the login it was given.
struct Open {
    account: String,
    dir: PathBuf,
    login: String,
    /// Set once the close started: the second close request is the real one.
    closing: bool,
}

static OPEN: Mutex<Option<HashMap<String, Open>>> = Mutex::new(None);

fn folder(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".")).join("site")
}

/// Only Roblox's own pages load in it. A roblox-player: link (the site's Play button) is refused
/// too: a client opened from there is not tied to the account's card (no title, no CPU line, no
/// rejoin), so the game opens from the card.
fn allowed(url: &tauri::Url) -> bool {
    if url.as_str() == "about:blank" {
        return true;
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    url.scheme() == "https" && ["roblox.com", "rbxcdn.com"].iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Deletes what a window left in its folder. The browser process lets go of its files a moment
/// after the window closes: tried for up to 30 s.
fn remove_later(dir: PathBuf) {
    std::thread::spawn(move || {
        for _ in 0..30 {
            if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        eprintln!("site: a site profile folder was not deleted; it goes at the next start");
    });
}

/// At start: the folders of site windows open when the app last quit.
pub fn clean_up(app: &AppHandle) {
    let dir = folder(app);
    if dir.exists() {
        remove_later(dir);
    }
}

/// Opens the site logged in to a saved account, or brings its window forward.
/// Async on purpose: on Windows, building a window inside a sync command deadlocks.
#[tauri::command]
pub async fn site_open(app: AppHandle, account: String) -> Result<(), String> {
    if account == "app" {
        return Err(tr!(
            "o site abre só pelas contas salvas: o login do app do Roblox é dele",
            "the site opens only from saved accounts: the Roblox app's login is its own"
        ));
    }
    let label = format!("{LABEL}{account}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let (name, expired) = crate::accounts::saved(&app, &account).ok_or_else(|| tr!("essa conta não está mais salva", "that account is no longer saved"))?;
    if expired {
        return Err(tr!("o login de {name} expirou: entra de novo nela antes", "{name}'s login expired: sign in to it again first"));
    }
    let login = crate::accounts::login(&app, &account)?;
    let dir = folder(&app).join(uuid::Uuid::new_v4().simple().to_string());
    let blank = "about:blank".parse().map_err(|_| tr!("endereço inválido", "invalid address"))?;
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(blank))
        .title(tr!("{name} · site do Roblox", "{name} · Roblox site"))
        .inner_size(1180.0, 820.0)
        .data_directory(dir.clone())
        .incognito(true)
        .on_navigation(allowed)
        .on_new_window({
            let (app, label) = (app.clone(), label.clone());
            move |url, _| {
                // A link that opens a tab would get a window outside this profile: it opens in
                // this one instead, when it is Roblox's.
                if allowed(&url) {
                    let (app, label) = (app.clone(), label.clone());
                    std::thread::spawn(move || app.get_webview_window(&label).map(|w| w.navigate(url)));
                }
                tauri::webview::NewWindowResponse::Deny
            }
        })
        .build()
        .map_err(|e| tr!("a janela do site não abriu: {e}", "the site window didn't open: {e}"))?;
    OPEN.lock().unwrap().get_or_insert_with(HashMap::new).insert(label.clone(), Open { account, dir, login: login.clone(), closing: false });

    // Cookies are set and read off the main thread: on Windows the call deadlocks inside a command.
    std::thread::spawn(move || {
        let cookie = Cookie::build((COOKIE, login)).domain(".roblox.com").path("/").secure(true).http_only(true).build();
        let home: tauri::Url = "https://www.roblox.com/home".parse().unwrap();
        let set = window.set_cookie(cookie).is_ok() && has_login(&window);
        if !set {
            eprintln!("site: the login did not go into the site window");
            let _ = window.close();
            return;
        }
        let _ = window.navigate(home);
    });
    Ok(())
}

fn login_in(window: &tauri::WebviewWindow) -> Option<String> {
    let url: tauri::Url = "https://www.roblox.com/".parse().unwrap();
    window.cookies_for_url(url).ok()?.into_iter().find(|c| c.name() == COOKIE).map(|c| c.value().to_string())
}

fn has_login(window: &tauri::WebviewWindow) -> bool {
    login_in(window).is_some()
}

/// A site window closing: the login read back (Roblox may have renewed it while he browsed) and
/// kept, the profile cleared, the window closed, its folder deleted.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let label = window.label().to_string();
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            let mut open = OPEN.lock().unwrap();
            let Some(o) = open.as_mut().and_then(|m| m.get_mut(&label)) else { return };
            if o.closing {
                return;
            }
            o.closing = true;
            api.prevent_close();
            let (account, given) = (o.account.clone(), o.login.clone());
            let app = window.app_handle().clone();
            std::thread::spawn(move || {
                let Some(w) = app.get_webview_window(&label) else { return };
                match login_in(&w) {
                    Some(now) if now != given => {
                        if let Err(e) = crate::accounts::renew(&app, &account, &now) {
                            eprintln!("site: renewed login not saved: {e}");
                        }
                    }
                    Some(_) => {}
                    // Signed out on the site: Roblox ends that login for every place it is used.
                    None => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if !crate::accounts::check_logins(&app).await.is_empty() {
                                let _ = tauri::Emitter::emit(&app, "accounts-changed", ());
                            }
                        });
                    }
                }
                let _ = w.clear_all_browsing_data();
                let _ = w.destroy();
            });
        }
        WindowEvent::Destroyed => {
            if let Some(o) = OPEN.lock().unwrap().as_mut().and_then(|m| m.remove(&label)) {
                remove_later(o.dir);
            }
        }
        _ => {}
    }
}

/// The accounts whose site window is open.
#[tauri::command]
pub fn site_open_list() -> Vec<String> {
    OPEN.lock().unwrap().as_ref().map(|m| m.values().map(|o| o.account.clone()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_roblox_pages() {
        let ok = |u: &str| allowed(&u.parse().unwrap());
        assert!(ok("https://www.roblox.com/home"));
        assert!(ok("https://auth.roblox.com/v2/login"));
        assert!(ok("https://tr.rbxcdn.com/x.png"));
        assert!(ok("about:blank"));
        assert!(!ok("http://www.roblox.com/home"));
        assert!(!ok("https://roblox.com.evil.example/"));
        assert!(!ok("https://evilroblox.com/"));
        assert!(!ok("roblox-player:1+launchmode:play"));
    }
}
