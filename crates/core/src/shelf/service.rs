use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    book::{Book, BookToken},
    device::{Device, DeviceId},
    filter::BookFilter,
    repository::RepositoryService,
    shelf::{BookShelf, Shelf, ShelfToken, ShelfType},
    user::UserId,
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait ShelfService: Send + Sync {
    /// Creates a new manual shelf for the given user.
    ///
    /// Returns an error if the name is empty or a shelf with the same name
    /// already exists for the user.
    async fn create_manual_shelf(&self, owner_id: UserId, name: String) -> Result<ShelfToken, Error>;

    /// Renames a shelf. Only the owner may rename.
    async fn rename_shelf(&self, token: ShelfToken, new_name: String, user_id: UserId) -> Result<(), Error>;

    /// Deletes a shelf. Only the owner may delete.
    async fn delete_shelf(&self, token: ShelfToken, user_id: UserId) -> Result<(), Error>;

    /// Adds a book to a shelf. Only the owner may add books.
    async fn add_book_to_shelf(&self, shelf_token: ShelfToken, book_token: BookToken, user_id: UserId) -> Result<(), Error>;

    /// Removes a book from a shelf. Only the owner may remove books.
    async fn remove_book_from_shelf(&self, shelf_token: ShelfToken, book_token: BookToken, user_id: UserId) -> Result<(), Error>;

    /// Returns paginated books for a shelf.
    ///
    /// Only the owner may access the shelf.
    async fn books_for_shelf(&self, token: ShelfToken, user_id: UserId, offset: Option<u64>, page_size: Option<u64>) -> Result<Vec<BookShelf>, Error>;

    /// Returns all shelves owned by the given user.
    async fn list_shelves_for_user(&self, user_id: UserId) -> Result<Vec<Shelf>, Error>;

    /// Returns metadata for a single shelf.
    ///
    /// Returns `NotFound` for missing shelves and a validation error
    /// if the requester does not own the shelf.
    async fn get_shelf(&self, token: ShelfToken, user_id: UserId) -> Result<Shelf, Error>;

    /// Updates the name of a shelf in a single transaction.
    ///
    /// Only the owner may update. Returns an error if the name is empty or
    /// another shelf owned by the same user already has that name.
    async fn update_shelf(&self, token: ShelfToken, new_name: String, user_id: UserId) -> Result<(), Error>;

    /// Creates a new smart shelf for the given user with the provided filter.
    ///
    /// Returns an error if the name is empty or a shelf with the same name
    /// already exists for the user.
    async fn create_smart_shelf(&self, owner_id: UserId, name: String, filter: BookFilter) -> Result<ShelfToken, Error>;

    /// Replaces the filter on an existing smart shelf.
    ///
    /// Only the owner may update. Returns an error if the shelf is not a smart
    /// shelf or the caller does not own it.
    async fn update_shelf_filter(&self, token: ShelfToken, filter: BookFilter, user_id: UserId) -> Result<(), Error>;

    /// Returns paginated books matching this smart shelf's filter.
    ///
    /// Only callable for smart shelves. Only the owner may access the shelf.
    async fn books_for_filter(
        &self,
        token: ShelfToken,
        user_id: UserId,
        offset: Option<u64>,
        page_size: Option<u64>,
        sort: Option<crate::book::BookSortOrder>,
    ) -> Result<Vec<Book>, Error>;

    /// Returns the number of books on this shelf for the given user.
    ///
    /// Works for both smart shelves (using their stored filter) and manual
    /// shelves. Returns an error if the caller is not the owner.
    async fn count_for_filter(&self, token: ShelfToken, user_id: UserId) -> Result<u64, Error>;

    /// Creates a private smart shelf linked to a device, with a default filter
    /// of `ReadStatus IncludesAny [Active]`.
    ///
    /// Intended to be called by `DeviceService` at device creation time. The
    /// name is expected to already be unique for the owner.
    async fn create_device_shelf(&self, device_id: DeviceId, owner_id: UserId, name: String) -> Result<ShelfToken, Error>;

    /// Returns the companion shelf linked to the given device, if one exists.
    async fn find_device_shelf(&self, device_id: DeviceId) -> Result<Option<Shelf>, Error>;
}

