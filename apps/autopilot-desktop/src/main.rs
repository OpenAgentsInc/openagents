use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use arboard::Clipboard;
use autopilot_app::{
    App as AutopilotApp, AppConfig, AppEvent, DvmHistorySnapshot, DvmProviderStatus, EventRecorder,
    MoltbookCommentSummary, MoltbookPostSummary, MoltbookProfileSummary, PylonStatus,
    RuntimeAuthStateView, SessionId, UserAction, WalletStatus,
};
use autopilot_core::guidance::{GuidanceMode, ensure_guidance_demo_lm};
use autopilot_ui::{
    MinimalRoot, ShortcutBinding, ShortcutChord, ShortcutCommand, ShortcutRegistry, ShortcutScope,
};
use chrono::Utc;
use clap::{Parser, Subcommand};
use codex_client::{
    AppServerClient, AppServerConfig, AskForApproval, ClientInfo, ReasoningEffort, SandboxMode,
    SandboxPolicy, ThreadListParams, ThreadReadParams, ThreadResumeParams, ThreadStartParams,
    TurnInterruptParams, TurnStartParams, UserInput,
};
use dsrs::signatures::{
    GuidanceDirectiveSignature, GuidanceRouterSignature, PlanningSignature,
    TaskUnderstandingSignature,
};
use dsrs::{Predict, Predictor, example};
use full_auto::{
    FullAutoAction, FullAutoDecision, FullAutoDecisionDiagnostics, FullAutoDecisionRequest,
    FullAutoDecisionResult, FullAutoState, FullAutoTurnSummary, decision_model, ensure_codex_lm,
    run_full_auto_decision,
};
use futures::{SinkExt, StreamExt};
use moltbook::{CommentSort, CreateCommentRequest, MoltbookClient, MoltbookError, PostSort};
use nostr::nip90::{JobInput, JobRequest, KIND_JOB_TEXT_GENERATION};
use nostr_client::dvm::DvmClient;
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use pylon::PylonConfig;
use pylon::db::{PylonDb, jobs::JobStatus};
use pylon::provider::{ProviderError, PylonProvider};
use reqwest::{Client as HttpClient, Method};
use runtime::UnifiedIdentity;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use wgpui::renderer::Renderer;
use wgpui::{
    Bounds, Component, Cursor, InputEvent, Key, Modifiers, MouseButton, NamedKey, PaintContext,
    Point, Scene, Size, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

mod full_auto;
mod inbox_domain;
mod runtime_auth;
mod runtime_codex_proto;

use inbox_domain::DesktopInboxState;
use runtime_auth::{
    DEFAULT_AUTH_BASE_URL, RuntimeSyncAuthFlow, RuntimeSyncAuthState, clear_runtime_auth_state,
    load_runtime_auth_state, login_with_email_code, persist_runtime_auth_state,
    runtime_auth_state_path,
};
use runtime_codex_proto::{
    ControlMethod, RuntimeCodexStreamEvent, build_error_receipt, build_khala_frame,
    extract_control_request,
    extract_desktop_handshake_ack_id,
    extract_ios_handshake_id, extract_ios_user_message, extract_runtime_events_from_khala_update,
    handshake_dedupe_key, khala_error_code, merge_retry_cursor, parse_khala_frame,
    request_dedupe_key, stream_event_seq,
};

const WINDOW_TITLE: &str = "Autopilot";
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 800.0;
const PADDING: f32 = 0.0;
const EVENT_BUFFER: usize = 256;
const DEFAULT_THREAD_MODEL: &str = "gpt-5.2-codex";
const ENV_GUIDANCE_GOAL: &str = "OPENAGENTS_GUIDANCE_GOAL";
const ENV_MOLTBOOK_PROXY_BASE: &str = "OPENAGENTS_MOLTBOOK_API_BASE";
const ENV_MOLTBOOK_LIVE_BASE: &str = "MOLTBOOK_API_BASE";
const DEFAULT_GUIDANCE_GOAL_INTENT: &str =
    "Keep making progress on the current task using the latest plan and diff.";
const DEFAULT_MOLTBOOK_PROXY_BASE: &str = "https://openagents.com/api/moltbook/api";
const DEFAULT_MOLTBOOK_LIVE_BASE: &str = "https://www.moltbook.com/api/v1";
const MOLTBOOK_CACHE_LIMIT: usize = 200;
const MOLTBOOK_CACHE_DB: &str = "moltbook.db";
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 2.5;
const ZOOM_STEP_KEY: f32 = 0.1;
const ZOOM_STEP_WHEEL: f32 = 0.05;
const HOTBAR_SLOT_MAX: u8 = 9;
const SHORTCUT_PRIORITY_APP: u8 = 100;
const SHORTCUT_PRIORITY_GLOBAL: u8 = 50;
const ENV_RUNTIME_SYNC_BASE_URL: &str = "OPENAGENTS_RUNTIME_SYNC_BASE_URL";
const ENV_RUNTIME_SYNC_TOKEN: &str = "OPENAGENTS_RUNTIME_SYNC_TOKEN";
const ENV_RUNTIME_SYNC_WORKSPACE_REF: &str = "OPENAGENTS_RUNTIME_SYNC_WORKSPACE_REF";
const ENV_RUNTIME_SYNC_CODEX_HOME_REF: &str = "OPENAGENTS_RUNTIME_SYNC_CODEX_HOME_REF";
const ENV_RUNTIME_SYNC_WORKER_PREFIX: &str = "OPENAGENTS_RUNTIME_SYNC_WORKER_PREFIX";
const ENV_RUNTIME_SYNC_HEARTBEAT_MS: &str = "OPENAGENTS_RUNTIME_SYNC_HEARTBEAT_MS";
const RUNTIME_SYNC_KHALA_TOPIC: &str = "runtime.codex_worker_events";
const RUNTIME_SYNC_KHALA_CHANNEL: &str = "sync:v1";
const RUNTIME_SYNC_KHALA_WS_VSN: &str = "2.0.0";
const RUNTIME_SYNC_KHALA_HEARTBEAT_MS: u64 = 20_000;
const RUNTIME_SYNC_STREAM_RECONNECT_SLEEP_MS: u64 = 2_000;
const RUNTIME_SYNC_CONTROL_PARSE_METHOD_FALLBACK: &str = "runtime/request";
const RUNTIME_SYNC_CONTROL_PARSE_REQUEST_ID_PREFIX: &str = "invalid-request-seq";
type RuntimeSyncWebSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Parser, Debug)]
#[command(name = "autopilot-desktop", about = "Autopilot desktop app")]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCommand {
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        code: Option<String>,
        #[arg(long, default_value = DEFAULT_AUTH_BASE_URL)]
        base_url: String,
    },
    Logout,
    Status,
}

#[derive(Clone)]
struct RuntimeCodexSync {
    client: HttpClient,
    base_url: Arc<tokio::sync::RwLock<String>>,
    token: Arc<tokio::sync::RwLock<Option<String>>>,
    base_url_locked_by_env: bool,
    token_locked_by_env: bool,
    workspace_ref: String,
    codex_home_ref: Option<String>,
    worker_prefix: String,
    heartbeat_interval_ms: u64,
    heartbeat_started: Arc<AtomicBool>,
    synced_workers: Arc<tokio::sync::Mutex<HashSet<String>>>,
    stream_workers: Arc<tokio::sync::Mutex<HashSet<String>>>,
    worker_sessions: Arc<tokio::sync::Mutex<HashMap<String, SessionId>>>,
    acked_handshakes: Arc<tokio::sync::Mutex<HashSet<String>>>,
    seen_ios_user_messages: Arc<tokio::sync::Mutex<HashSet<String>>>,
    seen_control_requests: Arc<tokio::sync::Mutex<HashSet<String>>>,
    remote_control_tx: tokio::sync::mpsc::Sender<RuntimeRemoteControlRequest>,
    action_tx: mpsc::Sender<UserAction>,
}

impl RuntimeCodexSync {
    fn from_env(
        cwd: &str,
        action_tx: mpsc::Sender<UserAction>,
        remote_control_tx: tokio::sync::mpsc::Sender<RuntimeRemoteControlRequest>,
    ) -> Option<Self> {
        let stored_auth = load_runtime_auth_state();
        let env_base_url = env::var(ENV_RUNTIME_SYNC_BASE_URL)
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let base_url = env_base_url.clone().or_else(|| {
            stored_auth
                .as_ref()
                .map(|value| value.base_url.trim().trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty())
        });
        let env_token = env::var(ENV_RUNTIME_SYNC_TOKEN)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let token = env_token.clone().or_else(|| {
            stored_auth
                .as_ref()
                .map(|value| value.token.trim().to_string())
                .filter(|value| !value.is_empty())
        });

        let workspace_ref = env::var(ENV_RUNTIME_SYNC_WORKSPACE_REF)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("desktop://{}", cwd));

        let codex_home_ref = env::var(ENV_RUNTIME_SYNC_CODEX_HOME_REF)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                resolve_codex_home().map(|path| format!("file://{}", path.to_string_lossy()))
            });

        let worker_prefix = env::var(ENV_RUNTIME_SYNC_WORKER_PREFIX)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "desktopw".to_string());

        let heartbeat_interval_ms = parse_positive_u64_env(ENV_RUNTIME_SYNC_HEARTBEAT_MS)
            .unwrap_or(30_000)
            .max(1_000);

        Some(Self {
            client: HttpClient::new(),
            base_url: Arc::new(tokio::sync::RwLock::new(
                base_url.unwrap_or_else(|| DEFAULT_AUTH_BASE_URL.to_string()),
            )),
            token: Arc::new(tokio::sync::RwLock::new(token)),
            base_url_locked_by_env: env_base_url.is_some(),
            token_locked_by_env: env_token.is_some(),
            workspace_ref,
            codex_home_ref,
            worker_prefix,
            heartbeat_interval_ms,
            heartbeat_started: Arc::new(AtomicBool::new(false)),
            synced_workers: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            stream_workers: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            worker_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            acked_handshakes: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            seen_ios_user_messages: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            seen_control_requests: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            remote_control_tx,
            action_tx,
        })
    }

    async fn refresh_auth_from_disk(&self) {
        if self.base_url_locked_by_env && self.token_locked_by_env {
            return;
        }

        if let Some(state) = load_runtime_auth_state() {
            self.apply_auth_state(&state).await;
            return;
        }

        if !self.token_locked_by_env {
            let mut token = self.token.write().await;
            *token = None;
        }
    }

    async fn auth_snapshot(&self) -> Result<(String, String), String> {
        self.refresh_auth_from_disk().await;
        let base_url = self.base_url.read().await.clone();
        let token = self
            .token
            .read()
            .await
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "runtime sync auth token missing; open Runtime Login pane (AU) and sign in"
                    .to_string()
            })?;

        Ok((base_url, token))
    }

    async fn current_base_url(&self) -> String {
        self.base_url.read().await.clone()
    }

    async fn apply_auth_state(&self, state: &RuntimeSyncAuthState) {
        if !self.base_url_locked_by_env {
            let mut base_url = self.base_url.write().await;
            *base_url = state.base_url.trim().trim_end_matches('/').to_string();
        }
        if !self.token_locked_by_env {
            let mut token = self.token.write().await;
            *token = Some(state.token.trim().to_string());
        }
    }

    async fn clear_auth_state(&self) {
        if !self.token_locked_by_env {
            let mut token = self.token.write().await;
            *token = None;
        }
        let mut synced = self.synced_workers.lock().await;
        synced.clear();
    }

    fn worker_id_for_thread(&self, _thread_id: &str) -> String {
        let scope = env::var("HOSTNAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "desktop".to_string());
        let safe_scope = sanitize_worker_component(&scope);
        let mut worker_id = format!("{}:{}:shared", self.worker_prefix, safe_scope);

        if worker_id.len() > 160 {
            worker_id.truncate(160);
        }

        if worker_id.len() < 3 {
            worker_id = format!("{}:{}", self.worker_prefix, Uuid::new_v4());
        }

        worker_id
    }

    async fn ensure_worker_for_thread(
        &self,
        thread_id: &str,
        session_id: Option<SessionId>,
    ) -> Result<String, String> {
        let worker_id = self.worker_id_for_thread(thread_id);
        let session_id_text = session_id.map(|value| value.to_string());

        if let Some(session_id) = session_id {
            self.set_worker_session_id(&worker_id, session_id).await;
        }

        let already_synced = {
            let guard = self.synced_workers.lock().await;
            guard.contains(&worker_id)
        };

        if already_synced {
            self.ensure_worker_stream(&worker_id).await;
            return Ok(worker_id);
        }

        let mut metadata = Map::new();
        metadata.insert("source".to_string(), json!("autopilot-desktop"));
        metadata.insert("sync_version".to_string(), json!("runtime_codex_v1"));
        metadata.insert("thread_id".to_string(), json!(thread_id));
        if let Some(session_id_text) = session_id_text {
            metadata.insert("session_id".to_string(), json!(session_id_text));
        }

        let mut payload = Map::new();
        payload.insert("worker_id".to_string(), json!(worker_id));
        payload.insert("workspace_ref".to_string(), json!(self.workspace_ref));
        payload.insert("adapter".to_string(), json!("desktop_bridge"));
        payload.insert("metadata".to_string(), Value::Object(metadata));

        if let Some(codex_home_ref) = self.codex_home_ref.clone() {
            payload.insert("codex_home_ref".to_string(), json!(codex_home_ref));
        }

        self.post_json("/api/runtime/codex/workers", Value::Object(payload))
            .await?;

        {
            let mut guard = self.synced_workers.lock().await;
            guard.insert(worker_id.clone());
        }

        self.ensure_worker_stream(&worker_id).await;

        Ok(worker_id)
    }

    fn start_heartbeat_loop(&self) {
        if self.heartbeat_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let sync = self.clone();
        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(Duration::from_millis(sync.heartbeat_interval_ms));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                ticker.tick().await;
                let worker_ids = {
                    let guard = sync.synced_workers.lock().await;
                    guard.iter().cloned().collect::<Vec<_>>()
                };

                for worker_id in worker_ids {
                    let payload = json!({
                        "source": "autopilot-desktop",
                        "kind": "periodic",
                        "occurred_at": Utc::now().to_rfc3339(),
                    });

                    if sync
                        .ingest_worker_event(&worker_id, "worker.heartbeat", payload)
                        .await
                        .is_err()
                    {
                        tracing::warn!(
                            worker_id = %worker_id,
                            "runtime sync heartbeat failed; worker will be re-synced on next activity"
                        );
                        let mut guard = sync.synced_workers.lock().await;
                        guard.remove(&worker_id);
                    }
                }
            }
        });
    }

    async fn ensure_worker_stream(&self, worker_id: &str) {
        let should_spawn = {
            let mut guard = self.stream_workers.lock().await;
            guard.insert(worker_id.to_string())
        };

        if !should_spawn {
            return;
        }

        let sync = self.clone();
        let worker_id = worker_id.to_string();
        tokio::spawn(async move {
            sync.run_worker_stream_loop(worker_id.clone()).await;
            let mut guard = sync.stream_workers.lock().await;
            guard.remove(&worker_id);
        });
    }

    async fn run_worker_stream_loop(&self, worker_id: String) {
        let mut cursor = match self.fetch_worker_latest_seq(&worker_id).await {
            Ok(latest_seq) => latest_seq,
            Err(error) => {
                tracing::warn!(worker_id = %worker_id, error = %error, "runtime sync khala bootstrap fallback to cursor=0");
                0
            }
        };

        tracing::info!(
            worker_id = %worker_id,
            cursor,
            "runtime sync khala stream started"
        );

        loop {
            match self.run_worker_khala_session(&worker_id, &mut cursor).await {
                Ok(()) => tracing::info!(
                    worker_id = %worker_id,
                    cursor,
                    "runtime sync khala stream session ended; reconnecting"
                ),
                Err(error) => tracing::warn!(
                    worker_id = %worker_id,
                    cursor,
                    error = %error,
                    "runtime sync khala stream failed; reconnecting"
                ),
            }

            tokio::time::sleep(Duration::from_millis(
                RUNTIME_SYNC_STREAM_RECONNECT_SLEEP_MS,
            ))
            .await;
        }
    }

    async fn run_worker_khala_session(
        &self,
        worker_id: &str,
        cursor: &mut u64,
    ) -> Result<(), String> {
        let khala_token = self
            .mint_khala_sync_token(vec![RUNTIME_SYNC_KHALA_TOPIC.to_string()])
            .await?;
        let websocket_url = self.build_khala_websocket_url(&khala_token).await?;
        let (mut socket, _response) = connect_async(websocket_url.as_str())
            .await
            .map_err(|error| error.to_string())?;

        let mut ref_counter: u64 = 0;
        let join_ref = Self::next_khala_ref(&mut ref_counter);
        let join_frame = build_khala_frame(
            None,
            Some(join_ref.as_str()),
            RUNTIME_SYNC_KHALA_CHANNEL,
            "phx_join",
            json!({}),
        );
        socket
            .send(Message::Text(join_frame))
            .await
            .map_err(|error| error.to_string())?;

        let _join_response = self
            .await_khala_reply(&mut socket, worker_id, cursor, join_ref.as_str())
            .await?;

        let subscribe_ref = Self::next_khala_ref(&mut ref_counter);
        let subscribe_frame = build_khala_frame(
            Some(join_ref.as_str()),
            Some(subscribe_ref.as_str()),
            RUNTIME_SYNC_KHALA_CHANNEL,
            "sync:subscribe",
            json!({
                "topics": [RUNTIME_SYNC_KHALA_TOPIC],
                "resume_after": {
                    RUNTIME_SYNC_KHALA_TOPIC: *cursor,
                },
                "replay_batch_size": 200,
            }),
        );
        socket
            .send(Message::Text(subscribe_frame))
            .await
            .map_err(|error| error.to_string())?;

        let _subscribe_response = self
            .await_khala_reply(&mut socket, worker_id, cursor, subscribe_ref.as_str())
            .await?;

        let mut heartbeat =
            tokio::time::interval(Duration::from_millis(RUNTIME_SYNC_KHALA_HEARTBEAT_MS));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let heartbeat_ref = Self::next_khala_ref(&mut ref_counter);
                    let heartbeat_frame = build_khala_frame(
                        Some(join_ref.as_str()),
                        Some(heartbeat_ref.as_str()),
                        RUNTIME_SYNC_KHALA_CHANNEL,
                        "sync:heartbeat",
                        json!({}),
                    );
                    socket
                        .send(Message::Text(heartbeat_frame))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                next_message = socket.next() => {
                    let Some(message) = next_message else {
                        return Err("khala websocket closed".to_string());
                    };
                    let message = message.map_err(|error| error.to_string())?;
                    self.process_khala_socket_message(worker_id, cursor, message).await?;
                }
            }
        }
    }

    async fn mint_khala_sync_token(&self, scopes: Vec<String>) -> Result<String, String> {
        let response = self
            .post_json("/api/khala/token", json!({ "scope": scopes }))
            .await?;
        response
            .get("data")
            .and_then(|value| value.get("token"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| "runtime sync khala token response missing token".to_string())
    }

    async fn build_khala_websocket_url(&self, token: &str) -> Result<String, String> {
        let base_url = self.base_url.read().await.clone();
        let mut url = reqwest::Url::parse(base_url.as_str()).map_err(|error| error.to_string())?;
        let ws_scheme = match url.scheme() {
            "https" => "wss",
            "http" => "ws",
            other => {
                return Err(format!(
                    "runtime sync base URL uses unsupported scheme for websocket: {other}"
                ));
            }
        };
        url.set_scheme(ws_scheme)
            .map_err(|_| "failed to set websocket URL scheme".to_string())?;
        url.set_path("/sync/socket/websocket");
        {
            let mut query = url.query_pairs_mut();
            query.clear();
            query.append_pair("token", token);
            query.append_pair("vsn", RUNTIME_SYNC_KHALA_WS_VSN);
        }
        Ok(url.to_string())
    }

    async fn await_khala_reply(
        &self,
        socket: &mut RuntimeSyncWebSocket,
        worker_id: &str,
        cursor: &mut u64,
        expected_reference: &str,
    ) -> Result<Value, String> {
        loop {
            let next_message = socket
                .next()
                .await
                .ok_or_else(|| "khala websocket closed while awaiting reply".to_string())?;
            let message = next_message.map_err(|error| error.to_string())?;
            let raw = Self::decode_websocket_text(message)?;
            let Some(frame) = parse_khala_frame(raw.as_str()) else {
                continue;
            };

            if frame.topic != RUNTIME_SYNC_KHALA_CHANNEL {
                continue;
            }

            if frame.event == "sync:update_batch" {
                self.process_khala_update_batch(worker_id, cursor, &frame.payload)
                    .await?;
                continue;
            }

            if frame.event == "sync:error" {
                if khala_error_code(&frame.payload).as_deref() == Some("stale_cursor") {
                    *cursor = 0;
                    return Err("khala stale_cursor; replay bootstrap required".to_string());
                }
                return Err(format!(
                    "khala sync error while awaiting reply: {}",
                    frame.payload
                ));
            }

            if frame.event != "phx_reply" || frame.reference.as_deref() != Some(expected_reference)
            {
                continue;
            }

            let payload_object = frame
                .payload
                .as_object()
                .ok_or_else(|| "khala phx_reply payload is not an object".to_string())?;
            let status = payload_object
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("error");
            if status != "ok" {
                let response = payload_object
                    .get("response")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let code = response
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                if code == "stale_cursor" {
                    *cursor = 0;
                }
                return Err(format!("khala phx_reply status={status} code={code}"));
            }

            return Ok(payload_object
                .get("response")
                .cloned()
                .unwrap_or_else(|| json!({})));
        }
    }

    async fn process_khala_socket_message(
        &self,
        worker_id: &str,
        cursor: &mut u64,
        message: Message,
    ) -> Result<(), String> {
        let raw = Self::decode_websocket_text(message)?;
        let Some(frame) = parse_khala_frame(raw.as_str()) else {
            return Ok(());
        };
        if frame.topic != RUNTIME_SYNC_KHALA_CHANNEL {
            return Ok(());
        }

        match frame.event.as_str() {
            "sync:update_batch" => {
                self.process_khala_update_batch(worker_id, cursor, &frame.payload)
                    .await
            }
            "sync:error" => {
                if khala_error_code(&frame.payload).as_deref() == Some("stale_cursor") {
                    *cursor = 0;
                    return Err("khala stale_cursor; resetting replay cursor".to_string());
                }
                Err(format!("khala sync error: {}", frame.payload))
            }
            "phx_error" | "phx_close" => Err(format!("khala channel error: {}", frame.event)),
            _ => Ok(()),
        }
    }

    async fn process_khala_update_batch(
        &self,
        worker_id: &str,
        cursor: &mut u64,
        payload: &Value,
    ) -> Result<(), String> {
        let events =
            extract_runtime_events_from_khala_update(payload, RUNTIME_SYNC_KHALA_TOPIC, worker_id);
        if events.is_empty() {
            return Ok(());
        }

        let mut next_cursor = *cursor;
        let mut retry_cursor: Option<u64> = None;

        for event in events {
            let seq = stream_event_seq(&event);
            if let Some(seq_value) = seq {
                next_cursor = next_cursor.max(seq_value);
            }

            if let Err(error) = self.handle_worker_stream_event(worker_id, &event).await {
                tracing::warn!(
                    worker_id = %worker_id,
                    seq = ?seq,
                    error = %error,
                    "runtime sync khala event processing failed"
                );

                if let Some(seq_value) = seq {
                    retry_cursor = Some(merge_retry_cursor(retry_cursor, seq_value));
                }
            }
        }

        if let Some(replay_cursor) = retry_cursor {
            tracing::info!(
                worker_id = %worker_id,
                cursor = replay_cursor,
                "runtime sync khala rewinding cursor to retry handshake processing"
            );
            *cursor = replay_cursor;
        } else {
            *cursor = next_cursor;
        }

        Ok(())
    }

    fn decode_websocket_text(message: Message) -> Result<String, String> {
        match message {
            Message::Text(text) => Ok(text.to_string()),
            Message::Binary(bytes) => {
                String::from_utf8(bytes.to_vec()).map_err(|err| err.to_string())
            }
            Message::Close(frame) => {
                let reason = frame
                    .map(|close| close.reason.to_string())
                    .unwrap_or_else(|| "no close reason".to_string());
                Err(format!("khala websocket closed: {reason}"))
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Ok(String::new()),
        }
    }

    fn next_khala_ref(counter: &mut u64) -> String {
        *counter = counter.saturating_add(1);
        counter.to_string()
    }

    async fn ingest_notification(
        &self,
        thread_id: &str,
        method: &str,
        params: Option<&Value>,
    ) -> Result<(), String> {
        let worker_id = self.ensure_worker_for_thread(thread_id, None).await?;
        let (event_type, payload) = runtime_event_from_notification(method, params);

        self.ingest_worker_event(&worker_id, &event_type, payload)
            .await
    }

    async fn handle_worker_stream_event(
        &self,
        worker_id: &str,
        event: &RuntimeCodexStreamEvent,
    ) -> Result<(), String> {
        if let Some(handshake_id) = extract_desktop_handshake_ack_id(&event.payload) {
            let inserted = self.mark_handshake_acked(worker_id, &handshake_id).await;
            if inserted {
                tracing::info!(
                    worker_id = %worker_id,
                    handshake_id = %handshake_id,
                    "runtime sync observed desktop handshake ack"
                );
            }
            return Ok(());
        }

        if let Some(incoming) = extract_ios_user_message(&event.payload) {
            let dedupe_key = request_dedupe_key(worker_id, &incoming.message_id);
            if self.is_ios_user_message_seen(&dedupe_key).await {
                tracing::debug!(
                    worker_id = %worker_id,
                    message_id = %incoming.message_id,
                    "runtime sync skipped duplicate ios user message"
                );
                return Ok(());
            }

            let Some(session_id) = self.resolve_worker_session_id(worker_id).await else {
                return Err(format!(
                    "runtime sync worker {worker_id} missing session mapping for ios message {}",
                    incoming.message_id
                ));
            };

            let action = UserAction::Message {
                session_id,
                text: incoming.text.clone(),
                model: incoming.model.clone(),
                reasoning: incoming.reasoning.clone(),
            };

            self.action_tx.send(action).map_err(|err| {
                format!(
                    "runtime sync failed to dispatch ios message {}: {}",
                    incoming.message_id, err
                )
            })?;

            self.mark_ios_user_message_seen(dedupe_key).await;
            tracing::info!(
                worker_id = %worker_id,
                message_id = %incoming.message_id,
                "runtime sync dispatched ios user message to desktop session"
            );
            return Ok(());
        }

        match extract_control_request(&event.payload) {
            Ok(Some(request)) => {
                let dedupe_key = request_dedupe_key(worker_id, &request.request_id);
                if self.is_control_request_seen(&dedupe_key).await {
                    tracing::debug!(
                        worker_id = %worker_id,
                        request_id = %request.request_id,
                        method = request.method.as_str(),
                        "runtime sync skipped duplicate remote control request"
                    );
                    return Ok(());
                }

                self.mark_control_request_seen(dedupe_key).await;
                let Some(session_id) = self.resolve_worker_session_id(worker_id).await else {
                    self.emit_control_error_receipt(
                        worker_id,
                        &request.request_id,
                        request.method.as_str(),
                        "worker_unavailable",
                        "desktop session mapping unavailable",
                        Some(json!({
                            "phase": "dispatch",
                            "reason": "missing_session_mapping",
                        })),
                    )
                    .await?;
                    return Ok(());
                };

                let dispatch = RuntimeRemoteControlRequest {
                    worker_id: worker_id.to_string(),
                    request_id: request.request_id.clone(),
                    method: request.method,
                    params: request.params.clone(),
                    session_id,
                    thread_id: request.thread_id.clone(),
                };

                self.remote_control_tx.send(dispatch).await.map_err(|err| {
                    format!(
                        "runtime sync failed to enqueue remote control request {}: {}",
                        request.request_id, err
                    )
                })?;

                tracing::info!(
                    worker_id = %worker_id,
                    request_id = %request.request_id,
                    method = request.method.as_str(),
                    "runtime sync queued remote control request"
                );
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                let request_id = error.request_id.unwrap_or_else(|| {
                    stream_event_seq(event)
                        .map(|seq| format!("{RUNTIME_SYNC_CONTROL_PARSE_REQUEST_ID_PREFIX}-{seq}"))
                        .unwrap_or_else(|| {
                            format!("{RUNTIME_SYNC_CONTROL_PARSE_REQUEST_ID_PREFIX}-unknown")
                        })
                });
                let method = error
                    .method
                    .unwrap_or_else(|| RUNTIME_SYNC_CONTROL_PARSE_METHOD_FALLBACK.to_string());
                self.emit_control_error_receipt(
                    worker_id,
                    &request_id,
                    &method,
                    error.code,
                    &error.message,
                    Some(json!({
                        "phase": "parse",
                        "worker_id": worker_id,
                        "seq": stream_event_seq(event),
                    })),
                )
                .await?;
                tracing::warn!(
                    worker_id = %worker_id,
                    request_id = %request_id,
                    method = %method,
                    code = error.code,
                    message = %error.message,
                    "runtime sync rejected malformed remote control request"
                );
                return Ok(());
            }
        }

        let Some(handshake_id) = extract_ios_handshake_id(&event.payload) else {
            return Ok(());
        };

        if self.is_handshake_acked(worker_id, &handshake_id).await {
            tracing::info!(
                worker_id = %worker_id,
                handshake_id = %handshake_id,
                "runtime sync skipped duplicate handshake ack"
            );
            return Ok(());
        }

        let desktop_session_id = self
            .desktop_session_id(worker_id)
            .await
            .unwrap_or_else(|| format!("worker:{worker_id}"));

        let ack_payload = json!({
            "source": "autopilot-desktop",
            "method": "desktop/handshake_ack",
            "handshake_id": handshake_id.clone(),
            "desktop_session_id": desktop_session_id,
            "occurred_at": Utc::now().to_rfc3339(),
        });

        self.ingest_worker_event(worker_id, "worker.event", ack_payload)
            .await?;

        self.mark_handshake_acked(worker_id, &handshake_id).await;
        tracing::info!(
            worker_id = %worker_id,
            handshake_id = %handshake_id,
            "runtime sync emitted desktop handshake ack"
        );

        Ok(())
    }

    async fn ingest_worker_event(
        &self,
        worker_id: &str,
        event_type: &str,
        payload: Value,
    ) -> Result<(), String> {
        let body = json!({
            "event": {
                "event_type": event_type,
                "payload": payload,
            }
        });
        let path = format!("/api/runtime/codex/workers/{worker_id}/events");
        self.post_json(&path, body).await.map(|_| ())
    }

    async fn emit_control_error_receipt(
        &self,
        worker_id: &str,
        request_id: &str,
        method: &str,
        code: &str,
        message: &str,
        details: Option<Value>,
    ) -> Result<(), String> {
        let receipt = build_error_receipt(
            request_id,
            method,
            code,
            message,
            false,
            details,
            &Utc::now().to_rfc3339(),
        );
        self.ingest_worker_event(worker_id, &receipt.event_type, receipt.payload)
            .await
    }

    async fn fetch_worker_latest_seq(&self, worker_id: &str) -> Result<u64, String> {
        let path = format!("/api/runtime/codex/workers/{worker_id}");
        let response = self.get_json(&path).await?;
        response
            .get("data")
            .and_then(|data| data.get("latest_seq"))
            .and_then(|latest_seq| latest_seq.as_u64())
            .ok_or_else(|| "runtime sync worker snapshot missing latest_seq".to_string())
    }

    async fn set_worker_session_id(&self, worker_id: &str, session_id: SessionId) {
        let mut guard = self.worker_sessions.lock().await;
        guard.insert(worker_id.to_string(), session_id);
    }

    async fn desktop_session_id(&self, worker_id: &str) -> Option<String> {
        let guard = self.worker_sessions.lock().await;
        guard.get(worker_id).map(|value| value.to_string())
    }

    async fn is_handshake_acked(&self, worker_id: &str, handshake_id: &str) -> bool {
        let key = handshake_dedupe_key(worker_id, handshake_id);
        let guard = self.acked_handshakes.lock().await;
        guard.contains(&key)
    }

    async fn mark_handshake_acked(&self, worker_id: &str, handshake_id: &str) -> bool {
        let key = handshake_dedupe_key(worker_id, handshake_id);
        let mut guard = self.acked_handshakes.lock().await;
        guard.insert(key)
    }

    async fn is_ios_user_message_seen(&self, dedupe_key: &str) -> bool {
        let guard = self.seen_ios_user_messages.lock().await;
        guard.contains(dedupe_key)
    }

    async fn mark_ios_user_message_seen(&self, dedupe_key: String) {
        let mut guard = self.seen_ios_user_messages.lock().await;
        guard.insert(dedupe_key);
    }

    async fn is_control_request_seen(&self, dedupe_key: &str) -> bool {
        let guard = self.seen_control_requests.lock().await;
        guard.contains(dedupe_key)
    }

    async fn mark_control_request_seen(&self, dedupe_key: String) {
        let mut guard = self.seen_control_requests.lock().await;
        guard.insert(dedupe_key);
    }

    async fn resolve_worker_session_id(&self, worker_id: &str) -> Option<SessionId> {
        if let Some(session_id) = self.session_id_from_map(worker_id).await {
            return Some(session_id);
        }

        let path = format!("/api/runtime/codex/workers/{worker_id}");
        let response = self.get_json(&path).await.ok()?;
        let raw = response
            .get("data")
            .and_then(|data| data.get("metadata"))
            .and_then(|metadata| metadata.get("session_id"))
            .and_then(|value| value.as_str())?;
        let session_id = parse_session_id(raw)?;
        self.set_worker_session_id(worker_id, session_id).await;
        Some(session_id)
    }

    async fn session_id_from_map(&self, worker_id: &str) -> Option<SessionId> {
        let guard = self.worker_sessions.lock().await;
        guard.get(worker_id).copied()
    }

    async fn get_json(&self, path: &str) -> Result<Value, String> {
        let response = self
            .request_builder(Method::GET, path)
            .await?
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|err| err.to_string())?;

        self.decode_json_response(response).await
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value, String> {
        let response = self
            .request_builder(Method::POST, path)
            .await?
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|err| err.to_string())?;

        self.decode_json_response(response).await
    }

    async fn request_builder(
        &self,
        method: Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, String> {
        let (base_url, token) = self.auth_snapshot().await?;
        Ok(self
            .client
            .request(method, format!("{base_url}{path}"))
            .bearer_auth(token)
            .header("x-request-id", Self::request_id()))
    }

    async fn decode_json_response(&self, response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        let text = response.text().await.map_err(|err| err.to_string())?;

        if !status.is_success() {
            return Err(runtime_sync_error_message(status, &text));
        }

        Ok(serde_json::from_str::<Value>(&text).unwrap_or(Value::Null))
    }

    fn request_id() -> String {
        format!("desktopreq-{}", Uuid::new_v4().to_string().to_lowercase())
    }
}

fn runtime_sync_error_message(status: reqwest::StatusCode, raw_body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(raw_body).unwrap_or(Value::Null);
    parsed
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|message| message.as_str())
        .map(|value| value.to_string())
        .or_else(|| (!raw_body.trim().is_empty()).then_some(raw_body.to_string()))
        .unwrap_or_else(|| format!("runtime sync request failed ({status})"))
}

