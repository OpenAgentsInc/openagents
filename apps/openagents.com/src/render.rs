use super::*;

pub(super) async fn web_shell_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let path = uri.path();
    if path.starts_with("/api/") {
        return Err(static_not_found(format!("Route '{}' was not found.", path)));
    }
    if is_retired_web_route(path) {
        return Err(static_not_found(format!("Route '{}' was not found.", path)));
    }
    if let Some(response) = maybe_serve_file_like_static_alias(&state, path, &headers).await? {
        return Ok(response);
    }

    let request_id = request_id(&headers);
    let cohort_key = resolve_route_cohort_key(&headers);
    let mut decision = state.route_split.evaluate(path, &cohort_key).await;
    if is_pilot_chat_route(path) {
        decision = RouteSplitDecision {
            path: path.to_string(),
            target: RouteTarget::RustShell,
            reason: "pilot_route_rust_only".to_string(),
            route_domain: "chat_pilot".to_string(),
            rollback_target: Some(RouteTarget::RustShell),
            cohort_bucket: decision.cohort_bucket,
            cohort_key: decision.cohort_key.clone(),
        };
    }

    emit_route_split_decision_audit(
        &state,
        &request_id,
        &decision,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    );

    match decision.target {
        RouteTarget::RustShell => render_web_page(&state, &headers, &uri).await,
        RouteTarget::Legacy => {
            let redirect = state
                .route_split
                .legacy_redirect_url(path, uri.query())
                .ok_or_else(|| {
                    error_response_with_status(
                        StatusCode::SERVICE_UNAVAILABLE,
                        ApiErrorCode::LegacyRouteUnavailable,
                        "Legacy route target is not configured.".to_string(),
                    )
                })?;
            Ok(Redirect::temporary(&redirect).into_response())
        }
    }
}

pub(super) async fn maybe_serve_file_like_static_alias(
    state: &AppState,
    request_path: &str,
    request_headers: &HeaderMap,
) -> Result<Option<Response>, (StatusCode, Json<ApiErrorResponse>)> {
    let trimmed = request_path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }

    if FsPath::new(trimmed).extension().is_none() {
        return Ok(None);
    }

    let relative_path = normalize_static_path(trimmed)
        .ok_or_else(|| static_not_found(format!("Asset '{}' was not found.", request_path)))?;
    let static_root = state.config.static_dir.as_path();
    let direct_path = static_root.join(&relative_path);
    let assets_fallback_path = static_root.join("assets").join(&relative_path);

    let asset_path = if direct_path.is_file() {
        direct_path
    } else if assets_fallback_path.is_file() {
        assets_fallback_path
    } else {
        return Err(static_not_found(format!(
            "Asset '{}' was not found.",
            relative_path
        )));
    };

    let cache_control = if is_hashed_asset_path(&relative_path) {
        CACHE_IMMUTABLE_ONE_YEAR
    } else {
        CACHE_SHORT_LIVED
    };

    let response = build_static_response(&asset_path, cache_control, Some(request_headers))
        .await
        .map_err(map_static_error)?;
    Ok(Some(response))
}

