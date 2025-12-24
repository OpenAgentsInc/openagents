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
            .route("/dashboard/apm-compare", web::get().to(apm_compare))
            .route("/export/sessions.json", web::get().to(export_json))
            .route("/export/sessions.csv", web::get().to(export_csv))
            .route("/ws", web::get().to(websocket))
            // API endpoints
            .route("/api/sessions", web::get().to(api_sessions))
            .route("/api/sessions/{id}", web::get().to(api_session_detail))
            .route("/api/metrics", web::get().to(api_metrics))
            .route("/api/anomalies", web::get().to(api_anomalies))
            .route("/api/trends", web::get().to(api_trends))
            .route("/api/velocity", web::get().to(api_velocity))
            .route("/api/apm-timeline", web::get().to(api_apm_timeline))
            .route("/api/action-breakdown", web::get().to(api_action_breakdown))
            .route("/api/apm-compare-data", web::get().to(api_apm_compare_data))
            .route("/ws/metrics", web::get().to(websocket_metrics))
            .route("/ws/apm", web::get().to(websocket_apm))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await?;

    Ok(())
}

/// Home page with session list
async fn index(state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    let store = MetricsDb::open(&state.db_path).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let sessions = store.get_recent_sessions(50).map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    let stats = store.get_summary_stats().map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

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

/// APM comparison page
async fn apm_compare(_state: web::Data<DashboardState>) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(apm_compare_page()))
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
                    div style="display: flex; justify-content: space-between; align-items: center;" {
                        div {
                            h1 { "⚡ Autopilot Metrics Dashboard" }
                            p.subtitle { "Continual Constant Improvement (d-004)" }
                        }
                        div style="text-align: right;" {
                            span #ws-status style="font-size: 1.5em;" title="WebSocket status" { "⚪" }
                            p style="font-size: 0.8em; margin: 0; color: #888;" { "Live Updates" }
                        }
                    }
                    nav.header-nav {
                        a href="/" class="active" { "Dashboard" }
                        a href="/sessions" { "All Sessions" }
                        a href="/dashboard/apm-compare" { "APM Compare" }
                        a href="/export/sessions.json" { "Export JSON" }
                        a href="/export/sessions.csv" { "Export CSV" }
                    }
                }

                main {
                    (summary_card(stats))
                    (apm_card(sessions))
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
                    h3 { "APM Timeline (Last 7 Days)" }
                    canvas id="apmTimelineChart" {}
                }
                div.chart-container {
                    h3 { "Action Breakdown (Last 7 Days)" }
                    canvas id="actionBreakdownChart" {}
                }
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

