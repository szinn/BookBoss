use bb_core::user::UserToken;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, entity::prelude::*};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(unique)]
    pub token: String,
    #[sea_orm(unique)]
    pub username: String,
    pub password_hash: String,
    #[sea_orm(unique)]
    pub email_address: String,
    pub capabilities: String,
    pub version: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        let token = UserToken::generate();

        Self {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
            ..ActiveModelTrait::default()
        }
    }

    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if self.is_changed() {
            self.version = Set(self.version.unwrap() + 1);
            self.updated_at = Set(Utc::now().into());
        }

        Ok(self)
    }
}
