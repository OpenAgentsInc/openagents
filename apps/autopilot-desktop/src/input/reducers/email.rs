use std::collections::BTreeSet;
use std::time::Duration;

use base64::Engine;
use openagents_email_agent::{
    ApprovalDecisionAction, ApprovalDecisionInput, DraftEnqueueRequest, DraftGenerationInput,
    DraftPolicy, GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage,
    GmailBackfillResult, GmailConnectorError, GmailDeltaItem, GmailDeltaOperation,
    GmailHistoryProvider, GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader,
    GmailMessageMetadata, GmailMessagePayload, GmailSendProvider, GmailSendSuccess, GmailSyncBatch,
    GmailSyncError, GmailSyncOutcome, GmailSyncState, NormalizationConfig, RetrievalQuery,
    SendDeliveryState, SendExecutionOutcome, SendExecutionPolicy, SendExecutionState,
    SendFailureClass, SendProviderError, SendRequest, ThreadFollowUpContext,
    apply_gmail_incremental_sync, authorize_draft_send, derive_style_profile,
    enqueue_draft_for_approval, execute_send_with_idempotency, generate_draft,
    normalize_gmail_message, record_approval_decision, run_follow_up_scheduler_tick,
    run_gmail_backfill, set_approval_kill_switch, set_approval_queue_paused,
};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::app_state::{PaneLoadState, PaneStatusAccess, RenderState};
use crate::credentials::{
    GOOGLE_GMAIL_ACCESS_TOKEN, GOOGLE_GMAIL_TOKEN_EXPIRY_UNIX, GoogleGmailOAuthLifecycle,
};
use crate::pane_system::{
    EmailApprovalQueuePaneAction, EmailDraftQueuePaneAction, EmailFollowUpQueuePaneAction,
    EmailInboxPaneAction, EmailSendLogPaneAction,
};

const GMAIL_API_ROOT: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const GOOGLE_OAUTH_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GMAIL_QUERY_INBOX: &str = "in:inbox";
const GMAIL_SYNC_REBOOTSTRAP_REASON: &str = "gmail sync cursor stale; rebootstrap required";
const OAUTH_REFRESH_SKEW_SECONDS: u64 = 120;
const HTTP_TIMEOUT_SECONDS: u64 = 20;
const USER_AGENT: &str = "openagents-autopilot-desktop/0.1";

pub(super) fn fetch_live_gmail_backfill(
    state: &mut RenderState,
    checkpoint: Option<&GmailBackfillCheckpoint>,
    config: &GmailBackfillConfig,
) -> Result<GmailBackfillResult, String> {
    let provider = LiveGmailMailboxProvider::new(gmail_session(state)?);
    run_gmail_backfill(&provider, checkpoint, config).map_err(|error| error.to_string())
}

pub(super) fn run_live_gmail_incremental_sync(
    state: &mut RenderState,
    sync_state: &mut GmailSyncState,
    max_results: usize,
) -> Result<GmailSyncOutcome, String> {
    let provider = LiveGmailHistoryProvider::new(gmail_session(state)?);
    apply_gmail_incremental_sync(sync_state, &provider, max_results).map_err(|error| {
        if matches!(error, GmailSyncError::Provider(_)) {
            return format!("gmail incremental sync failed: {error}");
        }
        error.to_string()
    })
}

pub(super) fn execute_live_gmail_send(
    state: &mut RenderState,
    send_state: &mut SendExecutionState,
    request: &SendRequest,
    policy: &SendExecutionPolicy,
    now_unix: u64,
) -> Result<SendExecutionOutcome, String> {
    let provider = LiveGmailSendProvider::new(gmail_session(state)?);
    execute_send_with_idempotency(send_state, &provider, request, policy, now_unix)
        .map_err(|error| error.to_string())
}

pub(super) fn refresh_email_lane_from_live_gmail(state: &mut RenderState) -> Result<(), String> {
    state.email_lane.load_state = PaneLoadState::Loading;
    state.email_lane.last_error = None;
    state.email_lane.last_sync_error = None;
    state.email_lane.last_action = Some("Refreshing Gmail inbox snapshot".to_string());

    let checkpoint = state.email_lane.backfill_checkpoint.clone();
    let backfill = fetch_live_gmail_backfill(
        state,
        checkpoint.as_ref(),
        &GmailBackfillConfig {
            page_size: 25,
            max_pages: 2,
        },
    )?;
    state.email_lane.ingest_backfill_result(backfill);
    sync_email_lane_incremental(state, 200)?;
    state.email_lane.pane_set_ready(format!(
        "Email lane refreshed (inbox={} drafts={} approvals={} sends={} followups={})",
        state.email_lane.inbox_rows.len(),
        state.email_lane.draft_rows.len(),
        state.email_lane.approval_rows.len(),
        state.email_lane.send_rows.len(),
        state.email_lane.follow_up_rows.len()
    ));
    Ok(())
}

