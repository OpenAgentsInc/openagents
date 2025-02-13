use crate::server::config::AppState;
use crate::server::services::{
    solver::{SolverService, SolverStatus},
    deepseek::DeepSeekService,
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
        solver.start_generating_changes(&mut solver_state, "/tmp/repo").await?;
    }

    // Format the status page based on current state
    let status_text = match solver_state.status {
        SolverStatus::Analyzing => "Analyzing Files...",
        SolverStatus::GeneratingChanges => "Generating Changes...",
        SolverStatus::ReviewingChanges => "Review Changes",
        SolverStatus::ApplyingChanges => "Applying Changes...",
        SolverStatus::Complete => "Changes Complete",
        SolverStatus::Error(ref msg) => msg,
    };

    let content = match solver_state.status {
        SolverStatus::Analyzing | SolverStatus::GeneratingChanges => format!(
            r#"<text color="gray" marginBottom="16">{}</text>
               <behavior
                   trigger="load"
                   action="replace"
                   href="/hyperview/solver/{}/status?github_id={}"
                   target="solver_status"
                   delay="2000"
               />"#,
            status_text, solver_id, github_id
        ),
        SolverStatus::ReviewingChanges => {
            let mut changes_xml = String::new();
            for file in &solver_state.files {
                if !file.changes.is_empty() {
                    changes_xml.push_str(&format!(
                        r#"<view style="fileSection" marginBottom="16">
                            <text color="white" fontSize="18" marginBottom="8">{}</text>"#,
                        html_escape::encode_text(&file.path)
                    ));

                    for change in &file.changes {
                        changes_xml.push_str(&format!(
                            r#"<view style="change" backgroundColor="rgb(34,34,34)" padding="16" marginBottom="8" borderRadius="8">
                                <text color="white" fontFamily="monospace" fontSize="14" whiteSpace="pre">{}</text>
                                <text color="gray" marginTop="8">{}</text>
                                <view style="actions" flexDirection="row" marginTop="8">
                                    <text style="button" backgroundColor="green" padding="8" borderRadius="4" marginRight="8">
                                        <behavior
                                            trigger="press"
                                            action="replace"
                                            href="/hyperview/solver/{}/change/{}/approve?github_id={}"
                                            target="change_{}"
                                        />
                                        Approve
                                    </text>
                                    <text style="button" backgroundColor="red" padding="8" borderRadius="4">
                                        <behavior
                                            trigger="press"
                                            action="replace"
                                            href="/hyperview/solver/{}/change/{}/reject?github_id={}"
                                            target="change_{}"
                                        />
                                        Reject
                                    </text>
                                </view>
                            </view>"#,
                            html_escape::encode_text(&change.replace),
                            html_escape::encode_text(&change.analysis),
                            solver_id,
                            change.id,
                            github_id,
                            change.id,
                            solver_id,
                            change.id,
                            github_id,
                            change.id
                        ));
                    }
                    changes_xml.push_str("</view>");
                }
            }
            changes_xml
        },
        _ => format!(r#"<text color="gray" marginBottom="16">{}</text>"#, status_text),
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
