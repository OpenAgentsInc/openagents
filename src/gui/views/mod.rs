//! View templates for the unified GUI

mod layout;

use actix_web::{web, HttpResponse};
use tracing::info;
use ui::{AgentInfo, AgentSelector, AgentType, ChatPane, ClaudeStatus, DaemonStatus, FullAutoSwitch};

use crate::gui::state::AppState;

pub use layout::base_layout_with_token;

/// Home page - black screen with FullAutoSwitch top-right, status panels bottom-right (daemon above Claude), ChatPane at bottom
pub async fn home(
    state: web::Data<AppState>,
    auth_token: web::Data<auth::AuthToken>,
) -> HttpResponse {
    info!("GET / home page requested");
    let full_auto = *state.full_auto.read().await;
    let switch = FullAutoSwitch::new(full_auto).build();

    // Get selected agent and availability
    let selected_agent = state.selected_agent.read().await.clone();
    let agent_availability = state.agent_availability.read().await.clone();

    let agent_type = AgentType::from_str(&selected_agent).unwrap_or(AgentType::Claude);
    let agents = vec![
        AgentInfo::new(
            AgentType::Claude,
            *agent_availability.get("claude").unwrap_or(&true), // Default to true for Claude
        ),
        AgentInfo::new(
            AgentType::Codex,
            *agent_availability.get("codex").unwrap_or(&false),
        ),
    ];
    let agent_selector = AgentSelector::new(agent_type).agents(agents).build();

    // Get Claude info from state
    let info = state.claude_info.read().await;

    let mut status = if info.loading {
        ClaudeStatus::loading()
    } else if info.authenticated {
        ClaudeStatus::authenticated()
    } else {
        ClaudeStatus::not_logged_in()
    };

    if let Some(ref model) = info.model {
        status = status.model(model.clone());
    }
    if let Some(ref version) = info.version {
        status = status.version(version.clone());
    }
    if let Some(sessions) = info.total_sessions {
        status = status.total_sessions(sessions);
    }
    if let Some(messages) = info.total_messages {
        status = status.total_messages(messages);
    }
    if let Some(tokens) = info.today_tokens {
        status = status.today_tokens(tokens);
    }

    // Add model usage
    for usage in &info.model_usage {
        status = status.add_model_usage(
            usage.model.clone(),
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens,
            usage.cache_creation_tokens,
            usage.web_search_requests,
            usage.cost_usd,
            usage.context_window,
        );
    }

    // Get daemon info from state
    let daemon_info = state.daemon_info.read().await;
    let daemon_status = if daemon_info.connected {
        let mut ds = DaemonStatus::connected()
            .worker_status(&daemon_info.worker_status)
            .uptime(daemon_info.uptime_seconds)
            .restarts(daemon_info.total_restarts, daemon_info.consecutive_failures)
            .memory(daemon_info.memory_available_bytes, daemon_info.memory_total_bytes);
        if let Some(pid) = daemon_info.worker_pid {
            ds = ds.worker_pid(pid);
        }
        ds
    } else {
        let mut ds = DaemonStatus::disconnected();
        if let Some(ref err) = daemon_info.error {
            ds = ds.error(err.clone());
        }
        ds
    };

    // Chat pane with Raw/Formatted toggle - visible when full_auto is ON
    let chat_pane = ChatPane::new(full_auto).build();

    // Stack daemon and Claude panels in a flex container with 12px gap
    let status_panels = format!(
        r#"<div style="position: fixed; bottom: 1rem; right: 1rem; display: flex; flex-direction: column; gap: 12px;">
            <div id="daemon-status"><div id="daemon-status-content">{}</div></div>
            <div id="claude-status" hx-get="/api/claude/status" hx-trigger="load, every 5s" hx-swap="innerHTML">{}</div>
        </div>"#,
        daemon_status.build().into_string(),
        status.build().into_string()
    );

    let content = format!(
        r#"<div style="position: fixed; top: 1rem; right: 1rem; z-index: 50; display: flex; gap: 1rem; align-items: center;">{}{}</div>{}{}"#,
        agent_selector.into_string(),
        switch.into_string(),
        status_panels,
        chat_pane.into_string()
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout_with_token(&content, Some(auth_token.token())))
}