pub(super) async fn render_web_page(
    state: &AppState,
    headers: &HeaderMap,
    uri: &axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let path = uri.path().to_string();
    let status = query_param_value(uri.query(), "status");
    let session_bundle = session_bundle_from_headers(state, headers).await.ok();
    let session = session_bundle.as_ref().map(session_view_from_bundle);

    if path == "/" || path == "/chat" || path.starts_with("/chat/") {
        let requested_thread_id = chat_thread_id_from_path(&path);
        let mut threads = Vec::new();
        let mut messages = Vec::new();
        let mut active_thread_id = requested_thread_id.clone();

        if let Some(bundle) = session_bundle.as_ref() {
            let views = chat_views_for_bundle(state, bundle, requested_thread_id).await?;
            threads = views.threads;
            active_thread_id = views.active_thread_id;
            messages = views.messages;
        }

        let page = WebPage {
            title: "Codex".to_string(),
            path,
            session,
            body: WebBody::Chat {
                status,
                threads,
                active_thread_id,
                messages,
            },
        };
        return Ok(web_response_for_page(state, headers, uri, page).await);
    }

    if path.starts_with("/settings") {
        let htmx = classify_htmx_request(headers);
        let bundle = if let Some(bundle) = session_bundle.as_ref() {
            bundle
        } else {
            if htmx.is_hx_request {
                return Ok(htmx_redirect_response("/login"));
            }
            return Ok(Redirect::temporary("/login").into_response());
        };

        let integrations = state
            ._domain_store
            .list_integrations_for_user(&bundle.user.id)
            .await
            .map_err(map_domain_store_error)?;
        let resend = integration_status_view_for_provider(&integrations, "resend");
        let google = integration_status_view_for_provider(&integrations, "google");

        let page = WebPage {
            title: "Settings".to_string(),
            path,
            session,
            body: WebBody::Settings {
                status,
                profile_name: bundle.user.name.clone(),
                profile_email: bundle.user.email.clone(),
                resend,
                google,
            },
        };
        return Ok(web_response_for_page(state, headers, uri, page).await);
    }

    if path.starts_with("/admin") {
        let htmx = classify_htmx_request(headers);
        let bundle = if let Some(bundle) = session_bundle.as_ref() {
            bundle
        } else {
            if htmx.is_hx_request {
                return Ok(htmx_redirect_response("/login"));
            }
            return Ok(Redirect::temporary("/login").into_response());
        };
        let is_admin = is_admin_email(&bundle.user.email, &state.config.admin_emails);
        let route_split_status_json =
            serde_json::to_string_pretty(&state.route_split.status().await)
                .unwrap_or_else(|_| "{}".to_string());
        let runtime_routing_status_json =
            serde_json::to_string_pretty(&state.runtime_routing.status(&state._domain_store).await)
                .unwrap_or_else(|_| "{}".to_string());
        let effective_status = if !is_admin && status.is_none() {
            Some("admin-forbidden".to_string())
        } else {
            status
        };

        let page = WebPage {
            title: "Admin".to_string(),
            path,
            session,
            body: WebBody::Admin {
                status: effective_status,
                is_admin,
                route_split_status_json,
                runtime_routing_status_json,
            },
        };
        return Ok(web_response_for_page(state, headers, uri, page).await);
    }

    if path.starts_with("/billing") || path.starts_with("/l402") {
        let htmx = classify_htmx_request(headers);
        let bundle = if let Some(bundle) = session_bundle.as_ref() {
            bundle
        } else {
            if htmx.is_hx_request {
                return Ok(htmx_redirect_response("/login"));
            }
            return Ok(Redirect::temporary("/login").into_response());
        };
        let views = l402_web_views_for_bundle(state, bundle).await?;

        let page = WebPage {
            title: if path.starts_with("/billing") {
                "Billing".to_string()
            } else {
                "L402".to_string()
            },
            path,
            session,
            body: WebBody::L402 {
                status,
                is_admin: views.is_admin,
                wallet: views.wallet,
                transactions: views.transactions,
                paywalls: views.paywalls,
                deployments: views.deployments,
            },
        };
        return Ok(web_response_for_page(state, headers, uri, page).await);
    }

    let (heading, description) = web_placeholder_for_path(&path);
    let page = WebPage {
        title: heading.clone(),
        path,
        session,
        body: WebBody::Placeholder {
            heading,
            description,
        },
    };
    Ok(web_response_for_page(state, headers, uri, page).await)
}

pub(super) fn session_view_from_bundle(bundle: &SessionBundle) -> SessionView {
    SessionView {
        email: bundle.user.email.clone(),
        display_name: bundle.user.name.clone(),
    }
}

pub(super) fn integration_status_view_for_provider(
    integrations: &[UserIntegrationRecord],
    provider: &str,
) -> IntegrationStatusView {
    let integration = integrations
        .iter()
        .find(|row| row.provider.eq_ignore_ascii_case(provider));
    integration_status_view(provider, integration)
}

