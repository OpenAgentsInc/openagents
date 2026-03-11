use crate::app_state::{
    EarnFailureClass, JobHistoryStatus, JobInboxDecision, JobInboxRequest, JobInboxValidation,
    JobLifecycleStage, PaneKind, PaneLoadState, ProviderMode, RenderState,
};
use crate::apple_fm_bridge::{
    AppleFmBridgeCommand, AppleFmBridgeUpdate, AppleFmExecutionCompleted, AppleFmExecutionFailed,
    AppleFmExecutionStarted, AppleFmGenerateJob,
};
use crate::codex_lane::{
    CodexLaneCommand, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneNotification,
};
use crate::local_inference_runtime::{
    LocalInferenceExecutionCompleted, LocalInferenceExecutionFailed,
    LocalInferenceExecutionStarted, LocalInferenceGenerateJob, LocalInferenceRuntimeCommand,
    LocalInferenceRuntimeUpdate,
};
use crate::pane_system::{
    ActiveJobPaneAction, JobHistoryPaneAction, JobInboxPaneAction, PaneController,
};
use crate::provider_nip90_lane::{
    ProviderNip90ComputeCapability, ProviderNip90LaneCommand, ProviderNip90PublishOutcome,
    ProviderNip90PublishRole,
};
use crate::state::provider_runtime::{
    LocalInferenceBackend, ProviderAppleFmRuntimeState, ProviderOllamaRuntimeState,
};
use nostr::nip90::{
    JobFeedback, JobResult, JobStatus, KIND_JOB_TEXT_GENERATION, create_job_feedback_event,
    create_job_result_event,
};
use nostr::{Event, EventTemplate, NostrIdentity};

const MIN_PROVIDER_PRICE_SATS: u64 = 1;
const MIN_PROVIDER_TTL_SECONDS: u64 = 30;
const RESULT_PUBLISH_RETRY_INTERVAL_SECONDS: u64 = 5;
const RESULT_PUBLISH_RETRY_GRACE_SECONDS: u64 = 15;
const MAX_RESULT_PUBLISH_ATTEMPTS: u32 = 4;
const PROVIDER_SETTLEMENT_GRACE_SECONDS: u64 = 120;
const MIN_PROVIDER_SETTLEMENT_TIMEOUT_SECONDS: u64 = 180;

pub(super) fn run_job_inbox_action(state: &mut RenderState, action: JobInboxPaneAction) -> bool {
    match action {
        JobInboxPaneAction::SelectRow(index) => {
            if !state.job_inbox.select_by_index(index) {
                state.job_inbox.last_error = Some("Request row out of range".to_string());
                state.job_inbox.load_state = PaneLoadState::Error;
            } else {
                state.job_inbox.load_state = PaneLoadState::Ready;
            }
            true
        }
        JobInboxPaneAction::AcceptSelected => {
            let Some(selected_request_id) = state.job_inbox.selected_request_id.clone() else {
                state.job_inbox.last_error = Some("Select a request first".to_string());
                state.job_inbox.load_state = PaneLoadState::Error;
                return true;
            };
            if let Err(error) = accept_request_by_id(
                state,
                selected_request_id.as_str(),
                "validated + queued for runtime",
                "job.inbox.accept",
            ) {
                state.job_inbox.last_error = Some(error);
                state.job_inbox.load_state = PaneLoadState::Error;
            }
            true
        }
        JobInboxPaneAction::RejectSelected => {
            if let Some(reason) = state
                .job_inbox
                .preview_block_reason(state.provider_runtime.mode)
            {
                state.job_inbox.last_error = Some(reason.to_string());
                state.job_inbox.last_action =
                    Some("Offline market preview is read-only".to_string());
                state.job_inbox.load_state = PaneLoadState::Error;
                return true;
            }
            match state
                .job_inbox
                .decide_selected(false, "failed policy preflight")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = PaneLoadState::Ready;
                    state.provider_runtime.last_result =
                        Some(format!("runtime rejected request {request_id}"));
                    let rejected_request = state
                        .job_inbox
                        .requests
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .cloned();
                    if let Some(request) = rejected_request.as_ref() {
                        state.earn_kernel_receipts.record_preflight_rejection(
                            request,
                            "failed policy preflight",
                            current_epoch_seconds(),
                            "job.inbox.reject",
                        );
                    }
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = PaneLoadState::Error;
                }
            }
            true
        }
    }
}

pub(super) fn run_job_inbox_auto_admission_tick(state: &mut RenderState) -> bool {
    if state.provider_runtime.mode != ProviderMode::Online {
        return false;
    }

    if let Some((request_id, reason)) = next_invalid_request_rejection(state) {
        if let Err(error) = reject_request_by_id(
            state,
            request_id.as_str(),
            reason.as_str(),
            "job.inbox.auto_reject",
        ) {
            state.job_inbox.last_error = Some(error);
            state.job_inbox.load_state = PaneLoadState::Error;
        }
        return true;
    }

    let Some(request_id) = next_auto_accept_request_id(state) else {
        return false;
    };

    if let Err(error) = accept_request_by_id(
        state,
        request_id.as_str(),
        "auto-accepted by provider policy",
        "job.inbox.auto_accept",
    ) {
        state.job_inbox.last_error = Some(error);
        state.job_inbox.load_state = PaneLoadState::Error;
    }
    true
}

pub(super) fn run_active_job_execution_tick(state: &mut RenderState) -> bool {
    let Some((stage, ttl_seconds, has_result_event)) = state.active_job.job.as_ref().map(|job| {
        (
            job.stage,
            job.ttl_seconds,
            job.sa_tick_result_event_id.is_some(),
        )
    }) else {
        return false;
    };
    if stage.is_terminal() {
        return false;
    }

    let now_epoch_seconds = current_epoch_seconds();
    if stage == JobLifecycleStage::Running
        && state.active_job.execution_turn_completed
        && !has_result_event
        && result_publish_retry_due(
            state.active_job.result_publish_last_queued_epoch_seconds,
            now_epoch_seconds,
        )
        && state.active_job.result_publish_attempt_count < MAX_RESULT_PUBLISH_ATTEMPTS
        && active_job_has_pending_result_publish(&state.active_job)
    {
        match retry_runtime_result_publish(state) {
            Ok(()) => {
                extend_active_job_phase_deadline_at_least(
                    &mut state.active_job,
                    RESULT_PUBLISH_RETRY_GRACE_SECONDS,
                    now_epoch_seconds,
                );
                return true;
            }
            Err(error) => {
                state
                    .active_job
                    .append_event(format!("result publish retry queue failed: {error}"));
                state.active_job.last_action =
                    Some("result publish retry queue failed".to_string());
                state.provider_runtime.last_result = Some(format!(
                    "provider result publish retry queue failed: {error}"
                ));
            }
        }
    }
    if state
        .active_job
        .execution_deadline_epoch_seconds
        .is_some_and(|deadline| now_epoch_seconds > deadline)
    {
        fail_active_job_execution(
            state,
            active_job_timeout_reason(
                stage,
                state.active_job.execution_turn_completed,
                ttl_seconds,
            ),
            "active_job.execution_timeout",
            true,
        );
        return true;
    }

    if stage == JobLifecycleStage::Accepted {
        match provider_execution_backend_for_active_job(state) {
            Some(ProviderExecutionBackend::Ollama) => {
                if state.active_job.execution_backend_request_id.is_some() {
                    return false;
                }
                match queue_provider_ollama_execution_start(state) {
                    Ok(()) => return true,
                    Err(error) => {
                        fail_active_job_execution(
                            state,
                            format!("failed to start local inference execution: {error}"),
                            "active_job.ollama_execution_start_failed",
                            true,
                        );
                        return true;
                    }
                }
            }
            Some(ProviderExecutionBackend::AppleFoundationModels) => {
                if state.active_job.execution_backend_request_id.is_some() {
                    return false;
                }
                match queue_provider_apple_fm_execution_start(state) {
                    Ok(()) => return true,
                    Err(error) => {
                        fail_active_job_execution(
                            state,
                            format!("failed to start Apple Foundation Models execution: {error}"),
                            "active_job.apple_fm_execution_start_failed",
                            true,
                        );
                        return true;
                    }
                }
            }
            Some(ProviderExecutionBackend::Codex) => {
                if state.active_job.execution_thread_id.is_some()
                    || state
                        .active_job
                        .execution_thread_start_command_seq
                        .is_some()
                {
                    return false;
                }
                match queue_provider_execution_thread_start(state) {
                    Ok(()) => return true,
                    Err(error) => {
                        fail_active_job_execution(
                            state,
                            format!("failed to start provider execution thread: {error}"),
                            "active_job.execution_thread_start_failed",
                            true,
                        );
                        return true;
                    }
                }
            }
            None => return false,
        }
    }

    if stage == JobLifecycleStage::Running
        && state.active_job.execution_turn_completed
        && !has_result_event
        && !state.active_job.result_publish_in_flight
        && !active_job_has_pending_result_publish(&state.active_job)
    {
        match queue_runtime_result_publish(state) {
            Ok(()) => return true,
            Err(error) => {
                fail_active_job_execution(
                    state,
                    format!("execution completed but result publish failed: {error}"),
                    "active_job.result_publish_failed",
                    true,
                );
                return true;
            }
        }
    }

    if stage == JobLifecycleStage::Delivered
        && active_job_requires_payment_required_feedback(state)
        && !state.active_job.payment_required_feedback_in_flight
        && !state.active_job.payment_required_failed
    {
        match queue_active_job_payment_required_feedback(state) {
            Ok(true) => return true,
            Ok(false) => {}
            Err(error) => {
                state.active_job.last_error = Some(error.clone());
                state.active_job.load_state = PaneLoadState::Error;
                state.provider_runtime.last_result = Some(error);
                state.provider_runtime.last_authoritative_error_class =
                    Some(EarnFailureClass::Payment);
                state.active_job.payment_required_failed = true;
                return true;
            }
        }
    }

    false
}

pub(super) fn active_job_owns_codex_command_response(
    state: &RenderState,
    command_seq: u64,
) -> bool {
    [
        state.active_job.execution_thread_start_command_seq,
        state.active_job.execution_turn_start_command_seq,
        state.active_job.execution_turn_interrupt_command_seq,
    ]
    .into_iter()
    .flatten()
    .any(|candidate| candidate == command_seq)
}

pub(super) fn apply_active_job_codex_command_response(
    state: &mut RenderState,
    response: &CodexLaneCommandResponse,
) {
    if !active_job_owns_codex_command_response(state, response.command_seq) {
        return;
    }

    if response.status == CodexLaneCommandStatus::Accepted {
        if state.active_job.execution_thread_start_command_seq == Some(response.command_seq) {
            state
                .active_job
                .append_event("provider execution thread start accepted");
        } else if state.active_job.execution_turn_start_command_seq == Some(response.command_seq) {
            state
                .active_job
                .append_event("provider execution turn start accepted");
        } else if state.active_job.execution_turn_interrupt_command_seq
            == Some(response.command_seq)
        {
            state
                .active_job
                .append_event("provider execution interrupt accepted");
            state.active_job.execution_turn_interrupt_command_seq = None;
        }
        return;
    }

    let detail = response
        .error
        .clone()
        .unwrap_or_else(|| format!("{} {}", response.command.label(), response.status.label()));
    if state.active_job.execution_thread_start_command_seq == Some(response.command_seq) {
        state.active_job.execution_thread_start_command_seq = None;
        fail_active_job_execution(
            state,
            format!("provider execution thread start rejected: {detail}"),
            "active_job.execution_thread_start_rejected",
            true,
        );
    } else if state.active_job.execution_turn_start_command_seq == Some(response.command_seq) {
        state.active_job.execution_turn_start_command_seq = None;
        fail_active_job_execution(
            state,
            format!("provider execution turn start rejected: {detail}"),
            "active_job.execution_turn_start_rejected",
            true,
        );
    } else if state.active_job.execution_turn_interrupt_command_seq == Some(response.command_seq) {
        state.active_job.execution_turn_interrupt_command_seq = None;
        state.active_job.append_event(format!(
            "provider execution interrupt request rejected: {detail}"
        ));
    }
}

