//! Actix-web server for GitAfter

use actix_web::{web, App, HttpResponse, HttpServer};
use openagents_spark::SparkWallet;
use std::sync::Arc;
use tokio::task::JoinHandle;
use wallet::core::identity::UnifiedIdentity;

use crate::git::{clone_repository, get_repository_path, is_repository_cloned, create_branch, get_status, generate_patch, apply_patch, push_branch, current_branch, diff_commits};
use crate::middleware::RateLimiter;
use crate::nostr::NostrClient;
use crate::nostr::events::{BountyClaimBuilder, BountyOfferBuilder, IssueClaimBuilder, PatchBuilder, PullRequestBuilder, RepositoryAnnouncementBuilder, StatusEventBuilder, ZapRequestBuilder};
use crate::views::{agent_marketplace_page, agent_profile_page, agents_list_page, bounties_discovery_page, diff_viewer_page, git_branch_create_form_page, git_status_page, home_page_with_repos, issue_create_form_page, issue_detail_page, issues_list_page, patch_create_form_page, patch_detail_page, patches_list_page, pr_create_form_page, pull_request_detail_page, pull_requests_list_page, repository_create_form_page, repository_detail_page, search_results_page, trajectory_viewer_page};
use crate::ws::{ws_handler, WsBroadcaster};
use nostr::{EventTemplate, Issue, KIND_ISSUE};
use std::time::{SystemTime, UNIX_EPOCH};

/// Application state shared across handlers
pub struct AppState {
    pub broadcaster: Arc<WsBroadcaster>,
    pub nostr_client: Arc<NostrClient>,
    pub identity: Option<Arc<UnifiedIdentity>>,
    pub wallet: Option<Arc<SparkWallet>>,
}

impl AppState {
    /// Sign an event template with the configured identity
    #[allow(dead_code)]
    pub fn sign_event(&self, template: nostr::EventTemplate) -> Result<nostr::Event, String> {
        match &self.identity {
            Some(identity) => {
                identity.sign_event(template)
                    .map_err(|e| format!("Failed to sign event: {}", e))
            }
            None => {
                Err("No identity configured. Set GITAFTER_MNEMONIC environment variable to enable event signing.".to_string())
            }
        }
    }
}

/// Starts server on 127.0.0.1:0, returns the assigned port and server handle
pub async fn start_server(
    broadcaster: Arc<WsBroadcaster>,
    nostr_client: Arc<NostrClient>,
    identity: Option<Arc<UnifiedIdentity>>,
    wallet: Option<Arc<SparkWallet>>,
) -> anyhow::Result<(u16, JoinHandle<Result<(), std::io::Error>>)> {
    let state = web::Data::new(AppState {
        broadcaster,
        nostr_client,
        identity,
        wallet,
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            // Add rate limiting to all POST endpoints to prevent DoS
            .wrap(RateLimiter::new(20)) // 20 requests/second global limit
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
            .route("/repo/{identifier}/issues/{issue_id}/comment", web::post().to(issue_comment))
            .route("/repo/{identifier}/patches", web::get().to(repository_patches))
            .route("/repo/{identifier}/patches/new", web::get().to(patch_create_form))
            .route("/repo/{identifier}/patches", web::post().to(patch_create))
            .route("/repo/{identifier}/patches/{patch_id}", web::get().to(patch_detail))
            .route("/repo/{identifier}/patches/{patch_id}/diff", web::get().to(patch_diff_view))
            .route("/repo/{identifier}/pulls", web::get().to(repository_pulls))
            .route("/repo/{identifier}/pulls/new", web::get().to(pr_create_form))
            .route("/repo/{identifier}/pulls/available-deps", web::get().to(pr_available_deps))
            .route("/repo/{identifier}/pulls/{pr_id}/stack-info", web::get().to(pr_stack_info))
            .route("/repo/{identifier}/pulls", web::post().to(pr_create))
            .route("/repo/{identifier}/pulls/{pr_id}", web::get().to(pull_request_detail))
            .route("/repo/{identifier}/pulls/{pr_id}/diff", web::get().to(pr_diff_view))
            .route("/repo/{identifier}/pulls/{pr_id}/review", web::post().to(pr_review_submit))
            .route("/repo/{identifier}/pulls/{pr_id}/status", web::post().to(pr_status_change))
            .route("/repo/{identifier}/pulls/{pr_id}/auto-checks", web::get().to(pr_auto_checks))
            .route("/trajectory/{session_id}", web::get().to(trajectory_detail))
            .route("/agent/{pubkey}", web::get().to(agent_profile))
            .route("/agents", web::get().to(agents_list))
            .route("/agents/marketplace", web::get().to(agent_marketplace))
            .route("/agent/{pubkey}/reputation", web::post().to(publish_reputation_label))
            .route("/search", web::get().to(search))
            .route("/watched", web::get().to(watched_repositories))
            .route("/repo/{identifier}/watch", web::post().to(watch_repository))
            .route("/repo/{identifier}/unwatch", web::post().to(unwatch_repository))
            .route("/repo/{identifier}/clone", web::post().to(clone_repo))
            .route("/repo/{identifier}/git/status", web::get().to(git_status))
            .route("/repo/{identifier}/git/branch/new", web::get().to(git_branch_form))
            .route("/repo/{identifier}/git/branch", web::post().to(git_branch_create))
            .route("/repo/{identifier}/git/patch/generate", web::post().to(git_patch_generate))
            .route("/repo/{identifier}/git/patch/apply", web::post().to(git_patch_apply))
            .route("/repo/{identifier}/git/push", web::post().to(git_push))
            .route("/bounty/{bounty_claim_id}/pay", web::post().to(bounty_payment))
            .route("/bounties", web::get().to(bounties_discovery))
            .route("/notifications", web::get().to(notifications_page))
            .route("/notifications/unread", web::get().to(unread_notifications_api))
            .route("/notifications/{notification_id}/read", web::post().to(mark_notification_read))
            .route("/notifications/mark-all-read", web::post().to(mark_all_notifications_read))
            .route("/ws", web::get().to(ws_route))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    // Store the server handle for graceful shutdown
    let handle = tokio::spawn(server.run());

    Ok((port, handle))
}

/// Query parameters for repository filtering
#[derive(Debug, serde::Deserialize)]
struct RepoFilterQuery {
    language: Option<String>,
    has_bounties: Option<String>,
    agent_friendly: Option<String>,
}

/// Home page
async fn index(
    state: web::Data<AppState>,
    query: web::Query<RepoFilterQuery>,
) -> HttpResponse {
    // Fetch repositories from cache
    let mut repositories = match state.nostr_client.get_cached_repositories(50).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to fetch repositories: {}", e);
            vec![]
        }
    };

    // Apply filters
    if let Some(language) = &query.language {
        if !language.is_empty() {
            repositories.retain(|repo| {
                repo.tags.iter().any(|tag| {
                    tag.first().map(|t| t == "language").unwrap_or(false)
                        && tag.get(1).map(|l| l.eq_ignore_ascii_case(language)).unwrap_or(false)
                })
            });
        }
    }

    // Build bounty counts per repository if filtering by has_bounties
    let mut repo_bounty_counts = std::collections::HashMap::new();
    let has_bounties_filter = query.has_bounties.as_deref() == Some("true");

    if has_bounties_filter {
        for repo in &repositories {
            let repo_id = repo.tags.iter()
                .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
                .and_then(|tag| tag.get(1).cloned())
                .unwrap_or_default();

            let repo_address = format!("30617:{}:{}", repo.pubkey, repo_id);

            // Get issues for this repo
            if let Ok(issues) = state.nostr_client.get_issues_by_repo(&repo_address, 100).await {
                let mut bounty_count = 0;
                for issue in issues {
                    if let Ok(bounties) = state.nostr_client.get_bounties_for_issue(&issue.id).await {
                        bounty_count += bounties.len();
                    }
                }
                repo_bounty_counts.insert(repo_id.clone(), bounty_count);
            }
        }

        // Filter to only repos with bounties
        repositories.retain(|repo| {
            let repo_id = repo.tags.iter()
                .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
                .and_then(|tag| tag.get(1).cloned())
                .unwrap_or_default();
            repo_bounty_counts.get(&repo_id).copied().unwrap_or(0) > 0
        });
    }

    if query.agent_friendly.as_deref() == Some("true") {
        // Filter repos marked as agent-friendly
        repositories.retain(|repo| {
            repo.tags.iter().any(|tag| {
                tag.first().map(|t| t == "agent-friendly").unwrap_or(false)
            })
        });
    }

    // Pass bounty counts to the view
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(home_page_with_repos(&repositories, &query.language, query.has_bounties.as_deref() == Some("true"), query.agent_friendly.as_deref() == Some("true"), &repo_bounty_counts).into_string())
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
        match get_repository_path(&identifier) {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(_) => None,
        }
    } else {
        None
    };

    // Fetch repository state (kind:30618)
    let repo_state = state.nostr_client.get_repository_state(&identifier).await.ok().flatten();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(repository_detail_page(&repository, is_cloned, local_path, repo_state.as_ref()).into_string())
}

/// Query parameters for issue filtering
#[derive(Debug, serde::Deserialize)]
struct IssueFilterQuery {
    filter_open: Option<String>,
    filter_closed: Option<String>,
    filter_has_bounty: Option<String>,
    filter_claimed: Option<String>,
}

