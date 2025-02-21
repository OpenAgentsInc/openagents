use axum::{extract::State, response::Json};
use axum_extra::extract::cookie::CookieJar;
use serde_json::json;

use crate::server::AppState;

pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({ "status": "healthy" }))
}

pub async fn get_user_info(
    cookies: CookieJar,
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    if let Some(session_cookie) = cookies.get("session") {
        let user_id = session_cookie.value().parse::<i32>().ok();
        if let Some(id) = user_id {
            if let Ok(user) = sqlx::query!(
                r#"
                SELECT
                    id,
                    scramble_id,
                    github_id,
                    github_token,
                    metadata as "metadata: sqlx::types::JsonValue",
                    pseudonym
                FROM users
                WHERE id = $1
                "#,
                id
            )
            .fetch_one(&state.pool)
            .await
            {
                return Json(json!({
                    "authenticated": true,
                    "user": {
                        "id": user.id,
                        "metadata": user.metadata,
                        "pseudonym": user.pseudonym
                    }
                }));
            }
        }
    }

    Json(json!({
        "authenticated": false,
        "user": null
    }))
}
