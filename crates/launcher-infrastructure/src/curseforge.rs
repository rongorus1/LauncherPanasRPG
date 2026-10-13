use std::{collections::HashMap, fs, path::Path};

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{PackSource, PortResult},
    },
    domain::{ContentKind, ModFileRef, PackManifest, PackRelease, PackVersion, ResolvedModFile},
};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    download::fetch_to_file,
    error::{from_io, from_join, from_json, from_reqwest, from_zip},
    fs_util::{create_dir_all, remove_dir_if_exists},
    http::{HttpClient, ensure_success, read_json},
};

pub const DEFAULT_API_BASE: &str = "https://api.curseforge.com/v1";
pub const DEFAULT_CDN_BASE: &str = "https://edge.forgecdn.net";
const FILES_PAGE_SIZE: u32 = 50;
const RESOLVE_BATCH: usize = 1000;
const RELEASE_TYPE_RELEASE: u8 = 1;
const HASH_ALGO_SHA1: u8 = 1;
const CLASS_RESOURCE_PACKS: u64 = 12;
const CLASS_SHADER_PACKS: u64 = 6552;
const PACK_ARCHIVE: &str = "pack.zip";

/// Reads the pack from the CurseForge Core API.
#[derive(Debug, Clone)]
pub struct CurseForgePackSource {
    http: HttpClient,
    api_key: String,
    project_id: u64,
    api_base: String,
    cdn_base: String,
}

impl CurseForgePackSource {
    pub fn new(http: HttpClient, api_key: impl Into<String>, project_id: u64) -> Self {
        Self {
            http,
            api_key: api_key.into(),
            project_id,
            api_base: DEFAULT_API_BASE.to_owned(),
            cdn_base: DEFAULT_CDN_BASE.to_owned(),
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into().trim_end_matches('/').to_owned();
        self
    }

    pub fn with_cdn_base(mut self, base: impl Into<String>) -> Self {
        self.cdn_base = base.into().trim_end_matches('/').to_owned();
        self
    }

    async fn call<T: DeserializeOwned>(&self, request: RequestBuilder) -> PortResult<T> {
        let response = request
            .header("x-api-key", &self.api_key)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(from_reqwest)?;
        let response = check_api_key(response)?;
        read_json(ensure_success(response)?).await
    }

    /// Mods, resource packs and shader packs share the file endpoints; the project class tells them apart.
    async fn content_kinds(&self, files: &[ModFileRef]) -> PortResult<HashMap<u64, ContentKind>> {
        let mut project_ids: Vec<u64> = files.iter().map(|f| f.project_id).collect();
        project_ids.sort_unstable();
        project_ids.dedup();

        let mut kinds = HashMap::new();
        for batch in project_ids.chunks(RESOLVE_BATCH) {
            let request = self
                .http
                .client()
                .post(format!("{}/mods", self.api_base))
                .json(&serde_json::json!({ "modIds": batch }));
            let envelope: Envelope<Vec<CfProject>> = self.call(request).await?;
            for project in envelope.data {
                let kind = match project.class_id {
                    Some(CLASS_RESOURCE_PACKS) => ContentKind::ResourcePack,
                    Some(CLASS_SHADER_PACKS) => ContentKind::ShaderPack,
                    _ => ContentKind::Mod,
                };
                kinds.insert(project.id, kind);
            }
        }
        Ok(kinds)
    }

    async fn file_info(&self, file_id: u64) -> PortResult<CfFile> {
        let url = format!("{}/mods/{}/files/{file_id}", self.api_base, self.project_id);
        let envelope: Envelope<CfFile> = self.call(self.http.client().get(url)).await?;
        Ok(envelope.data)
    }

    fn download_url(&self, file: &CfFile) -> String {
        match file.download_url.as_deref().filter(|u| !u.is_empty()) {
            Some(url) => url.to_owned(),
            None => cdn_url(&self.cdn_base, file.id, &file.file_name),
        }
    }
}

fn check_api_key(response: Response) -> PortResult<Response> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(PortError::Other(format!(
            "CurseForge rejected the API key ({}). Check the configured CurseForge API key.",
            response.status()
        ))),
        _ => Ok(response),
    }
}

/// Fallback used by CurseForge when a project disables third-party distribution URLs.
pub(crate) fn cdn_url(cdn_base: &str, file_id: u64, file_name: &str) -> String {
    format!(
        "{cdn_base}/files/{}/{}/{file_name}",
        file_id / 1000,
        file_id % 1000
    )
}

