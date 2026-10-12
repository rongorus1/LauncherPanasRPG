use std::sync::Arc;

use async_trait::async_trait;

use super::{ActivityTracker, EventLogSink, EventProgressReporter, install_pack::java_requirement};
use crate::application::{
    dto::Activity,
    error::{AppError, AppErrorKind},
    events::{LauncherEvent, LogStream},
    ports::{
        inbound::PlayGame,
        outbound::{
            AccountRepository, EventPublisher, GameProcess, InstanceStorage, JavaProvider,
            LoaderInstaller, MinecraftSource, SettingsRepository,
        },
    },
};
use crate::domain::{
    HostEnvironment, LaunchInput, LaunchPaths, build_launch_plan, version::VersionDescriptor,
};

pub struct PlayGameDeps {
    pub accounts: Arc<dyn AccountRepository>,
    pub settings: Arc<dyn SettingsRepository>,
    pub storage: Arc<dyn InstanceStorage>,
    pub loader: Arc<dyn LoaderInstaller>,
    pub minecraft: Arc<dyn MinecraftSource>,
    pub java: Arc<dyn JavaProvider>,
    pub game: Arc<dyn GameProcess>,
    pub events: Arc<dyn EventPublisher>,
    pub activity: ActivityTracker,
    pub host: HostEnvironment,
}

/// Starts the game and returns right away; a background task forwards its output and reports the exit.
/// Must be called from inside a Tokio runtime.
pub struct PlayGameUseCase {
    deps: PlayGameDeps,
}

impl PlayGameUseCase {
    pub fn new(deps: PlayGameDeps) -> Self {
        Self { deps }
    }

    /// The parent descriptor is read from disk so playing works offline; the network is only a fallback.
    async fn parent_descriptor(
        &self,
        child: &VersionDescriptor,
    ) -> Result<Option<VersionDescriptor>, AppError> {
        let Some(parent_id) = child.inherits_from.as_deref() else {
            return Ok(None);
        };
        let game_root = self.deps.storage.game_root();
        match self
            .deps
            .loader
            .read_installed_version(&game_root, parent_id)
            .await
        {
            Ok(parent) => Ok(Some(parent)),
            Err(_) => Ok(Some(self.deps.minecraft.fetch_version(parent_id).await?)),
        }
    }
}

#[async_trait]
impl PlayGame for PlayGameUseCase {
    async fn execute(&self) -> Result<(), AppError> {
        let d = &self.deps;
        let account =
            d.accounts.load().await?.ok_or_else(|| {
                AppError::new(AppErrorKind::NoAccount, "no offline account saved")
            })?;
        let installed = d.storage.load_installed_pack().await?.ok_or_else(|| {
            AppError::new(AppErrorKind::NotInstalled, "the pack is not installed")
        })?;
        let settings = d.settings.load().await?;

        let guard = d.activity.try_begin(Activity::Playing)?;
        let game_root = d.storage.game_root();
        let child = d
            .loader
            .read_installed_version(&game_root, &installed.launch_version_id)
            .await?;
        let parent = self.parent_descriptor(&child).await?;

        let java = match &settings.java_path {
            Some(path) => path.clone(),
            None => {
                let requirement = java_requirement(parent.as_ref().unwrap_or(&child));
                let reporter = EventProgressReporter::new(d.events.clone());
                d.java
                    .ensure_runtime(&requirement, &d.storage.runtimes_dir(), &reporter)
                    .await?
                    .executable
            }
        };

        let paths = LaunchPaths {
            game_root,
            instance_dir: d.storage.instance_dir(),
            java,
        };
        let plan = build_launch_plan(&LaunchInput {
            child: &child,
            parent: parent.as_ref(),
            account: &account,
            settings: &settings,
            paths: &paths,
            host: &d.host,
        })?;

        d.events.publish(LauncherEvent::GameStarted);
        let game = d.game.clone();
        let events = d.events.clone();
        tokio::spawn(async move {
            let sink = Arc::new(EventLogSink::new(events.clone()));
            let exit = game.run(&plan, sink).await;
            drop(guard);
            let code = match exit {
                Ok(exit) => exit.code,
                Err(error) => {
                    events.publish(LauncherEvent::GameLog {
                        stream: LogStream::Stderr,
                        line: format!("No se pudo ejecutar el juego: {error}"),
                    });
                    None
                }
            };
            events.publish(LauncherEvent::GameExited { code });
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::application::use_cases::testing::{FakeWorld, wait_until};
    use crate::domain::{HostOs, Settings};

    fn use_case(world: &Arc<FakeWorld>, activity: ActivityTracker) -> PlayGameUseCase {
        PlayGameUseCase::new(PlayGameDeps {
            accounts: world.clone(),
            settings: world.clone(),
            storage: world.clone(),
            loader: world.clone(),
            minecraft: world.clone(),
            java: world.clone(),
            game: world.clone(),
            events: world.clone(),
            activity,
            host: HostEnvironment::new(HostOs::Linux, "x86_64"),
        })
    }

    #[tokio::test]
    async fn requires_an_account() {
        let world = FakeWorld::new().with_installed("1.7.2");
        let error = use_case(&world, ActivityTracker::new())
            .execute()
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::NoAccount);
        assert!(world.launched_plan().is_none());
    }

    #[tokio::test]
    async fn requires_an_installed_pack() {
        let world = FakeWorld::new().with_account("Notch");
        let activity = ActivityTracker::new();
        let error = use_case(&world, activity.clone())
            .execute()
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::NotInstalled);
        assert_eq!(activity.current(), Activity::Idle);
    }

    #[tokio::test]
    async fn refuses_to_play_while_installing() {
        let world = FakeWorld::new()
            .with_account("Notch")
            .with_installed("1.7.2");
        let activity = ActivityTracker::new();
        let _installing = activity.try_begin(Activity::Installing).unwrap();
        let error = use_case(&world, activity).execute().await.unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Busy);
    }

