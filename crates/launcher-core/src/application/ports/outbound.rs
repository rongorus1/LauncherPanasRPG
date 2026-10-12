//! Outbound ports: what the application needs from the outside world.
//! They model capabilities, not technologies; `launcher-infrastructure` provides the adapters.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;

use crate::application::{
    error::PortError,
    events::{LauncherEvent, LogStream},
};
use crate::domain::{
    InstalledPack, LauncherUpdate, ModFileRef, OfflineAccount, PackManifest, PackRelease, Progress,
    ResolvedModFile, Settings, version::VersionDescriptor,
};

pub type PortResult<T> = Result<T, PortError>;

/// Where the pack is published (CurseForge).
#[async_trait]
pub trait PackSource: Send + Sync {
    async fn latest_release(&self) -> PortResult<PackRelease>;

    /// Downloads the release archive, extracts it into `staging_dir` (overrides end up in
    /// `staging_dir/overrides`) and returns the parsed manifest.
    async fn fetch_pack(
        &self,
        release: &PackRelease,
        staging_dir: &Path,
    ) -> PortResult<PackManifest>;

    /// Resolves manifest references into downloadable files.
    async fn resolve_files(&self, files: &[ModFileRef]) -> PortResult<Vec<ResolvedModFile>>;
}

/// Reports progress of a long operation.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: Progress);
}

/// Official Minecraft distribution (Mojang).
#[async_trait]
pub trait MinecraftSource: Send + Sync {
    /// Vanilla descriptor of a Minecraft version, e.g. `1.20.1`.
    async fn fetch_version(&self, version_id: &str) -> PortResult<VersionDescriptor>;

    /// Downloads client jar, libraries and assets of a *merged* descriptor under `root`
    /// (`versions/`, `libraries/`, `assets/`). Skips files that already verify.
    async fn install_game_files(
        &self,
        version: &VersionDescriptor,
        root: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<()>;
}

/// Installs the mod loader on top of vanilla.
#[async_trait]
pub trait LoaderInstaller: Send + Sync {
    /// Installs Forge into `root` and returns the launchable version id (e.g. `1.20.1-forge-47.4.10`).
    /// `java` is the executable used to run the installer.
    async fn install_forge(
        &self,
        minecraft_version: &str,
        forge_version: &str,
        root: &Path,
        java: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<String>;

    /// Reads an installed descriptor from `root/versions/<id>/<id>.json`.
    async fn read_installed_version(
        &self,
        root: &Path,
        version_id: &str,
    ) -> PortResult<VersionDescriptor>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRequirement {
    /// Mojang runtime component, e.g. `java-runtime-gamma`.
    pub component: String,
    pub major: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRuntime {
    pub executable: PathBuf,
    pub major: u32,
}

#[async_trait]
pub trait JavaProvider: Send + Sync {
    /// Returns a runtime that satisfies the requirement, downloading it if needed.
    async fn ensure_runtime(
        &self,
        requirement: &JavaRequirement,
        runtimes_dir: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<JavaRuntime>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTask {
    pub url: String,
    pub destination: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Parallel, verified, resumable-by-skip downloads.
#[async_trait]
pub trait FileDownloader: Send + Sync {
    /// Skips tasks whose destination already matches `sha1`. Retries transient failures.
    async fn download_all(
        &self,
        tasks: Vec<DownloadTask>,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<()>;
}

/// Layout and state of the launcher data on disk.
#[async_trait]
pub trait InstanceStorage: Send + Sync {
    /// Root of the shared game files (`versions`, `libraries`, `assets`).
    fn game_root(&self) -> PathBuf;
    /// Instance directory (`mods`, `saves`, `config`...).
    fn instance_dir(&self) -> PathBuf;
    fn runtimes_dir(&self) -> PathBuf;
    fn staging_dir(&self) -> PathBuf;

    async fn load_installed_pack(&self) -> PortResult<Option<InstalledPack>>;
    async fn save_installed_pack(&self, pack: &InstalledPack) -> PortResult<()>;

    /// Copies `staging_dir/overrides` into the instance without clobbering player data
    /// (`saves`, `options.txt`, `servers.dat`, screenshots, ...). Config files from the pack replace old ones.
    async fn apply_overrides(&self, overrides_dir: &Path) -> PortResult<()>;

    /// Deletes files in `instance/mods` that are not in `keep` (file names) and removes the staging directory afterwards if asked.
    async fn prune_mods(&self, keep: &[String]) -> PortResult<()>;

    async fn clear_staging(&self) -> PortResult<()>;
}

#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> PortResult<Settings>;
    async fn save(&self, settings: &Settings) -> PortResult<()>;
}

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn load(&self) -> PortResult<Option<OfflineAccount>>;
    async fn save(&self, account: &OfflineAccount) -> PortResult<()>;
}

/// Receives the output of the game process.
pub trait GameLogSink: Send + Sync {
    fn line(&self, stream: LogStream, line: String);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameExit {
    pub code: Option<i32>,
}

#[async_trait]
pub trait GameProcess: Send + Sync {
    /// Spawns the game, streams its output to `sink` and returns when the process exits.
    async fn run(
        &self,
        plan: &crate::domain::LaunchPlan,
        sink: Arc<dyn GameLogSink>,
    ) -> PortResult<GameExit>;
    /// Kills the running game, if any.
    async fn stop(&self) -> PortResult<()>;
}

/// Pushes events to the UI. Must not block.
pub trait EventPublisher: Send + Sync {
    fn publish(&self, event: LauncherEvent);
}

/// Receives the download progress of a launcher update.
pub trait DownloadObserver: Send + Sync {
    fn on_progress(&self, downloaded: u64, total: Option<u64>);
}

/// Self-update of the launcher binary.
#[async_trait]
pub trait LauncherUpdater: Send + Sync {
    /// The newer version, if one is published.
    async fn check(&self) -> PortResult<Option<LauncherUpdate>>;
    /// Downloads and installs the version found by the last `check`.
    async fn download_and_install(&self, observer: &dyn DownloadObserver) -> PortResult<()>;
    /// Restarts into the installed version.
    fn restart(&self);
}
