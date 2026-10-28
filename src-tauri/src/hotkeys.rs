// Global hotkeys: Ctrl+Shift+1 to 9 bring the client of the account on that card forward (1 is
// the Roblox app's card), Ctrl+Shift+0 minimizes every Roblox but the main one. Not Ctrl+Alt:
// on a Brazilian ABNT2 keyboard AltGr is Ctrl+Alt, and AltGr+1 types ¹. Each key acts on one
// window only; nothing is ever sent to the game.
use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{AppHandle, Emitter};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY};
use windows::Win32::Foundation::{LPARAM, WPARAM};

/// The thread that owns the hotkeys (a hotkey belongs to the thread that registered it).
static THREAD: AtomicU32 = AtomicU32::new(0);
/// Id 0 is Ctrl+Shift+0; ids 1 to 9 are the cards.
const KEYS: u32 = 10;

fn register(app: &AppHandle) {
    let mut failed = vec![];
    for id in 0..KEYS {
        let vk = b'0' as u32 + id;
        if unsafe { RegisterHotKey(None, id as i32, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, vk) }.is_err() {
            failed.push(format!("Ctrl+Shift+{id}"));
        }
    }
    // Another program holds them (the installed Quackblox beside the Dev build, a game overlay):
    // the others still work, and the window says which ones did not.
    if !failed.is_empty() {
        eprintln!("hotkeys: taken by another program: {}", failed.join(", "));
        let _ = app.emit("hotkeys-failed", failed.join(", "));
    }
}

fn unregister() {
    for id in 0..KEYS {
        let _ = unsafe { UnregisterHotKey(None, id as i32) };
    }
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut msg = MSG::default();
        // Creates the thread's message queue before anyone posts to it.
        let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
        THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        if crate::settings::hotkeys() {
            register(&app);
        }
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            match msg.message {
                WM_HOTKEY => match msg.wParam.0 {
                    0 => {
                        crate::clients::minimize_others(&app);
                    }
                    n => crate::clients::focus_nth(&app, n),
                },
                WM_APP => {
                    unregister();
                    if msg.wParam.0 == 1 {
                        register(&app);
                    }
                }
                _ => {}
            }
        }
    });
}

/// Turns the hotkeys on or off, from the setting's switch.
pub fn set(on: bool) {
    let thread = THREAD.load(Ordering::SeqCst);
    if thread != 0 {
        let _ = unsafe { PostThreadMessageW(thread, WM_APP, WPARAM(on as usize), LPARAM(0)) };
    }
}
