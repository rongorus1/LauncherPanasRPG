use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    diagnostics::{BackupInfo, DeviceDiagnostics, RestoreReport, ScanReport},
    dto::Activity,
    error::AppError,
    ports::{inbound::DiagnoseLocalInstallation, outbound::LocalDiagnostics},
    use_cases::ActivityTracker,
};

pub struct DiagnoseLocalInstallationUseCase {
    diagnostics: Arc<dyn LocalDiagnostics>,
    activity: ActivityTracker,
}

impl DiagnoseLocalInstallationUseCase {
    pub fn new(diagnostics: Arc<dyn LocalDiagnostics>, activity: ActivityTracker) -> Self {
        Self {
            diagnostics,
            activity,
        }
    }
}

#[async_trait]
impl DiagnoseLocalInstallation for DiagnoseLocalInstallationUseCase {
    async fn device(&self) -> Result<DeviceDiagnostics, AppError> {
        Ok(self.diagnostics.device().await?)
    }

    async fn scan(&self) -> Result<ScanReport, AppError> {
        Ok(self.diagnostics.scan().await?)
    }

    async fn backups(&self) -> Result<Vec<BackupInfo>, AppError> {
        Ok(self.diagnostics.backups().await?)
    }

    async fn create_backup(&self) -> Result<BackupInfo, AppError> {
        let _guard = self.activity.try_begin(Activity::Installing)?;
        Ok(self.diagnostics.create_backup().await?)
    }

    async fn restore_backup(&self, id: String) -> Result<RestoreReport, AppError> {
        let _guard = self.activity.try_begin(Activity::Installing)?;
        Ok(self.diagnostics.restore_backup(id).await?)
    }
}
