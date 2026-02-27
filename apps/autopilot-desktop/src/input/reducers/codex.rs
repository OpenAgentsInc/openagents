use crate::app_state::{PaneLoadState, RenderState};
use crate::codex_lane::CodexLaneCommand;
use crate::codex_lane::{
    CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneNotification,
    CodexLaneSnapshot,
};
use codex_client::{SkillsListExtraRootsForCwd, SkillsListParams};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: CodexLaneSnapshot) {
    state
        .autopilot_chat
        .set_connection_status(snapshot.lifecycle.label().to_string());
    if let Some(thread_id) = snapshot.active_thread_id.as_ref() {
        state.autopilot_chat.ensure_thread(thread_id.clone());
    }
    if let Some(error) = snapshot.last_error.as_ref() {
        state.autopilot_chat.last_error = Some(error.clone());
    }
    state.codex_lane = snapshot;
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(state: &mut RenderState, response: CodexLaneCommandResponse) {
    if response.status != CodexLaneCommandStatus::Accepted {
        state.sync_health.last_error = response
            .error
            .as_ref()
            .map(|error| format!("codex {}: {error}", response.status.label()));
        if response.command == CodexLaneCommandKind::TurnStart {
            state.autopilot_chat.mark_pending_turn_dispatch_failed(
                response
                    .error
                    .clone()
                    .unwrap_or_else(|| "turn/start rejected".to_string()),
            );
        } else if response.command == CodexLaneCommandKind::SkillsList {
            state.skill_registry.load_state = PaneLoadState::Error;
            state.skill_registry.last_error = response
                .error
                .clone()
                .or_else(|| Some("codex skills/list failed".to_string()));
        }
    } else if response.command == CodexLaneCommandKind::TurnStart {
        state.autopilot_chat.last_error = None;
    } else if response.command == CodexLaneCommandKind::SkillsList {
        state.skill_registry.last_error = None;
    } else if response.command == CodexLaneCommandKind::SkillsConfigWrite {
        state.skill_registry.last_error = None;
        state.skill_registry.last_action = Some("skills/config/write applied; refreshing list".to_string());
        queue_skills_list_refresh(state);
    }
    state.sync_health.last_action = Some(format!(
        "codex {} {}",
        response.command.label(),
        response.status.label()
    ));
    state.record_codex_command_response(response);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn queue_skills_list_refresh(state: &mut RenderState) {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            state.skill_registry.last_error =
                Some(format!("Failed to refresh skills/list after config write: {error}"));
            state.skill_registry.load_state = PaneLoadState::Error;
            return;
        }
    };
    let repo_skills_root = cwd.join("skills");
    if !repo_skills_root.is_absolute() || !repo_skills_root.exists() {
        state.skill_registry.last_error = Some(format!(
            "Cannot refresh codex skills/list; invalid root {}",
            repo_skills_root.display()
        ));
        state.skill_registry.load_state = PaneLoadState::Error;
        return;
    }

    let params = SkillsListParams {
        cwds: vec![cwd.clone()],
        force_reload: true,
        per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
            cwd,
            extra_user_roots: vec![repo_skills_root],
        }]),
    };
    if let Err(error) = state.queue_codex_command(CodexLaneCommand::SkillsList(params)) {
        state.skill_registry.last_error = Some(error);
        state.skill_registry.load_state = PaneLoadState::Error;
    }
}

pub(super) fn apply_notification(state: &mut RenderState, notification: CodexLaneNotification) {
    let stored = notification.clone();
    match notification {
        CodexLaneNotification::SkillsListLoaded { entries } => {
            state.skill_registry.source = "codex".to_string();
            state.skill_registry.discovered_skills.clear();
            state.skill_registry.discovery_errors.clear();

            for entry in entries {
                if state.skill_registry.repo_skills_root.is_none() {
                    state.skill_registry.repo_skills_root = Some(entry.cwd.clone());
                }
                for skill in entry.skills {
                    state
                        .skill_registry
                        .discovered_skills
                        .push(crate::app_state::SkillRegistryDiscoveredSkill {
                            name: skill.name,
                            path: skill.path,
                            scope: skill.scope,
                            enabled: skill.enabled,
                            interface_display_name: skill.interface_display_name,
                            dependency_count: skill.dependency_count,
                        });
                }
                state.skill_registry.discovery_errors.extend(entry.errors);
            }

            state.skill_registry.selected_skill_index = if state.skill_registry.discovered_skills.is_empty() {
                None
            } else {
                Some(0)
            };
            state.skill_registry.load_state = PaneLoadState::Ready;
            state.skill_registry.last_action = Some(format!(
                "Loaded {} codex skills",
                state.skill_registry.discovered_skills.len()
            ));
            if state.skill_registry.discovery_errors.is_empty() {
                state.skill_registry.last_error = None;
            } else {
                state.skill_registry.last_error = state.skill_registry.discovery_errors.first().cloned();
            }
        }
        CodexLaneNotification::ThreadListLoaded { thread_ids } => {
            state.autopilot_chat.set_threads(thread_ids);
        }
        CodexLaneNotification::ThreadSelected { thread_id }
        | CodexLaneNotification::ThreadStarted { thread_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
        }
        CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
            state.autopilot_chat.mark_turn_started(turn_id);
        }
        CodexLaneNotification::AgentMessageDelta { delta, .. } => {
            state.autopilot_chat.append_turn_delta(&delta);
        }
        CodexLaneNotification::TurnCompleted { .. } => {
            state.autopilot_chat.mark_turn_completed();
        }
        CodexLaneNotification::TurnError { message, .. } => {
            state.autopilot_chat.mark_turn_error(message);
        }
        CodexLaneNotification::ServerRequest { .. } | CodexLaneNotification::Raw { .. } => {}
    }

    state.sync_health.last_action = Some("codex notification received".to_string());
    state.record_codex_notification(stored);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}
