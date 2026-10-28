// The watch over the clients (phase 3): what happens when one drops. clients.rs sees the drop in
// the client's log (the client stays open with Roblox's error dialog) or its end with no reason
// (a crash); here the drop is told (the window, a Windows notification, Discord when he set a
// webhook) and, for an account with rejoin on, the dropped client is ended and the account goes
// back in through the same `play` as a click. A close by him (285) is never a drop. The saved
// logins are also asked of Roblox now and then, so one that expired is told when it expires.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::clients::Clients;

/// Waits before each try of a row: the first soon, the last after the server had time to come
/// back. Past the last, it stops until the account is in a game again.
const BACKOFF: [u64; 3] = [5, 30, 120];
const LOGIN_CHECK_EVERY: Duration = Duration::from_secs(30 * 60);

/// A client that dropped, crashed or never got into a game.
pub struct Fall {
    /// Set while the client still runs (the error dialog on screen).
    pub pid: Option<u32>,
    pub account: String,
    pub user: Option<u64>,
    pub name: String,
    pub game: String,
    pub reason: Option<u32>,
    /// "dropped", "crashed" or "joinFailed".
    pub kind: &'static str,
    /// The card whose rejoin switch it follows when that is not its own (clients.rs).
    pub from: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fell {
    account: String,
    name: String,
    reason: Option<u32>,
    kind: &'static str,
    /// Seconds until it goes back in, when it will.
    rejoin_in: Option<u64>,
    /// Rejoin is on but this drop does not get one: why (a code that a rejoin cannot fix, the
    /// tries used up, the multi off, the game unknown).
    no_rejoin: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Rejoin {
    account: String,
    name: String,
    /// None when it opened; the reason it did not otherwise.
    error: Option<String>,
}

/// Tries in a row per Roblox user (or account key when the user is unknown), and the run each
/// belongs to: a run turned off, or ended by a join, stops its waiting task. A row starts over
/// once the account stayed in a game for a while: one that drops right after each join stops
/// after the last try instead of going in and out for ever.
struct Tries {
    count: usize,
    run: u64,
    joined_at: Option<Instant>,
}
const ROW_RESET: Duration = Duration::from_secs(5 * 60);
static TRIES: Mutex<Option<HashMap<String, Tries>>> = Mutex::new(None);
static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn tries_key(account: &str, user: Option<u64>) -> String {
    user.map(|u| u.to_string()).unwrap_or_else(|| account.to_string())
}

/// Codes a rejoin cannot fix or must not fight: him leaving (285), the same account playing
/// elsewhere (264, 273, 276: going back in would knock that session out), a kick or ban by the
/// game or by Roblox, a blocked network, an outdated client.
fn rejoinable(reason: Option<u32>) -> bool {
    // Tests of a rejoin from a live drop with the one drop that can be made on purpose (the same
    // account opened twice, 276). Dev builds only.
    if cfg!(debug_assertions) && std::env::var_os("QUACKBLOX_REJOIN_ANY").is_some() {
        return reason != Some(285);
    }
    !matches!(reason, Some(5 | 264 | 267 | 273 | 276 | 280 | 282 | 285 | 287 | 291 | 309 | 600))
}

pub fn is_webhook(url: &str) -> bool {
    ["https://discord.com/api/webhooks/", "https://discordapp.com/api/webhooks/", "https://ptb.discord.com/api/webhooks/", "https://canary.discord.com/api/webhooks/"]
        .iter()
        .any(|p| url.starts_with(p) && url.len() > p.len())
}

/// A client of the account got into a game: a try still waiting does not start.
pub fn joined(_app: &AppHandle, account: &str, user: Option<u64>) {
    if let Some(map) = TRIES.lock().unwrap().as_mut() {
        for key in [tries_key(account, user), account.to_string()] {
            if let Some(t) = map.get_mut(&key) {
                t.run = 0;
                t.joined_at = Some(Instant::now());
            }
        }
    }
}

/// Rejoin turned off for the account: a try waiting to start does not.
pub fn cancel(account: &str) {
    if let Some(map) = TRIES.lock().unwrap().as_mut() {
        map.retain(|k, _| k != account && crate::accounts::user_id(account).is_none_or(|u| *k != u.to_string()));
    }
}

/// What the notice says about why, in a line: Roblox's code when the log had one.
fn why(f: &Fall) -> String {
    match (f.kind, f.reason) {
        ("joinFailed", _) => tr!("não conseguiu entrar no jogo", "couldn't get into the game"),
        ("crashed", _) => tr!("o Roblox fechou sozinho", "Roblox closed by itself"),
        (_, Some(code)) => tr!("caiu do jogo (código {code})", "dropped from the game (code {code})"),
        _ => tr!("caiu do jogo", "dropped from the game"),
    }
}

pub fn fell(app: &AppHandle, f: Fall) {
    let on = on(app, &f);
    let key = tries_key(&f.account, f.user);
    let mut no_rejoin = None;
    let mut plan = None;
    if on {
        let mut guard = TRIES.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        let count = map.get(&key).filter(|t| t.joined_at.is_none_or(|j| j.elapsed() < ROW_RESET)).map_or(0, |t| t.count);
        if !rejoinable(f.reason) {
            no_rejoin = Some(tr!("esse código não se resolve entrando de novo", "entering again doesn't fix that code"));
        } else if f.game.is_empty() {
            no_rejoin = Some(tr!("não sei em que jogo ela estava", "I don't know which game it was in"));
        } else if count >= BACKOFF.len() {
            no_rejoin = Some(tr!("já tentei {} vezes seguidas", "already tried {} times in a row", BACKOFF.len()));
        } else {
            let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            map.insert(key.clone(), Tries { count: count + 1, run, joined_at: None });
            plan = Some((count, run));
        }
    }
    let rejoin_in = plan.map(|(count, _)| BACKOFF[count]);
    let _ = app.emit(
        "client-fell",
        Fell { account: f.account.clone(), name: f.name.clone(), reason: f.reason, kind: f.kind, rejoin_in, no_rejoin: no_rejoin.clone() },
    );
    let mut text = format!("{} {}.", f.name, why(&f));
    if let Some(s) = rejoin_in {
        text += &tr!(" Entrando de novo em {s} s.", " Going back in in {s} s.");
    } else if let Some(n) = &no_rejoin {
        text += &tr!(" Não vou entrar de novo: {n}.", " Not going back in: {n}.");
    }
    alert(app, &text);
    if let Some((count, run)) = plan {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { rejoin(app, f, key, count, run).await });
    }
}

/// Waits, ends the dropped client, and opens the account again; a failed open waits for the next
/// try of the row, until the row is used up.
async fn rejoin(app: AppHandle, f: Fall, key: String, mut count: usize, run: u64) {
    let current = |app: &AppHandle| TRIES.lock().unwrap().as_ref().and_then(|m| m.get(&key).map(|t| t.run)) == Some(run) && on(app, &f);
    let mut pid = f.pid;
    loop {
        tokio::time::sleep(Duration::from_secs(BACKOFF[count])).await;
        if !current(&app) {
            return;
        }
        let error = match open(&app, &f, &mut pid, &|| current(&app)).await {
            Ok(()) => None,
            Err(e) => Some(e),
        };
        // Turned off, or back in by itself, while it got ready: nothing was opened, nothing to tell.
        if error.is_some() && !current(&app) {
            return;
        }
        let _ = app.emit("client-rejoin", Rejoin { account: f.account.clone(), name: f.name.clone(), error: error.clone() });
        let Some(e) = error else { return };
        count += 1;
        let next = {
            let mut guard = TRIES.lock().unwrap();
            match guard.as_mut().and_then(|m| m.get_mut(&key)).filter(|t| t.run == run) {
                Some(t) if count < BACKOFF.len() => {
                    t.count = count + 1;
                    true
                }
                _ => false,
            }
        };
        if !next {
            alert(&app, &tr!("{} não entrou de novo: {e}. Parei de tentar.", "{} didn't get back in: {e}. Stopped trying.", f.name));
            return;
        }
    }
}

/// A rejoin opening a user under another card than the one with rejoin on (the Roblox app's
/// card's account, opened again by its saved login): the client it opens is marked with that
/// card (clients.rs), so its next drop follows that card's switch. Only for the open under way:
/// a client he opens himself later follows its own card.
static OPENING_FROM: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

/// The card a client of this user being opened now by a rejoin follows, taken once.
pub fn take_from(user: u64) -> Option<String> {
    OPENING_FROM.lock().unwrap().as_mut().and_then(|m| m.remove(&user))
}

/// Whether rejoin is on for the client's card, its saved account, or the card it came back from.
fn on(app: &AppHandle, f: &Fall) -> bool {
    let clients = app.state::<Clients>();
    clients.rejoin_on(&f.account) || f.user.is_some_and(|u| clients.rejoin_on(&u.to_string())) || f.from.as_ref().is_some_and(|k| clients.rejoin_on(k))
}

/// `still`: whether this try is still wanted, asked again after each wait, before a client is
/// ended or opened.
async fn open(app: &AppHandle, f: &Fall, pid: &mut Option<u32>, still: &(dyn Fn() -> bool + Sync)) -> Result<(), String> {
    let stopped = || tr!("parei", "stopped");
    // With the multi off, a new client closes the ones open (Roblox's own lock).
    if !crate::multi::active() {
        return Err(tr!("o multi está desligado", "the multi is off"));
    }
    // The Roblox app's login turns into whoever played last: the account that dropped goes back
    // in by its saved login when it has one, and by the app's only while the app is still it.
    let account = match (f.account.as_str(), f.user) {
        ("app", Some(user)) => match crate::accounts::saved_key(app, user) {
            Some(key) => key,
            None => {
                let now = crate::accounts::app_account().await?;
                if now.id != user {
                    return Err(tr!("o app do Roblox está em outra conta agora", "the Roblox app is on another account now"));
                }
                "app".into()
            }
        },
        (key, _) => key.to_string(),
    };
    if !still() {
        return Err(stopped());
    }
    if let Some(p) = *pid {
        // Waits up to 3 s for the end: off the async runtime. One that would not end is kept for
        // the next try: a second client beside it would only pile up.
        let handle = app.clone();
        let ended = tauri::async_runtime::spawn_blocking(move || handle.state::<Clients>().end(p)).await.unwrap_or(false);
        let still_there = !ended && app.state::<Clients>().pids().contains(&p);
        if still_there {
            return Err(tr!("não consegui fechar o Roblox que caiu", "couldn't close the Roblox that dropped"));
        }
        *pid = None;
    }
    if !still() {
        return Err(stopped());
    }
    let game = match f.kind {
        // A server that would not take it is not asked again: the next try goes to the game.
        "joinFailed" => match crate::roblox::parse_game(&f.game) {
            Some(p) if p.link_code.is_none() && p.job_id.is_some() => p.place_id.to_string(),
            _ => f.game.clone(),
        },
        _ => f.game.clone(),
    };
    let from = f.user.filter(|_| account != f.account);
    if let Some(user) = from {
        OPENING_FROM.lock().unwrap().get_or_insert_with(HashMap::new).insert(user, f.from.clone().unwrap_or_else(|| f.account.clone()));
    }
    let played = crate::accounts::play(app.clone(), account, game).await.map(|_| ());
    if let Some(user) = from {
        take_from(user);
    }
    played
}

/// A notice off the window: Windows' own, and Discord's when he turned it on.
pub fn alert(app: &AppHandle, text: &str) {
    let s = crate::settings::current();
    if s.alert_windows {
        if let Err(e) = app.notification().builder().title("Quackblox").body(text).show() {
            eprintln!("vigia: Windows notification not shown: {e}");
        }
    }
    if s.alert_discord && is_webhook(&s.discord_webhook) {
        let text = text.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = discord(&s.discord_webhook, &text).await {
                // The address is never written: it is a key to his channel.
                eprintln!("vigia: Discord notice not sent: {e}");
            }
        });
    }
}