pub(super) fn apply_active_job_codex_notification(
    state: &mut RenderState,
    notification: &CodexLaneNotification,
) -> bool {
    match notification {
        CodexLaneNotification::ThreadStarted { thread_id, .. } => {
            if state.active_job.job.is_none()
                || state.active_job.execution_thread_id.is_some()
                || state
                    .active_job
                    .execution_thread_start_command_seq
                    .is_none()
            {
                return false;
            }
            state.active_job.execution_thread_start_command_seq = None;
            state.active_job.execution_thread_id = Some(thread_id.clone());
            state
                .active_job
                .append_event(format!("provider execution thread ready: {thread_id}"));
            if let Err(error) = queue_provider_execution_turn_start(state, thread_id.as_str()) {
                fail_active_job_execution(
                    state,
                    format!("failed to start provider execution turn: {error}"),
                    "active_job.execution_turn_start_failed",
                    true,
                );
            }
            true
        }
        CodexLaneNotification::TurnStarted { thread_id, turn_id }
            if active_job_matches_execution_thread(state, thread_id) =>
        {
            state.active_job.execution_turn_start_command_seq = None;
            state.active_job.execution_turn_id = Some(turn_id.clone());
            state.active_job.runtime_supports_abort = true;
            state.provider_runtime.last_result = Some(format!(
                "provider execution started thread={} turn={}",
                thread_id, turn_id
            ));
            state
                .active_job
                .append_event(format!("provider execution turn started: {turn_id}"));
            if let Err(error) =
                transition_active_job_to_running(state, "active_job.execution_started")
            {
                fail_active_job_execution(
                    state,
                    format!("failed to transition active job to running: {error}"),
                    "active_job.execution_running_transition_failed",
                    true,
                );
            }
            true
        }
        CodexLaneNotification::AgentMessageCompleted {
            thread_id,
            turn_id,
            message,
            ..
        } if active_job_matches_execution_turn(state, thread_id, turn_id) => {
            store_execution_output(state, message.as_str());
            true
        }
        CodexLaneNotification::ItemCompleted {
            thread_id,
            turn_id,
            item_type,
            message,
            ..
        } if active_job_matches_execution_turn(state, thread_id, turn_id) => {
            if item_type
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("agentMessage"))
                && let Some(message) = message.as_deref()
            {
                store_execution_output(state, message);
            }
            true
        }
        CodexLaneNotification::TurnCompleted {
            thread_id,
            turn_id,
            status,
            error_message,
            final_message,
        } if active_job_matches_execution_turn(state, thread_id, turn_id) => {
            if let Some(final_message) = final_message.as_deref() {
                store_execution_output(state, final_message);
            }
            state.active_job.execution_turn_completed = true;
            state.active_job.runtime_supports_abort = false;
            state.active_job.append_event(format!(
                "provider execution turn completed status={}",
                status.as_deref().unwrap_or("none")
            ));
            if turn_completed_failed(status.as_deref(), error_message.as_deref()) {
                fail_active_job_execution(
                    state,
                    error_message.clone().unwrap_or_else(|| {
                        format!(
                            "provider execution failed with status {}",
                            status.as_deref().unwrap_or("error")
                        )
                    }),
                    "active_job.execution_failed",
                    true,
                );
            } else {
                refresh_active_job_phase_deadline(state);
            }
            true
        }
        CodexLaneNotification::TurnError {
            thread_id,
            turn_id,
            message,
        } if active_job_matches_execution_turn(state, thread_id, turn_id) => {
            fail_active_job_execution(
                state,
                format!("provider execution turn error: {message}"),
                "active_job.execution_error",
                true,
            );
            true
        }
        _ => false,
    }
}

pub(super) fn apply_active_job_publish_outcome(
    state: &mut RenderState,
    outcome: &ProviderNip90PublishOutcome,
) {
    if !active_job_matches_publish_outcome(&state.active_job, outcome) {
        return;
    }

    match outcome.role {
        ProviderNip90PublishRole::Capability => {}
        ProviderNip90PublishRole::Result => {
            state.active_job.result_publish_in_flight = false;
            if outcome.accepted_relays == 0 {
                let message = format!(
                    "result publish failed; retry pending ({})",
                    outcome
                        .first_error
                        .as_deref()
                        .unwrap_or("all relays rejected publish")
                );
                state.active_job.append_event(message.clone());
                state.active_job.last_action = Some(message.clone());
                tracing::error!(
                    target: "autopilot_desktop::provider",
                    "Provider result publish failed request_id={} event_id={} attempt={} error={}",
                    outcome.request_id,
                    outcome.event_id,
                    state.active_job.result_publish_attempt_count,
                    outcome
                        .first_error
                        .as_deref()
                        .unwrap_or("all relays rejected publish")
                );
                return;
            }
            state.active_job.pending_result_publish_event_id = None;
            state.active_job.pending_result_publish_event = None;
            state.active_job.result_publish_attempt_count = 0;
            state.active_job.result_publish_last_queued_epoch_seconds = None;
            if let Some(job) = state.active_job.job.as_mut()
                && job.sa_tick_result_event_id.is_none()
            {
                job.sa_tick_result_event_id = Some(outcome.event_id.clone());
            }
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Provider result published request_id={} event_id={} accepted_relays={} rejected_relays={}",
                outcome.request_id,
                outcome.event_id,
                outcome.accepted_relays,
                outcome.rejected_relays
            );
            if let Ok(JobLifecycleStage::Delivered) =
                transition_active_job_to_delivered(state, "active_job.result_published")
            {
                state.provider_runtime.last_result = Some(format!(
                    "provider execution delivered request {}",
                    outcome.request_id
                ));
            }
        }
        ProviderNip90PublishRole::Feedback => {
            if apply_payment_required_feedback_publish_outcome(
                &mut state.active_job,
                &mut state.provider_runtime,
                outcome,
            ) {
                return;
            }
            if outcome.accepted_relays == 0 {
                state.active_job.append_event(format!(
                    "feedback publish failed ({})",
                    outcome
                        .first_error
                        .as_deref()
                        .unwrap_or("all relays rejected publish")
                ));
            }
        }
        ProviderNip90PublishRole::Request => {}
    }
}

pub(super) fn apply_active_job_local_inference_runtime_update(
    state: &mut RenderState,
    update: LocalInferenceRuntimeUpdate,
) -> bool {
    match update {
        LocalInferenceRuntimeUpdate::Snapshot(snapshot) => {
            state.ollama_execution = (*snapshot).into();
            sync_provider_runtime_ollama_state(state);
            if state
                .provider_runtime
                .inventory_session_started_at_ms
                .is_some()
                && !matches!(state.provider_runtime.mode, ProviderMode::Offline)
                && let Err(error) =
                    crate::kernel_control::register_online_compute_inventory_with_kernel(state)
            {
                state.provider_runtime.last_error_detail = Some(error.clone());
                state.provider_runtime.last_result = Some(format!(
                    "Kernel online inventory registration failed: {error}"
                ));
                state.provider_runtime.last_authoritative_error_class =
                    Some(EarnFailureClass::Reconciliation);
            }
            sync_provider_nip90_compute_capability(state);
            super::provider_ingress::sync_provider_runtime_mode_from_provider_state(state);
            true
        }
        LocalInferenceRuntimeUpdate::Started(started) => {
            apply_ollama_execution_started(state, started)
        }
        LocalInferenceRuntimeUpdate::Completed(completed) => {
            apply_ollama_execution_completed(state, completed)
        }
        LocalInferenceRuntimeUpdate::Failed(failed) => apply_ollama_execution_failed(state, failed),
    }
}

pub(super) fn apply_active_job_apple_fm_update(
    state: &mut RenderState,
    update: AppleFmBridgeUpdate,
) -> bool {
    match update {
        AppleFmBridgeUpdate::Snapshot(snapshot) => {
            state.apple_fm_execution = *snapshot;
            sync_provider_runtime_apple_fm_state(state);
            if state
                .provider_runtime
                .inventory_session_started_at_ms
                .is_some()
                && !matches!(state.provider_runtime.mode, ProviderMode::Offline)
                && let Err(error) =
                    crate::kernel_control::register_online_compute_inventory_with_kernel(state)
            {
                state.provider_runtime.last_error_detail = Some(error.clone());
                state.provider_runtime.last_result = Some(format!(
                    "Kernel online inventory registration failed: {error}"
                ));
                state.provider_runtime.last_authoritative_error_class =
                    Some(EarnFailureClass::Reconciliation);
            }
            sync_provider_nip90_compute_capability(state);
            super::provider_ingress::sync_provider_runtime_mode_from_provider_state(state);
            true
        }
        AppleFmBridgeUpdate::Started(started) => apply_apple_fm_execution_started(state, started),
        AppleFmBridgeUpdate::Completed(completed) => {
            apply_apple_fm_execution_completed(state, completed)
        }
        AppleFmBridgeUpdate::Failed(failed) => apply_apple_fm_execution_failed(state, failed),
        AppleFmBridgeUpdate::Workbench(_) | AppleFmBridgeUpdate::MissionControlSummary(_) => false,
    }
}

fn sync_provider_runtime_ollama_state(state: &mut RenderState) {
    state.provider_runtime.ollama.reachable = state.ollama_execution.reachable;
    state.provider_runtime.ollama.configured_model =
        state.ollama_execution.configured_model.clone();
    state.provider_runtime.ollama.ready_model = state.ollama_execution.ready_model.clone();
    state.provider_runtime.ollama.available_models =
        state.ollama_execution.available_models.clone();
    state.provider_runtime.ollama.loaded_models = state.ollama_execution.loaded_models.clone();
    state.provider_runtime.ollama.last_error = state.ollama_execution.last_error.clone();
    state.provider_runtime.ollama.last_action = state.ollama_execution.last_action.clone();
    state.provider_runtime.ollama.last_request_id = state.ollama_execution.last_request_id.clone();
    state.provider_runtime.ollama.last_metrics = state.ollama_execution.last_metrics.clone();
    state.provider_runtime.ollama.refreshed_at = state.ollama_execution.refreshed_at;
}

fn sync_provider_runtime_apple_fm_state(state: &mut RenderState) {
    state.provider_runtime.apple_fm.reachable = state.apple_fm_execution.reachable;
    state.provider_runtime.apple_fm.model_available = state.apple_fm_execution.model_available;
    state.provider_runtime.apple_fm.system_model = state.apple_fm_execution.system_model.clone();
    state.provider_runtime.apple_fm.unavailable_reason =
        state.apple_fm_execution.unavailable_reason;
    state.provider_runtime.apple_fm.supported_use_cases =
        state.apple_fm_execution.supported_use_cases.clone();
    state.provider_runtime.apple_fm.supported_guardrails =
        state.apple_fm_execution.supported_guardrails.clone();
    state.provider_runtime.apple_fm.ready_model = state.apple_fm_execution.ready_model.clone();
    state.provider_runtime.apple_fm.available_models =
        state.apple_fm_execution.available_models.clone();
    state.provider_runtime.apple_fm.last_error = state.apple_fm_execution.last_error.clone();
    state.provider_runtime.apple_fm.last_action = state.apple_fm_execution.last_action.clone();
    state.provider_runtime.apple_fm.last_request_id =
        state.apple_fm_execution.last_request_id.clone();
    state.provider_runtime.apple_fm.last_metrics = state.apple_fm_execution.last_metrics.clone();
    state.provider_runtime.apple_fm.refreshed_at = state.apple_fm_execution.refreshed_at;
    state.provider_runtime.apple_fm.availability_message =
        state.apple_fm_execution.availability_message.clone();
    state.provider_runtime.apple_fm.bridge_status = state.apple_fm_execution.bridge_status.clone();
}

fn sync_provider_nip90_compute_capability(state: &mut RenderState) {
    if !crate::app_state::mission_control_sell_compute_supported(
        state.desktop_shell_mode,
        &state.ollama_execution,
    ) {
        return;
    }
    if matches!(
        crate::app_state::mission_control_local_runtime_lane(
            state.desktop_shell_mode,
            &state.ollama_execution
        ),
        Some(crate::app_state::MissionControlLocalRuntimeLane::AppleFoundationModels)
    ) && !state
        .provider_runtime
        .apple_fm
        .has_authoritative_capability_state()
    {
        return;
    }
    let capability = preferred_provider_compute_capability(state);
    let _ = state.queue_provider_nip90_lane_command(
        ProviderNip90LaneCommand::ConfigureComputeCapability { capability },
    );
}

fn sync_provider_publish_continuity(state: &mut RenderState) {
    let request_ids = state
        .active_job
        .job
        .as_ref()
        .filter(|job| !job.stage.is_terminal())
        .map(|job| vec![job.request_id.clone()])
        .unwrap_or_default();
    if let Err(error) = state.queue_provider_nip90_lane_command(
        ProviderNip90LaneCommand::TrackProviderPublishRequestIds { request_ids },
    ) {
        state.active_job.append_event(format!(
            "failed to sync provider publish continuity: {error}"
        ));
        tracing::warn!(
            target: "autopilot_desktop::provider",
            "Failed syncing provider publish continuity request_ids={} error={}",
            state
                .active_job
                .job
                .as_ref()
                .map(|job| job.request_id.as_str())
                .unwrap_or("none"),
            error
        );
    }
}

fn finalize_deferred_provider_runtime_shutdown_if_idle(state: &mut RenderState) {
    if !state.provider_runtime.defer_runtime_shutdown_until_idle
        || state.active_job.inflight_job_count() > 0
    {
        return;
    }
    state.provider_runtime.defer_runtime_shutdown_until_idle = false;
    state.provider_runtime.inventory_session_started_at_ms = None;
    let _ = state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::StopBridge);
    let _ = state
        .queue_local_inference_runtime_command(LocalInferenceRuntimeCommand::UnloadConfiguredModel);
    state
        .active_job
        .append_event("provider runtime drain complete; shutting down local execution runtimes");
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider runtime drain complete; stopped local execution runtimes after offline request"
    );
}

