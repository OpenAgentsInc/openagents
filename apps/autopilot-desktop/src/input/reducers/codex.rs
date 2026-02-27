use crate::app_state::{PaneLoadState, RenderState};
use crate::codex_lane::CodexLaneCommand;
use crate::codex_lane::{
    CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneNotification,
    CodexLaneSnapshot,
};
use codex_client::{SkillsListExtraRootsForCwd, SkillsListParams, ThreadStartParams};

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
    let response_error = response.error.clone();
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
        } else if response.command == CodexLaneCommandKind::ThreadResume {
            let message = response
                .error
                .clone()
                .unwrap_or_else(|| "thread/resume rejected".to_string());
            eprintln!(
                "codex thread/resume rejected seq={} active_thread={:?} error={}",
                response.command_seq, state.autopilot_chat.active_thread_id, message
            );
            let active_thread_id = state.autopilot_chat.active_thread_id.clone();
            let is_missing_rollout = message.contains("no rollout found for thread id");
            if is_missing_rollout {
                if let Some(thread_id) = active_thread_id.as_ref() {
                    state.autopilot_chat.remove_thread(thread_id);
                }
                state.autopilot_chat.last_error = Some(
                    "Selected thread is missing a rollout on disk; removed stale entry and started a new thread."
                        .to_string(),
                );
                queue_new_thread(state);
            } else {
                state.autopilot_chat.last_error = Some(message);
            }
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
        state.skill_registry.last_action =
            Some("skills/config/write applied; refreshing list".to_string());
        queue_skills_list_refresh(state);
    }

    match response.command {
        CodexLaneCommandKind::AccountRead
        | CodexLaneCommandKind::AccountLoginStart
        | CodexLaneCommandKind::AccountLoginCancel
        | CodexLaneCommandKind::AccountLogout
        | CodexLaneCommandKind::AccountRateLimitsRead => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_account.load_state = PaneLoadState::Ready;
                state.codex_account.last_error = None;
                state.codex_account.last_action =
                    Some(format!("{} accepted", response.command.label()));
                if response.command == CodexLaneCommandKind::AccountLoginCancel {
                    state.codex_account.pending_login_id = None;
                    state.codex_account.pending_login_url = None;
                }
            } else {
                state.codex_account.load_state = PaneLoadState::Error;
                state.codex_account.last_error = response_error
                    .clone()
                    .or_else(|| Some(format!("{} rejected", response.command.label())));
            }
        }
        CodexLaneCommandKind::ModelList => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_models.load_state = PaneLoadState::Ready;
                state.codex_models.last_error = None;
                state.codex_models.last_action = Some("model/list accepted".to_string());
            } else {
                state.codex_models.load_state = PaneLoadState::Error;
                state.codex_models.last_error = response_error
                    .clone()
                    .or_else(|| Some("model/list failed".to_string()));
            }
        }
        CodexLaneCommandKind::ConfigRead
        | CodexLaneCommandKind::ConfigRequirementsRead
        | CodexLaneCommandKind::ConfigValueWrite
        | CodexLaneCommandKind::ConfigBatchWrite
        | CodexLaneCommandKind::ExternalAgentConfigDetect
        | CodexLaneCommandKind::ExternalAgentConfigImport => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.last_error = None;
                state.codex_config.last_action =
                    Some(format!("{} accepted", response.command.label()));
            } else {
                state.codex_config.load_state = PaneLoadState::Error;
                state.codex_config.last_error = response_error
                    .clone()
                    .or_else(|| Some(format!("{} failed", response.command.label())));
            }
        }
        _ => {}
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
            state.skill_registry.last_error = Some(format!(
                "Failed to refresh skills/list after config write: {error}"
            ));
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

