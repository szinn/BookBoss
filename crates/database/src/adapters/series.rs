use bb_core::{
    Error, RepositoryError,
    book::{NewSeries, Series, SeriesId, SeriesRepository, SeriesToken},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{books, prelude, series},
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
    async fn add_series(&self, transaction: &dyn Transaction, series: NewSeries) -> Result<Series, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = SeriesToken::generate();
        let now = Utc::now();

        let model = series::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(series.name),
            description: Set(series.description),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_series(&self, transaction: &dyn Transaction, series: Series) -> Result<Series, Error> {
        if series.id == 0 {
            return Err(Error::InvalidId(series.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Series::find_by_id(series.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != series.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: series::ActiveModel = existing.clone().into();

        if existing.name != series.name {
            updater.name = Set(series.name);
        }
        if existing.description != series.description {
            updater.description = Set(series.description);
        }

        let result = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(result.into())
    }

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

    async fn find_by_token(&self, transaction: &dyn Transaction, token: SeriesToken) -> Result<Option<Series>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Series>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Series::find()
            .filter(super::lower_name_eq(series::Column::Name, name))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_all_series(&self, transaction: &dyn Transaction) -> Result<Vec<Series>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::Series::find()
            .order_by_asc(series::Column::Name)
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn max_series_number_for_series(&self, transaction: &dyn Transaction, series_id: SeriesId) -> Result<Option<rust_decimal::Decimal>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let result = prelude::Books::find()
            .filter(books::Column::SeriesId.eq(series_id as i64))
            .select_only()
            .column(books::Column::SeriesNumber)
            .into_tuple::<Option<rust_decimal::Decimal>>()
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        let max = result.into_iter().flatten().max();
        Ok(max)
    }

    async fn list_series(&self, transaction: &dyn Transaction, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error> {
        if let Some(page_size) = page_size
            && page_size < 1
        {
            return Err(Error::InvalidPageSize(page_size));
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Series::find().order_by_asc(series::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(series::Column::Id.gte(start_id as i64));
        }

        if let Some(page_size) = page_size {
            query = query.limit(page_size);
        }

        let rows = query.all(transaction).await.map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_books_for_series(&self, transaction: &dyn Transaction, series_id: SeriesId) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let count = prelude::Books::find()
            .filter(books::Column::SeriesId.eq(series_id as i64))
            .count(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(count)
    }

    async fn delete_series(&self, transaction: &dyn Transaction, series_id: SeriesId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        if let Some(existing) = prelude::Series::find_by_id(series_id as i64).one(transaction).await.map_err(handle_dberr)? {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    async fn find_by_ids(&self, transaction: &dyn Transaction, ids: &[SeriesId]) -> Result<Vec<Series>, Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;
        let ids: Vec<i64> = ids.iter().map(|&id| id as i64).collect();

        let rows = prelude::Series::find()
            .filter(series::Column::Id.is_in(ids))
            .order_by_asc(series::Column::Name)
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        Error, RepositoryError,
        book::{NewSeries, Series, SeriesToken},
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

        let result = svc.series_repository().find_by_token(&*tx, inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.series_repository().find_by_token(&*tx, SeriesToken::new(999)).await.unwrap().is_none());
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

    // ─── count_books_for_series ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_books_for_series_zero_when_no_books() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Empty Series".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(svc.series_repository().count_books_for_series(&*tx, s.id).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_count_books_for_series_counts_linked_books() {
        use bb_core::book::{BookStatus, NewBook};

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        // Create two books linked to the series.
        for title in &["Dune", "Dune Messiah"] {
            svc.book_repository()
                .add_book(
                    &*tx,
                    NewBook {
                        title: (*title).to_string(),
                        status: BookStatus::Available,
                        description: None,
                        published_date: None,
                        language: None,
                        series_id: Some(s.id),
                        series_number: None,
                        publisher_id: None,
                        page_count: None,
                        rating: None,
                        metadata_source: None,
                        has_cover: false,
                    },
                )
                .await
                .unwrap();
        }

        assert_eq!(svc.series_repository().count_books_for_series(&*tx, s.id).await.unwrap(), 2);
    }

    // ─── delete_series ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_series_removes_record() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Doomed Series".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        svc.series_repository().delete_series(&*tx, s.id).await.unwrap();

        assert!(svc.series_repository().find_by_id(&*tx, s.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_series_nonexistent_is_noop() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        // Should not error even if the series doesn't exist.
        svc.series_repository().delete_series(&*tx, 999).await.unwrap();
    }

    // ─── find_by_ids ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_series_find_by_ids_empty_input() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        let result = svc.series_repository().find_by_ids(&*tx, &[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_series_find_by_ids_returns_matching() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let s1 = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune".into(),
                    description: None,
                },
            )
            .await
            .unwrap();
        let s2 = svc
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
        let _s3 = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Culture".into(),
                    description: None,
                },
            )
            .await
            .unwrap();

        let result = svc.series_repository().find_by_ids(&*tx, &[s1.id, s2.id]).await.unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<&str> = result.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Dune"));
        assert!(names.contains(&"Foundation"));
    }

    #[tokio::test]
    async fn test_series_find_by_ids_unknown_ids_ignored() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        let result = svc.series_repository().find_by_ids(&*tx, &[999, 1000]).await.unwrap();
        assert!(result.is_empty());
    }
}