fn sanitize_worker_component(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if out.is_empty() {
        out = "thread".to_string();
    }

    out
}

fn parse_session_id(raw: &str) -> Option<SessionId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    serde_json::from_value::<SessionId>(Value::String(trimmed.to_string())).ok()
}

fn resolve_codex_home() -> Option<PathBuf> {
    if let Ok(raw) = env::var("CODEX_HOME") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    env::var("HOME")
        .ok()
        .map(|home| Path::new(&home).join(".codex"))
}

fn parse_positive_u64_env(name: &str) -> Option<u64> {
    let raw = env::var(name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed.parse::<u64>().ok().filter(|value| *value > 0)
}

fn runtime_event_from_notification(method: &str, params: Option<&Value>) -> (String, Value) {
    let mut payload = Map::new();
    payload.insert("source".to_string(), json!("autopilot-desktop"));
    payload.insert("method".to_string(), json!(method));
    payload.insert(
        "params".to_string(),
        params.cloned().unwrap_or_else(|| json!({})),
    );
    payload.insert("occurred_at".to_string(), json!(Utc::now().to_rfc3339()));

    let event_type = if method == "thread/started" {
        payload.insert("status".to_string(), json!("running"));
        "worker.started"
    } else if method == "thread/stopped" || method == "thread/completed" {
        payload.insert("status".to_string(), json!("stopped"));
        "worker.stopped"
    } else if method.ends_with("/error") || method == "codex/error" {
        "worker.error"
    } else if method.ends_with("/heartbeat") {
        "worker.heartbeat"
    } else {
        "worker.event"
    };

    (event_type.to_string(), Value::Object(payload))
}

fn should_sync_runtime_notification(method: &str) -> bool {
    !method.trim().is_empty()
}

async fn runtime_auth_state_view(
    runtime_sync: &Arc<Option<RuntimeCodexSync>>,
    pending_auth_flow: &Arc<tokio::sync::Mutex<Option<RuntimeSyncAuthFlow>>>,
    last_message: Option<String>,
    last_error: Option<String>,
) -> RuntimeAuthStateView {
    let stored = load_runtime_auth_state();

    let (base_url, token_present) = if let Some(sync) = runtime_sync.as_ref() {
        sync.refresh_auth_from_disk().await;
        let base_url = sync.current_base_url().await;
        let token_present = sync
            .token
            .read()
            .await
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        (Some(base_url), token_present)
    } else {
        let base_url = stored
            .as_ref()
            .map(|state| state.base_url.clone())
            .or_else(|| Some(DEFAULT_AUTH_BASE_URL.to_string()));
        let token_present = stored
            .as_ref()
            .map(|state| !state.token.trim().is_empty())
            .unwrap_or(false);
        (base_url, token_present)
    };

    let pending_email = {
        let pending = pending_auth_flow.lock().await;
        pending
            .as_ref()
            .and_then(|flow| flow.pending_email())
            .map(|value| value.to_string())
    };

    RuntimeAuthStateView {
        base_url,
        email: stored.as_ref().and_then(|state| state.email.clone()),
        user_id: stored.as_ref().and_then(|state| state.user_id.clone()),
        token_present,
        pending_email,
        last_message,
        last_error,
        updated_at: Some(Utc::now().to_rfc3339()),
    }
}

async fn emit_runtime_auth_state(
    proxy: &EventLoopProxy<AppEvent>,
    runtime_sync: &Arc<Option<RuntimeCodexSync>>,
    pending_auth_flow: &Arc<tokio::sync::Mutex<Option<RuntimeSyncAuthFlow>>>,
    last_message: Option<String>,
    last_error: Option<String>,
) {
    let state =
        runtime_auth_state_view(runtime_sync, pending_auth_flow, last_message, last_error).await;
    let _ = proxy.send_event(AppEvent::RuntimeAuthState { state });
}

fn parse_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_lowercase().as_str() {
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" | "x-high" => Some(ReasoningEffort::XHigh),
        "minimal" => Some(ReasoningEffort::Minimal),
        "none" => Some(ReasoningEffort::None),
        _ => None,
    }
}

fn remote_control_invalid(
    message: impl Into<String>,
    details: Option<Value>,
) -> RuntimeRemoteControlDispatchError {
    RuntimeRemoteControlDispatchError {
        code: "invalid_request",
        message: message.into(),
        details,
    }
}

fn remote_control_worker_unavailable(
    message: impl Into<String>,
    details: Option<Value>,
) -> RuntimeRemoteControlDispatchError {
    RuntimeRemoteControlDispatchError {
        code: "worker_unavailable",
        message: message.into(),
        details,
    }
}

fn remote_control_internal(
    message: impl Into<String>,
    details: Option<Value>,
) -> RuntimeRemoteControlDispatchError {
    RuntimeRemoteControlDispatchError {
        code: "internal_error",
        message: message.into(),
        details,
    }
}

fn control_param_string(params: &Value, keys: &[&str]) -> Option<String> {
    let object = params.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn control_param_bool(params: &Value, keys: &[&str]) -> Option<bool> {
    let object = params.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_bool) {
            return Some(value);
        }
    }
    None
}

fn control_param_u32(params: &Value, keys: &[&str], max: u32) -> Option<u32> {
    let object = params.as_object()?;
    for key in keys {
        if let Some(raw) = object.get(*key).and_then(Value::as_u64) {
            let capped = raw.min(u64::from(max));
            let converted = u32::try_from(capped).ok()?;
            if converted > 0 {
                return Some(converted);
            }
        }
    }
    None
}

fn to_json_value_lossy<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn decode_turn_start_input(
    params: &Value,
) -> Result<Vec<UserInput>, RuntimeRemoteControlDispatchError> {
    if let Some(input) = params.get("input") {
        let decoded = serde_json::from_value::<Vec<UserInput>>(input.clone()).map_err(|err| {
            remote_control_invalid(
                format!("turn/start input payload is invalid: {err}"),
                Some(json!({ "field": "input" })),
            )
        })?;

        if decoded.is_empty() {
            return Err(remote_control_invalid(
                "turn/start input payload must not be empty",
                Some(json!({ "field": "input" })),
            ));
        }

        return Ok(decoded);
    }

    if let Some(text) = control_param_string(params, &["text", "message"]) {
        return Ok(vec![UserInput::Text { text }]);
    }

    Err(remote_control_invalid(
        "turn/start requires `input` or `text`",
        Some(json!({ "field": "input" })),
    ))
}

async fn execute_runtime_remote_control_request(
    client: Arc<AppServerClient>,
    request: RuntimeRemoteControlRequest,
    cwd: String,
    runtime_sync: Arc<Option<RuntimeCodexSync>>,
    session_states: Arc<tokio::sync::Mutex<HashMap<SessionId, SessionRuntime>>>,
    thread_to_session: Arc<tokio::sync::Mutex<HashMap<String, SessionId>>>,
) -> Result<Value, RuntimeRemoteControlDispatchError> {
    let session_state = session_states
        .lock()
        .await
        .get(&request.session_id)
        .cloned()
        .ok_or_else(|| {
            remote_control_worker_unavailable(
                format!("session {} is not available", request.session_id),
                Some(json!({ "session_id": request.session_id.to_string() })),
            )
        })?;

    let requested_thread_id = request
        .thread_id
        .clone()
        .or_else(|| extract_thread_id(Some(&request.params)));
    let mapped_thread_id = session_state.thread_id.lock().await.clone();
    let resolved_thread_id = requested_thread_id.or(mapped_thread_id.clone());

    match request.method {
        ControlMethod::ThreadStart => {
            let model = control_param_string(&request.params, &["model"]);
            let model_provider = control_param_string(&request.params, &["model_provider"]);
            let response = client
                .thread_start(ThreadStartParams {
                    model,
                    model_provider,
                    cwd: Some(cwd.clone()),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: Some(SandboxMode::DangerFullAccess),
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("thread/start failed: {err}"),
                        Some(json!({ "method": "thread/start" })),
                    )
                })?;

            {
                let mut guard = session_state.thread_id.lock().await;
                *guard = Some(response.thread.id.clone());
            }
            {
                let mut guard = thread_to_session.lock().await;
                guard.insert(response.thread.id.clone(), request.session_id);
            }
            if let Some(sync) = runtime_sync.as_ref() {
                sync.ensure_worker_for_thread(&response.thread.id, Some(request.session_id))
                    .await
                    .map_err(|err| {
                        remote_control_internal(
                            format!("thread/start worker mapping sync failed: {err}"),
                            Some(json!({ "thread_id": response.thread.id })),
                        )
                    })?;
            }

            Ok(to_json_value_lossy(&response))
        }
        ControlMethod::ThreadResume => {
            let thread_id = resolved_thread_id.ok_or_else(|| {
                remote_control_invalid(
                    "thread/resume requires thread_id",
                    Some(json!({ "field": "thread_id" })),
                )
            })?;
            let model = control_param_string(&request.params, &["model"]);
            let model_provider = control_param_string(&request.params, &["model_provider"]);

            let response = client
                .thread_resume(ThreadResumeParams {
                    thread_id: thread_id.clone(),
                    model,
                    model_provider,
                    cwd: Some(cwd.clone()),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: Some(SandboxMode::DangerFullAccess),
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("thread/resume failed: {err}"),
                        Some(json!({ "method": "thread/resume", "thread_id": thread_id })),
                    )
                })?;

            {
                let mut guard = session_state.thread_id.lock().await;
                *guard = Some(response.thread.id.clone());
            }
            {
                let mut guard = thread_to_session.lock().await;
                guard.insert(response.thread.id.clone(), request.session_id);
            }
            if let Some(sync) = runtime_sync.as_ref() {
                sync.ensure_worker_for_thread(&response.thread.id, Some(request.session_id))
                    .await
                    .map_err(|err| {
                        remote_control_internal(
                            format!("thread/resume worker mapping sync failed: {err}"),
                            Some(json!({ "thread_id": response.thread.id })),
                        )
                    })?;
            }

            Ok(to_json_value_lossy(&response))
        }
        ControlMethod::TurnStart => {
            let thread_id = resolved_thread_id.ok_or_else(|| {
                remote_control_invalid(
                    "turn/start requires thread_id",
                    Some(json!({ "field": "thread_id" })),
                )
            })?;
            let input = decode_turn_start_input(&request.params)?;
            let model = control_param_string(&request.params, &["model"]);
            let effort = control_param_string(
                &request.params,
                &["effort", "reasoning", "reasoning_effort"],
            )
            .and_then(|value| parse_reasoning_effort(&value));

            let response = client
                .turn_start(TurnStartParams {
                    thread_id: thread_id.clone(),
                    input,
                    model,
                    effort,
                    summary: None,
                    approval_policy: Some(AskForApproval::Never),
                    sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                    cwd: Some(cwd.clone()),
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("turn/start failed: {err}"),
                        Some(json!({ "method": "turn/start", "thread_id": thread_id })),
                    )
                })?;

            {
                let mut thread_guard = session_state.thread_id.lock().await;
                *thread_guard = Some(thread_id.clone());
            }
            {
                let mut turn_guard = session_state.turn_id.lock().await;
                *turn_guard = Some(response.turn.id.clone());
            }

            Ok(to_json_value_lossy(&response))
        }
        ControlMethod::TurnInterrupt => {
            let thread_id = resolved_thread_id.ok_or_else(|| {
                remote_control_invalid(
                    "turn/interrupt requires thread_id",
                    Some(json!({ "field": "thread_id" })),
                )
            })?;
            let requested_turn_id = control_param_string(&request.params, &["turn_id", "turnId"]);
            let mapped_turn_id = session_state.turn_id.lock().await.clone();
            let turn_id = requested_turn_id.or(mapped_turn_id).ok_or_else(|| {
                remote_control_invalid(
                    "turn/interrupt requires turn_id",
                    Some(json!({ "field": "turn_id" })),
                )
            })?;

            client
                .turn_interrupt(TurnInterruptParams {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("turn/interrupt failed: {err}"),
                        Some(
                            json!({
                                "method": "turn/interrupt",
                                "thread_id": thread_id,
                                "turn_id": turn_id,
                            }),
                        ),
                    )
                })?;

            {
                let mut guard = session_state.turn_id.lock().await;
                *guard = None;
            }

            Ok(json!({ "status": "interrupted", "turn_id": turn_id }))
        }
        ControlMethod::ThreadList => {
            let limit = control_param_u32(&request.params, &["limit"], 200);
            let cursor = control_param_string(&request.params, &["cursor"]);
            let model_provider = control_param_string(&request.params, &["model_provider"]);
            let model_providers = model_provider.map(|value| vec![value]);

            let response = client
                .thread_list(ThreadListParams {
                    cursor,
                    limit,
                    model_providers,
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("thread/list failed: {err}"),
                        Some(json!({ "method": "thread/list" })),
                    )
                })?;

            Ok(to_json_value_lossy(&response))
        }
        ControlMethod::ThreadRead => {
            let thread_id = resolved_thread_id.ok_or_else(|| {
                remote_control_invalid(
                    "thread/read requires thread_id",
                    Some(json!({ "field": "thread_id" })),
                )
            })?;
            let include_turns =
                control_param_bool(&request.params, &["include_turns", "includeTurns"])
                    .unwrap_or(false);

            let response = client
                .thread_read(ThreadReadParams {
                    thread_id: thread_id.clone(),
                    include_turns,
                })
                .await
                .map_err(|err| {
                    remote_control_internal(
                        format!("thread/read failed: {err}"),
                        Some(json!({ "method": "thread/read", "thread_id": thread_id })),
                    )
                })?;

            {
                let mut guard = session_state.thread_id.lock().await;
                *guard = Some(response.thread.id.clone());
            }
            {
                let mut guard = thread_to_session.lock().await;
                guard.insert(response.thread.id.clone(), request.session_id);
            }
            Ok(to_json_value_lossy(&response))
        }
    }
}

