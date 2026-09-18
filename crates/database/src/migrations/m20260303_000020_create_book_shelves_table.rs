use sea_orm_migration::{
    prelude::*,
    schema::{big_integer, integer, timestamp_with_time_zone},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookShelves::Table)
                    .if_not_exists()
                    .col(big_integer(BookShelves::BookId).not_null())
                    .col(big_integer(BookShelves::ShelfId).not_null())
                    .col(timestamp_with_time_zone(BookShelves::AddedAt))
                    .col(integer(BookShelves::SortOrder))
                    .primary_key(Index::create().col(BookShelves::BookId).col(BookShelves::ShelfId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_shelves_book_id")
                            .from(BookShelves::Table, BookShelves::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_shelves_shelf_id")
                            .from(BookShelves::Table, BookShelves::ShelfId)
                            .to(Shelves::Table, Shelves::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Composite index covers both "all books in shelf X" and paginated
        // "books in shelf X after book_id Y" queries, which are the
        // dominant access patterns for this table.
        manager
            .create_index(
                Index::create()
                    .name("idx_book_shelves_shelf_book")
                    .table(BookShelves::Table)
                    .col(BookShelves::ShelfId)
                    .col(BookShelves::BookId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum BookShelves {
    Table,
    BookId,
    ShelfId,
    AddedAt,
    SortOrder,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Shelves {
    Table,
    Id,
}
