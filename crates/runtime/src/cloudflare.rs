//! Cloudflare Durable Object runtime backend.

use crate::agent::Agent;
use crate::budget::BudgetPolicy;
use crate::compute::{CloudflareProvider, ComputeFs, ComputePolicy, ComputeRouter};
use crate::env::AgentEnv;
use crate::envelope::Envelope;
use crate::error::{AgentError, Result};
use crate::fs::FsError;
use crate::idempotency::DoJournal;
use crate::storage::CloudflareStorage;
use crate::tick::TickResult;
use crate::trigger::{AlarmTrigger, Trigger, TriggerMeta};
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{manual_trigger, StatusSnapshot, TickEngine};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};
use worker::{durable_object, DurableObject, Env, Method, Request, Response, State};

const DEFAULT_AI_BINDING: &str = "AI";
const ALARM_STORAGE_KEY: &str = "__runtime_alarm";

type AgentFactory = dyn Fn(AgentId) -> Arc<dyn DynAgent> + Send + Sync;

static AGENT_FACTORY: OnceLock<Arc<AgentFactory>> = OnceLock::new();

/// Register the agent factory used by the Cloudflare backend.
pub fn set_cloudflare_agent_factory<F, A>(factory: F) -> Result<()>
where
    F: Fn(AgentId) -> A + Send + Sync + 'static,
    A: Agent,
{
    let wrapped = move |agent_id: AgentId| -> Arc<dyn DynAgent> {
        Arc::new(AgentRunner::new(factory(agent_id)))
    };
    AGENT_FACTORY
        .set(Arc::new(wrapped))
        .map_err(|_| AgentError::Tick("cloudflare agent factory already set".to_string()))?;
    Ok(())
}

#[async_trait]
trait DynAgent: Send + Sync {
    async fn tick(
        &self,
        engine: &TickEngine,
        agent_id: AgentId,
        trigger: Trigger,
    ) -> Result<TickResult>;
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

struct AgentRuntime {
    id: AgentId,
    env: Arc<AgentEnv>,
    agent: Arc<dyn DynAgent>,
    tick_engine: TickEngine,
    created_at: Timestamp,
    state: Mutex<AgentRuntimeState>,
}

impl AgentRuntime {
    fn new(
        id: AgentId,
        env: Arc<AgentEnv>,
        agent: Arc<dyn DynAgent>,
        tick_engine: TickEngine,
    ) -> Self {
        Self {
            id,
            env,
            agent,
            tick_engine,
            created_at: Timestamp::now(),
            state: Mutex::new(AgentRuntimeState::new()),
        }
    }

    async fn tick(&self, trigger: Trigger, cause: &str) -> Result<TickResult> {
        let result = self
            .agent
            .tick(&self.tick_engine, self.id.clone(), trigger)
            .await?;
        self.record_tick(cause);
        Ok(result)
    }

    fn record_tick(&self, cause: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.tick_count = state.tick_count.saturating_add(1);
        state.last_tick_at = Some(Timestamp::now());
        state.last_tick_cause = Some(cause.to_string());
        drop(state);
        self.refresh_status();
    }

