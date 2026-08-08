use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{DateTime, Utc};

use crate::{
    Error, RepositoryError,
    book::{BookFile, BookId, FileFormat, FileRole},
    device::{BookSyncEntry, Device, DeviceBook, DeviceId, DeviceToken, NewDevice, NewDeviceSyncLog, OnRemovalAction, SyncDiff, SyncStatus},
    filter::{BookFilter, FilterReadStatus, FilterRule, SetOp},
    reading::{
        ReadStatus,
        service::{apply_transition, default_state},
    },
    repository::RepositoryService,
    shelf::{NewShelf, Shelf, ShelfType},
    user::UserId,
    with_read_only_transaction, with_transaction,
};

// ── Trait ──────────────────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait DeviceService: Send + Sync {
    /// Returns all devices belonging to the given user.
    async fn list_devices_for_user(&self, user_id: UserId) -> Result<Vec<Device>, Error>;

    /// Returns a single device identified by token, verifying ownership.
    ///
    /// Returns `NotFound` if the token does not exist or belongs to another
    /// user.
    async fn get_device(&self, token: DeviceToken, user_id: UserId) -> Result<Device, Error>;

    /// Creates a new device and a companion private smart shelf (atomically).
    ///
    /// The companion shelf is named after the device and pre-configured with a
    /// `ReadStatus IncludesAny [Active]` filter — the standard "sync to device"
    /// filter. Returns the new device's token.
    async fn create_device(&self, owner_id: UserId, name: String, device_type: String, on_removal_action: OnRemovalAction) -> Result<DeviceToken, Error>;

    /// Updates a device's name and on-removal action.
    ///
    /// If the name changes the companion shelf (if any) is renamed to match
    /// within the same transaction.
    async fn update_device(&self, token: DeviceToken, name: String, on_removal_action: OnRemovalAction, user_id: UserId) -> Result<(), Error>;

    /// Deletes a device.
    ///
    /// When `delete_companion_shelf` is `true` the linked shelf is deleted
    /// first; otherwise it survives as a regular unlinked smart shelf (the FK
    /// is cleared automatically by `ON DELETE SET NULL`).
    async fn delete_device(&self, token: DeviceToken, delete_companion_shelf: bool, user_id: UserId) -> Result<(), Error>;

    /// Returns the companion shelf linked to the given device, if one exists.
    async fn get_companion_shelf(&self, device_id: DeviceId) -> Result<Option<Shelf>, Error>;

    /// Suggests a default name for a new device owned by the given user.
    ///
    /// Uses the first word of the user's `full_name` (falling back to `"My"`)
    /// to build `"{first}'s Device"`. Appends `" (N)"` when that name is
    /// already taken.
    async fn default_device_name(&self, owner_id: UserId) -> Result<String, Error>;

    /// Computes which books need to be added, upgraded, refreshed, or removed
    /// for a device sync page.
    ///
    /// Loads all books from the device's companion shelf and all current
    /// `DeviceBook` records, classifies each book, sorts by `book_id`, then
    /// applies the keyset cursor (`after_book_id`) to return at most
    /// `page_size` entries. Removals are included only on the first page
    /// (`after_book_id` is `None`).
    ///
    /// `since` should be `device.last_synced_at` for incremental syncs, or
    /// `None` for a full initial sync.
    async fn compute_sync_diff(
        &self,
        device_id: DeviceId,
        owner_id: UserId,
        since: Option<DateTime<Utc>>,
        after_book_id: Option<BookId>,
        page_size: u64,
    ) -> Result<SyncDiff, Error>;

    /// Applies a sync diff page: upserts `DeviceBook` records for new and
    /// upgraded books, deletes records for removed books, and writes a
    /// `DeviceSyncLog` entry. Updates `Device.last_synced_at` only on the
    /// final page (`!diff.has_more`).
    async fn apply_sync(&self, device_id: DeviceId, diff: &SyncDiff) -> Result<(), Error>;

    /// Looks up a device by its token without verifying ownership.
    ///
    /// Used by the Kobo sync extractor where the token itself is the
    /// authentication credential. Returns `None` if no device with that token
    /// exists.
    async fn find_device_by_token(&self, token: DeviceToken) -> Result<Option<Device>, Error>;

    /// Clears all `DeviceBook` records for a device, forcing a full resync on
    /// the next Kobo library sync. Also resets `last_synced_at` to `None`.
    ///
    /// Returns `NotFound` if the token does not exist or belongs to another
    /// user.
    async fn reset_device_sync(&self, token: DeviceToken, user_id: UserId) -> Result<(), Error>;

    /// Removes a single `DeviceBook` record for the given device and book.
    ///
    /// Called when the Kobo sends `DELETE /v1/library/{uuid}` to signal that
    /// the user removed the book from the device. Removing the record ensures
    /// the book is re-delivered as `New` on the next sync. Idempotent — returns
    /// `Ok(())` if the record did not exist.
    async fn remove_book_from_device(&self, device_id: DeviceId, book_id: BookId) -> Result<(), Error>;
}

