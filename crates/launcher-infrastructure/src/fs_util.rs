use std::{
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use launcher_core::application::ports::outbound::PortResult;
use sha1::{Digest, Sha1};

use crate::error::{from_io, from_join};

pub(crate) fn sha1_of_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// True when `path` exists and matches the expected hash and size (each only if given).
pub(crate) async fn file_matches(path: &Path, sha1: Option<&str>, size: Option<u64>) -> bool {
    let Ok(metadata) = tokio::fs::metadata(path).await else {
        return false;
    };
    if !metadata.is_file() || size.is_some_and(|s| s != metadata.len()) {
        return false;
    }
    let Some(expected) = sha1 else {
        return true;
    };
    let path = path.to_path_buf();
    let expected = expected.to_ascii_lowercase();
    tokio::task::spawn_blocking(move || sha1_of_file(&path).is_ok_and(|actual| actual == expected))
        .await
        .unwrap_or(false)
}

pub(crate) fn temp_sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Writes through a sibling temp file and renames it, so readers never see a half-written file.
pub(crate) fn write_atomic_blocking(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = temp_sibling(path, ".tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)
}

pub(crate) async fn write_atomic(path: &Path, contents: Vec<u8>) -> PortResult<()> {
    let owned = path.to_path_buf();
    tokio::task::spawn_blocking(move || write_atomic_blocking(&owned, &contents))
        .await
        .map_err(from_join)?
        .map_err(|e| from_io(e, path))
}

pub(crate) async fn create_dir_all(path: &Path) -> PortResult<()> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| from_io(e, path))
}

pub(crate) async fn remove_dir_if_exists(path: &Path) -> PortResult<()> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(from_io(e, path)),
    }
}

#[cfg(unix)]
pub(crate) fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
pub(crate) fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
