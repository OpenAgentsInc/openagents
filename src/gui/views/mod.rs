//! View templates for the unified GUI

mod layout;

use actix_web::{web, HttpResponse};
use maud::{Markup, PreEscaped, html};
use tracing::info;
use ui::{AgentInfo, AgentSelector, AgentType, ClaudeStatus, DaemonStatus, FullAutoSwitch};

use crate::gui::state::AppState;

pub use layout::base_layout_with_token;

/// Render the main chat area (inline, not modal)
fn render_main_chat() -> Markup {
    let btn_base = "inline-flex items-center gap-1 font-mono cursor-pointer transition-colors select-none px-3 py-1.5 text-xs";
    let btn_active = "bg-secondary text-foreground border border-border";
    let btn_inactive = "bg-transparent text-muted-foreground border border-transparent hover:bg-accent hover:text-foreground";

    html! {
        div class="flex flex-col h-full" {
            // Header bar
            div class="flex items-center justify-between px-4 py-2 border-b border-border bg-card flex-shrink-0" {
                span class="text-xs text-muted-foreground uppercase tracking-wider" { "Autopilot" }
                div class="flex items-center gap-2" {
                    div class="flex items-center border border-border" {
                        button id="chat-view-formatted" onclick="switchChatView('formatted')"
                            class={(btn_base) " " (btn_active)} { "Formatted" }
                        button id="chat-view-json" onclick="switchChatView('json')"
                            class={(btn_base) " " (btn_inactive)} { "Raw JSON" }
                        button id="chat-view-raw" onclick="switchChatView('raw')"
                            class={(btn_base) " " (btn_inactive)} { "Raw RLOG" }
                    }
                    div class="w-px h-4 bg-border" {}
                    button id="chat-copy-btn" onclick="copyCurrentView()"
                        class={(btn_base) " bg-transparent text-muted-foreground border border-border hover:bg-accent hover:text-foreground"}
                        title="Copy to clipboard" {
                        (PreEscaped(r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter"><rect x="9" y="9" width="13" height="13" rx="0" ry="0"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>"#))
                        span { "Copy" }
                    }
                }
            }
            // Content area
            div class="flex-1 overflow-hidden relative" {
                div id="chat-content-formatted" class="absolute inset-0 overflow-y-auto p-4" {
                    div class="text-sm text-muted-foreground" { "Waiting for autopilot output..." }
                }
                div id="chat-content-json" class="absolute inset-0 overflow-y-auto p-4 text-xs leading-relaxed hidden" {
                    div class="text-muted-foreground" { "Waiting for JSON events..." }
                }
                div id="chat-content-raw" class="absolute inset-0 overflow-y-auto p-4 text-xs leading-relaxed hidden" {
                    div class="text-muted-foreground" { "Waiting for autopilot output..." }
                }
            }
        }
        // Chat view switching and copy scripts
        (PreEscaped(r#"<script>
function switchChatView(mode) {
    const formattedContent = document.getElementById('chat-content-formatted');
    const jsonContent = document.getElementById('chat-content-json');
    const rawContent = document.getElementById('chat-content-raw');
    const formattedBtn = document.getElementById('chat-view-formatted');
    const jsonBtn = document.getElementById('chat-view-json');
    const rawBtn = document.getElementById('chat-view-raw');
    const activeClasses = ['bg-secondary', 'text-foreground', 'border', 'border-border'];
    const inactiveClasses = ['bg-transparent', 'text-muted-foreground', 'border', 'border-transparent', 'hover:bg-accent', 'hover:text-foreground'];
    function setActive(btn) { activeClasses.forEach(c => btn.classList.add(c)); inactiveClasses.forEach(c => btn.classList.remove(c)); }
    function setInactive(btn) { inactiveClasses.forEach(c => btn.classList.add(c)); activeClasses.forEach(c => btn.classList.remove(c)); }
    formattedContent.classList.add('hidden'); jsonContent.classList.add('hidden'); rawContent.classList.add('hidden');
    setInactive(formattedBtn); setInactive(jsonBtn); setInactive(rawBtn);
    if (mode === 'formatted') { formattedContent.classList.remove('hidden'); setActive(formattedBtn); }
    else if (mode === 'json') { jsonContent.classList.remove('hidden'); setActive(jsonBtn); }
    else if (mode === 'raw') { rawContent.classList.remove('hidden'); setActive(rawBtn); }
    localStorage.setItem('chatViewMode', mode);
}
function copyCurrentView() {
    const formattedContent = document.getElementById('chat-content-formatted');
    const jsonContent = document.getElementById('chat-content-json');
    const rawContent = document.getElementById('chat-content-raw');
    const btn = document.getElementById('chat-copy-btn');
    let content = formattedContent;
    if (!jsonContent.classList.contains('hidden')) content = jsonContent;
    else if (!rawContent.classList.contains('hidden')) content = rawContent;
    const text = content.innerText || content.textContent;
    navigator.clipboard.writeText(text).then(() => {
        const originalHTML = btn.innerHTML;
        btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"></polyline></svg><span>Copied!</span>';
        btn.classList.add('text-green');
        setTimeout(() => { btn.innerHTML = originalHTML; btn.classList.remove('text-green'); }, 1500);
    });
}
(function() {
    ['chat-content-formatted', 'chat-content-json', 'chat-content-raw'].forEach(id => {
        const el = document.getElementById(id);
        if (el) {
            const observer = new MutationObserver(() => { el.scrollTop = el.scrollHeight; });
            observer.observe(el, { childList: true, subtree: true });
        }
    });
    const saved = localStorage.getItem('chatViewMode');
    if (saved === 'formatted' || saved === 'json' || saved === 'raw') switchChatView(saved);
})();
</script>"#))
        (PreEscaped(r#"<style>
#chat-content-formatted.hidden, #chat-content-json.hidden, #chat-content-raw.hidden { display: none !important; }
#chat-content-raw .log-line { color: var(--color-muted-foreground); }
#chat-content-raw .log-error { color: var(--color-red); }
#chat-content-raw .log-success { color: var(--color-green); }
#chat-content-json .json-line { color: var(--color-muted-foreground); word-break: break-all; }
</style>"#))
    }
}

/// Render the parallel agents pane for the right sidebar
fn render_parallel_pane() -> Markup {
    html! {
        div id="parallel-pane" class="flex flex-col border-b border-border" {
            // Header
            div class="px-3 py-2 border-b border-border flex justify-between items-center" {
                span class="text-xs text-primary" { "Parallel Agents" }
                div id="parallel-status" hx-get="/api/parallel/status" hx-trigger="load, every 2s" hx-swap="innerHTML"
                    class="text-xs text-muted-foreground" { "Loading..." }
            }
            // Controls
            div class="px-3 py-2 border-b border-border flex flex-col gap-2" {
                div class="flex gap-2 items-center" {
                    form hx-post="/api/parallel/start" hx-target="#parallel-feedback" hx-swap="innerHTML"
                        class="flex gap-1 items-center" {
                        select name="count" class="bg-background text-muted-foreground border border-border px-1 py-0.5 text-xs" {
                            option value="1" { "1" }
                            option value="2" { "2" }
                            option value="3" selected { "3" }
                        }
                        button type="submit" class="bg-green/20 text-green border border-green/30 px-2 py-0.5 cursor-pointer text-xs" { "Start" }
                    }
                    form hx-post="/api/parallel/stop" hx-target="#parallel-feedback" hx-swap="innerHTML" {
                        button type="submit" class="bg-red/20 text-red border border-red/30 px-2 py-0.5 cursor-pointer text-xs" { "Stop" }
                    }
                    span id="parallel-feedback" class="text-xs text-muted-foreground" {}
                }
                div class="flex justify-between items-center" {
                    div class="flex border border-border" {
                        button id="parallel-view-rlog" onclick="switchParallelView('rlog')"
                            class="bg-secondary text-primary border-none px-2 py-0.5 cursor-pointer text-xs" { "RLOG" }
                        button id="parallel-view-jsonl" onclick="switchParallelView('jsonl')"
                            class="bg-transparent text-muted-foreground border-none px-2 py-0.5 cursor-pointer text-xs" { "JSONL" }
                        button id="parallel-view-formatted" onclick="switchParallelView('formatted')"
                            class="bg-transparent text-muted-foreground border-none px-2 py-0.5 cursor-pointer text-xs" { "Fmt" }
                    }
                    button id="parallel-copy-btn" onclick="copyParallelLogs()"
                        class="bg-transparent text-muted-foreground border border-border px-2 py-0.5 cursor-pointer text-xs flex items-center gap-1" {
                        (PreEscaped(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>"#))
                        "Copy"
                    }
                }
            }
            // Log views
            div id="parallel-logs-rlog" hx-get="/api/parallel/logs/001?format=rlog" hx-trigger="load, every 1s" hx-swap="innerHTML"
                class="flex-1 overflow-y-auto p-2 bg-card text-xs text-muted-foreground whitespace-pre-wrap break-all max-h-[200px]" {
                span class="text-muted-foreground/50" { "Waiting for logs..." }
            }
            div id="parallel-logs-jsonl" hx-get="/api/parallel/logs/001?format=jsonl" hx-trigger="load, every 1s" hx-swap="innerHTML"
                class="hidden flex-1 overflow-y-auto p-2 bg-card text-xs text-muted-foreground whitespace-pre-wrap break-all max-h-[200px]" {
                span class="text-muted-foreground/50" { "Waiting for logs..." }
            }
            div id="parallel-logs-formatted" hx-get="/api/parallel/logs/001?format=formatted" hx-trigger="load, every 1s" hx-swap="innerHTML"
                class="hidden flex-1 overflow-y-auto p-2 bg-card text-xs text-muted-foreground max-h-[200px]" {
                span class="text-muted-foreground/50" { "Waiting for logs..." }
            }
        }
        // Parallel pane scripts
        (PreEscaped(r#"<script>
function switchParallelView(mode) {
    const rlog = document.getElementById('parallel-logs-rlog');
    const jsonl = document.getElementById('parallel-logs-jsonl');
    const formatted = document.getElementById('parallel-logs-formatted');
    const rlogBtn = document.getElementById('parallel-view-rlog');
    const jsonlBtn = document.getElementById('parallel-view-jsonl');
    const formattedBtn = document.getElementById('parallel-view-formatted');
    rlog.classList.add('hidden'); jsonl.classList.add('hidden'); formatted.classList.add('hidden');
    rlogBtn.classList.remove('bg-secondary', 'text-primary'); rlogBtn.classList.add('bg-transparent', 'text-muted-foreground');
    jsonlBtn.classList.remove('bg-secondary', 'text-primary'); jsonlBtn.classList.add('bg-transparent', 'text-muted-foreground');
    formattedBtn.classList.remove('bg-secondary', 'text-primary'); formattedBtn.classList.add('bg-transparent', 'text-muted-foreground');
    if (mode === 'rlog') { rlog.classList.remove('hidden'); rlogBtn.classList.add('bg-secondary', 'text-primary'); rlogBtn.classList.remove('bg-transparent', 'text-muted-foreground'); }
    else if (mode === 'jsonl') { jsonl.classList.remove('hidden'); jsonlBtn.classList.add('bg-secondary', 'text-primary'); jsonlBtn.classList.remove('bg-transparent', 'text-muted-foreground'); }
    else if (mode === 'formatted') { formatted.classList.remove('hidden'); formattedBtn.classList.add('bg-secondary', 'text-primary'); formattedBtn.classList.remove('bg-transparent', 'text-muted-foreground'); }
    localStorage.setItem('parallelViewMode', mode);
}
function copyParallelLogs() {
    const rlog = document.getElementById('parallel-logs-rlog');
    const jsonl = document.getElementById('parallel-logs-jsonl');
    const formatted = document.getElementById('parallel-logs-formatted');
    const btn = document.getElementById('parallel-copy-btn');
    let content = rlog;
    if (!jsonl.classList.contains('hidden')) content = jsonl;
    if (!formatted.classList.contains('hidden')) content = formatted;
    const text = content.innerText || content.textContent;
    navigator.clipboard.writeText(text).then(() => {
        btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"></polyline></svg>Copied!';
        btn.style.color = 'var(--color-green)';
        setTimeout(() => {
            btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>Copy';
            btn.style.color = '';
        }, 1500);
    });
}
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
    let parallel_pane = render_parallel_pane();

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

            // Right Sidebar (260px) - parallel agents + controls
            aside class="w-[260px] flex-shrink-0 border-l border-border bg-background flex flex-col" {
                // Parallel agents pane
                (parallel_pane)

                // Control stack
                div class="flex-1 flex flex-col gap-3 p-3" {
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
        }
        // Global styles
        (PreEscaped(r#"<style>.hidden { display: none !important; }</style>"#))
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout_with_token(&content.into_string(), Some(auth_token.token())))
}
