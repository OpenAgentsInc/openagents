use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use crate::server::models::timestamp::Timestamp;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: Option<String>,
    pub github_id: Option<i64>,
    pub github_token: Option<String>,
    pub metadata: Option<JsonValue>,
    pub last_login_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub scramble_id: String,
    pub metadata: Option<JsonValue>,
    #[serde(default)]
    pub github_id: Option<i64>,
    #[serde(default)]
    pub github_token: Option<String>,
}
