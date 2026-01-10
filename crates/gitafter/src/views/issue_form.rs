/// Issue creation form page
pub fn issue_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "New Issue - " (repo_name) " - GitAfter" }
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
                        div.issue-form-container {
                            div.issue-form-header {
                                div {
                                    h1.issue-form-title { "Create New Issue" }
                                    p.issue-form-subtitle { "Repository: " (repo_name) }
                                }
                                div.issue-form-actions {
                                    a.back-link href={"/repo/" (identifier) "/issues"} { "← Back to Issues" }
                                }
                            }

                            form.issue-form method="post" action={"/repo/" (identifier) "/issues"} {
                                div.form-group {
                                    label.form-label for="title" { "Title" }
                                    input class="form-input" id="title" type="text" name="title" required placeholder="Brief description of the issue";
                                }

                                div.form-group {
                                    label.form-label for="description" { "Description" }
                                    textarea class="form-textarea" id="description" name="description" rows="10" placeholder="Detailed description of the issue (optional)";
                                }

                                div.form-group {
                                    label.form-label for="labels" { "Labels" }
                                    input class="form-input" id="labels" type="text" name="labels" placeholder="Comma-separated labels (optional, e.g., bug, enhancement)";
                                    p.form-help { "Labels help categorize and filter issues" }
                                }

                                div.form-actions {
                                    button.btn-primary type="submit" { "Create Issue" }
                                    a.btn-secondary href={"/repo/" (identifier) "/issues"} { "Cancel" }
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

