//! HTTP server for daemon metrics and monitoring

use crate::daemon::metrics::DaemonMetricsCollector;
use crate::daemon::state::WorkerState;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use maud::{DOCTYPE, html};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for HTTP handlers
pub struct MetricsState {
    pub collector: Arc<DaemonMetricsCollector>,
    pub worker_state: Arc<RwLock<WorkerState>>,
}

/// GET /metrics - JSON metrics
#[get("/metrics")]
async fn metrics_json(state: web::Data<MetricsState>) -> impl Responder {
    let worker_state = state.worker_state.read().await;
    let current_memory = get_current_memory_mb();
    let snapshot = state.collector.snapshot(&worker_state, current_memory);

    HttpResponse::Ok().json(snapshot)
}

/// GET /dashboard - HTML dashboard
#[get("/dashboard")]
async fn dashboard(state: web::Data<MetricsState>) -> impl Responder {
    let worker_state = state.worker_state.read().await;
    let current_memory = get_current_memory_mb();
    let metrics = state.collector.snapshot(&worker_state, current_memory);

    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Autopilot Daemon Dashboard" }
                style {
                    r#"
                    * { margin: 0; padding: 0; box-sizing: border-box; }
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                        background: #0d1117;
                        color: #c9d1d9;
                        padding: 2rem;
                    }
                    .container { max-width: 1200px; margin: 0 auto; }
                    h1 { margin-bottom: 2rem; font-size: 2rem; }
                    .cards {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
                        gap: 1rem;
                        margin-bottom: 2rem;
                    }
                    .card {
                        background: #161b22;
                        border: 1px solid #30363d;
                        padding: 1.5rem;
                    }
                    .card h2 {
                        font-size: 0.875rem;
                        color: #8b949e;
                        margin-bottom: 0.5rem;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                    }
                    .card .value {
                        font-size: 2rem;
                        font-weight: bold;
                        color: #58a6ff;
                    }
                    .card .unit { font-size: 1rem; color: #8b949e; }
                    table {
                        width: 100%;
                        border-collapse: collapse;
                        background: #161b22;
                        border: 1px solid #30363d;
                        overflow: hidden;
                    }
                    th, td {
                        text-align: left;
                        padding: 1rem;
                        border-bottom: 1px solid #30363d;
                    }
                    th {
                        background: #0d1117;
                        color: #8b949e;
                        font-weight: 600;
                        text-transform: uppercase;
                        font-size: 0.75rem;
                        letter-spacing: 0.5px;
                    }
                    tr:last-child td { border-bottom: none; }
                    .status {
                        display: inline-block;
                        padding: 0.25rem 0.75rem;
                        font-size: 0.75rem;
                        font-weight: 600;
                    }
                    .status.crash { background: #da3633; color: white; }
                    .status.memory { background: #f0883e; color: white; }
                    .status.manual { background: #3fb950; color: white; }
                    .status.timeout { background: #d29922; color: white; }
                    "#
                }
                script {
                    r#"
                    // Auto-refresh every 5 seconds
                    setTimeout(() => location.reload(), 5000);
                    "#
                }
            }
            body {
                div class="container" {
                    h1 { "Autopilot Daemon Dashboard" }

                    div class="cards" {
                        div class="card" {
                            h2 { "Daemon Uptime" }
                            div class="value" { (format_duration(metrics.daemon_uptime_seconds)) }
                        }
                        div class="card" {
                            h2 { "Worker Uptime" }
                            div class="value" { (format_duration(metrics.worker_uptime_seconds)) }
                        }
                        div class="card" {
                            h2 { "Total Restarts" }
                            div class="value" { (metrics.total_restarts) }
                        }
                        div class="card" {
                            h2 { "Current Memory" }
                            div class="value" { (metrics.current_memory_mb.round() as u64) }
                            span class="unit" { " MB" }
                        }
                        div class="card" {
                            h2 { "Peak Memory" }
                            div class="value" { (metrics.peak_memory_mb.round() as u64) }
                            span class="unit" { " MB" }
                        }
                    }

                    h2 style="margin-bottom: 1rem;" { "Restart Reasons" }
                    div class="cards" style="margin-bottom: 2rem;" {
                        div class="card" {
                            h2 { "Crashes" }
                            div class="value" { (metrics.restart_reasons.crash) }
                        }
                        div class="card" {
                            h2 { "Memory Pressure" }
                            div class="value" { (metrics.restart_reasons.memory_pressure) }
                        }
                        div class="card" {
                            h2 { "Manual" }
                            div class="value" { (metrics.restart_reasons.manual) }
                        }
                        div class="card" {
                            h2 { "Timeout" }
                            div class="value" { (metrics.restart_reasons.timeout) }
                        }
                    }

                    @if !metrics.recent_restarts.is_empty() {
                        h2 style="margin-bottom: 1rem;" { "Recent Restarts" }
                        table {
                            thead {
                                tr {
                                    th { "Timestamp" }
                                    th { "Reason" }
                                    th { "Uptime" }
                                    th { "Memory" }
                                }
                            }
                            tbody {
                                @for restart in &metrics.recent_restarts {
                                    tr {
                                        td { (restart.timestamp.format("%Y-%m-%d %H:%M:%S")) }
                                        td {
                                            span class=(format!("status {}", categorize_reason(&restart.reason))) {
                                                (restart.reason)
                                            }
                                        }
                                        td { (format_duration(restart.uptime_seconds)) }
                                        td {
                                            @if let Some(mem) = restart.memory_mb {
                                                (mem.round() as u64) " MB"
                                            } @else {
                                                "—"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(markup.into_string())
}

/// Format duration in seconds to human-readable string
fn format_duration(seconds: f64) -> String {
    let secs = seconds as u64;
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Categorize restart reason for CSS class
fn categorize_reason(reason: &str) -> &'static str {
    let reason_lower = reason.to_lowercase();
    if reason_lower.contains("crash") || reason_lower.contains("fail") {
        "crash"
    } else if reason_lower.contains("memory") {
        "memory"
    } else if reason_lower.contains("manual") {
        "manual"
    } else if reason_lower.contains("timeout") || reason_lower.contains("stall") {
        "timeout"
    } else {
        "crash"
    }
}

/// Get current memory usage in MB
fn get_current_memory_mb() -> f64 {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_memory();

    let used_memory = sys.used_memory();
    // Convert from bytes to MB
    (used_memory as f64) / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_memory_mb() {
        let memory_mb = get_current_memory_mb();
        // Memory usage should be positive and reasonable (< 100GB)
        assert!(memory_mb > 0.0, "Memory usage should be positive");
        assert!(memory_mb < 100_000.0, "Memory usage should be < 100GB");
    }
}

/// Start HTTP metrics server
pub async fn start_metrics_server(
    collector: Arc<DaemonMetricsCollector>,
    worker_state: Arc<RwLock<WorkerState>>,
    port: u16,
) -> std::io::Result<()> {
    let state = web::Data::new(MetricsState {
        collector,
        worker_state,
    });

    eprintln!("Starting metrics server on http://127.0.0.1:{}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(metrics_json)
            .service(dashboard)
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}
