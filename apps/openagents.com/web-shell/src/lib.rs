#![allow(clippy::needless_pass_by_value)]

#[cfg(any(target_arch = "wasm32", test))]
mod codex_thread;
#[cfg(target_arch = "wasm32")]
mod wasm_constants;
#[cfg(target_arch = "wasm32")]
mod wasm_state;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use web_time::{Duration, Instant};

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
    use web_sys::{
        HtmlAnchorElement, HtmlCanvasElement, HtmlElement, HtmlInputElement, MouseEvent,
    };
    use wgpui::{Platform, Scene, WebPlatform, run_animation_loop, setup_resize_observer};

    use crate::codex_thread::{CodexMessageRole, CodexThreadMessage, CodexThreadState};
    use crate::wasm_constants::*;
    use crate::wasm_state::{
        AdminWorkerSurfaceState, CodexHistoryState, L402SurfaceState, ManagementSurfaceState,
        SettingsSurfaceState, SyncRuntimeState,
    };

    mod dom;
    mod lifecycle;
    mod network;
    mod routing;

    use dom::ensure_codex_chat_dom;
    use lifecycle::*;
    use network::*;
    use routing::*;

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
        static CODEX_HISTORY_STATE: RefCell<CodexHistoryState> = RefCell::new(CodexHistoryState::default());
        static CODEX_HISTORY_LOADING: Cell<bool> = const { Cell::new(false) };
        static MANAGEMENT_SURFACE_STATE: RefCell<ManagementSurfaceState> = RefCell::new(ManagementSurfaceState::default());
        static MANAGEMENT_SURFACE_LOADING: Cell<bool> = const { Cell::new(false) };
        static CODEX_SEND_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static CODEX_INPUT_KEYDOWN_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>> = const { RefCell::new(None) };
        static AUTH_SEND_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_VERIFY_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_RESTORE_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static AUTH_LOGOUT_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static CODEX_QUICK_PROMPT_CLICK_HANDLERS: RefCell<Vec<Closure<dyn FnMut(web_sys::Event)>>> = RefCell::new(Vec::new());
        static SETTINGS_SURFACE_STATE: RefCell<SettingsSurfaceState> = RefCell::new(SettingsSurfaceState::default());
        static SETTINGS_SURFACE_LOADING: Cell<bool> = const { Cell::new(false) };
        static SETTINGS_PROFILE_SAVE_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_PROFILE_DELETE_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_AUTOPILOT_SAVE_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_RESEND_CONNECT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_RESEND_DISCONNECT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_RESEND_TEST_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_GOOGLE_CONNECT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static SETTINGS_GOOGLE_DISCONNECT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static L402_SURFACE_STATE: RefCell<L402SurfaceState> = RefCell::new(L402SurfaceState::default());
        static L402_SURFACE_LOADING: Cell<bool> = const { Cell::new(false) };
        static ADMIN_WORKER_SURFACE_STATE: RefCell<AdminWorkerSurfaceState> = RefCell::new(AdminWorkerSurfaceState::default());
        static ADMIN_WORKER_SURFACE_LOADING: Cell<bool> = const { Cell::new(false) };
        static ADMIN_WORKER_CREATE_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ADMIN_WORKER_REFRESH_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ADMIN_WORKER_STOP_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ADMIN_WORKER_REQUEST_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ADMIN_WORKER_EVENT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ADMIN_WORKER_STREAM_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static GLOBAL_SHORTCUT_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>> = const { RefCell::new(None) };
        static ROUTE_SCROLL_POSITIONS: RefCell<HashMap<String, i32>> = RefCell::new(HashMap::new());
        static ROUTE_POPSTATE_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
        static ROUTE_LINK_CLICK_HANDLER: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = const { RefCell::new(None) };
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RuntimeThreadsResponse {
        data: RuntimeThreadsPayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RuntimeThreadsPayload {
        threads: Vec<RuntimeThreadRecord>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub(super) struct RuntimeThreadRecord {
        thread_id: String,
        #[serde(default)]
        message_count: u32,
        #[serde(default)]
        updated_at: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RuntimeThreadMessagesResponse {
        data: RuntimeThreadMessagesPayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RuntimeThreadMessagesPayload {
        #[serde(default)]
        messages: Vec<RuntimeThreadMessageRecord>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct RuntimeThreadMessageRecord {
        message_id: String,
        role: String,
        text: String,
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
    pub(super) struct MembershipRecord {
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
    pub(super) struct RouteSplitStatus {
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
    pub(super) struct PolicyDecision {
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

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsProfileEnvelope {
        data: SettingsProfilePayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsProfilePayload {
        id: String,
        name: String,
        email: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct AutopilotListEnvelope {
        data: Vec<AutopilotPayload>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct AutopilotPayload {
        #[serde(default, rename = "displayName")]
        display_name: String,
        #[serde(default)]
        tagline: Option<String>,
        profile: AutopilotProfilePayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct AutopilotProfilePayload {
        #[serde(default, rename = "ownerDisplayName")]
        owner_display_name: String,
        #[serde(default, rename = "personaSummary")]
        persona_summary: Option<String>,
        #[serde(default, rename = "autopilotVoice")]
        autopilot_voice: Option<String>,
        #[serde(default)]
        principles: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsAutopilotUpdateEnvelope {
        data: SettingsAutopilotUpdatePayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsAutopilotUpdatePayload {
        status: String,
        autopilot: AutopilotPayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsIntegrationEnvelope {
        data: SettingsIntegrationPayloadEnvelope,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsIntegrationPayloadEnvelope {
        status: String,
        action: Option<String>,
        integration: SettingsIntegrationPayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsIntegrationPayload {
        connected: bool,
        #[serde(default, rename = "secretLast4")]
        secret_last4: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsDeleteProfileEnvelope {
        data: SettingsDeleteProfilePayload,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct SettingsDeleteProfilePayload {
        deleted: bool,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct JsonDataEnvelope {
        data: serde_json::Value,
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
    pub fn navigate(path: String) {
        let route = AppRoute::from_path(&path);
        queue_intent(CommandIntent::Navigate { route });
    }

    #[wasm_bindgen]
    pub fn codex_send_message(text: String) {
        if let Some(thread_id) = active_thread_id() {
            CODEX_THREAD_STATE.with(|state| {
                state.borrow_mut().append_local_user_message(&text);
            });
            render_codex_chat_dom();
            queue_intent(CommandIntent::SendThreadMessage { thread_id, text });
            return;
        }

        start_codex_thread_with_prompt(text);
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
        install_browser_navigation_handlers();

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
        APP_STATE.with(|state| replace_route_in_browser_history(&state.borrow().route));
        schedule_management_surface_refresh();
        schedule_settings_surface_refresh();
        schedule_l402_surface_refresh();
        schedule_admin_worker_surface_refresh();
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
                apply_route_transition(route, true);
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
                    reset_settings_surface_state();
                    reset_l402_surface_state();
                    reset_admin_worker_surface_state();
                    reset_codex_history_state();
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
            reset_settings_surface_state();
            reset_l402_surface_state();
            reset_admin_worker_surface_state();
            reset_codex_history_state();
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
                            reset_settings_surface_state();
                            reset_l402_surface_state();
                            reset_admin_worker_surface_state();
                            reset_codex_history_state();
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
        reset_settings_surface_state();
        reset_l402_surface_state();
        reset_admin_worker_surface_state();
        reset_codex_history_state();
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
        let response = send_legacy_chat_stream_request(&thread_id, &normalized, &state).await;
        if let Err(error) = &response {
            CODEX_THREAD_STATE.with(|chat| {
                chat.borrow_mut()
                    .append_local_system_message(&format!("Send failed: {}", error.message));
            });
            render_codex_chat_dom();
        }
        response.map(|wire| {
            CODEX_THREAD_STATE.with(|chat| {
                chat.borrow_mut().ingest_vercel_sse_wire(&wire);
            });
            render_codex_chat_dom();
            invalidate_codex_history_cache();
            schedule_codex_history_refresh();
        })
    }

    async fn send_legacy_chat_stream_request(
        thread_id: &str,
        text: &str,
        state: &AppState,
    ) -> Result<String, ControlApiError> {
        let Some(access_token) = state.auth.access_token.as_ref() else {
            return Err(ControlApiError::from_command_error(
                CommandError::missing_credential(
                    "Access token is required to send thread message.",
                ),
            ));
        };

        let encoded_thread_id = encode_path_component(thread_id);
        let path = format!("/api/chats/{encoded_thread_id}/stream");
        let body = serde_json::to_string(&serde_json::json!({
            "messages": [
                {
                    "id": format!("web-msg-{}", now_unix_ms()),
                    "role": "user",
                    "content": text,
                }
            ]
        }))
        .map_err(|error| ControlApiError {
            status_code: 500,
            code: Some("request_body_serialize_failed".to_string()),
            message: format!("failed to serialize request body: {error}"),
            kind: CommandErrorKind::Decode,
            retryable: false,
        })?;

        let authorization = format!("Bearer {access_token}");
        let response = Request::post(&path)
            .header("content-type", "application/json")
            .header("authorization", &authorization)
            .header("x-oa-client-build-id", WEB_SHELL_COMPAT_CLIENT_BUILD_ID)
            .header("x-oa-protocol-version", WEB_SHELL_COMPAT_PROTOCOL_VERSION)
            .header("x-oa-schema-version", WEB_SHELL_COMPAT_SCHEMA_VERSION)
            .body(body)
            .map_err(|error| ControlApiError {
                status_code: 500,
                code: Some("request_build_failed".to_string()),
                message: format!("failed to build request body: {error}"),
                kind: CommandErrorKind::Unknown,
                retryable: false,
            })?
            .send()
            .await
            .map_err(map_network_error)?;

        decode_sse_response(response).await
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
        schedule_codex_history_refresh();
        schedule_management_surface_refresh();
        schedule_settings_surface_refresh();
        schedule_l402_surface_refresh();
        schedule_admin_worker_surface_refresh();
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

    fn start_codex_thread_with_prompt(prompt: String) {
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }

        let has_session = APP_STATE.with(|state| state.borrow().auth.has_active_session());
        if !has_session {
            CODEX_THREAD_STATE.with(|state| {
                state
                    .borrow_mut()
                    .append_local_system_message("Sign in first to use Codex chat.");
            });
            render_codex_chat_dom();
            return;
        }

        let thread_id = generate_codex_thread_id();
        apply_route_transition(
            AppRoute::Chat {
                thread_id: Some(thread_id.clone()),
            },
            true,
        );
        CODEX_THREAD_STATE.with(|state| {
            state.borrow_mut().append_local_user_message(&prompt);
        });
        render_codex_chat_dom();
        queue_intent(CommandIntent::SendThreadMessage {
            thread_id,
            text: prompt,
        });
    }

    fn generate_codex_thread_id() -> String {
        let timestamp = now_unix_ms();
        let random = (js_sys::Math::random() * 1_000_000.0).floor() as u64;
        format!("thread_web_{timestamp}_{random:06}")
    }

    fn reset_management_surface_state() {
        MANAGEMENT_SURFACE_LOADING.with(|loading| loading.set(false));
        MANAGEMENT_SURFACE_STATE.with(|state| {
            *state.borrow_mut() = ManagementSurfaceState::default();
        });
    }

    fn reset_settings_surface_state() {
        SETTINGS_SURFACE_LOADING.with(|loading| loading.set(false));
        SETTINGS_SURFACE_STATE.with(|state| {
            *state.borrow_mut() = SettingsSurfaceState::default();
        });
    }

    fn reset_l402_surface_state() {
        L402_SURFACE_LOADING.with(|loading| loading.set(false));
        L402_SURFACE_STATE.with(|state| {
            *state.borrow_mut() = L402SurfaceState::default();
        });
    }

    fn reset_admin_worker_surface_state() {
        ADMIN_WORKER_SURFACE_LOADING.with(|loading| loading.set(false));
        ADMIN_WORKER_SURFACE_STATE.with(|state| {
            *state.borrow_mut() = AdminWorkerSurfaceState::default();
        });
    }

    fn reset_codex_history_state() {
        CODEX_HISTORY_LOADING.with(|loading| loading.set(false));
        CODEX_HISTORY_STATE.with(|state| {
            *state.borrow_mut() = CodexHistoryState::default();
        });
    }

    fn invalidate_codex_history_cache() {
        CODEX_HISTORY_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.loaded_session_id = None;
            state.loaded_thread_id = None;
        });
    }

    fn schedule_codex_history_refresh() {
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
            reset_codex_history_state();
            return;
        };
        let Some(session_id) = session_id else {
            reset_codex_history_state();
            return;
        };
        if access_token.trim().is_empty() {
            reset_codex_history_state();
            return;
        }
        if !route_is_codex_chat_surface(&route) {
            return;
        }

        let route_thread_id = thread_id_from_route(&route);
        let already_loaded = CODEX_HISTORY_STATE.with(|state| {
            let state = state.borrow();
            state.loaded_session_id.as_deref() == Some(session_id.as_str())
                && state.loaded_thread_id == route_thread_id
                && state.last_error.is_none()
        });
        if already_loaded {
            return;
        }

        let already_loading = CODEX_HISTORY_LOADING.with(|loading| {
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
            let result =
                fetch_codex_history_state(&access_token, &session_id, route_thread_id.clone())
                    .await;
            CODEX_HISTORY_LOADING.with(|loading| loading.set(false));

            let still_current = APP_STATE.with(|state| {
                let state = state.borrow();
                state
                    .auth
                    .session
                    .as_ref()
                    .map(|session| session.session_id.as_str())
                    == Some(session_id.as_str())
                    && thread_id_from_route(&state.route) == route_thread_id
            });
            if !still_current {
                schedule_codex_history_refresh();
                return;
            }

            match result {
                Ok((snapshot, active_thread_history)) => {
                    CODEX_HISTORY_STATE.with(|state| {
                        *state.borrow_mut() = snapshot;
                    });

                    if let Some((thread_id, messages)) = active_thread_history {
                        CODEX_THREAD_STATE.with(|chat| {
                            chat.borrow_mut()
                                .hydrate_history_if_empty(Some(thread_id), messages);
                        });
                    }
                }
                Err(error) => {
                    CODEX_HISTORY_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.loaded_session_id = Some(session_id.clone());
                        state.loaded_thread_id = route_thread_id.clone();
                        state.last_error = Some(error.message.clone());
                    });
                }
            }

            render_codex_chat_dom();
        });
    }

    async fn fetch_codex_history_state(
        access_token: &str,
        session_id: &str,
        route_thread_id: Option<String>,
    ) -> Result<(CodexHistoryState, Option<(String, Vec<CodexThreadMessage>)>), ControlApiError>
    {
        let threads = fetch_runtime_threads(access_token).await?;

        let mut snapshot = CodexHistoryState {
            loaded_session_id: Some(session_id.to_string()),
            loaded_thread_id: route_thread_id.clone(),
            threads,
            active_thread_exists: None,
            last_error: None,
        };

        let active_thread_history = if let Some(thread_id) = route_thread_id {
            let (messages, exists) =
                fetch_runtime_thread_messages(access_token, &thread_id).await?;
            snapshot.active_thread_exists = Some(exists);
            Some((thread_id, messages))
        } else {
            None
        };

        Ok((snapshot, active_thread_history))
    }

    async fn fetch_runtime_threads(
        access_token: &str,
    ) -> Result<Vec<RuntimeThreadRecord>, ControlApiError> {
        let request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/runtime/threads".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let response: RuntimeThreadsResponse =
            send_json_request(&request, &AppState::default()).await?;
        Ok(response.data.threads)
    }

    async fn fetch_runtime_thread_messages(
        access_token: &str,
        thread_id: &str,
    ) -> Result<(Vec<CodexThreadMessage>, bool), ControlApiError> {
        let request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: format!("/api/runtime/threads/{thread_id}/messages"),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };

        let response: RuntimeThreadMessagesResponse =
            match send_json_request(&request, &AppState::default()).await {
                Ok(response) => response,
                Err(error) if error.status_code == 404 => return Ok((Vec::new(), false)),
                Err(error) => return Err(error),
            };

        let messages = response
            .data
            .messages
            .into_iter()
            .map(|message| CodexThreadMessage {
                id: message.message_id,
                role: map_runtime_message_role(&message.role),
                text: message.text,
                streaming: false,
            })
            .collect::<Vec<_>>();
        Ok((messages, true))
    }

    fn map_runtime_message_role(role: &str) -> CodexMessageRole {
        match role.trim().to_ascii_lowercase().as_str() {
            "user" => CodexMessageRole::User,
            "assistant" => CodexMessageRole::Assistant,
            "reasoning" => CodexMessageRole::Reasoning,
            _ => CodexMessageRole::System,
        }
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

    fn schedule_settings_surface_refresh() {
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

        if !route_is_settings_surface(&route) {
            return;
        }

        let Some(access_token) = access_token else {
            reset_settings_surface_state();
            return;
        };
        let Some(session_id) = session_id else {
            reset_settings_surface_state();
            return;
        };
        if access_token.trim().is_empty() {
            reset_settings_surface_state();
            return;
        }

        let already_loaded = SETTINGS_SURFACE_STATE.with(|state| {
            let state = state.borrow();
            state.loaded_session_id.as_deref() == Some(session_id.as_str())
                && state.last_error.is_none()
        });
        if already_loaded {
            return;
        }

        let already_loading = SETTINGS_SURFACE_LOADING.with(|loading| {
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
            let result = fetch_settings_surface_state(&access_token, &session_id).await;
            SETTINGS_SURFACE_LOADING.with(|loading| loading.set(false));
            SETTINGS_SURFACE_STATE.with(|state| {
                let mut state = state.borrow_mut();
                match result {
                    Ok(snapshot) => {
                        *state = snapshot;
                    }
                    Err(error) => {
                        state.loaded_session_id = Some(session_id.clone());
                        state.last_error = Some(error.message.clone());
                        state.last_status = None;
                    }
                }
            });
            render_codex_chat_dom();
        });
    }

    async fn fetch_settings_surface_state(
        access_token: &str,
        session_id: &str,
    ) -> Result<SettingsSurfaceState, ControlApiError> {
        let profile_request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/settings/profile".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let profile: SettingsProfileEnvelope =
            send_json_request(&profile_request, &AppState::default()).await?;

        let autopilot_request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/autopilots?limit=1".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let autopilots: AutopilotListEnvelope =
            send_json_request(&autopilot_request, &AppState::default()).await?;

        let mut state = SettingsSurfaceState {
            loaded_session_id: Some(session_id.to_string()),
            profile_id: Some(profile.data.id),
            profile_name: profile.data.name,
            profile_email: profile.data.email,
            ..SettingsSurfaceState::default()
        };
        apply_autopilot_payload_to_settings_state(&mut state, autopilots.data.first());
        Ok(state)
    }

    fn schedule_l402_surface_refresh() {
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

        if !route_is_l402_surface(&route) {
            return;
        }

        let route_path = route.to_path();
        let Some(access_token) = access_token else {
            reset_l402_surface_state();
            return;
        };
        let Some(session_id) = session_id else {
            reset_l402_surface_state();
            return;
        };
        if access_token.trim().is_empty() {
            reset_l402_surface_state();
            return;
        }

        let already_loaded = L402_SURFACE_STATE.with(|state| {
            let state = state.borrow();
            state.loaded_session_id.as_deref() == Some(session_id.as_str())
                && state.loaded_route_path.as_deref() == Some(route_path.as_str())
                && state.last_error.is_none()
        });
        if already_loaded {
            return;
        }

        let already_loading = L402_SURFACE_LOADING.with(|loading| {
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
            let result =
                fetch_l402_surface_state(&access_token, &session_id, &route, &route_path).await;
            L402_SURFACE_LOADING.with(|loading| loading.set(false));
            L402_SURFACE_STATE.with(|state| {
                let mut state = state.borrow_mut();
                match result {
                    Ok(snapshot) => {
                        *state = snapshot;
                    }
                    Err(error) => {
                        state.loaded_session_id = Some(session_id.clone());
                        state.loaded_route_path = Some(route_path.clone());
                        state.payload = None;
                        state.last_error = Some(error.message);
                    }
                }
            });
            render_codex_chat_dom();
        });
    }

    async fn fetch_l402_surface_state(
        access_token: &str,
        session_id: &str,
        route: &AppRoute,
        route_path: &str,
    ) -> Result<L402SurfaceState, ControlApiError> {
        let request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: l402_api_path_for_route(route),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let response: JsonDataEnvelope = send_json_request(&request, &AppState::default()).await?;
        Ok(L402SurfaceState {
            loaded_session_id: Some(session_id.to_string()),
            loaded_route_path: Some(route_path.to_string()),
            payload: Some(response.data),
            last_error: None,
        })
    }

    fn l402_api_path_for_route(route: &AppRoute) -> String {
        let AppRoute::Billing { section } = route else {
            return "/api/l402/wallet".to_string();
        };

        match section.as_deref() {
            None | Some("") | Some("wallet") => "/api/l402/wallet".to_string(),
            Some("transactions") => "/api/l402/transactions?per_page=50&page=1".to_string(),
            Some(path) if path.starts_with("transactions/") => {
                let event_id = path.trim_start_matches("transactions/").trim();
                if event_id.is_empty() {
                    "/api/l402/transactions?per_page=50&page=1".to_string()
                } else {
                    format!("/api/l402/transactions/{event_id}")
                }
            }
            Some("paywalls") => "/api/l402/paywalls".to_string(),
            Some("settlements") => "/api/l402/settlements".to_string(),
            Some("deployments") => "/api/l402/deployments".to_string(),
            Some(_) => "/api/l402/wallet".to_string(),
        }
    }

    fn schedule_admin_worker_surface_refresh() {
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

        if !route_is_admin_surface(&route) {
            return;
        }

        let route_path = route.to_path();
        let selected_worker_id = admin_worker_id_from_route(&route).or_else(|| {
            ADMIN_WORKER_SURFACE_STATE.with(|state| state.borrow().selected_worker_id.clone())
        });

        let Some(access_token) = access_token else {
            reset_admin_worker_surface_state();
            return;
        };
        let Some(session_id) = session_id else {
            reset_admin_worker_surface_state();
            return;
        };
        if access_token.trim().is_empty() {
            reset_admin_worker_surface_state();
            return;
        }

        let already_loaded = ADMIN_WORKER_SURFACE_STATE.with(|state| {
            let state = state.borrow();
            state.loaded_session_id.as_deref() == Some(session_id.as_str())
                && state.loaded_route_path.as_deref() == Some(route_path.as_str())
                && state.selected_worker_id == selected_worker_id
                && state.last_error.is_none()
        });
        if already_loaded {
            return;
        }

        let already_loading = ADMIN_WORKER_SURFACE_LOADING.with(|loading| {
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
            let result = fetch_admin_worker_surface_state(
                &access_token,
                &session_id,
                &route_path,
                selected_worker_id.clone(),
            )
            .await;
            ADMIN_WORKER_SURFACE_LOADING.with(|loading| loading.set(false));
            ADMIN_WORKER_SURFACE_STATE.with(|state| {
                let mut state = state.borrow_mut();
                match result {
                    Ok(snapshot) => {
                        *state = snapshot;
                    }
                    Err(error) => {
                        state.loaded_session_id = Some(session_id.clone());
                        state.loaded_route_path = Some(route_path.clone());
                        state.selected_worker_id = selected_worker_id;
                        state.last_error = Some(error.message);
                    }
                }
            });
            render_codex_chat_dom();
        });
    }

    async fn fetch_admin_worker_surface_state(
        access_token: &str,
        session_id: &str,
        route_path: &str,
        selected_worker_id: Option<String>,
    ) -> Result<AdminWorkerSurfaceState, ControlApiError> {
        let list_request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: "/api/runtime/codex/workers?limit=50".to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        let listed: JsonDataEnvelope =
            send_json_request(&list_request, &AppState::default()).await?;
        let workers = listed.data.as_array().cloned().unwrap_or_default();

        let selected_worker_id = selected_worker_id
            .and_then(trimmed_non_empty_string)
            .or_else(|| {
                workers
                    .iter()
                    .find_map(|worker| worker.get("worker_id"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            });

        let mut worker_snapshot = None;
        let mut worker_stream = None;

        if let Some(worker_id) = selected_worker_id.as_ref() {
            let encoded_worker_id = encode_path_component(worker_id);
            let show_request = HttpCommandRequest {
                method: HttpMethod::Get,
                path: format!("/api/runtime/codex/workers/{encoded_worker_id}"),
                body: None,
                auth: AuthRequirement::None,
                headers: vec![(
                    "authorization".to_string(),
                    format!("Bearer {access_token}"),
                )],
            };
            let showed: JsonDataEnvelope =
                send_json_request(&show_request, &AppState::default()).await?;
            worker_snapshot = Some(showed.data);

            let stream_request = HttpCommandRequest {
                method: HttpMethod::Get,
                path: format!(
                    "/api/runtime/codex/workers/{encoded_worker_id}/stream?cursor=0&tail_ms=15000"
                ),
                body: None,
                auth: AuthRequirement::None,
                headers: vec![(
                    "authorization".to_string(),
                    format!("Bearer {access_token}"),
                )],
            };
            let streamed: JsonDataEnvelope =
                send_json_request(&stream_request, &AppState::default()).await?;
            worker_stream = Some(streamed.data);
        }

        Ok(AdminWorkerSurfaceState {
            loaded_session_id: Some(session_id.to_string()),
            loaded_route_path: Some(route_path.to_string()),
            selected_worker_id,
            workers,
            worker_snapshot,
            worker_stream,
            last_response: None,
            last_status: None,
            last_error: None,
        })
    }

    fn encode_path_component(raw: &str) -> String {
        js_sys::encode_uri_component(raw)
            .as_string()
            .unwrap_or_else(|| raw.to_string())
    }

    fn trimmed_non_empty_string(raw: String) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn admin_worker_id_from_route(route: &AppRoute) -> Option<String> {
        let AppRoute::Admin { section } = route else {
            return None;
        };
        let section = section.as_ref()?;
        section
            .strip_prefix("workers/")
            .map(ToString::to_string)
            .and_then(trimmed_non_empty_string)
    }

    fn apply_autopilot_payload_to_settings_state(
        state: &mut SettingsSurfaceState,
        autopilot: Option<&AutopilotPayload>,
    ) {
        let Some(autopilot) = autopilot else {
            return;
        };

        state.autopilot_display_name = autopilot.display_name.clone();
        state.autopilot_tagline = autopilot.tagline.clone().unwrap_or_default();
        state.autopilot_owner_display_name = autopilot.profile.owner_display_name.clone();
        state.autopilot_persona_summary = autopilot
            .profile
            .persona_summary
            .clone()
            .unwrap_or_default();
        state.autopilot_voice = autopilot
            .profile
            .autopilot_voice
            .clone()
            .unwrap_or_default();
        state.autopilot_principles_text =
            principles_text_from_value(autopilot.profile.principles.as_ref());
    }

    fn principles_text_from_value(value: Option<&serde_json::Value>) -> String {
        let Some(value) = value else {
            return String::new();
        };
        let Some(values) = value.as_array() else {
            return String::new();
        };

        values
            .iter()
            .filter_map(|entry| entry.as_str())
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
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
        epoch_millis_now()
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

    fn handle_global_shortcut(event: web_sys::KeyboardEvent) {
        if event.default_prevented() || event.repeat() {
            return;
        }
        if event.key().to_ascii_lowercase() != "k" {
            return;
        }
        if !(event.meta_key() || event.ctrl_key()) || event.alt_key() || event.shift_key() {
            return;
        }

        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        if active_element_accepts_text(&document) {
            return;
        }

        event.prevent_default();
        let should_navigate_to_chat = APP_STATE.with(|state| {
            let route = state.borrow().route.clone();
            !route_is_codex_chat_surface(&route)
        });
        if should_navigate_to_chat {
            apply_route_transition(AppRoute::Chat { thread_id: None }, true);
        }
        let _ = focus_codex_chat_input(&document);
    }

    fn focus_codex_chat_input(document: &web_sys::Document) -> bool {
        let Some(composer) = document
            .get_element_by_id(CODEX_CHAT_COMPOSER_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        else {
            return false;
        };
        if composer
            .style()
            .get_property_value("display")
            .unwrap_or_default()
            == "none"
        {
            return false;
        }

        let Some(input) = document
            .get_element_by_id(CODEX_CHAT_INPUT_ID)
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        else {
            return false;
        };
        let _ = input.focus();
        true
    }

    fn active_element_accepts_text(document: &web_sys::Document) -> bool {
        let Some(active) = document.active_element() else {
            return false;
        };

        if active.has_attribute("contenteditable") {
            let content_editable = active
                .get_attribute("contenteditable")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if content_editable != "false" {
                return true;
            }
        }

        let tag = active.tag_name().to_ascii_lowercase();
        if tag == "textarea" {
            return true;
        }
        if tag != "input" {
            return false;
        }

        if let Ok(input) = active.dyn_into::<HtmlInputElement>() {
            let kind = input.type_();
            is_text_input_type(&kind)
        } else {
            true
        }
    }

    fn is_text_input_type(input_type: &str) -> bool {
        matches!(
            input_type.trim().to_ascii_lowercase().as_str(),
            "" | "text" | "search" | "email" | "password" | "tel" | "url" | "number"
        )
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

    fn settings_set_loading(value: bool) {
        SETTINGS_SURFACE_LOADING.with(|loading| loading.set(value));
    }

    fn settings_set_status(message: impl Into<String>) {
        SETTINGS_SURFACE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.last_status = Some(message.into());
            state.last_error = None;
        });
    }

    fn settings_set_error(message: impl Into<String>) {
        SETTINGS_SURFACE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.last_error = Some(message.into());
            state.last_status = None;
        });
    }

    fn admin_set_loading(value: bool) {
        ADMIN_WORKER_SURFACE_LOADING.with(|loading| loading.set(value));
    }

    fn admin_set_status(message: impl Into<String>) {
        ADMIN_WORKER_SURFACE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.last_status = Some(message.into());
            state.last_error = None;
        });
    }

    fn admin_set_error(message: impl Into<String>) {
        ADMIN_WORKER_SURFACE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.last_error = Some(message.into());
            state.last_status = None;
        });
    }

    fn admin_set_last_response(payload: serde_json::Value) {
        ADMIN_WORKER_SURFACE_STATE.with(|state| {
            state.borrow_mut().last_response = Some(payload);
        });
    }

    fn force_admin_surface_refresh(selected_worker_id: Option<String>) {
        ADMIN_WORKER_SURFACE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if let Some(worker_id) = selected_worker_id
                .clone()
                .and_then(trimmed_non_empty_string)
            {
                state.selected_worker_id = Some(worker_id);
            }
            state.loaded_route_path = None;
            state.last_error = None;
        });
        schedule_admin_worker_surface_refresh();
    }

    fn active_admin_worker_id_from_inputs_or_state() -> Option<String> {
        read_input_value(ADMIN_WORKER_ID_ID)
            .and_then(trimmed_non_empty_string)
            .or_else(|| {
                ADMIN_WORKER_SURFACE_STATE.with(|state| state.borrow().selected_worker_id.clone())
            })
    }

    fn parse_json_object_input(
        raw: &str,
        field: &str,
    ) -> Result<serde_json::Value, ControlApiError> {
        if raw.trim().is_empty() {
            return Ok(serde_json::json!({}));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(raw).map_err(|error| ControlApiError {
                status_code: 422,
                code: Some("validation_error".to_string()),
                message: format!("{field} must be valid JSON: {error}"),
                kind: CommandErrorKind::Validation,
                retryable: false,
            })?;
        if !parsed.is_object() {
            return Err(ControlApiError {
                status_code: 422,
                code: Some("validation_error".to_string()),
                message: format!("{field} must be a JSON object."),
                kind: CommandErrorKind::Validation,
                retryable: false,
            });
        }
        Ok(parsed)
    }

    async fn admin_get_json<T: for<'de> Deserialize<'de>>(
        path: &str,
    ) -> Result<T, ControlApiError> {
        let access_token = current_access_token()?;
        let request = HttpCommandRequest {
            method: HttpMethod::Get,
            path: path.to_string(),
            body: None,
            auth: AuthRequirement::None,
            headers: vec![(
                "authorization".to_string(),
                format!("Bearer {access_token}"),
            )],
        };
        send_json_request(&request, &AppState::default()).await
    }

    fn submit_admin_worker_refresh_from_inputs() {
        let worker_id = active_admin_worker_id_from_inputs_or_state();
        admin_set_status("Refreshing worker view...");
        force_admin_surface_refresh(worker_id);
        render_codex_chat_dom();
    }

    fn submit_admin_worker_create_from_inputs() {
        let worker_id = read_input_value(ADMIN_WORKER_ID_ID).and_then(trimmed_non_empty_string);
        let workspace_ref = read_input_value(ADMIN_WORKSPACE_ID).and_then(trimmed_non_empty_string);
        let adapter = read_input_value(ADMIN_ADAPTER_ID).and_then(trimmed_non_empty_string);

        admin_set_loading(true);
        admin_set_status("Creating or reattaching worker...");
        render_codex_chat_dom();

        spawn_local(async move {
            let mut body = serde_json::Map::new();
            if let Some(worker_id) = worker_id.clone() {
                body.insert("worker_id".to_string(), serde_json::json!(worker_id));
            }
            if let Some(workspace_ref) = workspace_ref.clone() {
                body.insert(
                    "workspace_ref".to_string(),
                    serde_json::json!(workspace_ref),
                );
            }
            if let Some(adapter) = adapter.clone() {
                body.insert("adapter".to_string(), serde_json::json!(adapter));
            }

            let result = settings_post_json::<JsonDataEnvelope>(
                "/api/runtime/codex/workers",
                serde_json::Value::Object(body),
            )
            .await;
            admin_set_loading(false);
            match result {
                Ok(response) => {
                    let selected_worker_id = response
                        .data
                        .get("workerId")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                        .or(worker_id);
                    admin_set_last_response(response.data);
                    admin_set_status("Worker create/reattach accepted.");
                    force_admin_surface_refresh(selected_worker_id);
                }
                Err(error) => {
                    admin_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_admin_worker_stop_from_inputs() {
        let Some(worker_id) = active_admin_worker_id_from_inputs_or_state() else {
            admin_set_error("Worker id is required before stop.");
            render_codex_chat_dom();
            return;
        };
        let confirmation = read_input_value(ADMIN_STOP_CONFIRM_ID)
            .and_then(trimmed_non_empty_string)
            .unwrap_or_default();
        if confirmation != worker_id {
            admin_set_error("Stop confirmation must exactly match worker id.");
            render_codex_chat_dom();
            return;
        }
        let reason = read_input_value(ADMIN_STOP_REASON_ID).and_then(trimmed_non_empty_string);
        let encoded = encode_path_component(&worker_id);
        let path = format!("/api/runtime/codex/workers/{encoded}/stop");
        let body = reason
            .map(|reason| serde_json::json!({ "reason": reason }))
            .unwrap_or_else(|| serde_json::json!({}));

        admin_set_loading(true);
        admin_set_status("Stopping worker...");
        render_codex_chat_dom();

        spawn_local(async move {
            let result = settings_post_json::<JsonDataEnvelope>(&path, body).await;
            admin_set_loading(false);
            match result {
                Ok(response) => {
                    admin_set_last_response(response.data);
                    admin_set_status("Worker stop request accepted.");
                    force_admin_surface_refresh(Some(worker_id));
                }
                Err(error) => {
                    admin_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_admin_worker_request_from_inputs() {
        let Some(worker_id) = active_admin_worker_id_from_inputs_or_state() else {
            admin_set_error("Worker id is required before sending a control request.");
            render_codex_chat_dom();
            return;
        };
        let method = read_input_value(ADMIN_REQUEST_METHOD_ID)
            .and_then(trimmed_non_empty_string)
            .unwrap_or_else(|| "thread/list".to_string())
            .to_lowercase();
        if !ADMIN_WORKER_ALLOWED_METHODS.contains(&method.as_str()) {
            admin_set_error("Control method is not allowlisted for admin UI.");
            render_codex_chat_dom();
            return;
        }
        let params_raw = read_input_value(ADMIN_REQUEST_PARAMS_ID).unwrap_or_default();
        let params = match parse_json_object_input(&params_raw, "request params") {
            Ok(value) => value,
            Err(error) => {
                admin_set_error(error.message);
                render_codex_chat_dom();
                return;
            }
        };
        let request_id = read_input_value(ADMIN_REQUEST_ID_ID)
            .and_then(trimmed_non_empty_string)
            .unwrap_or_else(|| format!("req_{}", current_unix_ms()));

        let encoded = encode_path_component(&worker_id);
        let path = format!("/api/runtime/codex/workers/{encoded}/requests");
        let body = serde_json::json!({
            "request": {
                "request_id": request_id,
                "method": method,
                "params": params,
                "source": "openagents.web.admin",
                "request_version": "v1"
            }
        });

        admin_set_loading(true);
        admin_set_status("Submitting worker control request...");
        render_codex_chat_dom();

        spawn_local(async move {
            let result = settings_post_json::<JsonDataEnvelope>(&path, body).await;
            admin_set_loading(false);
            match result {
                Ok(response) => {
                    admin_set_last_response(response.data);
                    admin_set_status("Worker control request accepted.");
                    force_admin_surface_refresh(Some(worker_id));
                }
                Err(error) => {
                    admin_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_admin_worker_event_from_inputs() {
        let Some(worker_id) = active_admin_worker_id_from_inputs_or_state() else {
            admin_set_error("Worker id is required before sending an event.");
            render_codex_chat_dom();
            return;
        };
        let event_type = read_input_value(ADMIN_EVENT_TYPE_ID)
            .and_then(trimmed_non_empty_string)
            .unwrap_or_else(|| "worker.event".to_string());
        if !event_type.starts_with("worker.") {
            admin_set_error("Event type must start with `worker.`.");
            render_codex_chat_dom();
            return;
        }
        let payload_raw = read_input_value(ADMIN_EVENT_PAYLOAD_ID).unwrap_or_default();
        let payload = match parse_json_object_input(&payload_raw, "event payload") {
            Ok(value) => value,
            Err(error) => {
                admin_set_error(error.message);
                render_codex_chat_dom();
                return;
            }
        };
        let encoded = encode_path_component(&worker_id);
        let path = format!("/api/runtime/codex/workers/{encoded}/events");
        let body = serde_json::json!({
            "event": {
                "event_type": event_type,
                "payload": payload,
            }
        });

        admin_set_loading(true);
        admin_set_status("Appending worker event...");
        render_codex_chat_dom();

        spawn_local(async move {
            let result = settings_post_json::<JsonDataEnvelope>(&path, body).await;
            admin_set_loading(false);
            match result {
                Ok(response) => {
                    admin_set_last_response(response.data);
                    admin_set_status("Worker event accepted.");
                    force_admin_surface_refresh(Some(worker_id));
                }
                Err(error) => {
                    admin_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_admin_worker_stream_fetch_from_inputs() {
        let Some(worker_id) = active_admin_worker_id_from_inputs_or_state() else {
            admin_set_error("Worker id is required before fetching stream.");
            render_codex_chat_dom();
            return;
        };
        let cursor = read_input_value(ADMIN_STREAM_CURSOR_ID)
            .and_then(trimmed_non_empty_string)
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(0);
        let tail_ms = read_input_value(ADMIN_STREAM_TAIL_ID)
            .and_then(trimmed_non_empty_string)
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(15_000)
            .clamp(1, 120_000);
        let encoded = encode_path_component(&worker_id);
        let path = format!(
            "/api/runtime/codex/workers/{encoded}/stream?cursor={cursor}&tail_ms={tail_ms}"
        );

        admin_set_loading(true);
        admin_set_status("Fetching worker stream snapshot...");
        render_codex_chat_dom();

        spawn_local(async move {
            let result = admin_get_json::<JsonDataEnvelope>(&path).await;
            admin_set_loading(false);
            match result {
                Ok(response) => {
                    ADMIN_WORKER_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.selected_worker_id = Some(worker_id.clone());
                        state.worker_stream = Some(response.data.clone());
                    });
                    admin_set_last_response(response.data);
                    admin_set_status("Worker stream fetched.");
                    force_admin_surface_refresh(Some(worker_id));
                }
                Err(error) => {
                    admin_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn current_access_token() -> Result<String, ControlApiError> {
        APP_STATE
            .with(|state| state.borrow().auth.access_token.clone())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ControlApiError::from_command_error(CommandError::missing_credential(
                    "access token is unavailable",
                ))
            })
    }

    async fn settings_post_json<T: for<'de> Deserialize<'de>>(
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, ControlApiError> {
        let access_token = current_access_token()?;
        let body = serde_json::to_string(&body).map_err(|error| ControlApiError {
            status_code: 500,
            code: Some("request_body_serialize_failed".to_string()),
            message: format!("failed to serialize request body: {error}"),
            kind: CommandErrorKind::Decode,
            retryable: false,
        })?;
        let response = Request::post(path)
            .header("authorization", &format!("Bearer {access_token}"))
            .header("content-type", "application/json")
            .body(body)
            .map_err(map_network_error)?
            .send()
            .await
            .map_err(map_network_error)?;
        decode_json_response(response).await
    }

    async fn settings_patch_json<T: for<'de> Deserialize<'de>>(
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, ControlApiError> {
        let access_token = current_access_token()?;
        let body = serde_json::to_string(&body).map_err(|error| ControlApiError {
            status_code: 500,
            code: Some("request_body_serialize_failed".to_string()),
            message: format!("failed to serialize request body: {error}"),
            kind: CommandErrorKind::Decode,
            retryable: false,
        })?;
        let response = Request::patch(path)
            .header("authorization", &format!("Bearer {access_token}"))
            .header("content-type", "application/json")
            .body(body)
            .map_err(map_network_error)?
            .send()
            .await
            .map_err(map_network_error)?;
        decode_json_response(response).await
    }

    async fn settings_delete_json<T: for<'de> Deserialize<'de>>(
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ControlApiError> {
        let access_token = current_access_token()?;
        let mut builder =
            Request::delete(path).header("authorization", &format!("Bearer {access_token}"));

        let response = if let Some(body) = body {
            let body = serde_json::to_string(&body).map_err(|error| ControlApiError {
                status_code: 500,
                code: Some("request_body_serialize_failed".to_string()),
                message: format!("failed to serialize request body: {error}"),
                kind: CommandErrorKind::Decode,
                retryable: false,
            })?;
            builder = builder.header("content-type", "application/json");
            builder
                .body(body)
                .map_err(map_network_error)?
                .send()
                .await
                .map_err(map_network_error)?
        } else {
            builder.send().await.map_err(map_network_error)?
        };
        decode_json_response(response).await
    }

    fn read_input_value(id: &str) -> Option<String> {
        let window = web_sys::window()?;
        let document = window.document()?;
        let input = document.get_element_by_id(id)?;
        let input = input.dyn_into::<HtmlInputElement>().ok()?;
        Some(input.value())
    }

    fn submit_settings_profile_update_from_inputs() {
        let name = read_input_value(SETTINGS_PROFILE_NAME_ID)
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            settings_set_error("Profile name is required.");
            render_codex_chat_dom();
            return;
        }
        if name.chars().count() > 255 {
            settings_set_error("Profile name may not be greater than 255 characters.");
            render_codex_chat_dom();
            return;
        }

        settings_set_loading(true);
        settings_set_status("Saving profile...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_patch_json::<SettingsProfileEnvelope>(
                "/api/settings/profile",
                serde_json::json!({ "name": name }),
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.profile_id = Some(response.data.id);
                        state.profile_name = response.data.name;
                        state.profile_email = response.data.email;
                        state.last_status = Some("Profile saved.".to_string());
                        state.last_error = None;
                    });
                }
                Err(error) => {
                    settings_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_profile_delete() {
        let email = SETTINGS_SURFACE_STATE.with(|state| state.borrow().profile_email.clone());
        if email.trim().is_empty() {
            settings_set_error("Profile email is unavailable; refresh settings first.");
            render_codex_chat_dom();
            return;
        }

        settings_set_loading(true);
        settings_set_status("Deleting profile...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_delete_json::<SettingsDeleteProfileEnvelope>(
                "/api/settings/profile",
                Some(serde_json::json!({ "email": email })),
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    if response.data.deleted {
                        settings_set_status("Profile deleted. You may need to sign in again.");
                    } else {
                        settings_set_error("Profile delete response did not confirm deletion.");
                    }
                }
                Err(error) => {
                    settings_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_autopilot_update_from_inputs() {
        let display_name = read_input_value(SETTINGS_AUTOPILOT_DISPLAY_NAME_ID).unwrap_or_default();
        let tagline = read_input_value(SETTINGS_AUTOPILOT_TAGLINE_ID).unwrap_or_default();
        let owner_display_name = read_input_value(SETTINGS_AUTOPILOT_OWNER_ID).unwrap_or_default();
        let persona_summary = read_input_value(SETTINGS_AUTOPILOT_PERSONA_ID).unwrap_or_default();
        let autopilot_voice = read_input_value(SETTINGS_AUTOPILOT_VOICE_ID).unwrap_or_default();
        let principles_text =
            read_input_value(SETTINGS_AUTOPILOT_PRINCIPLES_ID).unwrap_or_default();

        if display_name.chars().count() > 120 {
            settings_set_error("Autopilot display name may not exceed 120 characters.");
            render_codex_chat_dom();
            return;
        }
        if tagline.chars().count() > 255 {
            settings_set_error("Autopilot tagline may not exceed 255 characters.");
            render_codex_chat_dom();
            return;
        }
        if owner_display_name.chars().count() > 120 {
            settings_set_error("Owner display name may not exceed 120 characters.");
            render_codex_chat_dom();
            return;
        }
        if autopilot_voice.chars().count() > 64 {
            settings_set_error("Autopilot voice may not exceed 64 characters.");
            render_codex_chat_dom();
            return;
        }

        settings_set_loading(true);
        settings_set_status("Saving autopilot settings...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_patch_json::<SettingsAutopilotUpdateEnvelope>(
                "/settings/autopilot",
                serde_json::json!({
                    "displayName": display_name,
                    "tagline": tagline,
                    "ownerDisplayName": owner_display_name,
                    "personaSummary": persona_summary,
                    "autopilotVoice": autopilot_voice,
                    "principlesText": principles_text,
                }),
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        apply_autopilot_payload_to_settings_state(
                            &mut state,
                            Some(&response.data.autopilot),
                        );
                        state.last_status =
                            Some(format!("Autopilot saved: {}", response.data.status));
                        state.last_error = None;
                    });
                }
                Err(error) => {
                    settings_set_error(error.message);
                }
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_resend_connect_from_inputs() {
        let resend_api_key = read_input_value(SETTINGS_RESEND_KEY_ID)
            .unwrap_or_default()
            .trim()
            .to_string();
        let sender_email = read_input_value(SETTINGS_RESEND_EMAIL_ID)
            .unwrap_or_default()
            .trim()
            .to_string();
        let sender_name = read_input_value(SETTINGS_RESEND_NAME_ID)
            .unwrap_or_default()
            .trim()
            .to_string();

        if resend_api_key.len() < 8 {
            settings_set_error("Resend API key must be at least 8 characters.");
            render_codex_chat_dom();
            return;
        }
        if resend_api_key.len() > 4096 {
            settings_set_error("Resend API key may not exceed 4096 characters.");
            render_codex_chat_dom();
            return;
        }
        if !sender_email.is_empty() && !sender_email.contains('@') {
            settings_set_error("Resend sender email must be a valid email address.");
            render_codex_chat_dom();
            return;
        }

        settings_set_loading(true);
        settings_set_status("Connecting Resend integration...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_post_json::<SettingsIntegrationEnvelope>(
                "/settings/integrations/resend",
                serde_json::json!({
                    "resendApiKey": resend_api_key,
                    "senderEmail": if sender_email.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(sender_email) },
                    "senderName": if sender_name.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(sender_name) },
                }),
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.resend_connected = Some(response.data.integration.connected);
                        state.resend_secret_last4 = response.data.integration.secret_last4.clone();
                        state.last_status = Some(format!(
                            "Resend status: {} ({})",
                            response.data.status,
                            response.data.action.unwrap_or_else(|| "ok".to_string())
                        ));
                        state.last_error = None;
                    });
                }
                Err(error) => settings_set_error(error.message),
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_resend_disconnect() {
        settings_set_loading(true);
        settings_set_status("Disconnecting Resend integration...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_delete_json::<SettingsIntegrationEnvelope>(
                "/settings/integrations/resend",
                None,
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.resend_connected = Some(response.data.integration.connected);
                        state.resend_secret_last4 = response.data.integration.secret_last4.clone();
                        state.last_status =
                            Some(format!("Resend status: {}", response.data.status));
                        state.last_error = None;
                    });
                }
                Err(error) => settings_set_error(error.message),
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_resend_test() {
        settings_set_loading(true);
        settings_set_status("Sending Resend integration test...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_post_json::<SettingsIntegrationEnvelope>(
                "/settings/integrations/resend/test",
                serde_json::json!({}),
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.last_status =
                            Some(format!("Resend status: {}", response.data.status));
                        state.last_error = None;
                    });
                }
                Err(error) => settings_set_error(error.message),
            }
            render_codex_chat_dom();
        });
    }

    fn submit_settings_google_disconnect() {
        settings_set_loading(true);
        settings_set_status("Disconnecting Google integration...");
        render_codex_chat_dom();

        spawn_local(async move {
            let response = settings_delete_json::<SettingsIntegrationEnvelope>(
                "/settings/integrations/google",
                None,
            )
            .await;
            settings_set_loading(false);
            match response {
                Ok(response) => {
                    SETTINGS_SURFACE_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.google_connected = Some(response.data.integration.connected);
                        state.google_secret_last4 = response.data.integration.secret_last4.clone();
                        state.last_status =
                            Some(format!("Google status: {}", response.data.status));
                        state.last_error = None;
                    });
                }
                Err(error) => settings_set_error(error.message),
            }
            render_codex_chat_dom();
        });
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
        let codex_history_state = CODEX_HISTORY_STATE.with(|state| state.borrow().clone());
        let codex_history_loading = CODEX_HISTORY_LOADING.with(Cell::get);
        let management_state = MANAGEMENT_SURFACE_STATE.with(|state| state.borrow().clone());
        let management_loading = MANAGEMENT_SURFACE_LOADING.with(Cell::get);
        let settings_state = SETTINGS_SURFACE_STATE.with(|state| state.borrow().clone());
        let settings_loading = SETTINGS_SURFACE_LOADING.with(Cell::get);
        let l402_state = L402_SURFACE_STATE.with(|state| state.borrow().clone());
        let l402_loading = L402_SURFACE_LOADING.with(Cell::get);
        let admin_state = ADMIN_WORKER_SURFACE_STATE.with(|state| state.borrow().clone());
        let admin_loading = ADMIN_WORKER_SURFACE_LOADING.with(Cell::get);

        let thread_id = thread_id_from_route(&route);
        let is_management_route = route_is_management_surface(&route);
        let is_auth_route = route_is_auth_surface(&route);
        let is_codex_chat_route = route_is_codex_chat_surface(&route);
        if thread_id.is_none() && !is_management_route && !is_auth_route && !is_codex_chat_route {
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
        remember_scroll_position_for_route(&messages_container, &route);
        messages_container.set_inner_html("");

        let Some(composer) = document
            .get_element_by_id(CODEX_CHAT_COMPOSER_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        else {
            return;
        };
        let quick_prompt_row = document
            .get_element_by_id(CODEX_CHAT_QUICK_PROMPTS_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());
        let auth_panel = document
            .get_element_by_id(AUTH_PANEL_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());
        let settings_panel = document
            .get_element_by_id(SETTINGS_PANEL_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());
        let admin_panel = document
            .get_element_by_id(ADMIN_PANEL_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());

        if is_codex_chat_route && !auth_state.has_active_session() {
            let _ = root.style().set_property("display", "flex");
            let _ = composer.style().set_property("display", "none");
            if let Some(quick_prompt_row) = quick_prompt_row.as_ref() {
                let _ = quick_prompt_row.style().set_property("display", "none");
            }
            if let Some(auth_panel) = auth_panel.as_ref() {
                let _ = auth_panel.style().set_property("display", "none");
            }
            if let Some(settings_panel) = settings_panel.as_ref() {
                let _ = settings_panel.style().set_property("display", "none");
            }
            if let Some(admin_panel) = admin_panel.as_ref() {
                let _ = admin_panel.style().set_property("display", "none");
            }
            render_chat_auth_gate_messages(&document, &messages_container, &route);
            restore_route_scroll_position(&messages_container, &route, 0);
            return;
        }

        if let Some(thread_id) = thread_id {
            let _ = root.style().set_property("display", "flex");
            let _ = composer.style().set_property("display", "flex");
            if let Some(quick_prompt_row) = quick_prompt_row.as_ref() {
                let _ = quick_prompt_row.style().set_property("display", "none");
            }
            if let Some(auth_panel) = auth_panel.as_ref() {
                let _ = auth_panel.style().set_property("display", "none");
            }
            if let Some(settings_panel) = settings_panel.as_ref() {
                let _ = settings_panel.style().set_property("display", "none");
            }
            if let Some(admin_panel) = admin_panel.as_ref() {
                let _ = admin_panel.style().set_property("display", "none");
            }
            render_codex_thread_messages(&document, &messages_container);
            render_codex_thread_status(
                &document,
                &messages_container,
                &thread_id,
                &codex_history_state,
                codex_history_loading,
            );
            messages_container.set_scroll_top(messages_container.scroll_height());
            if thread_id.is_empty() {
                let _ = messages_container.style().set_property("opacity", "0.9");
            } else {
                let _ = messages_container.style().set_property("opacity", "1");
            }
            return;
        }

        if is_codex_chat_route {
            let _ = root.style().set_property("display", "flex");
            let _ = composer.style().set_property("display", "flex");
            if let Some(quick_prompt_row) = quick_prompt_row.as_ref() {
                let _ = quick_prompt_row.style().set_property("display", "flex");
            }
            if let Some(auth_panel) = auth_panel.as_ref() {
                let _ = auth_panel.style().set_property("display", "none");
            }
            if let Some(settings_panel) = settings_panel.as_ref() {
                let _ = settings_panel.style().set_property("display", "none");
            }
            if let Some(admin_panel) = admin_panel.as_ref() {
                let _ = admin_panel.style().set_property("display", "none");
            }
            render_chat_landing_messages(
                &document,
                &messages_container,
                &route,
                &auth_state,
                &codex_history_state,
                codex_history_loading,
            );
            restore_route_scroll_position(&messages_container, &route, 0);
            return;
        }

        let _ = root.style().set_property("display", "flex");
        let _ = composer.style().set_property("display", "none");
        if let Some(quick_prompt_row) = quick_prompt_row.as_ref() {
            let _ = quick_prompt_row.style().set_property("display", "none");
        }
        if let Some(auth_panel) = auth_panel.as_ref() {
            let _ = auth_panel
                .style()
                .set_property("display", if is_auth_route { "flex" } else { "none" });
        }
        if let Some(settings_panel) = settings_panel.as_ref() {
            let _ = settings_panel.style().set_property(
                "display",
                if route_is_settings_surface(&route) {
                    "flex"
                } else {
                    "none"
                },
            );
        }
        if let Some(admin_panel) = admin_panel.as_ref() {
            let _ = admin_panel.style().set_property(
                "display",
                if route_is_admin_surface(&route) {
                    "flex"
                } else {
                    "none"
                },
            );
        }

        if is_auth_route {
            sync_auth_form_inputs(&document, &auth_state);
            render_auth_surface_messages(&document, &messages_container, &route, &auth_state);
            restore_route_scroll_position(&messages_container, &route, 0);
            return;
        }

        if route_is_settings_surface(&route) {
            sync_settings_form_inputs(&document, &settings_state, settings_loading);
        }

        if route_is_admin_surface(&route) {
            sync_admin_worker_form_inputs(&document, &admin_state, admin_loading);
            render_admin_worker_surface_messages(
                &document,
                &messages_container,
                &route,
                &admin_state,
                admin_loading,
            );
            restore_route_scroll_position(&messages_container, &route, 0);
            return;
        }

        if route_is_l402_surface(&route) {
            render_l402_surface_messages(
                &document,
                &messages_container,
                &route,
                &auth_state,
                &management_state,
                &l402_state,
                l402_loading,
            );
            restore_route_scroll_position(&messages_container, &route, 0);
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
        restore_route_scroll_position(&messages_container, &route, 0);
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

    fn sync_settings_form_inputs(
        document: &web_sys::Document,
        settings_state: &SettingsSurfaceState,
        loading: bool,
    ) {
        set_settings_input_value(
            document,
            SETTINGS_PROFILE_NAME_ID,
            &settings_state.profile_name,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_DISPLAY_NAME_ID,
            &settings_state.autopilot_display_name,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_TAGLINE_ID,
            &settings_state.autopilot_tagline,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_OWNER_ID,
            &settings_state.autopilot_owner_display_name,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_PERSONA_ID,
            &settings_state.autopilot_persona_summary,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_VOICE_ID,
            &settings_state.autopilot_voice,
        );
        set_settings_input_value(
            document,
            SETTINGS_AUTOPILOT_PRINCIPLES_ID,
            &settings_state.autopilot_principles_text,
        );

        let resend_connected = settings_state
            .resend_connected
            .map(|connected| {
                if connected {
                    "connected"
                } else {
                    "disconnected"
                }
            })
            .unwrap_or("unknown");
        let google_connected = settings_state
            .google_connected
            .map(|connected| {
                if connected {
                    "connected"
                } else {
                    "disconnected"
                }
            })
            .unwrap_or("unknown");

        if let Some(status) = document
            .get_element_by_id(SETTINGS_STATUS_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        {
            if loading {
                status.set_inner_text("Saving settings...");
                let _ = status.style().set_property("color", "#93c5fd");
            } else if let Some(error) = settings_state.last_error.as_ref() {
                status.set_inner_text(error);
                let _ = status.style().set_property("color", "#f87171");
            } else if let Some(message) = settings_state.last_status.as_ref() {
                status.set_inner_text(&format!(
                    "{message} | resend={resend_connected} google={google_connected}"
                ));
                let _ = status.style().set_property("color", "#93c5fd");
            } else {
                status.set_inner_text(&format!(
                    "profile={} | resend={}{} | google={}{}",
                    if settings_state.profile_email.is_empty() {
                        "unknown"
                    } else {
                        settings_state.profile_email.as_str()
                    },
                    resend_connected,
                    settings_state
                        .resend_secret_last4
                        .as_ref()
                        .map(|value| format!("({value})"))
                        .unwrap_or_default(),
                    google_connected,
                    settings_state
                        .google_secret_last4
                        .as_ref()
                        .map(|value| format!("({value})"))
                        .unwrap_or_default(),
                ));
                let _ = status.style().set_property("color", "#94a3b8");
            }
        }
    }

    fn sync_admin_worker_form_inputs(
        document: &web_sys::Document,
        admin_state: &AdminWorkerSurfaceState,
        loading: bool,
    ) {
        if let Some(worker_id) = admin_state.selected_worker_id.as_ref() {
            set_admin_input_value(document, ADMIN_WORKER_ID_ID, worker_id);
        }

        if let Some(snapshot) = admin_state.worker_snapshot.as_ref() {
            let workspace_ref = l402_value_text(snapshot.get("workspace_ref"));
            if workspace_ref != "-" {
                set_admin_input_value(document, ADMIN_WORKSPACE_ID, workspace_ref.as_str());
            }
            let adapter = l402_value_text(snapshot.get("adapter"));
            if adapter != "-" {
                set_admin_input_value(document, ADMIN_ADAPTER_ID, adapter.as_str());
            }
        }

        if let Some(status) = document
            .get_element_by_id(ADMIN_STATUS_ID)
            .and_then(|element| element.dyn_into::<HtmlElement>().ok())
        {
            if loading {
                status.set_inner_text("Running admin worker request...");
                let _ = status.style().set_property("color", "#93c5fd");
            } else if let Some(error) = admin_state.last_error.as_ref() {
                status.set_inner_text(error);
                let _ = status.style().set_property("color", "#f87171");
            } else if let Some(message) = admin_state.last_status.as_ref() {
                status.set_inner_text(message);
                let _ = status.style().set_property("color", "#93c5fd");
            } else {
                status.set_inner_text(&format!(
                    "workers={} selected={}",
                    admin_state.workers.len(),
                    admin_state.selected_worker_id.as_deref().unwrap_or("none")
                ));
                let _ = status.style().set_property("color", "#94a3b8");
            }
        }
    }

    fn set_admin_input_value(document: &web_sys::Document, id: &str, value: &str) {
        if let Some(input) = document
            .get_element_by_id(id)
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        {
            input.set_value(value);
        }
    }

    fn set_settings_input_value(document: &web_sys::Document, id: &str, value: &str) {
        if let Some(input) = document
            .get_element_by_id(id)
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        {
            input.set_value(value);
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

    fn render_codex_thread_status(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        thread_id: &str,
        history_state: &CodexHistoryState,
        history_loading: bool,
    ) {
        let message_count = CODEX_THREAD_STATE.with(|state| state.borrow().messages.len());
        if message_count > 0 {
            return;
        }

        if history_loading {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Transcript".to_string(),
                    body: format!("Loading Codex transcript for `{thread_id}`."),
                    tone: ManagementCardTone::Info,
                },
            );
            return;
        }

        if let Some(error) = history_state.last_error.as_ref() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Transcript Load Error".to_string(),
                    body: error.clone(),
                    tone: ManagementCardTone::Error,
                },
            );
            return;
        }

        if history_state.active_thread_exists == Some(false) {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "New Thread".to_string(),
                    body: format!(
                        "No stored transcript found for `{thread_id}` yet. Send a message to create it."
                    ),
                    tone: ManagementCardTone::Info,
                },
            );
            return;
        }

        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Transcript".to_string(),
                body: "No transcript messages yet for this thread.".to_string(),
                tone: ManagementCardTone::Neutral,
            },
        );
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

    fn render_chat_auth_gate_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
    ) {
        let cards = vec![
            ManagementCard {
                title: "Codex Access Required".to_string(),
                body: "Sign in to use Codex web chat. First-pass policy requires a ChatGPT-linked account."
                    .to_string(),
                tone: ManagementCardTone::Warning,
            },
            ManagementCard {
                title: "Next Step".to_string(),
                body: "Open /login, verify your email, then return to / or /chat/<thread-id>."
                    .to_string(),
                tone: ManagementCardTone::Info,
            },
            ManagementCard {
                title: "Route".to_string(),
                body: route.to_path(),
                tone: ManagementCardTone::Neutral,
            },
        ];

        for card in cards {
            append_management_card(document, messages_container, card);
        }
    }

    fn render_chat_landing_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
        auth_state: &openagents_app_state::AuthState,
        history_state: &CodexHistoryState,
        history_loading: bool,
    ) {
        let mut cards = vec![
            ManagementCard {
                title: "Codex Chat Lane".to_string(),
                body: "Web chat requests stream through `/api/chats/{thread}/stream` (Vercel-compatible SSE) while Khala remains the runtime sync lane."
                    .to_string(),
                tone: ManagementCardTone::Success,
            },
            ManagementCard {
                title: "Start Thread".to_string(),
                body: "Use a quick prompt below or send from the composer to create a new /chat/<thread-id> thread."
                    .to_string(),
                tone: ManagementCardTone::Info,
            },
            ManagementCard {
                title: "Route".to_string(),
                body: route.to_path(),
                tone: ManagementCardTone::Neutral,
            },
        ];

        if let Some(user) = auth_state.user.as_ref() {
            cards.push(ManagementCard {
                title: "Signed In".to_string(),
                body: format!("{} <{}>", user.name, user.email),
                tone: ManagementCardTone::Neutral,
            });
        }

        if history_loading {
            cards.push(ManagementCard {
                title: "Thread History".to_string(),
                body: "Loading from `/api/runtime/threads`.".to_string(),
                tone: ManagementCardTone::Info,
            });
        } else if let Some(error) = history_state.last_error.as_ref() {
            cards.push(ManagementCard {
                title: "Thread History Error".to_string(),
                body: error.clone(),
                tone: ManagementCardTone::Error,
            });
        } else if history_state.threads.is_empty() {
            cards.push(ManagementCard {
                title: "Thread History".to_string(),
                body: "No saved Codex threads yet.".to_string(),
                tone: ManagementCardTone::Warning,
            });
        } else {
            let history_listing = history_state
                .threads
                .iter()
                .take(8)
                .map(|thread| {
                    format!(
                        "/chat/{}  messages={}  updated={}",
                        thread.thread_id,
                        thread.message_count,
                        if thread.updated_at.is_empty() {
                            "unknown".to_string()
                        } else {
                            thread.updated_at.clone()
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            cards.push(ManagementCard {
                title: "Recent Threads".to_string(),
                body: history_listing,
                tone: ManagementCardTone::Neutral,
            });
        }

        for card in cards {
            append_management_card(document, messages_container, card);
        }
    }

    fn render_admin_worker_surface_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
        admin_state: &AdminWorkerSurfaceState,
        loading: bool,
    ) {
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Admin Route".to_string(),
                body: route.to_path(),
                tone: ManagementCardTone::Info,
            },
        );
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Action Safety".to_string(),
                body: "Stop requires exact worker-id confirmation. Control methods are allowlisted. Event payloads are validated as JSON objects."
                    .to_string(),
                tone: ManagementCardTone::Neutral,
            },
        );

        if let Some(panel) = append_l402_panel(document, messages_container, "Utility Surfaces") {
            for (href, label) in [
                ("/openapi.json", "OpenAPI document"),
                ("/api/smoke/stream", "Khala smoke stream contract"),
            ] {
                let Ok(anchor) = document.create_element("a") else {
                    continue;
                };
                let Ok(anchor) = anchor.dyn_into::<HtmlElement>() else {
                    continue;
                };
                let _ = anchor.set_attribute("href", href);
                let _ = anchor.style().set_property("display", "inline-flex");
                let _ = anchor.style().set_property("padding", "4px 0");
                let _ = anchor.style().set_property("color", "#93c5fd");
                let _ = anchor.style().set_property("font-size", "12px");
                let _ = anchor.style().set_property("text-decoration", "none");
                anchor.set_inner_text(&format!("{label} ({href})"));
                let _ = panel.append_child(&anchor);
            }
        }

        if loading {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Admin Data".to_string(),
                    body: "Loading codex worker list/snapshot/stream state.".to_string(),
                    tone: ManagementCardTone::Info,
                },
            );
            return;
        }
        if let Some(error) = admin_state.last_error.as_ref() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Admin Error".to_string(),
                    body: error.clone(),
                    tone: ManagementCardTone::Error,
                },
            );
        }

        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Workers".to_string(),
                body: format!(
                    "count={} selected={}",
                    admin_state.workers.len(),
                    admin_state.selected_worker_id.as_deref().unwrap_or("none")
                ),
                tone: if admin_state.workers.is_empty() {
                    ManagementCardTone::Warning
                } else {
                    ManagementCardTone::Success
                },
            },
        );

        if let Some(panel) = append_l402_panel(document, messages_container, "Worker Index") {
            if admin_state.workers.is_empty() {
                append_l402_panel_line(document, &panel, "No workers for this user.", true, false);
            } else {
                for worker in admin_state.workers.iter().take(40) {
                    let worker_id = l402_value_text(worker.get("worker_id"));
                    let status = l402_value_text(worker.get("status"));
                    let latest_seq = l402_value_text(worker.get("latest_seq"));
                    let heartbeat = l402_value_text(worker.get("heartbeat_state"));

                    let Ok(anchor) = document.create_element("a") else {
                        continue;
                    };
                    let Ok(anchor) = anchor.dyn_into::<HtmlElement>() else {
                        continue;
                    };
                    let _ = anchor
                        .set_attribute("href", format!("/admin/workers/{worker_id}").as_str());
                    let _ = anchor.style().set_property("display", "block");
                    let _ = anchor.style().set_property("padding", "6px 8px");
                    let _ = anchor.style().set_property("border-radius", "8px");
                    let _ = anchor.style().set_property("border", "1px solid #1f2937");
                    let _ = anchor.style().set_property("background", "#0b1220");
                    let _ = anchor.style().set_property("color", "#dbeafe");
                    let _ = anchor.style().set_property("font-size", "12px");
                    let _ = anchor.style().set_property("text-decoration", "none");
                    anchor.set_inner_text(&format!(
                        "{worker_id}  status={status}  latest_seq={latest_seq}  heartbeat={heartbeat}"
                    ));
                    let _ = panel.append_child(&anchor);
                }
            }
        }

        if let Some(snapshot) = admin_state.worker_snapshot.as_ref() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Selected Worker Snapshot".to_string(),
                    body: format!(
                        "worker_id={} status={} latest_seq={} workspace_ref={} adapter={} heartbeat_state={} heartbeat_age_ms={} updated_at={}",
                        l402_value_text(snapshot.get("worker_id")),
                        l402_value_text(snapshot.get("status")),
                        l402_value_text(snapshot.get("latest_seq")),
                        l402_value_text(snapshot.get("workspace_ref")),
                        l402_value_text(snapshot.get("adapter")),
                        l402_value_text(snapshot.get("heartbeat_state")),
                        l402_value_text(snapshot.get("heartbeat_age_ms")),
                        l402_value_text(snapshot.get("updated_at")),
                    ),
                    tone: ManagementCardTone::Neutral,
                },
            );
        }

        if let Some(stream) = admin_state.worker_stream.as_ref() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Delivery".to_string(),
                    body: format!(
                        "stream_protocol={} transport={} topic={} scope={} syncTokenRoute={} cursor={} tail_ms={}",
                        l402_value_text(stream.get("stream_protocol")),
                        l402_value_text(
                            stream
                                .get("delivery")
                                .and_then(|value| value.get("transport"))
                        ),
                        l402_value_text(
                            stream.get("delivery").and_then(|value| value.get("topic"))
                        ),
                        l402_value_text(
                            stream.get("delivery").and_then(|value| value.get("scope"))
                        ),
                        l402_value_text(
                            stream
                                .get("delivery")
                                .and_then(|value| value.get("syncTokenRoute"))
                        ),
                        l402_value_text(stream.get("cursor")),
                        l402_value_text(stream.get("tail_ms")),
                    ),
                    tone: ManagementCardTone::Info,
                },
            );

            if let Some(panel) =
                append_l402_panel(document, messages_container, "Worker Stream Events")
            {
                let events = stream
                    .get("events")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if events.is_empty() {
                    append_l402_panel_line(
                        document,
                        &panel,
                        "No events at current cursor.",
                        true,
                        false,
                    );
                } else {
                    for event in events.iter().take(120) {
                        append_l402_panel_line(
                            document,
                            &panel,
                            &format!(
                                "seq={} event_type={} occurred_at={}",
                                l402_value_text(event.get("seq")),
                                l402_value_text(event.get("event_type")),
                                l402_value_text(event.get("occurred_at")),
                            ),
                            true,
                            true,
                        );
                        if let Some(payload) = event.get("payload") {
                            if let Ok(payload_text) = serde_json::to_string(payload) {
                                append_l402_panel_line(document, &panel, &payload_text, true, true);
                            }
                        }
                    }
                }
            }
        }

        if let Some(last_response) = admin_state.last_response.as_ref() {
            if let Ok(response_json) = serde_json::to_string_pretty(last_response) {
                if let Some(panel) =
                    append_l402_panel(document, messages_container, "Last Admin Action Response")
                {
                    append_l402_pre(document, &panel, &response_json);
                }
            }
        }
    }

    fn render_l402_surface_messages(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        route: &AppRoute,
        auth_state: &openagents_app_state::AuthState,
        management_state: &ManagementSurfaceState,
        l402_state: &L402SurfaceState,
        loading: bool,
    ) {
        let route_path = route.to_path();
        append_l402_navigation(document, messages_container, route_path.as_str());

        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "L402 Route".to_string(),
                body: route_path,
                tone: ManagementCardTone::Info,
            },
        );

        if let Some(policy) = management_state.billing_policy.as_ref() {
            let tone = if policy.allowed {
                ManagementCardTone::Success
            } else {
                ManagementCardTone::Error
            };
            let denied_reasons = if policy.denied_reasons.is_empty() {
                "none".to_string()
            } else {
                policy.denied_reasons.join("|")
            };
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Policy".to_string(),
                    body: format!(
                        "allowed={} org={} scopes={} denied={}",
                        policy.allowed,
                        policy.resolved_org_id,
                        policy.granted_scopes.join(","),
                        denied_reasons
                    ),
                    tone,
                },
            );
        }

        if l402_section_from_route(route).as_deref() == Some("paywalls")
            && !has_admin_access(auth_state, management_state)
        {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Paywall Guard".to_string(),
                    body: "Paywall mutation operations require owner/admin membership on the active org."
                        .to_string(),
                    tone: ManagementCardTone::Warning,
                },
            );
        }

        if loading {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "L402 Data".to_string(),
                    body: format!("Loading {}.", l402_api_path_for_route(route)),
                    tone: ManagementCardTone::Info,
                },
            );
            return;
        }

        if let Some(error) = l402_state.last_error.as_ref() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "L402 Data Error".to_string(),
                    body: error.clone(),
                    tone: ManagementCardTone::Error,
                },
            );
            return;
        }

        let Some(payload) = l402_state.payload.as_ref() else {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "L402 Data".to_string(),
                    body: "No L402 data has loaded for this route yet.".to_string(),
                    tone: ManagementCardTone::Warning,
                },
            );
            return;
        };

        match l402_route_view(route) {
            L402RouteView::Wallet => render_l402_wallet_view(document, messages_container, payload),
            L402RouteView::Transactions => {
                render_l402_transactions_view(document, messages_container, payload)
            }
            L402RouteView::TransactionDetail => {
                render_l402_transaction_detail_view(document, messages_container, payload)
            }
            L402RouteView::Paywalls => {
                render_l402_paywalls_view(document, messages_container, payload)
            }
            L402RouteView::Settlements => {
                render_l402_settlements_view(document, messages_container, payload)
            }
            L402RouteView::Deployments => {
                render_l402_deployments_view(document, messages_container, payload)
            }
            L402RouteView::Unknown(section) => {
                append_management_card(
                    document,
                    messages_container,
                    ManagementCard {
                        title: "Unknown Section".to_string(),
                        body: format!("Unknown L402 section `{section}`."),
                        tone: ManagementCardTone::Warning,
                    },
                );
            }
        }
    }

    enum L402RouteView {
        Wallet,
        Transactions,
        TransactionDetail,
        Paywalls,
        Settlements,
        Deployments,
        Unknown(String),
    }

    fn l402_route_view(route: &AppRoute) -> L402RouteView {
        let section = l402_section_from_route(route).unwrap_or_else(|| "wallet".to_string());
        if section == "wallet" {
            return L402RouteView::Wallet;
        }
        if section == "transactions" {
            return L402RouteView::Transactions;
        }
        if section.starts_with("transactions/") {
            return L402RouteView::TransactionDetail;
        }
        if section == "paywalls" {
            return L402RouteView::Paywalls;
        }
        if section == "settlements" {
            return L402RouteView::Settlements;
        }
        if section == "deployments" {
            return L402RouteView::Deployments;
        }
        L402RouteView::Unknown(section)
    }

    fn l402_section_from_route(route: &AppRoute) -> Option<String> {
        match route {
            AppRoute::Billing { section } => Some(
                section
                    .clone()
                    .unwrap_or_else(|| "wallet".to_string())
                    .trim()
                    .to_string(),
            ),
            _ => None,
        }
    }

    fn append_l402_navigation(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        current_path: &str,
    ) {
        let Some(panel) = append_l402_panel(document, messages_container, "L402 Navigation") else {
            return;
        };
        let Ok(nav_row) = document.create_element("div") else {
            return;
        };
        let Ok(nav_row) = nav_row.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = nav_row.style().set_property("display", "flex");
        let _ = nav_row.style().set_property("flex-wrap", "wrap");
        let _ = nav_row.style().set_property("gap", "8px");
        let _ = nav_row.style().set_property("margin-top", "4px");

        for (href, label) in [
            ("/l402", "Wallet"),
            ("/l402/transactions", "Transactions"),
            ("/l402/paywalls", "Paywalls"),
            ("/l402/settlements", "Settlements"),
            ("/l402/deployments", "Deployments"),
        ] {
            let Ok(anchor) = document.create_element("a") else {
                continue;
            };
            let Ok(anchor) = anchor.dyn_into::<HtmlElement>() else {
                continue;
            };
            let _ = anchor.set_attribute("href", href);
            let is_active = current_path == href
                || (href == "/l402/transactions"
                    && current_path.starts_with("/l402/transactions/"));
            let _ = anchor.style().set_property("display", "inline-flex");
            let _ = anchor.style().set_property("align-items", "center");
            let _ = anchor.style().set_property("padding", "4px 9px");
            let _ = anchor.style().set_property("border-radius", "999px");
            let _ = anchor.style().set_property(
                "border",
                if is_active {
                    "1px solid #2563eb"
                } else {
                    "1px solid #1f2937"
                },
            );
            let _ = anchor
                .style()
                .set_property("background", if is_active { "#1d4ed8" } else { "#0f172a" });
            let _ = anchor
                .style()
                .set_property("color", if is_active { "#ffffff" } else { "#cbd5e1" });
            let _ = anchor.style().set_property("font-size", "12px");
            let _ = anchor.style().set_property("text-decoration", "none");
            anchor.set_inner_text(label);
            let _ = nav_row.append_child(&anchor);
        }

        let _ = panel.append_child(&nav_row);
    }

    fn render_l402_wallet_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        let summary = payload.get("summary").unwrap_or(&serde_json::Value::Null);
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Wallet Summary".to_string(),
                body: format!(
                    "attempts={} paid={} cached={} blocked={} failed={} totalPaidSats={} totalPaidMsats={}",
                    l402_value_text(summary.get("totalAttempts")),
                    l402_value_text(summary.get("paidCount")),
                    l402_value_text(summary.get("cachedCount")),
                    l402_value_text(summary.get("blockedCount")),
                    l402_value_text(summary.get("failedCount")),
                    l402_value_text(summary.get("totalPaidSats")),
                    l402_value_text(summary.get("totalPaidMsats"))
                ),
                tone: ManagementCardTone::Neutral,
            },
        );

        if let Some(last_paid) = payload.get("lastPaid").filter(|value| value.is_object()) {
            let event_id = l402_value_text(last_paid.get("eventId"));
            let host = l402_value_text(last_paid.get("host"));
            let status = l402_value_text(last_paid.get("status"));
            let amount = l402_primary_amount(last_paid);
            let scope = l402_value_text(last_paid.get("scope"));
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Last Paid".to_string(),
                    body: format!(
                        "eventId={event_id}\nhost={host}\nstatus={status}\namountSats={amount}\nscope={scope}"
                    ),
                    tone: ManagementCardTone::Info,
                },
            );
            append_l402_link_row(
                document,
                messages_container,
                "/l402/transactions/".to_string() + event_id.as_str(),
                "Open transaction detail",
                &format!("event #{event_id}"),
            );
        }

        if let Some(panel) = append_l402_panel(document, messages_container, "Recent L402 Attempts")
        {
            let rows = payload
                .get("recent")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            if rows.is_empty() {
                append_l402_panel_line(document, &panel, "No recent L402 attempts.", true, false);
            } else {
                for item in rows.iter().take(12) {
                    append_l402_transaction_link(document, &panel, item);
                }
            }
        }

        if let Some(panel) = append_l402_panel(document, messages_container, "Runtime Settings") {
            let settings = payload.get("settings").unwrap_or(&serde_json::Value::Null);
            append_l402_panel_line(
                document,
                &panel,
                &format!(
                    "invoicePayer={} credentialTtlSeconds={} paymentTimeoutMs={}",
                    l402_value_text(settings.get("invoicePayer")),
                    l402_value_text(settings.get("credentialTtlSeconds")),
                    l402_value_text(settings.get("paymentTimeoutMs"))
                ),
                true,
                false,
            );
            append_l402_panel_line(
                document,
                &panel,
                &format!(
                    "responseMaxBytes={} responsePreviewBytes={} allowlistHosts={}",
                    l402_value_text(settings.get("responseMaxBytes")),
                    l402_value_text(settings.get("responsePreviewBytes")),
                    l402_csv_text(settings.get("allowlistHosts")),
                ),
                true,
                false,
            );

            let spark_wallet = payload
                .get("sparkWallet")
                .unwrap_or(&serde_json::Value::Null);
            append_l402_panel_line(
                document,
                &panel,
                &format!(
                    "sparkWalletId={} sparkAddress={}",
                    l402_value_text(spark_wallet.get("walletId")),
                    l402_value_text(spark_wallet.get("sparkAddress"))
                ),
                true,
                false,
            );
        }
    }

    fn render_l402_transactions_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        let pagination = payload
            .get("pagination")
            .unwrap_or(&serde_json::Value::Null);
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Transactions Pagination".to_string(),
                body: format!(
                    "page={} lastPage={} perPage={} total={} hasMore={}",
                    l402_value_text(pagination.get("currentPage")),
                    l402_value_text(pagination.get("lastPage")),
                    l402_value_text(pagination.get("perPage")),
                    l402_value_text(pagination.get("total")),
                    l402_value_text(pagination.get("hasMorePages"))
                ),
                tone: ManagementCardTone::Neutral,
            },
        );

        let Some(panel) = append_l402_panel(document, messages_container, "Transactions Table")
        else {
            return;
        };
        let rows = payload
            .get("transactions")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            append_l402_panel_line(document, &panel, "No L402 transactions found.", true, false);
            return;
        }

        for tx in rows.iter().take(60) {
            append_l402_transaction_link(document, &panel, tx);
        }
    }

    fn render_l402_transaction_detail_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        let transaction = payload
            .get("transaction")
            .unwrap_or(&serde_json::Value::Null);
        if !transaction.is_object() {
            append_management_card(
                document,
                messages_container,
                ManagementCard {
                    title: "Transaction".to_string(),
                    body: "Transaction details are unavailable for this route.".to_string(),
                    tone: ManagementCardTone::Warning,
                },
            );
            return;
        }

        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Transaction Detail".to_string(),
                body: format!(
                    "eventId={} status={} host={} amountSats={} createdAt={}",
                    l402_value_text(transaction.get("eventId")),
                    l402_value_text(transaction.get("status")),
                    l402_value_text(transaction.get("host")),
                    l402_primary_amount(transaction),
                    l402_value_text(transaction.get("createdAt"))
                ),
                tone: ManagementCardTone::Info,
            },
        );
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Receipt Context".to_string(),
                body: format!(
                    "scope={} paid={} cacheStatus={} denyCode={} proofReference={}\nthreadId={} threadTitle={} runId={} runStatus={}",
                    l402_value_text(transaction.get("scope")),
                    l402_value_text(transaction.get("paid")),
                    l402_value_text(transaction.get("cacheStatus")),
                    l402_value_text(transaction.get("denyCode")),
                    l402_value_text(transaction.get("proofReference")),
                    l402_value_text(transaction.get("threadId")),
                    l402_value_text(transaction.get("threadTitle")),
                    l402_value_text(transaction.get("runId")),
                    l402_value_text(transaction.get("runStatus")),
                ),
                tone: ManagementCardTone::Neutral,
            },
        );
        append_l402_link_row(
            document,
            messages_container,
            "/l402/transactions".to_string(),
            "Back to transactions",
            "",
        );

        let thread_id = l402_value_text(transaction.get("threadId"));
        if thread_id != "-" {
            append_l402_link_row(
                document,
                messages_container,
                format!("/chat/{thread_id}"),
                "Open thread",
                &format!("thread {thread_id}"),
            );
        }

        if let Ok(raw_json) = serde_json::to_string_pretty(transaction) {
            if let Some(panel) =
                append_l402_panel(document, messages_container, "Raw Transaction JSON")
            {
                append_l402_pre(document, &panel, &raw_json);
            }
        }
    }

    fn render_l402_paywalls_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        let summary = payload.get("summary").unwrap_or(&serde_json::Value::Null);
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Paywalls Summary".to_string(),
                body: format!(
                    "uniqueTargets={} totalAttempts={} totalPaidCount={}",
                    l402_value_text(summary.get("uniqueTargets")),
                    l402_value_text(summary.get("totalAttempts")),
                    l402_value_text(summary.get("totalPaidCount"))
                ),
                tone: ManagementCardTone::Neutral,
            },
        );

        let Some(panel) = append_l402_panel(document, messages_container, "Paywall Targets") else {
            return;
        };
        let rows = payload
            .get("paywalls")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            append_l402_panel_line(
                document,
                &panel,
                "No paywall targets observed yet.",
                true,
                false,
            );
            return;
        }

        for row in rows.iter().take(100) {
            let line = format!(
                "{} {} attempts={} paid={} cached={} blocked={} failed={} totalPaidSats={} lastStatus={} lastAttemptAt={}",
                l402_value_text(row.get("host")),
                l402_value_text(row.get("scope")),
                l402_value_text(row.get("attempts")),
                l402_value_text(row.get("paid")),
                l402_value_text(row.get("cached")),
                l402_value_text(row.get("blocked")),
                l402_value_text(row.get("failed")),
                l402_value_text(row.get("totalPaidSats")),
                l402_value_text(row.get("lastStatus")),
                l402_value_text(row.get("lastAttemptAt")),
            );
            append_l402_panel_line(document, &panel, &line, true, true);
        }
    }

    fn render_l402_settlements_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        let summary = payload.get("summary").unwrap_or(&serde_json::Value::Null);
        append_management_card(
            document,
            messages_container,
            ManagementCard {
                title: "Settlements Summary".to_string(),
                body: format!(
                    "settledCount={} totalSats={} totalMsats={} latestSettlementAt={}",
                    l402_value_text(summary.get("settledCount")),
                    l402_value_text(summary.get("totalSats")),
                    l402_value_text(summary.get("totalMsats")),
                    l402_value_text(summary.get("latestSettlementAt")),
                ),
                tone: ManagementCardTone::Neutral,
            },
        );

        if let Some(panel) =
            append_l402_panel(document, messages_container, "Daily Settlement Totals")
        {
            let rows = payload
                .get("daily")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            if rows.is_empty() {
                append_l402_panel_line(
                    document,
                    &panel,
                    "No settlement totals available.",
                    true,
                    false,
                );
            } else {
                for row in rows.iter().take(60) {
                    let line = format!(
                        "{} count={} totalSats={} totalMsats={}",
                        l402_value_text(row.get("date")),
                        l402_value_text(row.get("count")),
                        l402_value_text(row.get("totalSats")),
                        l402_value_text(row.get("totalMsats")),
                    );
                    append_l402_panel_line(document, &panel, &line, true, true);
                }
            }
        }

        if let Some(panel) = append_l402_panel(document, messages_container, "Recent Settlements") {
            let rows = payload
                .get("settlements")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            if rows.is_empty() {
                append_l402_panel_line(document, &panel, "No settled receipts yet.", true, false);
            } else {
                for row in rows.iter().take(80) {
                    append_l402_transaction_link(document, &panel, row);
                }
            }
        }
    }

    fn render_l402_deployments_view(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        payload: &serde_json::Value,
    ) {
        if let Some(panel) = append_l402_panel(document, messages_container, "Config Snapshot") {
            let config = payload
                .get("configSnapshot")
                .unwrap_or(&serde_json::Value::Null);
            append_l402_panel_line(
                document,
                &panel,
                &format!(
                    "invoicePayer={} credentialTtlSeconds={} paymentTimeoutMs={} allowlistHosts={} demoPresets={}",
                    l402_value_text(config.get("invoicePayer")),
                    l402_value_text(config.get("credentialTtlSeconds")),
                    l402_value_text(config.get("paymentTimeoutMs")),
                    l402_csv_text(config.get("allowlistHosts")),
                    l402_csv_text(config.get("demoPresets")),
                ),
                true,
                false,
            );
        }

        let Some(panel) = append_l402_panel(document, messages_container, "Deployment Events")
        else {
            return;
        };
        let rows = payload
            .get("deployments")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            append_l402_panel_line(
                document,
                &panel,
                "No deployment events captured for this account yet.",
                true,
                false,
            );
            return;
        }

        for row in rows.iter().take(40) {
            let event_line = format!(
                "#{} {} {}",
                l402_value_text(row.get("eventId")),
                l402_value_text(row.get("type")),
                l402_value_text(row.get("createdAt")),
            );
            append_l402_panel_line(document, &panel, &event_line, true, true);
            if let Ok(payload_json) =
                serde_json::to_string(row.get("payload").unwrap_or(&serde_json::Value::Null))
            {
                append_l402_panel_line(document, &panel, &payload_json, true, true);
            }
        }
    }

    fn append_l402_transaction_link(
        document: &web_sys::Document,
        panel: &HtmlElement,
        transaction: &serde_json::Value,
    ) {
        let event_id = l402_value_text(transaction.get("eventId"));
        let host = l402_value_text(transaction.get("host"));
        let status = l402_value_text(transaction.get("status"));
        let amount = l402_primary_amount(transaction);
        let scope = l402_value_text(transaction.get("scope"));
        let created_at = l402_value_text(transaction.get("createdAt"));

        let Ok(anchor) = document.create_element("a") else {
            return;
        };
        let Ok(anchor) = anchor.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = anchor.set_attribute("href", &format!("/l402/transactions/{event_id}"));
        let _ = anchor.style().set_property("display", "flex");
        let _ = anchor
            .style()
            .set_property("justify-content", "space-between");
        let _ = anchor.style().set_property("gap", "10px");
        let _ = anchor.style().set_property("padding", "6px 8px");
        let _ = anchor.style().set_property("border-radius", "8px");
        let _ = anchor.style().set_property("border", "1px solid #1f2937");
        let _ = anchor.style().set_property("background", "#0b1220");
        let _ = anchor.style().set_property("color", "#dbeafe");
        let _ = anchor.style().set_property("font-size", "12px");
        let _ = anchor.style().set_property("line-height", "1.4");
        let _ = anchor.style().set_property("text-decoration", "none");

        let Ok(left) = document.create_element("div") else {
            return;
        };
        let Ok(left) = left.dyn_into::<HtmlElement>() else {
            return;
        };
        left.set_inner_text(&format!("{host} [{status}]  sats={amount}  scope={scope}"));
        let _ = anchor.append_child(&left);

        let Ok(right) = document.create_element("div") else {
            return;
        };
        let Ok(right) = right.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = right.style().set_property("color", "#94a3b8");
        right.set_inner_text(&format!("event={event_id}  {created_at}"));
        let _ = anchor.append_child(&right);
        let _ = panel.append_child(&anchor);
    }

    fn append_l402_link_row(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        href: String,
        label: &str,
        detail: &str,
    ) {
        let Some(panel) = append_l402_panel(document, messages_container, label) else {
            return;
        };
        let Ok(anchor) = document.create_element("a") else {
            return;
        };
        let Ok(anchor) = anchor.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = anchor.set_attribute("href", &href);
        let _ = anchor.style().set_property("color", "#93c5fd");
        let _ = anchor.style().set_property("font-size", "13px");
        let _ = anchor.style().set_property("text-decoration", "none");
        anchor.set_inner_text(href.as_str());
        let _ = panel.append_child(&anchor);
        if !detail.trim().is_empty() {
            append_l402_panel_line(document, &panel, detail, true, false);
        }
    }

    fn append_l402_panel(
        document: &web_sys::Document,
        messages_container: &HtmlElement,
        title: &str,
    ) -> Option<HtmlElement> {
        let row = document
            .create_element("div")
            .ok()?
            .dyn_into::<HtmlElement>()
            .ok()?;
        let _ = row.style().set_property("display", "flex");
        let _ = row.style().set_property("width", "100%");
        let _ = row.style().set_property("justify-content", "flex-start");

        let panel = document
            .create_element("div")
            .ok()?
            .dyn_into::<HtmlElement>()
            .ok()?;
        let _ = panel.style().set_property("max-width", "92%");
        let _ = panel.style().set_property("width", "100%");
        let _ = panel.style().set_property("padding", "10px 12px");
        let _ = panel.style().set_property("border-radius", "12px");
        let _ = panel.style().set_property("background", "#0b1220");
        let _ = panel.style().set_property("border", "1px solid #1f2937");
        let _ = panel.style().set_property("display", "flex");
        let _ = panel.style().set_property("flex-direction", "column");
        let _ = panel.style().set_property("gap", "6px");

        if !title.trim().is_empty() {
            let heading = document
                .create_element("div")
                .ok()?
                .dyn_into::<HtmlElement>()
                .ok()?;
            let _ = heading.style().set_property("font-size", "12px");
            let _ = heading.style().set_property("text-transform", "uppercase");
            let _ = heading.style().set_property("letter-spacing", "0.06em");
            let _ = heading.style().set_property("color", "#93c5fd");
            heading.set_inner_text(title);
            let _ = panel.append_child(&heading);
        }

        let _ = row.append_child(&panel);
        let _ = messages_container.append_child(&row);
        Some(panel)
    }

    fn append_l402_panel_line(
        document: &web_sys::Document,
        panel: &HtmlElement,
        text: &str,
        muted: bool,
        monospace: bool,
    ) {
        let Ok(line) = document.create_element("div") else {
            return;
        };
        let Ok(line) = line.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = line.style().set_property("font-size", "12px");
        let _ = line.style().set_property("line-height", "1.4");
        let _ = line.style().set_property("white-space", "pre-wrap");
        let _ = line
            .style()
            .set_property("color", if muted { "#cbd5e1" } else { "#e2e8f0" });
        if monospace {
            let _ = line.style().set_property(
                "font-family",
                "ui-monospace, SFMono-Regular, Menlo, monospace",
            );
        }
        line.set_inner_text(text);
        let _ = panel.append_child(&line);
    }

    fn append_l402_pre(document: &web_sys::Document, panel: &HtmlElement, text: &str) {
        let Ok(pre) = document.create_element("pre") else {
            return;
        };
        let Ok(pre) = pre.dyn_into::<HtmlElement>() else {
            return;
        };
        let _ = pre.style().set_property("margin", "0");
        let _ = pre.style().set_property("max-height", "220px");
        let _ = pre.style().set_property("overflow", "auto");
        let _ = pre.style().set_property("padding", "8px");
        let _ = pre.style().set_property("border-radius", "8px");
        let _ = pre.style().set_property("background", "#020617");
        let _ = pre.style().set_property("border", "1px solid #1f2937");
        let _ = pre.style().set_property("color", "#cbd5e1");
        let _ = pre.style().set_property("font-size", "11px");
        let _ = pre.style().set_property("line-height", "1.45");
        pre.set_inner_text(text);
        let _ = panel.append_child(&pre);
    }

    fn l402_primary_amount(transaction: &serde_json::Value) -> String {
        let amount = l402_value_text(transaction.get("amountSats"));
        if amount != "-" {
            return amount;
        }
        l402_value_text(transaction.get("quotedAmountSats"))
    }

    fn l402_csv_text(value: Option<&serde_json::Value>) -> String {
        let Some(value) = value else {
            return "-".to_string();
        };
        let Some(items) = value.as_array() else {
            return l402_value_text(Some(value));
        };
        if items.is_empty() {
            return "(none)".to_string();
        }
        items
            .iter()
            .map(|item| l402_value_text(Some(item)))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn l402_value_text(value: Option<&serde_json::Value>) -> String {
        match value {
            None | Some(serde_json::Value::Null) => "-".to_string(),
            Some(serde_json::Value::String(text)) => {
                if text.trim().is_empty() {
                    "-".to_string()
                } else {
                    text.clone()
                }
            }
            Some(serde_json::Value::Bool(value)) => value.to_string(),
            Some(serde_json::Value::Number(value)) => value.to_string(),
            Some(serde_json::Value::Array(values)) => {
                if values.is_empty() {
                    "[]".to_string()
                } else {
                    values
                        .iter()
                        .map(|item| l402_value_text(Some(item)))
                        .collect::<Vec<_>>()
                        .join("|")
                }
            }
            Some(other) => serde_json::to_string(other).unwrap_or_else(|_| "-".to_string()),
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

        if let AppRoute::Settings { section } = route {
            cards.push(ManagementCard {
                title: "Settings Surface".to_string(),
                body: format!(
                    "section: {}\nforms: profile + autopilot + integrations (Resend/Google)",
                    section.clone().unwrap_or_else(|| "profile".to_string())
                ),
                tone: ManagementCardTone::Info,
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
            AppRoute::Feed => "Feed".to_string(),
            AppRoute::Home => "Codex".to_string(),
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

}

#[cfg(target_arch = "wasm32")]
pub use wasm::boot_diagnostics_json;

#[cfg(not(target_arch = "wasm32"))]
pub fn boot_diagnostics_json() -> String {
    "{\"phase\":\"native\",\"detail\":\"web shell diagnostics only available on wasm\"}".to_string()
}
