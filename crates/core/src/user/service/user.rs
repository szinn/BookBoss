use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    library::ALL_BOOKS_LIBRARY_TOKEN,
    repository::RepositoryService,
    user::{NewUser, User, UserId, UserToken},
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    async fn add_user(&self, user: NewUser) -> Result<User, Error>;
    async fn update_user(&self, user: User) -> Result<User, Error>;
    async fn list_users(&self, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error>;
    async fn delete_user(&self, id: UserId) -> Result<User, Error>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, Error>;
    async fn find_by_token(&self, token: UserToken) -> Result<Option<User>, Error>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, Error>;
}

pub(crate) struct UserServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl UserServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl UserService for UserServiceImpl {
    async fn add_user(&self, user: NewUser) -> Result<User, Error> {
        with_transaction!(self, user_repository, |tx| user_repository.add_user(tx, user).await)
    }

    async fn update_user(&self, user: User) -> Result<User, Error> {
        with_transaction!(self, user_repository, |tx| user_repository.update_user(tx, user).await)
    }

    async fn list_users(&self, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.list_users(tx, start_id, page_size).await)
    }

    async fn delete_user(&self, id: UserId) -> Result<User, Error> {
        with_transaction!(self, user_repository, library_repository, shelf_repository, |tx| {
            let user = user_repository
                .find_by_id(tx, id)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            // Delete all shelves owned by this user. Shelves belong to the
            // user, so they are removed rather than re-parented.
            shelf_repository.delete_shelves_for_user(tx, id).await?;

            // Clean up the user's personal library before deleting the user.
            // The DB FK is ON DELETE CASCADE (Postgres/MySQL) but we do this
            // explicitly so that:
            //   a) stale default_library settings are reset,
            //   b) SQLite (test-only) is covered even without FK enforcement.
            if let Some(lib) = library_repository.find_by_owner(tx, id).await? {
                library_repository
                    .reset_default_library_for_users(tx, &lib.token.to_string(), ALL_BOOKS_LIBRARY_TOKEN)
                    .await?;
                library_repository.delete_library(tx, lib.id).await?;
            }

            user_repository.delete_user(tx, user).await
        })
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_id(tx, id).await)
    }

    async fn find_by_token(&self, token: UserToken) -> Result<Option<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_id(tx, token.id()).await)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, Error> {
        let username = username.to_owned();
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_username(tx, &username).await)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use super::{UserService, UserServiceImpl};
    use crate::{
        Error, RepositoryError,
        library::repository::MockLibraryRepository,
        shelf::repository::shelf::MockShelfRepository,
        user::{NewUser, User, UserToken, repository::user::MockUserRepository},
    };

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn create_service(user_repo: MockUserRepository) -> UserServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .user_repository(Arc::new(user_repo))
                .build()
                .expect("all fields provided"),
        );
        UserServiceImpl::new(repository_service)
    }

    fn create_service_for_delete(user_repo: MockUserRepository, shelf_repo: MockShelfRepository, lib_repo: MockLibraryRepository) -> UserServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .user_repository(Arc::new(user_repo))
                .shelf_repository(Arc::new(shelf_repo))
                .library_repository(Arc::new(lib_repo))
                .build()
                .expect("all fields provided"),
        );
        UserServiceImpl::new(repository_service)
    }

    // ─── add_user ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_user_success() {
        let expected = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_add_user().returning(move |_, _| {
            let expected = expected.clone();
            Box::pin(async move { Ok(expected) })
        });
        let svc = create_service(user_repo);

        let result = svc
            .add_user(NewUser::new("alice", "hash", "alice@example.com", HashSet::new(), "Alice", false).unwrap())
            .await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.username, "alice");
        assert_eq!(user.email_address.as_str(), "alice@example.com");
    }

    #[tokio::test]
    async fn test_add_user_propagates_constraint_error() {
        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_add_user()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Constraint("duplicate email".into()))) }));
        let svc = create_service(user_repo);

        let result = svc
            .add_user(NewUser::new("alice", "hash", "alice@example.com", HashSet::new(), "Alice", false).unwrap())
            .await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── update_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_user_success() {
        let updated = User::fake(1, "alice-updated", "newhash", "new@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_update_user().returning(move |_, _| {
            let updated = updated.clone();
            Box::pin(async move { Ok(updated) })
        });
        let svc = create_service(user_repo);

        let result = svc.update_user(User::fake(1, "alice", "hash", "alice@example.com", HashSet::new())).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().username, "alice-updated");
    }

    #[tokio::test]
    async fn test_update_user_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_update_user()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::NotFound)) }));
        let svc = create_service(user_repo);

        let result = svc.update_user(User::fake(999, "ghost", "hash", "ghost@example.com", HashSet::new())).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── list_users ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_users_returns_all() {
        let users = vec![
            User::fake(1, "alice", "h1", "alice@example.com", HashSet::new()),
            User::fake(2, "bob", "h2", "bob@example.com", HashSet::new()),
        ];
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_list_users().returning(move |_, _, _| {
            let users = users.clone();
            Box::pin(async move { Ok(users) })
        });
        let svc = create_service(user_repo);

        let result = svc.list_users(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].username, "alice");
        assert_eq!(list[1].username, "bob");
    }

    #[tokio::test]
    async fn test_list_users_empty() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_list_users().returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_service(user_repo);

        let result = svc.list_users(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── delete_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_user_success() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let deleted = user.clone();
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        user_repo.expect_delete_user().returning(move |_, _| {
            let deleted = deleted.clone();
            Box::pin(async move { Ok(deleted) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_delete_shelves_for_user().returning(|_, _| Box::pin(async { Ok(()) }));
        // No personal library for this user.
        let mut lib_repo = MockLibraryRepository::new();
        lib_repo.expect_find_by_owner().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service_for_delete(user_repo, shelf_repo, lib_repo);

        let result = svc.delete_user(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, 1);
    }

    #[tokio::test]
    async fn test_delete_user_with_personal_library_deletes_library() {
        use chrono::Utc;

        use crate::library::{Library, LibraryToken};

        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let deleted = user.clone();
        let lib = Library {
            id: 42,
            version: 1,
            token: LibraryToken::new(42),
            name: "Alice's Library".into(),
            is_system: false,
            owner_id: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let lib_token = lib.token.to_string();

        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        user_repo.expect_delete_user().returning(move |_, _| {
            let deleted = deleted.clone();
            Box::pin(async move { Ok(deleted) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_delete_shelves_for_user().returning(|_, _| Box::pin(async { Ok(()) }));
        let mut lib_repo = MockLibraryRepository::new();
        lib_repo.expect_find_by_owner().returning(move |_, _| {
            let lib = lib.clone();
            Box::pin(async move { Ok(Some(lib)) })
        });
        lib_repo
            .expect_reset_default_library_for_users()
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        lib_repo.expect_delete_library().returning(|_, _| Box::pin(async { Ok(()) }));

        let svc = create_service_for_delete(user_repo, shelf_repo, lib_repo);

        let result = svc.delete_user(1).await;

        result.unwrap();
        let _ = lib_token; // used by closure above
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(user_repo);

        let result = svc.delete_user(999).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_service(user_repo);

        let result = svc.find_by_id(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(user_repo);

        let result = svc.find_by_id(999).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── find_by_token ───────────────────────────────────────────────────────
    // The service extracts token.id() and delegates to find_by_id, so we
    // configure find_by_id rather than a separate token mock.

    #[tokio::test]
    async fn test_find_by_token_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let token = user.token;
        let user_clone = user.clone();
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let user_clone = user_clone.clone();
            Box::pin(async move { Ok(Some(user_clone)) })
        });
        let svc = create_service(user_repo);

        let result = svc.find_by_token(token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(user_repo);

        let result = svc.find_by_token(UserToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── find_by_username ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_username_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(move |_, _| {
            let user = user.clone();
            Box::pin(async move { Ok(Some(user)) })
        });
        let svc = create_service(user_repo);

        let result = svc.find_by_username("alice").await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_username_not_found() {
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_username().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(user_repo);

        let result = svc.find_by_username("nobody").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_username_propagates_error() {
        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_find_by_username()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::NotFound)) }));
        let svc = create_service(user_repo);

        let result = svc.find_by_username("alice").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }
}