/// Finds the last dotted number in a file name such as `Panas RPG 1.7.2.zip`.
pub(crate) fn version_from_file_name(file_name: &str) -> Option<PackVersion> {
    let stem = file_name.strip_suffix(".zip").unwrap_or(file_name);
    let mut candidates = Vec::new();
    let mut current = String::new();
    for c in stem.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == '.' && !current.is_empty()) {
            current.push(c);
        } else if !current.is_empty() {
            candidates.push(std::mem::take(&mut current));
        }
    }
    candidates
        .iter()
        .rev()
        .find_map(|raw| raw.trim_end_matches('.').parse().ok())
}

#[async_trait]
impl PackSource for CurseForgePackSource {
    async fn latest_release(&self) -> PortResult<PackRelease> {
        let url = format!(
            "{}/mods/{}/files?pageSize={FILES_PAGE_SIZE}",
            self.api_base, self.project_id
        );
        let request = self.http.client().get(url);
        let envelope: Envelope<Vec<CfFile>> = self.call(request).await?;
        envelope
            .data
            .into_iter()
            .filter(|f| f.release_type == RELEASE_TYPE_RELEASE && f.is_available.unwrap_or(true))
            .filter_map(|f| {
                let version = version_from_file_name(&f.file_name)
                    .or_else(|| f.display_name.as_deref().and_then(version_from_file_name))?;
                Some(PackRelease {
                    version,
                    file_id: f.id,
                    file_name: f.file_name,
                })
            })
            .max_by(|a, b| a.version.cmp(&b.version).then(a.file_id.cmp(&b.file_id)))
            .ok_or_else(|| {
                PortError::NotFound(format!(
                    "no release found for CurseForge project {}",
                    self.project_id
                ))
            })
    }

    async fn fetch_pack(
        &self,
        release: &PackRelease,
        staging_dir: &Path,
    ) -> PortResult<PackManifest> {
        let info = self.file_info(release.file_id).await?;
        let url = self.download_url(&info);

        remove_dir_if_exists(staging_dir).await?;
        create_dir_all(staging_dir).await?;
        let archive = staging_dir.join(PACK_ARCHIVE);
        fetch_to_file(
            &self.http,
            &url,
            &archive,
            info.sha1().as_deref(),
            info.file_length,
        )
        .await?;

        let staging = staging_dir.to_path_buf();
        let raw_manifest = tokio::task::spawn_blocking(move || extract_pack(&archive, &staging))
            .await
            .map_err(from_join)??;
        let _ = tokio::fs::remove_file(staging_dir.join(PACK_ARCHIVE)).await;
        raw_manifest.into_manifest()
    }

    async fn resolve_files(&self, files: &[ModFileRef]) -> PortResult<Vec<ResolvedModFile>> {
        let mut found: HashMap<u64, CfFile> = HashMap::new();
        for batch in files.chunks(RESOLVE_BATCH) {
            let ids: Vec<u64> = batch.iter().map(|f| f.file_id).collect();
            let url = format!("{}/mods/files", self.api_base);
            let request = self
                .http
                .client()
                .post(url)
                .json(&serde_json::json!({ "fileIds": ids }));
            let envelope: Envelope<Vec<CfFile>> = self.call(request).await?;
            found.extend(envelope.data.into_iter().map(|f| (f.id, f)));
        }

        let kinds = self.content_kinds(files).await?;

        let mut missing = Vec::new();
        let mut resolved = Vec::with_capacity(files.len());
        for reference in files {
            match found.get(&reference.file_id) {
                Some(file) => resolved.push(ResolvedModFile {
                    reference: *reference,
                    kind: kinds
                        .get(&reference.project_id)
                        .copied()
                        .unwrap_or_default(),
                    file_name: file.file_name.clone(),
                    download_url: self.download_url(file),
                    sha1: file.sha1(),
                }),
                None => missing.push(reference.file_id.to_string()),
            }
        }
        if missing.is_empty() {
            Ok(resolved)
        } else {
            Err(PortError::NotFound(format!(
                "CurseForge files not found: {}",
                missing.join(", ")
            )))
        }
    }
}

