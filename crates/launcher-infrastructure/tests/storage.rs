use std::{fs, path::Path};

use launcher_core::{
    application::ports::outbound::{AccountRepository, InstanceStorage, SettingsRepository},
    domain::{InstalledPack, OfflineAccount, PackVersion, Settings, Username},
};
use launcher_infrastructure::{FsInstanceStorage, JsonAccountRepository, JsonSettingsRepository};

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn layout_is_rooted_in_the_data_dir() {
    let storage = FsInstanceStorage::new("/data");
    assert_eq!(storage.game_root(), Path::new("/data/game"));
    assert_eq!(storage.instance_dir(), Path::new("/data/instance"));
    assert_eq!(storage.runtimes_dir(), Path::new("/data/runtimes"));
    assert_eq!(storage.staging_dir(), Path::new("/data/staging"));
}

#[tokio::test]
async fn overrides_replace_pack_files_but_keep_player_data() {
    let data = tempfile::tempdir().unwrap();
    let storage = FsInstanceStorage::new(data.path());
    let instance = storage.instance_dir();
    let overrides = storage.staging_dir().join("overrides");

    write(&instance.join("config/foo.toml"), "old");
    write(&instance.join("config/player-only.toml"), "mine");
    write(&instance.join("options.txt"), "player options");
    write(&instance.join("servers.dat"), "player servers");
    write(&instance.join("saves/World/level.dat"), "player world");
    write(&instance.join("screenshots/a.png"), "shot");

    write(&overrides.join("config/foo.toml"), "new");
    write(&overrides.join("kubejs/startup.js"), "js");
    write(&overrides.join("resourcepacks/pack.zip"), "rp");
    write(&overrides.join("options.txt"), "pack options");
    write(&overrides.join("servers.dat"), "pack servers");
    write(&overrides.join("saves/World/level.dat"), "pack world");
    write(&overrides.join("saves/Tutorial/level.dat"), "tutorial");

    storage.apply_overrides(&overrides).await.unwrap();

    assert_eq!(read(&instance.join("config/foo.toml")), "new");
    assert_eq!(read(&instance.join("config/player-only.toml")), "mine");
    assert_eq!(read(&instance.join("kubejs/startup.js")), "js");
    assert_eq!(read(&instance.join("resourcepacks/pack.zip")), "rp");
    assert_eq!(read(&instance.join("options.txt")), "player options");
    assert_eq!(read(&instance.join("servers.dat")), "player servers");
    assert_eq!(
        read(&instance.join("saves/World/level.dat")),
        "player world"
    );
    assert_eq!(read(&instance.join("saves/Tutorial/level.dat")), "tutorial");
    assert_eq!(read(&instance.join("screenshots/a.png")), "shot");
}

#[tokio::test]
async fn options_from_the_pack_are_used_on_first_install() {
    let data = tempfile::tempdir().unwrap();
    let storage = FsInstanceStorage::new(data.path());
    let overrides = storage.staging_dir().join("overrides");
    write(&overrides.join("options.txt"), "pack options");

    storage.apply_overrides(&overrides).await.unwrap();

    assert_eq!(
        read(&storage.instance_dir().join("options.txt")),
        "pack options"
    );
}

#[tokio::test]
async fn prune_removes_only_stale_jars() {
    let data = tempfile::tempdir().unwrap();
    let storage = FsInstanceStorage::new(data.path());
    let mods = storage.instance_dir().join("mods");
    for name in [
        "keep.jar",
        "stale.jar",
        "notes.txt",
        "disabled.jar.disabled",
    ] {
        write(&mods.join(name), "x");
    }

    storage.prune_mods(&["keep.jar".to_owned()]).await.unwrap();

    let mut left: Vec<String> = fs::read_dir(&mods)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, ["disabled.jar.disabled", "keep.jar", "notes.txt"]);
}

#[tokio::test]
async fn installed_pack_round_trips_and_staging_is_cleared() {
    let data = tempfile::tempdir().unwrap();
    let storage = FsInstanceStorage::new(data.path());
    assert_eq!(storage.load_installed_pack().await.unwrap(), None);
    let pack = InstalledPack {
        version: PackVersion::new(1, 7, 2),
        minecraft_version: "1.20.1".into(),
        forge_version: "47.4.10".into(),
        launch_version_id: "1.20.1-forge-47.4.10".into(),
        mod_files: vec!["a.jar".into()],
    };

    storage.save_installed_pack(&pack).await.unwrap();
    assert_eq!(storage.load_installed_pack().await.unwrap(), Some(pack));

    write(&storage.staging_dir().join("overrides/x"), "x");
    storage.clear_staging().await.unwrap();
    assert!(!storage.staging_dir().exists());
    storage.clear_staging().await.unwrap();
}

#[tokio::test]
async fn settings_default_when_missing_and_recover_from_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let repository = JsonSettingsRepository::new(&path);
    assert_eq!(repository.load().await.unwrap(), Settings::default());

    let custom = Settings {
        max_memory_mb: 8192,
        ..Settings::default()
    };
    repository.save(&custom).await.unwrap();
    assert_eq!(repository.load().await.unwrap(), custom);

    fs::write(&path, "{ not json").unwrap();
    assert_eq!(repository.load().await.unwrap(), Settings::default());
    assert_eq!(read(&dir.path().join("settings.json.bak")), "{ not json");
    assert!(!path.exists());
}

#[tokio::test]
async fn account_is_none_when_missing_or_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("account.json");
    let repository = JsonAccountRepository::new(&path);
    assert_eq!(repository.load().await.unwrap(), None);

    let account = OfflineAccount::new(Username::parse("Gato_Art").unwrap());
    repository.save(&account).await.unwrap();
    assert_eq!(repository.load().await.unwrap(), Some(account));

    fs::write(&path, r#"{"username":"x"}"#).unwrap();
    assert_eq!(repository.load().await.unwrap(), None);
    assert!(dir.path().join("account.json.bak").exists());
}
