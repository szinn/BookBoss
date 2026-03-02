use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookGenres::Table)
                    .if_not_exists()
                    .col(big_integer(BookGenres::BookId).not_null())
                    .col(big_integer(BookGenres::GenreId).not_null())
                    .primary_key(Index::create().col(BookGenres::BookId).col(BookGenres::GenreId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_genres_book_id")
                            .from(BookGenres::Table, BookGenres::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_genres_genre_id")
                            .from(BookGenres::Table, BookGenres::GenreId)
                            .to(Genres::Table, Genres::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_book_genres_genre_id")
                    .table(BookGenres::Table)
                    .col(BookGenres::GenreId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_book_genres_genre_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(BookGenres::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum BookGenres {
    Table,
    BookId,
    GenreId,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Genres {
    Table,
    Id,
}
