// Living on the Windows desktop: the tray icon and its menu, closing to the tray (the multi
// only works while Quackblox runs), starting with Windows straight to the tray.
use auto_launch::{AutoLaunch, AutoLaunchBuilder, WindowsEnableMode};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, WindowEvent};

/// Passed by the Windows startup entry: start in the tray, without the window.
pub const MINIMIZED_ARG: &str = "--minimized";

static QUITTING: AtomicBool = AtomicBool::new(false);

pub struct TrayStartup(CheckMenuItem<tauri::Wry>);

fn launcher() -> Result<AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    AutoLaunchBuilder::new()
        .set_app_name("Quackblox")
        .set_app_path(&exe.display().to_string())
        .set_args(&[MINIMIZED_ARG])
        .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
        .build()
        .map_err(|e| e.to_string())
}

fn start_with_windows() -> bool {
    launcher().and_then(|l| l.is_enabled().map_err(|e| e.to_string())).unwrap_or(false)
}

/// `by` names the caller in the log: on 28/10/26 the option turned on once with no click seen,
/// and this line says who did it if it happens again.
fn set_start_with_windows(on: bool, by: &str) -> Result<(), String> {
    eprintln!("startup: set to {on} by {by}");
    let l = launcher()?;
    if on { l.enable() } else { l.disable() }.map_err(|e| e.to_string())
}

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn setup(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Mostrar o Quackblox", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(app, "autostart", "Iniciar com o Windows", true, start_with_windows(), None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair do Quackblox", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &autostart, &PredefinedMenuItem::separator(app)?, &quit])?;
    let autostart_item = autostart.clone();
    // Kept so the window's switch can move the tray's check mark too.
    app.manage(TrayStartup(autostart.clone()));
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Quackblox")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "autostart" => {
                let _ = set_start_with_windows(!start_with_windows(), "tray menu");
                let now = start_with_windows();
                let _ = autostart_item.set_checked(now);
                // The window's switch follows, even while it is on screen.
                let _ = app.emit("startup", now);
            }
            "quit" => {
                QUITTING.store(true, Ordering::Relaxed);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;

    if !std::env::args().any(|a| a == MINIMIZED_ARG) {
        show_window(app.handle());
    }
    Ok(())
}

/// Closing the main window hides it: the multi has to stay on.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        if !QUITTING.load(Ordering::Relaxed) {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}

#[tauri::command]
pub fn startup_get() -> bool {
    start_with_windows()
}

#[tauri::command]
pub fn startup_set(tray: tauri::State<TrayStartup>, on: bool) -> Result<bool, String> {
    set_start_with_windows(on, "window switch")?;
    let now = start_with_windows();
    let _ = tray.0.set_checked(now);
    Ok(now)
}