/// Repository issues list page
async fn repository_issues(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<IssueFilterQuery>,
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
    let mut issues = match state.nostr_client.get_issues_by_repo(&repo_address, 100).await {
        Ok(iss) => iss,
        Err(e) => {
            tracing::warn!("Failed to fetch issues: {}", e);
            vec![]
        }
    };

    // Apply filters
    let filter_open = query.filter_open.as_deref() == Some("true");
    let filter_closed = query.filter_closed.as_deref() == Some("true");
    let filter_has_bounty = query.filter_has_bounty.as_deref() == Some("true");
    let filter_claimed = query.filter_claimed.as_deref() == Some("true");

    // Pre-fetch bounties and claims for all issues
    let mut issue_bounties = std::collections::HashMap::new();
    let mut issue_claims = std::collections::HashMap::new();
    let mut issue_first_claims = std::collections::HashMap::new();

    for issue in &issues {
        if filter_has_bounty {
            let bounties = state.nostr_client.get_bounties_for_issue(&issue.id).await.unwrap_or_default();
            issue_bounties.insert(issue.id.clone(), !bounties.is_empty());
        }

        // Always fetch claims to display claim badges
        let claims = state.nostr_client.get_claims_for_issue(&issue.id).await.unwrap_or_default();
        let has_claims = !claims.is_empty();
        issue_claims.insert(issue.id.clone(), has_claims);

        // Store first claim info for badge display
        if has_claims {
            let mut sorted_claims = claims;
            sorted_claims.sort_by_key(|c| c.created_at);
            if let Some(first_claim) = sorted_claims.first() {
                issue_first_claims.insert(issue.id.clone(), first_claim.clone());
            }
        }
    }

    issues.retain(|issue| {
        // Get issue status
        let status = issue.tags.iter()
            .find(|tag| tag.first().map(|t| t == "status").unwrap_or(false))
            .and_then(|tag| tag.get(1))
            .map(|s| s.as_str())
            .unwrap_or("open");

        // Check if issue has bounty
        let has_bounty = issue_bounties.get(&issue.id).copied().unwrap_or(false);

        // Check if issue is claimed
        let is_claimed = issue_claims.get(&issue.id).copied().unwrap_or(false);

        // Apply filter logic
        let status_match = if filter_open && !filter_closed {
            status == "open"
        } else if filter_closed && !filter_open {
            status == "closed"
        } else if filter_open && filter_closed {
            true // show both
        } else {
            status == "open" // default to showing open issues only
        };

        let bounty_match = !filter_has_bounty || has_bounty;
        let claimed_match = !filter_claimed || is_claimed;

        status_match && bounty_match && claimed_match
    });

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(issues_list_page(&repository, &issues, is_watched, &identifier, filter_open, filter_closed, filter_has_bounty, filter_claimed, &issue_first_claims).into_string())
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

    // Fetch comments for this issue (NIP-22)
    let comments = match state.nostr_client.get_comments_for_issue(&issue_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to fetch comments for issue {}: {}", issue_id, e);
            Vec::new() // Continue with empty comments if fetch fails
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(issue_detail_page(&repository, &issue, &claims, &bounties, &comments, &identifier).into_string())
}

/// Claim an issue
async fn issue_claim(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (identifier, issue_id) = path.into_inner();
    let content = form.get("content").cloned().unwrap_or_default();
    let estimate = form.get("estimate").and_then(|s| s.parse::<u64>().ok());

    // Check for existing claims first
    match state.nostr_client.get_claims_for_issue(&issue_id).await {
        Ok(claims) if !claims.is_empty() => {
            // Sort by created_at to find first claim
            let mut sorted_claims = claims;
            sorted_claims.sort_by_key(|c| c.created_at);

            if let Some(first_claim) = sorted_claims.first() {
                let claimer_pubkey = if first_claim.pubkey.len() > 16 {
                    format!("{}...{}", &first_claim.pubkey[..8], &first_claim.pubkey[first_claim.pubkey.len()-8..])
                } else {
                    first_claim.pubkey.clone()
                };

                return HttpResponse::Conflict()
                    .content_type("text/html; charset=utf-8")
                    .body(format!(
                        r#"<div style="padding: 1rem; background: #fef3c7; border-left: 4px solid #f59e0b; color: #92400e;">
                            <h3>⚠️ Issue Already Claimed</h3>
                            <p>This issue was claimed by <strong>{}</strong></p>
                            <p>First claim wins - this claim takes precedence.</p>
                        </div>"#,
                        claimer_pubkey
                    ));
            }
        }
        Ok(_) => {
            // No existing claims, proceed
        }
        Err(e) => {
            tracing::error!("Failed to check existing claims: {}", e);
            // Continue anyway - better to allow claim than block on error
        }
    }

    // Fetch repository to get pubkey
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

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Fetch issue to get author pubkey
    let issue = match state.nostr_client.get_cached_event(&issue_id).await {
        Ok(Some(iss)) => iss,
        Ok(None) => {
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

    let issue_author_pubkey = &issue.pubkey;

    // Build issue claim event
    let mut builder = IssueClaimBuilder::new(
        &issue_id,
        &repo_address,
        issue_author_pubkey,
    );

    if !content.is_empty() {
        builder = builder.content(&content);
    }

    if let Some(est) = estimate {
        builder = builder.estimate(est);
    }

    let event_template = builder.build();

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published issue claim: event_id={}, issue_id={}", event_id, issue_id);

                    // Return success message
                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div class="success-message">
                                <p>✅ Issue claim submitted!</p>
                                <p>Message: {}</p>
                                {}
                            </div>"#,
                            if content.is_empty() { "No message" } else { &content },
                            estimate.map(|e| format!("<p>Estimate: {} seconds</p>", e)).unwrap_or_default()
                        ))
                }
                Err(e) => {
                    tracing::error!("Failed to publish issue claim: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Claim</h3>
                                <p>Error: {}</p>
                            </div>"#,
                            e
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign issue claim event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Claim Event</h3>
                        <p>Error: {}</p>
                    </div>"#,
                    e
                ))
        }
    }
}

/// Create a bounty for an issue
async fn issue_bounty_create(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (identifier, issue_id) = path.into_inner();

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

    // Fetch repository to get pubkey
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

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build bounty offer event
    let mut builder = BountyOfferBuilder::new(
        &issue_id,
        &repo_address,
        amount,
    );

    if let Some(exp) = expiry {
        builder = builder.expiry(exp);
    }

    for condition in &conditions {
        builder = builder.condition(condition);
    }

    let event_template = builder.build();

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published bounty offer: event_id={}, issue_id={}, amount={}", event_id, issue_id, amount);

                    // Return success message
                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
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
                        ))
                }
                Err(e) => {
                    tracing::error!("Failed to publish bounty offer: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Bounty</h3>
                                <p>Error: {}</p>
                            </div>"#,
                            e
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign bounty offer event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Bounty Event</h3>
                        <p>Error: {}</p>
                    </div>"#,
                    e
                ))
        }
    }
}

/// Post a comment on an issue (NIP-22)
async fn issue_comment(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (_identifier, issue_id) = path.into_inner();
    let content = form.get("content").cloned().unwrap_or_default();

    if content.trim().is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Comment cannot be empty</p></div>"#);
    }

    // Fetch issue to get author pubkey for p tag
    let issue = match state.nostr_client.get_cached_event(&issue_id).await {
        Ok(Some(iss)) => iss,
        Ok(None) => {
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

    let issue_author_pubkey = &issue.pubkey;

    // Build NIP-22 comment event (kind:1 with e and p tags)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let event_template = EventTemplate {
        kind: 1, // Text note (NIP-01)
        content: content.clone(),
        tags: vec![
            vec!["e".to_string(), issue_id.clone(), "".to_string(), "root".to_string()],
            vec!["p".to_string(), issue_author_pubkey.clone()],
        ],
        created_at: now,
    };

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published issue comment: event_id={}, issue_id={}", event_id, issue_id);

                    // Return new comment HTML for HTMX to insert
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let commenter_pubkey = if let Some(identity) = &state.identity {
                        let pk = identity.nostr_public_key();
                        if pk.len() > 16 {
                            format!("{}...{}", &pk[..8], &pk[pk.len()-8..])
                        } else {
                            pk.to_string()
                        }
                    } else {
                        "unknown".to_string()
                    };

                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div class="comment-card" style="background: var(--card-bg, #1a1a1a); border: 1px solid var(--border-color, #333); padding: 1rem;">
                                <div class="comment-header" style="display: flex; justify-content: space-between; margin-bottom: 0.75rem;">
                                    <span class="comment-author" style="font-weight: 600; color: var(--accent-color, #0ea5e9);">{}</span>
                                    <span class="comment-time" style="font-size: 0.875rem; color: var(--muted-color, #888);" title="{}">just now</span>
                                </div>
                                <div class="comment-content" style="white-space: pre-wrap;">{}</div>
                            </div>"#,
                            commenter_pubkey,
                            timestamp,
                            content
                        ))
                }
                Err(e) => {
                    tracing::error!("Failed to publish comment: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Comment</h3>
                                <p>Error: {}</p>
                            </div>"#,
                            e
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign comment event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Comment Event</h3>
                        <p>Error: {}</p>
                    </div>"#,
                    e
                ))
        }
    }
}

/// Verify trajectory from Nostr cache by fetching events and comparing hash
async fn verify_trajectory_from_cache(
    nostr_client: &Arc<NostrClient>,
    session_id: &str,
    expected_hash: &str,
) -> anyhow::Result<bool> {
    use crate::trajectory::verifier::calculate_trajectory_hash;

    // Fetch trajectory session (kind:38030)
    let session_event = nostr_client.get_trajectory_session(session_id).await?;
    if session_event.is_none() {
        tracing::warn!("Trajectory session {} not found in cache", session_id);
        return Ok(false);
    }

    // Fetch trajectory events (kind:38031) for this session
    let events = nostr_client.get_trajectory_events(session_id).await?;
    if events.is_empty() {
        tracing::warn!("No trajectory events found for session {}", session_id);
        return Ok(false);
    }

    // Convert events to JSON strings for hash calculation
    let event_jsons: Vec<String> = events
        .iter()
        .map(|e| serde_json::to_string(&e.content).unwrap_or_default())
        .collect();

    // Calculate hash and compare with expected
    let calculated_hash = calculate_trajectory_hash(&event_jsons)?;
    let matches = calculated_hash == expected_hash;

    if matches {
        tracing::info!(
            "Trajectory {} verified successfully ({} events, hash: {}...)",
            session_id,
            events.len(),
            &calculated_hash[..16]
        );
    } else {
        tracing::warn!(
            "Trajectory {} hash mismatch: expected {}..., got {}...",
            session_id,
            &expected_hash[..16],
            &calculated_hash[..16]
        );
    }

    Ok(matches)
}

