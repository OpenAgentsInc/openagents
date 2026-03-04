use crate::{
    ApprovalDecisionAction, ApprovalDecisionInput, ApprovalWorkflowState, DraftGenerationInput,
    DraftPolicy, FollowUpRule, FollowUpRuleKind, FollowUpSchedulerPolicy, FollowUpSchedulerState,
    GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailConnectorError,
    GmailDeltaItem, GmailDeltaOperation, GmailHistoryProvider, GmailMailboxProvider, GmailMessage,
    GmailMessageBody, GmailMessageHeader, GmailMessageMetadata, GmailMessagePayload,
    GmailSendProvider, GmailSendSuccess, GmailSyncBatch, GmailSyncCursor, GmailSyncError,
    GmailSyncState, GmailThreadParticipant, GroundingReference, KnowledgeBase,
    KnowledgeChunkingConfig, KnowledgeDocument, NormalizationConfig, RetrievalIndex, RetrievalQuery,
    SendDeliveryState, SendExecutionPolicy, SendFailureClass, SendProviderError, SendRequest,
    apply_gmail_incremental_sync, derive_style_profile, enqueue_draft_for_approval,
    execute_send_with_idempotency, generate_draft, normalize_gmail_message,
    record_approval_decision, run_follow_up_scheduler_tick, run_gmail_backfill,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum E2eFailureInjection {
    None,
    TokenExpired,
    GmailRateLimitDuringSync,
    SendPermanentFailure,
    SyncCursorStale,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct E2eHarnessConfig {
    pub now_unix: u64,
    pub access_token_expires_at_unix: u64,
    pub failure_injection: E2eFailureInjection,
}

impl Default for E2eHarnessConfig {
    fn default() -> Self {
        Self {
            now_unix: 1_750_000_000,
            access_token_expires_at_unix: 1_750_000_000 + 3_600,
            failure_injection: E2eFailureInjection::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum E2eHarnessStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct E2eHarnessOutcome {
    pub status: E2eHarnessStatus,
    pub completed_stages: Vec<String>,
    pub failure_stage: Option<String>,
    pub failure_reason: Option<String>,
    pub send_state: Option<SendDeliveryState>,
    pub follow_up_event_count: usize,
}

pub fn run_email_pipeline_e2e(config: &E2eHarnessConfig) -> E2eHarnessOutcome {
    let mut completed = Vec::<String>::new();

    if config.access_token_expires_at_unix <= config.now_unix
        || config.failure_injection == E2eFailureInjection::TokenExpired
    {
        return fail(
            completed,
            "connect_mailbox",
            "gmail oauth access token expired".to_string(),
            None,
        );
    }
    completed.push("connect_mailbox".to_string());

    let mailbox_provider = HarnessMailboxProvider::new();
    let backfill = match run_gmail_backfill(
        &mailbox_provider,
        Some(&GmailBackfillCheckpoint {
            next_page_token: None,
            imported_count: 0,
        }),
        &GmailBackfillConfig {
            page_size: 5,
            max_pages: 1,
        },
    ) {
        Ok(result) => result,
        Err(error) => {
            return fail(completed, "backfill", error.to_string(), None);
        }
    };
    completed.push("backfill".to_string());

    let mut sync_state = if config.failure_injection == E2eFailureInjection::SyncCursorStale {
        GmailSyncState {
            cursor: Some(GmailSyncCursor { history_id: 10 }),
            ..GmailSyncState::default()
        }
    } else {
        GmailSyncState::default()
    };
    let history_provider = HarnessHistoryProvider::new(config.failure_injection);
    let sync_outcome = match apply_gmail_incremental_sync(&mut sync_state, &history_provider, 50) {
        Ok(outcome) => outcome,
        Err(error) => {
            return fail(completed, "sync", error.to_string(), None);
        }
    };
    if sync_outcome.rebootstrap_required {
        return fail(
            completed,
            "sync",
            sync_outcome
                .reason
                .unwrap_or_else(|| "rebootstrap required".to_string()),
            None,
        );
    }
    completed.push("sync".to_string());

    let inbound_message = backfill
        .imported_messages
        .first()
        .cloned()
        .unwrap_or_else(sample_gmail_message);
    let normalized = normalize_gmail_message(&inbound_message, &NormalizationConfig::default());
    completed.push("normalize".to_string());

    let mut retrieval_index = RetrievalIndex::default();
    retrieval_index.upsert(normalized.clone());
    let retrieval = retrieval_index.search(&RetrievalQuery {
        text: "invoice reminder".to_string(),
        thread_id: Some(normalized.thread_id.clone()),
        participant_email: None,
        label: Some("INBOX".to_string()),
        limit: 3,
    });
    completed.push("retrieve".to_string());

    let style_profile = derive_style_profile("profile-e2e", std::slice::from_ref(&normalized));
    let mut knowledge_base = KnowledgeBase::default();
    knowledge_base.ingest_document(
        &KnowledgeDocument {
            document_id: "kb-ops".to_string(),
            title: "Ops policy".to_string(),
            source_uri: "kb://ops".to_string(),
            body: "invoice policy reminder cadence and next steps".to_string(),
            tags: vec!["ops".to_string()],
        },
        &KnowledgeChunkingConfig::default(),
    );
    let grounding = knowledge_base.search("invoice policy", &["ops".to_string()], 2);
    let draft = match generate_draft(&DraftGenerationInput {
        inbound_message: normalized.clone(),
        style_profile,
        retrieval_context: retrieval,
        grounding_references: grounding
            .iter()
            .map(|reference| GroundingReference {
                chunk_id: reference.chunk_id.clone(),
                document_id: reference.document_id.clone(),
                source_uri: reference.source_uri.clone(),
                snippet: reference.snippet.clone(),
                score_milli: reference.score_milli,
            })
            .collect(),
        policy: DraftPolicy {
            max_output_chars: 1200,
            minimum_grounding_refs: 1,
        },
    }) {
        Ok(draft) => draft,
        Err(error) => return fail(completed, "draft", error.to_string(), None),
    };
    completed.push("draft".to_string());

    let mut approvals = ApprovalWorkflowState::default();
    if let Err(error) = enqueue_draft_for_approval(
        &mut approvals,
        crate::DraftEnqueueRequest {
            draft_id: draft.draft_id.clone(),
            queued_at_unix: config.now_unix,
            auto_policy_id: None,
        },
    ) {
        return fail(completed, "approve", error.to_string(), None);
    }
    if let Err(error) = record_approval_decision(
        &mut approvals,
        ApprovalDecisionInput {
            draft_id: draft.draft_id.clone(),
            action: ApprovalDecisionAction::Approve,
            actor: "operator-e2e".to_string(),
            decided_at_unix: config.now_unix + 1,
            reason: Some("approved in e2e".to_string()),
        },
    ) {
        return fail(completed, "approve", error.to_string(), None);
    }
    if let Err(error) = crate::authorize_draft_send(&approvals, draft.draft_id.as_str(), config.now_unix + 2) {
        return fail(completed, "approve", error.to_string(), None);
    }
    completed.push("approve".to_string());

    let send_provider = HarnessSendProvider::new(config.failure_injection);
    let send_outcome = match execute_send_with_idempotency(
        &mut crate::SendExecutionState::default(),
        &send_provider,
        &SendRequest {
            draft_id: draft.draft_id.clone(),
            idempotency_key: format!("send:{}", draft.draft_id),
            recipient_email: normalized.sender_email.clone(),
            subject: normalized.subject.clone(),
            body: draft.body.clone(),
        },
        &SendExecutionPolicy::default(),
        config.now_unix + 3,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return fail(completed, "send", error.to_string(), None),
    };
    if send_outcome.state != SendDeliveryState::Sent {
        return fail(
            completed,
            "send",
            format!("send state was {:?}", send_outcome.state),
            Some(send_outcome.state),
        );
    }
    completed.push("send".to_string());

    let mut follow_up_state = FollowUpSchedulerState::default();
    let follow_up = match run_follow_up_scheduler_tick(
        &mut follow_up_state,
        &[crate::ThreadFollowUpContext {
            thread_id: normalized.thread_id.clone(),
            recipient_email: normalized.sender_email.clone(),
            last_inbound_unix: normalized.timestamp_ms / 1000,
            last_outbound_unix: config.now_unix.saturating_sub(10_000),
            awaiting_reply: true,
            is_critical: true,
            reminder_count: 0,
        }],
        &FollowUpSchedulerPolicy {
            rules: vec![FollowUpRule {
                rule_id: "rule.critical_2h".to_string(),
                enabled: true,
                kind: FollowUpRuleKind::UnansweredCriticalThread {
                    hours_without_reply: 2,
                },
            }],
            business_hours_start_utc: 0,
            business_hours_end_utc: 23,
            quiet_hours_start_utc: 23,
            quiet_hours_end_utc: 0,
            per_recipient_daily_limit: 3,
        },
        config.now_unix + 4,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return fail(completed, "follow_up", error.to_string(), Some(send_outcome.state)),
    };
    completed.push("follow_up".to_string());

    E2eHarnessOutcome {
        status: E2eHarnessStatus::Success,
        completed_stages: completed,
        failure_stage: None,
        failure_reason: None,
        send_state: Some(send_outcome.state),
        follow_up_event_count: follow_up.emitted_events.len(),
    }
}

fn fail(
    completed_stages: Vec<String>,
    stage: &str,
    reason: String,
    send_state: Option<SendDeliveryState>,
) -> E2eHarnessOutcome {
    E2eHarnessOutcome {
        status: E2eHarnessStatus::Failed,
        completed_stages,
        failure_stage: Some(stage.to_string()),
        failure_reason: Some(reason),
        send_state,
        follow_up_event_count: 0,
    }
}

struct HarnessMailboxProvider;

impl HarnessMailboxProvider {
    fn new() -> Self {
        Self
    }
}

impl GmailMailboxProvider for HarnessMailboxProvider {
    fn list_messages(
        &self,
        _page_token: Option<&str>,
        _page_size: usize,
    ) -> Result<GmailBackfillPage, GmailConnectorError> {
        Ok(GmailBackfillPage {
            message_ids: vec!["msg-e2e-1".to_string()],
            next_page_token: None,
        })
    }

    fn get_message(&self, _message_id: &str) -> Result<GmailMessage, GmailConnectorError> {
        Ok(sample_gmail_message())
    }
}

struct HarnessHistoryProvider {
    failure_injection: E2eFailureInjection,
}

impl HarnessHistoryProvider {
    fn new(failure_injection: E2eFailureInjection) -> Self {
        Self { failure_injection }
    }
}

impl GmailHistoryProvider for HarnessHistoryProvider {
    fn fetch_history_since(
        &self,
        _since_history_id: Option<u64>,
        _max_results: usize,
    ) -> Result<GmailSyncBatch, GmailSyncError> {
        if self.failure_injection == E2eFailureInjection::GmailRateLimitDuringSync {
            return Err(GmailSyncError::Provider("rate_limit".to_string()));
        }
        if self.failure_injection == E2eFailureInjection::SyncCursorStale {
            return Ok(GmailSyncBatch {
                next_history_id: 9,
                deltas: Vec::new(),
            });
        }

        Ok(GmailSyncBatch {
            next_history_id: 11,
            deltas: vec![GmailDeltaItem {
                message_id: "msg-e2e-new".to_string(),
                operation: GmailDeltaOperation::Create,
                history_id: 11,
            }],
        })
    }
}

struct HarnessSendProvider {
    failure_injection: E2eFailureInjection,
}

impl HarnessSendProvider {
    fn new(failure_injection: E2eFailureInjection) -> Self {
        Self { failure_injection }
    }
}

impl GmailSendProvider for HarnessSendProvider {
    fn send_message(&self, _request: &SendRequest) -> Result<GmailSendSuccess, SendProviderError> {
        if self.failure_injection == E2eFailureInjection::SendPermanentFailure {
            return Err(SendProviderError {
                class: SendFailureClass::Permanent,
                reason: "permanent_send_failure".to_string(),
            });
        }
        Ok(GmailSendSuccess {
            provider_message_id: "gmail:e2e-provider-id".to_string(),
        })
    }
}

fn sample_gmail_message() -> GmailMessage {
    GmailMessage {
        id: "msg-e2e-1".to_string(),
        thread_id: "thread-e2e-1".to_string(),
        payload: GmailMessagePayload {
            headers: vec![
                GmailMessageHeader {
                    name: "Subject".to_string(),
                    value: "Invoice reminder".to_string(),
                },
                GmailMessageHeader {
                    name: "From".to_string(),
                    value: "sender@example.com".to_string(),
                },
                GmailMessageHeader {
                    name: "To".to_string(),
                    value: "ops@example.com".to_string(),
                },
            ],
            body: GmailMessageBody {
                mime_type: "text/plain".to_string(),
                data: "Hello team, please send invoice status update.".to_string(),
            },
        },
        participants: vec![GmailThreadParticipant {
            email: "sender@example.com".to_string(),
            display_name: Some("Sender".to_string()),
        }],
        metadata: GmailMessageMetadata {
            internal_date_ms: 1_750_000_000_000,
            label_ids: vec!["INBOX".to_string()],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{E2eFailureInjection, E2eHarnessConfig, E2eHarnessStatus, run_email_pipeline_e2e};
    use crate::SendDeliveryState;

    #[test]
    fn e2e_harness_completes_full_pipeline() {
        let outcome = run_email_pipeline_e2e(&E2eHarnessConfig::default());
        assert_eq!(outcome.status, E2eHarnessStatus::Success);
        assert_eq!(
            outcome.completed_stages,
            vec![
                "connect_mailbox".to_string(),
                "backfill".to_string(),
                "sync".to_string(),
                "normalize".to_string(),
                "retrieve".to_string(),
                "draft".to_string(),
                "approve".to_string(),
                "send".to_string(),
                "follow_up".to_string(),
            ]
        );
        assert_eq!(outcome.send_state, Some(SendDeliveryState::Sent));
        assert!(outcome.follow_up_event_count > 0);
    }

    #[test]
    fn e2e_harness_fails_when_token_expired() {
        let outcome = run_email_pipeline_e2e(&E2eHarnessConfig {
            access_token_expires_at_unix: 10,
            now_unix: 100,
            failure_injection: E2eFailureInjection::None,
        });
        assert_eq!(outcome.status, E2eHarnessStatus::Failed);
        assert_eq!(outcome.failure_stage.as_deref(), Some("connect_mailbox"));
    }

    #[test]
    fn e2e_harness_fails_on_sync_rate_limit() {
        let outcome = run_email_pipeline_e2e(&E2eHarnessConfig {
            failure_injection: E2eFailureInjection::GmailRateLimitDuringSync,
            ..E2eHarnessConfig::default()
        });
        assert_eq!(outcome.status, E2eHarnessStatus::Failed);
        assert_eq!(outcome.failure_stage.as_deref(), Some("sync"));
        assert!(
            outcome
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("rate_limit"))
        );
    }

    #[test]
    fn e2e_harness_fails_on_permanent_send_failure() {
        let outcome = run_email_pipeline_e2e(&E2eHarnessConfig {
            failure_injection: E2eFailureInjection::SendPermanentFailure,
            ..E2eHarnessConfig::default()
        });
        assert_eq!(outcome.status, E2eHarnessStatus::Failed);
        assert_eq!(outcome.failure_stage.as_deref(), Some("send"));
        assert_eq!(outcome.send_state, Some(SendDeliveryState::FailedPermanent));
    }

    #[test]
    fn e2e_harness_fails_on_stale_sync_cursor() {
        let outcome = run_email_pipeline_e2e(&E2eHarnessConfig {
            failure_injection: E2eFailureInjection::SyncCursorStale,
            ..E2eHarnessConfig::default()
        });
        assert_eq!(outcome.status, E2eHarnessStatus::Failed);
        assert_eq!(outcome.failure_stage.as_deref(), Some("sync"));
        assert!(
            outcome
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("moved backwards"))
        );
    }
}
