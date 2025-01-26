use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: i32,
    pub scramble_id: String,
    pub metadata: Option<JsonValue>,
    pub last_login_at: Option<OffsetDateTime>,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub scramble_id: String,
    pub metadata: Option<JsonValue>,
}