fn resolve_path(raw: &str, cwd: &str) -> PathBuf {
    let trimmed = raw.trim();
    let path_buf = if trimmed == "~" || trimmed.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            PathBuf::from(trimmed)
        } else if trimmed == "~" {
            PathBuf::from(home)
        } else {
            PathBuf::from(home).join(&trimmed[2..])
        }
    } else {
        PathBuf::from(trimmed)
    };

    if path_buf.is_absolute() {
        path_buf
    } else {
        PathBuf::from(cwd).join(path_buf)
    }
}

fn build_shortcut_registry() -> ShortcutRegistry {
    let mut registry = ShortcutRegistry::new();

    let app_modifiers = [
        Modifiers {
            meta: true,
            ctrl: false,
            alt: false,
            shift: false,
        },
        Modifiers {
            meta: false,
            ctrl: true,
            alt: false,
            shift: false,
        },
    ];

    for modifiers in app_modifiers {
        for slot in 0..=HOTBAR_SLOT_MAX {
            register_shortcut(
                &mut registry,
                ShortcutBinding {
                    id: "hotbar_slot",
                    chord: ShortcutChord::new(Key::Character(slot.to_string()), modifiers),
                    scope: ShortcutScope::App,
                    priority: SHORTCUT_PRIORITY_APP,
                    command: ShortcutCommand::HotbarSlot(slot),
                },
            );
        }

        register_shortcut(
            &mut registry,
            ShortcutBinding {
                id: "zoom_in",
                chord: ShortcutChord::new(Key::Character("+".to_string()), modifiers),
                scope: ShortcutScope::Global,
                priority: SHORTCUT_PRIORITY_GLOBAL,
                command: ShortcutCommand::ZoomIn,
            },
        );
        register_shortcut(
            &mut registry,
            ShortcutBinding {
                id: "zoom_in_eq",
                chord: ShortcutChord::new(Key::Character("=".to_string()), modifiers),
                scope: ShortcutScope::Global,
                priority: SHORTCUT_PRIORITY_GLOBAL,
                command: ShortcutCommand::ZoomIn,
            },
        );
        register_shortcut(
            &mut registry,
            ShortcutBinding {
                id: "zoom_out",
                chord: ShortcutChord::new(Key::Character("-".to_string()), modifiers),
                scope: ShortcutScope::Global,
                priority: SHORTCUT_PRIORITY_GLOBAL,
                command: ShortcutCommand::ZoomOut,
            },
        );
        register_shortcut(
            &mut registry,
            ShortcutBinding {
                id: "zoom_reset",
                chord: ShortcutChord::new(Key::Character("0".to_string()), modifiers),
                scope: ShortcutScope::Global,
                priority: SHORTCUT_PRIORITY_GLOBAL,
                command: ShortcutCommand::ZoomReset,
            },
        );
    }

    register_shortcut(
        &mut registry,
        ShortcutBinding {
            id: "close_active_pane",
            chord: ShortcutChord::new(Key::Named(NamedKey::Escape), Modifiers::default()),
            scope: ShortcutScope::App,
            priority: SHORTCUT_PRIORITY_APP,
            command: ShortcutCommand::CloseActivePane,
        },
    );
    register_shortcut(
        &mut registry,
        ShortcutBinding {
            id: "cycle_chat_focus",
            chord: ShortcutChord::new(Key::Named(NamedKey::Tab), Modifiers::default()),
            scope: ShortcutScope::App,
            priority: SHORTCUT_PRIORITY_APP,
            command: ShortcutCommand::CycleChatFocus,
        },
    );
    register_shortcut(
        &mut registry,
        ShortcutBinding {
            id: "cycle_chat_model",
            chord: ShortcutChord::new(
                Key::Named(NamedKey::Tab),
                Modifiers {
                    shift: true,
                    ctrl: false,
                    alt: false,
                    meta: false,
                },
            ),
            scope: ShortcutScope::App,
            priority: SHORTCUT_PRIORITY_APP,
            command: ShortcutCommand::CycleChatModel,
        },
    );

    registry
}

fn register_shortcut(registry: &mut ShortcutRegistry, binding: ShortcutBinding) {
    let conflicts = registry.register(binding);
    for conflict in conflicts {
        tracing::warn!(
            chord = ?conflict.chord,
            existing = ?conflict.existing.command,
            incoming = ?conflict.incoming.command,
            "shortcut conflict detected"
        );
    }
}

#[derive(Clone)]
struct SessionRuntime {
    thread_id: Arc<tokio::sync::Mutex<Option<String>>>,
    turn_id: Arc<tokio::sync::Mutex<Option<String>>>,
    pending_interrupt: Arc<AtomicBool>,
}

impl SessionRuntime {
    fn new() -> Self {
        Self {
            thread_id: Arc::new(tokio::sync::Mutex::new(None)),
            turn_id: Arc::new(tokio::sync::Mutex::new(None)),
            pending_interrupt: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone)]
struct MoltbookReplyTarget {
    post_id: String,
}

#[derive(Debug, Clone)]
struct RuntimeRemoteControlRequest {
    worker_id: String,
    request_id: String,
    method: ControlMethod,
    params: Value,
    session_id: SessionId,
    thread_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeRemoteControlDispatchError {
    code: &'static str,
    message: String,
    details: Option<Value>,
}

struct InProcessPylon {
    provider: Option<PylonProvider>,
    started_at: Option<std::time::Instant>,
    last_error: Option<String>,
}

impl InProcessPylon {
    fn new() -> Self {
        Self {
            provider: None,
            started_at: None,
            last_error: None,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();
    let _ = rustls::crypto::ring::default_provider().install_default();
    inbox_domain::warm_inbox_domain_bridge();

    if let Some(command) = cli.command {
        return run_cli_command(command);
    }

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .context("failed to create event loop")?;
    let proxy = event_loop.create_proxy();
    let (action_tx, action_rx) = mpsc::channel();
    spawn_event_bridge(proxy, action_tx.clone(), action_rx);
    let mut app = App::new(action_tx);
    event_loop.run_app(&mut app).context("event loop failed")?;
    Ok(())
}

fn run_cli_command(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::Auth { command } => run_auth_command(command),
    }
}

fn run_auth_command(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login {
            email,
            code,
            base_url,
        } => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("failed to start auth runtime")?;

            let auth_state = runtime
                .block_on(login_with_email_code(&base_url, &email, code))
                .map_err(|err| anyhow::anyhow!(err))
                .context("desktop auth login failed")?;

            let path = persist_runtime_auth_state(&auth_state)
                .map_err(|err| anyhow::anyhow!(err))
                .context("failed to persist desktop auth token")?;

            println!(
                "Desktop auth login succeeded for {}. Token saved at {}",
                auth_state.email.as_deref().unwrap_or("<unknown>"),
                path.display()
            );
            println!("Runtime sync will now authenticate as this user by default.");
            Ok(())
        }
        AuthCommand::Logout => {
            clear_runtime_auth_state()
                .map_err(|err| anyhow::anyhow!(err))
                .context("failed to clear desktop auth token")?;
            println!("Desktop auth token cleared.");
            Ok(())
        }
        AuthCommand::Status => {
            if let Some(state) = load_runtime_auth_state() {
                let path = runtime_auth_state_path()
                    .map(|value| value.display().to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                println!(
                    "Desktop auth is configured.\n  base_url: {}\n  email: {}\n  user_id: {}\n  issued_at: {}\n  state_path: {}",
                    state.base_url,
                    state.email.unwrap_or_else(|| "<unknown>".to_string()),
                    state.user_id.unwrap_or_else(|| "<unknown>".to_string()),
                    state.issued_at,
                    path
                );
            } else {
                println!(
                    "Desktop auth is not configured. Run `autopilot-desktop auth login --email <you@domain>`."
                );
            }
            Ok(())
        }
    }
}

struct App {
    state: Option<RenderState>,
    pending_events: Vec<AppEvent>,
    action_tx: mpsc::Sender<UserAction>,
    cursor_position: Point,
    modifiers: ModifiersState,
}

impl App {
    fn new(action_tx: mpsc::Sender<UserAction>) -> Self {
        Self {
            state: None,
            pending_events: Vec::new(),
            action_tx,
            cursor_position: Point::ZERO,
            modifiers: ModifiersState::default(),
        }
    }
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    scale_factor: f32,
    zoom_factor: f32,
    root: MinimalRoot,
    shortcuts: ShortcutRegistry,
    cursor_icon: Cursor,
}

impl RenderState {
    fn effective_scale(&self) -> f32 {
        (self.scale_factor * self.zoom_factor).max(0.1)
    }

    fn bump_zoom(&mut self, delta: f32) {
        let next = (self.zoom_factor + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        if (next - self.zoom_factor).abs() > f32::EPSILON {
            self.zoom_factor = next;
            self.text_system.set_scale_factor(self.effective_scale());
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        let next = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        if (next - self.zoom_factor).abs() > f32::EPSILON {
            self.zoom_factor = next;
            self.text_system.set_scale_factor(self.effective_scale());
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let action_tx = self.action_tx.clone();
        match init_state(event_loop, action_tx) {
            Ok(mut state) => {
                for event in self.pending_events.drain(..) {
                    state.root.apply_event(event);
                }
                update_cursor(&mut state);
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to initialize WGPUI window");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor as f32;
                state.text_system.set_scale_factor(state.effective_scale());
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = state.effective_scale();
                self.cursor_position =
                    Point::new(position.x as f32 / scale, position.y as f32 / scale);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button,
                ..
            } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let modifiers = to_modifiers(self.modifiers);
                let input_event = match mouse_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                        modifiers,
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                };

                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy, zoom_dir) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0, y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let scale = state.effective_scale();
                        (-pos.x as f32 / scale, -pos.y as f32 / scale, pos.y as f32)
                    }
                };

                let modifiers = to_modifiers(self.modifiers);
                let mut handled = false;

                if modifiers.meta {
                    let step = if zoom_dir > 0.0 {
                        ZOOM_STEP_WHEEL
                    } else if zoom_dir < 0.0 {
                        -ZOOM_STEP_WHEEL
                    } else {
                        0.0
                    };
                    if step != 0.0 {
                        state.bump_zoom(step);
                        state.window.request_redraw();
                        handled = true;
                    }
                }

                if !handled {
                    let input_event = InputEvent::Scroll { dx, dy };
                    let bounds =
                        content_bounds(logical_size(&state.config, state.effective_scale()));
                    if state.root.handle_input(&input_event, bounds) {
                        state.window.request_redraw();
                        handled = true;
                    }
                }

                if !handled {
                    let step = if zoom_dir > 0.0 {
                        ZOOM_STEP_WHEEL
                    } else if zoom_dir < 0.0 {
                        -ZOOM_STEP_WHEEL
                    } else {
                        0.0
                    };
                    if step != 0.0 {
                        state.bump_zoom(step);
                        state.window.request_redraw();
                    }
                }

                update_cursor(state);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(key) = map_key(&event.logical_key) else {
                    return;
                };
                let modifiers = to_modifiers(self.modifiers);
                if event.state == ElementState::Pressed {
                    if let Some(resolution) = state.shortcuts.resolve(
                        ShortcutChord::new(key.clone(), modifiers),
                        state.root.shortcut_context(),
                    ) {
                        let handled = match resolution.command {
                            ShortcutCommand::ZoomIn => {
                                state.bump_zoom(ZOOM_STEP_KEY);
                                true
                            }
                            ShortcutCommand::ZoomOut => {
                                state.bump_zoom(-ZOOM_STEP_KEY);
                                true
                            }
                            ShortcutCommand::ZoomReset => {
                                state.set_zoom(1.0);
                                true
                            }
                            _ => state.root.apply_shortcut(resolution.command),
                        };
                        if handled {
                            state.window.request_redraw();
                            update_cursor(state);
                            return;
                        }
                    }
                }

                let input_event = match event.state {
                    ElementState::Pressed => InputEvent::KeyDown { key, modifiers },
                    ElementState::Released => InputEvent::KeyUp { key, modifiers },
                };
                let bounds = content_bounds(logical_size(&state.config, state.effective_scale()));
                if state.root.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
                update_cursor(state);
            }
            WindowEvent::RedrawRequested => {
                let continue_redraw = state.root.needs_redraw();
                if let Err(err) = render_frame(state) {
                    tracing::warn!(error = %err, "render frame failed");
                }
                if continue_redraw {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        if let Some(state) = &mut self.state {
            state.root.apply_event(event);
            state.window.request_redraw();
        } else {
            self.pending_events.push(event);
        }
    }
}

fn init_state(
    event_loop: &ActiveEventLoop,
    action_tx: mpsc::Sender<UserAction>,
) -> Result<RenderState> {
    let mut window_attrs = Window::default_attributes().with_title(WINDOW_TITLE);
    if let Some(monitor) = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
    {
        let size: winit::dpi::LogicalSize<f64> = monitor.size().to_logical(monitor.scale_factor());
        window_attrs = window_attrs.with_inner_size(size).with_maximized(true);
    } else {
        window_attrs = window_attrs
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_maximized(true);
    }

    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .context("failed to create window")?,
    );

    let mut root = MinimalRoot::new();
    let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));
    let read_clip = clipboard.clone();
    let write_clip = clipboard.clone();
    root.set_clipboard(
        move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
        move |text| {
            if let Some(clip) = write_clip.borrow_mut().as_mut() {
                let _ = clip.set_text(text);
            }
        },
    );
    root.set_send_handler(move |action| {
        let _ = action_tx.send(action);
    });
    let shortcuts = build_shortcut_registry();

    pollster::block_on(async move {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .context("failed to create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find a compatible adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .context("failed to create device")?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .copied()
            .or_else(|| surface_caps.formats.first().copied())
            .context("surface formats empty")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, surface_format);
        let scale_factor = window.scale_factor() as f32;
        let text_system = TextSystem::new(scale_factor);

        Ok(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor,
            zoom_factor: 1.0,
            root,
            shortcuts,
            cursor_icon: Cursor::Default,
        })
    })
}

fn render_frame(state: &mut RenderState) -> Result<()> {
    let scale_factor = state.effective_scale();
    let logical = logical_size(&state.config, scale_factor);
    let content_bounds = content_bounds(logical);

    let mut scene = Scene::new();
    let mut paint = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
    state.root.set_zoom_factor(state.zoom_factor);
    state.root.paint(content_bounds, &mut paint);

    state.renderer.resize(&state.queue, logical, scale_factor);

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    let output = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return Ok(());
        }
        Err(err) => {
            return Err(anyhow::anyhow!("surface error: {err:?}"));
        }
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    state
        .renderer
        .prepare(&state.device, &state.queue, &scene, scale_factor);
    state.renderer.render(&mut encoder, &view);
    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

fn update_cursor(state: &mut RenderState) {
    let cursor = state.root.cursor();
    if cursor != state.cursor_icon {
        state.window.set_cursor(map_cursor_icon(cursor));
        state.cursor_icon = cursor;
    }
}

fn map_cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::ResizeNs => CursorIcon::NsResize,
        Cursor::ResizeEw => CursorIcon::EwResize,
        Cursor::ResizeNesw => CursorIcon::NeswResize,
        Cursor::ResizeNwse => CursorIcon::NwseResize,
    }
}

fn window_bounds(size: Size) -> Bounds {
    Bounds::new(0.0, 0.0, size.width, size.height)
}

fn content_bounds(size: Size) -> Bounds {
    inset_bounds(window_bounds(size), PADDING)
}

fn logical_size(config: &wgpu::SurfaceConfiguration, scale_factor: f32) -> Size {
    let scale = scale_factor.max(0.1);
    Size::new(config.width as f32 / scale, config.height as f32 / scale)
}

fn inset_bounds(bounds: Bounds, padding: f32) -> Bounds {
    let width = (bounds.size.width - padding * 2.0).max(0.0);
    let height = (bounds.size.height - padding * 2.0).max(0.0);
    Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        width,
        height,
    )
}

