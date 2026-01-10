/// Git status page showing local changes
pub fn git_status_page(identifier: &str, changes: &[crate::git::FileChange]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Git Status - " (identifier) " - GitAfter" }
                style {
                    (include_str!("../styles.css"))
                }
            }
            body {
                header {
                    h1 { "GitAfter" }
                    nav {
                        a href="/" { "Home" }
                        " | "
                        a href={"/repo/" (identifier)} { "Repository" }
                        " | "
                        span { "Git Status" }
                    }
                }

                main {
                    h2 { "Local Changes - " (identifier) }

                    @if changes.is_empty() {
                        p { "No local changes" }
                    } @else {
                        div.file-changes {
                            h3 { "Modified Files" }
                            table {
                                thead {
                                    tr {
                                        th { "Status" }
                                        th { "File Path" }
                                    }
                                }
                                tbody {
                                    @for change in changes {
                                        tr {
                                            td.status {
                                                @match change.status {
                                                    crate::git::FileStatus::Untracked => span.status-untracked { "Untracked" },
                                                    crate::git::FileStatus::Modified => span.status-modified { "Modified" },
                                                    crate::git::FileStatus::Added => span.status-added { "Added" },
                                                    crate::git::FileStatus::Deleted => span.status-deleted { "Deleted" },
                                                    crate::git::FileStatus::Renamed => span.status-renamed { "Renamed" },
                                                    crate::git::FileStatus::Conflicted => span.status-conflicted { "Conflicted" },
                                                }
                                            }
                                            td.file-path { (change.path) }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div.actions {
                        a.button href={"/repo/" (identifier)} { "← Back to Repository" }
                        " "
                        @if !changes.is_empty() {
                            form method="post" action={"/repo/" (identifier) "/git/push"} style="display: inline-block; margin-left: 1rem;" {
                                input type="hidden" name="remote" value="origin";
                                button.button type="submit" { "Push to origin" }
                            }
                        }
                    }
                }

                footer {
                    p { "GitAfter - Nostr-native GitHub alternative" }
                }
            }
        }
    }
}

