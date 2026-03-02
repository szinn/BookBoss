use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Genres::Table)
                    .if_not_exists()
                    .col(big_integer(Genres::Id).primary_key())
                    .col(string(Genres::Token).unique_key())
                    .col(string(Genres::Name).unique_key())
                    .col(big_integer(Genres::Version))
                    .col(timestamp_with_time_zone(Genres::CreatedAt))
                    .col(timestamp_with_time_zone(Genres::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Genres::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Genres {
    Table,
    Id,
    Token,
    Name,
    Version,
    CreatedAt,
    UpdatedAt,
}
