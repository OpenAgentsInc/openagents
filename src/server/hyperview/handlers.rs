use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
};
use reqwest;
use serde_json::json;
use tracing::{error, info};

use crate::server::{
    models::{repository::Repository, user::User},
    AppState,
};

use super::templates::{render_error_screen, render_loading_screen, render_repositories_screen};

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

pub async fn repositories_screen(State(state): State<AppState>, user: User) -> Response {
    info!("Fetching repositories for user: {}", user.id);

    // Get GitHub token from user metadata
    let github_metadata = user
        .metadata
        .get("github")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            error!("GitHub metadata not found for user: {}", user.id);
            render_error_screen("GitHub account not connected")
        });

    let github_metadata = match github_metadata {
        Ok(metadata) => metadata,
        Err(error_screen) => {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(error_screen.into())
                .unwrap()
        }
    };

    let access_token = github_metadata
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error!("GitHub access token not found for user: {}", user.id);
            render_error_screen("GitHub access token not found")
        });

    let access_token = match access_token {
        Ok(token) => token,
        Err(error_screen) => {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(error_screen.into())
                .unwrap()
        }
    };

    // Fetch repositories from GitHub
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user/repos")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "OpenAgents")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await;

    let response = match response {
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

    let repos = response.json::<Vec<Repository>>().await;
    let repos = match repos {
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