pub(super) fn sync_email_lane_incremental(
    state: &mut RenderState,
    max_results: usize,
) -> Result<GmailSyncOutcome, String> {
    let mut sync_state = state.email_lane.gmail_sync.clone();
    let outcome = run_live_gmail_incremental_sync(state, &mut sync_state, max_results)?;
    state.email_lane.gmail_sync = sync_state;
    state.email_lane.apply_sync_outcome(&outcome);

    if outcome.rebootstrap_required {
        let reason = outcome
            .reason
            .clone()
            .unwrap_or_else(|| stale_cursor_reason().to_string());
        state.email_lane.load_state = PaneLoadState::Error;
        state.email_lane.last_sync_error = Some(reason.clone());
        state.email_lane.last_error = Some(reason);
        state.email_lane.sync_rebootstrap_required = true;
        state.email_lane.rebuild_rows();
        return Ok(outcome);
    }

    for delta in &outcome.applied_deltas {
        match delta.operation {
            GmailDeltaOperation::Delete => {
                state
                    .email_lane
                    .remove_normalized_item(delta.message_id.as_str());
            }
            GmailDeltaOperation::Create | GmailDeltaOperation::Update => {
                let message = fetch_live_gmail_message(state, delta.message_id.as_str())?;
                let normalized = normalize_gmail_message(&message, &NormalizationConfig::default());
                state.email_lane.upsert_normalized_item(normalized);
            }
        }
    }
    state.email_lane.sync_rebootstrap_required = false;
    state.email_lane.sync_rebootstrap_reason = None;
    state.email_lane.rebuild_rows();
    Ok(outcome)
}

pub(super) fn generate_selected_inbox_draft(state: &mut RenderState) -> Result<String, String> {
    let inbound = state
        .email_lane
        .selected_inbox_item()
        .cloned()
        .ok_or_else(|| "Select an inbox row before generating a draft".to_string())?;

    let existing_draft = state
        .email_lane
        .drafts_by_id
        .values()
        .find(|draft| draft.source_message_id == inbound.source_message_id)
        .map(|draft| draft.draft_id.clone());
    if let Some(existing_draft) = existing_draft {
        state.email_lane.selected_draft_id = Some(existing_draft.clone());
        state
            .email_lane
            .pane_set_ready(format!("Draft already exists: {existing_draft}"));
        state.email_lane.rebuild_rows();
        return Ok(existing_draft);
    }

    let style_corpus = state
        .email_lane
        .normalized_by_message_id
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let style_profile = derive_style_profile("desktop-live-gmail", style_corpus.as_slice());
    let retrieval_context = state.email_lane.retrieval_index.search(&RetrievalQuery {
        text: inbound.body_summary.clone(),
        thread_id: Some(inbound.thread_id.clone()),
        participant_email: Some(inbound.sender_email.clone()),
        label: None,
        limit: 4,
    });

    let mut grounding_references = state.email_lane.knowledge_base.search(
        inbound.body_summary.as_str(),
        &["email".to_string()],
        3,
    );
    if grounding_references.is_empty() {
        grounding_references =
            state
                .email_lane
                .knowledge_base
                .search(inbound.body_summary.as_str(), &[], 3);
    }

    let draft = generate_draft(&DraftGenerationInput {
        inbound_message: inbound.clone(),
        style_profile,
        retrieval_context,
        grounding_references,
        policy: DraftPolicy::default(),
    })
    .map_err(|error| format!("draft generation failed: {error}"))?;
    let draft_id = draft.draft_id.clone();
    state.email_lane.draft_subject_by_id.insert(
        draft_id.clone(),
        if inbound.subject.trim().is_empty() {
            "Re: (no subject)".to_string()
        } else {
            format!("Re: {}", inbound.subject.trim())
        },
    );
    state
        .email_lane
        .draft_recipient_by_id
        .insert(draft_id.clone(), inbound.sender_email.clone());
    state
        .email_lane
        .drafts_by_id
        .insert(draft_id.clone(), draft);

    let now_unix = now_epoch_seconds();
    enqueue_draft_for_approval(
        &mut state.email_lane.approval_workflow,
        DraftEnqueueRequest {
            draft_id: draft_id.clone(),
            queued_at_unix: now_unix,
            auto_policy_id: None,
        },
    )
    .map_err(|error| format!("approval enqueue failed: {error}"))?;
    state.email_lane.selected_draft_id = Some(draft_id.clone());
    state.email_lane.selected_approval_draft_id = Some(draft_id.clone());
    state.email_lane.pane_set_ready(format!(
        "Generated draft {} for inbox message {}",
        draft_id, inbound.source_message_id
    ));
    state.email_lane.rebuild_rows();
    Ok(draft_id)
}

pub(super) fn approve_or_reject_selected_draft(
    state: &mut RenderState,
    action: ApprovalDecisionAction,
    reason: Option<String>,
) -> Result<String, String> {
    let draft_id = state
        .email_lane
        .selected_approval_draft_id()
        .ok_or_else(|| "Select an approval queue row first".to_string())?
        .to_string();
    let decision = record_approval_decision(
        &mut state.email_lane.approval_workflow,
        ApprovalDecisionInput {
            draft_id: draft_id.clone(),
            action,
            actor: "operator".to_string(),
            decided_at_unix: now_epoch_seconds(),
            reason,
        },
    )
    .map_err(|error| format!("approval decision failed: {error}"))?;
    state.email_lane.pane_set_ready(format!(
        "Recorded approval decision {} for {}",
        decision.decision_id, draft_id
    ));
    state.email_lane.rebuild_rows();
    Ok(draft_id)
}