    fn refresh_status(&self) {
        let queue_depth = {
            let queue_ref = self.env.inbox.queue();
            let queue = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
            queue.len() as u64
        };
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
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

    fn info(&self) -> AgentInfo {
        let queue_depth = {
            let queue_ref = self.env.inbox.queue();
            let queue = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
            queue.len() as u64
        };
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        AgentInfo {
            id: self.id.to_string(),
            created_at: self.created_at,
            tick_count: state.tick_count,
            last_tick_at: state.last_tick_at,
            last_tick_cause: state.last_tick_cause.clone(),
            queue_depth,
        }
    }

    fn send_envelope(&self, envelope: Envelope) -> Result<()> {
        let data = serde_json::to_vec(&envelope)?;
        self.env.write("/inbox", &data).map_err(|err| err.to_string())?;
        self.refresh_status();
        Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlarmState {
    alarm_id: String,
    scheduled_at: Timestamp,
    payload: Option<Vec<u8>>,
}

#[durable_object]
pub struct CloudflareAgent {
    state: State,
    env: Env,
    runtime: Mutex<Option<Arc<AgentRuntime>>>,
    agent_id: Mutex<Option<AgentId>>,
}

impl DurableObject for CloudflareAgent {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            runtime: Mutex::new(None),
            agent_id: Mutex::new(None),
        }
    }

    async fn fetch(&self, mut req: Request) -> worker::Result<Response> {
        let url = req.url()?;
        let (path, has_trailing_slash) = normalize_path(url.path());
        let watch = query_watch(&url);

        match (req.method(), path.as_str()) {
            (Method::Post, "/tick") => self.handle_tick().await,
            (Method::Post, "/send") => self.handle_send(&mut req).await,
            (Method::Get, "/info") => self.handle_info().await,
            (Method::Get, "/") if !has_trailing_slash => self.handle_info().await,
            (Method::Get, _) => self.handle_read(&path, has_trailing_slash, watch).await,
            (Method::Post, _) => self.handle_write(&path, &mut req).await,
            _ => Response::error("Method Not Allowed", 405),
        }
    }

    async fn alarm(&self) -> worker::Result<Response> {
        let alarm_state = self.load_alarm().await?;
        let Some(alarm_state) = alarm_state else {
            return Response::empty();
        };

        let trigger = Trigger::Alarm(AlarmTrigger {
            meta: TriggerMeta {
                envelope_id: EnvelopeId::new(uuid::Uuid::new_v4().to_string()),
                source: "alarm".to_string(),
                seq: None,
                created_at: Timestamp::now(),
            },
            alarm_id: alarm_state.alarm_id,
            scheduled_at: alarm_state.scheduled_at,
            fired_at: Timestamp::now(),
            payload: alarm_state.payload,
        });

        let runtime = self.runtime().map_err(map_agent_error)?;
        let result = runtime.tick(trigger, "alarm").await.map_err(map_agent_error)?;
        self.apply_alarm_result(&result).await?;
        Response::from_json(&result)
    }
}

impl CloudflareAgent {
    fn runtime(&self) -> Result<Arc<AgentRuntime>> {
        let mut guard = self.runtime.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(runtime) = guard.as_ref() {
            return Ok(runtime.clone());
        }

        let agent_id = self.resolve_agent_id();
        let storage = Arc::new(CloudflareStorage::new(self.state.storage().sql())?);
        let tick_engine = TickEngine::new(storage.clone());
        let mut env = AgentEnv::new(agent_id.clone(), storage);

        if let Some(compute_fs) = self.compute_fs(&agent_id)? {
            env.mount("/compute", Arc::new(compute_fs), crate::fs::AccessLevel::ReadWrite);
        }

        let factory = AGENT_FACTORY
            .get()
            .ok_or_else(|| AgentError::Tick("cloudflare agent factory not set".to_string()))?;
        let agent = factory(agent_id.clone());

        let runtime = Arc::new(AgentRuntime::new(agent_id, Arc::new(env), agent, tick_engine));
        *guard = Some(runtime.clone());
        Ok(runtime)
    }

    fn compute_fs(&self, agent_id: &AgentId) -> Result<Option<ComputeFs>> {
        let binding = resolve_ai_binding(&self.env);
        let ai = match self.env.ai(&binding) {
            Ok(ai) => ai,
            Err(_) => return Ok(None),
        };

        let journal = DoJournal::new(self.state.storage().sql(), "compute:")
            .map_err(|err| AgentError::Tick(err.to_string()))?;
        let mut router = ComputeRouter::new();
        router.register(Arc::new(CloudflareProvider::new(ai)));

        let mut policy = ComputePolicy::default();
        policy.require_max_cost = true;

        let budget_policy = budget_policy_from_env(&self.env);
        let compute = ComputeFs::new(
            agent_id.clone(),
            router,
            policy,
            budget_policy,
            Arc::new(journal),
        );
        Ok(Some(compute))
    }

