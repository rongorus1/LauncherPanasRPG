//! End-to-end checks against the real Mojang and Forge servers. Slow and bandwidth-heavy.

mod common;

use common::{RecordingReporter, http};
use launcher_core::application::ports::outbound::{JavaProvider, JavaRequirement, LoaderInstaller};
use launcher_infrastructure::{ForgeLoaderInstaller, MojangJavaProvider};

#[tokio::test]
#[ignore = "downloads a full Java runtime and runs the real Forge installer"]
async fn installs_real_forge_with_mojang_java() {
    let data = tempfile::tempdir().unwrap();
    let requirement = JavaRequirement {
        component: "java-runtime-gamma".into(),
        major: 17,
    };
    let runtime = MojangJavaProvider::new(http())
        .ensure_runtime(
            &requirement,
            &data.path().join("runtimes"),
            &RecordingReporter::default(),
        )
        .await
        .unwrap();
    let output = std::process::Command::new(&runtime.executable)
        .arg("-version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("17."));

    let root = data.path().join("game");
    let installer = ForgeLoaderInstaller::new(http());
    let id = installer
        .install_forge(
            "1.20.1",
            "47.4.10",
            &root,
            &runtime.executable,
            &RecordingReporter::default(),
        )
        .await
        .unwrap();
    assert_eq!(id, "1.20.1-forge-47.4.10");
    let descriptor = installer.read_installed_version(&root, &id).await.unwrap();
    assert_eq!(descriptor.inherits_from.as_deref(), Some("1.20.1"));
    assert!(descriptor.libraries.len() > 10);
}
