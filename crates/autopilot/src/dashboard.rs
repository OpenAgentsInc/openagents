//! Web dashboard for autopilot metrics visualization and real-time monitoring.
//!
//! This module provides an Actix-web server with Maud/HTMX UI for visualizing
//! autopilot performance metrics, trajectory analysis, and detecting anomalies.
//! Launched via `cargo autopilot dashboard`.
//!
//! # Features
//!
//! - **Session List**: All trajectory sessions with key statistics
//! - **Trend Charts**: Metrics visualization over time
//! - **Anomaly Detection**: Highlight sessions with unusual patterns
//! - **Drill-Down**: Detailed view of individual sessions
//! - **Real-Time Updates**: WebSocket streaming of new metrics
//! - **Export**: JSON/CSV export for external analysis
//!
//! # Usage
//!
//! ```no_run
//! use autopilot::dashboard::start_dashboard;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let port = start_dashboard().await?;
//! println!("Dashboard running at http://localhost:{}", port);
//! # Ok(())
//! # }
//! ```
//!
//! # Related Modules
//!
//! - [`crate::metrics`]: Metrics database and storage
//! - [`crate::analyze`]: Trajectory analysis engine

use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use actix_ws::Message;
use chrono::{DateTime, Utc};
use maud::{html, Markup, DOCTYPE};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus};

/// Format a Unix timestamp as a human-readable relative time
fn format_relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if duration.num_days() < 7 {
        let days = duration.num_days();
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        // For older dates, show the actual date
        dt.format("%b %d, %Y").to_string()
    }
}

/// Global broadcast sender for metrics updates
/// This allows the metrics collection system to push updates without direct coupling
static METRICS_BROADCAST: once_cell::sync::OnceCell<broadcast::Sender<MetricsUpdate>> = once_cell::sync::OnceCell::new();

/// Set the global metrics broadcast sender (called when dashboard starts)
pub fn set_metrics_broadcast(sender: broadcast::Sender<MetricsUpdate>) {
    let _ = METRICS_BROADCAST.set(sender);
}

/// Broadcast a metrics update globally (can be called from anywhere)
pub fn broadcast_metrics_update(update_type: &str, session_id: Option<String>) {
    if let Some(tx) = METRICS_BROADCAST.get() {
        let update = MetricsUpdate {
            update_type: update_type.to_string(),
            session_id,
            timestamp: Utc::now().to_rfc3339(),
        };
        let _ = tx.send(update);
    }
}

/// Metrics update event for WebSocket broadcasting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsUpdate {
    pub update_type: String,
    pub session_id: Option<String>,
    pub timestamp: String,
}

/// Dashboard application state
pub struct DashboardState {
    db_path: String,
    /// Broadcast channel for real-time metrics updates
    pub metrics_tx: broadcast::Sender<MetricsUpdate>,
}

impl DashboardState {
    /// Broadcast a metrics update to all connected WebSocket clients
    pub fn broadcast_update(&self, update_type: &str, session_id: Option<String>) {
        let update = MetricsUpdate {
            update_type: update_type.to_string(),
            session_id,
            timestamp: Utc::now().to_rfc3339(),
        };
        // Ignore send errors (no receivers is fine)
        let _ = self.metrics_tx.send(update);
    }
}

/// Start the metrics dashboard server
pub async fn start_dashboard(db_path: &str, port: u16) -> anyhow::Result<()> {
    // Test that we can open the database
    MetricsDb::open(db_path)?;

    // Create broadcast channel for real-time metrics updates
    // Buffer of 100 messages should be sufficient for live updates
    let (metrics_tx, _) = broadcast::channel::<MetricsUpdate>(100);

    // Set the global broadcast sender so metrics collection can use it
    set_metrics_broadcast(metrics_tx.clone());

    let state = web::Data::new(DashboardState {
        db_path: db_path.to_string(),
        metrics_tx,
    });

    println!("Starting autopilot metrics dashboard on http://127.0.0.1:{}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/sessions", web::get().to(sessions_list))
            .route("/session/{id}", web::get().to(session_detail))
            .route("/export/sessions.json", web::get().to(export_json))
            .route("/export/sessions.csv", web::get().to(export_csv))
            .route("/ws", web::get().to(websocket))
            // API endpoints
            .route("/api/sessions", web::get().to(api_sessions))
            .route("/api/sessions/{id}", web::get().to(api_session_detail))
            .route("/api/metrics", web::get().to(api_metrics))
            .route("/api/anomalies", web::get().to(api_anomalies))
            .route("/api/trends", web::get().to(api_trends))
            .route("/ws/metrics", web::get().to(websocket_metrics))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await?;

    Ok(())
}