pub(crate) struct ShelfServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl ShelfServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl ShelfService for ShelfServiceImpl {
    async fn create_manual_shelf(&self, owner_id: UserId, name: String) -> Result<ShelfToken, Error> {
        if name.trim().is_empty() {
            return Err(Error::Validation("shelf name must not be empty".to_string()));
        }

        let name_lower = name.to_lowercase();

        with_transaction!(self, shelf_repository, user_setting_repository, library_repository, |tx| {
            // Resolve the user's default library, falling back to All Books.
            let library_id = crate::library::resolve_user_default_library(tx, user_setting_repository.as_ref(), library_repository.as_ref(), owner_id).await?;

            let existing = shelf_repository.list_for_user(tx, owner_id).await?;
            if existing.iter().any(|s| s.name.to_lowercase() == name_lower) {
                return Err(Error::RepositoryError(RepositoryError::Conflict));
            }

            let new_shelf = shelf_repository
                .add_shelf(
                    tx,
                    crate::shelf::NewShelf {
                        owner_id,
                        library_id,
                        name,
                        shelf_type: crate::shelf::ShelfType::Manual,
                        device_id: None,
                        filter_criteria: None,
                    },
                )
                .await?;

            Ok(new_shelf.token)
        })
    }

    async fn rename_shelf(&self, token: ShelfToken, new_name: String, user_id: UserId) -> Result<(), Error> {
        if new_name.trim().is_empty() {
            return Err(Error::Validation("shelf name must not be empty".to_string()));
        }

        let new_name_lower = new_name.to_lowercase();

        with_transaction!(self, shelf_repository, device_repository, |tx| {
            let existing_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if existing_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may rename a shelf".to_string()));
            }

            let existing = shelf_repository.list_for_user(tx, user_id).await?;
            if existing.iter().any(|s| s.token != token && s.name.to_lowercase() == new_name_lower) {
                return Err(Error::RepositoryError(RepositoryError::Conflict));
            }

            let device_id = existing_shelf.device_id;
            let updated = Shelf {
                name: new_name.clone(),
                ..existing_shelf
            };
            shelf_repository.update_shelf(tx, updated).await?;

            if let Some(did) = device_id
                && let Some(device) = device_repository.find_by_id(tx, did).await?
            {
                device_repository.update_device(tx, Device { name: new_name, ..device }).await?;
            }

            Ok(())
        })
    }

    async fn delete_shelf(&self, token: ShelfToken, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, shelf_repository, |tx| {
            let shelf_to_delete = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if shelf_to_delete.owner_id != user_id {
                return Err(Error::Validation("only the owner may delete a shelf".to_string()));
            }

            shelf_repository.delete_shelf(tx, shelf_to_delete).await
        })
    }

    async fn add_book_to_shelf(&self, shelf_token: ShelfToken, book_token: BookToken, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, shelf_repository, book_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, shelf_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may add books to a shelf".to_string()));
            }

            let book = book_repository
                .find_by_token(tx, book_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            shelf_repository
                .add_book_to_shelf(
                    tx,
                    BookShelf {
                        shelf_id: target_shelf.id,
                        book_id: book.id,
                        sort_order: 0,
                        added_at: chrono::Utc::now(),
                    },
                )
                .await?;

            Ok(())
        })
    }

    async fn remove_book_from_shelf(&self, shelf_token: ShelfToken, book_token: BookToken, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, shelf_repository, book_repository, |tx| {
            let current_shelf = shelf_repository
                .find_by_token(tx, shelf_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if current_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may remove books from a shelf".to_string()));
            }

            let book = book_repository
                .find_by_token(tx, book_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            shelf_repository.remove_book_from_shelf(tx, current_shelf.id, book.id).await
        })
    }

    async fn books_for_shelf(&self, token: ShelfToken, user_id: UserId, offset: Option<u64>, page_size: Option<u64>) -> Result<Vec<BookShelf>, Error> {
        with_read_only_transaction!(self, shelf_repository, |tx| {
            let current_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if current_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may access this shelf".to_string()));
            }

            shelf_repository.books_for_shelf(tx, current_shelf.id, offset, page_size).await
        })
    }

    async fn list_shelves_for_user(&self, user_id: UserId) -> Result<Vec<Shelf>, Error> {
        with_read_only_transaction!(self, shelf_repository, |tx| shelf_repository.list_for_user(tx, user_id).await)
    }

    async fn get_shelf(&self, token: ShelfToken, user_id: UserId) -> Result<Shelf, Error> {
        with_read_only_transaction!(self, shelf_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may access this shelf".to_string()));
            }

            Ok(target_shelf)
        })
    }

    async fn update_shelf(&self, token: ShelfToken, new_name: String, user_id: UserId) -> Result<(), Error> {
        if new_name.trim().is_empty() {
            return Err(Error::Validation("shelf name must not be empty".to_string()));
        }

        let new_name_lower = new_name.to_lowercase();

        with_transaction!(self, shelf_repository, device_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may update a shelf".to_string()));
            }

            let existing = shelf_repository.list_for_user(tx, user_id).await?;
            if existing.iter().any(|s| s.token != token && s.name.to_lowercase() == new_name_lower) {
                return Err(Error::RepositoryError(RepositoryError::Conflict));
            }

            let device_id = target_shelf.device_id;
            let updated = Shelf {
                name: new_name.clone(),
                ..target_shelf
            };
            shelf_repository.update_shelf(tx, updated).await?;

            if let Some(did) = device_id
                && let Some(device) = device_repository.find_by_id(tx, did).await?
            {
                device_repository.update_device(tx, Device { name: new_name, ..device }).await?;
            }

            Ok(())
        })
    }

    async fn create_smart_shelf(&self, owner_id: UserId, name: String, filter: BookFilter) -> Result<ShelfToken, Error> {
        if name.trim().is_empty() {
            return Err(Error::Validation("shelf name must not be empty".to_string()));
        }

        let name_lower = name.to_lowercase();

        with_transaction!(self, shelf_repository, user_setting_repository, library_repository, |tx| {
            // Resolve the user's default library, falling back to All Books.
            let library_id = crate::library::resolve_user_default_library(tx, user_setting_repository.as_ref(), library_repository.as_ref(), owner_id).await?;

            let existing = shelf_repository.list_for_user(tx, owner_id).await?;
            if existing.iter().any(|s| s.name.to_lowercase() == name_lower) {
                return Err(Error::RepositoryError(RepositoryError::Conflict));
            }

            let new_shelf = shelf_repository
                .add_shelf(
                    tx,
                    crate::shelf::NewShelf {
                        owner_id,
                        library_id,
                        name,
                        shelf_type: ShelfType::Smart,
                        device_id: None,
                        filter_criteria: Some(filter),
                    },
                )
                .await?;

            Ok(new_shelf.token)
        })
    }

    async fn update_shelf_filter(&self, token: ShelfToken, filter: BookFilter, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, shelf_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may update a shelf filter".to_string()));
            }

            if target_shelf.shelf_type != ShelfType::Smart {
                return Err(Error::Validation("filter can only be set on a smart shelf".to_string()));
            }

            let updated = Shelf {
                filter_criteria: Some(filter),
                ..target_shelf
            };
            shelf_repository.update_shelf(tx, updated).await?;

            Ok(())
        })
    }

    async fn books_for_filter(
        &self,
        token: ShelfToken,
        user_id: UserId,
        offset: Option<u64>,
        page_size: Option<u64>,
        sort: Option<crate::book::BookSortOrder>,
    ) -> Result<Vec<Book>, Error> {
        with_read_only_transaction!(self, shelf_repository, collection_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may access this shelf".to_string()));
            }

            if target_shelf.shelf_type != ShelfType::Smart {
                return Err(Error::Validation("books_for_filter only works on smart shelves".to_string()));
            }

            let filter = target_shelf
                .filter_criteria
                .ok_or_else(|| Error::Validation("smart shelf has no filter criteria".to_string()))?;

            collection_repository
                .books_for_filter(tx, &filter, user_id, Some(target_shelf.library_id), offset, page_size, sort)
                .await
        })
    }

    async fn count_for_filter(&self, token: ShelfToken, user_id: UserId) -> Result<u64, Error> {
        use crate::filter::{BookFilter, EntityRef, FilterRule, SetOp};

        with_read_only_transaction!(self, shelf_repository, collection_repository, |tx| {
            let target_shelf = shelf_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if target_shelf.owner_id != user_id {
                return Err(Error::Validation("only the owner may access this shelf".to_string()));
            }

            let filter = match target_shelf.shelf_type {
                // System shelves are always filter-based (like Smart), so they use stored filter_criteria.
                ShelfType::Smart | ShelfType::System => target_shelf
                    .filter_criteria
                    .ok_or_else(|| Error::Validation("smart shelf has no filter criteria".to_string()))?,
                ShelfType::Manual => BookFilter::Rule(FilterRule::Shelf {
                    op: SetOp::IncludesAny,
                    values: vec![EntityRef {
                        id: target_shelf.id as i64,
                        label: target_shelf.name.clone(),
                    }],
                }),
            };

            // Manual shelves are membership lists, not library-scoped queries —
            // their books may live in any library. Smart/System shelves run filter
            // queries against a library, so they still pass Some(library_id).
            let scope_library = match target_shelf.shelf_type {
                ShelfType::Manual => None,
                ShelfType::Smart | ShelfType::System => Some(target_shelf.library_id),
            };
            collection_repository.count_for_filter(tx, &filter, user_id, scope_library).await
        })
    }

    async fn create_device_shelf(&self, device_id: DeviceId, owner_id: UserId, name: String) -> Result<ShelfToken, Error> {
        use crate::filter::{BookFilter, FilterReadStatus, FilterRule, SetOp};

        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Active],
        });

        with_transaction!(self, shelf_repository, user_setting_repository, library_repository, |tx| {
            // Resolve the user's default library, falling back to All Books.
            let library_id = crate::library::resolve_user_default_library(tx, user_setting_repository.as_ref(), library_repository.as_ref(), owner_id).await?;

            let target_shelf = shelf_repository
                .add_shelf(
                    tx,
                    crate::shelf::NewShelf {
                        owner_id,
                        library_id,
                        name,
                        shelf_type: ShelfType::Smart,
                        device_id: Some(device_id),
                        filter_criteria: Some(filter),
                    },
                )
                .await?;
            Ok(target_shelf.token)
        })
    }

    async fn find_device_shelf(&self, device_id: DeviceId) -> Result<Option<Shelf>, Error> {
        with_read_only_transaction!(self, shelf_repository, |tx| shelf_repository.find_by_device_id(tx, device_id).await)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;

    use super::{ShelfService, ShelfServiceImpl};
    use crate::{
        Error, RepositoryError,
        book::{Book, BookStatus, BookToken, repository::book::MockBookRepository},
        collection::MockCollectionRepository,
        library::MockLibraryRepository,
        shelf::{BookShelf, Shelf, ShelfToken, ShelfType, repository::shelf::MockShelfRepository},
        user::{UserId, repository::user_settings::MockUserSettingRepository},
    };

    // ─── Helpers ──────────────────────────────────────────────────────────────

    /// General-purpose service factory for tests that do NOT call
    /// `create_manual_shelf` or `create_smart_shelf`. Those methods require
    /// `user_setting_repository` and `library_repository` — use
    /// `create_service_with_default_library_repos` instead.
    fn create_service(shelf_repo: MockShelfRepository, book_repo: MockBookRepository) -> ShelfServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .shelf_repository(Arc::new(shelf_repo))
                .build()
                .expect("all fields provided"),
        );
        ShelfServiceImpl::new(repository_service)
    }

    fn create_service_with_collection_repo(shelf_repo: MockShelfRepository, collection_repo: MockCollectionRepository) -> ShelfServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .shelf_repository(Arc::new(shelf_repo))
                .collection_repository(Arc::new(collection_repo))
                .build()
                .expect("all fields provided"),
        );
        ShelfServiceImpl::new(repository_service)
    }

    /// Helper for tests that don't care about library resolution — returns
    /// `Ok(None)` for the setting so the implementation falls back to
    /// `ALL_BOOKS_LIBRARY_ID`.
    fn create_service_with_default_library_repos(shelf_repo: MockShelfRepository, book_repo: MockBookRepository) -> ShelfServiceImpl {
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        create_service_with_setting_and_library_repos(shelf_repo, book_repo, setting_repo, MockLibraryRepository::new())
    }

    fn create_service_with_setting_and_library_repos(
        shelf_repo: MockShelfRepository,
        book_repo: MockBookRepository,
        setting_repo: MockUserSettingRepository,
        library_repo: MockLibraryRepository,
    ) -> ShelfServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .shelf_repository(Arc::new(shelf_repo))
                .user_setting_repository(Arc::new(setting_repo))
                .library_repository(Arc::new(library_repo))
                .build()
                .expect("all fields provided"),
        );
        ShelfServiceImpl::new(repository_service)
    }

    fn fake_shelf(owner_id: UserId) -> Shelf {
        Shelf {
            id: 1,
            version: 1,
            token: ShelfToken::new(1),
            owner_id,
            library_id: crate::library::ALL_BOOKS_LIBRARY_ID,
            name: "My Shelf".to_string(),
            shelf_type: ShelfType::Manual,
            device_id: None,
            filter_criteria: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn fake_book() -> Book {
        Book::fake(1, "Test Book", BookStatus::Available)
    }

    fn fake_book_shelf() -> BookShelf {
        BookShelf {
            shelf_id: 1,
            book_id: 1,
            sort_order: 1,
            added_at: Utc::now(),
        }
    }

    // ─── create_manual_shelf ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_manual_shelf_returns_token() {
        let shelf = fake_shelf(1);
        let expected_token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_add_shelf().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let svc = create_service_with_default_library_repos(shelf_repo, MockBookRepository::new());

        let result = svc.create_manual_shelf(1, "My Shelf".to_string()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_token);
    }

    #[tokio::test]
    async fn test_create_manual_shelf_empty_name_returns_validation_error() {
        let svc = create_service_with_default_library_repos(MockShelfRepository::new(), MockBookRepository::new());

        for name in ["", "   "] {
            let result = svc.create_manual_shelf(1, name.to_string()).await;
            assert!(matches!(result, Err(Error::Validation(_))), "expected Validation for name={name:?}");
        }
    }

    #[tokio::test]
    async fn test_create_manual_shelf_duplicate_name_returns_conflict() {
        let existing = fake_shelf(1);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let e = existing.clone();
            Box::pin(async move { Ok(vec![e]) })
        });
        let svc = create_service_with_default_library_repos(shelf_repo, MockBookRepository::new());

        let result = svc.create_manual_shelf(1, "My Shelf".to_string()).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_create_manual_shelf_case_insensitive_duplicate() {
        let mut existing = fake_shelf(1);
        existing.name = "Fantasy".to_string();
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let e = existing.clone();
            Box::pin(async move { Ok(vec![e]) })
        });
        let svc = create_service_with_default_library_repos(shelf_repo, MockBookRepository::new());

        let result = svc.create_manual_shelf(1, "fantasy".to_string()).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    // ─── rename_shelf ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_rename_shelf_success() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let updated = Shelf {
            name: "New Name".to_string(),
            ..shelf.clone()
        };
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_find_by_id().returning(|_, _| Box::pin(async { Ok(None) }));
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let u = updated.clone();
            Box::pin(async move { Ok(u) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.rename_shelf(token, "New Name".to_string(), 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_rename_shelf_empty_name_returns_validation_error() {
        let svc = create_service(MockShelfRepository::new(), MockBookRepository::new());
        let token = ShelfToken::new(1);

        let result = svc.rename_shelf(token, String::new(), 1).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_rename_shelf_not_found() {
        let token = ShelfToken::new(99);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.rename_shelf(token, "New Name".to_string(), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_rename_shelf_wrong_owner_returns_validation_error() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.rename_shelf(token, "New Name".to_string(), 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_rename_shelf_duplicate_name_returns_conflict() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let mut other = fake_shelf(1);
        other.id = 2;
        other.token = ShelfToken::new(2);
        other.name = "Other Shelf".to_string();
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let o = other.clone();
            Box::pin(async move { Ok(vec![o]) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        // Rename current shelf to the name of the other shelf
        let result = svc.rename_shelf(token, "Other Shelf".to_string(), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_rename_shelf_same_name_as_self_succeeds() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let updated = shelf.clone();
        let shelf_for_token = shelf.clone();
        let shelf_for_list = shelf.clone();
        // list_for_user returns the shelf itself — must not conflict with itself
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf_for_token.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let s = shelf_for_list.clone();
            Box::pin(async move { Ok(vec![s]) })
        });
        shelf_repo.expect_find_by_id().returning(|_, _| Box::pin(async { Ok(None) }));
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let u = updated.clone();
            Box::pin(async move { Ok(u) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.rename_shelf(token, "My Shelf".to_string(), 1).await;

        result.unwrap();
    }

    // ─── delete_shelf ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_shelf_success() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_delete_shelf().times(1).returning(|_, _| Box::pin(async { Ok(()) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());
        let token = ShelfToken::new(1);

        let result = svc.delete_shelf(token, 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_delete_shelf_not_found() {
        let token = ShelfToken::new(99);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.delete_shelf(token, 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_delete_shelf_wrong_owner() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.delete_shelf(token, 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── add_book_to_shelf ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_book_to_shelf_success() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_add_book_to_shelf().times(1).returning(|_, _| {
            let bs = fake_book_shelf();
            Box::pin(async move { Ok(bs) })
        });
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(|_, _| {
            let b = fake_book();
            Box::pin(async move { Ok(Some(b)) })
        });
        let svc = create_service(shelf_repo, book_repo);

        let result = svc.add_book_to_shelf(ShelfToken::new(1), BookToken::new(1), 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_add_book_to_shelf_shelf_not_found() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.add_book_to_shelf(ShelfToken::new(1), BookToken::new(1), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_add_book_to_shelf_book_not_found() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, book_repo);

        let result = svc.add_book_to_shelf(ShelfToken::new(1), BookToken::new(99), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_add_book_to_shelf_wrong_owner() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.add_book_to_shelf(ShelfToken::new(1), BookToken::new(1), 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── remove_book_from_shelf ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_remove_book_from_shelf_success() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo
            .expect_remove_book_from_shelf()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(|_, _| {
            let b = fake_book();
            Box::pin(async move { Ok(Some(b)) })
        });
        let svc = create_service(shelf_repo, book_repo);

        let result = svc.remove_book_from_shelf(ShelfToken::new(1), BookToken::new(1), 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_remove_book_from_shelf_wrong_owner() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| {
            let s = fake_shelf(1);
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.remove_book_from_shelf(ShelfToken::new(1), BookToken::new(1), 2).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── books_for_shelf ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_shelf_owner_can_access_private() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let bs = fake_book_shelf();
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_books_for_shelf().returning(move |_, _, _, _| {
            let b = bs.clone();
            Box::pin(async move { Ok(vec![b]) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.books_for_shelf(token, 1, None, None).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_books_for_shelf_non_owner_blocked() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.books_for_shelf(token, 2, None, None).await; // user 2 accessing user 1's shelf

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_books_for_shelf_not_found() {
        let token = ShelfToken::new(99);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.books_for_shelf(token, 1, None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── list_shelves_for_user ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_shelves_for_user_returns_all() {
        let mut shelf2 = fake_shelf(1);
        shelf2.id = 2;
        shelf2.token = ShelfToken::new(2);
        shelf2.name = "Public Shelf".to_string();
        let shelves = vec![fake_shelf(1), shelf2];
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let v = shelves.clone();
            Box::pin(async move { Ok(v) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.list_shelves_for_user(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    // ─── get_shelf ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_shelf_owner_can_access() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        svc.get_shelf(token, 1).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_shelf_non_owner_denied_private() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.get_shelf(token, 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_get_shelf_non_owner_blocked() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.get_shelf(token, 2).await; // user 2 — not the owner

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── update_shelf ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_shelf_success() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let updated = Shelf {
            name: "Renamed".to_string(),
            ..shelf.clone()
        };
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let u = updated.clone();
            Box::pin(async move { Ok(u) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf(token, "Renamed".to_string(), 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_update_shelf_empty_name_returns_validation_error() {
        let svc = create_service(MockShelfRepository::new(), MockBookRepository::new());
        let token = ShelfToken::new(1);

        let result = svc.update_shelf(token, "  ".to_string(), 1).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_update_shelf_not_found() {
        let token = ShelfToken::new(99);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf(token, "New Name".to_string(), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_update_shelf_wrong_owner_returns_validation_error() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf(token, "New Name".to_string(), 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_update_shelf_duplicate_name_returns_conflict() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let mut other = fake_shelf(1);
        other.id = 2;
        other.token = ShelfToken::new(2);
        other.name = "Other Shelf".to_string();
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let o = other.clone();
            Box::pin(async move { Ok(vec![o]) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf(token, "Other Shelf".to_string(), 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_update_shelf_same_name_as_self_succeeds() {
        let shelf = fake_shelf(1);
        let token = shelf.token;
        let updated = shelf.clone();
        let shelf_for_token = shelf.clone();
        let shelf_for_list = shelf.clone();
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf_for_token.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let s = shelf_for_list.clone();
            Box::pin(async move { Ok(vec![s]) })
        });
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let u = updated.clone();
            Box::pin(async move { Ok(u) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        // Same name — must not conflict with itself
        let result = svc.update_shelf(token, "My Shelf".to_string(), 1).await;

        result.unwrap();
    }

    // ─── create_smart_shelf ───────────────────────────────────────────────────

    fn simple_filter() -> crate::filter::BookFilter {
        use crate::filter::{BookFilter, FilterReadStatus, FilterRule, SetOp};
        BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Active],
        })
    }

    fn fake_smart_shelf(owner_id: UserId) -> Shelf {
        Shelf {
            shelf_type: ShelfType::Smart,
            filter_criteria: Some(simple_filter()),
            ..fake_shelf(owner_id)
        }
    }

    #[tokio::test]
    async fn test_create_smart_shelf_succeeds() {
        let shelf = fake_smart_shelf(1);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_add_shelf().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let svc = create_service_with_default_library_repos(shelf_repo, MockBookRepository::new());

        svc.create_smart_shelf(1, "Unread Sci-Fi".to_string(), simple_filter()).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_smart_shelf_rejects_empty_name() {
        let svc = create_service_with_default_library_repos(MockShelfRepository::new(), MockBookRepository::new());

        let result = svc.create_smart_shelf(1, "  ".to_string(), simple_filter()).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_create_smart_shelf_rejects_duplicate_name() {
        let existing = fake_shelf(1);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(move |_, _| {
            let e = existing.clone();
            Box::pin(async move { Ok(vec![e]) })
        });
        let svc = create_service_with_default_library_repos(shelf_repo, MockBookRepository::new());

        // "my shelf" matches existing "My Shelf" (case-insensitive)
        let result = svc.create_smart_shelf(1, "My Shelf".to_string(), simple_filter()).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    // ─── update_shelf_filter ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_shelf_filter_succeeds() {
        let shelf = fake_smart_shelf(1);
        let token = shelf.token;
        let updated = Shelf {
            filter_criteria: Some(simple_filter()),
            ..shelf.clone()
        };
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let u = updated.clone();
            Box::pin(async move { Ok(u) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf_filter(token, simple_filter(), 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_update_shelf_filter_rejects_non_owner() {
        let shelf = fake_smart_shelf(1);
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf_filter(token, simple_filter(), 99).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn test_update_shelf_filter_rejects_non_smart_shelf() {
        let shelf = fake_shelf(1); // Manual shelf
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.update_shelf_filter(token, simple_filter(), 1).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── count_for_filter ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_for_filter_manual_shelf_returns_count() {
        // Manual shelves pass `library_id: None` so the count includes books
        // regardless of library membership (BB-16).
        let shelf = fake_shelf(1); // ShelfType::Manual, id=1, name="My Shelf", owner_id=1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_count_for_filter()
            .times(1)
            .withf(|_, _, _, library_id| library_id.is_none())
            .returning(|_, _, _, _| Box::pin(async { Ok(7) }));
        let svc = create_service_with_collection_repo(shelf_repo, collection_repo);

        let result = svc.count_for_filter(token, 1).await;

        assert_eq!(result.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_count_for_filter_smart_shelf_returns_count() {
        let shelf = fake_smart_shelf(1); // ShelfType::Smart with filter_criteria
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_count_for_filter()
            .times(1)
            .withf(|_, _, _, library_id| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(|_, _, _, _| Box::pin(async { Ok(3) }));
        let svc = create_service_with_collection_repo(shelf_repo, collection_repo);

        let result = svc.count_for_filter(token, 1).await;

        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_count_for_filter_wrong_owner_returns_error() {
        let shelf = fake_shelf(1); // owned by user 1
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service_with_collection_repo(shelf_repo, MockCollectionRepository::new());

        let result = svc.count_for_filter(token, 2).await; // user 2

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── books_for_filter ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_forwards_library_id() {
        let shelf = fake_smart_shelf(1);
        let token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_token().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .times(1)
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(|_, _, _, _, _, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_service_with_collection_repo(shelf_repo, collection_repo);

        svc.books_for_filter(token, 1, None, None, None).await.unwrap();
    }

    // ─── create_device_shelf ──────────────────────────────────────────────────

    fn fake_device_shelf(owner_id: UserId, device_id: u64) -> Shelf {
        use crate::filter::{BookFilter, FilterReadStatus, FilterRule, SetOp};
        Shelf {
            shelf_type: ShelfType::Smart,
            device_id: Some(device_id),
            filter_criteria: Some(BookFilter::Rule(FilterRule::ReadStatus {
                op: SetOp::IncludesAny,
                values: vec![FilterReadStatus::Active],
            })),
            ..fake_shelf(owner_id)
        }
    }

    #[tokio::test]
    async fn test_create_device_shelf_succeeds() {
        let device_id = 42u64;
        let shelf = fake_device_shelf(1, device_id);
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_add_shelf().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        let svc = create_service_with_setting_and_library_repos(shelf_repo, MockBookRepository::new(), setting_repo, MockLibraryRepository::new());

        let result = svc.create_device_shelf(device_id, 1, "My Kobo".to_string()).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_create_device_shelf_creates_private_smart_shelf() {
        let device_id = 42u64;
        let shelf = fake_device_shelf(1, device_id);
        let returned_token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_add_shelf().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        let svc = create_service_with_setting_and_library_repos(shelf_repo, MockBookRepository::new(), setting_repo, MockLibraryRepository::new());

        let token = svc.create_device_shelf(device_id, 1, "My Kobo".to_string()).await.unwrap();

        assert_eq!(token, returned_token);
    }

    // ─── find_device_shelf ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_device_shelf_returns_shelf_when_found() {
        let device_id = 42u64;
        let shelf = fake_device_shelf(1, device_id);
        let shelf_token = shelf.token;
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.find_device_shelf(device_id).await.unwrap();

        assert_eq!(result.map(|s| s.token), Some(shelf_token));
    }

    #[tokio::test]
    async fn test_find_device_shelf_returns_none_when_not_found() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(shelf_repo, MockBookRepository::new());

        let result = svc.find_device_shelf(99).await.unwrap();

        assert!(result.is_none());
    }

    // ─── library resolution in create_manual_shelf / create_smart_shelf ──────

    #[tokio::test]
    async fn test_create_manual_shelf_uses_user_default_library() {
        use crate::library::LibraryToken;

        // Setting repo: returns a setting pointing at library_token
        let token_str = LibraryToken::generate().to_string();
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(move |_, _, _| {
            let v = token_str.clone();
            Box::pin(async move {
                Ok(Some(crate::user::UserSetting {
                    user_id: 1,
                    key: "default_library".to_string(),
                    value: v,
                }))
            })
        });

        // Library repo: find_by_token returns a library with id=42
        let mut library_repo = MockLibraryRepository::new();
        library_repo.expect_find_by_token().returning(|_, _| {
            Box::pin(async {
                Ok(Some(crate::library::Library {
                    id: 42,
                    version: 1,
                    token: crate::library::LibraryToken::generate(),
                    owner_id: Some(1),
                    name: "Alice's Library".to_string(),
                    is_system: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))
            })
        });

        // Shelf repo: no existing shelves; add_shelf asserts library_id=42
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_add_shelf().returning(move |_, new_shelf| {
            assert_eq!(new_shelf.library_id, 42, "must use resolved library");
            let mut s = fake_shelf(1);
            s.library_id = new_shelf.library_id;
            Box::pin(async move { Ok(s) })
        });

        let svc = create_service_with_setting_and_library_repos(shelf_repo, MockBookRepository::new(), setting_repo, library_repo);

        svc.create_manual_shelf(1, "My Shelf".to_string()).await.unwrap();
        // Assertion is inside the add_shelf mock above.
    }

    #[tokio::test]
    async fn test_create_smart_shelf_uses_user_default_library() {
        use crate::{
            filter::{BookFilter, FilterCondition, FilterGroup},
            library::LibraryToken,
        };

        let token_str = LibraryToken::generate().to_string();
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(move |_, _, _| {
            let v = token_str.clone();
            Box::pin(async move {
                Ok(Some(crate::user::UserSetting {
                    user_id: 1,
                    key: "default_library".to_string(),
                    value: v,
                }))
            })
        });

        let mut library_repo = MockLibraryRepository::new();
        library_repo.expect_find_by_token().returning(|_, _| {
            Box::pin(async {
                Ok(Some(crate::library::Library {
                    id: 42,
                    version: 1,
                    token: crate::library::LibraryToken::generate(),
                    owner_id: Some(1),
                    name: "Alice's Library".to_string(),
                    is_system: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))
            })
        });

        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_list_for_user().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        shelf_repo.expect_add_shelf().returning(move |_, new_shelf| {
            assert_eq!(new_shelf.library_id, 42, "must use resolved library");
            let mut s = fake_shelf(1);
            s.library_id = new_shelf.library_id;
            Box::pin(async move { Ok(s) })
        });

        let svc = create_service_with_setting_and_library_repos(shelf_repo, MockBookRepository::new(), setting_repo, library_repo);

        let filter = BookFilter::Group(FilterGroup {
            condition: FilterCondition::And,
            items: vec![],
        });
        svc.create_smart_shelf(1, "Active".to_string(), filter).await.unwrap();
        // Assertion is inside the add_shelf mock above.
    }
}
