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

    // Parallel agents pane (top-left) - with live log streaming and view toggle
    let parallel_pane = r###"<div id="parallel-pane" style="position: fixed; top: 1rem; left: 1rem; background: #111; border: 1px solid #333; font-family: monospace; font-size: 0.75rem; width: 700px; max-height: 80vh; display: flex; flex-direction: column;">
        <div style="padding: 0.5rem 0.75rem; border-bottom: 1px solid #333; display: flex; justify-content: space-between; align-items: center;">
            <span style="color: #4a9eff;">Parallel Agents</span>
            <div id="parallel-status" hx-get="/api/parallel/status" hx-trigger="load, every 2s" hx-swap="innerHTML" style="color: #666;">
                Loading...
            </div>
        </div>
        <div style="padding: 0.5rem 0.75rem; border-bottom: 1px solid #333; display: flex; gap: 0.5rem; align-items: center; justify-content: space-between;">
            <div style="display: flex; gap: 0.5rem; align-items: center;">
                <form hx-post="/api/parallel/start" hx-target="#parallel-feedback" hx-swap="innerHTML" style="display: flex; gap: 0.25rem; align-items: center;">
                    <select name="count" style="background: #000; color: #888; border: 1px solid #333; padding: 0.25rem; font-size: 0.7rem;">
                        <option value="1">1</option>
                        <option value="2">2</option>
                        <option value="3" selected>3</option>
                    </select>
                    <button type="submit" style="background: #1a3a1a; color: #7dff7d; border: 1px solid #2d5016; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.7rem;">Start</button>
                </form>
                <form hx-post="/api/parallel/stop" hx-target="#parallel-feedback" hx-swap="innerHTML">
                    <button type="submit" style="background: #3a1a1a; color: #ff7d7d; border: 1px solid #501616; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.7rem;">Stop</button>
                </form>
                <span id="parallel-feedback" style="color: #888;"></span>
            </div>
            <div style="display: flex; gap: 0.25rem; align-items: center;">
                <div style="display: flex; border: 1px solid #333;">
                    <button id="parallel-view-rlog" onclick="switchParallelView('rlog')" style="background: #222; color: #4a9eff; border: none; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.65rem;">RLOG</button>
                    <button id="parallel-view-jsonl" onclick="switchParallelView('jsonl')" style="background: transparent; color: #666; border: none; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.65rem;">JSONL</button>
                    <button id="parallel-view-formatted" onclick="switchParallelView('formatted')" style="background: transparent; color: #666; border: none; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.65rem;">Formatted</button>
                </div>
                <button id="parallel-copy-btn" onclick="copyParallelLogs()" style="background: transparent; color: #666; border: 1px solid #333; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.65rem; display: flex; align-items: center; gap: 0.25rem;">
                    <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>
                    Copy
                </button>
            </div>
        </div>
        <div id="parallel-logs-rlog"
             hx-get="/api/parallel/logs/001?format=rlog"
             hx-trigger="load, every 1s"
             hx-swap="innerHTML"
             style="flex: 1; overflow-y: auto; padding: 0.5rem; background: #0a0a0a; color: #888; font-size: 0.65rem; line-height: 1.4; white-space: pre-wrap; word-break: break-all; max-height: 500px;">
            <span style="color: #555;">Waiting for logs...</span>
        </div>
        <div id="parallel-logs-jsonl"
             hx-get="/api/parallel/logs/001?format=jsonl"
             hx-trigger="load, every 1s"
             hx-swap="innerHTML"
             style="flex: 1; overflow-y: auto; padding: 0.5rem; background: #0a0a0a; color: #888; font-size: 0.65rem; line-height: 1.4; white-space: pre-wrap; word-break: break-all; max-height: 500px; display: none;">
            <span style="color: #555;">Waiting for logs...</span>
        </div>
        <div id="parallel-logs-formatted"
             hx-get="/api/parallel/logs/001?format=formatted"
             hx-trigger="load, every 1s"
             hx-swap="innerHTML"
             style="flex: 1; overflow-y: auto; padding: 0.5rem; background: #0a0a0a; color: #888; font-size: 0.65rem; line-height: 1.4; max-height: 500px; display: none;">
            <span style="color: #555;">Waiting for logs...</span>
        </div>
    </div>
    <script>
    function switchParallelView(mode) {
        const rlog = document.getElementById('parallel-logs-rlog');
        const jsonl = document.getElementById('parallel-logs-jsonl');
        const formatted = document.getElementById('parallel-logs-formatted');
        const rlogBtn = document.getElementById('parallel-view-rlog');
        const jsonlBtn = document.getElementById('parallel-view-jsonl');
        const formattedBtn = document.getElementById('parallel-view-formatted');

        rlog.style.display = 'none';
        jsonl.style.display = 'none';
        formatted.style.display = 'none';
        rlogBtn.style.background = 'transparent';
        rlogBtn.style.color = '#666';
        jsonlBtn.style.background = 'transparent';
        jsonlBtn.style.color = '#666';
        formattedBtn.style.background = 'transparent';
        formattedBtn.style.color = '#666';

        if (mode === 'rlog') {
            rlog.style.display = 'block';
            rlogBtn.style.background = '#222';
            rlogBtn.style.color = '#4a9eff';
        } else if (mode === 'jsonl') {
            jsonl.style.display = 'block';
            jsonlBtn.style.background = '#222';
            jsonlBtn.style.color = '#4a9eff';
        } else if (mode === 'formatted') {
            formatted.style.display = 'block';
            formattedBtn.style.background = '#222';
            formattedBtn.style.color = '#4a9eff';
        }
        localStorage.setItem('parallelViewMode', mode);
    }
    function copyParallelLogs() {
        const rlog = document.getElementById('parallel-logs-rlog');
        const jsonl = document.getElementById('parallel-logs-jsonl');
        const formatted = document.getElementById('parallel-logs-formatted');
        const btn = document.getElementById('parallel-copy-btn');
        let content = rlog;
        if (jsonl.style.display !== 'none') content = jsonl;
        if (formatted.style.display !== 'none') content = formatted;
        const text = content.innerText || content.textContent;
        navigator.clipboard.writeText(text).then(() => {
            btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"></polyline></svg>Copied!';
            btn.style.color = '#7dff7d';
            setTimeout(() => {
                btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>Copy';
                btn.style.color = '#666';
            }, 1500);
        });
    }
    // Auto-scroll logs and restore view preference
    (function() {
        ['parallel-logs-rlog', 'parallel-logs-jsonl', 'parallel-logs-formatted'].forEach(id => {
            const el = document.getElementById(id);
            if (el) {
                const observer = new MutationObserver(() => { el.scrollTop = el.scrollHeight; });
                observer.observe(el, { childList: true, subtree: true });
            }
        });
        const saved = localStorage.getItem('parallelViewMode');
        if (saved) switchParallelView(saved);
    })();
    </script>
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
