use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookIdentifiers::Table)
                    .if_not_exists()
                    .col(big_integer(BookIdentifiers::BookId).not_null())
                    .col(string(BookIdentifiers::IdentifierType).not_null())
                    .col(string(BookIdentifiers::Value))
                    .primary_key(Index::create().col(BookIdentifiers::BookId).col(BookIdentifiers::IdentifierType))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_identifiers_book_id")
                            .from(BookIdentifiers::Table, BookIdentifiers::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(BookIdentifiers::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum BookIdentifiers {
    Table,
    BookId,
    IdentifierType,
    Value,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
