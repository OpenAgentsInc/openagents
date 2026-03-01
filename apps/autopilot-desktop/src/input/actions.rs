use super::*;

pub(super) fn run_chat_submit_action(state: &mut crate::app_state::RenderState) -> bool {
    focus_chat_composer(state);
    let prompt = state.chat_inputs.composer.get_value().trim().to_string();
    let prompt_chars = prompt.chars().count();
    if prompt.is_empty() {
        state.autopilot_chat.last_error = Some("Prompt cannot be empty".to_string());
        return true;
    }
    let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
        state.autopilot_chat.last_error =
            Some("No active thread yet. Wait for Codex lane readiness.".to_string());
        return true;
    };

    state.chat_inputs.composer.set_value(String::new());
    state.autopilot_chat.submit_prompt(prompt.clone());
    let _ = super::reducers::apply_chat_prompt_to_cad_session(state, &thread_id, &prompt);

    let selected_skill = state
        .skill_registry
        .selected_skill_index
        .and_then(|index| state.skill_registry.discovered_skills.get(index))
        .map(|skill| (skill.name.as_str(), skill.path.as_str(), skill.enabled));
    let (input, skill_error) = assemble_chat_turn_input(prompt, selected_skill);
    if let Some(skill_error) = skill_error {
        state.autopilot_chat.last_error = Some(skill_error);
    }
    let model_override = state.autopilot_chat.selected_model_override();
    let model_label = model_override.as_deref().unwrap_or("server-default");

    let command = crate::codex_lane::CodexLaneCommand::TurnStart(TurnStartParams {
        thread_id: thread_id.clone(),
        input,
        cwd: None,
        approval_policy: None,
        sandbox_policy: None,
        model: model_override.clone(),
        effort: None,
        summary: None,
        personality: None,
        output_schema: None,
        collaboration_mode: None,
    });

    tracing::info!(
        "codex turn/start request thread_id={} model={} chars={}",
        thread_id,
        model_label,
        prompt_chars
    );
    match state.queue_codex_command(command) {
        Ok(seq) => {
            tracing::info!(
                "codex turn/start queued seq={} thread_id={}",
                seq,
                thread_id
            );
        }
        Err(error) => {
            state
                .autopilot_chat
                .mark_pending_turn_dispatch_failed(error);
        }
    }
    true
}

pub(super) fn assemble_chat_turn_input(
    prompt: String,
    selected_skill: Option<(&str, &str, bool)>,
) -> (Vec<UserInput>, Option<String>) {
    let mut input = vec![UserInput::Text {
        text: prompt,
        text_elements: Vec::new(),
    }];
    let mut last_error = None;
    if let Some((name, path, enabled)) = selected_skill {
        if enabled {
            input.push(UserInput::Skill {
                name: name.to_string(),
                path: std::path::PathBuf::from(path),
            });
        } else {
            last_error = Some(format!(
                "Selected skill '{}' is disabled; enable it first.",
                name
            ));
        }
    }

    (input, last_error)
}

pub(super) fn run_chat_refresh_threads_action(state: &mut crate::app_state::RenderState) -> bool {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    let params = state.autopilot_chat.build_thread_list_params(cwd);
    let command = crate::codex_lane::CodexLaneCommand::ThreadList(params);
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    } else {
        state.autopilot_chat.last_error = None;
        if let Err(error) = state.queue_codex_command(
            crate::codex_lane::CodexLaneCommand::ThreadLoadedList(ThreadLoadedListParams {
                cursor: None,
                limit: Some(200),
            }),
        ) {
            state.autopilot_chat.last_error = Some(error);
        }
    }
    true
}

pub(super) fn run_chat_new_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    focus_chat_composer(state);
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    let model_override = state.autopilot_chat.selected_model_override();
    let command = crate::codex_lane::CodexLaneCommand::ThreadStart(ThreadStartParams {
        model: model_override,
        model_provider: None,
        cwd,
        approval_policy: None,
        sandbox: None,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    } else {
        state.autopilot_chat.last_error = None;
    }
    true
}

pub(super) fn run_chat_cycle_model_action(state: &mut crate::app_state::RenderState) -> bool {
    state.autopilot_chat.cycle_model();
    true
}

pub(super) fn run_chat_toggle_archived_filter_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_thread_filter_archived();
    run_chat_refresh_threads_action(state)
}

pub(super) fn run_chat_cycle_sort_filter_action(state: &mut crate::app_state::RenderState) -> bool {
    state.autopilot_chat.cycle_thread_filter_sort_key();
    run_chat_refresh_threads_action(state)
}

pub(super) fn run_chat_cycle_source_filter_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_thread_filter_source_kind();
    run_chat_refresh_threads_action(state)
}

pub(super) fn run_chat_cycle_provider_filter_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_thread_filter_model_provider();
    run_chat_refresh_threads_action(state)
}

pub(super) fn run_chat_interrupt_turn_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
        state.autopilot_chat.last_error = Some("No active thread to interrupt".to_string());
        return true;
    };
    let Some(turn_id) = state.autopilot_chat.active_turn_id.clone() else {
        state.autopilot_chat.last_error = Some("No active turn to interrupt".to_string());
        return true;
    };

    let command =
        crate::codex_lane::CodexLaneCommand::TurnInterrupt(codex_client::TurnInterruptParams {
            thread_id,
            turn_id: turn_id.clone(),
        });
    match state.queue_codex_command(command) {
        Ok(_) => {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("interrupt requested: {turn_id}"));
            state
                .autopilot_chat
                .set_turn_status(Some("interruptRequested".to_string()));
            state.autopilot_chat.last_error = None;
        }
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
        }
    }
    true
}

pub(super) fn active_thread_id(state: &crate::app_state::RenderState) -> Option<String> {
    state.autopilot_chat.active_thread_id.clone()
}

pub(super) fn run_chat_fork_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to fork".to_string());
        return true;
    };
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    let model_override = state.autopilot_chat.selected_model_override();
    let command = crate::codex_lane::CodexLaneCommand::ThreadFork(ThreadForkParams {
        thread_id,
        path: None,
        model: model_override,
        model_provider: None,
        cwd,
        approval_policy: None,
        sandbox: None,
        config: None,
        base_instructions: None,
        developer_instructions: None,
        persist_extended_history: false,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_archive_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to archive".to_string());
        return true;
    };
    if let Err(error) = state.queue_codex_command(
        crate::codex_lane::CodexLaneCommand::ThreadArchive(ThreadArchiveParams { thread_id }),
    ) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_unarchive_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to unarchive".to_string());
        return true;
    };
    if let Err(error) = state.queue_codex_command(
        crate::codex_lane::CodexLaneCommand::ThreadUnarchive(ThreadUnarchiveParams { thread_id }),
    ) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_rename_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to rename".to_string());
        return true;
    };
    let name = state.autopilot_chat.next_thread_name();
    let command = crate::codex_lane::CodexLaneCommand::ThreadNameSet(ThreadSetNameParams {
        thread_id: thread_id.clone(),
        name: name.clone(),
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    } else {
        state.autopilot_chat.set_thread_name(&thread_id, Some(name));
    }
    true
}

pub(super) fn run_chat_rollback_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to rollback".to_string());
        return true;
    };
    let command = crate::codex_lane::CodexLaneCommand::ThreadRollback(ThreadRollbackParams {
        thread_id,
        num_turns: 1,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_compact_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to compact".to_string());
        return true;
    };
    let command =
        crate::codex_lane::CodexLaneCommand::ThreadCompactStart(ThreadCompactStartParams {
            thread_id,
        });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_unsubscribe_thread_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to unsubscribe".to_string());
        return true;
    };
    let command = crate::codex_lane::CodexLaneCommand::ThreadUnsubscribe(ThreadUnsubscribeParams {
        thread_id,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    }
    true
}

pub(super) fn run_chat_select_thread_action(
    state: &mut crate::app_state::RenderState,
    index: usize,
) -> bool {
    let Some(target) = state.autopilot_chat.select_thread_by_index(index) else {
        return false;
    };
    focus_chat_composer(state);
    let experimental_api = state.codex_lane_config.experimental_api;
    let resume_path = if experimental_api {
        target.path.clone()
    } else {
        None
    };

    tracing::info!(
        "codex thread/resume target id={} cwd={:?} path={:?} experimental_api={}",
        target.thread_id,
        target.cwd,
        resume_path,
        experimental_api
    );

    let command = crate::codex_lane::CodexLaneCommand::ThreadResume(ThreadResumeParams {
        thread_id: target.thread_id,
        model: None,
        model_provider: None,
        cwd: target.cwd,
        approval_policy: None,
        sandbox: None,
        path: resume_path.map(std::path::PathBuf::from),
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
        return true;
    }

    if let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() {
        let read =
            crate::codex_lane::CodexLaneCommand::ThreadRead(codex_client::ThreadReadParams {
                thread_id,
                include_turns: true,
            });
        if let Err(error) = state.queue_codex_command(read) {
            state.autopilot_chat.last_error = Some(error);
        }
    }
    true
}

pub(super) fn run_chat_approval_response_action(
    state: &mut crate::app_state::RenderState,
    decision: ApprovalDecision,
) -> bool {
    if let Some(request) = state.autopilot_chat.pop_command_approval() {
        let command = crate::codex_lane::CodexLaneCommand::ServerRequestCommandApprovalRespond {
            request_id: request.request_id.clone(),
            response: CommandExecutionRequestApprovalResponse {
                decision: decision.clone(),
            },
        };
        if let Err(error) = state.queue_codex_command(command) {
            state
                .autopilot_chat
                .pending_command_approvals
                .insert(0, request);
            state.autopilot_chat.last_error = Some(error);
        } else {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("command approval response: {:?}", decision));
        }
        return true;
    }

    if let Some(request) = state.autopilot_chat.pop_file_change_approval() {
        let command = crate::codex_lane::CodexLaneCommand::ServerRequestFileApprovalRespond {
            request_id: request.request_id.clone(),
            response: FileChangeRequestApprovalResponse { decision },
        };
        if let Err(error) = state.queue_codex_command(command) {
            state
                .autopilot_chat
                .pending_file_change_approvals
                .insert(0, request);
            state.autopilot_chat.last_error = Some(error);
        } else {
            state
                .autopilot_chat
                .record_turn_timeline_event("file-change approval response submitted");
        }
        return true;
    }

    state.autopilot_chat.last_error = Some("No pending approval requests".to_string());
    true
}

