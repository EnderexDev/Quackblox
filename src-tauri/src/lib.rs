// The phone page's texts are one json! (phone.rs), past the default depth of 128.
#![recursion_limit = "256"]

/// A text for the user in the language picked (settings.rs): `tr!("pt", "en", args...)`, both
/// format strings taking the same arguments. Defined before the modules so all of them see it.
macro_rules! tr {
    ($pt:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if crate::settings::english() { format!($en $(, $arg)*) } else { format!($pt $(, $arg)*) }
    };
}

mod accounts;
mod app_account;
mod audio;
mod capture;
mod clients;
mod desktop;
mod discord;
mod history;
mod hotkeys;
mod launch;
mod logs;
mod multi;
mod organize;
mod phone;
mod roblox;
mod settings;
mod setup;
mod site;
mod stats;
mod transfer;
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
        // The accounts file's save and open dialogs (transfer.rs), called from Rust only.
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(accounts::Store::load(app.handle()));
            // Before the multi, which reads auto_close.
            settings::load(app.handle());
            multi::start(app.handle().clone());
            clients::start(app.handle());
            hotkeys::start(app.handle().clone());
            vigia::start(app.handle().clone());
            organize::start(app.handle().clone());
            site::clean_up(app.handle());
            phone::apply(app.handle());
            discord::start(app.handle().clone());
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
            accounts::accounts_set_label,
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
            clients::clients_set_mute,
            clients::clients_set_restart,
            clients::clients_save_place,
            clients::clients_forget_place,
            clients::clients_apply_places,
            clients::clients_close,
            history::history_get,
            history::history_clear,
            transfer::transfer_export,
            transfer::transfer_import,
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
            launch::launch_run,
            launch::launch_stop,
            launch::launch_busy,
            organize::organize_get,
            organize::organize_set,
            organize::profile_run,
            site::site_open,
            site::site_open_list,
            capture::capture_frame,
            capture::capture_print,
            capture::capture_reveal,
            phone::phone_info,
            phone::phone_new_key,
            phone::phone_games,
            settings::rich_presence_available,
        ])
        .run(context)
        .expect("error while running tauri application");
}
