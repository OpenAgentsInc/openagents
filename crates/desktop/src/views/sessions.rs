//! Sessions history view

use maud::{Markup, html, PreEscaped};
use super::layout;

/// Session data structure for display
#[derive(Debug)]
pub struct SessionRow {
    pub id: String,
    pub project_name: String,
    pub status: String,
    pub started_at: String,
    pub duration: Option<String>,
    pub cost: f64,
}

/// Project option for filter dropdown
#[derive(Debug)]
pub struct ProjectOption {
    pub id: String,
    pub name: String,
}

/// Full sessions page
pub fn sessions_page(
    sessions: Vec<SessionRow>,
    projects: Vec<ProjectOption>,
    selected_project: Option<String>,
) -> Markup {
    layout(
        "Sessions - OpenAgents",
        html! {
            div class="w-full max-w-6xl mx-auto p-8" {
                div class="flex justify-between items-center mb-8" {
                    h1 class="text-3xl font-semibold" {
                        "Sessions"
                    }

                    // Project filter dropdown
                    @if !projects.is_empty() {
                        div class="flex items-center gap-4" {
                            label class="text-sm text-muted-foreground" { "Filter by project:" }
                            select
                                id="project-filter"
                                class="px-4 py-2 bg-background border border-border text-foreground font-mono";
                                {
                                option value="" selected[selected_project.is_none()] { "All Projects" }
                                @for project in projects {
                                    option
                                        value=(project.id)
                                        selected[selected_project.as_ref() == Some(&project.id)];
                                        {
                                        (project.name)
                                    }
                                }
                            }
                        }
                    }
                }

                // Sessions table
                @if sessions.is_empty() {
                    div class="text-center py-12 text-muted-foreground" {
                        p { "No sessions found." }
                    }
                } @else {
                    table class="w-full border-collapse" {
                        thead {
                            tr class="border-b border-border" {
                                th class="text-left py-3 px-4 font-semibold" { "Session ID" }
                                th class="text-left py-3 px-4 font-semibold" { "Project" }
                                th class="text-left py-3 px-4 font-semibold" { "Status" }
                                th class="text-left py-3 px-4 font-semibold" { "Started" }
                                th class="text-left py-3 px-4 font-semibold" { "Duration" }
                                th class="text-right py-3 px-4 font-semibold" { "Cost" }
                            }
                        }
                        tbody {
                            @for session in sessions {
                                tr
                                    class="border-b border-border hover:bg-card cursor-pointer"
                                    onclick=(format!("openSession('{}')", session.id));
                                    {
                                    td class="py-3 px-4 font-mono text-sm" {
                                        (format!("{}...", &session.id[..8]))
                                    }
                                    td class="py-3 px-4 font-mono" { (session.project_name) }
                                    td class="py-3 px-4" {
                                        (status_badge(&session.status))
                                    }
                                    td class="py-3 px-4 font-mono text-sm" { (session.started_at) }
                                    td class="py-3 px-4 font-mono text-sm" {
                                        @if let Some(duration) = session.duration {
                                            (duration)
                                        } @else {
                                            span class="text-muted-foreground" { "Running" }
                                        }
                                    }
                                    td class="py-3 px-4 text-right font-mono" {
                                        (format!("${:.2}", session.cost))
                                    }
                                }
                            }
                        }
                    }
                }

                // Scripts for filtering and navigation
                script {
                    (PreEscaped(r#"
                    document.getElementById('project-filter')?.addEventListener('change', function(e) {
                        const projectId = e.target.value;
                        const url = projectId ? '/sessions?project=' + projectId : '/sessions';
                        window.location.href = url;
                    });

                    function openSession(sessionId) {
                        window.location.href = '/autopilot/replay?session=' + sessionId;
                    }
                    "#))
                }
            }
        },
    )
}

/// Status badge with color coding
fn status_badge(status: &str) -> Markup {
    let (color_class, display_text) = match status {
        "running" => ("text-blue border-blue", "Running"),
        "completed" => ("text-green border-green", "Completed"),
        "failed" => ("text-red border-red", "Failed"),
        "cancelled" => ("text-muted-foreground border-muted-foreground", "Cancelled"),
        _ => ("text-foreground border-foreground", status),
    };

    html! {
        span class=(format!("px-2 py-1 text-xs border font-mono {}", color_class)) {
            (display_text)
        }
    }
}
