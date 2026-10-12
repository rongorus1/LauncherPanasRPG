mod common;

use std::time::Duration;

use common::{RecordingReporter, http, sha1_hex};
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{DownloadTask, FileDownloader},
    },
    domain::InstallStep,
};
use launcher_infrastructure::ParallelDownloader;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

fn downloader() -> ParallelDownloader {
    ParallelDownloader::for_step(http(), InstallStep::DownloadingMods)
        .with_backoff(Duration::from_millis(1))
}

fn task(
    server: &MockServer,
    name: &str,
    dir: &std::path::Path,
    sha1: Option<String>,
) -> DownloadTask {
    DownloadTask {
        url: format!("{}/{name}", server.uri()),
        destination: dir.join("nested").join(name),
        sha1,
        size: None,
    }
}

#[tokio::test]
async fn downloads_and_verifies_files_reporting_progress() {
    let server = MockServer::start().await;
    for i in 0..20 {
        Mock::given(method("GET"))
            .and(path(format!("/file{i}.jar")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(format!("content {i}")))
            .expect(1)
            .mount(&server)
            .await;
    }
    let dir = tempfile::tempdir().unwrap();
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            let name = format!("file{i}.jar");
            task(
                &server,
                &name,
                dir.path(),
                Some(sha1_hex(format!("content {i}").as_bytes())),
            )
        })
        .collect();
    let reporter = RecordingReporter::default();

    downloader()
        .download_all(tasks.clone(), &reporter)
        .await
        .unwrap();

    for i in 0..20 {
        let contents =
            std::fs::read_to_string(dir.path().join("nested").join(format!("file{i}.jar")))
                .unwrap();
        assert_eq!(contents, format!("content {i}"));
    }
    let last = reporter.last().unwrap();
    assert_eq!(
        (last.step, last.current, last.total),
        (InstallStep::DownloadingMods, 20, 20)
    );
    let leftovers = std::fs::read_dir(dir.path().join("nested"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        })
        .count();
    assert_eq!(leftovers, 0);

    // A second run must not hit the server again (`expect(1)` above is verified on drop).
    downloader().download_all(tasks, &reporter).await.unwrap();
}

#[tokio::test]
async fn wrong_hash_is_retried_then_reported_as_integrity_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bad.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("tampered"))
        .expect(4)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("nested").join("bad.jar");

    let error = downloader()
        .download_all(
            vec![task(
                &server,
                "bad.jar",
                dir.path(),
                Some(sha1_hex(b"original")),
            )],
            &RecordingReporter::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::Integrity(_)), "{error:?}");
    assert!(!destination.exists());
}

#[tokio::test]
async fn transient_server_errors_are_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flaky.jar"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/flaky.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("ok"))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();

    downloader()
        .download_all(
            vec![task(
                &server,
                "flaky.jar",
                dir.path(),
                Some(sha1_hex(b"ok")),
            )],
            &RecordingReporter::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        std::fs::read(dir.path().join("nested/flaky.jar")).unwrap(),
        b"ok"
    );
}

#[tokio::test]
async fn missing_files_fail_fast_as_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();

    let error = downloader()
        .download_all(
            vec![task(&server, "gone.jar", dir.path(), None)],
            &RecordingReporter::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::NotFound(_)), "{error:?}");
}

#[tokio::test]
async fn existing_file_with_wrong_hash_is_replaced() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/lib.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("fresh"))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("nested/lib.jar");
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    std::fs::write(&destination, "stale").unwrap();

    downloader()
        .download_all(
            vec![task(
                &server,
                "lib.jar",
                dir.path(),
                Some(sha1_hex(b"fresh")),
            )],
            &RecordingReporter::default(),
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), b"fresh");
}
