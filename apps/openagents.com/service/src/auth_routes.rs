use super::*;

pub(super) async fn login_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

    let status = query_param_value(uri.query(), "status");
    let page = WebPage {
        title: "Sign in".to_string(),
        path: "/login".to_string(),
        session: None,
        body: WebBody::Login { status },
    };
    Ok(web_response_for_page(&state, &headers, &uri, page).await)
}

pub(super) async fn login_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<LoginEmailForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        if htmx.is_hx_request {
            return Ok(htmx_redirect_response("/"));
        }
        return Ok(Redirect::temporary("/").into_response());
    }

    let request_id = request_id(&headers);
    let challenge = state
        .auth
        .start_challenge(payload.email)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.web.challenge.requested", request_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute(
                "email_domain",
                email_domain(&challenge.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.web.challenge.requested", &request_id);

    let cookie = challenge_cookie(
        &challenge.challenge_id,
        state.config.auth_challenge_ttl_seconds,
    );

    let mut response = if htmx.is_hx_request {
        htmx_notice_response("login-status", "code-sent", false, StatusCode::OK)
    } else {
        Redirect::temporary("/login?status=code-sent").into_response()
    };
    append_set_cookie_header(&mut response, &cookie)?;
    Ok(response)
}

pub(super) async fn login_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<LoginVerifyForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        if htmx.is_hx_request {
            return Ok(htmx_redirect_response("/"));
        }
        return Ok(Redirect::temporary("/").into_response());
    }

    let request_id = request_id(&headers);
    let challenge_id = payload
        .challenge_id
        .and_then(non_empty)
        .or_else(|| extract_cookie_value(&headers, CHALLENGE_COOKIE_NAME));

    let challenge_id = match challenge_id {
        Some(value) => value,
        None => {
            if htmx.is_hx_request {
                return Ok(htmx_notice_response(
                    "login-status",
                    "code-expired",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                ));
            }
            return Err(validation_error(
                "code",
                "Your sign-in code expired. Request a new code.",
            ));
        }
    };

    let ip_address = header_string(&headers, HEADER_X_FORWARDED_FOR).unwrap_or_default();
    let user_agent = header_string(&headers, "user-agent").unwrap_or_default();
    let verified = match state
        .auth
        .verify_challenge(
            &challenge_id,
            payload.code,
            Some("openagents-web"),
            header_string(&headers, "x-device-id").as_deref(),
            &ip_address,
            &user_agent,
        )
        .await
    {
        Ok(verified) => verified,
        Err(error) => {
            if htmx.is_hx_request {
                return Ok(htmx_notice_response(
                    "login-status",
                    "invalid-code",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                ));
            }
            return Err(map_auth_error(error));
        }
    };

    state.observability.audit(
        AuditEvent::new("auth.web.verify.completed", request_id.clone())
            .with_user_id(verified.user.id.clone())
            .with_session_id(verified.session.session_id.clone())
            .with_org_id(verified.session.active_org_id.clone())
            .with_device_id(verified.session.device_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute("new_user", verified.new_user.to_string()),
    );
    state
        .observability
        .increment_counter("auth.web.verify.completed", &request_id);

    let mut response = if htmx.is_hx_request {
        htmx_redirect_response("/")
    } else {
        Redirect::temporary("/").into_response()
    };
    append_set_cookie_header(
        &mut response,
        &auth_access_cookie(&verified.access_token, state.config.auth_access_ttl_seconds),
    )?;
    append_set_cookie_header(
        &mut response,
        &auth_refresh_cookie(
            &verified.refresh_token,
            state.config.auth_refresh_ttl_seconds,
        ),
    )?;
    append_set_cookie_header(&mut response, &clear_cookie(CHALLENGE_COOKIE_NAME))?;
    Ok(response)
}

