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
mod m20260305_000022_create_jobs_table;
mod m20260321_000023_add_sort_indexes;
mod m20260322_000024_create_system_messages_table;
mod m20260322_000025_add_created_at_to_book_files;
mod m20260323_000026_drop_devices_preferred_format;
mod m20260330_000027_add_sidecar_fingerprint_to_books;
mod m20260330_000028_add_index_book_files_format_role;
mod m20260330_000029_add_index_jobs_indexes;
mod m20260330_000030_add_index_import_jobs_updated_at;
mod m20260330_000031_add_index_shelves_visibility;
mod m20260401_000032_replace_cover_path_with_has_cover;
mod m20260403_000033_create_libraries_table;
mod m20260403_000034_create_library_books_table;
mod m20260403_000035_create_user_libraries_table;
mod m20260403_000036_add_library_id_to_shelves;
mod m20260403_000037_seed_default_library_user_setting;
mod m20260403_000038_drop_shelf_visibility;
mod m20260403_000039_add_owner_id_to_libraries;
mod m20260404_000040_create_koreader_document_hashes_table;
mod m20260415_000041_create_app_settings_table;
mod m20260923_000042_add_position_source_to_user_book_metadata;

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
            Box::new(m20260305_000022_create_jobs_table::Migration),
            Box::new(m20260321_000023_add_sort_indexes::Migration),
            Box::new(m20260322_000024_create_system_messages_table::Migration),
            Box::new(m20260322_000025_add_created_at_to_book_files::Migration),
            Box::new(m20260323_000026_drop_devices_preferred_format::Migration),
            Box::new(m20260330_000027_add_sidecar_fingerprint_to_books::Migration),
            Box::new(m20260330_000028_add_index_book_files_format_role::Migration),
            Box::new(m20260330_000029_add_index_jobs_indexes::Migration),
            Box::new(m20260330_000030_add_index_import_jobs_updated_at::Migration),
            Box::new(m20260330_000031_add_index_shelves_visibility::Migration),
            Box::new(m20260401_000032_replace_cover_path_with_has_cover::Migration),
            Box::new(m20260403_000033_create_libraries_table::Migration),
            Box::new(m20260403_000034_create_library_books_table::Migration),
            Box::new(m20260403_000035_create_user_libraries_table::Migration),
            Box::new(m20260403_000036_add_library_id_to_shelves::Migration),
            Box::new(m20260403_000037_seed_default_library_user_setting::Migration),
            Box::new(m20260403_000038_drop_shelf_visibility::Migration),
            Box::new(m20260403_000039_add_owner_id_to_libraries::Migration),
            Box::new(m20260404_000040_create_koreader_document_hashes_table::Migration),
            Box::new(m20260415_000041_create_app_settings_table::Migration),
            Box::new(m20260923_000042_add_position_source_to_user_book_metadata::Migration),
        ]
    }
}
