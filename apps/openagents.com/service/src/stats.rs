use super::*;
use openagents_runtime_client::{
    ComputeRuntimeProviderCatalogEntry, ComputeRuntimeTelemetryResponse,
    ComputeRuntimeTreasurySummary, ComputeRuntimeWorkerSnapshot, RuntimeClientError,
    RuntimeInternalClient, RuntimePoolSnapshotResponseV1, RuntimePoolStatusResponseV1,
    RuntimeWorkerStatusTransitionRequest,
};

pub(super) async fn compute_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return Ok(htmx_redirect_response("/login"));
            }
            return Ok(Redirect::temporary("/login").into_response());
        }
    };

    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let status = query_param_value(uri.query(), "status");
    let mut effective_status = status;

    let (metrics, providers, devices) =
        match fetch_compute_dashboard_views(&state, owner_user_id).await {
            Ok(views) => views,
            Err(error) => {
                tracing::warn!(owner_user_id, error = %error, "compute dashboard fetch failed");
                effective_status = Some("compute-runtime-unavailable".to_string());
                (empty_compute_metrics_view(), Vec::new(), Vec::new())
            }
        };

    let page = WebPage {
        title: "Compute".to_string(),
        path: "/compute".to_string(),
        session: Some(session_view_from_bundle(&bundle)),
        body: WebBody::Compute {
            status: effective_status,
            metrics,
            providers,
            devices,
        },
    };

    Ok(web_response_for_page(&state, &headers, &uri, page).await)
}

pub(super) async fn compute_main_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let htmx_mode = state.route_split.htmx_mode_for_path("/compute").await;
    if !htmx.is_hx_request || htmx_mode.mode == HtmxModeTarget::FullPage {
        let suffix = uri
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        return Ok(Redirect::temporary(&format!("/compute{suffix}")).into_response());
    }

    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => return Ok(htmx_redirect_response("/login")),
    };
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let status = query_param_value(uri.query(), "status");
    let mut effective_status = status;

    let (metrics, providers, devices) =
        match fetch_compute_dashboard_views(&state, owner_user_id).await {
            Ok(views) => views,
            Err(error) => {
                tracing::warn!(owner_user_id, error = %error, "compute dashboard fetch failed");
                effective_status = Some("compute-runtime-unavailable".to_string());
                (empty_compute_metrics_view(), Vec::new(), Vec::new())
            }
        };

    let page = WebPage {
        title: "Compute".to_string(),
        path: "/compute".to_string(),
        session: Some(session_view_from_bundle(&bundle)),
        body: WebBody::Compute {
            status: effective_status,
            metrics,
            providers,
            devices,
        },
    };

    Ok(web_fragment_response(&page))
}

pub(super) async fn compute_metrics_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if !htmx.is_hx_request {
        return Ok(Redirect::temporary("/compute").into_response());
    }

    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => return Ok(htmx_redirect_response("/login")),
    };
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let metrics = fetch_compute_metrics_view(&state, owner_user_id)
        .await
        .map_err(|message| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                message,
            )
        })?;
    Ok(compute_metrics_fragment_response(&metrics))
}

pub(super) async fn compute_fleet_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if !htmx.is_hx_request {
        return Ok(Redirect::temporary("/compute").into_response());
    }

    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => return Ok(htmx_redirect_response("/login")),
    };
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let (providers, devices) = fetch_compute_fleet_views(&state, owner_user_id)
        .await
        .map_err(|message| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                message,
            )
        })?;
    Ok(compute_fleet_fragment_response(&providers, &devices))
}

pub(super) async fn stats_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let status = query_param_value(uri.query(), "status");
    let mut effective_status = status;

    let (metrics, pools) = match fetch_stats_dashboard_views(&state).await {
        Ok(views) => views,
        Err(error) => {
            tracing::warn!(error = %error, "stats dashboard fetch failed");
            effective_status = Some("stats-runtime-unavailable".to_string());
            (empty_stats_metrics_view(), Vec::new())
        }
    };

    let page = WebPage {
        title: "Stats".to_string(),
        path: "/stats".to_string(),
        session: None,
        body: WebBody::Stats {
            status: effective_status,
            metrics,
            pools,
        },
    };

    let mut response = web_response_for_page(&state, &headers, &uri, page).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_SHORT_LIVED));
    Ok(response)
}

