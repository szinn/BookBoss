use bb_core::{
    Error, RepositoryError,
    book::{NewSeries, Series, SeriesId, SeriesRepository, SeriesToken},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{prelude, series},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<series::Model> for Series {
    fn from(model: series::Model) -> Self {
        let token = SeriesToken::new(model.id as u64);
        Self {
            id: model.id as u64,
            version: model.version as u64,
            token,
            name: model.name,
            description: model.description,
            created_at: model.created_at.with_timezone(&Utc),
            updated_at: model.updated_at.with_timezone(&Utc),
        }
    }
}

pub(crate) struct SeriesRepositoryAdapter;

impl SeriesRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SeriesRepository for SeriesRepositoryAdapter {
    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn add_series(&self, transaction: &dyn Transaction, s: NewSeries) -> Result<Series, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = SeriesToken::generate();
        let now = Utc::now();

        let model = series::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(s.name),
            description: Set(s.description),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn update_series(&self, transaction: &dyn Transaction, s: Series) -> Result<Series, Error> {
        if s.id == 0 {
            return Err(Error::InvalidId(s.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Series::find_by_id(s.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != s.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: series::ActiveModel = existing.clone().into();

        if existing.name != s.name {
            updater.name = Set(s.name);
        }
        if existing.description != s.description {
            updater.description = Set(s.description);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_id(&self, transaction: &dyn Transaction, id: SeriesId) -> Result<Option<Series>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Series::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &SeriesToken) -> Result<Option<Series>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn list_series(&self, transaction: &dyn Transaction, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Series::find().order_by_asc(series::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(series::Column::Id.gte(start_id as i64));
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
        book::{NewSeries, Series, SeriesRepository, SeriesToken},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ─── add_series ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_series_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune".into(),
                    description: None,
                },
            )
            .await;

        assert!(result.is_ok());
        let s = result.unwrap();
        assert_ne!(s.id, 0);
        assert_eq!(s.name, "Dune");
        assert!(s.description.is_none());
        assert_eq!(s.token.id(), s.id);
    }

    #[tokio::test]
    async fn test_add_series_with_description() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let desc = Some("A science fiction saga".to_string());
        let s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune".into(),
                    description: desc.clone(),
                },
            )
            .await
            .unwrap();

        assert_eq!(s.description, desc);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Foundation".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        let result = svc.series_repository().find_by_id(&*tx, inserted.id).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().name, "Foundation");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.series_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.series_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Foundation".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        let result = svc.series_repository().find_by_token(&*tx, &inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.series_repository().find_by_token(&*tx, &SeriesToken::new(999)).await.unwrap().is_none());
    }

    // ─── list_series ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_series_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.series_repository().list_series(&*tx, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_series_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series A".into(),
                    description: None,
                },
            )
            .await
            .unwrap();
        svc.series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series B".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(svc.series_repository().list_series(&*tx, None, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_series_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series A".into(),
                    description: None,
                },
            )
            .await
            .unwrap();
        svc.series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series B".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        let all = svc.series_repository().list_series(&*tx, None, None).await.unwrap();
        let result = svc.series_repository().list_series(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_series_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.series_repository().list_series(&*tx, None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── update_series ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_series_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Old Name".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        s.name = "New Name".into();
        let updated = svc.series_repository().update_series(&*tx, s).await.unwrap();

        assert_eq!(updated.name, "New Name");
    }

    #[tokio::test]
    async fn test_update_series_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        let version_before = s.version;
        s.name = "Updated".into();
        let updated = svc.series_repository().update_series(&*tx, s).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_series_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s = Series {
            id: 999,
            version: 0,
            token: SeriesToken::new(999),
            name: "Ghost".into(),
            description: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(
            svc.series_repository().update_series(&*tx, s).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_series_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Series".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        s.version = 99;
        s.name = "Updated".into();

        assert!(matches!(
            svc.series_repository().update_series(&*tx, s).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_series_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s = Series {
            id: 0,
            version: 0,
            token: SeriesToken::new(1),
            name: "Invalid".into(),
            description: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(svc.series_repository().update_series(&*tx, s).await, Err(Error::InvalidId(0))));
    }
}
