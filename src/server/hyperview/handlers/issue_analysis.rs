use crate::server::config::AppState;
use crate::server::models::user::User;
use crate::server::services::{
    github_issue::{GitHubIssueAnalyzer, GitHubService},
    openrouter::OpenRouterService,
};
use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use html_escape;
use std::collections::HashMap;
use tracing::{error, info};

pub async fn analyze_issue(
    State(state): State<AppState>,
    Path((owner, repo, issue_number)): Path<(String, String, i32)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!(
        "Handling issue analysis request for {}/{} #{}",
        owner, repo, issue_number
    );

    let error_xml = |msg: &str| -> String {
        error!("Issue analysis error: {}", msg);
        format!(
            r#"<view xmlns="https://hyperview.org/hyperview" id="issue_analysis" backgroundColor="black" flex="1" padding="16">
                <text color="red" fontSize="18" marginBottom="16">{}</text>
                <text color="white" backgroundColor="gray" padding="8" borderRadius="4" marginTop="16">
                    <behavior
                        trigger="press"
                        action="replace"
                        href="/hyperview/repo/{}/{}/issues?github_id={}"
                        target="issue_analysis"
                    />
                    Back to Issues
                </text>
            </view>"#,
            msg,
            owner,
            repo,
            params.get("github_id").unwrap_or(&"".to_string())
        )
    };

    // Show loading state first
    if !params.contains_key("start") {
        info!("Showing loading state for issue analysis");
        let loading_xml = format!(
            r#"<view xmlns="https://hyperview.org/hyperview" id="issue_analysis" backgroundColor="black" flex="1" padding="16">
                <text color="white" fontSize="24" marginBottom="16">Analyzing Issue #{}</text>
                <text color="gray" marginBottom="16">Please wait while we analyze this issue...</text>
                <behavior
                    trigger="load"
                    action="replace"
                    href="/hyperview/repo/{}/{}/issues/{}/analyze?github_id={}&start=true"
                    target="issue_analysis"
                />
            </view>"#,
            issue_number,
            owner,
            repo,
            issue_number,
            params.get("github_id").unwrap_or(&"".to_string())
        );

        return Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(loading_xml.into())
            .unwrap();
    }

    info!(
        "Starting issue analysis for {}/{} #{}",
        owner, repo, issue_number
    );
    let result = analyze_issue_internal(state, &owner, &repo, issue_number, &params).await;

    match result {
        Ok(xml) => {
            info!(
                "Successfully analyzed issue {}/{} #{}",
                owner, repo, issue_number
            );
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(xml.into())
                .unwrap()
        }
        Err(e) => {
            error!(
                "Failed to analyze issue {}/{} #{}: {}",
                owner, repo, issue_number, e
            );
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

async fn analyze_issue_internal(
    state: AppState,
    owner: &str,
    repo: &str,
    issue_number: i32,
    params: &HashMap<String, String>,
) -> Result<String> {
    info!(
        "Fetching issue data for {}/{} #{}",
        owner, repo, issue_number
    );

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
    let github_token = user
        .github_token
        .ok_or_else(|| anyhow!("No GitHub token found"))?;

    info!("Fetching issue and comments from GitHub");
    let github_service = GitHubService::new(Some(github_token))?;
    let issue = github_service.get_issue(owner, repo, issue_number).await?;
    let comments = github_service
        .get_issue_comments(owner, repo, issue_number)
        .await?;

    // Combine issue and comments into a single text for analysis
    let mut content = format!(
        "Title: {}\n\n{}\n\n",
        issue.title,
        issue.body.unwrap_or_default()
    );
    for comment in comments {
        content.push_str(&format!(
            "Comment by {}: {}\n\n",
            comment.user.login, comment.body
        ));
    }

    info!("Initializing OpenRouter service for analysis");
    // Get OpenRouter API key from environment
    let openrouter_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow!("OPENROUTER_API_KEY environment variable not set"))?;

    let openrouter = OpenRouterService::new(openrouter_key);
    let analyzer = GitHubIssueAnalyzer::new(openrouter);

    info!("Sending issue content to OpenRouter for analysis");
    let files = analyzer.analyze_issue(&content).await?;

    info!("Formatting file list as XML");
    // Format the files list as Hyperview XML
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="issue_analysis" backgroundColor="black" flex="1" padding="16">
            <text color="white" fontSize="24" marginBottom="16">Relevant Files for Issue #{}</text>

            <view scroll="true" scroll-orientation="vertical" shows-scroll-indicator="true">
                {}

                <text color="white" backgroundColor="gray" padding="8" borderRadius="4" marginTop="16">
                    <behavior
                        trigger="press"
                        action="replace"
                        href="/hyperview/repo/{}/{}/issues?github_id={}"
                        target="issue_analysis"
                    />
                    Back to Issues
                </text>
            </view>
        </view>"#,
        issue_number,
        files.files
            .iter()
            .map(|file| format!(
                r#"<view style="fileItem" backgroundColor="rgb(34,34,34)" padding="16" marginBottom="8" borderRadius="8">
                    <text color="white" fontSize="18">{}</text>
                    <text color="rgb(128,128,128)" marginTop="4">{}</text>
                    <text color="white" marginTop="4">Priority: {}/10</text>
                </view>"#,
                html_escape::encode_text(&file.filepath),
                html_escape::encode_text(&file.comment),
                file.priority
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        owner,
        repo,
        github_id
    );

    info!("Successfully generated files list XML");
    Ok(xml)
}
