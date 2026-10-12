#![cfg(unix)]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use launcher_core::{
    application::{
        events::LogStream,
        ports::outbound::{GameLogSink, GameProcess},
    },
    domain::LaunchPlan,
};
use launcher_infrastructure::GameProcessRunner;

#[derive(Default)]
struct Lines(Mutex<Vec<(LogStream, String)>>);

impl GameLogSink for Lines {
    fn line(&self, stream: LogStream, line: String) {
        self.0.lock().unwrap().push((stream, line));
    }
}

fn shell(script: &str, dir: &std::path::Path) -> LaunchPlan {
    LaunchPlan {
        java: "sh".into(),
        jvm_args: vec!["-c".into()],
        main_class: script.into(),
        game_args: vec![],
        working_dir: dir.to_path_buf(),
    }
}

#[tokio::test]
async fn streams_output_and_returns_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let lines = Arc::new(Lines::default());
    let plan = shell("pwd; echo out; echo err >&2; exit 3", dir.path());

    let exit = GameProcessRunner::new()
        .run(&plan, lines.clone())
        .await
        .unwrap();

    assert_eq!(exit.code, Some(3));
    let lines = lines.0.lock().unwrap();
    assert!(lines.contains(&(LogStream::Stdout, "out".into())));
    assert!(lines.contains(&(LogStream::Stderr, "err".into())));
    let cwd = std::fs::canonicalize(dir.path()).unwrap();
    assert!(
        lines
            .iter()
            .any(|(_, l)| std::path::Path::new(l) == cwd || l == dir.path().to_str().unwrap())
    );
}

#[tokio::test]
async fn stop_kills_the_running_game() {
    let dir = tempfile::tempdir().unwrap();
    let runner = GameProcessRunner::new();
    let plan = shell("exec sleep 30", dir.path());
    let running = {
        let runner = runner.clone();
        tokio::spawn(async move { runner.run(&plan, Arc::new(Lines::default())).await })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;

    runner.stop().await.unwrap();

    let exit = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(exit.code, None);
}

#[tokio::test]
async fn missing_java_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut plan = shell("true", dir.path());
    plan.java = dir.path().join("no-java");

    let error = GameProcessRunner::new()
        .run(&plan, Arc::new(Lines::default()))
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            launcher_core::application::error::PortError::NotFound(_)
        ),
        "{error:?}"
    );
}
