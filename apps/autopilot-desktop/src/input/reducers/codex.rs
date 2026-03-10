use crate::app_state::{CadBuildFailureClass, CadBuildSessionPhase, PaneLoadState, RenderState};
use crate::codex_lane::CodexLaneCommand;
use crate::codex_lane::{
    CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneLifecycle,
    CodexLaneNotification, CodexLaneSnapshot, CodexThreadTranscriptRole,
};
use codex_client::{SkillsListExtraRootsForCwd, SkillsListParams, ThreadStartParams};

const CAD_TOOL_RESPONSE_SUBMIT_RETRY_LIMIT: u8 = 2;

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn advance_completed_turn_labor_pipeline(state: &mut RenderState, turn_id: &str) {
    let is_labor_bound = state
        .autopilot_chat
        .turn_metadata_for(turn_id)
        .and_then(|metadata| metadata.labor_binding.as_ref())
        .is_some();
    if !is_labor_bound {
        return;
    }

    let had_submission = state
        .autopilot_chat
        .turn_labor_submission_for(turn_id)
        .is_some();
    match state
        .autopilot_chat
        .assemble_turn_labor_submission(turn_id, current_epoch_millis())
    {
        Ok(Some(submission)) if !had_submission => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "labor submission assembled: submission_id={} verifier={} settlement_ready={}",
                submission.submission.submission_id,
                submission.verifier_id,
                submission.settlement_ready
            ));
        }
        Ok(Some(_)) | Ok(None) => {}
        Err(error) => {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("labor submission failed: {error}"));
            return;
        }
    }

    let had_verdict = state
        .autopilot_chat
        .turn_labor_verdict_for(turn_id)
        .is_some();
    match state
        .autopilot_chat
        .finalize_turn_labor_verdict(turn_id, current_epoch_millis())
    {
        Ok(Some(verdict)) if !had_verdict => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "labor verdict finalized: verdict_id={} outcome={} settlement_ready={} verifier={}",
                verdict.verdict.verdict_id,
                verdict.outcome_label(),
                verdict.settlement_ready,
                verdict.verifier_id
            ));
            if !verdict.settlement_ready {
                match state.autopilot_chat.open_turn_labor_claim(
                    turn_id,
                    current_epoch_millis(),
                    verdict.verdict.reason_code.as_deref(),
                    verdict.settlement_withheld_reason.as_deref(),
                ) {
                    Ok(Some(claim)) => {
                        state.autopilot_chat.record_turn_timeline_event(format!(
                            "labor claim opened: claim_id={} state={}",
                            claim.claim.claim_id,
                            claim.status_label()
                        ));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        state.autopilot_chat.record_turn_timeline_event(format!(
                            "labor claim open failed: {error}"
                        ));
                    }
                }
            }
        }
        Ok(Some(_)) | Ok(None) => {}
        Err(error) => {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("labor verifier blocked settlement: {error}"));
            match state.autopilot_chat.open_turn_labor_claim(
                turn_id,
                current_epoch_millis(),
                None,
                Some(error.as_str()),
            ) {
                Ok(Some(claim)) => {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "labor claim opened: claim_id={} state={}",
                        claim.claim.claim_id,
                        claim.status_label()
                    ));
                }
                Ok(None) => {}
                Err(claim_error) => {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "labor claim open failed: {claim_error}"
                    ));
                }
            }
        }
    }
}

fn sync_goal_attempt_labor_linkage(state: &mut RenderState, turn_id: &str) {
    let Some(labor) = state.autopilot_chat.turn_labor_linkage_for(turn_id) else {
        return;
    };
    state
        .goal_loop_executor
        .merge_attempt_labor_linkage(Some(turn_id), labor);
}

fn cad_failure_class_from_tool_response(
    envelope: &super::super::tool_bridge::ToolBridgeResultEnvelope,
) -> CadBuildFailureClass {
    let class_label = envelope
        .details
        .get("failure_class")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase());
    match class_label.as_deref() {
        Some("tool_transport") => CadBuildFailureClass::ToolTransport,
        Some("intent_parse_validation") => CadBuildFailureClass::IntentParseValidation,
        Some("dispatch_rebuild") => CadBuildFailureClass::DispatchRebuild,
        _ => {
            if envelope.code.contains("PARSE")
                || envelope.code.contains("MISSING-PAYLOAD")
                || envelope.code.contains("NO-CHANGE")
            {
                CadBuildFailureClass::IntentParseValidation
            } else {
                CadBuildFailureClass::DispatchRebuild
            }
        }
    }
}

fn cad_failure_hint_from_tool_response(
    envelope: &super::super::tool_bridge::ToolBridgeResultEnvelope,
) -> String {
    envelope
        .details
        .pointer("/fallback/remediation_hint")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "inspect tool response details and retry CAD turn".to_string())
}

fn queue_cad_tool_response_with_retry(
    state: &mut RenderState,
    pending: &crate::app_state::AutopilotToolCallRequest,
    envelope: &super::super::tool_bridge::ToolBridgeResultEnvelope,
    retry_limit: u8,
) -> Result<u8, String> {
    let mut retries_used = 0u8;
    loop {
        let command = crate::codex_lane::CodexLaneCommand::ServerRequestToolCallRespond {
            request_id: pending.request_id.clone(),
            response: envelope.to_response(),
        };
        match state.queue_codex_command(command) {
            Ok(_) => return Ok(retries_used),
            Err(error) => {
                if retries_used >= retry_limit {
                    return Err(error);
                }
                retries_used = retries_used.saturating_add(1);
                state
                    .cad_demo
                    .record_agent_build_retry_metric(CadBuildFailureClass::ToolTransport);
                state.cad_demo.last_action = Some(format!(
                    "Retrying CAD tool response submit {}/{} for call {}",
                    retries_used, retry_limit, pending.call_id
                ));
            }
        }
    }
}

fn fail_active_cad_build_session(
    state: &mut RenderState,
    class: CadBuildFailureClass,
    event_code: &str,
    reason: String,
    remediation_hint: String,
    retry_attempts: u8,
    retry_limit: u8,
) {
    tracing::error!(
        "cad build/fail class={} event_code={} retries={}/{} reason={} remediation_hint={}",
        class.label(),
        event_code,
        retry_attempts,
        retry_limit,
        reason,
        remediation_hint
    );
    if state.cad_demo.build_session.phase == CadBuildSessionPhase::Idle {
        state.cad_demo.last_error = Some(reason);
        super::sync_cad_build_progress_to_chat(state);
        return;
    }
    state
        .cad_demo
        .set_agent_build_failure_context(class, retry_attempts, retry_limit);
    let _ = state
        .cad_demo
        .fail_agent_build_session(event_code, reason, Some(remediation_hint));
    super::sync_cad_build_progress_to_chat(state);
}

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: CodexLaneSnapshot) {
    let previous_lifecycle = state.codex_lane.lifecycle;
    state
        .autopilot_chat
        .set_connection_status(snapshot.lifecycle.label().to_string());
    if snapshot.lifecycle != CodexLaneLifecycle::Ready
        && state.autopilot_chat.startup_new_thread_bootstrap_pending
    {
        state.autopilot_chat.startup_new_thread_bootstrap_sent = false;
    }
    if let Some(error) = snapshot.last_error.as_ref() {
        tracing::info!("codex lane snapshot error: {}", error);
        state.autopilot_chat.last_error = Some(error.clone());
        state.codex_diagnostics.last_snapshot_error = Some(error.clone());
        state.codex_diagnostics.last_error = Some(error.clone());
        push_diagnostics_event(
            state,
            format!("snapshot {} error={error}", snapshot.lifecycle.label()),
        );
    }
    state.codex_diagnostics.load_state = PaneLoadState::Ready;
    state.codex_diagnostics.last_action = Some(format!(
        "lane snapshot lifecycle={}",
        snapshot.lifecycle.label()
    ));
    state.codex_account.install_available = snapshot.install_probe.available;
    state.codex_account.install_command = snapshot.install_probe.invocation.clone();
    state.codex_account.install_version = snapshot.install_probe.version.clone();
    state.codex_lane = snapshot;
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    refresh_codex_readiness_summary(state);
    if previous_lifecycle != CodexLaneLifecycle::Ready
        && state.codex_lane.lifecycle == CodexLaneLifecycle::Ready
    {
        queue_codex_readiness_refresh(state, true, "lane ready");
    }
}

