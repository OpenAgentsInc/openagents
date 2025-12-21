//! HTML layout templates using Maud

use maud::{html, Markup, DOCTYPE};

/// Base page layout with navigation
pub fn page(title: &str, content: Markup) -> String {
    page_with_current(title, content, None)
}

/// Base page layout with current page highlighting
pub fn page_with_current(title: &str, content: Markup, current_page: Option<&str>) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                style {
                    r#"
                    * {
                        margin: 0;
                        padding: 0;
                        box-sizing: border-box;
                    }
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                        background: #1a1a1a;
                        color: #e0e0e0;
                        line-height: 1.6;
                    }
                    .container {
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 2rem;
                    }
                    nav {
                        background: #2a2a2a;
                        padding: 1rem 2rem;
                        border-bottom: 1px solid #3a3a3a;
                    }
                    nav h1 {
                        color: #4a9eff;
                        font-size: 1.5rem;
                    }
                    .card {
                        background: #2a2a2a;
                        border: 1px solid #3a3a3a;
                        padding: 1.5rem;
                        margin-bottom: 1rem;
                    }
                    .card h2 {
                        color: #4a9eff;
                        margin-bottom: 1rem;
                    }
                    .status {
                        display: inline-block;
                        padding: 0.25rem 0.75rem;
                        background: #2d5016;
                        color: #7dff7d;
                        font-size: 0.875rem;
                    }
                    .nav-links {
                        display: flex;
                        gap: 1.5rem;
                        align-items: center;
                    }
                    .nav-links a {
                        color: #a0a0a0;
                        text-decoration: none;
                        padding: 0.5rem 1rem;
                        transition: color 0.2s;
                        font-size: 0.95rem;
                    }
                    .nav-links a:hover {
                        color: #4a9eff;
                    }
                    .nav-links a.active {
                        color: #4a9eff;
                        border-bottom: 2px solid #4a9eff;
                    }
                    @media (max-width: 768px) {
                        .nav-links {
                            flex-direction: column;
                            gap: 0.5rem;
                            align-items: flex-start;
                        }
                        nav > div {
                            flex-direction: column !important;
                            gap: 1rem;
                        }
                    }
                    "#
                }
            }
            body {
                nav {
                    div style="display: flex; justify-content: space-between; align-items: center;" {
                        h1 { "🤖 Autopilot GUI" }
                        div class="nav-links" {
                            a href="/" class={ @if current_page == Some("dashboard") { "active" } } { "Dashboard" }
                            a href="/chat" class={ @if current_page == Some("chat") { "active" } } { "Chat" }
                            a href="/context" class={ @if current_page == Some("context") { "active" } } { "Context" }
                            a href="/permissions" class={ @if current_page == Some("permissions") { "active" } } { "Permissions" }
                        }
                    }
                }
                (content)
            }
        }
    };

    markup.into_string()
}

/// Dashboard view
pub fn dashboard() -> Markup {
    html! {
        div class="container" {
            div class="card" {
                h2 { "Dashboard" }
                p { "Welcome to Autopilot GUI - Visual interface for OpenAgents autonomous agent." }
                p style="margin-top: 1rem;" {
                    span class="status" { "Ready" }
                }
            }

            div class="card" {
                h2 { "Quick Start" }
                ul style="list-style-position: inside; padding-left: 1rem;" {
                    li { "Sessions - View and manage autopilot sessions (coming soon)" }
                    li { "Permissions - Configure tool permissions (coming soon)" }
                    li { "Context - Inspect agent context (coming soon)" }
                }
            }

            div class="card" {
                h2 { "System Status" }
                p { "Server: " span style="color: #7dff7d;" { "Running" } }
                p { "Port: 3847" }
                p { "Version: 0.1.0" }
            }
        }
    }
}
