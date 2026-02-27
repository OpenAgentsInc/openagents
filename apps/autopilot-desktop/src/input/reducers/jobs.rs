use crate::app_state::{JobHistoryStatus, JobLifecycleStage, PaneKind, PaneLoadState, RenderState};
use crate::pane_system::{
    ActiveJobPaneAction, JobHistoryPaneAction, JobInboxPaneAction, PaneController,
};

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
                        request.sa_tick_request_event_id = request
                            .sa_tick_request_event_id
                            .clone()
                            .or_else(|| state.sa_lane.last_tick_request_event_id.clone());
                        request.sa_tick_result_event_id = request
                            .sa_tick_result_event_id
                            .clone()
                            .or_else(|| state.sa_lane.last_tick_result_event_id.clone());
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
            if let Ok(stage) = state.active_job.advance_stage() {
                state.provider_runtime.last_result =
                    Some(format!("active job advanced to {}", stage.label()));
                if stage == JobLifecycleStage::Paid {
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_sub(1);
                    state.provider_runtime.last_completed_job_at = Some(now);
                    if let Some(job) = state.active_job.job.as_ref() {
                        state
                            .job_history
                            .record_from_active_job(job, JobHistoryStatus::Succeeded);
                    }
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
                state.provider_runtime.last_result = Some("active job aborted".to_string());
                state.provider_runtime.queue_depth =
                    state.provider_runtime.queue_depth.saturating_sub(1);
                state.provider_runtime.last_completed_job_at = Some(now);
                if let Some(job) = state.active_job.job.as_ref() {
                    state
                        .job_history
                        .record_from_active_job(job, JobHistoryStatus::Failed);
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
