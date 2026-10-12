use serde::{Deserialize, Serialize};

/// A newer version of the launcher itself (not of the modpack).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherUpdate {
    pub version: String,
    pub notes: Option<String>,
}