pub(super) fn apply_command_response(state: &mut RenderState, response: CodexLaneCommandResponse) {
    if !super::apply_stream_event_seq(state, "codex.command", response.command_seq) {
        return;
    }

    let provider_owned = super::active_job_owns_codex_command_response(state, response.command_seq);
    let response_error = response.error.clone();
    increment_diagnostics_count(
        &mut state.codex_diagnostics.notification_counts,
        response.command.label(),
    );
    push_diagnostics_event(
        state,
        format!(
            "command {} {}",
            response.command.label(),
            response.status.label()
        ),
    );
    if response.status != CodexLaneCommandStatus::Accepted {
        tracing::info!(
            "codex command rejected command={} status={} seq={} error={}",
            response.command.label(),
            response.status.label(),
            response.command_seq,
            response
                .error
                .clone()
                .unwrap_or_else(|| "unknown error".to_string())
        );
        state.codex_diagnostics.last_command_failure = Some(format!(
            "{} {} {}",
            response.command.label(),
            response.status.label(),
            response_error
                .clone()
                .unwrap_or_else(|| "unknown error".to_string())
        ));
        state.codex_diagnostics.last_error = response_error.clone();
        state.sync_health.last_error = response
            .error
            .as_ref()
            .map(|error| format!("codex {}: {error}", response.status.label()));
        if response.command == CodexLaneCommandKind::TurnStart && !provider_owned {
            tracing::info!(
                "codex turn/start rejected seq={} active_thread={:?} error={}",
                response.command_seq,
                state.autopilot_chat.active_thread_id,
                response
                    .error
                    .clone()
                    .unwrap_or_else(|| "turn/start rejected".to_string())
            );
            state.autopilot_chat.mark_pending_turn_dispatch_failed(
                response
                    .error
                    .clone()
                    .unwrap_or_else(|| "turn/start rejected".to_string()),
            );
            if let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() {
                super::super::actions::restore_last_submission_draft(state, &thread_id);
            }
        } else if response.command == CodexLaneCommandKind::TurnSteer {
            let message = response
                .error
                .clone()
                .unwrap_or_else(|| "turn/steer rejected".to_string());
            if let Some((thread_id, prompt)) = state
                .autopilot_chat
                .take_pending_steer_submission(response.command_seq)
            {
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.chat_inputs.composer.set_value(prompt.clone());
                    state.autopilot_chat.record_composer_draft(prompt);
                }
            }
            state.autopilot_chat.last_error = Some(message);
        } else if response.command == CodexLaneCommandKind::ThreadResume {
            let message = response
                .error
                .clone()
                .unwrap_or_else(|| "thread/resume rejected".to_string());
            tracing::info!(
                "codex thread/resume rejected seq={} active_thread={:?} error={}",
                response.command_seq,
                state.autopilot_chat.active_thread_id,
                message
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
                queue_new_thread(
                    state,
                    "Failed to start replacement thread after stale resume",
                );
            } else {
                state.autopilot_chat.last_error = Some(message);
            }
        } else if response.command == CodexLaneCommandKind::SkillsList {
            state.skill_registry.load_state = PaneLoadState::Error;
            state.skill_registry.last_error = response
                .error
                .clone()
                .or_else(|| Some("codex skills/list failed".to_string()));
        } else if matches!(
            response.command,
            CodexLaneCommandKind::SkillsRemoteList | CodexLaneCommandKind::SkillsRemoteExport
        ) {
            state.skill_registry.load_state = PaneLoadState::Error;
            state.skill_registry.last_error = response
                .error
                .clone()
                .or_else(|| Some(format!("{} failed", response.command.label())));
        }
    } else if response.command == CodexLaneCommandKind::TurnStart {
        tracing::info!(
            "codex turn/start accepted seq={} active_thread={:?}",
            response.command_seq,
            state.autopilot_chat.active_thread_id
        );
        state.autopilot_chat.last_error = None;
        state.codex_diagnostics.last_error = None;
    } else if response.command == CodexLaneCommandKind::TurnSteer {
        tracing::info!(
            "codex turn/steer accepted seq={} active_thread={:?}",
            response.command_seq,
            state.autopilot_chat.active_thread_id
        );
        if let Some((thread_id, prompt)) = state
            .autopilot_chat
            .take_pending_steer_submission(response.command_seq)
            && state.autopilot_chat.is_active_thread(&thread_id)
        {
            state.autopilot_chat.submit_steer_prompt(prompt.clone());
            state
                .autopilot_chat
                .record_turn_timeline_event("turn steer accepted");
        }
        state.autopilot_chat.last_error = None;
        state.codex_diagnostics.last_error = None;
    } else if response.command == CodexLaneCommandKind::SkillsList {
        state.skill_registry.last_error = None;
        state.codex_diagnostics.last_error = None;
    } else if matches!(
        response.command,
        CodexLaneCommandKind::SkillsRemoteList | CodexLaneCommandKind::SkillsRemoteExport
    ) {
        state.skill_registry.last_error = None;
        state.codex_diagnostics.last_error = None;
    } else if response.command == CodexLaneCommandKind::SkillsConfigWrite {
        state.skill_registry.last_error = None;
        state.skill_registry.last_action =
            Some("skills/config/write applied; refreshing list".to_string());
        queue_skills_list_refresh(state);
        state.codex_diagnostics.last_error = None;
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
        CodexLaneCommandKind::McpServerStatusList
        | CodexLaneCommandKind::McpServerOauthLogin
        | CodexLaneCommandKind::McpServerReload => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_mcp.load_state = PaneLoadState::Ready;
                state.codex_mcp.last_error = None;
                state.codex_mcp.last_action =
                    Some(format!("{} accepted", response.command.label()));
            } else {
                state.codex_mcp.load_state = PaneLoadState::Error;
                state.codex_mcp.last_error = response_error
                    .clone()
                    .or_else(|| Some(format!("{} failed", response.command.label())));
            }
        }
        CodexLaneCommandKind::AppsList => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_apps.load_state = PaneLoadState::Ready;
                state.codex_apps.last_error = None;
                state.codex_apps.last_action = Some("app/list accepted".to_string());
            } else {
                state.codex_apps.load_state = PaneLoadState::Error;
                state.codex_apps.last_error = response_error
                    .clone()
                    .or_else(|| Some("app/list failed".to_string()));
            }
        }
        CodexLaneCommandKind::SkillsRemoteList | CodexLaneCommandKind::SkillsRemoteExport => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.skill_registry.load_state = PaneLoadState::Ready;
                state.skill_registry.last_error = None;
                state.skill_registry.last_action =
                    Some(format!("{} accepted", response.command.label()));
            } else {
                state.skill_registry.load_state = PaneLoadState::Error;
                state.skill_registry.last_error = response_error
                    .clone()
                    .or_else(|| Some(format!("{} failed", response.command.label())));
            }
        }
        CodexLaneCommandKind::ReviewStart
        | CodexLaneCommandKind::CommandExec
        | CodexLaneCommandKind::CollaborationModeList
        | CodexLaneCommandKind::ExperimentalFeatureList
        | CodexLaneCommandKind::ThreadRealtimeStart
        | CodexLaneCommandKind::ThreadRealtimeAppendText
        | CodexLaneCommandKind::ThreadRealtimeStop
        | CodexLaneCommandKind::WindowsSandboxSetupStart
        | CodexLaneCommandKind::FuzzyFileSearchSessionStart
        | CodexLaneCommandKind::FuzzyFileSearchSessionUpdate
        | CodexLaneCommandKind::FuzzyFileSearchSessionStop => {
            if response.status == CodexLaneCommandStatus::Accepted {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.last_error = None;
                state.codex_labs.last_action =
                    Some(format!("{} accepted", response.command.label()));
            } else {
                state.codex_labs.load_state = PaneLoadState::Error;
                state.codex_labs.last_error = response_error
                    .clone()
                    .or_else(|| Some(format!("{} failed", response.command.label())));
            }
        }
        _ => {}
    }

    super::apply_active_job_codex_command_response(state, &response);

    state.sync_health.last_action = Some(format!(
        "codex {} {}",
        response.command.label(),
        response.status.label()
    ));
    let refresh_readiness = matches!(
        response.command,
        CodexLaneCommandKind::AccountRead
            | CodexLaneCommandKind::AccountLoginStart
            | CodexLaneCommandKind::AccountLoginCancel
            | CodexLaneCommandKind::AccountLogout
            | CodexLaneCommandKind::AccountRateLimitsRead
            | CodexLaneCommandKind::ConfigRead
            | CodexLaneCommandKind::ConfigRequirementsRead
            | CodexLaneCommandKind::ConfigValueWrite
            | CodexLaneCommandKind::ConfigBatchWrite
    );
    state.record_codex_command_response(response);
    if refresh_readiness {
        refresh_codex_readiness_summary(state);
    }
}

pub(super) fn queue_codex_readiness_refresh(
    state: &mut RenderState,
    refresh_token: bool,
    reason: &str,
) {
    state.codex_account.load_state = PaneLoadState::Loading;
    state.codex_account.last_error = None;
    state.codex_account.last_action = Some(format!("Queued Codex readiness refresh ({reason})"));
    state.codex_config.load_state = PaneLoadState::Loading;
    state.codex_config.last_error = None;
    state.codex_config.last_action = Some(format!("Queued config truth refresh ({reason})"));

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    let commands = [
        CodexLaneCommand::AccountRead(codex_client::GetAccountParams { refresh_token }),
        CodexLaneCommand::AccountRateLimitsRead,
        CodexLaneCommand::ConfigRead(codex_client::ConfigReadParams {
            include_layers: true,
            cwd,
        }),
        CodexLaneCommand::ConfigRequirementsRead,
        CodexLaneCommand::CollaborationModeList(
            codex_client::CollaborationModeListParams::default(),
        ),
    ];
    let mut errors = Vec::new();
    for command in commands {
        if let Err(error) = state.queue_codex_command(command) {
            errors.push(error);
        }
    }
    if !errors.is_empty() {
        let message = errors.join(" | ");
        state.codex_account.load_state = PaneLoadState::Error;
        state.codex_account.last_error = Some(message.clone());
        state.codex_config.load_state = PaneLoadState::Error;
        state.codex_config.last_error = Some(message);
    }
    refresh_codex_readiness_summary(state);
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

    if let Err(error) = crate::skill_autoload::ensure_required_cad_skills() {
        tracing::warn!(
            "failed to auto-provision managed CAD skills before skills/list refresh: {}",
            error
        );
    }
    let extra_user_roots = crate::skill_autoload::codex_extra_skill_roots(&cwd);

    let params = SkillsListParams {
        cwds: vec![cwd.clone()],
        force_reload: true,
        per_cwd_extra_user_roots: (!extra_user_roots.is_empty()).then_some(vec![
            SkillsListExtraRootsForCwd {
                cwd,
                extra_user_roots,
            },
        ]),
    };
    if let Err(error) = state.queue_codex_command(CodexLaneCommand::SkillsList(params)) {
        state.skill_registry.last_error = Some(error);
        state.skill_registry.load_state = PaneLoadState::Error;
    }
}

fn queue_mcp_status_refresh(state: &mut RenderState) {
    if let Err(error) = state.queue_codex_command(CodexLaneCommand::McpServerStatusList(
        codex_client::ListMcpServerStatusParams {
            cursor: None,
            limit: Some(100),
        },
    )) {
        state.codex_mcp.last_error = Some(error);
        state.codex_mcp.load_state = PaneLoadState::Error;
    }
}

fn queue_apps_list_refresh(state: &mut RenderState, force_refetch: bool) {
    if let Err(error) =
        state.queue_codex_command(CodexLaneCommand::AppsList(codex_client::AppsListParams {
            cursor: None,
            limit: Some(100),
            thread_id: state.autopilot_chat.active_thread_id.clone(),
            force_refetch,
        }))
    {
        state.codex_apps.last_error = Some(error);
        state.codex_apps.load_state = PaneLoadState::Error;
    }
}

fn queue_new_thread(state: &mut RenderState, error_prefix: &str) -> bool {
    let cwd = super::super::actions::current_chat_session_cwd(state);
    let command = CodexLaneCommand::ThreadStart(ThreadStartParams {
        model: state.autopilot_chat.selected_model_override(),
        model_provider: None,
        service_tier: super::super::actions::chat_session_service_tier(state),
        cwd,
        approval_policy: super::super::actions::chat_session_approval_policy(state),
        sandbox: super::super::actions::chat_session_thread_sandbox_mode(state),
        personality: super::super::actions::chat_session_personality(state),
        ephemeral: None,
        dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(format!("{error_prefix}: {error}"));
        return false;
    }
    true
}

fn queue_thread_history_refresh(state: &mut RenderState) {
    let cwd = super::super::actions::current_chat_session_cwd(state).or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|value| value.into_os_string().into_string().ok())
    });
    let params = state.autopilot_chat.build_thread_list_params(cwd);
    if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadList(params)) {
        state.autopilot_chat.last_error = Some(error);
        return;
    }
    if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadLoadedList(
        codex_client::ThreadLoadedListParams {
            cursor: None,
            limit: Some(200),
        },
    )) {
        state.autopilot_chat.last_error = Some(error);
    }
}

