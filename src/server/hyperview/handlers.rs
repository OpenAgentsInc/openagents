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
    models::repository::Repository,
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

pub async fn repositories_screen(State(state): State<AppState>) -> Response {
    // TODO: Get user from session/token
    // For now, return a mock list of repositories
    let repos = vec![
        Repository {
            id: 1,
            name: "openagents".to_string(),
            full_name: "OpenAgentsInc/openagents".to_string(),
            description: Some("OpenAgents server and API".to_string()),
            private: false,
            html_url: "https://github.com/OpenAgentsInc/openagents".to_string(),
            updated_at: "2024-02-10T12:00:00Z".to_string(),
        },
        Repository {
            id: 2,
            name: "onyx".to_string(),
            full_name: "OpenAgentsInc/onyx".to_string(),
            description: Some("OpenAgents mobile app".to_string()),
            private: false,
            html_url: "https://github.com/OpenAgentsInc/onyx".to_string(),
            updated_at: "2024-02-09T15:30:00Z".to_string(),
        },
    ];

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(render_repositories_screen(repos).into())
        .unwrap()
}