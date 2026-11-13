//! Shared domain contracts. HTTP and persistence use these same validators.
use serde::{Deserialize, Serialize};
pub mod engine;
pub mod model;
pub mod sync;
pub mod tv;
pub use model::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),
    #[error("Entity not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("Strict WIP limit reached")]
    WipLimit,
    #[error("An unfinished prerequisite blocks this action")]
    Blocked,
    #[error("Authentication required or expired")]
    Unauthorized,
    #[error("Storage operation failed")]
    Storage,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn text(value: &str, max: usize, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(Error::Validation(format!(
            "{field} must contain 1–{max} characters"
        )));
    }
    Ok(value.to_owned())
}

pub fn board_prefix(value: &str) -> Result<String> {
    if value.is_empty() || value.len() > 12 || !value.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(Error::Validation(
            "prefix must contain 1–12 uppercase ASCII letters".into(),
        ));
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub revision: i64,
    #[serde(default = "initial_counter")]
    pub next_ticket_number: i64,
}
fn initial_counter() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Column {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub position: i64,
    pub is_done: bool,
    pub wip_limit: Option<u32>,
    pub wip_strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Ticket {
    pub id: String,
    pub public_id: String,
    pub board_id: String,
    pub column_id: Option<String>,
    pub title: String,
    pub description: String,
    pub importance: u8,
    pub done: bool,
    pub revision: i64,
    #[serde(default)]
    pub details: TicketDetails,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewBoard {
    pub name: String,
    pub prefix: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewColumn {
    pub name: String,
    #[serde(default)]
    pub is_done: bool,
    pub wip_limit: Option<u32>,
    #[serde(default)]
    pub wip_strict: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTicket {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub importance: u8,
    pub column_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveTicket {
    pub board_id: String,
    pub column_id: Option<String>,
    pub base_revision: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_cannot_inject_or_alias_identifiers() {
        for value in ["", "dev", "DEV-1", "Ä", "DEV_", "ABCDEFGHIJKLM"] {
            assert!(board_prefix(value).is_err(), "{value}");
        }
        assert_eq!(board_prefix("DEV").unwrap(), "DEV");
    }

    #[test]
    fn text_limits_count_unicode_characters() {
        assert_eq!(text("  öffnen  ", 6, "title").unwrap(), "öffnen");
        assert!(text("   ", 300, "title").is_err());
        assert!(text("abcdefg", 6, "title").is_err());
    }
}
