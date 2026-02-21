#![allow(clippy::needless_pass_by_value)]

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::{Cell, RefCell};

    use gloo_net::http::Request;
    use openagents_app_state::{
        AppAction, AppState, AuthUser, CommandIntent, SessionLifecycleStatus, SessionSnapshot,
        apply_action,
    };
    use openagents_ui_core::{ShellCardSpec, draw_shell_backdrop, draw_shell_card};
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{HtmlCanvasElement, HtmlElement};
    use wgpui::{Platform, Scene, WebPlatform, run_animation_loop, setup_resize_observer};

    thread_local! {
        static APP: RefCell<Option<WebShellApp>> = const { RefCell::new(None) };
        static APP_STATE: RefCell<AppState> = RefCell::new(AppState::default());
        static DIAGNOSTICS: RefCell<BootDiagnostics> = RefCell::new(BootDiagnostics::default());
        static COMMAND_LOOP_ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    const AUTH_STORAGE_KEY: &str = "openagents.web.auth.v1";

    #[derive(Debug, Clone, Serialize)]
    struct BootDiagnostics {
        phase: String,
        detail: String,
        frames_rendered: u64,
        route_path: String,
        pending_intents: usize,
        last_error: Option<String>,
    }

    impl Default for BootDiagnostics {
        fn default() -> Self {
            Self {
                phase: "idle".to_string(),
                detail: "web shell not started".to_string(),
                frames_rendered: 0,
                route_path: "/".to_string(),
                pending_intents: 0,
                last_error: None,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct StoredAuthTokens {
        token_type: String,
        access_token: String,
        refresh_token: String,
    }

    #[derive(Debug, Clone)]
    struct ControlApiError {
        status_code: u16,
        code: Option<String>,
        message: String,
    }

    impl ControlApiError {
        fn unauthorized(message: impl Into<String>) -> Self {
            Self {
                status_code: 401,
                code: Some("unauthorized".to_string()),
                message: message.into(),
            }
        }

        fn is_unauthorized(&self) -> bool {
            self.status_code == 401
        }
    }

    impl std::fmt::Display for ControlApiError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if let Some(code) = &self.code {
                write!(f, "{} ({code}, status={})", self.message, self.status_code)
            } else {
                write!(f, "{} (status={})", self.message, self.status_code)
            }
        }
    }

    #[derive(Debug, Deserialize)]
    struct ApiErrorBody {
        message: Option<String>,
        error: Option<ApiErrorDetail>,
    }

    #[derive(Debug, Deserialize)]
    struct ApiErrorDetail {
        code: Option<String>,
        message: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct SendCodeResponse {
        email: String,
        #[serde(rename = "challengeId")]
        challenge_id: String,
    }

    #[derive(Debug, Deserialize)]
    struct VerifyCodeResponse {
        #[serde(rename = "tokenType")]
        token_type: String,
        token: String,
        #[serde(rename = "refreshToken")]
        refresh_token: String,
        #[serde(rename = "sessionId")]
        _session_id: String,
    }

    #[derive(Debug, Deserialize)]
    struct RefreshResponse {
        #[serde(rename = "tokenType")]
        token_type: String,
        token: String,
        #[serde(rename = "refreshToken")]
        refresh_token: String,
    }

    #[derive(Debug, Deserialize)]
    struct SessionResponse {
        data: SessionResponseData,
    }

    #[derive(Debug, Deserialize)]
    struct SessionResponseData {
        session: SessionData,
        user: SessionUser,
    }

    #[derive(Debug, Deserialize)]
    struct SessionData {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "userId")]
        user_id: String,
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "tokenName")]
        token_name: String,
        #[serde(rename = "activeOrgId")]
        active_org_id: String,
        status: String,
        #[serde(rename = "reauthRequired")]
        reauth_required: bool,
        #[serde(rename = "issuedAt")]
        issued_at: Option<String>,
        #[serde(rename = "accessExpiresAt")]
        access_expires_at: Option<String>,
        #[serde(rename = "refreshExpiresAt")]
        refresh_expires_at: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct SessionUser {
        id: String,
        email: String,
        name: String,
        #[serde(rename = "workosId")]
        workos_id: String,
    }

    #[derive(Debug, Clone)]
    struct SessionSnapshotWithUser {
        session: SessionSnapshot,
        user: AuthUser,
    }

    struct WebShellApp {
        platform: WebPlatform,
        scene: Scene,
    }

    impl WebShellApp {
        fn new(platform: WebPlatform) -> Self {
            Self {
                platform,
                scene: Scene::new(),
            }
        }

        fn render_frame(&mut self) -> Result<(), String> {
            self.scene.clear();
            let size = self.platform.logical_size();
            draw_shell_backdrop(&mut self.scene, size);
            let _ = draw_shell_card(&mut self.scene, size, ShellCardSpec::default());

            self.platform.render(&self.scene)
        }
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        set_boot_phase("booting", "initializing OpenAgents web shell runtime");
        spawn_local(async {
            if let Err(error) = boot().await {
                set_boot_error(&error);
            }
        });
    }

    #[wasm_bindgen]
    pub fn boot_diagnostics_json() -> String {
        DIAGNOSTICS.with(|state| {
            serde_json::to_string(&*state.borrow()).unwrap_or_else(|_| {
                "{\"phase\":\"error\",\"detail\":\"diagnostics serialization failed\"}".to_string()
            })
        })
    }

    #[wasm_bindgen]
    pub fn app_state_json() -> String {
        APP_STATE.with(|state| {
            serde_json::to_string(&*state.borrow()).unwrap_or_else(|_| "{}".to_string())
        })
    }

    #[wasm_bindgen]
    pub fn auth_state_json() -> String {
        APP_STATE.with(|state| {
            serde_json::to_string(&state.borrow().auth).unwrap_or_else(|_| "{}".to_string())
        })
    }

    #[wasm_bindgen]
    pub fn auth_send_code(email: String) {
        queue_intent(CommandIntent::StartAuthChallenge { email });
    }

    #[wasm_bindgen]
    pub fn auth_verify_code(code: String) {
        queue_intent(CommandIntent::VerifyAuthCode { code });
    }

    #[wasm_bindgen]
    pub fn auth_refresh_session() {
        queue_intent(CommandIntent::RefreshSession);
    }

    #[wasm_bindgen]
    pub fn auth_restore_session() {
        queue_intent(CommandIntent::RestoreSession);
    }

    #[wasm_bindgen]
    pub fn auth_logout() {
        queue_intent(CommandIntent::LogoutSession);
    }

    async fn boot() -> Result<(), String> {
        if should_force_boot_failure() {
            return Err("forced startup failure because query contains oa_boot_fail=1".to_string());
        }

        let canvas = ensure_shell_dom()?;

        let current_path = current_pathname();
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(
                &mut state,
                AppAction::BootstrapFromPath {
                    path: current_path.clone(),
                },
            );
            let _ = apply_action(
                &mut state,
                AppAction::QueueIntent {
                    intent: CommandIntent::Bootstrap,
                },
            );
            let _ = apply_action(
                &mut state,
                AppAction::QueueIntent {
                    intent: CommandIntent::RestoreSession,
                },
            );
            let route_path = state.route.to_path();
            let pending_intents = state.intent_queue.len();
            update_diagnostics_from_state(route_path, pending_intents);
        });

        set_boot_phase("booting", "initializing GPU platform");
        let platform = WebPlatform::init_on_canvas(canvas).await?;
        let app = WebShellApp::new(platform);

        setup_resize_observer(app.platform.canvas(), || {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    app.platform.handle_resize();
                }
            });
        });

        APP.with(|cell| {
            *cell.borrow_mut() = Some(app);
        });

        set_boot_phase("ready", "render loop active");
        run_animation_loop(|| {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    if let Err(error) = app.render_frame() {
                        set_boot_error(&format!("render loop failure: {error}"));
                        return;
                    }
                    DIAGNOSTICS.with(|state| {
                        let mut state = state.borrow_mut();
                        state.frames_rendered = state.frames_rendered.saturating_add(1);
                    });
                }
            });
        });

        schedule_command_processing();

        Ok(())
    }

    fn queue_intent(intent: CommandIntent) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, AppAction::QueueIntent { intent });
            update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
        });
        schedule_command_processing();
    }

    fn schedule_command_processing() {
        let already_active = COMMAND_LOOP_ACTIVE.with(|active| {
            if active.get() {
                true
            } else {
                active.set(true);
                false
            }
        });

        if already_active {
            return;
        }

        spawn_local(async {
            loop {
                let intents = APP_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let drained = apply_action(&mut state, AppAction::DrainIntents).drained_intents;
                    update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
                    drained
                });

                if intents.is_empty() {
                    break;
                }

                for queued_intent in intents {
                    let outcome = handle_intent(queued_intent.intent).await;
                    APP_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        match outcome {
                            Ok(()) => {
                                let _ = apply_action(
                                    &mut state,
                                    AppAction::IntentCompleted {
                                        id: queued_intent.id,
                                    },
                                );
                            }
                            Err(message) => {
                                let _ = apply_action(
                                    &mut state,
                                    AppAction::IntentFailed {
                                        id: queued_intent.id,
                                        message: message.clone(),
                                    },
                                );
                                let _ = apply_action(&mut state, AppAction::AuthFailed { message });
                            }
                        }
                        update_diagnostics_from_state(
                            state.route.to_path(),
                            state.intent_queue.len(),
                        );
                    });
                }
            }

            COMMAND_LOOP_ACTIVE.with(|active| active.set(false));

            let has_pending = APP_STATE.with(|state| !state.borrow().intent_queue.is_empty());
            if has_pending {
                schedule_command_processing();
            }
        });
    }

    async fn handle_intent(intent: CommandIntent) -> Result<(), String> {
        match intent {
            CommandIntent::Bootstrap => Ok(()),
            CommandIntent::StartAuthChallenge { email } => {
                apply_auth_action(AppAction::AuthChallengeRequested {
                    email: email.clone(),
                });
                let response = post_send_code(&email)
                    .await
                    .map_err(|error| format!("auth challenge failed: {error}"))?;
                apply_auth_action(AppAction::AuthChallengeAccepted {
                    email: response.email,
                    challenge_id: response.challenge_id,
                });
                Ok(())
            }
            CommandIntent::VerifyAuthCode { code } => {
                apply_auth_action(AppAction::AuthVerifyRequested);
                verify_code_flow(code).await
            }
            CommandIntent::RestoreSession => restore_session_flow().await,
            CommandIntent::RefreshSession => refresh_session_flow().await,
            CommandIntent::LogoutSession => logout_flow().await,
            CommandIntent::RequestSyncToken { .. } => Ok(()),
            CommandIntent::ConnectStream { .. } => Ok(()),
            CommandIntent::DisconnectStream => Ok(()),
            CommandIntent::SendThreadMessage { .. } => Ok(()),
            CommandIntent::Navigate { route } => {
                APP_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let _ = apply_action(&mut state, AppAction::Navigate { route });
                    update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
                });
                Ok(())
            }
        }
    }

    async fn verify_code_flow(code: String) -> Result<(), String> {
        let challenge_id = APP_STATE.with(|state| state.borrow().auth.challenge_id.clone());
        let verified = post_verify_code(&code, challenge_id.as_deref())
            .await
            .map_err(|error| format!("auth verify failed: {error}"))?;

        let tokens = StoredAuthTokens {
            token_type: verified.token_type.clone(),
            access_token: verified.token.clone(),
            refresh_token: verified.refresh_token.clone(),
        };

        persist_tokens(&tokens)?;

        match get_current_session(&tokens.access_token).await {
            Ok(hydrated) => {
                apply_auth_action(AppAction::AuthSessionEstablished {
                    user: hydrated.user,
                    session: hydrated.session,
                    token_type: tokens.token_type,
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                });
                Ok(())
            }
            Err(error) if error.is_unauthorized() => match refresh_then_hydrate(tokens).await {
                Ok(hydrated) => {
                    apply_auth_action(AppAction::AuthSessionEstablished {
                        user: hydrated.user,
                        session: hydrated.session,
                        token_type: hydrated.token_type,
                        access_token: hydrated.access_token,
                        refresh_token: hydrated.refresh_token,
                    });
                    Ok(())
                }
                Err(refresh_error) => {
                    clear_persisted_tokens();
                    apply_auth_action(AppAction::AuthReauthRequired {
                        message: "Reauthentication required.".to_string(),
                    });
                    Err(format!("session hydrate failed: {refresh_error}"))
                }
            },
            Err(error) => Err(format!("session hydrate failed: {error}")),
        }
    }

    async fn restore_session_flow() -> Result<(), String> {
        apply_auth_action(AppAction::AuthSessionRestoreRequested);

        let tokens = load_tokens()
            .or_else(auth_tokens_from_state)
            .ok_or_else(|| {
                apply_auth_action(AppAction::AuthSignedOut);
                "no stored session".to_string()
            })?;

        hydrate_or_refresh(tokens).await
    }

    async fn hydrate_or_refresh(tokens: StoredAuthTokens) -> Result<(), String> {
        match get_current_session(&tokens.access_token).await {
            Ok(snapshot) => {
                persist_tokens(&tokens)?;
                apply_auth_action(AppAction::AuthSessionEstablished {
                    user: snapshot.user,
                    session: snapshot.session,
                    token_type: tokens.token_type,
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                });
                Ok(())
            }
            Err(error) => {
                if error.is_unauthorized() {
                    match refresh_then_hydrate(tokens).await {
                        Ok(snapshot) => {
                            apply_auth_action(AppAction::AuthSessionEstablished {
                                user: snapshot.user,
                                session: snapshot.session,
                                token_type: snapshot.token_type,
                                access_token: snapshot.access_token,
                                refresh_token: snapshot.refresh_token,
                            });
                            Ok(())
                        }
                        Err(refresh_error) => {
                            clear_persisted_tokens();
                            apply_auth_action(AppAction::AuthReauthRequired {
                                message: "Session expired. Sign in again.".to_string(),
                            });
                            Err(format!("session restore failed: {refresh_error}"))
                        }
                    }
                } else {
                    Err(format!("session restore failed: {error}"))
                }
            }
        }
    }

    async fn refresh_session_flow() -> Result<(), String> {
        apply_auth_action(AppAction::AuthSessionRefreshRequested);

        let tokens = load_tokens()
            .or_else(auth_tokens_from_state)
            .ok_or_else(|| "refresh token is unavailable".to_string())?;

        let refreshed = post_refresh_session(&tokens.refresh_token)
            .await
            .map_err(|error| format!("session refresh failed: {error}"))?;

        let next_tokens = StoredAuthTokens {
            token_type: refreshed.token_type,
            access_token: refreshed.token,
            refresh_token: refreshed.refresh_token,
        };

        persist_tokens(&next_tokens)?;
        let hydrated = get_current_session(&next_tokens.access_token)
            .await
            .map_err(|error| format!("session refresh hydrate failed: {error}"))?;

        apply_auth_action(AppAction::AuthSessionEstablished {
            user: hydrated.user,
            session: hydrated.session,
            token_type: next_tokens.token_type,
            access_token: next_tokens.access_token,
            refresh_token: next_tokens.refresh_token,
        });
        Ok(())
    }

    async fn logout_flow() -> Result<(), String> {
        let access_token = APP_STATE
            .with(|state| state.borrow().auth.access_token.clone())
            .unwrap_or_default();

        if !access_token.is_empty() {
            post_logout(&access_token)
                .await
                .map_err(|error| format!("logout failed: {error}"))?;
        }

        clear_persisted_tokens();
        apply_auth_action(AppAction::AuthSignedOut);
        Ok(())
    }

    fn apply_auth_action(action: AppAction) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, action);
            update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
        });
    }

    async fn refresh_then_hydrate(
        tokens: StoredAuthTokens,
    ) -> Result<HydratedSessionWithTokens, ControlApiError> {
        let refreshed = post_refresh_session(&tokens.refresh_token).await?;
        let next_tokens = StoredAuthTokens {
            token_type: refreshed.token_type,
            access_token: refreshed.token,
            refresh_token: refreshed.refresh_token,
        };
        persist_tokens(&next_tokens).map_err(|message| ControlApiError {
            status_code: 500,
            code: Some("storage_error".to_string()),
            message,
        })?;
        let hydrated = get_current_session(&next_tokens.access_token).await?;
        Ok(HydratedSessionWithTokens {
            user: hydrated.user,
            session: hydrated.session,
            token_type: next_tokens.token_type,
            access_token: next_tokens.access_token,
            refresh_token: next_tokens.refresh_token,
        })
    }

    #[derive(Debug)]
    struct HydratedSessionWithTokens {
        user: AuthUser,
        session: SessionSnapshot,
        token_type: String,
        access_token: String,
        refresh_token: String,
    }

    async fn post_send_code(email: &str) -> Result<SendCodeResponse, ControlApiError> {
        let body = serde_json::json!({ "email": email });
        send_json("/api/auth/email", Some(body.to_string()), None).await
    }

    async fn post_verify_code(
        code: &str,
        challenge_id: Option<&str>,
    ) -> Result<VerifyCodeResponse, ControlApiError> {
        let mut payload = serde_json::json!({ "code": code });
        if let Some(challenge_id) = challenge_id {
            payload["challenge_id"] = serde_json::Value::String(challenge_id.to_string());
        }
        send_json(
            "/api/auth/verify",
            Some(payload.to_string()),
            Some(("x-client", "openagents-web-shell")),
        )
        .await
    }

    async fn post_refresh_session(refresh_token: &str) -> Result<RefreshResponse, ControlApiError> {
        let payload = serde_json::json!({
            "refresh_token": refresh_token,
            "rotate_refresh_token": true,
        });
        send_json("/api/auth/refresh", Some(payload.to_string()), None).await
    }

    async fn post_logout(access_token: &str) -> Result<serde_json::Value, ControlApiError> {
        send_json_with_auth("/api/auth/logout", None, access_token).await
    }

    async fn get_current_session(
        access_token: &str,
    ) -> Result<SessionSnapshotWithUser, ControlApiError> {
        let response: SessionResponse =
            send_json_with_auth("/api/auth/session", None, access_token).await?;
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

    async fn send_json<T: for<'de> Deserialize<'de>>(
        path: &str,
        body: Option<String>,
        header: Option<(&str, &str)>,
    ) -> Result<T, ControlApiError> {
        let mut request_builder = Request::post(path).header("content-type", "application/json");
        if let Some((header_name, header_value)) = header {
            request_builder = request_builder.header(header_name, header_value);
        }

        let response = if let Some(body) = body {
            let request = request_builder
                .body(body)
                .map_err(|error| ControlApiError {
                    status_code: 500,
                    code: Some("request_build_failed".to_string()),
                    message: format!("failed to build request body: {error}"),
                })?;
            request.send().await.map_err(map_network_error)?
        } else {
            request_builder.send().await.map_err(map_network_error)?
        };

        decode_json_response(response).await
    }

    async fn send_json_with_auth<T: for<'de> Deserialize<'de>>(
        path: &str,
        body: Option<String>,
        access_token: &str,
    ) -> Result<T, ControlApiError> {
        let mut request_builder = Request::post(path)
            .header("content-type", "application/json")
            .header("authorization", &format!("Bearer {access_token}"));
        if path.ends_with("/session") {
            request_builder =
                Request::get(path).header("authorization", &format!("Bearer {access_token}"));
        }

        let response = if let Some(body) = body {
            let request = request_builder
                .body(body)
                .map_err(|error| ControlApiError {
                    status_code: 500,
                    code: Some("request_build_failed".to_string()),
                    message: format!("failed to build request body: {error}"),
                })?;
            request.send().await.map_err(map_network_error)?
        } else {
            request_builder.send().await.map_err(map_network_error)?
        };
        decode_json_response(response).await
    }

    fn map_network_error(error: gloo_net::Error) -> ControlApiError {
        ControlApiError {
            status_code: 0,
            code: Some("network_error".to_string()),
            message: error.to_string(),
        }
    }

    async fn decode_json_response<T: for<'de> Deserialize<'de>>(
        response: gloo_net::http::Response,
    ) -> Result<T, ControlApiError> {
        let status = response.status();
        let raw = response.text().await.map_err(|error| ControlApiError {
            status_code: status,
            code: Some("response_read_failed".to_string()),
            message: error.to_string(),
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
            return Err(ControlApiError {
                status_code: status,
                code,
                message,
            });
        }

        serde_json::from_str(&raw).map_err(|error| ControlApiError {
            status_code: status,
            code: Some("decode_failed".to_string()),
            message: format!("failed to decode response: {error}"),
        })
    }

    fn map_session_status(raw: &str) -> SessionLifecycleStatus {
        match raw {
            "active" => SessionLifecycleStatus::Active,
            "reauth_required" => SessionLifecycleStatus::ReauthRequired,
            "expired" => SessionLifecycleStatus::Expired,
            "revoked" => SessionLifecycleStatus::Revoked,
            _ => SessionLifecycleStatus::ReauthRequired,
        }
    }

    fn load_tokens() -> Option<StoredAuthTokens> {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        let raw = storage.get_item(AUTH_STORAGE_KEY).ok()??;
        serde_json::from_str(&raw).ok()
    }

    fn auth_tokens_from_state() -> Option<StoredAuthTokens> {
        APP_STATE.with(|state| {
            let state = state.borrow();
            Some(StoredAuthTokens {
                token_type: state.auth.token_type.clone()?,
                access_token: state.auth.access_token.clone()?,
                refresh_token: state.auth.refresh_token.clone()?,
            })
        })
    }

    fn persist_tokens(tokens: &StoredAuthTokens) -> Result<(), String> {
        let Some(window) = web_sys::window() else {
            return Err("window is unavailable for token storage".to_string());
        };
        let storage = window
            .local_storage()
            .map_err(|_| "failed to access local storage".to_string())?
            .ok_or_else(|| "local storage is unavailable".to_string())?;
        let serialized = serde_json::to_string(tokens)
            .map_err(|error| format!("failed to serialize auth tokens: {error}"))?;
        storage
            .set_item(AUTH_STORAGE_KEY, &serialized)
            .map_err(|_| "failed to persist auth tokens".to_string())
    }

    fn clear_persisted_tokens() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(storage) = window.local_storage() else {
            return;
        };
        let Some(storage) = storage else {
            return;
        };
        let _ = storage.remove_item(AUTH_STORAGE_KEY);
    }

    fn ensure_shell_dom() -> Result<HtmlCanvasElement, String> {
        let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
        let document = window
            .document()
            .ok_or_else(|| "document is unavailable".to_string())?;
        let body = document
            .body()
            .ok_or_else(|| "document body is unavailable".to_string())?;

        let status = match document.get_element_by_id("openagents-web-shell-status") {
            Some(existing) => existing
                .dyn_into::<HtmlElement>()
                .map_err(|_| "status element exists but is not HtmlElement".to_string())?,
            None => {
                let element = document
                    .create_element("div")
                    .map_err(|_| "failed to create status element".to_string())?;
                element.set_id("openagents-web-shell-status");
                let status = element
                    .dyn_into::<HtmlElement>()
                    .map_err(|_| "status element is not HtmlElement".to_string())?;
                status
                    .style()
                    .set_property("position", "fixed")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("top", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("left", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("font-family", "monospace")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("font-size", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("color", "#cbd5e1")
                    .map_err(|_| "failed to style status element".to_string())?;
                body.append_child(&status)
                    .map_err(|_| "failed to append status element".to_string())?;
                status
            }
        };

        status.set_inner_text("Boot: starting");

        match document.get_element_by_id("openagents-web-shell-canvas") {
            Some(existing) => existing
                .dyn_into::<HtmlCanvasElement>()
                .map_err(|_| "canvas element exists but is not HtmlCanvasElement".to_string()),
            None => {
                let element = document
                    .create_element("canvas")
                    .map_err(|_| "failed to create canvas element".to_string())?;
                element.set_id("openagents-web-shell-canvas");
                let canvas = element
                    .dyn_into::<HtmlCanvasElement>()
                    .map_err(|_| "canvas element is not HtmlCanvasElement".to_string())?;
                canvas
                    .style()
                    .set_property("display", "block")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("width", "100vw")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("height", "100vh")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("background", "#080A10")
                    .map_err(|_| "failed to style canvas".to_string())?;
                body.append_child(&canvas)
                    .map_err(|_| "failed to append canvas".to_string())?;
                Ok(canvas)
            }
        }
    }

    fn set_boot_phase(phase: &str, detail: &str) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.phase = phase.to_string();
            state.detail = detail.to_string();
            if phase != "error" {
                state.last_error = None;
            }
        });
        update_status_dom(phase, detail, false);
    }

    fn set_boot_error(message: &str) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.phase = "error".to_string();
            state.detail = "startup failed".to_string();
            state.last_error = Some(message.to_string());
        });
        update_status_dom("error", message, true);
    }

    fn update_status_dom(phase: &str, detail: &str, is_error: bool) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(status) = document.get_element_by_id("openagents-web-shell-status") {
                    if let Ok(status) = status.dyn_into::<HtmlElement>() {
                        let label = if is_error { "Boot error" } else { "Boot" };
                        status.set_inner_text(&format!("{label}: {phase} ({detail})"));
                        let color = if is_error { "#f87171" } else { "#cbd5e1" };
                        let _ = status.style().set_property("color", color);
                    }
                }
            }
        }
    }

    fn should_force_boot_failure() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Ok(search) = window.location().search() else {
            return false;
        };
        search.contains("oa_boot_fail=1")
    }

    fn current_pathname() -> String {
        let Some(window) = web_sys::window() else {
            return "/".to_string();
        };
        let Ok(pathname) = window.location().pathname() else {
            return "/".to_string();
        };
        if pathname.trim().is_empty() {
            "/".to_string()
        } else {
            pathname
        }
    }

    fn update_diagnostics_from_state(route_path: String, pending_intents: usize) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.route_path = route_path;
            state.pending_intents = pending_intents;
        });
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::boot_diagnostics_json;

#[cfg(not(target_arch = "wasm32"))]
pub fn boot_diagnostics_json() -> String {
    "{\"phase\":\"native\",\"detail\":\"web shell diagnostics only available on wasm\"}".to_string()
}
