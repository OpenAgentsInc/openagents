//! HTML layout templates using Maud

use maud::{html, Markup, PreEscaped, DOCTYPE};

/// Base page layout with navigation
pub fn page(title: &str, content: Markup) -> String {
    page_with_current(title, content, None)
}

/// Base page layout with current page highlighting
pub fn page_with_current(title: &str, content: Markup, current_page: Option<&str>) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                style {
                    r#"
                    * {
                        margin: 0;
                        padding: 0;
                        box-sizing: border-box;
                    }
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                        background: #1a1a1a;
                        color: #e0e0e0;
                        line-height: 1.6;
                    }
                    .container {
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 2rem;
                    }
                    nav {
                        background: #2a2a2a;
                        padding: 1rem 2rem;
                        border-bottom: 1px solid #3a3a3a;
                    }
                    nav h1 {
                        color: #4a9eff;
                        font-size: 1.5rem;
                    }
                    .card {
                        background: #2a2a2a;
                        border: 1px solid #3a3a3a;
                        padding: 1.5rem;
                        margin-bottom: 1rem;
                    }
                    .card h2 {
                        color: #4a9eff;
                        margin-bottom: 1rem;
                    }
                    .status {
                        display: inline-block;
                        padding: 0.25rem 0.75rem;
                        background: #2d5016;
                        color: #7dff7d;
                        font-size: 0.875rem;
                    }
                    .nav-links {
                        display: flex;
                        gap: 1.5rem;
                        align-items: center;
                    }
                    .nav-links a {
                        color: #a0a0a0;
                        text-decoration: none;
                        padding: 0.5rem 1rem;
                        transition: color 0.2s;
                        font-size: 0.95rem;
                    }
                    .nav-links a:hover {
                        color: #4a9eff;
                    }
                    .nav-links a.active {
                        color: #4a9eff;
                        border-bottom: 2px solid #4a9eff;
                    }
                    @media (max-width: 768px) {
                        .nav-links {
                            flex-direction: column;
                            gap: 0.5rem;
                            align-items: flex-start;
                        }
                        nav > div {
                            flex-direction: column !important;
                            gap: 1rem;
                        }
                    }
                    .live-indicator {
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.25rem 0.75rem;
                        background: #2d5016;
                        color: #7dff7d;
                        font-size: 0.75rem;
                        font-weight: 600;
                    }
                    .live-dot {
                        width: 8px;
                        height: 8px;
                        background: #7dff7d;
                        animation: pulse 2s infinite;
                    }
                    @keyframes pulse {
                        0%, 100% { opacity: 1; }
                        50% { opacity: 0.3; }
                    }
                    .apm-indicator {
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.25rem 0.75rem;
                        background: #1a3a5a;
                        color: #4a9eff;
                        font-size: 0.75rem;
                        font-weight: 600;
                        font-family: monospace;
                    }
                    .apm-value {
                        color: #7dff7d;
                        font-weight: 700;
                    }
                    "#
                }
                script {
                    (PreEscaped(r#"
                    // WebSocket connection for live updates
                    let ws = null;
                    let reconnectTimer = null;

                    function connectWebSocket() {
                        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                        const wsUrl = `${protocol}//${window.location.host}/ws`;

                        ws = new WebSocket(wsUrl);

                        ws.onopen = () => {
                            console.log('WebSocket connected');
                            updateConnectionStatus(true);
                            if (reconnectTimer) {
                                clearTimeout(reconnectTimer);
                                reconnectTimer = null;
                            }
                        };

                        ws.onmessage = (event) => {
                            try {
                                const msg = JSON.parse(event.data);
                                handleWebSocketMessage(msg);
                            } catch (e) {
                                console.error('Failed to parse WebSocket message:', e);
                            }
                        };

                        ws.onerror = (error) => {
                            console.error('WebSocket error:', error);
                        };

                        ws.onclose = () => {
                            console.log('WebSocket disconnected');
                            updateConnectionStatus(false);
                            // Reconnect after 3 seconds
                            reconnectTimer = setTimeout(connectWebSocket, 3000);
                        };
                    }

                    function updateConnectionStatus(connected) {
                        const indicator = document.getElementById('live-indicator');
                        if (indicator) {
                            if (connected) {
                                indicator.innerHTML = '<span class="live-dot"></span>LIVE';
                                indicator.style.background = '#2d5016';
                                indicator.style.color = '#7dff7d';
                            } else {
                                indicator.innerHTML = 'DISCONNECTED';
                                indicator.style.background = '#5d1616';
                                indicator.style.color = '#ff7d7d';
                            }
                        }
                    }

                    function updateAPM(avgApm) {
                        const apmValue = document.querySelector('#apm-indicator .apm-value');
                        if (apmValue) {
                            apmValue.textContent = avgApm.toFixed(1);
                        }
                    }

                    function handleWebSocketMessage(msg) {
                        console.log('WebSocket message:', msg);

                        switch (msg.type) {
                            case 'session_started':
                                console.log('Session started:', msg.session_id);
                                // Reload page to show new session
                                setTimeout(() => window.location.reload(), 1000);
                                break;

                            case 'session_completed':
                                console.log('Session completed:', msg.session_id);
                                // Reload page to update stats
                                setTimeout(() => window.location.reload(), 1000);
                                break;

                            case 'stats_updated':
                                console.log('Stats updated');
                                // Could update stats in-place without reload
                                break;

                            case 'apm_updated':
                                console.log('APM updated:', msg.avg_apm);
                                updateAPM(msg.avg_apm);
                                break;
                        }
                    }

                    // Connect on page load
                    if (window.location.pathname === '/') {
                        connectWebSocket();
                    }
                    "#))
                }
            }
            body {
                nav {
                    div style="display: flex; justify-content: space-between; align-items: center;" {
                        h1 { "🤖 Autopilot GUI" }
                        div class="nav-links" {
                            a href="/" class={ @if current_page == Some("dashboard") { "active" } } { "Dashboard" }
                            a href="/chat" class={ @if current_page == Some("chat") { "active" } } { "Chat" }
                            a href="/context" class={ @if current_page == Some("context") { "active" } } { "Context" }
                            a href="/parallel" class={ @if current_page == Some("parallel") { "active" } } { "Parallel Agents" }
                            a href="/permissions" class={ @if current_page == Some("permissions") { "active" } } { "Permissions" }
                        }
                    }
                }
                (content)
            }
        }
    };

    markup.into_string()
}

