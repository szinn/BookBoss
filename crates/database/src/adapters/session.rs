use bb_core::{
    Error, RepositoryError,
    auth::{NewSession, Session, SessionRepository},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QuerySelect, sea_query::OnConflict};

use crate::{
    entities::{prelude, sessions},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<sessions::Model> for Session {
    fn from(model: sessions::Model) -> Self {
        Self {
            id: model.id,
            session: model.session,
            expires_at: model.expires_at.with_timezone(&Utc),
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}

pub struct SessionRepositoryAdapter;

impl SessionRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SessionRepository for SessionRepositoryAdapter {
    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn count(&self, transaction: &dyn Transaction) -> Result<i64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let count = prelude::Sessions::find().count(transaction).await.map_err(handle_dberr)?;

        Ok(count as i64)
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn store(&self, transaction: &dyn Transaction, session: NewSession) -> Result<Session, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let model = sessions::ActiveModel {
            id: Set(session.id.clone()),
            session: Set(session.session),
            expires_at: Set(session.expires_at.into()),
            created_at: Set(Utc::now().into()),
        };

        let on_conflict = OnConflict::column(sessions::Column::Id)
            .update_columns([sessions::Column::Session, sessions::Column::ExpiresAt])
            .to_owned();

        prelude::Sessions::insert(model)
            .on_conflict(on_conflict)
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        // Reload to get the final state (handles both insert and update cases)
        let stored = prelude::Sessions::find_by_id(&session.id)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        Ok(stored.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn load(&self, transaction: &dyn Transaction, id: &str) -> Result<Option<Session>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Sessions::find_by_id(id).one(transaction).await.map_err(handle_dberr)?.map(Into::into))
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn delete_by_id(&self, transaction: &dyn Transaction, id: &str) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Sessions::find_by_id(id).one(transaction).await.map_err(handle_dberr)?;

        if let Some(existing) = existing {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn exists(&self, transaction: &dyn Transaction, id: &str) -> Result<bool, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let count = prelude::Sessions::find_by_id(id).count(transaction).await.map_err(handle_dberr)?;

        Ok(count > 0)
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn delete_by_expiry(&self, transaction: &dyn Transaction) -> Result<Vec<String>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;
        let now = Utc::now();

        // Fetch only the IDs of expired sessions
        let ids: Vec<String> = prelude::Sessions::find()
            .select_only()
            .column(sessions::Column::Id)
            .filter(sessions::Column::ExpiresAt.lt(now))
            .into_tuple()
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        // Bulk delete all expired sessions in a single query
        prelude::Sessions::delete_many()
            .filter(sessions::Column::ExpiresAt.lt(now))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(ids)
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn delete_all(&self, transaction: &dyn Transaction) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        prelude::Sessions::delete_many().exec(transaction).await.map_err(handle_dberr)?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn get_ids(&self, transaction: &dyn Transaction) -> Result<Vec<String>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let ids: Vec<String> = prelude::Sessions::find()
            .select_only()
            .column(sessions::Column::Id)
            .into_tuple()
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{auth::NewSession, repository::RepositoryService};
    use chrono::{Duration, Utc};
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ===================
    // Tests: store
    // ===================
    #[tokio::test]
    async fn test_store_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let new_session = NewSession::new("sess-1", "session-data", Utc::now() + Duration::hours(1)).unwrap();
        let result = svc.session_repository().store(&*tx, new_session).await;

        assert!(result.is_ok());
        let session = result.unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.session, "session-data");
    }

    #[tokio::test]
    async fn test_store_updates_existing() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let new_session = NewSession::new("sess-1", "original-data", Utc::now() + Duration::hours(1)).unwrap();
        svc.session_repository().store(&*tx, new_session).await.unwrap();

        let updated_session = NewSession::new("sess-1", "updated-data", Utc::now() + Duration::hours(2)).unwrap();
        let result = svc.session_repository().store(&*tx, updated_session).await;

        assert!(result.is_ok());
        let session = result.unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.session, "updated-data");

        // Only one record exists
        let count = svc.session_repository().count(&*tx).await.unwrap();
        assert_eq!(count, 1);
    }

    // ===================
    // Tests: count
    // ===================
    #[tokio::test]
    async fn test_count_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().count(&*tx).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_count_after_inserts() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "data-1", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();
        svc.session_repository()
            .store(&*tx, NewSession::new("sess-2", "data-2", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().count(&*tx).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    // ===================
    // Tests: load
    // ===================
    #[tokio::test]
    async fn test_load_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "session-data", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().load(&*tx, "sess-1").await;

        assert!(result.is_ok());
        let session = result.unwrap();
        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.session, "session-data");
    }

    #[tokio::test]
    async fn test_load_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().load(&*tx, "nonexistent").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ===================
    // Tests: exists
    // ===================
    #[tokio::test]
    async fn test_exists_true() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "data", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().exists(&*tx, "sess-1").await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_exists_false() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().exists(&*tx, "nonexistent").await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // ===================
    // Tests: delete_by_id
    // ===================
    #[tokio::test]
    async fn test_delete_by_id_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "data", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().delete_by_id(&*tx, "sess-1").await;
        assert!(result.is_ok());

        let loaded = svc.session_repository().load(&*tx, "sess-1").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_id_nonexistent_is_ok() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().delete_by_id(&*tx, "nonexistent").await;

        assert!(result.is_ok());
    }

    // ===================
    // Tests: delete_all
    // ===================
    #[tokio::test]
    async fn test_delete_all_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "data-1", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();
        svc.session_repository()
            .store(&*tx, NewSession::new("sess-2", "data-2", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().delete_all(&*tx).await;
        assert!(result.is_ok());

        let count = svc.session_repository().count(&*tx).await.unwrap();
        assert_eq!(count, 0);
    }

    // ===================
    // Tests: get_ids
    // ===================
    #[tokio::test]
    async fn test_get_ids_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().get_ids(&*tx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_ids_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("sess-1", "data-1", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();
        svc.session_repository()
            .store(&*tx, NewSession::new("sess-2", "data-2", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().get_ids(&*tx).await;

        assert!(result.is_ok());
        let mut ids = result.unwrap();
        ids.sort();
        assert_eq!(ids, vec!["sess-1", "sess-2"]);
    }

    // ===================
    // Tests: delete_by_expiry
    // ===================
    #[tokio::test]
    async fn test_delete_by_expiry_removes_expired() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        // Insert an expired session
        svc.session_repository()
            .store(&*tx, NewSession::new("expired", "data", Utc::now() - Duration::hours(1)).unwrap())
            .await
            .unwrap();
        // Insert a valid session
        svc.session_repository()
            .store(&*tx, NewSession::new("valid", "data", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().delete_by_expiry(&*tx).await;

        assert!(result.is_ok());
        let deleted_ids = result.unwrap();
        assert_eq!(deleted_ids, vec!["expired"]);

        // Valid session still exists
        assert!(svc.session_repository().exists(&*tx, "valid").await.unwrap());
        // Expired session is gone
        assert!(!svc.session_repository().exists(&*tx, "expired").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_by_expiry_empty_table() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.session_repository().delete_by_expiry(&*tx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_store_preserves_created_at_on_upsert() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let original = svc
            .session_repository()
            .store(&*tx, NewSession::new("sess-1", "original", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let updated = svc
            .session_repository()
            .store(&*tx, NewSession::new("sess-1", "updated", Utc::now() + Duration::hours(2)).unwrap())
            .await
            .unwrap();

        assert_eq!(updated.session, "updated");
        assert_eq!(
            updated.created_at.timestamp(),
            original.created_at.timestamp(),
            "created_at should not change on upsert"
        );
    }

    #[tokio::test]
    async fn test_delete_by_expiry_none_expired() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.session_repository()
            .store(&*tx, NewSession::new("valid", "data", Utc::now() + Duration::hours(1)).unwrap())
            .await
            .unwrap();

        let result = svc.session_repository().delete_by_expiry(&*tx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
