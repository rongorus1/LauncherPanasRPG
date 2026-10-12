#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;

use common::{RecordingReporter, http};
use launcher_core::application::{error::PortError, ports::outbound::LoaderInstaller};
use launcher_infrastructure::ForgeLoaderInstaller;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const INSTALLER_PATH: &str =
    "/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar";

/// A fake `java` that behaves like `java -jar installer --installClient <root>`.
fn fake_java(dir: &std::path::Path, script_body: &str) -> std::path::PathBuf {
    let path = dir.join("java");
    std::fs::write(
        &path,
        format!("#!/bin/sh\nset -e\nroot=\"$4\"\n{script_body}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

async fn server_with_installer() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(INSTALLER_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("installer"))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn runs_the_installer_and_reads_the_descriptor() {
    let server = server_with_installer().await;
    let tools = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let java = fake_java(
        tools.path(),
        r#"test "$1" = "-jar"; test "$3" = "--installClient"; test -f "$root/launcher_profiles.json"
echo "Installing Forge"
mkdir -p "$root/versions/1.20.1-forge-47.4.10"
echo '{"id":"1.20.1-forge-47.4.10","inheritsFrom":"1.20.1","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher"}' > "$root/versions/1.20.1-forge-47.4.10/1.20.1-forge-47.4.10.json""#,
    );
    let installer = ForgeLoaderInstaller::new(http()).with_maven_base(server.uri());
    let reporter = RecordingReporter::default();

    let id = installer
        .install_forge("1.20.1", "47.4.10", root.path(), &java, &reporter)
        .await
        .unwrap();

    assert_eq!(id, "1.20.1-forge-47.4.10");
    assert!(
        reporter
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.message == "Installing Forge")
    );
    let descriptor = installer
        .read_installed_version(root.path(), &id)
        .await
        .unwrap();
    assert_eq!(descriptor.inherits_from.as_deref(), Some("1.20.1"));
    assert_eq!(
        descriptor.main_class.as_deref(),
        Some("cpw.mods.bootstraplauncher.BootstrapLauncher")
    );
}

#[tokio::test]
async fn failing_installer_reports_its_output() {
    let server = server_with_installer().await;
    let tools = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let java = fake_java(tools.path(), "echo 'boom: missing vanilla jar' >&2; exit 1");
    let installer = ForgeLoaderInstaller::new(http()).with_maven_base(server.uri());

    let error = installer
        .install_forge(
            "1.20.1",
            "47.4.10",
            root.path(),
            &java,
            &RecordingReporter::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(&error, PortError::Other(m) if m.contains("boom")),
        "{error:?}"
    );
}

#[tokio::test]
async fn reading_a_missing_version_is_not_found() {
    let root = tempfile::tempdir().unwrap();
    let error = ForgeLoaderInstaller::new(http())
        .read_installed_version(root.path(), "nope")
        .await
        .unwrap_err();
    assert!(matches!(error, PortError::NotFound(_)), "{error:?}");
}
