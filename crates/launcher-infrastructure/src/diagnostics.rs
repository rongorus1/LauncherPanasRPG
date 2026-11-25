//! Local, opt-in diagnostics and recovery. Never contacts a remote service.
use async_trait::async_trait;
use launcher_core::application::{
    diagnostics::{BackupInfo, DeviceDiagnostics, RestoreReport, ScanReport},
    error::PortError,
    ports::outbound::{LocalDiagnostics, PortResult},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, ffi::OsStr, fs, io::{self, Read}, path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

const METADATA: &str = "installed-pack.json";

pub struct FsLocalDiagnostics {
    data_dir: PathBuf,
}

impl FsLocalDiagnostics {
    pub fn new(data_dir: PathBuf) -> Self { Self { data_dir } }
}

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    info: BackupInfo,
    files: Vec<BackupFile>,
}

#[derive(Serialize, Deserialize)]
struct BackupFile {
    name: String,
    sha256: String,
}

struct InstalledPack {
    version: String,
    mod_files: Vec<String>,
}

fn read_pack(dir: &Path) -> Result<InstalledPack, String> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(dir.join(METADATA))
        .map_err(|_| "No se encontró el registro de instalación del modpack.".to_string())?)
        .map_err(|_| "El registro de instalación no es válido.".to_string())?;
    let version = value.get("version").and_then(|x| x.as_str())
        .filter(|x| !x.is_empty()).ok_or("Falta la versión instalada.")?.to_string();
    let files = value.get("mod_files").and_then(|x| x.as_array())
        .ok_or("Falta el listado de mods instalados.")?;
    let mut mod_files = Vec::with_capacity(files.len());
    let mut seen = HashSet::new();
    for entry in files {
        let name = entry.as_str().ok_or("Hay un mod con nombre inválido.")?;
        if !valid_name(name) || !seen.insert(name.to_ascii_lowercase()) {
            return Err("El registro contiene un mod inválido o repetido.".into());
        }
        mod_files.push(name.to_string());
    }
    Ok(InstalledPack { version, mod_files })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "." && name != ".."
        && !name.contains(['/', '\\', ':'])
        && Path::new(name).file_name() == Some(OsStr::new(name))
        && matches!(Path::new(name).extension().and_then(OsStr::to_str).map(str::to_ascii_lowercase).as_deref(), Some("jar" | "zip"))
}

fn mods_dir(dir: &Path) -> PathBuf { dir.join("instance").join("mods") }

fn checked_mods_dir(dir: &Path) -> Result<PathBuf, String> {
    let instance = dir.join("instance");
    let mods = mods_dir(dir);
    for path in [&instance, &mods] {
        if let Ok(meta) = fs::symlink_metadata(path) {
            if !meta.file_type().is_dir() {
                return Err("La carpeta de mods no es un directorio normal.".into());
            }
        }
    }
    Ok(mods)
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|m| m.file_type().is_file()).unwrap_or(false)
}

fn normal_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|m| m.file_type().is_dir()).unwrap_or(false)
}

fn archive_ok(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else { return false };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return false };
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else { return false };
        if io::copy(&mut entry, &mut io::sink()).is_err() { return false; }
    }
    true
}

fn sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|_| "No se pudo leer un archivo de la copia.".to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|_| "No se pudo leer un archivo de la copia.".to_string())?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn unique_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{nanos}-{}", std::process::id())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|c| c.is_ascii_digit() || c == b'-')
}

fn read_backup(root: &Path, id: &str) -> Result<BackupManifest, String> {
    if !valid_id(id) { return Err("Identificador de copia inválido.".into()); }
    let folder = root.join(id);
    if !normal_directory(root) || !normal_directory(&folder) || !normal_directory(&folder.join("mods")) {
        return Err("La carpeta de la copia no es un directorio normal.".into());
    }
    let manifest_path = folder.join("manifest.json");
    if !regular_file(&manifest_path) { return Err("El manifiesto de la copia no es un archivo normal.".into()); }
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(manifest_path)
        .map_err(|_| "No se encontró el manifiesto de la copia.".to_string())?)
        .map_err(|_| "El manifiesto de la copia está dañado.".to_string())?;
    if manifest.info.id != id || manifest.info.file_count != manifest.files.len() {
        return Err("El manifiesto de la copia no coincide.".into());
    }
    let mut seen = HashSet::new();
    for entry in &manifest.files {
        if !valid_name(&entry.name) || !seen.insert(entry.name.to_ascii_lowercase())
            || entry.sha256.len() != 64 || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("La copia contiene un archivo inválido.".into());
        }
        let source = folder.join("mods").join(&entry.name);
        if !regular_file(&source) || sha256(&source)? != entry.sha256 || !archive_ok(&source) {
            return Err(format!("La copia del mod {} está dañada.", entry.name));
        }
    }
    Ok(manifest)
}

