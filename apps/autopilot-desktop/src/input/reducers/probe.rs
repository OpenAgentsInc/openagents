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
                let attached = state.probe_lane.active_session_id.as_deref()
                    == Some(listed.session.id.as_str());
                state.autopilot_chat.set_probe_thread_projection_state(
                    listed.session.id.as_str(),
                    runtime_status,
                    listed.session.state == probe_protocol::session::SessionState::Archived,
                    attached,
                );
            }
            if let Some(session_id) = workspace_session_id.as_deref() {
                state.autopilot_chat.ensure_thread(session_id.to_string());
            }
            if state.uses_probe_runtime() {
                state.autopilot_chat.last_error = if workspace_collision_session_ids.is_empty() {
                    None
                } else {
                    Some(format!(
                        "Multiple live Probe sessions match the current workspace: {}. Select one from the thread rail instead of starting a new session.",
                        workspace_collision_session_ids.join(", ")
                    ))
                };
            }
        }
        ProbeLaneNotification::SessionLoaded { snapshot, control } => {
            let thread_id = snapshot.session.id.as_str().to_string();
            let previous_active_session_id = state.probe_lane.active_session_id.clone();
            let runtime_status = probe_control_status_label(control);
            let runtime_archived =
                snapshot.session.state == probe_protocol::session::SessionState::Archived;
            state.autopilot_chat.ensure_thread(thread_id.clone());
            if let Some(metadata) = state.autopilot_chat.thread_metadata.get_mut(&thread_id) {
                metadata.loaded = true;
                metadata.created_at = Some(snapshot.session.created_at_ms as i64);
                metadata.updated_at = Some(snapshot.session.updated_at_ms as i64);
            }
            state
                .autopilot_chat
                .set_thread_name(thread_id.as_str(), Some(snapshot.session.title.clone()));
            state.autopilot_chat.set_thread_preview(
                thread_id.as_str(),
                probe_session_preview(&snapshot.session, snapshot.transcript.as_slice()),
            );
            sync_probe_attached_thread_projection(
                state,
                previous_active_session_id.as_deref(),
                Some(thread_id.as_str()),
            );
            state.autopilot_chat.set_thread_workspace_location(
                thread_id.as_str(),
                Some(snapshot.session.cwd.display().to_string()),
                Some(snapshot.session.transcript_path.display().to_string()),
            );
            let _ = state.autopilot_chat.ensure_probe_shared_session_for_thread(
                thread_id.as_str(),
                snapshot.session.updated_at_ms,
            );
            let _ = state
                .autopilot_chat
                .sync_probe_remote_session_projection_for_thread(
                    thread_id.as_str(),
                    snapshot.session.runtime_owner.as_ref(),
                    snapshot.session.workspace_state.as_ref(),
                    snapshot.session.hosted_receipts.as_ref(),
                    snapshot.session.updated_at_ms,
                );
            let _ = state.autopilot_chat.sync_probe_child_sessions_for_thread(
                thread_id.as_str(),
                snapshot
                    .child_sessions
                    .iter()
                    .map(forge_child_session_card_from_probe)
                    .collect(),
                snapshot.session.updated_at_ms,
            );
            let delivery_watch_updated_at_ms = probe_delivery_watch_updated_at_ms(
                snapshot.branch_state.as_ref(),
                snapshot.delivery_state.as_ref(),
                snapshot.session.updated_at_ms,
            );
            let _ = state
                .autopilot_chat
                .sync_probe_delivery_runtime_watch_for_thread(
                    thread_id.as_str(),
                    snapshot.branch_state.as_ref().map(|branch_state| {
                        forge_delivery_branch_watch_from_probe(
                            branch_state,
                            delivery_watch_updated_at_ms,
                        )
                    }),
                    snapshot
                        .delivery_state
                        .as_ref()
                        .map(forge_delivery_compare_watch_from_probe),
                    delivery_watch_updated_at_ms,
                );
            let _ = state
                .autopilot_chat
                .sync_probe_knowledge_mount_projection_for_thread(
                    thread_id.as_str(),
                    snapshot.session.mounted_refs.as_slice(),
                    snapshot.summary_artifacts.as_slice(),
                    snapshot.session.updated_at_ms,
                );
            if let Some(shell_title) = state
                .autopilot_chat
                .probe_shared_session_shell_title(thread_id.as_str())
            {
                state
                    .autopilot_chat
                    .set_thread_name(thread_id.as_str(), Some(shell_title));
            }
            state.autopilot_chat.set_probe_thread_projection_state(
                thread_id.as_str(),
                Some(runtime_status),
                runtime_archived,
                true,
            );
            state.autopilot_chat.set_active_thread_transcript(
                thread_id.as_str(),
                probe_transcript_messages(snapshot.transcript.as_slice()),
            );
            hydrate_probe_pending_approvals(
                state,
                &thread_id,
                snapshot.pending_approvals.as_slice(),
            );
            sync_probe_turn_status(state, control);
            if state.uses_probe_runtime() {
                state
                    .autopilot_chat
                    .set_connection_status(state.probe_lane.lifecycle.label().to_string());
                state.autopilot_chat.last_error = None;
            }
        }
        ProbeLaneNotification::ChildSessionUpdated { session_id, child } => {
            let _ = state
                .autopilot_chat
                .record_probe_child_session_update_for_thread(
                    session_id.as_str(),
                    forge_child_session_card_from_probe(child),
                    current_epoch_millis(),
                );
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe delegated child: {} {}",
                child.title,
                forge_child_status_from_probe(child.status).display_label()
            ));
        }
        ProbeLaneNotification::WorkspaceStateUpdated {
            session_id,
            workspace_state,
            branch_state,
            delivery_state,
        } => {
            let _ = state
                .autopilot_chat
                .sync_probe_remote_session_projection_for_thread(
                    session_id.as_str(),
                    None,
                    workspace_state.as_ref(),
                    None,
                    current_epoch_millis(),
                );
            let delivery_watch_updated_at_ms = probe_delivery_watch_updated_at_ms(
                branch_state.as_ref(),
                delivery_state.as_ref(),
                current_epoch_millis(),
            );
            let _ = state
                .autopilot_chat
                .sync_probe_delivery_runtime_watch_for_thread(
                    session_id.as_str(),
                    branch_state.as_ref().map(|branch_state| {
                        forge_delivery_branch_watch_from_probe(
                            branch_state,
                            delivery_watch_updated_at_ms,
                        )
                    }),
                    delivery_state
                        .as_ref()
                        .map(forge_delivery_compare_watch_from_probe),
                    delivery_watch_updated_at_ms,
                );
            if let Some(compare_watch) = delivery_state
                .as_ref()
                .map(forge_delivery_compare_watch_from_probe)
            {
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "probe delivery watch: {}",
                    compare_watch.posture.display_label()
                ));
            }
        }
        ProbeLaneNotification::RuntimeProgress { session_id, event } => {
            apply_runtime_progress(state, session_id.as_str(), event);
        }
        ProbeLaneNotification::PendingApprovalsUpdated {
            session_id,
            approvals,
        } => {
            hydrate_probe_pending_approvals(state, session_id.as_str(), approvals.as_slice());
            if approvals.is_empty()
                && state
                    .autopilot_chat
                    .last_turn_status
                    .as_deref()
                    .is_some_and(|status| status.starts_with("paused"))
            {
                let next_status = if state
                    .autopilot_chat
                    .active_thread_status()
                    .is_some_and(|status| status.contains("queued"))
                {
                    String::from("running+queued")
                } else {
                    String::from("running")
                };
                state.autopilot_chat.set_turn_status(Some(next_status));
            }
        }
        ProbeLaneNotification::TurnQueued { response, control } => {
            let thread_id = response.turn.session_id.as_str();
            state
                .autopilot_chat
                .set_turn_status(Some(probe_control_status_label(control)));
            state.autopilot_chat.set_probe_thread_projection_state(
                thread_id,
                Some(probe_control_status_label(control)),
                probe_thread_is_archived(state, thread_id),
                true,
            );
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe queued turn: {} position={}",
                response.turn.turn_id,
                response.turn.queue_position.unwrap_or(0)
            ));
            let _ = state.autopilot_chat.maybe_record_probe_owner_transition(
                thread_id,
                crate::app_state::ForgeSharedSessionControlOwner::ProbeLocalAgent,
                "Queued a new Probe turn from the Autopilot shell.",
                "probe.turn_queued",
                current_epoch_millis(),
            );
        }
        ProbeLaneNotification::TurnInterrupted { response, control } => {
            let thread_id = response.session_id.as_str();
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("cancelled")));
            state.autopilot_chat.set_probe_thread_projection_state(
                thread_id,
                Some(probe_control_status_label(control)),
                probe_thread_is_archived(state, thread_id),
                true,
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe interrupt: {}", response.message));
            let _ = state.autopilot_chat.maybe_record_probe_owner_transition(
                thread_id,
                crate::app_state::ForgeSharedSessionControlOwner::HumanLocal,
                "Interrupted the active Probe turn and returned control to the local human.",
                "probe.turn_interrupted",
                current_epoch_millis(),
            );
        }
        ProbeLaneNotification::QueuedTurnCancelled { response, control } => {
            let thread_id = response.session_id.as_str();
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("cancelled")));
            state.autopilot_chat.set_probe_thread_projection_state(
                thread_id,
                Some(probe_control_status_label(control)),
                probe_thread_is_archived(state, thread_id),
                true,
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe queue cancel: {}", response.message));
            let _ = state.autopilot_chat.maybe_record_probe_owner_transition(
                thread_id,
                crate::app_state::ForgeSharedSessionControlOwner::HumanLocal,
                "Cancelled the queued Probe turn and kept control in the local shell.",
                "probe.queued_turn_cancelled",
                current_epoch_millis(),
            );
        }
    }
    state.record_probe_notification(notification);
}

