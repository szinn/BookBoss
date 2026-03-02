use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Publishers::Table)
                    .if_not_exists()
                    .col(big_integer(Publishers::Id).primary_key())
                    .col(string(Publishers::Token).unique_key())
                    .col(string(Publishers::Name))
                    .col(big_integer(Publishers::Version))
                    .col(timestamp_with_time_zone(Publishers::CreatedAt))
                    .col(timestamp_with_time_zone(Publishers::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Publishers::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Publishers {
    Table,
    Id,
    Token,
    Name,
    Version,
    CreatedAt,
    UpdatedAt,
}