pub(super) fn send_selected_approved_draft(state: &mut RenderState) -> Result<String, String> {
    let draft_id = state
        .email_lane
        .selected_approval_draft_id()
        .or(state.email_lane.selected_draft_id.as_deref())
        .ok_or_else(|| "Select a draft row before sending".to_string())?
        .to_string();

    let authorization = authorize_draft_send(
        &state.email_lane.approval_workflow,
        draft_id.as_str(),
        now_epoch_seconds(),
    )
    .map_err(|error| format!("draft send authorization failed: {error}"))?;
    let draft = state
        .email_lane
        .drafts_by_id
        .get(draft_id.as_str())
        .cloned()
        .ok_or_else(|| format!("draft {draft_id} not found in runtime cache"))?;
    let recipient = state
        .email_lane
        .draft_recipient_by_id
        .get(draft_id.as_str())
        .cloned()
        .ok_or_else(|| format!("recipient metadata missing for draft {draft_id}"))?;
    let subject = state
        .email_lane
        .draft_subject_by_id
        .get(draft_id.as_str())
        .cloned()
        .unwrap_or_else(|| "Re: update".to_string());

    let request = SendRequest {
        draft_id: draft_id.clone(),
        idempotency_key: format!("{}:{}", draft_id, authorization.decision_id),
        recipient_email: recipient,
        subject,
        body: draft.body,
    };
    let send_policy = state.email_lane.send_policy.clone();
    let mut send_state = state.email_lane.send_execution.clone();
    let outcome = execute_live_gmail_send(
        state,
        &mut send_state,
        &request,
        &send_policy,
        now_epoch_seconds(),
    )?;
    state.email_lane.send_execution = send_state;
    state.email_lane.last_send_error =
        if outcome.state.is_final() && outcome.provider_message_id.is_none() {
            state
                .email_lane
                .send_execution
                .records_by_idempotency_key
                .get(request.idempotency_key.as_str())
                .and_then(|record| record.last_error.clone())
        } else {
            None
        };
    state.email_lane.last_result = Some(format!(
        "send {} -> {} (attempts={})",
        outcome.send_id,
        send_delivery_state_label(outcome.state),
        outcome.attempt_count
    ));
    state.email_lane.selected_send_idempotency_key = Some(outcome.idempotency_key.clone());
    state.email_lane.pane_set_ready(format!(
        "Send outcome for {}: {:?}",
        draft_id, outcome.state
    ));
    state.email_lane.rebuild_rows();
    Ok(outcome.send_id)
}

pub(super) fn run_follow_up_scheduler(state: &mut RenderState) -> Result<usize, String> {
    let contexts = build_follow_up_contexts(state);
    let mut scheduler_state = state.email_lane.follow_up_scheduler.clone();
    let outcome = run_follow_up_scheduler_tick(
        &mut scheduler_state,
        contexts.as_slice(),
        &state.email_lane.follow_up_policy,
        now_epoch_seconds(),
    )
    .map_err(|error| format!("follow-up scheduler failed: {error}"))?;
    state.email_lane.follow_up_scheduler = scheduler_state;
    state.email_lane.last_result = Some(format!(
        "follow-up tick emitted {} events",
        outcome.emitted_events.len()
    ));
    state.email_lane.pane_set_ready(format!(
        "Follow-up tick complete (scheduled={}, executed={})",
        outcome.upcoming_jobs.len(),
        outcome.executed_jobs.len()
    ));
    state.email_lane.rebuild_rows();
    Ok(outcome.emitted_events.len())
}

pub(super) fn run_email_inbox_action(
    state: &mut RenderState,
    action: EmailInboxPaneAction,
) -> bool {
    match action {
        EmailInboxPaneAction::Refresh => {
            if let Err(error) = refresh_email_lane_from_live_gmail(state) {
                state
                    .email_lane
                    .pane_set_error(format!("email inbox refresh failed: {error}"));
            }
            true
        }
        EmailInboxPaneAction::GenerateDraftSelected => {
            if let Err(error) = generate_selected_inbox_draft(state) {
                state
                    .email_lane
                    .pane_set_error(format!("draft generation failed: {error}"));
            }
            true
        }
        EmailInboxPaneAction::SelectRow(index) => {
            if state.email_lane.select_inbox_row(index) {
                state.email_lane.pane_set_ready(format!(
                    "Selected inbox message row {}",
                    index.saturating_add(1)
                ));
            } else {
                state.email_lane.pane_set_error(format!(
                    "Inbox row {} out of range",
                    index.saturating_add(1)
                ));
            }
            true
        }
    }
}

pub(super) fn run_email_draft_queue_action(
    state: &mut RenderState,
    action: EmailDraftQueuePaneAction,
) -> bool {
    match action {
        EmailDraftQueuePaneAction::SelectRow(index) => {
            if state.email_lane.select_draft_row(index) {
                if let Some(draft_id) = state.email_lane.selected_draft_id.clone() {
                    state.email_lane.selected_approval_draft_id = Some(draft_id.clone());
                    state
                        .email_lane
                        .pane_set_ready(format!("Selected draft {}", draft_id));
                } else {
                    state
                        .email_lane
                        .pane_set_ready(format!("Selected draft row {}", index.saturating_add(1)));
                }
            } else {
                state.email_lane.pane_set_error(format!(
                    "Draft row {} out of range",
                    index.saturating_add(1)
                ));
            }
            true
        }
    }
}