/// Dashboard view with session data
pub fn dashboard_with_data(
    recent_sessions: Vec<crate::sessions::SessionInfo>,
    stats: crate::sessions::DashboardStats,
) -> Markup {
    html! {
        div class="container" {
            // Quick Stats
            div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem;" {
                div class="card" style="text-align: center;" {
                    h3 style="color: #4a9eff; font-size: 2rem; margin-bottom: 0.5rem;" {
                        (stats.sessions_today)
                    }
                    p style="color: #a0a0a0; font-size: 0.9rem;" { "Sessions Today" }
                }
                div class="card" style="text-align: center;" {
                    h3 style="color: #4a9eff; font-size: 2rem; margin-bottom: 0.5rem;" {
                        (format!("{:.1}%", stats.success_rate))
                    }
                    p style="color: #a0a0a0; font-size: 0.9rem;" { "Success Rate (30d)" }
                }
                div class="card" style="text-align: center;" {
                    h3 style="color: #4a9eff; font-size: 2rem; margin-bottom: 0.5rem;" {
                        (format!("{:.0}k", stats.total_tokens as f64 / 1000.0))
                    }
                    p style="color: #a0a0a0; font-size: 0.9rem;" { "Total Tokens (30d)" }
                }
                div class="card" style="text-align: center;" {
                    h3 style="color: #4a9eff; font-size: 2rem; margin-bottom: 0.5rem;" {
                        (format!("${:.2}", stats.total_cost))
                    }
                    p style="color: #a0a0a0; font-size: 0.9rem;" { "Total Cost (30d)" }
                }
                div class="card" style="text-align: center;" {
                    h3 style="color: #4a9eff; font-size: 2rem; margin-bottom: 0.5rem;" {
                        (format!("{:.0}m", stats.avg_duration / 60.0))
                    }
                    p style="color: #a0a0a0; font-size: 0.9rem;" { "Avg Duration (30d)" }
                }
            }

            // Recent Sessions
            div class="card" {
                div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" {
                    h2 style="margin: 0;" { "Recent Sessions" }
                    div style="display: flex; gap: 0.5rem; align-items: center;" {
                        span id="apm-indicator" class="apm-indicator" {
                            "APM: "
                            span class="apm-value" { (format!("{:.1}", stats.avg_apm)) }
                        }
                        span id="live-indicator" class="live-indicator" {
                            "CONNECTING..."
                        }
                    }
                }

                @if recent_sessions.is_empty() {
                    p style="color: #a0a0a0; margin-top: 1rem;" { "No sessions found. Run autopilot to see sessions here." }
                } @else {
                    table style="width: 100%; margin-top: 1rem; border-collapse: collapse;" {
                        thead {
                            tr style="border-bottom: 1px solid #3a3a3a;" {
                                th style="text-align: left; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Time" }
                                th style="text-align: left; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Model" }
                                th style="text-align: left; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Duration" }
                                th style="text-align: right; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Tokens" }
                                th style="text-align: right; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Issues" }
                                th style="text-align: center; padding: 0.75rem; color: #a0a0a0; font-weight: 600;" { "Status" }
                            }
                        }
                        tbody {
                            @for session in recent_sessions {
                                tr style="border-bottom: 1px solid #2a2a2a;" {
                                    td style="padding: 0.75rem; color: #e0e0e0; font-family: monospace; font-size: 0.85rem;" {
                                        (format_timestamp(&session.timestamp))
                                    }
                                    td style="padding: 0.75rem; color: #e0e0e0; font-size: 0.85rem;" {
                                        (format_model(&session.model))
                                    }
                                    td style="padding: 0.75rem; color: #e0e0e0; font-size: 0.85rem;" {
                                        (format_duration(session.duration_seconds))
                                    }
                                    td style="padding: 0.75rem; color: #e0e0e0; text-align: right; font-size: 0.85rem;" {
                                        (format_tokens(session.tokens_in + session.tokens_out))
                                    }
                                    td style="padding: 0.75rem; color: #e0e0e0; text-align: right; font-size: 0.85rem;" {
                                        (session.issues_completed)
                                    }
                                    td style="padding: 0.75rem; text-align: center;" {
                                        (format_status(&session.final_status))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // System Status
            div class="card" {
                h2 { "System Status" }
                div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-top: 1rem;" {
                    div {
                        p style="color: #a0a0a0; margin-bottom: 0.25rem;" { "Server" }
                        p style="color: #7dff7d; font-weight: 600;" { "Running" }
                    }
                    div {
                        p style="color: #a0a0a0; margin-bottom: 0.25rem;" { "Port" }
                        p style="color: #e0e0e0; font-weight: 600;" { "3847" }
                    }
                    div {
                        p style="color: #a0a0a0; margin-bottom: 0.25rem;" { "Version" }
                        p style="color: #e0e0e0; font-weight: 600;" { "0.1.0" }
                    }
                }
            }
        }
    }
}

