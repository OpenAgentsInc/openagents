use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::server::config::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub scramble_id: String,
    pub metadata: Option<Value>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
impl<S> FromRequestParts<S> for User
where
    PgPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
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
            })),
            last_login_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }
}