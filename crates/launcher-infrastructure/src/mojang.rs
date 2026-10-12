use std::{collections::HashMap, path::Path, sync::Arc};

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{
            DownloadTask, FileDownloader, MinecraftSource, PortResult, ProgressReporter,
        },
    },
    domain::{
        InstallStep, Progress,
        version::{Library, Rule, RuleAction, VersionDescriptor},
    },
};
use serde::Deserialize;

use crate::{
    download::ParallelDownloader,
    error::{from_io, from_json},
    http::HttpClient,
    platform::Platform,
};

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES_BASE: &str = "https://resources.download.minecraft.net";
pub const DEFAULT_LIBRARIES_BASE: &str = "https://libraries.minecraft.net/";

/// Vanilla Minecraft from Mojang's piston servers.
#[derive(Clone)]
pub struct MojangMinecraftSource {
    http: HttpClient,
    downloader: Arc<dyn FileDownloader>,
    manifest_url: String,
    resources_base: String,
    platform: Platform,
}

impl MojangMinecraftSource {
    pub fn new(http: HttpClient) -> Self {
        let downloader = Arc::new(ParallelDownloader::for_step(
            http.clone(),
            InstallStep::DownloadingMinecraft,
        ));
        Self {
            http,
            downloader,
            manifest_url: VERSION_MANIFEST_URL.to_owned(),
            resources_base: RESOURCES_BASE.to_owned(),
            platform: Platform::current(),
        }
    }

    pub fn with_downloader(mut self, downloader: Arc<dyn FileDownloader>) -> Self {
        self.downloader = downloader;
        self
    }

    pub fn with_manifest_url(mut self, url: impl Into<String>) -> Self {
        self.manifest_url = url.into();
        self
    }

    pub fn with_resources_base(mut self, base: impl Into<String>) -> Self {
        self.resources_base = base.into().trim_end_matches('/').to_owned();
        self
    }
}

#[async_trait]
impl MinecraftSource for MojangMinecraftSource {
    async fn fetch_version(&self, version_id: &str) -> PortResult<VersionDescriptor> {
        let manifest: VersionManifest = self.http.get_json(&self.manifest_url).await?;
        let entry = manifest
            .versions
            .into_iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| PortError::NotFound(format!("Minecraft version {version_id}")))?;
        self.http.get_json(&entry.url).await
    }

    async fn install_game_files(
        &self,
        version: &VersionDescriptor,
        root: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<()> {
        let mut tasks = Vec::new();

        let jar_id = version.inherits_from.as_deref().unwrap_or(&version.id);
        if let Some(client) = version.downloads.as_ref().and_then(|d| d.client.as_ref()) {
            tasks.push(DownloadTask {
                url: client.url.clone(),
                destination: root
                    .join("versions")
                    .join(jar_id)
                    .join(format!("{jar_id}.jar")),
                sha1: client.sha1.clone(),
                size: client.size,
            });
        }

        let libraries_dir = root.join("libraries");
        tasks.extend(
            version
                .libraries
                .iter()
                .flat_map(|library| library_downloads(library, &self.platform))
                .map(|(path, url, sha1, size)| DownloadTask {
                    url,
                    destination: libraries_dir.join(path),
                    sha1,
                    size,
                }),
        );

        if let Some(index) = &version.asset_index {
            let index_path = root
                .join("assets")
                .join("indexes")
                .join(format!("{}.json", index.id));
            let index_task = DownloadTask {
                url: index.url.clone(),
                destination: index_path.clone(),
                sha1: index.sha1.clone(),
                size: index.size,
            };
            self.downloader
                .download_all(vec![index_task], &SilentReporter)
                .await?;
            let raw = tokio::fs::read(&index_path)
                .await
                .map_err(|e| from_io(e, &index_path))?;
            let assets: AssetIndex = serde_json::from_slice(&raw)
                .map_err(|e| from_json(e, &index_path.display().to_string()))?;
            let objects_dir = root.join("assets").join("objects");
            tasks.extend(
                assets
                    .objects
                    .into_values()
                    .filter(|o| o.hash.len() > 2)
                    .map(|object| {
                        let prefix = &object.hash[..2];
                        DownloadTask {
                            url: format!("{}/{prefix}/{}", self.resources_base, object.hash),
                            destination: objects_dir.join(prefix).join(&object.hash),
                            sha1: Some(object.hash.clone()),
                            size: Some(object.size),
                        }
                    }),
            );
        }

        self.downloader.download_all(tasks, reporter).await
    }
}

struct SilentReporter;

impl ProgressReporter for SilentReporter {
    fn report(&self, _progress: Progress) {}
}

/// Evaluates Mojang rules: the last matching rule wins; no rules means allowed.
/// Feature-gated rules never match because the launcher enables no optional features.
pub(crate) fn rules_allow(rules: Option<&[Rule]>, platform: &Platform) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, platform) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, platform: &Platform) -> bool {
    if rule.features.as_ref().is_some_and(|f| !f.is_empty()) {
        return false;
    }
    let Some(os) = &rule.os else {
        return true;
    };
    let name_ok = os.name.as_deref().is_none_or(|name| name == platform.os);
    let arch_ok = os.arch.as_deref().is_none_or(|arch| arch == platform.arch);
    name_ok && arch_ok
}