fn apply_runtime_progress(state: &mut RenderState, session_id: &str, event: &RuntimeProgressEvent) {
    let turn_id = probe_live_turn_id(session_id);
    match event {
        RuntimeProgressEvent::TurnStarted { .. } => {
            state.autopilot_chat.mark_turn_started(turn_id);
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("running")));
            state.autopilot_chat.set_probe_thread_projection_state(
                session_id,
                Some(String::from("running")),
                probe_thread_is_archived(state, session_id),
                true,
            );
            let _ = state.autopilot_chat.maybe_record_probe_owner_transition(
                session_id,
                crate::app_state::ForgeSharedSessionControlOwner::ProbeLocalAgent,
                "Probe resumed execution in the attached shared session.",
                "probe.turn_started",
                current_epoch_millis(),
            );
        }
        RuntimeProgressEvent::AssistantDelta { delta, .. } => {
            state
                .autopilot_chat
                .append_turn_delta_for_turn(turn_id.as_str(), delta);
        }
        RuntimeProgressEvent::AssistantSnapshot { snapshot, .. } => {
            state
                .autopilot_chat
                .set_turn_message_for_turn(turn_id.as_str(), snapshot);
        }
        RuntimeProgressEvent::ModelRequestFailed { error, .. } => {
            state
                .autopilot_chat
                .mark_turn_error_for(turn_id.as_str(), error.clone());
        }
        RuntimeProgressEvent::AssistantTurnCommitted { assistant_text, .. } => {
            state
                .autopilot_chat
                .set_turn_message_for_turn(turn_id.as_str(), assistant_text);
            state
                .autopilot_chat
                .mark_turn_completed_for(turn_id.as_str());
        }
        RuntimeProgressEvent::ToolExecutionStarted { tool_name, .. } => {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe tool started: {tool_name}"));
        }
        RuntimeProgressEvent::ToolExecutionCompleted { tool, .. } => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe tool completed: {} ({:?})",
                tool.name, tool.tool_execution.risk_class
            ));
        }
        RuntimeProgressEvent::ToolRefused { tool, .. } => {
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("refused")));
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe tool refused: {}", tool.name));
        }
        RuntimeProgressEvent::ToolPaused { tool, .. } => {
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("paused")));
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe tool paused: {}", tool.name));
        }
        RuntimeProgressEvent::AssistantStreamFinished { .. }
        | RuntimeProgressEvent::ModelRequestStarted { .. }
        | RuntimeProgressEvent::AssistantStreamStarted { .. }
        | RuntimeProgressEvent::TimeToFirstTokenObserved { .. }
        | RuntimeProgressEvent::ToolCallDelta { .. }
        | RuntimeProgressEvent::ToolCallRequested { .. } => {}
    }
}

