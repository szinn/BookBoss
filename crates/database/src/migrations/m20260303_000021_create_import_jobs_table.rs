use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ImportJobs::Table)
                    .if_not_exists()
                    .col(big_integer(ImportJobs::Id).primary_key())
                    .col(string(ImportJobs::Token).unique_key())
                    .col(string(ImportJobs::FilePath))
                    .col(string(ImportJobs::FileHash))
                    .col(string(ImportJobs::FileFormat))
                    .col(timestamp_with_time_zone(ImportJobs::DetectedAt))
                    .col(string(ImportJobs::Status))
                    .col(big_integer(ImportJobs::CandidateBookId).null())
                    .col(string(ImportJobs::MetadataSource).null())
                    .col(text(ImportJobs::ErrorMessage).null())
                    .col(big_integer(ImportJobs::ReviewedBy).null())
                    .col(timestamp_with_time_zone(ImportJobs::ReviewedAt).null())
                    .col(big_integer(ImportJobs::Version))
                    .col(timestamp_with_time_zone(ImportJobs::CreatedAt))
                    .col(timestamp_with_time_zone(ImportJobs::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_import_jobs_candidate_book_id")
                            .from(ImportJobs::Table, ImportJobs::CandidateBookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_import_jobs_reviewed_by")
                            .from(ImportJobs::Table, ImportJobs::ReviewedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_import_jobs_status")
                    .table(ImportJobs::Table)
                    .col(ImportJobs::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_import_jobs_file_hash")
                    .table(ImportJobs::Table)
                    .col(ImportJobs::FileHash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_import_jobs_file_hash").to_owned()).await?;
        manager.drop_index(Index::drop().name("idx_import_jobs_status").to_owned()).await?;
        manager.drop_table(Table::drop().table(ImportJobs::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum ImportJobs {
    Table,
    Id,
    Token,
    FilePath,
    FileHash,
    FileFormat,
    DetectedAt,
    Status,
    CandidateBookId,
    MetadataSource,
    ErrorMessage,
    ReviewedBy,
    ReviewedAt,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