fn active_job_timeout_reason(
    stage: JobLifecycleStage,
    execution_turn_completed: bool,
    ttl_seconds: u64,
) -> String {
    match stage {
        JobLifecycleStage::Running if execution_turn_completed => format!(
            "job result publish continuity timed out after {}s while awaiting relay delivery confirmation",
            active_job_result_publish_continuity_timeout_seconds(ttl_seconds)
        ),
        JobLifecycleStage::Delivered => format!(
            "job settlement timed out after {}s while awaiting payment flow",
            active_job_settlement_timeout_seconds(ttl_seconds)
        ),
        _ => format!("job execution timed out after {}s", ttl_seconds),
    }
}

fn provider_grace_timeout_seconds(ttl_seconds: u64) -> u64 {
    ttl_seconds
        .saturating_add(PROVIDER_SETTLEMENT_GRACE_SECONDS)
        .max(MIN_PROVIDER_SETTLEMENT_TIMEOUT_SECONDS)
}

fn active_job_result_publish_continuity_timeout_seconds(ttl_seconds: u64) -> u64 {
    provider_grace_timeout_seconds(ttl_seconds)
}

fn active_job_settlement_timeout_seconds(ttl_seconds: u64) -> u64 {
    provider_grace_timeout_seconds(ttl_seconds)
}

fn active_job_phase_timeout_seconds(
    stage: JobLifecycleStage,
    execution_turn_completed: bool,
    ttl_seconds: u64,
) -> u64 {
    match stage {
        JobLifecycleStage::Running if execution_turn_completed => {
            active_job_result_publish_continuity_timeout_seconds(ttl_seconds)
        }
        JobLifecycleStage::Delivered => active_job_settlement_timeout_seconds(ttl_seconds),
        _ => ttl_seconds,
    }
}

fn set_active_job_phase_deadline_at(
    active_job: &mut crate::app_state::ActiveJobState,
    ttl_seconds: u64,
    now_epoch_seconds: u64,
) {
    active_job.execution_deadline_epoch_seconds =
        Some(now_epoch_seconds.saturating_add(ttl_seconds));
}

fn extend_active_job_phase_deadline_at_least(
    active_job: &mut crate::app_state::ActiveJobState,
    ttl_seconds: u64,
    now_epoch_seconds: u64,
) {
    let extended_deadline = now_epoch_seconds.saturating_add(ttl_seconds);
    active_job.execution_deadline_epoch_seconds = Some(
        active_job
            .execution_deadline_epoch_seconds
            .unwrap_or(0)
            .max(extended_deadline),
    );
}

fn refresh_active_job_phase_deadline(state: &mut RenderState) {
    let Some((stage, ttl_seconds)) = state
        .active_job
        .job
        .as_ref()
        .map(|job| (job.stage, job.ttl_seconds))
    else {
        return;
    };
    let timeout_seconds = active_job_phase_timeout_seconds(
        stage,
        state.active_job.execution_turn_completed,
        ttl_seconds,
    );
    set_active_job_phase_deadline_at(
        &mut state.active_job,
        timeout_seconds,
        current_epoch_seconds(),
    );
}

fn clear_active_job_phase_deadline(active_job: &mut crate::app_state::ActiveJobState) {
    active_job.execution_deadline_epoch_seconds = None;
}

fn result_publish_retry_due(
    last_queued_epoch_seconds: Option<u64>,
    now_epoch_seconds: u64,
) -> bool {
    last_queued_epoch_seconds.is_some_and(|queued_at| {
        now_epoch_seconds.saturating_sub(queued_at) >= RESULT_PUBLISH_RETRY_INTERVAL_SECONDS
    })
}

fn active_job_has_pending_result_publish(active_job: &crate::app_state::ActiveJobState) -> bool {
    active_job.pending_result_publish_event.is_some()
        || active_job.pending_result_publish_event_id.is_some()
}

fn preferred_provider_compute_capability(state: &RenderState) -> ProviderNip90ComputeCapability {
    match state.provider_runtime.active_inference_backend() {
        Some(LocalInferenceBackend::AppleFoundationModels) => {
            provider_compute_capability_from_apple_fm(state)
        }
        Some(LocalInferenceBackend::Ollama) => provider_compute_capability_from_ollama(state),
        None if state.provider_runtime.ollama.reachable
            || state.provider_runtime.ollama.last_error.is_some()
            || state.provider_runtime.ollama.configured_model.is_some() =>
        {
            provider_compute_capability_from_ollama(state)
        }
        None => provider_compute_capability_from_apple_fm(state),
    }
}

fn provider_compute_capability_from_ollama(state: &RenderState) -> ProviderNip90ComputeCapability {
    ProviderNip90ComputeCapability {
        backend: "psionic".to_string(),
        reachable: state.ollama_execution.reachable,
        configured_model: state.ollama_execution.configured_model.clone(),
        ready_model: state.ollama_execution.ready_model.clone(),
        available_models: state.ollama_execution.available_models.clone(),
        loaded_models: state.ollama_execution.loaded_models.clone(),
        last_error: state.ollama_execution.last_error.clone(),
    }
}

fn provider_compute_capability_from_apple_fm(
    state: &RenderState,
) -> ProviderNip90ComputeCapability {
    let apple_fm = &state.provider_runtime.apple_fm;
    ProviderNip90ComputeCapability {
        backend: "apple_foundation_models".to_string(),
        reachable: apple_fm.reachable,
        configured_model: None,
        ready_model: apple_fm.ready_model.clone(),
        available_models: apple_fm.available_models.clone(),
        loaded_models: Vec::new(),
        last_error: apple_fm.availability_error_message(),
    }
}

pub(super) fn run_active_job_action(state: &mut RenderState, action: ActiveJobPaneAction) -> bool {
    let now = std::time::Instant::now();
    match action {
        ActiveJobPaneAction::AdvanceStage => {
            state.active_job.last_error = Some(
                "Active-job lifecycle is runtime-driven. Manual stage advance is disabled."
                    .to_string(),
            );
            state.active_job.load_state = PaneLoadState::Error;
            state.provider_runtime.last_result =
                Some("manual active-job stage advance ignored".to_string());
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
        ActiveJobPaneAction::AbortJob => {
            let _ = queue_provider_execution_interrupt(state);
            fail_active_job_execution(
                state,
                "operator requested abort".to_string(),
                "active_job.abort",
                true,
            );
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

pub(super) fn run_job_history_action(
    state: &mut RenderState,
    action: JobHistoryPaneAction,
) -> bool {
    let now = std::time::Instant::now();
    match action {
        JobHistoryPaneAction::CycleStatusFilter => {
            state.job_history.cycle_status_filter();
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
        JobHistoryPaneAction::CycleTimeRange => {
            state.job_history.cycle_time_range();
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
        JobHistoryPaneAction::PreviousPage => {
            state.job_history.previous_page();
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
        JobHistoryPaneAction::NextPage => {
            state.job_history.next_page();
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

fn build_nip90_result_event_for_active_job(state: &mut RenderState) -> Result<Event, String> {
    let Some(identity) = state.nostr_identity.as_ref() else {
        return Err("Cannot build result publish: Nostr identity unavailable".to_string());
    };
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot build result publish: no active job selected".to_string());
    };

    let request_kind = job.request_kind;
    let request_id = job.request_id.clone();
    let requester = job.requester.clone();
    let quoted_price_sats = job.quoted_price_sats;
    let execution_output = state
        .active_job
        .execution_output
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider execution completed without explicit output");
    let visible_content = visible_result_content_for_job_kind(request_kind, execution_output);

    let mut result = JobResult::new(request_kind, request_id.clone(), requester, visible_content)
        .map_err(|error| format!("Cannot build NIP-90 result event: {error}"))?;
    if quoted_price_sats > 0 {
        result = result.with_amount(quoted_price_sats.saturating_mul(1000), None);
    }
    let template = create_job_result_event(&result);
    sign_event_template(identity, &template)
}

fn queue_signed_nip90_result_publish_for_active_job(
    state: &mut RenderState,
    event: Event,
) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot publish result: no active job selected".to_string());
    };
    let request_id = job.request_id.clone();
    let event_id = event.id.clone();
    state
        .queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id,
            role: ProviderNip90PublishRole::Result,
            event: Box::new(event),
        })
        .map_err(|error| format!("Cannot queue NIP-90 result publish: {error}"))?;

    Ok(event_id)
}

fn visible_result_content_for_job_kind(request_kind: u16, execution_output: &str) -> String {
    if request_kind == KIND_JOB_TEXT_GENERATION {
        return execution_output.trim().to_string();
    }
    serde_json::json!({
        "status": "completed",
        "output": execution_output.trim(),
    })
    .to_string()
}

fn queue_nip90_feedback_for_active_job(
    state: &mut RenderState,
    status: JobStatus,
    status_extra: impl Into<String>,
    content: Option<String>,
    include_amount: bool,
    bolt11: Option<&str>,
) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot publish feedback: no active job selected".to_string());
    };
    let event = build_nip90_feedback_event(
        state
            .nostr_identity
            .as_ref()
            .ok_or_else(|| "Cannot publish feedback: Nostr identity unavailable".to_string())?,
        job.request_id.as_str(),
        job.requester.as_str(),
        job.quoted_price_sats,
        status,
        status_extra,
        content,
        include_amount,
        bolt11,
    )?;
    let request_id = job.request_id.clone();
    let event_id = event.id.clone();

    state
        .queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id,
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(event),
        })
        .map_err(|error| format!("Cannot queue NIP-90 feedback publish: {error}"))?;

    Ok(event_id)
}

fn build_nip90_feedback_event(
    identity: &NostrIdentity,
    request_id: &str,
    requester: &str,
    quoted_price_sats: u64,
    status: JobStatus,
    status_extra: impl Into<String>,
    content: Option<String>,
    include_amount: bool,
    bolt11: Option<&str>,
) -> Result<Event, String> {
    let mut feedback =
        JobFeedback::new(status, request_id, requester).with_status_extra(status_extra.into());
    if let Some(content) = content {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            feedback = feedback.with_content(trimmed.to_string());
        }
    }
    if include_amount && quoted_price_sats > 0 {
        feedback = feedback.with_amount(
            quoted_price_sats.saturating_mul(1000),
            bolt11.map(ToString::to_string),
        );
    }
    let template = create_job_feedback_event(&feedback);
    sign_event_template(identity, &template)
}

fn apply_payment_required_feedback_publish_outcome(
    active_job: &mut crate::app_state::ActiveJobState,
    provider_runtime: &mut crate::state::provider_runtime::ProviderRuntimeState,
    outcome: &ProviderNip90PublishOutcome,
) -> bool {
    if !active_job.payment_required_feedback_in_flight {
        return false;
    }

    active_job.payment_required_feedback_in_flight = false;
    if outcome.accepted_relays == 0 {
        if let Some(job) = active_job.job.as_mut() {
            job.invoice_id = None;
        }
        active_job.payment_required_failed = true;
        let message = format!(
            "payment-required feedback publish failed ({})",
            outcome
                .first_error
                .as_deref()
                .unwrap_or("all relays rejected publish")
        );
        active_job.append_event(message.clone());
        active_job.last_error = Some(message.clone());
        active_job.load_state = PaneLoadState::Error;
        provider_runtime.last_result = Some(message);
        provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Payment);
        return true;
    }

    active_job.payment_required_failed = false;
    active_job.last_error = None;
    active_job.load_state = PaneLoadState::Ready;
    if let Some(ttl_seconds) = active_job.job.as_ref().map(|job| job.ttl_seconds) {
        extend_active_job_phase_deadline_at_least(
            active_job,
            active_job_settlement_timeout_seconds(ttl_seconds),
            current_epoch_seconds(),
        );
    }
    active_job.append_event(format!(
        "payment-required feedback published {}",
        outcome.event_id
    ));
    active_job.last_action = Some(format!(
        "Awaiting Lightning settlement after publishing {}",
        outcome.event_id
    ));
    provider_runtime.last_result = Some(format!(
        "provider requested Lightning settlement for request {}",
        outcome.request_id
    ));
    true
}

fn active_job_requires_payment_required_feedback(state: &RenderState) -> bool {
    let Some(job) = state.active_job.job.as_ref() else {
        return false;
    };
    job.stage == JobLifecycleStage::Delivered
        && job.demand_source == crate::app_state::JobDemandSource::OpenNetwork
        && job.quoted_price_sats > 0
        && job.payment_id.is_none()
        && job.invoice_id.is_none()
}

