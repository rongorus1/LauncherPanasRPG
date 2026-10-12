//! Inbound ports: what the application can do. Driving adapters (Tauri commands) call these.

use async_trait::async_trait;

use crate::application::{
    dto::{AccountView, LauncherState, PackStatus},
    error::AppError,
};
use crate::domain::{LauncherUpdate, Settings};

#[async_trait]
pub trait GetLauncherState: Send + Sync {
    async fn execute(&self) -> Result<LauncherState, AppError>;
}

#[async_trait]
pub trait SaveOfflineAccount: Send + Sync {
    async fn execute(&self, username: String) -> Result<AccountView, AppError>;
}

#[async_trait]
pub trait SaveSettings: Send + Sync {
    async fn execute(&self, settings: Settings) -> Result<Settings, AppError>;
}

/// Compares the installed pack with the latest published release.
#[async_trait]
pub trait CheckPackUpdate: Send + Sync {
    async fn execute(&self) -> Result<PackStatus, AppError>;
}

/// Installs the latest pack or updates the installed one. Progress goes out as `LauncherEvent`s.
#[async_trait]
pub trait InstallPack: Send + Sync {
    async fn execute(&self) -> Result<(), AppError>;
}

/// Starts the game (offline). Log lines and exit go out as `LauncherEvent`s.
#[async_trait]
pub trait PlayGame: Send + Sync {
    async fn execute(&self) -> Result<(), AppError>;
}

#[async_trait]
pub trait StopGame: Send + Sync {
    async fn execute(&self) -> Result<(), AppError>;
}

/// Looks for a newer launcher version. `None` means the launcher is up to date.
#[async_trait]
pub trait CheckLauncherUpdate: Send + Sync {
    async fn execute(&self) -> Result<Option<LauncherUpdate>, AppError>;
}

/// Downloads the pending launcher update, installs it and restarts. Progress goes out as `LauncherEvent`s.
#[async_trait]
pub trait InstallLauncherUpdate: Send + Sync {
    async fn execute(&self) -> Result<(), AppError>;
}
