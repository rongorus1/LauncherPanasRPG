use std::sync::Arc;

use async_trait::async_trait;

use super::{ActivityTracker, EventProgressReporter};
use crate::application::{
    dto::Activity,
    error::AppError,
    events::LauncherEvent,
    ports::{
        inbound::InstallPack,
        outbound::{
            DownloadTask, EventPublisher, FileDownloader, InstanceStorage, JavaProvider,
            JavaRequirement, LoaderInstaller, MinecraftSource, PackSource, ProgressReporter,
        },
    },
};
use crate::domain::{
    ContentKind, INSTALL_LAYOUT, InstallStep, InstalledPack, Progress, merge_descriptors,
    version::VersionDescriptor,
};

const DEFAULT_JAVA_COMPONENT: &str = "java-runtime-gamma";
const DEFAULT_JAVA_MAJOR: u32 = 17;

pub(crate) fn java_requirement(descriptor: &VersionDescriptor) -> JavaRequirement {
    match &descriptor.java_version {
        Some(java) => JavaRequirement {
            component: java.component.clone(),
            major: java.major_version,
        },
        None => JavaRequirement {
            component: DEFAULT_JAVA_COMPONENT.to_owned(),
            major: DEFAULT_JAVA_MAJOR,
        },
    }
}

pub struct InstallPackDeps {
    pub pack_source: Arc<dyn PackSource>,
    pub minecraft: Arc<dyn MinecraftSource>,
    pub loader: Arc<dyn LoaderInstaller>,
    pub java: Arc<dyn JavaProvider>,
    pub downloader: Arc<dyn FileDownloader>,
    pub storage: Arc<dyn InstanceStorage>,
    pub events: Arc<dyn EventPublisher>,
    pub activity: ActivityTracker,
}

/// Installs or updates the pack. Every step is idempotent, so a failed run can simply be retried.
pub struct InstallPackUseCase {
    deps: InstallPackDeps,
}

impl InstallPackUseCase {
    pub fn new(deps: InstallPackDeps) -> Self {
        Self { deps }
    }

    async fn install(&self, reporter: &dyn ProgressReporter) -> Result<String, AppError> {
        let d = &self.deps;
        let game_root = d.storage.game_root();
        let instance_dir = d.storage.instance_dir();
        let staging_dir = d.storage.staging_dir();

        reporter.report(Progress::new(
            InstallStep::CheckingPack,
            0,
            0,
            "Buscando la última versión del pack",
        ));
        let release = d.pack_source.latest_release().await?;
        reporter.report(Progress::new(
            InstallStep::CheckingPack,
            0,
            0,
            format!("Descargando el pack {}", release.version),
        ));
        let manifest = d.pack_source.fetch_pack(&release, &staging_dir).await?;
        let mods = d.pack_source.resolve_files(&manifest.files).await?;

        // The vanilla descriptor comes first because it says which Java runtime the game needs.
        reporter.report(Progress::new(
            InstallStep::PreparingJava,
            0,
            0,
            "Preparando Java",
        ));
        let vanilla = d
            .minecraft
            .fetch_version(&manifest.minecraft_version)
            .await?;
        let java = d
            .java
            .ensure_runtime(
                &java_requirement(&vanilla),
                &d.storage.runtimes_dir(),
                reporter,
            )
            .await?;

        reporter.report(Progress::new(
            InstallStep::InstallingForge,
            0,
            0,
            "Instalando Forge",
        ));
        let launch_version_id = d
            .loader
            .install_forge(
                &manifest.minecraft_version,
                &manifest.forge_version,
                &game_root,
                &java.executable,
                reporter,
            )
            .await?;
        let forge = d
            .loader
            .read_installed_version(&game_root, &launch_version_id)
            .await?;
        let merged = merge_descriptors(&forge, &vanilla);

        reporter.report(Progress::new(
            InstallStep::DownloadingMinecraft,
            0,
            0,
            "Descargando Minecraft",
        ));
        d.minecraft
            .install_game_files(&merged, &game_root, reporter)
            .await?;

        reporter.report(Progress::new(
            InstallStep::DownloadingMods,
            0,
            mods.len() as u64,
            "Descargando mods",
        ));
        let tasks = mods
            .iter()
            .map(|file| DownloadTask {
                url: file.download_url.clone(),
                destination: instance_dir.join(file.kind.folder()).join(&file.file_name),
                sha1: file.sha1.clone(),
                size: None,
            })
            .collect();
        d.downloader.download_all(tasks, reporter).await?;

        reporter.report(Progress::new(
            InstallStep::ApplyingFiles,
            0,
            0,
            "Aplicando configuración del pack",
        ));
        let mod_files: Vec<String> = mods
            .iter()
            .filter(|file| file.kind == ContentKind::Mod)
            .map(|file| file.file_name.clone())
            .collect();
        d.storage.prune_mods(&mod_files).await?;
        d.storage
            .apply_overrides(&staging_dir.join("overrides"))
            .await?;
        d.storage
            .save_installed_pack(&InstalledPack {
                version: release.version,
                minecraft_version: manifest.minecraft_version.clone(),
                forge_version: manifest.forge_version.clone(),
                launch_version_id,
                mod_files,
                layout: INSTALL_LAYOUT,
            })
            .await?;
        d.storage.clear_staging().await?;

        let version = release.version.to_string();
        reporter.report(Progress::new(
            InstallStep::Finished,
            1,
            1,
            format!("Panas RPG {version} listo"),
        ));
        Ok(version)
    }
}

