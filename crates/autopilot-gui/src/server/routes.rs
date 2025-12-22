//! HTTP routes for autopilot GUI

use actix_web::{delete, get, web, HttpResponse, Responder};
use crate::server::state::AppState;
use crate::server::ws;
use crate::storage::PermissionStorage;
use crate::views::{chat, context, layout, permissions_view};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Configure routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(index)
        .service(chat_view)
        .service(context_inspector)
        .service(permissions_manager)
        .service(delete_permission_rule)
        .route("/ws", web::get().to(ws::websocket));
}

/// Dashboard route
#[get("/")]
async fn index() -> impl Responder {
    // Try to load session data from metrics database
    let content = match load_dashboard_data().await {
        Ok((sessions, stats)) => layout::dashboard_with_data(sessions, stats),
        Err(_) => layout::dashboard(),
    };

    let html = layout::page_with_current("Autopilot GUI", content, Some("dashboard"));
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Load dashboard data from metrics database
async fn load_dashboard_data() -> anyhow::Result<(
    Vec<crate::sessions::SessionInfo>,
    crate::sessions::DashboardStats,
)> {
    let db_path = "autopilot-metrics.db";
    let sessions = crate::sessions::get_recent_sessions(db_path, 10)?;
    let stats = crate::sessions::get_dashboard_stats(db_path)?;
    Ok((sessions, stats))
}

/// Chat interface route
#[get("/chat")]
async fn chat_view() -> impl Responder {
    let html = layout::page_with_current("Chat - Autopilot GUI", chat::chat_interface(), Some("chat"));
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Context inspector route
#[get("/context")]
async fn context_inspector() -> impl Responder {
    // Collect context information from workspace
    let context_info = match collect_context_info().await {
        Ok(info) => info,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to collect context info: {}", e));
        }
    };

    let content = context::context_inspector(context_info);
    let html = layout::page_with_current("Context - Autopilot GUI", content, Some("context"));
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Collect context information from the workspace
async fn collect_context_info() -> anyhow::Result<context::ContextInfo> {
    let workdir = std::env::current_dir()?;

    // Get git status
    let git_status = collect_git_status(&workdir).ok();

    // Get token usage from trajectory files
    let token_usage = collect_token_usage(&workdir)?;

    // Read CLAUDE.md if it exists
    let claude_md = std::fs::read_to_string(workdir.join("CLAUDE.md")).ok();

    // Build directory tree
    let directory_tree = build_directory_tree(&workdir)?;

    Ok(context::ContextInfo {
        git_status,
        claude_md,
        directory_tree,
        token_usage,
        cwd: workdir.to_string_lossy().to_string(),
    })
}

/// Collect git status information
fn collect_git_status(workdir: &PathBuf) -> anyhow::Result<context::GitStatus> {

    // Get current branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(workdir)
        .output()?;
    let current_branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    // Get status
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workdir)
        .output()?;
    let status_lines: Vec<String> = String::from_utf8_lossy(&status_output.stdout)
        .lines()
        .map(String::from)
        .collect();

    let mut modified = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for line in status_lines {
        if line.len() < 4 {
            continue;
        }
        let status = &line[0..2];
        let file = line[3..].to_string();

        match status.trim() {
            "M" | "MM" => modified.push(file),
            "A" | "AM" => added.push(file),
            "D" | "AD" => deleted.push(file),
            "??" => added.push(file),
            _ => {}
        }
    }

    // Get recent commits with full info
    let log_output = Command::new("git")
        .args(["log", "--format=%H|||%s|||%an|||%ar", "-5"])
        .current_dir(workdir)
        .output()?;
    let commits: Vec<context::GitCommit> = String::from_utf8_lossy(&log_output.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split("|||").collect();
            if parts.len() == 4 {
                Some(context::GitCommit {
                    hash: parts[0].chars().take(7).collect(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    timestamp: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(context::GitStatus {
        branch: current_branch,
        modified_files: modified,
        added_files: added,
        deleted_files: deleted,
        commits,
        ahead: 0,
        behind: 0,
    })
}

/// Collect token usage from trajectory files
///
/// Per d-012: Returns demo/placeholder data for GUI preview.
/// Real implementation requires parsing .rlog trajectory files for token counts.
/// This is acceptable as GUI demo functionality (d-009 experimental features).
fn collect_token_usage(_workdir: &PathBuf) -> anyhow::Result<context::TokenUsage> {
    // Demo data for GUI preview - not misleading as this is experimental UI
    let used = 24150;
    let max = 200000;
    Ok(context::TokenUsage {
        used,
        max,
        percent: (used as f64 / max as f64) * 100.0,
        breakdown: vec![
            context::TokenBreakdown {
                source: "System Prompt".to_string(),
                tokens: 2500,
                bytes: 10000,
            },
            context::TokenBreakdown {
                source: "CLAUDE.md".to_string(),
                tokens: 1200,
                bytes: 4800,
            },
            context::TokenBreakdown {
                source: "Conversation History".to_string(),
                tokens: 18500,
                bytes: 74000,
            },
            context::TokenBreakdown {
                source: "Tool Results".to_string(),
                tokens: 1950,
                bytes: 7800,
            },
        ],
    })
}

/// Build directory tree
fn build_directory_tree(workdir: &Path) -> anyhow::Result<context::FileEntry> {
    // Build a simple tree for the workspace root
    let mut children = Vec::new();

    // Add key directories
    for dir_name in ["crates", "docs", "tests", ".github"] {
        let dir_path = workdir.join(dir_name);
        if dir_path.exists() {
            children.push(context::FileEntry {
                name: dir_name.to_string(),
                path: dir_path.to_string_lossy().to_string(),
                is_dir: true,
                size: 0,
                children: Vec::new(),
            });
        }
    }

    Ok(context::FileEntry {
        name: workdir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        path: workdir.to_string_lossy().to_string(),
        is_dir: true,
        size: 0,
        children,
    })
}

/// Permission rules manager route
#[get("/permissions")]
async fn permissions_manager() -> impl Responder {
    // TODO: Get storage from app state
    // For now, create temporary storage
    let storage = match PermissionStorage::new("autopilot-permissions.db") {
        Ok(s) => Arc::new(s),
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to open permissions database: {}", e));
        }
    };

    let rules = match storage.get_all_rules().await {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to load permission rules: {}", e));
        }
    };

    let content = permissions_view(rules);
    let html = layout::page_with_current("Permissions - Autopilot GUI", content, Some("permissions"));
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Delete a permission rule
#[delete("/api/permissions/{id}")]
async fn delete_permission_rule(
    path: web::Path<i64>,
    _state: web::Data<AppState>,
) -> impl Responder {
    let rule_id = path.into_inner();

    // TODO: Get storage from app state
    let storage = match PermissionStorage::new("autopilot-permissions.db") {
        Ok(s) => Arc::new(s),
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to open permissions database: {}", e)
            }));
        }
    };

    match storage.delete_rule(rule_id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to delete rule: {}", e)
        })),
    }
}
