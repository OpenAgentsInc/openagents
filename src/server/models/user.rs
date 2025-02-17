use crate::server::models::timestamp::{DateTimeWrapper, Timestamp, TimestampExt};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: Option<String>,
    pub github_id: Option<i64>,
    pub github_token: Option<String>,
    pub metadata: JsonValue,
    pub created_at: Timestamp,
    pub last_login_at: Option<Timestamp>,
    pub pseudonym: Option<String>,
}

impl User {
    pub fn new(
        id: i32,
        scramble_id: Option<String>,
        github_id: Option<i64>,
        github_token: Option<String>,
        metadata: JsonValue,
        created_at: DateTimeWrapper,
        last_login_at: Option<DateTimeWrapper>,
        pseudonym: Option<String>,
    ) -> Self {
        Self {
            id,
            scramble_id,
            github_id,
            github_token,
            metadata,
            created_at: created_at.into(),
            last_login_at: last_login_at.to_timestamp(),
            pseudonym,
        }
    }
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
