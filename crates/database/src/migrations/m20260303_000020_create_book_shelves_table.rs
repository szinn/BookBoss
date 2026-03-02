use sea_orm_migration::{prelude::*, schema::*};

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

        manager
            .create_index(
                Index::create()
                    .name("idx_book_shelves_shelf_id")
                    .table(BookShelves::Table)
                    .col(BookShelves::ShelfId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_book_shelves_shelf_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(BookShelves::Table).to_owned()).await
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