pub(super) async fn stats_main_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let htmx_mode = state.route_split.htmx_mode_for_path("/stats").await;
    if !htmx.is_hx_request || htmx_mode.mode == HtmxModeTarget::FullPage {
        let suffix = uri
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        return Ok(Redirect::temporary(&format!("/stats{suffix}")).into_response());
    }

    let status = query_param_value(uri.query(), "status");
    let mut effective_status = status;

    let (metrics, pools) = match fetch_stats_dashboard_views(&state).await {
        Ok(views) => views,
        Err(error) => {
            tracing::warn!(error = %error, "stats dashboard fetch failed");
            effective_status = Some("stats-runtime-unavailable".to_string());
            (empty_stats_metrics_view(), Vec::new())
        }
    };

    let page = WebPage {
        title: "Stats".to_string(),
        path: "/stats".to_string(),
        session: None,
        body: WebBody::Stats {
            status: effective_status,
            metrics,
            pools,
        },
    };

    let mut response = web_fragment_response(&page);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_SHORT_LIVED));
    Ok(response)
}

pub(super) async fn stats_metrics_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if !htmx.is_hx_request {
        return Ok(Redirect::temporary("/stats").into_response());
    }

    let metrics = fetch_stats_metrics_view(&state)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "stats metrics fetch failed");
            empty_stats_metrics_view()
        });

    Ok(stats_metrics_fragment_response(&metrics))
}

pub(super) async fn stats_pools_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if !htmx.is_hx_request {
        return Ok(Redirect::temporary("/stats").into_response());
    }

    let pools = fetch_stats_pools_view(&state)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "stats pools fetch failed");
            Vec::new()
        });

    Ok(stats_pools_fragment_response(&pools))
}

pub(super) async fn web_compute_provider_disable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(worker_id): Path<String>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return Ok(htmx_redirect_response("/login"));
            }
            return Ok(Redirect::temporary("/login").into_response());
        }
    };

    let client = match runtime_internal_client(&state) {
        Ok(client) => client,
        Err(_) => {
            return Ok(htmx_notice_response(
                "compute-status",
                "compute-runtime-unavailable",
                true,
                StatusCode::SERVICE_UNAVAILABLE,
            ));
        }
    };

    let worker_id = worker_id.trim().to_string();
    if worker_id.is_empty() {
        return Ok(htmx_notice_response(
            "compute-status",
            "compute-runtime-unavailable",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        ));
    }

    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let request = RuntimeWorkerStatusTransitionRequest {
        owner_user_id,
        status: "stopped".to_string(),
        reason: "disabled_from_web".to_string(),
    };

    match client.transition_worker_status(&worker_id, &request).await {
        Ok(_) => Ok(htmx_notice_response(
            "compute-status",
            "compute-provider-disabled",
            false,
            StatusCode::OK,
        )),
        Err(error) => {
            tracing::warn!(owner_user_id, error = %error, "runtime disable provider request failed");
            Ok(htmx_notice_response(
                "compute-status",
                "compute-action-failed",
                true,
                StatusCode::BAD_GATEWAY,
            ))
        }
    }
}

pub(super) fn compute_metrics_fragment_response(metrics: &ComputeMetricsView) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_compute_metrics_fragment(metrics),
        StatusCode::OK,
    );
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn compute_fleet_fragment_response(
    providers: &[ComputeProviderView],
    devices: &[ComputeDeviceView],
) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_compute_fleet_fragment(providers, devices),
        StatusCode::OK,
    );
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn stats_metrics_fragment_response(metrics: &LiquidityStatsMetricsView) -> Response {
    let mut response = crate::web_htmx::fragment_response(
        render_maud_stats_metrics_fragment(metrics),
        StatusCode::OK,
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_SHORT_LIVED));
    apply_html_security_headers(response.headers_mut());
    response
}

