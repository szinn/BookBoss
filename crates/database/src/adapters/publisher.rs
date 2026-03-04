use bb_core::{
    Error, RepositoryError,
    book::{NewPublisher, Publisher, PublisherId, PublisherRepository, PublisherToken},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{prelude, publishers},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<publishers::Model> for Publisher {
    fn from(model: publishers::Model) -> Self {
        let token = PublisherToken::new(model.id as u64);
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

pub(crate) struct PublisherRepositoryAdapter;

impl PublisherRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl PublisherRepository for PublisherRepositoryAdapter {
    async fn add_publisher(&self, transaction: &dyn Transaction, publisher: NewPublisher) -> Result<Publisher, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = PublisherToken::generate();
        let now = Utc::now();

        let model = publishers::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(publisher.name),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_publisher(&self, transaction: &dyn Transaction, publisher: Publisher) -> Result<Publisher, Error> {
        if publisher.id == 0 {
            return Err(Error::InvalidId(publisher.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Publishers::find_by_id(publisher.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != publisher.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: publishers::ActiveModel = existing.clone().into();

        if existing.name != publisher.name {
            updater.name = Set(publisher.name);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: PublisherId) -> Result<Option<Publisher>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Publishers::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: &PublisherToken) -> Result<Option<Publisher>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn list_publishers(&self, transaction: &dyn Transaction, start_id: Option<PublisherId>, page_size: Option<u64>) -> Result<Vec<Publisher>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Publishers::find().order_by_asc(publishers::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(publishers::Column::Id.gte(start_id as i64));
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
        book::{NewPublisher, Publisher, PublisherRepository, PublisherToken},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ─── add_publisher ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_publisher_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.publisher_repository().add_publisher(&*tx, NewPublisher { name: "Tor Books".into() }).await;

        assert!(result.is_ok());
        let p = result.unwrap();
        assert_ne!(p.id, 0);
        assert_eq!(p.name, "Tor Books");
        assert_eq!(p.token.id(), p.id);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Tor Books".into() })
            .await
            .unwrap();
        let result = svc.publisher_repository().find_by_id(&*tx, inserted.id).await;

        assert_eq!(result.unwrap().unwrap().name, "Tor Books");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.publisher_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.publisher_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Tor Books".into() })
            .await
            .unwrap();
        let result = svc.publisher_repository().find_by_token(&*tx, &inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(
            svc.publisher_repository()
                .find_by_token(&*tx, &PublisherToken::new(999))
                .await
                .unwrap()
                .is_none()
        );
    }

    // ─── list_publishers ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_publishers_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.publisher_repository().list_publishers(&*tx, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_publishers_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher A".into() })
            .await
            .unwrap();
        svc.publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher B".into() })
            .await
            .unwrap();

        assert_eq!(svc.publisher_repository().list_publishers(&*tx, None, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_publishers_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher A".into() })
            .await
            .unwrap();
        svc.publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher B".into() })
            .await
            .unwrap();

        let all = svc.publisher_repository().list_publishers(&*tx, None, None).await.unwrap();
        let result = svc.publisher_repository().list_publishers(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_publishers_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.publisher_repository().list_publishers(&*tx, None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── update_publisher ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_publisher_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut p = svc
            .publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Old Name".into() })
            .await
            .unwrap();
        p.name = "New Name".into();
        let updated = svc.publisher_repository().update_publisher(&*tx, p).await.unwrap();

        assert_eq!(updated.name, "New Name");
    }

    #[tokio::test]
    async fn test_update_publisher_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut p = svc
            .publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher".into() })
            .await
            .unwrap();
        let version_before = p.version;
        p.name = "Updated".into();
        let updated = svc.publisher_repository().update_publisher(&*tx, p).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_publisher_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let p = Publisher {
            id: 999,
            version: 0,
            token: PublisherToken::new(999),
            name: "Ghost".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(
            svc.publisher_repository().update_publisher(&*tx, p).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_publisher_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut p = svc
            .publisher_repository()
            .add_publisher(&*tx, NewPublisher { name: "Publisher".into() })
            .await
            .unwrap();
        p.version = 99;
        p.name = "Updated".into();

        assert!(matches!(
            svc.publisher_repository().update_publisher(&*tx, p).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_publisher_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let p = Publisher {
            id: 0,
            version: 0,
            token: PublisherToken::new(1),
            name: "Invalid".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(svc.publisher_repository().update_publisher(&*tx, p).await, Err(Error::InvalidId(0))));
    }
}