fn queue_active_job_payment_required_feedback(state: &mut RenderState) -> Result<bool, String> {
    if !active_job_requires_payment_required_feedback(state) {
        return Ok(false);
    }
    let Some(job) = state.active_job.job.as_ref() else {
        return Ok(false);
    };
    let request_id = job.request_id.clone();
    let quoted_price_sats = job.quoted_price_sats;

    if let Some(bolt11) = state.active_job.pending_bolt11.clone() {
        let feedback_event_id = queue_nip90_feedback_for_active_job(
            state,
            JobStatus::PaymentRequired,
            "lightning settlement required",
            Some("pay the attached Lightning invoice to settle this result".to_string()),
            true,
            Some(bolt11.as_str()),
        )?;
        if let Some(job) = state.active_job.job.as_mut() {
            job.invoice_id = Some(feedback_event_id.clone());
        }
        state.active_job.payment_required_feedback_in_flight = true;
        state.active_job.payment_required_failed = false;
        state.active_job.append_event(format!(
            "queued canonical NIP-90 payment-required feedback {}",
            feedback_event_id
        ));
        state.provider_runtime.last_result = Some(format!(
            "queued provider payment-required feedback {} for request {}",
            feedback_event_id, request_id
        ));
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider queued payment-required feedback request_id={} feedback_event_id={} quoted_price_sats={}",
            request_id,
            feedback_event_id,
            quoted_price_sats
        );
        return Ok(true);
    }

    if state.active_job.payment_required_invoice_requested {
        return Ok(false);
    }

    super::super::queue_spark_command(
        state,
        crate::spark_wallet::SparkWalletCommand::CreateBolt11Invoice {
            amount_sats: quoted_price_sats,
            description: Some(format!("OpenAgents job {}", request_id)),
            expiry_seconds: Some(3600),
        },
    );
    if let Some(error) = state.spark_wallet.last_error.clone() {
        return Err(format!(
            "provider settlement invoice creation failed: {error}"
        ));
    }

    state.active_job.payment_required_invoice_requested = true;
    state.active_job.payment_required_failed = false;
    state.active_job.append_event(format!(
        "queued Spark BOLT11 invoice creation for {} sats",
        quoted_price_sats
    ));
    state.provider_runtime.last_result = Some(format!(
        "queued provider settlement invoice for request {}",
        request_id
    ));
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider requested Spark invoice request_id={} quoted_price_sats={}",
        request_id,
        quoted_price_sats
    );
    Ok(true)
}

fn sign_event_template(
    identity: &NostrIdentity,
    template: &EventTemplate,
) -> Result<Event, String> {
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign NIP-90 event template: {error}"))
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

fn set_active_job_action_error(state: &mut RenderState, error: impl Into<String>) {
    let error = error.into();
    state.active_job.last_error = Some(error.clone());
    state.active_job.load_state = PaneLoadState::Error;
    state.provider_runtime.last_error_detail = Some(error.clone());
    state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Execution);
    state.provider_runtime.last_result = Some(format!("active job advance blocked: {error}"));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderExecutionBackend {
    Codex,
    Ollama,
    AppleFoundationModels,
}

fn provider_execution_backend_for_kind(request_kind: u16) -> ProviderExecutionBackend {
    if request_kind == KIND_JOB_TEXT_GENERATION {
        ProviderExecutionBackend::Ollama
    } else {
        ProviderExecutionBackend::Codex
    }
}

fn provider_execution_backend_for_active_job(
    state: &RenderState,
) -> Option<ProviderExecutionBackend> {
    let request_kind = state.active_job.job.as_ref()?.request_kind;
    if request_kind == KIND_JOB_TEXT_GENERATION {
        return match state.provider_runtime.active_inference_backend()? {
            LocalInferenceBackend::Ollama => Some(ProviderExecutionBackend::Ollama),
            LocalInferenceBackend::AppleFoundationModels => {
                Some(ProviderExecutionBackend::AppleFoundationModels)
            }
        };
    }
    Some(provider_execution_backend_for_kind(request_kind))
}

fn queue_provider_execution_thread_start(state: &mut RenderState) -> Result<(), String> {
    let Some(ttl_seconds) = state.active_job.job.as_ref().map(|job| job.ttl_seconds) else {
        return Err("Cannot start provider execution without an active job".to_string());
    };
    let cwd = super::super::actions::goal_scoped_turn_cwd(state).or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|value| value.into_os_string().into_string().ok())
    });
    let command = CodexLaneCommand::ThreadStart(codex_client::ThreadStartParams {
        model: state.autopilot_chat.selected_model_override(),
        model_provider: None,
        service_tier: None,
        cwd,
        approval_policy: super::super::actions::cad_turn_approval_policy(false),
        sandbox: super::super::actions::goal_scoped_thread_sandbox_mode(state),
        personality: None,
        ephemeral: None,
        dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
    });
    let seq = state.queue_codex_command(command)?;
    state.active_job.execution_thread_start_command_seq = Some(seq);
    state.active_job.execution_deadline_epoch_seconds =
        Some(current_epoch_seconds().saturating_add(ttl_seconds));
    state
        .active_job
        .append_event(format!("queued provider execution thread start cmd#{seq}"));
    Ok(())
}

fn queue_provider_ollama_execution_start(state: &mut RenderState) -> Result<(), String> {
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot start local inference execution without an active job".to_string());
    };
    let request_id = job.request_id.clone();
    let prompt = job
        .execution_prompt
        .clone()
        .ok_or_else(|| "Active job is missing normalized prompt input".to_string())?;
    let prompt_chars = prompt.chars().count();
    let requested_model = job.requested_model.clone();
    let requested_model_label = requested_model.as_deref().unwrap_or("default").to_string();
    let params = job.execution_params.clone();
    let ttl_seconds = job.ttl_seconds;
    state.queue_local_inference_runtime_command(LocalInferenceRuntimeCommand::Generate(
        LocalInferenceGenerateJob {
            request_id: request_id.clone(),
            prompt,
            requested_model,
            params,
        },
    ))?;
    state.active_job.execution_backend_request_id = Some(request_id.clone());
    state.active_job.execution_deadline_epoch_seconds =
        Some(current_epoch_seconds().saturating_add(ttl_seconds));
    let message = format!("queued local inference generation for {request_id}");
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider queued local inference execution request_id={} requested_model={} prompt_chars={}",
        request_id,
        requested_model_label,
        prompt_chars
    );
    Ok(())
}

fn queue_provider_apple_fm_execution_start(state: &mut RenderState) -> Result<(), String> {
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot start Apple FM execution without an active job".to_string());
    };
    let request_id = job.request_id.clone();
    let prompt = job
        .execution_prompt
        .clone()
        .ok_or_else(|| "Active job is missing normalized prompt input".to_string())?;
    let prompt_chars = prompt.chars().count();
    let requested_model = job.requested_model.clone();
    let requested_model_label = requested_model.as_deref().unwrap_or("default").to_string();
    let ttl_seconds = job.ttl_seconds;
    state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::Generate(AppleFmGenerateJob {
        request_id: request_id.clone(),
        prompt,
        requested_model,
    }))?;
    state.active_job.execution_backend_request_id = Some(request_id.clone());
    state.active_job.execution_deadline_epoch_seconds =
        Some(current_epoch_seconds().saturating_add(ttl_seconds));
    let message = format!("queued Apple Foundation Models generation for {request_id}");
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider queued Apple FM execution request_id={} requested_model={} prompt_chars={}",
        request_id,
        requested_model_label,
        prompt_chars
    );
    Ok(())
}

fn queue_provider_execution_turn_start(
    state: &mut RenderState,
    thread_id: &str,
) -> Result<(), String> {
    let prompt = provider_execution_prompt_for_active_job(state)?;
    let command = CodexLaneCommand::TurnStart(codex_client::TurnStartParams {
        thread_id: thread_id.to_string(),
        input: vec![codex_client::UserInput::Text {
            text: prompt,
            text_elements: Vec::new(),
        }],
        cwd: super::super::actions::goal_scoped_turn_cwd(state).map(std::path::PathBuf::from),
        approval_policy: super::super::actions::cad_turn_approval_policy(false),
        sandbox_policy: super::super::actions::goal_scoped_turn_sandbox_policy(state),
        model: state.autopilot_chat.selected_model_override(),
        service_tier: None,
        effort: None,
        summary: None,
        personality: None,
        output_schema: None,
        collaboration_mode: None,
    });
    let seq = state.queue_codex_command(command)?;
    state.active_job.execution_turn_start_command_seq = Some(seq);
    state
        .active_job
        .append_event(format!("queued provider execution turn start cmd#{seq}"));
    Ok(())
}

fn queue_provider_execution_interrupt(state: &mut RenderState) -> Result<(), String> {
    let Some(thread_id) = state.active_job.execution_thread_id.clone() else {
        return Err("Active job has no provider execution thread".to_string());
    };
    let Some(turn_id) = state.active_job.execution_turn_id.clone() else {
        return Err("Active job has no provider execution turn".to_string());
    };
    if state
        .active_job
        .execution_turn_interrupt_command_seq
        .is_some()
    {
        return Ok(());
    }
    let seq = state.queue_codex_command(CodexLaneCommand::TurnInterrupt(
        codex_client::TurnInterruptParams { thread_id, turn_id },
    ))?;
    state.active_job.execution_turn_interrupt_command_seq = Some(seq);
    state
        .active_job
        .append_event(format!("queued provider execution interrupt cmd#{seq}"));
    Ok(())
}

fn provider_execution_prompt_for_active_job(state: &RenderState) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("No active job selected".to_string());
    };
    let execution_input = job
        .execution_input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Active job is missing normalized execution input".to_string())?;
    Ok(format!(
        "You are Autopilot executing an OpenAgents provider job locally.\n\
Return the best final result for the requester. Do not explain internal policy or hidden chain-of-thought.\n\n\
request_id: {}\n\
capability: {}\n\
demand_source: {}\n\
quoted_price_sats: {}\n\
ttl_seconds: {}\n\n\
job_input:\n{}",
        job.request_id,
        job.capability,
        job.demand_source.label(),
        job.quoted_price_sats,
        job.ttl_seconds,
        execution_input,
    ))
}

fn active_job_matches_execution_thread(state: &RenderState, thread_id: &str) -> bool {
    state.active_job.execution_thread_id.as_deref() == Some(thread_id)
}

fn active_job_matches_execution_turn(state: &RenderState, thread_id: &str, turn_id: &str) -> bool {
    active_job_matches_execution_thread(state, thread_id)
        && state.active_job.execution_turn_id.as_deref() == Some(turn_id)
}

fn active_job_matches_ollama_request(state: &RenderState, request_id: &str) -> bool {
    state.active_job.job.as_ref().is_some_and(|job| {
        job.request_id == request_id
            && state.active_job.execution_backend_request_id.as_deref() == Some(request_id)
    })
}

fn active_job_matches_apple_request(state: &RenderState, request_id: &str) -> bool {
    active_job_matches_ollama_request(state, request_id)
}

pub(super) fn active_job_matches_publish_outcome(
    active_job: &crate::app_state::ActiveJobState,
    outcome: &ProviderNip90PublishOutcome,
) -> bool {
    let Some(job) = active_job.job.as_ref() else {
        return false;
    };
    if job.request_id == outcome.request_id {
        return true;
    }

    match outcome.role {
        ProviderNip90PublishRole::Result => {
            active_job.result_publish_in_flight
                && active_job.pending_result_publish_event_id.as_deref()
                    == Some(outcome.event_id.as_str())
        }
        ProviderNip90PublishRole::Feedback => {
            active_job.payment_required_feedback_in_flight
                && job.invoice_id.as_deref() == Some(outcome.event_id.as_str())
        }
        ProviderNip90PublishRole::Capability | ProviderNip90PublishRole::Request => false,
    }
}

fn store_execution_output(state: &mut RenderState, output: &str) {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return;
    }
    if state.active_job.execution_output.as_deref() == Some(trimmed) {
        return;
    }
    state.active_job.execution_output = Some(trimmed.to_string());
    let chars = trimmed.chars().count();
    let message = format!("captured provider execution output (chars={chars})");
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    if let Some(job) = state.active_job.job.as_ref() {
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider captured execution output request_id={} chars={}",
            job.request_id,
            chars
        );
    }
}

fn apply_ollama_execution_started(
    state: &mut RenderState,
    started: LocalInferenceExecutionStarted,
) -> bool {
    if !active_job_matches_ollama_request(state, started.request_id.as_str()) {
        return false;
    }
    state.provider_runtime.last_result = Some(format!(
        "local inference execution started request={} model={}",
        started.request_id, started.model
    ));
    state.active_job.append_event(format!(
        "local inference generation started with model {}",
        started.model
    ));
    if let Err(error) =
        transition_active_job_to_running(state, "active_job.ollama_execution_started")
    {
        fail_active_job_execution(
            state,
            format!("failed to transition active local inference job to running: {error}"),
            "active_job.ollama_running_transition_failed",
            true,
        );
    }
    true
}

