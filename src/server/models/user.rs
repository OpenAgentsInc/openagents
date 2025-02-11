use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: Option<String>,
    pub github_id: Option<i64>,
    pub github_token: Option<String>,
    pub metadata: Option<JsonValue>,
    #[serde(with = "time::serde::timestamp::option")]
    pub last_login_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::timestamp::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::timestamp::option")]
    pub updated_at: Option<OffsetDateTime>,
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