pub(super) async fn web_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers);

    if let Some(token) = access_token {
        match state.auth.revoke_session_by_access_token(&token).await {
            Ok(revoked) => {
                propagate_runtime_revocation(
                    &state,
                    &request_id,
                    vec![revoked.session_id],
                    vec![revoked.device_id],
                    SessionRevocationReason::UserRequested,
                )
                .await?;
            }
            Err(AuthError::Unauthorized { .. }) => {}
            Err(error) => return Err(map_auth_error(error)),
        }
    }

    let mut response = if htmx.is_hx_request {
        htmx_redirect_response("/")
    } else {
        Redirect::temporary("/").into_response()
    };
    append_set_cookie_header(&mut response, &clear_cookie(AUTH_ACCESS_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(AUTH_REFRESH_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(LOCAL_TEST_AUTH_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(CHALLENGE_COOKIE_NAME))?;
    Ok(response)
}

pub(super) async fn local_test_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(payload): Query<LocalTestLoginQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

    if !state.config.auth_local_test_login_enabled {
        return Err(not_found_error("Not found."));
    }

    let signing_key = state
        .config
        .auth_local_test_login_signing_key
        .as_deref()
        .ok_or_else(|| not_found_error("Not found."))?;
    let signature = payload
        .signature
        .and_then(non_empty)
        .ok_or_else(|| forbidden_error("Invalid signature."))?;
    if signature.is_empty() {
        return Err(forbidden_error("Invalid signature."));
    }
    let expires = payload.expires.unwrap_or_default();
    if expires <= Utc::now().timestamp() {
        return Err(forbidden_error("Invalid signature."));
    }

    if !local_test_login_signature_is_valid(&uri, signing_key) {
        return Err(forbidden_error("Invalid signature."));
    }

    let email = non_empty(payload.email)
        .ok_or_else(|| validation_error("email", "Invalid email."))?
        .to_lowercase();
    if !local_test_login_email_allowed(&email, &state.config.auth_local_test_login_allowed_emails) {
        return Err(forbidden_error("Forbidden."));
    }

    let verified = state
        .auth
        .local_test_sign_in(
            email,
            payload.name.and_then(non_empty),
            Some("openagents-web"),
            header_string(&headers, "x-device-id").as_deref(),
        )
        .await
        .map_err(map_auth_error)?;

    let mut response = Redirect::temporary("/").into_response();
    append_set_cookie_header(
        &mut response,
        &auth_access_cookie(&verified.access_token, state.config.auth_access_ttl_seconds),
    )?;
    append_set_cookie_header(
        &mut response,
        &auth_refresh_cookie(
            &verified.refresh_token,
            state.config.auth_refresh_ttl_seconds,
        ),
    )?;
    append_set_cookie_header(
        &mut response,
        &local_test_auth_cookie(state.config.auth_refresh_ttl_seconds),
    )?;
    Ok(response)
}

pub(super) async fn send_email_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let challenge = match state.auth.start_challenge(payload.email).await {
        Ok(challenge) => challenge,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.challenge.failed", &error);
            return Err(map_auth_error(error));
        }
    };

    state.observability.audit(
        AuditEvent::new("auth.challenge.requested", request_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute(
                "email_domain",
                email_domain(&challenge.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.challenge.requested", &request_id);

    let cookie = challenge_cookie(
        &challenge.challenge_id,
        state.config.auth_challenge_ttl_seconds,
    );
    let response = serde_json::json!({
        "ok": true,
        "status": "code-sent",
        "email": challenge.email,
        "challengeId": challenge.challenge_id,
    });

    Ok((
        [(SET_COOKIE, header_value(&cookie)?)],
        (StatusCode::OK, Json(response)),
    ))
}

pub(super) async fn auth_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AuthRegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);

    if !auth_api_signup_is_enabled(&state.config) {
        return Err(not_found_error("Not found."));
    }

    let email =
        non_empty(payload.email).ok_or_else(|| validation_error("email", "Email is required."))?;
    let email = email.to_lowercase();

    if let Some(name) = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if name.chars().count() > 120 {
            return Err(validation_error(
                "name",
                "Name may not be greater than 120 characters.",
            ));
        }
    }

    if let Some(domain) = email_domain(&email).map(|value| value.to_lowercase()) {
        if !state.config.auth_api_signup_allowed_domains.is_empty()
            && !state
                .config
                .auth_api_signup_allowed_domains
                .iter()
                .any(|allowed| allowed == &domain)
        {
            return Err(validation_error(
                "email",
                "Email domain is not allowed for API signup in this environment.",
            ));
        }
    }

    let token_name = normalize_register_token_name(
        payload.token_name,
        &state.config.auth_api_signup_default_token_name,
    )?;
    let token_abilities = normalize_register_token_abilities(payload.token_abilities)?;
    let create_autopilot = payload.create_autopilot.unwrap_or(false);
    let autopilot_display_name =
        normalize_optional_display_name(payload.autopilot_display_name, "autopilotDisplayName")?;
    let requested_name = normalize_optional_display_name(payload.name, "name")?;

    let registered = state
        .auth
        .register_api_user(email, requested_name)
        .await
        .map_err(map_auth_error)?;

    let issued_token = state
        .auth
        .issue_personal_access_token(
            &registered.user.id,
            token_name.clone(),
            token_abilities.clone(),
            None,
        )
        .await
        .map_err(map_auth_error)?;

    let autopilot_payload = if create_autopilot {
        let autopilot_display = autopilot_display_name
            .clone()
            .unwrap_or_else(|| "Autopilot".to_string());
        let autopilot = state
            ._domain_store
            .create_autopilot(CreateAutopilotInput {
                owner_user_id: registered.user.id.clone(),
                owner_display_name: registered.user.name.clone(),
                display_name: autopilot_display,
                handle_seed: None,
                avatar: None,
                status: None,
                visibility: None,
                tagline: None,
            })
            .await
            .map_err(map_domain_store_error)?;
        Some(serde_json::json!({
            "id": autopilot.autopilot.id,
            "handle": autopilot.autopilot.handle,
            "displayName": autopilot.autopilot.display_name,
            "status": autopilot.autopilot.status,
            "visibility": autopilot.autopilot.visibility,
        }))
    } else {
        None
    };

    state.observability.audit(
        AuditEvent::new("auth.register.completed", request_id.clone())
            .with_user_id(registered.user.id.clone())
            .with_attribute("created", registered.created.to_string())
            .with_attribute("token_name", token_name.clone())
            .with_attribute("autopilot_created", create_autopilot.to_string())
            .with_attribute(
                "email_domain",
                email_domain(&registered.user.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.register.completed", &request_id);

    let status = if registered.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let response = serde_json::json!({
        "data": {
            "created": registered.created,
            "tokenType": "Bearer",
            "token": issued_token.plain_text_token,
            "tokenName": token_name,
            "tokenAbilities": token_abilities,
            "user": {
                "id": registered.user.id,
                "name": registered.user.name,
                "email": registered.user.email,
                "handle": user_handle_from_email(&registered.user.email),
            },
            "autopilot": autopilot_payload,
        }
    });

    Ok((status, Json(response)))
}

pub(super) async fn verify_email_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<VerifyEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let challenge_id = payload
        .challenge_id
        .and_then(non_empty)
        .or_else(|| extract_cookie_value(&headers, CHALLENGE_COOKIE_NAME));

    let challenge_id = match challenge_id {
        Some(value) => value,
        None => {
            return Err(validation_error(
                "code",
                "Your sign-in code expired. Request a new code.",
            ));
        }
    };

    let client_name = header_string(&headers, "x-client");
    let device_id = payload
        .device_id
        .or_else(|| header_string(&headers, "x-device-id"));
    let ip_address = header_string(&headers, "x-forwarded-for").unwrap_or_default();
    let user_agent = header_string(&headers, "user-agent").unwrap_or_default();

    let verified = match state
        .auth
        .verify_challenge(
            &challenge_id,
            payload.code,
            client_name.as_deref(),
            device_id.as_deref(),
            &ip_address,
            &user_agent,
        )
        .await
    {
        Ok(verified) => verified,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.verify.failed", &error);
            return Err(map_auth_error(error));
        }
    };

    state.observability.audit(
        AuditEvent::new("auth.verify.completed", request_id.clone())
            .with_user_id(verified.user.id.clone())
            .with_session_id(verified.session.session_id.clone())
            .with_org_id(verified.session.active_org_id.clone())
            .with_device_id(verified.session.device_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute("new_user", verified.new_user.to_string()),
    );
    state
        .observability
        .increment_counter("auth.verify.completed", &request_id);

    let clear_cookie = clear_cookie(CHALLENGE_COOKIE_NAME);

    let response = serde_json::json!({
        "ok": true,
        "userId": verified.user.id,
        "status": "authenticated",
        "user": {
            "id": verified.user.id,
            "email": verified.user.email,
            "name": verified.user.name,
            "workosId": verified.user.workos_user_id,
        },
        "redirect": "/",
        "tokenType": verified.token_type,
        "token": verified.access_token,
        "tokenName": verified.token_name,
        "refreshToken": verified.refresh_token,
        "sessionId": verified.session.session_id,
        "newUser": verified.new_user,
    });

    Ok((
        [(SET_COOKIE, header_value(&clear_cookie)?)],
        (StatusCode::OK, Json(response)),
    ))
}

pub(super) async fn current_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    Ok((StatusCode::OK, Json(session_payload(bundle))))
}

pub(super) async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListSessionsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let sessions = state
        .auth
        .list_user_sessions(&bundle.user.id, query.device_id.as_deref())
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.sessions.listed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("session_count", sessions.len().to_string()),
    );
    state
        .observability
        .increment_counter("auth.sessions.listed", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "currentSessionId": bundle.session.session_id,
                "sessions": sessions,
            }
        })),
    ))
}