/// Submit a review for a PR
async fn pr_review_submit(
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (_identifier, _pr_id) = path.into_inner();

    // Extract form data
    let review_type = form.get("review_type").cloned().unwrap_or_else(|| "comment".to_string());
    let content = form.get("content").cloned().unwrap_or_default();
    let trajectory_session_id = form.get("trajectory_session_id").cloned();
    let trajectory_hash = form.get("trajectory_hash").cloned();

    if content.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Review content cannot be empty</p></div>"#);
    }

    // For now, return a success message
    // In a real implementation, this would:
    // 1. Build a kind:1 (text note) event with e tag referencing the PR
    // 2. Add review type tags (approve, request_changes, comment)
    // 3. If agent-authored, add trajectory proof tags
    // 4. Sign it with the user's key
    // 5. Publish it to relays
    // 6. Cache it locally

    let review_emoji = match review_type.as_str() {
        "approve" => "✅",
        "request_changes" => "🔴",
        _ => "💬",
    };

    let is_agent_review = trajectory_session_id.is_some() && trajectory_hash.is_some();

    // Verify trajectory if present
    let verification_badge = if let (Some(session_id), Some(hash)) = (&trajectory_session_id, &trajectory_hash) {
        // Fetch and verify trajectory events from Nostr cache
        match verify_trajectory_from_cache(&state.nostr_client, session_id, hash).await {
            Ok(true) => r#"<span style="color: #48bb78; margin-left: 8px;" title="Trajectory verified">✓ Verified</span>"#,
            Ok(false) => r#"<span style="color: #f59e0b; margin-left: 8px;" title="Trajectory hash mismatch">⚠ Hash Mismatch</span>"#,
            Err(e) => {
                tracing::warn!("Failed to verify trajectory {}: {}", session_id, e);
                r#"<span style="color: #ef4444; margin-left: 8px;" title="Verification failed">✗ Verification Failed</span>"#
            }
        }
    } else {
        ""
    };

    let agent_badge = if is_agent_review {
        format!(r#"<span style="background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 2px 8px; font-size: 0.85em; font-weight: 600; margin-left: 8px;">🤖 AGENT</span>{}"#, verification_badge)
    } else {
        String::new()
    };

    let trajectory_section = if let (Some(session_id), Some(hash)) = (trajectory_session_id, trajectory_hash) {
        format!(
            r#"<div style="margin-top: 12px; padding: 10px; background: #f8f9fa; border-left: 3px solid #667eea;">
                <details>
                    <summary style="cursor: pointer; font-weight: 600; color: #667eea;">🔍 View Agent Reasoning</summary>
                    <div style="margin-top: 8px; font-size: 0.9em;">
                        <p><strong>Session ID:</strong> <code>{}</code></p>
                        <p><strong>Trajectory Hash:</strong> <code>{}</code></p>
                        <p><a href="/trajectory/{}" style="color: #667eea; text-decoration: underline;">View Full Trajectory Timeline</a></p>
                    </div>
                </details>
            </div>"#,
            session_id, hash, session_id
        )
    } else {
        String::new()
    };

    let response_html = format!(
        r#"<div class="success-message">
            <p>{} Review submitted!{}</p>
            <p>Type: {}</p>
            <p>Comment: {}</p>
            {}
        </div>"#,
        review_emoji,
        agent_badge,
        review_type,
        content,
        trajectory_section
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Helper function to extract tag value from event
fn get_tag_value_from_event(event: &nostr::Event, tag_name: &str) -> Option<String> {
    event.tags.iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Try to create a bounty claim when a PR is merged
/// Returns Ok(Some(message)) if bounty claim was created successfully
/// Returns Ok(None) if no bounty was found
/// Returns Err if there was an error
async fn try_create_bounty_claim(
    state: &web::Data<AppState>,
    pr_id: &str,
    repo_address: &str,
) -> Result<Option<String>, String> {
    // 1. Get the PR event to extract trajectory and issue reference
    let pr_event = match state.nostr_client.get_cached_event(pr_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            return Err(format!("PR event {} not found in cache", pr_id));
        }
        Err(e) => {
            return Err(format!("Failed to fetch PR event: {}", e));
        }
    };

    // 2. Extract trajectory session ID and hash from PR tags
    let trajectory_session_id = match get_tag_value_from_event(&pr_event, "trajectory") {
        Some(id) => id,
        None => {
            tracing::debug!("PR {} has no trajectory tag, skipping bounty claim", pr_id);
            return Ok(None);
        }
    };

    let trajectory_hash = get_tag_value_from_event(&pr_event, "trajectory_hash")
        .unwrap_or_else(|| "unknown".to_string());

    // 3. Find issue reference in PR
    // PRs can reference issues via "e" tags with "mention" or "reply" markers
    // Or they might reference issue claims
    let issue_event_id = pr_event.tags.iter()
        .find(|tag| {
            tag.len() >= 2 && tag[0] == "e" &&
            (tag.len() < 4 || (tag.get(3).map(|m| m == "mention" || m == "reply" || m == "root").unwrap_or(false)))
        })
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string());

    let issue_id = match issue_event_id {
        Some(id) => id,
        None => {
            tracing::debug!("PR {} does not reference an issue, skipping bounty claim", pr_id);
            return Ok(None);
        }
    };

    // 4. Check if there's a bounty on this issue
    let bounties = match state.nostr_client.get_bounties_for_issue(&issue_id).await {
        Ok(b) => b,
        Err(e) => {
            return Err(format!("Failed to fetch bounties for issue {}: {}", issue_id, e));
        }
    };

    if bounties.is_empty() {
        tracing::debug!("No bounties found for issue {}", issue_id);
        return Ok(None);
    }

    // 5. Get the first (most recent) bounty
    let bounty = &bounties[0];
    let bounty_id = bounty.id.clone();

    // Extract bounty amount for display
    let bounty_amount = get_tag_value_from_event(bounty, "amount")
        .unwrap_or_else(|| "unknown".to_string());

    // 6. Get Lightning address from identity (if available)
    let lightning_address = state.identity.as_ref()
        .and_then(|_id| {
            // Try to get lud16 from identity metadata
            // For now, we'll leave this optional
            None::<String>
        });

    // 7. Build and publish bounty claim event
    let mut builder = BountyClaimBuilder::new(
        &bounty_id,
        pr_id,
        repo_address,
        &trajectory_session_id,
        &trajectory_hash,
    );

    if let Some(lud16) = lightning_address {
        builder = builder.lightning_address(lud16);
    }

    // Add relay hint for trajectory events
    builder = builder.relay("wss://relay.nostr.bg");

    let bounty_claim_template = builder.build();

    // Sign and publish the bounty claim
    match state.sign_event(bounty_claim_template) {
        Ok(signed_event) => {
            let claim_event_id = signed_event.id.clone();

            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!(
                        "Published bounty claim: claim_id={}, bounty_id={}, pr_id={}, amount={}",
                        claim_event_id, bounty_id, pr_id, bounty_amount
                    );
                    Ok(Some(format!(
                        "Bounty claim created! Amount: {} sats. Claim ID: {}",
                        bounty_amount,
                        &claim_event_id[..8]
                    )))
                }
                Err(e) => {
                    Err(format!("Failed to publish bounty claim event: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to sign bounty claim event: {}", e))
        }
    }
}

/// Change the status of a PR
async fn pr_status_change(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let (identifier, pr_id) = path.into_inner();

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

    // Fetch repository to get pubkey
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

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build status event
    let mut builder = StatusEventBuilder::new(
        &pr_id,
        &repo_address,
        status_kind,
    );

    if !reason.is_empty() {
        builder = builder.reason(&reason);
    }

    let event_template = builder.build();

    // Validate merge order for stacked PRs
    if status_kind == 1631 {
        // Fetch all PRs for this repository to check dependencies
        match state.nostr_client.get_pull_requests_by_repo(&repo_address, 1000).await {
            Ok(prs) => {
                // Find the PR being merged
                if let Some(pr) = prs.iter().find(|p| p.id == pr_id) {
                    // Check if PR has dependencies
                    let depends_on = pr.tags.iter()
                        .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
                        .and_then(|tag| tag.get(1))
                        .map(|s| s.to_string());

                    if let Some(dep_id) = depends_on {
                        // Find dependency PR
                        if let Some(_dep_pr) = prs.iter().find(|p| p.id == dep_id) {
                            // Check if dependency is merged by looking for status events
                            match state.nostr_client.get_pr_status(&dep_id).await {
                                Ok(dep_status) => {
                                    if dep_status != 1631 {
                                        tracing::warn!(
                                            "Attempted to merge PR {} out of order - dependency {} not merged (status: {})",
                                            pr_id,
                                            dep_id,
                                            dep_status
                                        );
                                        return HttpResponse::BadRequest()
                                            .content_type("text/html; charset=utf-8")
                                            .body(format!(
                                                r#"<div class="error-message">
                                                    <p>❌ Cannot merge: dependency PR not merged yet</p>
                                                    <p>This PR depends on: <a href="/repo/{}/pulls/{}">{}</a></p>
                                                    <p>The dependency must be merged before this PR can be merged.</p>
                                                </div>"#,
                                                identifier,
                                                dep_id,
                                                dep_id
                                            ));
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to get dependency PR status: {}", e);
                                    // Continue with merge - better to allow than block on error
                                }
                            }
                        } else {
                            tracing::warn!("Dependency PR {} not found for PR {}", dep_id, pr_id);
                            return HttpResponse::BadRequest()
                                .content_type("text/html; charset=utf-8")
                                .body(format!(
                                    r#"<div class="error-message">
                                        <p>❌ Cannot merge: dependency PR not found</p>
                                        <p>This PR depends on PR {} which no longer exists.</p>
                                    </div>"#,
                                    dep_id
                                ));
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch PRs for merge validation: {}", e);
                // Continue with merge - better to allow than block on error
            }
        }
    }

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published status change: event_id={}, pr_id={}, status={}", event_id, pr_id, status_label);

                    // If PR was merged (status_kind == 1631), try to create bounty claim
                    let mut bounty_claim_message = String::new();
                    if status_kind == 1631 {
                        match try_create_bounty_claim(&state, &pr_id, &repo_address).await {
                            Ok(Some(msg)) => {
                                bounty_claim_message = format!("<p>⚡ {}</p>", msg);
                            }
                            Ok(None) => {
                                // No bounty to claim, this is fine
                                tracing::debug!("No bounty found for merged PR: {}", pr_id);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create bounty claim for PR {}: {}", pr_id, e);
                                bounty_claim_message = format!("<p>⚠️ Could not create bounty claim: {}</p>", e);
                            }
                        }
                    }

                    // Return success message
                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div class="success-message">
                                <p>✅ Status changed to: {}</p>
                                <p>Kind: {}</p>
                                {}
                                {}
                            </div>"#,
                            status_label,
                            status_kind,
                            if !reason.is_empty() {
                                format!("<p>Reason: {}</p>", reason)
                            } else {
                                String::new()
                            },
                            bounty_claim_message
                        ))
                }
                Err(e) => {
                    tracing::error!("Failed to publish status change: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Status Change</h3>
                                <p>Error: {}</p>
                            </div>"#,
                            e
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign status change event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Status Change Event</h3>
                        <p>Error: {}</p>
                    </div>"#,
                    e
                ))
        }
    }
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

    // Build issue content (subject as title, description as content body)
    let content = form.description.as_deref().unwrap_or("").to_string();

    // Create issue using NIP-34 Issue struct
    let mut issue = Issue::new(
        &content,
        &repo_address,
        &repository.pubkey,
    ).with_subject(&form.title);

    // Add labels if provided
    if let Some(labels) = &form.labels {
        for label in labels.lines() {
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                issue = issue.with_label(trimmed);
            }
        }
    }

    // Build event template
    let event_template = EventTemplate {
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        kind: KIND_ISSUE,
        content,
        tags: issue.build_tags(),
    };

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published issue: event_id={}, title='{}'", event_id, form.title);

                    // Redirect to issues list
                    HttpResponse::SeeOther()
                        .insert_header(("Location", format!("/repo/{}/issues", identifier)))
                        .finish()
                }
                Err(e) => {
                    tracing::error!("Failed to publish issue: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Issue</h3>
                                <p>Error: {}</p>
                                <p><a href="/repo/{}/issues/new">← Try Again</a></p>
                            </div>"#,
                            e,
                            identifier
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign issue event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Issue Event</h3>
                        <p>Error: {}</p>
                        <p><a href="/repo/{}/issues/new">← Try Again</a></p>
                    </div>"#,
                    e,
                    identifier
                ))
        }
    }
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

    // Fetch reviews for this patch (patches use same review mechanism as PRs)
    let reviews = match state.nostr_client.get_reviews_for_pr(&patch_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to fetch reviews for patch {}: {}", patch_id, e);
            Vec::new()
        }
    };

    // Fetch reviewer reputation scores
    let mut reviewer_reputations = std::collections::HashMap::new();
    for review in &reviews {
        let reviewer_pubkey = &review.pubkey;
        if !reviewer_reputations.contains_key(reviewer_pubkey) {
            let reputation_labels = match state.nostr_client.get_reputation_labels_for_agent(reviewer_pubkey).await {
                Ok(labels) => labels,
                Err(e) => {
                    tracing::warn!("Failed to fetch reputation for {}: {}", reviewer_pubkey, e);
                    Vec::new()
                }
            };
            let reputation_score = calculate_reputation_score(&reputation_labels);
            reviewer_reputations.insert(reviewer_pubkey.clone(), reputation_score);
        }
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(patch_detail_page(&repository, &patch, &reviews, &reviewer_reputations, &identifier).into_string())
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

    // Fetch inline comment events (NIP-22 comments on the PR that may have line tags)
    // For now, we'll use an empty vector since we need to implement get_comments_for_pr
    let inline_comments = Vec::new();

    // Fetch reviewer reputation scores and calculate weighted reviews
    let mut reviewer_reputations = std::collections::HashMap::new();
    for review in &reviews {
        let reviewer_pubkey = &review.pubkey;
        if !reviewer_reputations.contains_key(reviewer_pubkey) {
            let reputation_labels = match state.nostr_client.get_reputation_labels_for_agent(reviewer_pubkey).await {
                Ok(labels) => labels,
                Err(e) => {
                    tracing::warn!("Failed to fetch reputation for {}: {}", reviewer_pubkey, e);
                    Vec::new()
                }
            };
            let reputation_score = calculate_reputation_score(&reputation_labels);
            reviewer_reputations.insert(reviewer_pubkey.clone(), reputation_score);
        }
    }

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

    // Fetch PRs that depend on this one (later layers)
    let dependent_prs = match state.nostr_client.get_dependent_prs(&pr_id).await {
        Ok(deps) => deps,
        Err(e) => {
            tracing::warn!("Failed to fetch dependent PRs for {}: {}", pr_id, e);
            Vec::new()
        }
    };

    let is_mergeable = match state.nostr_client.is_pr_mergeable(&pull_request).await {
        Ok(mergeable) => mergeable,
        Err(e) => {
            tracing::warn!("Failed to check if PR is mergeable: {}", e);
            false
        }
    };

    // Fetch PR updates (kind:1619)
    let pr_updates = match state.nostr_client.get_pr_updates(&pr_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("Failed to fetch PR updates for {}: {}", pr_id, e);
            Vec::new()
        }
    };

    // Try to fetch diff from locally cloned repo
    let diff_text = {
        // Extract commit ID from PR
        let commit_id = pull_request.tags.iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "c")
            .map(|tag| tag[1].as_str());

        // Extract clone URL to determine local repo path
        let clone_url = pull_request.tags.iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "clone")
            .map(|tag| tag[1].as_str());

        if let (Some(commit_id), Some(_clone_url)) = (commit_id, clone_url) {
            // Try to find cloned repo in workspace
            use crate::git::clone::get_repository_path;
            let repo_path = match get_repository_path(&identifier) {
                Ok(path) => path,
                Err(e) => {
                    tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Invalid repository identifier"
                    }));
                }
            };

            if repo_path.exists() {
                // Try to generate diff for this commit (compare with parent)
                use crate::git::diff::diff_commits;
                match git2::Repository::open(&repo_path) {
                    Ok(repo) => {
                        match git2::Oid::from_str(commit_id) {
                            Ok(oid) => {
                                match repo.find_commit(oid) {
                                    Ok(commit) => {
                                        // Get parent commit (if exists)
                                        if commit.parent_count() > 0 {
                                            match commit.parent(0) {
                                                Ok(parent) => {
                                                    match diff_commits(
                                                        &repo_path,
                                                        &parent.id().to_string(),
                                                        commit_id,
                                                    ) {
                                                        Ok(diff) => Some(diff),
                                                        Err(e) => {
                                                            tracing::warn!("Failed to generate diff for PR {}: {}", pr_id, e);
                                                            None
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!("Failed to get parent commit for {}: {}", commit_id, e);
                                                    None
                                                }
                                            }
                                        } else {
                                            tracing::debug!("Commit {} has no parent (initial commit)", commit_id);
                                            None
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to find commit {}: {}", commit_id, e);
                                        None
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Invalid commit ID {}: {}", commit_id, e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Failed to open repo at {}: {}", repo_path.display(), e);
                        None
                    }
                }
            } else {
                tracing::debug!("Repo not cloned locally: {}", repo_path.display());
                None
            }
        } else {
            tracing::debug!("PR {} missing commit ID or clone URL", pr_id);
            None
        }
    };

    // Fetch bounties for this PR (including per-layer bounties)
    let bounties = if let Some(sid) = stack_id.as_ref() {
        // Get layer info for this PR
        let layer_num = pull_request.tags.iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "layer")
            .and_then(|tag| tag[1].parse::<u32>().ok());

        if let Some(layer) = layer_num {
            // Fetch bounties for this specific layer
            match state.nostr_client.get_bounties_for_layer(sid, layer).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("Failed to fetch layer bounties for stack {} layer {}: {}", sid, layer, e);
                    Vec::new()
                }
            }
        } else {
            // No layer tag, try to get bounties by PR ID
            match state.nostr_client.get_bounties_for_pr(&pr_id).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("Failed to fetch bounties for PR {}: {}", pr_id, e);
                    Vec::new()
                }
            }
        }
    } else {
        // Not part of a stack, get bounties by PR ID
        match state.nostr_client.get_bounties_for_pr(&pr_id).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Failed to fetch bounties for PR {}: {}", pr_id, e);
                Vec::new()
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(pull_request_detail_page(&repository, &pull_request, &reviews, &reviewer_reputations, &status_events, &identifier, trajectory_session.as_ref(), &trajectory_events, &stack_prs, dependency_pr.as_ref(), &dependent_prs, is_mergeable, &pr_updates, diff_text.as_deref(), &inline_comments, &bounties).into_string())
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
        let path = match get_repository_path(&identifier) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
                return HttpResponse::BadRequest()
                    .content_type("text/html; charset=utf-8")
                    .body(r#"<div class="error-message"><p>❌ Invalid repository identifier</p></div>"#);
            }
        };
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
    let dest_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error-message"><p>❌ Invalid repository identifier</p></div>"#);
        }
    };
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