pub(super) fn run_email_approval_queue_action(
    state: &mut RenderState,
    action: EmailApprovalQueuePaneAction,
) -> bool {
    match action {
        EmailApprovalQueuePaneAction::ApproveSelected => {
            if let Err(error) =
                approve_or_reject_selected_draft(state, ApprovalDecisionAction::Approve, None)
            {
                state
                    .email_lane
                    .pane_set_error(format!("approve draft failed: {error}"));
            }
            true
        }
        EmailApprovalQueuePaneAction::RejectSelected => {
            if let Err(error) = approve_or_reject_selected_draft(
                state,
                ApprovalDecisionAction::Reject,
                Some("Rejected by operator".to_string()),
            ) {
                state
                    .email_lane
                    .pane_set_error(format!("reject draft failed: {error}"));
            }
            true
        }
        EmailApprovalQueuePaneAction::RequestEditsSelected => {
            if let Err(error) = approve_or_reject_selected_draft(
                state,
                ApprovalDecisionAction::RequestEdits,
                Some("Edits requested by operator".to_string()),
            ) {
                state
                    .email_lane
                    .pane_set_error(format!("request edits failed: {error}"));
            }
            true
        }
        EmailApprovalQueuePaneAction::TogglePauseQueue => {
            let pause_queue = !state.email_lane.approval_workflow.queue_paused;
            match set_approval_queue_paused(
                &mut state.email_lane.approval_workflow,
                pause_queue,
                "operator",
                now_epoch_seconds(),
                Some("desktop pane action"),
            ) {
                Ok(event) => {
                    state.email_lane.last_result = Some(format!(
                        "approval queue control: event={} action={:?}",
                        event.event_id, event.action
                    ));
                    state.email_lane.pane_set_ready(if pause_queue {
                        "Approval queue paused".to_string()
                    } else {
                        "Approval queue resumed".to_string()
                    });
                    state.email_lane.rebuild_rows();
                }
                Err(error) => {
                    state
                        .email_lane
                        .pane_set_error(format!("toggle approval queue pause failed: {error}"));
                }
            }
            true
        }
        EmailApprovalQueuePaneAction::ToggleKillSwitch => {
            let engage_kill_switch = !state.email_lane.approval_workflow.kill_switch_engaged;
            match set_approval_kill_switch(
                &mut state.email_lane.approval_workflow,
                engage_kill_switch,
                "operator",
                now_epoch_seconds(),
                Some("desktop pane action"),
            ) {
                Ok(event) => {
                    state.email_lane.last_result = Some(format!(
                        "approval kill switch control: event={} action={:?}",
                        event.event_id, event.action
                    ));
                    state.email_lane.pane_set_ready(if engage_kill_switch {
                        "Approval kill switch engaged".to_string()
                    } else {
                        "Approval kill switch disengaged".to_string()
                    });
                    state.email_lane.rebuild_rows();
                }
                Err(error) => {
                    state
                        .email_lane
                        .pane_set_error(format!("toggle approval kill switch failed: {error}"));
                }
            }
            true
        }
        EmailApprovalQueuePaneAction::SelectRow(index) => {
            if state.email_lane.select_approval_row(index) {
                state.email_lane.selected_draft_id = state
                    .email_lane
                    .selected_approval_draft_id()
                    .map(ToString::to_string);
                state
                    .email_lane
                    .pane_set_ready(format!("Selected approval row {}", index.saturating_add(1)));
            } else {
                state.email_lane.pane_set_error(format!(
                    "Approval row {} out of range",
                    index.saturating_add(1)
                ));
            }
            true
        }
    }
}

pub(super) fn run_email_send_log_action(
    state: &mut RenderState,
    action: EmailSendLogPaneAction,
) -> bool {
    match action {
        EmailSendLogPaneAction::SendSelected => {
            if let Err(error) = send_selected_approved_draft(state) {
                state
                    .email_lane
                    .pane_set_error(format!("send selected draft failed: {error}"));
            }
            true
        }
        EmailSendLogPaneAction::SelectRow(index) => {
            if state.email_lane.select_send_row(index) {
                state
                    .email_lane
                    .pane_set_ready(format!("Selected send row {}", index.saturating_add(1)));
            } else {
                state
                    .email_lane
                    .pane_set_error(format!("Send row {} out of range", index.saturating_add(1)));
            }
            true
        }
    }
}

pub(super) fn run_email_follow_up_queue_action(
    state: &mut RenderState,
    action: EmailFollowUpQueuePaneAction,
) -> bool {
    match action {
        EmailFollowUpQueuePaneAction::RunSchedulerTick => {
            if let Err(error) = run_follow_up_scheduler(state) {
                state
                    .email_lane
                    .pane_set_error(format!("follow-up scheduler tick failed: {error}"));
            }
            true
        }
        EmailFollowUpQueuePaneAction::SelectRow(index) => {
            if state.email_lane.select_follow_up_row(index) {
                state.email_lane.pane_set_ready(format!(
                    "Selected follow-up row {}",
                    index.saturating_add(1)
                ));
            } else {
                state.email_lane.pane_set_error(format!(
                    "Follow-up row {} out of range",
                    index.saturating_add(1)
                ));
            }
            true
        }
    }
}

