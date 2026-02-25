use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub type Capabilities = HashSet<Capability>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Admin,
    ConvertBook,
    DeleteBook,
    EditBook,
}
