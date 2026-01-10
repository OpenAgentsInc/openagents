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
            .route(
                "/repo/{identifier}/issues",
                web::get().to(repository_issues),
            )
            .route(
                "/repo/{identifier}/issues/new",
                web::get().to(issue_create_form),
            )
            .route("/repo/{identifier}/issues", web::post().to(issue_create))
            .route(
                "/repo/{identifier}/issues/{issue_id}",
                web::get().to(issue_detail),
            )
            .route(
                "/repo/{identifier}/issues/{issue_id}/claim",
                web::post().to(issue_claim),
            )
            .route(
                "/repo/{identifier}/issues/{issue_id}/bounty",
                web::post().to(issue_bounty_create),
            )
            .route(
                "/repo/{identifier}/issues/{issue_id}/comment",
                web::post().to(issue_comment),
            )
            .route(
                "/repo/{identifier}/patches",
                web::get().to(repository_patches),
            )
            .route(
                "/repo/{identifier}/patches/new",
                web::get().to(patch_create_form),
            )
            .route("/repo/{identifier}/patches", web::post().to(patch_create))
            .route(
                "/repo/{identifier}/patches/{patch_id}",
                web::get().to(patch_detail),
            )
            .route(
                "/repo/{identifier}/patches/{patch_id}/diff",
                web::get().to(patch_diff_view),
            )
            .route("/repo/{identifier}/pulls", web::get().to(repository_pulls))
            .route(
                "/repo/{identifier}/pulls/new",
                web::get().to(pr_create_form),
            )
            .route(
                "/repo/{identifier}/pulls/available-deps",
                web::get().to(pr_available_deps),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/stack-info",
                web::get().to(pr_stack_info),
            )
            .route("/repo/{identifier}/pulls", web::post().to(pr_create))
            .route(
                "/repo/{identifier}/pulls/{pr_id}",
                web::get().to(pull_request_detail),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/diff",
                web::get().to(pr_diff_view),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/review",
                web::post().to(pr_review_submit),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/status",
                web::post().to(pr_status_change),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/auto-checks",
                web::get().to(pr_auto_checks),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/checklist",
                web::get().to(pr_checklist),
            )
            .route(
                "/repo/{identifier}/pulls/{pr_id}/checklist/{item_id}",
                web::post().to(pr_checklist_toggle),
            )
            .route("/trajectory/{session_id}", web::get().to(trajectory_detail))
            .route("/agent/{pubkey}", web::get().to(agent_profile))
            .route("/agents", web::get().to(agents_list))
            .route("/agents/marketplace", web::get().to(agent_marketplace))
            .route(
                "/agent/{pubkey}/reputation",
                web::post().to(publish_reputation_label),
            )
            .route("/search", web::get().to(search))
            .route("/watched", web::get().to(watched_repositories))
            .route("/repo/{identifier}/watch", web::post().to(watch_repository))
            .route(
                "/repo/{identifier}/unwatch",
                web::post().to(unwatch_repository),
            )
            .route("/repo/{identifier}/clone", web::post().to(clone_repo))
            .route("/repo/{identifier}/git/status", web::get().to(git_status))
            .route(
                "/repo/{identifier}/git/branch/new",
                web::get().to(git_branch_form),
            )
            .route(
                "/repo/{identifier}/git/branch",
                web::post().to(git_branch_create),
            )
            .route(
                "/repo/{identifier}/git/patch/generate",
                web::post().to(git_patch_generate),
            )
            .route(
                "/repo/{identifier}/git/patch/apply",
                web::post().to(git_patch_apply),
            )
            .route("/repo/{identifier}/git/push", web::post().to(git_push))
            .route(
                "/bounty/{bounty_claim_id}/pay",
                web::post().to(bounty_payment),
            )
            .route("/bounties", web::get().to(bounties_discovery))
            .route("/notifications", web::get().to(notifications_page))
            .route(
                "/notifications/unread",
                web::get().to(unread_notifications_api),
            )
            .route(
                "/notifications/{notification_id}/read",
                web::post().to(mark_notification_read),
            )
            .route(
                "/notifications/mark-all-read",
                web::post().to(mark_all_notifications_read),
            )
            .route("/ws", web::get().to(ws_route))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    // Store the server handle for graceful shutdown
    let handle = tokio::spawn(server.run());

    Ok((port, handle))
}

