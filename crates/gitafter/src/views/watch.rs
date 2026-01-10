/// Watched repositories page
pub fn watched_repositories_page(repositories: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Watched Repositories - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("../styles.css"))
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
                        a href="/watched" class="active" { "Watched" }
                        a href="/search" { "Search" }
                    }
                    div.content {
                        div.repositories-container {
                            h1 { "⭐ Watched Repositories" }

                            @if repositories.is_empty() {
                                div.empty-state {
                                    p { "You're not watching any repositories yet." }
                                    p.info-text { "Watch repositories to get notified about new issues and pull requests." }
                                    a href="/" { "Browse repositories →" }
                                }
                            } @else {
                                p.repositories-count { (repositories.len()) " watched repositories" }

                                div.repositories-list {
                                    @for repo in repositories {
                                        @let repo_name = get_tag_value(repo, "name")
                                            .unwrap_or_else(|| "Unnamed Repository".to_string());
                                        @let description = if !repo.content.is_empty() {
                                            repo.content.clone()
                                        } else {
                                            "No description provided".to_string()
                                        };
                                        @let identifier = get_tag_value(repo, "d")
                                            .unwrap_or_else(|| repo.id.clone());

                                        div.repo-card {
                                            a.repo-link href={"/repo/" (identifier)} {
                                                h3.repo-name { (repo_name) }
                                            }
                                            p.repo-description { (description) }
                                            div.repo-meta {
                                                span.repo-author {
                                                    @let author_short = if repo.pubkey.len() > 16 {
                                                        format!("{}...{}", &repo.pubkey[..8], &repo.pubkey[repo.pubkey.len()-8..])
                                                    } else {
                                                        repo.pubkey.clone()
                                                    };
                                                    "by " (author_short)
                                                }
                                                span.repo-separator { "•" }
                                                a.repo-link href={"/repo/" (identifier)} { "View Issues" }
                                            }
                                        }
                                    }
                                }
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

