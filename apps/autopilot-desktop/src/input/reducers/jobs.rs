use crate::app_state::{
    EarnFailureClass, JobHistoryStatus, JobLifecycleStage, PaneKind, PaneLoadState, RenderState,
};
use crate::pane_system::{
    ActiveJobPaneAction, JobHistoryPaneAction, JobInboxPaneAction, PaneController,
};
use crate::provider_nip90_lane::{ProviderNip90LaneCommand, ProviderNip90PublishRole};
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
            match state
                .job_inbox
                .decide_selected(true, "validated + queued for runtime")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = PaneLoadState::Ready;
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_add(1);
                    state.provider_runtime.last_result =
                        Some(format!("runtime accepted request {request_id}"));
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
                    let selected_request = state
                        .job_inbox
                        .requests
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .cloned();
                    if let Some(request) = selected_request.as_ref() {
                        state.active_job.start_from_request(request);
                        if let Some(job) = state.active_job.job.as_ref() {
                            state.earn_job_lifecycle_projection.record_active_job_stage(
                                job,
                                JobLifecycleStage::Accepted,
                                current_epoch_seconds(),
                                "job.inbox.accept",
                            );
                            state.earn_kernel_receipts.record_active_job_stage(
                                job,
                                JobLifecycleStage::Accepted,
                                current_epoch_seconds(),
                                "job.inbox.accept",
                            );
                        }
                        let _ = PaneController::create_for_kind(state, PaneKind::ActiveJob);
                    }
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        JobInboxPaneAction::RejectSelected => {
            match state
                .job_inbox
                .decide_selected(false, "failed policy preflight")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = PaneLoadState::Ready;
                    state.provider_runtime.last_result =
                        Some(format!("runtime rejected request {request_id}"));
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

pub(super) fn run_active_job_action(state: &mut RenderState, action: ActiveJobPaneAction) -> bool {
    let now = std::time::Instant::now();
    match action {
        ActiveJobPaneAction::AdvanceStage => {
            let should_publish_result = state.active_job.job.as_ref().is_some_and(|job| {
                job.stage == JobLifecycleStage::Running && job.sa_tick_result_event_id.is_none()
            });
            if should_publish_result {
                match queue_nip90_result_publish_for_active_job(state) {
                    Ok(result_event_id) => {
                        if let Some(job) = state.active_job.job.as_mut() {
                            job.sa_tick_result_event_id = Some(result_event_id.clone());
                        }
                        state.active_job.append_event(format!(
                            "queued canonical NIP-90 result publish {}",
                            result_event_id
                        ));
                    }
                    Err(error) => {
                        set_active_job_action_error(state, error);
                        super::super::refresh_earnings_scoreboard(state, now);
                        return true;
                    }
                }
            }

            if let Ok(stage) = state.active_job.advance_stage() {
                state.provider_runtime.last_result =
                    Some(format!("active job advanced to {}", stage.label()));

                if stage == JobLifecycleStage::Running {
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
                }

                if stage == JobLifecycleStage::Paid {
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

                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_sub(1);
                    state.provider_runtime.last_completed_job_at = Some(now);
                    if let Some(job) = state.active_job.job.as_ref() {
                        state
                            .job_history
                            .record_from_active_job(job, JobHistoryStatus::Succeeded);
                    }
                }
                if let Some(job) = state.active_job.job.as_ref() {
                    state.earn_job_lifecycle_projection.record_active_job_stage(
                        job,
                        stage,
                        current_epoch_seconds(),
                        "active_job.advance_stage",
                    );
                    state.earn_kernel_receipts.record_active_job_stage(
                        job,
                        stage,
                        current_epoch_seconds(),
                        "active_job.advance_stage",
                    );
                }
            }
            super::super::refresh_earnings_scoreboard(state, now);
            true
        }
        ActiveJobPaneAction::AbortJob => {
            if state
                .active_job
                .abort_job("operator requested abort")
                .is_ok()
            {
                let failure_reason = state
                    .active_job
                    .job
                    .as_ref()
                    .and_then(|job| job.failure_reason.clone())
                    .unwrap_or_else(|| "operator requested abort".to_string());
                match queue_nip90_feedback_for_active_job(
                    state,
                    JobStatus::Error,
                    "job aborted",
                    Some(failure_reason),
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
                        state.provider_runtime.last_error_detail = Some(error.clone());
                        state.provider_runtime.last_authoritative_error_class =
                            Some(EarnFailureClass::Execution);
                        state.provider_runtime.last_result = Some(format!(
                            "active job aborted; feedback publish failed: {error}"
                        ));
                    }
                }

                state.provider_runtime.last_result = Some("active job aborted".to_string());
                state.provider_runtime.queue_depth =
                    state.provider_runtime.queue_depth.saturating_sub(1);
                state.provider_runtime.last_completed_job_at = Some(now);
                if let Some(job) = state.active_job.job.as_ref() {
                    state
                        .job_history
                        .record_from_active_job(job, JobHistoryStatus::Failed);
                    state.earn_job_lifecycle_projection.record_active_job_stage(
                        job,
                        JobLifecycleStage::Failed,
                        current_epoch_seconds(),
                        "active_job.abort",
                    );
                    state.earn_kernel_receipts.record_active_job_stage(
                        job,
                        JobLifecycleStage::Failed,
                        current_epoch_seconds(),
                        "active_job.abort",
                    );
                }
            }
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
    let payload = serde_json::json!({
        "request_id": request_id,
        "job_id": job.job_id,
        "capability": job.capability,
        "demand_source": job.demand_source.label(),
        "status": "completed",
        "source": "desktop.execution.lane"
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

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
