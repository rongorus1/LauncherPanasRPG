use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    error::AppError,
    ports::{inbound::CheckLauncherUpdate, outbound::LauncherUpdater},
};
use crate::domain::LauncherUpdate;

pub struct CheckLauncherUpdateUseCase {
    updater: Arc<dyn LauncherUpdater>,
}

impl CheckLauncherUpdateUseCase {
    pub fn new(updater: Arc<dyn LauncherUpdater>) -> Self {
        Self { updater }
    }
}

#[async_trait]
impl CheckLauncherUpdate for CheckLauncherUpdateUseCase {
    async fn execute(&self) -> Result<Option<LauncherUpdate>, AppError> {
        Ok(self.updater.check().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        error::{AppErrorKind, PortError},
        use_cases::testing::FakeUpdater,
    };

    #[tokio::test]
    async fn reports_the_newer_version() {
        let updater = FakeUpdater::with_update("0.2.0");
        let found = CheckLauncherUpdateUseCase::new(updater)
            .execute()
            .await
            .unwrap();
        assert_eq!(found.unwrap().version, "0.2.0");
    }

    #[tokio::test]
    async fn up_to_date_is_not_an_error() {
        let updater = FakeUpdater::up_to_date();
        assert_eq!(
            CheckLauncherUpdateUseCase::new(updater)
                .execute()
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn network_failures_keep_their_kind() {
        let updater = FakeUpdater::failing(PortError::Network("offline".into()));
        let error = CheckLauncherUpdateUseCase::new(updater)
            .execute()
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Network);
    }
}
