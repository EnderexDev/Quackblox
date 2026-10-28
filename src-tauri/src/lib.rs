mod accounts;
mod app_account;
mod desktop;
mod multi;
mod roblox;
mod settings;
mod win;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First: a second launch only brings this window back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| desktop::show_window(app)))
        .setup(|app| {
            app.manage(accounts::Store::load(app.handle()));
            // Before the multi, which reads auto_close.
            settings::load(app.handle());
            multi::start(app.handle().clone());
            desktop::setup(app)?;
            Ok(())
        })
        .on_window_event(desktop::on_window_event)
        .invoke_handler(tauri::generate_handler![
            multi::multi_status,
            multi::multi_close_now,
            settings::settings_get,
            settings::settings_set,
            accounts::accounts_list,
            accounts::accounts_add,
            accounts::accounts_remove,
            accounts::accounts_move,
            accounts::app_account,
            accounts::avatars,
            accounts::game_info,
            accounts::play,
            desktop::startup_get,
            desktop::startup_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
