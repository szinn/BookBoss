use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite only supports one column per ALTER TABLE statement.
        manager
            .alter_table(
                Table::alter()
                    .table(UserBookMetadata::Table)
                    .add_column(ColumnDef::new(UserBookMetadata::PositionSource).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserBookMetadata::Table)
                    .add_column(ColumnDef::new(UserBookMetadata::ContentSourceProgressPercentage).small_integer().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserBookMetadata {
    Table,
    PositionSource,
    ContentSourceProgressPercentage,
}