pub(super) fn integration_status_view(
    provider: &str,
    integration: Option<&UserIntegrationRecord>,
) -> IntegrationStatusView {
    let Some(integration) = integration else {
        return IntegrationStatusView {
            provider: provider.to_string(),
            connected: false,
            status: "inactive".to_string(),
            secret_last4: None,
            connected_at: None,
        };
    };

    let connected = integration.status == "active"
        && integration
            .encrypted_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();

    IntegrationStatusView {
        provider: integration.provider.clone(),
        connected,
        status: integration.status.clone(),
        secret_last4: integration.secret_last4.clone(),
        connected_at: integration.connected_at.map(timestamp),
    }
}

pub(super) struct L402WebViews {
    is_admin: bool,
    wallet: L402WalletSummaryView,
    transactions: Vec<L402TransactionView>,
    paywalls: Vec<L402PaywallView>,
    deployments: Vec<L402DeploymentView>,
}

pub(super) async fn l402_web_views_for_bundle(
    state: &AppState,
    bundle: &SessionBundle,
) -> Result<L402WebViews, (StatusCode, Json<ApiErrorResponse>)> {
    let receipts = state
        ._domain_store
        .list_l402_receipts_for_user(&bundle.user.id, None, 200, 0)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(map_l402_receipt_row)
        .collect::<Vec<_>>();
    let total_paid_msats: i64 = receipts
        .iter()
        .filter(|receipt| receipt.paid)
        .filter_map(|receipt| receipt.amount_msats)
        .sum();

    let wallet = L402WalletSummaryView {
        total_attempts: receipts.len(),
        paid_count: receipts.iter().filter(|receipt| receipt.paid).count(),
        total_paid_sats: l402_msats_to_sats(Some(total_paid_msats)).unwrap_or(0.0),
    };
    let transactions = receipts
        .iter()
        .take(60)
        .map(|receipt| L402TransactionView {
            event_id: receipt.event_id,
            host: receipt.host.clone(),
            scope: receipt.scope.clone().unwrap_or_else(|| "none".to_string()),
            status: receipt.status.clone(),
            paid: receipt.paid,
            amount_sats: l402_msats_to_sats(receipt.amount_msats),
            created_at: receipt.created_at.clone(),
        })
        .collect::<Vec<_>>();

    let paywalls = state
        ._domain_store
        .list_l402_paywalls_for_owner(&bundle.user.id, false)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(|paywall| L402PaywallView {
            id: paywall.id,
            name: paywall.name,
            host_regexp: paywall.host_regexp,
            path_regexp: paywall.path_regexp,
            price_msats: paywall.price_msats,
            upstream: paywall.upstream,
            enabled: paywall.enabled,
            updated_at: timestamp(paywall.updated_at),
        })
        .collect::<Vec<_>>();

    let allowed_types = [
        "l402_gateway_deployment",
        "l402_gateway_event",
        "l402_executor_heartbeat",
        "l402_paywall_created",
        "l402_paywall_updated",
        "l402_paywall_deleted",
    ];
    let deployments = state
        ._domain_store
        .list_l402_gateway_events_for_user(&bundle.user.id, None, 80)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .filter(|event| allowed_types.contains(&event.event_type.as_str()))
        .take(40)
        .map(|event| L402DeploymentView {
            event_id: event.id,
            event_type: event.event_type,
            created_at: timestamp(event.created_at),
        })
        .collect::<Vec<_>>();

    Ok(L402WebViews {
        is_admin: is_admin_email(&bundle.user.email, &state.config.admin_emails),
        wallet,
        transactions,
        paywalls,
        deployments,
    })
}

pub(super) fn chat_thread_id_from_path(path: &str) -> Option<String> {
    let remainder = path.strip_prefix("/chat/")?;
    let segment = remainder.split('/').next().unwrap_or_default().trim();
    if segment.is_empty() {
        None
    } else {
        Some(segment.to_string())
    }
}

