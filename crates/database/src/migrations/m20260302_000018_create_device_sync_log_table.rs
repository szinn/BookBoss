use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DeviceSyncLog::Table)
                    .if_not_exists()
                    .col(big_integer(DeviceSyncLog::Id).primary_key().auto_increment())
                    .col(big_integer(DeviceSyncLog::DeviceId))
                    .col(string(DeviceSyncLog::Status))
                    .col(integer(DeviceSyncLog::BooksAdded))
                    .col(integer(DeviceSyncLog::BooksRemoved))
                    .col(timestamp_with_time_zone(DeviceSyncLog::StartedAt))
                    .col(timestamp_with_time_zone(DeviceSyncLog::CompletedAt).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_device_sync_log_device_id")
                            .from(DeviceSyncLog::Table, DeviceSyncLog::DeviceId)
                            .to(Devices::Table, Devices::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_device_sync_log_device_id")
                    .table(DeviceSyncLog::Table)
                    .col(DeviceSyncLog::DeviceId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_device_sync_log_device_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(DeviceSyncLog::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum DeviceSyncLog {
    Table,
    Id,
    DeviceId,
    Status,
    BooksAdded,
    BooksRemoved,
    StartedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
}
