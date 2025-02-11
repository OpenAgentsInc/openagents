use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::sync::Arc;

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

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
{
    type Rejection = axum::response::Response;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Implement your user extraction logic here
        todo!("Implement user extraction")
    }
}