pub(super) async fn revoke_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RevokeSessionsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let target = resolve_session_revocation_target(&payload)?;
    let reason = payload
        .reason
        .unwrap_or(SessionRevocationReason::UserRequested);
    let include_current = payload.include_current.unwrap_or(false);

    let result = state
        .auth
        .revoke_user_sessions(
            &bundle.user.id,
            &bundle.session.session_id,
            SessionRevocationRequest {
                target,
                include_current,
                reason,
            },
        )
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.sessions.revoked", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("reason", revocation_reason_label(reason).to_string())
            .with_attribute(
                "revoked_session_count",
                result.revoked_session_ids.len().to_string(),
            )
            .with_attribute(
                "revoked_refresh_token_count",
                result.revoked_refresh_token_ids.len().to_string(),
            ),
    );
    state
        .observability
        .increment_counter("auth.sessions.revoked", &request_id);

    propagate_runtime_revocation(
        &state,
        &request_id,
        result.revoked_session_ids.clone(),
        result.revoked_device_ids.clone(),
        reason,
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "revokedSessionIds": result.revoked_session_ids,
            "revokedDeviceIds": result.revoked_device_ids,
            "revokedRefreshTokenIds": result.revoked_refresh_token_ids,
            "reason": reason,
            "revokedAt": timestamp(result.revoked_at),
        })),
    ))
}

pub(super) async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MeQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let chat_limit = query.chat_limit.unwrap_or(50).clamp(1, 200);
    let chat_threads = state
        .codex_thread_store
        .list_threads_for_user(&bundle.user.id, None)
        .await
        .map_err(map_thread_store_error)?
        .into_iter()
        .take(chat_limit)
        .map(|thread| {
            serde_json::json!({
                "id": thread.thread_id,
                "title": thread_title(&thread.thread_id, thread.message_count),
                "updatedAt": timestamp(thread.updated_at),
            })
        })
        .collect::<Vec<_>>();

    let response = serde_json::json!({
        "data": {
            "user": {
                "id": bundle.user.id,
                "email": bundle.user.email,
                "name": bundle.user.name,
                "handle": user_handle_from_email(&bundle.user.email),
                "avatar": "",
                "createdAt": serde_json::Value::Null,
                "updatedAt": serde_json::Value::Null,
                "workosId": bundle.user.workos_user_id,
            },
            "chatThreads": chat_threads,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}
