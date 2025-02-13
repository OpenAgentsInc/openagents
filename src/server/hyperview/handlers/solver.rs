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

    // If we're in Analyzing state and have the start parameter, begin generating changes
    if solver_state.status == SolverStatus::Analyzing && params.contains_key("start") {
        solver
            .start_generating_changes(&mut solver_state, "/tmp/repo")
            .await?;
    }

    // Format the status page based on current state
    let status_text = match solver_state.status {
        SolverStatus::Analyzing => "Analyzing Files...",
        SolverStatus::GeneratingChanges => "Generating Changes...",
        SolverStatus::ApplyingChanges => "Applying Changes...",
        SolverStatus::Complete => {
            // Clean up repository when complete
            if let Some(repo_path) = solver_state.repo_path.as_ref() {
                cleanup_temp_dir(&PathBuf::from(repo_path));
            }
            "Changes Complete - Pull Request Created"
        }
        SolverStatus::Error(ref msg) => msg,
    };

    let content = if solver_state.status == SolverStatus::Complete {
        format!(
            r#"<text color="gray" marginBottom="16">{}</text>
               <text color="white" backgroundColor="gray" padding="8" borderRadius="4" marginTop="16">
                   <behavior
                       trigger="press"
                       action="replace"
                       href="/hyperview/repo/{}/issues?github_id={}"
                       target="solver_status"
                   />
                   Back to Issues
               </text>"#,
            status_text,
            "owner", // TODO: Add owner/repo to solver state
            github_id
        )
    } else {
        format!(
            r#"<text color="gray" marginBottom="16">{}</text>
               <behavior
                   trigger="load"
                   action="replace"
                   href="/hyperview/solver/{}/status?github_id={}"
                   target="solver_status"
                   delay="2000"
               />"#,
            status_text, solver_id, github_id
        )
    };

    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="solver_status" backgroundColor="black" flex="1" padding="16">
            <text color="white" fontSize="24" marginBottom="16">{}</text>
            <view scroll="true" scroll-orientation="vertical" shows-scroll-indicator="true">
                {}
            </view>
        </view>"#,
        status_text, content
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
