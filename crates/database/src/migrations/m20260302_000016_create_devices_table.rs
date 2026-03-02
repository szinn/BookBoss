use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Devices::Table)
                    .if_not_exists()
                    .col(big_integer(Devices::Id).primary_key())
                    .col(string(Devices::Token).unique_key())
                    .col(big_integer(Devices::OwnerId))
                    .col(string(Devices::Name))
                    .col(string(Devices::DeviceType))
                    .col(string(Devices::PreferredFormat).null())
                    .col(string(Devices::OnRemovalAction))
                    .col(timestamp_with_time_zone(Devices::LastSyncedAt).null())
                    .col(big_integer(Devices::Version))
                    .col(timestamp_with_time_zone(Devices::CreatedAt))
                    .col(timestamp_with_time_zone(Devices::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_devices_owner_id")
                            .from(Devices::Table, Devices::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_devices_owner_id")
                    .table(Devices::Table)
                    .col(Devices::OwnerId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_devices_owner_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(Devices::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
    Token,
    OwnerId,
    Name,
    DeviceType,
    PreferredFormat,
    OnRemovalAction,
    LastSyncedAt,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
