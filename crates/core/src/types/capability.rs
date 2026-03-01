use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub type Capabilities = HashSet<Capability>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    SuperAdmin,
    Admin,
    ConvertBook,
    DeleteBook,
    EditBook,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SuperAdmin => "SuperAdmin",
            Self::Admin => "Admin",
            Self::ConvertBook => "ConvertBook",
            Self::DeleteBook => "DeleteBook",
            Self::EditBook => "EditBook",
        }
    }
}