pub(super) fn run_chat_tool_call_response_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let Some(request) = state.autopilot_chat.pop_tool_call() else {
        state.autopilot_chat.last_error = Some("No pending tool calls".to_string());
        return true;
    };

    let command = crate::codex_lane::CodexLaneCommand::ServerRequestToolCallRespond {
        request_id: request.request_id.clone(),
        response: DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: format!(
                    "OpenAgents desktop acknowledged tool '{}' for call '{}'",
                    request.tool, request.call_id
                ),
            }],
            success: true,
        },
    };
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.pending_tool_calls.insert(0, request);
        state.autopilot_chat.last_error = Some(error);
    } else {
        state
            .autopilot_chat
            .record_turn_timeline_event("tool call response submitted");
    }
    true
}

pub(super) fn run_chat_tool_user_input_response_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let Some(request) = state.autopilot_chat.pop_tool_user_input() else {
        state.autopilot_chat.last_error = Some("No pending tool user-input requests".to_string());
        return true;
    };

    let mut answers = HashMap::new();
    for question in &request.questions {
        let value = if let Some(first) = question.options.first() {
            vec![first.clone()]
        } else {
            vec!["ok".to_string()]
        };
        answers.insert(
            question.id.clone(),
            ToolRequestUserInputAnswer { answers: value },
        );
    }

    let command = crate::codex_lane::CodexLaneCommand::ServerRequestToolUserInputRespond {
        request_id: request.request_id.clone(),
        response: ToolRequestUserInputResponse { answers },
    };
    if let Err(error) = state.queue_codex_command(command) {
        state
            .autopilot_chat
            .pending_tool_user_input
            .insert(0, request);
        state.autopilot_chat.last_error = Some(error);
    } else {
        state
            .autopilot_chat
            .record_turn_timeline_event("tool user-input response submitted");
    }
    true
}

pub(super) fn run_chat_auth_refresh_response_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let Some(request) = state.autopilot_chat.pop_auth_refresh() else {
        state.autopilot_chat.last_error = Some("No pending auth refresh requests".to_string());
        return true;
    };

    let access_token = if state
        .autopilot_chat
        .auth_refresh_access_token
        .trim()
        .is_empty()
    {
        std::env::var("OPENAI_ACCESS_TOKEN").unwrap_or_default()
    } else {
        state.autopilot_chat.auth_refresh_access_token.clone()
    };
    let account_id = if state
        .autopilot_chat
        .auth_refresh_account_id
        .trim()
        .is_empty()
    {
        request
            .previous_account_id
            .clone()
            .unwrap_or_else(|| std::env::var("OPENAI_CHATGPT_ACCOUNT_ID").unwrap_or_default())
    } else {
        state.autopilot_chat.auth_refresh_account_id.clone()
    };
    let plan_type = if state
        .autopilot_chat
        .auth_refresh_plan_type
        .trim()
        .is_empty()
    {
        std::env::var("OPENAI_CHATGPT_PLAN_TYPE").unwrap_or_default()
    } else {
        state.autopilot_chat.auth_refresh_plan_type.clone()
    };

    if access_token.trim().is_empty() || account_id.trim().is_empty() {
        state.autopilot_chat.pending_auth_refresh.insert(0, request);
        state.autopilot_chat.last_error =
            Some("Missing OPENAI_ACCESS_TOKEN or account id for auth refresh response".to_string());
        return true;
    }

    let command = crate::codex_lane::CodexLaneCommand::ServerRequestAuthRefreshRespond {
        request_id: request.request_id.clone(),
        response: ChatgptAuthTokensRefreshResponse {
            access_token,
            chatgpt_account_id: account_id,
            chatgpt_plan_type: if plan_type.trim().is_empty() {
                None
            } else {
                Some(plan_type)
            },
        },
    };
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.pending_auth_refresh.insert(0, request);
        state.autopilot_chat.last_error = Some(error);
    } else {
        state
            .autopilot_chat
            .record_turn_timeline_event("auth refresh response submitted");
    }
    true
}

pub(super) fn run_codex_account_action(
    state: &mut crate::app_state::RenderState,
    action: CodexAccountPaneAction,
) -> bool {
    match action {
        CodexAccountPaneAction::Refresh => {
            state.codex_account.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_account.last_error = None;
            state.codex_account.last_action = Some("Queued account/read".to_string());
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::AccountRead(GetAccountParams {
                    refresh_token: true,
                }),
            ) {
                state.codex_account.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_account.last_error = Some(error);
            }
            true
        }
        CodexAccountPaneAction::LoginChatgpt => {
            state.codex_account.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_account.last_error = None;
            state.codex_account.last_action =
                Some("Queued account/login/start (chatgpt)".to_string());
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::AccountLoginStart(LoginAccountParams::Chatgpt),
            ) {
                state.codex_account.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_account.last_error = Some(error);
            }
            true
        }
        CodexAccountPaneAction::CancelLogin => {
            let Some(login_id) = state.codex_account.pending_login_id.clone() else {
                state.codex_account.last_error = Some("No pending login id to cancel".to_string());
                return true;
            };
            state.codex_account.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_account.last_error = None;
            state.codex_account.last_action =
                Some(format!("Queued account/login/cancel for {login_id}"));
            if let Err(error) =
                state.queue_codex_command(crate::codex_lane::CodexLaneCommand::AccountLoginCancel(
                    codex_client::CancelLoginAccountParams { login_id },
                ))
            {
                state.codex_account.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_account.last_error = Some(error);
            }
            true
        }
        CodexAccountPaneAction::Logout => {
            state.codex_account.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_account.last_error = None;
            state.codex_account.last_action = Some("Queued account/logout".to_string());
            if let Err(error) =
                state.queue_codex_command(crate::codex_lane::CodexLaneCommand::AccountLogout)
            {
                state.codex_account.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_account.last_error = Some(error);
            }
            true
        }
        CodexAccountPaneAction::RateLimits => {
            state.codex_account.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_account.last_error = None;
            state.codex_account.last_action = Some("Queued account/rateLimits/read".to_string());
            if let Err(error) = state
                .queue_codex_command(crate::codex_lane::CodexLaneCommand::AccountRateLimitsRead)
            {
                state.codex_account.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_account.last_error = Some(error);
            }
            true
        }
    }
}

pub(super) fn run_codex_models_action(
    state: &mut crate::app_state::RenderState,
    action: CodexModelsPaneAction,
) -> bool {
    match action {
        CodexModelsPaneAction::Refresh => {
            state.codex_models.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_models.last_error = None;
            state.codex_models.last_action = Some(format!(
                "Queued model/list includeHidden={}",
                state.codex_models.include_hidden
            ));
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ModelList(ModelListParams {
                    cursor: None,
                    limit: Some(100),
                    include_hidden: Some(state.codex_models.include_hidden),
                }),
            ) {
                state.codex_models.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_models.last_error = Some(error);
            }
            true
        }
        CodexModelsPaneAction::ToggleHidden => {
            state.codex_models.include_hidden = !state.codex_models.include_hidden;
            run_codex_models_action(state, CodexModelsPaneAction::Refresh)
        }
    }
}

pub(super) fn run_codex_config_action(
    state: &mut crate::app_state::RenderState,
    action: CodexConfigPaneAction,
) -> bool {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    match action {
        CodexConfigPaneAction::Read => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some("Queued config/read".to_string());
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ConfigRead(ConfigReadParams {
                    include_layers: true,
                    cwd,
                }),
            ) {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
        CodexConfigPaneAction::Requirements => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some("Queued configRequirements/read".to_string());
            if let Err(error) = state
                .queue_codex_command(crate::codex_lane::CodexLaneCommand::ConfigRequirementsRead)
            {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
        CodexConfigPaneAction::WriteSample => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some("Queued config/value/write sample".to_string());
            let Some(model_override) = state.autopilot_chat.selected_model_override() else {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(
                    "Cannot write model sample until model/list resolves a concrete model"
                        .to_string(),
                );
                return true;
            };
            let value = serde_json::Value::String(model_override);
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ConfigValueWrite(ConfigValueWriteParams {
                    key_path: "model".to_string(),
                    value,
                    merge_strategy: MergeStrategy::Replace,
                    file_path: None,
                    expected_version: None,
                }),
            ) {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
        CodexConfigPaneAction::BatchWriteSample => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some("Queued config/batchWrite sample".to_string());
            let Some(model_override) = state.autopilot_chat.selected_model_override() else {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(
                    "Cannot write model sample until model/list resolves a concrete model"
                        .to_string(),
                );
                return true;
            };
            let reasoning = state
                .autopilot_chat
                .reasoning_effort
                .clone()
                .unwrap_or_else(|| "medium".to_string());
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ConfigBatchWrite(ConfigBatchWriteParams {
                    edits: vec![
                        ConfigEdit {
                            key_path: "model".to_string(),
                            value: serde_json::Value::String(model_override),
                            merge_strategy: MergeStrategy::Replace,
                        },
                        ConfigEdit {
                            key_path: "modelReasoningEffort".to_string(),
                            value: serde_json::Value::String(reasoning),
                            merge_strategy: MergeStrategy::Replace,
                        },
                    ],
                    file_path: None,
                    expected_version: None,
                }),
            ) {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
        CodexConfigPaneAction::DetectExternal => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some("Queued externalAgentConfig/detect".to_string());
            let cwds = std::env::current_dir().ok().map(|value| vec![value]);
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ExternalAgentConfigDetect(
                    ExternalAgentConfigDetectParams {
                        include_home: true,
                        cwds,
                    },
                ),
            ) {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
        CodexConfigPaneAction::ImportExternal => {
            state.codex_config.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_config.last_error = None;
            state.codex_config.last_action = Some(
                "Queued externalAgentConfig/import (migrationItems empty placeholder)".to_string(),
            );
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ExternalAgentConfigImport(
                    ExternalAgentConfigImportParams {
                        migration_items: Vec::new(),
                    },
                ),
            ) {
                state.codex_config.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_config.last_error = Some(error);
            }
            true
        }
    }
}