pub(super) fn stats_pools_fragment_response(pools: &[LiquidityPoolView]) -> Response {
    let mut response =
        crate::web_htmx::fragment_response(render_maud_stats_pools_fragment(pools), StatusCode::OK);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_SHORT_LIVED));
    apply_html_security_headers(response.headers_mut());
    response
}

fn runtime_internal_client(state: &AppState) -> Result<RuntimeInternalClient, RuntimeClientError> {
    RuntimeInternalClient::from_base_url(
        state.config.runtime_elixir_base_url.as_deref(),
        COMPUTE_DASHBOARD_TIMEOUT_MS,
    )
}

fn runtime_client_error_string(error: RuntimeClientError) -> String {
    error.to_string()
}

pub(super) fn empty_compute_metrics_view() -> ComputeMetricsView {
    ComputeMetricsView {
        provider_eligible_total: 0,
        provider_eligible_owned: 0,
        provider_eligible_reserve: 0,
        dispatch_total: 0,
        dispatch_not_found: 0,
        dispatch_errors: 0,
        dispatch_fallbacks: 0,
        latency_ms_avg: None,
        latency_ms_p50: None,
        budget_limit_msats: 0,
        budget_reserved_msats: 0,
        budget_spent_msats: 0,
        budget_remaining_msats: 0,
        released_msats_total: 0,
        released_count: 0,
        withheld_count: 0,
    }
}

pub(super) fn empty_stats_metrics_view() -> LiquidityStatsMetricsView {
    LiquidityStatsMetricsView {
        pool_count: 0,
        total_assets_sats: 0,
        total_wallet_sats: 0,
        total_onchain_sats: 0,
        total_channel_sats: 0,
        total_channel_outbound_sats: 0,
        total_channel_inbound_sats: 0,
        total_channel_count: 0,
        total_connected_channel_count: 0,
        total_shares: 0,
        pending_withdrawals_sats_estimate: 0,
        last_snapshot_at: None,
    }
}

pub(super) async fn fetch_stats_dashboard_views(
    state: &AppState,
) -> Result<(LiquidityStatsMetricsView, Vec<LiquidityPoolView>), String> {
    let pools = fetch_stats_pools_view(state).await?;
    let metrics = build_stats_metrics_view(&pools);
    Ok((metrics, pools))
}

pub(super) async fn fetch_stats_metrics_view(
    state: &AppState,
) -> Result<LiquidityStatsMetricsView, String> {
    let pools = fetch_stats_pools_view(state).await?;
    Ok(build_stats_metrics_view(&pools))
}

pub(super) async fn fetch_stats_pools_view(
    state: &AppState,
) -> Result<Vec<LiquidityPoolView>, String> {
    let client = runtime_internal_client(state).map_err(runtime_client_error_string)?;
    let pool_ids = state.config.liquidity_stats_pool_ids.clone();
    if pool_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for pool_id in pool_ids {
        let status = client
            .pool_status(pool_id.as_str())
            .await
            .map_err(runtime_client_error_string)?;
        let snapshot = client
            .pool_snapshot_latest(pool_id.as_str())
            .await
            .map_err(runtime_client_error_string)?;

        out.push(build_pool_view(pool_id, status.as_ref(), snapshot.as_ref()));
    }

    Ok(out)
}