// ── Implementation
// ─────────────────────────────────────────────────────────────

/// Classification of a shelf book during sync diff computation.
enum EntryKind {
    New,
    Upgraded,
    Refreshed,
}

/// Selects the best file to send to a Kobo device from a book's available
/// files. Priority: Enriched Kepub → Enriched Epub → Original Kepub →
/// Original Epub. Returns `None` if no suitable file is available.
fn select_best_file(files: &[BookFile]) -> Option<BookFile> {
    let candidates = [
        (FileFormat::Kepub, FileRole::Enriched),
        (FileFormat::Epub, FileRole::Enriched),
        (FileFormat::Kepub, FileRole::Original),
        (FileFormat::Epub, FileRole::Original),
    ];
    for (format, role) in &candidates {
        if let Some(f) = files.iter().find(|f| &f.format == format && &f.file_role == role) {
            return Some(f.clone());
        }
    }
    None
}

pub(crate) struct DeviceServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl DeviceServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

/// Builds the default companion-shelf filter: `ReadStatus IncludesAny
/// [Active]`.
fn device_shelf_filter() -> BookFilter {
    BookFilter::Rule(FilterRule::ReadStatus {
        op: SetOp::IncludesAny,
        values: vec![FilterReadStatus::Active],
    })
}

#[async_trait::async_trait]
impl DeviceService for DeviceServiceImpl {
    async fn list_devices_for_user(&self, user_id: UserId) -> Result<Vec<Device>, Error> {
        with_read_only_transaction!(self, device_repository, |tx| device_repository.list_for_user(tx, user_id).await)
    }

    async fn get_device(&self, token: DeviceToken, user_id: UserId) -> Result<Device, Error> {
        with_read_only_transaction!(self, device_repository, |tx| {
            let device = device_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if device.owner_id != user_id {
                return Err(Error::RepositoryError(RepositoryError::NotFound));
            }

            Ok(device)
        })
    }

    async fn create_device(&self, owner_id: UserId, name: String, device_type: String, on_removal_action: OnRemovalAction) -> Result<DeviceToken, Error> {
        if name.trim().is_empty() {
            return Err(Error::Validation("device name must not be empty".to_string()));
        }

        with_transaction!(self, device_repository, shelf_repository, user_setting_repository, library_repository, |tx| {
            let device = device_repository
                .add_device(
                    tx,
                    NewDevice {
                        owner_id,
                        name: name.clone(),
                        device_type,
                        on_removal_action,
                    },
                )
                .await?;

            // Resolve the user's default library, falling back to All Books.
            let library_id = crate::library::resolve_user_default_library(tx, user_setting_repository.as_ref(), library_repository.as_ref(), owner_id).await?;

            shelf_repository
                .add_shelf(
                    tx,
                    NewShelf {
                        owner_id,
                        library_id,
                        name,
                        shelf_type: ShelfType::Smart,
                        device_id: Some(device.id),
                        filter_criteria: Some(device_shelf_filter()),
                    },
                )
                .await?;

            Ok(device.token)
        })
    }

