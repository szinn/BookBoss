use std::sync::Arc;

use bb_core::{
    Error,
    auth::SessionRepository,
    book::{AuthorRepository, GenreRepository, PublisherRepository, SeriesRepository, TagRepository},
    repository::{Repository, RepositoryService, RepositoryServiceBuilder},
    user::{UserRepository, UserSettingRepository},
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

use crate::{
    adapters::{
        author::AuthorRepositoryAdapter, genre::GenreRepositoryAdapter, publisher::PublisherRepositoryAdapter, series::SeriesRepositoryAdapter,
        session::SessionRepositoryAdapter, tag::TagRepositoryAdapter, user::UserRepositoryAdapter, user_settings::UserSettingRepositoryAdapter,
    },
    migrations::Migrator,
    repository::RepositoryImpl,
    transaction::*,
};

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
    let span = tracing::span!(tracing::Level::TRACE, "Migrations").entered();
    Migrator::up(&database, None).await.map_err(handle_dberr)?;
    span.exit();

    let repository_service = RepositoryServiceBuilder::default()
        .repository(Arc::new(RepositoryImpl::new(database)) as Arc<dyn Repository>)
        .session_repository(Arc::new(SessionRepositoryAdapter::new()) as Arc<dyn SessionRepository>)
        .user_repository(Arc::new(UserRepositoryAdapter::new()) as Arc<dyn UserRepository>)
        .user_setting_repository(Arc::new(UserSettingRepositoryAdapter::new()) as Arc<dyn UserSettingRepository>)
        .author_repository(Arc::new(AuthorRepositoryAdapter::new()) as Arc<dyn AuthorRepository>)
        .series_repository(Arc::new(SeriesRepositoryAdapter::new()) as Arc<dyn SeriesRepository>)
        .publisher_repository(Arc::new(PublisherRepositoryAdapter::new()) as Arc<dyn PublisherRepository>)
        .genre_repository(Arc::new(GenreRepositoryAdapter::new()) as Arc<dyn GenreRepository>)
        .tag_repository(Arc::new(TagRepositoryAdapter::new()) as Arc<dyn TagRepository>)
        .build()
        .map_err(|e| Error::Infrastructure(e.to_string()))?;

    Ok(Arc::new(repository_service))
}
