//! Maud view templates for AgentGit

use maud::{html, Markup, DOCTYPE};
use nostr::Event;

/// Helper function to extract tag value from event
fn get_tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event.tags.iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

/// Helper function to extract all values for a tag name
fn get_all_tag_values(event: &Event, tag_name: &str) -> Vec<String> {
    event.tags.iter()
        .filter(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

/// Render a single repository card
fn repository_card(event: &Event) -> Markup {
    let name = get_tag_value(event, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(event, "description").unwrap_or_default();
    let identifier = get_tag_value(event, "d").unwrap_or_default();
    let has_clone_url = get_tag_value(event, "clone").is_some();
    let has_web_url = get_tag_value(event, "web").is_some();

    // Truncate pubkey for display
    let short_pubkey = if event.pubkey.len() > 16 {
        format!("{}...{}", &event.pubkey[..8], &event.pubkey[event.pubkey.len()-8..])
    } else {
        event.pubkey.clone()
    };

    html! {
        a.repo-card href={"/repo/" (identifier)} {
            div.repo-header {
                h3.repo-name { (name) }
                span.repo-id { "d:" (identifier) }
            }
            @if !description.is_empty() {
                p.repo-description { (description) }
            }
            div.repo-meta {
                span.repo-author { "by " (short_pubkey) }
                @if has_clone_url {
                    span.repo-clone { "Clone" }
                }
                @if has_web_url {
                    span.repo-web { "View" }
                }
            }
        }
    }
}

/// Home page with repository list
pub fn home_page_with_repos(repositories: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "AgentGit - Nostr GitHub Alternative" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" class="active" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        h2 { "Repositories (" (repositories.len()) ")" }
                        @if repositories.is_empty() {
                            p.placeholder { "No repositories found. Listening for NIP-34 events..." }
                        } @else {
                            div.repo-list {
                                @for repo in repositories {
                                    (repository_card(repo))
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

/// Repository detail page
pub fn repository_detail_page(repository: &Event) -> Markup {
    let name = get_tag_value(repository, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(repository, "description").unwrap_or_default();
    let identifier = get_tag_value(repository, "d").unwrap_or_default();
    let clone_urls = get_all_tag_values(repository, "clone");
    let web_url = get_tag_value(repository, "web");
    let maintainers = get_all_tag_values(repository, "p");

    // Format pubkey for display
    let owner_pubkey = if repository.pubkey.len() > 16 {
        format!("{}...{}", &repository.pubkey[..8], &repository.pubkey[repository.pubkey.len()-8..])
    } else {
        repository.pubkey.clone()
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.repo-detail {
                            div.repo-detail-header {
                                div {
                                    h1.repo-detail-name { (name) }
                                    p.repo-detail-identifier { "Identifier: " (identifier) }
                                }
                                a.back-link href="/" { "← Back to Repositories" }
                            }

                            @if !description.is_empty() {
                                section.repo-section {
                                    h2 { "Description" }
                                    p.repo-detail-description { (description) }
                                }
                            }

                            section.repo-section {
                                h2 { "Owner" }
                                div.repo-owner {
                                    span.pubkey { (owner_pubkey) }
                                }
                            }

                            @if !maintainers.is_empty() {
                                section.repo-section {
                                    h2 { "Maintainers" }
                                    div.maintainer-list {
                                        @for maintainer in maintainers {
                                            @let short_maintainer = if maintainer.len() > 16 {
                                                format!("{}...{}", &maintainer[..8], &maintainer[maintainer.len()-8..])
                                            } else {
                                                maintainer.clone()
                                            };
                                            div.maintainer-item {
                                                span.pubkey { (short_maintainer) }
                                            }
                                        }
                                    }
                                }
                            }

                            @if !clone_urls.is_empty() {
                                section.repo-section {
                                    h2 { "Clone URLs" }
                                    div.clone-urls {
                                        @for url in clone_urls {
                                            @let onclick_code = format!("navigator.clipboard.writeText('{}')", url);
                                            div.clone-url-item {
                                                code { (url) }
                                                button.copy-btn onclick=(onclick_code) { "Copy" }
                                            }
                                        }
                                    }
                                }
                            }

                            @if let Some(url) = web_url {
                                section.repo-section {
                                    h2 { "Web Interface" }
                                    a.web-link href=(url) target="_blank" { (url) }
                                }
                            }

                            section.repo-section {
                                h2 { "Event Details" }
                                div.event-details {
                                    div.event-detail-item {
                                        span.label { "Event ID:" }
                                        code { (repository.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (repository.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Created:" }
                                        span { (repository.created_at) }
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
