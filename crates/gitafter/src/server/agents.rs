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
        let reputation_labels = state
            .nostr_client
            .get_reputation_labels_for_agent(&pubkey)
            .await
            .unwrap_or_default();

        // Calculate reputation score
        let reputation_score = calculate_reputation_score(&reputation_labels);

        // Count merged PRs
        let merged_prs = state
            .nostr_client
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
    let mut agents_by_specialty: std::collections::HashMap<String, Vec<(String, i32, i32)>> =
        std::collections::HashMap::new();
    let mut all_specialties = std::collections::HashSet::new();

    for pubkey in agent_pubkeys {
        // Fetch reputation labels
        let reputation_labels = state
            .nostr_client
            .get_reputation_labels_for_agent(&pubkey)
            .await
            .unwrap_or_default();

        // Calculate reputation score
        let reputation_score = calculate_reputation_score(&reputation_labels);

        // Count merged PRs
        let merged_prs = state
            .nostr_client
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
        if let Ok(prs) = state
            .nostr_client
            .get_pull_requests_by_agent(&pubkey, 10)
            .await
        {
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
        .body(
            agent_marketplace_page(
                &agents_by_specialty,
                &sorted_specialties,
                query.specialty.as_deref(),
                query.min_reputation,
                query.search.as_deref(),
            )
            .into_string(),
        )
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
        vec![
            "l".to_string(),
            label.clone(),
            "agent.reputation".to_string(),
        ],
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
        if let Some(rating_str) = label
            .tags
            .iter()
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
async fn bounty_payment(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
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
        .event(&bounty_claim_id) // Zap the bounty claim event
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
                    include_str!("../styles.css"),
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
                .body(format!(
                    "<h1>Error</h1><p>Failed to sign payment request: {}</p>",
                    e
                ))
        }
    }
}

