use bb_core::{
    Error, RepositoryError,
    book::{NewTag, Tag, TagId, TagRepository, TagToken},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{prelude, tags},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<tags::Model> for Tag {
    fn from(model: tags::Model) -> Self {
        let token = TagToken::new(model.id as u64);
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

pub(crate) struct TagRepositoryAdapter;

impl TagRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl TagRepository for TagRepositoryAdapter {
    async fn add_tag(&self, transaction: &dyn Transaction, tag: NewTag) -> Result<Tag, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = TagToken::generate();
        let now = Utc::now();

        let model = tags::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            name: Set(tag.name),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_tag(&self, transaction: &dyn Transaction, tag: Tag) -> Result<Tag, Error> {
        if tag.id == 0 {
            return Err(Error::InvalidId(tag.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Tags::find_by_id(tag.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != tag.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: tags::ActiveModel = existing.clone().into();

        if existing.name != tag.name {
            updater.name = Set(tag.name);
        }

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: TagId) -> Result<Option<Tag>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Tags::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: &TagToken) -> Result<Option<Tag>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Tag>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Tags::find()
            .filter(tags::Column::Name.eq(name))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_tags(&self, transaction: &dyn Transaction, start_id: Option<TagId>, page_size: Option<u64>) -> Result<Vec<Tag>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Tags::find().order_by_asc(tags::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(tags::Column::Id.gte(start_id as i64));
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
        book::{NewTag, Tag, TagRepository, TagToken},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    // ─── add_tag ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_tag_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.tag_repository().add_tag(&*tx, NewTag { name: "space-opera".into() }).await;

        assert!(result.is_ok());
        let t = result.unwrap();
        assert_ne!(t.id, 0);
        assert_eq!(t.name, "space-opera");
        assert_eq!(t.token.id(), t.id);
    }

    #[tokio::test]
    async fn test_add_tag_duplicate_name_fails() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.tag_repository().add_tag(&*tx, NewTag { name: "dystopia".into() }).await.unwrap();
        let result = svc.tag_repository().add_tag(&*tx, NewTag { name: "dystopia".into() }).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.tag_repository().add_tag(&*tx, NewTag { name: "dystopia".into() }).await.unwrap();
        let result = svc.tag_repository().find_by_id(&*tx, inserted.id).await;

        assert_eq!(result.unwrap().unwrap().name, "dystopia");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.tag_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.tag_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.tag_repository().add_tag(&*tx, NewTag { name: "dystopia".into() }).await.unwrap();
        let result = svc.tag_repository().find_by_token(&*tx, &inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.tag_repository().find_by_token(&*tx, &TagToken::new(999)).await.unwrap().is_none());
    }

    // ─── find_by_name ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_name_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.tag_repository().add_tag(&*tx, NewTag { name: "cyberpunk".into() }).await.unwrap();
        let result = svc.tag_repository().find_by_name(&*tx, "cyberpunk").await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_name_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.tag_repository().find_by_name(&*tx, "nonexistent").await.unwrap().is_none());
    }

    // ─── list_tags ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_tags_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.tag_repository().list_tags(&*tx, None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_tags_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.tag_repository().add_tag(&*tx, NewTag { name: "tag-a".into() }).await.unwrap();
        svc.tag_repository().add_tag(&*tx, NewTag { name: "tag-b".into() }).await.unwrap();

        assert_eq!(svc.tag_repository().list_tags(&*tx, None, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_tags_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.tag_repository().add_tag(&*tx, NewTag { name: "tag-a".into() }).await.unwrap();
        svc.tag_repository().add_tag(&*tx, NewTag { name: "tag-b".into() }).await.unwrap();

        let all = svc.tag_repository().list_tags(&*tx, None, None).await.unwrap();
        let result = svc.tag_repository().list_tags(&*tx, Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_tags_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.tag_repository().list_tags(&*tx, None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── update_tag ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_tag_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut t = svc.tag_repository().add_tag(&*tx, NewTag { name: "old-tag".into() }).await.unwrap();
        t.name = "new-tag".into();
        let updated = svc.tag_repository().update_tag(&*tx, t).await.unwrap();

        assert_eq!(updated.name, "new-tag");
    }

    #[tokio::test]
    async fn test_update_tag_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut t = svc.tag_repository().add_tag(&*tx, NewTag { name: "tag".into() }).await.unwrap();
        let version_before = t.version;
        t.name = "updated-tag".into();
        let updated = svc.tag_repository().update_tag(&*tx, t).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_tag_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let t = Tag {
            id: 999,
            version: 0,
            token: TagToken::new(999),
            name: "ghost".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(
            svc.tag_repository().update_tag(&*tx, t).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_tag_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut t = svc.tag_repository().add_tag(&*tx, NewTag { name: "tag".into() }).await.unwrap();
        t.version = 99;
        t.name = "updated".into();

        assert!(matches!(
            svc.tag_repository().update_tag(&*tx, t).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_tag_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let t = Tag {
            id: 0,
            version: 0,
            token: TagToken::new(1),
            name: "invalid".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(svc.tag_repository().update_tag(&*tx, t).await, Err(Error::InvalidId(0))));
    }
}