fn hydrate_probe_pending_approvals(
    state: &mut RenderState,
    thread_id: &str,
    approvals: &[probe_protocol::session::PendingToolApproval],
) {
    state.autopilot_chat.pending_command_approvals = approvals
        .iter()
        .map(|approval| crate::app_state::AutopilotApprovalRequest {
            request_id: codex_client::AppServerRequestId::String(format!(
                "probe-approval-{}",
                approval.tool_call_id
            )),
            thread_id: thread_id.to_string(),
            turn_id: probe_live_turn_id(thread_id),
            item_id: approval.tool_call_id.clone(),
            reason: approval
                .reason
                .clone()
                .or_else(|| Some(format!("{} {:?}", approval.tool_name, approval.risk_class))),
            command: Some(approval.tool_name.clone()),
            cwd: Some(thread_id.to_string()),
        })
        .collect();
}

fn sync_probe_turn_status(
    state: &mut RenderState,
    control: &probe_protocol::runtime::InspectSessionTurnsResponse,
) {
    let status = probe_control_status_label(control);
    if !status.is_empty() {
        state.autopilot_chat.set_turn_status(Some(status));
    }
}

fn session_metadata_to_thread_entry(session: &ProbeListedSession) -> AutopilotThreadListEntry {
    AutopilotThreadListEntry {
        thread_id: session.session.id.as_str().to_string(),
        thread_name: Some(session.session.title.clone()),
        preview: session.session.cwd.display().to_string(),
        status: Some(match session.session.state {
            probe_protocol::session::SessionState::Active => String::from("idle"),
            probe_protocol::session::SessionState::Archived => String::from("archived"),
        }),
        loaded: true,
        cwd: Some(session.session.cwd.display().to_string()),
        path: Some(session.session.transcript_path.display().to_string()),
        created_at: session.session.created_at_ms as i64,
        updated_at: session.session.updated_at_ms as i64,
    }
}

