use crate::server::config::AppState;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
};
use serde_json::Value;
use std::collections::HashMap;

pub async fn user_info(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    // Get GitHub ID from params
    let github_id = match params.get("github_id") {
        Some(id) => id,
        None => return auth_error_fragment_response(),
    };

    // Get user from GitHub ID
    let user = match get_user_from_github_id(&state, github_id).await {
        Some(user) => user,
        None => return auth_error_fragment_response(),
    };

    // Get username from GitHub metadata
    let username = if let Some(Value::Object(obj)) = user.metadata.as_ref() {
        if let Some(Value::Object(github)) = obj.get("github") {
            if let Some(Value::String(name)) = github.get("name") {
                name.as_str()
            } else {
                "User"
            }
        } else {
            "User"
        }
    } else {
        "User"
    };

    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="user-info">
        <text style="welcomeText">Welcome, {username}!</text>
    </view>"#
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}

// Helper function to get user from GitHub ID
async fn get_user_from_github_id(
    state: &AppState,
    github_id: &str,
) -> Option<crate::server::models::user::User> {
    let github_id: i64 = github_id.parse().ok()?;

    sqlx::query_as!(
        crate::server::models::user::User,
        "SELECT * FROM users WHERE github_id = $1",
        github_id
    )
    .fetch_optional(&state.pool)
    .await
    .ok()?
}

fn auth_error_fragment_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r#"<view xmlns="https://hyperview.org/hyperview">
            <behavior
              trigger="load"
              action="navigate"
              href="/auth/github/login?platform=mobile"
              new-stack="true"
              force-reset="true"
            />
        </view>"#
                .into(),
        )
        .unwrap()
}
