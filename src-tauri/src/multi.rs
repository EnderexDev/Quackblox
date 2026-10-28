// The multi: Roblox lets one client run per PC by owning a named mutex. If Quackblox owns it
// first, no client can, and each client launched by link (the site's Play button, or Play here)
// opens next to the others instead of closing them. Measured 27/10/26 (Notion, Quackblox >
// Memória): the Roblox app opened from its own icon uses another lock and stays single.
use std::sync::atomic::{AtomicBool, Ordering};
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
        loop {
            let handle = unsafe { CreateMutexW(None, true, w!("ROBLOX_singletonMutex")) };
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
            if crate::settings::AUTO_CLOSE.load(Ordering::Relaxed) || CLOSE_NOW.swap(false, Ordering::Relaxed) {
                let closed = close_roblox();
                if closed > 0 {
                    let _ = app.emit("roblox-closed", closed);
                    std::thread::sleep(Duration::from_millis(700));
                    continue;
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

static CLOSE_NOW: AtomicBool = AtomicBool::new(false);

/// Ends every Roblox player process (the window and the one left in the tray after closing it),
/// and says how many ended.
fn close_roblox() -> usize {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "RobloxPlayerBeta.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        // taskkill prints one line per process ended; the words follow the Windows language.
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.trim().is_empty()).count().max(1),
        _ => 0,
    }
}

#[tauri::command]
pub fn multi_status() -> bool {
    active()
}

/// The "Fechar agora" of the waiting pill: closes the Roblox in the multi's next pass.
#[tauri::command]
pub fn multi_close_now() {
    CLOSE_NOW.store(true, Ordering::Relaxed);
}
