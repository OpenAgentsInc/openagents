//! Replay functionality for .rlog files

use actix_web::{HttpResponse, web};
use maud::{Markup, html};
use recorder::{LineType, ParsedLine, ParsedSession, parse_file};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use ui::recorder::atoms::step_badge;
use ui::recorder::molecules::{ResultType, SessionMode};
use ui::recorder::organisms::{UserLine, AgentLine, ToolLine, lifecycle_line, LifecycleEvent, phase_line, SubagentLine, McpLine};
use ui::recorder::sections::{SessionHeader, SessionStats, ToolIndex, session_sidebar};

use crate::ws::WsBroadcaster;
use crate::server::AppState;

/// Query parameters for replay route
#[derive(serde::Deserialize)]
pub struct ReplayQuery {
    pub path: String,
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_speed() -> f32 {
    1.0
}

/// Replay handler - parses .rlog file and streams events
pub async fn replay_handler(
    query: web::Query<ReplayQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let path = Path::new(&query.path);

    // Parse the .rlog file
    let session = match parse_file(path) {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest()
                .body(format!("Failed to parse .rlog file: {}", e));
        }
    };

    // Spawn async task to stream events
    let broadcaster = state.broadcaster.clone();
    let speed = query.speed;
    tokio::spawn(async move {
        stream_replay_events(session, broadcaster, speed).await;
    });

    // Return the autopilot replay page
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(replay_page().into_string())
}