fn apply_ollama_execution_completed(
    state: &mut RenderState,
    completed: LocalInferenceExecutionCompleted,
) -> bool {
    if !active_job_matches_ollama_request(state, completed.request_id.as_str()) {
        return false;
    }
    state.active_job.execution_backend_request_id = None;
    state.active_job.execution_turn_completed = true;
    refresh_active_job_phase_deadline(state);
    store_execution_output(state, completed.output.as_str());
    if let Some(job) = state.active_job.job.as_mut() {
        job.execution_provenance = Some(completed.provenance.clone());
    }
    state.provider_runtime.last_result = Some(format!(
        "local inference execution completed request={} model={}",
        completed.request_id, completed.model
    ));
    state.active_job.append_event(format!(
        "local inference generation completed model={} prompt_eval={} eval={}",
        completed.model,
        completed.metrics.prompt_eval_count.unwrap_or(0),
        completed.metrics.eval_count.unwrap_or(0)
    ));
    true
}

fn apply_ollama_execution_failed(
    state: &mut RenderState,
    failed: LocalInferenceExecutionFailed,
) -> bool {
    if !active_job_matches_ollama_request(state, failed.request_id.as_str()) {
        return false;
    }
    state.active_job.execution_backend_request_id = None;
    fail_active_job_execution(
        state,
        format!("local inference execution failed: {}", failed.error),
        "active_job.ollama_execution_failed",
        true,
    );
    true
}

fn apply_apple_fm_execution_started(
    state: &mut RenderState,
    started: AppleFmExecutionStarted,
) -> bool {
    if !active_job_matches_apple_request(state, started.request_id.as_str()) {
        return false;
    }
    state.provider_runtime.last_result = Some(format!(
        "Apple Foundation Models execution started request={} model={}",
        started.request_id, started.model
    ));
    let message = format!(
        "Apple Foundation Models generation started with model {}",
        started.model
    );
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider observed Apple FM execution start request_id={} model={}",
        started.request_id,
        started.model
    );
    if let Err(error) =
        transition_active_job_to_running(state, "active_job.apple_fm_execution_started")
    {
        fail_active_job_execution(
            state,
            format!("failed to transition Apple FM job to running: {error}"),
            "active_job.apple_fm_running_transition_failed",
            true,
        );
    }
    true
}

fn apply_apple_fm_execution_completed(
    state: &mut RenderState,
    completed: AppleFmExecutionCompleted,
) -> bool {
    if !active_job_matches_apple_request(state, completed.request_id.as_str()) {
        return false;
    }
    state.active_job.execution_backend_request_id = None;
    state.active_job.execution_turn_completed = true;
    refresh_active_job_phase_deadline(state);
    store_execution_output(state, completed.output.as_str());
    if let Some(job) = state.active_job.job.as_mut() {
        job.execution_provenance = Some(completed.provenance.clone());
    }
    state.provider_runtime.last_result = Some(format!(
        "Apple Foundation Models execution completed request={} model={}",
        completed.request_id, completed.model
    ));
    let message = format!(
        "Apple Foundation Models generation completed model={} prompt_eval={} eval={}",
        completed.model,
        completed.metrics.prompt_eval_count.unwrap_or(0),
        completed.metrics.eval_count.unwrap_or(0)
    );
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider observed Apple FM execution completion request_id={} model={} prompt_eval={} eval={}",
        completed.request_id,
        completed.model,
        completed.metrics.prompt_eval_count.unwrap_or(0),
        completed.metrics.eval_count.unwrap_or(0)
    );
    true
}

fn apply_apple_fm_execution_failed(
    state: &mut RenderState,
    failed: AppleFmExecutionFailed,
) -> bool {
    if !active_job_matches_apple_request(state, failed.request_id.as_str()) {
        return false;
    }
    state.active_job.execution_backend_request_id = None;
    tracing::error!(
        target: "autopilot_desktop::provider",
        "Provider observed Apple FM execution failure request_id={} error={}",
        failed.request_id,
        failed.error
    );
    fail_active_job_execution(
        state,
        format!("Apple Foundation Models execution failed: {}", failed.error),
        "active_job.apple_fm_execution_failed",
        true,
    );
    true
}

fn turn_completed_failed(status: Option<&str>, error_message: Option<&str>) -> bool {
    if error_message.is_some_and(|value| !value.trim().is_empty()) {
        return true;
    }
    matches!(
        status
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("error" | "failed" | "cancelled" | "canceled" | "aborted" | "interrupted")
    )
}

fn queue_runtime_result_publish(state: &mut RenderState) -> Result<(), String> {
    let event = state
        .active_job
        .pending_result_publish_event
        .clone()
        .unwrap_or(build_nip90_result_event_for_active_job(state)?);
    let result_event_id = queue_signed_nip90_result_publish_for_active_job(state, event.clone())?;
    state.active_job.result_publish_in_flight = true;
    state.active_job.pending_result_publish_event_id = Some(result_event_id.clone());
    state.active_job.pending_result_publish_event = Some(event);
    state.active_job.result_publish_attempt_count = state
        .active_job
        .result_publish_attempt_count
        .saturating_add(1);
    state.active_job.result_publish_last_queued_epoch_seconds = Some(current_epoch_seconds());
    let attempt = state.active_job.result_publish_attempt_count;
    let continuity_timeout_seconds = state
        .active_job
        .job
        .as_ref()
        .map(|job| active_job_result_publish_continuity_timeout_seconds(job.ttl_seconds));
    if attempt == 1
        && let Some(timeout_seconds) = continuity_timeout_seconds
    {
        extend_active_job_phase_deadline_at_least(
            &mut state.active_job,
            timeout_seconds,
            current_epoch_seconds(),
        );
    }
    let message = if attempt == 1 {
        format!(
            "queued canonical NIP-90 result publish {} (awaiting relay confirmation; window={}s)",
            result_event_id,
            continuity_timeout_seconds.unwrap_or_default()
        )
    } else {
        format!(
            "retried canonical NIP-90 result publish {} attempt #{}",
            result_event_id, attempt
        )
    };
    state.active_job.append_event(message.clone());
    state.active_job.last_action = Some(message);
    state.provider_runtime.last_result = Some(if attempt == 1 {
        format!(
            "queued provider result publish {} (awaiting relay confirmation; window={}s)",
            result_event_id,
            continuity_timeout_seconds.unwrap_or_default()
        )
    } else {
        format!(
            "retried provider result publish {} attempt #{}",
            result_event_id, attempt
        )
    });
    if let Some(job) = state.active_job.job.as_ref() {
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider queued result publish request_id={} event_id={} attempt={}",
            job.request_id,
            result_event_id,
            attempt
        );
    }
    Ok(())
}

fn retry_runtime_result_publish(state: &mut RenderState) -> Result<(), String> {
    if !active_job_has_pending_result_publish(&state.active_job) {
        return Err("no pending result publish is available for retry".to_string());
    }
    queue_runtime_result_publish(state)
}

fn transition_active_job_to_running(
    state: &mut RenderState,
    source: &str,
) -> Result<JobLifecycleStage, String> {
    let stage = match state.active_job.job.as_ref().map(|job| job.stage) {
        Some(JobLifecycleStage::Running) => return Ok(JobLifecycleStage::Running),
        Some(JobLifecycleStage::Accepted) => state.active_job.advance_stage()?,
        Some(other) => {
            return Err(format!(
                "cannot transition active job to running from {}",
                other.label()
            ));
        }
        None => return Err("No active job selected".to_string()),
    };

    state.provider_runtime.last_result = Some("provider execution started".to_string());
    state.provider_runtime.last_authoritative_status = Some("processing".to_string());
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        record_active_job_stage_transition(state, &job, stage, source);
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider job running request_id={} capability={} backend={:?}",
            job.request_id,
            job.capability,
            provider_execution_backend_for_active_job(state)
        );
    }
    match queue_nip90_feedback_for_active_job(
        state,
        JobStatus::Processing,
        "provider execution started",
        Some("execution lane processing".to_string()),
        false,
        None,
    ) {
        Ok(feedback_event_id) => {
            state.active_job.append_event(format!(
                "queued canonical NIP-90 feedback publish {}",
                feedback_event_id
            ));
        }
        Err(error) => {
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Execution);
            state.provider_runtime.last_result = Some(format!(
                "active job running but feedback publish failed: {error}"
            ));
        }
    }
    Ok(stage)
}

fn transition_active_job_to_delivered(
    state: &mut RenderState,
    source: &str,
) -> Result<JobLifecycleStage, String> {
    let stage = match state.active_job.job.as_ref().map(|job| job.stage) {
        Some(JobLifecycleStage::Delivered) => return Ok(JobLifecycleStage::Delivered),
        Some(JobLifecycleStage::Running) => state.active_job.advance_stage()?,
        Some(other) => {
            return Err(format!(
                "cannot transition active job to delivered from {}",
                other.label()
            ));
        }
        None => return Err("No active job selected".to_string()),
    };
    refresh_active_job_phase_deadline(state);
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        let output_bytes = state
            .active_job
            .execution_output
            .as_deref()
            .map_or(0, str::len);
        record_active_job_stage_transition(state, &job, stage, source);
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider job delivered request_id={} capability={} output_bytes={}",
            job.request_id,
            job.capability,
            output_bytes
        );
        match crate::kernel_control::submit_active_job_output(state) {
            Ok(receipt_id) => {
                let event =
                    if crate::kernel_control::is_local_projection_receipt_id(receipt_id.as_str()) {
                        format!("recorded local-only output submission {}", receipt_id)
                    } else {
                        format!(
                            "submitted authoritative kernel output receipt {}",
                            receipt_id
                        )
                    };
                state.active_job.append_event(event);
            }
            Err(error) => {
                state.active_job.append_event(format!(
                    "kernel output submission failed; continuing with delivered state: {}",
                    error
                ));
                state.provider_runtime.last_error_detail = Some(error.clone());
                state.provider_runtime.last_authoritative_error_class =
                    Some(EarnFailureClass::Execution);
                state.provider_runtime.last_result = Some(format!(
                    "provider execution delivered but kernel submission failed: {}",
                    error
                ));
            }
        }
    }
    Ok(stage)
}

pub(super) fn transition_active_job_to_paid(
    state: &mut RenderState,
    source: &str,
    now: std::time::Instant,
) -> Result<JobLifecycleStage, String> {
    if matches!(
        state.active_job.job.as_ref().map(|job| job.stage),
        Some(JobLifecycleStage::Paid)
    ) {
        return Ok(JobLifecycleStage::Paid);
    }
    let verdict_receipt_id = crate::kernel_control::finalize_paid_active_job(state)?;
    let stage = match state.active_job.job.as_ref().map(|job| job.stage) {
        Some(JobLifecycleStage::Delivered) => state.active_job.advance_stage()?,
        Some(other) => {
            return Err(format!(
                "cannot transition active job to paid from {}",
                other.label()
            ));
        }
        None => return Err("No active job selected".to_string()),
    };

    match queue_nip90_feedback_for_active_job(
        state,
        JobStatus::Success,
        "wallet-confirmed settlement recorded",
        Some("execution lane settled".to_string()),
        true,
        None,
    ) {
        Ok(feedback_event_id) => {
            if let Some(job) = state.active_job.job.as_mut() {
                job.ac_settlement_event_id = Some(feedback_event_id.clone());
            }
            state.active_job.append_event(format!(
                "queued canonical NIP-90 success feedback {}",
                feedback_event_id
            ));
        }
        Err(error) => {
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.provider_runtime.last_authoritative_error_class =
                Some(EarnFailureClass::Execution);
            state.provider_runtime.last_result = Some(format!(
                "active job paid but status feedback publish failed: {error}"
            ));
        }
    }

    state.active_job.payment_required_invoice_requested = false;
    state.active_job.payment_required_feedback_in_flight = false;
    state.active_job.payment_required_failed = false;
    state.active_job.pending_bolt11 = None;
    state.active_job.pending_result_publish_event_id = None;
    state.active_job.pending_result_publish_event = None;
    state.active_job.result_publish_attempt_count = 0;
    state.active_job.result_publish_last_queued_epoch_seconds = None;
    clear_active_job_phase_deadline(&mut state.active_job);

    sync_provider_runtime_queue_depth(state);
    state.provider_runtime.last_error_detail = None;
    state.provider_runtime.last_authoritative_error_class = None;
    state.provider_runtime.last_completed_job_at = Some(now);
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider job paid request_id={} capability={} payment_id={}",
            job.request_id,
            job.capability,
            job.payment_id.as_deref().unwrap_or("missing")
        );
        let event =
            if crate::kernel_control::is_local_projection_receipt_id(verdict_receipt_id.as_str()) {
                format!(
                    "recorded local-only settlement verdict {}",
                    verdict_receipt_id
                )
            } else {
                format!(
                    "finalized authoritative kernel verdict receipt {}",
                    verdict_receipt_id
                )
            };
        state.active_job.append_event(event);
        state
            .job_history
            .record_from_active_job(&job, JobHistoryStatus::Succeeded);
        record_active_job_stage_transition(state, &job, stage, source);
    }
    sync_provider_publish_continuity(state);
    finalize_deferred_provider_runtime_shutdown_if_idle(state);
    Ok(stage)
}

