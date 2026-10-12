use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    dto::Activity,
    error::AppError,
    events::LauncherEvent,
    ports::{
        inbound::InstallLauncherUpdate,
        outbound::{DownloadObserver, EventPublisher, LauncherUpdater},
    },
};

use super::ActivityTracker;

pub struct InstallLauncherUpdateUseCase {
    updater: Arc<dyn LauncherUpdater>,
    events: Arc<dyn EventPublisher>,
    activity: ActivityTracker,
}

impl InstallLauncherUpdateUseCase {
    pub fn new(
        updater: Arc<dyn LauncherUpdater>,
        events: Arc<dyn EventPublisher>,
        activity: ActivityTracker,
    ) -> Self {
        Self {
            updater,
            events,
            activity,
        }
    }
}

struct EventDownloadObserver(Arc<dyn EventPublisher>);

impl DownloadObserver for EventDownloadObserver {
    fn on_progress(&self, downloaded: u64, total: Option<u64>) {
        self.0
            .publish(LauncherEvent::UpdateProgress { downloaded, total });
    }
}

#[async_trait]
impl InstallLauncherUpdate for InstallLauncherUpdateUseCase {
    async fn execute(&self) -> Result<(), AppError> {
        // Updating while installing or playing would kill the running work.
        let _busy = self.activity.try_begin(Activity::Installing)?;
        let observer = EventDownloadObserver(self.events.clone());
        self.updater.download_and_install(&observer).await?;
        self.updater.restart();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        error::{AppErrorKind, PortError},
        use_cases::testing::{FakeUpdater, RecordingEvents},
    };

    #[tokio::test]
    async fn installs_reports_progress_and_restarts() {
        let updater = FakeUpdater::with_update("0.2.0");
        let events = RecordingEvents::new();
        InstallLauncherUpdateUseCase::new(updater.clone(), events.clone(), ActivityTracker::new())
            .execute()
            .await
            .unwrap();
        assert_eq!(updater.calls(), ["download_and_install", "restart"]);
        assert!(matches!(
            events.all().first(),
            Some(LauncherEvent::UpdateProgress {
                downloaded: 50,
                total: Some(100)
            })
        ));
    }

    #[tokio::test]
    async fn does_not_restart_when_the_download_fails() {
        let updater = FakeUpdater::failing(PortError::Network("cut".into()));
        let error = InstallLauncherUpdateUseCase::new(
            updater.clone(),
            RecordingEvents::new(),
            ActivityTracker::new(),
        )
        .execute()
        .await
        .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Network);
        assert!(!updater.calls().contains(&"restart".to_owned()));
    }

    #[tokio::test]
    async fn refuses_while_the_launcher_is_busy() {
        let activity = ActivityTracker::new();
        let _playing = activity.try_begin(Activity::Playing).unwrap();
        let error = InstallLauncherUpdateUseCase::new(
            FakeUpdater::with_update("0.2.0"),
            RecordingEvents::new(),
            activity,
        )
        .execute()
        .await
        .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Busy);
    }
}
