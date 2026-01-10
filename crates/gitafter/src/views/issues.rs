/// Issues list page for a repository
pub fn issues_list_page(
    repository: &Event,
    issues: &[Event],
    is_watched: bool,
    identifier: &str,
    filter_open: bool,
    filter_closed: bool,
    filter_has_bounty: bool,
    filter_claimed: bool,
    issue_first_claims: &std::collections::HashMap<String, Event>,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Issues - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("../styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
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
                        div.issues-container {
                            div.issues-header {
                                div {
                                    h1.issues-title { (repo_name) " - Issues" }
                                    p.issues-subtitle { "Viewing issues for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    @if is_watched {
                                        form
                                            hx-post={"/repo/" (identifier) "/unwatch"}
                                            hx-target="this"
                                            hx-swap="outerHTML"
                                            style="display: inline;" {
                                            button.watch-button type="submit" { "⭐ Unwatch" }
                                        }
                                    } @else {
                                        form
                                            hx-post={"/repo/" (identifier) "/watch"}
                                            hx-target="this"
                                            hx-swap="outerHTML"
                                            style="display: inline;" {
                                            button.watch-button type="submit" { "☆ Watch" }
                                        }
                                    }
                                    a.nav-link href={"/repo/" (identifier) "/issues/new"} { "+ New Issue" }
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            // Filter controls
                            div.filter-controls style="margin: 1.5rem 0; padding: 1rem; background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333);" {
                                form
                                    hx-get={"/repo/" (identifier) "/issues"}
                                    hx-target=".issues-container"
                                    hx-swap="outerHTML"
                                    style="display: flex; gap: 1rem; align-items: center; flex-wrap: wrap;" {

                                    span style="font-weight: 600;" { "Filter:" }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_open {
                                            input type="checkbox" name="filter_open" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_open" value="true";
                                        }
                                        span { "Open" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_closed {
                                            input type="checkbox" name="filter_closed" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_closed" value="true";
                                        }
                                        span { "Closed" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_has_bounty {
                                            input type="checkbox" name="filter_has_bounty" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_has_bounty" value="true";
                                        }
                                        span { "Has Bounty" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_claimed {
                                            input type="checkbox" name="filter_claimed" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_claimed" value="true";
                                        }
                                        span { "Claimed" }
                                    }

                                    button.btn-secondary type="submit" style="margin-left: auto; padding: 0.5rem 1rem;" { "Apply Filters" }
                                }
                            }

                            @if issues.is_empty() {
                                div.empty-state {
                                    p { "No issues found for this repository." }
                                    p.info-text { "Issues will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.issues-count {
                                    span { (issues.len()) " issue" @if issues.len() != 1 { "s" } " found" }
                                }

                                div #issues-list .issues-list {
                                    @for issue in issues {
                                        @let issue_title = get_tag_value(issue, "subject")
                                            .unwrap_or_else(|| "Untitled Issue".to_string());
                                        @let issue_status = get_tag_value(issue, "status")
                                            .unwrap_or_else(|| "open".to_string());
                                        @let issue_author = if issue.pubkey.len() > 16 {
                                            format!("{}...{}", &issue.pubkey[..8], &issue.pubkey[issue.pubkey.len()-8..])
                                        } else {
                                            issue.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/issues/" (issue.id)} {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (issue_title) }
                                                    span class={"issue-status " (issue_status)} {
                                                        (issue_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (issue_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { (format_relative_time(issue.created_at)) }

                                                    @if let Some(claim) = issue_first_claims.get(&issue.id) {
                                                        @let claimer = if claim.pubkey.len() > 16 {
                                                            format!("{}...{}", &claim.pubkey[..8], &claim.pubkey[claim.pubkey.len()-8..])
                                                        } else {
                                                            claim.pubkey.clone()
                                                        };

                                                        @let estimate = claim.tags.iter()
                                                            .find(|tag| tag.first().map(|t| t == "estimate").unwrap_or(false))
                                                            .and_then(|tag| tag.get(1));

                                                        span.issue-separator { "•" }
                                                        span.claim-badge style="color: #fbbf24; font-weight: 600;" {
                                                            "🏆 Claimed by " (claimer)
                                                            @if let Some(est) = estimate {
                                                                " - " (est) "h"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            @if !issue.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if issue.content.len() > 200 {
                                                        format!("{}...", &issue.content[..200])
                                                    } else {
                                                        issue.content.clone()
                                                    };
                                                    p { (preview) }
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
                script {
                    (PreEscaped(r#"
                    // Toast notification system
                    function showToast(message, type = 'info') {
                        const toast = document.createElement('div');
                        toast.className = 'toast toast-' + type + ' item-inserted';
                        toast.textContent = message;
                        toast.style.cssText = 'position: fixed; top: 20px; right: 20px; padding: 1rem 1.5rem; background: var(--bg-tertiary); border: 2px solid var(--accent); color: var(--text-primary); z-index: 10000; font-weight: 600; box-shadow: 0 4px 12px rgba(0,0,0,0.5);';
                        if (type === 'success') {
                            toast.style.borderColor = '#00ff88';
                            toast.innerHTML = '<span style="color: #00ff88;">✓</span> ' + message;
                        } else if (type === 'bounty') {
                            toast.style.borderColor = '#ffaa00';
                            toast.innerHTML = '<span style="color: #ffaa00;">⚡</span> ' + message;
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates for issues
                    document.body.addEventListener('htmx:wsAfterMessage', function(evt) {
                        const message = evt.detail.message;
                        if (!message) return;

                        const parser = new DOMParser();
                        const doc = parser.parseFromString(message, 'text/html');
                        const eventDiv = doc.querySelector('.event');
                        if (!eventDiv) return;

                        const kind = parseInt(eventDiv.dataset.kind);
                        const jsonStr = eventDiv.textContent;
                        let event;
                        try {
                            event = JSON.parse(jsonStr);
                        } catch (e) {
                            console.error('Failed to parse event JSON:', e);
                            return;
                        }

                        // Handle issue events (kind:1621)
                        if (kind === 1621) {
                            console.log('New issue created:', event.id);
                            showToast('New issue created!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle bounty offer events (kind:1636)
                        if (kind === 1636) {
                            console.log('New bounty offer:', event.id);
                            showToast('New bounty offered!', 'bounty');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle issue claim events (kind:1634)
                        if (kind === 1634) {
                            console.log('Issue claimed:', event.id);
                            showToast('Issue claimed!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}
