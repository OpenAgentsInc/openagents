/// Pull request detail page
pub fn pull_request_detail_page(
    repository: &Event,
    pull_request: &Event,
    reviews: &[Event],
    reviewer_reputations: &std::collections::HashMap<String, i32>,
    status_events: &[Event],
    identifier: &str,
    trajectory_session: Option<&Event>,
    trajectory_events: &[Event],
    stack_prs: &[Event],
    dependency_pr: Option<&Event>,
    dependent_prs: &[Event],
    is_mergeable: bool,
    pr_updates: &[Event],
    diff_text: Option<&str>,
    inline_comments: &[crate::views::diff::InlineComment],
    bounties: &[Event],
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let pr_title = get_tag_value(pull_request, "subject")
        .unwrap_or_else(|| "Untitled Pull Request".to_string());
    let pr_status = get_tag_value(pull_request, "status").unwrap_or_else(|| "open".to_string());

    // Format pubkey for display
    let pr_author = if pull_request.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &pull_request.pubkey[..8],
            &pull_request.pubkey[pull_request.pubkey.len() - 8..]
        )
    } else {
        pull_request.pubkey.clone()
    };

    // Extract commit ID and clone URL
    let commit_id = get_tag_value(pull_request, "c");
    let clone_url = get_tag_value(pull_request, "clone");

    // Extract stack-related tags
    let depends_on = get_tag_value(pull_request, "depends_on");
    let stack = get_tag_value(pull_request, "stack");
    let layer = get_all_tag_values(pull_request, "layer");

    // Extract all tags for display
    let all_tags = &pull_request.tags;

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (pr_title) " - " (repo_name) " - GitAfter" }
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
                                    h1.issue-detail-title { (pr_title) }
                                    div.issue-detail-meta {
                                        span class={"issue-status " (pr_status)} {
                                            (pr_status)
                                        }
                                        span.issue-separator { "•" }
                                        span.issue-author { "by " (pr_author) }
                                        span.issue-separator { "•" }
                                        span.issue-time { (format_relative_time(pull_request.created_at)) }
                                    }
                                }
                                div.issue-detail-actions {
                                    a.back-link href={"/repo/" (identifier) "/pulls"} { "← Back to Pull Requests" }
                                }
                            }

                            section.issue-section {
                                h2 { "Repository Context" }
                                div.repo-context {
                                    a.repo-link href={"/repo/" (identifier)} { (repo_name) }
                                    span.repo-id-label { " (" (identifier) ")" }
                                }
                            }

                            @if !pull_request.content.is_empty() {
                                section.issue-section {
                                    h2 { "Description" }
                                    div.issue-content {
                                        p { (pull_request.content) }
                                    }
                                }
                            }

                            @if !bounties.is_empty() || stack.is_some() {
                                section.issue-section {
                                    h2 { "💰 Bounties" }

                                    @if bounties.is_empty() {
                                        p.empty-state { "No bounties for this layer yet." }
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
                                                @let bounty_stack = get_tag_value(bounty, "stack");
                                                @let bounty_layer = get_all_tag_values(bounty, "layer");

                                                div.bounty-card {
                                                    div.bounty-header {
                                                        @if let Some(amt) = amount {
                                                            span.bounty-amount { "⚡ " (amt) " sats" }
                                                        }
                                                        span.bounty-creator { "offered by " (bounty_creator) }
                                                    }
                                                    @if bounty_stack.is_some() && !bounty_layer.is_empty() && bounty_layer.len() >= 2 {
                                                        div.bounty-layer-info style="margin-top: 0.5rem; font-size: 0.9em; color: #888;" {
                                                            span { "📚 Layer " (bounty_layer[0]) " of " (bounty_layer[1]) }
                                                        }
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

                                    // Preview Later Layers section
                                    @if !dependent_prs.is_empty() {
                                        div style="margin-top: 16px;" {
                                            details {
                                                summary style="cursor: pointer; padding: 12px; background: #f0f9ff; border: 1px solid #bae6fd; font-weight: 600; user-select: none;" {
                                                    "🔍 Preview Later Layers (" (dependent_prs.len()) ")"
                                                }
                                                div style="margin-top: 8px; padding: 12px; background: #f9fafb; border: 1px solid #e5e7eb;" {
                                                    p style="margin: 0 0 12px 0; color: #6b7280; font-size: 0.875rem;" {
                                                        "These PRs build on top of this layer. Preview them to understand how your changes fit into the overall stack."
                                                    }
                                                    @for dep_pr in dependent_prs {
                                                        @let dep_title = get_tag_value(dep_pr, "subject").unwrap_or_else(|| "Untitled PR".to_string());
                                                        @let dep_status = get_tag_value(dep_pr, "status").unwrap_or_else(|| "open".to_string());
                                                        @let dep_layer = get_all_tag_values(dep_pr, "layer");
                                                        @let dep_status_emoji = match dep_status.as_str() {
                                                            "open" => "🟢",
                                                            "merged" | "applied" => "✅",
                                                            "closed" => "🔴",
                                                            "draft" => "📝",
                                                            _ => "❓"
                                                        };

                                                        div style="padding: 12px; margin-bottom: 8px; background: white; border: 1px solid #e5e7eb;" {
                                                            div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 8px;" {
                                                                div {
                                                                    @if !dep_layer.is_empty() && dep_layer.len() == 2 {
                                                                        span style="display: inline-block; padding: 2px 8px; background: #818cf8; color: white; font-size: 0.75rem; margin-right: 8px; font-weight: 600;" {
                                                                            "Layer " (dep_layer[0]) "/" (dep_layer[1])
                                                                        }
                                                                    }
                                                                    a href={"/repo/" (identifier) "/pulls/" (dep_pr.id)} style="font-weight: 600; color: #0ea5e9;" {
                                                                        (dep_title)
                                                                    }
                                                                }
                                                                span class={"issue-status " (dep_status)} style="font-size: 0.875rem;" {
                                                                    (dep_status_emoji) " " (dep_status)
                                                                }
                                                            }
                                                            @if !dep_pr.content.is_empty() {
                                                                p style="margin: 0; font-size: 0.875rem; color: #6b7280;" {
                                                                    (dep_pr.content)
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

                            section.issue-section {
                                h2 { "Status Management" }

                                @let current_status = if let Some(latest) = status_events.first() {
                                    match latest.kind {
                                        1630 => "Open",
                                        1631 => "Applied/Merged",
                                        1632 => "Closed",
                                        1633 => "Draft",
                                        _ => "Unknown"
                                    }
                                } else {
                                    &pr_status
                                };

                                div.status-current {
                                    h3 { "Current Status: " span class={"issue-status " (current_status.to_lowercase())} { (current_status) } }
                                }

                                form.claim-form
                                    hx-post={"/repo/" (identifier) "/pulls/" (pull_request.id) "/status"}
                                    hx-target="this"
                                    hx-swap="outerHTML" {
                                    h3 { "Change Status" }
                                    div.form-group {
                                        label for="status_select" { "New Status" }
                                        select name="status" id="status_select" {
                                            option value="open" { "🟢 Open (1630)" }
                                            option value="applied" { "✅ Applied/Merged (1631)" }
                                            option value="closed" { "🔴 Closed (1632)" }
                                            option value="draft" { "📝 Draft (1633)" }
                                        }
                                    }
                                    div.form-group {
                                        label for="status_reason" { "Reason (optional)" }
                                        textarea
                                            name="reason"
                                            id="status_reason"
                                            placeholder="Optional reason for status change..."
                                            rows="2" {}
                                    }
                                    button.submit-button type="submit" { "Update Status" }
                                }

                                @if !status_events.is_empty() {
                                    div.status-history {
                                        h3 { "Status History" }
                                        div.claims-list {
                                            @for status_event in status_events {
                                                @let status_name = match status_event.kind {
                                                    1630 => "🟢 Open",
                                                    1631 => "✅ Applied/Merged",
                                                    1632 => "🔴 Closed",
                                                    1633 => "📝 Draft",
                                                    _ => "❓ Unknown"
                                                };
                                                @let status_author = if status_event.pubkey.len() > 16 {
                                                    format!("{}...{}", &status_event.pubkey[..8], &status_event.pubkey[status_event.pubkey.len()-8..])
                                                } else {
                                                    status_event.pubkey.clone()
                                                };

                                                div.claim-card {
                                                    div.claim-header {
                                                        span.claim-author { (status_name) " by " (status_author) }
                                                        span.claim-time title={(status_event.created_at)} { (format_relative_time(status_event.created_at)) }
                                                    }
                                                    @if !status_event.content.is_empty() {
                                                        div.claim-content {
                                                            p { (status_event.content) }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            @if let Some(cid) = commit_id {
                                section.issue-section {
                                    h2 { "Commit Information" }
                                    div.event-details {
                                        div.event-detail-item {
                                            span.label { "Commit ID:" }
                                            code { (cid) }
                                        }
                                    }
                                }
                            }

                            @if let Some(curl) = clone_url {
                                section.issue-section {
                                    h2 { "Clone URL" }
                                    div.event-details {
                                        div.event-detail-item {
                                            code { (curl) }
                                        }
                                    }
                                }
                            }

                            @if depends_on.is_some() || stack.is_some() || !layer.is_empty() {
                                section.issue-section {
                                    h2 { "🔗 Stacked Diff Information" }

                                    @if !layer.is_empty() && layer.len() == 2 {
                                        div.stack-badge style="display: inline-block; padding: 8px 16px; background: #3b82f6; color: white; font-weight: bold; margin-bottom: 16px;" {
                                            "📊 Layer " (layer[0]) " of " (layer[1])
                                        }
                                    }

                                    // Visual dependency graph
                                    @if !stack_prs.is_empty() {
                                        div.dependency-graph style="margin-bottom: 24px; padding: 20px; background: #f9fafb; border: 1px solid #e5e7eb;" {
                                            h3 style="margin-bottom: 16px; font-size: 1rem; color: #374151;" { "Dependency Graph" }
                                            div.graph-container style="display: flex; flex-direction: column; gap: 8px;" {
                                                @let sorted_prs = {
                                                    let mut prs: Vec<_> = stack_prs.iter().collect();
                                                    prs.sort_by_key(|pr| {
                                                        let pr_layer = get_all_tag_values(pr, "layer");
                                                        if pr_layer.len() >= 2 {
                                                            pr_layer[0].parse::<i32>().unwrap_or(0)
                                                        } else {
                                                            0
                                                        }
                                                    });
                                                    prs
                                                };
                                                @for (idx, stack_pr) in sorted_prs.iter().enumerate() {
                                                    @let is_current = stack_pr.id == pull_request.id;
                                                    @let stack_pr_title = get_tag_value(stack_pr, "subject").unwrap_or_else(|| "Untitled PR".to_string());
                                                    @let stack_pr_status = get_tag_value(stack_pr, "status").unwrap_or_else(|| "unknown".to_string());
                                                    @let stack_pr_layer = get_all_tag_values(stack_pr, "layer");
                                                    @let layer_num = if stack_pr_layer.len() >= 2 { stack_pr_layer[0].clone() } else { "?".to_string() };
                                                    @let status_color = match stack_pr_status.as_str() {
                                                        "merged" | "applied" => "#10b981",
                                                        "closed" => "#ef4444",
                                                        "draft" => "#f59e0b",
                                                        _ => "#3b82f6"
                                                    };

                                                    div.graph-node style={"position: relative; display: flex; align-items: center; gap: 12px;"} {
                                                        // Connector line
                                                        @if idx > 0 {
                                                            div style="position: absolute; left: 15px; bottom: 100%; width: 2px; height: 8px; background: #d1d5db;" {}
                                                        }
                                                        // Node box (sharp corners per codebase convention)
                                                        div style={"width: 30px; height: 30px; background: " (status_color) "; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; font-size: 0.75rem; flex-shrink: 0; z-index: 1;"} {
                                                            (layer_num)
                                                        }
                                                        // PR card
                                                        div style={
                                                            "flex: 1; padding: 12px; background: white; border: 2px solid "
                                                            (if is_current { "#3b82f6" } else { "#e5e7eb" })
                                                            "; " (if is_current { "box-shadow: 0 0 0 3px #dbeafe;" } else { "" })
                                                        } {
                                                            a href={"/repo/" (identifier) "/pulls/" (stack_pr.id)} style={"font-weight: " (if is_current { "700" } else { "500" }) "; color: #111827; text-decoration: none;"} {
                                                                (stack_pr_title)
                                                            }
                                                            div style="margin-top: 4px; font-size: 0.875rem; color: #6b7280; display: flex; gap: 8px; align-items: center;" {
                                                                span style={"padding: 2px 8px; background: " (status_color) "22; color: " (status_color) "; font-size: 0.75rem; font-weight: 600;"} {
                                                                    (stack_pr_status)
                                                                }
                                                                @if is_current {
                                                                    span style="padding: 2px 8px; background: #3b82f6; color: white; font-size: 0.75rem; font-weight: 600;" {
                                                                        "YOU ARE HERE"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            div style="margin-top: 16px; padding: 12px; background: white; border-left: 4px solid #3b82f6; font-size: 0.875rem; color: #6b7280;" {
                                                "💡 Each PR in this stack builds on top of the previous one. PRs must be merged in order from bottom to top."
                                            }
                                        }
                                    }

                                    @if !is_mergeable && dependency_pr.is_some() {
                                        div.merge-warning style="padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b; margin-bottom: 16px;" {
                                            "⚠️ This PR cannot be merged until its dependencies are merged first."
                                        }
                                    } @else if is_mergeable && dependency_pr.is_some() {
                                        div.merge-info style="padding: 12px; background: #d1fae5; border-left: 4px solid #10b981; margin-bottom: 16px;" {
                                            "✅ All dependencies are satisfied. This PR is ready to merge."
                                        }
                                    }

                                    div.event-details {
                                        @if let Some(dep_pr) = dependency_pr {
                                            div.event-detail-item style="margin-bottom: 16px;" {
                                                span.label { "⬆️ Depends On:" }
                                                div style="margin-top: 8px; padding: 12px; background: #f3f4f6;" {
                                                    @let dep_title = get_tag_value(dep_pr, "subject").unwrap_or_else(|| "Untitled PR".to_string());
                                                    @let dep_status = get_tag_value(dep_pr, "status").unwrap_or_else(|| "unknown".to_string());
                                                    @let dep_status_emoji = match dep_status.as_str() {
                                                        "open" => "🟢",
                                                        "merged" | "applied" => "✅",
                                                        "closed" => "🔴",
                                                        "draft" => "📝",
                                                        _ => "❓"
                                                    };
                                                    div {
                                                        a.repo-link href={"/repo/" (identifier) "/pulls/" (dep_pr.id)} style="font-weight: 600;" {
                                                            (dep_status_emoji) " " (dep_title)
                                                        }
                                                    }
                                                    div style="margin-top: 4px; font-size: 0.875rem; color: #6b7280;" {
                                                        "Status: " span class={"issue-status " (dep_status)} { (dep_status) }
                                                    }
                                                }
                                            }
                                        }

                                        @if let Some(ref s) = stack {
                                            @if !stack_prs.is_empty() {
                                                div.event-detail-item style="margin-top: 16px;" {
                                                    span.label { "📚 Stack Group (" (stack_prs.len()) " PRs)" }
                                                    div style="margin-top: 12px;" {
                                                        @for stack_pr in stack_prs {
                                                            @let is_current = stack_pr.id == pull_request.id;
                                                            @let stack_pr_title = get_tag_value(stack_pr, "subject").unwrap_or_else(|| "Untitled PR".to_string());
                                                            @let stack_pr_status = get_tag_value(stack_pr, "status").unwrap_or_else(|| "unknown".to_string());
                                                            @let stack_pr_layer = get_all_tag_values(stack_pr, "layer");
                                                            @let stack_status_emoji = match stack_pr_status.as_str() {
                                                                "open" => "🟢",
                                                                "merged" | "applied" => "✅",
                                                                "closed" => "🔴",
                                                                "draft" => "📝",
                                                                _ => "❓"
                                                            };

                                                            div style={
                                                                "padding: 12px; margin-bottom: 8px; background: "
                                                                (if is_current { "#dbeafe" } else { "#f9fafb" })
                                                                "; border-left: 4px solid "
                                                                (if is_current { "#3b82f6" } else { "#e5e7eb" })
                                                                ";"
                                                            } {
                                                                div style="display: flex; justify-content: space-between; align-items: center;" {
                                                                    div {
                                                                        @if !stack_pr_layer.is_empty() && stack_pr_layer.len() == 2 {
                                                                            span style="display: inline-block; padding: 2px 8px; background: #6366f1; color: white; font-size: 0.75rem; margin-right: 8px;" {
                                                                                "L" (stack_pr_layer[0])
                                                                            }
                                                                        }
                                                                        @if is_current {
                                                                            span style="font-weight: 700; color: #1e40af;" {
                                                                                (stack_status_emoji) " " (stack_pr_title) " (current)"
                                                                            }
                                                                        } @else {
                                                                            a.repo-link href={"/repo/" (identifier) "/pulls/" (stack_pr.id)} {
                                                                                (stack_status_emoji) " " (stack_pr_title)
                                                                            }
                                                                        }
                                                                    }
                                                                    span class={"issue-status " (stack_pr_status)} style="font-size: 0.875rem;" {
                                                                        (stack_pr_status)
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                div.event-detail-item style="margin-top: 12px; font-size: 0.875rem; color: #6b7280;" {
                                                    span.label { "Stack ID:" }
                                                    code style="font-size: 0.75rem;" { (s) }
                                                }
                                            } @else {
                                                div.event-detail-item {
                                                    span.label { "Stack ID:" }
                                                    code { (s) }
                                                }
                                            }
                                        }
                                    }

                                    div style="margin-top: 16px; padding: 12px; background: #eff6ff; font-size: 0.875rem;" {
                                        p style="margin: 0;" {
                                            "💡 " strong { "Stacked Diffs: " }
                                            "This PR is part of a stack of smaller, reviewable changes. Each layer must be merged in order to maintain the dependency chain."
                                        }
                                    }

                                    // Stack Review Context: Show what PRs depend on this one
                                    @if !stack_prs.is_empty() {
                                        @let dependents: Vec<&Event> = stack_prs.iter()
                                            .filter(|pr| {
                                                pr.tags.iter().any(|tag| {
                                                    tag.len() >= 2 && tag[0] == "depends_on" && tag[1] == pull_request.id
                                                })
                                            })
                                            .collect();

                                        @if !dependents.is_empty() {
                                            div style="margin-top: 16px; padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b;" {
                                                p style="margin: 0 0 8px 0; font-weight: 600;" { "⚠️ Review Context:" }
                                                p style="margin: 0; font-size: 0.875rem;" {
                                                    (dependents.len()) " PR(s) depend on this layer. Changes here may affect:"
                                                }
                                                ul style="margin: 8px 0 0 1.5rem; padding: 0; font-size: 0.875rem;" {
                                                    @for dep in dependents.iter().take(3) {
                                                        @let dep_title = get_tag_value(dep, "subject").unwrap_or_else(|| "Untitled PR".to_string());
                                                        @let dep_layer = get_all_tag_values(dep, "layer");
                                                        li {
                                                            a href={"/repo/" (identifier) "/pulls/" (dep.id)} style="color: #0ea5e9;" {
                                                                @if !dep_layer.is_empty() && dep_layer.len() == 2 {
                                                                    "Layer " (dep_layer[0]) ": "
                                                                }
                                                                (dep_title)
                                                            }
                                                        }
                                                    }
                                                    @if dependents.len() > 3 {
                                                        li { "... and " (dependents.len() - 3) " more" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            section.issue-section {
                                h2 { "Reviews" }
                                @if reviews.is_empty() {
                                    p.empty-state { "No reviews yet. Be the first to review this PR!" }
                                } @else {
                                    @let (weighted_score, total_weight) = {
                                        let mut ws = 0.0;
                                        let mut tw = 0.0;
                                        for review in reviews {
                                            let review_type = get_tag_value(review, "review_type").unwrap_or_else(|| "comment".to_string());
                                            let base_score = match review_type.as_str() {
                                                "approve" => 1.0,
                                                "request_changes" => -1.0,
                                                _ => 0.0
                                            };
                                            if base_score != 0.0 {
                                                let reputation = reviewer_reputations.get(&review.pubkey).copied().unwrap_or(0);
                                                let weight = calculate_review_weight(reputation);
                                                ws += base_score * weight;
                                                tw += weight;
                                            }
                                        }
                                        (ws, tw)
                                    };
                                    @let avg_weighted_score = if total_weight > 0.0 { weighted_score / total_weight } else { 0.0 };

                                    @if total_weight > 0.0 {
                                        div style="background: #1c1f26; padding: 1rem; margin-bottom: 1rem; border-left: 4px solid #3b82f6;" {
                                            h3 style="margin: 0 0 0.5rem 0; font-size: 1rem;" { "Weighted Review Score" }
                                            @let score_color = if avg_weighted_score > 0.3 {
                                                "#22c55e"
                                            } else if avg_weighted_score < -0.3 {
                                                "#ef4444"
                                            } else {
                                                "#f59e0b"
                                            };
                                            @let score_emoji = if avg_weighted_score > 0.3 {
                                                "✅"
                                            } else if avg_weighted_score < -0.3 {
                                                "🔴"
                                            } else {
                                                "⚠️"
                                            };
                                            div style={"font-size: 1.5rem; font-weight: 700; color: " (score_color)} {
                                                (score_emoji) " " (format!("{:.2}", avg_weighted_score))
                                            }
                                            p style="margin: 0.5rem 0 0 0; font-size: 0.875rem; color: #9ca3af;" {
                                                "Score calculated from " (reviews.len()) " review(s) weighted by reputation"
                                            }
                                        }
                                    }

                                    @let sorted_reviews = {
                                        let mut sorted: Vec<_> = reviews.iter().collect();
                                        sorted.sort_by(|a, b| {
                                            let rep_a = reviewer_reputations.get(&a.pubkey).copied().unwrap_or(0);
                                            let rep_b = reviewer_reputations.get(&b.pubkey).copied().unwrap_or(0);
                                            rep_b.cmp(&rep_a)
                                        });
                                        sorted
                                    };

                                    div.claims-list {
                                        @for review in sorted_reviews {
                                            @let reviewer_pubkey = if review.pubkey.len() > 16 {
                                                format!("{}...{}", &review.pubkey[..8], &review.pubkey[review.pubkey.len()-8..])
                                            } else {
                                                review.pubkey.clone()
                                            };
                                            @let review_type = get_tag_value(review, "review_type").unwrap_or_else(|| "comment".to_string());
                                            @let review_emoji = match review_type.as_str() {
                                                "approve" => "✅",
                                                "request_changes" => "🔴",
                                                _ => "💬"
                                            };
                                            @let reputation = reviewer_reputations.get(&review.pubkey).copied().unwrap_or(0);
                                            @let tier = ReputationTier::from_score(reputation);
                                            @let weight = calculate_review_weight(reputation);
                                            @let tier_color = tier.color();
                                            @let tier_emoji = tier.emoji();
                                            @let tier_name = tier.name();
                                            @let card_style = format!("background: linear-gradient(135deg, #1c1f26 0%, #2a2f3a 100{}); border-left: 3px solid {}", "%", tier_color);

                                            div.claim-card style=(card_style) {
                                                div.claim-header {
                                                    span.claim-author {
                                                        (review_emoji) " " (reviewer_pubkey)
                                                        span style={"color: " (tier_color) "; margin-left: 0.5rem;"} {
                                                            (tier_emoji) " " (tier_name) " (" (reputation) ")"
                                                        }
                                                        span style="color: #6b7280; margin-left: 0.5rem; font-size: 0.875rem;" {
                                                            "• Weight: " (format!("{:.0}x", weight))
                                                        }
                                                        @if !layer.is_empty() && layer.len() >= 2 && stack.is_some() {
                                                            span.layer-badge style="background: #3b82f6; color: white; padding: 0.125rem 0.5rem; margin-left: 0.5rem; font-size: 0.75rem; font-weight: 600;" {
                                                                "📚 Layer " (layer[0]) "/" (layer[1])
                                                            }
                                                        }
                                                    }
                                                    span.claim-time title={(review.created_at)} { (format_relative_time(review.created_at)) }
                                                }
                                                @if !review.content.is_empty() {
                                                    div.claim-content {
                                                        p { (review.content) }
                                                    }
                                                }
                                                @if review_type != "comment" {
                                                    div.claim-estimate {
                                                        span.label { "Review: " }
                                                        span { (review_type) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                form.claim-form
                                    hx-post={"/repo/" (identifier) "/pulls/" (pull_request.id) "/review"}
                                    hx-target="this"
                                    hx-swap="outerHTML" {
                                    h3 { "Submit Review" }
                                    div.form-group {
                                        label for="review_type" { "Review Type" }
                                        select name="review_type" id="review_type" {
                                            option value="comment" { "💬 Comment" }
                                            option value="approve" { "✅ Approve" }
                                            option value="request_changes" { "🔴 Request Changes" }
                                        }
                                    }
                                    div.form-group {
                                        label for="review_content" { "Comment" }
                                        textarea
                                            name="content"
                                            id="review_content"
                                            placeholder="Leave your review comments here..."
                                            rows="4"
                                            required {}
                                    }
                                    button.submit-button type="submit" { "Submit Review" }
                                }
                            }

                            // Automated review checklist
                            (review_checklist_component(identifier, &pull_request.id))

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

                            @if let Some(session) = trajectory_session {
                                section.issue-section {
                                    h2 { "🔍 Agent Trajectory" }

                                    @let session_metadata = serde_json::from_str::<serde_json::Value>(&session.content).ok();
                                    @let trajectory_hash = session_metadata
                                        .as_ref()
                                        .and_then(|m| m.get("trajectory_hash"))
                                        .and_then(|h| h.as_str());
                                    @let session_description = session_metadata
                                        .as_ref()
                                        .and_then(|m| m.get("description"))
                                        .and_then(|d| d.as_str());
                                    @let participants = session_metadata
                                        .as_ref()
                                        .and_then(|m| m.get("participants"))
                                        .and_then(|p| p.as_array());

                                    // Perform trajectory-to-diff comparison if we have both trajectory events and diff
                                    @let comparison_result = if !trajectory_events.is_empty() && diff_text.is_some() {
                                        use crate::trajectory::{parse_trajectory_events, compare_trajectory_to_diff};

                                        // Extract trajectory events JSON
                                        let events_json: Vec<String> = trajectory_events.iter()
                                            .map(|e| e.content.clone())
                                            .collect();

                                        // Parse trajectory and compare to diff
                                        parse_trajectory_events(&events_json)
                                            .and_then(|mods| compare_trajectory_to_diff(&mods, diff_text.unwrap()))
                                            .ok()
                                    } else {
                                        None
                                    };

                                    div.trajectory-summary style="background: #1e293b; padding: 1.5rem; margin-bottom: 1.5rem; border-left: 4px solid #0ea5e9;" {
                                        div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 1rem;" {
                                            div {
                                                h3 style="margin: 0 0 0.5rem 0; color: #0ea5e9;" { "Trajectory Session" }
                                                @if let Some(desc) = session_description {
                                                    p style="margin: 0; color: #cbd5e1;" { (desc) }
                                                }
                                            }
                                            a href={"/trajectory/" (session.id)} style="padding: 8px 16px; background: #0ea5e9; color: white; text-decoration: none; font-weight: 600;" {
                                                "View Full Trajectory →"
                                            }
                                        }

                                        div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-top: 1rem;" {
                                            div {
                                                p style="margin: 0; font-size: 0.875rem; color: #94a3b8;" { "Session ID" }
                                                code style="font-size: 0.875rem;" { (session.id) }
                                            }
                                            div {
                                                p style="margin: 0; font-size: 0.875rem; color: #94a3b8;" { "Started" }
                                                span style="font-size: 0.875rem;" title={(session.created_at)} { (format_relative_time(session.created_at)) }
                                            }
                                            div {
                                                p style="margin: 0; font-size: 0.875rem; color: #94a3b8;" { "Events" }
                                                span style="font-size: 0.875rem; font-weight: 600; color: #10b981;" { (trajectory_events.len()) }
                                            }
                                            @if let Some(hash) = trajectory_hash {
                                                div {
                                                    p style="margin: 0; font-size: 0.875rem; color: #94a3b8;" { "Hash" }
                                                    code style="font-size: 0.75rem;" { (hash) }
                                                }
                                            }
                                        }

                                        @if let Some(parts) = participants {
                                            div style="margin-top: 1rem;" {
                                                p style="margin: 0 0 0.5rem 0; font-size: 0.875rem; color: #94a3b8;" { "Participants" }
                                                div style="display: flex; gap: 0.5rem; flex-wrap: wrap;" {
                                                    @for participant in parts {
                                                        @if let Some(p) = participant.as_str() {
                                                            span style="padding: 4px 8px; background: #334155; font-size: 0.875rem;" {
                                                                (p)
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        @if trajectory_events.is_empty() {
                                            div style="margin-top: 1rem; padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b;" {
                                                "⚠️ Warning: No trajectory events found"
                                            }
                                        } @else if trajectory_hash.is_none() {
                                            div style="margin-top: 1rem; padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b;" {
                                                "⚠️ Warning: No trajectory hash in session - integrity cannot be verified"
                                            }
                                        }

                                        // Display trajectory-to-diff comparison result
                                        @if let Some(result) = &comparison_result {
                                            @if result.status == MatchStatus::FullMatch {
                                                div style="margin-top: 1rem; padding: 12px; background: #d1fae5; border-left: 4px solid #10b981; color: #065f46;" {
                                                    "✅ Diff Verification: All " (result.matched_files.len()) " modified file(s) match trajectory tool calls"
                                                }
                                            } @else if result.status == MatchStatus::MinorDiscrepancy {
                                                div style="margin-top: 1rem; padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b; color: #92400e;" {
                                                    "⚠️ Diff Verification: Minor discrepancies detected"
                                                    @if !result.extra_in_diff.is_empty() {
                                                        p style="margin-top: 0.5rem; font-size: 0.875rem;" {
                                                            "Files in diff but not in trajectory: " (result.extra_in_diff.join(", "))
                                                        }
                                                    }
                                                }
                                            } @else {
                                                div style="margin-top: 1rem; padding: 12px; background: #fee2e2; border-left: 4px solid #ef4444; color: #991b1b;" {
                                                    "❌ Diff Verification: Major discrepancies detected"
                                                    @if !result.missing_in_diff.is_empty() {
                                                        p style="margin-top: 0.5rem; font-size: 0.875rem;" {
                                                            "Files in trajectory but not in diff: " (result.missing_in_diff.join(", "))
                                                        }
                                                    }
                                                    @if !result.extra_in_diff.is_empty() {
                                                        p style="margin-top: 0.5rem; font-size: 0.875rem;" {
                                                            "Files in diff but not in trajectory: " (result.extra_in_diff.join(", "))
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    @if !trajectory_events.is_empty() {
                                        details open {
                                            summary style="cursor: pointer; padding: 12px; background: #334155; margin-bottom: 1rem; font-weight: 600; font-size: 1.1rem;" {
                                                "📜 Event Timeline (" (trajectory_events.len()) " events)"
                                            }

                                            div style="position: relative; padding-left: 2rem; border-left: 2px solid #475569;" {
                                                @for event in trajectory_events {
                                                    @let step_type = get_tag_value(event, "step").unwrap_or_else(|| "Unknown".to_string());
                                                    @let seq = get_tag_value(event, "seq").unwrap_or_else(|| "?".to_string());

                                                    @let event_data = serde_json::from_str::<serde_json::Value>(&event.content).ok();
                                                    @let tool_name = event_data.as_ref()
                                                        .and_then(|d| d.get("tool"))
                                                        .and_then(|t| t.as_str());

                                                    @let (badge_bg, badge_icon, badge_text) = if step_type == "ToolUse" {
                                                        ("#3b82f6", "🔧", tool_name.unwrap_or("Tool Use"))
                                                    } else if step_type == "ToolResult" {
                                                        ("#10b981", "✅", tool_name.unwrap_or("Tool Result"))
                                                    } else if step_type == "Thinking" {
                                                        ("#8b5cf6", "💭", "Thinking")
                                                    } else if step_type == "Message" {
                                                        ("#6366f1", "💬", "Message")
                                                    } else {
                                                        ("#64748b", "📝", step_type.as_str())
                                                    };

                                                    div style={"position: relative; margin-bottom: 1rem; margin-left: -0.5rem; padding-left: 1.5rem;"} {
                                                        // Timeline dot
                                                        div style={"position: absolute; left: -6px; top: 8px; width: 12px; height: 12px; background: " (badge_bg) "; border: 2px solid #0f172a;"} {}

                                                        div style={"padding: 1rem; background: #1e293b; border-left: 3px solid " (badge_bg) ";"} {
                                                            div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem;" {
                                                                div style="display: flex; align-items: center; gap: 0.75rem;" {
                                                                    span style={"padding: 4px 10px; background: " (badge_bg) "; color: white; font-size: 0.875rem; font-weight: 600;"} {
                                                                        (badge_icon) " " (badge_text)
                                                                    }
                                                                    span style="font-size: 0.875rem; color: #94a3b8;" {
                                                                        "#" (seq)
                                                                    }
                                                                }
                                                                span style="font-size: 0.8rem; color: #64748b;" title={(event.created_at)} {
                                                                    (format_relative_time(event.created_at))
                                                                }
                                                            }

                                                            @if step_type == "Thinking" {
                                                                @if let Some(data) = event_data.as_ref() {
                                                                    @if let Some(hash) = data.get("hash").and_then(|h| h.as_str()) {
                                                                        p style="font-size: 0.875rem; color: #94a3b8; margin: 0;" {
                                                                            "🔒 Content redacted (hash: " code style="font-size: 0.75rem;" { (hash) } ")"
                                                                        }
                                                                    } @else {
                                                                        p style="font-size: 0.875rem; color: #94a3b8; margin: 0;" { "🔒 Content redacted" }
                                                                    }
                                                                }
                                                            } @else if !event.content.is_empty() {
                                                                details {
                                                                    summary style="cursor: pointer; color: #0ea5e9; font-size: 0.875rem;" { "▶ View event data" }
                                                                    div style="margin-top: 0.75rem; padding: 0.75rem; background: #0f172a; font-family: monospace; font-size: 0.8rem; white-space: pre-wrap; overflow-x: auto; max-height: 400px; overflow-y: auto;" {
                                                                        (event.content)
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        div style="margin-top: 1rem; padding: 12px; background: #eff6ff; font-size: 0.875rem;" {
                                            p style="margin: 0;" {
                                                "💡 " strong { "Trajectory Transparency: " }
                                                "This timeline shows every step the agent took to create this PR. Reviewers can verify the agent's reasoning and catch any suspicious behavior."
                                            }
                                        }
                                    }
                                }
                            }

                            @if let Some(diff) = diff_text {
                                section.issue-section {
                                    h2 { "📄 Pull Request Diff" }

                                    div style="margin-bottom: 1rem; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem;" {
                                        p style="margin: 0; color: #9ca3af;" {
                                            "Showing changes from this pull request"
                                        }
                                        div style="display: flex; gap: 0.5rem;" {
                                            // Preview later layers button
                                            @if !stack_prs.is_empty() && !layer.is_empty() && layer.len() == 2 {
                                                @if let Ok(current_layer) = layer[0].parse::<u32>() {
                                                    @let later_layers: Vec<&Event> = stack_prs.iter()
                                                        .filter(|pr| {
                                                            if let Some(pr_layer) = get_all_tag_values(pr, "layer").get(0) {
                                                                if let Ok(pr_layer_num) = pr_layer.parse::<u32>() {
                                                                    return pr_layer_num > current_layer;
                                                                }
                                                            }
                                                            false
                                                        })
                                                        .collect();

                                                    @if !later_layers.is_empty() {
                                                        details style="padding: 8px 16px; background: #f3f4f6; border: 1px solid #d1d5db;" {
                                                            summary style="cursor: pointer; font-weight: 600; color: #374151;" {
                                                                "👁️ Preview Later Layers (" (later_layers.len()) ")"
                                                            }
                                                            div style="margin-top: 8px; padding: 8px; background: white;" {
                                                                @for later_pr in later_layers.iter().take(5) {
                                                                    @let later_title = get_tag_value(later_pr, "subject").unwrap_or_else(|| "Untitled".to_string());
                                                                    @let later_layer = get_all_tag_values(later_pr, "layer");
                                                                    div style="margin-bottom: 4px;" {
                                                                        a href={"/repo/" (identifier) "/pulls/" (later_pr.id)} style="color: #0ea5e9; font-size: 0.875rem;" {
                                                                            @if !later_layer.is_empty() && later_layer.len() == 2 {
                                                                                "L" (later_layer[0]) ": "
                                                                            }
                                                                            (later_title)
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            a href={"data:text/plain;charset=utf-8," (urlencoding::encode(diff))}
                                               download={(pr_title.clone()) ".patch"}
                                               style="padding: 8px 16px; background: #3b82f6; color: white; text-decoration: none; font-weight: 600;" {
                                                "⬇️ Download Patch"
                                            }
                                        }
                                    }

                                    (crate::views::diff::render_diff_optimized(
                                        diff,
                                        inline_comments,
                                        &pull_request.id,
                                        identifier,
                                        crate::views::diff::DiffRenderConfig::default(),
                                    ))
                                }
                            } @else {
                                section.issue-section {
                                    h2 { "📄 Pull Request Diff" }
                                    div.empty-state style="padding: 2rem; background: #1e293b; text-align: center;" {
                                        p style="margin: 0 0 1rem 0; color: #94a3b8;" {
                                            "Diff not available. The repository needs to be cloned locally to view the diff."
                                        }
                                        @let clone_url = get_tag_value(pull_request, "clone");
                                        @if let Some(url) = clone_url {
                                            div style="margin-top: 1rem;" {
                                                p style="margin: 0 0 0.5rem 0; font-weight: 600; color: #e2e8f0;" { "Clone this repository:" }
                                                code style="padding: 8px 12px; background: #0f172a; display: inline-block;" {
                                                    "git clone " (url)
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
                                        code { (pull_request.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (pull_request.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Pubkey:" }
                                        code { (pull_request.pubkey) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Signature:" }
                                        code.signature { (pull_request.sig) }
                                    }
                                }
                            }

                            @if !pr_updates.is_empty() {
                                section.issue-section {
                                    h2 { "📝 Pull Request Updates (" (pr_updates.len()) ")" }
                                    div style="display: flex; flex-direction: column; gap: 1rem;" {
                                        @for update in pr_updates {
                                            @let update_author = if update.pubkey.len() > 16 {
                                                format!("{}...{}", &update.pubkey[..8], &update.pubkey[update.pubkey.len()-8..])
                                            } else {
                                                update.pubkey.clone()
                                            };

                                            div style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;" {
                                                div style="display: flex; justify-content: space-between; margin-bottom: 0.5rem;" {
                                                    span style="font-weight: 600; color: var(--accent-color, #0ea5e9);" { (update_author) }
                                                    span style="font-size: 0.875rem; color: var(--muted-color, #888);" title={(update.created_at)} { (format_relative_time(update.created_at)) }
                                                }
                                                @if !update.content.is_empty() {
                                                    div style="white-space: pre-wrap; color: var(--text-color, #ccc);" {
                                                        (update.content)
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

