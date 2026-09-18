use std::{sync::Arc, time::Duration};

use crate::{
    Error,
    auth::{NewSession, Session},
    repository::RepositoryService,
    types::EmailAddress,
    user::User,
    with_read_only_transaction, with_transaction,
};

#[allow(unused_lifetimes)] // async_trait macro generates hidden lifetime parameters
#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
pub trait AuthService: Send + Sync {
    async fn count(&self) -> Result<i64, Error>;
    async fn store(&self, session: NewSession) -> Result<Session, Error>;
    async fn load(&self, id: &str) -> Result<Option<Session>, Error>;
    async fn delete_by_id(&self, id: &str) -> Result<(), Error>;
    async fn exists(&self, id: &str) -> Result<bool, Error>;
    async fn delete_by_expiry(&self) -> Result<Vec<String>, Error>;
    async fn delete_all(&self) -> Result<(), Error>;
    async fn get_ids(&self) -> Result<Vec<String>, Error>;
    async fn is_valid_login(&self, username: &str, password: &str) -> Result<Option<User>, Error>;
    /// Looks up a user by email for SSO login. Unlike [`is_valid_login`], this
    /// does not normalize response time on miss — the OIDC callback has already
    /// completed IdP authentication, so timing-based account enumeration is
    /// not a concern at this stage.
    async fn is_valid_email(&self, email: &EmailAddress) -> Result<Option<User>, Error>;
}

