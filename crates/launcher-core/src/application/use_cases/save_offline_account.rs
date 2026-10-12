use std::sync::Arc;

use async_trait::async_trait;

use crate::application::{
    dto::AccountView,
    error::AppError,
    ports::{inbound::SaveOfflineAccount, outbound::AccountRepository},
};
use crate::domain::{OfflineAccount, Username};

pub struct SaveOfflineAccountUseCase {
    accounts: Arc<dyn AccountRepository>,
}

impl SaveOfflineAccountUseCase {
    pub fn new(accounts: Arc<dyn AccountRepository>) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl SaveOfflineAccount for SaveOfflineAccountUseCase {
    async fn execute(&self, username: String) -> Result<AccountView, AppError> {
        let account = OfflineAccount::new(Username::parse(&username)?);
        self.accounts.save(&account).await?;
        Ok(AccountView::from(&account))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{error::AppErrorKind, use_cases::testing::FakeWorld};

    #[tokio::test]
    async fn saves_a_valid_name_and_returns_its_offline_uuid() {
        let world = FakeWorld::new();
        let view = SaveOfflineAccountUseCase::new(world.clone())
            .execute(" Notch ".into())
            .await
            .unwrap();
        assert_eq!(view.username, "Notch");
        assert_eq!(view.uuid, "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(world.account().unwrap().username().as_str(), "Notch");
    }

    #[tokio::test]
    async fn rejects_an_invalid_name_without_saving() {
        let world = FakeWorld::new();
        let error = SaveOfflineAccountUseCase::new(world.clone())
            .execute("no spaces".into())
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::InvalidInput);
        assert!(world.account().is_none());
    }
}
