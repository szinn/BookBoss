use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DeviceBooks::Table)
                    .if_not_exists()
                    .col(big_integer(DeviceBooks::DeviceId).not_null())
                    .col(big_integer(DeviceBooks::BookId).not_null())
                    .col(string(DeviceBooks::Format))
                    .col(timestamp_with_time_zone(DeviceBooks::SyncedAt))
                    .col(timestamp_with_time_zone(DeviceBooks::RemovedAt).null())
                    .primary_key(Index::create().col(DeviceBooks::DeviceId).col(DeviceBooks::BookId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_device_books_device_id")
                            .from(DeviceBooks::Table, DeviceBooks::DeviceId)
                            .to(Devices::Table, Devices::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_device_books_book_id")
                            .from(DeviceBooks::Table, DeviceBooks::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_device_books_book_id")
                    .table(DeviceBooks::Table)
                    .col(DeviceBooks::BookId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_device_books_book_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(DeviceBooks::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum DeviceBooks {
    Table,
    DeviceId,
    BookId,
    Format,
    SyncedAt,
    RemovedAt,
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
