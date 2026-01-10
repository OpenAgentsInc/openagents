/// Agent profile page
pub fn agent_profile_page(
    agent_pubkey: &str,
    pull_requests: &[Event],
    issue_claims: &[Event],
    reputation_labels: &[Event],
) -> Markup {
    // Format pubkey for display
    let display_pubkey = if agent_pubkey.len() > 16 {
        format!(
            "{}...{}",
            &agent_pubkey[..8],
            &agent_pubkey[agent_pubkey.len() - 8..]
        )
    } else {
        agent_pubkey.to_string()
    };

    // Count merged PRs (those with status applied/merged)
    let merged_count = pull_requests
        .iter()
        .filter(|pr| {
            get_tag_value(pr, "status")
                .map(|s| s == "applied" || s == "merged")
                .unwrap_or(false)
        })
        .count();

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Agent Profile: " (display_pubkey) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("../styles.css"))
                    "
                    .stats-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
                        gap: 1rem;
                        margin-bottom: 2rem;
                    }
                    .stat-card {
                        background: var(--card-bg, #1a1a1a);
                        border: 1px solid var(--border-color, #333);
                        padding: 1.5rem;
                        text-align: center;
                    }
                    .stat-value {
                        display: block;
                        font-size: 2rem;
                        font-weight: 600;
                        color: var(--accent-color, #0ea5e9);
                        margin-bottom: 0.5rem;
                    }
                    .stat-label {
                        display: block;
                        font-size: 0.9rem;
                        color: var(--text-secondary, #888);
                    }
                    .pubkey-display {
                        margin-top: 1rem;
                        padding: 1rem;
                        background: var(--card-bg, #1a1a1a);
                        border: 1px solid var(--border-color, #333);
                    }
                    .agent-pubkey {
                        word-break: break-all;
                        font-size: 0.9rem;
                    }
                    .profile-header {
                        margin-bottom: 2rem;
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
                        a href="/agents" class="active" { "Agents" }
                    }
                    div.content {
                        div.agent-profile {
                            div.profile-header {
                                h1 { "🤖 Agent Profile" }
                                div.pubkey-display {
                                    span.label { "Pubkey: " }
                                    code.agent-pubkey { (agent_pubkey) }
                                }
                            }

                            section.issue-section {
                                h2 { "Contribution Stats" }
                                div.stats-grid {
                                    div.stat-card {
                                        span.stat-value { (pull_requests.len()) }
                                        span.stat-label { "Pull Requests" }
                                    }
                                    div.stat-card {
                                        span.stat-value { (merged_count) }
                                        span.stat-label { "Merged PRs" }
                                    }
                                    div.stat-card {
                                        span.stat-value { (issue_claims.len()) }
                                        span.stat-label { "Issues Claimed" }
                                    }
                                    div.stat-card {
                                        span.stat-value { (reputation_labels.len()) }
                                        span.stat-label { "Reputation Labels" }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "Reputation Overview" }
                                @let reputation_score = (pull_requests.len() * 5 + merged_count * 15 + reputation_labels.len() * 10) as i32;
                                @let tier = crate::reputation::ReputationTier::from_score(reputation_score);
                                @let tier_name = tier.name();
                                @let tier_color = tier.color();
                                @let tier_emoji = tier.emoji();

                                div.reputation-display {
                                    div.reputation-tier style={"background: " (tier_color) "22; border: 2px solid " (tier_color) "; padding: 1.5rem; margin-bottom: 1rem; text-align: center;"} {
                                        div style="font-size: 3rem; margin-bottom: 0.5rem;" { (tier_emoji) }
                                        div style={"font-size: 1.5rem; font-weight: 600; color: " (tier_color) ";"} { (tier_name) " Tier" }
                                        div style="font-size: 1rem; color: var(--text-secondary); margin-top: 0.5rem;" {
                                            "Reputation Score: " (reputation_score)
                                        }
                                    }

                                    div.reputation-breakdown style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem;" {
                                        div.breakdown-item style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;" {
                                            div style="font-size: 0.9rem; color: var(--text-secondary); margin-bottom: 0.5rem;" { "Total PRs" }
                                            div style="font-size: 1.5rem; font-weight: 600; color: var(--accent-color, #0ea5e9);" { (pull_requests.len()) }
                                            div style="font-size: 0.85rem; color: var(--text-secondary); margin-top: 0.25rem;" {
                                                "+" (pull_requests.len() * 5) " points"
                                            }
                                        }
                                        div.breakdown-item style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;" {
                                            div style="font-size: 0.9rem; color: var(--text-secondary); margin-bottom: 0.5rem;" { "Merged PRs" }
                                            div style="font-size: 1.5rem; font-weight: 600; color: #00ff88;" { (merged_count) }
                                            div style="font-size: 0.85rem; color: var(--text-secondary); margin-top: 0.25rem;" {
                                                "+" (merged_count * 15) " points"
                                            }
                                        }
                                        div.breakdown-item style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;" {
                                            div style="font-size: 0.9rem; color: var(--text-secondary); margin-bottom: 0.5rem;" { "Reputation Labels" }
                                            div style="font-size: 1.5rem; font-weight: 600; color: #ffd700;" { (reputation_labels.len()) }
                                            div style="font-size: 0.85rem; color: var(--text-secondary); margin-top: 0.25rem;" {
                                                "+" (reputation_labels.len() * 10) " points"
                                            }
                                        }
                                    }

                                    div.tier-progress style="margin-top: 1.5rem; padding: 1rem; background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333);" {
                                        div style="font-size: 0.9rem; color: var(--text-secondary); margin-bottom: 0.5rem;" { "Next Tier Progress" }
                                        @let next_tier_threshold = match tier {
                                            crate::reputation::ReputationTier::New => 10,
                                            crate::reputation::ReputationTier::Established => 50,
                                            crate::reputation::ReputationTier::Trusted => 100,
                                            crate::reputation::ReputationTier::Expert => reputation_score,
                                        };
                                        @let progress_pct = if matches!(tier, crate::reputation::ReputationTier::Expert) {
                                            100
                                        } else {
                                            ((reputation_score as f64 / next_tier_threshold as f64) * 100.0).min(100.0) as i32
                                        };
                                        div style="width: 100%; height: 20px; background: #333; margin-bottom: 0.5rem;" {
                                            div style={"width: " (progress_pct) "%; height: 100%; background: " (tier_color) "; transition: width 0.3s;"} {}
                                        }
                                        div style="font-size: 0.85rem; color: var(--text-secondary);" {
                                            @if matches!(tier, crate::reputation::ReputationTier::Expert) {
                                                "Maximum tier reached!"
                                            } @else {
                                                (reputation_score) " / " (next_tier_threshold) " points (" (progress_pct) "%)"
                                            }
                                        }
                                    }
                                }
                            }

                            @if !reputation_labels.is_empty() {
                                section.issue-section {
                                    h2 { "Reputation Labels" }
                                    div.claims-list {
                                        @for label in reputation_labels {
                                            @let label_issuer = if label.pubkey.len() > 16 {
                                                format!("{}...{}", &label.pubkey[..8], &label.pubkey[label.pubkey.len()-8..])
                                            } else {
                                                label.pubkey.clone()
                                            };
                                            @let label_value = get_tag_value(label, "l");
                                            @let label_namespace = get_tag_value(label, "L");

                                            div.claim-card {
                                                div.claim-header {
                                                    span.claim-author {
                                                        @if let Some(val) = &label_value {
                                                            "🏷️ " (val)
                                                        } @else {
                                                            "🏷️ Label"
                                                        }
                                                    }
                                                    span.claim-time title={(label.created_at)} { (format_relative_time(label.created_at)) }
                                                }
                                                div.claim-content {
                                                    p { "From: " (label_issuer) }
                                                    @if let Some(ns) = label_namespace {
                                                        p { "Namespace: " (ns) }
                                                    }
                                                    @if !label.content.is_empty() {
                                                        p { (label.content) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            @if !pull_requests.is_empty() {
                                section.issue-section {
                                    h2 { "Recent Pull Requests" }
                                    div.issues-list {
                                        @for pr in pull_requests.iter().take(10) {
                                            @let pr_title = get_tag_value(pr, "subject")
                                                .unwrap_or_else(|| "Untitled PR".to_string());
                                            @let pr_status = get_tag_value(pr, "status")
                                                .unwrap_or_else(|| "open".to_string());
                                            @let repo_address = get_tag_value(pr, "a");

                                            div.issue-card {
                                                div.issue-header {
                                                    div.issue-title-row {
                                                        h3.issue-title { (pr_title) }
                                                        span class={"issue-status " (pr_status)} {
                                                            (pr_status)
                                                        }
                                                    }
                                                    div.issue-meta {
                                                        @if let Some(repo) = repo_address {
                                                            span.issue-author { "Repo: " (repo) }
                                                            span.issue-separator { "•" }
                                                        }
                                                        span.issue-time { (format_relative_time(pr.created_at)) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            @if !issue_claims.is_empty() {
                                section.issue-section {
                                    h2 { "Recent Issue Claims" }
                                    div.claims-list {
                                        @for claim in issue_claims.iter().take(10) {
                                            @let trajectory = get_tag_value(claim, "trajectory");
                                            @let estimate = get_tag_value(claim, "estimate");

                                            div.claim-card {
                                                div.claim-header {
                                                    span.claim-author { "Issue Claim" }
                                                    span.claim-time title={(claim.created_at)} { (format_relative_time(claim.created_at)) }
                                                }
                                                @if !claim.content.is_empty() {
                                                    div.claim-content {
                                                        p { (claim.content) }
                                                    }
                                                }
                                                @if let Some(est) = estimate {
                                                    div.claim-estimate {
                                                        span.label { "Estimated: " }
                                                        span { (est) " seconds" }
                                                    }
                                                }
                                                @if let Some(traj) = trajectory {
                                                    div.claim-trajectory {
                                                        span.label { "Trajectory: " }
                                                        a href={"/trajectory/" (traj)} {
                                                            code { (traj) }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "Publish Reputation Label" }
                                p style="color: var(--text-secondary, #888); margin-bottom: 1rem;" {
                                    "Publish a NIP-32 reputation label for this agent (kind:1985)"
                                }
                                form method="post" action={"/agent/" (agent_pubkey) "/reputation"} {
                                    div.form-group {
                                        label for="label" { "Label Type" }
                                        select name="label" id="label" required="" {
                                            option value="quality" { "Quality - High quality code" }
                                            option value="review" { "Review - Thorough code reviews" }
                                            option value="merge" { "Merge - Successfully merged PRs" }
                                            option value="responsive" { "Responsive - Quick response time" }
                                            option value="helpful" { "Helpful - Helpful collaborator" }
                                        }
                                    }
                                    div.form-group {
                                        label for="rating" { "Rating (1-10)" }
                                        input type="number" name="rating" id="rating" min="1" max="10" placeholder="5" {}
                                    }
                                    button.btn-primary type="submit" { "Publish Label" }
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

