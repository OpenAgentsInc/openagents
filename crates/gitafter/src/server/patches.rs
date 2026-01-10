/// Repository patches list page
async fn repository_patches(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    let repo_address = format!(
        "30617:{}:{}",
        repository.pubkey,
        repository
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Fetch patches for this repository
    let patches = match state
        .nostr_client
        .get_patches_by_repo(&repo_address, 100)
        .await
    {
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
async fn repository_pulls(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    let repo_address = format!(
        "30617:{}:{}",
        repository.pubkey,
        repository
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Fetch pull requests for this repository
    let pull_requests = match state
        .nostr_client
        .get_pull_requests_by_repo(&repo_address, 100)
        .await
    {
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
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
            let reputation_labels = match state
                .nostr_client
                .get_reputation_labels_for_agent(reviewer_pubkey)
                .await
            {
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
        .body(
            patch_detail_page(
                &repository,
                &patch,
                &reviews,
                &reviewer_reputations,
                &identifier,
            )
            .into_string(),
        )
}

/// Pull request detail page
async fn pull_request_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, pr_id) = path.into_inner();

    // Fetch repository from cache by identifier
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
            let reputation_labels = match state
                .nostr_client
                .get_reputation_labels_for_agent(reviewer_pubkey)
                .await
            {
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
    let trajectory_session_id = pull_request
        .tags
        .iter()
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
                tracing::debug!(
                    "Found {} trajectory events for session {}",
                    events.len(),
                    session.id
                );
                events
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch trajectory events for session {}: {}",
                    session.id,
                    e
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Fetch stacked diff metadata
    let stack_id = pull_request
        .tags
        .iter()
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
        let commit_id = pull_request
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "c")
            .map(|tag| tag[1].as_str());

        // Extract clone URL to determine local repo path
        let clone_url = pull_request
            .tags
            .iter()
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
                                                            tracing::warn!(
                                                                "Failed to generate diff for PR {}: {}",
                                                                pr_id,
                                                                e
                                                            );
                                                            None
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Failed to get parent commit for {}: {}",
                                                        commit_id,
                                                        e
                                                    );
                                                    None
                                                }
                                            }
                                        } else {
                                            tracing::debug!(
                                                "Commit {} has no parent (initial commit)",
                                                commit_id
                                            );
                                            None
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to find commit {}: {}",
                                            commit_id,
                                            e
                                        );
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
        let layer_num = pull_request
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "layer")
            .and_then(|tag| tag[1].parse::<u32>().ok());

        if let Some(layer) = layer_num {
            // Fetch bounties for this specific layer
            match state.nostr_client.get_bounties_for_layer(sid, layer).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch layer bounties for stack {} layer {}: {}",
                        sid,
                        layer,
                        e
                    );
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
        .body(
            pull_request_detail_page(
                &repository,
                &pull_request,
                &reviews,
                &reviewer_reputations,
                &status_events,
                &identifier,
                trajectory_session.as_ref(),
                &trajectory_events,
                &stack_prs,
                dependency_pr.as_ref(),
                &dependent_prs,
                is_mergeable,
                &pr_updates,
                diff_text.as_deref(),
                &inline_comments,
                &bounties,
            )
            .into_string(),
        )
}

/// Trajectory detail page
async fn trajectory_detail(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
            tracing::warn!(
                "Failed to fetch trajectory events for session {}: {}",
                session_id,
                e
            );
            Vec::new() // Continue with empty events if fetch fails
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(trajectory_viewer_page(&session, &events).into_string())
}

/// Agent profile page
async fn agent_profile(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let pubkey = path.into_inner();

    // Fetch pull requests by this agent
    let pull_requests = match state
        .nostr_client
        .get_pull_requests_by_agent(&pubkey, 50)
        .await
    {
        Ok(prs) => prs,
        Err(e) => {
            tracing::warn!("Failed to fetch pull requests for agent {}: {}", pubkey, e);
            Vec::new()
        }
    };

    // Fetch issue claims by this agent
    let issue_claims = match state
        .nostr_client
        .get_issue_claims_by_agent(&pubkey, 50)
        .await
    {
        Ok(claims) => claims,
        Err(e) => {
            tracing::warn!("Failed to fetch issue claims for agent {}: {}", pubkey, e);
            Vec::new()
        }
    };

    // Fetch reputation labels for this agent
    let reputation_labels = match state
        .nostr_client
        .get_reputation_labels_for_agent(&pubkey)
        .await
    {
        Ok(labels) => labels,
        Err(e) => {
            tracing::warn!(
                "Failed to fetch reputation labels for agent {}: {}",
                pubkey,
                e
            );
            Vec::new()
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            agent_profile_page(&pubkey, &pull_requests, &issue_claims, &reputation_labels)
                .into_string(),
        )
}

/// Watch a repository
async fn watch_repository(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Get repository to extract address
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    let repo_address = format!(
        "30617:{}:{}",
        repository.pubkey,
        repository
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
            .unwrap_or(&identifier)
    );

    // Watch the repository
    if let Err(e) = state
        .nostr_client
        .watch_repository(&identifier, &repo_address)
        .await
    {
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
async fn unwatch_repository(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
async fn watched_repositories(state: web::Data<AppState>) -> HttpResponse {
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
async fn clone_repo(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository from cache to get clone URL
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    let clone_url = repository
        .tags
        .iter()
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
                .body(
                    r#"<div class="error-message"><p>❌ Invalid repository identifier</p></div>"#,
                );
        }
    };
    match clone_repository(&clone_url, &dest_path, None) {
        Ok(_) => {
            tracing::info!(
                "Successfully cloned repository {} to {:?}",
                identifier,
                dest_path
            );
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
async fn pr_available_deps(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch repository to get pubkey
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    match state
        .nostr_client
        .get_pull_requests_by_repo(&repo_address, 1000)
        .await
    {
        Ok(prs) => {
            // Filter to only unmerged, non-closed PRs
            let mut available: Vec<serde_json::Value> = Vec::new();

            for pr in prs {
                // Get PR status
                match state.nostr_client.get_pr_status(&pr.id).await {
                    Ok(status) => {
                        // Only include Open (1630) and Draft (1633) PRs
                        if status == 1630 || status == 1633 {
                            let subject = pr
                                .tags
                                .iter()
                                .find(|tag| tag.len() >= 2 && tag[0] == "subject")
                                .and_then(|tag| tag.get(1))
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "Untitled PR".to_string());

                            let layer_info = pr
                                .tags
                                .iter()
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
            let mut html =
                String::from(r#"<option value="">-- No dependency (base layer) --</option>"#);

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
    let stack_id = pr_event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "stack")
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string());

    let (layer_current, layer_total) = pr_event
        .tags
        .iter()
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