pub(super) fn apply_notification(state: &mut RenderState, notification: CodexLaneNotification) {
    let stored = notification.clone();
    let provider_owned = super::apply_active_job_codex_notification(state, &notification);
    if !provider_owned {
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
                if state.codex_account.account_summary == "logged out" {
                    state.codex_account.auth_mode = None;
                    state.codex_account.rate_limits_summary = None;
                }
                state.codex_account.last_action = Some("Loaded account state".to_string());
                state.codex_account.last_error = None;
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::AccountRateLimitsLoaded { summary } => {
                state.codex_account.load_state = PaneLoadState::Ready;
                state.codex_account.rate_limits_summary = Some(summary);
                state.codex_account.last_action = Some("Loaded account rate limits".to_string());
                state.codex_account.last_error = None;
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::AccountUpdated { auth_mode } => {
                state.codex_account.load_state = PaneLoadState::Ready;
                state.codex_account.auth_mode = auth_mode;
                state.codex_account.last_action = Some("Received account/updated".to_string());
                state.codex_account.last_error = None;
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::AccountLoginStarted { login_id, auth_url } => {
                state.codex_account.load_state = PaneLoadState::Ready;
                state.codex_account.pending_login_id = login_id;
                state.codex_account.pending_login_url = auth_url;
                state.codex_account.last_action =
                    Some("Login started; complete auth in browser".to_string());
                state.codex_account.last_error = None;
                refresh_codex_readiness_summary(state);
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
                refresh_codex_readiness_summary(state);
                if success {
                    queue_codex_readiness_refresh(state, true, "login completed");
                }
            }
            CodexLaneNotification::ConfigLoaded {
                config,
                origins,
                layers,
            } => {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.config_json = config;
                state.codex_config.origins_json = origins;
                state.codex_config.layers_json = layers;
                state.codex_config.last_action = Some("Loaded config/read".to_string());
                state.codex_config.last_error = None;
                refresh_codex_config_truth(state);
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::ConfigRequirementsLoaded { requirements } => {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.requirements_json = requirements;
                state.codex_config.last_action = Some("Loaded config requirements".to_string());
                state.codex_config.last_error = None;
                refresh_codex_config_truth(state);
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::ConfigWriteApplied { status, version } => {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.last_action = Some(format!(
                    "Config write applied status={} version={}",
                    status, version
                ));
                state.codex_config.last_error = None;
                queue_codex_readiness_refresh(state, false, "config write applied");
            }
            CodexLaneNotification::ExternalAgentConfigDetected { count } => {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.detected_external_configs = count;
                state.codex_config.last_action =
                    Some(format!("Detected {} external agent configs", count));
                state.codex_config.last_error = None;
                refresh_codex_readiness_summary(state);
            }
            CodexLaneNotification::ExternalAgentConfigImported => {
                state.codex_config.load_state = PaneLoadState::Ready;
                state.codex_config.last_action =
                    Some("External agent config import completed".to_string());
                state.codex_config.last_error = None;
                queue_codex_readiness_refresh(state, false, "external config import");
            }
            CodexLaneNotification::McpServerStatusListLoaded {
                entries,
                next_cursor,
            } => {
                state.codex_mcp.load_state = PaneLoadState::Ready;
                state.codex_mcp.servers = entries
                    .into_iter()
                    .map(|entry| crate::app_state::CodexMcpServerEntryState {
                        name: entry.name,
                        auth_status: entry.auth_status,
                        tool_count: entry.tool_count,
                        resource_count: entry.resource_count,
                        template_count: entry.template_count,
                    })
                    .collect();
                state.codex_mcp.next_cursor = next_cursor;
                state.codex_mcp.selected_server_index = if state.codex_mcp.servers.is_empty() {
                    None
                } else {
                    Some(
                        state
                            .codex_mcp
                            .selected_server_index
                            .unwrap_or(0)
                            .min(state.codex_mcp.servers.len().saturating_sub(1)),
                    )
                };
                state.codex_mcp.last_action = Some(format!(
                    "Loaded MCP status for {} servers",
                    state.codex_mcp.servers.len()
                ));
                state.codex_mcp.last_error = None;
            }
            CodexLaneNotification::McpServerOauthLoginStarted {
                server_name,
                authorization_url,
            } => {
                state.codex_mcp.load_state = PaneLoadState::Ready;
                state.codex_mcp.last_oauth_url = Some(authorization_url);
                state.codex_mcp.last_oauth_result =
                    Some(format!("OAuth started for server {}", server_name));
                state.codex_mcp.last_action = Some("MCP OAuth login started".to_string());
                state.codex_mcp.last_error = None;
            }
            CodexLaneNotification::McpServerOauthLoginCompleted {
                server_name,
                success,
                error,
            } => {
                state.codex_mcp.load_state = if success {
                    PaneLoadState::Ready
                } else {
                    PaneLoadState::Error
                };
                state.codex_mcp.last_oauth_result = Some(if success {
                    format!("OAuth completed successfully for {}", server_name)
                } else {
                    format!("OAuth failed for {}", server_name)
                });
                state.codex_mcp.last_action = Some("Received MCP OAuth completion".to_string());
                state.codex_mcp.last_error = if success { None } else { error };
                queue_mcp_status_refresh(state);
            }
            CodexLaneNotification::McpServerReloaded => {
                state.codex_mcp.load_state = PaneLoadState::Ready;
                state.codex_mcp.last_action = Some("Reloaded MCP config".to_string());
                state.codex_mcp.last_error = None;
                queue_mcp_status_refresh(state);
            }
            CodexLaneNotification::AppsListLoaded {
                entries,
                next_cursor,
            } => {
                state.codex_apps.load_state = PaneLoadState::Ready;
                state.codex_apps.apps = entries
                    .into_iter()
                    .map(|entry| crate::app_state::CodexAppEntryState {
                        id: entry.id,
                        name: entry.name,
                        description: entry.description,
                        is_accessible: entry.is_accessible,
                        is_enabled: entry.is_enabled,
                    })
                    .collect();
                state.codex_apps.next_cursor = next_cursor;
                state.codex_apps.selected_app_index = if state.codex_apps.apps.is_empty() {
                    None
                } else {
                    Some(
                        state
                            .codex_apps
                            .selected_app_index
                            .unwrap_or(0)
                            .min(state.codex_apps.apps.len().saturating_sub(1)),
                    )
                };
                state.codex_apps.last_action =
                    Some(format!("Loaded {} apps", state.codex_apps.apps.len()));
                state.codex_apps.last_error = None;
            }
            CodexLaneNotification::AppsListUpdated => {
                state.codex_apps.update_count = state.codex_apps.update_count.saturating_add(1);
                state.codex_apps.last_action = Some("Received app/list/updated".to_string());
                state.codex_apps.last_error = None;
                queue_apps_list_refresh(state, true);
            }
            CodexLaneNotification::ReviewStarted {
                thread_id,
                turn_id,
                review_thread_id,
                delivery,
                target,
            } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.review_last_thread_id = Some(review_thread_id.clone());
                state.codex_labs.review_last_turn_id = Some(turn_id.clone());
                state.codex_labs.last_action = Some(format!(
                    "Review started for thread={} turn={} reviewThread={} delivery={} target={}",
                    thread_id, turn_id, review_thread_id, delivery, target
                ));
                state.codex_labs.last_error = None;
                state.autopilot_chat.begin_review_artifact(
                    &thread_id,
                    turn_id,
                    review_thread_id,
                    delivery,
                    target,
                    super::super::actions::current_epoch_millis(),
                );
            }
            CodexLaneNotification::CommandExecCompleted {
                exit_code,
                stdout,
                stderr,
            } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.command_last_exit_code = Some(exit_code);
                state.codex_labs.command_last_stdout = stdout;
                state.codex_labs.command_last_stderr = stderr;
                state.codex_labs.last_action =
                    Some(format!("command/exec completed (exit={exit_code})"));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::CollaborationModesLoaded { modes_json, count } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.collaboration_modes_json = modes_json;
                state.codex_labs.last_action =
                    Some(format!("Loaded collaborationMode/list entries={count}"));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::ExperimentalFeaturesLoaded {
                features_json,
                count,
                next_cursor,
            } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.experimental_features_json = features_json;
                state.codex_labs.last_action = Some(format!(
                    "Loaded experimentalFeature/list entries={} nextCursor={}",
                    count,
                    next_cursor.unwrap_or_else(|| "none".to_string())
                ));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::RealtimeStarted {
                thread_id,
                session_id,
            } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.realtime_started = true;
                state.codex_labs.last_action = Some(format!(
                    "thread/realtime started thread={} session={}",
                    thread_id,
                    session_id.unwrap_or_else(|| "none".to_string())
                ));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::RealtimeTextAppended {
                thread_id,
                text_len,
            } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.last_action = Some(format!(
                    "thread/realtime appendText thread={} chars={}",
                    thread_id, text_len
                ));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::RealtimeStopped { thread_id } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.realtime_started = false;
                state.codex_labs.last_action =
                    Some(format!("thread/realtime stopped thread={thread_id}"));
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::RealtimeError { thread_id, message } => {
                state.codex_labs.load_state = PaneLoadState::Error;
                state.codex_labs.realtime_started = false;
                state.codex_labs.last_action =
                    Some(format!("thread/realtime error thread={thread_id}"));
                state.codex_labs.last_error = Some(message);
            }
            CodexLaneNotification::WindowsSandboxSetupStarted { mode, started } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.windows_last_status = Some(format!(
                    "windowsSandbox/setupStart mode={} started={}",
                    mode, started
                ));
                state.codex_labs.last_action =
                    Some("windowsSandbox/setupStart completed".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::WindowsSandboxSetupCompleted { mode, success } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.windows_last_status = Some(format!(
                    "windowsSandbox/setupCompleted mode={} success={}",
                    mode.unwrap_or_else(|| "unknown".to_string()),
                    success
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ));
                state.codex_labs.last_action =
                    Some("Received windowsSandbox/setupCompleted".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::FuzzySessionStarted { session_id } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.fuzzy_session_id = session_id;
                state.codex_labs.fuzzy_last_status = "started".to_string();
                state.codex_labs.last_action =
                    Some("fuzzyFileSearch/sessionStart completed".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::FuzzySessionUpdated { session_id, status } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.fuzzy_session_id = session_id;
                state.codex_labs.fuzzy_last_status = status;
                state.codex_labs.last_action =
                    Some("fuzzyFileSearch/sessionUpdate received".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::FuzzySessionCompleted { session_id } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.fuzzy_session_id = session_id;
                state.codex_labs.fuzzy_last_status = "completed".to_string();
                state.codex_labs.last_action =
                    Some("fuzzyFileSearch/sessionCompleted received".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::FuzzySessionStopped { session_id } => {
                state.codex_labs.load_state = PaneLoadState::Ready;
                state.codex_labs.fuzzy_session_id = session_id;
                state.codex_labs.fuzzy_last_status = "stopped".to_string();
                state.codex_labs.last_action =
                    Some("fuzzyFileSearch/sessionStop completed".to_string());
                state.codex_labs.last_error = None;
            }
            CodexLaneNotification::SkillsRemoteListLoaded { entries } => {
                state.skill_registry.remote_skills = entries
                    .into_iter()
                    .map(|entry| crate::app_state::SkillRegistryRemoteSkill {
                        id: entry.id,
                        name: entry.name,
                        description: entry.description,
                    })
                    .collect();
                state.skill_registry.load_state = PaneLoadState::Ready;
                state.skill_registry.last_action = Some(format!(
                    "Loaded {} remote skills",
                    state.skill_registry.remote_skills.len()
                ));
                state.skill_registry.last_error = None;
            }
            CodexLaneNotification::SkillsRemoteExported { id, path } => {
                state.skill_registry.last_remote_export_id = Some(id.clone());
                state.skill_registry.last_remote_export_path = Some(path.clone());
                state.skill_registry.load_state = PaneLoadState::Ready;
                state.skill_registry.last_action =
                    Some(format!("Exported remote skill {} to {}", id, path));
                state.skill_registry.last_error = None;
                queue_skills_list_refresh(state);
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
                tracing::info!("codex thread/list loaded {} entries", entries.len());
                state.autopilot_chat.set_thread_entries(
                    entries
                        .into_iter()
                        .map(|entry| crate::app_state::AutopilotThreadListEntry {
                            thread_id: entry.thread_id,
                            thread_name: entry.thread_name,
                            preview: entry.preview,
                            status: entry.status,
                            loaded: entry.loaded,
                            cwd: entry.cwd,
                            path: entry.path,
                            created_at: entry.created_at,
                            updated_at: entry.updated_at,
                        })
                        .collect(),
                );
            }
            CodexLaneNotification::ThreadLoadedListLoaded { thread_ids } => {
                state.autopilot_chat.set_thread_loaded_ids(&thread_ids);
            }
            CodexLaneNotification::ThreadReadLoaded {
                thread_id,
                messages,
                latest_plan,
                latest_review,
                latest_compaction,
            } => {
                state.autopilot_chat.remember_thread(thread_id.clone());
                if let Some(latest_plan) = latest_plan {
                    let updated_at_epoch_ms = state
                        .autopilot_chat
                        .thread_metadata
                        .get(&thread_id)
                        .and_then(|metadata| metadata.updated_at)
                        .filter(|value| *value > 0)
                        .map(|value| value as u64)
                        .unwrap_or_else(super::super::actions::current_epoch_millis);
                    state.autopilot_chat.restore_plan_artifact_from_text(
                        &thread_id,
                        latest_plan.turn_id,
                        &latest_plan.text,
                        updated_at_epoch_ms,
                    );
                } else {
                    state.autopilot_chat.clear_plan_artifact(&thread_id);
                }
                if let Some(latest_review) = latest_review {
                    let updated_at_epoch_ms = state
                        .autopilot_chat
                        .thread_metadata
                        .get(&thread_id)
                        .and_then(|metadata| metadata.updated_at)
                        .filter(|value| *value > 0)
                        .map(|value| value as u64)
                        .unwrap_or_else(super::super::actions::current_epoch_millis);
                    if latest_review.completed {
                        state.autopilot_chat.restore_review_artifact_from_text(
                            &thread_id,
                            latest_review.turn_id,
                            &latest_review.review,
                            updated_at_epoch_ms,
                        );
                    } else {
                        state.autopilot_chat.begin_review_artifact(
                            &thread_id,
                            latest_review.turn_id,
                            thread_id.clone(),
                            "inline",
                            latest_review.review,
                            updated_at_epoch_ms,
                        );
                    }
                }
                if let Some(latest_compaction) = latest_compaction {
                    let updated_at_epoch_ms = state
                        .autopilot_chat
                        .thread_metadata
                        .get(&thread_id)
                        .and_then(|metadata| metadata.updated_at)
                        .filter(|value| *value > 0)
                        .map(|value| value as u64)
                        .unwrap_or_else(super::super::actions::current_epoch_millis);
                    state.autopilot_chat.restore_compaction_artifact(
                        &thread_id,
                        latest_compaction.turn_id,
                        updated_at_epoch_ms,
                    );
                }
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    let transcript = messages
                        .into_iter()
                        .map(|message| {
                            let role = match message.role {
                                CodexThreadTranscriptRole::User => {
                                    crate::app_state::AutopilotRole::User
                                }
                                CodexThreadTranscriptRole::Codex => {
                                    crate::app_state::AutopilotRole::Codex
                                }
                            };
                            (role, message.content)
                        })
                        .collect::<Vec<_>>();
                    state
                        .autopilot_chat
                        .cache_thread_transcript(&thread_id, transcript.clone());
                    if state.autopilot_chat.has_pending_messages() {
                        tracing::info!(
                            "codex thread/read skipped id={} messages={} reason=pending-turn",
                            thread_id,
                            transcript.len()
                        );
                        state.autopilot_chat.record_turn_timeline_event(
                            "thread/read skipped while turn is pending",
                        );
                        return;
                    }
                    tracing::info!(
                        "codex thread/read loaded id={} messages={}",
                        thread_id,
                        transcript.len()
                    );
                    state
                        .autopilot_chat
                        .set_active_thread_transcript(&thread_id, transcript);
                } else {
                    let transcript = messages
                        .into_iter()
                        .map(|message| {
                            let role = match message.role {
                                CodexThreadTranscriptRole::User => {
                                    crate::app_state::AutopilotRole::User
                                }
                                CodexThreadTranscriptRole::Codex => {
                                    crate::app_state::AutopilotRole::Codex
                                }
                            };
                            (role, message.content)
                        })
                        .collect::<Vec<_>>();
                    state
                        .autopilot_chat
                        .cache_thread_transcript(&thread_id, transcript);
                }
            }
            CodexLaneNotification::ThreadSelected { thread_id } => {
                if state.autopilot_chat.startup_new_thread_bootstrap_pending {
                    if !state.autopilot_chat.startup_new_thread_bootstrap_sent
                        && queue_new_thread(state, "Failed to start initial Autopilot Chat thread")
                    {
                        state.autopilot_chat.startup_new_thread_bootstrap_sent = true;
                    }
                    tracing::info!(
                        "codex thread/selected ignored during startup bootstrap thread_id={}",
                        thread_id
                    );
                } else {
                    state.autopilot_chat.ensure_thread(thread_id.clone());
                    state
                        .autopilot_chat
                        .restore_session_preferences_from_thread(&thread_id);
                    super::super::actions::restore_chat_composer_draft(state);
                    queue_thread_history_refresh(state);
                    let metadata = state
                        .autopilot_chat
                        .thread_metadata
                        .get(&thread_id)
                        .cloned();
                    let resume_path = if state.codex_lane_config.experimental_api {
                        metadata.as_ref().and_then(|value| value.path.clone())
                    } else {
                        None
                    };
                    if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadResume(
                        codex_client::ThreadResumeParams {
                            thread_id: thread_id.clone(),
                            model: state.autopilot_chat.selected_model_override(),
                            model_provider: None,
                            service_tier: super::super::actions::chat_session_service_tier(state),
                            cwd: metadata
                                .as_ref()
                                .and_then(|value| value.cwd.clone())
                                .or_else(|| super::super::actions::current_chat_session_cwd(state)),
                            approval_policy: super::super::actions::chat_session_approval_policy(
                                state,
                            ),
                            sandbox: super::super::actions::chat_session_thread_sandbox_mode(state),
                            personality: super::super::actions::chat_session_personality(state),
                            path: resume_path.map(std::path::PathBuf::from),
                        },
                    )) {
                        state.autopilot_chat.last_error = Some(error);
                    }
                    state
                        .autopilot_chat
                        .set_active_thread_transcript(&thread_id, Vec::new());
                    if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadRead(
                        codex_client::ThreadReadParams {
                            thread_id,
                            include_turns: true,
                        },
                    )) {
                        state.autopilot_chat.last_error = Some(error);
                    }
                }
            }
            CodexLaneNotification::ThreadStarted {
                thread_id,
                model,
                cwd,
                approval_policy,
                sandbox_mode,
                service_tier,
                reasoning_effort,
            } => {
                let adopt_detached_draft = state.autopilot_chat.active_thread_id.is_none();
                state.autopilot_chat.ensure_thread(thread_id.clone());
                if adopt_detached_draft {
                    state
                        .autopilot_chat
                        .adopt_detached_composer_draft(&thread_id);
                }
                state
                    .autopilot_chat
                    .set_active_thread_transcript(&thread_id, Vec::new());
                state.autopilot_chat.apply_thread_session_configuration(
                    &thread_id,
                    model,
                    cwd,
                    approval_policy,
                    sandbox_mode,
                    service_tier,
                    reasoning_effort,
                );
                state.autopilot_chat.startup_new_thread_bootstrap_pending = false;
                state.autopilot_chat.startup_new_thread_bootstrap_sent = false;
                state.autopilot_chat.last_error = None;
                super::super::actions::restore_chat_composer_draft(state);
                queue_thread_history_refresh(state);
            }
            CodexLaneNotification::ThreadSessionConfigured {
                thread_id,
                model,
                cwd,
                approval_policy,
                sandbox_mode,
                service_tier,
                reasoning_effort,
            } => {
                state.autopilot_chat.apply_thread_session_configuration(
                    &thread_id,
                    model,
                    cwd,
                    approval_policy,
                    sandbox_mode,
                    service_tier,
                    reasoning_effort,
                );
            }
            CodexLaneNotification::ThreadStatusChanged { thread_id, status } => {
                let thread_id = resolve_thread_id(state, thread_id);
                tracing::info!(
                    "codex thread/status changed thread_id={} status={} active_thread={:?}",
                    thread_id,
                    status,
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                state
                    .autopilot_chat
                    .set_thread_status(&thread_id, Some(status.clone()));
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "thread status: {thread_id} => {status}"
                    ));
                    // Recovery probe: if turn-completion notifications are dropped, an idle flip is the
                    // earliest signal that we should fetch the materialized transcript.
                    if status == "idle"
                        && state.autopilot_chat.active_turn_id.is_some()
                        && state.autopilot_chat.last_turn_status.as_deref() != Some("syncProbing")
                    {
                        let active_turn = state
                            .autopilot_chat
                            .active_turn_id
                            .clone()
                            .unwrap_or_default();
                        tracing::info!(
                            "codex thread/status idle while turn active; probing thread/read thread_id={} turn_id={}",
                            thread_id,
                            active_turn
                        );
                        state
                            .autopilot_chat
                            .set_turn_status(Some("syncProbing".to_string()));
                        state.autopilot_chat.record_turn_timeline_event(
                            "thread idle while turn active; probing transcript",
                        );
                        if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadRead(
                            codex_client::ThreadReadParams {
                                thread_id: thread_id.clone(),
                                include_turns: true,
                            },
                        )) {
                            state.autopilot_chat.last_error = Some(error);
                        }
                    }
                }
            }
            CodexLaneNotification::ThreadArchived { thread_id } => {
                state
                    .autopilot_chat
                    .set_thread_status(&thread_id, Some("archived".to_string()));
                if state.autopilot_chat.thread_filter_archived == Some(false) {
                    state.autopilot_chat.remove_thread(&thread_id);
                    super::super::actions::restore_chat_composer_draft(state);
                }
                queue_thread_history_refresh(state);
            }
            CodexLaneNotification::ThreadUnarchived { thread_id } => {
                state
                    .autopilot_chat
                    .set_thread_status(&thread_id, Some("idle".to_string()));
                state.autopilot_chat.remember_thread(thread_id);
                queue_thread_history_refresh(state);
            }
            CodexLaneNotification::ThreadClosed { thread_id } => {
                state.autopilot_chat.remove_thread(&thread_id);
                super::super::actions::restore_chat_composer_draft(state);
                queue_thread_history_refresh(state);
            }
            CodexLaneNotification::ThreadNameUpdated {
                thread_id,
                thread_name,
            } => {
                state
                    .autopilot_chat
                    .set_thread_name(&thread_id, thread_name);
                queue_thread_history_refresh(state);
            }
            CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex turn/started thread_id={} turn_id={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state
                        .autopilot_chat
                        .record_turn_timeline_event(format!("turn started: {turn_id}"));
                    state.autopilot_chat.mark_turn_started(turn_id);
                    if let Some(active_turn_id) = state.autopilot_chat.active_turn_id.clone() {
                        state
                            .goal_loop_executor
                            .bind_attempt_turn_id(active_turn_id.as_str());
                        sync_goal_attempt_labor_linkage(state, active_turn_id.as_str());
                    }
                }
            }
            CodexLaneNotification::ItemStarted {
                thread_id,
                turn_id,
                item_id,
                item_type,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                let item_type_label = item_type.as_deref().unwrap_or("n/a").to_string();
                let is_command_execution_item =
                    item_type_label.eq_ignore_ascii_case("commandExecution");
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "item started: turn={turn_id} id={} type={}",
                        item_id.as_deref().unwrap_or("n/a"),
                        item_type_label
                    ));
                    if is_command_execution_item {
                        tracing::info!(
                            "codex item/commandExecution started thread_id={} turn_id={} item_id={} active_thread={:?}",
                            thread_id,
                            turn_id,
                            item_id.as_deref().unwrap_or("n/a"),
                            state.autopilot_chat.active_thread_id
                        );
                    }
                }
            }
            CodexLaneNotification::ItemCompleted {
                thread_id,
                turn_id,
                item_id,
                item_type,
                message,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                let item_type_label = item_type.as_deref().unwrap_or("n/a");
                tracing::info!(
                    "codex item/completed thread_id={} turn_id={} item_id={} item_type={} message_chars={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    item_id.as_deref().unwrap_or("n/a"),
                    item_type_label,
                    message
                        .as_ref()
                        .map(|value| value.chars().count())
                        .unwrap_or(0),
                    state.autopilot_chat.active_thread_id
                );
                if let Some(message_text) = message.as_deref() {
                    if item_type_label.eq_ignore_ascii_case("userMessage") {
                        log_chat_message_to_console(
                            "user",
                            &thread_id,
                            &turn_id,
                            "item/completed:userMessage",
                            message_text,
                        );
                    } else if item_type_label.eq_ignore_ascii_case("agentMessage") {
                        log_chat_message_to_console(
                            "assistant",
                            &thread_id,
                            &turn_id,
                            "item/completed:agentMessage",
                            message_text,
                        );
                    }
                }
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "item completed: turn={turn_id} id={} type={}",
                        item_id.as_deref().unwrap_or("n/a"),
                        item_type_label
                    ));
                    if let Some(message) = message {
                        state
                            .autopilot_chat
                            .set_turn_message_for_turn(&turn_id, &message);
                        state.autopilot_chat.mark_turn_completed_for(&turn_id);
                    }
                }
            }
            CodexLaneNotification::AgentMessageDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex agent/delta thread_id={} turn_id={} item_id={} chars={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    item_id,
                    delta.chars().count(),
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    if state
                        .autopilot_chat
                        .is_duplicate_agent_delta(&turn_id, &item_id, &delta)
                    {
                        tracing::info!(
                            "codex agent/delta duplicate suppressed thread_id={} turn_id={} item_id={} chars={}",
                            thread_id,
                            turn_id,
                            item_id,
                            delta.chars().count()
                        );
                        return;
                    }
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "agent delta: turn={} item={} chars={}",
                        turn_id,
                        item_id,
                        delta.chars().count()
                    ));
                    state
                        .autopilot_chat
                        .append_turn_delta_for_turn(&turn_id, &delta);
                }
            }
            CodexLaneNotification::AgentMessageCompleted {
                thread_id,
                turn_id,
                item_id,
                message,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex agent/completed thread_id={} turn_id={} item_id={} chars={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    item_id.as_deref().unwrap_or("n/a"),
                    message.chars().count(),
                    state.autopilot_chat.active_thread_id
                );
                log_chat_message_to_console(
                    "assistant",
                    &thread_id,
                    &turn_id,
                    "item/agentMessage/completed",
                    &message,
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "agent completed: turn={} item={} chars={}",
                        turn_id,
                        item_id.as_deref().unwrap_or("n/a"),
                        message.chars().count()
                    ));
                    state
                        .autopilot_chat
                        .set_turn_message_for_turn(&turn_id, &message);
                    state.autopilot_chat.mark_turn_completed_for(&turn_id);
                }
            }
            CodexLaneNotification::ReasoningDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex reasoning/delta thread_id={} turn_id={} item_id={} chars={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    item_id.as_deref().unwrap_or("n/a"),
                    delta.chars().count(),
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    let item_id_for_dedupe = item_id.as_deref().unwrap_or("n/a");
                    if state.autopilot_chat.is_duplicate_reasoning_delta(
                        &turn_id,
                        item_id_for_dedupe,
                        &delta,
                    ) {
                        tracing::info!(
                            "codex reasoning/delta duplicate suppressed thread_id={} turn_id={} item_id={} chars={}",
                            thread_id,
                            turn_id,
                            item_id_for_dedupe,
                            delta.chars().count()
                        );
                        return;
                    }
                    state.autopilot_chat.record_turn_timeline_event(format!(
                        "reasoning delta: turn={} item={} chars={}",
                        turn_id,
                        item_id.as_deref().unwrap_or("n/a"),
                        delta.chars().count()
                    ));
                    state
                        .autopilot_chat
                        .append_turn_reasoning_delta_for_turn(&turn_id, &delta);
                }
            }
            CodexLaneNotification::TurnCompleted {
                thread_id,
                turn_id,
                status,
                error_message,
                final_message,
                ..
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex turn/completed thread_id={} turn_id={} status={:?} final_message_chars={} error={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    status,
                    final_message
                        .as_ref()
                        .map(|value| value.chars().count())
                        .unwrap_or(0),
                    error_message.as_deref().unwrap_or("none"),
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    let had_visible_output = state.autopilot_chat.turn_has_visible_output(&turn_id);
                    state.autopilot_chat.set_turn_status(status.clone());
                    if let Some(message) = final_message
                        && !message.trim().is_empty()
                    {
                        log_chat_message_to_console(
                            "assistant",
                            &thread_id,
                            &turn_id,
                            "turn/completed:final_message",
                            &message,
                        );
                        state
                            .autopilot_chat
                            .set_turn_message_for_turn(&turn_id, &message);
                    }
                    match status.as_deref() {
                        Some("failed") => {
                            state.autopilot_chat.mark_turn_error_for(
                                &turn_id,
                                error_message.unwrap_or_else(|| "Turn failed".to_string()),
                            );
                        }
                        Some("interrupted") => {
                            state.autopilot_chat.mark_turn_completed_for(&turn_id);
                            state
                                .autopilot_chat
                                .set_turn_status(Some("interrupted".to_string()));
                            state
                                .autopilot_chat
                                .record_turn_timeline_event("turn interrupted");
                        }
                        _ => {
                            state.autopilot_chat.mark_turn_completed_for(&turn_id);
                            advance_completed_turn_labor_pipeline(state, &turn_id);
                            sync_goal_attempt_labor_linkage(state, &turn_id);
                        }
                    }
                    // Do not immediately clobber a valid streamed message with a stale/partial
                    // thread/read snapshot. Only re-read when no visible assistant output exists.
                    let needs_transcript_probe = !had_visible_output
                        && !state.autopilot_chat.turn_has_visible_output(&turn_id);
                    if needs_transcript_probe {
                        tracing::info!(
                            "codex turn/completed without visible output; probing thread/read thread_id={} turn_id={}",
                            thread_id,
                            turn_id
                        );
                        if let Err(error) = state.queue_codex_command(CodexLaneCommand::ThreadRead(
                            codex_client::ThreadReadParams {
                                thread_id,
                                include_turns: true,
                            },
                        )) {
                            state.autopilot_chat.last_error = Some(error);
                        }
                    }
                }
            }
            CodexLaneNotification::TurnDiffUpdated {
                thread_id,
                turn_id,
                diff,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                state.autopilot_chat.remember_thread(thread_id.clone());
                state.autopilot_chat.set_diff_artifact(
                    &thread_id,
                    turn_id,
                    diff,
                    super::super::actions::current_epoch_millis(),
                );
            }
            CodexLaneNotification::ReviewProgressUpdated {
                thread_id,
                turn_id,
                review,
                completed,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                state.autopilot_chat.remember_thread(thread_id.clone());
                if completed {
                    state.autopilot_chat.complete_review_artifact(
                        &thread_id,
                        turn_id,
                        &review,
                        super::super::actions::current_epoch_millis(),
                        false,
                    );
                }
            }
            CodexLaneNotification::ThreadCompacted { thread_id, turn_id } => {
                let thread_id = resolve_thread_id(state, thread_id);
                state.autopilot_chat.remember_thread(thread_id.clone());
                state.autopilot_chat.set_compaction_artifact(
                    &thread_id,
                    turn_id,
                    super::super::actions::current_epoch_millis(),
                    false,
                );
            }
            CodexLaneNotification::TurnPlanUpdated {
                thread_id,
                turn_id,
                explanation,
                plan,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                state.autopilot_chat.remember_thread(thread_id.clone());
                let updated_at_epoch_ms = super::super::actions::current_epoch_millis();
                state.autopilot_chat.set_plan_artifact(
                    &thread_id,
                    turn_id,
                    explanation,
                    plan.into_iter()
                        .map(|step| crate::app_state::AutopilotTurnPlanStep {
                            step: step.step,
                            status: step.status,
                        })
                        .collect(),
                    updated_at_epoch_ms,
                    false,
                );
            }
            CodexLaneNotification::ThreadTokenUsageUpdated {
                thread_id,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                ..
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.set_token_usage(
                        input_tokens,
                        cached_input_tokens,
                        output_tokens,
                    );
                }
            }
            CodexLaneNotification::TurnError {
                thread_id,
                turn_id,
                message,
            } => {
                let thread_id = resolve_thread_id(state, thread_id);
                let turn_id = resolve_turn_id(state, turn_id);
                tracing::info!(
                    "codex turn/error thread_id={} turn_id={} message={} active_thread={:?}",
                    thread_id,
                    turn_id,
                    message,
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.remember_thread(thread_id.clone());
                if state.autopilot_chat.is_active_thread(&thread_id) {
                    state.autopilot_chat.mark_turn_error_for(&turn_id, message);
                }
            }
            CodexLaneNotification::CommandApprovalRequested {
                request_id,
                request,
            } => {
                let approval_request = crate::app_state::AutopilotApprovalRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    item_id: request.item_id,
                    reason: request.reason,
                    command: request.command,
                    cwd: request.cwd,
                };
                tracing::info!(
                    "codex command/approval requested thread_id={} turn_id={} item_id={} request_id={:?} active_thread={:?}",
                    approval_request.thread_id,
                    approval_request.turn_id,
                    approval_request.item_id,
                    approval_request.request_id,
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.record_turn_command_approval_requested(
                    approval_request.turn_id.as_str(),
                    approval_request.item_id.as_str(),
                    approval_request.reason.as_deref(),
                    approval_request.command.as_deref(),
                    approval_request.cwd.as_deref(),
                    current_epoch_millis(),
                );
                state
                    .autopilot_chat
                    .enqueue_command_approval(approval_request);
                state
                    .autopilot_chat
                    .record_turn_timeline_event("command approval requested");
            }
            CodexLaneNotification::FileChangeApprovalRequested {
                request_id,
                request,
            } => {
                let approval_request = crate::app_state::AutopilotFileChangeApprovalRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    item_id: request.item_id,
                    reason: request.reason,
                    grant_root: request.grant_root,
                };
                state
                    .autopilot_chat
                    .record_turn_file_change_approval_requested(
                        approval_request.turn_id.as_str(),
                        approval_request.item_id.as_str(),
                        approval_request.reason.as_deref(),
                        approval_request.grant_root.as_deref(),
                        current_epoch_millis(),
                    );
                state
                    .autopilot_chat
                    .enqueue_file_change_approval(approval_request);
                state
                    .autopilot_chat
                    .record_turn_timeline_event("file-change approval requested");
            }
            CodexLaneNotification::ToolCallRequested {
                request_id,
                request,
            } => {
                let pending = crate::app_state::AutopilotToolCallRequest {
                    request_id,
                    thread_id: request.thread_id,
                    turn_id: request.turn_id,
                    call_id: request.call_id,
                    tool: request.tool,
                    arguments: request.arguments,
                };
                tracing::info!(
                    "codex tool/call requested thread_id={} turn_id={} call_id={} tool={} args_chars={} request_id={:?} active_thread={:?}",
                    pending.thread_id,
                    pending.turn_id,
                    pending.call_id,
                    pending.tool,
                    pending.arguments.chars().count(),
                    pending.request_id,
                    state.autopilot_chat.active_thread_id
                );
                state.autopilot_chat.record_turn_tool_request(
                    pending.turn_id.as_str(),
                    format!("{:?}", pending.request_id).as_str(),
                    pending.call_id.as_str(),
                    pending.tool.as_str(),
                    pending.arguments.as_str(),
                    current_epoch_millis(),
                );

                if super::super::tool_bridge::is_openagents_tool_namespace(&pending.tool) {
                    let is_cad_intent_tool =
                        super::super::tool_bridge::is_openagents_cad_intent_tool(
                            pending.tool.trim(),
                        );
                    if is_cad_intent_tool {
                        if let Err(error) = state
                            .cad_demo
                            .begin_agent_build_session(&pending.thread_id, &pending.turn_id)
                        {
                            state.cad_demo.last_error =
                                Some(format!("CAD build session start failed: {error}"));
                        } else if let Err(error) = state.cad_demo.transition_agent_build_phase(
                            CadBuildSessionPhase::Applying,
                            "cad.build.applying.start",
                            format!(
                                "tool={} call_id={}",
                                pending.tool.trim(),
                                pending.call_id.trim()
                            ),
                        ) {
                            state.cad_demo.last_error =
                                Some(format!("CAD build phase transition failed: {error}"));
                        }
                        super::sync_cad_build_progress_to_chat(state);
                    }

                    let envelope =
                        super::super::tool_bridge::execute_openagents_tool_request(state, &pending);
                    let recorded_at_epoch_seconds = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_secs())
                        .unwrap_or(0);
                    state.goal_loop_executor.record_tool_invocation(
                        format!("{:?}", pending.request_id).as_str(),
                        pending.call_id.as_str(),
                        pending.tool.as_str(),
                        envelope.code.as_str(),
                        envelope.success,
                        envelope.message.as_str(),
                        recorded_at_epoch_seconds,
                    );
                    state.autopilot_chat.record_turn_tool_result(
                        pending.turn_id.as_str(),
                        format!("{:?}", pending.request_id).as_str(),
                        pending.call_id.as_str(),
                        pending.tool.as_str(),
                        envelope.code.as_str(),
                        envelope.success,
                        envelope.message.as_str(),
                        current_epoch_millis(),
                    );
                    if is_cad_intent_tool {
                        state.cad_demo.record_agent_build_tool_result(
                            &envelope.code,
                            envelope.success,
                            &envelope.message,
                        );
                    }
                    let code = envelope.code.clone();
                    let success = envelope.success;
                    let submit_result = if is_cad_intent_tool {
                        queue_cad_tool_response_with_retry(
                            state,
                            &pending,
                            &envelope,
                            CAD_TOOL_RESPONSE_SUBMIT_RETRY_LIMIT,
                        )
                    } else {
                        let command =
                            crate::codex_lane::CodexLaneCommand::ServerRequestToolCallRespond {
                                request_id: pending.request_id.clone(),
                                response: envelope.to_response(),
                            };
                        state.queue_codex_command(command).map(|_| 0u8)
                    };
                    match submit_result {
                        Err(error) => {
                            tracing::info!(
                                "codex tool/call auto-response failed call_id={} tool={} code={} success={} error={}",
                                pending.call_id,
                                pending.tool,
                                code,
                                success,
                                error
                            );
                            state.autopilot_chat.enqueue_tool_call(pending);
                            state.autopilot_chat.last_error =
                                Some(format!("Failed to auto-respond to tool call: {}", error));
                            if is_cad_intent_tool {
                                state.cad_demo.record_agent_build_failure_metric(
                                    CadBuildFailureClass::ToolTransport,
                                );
                                fail_active_cad_build_session(
                                    state,
                                    CadBuildFailureClass::ToolTransport,
                                    "cad.build.response.submit_failed",
                                    format!("failed to submit CAD tool response: {error}"),
                                    "verify Codex lane connectivity and retry CAD turn".to_string(),
                                    CAD_TOOL_RESPONSE_SUBMIT_RETRY_LIMIT,
                                    CAD_TOOL_RESPONSE_SUBMIT_RETRY_LIMIT,
                                );
                            }
                        }
                        Ok(retries_used) => {
                            tracing::info!(
                                "codex tool/call auto-response submitted call_id={} tool={} code={} success={} retries_used={}",
                                pending.call_id,
                                pending.tool,
                                code,
                                success,
                                retries_used
                            );
                            if !success {
                                state.autopilot_chat.last_error =
                                    Some(format!("Tool call failed with code {}", code));
                            }
                            if is_cad_intent_tool {
                                if success {
                                    if state.cad_demo.pending_rebuild_request_id.is_some() {
                                    if let Err(error) = state.cad_demo.transition_agent_build_phase(
                                        CadBuildSessionPhase::Rebuilding,
                                        "cad.build.rebuilding.wait",
                                        format!(
                                            "waiting for request_id={}",
                                            state.cad_demo.pending_rebuild_request_id.unwrap_or(0)
                                        ),
                                    ) {
                                        state.cad_demo.last_error = Some(format!(
                                            "CAD build phase transition failed: {error}"
                                        ));
                                    }
                                } else if let Err(error) =
                                    state.cad_demo.transition_agent_build_phase(
                                        CadBuildSessionPhase::Summarizing,
                                        "cad.build.summarizing.start",
                                        "tool applied without queued rebuild".to_string(),
                                    )
                                {
                                    state.cad_demo.last_error = Some(format!(
                                        "CAD build phase transition failed: {error}"
                                    ));
                                } else if let Err(error) =
                                    state.cad_demo.complete_agent_build_session(format!(
                                        "cad intent tool call {} completed without background rebuild",
                                        pending.call_id
                                    ))
                                {
                                    state.cad_demo.last_error =
                                        Some(format!("CAD build session finalize failed: {error}"));
                                }
                                } else {
                                    let class = cad_failure_class_from_tool_response(&envelope);
                                    let retry_attempts = envelope
                                        .details
                                        .pointer("/retries/parse_retry_count")
                                        .and_then(serde_json::Value::as_u64)
                                        .or_else(|| {
                                            envelope
                                                .details
                                                .pointer("/retries/dispatch_retry_count")
                                                .and_then(serde_json::Value::as_u64)
                                        })
                                        .unwrap_or(0)
                                        as u8;
                                    let retry_limit = envelope
                                        .details
                                        .pointer("/retries/parse_retry_limit")
                                        .and_then(serde_json::Value::as_u64)
                                        .or_else(|| {
                                            envelope
                                                .details
                                                .pointer("/retries/dispatch_retry_limit")
                                                .and_then(serde_json::Value::as_u64)
                                        })
                                        .unwrap_or(0)
                                        as u8;
                                    let event_code = match class {
                                        CadBuildFailureClass::IntentParseValidation => {
                                            "cad.build.intent.parse_failed"
                                        }
                                        CadBuildFailureClass::DispatchRebuild => {
                                            "cad.build.dispatch.failed"
                                        }
                                        CadBuildFailureClass::ToolTransport => {
                                            "cad.build.response.submit_failed"
                                        }
                                    };
                                    state.cad_demo.record_agent_build_failure_metric(class);
                                    let remediation_hint =
                                        cad_failure_hint_from_tool_response(&envelope);
                                    fail_active_cad_build_session(
                                        state,
                                        class,
                                        event_code,
                                        format!(
                                            "CAD intent tool failed class={} code={} message={}",
                                            class.label(),
                                            code,
                                            envelope.message
                                        ),
                                        remediation_hint,
                                        retry_attempts,
                                        retry_limit,
                                    );
                                }
                            }
                            state.autopilot_chat.record_turn_timeline_event(format!(
                                "tool call auto-response submitted code={} success={}",
                                code, success
                            ));
                        }
                    }
                    if is_cad_intent_tool {
                        super::sync_cad_build_progress_to_chat(state);
                    }
                } else {
                    state.autopilot_chat.enqueue_tool_call(pending);
                    state
                        .autopilot_chat
                        .record_turn_timeline_event("tool call requested");
                }
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
            CodexLaneNotification::ServerRequest { method } => {
                tracing::info!(
                    "codex server/request method={} active_thread={:?}",
                    method,
                    state.autopilot_chat.active_thread_id
                );
            }
            CodexLaneNotification::Raw { method } => {
                tracing::info!(
                    "codex notify/raw method={} active_thread={:?}",
                    method,
                    state.autopilot_chat.active_thread_id
                );
            }
        }
    }

    let method_label = notification_method_label(&stored);
    increment_diagnostics_count(
        &mut state.codex_diagnostics.notification_counts,
        method_label.as_str(),
    );
    if let CodexLaneNotification::ServerRequest { method } = &stored {
        increment_diagnostics_count(
            &mut state.codex_diagnostics.server_request_counts,
            method.as_str(),
        );
    }
    push_diagnostics_event(state, format!("notify {}", method_label));
    state.codex_diagnostics.load_state = PaneLoadState::Ready;
    state.codex_diagnostics.last_action = Some(format!("notification {}", method_label));

    state.sync_health.last_action = Some("codex notification received".to_string());
    state.record_codex_notification(stored);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CodexConfigTruth {
    summary: String,
    requirements_summary: String,
    constraint_summary: Option<String>,
}