fn spawn_event_bridge(
    proxy: EventLoopProxy<AppEvent>,
    action_tx: mpsc::Sender<UserAction>,
    action_rx: mpsc::Receiver<UserAction>,
) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime init failed");
        runtime.block_on(async move {
            let app = AutopilotApp::new(AppConfig {
                event_buffer: EVENT_BUFFER,
            });

            if let Ok(cached) = load_moltbook_cache(MOLTBOOK_CACHE_LIMIT) {
                if !cached.is_empty() {
                    let _ = proxy.send_event(AppEvent::MoltbookFeedUpdated { posts: cached.clone() });
                    let _ = proxy.send_event(AppEvent::MoltbookLog {
                        message: format!("Loaded {} cached Moltbook posts.", cached.len()),
                    });
                }
            }

            let mut recorder = std::env::var("AUTOPILOT_REPLAY_PATH")
                .ok()
                .and_then(|path| EventRecorder::create(path).ok());

            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let cwd_string = cwd.to_string_lossy().to_string();
            let workspace = Arc::new(app.open_workspace(cwd.clone()));
            let workspace_id = workspace.workspace_id().to_string();
            let (remote_control_tx, remote_control_rx) =
                tokio::sync::mpsc::channel::<RuntimeRemoteControlRequest>(128);
            let runtime_sync = Arc::new(RuntimeCodexSync::from_env(
                &cwd_string,
                action_tx.clone(),
                remote_control_tx,
            ));
            let pending_runtime_auth_flow =
                Arc::new(tokio::sync::Mutex::new(None::<RuntimeSyncAuthFlow>));
            if let Some(sync) = runtime_sync.as_ref() {
                sync.start_heartbeat_loop();
            }
            emit_runtime_auth_state(
                &proxy,
                &runtime_sync,
                &pending_runtime_auth_flow,
                None,
                None,
            )
            .await;
            let mut stream = workspace.events();
            let session_states = Arc::new(tokio::sync::Mutex::new(HashMap::<
                SessionId,
                SessionRuntime,
            >::new()));
            let thread_to_session =
                Arc::new(tokio::sync::Mutex::new(HashMap::<String, SessionId>::new()));
            let moltbook_reply_targets =
                Arc::new(tokio::sync::Mutex::new(HashMap::<String, MoltbookReplyTarget>::new()));

            let bootstrap_session = workspace.start_session(Some("Bootstrap".to_string()));
            let bootstrap_id = bootstrap_session.session_id();
            let bootstrap_state = SessionRuntime::new();
            {
                let mut guard = session_states.lock().await;
                guard.insert(bootstrap_id, bootstrap_state.clone());
            }

            let proxy_events = proxy.clone();
            tokio::spawn(async move {
                while let Some(event) = stream.next().await {
                    let _ = proxy_events.send_event(event.clone());
                    if let Some(writer) = recorder.as_mut() {
                        if let Err(err) = writer.record_event(&event) {
                            tracing::warn!(error = %err, "failed to record replay event");
                        }
                    }
                }
            });

            let (client, channels) = match AppServerClient::spawn(AppServerConfig {
                cwd: Some(cwd.clone()),
                wire_log: None,
                env: Vec::new(),
            })
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "codex/error",
                            "params": { "message": err.to_string() }
                        })
                        .to_string(),
                    });
                    futures::future::pending::<()>().await;
                    return;
                }
            };

            let client = Arc::new(client);
            let full_auto_state = Arc::new(tokio::sync::Mutex::new(None::<FullAutoState>));

            let client_info = ClientInfo {
                name: "autopilot-desktop".to_string(),
                title: Some("Autopilot Desktop".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            };
            if let Err(err) = client.initialize(client_info).await {
                let _ = proxy.send_event(AppEvent::AppServerEvent {
                    message: json!({
                        "method": "codex/error",
                        "params": { "message": err.to_string() }
                    })
                    .to_string(),
                });
            }

            match client
                .thread_start(ThreadStartParams {
                    model: Some(DEFAULT_THREAD_MODEL.to_string()),
                    model_provider: None,
                    cwd: Some(cwd_string.clone()),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: Some(SandboxMode::DangerFullAccess),
                })
                .await
            {
                Ok(response) => {
                    let mut guard = bootstrap_state.thread_id.lock().await;
                    *guard = Some(response.thread.id.clone());
                    {
                        let mut map = thread_to_session.lock().await;
                        map.insert(response.thread.id.clone(), bootstrap_id);
                    }
                    if let Some(sync) = runtime_sync.as_ref() {
                        if let Err(err) = sync
                            .ensure_worker_for_thread(&response.thread.id, Some(bootstrap_id))
                            .await
                        {
                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                message: json!({
                                    "method": "codex/error",
                                    "params": {
                                        "message": format!("Runtime worker sync failed: {err}")
                                    }
                                })
                                .to_string(),
                            });
                        }
                    }
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "thread/started",
                            "params": {
                                "threadId": response.thread.id,
                                "model": DEFAULT_THREAD_MODEL,
                                "sessionId": bootstrap_id.to_string()
                            }
                        })
                        .to_string(),
                    });
                }
                Err(err) => {
                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                        message: json!({
                            "method": "codex/error",
                            "params": { "message": err.to_string() }
                        })
                        .to_string(),
                    });
                }
            }

            let proxy_notifications = proxy.clone();
            let client_interrupt = client.clone();
            let session_states_notifications = session_states.clone();
            let thread_to_session_notifications = thread_to_session.clone();
            let full_auto_state_notifications = full_auto_state.clone();
            let proxy_full_auto = proxy.clone();
            let client_full_auto = client.clone();
            let cwd_full_auto = cwd_string.clone();
            let workspace_id_full_auto = workspace_id.clone();
            let runtime_sync_notifications = runtime_sync.clone();
            let reply_targets_notifications = moltbook_reply_targets.clone();
            let proxy_reply = proxy.clone();
            let client_reply = client.clone();
            tokio::spawn(async move {
                let mut notification_rx = channels.notifications;
                while let Some(notification) = notification_rx.recv().await {
                    let params = notification.params.as_ref();
                    let thread_id_value = extract_thread_id(params);
                    let turn_id_value = extract_turn_id(params);
                    if should_sync_runtime_notification(&notification.method) {
                        if let Some(sync) = runtime_sync_notifications.as_ref() {
                            let sync = sync.clone();
                            let thread_id = thread_id_value
                                .clone()
                                .unwrap_or_else(|| "shared".to_string());
                            let method = notification.method.clone();
                            let params = notification.params.clone();
                            let proxy_sync = proxy_notifications.clone();
                            tokio::spawn(async move {
                                if let Err(err) =
                                    sync.ingest_notification(&thread_id, &method, params.as_ref()).await
                                {
                                    let _ = proxy_sync.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": {
                                                "message": format!("Runtime event sync failed: {err}")
                                            }
                                        })
                                        .to_string(),
                                    });
                                }
                            });
                        }
                    }
                    if notification.method == "turn/started" {
                        if let Some(params) = notification.params.as_ref() {
                            let next_turn = params
                                .get("turnId")
                                .and_then(|id| id.as_str())
                                .or_else(|| {
                                    params
                                        .get("turn")
                                        .and_then(|turn| turn.get("id"))
                                        .and_then(|id| id.as_str())
                                })
                                .map(|id| id.to_string());
                            if let Some(next_turn) = next_turn {
                                let session_id = if let Some(thread_id) = thread_id_value.as_deref()
                                {
                                    thread_to_session_notifications
                                        .lock()
                                        .await
                                        .get(thread_id)
                                        .copied()
                                } else {
                                    None
                                };
                                if let Some(session_id) = session_id {
                                    let state = session_states_notifications
                                        .lock()
                                        .await
                                        .get(&session_id)
                                        .cloned();
                                    if let Some(state) = state {
                                        {
                                            let mut guard = state.turn_id.lock().await;
                                            *guard = Some(next_turn.clone());
                                        }
                                        if state.pending_interrupt.swap(false, Ordering::SeqCst) {
                                            if let Some(thread_id) =
                                                state.thread_id.lock().await.clone()
                                            {
                                                let _ = client_interrupt
                                                    .turn_interrupt(TurnInterruptParams {
                                                        thread_id,
                                                        turn_id: next_turn.clone(),
                                                    })
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if notification.method == "turn/completed" {
                        let completed_turn = notification
                            .params
                            .as_ref()
                            .and_then(|params| params.get("turnId"))
                            .and_then(|id| id.as_str())
                            .or_else(|| {
                                notification
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.get("turn"))
                                    .and_then(|turn| turn.get("id"))
                                    .and_then(|id| id.as_str())
                            })
                            .map(|id| id.to_string());
                        let session_id = if let Some(thread_id) = thread_id_value.as_deref() {
                            thread_to_session_notifications
                                .lock()
                                .await
                                .get(thread_id)
                                .copied()
                        } else {
                            None
                        };
                        if let Some(session_id) = session_id {
                            let state = session_states_notifications
                                .lock()
                                .await
                                .get(&session_id)
                                .cloned();
                            if let Some(state) = state {
                                let mut guard = state.turn_id.lock().await;
                                if completed_turn
                                    .as_deref()
                                    .map(|id| guard.as_deref() == Some(id))
                                    .unwrap_or(true)
                                {
                                    *guard = None;
                                }
                            }
                        }

                        if let Some(thread_id) = thread_id_value.as_deref() {
                            let target = {
                                let mut guard = reply_targets_notifications.lock().await;
                                guard.remove(thread_id)
                            };
                            if let Some(target) = target {
                                let proxy = proxy_reply.clone();
                                let client = client_reply.clone();
                                let thread_id = thread_id.to_string();
                                tokio::spawn(async move {
                                    let mut reply_text = None;
                                    for _ in 0..5 {
                                        match fetch_moltbook_reply_text(client.as_ref(), &thread_id)
                                            .await
                                        {
                                            Ok(Some(text)) => {
                                                reply_text = Some(text);
                                                break;
                                            }
                                            Ok(None) => {}
                                            Err(err) => {
                                                let _ = proxy.send_event(AppEvent::MoltbookLog {
                                                    message: format!(
                                                        "Moltbook reply fetch failed: {err}"
                                                    ),
                                                });
                                                break;
                                            }
                                        }
                                        tokio::time::sleep(std::time::Duration::from_secs(1))
                                            .await;
                                    }

                                    if let Some(text) = reply_text {
                                        let reply_body = extract_moltbook_reply_body(&text)
                                            .unwrap_or_else(|| sanitize_moltbook_reply(&text));
                                        if looks_like_refusal(&reply_body) {
                                            let _ = proxy.send_event(AppEvent::MoltbookLog {
                                                message: "Moltbook reply was not posted (assistant refused).".to_string(),
                                            });
                                        } else if !reply_body.trim().is_empty() {
                                            if let Err(err) = post_moltbook_reply_comment(
                                                proxy.clone(),
                                                &target.post_id,
                                                &reply_body,
                                            )
                                            .await
                                            {
                                                let _ = proxy.send_event(AppEvent::MoltbookLog {
                                                    message: format!(
                                                        "Moltbook reply post failed: {err}"
                                                    ),
                                                });
                                            }
                                        }
                                    }

                                    let _ = load_moltbook_comments(proxy.clone(), &target.post_id)
                                        .await;
                                });
                            }
                        }
                    }

                    let decision_request: Option<FullAutoDecisionRequest> = {
                        let mut full_auto_guard = full_auto_state_notifications.lock().await;
                        if let Some(state) = full_auto_guard.as_mut() {
                            state.record_event(
                                &notification.method,
                                params,
                                thread_id_value.as_deref(),
                                turn_id_value.as_deref(),
                            );
                            if notification.method == "thread/started" {
                                if let Some(thread_id) = thread_id_value.as_deref() {
                                    state.adopt_thread(thread_id);
                                }
                            }
                            if notification.method == "turn/completed" {
                                state.prepare_decision(
                                    thread_id_value.as_deref(),
                                    turn_id_value.as_deref(),
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(request) = decision_request {
                        let full_auto_state = full_auto_state_notifications.clone();
                        let proxy = proxy_full_auto.clone();
                        let client = client_full_auto.clone();
                        let cwd = cwd_full_auto.clone();
                        let workspace_id = workspace_id_full_auto.clone();
                        tokio::spawn(async move {
                            let is_active = {
                                let guard = full_auto_state.lock().await;
                                guard
                                    .as_ref()
                                    .map(|state| {
                                        state.enabled
                                            && state.matches_thread(Some(request.thread_id.as_str()))
                                    })
                                    .unwrap_or(false)
                            };
                            if !is_active {
                                return;
                            }

                            let (guidance_mode, mut lm, goal_intent) = {
                                let guard = full_auto_state.lock().await;
                                if let Some(state) = guard.as_ref() {
                                    (
                                        state.guidance_mode(),
                                        state.decision_lm(),
                                        state.guidance_goal_intent(),
                                    )
                                } else {
                                    (GuidanceMode::Legacy, None, guidance_goal_intent())
                                }
                            };

                            if lm.is_none() {
                                let built = match guidance_mode {
                                    GuidanceMode::Demo => ensure_guidance_demo_lm().await,
                                    GuidanceMode::Legacy => {
                                        let model = decision_model();
                                        ensure_codex_lm(&model).await
                                    }
                                };
                                match built {
                                    Ok(built) => {
                                        lm = Some(built.clone());
                                        let mut guard = full_auto_state.lock().await;
                                        if let Some(state) = guard.as_mut() {
                                            state.set_decision_lm(built);
                                        }
                                    }
                                    Err(error) => {
                                        let payload = json!({
                                            "method": "fullauto/decision",
                                            "params": {
                                                "threadId": request.thread_id,
                                                "turnId": request.turn_id,
                                                "action": "pause",
                                                "reason": error,
                                                "confidence": 0.0,
                                                "state": "paused"
                                            }
                                        });
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: payload.to_string(),
                                        });
                                        let mut guard = full_auto_state.lock().await;
                                        *guard = None;
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "fullauto/status",
                                                "params": {
                                                    "workspaceId": workspace_id,
                                                    "enabled": false,
                                                    "state": "paused"
                                                }
                                            })
                                            .to_string(),
                                        });
                                        return;
                                    }
                                }
                            }

                            let Some(lm) = lm else {
                                return;
                            };

                            let decision_result = match guidance_mode {
                                GuidanceMode::Demo => {
                                    run_guidance_followup(
                                        &proxy,
                                        &request.thread_id,
                                        &request.summary,
                                        &goal_intent,
                                        &lm,
                                    )
                                    .await
                                }
                                GuidanceMode::Legacy => {
                                    run_full_auto_decision(&request.summary, &lm).await
                                }
                            };

                            let decision_result = match decision_result {
                                Ok(decision) => decision,
                                Err(error) => {
                                    let payload = json!({
                                        "method": "fullauto/decision",
                                        "params": {
                                            "threadId": request.thread_id,
                                            "turnId": request.turn_id,
                                            "action": "pause",
                                            "reason": error,
                                            "confidence": 0.0,
                                            "state": "paused"
                                        }
                                    });
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: payload.to_string(),
                                    });
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                }
                            };

                            let is_active = {
                                let guard = full_auto_state.lock().await;
                                guard
                                    .as_ref()
                                    .map(|state| {
                                        state.enabled
                                            && state.matches_thread(Some(request.thread_id.as_str()))
                                    })
                                    .unwrap_or(false)
                            };
                            if !is_active {
                                return;
                            }

                            let FullAutoDecisionResult {
                                decision: raw_decision,
                                diagnostics,
                            } = decision_result;

                            let (decision, run_id, sequence_id) = {
                                let mut guard = full_auto_state.lock().await;
                                if let Some(state) = guard.as_mut() {
                                    let decision = state.enforce_guardrails(
                                        &request.thread_id,
                                        &request.summary,
                                        raw_decision,
                                    );
                                    state.apply_decision(&request.thread_id, &decision);
                                    let sequence_id = state.next_decision_sequence();
                                    (decision, state.run_id.clone(), sequence_id)
                                } else {
                                    (raw_decision, "unknown".to_string(), 0)
                                }
                            };

                            let decision_state = if decision.action == FullAutoAction::Continue {
                                "running"
                            } else {
                                "paused"
                            };
                            let next_input_preview = decision
                                .next_input
                                .as_deref()
                                .unwrap_or_default()
                                .chars()
                                .take(140)
                                .collect::<String>();

                            let guardrail_value = decision
                                .guardrail
                                .as_ref()
                                .and_then(|g| serde_json::to_value(g).ok());
                            let summary_value =
                                serde_json::to_value(&request.summary).unwrap_or(Value::Null);
                            let diagnostics_value =
                                serde_json::to_value(&diagnostics).unwrap_or(Value::Null);

                            let payload = json!({
                                "method": "fullauto/decision",
                                "params": {
                                    "threadId": request.thread_id,
                                    "turnId": request.turn_id,
                                    "action": decision.action.as_str(),
                                    "reason": decision.reason,
                                    "confidence": decision.confidence,
                                    "state": decision_state,
                                    "nextInput": next_input_preview,
                                    "sequenceId": sequence_id,
                                    "runId": run_id,
                                    "guardrail": guardrail_value,
                                    "summary": summary_value
                                }
                            });
                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                message: payload.to_string(),
                            });

                            let raw_payload = json!({
                                "method": "fullauto/decision_raw",
                                "params": {
                                    "threadId": request.thread_id,
                                    "turnId": request.turn_id,
                                    "sequenceId": sequence_id,
                                    "runId": run_id,
                                    "rawPrediction": diagnostics.raw_prediction,
                                    "parseDiagnostics": diagnostics_value,
                                    "summary": summary_value
                                }
                            });
                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                message: raw_payload.to_string(),
                            });

                            match decision.action {
                                FullAutoAction::Continue => {
                                    let next_input = decision
                                        .next_input
                                        .unwrap_or_else(|| request.fallback_prompt.clone());
                                    let params = TurnStartParams {
                                        thread_id: request.thread_id.clone(),
                                        input: vec![UserInput::Text { text: next_input }],
                                        model: None,
                                        effort: None,
                                        summary: None,
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                                        cwd: Some(cwd),
                                    };
                                    let _ = client.turn_start(params).await;
                                }
                                FullAutoAction::Pause
                                | FullAutoAction::Stop
                                | FullAutoAction::Review => {
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                }
                            }
                        });
                    }

                    let payload = json!({
                        "method": notification.method,
                        "params": notification.params,
                    });
                    let _ = proxy_notifications.send_event(AppEvent::AppServerEvent {
                        message: payload.to_string(),
                    });
                }
            });

            let client_requests = client.clone();
            let proxy_requests = proxy.clone();
            let full_auto_state_requests = full_auto_state.clone();
            tokio::spawn(async move {
                let mut request_rx = channels.requests;
                while let Some(request) = request_rx.recv().await {
                    let payload = json!({
                        "id": request.id,
                        "method": request.method,
                        "params": request.params,
                    });
                    let _ = proxy_requests.send_event(AppEvent::AppServerEvent {
                        message: payload.to_string(),
                    });

                    let response = match request.method.as_str() {
                        _ => build_auto_response(request.method.as_str(), request.params.as_ref())
                            .unwrap_or_else(|| json!({})),
                    };
                    let _ = client_requests.respond(request.id, &response).await;

                    let params = request.params.as_ref();
                    let thread_id_value = extract_thread_id(params);
                    let turn_id_value = extract_turn_id(params);
                    let mut full_auto_guard = full_auto_state_requests.lock().await;
                    if let Some(state) = full_auto_guard.as_mut() {
                        state.record_event(
                            request.method.as_str(),
                            params,
                            thread_id_value.as_deref(),
                            turn_id_value.as_deref(),
                        );
                    }
                }
            });

            let client_remote_control = client.clone();
            let runtime_sync_remote_control = runtime_sync.clone();
            let session_states_remote_control = session_states.clone();
            let thread_to_session_remote_control = thread_to_session.clone();
            let cwd_remote_control = cwd_string.clone();
            let proxy_remote_control = proxy.clone();
            tokio::spawn(async move {
                let mut remote_control_rx = remote_control_rx;
                while let Some(request) = remote_control_rx.recv().await {
                    let outcome = execute_runtime_remote_control_request(
                        client_remote_control.clone(),
                        request.clone(),
                        cwd_remote_control.clone(),
                        runtime_sync_remote_control.clone(),
                        session_states_remote_control.clone(),
                        thread_to_session_remote_control.clone(),
                    )
                    .await;

                    match outcome {
                        Ok(response_payload) => {
                            tracing::info!(
                                worker_id = %request.worker_id,
                                request_id = %request.request_id,
                                method = request.method.as_str(),
                                "runtime sync executed remote control request"
                            );
                            let _ = proxy_remote_control.send_event(AppEvent::AppServerEvent {
                                message: json!({
                                    "method": "runtime/control_dispatch",
                                    "params": {
                                        "workerId": request.worker_id,
                                        "requestId": request.request_id,
                                        "method": request.method.as_str(),
                                        "status": "ok",
                                        "response": response_payload,
                                    }
                                })
                                .to_string(),
                            });
                        }
                        Err(error) => {
                            if let Some(sync) = runtime_sync_remote_control.as_ref() {
                                let _ = sync
                                    .emit_control_error_receipt(
                                        &request.worker_id,
                                        &request.request_id,
                                        request.method.as_str(),
                                        error.code,
                                        &error.message,
                                        error.details.clone(),
                                    )
                                    .await;
                            }
                            tracing::warn!(
                                worker_id = %request.worker_id,
                                request_id = %request.request_id,
                                method = request.method.as_str(),
                                code = error.code,
                                message = %error.message,
                                "runtime sync remote control dispatch failed"
                            );
                            let _ = proxy_remote_control.send_event(AppEvent::AppServerEvent {
                                message: json!({
                                    "method": "runtime/control_dispatch",
                                    "params": {
                                        "workerId": request.worker_id,
                                        "requestId": request.request_id,
                                        "method": request.method.as_str(),
                                        "status": "error",
                                        "code": error.code,
                                        "message": error.message,
                                    }
                                })
                                .to_string(),
                            });
                        }
                    }
                }
            });

            let handle = tokio::runtime::Handle::current();
            let workspace_for_actions = workspace.clone();
            let client_for_actions = client.clone();
            let session_states_for_actions = session_states.clone();
            let thread_to_session_for_actions = thread_to_session.clone();
            let proxy_actions = proxy.clone();
            let cwd_for_actions = cwd_string.clone();
            let runtime_sync_actions = runtime_sync.clone();
            let pending_runtime_auth_flow_actions = pending_runtime_auth_flow.clone();
            tokio::task::spawn_blocking(move || {
                let mut pylon_runtime = InProcessPylon::new();
                let mut inbox_state = DesktopInboxState::new();
                let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                    snapshot: inbox_state.snapshot(),
                    source: "bootstrap".to_string(),
                });
                while let Ok(action) = action_rx.recv() {
                    workspace_for_actions.dispatch(action.clone());
                    match action {
                        UserAction::NewChat { model, .. } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            let workspace = workspace_for_actions.clone();
                            let workspace_id = workspace_id.clone();
                            let session_states = session_states_for_actions.clone();
                            let thread_to_session = thread_to_session_for_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let session_handle =
                                workspace.start_session(Some("New chat".to_string()));
                            let session_id = session_handle.session_id();
                            let session_state = SessionRuntime::new();

                            handle.spawn(async move {
                                {
                                    let mut guard = session_states.lock().await;
                                    guard.insert(session_id, session_state.clone());
                                }

                                {
                                    let mut guard = full_auto_state.lock().await;
                                    if guard.is_some() {
                                        *guard = None;
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "fullauto/status",
                                                "params": {
                                                    "workspaceId": workspace_id,
                                                    "enabled": false,
                                                    "state": "paused"
                                                }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }

                                let selected_model =
                                    model.unwrap_or_else(|| DEFAULT_THREAD_MODEL.to_string());
                                match client
                                    .thread_start(ThreadStartParams {
                                        model: Some(selected_model.clone()),
                                        model_provider: None,
                                        cwd: Some(cwd.clone()),
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox: Some(SandboxMode::DangerFullAccess),
                                    })
                                    .await
                                {
                                    Ok(response) => {
                                        {
                                            let mut guard = session_state.thread_id.lock().await;
                                            *guard = Some(response.thread.id.clone());
                                        }
                                        {
                                            let mut map = thread_to_session.lock().await;
                                            map.insert(response.thread.id.clone(), session_id);
                                        }
                                        if let Some(sync) = runtime_sync.as_ref() {
                                            if let Err(err) = sync
                                                .ensure_worker_for_thread(
                                                    &response.thread.id,
                                                    Some(session_id),
                                                )
                                                .await
                                            {
                                                let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                    message: json!({
                                                        "method": "codex/error",
                                                        "params": {
                                                            "message": format!("Runtime worker sync failed: {err}")
                                                        }
                                                    })
                                                    .to_string(),
                                                });
                                            }
                                        }
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "thread/started",
                                                "params": {
                                                    "threadId": response.thread.id,
                                                    "model": selected_model,
                                                    "sessionId": session_id.to_string()
                                                }
                                            })
                                            .to_string(),
                                        });
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "codex/error",
                                                "params": { "message": err.to_string() }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }
                            });
                        }
                        UserAction::RuntimeAuthSendCode { email } => {
                            let proxy = proxy_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let pending_runtime_auth_flow =
                                pending_runtime_auth_flow_actions.clone();
                            handle.spawn(async move {
                                let base_url = if let Some(sync) = runtime_sync.as_ref() {
                                    sync.current_base_url().await
                                } else {
                                    DEFAULT_AUTH_BASE_URL.to_string()
                                };
                                let mut flow = match RuntimeSyncAuthFlow::new(&base_url) {
                                    Ok(flow) => flow,
                                    Err(err) => {
                                        emit_runtime_auth_state(
                                            &proxy,
                                            &runtime_sync,
                                            &pending_runtime_auth_flow,
                                            None,
                                            Some(format!("Auth init failed: {err}")),
                                        )
                                        .await;
                                        return;
                                    }
                                };

                                match flow.send_code(&email).await {
                                    Ok(normalized_email) => {
                                        {
                                            let mut pending = pending_runtime_auth_flow.lock().await;
                                            *pending = Some(flow);
                                        }
                                        emit_runtime_auth_state(
                                            &proxy,
                                            &runtime_sync,
                                            &pending_runtime_auth_flow,
                                            Some(format!(
                                                "Sent verification code to {}. Enter the newest code from your inbox and click Verify.",
                                                normalized_email
                                            )),
                                            None,
                                        )
                                        .await;
                                    }
                                    Err(err) => {
                                        emit_runtime_auth_state(
                                            &proxy,
                                            &runtime_sync,
                                            &pending_runtime_auth_flow,
                                            None,
                                            Some(format!("Failed to send verification code: {err}")),
                                        )
                                        .await;
                                    }
                                }
                            });
                        }
                        UserAction::RuntimeAuthVerifyCode { code } => {
                            let proxy = proxy_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let pending_runtime_auth_flow =
                                pending_runtime_auth_flow_actions.clone();
                            handle.spawn(async move {
                                let flow = {
                                    let mut pending = pending_runtime_auth_flow.lock().await;
                                    pending.take()
                                };
                                let Some(mut flow) = flow else {
                                    emit_runtime_auth_state(
                                        &proxy,
                                        &runtime_sync,
                                        &pending_runtime_auth_flow,
                                        None,
                                        Some(
                                            "No pending verification. Send code first."
                                                .to_string(),
                                        ),
                                    )
                                    .await;
                                    return;
                                };

                                match flow.verify_code(&code).await {
                                    Ok(state) => {
                                        if let Some(sync) = runtime_sync.as_ref() {
                                            sync.apply_auth_state(&state).await;
                                        }
                                        match persist_runtime_auth_state(&state) {
                                            Ok(path) => {
                                                emit_runtime_auth_state(
                                                    &proxy,
                                                    &runtime_sync,
                                                    &pending_runtime_auth_flow,
                                                    Some(format!(
                                                        "Signed in as {}. Runtime sync token saved at {}.",
                                                        state
                                                            .email
                                                            .as_deref()
                                                            .unwrap_or("<unknown>"),
                                                        path.display()
                                                    )),
                                                    None,
                                                )
                                                .await;
                                            }
                                            Err(err) => {
                                                emit_runtime_auth_state(
                                                    &proxy,
                                                    &runtime_sync,
                                                    &pending_runtime_auth_flow,
                                                    None,
                                                    Some(format!(
                                                        "Signed in but failed to persist auth state: {err}"
                                                    )),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        {
                                            let mut pending = pending_runtime_auth_flow.lock().await;
                                            *pending = Some(flow);
                                        }
                                        emit_runtime_auth_state(
                                            &proxy,
                                            &runtime_sync,
                                            &pending_runtime_auth_flow,
                                            None,
                                            Some(format!("Code verification failed: {err}")),
                                        )
                                        .await;
                                    }
                                }
                            });
                        }
                        UserAction::RuntimeAuthStatus => {
                            let proxy = proxy_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let pending_runtime_auth_flow =
                                pending_runtime_auth_flow_actions.clone();
                            handle.spawn(async move {
                                emit_runtime_auth_state(
                                    &proxy,
                                    &runtime_sync,
                                    &pending_runtime_auth_flow,
                                    None,
                                    None,
                                )
                                .await;
                            });
                        }
                        UserAction::RuntimeAuthLogout => {
                            let proxy = proxy_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let pending_runtime_auth_flow =
                                pending_runtime_auth_flow_actions.clone();
                            handle.spawn(async move {
                                let clear_error = clear_runtime_auth_state().err();
                                {
                                    let mut pending = pending_runtime_auth_flow.lock().await;
                                    *pending = None;
                                }
                                if let Some(sync) = runtime_sync.as_ref() {
                                    sync.clear_auth_state().await;
                                }
                                if let Some(err) = clear_error {
                                    emit_runtime_auth_state(
                                        &proxy,
                                        &runtime_sync,
                                        &pending_runtime_auth_flow,
                                        None,
                                        Some(format!(
                                            "Failed to clear runtime auth state: {err}"
                                        )),
                                    )
                                    .await;
                                } else {
                                    emit_runtime_auth_state(
                                        &proxy,
                                        &runtime_sync,
                                        &pending_runtime_auth_flow,
                                        Some("Logged out. Runtime sync auth token cleared.".to_string()),
                                        None,
                                    )
                                    .await;
                                }
                            });
                        }
                        UserAction::Message {
                            session_id,
                            text,
                            model,
                            reasoning,
                        } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            handle.spawn(async move {
                                let session_state =
                                    session_states.lock().await.get(&session_id).cloned();
                                let Some(session_state) = session_state else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Session not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };
                                let thread_id = session_state.thread_id.lock().await.clone();

                                let Some(thread_id) = thread_id else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Codex thread not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };
                                let (should_intercept, cached_lm, goal_intent) = {
                                    let mut guard = full_auto_state.lock().await;
                                    if let Some(state) = guard.as_mut() {
                                        if state.matches_thread(Some(thread_id.as_str())) {
                                            state.adopt_thread(&thread_id);
                                            let should_intercept = state.activate_guidance_mode();
                                            (
                                                should_intercept,
                                                state.decision_lm(),
                                                state.guidance_goal_intent(),
                                            )
                                        } else {
                                            (false, None, guidance_goal_intent())
                                        }
                                    } else {
                                        (false, None, guidance_goal_intent())
                                    }
                                };

                                if should_intercept {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "guidance/user_message",
                                            "params": {
                                                "threadId": thread_id,
                                                "text": text
                                            }
                                        })
                                        .to_string(),
                                    });
                                    emit_guidance_status(
                                        &proxy,
                                        &thread_id,
                                        None,
                                        "Guidance: preparing...",
                                    );
                                    let (lm, should_cache) = match resolve_guidance_lm(cached_lm)
                                        .await
                                    {
                                        Ok(result) => result,
                                        Err(error) => {
                                            let payload = json!({
                                                "method": "guidance/response",
                                                "params": {
                                                    "threadId": thread_id,
                                                    "text": format!("Guidance error: {error}"),
                                                    "signatures": ["GuidanceRouterSignature"],
                                                    "model": "unknown"
                                                }
                                            });
                                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                message: payload.to_string(),
                                            });
                                            return;
                                        }
                                    };
                                    if should_cache {
                                        let mut guard = full_auto_state.lock().await;
                                        if let Some(state) = guard.as_mut() {
                                            state.set_decision_lm(lm.clone());
                                        }
                                    }
                                    let (response, signatures) = match run_guidance_super(
                                        &proxy,
                                        &thread_id,
                                        &text,
                                        &goal_intent,
                                        &lm,
                                    )
                                    .await
                                    {
                                        Ok(result) => result,
                                        Err(error) => (
                                            format!("Guidance error: {error}"),
                                            vec![
                                                "TaskUnderstandingSignature".to_string(),
                                                "PlanningSignature".to_string(),
                                                "GuidanceDirectiveSignature".to_string(),
                                            ],
                                        ),
                                    };
                                    let should_dispatch = {
                                        let guard = full_auto_state.lock().await;
                                        guard
                                            .as_ref()
                                            .map(|state| {
                                                state.enabled
                                                    && state.matches_thread(Some(thread_id.as_str()))
                                            })
                                            .unwrap_or(false)
                                    };
                                    let response_text = response.clone();
                                    let payload = json!({
                                        "method": "guidance/response",
                                        "params": {
                                            "threadId": thread_id,
                                            "text": response_text,
                                            "signatures": signatures,
                                            "model": lm.model
                                        }
                                    });
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: payload.to_string(),
                                    });
                                    if should_dispatch {
                                        {
                                            let mut guard = full_auto_state.lock().await;
                                            if let Some(state) = guard.as_mut() {
                                                if state.matches_thread(Some(thread_id.as_str())) {
                                                    state.record_guidance_directive(
                                                        &thread_id,
                                                        response.clone(),
                                                    );
                                                }
                                            }
                                        }
                                        emit_guidance_status(
                                            &proxy,
                                            &thread_id,
                                            None,
                                            "Dispatching directive to Codex...",
                                        );
                                        let params = TurnStartParams {
                                            thread_id,
                                            input: vec![UserInput::Text { text: response.clone() }],
                                            model,
                                            effort: reasoning
                                                .as_deref()
                                                .and_then(parse_reasoning_effort),
                                            summary: None,
                                            approval_policy: Some(AskForApproval::Never),
                                            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                                            cwd: Some(cwd),
                                        };
                                        if let Err(err) = client.turn_start(params).await {
                                            let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                message: json!({
                                                    "method": "codex/error",
                                                    "params": { "message": err.to_string() }
                                                })
                                                .to_string(),
                                            });
                                        }
                                    }
                                    return;
                                }

                                let thread_id_for_turn = thread_id.clone();
                                let params = TurnStartParams {
                                    thread_id: thread_id_for_turn,
                                    input: vec![UserInput::Text { text }],
                                    model,
                                    effort: reasoning.as_deref().and_then(parse_reasoning_effort),
                                    summary: None,
                                    approval_policy: Some(AskForApproval::Never),
                                    sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                                    cwd: Some(cwd),
                                };

                                if let Err(err) = client.turn_start(params).await {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": err.to_string() }
                                        })
                                        .to_string(),
                                    });
                                }
                            });
                        }
                        UserAction::ThreadsRefresh => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let params = ThreadListParams {
                                    limit: Some(10),
                                    ..Default::default()
                                };
                                match client.thread_list(params).await {
                                    Ok(response) => {
                                        let threads = response
                                            .data
                                            .into_iter()
                                            .map(|thread| autopilot_app::ThreadSummary {
                                                id: thread.id,
                                                preview: thread.preview,
                                                model_provider: thread.model_provider,
                                                cwd: thread.cwd,
                                                created_at: thread.created_at,
                                            })
                                            .collect::<Vec<_>>();
                                        let _ =
                                            proxy.send_event(AppEvent::ThreadsUpdated {
                                                threads,
                                                next_cursor: response.next_cursor,
                                                append: false,
                                            });
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "codex/error",
                                                "params": { "message": err.to_string() }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }
                            });
                        }
                        UserAction::ThreadsLoadMore { cursor } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let Some(cursor) = cursor else {
                                    return;
                                };
                                let params = ThreadListParams {
                                    limit: Some(10),
                                    cursor: Some(cursor),
                                    ..Default::default()
                                };
                                match client.thread_list(params).await {
                                    Ok(response) => {
                                        let threads = response
                                            .data
                                            .into_iter()
                                            .map(|thread| autopilot_app::ThreadSummary {
                                                id: thread.id,
                                                preview: thread.preview,
                                                model_provider: thread.model_provider,
                                                cwd: thread.cwd,
                                                created_at: thread.created_at,
                                            })
                                            .collect::<Vec<_>>();
                                        let _ =
                                            proxy.send_event(AppEvent::ThreadsUpdated {
                                                threads,
                                                next_cursor: response.next_cursor,
                                                append: true,
                                            });
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "codex/error",
                                                "params": { "message": err.to_string() }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }
                            });
                        }
                        UserAction::ThreadOpen { thread_id } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let workspace = workspace_for_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let thread_to_session = thread_to_session_for_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let session_handle =
                                workspace.start_session(Some("Thread".to_string()));
                            let session_id = session_handle.session_id();
                            let session_state = SessionRuntime::new();

                            handle.spawn(async move {
                                {
                                    let mut guard = session_states.lock().await;
                                    guard.insert(session_id, session_state.clone());
                                }

                                match client
                                    .thread_resume(ThreadResumeParams {
                                        thread_id: thread_id.clone(),
                                        model: None,
                                        model_provider: None,
                                        cwd: Some(cwd),
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox: Some(SandboxMode::DangerFullAccess),
                                    })
                                    .await
                                {
                                    Ok(response) => {
                                        {
                                            let mut guard = session_state.thread_id.lock().await;
                                            *guard = Some(response.thread.id.clone());
                                        }
                                        {
                                            let mut map = thread_to_session.lock().await;
                                            map.insert(response.thread.id.clone(), session_id);
                                        }
                                        if let Some(sync) = runtime_sync.as_ref() {
                                            if let Err(err) = sync
                                                .ensure_worker_for_thread(
                                                    &response.thread.id,
                                                    Some(session_id),
                                                )
                                                .await
                                            {
                                                let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                    message: json!({
                                                        "method": "codex/error",
                                                        "params": {
                                                            "message": format!("Runtime worker sync failed: {err}")
                                                        }
                                                    })
                                                    .to_string(),
                                                });
                                            }
                                        }
                                        let thread = autopilot_app::ThreadSnapshot {
                                            id: response.thread.id,
                                            preview: response.thread.preview,
                                            turns: response
                                                .thread
                                                .turns
                                                .into_iter()
                                                .map(|turn| autopilot_app::ThreadTurn {
                                                    id: turn.id,
                                                    items: turn.items,
                                                })
                                                .collect(),
                                        };
                                        let _ = proxy.send_event(AppEvent::ThreadLoaded {
                                            session_id,
                                            thread,
                                            model: response.model,
                                        });
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "codex/error",
                                                "params": { "message": err.to_string() }
                                            })
                                            .to_string(),
                                        });
                                    }
                                }
                            });
                        }
                        UserAction::InboxRefresh => {
                            inbox_state.refresh();
                            let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                                snapshot: inbox_state.snapshot(),
                                source: "refresh".to_string(),
                            });
                        }
                        UserAction::InboxSelectThread { thread_id } => {
                            inbox_state.select_thread(thread_id.as_str());
                            let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                                snapshot: inbox_state.snapshot(),
                                source: "select_thread".to_string(),
                            });
                        }
                        UserAction::InboxApproveDraft { thread_id } => {
                            inbox_state.approve_draft(thread_id.as_str());
                            let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                                snapshot: inbox_state.snapshot(),
                                source: "approve_draft".to_string(),
                            });
                        }
                        UserAction::InboxRejectDraft { thread_id } => {
                            inbox_state.reject_draft(thread_id.as_str());
                            let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                                snapshot: inbox_state.snapshot(),
                                source: "reject_draft".to_string(),
                            });
                        }
                        UserAction::InboxLoadAudit { thread_id } => {
                            inbox_state.load_audit(thread_id.as_str());
                            let _ = proxy_actions.send_event(AppEvent::InboxUpdated {
                                snapshot: inbox_state.snapshot(),
                                source: "load_audit".to_string(),
                            });
                        }
                        UserAction::OpenFile { path } => {
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let resolved = resolve_path(&path, &cwd);
                            match std::fs::read_to_string(&resolved) {
                                Ok(contents) => {
                                    let _ = proxy.send_event(AppEvent::FileOpened {
                                        path: resolved,
                                        contents,
                                    });
                                }
                                Err(err) => {
                                    let _ = proxy.send_event(AppEvent::FileOpenFailed {
                                        path: resolved,
                                        error: err.to_string(),
                                    });
                                }
                            }
                        }
                        UserAction::SaveFile { path, contents } => {
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let resolved = resolve_path(&path, &cwd);
                            match std::fs::write(&resolved, contents) {
                                Ok(_) => {
                                    let _ =
                                        proxy.send_event(AppEvent::FileSaved { path: resolved });
                                }
                                Err(err) => {
                                    let _ = proxy.send_event(AppEvent::FileSaveFailed {
                                        path: resolved,
                                        error: err.to_string(),
                                    });
                                }
                            }
                        }
                        UserAction::Interrupt { session_id, .. } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            let workspace_id = workspace_id.clone();
                            handle.spawn(async move {
                                let session_state =
                                    session_states.lock().await.get(&session_id).cloned();
                                let Some(session_state) = session_state else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Session not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };
                                let thread_id = session_state.thread_id.lock().await.clone();
                                let Some(thread_id) = thread_id else {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": "Codex thread not ready" }
                                        })
                                        .to_string(),
                                    });
                                    return;
                                };

                                let should_disable_full_auto = {
                                    let mut guard = full_auto_state.lock().await;
                                    let matches = guard
                                        .as_ref()
                                        .map(|state| state.matches_thread(Some(thread_id.as_str())))
                                        .unwrap_or(false);
                                    if matches {
                                        *guard = None;
                                    }
                                    matches
                                };
                                if should_disable_full_auto {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                }

                                let mut turn_guard = session_state.turn_id.lock().await;
                                let turn_value =
                                    turn_guard.clone().unwrap_or_else(|| "pending".into());
                                if turn_guard.is_none() {
                                    session_state
                                        .pending_interrupt
                                        .store(true, Ordering::SeqCst);
                                } else {
                                    *turn_guard = None;
                                }

                                if let Err(err) = client
                                    .turn_interrupt(TurnInterruptParams {
                                        thread_id,
                                        turn_id: turn_value,
                                    })
                                    .await
                                {
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "codex/error",
                                            "params": { "message": err.to_string() }
                                        })
                                        .to_string(),
                                    });
                                }
                            });
                        }
                        UserAction::PylonInit => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        init_pylon_identity(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonStart => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        start_pylon_in_process(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonStop => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        stop_pylon_in_process(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::PylonRefresh => {
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        refresh_pylon_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => pylon_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy_actions.send_event(AppEvent::PylonStatus { status });
                        }
                        UserAction::WalletRefresh => {
                            let proxy = proxy_actions.clone();
                            handle.block_on(async move {
                                let status = fetch_wallet_status().await;
                                let _ = proxy.send_event(AppEvent::WalletStatus { status });
                            });
                        }
                        UserAction::DvmProviderStart => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        let _ = start_pylon_in_process(&mut pylon_runtime, &config)
                                            .await;
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmProviderStop => {
                            let proxy = proxy_actions.clone();
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        let _ = stop_pylon_in_process(&mut pylon_runtime, &config)
                                            .await;
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ = proxy.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmProviderRefresh => {
                            let status = handle.block_on(async {
                                match load_pylon_config_ollama() {
                                    Ok(config) => {
                                        fetch_dvm_provider_status(&mut pylon_runtime, &config).await
                                    }
                                    Err(err) => dvm_provider_status_error(err.to_string()),
                                }
                            });
                            let _ =
                                proxy_actions.send_event(AppEvent::DvmProviderStatus { status });
                        }
                        UserAction::DvmHistoryRefresh => {
                            let snapshot = fetch_dvm_history();
                            let _ = proxy_actions.send_event(AppEvent::DvmHistory { snapshot });
                        }
                        UserAction::Nip90Submit {
                            kind,
                            prompt,
                            relays,
                            provider,
                        } => {
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let log = |message: String| {
                                    let _ = proxy.send_event(AppEvent::Nip90Log { message });
                                };

                                log("Submitting NIP-90 job...".to_string());

                                let config = match PylonConfig::load() {
                                    Ok(config) => config,
                                    Err(err) => {
                                        log(format!("Failed to load Pylon config: {err}"));
                                        return;
                                    }
                                };

                                let data_dir = match config.data_path() {
                                    Ok(path) => path,
                                    Err(err) => {
                                        log(format!("Failed to resolve Pylon data dir: {err}"));
                                        return;
                                    }
                                };
                                let identity_path = data_dir.join("identity.mnemonic");
                                if !identity_path.exists() {
                                    log(format!(
                                        "No identity found. Run 'pylon init' first. Expected: {}",
                                        identity_path.display()
                                    ));
                                    return;
                                }

                                let mnemonic = match std::fs::read_to_string(&identity_path) {
                                    Ok(value) => value.trim().to_string(),
                                    Err(err) => {
                                        log(format!("Failed to read identity: {err}"));
                                        return;
                                    }
                                };

                                let keypair = match nostr::derive_keypair(&mnemonic) {
                                    Ok(pair) => pair,
                                    Err(err) => {
                                        log(format!("Failed to derive Nostr keys: {err}"));
                                        return;
                                    }
                                };

                                let client = match DvmClient::new(keypair.private_key) {
                                    Ok(client) => client,
                                    Err(err) => {
                                        log(format!("Failed to init DVM client: {err}"));
                                        return;
                                    }
                                };

                                let relays = if relays.is_empty() {
                                    config.relays.clone()
                                } else {
                                    relays
                                };
                                if relays.is_empty() {
                                    log("No relays configured for NIP-90 submission.".to_string());
                                    return;
                                }

                                let kind = if kind == 0 {
                                    KIND_JOB_TEXT_GENERATION
                                } else {
                                    kind
                                };

                                let mut request = match JobRequest::new(kind) {
                                    Ok(request) => request.add_input(JobInput::text(prompt)),
                                    Err(err) => {
                                        log(format!("Invalid job kind {kind}: {err}"));
                                        return;
                                    }
                                };

                                for relay in &relays {
                                    request = request.add_relay(relay.clone());
                                }
                                if let Some(provider) = provider {
                                    request = request.add_service_provider(provider);
                                }

                                let relay_refs: Vec<&str> =
                                    relays.iter().map(|relay| relay.as_str()).collect();
                                let submission = match client.submit_job(request, &relay_refs).await
                                {
                                    Ok(submission) => submission,
                                    Err(err) => {
                                        log(format!("Job submission failed: {err}"));
                                        return;
                                    }
                                };

                                log(format!("Submitted job {}", submission.event_id));

                                match client
                                    .await_result(
                                        &submission.event_id,
                                        std::time::Duration::from_secs(60),
                                    )
                                    .await
                                {
                                    Ok(result) => {
                                        let preview = if result.content.len() > 400 {
                                            format!("{}…", &result.content[..400])
                                        } else {
                                            result.content
                                        };
                                        log(format!("Result: {}", preview));
                                    }
                                    Err(err) => {
                                        log(format!("Result timeout/error: {err}"));
                                    }
                                }
                            });
                        }
                        UserAction::FullAutoToggle {
                            session_id,
                            enabled,
                            continue_prompt,
                            ..
                        } => {
                            let proxy = proxy_actions.clone();
                            let full_auto_state = full_auto_state.clone();
                            let workspace_id = workspace_id.clone();
                            let session_states = session_states_for_actions.clone();
                            handle.spawn(async move {
                                let thread_id = {
                                    let state =
                                        session_states.lock().await.get(&session_id).cloned();
                                    if let Some(state) = state {
                                        state.thread_id.lock().await.clone()
                                    } else {
                                        None
                                    }
                                };
                                if enabled {
                                    let mut guard = full_auto_state.lock().await;
                                    let mut next_state = FullAutoState::new(
                                        &workspace_id,
                                        thread_id.clone(),
                                        continue_prompt.clone(),
                                    );
                                    next_state.enabled = true;
                                    if let Some(thread_id) = thread_id.clone() {
                                        next_state.thread_id = Some(thread_id);
                                    }
                                    next_state.set_continue_prompt(continue_prompt);
                                    let continue_prompt = next_state.continue_prompt.clone();
                                    *guard = Some(next_state);
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": true,
                                                "state": "running",
                                                "continuePrompt": continue_prompt
                                            }
                                        })
                                        .to_string(),
                                    });
                                } else {
                                    let mut guard = full_auto_state.lock().await;
                                    *guard = None;
                                    let _ = proxy.send_event(AppEvent::AppServerEvent {
                                        message: json!({
                                            "method": "fullauto/status",
                                            "params": {
                                                "workspaceId": workspace_id,
                                                "enabled": false,
                                                "state": "paused"
                                            }
                                        })
                                        .to_string(),
                                    });
                                }
                            });
                        }
                        UserAction::MoltbookRefresh => {
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let log = |msg: String| {
                                    let _ = proxy.send_event(AppEvent::MoltbookLog { message: msg });
                                };
                                let Some(api_key) = moltbook_api_key() else {
                                    log("Moltbook: no API key. Set MOLTBOOK_API_KEY or ~/.config/moltbook/credentials.json (or ~/.config/moltbook)".to_string());
                                    return;
                                };
                                let proxy_base = moltbook_proxy_base();
                                let live_base = moltbook_live_base();
                                let proxy_client = match MoltbookClient::with_base_url(
                                    proxy_base.clone(),
                                    Some(api_key.clone()),
                                ) {
                                    Ok(c) => Some(c),
                                    Err(e) => {
                                        log(format!("Moltbook proxy client error: {e}"));
                                        None
                                    }
                                };
                                let live_client = match MoltbookClient::with_base_url(
                                    live_base.clone(),
                                    Some(api_key.clone()),
                                ) {
                                    Ok(c) => Some(c),
                                    Err(e) => {
                                        log(format!("Moltbook live client error: {e}"));
                                        None
                                    }
                                };
                                if proxy_client.is_none() && live_client.is_none() {
                                    log("Moltbook: no usable client configured.".to_string());
                                    return;
                                }
                                log("Fetching feed and profile…".to_string());
                                let _feed_source: &str;
                                let posts_result = if let Some(client) = proxy_client.as_ref() {
                                    match client.posts_feed(PostSort::New, Some(25), None).await {
                                        Ok(posts) => {
                                            _feed_source = "openagents_proxy";
                                            Ok(posts)
                                        }
                                        Err(e) => {
                                            log(format!("Proxy feed error: {e}"));
                                            if let Some(live) = live_client.as_ref() {
                                                _feed_source = "moltbook_live";
                                                log("Falling back to live Moltbook API for feed…".to_string());
                                                live.posts_feed(PostSort::New, Some(25), None).await
                                            } else {
                                                _feed_source = "openagents_proxy";
                                                Err(e)
                                            }
                                        }
                                    }
                                } else if let Some(live) = live_client.as_ref() {
                                    _feed_source = "moltbook_live";
                                    live.posts_feed(PostSort::New, Some(25), None).await
                                } else {
                                    unreachable!("moltbook clients checked above");
                                };
                                match posts_result {
                                    Ok(posts) => {
                                        let summaries: Vec<MoltbookPostSummary> = posts
                                            .into_iter()
                                            .map(|p| {
                                                let content = p.content;
                                                let content_preview = content
                                                    .as_deref()
                                                    .map(|c| c.chars().take(120).collect());
                                                MoltbookPostSummary {
                                                    id: p.id,
                                                    title: p.title,
                                                    content_preview,
                                                    content,
                                                    author_name: p.author.map(|a| a.name),
                                                    score: p.score,
                                                    comment_count: p.comment_count,
                                                    created_at: p.created_at,
                                                    submolt: p.submolt,
                                                }
                                            })
                                            .collect();
                                        let new_count = summaries.len();
                                        let cached = load_moltbook_cache(MOLTBOOK_CACHE_LIMIT)
                                            .unwrap_or_default();
                                        let merged = merge_moltbook_posts(summaries, cached);
                                        let total = merged.len();
                                        if let Err(err) = store_moltbook_cache(&merged) {
                                            log(format!("Moltbook cache error: {err}"));
                                        }
                                        let _ = proxy
                                            .send_event(AppEvent::MoltbookFeedUpdated { posts: merged });
                                        log(format!("Loaded {new_count} posts (total {total})"));
                                    }
                                    Err(e) => log(format!("Feed error: {e}")),
                                }
                                let profile_result = if let Some(client) = proxy_client.as_ref() {
                                    match client.agents_me().await {
                                        Ok(profile) => Ok(profile),
                                        Err(e) => {
                                            log(format!("Proxy profile error: {e}"));
                                            if let Some(live) = live_client.as_ref() {
                                                log("Falling back to live Moltbook API for profile…".to_string());
                                                live.agents_me().await
                                            } else {
                                                Err(e)
                                            }
                                        }
                                    }
                                } else if let Some(live) = live_client.as_ref() {
                                    live.agents_me().await
                                } else {
                                    unreachable!("moltbook clients checked above");
                                };
                                match profile_result {
                                    Ok(agent) => {
                                        let posts_count = agent.stats.as_ref().and_then(|s| s.posts).unwrap_or(0);
                                        let comments_count = agent.stats.as_ref().and_then(|s| s.comments).unwrap_or(0);
                                        let profile = MoltbookProfileSummary {
                                            agent_name: agent.name,
                                            posts_count,
                                            comments_count,
                                        };
                                        let _ = proxy.send_event(AppEvent::MoltbookProfileLoaded { profile });
                                    }
                                    Err(_) => {
                                        let _ = proxy.send_event(AppEvent::MoltbookProfileLoaded {
                                            profile: MoltbookProfileSummary {
                                                agent_name: "?".to_string(),
                                                posts_count: 0,
                                                comments_count: 0,
                                            },
                                        });
                                    }
                                };
                            });
                        }
                        UserAction::MoltbookLoadComments { post_id } => {
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                if let Err(err) =
                                    load_moltbook_comments(proxy.clone(), &post_id).await
                                {
                                    let _ = proxy.send_event(AppEvent::MoltbookLog {
                                        message: format!("Moltbook comments error: {err}"),
                                    });
                                }
                            });
                        }
                        UserAction::MoltbookReply { post_id } => {
                            let client = client_for_actions.clone();
                            let proxy = proxy_actions.clone();
                            let cwd = cwd_for_actions.clone();
                            let workspace = workspace_for_actions.clone();
                            let session_states = session_states_for_actions.clone();
                            let thread_to_session = thread_to_session_for_actions.clone();
                            let runtime_sync = runtime_sync_actions.clone();
                            let reply_targets = moltbook_reply_targets.clone();
                            handle.spawn(async move {
                                let session_handle =
                                    workspace.start_session(Some("Moltbook Reply".to_string()));
                                let session_id = session_handle.session_id();
                                let session_state = SessionRuntime::new();
                                {
                                    let mut guard = session_states.lock().await;
                                    guard.insert(session_id, session_state.clone());
                                }

                                let model = DEFAULT_THREAD_MODEL.to_string();
                                let thread_id = match client
                                    .thread_start(ThreadStartParams {
                                        model: Some(model.clone()),
                                        model_provider: None,
                                        cwd: Some(cwd.clone()),
                                        approval_policy: Some(AskForApproval::Never),
                                        sandbox: Some(SandboxMode::DangerFullAccess),
                                    })
                                    .await
                                {
                                    Ok(response) => {
                                        {
                                            let mut guard = session_state.thread_id.lock().await;
                                            *guard = Some(response.thread.id.clone());
                                        }
                                        {
                                            let mut map = thread_to_session.lock().await;
                                            map.insert(response.thread.id.clone(), session_id);
                                        }
                                        if let Some(sync) = runtime_sync.as_ref() {
                                            if let Err(err) = sync
                                                .ensure_worker_for_thread(
                                                    &response.thread.id,
                                                    Some(session_id),
                                                )
                                                .await
                                            {
                                                let _ = proxy.send_event(AppEvent::AppServerEvent {
                                                    message: json!({
                                                        "method": "codex/error",
                                                        "params": {
                                                            "message": format!("Runtime worker sync failed: {err}")
                                                        }
                                                    })
                                                    .to_string(),
                                                });
                                            }
                                        }
                                        let _ = proxy.send_event(AppEvent::AppServerEvent {
                                            message: json!({
                                                "method": "thread/started",
                                                "params": {
                                                    "threadId": response.thread.id,
                                                    "model": model,
                                                    "sessionId": session_id.to_string()
                                                }
                                            })
                                            .to_string(),
                                        });
                                        response.thread.id
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::MoltbookLog {
                                            message: format!(
                                                "Moltbook reply session error: {err}"
                                            ),
                                        });
                                        return;
                                    }
                                };

                                {
                                    let mut guard = reply_targets.lock().await;
                                    guard.insert(
                                        thread_id.clone(),
                                        MoltbookReplyTarget {
                                            post_id: post_id.clone(),
                                        },
                                    );
                                }

                                let api_key = moltbook_api_key();
                                let proxy_client = MoltbookClient::with_base_url(
                                    moltbook_proxy_base(),
                                    api_key.clone(),
                                )
                                .ok();
                                let live_client = MoltbookClient::with_base_url(
                                    moltbook_live_base(),
                                    api_key.clone(),
                                )
                                .ok();

                                let mut post = load_moltbook_post(&post_id).ok().flatten();
                                let needs_content = post
                                    .as_ref()
                                    .and_then(|p| p.content.as_deref())
                                    .map(|c| c.trim().is_empty())
                                    .unwrap_or(true);
                                if needs_content {
                                    let fetched = if let Some(client) = proxy_client.as_ref() {
                                        client.posts_get(&post_id).await.ok()
                                    } else {
                                        None
                                    };
                                    let fetched = match fetched {
                                        Some(post) => Some(post),
                                        None => {
                                            if let Some(client) = live_client.as_ref() {
                                                client.posts_get(&post_id).await.ok()
                                            } else {
                                                None
                                            }
                                        }
                                    };
                                    if let Some(fetched) = fetched {
                                        let summary = MoltbookPostSummary {
                                            id: fetched.id,
                                            title: fetched.title,
                                            content_preview: fetched
                                                .content
                                                .as_deref()
                                                .map(|c| c.chars().take(120).collect()),
                                            content: fetched.content,
                                            author_name: fetched.author.map(|a| a.name),
                                            score: fetched.score,
                                            comment_count: fetched.comment_count,
                                            created_at: fetched.created_at,
                                            submolt: fetched.submolt,
                                        };
                                        if let Err(err) = store_moltbook_cache(&[summary.clone()]) {
                                            let _ = proxy.send_event(AppEvent::MoltbookLog {
                                                message: format!(
                                                    "Moltbook cache error: {err}"
                                                ),
                                            });
                                        }
                                        post = Some(summary);
                                    }
                                }

                                let comments = if let Some(client) = proxy_client.as_ref() {
                                    client
                                        .comments_list(&post_id, CommentSort::Top, Some(200))
                                        .await
                                } else {
                                    Err(MoltbookError::Api {
                                        status: 0,
                                        error: "Missing Moltbook proxy client".to_string(),
                                        hint: None,
                                    })
                                };
                                let comments = match comments {
                                    Ok(list) => Ok(list),
                                    Err(_) => {
                                        if let Some(client) = live_client.as_ref() {
                                            client
                                                .comments_list(
                                                    &post_id,
                                                    CommentSort::Top,
                                                    Some(200),
                                                )
                                                .await
                                        } else {
                                            Err(MoltbookError::Api {
                                                status: 0,
                                                error: "No Moltbook client".to_string(),
                                                hint: None,
                                            })
                                        }
                                    }
                                };

                                let comment_summaries = match comments {
                                    Ok(list) => list
                                        .into_iter()
                                        .map(moltbook_comment_summary)
                                        .collect::<Vec<_>>(),
                                    Err(_) => Vec::new(),
                                };

                                if !comment_summaries.is_empty() {
                                    let _ = proxy.send_event(
                                        AppEvent::MoltbookCommentsLoaded {
                                            post_id: post_id.clone(),
                                            comments: comment_summaries.clone(),
                                        },
                                    );
                                }

                                let prompt = build_moltbook_reply_prompt(
                                    &post_id,
                                    post.as_ref(),
                                    &comment_summaries,
                                );
                                let _ = proxy.send_event(AppEvent::MoltbookLog {
                                    message: "Generating Moltbook reply via Codex...".to_string(),
                                });

                                let thread_id_for_turn = thread_id.clone();
                                let params = TurnStartParams {
                                    thread_id: thread_id_for_turn,
                                    input: vec![UserInput::Text { text: prompt }],
                                    model: Some(DEFAULT_THREAD_MODEL.to_string()),
                                    effort: None,
                                    summary: None,
                                    approval_policy: Some(AskForApproval::Never),
                                    sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                                    cwd: Some(cwd),
                                };
                                if let Err(err) = client.turn_start(params).await {
                                    let _ = proxy.send_event(AppEvent::MoltbookLog {
                                        message: format!(
                                            "Moltbook reply start failed: {err}"
                                        ),
                                    });
                                    let mut guard = reply_targets.lock().await;
                                    guard.remove(&thread_id);
                                    return;
                                }

                            });
                        }
                        UserAction::MoltbookComment { post_id, text } => {
                            let proxy = proxy_actions.clone();
                            handle.spawn(async move {
                                let Some(api_key) = moltbook_api_key() else {
                                    let _ = proxy.send_event(AppEvent::MoltbookLog {
                                        message: "Moltbook comment failed: missing API key."
                                            .to_string(),
                                    });
                                    return;
                                };
                                let proxy_client = MoltbookClient::with_base_url(
                                    moltbook_proxy_base(),
                                    Some(api_key.clone()),
                                )
                                .ok();
                                let live_client = MoltbookClient::with_base_url(
                                    moltbook_live_base(),
                                    Some(api_key.clone()),
                                )
                                .ok();
                                let request = CreateCommentRequest {
                                    content: text.clone(),
                                    parent_id: None,
                                };
                                let result = if let Some(client) = proxy_client.as_ref() {
                                    client.comments_create(&post_id, request.clone()).await
                                } else if let Some(client) = live_client.as_ref() {
                                    client.comments_create(&post_id, request.clone()).await
                                } else {
                                    Err(MoltbookError::Api {
                                        status: 0,
                                        error: "No Moltbook client".to_string(),
                                        hint: None,
                                    })
                                };
                                match result {
                                    Ok(comment) => {
                                        let _ = proxy.send_event(AppEvent::MoltbookLog {
                                            message: format!(
                                                "Posted Moltbook comment ({}).",
                                                comment.id
                                            ),
                                        });
                                    }
                                    Err(err) => {
                                        let _ = proxy.send_event(AppEvent::MoltbookLog {
                                            message: format!(
                                                "Moltbook comment error: {err}"
                                            ),
                                        });
                                    }
                                }
                            });
                        }
                        UserAction::MoltbookSay { .. } => {
                            let _ = proxy_actions.send_event(AppEvent::MoltbookLog {
                                message: "Moltbook post creation is disabled in this build."
                                    .to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            });

            futures::future::pending::<()>().await;
        });
    });
}

