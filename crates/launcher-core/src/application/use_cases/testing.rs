//! One in-memory fake that implements every outbound port, so tests can inspect a single call log.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use tokio::sync::Notify;

use crate::application::{
    error::PortError,
    events::{LauncherEvent, LogStream},
    ports::outbound::*,
};
use crate::domain::{
    InstalledPack, LaunchPlan, LauncherUpdate, ModFileRef, OfflineAccount, PackManifest,
    PackRelease, PackVersion, ResolvedModFile, Settings, Username, version::VersionDescriptor,
};

const MINECRAFT: &str = "1.20.1";
const FORGE: &str = "47.4.10";
const LAUNCH_ID: &str = "1.20.1-forge-47.4.10";

#[derive(Default)]
struct State {
    calls: Vec<String>,
    events: Vec<LauncherEvent>,
    failing: HashSet<String>,
    account: Option<OfflineAccount>,
    settings: Settings,
    installed: Option<InstalledPack>,
    game_files: Option<VersionDescriptor>,
    downloads: Vec<DownloadTask>,
    launched_plan: Option<LaunchPlan>,
    game_output: Vec<(LogStream, String)>,
    exit_code: Option<i32>,
    game_gate: Option<Arc<Notify>>,
}

#[derive(Default)]
pub struct FakeWorld {
    state: Mutex<State>,
}

impl FakeWorld {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Records the call and fails with a network error if `name` was marked as failing.
    fn call(&self, name: &str, detail: impl Into<String>) -> PortResult<()> {
        let detail = detail.into();
        let mut state = self.state();
        state.calls.push(if detail.is_empty() {
            name.to_owned()
        } else {
            format!("{name}:{detail}")
        });
        if state.failing.contains(name) {
            return Err(PortError::Network(format!("{name} failed")));
        }
        Ok(())
    }

    pub fn with_account(self: Arc<Self>, name: &str) -> Arc<Self> {
        self.state().account = Some(OfflineAccount::new(Username::parse(name).unwrap()));
        self
    }

    pub fn with_installed(self: Arc<Self>, version: &str) -> Arc<Self> {
        self.state().installed = Some(InstalledPack {
            version: version.parse().unwrap(),
            minecraft_version: MINECRAFT.into(),
            forge_version: FORGE.into(),
            launch_version_id: LAUNCH_ID.into(),
            mod_files: vec!["jei.jar".into()],
        });
        self
    }

    pub fn failing_on(self: Arc<Self>, name: &str) -> Arc<Self> {
        self.state().failing.insert(name.to_owned());
        self
    }

    pub fn with_game_output(
        self: Arc<Self>,
        lines: &[(LogStream, &str)],
        exit_code: Option<i32>,
    ) -> Arc<Self> {
        {
            let mut state = self.state();
            state.game_output = lines
                .iter()
                .map(|(stream, line)| (*stream, (*line).to_owned()))
                .collect();
            state.exit_code = exit_code;
        }
        self
    }

    /// Makes `GameProcess::run` wait until the returned notify fires.
    pub fn hold_game(&self) -> Arc<Notify> {
        let gate = Arc::new(Notify::new());
        self.state().game_gate = Some(gate.clone());
        gate
    }

    pub fn stop_failing(&self) {
        self.state().failing.clear();
    }

    pub fn set_settings(&self, settings: Settings) {
        self.state().settings = settings;
    }

    pub fn calls(&self) -> Vec<String> {
        self.state().calls.clone()
    }

    pub fn events(&self) -> Vec<LauncherEvent> {
        self.state().events.clone()
    }

    pub fn account(&self) -> Option<OfflineAccount> {
        self.state().account.clone()
    }

    pub fn settings(&self) -> Settings {
        self.state().settings.clone()
    }

    pub fn installed(&self) -> Option<InstalledPack> {
        self.state().installed.clone()
    }

    pub fn installed_game_files(&self) -> Option<VersionDescriptor> {
        self.state().game_files.clone()
    }

    pub fn downloads(&self) -> Vec<DownloadTask> {
        self.state().downloads.clone()
    }

    pub fn launched_plan(&self) -> Option<LaunchPlan> {
        self.state().launched_plan.clone()
    }
}

pub async fn wait_until(condition: impl Fn() -> bool) {
    for _ in 0..500 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("condition not reached in time");
}

