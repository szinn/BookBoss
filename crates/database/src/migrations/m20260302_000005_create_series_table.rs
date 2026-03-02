use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Series::Table)
                    .if_not_exists()
                    .col(big_integer(Series::Id).primary_key())
                    .col(string(Series::Token).unique_key())
                    .col(string(Series::Name))
                    .col(text(Series::Description).null())
                    .col(big_integer(Series::Version))
                    .col(timestamp_with_time_zone(Series::CreatedAt))
                    .col(timestamp_with_time_zone(Series::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Series::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
    Token,
    Name,
    Description,
    Version,
    CreatedAt,
    UpdatedAt,
}
