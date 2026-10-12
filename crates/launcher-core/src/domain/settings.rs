use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::DomainError;

pub const MIN_MEMORY_MB: u32 = 1024;
pub const MAX_MEMORY_MB: u32 = 65_536;

/// Player-editable launcher settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub max_memory_mb: u32,
    pub min_memory_mb: u32,
    /// Extra JVM arguments, space separated.
    pub extra_jvm_args: String,
    /// Use this Java instead of the downloaded runtime.
    pub java_path: Option<PathBuf>,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub close_launcher_on_play: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_memory_mb: 6144,
            min_memory_mb: 2048,
            extra_jvm_args: String::new(),
            java_path: None,
            window_width: 1280,
            window_height: 720,
            fullscreen: false,
            close_launcher_on_play: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !(MIN_MEMORY_MB..=MAX_MEMORY_MB).contains(&self.max_memory_mb) {
            return Err(DomainError::InvalidSetting(format!(
                "max memory must be between {MIN_MEMORY_MB} and {MAX_MEMORY_MB} MB"
            )));
        }
        if self.min_memory_mb < 256 || self.min_memory_mb > self.max_memory_mb {
            return Err(DomainError::InvalidSetting(
                "min memory must be at least 256 MB and not above max memory".into(),
            ));
        }
        if self.window_width < 640 || self.window_height < 480 {
            return Err(DomainError::InvalidSetting(
                "window must be at least 640x480".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_bad_memory_is_rejected() {
        assert!(Settings::default().validate().is_ok());
        let bad = Settings {
            max_memory_mb: 512,
            ..Settings::default()
        };
        assert!(bad.validate().is_err());
        let inverted = Settings {
            min_memory_mb: 8192,
            ..Settings::default()
        };
        assert!(inverted.validate().is_err());
    }
}
