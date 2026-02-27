//! Runtime update reducers are split by domain to keep `input.rs` focused on event routing.

mod ac;
mod sa;
mod skl;
pub(super) mod wallet;

use crate::app_state::RenderState;
use crate::runtime_lanes::{
    AcLaneUpdate, RuntimeCommandResponse, RuntimeCommandStatus, RuntimeLane, SaLaneUpdate,
    SklLaneUpdate,
};

pub(super) fn drain_runtime_lane_updates(state: &mut RenderState) -> bool {
    let mut changed = false;

    for update in state.sa_lane_worker.drain_updates() {
        changed = true;
        match update {
            SaLaneUpdate::Snapshot(snapshot) => sa::apply_lane_snapshot(state, *snapshot),
            SaLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response)
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

    changed
}

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    wallet::drain_spark_worker_updates(state)
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
