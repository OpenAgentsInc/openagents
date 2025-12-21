//! Permission dialog component

use crate::agent::PermissionRequest;
use maud::{html, Markup};

/// Render a permission request dialog
pub fn permission_dialog(request: &PermissionRequest) -> Markup {
    let pattern = request.pattern();
    let tool = &request.tool;
    let input_json = serde_json::to_string_pretty(&request.input)
        .unwrap_or_else(|_| request.input.to_string());

    html! {
        div
            id=(format!("permission-dialog-{}", request.id))
            class="permission-dialog"
            style="position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); background: #2a2a2a; border: 2px solid #4a9eff; padding: 2rem; z-index: 1000; min-width: 600px; max-width: 800px;"
        {
            // Header
            div style="margin-bottom: 1.5rem; border-bottom: 1px solid #3a3a3a; padding-bottom: 1rem;" {
                h2 style="margin: 0; color: #4a9eff; font-size: 1.25rem;" {
                    "🔐 Permission Required"
                }
                div style="font-size: 0.875rem; color: #b0b0b0; margin-top: 0.5rem;" {
                    (request.timestamp.format("%H:%M:%S"))
                }
            }

            // Tool info
            div style="margin-bottom: 1.5rem;" {
                div style="margin-bottom: 0.5rem;" {
                    span style="font-weight: bold; color: #e0e0e0;" { "Tool: " }
                    span style="color: #4a9eff; font-family: monospace;" { (tool) }
                }

                div style="margin-bottom: 0.5rem;" {
                    span style="font-weight: bold; color: #e0e0e0;" { "Pattern: " }
                    span style="color: #7dff7d; font-family: monospace;" { (pattern) }
                }

                @if let Some(desc) = &request.description {
                    div style="margin-top: 1rem; padding: 0.75rem; background: #1a1a1a; border-left: 3px solid #4a9eff;" {
                        (desc)
                    }
                }
            }

            // Input details
            details style="margin-bottom: 1.5rem;" {
                summary style="cursor: pointer; color: #b0b0b0; margin-bottom: 0.5rem; user-select: none;" {
                    "View Input Parameters"
                }
                pre style="background: #1a1a1a; padding: 1rem; overflow-x: auto; font-size: 0.875rem; color: #d0d0d0; border: 1px solid #3a3a3a; font-family: monospace;" {
                    code { (input_json) }
                }
            }

            // Persistence option
            div style="margin-bottom: 1.5rem; padding: 1rem; background: #1a2a1a; border: 1px solid #3a5a3a;" {
                label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer; color: #e0e0e0;" {
                    input
                        type="checkbox"
                        id=(format!("persistent-{}", request.id))
                        name="persistent"
                        checked;
                    span { "Make rule persistent (save across sessions)" }
                }
            }

            // Action buttons
            div style="display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem;" {
                // Allow buttons
                div style="display: flex; flex-direction: column; gap: 0.5rem;" {
                    button
                        class="permission-action"
                        data-request-id=(request.id)
                        data-action="allow"
                        style="background: #2a5a2a; color: #7dff7d; border: 1px solid #3a7a3a; padding: 0.75rem; cursor: pointer; font-weight: bold; font-size: 1rem;"
                    {
                        "✓ Allow Once"
                    }

                    button
                        class="permission-action"
                        data-request-id=(request.id)
                        data-action="always-allow"
                        data-pattern=(pattern)
                        style="background: #1a4a1a; color: #7dff7d; border: 1px solid #2a6a2a; padding: 0.75rem; cursor: pointer; font-weight: bold;"
                    {
                        "✓✓ Always Allow " (tool)
                    }
                }

                // Reject buttons
                div style="display: flex; flex-direction: column; gap: 0.5rem;" {
                    button
                        class="permission-action"
                        data-request-id=(request.id)
                        data-action="reject"
                        style="background: #5a2a2a; color: #ff7d7d; border: 1px solid #7a3a3a; padding: 0.75rem; cursor: pointer; font-weight: bold; font-size: 1rem;"
                    {
                        "✗ Reject Once"
                    }

                    button
                        class="permission-action"
                        data-request-id=(request.id)
                        data-action="always-reject"
                        data-pattern=(pattern)
                        style="background: #4a1a1a; color: #ff7d7d; border: 1px solid #6a2a2a; padding: 0.75rem; cursor: pointer; font-weight: bold;"
                    {
                        "✗✗ Always Reject " (tool)
                    }
                }
            }

            // Helper text
            div style="margin-top: 1rem; padding-top: 1rem; border-top: 1px solid #3a3a3a; font-size: 0.875rem; color: #b0b0b0; text-align: center;" {
                "Tip: Use 'Always' to create permission rules for similar requests"
            }
        }

        // Backdrop
        div
            id=(format!("permission-backdrop-{}", request.id))
            style="position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.7); z-index: 999;"
        {}
    }
}

/// Render pending permission count badge
pub fn permission_badge(count: usize) -> Markup {
    if count == 0 {
        html! {}
    } else {
        html! {
            div
                id="permission-badge"
                style="position: fixed; top: 1rem; right: 1rem; background: #c44; color: white; padding: 0.5rem 1rem; font-weight: bold; border: 2px solid #fff; z-index: 1001; animation: pulse 1s ease-in-out infinite;"
            {
                (count) " pending permission" (if count > 1 { "s" } else { "" })
            }
        }
    }
}

/// JavaScript for permission dialog interaction
pub fn permission_dialog_script() -> Markup {
    html! {
        script {
            r#"
            (function() {
                // Handle permission action buttons
                document.addEventListener('click', function(e) {
                    if (e.target.classList.contains('permission-action')) {
                        const requestId = e.target.dataset.requestId;
                        const action = e.target.dataset.action;
                        const pattern = e.target.dataset.pattern;
                        const persistentCheckbox = document.getElementById(`persistent-${requestId}`);
                        const persistent = persistentCheckbox ? persistentCheckbox.checked : true;

                        // Send response via WebSocket
                        if (window.ws && window.ws.readyState === WebSocket.OPEN) {
                            const response = {
                                type: 'permission_response',
                                request_id: requestId,
                                action: action,
                                pattern: pattern || null,
                                persistent: persistent
                            };

                            window.ws.send(JSON.stringify(response));
                        }

                        // Remove dialog
                        const dialog = document.getElementById(`permission-dialog-${requestId}`);
                        const backdrop = document.getElementById(`permission-backdrop-${requestId}`);
                        if (dialog) dialog.remove();
                        if (backdrop) backdrop.remove();

                        // Update badge count
                        const badge = document.getElementById('permission-badge');
                        if (badge) {
                            const remaining = document.querySelectorAll('.permission-dialog').length;
                            if (remaining === 0) {
                                badge.remove();
                            } else {
                                const suffix = remaining > 1 ? 's' : '';
                                badge.textContent = `${remaining} pending permission${suffix}`;
                            }
                        }
                    }
                });

                // Add pulse animation
                const style = document.createElement('style');
                style.textContent = `
                    @keyframes pulse {
                        0%, 100% { opacity: 1; }
                        50% { opacity: 0.7; }
                    }

                    .permission-action:hover {
                        filter: brightness(1.2);
                    }
                `;
                document.head.appendChild(style);
            })();
            "#
        }
    }
}