/// API endpoint to get available PRs for dependency selection
async fn pr_available_deps(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository to get pubkey
    let repository = match state.nostr_client.get_repository_by_identifier(&identifier).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("application/json")
                .body(r#"{"error": "Repository not found"}"#);
        }
        Err(e) => {
            tracing::error!("Failed to fetch repository: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(r#"{"error": "Failed to fetch repository"}"#);
        }
    };

    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Fetch all PRs for this repository
    match state.nostr_client.get_pull_requests_by_repo(&repo_address, 1000).await {
        Ok(prs) => {
            // Filter to only unmerged, non-closed PRs
            let mut available: Vec<serde_json::Value> = Vec::new();

            for pr in prs {
                // Get PR status
                match state.nostr_client.get_pr_status(&pr.id).await {
                    Ok(status) => {
                        // Only include Open (1630) and Draft (1633) PRs
                        if status == 1630 || status == 1633 {
                            let subject = pr.tags.iter()
                                .find(|tag| tag.len() >= 2 && tag[0] == "subject")
                                .and_then(|tag| tag.get(1))
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "Untitled PR".to_string());

                            let layer_info = pr.tags.iter()
                                .find(|tag| tag.len() >= 3 && tag[0] == "layer")
                                .and_then(|tag| {
                                    let current = tag.get(1)?;
                                    let total = tag.get(2)?;
                                    Some(format!("Layer {} of {}", current, total))
                                });

                            available.push(serde_json::json!({
                                "id": pr.id,
                                "subject": subject,
                                "status": if status == 1630 { "Open" } else { "Draft" },
                                "layer": layer_info
                            }));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get status for PR {}: {}", pr.id, e);
                    }
                }
            }

            // Return HTML options for HTMX
            let mut html = String::from(r#"<option value="">-- No dependency (base layer) --</option>"#);

            if available.is_empty() {
                html.push_str(r#"<option value="" disabled>No open PRs available</option>"#);
            } else {
                for pr in available {
                    let id = pr["id"].as_str().unwrap_or("");
                    let subject = pr["subject"].as_str().unwrap_or("Untitled PR");
                    let status = pr["status"].as_str().unwrap_or("");
                    let layer = pr["layer"].as_str();

                    let display_text = if let Some(layer_text) = layer {
                        format!("{} - {} ({})", subject, layer_text, status)
                    } else {
                        format!("{} ({})", subject, status)
                    };

                    html.push_str(&format!(
                        r#"<option value="{}">{}</option>"#,
                        id,
                        html_escape::encode_text(&display_text)
                    ));
                }
            }

            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html)
        }
        Err(e) => {
            tracing::error!("Failed to fetch PRs: {}", e);
            HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(r#"{"error": "Failed to fetch pull requests"}"#)
        }
    }
}

/// Get stack info for a PR to auto-fill stack_id and suggest layer number
async fn pr_stack_info(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (_identifier, pr_id) = path.into_inner();

    // Fetch the PR event
    let pr_event = match state.nostr_client.get_cached_event(&pr_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error">PR not found</div>"#);
        }
        Err(e) => {
            tracing::error!("Failed to fetch PR {}: {}", pr_id, e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(r#"<div class="error">Failed to fetch PR</div>"#);
        }
    };

    // Extract stack_id and layer info from tags
    let stack_id = pr_event.tags.iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "stack")
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string());

    let (layer_current, layer_total) = pr_event.tags.iter()
        .find(|tag| tag.len() >= 3 && tag[0] == "layer")
        .and_then(|tag| {
            let current = tag.get(1)?.parse::<u32>().ok()?;
            let total = tag.get(2)?.parse::<u32>().ok()?;
            Some((current, total))
        })
        .unwrap_or((0, 0));

    // Suggest next layer number
    let suggested_layer = if layer_current > 0 {
        layer_current + 1
    } else {
        1
    };

    // Return HTML that updates form fields via HTMX
    let html = if let Some(stack) = stack_id {
        format!(
            r#"<input type="hidden" id="auto_stack_id" value="{}" />
<input type="hidden" id="auto_layer_current" value="{}" />
<input type="hidden" id="auto_layer_total" value="{}" />
<script>
    document.getElementById('stack_id').value = '{}';
    document.getElementById('layer_current').value = '{}';
    document.getElementById('layer_total').value = '{}';
</script>"#,
            html_escape::encode_text(&stack),
            suggested_layer,
            layer_total,
            html_escape::encode_text(&stack),
            suggested_layer,
            layer_total
        )
    } else {
        String::new()
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
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

    // Validate stacked PR dependencies before signing
    if form.depends_on.is_some() || form.stack_id.is_some() {
        // Fetch all PRs for this repository to build dependency graph
        let prs = match state.nostr_client.get_pull_requests_by_repo(&repo_address, 1000).await {
            Ok(prs) => prs,
            Err(e) => {
                tracing::error!("Failed to fetch PRs for validation: {}", e);
                return HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(format!(
                        r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                            <h3>Failed to Validate PR Dependencies</h3>
                            <p>Error: {}</p>
                            <p><a href="/repo/{}/pulls/new">← Try Again</a></p>
                        </div>"#,
                        e,
                        identifier
                    ));
            }
        };

        // Build dependency graph from existing PRs
        if !prs.is_empty() {
            use gitafter::stacks::StackGraph;

            match StackGraph::from_pr_events(&prs) {
                Ok(graph) => {
                    // Create temporary signed event for validation
                    match state.sign_event(event_template.clone()) {
                        Ok(new_pr_event) => {
                            // Validate that the new PR won't create circular dependencies
                            if let Err(e) = graph.validate_new_pr(&new_pr_event) {
                                tracing::warn!("PR dependency validation failed: {}", e);
                                return HttpResponse::BadRequest()
                                    .content_type("text/html; charset=utf-8")
                                    .body(format!(
                                        r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                            <h3>Invalid PR Dependencies</h3>
                                            <p>Error: {}</p>
                                            <p><a href="/repo/{}/pulls/new">← Try Again</a></p>
                                        </div>"#,
                                        e,
                                        identifier
                                    ));
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to sign event for validation: {}", e);
                            // Continue without validation rather than blocking the PR
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to build dependency graph: {}", e);
                    // Continue without validation rather than blocking the PR
                }
            }
        }
    }

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published pull request: event_id={}, subject='{}'", event_id, form.subject);

                    // Redirect to PRs list
                    HttpResponse::SeeOther()
                        .insert_header(("Location", format!("/repo/{}/pulls", identifier)))
                        .finish()
                }
                Err(e) => {
                    tracing::error!("Failed to publish pull request: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Pull Request</h3>
                                <p>Error: {}</p>
                                <p><a href="/repo/{}/pulls/new">← Try Again</a></p>
                            </div>"#,
                            e,
                            identifier
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign pull request event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Pull Request Event</h3>
                        <p>Error: {}</p>
                        <p><a href="/repo/{}/pulls/new">← Try Again</a></p>
                    </div>"#,
                    e,
                    identifier
                ))
        }
    }
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

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published patch: event_id={}, title='{}'", event_id, form.title);

                    // Redirect to patches list
                    HttpResponse::SeeOther()
                        .insert_header(("Location", format!("/repo/{}/patches", identifier)))
                        .finish()
                }
                Err(e) => {
                    tracing::error!("Failed to publish patch: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Patch</h3>
                                <p>Error: {}</p>
                                <p><a href="/repo/{}/patches/new">← Try Again</a></p>
                            </div>"#,
                            e,
                            identifier
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign patch event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Patch Event</h3>
                        <p>Error: {}</p>
                        <p><a href="/repo/{}/patches/new">← Try Again</a></p>
                    </div>"#,
                    e,
                    identifier
                ))
        }
    }
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
    state: web::Data<AppState>,
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

    // Sign and publish event
    match state.sign_event(event_template) {
        Ok(signed_event) => {
            let event_id = signed_event.id.clone();

            // Publish to relays
            match state.nostr_client.publish_event(signed_event).await {
                Ok(_) => {
                    tracing::info!("Published repository announcement: event_id={}", event_id);

                    // Redirect to repository detail page
                    HttpResponse::SeeOther()
                        .insert_header(("Location", format!("/repo/{}", form.identifier)))
                        .finish()
                }
                Err(e) => {
                    tracing::error!("Failed to publish repository event: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                                <h3>Failed to Publish Repository</h3>
                                <p>Error: {}</p>
                                <p><a href="/repo/new">← Try Again</a></p>
                            </div>"#,
                            e
                        ))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to sign repository event: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Failed to Sign Repository Event</h3>
                        <p>Error: {}</p>
                        <p><a href="/repo/new">← Try Again</a></p>
                    </div>"#,
                    e
                ))
        }
    }
}

