include!("modals/model_picker.rs");
include!("modals/session_list.rs");
include!("modals/agent_list.rs");
include!("modals/skill_list.rs");
include!("modals/hooks.rs");
include!("modals/tool_list.rs");
include!("modals/permission_rules.rs");
include!("modals/wallet.rs");
include!("modals/oanix.rs");
include!("modals/dspy.rs");
include!("modals/nip28.rs");
include!("modals/nip90.rs");
include!("modals/config.rs");
include!("modals/mcp_config.rs");
include!("modals/help.rs");

fn render_modals(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    _sidebar_layout: &SidebarLayout,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
    let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

    // Draw modal if active
    let should_refresh_sessions = matches!(state.modal_state, ModalState::SessionList { .. })
        && state.session.session_cards.len() != state.session.session_index.len();
    if should_refresh_sessions {
        state.session.refresh_session_cards(state.chat.is_thinking);
    }
    let should_refresh_agents = matches!(state.modal_state, ModalState::AgentList { .. })
        && state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len();
    if should_refresh_agents {
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
    }
    let should_refresh_skills = matches!(state.modal_state, ModalState::SkillList { .. })
        && state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len();
    if should_refresh_skills {
        state.catalogs.refresh_skill_cards();
    }
    let modal_state = state.modal_state.clone();
    match modal_state {
        ModalState::None => {}
        ModalState::ModelPicker { selected } => {
            render_model_picker_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::SessionList { selected } => {
            render_session_list_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::AgentList { selected } => {
            render_agent_list_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::SkillList { selected } => {
            render_skill_list_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::Hooks { view, selected } => {
            render_hooks_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &view,
                &selected,
            );
        }
        ModalState::ToolList { selected } => {
            render_tool_list_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::PermissionRules => {
            render_permission_rules_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Wallet => {
            render_wallet_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Oanix => {
            render_oanix_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Dspy => {
            render_dspy_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Nip28Chat => {
            render_nip28_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Nip90Jobs => {
            render_nip90_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Config {
            tab,
            selected,
            search,
            input_mode,
        } => {
            render_config_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &tab,
                &selected,
                search.as_str(),
                &input_mode,
            );
        }
        ModalState::McpConfig { selected } => {
            render_mcp_config_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
                &selected,
            );
        }
        ModalState::Help => {
            render_help_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
    }
}
