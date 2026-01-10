/// Agents list page with filtering
pub fn agents_list_page(
    agents: &[(String, i32, i32)],
    min_reputation: &Option<i32>,
    min_merged_prs: &Option<i32>,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Agents - GitAfter" }
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
                        a href="/agents" class="active" { "Agents" }
                    }
                }

                main {
                    h2 { "Agents (" (agents.len()) ")" }

                    // Filter controls
                    form method="get" action="/agents" style="display: flex; gap: 1rem; margin-bottom: 1rem; padding: 1rem; background: #2a2a2a;" {
                        div {
                            label for="min_reputation" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Min Reputation" }
                            input type="number" name="min_reputation" id="min_reputation"
                                value=(min_reputation.map(|r| r.to_string()).unwrap_or_default())
                                style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;";
                        }

                        div {
                            label for="min_merged_prs" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Min Merged PRs" }
                            input type="number" name="min_merged_prs" id="min_merged_prs"
                                value=(min_merged_prs.map(|p| p.to_string()).unwrap_or_default())
                                style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;";
                        }

                        button type="submit" style="padding: 0.5rem 1rem; background: #4a9eff; color: #fff; border: none; cursor: pointer;" { "Apply Filters" }
                        a href="/agents" style="padding: 0.5rem 1rem; background: #444; color: #fff; text-decoration: none; display: inline-block;" { "Clear" }
                    }

                    @if agents.is_empty() {
                        p { "No agents found." }
                    } @else {
                        table style="width: 100%; border-collapse: collapse;" {
                            thead {
                                tr style="background: #2a2a2a;" {
                                    th style="padding: 0.75rem; text-align: left; border-bottom: 2px solid #444;" { "Agent" }
                                    th style="padding: 0.75rem; text-align: left; border-bottom: 2px solid #444;" { "Reputation Score" }
                                    th style="padding: 0.75rem; text-align: left; border-bottom: 2px solid #444;" { "Merged PRs" }
                                    th style="padding: 0.75rem; text-align: left; border-bottom: 2px solid #444;" { "Actions" }
                                }
                            }
                            tbody {
                                @for (pubkey, reputation, merged_prs) in agents {
                                    tr style="border-bottom: 1px solid #333;" {
                                        td style="padding: 0.75rem;" {
                                            a href={"/agent/" (pubkey)} { (format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len()-8..])) }
                                        }
                                        td style="padding: 0.75rem;" { (reputation) }
                                        td style="padding: 0.75rem;" { (merged_prs) }
                                        td style="padding: 0.75rem;" {
                                            a href={"/agent/" (pubkey)} style="color: #4a9eff;" { "View Profile" }
                                        }
                                    }
                                }
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

