use codex_client::{ThreadResumeParams, ThreadStartParams};

use crate::app_state::{DataSellerCodexSessionPhase, DataSellerSkillAttachment, RenderState};
use crate::codex_lane::CodexLaneCommand;

fn current_session_cwd() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok())
}

pub(crate) fn ensure_data_seller_codex_session(state: &mut RenderState) -> bool {
    if matches!(
        state.data_seller.codex_session_phase,
        DataSellerCodexSessionPhase::Starting | DataSellerCodexSessionPhase::Resuming
    ) {
        return true;
    }

    let cwd = current_session_cwd();
    match crate::skill_autoload::ensure_required_data_market_skills() {
        Ok(skills) => state.data_seller.set_required_skill_attachments(
            skills
                .into_iter()
                .map(|skill| DataSellerSkillAttachment {
                    name: skill.name,
                    path: skill.path,
                })
                .collect(),
        ),
        Err(error) => {
            tracing::warn!(
                "failed to auto-provision managed Data Market skills before seller session: {}",
                error
            );
            state.data_seller.last_error = Some(format!(
                "Failed to auto-provision seller skills: {error}"
            ));
        }
    }
    let model = state.autopilot_chat.selected_model_override();
    let service_tier = state.autopilot_chat.service_tier.request_value();
    let approval_policy = Some(state.autopilot_chat.approval_mode);
    let sandbox = Some(state.autopilot_chat.sandbox_mode);
    let personality = state.data_seller.codex_profile.personality.request_value();

    let command = if let Some(thread_id) = state.data_seller.codex_thread_id.clone() {
        state.data_seller.begin_codex_session_resume(cwd.clone());
        CodexLaneCommand::ThreadResume(ThreadResumeParams {
            thread_id,
            path: None,
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
        })
    } else {
        state.data_seller.begin_codex_session_start(cwd.clone());
        CodexLaneCommand::ThreadStart(ThreadStartParams {
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
            ephemeral: None,
            dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
        })
    };

    if let Err(error) = state.queue_codex_command(command) {
        state.data_seller.record_codex_session_error(format!(
            "Failed to queue Data Seller Codex session: {error}"
        ));
    }
    true
}
