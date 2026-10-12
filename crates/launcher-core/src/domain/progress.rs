use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStep {
    CheckingPack,
    PreparingJava,
    DownloadingMinecraft,
    InstallingForge,
    DownloadingMods,
    ApplyingFiles,
    Finished,
}

/// Progress of the current step. `total == 0` means unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub step: InstallStep,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

impl Progress {
    pub fn new(step: InstallStep, current: u64, total: u64, message: impl Into<String>) -> Self {
        Self {
            step,
            current,
            total,
            message: message.into(),
        }
    }
}
