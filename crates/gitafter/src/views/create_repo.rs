/// Repository creation form page
pub fn repository_create_form_page() -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Repository - GitAfter" }
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
                                h1.issue-detail-title { "Create New Repository" }
                                div.issue-detail-actions {
                                    a.back-link href="/" { "← Back to Repositories" }
                                }
                            }

                            form.claim-form
                                hx-post="/repo"
                                hx-target="this"
                                hx-swap="outerHTML" {

                                div.form-group {
                                    label for="repo_identifier" { "Repository Identifier" span.required { "*" } }
                                    input
                                        type="text"
                                        name="identifier"
                                        id="repo_identifier"
                                        placeholder="my-awesome-project"
                                        required {}
                                    p.hint { "Unique identifier for this repository (lowercase, hyphens allowed)" }
                                }

                                div.form-group {
                                    label for="repo_name" { "Name" span.required { "*" } }
                                    input
                                        type="text"
                                        name="name"
                                        id="repo_name"
                                        placeholder="My Awesome Project"
                                        required {}
                                }

                                div.form-group {
                                    label for="repo_description" { "Description" }
                                    textarea
                                        name="description"
                                        id="repo_description"
                                        placeholder="A brief description of this repository..."
                                        rows="4" {}
                                }

                                div.form-group {
                                    label for="repo_language" { "Primary Language" }
                                    select
                                        name="language"
                                        id="repo_language" {
                                        option value="" selected { "Select a language (optional)" }
                                        option value="rust" { "Rust" }
                                        option value="javascript" { "JavaScript" }
                                        option value="typescript" { "TypeScript" }
                                        option value="python" { "Python" }
                                        option value="go" { "Go" }
                                    }
                                }

                                div.form-group {
                                    label for="repo_topics" { "Topics" }
                                    input
                                        type="text"
                                        name="topics"
                                        id="repo_topics"
                                        placeholder="nostr, ai, tooling" {}
                                    p.hint { "Comma-separated topics to help discovery (optional)" }
                                }

                                div.form-group {
                                    label for="clone_url_git" { "Git Clone URL" span.required { "*" } }
                                    input
                                        type="url"
                                        name="clone_url_git"
                                        id="clone_url_git"
                                        placeholder="git@github.com:user/repo.git"
                                        required {}
                                    p.hint { "SSH clone URL (git protocol)" }
                                }

                                div.form-group {
                                    label for="clone_url_https" { "HTTPS Clone URL" }
                                    input
                                        type="url"
                                        name="clone_url_https"
                                        id="clone_url_https"
                                        placeholder="https://github.com/user/repo.git" {}
                                    p.hint { "Optional HTTPS clone URL" }
                                }

                                div.form-group {
                                    label for="web_url" { "Web URL" }
                                    input
                                        type="url"
                                        name="web_url"
                                        id="web_url"
                                        placeholder="https://github.com/user/repo" {}
                                    p.hint { "Optional web interface URL" }
                                }

                                div.form-group {
                                    label for="default_branch" { "Default Branch" }
                                    input
                                        type="text"
                                        name="default_branch"
                                        id="default_branch"
                                        placeholder="main"
                                        value="main" {}
                                }

                                div.form-group {
                                    label for="earliest_commit" { "Earliest Unique Commit" }
                                    input
                                        type="text"
                                        name="earliest_commit"
                                        id="earliest_commit"
                                        placeholder="abc123def456..." {}
                                    p.hint { "Optional: SHA of earliest commit unique to this repository (for fork tracking)" }
                                }

                                div.form-group {
                                    label for="maintainers" { "Additional Maintainers" }
                                    textarea
                                        name="maintainers"
                                        id="maintainers"
                                        placeholder="npub1...\nnpub2..."
                                        rows="3" {}
                                    p.hint { "One Nostr pubkey (npub) per line. You will be added as owner automatically." }
                                }

                                div style="padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b; margin-bottom: 1rem;" {
                                    p style="margin: 0; font-size: 0.875rem;" {
                                        "⚠️ " strong { "Note: " }
                                        "Event publishing requires identity integration (issue #342). This form will create the event template but cannot publish yet."
                                    }
                                }

                                button.submit-button type="submit" { "Create Repository Announcement" }
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