pub(super) fn run_codex_mcp_action(
    state: &mut crate::app_state::RenderState,
    action: CodexMcpPaneAction,
) -> bool {
    match action {
        CodexMcpPaneAction::Refresh => {
            state.codex_mcp.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_mcp.last_error = None;
            state.codex_mcp.last_action = Some("Queued mcpServerStatus/list".to_string());
            if let Err(error) =
                state.queue_codex_command(crate::codex_lane::CodexLaneCommand::McpServerStatusList(
                    ListMcpServerStatusParams {
                        cursor: None,
                        limit: Some(100),
                    },
                ))
            {
                state.codex_mcp.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_mcp.last_error = Some(error);
            }
            true
        }
        CodexMcpPaneAction::LoginSelected => {
            let selected_name = state
                .codex_mcp
                .selected_server_index
                .and_then(|idx| state.codex_mcp.servers.get(idx))
                .map(|entry| entry.name.clone());
            let Some(name) = selected_name else {
                state.codex_mcp.last_error =
                    Some("Select an MCP server before starting OAuth login".to_string());
                return true;
            };
            state.codex_mcp.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_mcp.last_error = None;
            state.codex_mcp.last_action = Some(format!("Queued mcpServer/oauth/login for {name}"));
            if let Err(error) =
                state.queue_codex_command(crate::codex_lane::CodexLaneCommand::McpServerOauthLogin(
                    McpServerOauthLoginParams {
                        name,
                        scopes: None,
                        timeout_secs: Some(180),
                    },
                ))
            {
                state.codex_mcp.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_mcp.last_error = Some(error);
            }
            true
        }
        CodexMcpPaneAction::Reload => {
            state.codex_mcp.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_mcp.last_error = None;
            state.codex_mcp.last_action = Some("Queued config/mcpServer/reload".to_string());
            if let Err(error) =
                state.queue_codex_command(crate::codex_lane::CodexLaneCommand::McpServerReload)
            {
                state.codex_mcp.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_mcp.last_error = Some(error);
            }
            true
        }
        CodexMcpPaneAction::SelectRow(index) => {
            if index < state.codex_mcp.servers.len() {
                state.codex_mcp.selected_server_index = Some(index);
                state.codex_mcp.last_action = Some(format!("Selected MCP row {}", index + 1));
                state.codex_mcp.last_error = None;
            }
            true
        }
    }
}

pub(super) fn run_codex_apps_action(
    state: &mut crate::app_state::RenderState,
    action: CodexAppsPaneAction,
) -> bool {
    match action {
        CodexAppsPaneAction::Refresh => {
            state.codex_apps.load_state = crate::app_state::PaneLoadState::Loading;
            state.codex_apps.last_error = None;
            state.codex_apps.last_action = Some("Queued app/list".to_string());
            if let Err(error) = state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::AppsList(AppsListParams {
                    cursor: None,
                    limit: Some(100),
                    thread_id: state.autopilot_chat.active_thread_id.clone(),
                    force_refetch: true,
                }),
            ) {
                state.codex_apps.load_state = crate::app_state::PaneLoadState::Error;
                state.codex_apps.last_error = Some(error);
            }
            true
        }
        CodexAppsPaneAction::SelectRow(index) => {
            if index < state.codex_apps.apps.len() {
                state.codex_apps.selected_app_index = Some(index);
                state.codex_apps.last_action = Some(format!("Selected app row {}", index + 1));
                state.codex_apps.last_error = None;
            }
            true
        }
    }
}

pub(super) fn run_codex_labs_action(
    state: &mut crate::app_state::RenderState,
    action: CodexLabsPaneAction,
) -> bool {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok());
    let queue = |state: &mut crate::app_state::RenderState,
                 action_label: &str,
                 command: crate::codex_lane::CodexLaneCommand| {
        state.codex_labs.load_state = crate::app_state::PaneLoadState::Loading;
        state.codex_labs.last_error = None;
        state.codex_labs.last_action = Some(action_label.to_string());
        if let Err(error) = state.queue_codex_command(command) {
            state.codex_labs.load_state = crate::app_state::PaneLoadState::Error;
            state.codex_labs.last_error = Some(error);
        }
        true
    };

    match action {
        CodexLabsPaneAction::ReviewInline | CodexLabsPaneAction::ReviewDetached => {
            let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
                state.codex_labs.last_error =
                    Some("Select an active thread before starting review".to_string());
                return true;
            };
            let delivery = match action {
                CodexLabsPaneAction::ReviewInline => ReviewDelivery::Inline,
                CodexLabsPaneAction::ReviewDetached => ReviewDelivery::Detached,
                _ => ReviewDelivery::Inline,
            };
            queue(
                state,
                &format!("Queued review/start ({:?})", delivery),
                crate::codex_lane::CodexLaneCommand::ReviewStart(ReviewStartParams {
                    thread_id,
                    target: ReviewTarget::UncommittedChanges,
                    delivery: Some(delivery),
                }),
            )
        }
        CodexLabsPaneAction::CommandExec => queue(
            state,
            "Queued command/exec",
            crate::codex_lane::CodexLaneCommand::CommandExec(CommandExecParams {
                command: vec!["pwd".to_string()],
                timeout_ms: Some(5000),
                cwd,
                sandbox_policy: None,
            }),
        ),
        CodexLabsPaneAction::CollaborationModes => queue(
            state,
            "Queued collaborationMode/list",
            crate::codex_lane::CodexLaneCommand::CollaborationModeList(
                CollaborationModeListParams::default(),
            ),
        ),
        CodexLabsPaneAction::ExperimentalFeatures => queue(
            state,
            "Queued experimentalFeature/list",
            crate::codex_lane::CodexLaneCommand::ExperimentalFeatureList(
                ExperimentalFeatureListParams {
                    cursor: None,
                    limit: Some(100),
                },
            ),
        ),
        CodexLabsPaneAction::ToggleExperimental => {
            state.codex_labs.experimental_enabled = !state.codex_labs.experimental_enabled;
            state.codex_labs.last_error = None;
            state.codex_labs.last_action = Some(format!(
                "Experimental gating set to {}",
                state.codex_labs.experimental_enabled
            ));
            true
        }
        CodexLabsPaneAction::RealtimeStart => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before realtime controls".to_string());
                return true;
            }
            let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
                state.codex_labs.last_error =
                    Some("Select an active thread before realtime start".to_string());
                return true;
            };
            queue(
                state,
                "Queued thread/realtime/start",
                crate::codex_lane::CodexLaneCommand::ThreadRealtimeStart(
                    ThreadRealtimeStartParams {
                        thread_id,
                        prompt: "Start realtime session".to_string(),
                        session_id: Some(format!("labs-{}", std::process::id())),
                    },
                ),
            )
        }
        CodexLabsPaneAction::RealtimeAppendText => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before realtime controls".to_string());
                return true;
            }
            let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
                state.codex_labs.last_error =
                    Some("Select an active thread before realtime append".to_string());
                return true;
            };
            queue(
                state,
                "Queued thread/realtime/appendText",
                crate::codex_lane::CodexLaneCommand::ThreadRealtimeAppendText(
                    ThreadRealtimeAppendTextParams {
                        thread_id,
                        text: "ping from Codex Labs".to_string(),
                    },
                ),
            )
        }
        CodexLabsPaneAction::RealtimeStop => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before realtime controls".to_string());
                return true;
            }
            let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
                state.codex_labs.last_error =
                    Some("Select an active thread before realtime stop".to_string());
                return true;
            };
            queue(
                state,
                "Queued thread/realtime/stop",
                crate::codex_lane::CodexLaneCommand::ThreadRealtimeStop(ThreadRealtimeStopParams {
                    thread_id,
                }),
            )
        }
        CodexLabsPaneAction::WindowsSandboxSetup => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before sandbox setup".to_string());
                return true;
            }
            if !cfg!(target_os = "windows") {
                state.codex_labs.last_error =
                    Some("windowsSandbox/setupStart is only available on Windows".to_string());
                return true;
            }
            queue(
                state,
                "Queued windowsSandbox/setupStart",
                crate::codex_lane::CodexLaneCommand::WindowsSandboxSetupStart(
                    WindowsSandboxSetupStartParams {
                        mode: "enable".to_string(),
                    },
                ),
            )
        }
        CodexLabsPaneAction::FuzzyStart => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before fuzzy session controls".to_string());
                return true;
            }
            let roots = vec![cwd.unwrap_or_else(|| ".".to_string())];
            queue(
                state,
                "Queued fuzzyFileSearch/sessionStart",
                crate::codex_lane::CodexLaneCommand::FuzzyFileSearchSessionStart(
                    FuzzyFileSearchSessionStartParams {
                        session_id: state.codex_labs.fuzzy_session_id.clone(),
                        roots,
                    },
                ),
            )
        }
        CodexLabsPaneAction::FuzzyUpdate => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before fuzzy session controls".to_string());
                return true;
            }
            queue(
                state,
                "Queued fuzzyFileSearch/sessionUpdate",
                crate::codex_lane::CodexLaneCommand::FuzzyFileSearchSessionUpdate(
                    FuzzyFileSearchSessionUpdateParams {
                        session_id: state.codex_labs.fuzzy_session_id.clone(),
                        query: "codex integration".to_string(),
                    },
                ),
            )
        }
        CodexLabsPaneAction::FuzzyStop => {
            if !state.codex_labs.experimental_enabled {
                state.codex_labs.last_error =
                    Some("Enable experimental gating before fuzzy session controls".to_string());
                return true;
            }
            queue(
                state,
                "Queued fuzzyFileSearch/sessionStop",
                crate::codex_lane::CodexLaneCommand::FuzzyFileSearchSessionStop(
                    FuzzyFileSearchSessionStopParams {
                        session_id: state.codex_labs.fuzzy_session_id.clone(),
                    },
                ),
            )
        }
    }
}

