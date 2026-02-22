//! Shared iOS Codex state/domain primitives migrated from Swift models.
//! These types are intentionally platform-neutral so iOS/web/desktop can reuse
//! deterministic behavior for auth flow races, reconnect policy, control request
//! reconciliation, replay dedupe, and streaming message assembly.

use std::collections::{HashMap, HashSet, VecDeque};

use openagents_codex_control::ControlMethod;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAuthVerifyContext {
    pub generation: u64,
    pub challenge_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodexAuthFlowState {
    generation: u64,
    pending_email: Option<String>,
    pending_challenge_id: Option<String>,
}

impl CodexAuthFlowState {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn pending_email(&self) -> Option<&str> {
        self.pending_email.as_deref()
    }

    pub fn pending_challenge_id(&self) -> Option<&str> {
        self.pending_challenge_id.as_deref()
    }

    pub fn begin_send(&mut self, email: impl Into<String>) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending_email = Some(email.into());
        self.pending_challenge_id = None;
        self.generation
    }

    pub fn resolve_send(&mut self, generation: u64, challenge_id: impl Into<String>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.pending_challenge_id = Some(challenge_id.into());
        true
    }

    pub fn begin_verify(&self) -> Option<CodexAuthVerifyContext> {
        Some(CodexAuthVerifyContext {
            generation: self.generation,
            challenge_id: self.pending_challenge_id.clone()?,
            email: self.pending_email.clone()?,
        })
    }

    pub fn should_accept_response(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending_email = None;
        self.pending_challenge_id = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodexLifecycleResumeState {
    generation: u64,
    suspended_at_generation: Option<u64>,
}

impl CodexLifecycleResumeState {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn mark_background(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.suspended_at_generation = Some(self.generation);
        self.generation
    }

    pub fn begin_foreground_resume(&mut self) -> Option<u64> {
        self.suspended_at_generation?;
        self.generation = self.generation.wrapping_add(1);
        self.suspended_at_generation = None;
        Some(self.generation)
    }

    pub fn should_accept(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.suspended_at_generation = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexResumeCheckpoint {
    pub namespace: String,
    pub worker_id: String,
    pub topic_watermarks: HashMap<String, u64>,
    pub session_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CodexResumeCheckpointStore {
    checkpoints: HashMap<String, CodexResumeCheckpoint>,
}

impl CodexResumeCheckpointStore {
    pub fn checkpoints(&self) -> &HashMap<String, CodexResumeCheckpoint> {
        &self.checkpoints
    }

    pub fn watermark(&self, namespace: &str, worker_id: &str, topic: &str) -> u64 {
        self.checkpoints
            .get(&checkpoint_key(namespace, worker_id))
            .and_then(|checkpoint| checkpoint.topic_watermarks.get(topic).copied())
            .unwrap_or(0)
    }

    pub fn upsert(
        &mut self,
        namespace: impl Into<String>,
        worker_id: impl Into<String>,
        topic: impl Into<String>,
        watermark: u64,
        session_id: Option<String>,
        updated_at: impl Into<String>,
    ) {
        let namespace = namespace.into();
        let worker_id = worker_id.into();
        let topic = topic.into();
        let updated_at = updated_at.into();
        let key = checkpoint_key(&namespace, &worker_id);
        let checkpoint = self
            .checkpoints
            .entry(key)
            .or_insert_with(|| CodexResumeCheckpoint {
                namespace: namespace.clone(),
                worker_id: worker_id.clone(),
                topic_watermarks: HashMap::new(),
                session_id: None,
                updated_at: updated_at.clone(),
            });

        let existing = checkpoint
            .topic_watermarks
            .get(&topic)
            .copied()
            .unwrap_or(0);
        checkpoint
            .topic_watermarks
            .insert(topic, existing.max(watermark));

        if let Some(session_id) = session_id.and_then(normalize_string) {
            checkpoint.session_id = Some(session_id);
        }
        checkpoint.updated_at = updated_at;
    }

    pub fn reset_topic(
        &mut self,
        namespace: &str,
        worker_id: &str,
        topic: &str,
        updated_at: impl Into<String>,
    ) {
        let key = checkpoint_key(namespace, worker_id);
        let Some(checkpoint) = self.checkpoints.get_mut(&key) else {
            return;
        };

        checkpoint.topic_watermarks.remove(topic);
        checkpoint.updated_at = updated_at.into();

        if checkpoint.topic_watermarks.is_empty() {
            self.checkpoints.remove(&key);
        }
    }

    pub fn remove_namespace(&mut self, namespace: &str) {
        self.checkpoints
            .retain(|_, checkpoint| checkpoint.namespace != namespace);
    }
}

fn checkpoint_key(namespace: &str, worker_id: &str) -> String {
    format!("{namespace}|{worker_id}")
}

fn normalize_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeApiErrorCode {
    Auth,
    Forbidden,
    Conflict,
    Invalid,
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApiError {
    pub message: String,
    pub code: RuntimeApiErrorCode,
    pub status: Option<u16>,
}

impl RuntimeApiError {
    pub fn new(message: impl Into<String>, code: RuntimeApiErrorCode, status: Option<u16>) -> Self {
        Self {
            message: message.into(),
            code,
            status,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KhalaLifecycleDisconnectReason {
    StreamClosed,
    GatewayRestart,
    StaleCursor,
    Unauthorized,
    Forbidden,
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconnectClassifiableError {
    Runtime(RuntimeApiError),
    /// URL-layer network code (mirrors iOS URL error codes when available).
    NetworkCode(i32),
    Other,
}

pub const URLERR_NETWORK_CONNECTION_LOST: i32 = -1005;
pub const URLERR_TIMED_OUT: i32 = -1001;
pub const URLERR_CANNOT_CONNECT_TO_HOST: i32 = -1004;

#[derive(Debug, Clone, PartialEq)]
pub struct KhalaReconnectPolicy {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_ratio: f64,
}

impl Default for KhalaReconnectPolicy {
    fn default() -> Self {
        Self {
            base_delay_ms: 250,
            max_delay_ms: 8_000,
            jitter_ratio: 0.5,
        }
    }
}

impl KhalaReconnectPolicy {
    pub fn delay_ms(&self, attempt: u32, jitter_unit: f64) -> u64 {
        if attempt == 0 {
            return 0;
        }

        let exponent = (attempt - 1).min(10);
        let scaled = self
            .base_delay_ms
            .saturating_mul(1_u64 << exponent)
            .min(self.max_delay_ms);

        let clamped_unit = jitter_unit.clamp(0.0, 1.0);
        let jitter_max = (scaled as f64 * self.jitter_ratio.max(0.0)) as u64;
        let jitter = (jitter_max as f64 * clamped_unit) as u64;
        scaled.saturating_add(jitter)
    }
}

pub fn classify_reconnect_error(
    error: &ReconnectClassifiableError,
) -> KhalaLifecycleDisconnectReason {
    match error {
        ReconnectClassifiableError::Runtime(runtime_error) => match runtime_error.code {
            RuntimeApiErrorCode::Auth => KhalaLifecycleDisconnectReason::Unauthorized,
            RuntimeApiErrorCode::Forbidden => KhalaLifecycleDisconnectReason::Forbidden,
            RuntimeApiErrorCode::Conflict => KhalaLifecycleDisconnectReason::StaleCursor,
            RuntimeApiErrorCode::Network => {
                let message = runtime_error.message.to_ascii_lowercase();
                if message.contains("stream_closed") || message.contains("reply_cancelled") {
                    KhalaLifecycleDisconnectReason::StreamClosed
                } else {
                    KhalaLifecycleDisconnectReason::Network
                }
            }
            RuntimeApiErrorCode::Invalid | RuntimeApiErrorCode::Unknown => {
                KhalaLifecycleDisconnectReason::Unknown
            }
        },
        ReconnectClassifiableError::NetworkCode(code)
            if matches!(
                *code,
                URLERR_NETWORK_CONNECTION_LOST | URLERR_TIMED_OUT | URLERR_CANNOT_CONNECT_TO_HOST
            ) =>
        {
            KhalaLifecycleDisconnectReason::GatewayRestart
        }
        ReconnectClassifiableError::NetworkCode(_) => KhalaLifecycleDisconnectReason::Network,
        ReconnectClassifiableError::Other => KhalaLifecycleDisconnectReason::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Connecting,
    Live,
    Reconnecting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeState {
    Idle,
    Sending,
    WaitingAck { handshake_id: String },
    Success { handshake_id: String },
    TimedOut { handshake_id: String },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KhalaLifecycleSnapshot {
    pub connect_attempts: u32,
    pub reconnect_attempts: u32,
    pub successful_sessions: u32,
    pub recovered_sessions: u32,
    pub last_backoff_ms: u64,
    pub last_recovery_latency_ms: u64,
    pub last_disconnect_reason: Option<KhalaLifecycleDisconnectReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexChatRole {
    User,
    Assistant,
    Reasoning,
    Tool,
    System,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexChatMessage {
    pub id: String,
    pub role: CodexChatRole,
    pub text: String,
    pub is_streaming: bool,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub occurred_at: Option<String>,
}

impl CodexChatMessage {
    pub fn new(role: CodexChatRole, text: impl Into<String>) -> Self {
        Self {
            id: uuid_like_id(),
            role,
            text: text.into(),
            is_streaming: false,
            thread_id: None,
            turn_id: None,
            item_id: None,
            occurred_at: None,
        }
    }
}

fn uuid_like_id() -> String {
    // Deterministic-enough local ID shape without external deps.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    format!("msg-{nanos:x}")
}

pub fn should_display_system_method(method: &str) -> bool {
    method != "thread/started"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexAssistantDeltaSource {
    Modern,
    LegacyContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexAssistantDeltaDecision {
    pub selected_source: CodexAssistantDeltaSource,
    pub should_accept: bool,
    pub should_reset: bool,
}

pub fn decide_assistant_delta_source(
    current: Option<CodexAssistantDeltaSource>,
    incoming: CodexAssistantDeltaSource,
) -> CodexAssistantDeltaDecision {
    let Some(current) = current else {
        return CodexAssistantDeltaDecision {
            selected_source: incoming,
            should_accept: true,
            should_reset: false,
        };
    };

    match (current, incoming) {
        (CodexAssistantDeltaSource::Modern, CodexAssistantDeltaSource::Modern)
        | (CodexAssistantDeltaSource::LegacyContent, CodexAssistantDeltaSource::LegacyContent) => {
            CodexAssistantDeltaDecision {
                selected_source: current,
                should_accept: true,
                should_reset: false,
            }
        }
        (CodexAssistantDeltaSource::Modern, CodexAssistantDeltaSource::LegacyContent) => {
            CodexAssistantDeltaDecision {
                selected_source: CodexAssistantDeltaSource::LegacyContent,
                should_accept: true,
                should_reset: true,
            }
        }
        (CodexAssistantDeltaSource::LegacyContent, CodexAssistantDeltaSource::Modern) => {
            CodexAssistantDeltaDecision {
                selected_source: CodexAssistantDeltaSource::LegacyContent,
                should_accept: false,
                should_reset: false,
            }
        }
    }
}

pub fn append_streaming_text(existing: &str, delta: &str) -> String {
    if delta.is_empty() {
        return existing.to_string();
    }
    if existing.is_empty() {
        return delta.to_string();
    }
    if existing.ends_with(delta) {
        return existing.to_string();
    }

    let overlap = overlap_length(existing, delta);
    if overlap == 0 {
        let mut merged = existing.to_string();
        merged.push_str(delta);
        return merged;
    }

    let mut merged = existing.to_string();
    merged.push_str(&delta[overlap..]);
    merged
}

fn overlap_length(existing: &str, delta: &str) -> usize {
    if existing.is_empty() || delta.is_empty() {
        return 0;
    }

    let mut lengths = delta
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    lengths.push(delta.len());
    lengths.sort_unstable();
    lengths.dedup();

    for length in lengths.into_iter().rev() {
        if length == 0 || length > existing.len() {
            continue;
        }
        if existing.ends_with(&delta[..length]) {
            return length;
        }
    }

    0
}

#[derive(Debug, Default)]
pub struct CodexEventSeqDedupe {
    cache_limit: usize,
    seen: HashSet<u64>,
    order: VecDeque<u64>,
}

impl CodexEventSeqDedupe {
    pub fn new(cache_limit: usize) -> Self {
        Self {
            cache_limit: cache_limit.max(1),
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    pub fn should_process(&mut self, seq: Option<u64>) -> bool {
        let Some(seq) = seq else {
            return true;
        };

        if self.seen.contains(&seq) {
            return false;
        }

        self.seen.insert(seq);
        self.order.push_back(seq);

        while self.order.len() > self.cache_limit {
            if let Some(dropped) = self.order.pop_front() {
                self.seen.remove(&dropped);
            }
        }

        true
    }
}

#[derive(Debug, Default)]
pub struct CodexPerItemMessageAssembler {
    assistant_text: HashMap<String, String>,
    assistant_source: HashMap<String, CodexAssistantDeltaSource>,
    reasoning_text: HashMap<String, String>,
    tool_text: HashMap<String, String>,
}

impl CodexPerItemMessageAssembler {
    pub fn append_assistant_delta(
        &mut self,
        item_key: impl Into<String>,
        delta: &str,
        incoming_source: CodexAssistantDeltaSource,
    ) -> Option<&str> {
        if delta.is_empty() {
            return None;
        }

        let item_key = item_key.into();
        let current_source = self.assistant_source.get(&item_key).copied();
        let decision = decide_assistant_delta_source(current_source, incoming_source);
        self.assistant_source
            .insert(item_key.clone(), decision.selected_source);

        if !decision.should_accept {
            return self.assistant_text.get(&item_key).map(String::as_str);
        }

        let current = if decision.should_reset {
            String::new()
        } else {
            self.assistant_text
                .get(&item_key)
                .cloned()
                .unwrap_or_default()
        };

        let merged = append_streaming_text(&current, delta);
        self.assistant_text.insert(item_key.clone(), merged);
        self.assistant_text.get(&item_key).map(String::as_str)
    }

    pub fn finish_assistant_message(
        &mut self,
        item_key: impl Into<String>,
        text_override: Option<&str>,
    ) -> Option<&str> {
        let item_key = item_key.into();
        if let Some(text) = text_override.and_then(trim_non_empty) {
            self.assistant_text
                .insert(item_key.clone(), text.to_string());
        }
        self.assistant_text.get(&item_key).map(String::as_str)
    }

    pub fn assistant_text(&self, item_key: &str) -> Option<&str> {
        self.assistant_text.get(item_key).map(String::as_str)
    }

    pub fn append_reasoning_delta(
        &mut self,
        item_key: impl Into<String>,
        delta: &str,
    ) -> Option<&str> {
        if delta.is_empty() {
            return None;
        }
        let item_key = item_key.into();
        let current = self
            .reasoning_text
            .get(&item_key)
            .cloned()
            .unwrap_or_default();
        let merged = append_streaming_text(&current, delta);
        self.reasoning_text.insert(item_key.clone(), merged);
        self.reasoning_text.get(&item_key).map(String::as_str)
    }

    pub fn finish_reasoning_message(
        &mut self,
        item_key: impl Into<String>,
        text_override: Option<&str>,
    ) -> Option<&str> {
        let item_key = item_key.into();
        if let Some(text) = text_override.and_then(trim_non_empty) {
            self.reasoning_text
                .insert(item_key.clone(), text.to_string());
        }

        let text = self.reasoning_text.get(&item_key).cloned();
        if let Some(text) = text {
            let normalized = text.trim();
            if normalized.is_empty() || normalized == "..." || normalized == "…" {
                self.reasoning_text.remove(&item_key);
                return None;
            }
        }

        self.reasoning_text.get(&item_key).map(String::as_str)
    }

    pub fn append_tool_delta(&mut self, item_key: impl Into<String>, delta: &str) -> Option<&str> {
        if delta.is_empty() {
            return None;
        }
        let item_key = item_key.into();
        let entry = self.tool_text.entry(item_key.clone()).or_default();
        entry.push_str(delta);
        self.tool_text.get(&item_key).map(String::as_str)
    }

    pub fn finish_tool_message(&mut self, item_key: impl Into<String>, text: &str) -> Option<&str> {
        let item_key = item_key.into();
        if let Some(text) = trim_non_empty(text) {
            self.tool_text.insert(item_key.clone(), text.to_string());
        }
        self.tool_text.get(&item_key).map(String::as_str)
    }

    pub fn reasoning_text(&self, item_key: &str) -> Option<&str> {
        self.reasoning_text.get(item_key).map(String::as_str)
    }

    pub fn tool_text(&self, item_key: &str) -> Option<&str> {
        self.tool_text.get(item_key).map(String::as_str)
    }
}

fn trim_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexWorkerActionRequest {
    pub request_id: String,
    pub method: ControlMethod,
    pub params: Value,
    pub request_version: Option<String>,
    pub sent_at: Option<String>,
    pub source: Option<String>,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
}

impl RuntimeCodexWorkerActionRequest {
    pub fn new(request_id: impl Into<String>, method: ControlMethod, params: Value) -> Self {
        Self {
            request_id: request_id.into(),
            method,
            params,
            request_version: Some("v1".to_string()),
            sent_at: None,
            source: Some("autopilot-ios".to_string()),
            session_id: None,
            thread_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeCodexControlReceiptOutcome {
    Success {
        response: Option<Value>,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
        details: Option<Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexControlReceipt {
    pub request_id: String,
    pub method: String,
    pub occurred_at: Option<String>,
    pub outcome: RuntimeCodexControlReceiptOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCodexControlRequestState {
    Queued,
    Running,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCodexControlRequestTracker {
    pub worker_id: String,
    pub request: RuntimeCodexWorkerActionRequest,
    pub created_at: String,
    pub last_updated_at: String,
    pub state: RuntimeCodexControlRequestState,
    pub sent_at: Option<String>,
    pub receipt_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
    pub response: Option<Value>,
}

impl RuntimeCodexControlRequestTracker {
    pub fn request_id(&self) -> &str {
        &self.request.request_id
    }
}

#[derive(Debug, Default)]
pub struct RuntimeCodexControlCoordinator {
    request_order: Vec<String>,
    tracked_requests_by_id: HashMap<String, RuntimeCodexControlRequestTracker>,
    queued_request_ids: Vec<String>,
    terminal_receipt_dedupe: HashSet<String>,
}

impl RuntimeCodexControlCoordinator {
    pub fn snapshots(&self) -> Vec<RuntimeCodexControlRequestTracker> {
        self.request_order
            .iter()
            .filter_map(|request_id| self.tracked_requests_by_id.get(request_id).cloned())
            .collect()
    }

    pub fn queued_requests(&self) -> Vec<RuntimeCodexControlRequestTracker> {
        self.queued_request_ids
            .iter()
            .filter_map(|request_id| self.tracked_requests_by_id.get(request_id).cloned())
            .filter(|tracker| tracker.state == RuntimeCodexControlRequestState::Queued)
            .collect()
    }

    pub fn enqueue(
        &mut self,
        worker_id: impl Into<String>,
        request: RuntimeCodexWorkerActionRequest,
        occurred_at: impl Into<String>,
    ) -> RuntimeCodexControlRequestTracker {
        let worker_id = worker_id.into();
        let occurred_at = occurred_at.into();

        if self
            .tracked_requests_by_id
            .contains_key(&request.request_id)
        {
            let mut should_queue = false;
            let existing = {
                let tracker = self
                    .tracked_requests_by_id
                    .get_mut(&request.request_id)
                    .expect("checked contains_key above");
                if tracker.state == RuntimeCodexControlRequestState::Error && tracker.retryable {
                    tracker.state = RuntimeCodexControlRequestState::Queued;
                    tracker.error_code = None;
                    tracker.error_message = None;
                    tracker.last_updated_at = occurred_at.clone();
                    should_queue = true;
                } else if tracker.state == RuntimeCodexControlRequestState::Queued {
                    should_queue = true;
                }
                tracker.clone()
            };
            if should_queue {
                self.ensure_queued(request.request_id.as_str());
            }
            return existing;
        }

        let tracker = RuntimeCodexControlRequestTracker {
            worker_id,
            request: request.clone(),
            created_at: occurred_at.clone(),
            last_updated_at: occurred_at,
            state: RuntimeCodexControlRequestState::Queued,
            sent_at: None,
            receipt_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
            response: None,
        };

        self.tracked_requests_by_id
            .insert(request.request_id.clone(), tracker.clone());
        self.request_order.push(request.request_id.clone());
        self.ensure_queued(request.request_id.as_str());

        tracker
    }

    pub fn mark_running(
        &mut self,
        request_id: &str,
        occurred_at: impl Into<String>,
    ) -> Option<RuntimeCodexControlRequestTracker> {
        let occurred_at = occurred_at.into();
        let snapshot = {
            let tracker = self.tracked_requests_by_id.get_mut(request_id)?;
            tracker.state = RuntimeCodexControlRequestState::Running;
            if tracker.sent_at.is_none() {
                tracker.sent_at = Some(occurred_at.clone());
            }
            tracker.last_updated_at = occurred_at;
            tracker.error_code = None;
            tracker.error_message = None;
            tracker.retryable = false;
            tracker.clone()
        };
        self.remove_queued(request_id);
        Some(snapshot)
    }

    pub fn requeue(
        &mut self,
        request_id: &str,
        message: Option<String>,
        occurred_at: impl Into<String>,
    ) -> Option<RuntimeCodexControlRequestTracker> {
        let occurred_at = occurred_at.into();
        let snapshot = {
            let tracker = self.tracked_requests_by_id.get_mut(request_id)?;
            tracker.state = RuntimeCodexControlRequestState::Queued;
            tracker.last_updated_at = occurred_at;
            tracker.error_message = message;
            tracker.retryable = true;
            tracker.clone()
        };
        self.ensure_queued(request_id);
        Some(snapshot)
    }

    pub fn mark_dispatch_error(
        &mut self,
        request_id: &str,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
        occurred_at: impl Into<String>,
    ) -> Option<RuntimeCodexControlRequestTracker> {
        let occurred_at = occurred_at.into();
        let snapshot = {
            let tracker = self.tracked_requests_by_id.get_mut(request_id)?;
            tracker.last_updated_at = occurred_at;
            tracker.error_code = Some(code.into());
            tracker.error_message = Some(message.into());
            tracker.retryable = retryable;

            if retryable {
                tracker.state = RuntimeCodexControlRequestState::Queued;
            } else {
                tracker.state = RuntimeCodexControlRequestState::Error;
            }
            tracker.clone()
        };

        if retryable {
            self.ensure_queued(request_id);
        } else {
            self.remove_queued(request_id);
        }

        Some(snapshot)
    }

    pub fn mark_timeout(
        &mut self,
        request_id: &str,
        occurred_at: impl Into<String>,
    ) -> Option<RuntimeCodexControlRequestTracker> {
        self.mark_dispatch_error(
            request_id,
            "timeout",
            "Timed out waiting for worker receipt.",
            false,
            occurred_at,
        )
    }

    pub fn reconcile(
        &mut self,
        worker_id: &str,
        receipt: RuntimeCodexControlReceipt,
    ) -> Option<RuntimeCodexControlRequestTracker> {
        let dedupe_key = format!("{worker_id}::terminal::{}", receipt.request_id);
        if !self.terminal_receipt_dedupe.insert(dedupe_key) {
            return None;
        }

        let snapshot = {
            let tracker = self.tracked_requests_by_id.get_mut(&receipt.request_id)?;
            if tracker.worker_id != worker_id {
                return None;
            }
            if let Some(occurred_at) = receipt.occurred_at.clone() {
                tracker.last_updated_at = occurred_at.clone();
                tracker.receipt_at = Some(occurred_at);
            }

            match receipt.outcome {
                RuntimeCodexControlReceiptOutcome::Success { response } => {
                    tracker.state = RuntimeCodexControlRequestState::Success;
                    tracker.retryable = false;
                    tracker.error_code = None;
                    tracker.error_message = None;
                    tracker.response = response;
                }
                RuntimeCodexControlReceiptOutcome::Error {
                    code,
                    message,
                    retryable,
                    details: _,
                } => {
                    tracker.state = RuntimeCodexControlRequestState::Error;
                    tracker.error_code = Some(code);
                    tracker.error_message = Some(message);
                    tracker.retryable = retryable;
                }
            }
            tracker.clone()
        };

        self.remove_queued(&receipt.request_id);
        Some(snapshot)
    }

    fn ensure_queued(&mut self, request_id: &str) {
        if !self
            .queued_request_ids
            .iter()
            .any(|queued| queued == request_id)
        {
            self.queued_request_ids.push(request_id.to_string());
        }
    }

    fn remove_queued(&mut self, request_id: &str) {
        self.queued_request_ids
            .retain(|queued| queued.as_str() != request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_event_display_policy_suppresses_thread_started() {
        assert!(!should_display_system_method("thread/started"));
        assert!(should_display_system_method("turn/started"));
        assert!(should_display_system_method("turn/completed"));
    }

    #[test]
    fn streaming_text_assembler_preserves_order_and_overlap() {
        assert_eq!(append_streaming_text("I'm Cod", "ex,"), "I'm Codex,");
        assert_eq!(append_streaming_text("Hello wor", "world"), "Hello world");
        assert_eq!(append_streaming_text("Hello", "Hello"), "Hello");
    }

    #[test]
    fn assistant_delta_policy_prefers_legacy() {
        let first_legacy =
            decide_assistant_delta_source(None, CodexAssistantDeltaSource::LegacyContent);
        assert_eq!(
            first_legacy.selected_source,
            CodexAssistantDeltaSource::LegacyContent
        );
        assert!(first_legacy.should_accept);
        assert!(!first_legacy.should_reset);

        let switch_from_modern = decide_assistant_delta_source(
            Some(CodexAssistantDeltaSource::Modern),
            CodexAssistantDeltaSource::LegacyContent,
        );
        assert_eq!(
            switch_from_modern.selected_source,
            CodexAssistantDeltaSource::LegacyContent
        );
        assert!(switch_from_modern.should_accept);
        assert!(switch_from_modern.should_reset);

        let ignore_modern_after_legacy = decide_assistant_delta_source(
            Some(CodexAssistantDeltaSource::LegacyContent),
            CodexAssistantDeltaSource::Modern,
        );
        assert_eq!(
            ignore_modern_after_legacy.selected_source,
            CodexAssistantDeltaSource::LegacyContent
        );
        assert!(!ignore_modern_after_legacy.should_accept);
        assert!(!ignore_modern_after_legacy.should_reset);
    }

    #[test]
    fn khala_reconnect_policy_uses_bounded_backoff() {
        let policy = KhalaReconnectPolicy::default();

        assert_eq!(policy.delay_ms(1, 0.0), 250);
        assert_eq!(policy.delay_ms(2, 0.0), 500);
        assert_eq!(policy.delay_ms(3, 0.0), 1_000);
        assert_eq!(policy.delay_ms(32, 0.0), 8_000);
        assert_eq!(policy.delay_ms(4, 1.0), 3_000);
    }

    #[test]
    fn khala_reconnect_classifier_maps_failure_classes() {
        let unauthorized = ReconnectClassifiableError::Runtime(RuntimeApiError::new(
            "unauthorized",
            RuntimeApiErrorCode::Auth,
            Some(401),
        ));
        let stale_cursor = ReconnectClassifiableError::Runtime(RuntimeApiError::new(
            "stale_cursor",
            RuntimeApiErrorCode::Conflict,
            Some(409),
        ));
        let stream_closed = ReconnectClassifiableError::Runtime(RuntimeApiError::new(
            "khala_stream_closed",
            RuntimeApiErrorCode::Network,
            None,
        ));
        let gateway_restart =
            ReconnectClassifiableError::NetworkCode(URLERR_NETWORK_CONNECTION_LOST);

        assert_eq!(
            classify_reconnect_error(&unauthorized),
            KhalaLifecycleDisconnectReason::Unauthorized
        );
        assert_eq!(
            classify_reconnect_error(&stale_cursor),
            KhalaLifecycleDisconnectReason::StaleCursor
        );
        assert_eq!(
            classify_reconnect_error(&stream_closed),
            KhalaLifecycleDisconnectReason::StreamClosed
        );
        assert_eq!(
            classify_reconnect_error(&gateway_restart),
            KhalaLifecycleDisconnectReason::GatewayRestart
        );
    }

    #[test]
    fn auth_flow_state_drops_stale_send_completion() {
        let mut flow = CodexAuthFlowState::default();
        let first = flow.begin_send("one@openagents.com");
        let second = flow.begin_send("two@openagents.com");

        let stale_resolved = flow.resolve_send(first, "challenge-1");
        let latest_resolved = flow.resolve_send(second, "challenge-2");
        assert!(!stale_resolved);
        assert!(latest_resolved);

        let verify = flow.begin_verify().expect("verify context should exist");
        assert_eq!(verify.generation, second);
        assert_eq!(verify.challenge_id, "challenge-2");
        assert_eq!(verify.email, "two@openagents.com");
    }

    #[test]
    fn auth_flow_state_rejects_stale_verify_responses() {
        let mut flow = CodexAuthFlowState::default();
        let generation = flow.begin_send("race@openagents.com");
        assert!(flow.resolve_send(generation, "challenge-race"));

        let verify = flow.begin_verify().expect("verify context should exist");
        assert_eq!(verify.generation, generation);

        flow.invalidate();
        assert!(!flow.should_accept_response(generation));
    }

    #[test]
    fn resume_checkpoint_store_namespaces_watermarks() {
        let mut store = CodexResumeCheckpointStore::default();
        let topic = "runtime.codex_worker_events";

        store.upsert(
            "device:ios-1|user:user-1",
            "desktopw:shared",
            topic,
            12,
            Some("session-1".to_string()),
            "2026-02-21T10:00:00Z",
        );
        store.upsert(
            "device:ios-1|user:user-1",
            "desktopw:shared",
            topic,
            8,
            Some("session-1".to_string()),
            "2026-02-21T10:00:01Z",
        );
        store.upsert(
            "device:ios-1|user:user-2",
            "desktopw:shared",
            topic,
            4,
            Some("session-2".to_string()),
            "2026-02-21T10:00:02Z",
        );

        assert_eq!(
            store.watermark("device:ios-1|user:user-1", "desktopw:shared", topic),
            12
        );
        assert_eq!(
            store.watermark("device:ios-1|user:user-2", "desktopw:shared", topic),
            4
        );
        assert_eq!(
            store.watermark("device:ios-1|user:user-1", "desktopw:other", topic),
            0
        );
    }

    #[test]
    fn resume_checkpoint_store_topic_reset_is_scoped() {
        let mut store = CodexResumeCheckpointStore::default();
        let namespace = "device:ios-2|user:user-10";
        let worker_id = "desktopw:shared";

        store.upsert(
            namespace,
            worker_id,
            "runtime.codex_worker_events",
            40,
            Some("session-a".to_string()),
            "2026-02-21T10:10:00Z",
        );
        store.upsert(
            namespace,
            worker_id,
            "runtime.other_topic",
            9,
            Some("session-a".to_string()),
            "2026-02-21T10:10:01Z",
        );

        store.reset_topic(
            namespace,
            worker_id,
            "runtime.codex_worker_events",
            "2026-02-21T10:10:02Z",
        );

        assert_eq!(
            store.watermark(namespace, worker_id, "runtime.codex_worker_events"),
            0
        );
        assert_eq!(
            store.watermark(namespace, worker_id, "runtime.other_topic"),
            9
        );
    }

    #[test]
    fn lifecycle_resume_state_rejects_stale_generations() {
        let mut state = CodexLifecycleResumeState::default();

        let first_background = state.mark_background();
        assert!(state.should_accept(first_background));

        let first_resume = state
            .begin_foreground_resume()
            .expect("first resume should exist");
        assert!(!state.should_accept(first_background));

        let second_background = state.mark_background();
        assert_ne!(second_background, first_background);

        let second_resume = state
            .begin_foreground_resume()
            .expect("second resume should exist");
        assert_ne!(Some(second_resume), Some(first_resume));

        state.invalidate();
        assert!(!state.should_accept(second_resume));
    }

    #[test]
    fn control_coordinator_transitions_and_dedupes_receipts() {
        let mut coordinator = RuntimeCodexControlCoordinator::default();

        let request = RuntimeCodexWorkerActionRequest {
            request_id: "iosreq-control-1".to_string(),
            method: ControlMethod::TurnStart,
            params: serde_json::json!({"thread_id":"thread-123","text":"continue"}),
            request_version: Some("v1".to_string()),
            sent_at: Some("2026-02-22T01:01:00Z".to_string()),
            source: Some("autopilot-ios".to_string()),
            session_id: None,
            thread_id: None,
        };

        let queued =
            coordinator.enqueue("desktopw:shared", request.clone(), "2026-02-22T01:01:00Z");
        assert_eq!(queued.state, RuntimeCodexControlRequestState::Queued);
        assert_eq!(coordinator.queued_requests().len(), 1);

        let running = coordinator
            .mark_running(&request.request_id, "2026-02-22T01:01:01Z")
            .expect("running tracker should exist");
        assert_eq!(running.state, RuntimeCodexControlRequestState::Running);
        assert!(coordinator.queued_requests().is_empty());

        let success_receipt = RuntimeCodexControlReceipt {
            request_id: request.request_id.clone(),
            method: "turn/start".to_string(),
            occurred_at: Some("2026-02-22T01:01:02Z".to_string()),
            outcome: RuntimeCodexControlReceiptOutcome::Success {
                response: Some(serde_json::json!({"ok": true})),
            },
        };

        let reconciled = coordinator
            .reconcile("desktopw:shared", success_receipt.clone())
            .expect("receipt should reconcile");
        assert_eq!(reconciled.state, RuntimeCodexControlRequestState::Success);
        assert_eq!(reconciled.response, Some(serde_json::json!({"ok": true})));

        let duplicate = coordinator.reconcile("desktopw:shared", success_receipt);
        assert!(duplicate.is_none());
    }

    #[test]
    fn control_coordinator_reconciles_replay_after_disconnect() {
        let mut coordinator = RuntimeCodexControlCoordinator::default();
        let worker_id = "desktopw:shared";

        let request = RuntimeCodexWorkerActionRequest {
            request_id: "iosreq-replay-1".to_string(),
            method: ControlMethod::TurnInterrupt,
            params: serde_json::json!({"thread_id":"thread-123","turn_id":"turn-123"}),
            request_version: Some("v1".to_string()),
            sent_at: Some("2026-02-22T01:02:00Z".to_string()),
            source: Some("autopilot-ios".to_string()),
            session_id: None,
            thread_id: None,
        };

        coordinator.enqueue(worker_id, request.clone(), "2026-02-22T01:02:00Z");
        coordinator.mark_running(&request.request_id, "2026-02-22T01:02:01Z");

        let replay_receipt = RuntimeCodexControlReceipt {
            request_id: request.request_id.clone(),
            method: "turn/interrupt".to_string(),
            occurred_at: Some("2026-02-22T01:02:05Z".to_string()),
            outcome: RuntimeCodexControlReceiptOutcome::Error {
                code: "conflict".to_string(),
                message: "turn already completed".to_string(),
                retryable: false,
                details: None,
            },
        };

        let reconciled = coordinator
            .reconcile(worker_id, replay_receipt)
            .expect("replay receipt should reconcile");
        assert_eq!(reconciled.state, RuntimeCodexControlRequestState::Error);
        assert_eq!(reconciled.error_code.as_deref(), Some("conflict"));
        assert_eq!(
            reconciled.error_message.as_deref(),
            Some("turn already completed")
        );
    }

    #[test]
    fn control_coordinator_turn_start_then_interrupt_scenario() {
        let mut coordinator = RuntimeCodexControlCoordinator::default();
        let worker_id = "desktopw:shared";

        let start_request = RuntimeCodexWorkerActionRequest {
            request_id: "iosreq-turn-start".to_string(),
            method: ControlMethod::TurnStart,
            params: serde_json::json!({
                "thread_id":"thread-123",
                "text":"continue",
                "model":"gpt-5-codex",
                "effort":"medium"
            }),
            request_version: Some("v1".to_string()),
            sent_at: Some("2026-02-22T02:00:00Z".to_string()),
            source: Some("autopilot-ios".to_string()),
            session_id: None,
            thread_id: None,
        };

        coordinator.enqueue(worker_id, start_request.clone(), "2026-02-22T02:00:00Z");
        coordinator.mark_running(&start_request.request_id, "2026-02-22T02:00:01Z");

        let start_receipt = RuntimeCodexControlReceipt {
            request_id: start_request.request_id.clone(),
            method: "turn/start".to_string(),
            occurred_at: Some("2026-02-22T02:00:02Z".to_string()),
            outcome: RuntimeCodexControlReceiptOutcome::Success {
                response: Some(serde_json::json!({
                    "thread_id":"thread-123",
                    "turn": {"id":"turn-abc"}
                })),
            },
        };
        let start_reconciled = coordinator
            .reconcile(worker_id, start_receipt)
            .expect("start receipt should reconcile");
        assert_eq!(
            start_reconciled.state,
            RuntimeCodexControlRequestState::Success
        );

        let interrupt_request = RuntimeCodexWorkerActionRequest {
            request_id: "iosreq-turn-interrupt".to_string(),
            method: ControlMethod::TurnInterrupt,
            params: serde_json::json!({"thread_id":"thread-123","turn_id":"turn-abc"}),
            request_version: Some("v1".to_string()),
            sent_at: Some("2026-02-22T02:00:03Z".to_string()),
            source: Some("autopilot-ios".to_string()),
            session_id: None,
            thread_id: None,
        };

        coordinator.enqueue(worker_id, interrupt_request.clone(), "2026-02-22T02:00:03Z");
        coordinator.mark_running(&interrupt_request.request_id, "2026-02-22T02:00:04Z");

        let interrupt_receipt = RuntimeCodexControlReceipt {
            request_id: interrupt_request.request_id.clone(),
            method: "turn/interrupt".to_string(),
            occurred_at: Some("2026-02-22T02:00:05Z".to_string()),
            outcome: RuntimeCodexControlReceiptOutcome::Success {
                response: Some(serde_json::json!({"status":"interrupted","turn_id":"turn-abc"})),
            },
        };

        let interrupt_reconciled = coordinator
            .reconcile(worker_id, interrupt_receipt)
            .expect("interrupt receipt should reconcile");
        assert_eq!(
            interrupt_reconciled.state,
            RuntimeCodexControlRequestState::Success
        );
        assert_eq!(
            interrupt_reconciled
                .response
                .and_then(|value| value.get("status").cloned()),
            Some(Value::String("interrupted".to_string()))
        );
    }

    #[test]
    fn seq_dedupe_cache_is_bounded_and_reaccepts_evicted_entries() {
        let mut dedupe = CodexEventSeqDedupe::new(2);

        assert!(dedupe.should_process(Some(1)));
        assert!(dedupe.should_process(Some(2)));
        assert!(!dedupe.should_process(Some(1)));

        assert!(dedupe.should_process(Some(3)));
        assert!(dedupe.should_process(Some(1)));
    }

    #[test]
    fn per_item_assembler_applies_legacy_source_switch_and_overlap() {
        let mut assembler = CodexPerItemMessageAssembler::default();

        let initial = assembler
            .append_assistant_delta("item-1", "Hello wor", CodexAssistantDeltaSource::Modern)
            .expect("assistant text should exist");
        assert_eq!(initial, "Hello wor");

        let merged = assembler
            .append_assistant_delta("item-1", "world", CodexAssistantDeltaSource::Modern)
            .expect("assistant text should exist");
        assert_eq!(merged, "Hello world");

        let switched = assembler
            .append_assistant_delta(
                "item-1",
                "Legacy says hi",
                CodexAssistantDeltaSource::LegacyContent,
            )
            .expect("assistant text should exist");
        assert_eq!(switched, "Legacy says hi");

        let ignored = assembler
            .append_assistant_delta("item-1", " modern", CodexAssistantDeltaSource::Modern)
            .expect("assistant text should exist");
        assert_eq!(ignored, "Legacy says hi");
    }

    #[test]
    fn per_item_assembler_reasoning_and_tool_paths_are_deterministic() {
        let mut assembler = CodexPerItemMessageAssembler::default();

        let reasoning = assembler
            .append_reasoning_delta("reasoning-1", "Thinking")
            .expect("reasoning text should exist");
        assert_eq!(reasoning, "Thinking");

        let reasoning = assembler
            .append_reasoning_delta("reasoning-1", "ing")
            .expect("reasoning text should exist");
        assert_eq!(reasoning, "Thinking");

        let tool = assembler
            .append_tool_delta("tool-1", "$ ls")
            .expect("tool text should exist");
        assert_eq!(tool, "$ ls");

        let tool = assembler
            .append_tool_delta("tool-1", "\nREADME.md")
            .expect("tool text should exist");
        assert_eq!(tool, "$ ls\nREADME.md");
    }
}
