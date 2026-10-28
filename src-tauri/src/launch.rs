// Opening accounts in a game, one after another (phase 4): a card's Jogar, "Jogar com todas", a
// group, a profile and the schedule all come through here, so they wait for room, follow the
// first account's server and space the clients the same way. In Rust and not in the window
// because the schedule opens accounts while the window is hidden in the tray. Each client still
// opens through `accounts::play`, the same as a click: the tray players ended first, the login
// renewed, QUACKBLOX_NO_LAUNCH obeyed.
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::clients::Clients;

/// Between two clients of a run, as the Account Manager does (2.5 s there).
const GAP: Duration = Duration::from_secs(3);
/// How often a full server is looked at again while an account waits for room in it.
const SEAT_EVERY: Duration = Duration::from_secs(15);
/// How long "Mesmo servidor" waits for the first account to join a server.
const FIRST_SERVER: Duration = Duration::from_secs(90);
/// How long the grid waits for the windows of a run to show (a join gives up at 120 s).
const WINDOWS_WAIT: Duration = Duration::from_secs(120);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Account keys ("app" or a saved account's id), in the order they open.
    pub accounts: Vec<String>,
    /// What `play` takes: a place id, or a link naming the server or the private server.
    pub game: String,
    pub game_name: String,
    /// The others go to the server the first one joined (with no server picked).
    #[serde(default)]
    pub same_server: bool,
    /// Accounts to put in economy mode, the rest of the run's taken out of it. None leaves each
    /// account as it is.
    #[serde(default)]
    pub eco: Option<Vec<String>>,
    /// The windows laid out in a grid once all of them show.
    #[serde(default)]
    pub grid: bool,
}

/// A step of a run, for the window: what each card shows and the notice it gives.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    account: String,
    name: String,
    /// "seat" (waiting for room), "opening", "opened", "failed", "stopped" (he stopped the wait),
    /// "seatOpened", "firstServer" (waiting for the first account's server), "sameServer" (found),
    /// "sameServerTimeout", "grid".
    kind: &'static str,
    game: String,
    playing: u32,
    max: u32,
    /// The client opened with the multi on.
    multi: bool,
    error: Option<String>,
    /// How many windows the grid placed.
    count: usize,
}

impl Step {
    fn new(account: &str, name: &str, kind: &'static str) -> Self {
        Step { account: account.into(), name: name.into(), kind, game: String::new(), playing: 0, max: 0, multi: false, error: None, count: 0 }
    }
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub opened: usize,
    /// "Name: why" for each account that did not open.
    pub failed: Vec<String>,
}

/// Accounts with a client being opened or waiting for room: a second click (or a schedule) on
/// one of them does nothing.
static BUSY: Mutex<Option<HashSet<String>>> = Mutex::new(None);
/// Accounts waiting for room; taking one out stops its wait.
static WAITING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn take(account: &str) -> bool {
    BUSY.lock().unwrap().get_or_insert_with(HashSet::new).insert(account.to_string())
}

fn give_back(account: &str) {
    if let Some(b) = BUSY.lock().unwrap().as_mut() {
        b.remove(account);
    }
}

fn emit(app: &AppHandle, step: Step) {
    let _ = app.emit("launch", step);
}

/// A run's accounts as they can open now, with each one's name: the Roblox app's read again (it
/// is whoever played last), a saved account equal to it kept and the app's card dropped, so one
/// user never opens twice. Accounts no longer saved are left out; expired ones fail.
async fn who(app: &AppHandle, keys: &[String]) -> Vec<(String, String, Result<(), String>)> {
    let app_user = if keys.iter().any(|k| k == "app") { crate::accounts::app_account().await.ok() } else { None };
    let mut seen = HashSet::new();
    let mut out = vec![];
    for key in keys {
        if !seen.insert(key.clone()) {
            continue;
        }
        if key == "app" {
            let Some(u) = &app_user else {
                // With others in the run, an app with no login is left out, as its card shows.
                if keys.len() > 1 {
                    continue;
                }
                out.push((key.clone(), "Roblox".into(), Err(tr!("não consegui ler a conta do app do Roblox", "couldn't read the Roblox app's account"))));
                continue;
            };
            if keys.iter().any(|k| *k == u.id.to_string()) {
                continue;
            }
            out.push((key.clone(), u.display_name.clone(), Ok(())));
            continue;
        }
        let Some((name, expired)) = crate::accounts::saved(app, key) else { continue };
        out.push((key.clone(), name, if expired { Err(tr!("o login expirou", "the login expired")) } else { Ok(()) }));
    }
    out
}