pub(super) fn build_pool_view(
    pool_id: String,
    status: Option<&RuntimePoolStatusResponseV1>,
    snapshot: Option<&RuntimePoolSnapshotResponseV1>,
) -> LiquidityPoolView {
    let (pool_kind, pool_status, share_price_sats, total_shares, pending_withdrawals) = match status
    {
        Some(status) => (
            status.pool.pool_kind.clone(),
            status.pool.status.clone(),
            status.share_price_sats,
            status.total_shares,
            status.pending_withdrawals_sats_estimate,
        ),
        None => ("-".to_string(), "not_found".to_string(), 0, 0, 0),
    };

    let (
        snapshot_id,
        snapshot_as_of,
        snapshot_sha256,
        snapshot_signed,
        wallet_balance_sats,
        lightning_backend,
        lightning_onchain_sats,
        lightning_channel_total_sats,
        lightning_channel_outbound_sats,
        lightning_channel_inbound_sats,
        lightning_channel_count,
        lightning_connected_channel_count,
        lightning_last_error,
    ) = match snapshot {
        Some(snapshot) => {
            let wallet_balance_sats = snapshot
                .snapshot
                .assets_json
                .pointer("/walletBalanceSats")
                .and_then(serde_json::Value::as_i64);
            let lightning_backend = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/backend")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let lightning_onchain_sats = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/onchainSats")
                .and_then(serde_json::Value::as_i64);
            let lightning_channel_total_sats = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/channelTotalSats")
                .and_then(serde_json::Value::as_i64);
            let lightning_channel_outbound_sats = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/channelOutboundSats")
                .and_then(serde_json::Value::as_i64);
            let lightning_channel_inbound_sats = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/channelInboundSats")
                .and_then(serde_json::Value::as_i64);
            let lightning_channel_count = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/channelCount")
                .and_then(serde_json::Value::as_i64);
            let lightning_connected_channel_count = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/connectedChannelCount")
                .and_then(serde_json::Value::as_i64);
            let lightning_last_error = snapshot
                .snapshot
                .assets_json
                .pointer("/lightning/lastError")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            (
                Some(snapshot.snapshot.snapshot_id.clone()),
                Some(timestamp(snapshot.snapshot.as_of)),
                Some(snapshot.snapshot.canonical_json_sha256.clone()),
                snapshot.snapshot.signature_json.is_some(),
                wallet_balance_sats,
                lightning_backend,
                lightning_onchain_sats,
                lightning_channel_total_sats,
                lightning_channel_outbound_sats,
                lightning_channel_inbound_sats,
                lightning_channel_count,
                lightning_connected_channel_count,
                lightning_last_error,
            )
        }
        None => (
            None, None, None, false, None, None, None, None, None, None, None, None, None,
        ),
    };

    LiquidityPoolView {
        pool_id,
        pool_kind,
        status: pool_status,
        share_price_sats,
        total_shares,
        pending_withdrawals_sats_estimate: pending_withdrawals,
        latest_snapshot_id: snapshot_id,
        latest_snapshot_as_of: snapshot_as_of,
        latest_snapshot_sha256: snapshot_sha256,
        latest_snapshot_signed: snapshot_signed,
        wallet_balance_sats,
        lightning_backend,
        lightning_onchain_sats,
        lightning_channel_total_sats,
        lightning_channel_outbound_sats,
        lightning_channel_inbound_sats,
        lightning_channel_count,
        lightning_connected_channel_count,
        lightning_last_error,
    }
}

pub(super) fn build_stats_metrics_view(pools: &[LiquidityPoolView]) -> LiquidityStatsMetricsView {
    let mut total_assets_sats = 0_i64;
    let mut total_wallet_sats = 0_i64;
    let mut total_onchain_sats = 0_i64;
    let mut total_channel_sats = 0_i64;
    let mut total_channel_outbound_sats = 0_i64;
    let mut total_channel_inbound_sats = 0_i64;
    let mut total_channel_count = 0_i64;
    let mut total_connected_channel_count = 0_i64;
    let mut total_shares = 0_i64;
    let mut pending_withdrawals_sats_estimate = 0_i64;
    let mut last_snapshot_at: Option<String> = None;

    for pool in pools {
        let wallet = pool.wallet_balance_sats.unwrap_or(0);
        let onchain = pool.lightning_onchain_sats.unwrap_or(0);
        let channel_total = pool.lightning_channel_total_sats.unwrap_or(0);
        total_wallet_sats = total_wallet_sats.saturating_add(wallet);
        total_onchain_sats = total_onchain_sats.saturating_add(onchain);
        total_channel_sats = total_channel_sats.saturating_add(channel_total);
        total_assets_sats = total_assets_sats
            .saturating_add(wallet.saturating_add(onchain).saturating_add(channel_total));
        total_channel_outbound_sats = total_channel_outbound_sats
            .saturating_add(pool.lightning_channel_outbound_sats.unwrap_or(0));
        total_channel_inbound_sats = total_channel_inbound_sats
            .saturating_add(pool.lightning_channel_inbound_sats.unwrap_or(0));
        total_channel_count =
            total_channel_count.saturating_add(pool.lightning_channel_count.unwrap_or(0));
        total_connected_channel_count = total_connected_channel_count
            .saturating_add(pool.lightning_connected_channel_count.unwrap_or(0));
        total_shares = total_shares.saturating_add(pool.total_shares);
        pending_withdrawals_sats_estimate = pending_withdrawals_sats_estimate
            .saturating_add(pool.pending_withdrawals_sats_estimate);
        if let Some(ts) = pool.latest_snapshot_as_of.clone() {
            if last_snapshot_at
                .as_deref()
                .map(|prev| ts.as_str() > prev)
                .unwrap_or(true)
            {
                last_snapshot_at = Some(ts);
            }
        }
    }

    LiquidityStatsMetricsView {
        pool_count: pools.len(),
        total_assets_sats,
        total_wallet_sats,
        total_onchain_sats,
        total_channel_sats,
        total_channel_outbound_sats,
        total_channel_inbound_sats,
        total_channel_count,
        total_connected_channel_count,
        total_shares,
        pending_withdrawals_sats_estimate,
        last_snapshot_at,
    }
}

