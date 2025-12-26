//! Agent selection API routes
//!
//! Provides endpoints for listing available agents and selecting the active agent.

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ui::{AgentInfo, AgentSelector, AgentType};

use crate::gui::state::AppState;

/// Configure agent API routes
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(list_agents))
        .route("/select", web::post().to(select_agent))
        .route("/current", web::get().to(get_current));
}

/// Agent list response
#[derive(Serialize)]
struct AgentListResponse {
    selected: String,
    agents: Vec<AgentResponse>,
}

/// Individual agent response
#[derive(Serialize)]
struct AgentResponse {
    id: String,
    name: String,
    available: bool,
}

/// Select agent request
#[derive(Deserialize)]
struct SelectAgentRequest {
    agent: String,
}

/// List available agents with their availability status
async fn list_agents(state: web::Data<AppState>) -> HttpResponse {
    let selected = state.selected_agent.read().await.clone();
    let availability = check_all_agents().await;

    // Update cache
    {
        let mut cache = state.agent_availability.write().await;
        for (id, available) in &availability {
            cache.insert(id.clone(), *available);
        }
    }

    let response = AgentListResponse {
        selected: selected.clone(),
        agents: vec![
            AgentResponse {
                id: "claude".to_string(),
                name: "Claude Code".to_string(),
                available: *availability.get("claude").unwrap_or(&false),
            },
            AgentResponse {
                id: "codex".to_string(),
                name: "Codex".to_string(),
                available: *availability.get("codex").unwrap_or(&false),
            },
            AgentResponse {
                id: "gpt-oss".to_string(),
                name: "GPT-OSS".to_string(),
                available: *availability.get("gpt-oss").unwrap_or(&false),
            },
        ],
    };

    HttpResponse::Ok().json(response)
}

/// Select an agent
async fn select_agent(
    state: web::Data<AppState>,
    body: web::Json<SelectAgentRequest>,
) -> HttpResponse {
    let agent_id = body.agent.to_lowercase();

    // Validate agent
    if agent_id != "claude" && agent_id != "codex" && agent_id != "gpt-oss" {
        return HttpResponse::BadRequest().body(format!("Unknown agent: {}", body.agent));
    }

    // Check availability
    let available = check_agent_available(&agent_id).await;
    if !available {
        return HttpResponse::BadRequest()
            .body(format!("Agent '{}' is not available (not installed)", agent_id));
    }

    // Update selected agent
    {
        let mut selected = state.selected_agent.write().await;
        *selected = agent_id.clone();
    }

    // Update availability cache
    let availability = check_all_agents().await;
    {
        let mut cache = state.agent_availability.write().await;
        for (id, avail) in &availability {
            cache.insert(id.clone(), *avail);
        }
    }

    // Return updated selector component
    let agent_type = AgentType::from_str(&agent_id).unwrap_or(AgentType::Claude);
    let agents = vec![
        AgentInfo::new(AgentType::Claude, *availability.get("claude").unwrap_or(&false)),
        AgentInfo::new(AgentType::Codex, *availability.get("codex").unwrap_or(&false)),
        AgentInfo::new(AgentType::GptOss, *availability.get("gpt-oss").unwrap_or(&false)),
    ];

    let selector = AgentSelector::new(agent_type).agents(agents);

    HttpResponse::Ok()
        .content_type("text/html")
        .body(selector.build().into_string())
}

/// Get current agent selection (for HTMX polling)
async fn get_current(state: web::Data<AppState>) -> HttpResponse {
    let selected = state.selected_agent.read().await.clone();
    let availability = check_all_agents().await;

    let agent_type = AgentType::from_str(&selected).unwrap_or(AgentType::Claude);
    let agents = vec![
        AgentInfo::new(AgentType::Claude, *availability.get("claude").unwrap_or(&false)),
        AgentInfo::new(AgentType::Codex, *availability.get("codex").unwrap_or(&false)),
        AgentInfo::new(AgentType::GptOss, *availability.get("gpt-oss").unwrap_or(&false)),
    ];

    let selector = AgentSelector::new(agent_type).agents(agents);

    HttpResponse::Ok()
        .content_type("text/html")
        .body(selector.build().into_string())
}

/// Check if a specific agent is available
async fn check_agent_available(agent: &str) -> bool {
    match agent {
        "claude" => find_claude_executable().is_some(),
        "codex" => find_codex_executable().is_some(),
        "gpt-oss" => check_gpt_oss_available().await,
        _ => false,
    }
}

/// Check availability of all agents
async fn check_all_agents() -> std::collections::HashMap<String, bool> {
    let mut result = std::collections::HashMap::new();
    result.insert("claude".to_string(), find_claude_executable().is_some());
    result.insert("codex".to_string(), find_codex_executable().is_some());
    result.insert("gpt-oss".to_string(), check_gpt_oss_available().await);
    result
}

/// Check if GPT-OSS (llama-server) is available
///
/// Checks for llama-server health endpoint at the configured URL
async fn check_gpt_oss_available() -> bool {
    let base_url = std::env::var("GPT_OSS_URL")
        .or_else(|_| std::env::var("LLAMACPP_URL"))
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Try the health endpoint
    match client.get(format!("{}/health", base_url)).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Find Claude Code executable
fn find_claude_executable() -> Option<PathBuf> {
    // Try which first
    if let Ok(path) = which::which("claude") {
        return Some(path);
    }

    // Try common locations
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.claude/local/claude", home),
        format!("{}/.npm-global/bin/claude", home),
        format!("{}/.local/bin/claude", home),
        "/usr/local/bin/claude".to_string(),
        "/opt/homebrew/bin/claude".to_string(),
    ];

    for path in &paths {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Find Codex executable
fn find_codex_executable() -> Option<PathBuf> {
    // Try which first
    if let Ok(path) = which::which("codex") {
        return Some(path);
    }

    // Try common locations
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.npm-global/bin/codex", home),
        format!("{}/.local/bin/codex", home),
        "/usr/local/bin/codex".to_string(),
        "/opt/homebrew/bin/codex".to_string(),
    ];

    for path in &paths {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
