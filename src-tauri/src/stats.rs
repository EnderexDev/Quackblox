// Time in game per Roblox user and per place, in stats.json in the app's data folder. Counted by
// the clients' watcher (clients.rs) once a second for each client its log shows inside a game:
// the loading screen and a drop's dialog do not count. Kept by user id, not by card: the Roblox
// app's card is whoever is logged in there.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Stats {
    /// When counting started, Unix seconds.
    since: u64,
    /// Seconds in game, by user id and then by place id (both as text, for JSON).
    users: HashMap<String, HashMap<String, u64>>,
}

struct State {
    path: PathBuf,
    stats: Stats,
    /// Milliseconds not yet a whole second, per user and place.
    rest: HashMap<(u64, u64), u64>,
    last: Option<Instant>,
    saved: Instant,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
const SAVE_EVERY: Duration = Duration::from_secs(60);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load(app: &AppHandle) -> State {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = dir.join("stats.json");
    let mut stats: Stats = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if stats.since == 0 {
        stats.since = now();
    }
    State { path, stats, rest: HashMap::new(), last: None, saved: Instant::now() }
}

fn save(s: &State) {
    let Ok(text) = serde_json::to_string_pretty(&s.stats) else { return };
    let tmp = s.path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &s.path)) {
        eprintln!("stats: stats.json not saved: {e}");
    }
}

/// Adds the time since the last call to each (user, place) in a game now. A gap over 5 s (the
/// PC asleep) counts as 5 s.
pub fn add(app: &AppHandle, in_game: &[(u64, u64)]) {
    let mut guard = STATE.lock().unwrap();
    let s = guard.get_or_insert_with(|| load(app));
    let ms = s.last.map_or(0, |t| t.elapsed().as_millis().min(5000) as u64);
    s.last = Some(Instant::now());
    for &(user, place) in in_game {
        let rest = s.rest.entry((user, place)).or_default();
        *rest += ms;
        let whole = *rest / 1000;
        *rest %= 1000;
        if whole > 0 {
            *s.stats.users.entry(user.to_string()).or_default().entry(place.to_string()).or_default() += whole;
        }
    }
    if s.saved.elapsed() >= SAVE_EVERY {
        s.saved = Instant::now();
        save(s);
    }
}

#[tauri::command]
pub fn stats_get(app: AppHandle) -> Stats {
    let mut guard = STATE.lock().unwrap();
    guard.get_or_insert_with(|| load(&app)).stats.clone()
}

/// Starts the count over from now.
#[tauri::command]
pub fn stats_reset(app: AppHandle) -> Stats {
    let mut guard = STATE.lock().unwrap();
    let s = guard.get_or_insert_with(|| load(&app));
    s.stats = Stats { since: now(), users: HashMap::new() };
    s.rest.clear();
    // The time before the reset is not counted after it.
    s.last = Some(Instant::now());
    save(s);
    s.stats.clone()
}
