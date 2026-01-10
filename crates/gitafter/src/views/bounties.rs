/// Bounty discovery page - list all available bounties across repositories
pub fn bounties_discovery_page(
    bounties: &[(String, String, String, String, u64, String)],
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Bounties - GitAfter" }
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
                        a href="/issues" { "Issues" }
                        a href="/bounties" class="active" { "Bounties" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        h2 { "💰 Available Bounties (" (bounties.len()) ")" }

                        @if bounties.is_empty() {
                            p.placeholder { "No bounties available. Create an issue and attach a bounty!" }
                        } @else {
                            table style="width: 100%; border-collapse: collapse; margin-top: 1rem;" {
                                thead {
                                    tr style="background: #2a2a2a; border-bottom: 2px solid #444;" {
                                        th style="padding: 0.75rem; text-align: left;" { "Repository" }
                                        th style="padding: 0.75rem; text-align: left;" { "Issue" }
                                        th style="padding: 0.75rem; text-align: right;" { "Amount (sats)" }
                                        th style="padding: 0.75rem; text-align: center;" { "Actions" }
                                    }
                                }
                                tbody {
                                    @for (repo_name, repo_id, issue_subject, issue_id, amount, _bounty_id) in bounties {
                                        tr style="border-bottom: 1px solid #333;" {
                                            td style="padding: 0.75rem;" {
                                                a href={"/repo/" (repo_id)} style="color: #4a9eff; text-decoration: none;" { (repo_name) }
                                            }
                                            td style="padding: 0.75rem;" {
                                                a href={"/repo/" (repo_id) "/issues/" (issue_id)} style="color: #fff; text-decoration: none;" { (issue_subject) }
                                            }
                                            td style="padding: 0.75rem; text-align: right; color: #fbbf24; font-weight: bold;" {
                                                "⚡ " (amount)
                                            }
                                            td style="padding: 0.75rem; text-align: center;" {
                                                a href={"/repo/" (repo_id) "/issues/" (issue_id)} style="background: #4a9eff; color: #fff; padding: 0.5rem 1rem; text-decoration: none; display: inline-block;" { "View Issue" }
                                            }
                                        }
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

