/// Issue detail page
async fn issue_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (identifier, issue_id) = path.into_inner();

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
        .body(
            issue_detail_page(
                &repository,
                &issue,
                &claims,
                &bounties,
                &comments,
                &identifier,
            )
            .into_string(),
        )
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
                    format!(
                        "{}...{}",
                        &first_claim.pubkey[..8],
                        &first_claim.pubkey[first_claim.pubkey.len() - 8..]
                    )
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
    let mut builder = IssueClaimBuilder::new(&issue_id, &repo_address, issue_author_pubkey);

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
                    tracing::info!(
                        "Published issue claim: event_id={}, issue_id={}",
                        event_id,
                        issue_id
                    );

                    // Return success message
                    HttpResponse::Ok()
                        .content_type("text/html; charset=utf-8")
                        .body(format!(
                            r#"<div class="success-message">
                                <p>✅ Issue claim submitted!</p>
                                <p>Message: {}</p>
                                {}
                            </div>"#,
                            if content.is_empty() {
                                "No message"
                            } else {
                                &content
                            },
                            estimate
                                .map(|e| format!("<p>Estimate: {} seconds</p>", e))
                                .unwrap_or_default()
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
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Fetch repository to get pubkey
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

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build bounty offer event
    let mut builder = BountyOfferBuilder::new(&issue_id, &repo_address, amount);

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
                    tracing::info!(
                        "Published bounty offer: event_id={}, issue_id={}, amount={}",
                        event_id,
                        issue_id,
                        amount
                    );

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
                            expiry
                                .map(|e| format!("<p>Expires: {}</p>", e))
                                .unwrap_or_default(),
                            if !conditions.is_empty() {
                                format!(
                                    "<p>Conditions: <ul>{}</ul></p>",
                                    conditions
                                        .iter()
                                        .map(|c| format!("<li>{}</li>", c))
                                        .collect::<String>()
                                )
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
            vec![
                "e".to_string(),
                issue_id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
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
                    tracing::info!(
                        "Published issue comment: event_id={}, issue_id={}",
                        event_id,
                        issue_id
                    );

                    // Return new comment HTML for HTMX to insert
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let commenter_pubkey = if let Some(identity) = &state.identity {
                        let pk = identity.nostr_public_key();
                        if pk.len() > 16 {
                            format!("{}...{}", &pk[..8], &pk[pk.len() - 8..])
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
    let (_identifier, pr_id) = path.into_inner();

    // Extract form data
    let review_type = form
        .get("review_type")
        .cloned()
        .unwrap_or_else(|| "comment".to_string());
    let content = form.get("content").cloned().unwrap_or_default();
    let trajectory_session_id = form.get("trajectory_session_id").cloned();
    let trajectory_hash = form.get("trajectory_hash").cloned();

    if content.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(r#"<div class="error-message"><p>❌ Review content cannot be empty</p></div>"#);
    }

    let mut tags = vec![
        vec![
            "e".to_string(),
            pr_id.clone(),
            "".to_string(),
            "root".to_string(),
        ],
        vec!["review_type".to_string(), review_type.clone()],
    ];

    if let Ok(Some(pr_event)) = state.nostr_client.get_cached_event(&pr_id).await {
        tags.push(vec!["p".to_string(), pr_event.pubkey.clone()]);
    }

    if let Some(session_id) = &trajectory_session_id {
        tags.push(vec!["trajectory".to_string(), session_id.clone()]);
    }

    if let Some(hash) = &trajectory_hash {
        tags.push(vec!["trajectory_hash".to_string(), hash.clone()]);
    }

    let event_template = EventTemplate {
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        kind: 1,
        content: content.clone(),
        tags,
    };

    let signed_event = match state.sign_event(event_template) {
        Ok(event) => event,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body(format!(
                    r#"<div class="error-message"><p>❌ Failed to sign review: {}</p></div>"#,
                    e
                ));
        }
    };

    if let Err(e) = state.nostr_client.publish_event(signed_event).await {
        tracing::error!("Failed to publish review: {}", e);
        return HttpResponse::InternalServerError()
            .content_type("text/html; charset=utf-8")
            .body(format!(
                r#"<div class="error-message"><p>❌ Failed to publish review: {}</p></div>"#,
                e
            ));
    }

    let review_emoji = match review_type.as_str() {
        "approve" => "✅",
        "request_changes" => "🔴",
        _ => "💬",
    };

    let is_agent_review = trajectory_session_id.is_some() && trajectory_hash.is_some();

    // Verify trajectory if present
    let verification_badge = if let (Some(session_id), Some(hash)) =
        (&trajectory_session_id, &trajectory_hash)
    {
        // Fetch and verify trajectory events from Nostr cache
        match verify_trajectory_from_cache(&state.nostr_client, session_id, hash).await {
            Ok(true) => {
                r#"<span style="color: #48bb78; margin-left: 8px;" title="Trajectory verified">✓ Verified</span>"#
            }
            Ok(false) => {
                r#"<span style="color: #f59e0b; margin-left: 8px;" title="Trajectory hash mismatch">⚠ Hash Mismatch</span>"#
            }
            Err(e) => {
                tracing::warn!("Failed to verify trajectory {}: {}", session_id, e);
                r#"<span style="color: #ef4444; margin-left: 8px;" title="Verification failed">✗ Verification Failed</span>"#
            }
        }
    } else {
        ""
    };

    let agent_badge = if is_agent_review {
        format!(
            r#"<span style="background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 2px 8px; font-size: 0.85em; font-weight: 600; margin-left: 8px;">🤖 AGENT</span>{}"#,
            verification_badge
        )
    } else {
        String::new()
    };

    let trajectory_section = if let (Some(session_id), Some(hash)) =
        (trajectory_session_id, trajectory_hash)
    {
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
        review_emoji, agent_badge, review_type, content, trajectory_section
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(response_html)
}

/// Helper function to extract tag value from event
fn get_tag_value_from_event(event: &nostr::Event, tag_name: &str) -> Option<String> {
    event
        .tags
        .iter()
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
    let issue_event_id = pr_event
        .tags
        .iter()
        .find(|tag| {
            tag.len() >= 2
                && tag[0] == "e"
                && (tag.len() < 4
                    || (tag
                        .get(3)
                        .map(|m| m == "mention" || m == "reply" || m == "root")
                        .unwrap_or(false)))
        })
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string());

    let issue_id = match issue_event_id {
        Some(id) => id,
        None => {
            tracing::debug!(
                "PR {} does not reference an issue, skipping bounty claim",
                pr_id
            );
            return Ok(None);
        }
    };

    // 4. Check if PR has stack and layer tags for per-layer bounties
    let stack_id = get_tag_value_from_event(&pr_event, "stack");
    let layer_info = pr_event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "layer")
        .and_then(|tag| tag.get(1))
        .and_then(|s| s.parse::<u32>().ok());

    // 5. Try to find bounty - check layer-specific bounties first, then fall back to issue bounties
    let bounties = if let (Some(stack), Some(layer)) = (stack_id.as_ref(), layer_info) {
        // PR is part of a stack - check for layer-specific bounty first
        match state
            .nostr_client
            .get_bounties_for_layer(stack, layer)
            .await
        {
            Ok(layer_bounties) if !layer_bounties.is_empty() => {
                tracing::info!(
                    "Found layer-specific bounty for stack {} layer {}",
                    stack,
                    layer
                );
                layer_bounties
            }
            _ => {
                // No layer-specific bounty, fall back to issue bounty
                tracing::debug!("No layer-specific bounty, checking issue-level bounties");
                match state.nostr_client.get_bounties_for_issue(&issue_id).await {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(format!(
                            "Failed to fetch bounties for issue {}: {}",
                            issue_id, e
                        ));
                    }
                }
            }
        }
    } else {
        // Not a stacked PR - check issue-level bounties
        match state.nostr_client.get_bounties_for_issue(&issue_id).await {
            Ok(b) => b,
            Err(e) => {
                return Err(format!(
                    "Failed to fetch bounties for issue {}: {}",
                    issue_id, e
                ));
            }
        }
    };

    if bounties.is_empty() {
        tracing::debug!("No bounties found for issue {} or layer", issue_id);
        return Ok(None);
    }

    // 6. Get the first (most recent) bounty
    let bounty = &bounties[0];
    let bounty_id = bounty.id.clone();

    // Extract bounty amount for display
    let bounty_amount =
        get_tag_value_from_event(bounty, "amount").unwrap_or_else(|| "unknown".to_string());

    // 6. Get Lightning address from identity (if available)
    let lightning_address = state.identity.as_ref().and_then(|_id| {
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
                        claim_event_id,
                        bounty_id,
                        pr_id,
                        bounty_amount
                    );
                    Ok(Some(format!(
                        "Bounty claim created! Amount: {} sats. Claim ID: {}",
                        bounty_amount,
                        &claim_event_id[..8]
                    )))
                }
                Err(e) => Err(format!("Failed to publish bounty claim event: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to sign bounty claim event: {}", e)),
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
    let status = form
        .get("status")
        .cloned()
        .unwrap_or_else(|| "open".to_string());
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

    // Build repository address
    let repo_address = format!("30617:{}:{}", repository.pubkey, identifier);

    // Build status event
    let mut builder = StatusEventBuilder::new(&pr_id, &repo_address, status_kind);

    if !reason.is_empty() {
        builder = builder.reason(&reason);
    }

    let event_template = builder.build();

    // Validate merge order for stacked PRs
    if status_kind == 1631 {
        // Fetch all PRs for this repository to check dependencies
        match state
            .nostr_client
            .get_pull_requests_by_repo(&repo_address, 1000)
            .await
        {
            Ok(prs) => {
                // Find the PR being merged
                if let Some(pr) = prs.iter().find(|p| p.id == pr_id) {
                    // Check if PR has dependencies
                    let depends_on = pr
                        .tags
                        .iter()
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
                    tracing::info!(
                        "Published status change: event_id={}, pr_id={}, status={}",
                        event_id,
                        pr_id,
                        status_label
                    );

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
                                tracing::warn!(
                                    "Failed to create bounty claim for PR {}: {}",
                                    pr_id,
                                    e
                                );
                                bounty_claim_message =
                                    format!("<p>⚠️ Could not create bounty claim: {}</p>", e);
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
async fn issue_create_form(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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

    // Build issue content (subject as title, description as content body)
    let content = form.description.as_deref().unwrap_or("").to_string();

    // Create issue using NIP-34 Issue struct
    let mut issue =
        Issue::new(&content, &repo_address, &repository.pubkey).with_subject(&form.title);

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
                    tracing::info!(
                        "Published issue: event_id={}, title='{}'",
                        event_id,
                        form.title
                    );

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

