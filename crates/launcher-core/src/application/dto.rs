use serde::Serialize;

use crate::domain::{OfflineAccount, Settings};

/// What the UI needs to render the home screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LauncherState {
    pub account: Option<AccountView>,
    pub settings: Settings,
    pub pack: PackStatus,
    pub activity: Activity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountView {
    pub username: String,
    pub uuid: String,
}

impl From<&OfflineAccount> for AccountView {
    fn from(account: &OfflineAccount) -> Self {
        Self {
            username: account.username().as_str().to_owned(),
            uuid: account.uuid().hyphenated().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackStatus {
    pub installed_version: Option<String>,
    pub minecraft_version: Option<String>,
    pub forge_version: Option<String>,
    /// `None` when the latest version could not be checked (offline).
    pub latest_version: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Idle,
    Installing,
    Playing,
}