    fn resolve_agent_id(&self) -> AgentId {
        let mut guard = self.agent_id.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(id) = guard.as_ref() {
            return id.clone();
        }
        let id = self
            .state
            .id()
            .name()
            .unwrap_or_else(|| self.state.id().to_string());
        let agent_id = AgentId::new(id);
        *guard = Some(agent_id.clone());
        agent_id
    }

    async fn handle_tick(&self) -> worker::Result<Response> {
        let runtime = self.runtime().map_err(map_agent_error)?;
        let envelope_id = EnvelopeId::new(uuid::Uuid::new_v4().to_string());
        let trigger = manual_trigger(envelope_id, "control-plane");
        let result = runtime.tick(trigger, "manual").await.map_err(map_agent_error)?;
        self.apply_alarm_result(&result).await?;
        Response::from_json(&result)
    }

    async fn handle_send(&self, req: &mut Request) -> worker::Result<Response> {
        let body = req.bytes().await?;
        let envelope = parse_envelope(&body);
        let runtime = self.runtime().map_err(map_agent_error)?;
        runtime.send_envelope(envelope).map_err(map_agent_error)?;
        Ok(Response::empty()?.with_status(204))
    }

    async fn handle_info(&self) -> worker::Result<Response> {
        let runtime = self.runtime().map_err(map_agent_error)?;
        Response::from_json(&runtime.info())
    }

    async fn handle_read(
        &self,
        path: &str,
        has_trailing_slash: bool,
        watch: bool,
    ) -> worker::Result<Response> {
        if watch {
            return Response::error("watch not supported", 400);
        }

        let runtime = self.runtime().map_err(map_agent_error)?;

        if path == "/logs/trace" {
            return Response::error("trace access requires watch", 400);
        }

        if path == "/status" {
            runtime.refresh_status();
        }

        if has_trailing_slash {
            return match runtime.env.list(path) {
                Ok(entries) => Response::from_json(&entries),
                Err(err) => Response::error(err.to_string(), map_fs_error(err)),
            };
        }

        match runtime.env.read(path) {
            Ok(bytes) => bytes_response(bytes),
            Err(FsError::IsDirectory) => match runtime.env.list(path) {
                Ok(entries) => Response::from_json(&entries),
                Err(err) => Response::error(err.to_string(), map_fs_error(err)),
            },
            Err(err) => Response::error(err.to_string(), map_fs_error(err)),
        }
    }

    async fn handle_write(&self, path: &str, req: &mut Request) -> worker::Result<Response> {
        let runtime = self.runtime().map_err(map_agent_error)?;
        let body = req.bytes().await?;
        match runtime.env.call_admin(path, &body) {
            Ok(bytes) if bytes.is_empty() => Ok(Response::empty()?.with_status(204)),
            Ok(bytes) => bytes_response(bytes),
            Err(err) => Response::error(err.to_string(), map_fs_error(err)),
        }
    }

    async fn apply_alarm_result(&self, result: &TickResult) -> worker::Result<()> {
        if let Some(next_alarm) = result.next_alarm {
            let alarm_state = AlarmState {
                alarm_id: uuid::Uuid::new_v4().to_string(),
                scheduled_at: next_alarm,
                payload: None,
            };
            self.schedule_alarm(alarm_state).await?;
        } else {
            self.clear_alarm().await?;
        }
        Ok(())
    }

    async fn schedule_alarm(&self, alarm: AlarmState) -> worker::Result<()> {
        let now_ms = Timestamp::now().as_millis() as i64;
        let scheduled_ms = alarm.scheduled_at.as_millis() as i64;
        let offset_ms = scheduled_ms - now_ms;
        self.state.storage().put(ALARM_STORAGE_KEY, alarm).await?;
        self.state.storage().set_alarm(offset_ms).await?;
        Ok(())
    }

