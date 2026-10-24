use std::{any::Any, sync::Arc};

use bb_core::{CoreServices, repository::RepositoryService};

pub struct TestContext {
    pub services: Arc<CoreServices>,
    pub repos: Arc<RepositoryService>,
    // Keeps container handles (or other resources) alive for the duration of
    // the test.
    _handle: Box<dyn Any + Send>,
}

impl TestContext {
    pub fn new(services: Arc<CoreServices>, repos: Arc<RepositoryService>, handle: impl Any + Send + 'static) -> Self {
        Self {
            services,
            repos,
            _handle: Box::new(handle),
        }
    }
}
