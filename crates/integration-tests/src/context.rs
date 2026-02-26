use std::{any::Any, sync::Arc};

use bb_core::CoreServices;

pub struct TestContext {
    // pub services: Arc<CoreServices>,
    // Keeps container handles (or other resources) alive for the duration of the test.
    _handle: Box<dyn Any + Send>,
}

impl TestContext {
    pub fn new(_services: Arc<CoreServices>, handle: impl Any + Send + 'static) -> Self {
        Self {
            // services,
            _handle: Box::new(handle),
        }
    }
}
