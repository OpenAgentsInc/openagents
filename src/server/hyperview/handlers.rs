use crate::server::config::AppState;
use axum::{
    extract::{State, Query},
    http::{header, StatusCode},
    response::Response,
};
use super::services::GitHubReposService;
use anyhow::Result;
use serde_json::Value;
use crate::server::models::user::User;
use serde::Deserialize;
use tracing::{info, error};

pub async fn main_screen(State(state): State<AppState>) -> Response {
    info!("Handling main_screen request");

    // Get the most recently logged in user with GitHub auth
    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users
         WHERE github_id IS NOT NULL
         ORDER BY last_login_at DESC NULLS LAST
         LIMIT 1"
    )
    .fetch_optional(&state.pool)
    .await;

    match user {
        Ok(Some(user)) => {
            info!("Found most recent user: {:?}", user);
            if let Some(github_id) = user.github_id {
                info!("Extracted GitHub ID: {}", github_id);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                    .body(
                        format!(r###"<?xml version="1.0" encoding="UTF-8" ?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="black" alignItems="center" justifyContent="center" />
      <style id="title" fontSize="24" color="white" marginBottom="32" />
      <style id="button" backgroundColor="white" padding="16" borderRadius="8" marginTop="16" width="240" alignItems="center" />
      <style id="buttonText" color="black" fontSize="16" fontWeight="600" />
      <style id="reposList" width="100%" padding="16" />
      <style id="repoItem" backgroundColor="#222" padding="16" marginBottom="8" borderRadius="8" />
      <style id="repoName" color="white" fontSize="16" fontWeight="600" />
      <style id="repoDescription" color="#808080" fontSize="14" marginTop="4" />
      <style id="error" color="red" fontSize="16" fontWeight="600" />
      <style id="welcomeText" color="white" fontSize="16" />
    </styles>

    <body style="container">
      <view id="user-info">
        <behavior
          trigger="load"
          action="replace"
          target="user-info"
          href="/hyperview/fragments/user-info?github_id={github_id}"
        />
      </view>

      <text style="title">Welcome to OpenAgents</text>

      <view id="repos-list" style="reposList">
        <behavior
          trigger="load"
          action="replace"
          target="repos-list"
          href="/hyperview/fragments/github-repos?github_id={github_id}"
        />
      </view>

      <!-- Chat Button -->
      <view style="button">
        <behavior
          trigger="press"
          action="push"
          href="/hyperview/chat"
        />
        <text style="buttonText">Start Chat</text>
      </view>
    </body>
  </screen>
</doc>"###, github_id=github_id).into()
                    )
                    .unwrap()
            } else {
                error!("No GitHub ID found in user metadata");
                redirect_to_auth()
            }
        }
        Ok(None) => {
            error!("No users found in database");
            redirect_to_auth()
        }
        Err(e) => {
            error!("Database error: {}", e);
            redirect_to_auth()
        }
    }
}

fn extract_github_id(metadata: &Option<serde_json::Value>) -> Option<String> {
    if let Some(metadata) = metadata {
        if let Value::Object(obj) = metadata {
            if let Some(Value::Object(github)) = obj.get("github") {
                if let Some(Value::Number(id)) = github.get("id") {
                    return Some(id.to_string());
                }
            }
        }
    }
    None
}

fn redirect_to_auth() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(include_str!("../../../templates/pages/auth/login.xml").into())
        .unwrap()
}

pub async fn connected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusConnected">
  Connected
</text>"###
                .into(),
        )
        .unwrap()
}

pub async fn disconnected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusDisconnected">
  Disconnected - Reconnecting...
</text>"###
                .into(),
        )
        .unwrap()
}

// Helper function to get user from GitHub ID
async fn get_user_from_github_id(
    state: &AppState,
    github_id: &str,
) -> Option<User> {
    let github_id: i64 = github_id.parse().ok()?;

    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE github_id = $1",
        github_id
    )
    .fetch_optional(&state.pool)
    .await
    .ok()?;

    user
}

pub async fn user_info(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Get GitHub ID from params
    let github_id = match params.get("github_id") {
        Some(id) => id,
        None => return error_response("GitHub ID not provided"),
    };

    // Get user from GitHub ID
    let user = match get_user_from_github_id(&state, github_id).await {
        Some(user) => user,
        None => return error_response("User not found"),
    };

    // Get username from GitHub metadata
    let username = if let Some(ref metadata) = user.metadata {
        if let Value::Object(obj) = metadata {
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
        }
    } else {
        "User"
    };

    let xml = format!(r#"<view xmlns="https://hyperview.org/hyperview" id="user-info">
        <text style="welcomeText">Welcome, {username}!</text>
    </view>"#);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}

pub async fn github_repos(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    info!("Handling github_repos request with params: {:?}", params);

    // Get GitHub ID from params
    let github_id = match params.get("github_id") {
        Some(id) => {
            info!("Got GitHub ID from params: {}", id);
            id
        }
        None => {
            error!("No GitHub ID provided in params");
            return error_response("GitHub ID not provided");
        }
    };

    // Get user from GitHub ID
    let user = match get_user_from_github_id(&state, github_id).await {
        Some(user) => user,
        None => return error_response("User not found"),
    };

    // Get GitHub token
    let github_token = match get_user_github_token(&state, user.id).await {
        Ok(Some(token)) => token,
        _ => return error_response("No GitHub token found"),
    };

    // Initialize GitHub service
    let github_service = match GitHubReposService::new(github_token) {
        Ok(service) => service,
        Err(_) => return error_response("Failed to initialize GitHub service"),
    };

    // Fetch repos
    let repos = match github_service.get_user_repos().await {
        Ok(repos) => repos,
        Err(_) => return error_response("Failed to fetch repositories"),
    };

    // Generate HXML for repos
    let mut xml = String::from(r#"<view xmlns="https://hyperview.org/hyperview" id="repos-list" style="reposList">"#);

    for repo in repos {
        xml.push_str(&format!(r#"
            <view style="repoItem">
                <text style="repoName">{}</text>
                <text style="repoDescription">{}</text>
            </view>"#,
            repo.name,
            repo.description.unwrap_or_default()
        ));
    }

    xml.push_str("</view>");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}

fn error_response(message: &str) -> Response {
    let xml = format!(r#"<view xmlns="https://hyperview.org/hyperview" id="repos-list" style="reposList">
        <text style="error">{}</text>
    </view>"#, message);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}

async fn get_user_github_token(state: &AppState, user_id: i32) -> Result<Option<String>> {
    // Get user from database
    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE id = $1",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    // Extract token from metadata
    if let Some(user) = user {
        if let Some(metadata) = user.metadata {
            if let Value::Object(obj) = metadata {
                if let Some(Value::Object(github)) = obj.get("github") {
                    if let Some(Value::String(token)) = github.get("access_token") {
                        return Ok(Some(token.clone()));
                    }
                }
            }
        }
    }

    Ok(None)
}

#[derive(Deserialize)]
pub struct ContentQuery {
    section: String,
    user_id: i32,
}

pub async fn content(
    State(_state): State<AppState>,
    Query(params): Query<ContentQuery>,
) -> Response {
    let xml = format!(r#"<view xmlns="https://hyperview.org/hyperview" id="content">
        <text style="welcomeText">Content section: {}</text>
    </view>"#, params.section);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}
