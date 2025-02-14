use crate::repo::cleanup_temp_dir;
use crate::server::config::AppState;
use crate::server::services::{
    deepseek::DeepSeekService,
    openrouter::OpenRouterService,
    solver::{SolverService, SolverStatus},
};
use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use html_escape;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info};

pub async fn solver_status(
    State(state): State<AppState>,
    Path(solver_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!("Handling solver status request for {}", solver_id);

    let result = solver_status_internal(state, &solver_id, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => {
            error!("Failed to get solver status: {}", e);
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

async fn solver_status_internal(
    state: AppState,
    solver_id: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Verify user exists and has access
    let _exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE github_id = $1)",
        github_id
    )
    .fetch_one(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("User not found"))?;

    // Initialize services
    let openrouter = OpenRouterService::new(std::env::var("OPENROUTER_API_KEY")?);
    let deepseek = DeepSeekService::new(std::env::var("DEEPSEEK_API_KEY")?);
    let solver = SolverService::new(state.pool.clone(), openrouter, deepseek);

    // Get solver state
    let mut solver_state = solver
        .get_solver(solver_id)
        .await?
        .ok_or_else(|| anyhow!("Solver not found"))?;

    // If we're in Analyzing state and have the start parameter, begin generating changes
    if solver_state.status == SolverStatus::Analyzing && params.contains_key("start") {
        solver
            .start_generating_changes(&mut solver_state, "/tmp/repo")
            .await?;
    }

    // Calculate progress percentage based on state
    let progress_percent = match solver_state.status {
        SolverStatus::Analyzing => 25,
        SolverStatus::GeneratingChanges => 50,
        SolverStatus::ApplyingChanges => 75,
        SolverStatus::Complete => 100,
        SolverStatus::Error(_) => 0,
    };

    // Get status text and message
    let (status_text, status_message) = match solver_state.status {
        SolverStatus::Analyzing => ("Analyzing", "Analyzing repository files..."),
        SolverStatus::GeneratingChanges => ("Generating", "Generating code changes..."),
        SolverStatus::ApplyingChanges => ("Applying", "Applying code changes..."),
        SolverStatus::Complete => {
            // Clean up repository when complete
            if let Some(repo_path) = solver_state.repo_path.as_ref() {
                cleanup_temp_dir(&PathBuf::from(repo_path));
            }
            ("Complete", "Changes have been applied and PR created")
        }
        SolverStatus::Error(ref msg) => ("Error", msg),
    };

    // Format the status section
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="solver-status" style="statusContainer">
            <view style="statusHeader">
                <text style="statusTitle">Solving Issue #{}</text>
                <view style="statusBadge">
                    <text style="statusText">{}</text>
                </view>
            </view>
            <text style="statusText">{}</text>
            <view style="progressBar">
                <view style="progressFill" width="{}%" />
            </view>
            <behavior
                trigger="load"
                action="replace"
                href="/hyperview/solver/{}/status?github_id={}"
                target="solver-status"
                delay="2000"
            />
        </view>"#,
        solver_state.issue_number,
        status_text,
        status_message,
        progress_percent,
        solver_id,
        github_id
    );

    Ok(xml)
}

fn error_xml(message: &str) -> String {
    format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="solver_status" backgroundColor="black" flex="1" padding="16">
            <text color="red" fontSize="18">{}</text>
        </view>"#,
        html_escape::encode_text(message)
    )
}