pub(super) async fn fetch_compute_dashboard_views(
    state: &AppState,
    owner_user_id: u64,
) -> Result<
    (
        ComputeMetricsView,
        Vec<ComputeProviderView>,
        Vec<ComputeDeviceView>,
    ),
    String,
> {
    let (telemetry, treasury, providers, workers) = tokio::try_join!(
        fetch_runtime_compute_telemetry(state, owner_user_id),
        fetch_runtime_compute_treasury(state, owner_user_id),
        fetch_runtime_provider_catalog(state, owner_user_id),
        fetch_runtime_workers(state, owner_user_id),
    )?;

    let metrics = build_compute_metrics_view(&telemetry, &treasury);
    let provider_views = build_compute_provider_views(&providers, &treasury);
    let device_views = build_compute_device_views(&workers);
    Ok((metrics, provider_views, device_views))
}

pub(super) async fn fetch_compute_metrics_view(
    state: &AppState,
    owner_user_id: u64,
) -> Result<ComputeMetricsView, String> {
    let (telemetry, treasury) = tokio::try_join!(
        fetch_runtime_compute_telemetry(state, owner_user_id),
        fetch_runtime_compute_treasury(state, owner_user_id),
    )?;
    Ok(build_compute_metrics_view(&telemetry, &treasury))
}

pub(super) async fn fetch_compute_fleet_views(
    state: &AppState,
    owner_user_id: u64,
) -> Result<(Vec<ComputeProviderView>, Vec<ComputeDeviceView>), String> {
    let (treasury, providers, workers) = tokio::try_join!(
        fetch_runtime_compute_treasury(state, owner_user_id),
        fetch_runtime_provider_catalog(state, owner_user_id),
        fetch_runtime_workers(state, owner_user_id),
    )?;
    Ok((
        build_compute_provider_views(&providers, &treasury),
        build_compute_device_views(&workers),
    ))
}

pub(super) fn build_compute_metrics_view(
    telemetry: &ComputeRuntimeTelemetryResponse,
    treasury: &ComputeRuntimeTreasurySummary,
) -> ComputeMetricsView {
    let remaining = treasury
        .account
        .limit_msats
        .saturating_sub(treasury.account.spent_msats)
        .saturating_sub(treasury.account.reserved_msats);

    ComputeMetricsView {
        provider_eligible_total: telemetry.provider_eligible_total,
        provider_eligible_owned: telemetry.provider_eligible_owned,
        provider_eligible_reserve: telemetry.provider_eligible_reserve,
        dispatch_total: telemetry.dispatch.dispatch_total,
        dispatch_not_found: telemetry.dispatch.dispatch_not_found,
        dispatch_errors: telemetry.dispatch.dispatch_errors,
        dispatch_fallbacks: telemetry.dispatch.dispatch_fallbacks,
        latency_ms_avg: telemetry.dispatch.latency_ms_avg,
        latency_ms_p50: telemetry.dispatch.latency_ms_p50,
        budget_limit_msats: treasury.account.limit_msats,
        budget_reserved_msats: treasury.account.reserved_msats,
        budget_spent_msats: treasury.account.spent_msats,
        budget_remaining_msats: remaining,
        released_msats_total: treasury.released_msats_total,
        released_count: treasury.released_count,
        withheld_count: treasury.withheld_count,
    }
}