fn fetch_live_gmail_message(
    state: &mut RenderState,
    message_id: &str,
) -> Result<GmailMessage, String> {
    let provider = LiveGmailMailboxProvider::new(gmail_session(state)?);
    provider
        .get_message(message_id)
        .map_err(|error| format!("gmail get message {} failed: {}", message_id, error))
}

fn build_follow_up_contexts(state: &RenderState) -> Vec<ThreadFollowUpContext> {
    let mut contexts = Vec::<ThreadFollowUpContext>::new();
    for draft in state.email_lane.drafts_by_id.values() {
        let Some(inbound) = state
            .email_lane
            .normalized_by_message_id
            .get(draft.source_message_id.as_str())
        else {
            continue;
        };

        let sent_record = state
            .email_lane
            .send_execution
            .records_by_idempotency_key
            .values()
            .filter(|record| record.draft_id == draft.draft_id)
            .filter(|record| record.state == SendDeliveryState::Sent)
            .max_by_key(|record| record.finalized_at_unix.unwrap_or(0));
        let Some(sent_record) = sent_record else {
            continue;
        };

        let recipient_email = state
            .email_lane
            .draft_recipient_by_id
            .get(draft.draft_id.as_str())
            .cloned()
            .unwrap_or_else(|| inbound.sender_email.clone());
        let reminder_count = state
            .email_lane
            .follow_up_scheduler
            .jobs
            .values()
            .filter(|job| job.thread_id == inbound.thread_id)
            .filter(|job| {
                job.recipient_email
                    .eq_ignore_ascii_case(recipient_email.as_str())
            })
            .count() as u32;
        contexts.push(ThreadFollowUpContext {
            thread_id: inbound.thread_id.clone(),
            recipient_email,
            last_inbound_unix: inbound.timestamp_ms / 1000,
            last_outbound_unix: sent_record
                .finalized_at_unix
                .unwrap_or(inbound.timestamp_ms / 1000),
            awaiting_reply: true,
            is_critical: inbound
                .labels
                .iter()
                .any(|label| label.eq_ignore_ascii_case("important")),
            reminder_count,
        });
    }
    contexts.sort_by(|left, right| {
        left.thread_id
            .cmp(&right.thread_id)
            .then_with(|| left.recipient_email.cmp(&right.recipient_email))
    });
    contexts
}

fn send_delivery_state_label(state: SendDeliveryState) -> &'static str {
    match state {
        SendDeliveryState::Pending => "pending",
        SendDeliveryState::RetryScheduled => "retry_scheduled",
        SendDeliveryState::Sent => "sent",
        SendDeliveryState::FailedPermanent => "failed_permanent",
        SendDeliveryState::FailedTransientExhausted => "failed_transient_exhausted",
    }
}

#[derive(Clone)]
struct GmailSession {
    client: Client,
    access_token: String,
}

impl GmailSession {
    fn auth_get_json<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        let url = format!("{GMAIL_API_ROOT}/{endpoint}");
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .query(query)
            .send()
            .map_err(|error| format!("gmail GET {endpoint} failed: {error}"))?;

        parse_json_response(response, format!("gmail GET {endpoint}"))
    }

    fn auth_post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, SendProviderError> {
        let url = format!("{GMAIL_API_ROOT}/{endpoint}");
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .map_err(|error| SendProviderError {
                class: SendFailureClass::Transient,
                reason: format!("gmail POST {endpoint} failed: {error}"),
            })?;
        parse_send_json_response(response, format!("gmail POST {endpoint}"))
    }
}

fn gmail_session(state: &mut RenderState) -> Result<GmailSession, String> {
    let access_token = gmail_access_token(state)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("gmail http client init failed: {error}"))?;
    Ok(GmailSession {
        client,
        access_token,
    })
}

fn gmail_access_token(state: &mut RenderState) -> Result<String, String> {
    let mut lifecycle = state
        .credentials
        .load_google_gmail_oauth_lifecycle()?
        .ok_or_else(|| "Missing Gmail OAuth credentials in Credentials pane".to_string())?;

    let now_unix = now_epoch_seconds();
    if lifecycle.should_refresh_at(now_unix, OAUTH_REFRESH_SKEW_SECONDS) {
        let refreshed = refresh_google_access_token(&lifecycle)?;
        lifecycle.access_token = refreshed.access_token.clone();
        lifecycle.expires_at_unix = refreshed.expires_at_unix;
        state
            .credentials
            .set_value_for_name(GOOGLE_GMAIL_ACCESS_TOKEN, lifecycle.access_token.as_str())?;
        state.credentials.set_value_for_name(
            GOOGLE_GMAIL_TOKEN_EXPIRY_UNIX,
            lifecycle.expires_at_unix.to_string().as_str(),
        )?;
    }

    Ok(lifecycle.access_token)
}

struct RefreshedGoogleToken {
    access_token: String,
    expires_at_unix: u64,
}