#[allow(dead_code)]
pub async fn approve_change(
    State(state): State<AppState>,
    Path((solver_id, change_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!(
        "Handling change approval for solver {} change {}",
        solver_id, change_id
    );

    let result = approve_change_internal(state, &solver_id, &change_id, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => {
            error!("Failed to approve change: {}", e);
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

#[allow(dead_code)]
pub async fn reject_change(
    State(state): State<AppState>,
    Path((solver_id, change_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!(
        "Handling change rejection for solver {} change {}",
        solver_id, change_id
    );

    let result = reject_change_internal(state, &solver_id, &change_id, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => {
            error!("Failed to reject change: {}", e);
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

async fn approve_change_internal(
    state: AppState,
    solver_id: &str,
    change_id: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Verify user exists and has access
    let _exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE github_id = $1)",
        github_id
    )
    .fetch_one(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("User not found"))?;

    // Get API keys from environment
    let openrouter_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow!("OPENROUTER_API_KEY environment variable not set"))?;
    let deepseek_key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| anyhow!("DEEPSEEK_API_KEY environment variable not set"))?;

    // Initialize services
    let openrouter = OpenRouterService::new(openrouter_key);
    let deepseek = DeepSeekService::new(deepseek_key);
    let solver = SolverService::new(state.pool.clone(), openrouter, deepseek);

    // Get solver state
    let mut solver_state = solver
        .get_solver(solver_id)
        .await?
        .ok_or_else(|| anyhow!("Solver not found"))?;

    // Approve the change
    solver.approve_change(&mut solver_state, change_id).await?;

    // Return to status page
    Ok(format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="solver_status">
            <behavior
                trigger="load"
                action="replace"
                href="/hyperview/solver/{}/status?github_id={}"
                target="solver_status"
            />
        </view>"#,
        solver_id, github_id
    ))
}

async fn reject_change_internal(
    state: AppState,
    solver_id: &str,
    change_id: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Verify user exists and has access
    let _exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE github_id = $1)",
        github_id
    )
    .fetch_one(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("User not found"))?;

    // Get API keys from environment
    let openrouter_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow!("OPENROUTER_API_KEY environment variable not set"))?;
    let deepseek_key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| anyhow!("DEEPSEEK_API_KEY environment variable not set"))?;

    // Initialize services
    let openrouter = OpenRouterService::new(openrouter_key);
    let deepseek = DeepSeekService::new(deepseek_key);
    let solver = SolverService::new(state.pool.clone(), openrouter, deepseek);

    // Get solver state
    let mut solver_state = solver
        .get_solver(solver_id)
        .await?
        .ok_or_else(|| anyhow!("Solver not found"))?;

    // Reject the change
    solver.reject_change(&mut solver_state, change_id).await?;

    // Return to status page
    Ok(format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="solver_status">
            <behavior
                trigger="load"
                action="replace"
                href="/hyperview/solver/{}/status?github_id={}"
                target="solver_status"
            />
        </view>"#,
        solver_id, github_id
    ))
}

