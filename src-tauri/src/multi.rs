// The multi: Roblox lets one client run per PC by owning a named mutex. If Quackblox owns it
// first, no client can, and each client launched by link (the site's Play button, or Play here)
// opens next to the others instead of closing them. Measured 27/10/26 (Notion, Quackblox >
// Memória): the Roblox app opened from its own icon uses another lock and stays single.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, ReleaseMutex, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    WaitForSingleObject,
};
use windows::core::{PCWSTR, w};

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// True while Quackblox owns the lock.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Takes the lock on a thread of its own, which keeps it for the life of the app (a mutex
/// belongs to the thread that took it). While a Roblox opened before Quackblox holds it, it
/// retries every 2 s; the window hears "multi" each time the state changes.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut first = true;
        // Passes that ended Roblox processes and still found the lock held: past 3, something
        // else holds it, and the automatic close stops instead of looping forever.
        let mut futile = 0;
        let mut open_errors = 0;
        loop {
            let handle = unsafe { CreateMutexW(None, true, w!("ROBLOX_singletonMutex")) };
            // The lock could not even be opened: nothing says a Roblox holds it, so nothing is closed.
            if handle.is_err() {
                // Counted apart from the closing passes, so a passing error never turns the
                // automatic close off. Three in a row: the window says it is stuck. A click on
                // "Fechar agora" starts the count over.
                open_errors += 1;
                if CLOSE_NOW.swap(false, Ordering::Relaxed) {
                    open_errors = 0;
                    STUCK.store(false, Ordering::Relaxed);
                }
                if open_errors >= 3 && !STUCK.swap(true, Ordering::Relaxed) {
                    let _ = app.emit("multi-stuck", true);
                }
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
            open_errors = 0;
            let taken = match handle {
                // It already exists: a Roblox may own it, or only keep a handle to one whose
                // owner is gone (a Roblox opened while an earlier Quackblox held it). A zero wait
                // takes it in the second case, as the Account Manager does.
                Ok(h) if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS => {
                    let mut r = unsafe { WaitForSingleObject(h, 0) };
                    // The update window holds it for the clients (bridge): this Quackblox gets
                    // in line before the window leaves, instead of closing them as a Roblox
                    // that blocks the multi.
                    // Never on to the close while the window holds it: an update that failed
                    // keeps its window open, and the window hands it over when it hears this.
                    if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED && updater_holding() {
                        signal(APP_WAITING);
                        loop {
                            r = unsafe { WaitForSingleObject(h, 2000) };
                            if r == WAIT_OBJECT_0 || r == WAIT_ABANDONED || !updater_holding() {
                                break;
                            }
                        }
                    }
                    let ours = r == WAIT_OBJECT_0 || r == WAIT_ABANDONED;
                    if !ours {
                        let _ = unsafe { CloseHandle(h) };
                    }
                    ours
                }
                Ok(_) => true,
                Err(_) => false,
            };
            if ACTIVE.swap(taken, Ordering::Relaxed) != taken || first {
                let _ = app.emit("multi", taken);
            }
            first = false;
            if taken {
                // Holding it is the whole job: this thread only stays alive.
                loop {
                    std::thread::park();
                }
            }
            // A Roblox holds it. With the setting on (or a click on "Fechar agora"), it is
            // closed; its lock goes with it and the next pass takes it.
            let asked = CLOSE_NOW.swap(false, Ordering::Relaxed);
            if asked {
                futile = 0;
                STUCK.store(false, Ordering::Relaxed);
            }
            let auto = crate::settings::AUTO_CLOSE.load(Ordering::Relaxed) && futile < 3;
            if auto || asked {
                let closed = close_roblox();
                if closed > 0 {
                    futile += 1;
                    CLOSED.fetch_add(closed, Ordering::Relaxed);
                    let _ = app.emit("roblox-closed", closed);
                    std::thread::sleep(Duration::from_millis(700));
                    continue;
                }
                // Nothing to close and still held: not a Roblox player, so the automatic close
                // gives up and the window offers the button instead.
                futile = 3;
            }
            if futile >= 3 && !STUCK.swap(true, Ordering::Relaxed) {
                let _ = app.emit("multi-stuck", true);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

// The lock across an update. Measured 05/11/26 (Notion, Quackblox > Memória): each Roblox
// client tries for the lock every 250 ms; the one that takes it (its owner gone) closes within
// 2 s, and the next takes it from that one, until no client is left. Between the app closing
// for an update and the new one starting, nobody held it, and every client closed. So the lock
// goes down the chain old app, update window, new app, each one waiting for it before the one
// that holds it leaves: a wait that never stopped is served before the clients' 250 ms ones,
// which start over each time (25 of 25 in the test of 05/11/26).
const UPDATER_WAITING: PCWSTR = w!("StarX_Quackblox_UpdaterWaiting");
const UPDATER_HOLDING: PCWSTR = w!("StarX_Quackblox_UpdaterHolding");
const APP_WAITING: PCWSTR = w!("StarX_Quackblox_AppWaiting");
/// How long the one leaving waits past the other's signal, so the other's wait has started.
const HANDOVER_GAP: Duration = Duration::from_millis(500);
/// True in the update window once it holds the lock.
static BRIDGING: AtomicBool = AtomicBool::new(false);
/// True in the update window once it let the lock go to the new app.
static HANDED: AtomicBool = AtomicBool::new(false);

/// A named event (manual reset), created unsignaled when it was not there yet.
struct Event(HANDLE);

// A kernel handle, usable from any thread.
unsafe impl Send for Event {}

impl Event {
    fn new(name: PCWSTR) -> Option<Event> {
        unsafe { CreateEventW(None, true, false, name) }.ok().map(Event)
    }

    /// Waits for it to be signaled, up to `ms`.
    fn wait(&self, ms: u32) -> bool {
        (unsafe { WaitForSingleObject(self.0, ms) }) == WAIT_OBJECT_0
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn signal(name: PCWSTR) {
    if let Some(e) = Event::new(name) {
        let _ = unsafe { SetEvent(e.0) };
    }
}

fn updater_holding() -> bool {
    unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, UPDATER_HOLDING) }.ok().map(Event).is_some()
}

/// In the app, before starting the update window: Some while this app holds the lock, to wait
/// on with `wait_for_updater` once the window started.
pub fn before_update() -> Option<impl FnOnce() + Send> {
    if !active() {
        return None;
    }
    let waiting = Event::new(UPDATER_WAITING)?;
    // Left signaled by an update window still open from before: this one has to say it again.
    let _ = unsafe { ResetEvent(waiting.0) };
    Some(move || {
        // Up to 10 s for the window to start; past that the app closes as before the bridge.
        if waiting.wait(10_000) {
            std::thread::sleep(HANDOVER_GAP);
        }
    })
}

/// In the update window, first thing: when the app that started it holds the lock, waits for it
/// on a thread of its own and keeps it until the window closes.
pub fn bridge() {
    let Ok(waiting) = (unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE | EVENT_MODIFY_STATE, false, UPDATER_WAITING) }).map(Event) else {
        return;
    };
    std::thread::spawn(move || {
        // The whole Event moves here, not only its handle.
        let waiting = waiting;
        let Ok(lock) = (unsafe { CreateMutexW(None, false, w!("ROBLOX_singletonMutex")) }) else { return };
        let _ = unsafe { SetEvent(waiting.0) };
        let r = unsafe { WaitForSingleObject(lock, INFINITE) };
        if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
            return;
        }
        // The new app looks for `holding` to know whose lock it is, and signals `app_waiting`
        // once it waits for it: opened by NSIS after the update, or by hand after one that
        // failed, with this window still open.
        let holding = Event::new(UPDATER_HOLDING);
        let app_waiting = Event::new(APP_WAITING);
        BRIDGING.store(true, Ordering::SeqCst);
        let Some(app_waiting) = app_waiting else {
            // No way to hear the new app: the lock goes when this window closes.
            loop {
                std::thread::park();
            }
        };
        app_waiting.wait(INFINITE);
        std::thread::sleep(HANDOVER_GAP);
        // Let go before `holding` goes: the new app, already waiting, takes it now.
        let _ = unsafe { ReleaseMutex(lock) };
        drop(holding);
        HANDED.store(true, Ordering::SeqCst);
        loop {
            std::thread::park();
        }
    });
}