fn refresh_google_access_token(
    lifecycle: &GoogleGmailOAuthLifecycle,
) -> Result<RefreshedGoogleToken, String> {
    #[derive(Deserialize)]
    struct OAuthRefreshSuccess {
        access_token: String,
        expires_in: u64,
    }

    #[derive(Deserialize)]
    struct OAuthRefreshError {
        error: Option<String>,
        error_description: Option<String>,
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("oauth refresh client init failed: {error}"))?;

    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", lifecycle.refresh_token.as_str()),
        ("client_id", lifecycle.client_id.as_str()),
        ("client_secret", lifecycle.client_secret.as_str()),
        ("redirect_uri", lifecycle.redirect_uri.as_str()),
    ];
    let response = client
        .post(GOOGLE_OAUTH_TOKEN_ENDPOINT)
        .form(&form)
        .send()
        .map_err(|error| format!("oauth token refresh request failed: {error}"))?;

    if response.status().is_success() {
        let payload = response
            .json::<OAuthRefreshSuccess>()
            .map_err(|error| format!("oauth refresh response parse failed: {error}"))?;
        let now_unix = now_epoch_seconds();
        return Ok(RefreshedGoogleToken {
            access_token: payload.access_token,
            expires_at_unix: now_unix.saturating_add(payload.expires_in),
        });
    }

    let status = response.status();
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<OAuthRefreshError>(body.as_str())
        .ok()
        .map(|payload| {
            if let Some(description) = payload.error_description {
                return description;
            }
            payload
                .error
                .unwrap_or_else(|| "oauth refresh failed".to_string())
        })
        .unwrap_or_else(|| body.trim().to_string());
    Err(format!("oauth token refresh failed ({status}): {detail}"))
}

fn parse_json_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
    context: String,
) -> Result<T, String> {
    if response.status().is_success() {
        return response
            .json::<T>()
            .map_err(|error| format!("{context} response parse failed: {error}"));
    }
    let status = response.status();
    let text = response.text().unwrap_or_default();
    Err(format!(
        "{context} failed with status {}: {}",
        status.as_u16(),
        text.trim()
    ))
}

fn parse_send_json_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
    context: String,
) -> Result<T, SendProviderError> {
    if response.status().is_success() {
        return response.json::<T>().map_err(|error| SendProviderError {
            class: SendFailureClass::Transient,
            reason: format!("{context} response parse failed: {error}"),
        });
    }

    let status = response.status();
    let text = response.text().unwrap_or_default();
    let class = if status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::UNAUTHORIZED
        || status.is_server_error()
    {
        SendFailureClass::Transient
    } else {
        SendFailureClass::Permanent
    };
    Err(SendProviderError {
        class,
        reason: format!(
            "{context} failed with status {}: {}",
            status.as_u16(),
            text.trim()
        ),
    })
}

pub(super) struct LiveGmailMailboxProvider {
    session: GmailSession,
}

impl LiveGmailMailboxProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailMailboxProvider for LiveGmailMailboxProvider {
    fn list_messages(
        &self,
        page_token: Option<&str>,
        page_size: usize,
    ) -> Result<GmailBackfillPage, GmailConnectorError> {
        #[derive(Deserialize)]
        struct MessageRow {
            id: String,
        }

        #[derive(Deserialize)]
        struct ListResponse {
            #[serde(default)]
            messages: Vec<MessageRow>,
            #[serde(rename = "nextPageToken")]
            next_page_token: Option<String>,
        }

        let mut query = vec![
            ("maxResults", page_size.to_string()),
            ("q", GMAIL_QUERY_INBOX.to_string()),
        ];
        if let Some(page_token) = page_token {
            query.push(("pageToken", page_token.to_string()));
        }
        let response = self
            .session
            .auth_get_json::<ListResponse>("messages", query.as_slice())
            .map_err(GmailConnectorError::Provider)?;
        let message_ids = response
            .messages
            .into_iter()
            .map(|row| row.id)
            .collect::<Vec<_>>();
        Ok(GmailBackfillPage {
            message_ids,
            next_page_token: response.next_page_token,
        })
    }

    fn get_message(&self, message_id: &str) -> Result<GmailMessage, GmailConnectorError> {
        let endpoint = format!("messages/{message_id}");
        let response = self
            .session
            .auth_get_json::<GmailMessageResponse>(
                endpoint.as_str(),
                &[("format", "full".to_string())],
            )
            .map_err(GmailConnectorError::Provider)?;

        decode_gmail_message(response)
    }
}

#[derive(Debug, Deserialize)]
struct GmailMessageResponse {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(rename = "labelIds", default)]
    label_ids: Vec<String>,
    payload: Option<GmailPayloadNode>,
}

#[derive(Debug, Deserialize)]
struct GmailPayloadNode {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default)]
    headers: Vec<GmailHeaderValue>,
    body: Option<GmailPayloadBody>,
    #[serde(default)]
    parts: Vec<GmailPayloadNode>,
}

