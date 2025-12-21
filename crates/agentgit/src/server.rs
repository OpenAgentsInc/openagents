//! Actix-web server for AgentGit

use actix_web::{web, App, HttpResponse, HttpServer};
use std::sync::Arc;

use crate::git::{clone_repository, get_repository_path, is_repository_cloned};
use crate::nostr::NostrClient;
use crate::nostr::events::{PatchBuilder, PullRequestBuilder, RepositoryAnnouncementBuilder};
use crate::views::{agent_profile_page, home_page_with_repos, issue_create_form_page, issue_detail_page, issues_list_page, patch_create_form_page, patch_detail_page, patches_list_page, pr_create_form_page, pull_request_detail_page, pull_requests_list_page, repository_create_form_page, repository_detail_page, search_results_page, trajectory_viewer_page};
use crate::ws::{ws_handler, WsBroadcaster};

/// Application state shared across handlers
pub struct AppState {
    pub broadcaster: Arc<WsBroadcaster>,
    pub nostr_client: Arc<NostrClient>,
}

/// Starts server on 127.0.0.1:0, returns the assigned port
pub async fn start_server(
    broadcaster: Arc<WsBroadcaster>,
    nostr_client: Arc<NostrClient>,
) -> anyhow::Result<u16> {
    let state = web::Data::new(AppState {
        broadcaster,
        nostr_client,
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/repo/new", web::get().to(repository_create_form))
            .route("/repo", web::post().to(repository_create))
            .route("/repo/{identifier}", web::get().to(repository_detail))
            .route("/repo/{identifier}/issues", web::get().to(repository_issues))
            .route("/repo/{identifier}/issues/new", web::get().to(issue_create_form))
            .route("/repo/{identifier}/issues", web::post().to(issue_create))
            .route("/repo/{identifier}/issues/{issue_id}", web::get().to(issue_detail))
            .route("/repo/{identifier}/issues/{issue_id}/claim", web::post().to(issue_claim))
            .route("/repo/{identifier}/issues/{issue_id}/bounty", web::post().to(issue_bounty_create))
            .route("/repo/{identifier}/patches", web::get().to(repository_patches))
            .route("/repo/{identifier}/patches/new", web::get().to(patch_create_form))
            .route("/repo/{identifier}/patches", web::post().to(patch_create))
            .route("/repo/{identifier}/patches/{patch_id}", web::get().to(patch_detail))
            .route("/repo/{identifier}/pulls", web::get().to(repository_pulls))
            .route("/repo/{identifier}/pulls/new", web::get().to(pr_create_form))
            .route("/repo/{identifier}/pulls", web::post().to(pr_create))
            .route("/repo/{identifier}/pulls/{pr_id}", web::get().to(pull_request_detail))
            .route("/repo/{identifier}/pulls/{pr_id}/review", web::post().to(pr_review_submit))
            .route("/repo/{identifier}/pulls/{pr_id}/status", web::post().to(pr_status_change))
            .route("/trajectory/{session_id}", web::get().to(trajectory_detail))
            .route("/agent/{pubkey}", web::get().to(agent_profile))
            .route("/search", web::get().to(search))
            .route("/watched", web::get().to(watched_repositories))
            .route("/repo/{identifier}/watch", web::post().to(watch_repository))
            .route("/repo/{identifier}/unwatch", web::post().to(unwatch_repository))
            .route("/repo/{identifier}/clone", web::post().to(clone_repo))
            .route("/ws", web::get().to(ws_route))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    tokio::spawn(server.run());

    Ok(port)
}

/// Home page
async fn index(state: web::Data<AppState>) -> HttpResponse {
    // Fetch repositories from cache
    let repositories = match state.nostr_client.get_cached_repositories(50).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to fetch repositories: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(home_page_with_repos(&repositories).into_string())
}

/// Repository detail page
async fn repository_detail(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Check if repository is cloned locally
    let is_cloned = is_repository_cloned(&identifier);
    let local_path = if is_cloned {
        Some(get_repository_path(&identifier).to_string_lossy().to_string())
    } else {
        None
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(repository_detail_page(&repository, is_cloned, local_path).into_string())
}

/// Repository issues list page
async fn repository_issues(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}", repository.pubkey,
        repository.tags.iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Check if repository is watched
    let is_watched = state.nostr_client.is_repository_watched(&identifier).await.unwrap_or(false);

    // Fetch issues for this repository
    let issues = match state.nostr_client.get_issues_by_repo(&repo_address, 100).await {
        Ok(iss) => iss,
        Err(e) => {
            tracing::warn!("Failed to fetch issues: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(issues_list_page(&repository, &issues, is_watched, &identifier).into_string())
}

/// Issue detail page
async fn issue_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, issue_id) = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Fetch issue by event ID
    let issue = match state.nostr_client.get_cached_event(&issue_id).await {
        Ok(Some(iss)) => iss,
        Ok(None) => {
            tracing::warn!("Issue not found: {}", issue_id);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Issue not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch issue: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching issue</h1>");
        }
    };

    // Fetch claims for this issue
    let claims = match state.nostr_client.get_claims_for_issue(&issue_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to fetch claims for issue {}: {}", issue_id, e);
            Vec::new() // Continue with empty claims if fetch fails
        }
    };

    // Fetch bounties for this issue
    let bounties = match state.nostr_client.get_bounties_for_issue(&issue_id).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to fetch bounties for issue {}: {}", issue_id, e);
            Vec::new() // Continue with empty bounties if fetch fails
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(issue_detail_page(&repository, &issue, &claims, &bounties, &identifier).into_string())
}

/// Claim an issue
async fn issue_claim(
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (_identifier, _issue_id) = path.into_inner();
    let content = form.get("content").cloned().unwrap_or_default();
    let estimate = form.get("estimate").and_then(|s| s.parse::<u64>().ok());

    // For now, return a success message
    // In a real implementation, this would:
    // 1. Build an IssueClaimBuilder event
    // 2. Sign it with the user's key
    // 3. Publish it to relays
    // 4. Cache it locally

    let response_html = format!(
        r#"<div class="success-message">
            <p>✅ Issue claim submitted!</p>
            <p>Message: {}</p>
            {}
        </div>"#,
        if content.is_empty() { "No message" } else { &content },
        estimate.map(|e| format!("<p>Estimate: {} seconds</p>", e)).unwrap_or_default()
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Create a bounty for an issue
async fn issue_bounty_create(
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (_identifier, _issue_id) = path.into_inner();

    // Extract form data
    let amount = match form.get("amount").and_then(|s| s.parse::<u64>().ok()) {
        Some(amt) => amt,
        None => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Invalid amount</p></div>"#);
        }
    };

    let expiry = form.get("expiry").and_then(|s| s.parse::<u64>().ok());
    let conditions: Vec<String> = form
        .get("conditions")
        .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default();

    // For now, return a success message
    // In a real implementation, this would:
    // 1. Build a BountyOfferBuilder event
    // 2. Sign it with the user's key
    // 3. Publish it to relays
    // 4. Cache it locally

    let response_html = format!(
        r#"<div class="success-message">
            <p>✅ Bounty created!</p>
            <p>Amount: ⚡ {} sats</p>
            {}
            {}
        </div>"#,
        amount,
        expiry.map(|e| format!("<p>Expires: {}</p>", e)).unwrap_or_default(),
        if !conditions.is_empty() {
            format!("<p>Conditions: <ul>{}</ul></p>",
                conditions.iter().map(|c| format!("<li>{}</li>", c)).collect::<String>())
        } else {
            String::new()
        }
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Submit a review for a PR
async fn pr_review_submit(
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (_identifier, _pr_id) = path.into_inner();

    // Extract form data
    let review_type = form.get("review_type").cloned().unwrap_or_else(|| "comment".to_string());
    let content = form.get("content").cloned().unwrap_or_default();

    if content.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Review content cannot be empty</p></div>"#);
    }

    // For now, return a success message
    // In a real implementation, this would:
    // 1. Build a kind:1 (text note) event with e tag referencing the PR
    // 2. Add review type tags (approve, request_changes, comment)
    // 3. Sign it with the user's key
    // 4. Publish it to relays
    // 5. Cache it locally

    let review_emoji = match review_type.as_str() {
        "approve" => "✅",
        "request_changes" => "🔴",
        _ => "💬",
    };

    let response_html = format!(
        r#"<div class="success-message">
            <p>{} Review submitted!</p>
            <p>Type: {}</p>
            <p>Comment: {}</p>
        </div>"#,
        review_emoji,
        review_type,
        content
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Change the status of a PR
async fn pr_status_change(
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (_identifier, _pr_id) = path.into_inner();

    // Extract form data
    let status = form.get("status").cloned().unwrap_or_else(|| "open".to_string());
    let reason = form.get("reason").cloned().unwrap_or_default();

    // Map status string to kind
    let (status_kind, status_label) = match status.as_str() {
        "open" => (1630, "Open"),
        "applied" | "merged" => (1631, "Applied/Merged"),
        "closed" => (1632, "Closed"),
        "draft" => (1633, "Draft"),
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Invalid status</p></div>"#);
        }
    };

    // For now, return a success message
    // In a real implementation, this would:
    // 1. Build a StatusEventBuilder with the appropriate kind
    // 2. Sign it with the user's key
    // 3. Publish it to relays
    // 4. Cache it locally

    let response_html = format!(
        r#"<div class="success-message">
            <p>✅ Status changed to: {}</p>
            <p>Kind: {}</p>
            {}
        </div>"#,
        status_label,
        status_kind,
        if !reason.is_empty() {
            format!("<p>Reason: {}</p>", reason)
        } else {
            String::new()
        }
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Issue creation form
async fn issue_create_form(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(issue_create_form_page(&repository, &identifier).into_string())
}

/// Issue creation handler
async fn issue_create(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<IssueCreateForm>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository to get pubkey and build address
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // TODO: Implement event creation and publishing
    // For now, return a placeholder message
    tracing::warn!("Issue creation not yet implemented - need identity/signing integration");
    tracing::info!(
        "Would create issue: title='{}', description={:?}, labels={:?}, repo={}",
        form.title,
        form.description,
        form.labels,
        repo_address
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!(
            "<h1>Issue Creation Not Yet Implemented</h1>\
             <p><strong>Title:</strong> {}</p>\
             <p><strong>Repository:</strong> {}</p>\
             <p><strong>Description:</strong> {}</p>\
             <p><strong>Labels:</strong> {}</p>\
             <p>This feature requires identity/wallet integration for event signing.</p>\
             <p><a href=\"/repo/{}/issues\">Back to issues</a></p>",
            form.title,
            repo_address,
            form.description.as_deref().unwrap_or("None"),
            form.labels.as_deref().unwrap_or("None"),
            identifier
        ))
}

/// Form data for issue creation
#[derive(Debug, serde::Deserialize)]
struct IssueCreateForm {
    title: String,
    description: Option<String>,
    labels: Option<String>,
}

/// Repository patches list page
async fn repository_patches(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}",
        repository.pubkey,
        repository.tags.iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Fetch patches for this repository
    let patches = match state.nostr_client.get_patches_by_repo(&repo_address, 100).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to fetch patches: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(patches_list_page(&repository, &patches, &identifier).into_string())
}

/// Repository pull requests list page
async fn repository_pulls(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}",
        repository.pubkey,
        repository.tags.iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Fetch pull requests for this repository
    let pull_requests = match state.nostr_client.get_pull_requests_by_repo(&repo_address, 100).await {
        Ok(prs) => prs,
        Err(e) => {
            tracing::warn!("Failed to fetch pull requests: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(pull_requests_list_page(&repository, &pull_requests, &identifier).into_string())
}

/// Patch detail page
async fn patch_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, patch_id) = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Fetch patch by event ID
    let patch = match state.nostr_client.get_cached_event(&patch_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            tracing::warn!("Patch not found: {}", patch_id);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Patch not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch patch: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching patch</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(patch_detail_page(&repository, &patch, &identifier).into_string())
}

/// Pull request detail page
async fn pull_request_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, pr_id) = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    // Fetch pull request by event ID
    let pull_request = match state.nostr_client.get_cached_event(&pr_id).await {
        Ok(Some(pr)) => pr,
        Ok(None) => {
            tracing::warn!("Pull request not found: {}", pr_id);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Pull request not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch pull request: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching pull request</h1>");
        }
    };

    // Fetch reviews for this PR
    let reviews = match state.nostr_client.get_reviews_for_pr(&pr_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to fetch reviews for PR {}: {}", pr_id, e);
            Vec::new() // Continue with empty reviews if fetch fails
        }
    };

    // Fetch status events for this PR
    let status_events = match state.nostr_client.get_status_events_for_pr(&pr_id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to fetch status events for PR {}: {}", pr_id, e);
            Vec::new() // Continue with empty status events if fetch fails
        }
    };

    // Fetch trajectory session linked to this PR via "trajectory" tag
    let trajectory_session_id = pull_request.tags.iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "trajectory")
        .map(|tag| tag[1].clone());

    let trajectory_session = if let Some(session_id) = trajectory_session_id {
        match state.nostr_client.get_trajectory_session(&session_id).await {
            Ok(session) => {
                tracing::debug!("Found trajectory session for PR {}: {}", pr_id, session_id);
                session
            }
            Err(e) => {
                tracing::warn!("Failed to fetch trajectory session {}: {}", session_id, e);
                None
            }
        }
    } else {
        None
    };

    // Fetch trajectory events if session exists
    let trajectory_events = if let Some(ref session) = trajectory_session {
        match state.nostr_client.get_trajectory_events(&session.id).await {
            Ok(events) => {
                tracing::debug!("Found {} trajectory events for session {}", events.len(), session.id);
                events
            }
            Err(e) => {
                tracing::warn!("Failed to fetch trajectory events for session {}: {}", session.id, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Fetch stacked diff metadata
    let stack_id = pull_request.tags.iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "stack")
        .map(|tag| tag[1].clone());

    let stack_prs = if let Some(sid) = stack_id.as_ref() {
        match state.nostr_client.get_pull_requests_by_stack(sid).await {
            Ok(prs) => prs,
            Err(e) => {
                tracing::warn!("Failed to fetch stack PRs for {}: {}", sid, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let dependency_pr = match state.nostr_client.get_dependency_pr(&pull_request).await {
        Ok(dep) => dep,
        Err(e) => {
            tracing::warn!("Failed to fetch dependency PR: {}", e);
            None
        }
    };

    let is_mergeable = match state.nostr_client.is_pr_mergeable(&pull_request).await {
        Ok(mergeable) => mergeable,
        Err(e) => {
            tracing::warn!("Failed to check if PR is mergeable: {}", e);
            false
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(pull_request_detail_page(&repository, &pull_request, &reviews, &status_events, &identifier, trajectory_session.as_ref(), &trajectory_events, &stack_prs, dependency_pr.as_ref(), is_mergeable).into_string())
}

/// Trajectory detail page
async fn trajectory_detail(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Fetch trajectory session
    let session = match state.nostr_client.get_trajectory_session(&session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            tracing::warn!("Trajectory session not found: {}", session_id);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Trajectory session not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch trajectory session: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching trajectory session</h1>");
        }
    };

    // Fetch trajectory events
    let events = match state.nostr_client.get_trajectory_events(&session_id).await {
        Ok(evts) => evts,
        Err(e) => {
            tracing::warn!("Failed to fetch trajectory events for session {}: {}", session_id, e);
            Vec::new() // Continue with empty events if fetch fails
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(trajectory_viewer_page(&session, &events).into_string())
}

/// Agent profile page
async fn agent_profile(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let pubkey = path.into_inner();

    // Fetch pull requests by this agent
    let pull_requests = match state.nostr_client.get_pull_requests_by_agent(&pubkey, 50).await {
        Ok(prs) => prs,
        Err(e) => {
            tracing::warn!("Failed to fetch pull requests for agent {}: {}", pubkey, e);
            Vec::new()
        }
    };

    // Fetch issue claims by this agent
    let issue_claims = match state.nostr_client.get_issue_claims_by_agent(&pubkey, 50).await {
        Ok(claims) => claims,
        Err(e) => {
            tracing::warn!("Failed to fetch issue claims for agent {}: {}", pubkey, e);
            Vec::new()
        }
    };

    // Fetch reputation labels for this agent
    let reputation_labels = match state.nostr_client.get_reputation_labels_for_agent(&pubkey).await {
        Ok(labels) => labels,
        Err(e) => {
            tracing::warn!("Failed to fetch reputation labels for agent {}: {}", pubkey, e);
            Vec::new()
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(agent_profile_page(&pubkey, &pull_requests, &issue_claims, &reputation_labels).into_string())
}

/// Watch a repository
async fn watch_repository(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Get repository to extract address
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Repository not found</p></div>"#);
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Error fetching repository</p></div>"#);
        }
    };

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey,
        repository.tags.iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Watch the repository
    if let Err(e) = state.nostr_client.watch_repository(&identifier, &repo_address).await {
        tracing::error!("Failed to watch repository {}: {}", identifier, e);
        return HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Failed to watch repository</p></div>"#);
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(r#"<div class="success-message"><p>✅ Watching repository!</p></div>"#)
}

/// Unwatch a repository
async fn unwatch_repository(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Unwatch the repository
    if let Err(e) = state.nostr_client.unwatch_repository(&identifier).await {
        tracing::error!("Failed to unwatch repository {}: {}", identifier, e);
        return HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Failed to unwatch repository</p></div>"#);
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(r#"<div class="success-message"><p>✅ Unwatched repository</p></div>"#)
}

/// Watched repositories page
async fn watched_repositories(
    state: web::Data<AppState>,
) -> HttpResponse {
    // Get list of watched repository identifiers
    let watched_ids = match state.nostr_client.get_watched_repositories().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!("Failed to get watched repositories: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error loading watched repositories</h1>");
        }
    };

    // Fetch repository details for each watched identifier
    let mut repositories = Vec::new();
    for id in watched_ids {
        if let Ok(Some(repo)) = state.nostr_client.get_repository_by_identifier(&id).await {
            repositories.push(repo);
        }
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(crate::views::watched_repositories_page(&repositories).into_string())
}

/// Clone a repository to local workspace
async fn clone_repo(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache to get clone URL
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Repository not found</p></div>"#);
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Error fetching repository</p></div>"#);
        }
    };

    // Get clone URL from repository tags
    let clone_url = repository.tags.iter()
        .find(|tag| tag.first().map(|t| t == "clone").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    let clone_url = match clone_url {
        Some(url) => url,
        None => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ No clone URL available for this repository</p></div>"#);
        }
    };

    // Check if already cloned
    if is_repository_cloned(&identifier) {
        let path = get_repository_path(&identifier);
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(format!(
                r#"<div class="success-message">
                    <p>✅ Repository already cloned</p>
                    <p>Path: <code>{}</code></p>
                </div>"#,
                path.display()
            ));
    }

    // Clone the repository
    let dest_path = get_repository_path(&identifier);
    match clone_repository(&clone_url, &dest_path, None) {
        Ok(_) => {
            tracing::info!("Successfully cloned repository {} to {:?}", identifier, dest_path);
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div class="success-message">
                        <p>✅ Repository cloned successfully!</p>
                        <p>Path: <code>{}</code></p>
                    </div>"#,
                    dest_path.display()
                ))
        }
        Err(e) => {
            tracing::error!("Failed to clone repository {}: {}", identifier, e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div class="error-message">
                        <p>❌ Failed to clone repository</p>
                        <p>Error: {}</p>
                    </div>"#,
                    e
                ))
        }
    }
}

/// Search page
async fn search(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    // Get query parameter
    let q = query.get("q").map(|s| s.as_str()).unwrap_or("");

    if q.is_empty() {
        // Return empty search page
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(search_results_page("", &[], &[]).into_string());
    }

    // Search repositories
    let repositories = match state.nostr_client.search_repositories(q, 50).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to search repositories for '{}': {}", q, e);
            Vec::new()
        }
    };

    // Search issues
    let issues = match state.nostr_client.search_issues(q, 50).await {
        Ok(iss) => iss,
        Err(e) => {
            tracing::warn!("Failed to search issues for '{}': {}", q, e);
            Vec::new()
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(search_results_page(q, &repositories, &issues).into_string())
}

/// Pull request creation form
async fn pr_create_form(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(pr_create_form_page(&repository, &identifier).into_string())
}

/// Pull request creation handler
async fn pr_create(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<PrCreateForm>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository to get pubkey and build address
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build the PR event using PullRequestBuilder
    let mut builder = PullRequestBuilder::new(
        &repo_address,
        &form.subject,
        form.description.as_deref().unwrap_or(""),
    )
    .commit(&form.commit_id)
    .clone_url(&form.clone_url);

    // Add optional trajectory data
    if let Some(traj) = &form.trajectory_session {
        builder = builder.trajectory(traj);
    }
    if let Some(hash) = &form.trajectory_hash {
        builder = builder.trajectory_hash(hash);
    }

    // Add optional stacked diff data
    if let Some(dep) = &form.depends_on {
        builder = builder.depends_on(dep);
    }
    if let Some(stack) = &form.stack_id {
        builder = builder.stack(stack);
    }
    if let (Some(current), Some(total)) = (form.layer_current, form.layer_total) {
        builder = builder.layer(current, total);
    }

    let event_template = builder.build();

    // TODO: Sign the event template with user's identity
    // TODO: Publish to relays
    // TODO: Cache locally for immediate display
    // For now, log what would be published
    tracing::info!(
        "Would publish PR event: kind={}, subject='{}', repo={}",
        event_template.kind,
        form.subject,
        repo_address
    );
    tracing::debug!("Event template: {:?}", event_template);

    // Return success message with event details
    let mut response = format!(
        "<div style=\"padding: 2rem; max-width: 600px; margin: 0 auto;\">\
         <h1 style=\"color: #4ade80;\">✅ Pull Request Event Created</h1>\
         <p style=\"color: #888;\">Event built successfully. Publishing requires identity/wallet integration.</p>\
         <div style=\"background: #1a1a1a; padding: 1rem; margin: 1rem 0; border-left: 3px solid #4ade80;\">\
         <p><strong>Kind:</strong> {}</p>\
         <p><strong>Title:</strong> {}</p>\
         <p><strong>Repository:</strong> {}</p>\
         <p><strong>Commit ID:</strong> {}</p>\
         <p><strong>Clone URL:</strong> {}</p>\
         <p><strong>Tags:</strong> {} tags</p>\
         </div>",
        event_template.kind,
        form.subject,
        repo_address,
        form.commit_id,
        form.clone_url,
        event_template.tags.len()
    );

    if form.trajectory_session.is_some() || form.trajectory_hash.is_some() {
        response.push_str("<p><strong>Trajectory:</strong> Linked ✓</p>");
    }

    if form.depends_on.is_some() || form.stack_id.is_some() {
        response.push_str("<p><strong>Stacked Diff:</strong> Configured ✓</p>");
    }

    response.push_str(&format!(
        "<p style=\"margin-top: 2rem;\">\
         <a href=\"/repo/{}/pulls\" style=\"color: #4ade80;\">← Back to pull requests</a>\
         </p>\
         </div>",
        identifier
    ));

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response)
}

/// Form data for PR creation
#[derive(Debug, serde::Deserialize)]
struct PrCreateForm {
    subject: String,
    description: Option<String>,
    commit_id: String,
    clone_url: String,
    trajectory_session: Option<String>,
    trajectory_hash: Option<String>,
    depends_on: Option<String>,
    stack_id: Option<String>,
    layer_current: Option<u32>,
    layer_total: Option<u32>,
}

/// Patch creation form
async fn patch_create_form(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            tracing::warn!("Repository not found: {}", identifier);
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching repository</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(patch_create_form_page(&repository, &identifier).into_string())
}

/// Patch creation handler
async fn patch_create(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<PatchCreateForm>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository to get pubkey and build address
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Repository not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1>");
        }
    };

    // Build repository address (30617:pubkey:identifier)
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build the patch event using PatchBuilder
    let mut builder = PatchBuilder::new(
        &repo_address,
        &form.title,
        &form.patch_content,
    );

    // Add optional description
    if let Some(desc) = &form.description {
        builder = builder.description(desc);
    }

    let event_template = builder.build();

    // TODO: Sign the event template with user's identity
    // TODO: Publish to relays
    // TODO: Cache locally for immediate display
    // For now, log what would be published
    tracing::info!(
        "Would publish patch event: kind={}, title='{}', patch_size={} bytes, repo={}",
        event_template.kind,
        form.title,
        form.patch_content.len(),
        repo_address
    );
    tracing::debug!("Event template content length: {} bytes", event_template.content.len());

    // Return success message with event details
    let response = format!(
        "<div style=\"padding: 2rem; max-width: 600px; margin: 0 auto;\">\
         <h1 style=\"color: #4ade80;\">✅ Patch Event Created</h1>\
         <p style=\"color: #888;\">Event built successfully. Publishing requires identity/wallet integration.</p>\
         <div style=\"background: #1a1a1a; padding: 1rem; margin: 1rem 0; border-left: 3px solid #4ade80;\">\
         <p><strong>Kind:</strong> {}</p>\
         <p><strong>Title:</strong> {}</p>\
         <p><strong>Repository:</strong> {}</p>\
         <p><strong>Patch Size:</strong> {} bytes</p>\
         <p><strong>Tags:</strong> {} tags</p>\
         <p><strong>Has Description:</strong> {}</p>\
         </div>\
         <p style=\"margin-top: 2rem;\">\
         <a href=\"/repo/{}/patches\" style=\"color: #4ade80;\">← Back to patches</a>\
         </p>\
         </div>",
        event_template.kind,
        form.title,
        repo_address,
        form.patch_content.len(),
        event_template.tags.len(),
        if form.description.is_some() { "Yes ✓" } else { "No" },
        identifier
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response)
}

/// Form data for patch creation
#[derive(Debug, serde::Deserialize)]
struct PatchCreateForm {
    title: String,
    patch_content: String,
    description: Option<String>,
}

/// WebSocket upgrade
async fn ws_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    ws_handler(req, stream, state.broadcaster.clone()).await
}

/// Repository creation form page
async fn repository_create_form() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(repository_create_form_page().into_string())
}

/// Form data for repository creation
#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct RepositoryCreateForm {
    identifier: String,
    name: String,
    description: Option<String>,
    clone_url_git: String,
    clone_url_https: Option<String>,
    web_url: Option<String>,
    default_branch: Option<String>,
    earliest_commit: Option<String>,
    maintainers: Option<String>,
}

/// Repository creation handler
async fn repository_create(
    _state: web::Data<AppState>,
    form: web::Form<RepositoryCreateForm>,
) -> HttpResponse {
    // Parse maintainers (one npub per line)
    let maintainers: Vec<String> = form
        .maintainers
        .as_ref()
        .map(|m| m.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default();

    // Build repository announcement event using RepositoryAnnouncementBuilder
    let mut builder = RepositoryAnnouncementBuilder::new(&form.identifier, &form.name);

    // Add optional description
    if let Some(desc) = &form.description {
        if !desc.is_empty() {
            builder = builder.description(desc);
        }
    }

    // Add clone URLs
    builder = builder.clone_url(&form.clone_url_git);
    if let Some(https_url) = &form.clone_url_https {
        if !https_url.is_empty() {
            builder = builder.clone_url(https_url);
        }
    }

    // Add web URL
    if let Some(web) = &form.web_url {
        if !web.is_empty() {
            builder = builder.web_url(web);
        }
    }

    // Add maintainers
    for maintainer in &maintainers {
        builder = builder.maintainer(maintainer);
    }

    // Add earliest commit
    if let Some(commit) = &form.earliest_commit {
        if !commit.is_empty() {
            builder = builder.earliest_commit(commit);
        }
    }

    // Add default branch
    if let Some(branch) = &form.default_branch {
        if !branch.is_empty() {
            builder = builder.default_branch(branch);
        }
    }

    let event_template = builder.build();

    // Log event details for debugging
    tracing::info!(
        "Created repository announcement event template: kind={}, identifier='{}', name='{}', tags_count={}",
        event_template.kind,
        form.identifier,
        form.name,
        event_template.tags.len()
    );
    tracing::debug!("Event template tags: {:?}", event_template.tags);

    // TODO: Sign with identity (requires wallet integration)
    // TODO: Publish to relays
    // TODO: Cache locally
    // TODO: Redirect to /repo/{identifier}

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!(
            r#"<div class="success-message" style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                <h3>Repository Event Template Created Successfully</h3>
                <p><strong>Identifier:</strong> <code>{}</code></p>
                <p><strong>Name:</strong> {}</p>
                <p><strong>Event Kind:</strong> 30617 (Repository Announcement)</p>
                <p><strong>Tags:</strong> {} tags generated</p>
                <p><strong>Clone URLs:</strong> {}</p>
                {}
                <div style="margin-top: 1rem; padding: 12px; background: #fef3c7; border-left: 4px solid #f59e0b;">
                    <p style="margin: 0; font-size: 0.875rem;">
                        ⚠️ <strong>Note:</strong> Event signing and publishing requires identity integration (issue #342).
                        The event template has been created but not published to relays.
                    </p>
                </div>
                <p style="margin-top: 1rem;"><a href="/">← Back to Repositories</a></p>
            </div>"#,
            form.identifier,
            form.name,
            event_template.tags.len(),
            if form.clone_url_https.is_some() { "Git + HTTPS" } else { "Git" },
            if !maintainers.is_empty() {
                format!("<p><strong>Maintainers:</strong> {} additional maintainers</p>", maintainers.len())
            } else {
                String::new()
            }
        ))
}
