//! Email lane runtime state and deterministic pane row models.

use std::collections::BTreeMap;

use openagents_email_agent::{
    ApprovalWorkflowState, DraftApprovalStatus, DraftArtifact, FollowUpJobStatus, FollowUpRule,
    FollowUpRuleKind, FollowUpSchedulerPolicy, FollowUpSchedulerState, GmailBackfillResult,
    GmailSyncOutcome, GmailSyncState, KnowledgeBase, KnowledgeChunkingConfig, KnowledgeDocument,
    NormalizationConfig, NormalizedConversationItem, RetrievalIndex, SendDeliveryState,
    SendExecutionPolicy, SendExecutionState, SendRecord, normalize_gmail_message,
};

use crate::app_state::{PaneLoadState, PaneStatusAccess};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailInboxRow {
    pub message_id: String,
    pub thread_id: String,
    pub sender_email: String,
    pub subject: String,
    pub received_at_unix: u64,
    pub labels: Vec<String>,
    pub pipeline_state: String,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailDraftRow {
    pub draft_id: String,
    pub source_message_id: String,
    pub recipient_email: String,
    pub subject: String,
    pub confidence_milli: u32,
    pub approval_status: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailApprovalRow {
    pub draft_id: String,
    pub queued_at_unix: u64,
    pub status: String,
    pub last_decision_id: Option<String>,
    pub decision_actor: Option<String>,
    pub decision_reason: Option<String>,
    pub policy_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailSendRow {
    pub send_id: String,
    pub draft_id: String,
    pub idempotency_key: String,
    pub state: String,
    pub attempt_count: u32,
    pub provider_message_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailFollowUpRow {
    pub job_id: String,
    pub thread_id: String,
    pub recipient_email: String,
    pub rule_id: String,
    pub scheduled_for_unix: u64,
    pub status: String,
    pub reason: Option<String>,
}

pub struct EmailLaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_result: Option<String>,
    pub last_sync_error: Option<String>,
    pub last_send_error: Option<String>,
    pub sync_rebootstrap_required: bool,
    pub sync_rebootstrap_reason: Option<String>,
    pub last_sync_history_id: Option<u64>,
    pub backfill_checkpoint: Option<openagents_email_agent::GmailBackfillCheckpoint>,
    pub gmail_sync: GmailSyncState,
    pub retrieval_index: RetrievalIndex,
    pub approval_workflow: ApprovalWorkflowState,
    pub send_execution: SendExecutionState,
    pub follow_up_scheduler: FollowUpSchedulerState,
    pub follow_up_policy: FollowUpSchedulerPolicy,
    pub send_policy: SendExecutionPolicy,
    pub knowledge_base: KnowledgeBase,
    pub normalized_by_message_id: BTreeMap<String, NormalizedConversationItem>,
    pub drafts_by_id: BTreeMap<String, DraftArtifact>,
    pub draft_subject_by_id: BTreeMap<String, String>,
    pub draft_recipient_by_id: BTreeMap<String, String>,
    pub inbox_rows: Vec<EmailInboxRow>,
    pub draft_rows: Vec<EmailDraftRow>,
    pub approval_rows: Vec<EmailApprovalRow>,
    pub send_rows: Vec<EmailSendRow>,
    pub follow_up_rows: Vec<EmailFollowUpRow>,
    pub selected_inbox_message_id: Option<String>,
    pub selected_draft_id: Option<String>,
    pub selected_approval_draft_id: Option<String>,
    pub selected_send_idempotency_key: Option<String>,
    pub selected_follow_up_job_id: Option<String>,
}

impl Default for EmailLaneState {
    fn default() -> Self {
        let mut knowledge_base = KnowledgeBase::default();
        for document in seed_knowledge_documents() {
            let _ = knowledge_base.ingest_document(
                &document,
                &KnowledgeChunkingConfig {
                    max_words_per_chunk: 96,
                },
            );
        }

        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for email lane action".to_string()),
            last_result: None,
            last_sync_error: None,
            last_send_error: None,
            sync_rebootstrap_required: false,
            sync_rebootstrap_reason: None,
            last_sync_history_id: None,
            backfill_checkpoint: None,
            gmail_sync: GmailSyncState::default(),
            retrieval_index: RetrievalIndex::default(),
            approval_workflow: ApprovalWorkflowState::default(),
            send_execution: SendExecutionState::default(),
            follow_up_scheduler: FollowUpSchedulerState::default(),
            follow_up_policy: FollowUpSchedulerPolicy {
                rules: vec![
                    FollowUpRule {
                        rule_id: "no-reply-48h".to_string(),
                        enabled: true,
                        kind: FollowUpRuleKind::NoReplyAfterDays {
                            days_without_reply: 2,
                        },
                    },
                    FollowUpRule {
                        rule_id: "critical-12h".to_string(),
                        enabled: true,
                        kind: FollowUpRuleKind::UnansweredCriticalThread {
                            hours_without_reply: 12,
                        },
                    },
                ],
                ..FollowUpSchedulerPolicy::default()
            },
            send_policy: SendExecutionPolicy::default(),
            knowledge_base,
            normalized_by_message_id: BTreeMap::new(),
            drafts_by_id: BTreeMap::new(),
            draft_subject_by_id: BTreeMap::new(),
            draft_recipient_by_id: BTreeMap::new(),
            inbox_rows: Vec::new(),
            draft_rows: Vec::new(),
            approval_rows: Vec::new(),
            send_rows: Vec::new(),
            follow_up_rows: Vec::new(),
            selected_inbox_message_id: None,
            selected_draft_id: None,
            selected_approval_draft_id: None,
            selected_send_idempotency_key: None,
            selected_follow_up_job_id: None,
        }
    }
}

impl EmailLaneState {
    pub fn ingest_backfill_result(&mut self, result: GmailBackfillResult) {
        self.backfill_checkpoint = Some(result.final_checkpoint);
        for message in result.imported_messages {
            let normalized = normalize_gmail_message(&message, &NormalizationConfig::default());
            self.gmail_sync
                .known_message_ids
                .insert(normalized.source_message_id.clone());
            self.retrieval_index.upsert(normalized.clone());
            self.normalized_by_message_id
                .insert(normalized.source_message_id.clone(), normalized);
        }
        self.pane_set_ready(format!(
            "Imported {} inbox messages from Gmail",
            self.normalized_by_message_id.len()
        ));
        self.rebuild_rows();
    }

    pub fn upsert_normalized_item(&mut self, item: NormalizedConversationItem) {
        self.gmail_sync
            .known_message_ids
            .insert(item.source_message_id.clone());
        self.retrieval_index.upsert(item.clone());
        self.normalized_by_message_id
            .insert(item.source_message_id.clone(), item);
    }

    pub fn remove_normalized_item(&mut self, message_id: &str) {
        self.gmail_sync.known_message_ids.remove(message_id);
        self.normalized_by_message_id.remove(message_id);
    }

    pub fn apply_sync_outcome(&mut self, outcome: &GmailSyncOutcome) {
        self.last_sync_history_id = outcome.next_cursor.as_ref().map(|cursor| cursor.history_id);
        self.sync_rebootstrap_required = outcome.rebootstrap_required;
        self.sync_rebootstrap_reason = outcome.reason.clone();
        self.last_sync_error = if outcome.rebootstrap_required {
            outcome.reason.clone()
        } else {
            None
        };
        self.last_result = Some(format!(
            "sync applied {} deltas (duplicates dropped: {})",
            outcome.applied_deltas.len(),
            outcome.duplicate_drop_count
        ));
    }

    pub fn select_inbox_row(&mut self, index: usize) -> bool {
        let Some(message_id) = self.inbox_rows.get(index).map(|row| row.message_id.clone()) else {
            return false;
        };
        self.selected_inbox_message_id = Some(message_id);
        true
    }

    pub fn select_draft_row(&mut self, index: usize) -> bool {
        let Some(draft_id) = self.draft_rows.get(index).map(|row| row.draft_id.clone()) else {
            return false;
        };
        self.selected_draft_id = Some(draft_id);
        true
    }

    pub fn select_approval_row(&mut self, index: usize) -> bool {
        let Some(draft_id) = self
            .approval_rows
            .get(index)
            .map(|row| row.draft_id.clone())
        else {
            return false;
        };
        self.selected_approval_draft_id = Some(draft_id);
        true
    }

    pub fn select_send_row(&mut self, index: usize) -> bool {
        let Some(idempotency_key) = self
            .send_rows
            .get(index)
            .map(|row| row.idempotency_key.clone())
        else {
            return false;
        };
        self.selected_send_idempotency_key = Some(idempotency_key);
        true
    }

    pub fn select_follow_up_row(&mut self, index: usize) -> bool {
        let Some(job_id) = self.follow_up_rows.get(index).map(|row| row.job_id.clone()) else {
            return false;
        };
        self.selected_follow_up_job_id = Some(job_id);
        true
    }

    pub fn selected_inbox_item(&self) -> Option<&NormalizedConversationItem> {
        let message_id = self.selected_inbox_message_id.as_deref()?;
        self.normalized_by_message_id.get(message_id)
    }

    pub fn selected_draft(&self) -> Option<&DraftArtifact> {
        let draft_id = self.selected_draft_id.as_deref()?;
        self.drafts_by_id.get(draft_id)
    }

    pub fn selected_approval_draft_id(&self) -> Option<&str> {
        self.selected_approval_draft_id
            .as_deref()
            .or(self.selected_draft_id.as_deref())
    }

    pub fn selected_send_record(&self) -> Option<&SendRecord> {
        let key = self.selected_send_idempotency_key.as_deref()?;
        self.send_execution.records_by_idempotency_key.get(key)
    }

    pub fn rebuild_rows(&mut self) {
        self.rebuild_inbox_rows();
        self.rebuild_draft_rows();
        self.rebuild_approval_rows();
        self.rebuild_send_rows();
        self.rebuild_follow_up_rows();
    }

    fn rebuild_inbox_rows(&mut self) {
        let mut rows = self
            .normalized_by_message_id
            .values()
            .map(|item| {
                let has_draft = self
                    .drafts_by_id
                    .values()
                    .any(|draft| draft.source_message_id == item.source_message_id);
                EmailInboxRow {
                    message_id: item.source_message_id.clone(),
                    thread_id: item.thread_id.clone(),
                    sender_email: item.sender_email.clone(),
                    subject: item.subject.clone(),
                    received_at_unix: item.timestamp_ms / 1000,
                    labels: item.labels.clone(),
                    pipeline_state: if has_draft {
                        "draft_ready".to_string()
                    } else {
                        "normalized".to_string()
                    },
                    summary: item.body_summary.clone(),
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            right
                .received_at_unix
                .cmp(&left.received_at_unix)
                .then_with(|| left.message_id.cmp(&right.message_id))
        });
        self.inbox_rows = rows;
        self.ensure_selected_inbox_exists();
    }

    fn rebuild_draft_rows(&mut self) {
        let mut rows = self
            .drafts_by_id
            .values()
            .map(|draft| {
                let approval_status = self
                    .approval_workflow
                    .drafts
                    .get(draft.draft_id.as_str())
                    .map(|item| approval_status_label(item.status).to_string())
                    .unwrap_or_else(|| "untracked".to_string());
                EmailDraftRow {
                    draft_id: draft.draft_id.clone(),
                    source_message_id: draft.source_message_id.clone(),
                    recipient_email: self
                        .draft_recipient_by_id
                        .get(draft.draft_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                    subject: self
                        .draft_subject_by_id
                        .get(draft.draft_id.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    confidence_milli: draft.confidence_milli,
                    approval_status,
                    rationale: draft.rationale.clone(),
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.draft_id.cmp(&right.draft_id));
        self.draft_rows = rows;
        self.ensure_selected_draft_exists();
    }

    fn rebuild_approval_rows(&mut self) {
        let mut rows = self
            .approval_workflow
            .drafts
            .values()
            .map(|item| {
                let decision = item.last_decision_id.as_deref().and_then(|decision_id| {
                    self.approval_workflow
                        .decision_log
                        .iter()
                        .find(|entry| entry.decision_id == decision_id)
                });
                let (decision_actor, decision_reason, policy_path) = match decision {
                    Some(decision) => (
                        Some(decision.actor.clone()),
                        decision.reason.clone(),
                        Some(match &decision.policy_path {
                            openagents_email_agent::ApprovalPolicyPath::Manual { actor } => {
                                format!("manual:{actor}")
                            }
                            openagents_email_agent::ApprovalPolicyPath::AutoPolicy {
                                policy_id,
                            } => format!("auto:{policy_id}"),
                        }),
                    ),
                    None => (None, None, None),
                };

                EmailApprovalRow {
                    draft_id: item.draft_id.clone(),
                    queued_at_unix: item.queued_at_unix,
                    status: approval_status_label(item.status).to_string(),
                    last_decision_id: item.last_decision_id.clone(),
                    decision_actor,
                    decision_reason,
                    policy_path,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.draft_id.cmp(&right.draft_id));
        self.approval_rows = rows;
        self.ensure_selected_approval_exists();
    }

    fn rebuild_send_rows(&mut self) {
        let mut rows = self
            .send_execution
            .records_by_idempotency_key
            .values()
            .map(send_row_from_record)
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.send_id.cmp(&right.send_id));
        self.send_rows = rows;
        self.ensure_selected_send_exists();
    }

    fn rebuild_follow_up_rows(&mut self) {
        let mut rows = self
            .follow_up_scheduler
            .jobs
            .values()
            .map(|job| EmailFollowUpRow {
                job_id: job.job_id.clone(),
                thread_id: job.thread_id.clone(),
                recipient_email: job.recipient_email.clone(),
                rule_id: job.rule_id.clone(),
                scheduled_for_unix: job.scheduled_for_unix,
                status: follow_up_status_label(job.status).to_string(),
                reason: job.reason.clone(),
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        self.follow_up_rows = rows;
        self.ensure_selected_follow_up_exists();
    }

    fn ensure_selected_inbox_exists(&mut self) {
        if self
            .selected_inbox_message_id
            .as_deref()
            .is_some_and(|selected| {
                self.inbox_rows
                    .iter()
                    .any(|row| row.message_id.as_str() == selected)
            })
        {
            return;
        }
        self.selected_inbox_message_id = self.inbox_rows.first().map(|row| row.message_id.clone());
    }

    fn ensure_selected_draft_exists(&mut self) {
        if self.selected_draft_id.as_deref().is_some_and(|selected| {
            self.draft_rows
                .iter()
                .any(|row| row.draft_id.as_str() == selected)
        }) {
            return;
        }
        self.selected_draft_id = self.draft_rows.first().map(|row| row.draft_id.clone());
    }

    fn ensure_selected_approval_exists(&mut self) {
        if self
            .selected_approval_draft_id
            .as_deref()
            .is_some_and(|selected| {
                self.approval_rows
                    .iter()
                    .any(|row| row.draft_id.as_str() == selected)
            })
        {
            return;
        }
        self.selected_approval_draft_id =
            self.approval_rows.first().map(|row| row.draft_id.clone());
    }

    fn ensure_selected_send_exists(&mut self) {
        if self
            .selected_send_idempotency_key
            .as_deref()
            .is_some_and(|selected| {
                self.send_rows
                    .iter()
                    .any(|row| row.idempotency_key.as_str() == selected)
            })
        {
            return;
        }
        self.selected_send_idempotency_key = self
            .send_rows
            .first()
            .map(|row| row.idempotency_key.clone());
    }

    fn ensure_selected_follow_up_exists(&mut self) {
        if self
            .selected_follow_up_job_id
            .as_deref()
            .is_some_and(|selected| {
                self.follow_up_rows
                    .iter()
                    .any(|row| row.job_id.as_str() == selected)
            })
        {
            return;
        }
        self.selected_follow_up_job_id = self.follow_up_rows.first().map(|row| row.job_id.clone());
    }
}

fn seed_knowledge_documents() -> Vec<KnowledgeDocument> {
    vec![
        KnowledgeDocument {
            document_id: "email-ops-response-policy".to_string(),
            title: "Response Policy".to_string(),
            source_uri: "kb://email/response-policy".to_string(),
            body: "Acknowledge inbound requests quickly, confirm ownership, provide one concrete next step, and include expected timing when information is missing.".to_string(),
            tags: vec!["email".to_string(), "ops".to_string()],
        },
        KnowledgeDocument {
            document_id: "email-ops-follow-up-policy".to_string(),
            title: "Follow Up Policy".to_string(),
            source_uri: "kb://email/follow-up-policy".to_string(),
            body: "If a thread has no reply after forty eight hours, send a concise reminder with one clear question and one clear action to unblock progress.".to_string(),
            tags: vec!["email".to_string(), "follow-up".to_string()],
        },
    ]
}

fn approval_status_label(status: DraftApprovalStatus) -> &'static str {
    match status {
        DraftApprovalStatus::Pending => "pending",
        DraftApprovalStatus::Approved => "approved",
        DraftApprovalStatus::Rejected => "rejected",
        DraftApprovalStatus::NeedsEdits => "needs_edits",
    }
}

fn send_row_from_record(record: &SendRecord) -> EmailSendRow {
    EmailSendRow {
        send_id: record.send_id.clone(),
        draft_id: record.draft_id.clone(),
        idempotency_key: record.idempotency_key.clone(),
        state: send_state_label(record.state).to_string(),
        attempt_count: record.attempt_count,
        provider_message_id: record.provider_message_id.clone(),
        last_error: record.last_error.clone(),
    }
}

fn send_state_label(state: SendDeliveryState) -> &'static str {
    match state {
        SendDeliveryState::Pending => "pending",
        SendDeliveryState::RetryScheduled => "retry_scheduled",
        SendDeliveryState::Sent => "sent",
        SendDeliveryState::FailedPermanent => "failed_permanent",
        SendDeliveryState::FailedTransientExhausted => "failed_transient_exhausted",
    }
}

fn follow_up_status_label(status: FollowUpJobStatus) -> &'static str {
    match status {
        FollowUpJobStatus::Scheduled => "scheduled",
        FollowUpJobStatus::Executed => "executed",
        FollowUpJobStatus::SkippedRecipientLimit => "skipped_recipient_limit",
    }
}

#[cfg(test)]
mod tests {
    use super::EmailLaneState;
    use openagents_email_agent::{
        GmailBackfillCheckpoint, GmailBackfillResult, GmailMessage, GmailMessageBody,
        GmailMessageHeader, GmailMessageMetadata, GmailMessagePayload, GmailThreadParticipant,
    };

    fn gmail_message(id: &str, internal_date_ms: u64) -> GmailMessage {
        GmailMessage {
            id: id.to_string(),
            thread_id: "thread-1".to_string(),
            payload: GmailMessagePayload {
                headers: vec![
                    GmailMessageHeader {
                        name: "From".to_string(),
                        value: "sender@example.com".to_string(),
                    },
                    GmailMessageHeader {
                        name: "Subject".to_string(),
                        value: format!("Subject {id}"),
                    },
                ],
                body: GmailMessageBody {
                    mime_type: "text/plain".to_string(),
                    data: "Body".to_string(),
                },
            },
            participants: vec![GmailThreadParticipant {
                email: "sender@example.com".to_string(),
                display_name: None,
            }],
            metadata: GmailMessageMetadata {
                internal_date_ms,
                label_ids: vec!["INBOX".to_string()],
            },
        }
    }

    #[test]
    fn backfill_ingest_populates_inbox_rows_in_descending_time_order() {
        let mut state = EmailLaneState::default();
        state.ingest_backfill_result(GmailBackfillResult {
            imported_messages: vec![gmail_message("m1", 1000), gmail_message("m2", 2000)],
            final_checkpoint: GmailBackfillCheckpoint {
                next_page_token: None,
                imported_count: 2,
            },
            page_checkpoints: vec![],
        });

        assert_eq!(state.inbox_rows.len(), 2);
        assert_eq!(state.inbox_rows[0].message_id, "m2");
        assert_eq!(state.inbox_rows[1].message_id, "m1");
        assert_eq!(state.selected_inbox_message_id.as_deref(), Some("m2"));
    }
}
