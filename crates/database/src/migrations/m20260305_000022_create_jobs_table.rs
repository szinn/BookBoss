use sea_orm_migration::{
    prelude::*,
    schema::{big_integer, integer, json_binary, small_integer, string, text, timestamp_with_time_zone},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Jobs::Table)
                    .if_not_exists()
                    .col(big_integer(Jobs::Id).primary_key().auto_increment())
                    .col(string(Jobs::JobType))
                    .col(json_binary(Jobs::Payload))
                    .col(string(Jobs::Status))
                    .col(small_integer(Jobs::Priority))
                    .col(small_integer(Jobs::Attempt))
                    .col(small_integer(Jobs::MaxAttempts))
                    .col(integer(Jobs::Version))
                    .col(timestamp_with_time_zone(Jobs::ScheduledAt))
                    .col(timestamp_with_time_zone(Jobs::StartedAt).null())
                    .col(timestamp_with_time_zone(Jobs::CompletedAt).null())
                    .col(text(Jobs::ErrorMessage).null())
                    .col(timestamp_with_time_zone(Jobs::CreatedAt))
                    .col(timestamp_with_time_zone(Jobs::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Partial index for the claim query.
        // MySQL does not support partial indexes, so we create a regular
        // covering index there.
        let index = match manager.get_database_backend() {
            sea_orm::DatabaseBackend::MySql => Index::create()
                .name("jobs_claim")
                .table(Jobs::Table)
                .col((Jobs::Status, IndexOrder::Asc))
                .col((Jobs::Priority, IndexOrder::Desc))
                .col((Jobs::ScheduledAt, IndexOrder::Asc))
                .to_owned(),
            _ => Index::create()
                .name("jobs_claim")
                .table(Jobs::Table)
                .col((Jobs::Priority, IndexOrder::Desc))
                .col((Jobs::ScheduledAt, IndexOrder::Asc))
                .and_where(Expr::col(Jobs::Status).eq("pending"))
                .to_owned(),
        };
        manager.create_index(index).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Jobs {
    Table,
    Id,
    JobType,
    Payload,
    Status,
    Priority,
    Attempt,
    MaxAttempts,
    Version,
    ScheduledAt,
    StartedAt,
    CompletedAt,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
}
