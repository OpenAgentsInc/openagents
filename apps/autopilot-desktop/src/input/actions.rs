use super::*;
use crate::apple_fm_bridge::{
    AppleFmBridgeCommand, AppleFmMissionControlSummaryCommand, AppleFmWorkbenchCommand,
    AppleFmWorkbenchOperation, AppleFmWorkbenchToolMode,
};
use crate::bitcoin_display::format_sats_amount;
use crate::local_inference_runtime::{LocalInferenceGenerateJob, LocalInferenceRuntimeCommand};
use crate::pane_system::{
    AppleFmWorkbenchPaneAction, BuyModePaymentsPaneAction, CHAT_AUTOPILOT_THREAD_PREVIEW_LIMIT,
    LocalInferencePaneAction, MissionControlPaneAction,
};
use crate::spark_wallet::{
    decode_lightning_invoice_payment_hash, is_settled_wallet_payment_status,
    normalize_lightning_invoice_ref,
};
use crate::state::job_inbox::JobExecutionParam;
use psionic_apple_fm::{AppleFmGenerationOptions, AppleFmSamplingMode};

const MANAGED_CHAT_PUBLISH_TRANSPORT_UNWIRED: &str =
    "Managed chat relay publish transport is not wired yet; local echo saved for retry.";
const MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED: &str =
    "Managed chat relay control transport is not wired yet; no server state changed.";
const DIRECT_MESSAGE_PUBLISH_TRANSPORT_UNWIRED: &str =
    "Direct message relay publish transport is not wired yet; local echo saved for retry.";
const MISSION_CONTROL_BUY_MODE_PROMPT: &str = "Reply with the exact text BUY MODE OK.";
const MISSION_CONTROL_LOCAL_FM_SUMMARY_INSTRUCTIONS: &str = "You are the Mission Control local Foundation Models test. Summarize only the supplied context in 3 short markdown bullets. Highlight the latest result, current buyer/provider state, and the next operator action. Do not invent facts or mention missing data unless it matters.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ManagedChatComposerIntent {
    ChannelMessage {
        content: String,
        reply_reference: Option<String>,
    },
    Reaction {
        message_reference: String,
        reaction: String,
    },
    DeleteMessage {
        message_reference: String,
        reason: Option<String>,
    },
    RemoveUser {
        member_reference: String,
        reason: Option<String>,
    },
    Invite {
        code: String,
        reason: Option<String>,
    },
    Join {
        invite_code: Option<String>,
        reason: Option<String>,
    },
    Leave {
        reason: Option<String>,
    },
    EditMetadata {
        changes: Vec<Vec<String>>,
        summary: String,
    },
    MuteMember {
        member_reference: String,
        muted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirectMessageComposerIntent {
    CreateRoom {
        participant_pubkeys: Vec<String>,
        subject: Option<String>,
        content: String,
    },
    RoomMessage {
        content: String,
        reply_reference: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatWalletComposerIntent {
    PayInvoice {
        message_reference: String,
    },
    RequestInvoice {
        message_reference: String,
        description: Option<String>,
    },
    CopyAddress {
        message_reference: String,
    },
    InspectPaymentStatus {
        message_reference: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatSpacetimeComposerIntent {
    Search { query: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatGitComposerIntent {
    Status,
    Pull,
    Init,
    BranchList,
    BranchCreate { branch: String },
    Checkout { branch: String },
    WorktreeList,
    WorktreeAdd { path: String, branch: String },
    WorktreeRemove { path: String },
    PrPrep { base_branch: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatTerminalComposerIntent {
    Open,
    Write { text: String },
    Resize { cols: u16, rows: u16 },
    Clear,
    Restart,
    Close,
    ListSessions,
    CleanClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatSkillsComposerIntent {
    Summary,
    Refresh,
    Inspect { query: Option<String> },
    Use { query: String },
    Clear,
    SetEnabled { query: String, enabled: bool },
    RemoteSummary,
    RemoteList { scope: codex_client::HazelnutScope },
    RemoteExport { query: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatMcpComposerIntent {
    Summary,
    Refresh,
    Reload,
    Select { query: String },
    Login { query: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatAppsComposerIntent {
    Summary,
    Refresh,
    Inspect { query: Option<String> },
    Select { query: String },
}

#[derive(Debug, Clone)]
enum ChatRequestComposerIntent {
    Summary,
    Approval {
        decision: ApprovalDecision,
        label: &'static str,
    },
    ToolCallRespond,
    ToolUserInputRespond,
    AuthRefreshRespond,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatRemoteComposerIntent {
    Summary,
    Enable { bind_addr: Option<String> },
    Disable,
    RotateToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatWalletMessageSource {
    reference_label: String,
    message_id: String,
    content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ChatWalletMessagePayload {
    payment_request: Option<String>,
    payment_id: Option<String>,
    chat_reported_status: Option<String>,
    amount_sats: Option<u64>,
    copy_address: Option<String>,
    copy_address_label: Option<&'static str>,
    description: Option<String>,
}

pub(super) fn run_chat_submit_action(state: &mut crate::app_state::RenderState) -> bool {
    run_chat_submit_action_with_trigger(
        state,
        crate::labor_orchestrator::CodexRunTrigger::PersonalAgent,
    )
}

pub(super) fn run_chat_submit_action_with_trigger(
    state: &mut crate::app_state::RenderState,
    trigger: crate::labor_orchestrator::CodexRunTrigger,
) -> bool {
    focus_chat_composer(state);
    let prompt = state.chat_inputs.composer.get_value().to_string();
    let trimmed_prompt = prompt.trim().to_string();
    match parse_direct_message_creation_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_direct_message_submit_action(state, Some(intent));
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match state.autopilot_chat.chat_browse_mode() {
        crate::app_state::ChatBrowseMode::Managed => return run_managed_chat_submit_action(state),
        crate::app_state::ChatBrowseMode::DirectMessages => {
            return run_direct_message_submit_action(state, None);
        }
        crate::app_state::ChatBrowseMode::Autopilot => {}
    }
    let prompt_chars = trimmed_prompt.chars().count();
    if trimmed_prompt.is_empty() {
        return false;
    }
    match parse_chat_git_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_git_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_terminal_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_terminal_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_skills_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_skills_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_mcp_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_mcp_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_apps_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_apps_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_request_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_request_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_remote_intent(&trimmed_prompt) {
        Ok(Some(intent)) => {
            return run_chat_remote_action(state, prompt, intent);
        }
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() else {
        state.autopilot_chat.last_error =
            Some("No active thread yet. Wait for Codex lane readiness.".to_string());
        return true;
    };
    let session_cwd = current_chat_session_cwd(state);
    let (parsed_prompt, attachment_error) =
        parse_chat_turn_prompt(prompt.clone(), session_cwd.as_deref());
    if let Some(error) = attachment_error {
        state.autopilot_chat.last_error = Some(error);
        return true;
    }

    let classification =
        super::cad_turn_classifier::classify_chat_prompt(&parsed_prompt.prompt_text);
    let submitted_at_epoch_ms = current_epoch_millis();
    state.autopilot_chat.record_turn_timeline_event(format!(
        "cad-turn classifier: is_cad_turn={} reason={}",
        classification.is_cad_turn, classification.reason
    ));
    let _ = super::reducers::apply_chat_prompt_to_cad_session(
        state,
        &thread_id,
        &parsed_prompt.prompt_text,
    );

    let mut turn_skill_attachments = selected_skill_candidates_for_turn(state);
    if let Some(goal_selection) = goal_policy_skill_candidates_for_turn(state) {
        let names = goal_selection
            .candidates
            .iter()
            .map(|candidate| candidate.attachment.name.clone())
            .collect::<Vec<_>>()
            .join(",");
        let reasons = goal_selection
            .candidates
            .iter()
            .map(|candidate| format!("{}: {}", candidate.attachment.name, candidate.reason))
            .collect::<Vec<_>>()
            .join(" | ");
        state.autopilot_chat.record_turn_timeline_event(format!(
            "selected skills (goal={} objective={}): {names}",
            goal_selection.goal_id, goal_selection.objective_tag
        ));
        state
            .autopilot_chat
            .record_turn_timeline_event(format!("selected skills reasons: {reasons}"));
        for candidate in goal_selection.candidates {
            turn_skill_attachments.push(candidate.attachment);
        }
    }
    let mut required_skill_errors = Vec::new();
    let mut policy_skills = cad_policy_skill_candidates_for_turn(
        classification.is_cad_turn,
        &state.skill_registry.discovered_skills,
    );
    let mut policy_skill_names = policy_skills
        .iter()
        .map(|skill| skill.name.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    if classification.is_cad_turn {
        match crate::skill_autoload::ensure_required_cad_skills() {
            Ok(managed_skills) => {
                for managed_skill in managed_skills {
                    if policy_skill_names.insert(managed_skill.name.to_ascii_lowercase()) {
                        policy_skills.push(TurnSkillAttachment {
                            name: managed_skill.name,
                            path: managed_skill.path,
                            enabled: true,
                            source: TurnSkillSource::PolicyRequired,
                        });
                    }
                }
            }
            Err(error) => required_skill_errors
                .push(format!("Failed to auto-load required CAD skills: {error}")),
        }
    }
    if classification.is_cad_turn && !policy_skills.is_empty() {
        let names = policy_skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        state
            .autopilot_chat
            .record_turn_timeline_event(format!("cad required skills attached: {names}"));
    }
    for mut policy_skill in policy_skills {
        policy_skill.enabled = true;
        turn_skill_attachments.push(policy_skill);
    }
    let selected_skill_names = turn_skill_attachments
        .iter()
        .map(|skill| skill.name.clone())
        .collect::<Vec<_>>();

    log_chat_prompt_to_console(&thread_id, &prompt);
    let (input, skill_error) = assemble_chat_turn_input(parsed_prompt, turn_skill_attachments);
    if let Some(skill_error) = skill_error {
        state.autopilot_chat.last_error = Some(skill_error);
    }
    if !required_skill_errors.is_empty() {
        required_skill_errors.sort();
        required_skill_errors.dedup();
        let error = required_skill_errors.join(" | ");
        state.autopilot_chat.last_error = Some(error.clone());
        state
            .autopilot_chat
            .record_turn_timeline_event(format!("cad skill policy blocked turn: {error}"));
        return true;
    }

    state
        .autopilot_chat
        .remember_submission_draft(&thread_id, prompt.clone());
    state.chat_inputs.composer.set_value(String::new());
    state.autopilot_chat.record_composer_draft(String::new());

    if let Some(active_turn_id) = state.autopilot_chat.active_turn_id.clone() {
        let command =
            crate::codex_lane::CodexLaneCommand::TurnSteer(codex_client::TurnSteerParams {
                thread_id: thread_id.clone(),
                input,
                expected_turn_id: active_turn_id.clone(),
            });
        tracing::info!(
            "codex turn/steer request thread_id={} turn_id={} chars={}",
            thread_id,
            active_turn_id,
            prompt_chars
        );
        match state.queue_codex_command(command) {
            Ok(seq) => {
                state.autopilot_chat.enqueue_pending_steer_submission(
                    seq,
                    thread_id.clone(),
                    prompt,
                );
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "turn steer queued: seq={seq} turn_id={active_turn_id}"
                ));
                state.autopilot_chat.last_error = None;
            }
            Err(error) => {
                state.chat_inputs.composer.set_value(prompt.clone());
                state.autopilot_chat.record_composer_draft(prompt);
                state.autopilot_chat.last_error = Some(error);
            }
        }
        return true;
    }

    let model_override = state.autopilot_chat.selected_model_override();
    let model_label = model_override.as_deref().unwrap_or("server-default");
    let plan = crate::labor_orchestrator::orchestrate_codex_turn(
        crate::labor_orchestrator::CodexTurnExecutionRequest {
            trigger,
            submitted_at_epoch_ms,
            thread_id: thread_id.clone(),
            input,
            cwd: session_cwd.clone().map(std::path::PathBuf::from),
            approval_policy: chat_session_approval_policy(state),
            sandbox_policy: chat_session_turn_sandbox_policy(state),
            model: model_override.clone(),
            service_tier: chat_session_service_tier(state),
            effort: chat_session_reasoning_effort(state),
            personality: chat_session_personality(state),
            collaboration_mode: chat_session_collaboration_mode(state),
        },
    );
    state.autopilot_chat.record_turn_submission_metadata(
        &thread_id,
        plan.classification.clone(),
        plan.labor_binding.clone(),
        classification.is_cad_turn,
        classification.reason.clone(),
        submitted_at_epoch_ms,
        Vec::new(),
    );
    state
        .autopilot_chat
        .set_last_pending_turn_selected_skills(selected_skill_names);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor orchestrator: {}",
        plan.classification.timeline_descriptor()
    ));
    if let Some(binding) = plan.labor_binding.as_ref() {
        state.autopilot_chat.record_turn_timeline_event(format!(
            "labor binding: work_unit_id={} contract_id={} bundle_id={}",
            binding.work_unit_id, binding.contract_id, binding.provenance.bundle_id
        ));
    }
    state.autopilot_chat.submit_prompt(prompt.clone());

    tracing::info!(
        "codex turn/start request thread_id={} model={} chars={} class={} economic={} labor_bound={}",
        thread_id,
        model_label,
        prompt_chars,
        plan.classification.label(),
        plan.classification.is_economically_meaningful(),
        plan.classification.is_labor_market_bound()
    );
    match state.queue_codex_command(plan.command) {
        Ok(seq) => {
            tracing::info!(
                "codex turn/start queued seq={} thread_id={}",
                seq,
                thread_id
            );
        }
        Err(error) => {
            state.chat_inputs.composer.set_value(prompt.clone());
            state.autopilot_chat.record_composer_draft(prompt.clone());
            state
                .autopilot_chat
                .mark_pending_turn_dispatch_failed(error);
        }
    }
    true
}

fn run_managed_chat_submit_action(state: &mut crate::app_state::RenderState) -> bool {
    let prompt = state.chat_inputs.composer.get_value().trim().to_string();
    if prompt.is_empty() {
        let Some(event_id) = state
            .autopilot_chat
            .active_managed_chat_retryable_message()
            .map(|message| message.event_id.clone())
        else {
            return false;
        };
        if let Err(error) = state
            .autopilot_chat
            .managed_chat_projection
            .retry_outbound_message(&event_id)
        {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
        state.autopilot_chat.last_error = None;
        state.autopilot_chat.reset_transcript_scroll();
        return true;
    }

    match parse_chat_wallet_intent(&prompt) {
        Ok(Some(intent)) => return run_chat_wallet_action(state, intent),
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_spacetime_intent(&prompt) {
        Ok(Some(intent)) => return run_chat_spacetime_action(state, intent),
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }

    let intent = match parse_managed_chat_composer_intent(&prompt) {
        Ok(intent) => intent,
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    };
    let Some(group) = state.autopilot_chat.active_managed_chat_group().cloned() else {
        state.autopilot_chat.last_error = Some("No managed chat group is selected.".to_string());
        return true;
    };

    if let ManagedChatComposerIntent::MuteMember {
        member_reference,
        muted,
    } = &intent
    {
        let Some(member) = resolve_managed_chat_member_reference(&group, member_reference) else {
            state.autopilot_chat.last_error = Some(format!(
                "Unknown managed chat member reference: {member_reference}"
            ));
            return true;
        };
        if let Err(error) = state
            .autopilot_chat
            .managed_chat_projection
            .set_pubkey_muted(&member.pubkey, *muted)
        {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
        state.chat_inputs.composer.set_value(String::new());
        state.autopilot_chat.last_error = None;
        state.autopilot_chat.set_copy_notice(
            std::time::Instant::now(),
            if *muted {
                format!(
                    "Locally muted {}. Relay membership and permissions are unchanged.",
                    compact_member_reference(&member.pubkey)
                )
            } else {
                format!(
                    "Removed local mute for {}.",
                    compact_member_reference(&member.pubkey)
                )
            },
        );
        return true;
    }

    let Some(identity) = state.nostr_identity.as_ref() else {
        state.autopilot_chat.last_error =
            Some("No Nostr identity is loaded for managed chat publishing.".to_string());
        return true;
    };

    if matches!(
        &intent,
        ManagedChatComposerIntent::DeleteMessage { .. }
            | ManagedChatComposerIntent::RemoveUser { .. }
            | ManagedChatComposerIntent::Invite { .. }
            | ManagedChatComposerIntent::EditMetadata { .. }
    ) && !state.autopilot_chat.active_managed_chat_local_is_admin()
    {
        state.autopilot_chat.last_error = Some(
            "Managed relay moderation commands require an admin role in the active group."
                .to_string(),
        );
        return true;
    }

    match intent {
        ManagedChatComposerIntent::Reaction {
            message_reference,
            reaction,
        } => {
            let Some(target_message) =
                resolve_managed_chat_message_reference(&state.autopilot_chat, &message_reference)
            else {
                state.autopilot_chat.last_error = Some(format!(
                    "Unknown managed chat message reference for reaction: {message_reference}"
                ));
                return true;
            };
            let Some(channel) = state.autopilot_chat.active_managed_chat_channel().cloned() else {
                state.autopilot_chat.last_error =
                    Some("No managed chat channel is selected.".to_string());
                return true;
            };
            if let Err(error) = build_managed_chat_reaction_event(
                identity,
                &group.group_id,
                &channel,
                target_message,
                &reaction,
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_PUBLISH_TRANSPORT_UNWIRED} Reaction {reaction} for {} was not published.",
                target_message.event_id
            ));
            true
        }
        ManagedChatComposerIntent::ChannelMessage {
            content,
            reply_reference,
        } => {
            let reply_event_id = match reply_reference.as_deref() {
                Some(reference) => {
                    let Some(target_message) =
                        resolve_managed_chat_message_reference(&state.autopilot_chat, reference)
                    else {
                        state.autopilot_chat.last_error = Some(format!(
                            "Unknown managed chat message reference for reply: {reference}"
                        ));
                        return true;
                    };
                    Some(target_message.event_id.clone())
                }
                None => None,
            };
            state.chat_inputs.composer.set_value(String::new());
            if let Err(error) = queue_managed_chat_channel_message(
                &mut state.autopilot_chat,
                identity,
                &content,
                reply_event_id.as_deref(),
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            true
        }
        ManagedChatComposerIntent::DeleteMessage {
            message_reference,
            reason,
        } => {
            let Some(target_message) =
                resolve_managed_chat_message_reference(&state.autopilot_chat, &message_reference)
            else {
                state.autopilot_chat.last_error = Some(format!(
                    "Unknown managed chat message reference for delete: {message_reference}"
                ));
                return true;
            };
            if let Err(error) = build_managed_chat_moderation_event(
                identity,
                &group.group_id,
                nostr::ModerationAction::DeleteEvent {
                    event_id: target_message.event_id.clone(),
                },
                reason.as_deref(),
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Delete request for {} was not published.",
                target_message.event_id
            ));
            true
        }
        ManagedChatComposerIntent::RemoveUser {
            member_reference,
            reason,
        } => {
            let Some(member) = resolve_managed_chat_member_reference(&group, &member_reference)
            else {
                state.autopilot_chat.last_error = Some(format!(
                    "Unknown managed chat member reference: {member_reference}"
                ));
                return true;
            };
            if let Err(error) = build_managed_chat_moderation_event(
                identity,
                &group.group_id,
                nostr::ModerationAction::RemoveUser {
                    pubkey: member.pubkey.clone(),
                },
                reason.as_deref(),
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Remove-user request for {} was not published.",
                member.pubkey
            ));
            true
        }
        ManagedChatComposerIntent::Invite { code, reason } => {
            if let Err(error) = build_managed_chat_moderation_event(
                identity,
                &group.group_id,
                nostr::ModerationAction::CreateInvite { code: code.clone() },
                reason.as_deref(),
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Invite code `{code}` was not published."
            ));
            true
        }
        ManagedChatComposerIntent::Join {
            invite_code,
            reason,
        } => {
            if let Err(error) = build_managed_chat_join_request_event(
                identity,
                &group.group_id,
                invite_code.as_deref(),
                reason.as_deref(),
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Join request for {} was not published.",
                group.group_id
            ));
            true
        }
        ManagedChatComposerIntent::Leave { reason } => {
            if let Err(error) =
                build_managed_chat_leave_request_event(identity, &group.group_id, reason.as_deref())
            {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Leave request for {} was not published.",
                group.group_id
            ));
            true
        }
        ManagedChatComposerIntent::EditMetadata { changes, summary } => {
            if let Err(error) = build_managed_chat_moderation_event(
                identity,
                &group.group_id,
                nostr::ModerationAction::EditMetadata { changes },
                None,
            ) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = Some(format!(
                "{MANAGED_CHAT_CONTROL_TRANSPORT_UNWIRED} Metadata edit `{summary}` was not published."
            ));
            true
        }
        ManagedChatComposerIntent::MuteMember { .. } => unreachable!(),
    }
}

fn run_direct_message_submit_action(
    state: &mut crate::app_state::RenderState,
    override_intent: Option<DirectMessageComposerIntent>,
) -> bool {
    let prompt = state.chat_inputs.composer.get_value().trim().to_string();
    if prompt.is_empty() {
        let Some(message_id) = state
            .autopilot_chat
            .active_direct_message_retryable_message()
            .map(|message| message.message_id.clone())
        else {
            return false;
        };
        if let Err(error) = state
            .autopilot_chat
            .direct_message_projection
            .retry_outbound_message(&message_id)
        {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
        state.autopilot_chat.reset_transcript_scroll();
        if let Err(error) = state
            .autopilot_chat
            .direct_message_projection
            .fail_outbound_message(&message_id, DIRECT_MESSAGE_PUBLISH_TRANSPORT_UNWIRED)
        {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
        state.autopilot_chat.last_error =
            Some(DIRECT_MESSAGE_PUBLISH_TRANSPORT_UNWIRED.to_string());
        return true;
    }

    match parse_chat_wallet_intent(&prompt) {
        Ok(Some(intent)) => return run_chat_wallet_action(state, intent),
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }
    match parse_chat_spacetime_intent(&prompt) {
        Ok(Some(intent)) => return run_chat_spacetime_action(state, intent),
        Ok(None) => {}
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    }

    let Some(identity) = state.nostr_identity.as_ref() else {
        state.autopilot_chat.last_error =
            Some("No Nostr identity is loaded for direct message publishing.".to_string());
        return true;
    };
    let intent = match override_intent {
        Some(intent) => intent,
        None => match parse_direct_message_room_intent(&prompt) {
            Ok(intent) => intent,
            Err(error) => {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }
        },
    };

    let (content, participant_pubkeys, subject, reply_target) = match intent {
        DirectMessageComposerIntent::CreateRoom {
            participant_pubkeys,
            subject,
            content,
        } => (content, participant_pubkeys, subject, None),
        DirectMessageComposerIntent::RoomMessage {
            content,
            reply_reference,
        } => {
            let Some(room) = state.autopilot_chat.active_direct_message_room().cloned() else {
                state.autopilot_chat.last_error = Some(
                    "No direct message room is selected. Use `dm <pubkey> <text>` or `room <pubkey[,pubkey...]> | <subject> | <text>`."
                        .to_string(),
                );
                return true;
            };
            let reply_target = match reply_reference.as_deref() {
                Some(reference) => {
                    let Some(target_message) =
                        resolve_direct_message_reference(&state.autopilot_chat, reference)
                    else {
                        state.autopilot_chat.last_error = Some(format!(
                            "Unknown direct message reference for reply: {reference}"
                        ));
                        return true;
                    };
                    Some(target_message)
                }
                None => None,
            };
            (
                content,
                room.participant_pubkeys.clone(),
                room.subject.clone(),
                reply_target,
            )
        }
    };

    let recipient_relay_hints = resolve_direct_message_recipient_relay_hints(
        state,
        identity.public_key_hex.as_str(),
        &participant_pubkeys,
    );
    let outbound_message = match build_direct_message_outbound_message(
        identity,
        participant_pubkeys,
        recipient_relay_hints,
        &content,
        reply_target,
        subject,
    ) {
        Ok(outbound_message) => outbound_message,
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
            return true;
        }
    };
    let message_id = outbound_message.message_id.clone();
    let room_id = outbound_message.room_id.clone();

    state.chat_inputs.composer.set_value(String::new());
    if let Err(error) = state
        .autopilot_chat
        .direct_message_projection
        .queue_outbound_message(outbound_message)
    {
        state.autopilot_chat.last_error = Some(error);
        return true;
    }
    state.autopilot_chat.selected_workspace =
        crate::app_state::ChatWorkspaceSelection::DirectMessages;
    if let Err(error) = state
        .autopilot_chat
        .direct_message_projection
        .set_selected_room(&room_id)
    {
        state.autopilot_chat.last_error = Some(error);
        return true;
    }
    state.autopilot_chat.reset_transcript_scroll();
    if let Err(error) = state
        .autopilot_chat
        .direct_message_projection
        .fail_outbound_message(&message_id, DIRECT_MESSAGE_PUBLISH_TRANSPORT_UNWIRED)
    {
        state.autopilot_chat.last_error = Some(error);
        return true;
    }
    state.autopilot_chat.last_error = Some(DIRECT_MESSAGE_PUBLISH_TRANSPORT_UNWIRED.to_string());
    true
}

fn parse_chat_wallet_intent(prompt: &str) -> Result<Option<ChatWalletComposerIntent>, String> {
    let trimmed = prompt.trim();
    let Some(rest) = trimmed.strip_prefix("wallet ") else {
        return Ok(None);
    };
    let rest = rest.trim();
    if let Some(reference) = rest.strip_prefix("pay ") {
        let reference = reference.trim();
        if reference.is_empty() {
            return Err("Wallet pay syntax is `wallet pay <message-number|id-prefix>`".to_string());
        }
        return Ok(Some(ChatWalletComposerIntent::PayInvoice {
            message_reference: reference.to_string(),
        }));
    }
    if let Some(rest) = rest
        .strip_prefix("request ")
        .or_else(|| rest.strip_prefix("invoice "))
    {
        let (message_reference, description) = parse_reference_with_optional_reason(
            rest,
            "Wallet request syntax is `wallet request <message-number|id-prefix> [description]`",
        )?;
        return Ok(Some(ChatWalletComposerIntent::RequestInvoice {
            message_reference,
            description,
        }));
    }
    if let Some(reference) = rest
        .strip_prefix("copy-address ")
        .or_else(|| rest.strip_prefix("copy "))
    {
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(
                "Wallet copy syntax is `wallet copy-address <message-number|id-prefix>`"
                    .to_string(),
            );
        }
        return Ok(Some(ChatWalletComposerIntent::CopyAddress {
            message_reference: reference.to_string(),
        }));
    }
    if let Some(reference) = rest.strip_prefix("status ") {
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(
                "Wallet status syntax is `wallet status <message-number|id-prefix>`".to_string(),
            );
        }
        return Ok(Some(ChatWalletComposerIntent::InspectPaymentStatus {
            message_reference: reference.to_string(),
        }));
    }
    Err(
        "Wallet commands: `wallet pay <#|id>`, `wallet request <#|id> [description]`, `wallet copy-address <#|id>`, `wallet status <#|id>`"
            .to_string(),
    )
}

fn parse_chat_spacetime_intent(
    prompt: &str,
) -> Result<Option<ChatSpacetimeComposerIntent>, String> {
    let trimmed = prompt.trim();
    let Some(rest) = trimmed.strip_prefix("/search") else {
        return Ok(None);
    };
    let query = rest.trim();
    if query.is_empty() {
        return Err("Search syntax is `/search <text>`".to_string());
    }
    Ok(Some(ChatSpacetimeComposerIntent::Search {
        query: query.to_string(),
    }))
}

fn run_chat_spacetime_action(
    state: &mut crate::app_state::RenderState,
    intent: ChatSpacetimeComposerIntent,
) -> bool {
    match intent {
        ChatSpacetimeComposerIntent::Search { query } => {
            let result = crate::chat_spacetime::search_active_chat_messages(
                &state.autopilot_chat,
                &query,
                &state.spacetime_presence_snapshot,
            );
            let summary = if result.hit_count == 0 {
                format!(
                    "Search ({}) found no matches for `{query}`",
                    result.source_tag
                )
            } else {
                let preview = result
                    .hits
                    .iter()
                    .take(3)
                    .map(|hit| {
                        format!(
                            "{}{} {}",
                            hit.reference_label,
                            if hit.unread { " unread" } else { "" },
                            hit.preview
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  •  ");
                format!(
                    "Search ({}) {} hit(s): {}",
                    result.source_tag, result.hit_count, preview
                )
            };
            state.chat_inputs.composer.set_value(String::new());
            state.autopilot_chat.last_error = None;
            state
                .autopilot_chat
                .set_copy_notice(std::time::Instant::now(), summary);
            true
        }
    }
}

fn run_chat_wallet_action(
    state: &mut crate::app_state::RenderState,
    intent: ChatWalletComposerIntent,
) -> bool {
    let message_reference = match &intent {
        ChatWalletComposerIntent::PayInvoice { message_reference }
        | ChatWalletComposerIntent::RequestInvoice {
            message_reference, ..
        }
        | ChatWalletComposerIntent::CopyAddress { message_reference }
        | ChatWalletComposerIntent::InspectPaymentStatus { message_reference } => {
            message_reference.as_str()
        }
    };
    let Some(source) = resolve_active_chat_wallet_message(&state.autopilot_chat, message_reference)
    else {
        state.autopilot_chat.last_error = Some(format!(
            "Unknown chat message reference for wallet action: {message_reference}"
        ));
        return true;
    };
    let payload = extract_chat_wallet_payload(&source.content);

    match intent {
        ChatWalletComposerIntent::PayInvoice { .. } => {
            let Some(payment_request) = payload.payment_request.as_deref() else {
                state.autopilot_chat.last_error = Some(format!(
                    "{} does not contain a Lightning payment request.",
                    source.reference_label
                ));
                return true;
            };
            let payment_request = match validate_lightning_payment_request(payment_request) {
                Ok(payment_request) => payment_request,
                Err(error) => {
                    state.autopilot_chat.last_error = Some(error);
                    return true;
                }
            };
            focus_or_create_pane_kind(state, crate::app_state::PaneKind::SparkPayInvoice);
            state.chat_inputs.composer.set_value(String::new());
            state
                .pay_invoice_inputs
                .payment_request
                .set_value(payment_request.clone());
            if chat_wallet_request_supports_amount_override(&payment_request) {
                state.pay_invoice_inputs.amount_sats.set_value(
                    payload
                        .amount_sats
                        .map_or_else(String::new, |value| value.to_string()),
                );
            } else {
                state
                    .pay_invoice_inputs
                    .amount_sats
                    .set_value(String::new());
            }
            state.autopilot_chat.last_error = None;
            state.autopilot_chat.set_copy_notice(
                std::time::Instant::now(),
                format!("Prepared Spark pay pane from {}", source.reference_label),
            );
            true
        }
        ChatWalletComposerIntent::RequestInvoice { description, .. } => {
            let amount_sats = payload
                .amount_sats
                .unwrap_or(state.settings.document.wallet_default_send_sats);
            let description = description
                .and_then(|value| normalize_optional_text(&value))
                .or_else(|| payload.description.clone())
                .unwrap_or_else(|| format!("Chat invoice from {}", source.reference_label));
            focus_or_create_pane_kind(state, crate::app_state::PaneKind::SparkCreateInvoice);
            state.chat_inputs.composer.set_value(String::new());
            state
                .create_invoice_inputs
                .amount_sats
                .set_value(amount_sats.to_string());
            state
                .create_invoice_inputs
                .description
                .set_value(description.clone());
            if state
                .create_invoice_inputs
                .expiry_seconds
                .get_value()
                .trim()
                .is_empty()
            {
                state
                    .create_invoice_inputs
                    .expiry_seconds
                    .set_value("3600".to_string());
            }
            state.autopilot_chat.last_error = None;
            state.autopilot_chat.set_copy_notice(
                std::time::Instant::now(),
                format!(
                    "Prepared Spark invoice pane from {} for {}",
                    source.reference_label,
                    format_sats_amount(amount_sats)
                ),
            );
            true
        }
        ChatWalletComposerIntent::CopyAddress { .. } => {
            let Some(address) = payload.copy_address.as_deref() else {
                state.autopilot_chat.last_error = Some(format!(
                    "{} does not contain a copyable receive address.",
                    source.reference_label
                ));
                return true;
            };
            match copy_to_clipboard(address) {
                Ok(()) => {
                    state.chat_inputs.composer.set_value(String::new());
                    state.autopilot_chat.last_error = None;
                    state.autopilot_chat.set_copy_notice(
                        std::time::Instant::now(),
                        format!(
                            "Copied {} address from {}",
                            payload.copy_address_label.unwrap_or("wallet"),
                            source.reference_label
                        ),
                    );
                }
                Err(error) => {
                    state.autopilot_chat.last_error =
                        Some(format!("Failed to copy wallet address: {error}"));
                }
            }
            true
        }
        ChatWalletComposerIntent::InspectPaymentStatus { .. } => {
            let Some(summary) = chat_wallet_payment_status_summary(&payload, &state.spark_wallet)
            else {
                state.autopilot_chat.last_error = Some(format!(
                    "{} does not include payment metadata that Spark can inspect yet.",
                    source.reference_label
                ));
                return true;
            };
            focus_or_create_pane_kind(state, crate::app_state::PaneKind::SparkWallet);
            state.chat_inputs.composer.set_value(String::new());
            queue_spark_command(state, SparkWalletCommand::Refresh);
            state.autopilot_chat.last_error = None;
            state
                .autopilot_chat
                .set_copy_notice(std::time::Instant::now(), summary);
            true
        }
    }
}

fn parse_managed_chat_composer_intent(prompt: &str) -> Result<ManagedChatComposerIntent, String> {
    let trimmed = prompt.trim();
    if let Some(rest) = trimmed.strip_prefix("reply ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let reference = parts
            .next()
            .map(str::trim)
            .filter(|reference| !reference.is_empty())
            .ok_or_else(|| {
                "Reply syntax is `reply <message-number|id-prefix> <text>`".to_string()
            })?;
        let content = parts
            .next()
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                "Reply syntax is `reply <message-number|id-prefix> <text>`".to_string()
            })?;
        return Ok(ManagedChatComposerIntent::ChannelMessage {
            content: content.to_string(),
            reply_reference: Some(reference.to_string()),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("react ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let reference = parts
            .next()
            .map(str::trim)
            .filter(|reference| !reference.is_empty())
            .ok_or_else(|| {
                "Reaction syntax is `react <message-number|id-prefix> <emoji>`".to_string()
            })?;
        let reaction = parts
            .next()
            .map(str::trim)
            .filter(|reaction| !reaction.is_empty())
            .unwrap_or("+");
        return Ok(ManagedChatComposerIntent::Reaction {
            message_reference: reference.to_string(),
            reaction: reaction.to_string(),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("delete ") {
        let (message_reference, reason) = parse_reference_with_optional_reason(
            rest,
            "Delete syntax is `delete <message-number|id-prefix> [reason]`",
        )?;
        return Ok(ManagedChatComposerIntent::DeleteMessage {
            message_reference,
            reason,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("remove ") {
        let (member_reference, reason) = parse_reference_with_optional_reason(
            rest,
            "Remove syntax is `remove <member-pubkey-prefix> [reason]`",
        )?;
        return Ok(ManagedChatComposerIntent::RemoveUser {
            member_reference,
            reason,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("invite ") {
        let (code, reason) =
            parse_code_with_optional_reason(rest, "Invite syntax is `invite <code> [reason]`")?;
        return Ok(ManagedChatComposerIntent::Invite { code, reason });
    }
    if trimmed == "join" || trimmed.starts_with("join ") {
        let (invite_code, reason) =
            parse_join_command(trimmed.strip_prefix("join").unwrap_or_default())?;
        return Ok(ManagedChatComposerIntent::Join {
            invite_code,
            reason,
        });
    }
    if trimmed == "leave" || trimmed.starts_with("leave ") {
        return Ok(ManagedChatComposerIntent::Leave {
            reason: trimmed
                .strip_prefix("leave")
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .map(ToString::to_string),
        });
    }
    if let Some(rest) = trimmed
        .strip_prefix("meta ")
        .or_else(|| trimmed.strip_prefix("metadata "))
    {
        let (changes, summary) = parse_managed_chat_metadata_changes(rest)?;
        return Ok(ManagedChatComposerIntent::EditMetadata { changes, summary });
    }
    if let Some(rest) = trimmed.strip_prefix("mute ") {
        let reference = rest.trim();
        if reference.is_empty() {
            return Err("Mute syntax is `mute <member-pubkey-prefix>`".to_string());
        }
        return Ok(ManagedChatComposerIntent::MuteMember {
            member_reference: reference.to_string(),
            muted: true,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("unmute ") {
        let reference = rest.trim();
        if reference.is_empty() {
            return Err("Unmute syntax is `unmute <member-pubkey-prefix>`".to_string());
        }
        return Ok(ManagedChatComposerIntent::MuteMember {
            member_reference: reference.to_string(),
            muted: false,
        });
    }
    Ok(ManagedChatComposerIntent::ChannelMessage {
        content: trimmed.to_string(),
        reply_reference: None,
    })
}

fn resolve_active_chat_wallet_message(
    autopilot_chat: &crate::app_state::AutopilotChatState,
    reference: &str,
) -> Option<ChatWalletMessageSource> {
    match autopilot_chat.chat_browse_mode() {
        crate::app_state::ChatBrowseMode::Managed => {
            let message = resolve_managed_chat_message_reference(autopilot_chat, reference)?;
            let position = autopilot_chat
                .active_managed_chat_messages()
                .into_iter()
                .position(|candidate| candidate.event_id == message.event_id)
                .map(|index| index + 1)
                .unwrap_or(0);
            Some(ChatWalletMessageSource {
                reference_label: format!("#{}", position.max(1)),
                message_id: message.event_id.clone(),
                content: message.content.clone(),
            })
        }
        crate::app_state::ChatBrowseMode::DirectMessages => {
            let message = resolve_direct_message_reference(autopilot_chat, reference)?;
            let position = autopilot_chat
                .active_direct_message_messages()
                .into_iter()
                .position(|candidate| candidate.message_id == message.message_id)
                .map(|index| index + 1)
                .unwrap_or(0);
            Some(ChatWalletMessageSource {
                reference_label: format!("#{}", position.max(1)),
                message_id: message.message_id.clone(),
                content: message.content.clone(),
            })
        }
        crate::app_state::ChatBrowseMode::Autopilot => None,
    }
}

fn extract_chat_wallet_payload(content: &str) -> ChatWalletMessagePayload {
    let mut payload = ChatWalletMessagePayload::default();

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content.trim())
        && let Some(object) = value.as_object()
    {
        if let Some(candidate) = ["payment_request", "invoice", "bolt11"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        {
            if looks_like_lightning_payment_request(candidate) {
                payload.payment_request = Some(candidate.trim().to_string());
            } else if let Some(address) = extract_copyable_wallet_address(candidate) {
                payload.copy_address = Some(address.0);
                payload.copy_address_label = Some(address.1);
            }
        }
        payload.payment_id = ["payment_id", "id", "request_id"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        payload.chat_reported_status = ["status", "payment_status"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        payload.amount_sats = ["amount_sats", "amount_sat", "amount"]
            .into_iter()
            .find_map(|key| object.get(key))
            .and_then(parse_chat_wallet_amount);
        payload.description = ["description", "memo", "note", "message"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
            .and_then(normalize_optional_text);

        if payload.copy_address.is_none()
            && let Some(candidate) = [
                "bitcoin_address",
                "btc_address",
                "onchain_address",
                "spark_address",
                "address",
            ]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
            && let Some((address, label)) = extract_copyable_wallet_address(candidate)
        {
            payload.copy_address = Some(address);
            payload.copy_address_label = Some(label);
        }
    }

    for token in content.split_whitespace() {
        let token = trim_chat_wallet_token(token);
        if token.is_empty() {
            continue;
        }
        if payload.payment_request.is_none() && looks_like_lightning_payment_request(token) {
            payload.payment_request = Some(token.to_string());
            continue;
        }
        if payload.copy_address.is_none()
            && let Some((address, label)) = extract_copyable_wallet_address(token)
        {
            payload.copy_address = Some(address);
            payload.copy_address_label = Some(label);
        }
    }

    payload
}

fn trim_chat_wallet_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
        )
    })
}

fn looks_like_lightning_payment_request(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.starts_with("ln")
        || normalized.starts_with("lightning:ln")
        || normalized.starts_with("lightning://ln")
}

fn extract_copyable_wallet_address(value: &str) -> Option<(String, &'static str)> {
    let trimmed = trim_chat_wallet_token(value).trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    if let Some(uri) = normalized.strip_prefix("bitcoin:") {
        let raw_address = uri.split('?').next().unwrap_or(uri).trim();
        if raw_address.is_empty() {
            return None;
        }
        let original_address = trimmed
            .trim_start_matches("bitcoin:")
            .split('?')
            .next()
            .unwrap_or(trimmed)
            .trim();
        return Some((original_address.to_string(), "bitcoin"));
    }

    if normalized.starts_with("bc1")
        || normalized.starts_with("tb1")
        || normalized.starts_with("bcrt1")
        || (normalized.starts_with('1') && trimmed.len() >= 26)
        || (normalized.starts_with('3') && trimmed.len() >= 26)
    {
        return Some((trimmed.to_string(), "bitcoin"));
    }

    if trimmed.contains('@') {
        return Some((trimmed.to_string(), "spark"));
    }

    None
}

fn parse_chat_wallet_amount(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(raw) => raw
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0),
        _ => None,
    }
}

fn focus_or_create_pane_kind(
    state: &mut crate::app_state::RenderState,
    kind: crate::app_state::PaneKind,
) {
    if let Some(pane_id) = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.id)
    {
        crate::pane_system::PaneController::bring_to_front(state, pane_id);
    } else {
        crate::pane_system::PaneController::create_for_kind(state, kind);
    }
}

fn chat_wallet_request_supports_amount_override(payment_request: &str) -> bool {
    let normalized = payment_request.trim().to_ascii_lowercase();
    normalized.starts_with("lnurl1")
        || normalized.starts_with("lightning:lnurl1")
        || normalized.starts_with("lightning://lnurl1")
}

fn compact_chat_wallet_identifier(value: &str, prefix_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    if trimmed.chars().count() <= prefix_chars {
        return trimmed.to_string();
    }
    format!(
        "{}…",
        trimmed.chars().take(prefix_chars).collect::<String>()
    )
}

fn chat_wallet_payment_status_summary(
    payload: &ChatWalletMessagePayload,
    spark_wallet: &crate::spark_wallet::SparkPaneState,
) -> Option<String> {
    if let Some(payment_id) = payload.payment_id.as_deref() {
        if let Some(payment) = spark_wallet
            .recent_payments
            .iter()
            .find(|candidate| candidate.id == payment_id)
        {
            let mut summary = format!(
                "Spark confirms in the current wallet snapshot: {} {} {} ({})",
                payment.direction,
                payment.status,
                compact_chat_wallet_identifier(payment.id.as_str(), 10),
                format_sats_amount(payment.amount_sats)
            );
            if let Some(reported_status) = payload
                .chat_reported_status
                .as_deref()
                .filter(|reported| !reported.eq_ignore_ascii_case(payment.status.as_str()))
            {
                summary.push_str(format!("; chat reported {reported_status}").as_str());
            }
            return Some(summary);
        }

        if let Some(reported_status) = payload.chat_reported_status.as_deref() {
            return Some(format!(
                "Chat reports {} for payment {}, but Spark has not confirmed it in the current wallet snapshot.",
                reported_status,
                compact_chat_wallet_identifier(payment_id, 10)
            ));
        }

        return Some(format!(
            "Message references payment {}, but Spark has no matching payment in the current wallet snapshot yet.",
            compact_chat_wallet_identifier(payment_id, 10)
        ));
    }

    if let Some(reported_status) = payload.chat_reported_status.as_deref() {
        return Some(format!(
            "Chat reports {}, but Spark needs a wallet payment id before settlement can be confirmed.",
            reported_status
        ));
    }

    payload.payment_request.as_ref().map(|_| {
        "Message contains a payment request, but Spark needs a wallet payment id before settlement can be confirmed."
            .to_string()
    })
}

fn parse_reference_with_optional_reason(
    raw: &str,
    syntax: &str,
) -> Result<(String, Option<String>), String> {
    let mut parts = raw.trim().splitn(2, char::is_whitespace);
    let reference = parts
        .next()
        .map(str::trim)
        .filter(|reference| !reference.is_empty())
        .ok_or_else(|| syntax.to_string())?;
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(ToString::to_string);
    Ok((reference.to_string(), reason))
}

fn parse_code_with_optional_reason(
    raw: &str,
    syntax: &str,
) -> Result<(String, Option<String>), String> {
    let mut parts = raw.trim().splitn(2, char::is_whitespace);
    let code = parts
        .next()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .ok_or_else(|| syntax.to_string())?;
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(ToString::to_string);
    Ok((code.to_string(), reason))
}

fn parse_join_command(raw: &str) -> Result<(Option<String>, Option<String>), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok((None, None));
    }
    let mut parts = trimmed.splitn(2, '|').map(str::trim);
    let left = parts.next().unwrap_or_default();
    let right = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let invite_code = if left.is_empty() {
        None
    } else if left.contains(char::is_whitespace) {
        return Ok((None, Some(left.to_string())));
    } else {
        Some(left.to_string())
    };
    let reason = right
        .map(ToString::to_string)
        .or_else(|| (invite_code.is_none() && !left.is_empty()).then(|| left.to_string()));
    Ok((invite_code, reason))
}

fn parse_managed_chat_metadata_changes(raw: &str) -> Result<(Vec<Vec<String>>, String), String> {
    let mut changes = Vec::new();
    let mut summaries = Vec::new();
    for segment in raw.split('|') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, '=');
        let key = parts
            .next()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| {
                "Metadata syntax is `meta name=<...> | about=<...> | private=<true|false> | restricted=<true|false> | hidden=<true|false> | closed=<true|false> | picture=<url>`".to_string()
            })?
            .to_ascii_lowercase();
        let value = parts
            .next()
            .map(str::trim)
            .ok_or_else(|| {
                "Metadata syntax is `meta name=<...> | about=<...> | private=<true|false> | restricted=<true|false> | hidden=<true|false> | closed=<true|false> | picture=<url>`".to_string()
            })?;
        match key.as_str() {
            "name" | "about" | "picture" => {
                changes.push(vec![key.clone(), value.to_string()]);
                summaries.push(format!("{key}={value}"));
            }
            "private" | "restricted" | "hidden" | "closed" => {
                let enabled = parse_bool_flag(value).ok_or_else(|| {
                    format!("Metadata flag `{key}` expects true/false, on/off, or yes/no.")
                })?;
                let tag = match (key.as_str(), enabled) {
                    ("private", true) => "private",
                    ("private", false) => "public",
                    ("restricted", true) => "restricted",
                    ("restricted", false) => "unrestricted",
                    ("hidden", true) => "hidden",
                    ("hidden", false) => "visible",
                    ("closed", true) => "closed",
                    ("closed", false) => "open",
                    _ => unreachable!(),
                };
                changes.push(vec![tag.to_string()]);
                summaries.push(format!("{key}={enabled}"));
            }
            _ => {
                return Err(format!(
                    "Unsupported metadata field `{key}`. Use name, about, picture, private, restricted, hidden, or closed."
                ));
            }
        }
    }
    if changes.is_empty() {
        return Err(
            "Metadata syntax is `meta name=<...> | about=<...> | private=<true|false> | restricted=<true|false> | hidden=<true|false> | closed=<true|false> | picture=<url>`".to_string(),
        );
    }
    Ok((changes, summaries.join(" | ")))
}

fn parse_bool_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn resolve_managed_chat_member_reference<'a>(
    group: &'a crate::app_state::ManagedChatGroupProjection,
    reference: &str,
) -> Option<&'a crate::app_state::ManagedChatMemberProjection> {
    let normalized = reference
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('#')
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let matches = group
        .members
        .iter()
        .filter(|member| member.pubkey.starts_with(&normalized))
        .collect::<Vec<_>>();
    (matches.len() == 1).then_some(matches[0])
}

fn compact_member_reference(pubkey: &str) -> String {
    let trimmed = pubkey.trim();
    if trimmed.len() <= 8 {
        trimmed.to_string()
    } else {
        trimmed.chars().take(8).collect()
    }
}

fn resolve_managed_chat_message_reference<'a>(
    autopilot_chat: &'a crate::app_state::AutopilotChatState,
    reference: &str,
) -> Option<&'a crate::app_state::ManagedChatMessageProjection> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }

    let active_messages = autopilot_chat.active_managed_chat_messages();
    if let Some(index_ref) = reference.strip_prefix('#').or(Some(reference))
        && let Ok(index) = index_ref.parse::<usize>()
        && index > 0
    {
        return active_messages.get(index - 1).copied();
    }

    let normalized = reference.trim_start_matches('#').to_ascii_lowercase();
    active_messages
        .into_iter()
        .find(|message| message.event_id.starts_with(&normalized))
}

fn resolve_managed_chat_mentions(
    group: &crate::app_state::ManagedChatGroupProjection,
    content: &str,
) -> Vec<nostr::ManagedChatMention> {
    let mut matched_pubkeys = Vec::new();
    for token in content.split_whitespace() {
        let Some(prefix) = parse_managed_chat_mention_prefix(token) else {
            continue;
        };
        let matches = group
            .members
            .iter()
            .map(|member| member.pubkey.as_str())
            .filter(|pubkey| pubkey.starts_with(&prefix))
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            matched_pubkeys.push(matches[0].to_string());
        }
    }
    matched_pubkeys.sort();
    matched_pubkeys.dedup();
    matched_pubkeys
        .into_iter()
        .filter_map(|pubkey| nostr::ManagedChatMention::new(pubkey).ok())
        .collect()
}

fn parse_managed_chat_mention_prefix(token: &str) -> Option<String> {
    let mention = token.strip_prefix('@')?;
    let normalized = mention
        .trim_matches(|ch: char| !ch.is_ascii_hexdigit())
        .to_ascii_lowercase();
    (normalized.len() >= 4).then_some(normalized)
}

fn build_managed_chat_reaction_event(
    identity: &nostr::NostrIdentity,
    group_id: &str,
    channel: &crate::app_state::ManagedChatChannelProjection,
    target_message: &crate::app_state::ManagedChatMessageProjection,
    reaction: &str,
) -> Result<nostr::Event, String> {
    let relay_url = channel.relay_url.as_deref().ok_or_else(|| {
        format!(
            "Managed chat channel {} does not advertise a relay target yet.",
            channel.channel_id
        )
    })?;
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let secret_key_bytes = hex::decode(&identity.private_key_hex)
        .map_err(|error| format!("Invalid Nostr private key hex: {error}"))?;
    let secret_key = secret_key_bytes
        .try_into()
        .map_err(|_| "Invalid Nostr private key length".to_string())?;
    nostr::finalize_event(
        &nostr::EventTemplate {
            created_at,
            kind: 7,
            tags: vec![
                vec!["h".to_string(), group_id.to_string()],
                vec![
                    "e".to_string(),
                    target_message.event_id.clone(),
                    relay_url.to_string(),
                ],
                vec![
                    "p".to_string(),
                    target_message.author_pubkey.clone(),
                    relay_url.to_string(),
                ],
                vec!["k".to_string(), "42".to_string()],
                vec!["nonce".to_string(), now_ms.to_string()],
            ],
            content: reaction.trim().to_string(),
        },
        &secret_key,
    )
    .map_err(|error| format!("Failed to sign managed chat reaction: {error}"))
}

fn build_managed_chat_outbound_message(
    identity: &nostr::NostrIdentity,
    group_id: &str,
    channel: &crate::app_state::ManagedChatChannelProjection,
    relay_url_override: Option<&str>,
    content: &str,
    reply_target: Option<&crate::app_state::ManagedChatMessageProjection>,
    mentions: Vec<nostr::ManagedChatMention>,
) -> Result<crate::app_state::ManagedChatOutboundMessage, String> {
    let relay_url = relay_url_override
        .or(channel.relay_url.as_deref())
        .ok_or_else(|| {
            format!(
                "Managed chat channel {} does not advertise a relay target yet.",
                channel.channel_id
            )
        })?;
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let mut message_event = nostr::ManagedChannelMessageEvent::new(
        group_id,
        &channel.channel_id,
        relay_url,
        content,
        created_at,
    )
    .map_err(|error| format!("Failed to build managed chat message: {error}"))?;
    if let Some(reply_target) = reply_target {
        let reply = nostr::ManagedMessageReply::new(
            &reply_target.event_id,
            relay_url,
            &reply_target.author_pubkey,
        )
        .map_err(|error| format!("Failed to build managed chat reply target: {error}"))?;
        message_event = message_event.with_reply(reply);
    }
    if !mentions.is_empty() {
        message_event = message_event.with_mentions(mentions);
    }

    let mut tags = message_event
        .to_tags()
        .map_err(|error| format!("Failed to encode managed chat message tags: {error}"))?;
    tags.push(vec!["nonce".to_string(), now_ms.to_string()]);

    let secret_key_bytes = hex::decode(&identity.private_key_hex)
        .map_err(|error| format!("Invalid Nostr private key hex: {error}"))?;
    let secret_key = secret_key_bytes
        .try_into()
        .map_err(|_| "Invalid Nostr private key length".to_string())?;
    let event = nostr::finalize_event(
        &nostr::EventTemplate {
            created_at,
            kind: 42,
            tags,
            content: content.to_string(),
        },
        &secret_key,
    )
    .map_err(|error| format!("Failed to sign managed chat message: {error}"))?;

    Ok(crate::app_state::ManagedChatOutboundMessage {
        group_id: group_id.to_string(),
        channel_id: channel.channel_id.clone(),
        relay_url: relay_url.to_string(),
        event,
        delivery_state: crate::app_state::ManagedChatDeliveryState::Publishing,
        attempt_count: 1,
        last_error: None,
    })
}

pub(crate) fn queue_managed_chat_channel_message(
    chat: &mut crate::app_state::AutopilotChatState,
    identity: &nostr::NostrIdentity,
    content: &str,
    reply_event_id: Option<&str>,
) -> Result<String, String> {
    let Some(channel) = chat.active_managed_chat_channel().cloned() else {
        return Err("No managed chat channel is selected.".to_string());
    };
    let Some(group) = chat.active_managed_chat_group().cloned() else {
        return Err("No managed chat group is selected.".to_string());
    };
    queue_managed_chat_message_to_channel(
        chat,
        identity,
        group.group_id.as_str(),
        channel.channel_id.as_str(),
        content,
        reply_event_id,
    )
}

pub(crate) fn queue_managed_chat_message_to_channel(
    chat: &mut crate::app_state::AutopilotChatState,
    identity: &nostr::NostrIdentity,
    group_id: &str,
    channel_id: &str,
    content: &str,
    reply_event_id: Option<&str>,
) -> Result<String, String> {
    queue_managed_chat_message_to_channel_with_relay(
        chat,
        identity,
        group_id,
        channel_id,
        None,
        content,
        reply_event_id,
    )
}

pub(crate) fn queue_managed_chat_message_to_channel_with_relay(
    chat: &mut crate::app_state::AutopilotChatState,
    identity: &nostr::NostrIdentity,
    group_id: &str,
    channel_id: &str,
    relay_url_override: Option<&str>,
    content: &str,
    reply_event_id: Option<&str>,
) -> Result<String, String> {
    let Some(channel) = chat
        .managed_chat_projection
        .snapshot
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .cloned()
    else {
        return Err(format!("Unknown managed chat channel: {channel_id}"));
    };
    if channel.group_id != group_id {
        return Err(format!(
            "Managed chat channel {} does not belong to group {}",
            channel_id, group_id
        ));
    }
    let Some(group) = chat
        .managed_chat_projection
        .snapshot
        .groups
        .iter()
        .find(|group| group.group_id == group_id)
        .cloned()
    else {
        return Err(format!("Unknown managed chat group: {group_id}"));
    };
    let reply_target = match reply_event_id {
        Some(event_id) => {
            let Some(target_message) = chat.managed_chat_projection.snapshot.messages.get(event_id)
            else {
                return Err(format!(
                    "Unknown managed chat reply target event: {event_id}"
                ));
            };
            if target_message.group_id != group.group_id {
                return Err(format!(
                    "Managed chat reply target {} is not in the active group {}",
                    event_id, group.group_id
                ));
            }
            if target_message.channel_id != channel.channel_id {
                return Err(format!(
                    "Managed chat reply target {} is not in the active channel {}",
                    event_id, channel.channel_id
                ));
            }
            Some(target_message)
        }
        None => None,
    };
    let mentions = resolve_managed_chat_mentions(&group, content);
    let outbound_message = build_managed_chat_outbound_message(
        identity,
        &group.group_id,
        &channel,
        relay_url_override,
        content,
        reply_target,
        mentions,
    )?;
    let event_id = outbound_message.event.id.clone();
    chat.managed_chat_projection
        .queue_outbound_message(outbound_message)?;
    chat.last_error = None;
    chat.reset_transcript_scroll();
    Ok(event_id)
}

fn finalize_signed_nostr_event(
    identity: &nostr::NostrIdentity,
    kind: u16,
    created_at: u64,
    tags: Vec<Vec<String>>,
    content: String,
) -> Result<nostr::Event, String> {
    let secret_key_bytes = hex::decode(&identity.private_key_hex)
        .map_err(|error| format!("Invalid Nostr private key hex: {error}"))?;
    let secret_key = secret_key_bytes
        .try_into()
        .map_err(|_| "Invalid Nostr private key length".to_string())?;
    nostr::finalize_event(
        &nostr::EventTemplate {
            created_at,
            kind,
            tags,
            content,
        },
        &secret_key,
    )
    .map_err(|error| format!("Failed to sign managed chat control event: {error}"))
}

fn build_managed_chat_moderation_event(
    identity: &nostr::NostrIdentity,
    group_id: &str,
    action: nostr::ModerationAction,
    reason: Option<&str>,
) -> Result<nostr::Event, String> {
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let mut event = nostr::ModerationEvent::new(group_id, action, created_at)
        .map_err(|error| format!("Failed to build managed chat moderation event: {error}"))?;
    if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        event = event.with_reason(reason);
    }
    let mut tags = event
        .to_tags()
        .map_err(|error| format!("Failed to encode managed chat moderation tags: {error}"))?;
    tags.push(vec!["nonce".to_string(), now_ms.to_string()]);
    finalize_signed_nostr_event(
        identity,
        event.kind(),
        created_at,
        tags,
        event.reason.unwrap_or_default(),
    )
}

fn build_managed_chat_join_request_event(
    identity: &nostr::NostrIdentity,
    group_id: &str,
    invite_code: Option<&str>,
    reason: Option<&str>,
) -> Result<nostr::Event, String> {
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let mut event = nostr::JoinRequestEvent::new(group_id, created_at)
        .map_err(|error| format!("Failed to build join request: {error}"))?;
    if let Some(code) = invite_code.map(str::trim).filter(|code| !code.is_empty()) {
        event = event.with_invite_code(code);
    }
    if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        event = event.with_reason(reason);
    }
    let mut tags = event.to_tags();
    tags.push(vec!["nonce".to_string(), now_ms.to_string()]);
    finalize_signed_nostr_event(
        identity,
        nostr::nip29::KIND_JOIN_REQUEST,
        created_at,
        tags,
        event.reason.unwrap_or_default(),
    )
}

fn build_managed_chat_leave_request_event(
    identity: &nostr::NostrIdentity,
    group_id: &str,
    reason: Option<&str>,
) -> Result<nostr::Event, String> {
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let mut event = nostr::LeaveRequestEvent::new(group_id, created_at)
        .map_err(|error| format!("Failed to build leave request: {error}"))?;
    if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        event = event.with_reason(reason);
    }
    let mut tags = event.to_tags();
    tags.push(vec!["nonce".to_string(), now_ms.to_string()]);
    finalize_signed_nostr_event(
        identity,
        nostr::nip29::KIND_LEAVE_REQUEST,
        created_at,
        tags,
        event.reason.unwrap_or_default(),
    )
}

fn parse_direct_message_creation_intent(
    prompt: &str,
) -> Result<Option<DirectMessageComposerIntent>, String> {
    let trimmed = prompt.trim();
    if let Some(rest) = trimmed.strip_prefix("dm ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let recipient_pubkey = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "DM syntax is `dm <recipient-pubkey-hex> <text>`".to_string())?;
        let content = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "DM syntax is `dm <recipient-pubkey-hex> <text>`".to_string())?;
        return Ok(Some(DirectMessageComposerIntent::CreateRoom {
            participant_pubkeys: vec![normalize_direct_message_recipient_pubkey(recipient_pubkey)?],
            subject: None,
            content: content.to_string(),
        }));
    }
    if let Some(rest) = trimmed.strip_prefix("room ") {
        let parts = rest.splitn(3, '|').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
            return Err(
                "Room syntax is `room <pubkey[,pubkey...]> | <subject> | <text>`".to_string(),
            );
        }
        return Ok(Some(DirectMessageComposerIntent::CreateRoom {
            participant_pubkeys: parse_direct_message_participant_pubkeys(parts[0])?,
            subject: Some(parts[1].to_string()),
            content: parts[2].to_string(),
        }));
    }
    Ok(None)
}

fn parse_direct_message_room_intent(prompt: &str) -> Result<DirectMessageComposerIntent, String> {
    if let Some(intent) = parse_direct_message_creation_intent(prompt)? {
        return Ok(intent);
    }
    let trimmed = prompt.trim();
    if let Some(rest) = trimmed.strip_prefix("reply ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let reference = parts
            .next()
            .map(str::trim)
            .filter(|reference| !reference.is_empty())
            .ok_or_else(|| {
                "Reply syntax is `reply <message-number|id-prefix> <text>`".to_string()
            })?;
        let content = parts
            .next()
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                "Reply syntax is `reply <message-number|id-prefix> <text>`".to_string()
            })?;
        return Ok(DirectMessageComposerIntent::RoomMessage {
            content: content.to_string(),
            reply_reference: Some(reference.to_string()),
        });
    }
    Ok(DirectMessageComposerIntent::RoomMessage {
        content: trimmed.to_string(),
        reply_reference: None,
    })
}

fn parse_direct_message_participant_pubkeys(raw: &str) -> Result<Vec<String>, String> {
    let participants = raw
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_direct_message_recipient_pubkey)
        .collect::<Result<Vec<_>, _>>()?;
    if participants.is_empty() {
        return Err("Room syntax is `room <pubkey[,pubkey...]> | <subject> | <text>`".to_string());
    }
    Ok(participants)
}

fn normalize_direct_message_recipient_pubkey(pubkey: &str) -> Result<String, String> {
    let normalized = pubkey.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "Direct message recipient must be a 64-character hex pubkey: {pubkey}"
        ));
    }
    Ok(normalized)
}

fn resolve_direct_message_reference<'a>(
    autopilot_chat: &'a crate::app_state::AutopilotChatState,
    reference: &str,
) -> Option<&'a crate::app_state::DirectMessageMessageProjection> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }

    let active_messages = autopilot_chat.active_direct_message_messages();
    if let Some(index_ref) = reference.strip_prefix('#').or(Some(reference))
        && let Ok(index) = index_ref.parse::<usize>()
        && index > 0
    {
        return active_messages.get(index - 1).copied();
    }

    let normalized = reference.trim_start_matches('#').to_ascii_lowercase();
    active_messages
        .into_iter()
        .find(|message| message.message_id.starts_with(&normalized))
}

fn resolve_direct_message_recipient_relay_hints(
    state: &crate::app_state::RenderState,
    local_pubkey: &str,
    participant_pubkeys: &[String],
) -> std::collections::BTreeMap<String, Vec<String>> {
    let local_pubkey = local_pubkey.to_ascii_lowercase();
    let fallback_relays = state.configured_provider_relay_urls();
    participant_pubkeys
        .iter()
        .filter(|pubkey| **pubkey != local_pubkey)
        .map(|pubkey| {
            let relays = state
                .autopilot_chat
                .direct_message_projection
                .snapshot
                .relay_lists
                .get(pubkey.as_str())
                .cloned()
                .filter(|relays| !relays.is_empty())
                .unwrap_or_else(|| fallback_relays.clone());
            (pubkey.clone(), relays)
        })
        .collect()
}

fn build_direct_message_outbound_message(
    identity: &nostr::NostrIdentity,
    participant_pubkeys: Vec<String>,
    recipient_relay_hints: std::collections::BTreeMap<String, Vec<String>>,
    content: &str,
    reply_target: Option<&crate::app_state::DirectMessageMessageProjection>,
    subject: Option<String>,
) -> Result<crate::app_state::DirectMessageOutboundMessage, String> {
    let author_pubkey = normalize_direct_message_recipient_pubkey(&identity.public_key_hex)?;
    let mut all_participants = participant_pubkeys
        .into_iter()
        .map(|pubkey| normalize_direct_message_recipient_pubkey(pubkey.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    all_participants.push(author_pubkey.clone());
    all_participants.sort();
    all_participants.dedup();

    let recipient_pubkeys = all_participants
        .iter()
        .filter(|pubkey| **pubkey != author_pubkey)
        .cloned()
        .collect::<Vec<_>>();
    if recipient_pubkeys.is_empty() {
        return Err("Direct message room needs at least one remote participant.".to_string());
    }

    let normalized_subject = subject
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let now_ms = current_epoch_millis();
    let created_at = (now_ms / 1_000).max(1);
    let private_key = parse_nostr_private_key_hex(identity.private_key_hex.as_str())?;
    let mut message = nostr::nip17::ChatMessage::new(content);
    for recipient_pubkey in &recipient_pubkeys {
        let relay_hint = recipient_relay_hints
            .get(recipient_pubkey.as_str())
            .and_then(|relays| relays.first())
            .cloned();
        message = message.add_recipient(recipient_pubkey.clone(), relay_hint);
    }
    if let Some(reply_target) = reply_target {
        message = message.reply_to(reply_target.message_id.clone());
    }
    if let Some(subject) = normalized_subject.as_deref() {
        message = message.subject(subject);
    }
    let rumor = nostr::nip59::Rumor::new(message.to_unsigned_event(&author_pubkey, created_at))
        .map_err(|error| format!("Failed to encode direct message rumor: {error}"))?;
    let wrapped_events = recipient_pubkeys
        .iter()
        .map(|recipient_pubkey| {
            nostr::nip17::send_chat_message(&message, &private_key, recipient_pubkey, created_at)
                .map_err(|error| {
                    format!("Failed to gift wrap direct message for {recipient_pubkey}: {error}")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(crate::app_state::DirectMessageOutboundMessage {
        room_id: crate::app_state::direct_message_room_id(
            normalized_subject.as_deref(),
            &all_participants,
        ),
        message_id: rumor.id,
        author_pubkey,
        participant_pubkeys: all_participants,
        recipient_pubkeys,
        recipient_relay_hints,
        content: content.to_string(),
        created_at,
        reply_to_event_id: reply_target.map(|message| message.message_id.clone()),
        subject: normalized_subject,
        wrapped_events,
        delivery_state: crate::app_state::ManagedChatDeliveryState::Publishing,
        attempt_count: 1,
        last_error: None,
    })
}

fn log_chat_prompt_to_console(thread_id: &str, prompt: &str) {
    const MAX_CHARS: usize = 8_000;
    let trimmed = prompt.trim_end();
    if trimmed.is_empty() {
        return;
    }
    let chars = trimmed.chars().count();
    let (body, truncated) = if chars > MAX_CHARS {
        let body = trimmed.chars().take(MAX_CHARS).collect::<String>();
        (body, true)
    } else {
        (trimmed.to_string(), false)
    };
    if truncated {
        tracing::info!(
            "autopilot transcript/user thread_id={} chars={} (truncated to {})\n{}",
            thread_id,
            chars,
            MAX_CHARS,
            body
        );
    } else {
        tracing::info!(
            "autopilot transcript/user thread_id={} chars={}\n{}",
            thread_id,
            chars,
            body
        );
    }
}

pub(super) fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn sync_chat_composer_draft(state: &mut crate::app_state::RenderState) {
    if state.autopilot_chat.chat_browse_mode() != crate::app_state::ChatBrowseMode::Autopilot {
        return;
    }
    state
        .autopilot_chat
        .record_composer_draft(state.chat_inputs.composer.get_value().to_string());
}

pub(super) fn restore_chat_composer_draft(state: &mut crate::app_state::RenderState) {
    if state.autopilot_chat.chat_browse_mode() != crate::app_state::ChatBrowseMode::Autopilot {
        return;
    }
    state
        .chat_inputs
        .composer
        .set_value(state.autopilot_chat.active_composer_draft().to_string());
}

pub(super) fn restore_last_submission_draft(
    state: &mut crate::app_state::RenderState,
    thread_id: &str,
) {
    let Some(draft) = state
        .autopilot_chat
        .last_submission_draft(thread_id)
        .map(str::to_string)
    else {
        return;
    };
    if !state.autopilot_chat.is_active_thread(thread_id) {
        return;
    }
    if !state.chat_inputs.composer.get_value().trim().is_empty() {
        return;
    }
    state.chat_inputs.composer.set_value(draft.clone());
    state.autopilot_chat.record_composer_draft(draft);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum TurnSkillSource {
    UserSelected,
    GoalAutoSelected,
    PolicyRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TurnSkillAttachment {
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub source: TurnSkillSource,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ParsedChatTurnPrompt {
    pub prompt_text: String,
    pub mention_attachments: Vec<TurnMentionAttachment>,
    pub image_attachments: Vec<TurnImageAttachment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TurnMentionAttachment {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TurnImageAttachment {
    Remote { url: String },
    Local { path: std::path::PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GoalTurnSkillCandidate {
    pub attachment: TurnSkillAttachment,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GoalTurnSkillSelection {
    pub goal_id: String,
    pub objective_tag: String,
    pub candidates: Vec<GoalTurnSkillCandidate>,
}

pub(super) fn resolve_turn_skill_by_name(
    discovered_skills: &[crate::app_state::SkillRegistryDiscoveredSkill],
    skill_name: &str,
    source: TurnSkillSource,
) -> Result<TurnSkillAttachment, String> {
    let trimmed = skill_name.trim();
    let Some(skill) = discovered_skills
        .iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(trimmed))
    else {
        return Err(format!(
            "Skill '{}' is not available in this workspace.",
            trimmed
        ));
    };
    Ok(TurnSkillAttachment {
        name: skill.name.clone(),
        path: skill.path.clone(),
        enabled: skill.enabled,
        source,
    })
}

pub(super) fn resolve_turn_skill_by_path(
    discovered_skills: &[crate::app_state::SkillRegistryDiscoveredSkill],
    skill_path: &str,
    source: TurnSkillSource,
) -> Result<TurnSkillAttachment, String> {
    let trimmed = skill_path.trim();
    let Some(skill) = discovered_skills.iter().find(|skill| skill.path == trimmed) else {
        return Err(format!(
            "Skill path '{}' is not available in this workspace.",
            trimmed
        ));
    };
    Ok(TurnSkillAttachment {
        name: skill.name.clone(),
        path: skill.path.clone(),
        enabled: skill.enabled,
        source,
    })
}

pub(super) fn selected_skill_candidates_for_turn(
    state: &crate::app_state::RenderState,
) -> Vec<TurnSkillAttachment> {
    state
        .skill_registry
        .selected_skill_index
        .and_then(|index| state.skill_registry.discovered_skills.get(index))
        .map(|skill| TurnSkillAttachment {
            name: skill.name.clone(),
            path: skill.path.clone(),
            enabled: skill.enabled,
            source: TurnSkillSource::UserSelected,
        })
        .into_iter()
        .collect()
}

pub(super) fn cad_policy_skill_candidates_for_turn(
    is_cad_turn: bool,
    discovered_skills: &[crate::app_state::SkillRegistryDiscoveredSkill],
) -> Vec<TurnSkillAttachment> {
    if !is_cad_turn {
        return Vec::new();
    }

    let mut resolved = Vec::new();
    for skill_name in crate::skill_autoload::REQUIRED_CAD_POLICY_SKILLS {
        match resolve_turn_skill_by_name(
            discovered_skills,
            skill_name,
            TurnSkillSource::PolicyRequired,
        ) {
            Ok(skill) => resolved.push(skill),
            Err(_) => {}
        }
    }

    resolved
}

fn goal_priority_status_rank(
    status: crate::state::autopilot_goals::GoalLifecycleStatus,
) -> Option<u8> {
    match status {
        crate::state::autopilot_goals::GoalLifecycleStatus::Running => Some(0),
        crate::state::autopilot_goals::GoalLifecycleStatus::Queued => Some(1),
        _ => None,
    }
}

pub(super) fn goal_policy_skill_candidates_for_turn(
    state: &crate::app_state::RenderState,
) -> Option<GoalTurnSkillSelection> {
    let active_goal = state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .filter_map(|goal| {
            goal_priority_status_rank(goal.lifecycle_status).map(|rank| (rank, goal))
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.goal_id.cmp(&right.1.goal_id))
        })
        .map(|entry| entry.1)?;

    let resolution = state
        .autopilot_goals
        .resolve_skill_candidates_for_goal(
            &active_goal.goal_id,
            &state.skill_registry.discovered_skills,
        )
        .ok()?;
    if resolution.candidates.is_empty() {
        return None;
    }

    let mut dedupe = std::collections::HashSet::new();
    let candidates = resolution
        .candidates
        .into_iter()
        .filter(|candidate| {
            dedupe.insert(format!(
                "{}::{}",
                candidate.name.to_ascii_lowercase(),
                candidate.path
            ))
        })
        .map(|candidate| GoalTurnSkillCandidate {
            reason: candidate.reason,
            attachment: TurnSkillAttachment {
                name: candidate.name,
                path: candidate.path,
                enabled: true,
                source: TurnSkillSource::GoalAutoSelected,
            },
        })
        .collect::<Vec<_>>();

    Some(GoalTurnSkillSelection {
        goal_id: resolution.goal_id,
        objective_tag: resolution.objective_tag,
        candidates,
    })
}

pub(super) fn cad_turn_approval_policy(is_cad_turn: bool) -> Option<codex_client::AskForApproval> {
    let _ = is_cad_turn;
    // Force unsafe mode for all turns: never ask for approvals.
    Some(codex_client::AskForApproval::Never)
}

pub(super) fn dangerous_sandbox_policy() -> Option<codex_client::SandboxPolicy> {
    Some(codex_client::SandboxPolicy::DangerFullAccess)
}

pub(super) fn dangerous_sandbox_mode() -> Option<codex_client::SandboxMode> {
    Some(codex_client::SandboxMode::DangerFullAccess)
}

pub(super) fn current_chat_session_cwd(state: &crate::app_state::RenderState) -> Option<String> {
    goal_scoped_turn_cwd(state)
        .or_else(|| state.autopilot_chat.active_thread_cwd().map(str::to_string))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|value| value.into_os_string().into_string().ok())
        })
}

fn current_chat_workspace_root(state: &crate::app_state::RenderState) -> Option<String> {
    goal_scoped_turn_cwd(state)
        .or_else(|| {
            state
                .autopilot_chat
                .active_thread_workspace_root()
                .map(str::to_string)
        })
        .or_else(|| state.autopilot_chat.active_thread_cwd().map(str::to_string))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|value| value.into_os_string().into_string().ok())
        })
}

fn remember_chat_command_prompt(state: &mut crate::app_state::RenderState, prompt: &str) {
    if let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() {
        state
            .autopilot_chat
            .remember_submission_draft(&thread_id, prompt.to_string());
    }
}

fn clear_chat_command_prompt(state: &mut crate::app_state::RenderState) {
    state.chat_inputs.composer.set_value(String::new());
    state.autopilot_chat.record_composer_draft(String::new());
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ChatGitExecutionResult {
    response: String,
    thread_workspace_override: Option<String>,
}

fn parse_shell_like_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match quote {
            Some(active_quote) => match ch {
                '\\' if active_quote == '"' => escaped = true,
                value if value == active_quote => quote = None,
                _ => current.push(ch),
            },
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '\\' => escaped = true,
                value if value.is_whitespace() => {
                    if !current.is_empty() {
                        words.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            },
        }
    }

    if escaped {
        return Err("Command parser found a trailing escape character.".to_string());
    }
    if quote.is_some() {
        return Err("Command parser found an unterminated quoted string.".to_string());
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

fn parse_chat_git_intent(prompt: &str) -> Result<Option<ChatGitComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    match command {
        "/git" => parse_git_command_words(&words).map(Some),
        "/pr" => parse_pr_command_words(&words).map(Some),
        _ => Ok(None),
    }
}

fn parse_git_command_words(words: &[String]) -> Result<ChatGitComposerIntent, String> {
    let Some(subcommand) = words.get(1).map(String::as_str) else {
        return Err(
            "`/git` requires a subcommand: status, pull, init, branch, checkout, worktree."
                .to_string(),
        );
    };
    match subcommand {
        "status" if words.len() == 2 => Ok(ChatGitComposerIntent::Status),
        "pull" if words.len() == 2 => Ok(ChatGitComposerIntent::Pull),
        "init" if words.len() == 2 => Ok(ChatGitComposerIntent::Init),
        "branch" if words.len() == 2 || words.get(2).map(String::as_str) == Some("list") => {
            if words.len() <= 3 {
                Ok(ChatGitComposerIntent::BranchList)
            } else {
                Err("Usage: `/git branch` or `/git branch create <name>`.".to_string())
            }
        }
        "branch" if words.get(2).map(String::as_str) == Some("create") => {
            let Some(branch) = words.get(3).map(String::as_str) else {
                return Err("Usage: `/git branch create <name>`.".to_string());
            };
            if words.len() != 4 {
                return Err("Usage: `/git branch create <name>`.".to_string());
            }
            Ok(ChatGitComposerIntent::BranchCreate {
                branch: branch.to_string(),
            })
        }
        "checkout" | "switch" => {
            let Some(branch) = words.get(2).map(String::as_str) else {
                return Err("Usage: `/git checkout <branch>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/git checkout <branch>`.".to_string());
            }
            Ok(ChatGitComposerIntent::Checkout {
                branch: branch.to_string(),
            })
        }
        "worktree" if words.get(2).map(String::as_str) == Some("list") => {
            if words.len() == 3 {
                Ok(ChatGitComposerIntent::WorktreeList)
            } else {
                Err("Usage: `/git worktree list`.".to_string())
            }
        }
        "worktree" if words.get(2).map(String::as_str) == Some("add") => {
            let Some(path) = words.get(3).map(String::as_str) else {
                return Err("Usage: `/git worktree add <path> <branch>`.".to_string());
            };
            let Some(branch) = words.get(4).map(String::as_str) else {
                return Err("Usage: `/git worktree add <path> <branch>`.".to_string());
            };
            if words.len() != 5 {
                return Err("Usage: `/git worktree add <path> <branch>`.".to_string());
            }
            Ok(ChatGitComposerIntent::WorktreeAdd {
                path: path.to_string(),
                branch: branch.to_string(),
            })
        }
        "worktree" if words.get(2).map(String::as_str) == Some("remove") => {
            let Some(path) = words.get(3).map(String::as_str) else {
                return Err("Usage: `/git worktree remove <path>`.".to_string());
            };
            if words.len() != 4 {
                return Err("Usage: `/git worktree remove <path>`.".to_string());
            }
            Ok(ChatGitComposerIntent::WorktreeRemove {
                path: path.to_string(),
            })
        }
        _ => Err(format!(
            "Unsupported git command. Try `/git status`, `/git branch create <name>`, `/git checkout <branch>`, or `/git worktree add <path> <branch>`."
        )),
    }
}

fn parse_pr_command_words(words: &[String]) -> Result<ChatGitComposerIntent, String> {
    if words.get(1).map(String::as_str) != Some("prep") {
        return Err("Usage: `/pr prep [base-branch]`.".to_string());
    }
    if words.len() > 3 {
        return Err("Usage: `/pr prep [base-branch]`.".to_string());
    }
    Ok(ChatGitComposerIntent::PrPrep {
        base_branch: words.get(2).cloned(),
    })
}

fn chat_git_command_workspace(
    state: &crate::app_state::RenderState,
) -> Result<std::path::PathBuf, String> {
    let Some(raw_workspace) = current_chat_workspace_root(state)
        .or_else(|| current_chat_session_cwd(state))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|value| value.into_os_string().into_string().ok())
        })
    else {
        return Err("No workspace path is available for local git actions.".to_string());
    };
    let trimmed = raw_workspace.trim();
    if trimmed.is_empty() {
        return Err("No workspace path is available for local git actions.".to_string());
    }
    let path = std::path::PathBuf::from(trimmed);
    let normalized = if path.is_file() {
        path.parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or(path.clone())
    } else {
        path
    };
    if !normalized.exists() {
        return Err(format!(
            "Workspace path does not exist: {}",
            normalized.display()
        ));
    }
    Ok(std::fs::canonicalize(&normalized).unwrap_or(normalized))
}

fn run_chat_git_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatGitComposerIntent,
) -> bool {
    let active_thread_id = state.autopilot_chat.active_thread_id.clone();
    if let Some(thread_id) = active_thread_id.as_deref() {
        state
            .autopilot_chat
            .remember_submission_draft(thread_id, prompt.clone());
    }
    let workspace = match chat_git_command_workspace(state) {
        Ok(workspace) => workspace,
        Err(error) => {
            state
                .autopilot_chat
                .append_local_exchange(prompt, error, true);
            return true;
        }
    };

    state.chat_inputs.composer.set_value(String::new());
    state.autopilot_chat.record_composer_draft(String::new());

    match execute_chat_git_intent(state, workspace.as_path(), &intent) {
        Ok(result) => {
            if let (Some(thread_id), Some(workspace_override)) = (
                active_thread_id.as_deref(),
                result.thread_workspace_override.clone(),
            ) {
                state.autopilot_chat.set_thread_workspace_location(
                    thread_id,
                    Some(workspace_override.clone()),
                    Some(workspace_override),
                );
            } else {
                state.autopilot_chat.refresh_project_registry();
            }
            state
                .autopilot_chat
                .append_local_exchange(prompt, result.response, false);
        }
        Err(error) => {
            state.autopilot_chat.refresh_project_registry();
            state
                .autopilot_chat
                .append_local_exchange(prompt, error, true);
        }
    }
    true
}

fn execute_chat_git_intent(
    state: &crate::app_state::RenderState,
    workspace: &std::path::Path,
    intent: &ChatGitComposerIntent,
) -> Result<ChatGitExecutionResult, String> {
    match intent {
        ChatGitComposerIntent::Status => {
            let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
            let worktrees = git_command_checked(workspace, &["worktree", "list"])?;
            let mut response = format!("Git status for `{}`.", workspace.display());
            append_text_block(&mut response, "Status", &status);
            append_text_block(&mut response, "Worktrees", &worktrees);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::Pull => {
            let output = git_command_checked(workspace, &["pull", "--ff-only"])?;
            let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
            let mut response = format!("Pulled latest changes in `{}`.", workspace.display());
            append_text_block(&mut response, "git pull --ff-only", &output);
            append_text_block(&mut response, "Status", &status);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::Init => {
            let output = git_command_checked(workspace, &["init"])?;
            let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
            let mut response = format!("Initialized a git repo in `{}`.", workspace.display());
            append_text_block(&mut response, "git init", &output);
            append_text_block(&mut response, "Status", &status);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: Some(path_display_string(workspace)),
            })
        }
        ChatGitComposerIntent::BranchList => {
            let branches = git_command_checked(workspace, &["branch", "--all", "--verbose"])?;
            let mut response = format!("Branch inventory for `{}`.", workspace.display());
            append_text_block(&mut response, "Branches", &branches);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::BranchCreate { branch } => {
            let output = git_command_checked(workspace, &["checkout", "-b", branch.as_str()])?;
            let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
            let mut response = format!(
                "Created and checked out branch `{branch}` in `{}`.",
                workspace.display()
            );
            append_text_block(&mut response, "git checkout -b", &output);
            append_text_block(&mut response, "Status", &status);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::Checkout { branch } => {
            let output = git_command_checked(workspace, &["checkout", branch.as_str()])?;
            let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
            let mut response = format!(
                "Checked out branch `{branch}` in `{}`.",
                workspace.display()
            );
            append_text_block(&mut response, "git checkout", &output);
            append_text_block(&mut response, "Status", &status);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::WorktreeList => {
            let worktrees = git_command_checked(workspace, &["worktree", "list"])?;
            let mut response = format!("Worktree inventory for `{}`.", workspace.display());
            append_text_block(&mut response, "Worktrees", &worktrees);
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: None,
            })
        }
        ChatGitComposerIntent::WorktreeAdd { path, branch } => {
            let worktree_path = resolve_git_worktree_path(path, workspace);
            let branch_exists = git_local_branch_exists(workspace, branch.as_str())?;
            let worktree_path_string = worktree_path.display().to_string();
            let args = if branch_exists {
                vec![
                    "worktree",
                    "add",
                    worktree_path_string.as_str(),
                    branch.as_str(),
                ]
            } else {
                vec![
                    "worktree",
                    "add",
                    "-b",
                    branch.as_str(),
                    worktree_path_string.as_str(),
                ]
            };
            let output = git_command_checked(workspace, &args)?;
            let worktrees = git_command_checked(workspace, &["worktree", "list"])?;
            let next_workspace = path_display_string(worktree_path.as_path());
            let mut response =
                format!("Added worktree `{}` for branch `{branch}`.", next_workspace);
            append_text_block(&mut response, "git worktree add", &output);
            append_text_block(&mut response, "Worktrees", &worktrees);
            response.push_str(
                "\n\nActive thread workspace now points at the new worktree so follow-up threads stay on that branch context.",
            );
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: Some(next_workspace),
            })
        }
        ChatGitComposerIntent::WorktreeRemove { path } => {
            let worktree_path = resolve_git_worktree_path(path, workspace);
            let active_workspace = state
                .autopilot_chat
                .active_thread_workspace_root()
                .map(std::path::PathBuf::from);
            let fallback_workspace = active_workspace
                .as_ref()
                .filter(|active| paths_equivalent(active.as_path(), worktree_path.as_path()))
                .and_then(|_| git_common_worktree_root(workspace))
                .map(|path| path_display_string(path.as_path()));
            let worktree_path_string = worktree_path.display().to_string();
            let output = git_command_checked(
                workspace,
                &["worktree", "remove", worktree_path_string.as_str()],
            )?;
            let worktrees = git_command_checked(workspace, &["worktree", "list"])?;
            let mut response = format!("Removed worktree `{}`.", worktree_path.display());
            append_text_block(&mut response, "git worktree remove", &output);
            append_text_block(&mut response, "Worktrees", &worktrees);
            if let Some(next_workspace) = fallback_workspace.as_deref() {
                response.push_str(&format!(
                    "\n\nActive thread workspace fell back to `{next_workspace}`."
                ));
            }
            Ok(ChatGitExecutionResult {
                response,
                thread_workspace_override: fallback_workspace,
            })
        }
        ChatGitComposerIntent::PrPrep { base_branch } => {
            build_pr_prep_response(state, workspace, base_branch.as_deref())
        }
    }
}

fn run_git_command(
    workspace: &std::path::Path,
    args: &[&str],
) -> Result<std::process::Output, String> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to launch git in {}: {error}", workspace.display()))
}

fn git_output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn git_command_checked(workspace: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let output = run_git_command(workspace, args)?;
    let body = git_output_text(&output);
    if output.status.success() {
        return Ok(body);
    }
    let fallback = if body.is_empty() {
        format!("git exited with status {}", output.status)
    } else {
        body
    };
    Err(format!(
        "`git {}` failed in `{}`.\n\n```text\n{}\n```",
        args.join(" "),
        workspace.display(),
        fallback
    ))
}

fn git_command_optional(workspace: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = run_git_command(workspace, args).ok()?;
    output.status.success().then(|| git_output_text(&output))
}

fn git_ref_exists(workspace: &std::path::Path, reference: &str) -> Result<bool, String> {
    let output = run_git_command(workspace, &["rev-parse", "--verify", "--quiet", reference])?;
    Ok(output.status.success())
}

fn git_local_branch_exists(workspace: &std::path::Path, branch: &str) -> Result<bool, String> {
    git_ref_exists(workspace, format!("refs/heads/{branch}").as_str())
}

fn git_current_branch(workspace: &std::path::Path) -> Result<String, String> {
    let branch = git_command_checked(workspace, &["branch", "--show-current"])?;
    let trimmed = branch.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    let detached = git_command_checked(workspace, &["rev-parse", "--short", "HEAD"])?;
    let trimmed = detached.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "Could not determine the current git branch for `{}`.",
            workspace.display()
        ));
    }
    Ok(trimmed.to_string())
}

fn git_origin_head_branch(workspace: &std::path::Path) -> Option<String> {
    let value = git_command_optional(
        workspace,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    value
        .trim()
        .strip_prefix("origin/")
        .map(ToString::to_string)
}

fn default_pr_base_branch(workspace: &std::path::Path, head_branch: &str) -> String {
    if let Some(origin_head) = git_origin_head_branch(workspace)
        && origin_head != head_branch
    {
        return origin_head;
    }
    for candidate in ["main", "master", "develop", "trunk"] {
        if candidate == head_branch {
            continue;
        }
        if git_ref_exists(workspace, candidate).unwrap_or(false)
            || git_ref_exists(workspace, format!("origin/{candidate}").as_str()).unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    head_branch.to_string()
}

fn git_remote_origin_url(workspace: &std::path::Path) -> Option<String> {
    let value = git_command_optional(workspace, &["remote", "get-url", "origin"])?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn resolve_git_worktree_path(raw: &str, workspace: &std::path::Path) -> std::path::PathBuf {
    expand_attachment_path(raw, workspace.to_str())
}

fn path_display_string(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn paths_equivalent(left: &std::path::Path, right: &std::path::Path) -> bool {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn git_common_worktree_root(workspace: &std::path::Path) -> Option<std::path::PathBuf> {
    let value = git_command_optional(workspace, &["rev-parse", "--git-common-dir"])?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let common_dir = {
        let path = std::path::PathBuf::from(trimmed);
        if path.is_absolute() {
            path
        } else {
            workspace.join(path)
        }
    };
    common_dir.parent().map(std::path::Path::to_path_buf)
}

fn github_compare_url(remote_url: &str, base_branch: &str, head_branch: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches('/');
    let repo_path = if let Some(value) = trimmed.strip_prefix("git@github.com:") {
        value
    } else if let Some(value) = trimmed.strip_prefix("ssh://git@github.com/") {
        value
    } else if let Some(value) = trimmed.strip_prefix("https://github.com/") {
        value
    } else if let Some(value) = trimmed.strip_prefix("http://github.com/") {
        value
    } else {
        return None;
    };
    let repo_path = repo_path.trim_end_matches(".git").trim_matches('/');
    if repo_path.is_empty() || !repo_path.contains('/') {
        return None;
    }
    Some(format!(
        "https://github.com/{repo_path}/compare/{base_branch}...{head_branch}?expand=1"
    ))
}

fn append_text_block(response: &mut String, label: &str, body: &str) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return;
    }
    response.push_str("\n\n");
    response.push_str(label);
    response.push_str(":\n```text\n");
    response.push_str(trimmed);
    response.push_str("\n```");
}

fn append_markdown_block(response: &mut String, label: &str, body: &str) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return;
    }
    response.push_str("\n\n");
    response.push_str(label);
    response.push_str(":\n```md\n");
    response.push_str(trimmed);
    response.push_str("\n```");
}

fn truncate_line(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    if count <= max_chars {
        return trimmed.to_string();
    }
    let truncated = trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{truncated}…")
}

fn suggested_pr_title(state: &crate::app_state::RenderState, head_branch: &str) -> Option<String> {
    if let Some(thread_id) = state.autopilot_chat.active_thread_id.as_deref()
        && let Some(name) = state
            .autopilot_chat
            .thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.thread_name.as_deref())
            .map(str::trim)
        && !name.is_empty()
    {
        return Some(truncate_line(name, 72));
    }
    if let Some(explanation) = state
        .autopilot_chat
        .active_plan_artifact()
        .and_then(|artifact| artifact.explanation.as_deref())
    {
        let line = explanation.lines().find(|line| !line.trim().is_empty())?;
        return Some(truncate_line(line, 72));
    }
    if let Some(diff) = state.autopilot_chat.active_diff_artifact()
        && diff.files.len() == 1
    {
        return Some(truncate_line(
            format!("Update {}", diff.files[0].path).as_str(),
            72,
        ));
    }
    (!head_branch.trim().is_empty()).then(|| truncate_line(head_branch, 72))
}

fn suggested_pr_body(state: &crate::app_state::RenderState) -> String {
    let mut body = String::from("## Summary\n");
    let mut summary_items = Vec::new();
    if let Some(plan) = state.autopilot_chat.active_plan_artifact() {
        for step in plan.steps.iter().take(4) {
            let trimmed = step.step.trim();
            if trimmed.is_empty() {
                continue;
            }
            summary_items.push(format!("- {trimmed}"));
        }
    }
    if summary_items.is_empty()
        && let Some(diff) = state.autopilot_chat.active_diff_artifact()
    {
        for file in diff.files.iter().take(4) {
            summary_items.push(format!(
                "- Update `{}` (+{} / -{})",
                file.path, file.added_line_count, file.removed_line_count
            ));
        }
    }
    if summary_items.is_empty() {
        summary_items.push("- Summarize the main user-facing and technical changes.".to_string());
    }
    body.push_str(&summary_items.join("\n"));
    body.push_str("\n\n## Testing\n- Not run yet.");
    body
}

fn build_pr_prep_response(
    state: &crate::app_state::RenderState,
    workspace: &std::path::Path,
    requested_base_branch: Option<&str>,
) -> Result<ChatGitExecutionResult, String> {
    let head_branch = git_current_branch(workspace)?;
    let base_branch = requested_base_branch
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| default_pr_base_branch(workspace, head_branch.as_str()));
    let status = git_command_checked(workspace, &["status", "--short", "--branch"])?;
    let diff_stat = git_command_optional(
        workspace,
        &["diff", "--stat", format!("{base_branch}...HEAD").as_str()],
    )
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| format!("No diff stat available for `{base_branch}...HEAD`."));
    let commits = git_command_optional(
        workspace,
        &["log", "--oneline", format!("{base_branch}..HEAD").as_str()],
    )
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| format!("No commits available for `{base_branch}..HEAD`."));
    let compare_url = git_remote_origin_url(workspace).and_then(|remote| {
        github_compare_url(remote.as_str(), base_branch.as_str(), head_branch.as_str())
    });
    let title = suggested_pr_title(state, head_branch.as_str())
        .unwrap_or_else(|| format!("Update {head_branch}"));
    let body = suggested_pr_body(state);

    let mut response = format!(
        "PR prep for `{}` against `{}` in `{}`.",
        head_branch,
        base_branch,
        workspace.display()
    );
    response.push_str("\n\nSuggested title:\n");
    response.push_str(title.trim());
    if let Some(compare_url) = compare_url {
        response.push_str("\n\nCompare URL:\n");
        response.push_str(compare_url.as_str());
    }
    append_markdown_block(&mut response, "Suggested body", &body);
    append_text_block(&mut response, "Status", &status);
    append_text_block(
        &mut response,
        format!("Commits ({base_branch}..HEAD)").as_str(),
        &commits,
    );
    append_text_block(
        &mut response,
        format!("Diff stat ({base_branch}...HEAD)").as_str(),
        &diff_stat,
    );
    Ok(ChatGitExecutionResult {
        response,
        thread_workspace_override: None,
    })
}

fn parse_chat_terminal_intent(prompt: &str) -> Result<Option<ChatTerminalComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.eq("/ps") {
        return Ok(Some(ChatTerminalComposerIntent::ListSessions));
    }
    if trimmed.eq("/clean") {
        return Ok(Some(ChatTerminalComposerIntent::CleanClosed));
    }
    if matches!(
        trimmed,
        "/term" | "/terminal" | "/term open" | "/terminal open"
    ) {
        return Ok(Some(ChatTerminalComposerIntent::Open));
    }
    if matches!(trimmed, "/term clear" | "/terminal clear") {
        return Ok(Some(ChatTerminalComposerIntent::Clear));
    }
    if matches!(trimmed, "/term restart" | "/terminal restart") {
        return Ok(Some(ChatTerminalComposerIntent::Restart));
    }
    if matches!(trimmed, "/term close" | "/terminal close") {
        return Ok(Some(ChatTerminalComposerIntent::Close));
    }
    if let Some(text) = trimmed
        .strip_prefix("/term write ")
        .or_else(|| trimmed.strip_prefix("/terminal write "))
        .or_else(|| trimmed.strip_prefix("/term send "))
        .or_else(|| trimmed.strip_prefix("/terminal send "))
    {
        let text = text.trim();
        if text.is_empty() {
            return Err("Usage: `/term write <text>`.".to_string());
        }
        return Ok(Some(ChatTerminalComposerIntent::Write {
            text: text.to_string(),
        }));
    }
    if let Some(rest) = trimmed
        .strip_prefix("/term resize ")
        .or_else(|| trimmed.strip_prefix("/terminal resize "))
    {
        let parts = parse_shell_like_words(rest)?;
        if parts.len() != 2 {
            return Err("Usage: `/term resize <cols> <rows>`.".to_string());
        }
        let cols = parts[0]
            .parse::<u16>()
            .map_err(|_| "Terminal resize requires numeric `<cols>`.".to_string())?;
        let rows = parts[1]
            .parse::<u16>()
            .map_err(|_| "Terminal resize requires numeric `<rows>`.".to_string())?;
        return Ok(Some(ChatTerminalComposerIntent::Resize { cols, rows }));
    }
    if trimmed.starts_with("/term") || trimmed.starts_with("/terminal") {
        return Err(
            "Unsupported terminal command. Try `/term open`, `/term write <text>`, `/term resize <cols> <rows>`, `/ps`, or `/clean`."
                .to_string(),
        );
    }
    Ok(None)
}

fn hazelnut_scope_label(scope: codex_client::HazelnutScope) -> &'static str {
    match scope {
        codex_client::HazelnutScope::Example => "example",
        codex_client::HazelnutScope::WorkspaceShared => "workspace-shared",
        codex_client::HazelnutScope::AllShared => "all-shared",
        codex_client::HazelnutScope::Personal => "personal",
    }
}

fn parse_hazelnut_scope(value: &str) -> Option<codex_client::HazelnutScope> {
    match value.trim().to_ascii_lowercase().as_str() {
        "example" => Some(codex_client::HazelnutScope::Example),
        "workspace-shared" | "workspace_shared" | "workspace" => {
            Some(codex_client::HazelnutScope::WorkspaceShared)
        }
        "all-shared" | "all_shared" | "shared" | "all" => {
            Some(codex_client::HazelnutScope::AllShared)
        }
        "personal" => Some(codex_client::HazelnutScope::Personal),
        _ => None,
    }
}

fn parse_chat_skills_intent(prompt: &str) -> Result<Option<ChatSkillsComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    if command != "/skills" && command != "/skill" {
        return Ok(None);
    }
    let subcommand = words.get(1).map(String::as_str);
    match subcommand {
        None => Ok(Some(ChatSkillsComposerIntent::Summary)),
        Some("list") if words.len() == 2 => Ok(Some(ChatSkillsComposerIntent::Summary)),
        Some("refresh") if words.len() == 2 => Ok(Some(ChatSkillsComposerIntent::Refresh)),
        Some("inspect") if words.len() <= 3 => Ok(Some(ChatSkillsComposerIntent::Inspect {
            query: words.get(2).cloned(),
        })),
        Some("use") => {
            let Some(query) = words.get(2).cloned() else {
                return Err("Usage: `/skills use <name|path>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/skills use <name|path>`.".to_string());
            }
            Ok(Some(ChatSkillsComposerIntent::Use { query }))
        }
        Some("clear") if words.len() == 2 => Ok(Some(ChatSkillsComposerIntent::Clear)),
        Some("enable") => {
            let Some(query) = words.get(2).cloned() else {
                return Err("Usage: `/skills enable <name|path>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/skills enable <name|path>`.".to_string());
            }
            Ok(Some(ChatSkillsComposerIntent::SetEnabled {
                query,
                enabled: true,
            }))
        }
        Some("disable") => {
            let Some(query) = words.get(2).cloned() else {
                return Err("Usage: `/skills disable <name|path>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/skills disable <name|path>`.".to_string());
            }
            Ok(Some(ChatSkillsComposerIntent::SetEnabled {
                query,
                enabled: false,
            }))
        }
        Some("remote") if words.len() == 2 => Ok(Some(ChatSkillsComposerIntent::RemoteSummary)),
        Some("remote") if words.get(2).map(String::as_str) == Some("list") => {
            if words.len() > 4 {
                return Err(
                    "Usage: `/skills remote list [example|workspace-shared|all-shared|personal]`."
                        .to_string(),
                );
            }
            let scope = if let Some(raw_scope) = words.get(3).map(String::as_str) {
                parse_hazelnut_scope(raw_scope).ok_or_else(|| {
                    "Remote scope must be one of: example, workspace-shared, all-shared, personal."
                        .to_string()
                })?
            } else {
                codex_client::HazelnutScope::Example
            };
            Ok(Some(ChatSkillsComposerIntent::RemoteList { scope }))
        }
        Some("remote") if words.get(2).map(String::as_str) == Some("export") => {
            let Some(query) = words.get(3).cloned() else {
                return Err("Usage: `/skills remote export <name|id>`.".to_string());
            };
            if words.len() != 4 {
                return Err("Usage: `/skills remote export <name|id>`.".to_string());
            }
            Ok(Some(ChatSkillsComposerIntent::RemoteExport { query }))
        }
        Some("remote") => Err(
            "Usage: `/skills remote`, `/skills remote list [scope]`, or `/skills remote export <name|id>`."
                .to_string(),
        ),
        _ => Err(
            "Skills commands: `/skills`, `/skills refresh`, `/skills use <name>`, `/skills enable <name>`, `/skills disable <name>`, `/skills remote ...`"
                .to_string(),
        ),
    }
}

fn parse_chat_mcp_intent(prompt: &str) -> Result<Option<ChatMcpComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    if command != "/mcp" {
        return Ok(None);
    }
    match words.get(1).map(String::as_str) {
        None | Some("status") | Some("list") if words.len() <= 2 => {
            Ok(Some(ChatMcpComposerIntent::Summary))
        }
        Some("refresh") if words.len() == 2 => Ok(Some(ChatMcpComposerIntent::Refresh)),
        Some("reload") if words.len() == 2 => Ok(Some(ChatMcpComposerIntent::Reload)),
        Some("select") => {
            let Some(query) = words.get(2).cloned() else {
                return Err("Usage: `/mcp select <server>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/mcp select <server>`.".to_string());
            }
            Ok(Some(ChatMcpComposerIntent::Select { query }))
        }
        Some("login") if words.len() <= 3 => Ok(Some(ChatMcpComposerIntent::Login {
            query: words.get(2).cloned(),
        })),
        _ => Err(
            "MCP commands: `/mcp`, `/mcp refresh`, `/mcp reload`, `/mcp select <server>`, `/mcp login [server]`"
                .to_string(),
        ),
    }
}

fn parse_chat_apps_intent(prompt: &str) -> Result<Option<ChatAppsComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    if command != "/apps" && command != "/app" {
        return Ok(None);
    }
    match words.get(1).map(String::as_str) {
        None | Some("list") if words.len() <= 2 => Ok(Some(ChatAppsComposerIntent::Summary)),
        Some("refresh") if words.len() == 2 => Ok(Some(ChatAppsComposerIntent::Refresh)),
        Some("inspect") if words.len() <= 3 => Ok(Some(ChatAppsComposerIntent::Inspect {
            query: words.get(2).cloned(),
        })),
        Some("select") => {
            let Some(query) = words.get(2).cloned() else {
                return Err("Usage: `/apps select <name|id>`.".to_string());
            };
            if words.len() != 3 {
                return Err("Usage: `/apps select <name|id>`.".to_string());
            }
            Ok(Some(ChatAppsComposerIntent::Select { query }))
        }
        _ => Err(
            "Apps commands: `/apps`, `/apps refresh`, `/apps inspect [name|id]`, `/apps select <name|id>`"
                .to_string(),
        ),
    }
}

fn parse_chat_request_intent(prompt: &str) -> Result<Option<ChatRequestComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    match command {
        "/requests" => {
            if words.len() <= 2 && words.get(1).map(String::as_str).unwrap_or("list") == "list" {
                Ok(Some(ChatRequestComposerIntent::Summary))
            } else {
                Err("Usage: `/requests`.".to_string())
            }
        }
        "/approvals" | "/approval" => match words.get(1).map(String::as_str) {
            None | Some("list") if words.len() <= 2 => Ok(Some(ChatRequestComposerIntent::Summary)),
            Some("accept") if words.len() == 2 => Ok(Some(ChatRequestComposerIntent::Approval {
                decision: ApprovalDecision::Accept,
                label: "accept",
            })),
            Some("session") | Some("accept-session") if words.len() == 2 => {
                Ok(Some(ChatRequestComposerIntent::Approval {
                    decision: ApprovalDecision::AcceptForSession,
                    label: "accept-for-session",
                }))
            }
            Some("decline") if words.len() == 2 => Ok(Some(ChatRequestComposerIntent::Approval {
                decision: ApprovalDecision::Decline,
                label: "decline",
            })),
            Some("cancel") if words.len() == 2 => Ok(Some(ChatRequestComposerIntent::Approval {
                decision: ApprovalDecision::Cancel,
                label: "cancel",
            })),
            _ => Err(
                "Approval commands: `/approvals`, `/approvals accept`, `/approvals session`, `/approvals decline`, `/approvals cancel`"
                    .to_string(),
            ),
        },
        "/tool" => match words.get(1).map(String::as_str) {
            Some("respond") if words.len() == 2 => Ok(Some(ChatRequestComposerIntent::ToolCallRespond)),
            Some("prompt") | Some("input") if words.len() == 2 => {
                Ok(Some(ChatRequestComposerIntent::ToolUserInputRespond))
            }
            Some("prompt") | Some("input")
                if words.get(2).map(String::as_str) == Some("respond") && words.len() == 3 =>
            {
                Ok(Some(ChatRequestComposerIntent::ToolUserInputRespond))
            }
            _ => Ok(None),
        },
        "/auth" => match words.get(1).map(String::as_str) {
            Some("respond") if words.len() == 2 => Ok(Some(ChatRequestComposerIntent::AuthRefreshRespond)),
            Some("refresh") if words.len() == 2 => {
                Ok(Some(ChatRequestComposerIntent::AuthRefreshRespond))
            }
            Some("refresh")
                if words.get(2).map(String::as_str) == Some("respond") && words.len() == 3 =>
            {
                Ok(Some(ChatRequestComposerIntent::AuthRefreshRespond))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn parse_chat_remote_intent(prompt: &str) -> Result<Option<ChatRemoteComposerIntent>, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let words = parse_shell_like_words(trimmed)?;
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    if command != "/remote" {
        return Ok(None);
    }
    match words.get(1).map(String::as_str) {
        None | Some("status") | Some("show") if words.len() <= 2 => {
            Ok(Some(ChatRemoteComposerIntent::Summary))
        }
        Some("enable") if words.len() <= 3 => Ok(Some(ChatRemoteComposerIntent::Enable {
            bind_addr: words.get(2).cloned(),
        })),
        Some("disable") if words.len() == 2 => Ok(Some(ChatRemoteComposerIntent::Disable)),
        Some("rotate-token") | Some("rotate") if words.len() == 2 => {
            Ok(Some(ChatRemoteComposerIntent::RotateToken))
        }
        _ => Err(
            "Remote commands: `/remote`, `/remote enable [ip:port]`, `/remote disable`, `/remote rotate-token`"
                .to_string(),
        ),
    }
}

fn resolve_discovered_skill_index(
    state: &crate::app_state::RenderState,
    query: Option<&str>,
) -> Result<usize, String> {
    if let Some(query) = query {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err("Skill query cannot be empty.".to_string());
        }
        let normalized = trimmed.to_ascii_lowercase();
        let mut fuzzy_matches = Vec::new();
        for (index, skill) in state.skill_registry.discovered_skills.iter().enumerate() {
            if skill.name.eq_ignore_ascii_case(trimmed) || skill.path == trimmed {
                return Ok(index);
            }
            let path = skill.path.to_ascii_lowercase();
            if path.ends_with(&normalized) || path.contains(&normalized) {
                fuzzy_matches.push(index);
            }
        }
        return match fuzzy_matches.as_slice() {
            [index] => Ok(*index),
            [] => Err(format!(
                "No discovered skill matched `{trimmed}`. Use `/skills refresh` first if the cache is empty."
            )),
            matches => Err(format!(
                "Skill query `{trimmed}` matched multiple entries: {}",
                matches
                    .iter()
                    .filter_map(|index| state.skill_registry.discovered_skills.get(*index))
                    .map(|skill| skill.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        };
    }
    state.skill_registry.selected_skill_index.ok_or_else(|| {
        "No skill is selected. Use `/skills use <name>` or `/skills inspect <name>`.".to_string()
    })
}

fn resolve_remote_skill_index(
    state: &crate::app_state::RenderState,
    query: &str,
) -> Result<usize, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("Remote skill query cannot be empty.".to_string());
    }
    let normalized = trimmed.to_ascii_lowercase();
    let mut fuzzy_matches = Vec::new();
    for (index, skill) in state.skill_registry.remote_skills.iter().enumerate() {
        if skill.id == trimmed || skill.name.eq_ignore_ascii_case(trimmed) {
            return Ok(index);
        }
        let name = skill.name.to_ascii_lowercase();
        if name.contains(&normalized) {
            fuzzy_matches.push(index);
        }
    }
    match fuzzy_matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(format!(
            "No cached remote skill matched `{trimmed}`. Use `/skills remote list [scope]` first."
        )),
        matches => Err(format!(
            "Remote skill query `{trimmed}` matched multiple entries: {}",
            matches
                .iter()
                .filter_map(|index| state.skill_registry.remote_skills.get(*index))
                .map(|skill| skill.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn resolve_mcp_server_index(
    state: &crate::app_state::RenderState,
    query: Option<&str>,
) -> Result<usize, String> {
    if let Some(query) = query {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err("MCP server query cannot be empty.".to_string());
        }
        let normalized = trimmed.to_ascii_lowercase();
        let mut fuzzy_matches = Vec::new();
        for (index, server) in state.codex_mcp.servers.iter().enumerate() {
            if server.name.eq_ignore_ascii_case(trimmed) {
                return Ok(index);
            }
            if server.name.to_ascii_lowercase().contains(&normalized) {
                fuzzy_matches.push(index);
            }
        }
        return match fuzzy_matches.as_slice() {
            [index] => Ok(*index),
            [] => Err(format!(
                "No MCP server matched `{trimmed}`. Use `/mcp refresh` to reload the cache."
            )),
            matches => Err(format!(
                "MCP query `{trimmed}` matched multiple servers: {}",
                matches
                    .iter()
                    .filter_map(|index| state.codex_mcp.servers.get(*index))
                    .map(|server| server.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        };
    }
    state
        .codex_mcp
        .selected_server_index
        .ok_or_else(|| "No MCP server is selected. Use `/mcp select <server>` first.".to_string())
}

fn resolve_app_index(
    state: &crate::app_state::RenderState,
    query: Option<&str>,
) -> Result<usize, String> {
    if let Some(query) = query {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err("App query cannot be empty.".to_string());
        }
        let normalized = trimmed.to_ascii_lowercase();
        let mut fuzzy_matches = Vec::new();
        for (index, app) in state.codex_apps.apps.iter().enumerate() {
            if app.id == trimmed || app.name.eq_ignore_ascii_case(trimmed) {
                return Ok(index);
            }
            let name = app.name.to_ascii_lowercase();
            let id = app.id.to_ascii_lowercase();
            if name.contains(&normalized) || id.contains(&normalized) {
                fuzzy_matches.push(index);
            }
        }
        return match fuzzy_matches.as_slice() {
            [index] => Ok(*index),
            [] => Err(format!(
                "No cached app matched `{trimmed}`. Use `/apps refresh` to reload connectors."
            )),
            matches => Err(format!(
                "App query `{trimmed}` matched multiple entries: {}",
                matches
                    .iter()
                    .filter_map(|index| state.codex_apps.apps.get(*index))
                    .map(|app| app.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        };
    }
    state
        .codex_apps
        .selected_app_index
        .ok_or_else(|| "No app is selected. Use `/apps select <name|id>` first.".to_string())
}

fn format_skills_summary(state: &crate::app_state::RenderState) -> String {
    let selected = state
        .skill_registry
        .selected_skill_index
        .and_then(|index| state.skill_registry.discovered_skills.get(index))
        .map(|skill| {
            format!(
                "Selected skill: {} [{}] {}",
                skill.name, skill.scope, skill.path
            )
        })
        .unwrap_or_else(|| "Selected skill: none".to_string());
    let mut lines = vec![
        format!(
            "Local skills: {} discovered, {} discovery errors.",
            state.skill_registry.discovered_skills.len(),
            state.skill_registry.discovery_errors.len()
        ),
        format!(
            "Repo root: {}",
            state
                .skill_registry
                .repo_skills_root
                .as_deref()
                .unwrap_or("n/a")
        ),
        selected,
        format!(
            "Remote cache: {} skill(s) in {} scope.",
            state.skill_registry.remote_skills.len(),
            hazelnut_scope_label(state.skill_registry.remote_scope)
        ),
    ];
    if let Some(path) = state.skill_registry.last_remote_export_path.as_deref() {
        lines.push(format!(
            "Last remote export: {} -> {}",
            state
                .skill_registry
                .last_remote_export_id
                .as_deref()
                .unwrap_or("n/a"),
            path
        ));
    }
    if state.skill_registry.discovered_skills.is_empty() {
        lines.push("No local skills are cached yet. Use `/skills refresh`.".to_string());
    } else {
        lines.push("Cached local skills:".to_string());
        for skill in state.skill_registry.discovered_skills.iter().take(6) {
            lines.push(format!(
                "- {} [{}] scope={} deps={} path={}",
                skill.name,
                if skill.enabled { "enabled" } else { "disabled" },
                skill.scope,
                skill.dependency_count,
                skill.path
            ));
        }
    }
    lines.join("\n")
}

fn format_skill_detail(
    state: &crate::app_state::RenderState,
    index: usize,
) -> Result<String, String> {
    let Some(skill) = state.skill_registry.discovered_skills.get(index) else {
        return Err("Selected skill row is no longer valid.".to_string());
    };
    Ok(format!(
        "Skill `{}`\n- status: {}\n- scope: {}\n- deps: {}\n- interface: {}\n- path: {}",
        skill.name,
        if skill.enabled { "enabled" } else { "disabled" },
        skill.scope,
        skill.dependency_count,
        skill.interface_display_name.as_deref().unwrap_or("n/a"),
        skill.path
    ))
}

fn format_remote_skills_summary(state: &crate::app_state::RenderState) -> String {
    let mut lines = vec![format!(
        "Remote skills cache: {} item(s) in {} scope.",
        state.skill_registry.remote_skills.len(),
        hazelnut_scope_label(state.skill_registry.remote_scope)
    )];
    if state.skill_registry.remote_skills.is_empty() {
        lines.push(
            "No remote skills are cached yet. Use `/skills remote list [scope]`.".to_string(),
        );
    } else {
        for skill in state.skill_registry.remote_skills.iter().take(8) {
            lines.push(format!(
                "- {} ({}) {}",
                skill.name, skill.id, skill.description
            ));
        }
    }
    lines.join("\n")
}

fn format_mcp_summary(state: &crate::app_state::RenderState) -> String {
    let authorized = state
        .codex_mcp
        .servers
        .iter()
        .filter(|server| {
            let auth = server.auth_status.to_ascii_lowercase();
            auth.contains("ok")
                || auth.contains("ready")
                || auth.contains("authorized")
                || auth.contains("connected")
        })
        .count();
    let tool_count = state
        .codex_mcp
        .servers
        .iter()
        .map(|server| server.tool_count)
        .sum::<usize>();
    let resource_count = state
        .codex_mcp
        .servers
        .iter()
        .map(|server| server.resource_count)
        .sum::<usize>();
    let template_count = state
        .codex_mcp
        .servers
        .iter()
        .map(|server| server.template_count)
        .sum::<usize>();
    let mut lines = vec![format!(
        "MCP cache: {} server(s), {} authorized, {} tools, {} resources, {} templates.",
        state.codex_mcp.servers.len(),
        authorized,
        tool_count,
        resource_count,
        template_count
    )];
    if let Some(index) = state.codex_mcp.selected_server_index
        && let Some(server) = state.codex_mcp.servers.get(index)
    {
        lines.push(format!(
            "Selected server: {} auth={} tools={} resources={} templates={}",
            server.name,
            server.auth_status,
            server.tool_count,
            server.resource_count,
            server.template_count
        ));
    }
    if let Some(result) = state.codex_mcp.last_oauth_result.as_deref() {
        lines.push(format!("OAuth result: {result}"));
    }
    if let Some(url) = state.codex_mcp.last_oauth_url.as_deref() {
        lines.push(format!("OAuth URL: {}", truncate_line(url, 96)));
    }
    if state.codex_mcp.servers.is_empty() {
        lines.push("No MCP status is cached yet. Use `/mcp refresh`.".to_string());
    } else {
        for server in state.codex_mcp.servers.iter().take(6) {
            lines.push(format!(
                "- {} auth={} tools={} resources={} templates={}",
                server.name,
                server.auth_status,
                server.tool_count,
                server.resource_count,
                server.template_count
            ));
        }
    }
    lines.join("\n")
}

fn format_apps_summary(state: &crate::app_state::RenderState) -> String {
    let accessible = state
        .codex_apps
        .apps
        .iter()
        .filter(|app| app.is_accessible)
        .count();
    let enabled = state
        .codex_apps
        .apps
        .iter()
        .filter(|app| app.is_enabled)
        .count();
    let mut lines = vec![format!(
        "Apps cache: {} connector(s), {} accessible, {} enabled, updates seen={}.",
        state.codex_apps.apps.len(),
        accessible,
        enabled,
        state.codex_apps.update_count
    )];
    if let Some(index) = state.codex_apps.selected_app_index
        && let Some(app) = state.codex_apps.apps.get(index)
    {
        lines.push(format!(
            "Selected app: {} ({}) accessible={} enabled={} desc={}",
            app.name,
            app.id,
            app.is_accessible,
            app.is_enabled,
            app.description.as_deref().unwrap_or("n/a")
        ));
    }
    if state.codex_apps.apps.is_empty() {
        lines.push("No apps are cached yet. Use `/apps refresh`.".to_string());
    } else {
        for app in state.codex_apps.apps.iter().take(8) {
            lines.push(format!(
                "- {} ({}) accessible={} enabled={} desc={}",
                app.name,
                app.id,
                app.is_accessible,
                app.is_enabled,
                app.description.as_deref().unwrap_or("n/a")
            ));
        }
    }
    lines.join("\n")
}

fn format_request_summary(state: &crate::app_state::RenderState) -> String {
    let mut lines = vec![format!(
        "Pending requests: command approvals={}, file approvals={}, tool calls={}, tool prompts={}, auth refresh={}.",
        state.autopilot_chat.pending_command_approvals.len(),
        state.autopilot_chat.pending_file_change_approvals.len(),
        state.autopilot_chat.pending_tool_calls.len(),
        state.autopilot_chat.pending_tool_user_input.len(),
        state.autopilot_chat.pending_auth_refresh.len()
    )];
    if let Some(request) = state.autopilot_chat.pending_command_approvals.first() {
        lines.push(format!(
            "Next command approval: turn={} command={} cwd={} reason={}",
            request.turn_id,
            request.command.as_deref().unwrap_or("n/a"),
            request.cwd.as_deref().unwrap_or("n/a"),
            request.reason.as_deref().unwrap_or("n/a")
        ));
    }
    if let Some(request) = state.autopilot_chat.pending_file_change_approvals.first() {
        lines.push(format!(
            "Next file approval: turn={} grant_root={} reason={}",
            request.turn_id,
            request.grant_root.as_deref().unwrap_or("n/a"),
            request.reason.as_deref().unwrap_or("n/a")
        ));
    }
    if let Some(request) = state.autopilot_chat.pending_tool_calls.first() {
        lines.push(format!(
            "Next tool call: {} ({})",
            request.tool, request.call_id
        ));
    }
    if let Some(request) = state.autopilot_chat.pending_tool_user_input.first() {
        let headers = request
            .questions
            .iter()
            .map(|question| question.header.clone())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "Next tool prompt: {} question(s) [{}]",
            request.questions.len(),
            headers
        ));
    }
    if let Some(request) = state.autopilot_chat.pending_auth_refresh.first() {
        lines.push(format!(
            "Next auth refresh: reason={} account={}",
            request.reason,
            request.previous_account_id.as_deref().unwrap_or("n/a")
        ));
    }
    if lines.len() == 1
        && state.autopilot_chat.pending_command_approvals.is_empty()
        && state
            .autopilot_chat
            .pending_file_change_approvals
            .is_empty()
        && state.autopilot_chat.pending_tool_calls.is_empty()
        && state.autopilot_chat.pending_tool_user_input.is_empty()
        && state.autopilot_chat.pending_auth_refresh.is_empty()
    {
        lines.push("No pending approvals or request-flow prompts.".to_string());
    } else {
        lines.push(
            "Respond with `/approvals accept|session|decline|cancel`, `/tool respond`, `/tool prompt respond`, or `/auth respond`."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn append_chat_command_result(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    response: String,
    is_error: bool,
) -> bool {
    state
        .autopilot_chat
        .append_local_exchange(prompt, response, is_error);
    true
}

fn run_chat_skills_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatSkillsComposerIntent,
) -> bool {
    remember_chat_command_prompt(state, &prompt);
    clear_chat_command_prompt(state);

    match intent {
        ChatSkillsComposerIntent::Summary => {
            append_chat_command_result(state, prompt, format_skills_summary(state), false)
        }
        ChatSkillsComposerIntent::Refresh => {
            super::reducers::run_skill_registry_action(
                state,
                crate::pane_system::SkillRegistryPaneAction::DiscoverSkills,
            );
            let is_error = state.skill_registry.last_error.is_some();
            let response = state.skill_registry.last_error.clone().unwrap_or_else(|| {
                "Queued codex skills/list refresh for the current workspace.".to_string()
            });
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatSkillsComposerIntent::Inspect { query } => {
            match resolve_discovered_skill_index(state, query.as_deref()).and_then(|index| {
                super::reducers::run_skill_registry_action(
                    state,
                    crate::pane_system::SkillRegistryPaneAction::SelectRow(index),
                );
                format_skill_detail(state, index)
            }) {
                Ok(response) => append_chat_command_result(state, prompt, response, false),
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatSkillsComposerIntent::Use { query } => {
            match resolve_discovered_skill_index(state, Some(query.as_str())) {
                Ok(index) => {
                    super::reducers::run_skill_registry_action(
                        state,
                        crate::pane_system::SkillRegistryPaneAction::SelectRow(index),
                    );
                    let response = format_skill_detail(state, index)
                        .map(|detail| format!("{detail}\n\nNew turns will attach this skill by default until you run `/skills clear`."))
                        .unwrap_or_else(|error| error);
                    append_chat_command_result(state, prompt, response, false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatSkillsComposerIntent::Clear => {
            state.skill_registry.selected_skill_index = None;
            append_chat_command_result(
                state,
                prompt,
                "Cleared the manually selected skill. Goal-policy and required skills still attach when applicable.".to_string(),
                false,
            )
        }
        ChatSkillsComposerIntent::SetEnabled { query, enabled } => {
            match resolve_discovered_skill_index(state, Some(query.as_str())) {
                Ok(index) => {
                    let Some(skill) = state.skill_registry.discovered_skills.get(index).cloned()
                    else {
                        return append_chat_command_result(
                            state,
                            prompt,
                            "Selected skill row is no longer valid.".to_string(),
                            true,
                        );
                    };
                    state.skill_registry.selected_skill_index = Some(index);
                    if skill.enabled == enabled {
                        return append_chat_command_result(
                            state,
                            prompt,
                            format!(
                                "Skill `{}` is already {}.",
                                skill.name,
                                if enabled { "enabled" } else { "disabled" }
                            ),
                            false,
                        );
                    }
                    let path = std::path::PathBuf::from(skill.path.clone());
                    if !path.is_absolute() {
                        return append_chat_command_result(
                            state,
                            prompt,
                            format!("Skill path is not absolute: {}", skill.path),
                            true,
                        );
                    }
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Loading;
                    state.skill_registry.last_error = None;
                    state.skill_registry.last_action = Some(format!(
                        "Queued codex skills/config/write for {}",
                        skill.name
                    ));
                    let result = state.queue_codex_command(
                        crate::codex_lane::CodexLaneCommand::SkillsConfigWrite(
                            codex_client::SkillsConfigWriteParams { path, enabled },
                        ),
                    );
                    match result {
                        Ok(command_seq) => append_chat_command_result(
                            state,
                            prompt,
                            format!(
                                "Queued skills/config/write #{} to {} `{}`. Local skill cache will refresh after Codex applies it.",
                                command_seq,
                                if enabled { "enable" } else { "disable" },
                                skill.name
                            ),
                            false,
                        ),
                        Err(error) => append_chat_command_result(state, prompt, error, true),
                    }
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatSkillsComposerIntent::RemoteSummary => {
            append_chat_command_result(state, prompt, format_remote_skills_summary(state), false)
        }
        ChatSkillsComposerIntent::RemoteList { scope } => {
            state.skill_registry.remote_scope = scope;
            state.skill_registry.load_state = crate::app_state::PaneLoadState::Loading;
            state.skill_registry.last_error = None;
            state.skill_registry.last_action = Some(format!(
                "Queued skills/remote/list for {}",
                hazelnut_scope_label(scope)
            ));
            match state.queue_codex_command(crate::codex_lane::CodexLaneCommand::SkillsRemoteList(
                codex_client::SkillsRemoteReadParams {
                    hazelnut_scope: scope,
                    product_surface: codex_client::ProductSurface::Codex,
                    enabled: false,
                },
            )) {
                Ok(command_seq) => append_chat_command_result(
                    state,
                    prompt,
                    format!(
                        "Queued skills/remote/list #{} for {} scope.",
                        command_seq,
                        hazelnut_scope_label(scope)
                    ),
                    false,
                ),
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatSkillsComposerIntent::RemoteExport { query } => {
            match resolve_remote_skill_index(state, query.as_str()) {
                Ok(index) => {
                    let Some(skill) = state.skill_registry.remote_skills.get(index).cloned() else {
                        return append_chat_command_result(
                            state,
                            prompt,
                            "Selected remote skill is no longer cached.".to_string(),
                            true,
                        );
                    };
                    state.skill_registry.load_state = crate::app_state::PaneLoadState::Loading;
                    state.skill_registry.last_error = None;
                    state.skill_registry.last_action =
                        Some(format!("Queued skills/remote/export for {}", skill.name));
                    match state.queue_codex_command(
                        crate::codex_lane::CodexLaneCommand::SkillsRemoteExport(
                            codex_client::SkillsRemoteWriteParams {
                                hazelnut_id: skill.id.clone(),
                            },
                        ),
                    ) {
                        Ok(command_seq) => append_chat_command_result(
                            state,
                            prompt,
                            format!(
                                "Queued skills/remote/export #{} for `{}` ({}). Local skills will refresh after the export completes.",
                                command_seq, skill.name, skill.id
                            ),
                            false,
                        ),
                        Err(error) => append_chat_command_result(state, prompt, error, true),
                    }
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
    }
}

fn run_chat_mcp_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatMcpComposerIntent,
) -> bool {
    remember_chat_command_prompt(state, &prompt);
    clear_chat_command_prompt(state);

    match intent {
        ChatMcpComposerIntent::Summary => {
            append_chat_command_result(state, prompt, format_mcp_summary(state), false)
        }
        ChatMcpComposerIntent::Refresh => {
            run_codex_mcp_action(state, CodexMcpPaneAction::Refresh);
            let is_error = state.codex_mcp.last_error.is_some();
            let response = state.codex_mcp.last_error.clone().unwrap_or_else(|| {
                "Queued mcpServerStatus/list refresh. Use `/mcp` again after Codex responds."
                    .to_string()
            });
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatMcpComposerIntent::Reload => {
            run_codex_mcp_action(state, CodexMcpPaneAction::Reload);
            let is_error = state.codex_mcp.last_error.is_some();
            let response = state.codex_mcp.last_error.clone().unwrap_or_else(|| {
                "Queued MCP config reload. Status will refresh when Codex acknowledges it."
                    .to_string()
            });
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatMcpComposerIntent::Select { query } => {
            match resolve_mcp_server_index(state, Some(query.as_str())) {
                Ok(index) => {
                    run_codex_mcp_action(state, CodexMcpPaneAction::SelectRow(index));
                    append_chat_command_result(state, prompt, format_mcp_summary(state), false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatMcpComposerIntent::Login { query } => {
            match resolve_mcp_server_index(state, query.as_deref()) {
                Ok(index) => {
                    run_codex_mcp_action(state, CodexMcpPaneAction::SelectRow(index));
                    run_codex_mcp_action(state, CodexMcpPaneAction::LoginSelected);
                    let is_error = state.codex_mcp.last_error.is_some();
                    let response = state.codex_mcp.last_error.clone().unwrap_or_else(|| {
                        let server = state
                            .codex_mcp
                            .servers
                            .get(index)
                            .map(|entry| entry.name.as_str())
                            .unwrap_or("selected server");
                        format!(
                            "Queued MCP OAuth login for {}. Check `/mcp` for the authorization URL and result.",
                            server
                        )
                    });
                    append_chat_command_result(state, prompt, response, is_error)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
    }
}

fn run_chat_apps_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatAppsComposerIntent,
) -> bool {
    remember_chat_command_prompt(state, &prompt);
    clear_chat_command_prompt(state);

    match intent {
        ChatAppsComposerIntent::Summary => {
            append_chat_command_result(state, prompt, format_apps_summary(state), false)
        }
        ChatAppsComposerIntent::Refresh => {
            run_codex_apps_action(state, CodexAppsPaneAction::Refresh);
            let is_error = state.codex_apps.last_error.is_some();
            let response = state.codex_apps.last_error.clone().unwrap_or_else(|| {
                "Queued app/list refresh for the active thread context.".to_string()
            });
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatAppsComposerIntent::Inspect { query } => {
            match resolve_app_index(state, query.as_deref()) {
                Ok(index) => {
                    run_codex_apps_action(state, CodexAppsPaneAction::SelectRow(index));
                    append_chat_command_result(state, prompt, format_apps_summary(state), false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatAppsComposerIntent::Select { query } => {
            match resolve_app_index(state, Some(query.as_str())) {
                Ok(index) => {
                    run_codex_apps_action(state, CodexAppsPaneAction::SelectRow(index));
                    append_chat_command_result(state, prompt, format_apps_summary(state), false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
    }
}

fn run_chat_request_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatRequestComposerIntent,
) -> bool {
    remember_chat_command_prompt(state, &prompt);
    clear_chat_command_prompt(state);

    match intent {
        ChatRequestComposerIntent::Summary => {
            append_chat_command_result(state, prompt, format_request_summary(state), false)
        }
        ChatRequestComposerIntent::Approval { decision, label } => {
            let had_requests = !state.autopilot_chat.pending_command_approvals.is_empty()
                || !state
                    .autopilot_chat
                    .pending_file_change_approvals
                    .is_empty();
            run_chat_approval_response_action(state, decision);
            let is_error = state.autopilot_chat.last_error.is_some();
            let response = if is_error {
                state
                    .autopilot_chat
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Approval response failed".to_string())
            } else if had_requests {
                format!("Submitted the next pending approval response: {label}.")
            } else {
                "No pending approval requests".to_string()
            };
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatRequestComposerIntent::ToolCallRespond => {
            let had_requests = !state.autopilot_chat.pending_tool_calls.is_empty();
            run_chat_tool_call_response_action(state);
            let is_error = state.autopilot_chat.last_error.is_some();
            let response = if is_error {
                state
                    .autopilot_chat
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Tool-call response failed".to_string())
            } else if had_requests {
                "Submitted the next pending tool-call response.".to_string()
            } else {
                "No pending tool calls".to_string()
            };
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatRequestComposerIntent::ToolUserInputRespond => {
            let had_requests = !state.autopilot_chat.pending_tool_user_input.is_empty();
            run_chat_tool_user_input_response_action(state);
            let is_error = state.autopilot_chat.last_error.is_some();
            let response = if is_error {
                state
                    .autopilot_chat
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Tool user-input response failed".to_string())
            } else if had_requests {
                "Submitted answers for the next pending tool user-input request.".to_string()
            } else {
                "No pending tool user-input requests".to_string()
            };
            append_chat_command_result(state, prompt, response, is_error)
        }
        ChatRequestComposerIntent::AuthRefreshRespond => {
            let had_requests = !state.autopilot_chat.pending_auth_refresh.is_empty();
            run_chat_auth_refresh_response_action(state);
            let is_error = state.autopilot_chat.last_error.is_some();
            let response = if is_error {
                state
                    .autopilot_chat
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Auth-refresh response failed".to_string())
            } else if had_requests {
                "Submitted the next pending auth-refresh response.".to_string()
            } else {
                "No pending auth refresh requests".to_string()
            };
            append_chat_command_result(state, prompt, response, is_error)
        }
    }
}

fn run_chat_remote_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatRemoteComposerIntent,
) -> bool {
    remember_chat_command_prompt(state, &prompt);
    clear_chat_command_prompt(state);

    match intent {
        ChatRemoteComposerIntent::Summary => append_chat_command_result(
            state,
            prompt,
            crate::codex_remote::remote_status_lines(state).join("\n"),
            false,
        ),
        ChatRemoteComposerIntent::Enable { bind_addr } => {
            match crate::codex_remote::enable_remote_runtime(state, bind_addr.as_deref()) {
                Ok(mut message) => {
                    if let Some(pairing_url) = state.codex_remote.pairing_url.as_deref() {
                        let clipboard_note = match copy_to_clipboard(pairing_url) {
                            Ok(()) => "Pairing URL copied to clipboard.",
                            Err(error) => {
                                state.codex_remote.last_error =
                                    Some(format!("Failed to copy pairing URL: {error}"));
                                "Pairing URL copy failed."
                            }
                        };
                        message.push(' ');
                        message.push_str(clipboard_note);
                    }
                    append_chat_command_result(state, prompt, message, false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
        ChatRemoteComposerIntent::Disable => {
            let message = crate::codex_remote::disable_remote_runtime(state);
            append_chat_command_result(state, prompt, message, false)
        }
        ChatRemoteComposerIntent::RotateToken => {
            match crate::codex_remote::rotate_remote_runtime_token(state) {
                Ok(mut message) => {
                    if let Some(pairing_url) = state.codex_remote.pairing_url.as_deref() {
                        let clipboard_note = match copy_to_clipboard(pairing_url) {
                            Ok(()) => "Pairing URL copied to clipboard.",
                            Err(error) => {
                                state.codex_remote.last_error =
                                    Some(format!("Failed to copy pairing URL: {error}"));
                                "Pairing URL copy failed."
                            }
                        };
                        message.push(' ');
                        message.push_str(clipboard_note);
                    }
                    append_chat_command_result(state, prompt, message, false)
                }
                Err(error) => append_chat_command_result(state, prompt, error, true),
            }
        }
    }
}

fn active_terminal_thread_id(state: &crate::app_state::RenderState) -> Result<String, String> {
    state
        .autopilot_chat
        .active_thread_id
        .clone()
        .ok_or_else(|| "No active thread is selected for terminal control.".to_string())
}

fn terminal_workspace_for_thread(
    state: &crate::app_state::RenderState,
    thread_id: &str,
) -> Result<String, String> {
    let metadata = state.autopilot_chat.thread_metadata.get(thread_id);
    metadata
        .and_then(|value| value.workspace_root.clone())
        .or_else(|| metadata.and_then(|value| value.cwd.clone()))
        .or_else(|| current_chat_workspace_root(state))
        .or_else(|| current_chat_session_cwd(state))
        .ok_or_else(|| "No workspace is available for the active thread terminal.".to_string())
}

fn current_terminal_shell_label() -> String {
    std::env::var("SHELL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if cfg!(target_os = "windows") {
                Some("cmd".to_string())
            } else {
                Some("shell".to_string())
            }
        })
        .unwrap_or_else(|| "shell".to_string())
}

fn terminal_thread_label(
    state: &crate::app_state::RenderState,
    session: &crate::app_state::AutopilotTerminalSession,
) -> String {
    state
        .autopilot_chat
        .thread_metadata
        .get(&session.thread_id)
        .and_then(|metadata| metadata.thread_name.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| session.thread_id.clone())
}

fn format_terminal_session_inventory(state: &crate::app_state::RenderState) -> String {
    let sessions = state.autopilot_chat.terminal_session_inventory();
    if sessions.is_empty() {
        return "No terminal sessions yet. Use `/term open` inside a thread to start one."
            .to_string();
    }
    let mut lines = vec!["Terminal sessions:".to_string()];
    for session in sessions {
        let thread_label = terminal_thread_label(state, session);
        let pid = session
            .pid
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        lines.push(format!(
            "- {} [{}] pid:{} size:{}x{} ws:{}",
            thread_label,
            session.status.label(),
            pid,
            session.cols,
            session.rows,
            session.workspace_root
        ));
    }
    lines.join("\n")
}

fn queue_terminal_command(
    state: &mut crate::app_state::RenderState,
    command: crate::chat_terminal::ChatTerminalCommand,
) -> Result<(), String> {
    state.chat_terminal_worker.enqueue(command)
}

fn run_chat_terminal_action(
    state: &mut crate::app_state::RenderState,
    prompt: String,
    intent: ChatTerminalComposerIntent,
) -> bool {
    let active_thread_id = state.autopilot_chat.active_thread_id.clone();
    if let Some(thread_id) = active_thread_id.as_deref() {
        state
            .autopilot_chat
            .remember_submission_draft(thread_id, prompt.clone());
    }
    state.chat_inputs.composer.set_value(String::new());
    state.autopilot_chat.record_composer_draft(String::new());

    match intent {
        ChatTerminalComposerIntent::ListSessions => {
            let response = format_terminal_session_inventory(state);
            state
                .autopilot_chat
                .append_local_exchange(prompt, response, false);
            true
        }
        ChatTerminalComposerIntent::CleanClosed => {
            let removed = state.autopilot_chat.remove_inactive_terminal_sessions();
            let response = if removed == 0 {
                "No closed terminal sessions needed cleanup.".to_string()
            } else {
                format!("Removed {removed} closed terminal session(s).")
            };
            state
                .autopilot_chat
                .append_local_exchange(prompt, response, false);
            true
        }
        ChatTerminalComposerIntent::Clear => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            if state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .is_none()
            {
                state.autopilot_chat.append_local_exchange(
                    prompt,
                    "No terminal session exists for this thread.",
                    true,
                );
                return true;
            }
            state
                .autopilot_chat
                .clear_terminal_session_output(thread_id.as_str());
            state.autopilot_chat.append_local_exchange(
                prompt,
                "Cleared the active thread terminal buffer.",
                false,
            );
            true
        }
        ChatTerminalComposerIntent::Open => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            if state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .is_some_and(|session| session.status.is_active())
            {
                state.autopilot_chat.append_local_exchange(
                    prompt,
                    "A terminal session is already active for this thread. Use `/term restart` or `/term write <text>`.",
                    true,
                );
                return true;
            }
            let workspace = match terminal_workspace_for_thread(state, thread_id.as_str()) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            let (cols, rows) = crate::chat_terminal::default_terminal_size();
            state.autopilot_chat.prepare_terminal_session(
                thread_id.as_str(),
                workspace.clone(),
                current_terminal_shell_label(),
                cols,
                rows,
            );
            match queue_terminal_command(
                state,
                crate::chat_terminal::ChatTerminalCommand::Open {
                    thread_id: thread_id.clone(),
                    workspace: workspace.clone(),
                    cols,
                    rows,
                },
            ) {
                Ok(()) => state.autopilot_chat.append_local_exchange(
                    prompt,
                    format!("Opening thread terminal in `{workspace}`."),
                    false,
                ),
                Err(error) => {
                    state
                        .autopilot_chat
                        .record_terminal_session_failure(thread_id.as_str(), error.clone());
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                }
            }
            true
        }
        ChatTerminalComposerIntent::Write { text } => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            if !state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .is_some_and(|session| session.status.is_active())
            {
                state.autopilot_chat.append_local_exchange(
                    prompt,
                    "No active terminal session exists for this thread. Use `/term open` first.",
                    true,
                );
                return true;
            }
            match queue_terminal_command(
                state,
                crate::chat_terminal::ChatTerminalCommand::Write {
                    thread_id: thread_id.clone(),
                    text: text.clone(),
                },
            ) {
                Ok(()) => state.autopilot_chat.append_local_exchange(
                    prompt,
                    format!("Sent to thread terminal: `{}`", truncate_line(&text, 72)),
                    false,
                ),
                Err(error) => {
                    state
                        .autopilot_chat
                        .record_terminal_session_failure(thread_id.as_str(), error.clone());
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                }
            }
            true
        }
        ChatTerminalComposerIntent::Resize { cols, rows } => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            if !state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .is_some_and(|session| session.status.is_active())
            {
                state.autopilot_chat.append_local_exchange(
                    prompt,
                    "No active terminal session exists for this thread. Use `/term open` first.",
                    true,
                );
                return true;
            }
            let cols = crate::chat_terminal::normalize_terminal_cols(cols);
            let rows = crate::chat_terminal::normalize_terminal_rows(rows);
            state
                .autopilot_chat
                .resize_terminal_session(thread_id.as_str(), cols, rows);
            match queue_terminal_command(
                state,
                crate::chat_terminal::ChatTerminalCommand::Resize {
                    thread_id: thread_id.clone(),
                    cols,
                    rows,
                },
            ) {
                Ok(()) => state.autopilot_chat.append_local_exchange(
                    prompt,
                    format!("Resized the thread terminal to {}x{}.", cols, rows),
                    false,
                ),
                Err(error) => {
                    state
                        .autopilot_chat
                        .record_terminal_session_failure(thread_id.as_str(), error.clone());
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                }
            }
            true
        }
        ChatTerminalComposerIntent::Restart => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            let workspace = match terminal_workspace_for_thread(state, thread_id.as_str()) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            let (cols, rows) = state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .map(|session| (session.cols, session.rows))
                .unwrap_or_else(crate::chat_terminal::default_terminal_size);
            state.autopilot_chat.prepare_terminal_session(
                thread_id.as_str(),
                workspace.clone(),
                current_terminal_shell_label(),
                cols,
                rows,
            );
            match queue_terminal_command(
                state,
                crate::chat_terminal::ChatTerminalCommand::Restart {
                    thread_id: thread_id.clone(),
                    workspace: workspace.clone(),
                    cols,
                    rows,
                },
            ) {
                Ok(()) => state.autopilot_chat.append_local_exchange(
                    prompt,
                    format!("Restarting the thread terminal in `{workspace}`."),
                    false,
                ),
                Err(error) => {
                    state
                        .autopilot_chat
                        .record_terminal_session_failure(thread_id.as_str(), error.clone());
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                }
            }
            true
        }
        ChatTerminalComposerIntent::Close => {
            let thread_id = match active_terminal_thread_id(state) {
                Ok(value) => value,
                Err(error) => {
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                    return true;
                }
            };
            if state
                .autopilot_chat
                .terminal_session_for_thread(thread_id.as_str())
                .is_none()
            {
                state.autopilot_chat.append_local_exchange(
                    prompt,
                    "No terminal session exists for this thread.",
                    true,
                );
                return true;
            }
            match queue_terminal_command(
                state,
                crate::chat_terminal::ChatTerminalCommand::Close {
                    thread_id: thread_id.clone(),
                },
            ) {
                Ok(()) => state.autopilot_chat.append_local_exchange(
                    prompt,
                    "Closing the active thread terminal session.",
                    false,
                ),
                Err(error) => {
                    state
                        .autopilot_chat
                        .record_terminal_session_failure(thread_id.as_str(), error.clone());
                    state
                        .autopilot_chat
                        .append_local_exchange(prompt, error, true);
                }
            }
            true
        }
    }
}

fn chat_session_workspace_roots(state: &crate::app_state::RenderState) -> Vec<String> {
    let roots = normalized_policy_file_roots(state);
    if !roots.is_empty() {
        return roots;
    }
    current_chat_workspace_root(state).into_iter().collect()
}

pub(super) fn chat_session_approval_policy(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::AskForApproval> {
    Some(state.autopilot_chat.approval_mode)
}

pub(super) fn chat_session_thread_sandbox_mode(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::SandboxMode> {
    Some(state.autopilot_chat.sandbox_mode)
}

pub(super) fn chat_session_turn_sandbox_policy(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::SandboxPolicy> {
    match state.autopilot_chat.sandbox_mode {
        codex_client::SandboxMode::DangerFullAccess => dangerous_sandbox_policy(),
        codex_client::SandboxMode::ReadOnly => Some(codex_client::SandboxPolicy::ReadOnly),
        codex_client::SandboxMode::WorkspaceWrite => {
            let writable_roots = chat_session_workspace_roots(state);
            Some(codex_client::SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            })
        }
    }
}

pub(super) fn chat_session_service_tier(
    state: &crate::app_state::RenderState,
) -> Option<Option<codex_client::ServiceTier>> {
    state.autopilot_chat.service_tier.request_value()
}

pub(super) fn chat_session_personality(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::Personality> {
    state.autopilot_chat.personality.request_value()
}

pub(super) fn chat_session_reasoning_effort(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::ReasoningEffort> {
    let value = state.autopilot_chat.reasoning_effort.as_deref()?.trim();
    serde_json::from_str::<codex_client::ReasoningEffort>(&format!("\"{value}\"")).ok()
}

fn chat_session_concrete_model(state: &crate::app_state::RenderState) -> Option<String> {
    state
        .autopilot_chat
        .selected_model_override()
        .or_else(|| {
            state
                .autopilot_chat
                .active_thread_id
                .as_ref()
                .and_then(|thread_id| state.autopilot_chat.thread_metadata.get(thread_id))
                .and_then(|metadata| metadata.model.clone())
        })
        .or_else(|| {
            state
                .autopilot_chat
                .models
                .iter()
                .find(|value| !value.eq_ignore_ascii_case("auto"))
                .cloned()
        })
}

pub(super) fn chat_session_collaboration_mode(
    state: &crate::app_state::RenderState,
) -> Option<serde_json::Value> {
    let mode = match state.autopilot_chat.collaboration_mode {
        crate::app_state::AutopilotChatCollaborationMode::Off => return None,
        crate::app_state::AutopilotChatCollaborationMode::Default => "default",
        crate::app_state::AutopilotChatCollaborationMode::Plan => "plan",
    };
    let Some(model) = chat_session_concrete_model(state) else {
        return None;
    };
    Some(serde_json::json!({
        "mode": mode,
        "settings": {
            "model": model,
            "reasoning_effort": state.autopilot_chat.reasoning_effort.clone(),
            "developer_instructions": serde_json::Value::Null,
        }
    }))
}

fn active_goal_autonomy_policy(
    state: &crate::app_state::RenderState,
) -> Option<&crate::state::autopilot_goals::GoalAutonomyPolicy> {
    let active_run = state.goal_loop_executor.active_run.as_ref()?;
    state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .find(|goal| goal.goal_id == active_run.goal_id)
        .map(|goal| &goal.constraints.autonomy_policy)
}

fn normalized_policy_file_roots(state: &crate::app_state::RenderState) -> Vec<String> {
    let Some(policy) = active_goal_autonomy_policy(state) else {
        return Vec::new();
    };
    policy
        .allowed_file_roots
        .iter()
        .map(|root| root.trim())
        .filter(|root| !root.is_empty())
        .map(|root| root.to_string())
        .collect::<Vec<_>>()
}

pub(super) fn goal_scoped_turn_cwd(state: &crate::app_state::RenderState) -> Option<String> {
    normalized_policy_file_roots(state).into_iter().next()
}

pub(super) fn goal_scoped_turn_sandbox_policy(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::SandboxPolicy> {
    let roots = normalized_policy_file_roots(state);
    if roots.is_empty() {
        return dangerous_sandbox_policy();
    }
    Some(codex_client::SandboxPolicy::WorkspaceWrite {
        writable_roots: roots,
        network_access: true,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    })
}

pub(super) fn goal_scoped_thread_sandbox_mode(
    state: &crate::app_state::RenderState,
) -> Option<codex_client::SandboxMode> {
    if normalized_policy_file_roots(state).is_empty() {
        dangerous_sandbox_mode()
    } else {
        Some(codex_client::SandboxMode::WorkspaceWrite)
    }
}

fn parse_attachment_directive<'a>(line: &'a str, command: &str) -> Option<&'a str> {
    if line == command {
        return Some("");
    }
    let prefix = format!("{command} ");
    line.strip_prefix(prefix.as_str())
        .or_else(|| line.strip_prefix(format!("{command}\t").as_str()))
}

fn strip_wrapping_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len().saturating_sub(1)]
    } else {
        trimmed
    }
}

fn expand_attachment_path(raw: &str, cwd: Option<&str>) -> std::path::PathBuf {
    let normalized = strip_wrapping_quotes(raw);
    if let Some(home_relative) = normalized.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from)
    {
        return home.join(home_relative);
    }
    let path = std::path::PathBuf::from(normalized);
    if path.is_absolute() {
        return path;
    }
    if let Some(cwd) = cwd {
        return std::path::Path::new(cwd).join(path);
    }
    path
}

fn local_attachment_path_string(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn is_remote_image_reference(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("data:")
}

fn split_attachment_target_and_label(value: &str) -> (&str, Option<&str>) {
    let trimmed = value.trim();
    match trimmed.split_once('|') {
        Some((target, label)) => (target.trim(), Some(label.trim())),
        None => (trimmed, None),
    }
}

fn default_mention_name(path: &str) -> String {
    path.strip_prefix("app://")
        .or_else(|| path.strip_prefix("plugin://"))
        .or_else(|| path.strip_prefix("skill://"))
        .or_else(|| path.strip_prefix("mcp://"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.rsplit('/').next().unwrap_or(value).to_string())
        .or_else(|| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "attachment".to_string())
}

fn parse_turn_mention_attachment(
    raw: &str,
    cwd: Option<&str>,
) -> Result<TurnMentionAttachment, String> {
    let (target, label) = split_attachment_target_and_label(raw);
    let target = strip_wrapping_quotes(target);
    if target.is_empty() {
        return Err("`/mention` requires a path or app/plugin target".to_string());
    }
    let path = if target.starts_with("app://")
        || target.starts_with("plugin://")
        || target.starts_with("skill://")
        || target.starts_with("mcp://")
    {
        target.to_string()
    } else {
        let resolved = expand_attachment_path(target, cwd);
        if !resolved.exists() {
            return Err(format!(
                "Mention target does not exist: {}",
                resolved.display()
            ));
        }
        local_attachment_path_string(resolved.as_path())
    };
    let name = label
        .map(strip_wrapping_quotes)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| default_mention_name(path.as_str()));
    Ok(TurnMentionAttachment { name, path })
}

fn parse_turn_image_attachment(
    raw: &str,
    cwd: Option<&str>,
) -> Result<TurnImageAttachment, String> {
    let target = strip_wrapping_quotes(raw);
    if target.is_empty() {
        return Err("`/image` requires a local file path or URL".to_string());
    }
    if is_remote_image_reference(target) {
        return Ok(TurnImageAttachment::Remote {
            url: target.to_string(),
        });
    }
    let resolved = expand_attachment_path(target, cwd);
    if !resolved.exists() {
        return Err(format!(
            "Image attachment does not exist: {}",
            resolved.display()
        ));
    }
    if !resolved.is_file() {
        return Err(format!(
            "Image attachment must be a file: {}",
            resolved.display()
        ));
    }
    Ok(TurnImageAttachment::Local {
        path: std::fs::canonicalize(&resolved).unwrap_or(resolved),
    })
}

pub(super) fn parse_chat_turn_prompt(
    prompt: String,
    cwd: Option<&str>,
) -> (ParsedChatTurnPrompt, Option<String>) {
    let mut prompt_lines = Vec::new();
    let mut mention_attachments = Vec::new();
    let mut image_attachments = Vec::new();
    let mut mention_keys = std::collections::HashSet::new();
    let mut image_keys = std::collections::HashSet::new();
    let mut errors = Vec::new();

    for line in prompt.lines() {
        let trimmed = line.trim();
        if let Some(rest) = parse_attachment_directive(trimmed, "/mention") {
            match parse_turn_mention_attachment(rest, cwd) {
                Ok(attachment) => {
                    if mention_keys.insert(attachment.path.clone()) {
                        mention_attachments.push(attachment);
                    }
                }
                Err(error) => errors.push(error),
            }
            continue;
        }
        if let Some(rest) = parse_attachment_directive(trimmed, "/image") {
            match parse_turn_image_attachment(rest, cwd) {
                Ok(attachment) => {
                    let key = match &attachment {
                        TurnImageAttachment::Remote { url } => url.clone(),
                        TurnImageAttachment::Local { path } => path.display().to_string(),
                    };
                    if image_keys.insert(key) {
                        image_attachments.push(attachment);
                    }
                }
                Err(error) => errors.push(error),
            }
            continue;
        }
        prompt_lines.push(line.to_string());
    }

    let prompt_text = prompt_lines.join("\n").trim().to_string();
    (
        ParsedChatTurnPrompt {
            prompt_text,
            mention_attachments,
            image_attachments,
        },
        (!errors.is_empty()).then(|| errors.join(" | ")),
    )
}

pub(super) fn assemble_chat_turn_input(
    parsed_prompt: ParsedChatTurnPrompt,
    skill_attachments: Vec<TurnSkillAttachment>,
) -> (Vec<UserInput>, Option<String>) {
    let mut input = Vec::new();
    if !parsed_prompt.prompt_text.is_empty() {
        input.push(UserInput::Text {
            text: parsed_prompt.prompt_text,
            text_elements: Vec::new(),
        });
    }
    for mention in parsed_prompt.mention_attachments {
        input.push(UserInput::Mention {
            name: mention.name,
            path: mention.path,
        });
    }
    for image in parsed_prompt.image_attachments {
        match image {
            TurnImageAttachment::Remote { url } => input.push(UserInput::Image { url }),
            TurnImageAttachment::Local { path } => input.push(UserInput::LocalImage { path }),
        }
    }
    let mut last_errors = Vec::new();
    let mut sorted_attachments = skill_attachments;
    sorted_attachments.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut attached = std::collections::HashSet::new();
    for attachment in sorted_attachments {
        let dedupe_key = format!(
            "{}::{}",
            attachment.name.to_ascii_lowercase(),
            attachment.path
        );
        if !attached.insert(dedupe_key) {
            continue;
        }
        if !attachment.enabled && attachment.source == TurnSkillSource::UserSelected {
            let label = match attachment.source {
                TurnSkillSource::UserSelected => "Selected",
                TurnSkillSource::GoalAutoSelected => "Auto-selected",
                TurnSkillSource::PolicyRequired => "Required",
            };
            last_errors.push(format!(
                "{} skill '{}' is disabled; enable it first.",
                label, attachment.name
            ));
            continue;
        }
        input.push(UserInput::Skill {
            name: attachment.name,
            path: std::path::PathBuf::from(attachment.path),
        });
    }

    let joined_errors = (!last_errors.is_empty()).then(|| last_errors.join(" | "));
    (input, joined_errors)
}

fn sync_chat_thread_search_term(state: &mut crate::app_state::RenderState) {
    state.autopilot_chat.thread_filter_search_term = state
        .chat_inputs
        .thread_search
        .get_value()
        .trim()
        .to_string();
}

pub(super) fn run_chat_refresh_threads_action(state: &mut crate::app_state::RenderState) -> bool {
    sync_chat_thread_search_term(state);
    let cwd = current_chat_workspace_root(state).or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|value| value.into_os_string().into_string().ok())
    });
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
    sync_chat_composer_draft(state);
    let project_defaults = state
        .autopilot_chat
        .active_project()
        .map(|project| project.defaults.clone());
    let cwd = state
        .autopilot_chat
        .active_project()
        .map(|project| project.workspace_root.clone())
        .or_else(|| current_chat_workspace_root(state));
    let model_override = project_defaults
        .as_ref()
        .and_then(|defaults| defaults.model.clone())
        .or_else(|| state.autopilot_chat.selected_model_override());
    let command = crate::codex_lane::CodexLaneCommand::ThreadStart(ThreadStartParams {
        model: model_override,
        model_provider: None,
        service_tier: project_defaults
            .as_ref()
            .and_then(|defaults| defaults.service_tier.request_value())
            .or_else(|| chat_session_service_tier(state)),
        cwd,
        approval_policy: project_defaults
            .as_ref()
            .and_then(|defaults| defaults.approval_policy)
            .or_else(|| chat_session_approval_policy(state)),
        sandbox: project_defaults
            .as_ref()
            .and_then(|defaults| defaults.sandbox_mode)
            .or_else(|| chat_session_thread_sandbox_mode(state)),
        personality: project_defaults
            .as_ref()
            .and_then(|defaults| defaults.personality.request_value())
            .or_else(|| chat_session_personality(state)),
        ephemeral: None,
        dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
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

pub(super) fn run_chat_cycle_reasoning_effort_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let supported = state
        .codex_models
        .entries
        .iter()
        .find(|entry| entry.model == state.autopilot_chat.current_model())
        .map(|entry| entry.supported_reasoning_efforts.clone())
        .unwrap_or_default();
    state.autopilot_chat.cycle_reasoning_effort(&supported);
    true
}

pub(super) fn run_chat_cycle_service_tier_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_service_tier();
    true
}

pub(super) fn run_chat_cycle_personality_action(state: &mut crate::app_state::RenderState) -> bool {
    state.autopilot_chat.cycle_personality();
    true
}

pub(super) fn run_chat_cycle_collaboration_mode_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_collaboration_mode();
    true
}

pub(super) fn run_chat_cycle_approval_mode_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_approval_mode();
    true
}

pub(super) fn run_chat_cycle_sandbox_mode_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    state.autopilot_chat.cycle_sandbox_mode();
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
    let cwd = current_chat_session_cwd(state);
    let model_override = state.autopilot_chat.selected_model_override();
    let command = crate::codex_lane::CodexLaneCommand::ThreadFork(ThreadForkParams {
        thread_id,
        path: None,
        model: model_override,
        model_provider: None,
        service_tier: chat_session_service_tier(state),
        cwd,
        approval_policy: chat_session_approval_policy(state),
        sandbox: chat_session_thread_sandbox_mode(state),
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
    let name = state
        .autopilot_chat
        .suggested_thread_name(&thread_id)
        .unwrap_or_else(|| state.autopilot_chat.next_thread_name());
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

pub(super) fn run_chat_reload_thread_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to reload".to_string());
        return true;
    };
    let command = crate::codex_lane::CodexLaneCommand::ThreadRead(codex_client::ThreadReadParams {
        thread_id,
        include_turns: true,
    });
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.last_error = Some(error);
    } else {
        state.autopilot_chat.last_error = None;
    }
    true
}

pub(super) fn run_chat_open_workspace_in_editor_action(
    state: &mut crate::app_state::RenderState,
) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error =
            Some("No active thread workspace is available to open".to_string());
        return true;
    };
    let metadata = state.autopilot_chat.thread_metadata.get(&thread_id);
    let path = state
        .autopilot_chat
        .active_project()
        .map(|project| std::path::PathBuf::from(project.workspace_root.clone()))
        .or_else(|| {
            metadata
                .and_then(|metadata| metadata.workspace_root.as_deref())
                .map(std::path::PathBuf::from)
        })
        .or_else(|| {
            metadata
                .and_then(|metadata| metadata.cwd.as_deref())
                .map(std::path::PathBuf::from)
        })
        .or_else(|| {
            metadata
                .and_then(|metadata| metadata.path.as_deref())
                .map(std::path::PathBuf::from)
                .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        });
    let Some(path) = path else {
        state.autopilot_chat.last_error =
            Some("This thread does not have a workspace path to open".to_string());
        return true;
    };

    match open_path_in_editor_or_default_app(path.as_path()) {
        Ok(summary) => {
            state.autopilot_chat.last_error = None;
            state
                .autopilot_chat
                .set_copy_notice(std::time::Instant::now(), summary);
        }
        Err(error) => {
            state.autopilot_chat.last_error = Some(error);
        }
    }
    true
}

pub(super) fn run_chat_copy_last_output_action(state: &mut crate::app_state::RenderState) -> bool {
    let now = std::time::Instant::now();
    let latest_message_id = state
        .autopilot_chat
        .messages
        .iter()
        .rev()
        .find(|message| {
            message.role == crate::app_state::AutopilotRole::Codex
                && !message.content.trim().is_empty()
        })
        .map(|message| message.id);
    let Some(output) = latest_message_id.and_then(|message_id| {
        crate::panes::chat::transcript_message_copy_text_by_id(state, message_id)
    }) else {
        state.autopilot_chat.last_error = Some("No assistant output available to copy".to_string());
        return true;
    };
    let notice = match copy_to_clipboard(&output) {
        Ok(()) => "Copied latest assistant output".to_string(),
        Err(error) => format!("Failed to copy latest assistant output: {error}"),
    };
    state.autopilot_chat.set_copy_notice(now, notice);
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

pub(super) fn run_chat_implement_plan_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(artifact) = state.autopilot_chat.active_plan_artifact().cloned() else {
        state.autopilot_chat.last_error =
            Some("No saved plan artifact available for this thread".to_string());
        return true;
    };

    let mut prompt = String::from(
        "Implement the saved plan artifact for this thread. Use the steps below as the source of truth, execute them in order, and report progress as you go.\n",
    );
    if let Some(explanation) = artifact.explanation.as_deref() {
        if !explanation.trim().is_empty() {
            prompt.push_str("\nPlan context:\n");
            prompt.push_str(explanation.trim());
            prompt.push('\n');
        }
    }
    if !artifact.steps.is_empty() {
        prompt.push_str("\nSaved steps:\n");
        for step in &artifact.steps {
            prompt.push_str("- [");
            prompt.push_str(match step.status.as_str() {
                "completed" => "x",
                "inProgress" => "~",
                _ => " ",
            });
            prompt.push_str("] ");
            prompt.push_str(step.step.trim());
            prompt.push('\n');
        }
    }

    state.chat_inputs.composer.set_value(prompt.clone());
    state.autopilot_chat.record_composer_draft(prompt);
    run_chat_submit_action_with_trigger(
        state,
        crate::labor_orchestrator::CodexRunTrigger::PersonalAgent,
    )
}

pub(super) fn run_chat_review_action(state: &mut crate::app_state::RenderState) -> bool {
    let Some(thread_id) = active_thread_id(state) else {
        state.autopilot_chat.last_error = Some("No active thread to review".to_string());
        return true;
    };
    let command =
        crate::codex_lane::CodexLaneCommand::ReviewStart(codex_client::ReviewStartParams {
            thread_id,
            target: codex_client::ReviewTarget::UncommittedChanges,
            delivery: Some(codex_client::ReviewDelivery::Inline),
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
    match state.autopilot_chat.chat_browse_mode() {
        crate::app_state::ChatBrowseMode::Managed => state
            .autopilot_chat
            .select_managed_chat_channel_row_by_index(index),
        crate::app_state::ChatBrowseMode::DirectMessages => state
            .autopilot_chat
            .select_direct_message_room_by_index(index),
        crate::app_state::ChatBrowseMode::Autopilot => {
            sync_chat_composer_draft(state);
            if index == 0 {
                state.autopilot_chat.active_thread_id = None;
                state.autopilot_chat.reset_transcript_scroll();
                state.autopilot_chat.last_error = None;
                restore_chat_composer_draft(state);
                focus_chat_composer(state);
                return true;
            }

            let preview_index = index - 1;
            if preview_index >= CHAT_AUTOPILOT_THREAD_PREVIEW_LIMIT {
                return false;
            }

            let Some(target) = state.autopilot_chat.select_thread_by_index(preview_index) else {
                return false;
            };
            restore_chat_composer_draft(state);
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

            state
                .autopilot_chat
                .restore_session_preferences_from_thread(&target.thread_id);

            let command = crate::codex_lane::CodexLaneCommand::ThreadResume(ThreadResumeParams {
                thread_id: target.thread_id,
                model: state.autopilot_chat.selected_model_override(),
                model_provider: None,
                service_tier: chat_session_service_tier(state),
                cwd: target.cwd.or_else(|| current_chat_session_cwd(state)),
                approval_policy: chat_session_approval_policy(state),
                sandbox: chat_session_thread_sandbox_mode(state),
                personality: chat_session_personality(state),
                path: resume_path.map(std::path::PathBuf::from),
            });
            if let Err(error) = state.queue_codex_command(command) {
                state.autopilot_chat.last_error = Some(error);
                return true;
            }

            if let Some(thread_id) = state.autopilot_chat.active_thread_id.clone() {
                let read = crate::codex_lane::CodexLaneCommand::ThreadRead(
                    codex_client::ThreadReadParams {
                        thread_id,
                        include_turns: true,
                    },
                );
                if let Err(error) = state.queue_codex_command(read) {
                    state.autopilot_chat.last_error = Some(error);
                }
            }
            return true;
        }
    }
}

pub(super) fn run_chat_select_workspace_action(
    state: &mut crate::app_state::RenderState,
    index: usize,
) -> bool {
    if !state.autopilot_chat.chat_has_browseable_content() {
        return false;
    }
    state.autopilot_chat.select_chat_workspace_by_index(index)
}

pub(super) fn run_chat_toggle_category_action(
    state: &mut crate::app_state::RenderState,
    index: usize,
) -> bool {
    if state.autopilot_chat.chat_browse_mode() != crate::app_state::ChatBrowseMode::Managed {
        return false;
    }
    state
        .autopilot_chat
        .toggle_managed_chat_category_by_row_index(index)
}

pub(super) fn run_chat_approval_response_action(
    state: &mut crate::app_state::RenderState,
    decision: ApprovalDecision,
) -> bool {
    if let Some(request) = state.autopilot_chat.pop_command_approval() {
        let decision_label = format!("{:?}", decision);
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
            state.autopilot_chat.record_turn_timeline_event(format!(
                "command approval response: {}",
                decision_label
            ));
            state.autopilot_chat.record_turn_command_approval_response(
                request.turn_id.as_str(),
                request.item_id.as_str(),
                decision_label.as_str(),
                current_epoch_millis(),
            );
        }
        return true;
    }

    if let Some(request) = state.autopilot_chat.pop_file_change_approval() {
        let decision_label = format!("{:?}", decision);
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
            state
                .autopilot_chat
                .record_turn_file_change_approval_response(
                    request.turn_id.as_str(),
                    request.item_id.as_str(),
                    decision_label.as_str(),
                    current_epoch_millis(),
                );
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

    let request_id = request.request_id.clone();
    let tool_name = request.tool.clone();
    let call_id = request.call_id.clone();
    let labor_result = if super::tool_bridge::is_openagents_tool_namespace(&tool_name) {
        let envelope = super::tool_bridge::execute_openagents_tool_request(state, &request);
        state.autopilot_chat.record_turn_timeline_event(format!(
            "tool call auto-executed tool={} code={} success={}",
            tool_name, envelope.code, envelope.success
        ));
        Some((
            envelope.code.clone(),
            envelope.success,
            envelope.message.clone(),
            envelope.to_response(),
        ))
    } else {
        None
    };
    let response = if let Some((_, _, _, response)) = labor_result.as_ref() {
        response.clone()
    } else {
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: format!(
                    "OpenAgents desktop acknowledged tool '{}' for call '{}'",
                    tool_name, call_id
                ),
            }],
            success: true,
        }
    };

    let command = crate::codex_lane::CodexLaneCommand::ServerRequestToolCallRespond {
        request_id,
        response,
    };
    if let Err(error) = state.queue_codex_command(command) {
        state.autopilot_chat.pending_tool_calls.insert(0, request);
        state.autopilot_chat.last_error = Some(error);
    } else {
        state
            .autopilot_chat
            .record_turn_timeline_event("tool call response submitted");
        if let Some((response_code, success, response_message, _)) = labor_result {
            state.autopilot_chat.record_turn_tool_result(
                request.turn_id.as_str(),
                format!("{:?}", request.request_id).as_str(),
                request.call_id.as_str(),
                request.tool.as_str(),
                response_code.as_str(),
                success,
                response_message.as_str(),
                current_epoch_millis(),
            );
        } else {
            state.autopilot_chat.record_turn_tool_result(
                request.turn_id.as_str(),
                format!("{:?}", request.request_id).as_str(),
                request.call_id.as_str(),
                request.tool.as_str(),
                "manual_response_submitted",
                true,
                "manual tool response submitted",
                current_epoch_millis(),
            );
        }
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
            super::reducers::queue_codex_readiness_refresh(state, true, "manual refresh");
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
                sandbox_policy: dangerous_sandbox_policy(),
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

pub(super) fn run_cast_control_action(
    state: &mut crate::app_state::RenderState,
    action: CastControlPaneAction,
) -> bool {
    match action {
        CastControlPaneAction::RefreshStatus => {
            state.cast_control.load_state = crate::app_state::PaneLoadState::Ready;
            state.cast_control.last_error = None;
            state.cast_control.prereq_status = format!(
                "autotrade script: {} | process: {}",
                if cast_autotrade_script_path().is_file() {
                    "present"
                } else {
                    "missing"
                },
                if state.cast_control_process.is_some() {
                    "running"
                } else {
                    "idle"
                }
            );
            state.cast_control.last_action = Some("CAST status refreshed".to_string());
            publish_cast_activity_event(
                state,
                "refresh_status",
                crate::app_state::ActivityEventDomain::Network,
                "CAST status refreshed".to_string(),
                "Refreshed script/config/runtime status".to_string(),
            );
            true
        }
        CastControlPaneAction::RunCheck => queue_cast_autotrade_once(
            state,
            "run_check",
            "check",
            false,
            crate::app_state::ActivityEventDomain::Network,
        ),
        CastControlPaneAction::RunProve => queue_cast_autotrade_once(
            state,
            "run_prove",
            "check,prove",
            false,
            crate::app_state::ActivityEventDomain::Network,
        ),
        CastControlPaneAction::RunSignBroadcast => queue_cast_autotrade_once(
            state,
            if state.cast_control.broadcast_armed {
                "run_sign_broadcast"
            } else {
                "run_sign_safe"
            },
            "sign",
            state.cast_control.broadcast_armed,
            crate::app_state::ActivityEventDomain::Wallet,
        ),
        CastControlPaneAction::RunInspect => queue_cast_autotrade_once(
            state,
            "run_inspect",
            "inspect",
            false,
            crate::app_state::ActivityEventDomain::Network,
        ),
        CastControlPaneAction::RunLoopOnce => queue_cast_autotrade_once(
            state,
            "run_loop_once",
            "check,prove,sign,inspect",
            state.cast_control.broadcast_armed,
            crate::app_state::ActivityEventDomain::Network,
        ),
        CastControlPaneAction::ToggleAutoLoop => {
            if state.cast_control.auto_loop_enabled {
                state.cast_control.stop_auto_loop();
                if let Some(mut process) = state.cast_control_process.take() {
                    let _ = process.child.kill();
                    let _ = process.child.wait();
                    state.cast_control.active_pid = None;
                }
                state.cast_control.load_state = crate::app_state::PaneLoadState::Ready;
                state.cast_control.last_action = Some("CAST auto loop stopped".to_string());
                publish_cast_activity_event(
                    state,
                    "toggle_auto_loop",
                    crate::app_state::ActivityEventDomain::Network,
                    "CAST auto loop stopped".to_string(),
                    "Disabled recurring CAST autotrade iterations".to_string(),
                );
            } else {
                state.cast_control.start_auto_loop();
                state.cast_control.load_state = crate::app_state::PaneLoadState::Ready;
                state.cast_control.last_action = Some("CAST auto loop enabled".to_string());
                publish_cast_activity_event(
                    state,
                    "toggle_auto_loop",
                    crate::app_state::ActivityEventDomain::Network,
                    "CAST auto loop enabled".to_string(),
                    "Recurring CAST autotrade iterations will run on pane interval".to_string(),
                );
            }
            true
        }
        CastControlPaneAction::ToggleBroadcastArmed => {
            state.cast_control.broadcast_armed = !state.cast_control.broadcast_armed;
            state.cast_control.load_state = crate::app_state::PaneLoadState::Ready;
            state.cast_control.last_error = None;
            state.cast_control.last_action = Some(if state.cast_control.broadcast_armed {
                "CAST broadcast armed".to_string()
            } else {
                "CAST broadcast disarmed".to_string()
            });
            publish_cast_activity_event(
                state,
                "toggle_broadcast_armed",
                crate::app_state::ActivityEventDomain::Wallet,
                state
                    .cast_control
                    .last_action
                    .clone()
                    .unwrap_or_else(|| "CAST broadcast toggle".to_string()),
                "Toggled CAST broadcast safety gate".to_string(),
            );
            true
        }
    }
}

fn cast_repo_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

fn cast_autotrade_script_path() -> std::path::PathBuf {
    cast_repo_root()
        .join("skills")
        .join("cast")
        .join("scripts")
        .join("cast-autotrade-loop.sh")
}

fn publish_cast_activity_event(
    state: &mut crate::app_state::RenderState,
    operation: &str,
    domain: crate::app_state::ActivityEventDomain,
    summary: String,
    detail: String,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let now_epoch_seconds = now.as_secs();
    let event_id = format!("cast:{operation}:{}", now.as_millis());
    state
        .activity_feed
        .upsert_event(crate::app_state::ActivityEventRow {
            event_id,
            domain,
            source_tag: "cast.dex".to_string(),
            occurred_at_epoch_seconds: now_epoch_seconds,
            summary: summary.clone(),
            detail,
        });
    state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
    state.provider_runtime.last_result = Some(summary);
}

fn queue_cast_autotrade_once(
    state: &mut crate::app_state::RenderState,
    operation: &str,
    stages: &str,
    broadcast: bool,
    domain: crate::app_state::ActivityEventDomain,
) -> bool {
    if state.cast_control_process.is_some() {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
        state.cast_control.last_error = Some(
            "CAST command already running; wait for current run to finish or stop auto loop."
                .to_string(),
        );
        return true;
    }

    let repo_root = cast_repo_root();
    let script_path = cast_autotrade_script_path();
    if !script_path.is_file() {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
        state.cast_control.last_error = Some(format!(
            "CAST autotrade script not found: {}",
            script_path.display()
        ));
        return true;
    }

    let run_latest_dir = repo_root.join("run").join("latest");
    let receipt_dir = run_latest_dir.join("receipts");
    let log_dir = run_latest_dir.join("logs");
    if let Err(error) = std::fs::create_dir_all(&receipt_dir) {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
        state.cast_control.last_error = Some(format!(
            "failed to create CAST receipt dir {}: {}",
            receipt_dir.display(),
            error
        ));
        return true;
    }
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
        state.cast_control.last_error = Some(format!(
            "failed to create CAST log dir {}: {}",
            log_dir.display(),
            error
        ));
        return true;
    }

    let receipt_path = receipt_dir.join(format!("{operation}.json"));
    let log_path = log_dir.join(format!("{operation}.log"));
    let log_file = match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
    {
        Ok(file) => file,
        Err(error) => {
            state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
            state.cast_control.last_error = Some(format!(
                "failed to open CAST log file {}: {}",
                log_path.display(),
                error
            ));
            return true;
        }
    };

    let mut command = std::process::Command::new("bash");
    command
        .current_dir(&repo_root)
        .arg(script_path.as_os_str())
        .arg("--once")
        .arg("--stages")
        .arg(stages)
        .arg("--summary-file")
        .arg(receipt_path.as_os_str());

    let config_trimmed = state.cast_control.loop_config_path.trim();
    if !config_trimmed.is_empty() {
        let config_path = {
            let path = std::path::PathBuf::from(config_trimmed);
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        };
        if config_path.is_file() {
            state.cast_control.prereq_status = format!("config loaded: {}", config_path.display());
            command.arg("--config").arg(config_path.as_os_str());
        } else {
            state.cast_control.prereq_status = format!(
                "config missing: {} (falling back to process env)",
                config_path.display()
            );
        }
    } else {
        state.cast_control.prereq_status = "config unset (using process env)".to_string();
    }

    if broadcast {
        command.arg("--broadcast");
    }

    let stderr_file = match log_file.try_clone() {
        Ok(file) => file,
        Err(error) => {
            state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
            state.cast_control.last_error = Some(format!(
                "failed to duplicate CAST log handle {}: {}",
                log_path.display(),
                error
            ));
            return true;
        }
    };

    command.stdout(std::process::Stdio::from(log_file));
    command.stderr(std::process::Stdio::from(stderr_file));

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
            state.cast_control.last_error =
                Some(format!("failed to spawn CAST command: {}", error));
            return true;
        }
    };

    let pid = child.id();
    state.cast_control_process = Some(crate::app_state::CastControlProcess {
        child,
        operation: operation.to_string(),
        receipt_path: receipt_path.display().to_string(),
        log_path: log_path.display().to_string(),
    });
    state.cast_control.load_state = crate::app_state::PaneLoadState::Loading;
    state.cast_control.last_error = None;
    state.cast_control.last_operation = Some(operation.to_string());
    state.cast_control.last_receipt_path = Some(receipt_path.display().to_string());
    state.cast_control.last_log_path = Some(log_path.display().to_string());
    state.cast_control.active_pid = Some(pid.to_string());
    state.cast_control.run_count = state.cast_control.run_count.saturating_add(1);
    state.cast_control.last_action = Some(format!(
        "CAST {} started (pid {})",
        operation.replace('_', " "),
        pid
    ));

    publish_cast_activity_event(
        state,
        operation,
        domain,
        state
            .cast_control
            .last_action
            .clone()
            .unwrap_or_else(|| "CAST command started".to_string()),
        format!(
            "stages={} broadcast={} receipt={} log={}",
            stages,
            broadcast,
            receipt_path.display(),
            log_path.display()
        ),
    );
    true
}

pub(super) fn run_cast_control_process_tick(state: &mut crate::app_state::RenderState) -> bool {
    let status = {
        let Some(process) = state.cast_control_process.as_mut() else {
            return false;
        };
        match process.child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => return false,
            Err(error) => {
                state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
                state.cast_control.last_error =
                    Some(format!("failed to poll CAST process: {}", error));
                let _ = state.cast_control_process.take();
                state.cast_control.active_pid = None;
                return true;
            }
        }
    };

    let Some(process) = state.cast_control_process.take() else {
        return false;
    };
    state.cast_control.active_pid = None;
    state.cast_control.last_receipt_path = Some(process.receipt_path.clone());
    state.cast_control.last_log_path = Some(process.log_path.clone());

    let mut summary_ok = status.success();
    let mut summary_detail = format!("exit_status={}", status);
    if let Ok(raw) = std::fs::read_to_string(&process.receipt_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        if let Some(ok) = value.get("ok").and_then(serde_json::Value::as_bool) {
            summary_ok = summary_ok && ok;
        }
        if let Some(stage) = value
            .get("failed_stage")
            .and_then(serde_json::Value::as_str)
            .filter(|stage| !stage.is_empty())
        {
            let message = value
                .get("failed_message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("stage failure");
            summary_detail = format!("failed_stage={} message={}", stage, message);
        }
        let txid = value
            .get("last_broadcast_txid")
            .and_then(serde_json::Value::as_str)
            .filter(|txid| !txid.is_empty())
            .or_else(|| {
                value
                    .get("last_local_txid")
                    .and_then(serde_json::Value::as_str)
                    .filter(|txid| !txid.is_empty())
            });
        if let Some(txid) = txid {
            state.cast_control.last_txid = Some(txid.to_string());
        }
    }

    if summary_ok {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Ready;
        state.cast_control.last_error = None;
        state.cast_control.last_action = Some(format!(
            "CAST {} completed",
            process.operation.replace('_', " ")
        ));
    } else {
        state.cast_control.load_state = crate::app_state::PaneLoadState::Error;
        state.cast_control.last_error = Some(format!(
            "CAST {} failed ({}). See log: {}",
            process.operation.replace('_', " "),
            summary_detail,
            process.log_path
        ));
        state.cast_control.last_action = Some(format!(
            "CAST {} failed",
            process.operation.replace('_', " ")
        ));
    }

    publish_cast_activity_event(
        state,
        &format!("complete_{}", process.operation),
        if process.operation.contains("sign") || process.operation.contains("broadcast") {
            crate::app_state::ActivityEventDomain::Wallet
        } else {
            crate::app_state::ActivityEventDomain::Network
        },
        state
            .cast_control
            .last_action
            .clone()
            .unwrap_or_else(|| "CAST command completed".to_string()),
        format!(
            "{} | receipt={} | log={}",
            summary_detail, process.receipt_path, process.log_path
        ),
    );
    true
}

pub(super) fn run_auto_cast_control_loop(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if state.cast_control_process.is_some() {
        return false;
    }
    if !state.cast_control.should_run_auto_loop(now) {
        return false;
    }
    let changed = queue_cast_autotrade_once(
        state,
        "auto_loop_tick",
        "check,prove,sign,inspect",
        state.cast_control.broadcast_armed,
        crate::app_state::ActivityEventDomain::Network,
    );
    if state.cast_control_process.is_some() {
        state.cast_control.mark_auto_loop_tick(now);
    } else {
        state.cast_control.stop_auto_loop();
        state.cast_control.last_action =
            Some("CAST auto loop stopped after launch failure".to_string());
    }
    changed
}

const STABLE_SATS_REAL_ROUND_MIN_INTERVAL_SECONDS: u64 = 5;
const STABLE_SATS_REAL_ROUND_MAX_STEPS: usize = 1;
const STABLE_SATS_REAL_ROUND_MAX_TRANSFER_SATS: u64 = 25_000;
const STABLE_SATS_REAL_ROUND_MAX_TRANSFER_CENTS: u64 = 8_000;
const STABLE_SATS_REAL_ROUND_MAX_CONVERT_SATS: u64 = 18_000;
const STABLE_SATS_REAL_ROUND_MAX_CONVERT_CENTS: u64 = 4_000;
const STABLE_SATS_REAL_ROUND_PERIOD_WINDOW_SECONDS: u64 = 3_600;
const STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_SATS: u64 = 120_000;
const STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_CENTS: u64 = 25_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StableSatsRealRoundStep {
    TransferBtc { from_index: usize, to_index: usize },
    TransferUsd { from_index: usize, to_index: usize },
    ConvertBtcToUsd { owner_index: usize },
    ConvertUsdToBtc { owner_index: usize },
}

impl StableSatsRealRoundStep {
    fn operation_kind(self) -> crate::app_state::StableSatsTreasuryOperationKind {
        match self {
            Self::TransferBtc { .. } => {
                crate::app_state::StableSatsTreasuryOperationKind::TransferBtc
            }
            Self::TransferUsd { .. } => {
                crate::app_state::StableSatsTreasuryOperationKind::TransferUsd
            }
            Self::ConvertBtcToUsd { .. } | Self::ConvertUsdToBtc { .. } => {
                crate::app_state::StableSatsTreasuryOperationKind::Convert
            }
        }
    }

    fn policy_tag(self) -> &'static str {
        match self {
            Self::TransferBtc { .. } => "transfer_btc",
            Self::TransferUsd { .. } => "transfer_usd",
            Self::ConvertBtcToUsd { .. } => "convert_btc_to_usd",
            Self::ConvertUsdToBtc { .. } => "convert_usd_to_btc",
        }
    }
}

fn queue_stable_sats_real_mode_round(state: &mut crate::app_state::RenderState) -> bool {
    if state.stable_sats_simulation.auto_run_enabled {
        state.stable_sats_simulation.stop_auto_run();
    }
    if state.stable_sats_simulation.live_refresh_pending {
        let reason = "Real mode round blocked: live refresh already pending".to_string();
        state.stable_sats_simulation.last_error = Some(reason.clone());
        state.stable_sats_simulation.last_action = Some(reason);
        state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
        return true;
    }

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    if !state.stable_sats_simulation.has_settled_live_refresh() {
        state.stable_sats_simulation.record_runtime_event(
            "NIP-SA",
            format!("sa:round:block:needs-refresh:{now_epoch_seconds}"),
            "Real mode round requires initial live refresh".to_string(),
        );
        return queue_stable_sats_live_refresh(state);
    }
    if let Some(last_orchestration) = state
        .stable_sats_simulation
        .treasury_operations
        .iter()
        .rev()
        .find(|entry| {
            matches!(
                entry.kind,
                crate::app_state::StableSatsTreasuryOperationKind::TransferBtc
                    | crate::app_state::StableSatsTreasuryOperationKind::TransferUsd
                    | crate::app_state::StableSatsTreasuryOperationKind::Convert
            )
        })
    {
        let elapsed = now_epoch_seconds.saturating_sub(last_orchestration.updated_at_epoch_seconds);
        if elapsed < STABLE_SATS_REAL_ROUND_MIN_INTERVAL_SECONDS {
            let reason = format!(
                "Real mode round blocked by rate policy: wait {}s",
                STABLE_SATS_REAL_ROUND_MIN_INTERVAL_SECONDS.saturating_sub(elapsed)
            );
            state.stable_sats_simulation.last_error = Some(reason.clone());
            state.stable_sats_simulation.last_action = Some(reason.clone());
            state.stable_sats_simulation.record_runtime_event(
                "NIP-SA",
                format!("sa:round:block:rate:{now_epoch_seconds}"),
                reason.clone(),
            );
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            return true;
        }
    }

    let operator_index = match stable_sats_real_round_topology(state) {
        Ok(value) => value,
        Err(reason) => {
            state.stable_sats_simulation.last_error = Some(reason.clone());
            state.stable_sats_simulation.last_action = Some(reason);
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            return true;
        }
    };
    let phase = stable_sats_real_round_phase(state);
    let round_id = state.stable_sats_simulation.rounds_run.saturating_add(1);
    let sa_evidence = stable_sats_sa_tick_evidence(state);
    let (operator_name, operator_btc_sats, operator_usd_cents) = {
        let operator = state
            .stable_sats_simulation
            .agents
            .get(operator_index)
            .expect("operator index should be valid after topology check");
        (
            operator.agent_name.clone(),
            operator.btc_balance_sats,
            operator.usd_balance_cents,
        )
    };
    let can_convert_btc = operator_btc_sats.saturating_sub(320) >= 300 && operator_btc_sats >= 300;
    let can_convert_usd = operator_usd_cents.saturating_sub(90) >= 80 && operator_usd_cents >= 80;
    let preferred_step = if phase == 0 {
        StableSatsRealRoundStep::ConvertBtcToUsd {
            owner_index: operator_index,
        }
    } else {
        StableSatsRealRoundStep::ConvertUsdToBtc {
            owner_index: operator_index,
        }
    };
    let fallback_step = if phase == 0 {
        StableSatsRealRoundStep::ConvertUsdToBtc {
            owner_index: operator_index,
        }
    } else {
        StableSatsRealRoundStep::ConvertBtcToUsd {
            owner_index: operator_index,
        }
    };
    let step_is_available = |step: StableSatsRealRoundStep| match step {
        StableSatsRealRoundStep::ConvertBtcToUsd { .. } => can_convert_btc,
        StableSatsRealRoundStep::ConvertUsdToBtc { .. } => can_convert_usd,
        StableSatsRealRoundStep::TransferBtc { .. }
        | StableSatsRealRoundStep::TransferUsd { .. } => false,
    };
    let steps = if step_is_available(preferred_step) {
        vec![preferred_step]
    } else if step_is_available(fallback_step) {
        vec![fallback_step]
    } else {
        let reason = format!(
            "policy guard convert blocked: {operator_name} available {} / {} below minimums (>=₿300 or >=$0.80)",
            format_sats_amount(operator_btc_sats),
            format_usd_cents(operator_usd_cents)
        );
        state.stable_sats_simulation.last_error = Some(reason.clone());
        state.stable_sats_simulation.last_action = Some(reason.clone());
        state.stable_sats_simulation.record_runtime_event(
            "NIP-SA",
            format!("sa:round:{round_id:04}:blocked"),
            reason,
        );
        state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
        return true;
    };
    state.stable_sats_simulation.record_runtime_event(
        "NIP-SA",
        format!("sa:round:{round_id:04}:plan"),
        format!(
            "planned phase={} steps={} sa_evidence={}",
            phase,
            steps.len(),
            sa_evidence
        ),
    );

    let mut queued_steps = 0usize;
    let mut blocked_reason: Option<String> = None;
    for (step_index, step) in steps
        .iter()
        .copied()
        .take(STABLE_SATS_REAL_ROUND_MAX_STEPS)
        .enumerate()
    {
        match queue_stable_sats_real_round_step(
            state,
            now_epoch_seconds,
            round_id,
            step_index,
            step,
            sa_evidence.as_str(),
        ) {
            Ok(request_id) => {
                queued_steps = queued_steps.saturating_add(1);
                state.stable_sats_simulation.record_runtime_event(
                    "NIP-SA",
                    format!("sa:round:{round_id:04}:step:{step_index}:queued"),
                    format!(
                        "queued {} request={} evidence={}",
                        step.policy_tag(),
                        request_id,
                        sa_evidence
                    ),
                );
            }
            Err(reason) => {
                record_stable_sats_real_round_policy_block(
                    state,
                    now_epoch_seconds,
                    round_id,
                    step_index,
                    step,
                    sa_evidence.as_str(),
                    reason.clone(),
                );
                blocked_reason = Some(reason);
                break;
            }
        }
    }

    if queued_steps > 0 {
        let _ = queue_stable_sats_live_refresh(state);
    }

    let summary = if let Some(reason) = blocked_reason.as_ref() {
        format!(
            "Real mode round {round_id} partially queued ({} step(s)); blocked: {}",
            queued_steps, reason
        )
    } else {
        format!(
            "Real mode round {round_id} queued {} step(s) with SA evidence {}",
            queued_steps, sa_evidence
        )
    };
    state.stable_sats_simulation.last_action = Some(summary.clone());
    state.stable_sats_simulation.last_error = blocked_reason.clone();
    state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
    state
        .activity_feed
        .upsert_event(crate::app_state::ActivityEventRow {
            event_id: format!("stablesats:sa-round:{round_id}:{now_epoch_seconds}"),
            domain: crate::app_state::ActivityEventDomain::Wallet,
            source_tag: "stablesats.sa".to_string(),
            summary: if blocked_reason.is_some() {
                "StableSats real round policy-blocked".to_string()
            } else {
                "StableSats real round queued".to_string()
            },
            detail: summary,
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
    state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
    true
}

fn stable_sats_real_round_topology(state: &crate::app_state::RenderState) -> Result<usize, String> {
    let Some(operator_index) = state
        .stable_sats_simulation
        .agents
        .iter()
        .position(|wallet| {
            wallet.owner_kind == crate::app_state::StableSatsWalletOwnerKind::Operator
        })
    else {
        return Err("Real mode round blocked: operator wallet is not configured".to_string());
    };
    Ok(operator_index)
}

fn stable_sats_real_round_phase(state: &crate::app_state::RenderState) -> usize {
    let operation_count = state
        .stable_sats_simulation
        .treasury_operations
        .iter()
        .filter(|entry| entry.kind == crate::app_state::StableSatsTreasuryOperationKind::Convert)
        .count();
    stable_sats_real_round_phase_from_operation_count(operation_count)
}

fn stable_sats_real_round_phase_from_operation_count(operation_count: usize) -> usize {
    (operation_count / STABLE_SATS_REAL_ROUND_MAX_STEPS).wrapping_rem(2)
}

fn stable_sats_sa_tick_evidence(state: &crate::app_state::RenderState) -> String {
    state
        .sa_lane
        .last_tick_result_event_id
        .clone()
        .or_else(|| state.sa_lane.last_tick_request_event_id.clone())
        .unwrap_or_else(|| format!("sa:tick:{}", state.sa_lane.tick_count.saturating_add(1)))
}

fn queue_stable_sats_real_round_step(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
    round_id: u32,
    step_index: usize,
    step: StableSatsRealRoundStep,
    sa_evidence: &str,
) -> Result<u64, String> {
    match step {
        StableSatsRealRoundStep::TransferBtc {
            from_index,
            to_index,
        } => {
            let source = state
                .stable_sats_simulation
                .agents
                .get(from_index)
                .ok_or_else(|| {
                    format!(
                        "policy guard transfer_btc failed: source wallet index {from_index} invalid"
                    )
                })?;
            let available = source.btc_balance_sats.saturating_sub(240);
            if available < 200 {
                return Err(format!(
                    "policy guard transfer_btc blocked: {} available {} below minimum",
                    source.agent_name,
                    format_sats_amount(source.btc_balance_sats)
                ));
            }
            let amount = (available / 10)
                .clamp(200, STABLE_SATS_REAL_ROUND_MAX_TRANSFER_SATS)
                .min(available);
            enqueue_stable_sats_transfer_worker(
                state,
                now_epoch_seconds,
                step,
                from_index,
                to_index,
                crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats,
                amount,
                Some(format!(
                    "sa-round-{round_id:04}-step-{step_index}-btc evidence={sa_evidence}"
                )),
                sa_evidence,
            )
        }
        StableSatsRealRoundStep::TransferUsd {
            from_index,
            to_index,
        } => {
            let source = state
                .stable_sats_simulation
                .agents
                .get(from_index)
                .ok_or_else(|| {
                    format!(
                        "policy guard transfer_usd failed: source wallet index {from_index} invalid"
                    )
                })?;
            let available = source.usd_balance_cents.saturating_sub(70);
            if available < 50 {
                return Err(format!(
                    "policy guard transfer_usd blocked: {} available {} below minimum",
                    source.agent_name,
                    format_usd_cents(source.usd_balance_cents)
                ));
            }
            let amount = (available / 10)
                .clamp(50, STABLE_SATS_REAL_ROUND_MAX_TRANSFER_CENTS)
                .min(available);
            enqueue_stable_sats_transfer_worker(
                state,
                now_epoch_seconds,
                step,
                from_index,
                to_index,
                crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents,
                amount,
                Some(format!(
                    "sa-round-{round_id:04}-step-{step_index}-usd evidence={sa_evidence}"
                )),
                sa_evidence,
            )
        }
        StableSatsRealRoundStep::ConvertBtcToUsd { owner_index } => {
            let owner = state
                .stable_sats_simulation
                .agents
                .get(owner_index)
                .ok_or_else(|| {
                    format!(
                        "policy guard convert_btc_to_usd failed: owner index {owner_index} invalid"
                    )
                })?;
            let available = owner.btc_balance_sats.saturating_sub(320);
            if available < 300 {
                return Err(format!(
                    "policy guard convert_btc_to_usd blocked: {} available {} below minimum",
                    owner.agent_name,
                    format_sats_amount(owner.btc_balance_sats)
                ));
            }
            let amount = (available / 12)
                .clamp(300, STABLE_SATS_REAL_ROUND_MAX_CONVERT_SATS)
                .min(available);
            let (window_sats, _window_cents) =
                stable_sats_period_convert_totals(state, now_epoch_seconds);
            if window_sats.saturating_add(amount) > STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_SATS {
                return Err(format!(
                    "policy guard convert_btc_to_usd blocked: hourly converted {} + {} exceeds {}",
                    format_sats_amount(window_sats),
                    format_sats_amount(amount),
                    format_sats_amount(STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_SATS)
                ));
            }
            enqueue_stable_sats_convert_worker(
                state,
                now_epoch_seconds,
                step,
                owner_index,
                "btc-to-usd",
                amount,
                "sats",
                Some(format!(
                    "sa-round-{round_id:04}-step-{step_index}-convert-btc evidence={sa_evidence}"
                )),
                sa_evidence,
            )
        }
        StableSatsRealRoundStep::ConvertUsdToBtc { owner_index } => {
            let owner = state
                .stable_sats_simulation
                .agents
                .get(owner_index)
                .ok_or_else(|| {
                    format!(
                        "policy guard convert_usd_to_btc failed: owner index {owner_index} invalid"
                    )
                })?;
            let available = owner.usd_balance_cents.saturating_sub(90);
            if available < 80 {
                return Err(format!(
                    "policy guard convert_usd_to_btc blocked: {} available {} below minimum",
                    owner.agent_name,
                    format_usd_cents(owner.usd_balance_cents)
                ));
            }
            let amount = (available / 12)
                .clamp(80, STABLE_SATS_REAL_ROUND_MAX_CONVERT_CENTS)
                .min(available);
            let (_window_sats, window_cents) =
                stable_sats_period_convert_totals(state, now_epoch_seconds);
            if window_cents.saturating_add(amount) > STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_CENTS
            {
                return Err(format!(
                    "policy guard convert_usd_to_btc blocked: hourly converted cents {} + {} exceeds {}",
                    window_cents, amount, STABLE_SATS_REAL_ROUND_MAX_PERIOD_CONVERT_CENTS
                ));
            }
            enqueue_stable_sats_convert_worker(
                state,
                now_epoch_seconds,
                step,
                owner_index,
                "usd-to-btc",
                amount,
                "cents",
                Some(format!(
                    "sa-round-{round_id:04}-step-{step_index}-convert-usd evidence={sa_evidence}"
                )),
                sa_evidence,
            )
        }
    }
}

fn payload_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|raw| {
        raw.as_u64()
            .or_else(|| raw.as_i64().and_then(|number| u64::try_from(number).ok()))
            .or_else(|| {
                raw.as_str()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            })
    })
}

fn format_usd_cents(usd_cents: u64) -> String {
    format!("${}.{:02}", usd_cents / 100, usd_cents % 100)
}

fn stable_sats_period_convert_totals(
    state: &crate::app_state::RenderState,
    now_epoch_seconds: u64,
) -> (u64, u64) {
    let cutoff = now_epoch_seconds.saturating_sub(STABLE_SATS_REAL_ROUND_PERIOD_WINDOW_SECONDS);
    stable_sats_period_convert_totals_from_receipts(
        state.stable_sats_simulation.treasury_receipts.as_slice(),
        cutoff,
    )
}

fn stable_sats_period_convert_totals_from_receipts(
    receipts: &[crate::app_state::StableSatsTreasuryReceipt],
    cutoff_epoch_seconds: u64,
) -> (u64, u64) {
    receipts
        .iter()
        .filter(|receipt| {
            receipt.kind == crate::app_state::StableSatsTreasuryOperationKind::Convert
                && receipt.occurred_at_epoch_seconds >= cutoff_epoch_seconds
        })
        .fold((0_u64, 0_u64), |(sats, cents), receipt| {
            let status = receipt
                .payload
                .get("status")
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().to_ascii_uppercase())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            if status != "SUCCESS" && status != "SETTLED" {
                return (sats, cents);
            }
            let amount = payload_u64(&receipt.payload, "amount").unwrap_or(0);
            let unit = receipt
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if unit == "sats" || unit == "sat" {
                (sats.saturating_add(amount), cents)
            } else if unit == "cents" || unit == "cent" {
                (sats, cents.saturating_add(amount))
            } else {
                (sats, cents)
            }
        })
}

fn enqueue_stable_sats_transfer_worker(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
    step: StableSatsRealRoundStep,
    from_index: usize,
    to_index: usize,
    asset: crate::stablesats_blink_worker::StableSatsBlinkTransferAsset,
    amount: u64,
    memo: Option<String>,
    sa_evidence: &str,
) -> Result<u64, String> {
    let from_wallet = state
        .stable_sats_simulation
        .agents
        .get(from_index)
        .ok_or_else(|| format!("source wallet index {} out of bounds", from_index))?
        .clone();
    let to_wallet = state
        .stable_sats_simulation
        .agents
        .get(to_index)
        .ok_or_else(|| format!("destination wallet index {} out of bounds", to_index))?
        .clone();
    let source_env_overrides = resolve_wallet_blink_env(state, &from_wallet)?;
    let destination_env_overrides = resolve_wallet_blink_env(state, &to_wallet)?;
    let balance_script_path = resolve_blink_script_path(state, "balance.js")?;
    let create_invoice_script_path = resolve_blink_script_path(state, "create_invoice.js")?;
    let create_invoice_usd_script_path = resolve_blink_script_path(state, "create_invoice_usd.js")?;
    let fee_probe_script_path = resolve_blink_script_path(state, "fee_probe.js")?;
    let pay_invoice_script_path = resolve_blink_script_path(state, "pay_invoice.js")?;

    let request_id = state.stable_sats_simulation.reserve_worker_request_id();
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            request_id,
            step.operation_kind(),
            now_epoch_seconds,
            format!(
                "sa round queued {} amount={} from={} to={} evidence={}",
                step.policy_tag(),
                amount,
                from_wallet.owner_id,
                to_wallet.owner_id,
                sa_evidence
            ),
        );

    let request = crate::stablesats_blink_worker::StableSatsBlinkTransferRequest {
        request_id,
        now_epoch_seconds,
        from_owner_id: from_wallet.owner_id.clone(),
        from_wallet_name: from_wallet.agent_name.clone(),
        to_owner_id: to_wallet.owner_id.clone(),
        to_wallet_name: to_wallet.agent_name.clone(),
        asset,
        amount,
        memo,
        source_env_overrides,
        destination_env_overrides,
        balance_script_path,
        create_invoice_script_path,
        create_invoice_usd_script_path,
        fee_probe_script_path,
        pay_invoice_script_path,
    };
    if let Err(error) = state.stable_sats_blink_worker.enqueue_transfer(request) {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                request_id,
                step.operation_kind(),
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("sa round enqueue transfer failed: {error}"),
            );
        return Err(format!("failed queueing {}: {}", step.policy_tag(), error));
    }
    Ok(request_id)
}

fn enqueue_stable_sats_convert_worker(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
    step: StableSatsRealRoundStep,
    owner_index: usize,
    direction: &str,
    amount: u64,
    unit: &str,
    memo: Option<String>,
    sa_evidence: &str,
) -> Result<u64, String> {
    let wallet = state
        .stable_sats_simulation
        .agents
        .get(owner_index)
        .ok_or_else(|| format!("wallet index {} out of bounds", owner_index))?
        .clone();
    let env_overrides = resolve_wallet_blink_env(state, &wallet)?;
    let swap_quote_script_path = resolve_blink_script_path(state, "swap_quote.js")?;
    let swap_execute_script_path = resolve_blink_script_path(state, "swap_execute.js")?;

    let request_id = state.stable_sats_simulation.reserve_worker_request_id();
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            request_id,
            step.operation_kind(),
            now_epoch_seconds,
            format!(
                "sa round queued {} amount={} {} owner={} evidence={}",
                step.policy_tag(),
                amount,
                unit,
                wallet.owner_id,
                sa_evidence
            ),
        );

    let request = crate::stablesats_blink_worker::StableSatsBlinkConvertRequest {
        request_id,
        now_epoch_seconds,
        owner_id: wallet.owner_id.clone(),
        wallet_name: wallet.agent_name.clone(),
        direction: direction.to_string(),
        amount,
        unit: unit.to_string(),
        memo,
        env_overrides,
        swap_execute_script_path,
        swap_quote_script_path,
    };
    if let Err(error) = state.stable_sats_blink_worker.enqueue_convert(request) {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                request_id,
                step.operation_kind(),
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("sa round enqueue convert failed: {error}"),
            );
        return Err(format!("failed queueing {}: {}", step.policy_tag(), error));
    }
    Ok(request_id)
}

fn record_stable_sats_real_round_policy_block(
    state: &mut crate::app_state::RenderState,
    now_epoch_seconds: u64,
    round_id: u32,
    step_index: usize,
    step: StableSatsRealRoundStep,
    sa_evidence: &str,
    reason: String,
) {
    let request_id = state.stable_sats_simulation.reserve_worker_request_id();
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            request_id,
            step.operation_kind(),
            now_epoch_seconds,
            format!(
                "policy blocked {} round={} step={} evidence={}",
                step.policy_tag(),
                round_id,
                step_index,
                sa_evidence
            ),
        );
    state
        .stable_sats_simulation
        .record_treasury_operation_finished(
            request_id,
            step.operation_kind(),
            crate::app_state::StableSatsTreasuryOperationStatus::Failed,
            now_epoch_seconds,
            reason.clone(),
        );
    state.stable_sats_simulation.record_treasury_receipt(
        request_id,
        step.operation_kind(),
        now_epoch_seconds,
        serde_json::json!({
            "status": "blocked",
            "reason": reason,
            "policy_tag": step.policy_tag(),
            "round_id": round_id,
            "step_index": step_index,
            "sa_tick_evidence": sa_evidence,
        }),
    );
    state.stable_sats_simulation.record_runtime_event(
        "NIP-SA",
        format!("sa:round:{round_id:04}:step:{step_index}:blocked"),
        format!(
            "blocked {} reason={} evidence={}",
            step.policy_tag(),
            reason,
            sa_evidence
        ),
    );
}

fn queue_stable_sats_live_refresh(state: &mut crate::app_state::RenderState) -> bool {
    if state.stable_sats_simulation.auto_run_enabled {
        state.stable_sats_simulation.stop_auto_run();
    }

    let request_id = match state.stable_sats_simulation.begin_live_refresh() {
        Ok(request_id) => request_id,
        Err(error) => {
            state.stable_sats_simulation.last_error = Some(error.clone());
            state.stable_sats_simulation.last_action = Some(error);
            state.stable_sats_simulation.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            return true;
        }
    };

    let balance_script = match resolve_blink_script_path(state, "balance.js") {
        Ok(path) => path,
        Err(error) => {
            let _ = state
                .stable_sats_simulation
                .fail_live_refresh(request_id, error);
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            return true;
        }
    };
    let price_script = match resolve_blink_script_path(state, "price.js") {
        Ok(path) => path,
        Err(error) => {
            let _ = state
                .stable_sats_simulation
                .fail_live_refresh(request_id, error);
            state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
            return true;
        }
    };
    let (wallet_requests, scoped_wallet_errors) = resolve_blink_wallet_refresh_requests(state);
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    apply_scoped_wallet_refresh_errors(state, now_epoch_seconds, scoped_wallet_errors.as_slice());
    if wallet_requests.is_empty() {
        let _ = state.stable_sats_simulation.fail_live_refresh(
            request_id,
            "No live Blink wallets resolved from secure credential bindings".to_string(),
        );
        state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
        return true;
    }

    let request = crate::stablesats_blink_worker::StableSatsBlinkRefreshRequest {
        request_id,
        now_epoch_seconds,
        balance_script_path: balance_script,
        price_script_path: price_script,
        wallets: wallet_requests,
        preflight_failures: scoped_wallet_errors,
    };
    if let Err(error) = state.stable_sats_blink_worker.enqueue_refresh(request) {
        let _ = state
            .stable_sats_simulation
            .fail_live_refresh(request_id, error);
    }

    state.provider_runtime.last_result = state.stable_sats_simulation.last_action.clone();
    true
}

fn resolve_blink_wallet_refresh_requests(
    state: &crate::app_state::RenderState,
) -> (
    Vec<crate::stablesats_blink_worker::StableSatsBlinkWalletRefreshRequest>,
    Vec<crate::stablesats_blink_worker::StableSatsBlinkWalletFailure>,
) {
    let mut requests =
        Vec::<crate::stablesats_blink_worker::StableSatsBlinkWalletRefreshRequest>::new();
    let mut scoped_errors =
        Vec::<crate::stablesats_blink_worker::StableSatsBlinkWalletFailure>::new();
    for wallet in &state.stable_sats_simulation.agents {
        match resolve_wallet_blink_env(state, wallet) {
            Ok(env_overrides) => {
                requests.push(
                    crate::stablesats_blink_worker::StableSatsBlinkWalletRefreshRequest {
                        owner_id: wallet.owner_id.clone(),
                        wallet_name: wallet.agent_name.clone(),
                        env_overrides,
                    },
                );
            }
            Err(error) => {
                scoped_errors.push(
                    crate::stablesats_blink_worker::StableSatsBlinkWalletFailure {
                        owner_id: wallet.owner_id.clone(),
                        wallet_name: wallet.agent_name.clone(),
                        error,
                    },
                );
            }
        }
    }
    (requests, scoped_errors)
}

fn resolve_wallet_blink_env(
    state: &crate::app_state::RenderState,
    wallet: &crate::app_state::StableSatsAgentWalletState,
) -> Result<Vec<(String, String)>, String> {
    let mut secure_values = std::collections::BTreeMap::<String, String>::new();
    let key_name = crate::credentials::normalize_env_var_name(wallet.credential_key_name.as_str());
    if let Some(value) = state
        .credentials
        .read_secure_value(wallet.credential_key_name.as_str())
        .map_err(|error| {
            format!(
                "{} secure credential read failed for {}: {error}",
                wallet.agent_name, wallet.credential_key_name
            )
        })?
        .filter(|value| !value.trim().is_empty())
    {
        secure_values.insert(key_name, value);
    }
    if let Some(url_name) = wallet.credential_url_name.as_deref() {
        let normalized_url_name = crate::credentials::normalize_env_var_name(url_name);
        if let Some(value) = state
            .credentials
            .read_secure_value(url_name)
            .map_err(|error| {
                format!(
                    "{} secure credential read failed for {}: {error}",
                    wallet.agent_name, url_name
                )
            })?
            .filter(|value| !value.trim().is_empty())
        {
            secure_values.insert(normalized_url_name, value);
        }
    }
    resolve_wallet_blink_env_from_secure_values(
        wallet,
        state.credentials.entries.as_slice(),
        &secure_values,
    )
}

fn ensure_wallet_credential_slot_enabled(
    entries: &[crate::credentials::CredentialRecord],
    credential_name: &str,
) -> Result<(), String> {
    let normalized = crate::credentials::normalize_env_var_name(credential_name);
    let Some(entry) = entries.iter().find(|entry| entry.name == normalized) else {
        return Err(format!(
            "Credential slot {} is missing from credential manager",
            normalized
        ));
    };
    if !entry.enabled {
        return Err(format!("Credential slot {} is disabled", normalized));
    }
    Ok(())
}

fn has_enabled_credential_slot(
    entries: &[crate::credentials::CredentialRecord],
    credential_name: &str,
) -> bool {
    let normalized = crate::credentials::normalize_env_var_name(credential_name);
    entries
        .iter()
        .any(|entry| entry.name == normalized && entry.enabled)
}

fn resolve_wallet_blink_env_from_secure_values(
    wallet: &crate::app_state::StableSatsAgentWalletState,
    entries: &[crate::credentials::CredentialRecord],
    secure_values: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<(String, String)>, String> {
    ensure_wallet_credential_slot_enabled(entries, wallet.credential_key_name.as_str())?;
    let normalized_key =
        crate::credentials::normalize_env_var_name(wallet.credential_key_name.as_str());
    let Some(api_key) = secure_values
        .get(normalized_key.as_str())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Err(format!(
            "{} missing secure value for {}",
            wallet.agent_name, wallet.credential_key_name
        ));
    };
    let mut env_overrides = vec![("BLINK_API_KEY".to_string(), api_key.to_string())];
    if let Some(url_name) = wallet.credential_url_name.as_deref()
        && has_enabled_credential_slot(entries, url_name)
    {
        let normalized_url = crate::credentials::normalize_env_var_name(url_name);
        if let Some(url) = secure_values
            .get(normalized_url.as_str())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            env_overrides.push(("BLINK_API_URL".to_string(), url.to_string()));
        }
    }
    Ok(env_overrides)
}

fn apply_scoped_wallet_refresh_errors(
    state: &mut crate::app_state::RenderState,
    _now_epoch_seconds: u64,
    scoped_errors: &[crate::stablesats_blink_worker::StableSatsBlinkWalletFailure],
) {
    let mut summaries = Vec::<String>::new();
    for scoped_error in scoped_errors {
        let owner_id = scoped_error.owner_id.as_str();
        let error = scoped_error.error.as_str();
        if let Some(wallet_index) = state
            .stable_sats_simulation
            .agents
            .iter()
            .position(|wallet| wallet.owner_id == owner_id)
        {
            let wallet_name = state.stable_sats_simulation.agents[wallet_index]
                .agent_name
                .clone();
            state.stable_sats_simulation.agents[wallet_index].last_switch_summary =
                format!("credential error: {error}");
            summaries.push(format!("{wallet_name}: {error}"));
        }
    }
    if !summaries.is_empty() {
        state.stable_sats_simulation.last_error = Some(format!(
            "Scoped wallet credential errors: {}",
            summaries.join(" | ")
        ));
    }
}

fn resolve_blink_script_path(
    state: &crate::app_state::RenderState,
    script_name: &str,
) -> Result<std::path::PathBuf, String> {
    let mut candidates = std::collections::BTreeSet::<std::path::PathBuf>::new();
    for skill in &state.skill_registry.discovered_skills {
        if !skill.enabled || !skill.name.eq_ignore_ascii_case("blink") {
            continue;
        }
        if let Some(root) = normalize_skill_root_path(skill.path.as_str()) {
            candidates.insert(root.join("scripts").join(script_name));
        }
    }

    if let Some(repo_root) = state.skill_registry.repo_skills_root.as_deref() {
        candidates.insert(
            std::path::PathBuf::from(repo_root)
                .join("blink")
                .join("scripts")
                .join(script_name),
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.insert(
            cwd.join("skills")
                .join("blink")
                .join("scripts")
                .join(script_name),
        );
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "Blink script '{}' not found in discovered skills",
                script_name
            )
        })
}

fn normalize_skill_root_path(raw_skill_path: &str) -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from(raw_skill_path.trim());
    if path
        .file_name()
        .map(|name| name.to_string_lossy().eq_ignore_ascii_case("SKILL.md"))
        .unwrap_or(false)
    {
        return path.parent().map(std::path::Path::to_path_buf);
    }
    if path.is_dir() {
        return Some(path);
    }
    path.parent().map(std::path::Path::to_path_buf)
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
            match state.settings.add_backup_relay(relay_url, true) {
                Ok(()) => {
                    state.relay_connections.replace_configured_relays(
                        state.settings.document.configured_relay_urls().as_slice(),
                    );
                    state.provider_runtime.last_result = state.settings.last_action.clone();
                    let _ = state.sync_provider_nip90_lane_relays();
                }
                Err(error) => {
                    state.relay_connections.last_error = Some(error);
                }
            }
        }
        RelayConnectionsPaneAction::RemoveSelected => {
            let selected = state.relay_connections.selected_url.clone();
            match selected
                .as_deref()
                .ok_or_else(|| "Select a relay first".to_string())
                .and_then(|relay_url| state.settings.remove_configured_relay(relay_url, true))
            {
                Ok(message) => {
                    state.relay_connections.replace_configured_relays(
                        state.settings.document.configured_relay_urls().as_slice(),
                    );
                    state.provider_runtime.last_result = Some(message);
                    let _ = state.sync_provider_nip90_lane_relays();
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
                    let _ = state.sync_provider_nip90_lane_relays();
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
            let worker_id = state.sync_lifecycle_worker_id.clone();
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor,
                Some("manual sync rebootstrap requested".to_string()),
            );
            state.sync_lifecycle.mark_replay_bootstrap(
                worker_id.as_str(),
                state.sync_health.cursor_position,
                Some(state.sync_health.cursor_position),
            );
            state.sync_lifecycle.mark_connecting(worker_id.as_str());
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
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
        NetworkRequestsPaneAction::RequestQuotes => {
            let delivery_start_minutes = match parse_non_negative_amount_str(
                state
                    .network_requests_inputs
                    .delivery_start_minutes
                    .get_value(),
                "Delivery start minutes",
            ) {
                Ok(value) => value,
                Err(error) => {
                    state.network_requests.clear_spot_quotes_with_error(error);
                    return true;
                }
            };
            if delivery_start_minutes > 0 {
                let rfq = match build_forward_compute_rfq_from_inputs(state, delivery_start_minutes)
                {
                    Ok(rfq) => rfq,
                    Err(error) => {
                        state
                            .network_requests
                            .clear_forward_quotes_with_error(error);
                        return true;
                    }
                };
                match crate::kernel_control::request_forward_compute_quotes(state, &rfq) {
                    Ok(quotes) => {
                        state
                            .network_requests
                            .replace_forward_quotes(rfq.clone(), quotes);
                        state.provider_runtime.last_result =
                            Some(format!("loaded compute quotes for {}", rfq.summary()));
                    }
                    Err(error) => {
                        state
                            .network_requests
                            .clear_forward_quotes_with_error(error.clone());
                        state.provider_runtime.last_error_detail = Some(error.clone());
                        state.provider_runtime.last_result =
                            Some(format!("compute quote request failed: {error}"));
                    }
                }
            } else {
                let rfq = match build_spot_compute_rfq_from_inputs(state) {
                    Ok(rfq) => rfq,
                    Err(error) => {
                        state.network_requests.clear_spot_quotes_with_error(error);
                        return true;
                    }
                };
                match crate::kernel_control::request_spot_compute_quotes(state, &rfq) {
                    Ok(quotes) => {
                        state
                            .network_requests
                            .replace_spot_quotes(rfq.clone(), quotes);
                        state.provider_runtime.last_result =
                            Some(format!("loaded compute quotes for {}", rfq.summary()));
                    }
                    Err(error) => {
                        state
                            .network_requests
                            .clear_spot_quotes_with_error(error.clone());
                        state.provider_runtime.last_error_detail = Some(error.clone());
                        state.provider_runtime.last_result =
                            Some(format!("compute quote request failed: {error}"));
                    }
                }
            }
            true
        }
        NetworkRequestsPaneAction::AcceptSelectedQuote => {
            match state.network_requests.quote_mode {
                crate::app_state::ComputeQuoteMode::Spot => {
                    let Some(rfq) = state.network_requests.last_spot_rfq.clone() else {
                        state
                            .network_requests
                            .clear_spot_quotes_with_error("Load quotes before accepting terms");
                        return true;
                    };
                    let Some(quote) = state.network_requests.selected_spot_quote().cloned() else {
                        state
                            .network_requests
                            .clear_spot_quotes_with_error("Select a quote before accepting terms");
                        return true;
                    };
                    match crate::kernel_control::accept_spot_compute_quote(state, &rfq, &quote) {
                        Ok(order) => {
                            let instrument_id = order.instrument_id.clone();
                            let quote_id = order.quote_id.clone();
                            state.network_requests.record_spot_order_acceptance(order);
                            state.provider_runtime.last_result = Some(format!(
                                "accepted compute quote {} -> {}",
                                quote_id, instrument_id
                            ));
                        }
                        Err(error) => {
                            state.network_requests.last_error = Some(error.clone());
                            state.network_requests.load_state =
                                crate::app_state::PaneLoadState::Error;
                            state.provider_runtime.last_error_detail = Some(error.clone());
                            state.provider_runtime.last_result =
                                Some(format!("compute quote acceptance failed: {error}"));
                        }
                    }
                }
                crate::app_state::ComputeQuoteMode::ForwardPhysical => {
                    let Some(rfq) = state.network_requests.last_forward_rfq.clone() else {
                        state
                            .network_requests
                            .clear_forward_quotes_with_error("Load quotes before accepting terms");
                        return true;
                    };
                    let Some(quote) = state.network_requests.selected_forward_quote().cloned()
                    else {
                        state.network_requests.clear_forward_quotes_with_error(
                            "Select a quote before accepting terms",
                        );
                        return true;
                    };
                    match crate::kernel_control::accept_forward_compute_quote(state, &rfq, &quote) {
                        Ok(order) => {
                            let instrument_id = order.instrument_id.clone();
                            let quote_id = order.quote_id.clone();
                            state
                                .network_requests
                                .record_forward_order_acceptance(order);
                            state.provider_runtime.last_result = Some(format!(
                                "accepted compute quote {} -> {}",
                                quote_id, instrument_id
                            ));
                        }
                        Err(error) => {
                            state.network_requests.last_error = Some(error.clone());
                            state.network_requests.load_state =
                                crate::app_state::PaneLoadState::Error;
                            state.provider_runtime.last_error_detail = Some(error.clone());
                            state.provider_runtime.last_result =
                                Some(format!("compute quote acceptance failed: {error}"));
                        }
                    }
                }
            }
            true
        }
        NetworkRequestsPaneAction::SelectQuote(index) => {
            if !state.network_requests.select_active_quote_by_index(index) {
                state.network_requests.last_error =
                    Some("Selected compute quote no longer exists".to_string());
                state.network_requests.load_state = crate::app_state::PaneLoadState::Error;
            }
            true
        }
    }
}

pub(super) fn run_provider_status_action(
    state: &mut crate::app_state::RenderState,
    action: ProviderStatusPaneAction,
) -> bool {
    match action {
        ProviderStatusPaneAction::ToggleInventory(target) => {
            let enabled = state.provider_runtime.toggle_inventory_target(target);
            let mode_note = if matches!(
                state.provider_runtime.mode,
                crate::app_state::ProviderMode::Online | crate::app_state::ProviderMode::Degraded
            ) {
                "local admission updated now; disabled supply remains listed until the next session because lot cancellation receipts are not implemented yet"
            } else {
                "applies on next Go Online session"
            };
            state.provider_runtime.inventory_last_action = Some(format!(
                "{} {} ({mode_note})",
                target.display_label(),
                if enabled { "enabled" } else { "disabled" }
            ));
            state.provider_runtime.last_result =
                state.provider_runtime.inventory_last_action.clone();
            if enabled
                && matches!(
                    state.provider_runtime.mode,
                    crate::app_state::ProviderMode::Online
                        | crate::app_state::ProviderMode::Degraded
                )
                && let Err(error) =
                    crate::kernel_control::register_online_compute_inventory_with_kernel(state)
            {
                state.provider_runtime.inventory_last_error = Some(error.clone());
                state.provider_runtime.last_error_detail = Some(error.clone());
                state.provider_runtime.last_result = Some(format!(
                    "inventory enable failed for {}: {error}",
                    target.product_id()
                ));
            }
            let _ = crate::kernel_control::refresh_provider_inventory_rows(state);
            true
        }
    }
}

pub(super) fn run_local_inference_action(
    state: &mut crate::app_state::RenderState,
    action: LocalInferencePaneAction,
) -> bool {
    match action {
        LocalInferencePaneAction::RefreshRuntime => queue_local_inference_pane_command(
            state,
            LocalInferenceRuntimeCommand::Refresh,
            "Queued local inference runtime refresh",
        ),
        LocalInferencePaneAction::WarmModel => queue_local_inference_pane_command(
            state,
            LocalInferenceRuntimeCommand::WarmConfiguredModel,
            "Queued GPT-OSS 20B load",
        ),
        LocalInferencePaneAction::UnloadModel => queue_local_inference_pane_command(
            state,
            LocalInferenceRuntimeCommand::UnloadConfiguredModel,
            "Queued configured GPT-OSS model unload",
        ),
        LocalInferencePaneAction::RunPrompt => {
            let prompt = state
                .local_inference_inputs
                .prompt
                .get_value()
                .trim()
                .to_string();
            if prompt.is_empty() {
                state.local_inference.load_state = crate::app_state::PaneLoadState::Error;
                state.local_inference.last_error =
                    Some("Prompt is required before running local inference".to_string());
                state.local_inference.last_action = Some("Local inference run blocked".to_string());
                return true;
            }

            let params = match build_local_inference_pane_params(state) {
                Ok(value) => value,
                Err(error) => {
                    state.local_inference.load_state = crate::app_state::PaneLoadState::Error;
                    state.local_inference.last_error = Some(error);
                    state.local_inference.last_action =
                        Some("Local inference run blocked".to_string());
                    return true;
                }
            };
            let request_id = format!(
                "local-inference-pane-{}",
                state.reserve_runtime_command_seq()
            );
            let requested_model =
                normalize_optional_text(state.local_inference_inputs.requested_model.get_value());
            let request_id_for_state = request_id.clone();
            let model_note = requested_model
                .clone()
                .unwrap_or_else(|| "configured".to_string());
            match state.queue_local_inference_runtime_command(
                LocalInferenceRuntimeCommand::Generate(LocalInferenceGenerateJob {
                    request_id,
                    prompt,
                    requested_model,
                    params,
                }),
            ) {
                Ok(()) => {
                    state.local_inference.load_state = crate::app_state::PaneLoadState::Loading;
                    state.local_inference.last_error = None;
                    state.local_inference.last_action = Some(format!(
                        "Queued local inference workbench run {request_id_for_state} ({model_note})"
                    ));
                    state.local_inference.pending_request_id = Some(request_id_for_state.clone());
                    state.local_inference.last_request_id = Some(request_id_for_state.clone());
                    state.local_inference.last_model = None;
                    state.provider_runtime.last_result = Some(format!(
                        "local inference workbench queued request {}",
                        request_id_for_state
                    ));
                }
                Err(error) => {
                    state.local_inference.load_state = crate::app_state::PaneLoadState::Error;
                    state.local_inference.last_error = Some(error.clone());
                    state.local_inference.last_action =
                        Some("Local inference runtime enqueue failed".to_string());
                    state.provider_runtime.last_error_detail = Some(error);
                }
            }
            true
        }
    }
}

/// Queues Apple FM bridge refresh or start when the Mission Control pane is opened.
/// Call once when the GoOnline pane is created (e.g. at startup).
pub(crate) fn ensure_mission_control_apple_fm_refresh(
    state: &mut crate::app_state::RenderState,
) -> bool {
    match crate::app_state::mission_control_local_runtime_lane(
        state.desktop_shell_mode,
        &state.ollama_execution,
    ) {
        Some(crate::app_state::MissionControlLocalRuntimeLane::AppleFoundationModels) => {
            let bridge_starting =
                state.provider_runtime.apple_fm.bridge_status.as_deref() == Some("starting");
            if bridge_starting {
                state
                    .mission_control
                    .record_action("Apple FM bridge start already in progress");
                state.mission_control.last_error = None;
                return true;
            }
            let handled = if state.provider_runtime.apple_fm.reachable {
                run_apple_fm_workbench_action(state, AppleFmWorkbenchPaneAction::RefreshBridge)
            } else {
                run_apple_fm_workbench_action(state, AppleFmWorkbenchPaneAction::StartBridge)
            };
            if handled && state.apple_fm_workbench.last_error.is_none() {
                state
                    .mission_control
                    .record_action(if state.provider_runtime.apple_fm.reachable {
                        "Queued Apple FM bridge refresh"
                    } else {
                        "Queued Apple FM bridge start"
                    });
            } else if !handled {
                state.mission_control.last_action =
                    Some("Apple FM mission control action failed".to_string());
                state.mission_control.last_error = state.apple_fm_workbench.last_error.clone();
            }
            handled
        }
        _ => false,
    }
}

pub(super) fn run_apple_fm_workbench_action(
    state: &mut crate::app_state::RenderState,
    action: AppleFmWorkbenchPaneAction,
) -> bool {
    match action {
        AppleFmWorkbenchPaneAction::RefreshBridge => queue_apple_fm_pane_command(
            state,
            AppleFmBridgeCommand::Refresh,
            "Queued Apple FM bridge refresh",
        ),
        AppleFmWorkbenchPaneAction::StartBridge => queue_apple_fm_pane_command(
            state,
            AppleFmBridgeCommand::EnsureBridgeRunning,
            "Queued Apple FM bridge start",
        ),
        AppleFmWorkbenchPaneAction::CreateSession => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::CreateSession)
        }
        AppleFmWorkbenchPaneAction::InspectSession => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::InspectSession)
        }
        AppleFmWorkbenchPaneAction::ResetSession => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::ResetSession)
        }
        AppleFmWorkbenchPaneAction::DeleteSession => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::DeleteSession)
        }
        AppleFmWorkbenchPaneAction::RunText => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RunText)
        }
        AppleFmWorkbenchPaneAction::RunChat => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RunChat)
        }
        AppleFmWorkbenchPaneAction::RunSession => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RunSession)
        }
        AppleFmWorkbenchPaneAction::RunStream => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RunStream)
        }
        AppleFmWorkbenchPaneAction::RunStructured => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RunStructured)
        }
        AppleFmWorkbenchPaneAction::ExportTranscript => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::ExportTranscript)
        }
        AppleFmWorkbenchPaneAction::RestoreTranscript => {
            queue_apple_fm_workbench_operation(state, AppleFmWorkbenchOperation::RestoreTranscript)
        }
        AppleFmWorkbenchPaneAction::CycleToolProfile => {
            state.apple_fm_workbench.tool_profile = state.apple_fm_workbench.tool_profile.cycle();
            state.apple_fm_workbench.last_action = Some(format!(
                "Apple FM workbench {}",
                state
                    .apple_fm_workbench
                    .tool_profile
                    .label()
                    .to_ascii_lowercase()
            ));
            state.apple_fm_workbench.last_error = None;
            true
        }
        AppleFmWorkbenchPaneAction::CycleSamplingMode => {
            state.apple_fm_workbench.sampling_mode = state.apple_fm_workbench.sampling_mode.cycle();
            state.apple_fm_workbench.last_action = Some(format!(
                "Apple FM workbench sampling {}",
                state
                    .apple_fm_workbench
                    .sampling_mode
                    .label()
                    .to_ascii_lowercase()
            ));
            state.apple_fm_workbench.last_error = None;
            true
        }
    }
}

pub(super) fn run_mission_control_action(
    state: &mut crate::app_state::RenderState,
    action: MissionControlPaneAction,
) -> bool {
    match action {
        MissionControlPaneAction::RefreshWallet => {
            queue_spark_command(state, SparkWalletCommand::Refresh);
            if let Some(error) = state.spark_wallet.last_error.clone() {
                state.mission_control.record_error(error);
            } else {
                state.mission_control.record_action("Queued wallet refresh");
            }
            true
        }
        MissionControlPaneAction::CreateLightningReceiveTarget => {
            let amount_sats = match parse_positive_amount_str(
                state.mission_control.load_funds_amount_sats.get_value(),
                "Lightning receive amount",
            ) {
                Ok(amount_sats) => amount_sats,
                Err(error) => {
                    state.spark_wallet.last_error = Some(error.clone());
                    state.mission_control.record_error(error);
                    return true;
                }
            };

            queue_spark_command(
                state,
                SparkWalletCommand::CreateBolt11Invoice {
                    amount_sats,
                    description: Some("Mission Control load funds".to_string()),
                    expiry_seconds: Some(3600),
                },
            );
            if let Some(error) = state.spark_wallet.last_error.clone() {
                state.mission_control.record_error(error);
            } else {
                state.mission_control.record_action(format!(
                    "Queued Lightning receive target for {}",
                    format_sats_amount(amount_sats)
                ));
            }
            true
        }
        MissionControlPaneAction::CopyLightningReceiveTarget => {
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let notice = match state.spark_wallet.last_invoice_state(now_epoch_seconds) {
                crate::spark_wallet::SparkInvoiceState::Empty => {
                    "No Lightning receive target available. Generate one first.".to_string()
                }
                crate::spark_wallet::SparkInvoiceState::Expired => {
                    "Lightning receive target expired. Generate a new one.".to_string()
                }
                crate::spark_wallet::SparkInvoiceState::Ready => {
                    match state.spark_wallet.last_invoice.as_deref() {
                        Some(invoice) if !invoice.trim().is_empty() => {
                            match copy_to_clipboard(invoice) {
                                Ok(()) => {
                                    "Copied Lightning receive target to clipboard".to_string()
                                }
                                Err(error) => {
                                    format!("Failed to copy Lightning receive target: {error}")
                                }
                            }
                        }
                        _ => {
                            "No Lightning receive target available. Generate one first.".to_string()
                        }
                    }
                }
            };

            if notice.starts_with("Copied") {
                state.mission_control.record_action(notice);
            } else {
                state.mission_control.record_error(notice);
            }
            true
        }
        MissionControlPaneAction::CopyLogStream => {
            let output = state
                .mission_control
                .log_stream
                .recent_lines(2000)
                .iter()
                .map(|line| line.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            let notice = if output.trim().is_empty() {
                "Mission Control log stream is empty".to_string()
            } else {
                match copy_to_clipboard(&output) {
                    Ok(()) => "Copied Mission Control log stream to clipboard".to_string(),
                    Err(error) => format!("Failed to copy Mission Control log stream: {error}"),
                }
            };

            if notice.starts_with("Copied") {
                state.mission_control.record_action(notice);
            } else {
                state.mission_control.record_error(notice);
            }
            true
        }
        MissionControlPaneAction::SendLightningPayment => {
            let command = match build_pay_invoice_command(
                PayInvoicePaneAction::SendPayment,
                state.mission_control.send_invoice.get_value(),
                state.mission_control.load_funds_amount_sats.get_value(),
            ) {
                Ok(command) => command,
                Err(error) => {
                    state.spark_wallet.last_error = Some(error.clone());
                    state.mission_control.record_error(error);
                    return true;
                }
            };

            queue_spark_command(state, command);
            if let Some(error) = state.spark_wallet.last_error.clone() {
                state.mission_control.record_error(error);
            } else {
                state
                    .mission_control
                    .record_action("Queued Lightning withdrawal");
                state.mission_control.send_invoice.set_value(String::new());
            }
            true
        }
        MissionControlPaneAction::CopySeedPhrase => {
            let notice = match state.nostr_identity.as_ref() {
                Some(identity) if !identity.mnemonic.trim().is_empty() => {
                    match copy_to_clipboard(&identity.mnemonic) {
                        Ok(()) => "Copied 12-word wallet seed to clipboard. Treat it like cash."
                            .to_string(),
                        Err(error) => format!("Failed to copy wallet seed: {error}"),
                    }
                }
                _ => "No wallet seed loaded yet.".to_string(),
            };

            if notice.starts_with("Copied") {
                state.mission_control.record_action(notice);
            } else {
                state.mission_control.record_error(notice);
            }
            true
        }
        MissionControlPaneAction::OpenLocalModelWorkbench => {
            match crate::app_state::mission_control_local_runtime_lane(
                state.desktop_shell_mode,
                &state.ollama_execution,
            ) {
                Some(crate::app_state::MissionControlLocalRuntimeLane::AppleFoundationModels) => {
                    if state.dev_mode_enabled() && state.provider_runtime.apple_fm.is_ready() {
                        crate::pane_system::PaneController::create_for_kind(
                            state,
                            crate::app_state::PaneKind::AppleFmWorkbench,
                        );
                        state
                            .mission_control
                            .record_action("Opened Apple FM workbench");
                        state.mission_control.last_error = None;
                        return true;
                    }

                    let bridge_starting = state.provider_runtime.apple_fm.bridge_status.as_deref()
                        == Some("starting");
                    let handled = if state.provider_runtime.apple_fm.reachable {
                        run_apple_fm_workbench_action(
                            state,
                            AppleFmWorkbenchPaneAction::RefreshBridge,
                        )
                    } else if bridge_starting {
                        true
                    } else {
                        run_apple_fm_workbench_action(
                            state,
                            AppleFmWorkbenchPaneAction::StartBridge,
                        )
                    };

                    if bridge_starting {
                        state
                            .mission_control
                            .record_action("Apple FM bridge start already in progress");
                        state.mission_control.last_error = None;
                    } else if state.apple_fm_workbench.last_error.is_none() {
                        state.mission_control.record_action(
                            if state.provider_runtime.apple_fm.reachable {
                                "Queued Apple FM bridge refresh"
                            } else {
                                "Queued Apple FM bridge start"
                            },
                        );
                    } else {
                        state.mission_control.last_action =
                            Some("Apple FM mission control action failed".to_string());
                        state.mission_control.last_error =
                            state.apple_fm_workbench.last_error.clone();
                    }
                    handled
                }
                Some(crate::app_state::MissionControlLocalRuntimeLane::NvidiaGptOss) => {
                    crate::pane_system::PaneController::create_for_kind(
                        state,
                        crate::app_state::PaneKind::LocalInference,
                    );

                    state
                        .mission_control
                        .record_action("Opened GPT-OSS workbench");
                    state.mission_control.last_error = None;
                    true
                }
                None => {
                    state.mission_control.last_action =
                        Some("No supported local runtime".to_string());
                    state.mission_control.last_error = Some(
                        "Mission Control has no supported local runtime. Apple Foundation Models is required for the release path."
                            .to_string(),
                    );
                    true
                }
            }
        }
        MissionControlPaneAction::RunLocalFmSummaryTest => {
            run_mission_control_local_fm_summary_test(state)
        }
        MissionControlPaneAction::ToggleBuyModeLoop => {
            if !state.mission_control_buy_mode_enabled() {
                state
                    .mission_control
                    .record_error("Buy Mode is disabled for this session");
                return true;
            }

            let now = std::time::Instant::now();
            if state.mission_control.buy_mode_loop_enabled {
                state.mission_control.toggle_buy_mode_loop(now);
                state
                    .mission_control
                    .record_action("Buy Mode stopped".to_string());
            } else if let Some(reason) =
                crate::app_state::mission_control_buy_mode_start_block_reason(&state.spark_wallet)
            {
                state.mission_control.record_error(reason.clone());
                state.provider_runtime.last_error_detail = Some(reason);
            } else if state.mission_control.toggle_buy_mode_loop(now) {
                state.mission_control.record_action(format!(
                    "Buy Mode armed // 5050 // {} sats // every {}",
                    crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                    crate::app_state::mission_control_buy_mode_interval_label()
                ));
            }
            true
        }
        MissionControlPaneAction::OpenBuyModePayments => {
            crate::pane_system::PaneController::create_for_kind(
                state,
                crate::app_state::PaneKind::BuyModePayments,
            );
            state
                .mission_control
                .record_action("Opened Buy Mode payment history");
            true
        }
        MissionControlPaneAction::SendWithdrawal => {
            let command = match build_pay_invoice_command(
                PayInvoicePaneAction::SendPayment,
                state.mission_control.withdraw_invoice.get_value(),
                "",
            ) {
                Ok(command) => command,
                Err(error) => {
                    state.spark_wallet.last_error = Some(error.clone());
                    state.mission_control.record_error(error);
                    return true;
                }
            };

            queue_spark_command(state, command);
            if let Some(error) = state.spark_wallet.last_error.clone() {
                state.mission_control.record_error(error);
            } else {
                state
                    .mission_control
                    .record_action("Queued Lightning withdrawal");
                state
                    .mission_control
                    .withdraw_invoice
                    .set_value(String::new());
            }
            true
        }
        MissionControlPaneAction::OpenDocumentation => {
            match open_mission_control_documentation() {
                Ok(path) => {
                    state
                        .mission_control
                        .record_action(format!("Opened documentation ({})", path.display()));
                }
                Err(error) => {
                    state.mission_control.last_action =
                        Some("Documentation open failed".to_string());
                    state.mission_control.last_error = Some(error);
                }
            }
            true
        }
    }
}

pub(super) fn run_buy_mode_payments_action(
    state: &mut crate::app_state::RenderState,
    action: BuyModePaymentsPaneAction,
) -> bool {
    match action {
        BuyModePaymentsPaneAction::CopyAll => {
            let output = crate::app_state::buy_mode_payments_clipboard_text(
                &state.mission_control,
                &state.network_requests,
                &state.spark_wallet,
            );
            let notice = match copy_to_clipboard(&output) {
                Ok(()) => "Copied Buy Mode payment history to clipboard".to_string(),
                Err(error) => format!("Failed to copy Buy Mode payment history: {error}"),
            };
            if notice.starts_with("Failed") {
                state.mission_control.record_error(notice);
            } else {
                state.mission_control.record_action(notice);
            }
            true
        }
    }
}

fn open_mission_control_documentation() -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/plans/mission-control-pane.md");
    if !path.is_file() {
        return Err(format!(
            "Mission Control documentation not found: {}",
            path.display()
        ));
    }
    open_path_in_default_app(path.as_path())?;
    Ok(path)
}

fn spawn_editor_command(
    program: &str,
    args: &[&str],
    path: &std::path::Path,
) -> Result<(), String> {
    std::process::Command::new(program)
        .args(args)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch editor '{program}': {error}"))
}

fn open_path_in_editor_or_default_app(path: &std::path::Path) -> Result<String, String> {
    for variable in ["OPENAGENTS_CODEX_EDITOR", "VISUAL", "EDITOR"] {
        let Ok(command) = std::env::var(variable) else {
            continue;
        };
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>();
        if spawn_editor_command(program, &args, path).is_ok() {
            return Ok(format!(
                "Opened workspace in {}",
                command.split_whitespace().next().unwrap_or(program)
            ));
        }
    }

    for (program, args, label) in [
        ("code", vec!["-r"], "VS Code"),
        ("cursor", vec![], "Cursor"),
        ("windsurf", vec![], "Windsurf"),
        ("zed", vec![], "Zed"),
        ("subl", vec![], "Sublime Text"),
    ] {
        if spawn_editor_command(program, &args, path).is_ok() {
            return Ok(format!("Opened workspace in {label}"));
        }
    }

    open_path_in_default_app(path)?;
    Ok("Opened workspace in the default app".to_string())
}

fn open_path_in_default_app(path: &std::path::Path) -> Result<(), String> {
    let status = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg(path)
            .status()
            .map_err(|error| format!("Failed to launch macOS open command: {error}"))?
    } else if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|error| format!("Failed to launch xdg-open: {error}"))?
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(path)
            .status()
            .map_err(|error| format!("Failed to launch Windows start command: {error}"))?
    } else {
        return Err("Opening documentation is unsupported on this platform".to_string());
    };

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Documentation launcher exited with status {status}"
        ))
    }
}

fn run_mission_control_local_fm_summary_test(state: &mut crate::app_state::RenderState) -> bool {
    if crate::app_state::mission_control_local_runtime_lane(
        state.desktop_shell_mode,
        &state.ollama_execution,
    ) != Some(crate::app_state::MissionControlLocalRuntimeLane::AppleFoundationModels)
    {
        state
            .mission_control
            .record_error("Local FM test is only available on Apple Foundation Models");
        return true;
    }

    if state.mission_control.local_fm_summary_is_pending() {
        state
            .mission_control
            .record_action("Local FM summary test already streaming");
        return true;
    }

    if !state.provider_runtime.apple_fm.is_ready() {
        state
            .mission_control
            .record_error("Local FM test requires Apple Foundation Models to be ready");
        return true;
    }

    let request_id = format!("mission-control-fm-{}", state.reserve_runtime_command_seq());
    let command =
        AppleFmBridgeCommand::MissionControlSummary(AppleFmMissionControlSummaryCommand {
            request_id: request_id.clone(),
            instructions: MISSION_CONTROL_LOCAL_FM_SUMMARY_INSTRUCTIONS.to_string(),
            prompt: build_mission_control_local_fm_summary_prompt(state),
            requested_model: state.provider_runtime.apple_fm.ready_model.clone(),
            options: Some(AppleFmGenerationOptions {
                sampling: None,
                temperature: Some(0.2),
                maximum_response_tokens: Some(160),
            }),
        });

    match state.queue_apple_fm_bridge_command(command) {
        Ok(()) => {
            state
                .mission_control
                .begin_local_fm_summary(request_id, "latest Mission Control results");
            state.provider_runtime.last_result =
                Some("Queued local Apple Foundation Models summary test".to_string());
        }
        Err(error) => {
            state
                .mission_control
                .record_error(format!("Failed to queue local FM summary test: {error}"));
        }
    }
    true
}

fn build_mission_control_local_fm_summary_prompt(state: &crate::app_state::RenderState) -> String {
    let mut sections = vec!["Summarize this Mission Control state.".to_string()];

    if let Some(result) = state.provider_runtime.last_result.as_deref()
        && !result.trim().is_empty()
    {
        sections.push(format!(
            "Latest provider result: {}",
            truncate_single_line(result, 220)
        ));
    }

    if let Some(error) = state.provider_runtime.last_error_detail.as_deref()
        && !error.trim().is_empty()
    {
        sections.push(format!(
            "Latest provider error: {}",
            truncate_single_line(error, 220)
        ));
    }

    if let Some(request) = state.network_requests.submitted.first() {
        sections.push(format!(
            "Latest buyer request: id={} type={} status={:?} budget={} payload={}",
            request.request_id,
            request.request_type,
            request.status,
            request.budget_sats,
            truncate_single_line(request.payload.as_str(), 240)
        ));
    }

    if let Some(job) = state.active_job.job.as_ref() {
        sections.push(format!(
            "Active provider job: request_id={} capability={} stage={:?} price_sats={}",
            job.request_id, job.capability, job.stage, job.quoted_price_sats
        ));
    }

    if let Some(output) = state.active_job.execution_output.as_deref()
        && !output.trim().is_empty()
    {
        sections.push(format!(
            "Latest execution output: {}",
            truncate_single_line(output, 240)
        ));
    }

    let recent_log_lines = state
        .mission_control
        .log_stream
        .recent_lines(8)
        .iter()
        .filter_map(|line| {
            let text = line.text.trim();
            if text.is_empty() || text.to_ascii_lowercase().contains("local fm summary") {
                None
            } else {
                Some(format!("- {}", truncate_single_line(text, 220)))
            }
        })
        .collect::<Vec<_>>();
    if !recent_log_lines.is_empty() {
        sections.push("Recent Mission Control logs:".to_string());
        sections.extend(recent_log_lines);
    }

    sections.join("\n")
}

fn truncate_single_line(value: &str, max_chars: usize) -> String {
    let flattened = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= max_chars {
        flattened
    } else {
        let prefix = flattened.chars().take(max_chars).collect::<String>();
        format!("{prefix}...")
    }
}

fn queue_local_inference_pane_command(
    state: &mut crate::app_state::RenderState,
    command: LocalInferenceRuntimeCommand,
    action_label: &str,
) -> bool {
    match state.queue_local_inference_runtime_command(command) {
        Ok(()) => {
            state.local_inference.load_state = crate::app_state::PaneLoadState::Loading;
            state.local_inference.last_error = None;
            state.local_inference.last_action = Some(action_label.to_string());
            state.provider_runtime.last_result = Some(action_label.to_string());
        }
        Err(error) => {
            state.local_inference.load_state = crate::app_state::PaneLoadState::Error;
            state.local_inference.last_error = Some(error.clone());
            state.local_inference.last_action =
                Some("Local inference runtime enqueue failed".to_string());
            state.provider_runtime.last_error_detail = Some(error);
        }
    }
    true
}

fn queue_apple_fm_pane_command(
    state: &mut crate::app_state::RenderState,
    command: AppleFmBridgeCommand,
    action_label: &str,
) -> bool {
    match state.queue_apple_fm_bridge_command(command) {
        Ok(()) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Loading;
            state.apple_fm_workbench.last_error = None;
            state.apple_fm_workbench.last_action = Some(action_label.to_string());
            state.provider_runtime.last_result = Some(action_label.to_string());
        }
        Err(error) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Error;
            state.apple_fm_workbench.last_error = Some(error.clone());
            state.apple_fm_workbench.last_action =
                Some("Apple FM bridge enqueue failed".to_string());
            state.provider_runtime.last_error_detail = Some(error);
        }
    }
    true
}

fn queue_apple_fm_workbench_operation(
    state: &mut crate::app_state::RenderState,
    operation: AppleFmWorkbenchOperation,
) -> bool {
    let options = if apple_fm_workbench_operation_uses_options(operation) {
        match build_apple_fm_workbench_options(state) {
            Ok(value) => value,
            Err(error) => {
                state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Error;
                state.apple_fm_workbench.last_error = Some(error);
                state.apple_fm_workbench.last_action =
                    Some("Apple FM workbench action blocked".to_string());
                return true;
            }
        }
    } else {
        None
    };

    let session_id = resolve_apple_fm_workbench_session_id(
        state.apple_fm_workbench_inputs.session_id.get_value(),
        state.apple_fm_workbench.active_session_id.as_deref(),
    );
    if state
        .apple_fm_workbench_inputs
        .session_id
        .get_value()
        .trim()
        .is_empty()
    {
        if let Some(session_id_value) = session_id.as_ref() {
            state
                .apple_fm_workbench_inputs
                .session_id
                .set_value(session_id_value.clone());
        }
    }

    let request_id = format!("apple-fm-workbench-{}", state.reserve_runtime_command_seq());
    let request_id_for_state = request_id.clone();
    let command = AppleFmWorkbenchCommand {
        request_id,
        operation,
        instructions: normalize_optional_text(
            state.apple_fm_workbench_inputs.instructions.get_value(),
        ),
        prompt: normalize_optional_text(state.apple_fm_workbench_inputs.prompt.get_value()),
        requested_model: normalize_optional_text(state.apple_fm_workbench_inputs.model.get_value()),
        session_id,
        options,
        schema_json: normalize_optional_text(
            state.apple_fm_workbench_inputs.schema_json.get_value(),
        ),
        transcript_json: normalize_optional_text(
            state.apple_fm_workbench_inputs.transcript_json.get_value(),
        ),
        tool_mode: apple_fm_workbench_tool_mode(state),
    };

    match state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::Workbench(command)) {
        Ok(()) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Loading;
            state.apple_fm_workbench.last_error = None;
            state.apple_fm_workbench.last_action =
                Some(format!("Queued Apple FM workbench {}", operation.label()));
            state.apple_fm_workbench.pending_request_id = Some(request_id_for_state.clone());
            state.apple_fm_workbench.last_request_id = Some(request_id_for_state.clone());
            state.apple_fm_workbench.last_operation = Some(operation.label().to_string());
            state.provider_runtime.last_result = Some(format!(
                "Apple FM workbench queued request {}",
                request_id_for_state
            ));
        }
        Err(error) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Error;
            state.apple_fm_workbench.last_error = Some(error.clone());
            state.apple_fm_workbench.last_action =
                Some("Apple FM bridge enqueue failed".to_string());
            state.provider_runtime.last_error_detail = Some(error);
        }
    }
    true
}

fn apple_fm_workbench_tool_mode(state: &crate::app_state::RenderState) -> AppleFmWorkbenchToolMode {
    match state.apple_fm_workbench.tool_profile {
        crate::app_state::AppleFmWorkbenchToolProfile::None => AppleFmWorkbenchToolMode::None,
        crate::app_state::AppleFmWorkbenchToolProfile::Demo => AppleFmWorkbenchToolMode::Demo,
        crate::app_state::AppleFmWorkbenchToolProfile::Failing => AppleFmWorkbenchToolMode::Failing,
    }
}

fn apple_fm_workbench_operation_uses_options(operation: AppleFmWorkbenchOperation) -> bool {
    matches!(
        operation,
        AppleFmWorkbenchOperation::RunText
            | AppleFmWorkbenchOperation::RunChat
            | AppleFmWorkbenchOperation::RunSession
            | AppleFmWorkbenchOperation::RunStream
            | AppleFmWorkbenchOperation::RunStructured
    )
}

fn resolve_apple_fm_workbench_session_id(
    input_session_id: &str,
    active_session_id: Option<&str>,
) -> Option<String> {
    normalize_optional_text(input_session_id).or_else(|| {
        active_session_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn build_apple_fm_workbench_options(
    state: &crate::app_state::RenderState,
) -> Result<Option<AppleFmGenerationOptions>, String> {
    let maximum_response_tokens =
        normalize_optional_text(state.apple_fm_workbench_inputs.max_tokens.get_value())
            .map(|value| parse_positive_amount_str(value.as_str(), "Max tokens"))
            .transpose()?
            .map(|value| value as u32);

    let temperature =
        normalize_optional_text(state.apple_fm_workbench_inputs.temperature.get_value())
            .map(|value| parse_local_inference_float(value.as_str(), "Temperature"))
            .transpose()?;

    let top = normalize_optional_text(state.apple_fm_workbench_inputs.top.get_value())
        .map(|value| parse_positive_amount_str(value.as_str(), "Top-k"))
        .transpose()?
        .map(|value| value as u32);

    let probability_threshold = normalize_optional_text(
        state
            .apple_fm_workbench_inputs
            .probability_threshold
            .get_value(),
    )
    .map(|value| parse_local_inference_float(value.as_str(), "Top-p"))
    .transpose()?;

    let seed = normalize_optional_text(state.apple_fm_workbench_inputs.seed.get_value())
        .map(|value| parse_non_negative_amount_str(value.as_str(), "Seed"))
        .transpose()?;

    let sampling = match state.apple_fm_workbench.sampling_mode {
        crate::app_state::AppleFmWorkbenchSamplingMode::Auto => None,
        crate::app_state::AppleFmWorkbenchSamplingMode::Greedy => {
            Some(AppleFmSamplingMode::greedy())
        }
        crate::app_state::AppleFmWorkbenchSamplingMode::Random => Some(
            AppleFmSamplingMode::random(top, probability_threshold, seed)
                .map_err(|error| error.to_string())?,
        ),
    };

    if sampling.is_none() && temperature.is_none() && maximum_response_tokens.is_none() {
        return Ok(None);
    }

    AppleFmGenerationOptions::new(sampling, temperature, maximum_response_tokens)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn build_local_inference_pane_params(
    state: &crate::app_state::RenderState,
) -> Result<Vec<JobExecutionParam>, String> {
    let mut params = Vec::new();

    if let Some(value) =
        normalize_optional_text(state.local_inference_inputs.max_tokens.get_value())
    {
        let max_tokens = parse_positive_amount_str(value.as_str(), "Max tokens")?;
        params.push(JobExecutionParam {
            key: "max_tokens".to_string(),
            value: max_tokens.to_string(),
        });
    }

    if let Some(value) =
        normalize_optional_text(state.local_inference_inputs.temperature.get_value())
    {
        let temperature = parse_local_inference_float(value.as_str(), "Temperature")?;
        params.push(JobExecutionParam {
            key: "temperature".to_string(),
            value: temperature.to_string(),
        });
    }

    if let Some(value) = normalize_optional_text(state.local_inference_inputs.top_k.get_value()) {
        let top_k = parse_non_negative_amount_str(value.as_str(), "Top-k")?;
        params.push(JobExecutionParam {
            key: "top_k".to_string(),
            value: top_k.to_string(),
        });
    }

    if let Some(value) = normalize_optional_text(state.local_inference_inputs.top_p.get_value()) {
        let top_p = parse_local_inference_float(value.as_str(), "Top-p")?;
        params.push(JobExecutionParam {
            key: "top_p".to_string(),
            value: top_p.to_string(),
        });
    }

    Ok(params)
}

fn parse_local_inference_float(raw: &str, label: &str) -> Result<f64, String> {
    raw.trim()
        .parse::<f64>()
        .map_err(|error| format!("{label} must be a number: {error}"))
}

fn build_spot_compute_rfq_from_inputs(
    state: &mut crate::app_state::RenderState,
) -> Result<crate::app_state::SpotComputeRfqDraft, String> {
    use openagents_kernel_core::compute::{ComputeBackendFamily, ComputeFamily};

    let compute_family = match state
        .network_requests_inputs
        .compute_family
        .get_value()
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '-'], "_")
        .as_str()
    {
        "inference" | "text_generation" | "text_generation_request" => ComputeFamily::Inference,
        "embedding" | "embeddings" | "text_embedding" | "text_embeddings" => {
            ComputeFamily::Embeddings
        }
        other => {
            return Err(format!(
                "Compute family must be inference or embeddings, got {other}"
            ));
        }
    };
    let preferred_backend = match normalize_optional_text(
        state.network_requests_inputs.preferred_backend.get_value(),
    ) {
        Some(value) => match value
            .trim()
            .to_ascii_lowercase()
            .replace(['.', '-'], "_")
            .as_str()
        {
            "psionic" | "local_inference" | "ollama" => Some(ComputeBackendFamily::Ollama),
            "apple_foundation_models" | "apple_fm" | "apple_foundation" => {
                Some(ComputeBackendFamily::AppleFoundationModels)
            }
            other => {
                return Err(format!(
                    "Preferred backend must be psionic, apple_foundation_models, or empty, got {other}"
                ));
            }
        },
        None => None,
    };
    let quantity = parse_positive_amount_str(
        state.network_requests_inputs.quantity.get_value(),
        "Requested quantity",
    )?;
    let window_minutes = parse_positive_amount_str(
        state.network_requests_inputs.window_minutes.get_value(),
        "Delivery window minutes",
    )?;
    let max_price_sats = parse_positive_amount_str(
        state.network_requests_inputs.max_price_sats.get_value(),
        "Max price sats",
    )?;
    let capability_constraints = parse_spot_compute_capability_constraints(
        state
            .network_requests_inputs
            .capability_constraints
            .get_value(),
    )?;
    Ok(crate::app_state::SpotComputeRfqDraft {
        rfq_id: format!("rfq-{}", current_epoch_seconds()),
        compute_family,
        preferred_backend,
        quantity,
        window_minutes,
        max_price_sats,
        capability_constraints,
    })
}

fn build_forward_compute_rfq_from_inputs(
    state: &mut crate::app_state::RenderState,
    delivery_start_minutes: u64,
) -> Result<crate::app_state::ForwardComputeRfqDraft, String> {
    let spot_rfq = build_spot_compute_rfq_from_inputs(state)?;
    Ok(crate::app_state::ForwardComputeRfqDraft {
        rfq_id: spot_rfq.rfq_id,
        compute_family: spot_rfq.compute_family,
        preferred_backend: spot_rfq.preferred_backend,
        quantity: spot_rfq.quantity,
        delivery_start_minutes,
        window_minutes: spot_rfq.window_minutes,
        max_price_sats: spot_rfq.max_price_sats,
        capability_constraints: spot_rfq.capability_constraints,
    })
}

fn parse_spot_compute_capability_constraints(
    raw: &str,
) -> Result<crate::app_state::SpotComputeCapabilityConstraints, String> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return Ok(crate::app_state::SpotComputeCapabilityConstraints::default());
    }
    if normalized.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(normalized)
            .map_err(|error| format!("Capability constraints JSON is invalid: {error}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "Capability constraints JSON must be an object".to_string())?;
        return capability_constraints_from_pairs(
            object
                .iter()
                .map(|(key, value)| {
                    (
                        key.to_string(),
                        value
                            .as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| value.to_string()),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }
    let pairs = normalized
        .split(',')
        .filter_map(|entry| {
            let mut segments = entry.splitn(2, '=');
            let key = segments.next()?.trim();
            let value = segments.next()?.trim();
            (!key.is_empty() && !value.is_empty()).then(|| (key.to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
    capability_constraints_from_pairs(pairs.as_slice())
}

fn capability_constraints_from_pairs(
    pairs: &[(String, String)],
) -> Result<crate::app_state::SpotComputeCapabilityConstraints, String> {
    let mut constraints = crate::app_state::SpotComputeCapabilityConstraints::default();
    for (key, value) in pairs {
        match key.trim() {
            "accelerator_vendor" => constraints.accelerator_vendor = Some(value.trim().to_string()),
            "accelerator_family" => constraints.accelerator_family = Some(value.trim().to_string()),
            "min_memory_gb" => {
                constraints.min_memory_gb = Some(
                    value
                        .trim()
                        .parse::<u32>()
                        .map_err(|error| format!("min_memory_gb must be an integer: {error}"))?,
                );
            }
            "max_latency_ms" => {
                constraints.max_latency_ms = Some(
                    value
                        .trim()
                        .parse::<u32>()
                        .map_err(|error| format!("max_latency_ms must be an integer: {error}"))?,
                );
            }
            "min_throughput_per_minute" => {
                constraints.min_throughput_per_minute =
                    Some(value.trim().parse::<u32>().map_err(|error| {
                        format!("min_throughput_per_minute must be an integer: {error}")
                    })?);
            }
            "model_policy" => constraints.model_policy = Some(value.trim().to_string()),
            "model_family" => constraints.model_family = Some(value.trim().to_string()),
            other => {
                return Err(format!(
                    "Unsupported capability constraint key {other}. Supported keys: accelerator_vendor, accelerator_family, min_memory_gb, max_latency_ms, min_throughput_per_minute, model_policy, model_family"
                ));
            }
        }
    }
    Ok(constraints)
}

fn submit_signed_network_request(
    state: &mut crate::app_state::RenderState,
    request_type: String,
    payload: String,
    skill_scope_id: Option<String>,
    credit_envelope_ref: Option<String>,
    budget_sats: u64,
    timeout_seconds: u64,
    target_provider_pubkeys: Vec<String>,
) -> Result<String, String> {
    let configured_relays = state.configured_provider_relay_urls();
    let request_event = build_nip90_request_event_for_network_submission(
        state.nostr_identity.as_ref(),
        request_type.as_str(),
        payload.as_str(),
        skill_scope_id.as_deref(),
        credit_envelope_ref.as_deref(),
        crate::app_state::BuyerResolutionMode::Race,
        budget_sats,
        timeout_seconds,
        configured_relays.as_slice(),
        target_provider_pubkeys.as_slice(),
    )?;
    submit_signed_network_request_with_event(
        state,
        request_type,
        payload,
        skill_scope_id,
        credit_envelope_ref,
        budget_sats,
        timeout_seconds,
        target_provider_pubkeys,
        request_event,
    )
}

fn submit_mission_control_buy_mode_request(
    state: &mut crate::app_state::RenderState,
    target_provider_pubkeys: Vec<String>,
) -> Result<String, String> {
    let configured_relays = state.configured_provider_relay_urls();
    let request_event = build_mission_control_buy_mode_request_event(
        state.nostr_identity.as_ref(),
        configured_relays.as_slice(),
        target_provider_pubkeys.as_slice(),
    )?;
    submit_signed_network_request_with_event(
        state,
        crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
        MISSION_CONTROL_BUY_MODE_PROMPT.to_string(),
        None,
        None,
        crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
        crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
        target_provider_pubkeys,
        request_event,
    )
}

pub(super) fn run_mission_control_buy_mode_tick(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if !state.mission_control_buy_mode_enabled() || !state.mission_control.buy_mode_loop_enabled {
        return false;
    }
    if let Some(reason) =
        crate::app_state::mission_control_buy_mode_start_block_reason(&state.spark_wallet)
    {
        state.mission_control.toggle_buy_mode_loop(now);
        state.provider_runtime.last_error_detail = Some(reason.clone());
        state.mission_control.record_error(reason);
        return true;
    }
    if !state.mission_control.buy_mode_dispatch_due(now) {
        return false;
    }
    if state
        .network_requests
        .has_in_flight_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
    {
        return false;
    }

    let now_epoch_seconds = current_epoch_seconds();
    let target_selection = state
        .autopilot_chat
        .select_autopilot_buy_mode_target(now_epoch_seconds);
    let Some(target_provider_pubkey) = target_selection.selected_peer_pubkey.clone() else {
        state.mission_control.schedule_buy_mode_retry_with_interval(
            now,
            crate::app_state::MISSION_CONTROL_BUY_MODE_BLOCKED_RETRY_INTERVAL,
        );
        let detail = target_selection.blocked_reason.unwrap_or_else(|| {
            "Buy Mode blocked: no eligible Autopilot peer is available".to_string()
        });
        let blocked_reason_code = target_selection
            .blocked_reason_code
            .as_deref()
            .unwrap_or("unknown");
        let blocked_signature = format!(
            "{blocked_reason_code}:{}:{}:{detail}",
            target_selection.observed_peer_count, target_selection.eligible_peer_count
        );
        if state
            .mission_control
            .should_emit_buy_mode_blocked_notice(now, blocked_signature.as_str())
        {
            state.provider_runtime.last_result = Some(detail.clone());
            tracing::info!(
                target: "autopilot_desktop::buy_mode",
                "Buy Mode dispatch blocked: code={} observed_peers={} eligible_peers={} detail={}",
                blocked_reason_code,
                target_selection.observed_peer_count,
                target_selection.eligible_peer_count,
                detail
            );
            state.mission_control.record_action(detail);
        }
        return true;
    };
    state.mission_control.clear_buy_mode_blocked_notice();

    match submit_mission_control_buy_mode_request(state, vec![target_provider_pubkey.clone()]) {
        Ok(request_id) => {
            state
                .autopilot_chat
                .note_buy_mode_target_dispatch(target_provider_pubkey.as_str());
            state.mission_control.schedule_next_buy_mode_dispatch(now);
            tracing::info!(
                target: "autopilot_desktop::buy_mode",
                "Buy Mode dispatched request_id={} budget_sats={} timeout_seconds={} target_provider={}",
                request_id,
                crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                target_provider_pubkey
            );
            state.mission_control.record_action(format!(
                "Buy Mode dispatched {request_id} -> {target_provider_pubkey}"
            ));
            true
        }
        Err(error) => {
            state.mission_control.schedule_buy_mode_retry(now);
            state.provider_runtime.last_error_detail = Some(error.clone());
            tracing::error!(
                target: "autopilot_desktop::buy_mode",
                "Buy Mode dispatch failed: {}",
                error
            );
            state
                .mission_control
                .record_error(format!("Buy Mode dispatch failed: {error}"));
            true
        }
    }
}

fn submit_signed_network_request_with_event(
    state: &mut crate::app_state::RenderState,
    request_type: String,
    payload: String,
    skill_scope_id: Option<String>,
    credit_envelope_ref: Option<String>,
    budget_sats: u64,
    timeout_seconds: u64,
    target_provider_pubkeys: Vec<String>,
    request_event: nostr::Event,
) -> Result<String, String> {
    let published_request_id = request_event.id.clone();
    let command_seq = state.reserve_runtime_command_seq();
    let request_type_for_log = request_type.clone();
    let request_id = state
        .network_requests
        .queue_request_submission(NetworkRequestSubmission {
            request_id: Some(published_request_id.clone()),
            request_type,
            payload,
            resolution_mode: crate::app_state::BuyerResolutionMode::Race,
            target_provider_pubkeys: target_provider_pubkeys.clone(),
            skill_scope_id,
            credit_envelope_ref,
            budget_sats,
            timeout_seconds,
            authority_command_seq: command_seq,
        })?;
    state.network_requests.mark_direct_authority_ready(
        request_id.as_str(),
        "relay-direct",
        Some(published_request_id.as_str()),
    );
    state.provider_runtime.last_result = Some(format!("Queued network request {request_id}"));

    let tracked_request_ids = state
        .network_requests
        .submitted
        .iter()
        .map(|request| request.request_id.clone())
        .collect::<Vec<_>>();
    if let Err(error) = state.queue_provider_nip90_lane_command(
        crate::provider_nip90_lane::ProviderNip90LaneCommand::TrackBuyerRequestIds {
            request_ids: tracked_request_ids,
        },
    ) {
        state.provider_runtime.last_error_detail = Some(error.clone());
        state.provider_runtime.last_result = Some(format!(
            "failed to track buyer request id {} for relay correlation: {}",
            request_id, error
        ));
    }
    if let Err(error) = state.queue_provider_nip90_lane_command(
        crate::provider_nip90_lane::ProviderNip90LaneCommand::PublishEvent {
            request_id: request_id.clone(),
            role: crate::provider_nip90_lane::ProviderNip90PublishRole::Request,
            event: Box::new(request_event),
        },
    ) {
        state.network_requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            request_id.as_str(),
            0,
            0,
            Some(error.as_str()),
        );
        state.provider_runtime.last_error_detail = Some(error.clone());
        state.provider_runtime.last_result = Some(format!(
            "failed to queue NIP-90 request publish for {}: {}",
            request_id, error
        ));
        return Err(error);
    }
    state.provider_runtime.last_result = Some(format!(
        "Queued NIP-90 request {} -> AC cmd#{}",
        request_id, command_seq
    ));
    tracing::info!(
        target: "autopilot_desktop::buyer",
        "Queued NIP-90 request request_id={} request_type={} budget_sats={} timeout_seconds={} command_seq={} published_event_id={}",
        request_id,
        request_type_for_log,
        budget_sats,
        timeout_seconds,
        command_seq,
        published_request_id
    );

    if local_network_request_inject_enabled() {
        state
            .job_inbox
            .upsert_network_request(JobInboxNetworkRequest {
                request_id: request_id.clone(),
                requester: "network-buyer".to_string(),
                demand_source: crate::app_state::JobDemandSource::OpenNetwork,
                request_kind: nostr::nip90::KIND_JOB_TEXT_GENERATION,
                capability: "local.injected.request".to_string(),
                execution_input: Some(state.network_requests.submitted.last().map_or_else(
                    || "Execute the injected local network request payload.".to_string(),
                    |request| request.payload.clone(),
                )),
                execution_prompt: Some(state.network_requests.submitted.last().map_or_else(
                    || "Execute the injected local network request payload.".to_string(),
                    |request| request.payload.clone(),
                )),
                execution_params: Vec::new(),
                requested_model: Some("llama3.2:latest".to_string()),
                requested_output_mime: Some("text/plain".to_string()),
                target_provider_pubkeys,
                encrypted: false,
                encrypted_payload: None,
                parsed_event_shape: None,
                raw_event_json: None,
                skill_scope_id: None,
                skl_manifest_a: None,
                skl_manifest_event_id: None,
                sa_tick_request_event_id: Some(request_id.clone()),
                sa_tick_result_event_id: None,
                ac_envelope_event_id: None,
                price_sats: budget_sats,
                ttl_seconds: timeout_seconds,
                created_at_epoch_seconds: Some(current_epoch_seconds()),
                expires_at_epoch_seconds: Some(
                    current_epoch_seconds().saturating_add(timeout_seconds),
                ),
                validation: JobInboxValidation::Pending,
            });
    }
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    refresh_sync_health(state);
    Ok(request_id)
}

fn local_network_request_inject_enabled() -> bool {
    std::env::var("OPENAGENTS_LOCAL_NETWORK_REQUEST_INJECT")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn extract_target_provider_pubkeys(payload: &str) -> Vec<String> {
    let mut providers = Vec::<String>::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return providers;
    };

    if let Some(provider) = value
        .get("target_provider_pubkey")
        .and_then(serde_json::Value::as_str)
    {
        providers.push(provider.to_string());
    }
    if let Some(provider) = value
        .get("target_provider")
        .and_then(serde_json::Value::as_str)
    {
        providers.push(provider.to_string());
    }
    if let Some(provider_list) = value
        .get("target_provider_pubkeys")
        .and_then(serde_json::Value::as_array)
    {
        providers.extend(
            provider_list
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string),
        );
    }

    providers = providers
        .into_iter()
        .map(|provider| provider.trim().to_string())
        .filter(|provider| !provider.is_empty())
        .collect::<Vec<_>>();
    providers.sort();
    providers.dedup();
    providers
}

fn build_nip90_request_event_for_network_submission(
    identity: Option<&nostr::NostrIdentity>,
    request_type: &str,
    payload: &str,
    skill_scope_id: Option<&str>,
    credit_envelope_ref: Option<&str>,
    resolution_mode: crate::app_state::BuyerResolutionMode,
    budget_sats: u64,
    timeout_seconds: u64,
    relay_urls: &[String],
    target_provider_pubkeys: &[String],
) -> Result<nostr::Event, String> {
    let Some(identity) = identity else {
        return Err("Cannot publish NIP-90 request: Nostr identity unavailable".to_string());
    };

    let request_kind = nip90_request_kind_for_request_type(request_type);
    let mut request = nostr::nip90::JobRequest::new(request_kind)
        .map_err(|error| format!("Cannot build NIP-90 request: {error}"))?
        .add_input(nostr::nip90::JobInput::text(payload))
        .add_param("request_type", request_type)
        .add_param("oa_resolution_mode", resolution_mode.label())
        .add_param("timeout_seconds", timeout_seconds.to_string())
        .with_bid(budget_sats.saturating_mul(1000));
    if let Some(scope_id) = skill_scope_id {
        let scope_id = scope_id.trim();
        if !scope_id.is_empty() {
            request = request.add_param("skill_scope_id", scope_id);
        }
    }
    if let Some(envelope_ref) = credit_envelope_ref {
        let envelope_ref = envelope_ref.trim();
        if !envelope_ref.is_empty() {
            request = request.add_param("credit_envelope_ref", envelope_ref);
        }
    }
    let normalized_relays = relay_urls
        .iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    if normalized_relays.is_empty() {
        return Err(
            "Cannot publish NIP-90 request: no relay URLs configured for request publication"
                .to_string(),
        );
    }
    for relay in normalized_relays {
        request = request.add_relay(relay);
    }
    for provider in target_provider_pubkeys {
        let provider = provider.trim();
        if !provider.is_empty() {
            request = request.add_service_provider(provider.to_string());
        }
    }

    let template = nostr::nip90::create_job_request_event(&request);
    sign_nip90_template(identity, &template)
}

pub(crate) fn build_mission_control_buy_mode_request_event(
    identity: Option<&nostr::NostrIdentity>,
    relay_urls: &[String],
    target_provider_pubkeys: &[String],
) -> Result<nostr::Event, String> {
    let Some(identity) = identity else {
        return Err("Cannot publish NIP-90 request: Nostr identity unavailable".to_string());
    };

    let normalized_relays = relay_urls
        .iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    if normalized_relays.is_empty() {
        return Err(
            "Cannot publish NIP-90 request: no relay URLs configured for request publication"
                .to_string(),
        );
    }

    let mut request =
        nostr::nip90::JobRequest::new(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_KIND)
            .map_err(|error| format!("Cannot build NIP-90 request: {error}"))?
            .add_input(
                nostr::nip90::JobInput::text(MISSION_CONTROL_BUY_MODE_PROMPT).with_marker("prompt"),
            )
            .add_param(
                "request_type",
                crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE,
            )
            .add_param(
                "oa_resolution_mode",
                crate::app_state::BuyerResolutionMode::Race.label(),
            )
            .add_param(
                "timeout_seconds",
                crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS.to_string(),
            )
            .add_param(
                "oa_dispatch_nonce",
                mission_control_buy_mode_dispatch_nonce(),
            )
            .with_bid(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS.saturating_mul(1000));
    for relay in normalized_relays {
        request = request.add_relay(relay);
    }
    for provider in target_provider_pubkeys {
        let provider = provider.trim();
        if !provider.is_empty() {
            request = request.add_service_provider(provider.to_string());
        }
    }

    let template = nostr::nip90::create_job_request_event(&request);
    sign_nip90_template(identity, &template)
}

fn mission_control_buy_mode_dispatch_nonce() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn nip90_request_kind_for_request_type(request_type: &str) -> u16 {
    let request_type = request_type.trim().to_ascii_lowercase();
    if request_type.contains("summary") || request_type.contains("summariz") {
        nostr::nip90::KIND_JOB_SUMMARIZATION
    } else if request_type.contains("translate") {
        nostr::nip90::KIND_JOB_TRANSLATION
    } else if request_type.contains("extract") {
        nostr::nip90::KIND_JOB_TEXT_EXTRACTION
    } else {
        nostr::nip90::KIND_JOB_TEXT_GENERATION
    }
}

fn sign_nip90_template(
    identity: &nostr::NostrIdentity,
    template: &nostr::EventTemplate,
) -> Result<nostr::Event, String> {
    let private_key = parse_nostr_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign NIP-90 request: {error}"))
}

fn parse_nostr_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

const STARTER_DEMAND_BUDGET_SATS_ENV: &str = "OPENAGENTS_STARTER_DEMAND_BUDGET_SATS";
const STARTER_DEMAND_DISPATCH_INTERVAL_ENV: &str =
    "OPENAGENTS_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS";
const STARTER_DEMAND_MAX_INFLIGHT_ENV: &str = "OPENAGENTS_STARTER_DEMAND_MAX_INFLIGHT";
const STARTER_DEMAND_LOCAL_SIMULATOR_ENV: &str = "OPENAGENTS_ENABLE_LOCAL_STARTER_DEMAND_SIMULATOR";
const STARTER_DEMAND_REQUEST_TIMEOUT_SECONDS: u64 = 75;
const HOSTED_STARTER_DEMAND_POLL_INTERVAL_SECONDS: u64 = 3;
const HOSTED_STARTER_DEMAND_HEARTBEAT_RETRY_SECONDS: u64 = 3;

pub(super) fn run_auto_starter_demand_generator(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    if starter_demand_local_simulator_enabled() {
        return run_local_starter_demand_simulator(state, now);
    }

    run_hosted_starter_demand_sync(state, now)
}

fn run_hosted_starter_demand_sync(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    let active_starter_request_id = state
        .active_job
        .job
        .as_ref()
        .filter(|job| job.demand_source == crate::app_state::JobDemandSource::StarterDemand)
        .map(|job| job.request_id.clone());
    let clear_hosted_rows = |state: &mut crate::app_state::RenderState, reason: &str| {
        let mut changed = state
            .starter_jobs
            .clear_hosted_offers_except(reason, active_starter_request_id.as_deref());
        let removed = state.job_inbox.remove_requests_by_demand_source(
            crate::app_state::JobDemandSource::StarterDemand,
            active_starter_request_id.as_deref(),
        );
        changed |= removed > 0;
        changed
    };

    if state.settings.document.primary_relay_url
        != crate::app_state::DEFAULT_NEXUS_PRIMARY_RELAY_URL
    {
        return clear_hosted_rows(
            state,
            "Hosted starter jobs are only available on the OpenAgents Nexus relay.",
        );
    }
    if !matches!(state.provider_runtime.mode, ProviderMode::Online)
        || state.provider_nip90_lane.connected_relays == 0
    {
        return clear_hosted_rows(
            state,
            "Hosted starter jobs become available after Go Online on the OpenAgents Nexus.",
        );
    }
    if state
        .starter_jobs
        .next_hosted_sync_due_at
        .is_some_and(|next_due_at| now < next_due_at)
    {
        return false;
    }

    let Some(control_base_url) = state.hosted_control_base_url.clone() else {
        return clear_hosted_rows(
            state,
            "Hosted starter jobs require an OpenAgents control base URL.",
        );
    };
    let Some(bearer_auth) = state.hosted_control_bearer_token.clone() else {
        return clear_hosted_rows(
            state,
            "Hosted starter jobs require an authenticated OpenAgents session.",
        );
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            let message = format!("starter demand client initialization failed: {error}");
            state.starter_jobs.last_error = Some(message.clone());
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_error_detail = Some(message.clone());
            state.provider_runtime.last_result = Some(message);
            return true;
        }
    };
    let poll_request = crate::starter_demand_client::StarterDemandPollRequest {
        provider_nostr_pubkey: state
            .nostr_identity
            .as_ref()
            .map(|identity| identity.npub.clone()),
        primary_relay_url: Some(state.settings.document.primary_relay_url.clone()),
    };
    let response = match crate::starter_demand_client::poll_starter_demand_blocking(
        &client,
        control_base_url.as_str(),
        bearer_auth.as_str(),
        &poll_request,
    ) {
        Ok(response) => response,
        Err(error) => {
            state.starter_jobs.next_hosted_sync_due_at = Some(
                now + std::time::Duration::from_secs(HOSTED_STARTER_DEMAND_POLL_INTERVAL_SECONDS),
            );
            state.starter_jobs.last_error = Some(error.clone());
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.provider_runtime.last_result =
                Some(format!("hosted starter demand sync failed: {error}"));
            return true;
        }
    };

    if !response.eligible {
        let reason = response
            .reason
            .as_deref()
            .unwrap_or("Hosted starter jobs are not eligible for this session.");
        return clear_hosted_rows(state, reason);
    }

    let hosted_jobs = response
        .offers
        .iter()
        .map(|offer| crate::app_state::StarterJobRow {
            job_id: offer.request_id.clone(),
            summary: offer
                .execution_input
                .clone()
                .unwrap_or_else(|| offer.capability.clone()),
            payout_sats: offer.price_sats,
            eligible: true,
            status: hosted_offer_status_to_starter_job_status(offer.status.as_str()),
            payout_pointer: None,
            start_confirm_by_unix_ms: offer.start_confirm_by_unix_ms,
            execution_started_at_unix_ms: offer.execution_started_at_unix_ms,
            execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms,
            last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms,
            next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms,
        })
        .collect::<Vec<_>>();
    state.starter_jobs.sync_hosted_offers(
        hosted_jobs,
        response.budget_cap_sats,
        response.budget_allocated_sats,
        response.dispatch_interval_seconds,
        response.max_active_offers_per_session,
        Some(now + std::time::Duration::from_secs(HOSTED_STARTER_DEMAND_POLL_INTERVAL_SECONDS)),
        "Synced hosted starter-demand offers from Nexus",
    );

    let keep_request_id = active_starter_request_id.clone().or_else(|| {
        response
            .offers
            .first()
            .map(|offer| offer.request_id.clone())
    });
    let removed = state.job_inbox.remove_requests_by_demand_source(
        crate::app_state::JobDemandSource::StarterDemand,
        keep_request_id.as_deref(),
    );
    let mut changed = removed > 0;
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());

    for offer in &response.offers {
        let request = JobInboxNetworkRequest {
            request_id: offer.request_id.clone(),
            requester: offer.requester.clone(),
            demand_source: crate::app_state::JobDemandSource::StarterDemand,
            request_kind: offer.request_kind,
            capability: offer.capability.clone(),
            execution_input: offer.execution_input.clone(),
            execution_prompt: offer.execution_input.clone(),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            encrypted: false,
            encrypted_payload: None,
            parsed_event_shape: Some(format!(
                "starter.offer authority={} status={} ttl_seconds={}",
                response.authority, offer.status, offer.ttl_seconds
            )),
            raw_event_json: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(offer.request_id.clone()),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: offer.price_sats,
            ttl_seconds: offer.ttl_seconds,
            created_at_epoch_seconds: Some(now_epoch_seconds),
            expires_at_epoch_seconds: Some(now_epoch_seconds.saturating_add(offer.ttl_seconds)),
            validation: JobInboxValidation::Valid,
        };
        let is_new = !state
            .job_inbox
            .requests
            .iter()
            .any(|existing| existing.request_id == offer.request_id);
        state.job_inbox.upsert_network_request(request.clone());
        if is_new {
            changed = true;
            state.earn_job_lifecycle_projection.record_ingress_request(
                &request,
                now_epoch_seconds,
                "starter.hosted.ingress",
            );
            state.earn_kernel_receipts.record_ingress_request(
                &request,
                now_epoch_seconds,
                "starter.hosted.ingress",
            );
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id: format!("starter.hosted.offer:{}", offer.request_id),
                    domain: crate::app_state::ActivityEventDomain::Network,
                    source_tag: "starter.hosted".to_string(),
                    summary: "Hosted starter offer arrived".to_string(),
                    detail: format!(
                        "request={} payout_sats={} authority={} relay={}",
                        offer.request_id,
                        offer.price_sats,
                        response.authority,
                        response.hosted_nexus_relay_url
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
        }
    }

    state.provider_runtime.last_result = Some(format!(
        "hosted starter demand synced ({} offer{})",
        response.offers.len(),
        if response.offers.len() == 1 { "" } else { "s" }
    ));
    changed
}

pub(super) fn run_hosted_starter_lease_heartbeat(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    let Some((request_id, demand_source, stage)) = state
        .active_job
        .job
        .as_ref()
        .map(|job| (job.request_id.clone(), job.demand_source, job.stage))
    else {
        return false;
    };
    if demand_source != crate::app_state::JobDemandSource::StarterDemand || stage.is_terminal() {
        return false;
    }
    if state
        .starter_jobs
        .next_hosted_heartbeat_due_at
        .is_some_and(|next_due_at| now < next_due_at)
    {
        return false;
    }

    let Some(control_base_url) = state.hosted_control_base_url.clone() else {
        return false;
    };
    let Some(bearer_auth) = state.hosted_control_bearer_token.clone() else {
        return false;
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            state.starter_jobs.last_error = Some(format!(
                "starter demand heartbeat client initialization failed: {error}"
            ));
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            return true;
        }
    };
    let provider_nostr_pubkey = state
        .nostr_identity
        .as_ref()
        .map(|identity| identity.npub.as_str());
    match crate::starter_demand_client::heartbeat_starter_demand_offer_blocking(
        &client,
        control_base_url.as_str(),
        bearer_auth.as_str(),
        request_id.as_str(),
        provider_nostr_pubkey,
    ) {
        Ok(response) => {
            state.starter_jobs.mark_heartbeat(
                response.request_id.as_str(),
                response.last_heartbeat_at_unix_ms,
                response.next_heartbeat_due_at_unix_ms,
                response.execution_expires_at_unix_ms,
                Some(
                    now + std::time::Duration::from_secs(
                        response.heartbeat_interval_seconds.max(1),
                    ),
                ),
            );
            state.starter_jobs.next_hosted_sync_due_at = Some(now);
            false
        }
        Err(error) => {
            let normalized = error.to_ascii_lowercase();
            if normalized.contains("starter_offer_heartbeat_missed")
                || normalized.contains("starter_offer_execution_expired")
                || normalized.contains("starter_offer_not_running")
                || normalized.contains("starter_offer_not_found")
            {
                state.starter_jobs.mark_released(
                    request_id.as_str(),
                    "lease lost during hosted starter execution",
                );
                state.starter_jobs.next_hosted_sync_due_at = Some(now);
                state.active_job.last_error = Some(format!(
                    "hosted starter lease lost for {}: {}",
                    request_id, error
                ));
                state.active_job.load_state = crate::app_state::PaneLoadState::Error;
                state.provider_runtime.last_error_detail =
                    Some(format!("hosted starter lease lost: {error}"));
                state.provider_runtime.last_result =
                    Some(format!("hosted starter lease lost for {}", request_id));
                state.provider_runtime.last_authoritative_error_class =
                    Some(EarnFailureClass::Execution);
                if let Err(fail_error) = fail_hosted_starter_active_job_for_lease_loss(
                    state,
                    "active_job.hosted_starter_lease_lost",
                ) {
                    state.active_job.last_error = Some(fail_error);
                }
                return true;
            }

            state.starter_jobs.next_hosted_heartbeat_due_at = Some(
                now + std::time::Duration::from_secs(HOSTED_STARTER_DEMAND_HEARTBEAT_RETRY_SECONDS),
            );
            state.starter_jobs.last_error = Some(format!(
                "hosted starter heartbeat reconciliation failed: {error}"
            ));
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            true
        }
    }
}

fn run_local_starter_demand_simulator(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    state.starter_jobs.apply_dispatch_controls(
        starter_demand_budget_sats(),
        starter_demand_dispatch_interval_seconds(),
        starter_demand_max_inflight_jobs(),
    );

    if !matches!(state.provider_runtime.mode, ProviderMode::Online)
        || state.provider_nip90_lane.connected_relays == 0
    {
        return false;
    }

    let dispatched = match state.starter_jobs.dispatch_next_if_due(now) {
        Ok(Some(job)) => job,
        Ok(None) => return false,
        Err(error) => {
            state.starter_jobs.last_error = Some(error.clone());
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.provider_runtime.last_result =
                Some(format!("starter demand dispatch blocked: {error}"));
            return true;
        }
    };

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    match queue_starter_demand_request(state, &dispatched, now_epoch_seconds) {
        Ok(request_id) => {
            state.provider_runtime.last_result = Some(format!(
                "starter demand dispatched {} -> {}",
                dispatched.job_id, request_id
            ));
            state
                .activity_feed
                .upsert_event(crate::app_state::ActivityEventRow {
                    event_id: format!("starter.quest.dispatch:{}", dispatched.job_id),
                    domain: crate::app_state::ActivityEventDomain::Network,
                    source_tag: "starter.quest".to_string(),
                    summary: "Starter demand dispatched".to_string(),
                    detail: format!(
                        "job={} request={} payout_sats={} remaining_budget={}",
                        dispatched.job_id,
                        request_id,
                        dispatched.payout_sats,
                        state.starter_jobs.budget_remaining_sats()
                    ),
                    occurred_at_epoch_seconds: now_epoch_seconds,
                });
            state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
            true
        }
        Err(error) => {
            let _ = state
                .starter_jobs
                .rollback_dispatched_job(dispatched.job_id.as_str());
            state.starter_jobs.last_error = Some(error.clone());
            state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.provider_runtime.last_result =
                Some(format!("starter demand dispatch failed: {error}"));
            true
        }
    }
}

fn starter_demand_local_simulator_enabled() -> bool {
    std::env::var(STARTER_DEMAND_LOCAL_SIMULATOR_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn queue_starter_demand_request(
    state: &mut crate::app_state::RenderState,
    starter_job: &crate::app_state::StarterJobRow,
    now_epoch_seconds: u64,
) -> Result<String, String> {
    let request_type = "starter.quest.text_generation".to_string();
    let payload = serde_json::json!({
        "starter_job_id": starter_job.job_id,
        "prompt": starter_job.summary,
    })
    .to_string();
    let timeout_seconds = STARTER_DEMAND_REQUEST_TIMEOUT_SECONDS;
    let command_seq = state.reserve_runtime_command_seq();
    let request_id = state
        .network_requests
        .queue_request_submission(NetworkRequestSubmission {
            request_id: None,
            request_type,
            payload,
            resolution_mode: crate::app_state::BuyerResolutionMode::Race,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            credit_envelope_ref: None,
            budget_sats: starter_job.payout_sats,
            timeout_seconds,
            authority_command_seq: command_seq,
        })?;
    state.network_requests.mark_direct_authority_ready(
        request_id.as_str(),
        "starter-hosted",
        Some(request_id.as_str()),
    );

    let starter_request = JobInboxNetworkRequest {
        request_id: request_id.clone(),
        requester: "starter-demand".to_string(),
        demand_source: crate::app_state::JobDemandSource::StarterDemand,
        request_kind: nostr::nip90::KIND_JOB_TEXT_GENERATION,
        capability: "starter.quest.dispatch".to_string(),
        execution_input: Some(format!(
            "Starter quest {}\n\n{}",
            starter_job.job_id, starter_job.summary
        )),
        execution_prompt: Some(format!(
            "Starter quest {}\n\n{}",
            starter_job.job_id, starter_job.summary
        )),
        execution_params: Vec::new(),
        requested_model: Some("llama3.2:latest".to_string()),
        requested_output_mime: Some("text/plain".to_string()),
        target_provider_pubkeys: Vec::new(),
        encrypted: false,
        encrypted_payload: None,
        parsed_event_shape: None,
        raw_event_json: None,
        skill_scope_id: None,
        skl_manifest_a: None,
        skl_manifest_event_id: None,
        sa_tick_request_event_id: Some(request_id.clone()),
        sa_tick_result_event_id: None,
        ac_envelope_event_id: None,
        price_sats: starter_job.payout_sats,
        ttl_seconds: timeout_seconds,
        created_at_epoch_seconds: Some(now_epoch_seconds),
        expires_at_epoch_seconds: Some(now_epoch_seconds.saturating_add(timeout_seconds)),
        validation: JobInboxValidation::Valid,
    };
    state
        .job_inbox
        .upsert_network_request(starter_request.clone());
    state.earn_job_lifecycle_projection.record_ingress_request(
        &starter_request,
        now_epoch_seconds,
        "starter.quest.ingress",
    );
    state.earn_kernel_receipts.record_ingress_request(
        &starter_request,
        now_epoch_seconds,
        "starter.quest.ingress",
    );
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
    refresh_sync_health(state);

    if let Some(job) = state
        .starter_jobs
        .jobs
        .iter_mut()
        .find(|job| job.job_id == starter_job.job_id)
    {
        job.summary = format!("{} [request={}]", job.summary, request_id);
    }

    state
        .activity_feed
        .upsert_event(crate::app_state::ActivityEventRow {
            event_id: format!("starter.quest.request:{}", request_id),
            domain: crate::app_state::ActivityEventDomain::Job,
            source_tag: "starter.quest".to_string(),
            summary: "Starter request queued".to_string(),
            detail: format!(
                "request={} job={} payout_sats={} timeout={}s",
                request_id, starter_job.job_id, starter_job.payout_sats, timeout_seconds
            ),
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
    state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
    Ok(request_id)
}

fn starter_demand_budget_sats() -> u64 {
    parse_env_u64_with_default(STARTER_DEMAND_BUDGET_SATS_ENV, 5_000, 1, 5_000_000)
}

fn starter_demand_dispatch_interval_seconds() -> u64 {
    parse_env_u64_with_default(STARTER_DEMAND_DISPATCH_INTERVAL_ENV, 12, 1, 3_600)
}

fn starter_demand_max_inflight_jobs() -> usize {
    parse_env_u64_with_default(STARTER_DEMAND_MAX_INFLIGHT_ENV, 1, 1, 1) as usize
}

fn parse_env_u64_with_default(key: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

const RECIPROCAL_LOOP_AUTOSTART_ENV: &str = "OPENAGENTS_RECIPROCAL_LOOP_AUTOSTART";
const RECIPROCAL_LOOP_PEER_PUBKEY_ENV: &str = "OPENAGENTS_RECIPROCAL_LOOP_PEER_PUBKEY";
const RECIPROCAL_LOOP_REQUEST_TYPE: &str = "loop.pingpong.10sat";

pub(super) fn run_reciprocal_loop_engine_tick(state: &mut crate::app_state::RenderState) -> bool {
    let mut changed = false;
    let now_epoch_seconds = current_epoch_seconds();
    if sync_reciprocal_loop_identity_and_peer(state) {
        changed = true;
    }

    let outbound_failed_before = state.reciprocal_loop.local_to_peer_failed;
    let inbound_failed_before = state.reciprocal_loop.peer_to_local_failed;
    if state
        .reciprocal_loop
        .reconcile_outbound_terminal_statuses(state.network_requests.submitted.as_slice())
    {
        changed = true;
    }
    if state
        .reciprocal_loop
        .reconcile_inbound_history(state.job_history.rows.as_slice())
    {
        changed = true;
    }

    if state.reciprocal_loop.local_to_peer_failed > outbound_failed_before {
        let detail = state
            .reciprocal_loop
            .last_failure_detail
            .clone()
            .unwrap_or_else(|| "reciprocal loop outbound payment failed".to_string());
        let terminal = state.reciprocal_loop.record_recoverable_failure(
            crate::app_state::ReciprocalLoopFailureClass::Payment,
            detail.as_str(),
            now_epoch_seconds,
        );
        set_reciprocal_loop_runtime_failure(
            state,
            crate::app_state::ReciprocalLoopFailureClass::Payment,
            if terminal {
                crate::app_state::ReciprocalLoopFailureDisposition::Terminal
            } else {
                crate::app_state::ReciprocalLoopFailureDisposition::Recoverable
            },
            crate::app_state::EarnFailureClass::Payment,
            detail.as_str(),
        );
        changed = true;
    }

    if state.reciprocal_loop.peer_to_local_failed > inbound_failed_before {
        let detail = state
            .reciprocal_loop
            .last_failure_detail
            .clone()
            .unwrap_or_else(|| "reciprocal loop inbound job failed".to_string());
        let terminal = state.reciprocal_loop.record_recoverable_failure(
            crate::app_state::ReciprocalLoopFailureClass::Job,
            detail.as_str(),
            now_epoch_seconds,
        );
        set_reciprocal_loop_runtime_failure(
            state,
            crate::app_state::ReciprocalLoopFailureClass::Job,
            if terminal {
                crate::app_state::ReciprocalLoopFailureDisposition::Terminal
            } else {
                crate::app_state::ReciprocalLoopFailureDisposition::Recoverable
            },
            crate::app_state::EarnFailureClass::Reconciliation,
            detail.as_str(),
        );
        changed = true;
    }

    if state
        .reciprocal_loop
        .clear_retry_backoff_if_elapsed(now_epoch_seconds)
    {
        changed = true;
    }
    state
        .reciprocal_loop
        .mark_peer_wait_started(now_epoch_seconds);

    if state
        .reciprocal_loop
        .outbound_stale_timed_out(now_epoch_seconds)
        && let Some(request_id) = state.reciprocal_loop.mark_outbound_stale_timeout()
    {
        let detail = format!(
            "outbound request {} exceeded stale timeout ({}s)",
            request_id, state.reciprocal_loop.stale_timeout_seconds
        );
        let terminal = state.reciprocal_loop.record_recoverable_failure(
            crate::app_state::ReciprocalLoopFailureClass::Dispatch,
            detail.as_str(),
            now_epoch_seconds,
        );
        set_reciprocal_loop_runtime_failure(
            state,
            crate::app_state::ReciprocalLoopFailureClass::Dispatch,
            if terminal {
                crate::app_state::ReciprocalLoopFailureDisposition::Terminal
            } else {
                crate::app_state::ReciprocalLoopFailureDisposition::Recoverable
            },
            crate::app_state::EarnFailureClass::Execution,
            detail.as_str(),
        );
        changed = true;
    }

    if state
        .reciprocal_loop
        .inbound_wait_timed_out(now_epoch_seconds)
    {
        state.reciprocal_loop.mark_inbound_stale_timeout();
        let detail = format!(
            "peer inbound wait exceeded stale timeout ({}s)",
            state.reciprocal_loop.stale_timeout_seconds
        );
        let terminal = state.reciprocal_loop.record_recoverable_failure(
            crate::app_state::ReciprocalLoopFailureClass::Job,
            detail.as_str(),
            now_epoch_seconds,
        );
        set_reciprocal_loop_runtime_failure(
            state,
            crate::app_state::ReciprocalLoopFailureClass::Job,
            if terminal {
                crate::app_state::ReciprocalLoopFailureDisposition::Terminal
            } else {
                crate::app_state::ReciprocalLoopFailureDisposition::Recoverable
            },
            crate::app_state::EarnFailureClass::Reconciliation,
            detail.as_str(),
        );
        changed = true;
    }

    if let Some(limit_violation) = state.reciprocal_loop.in_flight_limit_violation() {
        state.reciprocal_loop.record_terminal_failure(
            crate::app_state::ReciprocalLoopFailureClass::Dispatch,
            limit_violation.as_str(),
        );
        set_reciprocal_loop_runtime_failure(
            state,
            crate::app_state::ReciprocalLoopFailureClass::Dispatch,
            crate::app_state::ReciprocalLoopFailureDisposition::Terminal,
            crate::app_state::EarnFailureClass::Execution,
            limit_violation.as_str(),
        );
        changed = true;
        return changed;
    }

    if state.reciprocal_loop.in_backoff_window(now_epoch_seconds) {
        return changed;
    }

    if !state.reciprocal_loop.running
        && !state.reciprocal_loop.kill_switch_active
        && reciprocal_loop_autostart_enabled()
    {
        if state.reciprocal_loop.start().is_ok() {
            state.provider_runtime.last_result = Some("reciprocal loop autostarted".to_string());
            changed = true;
        }
    }

    if !state.reciprocal_loop.ready_to_dispatch() {
        return changed;
    }

    let Some(peer_pubkey) = state.reciprocal_loop.peer_pubkey.clone() else {
        return changed;
    };
    let amount_sats = state.reciprocal_loop.amount_sats;
    let timeout_seconds = state.reciprocal_loop.timeout_seconds;
    let skill_scope_id = Some(state.reciprocal_loop.skill_scope_id.clone());
    let credit_envelope_ref = state.ac_lane.envelope_event_id.clone();
    let sequence = state
        .reciprocal_loop
        .local_to_peer_dispatched
        .saturating_add(1);
    let payload = reciprocal_loop_payload(peer_pubkey.as_str(), amount_sats, sequence);

    match submit_signed_network_request(
        state,
        RECIPROCAL_LOOP_REQUEST_TYPE.to_string(),
        payload,
        skill_scope_id,
        credit_envelope_ref,
        amount_sats,
        timeout_seconds,
        vec![peer_pubkey],
    ) {
        Ok(request_id) => {
            state
                .reciprocal_loop
                .register_outbound_dispatch(request_id.as_str(), now_epoch_seconds);
            state.provider_runtime.last_result = Some(format!(
                "reciprocal loop dispatched {} request {}",
                format_sats_amount(amount_sats),
                request_id
            ));
            if state.provider_runtime.last_authoritative_error_class
                == Some(EarnFailureClass::Execution)
            {
                state.provider_runtime.last_authoritative_error_class = None;
            }
            state.provider_runtime.last_authoritative_status = Some("ok".to_string());
            changed = true;
        }
        Err(error) => {
            let (failure_class, disposition, earn_failure_class) =
                classify_reciprocal_loop_dispatch_error(error.as_str());
            match disposition {
                crate::app_state::ReciprocalLoopFailureDisposition::Recoverable => {
                    let terminal = state.reciprocal_loop.record_recoverable_failure(
                        failure_class,
                        error.as_str(),
                        now_epoch_seconds,
                    );
                    set_reciprocal_loop_runtime_failure(
                        state,
                        failure_class,
                        if terminal {
                            crate::app_state::ReciprocalLoopFailureDisposition::Terminal
                        } else {
                            crate::app_state::ReciprocalLoopFailureDisposition::Recoverable
                        },
                        earn_failure_class,
                        error.as_str(),
                    );
                }
                crate::app_state::ReciprocalLoopFailureDisposition::Terminal => {
                    state
                        .reciprocal_loop
                        .record_terminal_failure(failure_class, error.as_str());
                    set_reciprocal_loop_runtime_failure(
                        state,
                        failure_class,
                        crate::app_state::ReciprocalLoopFailureDisposition::Terminal,
                        earn_failure_class,
                        error.as_str(),
                    );
                }
            }
            changed = true;
        }
    }

    changed
}

fn set_reciprocal_loop_runtime_failure(
    state: &mut crate::app_state::RenderState,
    class: crate::app_state::ReciprocalLoopFailureClass,
    disposition: crate::app_state::ReciprocalLoopFailureDisposition,
    earn_failure_class: crate::app_state::EarnFailureClass,
    detail: &str,
) {
    let detail = detail.trim();
    let detail = if detail.is_empty() {
        "reciprocal loop runtime failure"
    } else {
        detail
    };
    state.provider_runtime.last_error_detail = Some(detail.to_string());
    state.provider_runtime.last_result = Some(format!(
        "reciprocal loop {} failure class={} detail={}",
        disposition.label(),
        class.label(),
        detail
    ));
    state.provider_runtime.last_authoritative_status = Some(disposition.label().to_string());
    state.provider_runtime.last_authoritative_error_class = Some(earn_failure_class);
}

fn classify_reciprocal_loop_dispatch_error(
    error: &str,
) -> (
    crate::app_state::ReciprocalLoopFailureClass,
    crate::app_state::ReciprocalLoopFailureDisposition,
    crate::app_state::EarnFailureClass,
) {
    let normalized = error.to_ascii_lowercase();
    let relay_markers = ["relay", "websocket", "tls", "publish", "connection"];
    if relay_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return (
            crate::app_state::ReciprocalLoopFailureClass::Dispatch,
            crate::app_state::ReciprocalLoopFailureDisposition::Recoverable,
            crate::app_state::EarnFailureClass::Relay,
        );
    }

    let wallet_markers = ["wallet", "spark", "invoice", "payment", "bolt11"];
    if wallet_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return (
            crate::app_state::ReciprocalLoopFailureClass::Payment,
            crate::app_state::ReciprocalLoopFailureDisposition::Recoverable,
            crate::app_state::EarnFailureClass::Payment,
        );
    }

    (
        crate::app_state::ReciprocalLoopFailureClass::Dispatch,
        crate::app_state::ReciprocalLoopFailureDisposition::Terminal,
        crate::app_state::EarnFailureClass::Execution,
    )
}

fn sync_reciprocal_loop_identity_and_peer(state: &mut crate::app_state::RenderState) -> bool {
    let mut changed = false;
    let before_local = state.reciprocal_loop.local_pubkey.clone();
    let local_pubkey = state
        .nostr_identity
        .as_ref()
        .map(|identity| identity.public_key_hex.as_str());
    state.reciprocal_loop.set_local_pubkey(local_pubkey);
    if before_local != state.reciprocal_loop.local_pubkey {
        changed = true;
    }

    if state.reciprocal_loop.peer_pubkey.is_none()
        && let Some(peer_pubkey) = reciprocal_loop_peer_pubkey_from_env()
    {
        state
            .reciprocal_loop
            .set_peer_pubkey(Some(peer_pubkey.as_str()));
        changed = true;
    }

    changed
}

fn reciprocal_loop_payload(peer_pubkey: &str, amount_sats: u64, sequence: u64) -> String {
    serde_json::json!({
        "prompt": format!(
            "Reciprocal loop task #{}: respond with 'pong' and include the sequence id.",
            sequence
        ),
        "loop_scope": "earn.loop.pingpong.v1",
        "loop_amount_sats": amount_sats,
        "loop_sequence": sequence,
        "target_provider_pubkeys": [peer_pubkey],
    })
    .to_string()
}

fn reciprocal_loop_peer_pubkey_from_env() -> Option<String> {
    std::env::var(RECIPROCAL_LOOP_PEER_PUBKEY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn reciprocal_loop_autostart_enabled() -> bool {
    std::env::var(RECIPROCAL_LOOP_AUTOSTART_ENV)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

pub(super) fn run_reciprocal_loop_action(
    state: &mut crate::app_state::RenderState,
    action: ReciprocalLoopPaneAction,
) -> bool {
    let _ = sync_reciprocal_loop_identity_and_peer(state);
    match action {
        ReciprocalLoopPaneAction::Start => match state.reciprocal_loop.start() {
            Ok(()) => {
                state.provider_runtime.last_result =
                    Some("reciprocal loop started from pane".to_string());
                state.provider_runtime.last_authoritative_status = Some("running".to_string());
            }
            Err(error) => {
                state.provider_runtime.last_error_detail = Some(error.clone());
                state.provider_runtime.last_result =
                    Some(format!("reciprocal loop start failed: {error}"));
            }
        },
        ReciprocalLoopPaneAction::Stop => {
            state
                .reciprocal_loop
                .stop("operator requested stop from pane");
            state.provider_runtime.last_result =
                Some("reciprocal loop stopped from pane".to_string());
            state.provider_runtime.last_authoritative_status = Some("stopped".to_string());
        }
        ReciprocalLoopPaneAction::Reset => {
            state.reciprocal_loop.reset_counters();
            state.provider_runtime.last_result =
                Some("reciprocal loop counters reset from pane".to_string());
        }
    }
    true
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
        StarterJobsPaneAction::ToggleKillSwitch => {
            if !starter_demand_local_simulator_enabled() {
                state.starter_jobs.last_error = Some(
                    "Hosted starter-demand controls live in Nexus; the desktop pane is read-only."
                        .to_string(),
                );
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            }
            let enabled = state.starter_jobs.toggle_kill_switch();
            state.provider_runtime.last_result = Some(if enabled {
                "starter demand kill switch enabled".to_string()
            } else {
                "starter demand kill switch disabled".to_string()
            });
            true
        }
        StarterJobsPaneAction::CompleteSelected => {
            if !starter_demand_local_simulator_enabled() {
                state.starter_jobs.last_error = Some(
                    "Hosted starter jobs settle through the normal provider execution path."
                        .to_string(),
                );
                state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
                return true;
            }
            match state.starter_jobs.start_selected_execution() {
                Ok((job_id, payout_sats)) => {
                    let payout_pointer =
                        resolve_wallet_settlement_pointer_for_starter_payout(state, payout_sats);
                    let Some(payout_pointer) = payout_pointer else {
                        state.starter_jobs.last_error = Some(format!(
                            "Starter quest {} is running but payout is not wallet-confirmed yet",
                            job_id
                        ));
                        state.starter_jobs.load_state = crate::app_state::PaneLoadState::Error;
                        state.provider_runtime.last_result = Some(format!(
                            "starter quest {} awaiting wallet settlement confirmation",
                            job_id
                        ));
                        return true;
                    };
                    match state
                        .starter_jobs
                        .complete_selected_with_payment(&payout_pointer)
                    {
                        Ok((job_id, payout_sats, payout_pointer)) => {
                            state.spark_wallet.last_payment_id = Some(payout_pointer.clone());
                            state.spark_wallet.last_action = Some(format!(
                                "Starter quest payout wallet-confirmed for {job_id} ({})",
                                format_sats_amount(payout_sats)
                            ));
                            state.provider_runtime.last_result =
                                Some(format!("completed starter quest {}", job_id));
                            let receipt_row = crate::app_state::JobHistoryReceiptRow {
                                job_id: job_id.clone(),
                                status: crate::app_state::JobHistoryStatus::Succeeded,
                                demand_source: crate::app_state::JobDemandSource::StarterDemand,
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
                                sa_trajectory_session_id: Some("traj:starter-quest".to_string()),
                                ac_envelope_event_id: state.ac_lane.envelope_event_id.clone(),
                                ac_settlement_event_id: state.ac_lane.settlement_event_id.clone(),
                                ac_default_event_id: None,
                                delivery_proof_id: None,
                                delivery_metering_rule_id: None,
                                delivery_proof_status_label: None,
                                delivery_metered_quantity: None,
                                delivery_accepted_quantity: None,
                                delivery_variance_reason_label: None,
                                delivery_rejection_reason_label: None,
                                payout_sats,
                                result_hash: "sha256:starter-quest-job".to_string(),
                                payment_pointer: payout_pointer.clone(),
                                failure_reason: None,
                                execution_provenance: None,
                            };
                            state.job_history.upsert_row(receipt_row.clone());
                            state.earn_job_lifecycle_projection.record_history_receipt(
                                &receipt_row,
                                receipt_row.completed_at_epoch_seconds,
                                "starter.quest.settlement",
                            );
                            state.earn_kernel_receipts.record_history_receipt(
                                &receipt_row,
                                receipt_row.completed_at_epoch_seconds,
                                "starter.quest.settlement",
                            );
                            state
                                .activity_feed
                                .upsert_event(crate::app_state::ActivityEventRow {
                                    event_id: format!(
                                        "starter.quest.settlement:{}",
                                        payout_pointer
                                    ),
                                    domain: crate::app_state::ActivityEventDomain::Wallet,
                                    source_tag: "starter.quest".to_string(),
                                    summary: "Starter quest payout wallet-confirmed".to_string(),
                                    detail: format!(
                                        "job={} payout_sats={} payment_pointer={}",
                                        job_id, payout_sats, payout_pointer
                                    ),
                                    occurred_at_epoch_seconds: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map_or(0, |duration| duration.as_secs()),
                                });
                            refresh_earnings_scoreboard(state, std::time::Instant::now());
                        }
                        Err(error) => {
                            state.starter_jobs.last_error = Some(error);
                        }
                    }
                }
                Err(error) => {
                    state.starter_jobs.last_error = Some(error);
                }
            }
            true
        }
    }
}

fn resolve_wallet_settlement_pointer_for_starter_payout(
    state: &crate::app_state::RenderState,
    payout_sats: u64,
) -> Option<String> {
    state
        .spark_wallet
        .recent_payments
        .iter()
        .filter(|payment| {
            payment.direction.eq_ignore_ascii_case("receive")
                && payment.status.eq_ignore_ascii_case("succeeded")
                && payment.amount_sats == payout_sats
                && !payment.id.trim().is_empty()
                && !is_synthetic_local_payment_pointer(payment.id.as_str())
        })
        .max_by(|left, right| left.timestamp.cmp(&right.timestamp))
        .map(|payment| payment.id.clone())
}

fn is_synthetic_local_payment_pointer(pointer: &str) -> bool {
    let normalized = pointer.trim().to_ascii_lowercase();
    normalized.starts_with("pay:")
        || normalized.starts_with("pending:")
        || normalized.starts_with("pay-req-")
}

const ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(5);

fn active_job_payment_evidence_refresh_due(
    active_job: &mut crate::app_state::ActiveJobState,
    now: std::time::Instant,
) -> bool {
    if matches!(
        active_job.next_payment_evidence_refresh_at,
        Some(next_refresh_at) if now < next_refresh_at
    ) {
        return false;
    }
    active_job.next_payment_evidence_refresh_at =
        Some(now + ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL);
    true
}

fn note_active_job_waiting_for_payment_evidence(
    active_job: &mut crate::app_state::ActiveJobState,
    provider_runtime: &mut crate::state::provider_runtime::ProviderRuntimeState,
    demand_source: crate::app_state::JobDemandSource,
    request_id: &str,
) {
    let waiting_detail = format!(
        "{} delivered job {} and is awaiting buyer Lightning payment confirmation",
        demand_source.label(),
        request_id
    );
    let legacy_waiting_error = format!(
        "{} delivered job {} is waiting for wallet-authoritative payment evidence",
        demand_source.label(),
        request_id
    );
    let waiting_runtime_error = format!("execution: {waiting_detail}");
    let legacy_waiting_runtime_error = format!("execution: {legacy_waiting_error}");
    let first_waiting_transition =
        active_job.last_action.as_deref() != Some(waiting_detail.as_str());
    if first_waiting_transition {
        active_job.append_event("result delivered; awaiting buyer Lightning payment");
        crate::nip90_compute_domain_events::emit_provider_delivered_awaiting_settlement(
            request_id,
            active_job.pending_bolt11.is_some(),
            active_job.payment_required_invoice_requested,
            active_job.payment_required_feedback_in_flight,
        );
    }
    active_job.last_action = Some(waiting_detail.clone());
    if matches!(
        active_job.last_error.as_deref(),
        Some(error)
            if error == waiting_detail.as_str() || error == legacy_waiting_error.as_str()
    ) {
        active_job.last_error = None;
        active_job.load_state = crate::app_state::PaneLoadState::Ready;
    }
    provider_runtime.last_result = Some(waiting_detail.clone());
    let cleared_waiting_runtime_error = matches!(
        provider_runtime.last_error_detail.as_deref(),
        Some(error)
            if error == waiting_runtime_error.as_str()
                || error == legacy_waiting_runtime_error.as_str()
    );
    if cleared_waiting_runtime_error {
        provider_runtime.last_error_detail = None;
    }
    if provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Payment)
        && (cleared_waiting_runtime_error || provider_runtime.last_error_detail.is_none())
    {
        provider_runtime.last_authoritative_error_class = None;
    }
}

pub(super) fn run_open_network_paid_transition_reconciliation(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) -> bool {
    let Some((request_id, demand_source)) = state
        .active_job
        .job
        .as_ref()
        .map(|job| (job.request_id.clone(), job.demand_source))
    else {
        return false;
    };
    if state
        .active_job
        .job
        .as_ref()
        .is_none_or(|job| job.stage != crate::app_state::JobLifecycleStage::Delivered)
    {
        return false;
    }

    let maybe_existing_pointer = state
        .active_job
        .job
        .as_ref()
        .and_then(|job| job.payment_id.clone());
    let wallet_pointer = maybe_existing_pointer.or_else(|| {
        resolve_wallet_settlement_pointer_for_open_network_job(
            state.active_job.job.as_ref()?,
            state.active_job.pending_bolt11_created_at_epoch_seconds,
            state.job_history.rows.as_slice(),
            state.spark_wallet.recent_payments.as_slice(),
        )
    });

    let Some(wallet_pointer) = wallet_pointer else {
        if !state.active_job.payment_required_failed {
            if (state.active_job.pending_bolt11.is_some()
                || state.active_job.payment_required_invoice_requested
                || state.active_job.payment_required_feedback_in_flight)
                && active_job_payment_evidence_refresh_due(&mut state.active_job, now)
            {
                queue_spark_command(state, SparkWalletCommand::Refresh);
                tracing::info!(
                    target: "autopilot_desktop::provider",
                    "Provider queued wallet refresh while awaiting payment evidence request_id={} interval_seconds={}",
                    request_id,
                    ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL.as_secs()
                );
            }
            note_active_job_waiting_for_payment_evidence(
                &mut state.active_job,
                &mut state.provider_runtime,
                demand_source,
                request_id.as_str(),
            );
        }
        return false;
    };

    if let Some(active_job) = state.active_job.job.as_mut() {
        active_job.payment_id = Some(wallet_pointer.clone());
    }
    state.active_job.append_event(format!(
        "wallet-authoritative payment pointer {} observed for delivered {} job",
        wallet_pointer,
        demand_source.label()
    ));

    if demand_source == crate::app_state::JobDemandSource::StarterDemand
        && let Err(error) = complete_hosted_starter_offer_if_configured(
            state,
            request_id.as_str(),
            wallet_pointer.as_str(),
        )
    {
        let message = format!(
            "wallet pointer {} found but hosted starter completion failed: {}",
            wallet_pointer, error
        );
        state.active_job.last_error = Some(message.clone());
        state.active_job.load_state = crate::app_state::PaneLoadState::Error;
        state.provider_runtime.last_result = Some(message);
        state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Payment);
        return false;
    }

    match super::reducers::transition_active_job_to_paid(
        state,
        "active_job.wallet_paid_transition",
        now,
    ) {
        Ok(crate::app_state::JobLifecycleStage::Paid) => {
            state.provider_runtime.last_result = Some(format!(
                "{} job paid with wallet pointer {}",
                demand_source.label(),
                wallet_pointer
            ));
            if state.provider_runtime.last_authoritative_error_class
                == Some(EarnFailureClass::Payment)
            {
                state.provider_runtime.last_authoritative_error_class = None;
            }
            true
        }
        Ok(_) => false,
        Err(error) => {
            let message = format!(
                "wallet pointer {} found but paid transition failed: {}",
                wallet_pointer, error
            );
            state.active_job.last_error = Some(message.clone());
            state.active_job.load_state = crate::app_state::PaneLoadState::Error;
            state.provider_runtime.last_result = Some(message);
            state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Payment);
            false
        }
    }
}

fn complete_hosted_starter_offer_if_configured(
    state: &mut crate::app_state::RenderState,
    request_id: &str,
    payment_pointer: &str,
) -> Result<(), String> {
    let Some(control_base_url) = state.hosted_control_base_url.clone() else {
        return Ok(());
    };
    let Some(bearer_auth) = state.hosted_control_bearer_token.clone() else {
        return Ok(());
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return Err(format!(
                "starter demand completion client initialization failed: {error}"
            ));
        }
    };
    match crate::starter_demand_client::complete_starter_demand_offer_blocking(
        &client,
        control_base_url.as_str(),
        bearer_auth.as_str(),
        request_id,
        payment_pointer,
    ) {
        Ok(response) => {
            state.starter_jobs.mark_completed(
                response.request_id.as_str(),
                response.payment_pointer.as_str(),
            );
            state.starter_jobs.budget_cap_sats = response.budget_cap_sats;
            state.starter_jobs.budget_allocated_sats = response.budget_allocated_sats;
            state.starter_jobs.next_hosted_sync_due_at = Some(std::time::Instant::now());
            Ok(())
        }
        Err(error) => Err(format!(
            "hosted starter completion reconciliation failed: {error}"
        )),
    }
}

fn hosted_offer_status_to_starter_job_status(status: &str) -> crate::app_state::StarterJobStatus {
    if status.eq_ignore_ascii_case("running") {
        crate::app_state::StarterJobStatus::Running
    } else if status.eq_ignore_ascii_case("completed") {
        crate::app_state::StarterJobStatus::Completed
    } else {
        crate::app_state::StarterJobStatus::Queued
    }
}

fn fail_hosted_starter_active_job_for_lease_loss(
    state: &mut crate::app_state::RenderState,
    source: &str,
) -> Result<(), String> {
    let Some(job) = state.active_job.job.as_ref().cloned() else {
        return Ok(());
    };
    state
        .active_job
        .mark_failed("hosted starter lease lost", "Hosted starter lease lost")?;
    state.active_job.result_publish_in_flight = false;
    state.active_job.pending_result_publish_event_id = None;
    state.active_job.execution_turn_completed = false;
    state.active_job.execution_thread_start_command_seq = None;
    state.active_job.execution_turn_start_command_seq = None;
    state.active_job.execution_turn_interrupt_command_seq = None;
    state
        .job_history
        .record_from_active_job(&job, crate::app_state::JobHistoryStatus::Failed);
    state.earn_job_lifecycle_projection.record_active_job_stage(
        &job,
        crate::app_state::JobLifecycleStage::Failed,
        current_epoch_seconds(),
        source,
    );
    state.earn_kernel_receipts.record_active_job_stage(
        &job,
        crate::app_state::JobLifecycleStage::Failed,
        current_epoch_seconds(),
        source,
    );
    Ok(())
}

fn resolve_wallet_settlement_pointer_for_open_network_job(
    job: &crate::app_state::ActiveJobRecord,
    settlement_invoice_created_at_epoch_seconds: Option<u64>,
    history_rows: &[crate::app_state::JobHistoryReceiptRow],
    recent_payments: &[openagents_spark::PaymentSummary],
) -> Option<String> {
    let _ = settlement_invoice_created_at_epoch_seconds;
    let used_pointers = history_rows
        .iter()
        .map(|row| row.payment_pointer.clone())
        .collect::<std::collections::HashSet<_>>();
    let expected_invoice = job
        .settlement_bolt11
        .as_deref()
        .and_then(normalize_lightning_invoice_ref);
    let expected_payment_hash = job
        .settlement_payment_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            job.settlement_bolt11
                .as_deref()
                .and_then(decode_lightning_invoice_payment_hash)
        });

    let candidate_payments = recent_payments.iter().filter(|payment| {
        payment.direction.eq_ignore_ascii_case("receive")
            && is_settled_wallet_payment_status(payment.status.as_str())
            && !payment.id.trim().is_empty()
            && !used_pointers.contains(payment.id.as_str())
            && !is_synthetic_local_payment_pointer(payment.id.as_str())
    });

    let exact_identity_match = candidate_payments
        .clone()
        .filter(|payment| {
            expected_invoice.as_deref().is_some_and(|expected_invoice| {
                payment
                    .invoice
                    .as_deref()
                    .and_then(normalize_lightning_invoice_ref)
                    .is_some_and(|candidate_invoice| candidate_invoice == expected_invoice)
            }) || expected_payment_hash
                .as_deref()
                .is_some_and(|expected_hash| {
                    payment
                        .payment_hash
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_ascii_lowercase)
                        .is_some_and(|candidate_hash| candidate_hash == expected_hash)
                })
        })
        .max_by(|left, right| left.timestamp.cmp(&right.timestamp))
        .map(|payment| payment.id.clone());
    if exact_identity_match.is_some()
        || expected_invoice.is_some()
        || expected_payment_hash.is_some()
    {
        return exact_identity_match;
    }

    // Open-network seller settlement must bind to the actual payout invoice or payment hash.
    // Falling back to amount/timestamp heuristics can incorrectly reuse an old receive and mark
    // a new delivered job as paid even when wallet balance did not increase.
    let _ = candidate_payments;
    None
}

pub(super) fn run_activity_feed_action(
    state: &mut crate::app_state::RenderState,
    action: ActivityFeedPaneAction,
) -> bool {
    match action {
        ActivityFeedPaneAction::Refresh => {
            let _ = state.activity_feed.reload_projection();
            true
        }
        ActivityFeedPaneAction::PreviousPage => {
            state.activity_feed.previous_page();
            true
        }
        ActivityFeedPaneAction::NextPage => {
            state.activity_feed.next_page();
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
                        state.sync_chat_identities();
                        let _ = state.sync_provider_nip90_lane_identity();
                        crate::render::apply_spacetime_sync_bootstrap(state);
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
                        Ok(url) => {
                            let _ = state.sync_provider_nip90_lane_relays();
                            Ok(format!("Relay reconnect attempted for {url}"))
                        }
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
                        .set_value(state.settings.document.primary_relay_url.clone());
                    state.relay_connections.replace_configured_relays(
                        state.settings.document.configured_relay_urls().as_slice(),
                    );
                    let _ = state.sync_provider_nip90_lane_relays();
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
                        let worker_id = state.sync_lifecycle_worker_id.clone();
                        let _ = state.sync_lifecycle.mark_disconnect(
                            worker_id.as_str(),
                            crate::sync_lifecycle::RuntimeSyncDisconnectReason::Network,
                            Some(
                                "settings updated connectivity; sync reconnect required"
                                    .to_string(),
                            ),
                        );
                        state.sync_lifecycle_snapshot =
                            state.sync_lifecycle.snapshot(worker_id.as_str());
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
                        .set_value(state.settings.document.primary_relay_url.clone());
                    state.relay_connections.replace_configured_relays(
                        state.settings.document.configured_relay_urls().as_slice(),
                    );
                    let _ = state.sync_provider_nip90_lane_relays();
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

    let succeeded_jobs = state
        .job_history
        .rows
        .iter()
        .filter(|row| row.status == crate::app_state::JobHistoryStatus::Succeeded)
        .count();
    let reconciled_jobs = state
        .job_history
        .wallet_reconciled_payout_rows(&state.spark_wallet)
        .len();
    let failure = classify_provider_failure(
        state.provider_nip90_lane.last_error.as_deref(),
        state.provider_runtime.degraded_reason_code.as_deref(),
        state.provider_runtime.last_error_detail.as_deref(),
        state.active_job.last_error.as_deref(),
        state.spark_wallet.last_error.as_deref(),
        state.spark_wallet.balance.is_some() && state.spark_wallet.last_error.is_none(),
        succeeded_jobs,
        reconciled_jobs,
    );
    if let Some((class, detail)) = failure {
        state.provider_runtime.last_authoritative_error_class = Some(class);
        state.provider_runtime.last_error_detail = Some(detail);
    } else {
        state.provider_runtime.last_authoritative_error_class = None;
        if state
            .provider_runtime
            .last_error_detail
            .as_deref()
            .is_some_and(is_taxonomy_failure_detail)
        {
            state.provider_runtime.last_error_detail = None;
        }
    }

    refresh_loop_integrity_slo_alerts(state);
}

fn classify_provider_failure(
    relay_lane_error: Option<&str>,
    degraded_reason_code: Option<&str>,
    runtime_error_detail: Option<&str>,
    active_job_error: Option<&str>,
    wallet_error: Option<&str>,
    wallet_ready: bool,
    succeeded_jobs: usize,
    reconciled_jobs: usize,
) -> Option<(crate::app_state::EarnFailureClass, String)> {
    if let Some(error) = relay_lane_error {
        return Some((
            crate::app_state::EarnFailureClass::Relay,
            taxonomy_failure_detail(crate::app_state::EarnFailureClass::Relay, error),
        ));
    }
    if degraded_reason_code.is_some_and(|code| code.starts_with("NIP90_")) {
        return Some((
            crate::app_state::EarnFailureClass::Relay,
            taxonomy_failure_detail(
                crate::app_state::EarnFailureClass::Relay,
                runtime_error_detail.unwrap_or("relay lane degraded"),
            ),
        ));
    }
    if let Some(error) = active_job_error {
        return Some((
            crate::app_state::EarnFailureClass::Execution,
            taxonomy_failure_detail(crate::app_state::EarnFailureClass::Execution, error),
        ));
    }
    if degraded_reason_code.is_some_and(|code| code.starts_with("SA_")) {
        return Some((
            crate::app_state::EarnFailureClass::Execution,
            taxonomy_failure_detail(
                crate::app_state::EarnFailureClass::Execution,
                runtime_error_detail.unwrap_or("execution lane degraded"),
            ),
        ));
    }
    if let Some(error) = wallet_error {
        return Some((
            crate::app_state::EarnFailureClass::Payment,
            taxonomy_failure_detail(crate::app_state::EarnFailureClass::Payment, error),
        ));
    }
    if wallet_ready && succeeded_jobs > reconciled_jobs {
        let missing = succeeded_jobs.saturating_sub(reconciled_jobs);
        return Some((
            crate::app_state::EarnFailureClass::Reconciliation,
            taxonomy_failure_detail(
                crate::app_state::EarnFailureClass::Reconciliation,
                format!("{missing} succeeded job(s) missing wallet-confirmed payout evidence"),
            ),
        ));
    }

    None
}

fn taxonomy_failure_detail(
    class: crate::app_state::EarnFailureClass,
    detail: impl AsRef<str>,
) -> String {
    let detail = detail.as_ref().trim();
    if detail.is_empty() {
        class.label().to_string()
    } else if detail
        .to_ascii_lowercase()
        .starts_with(&format!("{}:", class.label()))
    {
        detail.to_string()
    } else {
        format!("{}: {}", class.label(), detail)
    }
}

fn is_taxonomy_failure_detail(detail: &str) -> bool {
    [
        crate::app_state::EarnFailureClass::Relay,
        crate::app_state::EarnFailureClass::Execution,
        crate::app_state::EarnFailureClass::Payment,
        crate::app_state::EarnFailureClass::Reconciliation,
    ]
    .into_iter()
    .any(|class| detail.to_ascii_lowercase().starts_with(class.label()))
}

const FIRST_JOB_LATENCY_SLO_SECONDS: u64 = 300;
const COMPLETION_RATIO_SLO_BPS: u16 = 7_500;
const PAYOUT_SUCCESS_SLO_BPS: u16 = 9_000;
const WALLET_CONFIRM_LATENCY_SLO_SECONDS: u64 = 300;

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoopIntegrityAlertSpec {
    alert_id: &'static str,
    domain: crate::app_state::AlertDomain,
    severity: crate::app_state::AlertSeverity,
    active: bool,
    summary: String,
    remediation: String,
}

fn loop_integrity_alert_specs(
    first_job_latency_seconds: Option<u64>,
    completion_ratio_bps: Option<u16>,
    payout_success_ratio_bps: Option<u16>,
    avg_wallet_confirmation_latency_seconds: Option<u64>,
) -> Vec<LoopIntegrityAlertSpec> {
    let first_job_active =
        first_job_latency_seconds.is_some_and(|latency| latency > FIRST_JOB_LATENCY_SLO_SECONDS);
    let completion_active =
        completion_ratio_bps.is_some_and(|ratio| ratio < COMPLETION_RATIO_SLO_BPS);
    let payout_active =
        payout_success_ratio_bps.is_some_and(|ratio| ratio < PAYOUT_SUCCESS_SLO_BPS);
    let wallet_latency_active = avg_wallet_confirmation_latency_seconds
        .is_some_and(|latency| latency > WALLET_CONFIRM_LATENCY_SLO_SECONDS);

    vec![
        LoopIntegrityAlertSpec {
            alert_id: "alert:slo:first-job-latency",
            domain: crate::app_state::AlertDomain::ProviderRuntime,
            severity: crate::app_state::AlertSeverity::Warning,
            active: first_job_active,
            summary: first_job_latency_seconds.map_or_else(
                || "SLO first-job latency unavailable".to_string(),
                |latency| {
                    format!(
                        "SLO first-job latency {}s (target <= {}s)",
                        latency, FIRST_JOB_LATENCY_SLO_SECONDS
                    )
                },
            ),
            remediation:
                "Check relay connectivity and starter-demand health to reduce time-to-first-job."
                    .to_string(),
        },
        LoopIntegrityAlertSpec {
            alert_id: "alert:slo:completion-ratio",
            domain: crate::app_state::AlertDomain::ProviderRuntime,
            severity: crate::app_state::AlertSeverity::Warning,
            active: completion_active,
            summary: completion_ratio_bps.map_or_else(
                || "SLO completion ratio unavailable".to_string(),
                |ratio| {
                    format!(
                        "SLO completion ratio {:.2}% (target >= {:.2}%)",
                        ratio as f64 / 100.0,
                        COMPLETION_RATIO_SLO_BPS as f64 / 100.0
                    )
                },
            ),
            remediation:
                "Inspect execution failures in Active Job and resolve blocking runtime dependencies."
                    .to_string(),
        },
        LoopIntegrityAlertSpec {
            alert_id: "alert:slo:payout-success-ratio",
            domain: crate::app_state::AlertDomain::Wallet,
            severity: crate::app_state::AlertSeverity::Critical,
            active: payout_active,
            summary: payout_success_ratio_bps.map_or_else(
                || "SLO payout success ratio unavailable".to_string(),
                |ratio| {
                    format!(
                        "SLO payout success ratio {:.2}% (target >= {:.2}%)",
                        ratio as f64 / 100.0,
                        PAYOUT_SUCCESS_SLO_BPS as f64 / 100.0
                    )
                },
            ),
            remediation: "Audit wallet reconciliation mismatches and verify payout pointers against settled receive payments.".to_string(),
        },
        LoopIntegrityAlertSpec {
            alert_id: "alert:slo:wallet-confirm-latency",
            domain: crate::app_state::AlertDomain::Wallet,
            severity: crate::app_state::AlertSeverity::Warning,
            active: wallet_latency_active,
            summary: avg_wallet_confirmation_latency_seconds.map_or_else(
                || "SLO wallet confirmation latency unavailable".to_string(),
                |latency| {
                    format!(
                        "SLO wallet confirm latency {}s (target <= {}s)",
                        latency, WALLET_CONFIRM_LATENCY_SLO_SECONDS
                    )
                },
            ),
            remediation:
                "Investigate payment settlement delays and wallet ingest lag before accepting more demand."
                    .to_string(),
        },
    ]
}

fn refresh_loop_integrity_slo_alerts(state: &mut crate::app_state::RenderState) {
    let specs = loop_integrity_alert_specs(
        state.earnings_scoreboard.first_job_latency_seconds,
        state.earnings_scoreboard.completion_ratio_bps,
        state.earnings_scoreboard.payout_success_ratio_bps,
        state
            .earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds,
    );
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());

    let mut active_count = 0usize;
    for spec in specs {
        if let Some(existing) = state
            .alerts_recovery
            .alerts
            .iter_mut()
            .find(|alert| alert.alert_id == spec.alert_id)
        {
            if spec.active {
                active_count = active_count.saturating_add(1);
                existing.domain = spec.domain;
                existing.severity = spec.severity;
                existing.lifecycle = crate::app_state::AlertLifecycle::Active;
                existing.summary = spec.summary;
                existing.remediation = spec.remediation;
                existing.last_transition_epoch_seconds = now_epoch_seconds;
            } else if existing.lifecycle != crate::app_state::AlertLifecycle::Resolved {
                existing.lifecycle = crate::app_state::AlertLifecycle::Resolved;
                existing.summary = format!("{} (recovered)", spec.summary);
                existing.last_transition_epoch_seconds = now_epoch_seconds;
            }
            continue;
        }

        if spec.active {
            active_count = active_count.saturating_add(1);
            state
                .alerts_recovery
                .alerts
                .push(crate::app_state::RecoveryAlertRow {
                    alert_id: spec.alert_id.to_string(),
                    domain: spec.domain,
                    severity: spec.severity,
                    lifecycle: crate::app_state::AlertLifecycle::Active,
                    summary: spec.summary,
                    remediation: spec.remediation,
                    last_transition_epoch_seconds: now_epoch_seconds,
                });
        }
    }

    if active_count > 0 {
        state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
        state.alerts_recovery.last_error = None;
        state.alerts_recovery.last_action = Some(format!(
            "Loop integrity SLO alerts active: {}",
            active_count
        ));
    }
}

pub(super) fn refresh_network_aggregate_counters(
    state: &mut crate::app_state::RenderState,
    now: std::time::Instant,
) {
    state.network_aggregate_counters.refresh_from_sources(
        now,
        &state.spacetime_presence_snapshot,
        &state.job_history,
        &state.spark_wallet,
    );

    if !crate::kernel_control::should_compute_local_snapshots(state) {
        return;
    }

    let now_epoch_ms = current_epoch_millis().min(i64::MAX as u64) as i64;
    let computed_snapshot = {
        let receipts = state.earn_kernel_receipts.receipts.as_slice();
        state
            .economy_snapshot
            .compute_minute_snapshot(now_epoch_ms, receipts)
    };
    if let Some(compute_result) = computed_snapshot {
        state.earn_kernel_receipts.record_economy_snapshot_receipt(
            compute_result.snapshot.snapshot_id.as_str(),
            compute_result.snapshot.as_of_ms,
            compute_result.snapshot.snapshot_hash.as_str(),
            compute_result.snapshot.sv,
            compute_result.snapshot.sv_effective,
            compute_result.snapshot.rho,
            compute_result.snapshot.rho_effective,
            compute_result.snapshot.n,
            compute_result.snapshot.nv,
            compute_result.snapshot.delta_m_hat,
            compute_result.snapshot.xa_hat,
            compute_result.snapshot.correlated_verification_share,
            match compute_result
                .snapshot
                .liability_premiums_collected_24h
                .amount
            {
                crate::economy_kernel_receipts::MoneyAmount::AmountSats(value) => value,
                crate::economy_kernel_receipts::MoneyAmount::AmountMsats(value) => value / 1_000,
            },
            match compute_result.snapshot.claims_paid_24h.amount {
                crate::economy_kernel_receipts::MoneyAmount::AmountSats(value) => value,
                crate::economy_kernel_receipts::MoneyAmount::AmountMsats(value) => value / 1_000,
            },
            match compute_result.snapshot.bonded_exposure_24h.amount {
                crate::economy_kernel_receipts::MoneyAmount::AmountSats(value) => value,
                crate::economy_kernel_receipts::MoneyAmount::AmountMsats(value) => value / 1_000,
            },
            match compute_result.snapshot.capital_reserves_24h.amount {
                crate::economy_kernel_receipts::MoneyAmount::AmountSats(value) => value,
                crate::economy_kernel_receipts::MoneyAmount::AmountMsats(value) => value / 1_000,
            },
            compute_result.snapshot.loss_ratio,
            compute_result.snapshot.capital_coverage_ratio,
            compute_result.snapshot.drift_alerts_24h,
            compute_result.snapshot.drift_signals,
            compute_result.snapshot.top_drift_signals,
            compute_result.snapshot.rollback_attempts_24h,
            compute_result.snapshot.rollback_successes_24h,
            compute_result.snapshot.rollback_success_rate,
            compute_result
                .snapshot
                .top_rollback_reason_codes
                .into_iter()
                .map(|row| (row.reason_code, row.count_24h))
                .collect(),
            compute_result.snapshot.audit_package_public_digest,
            compute_result.snapshot.audit_package_restricted_digest,
            compute_result.input_evidence,
            "economy.snapshot.minute",
        );
    }
}

pub(super) fn refresh_sync_health(state: &mut crate::app_state::RenderState) {
    let now = std::time::Instant::now();
    let worker_id = state.sync_lifecycle_worker_id.clone();

    state.sync_lifecycle.mark_replay_progress(
        worker_id.as_str(),
        state.sync_health.last_applied_event_seq,
        Some(state.sync_health.last_applied_event_seq),
    );

    if let Some(error) = state.sync_bootstrap_error.as_deref() {
        let reason = crate::sync_lifecycle::classify_disconnect_reason(error);
        let already_marked = state
            .sync_lifecycle
            .snapshot(worker_id.as_str())
            .is_some_and(|snapshot| {
                snapshot.state == crate::sync_lifecycle::RuntimeSyncConnectionState::Backoff
                    && snapshot.last_disconnect_reason == Some(reason)
                    && snapshot.last_error.as_deref() == Some(error)
            });
        if !already_marked {
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                reason,
                Some(error.to_string()),
            );
        }
    } else if state
        .sync_bootstrap_note
        .as_deref()
        .is_some_and(|note| note.contains("disabled"))
    {
        state.sync_lifecycle.mark_idle(worker_id.as_str());
    } else {
        let current = state.sync_lifecycle.snapshot(worker_id.as_str());
        if !current.as_ref().is_some_and(|snapshot| {
            matches!(
                snapshot.state,
                crate::sync_lifecycle::RuntimeSyncConnectionState::Live
                    | crate::sync_lifecycle::RuntimeSyncConnectionState::Backoff
            )
        }) {
            let refresh_after = current
                .as_ref()
                .and_then(|snapshot| snapshot.token_refresh_after_in_seconds);
            state.sync_lifecycle.mark_connecting(worker_id.as_str());
            state
                .sync_lifecycle
                .mark_live(worker_id.as_str(), refresh_after);
        }
    }

    state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
    state
        .sync_health
        .refresh_from_lifecycle(now, state.sync_lifecycle_snapshot.as_ref());

    let stale_backoff_already_marked =
        state
            .sync_lifecycle_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.state == crate::sync_lifecycle::RuntimeSyncConnectionState::Backoff
                    && snapshot.last_disconnect_reason
                        == Some(crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor)
            });
    if state.sync_health.cursor_is_stale() && !stale_backoff_already_marked {
        let _ = state.sync_lifecycle.mark_disconnect(
            worker_id.as_str(),
            crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor,
            Some("sync cursor stale; replay rebootstrap required".to_string()),
        );
        state.sync_lifecycle.mark_replay_bootstrap(
            worker_id.as_str(),
            state.sync_health.cursor_position,
            Some(state.sync_health.cursor_position),
        );
        state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
        state
            .sync_health
            .refresh_from_lifecycle(now, state.sync_lifecycle_snapshot.as_ref());
        state.sync_health.last_action =
            Some("Spacetime cursor stale; click Rebootstrap sync".to_string());
    }

    if let Some(error) = state.sync_bootstrap_error.as_deref() {
        state.sync_health.last_action = Some("Spacetime bootstrap failed".to_string());
        state.sync_health.last_error = Some(error.to_string());
    } else if let Some(note) = state.sync_bootstrap_note.as_deref() {
        if state.sync_health.last_action.is_none() {
            state.sync_health.last_action = Some(note.to_string());
        }
    }
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
                Ok(()) => "Copied Lightning invoice to clipboard".to_string(),
                Err(error) => format!("Failed to copy Lightning invoice: {error}"),
            },
            _ => "No Lightning invoice generated yet. Create one first.".to_string(),
        };

        if notice.starts_with("Failed") || notice.starts_with("No Lightning invoice generated") {
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
        SparkPaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateBolt11Invoice {
            amount_sats: parse_positive_amount_str(invoice_amount, "Invoice amount")?,
            description: Some("OpenAgents Lightning receive".to_string()),
            expiry_seconds: Some(3600),
        }),
        SparkPaneAction::SendPayment => {
            let request = validate_lightning_payment_request(send_request)?;
            let amount = resolve_lightning_send_amount(send_amount, &request)?;

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
            let amount = resolve_lightning_send_amount(amount_sats, &request)?;

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
        CreateInvoicePaneAction::CreateInvoice => Ok(SparkWalletCommand::CreateBolt11Invoice {
            amount_sats: parse_positive_amount_str(amount_sats, "Invoice amount")?,
            description: normalize_optional_text(description),
            expiry_seconds: parse_optional_positive_u32_str(expiry_seconds, "Expiry seconds")?,
        }),
        CreateInvoicePaneAction::CopyInvoice => {
            Err("Copy Lightning invoice action is handled directly in UI".to_string())
        }
    }
}

fn resolve_lightning_send_amount(
    amount_sats: &str,
    payment_request: &str,
) -> Result<Option<u64>, String> {
    if amount_sats.trim().is_empty() {
        if lightning_invoice_requires_amount(payment_request) {
            Err("Send amount is required for zero-amount invoice".to_string())
        } else {
            Ok(None)
        }
    } else {
        Ok(Some(parse_positive_amount_str(amount_sats, "Send amount")?))
    }
}

fn lightning_invoice_requires_amount(payment_request: &str) -> bool {
    let normalized = payment_request.trim().to_ascii_lowercase();
    let normalized = normalized
        .strip_prefix("lightning://")
        .or_else(|| normalized.strip_prefix("lightning:"))
        .unwrap_or(normalized.as_str());
    let Some(separator_index) = normalized.rfind('1') else {
        return false;
    };
    let hrp = &normalized[..separator_index];

    ["lnbcrt", "lnbc", "lntb", "lntbs", "lnsb"]
        .iter()
        .find_map(|prefix| hrp.strip_prefix(prefix))
        .is_some_and(str::is_empty)
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

pub(super) fn parse_optional_positive_u32_str(
    raw: &str,
    label: &str,
) -> Result<Option<u32>, String> {
    let Some(value) = parse_optional_positive_amount_str(raw, label)? else {
        return Ok(None);
    };
    u32::try_from(value)
        .map(Some)
        .map_err(|_| format!("{label} must fit within a 32-bit unsigned integer"))
}

pub(super) fn queue_spark_command(
    state: &mut crate::app_state::RenderState,
    command: SparkWalletCommand,
) {
    state.spark_wallet.last_error = None;
    if let SparkWalletCommand::SendPayment {
        payment_request,
        amount_sats,
    } = &command
    {
        let caller_identity = state
            .nostr_identity
            .as_ref()
            .map(|identity| identity.npub.as_str())
            .unwrap_or("autopilot-desktop");
        let now_epoch_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as i64);
        if let Err(error) = state
            .earn_kernel_receipts
            .record_wallet_withdraw_send_attempt(
                caller_identity,
                payment_request.as_str(),
                *amount_sats,
                now_epoch_ms,
                "spark.send_payment",
            )
        {
            state.spark_wallet.last_error = Some(error);
            return;
        }
    }
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

pub(super) fn parse_non_negative_amount_str(raw: &str, label: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }

    trimmed
        .parse::<u64>()
        .map_err(|error| format!("{label} must be a valid integer: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL, ChatAppsComposerIntent,
        ChatGitComposerIntent, ChatMcpComposerIntent, ChatRemoteComposerIntent,
        ChatRequestComposerIntent, ChatSkillsComposerIntent, ChatSpacetimeComposerIntent,
        ChatTerminalComposerIntent, ChatWalletComposerIntent, ChatWalletMessagePayload,
        DirectMessageComposerIntent, MISSION_CONTROL_BUY_MODE_PROMPT, ManagedChatComposerIntent,
        active_job_payment_evidence_refresh_due, build_direct_message_outbound_message,
        build_managed_chat_join_request_event, build_managed_chat_leave_request_event,
        build_managed_chat_moderation_event, build_managed_chat_outbound_message,
        build_managed_chat_reaction_event, build_mission_control_buy_mode_request_event,
        build_nip90_request_event_for_network_submission, chat_wallet_payment_status_summary,
        classify_provider_failure, default_pr_base_branch, extract_chat_wallet_payload,
        extract_target_provider_pubkeys, git_common_worktree_root, git_current_branch,
        git_local_branch_exists, github_compare_url, is_taxonomy_failure_detail,
        loop_integrity_alert_specs, nip90_request_kind_for_request_type,
        note_active_job_waiting_for_payment_evidence, parse_chat_apps_intent,
        parse_chat_git_intent, parse_chat_mcp_intent, parse_chat_remote_intent,
        parse_chat_request_intent, parse_chat_skills_intent, parse_chat_spacetime_intent,
        parse_chat_terminal_intent, parse_chat_wallet_intent, parse_direct_message_creation_intent,
        parse_direct_message_room_intent, parse_managed_chat_composer_intent,
        parse_managed_chat_mention_prefix, resolve_apple_fm_workbench_session_id,
        resolve_wallet_blink_env_from_secure_values,
        resolve_wallet_settlement_pointer_for_open_network_job,
        stable_sats_period_convert_totals_from_receipts,
        stable_sats_real_round_phase_from_operation_count, taxonomy_failure_detail,
    };

    fn fixture_wallet() -> crate::app_state::StableSatsAgentWalletState {
        crate::app_state::StableSatsAgentWalletState {
            agent_name: "sa-wallet-1".to_string(),
            owner_kind: crate::app_state::StableSatsWalletOwnerKind::SovereignAgent,
            owner_id: "sa:wallet-1".to_string(),
            credential_key_name: "BLINK_API_KEY_SA_1".to_string(),
            credential_url_name: Some("BLINK_API_URL_SA_1".to_string()),
            btc_balance_sats: 0,
            usd_balance_cents: 0,
            active_wallet: crate::app_state::StableSatsWalletMode::Btc,
            switch_count: 0,
            last_switch_summary: "none".to_string(),
        }
    }

    fn fixture_entries() -> Vec<crate::credentials::CredentialRecord> {
        vec![
            crate::credentials::CredentialRecord {
                name: "BLINK_API_KEY_SA_1".to_string(),
                enabled: true,
                secret: true,
                template: true,
                scopes: crate::credentials::CREDENTIAL_SCOPE_ALL,
                has_value: true,
            },
            crate::credentials::CredentialRecord {
                name: "BLINK_API_URL_SA_1".to_string(),
                enabled: true,
                secret: false,
                template: true,
                scopes: crate::credentials::CREDENTIAL_SCOPE_ALL,
                has_value: true,
            },
            crate::credentials::CredentialRecord {
                name: "BLINK_API_KEY".to_string(),
                enabled: true,
                secret: true,
                template: true,
                scopes: crate::credentials::CREDENTIAL_SCOPE_ALL,
                has_value: true,
            },
        ]
    }

    fn git_ok(cwd: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .status()
            .expect("git status");
        assert!(
            status.success(),
            "git {:?} failed in {}",
            args,
            cwd.display()
        );
    }

    fn init_temp_git_repo(branch: &str) -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        git_ok(temp.path(), &["init"]);
        git_ok(temp.path(), &["config", "user.email", "agent@example.com"]);
        git_ok(temp.path(), &["config", "user.name", "OpenAgents Test"]);
        std::fs::write(temp.path().join("README.md"), "# test\n").expect("write readme");
        git_ok(temp.path(), &["add", "README.md"]);
        git_ok(temp.path(), &["commit", "-m", "initial"]);
        git_ok(temp.path(), &["branch", "-M", branch]);
        temp
    }

    fn fixture_identity() -> nostr::NostrIdentity {
        let private_key = [0x22u8; 32];
        nostr::NostrIdentity {
            identity_path: std::path::PathBuf::from("/tmp/test-identity.mnemonic"),
            mnemonic: "test mnemonic".to_string(),
            npub: "npub-test".to_string(),
            nsec: "nsec-test".to_string(),
            public_key_hex: nostr::get_public_key_hex(&private_key).expect("fixture pubkey"),
            private_key_hex: hex::encode(private_key),
        }
    }

    fn fixture_open_network_delivered_job(
        quoted_price_sats: u64,
    ) -> crate::app_state::ActiveJobRecord {
        crate::app_state::ActiveJobRecord {
            job_id: "job-open-network-001".to_string(),
            request_id: "req-open-network-001".to_string(),
            requester: "11".repeat(32),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            demand_risk_class: crate::app_state::JobDemandRiskClass::SpeculativeOpenNetwork,
            demand_risk_disposition: crate::app_state::JobDemandRiskDisposition::ManualReviewOnly,
            demand_risk_note:
                "untargeted open-network demand stays visible but requires manual review"
                    .to_string(),
            request_kind: nostr::nip90::KIND_JOB_TEXT_GENERATION,
            capability: "text.generation".to_string(),
            execution_input: Some("Return the generated text result.".to_string()),
            execution_prompt: Some("Return the generated text result.".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            execution_provenance: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("req-open-network-001".to_string()),
            sa_tick_result_event_id: Some("result-open-network-001".to_string()),
            sa_trajectory_session_id: Some("traj:req-open-network-001".to_string()),
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            compute_product_id: None,
            capacity_lot_id: None,
            capacity_instrument_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            quoted_price_sats,
            ttl_seconds: 75,
            request_created_at_epoch_seconds: Some(1_760_000_000),
            request_expires_at_epoch_seconds: Some(1_760_000_075),
            stage: crate::app_state::JobLifecycleStage::Delivered,
            invoice_id: None,
            settlement_bolt11: None,
            settlement_payment_hash: None,
            payment_id: None,
            failure_reason: None,
            events: vec![],
        }
    }

    fn fixture_history_row(payment_pointer: &str) -> crate::app_state::JobHistoryReceiptRow {
        crate::app_state::JobHistoryReceiptRow {
            job_id: "job-history-001".to_string(),
            status: crate::app_state::JobHistoryStatus::Succeeded,
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1_762_700_000,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats: 10,
            result_hash: "sha256:test".to_string(),
            payment_pointer: payment_pointer.to_string(),
            failure_reason: None,
            execution_provenance: None,
        }
    }

    #[test]
    fn resolves_open_network_wallet_pointer_by_exact_invoice_identity_before_amount_heuristic() {
        let mut job = fixture_open_network_delivered_job(10);
        job.settlement_bolt11 = Some("lnbc20n1targetinvoice".to_string());
        let history_rows = vec![fixture_history_row("wallet-used-001")];
        let payments = vec![
            openagents_spark::PaymentSummary {
                id: "wallet-newer-wrong-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_050,
                invoice: Some("lnbc20n1differentinvoice".to_string()),
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-exact-invoice-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_020,
                invoice: Some("lightning:lnbc20n1targetinvoice".to_string()),
                ..Default::default()
            },
        ];

        let pointer = resolve_wallet_settlement_pointer_for_open_network_job(
            &job,
            Some(1_762_700_010),
            history_rows.as_slice(),
            payments.as_slice(),
        )
        .expect("expected exact invoice match");
        assert_eq!(pointer, "wallet-exact-invoice-001");
    }

    #[test]
    fn resolves_open_network_wallet_pointer_by_exact_payment_hash_when_invoice_missing() {
        let mut job = fixture_open_network_delivered_job(10);
        job.settlement_payment_hash = Some("hash-settlement-001".to_string());
        let payments = vec![
            openagents_spark::PaymentSummary {
                id: "wallet-wrong-hash-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_050,
                payment_hash: Some("hash-other-001".to_string()),
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-hash-match-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_020,
                payment_hash: Some("HASH-SETTLEMENT-001".to_string()),
                ..Default::default()
            },
        ];

        let pointer = resolve_wallet_settlement_pointer_for_open_network_job(
            &job,
            Some(1_762_700_010),
            &[],
            payments.as_slice(),
        )
        .expect("expected payment hash match");
        assert_eq!(pointer, "wallet-hash-match-001");
    }

    #[test]
    fn open_network_wallet_pointer_requires_exact_invoice_or_payment_hash_identity() {
        let job = fixture_open_network_delivered_job(10);
        let history_rows = vec![fixture_history_row("wallet-used-001")];
        let payments = vec![
            openagents_spark::PaymentSummary {
                id: "wallet-send-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_001,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-failed-001".to_string(),
                direction: "receive".to_string(),
                status: "failed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_002,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-wrong-amount-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 9,
                timestamp: 1_762_700_003,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "pending:req-open-network-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_004,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-used-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_005,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-before-invoice-001".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_006,
                ..Default::default()
            },
            openagents_spark::PaymentSummary {
                id: "wallet-pointer-002".to_string(),
                direction: "receive".to_string(),
                status: "completed".to_string(),
                amount_sats: 10,
                timestamp: 1_762_700_012,
                ..Default::default()
            },
        ];

        let pointer = resolve_wallet_settlement_pointer_for_open_network_job(
            &job,
            Some(1_762_700_010),
            history_rows.as_slice(),
            payments.as_slice(),
        );
        assert_eq!(pointer, None);
    }

    #[test]
    fn extracts_optional_target_provider_pubkeys_from_payload_json() {
        let payload = serde_json::json!({
            "target_provider_pubkey": "pubkey-a",
            "target_provider_pubkeys": ["pubkey-b", "pubkey-a", ""]
        })
        .to_string();
        let providers = extract_target_provider_pubkeys(payload.as_str());
        assert_eq!(
            providers,
            vec!["pubkey-a".to_string(), "pubkey-b".to_string()]
        );
    }

    #[test]
    fn builds_signed_nip90_request_event_with_targeting_and_relays() {
        let identity = fixture_identity();
        let event = build_nip90_request_event_for_network_submission(
            Some(&identity),
            "summarize.text",
            "{\"prompt\":\"hello\"}",
            Some("scope-1"),
            Some("ac-env-1"),
            crate::app_state::BuyerResolutionMode::Race,
            10,
            60,
            &["wss://relay.one".to_string(), "wss://relay.two".to_string()],
            &["provider-pubkey-1".to_string()],
        )
        .expect("request event should build");
        assert_eq!(
            event.kind,
            nip90_request_kind_for_request_type("summarize.text")
        );
        let request = nostr::nip90::JobRequest::from_event(&event).expect("request should parse");
        assert_eq!(request.bid, Some(10_000));
        assert_eq!(request.relays.len(), 2);
        assert_eq!(
            request.service_providers,
            vec!["provider-pubkey-1".to_string()]
        );
        assert_eq!(request.inputs.len(), 1);
        assert_eq!(
            request
                .params
                .iter()
                .filter(|p| p.key == "request_type")
                .count(),
            1
        );
        assert!(
            request
                .params
                .iter()
                .any(|p| p.key == "oa_resolution_mode" && p.value == "race")
        );
    }

    #[test]
    fn builds_mission_control_buy_mode_request_with_fixed_5050_contract_and_target_provider() {
        let identity = fixture_identity();
        let event = build_mission_control_buy_mode_request_event(
            Some(&identity),
            &["wss://relay.one".to_string(), "wss://relay.two".to_string()],
            &["provider-pubkey-1".to_string()],
        )
        .expect("buy mode request event should build");
        assert_eq!(
            event.kind,
            crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_KIND
        );

        let request = nostr::nip90::JobRequest::from_event(&event).expect("request should parse");
        assert_eq!(
            request.bid,
            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS * 1000)
        );
        assert_eq!(request.relays.len(), 2);
        assert_eq!(
            request.service_providers,
            vec!["provider-pubkey-1".to_string()]
        );
        assert_eq!(request.inputs.len(), 1);
        assert_eq!(request.inputs[0].data, MISSION_CONTROL_BUY_MODE_PROMPT);
        assert_eq!(request.inputs[0].marker.as_deref(), Some("prompt"));
        assert!(
            request.params.iter().any(|param| {
                param.key == "request_type"
                    && param.value == crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
            }),
            "buy mode request should carry the fixed request type"
        );
        assert!(
            request.params.iter().any(|param| {
                param.key == "timeout_seconds"
                    && param.value
                        == crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS.to_string()
            }),
            "buy mode request should carry the fixed timeout"
        );
    }

    #[test]
    fn resolves_namespaced_wallet_binding_to_runtime_blink_env() {
        let wallet = fixture_wallet();
        let entries = fixture_entries();
        let secure_values = std::collections::BTreeMap::from([
            (
                "BLINK_API_KEY_SA_1".to_string(),
                "blink_namespaced_value".to_string(),
            ),
            (
                "BLINK_API_URL_SA_1".to_string(),
                "https://api.staging.blink.sv/graphql".to_string(),
            ),
            (
                "BLINK_API_KEY".to_string(),
                "blink_global_value_should_not_override".to_string(),
            ),
        ]);

        let resolved = resolve_wallet_blink_env_from_secure_values(
            &wallet,
            entries.as_slice(),
            &secure_values,
        )
        .expect("wallet binding should resolve");
        assert_eq!(
            resolved,
            vec![
                (
                    "BLINK_API_KEY".to_string(),
                    "blink_namespaced_value".to_string()
                ),
                (
                    "BLINK_API_URL".to_string(),
                    "https://api.staging.blink.sv/graphql".to_string()
                )
            ]
        );
    }

    #[test]
    fn wallet_binding_does_not_fallback_to_global_key_when_namespaced_value_missing() {
        let wallet = fixture_wallet();
        let entries = fixture_entries();
        let secure_values = std::collections::BTreeMap::from([(
            "BLINK_API_KEY".to_string(),
            "blink_global_only".to_string(),
        )]);

        let error = resolve_wallet_blink_env_from_secure_values(
            &wallet,
            entries.as_slice(),
            &secure_values,
        )
        .expect_err("namespaced key should be required");
        assert!(error.contains("BLINK_API_KEY_SA_1"));
    }

    #[test]
    fn apple_fm_workbench_session_id_prefers_explicit_input() {
        let resolved = resolve_apple_fm_workbench_session_id("sess-manual", Some("sess-active"));
        assert_eq!(resolved.as_deref(), Some("sess-manual"));
    }

    #[test]
    fn apple_fm_workbench_session_id_falls_back_to_active_session() {
        let resolved = resolve_apple_fm_workbench_session_id("   ", Some("sess-active"));
        assert_eq!(resolved.as_deref(), Some("sess-active"));
    }

    #[test]
    fn real_round_phase_cycles_deterministically_by_operation_count() {
        let phases = (0..8)
            .map(stable_sats_real_round_phase_from_operation_count)
            .collect::<Vec<_>>();
        assert_eq!(phases, vec![0, 1, 0, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn convert_window_totals_include_only_recent_settled_convert_receipts() {
        let now_epoch_seconds = 10_000_u64;
        let cutoff = now_epoch_seconds.saturating_sub(3_600);
        let receipts = vec![
            crate::app_state::StableSatsTreasuryReceipt {
                seq: 1,
                request_id: 11,
                kind: crate::app_state::StableSatsTreasuryOperationKind::Convert,
                payload: serde_json::json!({
                    "status": "SUCCESS",
                    "amount": 1_200,
                    "unit": "sats",
                }),
                occurred_at_epoch_seconds: now_epoch_seconds.saturating_sub(10),
            },
            crate::app_state::StableSatsTreasuryReceipt {
                seq: 2,
                request_id: 12,
                kind: crate::app_state::StableSatsTreasuryOperationKind::Convert,
                payload: serde_json::json!({
                    "status": " settled ",
                    "amount": "345",
                    "unit": "CENTS",
                }),
                occurred_at_epoch_seconds: now_epoch_seconds.saturating_sub(42),
            },
            crate::app_state::StableSatsTreasuryReceipt {
                seq: 3,
                request_id: 13,
                kind: crate::app_state::StableSatsTreasuryOperationKind::Convert,
                payload: serde_json::json!({
                    "status": "FAILED",
                    "amount": 900,
                    "unit": "sats",
                }),
                occurred_at_epoch_seconds: now_epoch_seconds.saturating_sub(18),
            },
            crate::app_state::StableSatsTreasuryReceipt {
                seq: 4,
                request_id: 14,
                kind: crate::app_state::StableSatsTreasuryOperationKind::TransferBtc,
                payload: serde_json::json!({
                    "status": "SUCCESS",
                    "amount": 2_000,
                    "unit": "sats",
                }),
                occurred_at_epoch_seconds: now_epoch_seconds.saturating_sub(22),
            },
            crate::app_state::StableSatsTreasuryReceipt {
                seq: 5,
                request_id: 15,
                kind: crate::app_state::StableSatsTreasuryOperationKind::Convert,
                payload: serde_json::json!({
                    "status": "SUCCESS",
                    "amount": 777,
                    "unit": "sats",
                }),
                occurred_at_epoch_seconds: cutoff.saturating_sub(1),
            },
        ];

        let totals = stable_sats_period_convert_totals_from_receipts(receipts.as_slice(), cutoff);
        assert_eq!(totals, (1_200, 345));
    }

    #[test]
    fn provider_failure_taxonomy_classifies_relay_execution_payment_and_reconciliation() {
        let relay = classify_provider_failure(
            Some("relay timeout"),
            None,
            None,
            Some("active job failed"),
            Some("wallet offline"),
            true,
            3,
            1,
        )
        .expect("relay failure should classify");
        assert_eq!(relay.0, crate::app_state::EarnFailureClass::Relay);
        assert!(relay.1.starts_with("relay:"));

        let execution = classify_provider_failure(
            None,
            Some("SA_COMMAND_REJECTED"),
            Some("sa lane rejected command"),
            Some("active job failed"),
            Some("wallet offline"),
            true,
            3,
            1,
        )
        .expect("execution failure should classify");
        assert_eq!(execution.0, crate::app_state::EarnFailureClass::Execution);
        assert!(execution.1.starts_with("execution:"));

        let payment = classify_provider_failure(
            None,
            None,
            None,
            None,
            Some("wallet backend unavailable"),
            true,
            3,
            1,
        )
        .expect("payment failure should classify");
        assert_eq!(payment.0, crate::app_state::EarnFailureClass::Payment);
        assert!(payment.1.starts_with("payment:"));

        let reconciliation = classify_provider_failure(None, None, None, None, None, true, 4, 2)
            .expect("reconciliation mismatch should classify");
        assert_eq!(
            reconciliation.0,
            crate::app_state::EarnFailureClass::Reconciliation
        );
        assert!(reconciliation.1.starts_with("reconciliation:"));
    }

    #[test]
    fn taxonomy_failure_detail_helpers_prefix_and_detect_labels() {
        let relay = taxonomy_failure_detail(
            crate::app_state::EarnFailureClass::Relay,
            " relay publish failed ",
        );
        assert_eq!(relay, "relay: relay publish failed");
        assert!(is_taxonomy_failure_detail(relay.as_str()));
        assert!(!is_taxonomy_failure_detail("wallet backend unavailable"));
    }

    #[test]
    fn loop_integrity_alert_specs_flags_expected_slo_breaches() {
        let degraded_specs =
            loop_integrity_alert_specs(Some(600), Some(6_000), Some(8_000), Some(420));
        assert_eq!(degraded_specs.len(), 4);
        assert!(degraded_specs.iter().all(|spec| spec.active));

        let healthy_specs =
            loop_integrity_alert_specs(Some(45), Some(9_500), Some(10_000), Some(90));
        assert_eq!(healthy_specs.len(), 4);
        assert!(healthy_specs.iter().all(|spec| !spec.active));
    }

    #[test]
    fn managed_chat_composer_intent_parses_reply_and_reaction_syntax() {
        assert_eq!(
            parse_managed_chat_composer_intent("reply 3 acknowledged").unwrap(),
            ManagedChatComposerIntent::ChannelMessage {
                content: "acknowledged".to_string(),
                reply_reference: Some("3".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("react #2 :wave:").unwrap(),
            ManagedChatComposerIntent::Reaction {
                message_reference: "#2".to_string(),
                reaction: ":wave:".to_string(),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("plain message").unwrap(),
            ManagedChatComposerIntent::ChannelMessage {
                content: "plain message".to_string(),
                reply_reference: None,
            }
        );
        assert!(parse_managed_chat_composer_intent("reply 2").is_err());
    }

    #[test]
    fn managed_chat_composer_intent_parses_admin_join_and_local_mute_commands() {
        assert_eq!(
            parse_managed_chat_composer_intent("delete #4 spam").unwrap(),
            ManagedChatComposerIntent::DeleteMessage {
                message_reference: "#4".to_string(),
                reason: Some("spam".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("remove abcd1234 abuse").unwrap(),
            ManagedChatComposerIntent::RemoveUser {
                member_reference: "abcd1234".to_string(),
                reason: Some("abuse".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("invite launchcode staged rollout").unwrap(),
            ManagedChatComposerIntent::Invite {
                code: "launchcode".to_string(),
                reason: Some("staged rollout".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("join beta42 | let me in").unwrap(),
            ManagedChatComposerIntent::Join {
                invite_code: Some("beta42".to_string()),
                reason: Some("let me in".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("leave stepping out").unwrap(),
            ManagedChatComposerIntent::Leave {
                reason: Some("stepping out".to_string()),
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("mute deadbeef").unwrap(),
            ManagedChatComposerIntent::MuteMember {
                member_reference: "deadbeef".to_string(),
                muted: true,
            }
        );
        assert_eq!(
            parse_managed_chat_composer_intent("unmute deadbeef").unwrap(),
            ManagedChatComposerIntent::MuteMember {
                member_reference: "deadbeef".to_string(),
                muted: false,
            }
        );
        assert!(matches!(
            parse_managed_chat_composer_intent(
                "meta name=Ops | private=true | restricted=false | closed=false"
            )
            .unwrap(),
            ManagedChatComposerIntent::EditMetadata { .. }
        ));
    }

    #[test]
    fn managed_chat_mention_prefix_parser_requires_hex_prefixes() {
        assert_eq!(
            parse_managed_chat_mention_prefix("@abcd1234,"),
            Some("abcd1234".to_string())
        );
        assert_eq!(parse_managed_chat_mention_prefix("@ops"), None);
        assert_eq!(parse_managed_chat_mention_prefix("plain"), None);
    }

    #[test]
    fn managed_chat_event_builders_include_reply_mentions_and_reaction_tags() {
        let channel = crate::app_state::ManagedChatChannelProjection {
            channel_id: "aa".repeat(32),
            group_id: "oa-main".to_string(),
            room_mode: nostr::ManagedRoomMode::ManagedChannel,
            metadata: nostr::ChannelMetadata::new("ops", "", ""),
            hints: nostr::ManagedChannelHints::default(),
            relay_url: Some("wss://relay.openagents.test".to_string()),
            message_ids: Vec::new(),
            root_message_ids: Vec::new(),
            unread_count: 0,
            mention_count: 0,
            latest_message_id: None,
        };
        let target_message = crate::app_state::ManagedChatMessageProjection {
            event_id: "bb".repeat(32),
            group_id: "oa-main".to_string(),
            channel_id: channel.channel_id.clone(),
            author_pubkey: "11".repeat(32),
            content: "root".to_string(),
            created_at: 20,
            reply_to_event_id: None,
            mention_pubkeys: Vec::new(),
            reaction_summaries: Vec::new(),
            reply_child_ids: Vec::new(),
            delivery_state: crate::app_state::ManagedChatDeliveryState::Confirmed,
            delivery_error: None,
            attempt_count: 0,
        };

        let outbound = build_managed_chat_outbound_message(
            &fixture_identity(),
            "oa-main",
            &channel,
            None,
            "ack @2222",
            Some(&target_message),
            vec![nostr::ManagedChatMention::new("22".repeat(32)).unwrap()],
        )
        .unwrap();
        let parsed_message =
            nostr::ManagedChannelMessageEvent::from_event(&outbound.event).unwrap();
        assert_eq!(
            parsed_message
                .reply_to
                .as_ref()
                .map(|reply| reply.event_id.as_str()),
            Some(target_message.event_id.as_str())
        );
        assert_eq!(parsed_message.mentions.len(), 1);
        assert_eq!(parsed_message.mentions[0].pubkey, "22".repeat(32));

        let reaction = build_managed_chat_reaction_event(
            &fixture_identity(),
            "oa-main",
            &channel,
            &target_message,
            "+",
        )
        .unwrap();
        assert_eq!(reaction.kind, 7);
        assert!(reaction.tags.iter().any(|tag| {
            tag.first().map(String::as_str) == Some("e")
                && tag.get(1).map(String::as_str) == Some(target_message.event_id.as_str())
        }));
        assert!(reaction.tags.iter().any(|tag| {
            tag.first().map(String::as_str) == Some("p")
                && tag.get(1).map(String::as_str) == Some(target_message.author_pubkey.as_str())
        }));
    }

    #[test]
    fn managed_chat_event_builders_cover_join_leave_and_moderation_controls() {
        let identity = fixture_identity();

        let join = build_managed_chat_join_request_event(
            &identity,
            "oa-main",
            Some("launchcode"),
            Some("request access"),
        )
        .expect("join request");
        assert_eq!(join.kind, nostr::nip29::KIND_JOIN_REQUEST);
        assert!(
            join.tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("code"))
        );

        let leave =
            build_managed_chat_leave_request_event(&identity, "oa-main", Some("done for today"))
                .expect("leave request");
        assert_eq!(leave.kind, nostr::nip29::KIND_LEAVE_REQUEST);

        let moderation = build_managed_chat_moderation_event(
            &identity,
            "oa-main",
            nostr::ModerationAction::EditMetadata {
                changes: vec![
                    vec!["name".to_string(), "Ops".to_string()],
                    vec!["private".to_string()],
                    vec!["open".to_string()],
                ],
            },
            Some("tighten access"),
        )
        .expect("moderation event");
        assert_eq!(moderation.kind, nostr::nip29::KIND_EDIT_METADATA);
        assert!(
            moderation
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("name"))
        );
        assert!(
            moderation
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("private"))
        );
        assert!(
            moderation
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("open"))
        );
    }

    #[test]
    fn direct_message_composer_parses_dm_room_and_reply_syntax() {
        assert_eq!(
            parse_direct_message_creation_intent(&format!("dm {} hello", "11".repeat(32))).unwrap(),
            Some(DirectMessageComposerIntent::CreateRoom {
                participant_pubkeys: vec!["11".repeat(32)],
                subject: None,
                content: "hello".to_string(),
            })
        );
        assert_eq!(
            parse_direct_message_creation_intent(&format!(
                "room {},{} | Design Review | kickoff",
                "11".repeat(32),
                "22".repeat(32)
            ))
            .unwrap(),
            Some(DirectMessageComposerIntent::CreateRoom {
                participant_pubkeys: vec!["11".repeat(32), "22".repeat(32)],
                subject: Some("Design Review".to_string()),
                content: "kickoff".to_string(),
            })
        );
        assert_eq!(
            parse_direct_message_room_intent("reply #2 acknowledged").unwrap(),
            DirectMessageComposerIntent::RoomMessage {
                content: "acknowledged".to_string(),
                reply_reference: Some("#2".to_string()),
            }
        );
    }

    #[test]
    fn chat_wallet_commands_parse_explicit_message_actions() {
        assert_eq!(
            parse_chat_wallet_intent("wallet pay #2").unwrap(),
            Some(ChatWalletComposerIntent::PayInvoice {
                message_reference: "#2".to_string(),
            })
        );
        assert_eq!(
            parse_chat_wallet_intent("wallet request abcd1234 bug bounty").unwrap(),
            Some(ChatWalletComposerIntent::RequestInvoice {
                message_reference: "abcd1234".to_string(),
                description: Some("bug bounty".to_string()),
            })
        );
        assert_eq!(
            parse_chat_wallet_intent("wallet copy-address #4").unwrap(),
            Some(ChatWalletComposerIntent::CopyAddress {
                message_reference: "#4".to_string(),
            })
        );
        assert_eq!(
            parse_chat_wallet_intent("wallet status abcdef").unwrap(),
            Some(ChatWalletComposerIntent::InspectPaymentStatus {
                message_reference: "abcdef".to_string(),
            })
        );
    }

    #[test]
    fn chat_spacetime_search_command_uses_slash_syntax() {
        assert_eq!(
            parse_chat_spacetime_intent("/search deploy").unwrap(),
            Some(ChatSpacetimeComposerIntent::Search {
                query: "deploy".to_string(),
            })
        );
        assert!(parse_chat_spacetime_intent("/search").is_err());
        assert_eq!(parse_chat_spacetime_intent("deploy"), Ok(None));
    }

    #[test]
    fn chat_git_commands_parse_branch_worktree_and_pr_syntax() {
        assert_eq!(
            parse_chat_git_intent("/git status").unwrap(),
            Some(ChatGitComposerIntent::Status)
        );
        assert_eq!(
            parse_chat_git_intent("/git branch create feature/cx-8").unwrap(),
            Some(ChatGitComposerIntent::BranchCreate {
                branch: "feature/cx-8".to_string(),
            })
        );
        assert_eq!(
            parse_chat_git_intent("/git checkout feature/cx-8").unwrap(),
            Some(ChatGitComposerIntent::Checkout {
                branch: "feature/cx-8".to_string(),
            })
        );
        assert_eq!(
            parse_chat_git_intent("/git worktree add \"../feature tree\" feature/cx-8").unwrap(),
            Some(ChatGitComposerIntent::WorktreeAdd {
                path: "../feature tree".to_string(),
                branch: "feature/cx-8".to_string(),
            })
        );
        assert_eq!(
            parse_chat_git_intent("/pr prep main").unwrap(),
            Some(ChatGitComposerIntent::PrPrep {
                base_branch: Some("main".to_string()),
            })
        );
        assert!(parse_chat_git_intent("/git worktree add ../feature-only").is_err());
    }

    #[test]
    fn chat_terminal_commands_parse_session_and_cleanup_syntax() {
        assert_eq!(
            parse_chat_terminal_intent("/term open").unwrap(),
            Some(ChatTerminalComposerIntent::Open)
        );
        assert_eq!(
            parse_chat_terminal_intent("/term write cargo test -p autopilot-desktop").unwrap(),
            Some(ChatTerminalComposerIntent::Write {
                text: "cargo test -p autopilot-desktop".to_string(),
            })
        );
        assert_eq!(
            parse_chat_terminal_intent("/term resize 140 48").unwrap(),
            Some(ChatTerminalComposerIntent::Resize {
                cols: 140,
                rows: 48
            })
        );
        assert_eq!(
            parse_chat_terminal_intent("/ps").unwrap(),
            Some(ChatTerminalComposerIntent::ListSessions)
        );
        assert_eq!(
            parse_chat_terminal_intent("/clean").unwrap(),
            Some(ChatTerminalComposerIntent::CleanClosed)
        );
        assert!(parse_chat_terminal_intent("/term resize 140").is_err());
    }

    #[test]
    fn chat_skills_commands_parse_local_and_remote_workflows() {
        assert_eq!(
            parse_chat_skills_intent("/skills").unwrap(),
            Some(ChatSkillsComposerIntent::Summary)
        );
        assert_eq!(
            parse_chat_skills_intent("/skills use blink").unwrap(),
            Some(ChatSkillsComposerIntent::Use {
                query: "blink".to_string(),
            })
        );
        assert_eq!(
            parse_chat_skills_intent("/skills disable skills/blink").unwrap(),
            Some(ChatSkillsComposerIntent::SetEnabled {
                query: "skills/blink".to_string(),
                enabled: false,
            })
        );
        assert_eq!(
            parse_chat_skills_intent("/skills remote list personal").unwrap(),
            Some(ChatSkillsComposerIntent::RemoteList {
                scope: codex_client::HazelnutScope::Personal,
            })
        );
        assert_eq!(
            parse_chat_skills_intent("/skills remote export hazelnut-1").unwrap(),
            Some(ChatSkillsComposerIntent::RemoteExport {
                query: "hazelnut-1".to_string(),
            })
        );
        assert!(parse_chat_skills_intent("/skills remote list nope").is_err());
    }

    #[test]
    fn chat_codex_operator_commands_parse_mcp_apps_and_requests() {
        assert_eq!(
            parse_chat_mcp_intent("/mcp login github").unwrap(),
            Some(ChatMcpComposerIntent::Login {
                query: Some("github".to_string()),
            })
        );
        assert_eq!(
            parse_chat_apps_intent("/apps select GitHub").unwrap(),
            Some(ChatAppsComposerIntent::Select {
                query: "GitHub".to_string(),
            })
        );
        assert!(matches!(
            parse_chat_request_intent("/requests").unwrap(),
            Some(ChatRequestComposerIntent::Summary)
        ));
        assert!(matches!(
            parse_chat_request_intent("/approvals session").unwrap(),
            Some(ChatRequestComposerIntent::Approval {
                label: "accept-for-session",
                ..
            })
        ));
        assert!(matches!(
            parse_chat_request_intent("/tool prompt respond").unwrap(),
            Some(ChatRequestComposerIntent::ToolUserInputRespond)
        ));
        assert!(matches!(
            parse_chat_request_intent("/auth respond").unwrap(),
            Some(ChatRequestComposerIntent::AuthRefreshRespond)
        ));
        assert_eq!(
            parse_chat_remote_intent("/remote enable 127.0.0.1:4848").unwrap(),
            Some(ChatRemoteComposerIntent::Enable {
                bind_addr: Some("127.0.0.1:4848".to_string()),
            })
        );
        assert_eq!(
            parse_chat_remote_intent("/remote rotate-token").unwrap(),
            Some(ChatRemoteComposerIntent::RotateToken)
        );
    }

    #[test]
    fn github_compare_url_handles_https_and_ssh_remotes() {
        assert_eq!(
            github_compare_url(
                "git@github.com:OpenAgentsInc/openagents.git",
                "main",
                "feature/cx-8"
            )
            .as_deref(),
            Some(
                "https://github.com/OpenAgentsInc/openagents/compare/main...feature/cx-8?expand=1"
            )
        );
        assert_eq!(
            github_compare_url(
                "https://github.com/OpenAgentsInc/openagents.git",
                "main",
                "feature/cx-8"
            )
            .as_deref(),
            Some(
                "https://github.com/OpenAgentsInc/openagents/compare/main...feature/cx-8?expand=1"
            )
        );
        assert!(
            github_compare_url(
                "https://gitlab.com/OpenAgentsInc/openagents.git",
                "main",
                "x"
            )
            .is_none()
        );
    }

    #[test]
    fn git_helpers_detect_branch_and_worktree_context() {
        let repo = init_temp_git_repo("main");
        assert_eq!(git_current_branch(repo.path()).unwrap(), "main");
        assert_eq!(default_pr_base_branch(repo.path(), "feature/cx-8"), "main");
        assert!(git_local_branch_exists(repo.path(), "main").unwrap());

        let worktree_parent = tempfile::tempdir().expect("worktree tempdir");
        let worktree_path = worktree_parent.path().join("feature-cx-8");
        let worktree_arg = worktree_path.display().to_string();
        git_ok(
            repo.path(),
            &[
                "worktree",
                "add",
                "-b",
                "feature/cx-8",
                worktree_arg.as_str(),
            ],
        );

        let common_root = git_common_worktree_root(worktree_path.as_path()).expect("common root");
        assert_eq!(
            std::fs::canonicalize(common_root).unwrap(),
            std::fs::canonicalize(repo.path()).unwrap()
        );
        assert!(git_local_branch_exists(repo.path(), "feature/cx-8").unwrap());
    }

    #[test]
    fn chat_wallet_payload_extracts_payment_requests_addresses_and_status() {
        let payload = extract_chat_wallet_payload(
            r#"{"payment_request":"lnbc1invoiceexample","amount_sats":"2100","status":"succeeded","payment_id":"pay-001","bitcoin_address":"bc1qwalletchat","description":"Bounty payout"}"#,
        );
        assert_eq!(
            payload,
            ChatWalletMessagePayload {
                payment_request: Some("lnbc1invoiceexample".to_string()),
                payment_id: Some("pay-001".to_string()),
                chat_reported_status: Some("succeeded".to_string()),
                amount_sats: Some(2100),
                copy_address: Some("bc1qwalletchat".to_string()),
                copy_address_label: Some("bitcoin"),
                description: Some("Bounty payout".to_string()),
            }
        );

        let token_payload = extract_chat_wallet_payload(
            "please settle bitcoin:bc1qexample?amount=0.001 or ping me at builder@spark.example",
        );
        assert_eq!(token_payload.copy_address.as_deref(), Some("bc1qexample"));
        assert_eq!(token_payload.copy_address_label, Some("bitcoin"));
    }

    #[test]
    fn chat_wallet_status_summary_distinguishes_chat_reported_and_wallet_confirmed() {
        let payload = ChatWalletMessagePayload {
            payment_id: Some("wallet-payment-001".to_string()),
            chat_reported_status: Some("succeeded".to_string()),
            ..ChatWalletMessagePayload::default()
        };
        let mut spark_wallet = crate::spark_wallet::SparkPaneState::default();
        spark_wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-001".to_string(),
                direction: "send".to_string(),
                status: "pending".to_string(),
                amount_sats: 1500,
                timestamp: 1_762_700_100,
                ..Default::default()
            });
        let summary =
            chat_wallet_payment_status_summary(&payload, &spark_wallet).expect("status summary");
        assert!(summary.contains("Spark confirms"));
        assert!(summary.contains("pending"));
        assert!(summary.contains("chat reported succeeded"));

        let unconfirmed = chat_wallet_payment_status_summary(
            &ChatWalletMessagePayload {
                payment_id: Some("wallet-payment-404".to_string()),
                chat_reported_status: Some("succeeded".to_string()),
                ..ChatWalletMessagePayload::default()
            },
            &crate::spark_wallet::SparkPaneState::default(),
        )
        .expect("unconfirmed summary");
        assert!(unconfirmed.contains("Chat reports succeeded"));
        assert!(unconfirmed.contains("has not confirmed"));
    }

    #[test]
    fn chat_wallet_status_summary_requires_payment_id_before_claiming_confirmation() {
        let summary = chat_wallet_payment_status_summary(
            &ChatWalletMessagePayload {
                payment_request: Some("lnbc1invoiceexample".to_string()),
                chat_reported_status: Some("succeeded".to_string()),
                ..ChatWalletMessagePayload::default()
            },
            &crate::spark_wallet::SparkPaneState::default(),
        )
        .expect("status summary");
        assert!(summary.contains("needs a wallet payment id"));
        assert!(!summary.contains("Spark confirms"));
    }

    #[test]
    fn waiting_for_wallet_payment_evidence_is_not_treated_as_an_error() {
        let mut active_job = crate::app_state::ActiveJobState::default();
        active_job.job = Some(fixture_open_network_delivered_job(2));
        active_job.last_error = Some(
            "open-network delivered job req-open-network-001 is waiting for wallet-authoritative payment evidence"
                .to_string(),
        );
        active_job.load_state = crate::app_state::PaneLoadState::Error;

        let mut provider_runtime = crate::state::provider_runtime::ProviderRuntimeState::default();
        provider_runtime.last_error_detail = Some(
            "execution: open-network delivered job req-open-network-001 is waiting for wallet-authoritative payment evidence"
                .to_string(),
        );
        provider_runtime.last_authoritative_error_class =
            Some(crate::app_state::EarnFailureClass::Payment);

        note_active_job_waiting_for_payment_evidence(
            &mut active_job,
            &mut provider_runtime,
            crate::app_state::JobDemandSource::OpenNetwork,
            "req-open-network-001",
        );

        assert_eq!(active_job.last_error, None);
        assert_eq!(
            active_job.load_state,
            crate::app_state::PaneLoadState::Ready
        );
        assert_eq!(
            active_job.last_action.as_deref(),
            Some(
                "open-network delivered job req-open-network-001 and is awaiting buyer Lightning payment confirmation"
            )
        );
        assert_eq!(
            provider_runtime.last_result.as_deref(),
            Some(
                "open-network delivered job req-open-network-001 and is awaiting buyer Lightning payment confirmation"
            )
        );
        assert_eq!(provider_runtime.last_error_detail, None);
        assert_eq!(provider_runtime.last_authoritative_error_class, None);
    }

    #[test]
    fn waiting_for_wallet_payment_evidence_does_not_clear_real_payment_errors() {
        let mut active_job = crate::app_state::ActiveJobState::default();
        active_job.job = Some(fixture_open_network_delivered_job(2));

        let mut provider_runtime = crate::state::provider_runtime::ProviderRuntimeState::default();
        provider_runtime.last_error_detail =
            Some("payment: provider settlement invoice creation failed".to_string());
        provider_runtime.last_authoritative_error_class =
            Some(crate::app_state::EarnFailureClass::Payment);

        note_active_job_waiting_for_payment_evidence(
            &mut active_job,
            &mut provider_runtime,
            crate::app_state::JobDemandSource::OpenNetwork,
            "req-open-network-001",
        );

        assert_eq!(
            provider_runtime.last_error_detail.as_deref(),
            Some("payment: provider settlement invoice creation failed")
        );
        assert_eq!(
            provider_runtime.last_authoritative_error_class,
            Some(crate::app_state::EarnFailureClass::Payment)
        );
    }

    #[test]
    fn payment_evidence_refresh_due_queues_immediately_and_then_throttles() {
        let mut active_job = crate::app_state::ActiveJobState::default();
        let now = std::time::Instant::now();

        assert!(active_job_payment_evidence_refresh_due(
            &mut active_job,
            now
        ));
        let next_due = active_job
            .next_payment_evidence_refresh_at
            .expect("refresh deadline should be scheduled");
        assert_eq!(next_due, now + ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL);

        assert!(!active_job_payment_evidence_refresh_due(
            &mut active_job,
            now + ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL
                - std::time::Duration::from_millis(1),
        ));
        assert!(active_job_payment_evidence_refresh_due(
            &mut active_job,
            now + ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL,
        ));
    }

    #[test]
    fn direct_message_builder_wraps_gift_events_and_room_identity() {
        let identity = fixture_identity();
        let reply_target = crate::app_state::DirectMessageMessageProjection {
            message_id: "aa".repeat(32),
            room_id: crate::app_state::direct_message_room_id(
                None,
                &[identity.public_key_hex.clone(), "33".repeat(32)],
            ),
            author_pubkey: "33".repeat(32),
            participant_pubkeys: vec![identity.public_key_hex.clone(), "33".repeat(32)],
            recipient_pubkeys: vec![identity.public_key_hex.clone()],
            content: "root".to_string(),
            created_at: 42,
            reply_to_event_id: None,
            subject: None,
            wrapped_event_ids: Vec::new(),
            delivery_state: crate::app_state::ManagedChatDeliveryState::Confirmed,
            delivery_error: None,
            attempt_count: 0,
        };
        let outbound = build_direct_message_outbound_message(
            &identity,
            vec!["33".repeat(32), "44".repeat(32)],
            std::collections::BTreeMap::from([
                ("33".repeat(32), vec!["wss://relay.one".to_string()]),
                ("44".repeat(32), vec!["wss://relay.two".to_string()]),
            ]),
            "draft posted",
            Some(&reply_target),
            Some("Design Review".to_string()),
        )
        .unwrap();

        assert_eq!(outbound.wrapped_events.len(), 2);
        assert_eq!(
            outbound.room_id,
            crate::app_state::direct_message_room_id(
                Some("Design Review"),
                &[
                    identity.public_key_hex.clone(),
                    "33".repeat(32),
                    "44".repeat(32)
                ],
            )
        );
        assert_eq!(
            outbound.reply_to_event_id.as_deref(),
            Some("aa".repeat(32).as_str())
        );
        assert!(
            outbound
                .wrapped_events
                .iter()
                .all(|event| event.kind == nostr::nip59::KIND_GIFT_WRAP)
        );
        assert!(
            outbound
                .wrapped_events
                .iter()
                .all(|event| !event.content.trim().is_empty())
        );
    }
}
