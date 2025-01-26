use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::json;
use sqlx::PgPool;
use tracing::error;

use crate::server::models::user::{CreateUser, User};

pub async fn create_user(
    State(pool): State<PgPool>,
    result: Result<Json<CreateUser>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<User>, (StatusCode, Json<serde_json::Value>)> {
    // Handle JSON parsing/validation errors
    let input = match result {
        Ok(json) => json.0,
        Err(err) => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": format!("Invalid input: {}", err),
                    "code": "VALIDATION_ERROR"
                }))
            ));
        }
    };

    // Attempt to create the user
    let result = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (scramble_id, metadata)
        VALUES ($1, $2)
        RETURNING id, scramble_id, metadata, last_login_at, created_at, updated_at
        "#,
        input.scramble_id,
        input.metadata.unwrap_or_else(|| json!({}))
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(user) => Ok(Json(user)),
        Err(err) => {
            error!("Failed to create user: {:?}", err);
            
            // Handle specific error cases
            let error_response = match err {
                sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                    (
                        StatusCode::CONFLICT,
                        Json(json!({
                            "error": "User with this scramble_id already exists",
                            "code": "DUPLICATE_SCRAMBLE_ID"
                        }))
                    )
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to create user",
                        "code": "INTERNAL_ERROR"
                    }))
                )
            };
            
            Err(error_response)
        }
    }
}