fn refresh_codex_config_truth(state: &mut RenderState) {
    let truth = derive_codex_config_truth(
        &state.codex_config.config_json,
        &state.codex_config.requirements_json,
        &state.codex_config.layers_json,
    );
    state.codex_config.summary = truth.summary;
    state.codex_config.requirements_summary = truth.requirements_summary;
    state.codex_config.constraint_summary = truth.constraint_summary;
}

fn refresh_codex_readiness_summary(state: &mut RenderState) {
    state.codex_account.config_summary = state.codex_config.summary.clone();
    state.codex_account.config_requirements_summary =
        state.codex_config.requirements_summary.clone();
    state.codex_account.config_constraint_summary = state.codex_config.constraint_summary.clone();
    state.codex_account.readiness_summary = derive_codex_readiness_summary(
        &state.codex_account,
        &state.codex_config,
        &state.codex_lane,
    );
}

fn derive_codex_readiness_summary(
    account: &crate::app_state::CodexAccountPaneState,
    config: &crate::app_state::CodexConfigPaneState,
    lane: &CodexLaneSnapshot,
) -> String {
    if !account.install_available {
        return lane
            .install_probe
            .error
            .as_deref()
            .map(|error| format!("blocked: {error}"))
            .unwrap_or_else(|| "blocked: codex executable not found".to_string());
    }

    match lane.lifecycle {
        CodexLaneLifecycle::Starting => return "starting: launching Codex lane".to_string(),
        CodexLaneLifecycle::Error => {
            return format!(
                "blocked: {}",
                lane.last_error
                    .as_deref()
                    .unwrap_or("Codex lane reported an error")
            );
        }
        CodexLaneLifecycle::Disconnected => {
            return format!(
                "blocked: {}",
                lane.last_error
                    .as_deref()
                    .unwrap_or("Codex lane disconnected")
            );
        }
        CodexLaneLifecycle::Stopped => return "stopped".to_string(),
        CodexLaneLifecycle::Ready => {}
    }

    if account.pending_login_id.is_some() {
        return "auth pending: complete ChatGPT login in browser".to_string();
    }
    if account.load_state == PaneLoadState::Loading || config.load_state == PaneLoadState::Loading {
        return "probing account and config truth".to_string();
    }
    if account.load_state == PaneLoadState::Error {
        return format!(
            "degraded: {}",
            account
                .last_error
                .as_deref()
                .unwrap_or("account/read failed")
        );
    }
    if config.load_state == PaneLoadState::Error {
        return format!(
            "degraded: {}",
            config.last_error.as_deref().unwrap_or("config/read failed")
        );
    }
    if account.requires_openai_auth
        && matches!(account.account_summary.as_str(), "none" | "unknown")
    {
        return "auth required: sign in before starting work".to_string();
    }
    if let Some(constraint) = account.config_constraint_summary.as_deref() {
        return format!("ready with managed constraints: {constraint}");
    }
    "ready".to_string()
}

