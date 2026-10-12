use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
};

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{LoaderInstaller, PortResult, ProgressReporter},
    },
    domain::{InstallStep, Progress, version::VersionDescriptor},
};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
    download::fetch_to_file,
    error::{from_io, from_json},
    fs_util::{create_dir_all, remove_dir_if_exists, write_atomic},
    http::HttpClient,
};

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const OUTPUT_TAIL_LINES: usize = 30;

/// Installs Forge by running its official installer in `--installClient` mode.
#[derive(Debug, Clone)]
pub struct ForgeLoaderInstaller {
    http: HttpClient,
    maven_base: String,
}

impl ForgeLoaderInstaller {
    pub fn new(http: HttpClient) -> Self {
        Self {
            http,
            maven_base: FORGE_MAVEN.to_owned(),
        }
    }

    pub fn with_maven_base(mut self, base: impl Into<String>) -> Self {
        self.maven_base = base.into().trim_end_matches('/').to_owned();
        self
    }

    pub fn installer_url(&self, minecraft_version: &str, forge_version: &str) -> String {
        let full = format!("{minecraft_version}-{forge_version}");
        format!(
            "{}/net/minecraftforge/forge/{full}/forge-{full}-installer.jar",
            self.maven_base
        )
    }

    pub fn version_id(minecraft_version: &str, forge_version: &str) -> String {
        format!("{minecraft_version}-forge-{forge_version}")
    }
}

fn descriptor_path(root: &Path, version_id: &str) -> PathBuf {
    root.join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"))
}

/// The client jar is the last file the installer writes, so it marks a finished install.
fn client_jar_path(root: &Path, minecraft_version: &str, forge_version: &str) -> PathBuf {
    let full = format!("{minecraft_version}-{forge_version}");
    root.join(format!(
        "libraries/net/minecraftforge/forge/{full}/forge-{full}-client.jar"
    ))
}

async fn ensure_launcher_profiles(root: &Path) -> PortResult<()> {
    let path = root.join("launcher_profiles.json");
    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(());
    }
    write_atomic(&path, br#"{"profiles":{}}"#.to_vec()).await
}

fn unique_work_dir(version_id: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "panas-forge-{version_id}-{}-{n}",
        std::process::id()
    ))
}

async fn exists(path: &Path) -> bool {
    tokio::fs::try_exists(path).await.unwrap_or(false)
}

#[async_trait]
impl LoaderInstaller for ForgeLoaderInstaller {
    async fn install_forge(
        &self,
        minecraft_version: &str,
        forge_version: &str,
        root: &Path,
        java: &Path,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<String> {
        let version_id = Self::version_id(minecraft_version, forge_version);
        let descriptor = descriptor_path(root, &version_id);
        if exists(&descriptor).await
            && exists(&client_jar_path(root, minecraft_version, forge_version)).await
        {
            return Ok(version_id);
        }

        reporter.report(Progress::new(
            InstallStep::InstallingForge,
            0,
            0,
            "Descargando instalador de Forge",
        ));
        create_dir_all(root).await?;
        ensure_launcher_profiles(root).await?;

        let work_dir = unique_work_dir(&version_id);
        remove_dir_if_exists(&work_dir).await?;
        create_dir_all(&work_dir).await?;
        let installer = work_dir.join("forge-installer.jar");
        let result = async {
            fetch_to_file(
                &self.http,
                &self.installer_url(minecraft_version, forge_version),
                &installer,
                None,
                None,
            )
            .await?;
            run_installer(java, &installer, root, &work_dir, reporter).await
        }
        .await;
        let _ = remove_dir_if_exists(&work_dir).await;
        result?;

        if !exists(&descriptor).await {
            return Err(PortError::Other(format!(
                "Forge installer finished but {} is missing",
                descriptor.display()
            )));
        }
        Ok(version_id)
    }

    async fn read_installed_version(
        &self,
        root: &Path,
        version_id: &str,
    ) -> PortResult<VersionDescriptor> {
        let path = descriptor_path(root, version_id);
        let raw = tokio::fs::read(&path)
            .await
            .map_err(|e| from_io(e, &path))?;
        serde_json::from_slice(&raw).map_err(|e| from_json(e, &path.display().to_string()))
    }
}

async fn run_installer(
    java: &Path,
    installer: &Path,
    root: &Path,
    work_dir: &Path,
    reporter: &dyn ProgressReporter,
) -> PortResult<()> {
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(installer)
        .arg("--installClient")
        .arg(root)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(crate::process::CREATE_NO_WINDOW);

    let mut child = command.spawn().map_err(|e| from_io(e, java))?;
    let stdout = child.stdout.take().map(|s| BufReader::new(s).lines());
    let stderr = child.stderr.take().map(|s| BufReader::new(s).lines());

    let mut tail: Vec<String> = Vec::new();
    let mut lines_seen = 0u64;
    let mut push = |line: String| {
        lines_seen += 1;
        reporter.report(Progress::new(
            InstallStep::InstallingForge,
            lines_seen,
            0,
            line.clone(),
        ));
        if tail.len() == OUTPUT_TAIL_LINES {
            tail.remove(0);
        }
        tail.push(line);
    };

    // The installer may fill either pipe, so both are drained concurrently.
    if let (Some(mut out), Some(mut err)) = (stdout, stderr) {
        let (mut out_done, mut err_done) = (false, false);
        while !(out_done && err_done) {
            tokio::select! {
                line = out.next_line(), if !out_done => match line {
                    Ok(Some(line)) => push(line),
                    _ => out_done = true,
                },
                line = err.next_line(), if !err_done => match line {
                    Ok(Some(line)) => push(line),
                    _ => err_done = true,
                },
            }
        }
    }

    let status = child.wait().await.map_err(|e| from_io(e, java))?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::Other(format!(
            "Forge installer failed ({status}):\n{}",
            tail.join("\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_installer_url_and_id() {
        let installer = ForgeLoaderInstaller::new(HttpClient::new("test").unwrap());
        assert_eq!(
            installer.installer_url("1.20.1", "47.4.10"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar"
        );
        assert_eq!(
            ForgeLoaderInstaller::version_id("1.20.1", "47.4.10"),
            "1.20.1-forge-47.4.10"
        );
    }

    #[tokio::test]
    #[ignore = "hits the real Forge maven"]
    async fn real_installer_url_answers_ok() {
        let http = HttpClient::new("test").unwrap();
        let url = ForgeLoaderInstaller::new(http.clone()).installer_url("1.20.1", "47.4.10");
        let response = http.client().head(&url).send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.status());
    }
}
