pub use sea_orm_migration::prelude::*;

mod m20260225_000001_create_users_table;
mod m20260225_000002_create_sessions_table;
mod m20260228_000003_create_user_settings_table;
mod m20260302_000004_create_authors_table;
mod m20260302_000005_create_series_table;
mod m20260302_000006_create_publishers_table;
mod m20260302_000007_create_genres_table;
mod m20260302_000008_create_tags_table;
mod m20260302_000009_create_books_table;
mod m20260302_000010_create_book_authors_table;
mod m20260302_000011_create_book_genres_table;
mod m20260302_000012_create_book_tags_table;
mod m20260302_000013_create_book_identifiers_table;
mod m20260302_000014_create_book_files_table;
mod m20260302_000015_create_user_book_metadata_table;
mod m20260302_000016_create_devices_table;
mod m20260302_000017_create_device_books_table;
mod m20260302_000018_create_device_sync_log_table;
mod m20260303_000019_create_shelves_table;
mod m20260303_000020_create_book_shelves_table;
mod m20260303_000021_create_import_jobs_table;
mod m20260303_000022_drop_file_path_from_book_files;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260225_000001_create_users_table::Migration),
            Box::new(m20260225_000002_create_sessions_table::Migration),
            Box::new(m20260228_000003_create_user_settings_table::Migration),
            Box::new(m20260302_000004_create_authors_table::Migration),
            Box::new(m20260302_000005_create_series_table::Migration),
            Box::new(m20260302_000006_create_publishers_table::Migration),
            Box::new(m20260302_000007_create_genres_table::Migration),
            Box::new(m20260302_000008_create_tags_table::Migration),
            Box::new(m20260302_000009_create_books_table::Migration),
            Box::new(m20260302_000010_create_book_authors_table::Migration),
            Box::new(m20260302_000011_create_book_genres_table::Migration),
            Box::new(m20260302_000012_create_book_tags_table::Migration),
            Box::new(m20260302_000013_create_book_identifiers_table::Migration),
            Box::new(m20260302_000014_create_book_files_table::Migration),
            Box::new(m20260302_000015_create_user_book_metadata_table::Migration),
            Box::new(m20260302_000016_create_devices_table::Migration),
            Box::new(m20260302_000017_create_device_books_table::Migration),
            Box::new(m20260302_000018_create_device_sync_log_table::Migration),
            Box::new(m20260303_000019_create_shelves_table::Migration),
            Box::new(m20260303_000020_create_book_shelves_table::Migration),
            Box::new(m20260303_000021_create_import_jobs_table::Migration),
            Box::new(m20260303_000022_drop_file_path_from_book_files::Migration),
        ]
    }
}