#[async_trait]
impl InstallPack for InstallPackUseCase {
    async fn execute(&self) -> Result<(), AppError> {
        let guard = self.deps.activity.try_begin(Activity::Installing)?;
        let reporter = EventProgressReporter::new(self.deps.events.clone());
        let result = self.install(&reporter).await;
        // Back to idle before the final event so a UI refreshing on it sees the new state.
        drop(guard);
        match result {
            Ok(version) => {
                self.deps
                    .events
                    .publish(LauncherEvent::InstallFinished { version });
                Ok(())
            }
            Err(error) => {
                self.deps.events.publish(LauncherEvent::InstallFailed {
                    detail: error.detail.clone(),
                });
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{error::AppErrorKind, use_cases::testing::FakeWorld};

    fn use_case(world: &Arc<FakeWorld>, activity: ActivityTracker) -> InstallPackUseCase {
        InstallPackUseCase::new(InstallPackDeps {
            pack_source: world.clone(),
            minecraft: world.clone(),
            loader: world.clone(),
            java: world.clone(),
            downloader: world.clone(),
            storage: world.clone(),
            events: world.clone(),
            activity,
        })
    }

    #[tokio::test]
    async fn installs_everything_in_order_and_records_the_pack() {
        let world = FakeWorld::new();
        let activity = ActivityTracker::new();

        use_case(&world, activity.clone()).execute().await.unwrap();

        assert_eq!(
            world.calls(),
            [
                "latest_release",
                "fetch_pack",
                "resolve_files",
                "fetch_version:1.20.1",
                "ensure_runtime:java-runtime-gamma:17",
                "install_forge:1.20.1:47.4.10",
                "read_installed_version:1.20.1-forge-47.4.10",
                "install_game_files:1.20.1-forge-47.4.10",
                "download_all:2",
                "prune_mods:jei.jar",
                "apply_overrides:/staging/overrides",
                "save_installed_pack",
                "clear_staging",
            ]
        );

        let merged = world.installed_game_files().unwrap();
        assert!(merged.inherits_from.is_none());
        assert!(
            merged.asset_index.is_some(),
            "merged with the vanilla parent"
        );
        assert_eq!(
            merged.main_class.as_deref(),
            Some("cpw.mods.bootstraplauncher.BootstrapLauncher")
        );

        let downloads = world.downloads();
        assert_eq!(
            downloads[0].destination,
            std::path::PathBuf::from("/instance/mods/jei.jar")
        );
        assert_eq!(downloads[0].sha1.as_deref(), Some("aaa"));
        assert_eq!(
            downloads[1].destination,
            std::path::PathBuf::from("/instance/resourcepacks/fresh-animations.zip"),
            "resource packs go to their own folder"
        );

        let installed = world.installed().unwrap();
        assert_eq!(installed.version.to_string(), "1.7.2");
        assert_eq!(installed.launch_version_id, "1.20.1-forge-47.4.10");
        assert_eq!(installed.mod_files, ["jei.jar"]);
        assert_eq!(installed.layout, INSTALL_LAYOUT);

        let events = world.events();
        assert!(
            matches!(events.first(), Some(LauncherEvent::InstallProgress(p)) if p.step == InstallStep::CheckingPack)
        );
        assert!(events.iter().any(
            |e| matches!(e, LauncherEvent::InstallProgress(p) if p.step == InstallStep::Finished)
        ));
        assert_eq!(
            events.last(),
            Some(&LauncherEvent::InstallFinished {
                version: "1.7.2".into()
            })
        );
        assert_eq!(activity.current(), Activity::Idle);
    }

    #[tokio::test]
    async fn network_failure_midway_reports_failure_and_returns_to_idle() {
        let world = FakeWorld::new().failing_on("download_all");
        let activity = ActivityTracker::new();

        let error = use_case(&world, activity.clone())
            .execute()
            .await
            .unwrap_err();

        assert_eq!(error.kind, AppErrorKind::Network);
        assert!(matches!(
            world.events().last(),
            Some(LauncherEvent::InstallFailed { .. })
        ));
        assert!(world.installed().is_none());
        assert!(
            !world
                .calls()
                .iter()
                .any(|c| c == "save_installed_pack" || c == "clear_staging")
        );
        assert_eq!(activity.current(), Activity::Idle);
    }

    #[tokio::test]
    async fn retry_after_failure_completes() {
        let world = FakeWorld::new().failing_on("fetch_pack");
        let activity = ActivityTracker::new();
        let install = use_case(&world, activity);
        assert!(install.execute().await.is_err());
        world.stop_failing();
        install.execute().await.unwrap();
        assert!(world.installed().is_some());
    }

    #[tokio::test]
    async fn refuses_to_run_while_busy() {
        let world = FakeWorld::new();
        let activity = ActivityTracker::new();
        let _running = activity.try_begin(Activity::Installing).unwrap();

        let error = use_case(&world, activity.clone())
            .execute()
            .await
            .unwrap_err();

        assert_eq!(error.kind, AppErrorKind::Busy);
        assert!(world.calls().is_empty());
        assert!(world.events().is_empty());
        assert_eq!(activity.current(), Activity::Installing);
    }
}
