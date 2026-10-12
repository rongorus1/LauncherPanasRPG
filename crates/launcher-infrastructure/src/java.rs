use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{
            DownloadTask, FileDownloader, JavaProvider, JavaRequirement, JavaRuntime, PortResult,
            ProgressReporter,
        },
    },
    domain::InstallStep,
};
use serde::Deserialize;

use crate::{
    download::ParallelDownloader,
    error::{from_io, from_join},
    fs_util::{create_dir_all, set_executable, write_atomic},
    http::HttpClient,
    platform::Platform,
};

pub const JAVA_RUNTIME_INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const COMPLETE_MARKER: &str = ".panas-complete";

/// Java runtimes published by Mojang for the official launcher.
#[derive(Clone)]
pub struct MojangJavaProvider {
    http: HttpClient,
    downloader: Arc<dyn FileDownloader>,
    index_url: String,
    platform_key: Option<&'static str>,
}

impl MojangJavaProvider {
    pub fn new(http: HttpClient) -> Self {
        let downloader = Arc::new(ParallelDownloader::for_step(
            http.clone(),
            InstallStep::PreparingJava,
        ));
        Self {
            http,
            downloader,
            index_url: JAVA_RUNTIME_INDEX_URL.to_owned(),
            platform_key: Platform::current().java_runtime_key(),
        }
    }

    pub fn with_downloader(mut self, downloader: Arc<dyn FileDownloader>) -> Self {
        self.downloader = downloader;
        self
    }

    pub fn with_index_url(mut self, url: impl Into<String>) -> Self {
        self.index_url = url.into();
        self
    }

    /// Overrides the platform key (`linux`, `windows-x64`, `mac-os-arm64`...).
    pub fn with_platform_key(mut self, key: &'static str) -> Self {
        self.platform_key = Some(key);
        self
    }

    fn platform_key(&self) -> PortResult<&'static str> {
        self.platform_key.ok_or_else(|| {
            PortError::Unsupported(format!(
                "no Mojang Java runtime for {}/{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ))
        })
    }

    /// Base directory of a runtime, laid out like the official launcher.
    pub fn runtime_home(runtimes_dir: &Path, component: &str, platform_key: &str) -> PathBuf {
        runtimes_dir
            .join(component)
            .join(platform_key)
            .join(component)
    }

    pub fn executable_path(home: &Path, platform_key: &str) -> PathBuf {
        if platform_key.starts_with("mac-os") {
            home.join("jre.bundle/Contents/Home/bin/java")
        } else if platform_key.starts_with("windows") {
            home.join("bin").join("java.exe")
        } else {
            home.join("bin").join("java")
        }
    }

    /// Fetches the file list of a runtime component for the current platform.
    pub async fn runtime_manifest(&self, component: &str) -> PortResult<RuntimeManifest> {
        let platform_key = self.platform_key()?;
        let index: BTreeMap<String, BTreeMap<String, Vec<RuntimeRelease>>> =
            self.http.get_json(&self.index_url).await?;
        let release = index
            .get(platform_key)
            .and_then(|components| components.get(component))
            .and_then(|releases| releases.first())
            .ok_or_else(|| {
                PortError::Unsupported(format!(
                    "Java runtime {component} is not published for {platform_key}"
                ))
            })?;
        self.http.get_json(&release.manifest.url).await
    }
}

#[async_trait]
impl JavaProvider for MojangJavaProvider {
    async fn ensure_runtime(
        &self,
        requirement: &JavaRequirement,
        runtimes_dir: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<JavaRuntime> {
        let platform_key = self.platform_key()?;
        let home = Self::runtime_home(runtimes_dir, &requirement.component, platform_key);
        let executable = Self::executable_path(&home, platform_key);
        let marker = home.with_file_name(COMPLETE_MARKER);
        if tokio::fs::try_exists(&marker).await.unwrap_or(false)
            && tokio::fs::try_exists(&executable).await.unwrap_or(false)
        {
            return Ok(JavaRuntime {
                executable,
                major: requirement.major,
            });
        }

        let manifest = self.runtime_manifest(&requirement.component).await?;
        let mut tasks = Vec::new();
        let mut executables = Vec::new();
        let mut links = Vec::new();
        for (relative, entry) in &manifest.files {
            let target = safe_join(&home, relative)?;
            match entry {
                RuntimeFile::Directory => create_dir_all(&target).await?,
                RuntimeFile::File {
                    downloads,
                    executable,
                } => {
                    tasks.push(DownloadTask {
                        url: downloads.raw.url.clone(),
                        destination: target.clone(),
                        sha1: Some(downloads.raw.sha1.clone()),
                        size: downloads.raw.size,
                    });
                    if *executable {
                        executables.push(target);
                    }
                }
                RuntimeFile::Link {
                    target: link_target,
                } => links.push((target, link_target.clone())),
            }
        }
        self.downloader.download_all(tasks, reporter).await?;

        tokio::task::spawn_blocking(move || finish_runtime(&executables, &links))
            .await
            .map_err(from_join)??;
        if !tokio::fs::try_exists(&executable).await.unwrap_or(false) {
            return Err(PortError::Integrity(format!(
                "Java runtime has no {}",
                executable.display()
            )));
        }
        write_atomic(&marker, requirement.component.clone().into_bytes()).await?;
        Ok(JavaRuntime {
            executable,
            major: requirement.major,
        })
    }
}

fn safe_join(base: &Path, relative: &str) -> PortResult<PathBuf> {
    let path = Path::new(relative);
    if path
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        Ok(base.join(path))
    } else {
        Err(PortError::Integrity(format!(
            "unsafe path in Java runtime manifest: {relative}"
        )))
    }
}

fn finish_runtime(executables: &[PathBuf], links: &[(PathBuf, String)]) -> PortResult<()> {
    for path in executables {
        set_executable(path).map_err(|e| from_io(e, path))?;
    }
    #[cfg(unix)]
    for (link, target) in links {
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent).map_err(|e| from_io(e, parent))?;
        }
        let _ = std::fs::remove_file(link);
        std::os::unix::fs::symlink(target, link).map_err(|e| from_io(e, link))?;
    }
    #[cfg(not(unix))]
    let _ = links;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RuntimeRelease {
    manifest: RemoteFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteFile {
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeManifest {
    pub files: BTreeMap<String, RuntimeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeFile {
    File {
        downloads: RuntimeDownloads,
        #[serde(default)]
        executable: bool,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct RuntimeDownloads {
    pub raw: RemoteFile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaping_paths() {
        assert!(safe_join(Path::new("/r"), "bin/java").is_ok());
        assert!(safe_join(Path::new("/r"), "../evil").is_err());
        assert!(safe_join(Path::new("/r"), "/etc/passwd").is_err());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    #[ignore = "hits the real Mojang servers"]
    async fn downloads_real_linux_java_binary() {
        let http = HttpClient::new("test").unwrap();
        let provider = MojangJavaProvider::new(http.clone());
        let manifest = provider
            .runtime_manifest("java-runtime-gamma")
            .await
            .unwrap();
        let RuntimeFile::File {
            downloads,
            executable,
        } = &manifest.files["bin/java"]
        else {
            panic!("bin/java is not a file");
        };
        assert!(executable);
        assert!(manifest.files.len() > 100);
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("java");
        crate::download::fetch_to_file(
            &http,
            &downloads.raw.url,
            &destination,
            Some(&downloads.raw.sha1),
            downloads.raw.size,
        )
        .await
        .unwrap();
        set_executable(&destination).unwrap();
        assert!(destination.metadata().unwrap().len() > 0);
    }
}