pub(super) fn build_compute_provider_views(
    providers: &[ComputeRuntimeProviderCatalogEntry],
    treasury: &ComputeRuntimeTreasurySummary,
) -> Vec<ComputeProviderView> {
    let mut earnings = HashMap::new();
    for entry in &treasury.provider_earnings {
        earnings.insert(entry.provider_id.as_str(), entry.earned_msats);
    }

    let mut out = Vec::with_capacity(providers.len());
    for provider in providers {
        out.push(ComputeProviderView {
            provider_id: provider.provider_id.clone(),
            worker_id: provider.worker_id.clone(),
            supply_class: provider.supply_class.clone(),
            reserve_pool: provider.reserve_pool,
            status: provider.status.clone(),
            heartbeat_state: provider.heartbeat_state.clone(),
            heartbeat_age_ms: provider.heartbeat_age_ms,
            min_price_msats: provider.min_price_msats,
            earned_msats: earnings
                .get(provider.provider_id.as_str())
                .copied()
                .unwrap_or(0),
            quarantined: provider.quarantined,
            capabilities: provider.capabilities.clone(),
        });
    }
    out
}

pub(super) fn build_compute_device_views(
    workers: &[ComputeRuntimeWorkerSnapshot],
) -> Vec<ComputeDeviceView> {
    let mut out = Vec::with_capacity(workers.len());
    for worker in workers {
        let roles = json_string_array(worker.worker.metadata.get("roles"));
        out.push(ComputeDeviceView {
            worker_id: worker.worker.worker_id.clone(),
            status: worker.worker.status.clone(),
            heartbeat_state: worker.liveness.heartbeat_state.clone(),
            heartbeat_age_ms: worker.liveness.heartbeat_age_ms,
            roles,
            updated_at: timestamp(worker.worker.updated_at),
        });
    }
    out
}

pub(super) fn json_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    match value.as_array() {
        Some(values) => values
            .iter()
            .filter_map(|value| value.as_str())
            .map(|value| value.to_string())
            .collect(),
        None => Vec::new(),
    }
}

pub(super) async fn fetch_runtime_workers(
    state: &AppState,
    owner_user_id: u64,
) -> Result<Vec<ComputeRuntimeWorkerSnapshot>, String> {
    let client = runtime_internal_client(state).map_err(runtime_client_error_string)?;
    let response = client
        .list_workers(owner_user_id)
        .await
        .map_err(runtime_client_error_string)?;
    Ok(response.workers)
}

pub(super) async fn fetch_runtime_provider_catalog(
    state: &AppState,
    owner_user_id: u64,
) -> Result<Vec<ComputeRuntimeProviderCatalogEntry>, String> {
    let client = runtime_internal_client(state).map_err(runtime_client_error_string)?;
    let response = client
        .provider_catalog(owner_user_id)
        .await
        .map_err(runtime_client_error_string)?;
    Ok(response.providers)
}

pub(super) async fn fetch_runtime_compute_telemetry(
    state: &AppState,
    owner_user_id: u64,
) -> Result<ComputeRuntimeTelemetryResponse, String> {
    let client = runtime_internal_client(state).map_err(runtime_client_error_string)?;
    client
        .compute_telemetry(owner_user_id, COMPUTE_DEFAULT_CAPABILITY)
        .await
        .map_err(runtime_client_error_string)
}

pub(super) async fn fetch_runtime_compute_treasury(
    state: &AppState,
    owner_user_id: u64,
) -> Result<ComputeRuntimeTreasurySummary, String> {
    let client = runtime_internal_client(state).map_err(runtime_client_error_string)?;
    client
        .compute_treasury_summary(owner_user_id)
        .await
        .map_err(runtime_client_error_string)
}
