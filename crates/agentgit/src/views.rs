//! Maud view templates for AgentGit

pub mod publish_status;

use maud::{html, Markup, DOCTYPE};
use nostr::Event;

#[allow(unused_imports)]
pub use publish_status::{publish_status_notification, publish_status_styles};

/// Helper function to extract tag value from event
fn get_tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event.tags.iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

/// Helper function to extract all values for a tag name
fn get_all_tag_values(event: &Event, tag_name: &str) -> Vec<String> {
    event.tags.iter()
        .filter(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

/// Render a single repository card
fn repository_card(event: &Event) -> Markup {
    let name = get_tag_value(event, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(event, "description").unwrap_or_default();
    let identifier = get_tag_value(event, "d").unwrap_or_default();
    let has_clone_url = get_tag_value(event, "clone").is_some();
    let has_web_url = get_tag_value(event, "web").is_some();

    // Truncate pubkey for display
    let short_pubkey = if event.pubkey.len() > 16 {
        format!("{}...{}", &event.pubkey[..8], &event.pubkey[event.pubkey.len()-8..])
    } else {
        event.pubkey.clone()
    };

    html! {
        a.repo-card href={"/repo/" (identifier)} {
            div.repo-header {
                h3.repo-name { (name) }
                span.repo-id { "d:" (identifier) }
            }
            @if !description.is_empty() {
                p.repo-description { (description) }
            }
            div.repo-meta {
                span.repo-author { "by " (short_pubkey) }
                @if has_clone_url {
                    span.repo-clone { "Clone" }
                }
                @if has_web_url {
                    span.repo-web { "View" }
                }
            }
        }
    }
}

/// Home page with repository list
pub fn home_page_with_repos(repositories: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "AgentGit - Nostr GitHub Alternative" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" class="active" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" {
                            h2 { "Repositories (" (repositories.len()) ")" }
                            a.submit-button href="/repo/new" style="text-decoration: none; padding: 10px 20px;" { "+ Create Repository" }
                        }
                        @if repositories.is_empty() {
                            p.placeholder { "No repositories found. Listening for NIP-34 events..." }
                        } @else {
                            div.repo-list {
                                @for repo in repositories {
                                    (repository_card(repo))
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

/// Repository detail page
pub fn repository_detail_page(repository: &Event, is_cloned: bool, local_path: Option<String>) -> Markup {
    let name = get_tag_value(repository, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(repository, "description").unwrap_or_default();
    let identifier = get_tag_value(repository, "d").unwrap_or_default();
    let clone_urls = get_all_tag_values(repository, "clone");
    let web_url = get_tag_value(repository, "web");
    let maintainers = get_all_tag_values(repository, "p");

    // Format pubkey for display
    let owner_pubkey = if repository.pubkey.len() > 16 {
        format!("{}...{}", &repository.pubkey[..8], &repository.pubkey[repository.pubkey.len()-8..])
    } else {
        repository.pubkey.clone()
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.repo-detail {
                            div.repo-detail-header {
                                div {
                                    h1.repo-detail-name { (name) }
                                    p.repo-detail-identifier { "Identifier: " (identifier) }
                                }
                                a.back-link href="/" { "← Back to Repositories" }
                            }

                            @if !description.is_empty() {
                                section.repo-section {
                                    h2 { "Description" }
                                    p.repo-detail-description { (description) }
                                }
                            }

                            section.repo-section {
                                h2 { "Navigation" }
                                div.repo-nav-links {
                                    a.nav-link href={"/repo/" (identifier) "/issues"} { "View Issues" }
                                    a.nav-link href={"/repo/" (identifier) "/patches"} { "View Patches" }
                                    a.nav-link href={"/repo/" (identifier) "/pulls"} { "View Pull Requests" }
                                }
                            }

                            section.repo-section {
                                h2 { "Contribute" }
                                div.repo-nav-links {
                                    a.nav-link href={"/repo/" (identifier) "/pulls/new"} { "+ Create Pull Request" }
                                    a.nav-link href={"/repo/" (identifier) "/patches/new"} { "+ Create Patch" }
                                }
                            }

                            section.repo-section {
                                h2 { "Owner" }
                                div.repo-owner {
                                    span.pubkey { (owner_pubkey) }
                                }
                            }

                            @if !maintainers.is_empty() {
                                section.repo-section {
                                    h2 { "Maintainers" }
                                    div.maintainer-list {
                                        @for maintainer in maintainers {
                                            @let short_maintainer = if maintainer.len() > 16 {
                                                format!("{}...{}", &maintainer[..8], &maintainer[maintainer.len()-8..])
                                            } else {
                                                maintainer.clone()
                                            };
                                            div.maintainer-item {
                                                span.pubkey { (short_maintainer) }
                                            }
                                        }
                                    }
                                }
                            }

                            @if !clone_urls.is_empty() {
                                section.repo-section {
                                    h2 { "Clone URLs" }
                                    div.clone-urls {
                                        @for url in &clone_urls {
                                            @let onclick_code = format!("navigator.clipboard.writeText('{}')", url);
                                            div.clone-url-item {
                                                code { (url) }
                                                button.copy-btn onclick=(onclick_code) { "Copy" }
                                            }
                                        }
                                    }
                                }
                            }

                            section.repo-section {
                                h2 { "Local Clone" }
                                @if is_cloned {
                                    @if let Some(path) = local_path {
                                        div.clone-status {
                                            p { "✅ Repository cloned locally" }
                                            div.clone-path {
                                                code { (path) }
                                            }
                                        }
                                    }
                                } @else {
                                    @if !clone_urls.is_empty() {
                                        div.clone-action {
                                            p { "Clone this repository to your local workspace for development" }
                                            form
                                                hx-post={"/repo/" (identifier) "/clone"}
                                                hx-target="#clone-result"
                                            {
                                                button type="submit" { "Clone Repository" }
                                            }
                                            div id="clone-result" {}
                                        }
                                    } @else {
                                        p.placeholder { "No clone URLs available for this repository" }
                                    }
                                }
                            }

                            @if let Some(url) = web_url {
                                section.repo-section {
                                    h2 { "Web Interface" }
                                    a.web-link href=(url) target="_blank" { (url) }
                                }
                            }

                            section.repo-section {
                                h2 { "Event Details" }
                                div.event-details {
                                    div.event-detail-item {
                                        span.label { "Event ID:" }
                                        code { (repository.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (repository.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Created:" }
                                        span { (repository.created_at) }
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

/// Issues list page for a repository
pub fn issues_list_page(repository: &Event, issues: &[Event], is_watched: bool, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Issues - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.issues-container {
                            div.issues-header {
                                div {
                                    h1.issues-title { (repo_name) " - Issues" }
                                    p.issues-subtitle { "Viewing issues for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    @if is_watched {
                                        form
                                            hx-post={"/repo/" (identifier) "/unwatch"}
                                            hx-target="this"
                                            hx-swap="outerHTML"
                                            style="display: inline;" {
                                            button.watch-button type="submit" { "⭐ Unwatch" }
                                        }
                                    } @else {
                                        form
                                            hx-post={"/repo/" (identifier) "/watch"}
                                            hx-target="this"
                                            hx-swap="outerHTML"
                                            style="display: inline;" {
                                            button.watch-button type="submit" { "☆ Watch" }
                                        }
                                    }
                                    a.nav-link href={"/repo/" (identifier) "/issues/new"} { "+ New Issue" }
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            // Filter controls
                            div.filter-controls style="margin: 1.5rem 0; padding: 1rem; background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333);" {
                                form
                                    hx-get={"/repo/" (identifier) "/issues"}
                                    hx-target=".issues-container"
                                    hx-swap="outerHTML"
                                    style="display: flex; gap: 1rem; align-items: center; flex-wrap: wrap;" {

                                    span style="font-weight: 600;" { "Filter:" }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        input type="checkbox" name="filter_open" value="true" checked;
                                        span { "Open" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        input type="checkbox" name="filter_closed" value="true";
                                        span { "Closed" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        input type="checkbox" name="filter_has_bounty" value="true";
                                        span { "Has Bounty" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        input type="checkbox" name="filter_claimed" value="true";
                                        span { "Claimed" }
                                    }

                                    button.btn-secondary type="submit" style="margin-left: auto; padding: 0.5rem 1rem;" { "Apply Filters" }
                                }
                            }

                            @if issues.is_empty() {
                                div.empty-state {
                                    p { "No issues found for this repository." }
                                    p.info-text { "Issues will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.issues-count {
                                    span { (issues.len()) " issue" @if issues.len() != 1 { "s" } " found" }
                                }

                                div.issues-list {
                                    @for issue in issues {
                                        @let issue_title = get_tag_value(issue, "subject")
                                            .unwrap_or_else(|| "Untitled Issue".to_string());
                                        @let issue_status = get_tag_value(issue, "status")
                                            .unwrap_or_else(|| "open".to_string());
                                        @let issue_author = if issue.pubkey.len() > 16 {
                                            format!("{}...{}", &issue.pubkey[..8], &issue.pubkey[issue.pubkey.len()-8..])
                                        } else {
                                            issue.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/issues/" (issue.id)} {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (issue_title) }
                                                    span class={"issue-status " (issue_status)} {
                                                        (issue_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (issue_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { "Created " (issue.created_at) }
                                                }
                                            }
                                            @if !issue.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if issue.content.len() > 200 {
                                                        format!("{}...", &issue.content[..200])
                                                    } else {
                                                        issue.content.clone()
                                                    };
                                                    p { (preview) }
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
/// Issue detail page
pub fn issue_detail_page(repository: &Event, issue: &Event, claims: &[Event], bounties: &[Event], comments: &[Event], identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let issue_title = get_tag_value(issue, "subject").unwrap_or_else(|| "Untitled Issue".to_string());
    let issue_status = get_tag_value(issue, "status").unwrap_or_else(|| "open".to_string());

    // Format pubkey for display
    let issue_author = if issue.pubkey.len() > 16 {
        format!("{}...{}", &issue.pubkey[..8], &issue.pubkey[issue.pubkey.len()-8..])
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
                title { (issue_title) " - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                        span.issue-time { "Created " (issue.created_at) }
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
                                    div.claims-list {
                                        @for claim in claims {
                                            @let claimer_pubkey = if claim.pubkey.len() > 16 {
                                                format!("{}...{}", &claim.pubkey[..8], &claim.pubkey[claim.pubkey.len()-8..])
                                            } else {
                                                claim.pubkey.clone()
                                            };
                                            @let trajectory = get_tag_value(claim, "trajectory");
                                            @let estimate = get_tag_value(claim, "estimate");

                                            div.claim-card {
                                                div.claim-header {
                                                    span.claim-author { "🤖 " (claimer_pubkey) }
                                                    span.claim-time { "claimed " (claim.created_at) }
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

                                form.claim-form
                                    hx-post={"/repo/" (identifier) "/issues/" (issue.id) "/claim"}
                                    hx-target="this"
                                    hx-swap="outerHTML" {
                                    h3 { "Claim this Issue" }
                                    div.form-group {
                                        label for="claim_message" { "Message (optional)" }
                                        textarea
                                            name="content"
                                            id="claim_message"
                                            placeholder="I'll work on this issue..."
                                            rows="3" {}
                                    }
                                    div.form-group {
                                        label for="estimate" { "Estimated completion time (seconds)" }
                                        input
                                            type="number"
                                            name="estimate"
                                            id="estimate"
                                            placeholder="7200" {}
                                    }
                                    button.submit-button type="submit" { "Claim Issue" }
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
                                                    span.comment-time style="font-size: 0.875rem; color: var(--muted-color, #888);" { (comment.created_at) }
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
                                    div style="padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b; margin-bottom: 1rem;" {
                                        p style="margin: 0; font-size: 0.875rem;" {
                                            "⚠️ " strong { "Note: " }
                                            "Event publishing requires identity integration (issue #342). Comments cannot be posted yet."
                                        }
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

/// Issue creation form page
pub fn issue_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "New Issue - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                    h1.issue-form-title { "Create New Issue" }
                                    p.issue-form-subtitle { "Repository: " (repo_name) }
                                }
                                div.issue-form-actions {
                                    a.back-link href={"/repo/" (identifier) "/issues"} { "← Back to Issues" }
                                }
                            }

                            form.issue-form method="post" action={"/repo/" (identifier) "/issues"} {
                                div.form-group {
                                    label.form-label for="title" { "Title" }
                                    input class="form-input" id="title" type="text" name="title" required placeholder="Brief description of the issue";
                                }

                                div.form-group {
                                    label.form-label for="description" { "Description" }
                                    textarea class="form-textarea" id="description" name="description" rows="10" placeholder="Detailed description of the issue (optional)";
                                }

                                div.form-group {
                                    label.form-label for="labels" { "Labels" }
                                    input class="form-input" id="labels" type="text" name="labels" placeholder="Comma-separated labels (optional, e.g., bug, enhancement)";
                                    p.form-help { "Labels help categorize and filter issues" }
                                }

                                div.form-actions {
                                    button.btn-primary type="submit" { "Create Issue" }
                                    a.btn-secondary href={"/repo/" (identifier) "/issues"} { "Cancel" }
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

/// Patches list page for a repository
pub fn patches_list_page(repository: &Event, patches: &[Event], identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Patches - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.issues-container {
                            div.issues-header {
                                div {
                                    h1.issues-title { (repo_name) " - Patches" }
                                    p.issues-subtitle { "Viewing patches for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            @if patches.is_empty() {
                                div.empty-state {
                                    p { "No patches found for this repository." }
                                    p.info-text { "Patches will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.issues-count {
                                    span { (patches.len()) " patch" @if patches.len() != 1 { "es" } " found" }
                                }

                                div.issues-list {
                                    @for patch in patches {
                                        @let patch_title = get_tag_value(patch, "subject")
                                            .unwrap_or_else(|| "Untitled Patch".to_string());
                                        @let patch_author = if patch.pubkey.len() > 16 {
                                            format!("{}...{}", &patch.pubkey[..8], &patch.pubkey[patch.pubkey.len()-8..])
                                        } else {
                                            patch.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/patches/" (patch.id)} {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (patch_title) }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (patch_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { "Created " (patch.created_at) }
                                                }
                                            }
                                            @if !patch.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if patch.content.len() > 200 {
                                                        format!("{}...", &patch.content[..200])
                                                    } else {
                                                        patch.content.clone()
                                                    };
                                                    p { (preview) }
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

/// Pull requests list page for a repository
pub fn pull_requests_list_page(repository: &Event, pull_requests: &[Event], identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Pull Requests - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.issues-container {
                            div.issues-header {
                                div {
                                    h1.issues-title { (repo_name) " - Pull Requests" }
                                    p.issues-subtitle { "Viewing pull requests for repository: " (identifier) }
                                }
                                div.issues-actions {
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
                                }
                            }

                            @if pull_requests.is_empty() {
                                div.empty-state {
                                    p { "No pull requests found for this repository." }
                                    p.info-text { "Pull requests will appear here as they are created on the Nostr network." }
                                }
                            } @else {
                                div.issues-count {
                                    span { (pull_requests.len()) " pull request" @if pull_requests.len() != 1 { "s" } " found" }
                                }

                                div.issues-list {
                                    @for pr in pull_requests {
                                        @let pr_title = get_tag_value(pr, "subject")
                                            .unwrap_or_else(|| "Untitled Pull Request".to_string());
                                        @let pr_status = get_tag_value(pr, "status")
                                            .unwrap_or_else(|| "open".to_string());
                                        @let pr_author = if pr.pubkey.len() > 16 {
                                            format!("{}...{}", &pr.pubkey[..8], &pr.pubkey[pr.pubkey.len()-8..])
                                        } else {
                                            pr.pubkey.clone()
                                        };

                                        a.issue-card href={"/repo/" (identifier) "/pulls/" (pr.id)} {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (pr_title) }
                                                    span class={"issue-status " (pr_status)} {
                                                        (pr_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (pr_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { "Created " (pr.created_at) }
                                                }
                                            }
                                            @if !pr.content.is_empty() {
                                                div.issue-preview {
                                                    @let preview = if pr.content.len() > 200 {
                                                        format!("{}...", &pr.content[..200])
                                                    } else {
                                                        pr.content.clone()
                                                    };
                                                    p { (preview) }
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

/// Patch detail page
pub fn patch_detail_page(repository: &Event, patch: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let patch_title = get_tag_value(patch, "subject").unwrap_or_else(|| "Untitled Patch".to_string());
    
    // Format pubkey for display
    let patch_author = if patch.pubkey.len() > 16 {
        format!("{}...{}", &patch.pubkey[..8], &patch.pubkey[patch.pubkey.len()-8..])
    } else {
        patch.pubkey.clone()
    };
    
    // Extract commit ID and clone URL
    let commit_id = get_tag_value(patch, "c");
    let clone_url = get_tag_value(patch, "clone");
    
    // Extract all tags for display
    let all_tags = &patch.tags;
    
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (patch_title) " - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                    h1.issue-detail-title { (patch_title) }
                                    div.issue-detail-meta {
                                        span.issue-author { "by " (patch_author) }
                                        span.issue-separator { "•" }
                                        span.issue-time { "Created " (patch.created_at) }
                                    }
                                }
                                div.issue-detail-actions {
                                    a.back-link href={"/repo/" (identifier) "/patches"} { "← Back to Patches" }
                                }
                            }

                            section.issue-section {
                                h2 { "Repository Context" }
                                div.repo-context {
                                    a.repo-link href={"/repo/" (identifier)} { (repo_name) }
                                    span.repo-id-label { " (" (identifier) ")" }
                                }
                            }

                            @if !patch.content.is_empty() {
                                section.issue-section {
                                    h2 { "Patch Content" }
                                    pre style="background: #0d1117; color: #c9d1d9; padding: 1rem; overflow-x: auto; border: 1px solid var(--border-color, #333); font-size: 0.875rem; line-height: 1.5;" {
                                        code { (patch.content) }
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
                                        code { (patch.id) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Kind:" }
                                        code { (patch.kind) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Pubkey:" }
                                        code { (patch.pubkey) }
                                    }
                                    div.event-detail-item {
                                        span.label { "Signature:" }
                                        code.signature { (patch.sig) }
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

/// Pull request detail page
pub fn pull_request_detail_page(repository: &Event, pull_request: &Event, reviews: &[Event], status_events: &[Event], identifier: &str, trajectory_session: Option<&Event>, trajectory_events: &[Event], stack_prs: &[Event], dependency_pr: Option<&Event>, is_mergeable: bool) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let pr_title = get_tag_value(pull_request, "subject").unwrap_or_else(|| "Untitled Pull Request".to_string());
    let pr_status = get_tag_value(pull_request, "status").unwrap_or_else(|| "open".to_string());
    
    // Format pubkey for display
    let pr_author = if pull_request.pubkey.len() > 16 {
        format!("{}...{}", &pull_request.pubkey[..8], &pull_request.pubkey[pull_request.pubkey.len()-8..])
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
                title { (pr_title) " - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                        span.issue-time { "Created " (pull_request.created_at) }
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
                                                        span.claim-time { (status_event.created_at) }
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

                                        @if let Some(s) = stack {
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
                                }
                            }

                            section.issue-section {
                                h2 { "Reviews" }
                                @if reviews.is_empty() {
                                    p.empty-state { "No reviews yet. Be the first to review this PR!" }
                                } @else {
                                    div.claims-list {
                                        @for review in reviews {
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

                                            div.claim-card {
                                                div.claim-header {
                                                    span.claim-author { (review_emoji) " " (reviewer_pubkey) }
                                                    span.claim-time { (review.created_at) }
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
                                                span style="font-size: 0.875rem;" { (session.created_at) }
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
                                                                span style="font-size: 0.8rem; color: #64748b;" {
                                                                    (event.created_at)
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

/// Trajectory viewer page
pub fn trajectory_viewer_page(session: &Event, events: &[Event]) -> Markup {
    let agent_pubkey = if session.pubkey.len() > 16 {
        format!("{}...{}", &session.pubkey[..8], &session.pubkey[session.pubkey.len()-8..])
    } else {
        session.pubkey.clone()
    };

    // Extract session metadata
    let session_title = get_tag_value(session, "title").unwrap_or_else(|| "Untitled Session".to_string());
    let task_description = get_tag_value(session, "task");

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Trajectory: " (session_title) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("./styles.css"))
                    "
                    .trajectory-timeline {
                        position: relative;
                        padding-left: 2rem;
                    }
                    .trajectory-timeline::before {
                        content: '';
                        position: absolute;
                        left: 0.5rem;
                        top: 0;
                        bottom: 0;
                        width: 2px;
                        background: var(--border-color, #333);
                    }
                    .trajectory-event {
                        position: relative;
                        margin-bottom: 1.5rem;
                        padding: 1rem;
                        background: var(--card-bg, #1a1a1a);
                        border: 1px solid var(--border-color, #333);
                    }
                    .trajectory-event::before {
                        content: '';
                        position: absolute;
                        left: -1.5rem;
                        top: 1.5rem;
                        width: 10px;
                        height: 10px;
                        background: var(--accent-color, #0ea5e9);
                    }
                    .trajectory-event-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 0.5rem;
                    }
                    .event-type {
                        font-weight: 600;
                        color: var(--accent-color, #0ea5e9);
                    }
                    .event-timestamp {
                        font-size: 0.85rem;
                        color: var(--text-secondary, #888);
                    }
                    .event-content {
                        margin-top: 0.5rem;
                        padding: 0.5rem;
                        background: var(--bg-color, #0a0a0a);
                        font-family: monospace;
                        font-size: 0.9rem;
                        white-space: pre-wrap;
                        overflow-x: auto;
                    }
                    .trajectory-summary {
                        background: var(--card-bg, #1a1a1a);
                        padding: 1.5rem;
                        margin-bottom: 2rem;
                        border: 1px solid var(--border-color, #333);
                    }
                    "
                }
            }
            body {
                header {
                    h1 { "⚡ AgentGit" }
                    p.subtitle { "Nostr-native GitHub Alternative" }
                }
                main {
                    nav {
                        a href="/" { "Repositories" }
                        a href="/issues" { "Issues" }
                        a href="/agents" { "Agents" }
                    }
                    div.content {
                        div.trajectory-viewer {
                            h1 { "🔍 Trajectory Viewer" }

                            div.trajectory-summary {
                                h2 { (session_title) }
                                div.session-meta {
                                    p { "Agent: " span.agent-pubkey { (agent_pubkey) } }
                                    p { "Session ID: " code { (session.id) } }
                                    p { "Started: " span { (session.created_at) } }
                                    @if let Some(task) = task_description {
                                        p { "Task: " (task) }
                                    }
                                }
                                @if !session.content.is_empty() {
                                    div.session-description {
                                        h3 { "Description" }
                                        p { (session.content) }
                                    }
                                }
                            }

                            @if events.is_empty() {
                                p.empty-state { "No trajectory events found for this session." }
                            } @else {
                                h2 { "Event Timeline (" (events.len()) " events)" }

                                div.trajectory-timeline {
                                    @for event in events {
                                        @let event_type = get_tag_value(event, "type").unwrap_or_else(|| "unknown".to_string());
                                        @let tool_name = get_tag_value(event, "tool");

                                        div.trajectory-event {
                                            div.trajectory-event-header {
                                                span.event-type {
                                                    @if let Some(tool) = &tool_name {
                                                        "🔧 " (tool)
                                                    } @else {
                                                        "📝 " (event_type)
                                                    }
                                                }
                                                span.event-timestamp { (event.created_at) }
                                            }

                                            @if !event.content.is_empty() {
                                                div.event-content {
                                                    (event.content)
                                                }
                                            }

                                            details {
                                                summary { "View raw event data" }
                                                div.event-content {
                                                    "Event ID: " (event.id) "\n"
                                                    "Kind: " (event.kind) "\n"
                                                    "Pubkey: " (event.pubkey) "\n"
                                                    "Tags:\n"
                                                    @for tag in &event.tags {
                                                        "  [" (tag.join(", ")) "]\n"
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

/// Agent profile page
pub fn agent_profile_page(
    agent_pubkey: &str,
    pull_requests: &[Event],
    issue_claims: &[Event],
    reputation_labels: &[Event],
) -> Markup {
    // Format pubkey for display
    let display_pubkey = if agent_pubkey.len() > 16 {
        format!("{}...{}", &agent_pubkey[..8], &agent_pubkey[agent_pubkey.len()-8..])
    } else {
        agent_pubkey.to_string()
    };

    // Count merged PRs (those with status applied/merged)
    let merged_count = pull_requests.iter().filter(|pr| {
        get_tag_value(pr, "status").map(|s| s == "applied" || s == "merged").unwrap_or(false)
    }).count();

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Agent Profile: " (display_pubkey) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("./styles.css"))
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
                    h1 { "⚡ AgentGit" }
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
                                                    span.claim-time { (label.created_at) }
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
                                                        span.issue-time { "Created " (pr.created_at) }
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
                                                    span.claim-time { (claim.created_at) }
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

/// Search results page
pub fn search_results_page(query: &str, repositories: &[Event], issues: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Search: " (query) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("./styles.css"))
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
                    h1 { "⚡ AgentGit" }
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
                                                    span.issue-time { "Created " (issue.created_at) }
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

/// Watched repositories page
pub fn watched_repositories_page(repositories: &[Event]) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Watched Repositories - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body {
                header {
                    h1 { "⚡ AgentGit" }
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

/// Create pull request form page
pub fn pr_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Pull Request - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                        label.form-label for="depends_on" { "Depends On (Event ID)" }
                                        input class="form-input" id="depends_on" type="text" name="depends_on" placeholder="Event ID of the PR this depends on";
                                        p.form-help { "This PR must be merged after the dependency" }
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

/// Create patch form page
pub fn patch_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Patch - " (repo_name) " - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                    h1.issue-form-title { "Create Patch" }
                                    p.issue-form-subtitle { "Repository: " (repo_name) }
                                }
                                div.issue-form-actions {
                                    a.back-link href={"/repo/" (identifier) "/patches"} { "← Back to Patches" }
                                }
                            }

                            form.issue-form method="post" action={"/repo/" (identifier) "/patches"} {
                                div.form-group {
                                    label.form-label for="title" { "Title" }
                                    input class="form-input" id="title" type="text" name="title" required placeholder="Brief description of the patch";
                                }

                                div.form-group {
                                    label.form-label for="patch_content" { "Patch Content" }
                                    textarea class="form-textarea" id="patch_content" name="patch_content" rows="20" required placeholder="Paste git diff output here..." style="font-family: monospace;";
                                    p.form-help { "Generate with: git diff > my-changes.patch" }
                                }

                                div.form-group {
                                    label.form-label for="description" { "Description (optional)" }
                                    textarea class="form-textarea" id="description" name="description" rows="5" placeholder="Additional context about this patch";
                                }

                                div.form-actions {
                                    button.btn-primary type="submit" { "Create Patch" }
                                    a.btn-secondary href={"/repo/" (identifier) "/patches"} { "Cancel" }
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

/// Repository creation form page
pub fn repository_create_form_page() -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Repository - AgentGit" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ AgentGit" }
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
                                h1.issue-detail-title { "Create New Repository" }
                                div.issue-detail-actions {
                                    a.back-link href="/" { "← Back to Repositories" }
                                }
                            }

                            form.claim-form
                                hx-post="/repo"
                                hx-target="this"
                                hx-swap="outerHTML" {

                                div.form-group {
                                    label for="repo_identifier" { "Repository Identifier" span.required { "*" } }
                                    input
                                        type="text"
                                        name="identifier"
                                        id="repo_identifier"
                                        placeholder="my-awesome-project"
                                        required {}
                                    p.hint { "Unique identifier for this repository (lowercase, hyphens allowed)" }
                                }

                                div.form-group {
                                    label for="repo_name" { "Name" span.required { "*" } }
                                    input
                                        type="text"
                                        name="name"
                                        id="repo_name"
                                        placeholder="My Awesome Project"
                                        required {}
                                }

                                div.form-group {
                                    label for="repo_description" { "Description" }
                                    textarea
                                        name="description"
                                        id="repo_description"
                                        placeholder="A brief description of this repository..."
                                        rows="4" {}
                                }

                                div.form-group {
                                    label for="clone_url_git" { "Git Clone URL" span.required { "*" } }
                                    input
                                        type="url"
                                        name="clone_url_git"
                                        id="clone_url_git"
                                        placeholder="git@github.com:user/repo.git"
                                        required {}
                                    p.hint { "SSH clone URL (git protocol)" }
                                }

                                div.form-group {
                                    label for="clone_url_https" { "HTTPS Clone URL" }
                                    input
                                        type="url"
                                        name="clone_url_https"
                                        id="clone_url_https"
                                        placeholder="https://github.com/user/repo.git" {}
                                    p.hint { "Optional HTTPS clone URL" }
                                }

                                div.form-group {
                                    label for="web_url" { "Web URL" }
                                    input
                                        type="url"
                                        name="web_url"
                                        id="web_url"
                                        placeholder="https://github.com/user/repo" {}
                                    p.hint { "Optional web interface URL" }
                                }

                                div.form-group {
                                    label for="default_branch" { "Default Branch" }
                                    input
                                        type="text"
                                        name="default_branch"
                                        id="default_branch"
                                        placeholder="main"
                                        value="main" {}
                                }

                                div.form-group {
                                    label for="earliest_commit" { "Earliest Unique Commit" }
                                    input
                                        type="text"
                                        name="earliest_commit"
                                        id="earliest_commit"
                                        placeholder="abc123def456..." {}
                                    p.hint { "Optional: SHA of earliest commit unique to this repository (for fork tracking)" }
                                }

                                div.form-group {
                                    label for="maintainers" { "Additional Maintainers" }
                                    textarea
                                        name="maintainers"
                                        id="maintainers"
                                        placeholder="npub1...\nnpub2..."
                                        rows="3" {}
                                    p.hint { "One Nostr pubkey (npub) per line. You will be added as owner automatically." }
                                }

                                div style="padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b; margin-bottom: 1rem;" {
                                    p style="margin: 0; font-size: 0.875rem;" {
                                        "⚠️ " strong { "Note: " }
                                        "Event publishing requires identity integration (issue #342). This form will create the event template but cannot publish yet."
                                    }
                                }

                                button.submit-button type="submit" { "Create Repository Announcement" }
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
