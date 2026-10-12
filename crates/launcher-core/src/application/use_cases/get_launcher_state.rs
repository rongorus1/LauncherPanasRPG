use std::sync::Arc;

use async_trait::async_trait;

use super::{ActivityTracker, check_pack_update::pack_status};
use crate::application::{
    dto::{AccountView, LauncherState},
    error::AppError,
    ports::{
        inbound::GetLauncherState,
        outbound::{AccountRepository, InstanceStorage, PackSource, SettingsRepository},
    },
};

pub struct GetLauncherStateUseCase {
    accounts: Arc<dyn AccountRepository>,
    settings: Arc<dyn SettingsRepository>,
    storage: Arc<dyn InstanceStorage>,
    pack_source: Arc<dyn PackSource>,
    activity: ActivityTracker,
}

impl GetLauncherStateUseCase {
    pub fn new(
        accounts: Arc<dyn AccountRepository>,
        settings: Arc<dyn SettingsRepository>,
        storage: Arc<dyn InstanceStorage>,
        pack_source: Arc<dyn PackSource>,
        activity: ActivityTracker,
    ) -> Self {
        Self {
            accounts,
            settings,
            storage,
            pack_source,
            activity,
        }
    }
}

#[async_trait]
impl GetLauncherState for GetLauncherStateUseCase {
    async fn execute(&self) -> Result<LauncherState, AppError> {
        let account = self.accounts.load().await?.as_ref().map(AccountView::from);
        let settings = self.settings.load().await?;
        let installed = self.storage.load_installed_pack().await?;
        // The launcher must open offline, so a failed check only hides the latest version.
        let latest = self
            .pack_source
            .latest_release()
            .await
            .ok()
            .map(|release| release.version);
        Ok(LauncherState {
            account,
            settings,
            pack: pack_status(installed.as_ref(), latest),
            activity: self.activity.current(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{dto::Activity, use_cases::testing::FakeWorld};

    fn use_case(world: &Arc<FakeWorld>, activity: ActivityTracker) -> GetLauncherStateUseCase {
        GetLauncherStateUseCase::new(
            world.clone(),
            world.clone(),
            world.clone(),
            world.clone(),
            activity,
        )
    }

    #[tokio::test]
    async fn collects_account_settings_pack_and_activity() {
        let world = FakeWorld::new()
            .with_account("Notch")
            .with_installed("1.6.0");
        let activity = ActivityTracker::new();
        let _guard = activity.try_begin(Activity::Playing).unwrap();

        let state = use_case(&world, activity.clone()).execute().await.unwrap();

        let account = state.account.unwrap();
        assert_eq!(account.username, "Notch");
        assert_eq!(account.uuid, "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(state.settings, world.settings());
        assert_eq!(state.pack.latest_version.as_deref(), Some("1.7.2"));
        assert!(state.pack.update_available);
        assert_eq!(state.activity, Activity::Playing);
    }

    #[tokio::test]
    async fn offline_state_has_no_latest_version_and_no_error() {
        let world = FakeWorld::new()
            .with_installed("1.6.0")
            .failing_on("latest_release");
        let state = use_case(&world, ActivityTracker::new())
            .execute()
            .await
            .unwrap();
        assert_eq!(state.account, None);
        assert_eq!(state.pack.installed_version.as_deref(), Some("1.6.0"));
        assert_eq!(state.pack.latest_version, None);
        assert!(!state.pack.update_available);
        assert_eq!(state.activity, Activity::Idle);
    }
}
