#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;

use common::{RecordingReporter, http, sha1_hex};
use launcher_core::application::ports::outbound::{JavaProvider, JavaRequirement};
use launcher_infrastructure::MojangJavaProvider;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
async fn installs_runtime_with_executables_and_links_then_reuses_it() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let java = b"#!/bin/sh\necho fake java".to_vec();
    let lib = b"libjvm".to_vec();
    let manifest = json!({ "files": {
        "bin": { "type": "directory" },
        "bin/java": { "type": "file", "executable": true,
            "downloads": { "raw": { "url": format!("{uri}/raw/java"), "sha1": sha1_hex(&java), "size": java.len() } } },
        "lib": { "type": "directory" },
        "lib/libjvm.so": { "type": "file", "executable": false,
            "downloads": { "raw": { "url": format!("{uri}/raw/libjvm"), "sha1": sha1_hex(&lib), "size": lib.len() } } },
        "lib/alias.so": { "type": "link", "target": "libjvm.so" }
    }})
    .to_string();
    let index = json!({
        "linux": { "java-runtime-gamma": [{
            "availability": { "group": 1, "progress": 100 },
            "manifest": { "url": format!("{uri}/manifest.json"), "sha1": sha1_hex(manifest.as_bytes()), "size": manifest.len() },
            "version": { "name": "17.0.8", "released": "2023-01-01T00:00:00+00:00" }
        }] },
        "windows-x64": { "java-runtime-gamma": [] }
    });
    for (route, body) in [
        ("/all.json", index.to_string().into_bytes()),
        ("/manifest.json", manifest.into_bytes()),
        ("/raw/java", java.clone()),
        ("/raw/libjvm", lib),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .expect(1)
            .mount(&server)
            .await;
    }
    let provider = MojangJavaProvider::new(http())
        .with_index_url(format!("{uri}/all.json"))
        .with_platform_key("linux");
    let runtimes = tempfile::tempdir().unwrap();
    let requirement = JavaRequirement {
        component: "java-runtime-gamma".into(),
        major: 17,
    };

    let runtime = provider
        .ensure_runtime(&requirement, runtimes.path(), &RecordingReporter::default())
        .await
        .unwrap();

    let home = runtimes
        .path()
        .join("java-runtime-gamma/linux/java-runtime-gamma");
    assert_eq!(runtime.executable, home.join("bin/java"));
    assert_eq!(runtime.major, 17);
    assert_eq!(std::fs::read(&runtime.executable).unwrap(), java);
    assert_ne!(
        std::fs::metadata(&runtime.executable)
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );
    assert_eq!(
        std::fs::read_link(home.join("lib/alias.so"))
            .unwrap()
            .to_str(),
        Some("libjvm.so")
    );

    // Second call must not touch the network (each mock expects a single hit).
    let again = provider
        .ensure_runtime(&requirement, runtimes.path(), &RecordingReporter::default())
        .await
        .unwrap();
    assert_eq!(again.executable, runtime.executable);
}

#[tokio::test]
async fn unpublished_platform_is_unsupported() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/all.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "linux": { "java-runtime-gamma": [] } })),
        )
        .mount(&server)
        .await;
    let provider = MojangJavaProvider::new(http())
        .with_index_url(format!("{}/all.json", server.uri()))
        .with_platform_key("linux");
    let runtimes = tempfile::tempdir().unwrap();
    let requirement = JavaRequirement {
        component: "java-runtime-gamma".into(),
        major: 17,
    };

    let error = provider
        .ensure_runtime(&requirement, runtimes.path(), &RecordingReporter::default())
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            launcher_core::application::error::PortError::Unsupported(_)
        ),
        "{error:?}"
    );
}