async fn discord(webhook: &str, text: &str) -> Result<(), String> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(15)).build().expect("http client"));
    let body = serde_json::json!({ "username": "Quackblox", "content": text, "allowed_mentions": { "parse": [] } });
    let res = client.post(webhook).json(&body).send().await.map_err(|e| tr!("sem conexão com o Discord ({})", "no connection to Discord ({})", e.without_url()))?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(tr!("o Discord recusou (HTTP {})", "Discord refused (HTTP {})", res.status().as_u16()))
    }
}

/// Sends a test notice to the webhook he typed, saved or not.
#[tauri::command]
pub async fn vigia_test_discord(webhook: String) -> Result<(), String> {
    let webhook = webhook.trim();
    if !is_webhook(webhook) {
        return Err(tr!("esse não é um endereço de webhook do Discord", "that isn't a Discord webhook address"));
    }
    discord(webhook, &tr!("Teste do Quackblox: os avisos de queda chegam aqui.", "Quackblox test: drop notices will arrive here.")).await
}

/// Sends a test Windows notification.
#[tauri::command]
pub fn vigia_test_windows(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("Quackblox")
        .body(tr!("Teste: os avisos de queda aparecem assim.", "Test: drop notices show up like this."))
        .show()
        .map_err(|e| tr!("o Windows não mostrou o aviso ({e})", "Windows didn't show the notice ({e})"))
}

