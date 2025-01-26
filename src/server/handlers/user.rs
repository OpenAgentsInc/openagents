use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, error};

use crate::server::models::user::{CreateUser, User};

pub async fn create_user(
    State(pool): State<PgPool>,
    result: Result<Json<CreateUser>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<User>, (StatusCode, Json<serde_json::Value>)> {
    // Handle JSON parsing/validation errors
    let input = match result {
        Ok(json) => json.0,
        Err(err) => {
            debug!("Validation error: {}", err);
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": format!("Invalid input: {}", err),
                    "code": "VALIDATION_ERROR"
                })),
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
            // Handle specific error cases
            match err {
                ref e if e.to_string().contains("users_scramble_id_key") => {
                    debug!("Duplicate scramble_id attempted");
                    Err((
                        StatusCode::CONFLICT,
                        Json(json!({
                            "error": "User with this scramble_id already exists",
                            "code": "DUPLICATE_SCRAMBLE_ID"
                        })),
                    ))
                }
                e => {
                    error!("Unexpected error creating user: {:?}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to create user",
                            "code": "INTERNAL_ERROR"
                        })),
                    ))
                }
            }
        }
    }
}