fn extract_thread_id(params: Option<&Value>) -> Option<String> {
    let params = params?;
    if let Some(thread_id) = params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .and_then(|id| id.as_str())
    {
        return Some(thread_id.to_string());
    }
    params
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
}

fn extract_turn_id(params: Option<&Value>) -> Option<String> {
    let params = params?;
    if let Some(turn_id) = params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(|id| id.as_str())
    {
        return Some(turn_id.to_string());
    }
    params
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
}

fn guidance_goal_intent() -> String {
    env::var(ENV_GUIDANCE_GOAL)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_GUIDANCE_GOAL_INTENT.to_string())
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn moltbook_cache_path() -> Result<PathBuf> {
    let config_root = dirs::config_dir().or_else(|| {
        env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".config"))
    });
    let config_root = config_root.context("missing config dir")?;
    let dir = config_root.join("openagents").join("autopilot-desktop");
    fs::create_dir_all(&dir).context("create moltbook cache dir")?;
    Ok(dir.join(MOLTBOOK_CACHE_DB))
}

fn moltbook_cache_connection() -> Result<Connection> {
    let path = moltbook_cache_path()?;
    let conn = Connection::open(path).context("open moltbook cache db")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS moltbook_posts (
            id TEXT PRIMARY KEY,
            title TEXT,
            content_preview TEXT,
            content TEXT,
            author_name TEXT,
            score INTEGER,
            comment_count INTEGER,
            created_at TEXT,
            submolt TEXT,
            ingested_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_moltbook_posts_created ON moltbook_posts(created_at);
        CREATE INDEX IF NOT EXISTS idx_moltbook_posts_ingested ON moltbook_posts(ingested_at);",
    )
    .context("init moltbook cache schema")?;
    ensure_moltbook_cache_column(&conn, "content", "TEXT")?;
    Ok(conn)
}

