use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tags::Table)
                    .if_not_exists()
                    .col(big_integer(Tags::Id).primary_key())
                    .col(string(Tags::Token).unique_key())
                    .col(string(Tags::Name).unique_key())
                    .col(big_integer(Tags::Version))
                    .col(timestamp_with_time_zone(Tags::CreatedAt))
                    .col(timestamp_with_time_zone(Tags::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Tags::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Tags {
    Table,
    Id,
    Token,
    Name,
    Version,
    CreatedAt,
    UpdatedAt,
}
