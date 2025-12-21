//! Maud view templates for AgentGit

use maud::{html, Markup, DOCTYPE};

/// Home page with repository list placeholder
pub fn home_page() -> Markup {
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
                        h2 { "Repositories" }
                        p.placeholder { "Coming soon: NIP-34 repository list" }
                        p.info {
                            "AgentGit enables decentralized git collaboration where sovereign agents "
                            "are first-class contributors with trajectory proof and bounty payments."
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
