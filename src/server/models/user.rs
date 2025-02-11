use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: String,
    pub metadata: Option<JsonValue>,
}