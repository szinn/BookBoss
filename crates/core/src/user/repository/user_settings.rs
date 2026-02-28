use crate::{
    Error,
    repository::Transaction,
    user::{NewUserSetting, UserId, UserSetting},
};

#[async_trait::async_trait]
pub trait UserSettingRepository: Send + Sync {
    async fn get(&self, tx: &dyn Transaction, user_id: UserId, key: &str) -> Result<Option<UserSetting>, Error>;
    async fn set(&self, tx: &dyn Transaction, setting: NewUserSetting) -> Result<UserSetting, Error>;
    async fn delete(&self, tx: &dyn Transaction, user_id: UserId, key: &str) -> Result<(), Error>;
    async fn list_by_user(&self, tx: &dyn Transaction, user_id: UserId) -> Result<Vec<UserSetting>, Error>;
}
