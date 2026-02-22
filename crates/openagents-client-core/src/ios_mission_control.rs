use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_EVENT_RING_LIMIT: usize = 1_024;
const DEFAULT_TIMELINE_LIMIT: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub struct WorkerThreadKey {
    pub worker_id: String,
    pub thread_id: String,
}

impl WorkerThreadKey {
    pub fn new(worker_id: impl Into<String>, thread_id: impl Into<String>) -> Option<Self> {
        let worker_id = normalized_owned(Some(worker_id.into()))?;
        let thread_id = normalized_owned(Some(thread_id.into()))?;
        Some(Self {
            worker_id,
            thread_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionWorkerState {
    pub worker_id: String,
    pub status: String,
    pub heartbeat_state: Option<String>,
    pub latest_seq: Option<u64>,
    pub lag_events: Option<u64>,
    pub reconnect_state: Option<String>,
    pub last_event_at: Option<String>,
    pub running_turns: u64,
    pub queued_requests: u64,
    pub failed_requests: u64,
}

impl MissionWorkerState {
    fn new(worker_id: impl Into<String>) -> Self {
        Self {
            worker_id: worker_id.into(),
            status: "unknown".to_string(),
            heartbeat_state: None,
            latest_seq: None,
            lag_events: None,
            reconnect_state: None,
            last_event_at: None,
            running_turns: 0,
            queued_requests: 0,
            failed_requests: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionThreadState {
    pub worker_id: String,
    pub thread_id: String,
    pub active_turn_id: Option<String>,
    pub last_summary: String,
    pub last_event_at: Option<String>,
    pub freshness_seq: Option<u64>,
    pub unread_count: u64,
    pub muted: bool,
}

impl MissionThreadState {
    fn new(key: &WorkerThreadKey) -> Self {
        Self {
            worker_id: key.worker_id.clone(),
            thread_id: key.thread_id.clone(),
            active_turn_id: None,
            last_summary: "".to_string(),
            last_event_at: None,
            freshness_seq: None,
            unread_count: 0,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionTimelineItem {
    pub id: String,
    pub role: String,
    pub text: String,
    pub is_streaming: bool,
    pub worker_id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionEventSeverity {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEventRecord {
    pub id: u64,
    pub topic: String,
    pub seq: Option<u64>,
    pub worker_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub request_id: Option<String>,
    pub event_type: Option<String>,
    pub method: Option<String>,
    pub summary: String,
    pub severity: MissionEventSeverity,
    pub occurred_at: Option<String>,
    pub payload: Value,
    pub resync_marker: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRequestState {
    pub request_id: String,
    pub worker_id: String,
    pub thread_id: Option<String>,
    pub method: String,
    pub state: String,
    pub occurred_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
    pub response: Option<Value>,
}

impl MissionRequestState {
    fn from_envelope(
        worker_id: String,
        request_id: String,
        method: String,
        state: String,
        thread_id: Option<String>,
        occurred_at: Option<String>,
    ) -> Self {
        Self {
            request_id,
            worker_id,
            thread_id,
            method,
            state,
            occurred_at,
            error_code: None,
            error_message: None,
            retryable: false,
            response: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionThreadTimeline {
    pub worker_id: String,
    pub thread_id: String,
    pub entries: Vec<MissionTimelineItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MissionControlProjection {
    pub workers: Vec<MissionWorkerState>,
    pub threads: Vec<MissionThreadState>,
    pub timelines: Vec<MissionThreadTimeline>,
    pub events: Vec<MissionEventRecord>,
    pub requests: Vec<MissionRequestState>,
    pub active_worker_id: Option<String>,
    pub active_thread_id: Option<String>,
    pub active_turn_id: Option<String>,
    pub compatibility_chat_messages: Vec<MissionTimelineItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MissionControlCommand {
    IngestStreamEvent {
        topic: String,
        #[serde(default)]
        seq: Option<u64>,
        #[serde(default)]
        worker_id: Option<String>,
        payload: Value,
    },
    IngestWorkerSummary {
        worker_id: String,
        status: String,
        #[serde(default)]
        heartbeat_state: Option<String>,
        #[serde(default)]
        latest_seq: Option<u64>,
        #[serde(default)]
        lag_events: Option<u64>,
        #[serde(default)]
        reconnect_state: Option<String>,
        #[serde(default)]
        occurred_at: Option<String>,
    },
    UpsertRequest {
        request: MissionRequestState,
    },
    SetActiveLane {
        #[serde(default)]
        worker_id: Option<String>,
        #[serde(default)]
        thread_id: Option<String>,
    },
    SetLaneMuted {
        worker_id: String,
        thread_id: String,
        muted: bool,
    },
    MarkResynced {
        topic: String,
        from_seq: u64,
        to_seq: u64,
        #[serde(default)]
        worker_id: Option<String>,
    },
    Configure {
        #[serde(default)]
        max_events: Option<usize>,
        #[serde(default)]
        max_timeline_entries: Option<usize>,
    },
    Reset,
    Projection,
}

#[derive(Debug, Default)]
pub struct IosMissionControlStore {
    worker_store: HashMap<String, MissionWorkerState>,
    thread_store: HashMap<WorkerThreadKey, MissionThreadState>,
    timeline_store: HashMap<WorkerThreadKey, VecDeque<MissionTimelineItem>>,
    event_store: VecDeque<MissionEventRecord>,
    event_dedupe: HashSet<String>,
    request_store: HashMap<String, MissionRequestState>,
    active_lane: Option<WorkerThreadKey>,
    next_event_id: u64,
    max_events: usize,
    max_timeline_entries: usize,
}

impl IosMissionControlStore {
    pub fn new() -> Self {
        Self {
            worker_store: HashMap::new(),
            thread_store: HashMap::new(),
            timeline_store: HashMap::new(),
            event_store: VecDeque::new(),
            event_dedupe: HashSet::new(),
            request_store: HashMap::new(),
            active_lane: None,
            next_event_id: 1,
            max_events: DEFAULT_EVENT_RING_LIMIT,
            max_timeline_entries: DEFAULT_TIMELINE_LIMIT,
        }
    }

    pub fn apply_command_json(&mut self, command_json: &str) -> Option<String> {
        let command = serde_json::from_str::<MissionControlCommand>(command_json).ok()?;
        self.apply_command(command);
        serde_json::to_string(&self.projection()).ok()
    }

    pub fn apply_command(&mut self, command: MissionControlCommand) {
        match command {
            MissionControlCommand::IngestStreamEvent {
                topic,
                seq,
                worker_id,
                payload,
            } => {
                self.ingest_stream_event(topic, seq, worker_id, payload);
            }
            MissionControlCommand::IngestWorkerSummary {
                worker_id,
                status,
                heartbeat_state,
                latest_seq,
                lag_events,
                reconnect_state,
                occurred_at,
            } => {
                self.ingest_worker_summary(
                    worker_id,
                    status,
                    heartbeat_state,
                    latest_seq,
                    lag_events,
                    reconnect_state,
                    occurred_at,
                );
            }
            MissionControlCommand::UpsertRequest { request } => {
                self.upsert_request(request);
            }
            MissionControlCommand::SetActiveLane {
                worker_id,
                thread_id,
            } => {
                self.set_active_lane(worker_id, thread_id);
            }
            MissionControlCommand::SetLaneMuted {
                worker_id,
                thread_id,
                muted,
            } => {
                self.set_lane_muted(worker_id, thread_id, muted);
            }
            MissionControlCommand::MarkResynced {
                topic,
                from_seq,
                to_seq,
                worker_id,
            } => {
                self.mark_resynced(topic, from_seq, to_seq, worker_id);
            }
            MissionControlCommand::Configure {
                max_events,
                max_timeline_entries,
            } => {
                if let Some(limit) = max_events {
                    self.max_events = limit.max(1);
                    self.enforce_event_bound();
                }
                if let Some(limit) = max_timeline_entries {
                    self.max_timeline_entries = limit.max(1);
                    self.enforce_timeline_bound();
                }
            }
            MissionControlCommand::Reset => {
                *self = Self::new();
            }
            MissionControlCommand::Projection => {}
        }
    }

    pub fn projection(&self) -> MissionControlProjection {
        let mut workers = self.worker_store.values().cloned().collect::<Vec<_>>();
        workers.sort_by(|lhs, rhs| lhs.worker_id.cmp(&rhs.worker_id));

        let mut threads = self.thread_store.values().cloned().collect::<Vec<_>>();
        threads.sort_by(|lhs, rhs| {
            lhs.worker_id
                .cmp(&rhs.worker_id)
                .then(lhs.thread_id.cmp(&rhs.thread_id))
        });

        let mut timelines = self
            .timeline_store
            .iter()
            .map(|(key, entries)| MissionThreadTimeline {
                worker_id: key.worker_id.clone(),
                thread_id: key.thread_id.clone(),
                entries: entries.iter().cloned().collect(),
            })
            .collect::<Vec<_>>();
        timelines.sort_by(|lhs, rhs| {
            lhs.worker_id
                .cmp(&rhs.worker_id)
                .then(lhs.thread_id.cmp(&rhs.thread_id))
        });

        let mut events = self.event_store.iter().cloned().collect::<Vec<_>>();
        events.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));

        let mut requests = self.request_store.values().cloned().collect::<Vec<_>>();
        requests.sort_by(|lhs, rhs| lhs.request_id.cmp(&rhs.request_id));

        let active_worker_id = self.active_lane.as_ref().map(|key| key.worker_id.clone());
        let active_thread_id = self.active_lane.as_ref().map(|key| key.thread_id.clone());
        let active_turn_id = self
            .active_lane
            .as_ref()
            .and_then(|key| self.thread_store.get(key))
            .and_then(|thread| thread.active_turn_id.clone());

        let compatibility_chat_messages = self
            .active_lane
            .as_ref()
            .and_then(|key| self.timeline_store.get(key))
            .map(|entries| entries.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        MissionControlProjection {
            workers,
            threads,
            timelines,
            events,
            requests,
            active_worker_id,
            active_thread_id,
            active_turn_id,
            compatibility_chat_messages,
        }
    }

    fn ingest_worker_summary(
        &mut self,
        worker_id: String,
        status: String,
        heartbeat_state: Option<String>,
        latest_seq: Option<u64>,
        lag_events: Option<u64>,
        reconnect_state: Option<String>,
        occurred_at: Option<String>,
    ) {
        let Some(worker_id) = normalized_owned(Some(worker_id)) else {
            return;
        };
        let worker = self
            .worker_store
            .entry(worker_id.clone())
            .or_insert_with(|| MissionWorkerState::new(worker_id.clone()));
        worker.status = normalized_owned(Some(status)).unwrap_or_else(|| "unknown".to_string());
        worker.heartbeat_state = normalized_owned(heartbeat_state);
        worker.latest_seq = latest_seq.or(worker.latest_seq);
        worker.lag_events = lag_events.or(worker.lag_events);
        worker.reconnect_state = normalized_owned(reconnect_state).or(worker.reconnect_state.clone());
        worker.last_event_at = normalized_owned(occurred_at).or(worker.last_event_at.clone());
        let worker_id = worker.worker_id.clone();
        self.refresh_worker_request_stats(worker_id.as_str());
    }

    fn ingest_stream_event(
        &mut self,
        topic: String,
        seq: Option<u64>,
        worker_id: Option<String>,
        payload: Value,
    ) {
        let topic = normalized_owned(Some(topic)).unwrap_or_else(|| "runtime.codex_worker_events".to_string());
        if let Some(seq) = seq {
            let dedupe_key = format!("{topic}:{seq}");
            if !self.event_dedupe.insert(dedupe_key) {
                return;
            }
        }

        let payload_object = payload.as_object().cloned().unwrap_or_default();
        let event_type = object_string(&payload_object, &["eventType", "event_type"]);
        let payload_body = payload_object
            .get("payload")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let method = object_string(&payload_body, &["method"])
            .or_else(|| object_string(&payload_object, &["method"]));

        let resolved_worker_id = object_string(&payload_object, &["workerId", "worker_id"])
            .or_else(|| object_string(&payload_body, &["workerId", "worker_id"]))
            .or_else(|| normalized_owned(worker_id));

        let params = payload_body
            .get("params")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let thread_id = extract_thread_id(&params).or_else(|| extract_thread_id(&payload_body));
        let turn_id = extract_turn_id(&params).or_else(|| extract_turn_id(&payload_body));
        let item_id = extract_item_id(&params).or_else(|| extract_item_id(&payload_body));
        let occurred_at = object_string(&payload_body, &["occurred_at"])
            .or_else(|| object_string(&payload_object, &["occurred_at"]));
        let request_id = object_string(&payload_body, &["request_id", "requestId"])
            .or_else(|| object_string(&params, &["request_id", "requestId"]));

        let severity = classify_event_severity(event_type.as_deref(), method.as_deref());
        let summary = summarize_event(event_type.as_deref(), method.as_deref(), request_id.as_deref());

        let record = MissionEventRecord {
            id: self.next_event_id,
            topic: topic.clone(),
            seq,
            worker_id: resolved_worker_id.clone(),
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            request_id: request_id.clone(),
            event_type: event_type.clone(),
            method: method.clone(),
            summary,
            severity,
            occurred_at: occurred_at.clone(),
            payload,
            resync_marker: false,
        };
        self.next_event_id = self.next_event_id.saturating_add(1);

        if let Some(worker_id) = resolved_worker_id.as_deref() {
            let worker = self
                .worker_store
                .entry(worker_id.to_string())
                .or_insert_with(|| MissionWorkerState::new(worker_id.to_string()));
            worker.latest_seq = seq.or(worker.latest_seq);
            worker.last_event_at = occurred_at.clone().or(worker.last_event_at.clone());

            if matches!(method.as_deref(), Some("turn/started")) {
                worker.running_turns = worker.running_turns.saturating_add(1);
            }
            if matches!(
                method.as_deref(),
                Some("turn/completed")
                    | Some("turn/failed")
                    | Some("turn/interrupted")
                    | Some("turn/aborted")
            ) {
                worker.running_turns = worker.running_turns.saturating_sub(1);
            }
        }

        if let (Some(worker_id), Some(thread_id)) = (resolved_worker_id.as_deref(), thread_id.as_deref()) {
            if let Some(key) = WorkerThreadKey::new(worker_id, thread_id) {
                let thread = self
                    .thread_store
                    .entry(key.clone())
                    .or_insert_with(|| MissionThreadState::new(&key));
                thread.last_event_at = occurred_at.clone().or(thread.last_event_at.clone());
                thread.freshness_seq = seq.or(thread.freshness_seq);
                thread.last_summary = summarize_event(
                    event_type.as_deref(),
                    method.as_deref(),
                    request_id.as_deref(),
                );

                if matches!(method.as_deref(), Some("turn/started")) {
                    thread.active_turn_id = turn_id.clone();
                }
                if matches!(
                    method.as_deref(),
                    Some("turn/completed")
                        | Some("turn/failed")
                        | Some("turn/interrupted")
                        | Some("turn/aborted")
                ) {
                    thread.active_turn_id = None;
                }

                if self
                    .active_lane
                    .as_ref()
                    .map(|active| active != &key)
                    .unwrap_or(true)
                {
                    thread.unread_count = thread.unread_count.saturating_add(1);
                }

                if self.active_lane.is_none() {
                    self.active_lane = Some(key.clone());
                }

                if let Some(timeline_item) = timeline_item_from_event(
                    self.next_event_id,
                    worker_id,
                    thread_id,
                    turn_id.as_deref(),
                    item_id.as_deref(),
                    method.as_deref(),
                    &params,
                    occurred_at.as_deref(),
                ) {
                    self.timeline_store
                        .entry(key)
                        .or_default()
                        .push_back(timeline_item);
                    self.enforce_timeline_bound();
                }
            }
        }

        self.reconcile_request_from_event(
            resolved_worker_id.as_deref(),
            request_id.as_deref(),
            method.as_deref(),
            event_type.as_deref(),
            thread_id.as_deref(),
            occurred_at.as_deref(),
            payload_body,
        );

        self.event_store.push_back(record);
        self.enforce_event_bound();

        if let Some(worker_id) = resolved_worker_id.as_deref() {
            self.refresh_worker_request_stats(worker_id);
        }
    }

    fn reconcile_request_from_event(
        &mut self,
        worker_id: Option<&str>,
        request_id: Option<&str>,
        method: Option<&str>,
        event_type: Option<&str>,
        thread_id: Option<&str>,
        occurred_at: Option<&str>,
        payload_body: serde_json::Map<String, Value>,
    ) {
        let Some(worker_id) = normalized_owned(worker_id.map(ToString::to_string)) else {
            return;
        };
        let Some(request_id) = normalized_owned(request_id.map(ToString::to_string)) else {
            return;
        };
        let method = normalized_owned(method.map(ToString::to_string)).unwrap_or_else(|| "unknown".to_string());

        let state = match event_type {
            Some("worker.request") => "running",
            Some("worker.response") => "success",
            Some("worker.error") => "error",
            _ => "running",
        };

        let thread_id = normalized_owned(thread_id.map(ToString::to_string));
        let occurred_at = normalized_owned(occurred_at.map(ToString::to_string));
        let entry = self
            .request_store
            .entry(request_id.clone())
            .or_insert_with(|| {
                MissionRequestState::from_envelope(
                    worker_id.clone(),
                    request_id.clone(),
                    method.clone(),
                    "queued".to_string(),
                    thread_id.clone(),
                    occurred_at.clone(),
                )
            });

        entry.worker_id = worker_id;
        entry.method = method;
        entry.thread_id = thread_id.or(entry.thread_id.clone());
        entry.state = state.to_string();
        entry.occurred_at = occurred_at.or(entry.occurred_at.clone());

        if state == "success" {
            entry.response = payload_body.get("response").cloned();
            entry.error_code = None;
            entry.error_message = None;
            entry.retryable = false;
        } else if state == "error" {
            entry.error_code = payload_body
                .get("code")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            entry.error_message = payload_body
                .get("message")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            entry.retryable = payload_body
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
    }

    fn upsert_request(&mut self, request: MissionRequestState) {
        let Some(request_id) = normalized_owned(Some(request.request_id.clone())) else {
            return;
        };

        self.request_store.insert(request_id.clone(), request.clone());
        self.refresh_worker_request_stats(request.worker_id.as_str());

        if let Some(thread_id) = normalized_owned(request.thread_id.clone()) {
            if let Some(key) = WorkerThreadKey::new(request.worker_id.clone(), thread_id) {
                let thread = self
                    .thread_store
                    .entry(key)
                    .or_insert_with(|| MissionThreadState::new(&WorkerThreadKey {
                        worker_id: request.worker_id.clone(),
                        thread_id: request.thread_id.clone().unwrap_or_default(),
                    }));
                thread.last_summary = format!("{} [{}]", request.method, request.state);
            }
        }
    }

    fn set_active_lane(&mut self, worker_id: Option<String>, thread_id: Option<String>) {
        let Some(worker_id) = normalized_owned(worker_id) else {
            self.active_lane = None;
            return;
        };
        let Some(thread_id) = normalized_owned(thread_id) else {
            self.active_lane = None;
            return;
        };

        let Some(key) = WorkerThreadKey::new(worker_id, thread_id) else {
            self.active_lane = None;
            return;
        };

        if let Some(thread) = self.thread_store.get_mut(&key) {
            thread.unread_count = 0;
        }
        self.active_lane = Some(key);
    }

    fn set_lane_muted(&mut self, worker_id: String, thread_id: String, muted: bool) {
        let Some(key) = WorkerThreadKey::new(worker_id, thread_id) else {
            return;
        };
        let thread = self
            .thread_store
            .entry(key.clone())
            .or_insert_with(|| MissionThreadState::new(&key));
        thread.muted = muted;
    }

    fn mark_resynced(
        &mut self,
        topic: String,
        from_seq: u64,
        to_seq: u64,
        worker_id: Option<String>,
    ) {
        let summary = format!("resynced {} -> {}", from_seq, to_seq);
        let record = MissionEventRecord {
            id: self.next_event_id,
            topic,
            seq: Some(to_seq),
            worker_id: normalized_owned(worker_id),
            thread_id: None,
            turn_id: None,
            request_id: None,
            event_type: Some("sync.resynced".to_string()),
            method: Some("sync/resynced".to_string()),
            summary,
            severity: MissionEventSeverity::Warning,
            occurred_at: None,
            payload: serde_json::json!({"from": from_seq, "to": to_seq}),
            resync_marker: true,
        };
        self.next_event_id = self.next_event_id.saturating_add(1);
        self.event_store.push_back(record);
        self.enforce_event_bound();
    }

    fn refresh_worker_request_stats(&mut self, worker_id: &str) {
        let worker = self
            .worker_store
            .entry(worker_id.to_string())
            .or_insert_with(|| MissionWorkerState::new(worker_id.to_string()));

        let mut queued = 0_u64;
        let mut failed = 0_u64;
        for request in self.request_store.values() {
            if request.worker_id != worker_id {
                continue;
            }
            if request.state == "queued" || request.state == "running" {
                queued = queued.saturating_add(1);
            }
            if request.state == "error" {
                failed = failed.saturating_add(1);
            }
        }

        worker.queued_requests = queued;
        worker.failed_requests = failed;
    }

    fn enforce_event_bound(&mut self) {
        while self.event_store.len() > self.max_events {
            if let Some(dropped) = self.event_store.pop_front() {
                if let Some(seq) = dropped.seq {
                    let dedupe_key = format!("{}:{}", dropped.topic, seq);
                    self.event_dedupe.remove(dedupe_key.as_str());
                }
            }
        }
    }

    fn enforce_timeline_bound(&mut self) {
        for timeline in self.timeline_store.values_mut() {
            while timeline.len() > self.max_timeline_entries {
                timeline.pop_front();
            }
        }
    }
}

fn normalized_owned(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn object_string(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn extract_thread_id(object: &serde_json::Map<String, Value>) -> Option<String> {
    object_string(object, &["thread_id", "threadId", "conversation_id", "conversationId"]).or_else(
        || {
            object
                .get("thread")
                .and_then(Value::as_object)
                .and_then(|thread| object_string(thread, &["id"]))
        },
    )
}

fn extract_turn_id(object: &serde_json::Map<String, Value>) -> Option<String> {
    object_string(object, &["turn_id", "turnId"]).or_else(|| {
        object
            .get("turn")
            .and_then(Value::as_object)
            .and_then(|turn| object_string(turn, &["id"]))
    })
}

fn extract_item_id(object: &serde_json::Map<String, Value>) -> Option<String> {
    object_string(object, &["item_id", "itemId"]).or_else(|| {
        object
            .get("item")
            .and_then(Value::as_object)
            .and_then(|item| object_string(item, &["id"]))
    })
}

fn classify_event_severity(event_type: Option<&str>, method: Option<&str>) -> MissionEventSeverity {
    if matches!(event_type, Some("worker.error")) || method_is_error(method) {
        MissionEventSeverity::Error
    } else if matches!(
        method,
        Some("sync/resynced")
            | Some("turn/failed")
            | Some("turn/interrupted")
            | Some("turn/aborted")
    ) {
        MissionEventSeverity::Warning
    } else {
        MissionEventSeverity::Info
    }
}

fn method_is_error(method: Option<&str>) -> bool {
    let Some(method) = method else {
        return false;
    };
    let method = method.to_ascii_lowercase();
    method.contains("error") || method.contains("failed")
}

fn summarize_event(event_type: Option<&str>, method: Option<&str>, request_id: Option<&str>) -> String {
    if let Some(method) = method {
        if let Some(request_id) = request_id {
            return format!("{} ({})", method, request_id);
        }
        return method.to_string();
    }

    event_type.unwrap_or("worker.event").to_string()
}

fn timeline_item_from_event(
    event_id: u64,
    worker_id: &str,
    thread_id: &str,
    turn_id: Option<&str>,
    item_id: Option<&str>,
    method: Option<&str>,
    params: &serde_json::Map<String, Value>,
    occurred_at: Option<&str>,
) -> Option<MissionTimelineItem> {
    let method = method?;

    let mut role = "system".to_string();
    let mut text = method.to_string();
    let mut is_streaming = false;

    match method {
        "codex/event/user_message" => {
            if let Some(msg) = params
                .get("msg")
                .and_then(Value::as_object)
                .and_then(|msg| object_string(msg, &["message"]))
            {
                role = "user".to_string();
                text = msg;
            }
        }
        "item/agentMessage/delta"
        | "codex/event/agent_message_content_delta"
        | "codex/event/agent_message_delta" => {
            let delta = object_string(params, &["delta"]).or_else(|| {
                params
                    .get("msg")
                    .and_then(Value::as_object)
                    .and_then(|msg| object_string(msg, &["delta"]))
            });
            if let Some(delta) = delta {
                role = "assistant".to_string();
                text = delta;
                is_streaming = true;
            }
        }
        "item/reasoning/summaryTextDelta"
        | "item/reasoning/textDelta"
        | "item/reasoning/contentDelta" => {
            if let Some(delta) = object_string(params, &["delta"]) {
                role = "reasoning".to_string();
                text = delta;
                is_streaming = true;
            }
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            if let Some(delta) = object_string(params, &["delta"]) {
                role = "tool".to_string();
                text = delta;
                is_streaming = true;
            }
        }
        "error" | "codex/error" => {
            role = "error".to_string();
            text = object_string(params, &["message"]).unwrap_or_else(|| "Codex error".to_string());
        }
        _ => {
            if method.contains("turn/") || method.contains("thread/") {
                role = "system".to_string();
                text = method.to_string();
            } else {
                return None;
            }
        }
    }

    Some(MissionTimelineItem {
        id: format!("timeline:{}", event_id),
        role,
        text,
        is_streaming,
        worker_id: worker_id.to_string(),
        thread_id: thread_id.to_string(),
        turn_id: turn_id.and_then(|value| normalized_owned(Some(value.to_string()))),
        item_id: item_id.and_then(|value| normalized_owned(Some(value.to_string()))),
        occurred_at: occurred_at.and_then(|value| normalized_owned(Some(value.to_string()))),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        IosMissionControlStore, MissionControlCommand, MissionRequestState, MissionTimelineItem,
        WorkerThreadKey,
    };

    #[test]
    fn mission_store_populates_worker_thread_timeline_and_event_ring() {
        let mut store = IosMissionControlStore::new();

        store.apply_command(MissionControlCommand::IngestWorkerSummary {
            worker_id: "desktopw:shared".to_string(),
            status: "running".to_string(),
            heartbeat_state: Some("fresh".to_string()),
            latest_seq: Some(41),
            lag_events: Some(0),
            reconnect_state: Some("live".to_string()),
            occurred_at: Some("2026-02-22T09:00:00Z".to_string()),
        });

        store.apply_command(MissionControlCommand::IngestStreamEvent {
            topic: "runtime.codex_worker_events".to_string(),
            seq: Some(42),
            worker_id: Some("desktopw:shared".to_string()),
            payload: serde_json::json!({
                "workerId": "desktopw:shared",
                "seq": 42,
                "eventType": "worker.event",
                "payload": {
                    "method": "codex/event/user_message",
                    "occurred_at": "2026-02-22T09:00:01Z",
                    "params": {
                        "thread_id": "thread-1",
                        "turn_id": "turn-1",
                        "msg": {"message": "hello"}
                    }
                }
            }),
        });

        let projection = store.projection();
        assert_eq!(projection.workers.len(), 1);
        assert_eq!(projection.threads.len(), 1);
        assert_eq!(projection.events.len(), 1);
        assert_eq!(projection.active_worker_id.as_deref(), Some("desktopw:shared"));
        assert_eq!(projection.active_thread_id.as_deref(), Some("thread-1"));
        assert_eq!(projection.compatibility_chat_messages.len(), 1);
        assert_eq!(projection.compatibility_chat_messages[0].role, "user");
        assert_eq!(projection.compatibility_chat_messages[0].text, "hello");
    }

    #[test]
    fn mission_store_event_ring_and_dedupe_are_bounded() {
        let mut store = IosMissionControlStore::new();
        store.apply_command(MissionControlCommand::Configure {
            max_events: Some(2),
            max_timeline_entries: None,
        });

        for seq in [1_u64, 2_u64, 2_u64, 3_u64] {
            store.apply_command(MissionControlCommand::IngestStreamEvent {
                topic: "runtime.codex_worker_events".to_string(),
                seq: Some(seq),
                worker_id: Some("desktopw:shared".to_string()),
                payload: serde_json::json!({
                    "workerId": "desktopw:shared",
                    "eventType": "worker.event",
                    "payload": {
                        "method": "turn/started",
                        "params": {"thread_id": "thread-1", "turn_id": format!("turn-{}", seq)}
                    }
                }),
            });
        }

        let projection = store.projection();
        assert_eq!(projection.events.len(), 2);
        assert_eq!(projection.events[0].seq, Some(2));
        assert_eq!(projection.events[1].seq, Some(3));
    }

    #[test]
    fn mission_store_request_store_tracks_terminal_states() {
        let mut store = IosMissionControlStore::new();

        store.apply_command(MissionControlCommand::UpsertRequest {
            request: MissionRequestState {
                request_id: "req-1".to_string(),
                worker_id: "desktopw:shared".to_string(),
                thread_id: Some("thread-1".to_string()),
                method: "turn/start".to_string(),
                state: "queued".to_string(),
                occurred_at: Some("2026-02-22T09:00:01Z".to_string()),
                error_code: None,
                error_message: None,
                retryable: false,
                response: None,
            },
        });

        store.apply_command(MissionControlCommand::IngestStreamEvent {
            topic: "runtime.codex_worker_events".to_string(),
            seq: Some(50),
            worker_id: Some("desktopw:shared".to_string()),
            payload: serde_json::json!({
                "workerId": "desktopw:shared",
                "eventType": "worker.response",
                "payload": {
                    "request_id": "req-1",
                    "method": "turn/start",
                    "occurred_at": "2026-02-22T09:00:02Z",
                    "response": {"ok": true},
                    "params": {"thread_id": "thread-1"}
                }
            }),
        });

        let projection = store.projection();
        assert_eq!(projection.requests.len(), 1);
        assert_eq!(projection.requests[0].state, "success");
        assert_eq!(
            projection.requests[0]
                .response
                .as_ref()
                .and_then(|value| value.get("ok"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn mission_store_set_active_lane_drives_compatibility_timeline() {
        let mut store = IosMissionControlStore::new();
        for (worker_id, thread_id, text) in [
            ("desktopw:shared", "thread-1", "hello-1"),
            ("desktopw:shared", "thread-2", "hello-2"),
        ] {
            store.apply_command(MissionControlCommand::IngestStreamEvent {
                topic: "runtime.codex_worker_events".to_string(),
                seq: None,
                worker_id: Some(worker_id.to_string()),
                payload: serde_json::json!({
                    "workerId": worker_id,
                    "eventType": "worker.event",
                    "payload": {
                        "method": "codex/event/user_message",
                        "params": {
                            "thread_id": thread_id,
                            "msg": {"message": text}
                        }
                    }
                }),
            });
        }

        store.apply_command(MissionControlCommand::SetActiveLane {
            worker_id: Some("desktopw:shared".to_string()),
            thread_id: Some("thread-2".to_string()),
        });

        let projection = store.projection();
        assert_eq!(projection.active_thread_id.as_deref(), Some("thread-2"));
        assert_eq!(projection.compatibility_chat_messages.len(), 1);
        assert_eq!(projection.compatibility_chat_messages[0].text, "hello-2");
    }

    #[test]
    fn worker_thread_key_normalizes_ids() {
        let key = WorkerThreadKey::new(" desktopw:shared ", " thread-1 ")
            .expect("key should normalize values");
        assert_eq!(key.worker_id, "desktopw:shared");
        assert_eq!(key.thread_id, "thread-1");
    }

    #[test]
    fn timeline_item_round_trip_shape_is_stable() {
        let item = MissionTimelineItem {
            id: "timeline:1".to_string(),
            role: "assistant".to_string(),
            text: "ok".to_string(),
            is_streaming: true,
            worker_id: "desktopw:shared".to_string(),
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            item_id: Some("item-1".to_string()),
            occurred_at: Some("2026-02-22T09:00:00Z".to_string()),
        };

        let encoded = serde_json::to_string(&item).expect("timeline item should encode");
        let decoded = serde_json::from_str::<MissionTimelineItem>(&encoded)
            .expect("timeline item should decode");
        assert_eq!(decoded, item);
    }
}