/// Git status page - shows local changes
async fn git_status(
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    // Get file status
    match get_status(&repo_path) {
        Ok(changes) => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(git_status_page(&identifier, &changes).into_string())
        }
        Err(e) => {
            tracing::error!("Failed to get git status: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to get status: {}</p>", e))
        }
    }
}

/// Git branch creation form
async fn git_branch_form(
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(git_branch_create_form_page(&identifier).into_string())
}

/// Create a new git branch
async fn git_branch_create(
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let identifier = path.into_inner();
    let branch_name = match form.get("branch_name") {
        Some(name) if !name.is_empty() => name,
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1><p>Branch name is required</p>");
        }
    };

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    match create_branch(&repo_path, branch_name) {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Branch Created</h3>
                        <p>Successfully created branch: {}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                    branch_name, identifier
                ))
        }
        Err(e) => {
            tracing::error!("Failed to create branch: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Error Creating Branch</h3>
                        <p>{}</p>
                        <p><a href="/repo/{}/git/branch/new">← Try Again</a></p>
                    </div>"#,
                    e, identifier
                ))
        }
    }
}

/// Generate patch from local commits
async fn git_patch_generate(
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let identifier = path.into_inner();
    let base_commit = match form.get("base_commit") {
        Some(c) if !c.is_empty() => c,
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1><p>Base commit ID is required</p>");
        }
    };

    let head_commit = match form.get("head_commit") {
        Some(c) if !c.is_empty() => c,
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1><p>Head commit ID is required</p>");
        }
    };

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    match generate_patch(&repo_path, base_commit, head_commit) {
        Ok(patch) => {
            HttpResponse::Ok()
                .content_type("text/plain; charset=utf-8")
                .body(patch)
        }
        Err(e) => {
            tracing::error!("Failed to generate patch: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to generate patch: {}</p>", e))
        }
    }
}

