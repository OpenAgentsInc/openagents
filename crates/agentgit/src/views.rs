//! Maud view templates for AgentGit

use maud::{html, Markup, DOCTYPE};
use nostr::Event;

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
                        h2 { "Repositories (" (repositories.len()) ")" }
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
pub fn repository_detail_page(repository: &Event) -> Markup {
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
                                        @for url in clone_urls {
                                            @let onclick_code = format!("navigator.clipboard.writeText('{}')", url);
                                            div.clone-url-item {
                                                code { (url) }
                                                button.copy-btn onclick=(onclick_code) { "Copy" }
                                            }
                                        }
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
pub fn issues_list_page(repository: &Event, issues: &[Event]) -> Markup {
    let repo_name = get_tag_value(repository, "name").unwrap_or_else(|| "Repository".to_string());
    let identifier = get_tag_value(repository, "d").unwrap_or_default();

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
                                    a.nav-link href={"/repo/" (identifier) "/issues/new"} { "+ New Issue" }
                                    a.back-link href={"/repo/" (identifier)} { "← Back to Repository" }
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
pub fn issue_detail_page(repository: &Event, issue: &Event, identifier: &str) -> Markup {
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