    async fn update_device(&self, token: DeviceToken, name: String, on_removal_action: OnRemovalAction, user_id: UserId) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error::Validation("device name must not be empty".to_string()));
        }

        with_transaction!(self, device_repository, shelf_repository, |tx| {
            let device = device_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if device.owner_id != user_id {
                return Err(Error::Validation("only the owner may update a device".to_string()));
            }

            let name_changed = device.name != name;

            let updated = Device {
                name: name.clone(),
                on_removal_action,
                ..device.clone()
            };
            device_repository.update_device(tx, updated).await?;

            if name_changed && let Some(shelf) = shelf_repository.find_by_device_id(tx, device.id).await? {
                let renamed = Shelf { name, ..shelf };
                shelf_repository.update_shelf(tx, renamed).await?;
            }

            Ok(())
        })
    }

    async fn delete_device(&self, token: DeviceToken, delete_companion_shelf: bool, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, device_repository, shelf_repository, |tx| {
            let device = device_repository
                .find_by_token(tx, token)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            if device.owner_id != user_id {
                return Err(Error::Validation("only the owner may delete a device".to_string()));
            }

            if delete_companion_shelf && let Some(shelf) = shelf_repository.find_by_device_id(tx, device.id).await? {
                shelf_repository.delete_shelf(tx, shelf).await?;
            }

            device_repository.delete_device(tx, device).await
        })
    }

    async fn get_companion_shelf(&self, device_id: DeviceId) -> Result<Option<Shelf>, Error> {
        with_read_only_transaction!(self, shelf_repository, |tx| shelf_repository.find_by_device_id(tx, device_id).await)
    }

    async fn default_device_name(&self, owner_id: UserId) -> Result<String, Error> {
        with_read_only_transaction!(self, user_repository, device_repository, |tx| {
            let user = user_repository
                .find_by_id(tx, owner_id)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            let first_word = user.full_name.split_whitespace().next().unwrap_or("My").to_string();

            let base = format!("{first_word}'s Device");

            let count = device_repository.count_with_name_prefix(tx, owner_id, &base).await?;

            if count == 0 { Ok(base) } else { Ok(format!("{base} ({})", count + 1)) }
        })
    }

    async fn compute_sync_diff(
        &self,
        device_id: DeviceId,
        owner_id: UserId,
        since: Option<DateTime<Utc>>,
        after_book_id: Option<BookId>,
        page_size: u64,
    ) -> Result<SyncDiff, Error> {
        with_read_only_transaction!(self, shelf_repository, collection_repository, book_repository, device_repository, |tx| {
            // 1. Find companion shelf — no shelf means nothing to sync
            let Some(companion_shelf) = shelf_repository.find_by_device_id(tx, device_id).await? else {
                tracing::debug!(device_id, "no companion shelf found, returning empty diff");
                return Ok(SyncDiff::empty());
            };
            let Some(ref filter) = companion_shelf.filter_criteria else {
                tracing::warn!(device_id, shelf_id = companion_shelf.id, "companion shelf has no filter criteria");
                return Ok(SyncDiff::empty());
            };

            // 2. Load all shelf books with no page limit, then sort by book_id for
            //    deterministic keyset pagination
            let mut shelf_books = collection_repository
                .books_for_filter(tx, filter, owner_id, Some(companion_shelf.library_id), None, None, None)
                .await?;
            shelf_books.sort_by_key(|b| b.id);

            // 3. Load all DeviceBook records for quick lookup
            let device_books = device_repository.books_for_device(tx, device_id).await?;
            let device_book_map: HashMap<BookId, DeviceBook> = device_books.iter().map(|db| (db.book_id, db.clone())).collect();

            // 4. Detect removals: books in DeviceBook that are no longer on the shelf. Only
            //    included on the first page to avoid duplicating them across pages.
            let shelf_id_set: HashSet<BookId> = shelf_books.iter().map(|b| b.id).collect();
            let removed_book_ids = if after_book_id.is_none() {
                device_books
                    .iter()
                    .filter(|db| !shelf_id_set.contains(&db.book_id))
                    .map(|db| db.book_id)
                    .collect()
            } else {
                vec![]
            };

            // 5. Classify each shelf book — N+1 files query accepted; fast indexed lookups
            //    on a personal library are negligible
            let mut classified: Vec<(EntryKind, BookSyncEntry)> = Vec::new();
            for book in &shelf_books {
                let files = book_repository.files_for_book(tx, book.id).await?;
                let Some(best) = select_best_file(&files) else {
                    tracing::debug!(
                        book_id = book.id,
                        title = %book.title,
                        "no suitable file available, skipping"
                    );
                    continue;
                };

                let kind = match device_book_map.get(&book.id) {
                    None => EntryKind::New,
                    Some(db) if db.format != best.format || db.file_role != best.file_role => EntryKind::Upgraded,
                    // No cursor (since = None) means full sync from scratch — re-deliver all
                    // existing books as Refreshed so nothing is skipped. Also re-deliver any
                    // book whose metadata has changed since the last sync.
                    Some(_) if since.is_none_or(|s| book.updated_at > s) => EntryKind::Refreshed,
                    Some(_) => continue, // unchanged — skip
                };

                tracing::info!(
                    book_id = book.id,
                    title = %book.title,
                    format = ?best.format,
                    file_role = ?best.file_role,
                    kind = match &kind {
                        EntryKind::New => "new",
                        EntryKind::Upgraded => "upgraded",
                        EntryKind::Refreshed => "refreshed",
                    },
                    "preparing book for kobo sync"
                );
                classified.push((
                    kind,
                    BookSyncEntry {
                        book: book.clone(),
                        file: best,
                    },
                ));
            }

            // 6. Apply keyset cursor and extract page
            let start = match after_book_id {
                None => 0,
                Some(id) => classified.partition_point(|(_, e)| e.book.id <= id),
            };
            let end = (start + page_size as usize).min(classified.len());
            let has_more = end < classified.len();

            // 7. Distribute page entries into their categories
            let mut new_books = Vec::new();
            let mut upgraded_books = Vec::new();
            let mut refreshed_books = Vec::new();
            for (kind, entry) in classified.drain(start..end) {
                match kind {
                    EntryKind::New => new_books.push(entry),
                    EntryKind::Upgraded => upgraded_books.push(entry),
                    EntryKind::Refreshed => refreshed_books.push(entry),
                }
            }

            Ok(SyncDiff {
                new_books,
                upgraded_books,
                refreshed_books,
                removed_book_ids,
                has_more,
            })
        })
    }

    async fn apply_sync(&self, device_id: DeviceId, diff: &SyncDiff) -> Result<(), Error> {
        let now = Utc::now();

        let new_count = diff.new_books.len();
        let upgraded_count = diff.upgraded_books.len();
        let refreshed_count = diff.refreshed_books.len();
        let removed_count = diff.removed_book_ids.len();
        let books_added = (new_count + upgraded_count + refreshed_count) as i32;
        let books_removed = removed_count as i32;
        let has_more = diff.has_more;

        // Clone entries for the async move closure (diff is a borrowed reference)
        let new_books = diff.new_books.clone();
        let upgraded_books = diff.upgraded_books.clone();
        let removed_book_ids = diff.removed_book_ids.clone();

        with_transaction!(self, device_repository, |tx| {
            // Add new DeviceBook records
            for entry in &new_books {
                device_repository
                    .add_device_book(
                        tx,
                        DeviceBook {
                            device_id,
                            book_id: entry.book.id,
                            format: entry.file.format.clone(),
                            file_role: entry.file.file_role.clone(),
                            synced_at: now,
                        },
                    )
                    .await?;
            }

            // Update DeviceBook records for upgraded books (different file)
            for entry in &upgraded_books {
                device_repository
                    .update_device_book(
                        tx,
                        DeviceBook {
                            device_id,
                            book_id: entry.book.id,
                            format: entry.file.format.clone(),
                            file_role: entry.file.file_role.clone(),
                            synced_at: now,
                        },
                    )
                    .await?;
            }

            // Remove DeviceBook records for books no longer on the companion shelf
            for book_id in &removed_book_ids {
                device_repository.remove_device_book(tx, device_id, *book_id).await?;
            }

            // Write a sync log entry for this page
            device_repository
                .add_sync_log(
                    tx,
                    NewDeviceSyncLog {
                        device_id,
                        status: SyncStatus::Completed,
                        books_added,
                        books_removed,
                        started_at: now,
                        completed_at: Some(now),
                    },
                )
                .await?;

            // On the final page, update the device's last_synced_at
            if !has_more {
                let device = device_repository
                    .find_by_id(tx, device_id)
                    .await?
                    .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

                device_repository
                    .update_device(
                        tx,
                        Device {
                            last_synced_at: Some(now),
                            ..device
                        },
                    )
                    .await?;
            }

            tracing::info!(
                device_id,
                new = new_count,
                upgraded = upgraded_count,
                refreshed = refreshed_count,
                removed = removed_count,
                has_more,
                "kobo sync page applied"
            );

            Ok(())
        })
    }

    async fn find_device_by_token(&self, token: DeviceToken) -> Result<Option<Device>, Error> {
        with_read_only_transaction!(self, device_repository, |tx| device_repository.find_by_token(tx, token).await)
    }

    async fn reset_device_sync(&self, token: DeviceToken, user_id: UserId) -> Result<(), Error> {
        with_transaction!(self, device_repository, |tx| {
            let device = device_repository
                .find_by_token(tx, token)
                .await?
                .filter(|d| d.owner_id == user_id)
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            // Resetting last_synced_at to None is enough: the library sync
            // handler treats None as a server-side override that ignores the
            // Kobo's cursor and forces a full re-classification on the next sync.
            // DeviceBook records are preserved so re-sent books are classified
            // as Refreshed (existing) or New (not yet sent) rather than all New.
            let updated = Device {
                last_synced_at: None,
                ..device
            };
            device_repository.update_device(tx, updated).await?;

            Ok(())
        })
    }

    async fn remove_book_from_device(&self, device_id: DeviceId, book_id: BookId) -> Result<(), Error> {
        with_transaction!(self, device_repository, user_book_metadata_repository, |tx| {
            let device = device_repository
                .find_by_id(tx, device_id)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            device_repository.remove_device_book(tx, device_id, book_id).await?;

            let target_status = match device.on_removal_action {
                OnRemovalAction::Nothing => return Ok(()),
                OnRemovalAction::MarkRead => ReadStatus::Read,
                OnRemovalAction::MarkDnf => ReadStatus::Abandoned,
            };

            let current = user_book_metadata_repository
                .find_by_user_and_book(tx, device.owner_id, book_id)
                .await?
                .unwrap_or_else(|| default_state(device.owner_id, book_id));
            let next = apply_transition(current, target_status);
            user_book_metadata_repository.upsert(tx, next).await?;

            Ok(())
        })
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        book::{Book, BookStatus, repository::book::MockBookRepository},
        collection::MockCollectionRepository,
        device::repository::device::MockDeviceRepository,
        library::MockLibraryRepository,
        shelf::{ShelfToken, repository::shelf::MockShelfRepository},
        user::{
            User, UserToken,
            repository::{user::MockUserRepository, user_settings::MockUserSettingRepository},
        },
    };

    // ─── Helpers ──────────────────────────────────────────────────────────────

    fn create_service(device_repo: MockDeviceRepository, shelf_repo: MockShelfRepository, user_repo: MockUserRepository) -> DeviceServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .user_repository(Arc::new(user_repo))
                .shelf_repository(Arc::new(shelf_repo))
                .device_repository(Arc::new(device_repo))
                .build()
                .expect("all fields provided"),
        );
        DeviceServiceImpl::new(repository_service)
    }

    fn create_service_for_create_device(
        device_repo: MockDeviceRepository,
        shelf_repo: MockShelfRepository,
        setting_repo: MockUserSettingRepository,
        library_repo: MockLibraryRepository,
    ) -> DeviceServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .shelf_repository(Arc::new(shelf_repo))
                .device_repository(Arc::new(device_repo))
                .user_setting_repository(Arc::new(setting_repo))
                .library_repository(Arc::new(library_repo))
                .build()
                .expect("all fields provided"),
        );
        DeviceServiceImpl::new(repository_service)
    }

    fn fake_device(owner_id: UserId) -> Device {
        Device {
            id: 1,
            version: 1,
            token: DeviceToken::new(1),
            owner_id,
            name: "My Device".to_string(),
            device_type: "kobo".to_string(),
            on_removal_action: OnRemovalAction::Nothing,
            last_synced_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn fake_shelf(owner_id: UserId, device_id: Option<DeviceId>) -> Shelf {
        Shelf {
            id: 10,
            version: 1,
            token: ShelfToken::new(10),
            owner_id,
            library_id: crate::library::ALL_BOOKS_LIBRARY_ID,
            name: "My Device".to_string(),
            shelf_type: ShelfType::Smart,
            device_id,
            filter_criteria: Some(device_shelf_filter()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn fake_user(id: UserId, full_name: &str) -> User {
        User {
            id,
            version: 1,
            token: UserToken::new(id),
            username: "test".to_string(),
            full_name: full_name.to_string(),
            password_hash: String::new(),
            email_address: crate::types::EmailAddress::new("test@example.com").unwrap(),
            capabilities: Default::default(),
            change_password_on_login: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_sync_service(
        device_repo: MockDeviceRepository,
        shelf_repo: MockShelfRepository,
        collection_repo: MockCollectionRepository,
        book_repo: MockBookRepository,
    ) -> DeviceServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .collection_repository(Arc::new(collection_repo))
                .shelf_repository(Arc::new(shelf_repo))
                .device_repository(Arc::new(device_repo))
                .build()
                .expect("all fields provided"),
        );
        DeviceServiceImpl::new(repository_service)
    }

    fn fake_book_file(book_id: BookId, format: FileFormat, file_role: FileRole) -> BookFile {
        BookFile {
            book_id,
            format,
            file_role,
            path: String::new(),
            file_size: 0,
            file_hash: String::new(),
            created_at: chrono::Utc::now(),
        }
    }

    fn fake_device_book(device_id: DeviceId, book_id: BookId, format: FileFormat, file_role: FileRole) -> DeviceBook {
        DeviceBook {
            device_id,
            book_id,
            format,
            file_role,
            synced_at: Utc::now(),
        }
    }

    // ─── Helper: build a MockBookRepository with per-book file maps ────────────

    fn book_repo_with_files(file_map: std::collections::HashMap<BookId, Vec<BookFile>>) -> MockBookRepository {
        let mut repo = MockBookRepository::new();
        repo.expect_files_for_book().returning(move |_, book_id| {
            let files = file_map.get(&book_id).cloned().unwrap_or_default();
            Box::pin(async move { Ok(files) })
        });
        repo
    }

    // ─── list_devices_for_user ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_devices_returns_devices() {
        let device = fake_device(1);
        let device_id = device.id;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_list_for_user().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(vec![d]) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.list_devices_for_user(1).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, device_id);
    }

    // ─── get_device ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_device_success() {
        let device = fake_device(1);
        let token = device.token;
        let device_id = device.id;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.get_device(token, 1).await.unwrap();

        assert_eq!(result.id, device_id);
    }

    #[tokio::test]
    async fn test_get_device_not_found() {
        let token = DeviceToken::new(99);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.get_device(token, 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_get_device_wrong_owner_returns_not_found() {
        let device = fake_device(1); // owned by user 1
        let token = device.token;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.get_device(token, 2).await; // user 2 requests

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── create_device ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_device_returns_token() {
        let device = fake_device(1);
        let expected_token = device.token;
        let shelf = fake_shelf(1, Some(device.id));
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_add_device().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(d) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_add_shelf().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let mut setting_repo = MockUserSettingRepository::new();
        setting_repo.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        let svc = create_service_for_create_device(device_repo, shelf_repo, setting_repo, MockLibraryRepository::new());

        let result = svc
            .create_device(1, "My Device".to_string(), "kobo".to_string(), OnRemovalAction::Nothing)
            .await;

        assert_eq!(result.unwrap(), expected_token);
    }

    #[tokio::test]
    async fn test_create_device_empty_name_returns_validation_error() {
        let svc = create_service(MockDeviceRepository::new(), MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.create_device(1, "  ".to_string(), "kobo".to_string(), OnRemovalAction::Nothing).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── update_device ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_device_renames_companion_shelf() {
        let device = fake_device(1);
        let token = device.token;
        let shelf = fake_shelf(1, Some(device.id));
        let updated_device = Device {
            name: "New Name".to_string(),
            ..device.clone()
        };
        let updated_shelf = Shelf {
            name: "New Name".to_string(),
            ..shelf.clone()
        };

        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        device_repo.expect_update_device().returning(move |_, _| {
            let d = updated_device.clone();
            Box::pin(async move { Ok(d) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_update_shelf().returning(move |_, _| {
            let s = updated_shelf.clone();
            Box::pin(async move { Ok(s) })
        });
        let svc = create_service(device_repo, shelf_repo, MockUserRepository::new());

        let result = svc.update_device(token, "New Name".to_string(), OnRemovalAction::Nothing, 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_update_device_same_name_skips_shelf_rename() {
        let device = fake_device(1);
        let token = device.token;
        let updated_device = device.clone();

        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        device_repo.expect_update_device().returning(move |_, _| {
            let d = updated_device.clone();
            Box::pin(async move { Ok(d) })
        });
        // shelf_repo has no expectations — it must not be called
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        // Same name as current — shelf rename should not be attempted
        let result = svc.update_device(token, "My Device".to_string(), OnRemovalAction::MarkRead, 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_update_device_wrong_owner_returns_validation_error() {
        let device = fake_device(1);
        let token = device.token;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.update_device(token, "New Name".to_string(), OnRemovalAction::Nothing, 2).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── delete_device ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_device_with_companion_shelf_deletion() {
        let device = fake_device(1);
        let token = device.token;
        let shelf = fake_shelf(1, Some(device.id));

        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        device_repo.expect_delete_device().returning(|_, _| Box::pin(async { Ok(()) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(move |_, _| {
            let s = shelf.clone();
            Box::pin(async move { Ok(Some(s)) })
        });
        shelf_repo.expect_delete_shelf().returning(|_, _| Box::pin(async { Ok(()) }));
        let svc = create_service(device_repo, shelf_repo, MockUserRepository::new());

        let result = svc.delete_device(token, true, 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_delete_device_without_shelf_deletion() {
        let device = fake_device(1);
        let token = device.token;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        device_repo.expect_delete_device().returning(|_, _| Box::pin(async { Ok(()) }));
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.delete_device(token, false, 1).await;

        result.unwrap();
    }

    #[tokio::test]
    async fn test_delete_device_wrong_owner_returns_validation_error() {
        let device = fake_device(1);
        let token = device.token;
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_find_by_token().returning(move |_, _| {
            let d = device.clone();
            Box::pin(async move { Ok(Some(d)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), MockUserRepository::new());

        let result = svc.delete_device(token, false, 2).await;

        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── default_device_name ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_default_device_name_no_collision() {
        let user = fake_user(1, "Alice Smith");
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_count_with_name_prefix().returning(|_, _, _| Box::pin(async { Ok(0) }));
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let u = user.clone();
            Box::pin(async move { Ok(Some(u)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), user_repo);

        let name = svc.default_device_name(1).await.unwrap();

        assert_eq!(name, "Alice's Device");
    }

    #[tokio::test]
    async fn test_default_device_name_with_collision() {
        let user = fake_user(1, "Alice Smith");
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_count_with_name_prefix().returning(|_, _, _| Box::pin(async { Ok(1) }));
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let u = user.clone();
            Box::pin(async move { Ok(Some(u)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), user_repo);

        let name = svc.default_device_name(1).await.unwrap();

        assert_eq!(name, "Alice's Device (2)");
    }

    #[tokio::test]
    async fn test_default_device_name_empty_full_name_falls_back_to_my() {
        let user = fake_user(1, "");
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_count_with_name_prefix().returning(|_, _, _| Box::pin(async { Ok(0) }));
        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_by_id().returning(move |_, _| {
            let u = user.clone();
            Box::pin(async move { Ok(Some(u)) })
        });
        let svc = create_service(device_repo, MockShelfRepository::new(), user_repo);

        let name = svc.default_device_name(1).await.unwrap();

        assert_eq!(name, "My's Device");
    }

    // ─── compute_sync_diff ────────────────────────────────────────────────────

    fn sync_shelf() -> Shelf {
        fake_shelf(1, Some(1))
    }

    #[tokio::test]
    async fn test_sync_diff_empty_shelf_returns_empty_diff() {
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(|_, _, _, _, _, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, MockBookRepository::new());

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert!(diff.is_empty());
        assert!(!diff.has_more);
    }

    #[tokio::test]
    async fn test_compute_sync_diff_scopes_to_shelf_library() {
        // Shelf has a non-default library_id (42) — verifies compute_sync_diff passes
        // the shelf's library_id rather than always passing ALL_BOOKS_LIBRARY_ID.
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = Shelf {
                id: 10,
                version: 1,
                token: ShelfToken::new(10),
                owner_id: 1,
                library_id: 42,
                name: "Personal Shelf".to_string(),
                shelf_type: ShelfType::Smart,
                device_id: Some(1),
                filter_criteria: Some(device_shelf_filter()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(42u64))
            .returning(|_, _, _, _, _, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, MockBookRepository::new());

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert!(diff.is_empty());
        assert!(!diff.has_more);
    }

    #[tokio::test]
    async fn test_compute_sync_diff_returns_all_rows_for_large_smart_shelf() {
        // BB-18 reproducer: 75 books on a smart-shelf must all surface in the
        // sync diff. Before the adapter pagination fix (caller-owned page size),
        // CollectionRepositoryAdapter::books_for_filter silently clamped to 50,
        // so a shelf with 51+ books would lose entries from the sync. This test
        // mocks the repository directly to return 75 books and asserts that
        // every one of them appears in diff.new_books — exercising the service
        // contract that the adapter pagination must honour.
        let books: Vec<Book> = (1u64..=75).map(|i| Book::fake(i, format!("Book {i}"), BookStatus::Available)).collect();
        let file_map: std::collections::HashMap<BookId, Vec<BookFile>> = (1u64..=75)
            .map(|i| (i, vec![fake_book_file(i, FileFormat::Epub, FileRole::Original)]))
            .collect();

        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = books.clone();
                Box::pin(async move { Ok(b) })
            });
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        // page_size=100 fits all 75 in a single page — no paging artefacts.
        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert_eq!(diff.new_books.len(), 75, "every shelf book must surface in the diff — no silent 50-cap");
        assert!(!diff.has_more);
        assert_eq!(diff.new_books.first().unwrap().book.id, 1);
        assert_eq!(diff.new_books.last().unwrap().book.id, 75);
        assert!(diff.upgraded_books.is_empty());
        assert!(diff.refreshed_books.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_no_companion_shelf_returns_empty() {
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_sync_service(
            MockDeviceRepository::new(),
            shelf_repo,
            MockCollectionRepository::new(),
            MockBookRepository::new(),
        );

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert!(diff.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_book_without_files_is_skipped() {
        let book = Book::fake(1, "No Files", BookStatus::Available);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        // no files configured → returns empty
        let book_repo = book_repo_with_files(std::collections::HashMap::new());
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo);

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert!(diff.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_new_books_sorted_by_id() {
        // Shelf returns books out of order; diff must be sorted by book_id
        let books = vec![
            Book::fake(3, "C", BookStatus::Available),
            Book::fake(1, "A", BookStatus::Available),
            Book::fake(2, "B", BookStatus::Available),
        ];
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();
        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = books.clone();
                Box::pin(async move { Ok(b) })
            });
        let file_map = [
            (1u64, vec![fake_book_file(1, FileFormat::Epub, FileRole::Original)]),
            (2u64, vec![fake_book_file(2, FileFormat::Epub, FileRole::Original)]),
            (3u64, vec![fake_book_file(3, FileFormat::Epub, FileRole::Original)]),
        ]
        .into_iter()
        .collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert_eq!(diff.new_books.len(), 3);
        assert_eq!(diff.new_books[0].book.id, 1);
        assert_eq!(diff.new_books[1].book.id, 2);
        assert_eq!(diff.new_books[2].book.id, 3);
        assert!(diff.upgraded_books.is_empty());
        assert!(diff.refreshed_books.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_best_file_selection_prefers_enriched_kepub() {
        let book = Book::fake(1, "Book", BookStatus::Available);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        let file_map = [(
            1u64,
            vec![
                fake_book_file(1, FileFormat::Epub, FileRole::Original),
                fake_book_file(1, FileFormat::Epub, FileRole::Enriched),
                fake_book_file(1, FileFormat::Kepub, FileRole::Enriched),
            ],
        )]
        .into_iter()
        .collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert_eq!(diff.new_books.len(), 1);
        assert_eq!(diff.new_books[0].file.format, FileFormat::Kepub);
        assert_eq!(diff.new_books[0].file.file_role, FileRole::Enriched);
    }

    #[tokio::test]
    async fn test_sync_diff_upgraded_book_when_better_file_available() {
        let book = Book::fake(1, "Book", BookStatus::Available);
        // Device has Original Epub; now Enriched Epub is available → upgraded
        let device_book = fake_device_book(1, 1, FileFormat::Epub, FileRole::Original);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(move |_, _| {
            let db = device_book.clone();
            Box::pin(async move { Ok(vec![db]) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        let file_map = [(
            1u64,
            vec![
                fake_book_file(1, FileFormat::Epub, FileRole::Original),
                fake_book_file(1, FileFormat::Epub, FileRole::Enriched),
            ],
        )]
        .into_iter()
        .collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();

        assert!(diff.new_books.is_empty());
        assert_eq!(diff.upgraded_books.len(), 1);
        assert_eq!(diff.upgraded_books[0].file.file_role, FileRole::Enriched);
    }

    #[tokio::test]
    async fn test_sync_diff_refreshed_book_when_metadata_updated() {
        let updated_at = DateTime::from_timestamp(1000, 0).unwrap();
        let since = DateTime::from_timestamp(500, 0).unwrap();
        let book = Book {
            updated_at,
            ..Book::fake(1, "Book", BookStatus::Available)
        };
        // Device has same file, but book.updated_at > since → refreshed
        let device_book = fake_device_book(1, 1, FileFormat::Epub, FileRole::Original);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(move |_, _| {
            let db = device_book.clone();
            Box::pin(async move { Ok(vec![db]) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        let file_map = [(1u64, vec![fake_book_file(1, FileFormat::Epub, FileRole::Original)])].into_iter().collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        let diff = svc.compute_sync_diff(1, 1, Some(since), None, 100).await.unwrap();

        assert!(diff.new_books.is_empty());
        assert!(diff.upgraded_books.is_empty());
        assert_eq!(diff.refreshed_books.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_diff_unchanged_book_is_skipped() {
        let updated_at = DateTime::from_timestamp(500, 0).unwrap();
        let since = DateTime::from_timestamp(1000, 0).unwrap();
        let book = Book {
            updated_at,
            ..Book::fake(1, "Book", BookStatus::Available)
        };
        let device_book = fake_device_book(1, 1, FileFormat::Epub, FileRole::Original);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(move |_, _| {
            let db = device_book.clone();
            Box::pin(async move { Ok(vec![db]) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        let file_map = [(1u64, vec![fake_book_file(1, FileFormat::Epub, FileRole::Original)])].into_iter().collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        let diff = svc.compute_sync_diff(1, 1, Some(since), None, 100).await.unwrap();

        assert!(diff.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_removals_only_on_first_page() {
        let book = Book::fake(1, "On Shelf", BookStatus::Available);
        // DeviceBook for book_id=2 is no longer on the shelf → should appear as removal
        let stale_book = fake_device_book(1, 2, FileFormat::Epub, FileRole::Original);
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(move |_, _| {
            let db = stale_book.clone();
            Box::pin(async move { Ok(vec![db]) })
        });
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = book.clone();
                Box::pin(async move { Ok(vec![b]) })
            });
        let file_map = [(1u64, vec![fake_book_file(1, FileFormat::Epub, FileRole::Original)])].into_iter().collect();
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        // First page (after_book_id = None): removals included
        let diff_first = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();
        assert_eq!(diff_first.removed_book_ids, vec![2]);

        // Subsequent page (after_book_id = Some(...)): no removals
        let diff_second = svc.compute_sync_diff(1, 1, None, Some(0), 100).await.unwrap();
        assert!(diff_second.removed_book_ids.is_empty());
    }

    #[tokio::test]
    async fn test_sync_diff_paging_150_books_has_more_on_first_page() {
        // 150 books (ids 1-150), all new, page_size=100
        let books: Vec<Book> = (1u64..=150).map(|i| Book::fake(i, format!("Book {i}"), BookStatus::Available)).collect();
        let file_map: std::collections::HashMap<BookId, Vec<BookFile>> = (1u64..=150)
            .map(|i| (i, vec![fake_book_file(i, FileFormat::Epub, FileRole::Original)]))
            .collect();
        let mut device_repo = MockDeviceRepository::new();
        device_repo.expect_books_for_device().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let mut shelf_repo = MockShelfRepository::new();
        shelf_repo.expect_find_by_device_id().returning(|_, _| {
            let s = sync_shelf();
            Box::pin(async move { Ok(Some(s)) })
        });
        let mut collection_repo = MockCollectionRepository::new();

        collection_repo
            .expect_books_for_filter()
            .withf(|_, _, _, library_id, _, _, _| *library_id == Some(crate::library::ALL_BOOKS_LIBRARY_ID))
            .returning(move |_, _, _, _, _, _, _| {
                let b = books.clone();
                Box::pin(async move { Ok(b) })
            });
        let svc = create_sync_service(device_repo, shelf_repo, collection_repo, book_repo_with_files(file_map));

        // First page
        let diff = svc.compute_sync_diff(1, 1, None, None, 100).await.unwrap();
        assert_eq!(diff.new_books.len(), 100);
        assert!(diff.has_more);
        assert_eq!(diff.new_books.first().unwrap().book.id, 1);
        assert_eq!(diff.new_books.last().unwrap().book.id, 100);

        // Second page
        let diff2 = svc.compute_sync_diff(1, 1, None, Some(100), 100).await.unwrap();
        assert_eq!(diff2.new_books.len(), 50);
        assert!(!diff2.has_more);
        assert_eq!(diff2.new_books.first().unwrap().book.id, 101);
        assert_eq!(diff2.new_books.last().unwrap().book.id, 150);
    }
}
