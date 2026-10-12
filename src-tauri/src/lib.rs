mod adapters;
mod composition;
mod env_file;

use tauri::Manager;

use adapters::inbound::commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let version = app.package_info().version.to_string();
            let services = composition::build_services(app.handle().clone(), data_dir, &version)?;
            app.manage(services);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_launcher_state,
            commands::save_offline_account,
            commands::save_settings,
            commands::check_pack_update,
            commands::install_pack,
            commands::play_game,
            commands::stop_game,
            commands::check_launcher_update,
            commands::install_launcher_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
