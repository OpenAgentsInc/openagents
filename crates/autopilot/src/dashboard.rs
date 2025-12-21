//! Web dashboard for autopilot metrics visualization
//!
//! Provides an Actix-web server that displays:
//! - Session list with key statistics
//! - Trend charts for metrics over time
//! - Anomaly highlighting
//! - Session detail drill-down
//! - Real-time updates via WebSocket
//! - JSON/CSV export capabilities

use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use actix_ws::Message;
use maud::{html, Markup, DOCTYPE};
use serde::{Deserialize, Serialize};
use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus};

/// Dashboard application state
pub struct DashboardState {
    db_path: String,
}

/// Start the metrics dashboard server
pub async fn start_dashboard(db_path: &str, port: u16) -> anyhow::Result<()> {
    // Test that we can open the database
    MetricsDb::open(db_path)?;

    let state = web::Data::new(DashboardState {
        db_path: db_path.to_string(),
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
fn dashboard_page(sessions: &[SessionMetrics], stats: &SummaryStats) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Autopilot Metrics Dashboard" }
                style { (dashboard_styles()) }
            }
            body {
                header {
                    h1 { "⚡ Autopilot Metrics Dashboard" }
                    p.subtitle { "Continual Constant Improvement (d-004)" }
                }

                main {
                    (summary_card(stats))
                    (sessions_table(sessions))
                }

                footer {
                    p { "OpenAgents Autopilot • "
                        a href="/export/sessions.json" { "JSON" }
                        " | "
                        a href="/export/sessions.csv" { "CSV" }
                    }
                }
            }
        }
    };

    markup.into_string()
}

/// Render summary statistics card
fn summary_card(stats: &SummaryStats) -> Markup {
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
fn sessions_table(sessions: &[SessionMetrics]) -> Markup {
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
            td { (session.timestamp.format("%Y-%m-%d %H:%M")) }
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
                            dd { (session.timestamp) }
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
