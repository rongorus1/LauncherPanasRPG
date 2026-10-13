mod common;

use common::{http, sha1_hex, zip_bytes};
use launcher_core::{
    application::{error::PortError, ports::outbound::PackSource},
    domain::{ContentKind, ModFileRef, PackRelease, PackVersion},
};
use launcher_infrastructure::CurseForgePackSource;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{body_partial_json, header, method, path, query_param},
};

const PROJECT: u64 = 1_493_979;
const KEY: &str = "test-key";

fn source(server: &MockServer) -> CurseForgePackSource {
    CurseForgePackSource::new(http(), KEY, PROJECT)
        .with_api_base(format!("{}/v1", server.uri()))
        .with_cdn_base(format!("{}/cdn", server.uri()))
}

const MANIFEST: &str = r#"{
  "minecraft": { "version": "1.20.1", "modLoaders": [{ "id": "forge-47.4.10", "primary": true }] },
  "manifestType": "minecraftModpack",
  "manifestVersion": 1,
  "name": "Panas RPG",
  "version": "1.7.2",
  "author": "GatoArtStudio",
  "files": [
    { "projectID": 10, "fileID": 100, "required": true },
    { "projectID": 11, "fileID": 110, "required": true },
    { "projectID": 12, "fileID": 120, "required": false }
  ],
  "overrides": "overrides"
}"#;

#[tokio::test]
async fn latest_release_picks_highest_release_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{PROJECT}/files")))
        .and(query_param("pageSize", "50"))
        .and(header("x-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "id": 1, "fileName": "Panas RPG 1.7.0.zip", "releaseType": 1 },
                { "id": 4, "fileName": "Panas RPG 1.9.0.zip", "releaseType": 2 },
                { "id": 2, "fileName": "Panas RPG 1.10.zip", "releaseType": 1 },
                { "id": 3, "fileName": "Panas RPG 1.7.2.zip", "releaseType": 1 },
                { "id": 5, "fileName": "notes.zip", "releaseType": 1 }
            ],
            "pagination": { "index": 0, "pageSize": 50, "resultCount": 5, "totalCount": 5 }
        })))
        .mount(&server)
        .await;

    let release = source(&server).latest_release().await.unwrap();

    assert_eq!(
        release,
        PackRelease {
            version: PackVersion::new(1, 10, 0),
            file_id: 2,
            file_name: "Panas RPG 1.10.zip".into()
        }
    );
}

#[tokio::test]
async fn rejected_api_key_gives_a_clear_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let error = source(&server).latest_release().await.unwrap_err();

    match error {
        PortError::Other(message) => assert!(message.contains("API key"), "{message}"),
        other => panic!("unexpected {other:?}"),
    }
}

fn file_info(id: u64, name: &str, download_url: Option<&str>, sha1: &str) -> Value {
    json!({ "data": {
        "id": id, "fileName": name, "releaseType": 1, "downloadUrl": download_url,
        "hashes": [{ "value": "ignored-md5", "algo": 2 }, { "value": sha1, "algo": 1 }]
    }})
}