/// Opens the plan's accounts one after another. More than one account needs the multi on: with
/// it off each client would close the one before.
pub async fn run(app: &AppHandle, plan: Plan) -> Result<Summary, String> {
    let mut place = crate::roblox::parse_game(&plan.game).ok_or_else(|| tr!("escolhe um jogo primeiro", "pick a game first"))?;
    let list = who(app, &plan.accounts).await;
    if list.is_empty() {
        return Err(tr!("nenhuma dessas contas está salva", "none of those accounts is saved"));
    }
    if list.len() > 1 && !crate::multi::active() {
        return Err(tr!("o multi está desligado: um Roblox aberto antes do Quackblox segura a trava", "the multi is off: a Roblox opened before Quackblox holds the lock"));
    }
    if let Some(eco) = &plan.eco {
        for (key, _, _) in &list {
            crate::clients::clients_set_eco(app.state::<Clients>(), key.clone(), eco.contains(key));
        }
    }
    let mut game = plan.game.clone();
    let mut summary = Summary::default();
    let mut opened_keys = vec![];
    for (i, (key, name, ok)) in list.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(GAP).await;
        }
        if let Err(e) = ok {
            fail(app, &mut summary, key, name, e.clone());
            continue;
        }
        if !take(key) {
            continue;
        }
        let result = open(app, key, name, &place, &game, &plan.game_name).await;
        give_back(key);
        match result {
            Ok(true) => {
                summary.opened += 1;
                opened_keys.push(key.clone());
            }
            Ok(false) => {}
            Err(e) => {
                fail(app, &mut summary, key, name, e);
                continue;
            }
        }
        // "Mesmo servidor" with no server picked: the others go where the first one landed.
        if i == 0 && summary.opened == 1 && plan.same_server && place.job_id.is_none() && place.link_code.is_none() && list.len() > 1 {
            emit(app, Step::new(key, name, "firstServer"));
            match first_server(app, key).await {
                Some((place_id, job)) => {
                    place = crate::roblox::Place { place_id, job_id: Some(job.clone()), link_code: None };
                    game = format!("https://www.roblox.com/games/start?placeId={place_id}&gameInstanceId={job}");
                    emit(app, Step::new(key, name, "sameServer"));
                }
                None => emit(app, Step::new(key, name, "sameServerTimeout")),
            }
        }
    }
    if plan.grid && !opened_keys.is_empty() {
        let start = Instant::now();
        while start.elapsed() < WINDOWS_WAIT && !opened_keys.iter().all(|k| app.state::<Clients>().shown(k)) {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let handle = app.clone();
        match tauri::async_runtime::spawn_blocking(move || crate::clients::grid(&handle)).await {
            Ok(Ok(n)) => emit(app, Step { count: n, ..Step::new("", "", "grid") }),
            Ok(Err(e)) => eprintln!("launch: grid after the run: {e}"),
            Err(e) => eprintln!("launch: grid after the run: {e}"),
        }
    }
    Ok(summary)
}

fn fail(app: &AppHandle, summary: &mut Summary, key: &str, name: &str, error: String) {
    summary.failed.push(format!("{name}: {error}"));
    emit(app, Step { error: Some(error), ..Step::new(key, name, "failed") });
}

/// Waits for room when a full server was picked, then opens the client. False when he stopped the
/// wait (nothing opened).
async fn open(app: &AppHandle, key: &str, name: &str, place: &crate::roblox::Place, game: &str, game_name: &str) -> Result<bool, String> {
    if let (Some(job), None) = (&place.job_id, &place.link_code) {
        if !wait_for_seat(app, key, name, place.place_id, job).await {
            return Ok(false);
        }
    }
    emit(app, Step::new(key, name, "opening"));
    let played = crate::accounts::play(app.clone(), key.to_string(), game.to_string()).await?;
    emit(app, Step { game: game_name.into(), multi: played.multi(), ..Step::new(key, name, "opened") });
    Ok(true)
}

/// Holds the join while the server is full, looking again every 15 s. False when he stopped the
/// wait. A server not in the list (it ended, or it is past the first pages) does not hold the
/// join: Roblox has the last word on it. A failed look before any wait lets the join go ahead; a
/// failed look while waiting keeps waiting.
async fn wait_for_seat(app: &AppHandle, key: &str, name: &str, place_id: u64, job: &str) -> bool {
    let waiting = |on: bool| {
        let mut w = WAITING.lock().unwrap();
        let w = w.get_or_insert_with(HashSet::new);
        if on { w.insert(key.to_string()) } else { w.remove(key) }
    };
    let still = || WAITING.lock().unwrap().as_ref().is_some_and(|w| w.contains(key));
    for looked in 0.. {
        let seat = crate::accounts::server_seat(app.clone(), place_id, job.to_string()).await.ok();
        let room = match &seat {
            Some(s) => !s.found() || s.playing() < s.max_players(),
            None => looked == 0,
        };
        if room {
            if looked > 0 {
                emit(app, Step::new(key, name, "seatOpened"));
            }
            waiting(false);
            return true;
        }
        if looked == 0 {
            waiting(true);
        }
        if let Some(s) = &seat {
            emit(app, Step { playing: s.playing(), max: s.max_players(), ..Step::new(key, name, "seat") });
        }
        // Looked at each second, so a stop lets the card go at once.
        for _ in 0..SEAT_EVERY.as_secs() {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !still() {
                emit(app, Step::new(key, name, "stopped"));
                return false;
            }
        }
    }
    unreachable!()
}

/// The server the account's new client joined: from its log, which says it as soon as it joins,
/// or else from its presence. None after 90 s.
async fn first_server(app: &AppHandle, key: &str) -> Option<(u64, String)> {
    let start = Instant::now();
    while start.elapsed() < FIRST_SERVER {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Some(s) = crate::accounts::client_server(app.clone(), key.to_string()) {
            return Some(s.into());
        }
    }
    let p = crate::accounts::presence(app.clone()).await.ok()?.remove(key)?;
    Some((p.place_id?, p.job_id?))
}

#[tauri::command]
pub async fn launch_run(app: AppHandle, plan: Plan) -> Result<Summary, String> {
    run(&app, plan).await
}

/// Stops an account's wait for room.
#[tauri::command]
pub fn launch_stop(account: String) {
    if let Some(w) = WAITING.lock().unwrap().as_mut() {
        w.remove(&account);
    }
}

/// The accounts waiting for room or opening now, for a window that loads in the middle of a run.
#[tauri::command]
pub fn launch_busy() -> Vec<String> {
    BUSY.lock().unwrap().as_ref().map(|b| b.iter().cloned().collect()).unwrap_or_default()
}
