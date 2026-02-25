use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(big_integer(Users::Id).primary_key())
                    .col(string(Users::Token).unique_key())
                    .col(string(Users::Username).unique_key())
                    .col(string(Users::PasswordHash))
                    .col(string(Users::EmailAddress).unique_key())
                    .col(string(Users::Capabilities))
                    .col(big_integer(Users::Version))
                    .col(timestamp_with_time_zone(Users::CreatedAt))
                    .col(timestamp_with_time_zone(Users::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Token,
    Username,
    PasswordHash,
    EmailAddress,
    Capabilities,
    Version,
    CreatedAt,
    UpdatedAt,
}
