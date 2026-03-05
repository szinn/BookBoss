use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, entity::prelude::*};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    pub job_type: String,
    pub payload: Json,
    pub status: String,
    pub priority: i16,
    pub attempt: i16,
    pub max_attempts: i16,
    pub version: i32,
    pub scheduled_at: DateTimeWithTimeZone,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        let now = Utc::now();
        Self {
            status: Set("pending".to_string()),
            priority: Set(0),
            attempt: Set(0),
            max_attempts: Set(3),
            version: Set(0),
            scheduled_at: Set(now.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
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
