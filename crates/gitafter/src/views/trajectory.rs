/// Trajectory viewer page
pub fn trajectory_viewer_page(session: &Event, events: &[Event]) -> Markup {
    let agent_pubkey = if session.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &session.pubkey[..8],
            &session.pubkey[session.pubkey.len() - 8..]
        )
    } else {
        session.pubkey.clone()
    };

    // Extract session metadata
    let session_title =
        get_tag_value(session, "title").unwrap_or_else(|| "Untitled Session".to_string());
    let task_description = get_tag_value(session, "task");

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Trajectory: " (session_title) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("../styles.css"))
                    "
                    .trajectory-timeline {
                        position: relative;
                        padding-left: 2rem;
                    }
                    .trajectory-timeline::before {
                        content: '';
                        position: absolute;
                        left: 0.5rem;
                        top: 0;
                        bottom: 0;
                        width: 2px;
                        background: var(--border-color, #333);
                    }
                    .trajectory-event {
                        position: relative;
                        margin-bottom: 1.5rem;
                        padding: 1rem;
                        background: var(--card-bg, #1a1a1a);
                        border: 1px solid var(--border-color, #333);
                    }
                    .trajectory-event::before {
                        content: '';
                        position: absolute;
                        left: -1.5rem;
                        top: 1.5rem;
                        width: 10px;
                        height: 10px;
                        background: var(--accent-color, #0ea5e9);
                    }
                    .trajectory-event-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 0.5rem;
                    }
                    .event-type {
                        font-weight: 600;
                        color: var(--accent-color, #0ea5e9);
                    }
                    .event-timestamp {
                        font-size: 0.85rem;
                        color: var(--text-secondary, #888);
                    }
                    .event-content {
                        margin-top: 0.5rem;
                        padding: 0.5rem;
                        background: var(--bg-color, #0a0a0a);
                        font-family: monospace;
                        font-size: 0.9rem;
                        white-space: pre-wrap;
                        overflow-x: auto;
                    }
                    .trajectory-summary {
                        background: var(--card-bg, #1a1a1a);
                        padding: 1.5rem;
                        margin-bottom: 2rem;
                        border: 1px solid var(--border-color, #333);
                    }
                    "
                }
            }
            body {
                header {
                    h1 { "⚡ GitAfter" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.trajectory-viewer {
                            h1 { "🔍 Trajectory Viewer" }

                            div.trajectory-summary {
                                h2 { (session_title) }
                                div.session-meta {
                                    p { "Agent: " span.agent-pubkey { (agent_pubkey) } }
                                    p { "Session ID: " code { (session.id) } }
                                    p { "Started: " span title={(session.created_at)} { (format_relative_time(session.created_at)) } }
                                    @if let Some(task) = task_description {
                                        p { "Task: " (task) }
                                    }
                                }
                                @if !session.content.is_empty() {
                                    div.session-description {
                                        h3 { "Description" }
                                        p { (session.content) }
                                    }
                                }
                            }

                            @if events.is_empty() {
                                p.empty-state { "No trajectory events found for this session." }
                            } @else {
                                h2 { "Event Timeline (" (events.len()) " events)" }

                                div.trajectory-timeline {
                                    @for event in events {
                                        @let event_type = get_tag_value(event, "type").unwrap_or_else(|| "unknown".to_string());
                                        @let tool_name = get_tag_value(event, "tool");

                                        div.trajectory-event {
                                            div.trajectory-event-header {
                                                span.event-type {
                                                    @if let Some(tool) = &tool_name {
                                                        "🔧 " (tool)
                                                    } @else {
                                                        "📝 " (event_type)
                                                    }
                                                }
                                                span.event-timestamp title={(event.created_at)} { (format_relative_time(event.created_at)) }
                                            }

                                            @if !event.content.is_empty() {
                                                div.event-content {
                                                    (event.content)
                                                }
                                            }

                                            details {
                                                summary { "View raw event data" }
                                                div.event-content {
                                                    "Event ID: " (event.id) "\n"
                                                    "Kind: " (event.kind) "\n"
                                                    "Pubkey: " (event.pubkey) "\n"
                                                    "Tags:\n"
                                                    @for tag in &event.tags {
                                                        "  [" (tag.join(", ")) "]\n"
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
                footer {
                    p { "Powered by NIP-34 (Git Stuff) • NIP-SA (Sovereign Agents) • NIP-57 (Zaps)" }
                }
            }
        }
    }
}

