//! Local control plane HTTP server and runtime registry.

use crate::agent::Agent;
use crate::env::AgentEnv;
use crate::envelope::Envelope;
use crate::error::Result;
use crate::fs::FsError;
use crate::storage::AgentStorage;
use crate::tick::TickResult;
use crate::trigger::Trigger;
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{manual_trigger, StatusSnapshot, TickEngine};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

/// Local runtime registry with in-process agents.
pub struct LocalRuntime {
    storage: Arc<dyn AgentStorage>,
    tick_engine: TickEngine,
    agents: Arc<RwLock<HashMap<AgentId, Arc<AgentEntry>>>>,
}

impl LocalRuntime {
    /// Create a local runtime using the provided storage backend.
    pub fn new(storage: Arc<dyn AgentStorage>) -> Self {
        Self {
            tick_engine: TickEngine::new(storage.clone()),
            storage,
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent instance with the runtime.
    pub async fn register_agent<A: Agent>(&self, id: AgentId, agent: A) -> Result<()> {
        let env = Arc::new(AgentEnv::new(id.clone(), self.storage.clone()));
        let entry = Arc::new(AgentEntry::new(id, env, agent));
        let mut guard = self.agents.write().await;
        guard.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// List all registered agent ids.
    pub async fn list_agents(&self) -> Vec<AgentId> {
        let guard = self.agents.read().await;
        guard.keys().cloned().collect()
    }

    /// Fetch an agent entry.
    async fn entry(&self, id: &AgentId) -> Option<Arc<AgentEntry>> {
        let guard = self.agents.read().await;
        guard.get(id).cloned()
    }

    /// Send an envelope to an agent's inbox.
    pub async fn send_envelope(&self, id: &AgentId, envelope: Envelope) -> Result<()> {
        let entry = self
            .entry(id)
            .await
            .ok_or_else(|| "agent not found".to_string())?;
        let data = serde_json::to_vec(&envelope)?;
        entry.env.write("/inbox", &data).map_err(|err| err.to_string())?;
        entry.refresh_status().await;
        Ok(())
    }

    /// Trigger a manual tick for an agent.
    pub async fn tick_manual(&self, id: &AgentId) -> Result<TickResult> {
        let entry = self
            .entry(id)
            .await
            .ok_or_else(|| "agent not found".to_string())?;
        let envelope_id = EnvelopeId::new(Uuid::new_v4().to_string());
        let trigger = manual_trigger(envelope_id, "control-plane");
        let result = entry
            .agent
            .tick(&self.tick_engine, entry.id.clone(), trigger)
            .await?;
        entry.record_tick("manual").await;
        Ok(result)
    }
}

struct AgentEntry {
    id: AgentId,
    env: Arc<AgentEnv>,
    agent: Arc<dyn DynAgent>,
    created_at: Timestamp,
    state: Mutex<AgentRuntimeState>,
}

impl AgentEntry {
    fn new<A: Agent>(id: AgentId, env: Arc<AgentEnv>, agent: A) -> Self {
        let created_at = Timestamp::now();
        let entry = Self {
            id,
            env,
            agent: Arc::new(AgentRunner::new(agent)),
            created_at,
            state: Mutex::new(AgentRuntimeState::new()),
        };
        entry
    }

    async fn record_tick(&self, cause: &str) {
        let mut state = self.state.lock().await;
        state.tick_count += 1;
        state.last_tick_at = Some(Timestamp::now());
        state.last_tick_cause = Some(cause.to_string());
        drop(state);
        self.refresh_status().await;
    }

    async fn refresh_status(&self) {
        let queue_depth = {
            let queue_ref = self.env.inbox.queue();
            let queue = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
            queue.len() as u64
        };
        let state = self.state.lock().await;
        let payload = serde_json::json!({
            "state": "active",
            "created_at": self.created_at.as_millis(),
            "last_tick_at": state.last_tick_at.map(|t| t.as_millis()),
            "last_tick_cause": state.last_tick_cause.clone(),
            "tick_count": state.tick_count,
            "queue_depth": queue_depth,
        });
        let snapshot = StatusSnapshot {
            agent_id: self.id.clone(),
            payload,
        };
        self.env.status.set_snapshot(snapshot);
    }

    async fn info(&self) -> AgentInfo {
        let queue_depth = {
            let queue_ref = self.env.inbox.queue();
            let queue = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
            queue.len() as u64
        };
        let state = self.state.lock().await;
        AgentInfo {
            id: self.id.to_string(),
            created_at: self.created_at,
            tick_count: state.tick_count,
            last_tick_at: state.last_tick_at,
            last_tick_cause: state.last_tick_cause.clone(),
            queue_depth,
        }
    }
}

#[derive(Default)]
struct AgentRuntimeState {
    tick_count: u64,
    last_tick_at: Option<Timestamp>,
    last_tick_cause: Option<String>,
}

impl AgentRuntimeState {
    fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
trait DynAgent: Send + Sync {
    async fn tick(&self, engine: &TickEngine, agent_id: AgentId, trigger: Trigger)
        -> Result<TickResult>;
}

struct AgentRunner<A: Agent> {
    agent: A,
}

impl<A: Agent> AgentRunner<A> {
    fn new(agent: A) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl<A: Agent> DynAgent for AgentRunner<A> {
    async fn tick(
        &self,
        engine: &TickEngine,
        agent_id: AgentId,
        trigger: Trigger,
    ) -> Result<TickResult> {
        engine.tick(agent_id, &self.agent, trigger).await
    }
}

/// HTTP control plane for a local runtime.
#[derive(Clone)]
pub struct ControlPlane {
    runtime: Arc<LocalRuntime>,
}

impl ControlPlane {
    /// Create a control plane for a runtime.
    pub fn new(runtime: Arc<LocalRuntime>) -> Self {
        Self { runtime }
    }

    /// Build an axum router implementing the control plane endpoints.
    pub fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/agents", get(list_agents))
            .route("/agents/:id", get(agent_info))
            .route("/agents/:id/tick", post(tick_agent))
            .route("/agents/:id/send", post(send_agent))
            .route(
                "/agents/:id/*path",
                get(read_path).post(write_path),
            )
            .with_state(self.runtime.clone())
    }
}

#[derive(Serialize)]
struct AgentInfo {
    id: String,
    created_at: Timestamp,
    tick_count: u64,
    last_tick_at: Option<Timestamp>,
    last_tick_cause: Option<String>,
    queue_depth: u64,
}

#[derive(Deserialize)]
struct QueryParams {
    watch: Option<String>,
}

async fn list_agents(State(runtime): State<Arc<LocalRuntime>>) -> impl IntoResponse {
    let agents = runtime.list_agents().await;
    let ids: Vec<String> = agents.into_iter().map(|id| id.to_string()).collect();
    Json(ids)
}

async fn agent_info(
    State(runtime): State<Arc<LocalRuntime>>,
    Path(id): Path<String>,
) -> Response {
    let agent_id = AgentId::new(id);
    let entry = match runtime.entry(&agent_id).await {
        Some(entry) => entry,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let info = entry.info().await;
    Json(info).into_response()
}

async fn tick_agent(
    State(runtime): State<Arc<LocalRuntime>>,
    Path(id): Path<String>,
) -> Response {
    let agent_id = AgentId::new(id);
    match runtime.tick_manual(&agent_id).await {
        Ok(result) => Json(result).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn send_agent(
    State(runtime): State<Arc<LocalRuntime>>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let agent_id = AgentId::new(id);
    let envelope = parse_envelope(body);
    match runtime.send_envelope(&agent_id, envelope).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn read_path(
    State(runtime): State<Arc<LocalRuntime>>,
    Path((id, path)): Path<(String, String)>,
    Query(params): Query<QueryParams>,
) -> Response {
    let agent_id = AgentId::new(id);
    let entry = match runtime.entry(&agent_id).await {
        Some(entry) => entry,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let fs_path = format!("/{}", path);
    let watch = params
        .watch
        .as_deref()
        .map(|value| matches!(value, "1" | "true" | "yes"))
        .unwrap_or(false);

    if watch {
        return match entry.env.watch(&fs_path) {
            Ok(Some(handle)) => watch_response(handle),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(err) => map_fs_error(err).into_response(),
        };
    }

    if fs_path == "/logs/trace" {
        return StatusCode::BAD_REQUEST.into_response();
    }

    if path == "status" {
        entry.refresh_status().await;
    }

    if path.ends_with('/') {
        return match entry.env.list(&fs_path) {
            Ok(entries) => Json(entries).into_response(),
            Err(err) => map_fs_error(err).into_response(),
        };
    }

    match entry.env.read(&fs_path) {
        Ok(bytes) => bytes_response(bytes),
        Err(FsError::IsDirectory) => match entry.env.list(&fs_path) {
            Ok(entries) => Json(entries).into_response(),
            Err(err) => map_fs_error(err).into_response(),
        },
        Err(err) => map_fs_error(err).into_response(),
    }
}

async fn write_path(
    State(runtime): State<Arc<LocalRuntime>>,
    Path((id, path)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let agent_id = AgentId::new(id);
    let entry = match runtime.entry(&agent_id).await {
        Some(entry) => entry,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let fs_path = format!("/{}", path);
    match entry.env.write(&fs_path, &body) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => map_fs_error(err).into_response(),
    }
}

fn parse_envelope(body: Bytes) -> Envelope {
    if let Ok(envelope) = serde_json::from_slice::<Envelope>(&body) {
        return envelope;
    }

    let payload = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(_) => serde_json::json!({
            "message": String::from_utf8_lossy(&body).to_string(),
        }),
    };

    Envelope {
        id: EnvelopeId::new(Uuid::new_v4().to_string()),
        timestamp: Timestamp::now(),
        payload,
    }
}

fn bytes_response(bytes: Vec<u8>) -> Response {
    let content_type = if is_probably_json(&bytes) {
        "application/json"
    } else if std::str::from_utf8(&bytes).is_ok() {
        "text/plain"
    } else {
        "application/octet-stream"
    };

    let mut response = Response::new(bytes.into());
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static(content_type));
    response
}

fn watch_response(handle: Box<dyn crate::fs::WatchHandle>) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::task::spawn_blocking(move || {
        let mut handle = handle;
        loop {
            match handle.next(Some(Duration::from_secs(15))) {
                Ok(Some(event)) => {
                    if tx.blocking_send(event).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(_) => break,
            }
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        let data = match event {
            crate::fs::WatchEvent::Data(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            crate::fs::WatchEvent::Modified { path } => {
                serde_json::json!({"event": "modified", "path": path}).to_string()
            }
            crate::fs::WatchEvent::Created { path } => {
                serde_json::json!({"event": "created", "path": path}).to_string()
            }
            crate::fs::WatchEvent::Deleted { path } => {
                serde_json::json!({"event": "deleted", "path": path}).to_string()
            }
        };
        Ok::<Event, Infallible>(Event::default().data(data))
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"))
        .into_response()
}

fn is_probably_json(bytes: &[u8]) -> bool {
    let first = bytes.iter().copied().find(|b| !b.is_ascii_whitespace());
    matches!(first, Some(b'{') | Some(b'['))
        && serde_json::from_slice::<serde_json::Value>(bytes).is_ok()
}

fn map_fs_error(err: FsError) -> StatusCode {
    match err {
        FsError::NotFound => StatusCode::NOT_FOUND,
        FsError::PermissionDenied => StatusCode::FORBIDDEN,
        FsError::AlreadyExists => StatusCode::CONFLICT,
        FsError::NotDirectory | FsError::IsDirectory => StatusCode::BAD_REQUEST,
        FsError::InvalidPath => StatusCode::BAD_REQUEST,
        FsError::BudgetExceeded => StatusCode::PAYMENT_REQUIRED,
        FsError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        FsError::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
