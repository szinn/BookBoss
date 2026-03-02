use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_sync_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub device_id: i64,
    pub status: String,
    pub books_added: i32,
    pub books_removed: i32,
    pub started_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
