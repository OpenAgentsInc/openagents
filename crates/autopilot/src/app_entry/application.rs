use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use arboard::Clipboard;
use rfd::FileDialog;
use tokio::sync::mpsc;
use web_time::Instant;
use wgpui::components::hud::CommandPalette;
use wgpui::components::{Component, EventContext, EventResult};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, InputEvent, Point, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::app::agents::AgentBackendsState;
use crate::app::agents::AgentRegistry;
use crate::app::autopilot::AutopilotState;
use crate::app::autopilot_issues::AutopilotIssuesState;
use crate::app::catalog::{
    CatalogState, load_agent_entries, load_hook_config, load_hook_scripts,
    load_mcp_project_servers, load_skill_entries,
};
use crate::app::chat::{ChatSelection, ChatState};
use crate::app::codex_app_server as app_server;
use crate::app::config::{AgentSelection, SettingsState, mcp_project_file};
use crate::app::dspy::DspyState;
use crate::app::dvm::DvmState;
use crate::app::events::{
    CoderMode, ModalState, convert_key_for_binding, convert_key_for_input, convert_modifiers,
    convert_mouse_button,
};
use crate::app::gateway::GatewayState;
use crate::app::git::GitState;
use crate::app::lm_router::LmRouterState;
use crate::app::nexus::NexusState;
use crate::app::nip28::Nip28State;
use crate::app::nip90::Nip90State;
use crate::app::permissions::{
    PermissionState, coder_mode_default_allow, coder_mode_label, load_permission_config,
};
use crate::app::pylon_earnings::PylonEarningsState;
use crate::app::pylon_jobs::PylonJobsState;
use crate::app::rlm::{RlmState, RlmTraceState};
use crate::app::session::{
    SessionState, apply_session_history_limit, load_session_index, save_session_index,
};
use crate::app::spark_wallet::SparkWalletState;
use crate::app::tools::ToolsState;
use crate::app::ui::{
    CONTENT_PADDING_X, SESSION_MODAL_HEIGHT, STATUS_BAR_HEIGHT, agent_list_layout,
    agent_modal_content_top, approvals_panel_layout, composer_bar_layout, composer_menu_layout,
    diff_back_button_bounds, git_diff_panel_layout, hook_event_layout, modal_y_in_content,
    new_session_button_bounds, session_list_layout, sidebar_layout, skill_list_layout,
    skill_modal_content_top, workspace_list_layout,
};
use crate::app::ui::{ThemeSetting, resolve_theme};
use crate::app::wallet::WalletState;
use crate::app::workspaces::{ComposerMenuKind, WorkspaceAccessMode, WorkspaceState};
use crate::app::{AppState, HookModalView, build_input};
use crate::keybindings::{Action as KeyAction, match_action};
use crate::panels::PanelLayout;

use super::AutopilotApp;
use super::commands::handle_modal_input;
use super::settings::{
    apply_codex_oss_env, fetch_rate_limits, load_keybindings, load_settings, settings_model_option,
};

