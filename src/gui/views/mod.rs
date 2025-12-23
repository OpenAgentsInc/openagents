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

    // Parallel agents pane (top-left) - collapsible
    let parallel_pane = r###"<div id="parallel-pane" style="position: fixed; top: 1rem; left: 1rem; background: #111; border: 1px solid #333; font-family: monospace; font-size: 0.75rem; max-width: 360px;">
        <div style="padding: 0.5rem 0.75rem; border-bottom: 1px solid #333; display: flex; justify-content: space-between; align-items: center; cursor: pointer;" onclick="document.getElementById('parallel-content').classList.toggle('hidden')">
            <span style="color: #4a9eff;">Parallel Agents</span>
            <span style="color: #666;">▼</span>
        </div>
        <div id="parallel-content" class="hidden" style="padding: 0.75rem;">
            <div id="parallel-status" hx-get="/api/parallel/status" hx-trigger="load, every 5s" hx-swap="innerHTML">
                <p style="color: #666;">Loading...</p>
            </div>
            <div id="parallel-feedback" style="margin-top: 0.5rem;"></div>
            <div style="margin-top: 0.75rem; display: flex; gap: 0.5rem;">
                <form hx-post="/api/parallel/start" hx-target="#parallel-feedback" hx-swap="innerHTML" style="display: flex; gap: 0.25rem; align-items: center;">
                    <select name="count" style="background: #000; color: #888; border: 1px solid #333; padding: 0.25rem; font-size: 0.7rem;">
                        <option value="1">1</option>
                        <option value="2">2</option>
                        <option value="3" selected>3</option>
                        <option value="5">5</option>
                        <option value="10">10</option>
                    </select>
                    <button type="submit" style="background: #1a3a1a; color: #7dff7d; border: 1px solid #2d5016; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.7rem;">Start</button>
                </form>
                <form hx-post="/api/parallel/stop" hx-target="#parallel-feedback" hx-swap="innerHTML">
                    <button type="submit" style="background: #3a1a1a; color: #ff7d7d; border: 1px solid #501616; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.7rem;">Stop All</button>
                </form>
            </div>
            <div style="margin-top: 0.75rem; padding-top: 0.5rem; border-top: 1px solid #333; color: #555; font-size: 0.6rem;">
                Requires: Docker running, images built
            </div>
        </div>
    </div>
    <style>
        .hidden { display: none !important; }
    </style>"###;

    // Unified control stack: WS indicator, Agent selector, Full Auto, Daemon, Claude (top to bottom)
    let control_stack = format!(
        r#"<div style="position: fixed; bottom: 1rem; right: 1rem; display: flex; flex-direction: column; gap: 12px; align-items: flex-end;">
            <div id="ws-indicator" style="background: #111; border: 1px solid #333; padding: 0.5rem 0.75rem; display: flex; align-items: center; gap: 0.35rem; font-size: 0.6rem; color: #666;">
                <span id="ws-dot" style="width: 6px; height: 6px; background: #555; display: inline-block;"></span>
                <span style="color: #888; text-transform: uppercase; letter-spacing: 0.05em;">WS</span>
            </div>
            {}
            <div style="background: #111; border: 1px solid #333; padding: 0.5rem 0.75rem;">{}</div>
            <div id="daemon-status"><div id="daemon-status-content">{}</div></div>
            <div id="claude-status" hx-get="/api/claude/status" hx-trigger="load, every 5s" hx-swap="innerHTML">{}</div>
        </div>"#,
        agent_selector.into_string(),
        switch.into_string(),
        daemon_status.build().into_string(),
        status.build().into_string()
    );

    let content = format!(
        r#"{}{}{}"#,
        parallel_pane,
        control_stack,
        chat_pane.into_string()
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout_with_token(&content, Some(auth_token.token())))
}
