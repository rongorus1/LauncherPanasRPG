mod common;

use common::{RecordingReporter, http, sha1_hex};
use launcher_core::{
    application::ports::outbound::MinecraftSource,
    domain::{InstallStep, version::VersionDescriptor},
};
use launcher_infrastructure::MojangMinecraftSource;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

async fn serve(server: &MockServer, route: &str, body: impl Into<Vec<u8>>, times: u64) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.into()))
        .expect(times)
        .mount(server)
        .await;
}

fn source(server: &MockServer) -> MojangMinecraftSource {
    MojangMinecraftSource::new(http())
        .with_manifest_url(format!("{}/manifest.json", server.uri()))
        .with_resources_base(format!("{}/objects", server.uri()))
}

#[tokio::test]
async fn fetch_version_follows_the_version_manifest() {
    let server = MockServer::start().await;
    let manifest = json!({
        "latest": { "release": "1.21", "snapshot": "1.21" },
        "versions": [
            { "id": "1.21", "type": "release", "url": format!("{}/v/1.21.json", server.uri()), "sha1": "x" },
            { "id": "1.20.1", "type": "release", "url": format!("{}/v/1.20.1.json", server.uri()), "sha1": "y" }
        ]
    });
    serve(&server, "/manifest.json", manifest.to_string(), 1).await;
    let descriptor = json!({
        "id": "1.20.1", "mainClass": "net.minecraft.client.main.Main", "type": "release",
        "javaVersion": { "component": "java-runtime-gamma", "majorVersion": 17 },
        "libraries": []
    });
    serve(&server, "/v/1.20.1.json", descriptor.to_string(), 1).await;

    let version = source(&server).fetch_version("1.20.1").await.unwrap();

    assert_eq!(version.id, "1.20.1");
    assert_eq!(version.java_version.unwrap().major_version, 17);
}

#[tokio::test]
async fn installs_client_libraries_and_assets_honouring_rules() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let client = b"client jar".to_vec();
    let common_lib = b"common lib".to_vec();
    let forge_lib = b"forge lib".to_vec();
    let asset = b"sound".to_vec();
    let asset_hash = sha1_hex(&asset);
    let index = json!({ "objects": { "minecraft/sounds/a.ogg": { "hash": asset_hash, "size": asset.len() } } }).to_string();

    serve(&server, "/client.jar", client.clone(), 1).await;
    serve(
        &server,
        "/libs/com/example/common/1.0/common-1.0.jar",
        common_lib.clone(),
        1,
    )
    .await;
    serve(
        &server,
        "/maven/net/minecraftforge/fmlcore/1.20.1-47.4.10/fmlcore-1.20.1-47.4.10.jar",
        forge_lib.clone(),
        1,
    )
    .await;
    serve(&server, "/indexes/5.json", index.clone(), 1).await;
    serve(
        &server,
        &format!("/objects/{}/{asset_hash}", &asset_hash[..2]),
        asset.clone(),
        1,
    )
    .await;

    let descriptor: VersionDescriptor = serde_json::from_value(json!({
        "id": "1.20.1-forge-47.4.10",
        "inheritsFrom": "1.20.1",
        "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
        "downloads": { "client": { "url": format!("{uri}/client.jar"), "sha1": sha1_hex(&client), "size": client.len() } },
        "assetIndex": { "id": "5", "url": format!("{uri}/indexes/5.json"), "sha1": sha1_hex(index.as_bytes()), "size": index.len(), "totalSize": 5 },
        "libraries": [
            { "name": "com.example:common:1.0",
              "downloads": { "artifact": { "path": "com/example/common/1.0/common-1.0.jar", "url": format!("{uri}/libs/com/example/common/1.0/common-1.0.jar"), "sha1": sha1_hex(&common_lib), "size": common_lib.len() } } },
            { "name": "com.example:never:1.0",
              "rules": [{ "action": "allow", "os": { "name": "plan9" } }],
              "downloads": { "artifact": { "path": "com/example/never/1.0/never-1.0.jar", "url": format!("{uri}/never.jar"), "sha1": "00", "size": 1 } } },
            { "name": "com.example:disallowed:1.0",
              "rules": [{ "action": "allow" }, { "action": "disallow", "os": { "name": std::env::consts::OS.replace("macos", "osx") } }],
              "downloads": { "artifact": { "path": "com/example/disallowed/1.0/disallowed-1.0.jar", "url": format!("{uri}/never.jar"), "sha1": "00", "size": 1 } } },
            { "name": "net.minecraftforge:fmlcore:1.20.1-47.4.10", "url": format!("{uri}/maven/") },
            { "name": "net.minecraftforge:forge:1.20.1-47.4.10:client",
              "downloads": { "artifact": { "path": "net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-client.jar", "url": "", "sha1": "11", "size": 1 } } }
        ]
    }))
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let reporter = RecordingReporter::default();

    source(&server)
        .install_game_files(&descriptor, root.path(), &reporter)
        .await
        .unwrap();

    let read = |p: &str| std::fs::read(root.path().join(p)).unwrap();
    assert_eq!(read("versions/1.20.1/1.20.1.jar"), client);
    assert_eq!(
        read("libraries/com/example/common/1.0/common-1.0.jar"),
        common_lib
    );
    assert_eq!(
        read("libraries/net/minecraftforge/fmlcore/1.20.1-47.4.10/fmlcore-1.20.1-47.4.10.jar"),
        forge_lib
    );
    assert_eq!(read("assets/indexes/5.json"), index.as_bytes());
    assert_eq!(
        read(&format!("assets/objects/{}/{asset_hash}", &asset_hash[..2])),
        asset
    );
    assert!(!root.path().join("libraries/com/example/never").exists());
    assert!(
        !root
            .path()
            .join("libraries/com/example/disallowed")
            .exists()
    );
    let last = reporter.last().unwrap();
    assert_eq!(
        (last.step, last.current, last.total),
        (InstallStep::DownloadingMinecraft, 4, 4)
    );

    // Already verified files are skipped: every mock above expects exactly one hit, except the
    // Forge library without a hash, which is skipped because it exists.
    source(&server)
        .install_game_files(&descriptor, root.path(), &reporter)
        .await
        .unwrap();
}
