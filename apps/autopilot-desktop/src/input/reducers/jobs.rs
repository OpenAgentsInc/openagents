use crate::app_state::{
    EarnFailureClass, JobHistoryStatus, JobInboxDecision, JobInboxRequest, JobInboxValidation,
    JobLifecycleStage, PaneKind, PaneLoadState, ProviderMode, RenderState,
};
use crate::codex_lane::{
    CodexLaneCommand, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneNotification,
};
use crate::pane_system::{
    ActiveJobPaneAction, JobHistoryPaneAction, JobInboxPaneAction, PaneController,
};
use crate::provider_nip90_lane::{
    ProviderNip90LaneCommand, ProviderNip90PublishOutcome, ProviderNip90PublishRole,
};
use nostr::nip90::{
    JobFeedback, JobResult, JobStatus, create_job_feedback_event, create_job_result_event,
};
use nostr::{Event, EventTemplate, NostrIdentity};

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
    if state
        .active_job
        .execution_deadline_epoch_seconds
        .is_some_and(|deadline| now_epoch_seconds > deadline)
    {
        fail_active_job_execution(
            state,
            format!("job execution timed out after {}s", ttl_seconds),
            "active_job.execution_timeout",
            true,
        );
        return true;
    }

    if stage == JobLifecycleStage::Accepted && state.active_job.execution_thread_id.is_none() {
        if state
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

    if stage == JobLifecycleStage::Running
        && state.active_job.execution_turn_completed
        && !has_result_event
        && !state.active_job.result_publish_in_flight
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
        CodexLaneNotification::ThreadStarted { thread_id } => {
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
    let Some(job) = state.active_job.job.as_ref() else {
        return;
    };
    if job.request_id != outcome.request_id {
        return;
    }

    match outcome.role {
        ProviderNip90PublishRole::Result => {
            state.active_job.result_publish_in_flight = false;
            if outcome.accepted_relays == 0 {
                state.active_job.append_event(format!(
                    "result publish failed; waiting retry ({})",
                    outcome
                        .first_error
                        .as_deref()
                        .unwrap_or("all relays rejected publish")
                ));
                return;
            }
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

fn queue_nip90_result_publish_for_active_job(state: &mut RenderState) -> Result<String, String> {
    let Some(identity) = state.nostr_identity.as_ref() else {
        return Err("Cannot publish result: Nostr identity unavailable".to_string());
    };
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot publish result: no active job selected".to_string());
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
    let payload = serde_json::json!({
        "request_id": request_id,
        "job_id": job.job_id,
        "capability": job.capability,
        "demand_source": job.demand_source.label(),
        "status": "completed",
        "source": "desktop.execution.lane",
        "input": job.execution_input.clone(),
        "output": execution_output,
        "provider_thread_id": state.active_job.execution_thread_id.clone(),
        "provider_turn_id": state.active_job.execution_turn_id.clone()
    })
    .to_string();

    let mut result = JobResult::new(request_kind, request_id.clone(), requester, payload)
        .map_err(|error| format!("Cannot build NIP-90 result event: {error}"))?;
    if quoted_price_sats > 0 {
        result = result.with_amount(quoted_price_sats.saturating_mul(1000), None);
    }
    let template = create_job_result_event(&result);
    let event = sign_event_template(identity, &template)?;
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

fn queue_nip90_feedback_for_active_job(
    state: &mut RenderState,
    status: JobStatus,
    status_extra: impl Into<String>,
    content: Option<String>,
    include_amount: bool,
) -> Result<String, String> {
    let Some(identity) = state.nostr_identity.as_ref() else {
        return Err("Cannot publish feedback: Nostr identity unavailable".to_string());
    };
    let Some(job) = state.active_job.job.as_ref() else {
        return Err("Cannot publish feedback: no active job selected".to_string());
    };

    let request_id = job.request_id.clone();
    let requester = job.requester.clone();
    let quoted_price_sats = job.quoted_price_sats;
    let mut feedback = JobFeedback::new(status, request_id.clone(), requester)
        .with_status_extra(status_extra.into());
    if let Some(content) = content {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            feedback = feedback.with_content(trimmed.to_string());
        }
    }
    if include_amount && quoted_price_sats > 0 {
        feedback = feedback.with_amount(quoted_price_sats.saturating_mul(1000), None);
    }
    let template = create_job_feedback_event(&feedback);
    let event = sign_event_template(identity, &template)?;
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
        cwd,
        approval_policy: super::super::actions::cad_turn_approval_policy(false),
        sandbox: super::super::actions::goal_scoped_thread_sandbox_mode(state),
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

fn store_execution_output(state: &mut RenderState, output: &str) {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return;
    }
    if state.active_job.execution_output.as_deref() == Some(trimmed) {
        return;
    }
    state.active_job.execution_output = Some(trimmed.to_string());
    state.active_job.append_event(format!(
        "captured provider execution output (chars={})",
        trimmed.chars().count()
    ));
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
    let result_event_id = queue_nip90_result_publish_for_active_job(state)?;
    state.active_job.result_publish_in_flight = true;
    state.active_job.append_event(format!(
        "queued canonical NIP-90 result publish {}",
        result_event_id
    ));
    state.provider_runtime.last_result = Some(format!(
        "queued provider result publish {}",
        result_event_id
    ));
    Ok(())
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
    }
    match queue_nip90_feedback_for_active_job(
        state,
        JobStatus::Processing,
        "provider execution started",
        Some("execution lane processing".to_string()),
        false,
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
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        record_active_job_stage_transition(state, &job, stage, source);
    }
    Ok(stage)
}

pub(super) fn transition_active_job_to_paid(
    state: &mut RenderState,
    source: &str,
    now: std::time::Instant,
) -> Result<JobLifecycleStage, String> {
    let stage = match state.active_job.job.as_ref().map(|job| job.stage) {
        Some(JobLifecycleStage::Paid) => return Ok(JobLifecycleStage::Paid),
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

    sync_provider_runtime_queue_depth(state);
    state.provider_runtime.last_completed_job_at = Some(now);
    if let Some(job) = state.active_job.job.as_ref().cloned() {
        state
            .job_history
            .record_from_active_job(&job, JobHistoryStatus::Succeeded);
        record_active_job_stage_transition(state, &job, stage, source);
    }
    Ok(stage)
}

fn fail_active_job_execution(
    state: &mut RenderState,
    reason: impl Into<String>,
    source: &str,
    publish_feedback: bool,
) {
    let reason = reason.into();
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
    state.active_job.execution_turn_completed = false;
    state.active_job.execution_thread_start_command_seq = None;
    state.active_job.execution_turn_start_command_seq = None;
    state.active_job.execution_turn_interrupt_command_seq = None;
    state.provider_runtime.last_error_detail = Some(reason.clone());
    state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Execution);
    state.provider_runtime.last_result = Some(format!("active job failed: {reason}"));

    if publish_feedback {
        match queue_nip90_feedback_for_active_job(
            state,
            JobStatus::Error,
            "job aborted",
            Some(reason.clone()),
            false,
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

fn accept_request_by_id(
    state: &mut RenderState,
    request_id: &str,
    decision_reason: &str,
    source: &str,
) -> Result<(), String> {
    if let Some(reason) = request_accept_block_reason(state, request_id) {
        return Err(reason);
    }

    state.job_inbox.selected_request_id = Some(request_id.to_string());
    let request_id = state
        .job_inbox
        .decide_request(request_id, true, decision_reason)?;
    hydrate_request_runtime_context(state, request_id.as_str());
    let selected_request = state
        .job_inbox
        .requests
        .iter()
        .find(|request| request.request_id == request_id)
        .cloned()
        .ok_or_else(|| "Accepted request no longer exists".to_string())?;

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
        request.skl_manifest_a = request
            .skl_manifest_a
            .clone()
            .or_else(|| state.skl_lane.manifest_a.clone());
        request.skl_manifest_event_id = request
            .skl_manifest_event_id
            .clone()
            .or_else(|| state.skl_lane.manifest_event_id.clone());
        request.ac_envelope_event_id = request
            .ac_envelope_event_id
            .clone()
            .or_else(|| state.ac_lane.envelope_event_id.clone());
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

    let provider_blockers = state.provider_blockers();
    if !provider_blockers.is_empty() {
        return Some(format!(
            "Provider preflight blocked: {}",
            provider_blockers
                .iter()
                .map(|blocker| blocker.code())
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
    next_auto_accept_request_id_for(
        state.job_inbox.requests.as_slice(),
        state.provider_runtime.mode,
        state.provider_blockers().len(),
        state.active_job.inflight_job_count(),
        provider_inflight_limit(state),
    )
}

fn provider_inflight_limit(state: &RenderState) -> usize {
    state.settings.document.provider_max_queue_depth.max(1) as usize
}

fn sync_provider_runtime_queue_depth(state: &mut RenderState) {
    state.provider_runtime.queue_depth = state.active_job.inflight_job_count();
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
        next_auto_accept_request_id_for, next_invalid_request_rejection_for, turn_completed_failed,
    };
    use crate::app_state::{
        JobDemandSource, JobInboxDecision, JobInboxRequest, JobInboxValidation,
    };
    use crate::state::provider_runtime::ProviderMode;

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
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