    async fn clear_alarm(&self) -> worker::Result<()> {
        let _ = self.state.storage().delete(ALARM_STORAGE_KEY).await?;
        self.state.storage().delete_alarm().await?;
        Ok(())
    }

    async fn load_alarm(&self) -> worker::Result<Option<AlarmState>> {
        self.state.storage().get(ALARM_STORAGE_KEY).await
    }
}

fn resolve_ai_binding(env: &Env) -> String {
    env.var("OPENAGENTS_AI_BINDING")
        .ok()
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_AI_BINDING.to_string())
}

fn budget_policy_from_env(env: &Env) -> BudgetPolicy {
    let per_tick = env
        .var("OPENAGENTS_BUDGET_TICK_USD")
        .ok()
        .and_then(|value| value.to_string().parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    let per_day = env
        .var("OPENAGENTS_BUDGET_DAY_USD")
        .ok()
        .and_then(|value| value.to_string().parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    let approval = env
        .var("OPENAGENTS_BUDGET_APPROVAL_USD")
        .ok()
        .and_then(|value| value.to_string().parse::<u64>().ok())
        .unwrap_or(0);
    BudgetPolicy {
        per_tick_usd: per_tick,
        per_day_usd: per_day,
        approval_threshold_usd: approval,
        approvers: Vec::new(),
    }
}

fn normalize_path(path: &str) -> (String, bool) {
    let has_trailing = path.ends_with('/') && path.len() > 1;
    let trimmed = path.trim_start_matches('/');
    let mut parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts.first() == Some(&"agents") && parts.len() >= 2 {
        parts.drain(0..2);
    }
    if parts.is_empty() {
        ("/".to_string(), has_trailing)
    } else {
        (format!("/{}", parts.join("/")), has_trailing)
    }
}

fn query_watch(url: &worker::Url) -> bool {
    url.query_pairs().any(|(key, value)| {
        key == "watch" && matches!(value.as_ref(), "1" | "true" | "yes")
    })
}

fn parse_envelope(body: &[u8]) -> Envelope {
    if let Ok(envelope) = serde_json::from_slice::<Envelope>(body) {
        return envelope;
    }

    let payload = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(value) => value,
        Err(_) => serde_json::json!({
            "message": String::from_utf8_lossy(body).to_string(),
        }),
    };

    Envelope {
        id: EnvelopeId::new(uuid::Uuid::new_v4().to_string()),
        timestamp: Timestamp::now(),
        payload,
    }
}

fn bytes_response(bytes: Vec<u8>) -> worker::Result<Response> {
    let content_type = if is_probably_json(&bytes) {
        "application/json"
    } else if std::str::from_utf8(&bytes).is_ok() {
        "text/plain"
    } else {
        "application/octet-stream"
    };
    let mut response = Response::from_bytes(bytes)?;
    response.headers_mut().set("content-type", content_type)?;
    Ok(response)
}

fn is_probably_json(bytes: &[u8]) -> bool {
    let first = bytes.iter().copied().find(|b| !b.is_ascii_whitespace());
    matches!(first, Some(b'{') | Some(b'['))
        && serde_json::from_slice::<serde_json::Value>(bytes).is_ok()
}

fn map_fs_error(err: FsError) -> u16 {
    match err {
        FsError::NotFound => 404,
        FsError::PermissionDenied => 403,
        FsError::AlreadyExists => 409,
        FsError::NotDirectory | FsError::IsDirectory | FsError::InvalidPath => 400,
        FsError::BudgetExceeded => 402,
        FsError::Io(_) | FsError::Other(_) => 500,
    }
}

fn map_agent_error(err: AgentError) -> worker::Error {
    worker::Error::RustError(err.to_string())
}
