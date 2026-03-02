use chrono::{DateTime, Utc};

use crate::{book::BookId, shelf::ShelfId};

#[derive(Debug, Clone)]
pub struct BookShelf {
    pub book_id: BookId,
    pub shelf_id: ShelfId,
    pub added_at: DateTime<Utc>,
    pub sort_order: i32,
}