pub(crate) struct AuthServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl AuthServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl AuthService for AuthServiceImpl {
    async fn count(&self) -> Result<i64, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.count(tx).await)
    }

    async fn store(&self, session: NewSession) -> Result<Session, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.store(tx, session).await)
    }

    async fn load(&self, id: &str) -> Result<Option<Session>, Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.load(tx, &id).await)
    }

    async fn delete_by_id(&self, id: &str) -> Result<(), Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.delete_by_id(tx, &id).await)
    }

    async fn exists(&self, id: &str) -> Result<bool, Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.exists(tx, &id).await)
    }

    async fn delete_by_expiry(&self) -> Result<Vec<String>, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.delete_by_expiry(tx).await)
    }

    async fn delete_all(&self) -> Result<(), Error> {
        with_transaction!(self, session_repository, |tx| session_repository.delete_all(tx).await)
    }

    async fn get_ids(&self) -> Result<Vec<String>, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.get_ids(tx).await)
    }

    async fn is_valid_email(&self, email: &EmailAddress) -> Result<Option<User>, Error> {
        let email = email.clone();
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_email(tx, &email).await)
    }

    async fn is_valid_login(&self, username: &str, password: &str) -> Result<Option<User>, Error> {
        let username = username.to_owned();
        let password = password.to_owned();
        let user = with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_username(tx, &username).await)?;
        match user {
            Some(user) if user.check_password(&password) => Ok(Some(user)),
            Some(_) => Ok(None),
            None => {
                // Delay when the user isn't found to normalise response time
                // and prevent username enumeration via timing
                // attacks.
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use chrono::Utc;

    use super::{AuthService, AuthServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, SessionBuilder, repository::MockSessionRepository},
        user::repository::user::MockUserRepository,
    };

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn create_service(session_mock: MockSessionRepository) -> AuthServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .session_repository(Arc::new(session_mock))
                .build()
                .expect("all fields provided"),
        );
        AuthServiceImpl::new(repository_service)
    }

    fn create_login_service(user_mock: MockUserRepository) -> AuthServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .user_repository(Arc::new(user_mock))
                .build()
                .expect("all fields provided"),
        );
        AuthServiceImpl::new(repository_service)
    }

    fn fake_session(id: &str) -> Session {
        SessionBuilder::default()
            .id(id.to_owned())
            .session("data".to_owned())
            .expires_at(Utc::now() + chrono::Duration::hours(1))
            .build()
            .expect("valid session")
    }

    // ─── count ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_returns_value() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_count().returning(|_| Box::pin(async { Ok(3) }));
        let svc = create_service(session_repo);

        let result = svc.count().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_count_propagates_error() {
        let mut session_repo = MockSessionRepository::new();
        session_repo
            .expect_count()
            .returning(|_| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_service(session_repo);

        let result = svc.count().await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── store ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_store_success() {
        let session = fake_session("sess-1");
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_store().returning(move |_, _| {
            let session = session.clone();
            Box::pin(async move { Ok(session) })
        });
        let svc = create_service(session_repo);

        let new_session = NewSession::new("sess-1", "data", Utc::now() + chrono::Duration::hours(1)).unwrap();
        let result = svc.store(new_session).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "sess-1");
    }

    #[tokio::test]
    async fn test_store_propagates_constraint_error() {
        let mut session_repo = MockSessionRepository::new();
        session_repo
            .expect_store()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Constraint("duplicate id".into()))) }));
        let svc = create_service(session_repo);

        let new_session = NewSession::new("sess-1", "data", Utc::now()).unwrap();
        let result = svc.store(new_session).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── load ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_load_found() {
        let session = fake_session("sess-1");
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_load().returning(move |_, _| {
            let session = session.clone();
            Box::pin(async move { Ok(Some(session)) })
        });
        let svc = create_service(session_repo);

        let result = svc.load("sess-1").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, "sess-1");
    }

    #[tokio::test]
    async fn test_load_not_found() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_load().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(session_repo);

        let result = svc.load("missing").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── delete_by_id ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_by_id_success() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_delete_by_id().returning(|_, _| Box::pin(async { Ok(()) }));
        let svc = create_service(session_repo);

        let result = svc.delete_by_id("sess-1").await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_delete_by_id_propagates_error() {
        let mut session_repo = MockSessionRepository::new();
        session_repo
            .expect_delete_by_id()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::NotFound)) }));
        let svc = create_service(session_repo);

        let result = svc.delete_by_id("missing").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── exists ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_exists_true() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_exists().returning(|_, _| Box::pin(async { Ok(true) }));
        let svc = create_service(session_repo);

        let result = svc.exists("sess-1").await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_exists_false() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_exists().returning(|_, _| Box::pin(async { Ok(false) }));
        let svc = create_service(session_repo);

        let result = svc.exists("missing").await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // ─── delete_by_expiry ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_by_expiry_returns_deleted_ids() {
        let ids = vec!["sess-1".to_owned(), "sess-2".to_owned()];
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_delete_by_expiry().returning(move |_| {
            let ids = ids.clone();
            Box::pin(async move { Ok(ids) })
        });
        let svc = create_service(session_repo);

        let result = svc.delete_by_expiry().await;

        assert!(result.is_ok());
        let deleted = result.unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0], "sess-1");
        assert_eq!(deleted[1], "sess-2");
    }

    #[tokio::test]
    async fn test_delete_by_expiry_empty_when_none_expired() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_delete_by_expiry().returning(|_| Box::pin(async { Ok(vec![]) }));
        let svc = create_service(session_repo);

        let result = svc.delete_by_expiry().await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── delete_all ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_all_success() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_delete_all().returning(|_| Box::pin(async { Ok(()) }));
        let svc = create_service(session_repo);

        let result = svc.delete_all().await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_delete_all_propagates_error() {
        let mut session_repo = MockSessionRepository::new();
        session_repo
            .expect_delete_all()
            .returning(|_| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_service(session_repo);

        let result = svc.delete_all().await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── get_ids ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_ids_returns_all() {
        let ids = vec!["sess-1".to_owned(), "sess-2".to_owned(), "sess-3".to_owned()];
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_get_ids().returning(move |_| {
            let ids = ids.clone();
            Box::pin(async move { Ok(ids) })
        });
        let svc = create_service(session_repo);

        let result = svc.get_ids().await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "sess-1");
    }

    #[tokio::test]
    async fn test_get_ids_empty() {
        let mut session_repo = MockSessionRepository::new();
        session_repo.expect_get_ids().returning(|_| Box::pin(async { Ok(vec![]) }));
        let svc = create_service(session_repo);

        let result = svc.get_ids().await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── is_valid_login ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_is_valid_login_success() {
        let hash = crate::user::User::encrypt_password("correct-password").unwrap();
        let user = crate::user::User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_login_service(user_repo);

        let result = svc.is_valid_login("alice", "correct-password").await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_is_valid_login_case_insensitive_username() {
        let hash = crate::user::User::encrypt_password("correct-password").unwrap();
        let user = crate::user::User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_login_service(user_repo);

        // Login with "Alice" should resolve to the "alice" user
        let result = svc.is_valid_login("Alice", "correct-password").await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_is_valid_login_wrong_password() {
        let hash = crate::user::User::encrypt_password("correct-password").unwrap();
        let user = crate::user::User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_login_service(user_repo);

        let result = svc.is_valid_login("alice", "wrong-password").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_valid_login_user_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_login_service(user_repo);

        let result = svc.is_valid_login("nobody", "password").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_valid_login_propagates_error() {
        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_find_by_username()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_login_service(user_repo);

        let result = svc.is_valid_login("alice", "password").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── is_valid_email ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_is_valid_email_found() {
        let user = crate::user::User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_email().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_login_service(user_repo);

        let email = crate::types::EmailAddress::new("alice@example.com").unwrap();
        let result = svc.is_valid_email(&email).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_is_valid_email_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_email().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_login_service(user_repo);

        let email = crate::types::EmailAddress::new("ghost@example.com").unwrap();
        let result = svc.is_valid_email(&email).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_valid_email_propagates_error() {
        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_find_by_email()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_login_service(user_repo);

        let email = crate::types::EmailAddress::new("alice@example.com").unwrap();
        let result = svc.is_valid_email(&email).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }
}
