use launcher_core::{
    application::{
        dto::{AccountView, LauncherState, PackStatus},
        error::AppError,
    },
    domain::{LauncherUpdate, Settings},
};
use tauri::State;

use crate::composition::Services;

#[tauri::command]
pub async fn get_launcher_state(services: State<'_, Services>) -> Result<LauncherState, AppError> {
    services.get_launcher_state.execute().await
}

#[tauri::command]
pub async fn save_offline_account(
    services: State<'_, Services>,
    username: String,
) -> Result<AccountView, AppError> {
    services.save_offline_account.execute(username).await
}

#[tauri::command]
pub async fn save_settings(
    services: State<'_, Services>,
    settings: Settings,
) -> Result<Settings, AppError> {
    services.save_settings.execute(settings).await
}

#[tauri::command]
pub async fn check_pack_update(services: State<'_, Services>) -> Result<PackStatus, AppError> {
    services.check_pack_update.execute().await
}

#[tauri::command]
pub async fn install_pack(services: State<'_, Services>) -> Result<(), AppError> {
    services.install_pack.execute().await
}

#[tauri::command]
pub async fn play_game(services: State<'_, Services>) -> Result<(), AppError> {
    services.play_game.execute().await
}

#[tauri::command]
pub async fn stop_game(services: State<'_, Services>) -> Result<(), AppError> {
    services.stop_game.execute().await
}

#[tauri::command]
pub async fn check_launcher_update(
    services: State<'_, Services>,
) -> Result<Option<LauncherUpdate>, AppError> {
    services.check_launcher_update.execute().await
}

#[tauri::command]
pub async fn install_launcher_update(services: State<'_, Services>) -> Result<(), AppError> {
    services.install_launcher_update.execute().await
}
