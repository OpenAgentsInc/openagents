use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum DataCategory {
    InboundMessage,
    Draft,
    SendAudit,
    FollowUpEvent,
    KnowledgeDocument,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DataRecord {
    pub record_id: String,
    pub category: DataCategory,
    pub created_at_unix: u64,
    pub metadata: BTreeMap<String, String>,
    pub content: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetentionPolicy {
    pub inbound_days: u32,
    pub draft_days: u32,
    pub send_audit_days: u32,
    pub follow_up_days: u32,
    pub knowledge_doc_days: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            inbound_days: 30,
            draft_days: 30,
            send_audit_days: 180,
            follow_up_days: 90,
            knowledge_doc_days: 365,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AccessRole {
    Operator,
    Auditor,
    Automation,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExportScope {
    MetadataOnly,
    FullContent,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AccessAction {
    Export,
    Delete,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AccessAuditEvent {
    pub event_id: String,
    pub actor_id: String,
    pub role: AccessRole,
    pub action: AccessAction,
    pub at_unix: u64,
    pub outcome: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SecurityPrivacyState {
    pub records: BTreeMap<String, DataRecord>,
    pub access_audit_log: Vec<AccessAuditEvent>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetentionSweepOutcome {
    pub deleted_record_ids: Vec<String>,
    pub remaining_record_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeletionRequest {
    pub actor_id: String,
    pub role: AccessRole,
    pub reason: String,
    pub requested_at_unix: u64,
    pub record_ids: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeletionReceipt {
    pub receipt_id: String,
    pub actor_id: String,
    pub reason: String,
    pub requested_at_unix: u64,
    pub deleted_record_ids: Vec<String>,
    pub missing_record_ids: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExportRequest {
    pub actor_id: String,
    pub role: AccessRole,
    pub requested_at_unix: u64,
    pub scope: ExportScope,
    pub categories: Vec<DataCategory>,
    pub start_unix: Option<u64>,
    pub end_unix: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExportRecord {
    pub record_id: String,
    pub category: DataCategory,
    pub created_at_unix: u64,
    pub metadata: BTreeMap<String, String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExportBundle {
    pub export_id: String,
    pub actor_id: String,
    pub scope: ExportScope,
    pub records: Vec<ExportRecord>,
    pub redacted_field_count: usize,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum SecurityPrivacyError {
    #[error("invalid security/privacy input: {0}")]
    InvalidInput(String),
    #[error("unauthorized export: {0}")]
    UnauthorizedExport(String),
}

pub fn enforce_retention_policy(
    state: &mut SecurityPrivacyState,
    policy: &RetentionPolicy,
    now_unix: u64,
) -> Result<RetentionSweepOutcome, SecurityPrivacyError> {
    validate_retention_policy(policy)?;

    let mut deleted = Vec::<String>::new();
    let candidate_ids = state.records.keys().cloned().collect::<Vec<String>>();
    for record_id in candidate_ids {
        let Some(record) = state.records.get(record_id.as_str()) else {
            continue;
        };
        let ttl_seconds = retention_ttl_seconds(record.category, policy);
        if record.created_at_unix.saturating_add(ttl_seconds) <= now_unix {
            state.records.remove(record_id.as_str());
            deleted.push(record_id);
        }
    }
    deleted.sort();

    Ok(RetentionSweepOutcome {
        deleted_record_ids: deleted,
        remaining_record_count: state.records.len(),
    })
}

pub fn run_deletion_workflow(
    state: &mut SecurityPrivacyState,
    request: DeletionRequest,
) -> Result<DeletionReceipt, SecurityPrivacyError> {
    if request.actor_id.trim().is_empty() {
        return Err(SecurityPrivacyError::InvalidInput(
            "actor_id must not be empty".to_string(),
        ));
    }
    if request.reason.trim().is_empty() {
        return Err(SecurityPrivacyError::InvalidInput(
            "reason must not be empty".to_string(),
        ));
    }

    let mut deleted_record_ids = Vec::<String>::new();
    let mut missing_record_ids = Vec::<String>::new();
    for record_id in request.record_ids {
        if state.records.remove(record_id.as_str()).is_some() {
            deleted_record_ids.push(record_id);
        } else {
            missing_record_ids.push(record_id);
        }
    }
    deleted_record_ids.sort();
    missing_record_ids.sort();

    let receipt = DeletionReceipt {
        receipt_id: format!("deletion-{:06}", state.access_audit_log.len().saturating_add(1)),
        actor_id: request.actor_id.clone(),
        reason: request.reason.clone(),
        requested_at_unix: request.requested_at_unix,
        deleted_record_ids,
        missing_record_ids,
    };
    push_access_audit_event(
        state,
        request.actor_id,
        request.role,
        AccessAction::Delete,
        request.requested_at_unix,
        format!("deleted={} missing={}", receipt.deleted_record_ids.len(), receipt.missing_record_ids.len()),
    );

    Ok(receipt)
}

pub fn export_records(
    state: &mut SecurityPrivacyState,
    request: ExportRequest,
) -> Result<ExportBundle, SecurityPrivacyError> {
    if request.actor_id.trim().is_empty() {
        return Err(SecurityPrivacyError::InvalidInput(
            "actor_id must not be empty".to_string(),
        ));
    }
    if request.categories.is_empty() {
        return Err(SecurityPrivacyError::InvalidInput(
            "categories must not be empty".to_string(),
        ));
    }
    if request.scope == ExportScope::FullContent && request.role != AccessRole::Auditor {
        push_access_audit_event(
            state,
            request.actor_id,
            request.role,
            AccessAction::Export,
            request.requested_at_unix,
            "denied: full content export requires auditor role".to_string(),
        );
        return Err(SecurityPrivacyError::UnauthorizedExport(
            "full content export requires auditor role".to_string(),
        ));
    }

    let mut redacted_field_count = 0usize;
    let mut records = state
        .records
        .values()
        .filter(|record| request.categories.contains(&record.category))
        .filter(|record| {
            request
                .start_unix
                .is_none_or(|start_unix| record.created_at_unix >= start_unix)
        })
        .filter(|record| {
            request
                .end_unix
                .is_none_or(|end_unix| record.created_at_unix <= end_unix)
        })
        .map(|record| {
            let (metadata, metadata_redactions) = redact_metadata(record.metadata.clone());
            redacted_field_count = redacted_field_count.saturating_add(metadata_redactions);

            let content = if request.scope == ExportScope::FullContent {
                Some(redact_debug_trace(record.content.as_str()))
            } else {
                None
            };
            if request.scope == ExportScope::FullContent {
                redacted_field_count =
                    redacted_field_count.saturating_add(content_redaction_count(record.content.as_str()));
            }

            ExportRecord {
                record_id: record.record_id.clone(),
                category: record.category,
                created_at_unix: record.created_at_unix,
                metadata,
                content,
            }
        })
        .collect::<Vec<ExportRecord>>();
    records.sort_by(|left, right| {
        left.created_at_unix
            .cmp(&right.created_at_unix)
            .then_with(|| left.record_id.cmp(&right.record_id))
    });

    push_access_audit_event(
        state,
        request.actor_id.clone(),
        request.role,
        AccessAction::Export,
        request.requested_at_unix,
        format!("allowed: scope={:?} records={}", request.scope, records.len()),
    );

    Ok(ExportBundle {
        export_id: format!("export-{:06}", state.access_audit_log.len()),
        actor_id: request.actor_id,
        scope: request.scope,
        records,
        redacted_field_count,
    })
}

pub fn redact_debug_trace(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| {
            let lowered = token.to_ascii_lowercase();
            if lowered.contains('@') && lowered.contains('.') {
                return "<redacted-email>".to_string();
            }
            if lowered.contains("token")
                || lowered.contains("secret")
                || lowered.contains("mnemonic")
                || lowered.contains("password")
                || lowered.starts_with("sk-")
            {
                return "<redacted>".to_string();
            }
            token.to_string()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn validate_retention_policy(policy: &RetentionPolicy) -> Result<(), SecurityPrivacyError> {
    for (name, days) in [
        ("inbound_days", policy.inbound_days),
        ("draft_days", policy.draft_days),
        ("send_audit_days", policy.send_audit_days),
        ("follow_up_days", policy.follow_up_days),
        ("knowledge_doc_days", policy.knowledge_doc_days),
    ] {
        if days == 0 {
            return Err(SecurityPrivacyError::InvalidInput(format!(
                "{name} must be greater than zero"
            )));
        }
    }
    Ok(())
}

fn retention_ttl_seconds(category: DataCategory, policy: &RetentionPolicy) -> u64 {
    let days = match category {
        DataCategory::InboundMessage => policy.inbound_days,
        DataCategory::Draft => policy.draft_days,
        DataCategory::SendAudit => policy.send_audit_days,
        DataCategory::FollowUpEvent => policy.follow_up_days,
        DataCategory::KnowledgeDocument => policy.knowledge_doc_days,
    };
    (days as u64).saturating_mul(86_400)
}

fn redact_metadata(metadata: BTreeMap<String, String>) -> (BTreeMap<String, String>, usize) {
    let mut redacted = BTreeMap::<String, String>::new();
    let mut count = 0usize;
    for (key, value) in metadata {
        if should_redact_key(key.as_str()) {
            redacted.insert(key, "<redacted>".to_string());
            count = count.saturating_add(1);
        } else {
            redacted.insert(key, value);
        }
    }
    (redacted, count)
}

fn should_redact_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "mnemonic",
        "authorization",
        "api_key",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

fn content_redaction_count(input: &str) -> usize {
    input
        .split_whitespace()
        .filter(|token| {
            let lowered = token.to_ascii_lowercase();
            (lowered.contains('@') && lowered.contains('.'))
                || lowered.contains("token")
                || lowered.contains("secret")
                || lowered.contains("mnemonic")
                || lowered.contains("password")
                || lowered.starts_with("sk-")
        })
        .count()
}

fn push_access_audit_event(
    state: &mut SecurityPrivacyState,
    actor_id: String,
    role: AccessRole,
    action: AccessAction,
    at_unix: u64,
    outcome: String,
) {
    state.access_audit_log.push(AccessAuditEvent {
        event_id: format!("access-{:06}", state.access_audit_log.len().saturating_add(1)),
        actor_id,
        role,
        action,
        at_unix,
        outcome,
    });
}

#[cfg(test)]
mod tests {
    use super::{
        AccessAction, AccessRole, DataCategory, DataRecord, DeletionRequest, ExportRequest,
        ExportScope, RetentionPolicy, SecurityPrivacyState, enforce_retention_policy,
        export_records, redact_debug_trace, run_deletion_workflow,
    };
    use std::collections::BTreeMap;

    fn record(record_id: &str, category: DataCategory, created_at_unix: u64, content: &str) -> DataRecord {
        DataRecord {
            record_id: record_id.to_string(),
            category,
            created_at_unix,
            metadata: BTreeMap::from([
                ("message_id".to_string(), record_id.to_string()),
                ("access_token".to_string(), "token-value".to_string()),
            ]),
            content: content.to_string(),
        }
    }

    #[test]
    fn retention_policy_deletes_expired_records() {
        let now = 10_000_000u64;
        let mut state = SecurityPrivacyState::default();
        state.records.insert(
            "old-inbound".to_string(),
            record(
                "old-inbound",
                DataCategory::InboundMessage,
                now.saturating_sub(40 * 86_400),
                "old",
            ),
        );
        state.records.insert(
            "fresh-draft".to_string(),
            record(
                "fresh-draft",
                DataCategory::Draft,
                now.saturating_sub(5 * 86_400),
                "fresh",
            ),
        );

        let outcome = enforce_retention_policy(&mut state, &RetentionPolicy::default(), now)
            .expect("retention sweep should succeed");
        assert_eq!(outcome.deleted_record_ids, vec!["old-inbound".to_string()]);
        assert_eq!(outcome.remaining_record_count, 1);
    }

    #[test]
    fn deletion_workflow_removes_records_and_audits_access() {
        let mut state = SecurityPrivacyState::default();
        state.records.insert(
            "msg-1".to_string(),
            record("msg-1", DataCategory::InboundMessage, 100, "content"),
        );
        state.records.insert(
            "msg-2".to_string(),
            record("msg-2", DataCategory::Draft, 100, "content"),
        );

        let receipt = run_deletion_workflow(
            &mut state,
            DeletionRequest {
                actor_id: "operator-1".to_string(),
                role: AccessRole::Operator,
                reason: "retention request".to_string(),
                requested_at_unix: 200,
                record_ids: vec!["msg-1".to_string(), "missing".to_string()],
            },
        )
        .expect("deletion should succeed");

        assert_eq!(receipt.deleted_record_ids, vec!["msg-1".to_string()]);
        assert_eq!(receipt.missing_record_ids, vec!["missing".to_string()]);
        assert_eq!(state.access_audit_log.len(), 1);
        assert_eq!(state.access_audit_log[0].action, AccessAction::Delete);
    }

    #[test]
    fn export_controls_enforce_role_and_redact_outputs() {
        let mut state = SecurityPrivacyState::default();
        state.records.insert(
            "msg-export".to_string(),
            record(
                "msg-export",
                DataCategory::InboundMessage,
                400,
                "contact ops@example.com token=abc123",
            ),
        );

        let denied = export_records(
            &mut state,
            ExportRequest {
                actor_id: "operator-2".to_string(),
                role: AccessRole::Operator,
                requested_at_unix: 500,
                scope: ExportScope::FullContent,
                categories: vec![DataCategory::InboundMessage],
                start_unix: None,
                end_unix: None,
            },
        )
        .expect_err("non-auditor full export should fail");
        assert!(denied.to_string().contains("auditor role"));

        let metadata_only = export_records(
            &mut state,
            ExportRequest {
                actor_id: "operator-2".to_string(),
                role: AccessRole::Operator,
                requested_at_unix: 501,
                scope: ExportScope::MetadataOnly,
                categories: vec![DataCategory::InboundMessage],
                start_unix: None,
                end_unix: None,
            },
        )
        .expect("metadata export should pass");
        assert_eq!(metadata_only.records.len(), 1);
        assert!(metadata_only.records[0].content.is_none());
        assert_eq!(
            metadata_only.records[0].metadata.get("access_token"),
            Some(&"<redacted>".to_string())
        );

        let full_content = export_records(
            &mut state,
            ExportRequest {
                actor_id: "auditor-1".to_string(),
                role: AccessRole::Auditor,
                requested_at_unix: 502,
                scope: ExportScope::FullContent,
                categories: vec![DataCategory::InboundMessage],
                start_unix: None,
                end_unix: None,
            },
        )
        .expect("auditor full export should pass");
        assert!(
            full_content.records[0]
                .content
                .as_deref()
                .is_some_and(|content| content.contains("<redacted-email>"))
        );
    }

    #[test]
    fn debug_trace_redaction_masks_sensitive_tokens() {
        let redacted = redact_debug_trace(
            "email user@example.com token=abc secret=xyz mnemonic words sk-12345",
        );
        assert!(redacted.contains("<redacted-email>"));
        assert!(redacted.contains("<redacted>"));
        assert!(!redacted.contains("user@example.com"));
    }
}
