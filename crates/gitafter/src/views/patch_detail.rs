/// Patch detail page
pub fn patch_detail_page(
    repository: &Event,
    patch: &Event,
    _reviews: &[Event],
    _reviewer_reputations: &std::collections::HashMap<String, i32>,
    identifier: &str,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let patch_title =
        get_tag_value(patch, "subject").unwrap_or_else(|| "Untitled Patch".to_string());

    // Format pubkey for display
    let patch_author = if patch.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &patch.pubkey[..8],
            &patch.pubkey[patch.pubkey.len() - 8..]
        )
    } else {
        patch.pubkey.clone()
    };

    // Extract commit ID and clone URL
    let commit_id = get_tag_value(patch, "c");
    let clone_url = get_tag_value(patch, "clone");

    // Extract all tags for display
    let all_tags = &patch.tags;

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (patch_title) " - " (repo_name) " - GitAfter" }
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
                        div.issue-detail {
                            div.issue-detail-header {
                                div {
                                    h1.issue-detail-title { (patch_title) }
                                    div.issue-detail-meta {
                                        span.issue-author { "by " (patch_author) }
                                        span.issue-separator { "•" }
                                        span.issue-time { (format_relative_time(patch.created_at)) }
                                    }
                                }
                                div.issue-detail-actions {
                                    a.back-link href={"/repo/" (identifier) "/patches"} { "← Back to Patches" }
                                }
                            }

                            section.issue-section {
                                h2 { "Repository Context" }
                                div.repo-context {
                                    a.repo-link href={"/repo/" (identifier)} { (repo_name) }
                                    span.repo-id-label { " (" (identifier) ")" }
                                }
                            }

                            @if !patch.content.is_empty() {
                                section.issue-section {
                                    h2 { "Patch Content" }
                                    pre style="background: #0d1117; color: #c9d1d9; padding: 1rem; overflow-x: auto; border: 1px solid var(--border-color, #333); font-size: 0.875rem; line-height: 1.5;" {
                                        code { (patch.content) }
                                    }
                                }
                            }

                            @if let Some(cid) = commit_id {
                                section.issue-section {
                                    h2 { "Commit Information" }
                                    div.event-details {
                                        div.event-detail-item {
                                            span.label { "Commit ID:" }
                                            code { (cid) }
                                        }
                                    }
                                }
                            }

                            @if let Some(curl) = clone_url {
                                section.issue-section {
                                    h2 { "Clone URL" }
                                    div.event-details {
                                        div.event-detail-item {
                                            code { (curl) }
                                        }
                                    }
                                }
                            }

                            @if !all_tags.is_empty() {
                                section.issue-section {
                                    h2 { "Tags" }
                                    div.tag-list {
                                        @for tag in all_tags {
                                            @if tag.len() >= 2 {
                                                @let tag_name = &tag[0];
                                                @let tag_value = &tag[1];
                                                @if !tag_name.is_empty() && !tag_value.is_empty() {
                                                    div.tag-item {
                                                        span.tag-name { (tag_name) }
                                                        span.tag-value { (tag_value) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "Event Details" }
                                div.event-details {
                                    div.event-detail-item {
                                        span.label { "Event ID:" }
                                        code { (patch.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (patch.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Pubkey:" }
                                        code { (patch.pubkey) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Signature:" }
                                        code.signature { (patch.sig) }
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
                script {
                    (PreEscaped(r#"
                    // PR filter functionality
                    document.querySelectorAll('.filter-btn').forEach(btn => {
                        btn.addEventListener('click', function() {
                            document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
                            this.classList.add('active');

                            const filter = this.dataset.filter;
                            const cards = document.querySelectorAll('.issue-card');

                            cards.forEach(card => {
                                if (filter === 'all' || card.dataset.status === filter) {
                                    card.style.display = '';
                                } else {
                                    card.style.display = 'none';
                                }
                            });
                        });
                    });
                    "#))
                }
    }
}

