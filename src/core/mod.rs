pub mod actions;
pub mod model;
pub mod notes;
pub mod preferences;
pub mod query;
pub mod recurrence;
pub mod sql;
pub mod store;

pub use model::*;
pub use store::Document;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),
    #[error("Database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("File: {0}")]
    Io(#[from] std::io::Error),
    #[error("Data: {0}")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn invalid(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}

pub mod mcp;

pub mod llm;
