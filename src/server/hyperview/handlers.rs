use crate::server::config::AppState;
use crate::server::models::user::User;
use crate::server::services::github_repos::GitHubReposService;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
};
use tracing::{error, info};

fn error_response(message: &str) -> Response {
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="repos-list" style="reposList">
        <text style="error">{}</text>
    </view>"#,
        message
    );

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
    let user = match sqlx::query_as!(User, "SELECT * FROM users WHERE github_id = $1", github_id)
        .fetch_optional(&state.pool)
        .await
    {
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
    let mut xml = String::from(
        r#"<view xmlns="https://hyperview.org/hyperview" id="repos-list" style="reposList">
        <view
            style="reposScroll"
            content-container-style="reposScrollContent"
            scroll="true"
            scroll-orientation="vertical"
            shows-scroll-indicator="true"
        >"#,
    );

    for repo in repos {
        // Extract owner from html_url
        let owner = repo.html_url
            .split('/')
            .nth(3)
            .unwrap_or("unknown");

        xml.push_str(&format!(
            r#"
            <view style="repoItem">
                <text style="repoName">{}</text>
                <text style="repoDescription">{}</text>
                <text style="repoUpdated">Updated {}</text>
                <view style="repoActions">
                    <view style="repoButton">
                        <behavior
                            trigger="press"
                            action="push"
                            href="/hyperview/repo/{}/{}/repomap?github_id={}"
                        />
                        <text style="repoButtonText">View Map</text>
                    </view>
                </view>
            </view>"#,
            repo.name,
            repo.description.unwrap_or_default(),
            repo.updated_at.split('T').next().unwrap_or("unknown"),
            owner,
            repo.name,
            github_id
        ));
    }

    xml.push_str("</view></view>");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}