    #[tokio::test]
    async fn launches_forwards_logs_and_reports_exit() {
        let world = FakeWorld::new()
            .with_account("Notch")
            .with_installed("1.7.2")
            .with_game_output(
                &[
                    (LogStream::Stdout, "Loading Forge"),
                    (LogStream::Stderr, "warn"),
                ],
                Some(0),
            );
        let activity = ActivityTracker::new();
        let gate = world.hold_game();

        use_case(&world, activity.clone()).execute().await.unwrap();
        assert_eq!(activity.current(), Activity::Playing);
        assert_eq!(world.events().first(), Some(&LauncherEvent::GameStarted));

        gate.notify_one();
        wait_until(|| {
            world
                .events()
                .iter()
                .any(|e| matches!(e, LauncherEvent::GameExited { .. }))
        })
        .await;

        assert_eq!(
            world.events(),
            [
                LauncherEvent::GameStarted,
                LauncherEvent::GameLog {
                    stream: LogStream::Stdout,
                    line: "Loading Forge".into()
                },
                LauncherEvent::GameLog {
                    stream: LogStream::Stderr,
                    line: "warn".into()
                },
                LauncherEvent::GameExited { code: Some(0) },
            ]
        );
        assert_eq!(activity.current(), Activity::Idle);

        let plan = world.launched_plan().unwrap();
        assert_eq!(
            plan.java,
            PathBuf::from("/runtimes/java-runtime-gamma/bin/java")
        );
        assert_eq!(
            plan.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        assert!(
            plan.game_args
                .windows(2)
                .any(|w| w == ["--username", "Notch"])
        );
        assert!(
            plan.game_args
                .windows(2)
                .any(|w| w == ["--accessToken", "0"])
        );
        assert!(
            plan.game_args
                .windows(2)
                .any(|w| w == ["--userType", "legacy"])
        );
        assert!(
            world
                .calls()
                .iter()
                .any(|c| c == "read_installed_version:1.20.1")
        );
        assert!(
            !world.calls().iter().any(|c| c.starts_with("fetch_version")),
            "played offline"
        );
    }

    #[tokio::test]
    async fn custom_java_path_skips_the_runtime_and_spawn_errors_still_exit() {
        let world = FakeWorld::new()
            .with_account("Notch")
            .with_installed("1.7.2")
            .failing_on("run");
        world.set_settings(Settings {
            java_path: Some(PathBuf::from("/opt/jdk17/bin/java")),
            ..Settings::default()
        });
        let activity = ActivityTracker::new();

        use_case(&world, activity.clone()).execute().await.unwrap();
        wait_until(|| {
            world
                .events()
                .iter()
                .any(|e| matches!(e, LauncherEvent::GameExited { .. }))
        })
        .await;

        assert!(
            !world
                .calls()
                .iter()
                .any(|c| c.starts_with("ensure_runtime"))
        );
        assert_eq!(
            world.events().last(),
            Some(&LauncherEvent::GameExited { code: None })
        );
        assert!(world.events().iter().any(|e| matches!(
            e,
            LauncherEvent::GameLog {
                stream: LogStream::Stderr,
                ..
            }
        )));
        assert_eq!(activity.current(), Activity::Idle);
    }
}