/// Apply a patch to local repository
async fn git_patch_apply(
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let identifier = path.into_inner();
    let patch_content = match form.get("patch") {
        Some(p) if !p.is_empty() => p,
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error</h1><p>Patch content is required</p>");
        }
    };

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    match apply_patch(&repo_path, patch_content) {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Patch Applied</h3>
                        <p>Successfully applied patch to repository</p>
                        <p><a href="/repo/{}/git/status">View Changes →</a></p>
                    </div>"#,
                    identifier
                ))
        }
        Err(e) => {
            tracing::error!("Failed to apply patch: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Error Applying Patch</h3>
                        <p>{}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                    e, identifier
                ))
        }
    }
}

/// Push local branch to remote
async fn git_push(
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let identifier = path.into_inner();
    let remote_name = form.get("remote").cloned().unwrap_or_else(|| "origin".to_string());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    // Get current branch name
    let branch_name = match current_branch(&repo_path) {
        Ok(name) => name,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to get current branch: {}</p>", e));
        }
    };

    match push_branch(&repo_path, &remote_name, &branch_name) {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Push Successful</h3>
                        <p>Pushed branch {} to {}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                    branch_name, remote_name, identifier
                ))
        }
        Err(e) => {
            tracing::error!("Failed to push branch: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #dc2626;">
                        <h3>Error Pushing Branch</h3>
                        <p>{}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                    e, identifier
                ))
        }
    }
}

