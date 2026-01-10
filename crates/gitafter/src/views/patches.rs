/// Patches list page for a repository
pub fn patches_list_page(repository: &Event, patches: &[Event], identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Patches - GitAfter" }
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
                                    h1.issues-title { (repo_name) " - Patches" }
                                    p.issues-subtitle { "Viewing patches for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            @if patches.is_empty() {
                                div.empty-state {
                                    p { "No patches found for this repository." }
                                    p.info-text { "Patches will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.issues-count {
                                    span { (patches.len()) " patch" @if patches.len() != 1 { "es" } " found" }
                                }

                                div.issues-list {
                                    @for patch in patches {
                                        @let patch_title = get_tag_value(patch, "subject")
                                            .unwrap_or_else(|| "Untitled Patch".to_string());
                                        @let patch_author = if patch.pubkey.len() > 16 {
                                            format!("{}...{}", &patch.pubkey[..8], &patch.pubkey[patch.pubkey.len()-8..])
                                        } else {
                                            patch.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/patches/" (patch.id)} {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (patch_title) }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (patch_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { (format_relative_time(patch.created_at)) }
                                                }
                                            }
                                            @if !patch.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if patch.content.len() > 200 {
                                                        format!("{}...", &patch.content[..200])
                                                    } else {
                                                        patch.content.clone()
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
            }
        }
    }
}

