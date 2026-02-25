use std::sync::Arc;

use bb_core::{
    Error,
    repository::{Repository, RepositoryService, RepositoryServiceBuilder},
    user::UserRepository,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde::Deserialize;

pub mod error;

pub use error::*;

mod adapters;
mod entities;
mod migrations;
mod repository;
mod transaction;

use crate::{adapters::user::UserRepositoryAdapter, migrations::Migrator, repository::RepositoryImpl, transaction::*};

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    /// (required) Fully qualified URL for accessing the database.
    /// e.g. postgres://user:password@host/database
    pub database_url: String,
}

pub async fn open_database(config: &DatabaseConfig) -> Result<DatabaseConnection, Error> {
    tracing::debug!("Connecting to database...");
    let mut opt = ConnectOptions::new(&config.database_url);
    opt.max_connections(9)
        .min_connections(5)
        .sqlx_logging(true)
        .sqlx_logging_level(tracing::log::LevelFilter::Info);

    Ok(Database::connect(opt).await.map_err(handle_dberr)?)
}

#[tracing::instrument(level = "trace", skip(database))]
pub async fn create_repository_service(database: DatabaseConnection) -> Result<Arc<RepositoryService>, Error> {
    tracing::debug!("Running migrations...");
    Migrator::up(&database, None).await.map_err(handle_dberr)?;

    let repository_service = RepositoryServiceBuilder::default()
        .repository(Arc::new(RepositoryImpl::new(database)) as Arc<dyn Repository>)
        .user_repository(Arc::new(UserRepositoryAdapter::new()) as Arc<dyn UserRepository>)
        .build()
        .map_err(|e| Error::Infrastructure(e.to_string()))?;

    Ok(Arc::new(repository_service))
}