fn vanilla_descriptor() -> VersionDescriptor {
    serde_json::from_str(
        r#"{"id": "1.20.1", "type": "release", "mainClass": "net.minecraft.client.main.Main", "assets": "5",
            "assetIndex": {"id": "5", "url": "https://piston-meta.mojang.com/5.json"},
            "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
            "libraries": [{"name": "com.mojang:logging:1.1.1"}],
            "arguments": {"game": ["--username", "${auth_player_name}", "--accessToken", "${auth_access_token}",
                                   "--userType", "${user_type}"],
                          "jvm": ["-cp", "${classpath}"]}}"#,
    )
    .unwrap()
}

fn forge_descriptor() -> VersionDescriptor {
    serde_json::from_str(
        r#"{"id": "1.20.1-forge-47.4.10", "inheritsFrom": "1.20.1",
            "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
            "libraries": [{"name": "net.minecraftforge:fmlloader:1.20.1-47.4.10"}],
            "arguments": {"game": ["--launchTarget", "forgeclient"], "jvm": []}}"#,
    )
    .unwrap()
}

#[async_trait]
impl PackSource for FakeWorld {
    async fn latest_release(&self) -> PortResult<PackRelease> {
        self.call("latest_release", "")?;
        Ok(PackRelease {
            version: PackVersion::new(1, 7, 2),
            file_id: 99,
            file_name: "Panas RPG 1.7.2.zip".into(),
        })
    }

    async fn fetch_pack(
        &self,
        release: &PackRelease,
        _staging_dir: &Path,
    ) -> PortResult<PackManifest> {
        self.call("fetch_pack", "")?;
        Ok(PackManifest {
            name: "Panas RPG".into(),
            version: release.version,
            minecraft_version: MINECRAFT.into(),
            forge_version: FORGE.into(),
            files: vec![
                ModFileRef {
                    project_id: 1,
                    file_id: 10,
                },
                ModFileRef {
                    project_id: 2,
                    file_id: 20,
                },
            ],
        })
    }

    async fn resolve_files(&self, files: &[ModFileRef]) -> PortResult<Vec<ResolvedModFile>> {
        self.call("resolve_files", "")?;
        let names = ["jei.jar", "ftb.jar"];
        let hashes = ["aaa", "bbb"];
        Ok(files
            .iter()
            .zip(names.iter().zip(hashes))
            .map(|(reference, (name, sha1))| ResolvedModFile {
                reference: *reference,
                file_name: (*name).into(),
                download_url: format!("https://cdn.example/{name}"),
                sha1: Some(sha1.into()),
            })
            .collect())
    }
}

#[async_trait]
impl MinecraftSource for FakeWorld {
    async fn fetch_version(&self, version_id: &str) -> PortResult<VersionDescriptor> {
        self.call("fetch_version", version_id)?;
        Ok(vanilla_descriptor())
    }

    async fn install_game_files(
        &self,
        version: &VersionDescriptor,
        _root: &Path,
        _reporter: &dyn ProgressReporter,
    ) -> PortResult<()> {
        self.call("install_game_files", version.id.clone())?;
        self.state().game_files = Some(version.clone());
        Ok(())
    }
}

#[async_trait]
impl LoaderInstaller for FakeWorld {
    async fn install_forge(
        &self,
        minecraft_version: &str,
        forge_version: &str,
        _root: &Path,
        _java: &Path,
        _reporter: &dyn ProgressReporter,
    ) -> PortResult<String> {
        self.call(
            "install_forge",
            format!("{minecraft_version}:{forge_version}"),
        )?;
        Ok(LAUNCH_ID.into())
    }

    async fn read_installed_version(
        &self,
        _root: &Path,
        version_id: &str,
    ) -> PortResult<VersionDescriptor> {
        self.call("read_installed_version", version_id)?;
        match version_id {
            LAUNCH_ID => Ok(forge_descriptor()),
            MINECRAFT => Ok(vanilla_descriptor()),
            other => Err(PortError::NotFound(other.into())),
        }
    }
}

#[async_trait]
impl JavaProvider for FakeWorld {
    async fn ensure_runtime(
        &self,
        requirement: &JavaRequirement,
        runtimes_dir: &Path,
        _reporter: &dyn ProgressReporter,
    ) -> PortResult<JavaRuntime> {
        self.call(
            "ensure_runtime",
            format!("{}:{}", requirement.component, requirement.major),
        )?;
        Ok(JavaRuntime {
            executable: runtimes_dir
                .join(&requirement.component)
                .join("bin")
                .join("java"),
            major: requirement.major,
        })
    }
}

#[async_trait]
impl FileDownloader for FakeWorld {
    async fn download_all(
        &self,
        tasks: Vec<DownloadTask>,
        _reporter: &dyn ProgressReporter,
    ) -> PortResult<()> {
        self.call("download_all", tasks.len().to_string())?;
        self.state().downloads.extend(tasks);
        Ok(())
    }
}

