use crate::app_state::{
    AutopilotRole, AutopilotThreadListEntry, ForgeDelegatedChildDeliveryStatus,
    ForgeDelegatedChildSessionCard, ForgeDelegatedChildSessionStatus, ForgeDeliveryBranchWatch,
    ForgeDeliveryComparePosture, ForgeDeliveryCompareWatch, RenderState,
};
use crate::probe_lane::{
    ProbeLaneCommandResponse, ProbeLaneLifecycle, ProbeLaneNotification, ProbeLaneSnapshot,
    ProbeListedSession,
};
use probe_protocol::runtime::{QueuedTurnStatus, RuntimeProgressEvent};
use probe_protocol::session::{
    SessionBranchState, SessionChildStatus, SessionChildSummary, SessionDeliveryState,
    SessionDeliveryStatus, SessionMetadata, SessionState, TranscriptEvent, TranscriptItemKind,
};
use std::collections::HashMap;

// Define a struct to hold campaign objects
#[derive(Debug, Clone)]
pub struct Campaign {
    pub id: String,
    pub goal_metadata: String,
    pub bounded_scope: String,
    pub retained_cases: Vec<String>,
    pub verification_refs: Vec<String>,
}

impl Campaign {
    pub fn new(id: String, goal_metadata: String, bounded_scope: String) -> Self {
        Campaign {
            id,
            goal_metadata,
            bounded_scope,
            retained_cases: Vec::new(),
            verification_refs: Vec::new(),
        }
    }
}

// Define a function to create a new campaign object
pub fn create_campaign(id: String, goal_metadata: String, bounded_scope: String) -> Campaign {
    Campaign::new(id, goal_metadata, bounded_scope)
}

// Define a function to add retained cases to a campaign object
pub fn add_retained_cases(campaign: &mut Campaign, cases: Vec<String>) {
    campaign.retained_cases.extend(cases);
}

// Define a function to add verification refs to a campaign object
pub fn add_verification_refs(campaign: &mut Campaign, refs: Vec<String>) {
    campaign.verification_refs.extend(refs);
}

fn probe_live_turn_id(session_id: &str) -> String {
    format!("probe-live:{session_id}")
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: ProbeLaneSnapshot) {
    let previous_active_session_id = state.probe_lane.active_session_id.clone();
    let lifecycle = snapshot.lifecycle;
    state.probe_lane = snapshot;
    let next_active_session_id = state.probe_lane.active_session_id.clone();
    sync_probe_attached_thread_projection(
        state,
        previous_active_session_id.as_deref(),
        next_active_session_id.as_deref(),
    );
    if state.uses_probe_runtime() {
        state
            .autopilot_chat
            .set_connection_status(lifecycle.label().to_string());
        if lifecycle == ProbeLaneLifecycle::Error || lifecycle == ProbeLaneLifecycle::Disconnected {
            state.autopilot_chat.last_error = state.probe_lane.last_error.clone();
        }
    }
    // Update campaign objects
    if let Some(campaigns) = state.campaigns.as_mut() {
        for campaign in campaigns.values_mut() {
            if campaign.retained_cases.contains(&next_active_session_id) {
                // Update campaign object with new session data
                campaign.goal_metadata = snapshot.session.title.clone();
                campaign.bounded_scope = snapshot.session.cwd.display().to_string();
            }
        }
    }
}

pub(super) fn apply_command_response(state: &mut RenderState, response: ProbeLaneCommandResponse) {
    if response.status == crate::probe_lane::ProbeLaneCommandStatus::Ok {
        match (response.command, response.session_id.as_deref()) {
            (crate::probe_lane::ProbeLaneCommandKind::StartSession, Some(session_id)) => {
                let _ = state
                    .autopilot_chat
                    .mark_probe_workspace_cold_start_for_thread(session_id, current_epoch_millis());
            }
            (crate::probe_lane::ProbeLaneCommandKind::LoadSession, Some(session_id)) => {
                let _ = state
                    .autopilot_chat
                    .mark_probe_workspace_warm_start_for_thread(
                        session_id,
                        Some(format!("probe-session:{session_id}")),
                        current_epoch_millis(),
                    );
            }
            _ => {}
        }
    }
    if state.uses_probe_runtime()
        && response.status == crate::probe_lane::ProbeLaneCommandStatus::Error
    {
        state.autopilot_chat.last_error = response.error.clone();
    }
    state.record_probe_command_response(response);
    // Update campaign objects
    if let Some(campaigns) = state.campaigns.as_mut() {
        for campaign in campaigns.values_mut() {
            if campaign.retained_cases.contains(&response.session_id) {
                // Update campaign object with new session data
                campaign.goal_metadata = response.session_id.clone();
                campaign.bounded_scope = response.session_id.clone();
            }
        }
    }
}

pub(super) fn apply_notification(state: &mut RenderState, notification: ProbeLaneNotification) {
    match &notification {
        ProbeLaneNotification::SessionsListed {
            sessions,
            workspace_session_id,
            workspace_collision_session_ids,
        } => {
            let entries = sessions
                .iter()
                .map(session_metadata_to_thread_entry)
                .collect::<Vec<_>>();
            state.autopilot_chat.set_thread_entries(entries);
            for listed in sessions {
                let runtime_status = listed
                    .control
                    .as_ref()
                    .map(probe_control_status_label)
                    .or_else(|| {
                        (listed.session.state == probe_protocol::session::SessionState::Active)
                            .then_some(String::from("idle"))
                    });
                // Add missing code here if necessary
            }
        }
        _ => {}
    }
}