fn ensure_moltbook_cache_column(conn: &Connection, name: &str, definition: &str) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(moltbook_posts)")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let column: String = row.get(1)?;
        if column == name {
            return Ok(());
        }
    }
    let ddl = format!("ALTER TABLE moltbook_posts ADD COLUMN {name} {definition}");
    conn.execute(&ddl, [])
        .context("alter moltbook cache schema")?;
    Ok(())
}

fn load_moltbook_cache(limit: usize) -> Result<Vec<MoltbookPostSummary>> {
    let conn = moltbook_cache_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, content_preview, content, author_name, score, comment_count, created_at, submolt
         FROM moltbook_posts
         ORDER BY
           CASE WHEN created_at IS NULL OR created_at = '' THEN 1 ELSE 0 END,
           created_at DESC,
           ingested_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(MoltbookPostSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            content_preview: row.get(2)?,
            content: row.get(3)?,
            author_name: row.get(4)?,
            score: row.get(5)?,
            comment_count: row.get(6)?,
            created_at: row.get(7)?,
            submolt: row.get(8)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

fn load_moltbook_post(post_id: &str) -> Result<Option<MoltbookPostSummary>> {
    let conn = moltbook_cache_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, content_preview, content, author_name, score, comment_count, created_at, submolt
         FROM moltbook_posts
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![post_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(MoltbookPostSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            content_preview: row.get(2)?,
            content: row.get(3)?,
            author_name: row.get(4)?,
            score: row.get(5)?,
            comment_count: row.get(6)?,
            created_at: row.get(7)?,
            submolt: row.get(8)?,
        }));
    }
    Ok(None)
}