fn probe_session_preview(
    session: &SessionMetadata,
    transcript: &[TranscriptEvent],
) -> Option<String> {
    transcript
        .iter()
        .flat_map(|event| event.turn.items.iter())
        .find(|item| matches!(item.kind, TranscriptItemKind::UserMessage))
        .map(|item| item.text.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(session.cwd.display().to_string()))
}

fn forge_child_status_from_probe(status: SessionChildStatus) -> ForgeDelegatedChildSessionStatus {
    match status {
        SessionChildStatus::Idle => ForgeDelegatedChildSessionStatus::Idle,
        SessionChildStatus::Running => ForgeDelegatedChildSessionStatus::Running,
        SessionChildStatus::Queued => ForgeDelegatedChildSessionStatus::Queued,
        SessionChildStatus::ApprovalPaused => ForgeDelegatedChildSessionStatus::ApprovalPaused,
        SessionChildStatus::Completed => ForgeDelegatedChildSessionStatus::Completed,
        SessionChildStatus::Failed => ForgeDelegatedChildSessionStatus::Failed,
        SessionChildStatus::Cancelled => ForgeDelegatedChildSessionStatus::Cancelled,
        SessionChildStatus::TimedOut => ForgeDelegatedChildSessionStatus::TimedOut,
    }
}

