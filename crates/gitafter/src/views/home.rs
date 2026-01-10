/// Home page with repository list
pub fn home_page_with_repos(
    repositories: &[Event],
    selected_language: &Option<String>,
    selected_topic: &Option<String>,
    has_bounties_filter: bool,
    agent_friendly_filter: bool,
    bounty_counts: &std::collections::HashMap<String, usize>,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "GitAfter - Nostr GitHub Alternative" }
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
                        a href="/" class="active" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" {
                            h2 { "Repositories (" (repositories.len()) ")" }
                            a.submit-button href="/repo/new" style="text-decoration: none; padding: 10px 20px;" { "+ Create Repository" }
                        }

                        // Filter controls
                        form method="get" action="/" style="display: flex; gap: 1rem; margin-bottom: 1rem; padding: 1rem; background: #2a2a2a;" {
                            div {
                                label for="language" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Language" }
                                select name="language" id="language" style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;" {
                                    option value="" selected[selected_language.is_none()] { "All" }
                                    option value="rust" selected[selected_language.as_deref() == Some("rust")] { "Rust" }
                                    option value="javascript" selected[selected_language.as_deref() == Some("javascript")] { "JavaScript" }
                                    option value="typescript" selected[selected_language.as_deref() == Some("typescript")] { "TypeScript" }
                                    option value="python" selected[selected_language.as_deref() == Some("python")] { "Python" }
                                    option value="go" selected[selected_language.as_deref() == Some("go")] { "Go" }
                                }
                            }

                            div {
                                label for="topic" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Topic" }
                                input
                                    type="text"
                                    name="topic"
                                    id="topic"
                                    placeholder="nostr, ai"
                                    value={(selected_topic.clone().unwrap_or_default())}
                                    style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;" {}
                            }

                            div {
                                label style="display: block; margin-bottom: 0.5rem; color: #aaa;" {
                                    input type="checkbox" name="has_bounties" value="true" checked[has_bounties_filter];
                                    " Has Bounties"
                                }
                            }

                            div {
                                label style="display: block; margin-bottom: 0.5rem; color: #aaa;" {
                                    input type="checkbox" name="agent_friendly" value="true" checked[agent_friendly_filter];
                                    " Agent-Friendly"
                                }
                            }

                            button type="submit" style="padding: 0.5rem 1rem; background: #4a9eff; color: #fff; border: none; cursor: pointer;" { "Apply Filters" }
                            a href="/" style="padding: 0.5rem 1rem; background: #444; color: #fff; text-decoration: none; display: inline-block;" { "Clear" }
                        }

                        @if repositories.is_empty() {
                            p.placeholder { "No repositories found. Listening for NIP-34 events..." }
                        } @else {
                            div #repo-list .repo-list {
                                @for repo in repositories {
                                    (repository_card_with_bounty_count(repo, bounty_counts))
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
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates
                    document.body.addEventListener('htmx:wsAfterMessage', function(evt) {
                        const message = evt.detail.message;
                        if (!message) return;

                        // Extract event from message
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

                        // Handle repository events (kind:30617)
                        if (kind === 30617) {
                            console.log('New repository announced:', event.id);
                            showToast('New repository added!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}