impl ApplicationHandler for AutopilotApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );
            let (command_palette_tx, command_palette_rx) = mpsc::unbounded_channel();
            let command_palette = CommandPalette::new()
                .max_visible_items(8)
                .mono(true)
                .on_select(move |command| {
                    let _ = command_palette_tx.send(command.id.clone());
                });
            let settings = load_settings();
            apply_codex_oss_env(&settings);
            let system_theme = window.theme().map(|theme| match theme {
                winit::window::Theme::Light => ThemeSetting::Light,
                winit::window::Theme::Dark => ThemeSetting::Dark,
            });
            let resolved_theme = resolve_theme(settings.theme, system_theme);
            let input = build_input(&settings, resolved_theme);

            let selected_model = settings_model_option(&settings);
            let mut session_index = load_session_index();
            let removed_sessions =
                apply_session_history_limit(&mut session_index, settings.session_history_limit);
            if !removed_sessions.is_empty() {
                let _ = save_session_index(&session_index);
            }
            let permission_config = load_permission_config();
            let coder_mode = permission_config.coder_mode;
            let permission_default_allow =
                coder_mode_default_allow(coder_mode, permission_config.default_allow);
            let coder_mode_label_str = coder_mode_label(coder_mode).to_string();
            let cwd = std::env::current_dir().unwrap_or_default();
            let (mcp_project_servers, mcp_project_error) = load_mcp_project_servers(&cwd);
            let mcp_project_path = Some(mcp_project_file(&cwd));
            let agent_catalog = load_agent_entries(&cwd);
            let skill_catalog = load_skill_entries(&cwd);
            let hook_config = load_hook_config();
            let hook_catalog = load_hook_scripts(&cwd);

            // Boot OANIX on startup (async, will be cached when ready)
            tracing::info!("Booting OANIX runtime...");
            let (oanix_tx, oanix_rx) = mpsc::unbounded_channel();
            let oanix_manifest_rx = Some(oanix_rx);
            tokio::spawn(async move {
                match adjutant::boot().await {
                    Ok(manifest) => {
                        tracing::info!(
                            "OANIX booted on startup, workspace: {:?}",
                            manifest.workspace.as_ref().map(|w| &w.root)
                        );
                        let _ = oanix_tx.send(manifest);
                    }
                    Err(e) => {
                        tracing::warn!("OANIX boot failed on startup: {}", e);
                    }
                }
            });

            // Fetch rate limits on startup
            let (rate_limit_tx, rate_limit_rx) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                if let Some(limits) = fetch_rate_limits().await {
                    let _ = rate_limit_tx.send(limits);
                }
            });

            AppState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                event_context,
                clipboard,
                command_palette,
                command_palette_action_rx: Some(command_palette_rx),
                input,
                mouse_pos: (0.0, 0.0),
                modifiers: ModifiersState::default(),
                last_tick: Instant::now(),
                modal_state: ModalState::None,
                panel_layout: PanelLayout::Single,
                left_sidebar_open: true,
                right_sidebar_open: true,
                new_session_button_hovered: false,
                chat: ChatState::new(&settings, resolved_theme),
                tools: ToolsState::new(),
                git: GitState::new(),
                session: SessionState::new(
                    selected_model,
                    coder_mode_label_str,
                    session_index,
                    Some(rate_limit_rx),
                ),
                catalogs: CatalogState::new(
                    agent_catalog,
                    skill_catalog,
                    hook_config,
                    hook_catalog,
                    mcp_project_servers,
                    mcp_project_error,
                    mcp_project_path,
                ),
                agent_backends: AgentBackendsState::new(selected_model),
                workspaces: WorkspaceState::new(),
                settings: SettingsState::new(settings, load_keybindings(), selected_model),
                permissions: PermissionState::new(
                    coder_mode,
                    permission_default_allow,
                    permission_config.allow_tools,
                    permission_config.deny_tools,
                    permission_config.bash_allow_patterns,
                    permission_config.bash_deny_patterns,
                ),
                autopilot: AutopilotState::new(oanix_manifest_rx),
                autopilot_issues: AutopilotIssuesState::new(),
                rlm: RlmState::new(),
                rlm_trace: RlmTraceState::new(),
                pylon_earnings: PylonEarningsState::new(),
                pylon_jobs: PylonJobsState::new(),
                wallet: WalletState::new(),
                dspy: DspyState::new(),
                dvm: DvmState::new(),
                gateway: GatewayState::new(),
                lm_router: LmRouterState::new(),
                nexus: NexusState::new(),
                spark_wallet: SparkWalletState::new(),
                nip28: Nip28State::new(),
                nip90: Nip90State::new(),
                system_theme,
                show_kitchen_sink: false,
                kitchen_sink_scroll: 0.0,
                help_scroll_offset: 0.0,
                agent_selection: AgentSelection::default(),
                agent_registry: AgentRegistry::new(),
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Poll for SDK responses first
        self.poll_responses();
        self.poll_permissions();
        self.poll_command_palette_actions();
        self.poll_session_actions();
        self.poll_agent_actions();
        self.poll_skill_actions();
        self.poll_settings_actions();
        self.poll_hook_inspector_actions();
        self.poll_oanix_manifest();
        self.poll_nip28_events();
        self.poll_nip90_events();
        self.poll_dvm_events();
        self.poll_gateway_events();
        self.poll_lm_router_events();
        self.poll_nexus_events();
        self.poll_spark_wallet_events();
        self.poll_agent_backends_events();
        self.poll_workspace_events();
        self.poll_git_events();
        self.poll_autopilot_history();
        self.poll_rate_limits();

        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        let sidebar_layout = sidebar_layout(
            logical_width,
            logical_height,
            state.left_sidebar_open,
            state.right_sidebar_open,
        );
        let content_x = sidebar_layout.main.origin.x + CONTENT_PADDING_X;
        let input_layout = state.build_input_layout(&sidebar_layout, logical_height);
        let input_bounds = input_layout.input_bounds;
        let input_disabled = state.workspaces.active_thread_is_reviewing();
        let permission_open = state
            .permissions
            .permission_dialog
            .as_ref()
            .map(|dialog| dialog.is_open())
            .unwrap_or(false);
        let permission_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Focused(true) => {
                state.workspaces.refresh_on_focus();
                if let Some(active_id) = state.workspaces.active_workspace_id.as_ref() {
                    state.git.force_refresh(active_id);
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                state.system_theme = Some(match theme {
                    winit::window::Theme::Light => ThemeSetting::Light,
                    winit::window::Theme::Dark => ThemeSetting::Dark,
                });
                if state.settings.coder_settings.theme == ThemeSetting::System {
                    state.apply_settings();
                }
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;
                state.mouse_pos = (x, y);
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let input_event = InputEvent::MouseMove { x, y };
                        let _ =
                            dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state
                            .session
                            .checkpoint_restore
                            .size_hint()
                            .1
                            .unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore.event(
                                &input_event,
                                bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Track hover state for left sidebar button
                if state.left_sidebar_open {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        let was_hovered = state.new_session_button_hovered;
                        state.new_session_button_hovered = btn_bounds.contains(Point::new(x, y));
                        if was_hovered != state.new_session_button_hovered {
                            // Change cursor to pointer when hovering button
                            let cursor = if state.new_session_button_hovered {
                                CursorIcon::Pointer
                            } else {
                                CursorIcon::Default
                            };
                            state.window.set_cursor(cursor);
                            state.window.request_redraw();
                        }
                    }
                } else if state.new_session_button_hovered {
                    // Reset cursor when sidebar closes
                    state.new_session_button_hovered = false;
                    state.window.set_cursor(CursorIcon::Default);
                }

                if state.git.center_mode == crate::app::CenterMode::Diff {
                    let back_bounds = diff_back_button_bounds(&sidebar_layout);
                    let hovered = back_bounds.contains(Point::new(x, y));
                    if state.git.back_button_hovered != hovered {
                        state.git.back_button_hovered = hovered;
                        state.window.request_redraw();
                    }
                    return;
                }

                let input_event = InputEvent::MouseMove { x, y };
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        state.window.request_redraw();
                        return;
                    }
                }
                if state.chat.chat_selection_dragging {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if let Some(selection) = &mut state.chat.chat_selection {
                            if selection.focus.message_index != point.message_index
                                || selection.focus.offset != point.offset
                            {
                                selection.focus = point;
                                state.window.request_redraw();
                            }
                        }
                    }
                }
                // Handle events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card.event(
                                    &input_event,
                                    block.card_bounds,
                                    &mut state.event_context
                                ),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail.event(
                                        &input_event,
                                        detail_bounds,
                                        &mut state.event_context
                                    ),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                if !input_disabled {
                    state
                        .input
                        .event(&input_event, input_bounds, &mut state.event_context);
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let (x, y) = state.mouse_pos;
                let modifiers = wgpui::Modifiers::default();
                let input_event = if button_state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        button: convert_mouse_button(button),
                        x,
                        y,
                        modifiers,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: convert_mouse_button(button),
                        x,
                        y,
                    }
                };
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let _ =
                            dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    let palette_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                    let _ = state.command_palette.event(
                        &input_event,
                        palette_bounds,
                        &mut state.event_context,
                    );
                    state.window.request_redraw();
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state
                            .session
                            .checkpoint_restore
                            .size_hint()
                            .1
                            .unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore.event(
                                &input_event,
                                bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected_index = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected_index,
                    );
                    let mut handled = false;
                    if button_state == ElementState::Released {
                        if layout.list_bounds.contains(Point::new(x, y)) {
                            for (index, bounds) in &layout.row_bounds {
                                if bounds.contains(Point::new(x, y)) {
                                    state.modal_state = ModalState::Hooks {
                                        view: HookModalView::Events,
                                        selected: *index,
                                    };
                                    state.sync_hook_inspector(*index);
                                    handled = true;
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Handle click on left sidebar "New Session" button
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                    && state.left_sidebar_open
                {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        if btn_bounds.contains(Point::new(x, y)) {
                            match FileDialog::new().pick_folder() {
                                Some(path) => {
                                    state.workspaces.runtime.add_workspace(path, None);
                                    state.push_system_message("Adding workspace...".to_string());
                                }
                                None => {
                                    state.push_system_message(
                                        "Workspace selection canceled.".to_string(),
                                    );
                                }
                            }
                            state.window.request_redraw();
                            return;
                        }
                    }
                }

                // Handle selection/connect clicks in workspace list
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                    && state.left_sidebar_open
                {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let list_layout =
                            workspace_list_layout(left_bounds, state.workspaces.workspaces.len());
                        for (index, workspace) in state.workspaces.workspaces.iter().enumerate() {
                            if index < list_layout.connect_pills.len()
                                && !workspace.connected
                                && list_layout.connect_pills[index].contains(Point::new(x, y))
                            {
                                let workspace_id = workspace.id.clone();
                                state.workspaces.runtime.connect_workspace(workspace_id);
                                state.window.request_redraw();
                                return;
                            }
                            if index < list_layout.rows.len()
                                && list_layout.rows[index].contains(Point::new(x, y))
                            {
                                let workspace_id = workspace.id.clone();
                                let is_connected = workspace.connected;
                                state.workspaces.set_active_workspace(workspace_id.clone());
                                state.git.set_active_workspace(Some(&workspace_id));
                                if is_connected {
                                    state.workspaces.runtime.list_threads(workspace_id.clone());
                                }
                                state.workspaces.request_composer_data(&workspace_id);
                                state.sync_workspace_timeline_view();
                                state.workspaces.timeline_dirty = false;
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }
                }

                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    if state.git.center_mode == crate::app::CenterMode::Diff {
                        let back_bounds = diff_back_button_bounds(&sidebar_layout);
                        if back_bounds.contains(Point::new(x, y)) {
                            state.git.exit_diff_view();
                            state.window.request_redraw();
                            return;
                        }
                    }
                    if state.right_sidebar_open {
                        if let Some(right_bounds) = sidebar_layout.right {
                            if let Some(active_id) = state.workspaces.active_workspace_id.clone() {
                                if let Some(status) = state.git.status_for_workspace(&active_id) {
                                    let file_paths: Vec<String> =
                                        status.files.iter().map(|file| file.path.clone()).collect();
                                    let panel_layout =
                                        git_diff_panel_layout(right_bounds, file_paths.len());
                                    if panel_layout.list_bounds.contains(Point::new(x, y)) {
                                        for (index, bounds) in &panel_layout.row_bounds {
                                            if bounds.contains(Point::new(x, y)) {
                                                if let Some(path) = file_paths.get(*index) {
                                                    state.git.select_diff_path(path.clone());
                                                    if let Some(active_id) = state
                                                        .workspaces
                                                        .active_workspace_id
                                                        .as_ref()
                                                    {
                                                        state.git.force_refresh(active_id);
                                                    }
                                                    state.window.request_redraw();
                                                    return;
                                                }
                                            }
                                        }
                                    }

                                    let approvals = state
                                        .workspaces
                                        .approvals_for_workspace(&active_id)
                                        .to_vec();
                                    if let Some(approvals_layout) = approvals_panel_layout(
                                        right_bounds,
                                        &panel_layout,
                                        approvals.len(),
                                    ) {
                                        for (index, bounds) in &approvals_layout.approve_bounds {
                                            if bounds.contains(Point::new(x, y)) {
                                                if let Some(request) = approvals.get(*index) {
                                                    let response = app_server::ApprovalResponse {
                                                        decision:
                                                            app_server::ApprovalDecision::Accept,
                                                        accept_settings: None,
                                                    };
                                                    state.workspaces.runtime.respond_to_request(
                                                        active_id.clone(),
                                                        request.id.clone(),
                                                        response,
                                                    );
                                                    state.workspaces.remove_approval(
                                                        &active_id,
                                                        &request.id_label(),
                                                    );
                                                    state.window.request_redraw();
                                                    return;
                                                }
                                            }
                                        }
                                        for (index, bounds) in &approvals_layout.decline_bounds {
                                            if bounds.contains(Point::new(x, y)) {
                                                if let Some(request) = approvals.get(*index) {
                                                    let response = app_server::ApprovalResponse {
                                                        decision:
                                                            app_server::ApprovalDecision::Decline,
                                                        accept_settings: None,
                                                    };
                                                    state.workspaces.runtime.respond_to_request(
                                                        active_id.clone(),
                                                        request.id.clone(),
                                                        response,
                                                    );
                                                    state.workspaces.remove_approval(
                                                        &active_id,
                                                        &request.id_label(),
                                                    );
                                                    state.window.request_redraw();
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if state.git.center_mode == crate::app::CenterMode::Diff {
                    state.window.request_redraw();
                    return;
                }

                let composer_disabled = state.workspaces.active_thread_is_reviewing();
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    let click = Point::new(x, y);
                    let labels = state.workspaces.composer_labels();
                    let bar_layout = composer_bar_layout(
                        &mut state.text_system,
                        input_layout.bar_bounds,
                        &crate::app::workspaces::ComposerLabels {
                            model: format!("{} ▾", labels.model),
                            effort: format!("{} ▾", labels.effort),
                            access: format!("{} ▾", labels.access),
                            skill: format!("{} ▾", labels.skill),
                        },
                    );

                    let mut menu_handled = false;
                    if let Some(menu) = state.workspaces.composer_menu {
                        let composer = state.workspaces.active_composer();
                        let item_count = match menu {
                            ComposerMenuKind::Model => {
                                composer.map(|c| c.models.len()).unwrap_or(0)
                            }
                            ComposerMenuKind::Effort => composer
                                .map(|c| c.reasoning_options().len().max(1))
                                .unwrap_or(0),
                            ComposerMenuKind::Access => WorkspaceAccessMode::all().len(),
                            ComposerMenuKind::Skill => {
                                composer.map(|c| c.skills.len().max(1)).unwrap_or(0)
                            }
                        };
                        if item_count > 0 {
                            let anchor = match menu {
                                ComposerMenuKind::Model => bar_layout.model_bounds,
                                ComposerMenuKind::Effort => bar_layout.effort_bounds,
                                ComposerMenuKind::Access => bar_layout.access_bounds,
                                ComposerMenuKind::Skill => bar_layout.skill_bounds,
                            };
                            let menu_layout = composer_menu_layout(anchor, item_count);
                            if menu_layout.bounds.contains(click) {
                                for (index, bounds) in menu_layout.item_bounds {
                                    if bounds.contains(click) {
                                        if let Some(composer) =
                                            state.workspaces.active_composer_mut()
                                        {
                                            match menu {
                                                ComposerMenuKind::Model => {
                                                    if let Some(model) = composer.models.get(index)
                                                    {
                                                        composer.selected_model_id =
                                                            Some(model.id.clone());
                                                        composer.selected_effort =
                                                            Some(model.default_reasoning_effort);
                                                    }
                                                }
                                                ComposerMenuKind::Effort => {
                                                    let options = composer.reasoning_options();
                                                    if let Some(effort) = options.get(index) {
                                                        composer.selected_effort = Some(*effort);
                                                    }
                                                }
                                                ComposerMenuKind::Access => {
                                                    if let Some(mode) =
                                                        WorkspaceAccessMode::all().get(index)
                                                    {
                                                        composer.access_mode = *mode;
                                                    }
                                                }
                                                ComposerMenuKind::Skill => {
                                                    if let Some(skill) = composer.skills.get(index)
                                                    {
                                                        let snippet = format!("${}", skill.name);
                                                        let current =
                                                            state.input.get_value().to_string();
                                                        let next = if current.trim().is_empty() {
                                                            format!("{} ", snippet)
                                                        } else if current.contains(&snippet) {
                                                            current
                                                        } else {
                                                            format!(
                                                                "{} {} ",
                                                                current.trim_end(),
                                                                snippet
                                                            )
                                                        };
                                                        state.input.set_value(next);
                                                    }
                                                }
                                            }
                                        }
                                        state.workspaces.set_composer_menu(None);
                                        menu_handled = true;
                                        break;
                                    }
                                }
                            } else {
                                state.workspaces.set_composer_menu(None);
                            }
                        } else {
                            state.workspaces.set_composer_menu(None);
                        }
                    }

                    if menu_handled {
                        state.window.request_redraw();
                        return;
                    }

                    if input_layout.send_bounds.contains(click) {
                        let window = state.window.clone();
                        if !composer_disabled {
                            self.submit_input();
                        }
                        window.request_redraw();
                        return;
                    }

                    if !composer_disabled {
                        let next_menu = if bar_layout.model_bounds.contains(click) {
                            Some(ComposerMenuKind::Model)
                        } else if bar_layout.effort_bounds.contains(click) {
                            Some(ComposerMenuKind::Effort)
                        } else if bar_layout.access_bounds.contains(click) {
                            Some(ComposerMenuKind::Access)
                        } else if bar_layout.skill_bounds.contains(click) {
                            Some(ComposerMenuKind::Skill)
                        } else {
                            None
                        };
                        if let Some(kind) = next_menu {
                            if state.workspaces.composer_menu == Some(kind) {
                                state.workspaces.set_composer_menu(None);
                            } else {
                                state.workspaces.set_composer_menu(Some(kind));
                            }
                            state.window.request_redraw();
                            return;
                        }
                    }
                }

                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        if let Some(action) = state.chat.chat_context_menu.take_selected() {
                            state.handle_chat_menu_action(&action, &chat_layout);
                            state.chat.chat_context_menu_target = None;
                        }
                        state.window.request_redraw();
                        return;
                    }
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if state.modifiers.shift_key() {
                            if let Some(selection) = &mut state.chat.chat_selection {
                                selection.focus = point;
                            } else {
                                state.chat.chat_selection = Some(ChatSelection {
                                    anchor: point,
                                    focus: point,
                                });
                            }
                        } else {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = true;
                        state.window.request_redraw();
                    } else {
                        state.chat.chat_selection = None;
                    }
                }
                if button_state == ElementState::Released
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    state.chat.chat_selection_dragging = false;
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Right)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if !state.chat_selection_contains(point) {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = false;
                        let copy_enabled = state
                            .chat
                            .chat_selection
                            .as_ref()
                            .is_some_and(|sel| !sel.is_empty())
                            || chat_layout
                                .message_layouts
                                .get(point.message_index)
                                .is_some();
                        state.open_chat_context_menu(
                            Point::new(x, y),
                            Some(point.message_index),
                            copy_enabled,
                        );
                        state.window.request_redraw();
                        return;
                    }
                }
                // Handle mouse events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card.event(
                                    &input_event,
                                    block.card_bounds,
                                    &mut state.event_context
                                ),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail.event(
                                        &input_event,
                                        detail_bounds,
                                        &mut state.event_context
                                    ),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                if button_state == ElementState::Released
                    && !state.session.session_info.permission_mode.is_empty()
                {
                    let status_y = logical_height - STATUS_BAR_HEIGHT - 2.0;
                    let mode_text = format!("[{}]", state.session.session_info.permission_mode);
                    let mode_width = mode_text.len() as f32 * 6.6;
                    let mode_bounds = Bounds::new(
                        content_x,
                        status_y - 4.0,
                        mode_width,
                        STATUS_BAR_HEIGHT + 8.0,
                    );
                    if mode_bounds.contains(Point::new(x, y)) {
                        state
                            .permissions
                            .cycle_coder_mode(&mut state.session.session_info);
                        state.window.request_redraw();
                        return;
                    }
                }
                if !input_disabled {
                    state
                        .input
                        .event(&input_event, input_bounds, &mut state.event_context);
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if permission_open {
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                // Kitchen sink scroll handling
                if state.show_kitchen_sink {
                    state.kitchen_sink_scroll = (state.kitchen_sink_scroll - dy * 40.0).max(0.0);
                    state.window.request_redraw();
                    return;
                }
                // Help modal scroll handling
                if matches!(state.modal_state, ModalState::Help) {
                    state.help_scroll_offset = (state.help_scroll_offset - dy * 40.0).max(0.0);
                    state.window.request_redraw();
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                    if layout.inspector_bounds.contains(mouse_point) {
                        let input_event = InputEvent::Scroll {
                            dx: 0.0,
                            dy: dy * 40.0,
                        };
                        if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                            if matches!(
                                inspector.event(
                                    &input_event,
                                    layout.inspector_bounds,
                                    &mut state.event_context
                                ),
                                EventResult::Handled
                            ) {
                                state.window.request_redraw();
                                return;
                            }
                        }
                    } else if layout.list_bounds.contains(mouse_point) {
                        let mut next_selected = selected;
                        if dy > 0.0 {
                            next_selected = next_selected.saturating_add(1);
                        } else if dy < 0.0 {
                            next_selected = next_selected.saturating_sub(1);
                        }
                        if !state.catalogs.hook_event_log.is_empty() {
                            next_selected =
                                next_selected.min(state.catalogs.hook_event_log.len() - 1);
                        } else {
                            next_selected = 0;
                        }
                        if next_selected != selected {
                            state.modal_state = ModalState::Hooks {
                                view: HookModalView::Events,
                                selected: next_selected,
                            };
                            state.sync_hook_inspector(next_selected);
                            state.window.request_redraw();
                        }
                        return;
                    }
                }
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                // Handle scroll events for inline tools
                let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                let scroll_input_event = InputEvent::Scroll {
                    dx: 0.0,
                    dy: dy * 40.0,
                };
                let mut scroll_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if block.card_bounds.contains(mouse_point) {
                            if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                                if matches!(
                                    tool.card.event(
                                        &scroll_input_event,
                                        block.card_bounds,
                                        &mut state.event_context
                                    ),
                                    EventResult::Handled
                                ) {
                                    scroll_handled = true;
                                }
                                if let Some(detail_bounds) = block.detail_bounds {
                                    if matches!(
                                        tool.detail.event(
                                            &scroll_input_event,
                                            detail_bounds,
                                            &mut state.event_context
                                        ),
                                        EventResult::Handled
                                    ) {
                                        scroll_handled = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if scroll_handled {
                    state.window.request_redraw();
                    return;
                }
                if state.git.center_mode == crate::app::CenterMode::Diff {
                    state.git.diff_scroll_offset =
                        (state.git.diff_scroll_offset - dy * 40.0).max(0.0);
                    state.window.request_redraw();
                    return;
                }
                // Scroll the message area (positive dy = scroll up, negative = scroll down)
                state.chat.scroll_offset = (state.chat.scroll_offset - dy * 40.0).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == ElementState::Pressed {
                    if permission_open {
                        return;
                    }

                    if state.command_palette.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            let palette_bounds =
                                Bounds::new(0.0, 0.0, logical_width, logical_height);
                            let _ = state.command_palette.event(
                                &input_event,
                                palette_bounds,
                                &mut state.event_context,
                            );
                            state.window.request_redraw();
                        }
                        return;
                    }

                    // Kitchen sink overlay - handle Escape to close
                    if state.show_kitchen_sink {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            state.show_kitchen_sink = false;
                            state.window.request_redraw();
                            return;
                        }
                        // Consume all other keys while kitchen sink is open
                        return;
                    }

                    // Autopilot loop interrupt - Escape stops autonomous execution
                    if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            if state.chat.is_thinking {
                                // Signal interrupt to the autopilot loop
                                state
                                    .autopilot
                                    .autopilot_interrupt_flag
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                                tracing::info!("Autopilot: interrupt requested by user");
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if let WinitKey::Named(WinitNamedKey::F1) = &key_event.logical_key {
                        if matches!(state.modal_state, ModalState::Help) {
                            state.modal_state = ModalState::None;
                        } else {
                            state.open_help();
                        }
                        state.window.request_redraw();
                        return;
                    }
                    if handle_modal_input(state, &key_event.logical_key) {
                        return;
                    }

                    let modifiers = convert_modifiers(&state.modifiers);

                    if state.git.center_mode == crate::app::CenterMode::Diff {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            state.git.exit_diff_view();
                            state.window.request_redraw();
                            return;
                        }
                    }

                    if state.chat.chat_context_menu.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            if matches!(
                                state.chat.chat_context_menu.event(
                                    &input_event,
                                    Bounds::new(0.0, 0.0, logical_width, logical_height),
                                    &mut state.event_context,
                                ),
                                EventResult::Handled
                            ) {
                                if let Some(action) = state.chat.chat_context_menu.take_selected() {
                                    let chat_layout =
                                        state.build_chat_layout(&sidebar_layout, logical_height);
                                    state.handle_chat_menu_action(&action, &chat_layout);
                                    state.chat.chat_context_menu_target = None;
                                }
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if state.git.center_mode != crate::app::CenterMode::Diff {
                        if state.handle_chat_shortcut(
                            &key_event.logical_key,
                            modifiers,
                            &sidebar_layout,
                            logical_height,
                        ) {
                            state.window.request_redraw();
                            return;
                        }
                    }

                    if let Some(key) = convert_key_for_binding(&key_event.logical_key) {
                        if let Some(action) =
                            match_action(&key, modifiers, &state.settings.keybindings)
                        {
                            match action {
                                KeyAction::Interrupt => state.interrupt_query(),
                                KeyAction::OpenCommandPalette => {
                                    state.open_command_palette();
                                }
                                KeyAction::OpenSettings => state.open_config(),
                                KeyAction::OpenWallet => state.open_wallet(),
                                KeyAction::OpenAgentBackends => state.open_agent_backends(),
                                KeyAction::OpenDvm => state.open_dvm(),
                                KeyAction::OpenGateway => state.open_gateway(),
                                KeyAction::OpenLmRouter => state.open_lm_router(),
                                KeyAction::OpenNexus => state.open_nexus(),
                                KeyAction::OpenSparkWallet => state.open_spark_wallet(),
                                KeyAction::OpenNip90 => state.open_nip90(),
                                KeyAction::OpenOanix => state.open_oanix(),
                                KeyAction::OpenDirectives => state.open_directives(),
                                KeyAction::OpenIssues => state.open_issues(),
                                KeyAction::OpenIssueTracker => state.open_issue_tracker(),
                                KeyAction::OpenRlm => state.open_rlm(),
                                KeyAction::OpenRlmTrace => state.open_rlm_trace(None),
                                KeyAction::OpenPylonEarnings => state.open_pylon_earnings(),
                                KeyAction::OpenPylonJobs => state.open_pylon_jobs(),
                                KeyAction::OpenDspy => state.open_dspy(),
                                KeyAction::OpenNip28 => state.open_nip28(),
                                KeyAction::ToggleLeftSidebar => state.toggle_left_sidebar(),
                                KeyAction::ToggleRightSidebar => state.toggle_right_sidebar(),
                                KeyAction::ToggleSidebars => state.toggle_sidebars(),
                            }
                            state.window.request_redraw();
                            return;
                        }
                    }

                    if state.git.center_mode != crate::app::CenterMode::Diff {
                        // Check for Enter key to submit (but not Shift+Enter, which inserts newline)
                        if let WinitKey::Named(WinitNamedKey::Enter) = &key_event.logical_key {
                            if !state.modifiers.shift_key() {
                                if input_disabled {
                                    return;
                                }
                                let window = state.window.clone();
                                self.submit_input();
                                window.request_redraw();
                                return;
                            }
                            // Shift+Enter falls through to input handler below
                        }

                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            if !input_disabled {
                                state.input.event(
                                    &input_event,
                                    input_bounds,
                                    &mut state.event_context,
                                );
                                state.window.request_redraw();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws when:
        // - Input is focused (cursor blinking)
        // - Streaming content (real-time updates)
        // - Thinking indicator active
        if let Some(state) = &self.state {
            let needs_redraw = state.input.is_focused()
                || !state.chat.streaming_markdown.source().is_empty()
                || state.chat.is_thinking
                || state.tools.has_running();
            if needs_redraw {
                state.window.request_redraw();
            }
        }
    }
}