/// Extracts every entry under `staging`, refusing paths that escape it (zip-slip).
pub(crate) fn extract_pack(archive_path: &Path, staging: &Path) -> PortResult<RawManifest> {
    let file = fs::File::open(archive_path).map_err(|e| from_io(e, archive_path))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| from_zip(e, archive_path))?;
    let mut manifest_bytes = None;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| from_zip(e, archive_path))?;
        let relative = entry.enclosed_name().ok_or_else(|| {
            PortError::Integrity(format!(
                "unsafe path in pack archive: {}",
                entry.name().unwrap_or_default()
            ))
        })?;
        let target = staging.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| from_io(e, &target))?;
            continue;
        }
        if relative == Path::new("manifest.json") {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)
                .map_err(|e| from_io(e, archive_path))?;
            fs::write(&target, &bytes).map_err(|e| from_io(e, &target))?;
            manifest_bytes = Some(bytes);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| from_io(e, parent))?;
        }
        let mut out = fs::File::create(&target).map_err(|e| from_io(e, &target))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| from_io(e, &target))?;
    }

    let bytes = manifest_bytes
        .ok_or_else(|| PortError::Integrity("pack archive has no manifest.json".to_owned()))?;
    let manifest: RawManifest =
        serde_json::from_slice(&bytes).map_err(|e| from_json(e, "manifest.json"))?;

    let declared = staging.join(manifest.overrides.as_deref().unwrap_or("overrides"));
    let expected = staging.join("overrides");
    if declared != expected && declared.is_dir() {
        fs::rename(&declared, &expected).map_err(|e| from_io(e, &declared))?;
    }
    Ok(manifest)
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfProject {
    id: u64,
    #[serde(default)]
    class_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    file_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default = "default_release_type")]
    release_type: u8,
    #[serde(default)]
    is_available: Option<bool>,
    #[serde(default)]
    file_length: Option<u64>,
    #[serde(default)]
    hashes: Vec<CfHash>,
}

fn default_release_type() -> u8 {
    RELEASE_TYPE_RELEASE
}

impl CfFile {
    fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|h| h.algo == HASH_ALGO_SHA1)
            .map(|h| h.value.to_ascii_lowercase())
    }
}

#[derive(Debug, Deserialize)]
struct CfHash {
    value: String,
    algo: u8,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawManifest {
    minecraft: RawMinecraft,
    name: String,
    version: String,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<RawLoader>,
}

#[derive(Debug, Deserialize)]
struct RawLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "default_required")]
    required: bool,
}

fn default_required() -> bool {
    true
}

impl RawManifest {
    pub(crate) fn into_manifest(self) -> PortResult<PackManifest> {
        let loader = self
            .minecraft
            .mod_loaders
            .iter()
            .find(|l| l.primary)
            .or_else(|| self.minecraft.mod_loaders.first())
            .ok_or_else(|| {
                PortError::Integrity("pack manifest declares no mod loader".to_owned())
            })?;
        let forge_version = loader
            .id
            .strip_prefix("forge-")
            .ok_or_else(|| {
                PortError::Unsupported(format!("mod loader {} is not Forge", loader.id))
            })?
            .to_owned();
        let version = version_from_file_name(&self.version).ok_or_else(|| {
            PortError::Integrity(format!("invalid pack version {}", self.version))
        })?;
        Ok(PackManifest {
            name: self.name,
            version,
            minecraft_version: self.minecraft.version,
            forge_version,
            files: self
                .files
                .iter()
                .filter(|f| f.required)
                .map(|f| ModFileRef {
                    project_id: f.project_id,
                    file_id: f.file_id,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_from_file_names() {
        assert_eq!(
            version_from_file_name("Panas RPG 1.7.2.zip"),
            Some(PackVersion::new(1, 7, 2))
        );
        assert_eq!(
            version_from_file_name("PanasRPG-v2.0.zip"),
            Some(PackVersion::new(2, 0, 0))
        );
        assert_eq!(
            version_from_file_name("Panas RPG 1.20.1 - 1.8.zip"),
            Some(PackVersion::new(1, 8, 0))
        );
        assert_eq!(version_from_file_name("Panas RPG.zip"), None);
    }

    #[test]
    fn builds_forgecdn_urls() {
        assert_eq!(
            cdn_url(DEFAULT_CDN_BASE, 4_567_012, "mod.jar"),
            "https://edge.forgecdn.net/files/4567/12/mod.jar"
        );
    }
}
