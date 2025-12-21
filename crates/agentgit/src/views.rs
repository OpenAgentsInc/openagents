//! Maud view templates for AgentGit

use maud::{html, Markup, DOCTYPE};
use nostr::Event;

/// Helper function to extract tag value from event
fn get_tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event.tags.iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

/// Render a single repository card
fn repository_card(event: &Event) -> Markup {
    let name = get_tag_value(event, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(event, "description").unwrap_or_default();
    let identifier = get_tag_value(event, "d").unwrap_or_default();
    let clone_url = get_tag_value(event, "clone");
    let web_url = get_tag_value(event, "web");

    // Truncate pubkey for display
    let short_pubkey = if event.pubkey.len() > 16 {
        format!("{}...{}", &event.pubkey[..8], &event.pubkey[event.pubkey.len()-8..])
    } else {
        event.pubkey.clone()
    };

    html! {
        div.repo-card {
            div.repo-header {
                h3.repo-name { (name) }
                span.repo-id { "d:" (identifier) }
            }
            @if !description.is_empty() {
                p.repo-description { (description) }
            }
            div.repo-meta {
                span.repo-author { "by " (short_pubkey) }
                @if let Some(url) = clone_url {
                    a.repo-clone href=(url) { "Clone" }
                }
                @if let Some(url) = web_url {
                    a.repo-web href=(url) target="_blank" { "View" }
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