/// The login check, while it is on: every 30 min, the first a minute after the start.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last: Option<Instant> = None;
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            if crate::settings::current().login_check && last.is_none_or(|t| t.elapsed() >= LOGIN_CHECK_EVERY) {
                last = Some(Instant::now());
                let expired = crate::accounts::check_logins(&app).await;
                if !expired.is_empty() {
                    let _ = app.emit("accounts-changed", ());
                    for name in expired {
                        alert(&app, &tr!("O login de {name} expirou. Entra de novo nela pelo Quackblox.", "{name}'s login expired. Sign in to it again in Quackblox."));
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

/// Checks the saved logins now (the button beside the setting).
#[tauri::command]
pub async fn vigia_check_logins(app: AppHandle) -> Vec<String> {
    let expired = crate::accounts::check_logins(&app).await;
    if !expired.is_empty() {
        let _ = app.emit("accounts-changed", ());
    }
    expired
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejoin_skips_what_it_cannot_fix() {
        assert!(rejoinable(Some(277)));
        assert!(rejoinable(None));
        assert!(!rejoinable(Some(285)));
        assert!(!rejoinable(Some(273)));
        assert!(!rejoinable(Some(276)));
    }

    #[test]
    fn only_discord_webhooks() {
        assert!(is_webhook("https://discord.com/api/webhooks/1/abc"));
        assert!(!is_webhook("https://discord.com/api/webhooks/"));
        assert!(!is_webhook("https://evil.example/api/webhooks/1/abc"));
        assert!(!is_webhook("http://discord.com/api/webhooks/1/abc"));
    }
}