fn store_moltbook_cache(posts: &[MoltbookPostSummary]) -> Result<()> {
    if posts.is_empty() {
        return Ok(());
    }
    let mut conn = moltbook_cache_connection()?;
    let tx = conn.transaction()?;
    let now = unix_timestamp();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO moltbook_posts
                (id, title, content_preview, content, author_name, score, comment_count, created_at, submolt, ingested_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content_preview = excluded.content_preview,
                content = excluded.content,
                author_name = excluded.author_name,
                score = excluded.score,
                comment_count = excluded.comment_count,
                created_at = excluded.created_at,
                submolt = excluded.submolt,
                ingested_at = excluded.ingested_at",
        )?;
        for post in posts {
            stmt.execute(params![
                post.id,
                post.title,
                post.content_preview,
                post.content,
                post.author_name,
                post.score,
                post.comment_count,
                post.created_at,
                post.submolt,
                now
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn merge_moltbook_posts(
    mut fresh: Vec<MoltbookPostSummary>,
    cached: Vec<MoltbookPostSummary>,
) -> Vec<MoltbookPostSummary> {
    let mut map: HashMap<String, MoltbookPostSummary> = HashMap::new();
    for post in cached {
        map.entry(post.id.clone()).or_insert(post);
    }
    for post in fresh.drain(..) {
        if let Some(existing) = map.remove(&post.id) {
            map.insert(post.id.clone(), merge_moltbook_post(existing, post));
        } else {
            map.insert(post.id.clone(), post);
        }
    }
    let mut merged: Vec<MoltbookPostSummary> = map.into_values().collect();
    merged.sort_by(|a, b| {
        let a_key = (
            a.created_at.is_none(),
            a.created_at.clone().unwrap_or_default(),
        );
        let b_key = (
            b.created_at.is_none(),
            b.created_at.clone().unwrap_or_default(),
        );
        b_key.cmp(&a_key)
    });
    if merged.len() > MOLTBOOK_CACHE_LIMIT {
        merged.truncate(MOLTBOOK_CACHE_LIMIT);
    }
    merged
}

fn merge_moltbook_post(
    existing: MoltbookPostSummary,
    incoming: MoltbookPostSummary,
) -> MoltbookPostSummary {
    MoltbookPostSummary {
        id: incoming.id,
        title: incoming.title.or(existing.title),
        content_preview: incoming.content_preview.or(existing.content_preview),
        content: incoming.content.or(existing.content),
        author_name: incoming.author_name.or(existing.author_name),
        score: incoming.score.or(existing.score),
        comment_count: incoming.comment_count.or(existing.comment_count),
        created_at: incoming.created_at.or(existing.created_at),
        submolt: incoming.submolt.or(existing.submolt),
    }
}

fn moltbook_comment_summary(comment: moltbook::Comment) -> MoltbookCommentSummary {
    MoltbookCommentSummary {
        id: comment.id,
        post_id: comment.post_id,
        parent_id: comment.parent_id,
        content: comment.content,
        author_name: comment.author.map(|a| a.name),
        score: comment.score,
        created_at: comment.created_at,
    }
}

fn extract_message_text(item: &Value) -> Option<String> {
    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
        for entry in content {
            if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn extract_assistant_reply(thread: &codex_client::ThreadSnapshot) -> Option<String> {
    for turn in thread.turns.iter().rev() {
        for item in turn.items.iter().rev() {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(
                item_type,
                "AgentMessage" | "agentMessage" | "assistantMessage" | "assistant"
            ) || role == "assistant"
            {
                if let Some(text) = extract_message_text(item) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn sanitize_moltbook_reply(text: &str) -> String {
    let mut trimmed = text.trim().to_string();
    if trimmed.starts_with("```") {
        trimmed = trimmed.trim_matches('`').trim().to_string();
    }
    trimmed
}

fn extract_moltbook_reply_body(text: &str) -> Option<String> {
    let markers = [
        "CONTENT:",
        "Draft comment content I attempted to post:",
        "Draft comment content:",
    ];
    for marker in markers {
        if let Some(index) = text.find(marker) {
            let mut slice = &text[index + marker.len()..];
            slice = slice.trim_start_matches([':', ' ', '\n']);
            let mut out = slice.trim().to_string();
            if out.starts_with("```") {
                out = out.trim_matches('`').trim().to_string();
            }
            if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
                out = out.trim_matches('"').to_string();
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

fn looks_like_refusal(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("i can't")
        || lower.contains("i cannot")
        || lower.contains("i’m sorry")
        || lower.contains("i am sorry")
        || lower.contains("cannot retrieve")
}

async fn fetch_moltbook_reply_text(
    client: &AppServerClient,
    thread_id: &str,
) -> Result<Option<String>> {
    let response = client
        .thread_resume(ThreadResumeParams {
            thread_id: thread_id.to_string(),
            ..Default::default()
        })
        .await?;
    Ok(extract_assistant_reply(&response.thread))
}

async fn resolve_moltbook_post_url(
    post_id: &str,
    proxy_client: Option<&MoltbookClient>,
    live_client: Option<&MoltbookClient>,
) -> Option<String> {
    if let Some(client) = proxy_client {
        if let Ok(post) = client.posts_get(post_id).await {
            if let Some(url) = post.url {
                return Some(url);
            }
        }
    }
    if let Some(client) = live_client {
        if let Ok(post) = client.posts_get(post_id).await {
            if let Some(url) = post.url {
                return Some(url);
            }
        }
    }
    None
}

async fn post_moltbook_reply_comment(
    proxy: EventLoopProxy<AppEvent>,
    post_id: &str,
    reply_text: &str,
) -> Result<()> {
    let Some(api_key) = moltbook_api_key() else {
        let _ = proxy.send_event(AppEvent::MoltbookLog {
            message: "Moltbook reply failed: missing API key.".to_string(),
        });
        return Ok(());
    };
    let proxy_client =
        MoltbookClient::with_base_url(moltbook_proxy_base(), Some(api_key.clone())).ok();
    let live_client =
        MoltbookClient::with_base_url(moltbook_live_base(), Some(api_key.clone())).ok();

    let request = CreateCommentRequest {
        content: reply_text.to_string(),
        parent_id: None,
    };
    let comment = if let Some(client) = proxy_client.as_ref() {
        client.comments_create(post_id, request.clone()).await
    } else if let Some(client) = live_client.as_ref() {
        client.comments_create(post_id, request.clone()).await
    } else {
        Err(MoltbookError::Api {
            status: 0,
            error: "No Moltbook client".to_string(),
            hint: None,
        })
    }?;

    let post_url = resolve_moltbook_post_url(post_id, proxy_client.as_ref(), live_client.as_ref())
        .await
        .unwrap_or_else(|| format!("https://www.moltbook.com/posts/{post_id}"));

    let _ = proxy.send_event(AppEvent::MoltbookLog {
        message: format!(
            "Posted reply on Moltbook: {post_url} (comment {}).",
            comment.id
        ),
    });

    let _ = load_moltbook_comments(proxy, post_id).await;

    Ok(())
}

async fn load_moltbook_comments(proxy: EventLoopProxy<AppEvent>, post_id: &str) -> Result<()> {
    let api_key = moltbook_api_key();
    let proxy_client = MoltbookClient::with_base_url(moltbook_proxy_base(), api_key.clone()).ok();
    let live_client = MoltbookClient::with_base_url(moltbook_live_base(), api_key.clone()).ok();

    let comments = if let Some(client) = proxy_client.as_ref() {
        client
            .comments_list(post_id, CommentSort::Top, Some(200))
            .await
    } else {
        Err(MoltbookError::Api {
            status: 0,
            error: "Missing Moltbook proxy client".to_string(),
            hint: None,
        })
    };
    let comments = match comments {
        Ok(list) => Ok(list),
        Err(err) => {
            let _ = proxy.send_event(AppEvent::MoltbookLog {
                message: format!("Moltbook comments proxy error: {err}"),
            });
            if let Some(client) = live_client.as_ref() {
                client
                    .comments_list(post_id, CommentSort::Top, Some(200))
                    .await
            } else {
                Err(err)
            }
        }
    };

    match comments {
        Ok(list) => {
            let summaries = list
                .into_iter()
                .map(moltbook_comment_summary)
                .collect::<Vec<_>>();
            let _ = proxy.send_event(AppEvent::MoltbookCommentsLoaded {
                post_id: post_id.to_string(),
                comments: summaries,
            });
        }
        Err(err) => {
            let _ = proxy.send_event(AppEvent::MoltbookLog {
                message: format!("Moltbook comments error: {err}"),
            });
        }
    }

    Ok(())
}

fn build_moltbook_reply_prompt(
    post_id: &str,
    post: Option<&MoltbookPostSummary>,
    comments: &[MoltbookCommentSummary],
) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are drafting a public Moltbook reply.\n");
    prompt.push_str("Use at most ONE tool call, only for this:\n");
    prompt.push_str("curl -fsSL https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main/MOLTBOOK.md\n");
    prompt.push_str("Do not run any other commands.\n");
    prompt.push_str("After reading, output ONLY the reply text (no extra commentary).\n");
    prompt.push_str("Do not mention tools, keys, or APIs.\n");
    prompt.push_str("Do not pause or ask for confirmation.\n");
    prompt.push_str("Keep it concise, helpful, and aligned with the post.\n\n");

    prompt.push_str(&format!("Post ID: {post_id}\n"));
    if let Some(post) = post {
        let title = post.title.as_deref().unwrap_or("(no title)");
        let author = post.author_name.as_deref().unwrap_or("?");
        let submolt = post.submolt.as_deref().unwrap_or("");
        let created = post.created_at.as_deref().unwrap_or("");
        let content = post
            .content
            .as_deref()
            .or_else(|| post.content_preview.as_deref())
            .unwrap_or("");

        prompt.push_str("Post:\n");
        prompt.push_str(&format!("Title: {title}\n"));
        prompt.push_str(&format!("Author: {author}\n"));
        if !submolt.is_empty() {
            prompt.push_str(&format!("Submolt: {submolt}\n"));
        }
        if !created.is_empty() {
            prompt.push_str(&format!("Created: {created}\n"));
        }
        prompt.push_str("Content:\n");
        prompt.push_str(content);
        prompt.push_str("\n\n");
    } else {
        prompt.push_str("Post content unavailable.\n\n");
    }

    if comments.is_empty() {
        prompt.push_str("Comments: (none yet)\n");
    } else {
        prompt.push_str("Comments:\n");
        for comment in comments {
            let author = comment.author_name.as_deref().unwrap_or("?");
            let score = comment
                .score
                .map(|s| s.to_string())
                .unwrap_or_else(|| "—".to_string());
            let created = comment.created_at.as_deref().unwrap_or("");
            let content = comment.content.as_deref().unwrap_or("");
            let mut line = format!("- {author} · {score} ↑");
            if !created.is_empty() {
                line.push_str(&format!(" · {created}"));
            }
            prompt.push_str(&line);
            prompt.push('\n');
            if !content.is_empty() {
                prompt.push_str(content);
                prompt.push('\n');
            }
        }
    }

    prompt
}

fn trimmed_env_url(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn moltbook_proxy_base() -> String {
    if let Some(value) = trimmed_env_url(ENV_MOLTBOOK_PROXY_BASE) {
        return value;
    }
    if let Some(oa_api) = trimmed_env_url("OA_API") {
        return format!("{oa_api}/moltbook/api");
    }
    DEFAULT_MOLTBOOK_PROXY_BASE.to_string()
}

fn moltbook_live_base() -> String {
    trimmed_env_url(ENV_MOLTBOOK_LIVE_BASE)
        .unwrap_or_else(|| DEFAULT_MOLTBOOK_LIVE_BASE.to_string())
}

fn moltbook_api_key() -> Option<String> {
    if let Ok(key) = env::var("MOLTBOOK_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Some(key);
        }
    }
    let home = env::var("HOME").ok()?;
    let config_dir = Path::new(&home).join(".config/moltbook");
    let json: Value = match fs::read_to_string(config_dir.join("credentials.json"))
        .or_else(|_| fs::read_to_string(&config_dir))
    {
        Ok(data) => serde_json::from_str(&data).ok()?,
        Err(_) => return None,
    };
    json.get("api_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[allow(dead_code)]
fn is_super_trigger(message: &str) -> bool {
    let trimmed = message.trim().to_lowercase();
    let normalized = trimmed
        .trim_matches(|ch: char| !ch.is_alphanumeric() && !ch.is_whitespace())
        .to_string();
    matches!(
        normalized.as_str(),
        "go" | "go ahead"
            | "do it"
            | "just do it"
            | "do the thing"
            | "do this"
            | "execute"
            | "run it"
            | "continue"
            | "proceed"
            | "ship it"
            | "make it happen"
    )
}

#[allow(dead_code)]
#[allow(dead_code)]
fn guidance_response_score(text: &str) -> f32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if trimmed.contains('\n') || trimmed.len() > 280 {
        return 0.0;
    }
    if trimmed.contains('?') || trimmed.contains('`') || trimmed.contains('\"') {
        return 0.0;
    }
    let lower = trimmed.to_lowercase();
    let banned = [
        "i am an ai",
        "i'm an ai",
        "as an ai",
        "i do not have",
        "i don't have",
        "i cannot",
        "i can't",
        "i am a language model",
    ];
    if banned.iter().any(|phrase| lower.contains(phrase)) {
        return 0.0;
    }
    1.0
}

fn sanitize_guidance_line(text: &str, max_len: usize) -> String {
    let mut line = text.lines().next().unwrap_or("").trim().to_string();
    line = line.trim_matches('"').trim_matches('\'').trim().to_string();
    if line.len() > max_len {
        line.truncate(max_len);
    }
    line
}

fn sanitize_guidance_response(text: &str) -> String {
    sanitize_guidance_line(text, 280)
}

fn sanitize_guidance_directive(text: &str) -> String {
    sanitize_guidance_line(text, 600)
}

#[allow(dead_code)]
fn fallback_guidance_response(goal_intent: &str) -> String {
    if goal_intent.trim().is_empty() {
        "Summarize the request and propose the next concrete step.".to_string()
    } else {
        format!(
            "Summarize the request and propose the next step toward: {}.",
            goal_intent.trim()
        )
    }
}

async fn resolve_guidance_lm(cached_lm: Option<dsrs::LM>) -> Result<(dsrs::LM, bool), String> {
    if let Some(lm) = cached_lm.clone()
        && lm.model.starts_with("codex:")
    {
        return Ok((lm, false));
    }
    if let Ok(lm) = ensure_codex_lm(&decision_model()).await {
        return Ok((lm, true));
    }
    if let Some(lm) = cached_lm {
        return Ok((lm, false));
    }
    let lm = ensure_guidance_demo_lm().await?;
    Ok((lm, true))
}

fn prediction_to_string(prediction: &dsrs::Prediction, key: &str) -> String {
    match prediction.get(key, None) {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "".to_string(),
        other => other.to_string(),
    }
}

fn guidance_repo_context() -> String {
    env::current_dir()
        .ok()
        .map(|path| format!("Repo path: {}", path.display()))
        .unwrap_or_else(|| "Repo path: unknown".to_string())
}

#[derive(Debug, Clone)]
struct RepoIntel {
    root: PathBuf,
    status: String,
    recent_commits: String,
    recent_files: Vec<String>,
    doc_notes: Vec<String>,
    task_hint: Option<String>,
    issues_hint: Option<String>,
    dirty: bool,
}

fn find_repo_root() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(root) = run_git(&cwd, &["rev-parse", "--show-toplevel"]) {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    cwd
}

fn run_git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(text)
}

fn truncate_text(mut text: String, max_len: usize) -> String {
    if text.len() > max_len {
        text.truncate(max_len);
    }
    text
}

fn extract_task_hint(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.to_lowercase().starts_with("todo")
            || trimmed.to_lowercase().starts_with("next")
        {
            return Some(trimmed.trim_matches(['-', '*', ' ']).to_string());
        }
    }
    None
}

fn read_doc_snippet(repo_root: &Path, rel: &str) -> Option<(String, String)> {
    let path = repo_root.join(rel);
    if !path.exists() {
        return None;
    }
    let contents = fs::read_to_string(&path).ok()?;
    let snippet = contents.lines().take(12).collect::<Vec<_>>().join(" ");
    let hint = extract_task_hint(&contents).or_else(|| {
        contents
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.to_string())
    });
    let note = hint.unwrap_or_else(|| snippet.clone());
    Some((rel.to_string(), truncate_text(note, 200)))
}

fn collect_repo_intel() -> RepoIntel {
    let repo_root = find_repo_root();
    let status = run_git(&repo_root, &["status", "-sb"]).unwrap_or_else(|| "unknown".to_string());
    let dirty = status.lines().skip(1).any(|line| !line.trim().is_empty());
    let recent_commits = run_git(
        &repo_root,
        &[
            "log",
            "-n",
            "10",
            "--pretty=format:%h %ad %s",
            "--date=short",
        ],
    )
    .unwrap_or_default();
    let recent_files_raw = run_git(
        &repo_root,
        &["log", "-n", "5", "--name-only", "--pretty=format:"],
    )
    .unwrap_or_default();
    let mut recent_files = Vec::new();
    for line in recent_files_raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !recent_files.contains(&trimmed.to_string()) {
            recent_files.push(trimmed.to_string());
        }
        if recent_files.len() >= 8 {
            break;
        }
    }

    let doc_candidates = [
        "ROADMAP.md",
        "PROJECT_OVERVIEW.md",
        "README.md",
        "TODO.md",
        "TASKS.md",
        "ISSUES.md",
        "BACKLOG.md",
        "docs/WORK_LOG.md",
        "docs/ROADMAP.md",
        "docs/ISSUES.md",
        "docs/PROJECTS.md",
        "docs/TODO.md",
    ];
    let mut doc_notes = Vec::new();
    let mut task_hint = None;
    for rel in doc_candidates {
        if let Some((name, note)) = read_doc_snippet(&repo_root, rel) {
            if task_hint.is_none() {
                task_hint = Some(note.clone());
            }
            doc_notes.push(format!("{name}: {note}"));
        }
    }

    let issues_hint = fs::read_dir(repo_root.join("issues"))
        .ok()
        .and_then(|entries| {
            let mut names = Vec::new();
            for entry in entries.flatten().take(5) {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
            if names.is_empty() {
                None
            } else {
                Some(names.join(", "))
            }
        });

    RepoIntel {
        root: repo_root,
        status,
        recent_commits,
        recent_files,
        doc_notes,
        task_hint,
        issues_hint,
        dirty,
    }
}

fn summarize_repo_intel(intel: &RepoIntel) -> String {
    let status_hint = if intel.dirty { "dirty" } else { "clean" };
    let commit_count = intel
        .recent_commits
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let docs_count = intel.doc_notes.len();
    let mut parts = vec![
        format!("status {status_hint}"),
        format!("commits {commit_count}"),
        format!("docs {docs_count}"),
    ];
    if let Some(issue) = intel.issues_hint.as_ref() {
        parts.push(format!("issues {}", issue));
    }
    format!("Repo research: {}.", parts.join(", "))
}

fn build_repo_intel_payload(intel: &RepoIntel) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Repo root: {}", intel.root.display()));
    if !intel.status.trim().is_empty() {
        parts.push(format!("Git status:\n{}", intel.status.trim()));
    }
    if !intel.recent_commits.trim().is_empty() {
        parts.push(format!("Recent commits:\n{}", intel.recent_commits.trim()));
    }
    if !intel.recent_files.is_empty() {
        parts.push(format!("Recent files:\n{}", intel.recent_files.join(", ")));
    }
    if !intel.doc_notes.is_empty() {
        parts.push(format!("Docs:\n{}", intel.doc_notes.join("\n")));
    }
    if let Some(issue) = intel.issues_hint.as_ref() {
        parts.push(format!("Issues:\n{issue}"));
    }
    truncate_text(parts.join("\n\n"), 2000)
}

fn build_followup_task_summary(summary: &FullAutoTurnSummary) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Last status: {}.", summary.last_turn_status));
    if !summary.turn_error.trim().is_empty() {
        parts.push(format!("Error: {}", summary.turn_error.trim()));
    }
    if !summary.last_guidance_directive.trim().is_empty() {
        parts.push(format!(
            "Last directive: {}",
            summary.last_guidance_directive.trim()
        ));
    }
    if !summary.last_agent_message.trim().is_empty() {
        parts.push(format!(
            "Last agent output: {}",
            summary.last_agent_message.trim()
        ));
    }
    if !summary.turn_plan.trim().is_empty() {
        parts.push(format!("Plan: {}", summary.turn_plan.trim()));
    }
    if !summary.diff_summary.trim().is_empty() {
        parts.push(format!("Diff: {}", summary.diff_summary.trim()));
    }
    if !summary.recent_actions.trim().is_empty() {
        parts.push(format!("Recent actions: {}", summary.recent_actions.trim()));
    }
    truncate_text(parts.join("\n"), 2000)
}

fn fallback_directive(intel: &RepoIntel, goal_intent: &str, plan_step: Option<&str>) -> String {
    if let Some(step) = plan_step {
        return sanitize_guidance_response(&format!("Proceed with: {}.", step));
    }
    if let Some(task) = intel.task_hint.as_ref() {
        return sanitize_guidance_response(&format!("Advance task: {}.", task));
    }
    if !goal_intent.trim().is_empty() {
        return sanitize_guidance_response(&format!("Proceed with: {}.", goal_intent.trim()));
    }
    sanitize_guidance_response("Review git status/log and continue the highest-priority open task.")
}

fn emit_guidance_step(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    signature: &str,
    text: &str,
    model: &str,
) {
    let payload = json!({
        "method": "guidance/step",
        "params": {
            "threadId": thread_id,
            "signature": signature,
            "text": text,
            "model": model
        }
    });
    let _ = proxy.send_event(AppEvent::AppServerEvent {
        message: payload.to_string(),
    });
}

fn emit_guidance_status(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    signature: Option<&str>,
    text: &str,
) {
    let payload = json!({
        "method": "guidance/status",
        "params": {
            "threadId": thread_id,
            "signature": signature,
            "text": text
        }
    });
    let _ = proxy.send_event(AppEvent::AppServerEvent {
        message: payload.to_string(),
    });
}

fn extract_first_json_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return None;
    }
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        for item in items {
            if let Some(text) = item.as_str() {
                if !text.trim().is_empty() {
                    return Some(text.trim().to_string());
                }
            } else if !item.is_null() {
                let text = item.to_string();
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    let first = trimmed
        .split('\n')
        .map(str::trim)
        .find(|line| !line.is_empty());
    first.map(|line| line.trim_matches('"').trim_matches('\'').to_string())
}

fn extract_first_step_description(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return None;
    }
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        for item in items {
            if let Some(desc) = item.get("description").and_then(|value| value.as_str()) {
                if !desc.trim().is_empty() {
                    return Some(desc.trim().to_string());
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
fn strip_question_marks(text: &str) -> String {
    text.replace('?', "").trim().to_string()
}

fn is_question_like(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower.contains('?') {
        return true;
    }
    let cues = [
        "clarify",
        "could you",
        "can you",
        "please provide",
        "need more",
        "what do you want",
        "which task",
        "specific task",
        "details",
    ];
    cues.iter().any(|cue| lower.contains(cue))
}

#[allow(dead_code)]
async fn run_task_understanding(
    message: &str,
    repo_context: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<String, String> {
    let predictor = Predict::new(TaskUnderstandingSignature::new());
    let inputs = example! {
        "user_request": "input" => message.to_string(),
        "repo_context": "input" => repo_context.to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(inputs, lm)
        .await
        .map_err(|e| format!("Task understanding failed: {e}"))?;
    let task_type = prediction_to_string(&prediction, "task_type");
    let requirements_raw = prediction_to_string(&prediction, "requirements");
    let questions_raw = prediction_to_string(&prediction, "clarifying_questions");
    if let Some(question) = extract_first_json_string(&questions_raw) {
        let question = strip_question_marks(&question);
        return Ok(sanitize_guidance_response(&format!(
            "Need clarification: {}.",
            question
        )));
    }
    if let Some(requirement) = extract_first_json_string(&requirements_raw) {
        let task_type = if task_type.trim().is_empty() {
            "Task".to_string()
        } else {
            task_type
        };
        return Ok(sanitize_guidance_response(&format!(
            "{} focus: {}.",
            task_type, requirement
        )));
    }
    Ok(fallback_guidance_response(goal_intent))
}

#[allow(dead_code)]
async fn run_planning_summary(
    message: &str,
    repo_context: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<String, String> {
    let predictor = Predict::new(PlanningSignature::new());
    let inputs = example! {
        "task_description": "input" => message.to_string(),
        "repo_context": "input" => repo_context.to_string(),
        "file_tree": "input" => "".to_string(),
        "context_summary": "input" => "".to_string(),
        "constraints": "input" => "full_auto_guidance".to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(inputs, lm)
        .await
        .map_err(|e| format!("Planning failed: {e}"))?;
    let steps_raw = prediction_to_string(&prediction, "steps");
    if let Some(step) = extract_first_step_description(&steps_raw) {
        return Ok(sanitize_guidance_response(&format!("Next step: {}.", step)));
    }
    Ok(fallback_guidance_response(goal_intent))
}

#[allow(dead_code)]
async fn handle_guidance_route(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    route: &str,
    response: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    let mut signatures = vec!["GuidanceRouterSignature".to_string()];
    let repo_context = guidance_repo_context();
    match route.trim().to_lowercase().as_str() {
        "understand" => {
            emit_guidance_status(
                proxy,
                thread_id,
                Some("TaskUnderstandingSignature"),
                "Running…",
            );
            let text = run_task_understanding(message, &repo_context, goal_intent, lm).await?;
            signatures.push("TaskUnderstandingSignature".to_string());
            Ok((text, signatures))
        }
        "plan" => {
            emit_guidance_status(proxy, thread_id, Some("PlanningSignature"), "Running…");
            let text = run_planning_summary(message, &repo_context, goal_intent, lm).await?;
            signatures.push("PlanningSignature".to_string());
            Ok((text, signatures))
        }
        _ => {
            let text = if response.trim().is_empty() {
                fallback_guidance_response(goal_intent)
            } else {
                sanitize_guidance_response(response)
            };
            Ok((text, signatures))
        }
    }
}

async fn run_guidance_super(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    let mut signatures = Vec::new();
    emit_guidance_status(proxy, thread_id, Some("RepoResearch"), "Running...");
    let repo_intel = collect_repo_intel();
    let repo_payload = build_repo_intel_payload(&repo_intel);
    let repo_summary = summarize_repo_intel(&repo_intel);
    emit_guidance_step(proxy, thread_id, "RepoResearch", &repo_summary, "local");
    let repo_context = format!("{} | {}", guidance_repo_context(), repo_summary);
    let lm_arc = std::sync::Arc::new(lm.clone());

    emit_guidance_status(
        proxy,
        thread_id,
        Some("TaskUnderstandingSignature"),
        "Running…",
    );
    let understanding_predictor = Predict::new(TaskUnderstandingSignature::new());
    let understanding_inputs = example! {
        "user_request": "input" => message.to_string(),
        "repo_context": "input" => repo_context.clone(),
    };
    let understanding = understanding_predictor
        .forward_with_config(understanding_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Task understanding failed: {e}"))?;
    let task_type = prediction_to_string(&understanding, "task_type");
    let requirements_raw = prediction_to_string(&understanding, "requirements");
    let questions_raw = prediction_to_string(&understanding, "clarifying_questions");
    signatures.push("TaskUnderstandingSignature".to_string());
    let requirement = extract_first_json_string(&requirements_raw);
    let question = extract_first_json_string(&questions_raw);
    let step_text = if let Some(requirement) = requirement.as_ref() {
        let task_label = if task_type.trim().is_empty() {
            "Task".to_string()
        } else {
            task_type.clone()
        };
        sanitize_guidance_response(&format!("{} focus: {}.", task_label, requirement))
    } else if question.is_some() {
        let intent = if goal_intent.trim().is_empty() {
            "continue with the next logical task".to_string()
        } else {
            goal_intent.trim().to_string()
        };
        sanitize_guidance_response(&format!("Assumed intent: {}.", intent))
    } else {
        sanitize_guidance_response("Assumed intent: continue with the next logical task.")
    };
    emit_guidance_step(
        proxy,
        thread_id,
        "TaskUnderstandingSignature",
        &step_text,
        &lm.model,
    );

    emit_guidance_status(proxy, thread_id, Some("PlanningSignature"), "Running…");
    let planning_predictor = Predict::new(PlanningSignature::new());
    let planning_message = if message.trim().len() <= 4 {
        if goal_intent.trim().is_empty() {
            "Continue with the next logical task.".to_string()
        } else {
            goal_intent.to_string()
        }
    } else {
        message.to_string()
    };
    let planning_inputs = example! {
        "task_description": "input" => planning_message,
        "repo_context": "input" => repo_context,
        "file_tree": "input" => "".to_string(),
        "context_summary": "input" => "".to_string(),
        "constraints": "input" => "full_auto_guidance".to_string(),
    };
    let planning = planning_predictor
        .forward_with_config(planning_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Planning failed: {e}"))?;
    let steps_raw = prediction_to_string(&planning, "steps");
    let first_step = extract_first_step_description(&steps_raw);
    signatures.push("PlanningSignature".to_string());
    let plan_step_summary = if let Some(step) = first_step.as_ref() {
        let step_text = sanitize_guidance_response(&format!("Plan step: {}.", step));
        emit_guidance_step(proxy, thread_id, "PlanningSignature", &step_text, &lm.model);
        step_text
    } else {
        emit_guidance_step(
            proxy,
            thread_id,
            "PlanningSignature",
            "Plan ready.",
            &lm.model,
        );
        "Plan ready.".to_string()
    };

    let task_summary = format!("{} {}", step_text, plan_step_summary);
    emit_guidance_status(
        proxy,
        thread_id,
        Some("GuidanceDirectiveSignature"),
        "Running...",
    );
    let directive_predictor = Predict::new(GuidanceDirectiveSignature::new());
    let directive_inputs = example! {
        "goal_intent": "input" => goal_intent.to_string(),
        "repo_intel": "input" => repo_payload,
        "task_summary": "input" => task_summary,
    };
    let directive = directive_predictor
        .forward_with_config(directive_inputs, lm_arc)
        .await
        .map_err(|e| format!("Guidance directive failed: {e}"))?;
    let directive_raw = prediction_to_string(&directive, "directive");
    signatures.push("GuidanceDirectiveSignature".to_string());
    let mut directive_text = if !directive_raw.trim().is_empty() {
        directive_raw.trim().to_string()
    } else {
        fallback_directive(&repo_intel, goal_intent, first_step.as_deref())
    };
    if is_question_like(&directive_text) {
        directive_text = fallback_directive(&repo_intel, goal_intent, first_step.as_deref());
    }
    emit_guidance_step(
        proxy,
        thread_id,
        "GuidanceDirectiveSignature",
        &sanitize_guidance_directive(&directive_text),
        &lm.model,
    );

    let final_response = sanitize_guidance_directive(&directive_text);

    Ok((final_response, signatures))
}

async fn run_guidance_followup(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    summary: &FullAutoTurnSummary,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<FullAutoDecisionResult, String> {
    emit_guidance_status(proxy, thread_id, Some("RepoResearch"), "Running...");
    let repo_intel = collect_repo_intel();
    let repo_payload = build_repo_intel_payload(&repo_intel);
    let repo_summary = summarize_repo_intel(&repo_intel);
    emit_guidance_step(proxy, thread_id, "RepoResearch", &repo_summary, "local");

    let task_summary = build_followup_task_summary(summary);
    let repo_context = format!("{} | {}", guidance_repo_context(), repo_summary);
    let lm_arc = std::sync::Arc::new(lm.clone());
    let mut signatures = Vec::new();

    emit_guidance_status(
        proxy,
        thread_id,
        Some("TaskUnderstandingSignature"),
        "Running…",
    );
    let understanding_predictor = Predict::new(TaskUnderstandingSignature::new());
    let understanding_inputs = example! {
        "user_request": "input" => format!("Continue work. {task_summary}"),
        "repo_context": "input" => repo_context.clone(),
    };
    let understanding = understanding_predictor
        .forward_with_config(understanding_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Task understanding failed: {e}"))?;
    let task_type = prediction_to_string(&understanding, "task_type");
    let requirements_raw = prediction_to_string(&understanding, "requirements");
    signatures.push("TaskUnderstandingSignature".to_string());
    let requirement = extract_first_json_string(&requirements_raw);
    let step_text = if let Some(requirement) = requirement.as_ref() {
        let task_label = if task_type.trim().is_empty() {
            "Task".to_string()
        } else {
            task_type.clone()
        };
        sanitize_guidance_response(&format!("{} focus: {}.", task_label, requirement))
    } else {
        sanitize_guidance_response("Assumed focus: keep advancing the highest-priority task.")
    };
    emit_guidance_step(
        proxy,
        thread_id,
        "TaskUnderstandingSignature",
        &step_text,
        &lm.model,
    );

    emit_guidance_status(proxy, thread_id, Some("PlanningSignature"), "Running…");
    let planning_predictor = Predict::new(PlanningSignature::new());
    let planning_inputs = example! {
        "task_description": "input" => format!("Continue work. {task_summary}"),
        "repo_context": "input" => repo_context,
        "file_tree": "input" => "".to_string(),
        "context_summary": "input" => "".to_string(),
        "constraints": "input" => "full_auto_guidance".to_string(),
    };
    let planning = planning_predictor
        .forward_with_config(planning_inputs, lm_arc.clone())
        .await
        .map_err(|e| format!("Planning failed: {e}"))?;
    let steps_raw = prediction_to_string(&planning, "steps");
    let first_step = extract_first_step_description(&steps_raw);
    signatures.push("PlanningSignature".to_string());
    let plan_step_summary = if let Some(step) = first_step.as_ref() {
        let step_text = sanitize_guidance_response(&format!("Plan step: {}.", step));
        emit_guidance_step(proxy, thread_id, "PlanningSignature", &step_text, &lm.model);
        step_text
    } else {
        emit_guidance_step(
            proxy,
            thread_id,
            "PlanningSignature",
            "Plan ready.",
            &lm.model,
        );
        "Plan ready.".to_string()
    };

    emit_guidance_status(
        proxy,
        thread_id,
        Some("GuidanceDirectiveSignature"),
        "Running...",
    );
    let directive_predictor = Predict::new(GuidanceDirectiveSignature::new());
    let directive_inputs = example! {
        "goal_intent": "input" => goal_intent.to_string(),
        "repo_intel": "input" => repo_payload,
        "task_summary": "input" => format!("{} {}", step_text, plan_step_summary),
    };
    let directive = directive_predictor
        .forward_with_config(directive_inputs, lm_arc)
        .await
        .map_err(|e| format!("Guidance directive failed: {e}"))?;
    let directive_raw = prediction_to_string(&directive, "directive");
    signatures.push("GuidanceDirectiveSignature".to_string());
    let mut directive_text = if !directive_raw.trim().is_empty() {
        directive_raw.trim().to_string()
    } else {
        fallback_directive(&repo_intel, goal_intent, first_step.as_deref())
    };
    if is_question_like(&directive_text) {
        directive_text = fallback_directive(&repo_intel, goal_intent, first_step.as_deref());
    }
    emit_guidance_step(
        proxy,
        thread_id,
        "GuidanceDirectiveSignature",
        &sanitize_guidance_directive(&directive_text),
        &lm.model,
    );

    let final_response = sanitize_guidance_directive(&directive_text);
    let decision = FullAutoDecision {
        action: FullAutoAction::Continue,
        next_input: Some(final_response.clone()),
        reason: "Guidance loop directive".to_string(),
        confidence: 0.75,
        guardrail: None,
    };
    let diagnostics = FullAutoDecisionDiagnostics {
        raw_prediction: json!({
            "directive": directive_raw,
            "task_summary": task_summary,
            "repo_summary": repo_summary,
            "signatures": signatures,
        }),
        action_raw: Some("continue".to_string()),
        next_input_raw: Some(final_response.clone()),
        reason_raw: Some("Guidance loop directive".to_string()),
        confidence_raw: Some(Value::from(0.75)),
        action_parsed: "continue".to_string(),
        next_input_parsed: final_response.clone(),
        reason_parsed: "Guidance loop directive".to_string(),
        confidence_parsed: 0.75,
        parse_errors: Vec::new(),
    };

    Ok(FullAutoDecisionResult {
        decision,
        diagnostics,
    })
}

#[allow(dead_code)]
async fn run_guidance_router(
    proxy: &EventLoopProxy<AppEvent>,
    thread_id: &str,
    message: &str,
    goal_intent: &str,
    lm: &dsrs::LM,
) -> Result<(String, Vec<String>), String> {
    emit_guidance_status(
        proxy,
        thread_id,
        Some("GuidanceRouterSignature"),
        "Running…",
    );
    let predictor = Predict::new(GuidanceRouterSignature::new());
    let inputs = example! {
        "user_message": "input" => message.to_string(),
        "goal_intent": "input" => goal_intent.to_string(),
        "context": "input" => "full_auto".to_string(),
    };
    let lm = std::sync::Arc::new(lm.clone());
    let mut best: Option<(String, String, f32)> = None;
    for _ in 0..3 {
        let prediction = predictor
            .forward_with_config(inputs.clone(), lm.clone())
            .await
            .map_err(|e| format!("Guidance router failed: {e}"))?;
        let response = prediction_to_string(&prediction, "response");
        let route = prediction_to_string(&prediction, "route");
        let mut score = guidance_response_score(&response);
        let route_norm = route.trim().to_lowercase();
        if matches!(route_norm.as_str(), "plan" | "understand") {
            score = score.max(0.9);
        }
        if score >= 0.9 {
            return handle_guidance_route(
                proxy,
                thread_id,
                message,
                goal_intent,
                &route,
                &response,
                lm.as_ref(),
            )
            .await;
        }
        if best.as_ref().map(|(_, _, s)| score > *s).unwrap_or(true) {
            best = Some((response, route, score));
        }
    }
    if let Some((response, route, score)) = best {
        if score > 0.0 {
            return handle_guidance_route(
                proxy,
                thread_id,
                message,
                goal_intent,
                &route,
                &response,
                lm.as_ref(),
            )
            .await;
        }
    }
    Ok((
        fallback_guidance_response(goal_intent),
        vec!["GuidanceRouterSignature".to_string()],
    ))
}

fn build_tool_input_response(params: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    let questions = params
        .get("questions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    for question in questions {
        let id = question
            .get("id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let answer = question
            .get("options")
            .and_then(|value| value.as_array())
            .and_then(|options| options.first())
            .and_then(|option| option.get("id"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "yes".to_string());

        if let Some(id) = id {
            answers.insert(
                id,
                json!({
                    "answers": [answer],
                }),
            );
        }
    }

    json!({ "answers": answers })
}

fn build_auto_response(method: &str, params: Option<&Value>) -> Option<Value> {
    match method {
        "execCommandApproval" | "applyPatchApproval" => Some(json!({ "decision": "approved" })),
        "item/tool/requestUserInput" => params.map(build_tool_input_response),
        _ => None,
    }
}

fn load_pylon_config_ollama() -> Result<PylonConfig> {
    let mut config = PylonConfig::load()?;
    config.backend_preference = vec!["ollama".to_string()];
    if config.default_model.trim().is_empty() {
        config.default_model = "llama3.2".to_string();
    }
    Ok(config)
}

fn identity_path_for_config(config: &PylonConfig) -> Result<PathBuf> {
    Ok(config.data_path()?.join("identity.mnemonic"))
}

fn pylon_identity_exists(config: &PylonConfig) -> bool {
    identity_path_for_config(config)
        .map(|path| path.exists())
        .unwrap_or(false)
}

fn load_or_init_identity(config: &PylonConfig) -> Result<UnifiedIdentity> {
    let identity_path = identity_path_for_config(config)?;
    if identity_path.exists() {
        let mnemonic = std::fs::read_to_string(&identity_path)?.trim().to_string();
        return UnifiedIdentity::from_mnemonic(&mnemonic, "")
            .map_err(|err| anyhow::anyhow!("Failed to load identity: {err}"));
    }

    if let Some(parent) = identity_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let identity = UnifiedIdentity::generate()
        .map_err(|err| anyhow::anyhow!("Failed to generate identity: {err}"))?;
    std::fs::write(&identity_path, identity.mnemonic())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(identity)
}

fn pylon_status_error(err: impl Into<String>) -> PylonStatus {
    PylonStatus {
        last_error: Some(err.into()),
        ..PylonStatus::default()
    }
}

fn dvm_provider_status_error(err: impl Into<String>) -> DvmProviderStatus {
    DvmProviderStatus {
        last_error: Some(err.into()),
        ..DvmProviderStatus::default()
    }
}

async fn init_pylon_identity(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    match load_or_init_identity(config) {
        Ok(_) => state.last_error = None,
        Err(err) => state.last_error = Some(err.to_string()),
    }
    refresh_pylon_status(state, config).await
}

async fn start_pylon_in_process(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    if let Some(provider) = state.provider.as_ref() {
        let provider_status = provider.status().await;
        if provider_status.running {
            state.last_error = None;
            return refresh_pylon_status(state, config).await;
        }
    }

    let identity = match load_or_init_identity(config) {
        Ok(identity) => identity,
        Err(err) => {
            state.last_error = Some(err.to_string());
            return refresh_pylon_status(state, config).await;
        }
    };

    let mut provider = match state.provider.take() {
        Some(provider) => provider,
        None => match PylonProvider::new(config.clone()).await {
            Ok(provider) => provider,
            Err(err) => {
                state.last_error = Some(err.to_string());
                return refresh_pylon_status(state, config).await;
            }
        },
    };

    if let Err(err) = provider.init_with_identity(identity).await {
        state.last_error = Some(err.to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    let provider_status = provider.status().await;
    if !provider_status
        .backends
        .iter()
        .any(|backend| backend == "ollama")
    {
        state.last_error = Some("Ollama backend not detected on localhost:11434.".to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    match provider.start().await {
        Ok(()) | Err(ProviderError::AlreadyRunning) => {
            if state.started_at.is_none() {
                state.started_at = Some(std::time::Instant::now());
            }
            state.last_error = None;
            state.provider = Some(provider);
        }
        Err(err) => {
            state.last_error = Some(err.to_string());
            state.provider = None;
            state.started_at = None;
        }
    }

    refresh_pylon_status(state, config).await
}

async fn stop_pylon_in_process(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    if let Some(provider) = state.provider.as_mut() {
        match provider.stop().await {
            Ok(()) | Err(ProviderError::NotRunning) => {
                state.started_at = None;
                state.last_error = None;
            }
            Err(err) => {
                state.last_error = Some(err.to_string());
            }
        }
    } else {
        state.started_at = None;
    }

    refresh_pylon_status(state, config).await
}

async fn refresh_pylon_status(state: &mut InProcessPylon, config: &PylonConfig) -> PylonStatus {
    let identity_exists = pylon_identity_exists(config);
    let (running, jobs_completed, earnings_msats) = if let Some(provider) = state.provider.as_ref()
    {
        let provider_status = provider.status().await;
        (
            provider_status.running,
            provider_status.jobs_processed,
            provider_status.total_earnings_msats,
        )
    } else {
        (false, 0, 0)
    };

    if running && state.started_at.is_none() {
        state.started_at = Some(std::time::Instant::now());
    }
    if !running {
        state.started_at = None;
    }

    PylonStatus {
        running,
        pid: None,
        uptime_secs: state.started_at.as_ref().map(|t| t.elapsed().as_secs()),
        provider_active: Some(running),
        host_active: Some(false),
        jobs_completed,
        earnings_msats,
        identity_exists,
        last_error: state.last_error.clone(),
    }
}

async fn fetch_dvm_provider_status(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> DvmProviderStatus {
    let running = if let Some(provider) = state.provider.as_ref() {
        provider.status().await.running
    } else {
        false
    };

    DvmProviderStatus {
        running,
        provider_active: Some(running),
        host_active: Some(false),
        min_price_msats: config.min_price_msats,
        require_payment: config.require_payment,
        default_model: config.default_model.clone(),
        backend_preference: config.backend_preference.clone(),
        network: config.network.clone(),
        enable_payments: config.enable_payments,
        last_error: state.last_error.clone(),
    }
}

fn fetch_dvm_history() -> DvmHistorySnapshot {
    let mut snapshot = DvmHistorySnapshot::default();

    let config = match PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load Pylon config: {err}"));
            return snapshot;
        }
    };

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to resolve Pylon data dir: {err}"));
            return snapshot;
        }
    };

    let path = data_dir.join("pylon.db");

    let db = match PylonDb::open(path) {
        Ok(db) => db,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to open Pylon DB: {err}"));
            return snapshot;
        }
    };

    match db.get_earnings_summary() {
        Ok(summary) => {
            snapshot.summary.total_msats = summary.total_msats;
            snapshot.summary.total_sats = summary.total_sats;
            snapshot.summary.job_count = summary.job_count;
            let mut sources = summary
                .by_source
                .into_iter()
                .collect::<Vec<(String, u64)>>();
            sources.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.summary.by_source = sources;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load earnings summary: {err}"));
        }
    }

    match db.count_jobs_by_status() {
        Ok(counts) => {
            let mut status_counts = counts
                .into_iter()
                .map(|(status, count)| (status.as_str().to_string(), count))
                .collect::<Vec<_>>();
            status_counts.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.status_counts = status_counts;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load job counts: {err}"));
        }
    }

    let mut jobs = Vec::new();
    for status in [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Processing,
        JobStatus::Pending,
    ] {
        if let Ok(list) = db.list_jobs_by_status(status, 25) {
            jobs.extend(list);
        }
    }
    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    jobs.truncate(25);
    snapshot.jobs = jobs
        .into_iter()
        .map(|job| autopilot_app::DvmJobSummary {
            id: job.id,
            status: job.status.as_str().to_string(),
            kind: job.kind,
            price_msats: job.price_msats,
            created_at: job.created_at,
        })
        .collect();

    snapshot
}

fn spark_network_for_pylon(network: &str) -> SparkNetwork {
    match network.to_lowercase().as_str() {
        "mainnet" => SparkNetwork::Mainnet,
        "testnet" => SparkNetwork::Testnet,
        "signet" => SparkNetwork::Signet,
        _ => SparkNetwork::Regtest,
    }
}

async fn fetch_wallet_status() -> WalletStatus {
    let mut status = WalletStatus {
        network: None,
        spark_sats: 0,
        lightning_sats: 0,
        onchain_sats: 0,
        total_sats: 0,
        spark_address: None,
        bitcoin_address: None,
        identity_exists: false,
        last_error: None,
    };

    let config = match PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            status.last_error = Some(format!("Failed to load Pylon config: {err}"));
            return status;
        }
    };

    status.network = Some(config.network.clone());

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            status.last_error = Some(format!("Failed to resolve Pylon data dir: {err}"));
            return status;
        }
    };
    let identity_path = data_dir.join("identity.mnemonic");
    if !identity_path.exists() {
        status.identity_exists = false;
        status.last_error = Some(format!(
            "No identity found. Run 'pylon init' first. Expected: {}",
            identity_path.display()
        ));
        return status;
    }
    status.identity_exists = true;

    let mnemonic = match std::fs::read_to_string(&identity_path) {
        Ok(value) => value.trim().to_string(),
        Err(err) => {
            status.last_error = Some(format!("Failed to read identity: {err}"));
            return status;
        }
    };

    let signer = match SparkSigner::from_mnemonic(&mnemonic, "") {
        Ok(signer) => signer,
        Err(err) => {
            status.last_error = Some(format!("Failed to derive Spark signer: {err}"));
            return status;
        }
    };

    let wallet_config = WalletConfig {
        network: spark_network_for_pylon(&config.network),
        api_key: None,
        storage_dir: data_dir.join("spark"),
    };

    let wallet = match SparkWallet::new(signer, wallet_config).await {
        Ok(wallet) => wallet,
        Err(err) => {
            status.last_error = Some(format!("Failed to init Spark wallet: {err}"));
            return status;
        }
    };

    match wallet.get_balance().await {
        Ok(balance) => {
            status.spark_sats = balance.spark_sats;
            status.lightning_sats = balance.lightning_sats;
            status.onchain_sats = balance.onchain_sats;
            status.total_sats = balance.total_sats();
        }
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch balance: {err}"));
            return status;
        }
    }

    match wallet.get_spark_address().await {
        Ok(address) => status.spark_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Spark address: {err}"));
        }
    }

    match wallet.get_bitcoin_address().await {
        Ok(address) => status.bitcoin_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Bitcoin address: {err}"));
        }
    }

    status
}

fn map_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named) => match named {
            WinitNamedKey::Enter => Some(Key::Named(NamedKey::Enter)),
            WinitNamedKey::Escape => Some(Key::Named(NamedKey::Escape)),
            WinitNamedKey::Backspace => Some(Key::Named(NamedKey::Backspace)),
            WinitNamedKey::Delete => Some(Key::Named(NamedKey::Delete)),
            WinitNamedKey::Tab => Some(Key::Named(NamedKey::Tab)),
            WinitNamedKey::Space => Some(Key::Named(NamedKey::Space)),
            WinitNamedKey::Home => Some(Key::Named(NamedKey::Home)),
            WinitNamedKey::End => Some(Key::Named(NamedKey::End)),
            WinitNamedKey::PageUp => Some(Key::Named(NamedKey::PageUp)),
            WinitNamedKey::PageDown => Some(Key::Named(NamedKey::PageDown)),
            WinitNamedKey::ArrowUp => Some(Key::Named(NamedKey::ArrowUp)),
            WinitNamedKey::ArrowDown => Some(Key::Named(NamedKey::ArrowDown)),
            WinitNamedKey::ArrowLeft => Some(Key::Named(NamedKey::ArrowLeft)),
            WinitNamedKey::ArrowRight => Some(Key::Named(NamedKey::ArrowRight)),
            _ => None,
        },
        WinitKey::Character(ch) => Some(Key::Character(ch.to_string())),
        _ => None,
    }
}

fn to_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

// DesktopRoot moved to `crates/autopilot_ui`.
