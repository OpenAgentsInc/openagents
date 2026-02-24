use super::*;

pub(super) async fn route_split_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let status = state.route_split.status().await;
    Ok(ok_data(status))
}

pub(super) async fn route_split_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RouteSplitOverrideRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_target = payload.target.trim().to_lowercase();
    let normalized_domain = payload
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());
    let mut override_kind = "route_target";
    let mut htmx_mode: Option<&'static str> = None;

    match normalized_target.as_str() {
        "legacy" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::Legacy))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::Legacy))
                    .await;
            }
        }
        "rust" | "rust_shell" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::RustShell))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::RustShell))
                    .await;
            }
        }
        "clear" | "default" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, None)
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state.route_split.set_override_target(None).await;
            }
        }
        "htmx_fragment" | "htmx_on" => {
            let domain = normalized_domain.as_deref().ok_or_else(|| {
                validation_error("domain", "Domain is required for HTMX overrides.")
            })?;
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(HtmxModeTarget::Fragment))
                .await
                .map_err(|message| validation_error("domain", &message))?;
            override_kind = "htmx_mode";
            htmx_mode = Some("fragment");
        }
        "htmx_full_page" | "htmx_off" => {
            let domain = normalized_domain.as_deref().ok_or_else(|| {
                validation_error("domain", "Domain is required for HTMX overrides.")
            })?;
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(HtmxModeTarget::FullPage))
                .await
                .map_err(|message| validation_error("domain", &message))?;
            override_kind = "htmx_mode";
            htmx_mode = Some("full_page");
        }
        "htmx_rollback" => {
            let domain = normalized_domain.as_deref().ok_or_else(|| {
                validation_error("domain", "Domain is required for HTMX overrides.")
            })?;
            let rollback_mode = state
                .route_split
                .htmx_rollback_mode_for_domain(Some(domain))
                .ok_or_else(|| validation_error("domain", "Unknown route domain."))?;
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(rollback_mode))
                .await
                .map_err(|message| validation_error("domain", &message))?;
            override_kind = "htmx_mode";
            htmx_mode = Some(match rollback_mode {
                HtmxModeTarget::Fragment => "fragment",
                HtmxModeTarget::FullPage => "full_page",
            });
        }
        "htmx_clear" => {
            let domain = normalized_domain.as_deref().ok_or_else(|| {
                validation_error("domain", "Domain is required for HTMX overrides.")
            })?;
            state
                .route_split
                .set_domain_htmx_mode(domain, None)
                .await
                .map_err(|message| validation_error("domain", &message))?;
            override_kind = "htmx_mode";
            htmx_mode = None;
        }
        "rollback" => {
            if let Some(domain) = normalized_domain.as_deref() {
                let rollback_target = state
                    .route_split
                    .rollback_target_for_domain(Some(domain))
                    .ok_or_else(|| validation_error("domain", "Unknown route domain."))?;
                state
                    .route_split
                    .set_domain_override_target(domain, Some(rollback_target))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else if let Some(global_target) = state.route_split.rollback_target_for_domain(None) {
                state
                    .route_split
                    .set_override_target(Some(global_target))
                    .await;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::Legacy))
                    .await;
            }
        }
        _ => {
            return Err(validation_error(
                "target",
                "Target must be one of: legacy, rust, rollback, clear, htmx_fragment, htmx_full_page, htmx_rollback, htmx_clear.",
            ));
        }
    }

    let status = state.route_split.status().await;
    let scope = normalized_domain
        .clone()
        .map(|domain| format!("domain:{domain}"))
        .unwrap_or_else(|| "global".to_string());
    let event_name = if override_kind == "htmx_mode" {
        "route.split.htmx.override.updated"
    } else {
        "route.split.override.updated"
    };

    state.observability.audit(
        AuditEvent::new(event_name, request_id.clone())
            .with_user_id(session.user.id)
            .with_session_id(session.session.session_id)
            .with_org_id(session.session.active_org_id)
            .with_device_id(session.session.device_id)
            .with_attribute("target", normalized_target)
            .with_attribute("scope", scope)
            .with_attribute("override_kind", override_kind)
            .with_attribute("htmx_mode", htmx_mode.unwrap_or("clear")),
    );
    state
        .observability
        .increment_counter(event_name, &request_id);

    Ok(ok_data(status))
}

pub(super) async fn route_split_evaluate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RouteSplitEvaluateRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    if payload.path.trim().is_empty() {
        return Err(validation_error("path", "Path is required."));
    }

    let cohort_key = payload
        .cohort_key
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| resolve_route_cohort_key(&headers));

    let decision = state.route_split.evaluate(&payload.path, &cohort_key).await;
    Ok(ok_data(decision))
}

pub(super) async fn runtime_routing_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let status = state.runtime_routing.status(&state._domain_store).await;
    Ok(ok_data(status))
}

pub(super) async fn runtime_routing_evaluate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeRoutingEvaluateRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let thread_id = payload.thread_id.trim().to_string();
    if thread_id.is_empty() {
        return Err(validation_error("thread_id", "Thread id is required."));
    }

    let user_id = payload
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| session.user.id.clone());

    if user_id != session.user.id {
        return Err(forbidden_error(
            "You may only evaluate runtime routing for your user.",
        ));
    }

    let decision = state
        .runtime_routing
        .resolve(
            &state._domain_store,
            &state.codex_thread_store,
            RuntimeRoutingResolveInput {
                user_id,
                thread_id,
                autopilot_id: payload.autopilot_id,
            },
        )
        .await;

    Ok(ok_data(decision))
}

pub(super) async fn runtime_routing_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeRoutingOverrideRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let scope_type = payload.scope_type.trim().to_ascii_lowercase();
    if !matches!(scope_type.as_str(), "user" | "autopilot") {
        return Err(validation_error(
            "scope_type",
            "Scope type must be one of: user, autopilot.",
        ));
    }

    let scope_id = payload.scope_id.trim().to_string();
    if scope_id.is_empty() {
        return Err(validation_error("scope_id", "Scope id is required."));
    }
    if scope_id.chars().count() > 160 {
        return Err(validation_error(
            "scope_id",
            "Scope id may not be greater than 160 characters.",
        ));
    }

    let driver = RuntimeDriver::parse(&payload.driver)
        .ok_or_else(|| validation_error("driver", "Driver must be one of: legacy, elixir."))?
        .as_str()
        .to_string();

    let reason = payload
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if reason
        .as_deref()
        .map(|value| value.chars().count() > 255)
        .unwrap_or(false)
    {
        return Err(validation_error(
            "reason",
            "Reason may not be greater than 255 characters.",
        ));
    }

    let override_record = state
        ._domain_store
        .upsert_runtime_driver_override(UpsertRuntimeDriverOverrideInput {
            scope_type,
            scope_id,
            driver,
            is_active: payload.is_active.unwrap_or(true),
            reason,
            meta: payload.meta,
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("runtime.routing.override.updated", request_id.clone())
            .with_user_id(session.user.id)
            .with_session_id(session.session.session_id)
            .with_org_id(session.session.active_org_id)
            .with_device_id(session.session.device_id)
            .with_attribute("scope_type", override_record.scope_type.clone())
            .with_attribute("scope_id", override_record.scope_id.clone())
            .with_attribute("driver", override_record.driver.clone())
            .with_attribute("is_active", override_record.is_active.to_string()),
    );
    state
        .observability
        .increment_counter("runtime.routing.override.updated", &request_id);

    let status = state.runtime_routing.status(&state._domain_store).await;
    Ok(ok_data(serde_json::json!({
        "override": override_record,
        "status": status,
    })))
}
