// The drop history (phase 6): each drop, crash and failed join the watch told (vigia.rs), with the
// time, the account and Roblox's code, in drops.json in the app's data folder. The newest 500 are
// kept.
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const KEEP: usize = 500;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Unix seconds.
    pub at: u64,
    /// "app" or a saved account's id, as everywhere.
    pub account: String,
    pub user: Option<u64>,
    pub name: String,
    /// "dropped", "crashed" or "joinFailed".
    pub kind: String,
    pub reason: Option<u32>,
    /// What `play` was given.
    pub game: String,
}

struct Store {
    path: PathBuf,
    list: Vec<Entry>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap();
    let store = guard.get_or_insert_with(|| {
        let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        let path = dir.join("drops.json");
        let list = match std::fs::read_to_string(&path) {
            Err(_) => vec![],
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                // Kept aside, so the next drop never writes over the history in it.
                let aside = path.with_extension(format!("broken-{}.json", crate::win::unix_now()));
                eprintln!("history: drops.json unreadable ({e}), moved to {}", aside.display());
                let _ = std::fs::rename(&path, &aside);
                vec![]
            }),
        };
        Store { path, list }
    });
    f(store)
}

fn save(store: &Store) {
    let Ok(text) = serde_json::to_string(&store.list) else { return };
    let tmp = store.path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &store.path)) {
        eprintln!("history: drops.json not saved: {e}");
    }
}

pub fn add(app: &AppHandle, entry: Entry) {
    with(app, |s| {
        s.list.push(entry);
        let extra = s.list.len().saturating_sub(KEEP);
        s.list.drain(..extra);
        save(s);
    });
}

/// Newest first.
#[tauri::command]
pub fn history_get(app: AppHandle) -> Vec<Entry> {
    with(&app, |s| s.list.iter().rev().cloned().collect())
}

#[tauri::command]
pub fn history_clear(app: AppHandle) {
    with(&app, |s| {
        s.list.clear();
        save(s);
    });
}