pub async fn solver_files(
    State(state): State<AppState>,
    Path(solver_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!("Handling solver files request for {}", solver_id);

    let result = solver_files_internal(state, &solver_id, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => {
            error!("Failed to get solver files: {}", e);
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

async fn solver_files_internal(
    state: AppState,
    solver_id: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Verify user exists and has access
    let _exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE github_id = $1)",
        github_id
    )
    .fetch_one(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("User not found"))?;

    // Initialize services
    let openrouter = OpenRouterService::new(std::env::var("OPENROUTER_API_KEY")?);
    let deepseek = DeepSeekService::new(std::env::var("DEEPSEEK_API_KEY")?);
    let solver = SolverService::new(state.pool.clone(), openrouter, deepseek);

    // Get solver state
    let solver_state = solver
        .get_solver(solver_id)
        .await?
        .ok_or_else(|| anyhow!("Solver not found"))?;

    // Format the files section
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="file-changes" style="changesContainer">
            <text style="sectionTitle">File Changes</text>
            <view style="fileList">
                <list id="files">
                    {}
                </list>
            </view>
            <behavior
                trigger="load"
                action="replace"
                href="/hyperview/solver/{}/files?github_id={}"
                target="file-changes"
                delay="2000"
            />
        </view>"#,
        if solver_state.files.is_empty() {
            r#"<item key="empty">
                <view style="emptyState">
                    <text style="emptyText">No files have been analyzed yet</text>
                </view>
            </item>"#.to_string()
        } else {
            solver_state
                .files
                .iter()
                .map(|file| {
                    format!(
                        r#"<item key="{}">
                            <view style="fileItem">
                                <local:svg src="file" style="fileIcon" />
                                <text style="fileName">{}</text>
                                <text style="fileStatus">{}</text>
                            </view>
                        </item>"#,
                        html_escape::encode_text(&file.path),
                        html_escape::encode_text(&file.path),
                        if file.changes.is_empty() {
                            "Analyzing"
                        } else {
                            "Changes Ready"
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
        solver_id,
        github_id
    );

    Ok(xml)
}

pub async fn solver_diffs(
    State(state): State<AppState>,
    Path(solver_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!("Handling solver diffs request for {}", solver_id);

    let result = solver_diffs_internal(state, &solver_id, &params).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => {
            error!("Failed to get solver diffs: {}", e);
            Response::builder()
                .header("Content-Type", "application/vnd.hyperview+xml")
                .body(error_xml(&e.to_string()).into())
                .unwrap()
        }
    }
}

async fn solver_diffs_internal(
    state: AppState,
    solver_id: &str,
    params: &HashMap<String, String>,
) -> Result<String> {
    // Get GitHub ID from params
    let github_id = params
        .get("github_id")
        .ok_or_else(|| anyhow!("GitHub ID not provided"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Invalid GitHub ID"))?;

    // Verify user exists and has access
    let _exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE github_id = $1)",
        github_id
    )
    .fetch_one(&state.pool)
    .await?
    .ok_or_else(|| anyhow!("User not found"))?;

    // Initialize services
    let openrouter = OpenRouterService::new(std::env::var("OPENROUTER_API_KEY")?);
    let deepseek = DeepSeekService::new(std::env::var("DEEPSEEK_API_KEY")?);
    let solver = SolverService::new(state.pool.clone(), openrouter, deepseek);

    // Get solver state
    let solver_state = solver
        .get_solver(solver_id)
        .await?
        .ok_or_else(|| anyhow!("Solver not found"))?;

    // Collect all changes from all files
    let mut all_changes = Vec::new();
    for file in &solver_state.files {
        for change in &file.changes {
            all_changes.push((file.path.clone(), change));
        }
    }

    // Format the diffs section
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="code-diffs" style="diffsContainer">
            <text style="sectionTitle">Generated Changes</text>
            <view style="diffList">
                <list id="diffs">
                    {}
                </list>
            </view>
            <behavior
                trigger="load"
                action="replace"
                href="/hyperview/solver/{}/diffs?github_id={}"
                target="code-diffs"
                delay="2000"
            />
        </view>"#,
        if all_changes.is_empty() {
            r#"<item key="empty">
                <view style="emptyState">
                    <text style="emptyText">No changes have been generated yet</text>
                </view>
            </item>"#.to_string()
        } else {
            all_changes
                .iter()
                .map(|(file_path, change)| {
                    format!(
                        r#"<item key="{}">
                            <view style="diffItem">
                                <view style="diffHeader">
                                    <text style="diffPath">{}</text>
                                    <view style="diffActions">
                                        <view style="actionButton approveButton">
                                            <behavior
                                                trigger="press"
                                                action="replace"
                                                href="/hyperview/solver/{}/approve/{}?github_id={}"
                                                target="code-diffs" />
                                            <text style="buttonText">Approve</text>
                                        </view>
                                        <view style="actionButton rejectButton">
                                            <behavior
                                                trigger="press"
                                                action="replace"
                                                href="/hyperview/solver/{}/reject/{}?github_id={}"
                                                target="code-diffs" />
                                            <text style="buttonText">Reject</text>
                                        </view>
                                    </view>
                                </view>
                                <view style="codeBlock">
                                    <list id="lines">
                                        <item key="search">
                                            <text style="codeLine removedLine">{}</text>
                                        </item>
                                        <item key="replace">
                                            <text style="codeLine addedLine">{}</text>
                                        </item>
                                    </list>
                                </view>
                            </view>
                        </item>"#,
                        change.id,
                        html_escape::encode_text(file_path),
                        solver_id,
                        change.id,
                        github_id,
                        solver_id,
                        change.id,
                        github_id,
                        html_escape::encode_text(&change.search),
                        html_escape::encode_text(&change.replace)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
        solver_id,
        github_id
    );

    Ok(xml)
}