fn game_running() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = Command::new("tasklist").args(["/fo", "csv", "/nh"])
            .creation_flags(0x08000000).output();
        // Fail closed if we cannot check whether files are in use.
        let Ok(output) = output else { return true };
        if !output.status.success() { return true; }
        let names = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        names.lines().any(|line| {
            line.starts_with("\"java.exe\"") || line.starts_with("\"javaw.exe\"")
                || line.starts_with("\"minecraftlauncher.exe\"")
        })
    }
    #[cfg(not(windows))]
    { true }
}

async fn get_device_diagnostics() -> Result<DeviceDiagnostics, String> {
    tokio::task::spawn_blocking(|| {
        let mut result = DeviceDiagnostics {
            total_memory_gb: None, processor_name: None,
            logical_cores: std::thread::available_parallelism().ok().map(|n| n.get()),
            gpu_names: Vec::new(),
        };
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let script = "$ErrorActionPreference='Stop'; $c=Get-CimInstance Win32_ComputerSystem; $p=Get-CimInstance Win32_Processor | Select-Object -First 1; $g=@(Get-CimInstance Win32_VideoController | ForEach-Object Name); @{ total_memory_gb=[math]::Round($c.TotalPhysicalMemory/1GB,2); processor_name=$p.Name; logical_cores=[int]$c.NumberOfLogicalProcessors; gpu_names=$g } | ConvertTo-Json -Compress";
            if let Ok(output) = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .creation_flags(0x08000000).output() {
                if output.status.success() {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        result.total_memory_gb = value.get("total_memory_gb").and_then(|v| v.as_f64());
                        result.processor_name = value.get("processor_name").and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
                        result.logical_cores = value.get("logical_cores").and_then(|v| v.as_u64()).and_then(|v| usize::try_from(v).ok()).or(result.logical_cores);
                        if let Some(gpus) = value.get("gpu_names") {
                            if let Some(list) = gpus.as_array() {
                                result.gpu_names = list.iter().filter_map(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string).collect();
                            } else if let Some(single) = gpus.as_str() {
                                result.gpu_names = vec![single.trim().to_string()];
                            }
                        }
                    }
                }
            }
        }
        Ok(result)
    }).await.map_err(|_| "No se pudo consultar el hardware.".to_string())?
}

async fn scan_installation(dir: PathBuf) -> Result<ScanReport, String> {
    tokio::task::spawn_blocking(move || {
        if !dir.join(METADATA).exists() {
            return Ok(ScanReport { installed_version: None, checked_files: 0,
                missing_files: vec![], corrupt_archives: vec![], unchecked_files: vec![], status: "not_installed".into() });
        }
        let pack = read_pack(&dir)?;
        let mods = checked_mods_dir(&dir)?;
        let mut report = ScanReport { installed_version: Some(pack.version), checked_files: 0,
            missing_files: vec![], corrupt_archives: vec![], unchecked_files: vec![], status: "ok".into() };
        for name in pack.mod_files {
            let path = mods.join(&name);
            if !regular_file(&path) { report.missing_files.push(name); }
            else {
                report.checked_files += 1;
                if !archive_ok(&path) { report.corrupt_archives.push(name); }
            }
        }
        if !report.missing_files.is_empty() || !report.corrupt_archives.is_empty() { report.status = "issues".into(); }
        Ok(report)
    }).await.map_err(|_| "No se pudo revisar la instalación.".to_string())?
}

