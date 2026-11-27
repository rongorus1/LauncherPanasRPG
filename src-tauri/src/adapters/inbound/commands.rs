use launcher_core::{
    application::{
        diagnostics::{BackupInfo, DeviceDiagnostics, RestoreReport, ScanReport},
        dto::{AccountView, LauncherState, PackStatus},
        error::AppError,
    },
    domain::{LauncherUpdate, Settings},
};
use tauri::State;

use crate::composition::Services;

#[tauri::command]
pub async fn get_device_diagnostics(
    services: State<'_, Services>,
) -> Result<DeviceDiagnostics, AppError> {
    services.diagnostics.device().await
}

#[tauri::command]
pub async fn scan_installation(services: State<'_, Services>) -> Result<ScanReport, AppError> {
    services.diagnostics.scan().await
}

#[tauri::command]
pub async fn list_backups(services: State<'_, Services>) -> Result<Vec<BackupInfo>, AppError> {
    services.diagnostics.backups().await
}

#[tauri::command]
pub async fn create_backup(services: State<'_, Services>) -> Result<BackupInfo, AppError> {
    services.diagnostics.create_backup().await
}

#[tauri::command]
pub async fn restore_backup(
    services: State<'_, Services>,
    id: String,
) -> Result<RestoreReport, AppError> {
    services.diagnostics.restore_backup(id).await
}

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
