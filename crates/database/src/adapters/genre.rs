use bb_core::{
    Error, RepositoryError,
    book::{Genre, GenreId, GenreRepository, GenreToken, NewGenre},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{genres, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<genres::Model> for Genre {
    fn from(model: genres::Model) -> Self {
        let token = GenreToken::new(model.id as u64);
        Self {
            id: model.id as u64,
            version: model.version as u64,
            token,
            name: model.name,
            created_at: model.created_at.with_timezone(&Utc),
            updated_at: model.updated_at.with_timezone(&Utc),
        }
    }
}

pub(crate) struct GenreRepositoryAdapter;

impl GenreRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl GenreRepository for GenreRepositoryAdapter {
    async fn add_genre(&self, transaction: &dyn Transaction, genre: NewGenre) -> Result<Genre, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = GenreToken::generate();
        let now = Utc::now();

        let model = genres::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(genre.name),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_genre(&self, transaction: &dyn Transaction, genre: Genre) -> Result<Genre, Error> {
        if genre.id == 0 {
            return Err(Error::InvalidId(genre.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Genres::find_by_id(genre.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != genre.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: genres::ActiveModel = existing.clone().into();

        if existing.name != genre.name {
            updater.name = Set(genre.name);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: GenreId) -> Result<Option<Genre>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Genres::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: &GenreToken) -> Result<Option<Genre>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Genre>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Genres::find()
            .filter(genres::Column::Name.eq(name))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_genres(&self, transaction: &dyn Transaction, start_id: Option<GenreId>, page_size: Option<u64>) -> Result<Vec<Genre>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Genres::find().order_by_asc(genres::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(genres::Column::Id.gte(start_id as i64));
        }

        let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
        query = query.limit(page_size);

        let rows = query.all(transaction).await.map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        Error, RepositoryError,
        book::{Genre, GenreRepository, GenreToken, NewGenre},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ─── add_genre ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_genre_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc
            .genre_repository()
            .add_genre(
                &*tx,
                NewGenre {
                    name: "Science Fiction".into(),
                },
            )
            .await;

        assert!(result.is_ok());
        let g = result.unwrap();
        assert_ne!(g.id, 0);
        assert_eq!(g.name, "Science Fiction");
        assert_eq!(g.token.id(), g.id);
    }

    #[tokio::test]
    async fn test_add_genre_duplicate_name_fails() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.genre_repository().add_genre(&*tx, NewGenre { name: "Fantasy".into() }).await.unwrap();
        let result = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Fantasy".into() }).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Horror".into() }).await.unwrap();
        let result = svc.genre_repository().find_by_id(&*tx, inserted.id).await;

        assert_eq!(result.unwrap().unwrap().name, "Horror");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.genre_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.genre_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Horror".into() }).await.unwrap();
        let result = svc.genre_repository().find_by_token(&*tx, &inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.genre_repository().find_by_token(&*tx, &GenreToken::new(999)).await.unwrap().is_none());
    }

    // ─── find_by_name ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_name_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Mystery".into() }).await.unwrap();
        let result = svc.genre_repository().find_by_name(&*tx, "Mystery").await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_name_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.genre_repository().find_by_name(&*tx, "Nonexistent").await.unwrap().is_none());
    }

    // ─── list_genres ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_genres_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.genre_repository().list_genres(&*tx, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_genres_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre A".into() }).await.unwrap();
        svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre B".into() }).await.unwrap();

        assert_eq!(svc.genre_repository().list_genres(&*tx, None, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_genres_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre A".into() }).await.unwrap();
        svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre B".into() }).await.unwrap();

        let all = svc.genre_repository().list_genres(&*tx, None, None).await.unwrap();
        let result = svc.genre_repository().list_genres(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_genres_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.genre_repository().list_genres(&*tx, None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── update_genre ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_genre_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut g = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Old Name".into() }).await.unwrap();
        g.name = "New Name".into();
        let updated = svc.genre_repository().update_genre(&*tx, g).await.unwrap();

        assert_eq!(updated.name, "New Name");
    }

    #[tokio::test]
    async fn test_update_genre_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut g = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre".into() }).await.unwrap();
        let version_before = g.version;
        g.name = "Updated".into();
        let updated = svc.genre_repository().update_genre(&*tx, g).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_genre_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let g = Genre {
            id: 999,
            version: 0,
            token: GenreToken::new(999),
            name: "Ghost".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(
            svc.genre_repository().update_genre(&*tx, g).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_genre_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut g = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Genre".into() }).await.unwrap();
        g.version = 99;
        g.name = "Updated".into();

        assert!(matches!(
            svc.genre_repository().update_genre(&*tx, g).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_genre_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let g = Genre {
            id: 0,
            version: 0,
            token: GenreToken::new(1),
            name: "Invalid".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(svc.genre_repository().update_genre(&*tx, g).await, Err(Error::InvalidId(0))));
    }
}