pub(super) struct ChatWebViews {
    pub(super) threads: Vec<ChatThreadView>,
    pub(super) active_thread_id: Option<String>,
    pub(super) messages: Vec<ChatMessageView>,
}

pub(super) async fn chat_views_for_bundle(
    state: &AppState,
    bundle: &SessionBundle,
    requested_active_thread_id: Option<String>,
) -> Result<ChatWebViews, (StatusCode, Json<ApiErrorResponse>)> {
    let thread_rows = state
        .codex_thread_store
        .list_threads_for_user(&bundle.user.id, Some(&bundle.session.active_org_id))
        .await
        .map_err(map_thread_store_error)?;

    let mut active_thread_id = requested_active_thread_id;
    if active_thread_id.is_none() {
        active_thread_id = thread_rows.first().map(|thread| thread.thread_id.clone());
    }
    if active_thread_id
        .as_ref()
        .map(|candidate| {
            !thread_rows
                .iter()
                .any(|thread| thread.thread_id.as_str() == candidate.as_str())
        })
        .unwrap_or(false)
    {
        active_thread_id = thread_rows.first().map(|thread| thread.thread_id.clone());
    }

    let active_lookup = active_thread_id.clone();
    let threads = thread_rows
        .into_iter()
        .map(|thread| {
            let thread_id = thread.thread_id.clone();
            let is_active = active_lookup.as_deref() == Some(thread_id.as_str());
            ChatThreadView {
                title: thread_title(&thread_id, thread.message_count),
                thread_id,
                updated_at: timestamp(thread.updated_at),
                message_count: thread.message_count,
                is_active,
            }
        })
        .collect::<Vec<_>>();

    let messages = if let Some(active_id) = active_thread_id.as_deref() {
        let message_rows = state
            .codex_thread_store
            .list_thread_messages_for_user(&bundle.user.id, active_id)
            .await
            .map_err(map_thread_store_error)?;
        let mut messages = message_rows
            .into_iter()
            .map(|message| ChatMessageView {
                role: message.role,
                text: message.text,
                created_at: timestamp(message.created_at),
            })
            .collect::<Vec<_>>();
        messages.extend(runtime_event_messages_for_thread(state, active_id).await);
        messages
    } else {
        Vec::new()
    };

    Ok(ChatWebViews {
        threads,
        active_thread_id,
        messages,
    })
}

pub(super) async fn runtime_event_messages_for_thread(
    state: &AppState,
    thread_id: &str,
) -> Vec<ChatMessageView> {
    let event_log = state.runtime_workers.events.lock().await;
    let mut mapped = Vec::new();

    for (worker_id, worker_events) in event_log.iter() {
        for event in worker_events {
            let payload_thread_id = worker_event_thread_id(&event.payload);
            let event_matches_thread =
                worker_id == thread_id || payload_thread_id.as_deref() == Some(thread_id);
            if !event_matches_thread {
                continue;
            }

            if let Some(message) = runtime_event_to_chat_message(event) {
                mapped.push((event.occurred_at, event.seq, message));
            }
        }
    }

    mapped.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    mapped.into_iter().map(|(_, _, message)| message).collect()
}