/// Home page with session list
async fn index(state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let sessions = store.get_recent_sessions(50).unwrap_or_default();
    let stats = store.get_summary_stats().unwrap_or_default();

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(dashboard_page(&sessions, &stats)))
}

/// Sessions list page
async fn sessions_list(state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let sessions = store.get_recent_sessions(100).unwrap_or_default();

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(sessions_list_page(&sessions)))
}

/// Session detail page
async fn session_detail(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let session_id = path.into_inner();
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    match store.get_session(&session_id) {
        Ok(Some(session)) => {
            let tool_calls = store.get_tool_calls(&session_id).unwrap_or_default();

            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(session_detail_page(&session, &tool_calls)))
        }
        _ => Ok(HttpResponse::NotFound().body("Session not found")),
    }
}

/// Export sessions as JSON
async fn export_json(state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let sessions = store.get_recent_sessions(1000).unwrap_or_default();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string_pretty(&sessions).unwrap()))
}

/// Export sessions as CSV
async fn export_csv(state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let sessions = store.get_recent_sessions(1000).unwrap_or_default();

    let mut csv = String::from("session_id,timestamp,model,duration_seconds,tokens_in,tokens_out,tokens_cached,cost_usd,issues_claimed,issues_completed,tool_calls,tool_errors,final_status\n");

    for session in sessions {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            session.id,
            session.timestamp,
            session.model,
            session.duration_seconds,
            session.tokens_in,
            session.tokens_out,
            session.tokens_cached,
            session.cost_usd,
            session.issues_claimed,
            session.issues_completed,
            session.tool_calls,
            session.tool_errors,
            format!("{:?}", session.final_status).to_lowercase()
        ));
    }

    Ok(HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header(("Content-Disposition", "attachment; filename=sessions.csv"))
        .body(csv))
}

/// WebSocket endpoint for real-time updates
async fn websocket(
    req: actix_web::HttpRequest,
    stream: web::Payload,
) -> ActixResult<HttpResponse> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.recv().await {
            match msg {
                Message::Ping(bytes) => {
                    let _ = session.pong(&bytes).await;
                }
                Message::Text(text) => {
                    // Echo back for now - in production would push metrics updates
                    let _ = session.text(text).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}

/// Summary statistics for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryStats {
    pub total_sessions: i64,
    pub total_issues_completed: i64,
    pub total_cost_usd: f64,
    pub avg_duration_seconds: f64,
    pub avg_tokens_per_session: f64,
    pub completion_rate: f64,
}

impl Default for SummaryStats {
    fn default() -> Self {
        Self {
            total_sessions: 0,
            total_issues_completed: 0,
            total_cost_usd: 0.0,
            avg_duration_seconds: 0.0,
            avg_tokens_per_session: 0.0,
            completion_rate: 0.0,
        }
    }
}

/// Render dashboard home page
pub(crate) fn dashboard_page(sessions: &[SessionMetrics], stats: &SummaryStats) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Autopilot Metrics Dashboard" }
                style { (dashboard_styles()) }
                script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js" {}
                script { (raw_html(dashboard_script())) }
            }
            body {
                header {
                    h1 { "⚡ Autopilot Metrics Dashboard" }
                    p.subtitle { "Continual Constant Improvement (d-004)" }
                    nav.header-nav {
                        a href="/" class="active" { "Dashboard" }
                        a href="/sessions" { "All Sessions" }
                        a href="/export/sessions.json" { "Export JSON" }
                        a href="/export/sessions.csv" { "Export CSV" }
                    }
                }

                main {
                    (summary_card(stats))
                    (charts_section())
                    (sessions_table(sessions))
                }

                footer {
                    p { "OpenAgents Autopilot • Real-time metrics powered by WebSocket" }
                }
            }
        }
    };

    markup.into_string()
}

/// Helper to insert raw HTML (for script content)
fn raw_html(s: &str) -> maud::PreEscaped<String> {
    maud::PreEscaped(s.to_string())
}

/// Render charts section with real-time visualizations
fn charts_section() -> Markup {
    html! {
        div.charts-section {
            h2 { "Metrics Trends" }
            div.charts-grid {
                div.chart-container {
                    h3 { "Tool Error Rate (Last 7 Days)" }
                    canvas id="errorRateChart" {}
                }
                div.chart-container {
                    h3 { "Completion Rate (Last 7 Days)" }
                    canvas id="completionRateChart" {}
                }
                div.chart-container {
                    h3 { "Average Cost per Session (Last 7 Days)" }
                    canvas id="costChart" {}
                }
                div.chart-container {
                    h3 { "Token Usage Trend (Last 7 Days)" }
                    canvas id="tokensChart" {}
                }
            }
        }
    }
}

