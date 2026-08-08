use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    Error,
    event::EventService,
    message::{NewSystemMessage, SystemMessage, SystemMessageId},
    repository::RepositoryService,
    with_read_only_transaction, with_transaction,
};

#[allow(unused_lifetimes)] // async_trait macro generates hidden lifetime parameters
#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
pub trait SystemMessageService: Send + Sync {
    async fn add_message(&self, msg: NewSystemMessage) -> Result<SystemMessage, Error>;
    async fn list_messages(&self) -> Result<Vec<SystemMessage>, Error>;
    async fn delete_message(&self, id: SystemMessageId) -> Result<(), Error>;
    async fn delete_all_messages(&self) -> Result<(), Error>;
    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64, Error>;
}

pub struct SystemMessageServiceImpl {
    repository_service: Arc<RepositoryService>,
    event_service: Arc<dyn EventService>,
}

impl SystemMessageServiceImpl {
    #[must_use]
    pub fn new(repository_service: Arc<RepositoryService>, event_service: Arc<dyn EventService>) -> Self {
        Self {
            repository_service,
            event_service,
        }
    }
}

#[async_trait::async_trait]
impl SystemMessageService for SystemMessageServiceImpl {
    async fn add_message(&self, msg: NewSystemMessage) -> Result<SystemMessage, Error> {
        let result = with_transaction!(self, system_message_repository, |tx| system_message_repository.add_message(tx, msg).await);
        if result.is_ok() {
            self.event_service.notify_system_messages_changed();
        }
        result
    }

    async fn list_messages(&self) -> Result<Vec<SystemMessage>, Error> {
        with_read_only_transaction!(self, system_message_repository, |tx| system_message_repository.list_messages(tx).await)
    }

    async fn delete_message(&self, id: SystemMessageId) -> Result<(), Error> {
        let result = with_transaction!(self, system_message_repository, |tx| system_message_repository.delete_message(tx, id).await);
        if result.is_ok() {
            self.event_service.notify_system_messages_changed();
        }
        result
    }

    async fn delete_all_messages(&self) -> Result<(), Error> {
        let result = with_transaction!(self, system_message_repository, |tx| {
            system_message_repository.delete_all_messages(tx).await
        });
        if result.is_ok() {
            self.event_service.notify_system_messages_changed();
        }
        result
    }

    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64, Error> {
        let result = with_transaction!(self, system_message_repository, |tx| {
            system_message_repository.delete_older_than(tx, cutoff).await
        });
        if let Ok(count) = &result
            && *count > 0
        {
            self.event_service.notify_system_messages_changed();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::{MessageSeverity, repository::MockSystemMessageRepository},
        repository::testing::{default_repository_service_builder, make_mock_repo},
        test_support::nop_event_service,
    };

    fn make_test_message(id: u64) -> SystemMessage {
        SystemMessage {
            id,
            source_task: "test.task".to_string(),
            severity: MessageSeverity::Info,
            message: "Test message".to_string(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn add_message_delegates_to_repository() {
        let mut mock_repo = MockSystemMessageRepository::new();
        let expected = make_test_message(1);
        let expected_clone = expected.clone();
        mock_repo.expect_add_message().returning(move |_, _| {
            let val = expected_clone.clone();
            Box::pin(async move { Ok(val) })
        });

        let svc = SystemMessageServiceImpl::new(
            Arc::new(
                default_repository_service_builder()
                    .repository(Arc::new(make_mock_repo()))
                    .system_message_repository(Arc::new(mock_repo))
                    .build()
                    .unwrap(),
            ),
            nop_event_service(),
        );

        let result = svc
            .add_message(NewSystemMessage {
                source_task: "test.task".to_string(),
                severity: MessageSeverity::Info,
                message: "Test message".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.id, expected.id);
        assert_eq!(result.source_task, expected.source_task);
    }

    #[tokio::test]
    async fn list_messages_delegates_to_repository() {
        let mut mock_repo = MockSystemMessageRepository::new();
        let msgs = vec![make_test_message(1), make_test_message(2)];
        let msgs_clone = msgs.clone();
        mock_repo.expect_list_messages().returning(move |_| {
            let val = msgs_clone.clone();
            Box::pin(async move { Ok(val) })
        });

        let svc = SystemMessageServiceImpl::new(
            Arc::new(
                default_repository_service_builder()
                    .repository(Arc::new(make_mock_repo()))
                    .system_message_repository(Arc::new(mock_repo))
                    .build()
                    .unwrap(),
            ),
            nop_event_service(),
        );

        let result = svc.list_messages().await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn delete_message_delegates_to_repository() {
        let mut mock_repo = MockSystemMessageRepository::new();
        mock_repo.expect_delete_message().returning(|_, _| Box::pin(async { Ok(()) }));

        let svc = SystemMessageServiceImpl::new(
            Arc::new(
                default_repository_service_builder()
                    .repository(Arc::new(make_mock_repo()))
                    .system_message_repository(Arc::new(mock_repo))
                    .build()
                    .unwrap(),
            ),
            nop_event_service(),
        );

        svc.delete_message(1).await.unwrap();
    }

    #[tokio::test]
    async fn delete_all_messages_delegates_to_repository() {
        let mut mock_repo = MockSystemMessageRepository::new();
        mock_repo.expect_delete_all_messages().returning(|_| Box::pin(async { Ok(()) }));

        let svc = SystemMessageServiceImpl::new(
            Arc::new(
                default_repository_service_builder()
                    .repository(Arc::new(make_mock_repo()))
                    .system_message_repository(Arc::new(mock_repo))
                    .build()
                    .unwrap(),
            ),
            nop_event_service(),
        );

        svc.delete_all_messages().await.unwrap();
    }

    #[tokio::test]
    async fn delete_older_than_delegates_to_repository() {
        let mut mock_repo = MockSystemMessageRepository::new();
        mock_repo.expect_delete_older_than().returning(|_, _| Box::pin(async { Ok(5) }));

        let svc = SystemMessageServiceImpl::new(
            Arc::new(
                default_repository_service_builder()
                    .repository(Arc::new(make_mock_repo()))
                    .system_message_repository(Arc::new(mock_repo))
                    .build()
                    .unwrap(),
            ),
            nop_event_service(),
        );

        let count = svc.delete_older_than(Utc::now()).await.unwrap();
        assert_eq!(count, 5);
    }
}
