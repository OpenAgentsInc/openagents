use super::*;

    pub(super) fn build_sync_websocket_url(token: &str) -> Result<String, ControlApiError> {
        let window = web_sys::window().ok_or_else(|| ControlApiError {
            status_code: 0,
            code: Some("window_unavailable".to_string()),
            message: "window is unavailable".to_string(),
            kind: CommandErrorKind::Unknown,
            retryable: false,
        })?;
        let location = window.location();
        let protocol = location.protocol().map_err(|_| ControlApiError {
            status_code: 0,
            code: Some("location_protocol_unavailable".to_string()),
            message: "browser protocol is unavailable".to_string(),
            kind: CommandErrorKind::Unknown,
            retryable: false,
        })?;
        let host = location.host().map_err(|_| ControlApiError {
            status_code: 0,
            code: Some("location_host_unavailable".to_string()),
            message: "browser host is unavailable".to_string(),
            kind: CommandErrorKind::Unknown,
            retryable: false,
        })?;

        let ws_protocol = if protocol == "https:" { "wss" } else { "ws" };
        Ok(format!(
            "{ws_protocol}://{host}/sync/socket/websocket?token={token}&vsn={KHALA_WS_VSN}"
        ))
    }

    pub(super) fn websocket_text(message: WsMessage) -> Result<String, ControlApiError> {
        match message {
            WsMessage::Text(text) => Ok(text),
            WsMessage::Bytes(bytes) => {
                String::from_utf8(bytes.to_vec()).map_err(|error| ControlApiError {
                    status_code: 0,
                    code: Some("khala_frame_utf8_error".to_string()),
                    message: format!("invalid websocket frame encoding: {error}"),
                    kind: CommandErrorKind::Decode,
                    retryable: false,
                })
            }
        }
    }

    pub(super) async fn post_send_code(email: &str) -> Result<SendCodeResponse, ControlApiError> {
        let normalized_email = normalize_email(email).map_err(auth_input_validation_error)?;
        let state = snapshot_state();
        let intent = CommandIntent::StartAuthChallenge {
            email: normalized_email,
        };
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    pub(super) async fn post_verify_code(
        code: &str,
        challenge_id: Option<&str>,
    ) -> Result<VerifyCodeResponse, ControlApiError> {
        let normalized_code =
            normalize_verification_code(code).map_err(auth_input_validation_error)?;
        let mut state = snapshot_state();
        state.auth.challenge_id = challenge_id.map(ToString::to_string);
        let intent = CommandIntent::VerifyAuthCode {
            code: normalized_code,
        };
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    pub(super) async fn post_refresh_session(refresh_token: &str) -> Result<RefreshResponse, ControlApiError> {
        let mut state = snapshot_state();
        state.auth.refresh_token = Some(refresh_token.to_string());
        let intent = CommandIntent::RefreshSession;
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    pub(super) async fn post_logout(access_token: &str) -> Result<serde_json::Value, ControlApiError> {
        let mut state = snapshot_state();
        state.auth.access_token = Some(access_token.to_string());
        let intent = CommandIntent::LogoutSession;
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    pub(super) async fn get_current_session(
        access_token: &str,
    ) -> Result<SessionSnapshotWithUser, ControlApiError> {
        let mut state = snapshot_state();
        state.auth.access_token = Some(access_token.to_string());
        let intent = CommandIntent::RestoreSession;
        let request = plan_http_request(&intent, &state)?;
        let response: SessionResponse = send_json_request(&request, &state).await?;
        let session_status = map_session_status(&response.data.session.status);
        let session = SessionSnapshot {
            session_id: response.data.session.session_id.clone(),
            user_id: response.data.session.user_id.clone(),
            device_id: response.data.session.device_id,
            token_name: response.data.session.token_name,
            active_org_id: response.data.session.active_org_id,
            status: session_status,
            reauth_required: response.data.session.reauth_required,
            issued_at: response.data.session.issued_at,
            access_expires_at: response.data.session.access_expires_at,
            refresh_expires_at: response.data.session.refresh_expires_at,
        };
        let user = AuthUser {
            user_id: response.data.user.id,
            email: response.data.user.email,
            name: response.data.user.name,
            workos_id: response.data.user.workos_id,
        };
        if session.status == SessionLifecycleStatus::ReauthRequired || session.reauth_required {
            return Err(ControlApiError::unauthorized(
                "Reauthentication required for this session.",
            ));
        }
        Ok(SessionSnapshotWithUser { session, user })
    }

    pub(super) fn snapshot_state() -> AppState {
        APP_STATE.with(|state| state.borrow().clone())
    }

    pub(super) fn plan_http_request(
        intent: &CommandIntent,
        state: &AppState,
    ) -> Result<HttpCommandRequest, ControlApiError> {
        map_intent_to_http(intent, state).map_err(ControlApiError::from_command_error)
    }

    pub(super) async fn send_json_request<T: for<'de> Deserialize<'de>>(
        request: &HttpCommandRequest,
        state: &AppState,
    ) -> Result<T, ControlApiError> {
        let mut request_builder = match request.method {
            HttpMethod::Get => Request::get(&request.path),
            HttpMethod::Post => {
                Request::post(&request.path).header("content-type", "application/json")
            }
        };

        for (header_name, header_value) in &request.headers {
            request_builder = request_builder.header(header_name, header_value);
        }

        if let Some(token) = resolve_bearer_token(&request.auth, state) {
            request_builder = request_builder.header("authorization", &format!("Bearer {token}"));
        }

        let response = if let Some(body) = request.body.as_ref() {
            let body = serde_json::to_string(body).map_err(|error| ControlApiError {
                status_code: 500,
                code: Some("request_body_serialize_failed".to_string()),
                message: format!("failed to serialize request body: {error}"),
                kind: CommandErrorKind::Decode,
                retryable: false,
            })?;
            let request = request_builder
                .body(body)
                .map_err(|error| ControlApiError {
                    status_code: 500,
                    code: Some("request_build_failed".to_string()),
                    message: format!("failed to build request body: {error}"),
                    kind: CommandErrorKind::Unknown,
                    retryable: false,
                })?;
            request.send().await.map_err(map_network_error)?
        } else {
            request_builder.send().await.map_err(map_network_error)?
        };

        decode_json_response(response).await
    }

    pub(super) fn resolve_bearer_token(auth: &AuthRequirement, state: &AppState) -> Option<String> {
        match auth {
            AuthRequirement::None => None,
            AuthRequirement::AccessToken => state.auth.access_token.clone(),
            AuthRequirement::RefreshToken => state.auth.refresh_token.clone(),
        }
    }

    pub(super) fn map_network_error(error: gloo_net::Error) -> ControlApiError {
        let classified = classify_http_error(0, Some("network_error"), error.to_string());
        ControlApiError {
            status_code: 0,
            code: Some("network_error".to_string()),
            message: classified.message,
            kind: classified.kind,
            retryable: classified.retryable,
        }
    }

    pub(super) fn map_websocket_error<E: std::fmt::Display>(error: E) -> ControlApiError {
        let classified = classify_http_error(0, Some("network_error"), error.to_string());
        ControlApiError {
            status_code: 0,
            code: Some("network_error".to_string()),
            message: classified.message,
            kind: classified.kind,
            retryable: classified.retryable,
        }
    }

    pub(super) async fn decode_json_response<T: for<'de> Deserialize<'de>>(
        response: gloo_net::http::Response,
    ) -> Result<T, ControlApiError> {
        let status = response.status();
        let raw = response.text().await.map_err(|error| ControlApiError {
            status_code: status,
            code: Some("response_read_failed".to_string()),
            message: error.to_string(),
            kind: CommandErrorKind::Unknown,
            retryable: false,
        })?;

        if !(200..=299).contains(&status) {
            let parsed_error: Option<ApiErrorBody> = serde_json::from_str(&raw).ok();
            let code = parsed_error
                .as_ref()
                .and_then(|error| error.error.as_ref())
                .and_then(|detail| detail.code.clone());
            let message = parsed_error
                .as_ref()
                .and_then(|error| error.message.clone())
                .or_else(|| {
                    parsed_error
                        .as_ref()
                        .and_then(|error| error.error.as_ref())
                        .and_then(|detail| detail.message.clone())
                })
                .unwrap_or_else(|| format!("request failed with status {status}"));
            let classified = classify_http_error(status, code.as_deref(), message);
            return Err(ControlApiError {
                status_code: status,
                code,
                message: classified.message,
                kind: classified.kind,
                retryable: classified.retryable,
            });
        }

        serde_json::from_str(&raw).map_err(|error| {
            let code = Some("decode_failed".to_string());
            let classified = classify_http_error(
                status,
                code.as_deref(),
                format!("failed to decode response: {error}"),
            );
            ControlApiError {
                status_code: status,
                code,
                message: classified.message,
                kind: classified.kind,
                retryable: classified.retryable,
            }
        })
    }

    pub(super) async fn decode_sse_response(
        response: gloo_net::http::Response,
    ) -> Result<String, ControlApiError> {
        let status = response.status();
        let raw = response.text().await.map_err(|error| ControlApiError {
            status_code: status,
            code: Some("response_read_failed".to_string()),
            message: error.to_string(),
            kind: CommandErrorKind::Unknown,
            retryable: false,
        })?;

        if !(200..=299).contains(&status) {
            let parsed_error: Option<ApiErrorBody> = serde_json::from_str(&raw).ok();
            let code = parsed_error
                .as_ref()
                .and_then(|error| error.error.as_ref())
                .and_then(|detail| detail.code.clone());
            let message = parsed_error
                .as_ref()
                .and_then(|error| error.message.clone())
                .or_else(|| {
                    parsed_error
                        .as_ref()
                        .and_then(|error| error.error.as_ref())
                        .and_then(|detail| detail.message.clone())
                })
                .unwrap_or_else(|| format!("request failed with status {status}"));
            let classified = classify_http_error(status, code.as_deref(), message);
            return Err(ControlApiError {
                status_code: status,
                code,
                message: classified.message,
                kind: classified.kind,
                retryable: classified.retryable,
            });
        }

        Ok(raw)
    }

    pub(super) fn storage_error(message: String) -> ControlApiError {
        ControlApiError {
            status_code: 500,
            code: Some("storage_error".to_string()),
            message,
            kind: CommandErrorKind::Unknown,
            retryable: false,
        }
    }

    pub(super) fn auth_input_validation_error(
        error: openagents_client_core::auth::AuthInputError,
    ) -> ControlApiError {
        ControlApiError {
            status_code: 422,
            code: Some("validation_error".to_string()),
            message: error.to_string(),
            kind: CommandErrorKind::Validation,
            retryable: false,
        }
    }

    pub(super) fn command_input_validation_error(
        error: openagents_client_core::command::CommandInputError,
    ) -> ControlApiError {
        ControlApiError {
            status_code: 422,
            code: Some("validation_error".to_string()),
            message: error.to_string(),
            kind: CommandErrorKind::Validation,
            retryable: false,
        }
    }

    pub(super) fn command_error_code(kind: &CommandErrorKind) -> &'static str {
        match kind {
            CommandErrorKind::MissingCredential => "missing_credential",
            CommandErrorKind::Unauthorized => "unauthorized",
            CommandErrorKind::Forbidden => "forbidden",
            CommandErrorKind::Validation => "validation",
            CommandErrorKind::ServiceUnavailable => "service_unavailable",
            CommandErrorKind::RateLimited => "rate_limited",
            CommandErrorKind::Network => "network",
            CommandErrorKind::Decode => "decode",
            CommandErrorKind::Unsupported => "unsupported",
            CommandErrorKind::Unknown => "unknown",
        }
    }

    pub(super) fn map_session_status(raw: &str) -> SessionLifecycleStatus {
        match raw {
            "active" => SessionLifecycleStatus::Active,
            "reauth_required" => SessionLifecycleStatus::ReauthRequired,
            "expired" => SessionLifecycleStatus::Expired,
            "revoked" => SessionLifecycleStatus::Revoked,
            _ => SessionLifecycleStatus::ReauthRequired,
        }
    }
