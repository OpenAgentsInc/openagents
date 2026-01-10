/// Create pull request form page
pub fn pr_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Pull Request - " (repo_name) " - GitAfter" }
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
                                    h1.issue-form-title { "Create Pull Request" }
                                    p.issue-form-subtitle { "Repository: " (repo_name) }
                                }
                                div.issue-form-actions {
                                    a.back-link href={"/repo/" (identifier) "/pulls"} { "← Back to Pull Requests" }
                                }
                            }

                            form.issue-form method="post" action={"/repo/" (identifier) "/pulls"} {
                                div.form-group {
                                    label.form-label for="subject" { "Title" }
                                    input class="form-input" id="subject" type="text" name="subject" required placeholder="Brief description of changes";
                                }

                                div.form-group {
                                    label.form-label for="description" { "Description" }
                                    textarea class="form-textarea" id="description" name="description" rows="10" placeholder="Detailed description of the pull request";
                                }

                                div.form-group {
                                    label.form-label for="commit_id" { "Commit ID" }
                                    input class="form-input" id="commit_id" type="text" name="commit_id" required placeholder="Git commit hash";
                                    p.form-help { "The commit ID from your local repository" }
                                }

                                div.form-group {
                                    label.form-label for="clone_url" { "Clone URL" }
                                    input class="form-input" id="clone_url" type="text" name="clone_url" required placeholder="https://github.com/user/repo.git or git@github.com:user/repo.git";
                                    p.form-help { "URL where reviewers can fetch your changes" }
                                }

                                div.form-group {
                                    label.form-label for="trajectory_session" { "Trajectory Session ID (optional)" }
                                    input class="form-input" id="trajectory_session" type="text" name="trajectory_session" placeholder="Event ID of trajectory session";
                                    p.form-help { "For agent-created PRs - links to the work session" }
                                }

                                div.form-group {
                                    label.form-label for="trajectory_hash" { "Trajectory Hash (optional)" }
                                    input class="form-input" id="trajectory_hash" type="text" name="trajectory_hash" placeholder="SHA256 hash of trajectory events";
                                    p.form-help { "For verification of agent work" }
                                }

                                details style="margin: 1rem 0; padding: 1rem; background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333);" {
                                    summary style="cursor: pointer; font-weight: 600; margin-bottom: 0.5rem;" { "⚡ Stacked Diff Options (Advanced)" }

                                    div.form-group {
                                        label.form-label for="depends_on" { "Depends On" }
                                        select
                                            class="form-input"
                                            id="depends_on"
                                            name="depends_on"
                                            hx-get={"/repo/" (identifier) "/pulls/available-deps"}
                                            hx-trigger="load"
                                            hx-swap="innerHTML"
                                            onchange={"htmx.ajax('GET', '/repo/" (identifier) "/pulls/' + this.value + '/stack-info', {target: '#stack-info-target', swap: 'innerHTML'})"}
                                        {
                                            option value="" { "-- Loading available PRs... --" }
                                        }
                                        p.form-help {
                                            "Select a PR this change depends on. Only open/draft PRs shown. The dependency must be merged first."
                                            br;
                                            strong { "⚠️ Creating circular dependencies will fail validation." }
                                        }
                                        div id="stack-info-target" {}
                                    }

                                    div.form-group {
                                        label.form-label for="stack_id" { "Stack ID" }
                                        input class="form-input" id="stack_id" type="text" name="stack_id" placeholder="UUID grouping related PRs";
                                        p.form-help { "Unique identifier for this stack of changes" }
                                    }

                                    div.form-group {
                                        label.form-label for="layer_current" { "Layer Number" }
                                        input class="form-input" id="layer_current" type="number" name="layer_current" placeholder="1" min="1";
                                    }

                                    div.form-group {
                                        label.form-label for="layer_total" { "Total Layers" }
                                        input class="form-input" id="layer_total" type="number" name="layer_total" placeholder="1" min="1";
                                        p.form-help { "E.g., layer 2 of 4" }
                                    }
                                }

                                div.form-actions {
                                    button.btn-primary type="submit" { "Create Pull Request" }
                                    a.btn-secondary href={"/repo/" (identifier) "/pulls"} { "Cancel" }
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

