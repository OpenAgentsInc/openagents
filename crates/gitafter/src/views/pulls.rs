/// Pull requests list page for a repository
pub fn pull_requests_list_page(
    repository: &Event,
    pull_requests: &[Event],
    identifier: &str,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Pull Requests - GitAfter" }
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
                                    h1.issues-title { (repo_name) " - Pull Requests" }
                                    p.issues-subtitle { "Viewing pull requests for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            @if pull_requests.is_empty() {
                                div.empty-state {
                                    p { "No pull requests found for this repository." }
                                    p.info-text { "Pull requests will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.pr-filters {
                                    button.filter-btn.active data-filter="all" { "All" }
                                    button.filter-btn data-filter="open" { "Open" }
                                    button.filter-btn data-filter="merged" { "Merged" }
                                    button.filter-btn data-filter="closed" { "Closed" }
                                    button.filter-btn data-filter="draft" { "Draft" }
                                }

                                div.issues-count {
                                    span { (pull_requests.len()) " pull request" @if pull_requests.len() != 1 { "s" } " found" }
                                }

                                div #pr-list .issues-list {
                                    @for pr in pull_requests {
                                        @let pr_title = get_tag_value(pr, "subject")
                                            .unwrap_or_else(|| "Untitled Pull Request".to_string());
                                        @let pr_status = get_tag_value(pr, "status")
                                            .unwrap_or_else(|| "open".to_string());
                                        @let pr_author = if pr.pubkey.len() > 16 {
                                            format!("{}...{}", &pr.pubkey[..8], &pr.pubkey[pr.pubkey.len()-8..])
                                        } else {
                                            pr.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/pulls/" (pr.id)} data-status=(pr_status) {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (pr_title) }
                                                    @let badge_class = match pr_status.as_str() {
                                                        "merged" => "status-badge status-merged",
                                                        "closed" => "status-badge status-closed",
                                                        "draft" => "status-badge status-draft",
                                                        _ => "status-badge status-open",
                                                    };
                                                    @let badge_icon = match pr_status.as_str() {
                                                        "merged" => "✓",
                                                        "closed" => "✗",
                                                        "draft" => "📝",
                                                        _ => "●",
                                                    };
                                                    span class=(badge_class) {
                                                        (badge_icon) " " (pr_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (pr_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { (format_relative_time(pr.created_at)) }
                                                }
                                            }
                                            @if !pr.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if pr.content.len() > 200 {
                                                        format!("{}...", &pr.content[..200])
                                                    } else {
                                                        pr.content.clone()
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
                        } else if (type === 'info') {
                            toast.style.borderColor = '#00bfff';
                            toast.innerHTML = '<span style="color: #00bfff;">ℹ</span> ' + message;
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates for pull requests
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

                        // Handle PR events (kind:1618)
                        if (kind === 1618) {
                            console.log('New PR created:', event.id);
                            showToast('New pull request!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle PR update events (kind:1619)
                        if (kind === 1619) {
                            console.log('PR updated:', event.id);
                            showToast('PR updated!', 'info');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle status events (kind:1630-1633)
                        if (kind >= 1630 && kind <= 1633) {
                            console.log('PR status changed:', event.id);
                            const statusMap = {1630: 'opened', 1631: 'merged', 1632: 'closed', 1633: 'marked as draft'};
                            showToast('PR ' + (statusMap[kind] || 'updated') + '!', kind === 1631 ? 'success' : 'info');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}