/// Render APM (Actions Per Minute) card
pub(crate) fn apm_card(sessions: &[SessionMetrics]) -> Markup {
    use crate::apm::APMTier;

    // Calculate APM statistics from recent sessions
    let autopilot_sessions: Vec<_> = sessions.iter()
        .filter(|s| s.source == "autopilot" && s.apm.is_some())
        .collect();

    let claude_code_sessions: Vec<_> = sessions.iter()
        .filter(|s| s.source == "claude_code" && s.apm.is_some())
        .collect();

    let autopilot_avg = if !autopilot_sessions.is_empty() {
        autopilot_sessions.iter()
            .filter_map(|s| s.apm)
            .sum::<f64>() / autopilot_sessions.len() as f64
    } else {
        0.0
    };

    let claude_code_avg = if !claude_code_sessions.is_empty() {
        claude_code_sessions.iter()
            .filter_map(|s| s.apm)
            .sum::<f64>() / claude_code_sessions.len() as f64
    } else {
        0.0
    };

    let efficiency_ratio = if claude_code_avg > 0.0 {
        autopilot_avg / claude_code_avg
    } else {
        0.0
    };

    let current_session_apm = sessions.first().and_then(|s| s.apm);
    let current_tier = current_session_apm.map(APMTier::from_apm);

    let total_actions: i64 = sessions.iter()
        .map(|s| s.messages as i64 + s.tool_calls as i64)
        .sum();

    html! {
        div.summary-card {
            h2 { "APM (Actions Per Minute)" }
            p.subtitle style="margin-bottom: 1rem; color: #666;" {
                "Agent velocity metrics • Autopilot vs Claude Code"
            }
            div.stats-grid {
                @if autopilot_avg > 0.0 {
                    div.stat {
                        span.stat-label { "Autopilot (avg)" }
                        span.stat-value style="color: #10b981;" { (format!("{:.1}", autopilot_avg)) }
                        span.stat-unit { "APM" }
                    }
                } @else {
                    div.stat {
                        span.stat-label { "Autopilot (avg)" }
                        span.stat-value style="color: #666;" { "N/A" }
                        span.stat-unit { "" }
                    }
                }
                @if claude_code_avg > 0.0 {
                    div.stat {
                        span.stat-label { "Claude Code (avg)" }
                        span.stat-value style="color: #3b82f6;" { (format!("{:.1}", claude_code_avg)) }
                        span.stat-unit { "APM" }
                    }
                } @else {
                    div.stat {
                        span.stat-label { "Claude Code (avg)" }
                        span.stat-value style="color: #666;" { "N/A" }
                        span.stat-unit { "" }
                    }
                }
                @if efficiency_ratio > 0.0 {
                    div.stat {
                        span.stat-label { "Efficiency Ratio" }
                        span.stat-value style="color: #f59e0b;" { (format!("{:.1}x", efficiency_ratio)) }
                        span.stat-unit { "faster" }
                    }
                } @else {
                    div.stat {
                        span.stat-label { "Efficiency Ratio" }
                        span.stat-value style="color: #666;" { "N/A" }
                        span.stat-unit { "" }
                    }
                }
                @if let Some(apm) = current_session_apm {
                    div.stat {
                        span.stat-label { "Latest Session" }
                        span.stat-value { (format!("{:.1}", apm)) }
                        span.stat-unit { "APM" }
                    }
                } @else {
                    div.stat {
                        span.stat-label { "Latest Session" }
                        span.stat-value style="color: #666;" { "N/A" }
                        span.stat-unit { "" }
                    }
                }
                @if let Some(tier) = current_tier {
                    div.stat {
                        span.stat-label { "Performance Tier" }
                        @match tier {
                            APMTier::Elite => {
                                span.stat-value style="color: #fbbf24;" { (tier.name()) }
                            }
                            APMTier::HighPerformance => {
                                span.stat-value style="color: #10b981;" { (tier.name()) }
                            }
                            APMTier::Productive => {
                                span.stat-value style="color: #10b981;" { (tier.name()) }
                            }
                            APMTier::Active => {
                                span.stat-value style="color: #3b82f6;" { (tier.name()) }
                            }
                            APMTier::Baseline => {
                                span.stat-value style="color: #6b7280;" { (tier.name()) }
                            }
                        }
                        span.stat-unit { (tier.color()) }
                    }
                } @else {
                    div.stat {
                        span.stat-label { "Performance Tier" }
                        span.stat-value style="color: #666;" { "N/A" }
                        span.stat-unit { "" }
                    }
                }
                div.stat {
                    span.stat-label { "Total Actions" }
                    span.stat-value { (format!("{}", total_actions)) }
                    span.stat-unit { "tracked" }
                }
            }
            @if sessions.is_empty() {
                p.note style="margin-top: 1rem; font-size: 0.85rem; color: #666; font-style: italic;" {
                    "Note: No session data available yet. Run autopilot to collect APM metrics."
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
                            th { "Velocity" }
                            th { "APM" }
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
    use crate::apm::APMTier;

    let status_class = match session.final_status {
        SessionStatus::Completed => "status-completed",
        SessionStatus::Crashed => "status-crashed",
        SessionStatus::BudgetExhausted => "status-budget",
        SessionStatus::MaxTurns => "status-maxturns",
        SessionStatus::Running => "status-running",
    };

    let total_tokens = session.tokens_in + session.tokens_out;

    // Calculate velocity: issues completed per hour
    let velocity_score = if session.duration_seconds > 0.0 {
        let hours = session.duration_seconds / 3600.0;
        session.issues_completed as f64 / hours
    } else {
        0.0
    };

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
            td title="Issues completed per hour" {
                @if velocity_score > 0.0 {
                    (format!("{:.2}", velocity_score))
                } @else {
                    span style="color: #666;" { "—" }
                }
            }
            td {
                @if let Some(apm) = session.apm {
                    @let tier = APMTier::from_apm(apm);
                    @let tier_color = match tier {
                        APMTier::Elite => "#fbbf24",
                        APMTier::HighPerformance => "#f59e0b",
                        APMTier::Productive => "#10b981",
                        APMTier::Active => "#3b82f6",
                        APMTier::Baseline => "#6b7280",
                    };
                    span.apm-badge style=(format!("background-color: {}; color: white; padding: 2px 6px; font-size: 0.85rem; font-weight: 500;", tier_color))
                         title=(tier.name()) {
                        (format!("{:.1}", apm))
                    }
                } @else {
                    span style="color: #666;" { "—" }
                }
            }
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

/// Render APM comparison page
fn apm_compare_page() -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "APM Comparison - Autopilot vs Claude Code" }
                style { (dashboard_styles()) }
                script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns@3.0.0/dist/chartjs-adapter-date-fns.bundle.min.js" {}
                script { (raw_html(apm_compare_script())) }
            }
            body {
                header {
                    h1 { "APM Comparison" }
                    p.subtitle { "Claude Code vs Autopilot Velocity Analysis (d-016)" }
                    nav.header-nav {
                        a href="/" { "Dashboard" }
                        a href="/sessions" { "All Sessions" }
                        a href="/dashboard/apm-compare" class="active" { "APM Compare" }
                    }
                }

                main {
                    // Time window selector
                    div.summary-card {
                        h2 { "Time Window" }
                        div style="display: flex; gap: 1rem; margin-top: 1rem;" {
                            button.time-filter.active data-window="1h" { "1 Hour" }
                            button.time-filter data-window="6h" { "6 Hours" }
                            button.time-filter data-window="1d" { "1 Day" }
                            button.time-filter data-window="1w" { "1 Week" }
                            button.time-filter data-window="1m" { "1 Month" }
                        }
                    }

                    // Summary comparison stats
                    div.summary-card {
                        h2 { "APM Summary" }
                        div #apm-summary .stats-grid {
                            div.stat {
                                span.stat-label { "Autopilot (avg)" }
                                span #autopilot-avg .stat-value style="color: #10b981;" { "—" }
                                span.stat-unit { "APM" }
                            }
                            div.stat {
                                span.stat-label { "Claude Code (avg)" }
                                span #claude-code-avg .stat-value style="color: #3b82f6;" { "—" }
                                span.stat-unit { "APM" }
                            }
                            div.stat {
                                span.stat-label { "Efficiency Ratio" }
                                span #efficiency-ratio .stat-value style="color: #f59e0b;" { "—" }
                                span.stat-unit { "" }
                            }
                            div.stat {
                                span.stat-label { "Autopilot Sessions" }
                                span #autopilot-count .stat-value { "—" }
                                span.stat-unit { "sessions" }
                            }
                            div.stat {
                                span.stat-label { "Claude Code Sessions" }
                                span #claude-code-count .stat-value { "—" }
                                span.stat-unit { "sessions" }
                            }
                            div.stat {
                                span.stat-label { "Total Actions" }
                                span #total-actions .stat-value { "—" }
                                span.stat-unit { "tracked" }
                            }
                        }
                    }

                    // Charts
                    div.charts-section {
                        h2 { "Comparison Charts" }
                        div.charts-grid {
                            div.chart-container {
                                h3 { "APM Over Time" }
                                canvas #apmCompareChart {}
                            }
                            div.chart-container {
                                h3 { "Average APM by Source" }
                                canvas #apmBarChart {}
                            }
                            div.chart-container {
                                h3 { "APM Distribution" }
                                canvas #apmHistogramChart {}
                            }
                            div.chart-container {
                                h3 { "Tool Category Breakdown" }
                                canvas #toolCategoryChart {}
                            }
                        }
                    }
                }

                footer {
                    p { "OpenAgents Autopilot • APM Comparison powered by d-016" }
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

.stat-unit {
    color: var(--text-secondary);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-top: 0.25rem;
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

.time-filter {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 2px solid var(--border);
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 600;
    transition: all 0.2s;
}

.time-filter:hover {
    background: var(--accent);
    color: var(--bg-primary);
    border-color: var(--accent);
}

.time-filter.active {
    background: var(--accent);
    color: var(--bg-primary);
    border-color: var(--accent);
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
        loadAPMTimelineChart(),
        loadActionBreakdownChart(),
        loadErrorRateChart(),
        loadCompletionRateChart(),
        loadCostChart(),
        loadTokensChart(),
    ]);
}

async function loadAPMTimelineChart() {
    try {
        const response = await fetch('/api/apm-timeline?hours=168');
        const data = await response.json();

        const ctx = document.getElementById('apmTimelineChart').getContext('2d');

        if (window.apmTimelineChart) {
            window.apmTimelineChart.destroy();
        }

        // Separate data by source
        const autopilotData = data.sessions
            .filter(s => s.source === 'autopilot')
            .map(s => ({ x: s.timestamp, y: s.apm }));
        const claudeCodeData = data.sessions
            .filter(s => s.source === 'claude_code')
            .map(s => ({ x: s.timestamp, y: s.apm }));

        window.apmTimelineChart = new Chart(ctx, {
            type: 'scatter',
            data: {
                datasets: [
                    {
                        label: 'Autopilot',
                        data: autopilotData,
                        borderColor: chartColors.green,
                        backgroundColor: chartColors.green,
                        pointRadius: 4,
                        showLine: true,
                        tension: 0.3,
                    },
                    {
                        label: 'Claude Code',
                        data: claudeCodeData,
                        borderColor: chartColors.primary,
                        backgroundColor: chartColors.primary,
                        pointRadius: 4,
                        showLine: true,
                        tension: 0.3,
                    },
                ],
            },
            options: {
                ...chartOptions,
                plugins: {
                    legend: {
                        display: true,
                        labels: {
                            color: chartColors.textColor,
                        },
                    },
                },
                scales: {
                    x: {
                        type: 'time',
                        time: {
                            unit: 'day',
                        },
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
                        title: {
                            display: true,
                            text: 'APM',
                            color: chartColors.textColor,
                        },
                    },
                },
            },
        });
    } catch (error) {
        console.error('Failed to load APM timeline chart:', error);
    }
}

async function loadActionBreakdownChart() {
    try {
        const response = await fetch('/api/action-breakdown?hours=168');
        const data = await response.json();

        const ctx = document.getElementById('actionBreakdownChart').getContext('2d');

        if (window.actionBreakdownChart) {
            window.actionBreakdownChart.destroy();
        }

        window.actionBreakdownChart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['Messages', 'Tool Calls'],
                datasets: [{
                    data: [data.total_messages, data.total_tool_calls],
                    backgroundColor: [chartColors.primary, chartColors.green],
                    borderColor: chartColors.bg,
                    borderWidth: 2,
                }],
            },
            options: {
                responsive: true,
                maintainAspectRatio: true,
                plugins: {
                    legend: {
                        display: true,
                        position: 'bottom',
                        labels: {
                            color: chartColors.textColor,
                            padding: 15,
                        },
                    },
                },
            },
        });
    } catch (error) {
        console.error('Failed to load action breakdown chart:', error);
    }
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

        // Handle different update types
        switch (data.update_type) {
            case 'session_updated':
            case 'session_created':
            case 'metrics_update':
                // Reload charts for session-level updates
                loadAllCharts();

                // If we're on the main page, reload the session list
                if (window.location.pathname === '/') {
                    setTimeout(() => {
                        window.location.reload();
                    }, 500);
                }
                break;

            case 'tool_call':
                // Show real-time tool call notification
                showToast(`Tool call executed in session ${data.session_id?.substring(0, 8)}...`, 'info');
                // Update live counters if on session detail page
                updateLiveCounters();
                break;

            case 'anomaly_detected':
                // Show alert for anomaly detection
                showToast(`⚠️ Anomaly detected in session ${data.session_id?.substring(0, 8)}...`, 'warning');
                // Reload to show new anomaly
                setTimeout(() => window.location.reload(), 1000);
                break;

            default:
                console.log('Unknown update type:', data.update_type);
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
    const statusEmoji = {
        'connected': '🟢',
        'disconnected': '🔴',
        'error': '🟠'
    };
    console.log(`${statusEmoji[status] || '⚪'} Connection status: ${status}`);

    // Update visual indicator if it exists
    const indicator = document.getElementById('ws-status');
    if (indicator) {
        indicator.textContent = statusEmoji[status] || '⚪';
        indicator.title = `WebSocket: ${status}`;
    }
}

function showToast(message, type = 'info') {
    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    toast.textContent = message;
    toast.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 1rem 1.5rem;
        background: var(--bg-tertiary, #1a1a1a);
        border: 2px solid ${type === 'warning' ? '#fbbf24' : type === 'info' ? '#3b82f6' : '#10b981'};
        color: white;
        z-index: 10000;
        font-weight: 600;
        box-shadow: 0 4px 12px rgba(0,0,0,0.5);
        animation: slideIn 0.3s ease-out;
    `;

    document.body.appendChild(toast);

    setTimeout(() => {
        toast.style.animation = 'slideOut 0.3s ease-out';
        setTimeout(() => toast.remove(), 300);
    }, 3000);
}

function updateLiveCounters() {
    // If we're on a session detail page, refresh tool call counts
    const sessionPath = window.location.pathname.match(/\/session\/([^/]+)/);
    if (sessionPath) {
        // Fetch updated session data and update counters
        fetch('/api/sessions/' + sessionPath[1])
            .then(r => r.json())
            .then(data => {
                const toolCallsElem = document.getElementById('live-tool-calls');
                const errorsElem = document.getElementById('live-errors');
                if (toolCallsElem) toolCallsElem.textContent = data.tool_calls || 0;
                if (errorsElem) toolCallsElem.textContent = data.tool_errors || 0;
            })
            .catch(err => console.error('Failed to update live counters:', err));
    }
}
"#
}

/// JavaScript for APM comparison page
fn apm_compare_script() -> &'static str {
    r#"
const chartColors = {
    autopilot: '#10b981',
    claudeCode: '#3b82f6',
    bg: '#1a1b26',
    gridColor: '#414868',
    textColor: '#c0caf5',
};

let currentWindow = '1h';
let compareChart, barChart, histogramChart, categoryChart;

document.addEventListener('DOMContentLoaded', () => {
    setupTimeFilters();
    loadComparisonData(currentWindow);
});

function setupTimeFilters() {
    document.querySelectorAll('.time-filter').forEach(btn => {
        btn.addEventListener('click', (e) => {
            // Update active state
            document.querySelectorAll('.time-filter').forEach(b => b.classList.remove('active'));
            e.target.classList.add('active');

            // Load new data
            currentWindow = e.target.getAttribute('data-window');
            loadComparisonData(currentWindow);
        });
    });
}

async function loadComparisonData(window) {
    try {
        const response = await fetch(`/api/apm-compare-data?window=${window}`);
        const data = await response.json();

        // Update summary stats
        updateSummaryStats(data);

        // Update charts
        updateComparisonChart(data);
        updateBarChart(data);
        updateHistogramChart(data);
        updateCategoryChart(data);
    } catch (error) {
        console.error('Failed to load comparison data:', error);
    }
}

function updateSummaryStats(data) {
    document.getElementById('autopilot-avg').textContent = data.autopilot.avg_apm.toFixed(1);
    document.getElementById('claude-code-avg').textContent = data.claude_code.avg_apm.toFixed(1);

    const ratio = data.summary.efficiency_ratio;
    const ratioElem = document.getElementById('efficiency-ratio');
    ratioElem.textContent = ratio > 0 ? `${ratio.toFixed(1)}x` : 'N/A';
    if (ratio > 0) {
        ratioElem.nextElementSibling.textContent = 'faster';
    } else {
        ratioElem.nextElementSibling.textContent = '';
    }

    document.getElementById('autopilot-count').textContent = data.autopilot.count;
    document.getElementById('claude-code-count').textContent = data.claude_code.count;
    document.getElementById('total-actions').textContent = data.summary.total_actions;
}

function updateComparisonChart(data) {
    const ctx = document.getElementById('apmCompareChart').getContext('2d');

    if (compareChart) {
        compareChart.destroy();
    }

    // Prepare data for line chart
    const autopilotData = data.autopilot.sessions.map(s => ({
        x: new Date(s.timestamp),
        y: s.apm
    })).reverse();

    const claudeCodeData = data.claude_code.sessions.map(s => ({
        x: new Date(s.timestamp),
        y: s.apm
    })).reverse();

    compareChart = new Chart(ctx, {
        type: 'scatter',
        data: {
            datasets: [
                {
                    label: 'Autopilot',
                    data: autopilotData,
                    borderColor: chartColors.autopilot,
                    backgroundColor: chartColors.autopilot,
                    showLine: true,
                    tension: 0.3,
                    pointRadius: 4,
                },
                {
                    label: 'Claude Code',
                    data: claudeCodeData,
                    borderColor: chartColors.claudeCode,
                    backgroundColor: chartColors.claudeCode,
                    showLine: true,
                    tension: 0.3,
                    pointRadius: 4,
                },
            ],
        },
        options: {
            responsive: true,
            maintainAspectRatio: true,
            plugins: {
                legend: {
                    display: true,
                    labels: {
                        color: chartColors.textColor,
                    },
                },
            },
            scales: {
                x: {
                    type: 'time',
                    time: {
                        unit: 'hour',
                    },
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
                    title: {
                        display: true,
                        text: 'APM',
                        color: chartColors.textColor,
                    },
                },
            },
        },
    });
}

function updateBarChart(data) {
    const ctx = document.getElementById('apmBarChart').getContext('2d');

    if (barChart) {
        barChart.destroy();
    }

    barChart = new Chart(ctx, {
        type: 'bar',
        data: {
            labels: ['Autopilot', 'Claude Code'],
            datasets: [{
                label: 'Average APM',
                data: [data.autopilot.avg_apm, data.claude_code.avg_apm],
                backgroundColor: [chartColors.autopilot, chartColors.claudeCode],
                borderColor: [chartColors.autopilot, chartColors.claudeCode],
                borderWidth: 2,
            }],
        },
        options: {
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
                    title: {
                        display: true,
                        text: 'APM',
                        color: chartColors.textColor,
                    },
                },
            },
        },
    });
}

function updateHistogramChart(data) {
    const ctx = document.getElementById('apmHistogramChart').getContext('2d');

    if (histogramChart) {
        histogramChart.destroy();
    }

    if (!data.histogram || data.histogram.length === 0) {
        // No data - show placeholder
        return;
    }

    histogramChart = new Chart(ctx, {
        type: 'bar',
        data: {
            labels: data.histogram.map(bin => bin.range),
            datasets: [{
                label: 'Sessions',
                data: data.histogram.map(bin => bin.count),
                backgroundColor: chartColors.autopilot,
                borderColor: chartColors.autopilot,
                borderWidth: 1,
            }],
        },
        options: {
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
                        maxRotation: 45,
                        minRotation: 45,
                    },
                    title: {
                        display: true,
                        text: 'APM Range',
                        color: chartColors.textColor,
                    },
                },
                y: {
                    grid: {
                        color: chartColors.gridColor,
                    },
                    ticks: {
                        color: chartColors.textColor,
                        precision: 0,
                    },
                    title: {
                        display: true,
                        text: 'Session Count',
                        color: chartColors.textColor,
                    },
                },
            },
        },
    });
}

function updateCategoryChart(data) {
    const ctx = document.getElementById('toolCategoryChart').getContext('2d');

    if (categoryChart) {
        categoryChart.destroy();
    }

    // Calculate breakdown from all sessions
    let totalMessages = 0;
    let totalToolCalls = 0;

    [...data.autopilot.sessions, ...data.claude_code.sessions].forEach(s => {
        totalMessages += s.messages || 0;
        totalToolCalls += s.tool_calls || 0;
    });

    categoryChart = new Chart(ctx, {
        type: 'doughnut',
        data: {
            labels: ['Messages', 'Tool Calls'],
            datasets: [{
                data: [totalMessages, totalToolCalls],
                backgroundColor: [chartColors.claudeCode, chartColors.autopilot],
                borderColor: chartColors.bg,
                borderWidth: 2,
            }],
        },
        options: {
            responsive: true,
            maintainAspectRatio: true,
            plugins: {
                legend: {
                    display: true,
                    position: 'bottom',
                    labels: {
                        color: chartColors.textColor,
                        padding: 15,
                    },
                },
            },
        },
    });
}
"#
}

// ============================================================================
// API Endpoints (JSON)
// ============================================================================

/// Query parameters for sessions list API
#[derive(Debug, Clone, Deserialize)]
struct SessionsQuery {
    /// Number of sessions to return (default: 50, max: 1000)
    #[serde(default = "default_limit")]
    limit: usize,
    /// Offset for pagination (default: 0)
    #[serde(default)]
    offset: usize,
    /// Filter by status (optional)
    status: Option<String>,
    /// Filter by issue number (optional)
    issue: Option<i32>,
    /// Filter by directive ID (optional)
    directive: Option<String>,
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
    let db_path = state.db_path.clone();
    let query_clone = query.clone();
    let query_params = query.into_inner();

    // Run blocking database operations in a separate thread pool
    let (sessions, total) = web::block(move || -> Result<(Vec<crate::metrics::SessionMetrics>, usize), anyhow::Error> {
        let store = MetricsDb::open(&db_path)?;

        // Cap limit at 1000
        let limit = query_params.limit.min(1000);

        // Get sessions based on filters
        let mut sessions = if let Some(issue_num) = query_params.issue {
            store.get_sessions_for_issue(issue_num)?
        } else if let Some(ref dir_id) = query_params.directive {
            store.get_sessions_for_directive(dir_id)?
        } else {
            store.get_all_sessions()?
        };

        // Filter by status if provided
        if let Some(ref status) = query_params.status {
            sessions.retain(|s| {
                format!("{:?}", s.final_status).to_lowercase() == status.to_lowercase()
            });
        }

        // Sort (handle NaN values by treating them as less than all other values)
        match query_params.sort.as_str() {
            "duration" => sessions.sort_by(|a, b| {
                match (a.duration_seconds.is_nan(), b.duration_seconds.is_nan()) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => a.duration_seconds.partial_cmp(&b.duration_seconds).unwrap(),
                }
            }),
            "cost" => sessions.sort_by(|a, b| {
                match (a.cost_usd.is_nan(), b.cost_usd.is_nan()) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => a.cost_usd.partial_cmp(&b.cost_usd).unwrap(),
                }
            }),
            "tokens" => sessions.sort_by(|a, b| {
                let a_total = a.tokens_in + a.tokens_out;
                let b_total = b.tokens_in + b.tokens_out;
                a_total.cmp(&b_total)
            }),
            _ => sessions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
        }

        // Reverse if descending
        if query_params.order == "desc" {
            sessions.reverse();
        }

        // Apply pagination
        let total = sessions.len();
        let sessions: Vec<_> = sessions.into_iter()
            .skip(query_params.offset)
            .take(limit)
            .collect();

        Ok((sessions, total))
    })
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = serde_json::json!({
        "sessions": sessions,
        "total": total,
        "limit": query_clone.limit.min(1000),
        "offset": query_clone.offset,
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

/// API endpoint: GET /api/velocity
/// Returns current velocity snapshot and recent historical snapshots
async fn api_velocity(
    state: web::Data<DashboardState>,
) -> ActixResult<HttpResponse> {
    use crate::analyze::{calculate_velocity, TimePeriod};

    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Calculate current velocity for this week
    let current_velocity = calculate_velocity(&store, TimePeriod::ThisWeek)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Get recent snapshots (last 10)
    let snapshots = store.get_velocity_snapshots(10)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = serde_json::json!({
        "current": current_velocity,
        "history": snapshots,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// API endpoint: GET /api/apm-timeline
/// Returns APM data for timeline chart with session timestamps and APM values
async fn api_apm_timeline(
    state: web::Data<DashboardState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> ActixResult<HttpResponse> {
    let hours = query.get("hours")
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(168); // Default to 7 days

    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
    let all_sessions = store.get_all_sessions()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Filter sessions within time range and with APM data
    let sessions: Vec<_> = all_sessions.into_iter()
        .filter(|s| s.timestamp >= cutoff && s.apm.is_some())
        .map(|s| serde_json::json!({
            "timestamp": s.timestamp.to_rfc3339(),
            "apm": s.apm.unwrap_or(0.0),
            "source": s.source,
        }))
        .collect();

    let response = serde_json::json!({
        "sessions": sessions,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// API endpoint: GET /api/action-breakdown
/// Returns total messages and tool calls for action breakdown pie chart
async fn api_action_breakdown(
    state: web::Data<DashboardState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> ActixResult<HttpResponse> {
    let hours = query.get("hours")
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(168); // Default to 7 days

    let store = MetricsDb::open(&state.db_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
    let all_sessions = store.get_all_sessions()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Calculate totals
    let mut total_messages: i64 = 0;
    let mut total_tool_calls: i64 = 0;

    for session in all_sessions.iter() {
        if session.timestamp >= cutoff {
            total_messages += session.messages as i64;
            total_tool_calls += session.tool_calls as i64;
        }
    }

    let response = serde_json::json!({
        "total_messages": total_messages,
        "total_tool_calls": total_tool_calls,
        "total_actions": total_messages + total_tool_calls,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// API endpoint: GET /api/apm-compare-data
/// Returns APM comparison data for Claude Code vs Autopilot sessions
async fn api_apm_compare_data(
    state: web::Data<DashboardState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> ActixResult<HttpResponse> {
    let window = query.get("window")
        .map(|s| s.as_str())
        .unwrap_or("1d");

    let db_path = state.db_path.clone();
    let window_str = window.to_string();

    // Run blocking database operations
    let result = web::block(move || -> Result<serde_json::Value, anyhow::Error> {
        let conn = rusqlite::Connection::open(&db_path)?;

        // Query apm_snapshots for the requested window
        let mut stmt = conn.prepare(
            "SELECT source, apm, actions, duration_minutes, messages, tool_calls, timestamp
             FROM apm_snapshots
             WHERE window = ?1
             ORDER BY timestamp DESC
             LIMIT 100"
        )?;

        let mut autopilot_sessions = Vec::new();
        let mut claude_code_sessions = Vec::new();
        let mut autopilot_apm_sum = 0.0;
        let mut claude_code_apm_sum = 0.0;
        let mut total_actions = 0i64;

        let rows = stmt.query_map([&window_str], |row| {
            Ok((
                row.get::<_, String>(0)?,  // source
                row.get::<_, f64>(1)?,      // apm
                row.get::<_, i64>(2)?,      // actions
                row.get::<_, f64>(3)?,      // duration_minutes
                row.get::<_, i64>(4)?,      // messages
                row.get::<_, i64>(5)?,      // tool_calls
                row.get::<_, String>(6)?,   // timestamp
            ))
        })?;

        for row_result in rows {
            let (source, apm, actions, duration_minutes, messages, tool_calls, timestamp) = row_result?;
            total_actions += actions;

            let session_data = serde_json::json!({
                "timestamp": timestamp,
                "apm": apm,
                "actions": actions,
                "duration_minutes": duration_minutes,
                "messages": messages,
                "tool_calls": tool_calls,
            });

            if source == "autopilot" {
                autopilot_apm_sum += apm;
                autopilot_sessions.push(session_data);
            } else if source == "claude_code" {
                claude_code_apm_sum += apm;
                claude_code_sessions.push(session_data);
            }
        }

        let autopilot_avg = if !autopilot_sessions.is_empty() {
            autopilot_apm_sum / autopilot_sessions.len() as f64
        } else {
            0.0
        };

        let claude_code_avg = if !claude_code_sessions.is_empty() {
            claude_code_apm_sum / claude_code_sessions.len() as f64
        } else {
            0.0
        };

        let efficiency_ratio = if claude_code_avg > 0.0 {
            autopilot_avg / claude_code_avg
        } else {
            0.0
        };

        // Calculate histogram bins for APM distribution
        let all_apm_values: Vec<f64> = autopilot_sessions.iter()
            .chain(claude_code_sessions.iter())
            .filter_map(|s| s.get("apm").and_then(|v| v.as_f64()))
            .collect();

        let histogram_bins = calculate_histogram(&all_apm_values, 10);

        Ok(serde_json::json!({
            "window": window_str,
            "autopilot": {
                "sessions": autopilot_sessions,
                "avg_apm": autopilot_avg,
                "count": autopilot_sessions.len(),
            },
            "claude_code": {
                "sessions": claude_code_sessions,
                "avg_apm": claude_code_avg,
                "count": claude_code_sessions.len(),
            },
            "summary": {
                "efficiency_ratio": efficiency_ratio,
                "total_actions": total_actions,
            },
            "histogram": histogram_bins,
        }))
    })
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
    .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok().json(result))
}

/// Calculate histogram bins for APM values
fn calculate_histogram(values: &[f64], bin_count: usize) -> Vec<serde_json::Value> {
    if values.is_empty() {
        return vec![];
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let bin_width = (max - min) / bin_count as f64;

    let mut bins = vec![0; bin_count];
    for &value in values {
        let bin_index = ((value - min) / bin_width).floor() as usize;
        let bin_index = bin_index.min(bin_count - 1);
        bins[bin_index] += 1;
    }

    bins.iter()
        .enumerate()
        .map(|(i, &count)| {
            let bin_start = min + i as f64 * bin_width;
            let bin_end = bin_start + bin_width;
            serde_json::json!({
                "range": format!("{:.1}-{:.1}", bin_start, bin_end),
                "count": count,
                "bin_start": bin_start,
                "bin_end": bin_end,
            })
        })
        .collect()
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

/// WebSocket endpoint for real-time APM metrics streaming
async fn websocket_apm(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    state: web::Data<DashboardState>,
) -> ActixResult<HttpResponse> {
    use crate::apm::{APMSource, APMTier, APMWindow};
    use crate::apm_storage;

    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;
    let db_path = state.db_path.clone();

    actix_web::rt::spawn(async move {
        // Send initial connection message
        let _ = session.text(serde_json::json!({
            "type": "connected",
            "message": "APM metrics stream connected",
            "timestamp": Utc::now().to_rfc3339(),
        }).to_string()).await;

        // Create interval for periodic updates (every 5 seconds)
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            let _ = session.pong(&bytes).await;
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                // Send periodic APM updates
                _ = interval.tick() => {
                    // Open database connection
                    let conn = match rusqlite::Connection::open(&db_path) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    // Collect APM snapshots for different windows
                    let mut apm_data = serde_json::Map::new();

                    // Add Combined source data
                    for window in [APMWindow::Hour1, APMWindow::Hour6, APMWindow::Day1] {
                        if let Ok(Some(snapshot)) = apm_storage::get_latest_snapshot(&conn, APMSource::Combined, window) {
                            let window_key = match window {
                                APMWindow::Hour1 => "1h",
                                APMWindow::Hour6 => "6h",
                                APMWindow::Day1 => "1d",
                                _ => continue,
                            };

                            let tier = APMTier::from_apm(snapshot.apm);
                            let color = match tier {
                                APMTier::Baseline => "gray",
                                APMTier::Active => "blue",
                                APMTier::Productive => "green",
                                APMTier::HighPerformance => "amber",
                                APMTier::Elite => "gold",
                            };

                            apm_data.insert(window_key.to_string(), serde_json::json!({
                                "apm": snapshot.apm,
                                "actions": snapshot.actions,
                                "duration_minutes": snapshot.duration_minutes,
                                "messages": snapshot.messages,
                                "tool_calls": snapshot.tool_calls,
                                "tier": tier.name(),
                                "color": color,
                            }));
                        }
                    }

                    // Get current session APM if available
                    let current_session_apm = if let Ok(mut stmt) = conn.prepare(
                        "SELECT id, apm FROM apm_sessions WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1"
                    ) {
                        stmt.query_row([], |row| {
                            let session_id: String = row.get(0)?;
                            let apm: Option<f64> = row.get(1).ok();
                            Ok((session_id, apm))
                        }).ok()
                    } else {
                        None
                    };

                    if let Some((session_id, Some(apm))) = current_session_apm {
                        let tier = APMTier::from_apm(apm);
                        let color = match tier {
                            APMTier::Baseline => "gray",
                            APMTier::Active => "blue",
                            APMTier::Productive => "green",
                            APMTier::HighPerformance => "amber",
                            APMTier::Elite => "gold",
                        };

                        apm_data.insert("current_session".to_string(), serde_json::json!({
                            "session_id": session_id,
                            "apm": apm,
                            "tier": tier.name(),
                            "color": color,
                        }));
                    }

                    // Send APM update
                    let update = serde_json::json!({
                        "type": "apm_update",
                        "timestamp": Utc::now().to_rfc3339(),
                        "data": apm_data,
                    });

                    if let Ok(json) = serde_json::to_string(&update) {
                        if session.text(json).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}
