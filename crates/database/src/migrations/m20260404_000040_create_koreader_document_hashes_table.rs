use sea_orm_migration::{
    prelude::*,
    schema::{big_integer, string, timestamp_with_time_zone},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KoreaderDocumentHashes::Table)
                    .if_not_exists()
                    .col(big_integer(KoreaderDocumentHashes::Id).auto_increment().primary_key())
                    .col(big_integer(KoreaderDocumentHashes::BookId).not_null())
                    .col(string(KoreaderDocumentHashes::DocumentHash).not_null())
                    .col(timestamp_with_time_zone(KoreaderDocumentHashes::CreatedAt).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_koreader_document_hashes_book_id")
                            .from(KoreaderDocumentHashes::Table, KoreaderDocumentHashes::BookId)
                            .to(Alias::new("books"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on document_hash for prefix queries
        manager
            .create_index(
                Index::create()
                    .name("idx_koreader_document_hashes_document_hash")
                    .table(KoreaderDocumentHashes::Table)
                    .col(KoreaderDocumentHashes::DocumentHash)
                    .to_owned(),
            )
            .await?;

        // Unique constraint on (document_hash, book_id) — duplicates are
        // silently ignored
        manager
            .create_index(
                Index::create()
                    .name("idx_koreader_document_hashes_unique")
                    .table(KoreaderDocumentHashes::Table)
                    .col(KoreaderDocumentHashes::DocumentHash)
                    .col(KoreaderDocumentHashes::BookId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum KoreaderDocumentHashes {
    Table,
    Id,
    BookId,
    DocumentHash,
    CreatedAt,
}
