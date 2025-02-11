use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::{request::Parts, StatusCode};
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: String,
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
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
    sqlx::PgPool: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // TODO: Get user from session/token
        // For now, return a mock user for testing
        Ok(User {
            id: 1,
            scramble_id: "test_user".to_string(),
            metadata: Some(serde_json::json!({
                "github": {
                    "id": 123456,
                    "login": "test",
                    "name": "Test User",
                    "email": "test@example.com",
                    "access_token": "gho_test",
                    "scope": "repo,user"
                }
            }).into()),
            last_login_at: Some(OffsetDateTime::now_utc()),
            created_at: Some(OffsetDateTime::now_utc()),
            updated_at: Some(OffsetDateTime::now_utc()),
        })
    }
}