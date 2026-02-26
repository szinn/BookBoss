use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(string(Sessions::Id).primary_key())
                    .col(string(Sessions::Session))
                    .col(timestamp_with_time_zone(Sessions::ExpiresAt))
                    .col(timestamp_with_time_zone(Sessions::CreatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Sessions::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    Session,
    ExpiresAt,
    CreatedAt,
}
