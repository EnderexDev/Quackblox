/// A text for the user in the language picked (settings.rs): `tr!("pt", "en", args...)`, both
/// format strings taking the same arguments. Defined before the modules so all of them see it.
macro_rules! tr {
    ($pt:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if crate::settings::english() { format!($en $(, $arg)*) } else { format!($pt $(, $arg)*) }
    };
}

mod accounts;
mod app_account;
mod clients;
mod desktop;
mod hotkeys;
mod logs;
mod multi;
mod roblox;
mod settings;
mod setup;
mod stats;
mod vigia;
mod win;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    // Quackblox-Setup.exe or the update window: the installer instead of the app.
    if let Some(mode) = setup::mode() {
        return setup::run(context, mode);
    }
    setup::clean_up();
    tauri::Builder::default()
        // First: a second launch only brings this window back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| desktop::show_window(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            app.manage(accounts::Store::load(app.handle()));
            // Before the multi, which reads auto_close.
            settings::load(app.handle());
            multi::start(app.handle().clone());
            clients::start(app.handle());
            hotkeys::start(app.handle().clone());
            vigia::start(app.handle().clone());
            desktop::setup(app)?;
            Ok(())
        })
        .on_window_event(desktop::on_window_event)
        .invoke_handler(tauri::generate_handler![
            multi::multi_status,
            multi::multi_close_now,
            settings::settings_get,
            settings::settings_set,
            setup::update_in_window,
            accounts::accounts_list,
            accounts::accounts_add,
            accounts::accounts_remove,
            accounts::accounts_move,
            accounts::app_account,
            accounts::avatars,
            accounts::game_info,
            accounts::play,
            accounts::presence,
            accounts::servers,
            accounts::server_seat,
            accounts::friends_in_game,
            accounts::client_server,
            clients::clients_get,
            clients::clients_set_eco,
            clients::clients_set_rejoin,
            vigia::vigia_test_discord,
            vigia::vigia_test_windows,
            vigia::vigia_check_logins,
            stats::stats_get,
            stats::stats_reset,
            clients::clients_focus,
            clients::clients_minimize_others,
            clients::clients_grid,
            desktop::startup_get,
            desktop::startup_set,
        ])
        .run(context)
        .expect("error while running tauri application");
}
