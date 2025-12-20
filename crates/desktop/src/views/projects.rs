//! Projects management view

use maud::{Markup, html, PreEscaped};
use super::layout;

/// Project data structure for display
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub session_count: i64,
    pub last_active: Option<String>,
}

/// Full projects page
#[allow(dead_code)]
pub fn projects_page(projects: Vec<ProjectRow>) -> Markup {
    layout(
        "Projects - OpenAgents",
        html! {
            div class="w-full max-w-6xl mx-auto p-8" {
                div class="flex justify-between items-center mb-8" {
                    h1 class="text-3xl font-semibold" {
                        "Projects"
                    }
                }

                // Add project form
                div class="mb-8 p-6 border border-border bg-card" {
                    h2 class="text-xl font-semibold mb-4" {
                        "Add New Project"
                    }
                    form id="add-project-form" class="flex gap-4" {
                        input
                            type="text"
                            name="name"
                            placeholder="Project Name"
                            required
                            class="flex-1 px-4 py-2 bg-background border border-border text-foreground font-mono";
                        input
                            type="text"
                            name="path"
                            placeholder="Project Path"
                            required
                            class="flex-1 px-4 py-2 bg-background border border-border text-foreground font-mono";
                        button
                            type="submit"
                            class="px-6 py-2 bg-green text-background font-mono hover:opacity-80";
                            {
                            "Add Project"
                        }
                    }
                }

                // Projects table
                @if projects.is_empty() {
                    div class="text-center py-12 text-muted-foreground" {
                        p { "No projects yet. Add one above to get started." }
                    }
                } @else {
                    table class="w-full border-collapse" {
                        thead {
                            tr class="border-b border-border" {
                                th class="text-left py-3 px-4 font-semibold" { "Name" }
                                th class="text-left py-3 px-4 font-semibold" { "Path" }
                                th class="text-right py-3 px-4 font-semibold" { "Sessions" }
                                th class="text-left py-3 px-4 font-semibold" { "Last Active" }
                                th class="text-right py-3 px-4 font-semibold" { "Actions" }
                            }
                        }
                        tbody {
                            @for project in projects {
                                tr class="border-b border-border hover:bg-card" {
                                    td class="py-3 px-4 font-mono" { (project.name) }
                                    td class="py-3 px-4 font-mono text-sm text-muted-foreground" { (project.path) }
                                    td class="py-3 px-4 text-right font-mono" { (project.session_count) }
                                    td class="py-3 px-4 font-mono text-sm" {
                                        @if let Some(last_active) = project.last_active {
                                            (last_active)
                                        } @else {
                                            span class="text-muted-foreground" { "Never" }
                                        }
                                    }
                                    td class="py-3 px-4 text-right" {
                                        button
                                            class="px-4 py-1 text-sm border border-red text-red hover:bg-red hover:text-background font-mono"
                                            onclick=(format!("confirmRemove('{}')", project.id));
                                            {
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Scripts for form submission and removal
                script {
                    (PreEscaped(r#"
                    document.getElementById('add-project-form').onsubmit = async function(e) {
                        e.preventDefault();
                        const formData = new FormData(e.target);
                        const data = {
                            name: formData.get('name'),
                            path: formData.get('path')
                        };

                        const response = await fetch('/projects/add', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify(data)
                        });

                        if (response.ok) {
                            location.reload();
                        } else {
                            const error = await response.text();
                            alert('Error: ' + error);
                        }
                    };

                    function confirmRemove(projectId) {
                        if (confirm('Are you sure you want to remove this project? All associated sessions will be deleted.')) {
                            fetch('/projects/remove/' + projectId, { method: 'POST' })
                                .then(response => {
                                    if (response.ok) {
                                        location.reload();
                                    } else {
                                        return response.text().then(err => alert('Error: ' + err));
                                    }
                                });
                        }
                    }
                    "#))
                }
            }
        },
    )
}