pub(super) fn run_codex_diagnostics_action(
    state: &mut crate::app_state::RenderState,
    action: CodexDiagnosticsPaneAction,
) -> bool {
    match action {
        CodexDiagnosticsPaneAction::EnableWireLog => {
            let configured_path = if state.codex_diagnostics.wire_log_path.trim().is_empty() {
                "/tmp/openagents-codex-wire.log".to_string()
            } else {
                state.codex_diagnostics.wire_log_path.trim().to_string()
            };
            state.codex_diagnostics.wire_log_path = configured_path.clone();
            state.codex_diagnostics.wire_log_enabled = true;
            state.codex_diagnostics.last_error = None;
            state.codex_diagnostics.last_action = Some(format!(
                "Restarting Codex lane with wire log {}",
                configured_path
            ));
            state.codex_lane_config.wire_log_path = Some(std::path::PathBuf::from(configured_path));
            state.restart_codex_lane();
            true
        }
        CodexDiagnosticsPaneAction::DisableWireLog => {
            state.codex_diagnostics.wire_log_enabled = false;
            state.codex_diagnostics.last_error = None;
            state.codex_diagnostics.last_action =
                Some("Restarting Codex lane with wire log disabled".to_string());
            state.codex_lane_config.wire_log_path = None;
            state.restart_codex_lane();
            true
        }
        CodexDiagnosticsPaneAction::ClearEvents => {
            state.codex_diagnostics.notification_counts.clear();
            state.codex_diagnostics.server_request_counts.clear();
            state.codex_diagnostics.raw_events.clear();
            state.codex_diagnostics.last_command_failure = None;
            state.codex_diagnostics.last_snapshot_error = None;
            state.codex_diagnostics.last_error = None;
            state.codex_diagnostics.last_action =
                Some("Cleared diagnostics event cache".to_string());
            true
        }
    }
}

pub(super) fn run_earnings_scoreboard_action(
    state: &mut crate::app_state::RenderState,
    action: EarningsScoreboardPaneAction,
) -> bool {
    match action {
        EarningsScoreboardPaneAction::Refresh => {
            refresh_earnings_scoreboard(state, std::time::Instant::now());
            true
        }
    }
}

pub(super) fn run_agent_network_simulation_action(
    state: &mut crate::app_state::RenderState,
    action: AgentNetworkSimulationPaneAction,
) -> bool {
    match action {
        AgentNetworkSimulationPaneAction::RunRound => {
            if state.agent_network_simulation.auto_run_enabled {
                state.agent_network_simulation.stop_auto_run();
                state.provider_runtime.last_result =
                    state.agent_network_simulation.last_action.clone();
                return true;
            }
            let now = std::time::Instant::now();
            state.agent_network_simulation.start_auto_run(now);
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let ran_round = run_agent_network_simulation_round(state, now_epoch_seconds);
            state.agent_network_simulation.mark_auto_round(now);
            ran_round
        }
        AgentNetworkSimulationPaneAction::Reset => {
            state.agent_network_simulation.reset();
            state.provider_runtime.last_result = state.agent_network_simulation.last_action.clone();
            true
        }
    }
}

