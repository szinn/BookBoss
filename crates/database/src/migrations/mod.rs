pub use sea_orm_migration::prelude::*;

mod m20260225_000001_create_users_table;
mod m20260225_000002_create_sessions_table;
mod m20260228_000003_create_user_settings_table;
mod m20260302_000004_create_authors_table;
mod m20260302_000005_create_series_table;
mod m20260302_000006_create_publishers_table;
mod m20260302_000007_create_genres_table;
mod m20260302_000008_create_tags_table;

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
        ]
    }
}
