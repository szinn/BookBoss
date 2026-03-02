use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Shelves::Table)
                    .if_not_exists()
                    .col(big_integer(Shelves::Id).primary_key())
                    .col(string(Shelves::Token).unique_key())
                    .col(big_integer(Shelves::OwnerId).not_null())
                    .col(string(Shelves::Name))
                    .col(string(Shelves::ShelfType))
                    .col(string(Shelves::Visibility))
                    .col(big_integer(Shelves::DeviceId).null())
                    .col(json_binary(Shelves::FilterCriteria).null())
                    .col(big_integer(Shelves::Version))
                    .col(timestamp_with_time_zone(Shelves::CreatedAt))
                    .col(timestamp_with_time_zone(Shelves::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shelves_owner_id")
                            .from(Shelves::Table, Shelves::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shelves_device_id")
                            .from(Shelves::Table, Shelves::DeviceId)
                            .to(Devices::Table, Devices::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shelves_owner_id")
                    .table(Shelves::Table)
                    .col(Shelves::OwnerId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shelves_device_id")
                    .table(Shelves::Table)
                    .col(Shelves::DeviceId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_shelves_device_id").to_owned()).await?;
        manager.drop_index(Index::drop().name("idx_shelves_owner_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(Shelves::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Shelves {
    Table,
    Id,
    Token,
    OwnerId,
    Name,
    ShelfType,
    Visibility,
    DeviceId,
    FilterCriteria,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
}
