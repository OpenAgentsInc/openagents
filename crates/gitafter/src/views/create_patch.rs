/// Create patch form page
pub fn patch_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Patch - " (repo_name) " - GitAfter" }
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
                                    h1.issue-form-title { "Create Patch" }
                                    p.issue-form-subtitle { "Repository: " (repo_name) }
                                }
                                div.issue-form-actions {
                                    a.back-link href={"/repo/" (identifier) "/patches"} { "← Back to Patches" }
                                }
                            }

                            form.issue-form method="post" action={"/repo/" (identifier) "/patches"} {
                                div.form-group {
                                    label.form-label for="title" { "Title" }
                                    input class="form-input" id="title" type="text" name="title" required placeholder="Brief description of the patch";
                                }

                                div.form-group {
                                    label.form-label for="patch_content" { "Patch Content" }
                                    textarea class="form-textarea" id="patch_content" name="patch_content" rows="20" required placeholder="Paste git diff output here..." style="font-family: monospace;";
                                    p.form-help { "Generate with: git diff > my-changes.patch" }
                                }

                                div.form-group {
                                    label.form-label for="description" { "Description (optional)" }
                                    textarea class="form-textarea" id="description" name="description" rows="5" placeholder="Additional context about this patch";
                                }

                                div.form-actions {
                                    button.btn-primary type="submit" { "Create Patch" }
                                    a.btn-secondary href={"/repo/" (identifier) "/patches"} { "Cancel" }
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

