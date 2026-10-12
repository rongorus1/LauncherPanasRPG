use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    error::AppError,
    ports::{inbound::SaveSettings, outbound::SettingsRepository},
};
use crate::domain::Settings;

pub struct SaveSettingsUseCase {
    settings: Arc<dyn SettingsRepository>,
}

impl SaveSettingsUseCase {
    pub fn new(settings: Arc<dyn SettingsRepository>) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl SaveSettings for SaveSettingsUseCase {
    async fn execute(&self, settings: Settings) -> Result<Settings, AppError> {
        settings.validate()?;
        self.settings.save(&settings).await?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{error::AppErrorKind, use_cases::testing::FakeWorld};

    #[tokio::test]
    async fn saves_valid_settings() {
        let world = FakeWorld::new();
        let wanted = Settings {
            max_memory_mb: 8192,
            fullscreen: true,
            ..Settings::default()
        };
        let saved = SaveSettingsUseCase::new(world.clone())
            .execute(wanted.clone())
            .await
            .unwrap();
        assert_eq!(saved, wanted);
        assert_eq!(world.settings(), wanted);
    }

    #[tokio::test]
    async fn rejects_invalid_settings_without_saving() {
        let world = FakeWorld::new();
        let bad = Settings {
            window_width: 100,
            ..Settings::default()
        };
        let error = SaveSettingsUseCase::new(world.clone())
            .execute(bad)
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::InvalidInput);
        assert_eq!(world.settings(), Settings::default());
    }
}
