use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(BookFiles::Table).drop_column(BookFiles::FilePath).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BookFiles::Table)
                    .add_column(ColumnDef::new(BookFiles::FilePath).string().null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BookFiles {
    Table,
    FilePath,
}