/// Stream replay events via WebSocket broadcaster
async fn stream_replay_events(session: ParsedSession, broadcaster: Arc<WsBroadcaster>, speed: f32) {
    // First, broadcast session header
    let header = build_session_header(&session);
    broadcaster.broadcast(&format!(r#"<div id="session-header">{}</div>"#, header.build().into_string()));

    sleep(Duration::from_millis(500)).await;

    // Initialize stats tracking
    let mut cumulative_stats = CumulativeStats::default();
    let mut tool_index = std::collections::HashMap::new();

    // Stream each line
    for (idx, line) in session.lines.iter().enumerate() {
        // Update cumulative stats
        cumulative_stats.update(line);

        // Update tool index
        if matches!(line.line_type, LineType::Tool | LineType::ToolStart) {
            let tool_name = extract_tool_name(line);
            *tool_index.entry(tool_name).or_insert(0) += 1;
        }

        // Render the line as HTML
        let line_html = render_parsed_line(line, &session.header);

        // Broadcast the line
        broadcaster.broadcast(&line_html.into_string());

        // Update sidebar with current stats every 10 lines
        if idx % 10 == 0 || idx == session.lines.len() - 1 {
            let sidebar_html = render_sidebar(&session, &cumulative_stats, &tool_index);
            broadcaster.broadcast(&format!(r#"<aside id="session-sidebar" class="w-72 h-screen sticky top-14 overflow-y-auto border-r border-border flex-shrink-0">{}</aside>"#, sidebar_html.into_string()));
        }

        // Calculate delay based on speed (default 100ms between lines)
        let delay_ms = (100.0 / speed) as u64;
        sleep(Duration::from_millis(delay_ms)).await;
    }
}

/// Cumulative statistics tracker for replay
#[derive(Default)]
struct CumulativeStats {
    lines: usize,
    user_msgs: usize,
    agent_msgs: usize,
    tool_calls: usize,
    mcp_calls: usize,
    subagents: usize,
    questions: usize,
    phases: usize,
    total_tokens_in: u64,
    total_tokens_out: u64,
    total_tokens_cached: u64,
}

impl CumulativeStats {
    fn update(&mut self, line: &ParsedLine) {
        self.lines += 1;
        match line.line_type {
            LineType::User => self.user_msgs += 1,
            LineType::Agent => self.agent_msgs += 1,
            LineType::Tool | LineType::ToolStart => self.tool_calls += 1,
            LineType::Mcp => self.mcp_calls += 1,
            LineType::Subagent => self.subagents += 1,
            LineType::Question => self.questions += 1,
            LineType::Phase => self.phases += 1,
            _ => {}
        }

        if let Some(tokens) = line.tokens_in {
            self.total_tokens_in += tokens;
        }
        if let Some(tokens) = line.tokens_out {
            self.total_tokens_out += tokens;
        }
        if let Some(tokens) = line.tokens_cached {
            self.total_tokens_cached += tokens;
        }
    }

    fn cost(&self) -> f64 {
        // Rough cost estimation (adjust based on actual model pricing)
        let input_cost = (self.total_tokens_in as f64 / 1_000_000.0) * 3.0;
        let output_cost = (self.total_tokens_out as f64 / 1_000_000.0) * 15.0;
        let cache_cost = (self.total_tokens_cached as f64 / 1_000_000.0) * 0.3;
        input_cost + output_cost + cache_cost
    }
}

/// Build session header from parsed session
fn build_session_header(session: &ParsedSession) -> SessionHeader {
    let header = &session.header;
    let repo = header.repo.as_deref().unwrap_or("unknown/repo");
    let model = header.model.as_deref().unwrap_or("unknown");

    let mut session_header = SessionHeader::new(&header.id, model, repo)
        .sha(&header.repo_sha);

    if let Some(ref branch) = header.branch {
        session_header = session_header.branch(branch);
    }

    if let Some(ref mode) = header.mode {
        let mode_enum = match mode.as_str() {
            "auto" => SessionMode::Auto,
            "plan" => SessionMode::Plan,
            _ => SessionMode::Chat,
        };
        session_header = session_header.mode(mode_enum);
    }

    if let Some(ref mcp) = header.mcp {
        let mcp_refs: Vec<&str> = mcp.iter().map(|s| s.as_str()).collect();
        session_header = session_header.mcp(mcp_refs);
    }

    session_header
}

/// Render sidebar with current stats
fn render_sidebar(
    session: &ParsedSession,
    stats: &CumulativeStats,
    tool_index: &std::collections::HashMap<String, usize>,
) -> Markup {
    let header = build_session_header(session);

    let session_stats = SessionStats {
        lines: stats.lines as u32,
        duration: "00:00:00".to_string(), // TODO: Calculate from timestamps
        cost: stats.cost(),
        user_msgs: stats.user_msgs as u32,
        agent_msgs: stats.agent_msgs as u32,
        tool_calls: stats.tool_calls as u32,
        mcp_calls: stats.mcp_calls as u32,
        subagents: stats.subagents as u32,
        questions: stats.questions as u32,
        phases: stats.phases as u32,
        blobs: 0,
        redacted: 0,
    };

    let mut tool_idx = ToolIndex::new();
    for (tool_name, count) in tool_index.iter() {
        tool_idx = tool_idx.add(tool_name, *count as u32);
    }

    let mode = session.header.mode.as_deref().unwrap_or("chat");
    let mode_enum = match mode {
        "auto" => SessionMode::Auto,
        "plan" => SessionMode::Plan,
        _ => SessionMode::Chat,
    };

    session_sidebar(
        header,
        mode_enum,
        None,
        0.0,
        0.0,
        stats.cost(),
        Some(0.0),
        session_stats,
        tool_idx,
    )
}

/// Render a parsed line as HTML
fn render_parsed_line(line: &ParsedLine, _header: &recorder::Header) -> Markup {
    match line.line_type {
        LineType::User => {
            let mut user_line = UserLine::new(&line.content);
            if let Some(step) = line.step {
                user_line = user_line.step(step);
            }
            user_line.build()
        }

        LineType::Agent => {
            let mut agent_line = AgentLine::new(&line.content);
            if let Some(step) = line.step {
                agent_line = agent_line.step(step);
            }
            // Add metrics if available
            if line.tokens_in.is_some() || line.tokens_out.is_some() {
                agent_line = agent_line.metrics(
                    line.tokens_in.unwrap_or(0) as u32,
                    line.tokens_out.unwrap_or(0) as u32,
                    line.tokens_cached.map(|t| t as u32),
                    calculate_cost(line),
                );
            }
            agent_line.build()
        }

        LineType::Tool => {
            let (tool_name, args) = parse_tool_line(&line.content);
            let result = determine_result_type(line);

            let mut tool_line = ToolLine::new(&tool_name, &args, result);
            if let Some(step) = line.step {
                tool_line = tool_line.step(step);
            }
            if let Some(ref call_id) = line.call_id {
                tool_line = tool_line.call_id(call_id);
            }
            if let Some(latency) = line.latency_ms {
                tool_line = tool_line.latency(latency as u32);
            }
            tool_line.build()
        }

        LineType::ToolStart => {
            let (tool_name, args) = parse_tool_line(&line.content);

            let mut tool_line = ToolLine::new(&tool_name, &args, ResultType::Pending);
            if let Some(step) = line.step {
                tool_line = tool_line.step(step);
            }
            if let Some(ref call_id) = line.call_id {
                tool_line = tool_line.call_id(call_id);
            }
            tool_line.build()
        }

        LineType::Lifecycle => {
            // Parse lifecycle event type
            let event = if line.content.starts_with("start") {
                LifecycleEvent::Start {
                    id: line.call_id.clone().unwrap_or_else(|| "unknown".to_string()),
                    budget: 0.0,
                    duration: "0h".to_string(),
                }
            } else if line.content.starts_with("end") {
                LifecycleEvent::End {
                    summary: "Session completed".to_string(),
                    issues_completed: 0,
                    prs_merged: 0,
                    cost: 0.0,
                    duration: "0h".to_string(),
                }
            } else if line.content.starts_with("checkpoint") {
                LifecycleEvent::Checkpoint {
                    hour: 0,
                    tokens: 0,
                    cost: 0.0,
                    budget_total: 0.0,
                }
            } else {
                LifecycleEvent::Resume
            };
            lifecycle_line(event, line.step, None)
        }

        LineType::Phase => {
            phase_line(&line.content)
        }

        LineType::Subagent => {
            // Parse subagent line: "type task → summary"
            let (agent_type, task) = parse_tool_line(&line.content); // Reuse parser
            let mut subagent_line = SubagentLine::new(&agent_type, &task);
            if let Some(step) = line.step {
                subagent_line = subagent_line.step(step);
            }
            subagent_line.build()
        }

        LineType::Mcp => {
            let (server_method, args) = parse_mcp_line(&line.content);
            let result = determine_result_type(line);

            let mut mcp_line = McpLine::new(&server_method, &args, result);
            if let Some(step) = line.step {
                mcp_line = mcp_line.step(step);
            }
            if let Some(ref call_id) = line.call_id {
                mcp_line = mcp_line.call_id(call_id);
            }
            mcp_line.build()
        }

        LineType::Thinking => {
            // Render thinking block (could be collapsed)
            html! {
                div class="p-3 bg-card/50 border border-border/50 text-xs text-muted-foreground font-mono" {
                    span class="text-yellow" { "💭 " }
                    @if let Some(step) = line.step {
                        (step_badge(step))
                        " "
                    }
                    (line.content)
                }
            }
        }

        LineType::Comment => {
            html! {
                div class="text-xs text-muted-foreground/50 italic" {
                    "# " (line.content)
                }
            }
        }

        LineType::Empty => html! {},

        LineType::Continuation => {
            html! {
                div class="pl-8 text-xs text-muted-foreground" {
                    (line.content)
                }
            }
        }

        _ => {
            // Unknown or unhandled types
            html! {
                div class="p-2 bg-red/10 border border-red text-xs text-red" {
                    span class="font-bold" { (format!("{:?}", line.line_type)) }
                    " "
                    (line.content)
                }
            }
        }
    }
}

/// Parse tool line to extract name and args
fn parse_tool_line(content: &str) -> (String, String) {
    // Format: "ToolName args → result" or just "ToolName args"
    let parts: Vec<&str> = content.splitn(2, ' ').collect();
    if parts.len() >= 1 {
        let tool_name = parts[0].to_string();
        let args = if parts.len() > 1 {
            // Remove result part if present
            parts[1].split(" → ").next().unwrap_or(parts[1]).to_string()
        } else {
            String::new()
        };
        (tool_name, args)
    } else {
        (content.to_string(), String::new())
    }
}

/// Parse MCP line to extract server.method and args
fn parse_mcp_line(content: &str) -> (String, String) {
    // Format: "server.method args → result" or similar
    // For now, just use the same logic as tool lines
    parse_tool_line(content)
}

/// Extract tool name from parsed line
fn extract_tool_name(line: &ParsedLine) -> String {
    let (name, _) = parse_tool_line(&line.content);
    name
}

/// Determine result type from parsed line
fn determine_result_type(line: &ParsedLine) -> ResultType {
    if let Some(ref result) = line.result {
        let result_lower = result.to_lowercase();
        if result_lower.contains("ok") || result_lower.contains("success") {
            ResultType::Ok
        } else if result_lower.contains("error") || result_lower.contains("failed") {
            ResultType::Error(result.clone())
        } else if result_lower.contains("count") || result_lower.contains("matches") {
            // Parse count if possible
            ResultType::Count { count: 0, unit: "items".to_string() }
        } else {
            ResultType::Ok
        }
    } else {
        ResultType::Ok
    }
}

/// Calculate cost from token usage
fn calculate_cost(line: &ParsedLine) -> f64 {
    let input_cost = (line.tokens_in.unwrap_or(0) as f64 / 1_000_000.0) * 3.0;
    let output_cost = (line.tokens_out.unwrap_or(0) as f64 / 1_000_000.0) * 15.0;
    let cache_cost = (line.tokens_cached.unwrap_or(0) as f64 / 1_000_000.0) * 0.3;
    input_cost + output_cost + cache_cost
}

/// Replay page HTML (similar to autopilot_page but with replay controls)
fn replay_page() -> Markup {
    use maud::{DOCTYPE, PreEscaped};
    use ui::{TAILWIND_CDN, TAILWIND_THEME};

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Replay - OpenAgents" }

                // Inline Tailwind CSS (Play CDN)
                script { (PreEscaped(TAILWIND_CDN)) }
                // Custom theme
                style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
            }
            body class="bg-background text-foreground font-mono antialiased min-h-screen" {
                // Header with replay controls
                header class="fixed top-0 left-0 right-0 bg-card border-b border-border z-10" {
                    div class="px-4 py-3 flex items-center gap-4" {
                        span class="text-xs text-muted-foreground tracking-widest" { "REPLAY" }
                        span id="session-status" class="text-xs text-green" { "playing..." }
                        span class="flex-1" {}
                        // Auto-scroll toggle
                        label class="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer" {
                            input type="checkbox" id="auto-scroll" checked class="accent-green";
                            "Auto-scroll"
                        }
                    }
                }

                // Main content area with sidebar layout
                main class="pt-14 flex" {
                    // Left sidebar (fixed width)
                    aside id="session-sidebar" class="w-72 h-screen sticky top-14 overflow-y-auto border-r border-border flex-shrink-0" {
                        // Sidebar will be populated via WebSocket
                    }

                    // Main timeline area (flex-grow)
                    div class="flex-1 px-4 pb-4" {
                        // Session header
                        div id="session-header" class="mb-4" {
                            // Header will be updated via WebSocket
                        }

                        // Timeline container
                        div id="timeline" class="space-y-2" {
                            // Timeline events will be appended here
                        }
                    }
                }

                // WebSocket connection for replay updates
                (PreEscaped(r#"<script>
                (function() {
                    var ws = new WebSocket('ws://' + location.host + '/ws');
                    var autoScroll = document.getElementById('auto-scroll');
                    var timeline = document.getElementById('timeline');
                    var status = document.getElementById('session-status');
                    var sidebar = document.getElementById('session-sidebar');
                    var header = document.getElementById('session-header');

                    ws.onopen = function() {
                        status.textContent = 'playing...';
                        status.className = 'text-xs text-green';
                    };

                    ws.onmessage = function(e) {
                        var fragment = e.data;

                        // Check if fragment has an id attribute (for OOB swap pattern)
                        var tempDiv = document.createElement('div');
                        tempDiv.innerHTML = fragment;
                        var firstChild = tempDiv.firstElementChild;

                        if (firstChild && firstChild.id) {
                            // Replace existing element with same id
                            var existingElement = document.getElementById(firstChild.id);
                            if (existingElement) {
                                existingElement.outerHTML = fragment;
                                return;
                            }

                            // Special handling for sidebar and header updates
                            if (firstChild.id === 'session-sidebar') {
                                sidebar.innerHTML = firstChild.innerHTML;
                                return;
                            }
                            if (firstChild.id === 'session-header') {
                                header.innerHTML = firstChild.innerHTML;
                                return;
                            }
                        }

                        // Default: append to timeline
                        timeline.insertAdjacentHTML('beforeend', fragment);

                        // Auto-scroll if enabled
                        if (autoScroll.checked) {
                            window.scrollTo(0, document.body.scrollHeight);
                        }
                    };

                    ws.onclose = function() {
                        status.textContent = 'completed';
                        status.className = 'text-xs text-blue';
                    };

                    ws.onerror = function() {
                        status.textContent = 'error';
                        status.className = 'text-xs text-red';
                    };
                })();
                </script>"#))
            }
        }
    }
}
