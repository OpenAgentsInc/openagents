use crate::app_state::{PaneLoadState, ProviderMode, RenderState};
use crate::pane_system::{
    AgentProfileStatePaneAction, AgentScheduleTickPaneAction, TrajectoryAuditPaneAction,
};
use crate::runtime_lanes::{
    RuntimeCommandResponse, RuntimeCommandStatus, SaLaneSnapshot, SaLifecycleCommand, SaRunnerMode,
};
use crate::state::autopilot_goals::{
    GoalConstraints, GoalLifecycleEvent, GoalLifecycleStatus, GoalObjective, GoalRecord,
    GoalRetryPolicy, GoalScheduleConfig, GoalStopCondition,
};
use crate::state::goal_loop_executor::{GoalLoopStopReason, select_runnable_goal};

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

    state.trajectory_audit.active_session_id = state
        .sa_lane
        .last_tick_request_event_id
        .as_deref()
        .map(|event| format!("traj:{event}"));
    if state.trajectory_audit.active_session_id.is_some() {
        state.trajectory_audit.load_state = PaneLoadState::Ready;
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
    );
    let active_count = state.autopilot_goals.document.active_goals.len();
    let historical_count = state.autopilot_goals.document.historical_goals.len();
    let receipt_count = state.autopilot_goals.document.receipts.len();
    state.agent_profile_state.goals_summary = format!(
        "Active: {} | Historical: {} | Receipts: {}",
        active_count, historical_count, receipt_count
    );

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
        )
}

pub(super) fn run_agent_schedule_tick_action(
    state: &mut RenderState,
    action: AgentScheduleTickPaneAction,
) -> bool {
    match action {
        AgentScheduleTickPaneAction::ApplySchedule => {
            match state.queue_sa_command(SaLifecycleCommand::ConfigureAgentSchedule {
                heartbeat_seconds: state.agent_schedule_tick.heartbeat_seconds.max(1),
            }) {
                Ok(command_seq) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action =
                        Some(format!("Queued schedule command #{command_seq}"));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        AgentScheduleTickPaneAction::PublishManualTick => {
            match state.queue_sa_command(SaLifecycleCommand::PublishTickRequest {
                reason: state.agent_schedule_tick.next_tick_reason.clone(),
            }) {
                Ok(command_seq) => {
                    state.agent_schedule_tick.last_error = None;
                    state.agent_schedule_tick.load_state = PaneLoadState::Ready;
                    state.agent_schedule_tick.last_action =
                        Some(format!("Queued manual tick request #{command_seq}"));
                }
                Err(error) => {
                    state.agent_schedule_tick.last_error = Some(error);
                    state.agent_schedule_tick.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        AgentScheduleTickPaneAction::InspectLastResult => {
            state.agent_schedule_tick.last_tick_outcome = state
                .sa_lane
                .last_result
                .clone()
                .unwrap_or_else(|| "No SA tick result yet".to_string());
            state.agent_schedule_tick.last_error = None;
            state.agent_schedule_tick.load_state = PaneLoadState::Ready;
            state.agent_schedule_tick.last_action =
                Some("Refreshed last tick outcome from SA lane".to_string());
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
                .sa_lane
                .last_tick_request_event_id
                .as_deref()
                .map(|event| format!("traj:{event}"))
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
