/// Repository detail page
pub fn repository_detail_page(
    repository: &Event,
    is_cloned: bool,
    local_path: Option<String>,
    repo_state: Option<&Event>,
) -> Markup {
    let name =
        get_tag_value(repository, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(repository, "description").unwrap_or_default();
    let identifier = get_tag_value(repository, "d").unwrap_or_default();
    let clone_urls = get_all_tag_values(repository, "clone");
    let web_url = get_tag_value(repository, "web");
    let maintainers = get_all_tag_values(repository, "p");

    // Format pubkey for display
    let owner_pubkey = if repository.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &repository.pubkey[..8],
            &repository.pubkey[repository.pubkey.len() - 8..]
        )
    } else {
        repository.pubkey.clone()
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (name) " - GitAfter" }
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
                                h2 { "Navigation" }
                                div.repo-nav-links {
                                    a.nav-link href={"/repo/" (identifier) "/issues"} { "View Issues" }
                                    a.nav-link href={"/repo/" (identifier) "/patches"} { "View Patches" }
                                    a.nav-link href={"/repo/" (identifier) "/pulls"} { "View Pull Requests" }
                                }
                            }

                            section.repo-section {
                                h2 { "Contribute" }
                                div.repo-nav-links {
                                    a.nav-link href={"/repo/" (identifier) "/pulls/new"} { "+ Create Pull Request" }
                                    a.nav-link href={"/repo/" (identifier) "/patches/new"} { "+ Create Patch" }
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
                                                // Display robot icon for potential agent maintainers
                                                // (agents typically have npub addresses like regular users,
                                                // but this provides a visual hint for agent-capable repos)
                                                span style="margin-right: 0.5rem;" { "🤖" }
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
                                        @for url in &clone_urls {
                                            @let onclick_code = format!("navigator.clipboard.writeText('{}')", url);
                                            div.clone-url-item {
                                                code { (url) }
                                                button.copy-btn onclick=(onclick_code) { "Copy" }
                                            }
                                        }
                                    }
                                }
                            }

                            @if let Some(state) = repo_state {
                                section.repo-section {
                                    h2 { "Repository State" }
                                    @let branches = get_all_tag_values(state, "refs/heads");
                                    @let tags = get_all_tag_values(state, "refs/tags");
                                    @let head = get_tag_value(state, "HEAD");

                                    @if let Some(ref h) = head {
                                        div.state-item style="margin-bottom: 1rem;" {
                                            span.label style="font-weight: 600; margin-right: 0.5rem;" { "HEAD:" }
                                            code { (h) }
                                        }
                                    }

                                    @if !branches.is_empty() {
                                        div.state-item style="margin-bottom: 1rem;" {
                                            h3 style="font-size: 1rem; margin-bottom: 0.5rem;" { "Branches (" (branches.len()) ")" }
                                            div.branch-list style="display: flex; flex-direction: column; gap: 0.25rem;" {
                                                @for branch in &branches {
                                                    div style="padding: 0.25rem 0; font-family: monospace; font-size: 0.875rem;" { "• " (branch) }
                                                }
                                            }
                                        }
                                    }

                                    @if !tags.is_empty() {
                                        div.state-item {
                                            h3 style="font-size: 1rem; margin-bottom: 0.5rem;" { "Tags (" (tags.len()) ")" }
                                            div.tag-list style="display: flex; flex-direction: column; gap: 0.25rem;" {
                                                @for tag in &tags {
                                                    div style="padding: 0.25rem 0; font-family: monospace; font-size: 0.875rem;" { "• " (tag) }
                                                }
                                            }
                                        }
                                    }

                                    @if branches.is_empty() && tags.is_empty() && head.is_none() {
                                        p.empty-state { "No repository state information available" }
                                    }
                                }
                            }

                            section.repo-section {
                                h2 { "Local Clone" }
                                @if is_cloned {
                                    @if let Some(path) = local_path {
                                        div.clone-status {
                                            p { "✅ Repository cloned locally" }
                                            div.clone-path {
                                                code { (path) }
                                            }
                                        }
                                    }
                                } @else {
                                    @if !clone_urls.is_empty() {
                                        div.clone-action {
                                            p { "Clone this repository to your local workspace for development" }
                                            form
                                                hx-post={"/repo/" (identifier) "/clone"}
                                                hx-target="#clone-result"
                                            {
                                                button type="submit" { "Clone Repository" }
                                            }
                                            div id="clone-result" {}
                                        }
                                    } @else {
                                        p.placeholder { "No clone URLs available for this repository" }
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
                                        span title={(repository.created_at)} { (format_relative_time(repository.created_at)) }
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

