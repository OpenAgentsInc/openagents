/// Issue detail page
pub fn issue_detail_page(
    repository: &Event,
    issue: &Event,
    claims: &[Event],
    bounties: &[Event],
    comments: &[Event],
    identifier: &str,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let issue_title =
        get_tag_value(issue, "subject").unwrap_or_else(|| "Untitled Issue".to_string());
    let issue_status = get_tag_value(issue, "status").unwrap_or_else(|| "open".to_string());

    // Format pubkey for display
    let issue_author = if issue.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &issue.pubkey[..8],
            &issue.pubkey[issue.pubkey.len() - 8..]
        )
    } else {
        issue.pubkey.clone()
    };

    // Extract all tags for display
    let all_tags = &issue.tags;

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (issue_title) " - " (repo_name) " - GitAfter" }
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
                        div.issue-detail {
                            div.issue-detail-header {
                                div {
                                    h1.issue-detail-title { (issue_title) }
                                    div.issue-detail-meta {
                                        span class={"issue-status " (issue_status)} {
                                            (issue_status)
                                        }
                                        span.issue-separator { "•" }
                                        span.issue-author { "by " (issue_author) }
                                        span.issue-separator { "•" }
                                        span.issue-time { (format_relative_time(issue.created_at)) }
                                    }
                                }
                                div.issue-detail-actions {
                                    a.back-link href={"/repo/" (identifier) "/issues"} { "← Back to Issues" }
                                }
                            }

                            section.issue-section {
                                h2 { "Repository Context" }
                                div.repo-context {
                                    a.repo-link href={"/repo/" (identifier)} { (repo_name) }
                                    span.repo-id-label { " (" (identifier) ")" }
                                }
                            }

                            @if !issue.content.is_empty() {
                                section.issue-section {
                                    h2 { "Description" }
                                    div.issue-content {
                                        p { (issue.content) }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "💰 Bounties" }
                                @if bounties.is_empty() {
                                    p.empty-state { "No bounties yet. Create one to incentivize work on this issue!" }
                                } @else {
                                    div.bounties-list {
                                        @for bounty in bounties {
                                            @let bounty_creator = if bounty.pubkey.len() > 16 {
                                                format!("{}...{}", &bounty.pubkey[..8], &bounty.pubkey[bounty.pubkey.len()-8..])
                                            } else {
                                                bounty.pubkey.clone()
                                            };
                                            @let amount = get_tag_value(bounty, "amount");
                                            @let expiry = get_tag_value(bounty, "expiry");
                                            @let conditions = get_all_tag_values(bounty, "conditions");

                                            div.bounty-card {
                                                div.bounty-header {
                                                    @if let Some(amt) = amount {
                                                        span.bounty-amount { "⚡ " (amt) " sats" }
                                                    }
                                                    span.bounty-creator { "offered by " (bounty_creator) }
                                                }
                                                @if let Some(exp) = expiry {
                                                    div.bounty-expiry {
                                                        span.label { "Expires: " }
                                                        span { (exp) }
                                                    }
                                                }
                                                @if !conditions.is_empty() {
                                                    div.bounty-conditions {
                                                        h4 { "Conditions:" }
                                                        ul {
                                                            @for condition in conditions {
                                                                li { (condition) }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                form.bounty-form
                                    hx-post={"/repo/" (identifier) "/issues/" (issue.id) "/bounty"}
                                    hx-target="this"
                                    hx-swap="outerHTML" {
                                    h3 { "Create Bounty" }
                                    div.form-group {
                                        label for="bounty_amount" { "Amount (sats)" }
                                        input
                                            type="number"
                                            name="amount"
                                            id="bounty_amount"
                                            placeholder="50000"
                                            required {}
                                    }
                                    div.form-group {
                                        label for="bounty_expiry" { "Expiry (Unix timestamp, optional)" }
                                        input
                                            type="number"
                                            name="expiry"
                                            id="bounty_expiry"
                                            placeholder="1700000000" {}
                                    }
                                    div.form-group {
                                        label for="bounty_conditions" { "Conditions (one per line, optional)" }
                                        textarea
                                            name="conditions"
                                            id="bounty_conditions"
                                            placeholder="Must include tests\nMust pass CI\nMust update docs"
                                            rows="3" {}
                                    }
                                    button.submit-button type="submit" { "Create Bounty" }
                                }
                            }

                            section.issue-section {
                                h2 { "Claims" }
                                @if claims.is_empty() {
                                    p.empty-state { "No claims yet. Be the first to claim this issue!" }
                                } @else {
                                    // Sort claims by timestamp (earliest first) for conflict resolution
                                    @let mut sorted_claims = claims.to_vec();
                                    @let _ = { sorted_claims.sort_by_key(|c| c.created_at); };

                                    div.claims-list {
                                        @for (idx, claim) in sorted_claims.iter().enumerate() {
                                            @let claimer_pubkey = if claim.pubkey.len() > 16 {
                                                format!("{}...{}", &claim.pubkey[..8], &claim.pubkey[claim.pubkey.len()-8..])
                                            } else {
                                                claim.pubkey.clone()
                                            };
                                            @let trajectory = get_tag_value(claim, "trajectory");
                                            @let estimate = get_tag_value(claim, "estimate");
                                            @let is_first_claim = idx == 0;

                                            @let card_style = if is_first_claim {
                                                "border: 2px solid #fbbf24; background: linear-gradient(135deg, #1a1a1a 0%, #2a2010 100%);"
                                            } else {
                                                ""
                                            };

                                            div.claim-card style=(card_style) {
                                                div.claim-header {
                                                    span.claim-author { "🤖 " (claimer_pubkey) }
                                                    @if is_first_claim {
                                                        span style="color: #fbbf24; font-weight: 600; margin-left: 0.5rem;" { "🏆 First Claim" }
                                                    } @else {
                                                        span style="color: #888; margin-left: 0.5rem;" { "⏳ Backup Claim" }
                                                    }
                                                    span.claim-time title={(claim.created_at)} { "claimed " (format_relative_time(claim.created_at)) }
                                                }
                                                @if !claim.content.is_empty() {
                                                    div.claim-content {
                                                        p { (claim.content) }
                                                    }
                                                }
                                                @if let Some(est) = estimate {
                                                    div.claim-estimate {
                                                        span.label { "Estimated completion: " }
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

                                @let has_claims = !claims.is_empty();

                                form.claim-form
                                    hx-post={"/repo/" (identifier) "/issues/" (issue.id) "/claim"}
                                    hx-target="this"
                                    hx-swap="outerHTML" {
                                    h3 { "Claim this Issue" }

                                    @if has_claims {
                                        div style="padding: 1rem; background: #fef3c7; border-left: 4px solid #f59e0b; color: #92400e; margin-bottom: 1rem;" {
                                            p { "⚠️ This issue has already been claimed. First claim takes precedence." }
                                        }
                                    }

                                    div.form-group {
                                        label for="claim_message" { "Message (optional)" }
                                        textarea
                                            name="content"
                                            id="claim_message"
                                            placeholder="I'll work on this issue..."
                                            rows="3"
                                            disabled[has_claims] {}
                                    }
                                    div.form-group {
                                        label for="estimate" { "Estimated completion time (seconds)" }
                                        input
                                            type="number"
                                            name="estimate"
                                            id="estimate"
                                            placeholder="7200"
                                            disabled[has_claims] {}
                                    }
                                    button.submit-button type="submit" disabled[has_claims] style={
                                        @if has_claims {
                                            "opacity: 0.5; cursor: not-allowed;"
                                        } @else {
                                            ""
                                        }
                                    } { "Claim Issue" }
                                }
                            }

                            section.issue-section {
                                h2 { "💬 Comments" }
                                @if comments.is_empty() {
                                    p.empty-state { "No comments yet. Be the first to comment!" }
                                } @else {
                                    div.comments-list style="display: flex; flex-direction: column; gap: 1rem;" {
                                        @for comment in comments {
                                            @let commenter_pubkey = if comment.pubkey.len() > 16 {
                                                format!("{}...{}", &comment.pubkey[..8], &comment.pubkey[comment.pubkey.len()-8..])
                                            } else {
                                                comment.pubkey.clone()
                                            };

                                            div.comment-card style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;" {
                                                div.comment-header style="display: flex; justify-content: space-between; margin-bottom: 0.75rem;" {
                                                    span.comment-author style="font-weight: 600; color: var(--accent-color, #0ea5e9);" { (commenter_pubkey) }
                                                    span.comment-time style="font-size: 0.875rem; color: var(--muted-color, #888);" { (format_relative_time(comment.created_at)) }
                                                }
                                                @if !comment.content.is_empty() {
                                                    div.comment-content style="white-space: pre-wrap;" {
                                                        (comment.content)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                form.claim-form
                                    hx-post={"/repo/" (identifier) "/issues/" (issue.id) "/comment"}
                                    hx-target=".comments-list"
                                    hx-swap="beforeend" {
                                    h3 { "Add Comment" }
                                    div.form-group {
                                        label for="comment_content" { "Comment" }
                                        textarea
                                            name="content"
                                            id="comment_content"
                                            placeholder="Write your comment..."
                                            rows="4"
                                            required {}
                                    }
                                    button.submit-button type="submit" { "Post Comment" }
                                }
                            }

                            @if !all_tags.is_empty() {
                                section.issue-section {
                                    h2 { "Tags" }
                                    div.tag-list {
                                        @for tag in all_tags {
                                            @if tag.len() >= 2 {
                                                @let tag_name = &tag[0];
                                                @let tag_value = &tag[1];
                                                @if !tag_name.is_empty() && !tag_value.is_empty() {
                                                    div.tag-item {
                                                        span.tag-name { (tag_name) }
                                                        span.tag-value { (tag_value) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "Event Details" }
                                div.event-details {
                                    div.event-detail-item {
                                        span.label { "Event ID:" }
                                        code { (issue.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (issue.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Pubkey:" }
                                        code { (issue.pubkey) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Signature:" }
                                        code.signature { (issue.sig) }
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