/// View syntax-highlighted diff for a patch
async fn patch_diff_view(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, patch_id) = path.into_inner();

    // Fetch patch event from cache
    let patch_event = match state.nostr_client.get_cached_event(&patch_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
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

    // Extract commit IDs from patch tags
    let commit_id = patch_event.tags.iter()
        .find(|tag| tag.first().map(|t| t == "c").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    let parent_commit = patch_event.tags.iter()
        .find(|tag| tag.first().map(|t| t == "parent").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned</h1><p>Clone the repository to view diffs.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    // Generate diff
    let diff_output = match (parent_commit.as_ref(), commit_id.as_ref()) {
        (Some(parent), Some(commit)) => {
            match diff_commits(&repo_path, parent, commit) {
                Ok(diff) => diff,
                Err(e) => {
                    return HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!("<h1>Error generating diff</h1><p>{}</p>", e));
                }
            }
        }
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Missing commit information</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(diff_viewer_page(&identifier, &patch_id, "patch", &diff_output).into_string())
}

/// View syntax-highlighted diff for a PR
async fn pr_diff_view(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, pr_id) = path.into_inner();

    // Fetch PR event from cache
    let pr_event = match state.nostr_client.get_cached_event(&pr_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Pull request not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch PR: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching PR</h1>");
        }
    };

    // Extract commit IDs from PR tags
    let commit_id = pr_event.tags.iter()
        .find(|tag| tag.first().map(|t| t == "c").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    let parent_commit = pr_event.tags.iter()
        .find(|tag| tag.first().map(|t| t == "parent").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned</h1><p>Clone the repository to view diffs.</p>");
    }

    let repo_path = match get_repository_path(&identifier) { Ok(p) => p, Err(e) => { tracing::warn!("Invalid repository identifier {}: {}", identifier, e); return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid repository identifier"})); } };

    // Generate diff
    let diff_output = match (parent_commit.as_ref(), commit_id.as_ref()) {
        (Some(parent), Some(commit)) => {
            match diff_commits(&repo_path, parent, commit) {
                Ok(diff) => diff,
                Err(e) => {
                    return HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!("<h1>Error generating diff</h1><p>{}</p>", e));
                }
            }
        }
        _ => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Missing commit information</h1>");
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(diff_viewer_page(&identifier, &pr_id, "pr", &diff_output).into_string())
}

/// Query parameters for agent filtering
#[derive(Debug, serde::Deserialize)]
struct AgentFilterQuery {
    min_reputation: Option<i32>,
    min_merged_prs: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
struct AgentMarketplaceQuery {
    specialty: Option<String>,
    min_reputation: Option<i32>,
    search: Option<String>,
}

/// Agents list page with filtering
async fn agents_list(
    state: web::Data<AppState>,
    query: web::Query<AgentFilterQuery>,
) -> HttpResponse {
    // Fetch all unique agent pubkeys from PRs and issue claims
    let mut agent_pubkeys = std::collections::HashSet::new();

    // Get agents from pull requests
    if let Ok(prs) = state.nostr_client.get_cached_pull_requests(100).await {
        for pr in prs {
            agent_pubkeys.insert(pr.pubkey.clone());
        }
    }

    // Get agents from issue claims (query all cached issues and get their claims)
    if let Ok(issues) = state.nostr_client.get_cached_issues(100).await {
        for issue in issues {
            if let Ok(claims) = state.nostr_client.get_claims_for_issue(&issue.id).await {
                for claim in claims {
                    agent_pubkeys.insert(claim.pubkey.clone());
                }
            }
        }
    }

    // Build agent data with reputation
    let mut agents = Vec::new();
    for pubkey in agent_pubkeys {
        // Fetch reputation labels
        let reputation_labels = state.nostr_client
            .get_reputation_labels_for_agent(&pubkey)
            .await
            .unwrap_or_default();

        // Calculate reputation score
        let reputation_score = calculate_reputation_score(&reputation_labels);

        // Count merged PRs
        let merged_prs = state.nostr_client
            .get_pull_requests_by_agent(&pubkey, 100)
            .await
            .unwrap_or_default()
            .iter()
            .filter(|pr| {
                // Check for merged status events
                pr.tags.iter().any(|tag| {
                    tag.first().map(|t| t == "status").unwrap_or(false)
                        && tag.get(1).map(|s| s == "1631").unwrap_or(false)
                })
            })
            .count() as i32;

        // Apply filters
        if let Some(min_rep) = query.min_reputation {
            if reputation_score < min_rep {
                continue;
            }
        }

        if let Some(min_prs) = query.min_merged_prs {
            if merged_prs < min_prs {
                continue;
            }
        }

        agents.push((pubkey, reputation_score, merged_prs));
    }

    // Sort by reputation score descending
    agents.sort_by(|a, b| b.1.cmp(&a.1));

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(agents_list_page(&agents, &query.min_reputation, &query.min_merged_prs).into_string())
}

/// Agent marketplace page - discover agents by specialty
async fn agent_marketplace(
    state: web::Data<AppState>,
    query: web::Query<AgentMarketplaceQuery>,
) -> HttpResponse {
    // Fetch all unique agent pubkeys from PRs and issue claims
    let mut agent_pubkeys = std::collections::HashSet::new();

    // Get agents from pull requests
    if let Ok(prs) = state.nostr_client.get_cached_pull_requests(100).await {
        for pr in prs {
            agent_pubkeys.insert(pr.pubkey.clone());
        }
    }

    // Get agents from issue claims
    if let Ok(issues) = state.nostr_client.get_cached_issues(100).await {
        for issue in issues {
            if let Ok(claims) = state.nostr_client.get_claims_for_issue(&issue.id).await {
                for claim in claims {
                    agent_pubkeys.insert(claim.pubkey.clone());
                }
            }
        }
    }

    // Build agent data with reputation and specialties
    let mut agents_by_specialty: std::collections::HashMap<String, Vec<(String, i32, i32)>> = std::collections::HashMap::new();
    let mut all_specialties = std::collections::HashSet::new();

    for pubkey in agent_pubkeys {
        // Fetch reputation labels
        let reputation_labels = state.nostr_client
            .get_reputation_labels_for_agent(&pubkey)
            .await
            .unwrap_or_default();

        // Calculate reputation score
        let reputation_score = calculate_reputation_score(&reputation_labels);

        // Count merged PRs
        let merged_prs = state.nostr_client
            .get_pull_requests_by_agent(&pubkey, 100)
            .await
            .unwrap_or_default()
            .iter()
            .filter(|pr| {
                pr.tags.iter().any(|tag| {
                    tag.first().map(|t| t == "status").unwrap_or(false)
                        && tag.get(1).map(|s| s == "1631").unwrap_or(false)
                })
            })
            .count() as i32;

        // Extract specialties from agent profile or reputation labels
        // For now, use a simple heuristic: check reputation labels for specialty tags
        let mut agent_specialties = vec!["general".to_string()]; // Default specialty

        // Try to extract specialties from PR languages (basic heuristic)
        if let Ok(prs) = state.nostr_client.get_pull_requests_by_agent(&pubkey, 10).await {
            for pr in prs {
                // Look for common file extensions in PR content or tags
                if pr.content.contains(".rs") || pr.content.contains("rust") {
                    agent_specialties.push("rust".to_string());
                }
                if pr.content.contains(".ts") || pr.content.contains("typescript") {
                    agent_specialties.push("typescript".to_string());
                }
                if pr.content.contains(".py") || pr.content.contains("python") {
                    agent_specialties.push("python".to_string());
                }
                if pr.content.contains(".js") || pr.content.contains("javascript") {
                    agent_specialties.push("javascript".to_string());
                }
                if pr.content.contains(".go") || pr.content.contains("golang") {
                    agent_specialties.push("go".to_string());
                }
            }
        }

        // Deduplicate specialties
        agent_specialties.sort();
        agent_specialties.dedup();

        // Add agent to each specialty group
        for specialty in &agent_specialties {
            all_specialties.insert(specialty.clone());
            agents_by_specialty
                .entry(specialty.clone())
                .or_default()
                .push((pubkey.clone(), reputation_score, merged_prs));
        }
    }

    // Sort all_specialties alphabetically
    let mut sorted_specialties: Vec<String> = all_specialties.into_iter().collect();
    sorted_specialties.sort();

    // Sort agents within each specialty by reputation
    for agents in agents_by_specialty.values_mut() {
        agents.sort_by(|a, b| b.1.cmp(&a.1));
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(agent_marketplace_page(
            &agents_by_specialty,
            &sorted_specialties,
            query.specialty.as_deref(),
            query.min_reputation,
            query.search.as_deref(),
        ).into_string())
}

/// Publish a reputation label for an agent
async fn publish_reputation_label(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let target_pubkey = path.into_inner();
    let label = form.get("label").cloned().unwrap_or_default();
    let rating = form.get("rating").and_then(|r| r.parse::<i32>().ok());

    // Build reputation label event (kind:1985)
    let mut tags = vec![
        vec!["p".to_string(), target_pubkey.clone()],
        vec!["L".to_string(), "agent.reputation".to_string()],
        vec!["l".to_string(), label.clone(), "agent.reputation".to_string()],
    ];

    if let Some(rating_val) = rating {
        tags.push(vec!["rating".to_string(), rating_val.to_string()]);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template = EventTemplate {
        kind: 1985,
        content: format!("Reputation label for agent: {}", label),
        tags,
        created_at: now,
    };

    // Sign and publish
    match state.sign_event(template) {
        Ok(event) => {
            match state.nostr_client.publish_event(event).await {
                Ok(_) => {
                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                                <h3>Reputation Published</h3>
                                <p>Successfully published reputation label for agent</p>
                                <p><a href="/agent/{}">← Back to Agent</a></p>
                            </div>"#,
                            target_pubkey
                        ))
                }
                Err(e) => {
                    tracing::error!("Failed to publish reputation label: {}", e);
                    HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body(format!("<h1>Error</h1><p>Failed to publish: {}</p>", e))
                }
            }
        }
        Err(e) => {
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to sign event: {}</p>", e))
        }
    }
}

/// Calculate reputation score from labels
fn calculate_reputation_score(labels: &[nostr::Event]) -> i32 {
    let mut score = 0;

    for label in labels {
        // Extract rating tag
        if let Some(rating_str) = label.tags.iter()
            .find(|tag| tag.first().map(|t| t == "rating").unwrap_or(false))
            .and_then(|tag| tag.get(1))
        {
            if let Ok(rating) = rating_str.parse::<i32>() {
                score += rating;
            }
        }
    }

    score
}

/// Process bounty payment via NIP-57 zap
///
/// This handler receives a bounty claim ID, fetches the claim details,
/// extracts the recipient's Lightning address, builds a zap request,
/// and initiates the payment flow.
///
/// NOTE: This is a simplified implementation that creates the zap request
/// but does not complete the full LNURL payment flow. Full implementation
/// requires:
/// 1. Fetching recipient's LNURL endpoint from their profile
/// 2. Making HTTP request to LNURL callback with zap request
/// 3. Receiving Lightning invoice from callback
/// 4. Paying invoice via wallet integration
/// 5. Waiting for zap receipt (kind:9735) from recipient's wallet
///
/// For now, this creates the zap request and returns a placeholder response.
async fn bounty_payment(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let bounty_claim_id = path.into_inner();

    // 1. Fetch bounty claim event
    let claim_event = match state.nostr_client.get_cached_event(&bounty_claim_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Bounty claim not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch bounty claim: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching bounty claim</h1>");
        }
    };

    // 2. Extract bounty ID from claim (e tag referencing kind:1636)
    let bounty_id = match get_tag_value_from_event(&claim_event, "e") {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Invalid claim: missing bounty reference</h1>");
        }
    };

    // 3. Fetch bounty offer event to get amount
    let bounty_event = match state.nostr_client.get_cached_event(&bounty_id).await {
        Ok(Some(event)) => event,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Bounty offer not found</h1>");
        }
        Err(e) => {
            tracing::error!("Failed to fetch bounty offer: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Error fetching bounty offer</h1>");
        }
    };

    // 4. Extract bounty amount (in sats)
    let amount_sats = match get_tag_value_from_event(&bounty_event, "amount") {
        Some(amount_str) => match amount_str.parse::<u64>() {
            Ok(amt) => amt,
            Err(_) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html; charset=utf-8")
                    .body("<h1>Invalid bounty amount</h1>");
            }
        },
        None => {
            return HttpResponse::BadRequest()
                .content_type("text/html; charset=utf-8")
                .body("<h1>Bounty has no amount specified</h1>");
        }
    };

    // 5. Get recipient pubkey from claim event (the author of the claim)
    let recipient_pubkey = claim_event.pubkey.clone();

    // 6. Try to get recipient's Lightning address from claim
    let recipient_lud16 = get_tag_value_from_event(&claim_event, "lud16");

    // Use lud16 from claim (full LNURL implementation requires fetching from kind:0 profile)
    let lud16 = recipient_lud16;

    // 7. Build zap request
    let zap_builder = ZapRequestBuilder::new(&recipient_pubkey)
        .amount_sats(amount_sats)
        .relay("wss://relay.damus.io")
        .relay("wss://relay.snort.social")
        .event(&bounty_claim_id)  // Zap the bounty claim event
        .content(format!("Bounty payment: {} sats", amount_sats));

    let zap_template = zap_builder.build();

    // 8. Sign the zap request
    match state.sign_event(zap_template) {
        Ok(signed_zap_request) => {
            let zap_request_json = match serde_json::to_string_pretty(&signed_zap_request) {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!("Failed to serialize zap request: {}", e);
                    return HttpResponse::InternalServerError()
                        .content_type("text/html; charset=utf-8")
                        .body("<h1>Error creating payment</h1>");
                }
            };

            // 9. Check if wallet is available for payment
            let payment_result = if let Some(wallet) = &state.wallet {
                // Try to pay via bolt11 if invoice is provided in the bounty claim
                if let Some(invoice) = get_tag_value_from_event(&claim_event, "invoice") {
                    // Invoice provided - pay it directly
                    match wallet.send_payment_simple(&invoice, None).await {
                        Ok(response) => {
                            tracing::info!(
                                "✓ Payment successful for bounty claim {}: {} sats",
                                &bounty_claim_id[..8],
                                amount_sats
                            );
                            Some(Ok(response))
                        }
                        Err(e) => {
                            tracing::error!("✗ Payment failed: {}", e);
                            Some(Err(e))
                        }
                    }
                } else {
                    // No invoice - need LNURL flow (not yet implemented)
                    tracing::warn!(
                        "Cannot pay bounty claim {} - LNURL flow not implemented. Need 'invoice' tag with bolt11.",
                        &bounty_claim_id[..8]
                    );
                    None
                }
            } else {
                tracing::warn!("Wallet not configured - bounty payment skipped");
                None
            };

            tracing::info!(
                "Created zap request for bounty claim {}: {} sats to {}",
                &bounty_claim_id[..8],
                amount_sats,
                &recipient_pubkey[..8]
            );

            // Return payment confirmation UI
            let payment_status_html = match payment_result {
                Some(Ok(_response)) => {
                    r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981; margin-top: 1rem;">
                        <p><strong>✓ Payment Successful</strong></p>
                        <p>Bounty payment has been sent via Lightning Network.</p>
                    </div>"#.to_string()
                }
                Some(Err(e)) => {
                    format!(
                        r#"<div style="padding: 1rem; background: #fee2e2; border-left: 4px solid #ef4444; margin-top: 1rem;">
                            <p><strong>✗ Payment Failed</strong></p>
                            <p>Error: {}</p>
                            <p>The bounty claim zap request was created but payment could not be completed.</p>
                        </div>"#,
                        e
                    )
                }
                None => {
                    r#"<div style="padding: 1rem; background: #fef3c7; border-left: 4px solid #f59e0b; margin-top: 1rem;">
                        <p><strong>⚠️ Payment Not Completed</strong></p>
                        <p>Payment requires one of:</p>
                        <ul>
                            <li>An 'invoice' tag with bolt11 invoice in the bounty claim event, OR</li>
                            <li>A configured Spark wallet + LNURL implementation (coming soon)</li>
                        </ul>
                        <p>The zap request (NIP-57) has been created and can be used for manual payment.</p>
                    </div>"#.to_string()
                }
            };

            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<!DOCTYPE html>
                    <html>
                    <head>
                        <meta charset="utf-8">
                        <title>Bounty Payment</title>
                        <style>{}</style>
                    </head>
                    <body>
                        <main>
                            <h2>⚡ Bounty Payment</h2>
                            <div style="padding: 1rem; background: #f0fdf4; border-left: 4px solid #22c55e;">
                                <p><strong>Bounty Claim:</strong> {}</p>
                                <p><strong>Amount:</strong> {} sats</p>
                                <p><strong>Recipient:</strong> {}...{}</p>
                                {}
                            </div>

                            {}

                            <details style="margin-top: 1rem;">
                                <summary style="cursor: pointer; font-weight: bold;">Show Zap Request (NIP-57)</summary>
                                <pre style="background: #1e1e1e; color: #d4d4d4; padding: 1rem; overflow-x: auto; margin-top: 0.5rem;">{}</pre>
                            </details>

                            <div style="margin-top: 1rem;">
                                <a href="javascript:history.back()">← Back</a>
                            </div>
                        </main>
                    </body>
                    </html>"#,
                    include_str!("./styles.css"),
                    &bounty_claim_id[..16],
                    amount_sats,
                    &recipient_pubkey[..8],
                    &recipient_pubkey[recipient_pubkey.len()-8..],
                    if let Some(lud16) = lud16 {
                        format!("<p><strong>Lightning Address:</strong> {}</p>", lud16)
                    } else {
                        "".to_string()
                    },
                    payment_status_html,
                    zap_request_json
                ))
        }
        Err(e) => {
            tracing::error!("Failed to sign zap request: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to sign payment request: {}</p>", e))
        }
    }
}

