use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum LifecycleStage {
    Ingest,
    Retrieve,
    Draft,
    Approve,
    Send,
    FollowUp,
}

impl LifecycleStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Retrieve => "retrieve",
            Self::Draft => "draft",
            Self::Approve => "approve",
            Self::Send => "send",
            Self::FollowUp => "follow_up",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PipelineEventStatus {
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PipelineEventInput {
    pub correlation_id: String,
    pub stage: LifecycleStage,
    pub status: PipelineEventStatus,
    pub occurred_at_unix: u64,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PipelineEvent {
    pub event_id: String,
    pub correlation_id: String,
    pub stage: LifecycleStage,
    pub status: PipelineEventStatus,
    pub occurred_at_unix: u64,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RedactedPipelineEvent {
    pub event_id: String,
    pub correlation_id: String,
    pub stage: LifecycleStage,
    pub status: PipelineEventStatus,
    pub occurred_at_unix: u64,
    pub metadata: BTreeMap<String, String>,
    pub redacted_metadata_keys: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct PipelineAuditTrail {
    pub events: Vec<PipelineEvent>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendTraceReport {
    pub correlation_id: String,
    pub stage_events: BTreeMap<LifecycleStage, Vec<RedactedPipelineEvent>>,
    pub missing_required_stages: Vec<LifecycleStage>,
    pub is_send_trace_complete: bool,
    pub redacted_metadata_count: usize,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum PipelineAuditError {
    #[error("invalid pipeline event input: {0}")]
    InvalidInput(String),
}

pub fn derive_correlation_id(seed_parts: &[&str]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for part in seed_parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= u64::from(b'|');
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("trace-{hash:016x}")
}

pub fn record_pipeline_event(
    trail: &mut PipelineAuditTrail,
    input: PipelineEventInput,
) -> Result<PipelineEvent, PipelineAuditError> {
    if input.correlation_id.trim().is_empty() {
        return Err(PipelineAuditError::InvalidInput(
            "correlation_id must not be empty".to_string(),
        ));
    }

    let event = PipelineEvent {
        event_id: format!("evt-{:06}", trail.events.len().saturating_add(1)),
        correlation_id: input.correlation_id,
        stage: input.stage,
        status: input.status,
        occurred_at_unix: input.occurred_at_unix,
        metadata: input.metadata,
    };
    trail.events.push(event.clone());
    Ok(event)
}

pub fn diagnostics_for_correlation(trail: &PipelineAuditTrail, correlation_id: &str) -> SendTraceReport {
    let mut stage_events = BTreeMap::<LifecycleStage, Vec<RedactedPipelineEvent>>::new();
    let mut redacted_metadata_count = 0usize;

    let mut matched = trail
        .events
        .iter()
        .filter(|event| event.correlation_id == correlation_id)
        .cloned()
        .collect::<Vec<PipelineEvent>>();
    matched.sort_by(|left, right| {
        left.occurred_at_unix
            .cmp(&right.occurred_at_unix)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });

    for event in matched {
        let (metadata, redacted_keys) = redact_metadata(event.metadata);
        redacted_metadata_count = redacted_metadata_count.saturating_add(redacted_keys.len());
        stage_events
            .entry(event.stage)
            .or_default()
            .push(RedactedPipelineEvent {
                event_id: event.event_id,
                correlation_id: event.correlation_id,
                stage: event.stage,
                status: event.status,
                occurred_at_unix: event.occurred_at_unix,
                metadata,
                redacted_metadata_keys: redacted_keys,
            });
    }

    let required = [
        LifecycleStage::Ingest,
        LifecycleStage::Retrieve,
        LifecycleStage::Draft,
        LifecycleStage::Approve,
        LifecycleStage::Send,
    ];
    let mut missing_required_stages = Vec::<LifecycleStage>::new();
    for stage in required {
        if !stage_events.contains_key(&stage) {
            missing_required_stages.push(stage);
        }
    }

    SendTraceReport {
        correlation_id: correlation_id.to_string(),
        stage_events,
        is_send_trace_complete: missing_required_stages.is_empty(),
        missing_required_stages,
        redacted_metadata_count,
    }
}

fn redact_metadata(metadata: BTreeMap<String, String>) -> (BTreeMap<String, String>, Vec<String>) {
    let mut redacted = BTreeMap::<String, String>::new();
    let mut redacted_keys = Vec::<String>::new();
    for (key, value) in metadata {
        if should_redact_metadata_key(key.as_str()) {
            redacted_keys.push(key.clone());
            redacted.insert(key, "<redacted>".to_string());
        } else {
            redacted.insert(key, value);
        }
    }
    (redacted, redacted_keys)
}

fn should_redact_metadata_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "mnemonic",
        "authorization",
        "api_key",
        "refresh_key",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{
        LifecycleStage, PipelineAuditTrail, PipelineEventInput, PipelineEventStatus,
        derive_correlation_id, diagnostics_for_correlation, record_pipeline_event,
    };
    use std::collections::BTreeMap;

    fn metadata(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn diagnostics_redacts_sensitive_metadata_in_logs() {
        let correlation_id = derive_correlation_id(&["tenant-a", "thread-1", "msg-1"]);
        let mut trail = PipelineAuditTrail::default();
        record_pipeline_event(
            &mut trail,
            PipelineEventInput {
                correlation_id: correlation_id.clone(),
                stage: LifecycleStage::Ingest,
                status: PipelineEventStatus::Completed,
                occurred_at_unix: 100,
                metadata: metadata(&[
                    ("gmail_access_token", "secret-token-value"),
                    ("message_id", "m1"),
                ]),
            },
        )
        .expect("event should record");

        let report = diagnostics_for_correlation(&trail, correlation_id.as_str());
        let ingest = &report.stage_events[&LifecycleStage::Ingest][0];
        assert_eq!(
            ingest.metadata.get("gmail_access_token"),
            Some(&"<redacted>".to_string())
        );
        assert_eq!(ingest.metadata.get("message_id"), Some(&"m1".to_string()));
        assert!(ingest.redacted_metadata_keys.contains(&"gmail_access_token".to_string()));
    }

    #[test]
    fn send_trace_report_covers_required_pipeline_stages() {
        let correlation_id = derive_correlation_id(&["tenant-b", "thread-2", "msg-2"]);
        let mut trail = PipelineAuditTrail::default();
        for (index, stage) in [
            LifecycleStage::Ingest,
            LifecycleStage::Retrieve,
            LifecycleStage::Draft,
            LifecycleStage::Approve,
            LifecycleStage::Send,
            LifecycleStage::FollowUp,
        ]
        .iter()
        .enumerate()
        {
            record_pipeline_event(
                &mut trail,
                PipelineEventInput {
                    correlation_id: correlation_id.clone(),
                    stage: *stage,
                    status: PipelineEventStatus::Completed,
                    occurred_at_unix: 200 + index as u64,
                    metadata: metadata(&[("event", stage.label())]),
                },
            )
            .expect("event should record");
        }

        let report = diagnostics_for_correlation(&trail, correlation_id.as_str());
        assert!(report.is_send_trace_complete);
        assert!(report.missing_required_stages.is_empty());
        assert!(report.stage_events.contains_key(&LifecycleStage::Send));
        assert!(report.stage_events.contains_key(&LifecycleStage::Draft));
    }

    #[test]
    fn send_trace_report_flags_missing_required_stages() {
        let correlation_id = derive_correlation_id(&["tenant-c", "thread-3", "msg-3"]);
        let mut trail = PipelineAuditTrail::default();
        for (index, stage) in [
            LifecycleStage::Ingest,
            LifecycleStage::Draft,
            LifecycleStage::Send,
        ]
        .iter()
        .enumerate()
        {
            record_pipeline_event(
                &mut trail,
                PipelineEventInput {
                    correlation_id: correlation_id.clone(),
                    stage: *stage,
                    status: PipelineEventStatus::Completed,
                    occurred_at_unix: 300 + index as u64,
                    metadata: metadata(&[("event", stage.label())]),
                },
            )
            .expect("event should record");
        }

        let report = diagnostics_for_correlation(&trail, correlation_id.as_str());
        assert!(!report.is_send_trace_complete);
        assert!(report.missing_required_stages.contains(&LifecycleStage::Retrieve));
        assert!(report.missing_required_stages.contains(&LifecycleStage::Approve));
    }
}
