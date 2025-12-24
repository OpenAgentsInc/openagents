//! View templates for the unified GUI

mod layout;

use actix_web::{web, HttpResponse};
use maud::{Markup, PreEscaped, html};
use tracing::info;
use ui::{AgentInfo, AgentSelector, AgentType, ClaudeStatus, DaemonStatus, FullAutoSwitch};

use crate::gui::state::AppState;

pub use layout::base_layout_with_token;

/// Render the main chat area - ACP format only, max 768px centered
fn render_main_chat() -> Markup {
    html! {
        div class="flex flex-col h-full" {
            // Simple header
            div class="px-4 py-2 border-b border-border bg-card flex-shrink-0" {
                span class="text-xs text-muted-foreground uppercase tracking-wider" { "Autopilot" }
            }

            // Content area - centered, max 768px
            div class="flex-1 overflow-y-auto" {
                div id="chat-content-formatted" class="max-w-[768px] mx-auto p-4" {
                    div class="text-sm text-muted-foreground" { "Waiting for autopilot output..." }
                }
            }
        }

        // Auto-scroll script
        (PreEscaped(r#"<script>
(function() {
    const el = document.getElementById('chat-content-formatted');
    if (el) {
        const observer = new MutationObserver(() => { el.scrollTop = el.scrollHeight; });
        observer.observe(el, { childList: true, subtree: true });
    }
})();
</script>"#))
    }
}

/// Home page - 3-column layout: left sidebar (chats), main area (chat view), right sidebar (controls)
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
            *agent_availability.get("claude").unwrap_or(&true),
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
    if let Some(ref model) = info.model { status = status.model(model.clone()); }
    if let Some(ref version) = info.version { status = status.version(version.clone()); }
    if let Some(sessions) = info.total_sessions { status = status.total_sessions(sessions); }
    if let Some(messages) = info.total_messages { status = status.total_messages(messages); }
    if let Some(tokens) = info.today_tokens { status = status.today_tokens(tokens); }
    for usage in &info.model_usage {
        status = status.add_model_usage(
            usage.model.clone(), usage.input_tokens, usage.output_tokens,
            usage.cache_read_tokens, usage.cache_creation_tokens,
            usage.web_search_requests, usage.cost_usd, usage.context_window,
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
        if let Some(pid) = daemon_info.worker_pid { ds = ds.worker_pid(pid); }
        ds
    } else {
        let mut ds = DaemonStatus::disconnected();
        if let Some(ref err) = daemon_info.error { ds = ds.error(err.clone()); }
        ds
    };

    // Build the 3-column layout
    let main_chat = render_main_chat();

    let content = html! {
        div class="flex h-screen w-full" {
            // Left Sidebar (260px) - chat list (empty for now)
            aside class="w-[260px] flex-shrink-0 border-r border-border bg-background flex flex-col" {
                div class="px-3 py-2 border-b border-border" {
                    span class="text-xs text-muted-foreground uppercase tracking-wider" { "Chats" }
                }
                div class="flex-1 p-3" {
                    div class="text-xs text-muted-foreground/50" { "No chats yet" }
                }
            }

            // Main Area (fill) - chat view
            main class="flex-1 flex flex-col min-w-0 bg-background" {
                (main_chat)
            }

            // Right Sidebar (260px) - controls only
            aside class="w-[260px] flex-shrink-0 border-l border-border bg-background flex flex-col p-3 gap-3" {
                // WS indicator
                div id="ws-indicator" class="bg-card border border-border px-3 py-2 flex items-center gap-2 text-xs text-muted-foreground" {
                    span id="ws-dot" class="w-1.5 h-1.5 bg-muted-foreground inline-block" {}
                    span id="ws-label" class="uppercase tracking-wider" { "WS" }
                }
                // Agent selector
                (PreEscaped(agent_selector.into_string()))
                // Full Auto switch
                div class="bg-card border border-border px-3 py-2" {
                    (PreEscaped(switch.into_string()))
                }
                // Spacer
                div class="flex-1" {}
                // Daemon status
                div id="daemon-status" {
                    div id="daemon-status-content" {
                        (PreEscaped(daemon_status.build().into_string()))
                    }
                }
                // Claude status
                div id="claude-status" hx-get="/api/claude/status" hx-trigger="load, every 5s" hx-swap="innerHTML" {
                    (PreEscaped(status.build().into_string()))
                }
            }
        }
        // Global styles
        (PreEscaped(r#"<style>.hidden { display: none !important; }</style>"#))
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout_with_token(&content.into_string(), Some(auth_token.token())))
}
