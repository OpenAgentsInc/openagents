/// Search results page
pub fn search_results_page(query: &str, repositories: &[Event], issues: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Search: " (query) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("../styles.css"))
                    "
                    .search-header {
                        margin-bottom: 2rem;
                    }
                    .search-stats {
                        color: var(--text-secondary, #888);
                        margin-top: 0.5rem;
                    }
                    .result-section {
                        margin-bottom: 3rem;
                    }
                    .result-section h2 {
                        border-bottom: 1px solid var(--border-color, #333);
                        padding-bottom: 0.5rem;
                        margin-bottom: 1rem;
                    }
                    "
                }
            }
            body {
                header {
                    h1 { "⚡ GitAfter" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/search" class="active" { "Search" }
                    }
                    div.content {
                        div.search-header {
                            h1 { "Search Results" }
                            p { "Query: \"" (query) "\"" }
                            p.search-stats {
                                (repositories.len()) " repositories, " (issues.len()) " issues found"
                            }
                        }

                        @if !repositories.is_empty() {
                            section.result-section {
                                h2 { "Repositories (" (repositories.len()) ")" }
                                div.repositories-list {
                                    @for repo in repositories {
                                        @let repo_name = get_tag_value(repo, "name")
                                            .unwrap_or_else(|| "Unnamed Repository".to_string());
                                        @let identifier = get_tag_value(repo, "d")
                                            .unwrap_or_else(|| repo.id.clone());

                                        div.repo-card {
                                            a.repo-link href={"/repo/" (identifier)} {
                                                h3.repo-name { (repo_name) }
                                            }
                                            @if !repo.content.is_empty() {
                                                p.repo-description { (repo.content) }
                                            }
                                            div.repo-meta {
                                                span.repo-author {
                                                    @let author_short = if repo.pubkey.len() > 16 {
                                                        format!("{}...{}", &repo.pubkey[..8], &repo.pubkey[repo.pubkey.len()-8..])
                                                    } else {
                                                        repo.pubkey.clone()
                                                    };
                                                    "by " (author_short)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        @if !issues.is_empty() {
                            section.result-section {
                                h2 { "Issues (" (issues.len()) ")" }
                                div.issues-list {
                                    @for issue in issues {
                                        @let issue_title = get_tag_value(issue, "subject")
                                            .or_else(|| get_tag_value(issue, "title"))
                                            .unwrap_or_else(|| "Untitled Issue".to_string());
                                        @let issue_status = get_tag_value(issue, "status")
                                            .unwrap_or_else(|| "open".to_string());
                                        @let repo_address = get_tag_value(issue, "a");

                                        div.issue-card {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (issue_title) }
                                                    span class={"issue-status " (issue_status)} {
                                                        (issue_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    @if let Some(repo) = repo_address {
                                                        @let repo_parts: Vec<&str> = repo.split(':').collect();
                                                        @if repo_parts.len() >= 3 {
                                                            @let repo_id = repo_parts[2];
                                                            span.issue-author { "Repo: " a href={"/repo/" (repo_id)} { (repo_id) } }
                                                            span.issue-separator { "•" }
                                                        }
                                                    }
                                                    span.issue-time { (format_relative_time(issue.created_at)) }
                                                }
                                            }
                                            @if !issue.content.is_empty() && issue.content.len() < 200 {
                                                p.issue-preview { (issue.content) }
                                            } @else if !issue.content.is_empty() {
                                                p.issue-preview { (&issue.content[..200]) "..." }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        @if repositories.is_empty() && issues.is_empty() {
                            div.empty-state {
                                p { "No results found for \"" (query) "\"" }
                                p { "Try different keywords or check your spelling." }
                            }
                        }
                    }
                }
                footer {
                    p { "Powered by NIP-34 (Git Stuff) • NIP-50 (Search) • NIP-57 (Zaps)" }
                }
            }
        }
    }
}

