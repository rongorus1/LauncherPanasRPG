use std::sync::Mutex;

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{DownloadObserver, LauncherUpdater, PortResult},
    },
    domain::LauncherUpdate,
};
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

/// Self-update through the Tauri updater plugin (signed releases published on GitHub).
pub struct TauriLauncherUpdater {
    app: AppHandle,
    pending: Mutex<Option<Update>>,
}

impl TauriLauncherUpdater {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            pending: Mutex::new(None),
        }
    }
}

fn port_error(error: tauri_plugin_updater::Error) -> PortError {
    match error {
        tauri_plugin_updater::Error::Reqwest(e) => PortError::Network(e.to_string()),
        other => PortError::Other(other.to_string()),
    }
}

#[async_trait]
impl LauncherUpdater for TauriLauncherUpdater {
    async fn check(&self) -> PortResult<Option<LauncherUpdate>> {
        let update = self
            .app
            .updater()
            .map_err(port_error)?
            .check()
            .await
            .map_err(port_error)?;
        let found = update.as_ref().map(|update| LauncherUpdate {
            version: update.version.clone(),
            notes: update.body.clone(),
        });
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = update;
        Ok(found)
    }

    async fn download_and_install(&self, observer: &dyn DownloadObserver) -> PortResult<()> {
        let update = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or_else(|| PortError::NotFound("no pending launcher update".into()))?;
        let mut downloaded = 0u64;
        update
            .download_and_install(
                |chunk, total| {
                    downloaded += chunk as u64;
                    observer.on_progress(downloaded, total);
                },
                || {},
            )
            .await
            .map_err(port_error)
    }

    fn restart(&self) {
        self.app.restart();
    }
}
