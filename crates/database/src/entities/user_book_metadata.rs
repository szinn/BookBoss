use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_book_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub book_id: i64,
    pub read_status: String,
    pub progress_percentage: Option<i16>,
    pub position_token: Option<String>,
    pub last_progress_at: Option<DateTimeWithTimeZone>,
    pub personal_rating: Option<i16>,
    pub times_read: i32,
    pub date_started: Option<DateTimeWithTimeZone>,
    pub date_finished: Option<DateTimeWithTimeZone>,
    pub last_opened_at: Option<DateTimeWithTimeZone>,
    pub notes: Option<String>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