/// In the update window, before it closes: while it holds the lock, waits (up to a minute) for
/// bridge to hand it to the new app. Blocks: call it off the async runtime.
pub fn hand_over() {
    let start = std::time::Instant::now();
    while BRIDGING.load(Ordering::SeqCst) && !HANDED.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

static CLOSE_NOW: AtomicBool = AtomicBool::new(false);
/// True once the automatic close gave up.
static STUCK: AtomicBool = AtomicBool::new(false);
/// Roblox processes ended so far, for a window that loads after the fact.
static CLOSED: AtomicUsize = AtomicUsize::new(0);

/// Ends every Roblox player process (the window and the one left in the tray after closing it),
/// and says how many ended.
fn close_roblox() -> usize {
    // The dev build would find the installed Quackblox holding the lock and close the Roblox
    // windows it opened.
    if cfg!(debug_assertions) {
        eprintln!("multi: the dev build does not close Roblox");
        return 0;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // By full path: a bare name is searched in the app's own folder first.
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let out = std::process::Command::new(format!(r"{system}\System32\taskkill.exe"))
        .args(["/F", "/IM", "RobloxPlayerBeta.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let closed = match out {
        // taskkill prints one line per process ended; the words follow the Windows language.
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.trim().is_empty()).count().max(1),
        _ => 0,
    };
    if closed > 0 {
        crate::ends::note(&format!("{closed} Roblox: fechados pelo multi, um Roblox segurava a trava (fechar automático ou \"Fechar agora\")"));
    }
    closed
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    active: bool,
    stuck: bool,
    /// Roblox processes the multi has ended since the app started.
    closed: usize,
}

#[tauri::command]
pub fn multi_status() -> Status {
    Status { active: active(), stuck: STUCK.load(Ordering::Relaxed), closed: CLOSED.load(Ordering::Relaxed) }
}

/// The "Fechar agora" of the waiting pill: closes the Roblox in the multi's next pass.
#[tauri::command]
pub fn multi_close_now() {
    CLOSE_NOW.store(true, Ordering::Relaxed);
}
