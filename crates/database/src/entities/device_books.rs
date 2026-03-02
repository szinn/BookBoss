use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_books")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub device_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub book_id: i64,
    pub format: String,
    pub synced_at: DateTimeWithTimeZone,
    pub removed_at: Option<DateTimeWithTimeZone>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