async fn list_backups(root: PathBuf) -> Result<Vec<BackupInfo>, String> {
    tokio::task::spawn_blocking(move || {
        let mut result = Vec::new();
        if !root.exists() { return Ok(result); }
        if !normal_directory(&root) { return Err("La carpeta de copias no es un directorio normal.".into()); }
        for entry in fs::read_dir(&root).map_err(|_| "No se pudieron leer las copias.".to_string())? {
            let Ok(entry) = entry else { continue };
            let id = entry.file_name().to_string_lossy().to_string();
            if valid_id(&id) {
                if let Ok(manifest) = read_backup(&root, &id) { result.push(manifest.info); }
            }
        }
        result.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(result)
    }).await.map_err(|_| "No se pudieron listar las copias.".to_string())?
}

async fn create_backup(dir: PathBuf, root: PathBuf) -> Result<BackupInfo, String> {
    tokio::task::spawn_blocking(move || {
        if game_running() { return Err("Cierra Minecraft antes de crear la copia.".into()); }
        let pack = read_pack(&dir)?;
        let mods = checked_mods_dir(&dir)?;
        let id = unique_id();
        fs::create_dir_all(&root).map_err(|_| "No se pudo preparar la carpeta de copias.".to_string())?;
        if !normal_directory(&root) { return Err("La carpeta de copias no es un directorio normal.".into()); }
        let staging = root.join(format!(".{id}.tmp"));
        let folder = root.join(&id);
        fs::create_dir_all(staging.join("mods")).map_err(|_| "No se pudo preparar la copia.".to_string())?;
        if !normal_directory(&staging) || !normal_directory(&staging.join("mods")) {
            return Err("La carpeta temporal de la copia no es un directorio normal.".into());
        }
        let operation = (|| -> Result<BackupInfo, String> {
            let mut files = Vec::new();
            for name in &pack.mod_files {
                let source = mods.join(name);
                if !regular_file(&source) || !archive_ok(&source) {
                    return Err(format!("El mod {} falta o está dañado; no se creó la copia.", name));
                }
                let source_hash = sha256(&source)?;
                let target = staging.join("mods").join(name);
                fs::copy(&source, &target).map_err(|_| format!("No se pudo copiar el mod {}.", name))?;
                let target_hash = sha256(&target)?;
                if source_hash != target_hash || !archive_ok(&target) {
                    return Err(format!("El mod {} cambió durante la copia.", name));
                }
                files.push(BackupFile { name: name.clone(), sha256: target_hash });
            }
            let info = BackupInfo { id: id.clone(), version: pack.version, file_count: files.len() };
            let manifest = BackupManifest { info: info.clone(), files };
            fs::write(staging.join("manifest.json"), serde_json::to_vec_pretty(&manifest).map_err(|_| "No se pudo preparar el manifiesto.".to_string())?)
                .map_err(|_| "No se pudo guardar el manifiesto.".to_string())?;
            fs::rename(&staging, &folder).map_err(|_| "No se pudo completar la copia.".to_string())?;
            Ok(info)
        })();
        if operation.is_err() { let _ = fs::remove_dir_all(&staging); }
        operation
    }).await.map_err(|_| "No se pudo crear la copia.".to_string())?
}