#[tokio::test]
async fn fetch_pack_extracts_manifest_and_overrides() {
    let server = MockServer::start().await;
    let archive = zip_bytes(&[
        ("manifest.json", MANIFEST.as_bytes()),
        ("modlist.html", b"<ul></ul>"),
        ("overrides/config/foo.toml", b"a = 1"),
        ("overrides/options.txt", b"resourcePacks:[\"vanilla\"]"),
    ]);
    let download = format!("{}/files/pack.zip", server.uri());
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{PROJECT}/files/77")))
        .respond_with(ResponseTemplate::new(200).set_body_json(file_info(
            77,
            "Panas RPG 1.7.2.zip",
            Some(&download),
            &sha1_hex(&archive),
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/pack.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(&server)
        .await;
    let staging = tempfile::tempdir().unwrap();
    std::fs::write(staging.path().join("stale.txt"), "old").unwrap();
    let release = PackRelease {
        version: PackVersion::new(1, 7, 2),
        file_id: 77,
        file_name: "Panas RPG 1.7.2.zip".into(),
    };

    let manifest = source(&server)
        .fetch_pack(&release, staging.path())
        .await
        .unwrap();

    assert_eq!(manifest.name, "Panas RPG");
    assert_eq!(manifest.version, PackVersion::new(1, 7, 2));
    assert_eq!(manifest.minecraft_version, "1.20.1");
    assert_eq!(manifest.forge_version, "47.4.10");
    assert_eq!(
        manifest.files,
        vec![
            ModFileRef {
                project_id: 10,
                file_id: 100
            },
            ModFileRef {
                project_id: 11,
                file_id: 110
            }
        ]
    );
    assert_eq!(
        std::fs::read_to_string(staging.path().join("overrides/config/foo.toml")).unwrap(),
        "a = 1"
    );
    assert!(staging.path().join("overrides/options.txt").exists());
    assert!(!staging.path().join("stale.txt").exists());
}

#[tokio::test]
async fn fetch_pack_falls_back_to_forgecdn_when_download_url_is_null() {
    let server = MockServer::start().await;
    let archive = zip_bytes(&[("manifest.json", MANIFEST.as_bytes())]);
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{PROJECT}/files/4567012")))
        .respond_with(ResponseTemplate::new(200).set_body_json(file_info(
            4_567_012,
            "Panas RPG 1.7.2.zip",
            None,
            &sha1_hex(&archive),
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/files/4567/12/Panas%20RPG%201.7.2.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .expect(1)
        .mount(&server)
        .await;
    let staging = tempfile::tempdir().unwrap();
    let release = PackRelease {
        version: PackVersion::new(1, 7, 2),
        file_id: 4_567_012,
        file_name: "Panas RPG 1.7.2.zip".into(),
    };

    let manifest = source(&server)
        .fetch_pack(&release, staging.path())
        .await
        .unwrap();

    assert_eq!(manifest.forge_version, "47.4.10");
}

#[tokio::test]
async fn fetch_pack_rejects_zip_slip_entries() {
    let server = MockServer::start().await;
    let archive = zip_bytes(&[
        ("manifest.json", MANIFEST.as_bytes()),
        ("../escaped.txt", b"evil"),
    ]);
    let download = format!("{}/files/pack.zip", server.uri());
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{PROJECT}/files/9")))
        .respond_with(ResponseTemplate::new(200).set_body_json(file_info(
            9,
            "p.zip",
            Some(&download),
            &sha1_hex(&archive),
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/pack.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(&server)
        .await;
    let root = tempfile::tempdir().unwrap();
    let staging = root.path().join("staging");
    let release = PackRelease {
        version: PackVersion::new(1, 0, 0),
        file_id: 9,
        file_name: "p.zip".into(),
    };

    let error = source(&server)
        .fetch_pack(&release, &staging)
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::Integrity(_)), "{error:?}");
    assert!(!root.path().join("escaped.txt").exists());
}

/// Answers `POST /mods/files` with one entry per requested id.
struct FilesResponder;

impl Respond for FilesResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let ids = body["fileIds"].as_array().unwrap();
        assert!(ids.len() <= 1000);
        let data: Vec<Value> = ids
            .iter()
            .map(|id| {
                let id = id.as_u64().unwrap();
                let url = if id % 2 == 0 { Value::String(format!("https://example.test/{id}.jar")) } else { Value::Null };
                json!({
                    "id": id, "modId": id / 10, "fileName": format!("mod {id}.jar"), "downloadUrl": url,
                    "hashes": [{ "value": format!("MD5{id}"), "algo": 2 }, { "value": format!("SHA{id}"), "algo": 1 }]
                })
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({ "data": data }))
    }
}

/// Answers `POST /mods`: project 3 is a resource pack (class 12), project 4 a shader pack (6552), the rest are mods.
struct ProjectsResponder;

impl Respond for ProjectsResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let data: Vec<Value> = body["modIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                let id = id.as_u64().unwrap();
                let class_id = match id {
                    3 => 12,
                    4 => 6552,
                    _ => 6,
                };
                json!({ "id": id, "classId": class_id })
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({ "data": data }))
    }
}

async fn mount_projects(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .and(header("x-api-key", KEY))
        .respond_with(ProjectsResponder)
        .mount(server)
        .await;
}

#[tokio::test]
async fn resolve_files_batches_requests_and_maps_hashes() {
    let server = MockServer::start().await;
    mount_projects(&server).await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(header("x-api-key", KEY))
        .respond_with(FilesResponder)
        .expect(2)
        .mount(&server)
        .await;
    let refs: Vec<ModFileRef> = (1..=1500)
        .map(|i| ModFileRef {
            project_id: i,
            file_id: 1000 + i,
        })
        .collect();

    let resolved = source(&server).resolve_files(&refs).await.unwrap();

    assert_eq!(resolved.len(), 1500);
    let even = &resolved[1];
    assert_eq!(even.reference, refs[1]);
    assert_eq!(even.file_name, "mod 1002.jar");
    assert_eq!(even.download_url, "https://example.test/1002.jar");
    assert_eq!(even.sha1.as_deref(), Some("sha1002"));
    let odd = &resolved[0];
    assert_eq!(
        odd.download_url,
        format!("{}/cdn/files/1/1/mod 1001.jar", server.uri())
    );
    assert_eq!(odd.kind, ContentKind::Mod);
    assert_eq!(resolved[2].kind, ContentKind::ResourcePack);
    assert_eq!(resolved[3].kind, ContentKind::ShaderPack);
}

#[tokio::test]
async fn resolve_files_reports_missing_ids() {
    let server = MockServer::start().await;
    mount_projects(&server).await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(body_partial_json(json!({ "fileIds": [5, 6] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [
            { "id": 5, "fileName": "a.jar", "downloadUrl": "https://x/a.jar", "hashes": [] }
        ]})))
        .mount(&server)
        .await;

    let error = source(&server)
        .resolve_files(&[
            ModFileRef {
                project_id: 1,
                file_id: 5,
            },
            ModFileRef {
                project_id: 2,
                file_id: 6,
            },
        ])
        .await
        .unwrap_err();

    assert!(
        matches!(&error, PortError::NotFound(m) if m.contains('6')),
        "{error:?}"
    );
}
