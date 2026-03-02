use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BookTags::Table)
                    .if_not_exists()
                    .col(big_integer(BookTags::BookId).not_null())
                    .col(big_integer(BookTags::TagId).not_null())
                    .primary_key(Index::create().col(BookTags::BookId).col(BookTags::TagId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_tags_book_id")
                            .from(BookTags::Table, BookTags::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_book_tags_tag_id")
                            .from(BookTags::Table, BookTags::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_book_tags_tag_id")
                    .table(BookTags::Table)
                    .col(BookTags::TagId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_book_tags_tag_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(BookTags::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum BookTags {
    Table,
    BookId,
    TagId,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tags {
    Table,
    Id,
}
