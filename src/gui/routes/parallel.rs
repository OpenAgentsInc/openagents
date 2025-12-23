//! Parallel agents routes for running multiple autopilot instances

use actix_web::{web, HttpResponse};
use maud::{html, Markup};
use serde::Deserialize;
use tracing::info;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(parallel_page));
}

/// Configure API routes
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/start", web::post().to(start_agents))
        .route("/stop", web::post().to(stop_agents))
        .route("/status", web::get().to(agent_status))
        .route("/logs/{agent_id}", web::get().to(agent_logs));
}

#[derive(Debug, Deserialize)]
struct StartAgentsForm {
    count: Option<usize>,
}

/// Parallel agents management page
async fn parallel_page() -> HttpResponse {
    // Get platform info
    let platform = autopilot::parallel::Platform::detect();
    let max_agents = platform.max_agents();
    let memory = platform.default_memory();
    let platform_name = match platform {
        autopilot::parallel::Platform::Linux => "Linux",
        autopilot::parallel::Platform::MacOS => "macOS",
    };

    // Get running agents
    let agents = autopilot::parallel::list_agents().await.unwrap_or_default();

    // Get open issues count (simplified)
    let open_issues = get_open_issues_count().await.unwrap_or(0);

    let content = render_parallel_page(&agents, open_issues, platform_name, max_agents, memory);

    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

fn render_parallel_page(
    agents: &[autopilot::parallel::AgentInfo],
    open_issues: usize,
    platform: &str,
    max_agents: usize,
    memory: &str,
) -> Markup {
    html! {
        script src="https://unpkg.com/htmx.org@1.9.10" {}

        div class="parallel-container" style="padding: 2rem; max-width: 1200px; margin: 0 auto;" {
            // Back button
            a href="/" style="display: inline-block; background: #111; border: 1px solid #333; padding: 0.5rem 0.75rem; color: #888; text-decoration: none; font-size: 0.75rem; font-family: monospace; margin-bottom: 1rem;" {
                "< Back to Home"
            }
            h1 style="color: #4a9eff; margin-bottom: 1.5rem;" { "Parallel Agents" }

            // Platform info
            div class="card" style="background: #2a2a2a; border: 1px solid #3a3a3a; padding: 1rem; margin-bottom: 1rem;" {
                h3 style="color: #a0a0a0; font-size: 0.875rem; margin-bottom: 0.5rem;" { "Platform" }
                p style="color: #e0e0e0;" { (platform) " - max " (max_agents) " agents @ " (memory) " each" }
                p style="color: #7dff7d; font-size: 0.875rem; margin-top: 0.5rem;" {
                    (open_issues) " open issues in queue"
                }
            }

            // Control panel
            div class="card" style="background: #2a2a2a; border: 1px solid #3a3a3a; padding: 1rem; margin-bottom: 1rem;" {
                h3 style="color: #a0a0a0; font-size: 0.875rem; margin-bottom: 0.5rem;" { "Control Panel" }
                div style="display: flex; gap: 1rem; align-items: center;" {
                    form hx-post="/api/parallel/start" hx-swap="none" hx-target="#agents-list" style="display: flex; gap: 0.5rem; align-items: center;" {
                        label for="agent-count" style="color: #e0e0e0;" { "Agents:" }
                        select name="count" id="agent-count" style="background: #1a1a1a; color: #e0e0e0; border: 1px solid #3a3a3a; padding: 0.25rem 0.5rem;" {
                            @for i in 1..=max_agents {
                                option value=(i) selected[i == 3] { (i) }
                            }
                        }
                        button type="submit" style="background: #2d5016; color: #7dff7d; border: none; padding: 0.5rem 1rem; cursor: pointer;" {
                            "Start Agents"
                        }
                    }
                    form hx-post="/api/parallel/stop" hx-swap="none" {
                        button type="submit" style="background: #501616; color: #ff7d7d; border: none; padding: 0.5rem 1rem; cursor: pointer;" {
                            "Stop All"
                        }
                    }
                }
            }

            // Running agents list
            div class="card" style="background: #2a2a2a; border: 1px solid #3a3a3a; padding: 1rem;" {
                div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" {
                    h3 style="color: #a0a0a0; font-size: 0.875rem; margin: 0;" { "Running Agents" }
                    span style="color: #4a9eff; font-size: 0.75rem;" {
                        (agents.len()) " active"
                    }
                }

                div id="agents-list" hx-get="/api/parallel/status" hx-trigger="every 5s" hx-swap="innerHTML" {
                    @if agents.is_empty() {
                        p style="color: #666; text-align: center; padding: 2rem;" {
                            "No agents running. Click 'Start Agents' to begin."
                        }
                    } @else {
                        table style="width: 100%; border-collapse: collapse;" {
                            thead {
                                tr style="border-bottom: 1px solid #3a3a3a;" {
                                    th style="text-align: left; padding: 0.5rem; color: #a0a0a0; font-size: 0.75rem;" { "ID" }
                                    th style="text-align: left; padding: 0.5rem; color: #a0a0a0; font-size: 0.75rem;" { "Status" }
                                    th style="text-align: left; padding: 0.5rem; color: #a0a0a0; font-size: 0.75rem;" { "Issue" }
                                    th style="text-align: left; padding: 0.5rem; color: #a0a0a0; font-size: 0.75rem;" { "Uptime" }
                                    th style="text-align: right; padding: 0.5rem; color: #a0a0a0; font-size: 0.75rem;" { "Actions" }
                                }
                            }
                            tbody {
                                @for agent in agents {
                                    tr style="border-bottom: 1px solid #2a2a2a;" {
                                        td style="padding: 0.5rem; color: #e0e0e0; font-family: monospace; font-size: 0.85rem;" {
                                            (agent.id.clone())
                                        }
                                        td style="padding: 0.5rem;" {
                                            (render_status(&agent.status))
                                        }
                                        td style="padding: 0.5rem; color: #e0e0e0; font-size: 0.85rem;" {
                                            @if let Some(issue) = agent.current_issue {
                                                "#" (issue)
                                            } @else {
                                                span style="color: #666;" { "-" }
                                            }
                                        }
                                        td style="padding: 0.5rem; color: #a0a0a0; font-size: 0.85rem;" {
                                            @if let Some(secs) = agent.uptime_seconds {
                                                (format_uptime(secs))
                                            } @else {
                                                "-"
                                            }
                                        }
                                        td style="padding: 0.5rem; text-align: right;" {
                                            button
                                                hx-get={"/api/parallel/logs/" (agent.id.clone())}
                                                hx-target="#log-modal"
                                                style="background: #1a3a5a; color: #4a9eff; border: none; padding: 0.25rem 0.5rem; cursor: pointer; font-size: 0.75rem;"
                                            { "Logs" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Log modal container
            div id="log-modal" {}

            // Instructions
            div class="card" style="background: #2a2a2a; border: 1px solid #3a3a3a; padding: 1rem; margin-top: 1rem;" {
                h3 style="color: #a0a0a0; font-size: 0.875rem; margin-bottom: 0.5rem;" { "How It Works" }
                ul style="color: #a0a0a0; font-size: 0.85rem; margin-left: 1rem;" {
                    li { "Each agent runs in an isolated Docker container with its own git worktree" }
                    li { "Agents automatically claim and work on open issues from the queue" }
                    li { "SQLite atomic locking prevents duplicate work" }
                    li { "Crashed agents are auto-restarted by the daemon" }
                }
                p style="color: #666; font-size: 0.75rem; margin-top: 0.5rem;" {
                    "CLI: " code style="background: #1a1a1a; padding: 0.125rem 0.25rem;" { "./scripts/parallel-autopilot.sh help" }
                }
            }
        }
    }
}

fn render_status(status: &autopilot::parallel::AgentStatus) -> Markup {
    let (color, text) = match status {
        autopilot::parallel::AgentStatus::Running => ("#7dff7d", "Running"),
        autopilot::parallel::AgentStatus::Stopped => ("#666", "Stopped"),
        autopilot::parallel::AgentStatus::Starting => ("#ffd97d", "Starting"),
        autopilot::parallel::AgentStatus::Error => ("#ff7d7d", "Error"),
    };
    html! {
        span style=(format!("color: {}; font-size: 0.85rem;", color)) { (text) }
    }
}

fn format_uptime(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    }
}

async fn get_open_issues_count() -> anyhow::Result<usize> {
    let db_path = "autopilot.db";
    if !std::path::Path::new(db_path).exists() {
        return Ok(0);
    }
    let conn = rusqlite::Connection::open(db_path)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE status = 'open'",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Start N parallel agents
async fn start_agents(form: web::Form<StartAgentsForm>) -> HttpResponse {
    let count = form.count.unwrap_or(3);
    info!("Starting {} parallel agents", count);

    match autopilot::parallel::start_agents(count).await {
        Ok(_agents) => {
            HttpResponse::Ok()
                .content_type("text/html")
                .body(format!(r#"<p style="color: #7dff7d; margin: 0;">Started {} agents</p>"#, count))
        }
        Err(e) => {
            tracing::error!("Failed to start agents: {}", e);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(format!(r#"<p style="color: #ff7d7d; margin: 0;">Error: {}</p>"#, html_escape(&e.to_string())))
        }
    }
}

/// Stop all parallel agents
async fn stop_agents() -> HttpResponse {
    info!("Stopping all parallel agents");

    match autopilot::parallel::stop_agents().await {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/html")
                .body(r#"<p style="color: #7dff7d; margin: 0;">All agents stopped</p>"#)
        }
        Err(e) => {
            tracing::error!("Failed to stop agents: {}", e);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(format!(r#"<p style="color: #ff7d7d; margin: 0;">Error: {}</p>"#, html_escape(&e.to_string())))
        }
    }
}

/// Get status of all agents (HTML for HTMX)
async fn agent_status() -> HttpResponse {
    match autopilot::parallel::list_agents().await {
        Ok(agents) => {
            if agents.is_empty() {
                return HttpResponse::Ok()
                    .content_type("text/html")
                    .body(r#"<p style="color: #666; margin: 0;">No agents running</p>"#);
            }

            // Simple compact status for pane
            let running = agents.iter().filter(|a| matches!(a.status, autopilot::parallel::AgentStatus::Running)).count();
            let html = format!(
                r#"<div style="color: #7dff7d;">{} agent{} running</div>
                <div style="color: #666; font-size: 0.65rem; margin-top: 0.25rem;">
                    {}
                </div>"#,
                running,
                if running == 1 { "" } else { "s" },
                agents.iter()
                    .map(|a| format!("{}: {}", a.id, if a.current_issue.is_some() { format!("#{}", a.current_issue.unwrap()) } else { "idle".to_string() }))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
            HttpResponse::Ok()
                .content_type("text/html")
                .body(html)
        }
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/html")
            .body(format!(r#"<p style="color: #ff7d7d;">Error: {}</p>"#, e)),
    }
}

/// Get logs for a specific agent
async fn agent_logs(path: web::Path<String>) -> HttpResponse {
    let agent_id = path.into_inner();

    match autopilot::parallel::get_logs(&agent_id, Some(100)).await {
        Ok(logs) => {
            let html = format!(
                r#"<div style="position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); background: #1a1a1a; border: 1px solid #3a3a3a; padding: 1rem; max-width: 800px; max-height: 600px; overflow: auto; z-index: 1000;">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; border-bottom: 1px solid #3a3a3a; padding-bottom: 0.5rem;">
                        <h3 style="color: #4a9eff; margin: 0;">Logs: agent-{}</h3>
                        <button onclick="this.closest('div').parentElement.innerHTML=''" style="background: #501616; color: #ff7d7d; border: none; padding: 0.25rem 0.5rem; cursor: pointer;">Close</button>
                    </div>
                    <pre style="font-family: monospace; font-size: 0.75rem; color: #a0a0a0; white-space: pre-wrap; word-wrap: break-word;">{}</pre>
                </div>"#,
                agent_id,
                html_escape(&logs)
            );

            HttpResponse::Ok()
                .content_type("text/html")
                .body(html)
        }
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/html")
            .body(format!(r#"<p style="color: #ff7d7d;">Error: {}</p>"#, e)),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
