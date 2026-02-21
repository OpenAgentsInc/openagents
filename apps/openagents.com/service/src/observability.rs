use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{SecondsFormat, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub event_name: String,
    pub request_id: String,
    pub outcome: String,
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, String>,
}

impl AuditEvent {
    pub fn new(event_name: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            event_name: event_name.into(),
            request_id: request_id.into(),
            outcome: "success".to_string(),
            occurred_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            user_id: None,
            session_id: None,
            org_id: None,
            device_id: None,
            attributes: HashMap::new(),
        }
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = outcome.into();
        self
    }

    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_org_id(mut self, org_id: impl Into<String>) -> Self {
        self.org_id = Some(org_id.into());
        self
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

pub trait AuditSink: Send + Sync {
    fn record(&self, event: &AuditEvent);
}

#[derive(Default)]
pub struct TracingAuditSink;

impl AuditSink for TracingAuditSink {
    fn record(&self, event: &AuditEvent) {
        let payload = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
        tracing::info!(
            target: "oa.audit",
            event_name = %event.event_name,
            request_id = %event.request_id,
            outcome = %event.outcome,
            user_id = %event.user_id.clone().unwrap_or_default(),
            session_id = %event.session_id.clone().unwrap_or_default(),
            org_id = %event.org_id.clone().unwrap_or_default(),
            device_id = %event.device_id.clone().unwrap_or_default(),
            attributes_json = %serde_json::to_string(&event.attributes).unwrap_or_else(|_| "{}".to_string()),
            event_json = %payload,
            "audit_event"
        );
    }
}

#[derive(Clone)]
pub struct Observability {
    sink: Arc<dyn AuditSink>,
    counters: Arc<Mutex<HashMap<String, u64>>>,
}

impl Default for Observability {
    fn default() -> Self {
        Self::new(Arc::new(TracingAuditSink))
    }
}

impl Observability {
    pub fn new(sink: Arc<dyn AuditSink>) -> Self {
        Self {
            sink,
            counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn audit(&self, event: AuditEvent) {
        self.sink.record(&event);
    }

    pub fn increment_counter(&self, metric_name: &str, request_id: &str) {
        let mut counters = self
            .counters
            .lock()
            .expect("observability counters mutex poisoned");
        let value = counters.entry(metric_name.to_string()).or_insert(0);
        *value += 1;
        tracing::info!(
            target: "oa.metric",
            metric_name = %metric_name,
            metric_value = *value,
            request_id = %request_id,
            "metric_counter_incremented"
        );
    }
}

#[derive(Clone, Default)]
pub struct RecordingAuditSink {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl RecordingAuditSink {
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .expect("recording audit sink mutex poisoned")
            .clone()
    }
}

impl AuditSink for RecordingAuditSink {
    fn record(&self, event: &AuditEvent) {
        self.events
            .lock()
            .expect("recording audit sink mutex poisoned")
            .push(event.clone());
    }
}
