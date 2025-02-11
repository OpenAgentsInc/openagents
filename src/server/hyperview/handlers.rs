use crate::server::config::AppState;
use axum::{
    extract::{State, Query},
    http::{header, StatusCode},
    response::Response,
};
use crate::server::services::github_repos::GitHubReposService;
use anyhow::Result;
use serde_json::Value;
use crate::server::models::user::User;
use serde::Deserialize;
use tracing::{info, error};
use crate::server::handlers::auth::session::clear_session_cookie;

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
        None => return auth_error_fragment_response(),
    };

    // Get user from GitHub ID
    let user = match get_user_from_github_id(&state, github_id).await {
        Some(user) => user,
        None => return auth_error_fragment_response(),
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

fn auth_error_fragment_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(r#"<view xmlns="https://hyperview.org/hyperview">
            <behavior
              trigger="load"
              action="navigate"
              href="/auth/github/login?platform=mobile"
              new-stack="true"
              force-reset="true"
            />
        </view>"#.into())
        .unwrap()
}

fn auth_error_response(_message: &str) -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(r###"<?xml version="1.0" encoding="UTF-8" ?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="black" alignItems="center" justifyContent="center" />
      <style id="title" fontSize="24" color="white" marginBottom="32" />
      <style id="button" backgroundColor="white" padding="16" borderRadius="8" marginTop="16" width="240" alignItems="center" />
      <style id="buttonText" color="black" fontSize="16" fontWeight="600" />
      <style id="loading" color="white" fontSize="14" marginTop="16" />
    </styles>
    <body style="container">
      <text style="title">Welcome to OpenAgents</text>

      <!-- Loading State -->
      <text id="loading-text" style="loading" display="none">Connecting to GitHub...</text>

      <!-- GitHub Login Button -->
      <view style="button" id="login-button">
        <behavior
          trigger="press"
          action="open-url"
          href="/auth/github/login?platform=mobile"
          verb="GET"
          show-during-load="loading-text"
          hide-during-load="login-button"
        />
        <text style="buttonText">Continue with GitHub</text>
      </view>
    </body>
  </screen>
</doc>"###.into())
        .unwrap()
}

pub async fn login_page() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(include_str!("../../../templates/pages/auth/login.xml").into())
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
            match id.parse::<i64>() {
                Ok(id) => id,
                Err(_) => return error_response("Invalid GitHub ID"),
            }
        }
        None => {
            error!("No GitHub ID provided in params");
            return error_response("GitHub ID not provided");
        }
    };

    // Get user with GitHub token
    let user = match sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE github_id = $1",
        github_id
    )
    .fetch_optional(&state.pool)
    .await {
        Ok(Some(user)) => user,
        Ok(None) => return error_response("User not found"),
        Err(e) => return error_response(&format!("Database error: {}", e)),
    };

    // Get GitHub token
    let github_token = match user.github_token {
        Some(token) => token,
        None => return error_response("No GitHub token found"),
    };

    // Initialize GitHub service
    let github_service = match GitHubReposService::new(github_token) {
        Ok(service) => service,
        Err(e) => return error_response(&format!("Failed to initialize GitHub service: {}", e)),
    };

    // Fetch repos
    let mut repos = match github_service.get_user_repos().await {
        Ok(repos) => repos,
        Err(e) => return error_response(&format!("Failed to fetch repos: {}", e)),
    };

    // Sort repos by updated_at (most recent first) and take first 10
    repos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let repos = repos.into_iter().take(10);

    // Generate HXML for repos with scrollview
    let mut xml = String::from(r#"<view xmlns="https://hyperview.org/hyperview" id="repos-list" style="reposList">
        <view
            style="reposScroll"
            content-container-style="reposScrollContent"
            scroll="true"
            scroll-orientation="vertical"
            shows-scroll-indicator="true"
        >"#);

    for repo in repos {
        xml.push_str(&format!(r#"
            <view style="repoItem">
                <text style="repoName">{}</text>
                <text style="repoDescription">{}</text>
                <text style="repoUpdated">Updated {}</text>
            </view>"#,
            repo.name,
            repo.description.unwrap_or_default(),
            repo.updated_at.split('T').next().unwrap_or("unknown")
        ));
    }

    xml.push_str("</view></view>");

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

pub async fn main_page(State(state): State<AppState>) -> Response {
    // Check auth first
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
        Ok(Some(user)) if user.github_id.is_some() => {
            // User is authenticated, serve the main page
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(include_str!("../../../templates/pages/main.xml").into())
                .unwrap()
        }
        _ => {
            // Not authenticated, force redirect to login
            auth_error_response("Not authenticated")
        }
    }
}

// Modify the logout handler
pub async fn logout() -> Response {
    info!("🔐 Handling logout request");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .header(header::SET_COOKIE, "session=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
        .body(r###"<behavior
          xmlns="https://hyperview.org/hyperview"
          trigger="load"
          action="navigate"
          href="/templates/pages/auth/login.xml"
          new-stack="true"
          force-reset="true"
        />"###.into())
        .unwrap()
}

pub async fn mobile_logout() -> Response {
    info!("🔐 Starting mobile logout request");

    let cookie = clear_session_cookie();
    info!("🔐 Created clear cookie: {}", cookie);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .header(header::SET_COOKIE, cookie)
        .body(r###"<view xmlns="https://hyperview.org/hyperview">
          <behavior
            trigger="load"
            action="navigate"
            href="/templates/pages/auth/login.xml"
            new-stack="true"
            force-reset="true"
          />
        </view>"###.into())
        .unwrap();

    info!("🔐 Sending logout response with navigation behavior");
    response
}
