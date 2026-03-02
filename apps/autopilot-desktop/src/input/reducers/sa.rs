use crate::app_state::{PaneLoadState, ProviderMode, RenderState};
use crate::pane_system::{
    AgentProfileStatePaneAction, AgentScheduleTickPaneAction, TrajectoryAuditPaneAction,
};
use crate::runtime_lanes::{
    RuntimeCommandResponse, RuntimeCommandStatus, SaLaneSnapshot, SaLifecycleCommand, SaRunnerMode,
};
use crate::state::autopilot_goals::{
    GoalConstraints, GoalLifecycleEvent, GoalLifecycleStatus, GoalMissedRunPolicy, GoalObjective,
    GoalRecord, GoalRetryPolicy, GoalScheduleConfig, GoalStopCondition,
};
use crate::state::cron_schedule::{next_cron_run_epoch_seconds, parse_cron_expression};
use crate::state::goal_loop_executor::{GoalLoopStopReason, select_runnable_goal};
use crate::state::os_scheduler::{OsSchedulerAdapterKind, preferred_adapter_for_host};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: SaLaneSnapshot) {
    state.provider_runtime.mode = match snapshot.mode {
        SaRunnerMode::Offline => ProviderMode::Offline,
        SaRunnerMode::Connecting => ProviderMode::Connecting,
        SaRunnerMode::Online => ProviderMode::Online,
    };
    state.provider_runtime.mode_changed_at = snapshot.mode_changed_at;
    state.provider_runtime.connecting_until = snapshot.connect_until;
    state.provider_runtime.online_since = snapshot.online_since;
    state.provider_runtime.last_heartbeat_at = snapshot.last_heartbeat_at;
    state.provider_runtime.heartbeat_interval =
        std::time::Duration::from_secs(snapshot.heartbeat_seconds.max(1));
    state.provider_runtime.queue_depth = snapshot.queue_depth;
    state.provider_runtime.last_result = snapshot.last_result.clone();
    state.provider_runtime.degraded_reason_code = snapshot.degraded_reason_code.clone();
    state.provider_runtime.last_error_detail = snapshot.last_error_detail.clone();
    state.sa_lane = snapshot;
    sync_agent_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.provider_runtime.last_result = Some(summary.to_string());
    state.provider_runtime.last_authoritative_status = Some(response.status.label().to_string());
    state
        .provider_runtime
        .last_authoritative_event_id
        .clone_from(&response.event_id);
    state.provider_runtime.last_authoritative_error_class = response
        .error
        .as_ref()
        .map(|error| error.class.label().to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        state.provider_runtime.last_error_detail = response
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .or_else(|| Some("SA lane command rejected".to_string()));
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = response
            .error
            .as_ref()
            .map(|error| format!("SA_{}", error.class.label().to_ascii_uppercase()))
            .or_else(|| Some("SA_COMMAND_REJECTED".to_string()));
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
        let error = response.error.as_ref().map_or_else(
            || "SA lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.agent_profile_state.last_error = Some(error.clone());
        state.agent_profile_state.load_state = PaneLoadState::Error;
        state.agent_schedule_tick.last_error = Some(error.clone());
        state.agent_schedule_tick.load_state = PaneLoadState::Error;
        state.trajectory_audit.last_error = Some(error);
        state.trajectory_audit.load_state = PaneLoadState::Error;
    }
}

fn sync_agent_pane_snapshots(state: &mut RenderState) {
    state.agent_profile_state.profile_event_id = state.sa_lane.profile_event_id.clone();
    state.agent_profile_state.state_event_id = state.sa_lane.state_event_id.clone();
    state.agent_profile_state.goals_event_id = state.sa_lane.schedule_event_id.clone();
    if state.agent_profile_state.profile_event_id.is_some()
        || state.agent_profile_state.state_event_id.is_some()
    {
        state.agent_profile_state.load_state = PaneLoadState::Ready;
    }
    let _ = refresh_goal_profile_state(state);

    state.agent_schedule_tick.heartbeat_seconds = state.sa_lane.heartbeat_seconds;
    state.agent_schedule_tick.schedule_event_id = state.sa_lane.schedule_event_id.clone();
    state.agent_schedule_tick.tick_request_event_id =
        state.sa_lane.last_tick_request_event_id.clone();
    state.agent_schedule_tick.tick_result_event_id =
        state.sa_lane.last_tick_result_event_id.clone();
    if let Some(outcome) = state.sa_lane.last_result.as_deref() {
        state.agent_schedule_tick.last_tick_outcome = outcome.to_string();
    }
    if state.agent_schedule_tick.schedule_event_id.is_some() {
        state.agent_schedule_tick.load_state = PaneLoadState::Ready;
    }
    let _ = refresh_goal_schedule_state(state);

    state.trajectory_audit.active_session_id = state
        .sa_lane
        .last_tick_request_event_id
        .as_deref()
        .map(|event| format!("traj:{event}"));
    if state.trajectory_audit.active_session_id.is_some() {
        state.trajectory_audit.load_state = PaneLoadState::Ready;
    }
    if let Some(last_transfer) = state.stable_sats_simulation.transfer_ledger.last() {
        state.trajectory_audit.treasury_event_ref = Some(last_transfer.transfer_ref.clone());
        state.trajectory_audit.treasury_event_summary =
            Some(render_treasury_transfer_summary(last_transfer));
    } else {
        state.trajectory_audit.treasury_event_ref = None;
        state.trajectory_audit.treasury_event_summary = None;
    }
}

pub(super) fn run_agent_profile_state_action(
    state: &mut RenderState,
    action: AgentProfileStatePaneAction,
) -> bool {
    match action {
        AgentProfileStatePaneAction::PublishProfile => {
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentProfile {
                display_name: state.agent_profile_state.profile_name.clone(),
                about: state.agent_profile_state.profile_about.clone(),
                version: "mvp".to_string(),
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.last_action =
                        Some(format!("Queued profile publish command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        AgentProfileStatePaneAction::PublishState => {
            let encrypted_state_ref = format!(
                "nip44:state:{}:{}",
                state.agent_profile_state.profile_name.to_lowercase(),
                state.agent_profile_state.profile_about.len()
            );
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
                encrypted_state_ref,
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.last_action =
                        Some(format!("Queued state publish command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        AgentProfileStatePaneAction::UpdateGoals => {
            let encrypted_state_ref = format!(
                "nip44:goals:{}",
                state.agent_profile_state.goals_summary.len()
            );
            match state.queue_sa_command(SaLifecycleCommand::PublishAgentState {
                encrypted_state_ref,
            }) {
                Ok(command_seq) => {
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.goals_event_id =
                        Some(format!("sa:goals:pending:{command_seq}"));
                    state.agent_profile_state.last_action =
                        Some(format!("Queued goals update command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_profile_state(state);
            true
        }
        AgentProfileStatePaneAction::CreateGoal => {
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let goal_id = format!("goal-{}", now_epoch_seconds);
            let goal = GoalRecord {
                goal_id: goal_id.clone(),
                title: "Autonomous earn sats".to_string(),
                objective: GoalObjective::EarnBitcoin {
                    min_wallet_delta_sats: 500,
                    note: Some("created from agent profile controls".to_string()),
                },
                constraints: GoalConstraints::default(),
                stop_conditions: vec![GoalStopCondition::WalletDeltaSatsAtLeast { sats: 500 }],
                retry_policy: GoalRetryPolicy::default(),
                schedule: GoalScheduleConfig::default(),
                lifecycle_status: GoalLifecycleStatus::Draft,
                created_at_epoch_seconds: now_epoch_seconds,
                updated_at_epoch_seconds: now_epoch_seconds,
                attempt_count: 0,
                last_failure_reason: None,
                terminal_reason: None,
                last_receipt_id: None,
                recovery_replay_pending: false,
            };
            match state.autopilot_goals.upsert_active_goal(goal) {
                Ok(()) => {
                    state.agent_profile_state.selected_goal_id = Some(goal_id.clone());
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.last_action = Some(format!("Created goal {goal_id}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_profile_state(state);
            true
        }
        AgentProfileStatePaneAction::StartGoal => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_profile_state.last_error =
                    Some("No goal selected to start".to_string());
                state.agent_profile_state.load_state = PaneLoadState::Error;
                let _ = refresh_goal_profile_state(state);
                return true;
            };

            let lifecycle = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .find(|goal| goal.goal_id == goal_id)
                .map(|goal| goal.lifecycle_status);
            let result = match lifecycle {
                Some(GoalLifecycleStatus::Draft) => state
                    .autopilot_goals
                    .transition_goal(&goal_id, GoalLifecycleEvent::Queue)
                    .and_then(|_| {
                        state
                            .autopilot_goals
                            .transition_goal(&goal_id, GoalLifecycleEvent::StartRun)
                    })
                    .map(|_| ()),
                Some(GoalLifecycleStatus::Paused) => state
                    .autopilot_goals
                    .transition_goal(&goal_id, GoalLifecycleEvent::Resume)
                    .and_then(|_| {
                        state
                            .autopilot_goals
                            .transition_goal(&goal_id, GoalLifecycleEvent::StartRun)
                    })
                    .map(|_| ()),
                Some(GoalLifecycleStatus::Queued) => state
                    .autopilot_goals
                    .transition_goal(&goal_id, GoalLifecycleEvent::StartRun)
                    .map(|_| ()),
                Some(GoalLifecycleStatus::Running) => Ok(()),
                Some(status) => Err(format!("Cannot start goal from {status:?}")),
                None => Err(format!("Active goal {goal_id} not found")),
            };

            match result {
                Ok(()) => {
                    state.agent_profile_state.selected_goal_id = Some(goal_id.clone());
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.last_action = Some(format!("Started goal {goal_id}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_profile_state(state);
            true
        }
        AgentProfileStatePaneAction::AbortGoal => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_profile_state.last_error =
                    Some("No goal selected to abort".to_string());
                state.agent_profile_state.load_state = PaneLoadState::Error;
                let _ = refresh_goal_profile_state(state);
                return true;
            };

            let result = state.autopilot_goals.transition_goal(
                &goal_id,
                GoalLifecycleEvent::Abort {
                    reason: "aborted from agent profile controls".to_string(),
                },
            );
            match result {
                Ok(_) => {
                    if state
                        .goal_loop_executor
                        .active_run
                        .as_ref()
                        .is_some_and(|run| run.goal_id == goal_id)
                    {
                        let now_epoch_seconds = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_secs())
                            .unwrap_or(0);
                        state.goal_loop_executor.complete_run(
                            now_epoch_seconds,
                            GoalLifecycleStatus::Aborted,
                            GoalLoopStopReason::ConditionStop {
                                reasons: vec!["aborted from UI".to_string()],
                            },
                        );
                    }
                    state.agent_profile_state.last_error = None;
                    state.agent_profile_state.load_state = PaneLoadState::Ready;
                    state.agent_profile_state.last_action = Some(format!("Aborted goal {goal_id}"));
                }
                Err(error) => {
                    state.agent_profile_state.last_error = Some(error);
                    state.agent_profile_state.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_profile_state(state);
            true
        }
        AgentProfileStatePaneAction::InspectGoalReceipt => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_profile_state.last_error =
                    Some("No goal selected for receipt inspection".to_string());
                state.agent_profile_state.load_state = PaneLoadState::Error;
                let _ = refresh_goal_profile_state(state);
                return true;
            };

            let summary = state
                .autopilot_goals
                .document
                .receipts
                .iter()
                .filter(|receipt| receipt.goal_id == goal_id)
                .max_by(|left, right| {
                    left.finished_at_epoch_seconds
                        .cmp(&right.finished_at_epoch_seconds)
                        .then_with(|| left.receipt_id.cmp(&right.receipt_id))
                })
                .map(|receipt| {
                    format!(
                        "{} status={:?} wallet_delta={} sats jobs={} errors={}",
                        receipt.receipt_id,
                        receipt.lifecycle_status,
                        receipt.wallet_delta_sats,
                        receipt.jobs_completed,
                        receipt.errors
                    )
                })
                .unwrap_or_else(|| "No receipt recorded for selected goal".to_string());
            state.agent_profile_state.selected_goal_receipt_summary = summary;
            state.agent_profile_state.last_error = None;
            state.agent_profile_state.load_state = PaneLoadState::Ready;
            state.agent_profile_state.last_action =
                Some(format!("Inspected receipt for goal {goal_id}"));
            let _ = refresh_goal_profile_state(state);
            true
        }
    }
}

fn selected_goal_id(state: &RenderState) -> Option<String> {
    if let Some(selected) = state.agent_profile_state.selected_goal_id.as_ref()
        && (state
            .autopilot_goals
            .document
            .active_goals
            .iter()
            .any(|goal| goal.goal_id == *selected)
            || state
                .autopilot_goals
                .document
                .historical_goals
                .iter()
                .any(|goal| goal.goal_id == *selected))
    {
        return Some(selected.clone());
    }

    if let Some(goal) = select_runnable_goal(&state.autopilot_goals.document.active_goals) {
        return Some(goal.goal_id.clone());
    }

    state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .min_by(|left, right| left.goal_id.cmp(&right.goal_id))
        .map(|goal| goal.goal_id.clone())
        .or_else(|| {
            state
                .autopilot_goals
                .document
                .historical_goals
                .iter()
                .max_by(|left, right| {
                    left.updated_at_epoch_seconds
                        .cmp(&right.updated_at_epoch_seconds)
                        .then_with(|| left.goal_id.cmp(&right.goal_id))
                })
                .map(|goal| goal.goal_id.clone())
        })
}

pub(super) fn refresh_goal_profile_state(state: &mut RenderState) -> bool {
    let before = (
        state.agent_profile_state.goals_summary.clone(),
        state.agent_profile_state.selected_goal_id.clone(),
        state.agent_profile_state.selected_goal_status.clone(),
        state.agent_profile_state.selected_goal_attempts,
        state
            .agent_profile_state
            .selected_goal_selected_skills
            .clone(),
        state
            .agent_profile_state
            .selected_goal_receipt_summary
            .clone(),
        state.agent_profile_state.treasury_wallet_projection_count,
        state
            .agent_profile_state
            .treasury_wallet_projection_summary
            .clone(),
    );
    let active_count = state.autopilot_goals.document.active_goals.len();
    let historical_count = state.autopilot_goals.document.historical_goals.len();
    let receipt_count = state.autopilot_goals.document.receipts.len();
    state.agent_profile_state.goals_summary = format!(
        "Active: {} | Historical: {} | Receipts: {}",
        active_count, historical_count, receipt_count
    );
    let (wallet_projection_count, wallet_projection_summary) =
        treasury_wallet_projection(state).unwrap_or_else(|| (0, "n/a".to_string()));
    state.agent_profile_state.treasury_wallet_projection_count = wallet_projection_count;
    state.agent_profile_state.treasury_wallet_projection_summary = wallet_projection_summary;
    if let Some(last_transfer) = state.stable_sats_simulation.transfer_ledger.last() {
        state.trajectory_audit.treasury_event_ref = Some(last_transfer.transfer_ref.clone());
        state.trajectory_audit.treasury_event_summary =
            Some(render_treasury_transfer_summary(last_transfer));
    } else {
        state.trajectory_audit.treasury_event_ref = None;
        state.trajectory_audit.treasury_event_summary = None;
    }

    let selected_goal_id = selected_goal_id(state);
    state.agent_profile_state.selected_goal_id = selected_goal_id.clone();

    let Some(goal_id) = selected_goal_id else {
        state.agent_profile_state.selected_goal_status = "n/a".to_string();
        state.agent_profile_state.selected_goal_attempts = 0;
        state.agent_profile_state.selected_goal_selected_skills = "n/a".to_string();
        state.agent_profile_state.selected_goal_receipt_summary = "n/a".to_string();
        return before
            != (
                state.agent_profile_state.goals_summary.clone(),
                state.agent_profile_state.selected_goal_id.clone(),
                state.agent_profile_state.selected_goal_status.clone(),
                state.agent_profile_state.selected_goal_attempts,
                state
                    .agent_profile_state
                    .selected_goal_selected_skills
                    .clone(),
                state
                    .agent_profile_state
                    .selected_goal_receipt_summary
                    .clone(),
                state.agent_profile_state.treasury_wallet_projection_count,
                state
                    .agent_profile_state
                    .treasury_wallet_projection_summary
                    .clone(),
            );
    };

    if let Some(goal) = state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .find(|goal| goal.goal_id == goal_id)
    {
        state.agent_profile_state.selected_goal_status = format!("{:?}", goal.lifecycle_status);
        state.agent_profile_state.selected_goal_attempts = goal.attempt_count;
        let selected_skills = state
            .autopilot_goals
            .resolve_skill_candidates_for_goal(&goal_id, &state.skill_registry.discovered_skills)
            .ok()
            .map(|resolution| {
                resolution
                    .candidates
                    .iter()
                    .map(|candidate| candidate.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|skills| !skills.trim().is_empty())
            .unwrap_or_else(|| "n/a".to_string());
        state.agent_profile_state.selected_goal_selected_skills = selected_skills;
    } else if let Some(goal) = state
        .autopilot_goals
        .document
        .historical_goals
        .iter()
        .find(|goal| goal.goal_id == goal_id)
    {
        state.agent_profile_state.selected_goal_status = format!("{:?}", goal.lifecycle_status);
        state.agent_profile_state.selected_goal_attempts = goal.attempt_count;
        state.agent_profile_state.selected_goal_selected_skills = "n/a".to_string();
    } else {
        state.agent_profile_state.selected_goal_status = "missing".to_string();
        state.agent_profile_state.selected_goal_attempts = 0;
        state.agent_profile_state.selected_goal_selected_skills = "n/a".to_string();
    }

    let receipt_summary = state
        .autopilot_goals
        .document
        .receipts
        .iter()
        .filter(|receipt| receipt.goal_id == goal_id)
        .max_by(|left, right| {
            left.finished_at_epoch_seconds
                .cmp(&right.finished_at_epoch_seconds)
                .then_with(|| left.receipt_id.cmp(&right.receipt_id))
        })
        .map(|receipt| {
            format!(
                "{} status={:?} wallet_delta={} jobs={} errors={}",
                receipt.receipt_id,
                receipt.lifecycle_status,
                receipt.wallet_delta_sats,
                receipt.jobs_completed,
                receipt.errors
            )
        })
        .unwrap_or_else(|| "n/a".to_string());
    state.agent_profile_state.selected_goal_receipt_summary = receipt_summary;

    before
        != (
            state.agent_profile_state.goals_summary.clone(),
            state.agent_profile_state.selected_goal_id.clone(),
            state.agent_profile_state.selected_goal_status.clone(),
            state.agent_profile_state.selected_goal_attempts,
            state
                .agent_profile_state
                .selected_goal_selected_skills
                .clone(),
            state
                .agent_profile_state
                .selected_goal_receipt_summary
                .clone(),
            state.agent_profile_state.treasury_wallet_projection_count,
            state
                .agent_profile_state
                .treasury_wallet_projection_summary
                .clone(),
        )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GoalSchedulePaneFingerprint {
    selected_goal_id: Option<String>,
    scheduler_mode: String,
    next_goal_run_epoch_seconds: Option<u64>,
    last_goal_run_epoch_seconds: Option<u64>,
    missed_run_policy: String,
    pending_catchup_runs: u32,
    last_recovery_epoch_seconds: Option<u64>,
    heartbeat_seconds: u64,
    cron_expression: String,
    cron_timezone: String,
    cron_next_run_preview_epoch_seconds: Option<u64>,
    cron_parse_error: Option<String>,
    os_scheduler_enabled: bool,
    os_scheduler_adapter: String,
    os_scheduler_descriptor_path: Option<String>,
    os_scheduler_last_reconciled_epoch_seconds: Option<u64>,
    os_scheduler_last_reconcile_result: Option<String>,
    next_tick_reason: String,
}

fn schedule_pane_fingerprint(state: &RenderState) -> GoalSchedulePaneFingerprint {
    GoalSchedulePaneFingerprint {
        selected_goal_id: state.agent_schedule_tick.selected_goal_id.clone(),
        scheduler_mode: state.agent_schedule_tick.scheduler_mode.clone(),
        next_goal_run_epoch_seconds: state.agent_schedule_tick.next_goal_run_epoch_seconds,
        last_goal_run_epoch_seconds: state.agent_schedule_tick.last_goal_run_epoch_seconds,
        missed_run_policy: state.agent_schedule_tick.missed_run_policy.clone(),
        pending_catchup_runs: state.agent_schedule_tick.pending_catchup_runs,
        last_recovery_epoch_seconds: state.agent_schedule_tick.last_recovery_epoch_seconds,
        heartbeat_seconds: state.agent_schedule_tick.heartbeat_seconds,
        cron_expression: state.agent_schedule_tick.cron_expression.clone(),
        cron_timezone: state.agent_schedule_tick.cron_timezone.clone(),
        cron_next_run_preview_epoch_seconds: state
            .agent_schedule_tick
            .cron_next_run_preview_epoch_seconds,
        cron_parse_error: state.agent_schedule_tick.cron_parse_error.clone(),
        os_scheduler_enabled: state.agent_schedule_tick.os_scheduler_enabled,
        os_scheduler_adapter: state.agent_schedule_tick.os_scheduler_adapter.clone(),
        os_scheduler_descriptor_path: state
            .agent_schedule_tick
            .os_scheduler_descriptor_path
            .clone(),
        os_scheduler_last_reconciled_epoch_seconds: state
            .agent_schedule_tick
            .os_scheduler_last_reconciled_epoch_seconds,
        os_scheduler_last_reconcile_result: state
            .agent_schedule_tick
            .os_scheduler_last_reconcile_result
            .clone(),
        next_tick_reason: state.agent_schedule_tick.next_tick_reason.clone(),
    }
}

pub(super) fn refresh_goal_schedule_state(state: &mut RenderState) -> bool {
    let before = schedule_pane_fingerprint(state);

    let selected_goal_id = selected_goal_id(state);
    state.agent_schedule_tick.selected_goal_id = selected_goal_id.clone();

    if let Some(goal_id) = selected_goal_id
        && let Some(goal) = state
            .autopilot_goals
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
    {
        state.agent_schedule_tick.scheduler_mode = match goal.schedule.kind {
            crate::state::autopilot_goals::GoalScheduleKind::Manual => "manual".to_string(),
            crate::state::autopilot_goals::GoalScheduleKind::IntervalSeconds { seconds } => {
                state.agent_schedule_tick.heartbeat_seconds = seconds.max(1);
                format!("interval:{}s", seconds)
            }
            crate::state::autopilot_goals::GoalScheduleKind::Cron {
                ref expression,
                ref timezone,
            } => {
                let timezone = timezone.as_deref().unwrap_or("UTC");
                state.agent_schedule_tick.cron_expression = expression.trim().to_string();
                state.agent_schedule_tick.cron_timezone = timezone.to_string();
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                match parse_cron_expression(expression).and_then(|spec| {
                    next_cron_run_epoch_seconds(&spec, timezone, now_epoch_seconds)
                }) {
                    Ok(next_preview) => {
                        state
                            .agent_schedule_tick
                            .cron_next_run_preview_epoch_seconds = Some(next_preview);
                        state.agent_schedule_tick.cron_parse_error = None;
                    }
                    Err(error) => {
                        state
                            .agent_schedule_tick
                            .cron_next_run_preview_epoch_seconds = None;
                        state.agent_schedule_tick.cron_parse_error = Some(error);
                    }
                }
                "cron".to_string()
            }
        };
        state.agent_schedule_tick.next_goal_run_epoch_seconds =
            goal.schedule.next_run_epoch_seconds;
        state.agent_schedule_tick.last_goal_run_epoch_seconds =
            goal.schedule.last_run_epoch_seconds;
        state.agent_schedule_tick.missed_run_policy = match goal.schedule.missed_run_policy {
            crate::state::autopilot_goals::GoalMissedRunPolicy::CatchUp => "catch_up".to_string(),
            crate::state::autopilot_goals::GoalMissedRunPolicy::Skip => "skip".to_string(),
            crate::state::autopilot_goals::GoalMissedRunPolicy::SingleReplay => {
                "single_replay".to_string()
            }
        };
        state.agent_schedule_tick.pending_catchup_runs = goal.schedule.pending_catchup_runs;
        state.agent_schedule_tick.last_recovery_epoch_seconds =
            goal.schedule.last_recovery_epoch_seconds;
        state.agent_schedule_tick.os_scheduler_enabled = goal.schedule.os_adapter.enabled;
        state.agent_schedule_tick.os_scheduler_adapter = goal
            .schedule
            .os_adapter
            .adapter
            .map(|kind| kind.as_str().to_string())
            .unwrap_or_else(|| "auto".to_string());
        state.agent_schedule_tick.os_scheduler_descriptor_path =
            goal.schedule.os_adapter.descriptor_path.clone();
        state
            .agent_schedule_tick
            .os_scheduler_last_reconciled_epoch_seconds =
            goal.schedule.os_adapter.last_reconciled_epoch_seconds;
        state.agent_schedule_tick.os_scheduler_last_reconcile_result =
            goal.schedule.os_adapter.last_reconcile_result.clone();
        state.agent_schedule_tick.next_tick_reason = if goal.schedule.enabled {
            match goal.schedule.kind {
                crate::state::autopilot_goals::GoalScheduleKind::Manual => {
                    "goal.scheduler.manual".to_string()
                }
                crate::state::autopilot_goals::GoalScheduleKind::IntervalSeconds { .. } => {
                    "goal.scheduler.interval".to_string()
                }
                crate::state::autopilot_goals::GoalScheduleKind::Cron { .. } => {
                    "goal.scheduler.cron".to_string()
                }
            }
        } else {
            "goal.scheduler.manual".to_string()
        };
        if !matches!(
            goal.schedule.kind,
            crate::state::autopilot_goals::GoalScheduleKind::Cron { .. }
        ) {
            state
                .agent_schedule_tick
                .cron_next_run_preview_epoch_seconds = None;
            state.agent_schedule_tick.cron_parse_error = None;
        }
    } else {
        state.agent_schedule_tick.scheduler_mode = "manual".to_string();
        state.agent_schedule_tick.next_goal_run_epoch_seconds = None;
        state.agent_schedule_tick.last_goal_run_epoch_seconds = None;
        state.agent_schedule_tick.missed_run_policy = "single_replay".to_string();
        state.agent_schedule_tick.pending_catchup_runs = 0;
        state.agent_schedule_tick.last_recovery_epoch_seconds = None;
        state
            .agent_schedule_tick
            .cron_next_run_preview_epoch_seconds = None;
        state.agent_schedule_tick.cron_parse_error = None;
        state.agent_schedule_tick.os_scheduler_enabled = false;
        state.agent_schedule_tick.os_scheduler_adapter = "auto".to_string();
        state.agent_schedule_tick.os_scheduler_descriptor_path = None;
        state
            .agent_schedule_tick
            .os_scheduler_last_reconciled_epoch_seconds = None;
        state.agent_schedule_tick.os_scheduler_last_reconcile_result = None;
        state.agent_schedule_tick.next_tick_reason = "manual.operator".to_string();
    }

    before != schedule_pane_fingerprint(state)
}

fn parse_missed_run_policy(raw: &str) -> Option<GoalMissedRunPolicy> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "catch_up" | "catchup" => Some(GoalMissedRunPolicy::CatchUp),
        "skip" => Some(GoalMissedRunPolicy::Skip),
        "single_replay" | "single-replay" | "single" => Some(GoalMissedRunPolicy::SingleReplay),
        _ => None,
    }
}

pub(super) fn run_agent_schedule_tick_action(
    state: &mut RenderState,
    action: AgentScheduleTickPaneAction,
) -> bool {
    match action {
        AgentScheduleTickPaneAction::ApplySchedule => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_schedule_tick.last_error = Some(
                    "Select or create an active goal before applying interval schedule".to_string(),
                );
                state.agent_schedule_tick.load_state = PaneLoadState::Error;
                let _ = refresh_goal_schedule_state(state);
                return true;
            };
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let schedule_mode = state
                .agent_schedule_tick
                .scheduler_mode
                .to_ascii_lowercase();
            let cron_override = state.agent_schedule_tick.next_tick_reason.trim();
            if let Some((_, policy_raw)) = cron_override.split_once("missed:")
                && let Some(policy) = parse_missed_run_policy(policy_raw)
            {
                let _ = state.autopilot_goals.set_goal_missed_run_policy(
                    &goal_id,
                    policy,
                    now_epoch_seconds,
                );
            }
            let is_cron_mode =
                schedule_mode.starts_with("cron") || cron_override.starts_with("cron:");

            let result = if is_cron_mode {
                let expression = if let Some((_, value)) = cron_override.split_once("cron:") {
                    value.trim()
                } else {
                    state.agent_schedule_tick.cron_expression.trim()
                };
                let timezone = state.agent_schedule_tick.cron_timezone.trim();
                state.autopilot_goals.set_goal_cron_schedule(
                    &goal_id,
                    expression,
                    timezone,
                    now_epoch_seconds,
                )
            } else {
                let interval_seconds = state.agent_schedule_tick.heartbeat_seconds.max(1);
                state.autopilot_goals.set_goal_interval_schedule(
                    &goal_id,
                    interval_seconds,
                    now_epoch_seconds,
                )
            };

            match result {
                Ok(()) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action = Some(format!(
                        "Applied {} schedule for {}",
                        if is_cron_mode { "cron" } else { "interval" },
                        goal_id
                    ));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_schedule_state(state);
            true
        }
        AgentScheduleTickPaneAction::PublishManualTick => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_schedule_tick.last_error =
                    Some("Select or create an active goal before manual run".to_string());
                state.agent_schedule_tick.load_state = PaneLoadState::Error;
                let _ = refresh_goal_schedule_state(state);
                return true;
            };
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            match state
                .autopilot_goals
                .schedule_goal_run_now(&goal_id, now_epoch_seconds)
            {
                Ok(()) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action =
                        Some(format!("Scheduled immediate goal run for {}", goal_id));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_schedule_state(state);
            true
        }
        AgentScheduleTickPaneAction::ToggleOsSchedulerAdapter => {
            let Some(goal_id) = selected_goal_id(state) else {
                state.agent_schedule_tick.last_error = Some(
                    "Select or create an active goal before toggling OS scheduler".to_string(),
                );
                state.agent_schedule_tick.load_state = PaneLoadState::Error;
                let _ = refresh_goal_schedule_state(state);
                return true;
            };

            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let (current_enabled, current_adapter) = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .find(|goal| goal.goal_id == goal_id)
                .map(|goal| {
                    (
                        goal.schedule.os_adapter.enabled,
                        goal.schedule.os_adapter.adapter,
                    )
                })
                .unwrap_or((false, None));
            let next_enabled = !current_enabled;
            let requested_adapter = if next_enabled {
                OsSchedulerAdapterKind::from_label(&state.agent_schedule_tick.os_scheduler_adapter)
                    .or(current_adapter)
                    .or_else(preferred_adapter_for_host)
                    .or(Some(OsSchedulerAdapterKind::Cron))
            } else {
                current_adapter
                    .or_else(|| {
                        OsSchedulerAdapterKind::from_label(
                            &state.agent_schedule_tick.os_scheduler_adapter,
                        )
                    })
                    .or(Some(OsSchedulerAdapterKind::Cron))
            };

            match state.autopilot_goals.set_goal_os_scheduler_adapter(
                &goal_id,
                next_enabled,
                requested_adapter,
                now_epoch_seconds,
            ) {
                Ok(()) => {
                    if next_enabled {
                        let _ = state
                            .autopilot_goals
                            .reconcile_os_scheduler_adapters(now_epoch_seconds);
                    }
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action = Some(format!(
                        "{} OS scheduler adapter for {}",
                        if next_enabled { "Enabled" } else { "Disabled" },
                        goal_id
                    ));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = PaneLoadState::Error;
                }
            }
            let _ = refresh_goal_schedule_state(state);
            true
        }
        AgentScheduleTickPaneAction::InspectLastResult => {
            let _ = refresh_goal_schedule_state(state);
            state.agent_schedule_tick.last_tick_outcome = state
                .autopilot_goals
                .last_action
                .clone()
                .unwrap_or_else(|| "No goal scheduler result yet".to_string());
            state.agent_schedule_tick.last_error = None;
            state.agent_schedule_tick.load_state = PaneLoadState::Ready;
            state.agent_schedule_tick.last_action =
                Some("Refreshed goal scheduler status".to_string());
            true
        }
    }
}

pub(super) fn run_trajectory_audit_action(
    state: &mut RenderState,
    action: TrajectoryAuditPaneAction,
) -> bool {
    match action {
        TrajectoryAuditPaneAction::OpenSession => {
            let session = state
                .trajectory_audit
                .treasury_event_ref
                .as_deref()
                .map(|event| format!("traj:treasury:{event}"))
                .or_else(|| {
                    state
                        .sa_lane
                        .last_tick_request_event_id
                        .as_deref()
                        .map(|event| format!("traj:{event}"))
                })
                .unwrap_or_else(|| format!("traj:manual:{}", state.sa_lane.tick_count + 1));
            state.trajectory_audit.active_session_id = Some(session.clone());
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = PaneLoadState::Ready;
            state.trajectory_audit.last_action =
                Some(format!("Opened trajectory session {session}"));
            true
        }
        TrajectoryAuditPaneAction::CycleStepFilter => {
            state.trajectory_audit.step_filter =
                super::super::next_trajectory_step_filter(&state.trajectory_audit.step_filter);
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = PaneLoadState::Ready;
            state.trajectory_audit.last_action = Some(format!(
                "Set trajectory filter to {}",
                state.trajectory_audit.step_filter
            ));
            true
        }
        TrajectoryAuditPaneAction::VerifyTrajectoryHash => {
            let Some(session) = state.trajectory_audit.active_session_id.as_deref() else {
                state.trajectory_audit.last_error =
                    Some("Open a trajectory session before verification".to_string());
                state.trajectory_audit.load_state = PaneLoadState::Error;
                return true;
            };
            state.trajectory_audit.verified_hash =
                Some(super::super::trajectory_verification_hash(
                    session,
                    state
                        .sa_lane
                        .last_tick_result_event_id
                        .as_deref()
                        .unwrap_or("none"),
                    state.sa_lane.tick_count,
                ));
            state.trajectory_audit.last_error = None;
            state.trajectory_audit.load_state = PaneLoadState::Ready;
            state.trajectory_audit.last_action =
                Some("Verified trajectory hash from SA tick context".to_string());
            true
        }
    }
}

fn treasury_wallet_projection(state: &RenderState) -> Option<(usize, String)> {
    let wallets = state.stable_sats_simulation.agents.as_slice();
    if wallets.is_empty() {
        return None;
    }
    let summary = wallets
        .iter()
        .map(|wallet| {
            format!(
                "{}({}): {} sats {}",
                wallet.agent_name,
                wallet.owner_kind.label(),
                wallet.btc_balance_sats,
                format_usd_cents(wallet.usd_balance_cents)
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    Some((wallets.len(), summary))
}

fn render_treasury_transfer_summary(
    transfer: &crate::app_state::StableSatsTransferLedgerEntry,
) -> String {
    let amount = match transfer.asset {
        crate::app_state::StableSatsTransferAsset::BtcSats => {
            format!("{} sats", transfer.amount)
        }
        crate::app_state::StableSatsTransferAsset::UsdCents => {
            format_usd_cents(transfer.amount)
        }
    };
    let fee = match transfer.asset {
        crate::app_state::StableSatsTransferAsset::BtcSats => {
            format!("{} sats", transfer.effective_fee)
        }
        crate::app_state::StableSatsTransferAsset::UsdCents => {
            format_usd_cents(transfer.effective_fee)
        }
    };
    format!(
        "{} {} {} {} -> {} fee={} [{}]",
        transfer.status.label(),
        transfer.asset.label(),
        amount,
        transfer.from_wallet,
        transfer.to_wallet,
        fee,
        transfer.transfer_ref
    )
}

fn format_usd_cents(usd_cents: u64) -> String {
    format!("${}.{:02}", usd_cents / 100, usd_cents % 100)
}

#[cfg(test)]
mod tests {
    use super::render_treasury_transfer_summary;
    use crate::app_state::{
        StableSatsTransferAsset, StableSatsTransferLedgerEntry, StableSatsTransferStatus,
    };

    #[test]
    fn renders_btc_treasury_transfer_summary() {
        let transfer = StableSatsTransferLedgerEntry {
            seq: 1,
            transfer_ref: "blink:live:transfer:0001".to_string(),
            from_wallet: "autopilot-user:BTC".to_string(),
            to_wallet: "sa-wallet-1:USD".to_string(),
            asset: StableSatsTransferAsset::BtcSats,
            amount: 500,
            effective_fee: 1,
            status: StableSatsTransferStatus::Settled,
            summary: "test".to_string(),
            occurred_at_epoch_seconds: 1_761_922_000,
        };

        let rendered = render_treasury_transfer_summary(&transfer);
        assert!(rendered.contains("settled BTC 500 sats"));
        assert!(rendered.contains("fee=1 sats"));
        assert!(rendered.contains("blink:live:transfer:0001"));
    }

    #[test]
    fn renders_usd_treasury_transfer_summary() {
        let transfer = StableSatsTransferLedgerEntry {
            seq: 2,
            transfer_ref: "blink:live:transfer:0002".to_string(),
            from_wallet: "sa-wallet-2:USD".to_string(),
            to_wallet: "autopilot-user:BTC".to_string(),
            asset: StableSatsTransferAsset::UsdCents,
            amount: 135,
            effective_fee: 2,
            status: StableSatsTransferStatus::Settled,
            summary: "test".to_string(),
            occurred_at_epoch_seconds: 1_761_922_001,
        };

        let rendered = render_treasury_transfer_summary(&transfer);
        assert!(rendered.contains("settled USD $1.35"));
        assert!(rendered.contains("fee=$0.02"));
        assert!(rendered.contains("blink:live:transfer:0002"));
    }
}
