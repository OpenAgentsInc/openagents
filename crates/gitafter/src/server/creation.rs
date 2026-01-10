/// Pull request creation form
async fn pr_create_form(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
        let prs = match state
            .nostr_client
            .get_pull_requests_by_repo(&repo_address, 1000)
            .await
        {
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
            use crate::stacks::StackGraph;

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
                    tracing::info!(
                        "Published pull request: event_id={}, subject='{}'",
                        event_id,
                        form.subject
                    );

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
async fn patch_create_form(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
    let repository = match state
        .nostr_client
        .get_repository_by_identifier(&identifier)
        .await
    {
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
    let mut builder = PatchBuilder::new(&repo_address, &form.title, &form.patch_content);

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
                    tracing::info!(
                        "Published patch: event_id={}, title='{}'",
                        event_id,
                        form.title
                    );

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
    language: Option<String>,
    topics: Option<String>,
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
        .map(|m| {
            m.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let topics: Vec<String> = form
        .topics
        .as_ref()
        .map(|topics| {
            topics
                .split(|c| c == ',' || c == '\n')
                .map(|topic| topic.trim().to_string())
                .filter(|topic| !topic.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Build repository announcement event using RepositoryAnnouncementBuilder
    let mut builder = RepositoryAnnouncementBuilder::new(&form.identifier, &form.name);

    // Add optional description
    if let Some(desc) = &form.description {
        if !desc.is_empty() {
            builder = builder.description(desc);
        }
    }

    if let Some(language) = &form.language {
        if !language.is_empty() {
            builder = builder.language(language);
        }
    }

    for topic in topics {
        builder = builder.topic(topic);
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