#[async_trait]
impl InstanceStorage for FakeWorld {
    fn game_root(&self) -> PathBuf {
        PathBuf::from("/game")
    }

    fn instance_dir(&self) -> PathBuf {
        PathBuf::from("/instance")
    }

    fn runtimes_dir(&self) -> PathBuf {
        PathBuf::from("/runtimes")
    }

    fn staging_dir(&self) -> PathBuf {
        PathBuf::from("/staging")
    }

    async fn load_installed_pack(&self) -> PortResult<Option<InstalledPack>> {
        Ok(self.installed())
    }

    async fn save_installed_pack(&self, pack: &InstalledPack) -> PortResult<()> {
        self.call("save_installed_pack", "")?;
        self.state().installed = Some(pack.clone());
        Ok(())
    }

    async fn apply_overrides(&self, overrides_dir: &Path) -> PortResult<()> {
        self.call("apply_overrides", overrides_dir.to_string_lossy())
    }

    async fn prune_mods(&self, keep: &[String]) -> PortResult<()> {
        self.call("prune_mods", keep.join(","))
    }

    async fn clear_staging(&self) -> PortResult<()> {
        self.call("clear_staging", "")
    }
}

#[async_trait]
impl SettingsRepository for FakeWorld {
    async fn load(&self) -> PortResult<Settings> {
        Ok(self.settings())
    }

    async fn save(&self, settings: &Settings) -> PortResult<()> {
        self.set_settings(settings.clone());
        Ok(())
    }
}

#[async_trait]
impl AccountRepository for FakeWorld {
    async fn load(&self) -> PortResult<Option<OfflineAccount>> {
        Ok(self.account())
    }

    async fn save(&self, account: &OfflineAccount) -> PortResult<()> {
        self.state().account = Some(account.clone());
        Ok(())
    }
}

#[async_trait]
impl GameProcess for FakeWorld {
    async fn run(&self, plan: &LaunchPlan, sink: Arc<dyn GameLogSink>) -> PortResult<GameExit> {
        self.call("run", "")?;
        let gate = {
            let mut state = self.state();
            state.launched_plan = Some(plan.clone());
            state.game_gate.clone()
        };
        if let Some(gate) = gate {
            gate.notified().await;
        }
        let (output, code) = {
            let state = self.state();
            (state.game_output.clone(), state.exit_code)
        };
        for (stream, line) in output {
            sink.line(stream, line);
        }
        Ok(GameExit { code })
    }

    async fn stop(&self) -> PortResult<()> {
        self.call("stop", "")
    }
}

impl EventPublisher for FakeWorld {
    fn publish(&self, event: LauncherEvent) {
        self.state().events.push(event);
    }
}

/// Collects published events.
#[derive(Default)]
pub struct RecordingEvents(Mutex<Vec<LauncherEvent>>);

impl RecordingEvents {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn all(&self) -> Vec<LauncherEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl EventPublisher for RecordingEvents {
    fn publish(&self, event: LauncherEvent) {
        self.0.lock().unwrap().push(event);
    }
}

pub struct FakeUpdater {
    update: Option<LauncherUpdate>,
    failure: Option<PortError>,
    calls: Mutex<Vec<String>>,
}

impl FakeUpdater {
    fn build(update: Option<&str>, failure: Option<PortError>) -> Arc<Self> {
        Arc::new(Self {
            update: update.map(|version| LauncherUpdate {
                version: version.to_owned(),
                notes: None,
            }),
            failure,
            calls: Mutex::default(),
        })
    }

    pub fn with_update(version: &str) -> Arc<Self> {
        Self::build(Some(version), None)
    }

    pub fn up_to_date() -> Arc<Self> {
        Self::build(None, None)
    }

    pub fn failing(error: PortError) -> Arc<Self> {
        Self::build(Some("0.2.0"), Some(error))
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, name: &str) {
        self.calls.lock().unwrap().push(name.to_owned());
    }
}

#[async_trait]
impl LauncherUpdater for FakeUpdater {
    async fn check(&self) -> PortResult<Option<LauncherUpdate>> {
        self.record("check");
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(self.update.clone()),
        }
    }

    async fn download_and_install(&self, observer: &dyn DownloadObserver) -> PortResult<()> {
        self.record("download_and_install");
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        observer.on_progress(50, Some(100));
        Ok(())
    }

    fn restart(&self) {
        self.record("restart");
    }
}
