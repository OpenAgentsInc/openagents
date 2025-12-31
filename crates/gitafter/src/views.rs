//! Maud view templates for GitAfter

pub mod diff;
pub mod publish_status;

use crate::reputation::{ReputationTier, calculate_review_weight};
use crate::trajectory::MatchStatus;
use chrono::{DateTime, Utc};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use nostr::Event;

#[allow(unused_imports)]
pub use publish_status::{publish_status_notification, publish_status_styles};

/// Helper function to extract tag value from event
pub fn get_tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

/// Helper function to extract all values for a tag name
fn get_all_tag_values(event: &Event, tag_name: &str) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

/// Format a Unix timestamp as relative time (e.g., "2 hours ago", "yesterday")
fn format_relative_time(timestamp: u64) -> String {
    let dt = DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_else(|| Utc::now());
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if duration.num_days() < 7 {
        let days = duration.num_days();
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        // For older dates, show the actual date
        dt.format("%b %d, %Y").to_string()
    }
}

/// Render a single repository card
#[allow(dead_code)]
fn repository_card(event: &Event) -> Markup {
    repository_card_with_bounty_count(event, &std::collections::HashMap::new())
}

fn repository_card_with_bounty_count(
    event: &Event,
    bounty_counts: &std::collections::HashMap<String, usize>,
) -> Markup {
    let name = get_tag_value(event, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(event, "description").unwrap_or_default();
    let identifier = get_tag_value(event, "d").unwrap_or_default();
    let has_clone_url = get_tag_value(event, "clone").is_some();
    let has_web_url = get_tag_value(event, "web").is_some();
    let bounty_count = bounty_counts.get(&identifier).copied().unwrap_or(0);

    // Truncate pubkey for display
    let short_pubkey = if event.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &event.pubkey[..8],
            &event.pubkey[event.pubkey.len() - 8..]
        )
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
                @if bounty_count > 0 {
                    span style="color: #fbbf24; margin-left: 0.5rem;" { "⚡ " (bounty_count) " bounties" }
                }
            }
        }
    }
}