fn forge_child_delivery_status_from_probe(
    status: SessionDeliveryStatus,
) -> ForgeDelegatedChildDeliveryStatus {
    match status {
        SessionDeliveryStatus::NeedsCommit => ForgeDelegatedChildDeliveryStatus::NeedsCommit,
        SessionDeliveryStatus::LocalOnly => ForgeDelegatedChildDeliveryStatus::LocalOnly,
        SessionDeliveryStatus::NeedsPush => ForgeDelegatedChildDeliveryStatus::NeedsPush,
        SessionDeliveryStatus::Synced => ForgeDelegatedChildDeliveryStatus::Synced,
        SessionDeliveryStatus::Diverged => ForgeDelegatedChildDeliveryStatus::Diverged,
    }
}

fn forge_delivery_branch_watch_from_probe(
    branch_state: &SessionBranchState,
    refreshed_at_epoch_ms: u64,
) -> ForgeDeliveryBranchWatch {
    ForgeDeliveryBranchWatch {
        repo_root: branch_state.repo_root.display().to_string(),
        head_ref: branch_state.head_ref.clone(),
        head_commit: branch_state.head_commit.clone(),
        detached_head: branch_state.detached_head,
        working_tree_dirty: branch_state.working_tree_dirty,
        upstream_ref: branch_state.upstream_ref.clone(),
        ahead_by: branch_state.ahead_by,
        behind_by: branch_state.behind_by,
        refreshed_at_epoch_ms,
    }
}

fn forge_delivery_compare_watch_from_probe(
    delivery_state: &SessionDeliveryState,
) -> ForgeDeliveryCompareWatch {
    ForgeDeliveryCompareWatch {
        posture: match delivery_state.status {
            SessionDeliveryStatus::NeedsCommit => ForgeDeliveryComparePosture::NeedsCommit,
            SessionDeliveryStatus::LocalOnly => ForgeDeliveryComparePosture::LocalOnly,
            SessionDeliveryStatus::NeedsPush => ForgeDeliveryComparePosture::NeedsPush,
            SessionDeliveryStatus::Synced => ForgeDeliveryComparePosture::Synced,
            SessionDeliveryStatus::Diverged => ForgeDeliveryComparePosture::Diverged,
        },
        branch_name: delivery_state.branch_name.clone(),
        remote_tracking_ref: delivery_state.remote_tracking_ref.clone(),
        compare_ref: delivery_state.compare_ref.clone(),
        refreshed_at_epoch_ms: delivery_state.updated_at_ms,
    }
}

fn probe_delivery_watch_updated_at_ms(
    branch_state: Option<&SessionBranchState>,
    delivery_state: Option<&SessionDeliveryState>,
    fallback: u64,
) -> u64 {
    delivery_state
        .map(|state| state.updated_at_ms)
        .or_else(|| branch_state.map(|_| fallback))
        .unwrap_or(fallback)
}

fn forge_child_session_card_from_probe(
    child: &SessionChildSummary,
) -> ForgeDelegatedChildSessionCard {
    ForgeDelegatedChildSessionCard {
        child_session_id: child.session_id.as_str().to_string(),
        title: child.title.clone(),
        cwd: child.cwd.clone(),
        archived_in_probe: child.state == SessionState::Archived,
        status: forge_child_status_from_probe(child.status),
        initiator_display_name: child
            .initiator
            .as_ref()
            .and_then(|initiator| initiator.display_name.clone()),
        initiator_client_name: child
            .initiator
            .as_ref()
            .map(|initiator| initiator.client_name.clone()),
        purpose: child.purpose.clone(),
        parent_turn_id: child.parent_turn_id.clone(),
        parent_turn_index: child.parent_turn_index,
        closure_status: child
            .closure
            .as_ref()
            .map(|closure| forge_child_status_from_probe(closure.status)),
        closure_delivery_status: child.closure.as_ref().and_then(|closure| {
            closure
                .delivery_status
                .map(forge_child_delivery_status_from_probe)
        }),
        closure_branch_name: child
            .closure
            .as_ref()
            .and_then(|closure| closure.branch_name.clone()),
        closure_head_commit: child
            .closure
            .as_ref()
            .and_then(|closure| closure.head_commit.clone()),
        closure_compare_ref: child
            .closure
            .as_ref()
            .and_then(|closure| closure.compare_ref.clone()),
        created_at_epoch_ms: child.created_at_ms,
        updated_at_epoch_ms: child.updated_at_ms,
    }
}