fn derive_codex_config_truth(
    config_json: &str,
    requirements_json: &str,
    layers_json: &str,
) -> CodexConfigTruth {
    let config = serde_json::from_str::<serde_json::Value>(config_json).unwrap_or_default();
    let requirements =
        serde_json::from_str::<serde_json::Value>(requirements_json).unwrap_or_default();
    let layers = serde_json::from_str::<serde_json::Value>(layers_json).unwrap_or_default();

    let effective_approval = json_string_field(&config, "approvalPolicy");
    let effective_sandbox = json_string_field(&config, "sandboxMode");
    let writable_roots = config
        .get("sandboxWorkspaceWrite")
        .and_then(|value| value.get("writableRoots"))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let summary = {
        let mut parts = vec![
            format!(
                "approval={}",
                effective_approval.as_deref().unwrap_or("n/a")
            ),
            format!("sandbox={}", effective_sandbox.as_deref().unwrap_or("n/a")),
        ];
        if writable_roots > 0 {
            parts.push(format!("writable_roots={writable_roots}"));
        }
        parts.join(" ")
    };

    let allowed_approvals = json_string_array_field(&requirements, "allowedApprovalPolicies");
    let allowed_sandboxes = json_string_array_field(&requirements, "allowedSandboxModes");
    let allowed_web_search = json_string_array_field(&requirements, "allowedWebSearchModes");
    let feature_count = requirements
        .get("featureRequirements")
        .and_then(serde_json::Value::as_object)
        .map_or(0, |value| value.len());
    let residency = json_string_field(&requirements, "enforceResidency");
    let requirements_summary = {
        let mut parts = Vec::new();
        if !allowed_approvals.is_empty() {
            parts.push(format!("approval={}", allowed_approvals.join(",")));
        }
        if !allowed_sandboxes.is_empty() {
            parts.push(format!("sandbox={}", allowed_sandboxes.join(",")));
        }
        if !allowed_web_search.is_empty() {
            parts.push(format!("web={}", allowed_web_search.join(",")));
        }
        if let Some(value) = residency.as_deref() {
            parts.push(format!("residency={value}"));
        }
        if feature_count > 0 {
            parts.push(format!("features={feature_count}"));
        }
        if let Some(network) = requirements.get("network") {
            let enabled = network
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .map(|value| if value { "enabled" } else { "disabled" })
                .unwrap_or("managed");
            let allowed_domains = network
                .get("allowedDomains")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            let denied_domains = network
                .get("deniedDomains")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            parts.push(format!(
                "network={enabled} allow_domains={allowed_domains} deny_domains={denied_domains}"
            ));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" ")
        }
    };

    let selected_approval = first_layer_config_value(&layers, "approvalPolicy");
    let selected_sandbox = first_layer_config_value(&layers, "sandboxMode");
    let mut constraints = Vec::new();
    if let Some(message) = constraint_message(
        "approval",
        selected_approval.as_ref(),
        effective_approval.as_deref(),
        &allowed_approvals,
    ) {
        constraints.push(message);
    }
    if let Some(message) = constraint_message(
        "sandbox",
        selected_sandbox.as_ref(),
        effective_sandbox.as_deref(),
        &allowed_sandboxes,
    ) {
        constraints.push(message);
    }

    CodexConfigTruth {
        summary,
        requirements_summary,
        constraint_summary: (!constraints.is_empty()).then(|| constraints.join(" | ")),
    }
}

