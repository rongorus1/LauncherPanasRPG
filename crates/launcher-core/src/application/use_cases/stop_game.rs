use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    error::AppError,
    ports::{inbound::StopGame, outbound::GameProcess},
};

/// Kills the game; `PlayGameUseCase` notices the exit and publishes `GameExited`.
pub struct StopGameUseCase {
    game: Arc<dyn GameProcess>,
}

impl StopGameUseCase {
    pub fn new(game: Arc<dyn GameProcess>) -> Self {
        Self { game }
    }
}

#[async_trait]
impl StopGame for StopGameUseCase {
    async fn execute(&self) -> Result<(), AppError> {
        self.game.stop().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_cases::testing::FakeWorld;

    #[tokio::test]
    async fn asks_the_process_to_stop() {
        let world = FakeWorld::new();
        StopGameUseCase::new(world.clone()).execute().await.unwrap();
        assert_eq!(world.calls(), ["stop"]);
    }
}