pub(super) fn runtime_event_to_chat_message(
    event: &RuntimeWorkerEventRecord,
) -> Option<ChatMessageView> {
    let event_kind = worker_event_kind(event)?;
    let text = match event_kind.as_str() {
        "turn.start" => {
            let turn_id = json_pointer_string(&event.payload, "/turn/id")
                .or_else(|| json_pointer_string(&event.payload, "/turn_id"));
            match turn_id {
                Some(turn_id) => format!("Turn started: {turn_id}"),
                None => "Turn started.".to_string(),
            }
        }
        "turn.finish" => {
            let output = json_pointer_string(&event.payload, "/output/text")
                .or_else(|| json_pointer_string(&event.payload, "/response/output_text"))
                .or_else(|| json_pointer_string(&event.payload, "/output_text"))
                .or_else(|| json_pointer_string(&event.payload, "/text"));
            match output {
                Some(output) => format!("Turn finished: {output}"),
                None => "Turn finished.".to_string(),
            }
        }
        "turn.error" => {
            let error_message = json_pointer_string(&event.payload, "/error/message")
                .or_else(|| json_pointer_string(&event.payload, "/error"))
                .or_else(|| json_pointer_string(&event.payload, "/message"))
                .unwrap_or_else(|| "unknown error".to_string());
            format!("Turn error: {error_message}")
        }
        "turn.tool" => {
            let tool_name = json_pointer_string(&event.payload, "/tool/name")
                .or_else(|| json_pointer_string(&event.payload, "/toolName"))
                .or_else(|| json_pointer_string(&event.payload, "/name"))
                .unwrap_or_else(|| "unknown_tool".to_string());
            let tool_status = json_pointer_string(&event.payload, "/tool/status")
                .or_else(|| json_pointer_string(&event.payload, "/status"))
                .unwrap_or_else(|| "invoked".to_string());
            format!("Tool {tool_name}: {tool_status}")
        }
        _ => return None,
    };

    Some(ChatMessageView {
        role: "assistant".to_string(),
        text,
        created_at: timestamp(event.occurred_at),
    })
}

pub(super) fn worker_event_kind(event: &RuntimeWorkerEventRecord) -> Option<String> {
    let raw = if event.event_type == "worker.event" || event.event_type == "worker.response" {
        json_pointer_string(&event.payload, "/method")
            .or_else(|| json_pointer_string(&event.payload, "/event/type"))
            .or_else(|| json_pointer_string(&event.payload, "/type"))
            .or_else(|| json_pointer_string(&event.payload, "/event_type"))?
    } else {
        return None;
    };

    let normalized = raw
        .trim()
        .to_ascii_lowercase()
        .replace('/', ".")
        .replace('-', ".");

    let canonical = match normalized.as_str() {
        "turn.started" => "turn.start",
        "turn.completed" => "turn.finish",
        "turn.failed" => "turn.error",
        "turn.toolcall" | "turn.tool.call" => "turn.tool",
        _ => normalized.as_str(),
    };
    if matches!(
        canonical,
        "turn.start" | "turn.finish" | "turn.error" | "turn.tool"
    ) {
        Some(canonical.to_string())
    } else {
        None
    }
}

pub(super) fn worker_event_thread_id(payload: &serde_json::Value) -> Option<String> {
    json_pointer_string(payload, "/thread_id")
        .or_else(|| json_pointer_string(payload, "/thread/id"))
}