fn queue_new_thread(state: &mut RenderState) {
    let cwd = std::env::current_dir().ok();
    let command = CodexLaneCommand::ThreadStart(ThreadStartParams {
        model: Some(state.autopilot_chat.current_model().to_string()),
        model_provider: None,
        cwd: cwd.and_then(|value| value.into_os_string().into_string().ok()),
        approval_policy: None,
        sandbox: None,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(format!(
            "Failed to start replacement thread after stale resume: {error}"
        ));
    }
}

pub(super) fn apply_notification(state: &mut RenderState, notification: CodexLaneNotification) {
    let stored = notification.clone();
    match notification {
        CodexLaneNotification::ModelsLoaded {
            models,
            default_model,
        } => {
            state.autopilot_chat.set_models(models, default_model);
        }
        CodexLaneNotification::ModelCatalogLoaded {
            entries,
            include_hidden,
            default_model,
        } => {
            let models = entries
                .iter()
                .map(|entry| entry.model.clone())
                .collect::<Vec<_>>();
            state.autopilot_chat.set_models(models, default_model);
            state.codex_models.load_state = PaneLoadState::Ready;
            state.codex_models.include_hidden = include_hidden;
            state.codex_models.entries = entries
                .into_iter()
                .map(|entry| crate::app_state::CodexModelCatalogEntryState {
                    model: entry.model,
                    display_name: entry.display_name,
                    description: entry.description,
                    hidden: entry.hidden,
                    is_default: entry.is_default,
                    default_reasoning_effort: entry.default_reasoning_effort,
                    supported_reasoning_efforts: entry.supported_reasoning_efforts,
                })
                .collect();
            state.codex_models.last_error = None;
            state.codex_models.last_action = Some(format!(
                "Loaded {} model catalog entries",
                state.codex_models.entries.len()
            ));
        }
        CodexLaneNotification::ModelRerouted {
            thread_id,
            turn_id,
            from_model,
            to_model,
            reason,
        } => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "model rerouted {} -> {} ({})",
                from_model, to_model, reason
            ));
            state.codex_models.last_reroute = Some(format!(
                "thread={} turn={} {} -> {} ({})",
                thread_id, turn_id, from_model, to_model, reason
            ));
            state.codex_models.last_action =
                Some("Received model/rerouted notification".to_string());
        }
        CodexLaneNotification::AccountLoaded {
            summary,
            requires_openai_auth,
        } => {
            state.codex_account.load_state = PaneLoadState::Ready;
            state.codex_account.account_summary = summary;
            state.codex_account.requires_openai_auth = requires_openai_auth;
            state.codex_account.last_action = Some("Loaded account state".to_string());
            state.codex_account.last_error = None;
        }
        CodexLaneNotification::AccountRateLimitsLoaded { summary } => {
            state.codex_account.load_state = PaneLoadState::Ready;
            state.codex_account.rate_limits_summary = Some(summary);
            state.codex_account.last_action = Some("Loaded account rate limits".to_string());
            state.codex_account.last_error = None;
        }
        CodexLaneNotification::AccountUpdated { auth_mode } => {
            state.codex_account.load_state = PaneLoadState::Ready;
            state.codex_account.auth_mode = auth_mode;
            state.codex_account.last_action = Some("Received account/updated".to_string());
            state.codex_account.last_error = None;
        }
        CodexLaneNotification::AccountLoginStarted { login_id, auth_url } => {
            state.codex_account.load_state = PaneLoadState::Ready;
            state.codex_account.pending_login_id = login_id;
            state.codex_account.pending_login_url = auth_url;
            state.codex_account.last_action =
                Some("Login started; complete auth in browser".to_string());
            state.codex_account.last_error = None;
        }
        CodexLaneNotification::AccountLoginCompleted {
            login_id,
            success,
            error,
        } => {
            state.codex_account.load_state = if success {
                PaneLoadState::Ready
            } else {
                PaneLoadState::Error
            };
            let completed_login = login_id.unwrap_or_else(|| "unknown".to_string());
            state.codex_account.last_action = Some(format!(
                "Login completed for {} (success={})",
                completed_login, success
            ));
            state.codex_account.last_error = if success { None } else { error };
            state.codex_account.pending_login_id = None;
            state.codex_account.pending_login_url = None;
        }
        CodexLaneNotification::ConfigLoaded { config } => {
            state.codex_config.load_state = PaneLoadState::Ready;
            state.codex_config.config_json = config;
            state.codex_config.last_action = Some("Loaded config/read".to_string());
            state.codex_config.last_error = None;
        }
        CodexLaneNotification::ConfigRequirementsLoaded { requirements } => {
            state.codex_config.load_state = PaneLoadState::Ready;
            state.codex_config.requirements_json = requirements;
            state.codex_config.last_action = Some("Loaded config requirements".to_string());
            state.codex_config.last_error = None;
        }
        CodexLaneNotification::ConfigWriteApplied { status, version } => {
            state.codex_config.load_state = PaneLoadState::Ready;
            state.codex_config.last_action = Some(format!(
                "Config write applied status={} version={}",
                status, version
            ));
            state.codex_config.last_error = None;
        }
        CodexLaneNotification::ExternalAgentConfigDetected { count } => {
            state.codex_config.load_state = PaneLoadState::Ready;
            state.codex_config.detected_external_configs = count;
            state.codex_config.last_action =
                Some(format!("Detected {} external agent configs", count));
            state.codex_config.last_error = None;
        }
        CodexLaneNotification::ExternalAgentConfigImported => {
            state.codex_config.load_state = PaneLoadState::Ready;
            state.codex_config.last_action =
                Some("External agent config import completed".to_string());
            state.codex_config.last_error = None;
        }
        CodexLaneNotification::SkillsListLoaded { entries } => {
            state.skill_registry.source = "codex".to_string();
            state.skill_registry.discovered_skills.clear();
            state.skill_registry.discovery_errors.clear();

            for entry in entries {
                if state.skill_registry.repo_skills_root.is_none() {
                    state.skill_registry.repo_skills_root = Some(entry.cwd.clone());
                }
                for skill in entry.skills {
                    state.skill_registry.discovered_skills.push(
                        crate::app_state::SkillRegistryDiscoveredSkill {
                            name: skill.name,
                            path: skill.path,
                            scope: skill.scope,
                            enabled: skill.enabled,
                            interface_display_name: skill.interface_display_name,
                            dependency_count: skill.dependency_count,
                        },
                    );
                }
                state.skill_registry.discovery_errors.extend(entry.errors);
            }

            state.skill_registry.selected_skill_index =
                if state.skill_registry.discovered_skills.is_empty() {
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
                state.skill_registry.last_error =
                    state.skill_registry.discovery_errors.first().cloned();
            }
        }
        CodexLaneNotification::ThreadListLoaded { entries } => {
            eprintln!("codex thread/list loaded {} entries", entries.len());
            state.autopilot_chat.set_thread_entries(
                entries
                    .into_iter()
                    .map(|entry| crate::app_state::AutopilotThreadListEntry {
                        thread_id: entry.thread_id,
                        thread_name: entry.thread_name,
                        status: entry.status,
                        loaded: entry.loaded,
                        cwd: entry.cwd,
                        path: entry.path,
                    })
                    .collect(),
            );
        }
        CodexLaneNotification::ThreadLoadedListLoaded { thread_ids } => {
            state.autopilot_chat.set_thread_loaded_ids(&thread_ids);
        }
        CodexLaneNotification::ThreadSelected { thread_id }
        | CodexLaneNotification::ThreadStarted { thread_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
        }
        CodexLaneNotification::ThreadStatusChanged { thread_id, status } => {
            state
                .autopilot_chat
                .set_thread_status(&thread_id, Some(status.clone()));
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("thread status: {thread_id} => {status}"));
        }
        CodexLaneNotification::ThreadArchived { thread_id } => {
            state
                .autopilot_chat
                .set_thread_status(&thread_id, Some("archived".to_string()));
            if state.autopilot_chat.thread_filter_archived == Some(false) {
                state.autopilot_chat.remove_thread(&thread_id);
            }
        }
        CodexLaneNotification::ThreadUnarchived { thread_id } => {
            state
                .autopilot_chat
                .set_thread_status(&thread_id, Some("idle".to_string()));
            state.autopilot_chat.ensure_thread(thread_id);
        }
        CodexLaneNotification::ThreadClosed { thread_id } => {
            state.autopilot_chat.remove_thread(&thread_id);
        }
        CodexLaneNotification::ThreadNameUpdated {
            thread_id,
            thread_name,
        } => {
            state
                .autopilot_chat
                .set_thread_name(&thread_id, thread_name);
        }
        CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("turn started: {turn_id}"));
            state.autopilot_chat.mark_turn_started(turn_id);
        }
        CodexLaneNotification::ItemStarted {
            thread_id,
            turn_id,
            item_id,
            item_type,
        } => {
            state.autopilot_chat.ensure_thread(thread_id);
            state.autopilot_chat.record_turn_timeline_event(format!(
                "item started: turn={turn_id} id={} type={}",
                item_id.as_deref().unwrap_or("n/a"),
                item_type.as_deref().unwrap_or("n/a")
            ));
        }
        CodexLaneNotification::ItemCompleted {
            thread_id,
            turn_id,
            item_id,
            item_type,
        } => {
            state.autopilot_chat.ensure_thread(thread_id);
            state.autopilot_chat.record_turn_timeline_event(format!(
                "item completed: turn={turn_id} id={} type={}",
                item_id.as_deref().unwrap_or("n/a"),
                item_type.as_deref().unwrap_or("n/a")
            ));
        }
        CodexLaneNotification::AgentMessageDelta { item_id, delta, .. } => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "agent delta: item={} chars={}",
                item_id,
                delta.chars().count()
            ));
            state.autopilot_chat.append_turn_delta(&delta);
        }
        CodexLaneNotification::TurnCompleted {
            status,
            error_message,
            ..
        } => {
            state.autopilot_chat.set_turn_status(status.clone());
            match status.as_deref() {
                Some("failed") => {
                    state.autopilot_chat.mark_turn_error(
                        error_message.unwrap_or_else(|| "Turn failed".to_string()),
                    );
                }
                Some("interrupted") => {
                    state.autopilot_chat.mark_turn_completed();
                    state
                        .autopilot_chat
                        .set_turn_status(Some("interrupted".to_string()));
                    state
                        .autopilot_chat
                        .record_turn_timeline_event("turn interrupted");
                }
                _ => {
                    state.autopilot_chat.mark_turn_completed();
                }
            }
        }
        CodexLaneNotification::TurnDiffUpdated { diff, .. } => {
            state.autopilot_chat.set_turn_diff(Some(diff));
        }
        CodexLaneNotification::TurnPlanUpdated {
            explanation, plan, ..
        } => {
            state.autopilot_chat.set_turn_plan(
                explanation,
                plan.into_iter()
                    .map(|step| crate::app_state::AutopilotTurnPlanStep {
                        step: step.step,
                        status: step.status,
                    })
                    .collect(),
            );
        }
        CodexLaneNotification::ThreadTokenUsageUpdated {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            ..
        } => {
            state
                .autopilot_chat
                .set_token_usage(input_tokens, cached_input_tokens, output_tokens);
        }
        CodexLaneNotification::TurnError { message, .. } => {
            state.autopilot_chat.mark_turn_error(message);
        }
        CodexLaneNotification::CommandApprovalRequested {
            request_id,
            request,
        } => {
            state.autopilot_chat.enqueue_command_approval(
                crate::app_state::AutopilotApprovalRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    item_id: request.item_id,
                    reason: request.reason,
                    command: request.command,
                    cwd: request.cwd,
                },
            );
            state
                .autopilot_chat
                .record_turn_timeline_event("command approval requested");
        }
        CodexLaneNotification::FileChangeApprovalRequested {
            request_id,
            request,
        } => {
            state.autopilot_chat.enqueue_file_change_approval(
                crate::app_state::AutopilotFileChangeApprovalRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    item_id: request.item_id,
                    reason: request.reason,
                    grant_root: request.grant_root,
                },
            );
            state
                .autopilot_chat
                .record_turn_timeline_event("file-change approval requested");
        }
        CodexLaneNotification::ToolCallRequested {
            request_id,
            request,
        } => {
            state
                .autopilot_chat
                .enqueue_tool_call(crate::app_state::AutopilotToolCallRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    call_id: request.call_id,
                    tool: request.tool,
                    arguments: request.arguments,
                });
            state
                .autopilot_chat
                .record_turn_timeline_event("tool call requested");
        }
        CodexLaneNotification::ToolUserInputRequested {
            request_id,
            request,
        } => {
            state.autopilot_chat.enqueue_tool_user_input(
                crate::app_state::AutopilotToolUserInputRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    item_id: request.item_id,
                    questions: request
                        .questions
                        .into_iter()
                        .map(
                            |question| crate::app_state::AutopilotToolUserInputQuestion {
                                id: question.id,
                                header: question.header,
                                question: question.question,
                                options: question.options,
                            },
                        )
                        .collect(),
                },
            );
            state
                .autopilot_chat
                .record_turn_timeline_event("tool user-input requested");
        }
        CodexLaneNotification::AuthTokensRefreshRequested {
            request_id,
            request,
        } => {
            state.autopilot_chat.enqueue_auth_refresh(
                crate::app_state::AutopilotAuthRefreshRequest {
                    request_id,
                    reason: request.reason,
                    previous_account_id: request.previous_account_id,
                },
            );
            state
                .autopilot_chat
                .record_turn_timeline_event("auth token refresh requested");
        }
        CodexLaneNotification::ServerRequest { .. } | CodexLaneNotification::Raw { .. } => {}
    }

    state.sync_health.last_action = Some("codex notification received".to_string());
    state.record_codex_notification(stored);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}