#[derive(Debug, Deserialize)]
struct GmailPayloadBody {
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailHeaderValue {
    name: String,
    value: String,
}

fn decode_gmail_message(
    response: GmailMessageResponse,
) -> Result<GmailMessage, GmailConnectorError> {
    let payload = response.payload.ok_or_else(|| {
        GmailConnectorError::Provider("gmail message payload missing".to_string())
    })?;
    let headers = flatten_headers(payload.headers.as_slice());
    let (mime_type, body) = decode_message_body(&payload);
    let internal_date_ms = response
        .internal_date
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    let participants = participants_from_headers(headers.as_slice());

    Ok(GmailMessage {
        id: response.id,
        thread_id: response.thread_id,
        payload: GmailMessagePayload {
            headers,
            body: GmailMessageBody {
                mime_type,
                data: body,
            },
        },
        participants,
        metadata: GmailMessageMetadata {
            internal_date_ms,
            label_ids: response.label_ids,
        },
    })
}

fn flatten_headers(headers: &[GmailHeaderValue]) -> Vec<GmailMessageHeader> {
    headers
        .iter()
        .map(|header| GmailMessageHeader {
            name: header.name.clone(),
            value: header.value.clone(),
        })
        .collect()
}

fn decode_message_body(payload: &GmailPayloadNode) -> (String, String) {
    if let Some((mime, data)) = first_part_data(payload, "text/plain") {
        return (mime, decode_gmail_body_data(data));
    }
    if let Some((mime, data)) = first_part_data(payload, "text/html") {
        return (mime, decode_gmail_body_data(data));
    }
    if let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref()) {
        return (
            payload
                .mime_type
                .clone()
                .unwrap_or_else(|| "text/plain".to_string()),
            decode_gmail_body_data(data),
        );
    }
    ("text/plain".to_string(), String::new())
}

fn first_part_data<'a>(
    payload: &'a GmailPayloadNode,
    mime_match: &str,
) -> Option<(String, &'a str)> {
    if payload
        .mime_type
        .as_deref()
        .is_some_and(|mime| mime.eq_ignore_ascii_case(mime_match))
        && let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref())
    {
        return Some((mime_match.to_string(), data));
    }

    for part in &payload.parts {
        if let Some(found) = first_part_data(part, mime_match) {
            return Some(found);
        }
    }
    None
}

fn decode_gmail_body_data(raw: &str) -> String {
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(raw));
    match decoded {
        Ok(bytes) => String::from_utf8(bytes)
            .unwrap_or_else(|_| String::from_utf8_lossy(raw.as_bytes()).to_string()),
        Err(_) => raw.to_string(),
    }
}

fn participants_from_headers(
    headers: &[GmailMessageHeader],
) -> Vec<openagents_email_agent::GmailThreadParticipant> {
    let mut participants = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for name in ["From", "To", "Cc", "Bcc"] {
        for header in headers
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case(name))
        {
            for token in header.value.split(',') {
                let (email, display_name) = parse_email_token(token);
                if email.is_empty() {
                    continue;
                }
                let normalized = email.to_ascii_lowercase();
                if !seen.insert(normalized) {
                    continue;
                }
                participants.push(openagents_email_agent::GmailThreadParticipant {
                    email,
                    display_name,
                });
            }
        }
    }
    participants
}

fn parse_email_token(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    if let (Some(open), Some(close)) = (trimmed.find('<'), trimmed.find('>'))
        && open < close
    {
        let display = trimmed[..open].trim().trim_matches('"').trim().to_string();
        let email = trimmed[open + 1..close].trim().to_string();
        let display_name = if display.is_empty() {
            None
        } else {
            Some(display)
        };
        return (email, display_name);
    }
    (trimmed.trim_matches('"').to_string(), None)
}

pub(super) struct LiveGmailHistoryProvider {
    session: GmailSession,
}

impl LiveGmailHistoryProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailHistoryProvider for LiveGmailHistoryProvider {
    fn fetch_history_since(
        &self,
        since_history_id: Option<u64>,
        max_results: usize,
    ) -> Result<GmailSyncBatch, GmailSyncError> {
        if let Some(since_history_id) = since_history_id {
            match self.fetch_history_batch(since_history_id, max_results) {
                Ok(batch) => Ok(batch),
                Err(error) if is_stale_history_error(error.as_str()) => Ok(GmailSyncBatch {
                    next_history_id: since_history_id.saturating_sub(1),
                    deltas: Vec::new(),
                }),
                Err(error) => Err(GmailSyncError::Provider(error)),
            }
        } else {
            self.fetch_profile_history_id()
        }
    }
}

impl LiveGmailHistoryProvider {
    fn fetch_profile_history_id(&self) -> Result<GmailSyncBatch, GmailSyncError> {
        #[derive(Deserialize)]
        struct ProfileResponse {
            #[serde(rename = "historyId")]
            history_id: Option<String>,
        }

        let profile = self
            .session
            .auth_get_json::<ProfileResponse>("profile", &[])
            .map_err(GmailSyncError::Provider)?;
        let next_history_id = profile
            .history_id
            .as_deref()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        Ok(GmailSyncBatch {
            next_history_id,
            deltas: Vec::new(),
        })
    }

    fn fetch_history_batch(
        &self,
        since_history_id: u64,
        max_results: usize,
    ) -> Result<GmailSyncBatch, String> {
        let response = self
            .session
            .auth_get_json::<LiveHistoryResponse>(
                "history",
                &[
                    ("startHistoryId", since_history_id.to_string()),
                    ("maxResults", max_results.to_string()),
                ],
            )
            .map_err(|error| {
                format!("gmail history fetch failed since {since_history_id}: {error}")
            })?;

        let mut deltas = Vec::<GmailDeltaItem>::new();
        let mut seen_delta_keys = BTreeSet::<String>::new();
        let mut max_history_id = response
            .history_id
            .as_deref()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(since_history_id);

        for entry in response.history {
            let history_id = entry
                .id
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(since_history_id);
            max_history_id = max_history_id.max(history_id);

            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.messages_added,
                GmailDeltaOperation::Create,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.messages_deleted,
                GmailDeltaOperation::Delete,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.labels_added,
                GmailDeltaOperation::Update,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.labels_removed,
                GmailDeltaOperation::Update,
                max_results,
            );
            if deltas.len() >= max_results {
                break;
            }
        }