pub(super) fn json_pointer_string(payload: &serde_json::Value, pointer: &str) -> Option<String> {
    payload
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn web_placeholder_for_path(path: &str) -> (String, String) {
    if path.starts_with("/settings") {
        return (
            "Settings".to_string(),
            "Profile and integration surfaces are now server-rendered in Rust.".to_string(),
        );
    }
    if path.starts_with("/billing") {
        return (
            "Billing".to_string(),
            "Billing controls remain available through Rust API contracts.".to_string(),
        );
    }
    if path.starts_with("/l402") {
        return (
            "L402".to_string(),
            "L402 wallet, transactions, paywalls, and settlements stay on Rust APIs.".to_string(),
        );
    }
    if path.starts_with("/account") {
        return (
            "Account".to_string(),
            "Account and session controls are served by Rust authority.".to_string(),
        );
    }
    if path.starts_with("/onboarding") || path.starts_with("/auth") || path.starts_with("/register")
    {
        return (
            "Authentication".to_string(),
            "Authentication routes are now rendered from Rust + Maud.".to_string(),
        );
    }
    if path.starts_with("/admin") {
        return (
            "Admin".to_string(),
            "Admin pages are rendered here while control-plane authority remains API-driven."
                .to_string(),
        );
    }

    (
        "OpenAgents".to_string(),
        "This route is now served as Rust-rendered HTML using Maud components.".to_string(),
    )
}

pub(super) fn web_html_response(page: WebPage, htmx_enabled: bool) -> Response {
    let mut response = (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, CACHE_MANIFEST),
        ],
        render_maud_page(&page, htmx_enabled),
    )
        .into_response();
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn web_fragment_response(page: &WebPage) -> Response {
    let mut response =
        crate::web_htmx::fragment_response(render_maud_main_fragment(page), StatusCode::OK);
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn chat_thread_select_fragment_response(
    session: Option<&SessionView>,
    status: Option<&str>,
    threads: &[ChatThreadView],
    active_thread_id: Option<&str>,
    messages: &[ChatMessageView],
    push_url: Option<&str>,
) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_chat_thread_select_fragment(
            session,
            status,
            threads,
            active_thread_id,
            messages,
        ),
        StatusCode::OK,
    );
    if let Some(push_url) = push_url {
        htmx_set_push_url_header(&mut response, push_url);
    }
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn feed_main_select_fragment_response(
    session: Option<&SessionView>,
    status: Option<&str>,
    items: &[FeedItemView],
    zones: &[FeedZoneView],
    next_cursor: Option<&str>,
    current_zone: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_feed_main_select_fragment(
            session,
            status,
            items,
            zones,
            next_cursor,
            current_zone,
            page_limit,
            since,
        ),
        StatusCode::OK,
    );
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn feed_items_append_fragment_response(
    items: &[FeedItemView],
    next_cursor: Option<&str>,
    current_zone: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_feed_items_append_fragment(items, next_cursor, current_zone, page_limit, since),
        StatusCode::OK,
    );
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn should_render_hx_get_fragment(headers: &HeaderMap, mode: HtmxModeTarget) -> bool {
    if mode == HtmxModeTarget::FullPage {
        return false;
    }
    let htmx = classify_htmx_request(headers);
    htmx.is_hx_request
        && (htmx.boosted
            || htmx.history_restore_request
            || htmx.target.as_deref() == Some("oa-main-shell")
            || htmx.current_url.is_some())
}

pub(super) fn request_path_with_query(uri: &axum::http::Uri) -> String {
    if let Some(query) = uri.query().filter(|query| !query.trim().is_empty()) {
        return format!("{}?{query}", uri.path());
    }
    uri.path().to_string()
}

pub(super) async fn web_response_for_page(
    state: &AppState,
    headers: &HeaderMap,
    uri: &axum::http::Uri,
    page: WebPage,
) -> Response {
    let htmx = classify_htmx_request(headers);
    let request_id = request_id(headers);
    let htmx_mode = state.route_split.htmx_mode_for_path(uri.path()).await;
    emit_htmx_mode_decision_audit(state, &request_id, &htmx_mode, &htmx);

    if should_render_hx_get_fragment(headers, htmx_mode.mode) {
        return web_fragment_response(&page);
    }

    if htmx.is_hx_request && htmx_mode.mode == HtmxModeTarget::FullPage {
        return htmx_redirect_response(&request_path_with_query(uri));
    }

    web_html_response(page, htmx_mode.mode == HtmxModeTarget::Fragment)
}

pub(super) fn apply_html_security_headers(headers: &mut HeaderMap) {
    headers.insert(
        HEADER_CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(HTML_CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        HEADER_REFERRER_POLICY,
        HeaderValue::from_static(HTML_REFERRER_POLICY),
    );
    headers.insert(
        HEADER_X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS_NOSNIFF),
    );
    headers.insert(
        HEADER_X_FRAME_OPTIONS,
        HeaderValue::from_static(HTML_X_FRAME_OPTIONS),
    );
    headers.insert(
        HEADER_PERMISSIONS_POLICY,
        HeaderValue::from_static(HTML_PERMISSIONS_POLICY),
    );
}

pub(super) fn apply_static_security_headers(headers: &mut HeaderMap) {
    headers.insert(
        HEADER_X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS_NOSNIFF),
    );
}
