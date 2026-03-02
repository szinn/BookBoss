use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookAuthors::Table)
                    .if_not_exists()
                    .col(big_integer(BookAuthors::BookId).not_null())
                    .col(big_integer(BookAuthors::AuthorId).not_null())
                    .col(string(BookAuthors::Role))
                    .col(integer(BookAuthors::SortOrder))
                    .primary_key(Index::create().col(BookAuthors::BookId).col(BookAuthors::AuthorId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_authors_book_id")
                            .from(BookAuthors::Table, BookAuthors::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_authors_author_id")
                            .from(BookAuthors::Table, BookAuthors::AuthorId)
                            .to(Authors::Table, Authors::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_book_authors_author_id")
                    .table(BookAuthors::Table)
                    .col(BookAuthors::AuthorId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_book_authors_author_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(BookAuthors::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum BookAuthors {
    Table,
    BookId,
    AuthorId,
    Role,
    SortOrder,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Authors {
    Table,
    Id,
}
