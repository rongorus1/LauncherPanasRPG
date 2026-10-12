use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    dto::PackStatus,
    error::AppError,
    ports::{
        inbound::CheckPackUpdate,
        outbound::{InstanceStorage, PackSource},
    },
};
use crate::domain::{InstalledPack, PackVersion};

pub struct CheckPackUpdateUseCase {
    storage: Arc<dyn InstanceStorage>,
    pack_source: Arc<dyn PackSource>,
}

impl CheckPackUpdateUseCase {
    pub fn new(storage: Arc<dyn InstanceStorage>, pack_source: Arc<dyn PackSource>) -> Self {
        Self {
            storage,
            pack_source,
        }
    }
}

/// Unlike the launcher state, an explicit check reports a network failure so the UI can say so.
#[async_trait]
impl CheckPackUpdate for CheckPackUpdateUseCase {
    async fn execute(&self) -> Result<PackStatus, AppError> {
        let installed = self.storage.load_installed_pack().await?;
        let latest = self.pack_source.latest_release().await?;
        Ok(pack_status(installed.as_ref(), Some(latest.version)))
    }
}

/// An update is only "available" over an existing install; a missing install is shown as "install".
pub(crate) fn pack_status(
    installed: Option<&InstalledPack>,
    latest: Option<PackVersion>,
) -> PackStatus {
    let update_available =
        matches!((installed, latest), (Some(pack), Some(latest)) if latest > pack.version);
    PackStatus {
        installed_version: installed.map(|pack| pack.version.to_string()),
        minecraft_version: installed.map(|pack| pack.minecraft_version.clone()),
        forge_version: installed.map(|pack| pack.forge_version.clone()),
        latest_version: latest.map(|version| version.to_string()),
        update_available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{error::AppErrorKind, use_cases::testing::FakeWorld};

    #[tokio::test]
    async fn reports_an_available_update() {
        let world = FakeWorld::new().with_installed("1.6.0");
        let status = CheckPackUpdateUseCase::new(world.clone(), world.clone())
            .execute()
            .await
            .unwrap();
        assert_eq!(status.installed_version.as_deref(), Some("1.6.0"));
        assert_eq!(status.latest_version.as_deref(), Some("1.7.2"));
        assert_eq!(status.forge_version.as_deref(), Some("47.4.10"));
        assert!(status.update_available);
    }

    #[tokio::test]
    async fn up_to_date_and_not_installed_are_not_updates() {
        let world = FakeWorld::new().with_installed("1.7.2");
        let status = CheckPackUpdateUseCase::new(world.clone(), world.clone())
            .execute()
            .await
            .unwrap();
        assert!(!status.update_available);

        let fresh = FakeWorld::new();
        let status = CheckPackUpdateUseCase::new(fresh.clone(), fresh.clone())
            .execute()
            .await
            .unwrap();
        assert_eq!(status.installed_version, None);
        assert!(!status.update_available);
    }

    #[tokio::test]
    async fn offline_check_is_a_network_error() {
        let world = FakeWorld::new()
            .with_installed("1.6.0")
            .failing_on("latest_release");
        let error = CheckPackUpdateUseCase::new(world.clone(), world.clone())
            .execute()
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Network);
    }
}
