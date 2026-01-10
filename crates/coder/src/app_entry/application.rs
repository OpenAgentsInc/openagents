use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use arboard::Clipboard;
use tokio::sync::mpsc;
use web_time::Instant;
use wgpui::components::{Component, EventContext, EventResult};
use wgpui::components::hud::CommandPalette;
use wgpui::renderer::Renderer;
use wgpui::{Bounds, InputEvent, Point, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::app::autopilot::AutopilotState;
use crate::app::catalog::{
    load_agent_entries, load_hook_config, load_hook_scripts, load_mcp_project_servers,
    load_skill_entries, CatalogState,
};
use crate::app::chat::{ChatSelection, ChatState};
use crate::app::config::{mcp_project_file, SettingsState};
use crate::app::events::{
    convert_key_for_binding, convert_key_for_input, convert_modifiers, convert_mouse_button,
    CommandAction, CoderMode, ModalState,
};
use crate::app::permissions::{
    coder_mode_default_allow, coder_mode_label, load_permission_config, PermissionState,
};
use crate::app::session::{
    apply_session_history_limit, load_session_index, save_session_index, SessionState,
};
use crate::app::tools::ToolsState;
use crate::app::ui::{
    agent_list_layout, agent_modal_content_top, hook_event_layout, modal_y_in_content,
    new_session_button_bounds, session_list_layout, sidebar_layout, skill_list_layout,
    skill_modal_content_top, INPUT_PADDING, OUTPUT_PADDING, SESSION_MODAL_HEIGHT,
    STATUS_BAR_HEIGHT,
};
use crate::app::wallet::WalletState;
use crate::app::dspy::DspyState;
use crate::app::nip28::Nip28State;
use crate::app::dvm::DvmState;
use crate::app::nip90::Nip90State;
use crate::app::gateway::GatewayState;
use crate::app::lm_router::LmRouterState;
use crate::app::{build_input, AppState, HookModalView};
use crate::commands::parse_command;
use crate::keybindings::{match_action, Action as KeyAction};
use crate::panels::PanelLayout;

use super::CoderApp;
use super::commands::{handle_command, handle_modal_input};
use super::settings::{
    auto_start_llama_server, fetch_rate_limits, load_keybindings, load_settings,
    settings_model_option,
};

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
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
            let input = build_input(&settings);

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

            // Auto-start llama-server if available but not running
            let llama_server_process = auto_start_llama_server();

            // Detect available LM providers on startup (after potential auto-start)
            let available_providers = adjutant::dspy::lm_config::detect_all_providers();
            tracing::info!("Available LM providers: {:?}", available_providers);

            // Boot OANIX on startup (async, will be cached when ready)
            tracing::info!("Booting OANIX runtime...");
            let (oanix_tx, oanix_rx) = mpsc::unbounded_channel();
            let oanix_manifest_rx = Some(oanix_rx);
            tokio::spawn(async move {
                match oanix::boot().await {
                    Ok(manifest) => {
                        tracing::info!("OANIX booted on startup, workspace: {:?}",
                            manifest.workspace.as_ref().map(|w| &w.root));
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
                left_sidebar_open: false,
                right_sidebar_open: false,
                new_session_button_hovered: false,
                chat: ChatState::new(&settings),
                tools: ToolsState::new(),
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
                settings: SettingsState::new(settings, load_keybindings(), selected_model),
                permissions: PermissionState::new(
                    coder_mode,
                    permission_default_allow,
                    permission_config.allow_tools,
                    permission_config.deny_tools,
                    permission_config.bash_allow_patterns,
                    permission_config.bash_deny_patterns,
                ),
                autopilot: AutopilotState::new(oanix_manifest_rx, available_providers),
                wallet: WalletState::new(),
                dspy: DspyState::new(),
                dvm: DvmState::new(),
                gateway: GatewayState::new(),
                lm_router: LmRouterState::new(),
                nip28: Nip28State::new(),
                nip90: Nip90State::new(),
                llama_server_process,
                show_kitchen_sink: false,
                kitchen_sink_scroll: 0.0,
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
        self.poll_hook_inspector_actions();
        self.poll_oanix_manifest();
        self.poll_nip28_events();
        self.poll_nip90_events();
        self.poll_dvm_events();
        self.poll_gateway_events();
        self.poll_lm_router_events();
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
        let content_x = sidebar_layout.main.origin.x + OUTPUT_PADDING;
        // Input bounds above status bar (max width 768px, centered)
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        let input_x = sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - input_width) / 2.0;
        // Set max width for text wrapping, then calculate dynamic height
        state.input.set_max_width(input_width);
        let input_height = state.input.current_height().max(40.0);
        let input_bounds = Bounds::new(
            input_x,
            logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT,
            input_width,
            input_height,
        );
        let permission_open = state.permissions.permission_dialog
            .as_ref()
            .map(|dialog| dialog.is_open())
            .unwrap_or(false);
        let permission_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
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
                        let _ = dialog.event(&input_event, permission_bounds, &mut state.event_context);
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
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
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
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
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
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
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
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
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
                    let _ = state
                        .command_palette
                        .event(&input_event, palette_bounds, &mut state.event_context);
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
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
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
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
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
                            state.start_new_session();
                            state.input.focus();
                            state.window.request_redraw();
                            return;
                        }
                    }
                }

                let chat_layout = state.build_chat_layout(
                    &sidebar_layout,
                    logical_height,
                );
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
                        let copy_enabled = state.chat.chat_selection
                            .as_ref()
                            .is_some_and(|sel| !sel.is_empty())
                            || chat_layout.message_layouts.get(point.message_index).is_some();
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
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
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
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
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
                        let input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
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
                            next_selected = next_selected.min(state.catalogs.hook_event_log.len() - 1);
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
                let scroll_input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
                let mut scroll_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if block.card_bounds.contains(mouse_point) {
                            if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                                if matches!(
                                    tool.card
                                        .event(&scroll_input_event, block.card_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    scroll_handled = true;
                                }
                                if let Some(detail_bounds) = block.detail_bounds {
                                    if matches!(
                                        tool.detail
                                            .event(&scroll_input_event, detail_bounds, &mut state.event_context),
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
                                state.autopilot.autopilot_interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
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
                                    let chat_layout = state.build_chat_layout(
                                        &sidebar_layout,
                                        logical_height,
                                    );
                                    state.handle_chat_menu_action(&action, &chat_layout);
                                    state.chat.chat_context_menu_target = None;
                                }
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if state.handle_chat_shortcut(
                        &key_event.logical_key,
                        modifiers,
                        &sidebar_layout,
                        logical_height,
                    ) {
                        state.window.request_redraw();
                        return;
                    }

                    if let Some(key) = convert_key_for_binding(&key_event.logical_key) {
                        if let Some(action) = match_action(&key, modifiers, &state.settings.keybindings) {
                            match action {
                                KeyAction::Interrupt => state.interrupt_query(),
                                KeyAction::OpenCommandPalette => {
                                    state.open_command_palette();
                                }
                                KeyAction::OpenSettings => state.open_config(),
                                KeyAction::OpenWallet => state.open_wallet(),
                                KeyAction::OpenDvm => state.open_dvm(),
                                KeyAction::OpenGateway => state.open_gateway(),
                                KeyAction::OpenLmRouter => state.open_lm_router(),
                                KeyAction::OpenNip90 => state.open_nip90(),
                                KeyAction::OpenOanix => state.open_oanix(),
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

                    if let WinitKey::Named(WinitNamedKey::Tab) = &key_event.logical_key {
                        if state.modifiers.shift_key() {
                            state
                                .permissions
                                .cycle_coder_mode(&mut state.session.session_info);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Check for Enter key to submit (but not Shift+Enter, which inserts newline)
                    if let WinitKey::Named(WinitNamedKey::Enter) = &key_event.logical_key {
                        if !state.modifiers.shift_key() {
                            let mut action = CommandAction::None;
                            let mut submit_prompt = None;

                            {
                                let prompt = state.input.get_value().to_string();
                                if prompt.trim().is_empty() {
                                    return;
                                }

                                if let Some(command) = parse_command(&prompt) {
                                    state.settings.command_history.push(prompt);
                                    state.input.set_value("");
                                    action = handle_command(state, command);
                                } else if !state.chat.is_thinking {
                                    state.settings.command_history.push(prompt.clone());
                                    state.input.set_value("");
                                    submit_prompt = Some(prompt);
                                } else {
                                    return;
                                }
                            }

                            if let CommandAction::SubmitPrompt(prompt) = action {
                                self.submit_prompt(prompt);
                            } else if let Some(prompt) = submit_prompt {
                                self.submit_prompt(prompt);
                            }

                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                            return;
                        }
                        // Shift+Enter falls through to input handler below
                    }

                    if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                        let input_event = InputEvent::KeyDown { key, modifiers };
                        state
                            .input
                            .event(&input_event, input_bounds, &mut state.event_context);
                        state.window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws when input is focused for cursor blinking
        if let Some(state) = &self.state {
            if state.input.is_focused() {
                state.window.request_redraw();
            }
        }
    }
}
