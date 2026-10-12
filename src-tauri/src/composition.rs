//! Composition root: the only place that knows concrete adapters and injects them into use cases.

use std::{path::PathBuf, sync::Arc};

use launcher_core::{
    application::{
        ports::{inbound::*, outbound::*},
        use_cases::{
            ActivityTracker, CheckLauncherUpdateUseCase, CheckPackUpdateUseCase,
            GetLauncherStateUseCase, InstallLauncherUpdateUseCase, InstallPackDeps,
            InstallPackUseCase, PlayGameDeps, PlayGameUseCase, SaveOfflineAccountUseCase,
            SaveSettingsUseCase, StopGameUseCase,
        },
    },
    domain::{HostEnvironment, launch_plan_builder::HostOs},
};
use launcher_infrastructure::{
    CurseForgePackSource, ForgeLoaderInstaller, FsInstanceStorage, GameProcessRunner, HttpClient,
    JsonAccountRepository, JsonSettingsRepository, MojangJavaProvider, MojangMinecraftSource,
    ParallelDownloader,
};

use crate::adapters::outbound::{TauriEventPublisher, TauriLauncherUpdater};

const PACK_PROJECT_ID: u64 = 1_493_979;

/// Inbound ports exposed to the Tauri commands.
pub struct Services {
    pub get_launcher_state: Arc<dyn GetLauncherState>,
    pub save_offline_account: Arc<dyn SaveOfflineAccount>,
    pub save_settings: Arc<dyn SaveSettings>,
    pub check_pack_update: Arc<dyn CheckPackUpdate>,
    pub install_pack: Arc<dyn InstallPack>,
    pub play_game: Arc<dyn PlayGame>,
    pub stop_game: Arc<dyn StopGame>,
    pub check_launcher_update: Arc<dyn CheckLauncherUpdate>,
    pub install_launcher_update: Arc<dyn InstallLauncherUpdate>,
}

pub fn build_services(
    app: tauri::AppHandle,
    data_dir: PathBuf,
    app_version: &str,
) -> Result<Services, String> {
    let http = HttpClient::new(app_version).map_err(|e| e.to_string())?;

    let launcher_updater: Arc<dyn LauncherUpdater> =
        Arc::new(TauriLauncherUpdater::new(app.clone()));
    let events: Arc<dyn EventPublisher> = Arc::new(TauriEventPublisher::new(app));
    let activity = ActivityTracker::new();

    let storage: Arc<dyn InstanceStorage> = Arc::new(FsInstanceStorage::new(&data_dir));
    let settings: Arc<dyn SettingsRepository> =
        Arc::new(JsonSettingsRepository::new(data_dir.join("settings.json")));
    let accounts: Arc<dyn AccountRepository> =
        Arc::new(JsonAccountRepository::new(data_dir.join("account.json")));

    let pack_source: Arc<dyn PackSource> = Arc::new(CurseForgePackSource::new(
        http.clone(),
        curseforge_api_key(),
        PACK_PROJECT_ID,
    ));
    let minecraft: Arc<dyn MinecraftSource> = Arc::new(MojangMinecraftSource::new(http.clone()));
    let loader: Arc<dyn LoaderInstaller> = Arc::new(ForgeLoaderInstaller::new(http.clone()));
    let java: Arc<dyn JavaProvider> = Arc::new(MojangJavaProvider::new(http.clone()));
    let downloader: Arc<dyn FileDownloader> = Arc::new(ParallelDownloader::new(http));
    let game: Arc<dyn GameProcess> = Arc::new(GameProcessRunner::new());

    Ok(Services {
        get_launcher_state: Arc::new(GetLauncherStateUseCase::new(
            accounts.clone(),
            settings.clone(),
            storage.clone(),
            pack_source.clone(),
            activity.clone(),
        )),
        save_offline_account: Arc::new(SaveOfflineAccountUseCase::new(accounts.clone())),
        save_settings: Arc::new(SaveSettingsUseCase::new(settings.clone())),
        check_pack_update: Arc::new(CheckPackUpdateUseCase::new(
            storage.clone(),
            pack_source.clone(),
        )),
        install_pack: Arc::new(InstallPackUseCase::new(InstallPackDeps {
            pack_source,
            minecraft: minecraft.clone(),
            loader: loader.clone(),
            java: java.clone(),
            downloader,
            storage: storage.clone(),
            events: events.clone(),
            activity: activity.clone(),
        })),
        play_game: Arc::new(PlayGameUseCase::new(PlayGameDeps {
            accounts,
            settings,
            storage,
            loader,
            minecraft,
            java,
            game: game.clone(),
            events: events.clone(),
            activity: activity.clone(),
            host: host_environment(),
        })),
        stop_game: Arc::new(StopGameUseCase::new(game)),
        check_launcher_update: Arc::new(CheckLauncherUpdateUseCase::new(launcher_updater.clone())),
        install_launcher_update: Arc::new(InstallLauncherUpdateUseCase::new(
            launcher_updater,
            events.clone(),
            activity.clone(),
        )),
    })
}

fn curseforge_api_key() -> String {
    std::env::var("CURSEFORGE_API_KEY")
        .ok()
        .filter(|key| !key.is_empty())
        .or_else(|| crate::env_file::read_value("CURSEFORGE_API_KEY"))
        .or_else(|| option_env!("CURSEFORGE_API_KEY").map(str::to_owned))
        .unwrap_or_default()
}

fn host_environment() -> HostEnvironment {
    let os = if cfg!(target_os = "windows") {
        HostOs::Windows
    } else if cfg!(target_os = "macos") {
        HostOs::MacOs
    } else {
        HostOs::Linux
    };
    HostEnvironment::new(os, std::env::consts::ARCH)
}
