#![allow(clippy::needless_pass_by_value)]

#[cfg(any(target_arch = "wasm32", test))]
mod codex_thread;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::{Cell, RefCell};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use futures_util::{FutureExt, SinkExt, StreamExt, pin_mut, select};
    use gloo_net::http::Request;
    use gloo_net::websocket::{Message as WsMessage, futures::WebSocket};
    use gloo_timers::future::sleep;
    use openagents_app_state::{
        AppAction, AppRoute, AppState, AuthRequirement, AuthUser, CommandError, CommandErrorKind,
        CommandIntent, HttpCommandRequest, HttpMethod, SessionLifecycleStatus, SessionSnapshot,
        StreamStatus, apply_action, classify_http_error, command_latency_metric,
        map_intent_to_http,
    };
    use openagents_client_core::auth::{normalize_email, normalize_verification_code};
    use openagents_client_core::command::normalize_thread_message_text;
    use openagents_client_core::khala_protocol::{
        KhalaEventPayload, PhoenixFrame, SyncErrorPayload, TopicWatermark, WatermarkDecision,
        apply_watermark, build_phoenix_frame, decode_khala_payload, parse_phoenix_frame,
    };
    use openagents_client_core::sync_persistence::{
        PersistedSyncState, decode_sync_state, normalized_topics, resume_after_map,
    };
    use openagents_client_core::web_sync_storage::{
        LoadedSyncSnapshot, PersistedSyncSnapshot, PersistedViewState, WEB_SYNC_DB_NAME,
        clear_sync_snapshot_in_indexeddb, load_sync_snapshot_from_indexeddb,
        persist_sync_snapshot_to_indexeddb,
    };
    use openagents_ui_core::{ShellCardSpec, draw_shell_backdrop, draw_shell_card};
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{HtmlCanvasElement, HtmlElement, HtmlInputElement};
    use wgpui::{Platform, Scene, WebPlatform, run_animation_loop, setup_resize_observer};

    use crate::codex_thread::CodexThreadState;

    thread_local! {
        static APP: RefCell<Option<WebShellApp>> = const { RefCell::new(None) };
        static APP_STATE: RefCell<AppState> = RefCell::new(AppState::default());
        static DIAGNOSTICS: RefCell<BootDiagnostics> = RefCell::new(BootDiagnostics::default());
        static COMMAND_LOOP_ACTIVE: Cell<bool> = const { Cell::new(false) };
        static KHALA_STREAM_ENABLED: Cell<bool> = const { Cell::new(false) };
        static KHALA_STREAM_RUNNING: Cell<bool> = const { Cell::new(false) };
        static KHALA_REF_COUNTER: Cell<u64> = const { Cell::new(1) };
        static SYNC_RUNTIME_STATE: RefCell<SyncRuntimeState> = RefCell::new(SyncRuntimeState::default());
        static SYNC_LAST_PERSIST_AT_MS: Cell<u64> = const { Cell::new(0) };
        static CODEX_THREAD_STATE: RefCell<CodexThreadState> = RefCell::new(CodexThreadState::default());
        static MANAGEMENT_SURFACE_STATE: RefCell<ManagementSurfaceState> = RefCell::new(ManagementSurfaceState::default());
        static MANAGEMENT_SURFACE_LOADING: Cell<bool> = const { Cell::new(false) };
        static CODEX_SEND_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static CODEX_INPUT_KEYDOWN_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>> = const { RefCell::new(None) };
        static AUTH_SEND_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_VERIFY_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_RESTORE_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_LOGOUT_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
    }

    const AUTH_STORAGE_KEY: &str = "openagents.web.auth.v1";
    const SYNC_STATE_STORAGE_KEY: &str = "openagents.web.sync.v1";
    const KHALA_CHANNEL_TOPIC: &str = "sync:v1";
    const KHALA_WS_VSN: &str = "2.0.0";
    const KHALA_DEFAULT_TOPIC: &str = "runtime.codex_worker_events";
    const KHALA_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
    const KHALA_POLL_INTERVAL: Duration = Duration::from_secs(4);
    const KHALA_RECONNECT_BASE_DELAY_MS: u64 = 750;
    const KHALA_RECONNECT_MAX_DELAY_MS: u64 = 8_000;
    const SYNC_PERSIST_MIN_INTERVAL_MS: u64 = 800;
    const CODEX_CHAT_ROOT_ID: &str = "openagents-web-shell-chat";
    const CODEX_CHAT_HEADER_ID: &str = "openagents-web-shell-chat-header";
    const CODEX_CHAT_MESSAGES_ID: &str = "openagents-web-shell-chat-messages";
    const CODEX_CHAT_COMPOSER_ID: &str = "openagents-web-shell-chat-composer";
    const CODEX_CHAT_INPUT_ID: &str = "openagents-web-shell-chat-input";
    const CODEX_CHAT_SEND_ID: &str = "openagents-web-shell-chat-send";
    const AUTH_PANEL_ID: &str = "openagents-web-shell-auth-panel";
    const AUTH_EMAIL_INPUT_ID: &str = "openagents-web-shell-auth-email";
    const AUTH_CODE_INPUT_ID: &str = "openagents-web-shell-auth-code";
    const AUTH_SEND_ID: &str = "openagents-web-shell-auth-send";
    const AUTH_VERIFY_ID: &str = "openagents-web-shell-auth-verify";
    const AUTH_RESTORE_ID: &str = "openagents-web-shell-auth-restore";
    const AUTH_LOGOUT_ID: &str = "openagents-web-shell-auth-logout";
    const DOM_READY_BUDGET_MS: u64 = 450;
    const GPU_INIT_BUDGET_MS: u64 = 1_600;
    const FIRST_FRAME_BUDGET_MS: u64 = 2_200;
    const BOOT_TOTAL_BUDGET_MS: u64 = 2_500;

    #[derive(Debug, Clone, Default)]
    struct SyncRuntimeState {
        subscribed_topics: Vec<String>,
    }

    #[derive(Debug, Clone, Default)]
    struct ManagementSurfaceState {
        loaded_session_id: Option<String>,
        memberships: Vec<MembershipRecord>,
        active_org_id: Option<String>,
        route_split_status: Option<RouteSplitStatus>,
        billing_policy: Option<PolicyDecision>,
        last_error: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct MembershipsResponse {
        data: MembershipsPayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct MembershipsPayload {
        #[serde(rename = "activeOrgId")]
        active_org_id: String,
        memberships: Vec<MembershipRecord>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct MembershipRecord {
        #[serde(rename = "orgId")]
        org_id: String,
        #[serde(rename = "orgSlug")]
        org_slug: String,
        role: String,
        #[serde(rename = "roleScopes")]
        role_scopes: Vec<String>,
        #[serde(rename = "defaultOrg")]
        default_org: bool,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RouteSplitStatusResponse {
        data: RouteSplitStatus,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RouteSplitStatus {
        enabled: bool,
        mode: String,
        #[serde(rename = "cohort_percentage")]
        #[serde(alias = "cohortPercentage")]
        cohort_percentage: u8,
        #[serde(rename = "rust_routes")]
        #[serde(alias = "rustRoutes")]
        rust_routes: Vec<String>,
        #[serde(rename = "force_legacy")]
        #[serde(alias = "forceLegacy")]
        force_legacy: bool,
        #[serde(rename = "legacy_base_url")]
        #[serde(alias = "legacyBaseUrl")]
        legacy_base_url: Option<String>,
        #[serde(rename = "override_target")]
        #[serde(alias = "overrideTarget")]
        override_target: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct PolicyDecisionResponse {
        data: PolicyDecision,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct PolicyDecision {
        allowed: bool,
        #[serde(rename = "resolved_org_id")]
        #[serde(alias = "resolvedOrgId")]
        resolved_org_id: String,
        #[serde(rename = "granted_scopes")]
        #[serde(alias = "grantedScopes")]
        granted_scopes: Vec<String>,
        #[serde(rename = "denied_reasons")]
        #[serde(alias = "deniedReasons")]
        denied_reasons: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct BootDiagnostics {
        phase: String,
        detail: String,
        frames_rendered: u64,
        boot_started_at_unix_ms: Option<u64>,
        dom_ready_latency_ms: Option<u64>,
        gpu_init_latency_ms: Option<u64>,
        first_frame_latency_ms: Option<u64>,
        boot_total_latency_ms: Option<u64>,
        render_backend: Option<String>,
        capability_mode: Option<String>,
        budget_breaches: Vec<String>,
        route_path: String,
        pending_intents: usize,
        command_total: u64,
        command_failures: u64,
        last_command: Option<String>,
        last_command_latency_ms: Option<u64>,
        last_command_error_kind: Option<String>,
        last_error: Option<String>,
    }

    impl Default for BootDiagnostics {
        fn default() -> Self {
            Self {
                phase: "idle".to_string(),
                detail: "web shell not started".to_string(),
                frames_rendered: 0,
                boot_started_at_unix_ms: None,
                dom_ready_latency_ms: None,
                gpu_init_latency_ms: None,
                first_frame_latency_ms: None,
                boot_total_latency_ms: None,
                render_backend: None,
                capability_mode: None,
                budget_breaches: Vec::new(),
                route_path: "/".to_string(),
                pending_intents: 0,
                command_total: 0,
                command_failures: 0,
                last_command: None,
                last_command_latency_ms: None,
                last_command_error_kind: None,
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
        kind: CommandErrorKind,
        retryable: bool,
    }

    impl ControlApiError {
        fn unauthorized(message: impl Into<String>) -> Self {
            Self {
                status_code: 401,
                code: Some("unauthorized".to_string()),
                message: message.into(),
                kind: CommandErrorKind::Unauthorized,
                retryable: false,
            }
        }

        fn is_unauthorized(&self) -> bool {
            self.status_code == 401
        }

        fn from_command_error(error: CommandError) -> Self {
            Self {
                status_code: 0,
                code: Some(command_error_code(&error.kind).to_string()),
                message: error.message,
                kind: error.kind,
                retryable: error.retryable,
            }
        }

        fn to_command_error(&self) -> CommandError {
            CommandError {
                kind: self.kind.clone(),
                message: self.message.clone(),
                retryable: self.retryable,
            }
        }
    }

    impl std::fmt::Display for ControlApiError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if let Some(code) = &self.code {
                write!(
                    f,
                    "{} ({code}, status={}, kind={:?}, retryable={})",
                    self.message, self.status_code, self.kind, self.retryable
                )
            } else {
                write!(
                    f,
                    "{} (status={}, kind={:?}, retryable={})",
                    self.message, self.status_code, self.kind, self.retryable
                )
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

    #[derive(Debug, Deserialize)]
    struct SyncTokenEnvelope {
        data: SyncTokenData,
    }

    #[derive(Debug, Deserialize)]
    struct SyncTokenData {
        token: String,
        #[serde(default)]
        scopes: Vec<String>,
        #[serde(default)]
        granted_topics: Vec<SyncTokenGrant>,
    }

    #[derive(Debug, Deserialize)]
    struct SyncTokenGrant {
        topic: String,
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
    pub fn codex_thread_state_json() -> String {
        CODEX_THREAD_STATE.with(|state| {
            serde_json::to_string(&*state.borrow()).unwrap_or_else(|_| "{}".to_string())
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

    #[wasm_bindgen]
    pub fn khala_connect() {
        queue_intent(CommandIntent::ConnectStream { worker_id: None });
    }

    #[wasm_bindgen]
    pub fn khala_disconnect() {
        queue_intent(CommandIntent::DisconnectStream);
    }

    #[wasm_bindgen]
    pub fn codex_send_message(text: String) {
        let Some(thread_id) = active_thread_id() else {
            CODEX_THREAD_STATE.with(|state| {
                state
                    .borrow_mut()
                    .append_local_system_message("No active thread route.");
            });
            render_codex_chat_dom();
            return;
        };
        CODEX_THREAD_STATE.with(|state| {
            state.borrow_mut().append_local_user_message(&text);
        });
        render_codex_chat_dom();
        queue_intent(CommandIntent::SendThreadMessage { thread_id, text });
    }

    async fn boot() -> Result<(), String> {
        if should_force_boot_failure() {
            return Err("forced startup failure because query contains oa_boot_fail=1".to_string());
        }

        let boot_started_at = Instant::now();
        let boot_started_at_unix_ms = now_unix_ms();
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.boot_started_at_unix_ms = Some(boot_started_at_unix_ms);
            state.dom_ready_latency_ms = None;
            state.gpu_init_latency_ms = None;
            state.first_frame_latency_ms = None;
            state.boot_total_latency_ms = None;
            state.render_backend = None;
            state.capability_mode = detect_gpu_mode_hint();
            state.budget_breaches.clear();
        });

        let canvas = ensure_shell_dom()?;
        ensure_codex_chat_dom()?;

        let current_path = current_pathname();
        let persisted_sync_state = restore_persisted_sync_state().await;
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(
                &mut state,
                AppAction::BootstrapFromPath {
                    path: current_path.clone(),
                },
            );
            if let Some(persisted_sync_state) = persisted_sync_state.as_ref() {
                hydrate_stream_state(&mut state, persisted_sync_state);
            }
            sync_thread_route_from_state(&state);
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
        schedule_management_surface_refresh();
        render_codex_chat_dom();

        let dom_ready_latency_ms =
            u64::try_from(boot_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        record_boot_milestone(
            "dom_ready_latency_ms",
            dom_ready_latency_ms,
            DOM_READY_BUDGET_MS,
        );

        set_boot_phase("booting", "initializing GPU platform");
        let gpu_init_started_at = Instant::now();
        let platform = WebPlatform::init_on_canvas(canvas).await?;
        let gpu_init_latency_ms =
            u64::try_from(gpu_init_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        record_boot_milestone(
            "gpu_init_latency_ms",
            gpu_init_latency_ms,
            GPU_INIT_BUDGET_MS,
        );
        let backend_name = platform.backend_name().to_string();
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.render_backend = Some(backend_name);
        });
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
        let boot_start_for_render = boot_started_at;
        run_animation_loop(move || {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    if let Err(error) = app.render_frame() {
                        set_boot_error(&format!("render loop failure: {error}"));
                        return;
                    }
                    DIAGNOSTICS.with(|state| {
                        let mut state = state.borrow_mut();
                        state.frames_rendered = state.frames_rendered.saturating_add(1);
                        if state.frames_rendered == 1 {
                            let first_frame_latency_ms =
                                u64::try_from(boot_start_for_render.elapsed().as_millis())
                                    .unwrap_or(u64::MAX);
                            state.first_frame_latency_ms = Some(first_frame_latency_ms);
                            state.boot_total_latency_ms = Some(first_frame_latency_ms);
                            if first_frame_latency_ms > FIRST_FRAME_BUDGET_MS {
                                state.budget_breaches.push(format!(
                                    "first_frame_latency_ms>{FIRST_FRAME_BUDGET_MS} (actual={first_frame_latency_ms})"
                                ));
                            }
                            if first_frame_latency_ms > BOOT_TOTAL_BUDGET_MS {
                                state.budget_breaches.push(format!(
                                    "boot_total_latency_ms>{BOOT_TOTAL_BUDGET_MS} (actual={first_frame_latency_ms})"
                                ));
                            }
                        }
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
                    let intent = queued_intent.intent;
                    let started_at = Instant::now();
                    let outcome = handle_intent(intent.clone()).await;
                    let latency_ms =
                        u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let metric = match &outcome {
                        Ok(()) => command_latency_metric(&intent, latency_ms, Ok(())),
                        Err(error) => {
                            let command_error = error.to_command_error();
                            command_latency_metric(&intent, latency_ms, Err(&command_error))
                        }
                    };
                    record_command_metric(metric);

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
                            Err(error) => {
                                let _ = apply_action(
                                    &mut state,
                                    AppAction::IntentFailed {
                                        id: queued_intent.id,
                                        message: error.to_string(),
                                    },
                                );
                                let _ = apply_action(
                                    &mut state,
                                    AppAction::AuthFailed {
                                        message: error.message.clone(),
                                    },
                                );
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

    fn record_command_metric(metric: openagents_app_state::CommandLatencyMetric) {
        DIAGNOSTICS.with(|diagnostics| {
            let mut diagnostics = diagnostics.borrow_mut();
            diagnostics.command_total = diagnostics.command_total.saturating_add(1);
            diagnostics.last_command = Some(metric.intent.clone());
            diagnostics.last_command_latency_ms = Some(metric.latency_ms);
            diagnostics.last_command_error_kind = metric
                .error_kind
                .as_ref()
                .map(|kind| command_error_code(kind).to_string());
            if !metric.success {
                diagnostics.command_failures = diagnostics.command_failures.saturating_add(1);
            }
        });
    }

    async fn handle_intent(intent: CommandIntent) -> Result<(), ControlApiError> {
        match intent {
            CommandIntent::Bootstrap => Ok(()),
            CommandIntent::StartAuthChallenge { email } => {
                apply_auth_action(AppAction::AuthChallengeRequested {
                    email: email.clone(),
                });
                let response = post_send_code(&email).await?;
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
            CommandIntent::RequestSyncToken { scopes } => {
                let _ = mint_sync_token(scopes).await?;
                Ok(())
            }
            CommandIntent::ConnectStream { .. } => {
                ensure_khala_stream_running();
                Ok(())
            }
            CommandIntent::DisconnectStream => {
                stop_khala_stream();
                Ok(())
            }
            CommandIntent::SendThreadMessage { thread_id, text } => {
                send_thread_message_flow(thread_id, text).await
            }
            CommandIntent::Navigate { route } => {
                APP_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let _ = apply_action(&mut state, AppAction::Navigate { route });
                    update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
                });
                let state = snapshot_state();
                sync_thread_route_from_state(&state);
                schedule_management_surface_refresh();
                render_codex_chat_dom();
                Ok(())
            }
        }
    }

    async fn verify_code_flow(code: String) -> Result<(), ControlApiError> {
        let challenge_id = APP_STATE.with(|state| state.borrow().auth.challenge_id.clone());
        let verified = post_verify_code(&code, challenge_id.as_deref()).await?;

        let tokens = StoredAuthTokens {
            token_type: verified.token_type.clone(),
            access_token: verified.token.clone(),
            refresh_token: verified.refresh_token.clone(),
        };

        persist_tokens(&tokens).map_err(storage_error)?;

        match get_current_session(&tokens.access_token).await {
            Ok(hydrated) => {
                apply_auth_action(AppAction::AuthSessionEstablished {
                    user: hydrated.user,
                    session: hydrated.session,
                    token_type: tokens.token_type,
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                });
                on_auth_session_established();
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
                    on_auth_session_established();
                    Ok(())
                }
                Err(refresh_error) => {
                    clear_persisted_tokens();
                    clear_persisted_sync_state();
                    clear_runtime_sync_state();
                    reset_management_surface_state();
                    apply_auth_action(AppAction::AuthReauthRequired {
                        message: "Reauthentication required.".to_string(),
                    });
                    Err(refresh_error)
                }
            },
            Err(error) => Err(error),
        }
    }

    async fn restore_session_flow() -> Result<(), ControlApiError> {
        apply_auth_action(AppAction::AuthSessionRestoreRequested);

        let Some(tokens) = load_tokens().or_else(auth_tokens_from_state) else {
            reset_management_surface_state();
            apply_auth_action(AppAction::AuthSignedOut);
            return Ok(());
        };

        hydrate_or_refresh(tokens).await
    }

    async fn hydrate_or_refresh(tokens: StoredAuthTokens) -> Result<(), ControlApiError> {
        match get_current_session(&tokens.access_token).await {
            Ok(snapshot) => {
                persist_tokens(&tokens).map_err(storage_error)?;
                apply_auth_action(AppAction::AuthSessionEstablished {
                    user: snapshot.user,
                    session: snapshot.session,
                    token_type: tokens.token_type,
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                });
                on_auth_session_established();
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
                            on_auth_session_established();
                            Ok(())
                        }
                        Err(refresh_error) => {
                            clear_persisted_tokens();
                            clear_persisted_sync_state();
                            clear_runtime_sync_state();
                            reset_management_surface_state();
                            apply_auth_action(AppAction::AuthReauthRequired {
                                message: "Session expired. Sign in again.".to_string(),
                            });
                            Err(refresh_error)
                        }
                    }
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn refresh_session_flow() -> Result<(), ControlApiError> {
        apply_auth_action(AppAction::AuthSessionRefreshRequested);

        let tokens = load_tokens()
            .or_else(auth_tokens_from_state)
            .ok_or_else(|| {
                ControlApiError::from_command_error(CommandError::missing_credential(
                    "refresh token is unavailable",
                ))
            })?;

        let refreshed = post_refresh_session(&tokens.refresh_token).await?;

        let next_tokens = StoredAuthTokens {
            token_type: refreshed.token_type,
            access_token: refreshed.token,
            refresh_token: refreshed.refresh_token,
        };

        persist_tokens(&next_tokens).map_err(storage_error)?;
        let hydrated = get_current_session(&next_tokens.access_token).await?;

        apply_auth_action(AppAction::AuthSessionEstablished {
            user: hydrated.user,
            session: hydrated.session,
            token_type: next_tokens.token_type,
            access_token: next_tokens.access_token,
            refresh_token: next_tokens.refresh_token,
        });
        on_auth_session_established();
        Ok(())
    }

    async fn logout_flow() -> Result<(), ControlApiError> {
        let access_token = APP_STATE
            .with(|state| state.borrow().auth.access_token.clone())
            .unwrap_or_default();

        if !access_token.is_empty() {
            post_logout(&access_token).await?;
        }

        clear_persisted_tokens();
        clear_persisted_sync_state();
        clear_runtime_sync_state();
        reset_management_surface_state();
        stop_khala_stream();
        apply_auth_action(AppAction::AuthSignedOut);
        Ok(())
    }

    async fn send_thread_message_flow(
        thread_id: String,
        text: String,
    ) -> Result<(), ControlApiError> {
        let normalized =
            normalize_thread_message_text(&text).map_err(command_input_validation_error)?;

        let state = snapshot_state();
        let intent = CommandIntent::SendThreadMessage {
            thread_id,
            text: normalized,
        };
        let request = plan_http_request(&intent, &state)?;
        let response = send_json_request::<serde_json::Value>(&request, &state).await;
        if let Err(error) = &response {
            CODEX_THREAD_STATE.with(|chat| {
                chat.borrow_mut()
                    .append_local_system_message(&format!("Send failed: {}", error.message));
            });
            render_codex_chat_dom();
        }
        response.map(|_| ())
    }

    fn apply_auth_action(action: AppAction) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, action);
            update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
        });
        render_codex_chat_dom();
    }

    fn on_auth_session_established() {
        queue_intent(CommandIntent::ConnectStream { worker_id: None });
        schedule_management_surface_refresh();
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
        persist_tokens(&next_tokens).map_err(storage_error)?;
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

    fn ensure_khala_stream_running() {
        KHALA_STREAM_ENABLED.with(|enabled| enabled.set(true));

        let already_running = KHALA_STREAM_RUNNING.with(|running| {
            if running.get() {
                true
            } else {
                running.set(true);
                false
            }
        });

        if already_running {
            return;
        }

        set_stream_status(StreamStatus::Connecting);

        spawn_local(async {
            let mut reconnect_attempt: u32 = 0;

            loop {
                if !khala_stream_enabled() {
                    break;
                }

                match run_khala_session().await {
                    Ok(()) => {
                        reconnect_attempt = 0;
                        if !khala_stream_enabled() {
                            break;
                        }
                        set_stream_status(StreamStatus::Connecting);
                    }
                    Err(error) => {
                        if error.kind == CommandErrorKind::Unauthorized
                            || error.kind == CommandErrorKind::Forbidden
                        {
                            KHALA_STREAM_ENABLED.with(|enabled| enabled.set(false));
                            set_stream_status(StreamStatus::Error {
                                message: error.message,
                            });
                            break;
                        }

                        set_stream_status(StreamStatus::Error {
                            message: error.message.clone(),
                        });

                        reconnect_attempt = reconnect_attempt.saturating_add(1);
                        let backoff_ms = reconnect_backoff_ms(reconnect_attempt);
                        sleep(Duration::from_millis(backoff_ms)).await;
                        set_stream_status(StreamStatus::Connecting);
                    }
                }
            }

            KHALA_STREAM_RUNNING.with(|running| running.set(false));
            if !khala_stream_enabled() {
                set_stream_status(StreamStatus::Disconnected);
            }
        });
    }

    fn stop_khala_stream() {
        KHALA_STREAM_ENABLED.with(|enabled| enabled.set(false));
        set_stream_status(StreamStatus::Disconnected);
    }

    fn khala_stream_enabled() -> bool {
        KHALA_STREAM_ENABLED.with(Cell::get)
    }

    fn reconnect_backoff_ms(attempt: u32) -> u64 {
        let multiplier = 2_u64.saturating_pow(attempt.min(6));
        let candidate = KHALA_RECONNECT_BASE_DELAY_MS.saturating_mul(multiplier);
        candidate.min(KHALA_RECONNECT_MAX_DELAY_MS)
    }

    fn set_stream_status(status: StreamStatus) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, AppAction::StreamStatusChanged { status });
            update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
        });
    }

    async fn run_khala_session() -> Result<(), ControlApiError> {
        let sync_token = mint_sync_token(desired_sync_topics()).await?;
        let socket_url = build_sync_websocket_url(&sync_token.token)?;
        let mut socket = WebSocket::open(&socket_url).map_err(map_websocket_error)?;

        let join_ref = next_khala_ref();
        send_khala_frame(
            &mut socket,
            None,
            Some(&join_ref),
            "phx_join",
            serde_json::json!({}),
        )
        .await?;
        let _join_response = await_khala_reply(&mut socket, &join_ref).await?;

        let topics = resolve_khala_topics(&sync_token);
        set_subscribed_topics(topics.clone());
        persist_sync_state_snapshot_nonfatal();
        let subscribe_ref = next_khala_ref();
        let resume_after = current_resume_after(&topics);
        send_khala_frame(
            &mut socket,
            Some(&join_ref),
            Some(&subscribe_ref),
            "sync:subscribe",
            serde_json::json!({
                "topics": topics,
                "resume_after": resume_after,
                "replay_batch_size": 200,
            }),
        )
        .await?;
        let _subscribe_response = await_khala_reply(&mut socket, &subscribe_ref).await?;

        set_stream_status(StreamStatus::Live);
        let mut last_heartbeat_at = Instant::now();

        while khala_stream_enabled() {
            let next_frame = socket.next().fuse();
            let tick = sleep(KHALA_POLL_INTERVAL).fuse();
            pin_mut!(next_frame, tick);

            select! {
                websocket_message = next_frame => {
                    match websocket_message {
                        Some(Ok(message)) => {
                            process_khala_message(message).await?;
                        }
                        Some(Err(error)) => return Err(map_websocket_error(error)),
                        None => {
                            return Err(ControlApiError {
                                status_code: 0,
                                code: Some("khala_socket_closed".to_string()),
                                message: "Khala socket closed.".to_string(),
                                kind: CommandErrorKind::Network,
                                retryable: true,
                            });
                        }
                    }
                }
                _ = tick => {}
            }

            if last_heartbeat_at.elapsed() >= KHALA_HEARTBEAT_INTERVAL {
                let heartbeat_ref = next_khala_ref();
                send_khala_frame(
                    &mut socket,
                    Some(&join_ref),
                    Some(&heartbeat_ref),
                    "sync:heartbeat",
                    serde_json::json!({}),
                )
                .await?;
                last_heartbeat_at = Instant::now();
            }
        }

        Ok(())
    }

    fn next_khala_ref() -> String {
        KHALA_REF_COUNTER.with(|counter| {
            let next = counter.get().saturating_add(1);
            counter.set(next);
            next.to_string()
        })
    }

    async fn send_khala_frame(
        socket: &mut WebSocket,
        join_ref: Option<&str>,
        reference: Option<&str>,
        event: &str,
        payload: serde_json::Value,
    ) -> Result<(), ControlApiError> {
        let raw = build_phoenix_frame(join_ref, reference, KHALA_CHANNEL_TOPIC, event, payload);
        socket
            .send(WsMessage::Text(raw))
            .await
            .map_err(map_websocket_error)
    }

    async fn await_khala_reply(
        socket: &mut WebSocket,
        expected_ref: &str,
    ) -> Result<serde_json::Value, ControlApiError> {
        loop {
            let Some(next_message) = socket.next().await else {
                return Err(ControlApiError {
                    status_code: 0,
                    code: Some("khala_reply_closed".to_string()),
                    message: "Khala socket closed while waiting for reply.".to_string(),
                    kind: CommandErrorKind::Network,
                    retryable: true,
                });
            };
            let message = next_message.map_err(map_websocket_error)?;
            let raw = websocket_text(message)?;
            let Some(frame) = parse_phoenix_frame(&raw) else {
                continue;
            };

            if frame.topic != KHALA_CHANNEL_TOPIC {
                continue;
            }

            if frame.event == "phx_reply" && frame.reference.as_deref() == Some(expected_ref) {
                let payload =
                    frame
                        .payload
                        .as_object()
                        .cloned()
                        .ok_or_else(|| ControlApiError {
                            status_code: 500,
                            code: Some("khala_invalid_reply".to_string()),
                            message: "Invalid Khala reply payload.".to_string(),
                            kind: CommandErrorKind::Decode,
                            retryable: false,
                        })?;

                let status = payload
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("error");

                if status == "ok" {
                    return Ok(payload
                        .get("response")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({})));
                }

                let response = payload
                    .get("response")
                    .and_then(serde_json::Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let code = response
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string);
                let message = response
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "Khala request failed.".to_string());
                if code.as_deref() == Some("stale_cursor") {
                    let topics = response
                        .get("stale_topics")
                        .and_then(serde_json::Value::as_array)
                        .map(|entries| {
                            entries
                                .iter()
                                .filter_map(|entry| {
                                    entry
                                        .get("topic")
                                        .and_then(serde_json::Value::as_str)
                                        .map(ToString::to_string)
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if !topics.is_empty() {
                        apply_topic_watermarks_reset(topics);
                    }
                }
                let classified = classify_http_error(409, code.as_deref(), message);
                return Err(ControlApiError {
                    status_code: 409,
                    code,
                    message: classified.message,
                    kind: classified.kind,
                    retryable: classified.retryable,
                });
            }

            apply_khala_frame(&frame)?;
        }
    }

    async fn process_khala_message(message: WsMessage) -> Result<(), ControlApiError> {
        let raw = websocket_text(message)?;
        let Some(frame) = parse_phoenix_frame(&raw) else {
            return Ok(());
        };
        if frame.event == "phx_error" || frame.event == "phx_close" {
            return Err(ControlApiError {
                status_code: 0,
                code: Some("khala_channel_error".to_string()),
                message: "Khala channel closed with error.".to_string(),
                kind: CommandErrorKind::Network,
                retryable: true,
            });
        }
        apply_khala_frame(&frame)
    }

    fn apply_khala_frame(frame: &PhoenixFrame) -> Result<(), ControlApiError> {
        let Some(payload) = decode_khala_payload(frame) else {
            return Ok(());
        };

        match payload {
            KhalaEventPayload::UpdateBatch(batch) => {
                for update in batch.updates {
                    if let Some(payload) = update.payload.as_ref() {
                        ingest_codex_thread_payload(payload);
                    }
                    let current = current_watermark(&update.topic);
                    match apply_watermark(current, update.watermark) {
                        WatermarkDecision::Advanced { next } => {
                            apply_topic_watermark_update(update.topic.clone(), next);
                        }
                        WatermarkDecision::Duplicate | WatermarkDecision::OutOfOrder { .. } => {}
                    }
                }
                Ok(())
            }
            KhalaEventPayload::Heartbeat(watermarks) => {
                apply_heartbeat_watermarks(watermarks);
                Ok(())
            }
            KhalaEventPayload::Error(error) => handle_khala_error(error),
            KhalaEventPayload::Other => Ok(()),
        }
    }

    fn apply_heartbeat_watermarks(watermarks: Vec<TopicWatermark>) {
        for watermark in watermarks {
            let current = current_watermark(&watermark.topic);
            if let WatermarkDecision::Advanced { next } =
                apply_watermark(current, watermark.watermark)
            {
                apply_topic_watermark_update(watermark.topic.clone(), next);
            }
        }
    }

    fn handle_khala_error(error: SyncErrorPayload) -> Result<(), ControlApiError> {
        if error.code == "stale_cursor" {
            let topics: Vec<String> = error
                .stale_topics
                .iter()
                .map(|topic| topic.topic.clone())
                .collect();
            if !topics.is_empty() {
                apply_topic_watermarks_reset(topics);
            }

            return Err(ControlApiError {
                status_code: 409,
                code: Some("stale_cursor".to_string()),
                message: error.message,
                kind: CommandErrorKind::Validation,
                retryable: true,
            });
        }

        let status = if error.code == "unauthorized" {
            401
        } else if error.code == "forbidden_topic" {
            403
        } else {
            500
        };
        let classified = classify_http_error(status, Some(&error.code), error.message);
        Err(ControlApiError {
            status_code: status,
            code: Some(error.code),
            message: classified.message,
            kind: classified.kind,
            retryable: classified.retryable,
        })
    }

    fn current_watermark(topic: &str) -> u64 {
        APP_STATE.with(|state| {
            state
                .borrow()
                .stream
                .topic_watermarks
                .get(topic)
                .copied()
                .unwrap_or(0)
        })
    }

    fn current_resume_after(topics: &[String]) -> serde_json::Map<String, serde_json::Value> {
        APP_STATE.with(|state| {
            let state = state.borrow();
            resume_after_map(topics, &state.stream.topic_watermarks)
                .into_iter()
                .map(|(topic, watermark)| (topic, serde_json::Value::Number(watermark.into())))
                .collect()
        })
    }

    fn active_thread_id() -> Option<String> {
        APP_STATE.with(|state| thread_id_from_route(&state.borrow().route))
    }

    fn sync_thread_route_from_state(state: &AppState) {
        CODEX_THREAD_STATE.with(|chat| {
            chat.borrow_mut()
                .set_thread_id(thread_id_from_route(&state.route));
        });
    }

    fn thread_id_from_route(route: &AppRoute) -> Option<String> {
        match route {
            AppRoute::Chat { thread_id } => thread_id.clone(),
            AppRoute::Home
            | AppRoute::Login
            | AppRoute::Register
            | AppRoute::Authenticate
            | AppRoute::Onboarding { .. }
            | AppRoute::Workers
            | AppRoute::Account { .. }
            | AppRoute::Settings { .. }
            | AppRoute::Billing { .. }
            | AppRoute::Admin { .. }
            | AppRoute::Debug => None,
        }
    }

    fn route_is_management_surface(route: &AppRoute) -> bool {
        matches!(
            route,
            AppRoute::Account { .. }
                | AppRoute::Settings { .. }
                | AppRoute::Billing { .. }
                | AppRoute::Admin { .. }
        )
    }

    fn route_is_auth_surface(route: &AppRoute) -> bool {
        matches!(
            route,
            AppRoute::Login
                | AppRoute::Register
                | AppRoute::Authenticate
                | AppRoute::Onboarding { .. }
        )
    }

    fn reset_management_surface_state() {
        MANAGEMENT_SURFACE_LOADING.with(|loading| loading.set(false));
        MANAGEMENT_SURFACE_STATE.with(|state| {
            *state.borrow_mut() = ManagementSurfaceState::default();
        });
    }

    fn schedule_management_surface_refresh() {
        let (access_token, session_id, route) = APP_STATE.with(|state| {
            let state = state.borrow();
            (
                state.auth.access_token.clone(),
                state
                    .auth
                    .session
                    .as_ref()
                    .map(|session| session.session_id.clone()),
                state.route.clone(),
            )
        });

        let Some(access_token) = access_token else {
            return;
        };
        let Some(session_id) = session_id else {
            return;
        };
        if access_token.trim().is_empty() {
            return;
        }
        if !route_is_management_surface(&route) {
            return;
        }

        let already_loaded = MANAGEMENT_SURFACE_STATE.with(|state| {
            let state = state.borrow();
            state.loaded_session_id.as_deref() == Some(session_id.as_str())
                && state.last_error.is_none()
        });
        if already_loaded {
            return;
        }

        let already_loading = MANAGEMENT_SURFACE_LOADING.with(|loading| {
            if loading.get() {
                true
            } else {
                loading.set(true);
                false
            }
        });
        if already_loading {
            return;
        }

        spawn_local(async move {
            let result = fetch_management_surface_state(&access_token, &session_id, &route).await;
            MANAGEMENT_SURFACE_LOADING.with(|loading| loading.set(false));
            MANAGEMENT_SURFACE_STATE.with(|state| {
                let mut state = state.borrow_mut();
                match result {
                    Ok(snapshot) => {
                        *state = snapshot;
                    }
                    Err(error) => {
                        state.loaded_session_id = Some(session_id.clone());
                        state.last_error = Some(error.message.clone());
                    }
                }
            });
            render_codex_chat_dom();
        });
    }

    async fn fetch_management_surface_state(
        access_token: &str,
        session_id: &str,
        route: &AppRoute,
    ) -> Result<ManagementSurfaceState, ControlApiError> {
        let memberships_request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/orgs/memberships".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let memberships: MembershipsResponse =
            send_json_request(&memberships_request, &AppState::default()).await?;

        let route_split_request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/v1/control/route-split/status".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let route_split: RouteSplitStatusResponse =
            send_json_request(&route_split_request, &AppState::default()).await?;

        let billing_policy =
            fetch_billing_policy(access_token, &memberships.data.active_org_id, route).await?;

        Ok(ManagementSurfaceState {
            loaded_session_id: Some(session_id.to_string()),
            memberships: memberships.data.memberships,
            active_org_id: Some(memberships.data.active_org_id),
            route_split_status: Some(route_split.data),
            billing_policy,
            last_error: None,
        })
    }

    async fn fetch_billing_policy(
        access_token: &str,
        active_org_id: &str,
        route: &AppRoute,
    ) -> Result<Option<PolicyDecision>, ControlApiError> {
        let AppRoute::Billing { section } = route else {
            return Ok(None);
        };

        let required_scopes = billing_required_scopes(section.as_deref());
        let authorize_request = HttpCommandRequest {
            method: HttpMethod::Post,
            path: "/api/policy/authorize".to_string(),
            body: Some(serde_json::json!({
                "orgId": active_org_id,
                "requiredScopes": required_scopes,
                "requestedTopics": [],
            })),
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };

        let response: PolicyDecisionResponse =
            send_json_request(&authorize_request, &AppState::default()).await?;
        Ok(Some(response.data))
    }

    fn billing_required_scopes(section: Option<&str>) -> Vec<String> {
        let mut scopes = vec!["runtime.read".to_string(), "policy.evaluate".to_string()];
        if matches!(section, Some("paywalls")) {
            scopes.push("runtime.write".to_string());
            scopes.push("org.membership.write".to_string());
        }
        scopes
    }

    fn ingest_codex_thread_payload(payload: &serde_json::Value) {
        let changed =
            CODEX_THREAD_STATE.with(|chat| chat.borrow_mut().ingest_khala_payload(payload));
        if changed {
            render_codex_chat_dom();
        }
    }

    fn desired_sync_topics() -> Vec<String> {
        let topics = subscribed_topics();
        if topics.is_empty() {
            vec![KHALA_DEFAULT_TOPIC.to_string()]
        } else {
            topics
        }
    }

    fn apply_topic_watermark_update(topic: String, watermark: u64) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(
                &mut state,
                AppAction::TopicWatermarkUpdated { topic, watermark },
            );
        });
        persist_sync_state_snapshot_nonfatal();
    }

    fn apply_topic_watermarks_reset(topics: Vec<String>) {
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, AppAction::TopicWatermarksReset { topics });
        });
        persist_sync_state_snapshot_nonfatal_force();
    }

    fn hydrate_stream_state(state: &mut AppState, persisted_sync_state: &PersistedSyncSnapshot) {
        state.stream.topic_watermarks = persisted_sync_state.sync_state.topic_watermarks.clone();
        state.stream.last_seq = persisted_sync_state
            .view_state
            .last_seq
            .or_else(|| state.stream.topic_watermarks.values().copied().max());
        state.stream.active_worker_id = persisted_sync_state.view_state.active_worker_id.clone();
        set_subscribed_topics(persisted_sync_state.sync_state.subscribed_topics.clone());
    }

    async fn restore_persisted_sync_state() -> Option<PersistedSyncSnapshot> {
        match load_sync_snapshot_from_indexeddb(WEB_SYNC_DB_NAME).await {
            Ok(Some(loaded)) => {
                if loaded.migrated {
                    set_boot_phase("booting", "migrated sync persistence payload in IndexedDB");
                    if let Err(error) =
                        persist_sync_state_snapshot_from(&loaded.snapshot, true).await
                    {
                        DIAGNOSTICS
                            .with(|diagnostics| diagnostics.borrow_mut().last_error = Some(error));
                    }
                }
                Some(loaded.snapshot)
            }
            Ok(None) => match load_legacy_sync_state_from_local_storage() {
                Ok(Some(loaded)) => {
                    set_boot_phase(
                        "booting",
                        "migrating legacy sync persistence from localStorage to IndexedDB",
                    );
                    let snapshot = loaded.snapshot;
                    if let Err(error) = persist_sync_state_snapshot_from(&snapshot, true).await {
                        DIAGNOSTICS
                            .with(|diagnostics| diagnostics.borrow_mut().last_error = Some(error));
                    } else {
                        clear_legacy_sync_state_from_local_storage();
                    }
                    Some(snapshot)
                }
                Ok(None) => None,
                Err(error) => {
                    clear_persisted_sync_state();
                    clear_runtime_sync_state();
                    set_boot_phase(
                        "booting",
                        &format!("resetting invalid legacy sync persistence payload: {error}"),
                    );
                    None
                }
            },
            Err(error) => {
                clear_persisted_sync_state();
                clear_runtime_sync_state();
                set_boot_phase(
                    "booting",
                    &format!("resetting invalid indexeddb sync persistence payload: {error}"),
                );
                None
            }
        }
    }

    fn load_legacy_sync_state_from_local_storage() -> Result<Option<LoadedSyncSnapshot>, String> {
        let Some(window) = web_sys::window() else {
            return Ok(None);
        };
        let storage = window
            .local_storage()
            .map_err(|_| "failed to access local storage for sync state".to_string())?;
        let Some(storage) = storage else {
            return Ok(None);
        };
        let raw = storage
            .get_item(SYNC_STATE_STORAGE_KEY)
            .map_err(|_| "failed to read sync state from local storage".to_string())?;
        let Some(raw) = raw else {
            return Ok(None);
        };

        let (sync_state, sync_migrated) = decode_sync_state(&raw)
            .map_err(|error| format!("failed to decode persisted sync state: {error}"))?;
        Ok(Some(LoadedSyncSnapshot {
            snapshot: PersistedSyncSnapshot {
                sync_state,
                view_state: PersistedViewState::default(),
            },
            migrated: sync_migrated,
        }))
    }

    fn persist_sync_state_snapshot_nonfatal() {
        spawn_local(async move {
            if let Err(error) = persist_sync_state_snapshot(false).await {
                DIAGNOSTICS.with(|diagnostics| diagnostics.borrow_mut().last_error = Some(error));
            }
        });
    }

    fn persist_sync_state_snapshot_nonfatal_force() {
        spawn_local(async move {
            if let Err(error) = persist_sync_state_snapshot(true).await {
                DIAGNOSTICS.with(|diagnostics| diagnostics.borrow_mut().last_error = Some(error));
            }
        });
    }

    async fn persist_sync_state_snapshot(force: bool) -> Result<(), String> {
        let snapshot = APP_STATE.with(|state| {
            let state = state.borrow();
            PersistedSyncSnapshot {
                sync_state: PersistedSyncState {
                    topic_watermarks: state.stream.topic_watermarks.clone(),
                    subscribed_topics: subscribed_topics(),
                    updated_at_unix_ms: current_unix_ms(),
                    ..PersistedSyncState::default()
                },
                view_state: PersistedViewState {
                    schema_version: PersistedViewState::default().schema_version,
                    active_worker_id: state.stream.active_worker_id.clone(),
                    last_seq: state.stream.last_seq,
                    updated_at_unix_ms: current_unix_ms(),
                },
            }
        });
        persist_sync_state_snapshot_from(&snapshot, force).await
    }

    async fn persist_sync_state_snapshot_from(
        snapshot: &PersistedSyncSnapshot,
        force: bool,
    ) -> Result<(), String> {
        let now = current_unix_ms();
        if !force {
            let persisted_recently = SYNC_LAST_PERSIST_AT_MS
                .with(|last| now.saturating_sub(last.get()) < SYNC_PERSIST_MIN_INTERVAL_MS);
            if persisted_recently {
                return Ok(());
            }
        }

        persist_sync_state(snapshot).await?;
        SYNC_LAST_PERSIST_AT_MS.with(|last| last.set(now));
        Ok(())
    }

    async fn persist_sync_state(snapshot: &PersistedSyncSnapshot) -> Result<(), String> {
        persist_sync_snapshot_to_indexeddb(WEB_SYNC_DB_NAME, snapshot)
            .await
            .map_err(|error| format!("failed to persist indexeddb sync state: {error}"))
    }

    fn clear_persisted_sync_state() {
        clear_legacy_sync_state_from_local_storage();
        SYNC_LAST_PERSIST_AT_MS.with(|last| last.set(0));

        spawn_local(async move {
            if let Err(error) = clear_sync_snapshot_in_indexeddb(WEB_SYNC_DB_NAME).await {
                DIAGNOSTICS.with(|diagnostics| {
                    diagnostics.borrow_mut().last_error =
                        Some(format!("failed to clear indexeddb sync state: {error}"));
                });
            }
        });
    }

    fn clear_legacy_sync_state_from_local_storage() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(storage) = window.local_storage() else {
            return;
        };
        let Some(storage) = storage else {
            return;
        };
        let _ = storage.remove_item(SYNC_STATE_STORAGE_KEY);
    }

    fn subscribed_topics() -> Vec<String> {
        SYNC_RUNTIME_STATE.with(|state| state.borrow().subscribed_topics.clone())
    }

    fn set_subscribed_topics(topics: Vec<String>) {
        let topics = normalized_topics(topics);
        SYNC_RUNTIME_STATE.with(|state| state.borrow_mut().subscribed_topics = topics);
    }

    fn clear_runtime_sync_state() {
        set_subscribed_topics(Vec::new());
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.stream.topic_watermarks.clear();
            state.stream.last_seq = None;
            state.stream.active_worker_id = None;
            state.stream.status = StreamStatus::Disconnected;
        });
    }

    fn current_unix_ms() -> u64 {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
    }

    async fn mint_sync_token(scopes: Vec<String>) -> Result<SyncTokenData, ControlApiError> {
        let mut state = snapshot_state();
        if state.auth.access_token.is_none() {
            return Err(ControlApiError::from_command_error(
                CommandError::missing_credential("access token is unavailable"),
            ));
        }

        let resolved_scopes = if scopes.is_empty() {
            vec![KHALA_DEFAULT_TOPIC.to_string()]
        } else {
            scopes
        };
        let request = plan_http_request(
            &CommandIntent::RequestSyncToken {
                scopes: resolved_scopes,
            },
            &state,
        )?;
        let response: SyncTokenEnvelope = send_json_request(&request, &state).await?;
        let token = response.data.token.trim().to_string();
        if token.is_empty() {
            return Err(ControlApiError {
                status_code: 500,
                code: Some("sync_token_missing".to_string()),
                message: "Sync token response is missing token.".to_string(),
                kind: CommandErrorKind::Decode,
                retryable: false,
            });
        }
        state.auth.access_token = state.auth.access_token.clone();
        Ok(response.data)
    }

    fn resolve_khala_topics(token: &SyncTokenData) -> Vec<String> {
        let mut topics = token
            .granted_topics
            .iter()
            .map(|grant| grant.topic.clone())
            .filter(|topic| !topic.trim().is_empty())
            .collect::<Vec<_>>();

        if topics.is_empty() {
            topics = token
                .scopes
                .iter()
                .map(|scope| scope.trim().to_string())
                .filter(|scope| scope.starts_with("runtime."))
                .collect();
        }

        if topics.is_empty() {
            topics.push(KHALA_DEFAULT_TOPIC.to_string());
        }

        topics.sort();
        topics.dedup();
        topics
    }

    fn build_sync_websocket_url(token: &str) -> Result<String, ControlApiError> {
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

    fn websocket_text(message: WsMessage) -> Result<String, ControlApiError> {
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

    async fn post_send_code(email: &str) -> Result<SendCodeResponse, ControlApiError> {
        let normalized_email = normalize_email(email).map_err(auth_input_validation_error)?;
        let state = snapshot_state();
        let intent = CommandIntent::StartAuthChallenge {
            email: normalized_email,
        };
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    async fn post_verify_code(
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

    async fn post_refresh_session(refresh_token: &str) -> Result<RefreshResponse, ControlApiError> {
        let mut state = snapshot_state();
        state.auth.refresh_token = Some(refresh_token.to_string());
        let intent = CommandIntent::RefreshSession;
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    async fn post_logout(access_token: &str) -> Result<serde_json::Value, ControlApiError> {
        let mut state = snapshot_state();
        state.auth.access_token = Some(access_token.to_string());
        let intent = CommandIntent::LogoutSession;
        let request = plan_http_request(&intent, &state)?;
        send_json_request(&request, &state).await
    }

    async fn get_current_session(
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

    fn snapshot_state() -> AppState {
        APP_STATE.with(|state| state.borrow().clone())
    }

    fn plan_http_request(
        intent: &CommandIntent,
        state: &AppState,
    ) -> Result<HttpCommandRequest, ControlApiError> {
        map_intent_to_http(intent, state).map_err(ControlApiError::from_command_error)
    }

    async fn send_json_request<T: for<'de> Deserialize<'de>>(
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

    fn resolve_bearer_token(auth: &AuthRequirement, state: &AppState) -> Option<String> {
        match auth {
            AuthRequirement::None => None,
            AuthRequirement::AccessToken => state.auth.access_token.clone(),
            AuthRequirement::RefreshToken => state.auth.refresh_token.clone(),
        }
    }

    fn map_network_error(error: gloo_net::Error) -> ControlApiError {
        let classified = classify_http_error(0, Some("network_error"), error.to_string());
        ControlApiError {
            status_code: 0,
            code: Some("network_error".to_string()),
            message: classified.message,
            kind: classified.kind,
            retryable: classified.retryable,
        }
    }

    fn map_websocket_error<E: std::fmt::Display>(error: E) -> ControlApiError {
        let classified = classify_http_error(0, Some("network_error"), error.to_string());
        ControlApiError {
            status_code: 0,
            code: Some("network_error".to_string()),
            message: classified.message,
            kind: classified.kind,
            retryable: classified.retryable,
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

    fn storage_error(message: String) -> ControlApiError {
        ControlApiError {
            status_code: 500,
            code: Some("storage_error".to_string()),
            message,
            kind: CommandErrorKind::Unknown,
            retryable: false,
        }
    }

    fn auth_input_validation_error(
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

    fn command_input_validation_error(
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

    fn command_error_code(kind: &CommandErrorKind) -> &'static str {
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

    fn ensure_codex_chat_dom() -> Result<(), String> {
        let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
        let document = window
            .document()
            .ok_or_else(|| "document is unavailable".to_string())?;
        let body = document
            .body()
            .ok_or_else(|| "document body is unavailable".to_string())?;

        if document.get_element_by_id(CODEX_CHAT_ROOT_ID).is_none() {
            let root = document
                .create_element("section")
                .map_err(|_| "failed to create codex chat root".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex chat root is not HtmlElement".to_string())?;
            root.set_id(CODEX_CHAT_ROOT_ID);
            root.style()
                .set_property("position", "fixed")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("inset", "0")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("display", "none")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("flex-direction", "column")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("justify-content", "space-between")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("padding", "72px 16px 108px")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("box-sizing", "border-box")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("z-index", "20")
                .map_err(|_| "failed to style codex chat root".to_string())?;
            root.style()
                .set_property("pointer-events", "none")
                .map_err(|_| "failed to style codex chat root".to_string())?;

            let header = document
                .create_element("div")
                .map_err(|_| "failed to create codex chat header".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex chat header is not HtmlElement".to_string())?;
            header.set_id(CODEX_CHAT_HEADER_ID);
            header
                .style()
                .set_property("color", "#cbd5e1")
                .map_err(|_| "failed to style codex chat header".to_string())?;
            header
                .style()
                .set_property("font-size", "12px")
                .map_err(|_| "failed to style codex chat header".to_string())?;
            header
                .style()
                .set_property(
                    "font-family",
                    "ui-monospace, SFMono-Regular, Menlo, monospace",
                )
                .map_err(|_| "failed to style codex chat header".to_string())?;
            header
                .style()
                .set_property("pointer-events", "none")
                .map_err(|_| "failed to style codex chat header".to_string())?;
            let _ = root.append_child(&header);

            let messages = document
                .create_element("div")
                .map_err(|_| "failed to create codex chat messages".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex chat messages is not HtmlElement".to_string())?;
            messages.set_id(CODEX_CHAT_MESSAGES_ID);
            messages
                .style()
                .set_property("display", "flex")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("flex-direction", "column")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("gap", "10px")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("max-width", "760px")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("width", "100%")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("margin", "0 auto")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("overflow-y", "auto")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("padding-right", "4px")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            messages
                .style()
                .set_property("pointer-events", "auto")
                .map_err(|_| "failed to style codex chat messages".to_string())?;
            let _ = root.append_child(&messages);

            let composer = document
                .create_element("div")
                .map_err(|_| "failed to create codex composer".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex composer is not HtmlElement".to_string())?;
            composer.set_id(CODEX_CHAT_COMPOSER_ID);
            composer
                .style()
                .set_property("display", "flex")
                .map_err(|_| "failed to style codex composer".to_string())?;
            composer
                .style()
                .set_property("gap", "8px")
                .map_err(|_| "failed to style codex composer".to_string())?;
            composer
                .style()
                .set_property("max-width", "760px")
                .map_err(|_| "failed to style codex composer".to_string())?;
            composer
                .style()
                .set_property("margin", "0 auto")
                .map_err(|_| "failed to style codex composer".to_string())?;
            composer
                .style()
                .set_property("width", "100%")
                .map_err(|_| "failed to style codex composer".to_string())?;
            composer
                .style()
                .set_property("pointer-events", "auto")
                .map_err(|_| "failed to style codex composer".to_string())?;

            let input = document
                .create_element("input")
                .map_err(|_| "failed to create codex input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "codex input is not HtmlInputElement".to_string())?;
            input.set_id(CODEX_CHAT_INPUT_ID);
            input.set_placeholder("Message Codex");
            let _ = input.style().set_property("flex", "1");
            let _ = input.style().set_property("height", "40px");
            let _ = input.style().set_property("padding", "0 12px");
            let _ = input.style().set_property("border-radius", "10px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#e2e8f0");
            let _ = input.style().set_property("font-size", "15px");
            let _ = input
                .style()
                .set_property("font-family", "Inter, sans-serif");
            let _ = composer.append_child(&input);

            let send_button = document
                .create_element("button")
                .map_err(|_| "failed to create codex send button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex send button is not HtmlElement".to_string())?;
            send_button.set_id(CODEX_CHAT_SEND_ID);
            send_button.set_inner_text("Send");
            let _ = send_button.style().set_property("height", "40px");
            let _ = send_button.style().set_property("padding", "0 16px");
            let _ = send_button.style().set_property("border-radius", "10px");
            let _ = send_button
                .style()
                .set_property("border", "1px solid #2563eb");
            let _ = send_button.style().set_property("background", "#2563eb");
            let _ = send_button.style().set_property("color", "#ffffff");
            let _ = send_button.style().set_property("font-weight", "600");
            let _ = composer.append_child(&send_button);

            let _ = root.append_child(&composer);

            let auth_panel = document
                .create_element("div")
                .map_err(|_| "failed to create auth panel".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth panel is not HtmlElement".to_string())?;
            auth_panel.set_id(AUTH_PANEL_ID);
            let _ = auth_panel.style().set_property("display", "none");
            let _ = auth_panel.style().set_property("flex-direction", "column");
            let _ = auth_panel.style().set_property("gap", "8px");
            let _ = auth_panel.style().set_property("max-width", "760px");
            let _ = auth_panel.style().set_property("margin", "0 auto");
            let _ = auth_panel.style().set_property("width", "100%");
            let _ = auth_panel.style().set_property("pointer-events", "auto");

            let auth_email_row = document
                .create_element("div")
                .map_err(|_| "failed to create auth email row".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth email row is not HtmlElement".to_string())?;
            let _ = auth_email_row.style().set_property("display", "flex");
            let _ = auth_email_row.style().set_property("gap", "8px");

            let auth_email_input = document
                .create_element("input")
                .map_err(|_| "failed to create auth email input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "auth email input is not HtmlInputElement".to_string())?;
            auth_email_input.set_id(AUTH_EMAIL_INPUT_ID);
            auth_email_input.set_placeholder("Email");
            let _ = auth_email_input.style().set_property("flex", "1");
            let _ = auth_email_input.style().set_property("height", "40px");
            let _ = auth_email_input.style().set_property("padding", "0 12px");
            let _ = auth_email_input
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_email_input
                .style()
                .set_property("border", "1px solid #1f2937");
            let _ = auth_email_input
                .style()
                .set_property("background", "#0f172a");
            let _ = auth_email_input.style().set_property("color", "#e2e8f0");
            let _ = auth_email_row.append_child(&auth_email_input);

            let auth_send_button = document
                .create_element("button")
                .map_err(|_| "failed to create auth send button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth send button is not HtmlElement".to_string())?;
            auth_send_button.set_id(AUTH_SEND_ID);
            auth_send_button.set_inner_text("Send code");
            let _ = auth_send_button.style().set_property("height", "40px");
            let _ = auth_send_button.style().set_property("padding", "0 14px");
            let _ = auth_send_button
                .style()
                .set_property("border", "1px solid #2563eb");
            let _ = auth_send_button
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_send_button
                .style()
                .set_property("background", "#2563eb");
            let _ = auth_send_button.style().set_property("color", "#ffffff");
            let _ = auth_email_row.append_child(&auth_send_button);
            let _ = auth_panel.append_child(&auth_email_row);

            let auth_code_row = document
                .create_element("div")
                .map_err(|_| "failed to create auth code row".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth code row is not HtmlElement".to_string())?;
            let _ = auth_code_row.style().set_property("display", "flex");
            let _ = auth_code_row.style().set_property("gap", "8px");

            let auth_code_input = document
                .create_element("input")
                .map_err(|_| "failed to create auth code input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "auth code input is not HtmlInputElement".to_string())?;
            auth_code_input.set_id(AUTH_CODE_INPUT_ID);
            auth_code_input.set_placeholder("Verification code");
            let _ = auth_code_input.style().set_property("flex", "1");
            let _ = auth_code_input.style().set_property("height", "40px");
            let _ = auth_code_input.style().set_property("padding", "0 12px");
            let _ = auth_code_input
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_code_input
                .style()
                .set_property("border", "1px solid #1f2937");
            let _ = auth_code_input
                .style()
                .set_property("background", "#0f172a");
            let _ = auth_code_input.style().set_property("color", "#e2e8f0");
            let _ = auth_code_row.append_child(&auth_code_input);

            let auth_verify_button = document
                .create_element("button")
                .map_err(|_| "failed to create auth verify button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth verify button is not HtmlElement".to_string())?;
            auth_verify_button.set_id(AUTH_VERIFY_ID);
            auth_verify_button.set_inner_text("Verify");
            let _ = auth_verify_button.style().set_property("height", "40px");
            let _ = auth_verify_button.style().set_property("padding", "0 14px");
            let _ = auth_verify_button
                .style()
                .set_property("border", "1px solid #10b981");
            let _ = auth_verify_button
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_verify_button
                .style()
                .set_property("background", "#10b981");
            let _ = auth_verify_button.style().set_property("color", "#ffffff");
            let _ = auth_code_row.append_child(&auth_verify_button);
            let _ = auth_panel.append_child(&auth_code_row);

            let auth_action_row = document
                .create_element("div")
                .map_err(|_| "failed to create auth action row".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth action row is not HtmlElement".to_string())?;
            let _ = auth_action_row.style().set_property("display", "flex");
            let _ = auth_action_row.style().set_property("gap", "8px");

            let auth_restore_button = document
                .create_element("button")
                .map_err(|_| "failed to create auth restore button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth restore button is not HtmlElement".to_string())?;
            auth_restore_button.set_id(AUTH_RESTORE_ID);
            auth_restore_button.set_inner_text("Restore session");
            let _ = auth_restore_button.style().set_property("height", "36px");
            let _ = auth_restore_button
                .style()
                .set_property("padding", "0 12px");
            let _ = auth_restore_button
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_restore_button
                .style()
                .set_property("border", "1px solid #1f2937");
            let _ = auth_restore_button
                .style()
                .set_property("background", "#111827");
            let _ = auth_restore_button.style().set_property("color", "#cbd5e1");
            let _ = auth_action_row.append_child(&auth_restore_button);

            let auth_logout_button = document
                .create_element("button")
                .map_err(|_| "failed to create auth logout button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "auth logout button is not HtmlElement".to_string())?;
            auth_logout_button.set_id(AUTH_LOGOUT_ID);
            auth_logout_button.set_inner_text("Sign out");
            let _ = auth_logout_button.style().set_property("height", "36px");
            let _ = auth_logout_button.style().set_property("padding", "0 12px");
            let _ = auth_logout_button
                .style()
                .set_property("border-radius", "10px");
            let _ = auth_logout_button
                .style()
                .set_property("border", "1px solid #7f1d1d");
            let _ = auth_logout_button
                .style()
                .set_property("background", "#7f1d1d");
            let _ = auth_logout_button.style().set_property("color", "#ffffff");
            let _ = auth_action_row.append_child(&auth_logout_button);
            let _ = auth_panel.append_child(&auth_action_row);

            let _ = root.append_child(&auth_panel);
            body.append_child(&root)
                .map_err(|_| "failed to append codex chat root".to_string())?;
        }

        let send_button = document
            .get_element_by_id(CODEX_CHAT_SEND_ID)
            .ok_or_else(|| "missing codex send button".to_string())?;
        let input = document
            .get_element_by_id(CODEX_CHAT_INPUT_ID)
            .ok_or_else(|| "missing codex input".to_string())?;

        CODEX_SEND_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                submit_codex_message_from_input();
            }));
            let _ = send_button
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        CODEX_INPUT_KEYDOWN_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::wrap(Box::new(
                move |event: web_sys::KeyboardEvent| {
                    if event.key() == "Enter" && !event.shift_key() {
                        event.prevent_default();
                        submit_codex_message_from_input();
                    }
                },
            ));
            let _ = input
                .add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        let auth_send_button = document
            .get_element_by_id(AUTH_SEND_ID)
            .ok_or_else(|| "missing auth send button".to_string())?;
        let auth_verify_button = document
            .get_element_by_id(AUTH_VERIFY_ID)
            .ok_or_else(|| "missing auth verify button".to_string())?;
        let auth_restore_button = document
            .get_element_by_id(AUTH_RESTORE_ID)
            .ok_or_else(|| "missing auth restore button".to_string())?;
        let auth_logout_button = document
            .get_element_by_id(AUTH_LOGOUT_ID)
            .ok_or_else(|| "missing auth logout button".to_string())?;

        AUTH_SEND_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                submit_auth_send_from_input();
            }));
            let _ = auth_send_button
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        AUTH_VERIFY_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                submit_auth_verify_from_input();
            }));
            let _ = auth_verify_button
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        AUTH_RESTORE_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                queue_intent(CommandIntent::RestoreSession);
            }));
            let _ = auth_restore_button
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        AUTH_LOGOUT_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                queue_intent(CommandIntent::LogoutSession);
            }));
            let _ = auth_logout_button
                .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        Ok(())
    }

    fn submit_codex_message_from_input() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(input) = document.get_element_by_id(CODEX_CHAT_INPUT_ID) else {
            return;
        };
        let Ok(input) = input.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let text = input.value();
        if text.trim().is_empty() {
            return;
        }
        input.set_value("");
        codex_send_message(text);
    }

    fn submit_auth_send_from_input() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(input) = document.get_element_by_id(AUTH_EMAIL_INPUT_ID) else {
            return;
        };
        let Ok(input) = input.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let email = input.value();
        if email.trim().is_empty() {
            return;
        }
        queue_intent(CommandIntent::StartAuthChallenge { email });
    }

    fn submit_auth_verify_from_input() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(input) = document.get_element_by_id(AUTH_CODE_INPUT_ID) else {
            return;
        };
        let Ok(input) = input.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let code = input.value();
        if code.trim().is_empty() {
            return;
        }
        input.set_value("");
        queue_intent(CommandIntent::VerifyAuthCode { code });
    }

    fn render_codex_chat_dom() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(root) = document.get_element_by_id(CODEX_CHAT_ROOT_ID) else {
            return;
        };
        let Ok(root) = root.dyn_into::<HtmlElement>() else {
            return;
        };

        let (route, auth_state) = APP_STATE.with(|state| {
            let state = state.borrow();
            (state.route.clone(), state.auth.clone())
        });
        let management_state = MANAGEMENT_SURFACE_STATE.with(|state| state.borrow().clone());
        let management_loading = MANAGEMENT_SURFACE_LOADING.with(Cell::get);

        let thread_id = thread_id_from_route(&route);
        let is_management_route = route_is_management_surface(&route);
        let is_auth_route = route_is_auth_surface(&route);
        if thread_id.is_none() && !is_management_route && !is_auth_route {
            let _ = root.style().set_property("display", "none");
            return;
        }

        if let Some(header) = document.get_element_by_id(CODEX_CHAT_HEADER_ID) {
            if let Ok(header) = header.dyn_into::<HtmlElement>() {
                header.set_inner_text(&route_header_title(&route));
            }
        }

        let Some(messages_container) = document.get_element_by_id(CODEX_CHAT_MESSAGES_ID) else {
            return;
        };
        let Ok(messages_container) = messages_container.dyn_into::<HtmlElement>() else {
            return;
        };
        messages_container.set_inner_html("");

        let Some(composer) = document
            .get_element_by_id(CODEX_CHAT_COMPOSER_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        else {
            return;
        };
        let auth_panel = document
            .get_element_by_id(AUTH_PANEL_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());

        if let Some(thread_id) = thread_id {
            let _ = root.style().set_property("display", "flex");
            let _ = composer.style().set_property("display", "flex");
            if let Some(auth_panel) = auth_panel.as_ref() {
                let _ = auth_panel.style().set_property("display", "none");
            }
            render_codex_thread_messages(&document, &messages_container);
            messages_container.set_scroll_top(messages_container.scroll_height());
            if thread_id.is_empty() {
                let _ = messages_container.style().set_property("opacity", "0.9");
            } else {
                let _ = messages_container.style().set_property("opacity", "1");
            }
            return;
        }

        let _ = root.style().set_property("display", "flex");
        let _ = composer.style().set_property("display", "none");
        if let Some(auth_panel) = auth_panel.as_ref() {
            let _ = auth_panel
                .style()
                .set_property("display", if is_auth_route { "flex" } else { "none" });
        }

        if is_auth_route {
            sync_auth_form_inputs(&document, &auth_state);
            render_auth_surface_messages(&document, &messages_container, &route, &auth_state);
            messages_container.set_scroll_top(0);
            return;
        }

        render_management_surface_messages(
            &document,
            &messages_container,
            &route,
            &auth_state,
            &management_state,
            management_loading,
        );
        messages_container.set_scroll_top(0);
    }

    fn sync_auth_form_inputs(
        document: &web_sys::Document,
        auth_state: &openagents_app_state::AuthState,
    ) {
        if let Some(email_input) = document
            .get_element_by_id(AUTH_EMAIL_INPUT_ID)
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        {
            if let Some(email) = auth_state.email.as_ref() {
                email_input.set_value(email);
            }
        }
    }

    fn render_codex_thread_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
    ) {
        CODEX_THREAD_STATE.with(|state| {
            for message in &state.borrow().messages {
                let Ok(row) = document.create_element("div") else {
                    continue;
                };
                let Ok(row) = row.dyn_into::<HtmlElement>() else {
                    continue;
                };
                let _ = row.style().set_property("display", "flex");
                let _ = row.style().set_property("width", "100%");
                let align = match message.role {
                    crate::codex_thread::CodexMessageRole::User => "flex-end",
                    crate::codex_thread::CodexMessageRole::Assistant
                    | crate::codex_thread::CodexMessageRole::Reasoning
                    | crate::codex_thread::CodexMessageRole::System => "flex-start",
                };
                let _ = row.style().set_property("justify-content", align);

                let Ok(bubble) = document.create_element("div") else {
                    continue;
                };
                let Ok(bubble) = bubble.dyn_into::<HtmlElement>() else {
                    continue;
                };
                let _ = bubble.style().set_property("max-width", "78%");
                let _ = bubble.style().set_property("padding", "10px 12px");
                let _ = bubble.style().set_property("border-radius", "12px");
                let _ = bubble.style().set_property("white-space", "pre-wrap");
                let _ = bubble.style().set_property("line-height", "1.4");
                let _ = bubble.style().set_property("font-size", "15px");
                match message.role {
                    crate::codex_thread::CodexMessageRole::User => {
                        let _ = bubble.style().set_property("background", "#2563eb");
                        let _ = bubble.style().set_property("color", "#ffffff");
                    }
                    crate::codex_thread::CodexMessageRole::Assistant => {
                        let _ = bubble.style().set_property("background", "#111827");
                        let _ = bubble.style().set_property("color", "#e5e7eb");
                    }
                    crate::codex_thread::CodexMessageRole::Reasoning => {
                        let _ = bubble.style().set_property("background", "#1f2937");
                        let _ = bubble.style().set_property("color", "#d1d5db");
                    }
                    crate::codex_thread::CodexMessageRole::System => {
                        let _ = bubble.style().set_property("background", "#0f172a");
                        let _ = bubble.style().set_property("color", "#94a3b8");
                    }
                }
                bubble.set_inner_text(&if message.streaming {
                    format!("{}\nstreaming", message.text)
                } else {
                    message.text.clone()
                });
                let _ = row.append_child(&bubble);
                let _ = messages_container.append_child(&row);
            }
        });
    }

    fn render_management_surface_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
        auth_state: &openagents_app_state::AuthState,
        management_state: &ManagementSurfaceState,
        loading: bool,
    ) {
        let cards = management_surface_cards(route, auth_state, management_state, loading);
        for card in cards {
            append_management_card(document, messages_container, card);
        }
    }

    fn render_auth_surface_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
        auth_state: &openagents_app_state::AuthState,
    ) {
        let mut cards = vec![ManagementCard {
            title: "Route".to_string(),
            body: route.to_path(),
            tone: ManagementCardTone::Info,
        }];

        cards.push(ManagementCard {
            title: "Auth Status".to_string(),
            body: format!("{:?}", auth_state.status),
            tone: if auth_state.has_active_session() {
                ManagementCardTone::Success
            } else {
                ManagementCardTone::Warning
            },
        });

        if let Some(email) = auth_state.email.as_ref() {
            cards.push(ManagementCard {
                title: "Email".to_string(),
                body: email.clone(),
                tone: ManagementCardTone::Neutral,
            });
        }

        if let Some(challenge_id) = auth_state.challenge_id.as_ref() {
            cards.push(ManagementCard {
                title: "Challenge".to_string(),
                body: challenge_id.clone(),
                tone: ManagementCardTone::Info,
            });
        }

        let guidance = match route {
            AppRoute::Login | AppRoute::Register | AppRoute::Authenticate => {
                "Use Send code -> Verify. Restore session checks existing auth; Sign out revokes current session."
                    .to_string()
            }
            AppRoute::Onboarding { section } => format!(
                "Onboarding section: {}",
                section.clone().unwrap_or_else(|| "start".to_string())
            ),
            _ => "Authentication route".to_string(),
        };
        cards.push(ManagementCard {
            title: "Flow".to_string(),
            body: guidance,
            tone: ManagementCardTone::Neutral,
        });

        for card in cards {
            append_management_card(document, messages_container, card);
        }
    }

    #[derive(Clone, Copy)]
    enum ManagementCardTone {
        Neutral,
        Info,
        Success,
        Warning,
        Error,
    }

    struct ManagementCard {
        title: String,
        body: String,
        tone: ManagementCardTone,
    }

    fn management_surface_cards(
        route: &AppRoute,
        auth_state: &openagents_app_state::AuthState,
        management_state: &ManagementSurfaceState,
        loading: bool,
    ) -> Vec<ManagementCard> {
        let mut cards = vec![ManagementCard {
            title: "Route".to_string(),
            body: route.to_path(),
            tone: ManagementCardTone::Info,
        }];

        let auth_tone = if auth_state.has_active_session() {
            ManagementCardTone::Success
        } else {
            ManagementCardTone::Warning
        };
        cards.push(ManagementCard {
            title: "Auth".to_string(),
            body: format!("{:?}", auth_state.status),
            tone: auth_tone,
        });

        if let Some(user) = &auth_state.user {
            cards.push(ManagementCard {
                title: "User".to_string(),
                body: format!("{} <{}>", user.name, user.email),
                tone: ManagementCardTone::Neutral,
            });
        }

        if let Some(session) = &auth_state.session {
            cards.push(ManagementCard {
                title: "Session".to_string(),
                body: format!(
                    "id: {}\norg: {}\ndevice: {}\nstatus: {:?}",
                    session.session_id, session.active_org_id, session.device_id, session.status
                ),
                tone: ManagementCardTone::Neutral,
            });
        }

        if loading {
            cards.push(ManagementCard {
                title: "Data".to_string(),
                body: "Loading account/admin context from control service.".to_string(),
                tone: ManagementCardTone::Info,
            });
        } else if let Some(error) = management_state.last_error.as_ref() {
            cards.push(ManagementCard {
                title: "Data Load Error".to_string(),
                body: error.clone(),
                tone: ManagementCardTone::Error,
            });
        } else if !management_state.memberships.is_empty() {
            let memberships = management_state
                .memberships
                .iter()
                .map(|membership| {
                    let default_tag = if membership.default_org {
                        " [default]"
                    } else {
                        ""
                    };
                    format!(
                        "{} ({}) role={}{} scopes={}",
                        membership.org_slug,
                        membership.org_id,
                        membership.role,
                        default_tag,
                        membership.role_scopes.join("|")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            cards.push(ManagementCard {
                title: "Org Memberships".to_string(),
                body: memberships,
                tone: ManagementCardTone::Neutral,
            });
        }

        if let AppRoute::Admin { .. } = route {
            if has_admin_access(auth_state, management_state) {
                if let Some(route_split_status) = management_state.route_split_status.as_ref() {
                    cards.push(ManagementCard {
                        title: "Route Split".to_string(),
                        body: format!(
                            "enabled: {}\nmode: {}\ncohort: {}%\nroutes: {}\noverride: {}\nforce_legacy: {}\nlegacy_base: {}",
                            route_split_status.enabled,
                            route_split_status.mode,
                            route_split_status.cohort_percentage,
                            route_split_status.rust_routes.join(","),
                            route_split_status
                                .override_target
                                .clone()
                                .unwrap_or_else(|| "none".to_string()),
                            route_split_status.force_legacy,
                            route_split_status
                                .legacy_base_url
                                .clone()
                                .unwrap_or_else(|| "none".to_string()),
                        ),
                        tone: ManagementCardTone::Info,
                    });
                }
            } else {
                cards.push(ManagementCard {
                    title: "Admin Guard".to_string(),
                    body: "Access denied for /admin route. Owner/Admin membership on the active org is required."
                        .to_string(),
                    tone: ManagementCardTone::Error,
                });
            }
        }

        if let AppRoute::Billing { section } = route {
            let section_label = section.clone().unwrap_or_else(|| "wallet".to_string());
            cards.push(ManagementCard {
                title: "Billing Surface".to_string(),
                body: format!("section: {section_label}\noperator route ownership: rust shell"),
                tone: ManagementCardTone::Info,
            });

            if let Some(policy) = management_state.billing_policy.as_ref() {
                let tone = if policy.allowed {
                    ManagementCardTone::Success
                } else {
                    ManagementCardTone::Error
                };
                let denied = if policy.denied_reasons.is_empty() {
                    "none".to_string()
                } else {
                    policy.denied_reasons.join("|")
                };
                cards.push(ManagementCard {
                    title: "Policy Decision".to_string(),
                    body: format!(
                        "allowed: {}\nresolved_org_id: {}\ngranted_scopes: {}\ndenied_reasons: {}",
                        policy.allowed,
                        policy.resolved_org_id,
                        policy.granted_scopes.join(","),
                        denied
                    ),
                    tone,
                });
            } else {
                cards.push(ManagementCard {
                    title: "Policy Decision".to_string(),
                    body: "Policy decision is unavailable for this billing route.".to_string(),
                    tone: ManagementCardTone::Warning,
                });
            }

            if matches!(section.as_deref(), Some("paywalls"))
                && !has_admin_access(auth_state, management_state)
            {
                cards.push(ManagementCard {
                    title: "Operator Guard".to_string(),
                    body: "Paywall operator section requires owner/admin membership on active org."
                        .to_string(),
                    tone: ManagementCardTone::Error,
                });
            }
        }

        cards
    }

    fn has_admin_access(
        auth_state: &openagents_app_state::AuthState,
        management_state: &ManagementSurfaceState,
    ) -> bool {
        let Some(active_org_id) = auth_state
            .session
            .as_ref()
            .map(|session| session.active_org_id.as_str())
            .or_else(|| management_state.active_org_id.as_deref())
        else {
            return false;
        };

        management_state.memberships.iter().any(|membership| {
            membership.org_id == active_org_id
                && matches!(membership.role.as_str(), "owner" | "admin")
        })
    }

    fn route_header_title(route: &AppRoute) -> String {
        match route {
            AppRoute::Chat {
                thread_id: Some(thread_id),
            } => format!("Codex Thread {thread_id}"),
            AppRoute::Chat { thread_id: None } => "Codex Thread".to_string(),
            AppRoute::Login => "Sign In".to_string(),
            AppRoute::Register => "Register".to_string(),
            AppRoute::Authenticate => "Authenticate".to_string(),
            AppRoute::Onboarding { section } => match section {
                Some(section) => format!("Onboarding / {section}"),
                None => "Onboarding".to_string(),
            },
            AppRoute::Account { section } => match section {
                Some(section) => format!("Account / {section}"),
                None => "Account".to_string(),
            },
            AppRoute::Settings { section } => match section {
                Some(section) => format!("Settings / {section}"),
                None => "Settings".to_string(),
            },
            AppRoute::Billing { section } => match section {
                Some(section) => format!("Billing / {section}"),
                None => "Billing".to_string(),
            },
            AppRoute::Admin { section } => match section {
                Some(section) => format!("Admin / {section}"),
                None => "Admin".to_string(),
            },
            AppRoute::Workers => "Workers".to_string(),
            AppRoute::Debug => "Debug".to_string(),
            AppRoute::Home => "OpenAgents".to_string(),
        }
    }

    fn append_management_card(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        card: ManagementCard,
    ) {
        let Ok(row) = document.create_element("div") else {
            return;
        };
        let Ok(row) = row.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = row.style().set_property("display", "flex");
        let _ = row.style().set_property("width", "100%");
        let _ = row.style().set_property("justify-content", "flex-start");

        let Ok(bubble) = document.create_element("div") else {
            return;
        };
        let Ok(bubble) = bubble.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = bubble.style().set_property("max-width", "92%");
        let _ = bubble.style().set_property("padding", "10px 12px");
        let _ = bubble.style().set_property("border-radius", "12px");
        let _ = bubble.style().set_property("white-space", "pre-wrap");
        let _ = bubble.style().set_property("line-height", "1.4");
        let _ = bubble.style().set_property("font-size", "14px");
        let _ = bubble.style().set_property(
            "font-family",
            "ui-monospace, SFMono-Regular, Menlo, monospace",
        );

        let (bg, fg) = match card.tone {
            ManagementCardTone::Neutral => ("#111827", "#e5e7eb"),
            ManagementCardTone::Info => ("#0f172a", "#bfdbfe"),
            ManagementCardTone::Success => ("#052e16", "#bbf7d0"),
            ManagementCardTone::Warning => ("#3f2f0a", "#fde68a"),
            ManagementCardTone::Error => ("#3f1d1d", "#fecaca"),
        };
        let _ = bubble.style().set_property("background", bg);
        let _ = bubble.style().set_property("color", fg);
        bubble.set_inner_text(&format!("{}\n{}", card.title, card.body));

        let _ = row.append_child(&bubble);
        let _ = messages_container.append_child(&row);
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

    fn record_boot_milestone(metric_name: &str, actual_ms: u64, budget_ms: u64) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            match metric_name {
                "dom_ready_latency_ms" => state.dom_ready_latency_ms = Some(actual_ms),
                "gpu_init_latency_ms" => state.gpu_init_latency_ms = Some(actual_ms),
                _ => {}
            }
            if actual_ms > budget_ms {
                state
                    .budget_breaches
                    .push(format!("{metric_name}>{budget_ms} (actual={actual_ms})"));
            }
        });
    }

    fn detect_gpu_mode_hint() -> Option<String> {
        let window = web_sys::window()?;
        let value = js_sys::Reflect::get(&window, &JsValue::from_str("__OA_GPU_MODE__")).ok()?;
        let mode = value.as_string()?.trim().to_ascii_lowercase();
        if mode.is_empty() { None } else { Some(mode) }
    }

    fn now_unix_ms() -> u64 {
        let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return 0;
        };
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
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
