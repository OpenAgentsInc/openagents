use crate::server::config::AppState;
use crate::server::models::user::User;
use crate::server::services::{
    github_issue::GitHubService,
    github_repos::GitHubReposService,
};
use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
};
use tracing::{error, info};
use std::collections::HashMap;
use html_escape;

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
        let owner = repo.html_url.split('/').nth(3).unwrap_or("unknown");

        let repomap_url = format!(
            "/hyperview/repo/{}/{}/repomap?github_id={}",
            owner, repo.name, github_id
        );
        info!(
            "🔘 BUTTON: Generating View Map button with URL: {}",
            repomap_url
        );

        xml.push_str(&format!(
            r#"
            <view style="repoItem">
                <text style="repoName">{}</text>
                <text style="repoDescription">{}</text>
                <text style="repoUpdated">Updated {}</text>
                <view style="repoActions">
                    <text style="repoButtonText repoButton">
                        <behavior
                            trigger="press"
                            action="replace"
                            href="/hyperview/repo/{}/{}/repomap?github_id={}"
                            target="repos_list"
                        />
                        View Map
                    </text>
                    <text style="repoButtonText repoButton" marginLeft="8">
                        <behavior
                            trigger="press"
                            action="replace"
                            href="/hyperview/repo/{}/{}/issues?github_id={}"
                            target="repos_list"
                        />
                        Issues
                    </text>
                </view>
            </view>"#,
            repo.name,
            repo.description.unwrap_or_default(),
            repo.updated_at.split('T').next().unwrap_or("unknown"),
            owner,
            repo.name,
            github_id,
            owner,
            repo.name,
            github_id
        ));
    }

    xml.push_str("</view></view>");

    // Comment out verbose XML logging
    // info!("Generated XML for repos list: {}", xml);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}

pub async fn github_issues(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let result = github_issues_internal(state, &owner, &repo, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(format!(
                r#"<view xmlns="https://hyperview.org/hyperview">
                    <text color="red">Error: {}</text>
                </view>"#,
                e
            ).into())
            .unwrap(),
    }
}

async fn github_issues_internal(
    state: AppState,
    owner: &str,
    repo: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Get user with GitHub token
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE github_id = $1", github_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| anyhow!("User not found"))?;

    // Get GitHub token
    let github_token = user.github_token.ok_or_else(|| anyhow!("No GitHub token found"))?;

    let github_service = GitHubService::new(Some(github_token))?;
    let issues = github_service.list_issues(owner, repo).await?;

    let issues_list = issues
        .iter()
        .map(|issue| {
            format!(
                r#"<view style="issueItem" backgroundColor="rgb(34,34,34)" padding="16" marginBottom="8" borderRadius="8">
                    <text style="issueTitle" color="white" fontSize="18">{title}</text>
                    <text style="issueDescription" color="rgb(128,128,128)" marginTop="4">{desc}</text>
                    <view style="issueActions" flexDirection="row" marginTop="8">
                        <text style="actionButton" backgroundColor="blue" padding="8" borderRadius="4" marginRight="8" color="white">
                            <behavior
                                trigger="press"
                                action="replace"
                                href="/hyperview/repo/{owner}/{repo}/issues/{number}/analyze?github_id={github_id}"
                                target="issues_list"
                            />
                            Analyze Issue
                        </text>
                        <text style="actionButton" backgroundColor="gray" padding="8" borderRadius="4" color="white">
                            <behavior
                                trigger="press"
                                action="new"
                                href="{html_url}"
                            />
                            View on GitHub
                        </text>
                    </view>
                </view>"#,
                title = html_escape::encode_text(&issue.title),
                desc = html_escape::encode_text(issue.body.as_deref().unwrap_or("No description")),
                owner = owner,
                repo = repo,
                number = issue.number,
                github_id = github_id,
                html_url = issue.html_url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="issues_list" backgroundColor="black" flex="1" padding="16">
            <text color="white" fontSize="24" marginBottom="16">Issues: {}/{}</text>
            <view scroll="true" scroll-orientation="vertical" shows-scroll-indicator="true">
                {}
                <text color="white" backgroundColor="gray" padding="8" borderRadius="4" marginTop="16">
                    <behavior
                        trigger="press"
                        action="replace"
                        href="/hyperview/fragments/github-repos?github_id={}"
                        target="repos_list"
                    />
                    Back to Repos
                </text>
            </view>
        </view>"#,
        owner,
        repo,
        issues_list,
        github_id
    );

    Ok(xml)
}
