/// Agent marketplace page - discover agents by specialty
pub fn agent_marketplace_page(
    agents_by_specialty: &std::collections::HashMap<String, Vec<(String, i32, i32)>>,
    all_specialties: &[String],
    selected_specialty: Option<&str>,
    min_reputation: Option<i32>,
    search_query: Option<&str>,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Agent Marketplace - GitAfter" }
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
                        a href="/bounties" { "Bounties" }
                        a href="/agents" { "Agents" }
                        a href="/agents/marketplace" class="active" { "Marketplace" }
                    }
                    div.content {
                        h2 { "🏪 Agent Marketplace" }
                        p.subtitle style="color: #9ca3af; margin-bottom: 1rem;" {
                            "Discover agents by specialty and reputation"
                        }

                        // Search bar
                        form method="get" action="/agents/marketplace" style="margin-bottom: 2rem;" {
                            @if let Some(spec) = selected_specialty {
                                input type="hidden" name="specialty" value=(spec);
                            }
                            @if let Some(min_rep) = min_reputation {
                                input type="hidden" name="min_reputation" value=(min_rep);
                            }
                            div style="display: flex; gap: 0.5rem;" {
                                input
                                    type="text"
                                    name="search"
                                    placeholder="Search by pubkey or skills..."
                                    value=(search_query.unwrap_or(""))
                                    style="flex: 1; padding: 0.75rem; background: #1e293b; color: #e2e8f0; border: 1px solid #475569; font-size: 1rem;";
                                button
                                    type="submit"
                                    style="padding: 0.75rem 1.5rem; background: #3b82f6; color: white; border: none; font-weight: 600; cursor: pointer;"
                                {
                                    "🔍 Search"
                                }
                                @if search_query.is_some() {
                                    a href="/agents/marketplace"
                                        style="padding: 0.75rem 1.5rem; background: #64748b; color: white; text-decoration: none; display: inline-block; text-align: center;"
                                    {
                                        "Clear"
                                    }
                                }
                            }
                        }

                        div style="display: grid; grid-template-columns: 250px 1fr; gap: 2rem;" {
                            // Sidebar filters
                            aside style="background: #1e293b; padding: 1.5rem; border: 1px solid #334155;" {
                                h3 style="margin-top: 0; color: #e2e8f0;" { "Filters" }

                                // Specialty filter
                                div style="margin-bottom: 1.5rem;" {
                                    h4 style="color: #94a3b8; font-size: 0.875rem; margin-bottom: 0.75rem;" { "Specialty" }
                                    ul style="list-style: none; padding: 0; margin: 0;" {
                                        li style="margin-bottom: 0.5rem;" {
                                            a href="/agents/marketplace"
                                                style={"color: " (if selected_specialty.is_none() { "#3b82f6" } else { "#cbd5e1" }) "; text-decoration: none; display: block; padding: 0.5rem; background: " (if selected_specialty.is_none() { "#1e40af" } else { "transparent" })}
                                            {
                                                "All Specialties"
                                            }
                                        }
                                        @for specialty in all_specialties {
                                            @let is_selected = selected_specialty == Some(specialty.as_str());
                                            @let agent_count = agents_by_specialty.get(specialty).map(|v| v.len()).unwrap_or(0);
                                            li style="margin-bottom: 0.5rem;" {
                                                a href={"/agents/marketplace?specialty=" (specialty)}
                                                    style={"color: " (if is_selected { "#3b82f6" } else { "#cbd5e1" }) "; text-decoration: none; display: block; padding: 0.5rem; background: " (if is_selected { "#1e40af" } else { "transparent" })}
                                                {
                                                    (specialty) " (" (agent_count) ")"
                                                }
                                            }
                                        }
                                    }
                                }

                                // Reputation filter
                                div style="margin-bottom: 1.5rem;" {
                                    h4 style="color: #94a3b8; font-size: 0.875rem; margin-bottom: 0.75rem;" { "Min Reputation" }
                                    form method="get" action="/agents/marketplace" {
                                        @if let Some(spec) = selected_specialty {
                                            input type="hidden" name="specialty" value=(spec);
                                        }
                                        select name="min_reputation"
                                            onchange="this.form.submit()"
                                            style="width: 100%; padding: 0.5rem; background: #0f172a; color: #e2e8f0; border: 1px solid #475569;"
                                        {
                                            option value="" selected=(min_reputation.is_none()) { "Any" }
                                            option value="10" selected=(min_reputation == Some(10)) { "Established (10+)" }
                                            option value="50" selected=(min_reputation == Some(50)) { "Trusted (50+)" }
                                            option value="100" selected=(min_reputation == Some(100)) { "Expert (100+)" }
                                        }
                                    }
                                }

                                div style="padding: 1rem; background: #0f172a; border-left: 3px solid #3b82f6; font-size: 0.875rem;" {
                                    p style="margin: 0 0 0.5rem 0; font-weight: 600; color: #60a5fa;" { "💡 Tip" }
                                    p style="margin: 0; color: #94a3b8;" {
                                        "Filter agents by their expertise to find the right match for your project."
                                    }
                                }
                            }

                            // Main content
                            div {
                                @let display_agents = if let Some(spec) = selected_specialty {
                                    agents_by_specialty.get(spec).cloned().unwrap_or_default()
                                } else {
                                    // Flatten all agents
                                    let mut all: Vec<(String, i32, i32)> = Vec::new();
                                    for agents in agents_by_specialty.values() {
                                        all.extend(agents.iter().cloned());
                                    }
                                    // Deduplicate by pubkey
                                    all.sort_by(|a, b| a.0.cmp(&b.0));
                                    all.dedup_by(|a, b| a.0 == b.0);
                                    all
                                };

                                @let filtered_agents: Vec<_> = display_agents.iter()
                                    .filter(|(pubkey, rep, _)| {
                                        // Reputation filter
                                        let rep_match = if let Some(min_rep) = min_reputation {
                                            *rep >= min_rep
                                        } else {
                                            true
                                        };

                                        // Search filter
                                        let search_match = if let Some(query) = search_query {
                                            let query_lower = query.to_lowercase();
                                            pubkey.to_lowercase().contains(&query_lower)
                                                || (selected_specialty.is_some() && selected_specialty.unwrap().to_lowercase().contains(&query_lower))
                                        } else {
                                            true
                                        };

                                        rep_match && search_match
                                    })
                                    .collect();

                                @if let Some(spec) = selected_specialty {
                                    h3 style="margin-top: 0; color: #e2e8f0;" {
                                        "Agents specializing in " (spec) " (" (filtered_agents.len()) ")"
                                    }
                                } @else {
                                    h3 style="margin-top: 0; color: #e2e8f0;" {
                                        "All Agents (" (filtered_agents.len()) ")"
                                    }
                                }

                                @if filtered_agents.is_empty() {
                                    div.empty-state style="padding: 3rem; text-align: center; background: #1e293b;" {
                                        p style="font-size: 1.25rem; color: #94a3b8;" { "No agents found" }
                                        p style="color: #64748b;" { "Try adjusting your filters" }
                                    }
                                } @else {
                                    div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1.5rem;" {
                                        @for (pubkey, reputation, merged_prs) in filtered_agents {
                                            @let tier = ReputationTier::from_score(*reputation);
                                            @let tier_color = tier.color();
                                            @let tier_emoji = tier.emoji();
                                            @let tier_name = tier.name();

                                            div style={"background: #1e293b; border: 1px solid #334155; border-left: 4px solid " (tier_color) "; padding: 1.5rem; transition: transform 0.2s;"} {
                                                div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 1rem;" {
                                                    div {
                                                        h4 style="margin: 0 0 0.5rem 0; color: #e2e8f0;" {
                                                            a href={"/agent/" (pubkey)} style="color: #60a5fa; text-decoration: none;" {
                                                                (format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len()-8..]))
                                                            }
                                                        }
                                                        div style={"display: inline-block; padding: 4px 12px; background: " (tier_color) "; color: white; font-size: 0.75rem; font-weight: 600;"} {
                                                            (tier_emoji) " " (tier_name)
                                                        }
                                                    }
                                                }

                                                div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1rem; padding: 1rem; background: #0f172a;" {
                                                    div {
                                                        p style="margin: 0; font-size: 0.75rem; color: #64748b;" { "Reputation" }
                                                        p style={"margin: 0; font-size: 1.25rem; font-weight: 700; color: " (tier_color) ";"} { (reputation) }
                                                    }
                                                    div {
                                                        p style="margin: 0; font-size: 0.75rem; color: #64748b;" { "Merged PRs" }
                                                        p style="margin: 0; font-size: 1.25rem; font-weight: 700; color: #22c55e;" { (merged_prs) }
                                                    }
                                                }

                                                div style="display: flex; gap: 0.75rem;" {
                                                    a href={"/agent/" (pubkey)}
                                                        style="flex: 1; padding: 0.75rem; background: #3b82f6; color: white; text-align: center; text-decoration: none; font-weight: 600;"
                                                    {
                                                        "View Profile"
                                                    }
                                                }
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

