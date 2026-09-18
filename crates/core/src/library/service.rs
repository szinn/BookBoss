use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    book::BookToken,
    library::{ALL_BOOKS_LIBRARY_ID, ALL_BOOKS_LIBRARY_TOKEN, Library, LibraryId, LibraryToken, NewLibrary},
    repository::RepositoryService,
    user::{UserId, UserSettingService},
    with_read_only_transaction, with_transaction,
};

pub struct LibraryEntry {
    pub library: Library,
    pub user_count: u64,
    pub book_count: u64,
}

#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
#[allow(unused_lifetimes, reason = "async_trait + mockall expansion emits a spurious 'life0 parameter")]
pub trait LibraryService: Send + Sync {
    /// Returns all libraries with user and book counts.
    async fn list_libraries(&self) -> Result<Vec<LibraryEntry>, Error>;

    /// Returns libraries assigned to a user.
    async fn libraries_for_user(&self, user_id: UserId) -> Result<Vec<Library>, Error>;

    /// Creates a new library with the given name. Fails if name is empty.
    async fn create_library(&self, name: String) -> Result<Library, Error>;

    /// Deletes a library. Fails if is_system = true.
    /// Re-parents all shelves to "All Books". Cascade handles library_books and
    /// user_libraries removal.
    async fn delete_library(&self, token: LibraryToken) -> Result<(), Error>;

    /// Assigns an existing library to a user.
    async fn assign_library_to_user(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error>;

    /// Removes a library assignment from a user.
    async fn unassign_library_from_user(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error>;

    /// Sets the user's default library. User must be assigned to the library.
    async fn set_default_library(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error>;

    /// Gets the user's default library token from user settings. Falls back to
    /// ALL_BOOKS_LIBRARY_TOKEN.
    async fn get_default_library_token(&self, user_id: UserId) -> Result<String, Error>;

    /// Adds a book to a library (by tokens).
    async fn add_book_to_library(&self, library_token: LibraryToken, book_token: BookToken) -> Result<(), Error>;

    /// Returns the library_ids for a book.
    async fn library_ids_for_book(&self, book_id: crate::book::BookId) -> Result<Vec<LibraryId>, Error>;

    /// Removes a book from a library (by tokens). Is a no-op if the book is not
    /// in the library. NEVER call this with a system library token — use
    /// the repository directly if needed.
    async fn remove_book_from_library(&self, library_token: LibraryToken, book_token: BookToken) -> Result<(), Error>;

    /// Validates that a user has access to the requested library. Returns Err
    /// if not.
    async fn validate_user_library_access(&self, user_id: UserId, library_id: LibraryId) -> Result<(), Error>;

    /// Looks up a library by token. Returns `None` if not found.
    async fn find_library_by_token(&self, token: LibraryToken) -> Result<Option<Library>, Error>;

    /// Creates a personal library for a user.
    /// Steps: create library, assign to user as default, re-parent user's
    /// shelves from All Books to new library, copy books from All Books
    /// into new library.
    async fn create_personal_library_for_user(&self, user_id: UserId, library_name: String) -> Result<Library, Error>;

    /// Returns the personal library owned by `user_id`, if any.
    async fn find_personal_library_for_user(&self, user_id: UserId) -> Result<Option<Library>, Error>;

    /// Renames a library. Fails if the library is a system library or the
    /// name is empty.
    async fn rename_library(&self, token: LibraryToken, new_name: String) -> Result<(), Error>;
}

pub struct LibraryServiceImpl {
    repository_service: Arc<RepositoryService>,
    user_setting_service: Arc<dyn UserSettingService>,
}

impl LibraryServiceImpl {
    pub fn new(repository_service: Arc<RepositoryService>, user_setting_service: Arc<dyn UserSettingService>) -> Self {
        Self {
            repository_service,
            user_setting_service,
        }
    }
}

#[async_trait::async_trait]
impl LibraryService for LibraryServiceImpl {
    async fn list_libraries(&self) -> Result<Vec<LibraryEntry>, Error> {
        with_read_only_transaction!(self, library_repository, |tx| {
            let libraries = library_repository.list_libraries(tx).await?;
            let mut entries = Vec::with_capacity(libraries.len());
            for lib in libraries {
                let user_count = library_repository.user_count_for_library(tx, lib.id).await?;
                let book_count = library_repository.book_count_for_library(tx, lib.id).await?;
                entries.push(LibraryEntry {
                    library: lib,
                    user_count,
                    book_count,
                });
            }
            Ok(entries)
        })
    }

    async fn libraries_for_user(&self, user_id: UserId) -> Result<Vec<Library>, Error> {
        with_read_only_transaction!(self, library_repository, |tx| library_repository.libraries_for_user(tx, user_id).await)
    }

    async fn create_library(&self, name: String) -> Result<Library, Error> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Validation("Library name cannot be empty".into()));
        }
        with_transaction!(self, library_repository, |tx| {
            // Check for duplicate name before inserting to give a clear error
            // message.
            if library_repository.find_by_name(tx, &name).await?.is_some() {
                return Err(Error::Validation("A library with this name already exists".into()));
            }
            library_repository.create_library(tx, NewLibrary { name, owner_id: None }).await
        })
    }