/// Render summary statistics card
pub(crate) fn summary_card(stats: &SummaryStats) -> Markup {
    html! {
        div.summary-card {
            h2 { "Summary Statistics" }
            div.stats-grid {
                div.stat {
                    span.stat-label { "Total Sessions" }
                    span.stat-value { (stats.total_sessions) }
                }
                div.stat {
                    span.stat-label { "Issues Completed" }
                    span.stat-value { (stats.total_issues_completed) }
                }
                div.stat {
                    span.stat-label { "Total Cost" }
                    span.stat-value { "$" (format!("{:.2}", stats.total_cost_usd)) }
                }
                div.stat {
                    span.stat-label { "Avg Duration" }
                    span.stat-value { (format!("{:.0}s", stats.avg_duration_seconds)) }
                }
                div.stat {
                    span.stat-label { "Avg Tokens" }
                    span.stat-value { (format!("{:.0}", stats.avg_tokens_per_session)) }
                }
                div.stat {
                    span.stat-label { "Completion Rate" }
                    span.stat-value { (format!("{:.1}%", stats.completion_rate * 100.0)) }
                }
            }
        }
    }
}

/// Render sessions table
pub(crate) fn sessions_table(sessions: &[SessionMetrics]) -> Markup {
    html! {
        div.sessions-section {
            h2 { "Recent Sessions (" (sessions.len()) ")" }
            @if sessions.is_empty() {
                p.placeholder { "No sessions recorded yet" }
            } @else {
                table.sessions-table {
                    thead {
                        tr {
                            th { "Session ID" }
                            th { "Timestamp" }
                            th { "Model" }
                            th { "Duration" }
                            th { "Tokens" }
                            th { "Cost" }
                            th { "Issues" }
                            th { "Status" }
                        }
                    }
                    tbody {
                        @for session in sessions {
                            (session_row(session))
                        }
                    }
                }
            }
        }
    }
}

/// Render single session row
fn session_row(session: &SessionMetrics) -> Markup {
    let status_class = match session.final_status {
        SessionStatus::Completed => "status-completed",
        SessionStatus::Crashed => "status-crashed",
        SessionStatus::BudgetExhausted => "status-budget",
        SessionStatus::MaxTurns => "status-maxturns",
        SessionStatus::Running => "status-running",
    };

    let total_tokens = session.tokens_in + session.tokens_out;

    html! {
        tr class=(status_class) {
            td {
                a href={"/session/" (session.id)} {
                    (format!("{}...", &session.id[..8]))
                }
            }
            td title=(session.timestamp.to_rfc3339()) { (format_relative_time(&session.timestamp)) }
            td { (session.model) }
            td { (format!("{:.0}s", session.duration_seconds)) }
            td { (format!("{}", total_tokens)) }
            td { "$" (format!("{:.3}", session.cost_usd)) }
            td { (session.issues_completed) "/" (session.issues_claimed) }
            td { (format!("{:?}", session.final_status)) }
        }
    }
}

/// Render sessions list page
fn sessions_list_page(sessions: &[SessionMetrics]) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "All Sessions" }
                style { (dashboard_styles()) }
            }
            body {
                header {
                    h1 { "All Sessions" }
                    p { a href="/" { "← Back to Dashboard" } }
                }
                main {
                    (sessions_table(sessions))
                }
            }
        }
    };

    markup.into_string()
}

/// Render session detail page
fn session_detail_page(session: &SessionMetrics, _tool_calls: &[crate::metrics::ToolCallMetrics]) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "Session " (session.id) }
                style { (dashboard_styles()) }
            }
            body {
                header {
                    h1 { "Session " (session.id) }
                    p { a href="/" { "← Back to Dashboard" } }
                }
                main {
                    div.session-details {
                        h2 { "Session Details" }
                        dl {
                            dt { "Timestamp" }
                            dd title=(session.timestamp.to_rfc3339()) { (format_relative_time(&session.timestamp)) " (" (session.timestamp.format("%Y-%m-%d %H:%M:%S UTC")) ")" }
                            dt { "Model" }
                            dd { (session.model) }
                            dt { "Duration" }
                            dd { (format!("{:.2}s", session.duration_seconds)) }
                            dt { "Tokens In" }
                            dd { (session.tokens_in) }
                            dt { "Tokens Out" }
                            dd { (session.tokens_out) }
                            dt { "Tokens Cached" }
                            dd { (session.tokens_cached) }
                            dt { "Cost" }
                            dd { "$" (format!("{:.4}", session.cost_usd)) }
                            dt { "Issues Claimed" }
                            dd { (session.issues_claimed) }
                            dt { "Issues Completed" }
                            dd { (session.issues_completed) }
                            dt { "Tool Calls" }
                            dd { (session.tool_calls) }
                            dt { "Tool Errors" }
                            dd { (session.tool_errors) }
                            dt { "Status" }
                            dd { (format!("{:?}", session.final_status)) }
                        }

                        h3 { "Prompt" }
                        pre { (session.prompt) }
                    }
                }
            }
        }
    };

    markup.into_string()
}

