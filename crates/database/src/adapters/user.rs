use bb_core::{
    Error, RepositoryError,
    repository::Transaction,
    types::EmailAddress,
    user::{NewUser, User, UserId, UserRepository, UserToken},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{prelude, users},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<users::Model> for User {
    fn from(model: users::Model) -> Self {
        let token = UserToken::new(model.id as u64);
        let email_address = EmailAddress::new(model.email_address).expect("database email should be valid");
        let capabilities = serde_json::from_str(&model.capabilities).unwrap_or_default();

        Self {
            id: model.id as u64,
            version: model.version as u64,
            token,
            username: model.username,
            password_hash: model.password_hash,
            email_address,
            capabilities,
            created_at: model.created_at.with_timezone(&Utc),
            updated_at: model.updated_at.with_timezone(&Utc),
        }
    }
}

pub(crate) struct UserRepositoryAdapter;

impl UserRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl UserRepository for UserRepositoryAdapter {
    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn add_user(&self, transaction: &dyn Transaction, user: NewUser) -> Result<User, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = UserToken::generate();
        let now = Utc::now();
        let capabilities = serde_json::to_string(&user.capabilities).map_err(|e| Error::Infrastructure(e.to_string()))?;

        let model = users::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            username: Set(user.username),
            password_hash: Set(user.password_hash),
            email_address: Set(user.email_address.into_inner()),
            capabilities: Set(capabilities),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn update_user(&self, transaction: &dyn Transaction, user: User) -> Result<User, Error> {
        if user.id == 0 {
            return Err(Error::InvalidId(user.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Users::find_by_id(user.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != user.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: users::ActiveModel = existing.clone().into();

        if existing.username != user.username {
            updater.username = Set(user.username);
        }
        if existing.password_hash != user.password_hash {
            updater.password_hash = Set(user.password_hash);
        }
        if existing.email_address != user.email_address.as_str() {
            updater.email_address = Set(user.email_address.into_inner());
        }
        let new_caps = serde_json::to_string(&user.capabilities).map_err(|e| Error::Infrastructure(e.to_string()))?;
        if existing.capabilities != new_caps {
            updater.capabilities = Set(new_caps);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn delete_user(&self, transaction: &dyn Transaction, user: User) -> Result<User, Error> {
        if user.id == 0 {
            return Err(Error::InvalidId(user.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Users::find_by_id(user.id as i64).one(transaction).await.map_err(handle_dberr)?;

        let Some(existing) = existing else {
            return Err(Error::RepositoryError(RepositoryError::NotFound));
        };

        if existing.version != user.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let user: User = existing.clone().into();
        existing.delete(transaction).await.map_err(handle_dberr)?;

        Ok(user)
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn list_users(&self, transaction: &dyn Transaction, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Users::find().order_by_asc(users::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(users::Column::Id.gte(start_id as i64));
        }

        let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
        query = query.limit(page_size);

        let users = query.all(transaction).await.map_err(handle_dberr)?;

        Ok(users.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_id(&self, transaction: &dyn Transaction, id: UserId) -> Result<Option<User>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Users::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_username(&self, transaction: &dyn Transaction, username: &str) -> Result<Option<User>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Users::find()
            .filter(users::Column::Username.eq(username))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use bb_core::{
        Error, RepositoryError,
        repository::RepositoryService,
        types::Capability,
        user::{NewUser, User},
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();

        create_repository_service(db).await.unwrap()
    }

    // ─── add_user ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_user_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let new_user = NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap();
        let result = svc.user_repository().add_user(&*tx, new_user).await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_ne!(user.id, 0);
        assert_eq!(user.username, "alice");
        assert_eq!(user.email_address.as_str(), "alice@example.com");
        assert!(user.capabilities.is_empty());
    }

    #[tokio::test]
    async fn test_add_user_duplicate_username_fails() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let result = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash2", "alice2@example.com", HashSet::new()).unwrap())
            .await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    #[tokio::test]
    async fn test_add_user_duplicate_email_fails() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "shared@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let result = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("bob", "hash2", "shared@example.com", HashSet::new()).unwrap())
            .await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    #[tokio::test]
    async fn test_add_user_capabilities_round_trip() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let caps = HashSet::from([Capability::Admin, Capability::EditBook]);
        let user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", caps.clone()).unwrap())
            .await
            .unwrap();

        let found = svc.user_repository().find_by_id(&*tx, user.id).await.unwrap().unwrap();
        assert_eq!(found.capabilities, caps);
    }

    #[tokio::test]
    async fn test_add_user_token_id_consistency() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        assert_eq!(user.token.id(), user.id);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let result = svc.user_repository().find_by_id(&*tx, inserted.id).await;

        assert!(result.is_ok());
        let user = result.unwrap().unwrap();
        assert_eq!(user.id, inserted.id);
        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_repository().find_by_id(&*tx, 999).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_repository().find_by_id(&*tx, 0).await;

        assert!(matches!(result, Err(Error::InvalidId(0))));
    }

    // ─── find_by_username ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_username_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let result = svc.user_repository().find_by_username(&*tx, "alice").await;

        assert!(result.is_ok());
        let user = result.unwrap().unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.email_address.as_str(), "alice@example.com");
    }

    #[tokio::test]
    async fn test_find_by_username_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_repository().find_by_username(&*tx, "nobody").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── list_users ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_users_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_repository()
            .add_user(&*tx, NewUser::new("alice", "h1", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();
        svc.user_repository()
            .add_user(&*tx, NewUser::new("bob", "h2", "bob@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let result = svc.user_repository().list_users(&*tx, None, None).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_users_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_repository().list_users(&*tx, None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_users_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_repository().list_users(&*tx, None, Some(0)).await;

        assert!(matches!(result, Err(Error::InvalidPageSize(0))));
    }

    #[tokio::test]
    async fn test_list_users_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_repository()
            .add_user(&*tx, NewUser::new("alice", "h1", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();
        svc.user_repository()
            .add_user(&*tx, NewUser::new("bob", "h2", "bob@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        // IDs are random; get the sorted list first, then use the second id as
        // start_id.
        let all = svc.user_repository().list_users(&*tx, None, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let result = svc.user_repository().list_users(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    // ─── update_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_user_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        user.username = "alice-updated".to_string();
        let result = svc.user_repository().update_user(&*tx, user).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().username, "alice-updated");
    }

    #[tokio::test]
    async fn test_update_user_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        let version_before = user.version;
        user.username = "alice-updated".to_string();
        let updated = svc.user_repository().update_user(&*tx, user).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_user_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = User::fake(999, "ghost", "hash", "ghost@example.com", HashSet::new());
        let result = svc.user_repository().update_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_update_user_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        user.version = 99;
        user.username = "alice-updated".to_string();
        let result = svc.user_repository().update_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_update_user_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = User::fake(0, "invalid", "hash", "invalid@example.com", HashSet::new());
        let result = svc.user_repository().update_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::InvalidId(0))));
    }

    // ─── delete_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_user_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();
        let id = user.id;

        let result = svc.user_repository().delete_user(&*tx, user).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, id);
        assert!(svc.user_repository().find_by_id(&*tx, id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = User::fake(999, "ghost", "hash", "ghost@example.com", HashSet::new());
        let result = svc.user_repository().delete_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_delete_user_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();

        user.version = 99;
        let result = svc.user_repository().delete_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_delete_user_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = User::fake(0, "invalid", "hash", "invalid@example.com", HashSet::new());
        let result = svc.user_repository().delete_user(&*tx, user).await;

        assert!(matches!(result, Err(Error::InvalidId(0))));
    }
}
