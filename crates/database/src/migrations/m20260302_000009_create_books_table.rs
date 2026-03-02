use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Books::Table)
                    .if_not_exists()
                    .col(big_integer(Books::Id).primary_key())
                    .col(string(Books::Token).unique_key())
                    .col(string(Books::Title))
                    .col(text(Books::Description).null())
                    .col(integer(Books::PublishedDate).null())
                    .col(string(Books::Language).null())
                    .col(big_integer(Books::SeriesId).null())
                    .col(decimal(Books::SeriesNumber).null())
                    .col(big_integer(Books::PublisherId).null())
                    .col(integer(Books::PageCount).null())
                    .col(small_integer(Books::Rating).null())
                    .col(string(Books::Status))
                    .col(string(Books::MetadataSource).null())
                    .col(string(Books::CoverPath).null())
                    .col(big_integer(Books::Version))
                    .col(timestamp_with_time_zone(Books::CreatedAt))
                    .col(timestamp_with_time_zone(Books::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_books_series_id")
                            .from(Books::Table, Books::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_books_publisher_id")
                            .from(Books::Table, Books::PublisherId)
                            .to(Publishers::Table, Publishers::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(Index::create().name("idx_books_series_id").table(Books::Table).col(Books::SeriesId).to_owned())
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_books_publisher_id")
                    .table(Books::Table)
                    .col(Books::PublisherId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_books_publisher_id").to_owned()).await?;
        manager.drop_index(Index::drop().name("idx_books_series_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(Books::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
    Token,
    Title,
    Description,
    PublishedDate,
    Language,
    SeriesId,
    SeriesNumber,
    PublisherId,
    PageCount,
    Rating,
    Status,
    MetadataSource,
    CoverPath,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Publishers {
    Table,
    Id,
}
