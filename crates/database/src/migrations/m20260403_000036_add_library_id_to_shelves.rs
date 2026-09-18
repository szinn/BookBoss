use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Shelves::Table)
                    .add_column(ColumnDef::new(Shelves::LibraryId).big_integer().not_null().default(1i64))
                    .to_owned(),
            )
            .await?;

        // SQLite does not support adding foreign keys to existing tables via
        // ALTER TABLE.
        if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_shelves_library_id")
                        .from(Shelves::Table, Shelves::LibraryId)
                        .to(Libraries::Table, Libraries::Id)
                        .on_delete(ForeignKeyAction::Restrict)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Shelves {
    Table,
    LibraryId,
}
#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