pub(super) fn run_agent_network_simulation_round(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
) -> bool {
    match state.agent_network_simulation.run_round(now_epoch_seconds) {
        Ok(()) => {
            state.provider_runtime.last_result = state.agent_network_simulation.last_action.clone();
            let event_id = format!("sim:round:{}", state.agent_network_simulation.rounds_run);
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id,
                    domain: crate::app_state::ActivityEventDomain::Sa,
                    source_tag: "simulation.nip28".to_string(),
                    summary: "Sovereign agents ran SA/SKL/AC simulation round".to_string(),
                    detail: format!(
                        "round={} transferred_sats={} learned_skills={}",
                        state.agent_network_simulation.rounds_run,
                        state.agent_network_simulation.total_transferred_sats,
                        state.agent_network_simulation.learned_skills.join(", ")
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
        }
        Err(error) => {
            state.agent_network_simulation.last_error = Some(error);
            state.agent_network_simulation.load_state = crate::app_state::PaneLoadState::Error;
        }
    }
    true
}

pub(super) fn run_auto_agent_network_simulation(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if !state.agent_network_simulation.should_run_auto_round(now) {
        return false;
    }
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let changed = run_agent_network_simulation_round(state, now_epoch_seconds);
    state.agent_network_simulation.mark_auto_round(now);
    changed
}

pub(super) fn run_treasury_exchange_simulation_action(
    state: &mut crate::app_state::RenderState,
    action: TreasuryExchangeSimulationPaneAction,
) -> bool {
    match action {
        TreasuryExchangeSimulationPaneAction::RunRound => {
            if state.treasury_exchange_simulation.auto_run_enabled {
                state.treasury_exchange_simulation.stop_auto_run();
                state.provider_runtime.last_result =
                    state.treasury_exchange_simulation.last_action.clone();
                return true;
            }
            let now = std::time::Instant::now();
            state.treasury_exchange_simulation.start_auto_run(now);
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let ran_round = run_treasury_exchange_simulation_round(state, now_epoch_seconds);
            state.treasury_exchange_simulation.mark_auto_round(now);
            ran_round
        }
        TreasuryExchangeSimulationPaneAction::Reset => {
            state.treasury_exchange_simulation.reset();
            state.provider_runtime.last_result =
                state.treasury_exchange_simulation.last_action.clone();
            true
        }
    }
}

pub(super) fn run_treasury_exchange_simulation_round(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
) -> bool {
    match state
        .treasury_exchange_simulation
        .run_round(now_epoch_seconds)
    {
        Ok(()) => {
            state.provider_runtime.last_result =
                state.treasury_exchange_simulation.last_action.clone();
            let event_id = format!(
                "sim:treasury:round:{}",
                state.treasury_exchange_simulation.rounds_run
            );
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id,
                    domain: crate::app_state::ActivityEventDomain::Network,
                    source_tag: "simulation.nip69".to_string(),
                    summary: "Treasury + exchange simulation round executed".to_string(),
                    detail: format!(
                        "round={} volume_sats={} liquidity_sats={}",
                        state.treasury_exchange_simulation.rounds_run,
                        state.treasury_exchange_simulation.trade_volume_sats,
                        state.treasury_exchange_simulation.total_liquidity_sats
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
        }
        Err(error) => {
            state.treasury_exchange_simulation.last_error = Some(error);
            state.treasury_exchange_simulation.load_state = crate::app_state::PaneLoadState::Error;
        }
    }
    true
}

pub(super) fn run_auto_treasury_exchange_simulation(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if !state
        .treasury_exchange_simulation
        .should_run_auto_round(now)
    {
        return false;
    }
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let changed = run_treasury_exchange_simulation_round(state, now_epoch_seconds);
    state.treasury_exchange_simulation.mark_auto_round(now);
    changed
}

pub(super) fn run_relay_security_simulation_action(
    state: &mut crate::app_state::RenderState,
    action: RelaySecuritySimulationPaneAction,
) -> bool {
    match action {
        RelaySecuritySimulationPaneAction::RunRound => {
            if state.relay_security_simulation.auto_run_enabled {
                state.relay_security_simulation.stop_auto_run();
                state.provider_runtime.last_result =
                    state.relay_security_simulation.last_action.clone();
                return true;
            }
            let now = std::time::Instant::now();
            state.relay_security_simulation.start_auto_run(now);
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let ran_round = run_relay_security_simulation_round(state, now_epoch_seconds);
            state.relay_security_simulation.mark_auto_round(now);
            ran_round
        }
        RelaySecuritySimulationPaneAction::Reset => {
            state.relay_security_simulation.reset();
            state.provider_runtime.last_result =
                state.relay_security_simulation.last_action.clone();
            true
        }
    }
}

pub(super) fn run_relay_security_simulation_round(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
) -> bool {
    match state.relay_security_simulation.run_round(now_epoch_seconds) {
        Ok(()) => {
            state.provider_runtime.last_result =
                state.relay_security_simulation.last_action.clone();
            let event_id = format!(
                "sim:relay-security:round:{}",
                state.relay_security_simulation.rounds_run
            );
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id,
                    domain: crate::app_state::ActivityEventDomain::Sync,
                    source_tag: "simulation.nip42".to_string(),
                    summary: "Relay security simulation round executed".to_string(),
                    detail: format!(
                        "round={} auth={} sync_ranges={}",
                        state.relay_security_simulation.rounds_run,
                        state
                            .relay_security_simulation
                            .auth_event_id
                            .as_deref()
                            .unwrap_or("n/a"),
                        state.relay_security_simulation.sync_ranges
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
        }
        Err(error) => {
            state.relay_security_simulation.last_error = Some(error);
            state.relay_security_simulation.load_state = crate::app_state::PaneLoadState::Error;
        }
    }
    true
}

pub(super) fn run_auto_relay_security_simulation(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if !state.relay_security_simulation.should_run_auto_round(now) {
        return false;
    }
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let changed = run_relay_security_simulation_round(state, now_epoch_seconds);
    state.relay_security_simulation.mark_auto_round(now);
    changed
}

pub(super) fn run_stable_sats_simulation_action(
    state: &mut crate::app_state::RenderState,
    action: StableSatsSimulationPaneAction,
) -> bool {
    match action {
        StableSatsSimulationPaneAction::RunRound => {
            if state.stable_sats_simulation.auto_run_enabled {
                state.stable_sats_simulation.stop_auto_run();
                state.provider_runtime.last_result =
                    state.stable_sats_simulation.last_action.clone();
                return true;
            }
            let now = std::time::Instant::now();
            state.stable_sats_simulation.start_auto_run(now);
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let ran_round = run_stable_sats_simulation_round(state, now_epoch_seconds);
            state.stable_sats_simulation.mark_auto_round(now);
            ran_round
        }
        StableSatsSimulationPaneAction::Reset => {
            state.stable_sats_simulation.reset();
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            true
        }
    }
}

pub(super) fn run_stable_sats_simulation_round(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
) -> bool {
    match state.stable_sats_simulation.run_round(now_epoch_seconds) {
        Ok(()) => {
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            let event_id = format!(
                "sim:stablesats:round:{}",
                state.stable_sats_simulation.rounds_run
            );
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id,
                    domain: crate::app_state::ActivityEventDomain::Wallet,
                    source_tag: "simulation.blink".to_string(),
                    summary: "StableSats BTC/USD switching round executed".to_string(),
                    detail: format!(
                        "round={} quote={} converted_sats={} converted_usd_cents={}",
                        state.stable_sats_simulation.rounds_run,
                        state.stable_sats_simulation.price_usd_cents_per_btc,
                        state.stable_sats_simulation.total_converted_sats,
                        state.stable_sats_simulation.total_converted_usd_cents
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
        }
        Err(error) => {
            state.stable_sats_simulation.last_error = Some(error);
            state.stable_sats_simulation.load_state = crate::app_state::PaneLoadState::Error;
        }
    }
    true
}

pub(super) fn run_auto_stable_sats_simulation(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if !state.stable_sats_simulation.should_run_auto_round(now) {
        return false;
    }
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let changed = run_stable_sats_simulation_round(state, now_epoch_seconds);
    state.stable_sats_simulation.mark_auto_round(now);
    changed
}

pub(super) fn run_relay_connections_action(
    state: &mut crate::app_state::RenderState,
    action: RelayConnectionsPaneAction,
) -> bool {
    match action {
        RelayConnectionsPaneAction::SelectRow(index) => {
            if !state.relay_connections.select_by_index(index) {
                state.relay_connections.last_error = Some("Relay row out of range".to_string());
                state.relay_connections.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.relay_connections.load_state = crate::app_state::PaneLoadState::Ready;
            }
        }
        RelayConnectionsPaneAction::AddRelay => {
            let relay_url = state.relay_connections_inputs.relay_url.get_value();
            match state.relay_connections.add_relay(relay_url) {
                Ok(()) => {
                    state.provider_runtime.last_result =
                        state.relay_connections.last_action.clone();
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
        RelayConnectionsPaneAction::RemoveSelected => {
            match state.relay_connections.remove_selected() {
                Ok(url) => {
                    state.provider_runtime.last_result = Some(format!("removed relay {url}"));
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
        RelayConnectionsPaneAction::RetrySelected => {
            match state.relay_connections.retry_selected() {
                Ok(url) => {
                    state.provider_runtime.last_result = Some(format!("retried relay {url}"));
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
    }

    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    refresh_sync_health(state);
    true
}

pub(super) fn run_sync_health_action(
    state: &mut crate::app_state::RenderState,
    action: SyncHealthPaneAction,
) -> bool {
    match action {
        SyncHealthPaneAction::Rebootstrap => {
            state.sync_health.rebootstrap();
            state.provider_runtime.last_result = state.sync_health.last_action.clone();
            refresh_sync_health(state);
            true
        }
    }
}

pub(super) fn run_network_requests_action(
    state: &mut crate::app_state::RenderState,
    action: NetworkRequestsPaneAction,
) -> bool {
    match action {
        NetworkRequestsPaneAction::SubmitRequest => {
            let request_type = state
                .network_requests_inputs
                .request_type
                .get_value()
                .trim()
                .to_string();
            let payload = state
                .network_requests_inputs
                .payload
                .get_value()
                .trim()
                .to_string();
            let skill_scope_id =
                normalize_optional_text(state.network_requests_inputs.skill_scope_id.get_value());
            let credit_envelope_ref = normalize_optional_text(
                state
                    .network_requests_inputs
                    .credit_envelope_ref
                    .get_value(),
            );
            let budget_sats = match parse_positive_amount_str(
                state.network_requests_inputs.budget_sats.get_value(),
                "Budget sats",
            ) {
                Ok(value) => value,
                Err(error) => {
                    state.network_requests.last_error = Some(error);
                    state.network_requests.load_state = crate::app_state::PaneLoadState::Error;
                    return true;
                }
            };
            let timeout_seconds = match parse_positive_amount_str(
                state.network_requests_inputs.timeout_seconds.get_value(),
                "Timeout seconds",
            ) {
                Ok(value) => value,
                Err(error) => {
                    state.network_requests.last_error = Some(error);
                    state.network_requests.load_state = crate::app_state::PaneLoadState::Error;
                    return true;
                }
            };

            let scope = if let Some(skill_scope) = skill_scope_id.as_deref() {
                format!("skill:{skill_scope}:constraints")
            } else {
                format!("network:{request_type}:{budget_sats}")
            };
            let queue_result = state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                scope,
                request_type: request_type.clone(),
                payload: payload.clone(),
                skill_scope_id: skill_scope_id.clone(),
                credit_envelope_ref: credit_envelope_ref.clone(),
                requested_sats: budget_sats,
                timeout_seconds,
            });
            match queue_result {
                Ok(command_seq) => {
                    let inbox_request_type = request_type.clone();
                    let inbox_payload = payload.clone();
                    let inbox_skill_scope_id = skill_scope_id.clone();
                    let inbox_credit_envelope_ref = credit_envelope_ref.clone();
                    match state.network_requests.queue_request_submission(
                        NetworkRequestSubmission {
                            request_type,
                            payload,
                            skill_scope_id,
                            credit_envelope_ref,
                            budget_sats,
                            timeout_seconds,
                            authority_command_seq: command_seq,
                        },
                    ) {
                        Ok(request_id) => {
                            state.provider_runtime.last_result = Some(format!(
                                "Queued network request {request_id} -> AC cmd#{command_seq}"
                            ));
                            let validation =
                                if inbox_payload.to_ascii_lowercase().contains("invalid") {
                                    JobInboxValidation::Invalid(
                                        "payload contains reserved invalid marker".to_string(),
                                    )
                                } else if inbox_payload.len() < 12 {
                                    JobInboxValidation::Pending
                                } else {
                                    JobInboxValidation::Valid
                                };
                            state
                                .job_inbox
                                .upsert_network_request(JobInboxNetworkRequest {
                                    request_id: request_id.clone(),
                                    requester: "network-buyer".to_string(),
                                    capability: inbox_request_type,
                                    skill_scope_id: inbox_skill_scope_id,
                                    skl_manifest_a: state.skl_lane.manifest_a.clone(),
                                    skl_manifest_event_id: state.skl_lane.manifest_event_id.clone(),
                                    sa_tick_request_event_id: state
                                        .sa_lane
                                        .last_tick_request_event_id
                                        .clone(),
                                    sa_tick_result_event_id: state
                                        .sa_lane
                                        .last_tick_result_event_id
                                        .clone(),
                                    ac_envelope_event_id: inbox_credit_envelope_ref,
                                    price_sats: budget_sats,
                                    ttl_seconds: timeout_seconds,
                                    validation,
                                });
                            state.sync_health.last_applied_event_seq =
                                state.sync_health.last_applied_event_seq.saturating_add(1);
                            state.sync_health.cursor_last_advanced_seconds_ago = 0;
                            refresh_sync_health(state);
                        }
                        Err(error) => {
                            state.network_requests.last_error = Some(error);
                        }
                    }
                }
                Err(error) => {
                    state.network_requests.last_error = Some(error.clone());
                    state.network_requests.mark_authority_enqueue_failure(
                        state.next_runtime_command_seq.saturating_sub(1),
                        RuntimeCommandErrorClass::Transport.label(),
                        &error,
                    );
                }
            }
            true
        }
    }
}

pub(super) fn run_starter_jobs_action(
    state: &mut crate::app_state::RenderState,
    action: StarterJobsPaneAction,
) -> bool {
    match action {
        StarterJobsPaneAction::SelectRow(index) => {
            if !state.starter_jobs.select_by_index(index) {
                state.starter_jobs.last_error = Some("Starter job row out of range".to_string());
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        StarterJobsPaneAction::CompleteSelected => {
            match state.starter_jobs.complete_selected() {
                Ok((job_id, payout_sats, payout_pointer)) => {
                    state.spark_wallet.last_payment_id = Some(payout_pointer.clone());
                    state.spark_wallet.last_action = Some(format!(
                        "Starter payout settled for {job_id} ({payout_sats} sats)"
                    ));
                    state.provider_runtime.last_result =
                        Some(format!("completed starter job {job_id}"));
                    state
                        .job_history
                        .upsert_row(crate::app_state::JobHistoryReceiptRow {
                            job_id,
                            status: crate::app_state::JobHistoryStatus::Succeeded,
                            completed_at_epoch_seconds: state
                                .job_history
                                .reference_epoch_seconds
                                .saturating_add(state.job_history.rows.len() as u64 * 19),
                            skill_scope_id: state
                                .network_requests
                                .submitted
                                .first()
                                .and_then(|request| request.skill_scope_id.clone()),
                            skl_manifest_a: state.skl_lane.manifest_a.clone(),
                            skl_manifest_event_id: state.skl_lane.manifest_event_id.clone(),
                            sa_tick_result_event_id: state
                                .sa_lane
                                .last_tick_result_event_id
                                .clone(),
                            sa_trajectory_session_id: Some("traj:starter-job".to_string()),
                            ac_envelope_event_id: state.ac_lane.envelope_event_id.clone(),
                            ac_settlement_event_id: state.ac_lane.settlement_event_id.clone(),
                            ac_default_event_id: None,
                            payout_sats,
                            result_hash: "sha256:starter-job".to_string(),
                            payment_pointer: payout_pointer,
                            failure_reason: None,
                        });
                    refresh_earnings_scoreboard(state, std::time::Instant::now());
                }
                Err(error) => {
                    state.starter_jobs.last_error = Some(error);
                }
            }
            true
        }
    }
}

pub(super) fn run_activity_feed_action(
    state: &mut crate::app_state::RenderState,
    action: ActivityFeedPaneAction,
) -> bool {
    match action {
        ActivityFeedPaneAction::Refresh => {
            let rows = build_activity_feed_snapshot_events(state);
            state.activity_feed.record_refresh(rows);
            true
        }
        ActivityFeedPaneAction::SetFilter(filter) => {
            state.activity_feed.set_filter(filter);
            true
        }
        ActivityFeedPaneAction::SelectRow(index) => {
            if !state.activity_feed.select_visible_row(index) {
                state.activity_feed.last_error = Some("Activity row out of range".to_string());
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
    }
}

pub(super) fn run_alerts_recovery_action(
    state: &mut crate::app_state::RenderState,
    action: AlertsRecoveryPaneAction,
) -> bool {
    match action {
        AlertsRecoveryPaneAction::SelectRow(index) => {
            if !state.alerts_recovery.select_by_index(index) {
                state.alerts_recovery.last_error = Some("Alert row out of range".to_string());
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        AlertsRecoveryPaneAction::AcknowledgeSelected => {
            match state.alerts_recovery.acknowledge_selected() {
                Ok(alert_id) => {
                    state.provider_runtime.last_result = Some(format!("acknowledged {alert_id}"));
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                }
            }
            true
        }
        AlertsRecoveryPaneAction::ResolveSelected => {
            match state.alerts_recovery.resolve_selected() {
                Ok(alert_id) => {
                    state.provider_runtime.last_result = Some(format!("resolved {alert_id}"));
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                }
            }
            true
        }
        AlertsRecoveryPaneAction::RecoverSelected => {
            let Some(domain) = state.alerts_recovery.selected_domain() else {
                state.alerts_recovery.last_error = Some("Select an alert first".to_string());
                state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            };

            let recovery = match domain {
                AlertDomain::Identity => match regenerate_identity() {
                    Ok(identity) => {
                        state.nostr_identity = Some(identity);
                        state.nostr_identity_error = None;
                        state.nostr_secret_state.revealed_until = None;
                        state.nostr_secret_state.set_copy_notice(
                            std::time::Instant::now(),
                            "Identity regenerated. Secrets are hidden by default.".to_string(),
                        );
                        queue_spark_command(state, SparkWalletCommand::Refresh);
                        Ok("Identity lane recovered".to_string())
                    }
                    Err(error) => Err(format!("Identity recovery failed: {error}")),
                },
                AlertDomain::Wallet => {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                    Ok("Wallet refresh queued".to_string())
                }
                AlertDomain::Relays => {
                    if state.relay_connections.selected_url.is_none() {
                        state.relay_connections.selected_url = state
                            .relay_connections
                            .relays
                            .first()
                            .map(|row| row.url.clone());
                    }
                    match state.relay_connections.retry_selected() {
                        Ok(url) => Ok(format!("Relay reconnect attempted for {url}")),
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::ProviderRuntime => {
                    let wants_online = matches!(
                        state.provider_runtime.mode,
                        ProviderMode::Offline | ProviderMode::Degraded
                    );
                    match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline {
                        online: wants_online,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued SA runner recovery command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::Sync => {
                    state.sync_health.rebootstrap();
                    Ok("Sync rebootstrap started".to_string())
                }
                AlertDomain::SkillTrust => {
                    match state.queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch {
                        query: "trust.recovery".to_string(),
                        limit: 8,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued SKL trust refresh command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
                AlertDomain::Credit => {
                    match state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                        scope: "credit:recovery".to_string(),
                        request_type: "credit.recovery".to_string(),
                        payload: "{\"recovery\":true}".to_string(),
                        skill_scope_id: None,
                        credit_envelope_ref: state.ac_lane.envelope_event_id.clone(),
                        requested_sats: 1200,
                        timeout_seconds: 60,
                    }) {
                        Ok(command_seq) => {
                            Ok(format!("Queued AC credit refresh command #{command_seq}"))
                        }
                        Err(error) => Err(error),
                    }
                }
            };

            match recovery {
                Ok(result) => {
                    if let Err(error) = state.alerts_recovery.resolve_selected() {
                        state.alerts_recovery.last_error = Some(error);
                        state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                    } else {
                        state.alerts_recovery.last_action = Some(result.clone());
                        state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
                        state.provider_runtime.last_result = Some(result);
                    }
                }
                Err(error) => {
                    state.alerts_recovery.last_error = Some(error);
                    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Error;
                }
            }

            refresh_sync_health(state);
            true
        }
    }
}

pub(super) fn run_settings_action(
    state: &mut crate::app_state::RenderState,
    action: SettingsPaneAction,
) -> bool {
    match action {
        SettingsPaneAction::Save => {
            let relay_url = state.settings_inputs.relay_url.get_value().to_string();
            let wallet_default_send_sats = state
                .settings_inputs
                .wallet_default_send_sats
                .get_value()
                .to_string();
            let provider_max_queue_depth = state
                .settings_inputs
                .provider_max_queue_depth
                .get_value()
                .to_string();
            match state.settings.apply_updates(
                &relay_url,
                &wallet_default_send_sats,
                &provider_max_queue_depth,
            ) {
                Ok(()) => {
                    state.settings_inputs.sync_from_state(&state.settings);
                    state
                        .relay_connections_inputs
                        .relay_url
                        .set_value(state.settings.document.relay_url.clone());
                    state
                        .spark_inputs
                        .send_amount
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state
                        .pay_invoice_inputs
                        .amount_sats
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state.provider_runtime.last_result = state.settings.last_action.clone();
                    if state.settings.document.reconnect_required {
                        state.sync_health.subscription_state = "resubscribing".to_string();
                        state.sync_health.last_action = Some(
                            "Settings changed connectivity lanes; reconnect required".to_string(),
                        );
                    }
                }
                Err(error) => {
                    state.settings.last_error = Some(error);
                }
            }
            true
        }
        SettingsPaneAction::ResetDefaults => {
            match state.settings.reset_defaults() {
                Ok(()) => {
                    state.settings_inputs.sync_from_state(&state.settings);
                    state
                        .relay_connections_inputs
                        .relay_url
                        .set_value(state.settings.document.relay_url.clone());
                    state
                        .spark_inputs
                        .send_amount
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state
                        .pay_invoice_inputs
                        .amount_sats
                        .set_value(state.settings.document.wallet_default_send_sats.to_string());
                    state.provider_runtime.last_result = state.settings.last_action.clone();
                }
                Err(error) => {
                    state.settings.last_error = Some(error);
                }
            }
            true
        }
    }
}

pub(super) fn run_credentials_action(
    state: &mut crate::app_state::RenderState,
    action: CredentialsPaneAction,
) -> bool {
    let mut restart_codex = false;
    let mut sync_runtime = false;
    let result = match action {
        CredentialsPaneAction::AddCustom => {
            sync_runtime = true;
            state
                .credentials
                .add_custom_entry(state.credentials_inputs.variable_name.get_value())
        }
        CredentialsPaneAction::SaveValue => {
            let value = state
                .credentials_inputs
                .variable_value
                .get_value()
                .to_string();
            tracing::info!(
                "credentials/save value requested selected={} chars={}",
                state.credentials.selected_name.as_deref().unwrap_or("none"),
                value.chars().count()
            );
            let saved = state.credentials.set_selected_value(value.as_str());
            if saved.is_ok() {
                sync_runtime = true;
                state
                    .credentials_inputs
                    .variable_value
                    .set_value(String::new());
                restart_codex = true;
                tracing::info!("credentials/save value stored; syncing runtime");
            }
            saved
        }
        CredentialsPaneAction::DeleteOrClear => {
            sync_runtime = true;
            restart_codex = true;
            state.credentials.delete_or_clear_selected()
        }
        CredentialsPaneAction::ToggleEnabled => {
            sync_runtime = true;
            restart_codex = true;
            state.credentials.toggle_selected_enabled()
        }
        CredentialsPaneAction::ToggleScopeCodex => {
            sync_runtime = true;
            restart_codex = true;
            state
                .credentials
                .toggle_selected_scope(crate::credentials::CREDENTIAL_SCOPE_CODEX)
        }
        CredentialsPaneAction::ToggleScopeSpark => {
            sync_runtime = true;
            restart_codex = true;
            state
                .credentials
                .toggle_selected_scope(crate::credentials::CREDENTIAL_SCOPE_SPARK)
        }
        CredentialsPaneAction::ToggleScopeSkills => {
            sync_runtime = true;
            restart_codex = true;
            state
                .credentials
                .toggle_selected_scope(crate::credentials::CREDENTIAL_SCOPE_SKILLS)
        }
        CredentialsPaneAction::ToggleScopeGlobal => {
            sync_runtime = true;
            restart_codex = true;
            state
                .credentials
                .toggle_selected_scope(crate::credentials::CREDENTIAL_SCOPE_GLOBAL)
        }
        CredentialsPaneAction::ImportFromEnv => {
            sync_runtime = true;
            restart_codex = true;
            state.credentials.import_from_process_env().map(|_| ())
        }
        CredentialsPaneAction::Reload => {
            sync_runtime = true;
            restart_codex = true;
            state.credentials = crate::app_state::CredentialsState::load_from_disk();
            Ok(())
        }
        CredentialsPaneAction::SelectRow(row_index) => state.credentials.select_row(row_index),
    };

    match result {
        Ok(()) => {
            state.credentials.sync_inputs(&mut state.credentials_inputs);
            if sync_runtime {
                state.sync_credentials_runtime(restart_codex);
                tracing::info!(
                    "credentials/runtime sync complete restart_codex={}",
                    restart_codex
                );
            }
        }
        Err(error) => {
            tracing::info!("credentials pane action failed: {error}");
            state.credentials.last_error = Some(error);
            state.credentials.load_state = crate::app_state::PaneLoadState::Error;
        }
    }

    true
}

pub(super) fn build_activity_feed_snapshot_events(
    state: &crate::app_state::RenderState,
) -> Vec<ActivityEventRow> {
    let now_epoch = state
        .job_history
        .reference_epoch_seconds
        .saturating_add(state.job_history.rows.len() as u64 * 23);
    let mut rows = Vec::new();

    for message in state.autopilot_chat.messages.iter().rev().take(6) {
        let role = match message.role {
            crate::app_state::AutopilotRole::User => "user",
            crate::app_state::AutopilotRole::Codex => "codex",
        };
        let status = match message.status {
            crate::app_state::AutopilotMessageStatus::Queued => "queued",
            crate::app_state::AutopilotMessageStatus::Running => "running",
            crate::app_state::AutopilotMessageStatus::Done => "done",
            crate::app_state::AutopilotMessageStatus::Error => "error",
        };
        rows.push(ActivityEventRow {
            event_id: format!("chat:msg:{}", message.id),
            domain: ActivityEventDomain::Chat,
            source_tag: ActivityEventDomain::Chat.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(message.id),
            summary: format!("{role} message {status}"),
            detail: message.content.clone(),
        });
    }

    for (index, event) in state.cad_demo.cad_events.iter().rev().take(16).enumerate() {
        rows.push(ActivityEventRow {
            event_id: event.event_id.clone(),
            domain: ActivityEventDomain::Cad,
            source_tag: format!("cad.{}", event.kind.as_str()),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(8 + index as u64),
            summary: event.summary.clone(),
            detail: format!(
                "doc={} rev={} variant={} {}",
                event.document_id,
                event.document_revision,
                event.variant_id.as_deref().unwrap_or("none"),
                event.detail
            ),
        });
    }

    for receipt in state.job_history.rows.iter().take(6) {
        rows.push(ActivityEventRow {
            event_id: format!("job:receipt:{}", receipt.job_id),
            domain: ActivityEventDomain::Job,
            source_tag: ActivityEventDomain::Job.source_tag().to_string(),
            occurred_at_epoch_seconds: receipt.completed_at_epoch_seconds,
            summary: format!(
                "{} {} sats {}",
                receipt.job_id,
                receipt.payout_sats,
                receipt.status.label()
            ),
            detail: receipt.payment_pointer.clone(),
        });
    }

    if let Some(last_payment_id) = state.spark_wallet.last_payment_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("wallet:payment:{last_payment_id}"),
            domain: ActivityEventDomain::Wallet,
            source_tag: ActivityEventDomain::Wallet.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch,
            summary: "Spark payment pointer updated".to_string(),
            detail: last_payment_id.to_string(),
        });
    }
    if let Some(wallet_action) = state.spark_wallet.last_action.as_deref() {
        rows.push(ActivityEventRow {
            event_id: "wallet:last_action".to_string(),
            domain: ActivityEventDomain::Wallet,
            source_tag: ActivityEventDomain::Wallet.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(1),
            summary: "Wallet activity".to_string(),
            detail: wallet_action.to_string(),
        });
    }

    for (idx, request) in state.network_requests.submitted.iter().take(6).enumerate() {
        rows.push(ActivityEventRow {
            event_id: format!("network:request:{}", request.request_id),
            domain: ActivityEventDomain::Network,
            source_tag: ActivityEventDomain::Network.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(20 + idx as u64 * 2),
            summary: format!("{} {}", request.request_id, request.status.label()),
            detail: format!("{} -> {}", request.request_type, request.response_stream_id),
        });
    }

    if let Some(profile_event_id) = state.sa_lane.profile_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("sa:profile:{profile_event_id}"),
            domain: ActivityEventDomain::Sa,
            source_tag: ActivityEventDomain::Sa.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(3),
            summary: format!("SA profile {}", state.sa_lane.mode.label()),
            detail: profile_event_id.to_string(),
        });
    }
    if let Some(tick_event_id) = state.sa_lane.last_tick_result_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("sa:tick:{tick_event_id}"),
            domain: ActivityEventDomain::Sa,
            source_tag: ActivityEventDomain::Sa.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(4),
            summary: format!("SA tick {}", state.sa_lane.tick_count),
            detail: tick_event_id.to_string(),
        });
    }

    if let Some(manifest) = state.skl_lane.manifest_a.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("skl:manifest:{manifest}"),
            domain: ActivityEventDomain::Skl,
            source_tag: ActivityEventDomain::Skl.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(5),
            summary: format!("SKL trust {}", state.skl_lane.trust_tier.label()),
            detail: manifest.to_string(),
        });
    }

    if let Some(intent_event_id) = state.ac_lane.intent_event_id.as_deref() {
        rows.push(ActivityEventRow {
            event_id: format!("ac:intent:{intent_event_id}"),
            domain: ActivityEventDomain::Ac,
            source_tag: ActivityEventDomain::Ac.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch.saturating_sub(6),
            summary: if state.ac_lane.credit_available {
                "AC credit available".to_string()
            } else {
                "AC credit unavailable".to_string()
            },
            detail: intent_event_id.to_string(),
        });
    }

    for response in state.runtime_command_responses.iter().rev().take(12) {
        let domain = match response.lane {
            RuntimeLane::SaLifecycle => ActivityEventDomain::Sa,
            RuntimeLane::SklDiscoveryTrust => ActivityEventDomain::Skl,
            RuntimeLane::AcCredit => ActivityEventDomain::Ac,
        };
        rows.push(ActivityEventRow {
            event_id: format!("runtime:cmd:{}", response.command_seq),
            domain,
            source_tag: domain.source_tag().to_string(),
            occurred_at_epoch_seconds: now_epoch
                .saturating_sub(7_u64.saturating_add(response.command_seq)),
            summary: format!("{} {}", response.command.label(), response.status.label()),
            detail: response
                .event_id
                .clone()
                .unwrap_or_else(|| "event:n/a".to_string()),
        });
    }

    rows.push(ActivityEventRow {
        event_id: format!("sync:cursor:{}", state.sync_health.last_applied_event_seq),
        domain: ActivityEventDomain::Sync,
        source_tag: ActivityEventDomain::Sync.source_tag().to_string(),
        occurred_at_epoch_seconds: now_epoch.saturating_sub(2),
        summary: format!(
            "cursor={} phase={}",
            state.sync_health.last_applied_event_seq,
            state.sync_health.recovery_phase.label()
        ),
        detail: format!(
            "stale_age={}s duplicate_drops={}",
            state.sync_health.cursor_last_advanced_seconds_ago,
            state.sync_health.duplicate_drop_count
        ),
    });

    rows
}

pub(super) fn refresh_earnings_scoreboard(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) {
    state.earnings_scoreboard.refresh_from_sources(
        now,
        &state.provider_runtime,
        &state.job_history,
        &state.spark_wallet,
    );
}

pub(super) fn refresh_sync_health(state: &mut crate::app_state::RenderState) {
    state.sync_health.refresh_from_runtime(
        std::time::Instant::now(),
        &state.provider_runtime,
        &state.relay_connections,
    );
}

pub(super) fn upsert_runtime_incident_alert(
    state: &mut crate::app_state::RenderState,
    response: &RuntimeCommandResponse,
) {
    let domain = match response.lane {
        RuntimeLane::SaLifecycle => AlertDomain::ProviderRuntime,
        RuntimeLane::SklDiscoveryTrust => AlertDomain::SkillTrust,
        RuntimeLane::AcCredit => AlertDomain::Credit,
    };
    let severity = match response.status {
        RuntimeCommandStatus::Accepted => crate::app_state::AlertSeverity::Info,
        RuntimeCommandStatus::Retryable => crate::app_state::AlertSeverity::Warning,
        RuntimeCommandStatus::Rejected => crate::app_state::AlertSeverity::Critical,
    };
    let alert_id = format!(
        "alert:{}:{}",
        response.lane.label(),
        response.command.label()
    );
    let summary = if let Some(error) = response.error.as_ref() {
        format!(
            "{} {} ({})",
            response.command.label(),
            response.status.label(),
            error.class.label()
        )
    } else {
        format!("{} {}", response.command.label(), response.status.label())
    };
    let remediation = if let Some(error) = response.error.as_ref() {
        format!(
            "Investigate {} lane command failure: {}",
            response.lane.label(),
            error.message
        )
    } else {
        format!("Review {} lane runtime status.", response.lane.label())
    };

    if let Some(existing) = state
        .alerts_recovery
        .alerts
        .iter_mut()
        .find(|alert| alert.alert_id == alert_id)
    {
        existing.domain = domain;
        existing.severity = severity;
        existing.lifecycle = crate::app_state::AlertLifecycle::Active;
        existing.summary = summary;
        existing.remediation = remediation;
        existing.last_transition_epoch_seconds =
            existing.last_transition_epoch_seconds.saturating_add(1);
    } else {
        state
            .alerts_recovery
            .alerts
            .push(crate::app_state::RecoveryAlertRow {
                alert_id,
                domain,
                severity,
                lifecycle: crate::app_state::AlertLifecycle::Active,
                summary,
                remediation,
                last_transition_epoch_seconds: state
                    .job_history
                    .reference_epoch_seconds
                    .saturating_add(state.alerts_recovery.alerts.len() as u64 * 29),
            });
    }
    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
    state.alerts_recovery.last_error = None;
    state.alerts_recovery.last_action = Some("Updated runtime incident queue".to_string());
}

pub(super) fn run_spark_action(
    state: &mut crate::app_state::RenderState,
    action: SparkPaneAction,
) -> bool {
    if action == SparkPaneAction::CopySparkAddress {
        state.spark_wallet.last_error = None;
        let notice = match state.spark_wallet.spark_address.as_deref() {
            Some(address) if !address.trim().is_empty() => match copy_to_clipboard(address) {
                Ok(()) => "Copied Spark address to clipboard".to_string(),
                Err(error) => format!("Failed to copy Spark address: {error}"),
            },
            _ => "No Spark address available. Generate Spark receive first.".to_string(),
        };

        if notice.starts_with("Failed") || notice.starts_with("No Spark address") {
            state.spark_wallet.last_error = Some(notice);
        } else {
            state.spark_wallet.last_action = Some(notice);
        }
        return true;
    }

    let command = match build_spark_command_for_action(
        action,
        state.spark_inputs.invoice_amount.get_value(),
        state.spark_inputs.send_request.get_value(),
        state.spark_inputs.send_amount.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

pub(super) fn run_pay_invoice_action(
    state: &mut crate::app_state::RenderState,
    action: PayInvoicePaneAction,
) -> bool {
    let command = match build_pay_invoice_command(
        action,
        state.pay_invoice_inputs.payment_request.get_value(),
        state.pay_invoice_inputs.amount_sats.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

pub(super) fn run_create_invoice_action(
    state: &mut crate::app_state::RenderState,
    action: CreateInvoicePaneAction,
) -> bool {
    if action == CreateInvoicePaneAction::CopyInvoice {
        state.spark_wallet.last_error = None;
        let notice = match state.spark_wallet.last_invoice.as_deref() {
            Some(invoice) if !invoice.trim().is_empty() => match copy_to_clipboard(invoice) {
                Ok(()) => "Copied invoice to clipboard".to_string(),
                Err(error) => format!("Failed to copy invoice: {error}"),
            },
            _ => "No invoice generated yet. Create one first.".to_string(),
        };

        if notice.starts_with("Failed") || notice.starts_with("No invoice generated") {
            state.spark_wallet.last_error = Some(notice);
        } else {
            state.spark_wallet.last_action = Some(notice);
        }
        return true;
    }

    let command = match build_create_invoice_command(
        action,
        state.create_invoice_inputs.amount_sats.get_value(),
        state.create_invoice_inputs.description.get_value(),
        state.create_invoice_inputs.expiry_seconds.get_value(),
    ) {
        Ok(command) => command,
        Err(error) => {
            state.spark_wallet.last_error = Some(error);
            return true;
        }
    };

    queue_spark_command(state, command);
    true
}

pub(super) fn build_spark_command_for_action(
    action: SparkPaneAction,
    invoice_amount: &str,
    send_request: &str,
    send_amount: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        SparkPaneAction::Refresh => Ok(SparkWalletCommand::Refresh),
        SparkPaneAction::GenerateSparkAddress => Ok(SparkWalletCommand::GenerateSparkAddress),
        SparkPaneAction::GenerateBitcoinAddress => Ok(SparkWalletCommand::GenerateBitcoinAddress),
        SparkPaneAction::CopySparkAddress => {
            Err("Spark copy action is handled directly in UI".to_string())
        }
        SparkPaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateInvoice {
            amount_sats: parse_positive_amount_str(invoice_amount, "Invoice amount")?,
            description: Some("OpenAgents Spark receive".to_string()),
            expiry_seconds: Some(3600),
        }),
        SparkPaneAction::SendPayment => {
            let request = validate_lightning_payment_request(send_request)?;

            let amount = if send_amount.trim().is_empty() {
                None
            } else {
                Some(parse_positive_amount_str(send_amount, "Send amount")?)
            };

            Ok(SparkWalletCommand::SendPayment {
                payment_request: request,
                amount_sats: amount,
            })
        }
    }
}

pub(super) fn build_pay_invoice_command(
    action: PayInvoicePaneAction,
    payment_request: &str,
    amount_sats: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        PayInvoicePaneAction::SendPayment => {
            let request = validate_lightning_payment_request(payment_request)?;

            let amount = if amount_sats.trim().is_empty() {
                None
            } else {
                Some(parse_positive_amount_str(amount_sats, "Send amount")?)
            };

            Ok(SparkWalletCommand::SendPayment {
                payment_request: request,
                amount_sats: amount,
            })
        }
    }
}

pub(super) fn build_create_invoice_command(
    action: CreateInvoicePaneAction,
    amount_sats: &str,
    description: &str,
    expiry_seconds: &str,
) -> Result<SparkWalletCommand, String> {
    match action {
        CreateInvoicePaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateInvoice {
            amount_sats: parse_positive_amount_str(amount_sats, "Invoice amount")?,
            description: normalize_optional_text(description),
            expiry_seconds: parse_optional_positive_amount_str(expiry_seconds, "Expiry seconds")?,
        }),
        CreateInvoicePaneAction::CopyInvoice => {
            Err("Copy invoice action is handled directly in UI".to_string())
        }
    }
}

pub(super) fn validate_lightning_payment_request(raw: &str) -> Result<String, String> {
    let request = raw.trim();
    if request.is_empty() {
        return Err("Payment request cannot be empty".to_string());
    }

    let normalized = request.to_ascii_lowercase();
    let is_invoice = normalized.starts_with("ln")
        || normalized.starts_with("lightning:ln")
        || normalized.starts_with("lightning://ln");
    if !is_invoice {
        return Err(
            "Payment request must be a Lightning invoice (expected prefix ln...)".to_string(),
        );
    }

    Ok(request.to_string())
}

pub(super) fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn next_trajectory_step_filter(current: &str) -> String {
    match current {
        "all" => "tick".to_string(),
        "tick" => "delivery".to_string(),
        "delivery" => "settlement".to_string(),
        _ => "all".to_string(),
    }
}

pub(super) fn trajectory_verification_hash(
    session_id: &str,
    tick_event: &str,
    tick_count: u64,
) -> String {
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    tick_event.hash(&mut hasher);
    tick_count.hash(&mut hasher);
    format!("trajhash:{:016x}", hasher.finish())
}

pub(super) fn skill_scope_from_scope(scope: &str) -> Option<String> {
    let trimmed = scope.trim();
    if !trimmed.starts_with("skill:") {
        return None;
    }
    let scope_value = trimmed.trim_start_matches("skill:");
    match scope_value.rsplit_once(':') {
        Some((skill_scope_id, _constraints_hash)) if !skill_scope_id.trim().is_empty() => {
            Some(skill_scope_id.to_string())
        }
        _ if !scope_value.is_empty() => Some(scope_value.to_string()),
        _ => None,
    }
}

pub(super) fn parse_optional_positive_amount_str(
    raw: &str,
    label: &str,
) -> Result<Option<u64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_positive_amount_str(trimmed, label).map(Some)
}

pub(super) fn queue_spark_command(
    state: &mut crate::app_state::RenderState,
    command: SparkWalletCommand,
) {
    state.spark_wallet.last_error = None;
    if let Err(error) = state.spark_worker.enqueue(command) {
        state.spark_wallet.last_error = Some(error);
    }
}

pub(super) fn parse_positive_amount_str(raw: &str, label: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }

    match trimmed.parse::<u64>() {
        Ok(value) if value > 0 => Ok(value),
        Ok(_) => Err(format!("{label} must be greater than 0")),
        Err(error) => Err(format!("{label} must be a valid integer: {error}")),
    }
}
