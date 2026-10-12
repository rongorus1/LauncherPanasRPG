use std::{
    collections::HashSet,
    fs, io,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use launcher_core::{
    application::ports::outbound::{
        AccountRepository, InstanceStorage, PortResult, SettingsRepository,
    },
    domain::{InstalledPack, OfflineAccount, Settings},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    error::{from_io, from_join},
    fs_util::{remove_dir_if_exists, temp_sibling, write_atomic},
};

/// Top-level entries that belong to the player: the pack may add them but never overwrites them.
const PLAYER_OWNED: &[&str] = &[
    "saves",
    "screenshots",
    "logs",
    "crash-reports",
    "servers.dat",
    "usercache.json",
    // The pack ships the resource pack order, but once the player has options they are theirs.
    "options.txt",
];

/// JSON document on disk with atomic writes. A corrupt file is moved to `<name>.bak` and treated as missing.
#[derive(Debug, Clone)]
struct JsonFile<T> {
    path: PathBuf,
    _type: PhantomData<fn() -> T>,
}

impl<T: Serialize + DeserializeOwned + Send + 'static> JsonFile<T> {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            _type: PhantomData,
        }
    }

    async fn load(&self) -> PortResult<Option<T>> {
        let raw = match tokio::fs::read(&self.path).await {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(from_io(e, &self.path)),
        };
        match serde_json::from_slice(&raw) {
            Ok(value) => Ok(Some(value)),
            Err(_) => {
                let backup = temp_sibling(&self.path, ".bak");
                tokio::fs::rename(&self.path, &backup)
                    .await
                    .map_err(|e| from_io(e, &self.path))?;
                Ok(None)
            }
        }
    }

    async fn save(&self, value: &T) -> PortResult<()> {
        let json = serde_json::to_vec_pretty(value)
            .map_err(|e| crate::error::from_json(e, &self.path.display().to_string()))?;
        write_atomic(&self.path, json).await
    }
}

#[derive(Debug, Clone)]
pub struct JsonSettingsRepository {
    file: JsonFile<Settings>,
}

impl JsonSettingsRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: JsonFile::new(path.into()),
        }
    }
}

#[async_trait]
impl SettingsRepository for JsonSettingsRepository {
    async fn load(&self) -> PortResult<Settings> {
        Ok(self.file.load().await?.unwrap_or_default())
    }

    async fn save(&self, settings: &Settings) -> PortResult<()> {
        self.file.save(settings).await
    }
}

#[derive(Debug, Clone)]
pub struct JsonAccountRepository {
    file: JsonFile<OfflineAccount>,
}

impl JsonAccountRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: JsonFile::new(path.into()),
        }
    }
}

#[async_trait]
impl AccountRepository for JsonAccountRepository {
    async fn load(&self) -> PortResult<Option<OfflineAccount>> {
        self.file.load().await
    }

    async fn save(&self, account: &OfflineAccount) -> PortResult<()> {
        self.file.save(account).await
    }
}

/// Launcher data layout: `game/`, `instance/`, `runtimes/`, `staging/` and `installed-pack.json`.
#[derive(Debug, Clone)]
pub struct FsInstanceStorage {
    data_dir: PathBuf,
    installed: JsonFile<InstalledPack>,
}

impl FsInstanceStorage {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let installed = JsonFile::new(data_dir.join("installed-pack.json"));
        Self {
            data_dir,
            installed,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

#[async_trait]
impl InstanceStorage for FsInstanceStorage {
    fn game_root(&self) -> PathBuf {
        self.data_dir.join("game")
    }

    fn instance_dir(&self) -> PathBuf {
        self.data_dir.join("instance")
    }

    fn runtimes_dir(&self) -> PathBuf {
        self.data_dir.join("runtimes")
    }

    fn staging_dir(&self) -> PathBuf {
        self.data_dir.join("staging")
    }

    async fn load_installed_pack(&self) -> PortResult<Option<InstalledPack>> {
        self.installed.load().await
    }

    async fn save_installed_pack(&self, pack: &InstalledPack) -> PortResult<()> {
        self.installed.save(pack).await
    }

    async fn apply_overrides(&self, overrides_dir: &Path) -> PortResult<()> {
        let source = overrides_dir.to_path_buf();
        let instance = self.instance_dir();
        tokio::task::spawn_blocking(move || apply_overrides_blocking(&source, &instance))
            .await
            .map_err(from_join)?
    }

    async fn prune_mods(&self, keep: &[String]) -> PortResult<()> {
        let mods = self.instance_dir().join("mods");
        let keep: HashSet<String> = keep.iter().cloned().collect();
        tokio::task::spawn_blocking(move || prune_mods_blocking(&mods, &keep))
            .await
            .map_err(from_join)?
    }

    async fn clear_staging(&self) -> PortResult<()> {
        remove_dir_if_exists(&self.staging_dir()).await
    }
}

fn apply_overrides_blocking(source: &Path, instance: &Path) -> PortResult<()> {
    fs::create_dir_all(instance).map_err(|e| from_io(e, instance))?;
    let entries = match fs::read_dir(source) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(from_io(e, source)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| from_io(e, source))?;
        let name = entry.file_name();
        let overwrite = !PLAYER_OWNED.iter().any(|owned| name == *owned);
        copy_tree(&entry.path(), &instance.join(&name), overwrite)?;
    }
    Ok(())
}

/// Copies files without ever deleting anything in `target`.
fn copy_tree(source: &Path, target: &Path, overwrite: bool) -> PortResult<()> {
    let metadata = fs::metadata(source).map_err(|e| from_io(e, source))?;
    if metadata.is_dir() {
        fs::create_dir_all(target).map_err(|e| from_io(e, target))?;
        for entry in fs::read_dir(source).map_err(|e| from_io(e, source))? {
            let entry = entry.map_err(|e| from_io(e, source))?;
            copy_tree(&entry.path(), &target.join(entry.file_name()), overwrite)?;
        }
        return Ok(());
    }
    match fs::symlink_metadata(target) {
        Ok(existing) if existing.is_dir() => Ok(()),
        Ok(_) if !overwrite => Ok(()),
        _ => {
            fs::copy(source, target).map_err(|e| from_io(e, target))?;
            Ok(())
        }
    }
}

fn prune_mods_blocking(mods: &Path, keep: &HashSet<String>) -> PortResult<()> {
    let entries = match fs::read_dir(mods) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(from_io(e, mods)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| from_io(e, mods))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_jar = name.to_ascii_lowercase().ends_with(".jar");
        if is_jar && !keep.contains(&name) && entry.file_type().is_ok_and(|t| t.is_file()) {
            fs::remove_file(entry.path()).map_err(|e| from_io(e, &entry.path()))?;
        }
    }
    Ok(())
}