    async fn delete_library(&self, token: LibraryToken) -> Result<(), Error> {
        with_transaction!(self, library_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if library.is_system {
                return Err(Error::Validation("Cannot delete system library".into()));
            }

            // Re-parent all shelves from this library to "All Books"
            library_repository.reparent_shelves(tx, library.id, ALL_BOOKS_LIBRARY_ID).await?;

            // Reset any user whose default library was this one back to "All
            // Books"
            library_repository
                .reset_default_library_for_users(tx, &library.token.to_string(), ALL_BOOKS_LIBRARY_TOKEN)
                .await?;

            // Delete the library (cascade handles library_books and
            // user_libraries via FK)
            library_repository.delete_library(tx, library.id).await
        })
    }

    async fn assign_library_to_user(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error> {
        with_transaction!(self, library_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, library_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            library_repository.assign_user_to_library(tx, user_id, library.id).await
        })
    }

    async fn unassign_library_from_user(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error> {
        with_transaction!(self, library_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, library_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            library_repository.unassign_user_from_library(tx, user_id, library.id).await
        })
    }

    async fn set_default_library(&self, user_id: UserId, library_token: LibraryToken) -> Result<(), Error> {
        // Verify user is assigned to this library
        let has_access = with_read_only_transaction!(self, library_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, library_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            library_repository.user_has_library(tx, user_id, library.id).await
        })?;

        if !has_access {
            return Err(Error::Validation("User is not assigned to this library".into()));
        }

        self.user_setting_service.set(user_id, "default_library", &library_token.to_string()).await?;
        Ok(())
    }

    async fn get_default_library_token(&self, user_id: UserId) -> Result<String, Error> {
        let setting = self.user_setting_service.get(user_id, "default_library").await?;
        Ok(setting.map_or_else(|| ALL_BOOKS_LIBRARY_TOKEN.to_string(), |s| s.value))
    }

    async fn add_book_to_library(&self, library_token: LibraryToken, book_token: BookToken) -> Result<(), Error> {
        with_transaction!(self, library_repository, book_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, library_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            let book = book_repository
                .find_by_token(tx, book_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            library_repository.add_book_to_library(tx, library.id, book.id).await
        })
    }

    async fn library_ids_for_book(&self, book_id: crate::book::BookId) -> Result<Vec<LibraryId>, Error> {
        with_read_only_transaction!(self, library_repository, |tx| library_repository.library_ids_for_book(tx, book_id).await)
    }

    async fn remove_book_from_library(&self, library_token: LibraryToken, book_token: BookToken) -> Result<(), Error> {
        with_transaction!(self, library_repository, book_repository, |tx| {
            let library = library_repository
                .find_by_token(tx, library_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if library.is_system {
                return Err(Error::Validation("Cannot remove a book from a system library".into()));
            }

            let book = book_repository
                .find_by_token(tx, book_token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            library_repository.remove_book_from_library(tx, library.id, book.id).await
        })
    }

    async fn find_library_by_token(&self, token: LibraryToken) -> Result<Option<Library>, Error> {
        with_read_only_transaction!(self, library_repository, |tx| library_repository.find_by_token(tx, token).await)
    }

    async fn validate_user_library_access(&self, user_id: UserId, library_id: LibraryId) -> Result<(), Error> {
        let has = with_read_only_transaction!(self, library_repository, |tx| library_repository
            .user_has_library(tx, user_id, library_id)
            .await)?;
        if !has {
            return Err(Error::Validation("User is not assigned to this library".into()));
        }
        Ok(())
    }

    async fn create_personal_library_for_user(&self, user_id: UserId, library_name: String) -> Result<Library, Error> {
        let library = with_transaction!(self, library_repository, shelf_repository, user_book_metadata_repository, |tx| {
            // 1. Create library
            let library = library_repository
                .create_library(
                    tx,
                    NewLibrary {
                        name: library_name,
                        owner_id: Some(user_id),
                    },
                )
                .await?;

            // 2. Assign user
            library_repository.assign_user_to_library(tx, user_id, library.id).await?;

            // 3. Re-parent user's shelves from All Books to new library
            library_repository
                .reparent_shelves_for_user(tx, user_id, ALL_BOOKS_LIBRARY_ID, library.id)
                .await?;

            // 4. Seed with books the user has a meaningful relationship with:
            //    books on any of their shelves, plus books they have metadata
            //    for (read status, rating, notes). add_book_to_library is
            //    idempotent so the union is naturally deduplicated.
            let shelf_book_ids = shelf_repository.book_ids_for_user(tx, user_id).await?;
            let metadata_book_ids = user_book_metadata_repository.book_ids_for_user(tx, user_id).await?;

            let mut all_book_ids: std::collections::HashSet<crate::book::BookId> = shelf_book_ids.into_iter().collect();
            all_book_ids.extend(metadata_book_ids);

            for book_id in all_book_ids {
                library_repository.add_book_to_library(tx, library.id, book_id).await?;
            }

            Ok(library)
        })?;

        // 5. Set default library (outside transaction; user_setting_service
        //    handles its own transaction)
        let _ = self.user_setting_service.set(user_id, "default_library", &library.token.to_string()).await;

        Ok(library)
    }

    async fn find_personal_library_for_user(&self, user_id: UserId) -> Result<Option<Library>, Error> {
        with_read_only_transaction!(self, library_repository, |tx| library_repository.find_by_owner(tx, user_id).await)
    }

    async fn rename_library(&self, token: LibraryToken, new_name: String) -> Result<(), Error> {
        let new_name = new_name.trim().to_string();
        if new_name.is_empty() {
            return Err(Error::Validation("Library name must not be empty".into()));
        }
        with_transaction!(self, library_repository, |tx| {
            let lib = library_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
            if lib.is_system {
                return Err(Error::Validation("Cannot rename a system library".into()));
            }
            library_repository.rename_library(tx, lib.id, new_name).await
        })
    }
}
