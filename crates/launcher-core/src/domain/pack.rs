use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::DomainError;

/// Pack version such as `1.7.2`. Missing components count as zero (`1.7` equals `1.7.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl PackVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl FromStr for PackVersion {
    type Err = DomainError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let invalid = || DomainError::InvalidPackVersion(raw.to_owned());
        let parts: Vec<&str> = raw.trim().trim_start_matches('v').split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(invalid());
        }
        let mut numbers = [0u32; 3];
        for (slot, part) in numbers.iter_mut().zip(&parts) {
            *slot = part.parse().map_err(|_| invalid())?;
        }
        Ok(Self::new(numbers[0], numbers[1], numbers[2]))
    }
}

impl TryFrom<String> for PackVersion {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<PackVersion> for String {
    fn from(value: PackVersion) -> Self {
        value.to_string()
    }
}

impl fmt::Display for PackVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for PackVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// A published release of the pack (a file of the CurseForge project).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRelease {
    pub version: PackVersion,
    pub file_id: u64,
    pub file_name: String,
}

/// Reference to a mod file listed in the pack manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModFileRef {
    pub project_id: u64,
    pub file_id: u64,
}

/// Kind of content a pack file is; decides the instance folder it goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    #[default]
    Mod,
    ResourcePack,
    ShaderPack,
}

impl ContentKind {
    pub fn folder(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::ShaderPack => "shaderpacks",
        }
    }
}

/// A pack file with everything needed to download and verify it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedModFile {
    pub reference: ModFileRef,
    pub kind: ContentKind,
    pub file_name: String,
    pub download_url: String,
    pub sha1: Option<String>,
}

/// What a pack release declares: game version, Forge version and mod files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub name: String,
    pub version: PackVersion,
    pub minecraft_version: String,
    /// Forge version without the `forge-` prefix, e.g. `47.4.10`.
    pub forge_version: String,
    pub files: Vec<ModFileRef>,
}

/// Version of the on-disk layout the installer produces. Bump it when a fix needs files re-installed.
pub const INSTALL_LAYOUT: u32 = 2;

fn legacy_layout() -> u32 {
    1
}

/// The pack currently installed in the instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPack {
    pub version: PackVersion,
    pub minecraft_version: String,
    pub forge_version: String,
    /// Id of the launchable version, e.g. `1.20.1-forge-47.4.10`.
    pub launch_version_id: String,
    pub mod_files: Vec<String>,
    /// Installs from before this field existed put resource packs in `mods/`.
    #[serde(default = "legacy_layout")]
    pub layout: u32,
}

impl InstalledPack {
    pub fn needs_repair(&self) -> bool {
        self.layout < INSTALL_LAYOUT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders_versions() {
        let a: PackVersion = "1.7.2".parse().unwrap();
        let b: PackVersion = "1.7".parse().unwrap();
        assert!(a > b);
        assert_eq!(b, PackVersion::new(1, 7, 0));
        assert_eq!(a.to_string(), "1.7.2");
        assert!("1.x".parse::<PackVersion>().is_err());
    }
}
