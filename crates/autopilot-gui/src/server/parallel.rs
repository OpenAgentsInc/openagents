//! HTTP API endpoints for parallel agents management

use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::views::{
    parallel_agents_page, agents_list, AgentViewInfo, AgentViewStatus, IssueViewInfo, PlatformViewInfo,
};
use crate::views::layout;

/// Request body for starting agents
#[derive(Debug, Deserialize)]
pub struct StartAgentsRequest {
    pub count: Option<usize>,
}

/// Response for agent operations
#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<AgentInfo>>,
}

/// Agent info for API responses
#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub status: String,
    pub current_issue: Option<i32>,
    pub uptime: Option<String>,
}

/// Parallel agents page
#[get("/parallel")]
pub async fn parallel_view() -> impl Responder {
    // Get agent status from autopilot parallel module
    let agents = match autopilot::parallel::list_agents().await {
        Ok(agent_list) => agent_list
            .into_iter()
            .map(|a| AgentViewInfo {
                id: a.id,
                status: match a.status {
                    autopilot::parallel::AgentStatus::Running => AgentViewStatus::Running,
                    autopilot::parallel::AgentStatus::Stopped => AgentViewStatus::Stopped,
                    autopilot::parallel::AgentStatus::Starting => AgentViewStatus::Running,
                    autopilot::parallel::AgentStatus::Error => AgentViewStatus::Stopped,
                },
                current_issue: a.current_issue,
                uptime: a.uptime_seconds.map(format_uptime),
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    // Get open issues (placeholder - would come from issues database)
    let open_issues = get_open_issues().await.unwrap_or_default();

    // Get platform info
    let platform = autopilot::parallel::Platform::detect();
    let platform_info = PlatformViewInfo {
        platform: match platform {
            autopilot::parallel::Platform::Linux => "Linux".to_string(),
            autopilot::parallel::Platform::MacOS => "macOS".to_string(),
        },
        max_agents: platform.max_agents(),
        memory_per_agent: platform.default_memory().to_string(),
    };

    let content = parallel_agents_page(agents, open_issues, platform_info);
    let html = layout::page_with_current("Parallel Agents - Autopilot GUI", content, Some("parallel"));

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Start N parallel agents
#[post("/api/parallel/start")]
pub async fn start_agents(body: web::Form<StartAgentsRequest>) -> impl Responder {
    let count = body.count.unwrap_or(3);

    match autopilot::parallel::start_agents(count).await {
        Ok(agents) => {
            let agent_infos: Vec<AgentInfo> = agents
                .into_iter()
                .map(|a| AgentInfo {
                    id: a.id,
                    status: format!("{:?}", a.status).to_lowercase(),
                    current_issue: a.current_issue,
                    uptime: a.uptime_seconds.map(format_uptime),
                })
                .collect();

            HttpResponse::Ok().json(AgentResponse {
                success: true,
                message: Some(format!("Started {} agents", count)),
                agents: Some(agent_infos),
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(AgentResponse {
            success: false,
            message: Some(format!("Failed to start agents: {}", e)),
            agents: None,
        }),
    }
}

/// Stop all parallel agents
#[post("/api/parallel/stop")]
pub async fn stop_agents() -> impl Responder {
    match autopilot::parallel::stop_agents().await {
        Ok(_) => HttpResponse::Ok().json(AgentResponse {
            success: true,
            message: Some("All agents stopped".to_string()),
            agents: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(AgentResponse {
            success: false,
            message: Some(format!("Failed to stop agents: {}", e)),
            agents: None,
        }),
    }
}

/// Get status of all agents (for HTMX polling)
#[get("/api/parallel/status")]
pub async fn agent_status() -> impl Responder {
    match autopilot::parallel::list_agents().await {
        Ok(agent_list) => {
            let agents: Vec<AgentViewInfo> = agent_list
                .into_iter()
                .map(|a| AgentViewInfo {
                    id: a.id,
                    status: match a.status {
                        autopilot::parallel::AgentStatus::Running => AgentViewStatus::Running,
                        autopilot::parallel::AgentStatus::Stopped => AgentViewStatus::Stopped,
                        autopilot::parallel::AgentStatus::Starting => AgentViewStatus::Running,
                        autopilot::parallel::AgentStatus::Error => AgentViewStatus::Stopped,
                    },
                    current_issue: a.current_issue,
                    uptime: a.uptime_seconds.map(format_uptime),
                })
                .collect();

            let html = agents_list(&agents).into_string();
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html)
        }
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body(format!("<p class=\"error\">Error: {}</p>", e)),
    }
}

/// Get logs for a specific agent
#[get("/api/parallel/logs/{agent_id}")]
pub async fn agent_logs(path: web::Path<String>) -> impl Responder {
    let agent_id = path.into_inner();

    match autopilot::parallel::get_logs(&agent_id, Some(100)).await {
        Ok(logs) => {
            let html = format!(
                r#"<div class="log-modal">
                    <div class="log-header">
                        <h3>Logs: agent-{}</h3>
                        <button onclick="this.closest('.log-modal').remove()" class="btn-close">Close</button>
                    </div>
                    <pre class="log-content">{}</pre>
                </div>
                <style>
                    .log-modal {{
                        position: fixed;
                        top: 50%;
                        left: 50%;
                        transform: translate(-50%, -50%);
                        background: #1a1a1a;
                        border: 1px solid #3a3a3a;
                        padding: 1rem;
                        max-width: 800px;
                        max-height: 600px;
                        overflow: auto;
                        z-index: 1000;
                    }}
                    .log-header {{
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 1rem;
                        border-bottom: 1px solid #3a3a3a;
                        padding-bottom: 0.5rem;
                    }}
                    .log-header h3 {{
                        color: #4a9eff;
                        margin: 0;
                    }}
                    .btn-close {{
                        background: #501616;
                        color: #ff7d7d;
                        border: none;
                        padding: 0.25rem 0.5rem;
                        cursor: pointer;
                    }}
                    .log-content {{
                        font-family: monospace;
                        font-size: 0.75rem;
                        color: #a0a0a0;
                        white-space: pre-wrap;
                        word-wrap: break-word;
                    }}
                </style>"#,
                agent_id,
                html_escape(&logs)
            );

            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html)
        }
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body(format!("<p class=\"error\">Error: {}</p>", e)),
    }
}

/// Format uptime in seconds to human readable
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

/// Get open issues from the database
async fn get_open_issues() -> anyhow::Result<Vec<IssueViewInfo>> {
    // Try to read from autopilot.db
    let db_path = "autopilot.db";
    if !std::path::Path::new(db_path).exists() {
        return Ok(Vec::new());
    }

    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT number, title, priority FROM issues WHERE status = 'open' ORDER BY
         CASE priority
             WHEN 'urgent' THEN 1
             WHEN 'high' THEN 2
             WHEN 'medium' THEN 3
             WHEN 'low' THEN 4
             ELSE 5
         END, number"
    )?;

    let issues = stmt.query_map([], |row| {
        Ok(IssueViewInfo {
            number: row.get(0)?,
            title: row.get(1)?,
            priority: row.get::<_, String>(2)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(issues)
}

/// Simple HTML escaping
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
