use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use sqlx::{types::JsonValue, PgPool};
use tracing::{debug, error};

use crate::server::models::{
    timestamp::DateTimeWrapper,
    user::{CreateUser, User},
};

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
    let result = sqlx::query!(
        r#"
        INSERT INTO users (scramble_id, metadata, github_id, github_token, pseudonym)
        VALUES ($1, $2, $3, $4, $1)
        RETURNING id, scramble_id, github_id, github_token, metadata,
                 created_at, last_login_at, pseudonym
        "#,
        input.scramble_id,
        input.metadata.unwrap_or_else(|| json!({})) as JsonValue,
        input.github_id,
        input.github_token
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(row) => {
            let user = User::new(
                row.id,
                row.scramble_id,
                row.github_id,
                row.github_token,
                row.metadata.expect("metadata should never be null"),
                DateTimeWrapper(row.created_at.expect("created_at should never be null")),
                row.last_login_at.map(DateTimeWrapper),
                row.pseudonym,
            );
            Ok(Json(user))
        }
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

pub async fn get_user(
    State(pool): State<PgPool>,
    Json(id): Json<i32>,
) -> Result<Json<User>, (StatusCode, String)> {
    let row = sqlx::query!(
        r#"
        SELECT id, scramble_id, github_id, github_token, metadata,
        created_at, last_login_at, pseudonym
        FROM users
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch user: {}", e),
        )
    })?;

    match row {
        Some(row) => Ok(Json(User::new(
            row.id,
            row.scramble_id,
            row.github_id,
            row.github_token,
            row.metadata.expect("metadata should never be null"),
            DateTimeWrapper(row.created_at.expect("created_at should never be null")),
            row.last_login_at.map(DateTimeWrapper),
            row.pseudonym,
        ))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("User with id {} not found", id),
        )),
    }
}
