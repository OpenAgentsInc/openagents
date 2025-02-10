use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
};
use reqwest;
use serde_json::Value;
use tracing::{error, info};

use crate::server::{
    config::AppState,
    models::{repository::Repository, user::User},
};

pub async fn hello_world() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="text" alignItems="center" justifyContent="center" />
    </styles>
    <body>
      <view style="text">
        <text>Hello from OpenAgents!</text>
      </view>
    </body>
  </screen>
</doc>"#
                .to_string()
                .into(),
        )
        .unwrap()
}

fn render_error_screen(error: &str) -> String {
    format!(
        r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>
        <styles>
            <style id="screen" backgroundColor="black" flex="1" />
            <style id="error" color="red" fontSize="16" padding="16" textAlign="center" />
            <style id="retry_button" backgroundColor="#333333" padding="12" margin="16" borderRadius="8" alignItems="center" />
            <style id="retry_text" color="white" fontSize="14" />
        </styles>
        <body style="screen">
            <text style="error">{}</text>
            <view style="retry_button" href="/hyperview/repositories">
                <text style="retry_text">Retry</text>
            </view>
        </body>
    </screen>
</doc>"###,
        error
    )
}

fn render_repositories_screen(repos: Vec<Repository>) -> String {
    let repo_items = repos
        .into_iter()
        .map(|repo| {
            format!(
                r###"<item style="repo_item" href="/hyperview/repo/{}/issues">
                    <text style="repo_name">{}</text>
                    {}
                    <text style="repo_meta">Last updated: {}</text>
                </item>"###,
                repo.full_name,
                repo.name,
                repo.description
                    .map(|d| format!(r###"<text style="repo_desc">{}</text>"###, d))
                    .unwrap_or_default(),
                repo.updated_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>
        <styles>
            <style id="screen" backgroundColor="black" flex="1" />
            <style id="header" backgroundColor="gray" padding="16" />
            <style id="title" color="white" fontSize="20" fontWeight="bold" />
            <style id="list" flex="1" />
            <style id="repo_item" backgroundColor="#111111" marginBottom="8" padding="16" borderRadius="8" />
            <style id="repo_name" color="white" fontSize="16" fontWeight="bold" />
            <style id="repo_desc" color="#999999" fontSize="14" marginTop="4" />
            <style id="repo_meta" color="#666666" fontSize="12" marginTop="8" />
            <style id="error" color="red" fontSize="16" padding="16" textAlign="center" />
            <style id="loading" flex="1" justifyContent="center" alignItems="center" />
        </styles>
        <body style="screen">
            <header style="header">
                <text style="title">Your Repositories</text>
            </header>
            <list style="list">
                {repo_items}
            </list>
        </body>
    </screen>
</doc>"###
    )
}

#[axum::debug_handler]
pub async fn repositories_screen(
    State(state): State<AppState>,
    user: User,
) -> Response {
    info!("Fetching repositories for user: {}", user.id);

    // Get GitHub token from user metadata
    let metadata = match user.metadata {
        Some(Value::Object(m)) => m,
        _ => {
            error!("Invalid metadata format for user: {}", user.id);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(render_error_screen("Invalid user metadata").into())
                .unwrap();
        }
    };

    let github = match metadata.get("github") {
        Some(Value::Object(gh)) => gh,
        _ => {
            error!("GitHub metadata not found for user: {}", user.id);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(render_error_screen("GitHub account not connected").into())
                .unwrap();
        }
    };

    let access_token = match github.get("access_token") {
        Some(Value::String(token)) => token,
        _ => {
            error!("GitHub access token not found for user: {}", user.id);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(render_error_screen("GitHub access token not found").into())
                .unwrap();
        }
    };

    // Fetch repositories from GitHub
    let client = reqwest::Client::new();
    let response = match client
        .get("https://api.github.com/user/repos")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "OpenAgents")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to fetch repositories: {}", e);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(render_error_screen("Failed to fetch repositories").into())
                .unwrap();
        }
    };

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        error!("GitHub API error: {}", error_text);
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
            .body(render_error_screen("Failed to fetch repositories").into())
            .unwrap();
    }

    let repos = match response.json::<Vec<Repository>>().await {
        Ok(repos) => repos,
        Err(e) => {
            error!("Failed to parse repository response: {}", e);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(render_error_screen("Failed to parse repository data").into())
                .unwrap();
        }
    };

    info!("Successfully fetched {} repositories", repos.len());

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(render_repositories_screen(repos).into())
        .unwrap()
}