fn fail_active_job_execution(
    state: &mut RenderState,
    reason: impl Into<String>,
    source: &str,
    publish_feedback: bool,
) {
    let reason = reason.into();
    let starter_request_id = state.active_job.job.as_ref().and_then(|job| {
        (job.demand_source == crate::app_state::JobDemandSource::StarterDemand)
            .then(|| job.request_id.clone())
    });
    let Some(stage) = state.active_job.job.as_ref().map(|job| job.stage) else {
        return;
    };
    if stage.is_terminal() {
        return;
    }

    if let Err(error) = state
        .active_job
        .mark_failed(reason.as_str(), "Failed active job")
    {
        set_active_job_action_error(state, error);
        return;
    }

    state.active_job.result_publish_in_flight = false;
    state.active_job.pending_result_publish_event_id = None;
    state.active_job.pending_result_publish_event = None;
    state.active_job.result_publish_attempt_count = 0;
    state.active_job.result_publish_last_queued_epoch_seconds = None;
    state.active_job.execution_turn_completed = false;
    clear_active_job_phase_deadline(&mut state.active_job);
    state.active_job.execution_backend_request_id = None;
    state.active_job.execution_thread_start_command_seq = None;
    state.active_job.execution_turn_start_command_seq = None;
    state.active_job.execution_turn_interrupt_command_seq = None;
    state.provider_runtime.last_error_detail = Some(reason.clone());
    state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Execution);
    state.provider_runtime.last_result = Some(format!("active job failed: {reason}"));
    if let Some(job) = state.active_job.job.as_ref() {
        tracing::error!(
            target: "autopilot_desktop::provider",
            "Provider job failed request_id={} capability={} reason={}",
            job.request_id,
            job.capability,
            reason
        );
    }

    if let Some(request_id) = starter_request_id.as_deref() {
        release_hosted_starter_offer_if_configured(state, request_id, reason.as_str());
    }

    if publish_feedback && starter_request_id.is_none() {
        match queue_nip90_feedback_for_active_job(
            state,
            JobStatus::Error,
            "job aborted",
            Some(reason.clone()),
            false,
            None,
        ) {
            Ok(feedback_event_id) => {
                if let Some(job) = state.active_job.job.as_mut() {
                    job.ac_default_event_id = Some(feedback_event_id.clone());
                }
                state.active_job.append_event(format!(
                    "queued canonical NIP-90 error feedback {}",
                    feedback_event_id
                ));
            }
            Err(error) => {
                state.active_job.append_event(format!(
                    "failed to queue canonical NIP-90 error feedback: {error}"
                ));
            }
        }
    }

    sync_provider_runtime_queue_depth(state);
    state.provider_runtime.last_completed_job_at = Some(std::time::Instant::now());
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        state
            .job_history
            .record_from_active_job(&job, JobHistoryStatus::Failed);
        record_active_job_stage_transition(state, &job, JobLifecycleStage::Failed, source);
    }
    sync_provider_publish_continuity(state);
    finalize_deferred_provider_runtime_shutdown_if_idle(state);
}

fn record_active_job_stage_transition(
    state: &mut RenderState,
    job: &crate::app_state::ActiveJobRecord,
    stage: JobLifecycleStage,
    source: &str,
) {
    state.earn_job_lifecycle_projection.record_active_job_stage(
        job,
        stage,
        current_epoch_seconds(),
        source,
    );
    state
        .earn_kernel_receipts
        .record_active_job_stage(job, stage, current_epoch_seconds(), source);
}

fn ack_hosted_starter_offer_if_configured(
    state: &mut RenderState,
    request_id: &str,
) -> Result<crate::starter_demand_client::StarterDemandAckResponse, String> {
    let control_base_url = state
        .hosted_control_base_url
        .clone()
        .ok_or_else(|| "Hosted starter jobs require an OpenAgents control base URL.".to_string())?;
    let bearer_auth = state.hosted_control_bearer_token.clone().ok_or_else(|| {
        "Hosted starter jobs require an authenticated OpenAgents session.".to_string()
    })?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|error| format!("starter demand ack client initialization failed: {error}"))?;
    crate::starter_demand_client::ack_starter_demand_offer_blocking(
        &client,
        control_base_url.as_str(),
        bearer_auth.as_str(),
        request_id,
        state
            .nostr_identity
            .as_ref()
            .map(|identity| identity.npub.as_str()),
    )
}

fn hosted_starter_ack_should_drop_request(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("starter_offer_start_confirm_missed")
        || normalized.contains("starter_offer_not_ackable")
        || normalized.contains("starter_offer_not_found")
}

fn release_hosted_starter_offer_if_configured(
    state: &mut RenderState,
    request_id: &str,
    failure_reason: &str,
) {
    let Some(control_base_url) = state.hosted_control_base_url.clone() else {
        return;
    };
    let Some(bearer_auth) = state.hosted_control_bearer_token.clone() else {
        return;
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            state.active_job.append_event(format!(
                "hosted starter release client initialization failed: {error}"
            ));
            return;
        }
    };
    match crate::starter_demand_client::fail_starter_demand_offer_blocking(
        &client,
        control_base_url.as_str(),
        bearer_auth.as_str(),
        request_id,
        failure_reason,
    ) {
        Ok(response) => {
            state.starter_jobs.mark_released(
                response.request_id.as_str(),
                response.failure_reason.as_str(),
            );
            state.starter_jobs.budget_cap_sats = response.budget_cap_sats;
            state.starter_jobs.budget_allocated_sats = response.budget_allocated_sats;
            state.starter_jobs.next_hosted_sync_due_at = Some(std::time::Instant::now());
        }
        Err(error) => {
            state.active_job.append_event(format!(
                "hosted starter release reconciliation failed: {error}"
            ));
        }
    }
}

fn accept_request_by_id(
    state: &mut RenderState,
    request_id: &str,
    decision_reason: &str,
    source: &str,
) -> Result<(), String> {
    if let Some(reason) = request_accept_block_reason(state, request_id) {
        return Err(reason);
    }

    let preselected_request = state
        .job_inbox
        .requests
        .iter()
        .find(|request| request.request_id == request_id)
        .cloned()
        .ok_or_else(|| "Selected request no longer exists".to_string())?;
    let starter_ack = if preselected_request.demand_source
        == crate::app_state::JobDemandSource::StarterDemand
    {
        match ack_hosted_starter_offer_if_configured(state, request_id) {
            Ok(response) => Some(response),
            Err(error) => {
                if hosted_starter_ack_should_drop_request(error.as_str()) {
                    state
                        .job_inbox
                        .requests
                        .retain(|request| request.request_id != request_id);
                    if state.job_inbox.selected_request_id.as_deref() == Some(request_id) {
                        state.job_inbox.selected_request_id = state
                            .job_inbox
                            .requests
                            .first()
                            .map(|request| request.request_id.clone());
                    }
                    state.starter_jobs.mark_released(request_id, "ack failed");
                    state.starter_jobs.next_hosted_sync_due_at = Some(std::time::Instant::now());
                }
                return Err(error);
            }
        }
    } else {
        None
    };

    state.job_inbox.selected_request_id = Some(request_id.to_string());
    let request_id = match state
        .job_inbox
        .decide_request(request_id, true, decision_reason)
    {
        Ok(request_id) => request_id,
        Err(error) => {
            if preselected_request.demand_source == crate::app_state::JobDemandSource::StarterDemand
            {
                release_hosted_starter_offer_if_configured(
                    state,
                    request_id,
                    "desktop_accept_decision_failed",
                );
            }
            return Err(error);
        }
    };
    hydrate_request_runtime_context(state, request_id.as_str());
    let selected_request = state
        .job_inbox
        .requests
        .iter()
        .find(|request| request.request_id == request_id)
        .cloned()
        .ok_or_else(|| {
            if preselected_request.demand_source == crate::app_state::JobDemandSource::StarterDemand
            {
                release_hosted_starter_offer_if_configured(
                    state,
                    request_id.as_str(),
                    "accepted_request_missing_after_decision",
                );
            }
            "Accepted request no longer exists".to_string()
        })?;

    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider accepting request_id={} capability={} price_sats={} ttl_seconds={} source={}",
        selected_request.request_id,
        selected_request.capability,
        selected_request.price_sats,
        selected_request.ttl_seconds,
        source
    );

    if let Err(error) =
        crate::kernel_control::register_accepted_request_with_kernel(state, &selected_request)
    {
        crate::kernel_control::reset_request_decision_after_kernel_error(
            state,
            selected_request.request_id.as_str(),
            error.as_str(),
        );
        if selected_request.demand_source == crate::app_state::JobDemandSource::StarterDemand {
            release_hosted_starter_offer_if_configured(
                state,
                selected_request.request_id.as_str(),
                "kernel_authority_acceptance_failed",
            );
        }
        return Err(error);
    }

    state.job_inbox.last_error = None;
    state.job_inbox.load_state = PaneLoadState::Ready;
    state.provider_runtime.last_result = Some(format!(
        "{} request {}",
        if source == "job.inbox.auto_accept" {
            "auto-accepted"
        } else {
            "runtime accepted"
        },
        request_id
    ));
    state.active_job.start_from_request(&selected_request);
    sync_provider_publish_continuity(state);
    crate::kernel_control::attach_compute_linkage_to_active_job(state, &selected_request);
    state.active_job.append_event(
        if crate::kernel_control::kernel_authority_available(state) {
            "provisioned authoritative kernel work unit and contract"
        } else {
            "started local-only relay job without hosted kernel authority"
        },
    );
    if let Some(job) = state.active_job.job.as_ref() {
        tracing::info!(
            target: "autopilot_desktop::provider",
            "Provider active job started request_id={} capability={} backend={:?}",
            job.request_id,
            job.capability,
            provider_execution_backend_for_active_job(state)
        );
    }
    if selected_request.demand_source == crate::app_state::JobDemandSource::StarterDemand
        && let Some(starter_ack) = starter_ack.as_ref()
    {
        state.starter_jobs.mark_running(
            request_id.as_str(),
            Some(starter_ack.started_at_unix_ms),
            Some(starter_ack.execution_expires_at_unix_ms),
            Some(starter_ack.last_heartbeat_at_unix_ms),
            Some(starter_ack.next_heartbeat_due_at_unix_ms),
            Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs(starter_ack.heartbeat_interval_seconds.max(1)),
            ),
        );
    }
    sync_provider_runtime_queue_depth(state);
    if let Some(job) = state.active_job.job.as_ref() {
        state.earn_job_lifecycle_projection.record_active_job_stage(
            job,
            JobLifecycleStage::Accepted,
            current_epoch_seconds(),
            source,
        );
        state.earn_kernel_receipts.record_active_job_stage(
            job,
            JobLifecycleStage::Accepted,
            current_epoch_seconds(),
            source,
        );
    }
    let _ = PaneController::create_for_kind(state, PaneKind::ActiveJob);
    Ok(())
}

fn reject_request_by_id(
    state: &mut RenderState,
    request_id: &str,
    decision_reason: &str,
    source: &str,
) -> Result<(), String> {
    state.job_inbox.selected_request_id = Some(request_id.to_string());
    let request_id = state
        .job_inbox
        .decide_request(request_id, false, decision_reason)?;
    state.job_inbox.last_error = None;
    state.job_inbox.load_state = PaneLoadState::Ready;
    state.provider_runtime.last_result = Some(format!("runtime rejected request {request_id}"));
    let rejected_request = state
        .job_inbox
        .requests
        .iter()
        .find(|request| request.request_id == request_id)
        .cloned();
    if let Some(request) = rejected_request.as_ref() {
        state.earn_kernel_receipts.record_preflight_rejection(
            request,
            decision_reason,
            current_epoch_seconds(),
            source,
        );
    }
    Ok(())
}

fn hydrate_request_runtime_context(state: &mut RenderState, request_id: &str) {
    if let Some(request) = state
        .job_inbox
        .requests
        .iter_mut()
        .find(|request| request.request_id == request_id)
    {
        request.skill_scope_id = request.skill_scope_id.clone().or_else(|| {
            state
                .network_requests
                .submitted
                .first()
                .and_then(|submitted| submitted.skill_scope_id.clone())
        });
    }
}