fn probe_control_status_label(
    control: &probe_protocol::runtime::InspectSessionTurnsResponse,
) -> String {
    if let Some(active_turn) = control.active_turn.as_ref() {
        let active_status = if active_turn.awaiting_approval {
            "paused"
        } else {
            "running"
        };
        return if control.queued_turns.is_empty() {
            active_status.to_string()
        } else {
            format!("{active_status}+queued")
        };
    }
    if !control.queued_turns.is_empty() {
        return String::from("queued");
    }
    if let Some(recent_turn) = control.recent_turns.first() {
        return match recent_turn.status {
            QueuedTurnStatus::Queued => String::from("queued"),
            QueuedTurnStatus::Running => {
                if recent_turn.awaiting_approval {
                    String::from("paused")
                } else {
                    String::from("running")
                }
            }
            QueuedTurnStatus::Completed => String::from("completed"),
            QueuedTurnStatus::Failed => String::from("failed"),
            QueuedTurnStatus::Cancelled => String::from("cancelled"),
            QueuedTurnStatus::TimedOut => String::from("timed_out"),
        };
    }
    String::from("idle")
}

fn probe_thread_is_archived(state: &RenderState, thread_id: &str) -> bool {
    state
        .autopilot_chat
        .thread_metadata
        .get(thread_id)
        .map(|metadata| metadata.probe_archived)
        .unwrap_or(false)
}

fn sync_probe_attached_thread_projection(
    state: &mut RenderState,
    previous_active_session_id: Option<&str>,
    next_active_session_id: Option<&str>,
) {
    let mut session_ids = Vec::<String>::new();
    for session_id in [previous_active_session_id, next_active_session_id]
        .into_iter()
        .flatten()
    {
        if !session_ids.iter().any(|existing| existing == session_id) {
            session_ids.push(session_id.to_string());
        }
    }
    for session_id in session_ids {
        let Some(metadata) = state
            .autopilot_chat
            .thread_metadata
            .get(&session_id)
            .cloned()
        else {
            continue;
        };
        if metadata.probe_runtime_status.is_none() && !metadata.probe_archived {
            continue;
        }
        state.autopilot_chat.set_probe_thread_projection_state(
            session_id.as_str(),
            metadata.probe_runtime_status,
            metadata.probe_archived,
            next_active_session_id == Some(session_id.as_str()),
        );
    }
}

fn probe_transcript_messages(transcript: &[TranscriptEvent]) -> Vec<(AutopilotRole, String)> {
    let mut messages = Vec::new();
    for event in transcript {
        let mut user_lines = Vec::new();
        let mut assistant_lines = Vec::new();
        for item in &event.turn.items {
            let text = item.text.trim();
            if text.is_empty() {
                continue;
            }
            match item.kind {
                TranscriptItemKind::UserMessage => user_lines.push(text.to_string()),
                TranscriptItemKind::AssistantMessage => assistant_lines.push(text.to_string()),
                TranscriptItemKind::Note => assistant_lines.push(format!("[note] {text}")),
                TranscriptItemKind::ToolCall => {
                    let tool_name = item.name.as_deref().unwrap_or("tool");
                    assistant_lines.push(format!("[tool] {tool_name}: {text}"));
                }
                TranscriptItemKind::ToolResult => {
                    let tool_name = item.name.as_deref().unwrap_or("tool");
                    assistant_lines.push(format!("[tool-result] {tool_name}: {text}"));
                }
            }
        }
        if !user_lines.is_empty() {
            messages.push((AutopilotRole::User, user_lines.join("\n\n")));
        }
        if !assistant_lines.is_empty() {
            messages.push((AutopilotRole::Codex, assistant_lines.join("\n\n")));
        }
    }
    messages
}