/// Bounty discovery page - list all bounties across all repositories
async fn bounties_discovery(state: web::Data<AppState>) -> HttpResponse {
    // Fetch all repositories
    let repositories = match state.nostr_client.get_cached_repositories(100).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to fetch repositories: {}", e);
            vec![]
        }
    };

    // Collect all bounties with their issue details
    let mut all_bounties = Vec::new();

    for repo in &repositories {
        let repo_id = repo.tags.iter()
            .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
            .and_then(|tag| tag.get(1).cloned())
            .unwrap_or_default();

        let repo_name = get_tag_value_from_event(repo, "name")
            .unwrap_or_else(|| "Unnamed".to_string());

        let repo_address = format!("30617:{}:{}", repo.pubkey, repo_id);

        // Get issues for this repo
        if let Ok(issues) = state.nostr_client.get_issues_by_repo(&repo_address, 100).await {
            for issue in issues {
                // Get bounties for this issue
                if let Ok(bounties) = state.nostr_client.get_bounties_for_issue(&issue.id).await {
                    for bounty in bounties {
                        let amount = get_tag_value_from_event(&bounty, "amount")
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);

                        let issue_subject = issue.tags.iter()
                            .find(|tag| tag.first().map(|t| t == "subject").unwrap_or(false))
                            .and_then(|tag| tag.get(1).cloned())
                            .unwrap_or_else(|| "Untitled Issue".to_string());

                        all_bounties.push((
                            repo_name.clone(),
                            repo_id.clone(),
                            issue_subject,
                            issue.id.clone(),
                            amount,
                            bounty.id.clone(),
                        ));
                    }
                }
            }
        }
    }

    // Sort by amount descending
    all_bounties.sort_by(|a, b| b.4.cmp(&a.4));

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(bounties_discovery_page(&all_bounties).into_string())
}

/// Handler for notifications page
async fn notifications_page(state: web::Data<AppState>) -> HttpResponse {
    // Get current user's pubkey from identity
    let user_pubkey = match &state.identity {
        Some(identity) => identity.nostr_public_key().to_string(),
        None => {
            return HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body("<html><body><h1>No identity configured</h1><p>Set GITAFTER_MNEMONIC to view notifications</p></body></html>");
        }
    };

    // Fetch all notifications for this user
    let notifications = match state.nostr_client.get_notifications(&user_pubkey, 50).await {
        Ok(notifs) => notifs,
        Err(e) => {
            tracing::error!("Failed to fetch notifications: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(crate::views::notifications_page(&notifications).into_string())
}

/// API endpoint for unread notifications count
async fn unread_notifications_api(state: web::Data<AppState>) -> HttpResponse {
    let user_pubkey = match &state.identity {
        Some(identity) => identity.nostr_public_key().to_string(),
        None => {
            return HttpResponse::Ok()
                .content_type("application/json")
                .body(r#"{"count": 0}"#);
        }
    };

    let count = match state.nostr_client.get_unread_count(&user_pubkey).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get unread count: {}", e);
            0
        }
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(format!(r#"{{"count": {}}}"#, count))
}

/// Handler for marking a notification as read
async fn mark_notification_read(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let notification_id = path.into_inner();

    match state.nostr_client.mark_notification_read(&notification_id).await {
        Ok(_) => HttpResponse::Ok()
            .content_type("application/json")
            .body(r#"{"success": true}"#),
        Err(e) => {
            tracing::error!("Failed to mark notification as read: {}", e);
            HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(r#"{"success": false}"#)
        }
    }
}

/// Handler for marking all notifications as read
async fn mark_all_notifications_read(state: web::Data<AppState>) -> HttpResponse {
    let user_pubkey = match &state.identity {
        Some(identity) => identity.nostr_public_key().to_string(),
        None => {
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .body(r#"{"success": false, "error": "No identity configured"}"#);
        }
    };

    match state.nostr_client.mark_all_notifications_read(&user_pubkey).await {
        Ok(count) => HttpResponse::Ok()
            .content_type("application/json")
            .body(format!(r#"{{"success": true, "count": {}}}"#, count)),
        Err(e) => {
            tracing::error!("Failed to mark all notifications as read: {}", e);
            HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(r#"{"success": false}"#)
        }
    }
}

/// Run automated checks for a PR
async fn pr_auto_checks(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    use gitafter::review::AutoCheckRunner;

    let (identifier, pr_id) = path.into_inner();

    // Fetch the PR to get trajectory and dependency info
    let pull_request = match state.nostr_client.get_cached_event(&pr_id).await {
        Ok(Some(pr)) => pr,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("application/json")
                .body(r#"{"error": "Pull request not found"}"#);
        }
        Err(e) => {
            tracing::error!("Failed to fetch PR: {}", e);
            return HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(r#"{"error": "Failed to fetch pull request"}"#);
        }
    };

    // Check if repo is cloned locally
    let repo_path = if is_repository_cloned(&identifier) {
        match get_repository_path(&identifier) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
                return HttpResponse::BadRequest()
                    .content_type("application/json")
                    .body(r#"{"error": "Invalid repository identifier"}"#);
            }
        }
    } else {
        // Can't run local checks without cloned repo
        return HttpResponse::Ok()
            .content_type("application/json")
            .body(r#"{"checks": [], "message": "Repository not cloned locally - clone to enable automated checks"}"#);
    };

    // Extract trajectory session ID
    let trajectory_session_id = crate::views::get_tag_value(&pull_request, "trajectory");

    // Extract dependencies
    let depends_on = pull_request.tags.iter()
        .filter(|tag| tag.first().map(|t| t == "depends_on").unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect::<Vec<_>>();

    // Build and run auto-check runner
    let mut runner = AutoCheckRunner::new(&repo_path, &pr_id);

    if let Some(trajectory_id) = trajectory_session_id {
        runner = runner.with_trajectory(trajectory_id);
    }

    if !depends_on.is_empty() {
        runner = runner.with_dependencies(depends_on);
    }

    let results = runner.run_all().await;

    // Render results as HTML component
    use maud::{html, Markup};
    use gitafter::review::CheckStatus;

    let html_body: Markup = html! {
        div.check-results {
            @if results.is_empty() {
                div.info-message style="padding: 1rem; background: #fef3c7; border-left: 4px solid #f59e0b;" {
                    "⚠️ Repository not cloned locally. Clone to enable automated checks."
                }
            } @else {
                @for check in &results {
                    div.check-item style=(format!(
                        "padding: 0.75rem 1rem; margin-bottom: 0.5rem; background: #1e293b; border-left: 3px solid {};",
                        match check.status {
                            CheckStatus::Pass => "#10b981",
                            CheckStatus::Fail => "#ef4444",
                            CheckStatus::Skip => "#6b7280",
                            CheckStatus::Running => "#3b82f6",
                            CheckStatus::Pending => "#94a3b8",
                        }
                    )) {
                        div style="display: flex; justify-content: space-between; align-items: center;" {
                            div {
                                span.check-status style="font-weight: 600; margin-right: 0.75rem;" {
                                    (check.status.to_string())
                                }
                                span.check-name style="color: #e2e8f0;" {
                                    (check.name)
                                }
                            }
                            @if let Some(duration) = check.duration_ms {
                                span.check-duration style="font-size: 0.875rem; color: #94a3b8;" {
                                    (format!("{}ms", duration))
                                }
                            }
                        }
                        @if let Some(message) = &check.message {
                            div.check-message style="margin-top: 0.5rem; font-size: 0.875rem; color: #cbd5e1; padding-left: 2.5rem;" {
                                (message)
                            }
                        }
                    }
                }

                div.check-summary style="margin-top: 1.5rem; padding: 1rem; background: #0f172a; display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 1rem;" {
                    @let pass_count = results.iter().filter(|c| matches!(c.status, CheckStatus::Pass)).count();
                    @let fail_count = results.iter().filter(|c| matches!(c.status, CheckStatus::Fail)).count();
                    @let skip_count = results.iter().filter(|c| matches!(c.status, CheckStatus::Skip)).count();

                    div.summary-item {
                        p style="margin: 0; font-size: 0.75rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em;" { "Passed" }
                        p style="margin: 0.25rem 0 0 0; font-size: 1.5rem; font-weight: 700; color: #10b981;" { (pass_count) }
                    }
                    div.summary-item {
                        p style="margin: 0; font-size: 0.75rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em;" { "Failed" }
                        p style="margin: 0.25rem 0 0 0; font-size: 1.5rem; font-weight: 700; color: #ef4444;" { (fail_count) }
                    }
                    div.summary-item {
                        p style="margin: 0; font-size: 0.75rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em;" { "Skipped" }
                        p style="margin: 0.25rem 0 0 0; font-size: 1.5rem; font-weight: 700; color: #6b7280;" { (skip_count) }
                    }
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_body.into_string())
}