fn json_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn json_string_array_field(value: &serde_json::Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn first_layer_config_value(layers: &serde_json::Value, field: &str) -> Option<(String, String)> {
    layers.as_array().and_then(|entries| {
        entries.iter().find_map(|layer| {
            let value = layer
                .get("config")
                .and_then(|config| config.get(field))
                .and_then(serde_json::Value::as_str)?;
            let source = layer
                .get("name")
                .map(config_layer_source_label)
                .unwrap_or_else(|| "unknown".to_string());
            Some((value.to_string(), source))
        })
    })
}

fn config_layer_source_label(value: &serde_json::Value) -> String {
    if let Some(value) = value.as_str() {
        return value.to_string();
    }
    let Some(object) = value.as_object() else {
        return "unknown".to_string();
    };
    let Some((kind, details)) = object.iter().next() else {
        return "unknown".to_string();
    };
    if let Some(file) = details.get("file").and_then(serde_json::Value::as_str) {
        return std::path::Path::new(file)
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| format!("{kind}:{value}"))
            .unwrap_or_else(|| format!("{kind}:{file}"));
    }
    kind.to_string()
}

fn constraint_message(
    field: &str,
    selected: Option<&(String, String)>,
    effective: Option<&str>,
    allowed: &[String],
) -> Option<String> {
    if let Some(effective) = effective
        && !allowed.is_empty()
        && !allowed.iter().any(|value| value == effective)
    {
        return Some(format!(
            "effective {field}={effective} is outside requirements ({})",
            allowed.join(",")
        ));
    }
    let Some((selected_value, source)) = selected else {
        return None;
    };
    let Some(effective) = effective else {
        return None;
    };
    if selected_value == effective || allowed.is_empty() {
        return None;
    }
    if allowed.iter().any(|value| value == selected_value) {
        return None;
    }
    Some(format!(
        "selected {field}={selected_value} from {source} is disallowed by requirements ({}); effective {field}={effective}",
        allowed.join(",")
    ))
}

