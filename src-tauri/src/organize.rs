// Groups of accounts, one-click profiles and the schedule (phase 4), in organize.json in the app's
// data folder. A file of its own and not settings.json: the window sends every setting at each
// change, and these lists would ride along on every switch. Accounts are named by their key ("app"
// or a saved account's id), as everywhere; one removed since is left out when the list opens.
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::launch;
use crate::roblox::Game;

/// A slot is opened up to 5 min late (the PC busy, the app started a moment after); later than
/// that it waits for its next day: the PC was asleep or off, and a schedule opening accounts hours
/// late is not what he set.
const LATE: u32 = 5;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub accounts: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game: Game,
    pub accounts: Vec<String>,
    /// The accounts in economy mode; the profile's others are taken out of it.
    #[serde(default)]
    pub eco: Vec<String>,
    #[serde(default)]
    pub grid: bool,
    #[serde(default)]
    pub same_server: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub id: String,
    /// The profile it opens.
    pub profile: String,
    /// Minutes after midnight, local time.
    pub minute: u32,
    /// Days of the week it runs on, 0 for Sunday; empty for every day.
    #[serde(default)]
    pub days: Vec<u8>,
    #[serde(default)]
    pub on: bool,
    /// The local day it last ran (YYYY-MM-DD), so a slot runs once a day.
    #[serde(default)]
    pub last: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Lists {
    pub groups: Vec<Group>,
    pub profiles: Vec<Profile>,
    pub schedule: Vec<Slot>,
}

struct Store {
    path: PathBuf,
    lists: Lists,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap();
    let store = guard.get_or_insert_with(|| {
        let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        let path = dir.join("organize.json");
        let lists = match std::fs::read_to_string(&path) {
            Err(_) => Lists::default(),
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                // Kept aside, so the next save never writes over the lists in it.
                let aside = path.with_extension(format!("broken-{}.json", crate::win::unix_now()));
                eprintln!("organize: organize.json unreadable ({e}), moved to {}", aside.display());
                let _ = std::fs::rename(&path, &aside);
                Lists::default()
            }),
        };
        Store { path, lists }
    });
    f(store)
}

fn save(store: &Store) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&store.lists).map_err(|e| e.to_string())?;
    let tmp = store.path.with_extension("json.tmp");
    std::fs::write(&tmp, text)
        .and_then(|_| std::fs::rename(&tmp, &store.path))
        .map_err(|e| tr!("não consegui salvar no disco: {e}", "couldn't save to disk: {e}"))
}

/// What `play` takes for a game: the id, or a link that also names the server or the private
/// server (the window's `gameArg`).
pub fn game_arg(g: &Game) -> String {
    match (&g.link_code, &g.job_id) {
        (Some(code), _) => format!("https://www.roblox.com/games/{}?privateServerLinkCode={code}", g.place_id),
        (None, Some(job)) => format!("https://www.roblox.com/games/start?placeId={}&gameInstanceId={job}", g.place_id),
        (None, None) => g.place_id.to_string(),
    }
}

#[tauri::command]
pub fn organize_get(app: AppHandle) -> Lists {
    with(&app, |s| s.lists.clone())
}

/// The window sends the three lists whole. A slot keeps the day it last ran from the disk: the
/// window's copy may be older than a run that happened meanwhile.
#[tauri::command]
pub fn organize_set(app: AppHandle, mut lists: Lists) -> Result<Lists, String> {
    for g in &lists.groups {
        if g.name.trim().is_empty() {
            return Err(tr!("dá um nome pro grupo", "give the group a name"));
        }
    }
    for p in &lists.profiles {
        if p.name.trim().is_empty() {
            return Err(tr!("dá um nome pro perfil", "give the profile a name"));
        }
        if p.accounts.is_empty() {
            return Err(tr!("o perfil {} não tem nenhuma conta", "the profile {} has no account", p.name));
        }
    }
    for slot in &lists.schedule {
        if slot.minute >= 24 * 60 {
            return Err(tr!("horário inválido", "invalid time"));
        }
        if !lists.profiles.iter().any(|p| p.id == slot.profile) {
            return Err(tr!("cada horário da agenda precisa de um perfil", "each schedule time needs a profile"));
        }
    }
    with(&app, |s| {
        for slot in lists.schedule.iter_mut() {
            if let Some(old) = s.lists.schedule.iter().find(|o| o.id == slot.id) {
                slot.last = old.last.clone();
            }
        }
        let before = std::mem::replace(&mut s.lists, lists);
        save(s).inspect_err(|_| s.lists = before)?;
        Ok(s.lists.clone())
    })
}

fn plan_of(p: &Profile) -> launch::Plan {
    launch::Plan {
        accounts: p.accounts.clone(),
        game: game_arg(&p.game),
        game_name: p.game.name.clone(),
        same_server: p.same_server,
        eco: Some(p.eco.clone()),
        grid: p.grid,
    }
}

/// Opens a profile: its accounts in its game, eco set as it says, the grid after.
#[tauri::command]
pub async fn profile_run(app: AppHandle, id: String) -> Result<launch::Summary, String> {
    let profile = with(&app, |s| s.lists.profiles.iter().find(|p| p.id == id).cloned()).ok_or_else(|| tr!("esse perfil não existe mais", "that profile no longer exists"))?;
    launch::run(&app, plan_of(&profile)).await
}

/// The schedule's clock: every 20 s, each slot that is on and due today opens its profile once.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let now = crate::win::local_now();
            let due: Vec<(Slot, Option<Profile>)> = with(&app, |s| {
                let mut due = vec![];
                for slot in s.lists.schedule.iter_mut() {
                    let today = slot.days.is_empty() || slot.days.contains(&now.weekday);
                    let late = now.minute.checked_sub(slot.minute);
                    if slot.on && today && slot.last != now.day && late.is_some_and(|m| m <= LATE) {
                        slot.last = now.day.clone();
                        due.push((slot.clone(), s.lists.profiles.iter().find(|p| p.id == slot.profile).cloned()));
                    }
                }
                if !due.is_empty() {
                    if let Err(e) = save(s) {
                        // Kept in memory: the slot still runs once today.
                        eprintln!("organize: schedule's last run not saved: {e}");
                    }
                }
                due
            });
            for (slot, profile) in due {
                let _ = app.emit("schedule-ran", slot.id.clone());
                let Some(profile) = profile else { continue };
                let app = app.clone();
                tauri::async_runtime::spawn(async move { scheduled(app, profile).await });
            }
        }
    });
}

async fn scheduled(app: AppHandle, profile: Profile) {
    let name = profile.name.clone();
    crate::vigia::alert(&app, &tr!("Agenda: abrindo o perfil {name}.", "Schedule: opening the profile {name}."));
    match launch::run(&app, plan_of(&profile)).await {
        Err(e) => crate::vigia::alert(&app, &tr!("Agenda: o perfil {name} não abriu: {e}.", "Schedule: the profile {name} didn't open: {e}.")),
        Ok(s) if !s.failed.is_empty() => crate::vigia::alert(
            &app,
            &tr!("Agenda, perfil {name}: {} aberta(s); não abriu: {}.", "Schedule, profile {name}: {} opened; didn't open: {}.", s.opened, s.failed.join("; ")),
        ),
        Ok(_) => {}
    }
}
