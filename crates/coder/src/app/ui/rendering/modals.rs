include!("modals/model_picker.rs");
include!("modals/session_list.rs");
include!("modals/agent_list.rs");
include!("modals/skill_list.rs");
include!("modals/hooks.rs");
include!("modals/tool_list.rs");
include!("modals/permission_rules.rs");
include!("modals/wallet.rs");
include!("modals/dvm.rs");
include!("modals/gateway.rs");
include!("modals/lm_router.rs");
include!("modals/nexus.rs");
include!("modals/oanix.rs");
include!("modals/directives.rs");
include!("modals/issues.rs");
include!("modals/autopilot_issues.rs");
include!("modals/rlm.rs");
include!("modals/rlm_trace.rs");
include!("modals/pylon_earnings.rs");
include!("modals/pylon_jobs.rs");
include!("modals/spark_wallet.rs");
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
        ModalState::DvmProviders => {
            render_dvm_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Gateway => {
            render_gateway_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::LmRouter => {
            render_lm_router_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Nexus => {
            render_nexus_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::SparkWallet => {
            render_spark_wallet_modal(
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
        ModalState::Directives => {
            render_directives_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Issues => {
            render_issues_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::AutopilotIssues => {
            render_autopilot_issues_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::Rlm => {
            render_rlm_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::RlmTrace => {
            render_rlm_trace_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::PylonEarnings => {
            render_pylon_earnings_modal(
                state,
                scene,
                palette,
                bounds,
                logical_width,
                logical_height,
                scale_factor,
            );
        }
        ModalState::PylonJobs => {
            render_pylon_jobs_modal(
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
        // Placeholder for new feature modals - render a basic "coming soon" overlay
        ModalState::Wallet
        | ModalState::DvmProviders
        | ModalState::Gateway
        | ModalState::LmRouter
        | ModalState::Nexus
        | ModalState::SparkWallet
        | ModalState::Nip90Jobs
        | ModalState::Oanix
        | ModalState::Directives
        | ModalState::Issues
        | ModalState::AutopilotIssues
        | ModalState::Rlm
        | ModalState::RlmTrace
        | ModalState::PylonEarnings
        | ModalState::Dspy
        | ModalState::Nip28Chat => {
            scene.set_layer(1);
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = 400.0;
            let modal_height = 120.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let title_run = state.text_system.layout_styled_mono(
                "Feature in development",
                Point::new(modal_x + 16.0, modal_y + 16.0),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);

            let desc_run = state.text_system.layout_styled_mono(
                "This feature is coming soon.",
                Point::new(modal_x + 16.0, modal_y + 50.0),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);

            let footer_run = state.text_system.layout_styled_mono(
                "Press Esc to close",
                Point::new(modal_x + 16.0, modal_y + modal_height - 24.0),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
    }
}