fn request_accept_block_reason(state: &RenderState, request_id: &str) -> Option<String> {
    if let Some(reason) = state
        .job_inbox
        .preview_block_reason(state.provider_runtime.mode)
    {
        return Some(reason.to_string());
    }
    if state.provider_runtime.mode != ProviderMode::Online {
        return Some("Provider must be online before jobs can be accepted".to_string());
    }

    let request = state
        .job_inbox
        .requests
        .iter()
        .find(|request| request.request_id == request_id)?;
    match &request.validation {
        JobInboxValidation::Valid => {}
        JobInboxValidation::Pending => {
            return Some(
                "Request is still pending validation and cannot be accepted yet".to_string(),
            );
        }
        JobInboxValidation::Invalid(reason) => {
            return Some(format!("Request is invalid: {reason}"));
        }
    }

    if request.price_sats < MIN_PROVIDER_PRICE_SATS {
        return Some(format!(
            "Price below provider minimum: {} sats offered, {} sats required",
            request.price_sats, MIN_PROVIDER_PRICE_SATS
        ));
    }
    if request.ttl_seconds < MIN_PROVIDER_TTL_SECONDS {
        return Some(format!(
            "TTL too short for sane execution: {}s offered, {}s required",
            request.ttl_seconds, MIN_PROVIDER_TTL_SECONDS
        ));
    }
    if request.request_kind == KIND_JOB_TEXT_GENERATION {
        match state.provider_runtime.active_inference_backend() {
            Some(LocalInferenceBackend::Ollama) => {
                if let Some(reason) =
                    ollama_request_accept_block_reason(&state.provider_runtime.ollama, request)
                {
                    return Some(reason);
                }
            }
            Some(LocalInferenceBackend::AppleFoundationModels) => {
                if let Some(reason) =
                    apple_fm_request_accept_block_reason(&state.provider_runtime.apple_fm, request)
                {
                    return Some(reason);
                }
            }
            None => {}
        }
    }

    let provider_blockers = state.provider_blockers();
    if !provider_blockers.is_empty() {
        return Some(format!(
            "Provider preflight blocked: {}",
            provider_blockers
                .iter()
                .map(|blocker| format!("{} ({})", blocker.code(), blocker.detail()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let inflight_limit = provider_inflight_limit(state);
    if state.active_job.inflight_job_count() as usize >= inflight_limit {
        return Some(format!(
            "Provider is already at max inflight jobs ({inflight_limit})"
        ));
    }

    None
}

fn next_invalid_request_rejection(state: &RenderState) -> Option<(String, String)> {
    next_invalid_request_rejection_for(state.job_inbox.requests.as_slice())
}

fn next_auto_accept_request_id(state: &RenderState) -> Option<String> {
    if next_auto_accept_request_id_for(
        state.job_inbox.requests.as_slice(),
        state.provider_runtime.mode,
        state.provider_blockers().len(),
        state.active_job.inflight_job_count(),
        provider_inflight_limit(state),
    )
    .is_none()
    {
        return None;
    }

    state.job_inbox.requests.iter().find_map(|request| {
        if !matches!(request.decision, JobInboxDecision::Pending)
            || !matches!(request.validation, JobInboxValidation::Valid)
        {
            return None;
        }
        if request_accept_block_reason(state, request.request_id.as_str()).is_none() {
            Some(request.request_id.clone())
        } else {
            None
        }
    })
}

fn provider_inflight_limit(state: &RenderState) -> usize {
    state.settings.document.provider_max_queue_depth.max(1) as usize
}

fn sync_provider_runtime_queue_depth(state: &mut RenderState) {
    state.provider_runtime.queue_depth = state.active_job.inflight_job_count();
}

fn ollama_request_accept_block_reason(
    ollama: &ProviderOllamaRuntimeState,
    request: &JobInboxRequest,
) -> Option<String> {
    if request.request_kind != KIND_JOB_TEXT_GENERATION {
        return Some(format!(
            "Unsupported request kind {}; local inference serves only kind 5050 text generation",
            request.request_kind
        ));
    }
    if request
        .execution_prompt
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        return Some("text-generation request missing prompt/text input".to_string());
    }
    if let Some(output_mime) = request.requested_output_mime.as_deref()
        && !matches!(output_mime, "text/plain" | "text/markdown")
    {
        return Some(format!(
            "Unsupported output MIME '{}'; provider currently serves text/plain or text/markdown",
            output_mime
        ));
    }
    if !ollama.reachable {
        return Some(
            ollama
                .last_error
                .clone()
                .unwrap_or_else(|| "Local inference backend is unavailable".to_string()),
        );
    }
    if let Some(requested_model) = request.requested_model.as_deref() {
        if !ollama
            .available_models
            .iter()
            .any(|candidate| candidate == requested_model)
        {
            return Some(format!(
                "Requested local model '{}' is not installed locally",
                requested_model
            ));
        }
        if let Some(serving_model) = ollama
            .ready_model
            .as_deref()
            .or(ollama.configured_model.as_deref())
            && requested_model != serving_model
        {
            return Some(format!(
                "Requested local model '{}' is blocked by local policy; provider currently serves '{}'",
                requested_model, serving_model
            ));
        }
    } else if !ollama.is_ready() {
        return Some(
            ollama
                .last_error
                .clone()
                .unwrap_or_else(|| "No local inference model is ready".to_string()),
        );
    }
    None
}

fn apple_fm_request_accept_block_reason(
    apple_fm: &ProviderAppleFmRuntimeState,
    request: &JobInboxRequest,
) -> Option<String> {
    if request.request_kind != KIND_JOB_TEXT_GENERATION {
        return Some(format!(
            "Unsupported request kind {}; Apple Foundation Models provider serves only kind 5050 text generation",
            request.request_kind
        ));
    }
    if request
        .execution_prompt
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        return Some("text-generation request missing prompt/text input".to_string());
    }
    if let Some(output_mime) = request.requested_output_mime.as_deref()
        && !matches!(output_mime, "text/plain" | "text/markdown")
    {
        return Some(format!(
            "Unsupported output MIME '{}'; provider currently serves text/plain or text/markdown",
            output_mime
        ));
    }
    if !apple_fm.reachable {
        return Some(
            apple_fm
                .availability_error_message()
                .unwrap_or_else(|| "Apple Foundation Models backend is unavailable".to_string()),
        );
    }
    if let Some(requested_model) = request.requested_model.as_deref() {
        let serving_model = apple_fm
            .ready_model
            .as_deref()
            .unwrap_or("apple-foundation-model");
        if requested_model != serving_model {
            return Some(format!(
                "Requested Apple Foundation Models model '{}' is blocked by local policy; provider currently serves '{}'",
                requested_model, serving_model
            ));
        }
    } else if !apple_fm.is_ready() {
        return Some(apple_fm.readiness_block_reason().unwrap_or_else(|| {
            "Apple Foundation Models is not ready to serve inference".to_string()
        }));
    }
    None
}

fn next_invalid_request_rejection_for(requests: &[JobInboxRequest]) -> Option<(String, String)> {
    requests.iter().find_map(|request| {
        if !matches!(request.decision, JobInboxDecision::Pending) {
            return None;
        }
        match &request.validation {
            JobInboxValidation::Invalid(reason) => Some((
                request.request_id.clone(),
                format!("auto policy rejected invalid request: {reason}"),
            )),
            JobInboxValidation::Pending | JobInboxValidation::Valid => None,
        }
    })
}

fn next_auto_accept_request_id_for(
    requests: &[JobInboxRequest],
    provider_mode: ProviderMode,
    provider_blocker_count: usize,
    active_inflight_jobs: u32,
    inflight_limit: usize,
) -> Option<String> {
    if provider_mode != ProviderMode::Online
        || provider_blocker_count > 0
        || active_inflight_jobs as usize >= inflight_limit
    {
        return None;
    }

    requests.iter().find_map(|request| {
        if matches!(request.decision, JobInboxDecision::Pending)
            && matches!(request.validation, JobInboxValidation::Valid)
        {
            Some(request.request_id.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderExecutionBackend, active_job_matches_publish_outcome,
        active_job_phase_timeout_seconds, active_job_result_publish_continuity_timeout_seconds,
        active_job_settlement_timeout_seconds, active_job_timeout_reason,
        apple_fm_request_accept_block_reason, apply_payment_required_feedback_publish_outcome,
        build_nip90_feedback_event, clear_active_job_phase_deadline,
        extend_active_job_phase_deadline_at_least, next_auto_accept_request_id_for,
        next_invalid_request_rejection_for, ollama_request_accept_block_reason,
        provider_execution_backend_for_kind, result_publish_retry_due,
        set_active_job_phase_deadline_at, turn_completed_failed,
        visible_result_content_for_job_kind,
    };
    use crate::app_state::{
        ActiveJobState, EarnFailureClass, JobDemandSource, JobInboxDecision, JobInboxRequest,
        JobInboxValidation, JobLifecycleStage, PaneLoadState,
    };
    use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
    use crate::provider_nip90_lane::{ProviderNip90PublishOutcome, ProviderNip90PublishRole};
    use crate::state::provider_runtime::{
        ProviderAppleFmRuntimeState, ProviderMode, ProviderOllamaRuntimeState, ProviderRuntimeState,
    };
    use nostr::{NostrIdentity, nip90::KIND_JOB_TEXT_GENERATION};
    use std::path::PathBuf;

    fn fixture_request(
        request_id: &str,
        validation: JobInboxValidation,
        decision: JobInboxDecision,
    ) -> JobInboxRequest {
        JobInboxRequest {
            request_id: request_id.to_string(),
            requester: "buyer".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "summarize.text".to_string(),
            execution_input: Some(format!("Process request {request_id}")),
            execution_prompt: Some(format!("Prompt for {request_id}")),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: Some("text/plain".to_string()),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(format!("req-event:{request_id}")),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 120,
            ttl_seconds: 60,
            validation,
            arrival_seq: 1,
            decision,
        }
    }

    fn fixture_ollama_runtime() -> ProviderOllamaRuntimeState {
        ProviderOllamaRuntimeState {
            reachable: true,
            configured_model: Some("llama3.2:latest".to_string()),
            ready_model: Some("llama3.2:latest".to_string()),
            available_models: vec!["llama3.2:latest".to_string(), "mistral:latest".to_string()],
            loaded_models: vec!["llama3.2:latest".to_string()],
            last_error: None,
            last_action: Some("Ollama ready".to_string()),
            last_request_id: None,
            last_metrics: None,
            refreshed_at: None,
        }
    }

    fn fixture_apple_fm_runtime() -> ProviderAppleFmRuntimeState {
        ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: true,
            system_model: Default::default(),
            unavailable_reason: None,
            supported_use_cases: vec![],
            supported_guardrails: vec![],
            ready_model: Some("apple-foundation-model".to_string()),
            available_models: vec!["apple-foundation-model".to_string()],
            last_error: None,
            last_action: Some("Apple FM ready".to_string()),
            last_request_id: None,
            last_metrics: None,
            refreshed_at: None,
            availability_message: None,
            bridge_status: Some("running".to_string()),
        }
    }

    fn fixture_nostr_identity() -> NostrIdentity {
        NostrIdentity {
            identity_path: PathBuf::from("/tmp/openagents-provider-nip90-tests.mnemonic"),
            mnemonic: "test test test test test test test test test test test ball".to_string(),
            npub: "npub1providerfeedbacktest".to_string(),
            nsec: "nsec1providerfeedbacktest".to_string(),
            public_key_hex: "02".repeat(32),
            private_key_hex: "11".repeat(32),
        }
    }

    fn fixture_delivered_active_job(request_id: &str) -> ActiveJobState {
        let request = fixture_request(
            request_id,
            JobInboxValidation::Valid,
            JobInboxDecision::Accepted {
                reason: "valid + priced".to_string(),
            },
        );
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("active job should exist");
        job.stage = crate::app_state::JobLifecycleStage::Delivered;
        job.invoice_id = None;
        job.payment_id = None;
        active_job
    }

    fn fixture_running_active_job(request_id: &str) -> ActiveJobState {
        let request = fixture_request(
            request_id,
            JobInboxValidation::Valid,
            JobInboxDecision::Accepted {
                reason: "valid + priced".to_string(),
            },
        );
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("active job should exist");
        job.stage = crate::app_state::JobLifecycleStage::Running;
        active_job.result_publish_in_flight = true;
        active_job
    }

    #[test]
    fn auto_accept_policy_selects_first_valid_pending_request() {
        let requests = vec![
            fixture_request(
                "req-invalid",
                JobInboxValidation::Invalid("missing bid".to_string()),
                JobInboxDecision::Rejected {
                    reason: "invalid".to_string(),
                },
            ),
            fixture_request(
                "req-valid",
                JobInboxValidation::Valid,
                JobInboxDecision::Pending,
            ),
        ];

        assert_eq!(
            next_auto_accept_request_id_for(requests.as_slice(), ProviderMode::Online, 0, 0, 1,),
            Some("req-valid".to_string())
        );
    }

    #[test]
    fn auto_accept_policy_respects_single_inflight_cap() {
        let requests = vec![fixture_request(
            "req-valid",
            JobInboxValidation::Valid,
            JobInboxDecision::Pending,
        )];

        assert_eq!(
            next_auto_accept_request_id_for(requests.as_slice(), ProviderMode::Online, 0, 1, 1,),
            None
        );
    }

    #[test]
    fn invalid_pending_requests_are_selected_for_auto_rejection() {
        let requests = vec![fixture_request(
            "req-invalid",
            JobInboxValidation::Invalid("decrypt failed".to_string()),
            JobInboxDecision::Pending,
        )];

        assert_eq!(
            next_invalid_request_rejection_for(requests.as_slice()),
            Some((
                "req-invalid".to_string(),
                "auto policy rejected invalid request: decrypt failed".to_string()
            ))
        );
    }

    #[test]
    fn turn_completed_failure_detection_treats_errors_and_abort_statuses_as_terminal_failures() {
        assert!(turn_completed_failed(
            Some("error"),
            Some("tool execution failed")
        ));
        assert!(turn_completed_failed(Some("interrupted"), None));
        assert!(!turn_completed_failed(Some("completed"), None));
        assert!(!turn_completed_failed(None, None));
    }

    #[test]
    fn timeout_reason_uses_publish_continuity_wording_after_local_completion() {
        assert_eq!(
            active_job_timeout_reason(JobLifecycleStage::Running, true, 75),
            "job result publish continuity timed out after 195s while awaiting relay delivery confirmation"
        );
    }

    #[test]
    fn timeout_reason_uses_settlement_wording_after_delivery() {
        assert_eq!(
            active_job_timeout_reason(JobLifecycleStage::Delivered, true, 75),
            "job settlement timed out after 195s while awaiting payment flow"
        );
    }

    #[test]
    fn result_publish_continuity_timeout_adds_relay_grace_window() {
        assert_eq!(
            active_job_result_publish_continuity_timeout_seconds(60),
            180
        );
        assert_eq!(
            active_job_result_publish_continuity_timeout_seconds(75),
            195
        );
        assert_eq!(
            active_job_result_publish_continuity_timeout_seconds(300),
            420
        );
    }

    #[test]
    fn phase_timeout_uses_publish_continuity_after_local_completion() {
        assert_eq!(
            active_job_phase_timeout_seconds(JobLifecycleStage::Running, true, 75),
            195
        );
        assert_eq!(
            active_job_phase_timeout_seconds(JobLifecycleStage::Running, false, 75),
            75
        );
    }

    #[test]
    fn settlement_timeout_adds_payment_grace_window() {
        assert_eq!(active_job_settlement_timeout_seconds(60), 180);
        assert_eq!(active_job_settlement_timeout_seconds(75), 195);
        assert_eq!(active_job_settlement_timeout_seconds(300), 420);
    }

    #[test]
    fn active_job_phase_deadline_refresh_replaces_existing_deadline() {
        let mut active_job = ActiveJobState::default();
        active_job.execution_deadline_epoch_seconds = Some(5);

        set_active_job_phase_deadline_at(&mut active_job, 75, 1_700_000_000);

        assert_eq!(
            active_job.execution_deadline_epoch_seconds,
            Some(1_700_000_075)
        );
    }

    #[test]
    fn active_job_phase_deadline_can_be_cleared() {
        let mut active_job = ActiveJobState::default();
        active_job.execution_deadline_epoch_seconds = Some(1_700_000_075);

        clear_active_job_phase_deadline(&mut active_job);

        assert_eq!(active_job.execution_deadline_epoch_seconds, None);
    }

    #[test]
    fn active_job_phase_deadline_extension_never_shrinks_existing_deadline() {
        let mut active_job = ActiveJobState::default();
        active_job.execution_deadline_epoch_seconds = Some(1_700_000_075);

        extend_active_job_phase_deadline_at_least(&mut active_job, 15, 1_700_000_010);

        assert_eq!(
            active_job.execution_deadline_epoch_seconds,
            Some(1_700_000_075)
        );
    }

    #[test]
    fn result_publish_retry_due_after_retry_interval() {
        assert!(!result_publish_retry_due(
            Some(1_700_000_000),
            1_700_000_004
        ));
        assert!(result_publish_retry_due(Some(1_700_000_000), 1_700_000_005));
    }

    #[test]
    fn text_generation_results_publish_plain_text_content() {
        let content =
            visible_result_content_for_job_kind(KIND_JOB_TEXT_GENERATION, "  hello world  ");
        assert_eq!(content, "hello world");

        let non_text_content = visible_result_content_for_job_kind(5999, "  ok  ");
        assert_eq!(non_text_content, r#"{"output":"ok","status":"completed"}"#);
    }

    #[test]
    fn text_generation_jobs_route_to_ollama_backend() {
        assert_eq!(
            provider_execution_backend_for_kind(KIND_JOB_TEXT_GENERATION),
            ProviderExecutionBackend::Ollama
        );
        assert_eq!(
            provider_execution_backend_for_kind(5999),
            ProviderExecutionBackend::Codex
        );
    }

    #[test]
    fn ollama_snapshot_needs_ready_model_before_serving() {
        let snapshot = LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: None,
            last_error: Some("No local Ollama text-generation models are installed".to_string()),
            ..LocalInferenceExecutionSnapshot::default()
        };
        assert!(!snapshot.is_ready());
    }

    #[test]
    fn ollama_rejects_requested_model_missing_locally() {
        let ollama = fixture_ollama_runtime();
        let mut request = fixture_request(
            "req-model-missing",
            JobInboxValidation::Valid,
            JobInboxDecision::Pending,
        );
        request.requested_model = Some("phi4:latest".to_string());

        assert_eq!(
            ollama_request_accept_block_reason(&ollama, &request),
            Some("Requested Ollama model 'phi4:latest' is not installed locally".to_string())
        );
    }

    #[test]
    fn ollama_rejects_requested_model_blocked_by_local_policy() {
        let ollama = fixture_ollama_runtime();
        let mut request = fixture_request(
            "req-model-blocked",
            JobInboxValidation::Valid,
            JobInboxDecision::Pending,
        );
        request.requested_model = Some("mistral:latest".to_string());

        assert_eq!(
            ollama_request_accept_block_reason(&ollama, &request),
            Some(
                "Requested Ollama model 'mistral:latest' is blocked by local policy; provider currently serves 'llama3.2:latest'"
                    .to_string()
            )
        );
    }

    #[test]
    fn ollama_rejects_unsupported_output_mime() {
        let ollama = fixture_ollama_runtime();
        let mut request = fixture_request(
            "req-output",
            JobInboxValidation::Valid,
            JobInboxDecision::Pending,
        );
        request.requested_output_mime = Some("application/json".to_string());

        assert_eq!(
            ollama_request_accept_block_reason(&ollama, &request),
            Some(
                "Unsupported output MIME 'application/json'; provider currently serves text/plain or text/markdown"
                    .to_string()
            )
        );
    }

    #[test]
    fn apple_fm_rejects_requested_model_mismatch() {
        let apple_fm = fixture_apple_fm_runtime();
        let mut request = fixture_request(
            "req-apple-model",
            JobInboxValidation::Valid,
            JobInboxDecision::Pending,
        );
        request.requested_model = Some("llama3.2:latest".to_string());

        assert_eq!(
            apple_fm_request_accept_block_reason(&apple_fm, &request),
            Some(
                "Requested Apple Foundation Models model 'llama3.2:latest' is blocked by local policy; provider currently serves 'apple-foundation-model'"
                    .to_string()
            )
        );
    }

    #[test]
    fn payment_required_feedback_event_includes_amount_and_bolt11() {
        let mut active_job = fixture_delivered_active_job("req-pay-required-shape");
        if let Some(job) = active_job.job.as_mut() {
            job.quoted_price_sats = 2;
        }

        let event = build_nip90_feedback_event(
            &fixture_nostr_identity(),
            "req-pay-required-shape",
            "buyer",
            2,
            nostr::nip90::JobStatus::PaymentRequired,
            "lightning settlement required",
            Some("pay the attached Lightning invoice to settle this result".to_string()),
            true,
            Some("lnbc20n1providerfeedback"),
        )
        .expect("feedback event should build");

        assert_eq!(event.kind, nostr::nip90::KIND_JOB_FEEDBACK);
        assert!(event.tags.iter().any(|tag| {
            tag.first().map(String::as_str) == Some("status")
                && tag.get(1).map(String::as_str) == Some("payment-required")
                && tag.get(2).map(String::as_str) == Some("lightning settlement required")
        }));
        assert!(event.tags.iter().any(|tag| {
            tag.first().map(String::as_str) == Some("amount")
                && tag.get(1).map(String::as_str) == Some("2000")
                && tag.get(2).map(String::as_str) == Some("lnbc20n1providerfeedback")
        }));
        assert_eq!(
            event.content,
            "pay the attached Lightning invoice to settle this result"
        );
    }

    #[test]
    fn payment_required_feedback_publish_failure_marks_payment_error() {
        let mut active_job = fixture_delivered_active_job("req-pay-required-fail");
        let mut provider_runtime = ProviderRuntimeState::default();
        active_job.payment_required_feedback_in_flight = true;
        if let Some(job) = active_job.job.as_mut() {
            job.invoice_id = Some("feedback-event-001".to_string());
        }

        assert!(apply_payment_required_feedback_publish_outcome(
            &mut active_job,
            &mut provider_runtime,
            &ProviderNip90PublishOutcome {
                request_id: "req-pay-required-fail".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event_id: "feedback-event-001".to_string(),
                accepted_relays: 0,
                rejected_relays: 2,
                first_error: Some("relay write failed".to_string()),
                parsed_event_shape: None,
                raw_event_json: None,
            },
        ));

        assert!(!active_job.payment_required_feedback_in_flight);
        assert!(active_job.payment_required_failed);
        assert_eq!(
            active_job
                .job
                .as_ref()
                .and_then(|job| job.invoice_id.as_deref()),
            None
        );
        assert_eq!(
            active_job.last_error.as_deref(),
            Some("payment-required feedback publish failed (relay write failed)")
        );
        assert_eq!(active_job.load_state, PaneLoadState::Error);
        assert_eq!(
            provider_runtime.last_authoritative_error_class,
            Some(EarnFailureClass::Payment)
        );
    }

    #[test]
    fn payment_required_feedback_publish_success_extends_settlement_deadline() {
        let mut active_job = fixture_delivered_active_job("req-pay-required-ok");
        let mut provider_runtime = ProviderRuntimeState::default();
        active_job.payment_required_feedback_in_flight = true;
        active_job.execution_deadline_epoch_seconds = Some(1_700_000_100);
        active_job.last_error = Some("stale wait error".to_string());
        active_job.load_state = PaneLoadState::Error;
        if let Some(job) = active_job.job.as_mut() {
            job.invoice_id = Some("feedback-event-002".to_string());
            job.ttl_seconds = 75;
        }

        assert!(apply_payment_required_feedback_publish_outcome(
            &mut active_job,
            &mut provider_runtime,
            &ProviderNip90PublishOutcome {
                request_id: "req-pay-required-ok".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event_id: "feedback-event-002".to_string(),
                accepted_relays: 3,
                rejected_relays: 1,
                first_error: None,
                parsed_event_shape: None,
                raw_event_json: None,
            },
        ));

        assert!(!active_job.payment_required_feedback_in_flight);
        assert!(!active_job.payment_required_failed);
        assert_eq!(active_job.last_error, None);
        assert_eq!(active_job.load_state, PaneLoadState::Ready);
        assert_eq!(
            active_job.last_action.as_deref(),
            Some("Awaiting Lightning settlement after publishing feedback-event-002")
        );
        assert!(
            active_job
                .execution_deadline_epoch_seconds
                .is_some_and(|deadline| deadline >= 1_700_000_195)
        );
    }

    #[test]
    fn publish_outcome_matches_pending_result_event_when_request_id_drifts() {
        let mut active_job = fixture_running_active_job("req-result-drift");
        active_job.pending_result_publish_event_id = Some("result-event-001".to_string());

        assert!(active_job_matches_publish_outcome(
            &active_job,
            &ProviderNip90PublishOutcome {
                request_id: "req-other".to_string(),
                role: ProviderNip90PublishRole::Result,
                event_id: "result-event-001".to_string(),
                accepted_relays: 1,
                rejected_relays: 0,
                first_error: None,
                parsed_event_shape: None,
                raw_event_json: None,
            },
        ));
    }

    #[test]
    fn publish_outcome_matches_pending_feedback_event_when_request_id_drifts() {
        let mut active_job = fixture_delivered_active_job("req-feedback-drift");
        active_job.payment_required_feedback_in_flight = true;
        if let Some(job) = active_job.job.as_mut() {
            job.invoice_id = Some("feedback-event-001".to_string());
        }

        assert!(active_job_matches_publish_outcome(
            &active_job,
            &ProviderNip90PublishOutcome {
                request_id: "req-other".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event_id: "feedback-event-001".to_string(),
                accepted_relays: 1,
                rejected_relays: 0,
                first_error: None,
                parsed_event_shape: None,
                raw_event_json: None,
            },
        ));
    }
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