/// CSS styles for dashboard
fn dashboard_styles() -> &'static str {
    r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

:root {
    --bg-primary: #1a1b26;
    --bg-secondary: #24283b;
    --bg-tertiary: #414868;
    --text-primary: #c0caf5;
    --text-secondary: #a9b1d6;
    --accent: #7aa2f7;
    --green: #9ece6a;
    --red: #f7768e;
    --orange: #ff9e64;
    --yellow: #e0af68;
    --border: #414868;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
    line-height: 1.6;
}

header {
    background: var(--bg-secondary);
    padding: 2rem;
    border-bottom: 2px solid var(--accent);
}

h1 {
    font-size: 2rem;
    margin-bottom: 0.5rem;
}

.subtitle {
    color: var(--text-secondary);
    font-size: 0.9rem;
}

.header-nav {
    margin-top: 1rem;
    display: flex;
    gap: 1rem;
}

.header-nav a {
    color: var(--text-secondary);
    text-decoration: none;
    padding: 0.5rem 1rem;
    transition: all 0.2s;
}

.header-nav a:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
}

.header-nav a.active {
    background: var(--accent);
    color: var(--bg-primary);
    font-weight: 600;
}

main {
    max-width: 1400px;
    margin: 0 auto;
    padding: 2rem;
}

.summary-card {
    background: var(--bg-secondary);
    padding: 2rem;
    margin-bottom: 2rem;
}

.stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1.5rem;
    margin-top: 1rem;
}

.stat {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}

.stat-label {
    color: var(--text-secondary);
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.stat-value {
    font-size: 1.75rem;
    font-weight: 600;
    color: var(--accent);
}

.charts-section {
    margin-bottom: 2rem;
}

.charts-section h2 {
    margin-bottom: 1.5rem;
}

.charts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(500px, 1fr));
    gap: 2rem;
}

.chart-container {
    background: var(--bg-secondary);
    padding: 1.5rem;
}

.chart-container h3 {
    color: var(--text-secondary);
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 1rem;
}

.chart-container canvas {
    max-height: 300px;
}

.sessions-section {
    margin-top: 2rem;
}

.sessions-table {
    width: 100%;
    border-collapse: collapse;
    background: var(--bg-secondary);
    margin-top: 1rem;
}

.sessions-table th {
    background: var(--bg-tertiary);
    padding: 1rem;
    text-align: left;
    font-weight: 600;
    text-transform: uppercase;
    font-size: 0.875rem;
    letter-spacing: 0.05em;
}

.sessions-table td {
    padding: 1rem;
    border-top: 1px solid var(--border);
}

.sessions-table tr:hover {
    background: var(--bg-tertiary);
}

.sessions-table a {
    color: var(--accent);
    text-decoration: none;
}

.sessions-table a:hover {
    text-decoration: underline;
}

.status-completed {
    border-left: 3px solid var(--green);
}

.status-crashed {
    border-left: 3px solid var(--red);
}

.status-budget {
    border-left: 3px solid var(--orange);
}

.status-maxturns {
    border-left: 3px solid var(--yellow);
}

.status-running {
    border-left: 3px solid var(--accent);
}

.placeholder {
    color: var(--text-secondary);
    font-style: italic;
    padding: 2rem;
    text-align: center;
}

.session-details {
    background: var(--bg-secondary);
    padding: 2rem;
}

.session-details dl {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 1rem;
    margin: 1rem 0;
}

.session-details dt {
    font-weight: 600;
    color: var(--text-secondary);
}

.session-details dd {
    color: var(--text-primary);
}

.session-details pre {
    background: var(--bg-primary);
    padding: 1rem;
    overflow-x: auto;
    margin-top: 0.5rem;
}

footer {
    text-align: center;
    padding: 2rem;
    color: var(--text-secondary);
    font-size: 0.875rem;
}

footer a {
    color: var(--accent);
    text-decoration: none;
}