fn resolve_thread_id(state: &RenderState, thread_id: String) -> String {
    if thread_id.trim().is_empty() || thread_id == "unknown-thread" {
        if let Some(active_thread_id) = state.autopilot_chat.active_thread_id.clone() {
            return active_thread_id;
        }
    }
    thread_id
}

fn resolve_turn_id(state: &RenderState, turn_id: String) -> String {
    if turn_id.trim().is_empty() || turn_id == "unknown-turn" {
        if let Some(active_turn_id) = state.autopilot_chat.active_turn_id.clone() {
            return active_turn_id;
        }
    }
    turn_id
}

fn log_chat_message_to_console(
    role: &str,
    thread_id: &str,
    turn_id: &str,
    source: &str,
    message: &str,
) {
    const MAX_CHARS: usize = 8_000;
    let trimmed = message.trim_end();
    if trimmed.trim().is_empty() {
        return;
    }
    static CHAT_MIRROR_STATE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, String>>,
    > = std::sync::OnceLock::new();
    let state =
        CHAT_MIRROR_STATE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let key = format!("{thread_id}:{turn_id}:{role}");
    let mut guard = match state.lock() {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("chat transcript mirror lock poisoned: {}", error);
            return;
        }
    };
    if guard.get(&key).is_some_and(|previous| previous == trimmed) {
        return;
    }
    guard.insert(key, trimmed.to_string());
    let chars = trimmed.chars().count();
    let (body, truncated) = if chars > MAX_CHARS {
        (trimmed.chars().take(MAX_CHARS).collect::<String>(), true)
    } else {
        (trimmed.to_string(), false)
    };
    if truncated {
        tracing::info!(
            "autopilot transcript/{} source={} thread_id={} turn_id={} chars={} (truncated to {})\n{}",
            role,
            source,
            thread_id,
            turn_id,
            chars,
            MAX_CHARS,
            body
        );
    } else {
        tracing::info!(
            "autopilot transcript/{} source={} thread_id={} turn_id={} chars={}\n{}",
            role,
            source,
            thread_id,
            turn_id,
            chars,
            body
        );
    }
}

fn increment_diagnostics_count(
    entries: &mut Vec<crate::app_state::CodexDiagnosticsMethodCountState>,
    method: &str,
) {
    if let Some(entry) = entries.iter_mut().find(|entry| entry.method == method) {
        entry.count = entry.count.saturating_add(1);
    } else {
        entries.push(crate::app_state::CodexDiagnosticsMethodCountState {
            method: method.to_string(),
            count: 1,
        });
    }
    entries.sort_by(|lhs, rhs| {
        rhs.count
            .cmp(&lhs.count)
            .then_with(|| lhs.method.cmp(&rhs.method))
    });
}

