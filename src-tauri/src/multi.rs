// The multi: Roblox lets one client run per PC by owning a named mutex. If Quackblox owns it
// first, no client can, and each client launched by link (the site's Play button, or Play here)
// opens next to the others instead of closing them. Measured 27/10/26 (Notion, Quackblox >
// Memória): the Roblox app opened from its own icon uses another lock and stays single.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
use windows::core::w;

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
                    let r = unsafe { WaitForSingleObject(h, 0) };
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
    match out {
        // taskkill prints one line per process ended; the words follow the Windows language.
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.trim().is_empty()).count().max(1),
        _ => 0,
    }
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
