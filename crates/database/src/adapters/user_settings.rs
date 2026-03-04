use bb_core::{
    Error, RepositoryError,
    repository::Transaction,
    user::{NewUserSetting, UserId, UserSetting, UserSettingRepository},
};
use chrono::Utc;
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
    entities::{prelude, user_settings},
    error::handle_dberr,
    transaction::TransactionImpl,
};

impl From<user_settings::Model> for UserSetting {
    fn from(model: user_settings::Model) -> Self {
        Self {
            user_id: model.user_id as u64,
            key: model.key,
            value: model.value,
        }
    }
}

pub(crate) struct UserSettingRepositoryAdapter;

impl UserSettingRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl UserSettingRepository for UserSettingRepositoryAdapter {
    async fn get(&self, transaction: &dyn Transaction, user_id: UserId, key: &str) -> Result<Option<UserSetting>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::UserSettings::find_by_id((user_id as i64, key.to_owned()))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn set(&self, transaction: &dyn Transaction, setting: NewUserSetting) -> Result<UserSetting, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;
        let now = Utc::now();

        let active_model = user_settings::ActiveModel {
            user_id: Set(setting.user_id as i64),
            key: Set(setting.key.clone()),
            value: Set(setting.value.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        prelude::UserSettings::insert(active_model)
            .on_conflict(
                OnConflict::columns([user_settings::Column::UserId, user_settings::Column::Key])
                    .update_columns([user_settings::Column::Value, user_settings::Column::UpdatedAt])
                    .to_owned(),
            )
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        prelude::UserSettings::find_by_id((setting.user_id as i64, setting.key))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into)
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))
    }

    async fn delete(&self, transaction: &dyn Transaction, user_id: UserId, key: &str) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        prelude::UserSettings::delete_by_id((user_id as i64, key.to_owned()))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(())
    }

    async fn list_by_user(&self, transaction: &dyn Transaction, user_id: UserId) -> Result<Vec<UserSetting>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let settings = prelude::UserSettings::find()
            .filter(user_settings::Column::UserId.eq(user_id as i64))
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(settings.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{repository::RepositoryService, user::NewUser};
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    async fn create_user(svc: &Arc<RepositoryService>) -> bb_core::user::User {
        use std::collections::HashSet;
        let tx = svc.repository().begin().await.unwrap();
        let user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        user
    }

    async fn create_second_user(svc: &Arc<RepositoryService>) -> bb_core::user::User {
        use std::collections::HashSet;
        let tx = svc.repository().begin().await.unwrap();
        let user = svc
            .user_repository()
            .add_user(&*tx, NewUser::new("bob", "hash", "bob@example.com", HashSet::new()).unwrap())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        user
    }

    // ─── set / get ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_set_creates_new_setting() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        let setting = svc
            .user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: user.id,
                    key: "theme".into(),
                    value: "dark".into(),
                },
            )
            .await
            .unwrap();

        assert_eq!(setting.user_id, user.id);
        assert_eq!(setting.key, "theme");
        assert_eq!(setting.value, "dark");
    }

    #[tokio::test]
    async fn test_set_updates_existing_setting() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: user.id,
                    key: "theme".into(),
                    value: "light".into(),
                },
            )
            .await
            .unwrap();

        let updated = svc
            .user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: user.id,
                    key: "theme".into(),
                    value: "dark".into(),
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.value, "dark");
    }

    #[tokio::test]
    async fn test_get_returns_none_for_missing_key() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_setting_repository().get(&*tx, user.id, "missing").await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_returns_setting_after_set() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: user.id,
                    key: "lang".into(),
                    value: "en".into(),
                },
            )
            .await
            .unwrap();

        let result = svc.user_setting_repository().get(&*tx, user.id, "lang").await.unwrap();

        assert!(result.is_some());
        let setting = result.unwrap();
        assert_eq!(setting.value, "en");
    }

    // ─── delete ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_removes_setting() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: user.id,
                    key: "theme".into(),
                    value: "dark".into(),
                },
            )
            .await
            .unwrap();

        svc.user_setting_repository().delete(&*tx, user.id, "theme").await.unwrap();

        let result = svc.user_setting_repository().get(&*tx, user.id, "theme").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_ok() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.user_setting_repository().delete(&*tx, user.id, "nonexistent").await;

        assert!(result.is_ok());
    }

    // ─── list_by_user ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_user_returns_all_settings() {
        let svc = setup().await;
        let user = create_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        for (key, value) in [("a", "1"), ("b", "2"), ("c", "3")] {
            svc.user_setting_repository()
                .set(
                    &*tx,
                    bb_core::user::NewUserSetting {
                        user_id: user.id,
                        key: key.into(),
                        value: value.into(),
                    },
                )
                .await
                .unwrap();
        }

        let result = svc.user_setting_repository().list_by_user(&*tx, user.id).await.unwrap();

        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_list_by_user_does_not_leak_other_users() {
        let svc = setup().await;
        let alice = create_user(&svc).await;
        let bob = create_second_user(&svc).await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: alice.id,
                    key: "theme".into(),
                    value: "dark".into(),
                },
            )
            .await
            .unwrap();
        svc.user_setting_repository()
            .set(
                &*tx,
                bb_core::user::NewUserSetting {
                    user_id: bob.id,
                    key: "theme".into(),
                    value: "light".into(),
                },
            )
            .await
            .unwrap();

        let alice_settings = svc.user_setting_repository().list_by_user(&*tx, alice.id).await.unwrap();
        let bob_settings = svc.user_setting_repository().list_by_user(&*tx, bob.id).await.unwrap();

        assert_eq!(alice_settings.len(), 1);
        assert_eq!(alice_settings[0].value, "dark");
        assert_eq!(bob_settings.len(), 1);
        assert_eq!(bob_settings[0].value, "light");
    }
}
