use serde::Serialize;

use crate::domain::Progress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
}

/// Notifications pushed from the application to whoever is listening (the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LauncherEvent {
    InstallProgress(Progress),
    InstallFinished { version: String },
    InstallFailed { detail: String },
    GameStarted,
    GameLog { stream: LogStream, line: String },
    GameExited { code: Option<i32> },
    UpdateProgress { downloaded: u64, total: Option<u64> },
}