/// `(relative path, url, sha1, size)` of every file a library needs on this platform.
pub(crate) fn library_downloads(
    library: &Library,
    platform: &Platform,
) -> Vec<(String, String, Option<String>, Option<u64>)> {
    if !rules_allow(library.rules.as_deref(), platform) {
        return Vec::new();
    }
    let mut files = Vec::new();
    let downloads = library.downloads.as_ref();

    if let Some(artifact) = downloads.and_then(|d| d.artifact.as_ref()) {
        // Forge lists the jars its installer generates locally with an empty URL.
        if !artifact.url.is_empty()
            && let Some(path) = artifact.path.clone().or_else(|| maven_path(&library.name))
        {
            files.push((
                path,
                artifact.url.clone(),
                artifact.sha1.clone(),
                artifact.size,
            ));
        }
    } else if downloads.is_none() && library.natives.is_none() {
        let base = library.url.as_deref().unwrap_or(DEFAULT_LIBRARIES_BASE);
        if let Some(path) = maven_path(&library.name) {
            let url = format!("{}/{path}", base.trim_end_matches('/'));
            files.push((path, url, None, None));
        }
    }

    if let Some(classifier) = library.natives.as_ref().and_then(|n| n.get(platform.os)) {
        let classifier =
            classifier.replace("${arch}", if platform.is_64bit() { "64" } else { "32" });
        if let Some(artifact) = downloads
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier))
            && let Some(path) = artifact
                .path
                .clone()
                .or_else(|| maven_path(&format!("{}:{classifier}", library.name)))
        {
            files.push((
                path,
                artifact.url.clone(),
                artifact.sha1.clone(),
                artifact.size,
            ));
        }
    }
    files
}

/// `group:artifact:version[:classifier][@ext]` to `group/path/artifact/version/artifact-version[-classifier].ext`.
pub(crate) fn maven_path(coordinates: &str) -> Option<String> {
    let (coords, extension) = coordinates.split_once('@').unwrap_or((coordinates, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version) = (parts.first()?, parts.get(1)?, parts.get(2)?);
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{}/{artifact}/{version}/{artifact}-{version}{classifier}.{extension}",
        group.replace('.', "/")
    ))
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

#[cfg(test)]
mod tests {
    use launcher_core::domain::version::OsRule;

    use super::*;

    const LINUX: Platform = Platform {
        os: "linux",
        arch: "x86_64",
    };
    const MAC: Platform = Platform {
        os: "osx",
        arch: "arm64",
    };

    fn os_rule(action: RuleAction, name: &str) -> Rule {
        Rule {
            action,
            os: Some(OsRule {
                name: Some(name.to_owned()),
                ..OsRule::default()
            }),
            features: None,
        }
    }

    #[test]
    fn evaluates_os_rules() {
        let allow_all_but_mac = vec![
            Rule {
                action: RuleAction::Allow,
                os: None,
                features: None,
            },
            os_rule(RuleAction::Disallow, "osx"),
        ];
        assert!(rules_allow(Some(&allow_all_but_mac), &LINUX));
        assert!(!rules_allow(Some(&allow_all_but_mac), &MAC));

        let only_linux = vec![os_rule(RuleAction::Allow, "linux")];
        assert!(rules_allow(Some(&only_linux), &LINUX));
        assert!(!rules_allow(Some(&only_linux), &MAC));
        assert!(rules_allow(None, &MAC));
    }

    #[test]
    fn builds_maven_paths() {
        assert_eq!(
            maven_path("net.minecraftforge:forge:1.20.1-47.4.10:universal").as_deref(),
            Some("net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-universal.jar")
        );
        assert_eq!(
            maven_path("a.b:c:1@zip").as_deref(),
            Some("a/b/c/1/c-1.zip")
        );
        assert_eq!(maven_path("broken"), None);
    }

    #[test]
    fn forge_style_library_uses_repository_url() {
        let library = Library {
            name: "cpw.mods:securejarhandler:2.1.10".to_owned(),
            url: Some("https://maven.minecraftforge.net/".to_owned()),
            ..Library::default()
        };
        let files = library_downloads(&library, &LINUX);
        assert_eq!(
            files[0].1,
            "https://maven.minecraftforge.net/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar"
        );
    }

    #[tokio::test]
    #[ignore = "hits the real Mojang servers"]
    async fn fetches_real_1_20_1_descriptor() {
        let source = MojangMinecraftSource::new(HttpClient::new("test").unwrap());
        let descriptor = source.fetch_version("1.20.1").await.unwrap();
        assert_eq!(descriptor.id, "1.20.1");
        assert_eq!(
            descriptor.main_class.as_deref(),
            Some("net.minecraft.client.main.Main")
        );
        assert!(descriptor.downloads.unwrap().client.is_some());
        assert_eq!(
            descriptor.java_version.unwrap().component,
            "java-runtime-gamma"
        );
        assert!(!descriptor.libraries.is_empty());
        let linux_libs = descriptor
            .libraries
            .iter()
            .filter(|l| !library_downloads(l, &LINUX).is_empty())
            .count();
        let mac_libs = descriptor
            .libraries
            .iter()
            .filter(|l| !library_downloads(l, &MAC).is_empty())
            .count();
        assert!(linux_libs > 0 && mac_libs > 0 && linux_libs != descriptor.libraries.len());
    }
}
