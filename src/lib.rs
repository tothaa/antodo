//! UI-independent task planning, SQLite storage, and recurrence for antodo.
pub mod core;
#[cfg(feature = "gui")]
pub mod ui;