fn push_diagnostics_event(state: &mut RenderState, line: String) {
    state.codex_diagnostics.raw_events.push(line);
    if state.codex_diagnostics.raw_events.len() > 96 {
        let overflow = state.codex_diagnostics.raw_events.len().saturating_sub(96);
        state.codex_diagnostics.raw_events.drain(0..overflow);
    }
}

fn notification_method_label(notification: &CodexLaneNotification) -> String {
    match notification {
        CodexLaneNotification::SkillsListLoaded { .. } => "skills/list".to_string(),
        CodexLaneNotification::ModelsLoaded { .. } => "model/list".to_string(),
        CodexLaneNotification::ModelCatalogLoaded { .. } => "model/list".to_string(),
        CodexLaneNotification::ModelRerouted { .. } => "model/rerouted".to_string(),
        CodexLaneNotification::AccountLoaded { .. } => "account/read".to_string(),
        CodexLaneNotification::AccountRateLimitsLoaded { .. } => {
            "account/rateLimits/read".to_string()
        }
        CodexLaneNotification::AccountUpdated { .. } => "account/updated".to_string(),
        CodexLaneNotification::AccountLoginStarted { .. } => "account/login/start".to_string(),
        CodexLaneNotification::AccountLoginCompleted { .. } => {
            "account/login/completed".to_string()
        }
        CodexLaneNotification::ConfigLoaded { .. } => "config/read".to_string(),
        CodexLaneNotification::ConfigRequirementsLoaded { .. } => {
            "configRequirements/read".to_string()
        }
        CodexLaneNotification::ConfigWriteApplied { .. } => "config/value/write".to_string(),
        CodexLaneNotification::ExternalAgentConfigDetected { .. } => {
            "externalAgentConfig/detect".to_string()
        }
        CodexLaneNotification::ExternalAgentConfigImported => {
            "externalAgentConfig/import".to_string()
        }
        CodexLaneNotification::McpServerStatusListLoaded { .. } => {
            "mcpServerStatus/list".to_string()
        }
        CodexLaneNotification::McpServerOauthLoginStarted { .. } => {
            "mcpServer/oauth/login".to_string()
        }
        CodexLaneNotification::McpServerOauthLoginCompleted { .. } => {
            "mcpServer/oauthLogin/completed".to_string()
        }
        CodexLaneNotification::McpServerReloaded => "config/mcpServer/reload".to_string(),
        CodexLaneNotification::AppsListLoaded { .. } => "app/list".to_string(),
        CodexLaneNotification::AppsListUpdated => "app/list/updated".to_string(),
        CodexLaneNotification::ReviewStarted { .. } => "review/start".to_string(),
        CodexLaneNotification::CommandExecCompleted { .. } => "command/exec".to_string(),
        CodexLaneNotification::CollaborationModesLoaded { .. } => {
            "collaborationMode/list".to_string()
        }
        CodexLaneNotification::ExperimentalFeaturesLoaded { .. } => {
            "experimentalFeature/list".to_string()
        }
        CodexLaneNotification::RealtimeStarted { .. } => "thread/realtime/started".to_string(),
        CodexLaneNotification::RealtimeTextAppended { .. } => {
            "thread/realtime/appendText".to_string()
        }
        CodexLaneNotification::RealtimeStopped { .. } => "thread/realtime/closed".to_string(),
        CodexLaneNotification::RealtimeError { .. } => "thread/realtime/error".to_string(),
        CodexLaneNotification::WindowsSandboxSetupStarted { .. } => {
            "windowsSandbox/setupStart".to_string()
        }
        CodexLaneNotification::WindowsSandboxSetupCompleted { .. } => {
            "windowsSandbox/setupCompleted".to_string()
        }
        CodexLaneNotification::FuzzySessionStarted { .. } => {
            "fuzzyFileSearch/sessionStart".to_string()
        }
        CodexLaneNotification::FuzzySessionUpdated { .. } => {
            "fuzzyFileSearch/sessionUpdated".to_string()
        }
        CodexLaneNotification::FuzzySessionCompleted { .. } => {
            "fuzzyFileSearch/sessionCompleted".to_string()
        }
        CodexLaneNotification::FuzzySessionStopped { .. } => {
            "fuzzyFileSearch/sessionStop".to_string()
        }
        CodexLaneNotification::SkillsRemoteListLoaded { .. } => "skills/remote/list".to_string(),
        CodexLaneNotification::SkillsRemoteExported { .. } => "skills/remote/export".to_string(),
        CodexLaneNotification::ThreadListLoaded { .. } => "thread/list".to_string(),
        CodexLaneNotification::ThreadLoadedListLoaded { .. } => "thread/loaded/list".to_string(),
        CodexLaneNotification::ThreadReadLoaded { .. } => "thread/read".to_string(),
        CodexLaneNotification::ThreadSelected { .. } => "thread/selected".to_string(),
        CodexLaneNotification::ThreadStarted { .. } => "thread/started".to_string(),
        CodexLaneNotification::ThreadSessionConfigured { .. } => {
            "thread/sessionConfigured".to_string()
        }
        CodexLaneNotification::ThreadStatusChanged { .. } => "thread/status/changed".to_string(),
        CodexLaneNotification::ThreadArchived { .. } => "thread/archived".to_string(),
        CodexLaneNotification::ThreadUnarchived { .. } => "thread/unarchived".to_string(),
        CodexLaneNotification::ThreadClosed { .. } => "thread/closed".to_string(),
        CodexLaneNotification::ThreadNameUpdated { .. } => "thread/name/updated".to_string(),
        CodexLaneNotification::TurnStarted { .. } => "turn/started".to_string(),
        CodexLaneNotification::ItemStarted { .. } => "item/started".to_string(),
        CodexLaneNotification::ItemCompleted { .. } => "item/completed".to_string(),
        CodexLaneNotification::AgentMessageDelta { .. } => "item/agentMessage/delta".to_string(),
        CodexLaneNotification::AgentMessageCompleted { .. } => {
            "item/agentMessage/completed".to_string()
        }
        CodexLaneNotification::ReasoningDelta { .. } => {
            "item/reasoning/summaryTextDelta".to_string()
        }
        CodexLaneNotification::TurnCompleted { .. } => "turn/completed".to_string(),
        CodexLaneNotification::TurnDiffUpdated { .. } => "turn/diff/updated".to_string(),
        CodexLaneNotification::ReviewProgressUpdated { .. } => "item/review/progress".to_string(),
        CodexLaneNotification::TurnPlanUpdated { .. } => "turn/plan/updated".to_string(),
        CodexLaneNotification::ThreadCompacted { .. } => "thread/compacted".to_string(),
        CodexLaneNotification::ThreadTokenUsageUpdated { .. } => {
            "thread/tokenUsage/updated".to_string()
        }
        CodexLaneNotification::TurnError { .. } => "error".to_string(),
        CodexLaneNotification::CommandApprovalRequested { .. } => {
            "item/commandExecution/requestApproval".to_string()
        }
        CodexLaneNotification::FileChangeApprovalRequested { .. } => {
            "item/fileChange/requestApproval".to_string()
        }
        CodexLaneNotification::ToolCallRequested { .. } => "item/tool/call".to_string(),
        CodexLaneNotification::ToolUserInputRequested { .. } => {
            "item/tool/requestUserInput".to_string()
        }
        CodexLaneNotification::AuthTokensRefreshRequested { .. } => {
            "account/chatgptAuthTokens/refresh".to_string()
        }
        CodexLaneNotification::ServerRequest { method } | CodexLaneNotification::Raw { method } => {
            method.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cad_failure_class_from_tool_response, cad_failure_hint_from_tool_response,
        derive_codex_config_truth, derive_codex_readiness_summary,
    };
    use crate::app_state::{
        CadBuildFailureClass, CodexAccountPaneState, CodexConfigPaneState, PaneLoadState,
    };
    use crate::codex_lane::{CodexLaneLifecycle, CodexLaneSnapshot};
    use crate::input::tool_bridge::ToolBridgeResultEnvelope;
    use codex_client::CodexInstallationProbe;
    use serde_json::json;

    #[test]
    fn cad_failure_class_resolution_prefers_explicit_detail_and_has_code_fallback() {
        let explicit = ToolBridgeResultEnvelope {
            success: false,
            code: "OA-CAD-INTENT-REBUILD-ENQUEUE-FAILED".to_string(),
            message: "dispatch failed".to_string(),
            details: json!({
                "failure_class": "dispatch_rebuild",
                "fallback": {
                    "remediation_hint": "retry with narrower intent_json"
                }
            }),
        };
        assert_eq!(
            cad_failure_class_from_tool_response(&explicit),
            CadBuildFailureClass::DispatchRebuild
        );
        assert_eq!(
            cad_failure_hint_from_tool_response(&explicit),
            "retry with narrower intent_json".to_string()
        );

        let fallback = ToolBridgeResultEnvelope {
            success: false,
            code: "OA-CAD-INTENT-PARSE-FAILED".to_string(),
            message: "parse failed".to_string(),
            details: json!({}),
        };
        assert_eq!(
            cad_failure_class_from_tool_response(&fallback),
            CadBuildFailureClass::IntentParseValidation
        );
    }

    #[test]
    fn codex_config_truth_reports_disallowed_selection_from_layers() {
        let truth = derive_codex_config_truth(
            &json!({
                "approvalPolicy": "on-request",
                "sandboxMode": "read-only"
            })
            .to_string(),
            &json!({
                "allowedApprovalPolicies": ["on-request"],
                "allowedSandboxModes": ["read-only"]
            })
            .to_string(),
            &json!([
                {
                    "name": {"user": {"file": "/tmp/config.toml"}},
                    "version": "user-v1",
                    "config": {
                        "approvalPolicy": "never",
                        "sandboxMode": "danger-full-access"
                    }
                }
            ])
            .to_string(),
        );

        assert_eq!(truth.summary, "approval=on-request sandbox=read-only");
        assert_eq!(
            truth.requirements_summary,
            "approval=on-request sandbox=read-only"
        );
        let constraint = truth
            .constraint_summary
            .as_deref()
            .unwrap_or("missing constraint summary");
        assert!(constraint.contains("selected approval=never"));
        assert!(constraint.contains("selected sandbox=danger-full-access"));
        assert!(constraint.contains("effective sandbox=read-only"));
    }

    #[test]
    fn readiness_summary_blocks_when_install_missing_and_requires_auth() {
        let mut account = CodexAccountPaneState::default();
        account.load_state = PaneLoadState::Ready;
        account.install_available = false;
        account.requires_openai_auth = true;
        account.account_summary = "none".to_string();

        let config = CodexConfigPaneState::default();
        let lane = CodexLaneSnapshot {
            lifecycle: CodexLaneLifecycle::Error,
            active_thread_id: None,
            last_error: Some("Codex lane startup failed: codex executable not found".to_string()),
            last_status: Some("Codex lane unavailable".to_string()),
            install_probe: CodexInstallationProbe {
                available: false,
                invocation: None,
                resolved_program: None,
                version: None,
                error: Some("codex executable not found".to_string()),
            },
        };

        assert_eq!(
            derive_codex_readiness_summary(&account, &config, &lane),
            "blocked: codex executable not found".to_string()
        );
    }
}
