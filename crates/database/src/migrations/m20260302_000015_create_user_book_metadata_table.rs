use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserBookMetadata::Table)
                    .if_not_exists()
                    .col(big_integer(UserBookMetadata::UserId).not_null())
                    .col(big_integer(UserBookMetadata::BookId).not_null())
                    .col(string(UserBookMetadata::ReadStatus))
                    .col(small_integer(UserBookMetadata::ProgressPercentage).null())
                    .col(string(UserBookMetadata::PositionToken).null())
                    .col(timestamp_with_time_zone(UserBookMetadata::LastProgressAt).null())
                    .col(small_integer(UserBookMetadata::PersonalRating).null())
                    .col(integer(UserBookMetadata::TimesRead))
                    .col(timestamp_with_time_zone(UserBookMetadata::DateStarted).null())
                    .col(timestamp_with_time_zone(UserBookMetadata::DateFinished).null())
                    .col(timestamp_with_time_zone(UserBookMetadata::LastOpenedAt).null())
                    .col(text(UserBookMetadata::Notes).null())
                    .primary_key(Index::create().col(UserBookMetadata::UserId).col(UserBookMetadata::BookId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_book_metadata_user_id")
                            .from(UserBookMetadata::Table, UserBookMetadata::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_book_metadata_book_id")
                            .from(UserBookMetadata::Table, UserBookMetadata::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_book_metadata_book_id")
                    .table(UserBookMetadata::Table)
                    .col(UserBookMetadata::BookId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_user_book_metadata_book_id").to_owned()).await?;
        manager.drop_table(Table::drop().table(UserBookMetadata::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum UserBookMetadata {
    Table,
    UserId,
    BookId,
    ReadStatus,
    ProgressPercentage,
    PositionToken,
    LastProgressAt,
    PersonalRating,
    TimesRead,
    DateStarted,
    DateFinished,
    LastOpenedAt,
    Notes,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
