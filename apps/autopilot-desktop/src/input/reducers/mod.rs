//! Runtime update reducers are split by domain to keep `input.rs` focused on event routing.

mod ac;
mod cad;
mod codex;
mod jobs;
mod sa;
mod skl;
pub(super) mod wallet;
pub(super) use cad::CadChatPromptApplyOutcome;

use crate::app_state::RenderState;
use crate::pane_system::{
    ActiveJobPaneAction, AgentProfileStatePaneAction, AgentScheduleTickPaneAction,
    CadDemoPaneAction, CreditDeskPaneAction, CreditSettlementLedgerPaneAction,
    JobHistoryPaneAction, JobInboxPaneAction, SkillRegistryPaneAction,
    SkillTrustRevocationPaneAction, TrajectoryAuditPaneAction,
};
use crate::runtime_lanes::{
    AcLaneUpdate, RuntimeCommandResponse, RuntimeCommandStatus, RuntimeLane, SaLaneUpdate,
    SklLaneUpdate,
};

pub(super) fn run_job_inbox_action(state: &mut RenderState, action: JobInboxPaneAction) -> bool {
    jobs::run_job_inbox_action(state, action)
}

pub(super) fn run_active_job_action(state: &mut RenderState, action: ActiveJobPaneAction) -> bool {
    jobs::run_active_job_action(state, action)
}

pub(super) fn run_job_history_action(
    state: &mut RenderState,
    action: JobHistoryPaneAction,
) -> bool {
    jobs::run_job_history_action(state, action)
}

