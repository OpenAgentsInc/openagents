/// Git status page - shows local changes
async fn git_status(path: web::Path<String>) -> HttpResponse {
    let identifier = path.into_inner();

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    // Get file status
    match get_status(&repo_path) {
        Ok(changes) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(git_status_page(&identifier, &changes).into_string()),
        Err(e) => {
            tracing::error!("Failed to get git status: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!("<h1>Error</h1><p>Failed to get status: {}</p>", e))
        }
    }
}

/// Git branch creation form
async fn git_branch_form(path: web::Path<String>) -> HttpResponse {
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

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    match create_branch(&repo_path, branch_name) {
        Ok(_) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(format!(
                r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Branch Created</h3>
                        <p>Successfully created branch: {}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                branch_name, identifier
            )),
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

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    match generate_patch(&repo_path, base_commit, head_commit) {
        Ok(patch) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(patch),
        Err(e) => {
            tracing::error!("Failed to generate patch: {}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    "<h1>Error</h1><p>Failed to generate patch: {}</p>",
                    e
                ))
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

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    match apply_patch(&repo_path, patch_content) {
        Ok(_) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(format!(
                r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Patch Applied</h3>
                        <p>Successfully applied patch to repository</p>
                        <p><a href="/repo/{}/git/status">View Changes →</a></p>
                    </div>"#,
                identifier
            )),
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
    let remote_name = form
        .get("remote")
        .cloned()
        .unwrap_or_else(|| "origin".to_string());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned locally</h1><p>Clone the repository first.</p>");
    }

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    // Get current branch name
    let branch_name = match current_branch(&repo_path) {
        Ok(name) => name,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    "<h1>Error</h1><p>Failed to get current branch: {}</p>",
                    e
                ));
        }
    };

    match push_branch(&repo_path, &remote_name, &branch_name) {
        Ok(_) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(format!(
                r#"<div style="padding: 1rem; background: #d1fae5; border-left: 4px solid #10b981;">
                        <h3>Push Successful</h3>
                        <p>Pushed branch {} to {}</p>
                        <p><a href="/repo/{}">← Back to Repository</a></p>
                    </div>"#,
                branch_name, remote_name, identifier
            )),
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
    let commit_id = patch_event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == "c").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    let parent_commit = patch_event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == "parent").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned</h1><p>Clone the repository to view diffs.</p>");
    }

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    // Generate diff
    let diff_output = match (parent_commit.as_ref(), commit_id.as_ref()) {
        (Some(parent), Some(commit)) => match diff_commits(&repo_path, parent, commit) {
            Ok(diff) => diff,
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(format!("<h1>Error generating diff</h1><p>{}</p>", e));
            }
        },
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
    let commit_id = pr_event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == "c").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    let parent_commit = pr_event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == "parent").unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned());

    // Check if repository is cloned
    if !is_repository_cloned(&identifier) {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body("<h1>Repository not cloned</h1><p>Clone the repository to view diffs.</p>");
    }

    let repo_path = match get_repository_path(&identifier) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Invalid repository identifier {}: {}", identifier, e);
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": "Invalid repository identifier"}));
        }
    };

    // Generate diff
    let diff_output = match (parent_commit.as_ref(), commit_id.as_ref()) {
        (Some(parent), Some(commit)) => match diff_commits(&repo_path, parent, commit) {
            Ok(diff) => diff,
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(format!("<h1>Error generating diff</h1><p>{}</p>", e));
            }
        },
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

