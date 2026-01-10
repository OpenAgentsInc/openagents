/// Render notifications page
pub fn notifications_page(notifications: &[crate::nostr::cache::Notification]) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Notifications - GitAfter" }
                style {
                    r#"
                    * { margin: 0; padding: 0; box-sizing: border-box; }
                    body { font-family: system-ui, -apple-system, sans-serif; background: #0d1117; color: #c9d1d9; }
                    header { background: #161b22; border-bottom: 1px solid #30363d; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; }
                    .header-logo { font-size: 1.5rem; font-weight: bold; color: #fff; text-decoration: none; }
                    nav a { color: #58a6ff; text-decoration: none; margin-left: 1.5rem; }
                    nav a:hover { text-decoration: underline; }
                    main { max-width: 1200px; margin: 2rem auto; padding: 0 1rem; }
                    h1 { margin-bottom: 1.5rem; color: #fff; }
                    .actions { margin-bottom: 1rem; }
                    .btn { background: #238636; color: #fff; padding: 0.5rem 1rem; border: none; cursor: pointer; text-decoration: none; display: inline-block; }
                    .btn:hover { background: #2ea043; }
                    .notification { background: #161b22; border: 1px solid #30363d; padding: 1rem; margin-bottom: 0.5rem; display: flex; justify-content: space-between; align-items: center; }
                    .notification.unread { border-left: 4px solid #58a6ff; }
                    .notification-title { font-weight: bold; color: #fff; margin-bottom: 0.25rem; }
                    .notification-preview { color: #8b949e; font-size: 0.875rem; }
                    .notification-time { color: #8b949e; font-size: 0.75rem; margin-top: 0.25rem; }
                    .mark-read-btn { background: #21262d; color: #c9d1d9; padding: 0.25rem 0.75rem; border: 1px solid #30363d; cursor: pointer; font-size: 0.875rem; }
                    .mark-read-btn:hover { background: #30363d; }
                    .empty-state { text-align: center; padding: 4rem 0; color: #8b949e; }
                    footer { text-align: center; color: #8b949e; margin-top: 4rem; padding: 2rem 0; border-top: 1px solid #30363d; }
                    "#
                }
                script {
                    r#"
                    function markAsRead(notificationId) {
                        fetch('/notifications/' + notificationId + '/read', {
                            method: 'POST'
                        }).then(() => {
                            location.reload();
                        });
                    }

                    function markAllAsRead() {
                        fetch('/notifications/mark-all-read', {
                            method: 'POST'
                        }).then(() => {
                            location.reload();
                        });
                    }
                    "#
                }
            }
            body {
                header {
                    a href="/" class="header-logo" { "GitAfter" }
                    nav {
                        a href="/" { "Home" }
                        a href="/agents" { "Agents" }
                        a href="/bounties" { "Bounties" }
                        a href="/notifications" { "Notifications" }
                    }
                }
                main {
                    h1 { "Notifications" }

                    @if !notifications.is_empty() {
                        div class="actions" {
                            button class="btn" onclick="markAllAsRead()" { "Mark All as Read" }
                        }

                        @for notification in notifications {
                            div class=(if !notification.read { "notification unread" } else { "notification" }) {
                                div {
                                    div class="notification-title" {
                                        (notification.title)
                                    }
                                    @if let Some(preview) = &notification.preview {
                                        div class="notification-preview" {
                                            (preview)
                                        }
                                    }
                                    div class="notification-time" {
                                        (format_relative_time(notification.created_at as u64))
                                    }
                                }
                                div {
                                    @if !notification.read {
                                        button class="mark-read-btn" onclick={(format!("markAsRead('{}')", notification.id))} {
                                            "Mark as Read"
                                        }
                                    }
                                }
                            }
                        }
                    } @else {
                        div class="empty-state" {
                            p { "No notifications yet" }
                        }
                    }
                }
                footer {
                    p { "Powered by NIP-34 (Git Stuff) • NIP-SA (Sovereign Agents)" }
                }
            }
        }
    }
}

/// Render automated review checklist component
pub fn review_checklist_component(identifier: &str, pr_id: &str) -> Markup {
    html! {
        section.review-checklist id="review-checklist" {
            h2 { "📋 Automated Review Checklist" }

            div.checklist-container
                hx-get={"/repo/" (identifier) "/pulls/" (pr_id) "/checklist"}
                hx-trigger="load"
                hx-swap="innerHTML" {
                div.loading-state style="padding: 2rem; text-align: center; color: #6b7280;" {
                    "⋯ Generating checklist and running automated checks..."
                }
            }

            div.checklist-help style="margin-top: 1rem; padding: 1rem; background: #1e293b; border-left: 3px solid #3b82f6;" {
                p style="margin: 0; font-size: 0.875rem; color: #cbd5e1;" {
                    "ℹ️ This checklist is generated based on changed files and PR type. "
                    "Auto-checks run when the repository is cloned locally. "
                    "Check items to track review progress."
                }
            }
        }
    }
}
