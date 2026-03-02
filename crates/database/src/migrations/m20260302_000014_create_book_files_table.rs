use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookFiles::Table)
                    .if_not_exists()
                    .col(big_integer(BookFiles::BookId).not_null())
                    .col(string(BookFiles::Format).not_null())
                    .col(string(BookFiles::FilePath))
                    .col(big_integer(BookFiles::FileSize))
                    .col(string(BookFiles::FileHash))
                    .primary_key(Index::create().col(BookFiles::BookId).col(BookFiles::Format))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_files_book_id")
                            .from(BookFiles::Table, BookFiles::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(BookFiles::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum BookFiles {
    Table,
    BookId,
    Format,
    FilePath,
    FileSize,
    FileHash,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
