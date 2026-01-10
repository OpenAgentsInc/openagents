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
        let repo_id = repo
            .tags
            .iter()
            .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
            .and_then(|tag| tag.get(1).cloned())
            .unwrap_or_default();

        let repo_name =
            get_tag_value_from_event(repo, "name").unwrap_or_else(|| "Unnamed".to_string());

        let repo_address = format!("30617:{}:{}", repo.pubkey, repo_id);

        // Get issues for this repo
        if let Ok(issues) = state
            .nostr_client
            .get_issues_by_repo(&repo_address, 100)
            .await
        {
            for issue in issues {
                // Get bounties for this issue
                if let Ok(bounties) = state.nostr_client.get_bounties_for_issue(&issue.id).await {
                    for bounty in bounties {
                        let amount = get_tag_value_from_event(&bounty, "amount")
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);

                        let issue_subject = issue
                            .tags
                            .iter()
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

    match state
        .nostr_client
        .mark_notification_read(&notification_id)
        .await
    {
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

    match state
        .nostr_client
        .mark_all_notifications_read(&user_pubkey)
        .await
    {
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
    use crate::review::AutoCheckRunner;

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
    let depends_on = pull_request
        .tags
        .iter()
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
    use crate::review::CheckStatus;
    use maud::{Markup, html};

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

/// Get review checklist for a PR
async fn pr_checklist(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    use crate::review::{AutoCheckRunner, CheckStatus, ChecklistGenerator, ReviewTemplate};

    let (identifier, pr_id) = path.into_inner();

    // Fetch the PR to get changed files
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

    // Extract changed files from PR event tags
    let changed_files: Vec<String> = pull_request
        .tags
        .iter()
        .filter(|tag| tag.first().map(|t| t == "file").unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect();

    // Determine review template from PR tags
    let template = if pull_request.tags.iter().any(|tag| {
        tag.first()
            .map(|t| t == "label" || t == "type")
            .unwrap_or(false)
            && tag
                .get(1)
                .map(|v| v == "nip" || v.starts_with("nip"))
                .unwrap_or(false)
    }) {
        ReviewTemplate::NipImplementation
    } else if pull_request.tags.iter().any(|tag| {
        tag.first()
            .map(|t| t == "label" || t == "type")
            .unwrap_or(false)
            && tag
                .get(1)
                .map(|v| v == "bugfix" || v == "bug")
                .unwrap_or(false)
    }) {
        ReviewTemplate::BugFix
    } else if pull_request.tags.iter().any(|tag| {
        tag.first()
            .map(|t| t == "label" || t == "type")
            .unwrap_or(false)
            && tag.get(1).map(|v| v == "refactor").unwrap_or(false)
    }) {
        ReviewTemplate::Refactor
    } else if pull_request.tags.iter().any(|tag| {
        tag.first()
            .map(|t| t == "label" || t == "type")
            .unwrap_or(false)
            && tag.get(1).map(|v| v == "feature").unwrap_or(false)
    }) {
        ReviewTemplate::Feature
    } else {
        ReviewTemplate::General
    };

    // Generate checklist
    let mut checklist = ChecklistGenerator::generate(&changed_files, template);

    // Check if repo is cloned to run auto-checks
    if is_repository_cloned(&identifier) {
        if let Ok(repo_path) = get_repository_path(&identifier) {
            // Run auto-checks and populate auto-check results
            let runner = AutoCheckRunner::new(&repo_path, &pr_id);
            let check_results = runner.run_all().await;

            // Match auto-check results to checklist items
            for item in &mut checklist {
                if item.auto_checkable {
                    // Find matching check result
                    let matching_result = check_results.iter().find(|r| {
                        // Match by ID patterns
                        r.id == item.id
                            || (item.id == "rust-clippy" && r.id == "compilation")
                            || (item.id == "rust-tests" && r.id == "tests")
                            || (item.id == "d012-no-stubs" && r.id == "compilation")
                            || (item.id == "d013-tests" && r.id == "tests")
                    });

                    if let Some(result) = matching_result {
                        item.set_auto_result(result.status.to_string());
                        if matches!(result.status, CheckStatus::Pass) {
                            item.check();
                        }
                    }
                }
            }
        }
    }

    // Render as HTML
    use maud::{Markup, html};

    let html_body: Markup = html! {
        div.review-checklist style="padding: 1rem;" {
            h2 style="margin: 0 0 1rem 0; color: #f1f5f9; font-size: 1.25rem;" { "Review Checklist" }

            @if checklist.is_empty() {
                div.info-message style="padding: 1rem; background: #1e293b; border-left: 4px solid #64748b;" {
                    "No specific checklist items for this PR."
                }
            } @else {
                // Group by category
                @let categories = ["code", "tests", "docs", "security", "performance"];
                @for category in categories {
                    @let items: Vec<_> = checklist.iter().filter(|i| i.category == *category).collect();
                    @if !items.is_empty() {
                        div.checklist-category style="margin-bottom: 1.5rem;" {
                            h3 style="margin: 0 0 0.75rem 0; color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em;" {
                                (category)
                            }
                            @for item in items {
                                div.checklist-item style=(format!(
                                    "padding: 0.75rem; margin-bottom: 0.5rem; background: #1e293b; border-left: 3px solid {};",
                                    if item.required { "#f59e0b" } else { "#64748b" }
                                )) {
                                    div style="display: flex; align-items: flex-start; gap: 0.75rem;" {
                                        input type="checkbox"
                                            id=(format!("check-{}", item.id))
                                            name=(format!("check-{}", item.id))
                                            checked[item.checked]
                                            style="margin-top: 0.25rem; width: 1rem; height: 1rem; cursor: pointer;"
                                            hx-post=(format!("/repo/{}/pulls/{}/checklist/{}", identifier, pr_id, item.id))
                                            hx-swap="none";

                                        div style="flex: 1;" {
                                            label for=(format!("check-{}", item.id)) style="color: #e2e8f0; cursor: pointer;" {
                                                (item.description)
                                                @if item.required {
                                                    span style="margin-left: 0.5rem; color: #f59e0b; font-size: 0.75rem;" { "*" }
                                                }
                                            }

                                            @if let Some(ref result) = item.auto_result {
                                                div.auto-result style="margin-top: 0.5rem; font-size: 0.75rem; color: #94a3b8; padding: 0.25rem 0.5rem; background: #0f172a; display: inline-block;" {
                                                    "Auto-check: " (result)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div.checklist-summary style="margin-top: 1.5rem; padding: 1rem; background: #0f172a;" {
                    @let total = checklist.len();
                    @let checked = checklist.iter().filter(|i| i.checked).count();
                    @let required = checklist.iter().filter(|i| i.required).count();
                    @let required_checked = checklist.iter().filter(|i| i.required && i.checked).count();

                    p style="margin: 0; color: #cbd5e1;" {
                        strong { (checked) } " of " strong { (total) } " items checked"
                    }
                    @if required > 0 {
                        p style="margin: 0.5rem 0 0 0; color: #cbd5e1;" {
                            strong { (required_checked) } " of " strong { (required) } " required items checked"
                        }
                    }
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_body.into_string())
}

/// Toggle checklist item
async fn pr_checklist_toggle(
    _state: web::Data<AppState>,
    path: web::Path<(String, String, String)>,
) -> HttpResponse {
    let (_identifier, _pr_id, _item_id) = path.into_inner();

    // In a full implementation, this would:
    // 1. Store checklist state in database
    // 2. Toggle the specific item
    // 3. Return success/failure
    //
    // For now, we'll just return success since the checkbox
    // state is managed client-side

    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"success": true}"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::Mnemonic;
    use openagents_spark::{
        Network, PaymentStatus as SparkStatus, PaymentType as SparkType, SparkSigner, SparkWallet,
        WalletConfig,
    };
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;
    use testing::{MockRelay, RegtestFaucet};

    #[tokio::test]
    async fn test_pr_review_submit_publishes_review_event() {
        let relay = MockRelay::start().await;
        let broadcaster = Arc::new(WsBroadcaster::new(64));
        let nostr_client =
            Arc::new(NostrClient::new(vec![relay.url().to_string()], broadcaster.clone()).unwrap());
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let identity = Arc::new(UnifiedIdentity::from_mnemonic(mnemonic).unwrap());

        let state = web::Data::new(AppState {
            broadcaster,
            nostr_client: nostr_client.clone(),
            identity: Some(identity.clone()),
            wallet: None,
        });

        let pr_template = EventTemplate {
            kind: 1618,
            content: "PR body".to_string(),
            tags: vec![
                vec![
                    "a".to_string(),
                    format!("30617:{}:test-repo", identity.nostr_public_key()),
                ],
                vec!["subject".to_string(), "Test PR".to_string()],
                vec!["c".to_string(), "commit-123".to_string()],
                vec![
                    "clone".to_string(),
                    "https://example.com/repo.git".to_string(),
                ],
            ],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let pr_event = identity.sign_event(pr_template).unwrap();
        nostr_client.publish_event(pr_event.clone()).await.unwrap();

        let mut form = std::collections::HashMap::new();
        form.insert("review_type".to_string(), "approve".to_string());
        form.insert("content".to_string(), "LGTM".to_string());

        let response = pr_review_submit(
            web::Path::from(("test-repo".to_string(), pr_event.id.clone())),
            web::Form(form),
            state,
        )
        .await;

        assert_eq!(response.status(), actix_web::http::StatusCode::OK);

        let reviews = nostr_client.get_reviews_for_pr(&pr_event.id).await.unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].content, "LGTM");
        assert!(
            reviews[0]
                .tags
                .iter()
                .any(|tag| tag.len() >= 2 && tag[0] == "review_type" && tag[1] == "approve")
        );
    }

    #[test]
    fn test_filter_repositories_by_language_and_topic() {
        let repo_rust_nostr = nostr::Event {
            id: "repo-1".to_string(),
            kind: 30617,
            pubkey: "pubkey1".to_string(),
            created_at: 1,
            content: "Repo 1".to_string(),
            tags: vec![
                vec!["d".to_string(), "repo-1".to_string()],
                vec!["language".to_string(), "rust".to_string()],
                vec!["topic".to_string(), "nostr".to_string()],
            ],
            sig: "sig1".to_string(),
        };

        let repo_python_ai = nostr::Event {
            id: "repo-2".to_string(),
            kind: 30617,
            pubkey: "pubkey2".to_string(),
            created_at: 2,
            content: "Repo 2".to_string(),
            tags: vec![
                vec!["d".to_string(), "repo-2".to_string()],
                vec!["language".to_string(), "python".to_string()],
                vec!["t".to_string(), "ai".to_string()],
            ],
            sig: "sig2".to_string(),
        };

        let repo_rust_tooling = nostr::Event {
            id: "repo-3".to_string(),
            kind: 30617,
            pubkey: "pubkey3".to_string(),
            created_at: 3,
            content: "Repo 3".to_string(),
            tags: vec![
                vec!["d".to_string(), "repo-3".to_string()],
                vec!["language".to_string(), "rust".to_string()],
                vec!["topic".to_string(), "tooling".to_string()],
            ],
            sig: "sig3".to_string(),
        };

        let mut repos = vec![
            repo_rust_nostr.clone(),
            repo_python_ai.clone(),
            repo_rust_tooling.clone(),
        ];

        let query = RepoFilterQuery {
            language: Some("rust".to_string()),
            topic: None,
            has_bounties: None,
            agent_friendly: None,
        };
        filter_repositories_by_query(&mut repos, &query);
        assert_eq!(repos.len(), 2);

        let mut repos = vec![
            repo_rust_nostr.clone(),
            repo_python_ai.clone(),
            repo_rust_tooling.clone(),
        ];
        let query = RepoFilterQuery {
            language: None,
            topic: Some("ai".to_string()),
            has_bounties: None,
            agent_friendly: None,
        };
        filter_repositories_by_query(&mut repos, &query);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "repo-2");

        let mut repos = vec![repo_rust_nostr, repo_python_ai, repo_rust_tooling];
        let query = RepoFilterQuery {
            language: Some("rust".to_string()),
            topic: Some("tooling".to_string()),
            has_bounties: None,
            agent_friendly: None,
        };
        filter_repositories_by_query(&mut repos, &query);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "repo-3");
    }

    const FAUCET_SENDER_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const FAUCET_RECEIVER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    struct RealE2eConfig {
        sender_mnemonic: String,
        receiver_mnemonic: String,
        amount_sats: u64,
        network: Network,
        api_key: Option<String>,
        timeout: Duration,
        use_faucet: bool,
    }

    fn env_any(keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| std::env::var(key).ok())
    }

    fn parse_network(value: &str) -> Option<Network> {
        match value.to_ascii_lowercase().as_str() {
            "mainnet" => Some(Network::Mainnet),
            "testnet" => Some(Network::Testnet),
            "signet" => Some(Network::Signet),
            "regtest" => Some(Network::Regtest),
            _ => None,
        }
    }

    fn real_e2e_config() -> Option<RealE2eConfig> {
        let use_faucet = env_any(&[
            "GITAFTER_E2E_USE_FAUCET",
            "MARKETPLACE_E2E_USE_FAUCET",
            "SPARK_E2E_USE_FAUCET",
        ])
        .is_some();
        let sender_env = env_any(&[
            "GITAFTER_E2E_SENDER_MNEMONIC",
            "MARKETPLACE_E2E_SENDER_MNEMONIC",
            "SPARK_E2E_SENDER_MNEMONIC",
        ]);
        let receiver_env = env_any(&[
            "GITAFTER_E2E_RECEIVER_MNEMONIC",
            "MARKETPLACE_E2E_RECEIVER_MNEMONIC",
            "SPARK_E2E_RECEIVER_MNEMONIC",
        ]);
        let (sender_mnemonic, receiver_mnemonic) = match (sender_env, receiver_env) {
            (Some(sender), Some(receiver)) => (sender, receiver),
            _ if use_faucet => (
                FAUCET_SENDER_MNEMONIC.to_string(),
                FAUCET_RECEIVER_MNEMONIC.to_string(),
            ),
            _ => return None,
        };

        let amount_sats = env_any(&[
            "GITAFTER_E2E_AMOUNT_SATS",
            "MARKETPLACE_E2E_AMOUNT_SATS",
            "SPARK_E2E_AMOUNT_SATS",
        ])
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);

        let timeout = env_any(&[
            "GITAFTER_E2E_TIMEOUT_SECS",
            "MARKETPLACE_E2E_TIMEOUT_SECS",
            "SPARK_E2E_TIMEOUT_SECS",
        ])
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(180));

        let network = env_any(&[
            "GITAFTER_E2E_NETWORK",
            "MARKETPLACE_E2E_NETWORK",
            "SPARK_E2E_NETWORK",
        ])
        .and_then(|value| parse_network(&value))
        .unwrap_or(Network::Testnet);

        if network == Network::Mainnet && std::env::var("GITAFTER_E2E_ALLOW_MAINNET").is_err() {
            println!(
                "Skipping mainnet GitAfter E2E test - set GITAFTER_E2E_ALLOW_MAINNET=1 to enable"
            );
            return None;
        }
        if network == Network::Mainnet && use_faucet {
            println!(
                "Skipping mainnet GitAfter E2E test - faucet funding only supported on regtest"
            );
            return None;
        }

        let api_key = env_any(&[
            "GITAFTER_E2E_API_KEY",
            "MARKETPLACE_E2E_API_KEY",
            "SPARK_E2E_API_KEY",
            "BREEZ_API_KEY",
        ]);

        Some(RealE2eConfig {
            sender_mnemonic,
            receiver_mnemonic,
            amount_sats,
            network,
            api_key,
            timeout,
            use_faucet,
        })
    }

    fn unique_storage_dir(label: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "openagents-gitafter-e2e-{}-{}-{}",
            label,
            std::process::id(),
            now
        ));
        std::fs::create_dir_all(&dir).expect("should create gitafter e2e storage dir");
        dir
    }

    async fn wait_for_min_balance(
        wallet: &SparkWallet,
        min_sats: u64,
        timeout: Duration,
    ) -> Result<(), anyhow::Error> {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let balance = wallet.get_balance().await?;
            if balance.total_sats() >= min_sats {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "timed out waiting for balance >= {} sats",
                    min_sats
                ));
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn ensure_funded(
        wallet: &SparkWallet,
        min_balance: u64,
        timeout: Duration,
    ) -> Result<(), anyhow::Error> {
        let balance = wallet.get_balance().await?;
        if balance.total_sats() >= min_balance {
            return Ok(());
        }

        let needed = min_balance.saturating_sub(balance.total_sats());
        let request_amount = needed.clamp(10_000, 50_000);
        let deposit_address = wallet.get_bitcoin_address().await?;

        let faucet = RegtestFaucet::new()?;
        faucet
            .fund_address(&deposit_address, request_amount)
            .await?;
        wait_for_min_balance(wallet, balance.total_sats().saturating_add(1), timeout).await?;

        Ok(())
    }

    async fn wait_for_payment_amount(
        wallet: &SparkWallet,
        payment_type: SparkType,
        amount_sats: u64,
        timeout: Duration,
    ) -> Result<openagents_spark::Payment, anyhow::Error> {
        let deadline = tokio::time::Instant::now() + timeout;
        let amount = amount_sats as u128;

        loop {
            let payments = wallet.list_payments(Some(50), Some(0)).await?;
            if let Some(payment) = payments.into_iter().find(|p| {
                p.payment_type == payment_type
                    && p.amount == amount
                    && p.status == SparkStatus::Completed
            }) {
                return Ok(payment);
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "timed out waiting for {:?} payment of {} sats",
                    payment_type,
                    amount_sats
                ));
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    #[tokio::test]
    #[ignore = "Requires funded Spark testnet wallets"]
    async fn test_bounty_claim_payout_real_sats() {
        let Some(config) = real_e2e_config() else {
            println!(
                "Skipping GitAfter E2E test - set GITAFTER_E2E_SENDER_MNEMONIC/GITAFTER_E2E_RECEIVER_MNEMONIC or GITAFTER_E2E_USE_FAUCET=1"
            );
            return;
        };

        if config.amount_sats == 0 {
            println!("Skipping GitAfter E2E test - amount must be > 0");
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        unsafe {
            std::env::set_var("XDG_DATA_HOME", temp_dir.path());
            std::env::set_var("HOME", temp_dir.path());
        }

        let broadcaster = Arc::new(WsBroadcaster::new(64));
        let nostr_client =
            Arc::new(NostrClient::new(vec![], broadcaster.clone()).expect("nostr client"));

        let identity_mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
            .expect("identity mnemonic");
        let identity =
            Arc::new(UnifiedIdentity::from_mnemonic(identity_mnemonic).expect("identity"));

        let sender_signer =
            SparkSigner::from_mnemonic(&config.sender_mnemonic, "").expect("sender signer");
        let receiver_signer =
            SparkSigner::from_mnemonic(&config.receiver_mnemonic, "").expect("receiver signer");

        let sender_wallet = Arc::new(
            SparkWallet::new(
                sender_signer,
                WalletConfig {
                    network: config.network,
                    api_key: config.api_key.clone(),
                    storage_dir: unique_storage_dir("sender"),
                },
            )
            .await
            .expect("sender wallet"),
        );

        let receiver_wallet = Arc::new(
            SparkWallet::new(
                receiver_signer,
                WalletConfig {
                    network: config.network,
                    api_key: config.api_key.clone(),
                    storage_dir: unique_storage_dir("receiver"),
                },
            )
            .await
            .expect("receiver wallet"),
        );

        if config.use_faucet {
            if let Err(error) =
                ensure_funded(&sender_wallet, config.amount_sats, config.timeout).await
            {
                println!(
                    "Skipping GitAfter E2E test - faucet funding failed: {}",
                    error
                );
                return;
            }
        } else {
            let sender_balance_before = sender_wallet.get_balance().await.expect("sender balance");
            if sender_balance_before.total_sats() < config.amount_sats {
                println!("Sender wallet requires funding before running this test");
                return;
            }
        }

        let invoice = receiver_wallet
            .create_invoice(
                config.amount_sats,
                Some("GitAfter bounty payout".to_string()),
                Some(3600),
            )
            .await
            .expect("invoice");

        let repo_address = format!("30617:{}:test-repo", identity.nostr_public_key());
        let issue_template = EventTemplate {
            kind: KIND_ISSUE,
            tags: vec![
                vec!["a".to_string(), repo_address.clone()],
                vec!["subject".to_string(), "Test issue".to_string()],
            ],
            content: "Issue used for bounty payout test".to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        let issue_event = identity.sign_event(issue_template).expect("issue event");
        nostr_client
            .publish_event_no_retry(issue_event.clone())
            .await
            .expect("cache issue");

        let bounty_offer_template = BountyOfferBuilder::new(
            issue_event.id.clone(),
            repo_address.clone(),
            config.amount_sats,
        )
        .build();
        let bounty_offer_event = identity
            .sign_event(bounty_offer_template)
            .expect("bounty offer");
        nostr_client
            .publish_event_no_retry(bounty_offer_event.clone())
            .await
            .expect("cache bounty");

        let bounty_claim_template = BountyClaimBuilder::new(
            bounty_offer_event.id.clone(),
            "merged-pr-event-id",
            repo_address.clone(),
            "trajectory-session-id",
            "trajectory-hash",
        )
        .lightning_address("agent@example.com")
        .invoice(invoice.payment_request.clone())
        .build();
        let bounty_claim_event = identity
            .sign_event(bounty_claim_template)
            .expect("bounty claim");
        nostr_client
            .publish_event_no_retry(bounty_claim_event.clone())
            .await
            .expect("cache claim");

        let state = web::Data::new(AppState {
            broadcaster,
            nostr_client,
            identity: Some(identity),
            wallet: Some(sender_wallet.clone()),
        });

        let response = bounty_payment(state, web::Path::from(bounty_claim_event.id.clone())).await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);

        wait_for_payment_amount(
            &sender_wallet,
            SparkType::Send,
            config.amount_sats,
            config.timeout,
        )
        .await
        .expect("sender payment should complete");

        wait_for_payment_amount(
            &receiver_wallet,
            SparkType::Receive,
            config.amount_sats,
            config.timeout,
        )
        .await
        .expect("receiver payment should complete");
    }
}