        Ok(GmailSyncBatch {
            next_history_id: max_history_id,
            deltas,
        })
    }
}

#[derive(Deserialize)]
struct LiveHistoryResponse {
    #[serde(rename = "historyId")]
    history_id: Option<String>,
    #[serde(default)]
    history: Vec<LiveHistoryEntry>,
}

#[derive(Deserialize)]
struct LiveHistoryEntry {
    id: Option<String>,
    #[serde(rename = "messagesAdded", default)]
    messages_added: Vec<LiveHistoryRecord>,
    #[serde(rename = "messagesDeleted", default)]
    messages_deleted: Vec<LiveHistoryRecord>,
    #[serde(rename = "labelsAdded", default)]
    labels_added: Vec<LiveHistoryRecord>,
    #[serde(rename = "labelsRemoved", default)]
    labels_removed: Vec<LiveHistoryRecord>,
}

#[derive(Deserialize)]
struct LiveHistoryRecord {
    message: Option<LiveHistoryMessage>,
}

#[derive(Deserialize)]
struct LiveHistoryMessage {
    id: Option<String>,
}

fn ingest_history_rows(
    deltas: &mut Vec<GmailDeltaItem>,
    seen_delta_keys: &mut BTreeSet<String>,
    history_id: u64,
    rows: Vec<LiveHistoryRecord>,
    operation: GmailDeltaOperation,
    max_results: usize,
) {
    for row in rows {
        let Some(message_id) = row.message.and_then(|message| message.id) else {
            continue;
        };
        if message_id.is_empty() {
            continue;
        }
        let delta_key = format!("{history_id}:{operation:?}:{message_id}");
        if !seen_delta_keys.insert(delta_key) {
            continue;
        }
        deltas.push(GmailDeltaItem {
            message_id,
            operation,
            history_id,
        });
        if deltas.len() >= max_results {
            break;
        }
    }
}

fn is_stale_history_error(error: &str) -> bool {
    let lowercase = error.to_ascii_lowercase();
    lowercase.contains("404")
        || lowercase.contains("start historyid")
        || lowercase.contains("requested entity was not found")
}

pub(super) struct LiveGmailSendProvider {
    session: GmailSession,
}

impl LiveGmailSendProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailSendProvider for LiveGmailSendProvider {
    fn send_message(&self, request: &SendRequest) -> Result<GmailSendSuccess, SendProviderError> {
        #[derive(Serialize)]
        struct SendRequestBody {
            raw: String,
        }

        #[derive(Deserialize)]
        struct SendResponse {
            id: Option<String>,
        }

        let rfc822 = compose_plain_text_rfc822(request);
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rfc822.as_bytes());
        let response = self
            .session
            .auth_post_json::<_, SendResponse>("messages/send", &SendRequestBody { raw })?;
        let provider_message_id = response.id.unwrap_or_else(|| "gmail:unknown".to_string());
        Ok(GmailSendSuccess {
            provider_message_id,
        })
    }
}

fn compose_plain_text_rfc822(request: &SendRequest) -> String {
    let sanitized_subject = request.subject.replace('\r', " ").replace('\n', " ");
    let body = request.body.replace("\r\n", "\n").replace('\r', "\n");
    format!(
        "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\nMIME-Version: 1.0\r\nX-OpenAgents-Idempotency-Key: {}\r\n\r\n{}\r\n",
        request.recipient_email.trim(),
        sanitized_subject.trim(),
        request.idempotency_key.trim(),
        body.trim_end()
    )
}

pub(super) fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(super) fn stale_cursor_reason() -> &'static str {
    GMAIL_SYNC_REBOOTSTRAP_REASON
}

#[cfg(test)]
mod tests {
    use super::{compose_plain_text_rfc822, parse_email_token};
    use openagents_email_agent::SendRequest;

    #[test]
    fn parse_email_token_extracts_display_name_and_address() {
        let (email, display_name) = parse_email_token("Example Person <person@example.com>");
        assert_eq!(email, "person@example.com");
        assert_eq!(display_name.as_deref(), Some("Example Person"));
    }

    #[test]
    fn compose_rfc822_keeps_idempotency_key_header() {
        let request = SendRequest {
            draft_id: "draft-1".to_string(),
            idempotency_key: "idem-1".to_string(),
            recipient_email: "person@example.com".to_string(),
            subject: "Subject".to_string(),
            body: "Body".to_string(),
        };
        let message = compose_plain_text_rfc822(&request);
        assert!(message.contains("To: person@example.com"));
        assert!(message.contains("Subject: Subject"));
        assert!(message.contains("X-OpenAgents-Idempotency-Key: idem-1"));
        assert!(message.ends_with("Body\r\n"));
    }
}