/// Legacy dashboard view (no data)
pub fn dashboard() -> Markup {
    html! {
        div class="container" {
            div class="card" {
                h2 { "Dashboard" }
                p { "Loading session data..." }
                p style="margin-top: 1rem; color: #a0a0a0;" {
                    "Unable to load metrics database. Make sure autopilot-metrics.db exists."
                }
            }
        }
    }
}

fn format_timestamp(ts: &str) -> String {
    // Parse ISO8601 timestamp and format as relative time
    // For now, just show the date part
    ts.split('T').next().unwrap_or(ts).to_string()
}

fn format_model(model: &str) -> String {
    // Shorten model name for display
    if model.contains("claude-sonnet") {
        "Sonnet 4.5".to_string()
    } else if model.contains("claude-opus") {
        "Opus 4.5".to_string()
    } else if model.contains("claude-haiku") {
        "Haiku 4".to_string()
    } else {
        model.to_string()
    }
}

fn format_duration(seconds: f64) -> String {
    let minutes = (seconds / 60.0) as i64;
    if minutes < 60 {
        format!("{}m", minutes)
    } else {
        let hours = minutes / 60;
        let remaining_minutes = minutes % 60;
        format!("{}h {}m", hours, remaining_minutes)
    }
}

fn format_tokens(tokens: i64) -> String {
    if tokens < 1000 {
        tokens.to_string()
    } else if tokens < 1_000_000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

fn format_status(status: &str) -> Markup {
    let (color, text) = match status {
        "completed" => ("#7dff7d", "✓ Complete"),
        "failed" => ("#ff7d7d", "✗ Failed"),
        "crashed" => ("#ff9d7d", "⚠ Crashed"),
        "budget_exhausted" => ("#ffd97d", "$ Budget"),
        _ => ("#a0a0a0", status),
    };

    html! {
        span style=(format!("color: {}; font-weight: 600; font-size: 0.85rem;", color)) {
            (text)
        }
    }
}