pub(super) fn drain_runtime_lane_updates(state: &mut RenderState) -> bool {
    let mut changed = false;

    for update in state.sa_lane_worker.drain_updates() {
        changed = true;
        match update {
            SaLaneUpdate::Snapshot(snapshot) => sa::apply_lane_snapshot(state, *snapshot),
            SaLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.skl_lane_worker.drain_updates() {
        changed = true;
        match update {
            SklLaneUpdate::Snapshot(snapshot) => skl::apply_lane_snapshot(state, *snapshot),
            SklLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.ac_lane_worker.drain_updates() {
        changed = true;
        match update {
            AcLaneUpdate::Snapshot(snapshot) => ac::apply_lane_snapshot(state, *snapshot),
            AcLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.codex_lane_worker.drain_updates() {
        changed = true;
        match update {
            crate::codex_lane::CodexLaneUpdate::Snapshot(snapshot) => {
                codex::apply_lane_snapshot(state, *snapshot);
            }
            crate::codex_lane::CodexLaneUpdate::CommandResponse(response) => {
                codex::apply_command_response(state, response);
            }
            crate::codex_lane::CodexLaneUpdate::Notification(notification) => {
                codex::apply_notification(state, notification);
            }
        }
    }

    changed
}

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    wallet::drain_spark_worker_updates(state)
}

pub(super) fn drain_stable_sats_blink_worker_updates(state: &mut RenderState) -> bool {
    let updates = state.stable_sats_blink_worker.drain_updates(4);
    if updates.is_empty() {
        return false;
    }

    let mut changed = false;
    for update in updates {
        match update {
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::Completed(snapshot) => {
                if !state
                    .stable_sats_simulation
                    .finish_live_refresh(snapshot.request_id)
                {
                    continue;
                }
                let wallet_snapshots = snapshot
                    .wallet_snapshots
                    .iter()
                    .map(|wallet| {
                        (
                            wallet.owner_id.clone(),
                            wallet.btc_balance_sats,
                            wallet.usd_balance_cents,
                            wallet.source_ref.clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                let wallet_failures = snapshot
                    .wallet_failures
                    .iter()
                    .map(|failure| (failure.owner_id.clone(), failure.error.clone()))
                    .collect::<Vec<_>>();
                state.stable_sats_simulation.apply_live_wallet_snapshots(
                    snapshot.now_epoch_seconds,
                    snapshot.price_usd_cents_per_btc,
                    wallet_snapshots.as_slice(),
                    wallet_failures.as_slice(),
                );
                state.provider_runtime.last_result =
                    state.stable_sats_simulation.last_action.clone();
                let event_id = format!(
                    "sim:stablesats:round:{}",
                    state.stable_sats_simulation.rounds_run
                );
                state
                    .activity_feed
                    .upsert_event(crate::app_state::ActivityEventRow {
                        event_id,
                        domain: crate::app_state::ActivityEventDomain::Wallet,
                        source_tag: "blink.live".to_string(),
                        summary: "StableSats live Blink balances refreshed".to_string(),
                        detail: format!(
                            "mode={} round={} quote={} converted_sats={} converted_usd_cents={}",
                            state.stable_sats_simulation.mode.label(),
                            state.stable_sats_simulation.rounds_run,
                            state.stable_sats_simulation.price_usd_cents_per_btc,
                            state.stable_sats_simulation.total_converted_sats,
                            state.stable_sats_simulation.total_converted_usd_cents
                        ),
                        occurred_at_epoch_seconds: snapshot.now_epoch_seconds,
                    });
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::Failed { request_id, error } => {
                if state
                    .stable_sats_simulation
                    .fail_live_refresh(request_id, error)
                {
                    state.provider_runtime.last_result =
                        state.stable_sats_simulation.last_action.clone();
                    changed = true;
                }
            }
        }
    }

    changed
}

pub(super) fn run_agent_profile_state_action(
    state: &mut RenderState,
    action: AgentProfileStatePaneAction,
) -> bool {
    sa::run_agent_profile_state_action(state, action)
}

pub(super) fn refresh_goal_profile_state(state: &mut RenderState) -> bool {
    let profile_changed = sa::refresh_goal_profile_state(state);
    let schedule_changed = sa::refresh_goal_schedule_state(state);
    profile_changed || schedule_changed
}

pub(super) fn run_agent_schedule_tick_action(
    state: &mut RenderState,
    action: AgentScheduleTickPaneAction,
) -> bool {
    sa::run_agent_schedule_tick_action(state, action)
}

pub(super) fn run_trajectory_audit_action(
    state: &mut RenderState,
    action: TrajectoryAuditPaneAction,
) -> bool {
    sa::run_trajectory_audit_action(state, action)
}

pub(super) fn run_skill_registry_action(
    state: &mut RenderState,
    action: SkillRegistryPaneAction,
) -> bool {
    skl::run_skill_registry_action(state, action)
}

pub(super) fn run_skill_trust_revocation_action(
    state: &mut RenderState,
    action: SkillTrustRevocationPaneAction,
) -> bool {
    skl::run_skill_trust_revocation_action(state, action)
}

pub(super) fn run_credit_desk_action(
    state: &mut RenderState,
    action: CreditDeskPaneAction,
) -> bool {
    ac::run_credit_desk_action(state, action)
}

pub(super) fn run_credit_settlement_ledger_action(
    state: &mut RenderState,
    action: CreditSettlementLedgerPaneAction,
) -> bool {
    ac::run_credit_settlement_ledger_action(state, action)
}

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    cad::run_cad_demo_action(state, action)
}

pub(super) fn apply_chat_prompt_to_cad_session(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
) -> bool {
    cad::apply_chat_prompt_to_cad_session(state, thread_id, prompt)
}

pub(super) fn apply_chat_prompt_to_cad_session_with_trigger(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
    rebuild_trigger_prefix: Option<&str>,
) -> bool {
    cad::apply_chat_prompt_to_cad_session_with_trigger(
        state,
        thread_id,
        prompt,
        rebuild_trigger_prefix,
    )
}

pub(super) fn apply_chat_prompt_to_cad_session_with_trigger_outcome(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
    rebuild_trigger_prefix: Option<&str>,
) -> CadChatPromptApplyOutcome {
    cad::apply_chat_prompt_to_cad_session_with_trigger_outcome(
        state,
        thread_id,
        prompt,
        rebuild_trigger_prefix,
    )
}

pub(super) fn sync_cad_build_progress_to_chat(state: &mut RenderState) {
    cad::sync_cad_build_progress_to_chat(state)
}

fn apply_runtime_command_response(state: &mut RenderState, response: RuntimeCommandResponse) {
    let summary = command_response_summary(&response);
    match response.lane {
        RuntimeLane::SaLifecycle => sa::apply_command_response(state, &response, &summary),
        RuntimeLane::SklDiscoveryTrust => skl::apply_command_response(state, &response, &summary),
        RuntimeLane::AcCredit => ac::apply_command_response(state, &response, &summary),
    }

    if response.status != RuntimeCommandStatus::Accepted {
        super::upsert_runtime_incident_alert(state, &response);
    }

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    state.record_runtime_command_response(response);
}

fn command_response_summary(response: &RuntimeCommandResponse) -> String {
    let mut parts = vec![format!(
        "{} {} {}",
        response.lane.label(),
        response.command.label(),
        response.status.label()
    )];
    if let Some(event_id) = response.event_id.as_deref() {
        parts.push(format!("event:{event_id}"));
    }
    if let Some(error) = response.error.as_ref() {
        parts.push(format!("{}:{}", error.class.label(), error.message));
    }
    parts.join(" | ")
}
