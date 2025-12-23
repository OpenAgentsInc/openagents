//! Permission rules management view

use crate::storage::PermissionRule;
use maud::{html, Markup, DOCTYPE};

/// Permission manager view
pub fn permissions_view(rules: Vec<PermissionRule>) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Permission Rules - Autopilot" }
                style {
                    r#"
                    * {
                        margin: 0;
                        padding: 0;
                        box-sizing: border-box;
                    }

                    body {
                        font-family: system-ui, -apple-system, sans-serif;
                        background: #1a1a1a;
                        color: #e0e0e0;
                        line-height: 1.6;
                    }

                    .container {
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 2rem;
                    }

                    .header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 2rem;
                        padding-bottom: 1rem;
                        border-bottom: 2px solid #3a3a3a;
                    }

                    .header h1 {
                        color: #4a9eff;
                        font-size: 1.75rem;
                    }

                    .header .back-link {
                        color: #4a9eff;
                        text-decoration: none;
                        padding: 0.5rem 1rem;
                        border: 1px solid #4a9eff;
                        background: transparent;
                        cursor: pointer;
                        font-weight: bold;
                    }

                    .header .back-link:hover {
                        background: #4a9eff;
                        color: #1a1a1a;
                    }

                    .rules-list {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                    }

                    .rule-card {
                        background: #2a2a2a;
                        border: 1px solid #3a3a3a;
                        padding: 1.5rem;
                        display: grid;
                        grid-template-columns: auto 1fr auto auto;
                        gap: 1.5rem;
                        align-items: center;
                    }

                    .rule-status {
                        font-size: 2rem;
                    }

                    .rule-info {
                        flex: 1;
                    }

                    .rule-pattern {
                        font-family: monospace;
                        font-size: 1.125rem;
                        font-weight: bold;
                        margin-bottom: 0.5rem;
                    }

                    .rule-meta {
                        font-size: 0.875rem;
                        color: #b0b0b0;
                        display: flex;
                        gap: 1.5rem;
                    }

                    .badge {
                        padding: 0.25rem 0.75rem;
                        font-size: 0.75rem;
                        font-weight: bold;
                        text-transform: uppercase;
                    }

                    .badge.persistent {
                        background: #2a4a7c;
                        color: #7daaff;
                    }

                    .badge.session {
                        background: #4a3a1a;
                        color: #ffaa7d;
                    }

                    .rule-actions {
                        display: flex;
                        gap: 0.5rem;
                    }

                    .btn {
                        padding: 0.5rem 1rem;
                        border: none;
                        cursor: pointer;
                        font-weight: bold;
                        font-size: 0.875rem;
                    }

                    .btn-edit {
                        background: #3a5a3a;
                        color: #7dff7d;
                    }

                    .btn-delete {
                        background: #5a3a3a;
                        color: #ff7d7d;
                    }

                    .btn:hover {
                        filter: brightness(1.2);
                    }

                    .empty-state {
                        text-align: center;
                        padding: 4rem 2rem;
                        color: #b0b0b0;
                    }

                    .empty-state h2 {
                        margin-bottom: 1rem;
                        color: #7a7a7a;
                    }

                    .add-rule-btn {
                        background: #2a5a2a;
                        color: #7dff7d;
                        border: 1px solid #3a7a3a;
                        padding: 0.75rem 1.5rem;
                        cursor: pointer;
                        font-weight: bold;
                        font-size: 1rem;
                    }

                    .add-rule-btn:hover {
                        filter: brightness(1.2);
                    }
                    "#
                }
            }
            body {
                div class="container" {
                    div class="header" {
                        h1 { "🔐 Permission Rules" }
                        a href="/" class="back-link" { "← Back to Chat" }
                    }

                    button class="add-rule-btn" onclick="addNewRule()" {
                        "+ Add New Rule"
                    }

                    @if rules.is_empty() {
                        div class="empty-state" {
                            h2 { "No Permission Rules" }
                            p { "Permission rules let you auto-approve or auto-reject tool executions." }
                            p style="margin-top: 0.5rem;" {
                                "Rules are created when you click 'Always Allow' or 'Always Reject' in permission dialogs."
                            }
                        }
                    } @else {
                        div class="rules-list" {
                            @for rule in &rules {
                                (rule_card(rule))
                            }
                        }
                    }
                }

                script {
                    r#"
                    function deleteRule(id) {
                        if (confirm('Are you sure you want to delete this rule?')) {
                            fetch(`/api/permissions/${id}`, {
                                method: 'DELETE'
                            })
                            .then(() => {
                                window.location.reload();
                            })
                            .catch(err => {
                                alert('Failed to delete rule: ' + err);
                            });
                        }
                    }

                    function editRule(id) {
                        const rules = window.RULES || [];
                        const rule = rules.find(r => r.id === id);
                        if (!rule) {
                            alert('Rule not found');
                            return;
                        }

                        const pattern = prompt('Edit pattern:', rule.pattern);
                        if (!pattern || pattern === rule.pattern) return;

                        const allowed = confirm('Allow this pattern? (OK=Allow, Cancel=Deny)');
                        const persistent = confirm('Make this rule persistent? (OK=Persistent, Cancel=Session only)');

                        fetch(`/api/permissions/${id}`, {
                            method: 'PUT',
                            headers: {'Content-Type': 'application/json'},
                            body: JSON.stringify({pattern, allowed, persistent})
                        })
                        .then(res => res.json())
                        .then(() => {
                            window.location.reload();
                        })
                        .catch(err => {
                            alert('Failed to update rule: ' + err);
                        });
                    }

                    function addNewRule() {
                        const pattern = prompt('Enter pattern (e.g., "Bash:npm" or "Edit:*.rs"):');
                        if (!pattern) return;

                        const allowed = confirm('Allow this pattern? (OK=Allow, Cancel=Deny)');
                        const persistent = confirm('Make this rule persistent? (OK=Persistent, Cancel=Session only)');

                        fetch('/api/permissions', {
                            method: 'POST',
                            headers: {'Content-Type': 'application/json'},
                            body: JSON.stringify({pattern, allowed, persistent})
                        })
                        .then(res => res.json())
                        .then(() => {
                            window.location.reload();
                        })
                        .catch(err => {
                            alert('Failed to add rule: ' + err);
                        });
                    }

                    "#;

                    // Store rules data for edit function
                    (format!("window.RULES = {};", serde_json::to_string(&rules).unwrap_or_default()))
                }
            }
        }
    }
}

/// Render a single rule card
fn rule_card(rule: &PermissionRule) -> Markup {
    let status_icon = if rule.allowed { "✓" } else { "✗" };
    let status_color = if rule.allowed { "#7dff7d" } else { "#ff7d7d" };

    html! {
        div class="rule-card" {
            div class="rule-status" style=(format!("color: {}", status_color)) {
                (status_icon)
            }

            div class="rule-info" {
                div class="rule-pattern" style=(format!("color: {}", status_color)) {
                    (rule.pattern)
                }
                div class="rule-meta" {
                    span { (rule.created_at.format("%Y-%m-%d %H:%M:%S")) }
                    span class=(format!("badge {}", if rule.persistent { "persistent" } else { "session" })) {
                        (if rule.persistent { "Persistent" } else { "Session Only" })
                    }
                }
            }

            div class="rule-actions" {
                button
                    class="btn btn-edit"
                    onclick=(format!("editRule({})", rule.id))
                {
                    "Edit"
                }
                button
                    class="btn btn-delete"
                    onclick=(format!("deleteRule({})", rule.id))
                {
                    "Delete"
                }
            }
        }
    }
}