/// Home page with repository list
pub fn home_page_with_repos(
    repositories: &[Event],
    selected_language: &Option<String>,
    selected_topic: &Option<String>,
    has_bounties_filter: bool,
    agent_friendly_filter: bool,
    bounty_counts: &std::collections::HashMap<String, usize>,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "GitAfter - Nostr GitHub Alternative" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
                }
            }
            body hx-ext="ws" ws-connect="/ws" {
                header {
                    h1 { "⚡ GitAfter" }
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

                        // Filter controls
                        form method="get" action="/" style="display: flex; gap: 1rem; margin-bottom: 1rem; padding: 1rem; background: #2a2a2a;" {
                            div {
                                label for="language" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Language" }
                                select name="language" id="language" style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;" {
                                    option value="" selected[selected_language.is_none()] { "All" }
                                    option value="rust" selected[selected_language.as_deref() == Some("rust")] { "Rust" }
                                    option value="javascript" selected[selected_language.as_deref() == Some("javascript")] { "JavaScript" }
                                    option value="typescript" selected[selected_language.as_deref() == Some("typescript")] { "TypeScript" }
                                    option value="python" selected[selected_language.as_deref() == Some("python")] { "Python" }
                                    option value="go" selected[selected_language.as_deref() == Some("go")] { "Go" }
                                }
                            }

                            div {
                                label for="topic" style="display: block; margin-bottom: 0.5rem; color: #aaa;" { "Topic" }
                                input
                                    type="text"
                                    name="topic"
                                    id="topic"
                                    placeholder="nostr, ai"
                                    value={(selected_topic.clone().unwrap_or_default())}
                                    style="padding: 0.5rem; background: #1a1a1a; color: #fff; border: 1px solid #444;" {}
                            }

                            div {
                                label style="display: block; margin-bottom: 0.5rem; color: #aaa;" {
                                    input type="checkbox" name="has_bounties" value="true" checked[has_bounties_filter];
                                    " Has Bounties"
                                }
                            }

                            div {
                                label style="display: block; margin-bottom: 0.5rem; color: #aaa;" {
                                    input type="checkbox" name="agent_friendly" value="true" checked[agent_friendly_filter];
                                    " Agent-Friendly"
                                }
                            }

                            button type="submit" style="padding: 0.5rem 1rem; background: #4a9eff; color: #fff; border: none; cursor: pointer;" { "Apply Filters" }
                            a href="/" style="padding: 0.5rem 1rem; background: #444; color: #fff; text-decoration: none; display: inline-block;" { "Clear" }
                        }

                        @if repositories.is_empty() {
                            p.placeholder { "No repositories found. Listening for NIP-34 events..." }
                        } @else {
                            div #repo-list .repo-list {
                                @for repo in repositories {
                                    (repository_card_with_bounty_count(repo, bounty_counts))
                                }
                            }
                        }
                    }
                }
                footer {
                    p { "Powered by NIP-34 (Git Stuff) • NIP-SA (Sovereign Agents) • NIP-57 (Zaps)" }
                }
                script {
                    (PreEscaped(r#"
                    // Toast notification system
                    function showToast(message, type = 'info') {
                        const toast = document.createElement('div');
                        toast.className = 'toast toast-' + type + ' item-inserted';
                        toast.textContent = message;
                        toast.style.cssText = 'position: fixed; top: 20px; right: 20px; padding: 1rem 1.5rem; background: var(--bg-tertiary); border: 2px solid var(--accent); color: var(--text-primary); z-index: 10000; font-weight: 600; box-shadow: 0 4px 12px rgba(0,0,0,0.5);';
                        if (type === 'success') {
                            toast.style.borderColor = '#00ff88';
                            toast.innerHTML = '<span style="color: #00ff88;">✓</span> ' + message;
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates
                    document.body.addEventListener('htmx:wsAfterMessage', function(evt) {
                        const message = evt.detail.message;
                        if (!message) return;

                        // Extract event from message
                        const parser = new DOMParser();
                        const doc = parser.parseFromString(message, 'text/html');
                        const eventDiv = doc.querySelector('.event');
                        if (!eventDiv) return;

                        const kind = parseInt(eventDiv.dataset.kind);
                        const jsonStr = eventDiv.textContent;
                        let event;
                        try {
                            event = JSON.parse(jsonStr);
                        } catch (e) {
                            console.error('Failed to parse event JSON:', e);
                            return;
                        }

                        // Handle repository events (kind:30617)
                        if (kind === 30617) {
                            console.log('New repository announced:', event.id);
                            showToast('New repository added!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}

/// Repository detail page
pub fn repository_detail_page(
    repository: &Event,
    is_cloned: bool,
    local_path: Option<String>,
    repo_state: Option<&Event>,
) -> Markup {
    let name =
        get_tag_value(repository, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(repository, "description").unwrap_or_default();
    let identifier = get_tag_value(repository, "d").unwrap_or_default();
    let clone_urls = get_all_tag_values(repository, "clone");
    let web_url = get_tag_value(repository, "web");
    let maintainers = get_all_tag_values(repository, "p");

    // Format pubkey for display
    let owner_pubkey = if repository.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &repository.pubkey[..8],
            &repository.pubkey[repository.pubkey.len() - 8..]
        )
    } else {
        repository.pubkey.clone()
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (name) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                                // Display robot icon for potential agent maintainers
                                                // (agents typically have npub addresses like regular users,
                                                // but this provides a visual hint for agent-capable repos)
                                                span style="margin-right: 0.5rem;" { "🤖" }
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

                            @if let Some(state) = repo_state {
                                section.repo-section {
                                    h2 { "Repository State" }
                                    @let branches = get_all_tag_values(state, "refs/heads");
                                    @let tags = get_all_tag_values(state, "refs/tags");
                                    @let head = get_tag_value(state, "HEAD");

                                    @if let Some(ref h) = head {
                                        div.state-item style="margin-bottom: 1rem;" {
                                            span.label style="font-weight: 600; margin-right: 0.5rem;" { "HEAD:" }
                                            code { (h) }
                                        }
                                    }

                                    @if !branches.is_empty() {
                                        div.state-item style="margin-bottom: 1rem;" {
                                            h3 style="font-size: 1rem; margin-bottom: 0.5rem;" { "Branches (" (branches.len()) ")" }
                                            div.branch-list style="display: flex; flex-direction: column; gap: 0.25rem;" {
                                                @for branch in &branches {
                                                    div style="padding: 0.25rem 0; font-family: monospace; font-size: 0.875rem;" { "• " (branch) }
                                                }
                                            }
                                        }
                                    }

                                    @if !tags.is_empty() {
                                        div.state-item {
                                            h3 style="font-size: 1rem; margin-bottom: 0.5rem;" { "Tags (" (tags.len()) ")" }
                                            div.tag-list style="display: flex; flex-direction: column; gap: 0.25rem;" {
                                                @for tag in &tags {
                                                    div style="padding: 0.25rem 0; font-family: monospace; font-size: 0.875rem;" { "• " (tag) }
                                                }
                                            }
                                        }
                                    }

                                    @if branches.is_empty() && tags.is_empty() && head.is_none() {
                                        p.empty-state { "No repository state information available" }
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
                                        span title={(repository.created_at)} { (format_relative_time(repository.created_at)) }
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
pub fn issues_list_page(
    repository: &Event,
    issues: &[Event],
    is_watched: bool,
    identifier: &str,
    filter_open: bool,
    filter_closed: bool,
    filter_has_bounty: bool,
    filter_claimed: bool,
    issue_first_claims: &std::collections::HashMap<String, Event>,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Issues - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                        @if filter_open {
                                            input type="checkbox" name="filter_open" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_open" value="true";
                                        }
                                        span { "Open" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_closed {
                                            input type="checkbox" name="filter_closed" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_closed" value="true";
                                        }
                                        span { "Closed" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_has_bounty {
                                            input type="checkbox" name="filter_has_bounty" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_has_bounty" value="true";
                                        }
                                        span { "Has Bounty" }
                                    }

                                    label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;" {
                                        @if filter_claimed {
                                            input type="checkbox" name="filter_claimed" value="true" checked;
                                        } @else {
                                            input type="checkbox" name="filter_claimed" value="true";
                                        }
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

                                div #issues-list .issues-list {
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
                                                    span.issue-time { (format_relative_time(issue.created_at)) }

                                                    @if let Some(claim) = issue_first_claims.get(&issue.id) {
                                                        @let claimer = if claim.pubkey.len() > 16 {
                                                            format!("{}...{}", &claim.pubkey[..8], &claim.pubkey[claim.pubkey.len()-8..])
                                                        } else {
                                                            claim.pubkey.clone()
                                                        };

                                                        @let estimate = claim.tags.iter()
                                                            .find(|tag| tag.first().map(|t| t == "estimate").unwrap_or(false))
                                                            .and_then(|tag| tag.get(1));

                                                        span.issue-separator { "•" }
                                                        span.claim-badge style="color: #fbbf24; font-weight: 600;" {
                                                            "🏆 Claimed by " (claimer)
                                                            @if let Some(est) = estimate {
                                                                " - " (est) "h"
                                                            }
                                                        }
                                                    }
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
                script {
                    (PreEscaped(r#"
                    // Toast notification system
                    function showToast(message, type = 'info') {
                        const toast = document.createElement('div');
                        toast.className = 'toast toast-' + type + ' item-inserted';
                        toast.textContent = message;
                        toast.style.cssText = 'position: fixed; top: 20px; right: 20px; padding: 1rem 1.5rem; background: var(--bg-tertiary); border: 2px solid var(--accent); color: var(--text-primary); z-index: 10000; font-weight: 600; box-shadow: 0 4px 12px rgba(0,0,0,0.5);';
                        if (type === 'success') {
                            toast.style.borderColor = '#00ff88';
                            toast.innerHTML = '<span style="color: #00ff88;">✓</span> ' + message;
                        } else if (type === 'bounty') {
                            toast.style.borderColor = '#ffaa00';
                            toast.innerHTML = '<span style="color: #ffaa00;">⚡</span> ' + message;
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates for issues
                    document.body.addEventListener('htmx:wsAfterMessage', function(evt) {
                        const message = evt.detail.message;
                        if (!message) return;

                        const parser = new DOMParser();
                        const doc = parser.parseFromString(message, 'text/html');
                        const eventDiv = doc.querySelector('.event');
                        if (!eventDiv) return;

                        const kind = parseInt(eventDiv.dataset.kind);
                        const jsonStr = eventDiv.textContent;
                        let event;
                        try {
                            event = JSON.parse(jsonStr);
                        } catch (e) {
                            console.error('Failed to parse event JSON:', e);
                            return;
                        }

                        // Handle issue events (kind:1621)
                        if (kind === 1621) {
                            console.log('New issue created:', event.id);
                            showToast('New issue created!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle bounty offer events (kind:1636)
                        if (kind === 1636) {
                            console.log('New bounty offer:', event.id);
                            showToast('New bounty offered!', 'bounty');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle issue claim events (kind:1634)
                        if (kind === 1634) {
                            console.log('Issue claimed:', event.id);
                            showToast('Issue claimed!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}
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
                    (include_str!("./styles.css"))
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

/// Issue creation form page
pub fn issue_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "New Issue - " (repo_name) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                title { (repo_name) " - Patches - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                                    span.issue-time { (format_relative_time(patch.created_at)) }
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
pub fn pull_requests_list_page(
    repository: &Event,
    pull_requests: &[Event],
    identifier: &str,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (repo_name) " - Pull Requests - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                div.pr-filters {
                                    button.filter-btn.active data-filter="all" { "All" }
                                    button.filter-btn data-filter="open" { "Open" }
                                    button.filter-btn data-filter="merged" { "Merged" }
                                    button.filter-btn data-filter="closed" { "Closed" }
                                    button.filter-btn data-filter="draft" { "Draft" }
                                }

                                div.issues-count {
                                    span { (pull_requests.len()) " pull request" @if pull_requests.len() != 1 { "s" } " found" }
                                }

                                div #pr-list .issues-list {
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

                                        a.issue-card href={"/repo/" (identifier) "/pulls/" (pr.id)} data-status=(pr_status) {
                                            div.issue-header {
                                                div.issue-title-row {
                                                    h3.issue-title { (pr_title) }
                                                    @let badge_class = match pr_status.as_str() {
                                                        "merged" => "status-badge status-merged",
                                                        "closed" => "status-badge status-closed",
                                                        "draft" => "status-badge status-draft",
                                                        _ => "status-badge status-open",
                                                    };
                                                    @let badge_icon = match pr_status.as_str() {
                                                        "merged" => "✓",
                                                        "closed" => "✗",
                                                        "draft" => "📝",
                                                        _ => "●",
                                                    };
                                                    span class=(badge_class) {
                                                        (badge_icon) " " (pr_status)
                                                    }
                                                }
                                                div.issue-meta {
                                                    span.issue-author { "by " (pr_author) }
                                                    span.issue-separator { "•" }
                                                    span.issue-time { (format_relative_time(pr.created_at)) }
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
                script {
                    (PreEscaped(r#"
                    // Toast notification system
                    function showToast(message, type = 'info') {
                        const toast = document.createElement('div');
                        toast.className = 'toast toast-' + type + ' item-inserted';
                        toast.textContent = message;
                        toast.style.cssText = 'position: fixed; top: 20px; right: 20px; padding: 1rem 1.5rem; background: var(--bg-tertiary); border: 2px solid var(--accent); color: var(--text-primary); z-index: 10000; font-weight: 600; box-shadow: 0 4px 12px rgba(0,0,0,0.5);';
                        if (type === 'success') {
                            toast.style.borderColor = '#00ff88';
                            toast.innerHTML = '<span style="color: #00ff88;">✓</span> ' + message;
                        } else if (type === 'info') {
                            toast.style.borderColor = '#00bfff';
                            toast.innerHTML = '<span style="color: #00bfff;">ℹ</span> ' + message;
                        }
                        document.body.appendChild(toast);
                        setTimeout(() => {
                            toast.classList.add('fading');
                            setTimeout(() => toast.remove(), 300);
                        }, 2000);
                    }

                    // WebSocket real-time updates for pull requests
                    document.body.addEventListener('htmx:wsAfterMessage', function(evt) {
                        const message = evt.detail.message;
                        if (!message) return;

                        const parser = new DOMParser();
                        const doc = parser.parseFromString(message, 'text/html');
                        const eventDiv = doc.querySelector('.event');
                        if (!eventDiv) return;

                        const kind = parseInt(eventDiv.dataset.kind);
                        const jsonStr = eventDiv.textContent;
                        let event;
                        try {
                            event = JSON.parse(jsonStr);
                        } catch (e) {
                            console.error('Failed to parse event JSON:', e);
                            return;
                        }

                        // Handle PR events (kind:1618)
                        if (kind === 1618) {
                            console.log('New PR created:', event.id);
                            showToast('New pull request!', 'success');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle PR update events (kind:1619)
                        if (kind === 1619) {
                            console.log('PR updated:', event.id);
                            showToast('PR updated!', 'info');
                            setTimeout(() => window.location.reload(), 1000);
                        }

                        // Handle status events (kind:1630-1633)
                        if (kind >= 1630 && kind <= 1633) {
                            console.log('PR status changed:', event.id);
                            const statusMap = {1630: 'opened', 1631: 'merged', 1632: 'closed', 1633: 'marked as draft'};
                            showToast('PR ' + (statusMap[kind] || 'updated') + '!', kind === 1631 ? 'success' : 'info');
                            setTimeout(() => window.location.reload(), 1000);
                        }
                    });
                    "#))
                }
            }
        }
    }
}

/// Patch detail page
pub fn patch_detail_page(
    repository: &Event,
    patch: &Event,
    _reviews: &[Event],
    _reviewer_reputations: &std::collections::HashMap<String, i32>,
    identifier: &str,
) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let patch_title =
        get_tag_value(patch, "subject").unwrap_or_else(|| "Untitled Patch".to_string());

    // Format pubkey for display
    let patch_author = if patch.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &patch.pubkey[..8],
            &patch.pubkey[patch.pubkey.len() - 8..]
        )
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
                title { (patch_title) " - " (repo_name) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                    h1.issue-detail-title { (patch_title) }
                                    div.issue-detail-meta {
                                        span.issue-author { "by " (patch_author) }
                                        span.issue-separator { "•" }
                                        span.issue-time { (format_relative_time(patch.created_at)) }
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
                script {
                    (PreEscaped(r#"
                    // PR filter functionality
                    document.querySelectorAll('.filter-btn').forEach(btn => {
                        btn.addEventListener('click', function() {
                            document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
                            this.classList.add('active');

                            const filter = this.dataset.filter;
                            const cards = document.querySelectorAll('.issue-card');

                            cards.forEach(card => {
                                if (filter === 'all' || card.dataset.status === filter) {
                                    card.style.display = '';
                                } else {
                                    card.style.display = 'none';
                                }
                            });
                        });
                    });
                    "#))
                }
    }
}

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
                    (include_str!("./styles.css"))
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

/// Trajectory viewer page
pub fn trajectory_viewer_page(session: &Event, events: &[Event]) -> Markup {
    let agent_pubkey = if session.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &session.pubkey[..8],
            &session.pubkey[session.pubkey.len() - 8..]
        )
    } else {
        session.pubkey.clone()
    };

    // Extract session metadata
    let session_title =
        get_tag_value(session, "title").unwrap_or_else(|| "Untitled Session".to_string());
    let task_description = get_tag_value(session, "task");

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Trajectory: " (session_title) " - GitAfter" }
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
                        div.trajectory-viewer {
                            h1 { "🔍 Trajectory Viewer" }

                            div.trajectory-summary {
                                h2 { (session_title) }
                                div.session-meta {
                                    p { "Agent: " span.agent-pubkey { (agent_pubkey) } }
                                    p { "Session ID: " code { (session.id) } }
                                    p { "Started: " span title={(session.created_at)} { (format_relative_time(session.created_at)) } }
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
                                                span.event-timestamp title={(event.created_at)} { (format_relative_time(event.created_at)) }
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
                    (include_str!("./styles.css"))
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

/// Create pull request form page
pub fn pr_create_form_page(repository: &Event, identifier: &str) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Pull Request - " (repo_name) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                        label.form-label for="depends_on" { "Depends On" }
                                        select
                                            class="form-input"
                                            id="depends_on"
                                            name="depends_on"
                                            hx-get={"/repo/" (identifier) "/pulls/available-deps"}
                                            hx-trigger="load"
                                            hx-swap="innerHTML"
                                            onchange={"htmx.ajax('GET', '/repo/" (identifier) "/pulls/' + this.value + '/stack-info', {target: '#stack-info-target', swap: 'innerHTML'})"}
                                        {
                                            option value="" { "-- Loading available PRs... --" }
                                        }
                                        p.form-help {
                                            "Select a PR this change depends on. Only open/draft PRs shown. The dependency must be merged first."
                                            br;
                                            strong { "⚠️ Creating circular dependencies will fail validation." }
                                        }
                                        div id="stack-info-target" {}
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
                title { "Create Patch - " (repo_name) " - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                title { "Create Repository - GitAfter" }
                script src="https://unpkg.com/htmx.org@2.0.4" {}
                script src="https://unpkg.com/htmx-ext-ws@2.0.1/ws.js" {}
                style {
                    (include_str!("./styles.css"))
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
                                    label for="repo_language" { "Primary Language" }
                                    select
                                        name="language"
                                        id="repo_language" {
                                        option value="" selected { "Select a language (optional)" }
                                        option value="rust" { "Rust" }
                                        option value="javascript" { "JavaScript" }
                                        option value="typescript" { "TypeScript" }
                                        option value="python" { "Python" }
                                        option value="go" { "Go" }
                                    }
                                }

                                div.form-group {
                                    label for="repo_topics" { "Topics" }
                                    input
                                        type="text"
                                        name="topics"
                                        id="repo_topics"
                                        placeholder="nostr, ai, tooling" {}
                                    p.hint { "Comma-separated topics to help discovery (optional)" }
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
                    (include_str!("./styles.css"))
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

/// Git branch creation form
pub fn git_branch_create_form_page(identifier: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Branch - " (identifier) " - GitAfter" }
                style {
                    (include_str!("./styles.css"))
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
                        span { "Create Branch" }
                    }
                }

                main {
                    h2 { "Create New Branch - " (identifier) }

                    form method="post" action={"/repo/" (identifier) "/git/branch"} {
                        div.form-group {
                            label for="branch_name" { "Branch Name" }
                            input type="text" id="branch_name" name="branch_name" required placeholder="feature/my-feature";
                        }

                        div.form-actions {
                            button type="submit" { "Create Branch" }
                            a.button.secondary href={"/repo/" (identifier)} { "Cancel" }
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

/// Diff viewer page with syntax highlighting
pub fn diff_viewer_page(
    identifier: &str,
    item_id: &str,
    item_type: &str,
    diff_content: &str,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Diff - " (item_id) " - GitAfter" }
                style {
                    (include_str!("./styles.css"))
                    "
                    .diff-viewer {
                        font-family: 'Courier New', monospace;
                        font-size: 0.9rem;
                        background: #1a1a1a;
                        padding: 1rem;
                        overflow-x: auto;
                    }
                    .diff-file {
                        margin-bottom: 2rem;
                        border: 1px solid #333;
                    }
                    .diff-file-header {
                        background: #2a2a2a;
                        padding: 0.5rem 1rem;
                        font-weight: bold;
                        color: #fff;
                        border-bottom: 1px solid #333;
                    }
                    .diff-line {
                        display: flex;
                        padding: 0.2rem 0;
                    }
                    .diff-line-number {
                        width: 4rem;
                        text-align: right;
                        padding-right: 1rem;
                        color: #666;
                        user-select: none;
                    }
                    .diff-line-content {
                        flex: 1;
                        white-space: pre;
                    }
                    .diff-added {
                        background: #1a3d1a;
                        color: #66ff66;
                    }
                    .diff-removed {
                        background: #3d1a1a;
                        color: #ff6666;
                    }
                    .diff-context {
                        color: #ccc;
                    }
                    .diff-hunk-header {
                        background: #2a3a4a;
                        color: #88ccff;
                        padding: 0.3rem 1rem;
                        font-weight: bold;
                    }
                    "
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
                        @if item_type == "pr" {
                            a href={"/repo/" (identifier) "/pulls/" (item_id)} { "Pull Request" }
                        } @else {
                            a href={"/repo/" (identifier) "/patches/" (item_id)} { "Patch" }
                        }
                        " | "
                        span { "Diff" }
                    }
                }

                main {
                    h2 { "Diff View" }

                    div.diff-viewer {
                        (render_diff_lines(diff_content))
                    }

                    div.actions style="margin-top: 2rem;" {
                        @if item_type == "pr" {
                            a.button href={"/repo/" (identifier) "/pulls/" (item_id)} { "← Back to PR" }
                        } @else {
                            a.button href={"/repo/" (identifier) "/patches/" (item_id)} { "← Back to Patch" }
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

/// Render diff lines with proper formatting
fn render_diff_lines(diff_content: &str) -> Markup {
    let lines: Vec<&str> = diff_content.lines().collect();
    let mut html_output = String::new();
    let mut in_file = false;
    let mut old_line_num = 0;
    let mut new_line_num = 0;

    for line in lines {
        if line.starts_with("diff --git") {
            if in_file {
                html_output.push_str("</div>");
            }
            if let Some(filename) = extract_filename(line) {
                html_output.push_str(&format!(
                    r#"<div class="diff-file"><div class="diff-file-header">File: {}</div>"#,
                    filename
                ));
                in_file = true;
            }
        } else if line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                old_line_num = old_start;
                new_line_num = new_start;
            }
            html_output.push_str(&format!(
                r#"<div class="diff-hunk-header">{}</div>"#,
                html_escape(line)
            ));
        } else if line.starts_with("+") && !line.starts_with("+++") {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-added"><span class="diff-line-number"></span><span class="diff-line-number">{}</span><span class="diff-line-content">{}</span></div>"#,
                new_line_num,
                html_escape(line)
            ));
            new_line_num += 1;
        } else if line.starts_with("-") && !line.starts_with("---") {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-removed"><span class="diff-line-number">{}</span><span class="diff-line-number"></span><span class="diff-line-content">{}</span></div>"#,
                old_line_num,
                html_escape(line)
            ));
            old_line_num += 1;
        } else if !line.starts_with("\\")
            && !line.starts_with("index ")
            && !line.starts_with("---")
            && !line.starts_with("+++")
            && !line.is_empty()
        {
            html_output.push_str(&format!(
                r#"<div class="diff-line diff-context"><span class="diff-line-number">{}</span><span class="diff-line-number">{}</span><span class="diff-line-content">{}</span></div>"#,
                old_line_num,
                new_line_num,
                html_escape(line)
            ));
            old_line_num += 1;
            new_line_num += 1;
        }
    }

    if in_file {
        html_output.push_str("</div>");
    }

    maud::PreEscaped(html_output)
}

/// HTML escape special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Extract filename from git diff header
fn extract_filename(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 4 {
        let filename = parts[2].trim_start_matches("a/");
        Some(filename.to_string())
    } else {
        None
    }
}

/// Parse hunk header to extract line numbers
fn parse_hunk_header(line: &str) -> Option<(i32, i32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let old_part = parts[1].trim_start_matches('-');
        let new_part = parts[2].trim_start_matches('+');

        let old_start = old_part.split(',').next()?.parse::<i32>().ok()?;
        let new_start = new_part.split(',').next()?.parse::<i32>().ok()?;

        Some((old_start, new_start))
    } else {
        None
    }
}

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
                    (include_str!("./styles.css"))
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
                    (include_str!("./styles.css"))
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
                    (include_str!("./styles.css"))
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

/// Render notifications page
pub fn notifications_page(notifications: &[crate::nostr::cache::Notification]) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Notifications - GitAfter" }
                style {
                    r#"
                    * { margin: 0; padding: 0; box-sizing: border-box; }
                    body { font-family: system-ui, -apple-system, sans-serif; background: #0d1117; color: #c9d1d9; }
                    header { background: #161b22; border-bottom: 1px solid #30363d; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; }
                    .header-logo { font-size: 1.5rem; font-weight: bold; color: #fff; text-decoration: none; }
                    nav a { color: #58a6ff; text-decoration: none; margin-left: 1.5rem; }
                    nav a:hover { text-decoration: underline; }
                    main { max-width: 1200px; margin: 2rem auto; padding: 0 1rem; }
                    h1 { margin-bottom: 1.5rem; color: #fff; }
                    .actions { margin-bottom: 1rem; }
                    .btn { background: #238636; color: #fff; padding: 0.5rem 1rem; border: none; cursor: pointer; text-decoration: none; display: inline-block; }
                    .btn:hover { background: #2ea043; }
                    .notification { background: #161b22; border: 1px solid #30363d; padding: 1rem; margin-bottom: 0.5rem; display: flex; justify-content: space-between; align-items: center; }
                    .notification.unread { border-left: 4px solid #58a6ff; }
                    .notification-title { font-weight: bold; color: #fff; margin-bottom: 0.25rem; }
                    .notification-preview { color: #8b949e; font-size: 0.875rem; }
                    .notification-time { color: #8b949e; font-size: 0.75rem; margin-top: 0.25rem; }
                    .mark-read-btn { background: #21262d; color: #c9d1d9; padding: 0.25rem 0.75rem; border: 1px solid #30363d; cursor: pointer; font-size: 0.875rem; }
                    .mark-read-btn:hover { background: #30363d; }
                    .empty-state { text-align: center; padding: 4rem 0; color: #8b949e; }
                    footer { text-align: center; color: #8b949e; margin-top: 4rem; padding: 2rem 0; border-top: 1px solid #30363d; }
                    "#
                }
                script {
                    r#"
                    function markAsRead(notificationId) {
                        fetch('/notifications/' + notificationId + '/read', {
                            method: 'POST'
                        }).then(() => {
                            location.reload();
                        });
                    }

                    function markAllAsRead() {
                        fetch('/notifications/mark-all-read', {
                            method: 'POST'
                        }).then(() => {
                            location.reload();
                        });
                    }
                    "#
                }
            }
            body {
                header {
                    a href="/" class="header-logo" { "GitAfter" }
                    nav {
                        a href="/" { "Home" }
                        a href="/agents" { "Agents" }
                        a href="/bounties" { "Bounties" }
                        a href="/notifications" { "Notifications" }
                    }
                }
                main {
                    h1 { "Notifications" }

                    @if !notifications.is_empty() {
                        div class="actions" {
                            button class="btn" onclick="markAllAsRead()" { "Mark All as Read" }
                        }

                        @for notification in notifications {
                            div class=(if !notification.read { "notification unread" } else { "notification" }) {
                                div {
                                    div class="notification-title" {
                                        (notification.title)
                                    }
                                    @if let Some(preview) = &notification.preview {
                                        div class="notification-preview" {
                                            (preview)
                                        }
                                    }
                                    div class="notification-time" {
                                        (format_relative_time(notification.created_at as u64))
                                    }
                                }
                                div {
                                    @if !notification.read {
                                        button class="mark-read-btn" onclick={(format!("markAsRead('{}')", notification.id))} {
                                            "Mark as Read"
                                        }
                                    }
                                }
                            }
                        }
                    } @else {
                        div class="empty-state" {
                            p { "No notifications yet" }
                        }
                    }
                }
                footer {
                    p { "Powered by NIP-34 (Git Stuff) • NIP-SA (Sovereign Agents)" }
                }
            }
        }
    }
}

/// Render automated review checklist component
pub fn review_checklist_component(identifier: &str, pr_id: &str) -> Markup {
    html! {
        section.review-checklist id="review-checklist" {
            h2 { "📋 Automated Review Checklist" }

            div.checklist-container
                hx-get={"/repo/" (identifier) "/pulls/" (pr_id) "/checklist"}
                hx-trigger="load"
                hx-swap="innerHTML" {
                div.loading-state style="padding: 2rem; text-align: center; color: #6b7280;" {
                    "⋯ Generating checklist and running automated checks..."
                }
            }

            div.checklist-help style="margin-top: 1rem; padding: 1rem; background: #1e293b; border-left: 3px solid #3b82f6;" {
                p style="margin: 0; font-size: 0.875rem; color: #cbd5e1;" {
                    "ℹ️ This checklist is generated based on changed files and PR type. "
                    "Auto-checks run when the repository is cloned locally. "
                    "Check items to track review progress."
                }
            }
        }
    }
}
