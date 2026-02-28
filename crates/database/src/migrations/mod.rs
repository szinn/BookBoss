pub use sea_orm_migration::prelude::*;

mod m20260225_000001_create_users_table;
mod m20260225_000002_create_sessions_table;
mod m20260228_000003_create_user_settings_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260225_000001_create_users_table::Migration),
            Box::new(m20260225_000002_create_sessions_table::Migration),
            Box::new(m20260228_000003_create_user_settings_table::Migration),
        ]
    }
}