async fn restore_backup(dir: PathBuf, root: PathBuf, id: String) -> Result<RestoreReport, String> {
    tokio::task::spawn_blocking(move || {
        if game_running() { return Err("Cierra Minecraft antes de restaurar mods.".into()); }
        let manifest = read_backup(&root, &id)?;
        let current = read_pack(&dir)?;
        if current.version != manifest.info.version {
            return Err("Esta copia es de otra versión. Solo puede reparar la misma versión instalada.".into());
        }
        let expected: HashSet<_> = current.mod_files.iter().map(String::as_str).collect();
        if manifest.files.len() != expected.len() || manifest.files.iter().any(|f| !expected.contains(f.name.as_str())) {
            return Err("El listado de mods de la copia no coincide con la instalación.".into());
        }
        let mods = checked_mods_dir(&dir)?;
        let mut restored_files = 0;
        for entry in manifest.files {
            let target = mods.join(&entry.name);
            if regular_file(&target) && archive_ok(&target) { continue; }
            if target.exists() && !regular_file(&target) {
                return Err(format!("{} no es un archivo normal; se detuvo la restauración.", entry.name));
            }
            fs::create_dir_all(&mods).map_err(|_| "No se pudo preparar la carpeta de mods.".to_string())?;
            let temp = mods.join(format!(".{}.{}.tmp", entry.name, unique_id()));
            fs::copy(root.join(&id).join("mods").join(&entry.name), &temp)
                .map_err(|_| format!("No se pudo preparar {}.", entry.name))?;
            if sha256(&temp)? != entry.sha256 || !archive_ok(&temp) {
                let _ = fs::remove_file(&temp);
                return Err(format!("La copia de {} no supera la verificación.", entry.name));
            }
            let mut displaced = None;
            if target.exists() {
                // Preserve the damaged file inside backup storage before replacing it.
                let saved = root.join(&id).join(format!("damaged-{}-{}", unique_id(), entry.name));
                fs::rename(&target, &saved).map_err(|_| format!("No se pudo preservar {} antes de repararlo.", entry.name))?;
                displaced = Some(saved);
            }
            if fs::rename(&temp, &target).is_err() {
                if let Some(saved) = displaced { let _ = fs::rename(saved, &target); }
                let _ = fs::remove_file(&temp);
                return Err(format!("No se pudo restaurar {}.", entry.name));
            }
            restored_files += 1;
        }
        Ok(RestoreReport { restored_files, version: current.version })
    }).await.map_err(|_| "No se pudo restaurar la copia.".to_string())?
}

#[async_trait]
impl LocalDiagnostics for FsLocalDiagnostics {
    async fn device(&self) -> PortResult<DeviceDiagnostics> {
        get_device_diagnostics().await.map_err(PortError::Other)
    }

    async fn scan(&self) -> PortResult<ScanReport> {
        scan_installation(self.data_dir.clone()).await.map_err(PortError::Other)
    }

    async fn backups(&self) -> PortResult<Vec<BackupInfo>> {
        list_backups(self.data_dir.join("backups")).await.map_err(PortError::Other)
    }

    async fn create_backup(&self) -> PortResult<BackupInfo> {
        create_backup(self.data_dir.clone(), self.data_dir.join("backups"))
            .await.map_err(PortError::Other)
    }

    async fn restore_backup(&self, id: String) -> PortResult<RestoreReport> {
        restore_backup(self.data_dir.clone(), self.data_dir.join("backups"), id)
            .await.map_err(PortError::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_ZIP: [u8; 22] = [
        0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    #[test]
    fn reject_paths_and_non_archives() {
        assert!(valid_name("mod-1.jar"));
        for invalid in ["../escape.jar", "C:\\evil.jar", "folder/mod.jar", "plain.txt", ".."] {
            assert!(!valid_name(invalid));
        }
    }

    #[tokio::test]
    async fn scan_reports_missing_and_corrupt_mods_from_isolated_installation() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("instance").join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(dir.path().join(METADATA), serde_json::json!({
            "version": "test",
            "mod_files": ["good.jar", "missing.jar", "corrupt.jar"]
        }).to_string()).unwrap();
        fs::write(mods.join("good.jar"), EMPTY_ZIP).unwrap();
        fs::write(mods.join("corrupt.jar"), b"not a zip").unwrap();

        let report = scan_installation(dir.path().to_path_buf()).await.unwrap();
        assert_eq!(report.status, "issues");
        assert_eq!(report.checked_files, 2);
        assert_eq!(report.missing_files, ["missing.jar"]);
        assert_eq!(report.corrupt_archives, ["corrupt.jar"]);
    }

    #[test]
    fn backup_validation_rejects_modified_mod() {
        let dir = tempfile::tempdir().unwrap();
        let id = "123-456";
        let folder = dir.path().join(id);
        fs::create_dir_all(folder.join("mods")).unwrap();
        let mod_path = folder.join("mods").join("good.jar");
        fs::write(&mod_path, EMPTY_ZIP).unwrap();
        let manifest = BackupManifest {
            info: BackupInfo { id: id.into(), version: "test".into(), file_count: 1 },
            files: vec![BackupFile { name: "good.jar".into(), sha256: sha256(&mod_path).unwrap() }],
        };
        fs::write(folder.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(read_backup(dir.path(), id).is_ok());
        fs::write(&mod_path, b"modified").unwrap();
        assert!(read_backup(dir.path(), id).is_err());
    }
}
