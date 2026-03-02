use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "book_authors")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub book_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub author_id: i64,
    pub role: String,
    pub sort_order: i32,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}
