/// Query parameters for repository filtering
#[derive(Debug, serde::Deserialize)]
struct RepoFilterQuery {
    language: Option<String>,
    topic: Option<String>,
    has_bounties: Option<String>,
    agent_friendly: Option<String>,
}

fn filter_repositories_by_query(repositories: &mut Vec<nostr::Event>, query: &RepoFilterQuery) {
    if let Some(language) = &query.language {
        if !language.is_empty() {
            repositories.retain(|repo| {
                repo.tags.iter().any(|tag| {
                    tag.first().map(|t| t == "language").unwrap_or(false)
                        && tag
                            .get(1)
                            .map(|l| l.eq_ignore_ascii_case(language))
                            .unwrap_or(false)
                })
            });
        }
    }

    if let Some(topic) = &query.topic {
        if !topic.is_empty() {
            repositories.retain(|repo| {
                repo.tags.iter().any(|tag| {
                    tag.len() >= 2
                        && (tag[0] == "topic" || tag[0] == "t")
                        && tag[1].eq_ignore_ascii_case(topic)
                })
            });
        }
    }
}

/// Home page
async fn index(state: web::Data<AppState>, query: web::Query<RepoFilterQuery>) -> HttpResponse {
    // Fetch repositories from cache
    let mut repositories = match state.nostr_client.get_cached_repositories(50).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to fetch repositories: {}", e);
            vec![]
        }
    };

    // Apply filters
    filter_repositories_by_query(&mut repositories, &query);

    // Build bounty counts per repository if filtering by has_bounties
    let mut repo_bounty_counts = std::collections::HashMap::new();
    let has_bounties_filter = query.has_bounties.as_deref() == Some("true");

    if has_bounties_filter {
        for repo in &repositories {
            let repo_id = repo
                .tags
                .iter()
                .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
                .and_then(|tag| tag.get(1).cloned())
                .unwrap_or_default();

            let repo_address = format!("30617:{}:{}", repo.pubkey, repo_id);

            // Get issues for this repo
            if let Ok(issues) = state
                .nostr_client
                .get_issues_by_repo(&repo_address, 100)
                .await
            {
                let mut bounty_count = 0;
                for issue in issues {
                    if let Ok(bounties) = state.nostr_client.get_bounties_for_issue(&issue.id).await
                    {
                        bounty_count += bounties.len();
                    }
                }
                repo_bounty_counts.insert(repo_id.clone(), bounty_count);
            }
        }

        // Filter to only repos with bounties
        repositories.retain(|repo| {
            let repo_id = repo
                .tags
                .iter()
                .find(|tag| tag.first().map(|t| t == "d").unwrap_or(false))
                .and_then(|tag| tag.get(1).cloned())
                .unwrap_or_default();
            repo_bounty_counts.get(&repo_id).copied().unwrap_or(0) > 0
        });
    }

    if query.agent_friendly.as_deref() == Some("true") {
        // Filter repos marked as agent-friendly
        repositories.retain(|repo| {
            repo.tags
                .iter()
                .any(|tag| tag.first().map(|t| t == "agent-friendly").unwrap_or(false))
        });
    }

    // Pass bounty counts to the view
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            home_page_with_repos(
                &repositories,
                &query.language,
                &query.topic,
                query.has_bounties.as_deref() == Some("true"),
                query.agent_friendly.as_deref() == Some("true"),
                &repo_bounty_counts,
            )
            .into_string(),
        )
}

/// Repository detail page
async fn repository_detail(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
    let repo_state = state
        .nostr_client
        .get_repository_state(&identifier)
        .await
        .ok()
        .flatten();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            repository_detail_page(&repository, is_cloned, local_path, repo_state.as_ref())
                .into_string(),
        )
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

    // Check if repository is watched
    let is_watched = state
        .nostr_client
        .is_repository_watched(&identifier)
        .await
        .unwrap_or(false);

    // Fetch issues for this repository
    let mut issues = match state
        .nostr_client
        .get_issues_by_repo(&repo_address, 100)
        .await
    {
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
            let bounties = state
                .nostr_client
                .get_bounties_for_issue(&issue.id)
                .await
                .unwrap_or_default();
            issue_bounties.insert(issue.id.clone(), !bounties.is_empty());
        }

        // Always fetch claims to display claim badges
        let claims = state
            .nostr_client
            .get_claims_for_issue(&issue.id)
            .await
            .unwrap_or_default();
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
        let status = issue
            .tags
            .iter()
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
        .body(
            issues_list_page(
                &repository,
                &issues,
                is_watched,
                &identifier,
                filter_open,
                filter_closed,
                filter_has_bounty,
                filter_claimed,
                &issue_first_claims,
            )
            .into_string(),
        )
}