footer a:hover {
    text-decoration: underline;
}
"#
}

/// JavaScript for dashboard interactivity and real-time charts
fn dashboard_script() -> &'static str {
    r#"
// Chart.js configuration with dark theme
const chartColors = {
    primary: '#7aa2f7',
    green: '#9ece6a',
    red: '#f7768e',
    orange: '#ff9e64',
    yellow: '#e0af68',
    bg: '#1a1b26',
    gridColor: '#414868',
    textColor: '#c0caf5',
};

const chartOptions = {
    responsive: true,
    maintainAspectRatio: true,
    plugins: {
        legend: {
            display: false,
        },
    },
    scales: {
        x: {
            grid: {
                color: chartColors.gridColor,
            },
            ticks: {
                color: chartColors.textColor,
            },
        },
        y: {
            grid: {
                color: chartColors.gridColor,
            },
            ticks: {
                color: chartColors.textColor,
            },
        },
    },
};

// Load charts when page loads
document.addEventListener('DOMContentLoaded', async () => {
    await loadAllCharts();
    setupWebSocket();

    // Refresh charts every 60 seconds
    setInterval(loadAllCharts, 60000);
});

async function loadAllCharts() {
    await Promise.all([
        loadErrorRateChart(),
        loadCompletionRateChart(),
        loadCostChart(),
        loadTokensChart(),
    ]);
}

async function loadErrorRateChart() {
    try {
        const response = await fetch('/api/trends?dimension=tool_error_rate&hours=168&granularity=day');
        const data = await response.json();

        const ctx = document.getElementById('errorRateChart').getContext('2d');

        // Destroy existing chart if it exists
        if (window.errorRateChart) {
            window.errorRateChart.destroy();
        }

        window.errorRateChart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.data.map(d => d.date),
                datasets: [{
                    label: 'Error Rate (%)',
                    data: data.data.map(d => d.value),
                    borderColor: chartColors.red,
                    backgroundColor: chartColors.red + '20',
                    fill: true,
                    tension: 0.3,
                }],
            },
            options: {
                ...chartOptions,
                scales: {
                    ...chartOptions.scales,
                    y: {
                        ...chartOptions.scales.y,
                        min: 0,
                        max: 100,
                    },
                },
            },
        });
    } catch (error) {
        console.error('Failed to load error rate chart:', error);
    }
}

async function loadCompletionRateChart() {
    try {
        const response = await fetch('/api/trends?dimension=completion_rate&hours=168&granularity=day');
        const data = await response.json();

        const ctx = document.getElementById('completionRateChart').getContext('2d');

        if (window.completionRateChart) {
            window.completionRateChart.destroy();
        }

        window.completionRateChart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.data.map(d => d.date),
                datasets: [{
                    label: 'Completion Rate (%)',
                    data: data.data.map(d => d.value),
                    borderColor: chartColors.green,
                    backgroundColor: chartColors.green + '20',
                    fill: true,
                    tension: 0.3,
                }],
            },
            options: {
                ...chartOptions,
                scales: {
                    ...chartOptions.scales,
                    y: {
                        ...chartOptions.scales.y,
                        min: 0,
                        max: 100,
                    },
                },
            },
        });
    } catch (error) {
        console.error('Failed to load completion rate chart:', error);
    }
}

async function loadCostChart() {
    try {
        const response = await fetch('/api/trends?dimension=avg_cost&hours=168&granularity=day');
        const data = await response.json();

        const ctx = document.getElementById('costChart').getContext('2d');

        if (window.costChart) {
            window.costChart.destroy();
        }

        window.costChart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.data.map(d => d.date),
                datasets: [{
                    label: 'Avg Cost ($)',
                    data: data.data.map(d => d.value),
                    backgroundColor: chartColors.orange,
                    borderColor: chartColors.orange,
                    borderWidth: 1,
                }],
            },
            options: chartOptions,
        });
    } catch (error) {
        console.error('Failed to load cost chart:', error);
    }
}

async function loadTokensChart() {
    try {
        const response = await fetch('/api/trends?dimension=avg_tokens&hours=168&granularity=day');
        const data = await response.json();

        const ctx = document.getElementById('tokensChart').getContext('2d');

        if (window.tokensChart) {
            window.tokensChart.destroy();
        }

        window.tokensChart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.data.map(d => d.date),
                datasets: [{
                    label: 'Avg Tokens',
                    data: data.data.map(d => d.value),
                    borderColor: chartColors.primary,
                    backgroundColor: chartColors.primary + '20',
                    fill: true,
                    tension: 0.3,
                }],
            },
            options: chartOptions,
        });
    } catch (error) {
        console.error('Failed to load tokens chart:', error);
    }
}

function setupWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/metrics`);

    ws.onopen = () => {
        console.log('✅ WebSocket connected for real-time metrics');
        // Show connection indicator
        showConnectionStatus('connected');
    };

    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        console.log('📊 Received metrics update:', data);

        if (data.update_type === 'connected') {
            console.log('🔌 ' + data.message);
            return;
        }

        // Reload charts and session list on any metrics update
        if (data.update_type === 'session_updated' ||
            data.update_type === 'session_created' ||
            data.update_type === 'metrics_update') {
            loadAllCharts();

            // If we're on the main page, reload the session list too
            if (window.location.pathname === '/') {
                setTimeout(() => {
                    window.location.reload();
                }, 500);
            }
        }
    };

    ws.onerror = (error) => {
        console.error('❌ WebSocket error:', error);
        showConnectionStatus('error');
    };

    ws.onclose = () => {
        console.log('🔌 WebSocket disconnected, reconnecting in 5s...');
        showConnectionStatus('disconnected');
        setTimeout(setupWebSocket, 5000);
    };
}

function showConnectionStatus(status) {
    // You could add a UI indicator here
    // For now just log to console
    const statusEmoji = {
        'connected': '🟢',
        'disconnected': '🔴',
        'error': '🟠'
    };
    console.log(`${statusEmoji[status] || '⚪'} Connection status: ${status}`);
}
"#
}

// ============================================================================
// API Endpoints (JSON)
// ============================================================================

/// Query parameters for sessions list API
#[derive(Debug, Deserialize)]
struct SessionsQuery {
    /// Number of sessions to return (default: 50, max: 1000)
    #[serde(default = "default_limit")]
    limit: usize,
    /// Offset for pagination (default: 0)
    #[serde(default)]
    offset: usize,
    /// Filter by status (optional)
    status: Option<String>,
    /// Sort by field (default: timestamp)
    #[serde(default = "default_sort")]
    sort: String,
    /// Sort direction: asc or desc (default: desc)
    #[serde(default = "default_order")]
    order: String,
}

fn default_limit() -> usize { 50 }
fn default_sort() -> String { "timestamp".to_string() }
fn default_order() -> String { "desc".to_string() }

/// API endpoint: GET /api/sessions
/// Returns paginated list of sessions with filtering and sorting
async fn api_sessions(
    state: web::Data<DashboardState>,
    query: web::Query<SessionsQuery>,
) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Cap limit at 1000
    let limit = query.limit.min(1000);

    // Get all sessions (we'll filter and sort in memory for simplicity)
    let mut sessions = store.get_all_sessions()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Filter by status if provided
    if let Some(ref status) = query.status {
        sessions.retain(|s| {
            format!("{:?}", s.final_status).to_lowercase() == status.to_lowercase()
        });
    }

    // Sort
    match query.sort.as_str() {
        "duration" => sessions.sort_by(|a, b| a.duration_seconds.partial_cmp(&b.duration_seconds).unwrap()),
        "cost" => sessions.sort_by(|a, b| a.cost_usd.partial_cmp(&b.cost_usd).unwrap()),
        "tokens" => sessions.sort_by(|a, b| {
            let a_total = a.tokens_in + a.tokens_out;
            let b_total = b.tokens_in + b.tokens_out;
            a_total.cmp(&b_total)
        }),
        _ => sessions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
    }

    // Reverse if descending
    if query.order == "desc" {
        sessions.reverse();
    }

    // Apply pagination
    let total = sessions.len();
    let sessions: Vec<_> = sessions.into_iter()
        .skip(query.offset)
        .take(limit)
        .collect();

    let response = serde_json::json!({
        "sessions": sessions,
        "total": total,
        "limit": limit,
        "offset": query.offset,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// API endpoint: GET /api/sessions/{id}
/// Returns detailed metrics for a specific session
async fn api_session_detail(
    state: web::Data<DashboardState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let session_id = path.into_inner();
    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let session = store.get_session(&session_id)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    match session {
        Some(session) => {
            let tool_calls = store.get_tool_calls(&session_id).unwrap_or_default();
            let anomalies = store.get_anomalies(&session_id).unwrap_or_default();

            let response = serde_json::json!({
                "session": session,
                "tool_calls": tool_calls,
                "anomalies": anomalies,
            });

            Ok(HttpResponse::Ok().json(response))
        }
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Session not found",
            "session_id": session_id,
        }))),
    }
}

/// API endpoint: GET /api/metrics
/// Returns aggregate metrics summary
async fn api_metrics(
    state: web::Data<DashboardState>,
) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let stats = store.get_summary_stats()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok().json(stats))
}

/// Query parameters for anomalies API
#[derive(Debug, Deserialize)]
struct AnomaliesQuery {
    /// Number of anomalies to return (default: 50, max: 500)
    #[serde(default = "default_anomalies_limit")]
    limit: usize,
    /// Filter by severity (optional): warning, error, critical
    severity: Option<String>,
    /// Show only uninvestigated anomalies (default: false)
    #[serde(default)]
    uninvestigated_only: bool,
}

fn default_anomalies_limit() -> usize { 50 }

/// API endpoint: GET /api/anomalies
/// Returns recent anomalies with optional filtering
async fn api_anomalies(
    state: web::Data<DashboardState>,
    query: web::Query<AnomaliesQuery>,
) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Get recent sessions to find their anomalies
    let sessions = store.get_recent_sessions(200)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let mut all_anomalies = Vec::new();
    for session in sessions {
        if let Ok(anomalies) = store.get_anomalies(&session.id) {
            all_anomalies.extend(anomalies);
        }
    }

    // Filter by severity if provided
    if let Some(ref severity) = query.severity {
        all_anomalies.retain(|a| {
            format!("{:?}", a.severity).to_lowercase() == severity.to_lowercase()
        });
    }

    // Filter by investigation status if requested
    if query.uninvestigated_only {
        all_anomalies.retain(|a| !a.investigated);
    }

    // Sort by most recent first (would need timestamps on anomalies for this)
    // For now, just take the limit
    let total = all_anomalies.len();
    let anomalies: Vec<_> = all_anomalies.into_iter()
        .take(query.limit.min(500))
        .collect();

    let response = serde_json::json!({
        "anomalies": anomalies,
        "total": total,
        "limit": query.limit.min(500),
    });

    Ok(HttpResponse::Ok().json(response))
}

/// Query parameters for trends API
#[derive(Debug, Deserialize)]
struct TrendsQuery {
    /// Metric dimension to trend (e.g., "tool_error_rate", "completion_rate")
    dimension: String,
    /// Time range in hours (default: 168 = 1 week, max: 720 = 30 days)
    #[serde(default = "default_hours")]
    hours: i64,
    /// Granularity: hour, day, week (default: day)
    #[serde(default = "default_granularity")]
    granularity: String,
}

fn default_hours() -> i64 { 168 } // 1 week
fn default_granularity() -> String { "day".to_string() }

/// API endpoint: GET /api/trends
/// Returns metric trends over time
async fn api_trends(
    state: web::Data<DashboardState>,
    query: web::Query<TrendsQuery>,
) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Cap at 30 days
    let hours = query.hours.min(720);

    // Get all sessions within the time range
    let all_sessions = store.get_all_sessions()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let cutoff = Utc::now() - chrono::Duration::hours(hours);
    let sessions: Vec<_> = all_sessions.into_iter()
        .filter(|s| s.timestamp >= cutoff)
        .collect();

    // Calculate the requested metric trend
    let data_points = match query.dimension.as_str() {
        "tool_error_rate" => calculate_error_rate_trend(&sessions, &query.granularity),
        "completion_rate" => calculate_completion_rate_trend(&sessions, &query.granularity),
        "avg_duration" => calculate_duration_trend(&sessions, &query.granularity),
        "avg_cost" => calculate_cost_trend(&sessions, &query.granularity),
        "avg_tokens" => calculate_tokens_trend(&sessions, &query.granularity),
        _ => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Unknown dimension",
                "dimension": query.dimension,
                "supported": ["tool_error_rate", "completion_rate", "avg_duration", "avg_cost", "avg_tokens"],
            })));
        }
    };

    let response = serde_json::json!({
        "dimension": query.dimension,
        "hours": hours,
        "granularity": query.granularity,
        "data": data_points,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// Calculate tool error rate trend
fn calculate_error_rate_trend(sessions: &[SessionMetrics], _granularity: &str) -> Vec<serde_json::Value> {
    // Simple implementation: daily buckets
    use std::collections::HashMap;

    let mut buckets: HashMap<String, (i32, i32)> = HashMap::new();

    for session in sessions {
        let day = session.timestamp.format("%Y-%m-%d").to_string();
        let entry = buckets.entry(day.clone()).or_insert((0, 0));
        entry.0 += session.tool_calls;
        entry.1 += session.tool_errors;
    }

    let mut result: Vec<_> = buckets.into_iter()
        .map(|(day, (calls, errors))| {
            let rate = if calls > 0 {
                (errors as f64 / calls as f64) * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "date": day,
                "value": rate,
                "tool_calls": calls,
                "tool_errors": errors,
            })
        })
        .collect();

    result.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}

/// Calculate completion rate trend
fn calculate_completion_rate_trend(sessions: &[SessionMetrics], _granularity: &str) -> Vec<serde_json::Value> {
    use std::collections::HashMap;

    let mut buckets: HashMap<String, (i32, i32)> = HashMap::new();

    for session in sessions {
        let day = session.timestamp.format("%Y-%m-%d").to_string();
        let entry = buckets.entry(day.clone()).or_insert((0, 0));
        entry.0 += session.issues_claimed;
        entry.1 += session.issues_completed;
    }

    let mut result: Vec<_> = buckets.into_iter()
        .map(|(day, (claimed, completed))| {
            let rate = if claimed > 0 {
                (completed as f64 / claimed as f64) * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "date": day,
                "value": rate,
                "issues_claimed": claimed,
                "issues_completed": completed,
            })
        })
        .collect();

    result.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}

/// Calculate average duration trend
fn calculate_duration_trend(sessions: &[SessionMetrics], _granularity: &str) -> Vec<serde_json::Value> {
    use std::collections::HashMap;

    let mut buckets: HashMap<String, Vec<f64>> = HashMap::new();

    for session in sessions {
        let day = session.timestamp.format("%Y-%m-%d").to_string();
        buckets.entry(day).or_default().push(session.duration_seconds);
    }

    let mut result: Vec<_> = buckets.into_iter()
        .map(|(day, durations)| {
            let avg = durations.iter().sum::<f64>() / durations.len() as f64;
            serde_json::json!({
                "date": day,
                "value": avg,
                "count": durations.len(),
            })
        })
        .collect();

    result.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}

/// Calculate average cost trend
fn calculate_cost_trend(sessions: &[SessionMetrics], _granularity: &str) -> Vec<serde_json::Value> {
    use std::collections::HashMap;

    let mut buckets: HashMap<String, Vec<f64>> = HashMap::new();

    for session in sessions {
        let day = session.timestamp.format("%Y-%m-%d").to_string();
        buckets.entry(day).or_default().push(session.cost_usd);
    }

    let mut result: Vec<_> = buckets.into_iter()
        .map(|(day, costs)| {
            let avg = costs.iter().sum::<f64>() / costs.len() as f64;
            serde_json::json!({
                "date": day,
                "value": avg,
                "count": costs.len(),
            })
        })
        .collect();

    result.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}

/// Calculate average tokens trend
fn calculate_tokens_trend(sessions: &[SessionMetrics], _granularity: &str) -> Vec<serde_json::Value> {
    use std::collections::HashMap;

    let mut buckets: HashMap<String, Vec<i64>> = HashMap::new();

    for session in sessions {
        let day = session.timestamp.format("%Y-%m-%d").to_string();
        let total_tokens = session.tokens_in + session.tokens_out;
        buckets.entry(day).or_default().push(total_tokens);
    }

    let mut result: Vec<_> = buckets.into_iter()
        .map(|(day, tokens)| {
            let avg = tokens.iter().sum::<i64>() as f64 / tokens.len() as f64;
            serde_json::json!({
                "date": day,
                "value": avg,
                "count": tokens.len(),
            })
        })
        .collect();

    result.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}

/// WebSocket endpoint for real-time metrics updates
async fn websocket_metrics(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    state: web::Data<DashboardState>,
) -> ActixResult<HttpResponse> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    // Subscribe to metrics updates
    let mut metrics_rx = state.metrics_tx.subscribe();

    actix_web::rt::spawn(async move {
        // Send initial connection message
        let _ = session.text(serde_json::json!({
            "update_type": "connected",
            "message": "Real-time metrics stream connected",
            "timestamp": Utc::now().to_rfc3339(),
        }).to_string()).await;

        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            let _ = session.pong(&bytes).await;
                        }
                        Message::Text(_) => {
                            // Client sent text, send back acknowledgment
                            let _ = session.text(serde_json::json!({
                                "type": "ack",
                                "message": "Message received",
                            }).to_string()).await;
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                // Handle broadcast metrics updates
                Ok(update) = metrics_rx.recv() => {
                    // Send the update to the WebSocket client
                    if let Ok(json) = serde_json::to_string(&update) {
                        if session.text(json).await.is_err() {
                            // Connection closed
                            break;
                        }
                    }
                }
                else => {
                    // Both channels closed
                    break;
                }
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}
