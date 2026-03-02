use bb_core::{
    Error, RepositoryError,
    book::{Author, AuthorId, AuthorRepository, AuthorToken, NewAuthor},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{authors, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<authors::Model> for Author {
    fn from(model: authors::Model) -> Self {
        let token = AuthorToken::new(model.id as u64);
        Self {
            id: model.id as u64,
            version: model.version as u64,
            token,
            name: model.name,
            bio: model.bio,
            created_at: model.created_at.with_timezone(&Utc),
            updated_at: model.updated_at.with_timezone(&Utc),
        }
    }
}

pub(crate) struct AuthorRepositoryAdapter;

impl AuthorRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl AuthorRepository for AuthorRepositoryAdapter {
    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn add_author(&self, transaction: &dyn Transaction, author: NewAuthor) -> Result<Author, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = AuthorToken::generate();
        let now = Utc::now();

        let model = authors::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(author.name),
            bio: Set(author.bio),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn update_author(&self, transaction: &dyn Transaction, author: Author) -> Result<Author, Error> {
        if author.id == 0 {
            return Err(Error::InvalidId(author.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Authors::find_by_id(author.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != author.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: authors::ActiveModel = existing.clone().into();

        if existing.name != author.name {
            updater.name = Set(author.name);
        }
        if existing.bio != author.bio {
            updater.bio = Set(author.bio);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_id(&self, transaction: &dyn Transaction, id: AuthorId) -> Result<Option<Author>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Authors::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &AuthorToken) -> Result<Option<Author>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn list_authors(&self, transaction: &dyn Transaction, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Authors::find().order_by_asc(authors::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(authors::Column::Id.gte(start_id as i64));
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
        book::{Author, AuthorRepository, NewAuthor},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ─── add_author ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_author_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Ursula K. Le Guin".into(),
                    bio: None,
                },
            )
            .await;

        assert!(result.is_ok());
        let author = result.unwrap();
        assert_ne!(author.id, 0);
        assert_eq!(author.name, "Ursula K. Le Guin");
        assert!(author.bio.is_none());
        assert_eq!(author.token.id(), author.id);
    }

    #[tokio::test]
    async fn test_add_author_with_bio() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let bio = Some("American author of speculative fiction".to_string());
        let author = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Ursula K. Le Guin".into(),
                    bio: bio.clone(),
                },
            )
            .await
            .unwrap();

        assert_eq!(author.bio, bio);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Frank Herbert".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        let result = svc.author_repository().find_by_id(&*tx, inserted.id).await;

        assert!(result.is_ok());
        let author = result.unwrap().unwrap();
        assert_eq!(author.id, inserted.id);
        assert_eq!(author.name, "Frank Herbert");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.author_repository().find_by_id(&*tx, 999).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.author_repository().find_by_id(&*tx, 0).await;

        assert!(matches!(result, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Frank Herbert".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        let result = svc.author_repository().find_by_token(&*tx, &inserted.token).await;

        assert!(result.is_ok());
        let author = result.unwrap().unwrap();
        assert_eq!(author.id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        use bb_core::book::AuthorToken;
        let token = AuthorToken::new(999);
        let result = svc.author_repository().find_by_token(&*tx, &token).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── list_authors ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_authors_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.author_repository().list_authors(&*tx, None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_authors_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author A".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();
        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author B".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        let result = svc.author_repository().list_authors(&*tx, None, None).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_authors_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author A".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();
        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author B".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        let all = svc.author_repository().list_authors(&*tx, None, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let result = svc.author_repository().list_authors(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_authors_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.author_repository().list_authors(&*tx, None, Some(0)).await;

        assert!(matches!(result, Err(Error::InvalidPageSize(0))));
    }

    // ─── update_author ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_author_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut author = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Old Name".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        author.name = "New Name".into();
        let result = svc.author_repository().update_author(&*tx, author).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "New Name");
    }

    #[tokio::test]
    async fn test_update_author_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut author = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        let version_before = author.version;
        author.name = "Updated".into();
        let updated = svc.author_repository().update_author(&*tx, author).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_author_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let author = Author {
            id: 999,
            version: 0,
            token: bb_core::book::AuthorToken::new(999),
            name: "Ghost".into(),
            bio: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let result = svc.author_repository().update_author(&*tx, author).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    #[tokio::test]
    async fn test_update_author_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut author = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        author.version = 99;
        author.name = "Updated".into();
        let result = svc.author_repository().update_author(&*tx, author).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    #[tokio::test]
    async fn test_update_author_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let author = Author {
            id: 0,
            version: 0,
            token: bb_core::book::AuthorToken::new(1),
            name: "Invalid".into(),
            bio: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let result = svc.author_repository().update_author(&*tx, author).await;

        assert!(matches!(result, Err(Error::InvalidId(0))));
    }
}
