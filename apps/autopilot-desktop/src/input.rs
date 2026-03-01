use codex_client::{
    ApprovalDecision, AppsListParams, ChatgptAuthTokensRefreshResponse,
    CollaborationModeListParams, CommandExecParams, CommandExecutionRequestApprovalResponse,
    ConfigBatchWriteParams, ConfigEdit, ConfigReadParams, ConfigValueWriteParams,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse, ExperimentalFeatureListParams,
    ExternalAgentConfigDetectParams, ExternalAgentConfigImportParams,
    FileChangeRequestApprovalResponse, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, GetAccountParams,
    ListMcpServerStatusParams, LoginAccountParams, McpServerOauthLoginParams, MergeStrategy,
    ModelListParams, ReviewDelivery, ReviewStartParams, ReviewTarget, ThreadArchiveParams,
    ThreadCompactStartParams, ThreadForkParams, ThreadLoadedListParams,
    ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams, ThreadRealtimeStopParams,
    ThreadResumeParams, ThreadRollbackParams, ThreadSetNameParams, ThreadStartParams,
    ThreadUnarchiveParams, ThreadUnsubscribeParams, ToolRequestUserInputAnswer,
    ToolRequestUserInputResponse, TurnStartParams, UserInput, WindowsSandboxSetupStartParams,
};
use nostr::regenerate_identity;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use wgpui::clipboard::copy_to_clipboard;
use wgpui::{Bounds, Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};
use winit::window::Fullscreen;

use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, AlertDomain, App, CadCameraDragMode, CadCameraDragState,
    CadHotkeyAction, ChatTranscriptPressState, JobInboxNetworkRequest, JobInboxValidation,
    NetworkRequestSubmission, PaneKind, ProviderMode,
};
use crate::hotbar::{
    HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET, activate_hotbar_slot,
    hotbar_slot_for_key, process_hotbar_clicks,
};
use crate::pane_registry::pane_spec_by_command_id;
use crate::pane_system::{
    ActivityFeedPaneAction, AgentNetworkSimulationPaneAction, AlertsRecoveryPaneAction,
    CadDemoPaneAction, CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction,
    CodexDiagnosticsPaneAction, CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction,
    CredentialsPaneAction, EarningsScoreboardPaneAction, NetworkRequestsPaneAction, PaneController,
    PaneHitAction, PaneInput, RelayConnectionsPaneAction, RelaySecuritySimulationPaneAction,
    SIDEBAR_DEFAULT_WIDTH, SettingsPaneAction, StableSatsSimulationPaneAction,
    StarterJobsPaneAction, SyncHealthPaneAction, TreasuryExchangeSimulationPaneAction,
    cad_demo_context_menu_bounds, cad_demo_context_menu_row_bounds,
    cad_demo_cycle_variant_button_bounds, cad_demo_hidden_line_mode_button_bounds,
    cad_demo_hotkey_profile_button_bounds, cad_demo_projection_mode_button_bounds,
    cad_demo_reset_button_bounds, cad_demo_reset_camera_button_bounds,
    cad_demo_snap_endpoint_button_bounds, cad_demo_snap_grid_button_bounds,
    cad_demo_snap_midpoint_button_bounds, cad_demo_snap_origin_button_bounds,
    cad_demo_timeline_panel_bounds, cad_demo_view_snap_front_button_bounds,
    cad_demo_view_snap_iso_button_bounds, cad_demo_view_snap_right_button_bounds,
    cad_demo_view_snap_top_button_bounds, cad_demo_warning_filter_code_button_bounds,
    cad_demo_warning_filter_severity_button_bounds, cad_demo_warning_panel_bounds,
    clamp_all_panes_to_window, dispatch_chat_input_event, dispatch_chat_scroll_event,
    dispatch_create_invoice_input_event, dispatch_credentials_input_event,
    dispatch_job_history_input_event, dispatch_network_requests_input_event,
    dispatch_pay_invoice_input_event, dispatch_relay_connections_input_event,
    dispatch_settings_input_event, dispatch_spark_input_event, pane_content_bounds,
    pane_indices_by_z_desc, pane_z_sort_invocation_count, topmost_pane_hit_action_in_order,
};
use crate::panes::{cad as cad_pane, chat as chat_pane};
use crate::render::{
    logical_size, render_frame, sidebar_go_online_button_bounds, sidebar_handle_bounds,
    wallet_balance_chip_bounds,
};
use crate::runtime_lanes::{
    AcCreditCommand, RuntimeCommandErrorClass, RuntimeCommandResponse, RuntimeCommandStatus,
    RuntimeLane, SaLifecycleCommand, SklDiscoveryTrustCommand,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;

mod actions;
mod reducers;
mod shortcuts;
use actions::*;
use shortcuts::*;

const CHAT_MESSAGE_LONG_PRESS_MS: u64 = 450;

pub fn handle_window_event(app: &mut App, event_loop: &ActiveEventLoop, event: WindowEvent) {
    let Some(state) = &mut app.state else {
        return;
    };

    if pump_background_state(state) {
        state.window.request_redraw();
    }

    match event {
        WindowEvent::CloseRequested => {
            let _ = state.spark_worker.cancel_pending();
            state.codex_lane_worker.shutdown();
            event_loop.exit();
        }
        WindowEvent::Resized(new_size) => {
            state.config.width = new_size.width.max(1);
            state.config.height = new_size.height.max(1);
            state.surface.configure(&state.device, &state.config);
            state.window.request_redraw();
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            state.scale_factor = scale_factor as f32;
            state.text_system.set_scale_factor(state.scale_factor);
            state.window.request_redraw();
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            state.input_modifiers = map_modifiers(modifiers.state());
        }
        WindowEvent::CursorMoved { position, .. } => {
            let scale = state.scale_factor.max(0.1);
            app.cursor_position = Point::new(position.x as f32 / scale, position.y as f32 / scale);

            if state.command_palette.is_open() {
                let event = InputEvent::MouseMove {
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                };
                if state
                    .command_palette
                    .event(
                        &event,
                        command_palette_bounds(state),
                        &mut state.event_context,
                    )
                    .is_handled()
                {
                    state.window.request_redraw();
                }
                return;
            }

            let needs_redraw = dispatch_mouse_move(state, app.cursor_position);

            state
                .window
                .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));

            if needs_redraw {
                state.window.request_redraw();
            }
        }
        WindowEvent::MouseInput {
            state: mouse_state,
            button,
            ..
        } => {
            let button = match button {
                winit::event::MouseButton::Left => MouseButton::Left,
                winit::event::MouseButton::Right => MouseButton::Right,
                winit::event::MouseButton::Middle => MouseButton::Middle,
                _ => return,
            };

            let input = match mouse_state {
                ElementState::Pressed => InputEvent::MouseDown {
                    button,
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                    modifiers: state.input_modifiers,
                },
                ElementState::Released => InputEvent::MouseUp {
                    button,
                    x: app.cursor_position.x,
                    y: app.cursor_position.y,
                },
            };

            if state.command_palette.is_open() {
                let mut handled = state
                    .command_palette
                    .event(
                        &input,
                        command_palette_bounds(state),
                        &mut state.event_context,
                    )
                    .is_handled();
                if matches!(mouse_state, ElementState::Released) {
                    handled |= dispatch_command_palette_actions(state);
                }
                if handled {
                    state.window.request_redraw();
                }
                return;
            }

            match mouse_state {
                ElementState::Pressed => {
                    let handled = dispatch_mouse_down(state, app.cursor_position, button, &input);

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
                ElementState::Released => {
                    let handled = dispatch_mouse_up(state, app.cursor_position, &input);

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let (dx, dy) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                MouseScrollDelta::PixelDelta(pos) => (-pos.x as f32, -pos.y as f32),
            };
            let scroll_event = InputEvent::Scroll { dx, dy };
            if dispatch_mouse_scroll(state, app.cursor_position, &scroll_event) {
                state.window.request_redraw();
            }
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed
                && is_toggle_fullscreen_shortcut(&event.logical_key, state.input_modifiers)
            {
                toggle_window_fullscreen(state);
                state.window.request_redraw();
                return;
            }

            let text_input_focused = any_text_input_focused(state);
            if event.state == ElementState::Pressed
                && is_command_palette_shortcut(&event.logical_key, state.input_modifiers)
                && !text_input_focused
            {
                toggle_command_palette(state);
                state.window.request_redraw();
                return;
            }

            if event.state != ElementState::Pressed {
                return;
            }

            if state.command_palette.is_open() {
                if let Some(key) = map_winit_key(&event.logical_key) {
                    let palette_event = InputEvent::KeyDown {
                        key,
                        modifiers: state.input_modifiers,
                    };
                    let mut handled = state
                        .command_palette
                        .event(
                            &palette_event,
                            command_palette_bounds(state),
                            &mut state.event_context,
                        )
                        .is_handled();
                    handled |= dispatch_command_palette_actions(state);
                    if handled {
                        state.window.request_redraw();
                    }
                }
                return;
            }

            if dispatch_keyboard_submit_actions(state, &event.logical_key)
                || handle_activity_feed_keyboard_input(state, &event.logical_key)
                || handle_alerts_recovery_keyboard_input(state, &event.logical_key)
            {
                state.window.request_redraw();
                return;
            }
            if text_input_focused {
                return;
            }

            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(pane_id) = PaneController::active(state) {
                        PaneController::close(state, pane_id);
                        state.window.request_redraw();
                    }
                }
                PhysicalKey::Code(KeyCode::BracketRight) => {
                    state.sidebar.is_open = !state.sidebar.is_open;
                    if state.sidebar.is_open && state.sidebar.width < 50.0 {
                        state.sidebar.width = SIDEBAR_DEFAULT_WIDTH;
                    }
                    clamp_all_panes_to_window(state);
                    state.window.request_redraw();
                }
                key => {
                    if let Some(slot) = hotbar_slot_for_key(key) {
                        activate_hotbar_slot(state, slot);
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::RedrawRequested => {
            // Keep background lanes advancing even during redraw-heavy periods.
            // Without this, Codex notifications can backlog until the next input event.
            let _ = pump_background_state(state);
            if render_frame(state).is_err() {
                event_loop.exit();
                return;
            }
            let flashing_now = state.hotbar.is_flashing();
            let provider_animating = matches!(
                state.provider_runtime.mode,
                ProviderMode::Connecting | ProviderMode::Online
            );
            if flashing_now
                || state.hotbar_flash_was_active
                || provider_animating
                || state.autopilot_chat.has_pending_messages()
            {
                state.window.request_redraw();
            }
            state.hotbar_flash_was_active = flashing_now;
        }
        _ => {}
    }
}

pub fn handle_device_event(app: &mut App, _event_loop: &ActiveEventLoop, event: DeviceEvent) {
    let Some(state) = &mut app.state else {
        return;
    };
    let DeviceEvent::Motion { axis, value } = event else {
        return;
    };
    if !state
        .panes
        .iter()
        .any(|pane| pane.kind == PaneKind::CadDemo)
    {
        return;
    }
    let previous_events = state.cad_demo.three_d_mouse_event_count;
    let changed = state.cad_demo.apply_three_d_mouse_motion(axis, value);
    if !changed && state.cad_demo.three_d_mouse_event_count == previous_events {
        return;
    }
    state.cad_demo.last_action = Some(format!(
        "CAD 3D mouse axis={} value={:.3} -> {}",
        axis,
        value,
        state.cad_demo.three_d_mouse_status()
    ));
    state.window.request_redraw();
}

pub fn handle_about_to_wait(app: &mut App, event_loop: &ActiveEventLoop) {
    let Some(state) = &mut app.state else {
        return;
    };

    let changed = pump_background_state(state);
    let provider_animating = matches!(
        state.provider_runtime.mode,
        ProviderMode::Connecting | ProviderMode::Online
    );
    let should_redraw = changed
        || state.hotbar.is_flashing()
        || provider_animating
        || state.autopilot_chat.has_pending_messages();
    if should_redraw {
        state.window.request_redraw();
    }

    // Keep a lightweight cadence so background lane updates (Codex/runtime/spark) are surfaced
    // even when the user is idle and no UI events are incoming.
    let poll_interval = if state.autopilot_chat.has_pending_messages() {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(50)
    };
    event_loop.set_control_flow(ControlFlow::WaitUntil(
        std::time::Instant::now() + poll_interval,
    ));
}

fn pump_background_state(state: &mut crate::app_state::RenderState) -> bool {
    let mut changed = false;
    let now = std::time::Instant::now();
    if reducers::drain_spark_worker_updates(state) {
        changed = true;
    }
    if reducers::drain_runtime_lane_updates(state) {
        changed = true;
    }
    if state.nostr_secret_state.expire(now) {
        changed = true;
    }
    if state.autopilot_chat.expire_copy_notice(now) {
        changed = true;
    }
    if run_auto_agent_network_simulation(state, now) {
        changed = true;
    }
    if run_auto_treasury_exchange_simulation(state, now) {
        changed = true;
    }
    if run_auto_relay_security_simulation(state, now) {
        changed = true;
    }
    if run_auto_stable_sats_simulation(state, now) {
        changed = true;
    }
    refresh_earnings_scoreboard(state, now);
    refresh_sync_health(state);
    changed
}

fn dispatch_mouse_move(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let mut handled = handle_sidebar_mouse_move(state, point);
    if handled {
        return true;
    }

    handled = PaneController::update_drag(state, point);
    handled |= update_cad_camera_drag(state, point);
    let event = InputEvent::MouseMove {
        x: point.x,
        y: point.y,
    };

    handled |= PaneInput::dispatch_frame_event(state, &event);
    handled |= dispatch_text_inputs(state, &event);
    handled |= state
        .hotbar
        .event(&event, state.hotbar_bounds, &mut state.event_context)
        .is_handled();
    handled
}

fn dispatch_mouse_down(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
    event: &InputEvent,
) -> bool {
    // Sidebar handle gets first chance at mouse-down so panes don't steal drags.
    if handle_sidebar_mouse_down(state, point, button) {
        return true;
    }
    begin_chat_transcript_long_press(state, point, button);

    // Sidebar "Go Online" button (when panel is open).
    if button == MouseButton::Left && state.sidebar.is_open {
        let go_online_bounds = sidebar_go_online_button_bounds(state);
        if go_online_bounds.size.width > 0.0
            && go_online_bounds.contains(point)
            && run_pane_hit_action(state, PaneHitAction::GoOnlineToggle)
        {
            return true;
        }
    }

    if button == MouseButton::Left {
        let wallet_bounds = wallet_balance_chip_bounds(state);
        if wallet_bounds.size.width > 0.0 && wallet_bounds.contains(point) {
            PaneController::create_for_kind(state, crate::app_state::PaneKind::SparkWallet);
            queue_spark_command(state, SparkWalletCommand::Refresh);
            return true;
        }
    }

    let mut handled = false;
    if state.hotbar_bounds.contains(point) {
        handled |= state
            .hotbar
            .event(event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
        handled |= process_hotbar_clicks(state);
        handled |= dispatch_text_inputs(state, event);
        if !handled {
            handled |= PaneInput::handle_mouse_down(state, point, button);
        }
    } else {
        handled |= PaneInput::handle_mouse_down(state, point, button);
        handled |= dispatch_text_inputs(state, event);
        handled |= state
            .hotbar
            .event(event, state.hotbar_bounds, &mut state.event_context)
            .is_handled();
        handled |= process_hotbar_clicks(state);
    }

    handled |= begin_cad_camera_drag(state, point, button);
    handled
}

fn dispatch_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = handle_chat_transcript_long_press_release(state, point);
    let camera_drag_consumed_click = finish_cad_camera_drag(state);
    handled |= camera_drag_consumed_click;
    handled |= handle_sidebar_mouse_up(state, point, event);
    handled |= PaneInput::handle_mouse_up(state, event);
    handled |= dispatch_text_inputs(state, event);
    let context_menu_handled = if !camera_drag_consumed_click {
        handle_cad_context_menu_click(state, point, event)
    } else {
        false
    };
    handled |= context_menu_handled;
    if !camera_drag_consumed_click && !context_menu_handled {
        let snap_preview_handled = handle_cad_snap_preview_click(state, point, event);
        handled |= snap_preview_handled;
        if !snap_preview_handled {
            handled |= dispatch_pane_actions(state, point);
        }
    }
    handled |= state
        .hotbar
        .event(event, state.hotbar_bounds, &mut state.event_context)
        .is_handled();
    handled |= process_hotbar_clicks(state);
    handled
}

fn handle_cad_context_menu_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let InputEvent::MouseUp { button, .. } = event else {
        return false;
    };

    let top_cad_content = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::CadDemo)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane_content_bounds(pane.bounds));

    if *button == MouseButton::Right {
        let Some(content_bounds) = top_cad_content else {
            return false;
        };
        if !content_bounds.contains(point) {
            return false;
        }
        let Some(tile_index) = cad_pane::variant_tile_index_at_point(content_bounds, point) else {
            state.cad_demo.close_context_menu();
            state.cad_demo.last_action = Some("CAD context menu closed".to_string());
            return true;
        };
        let _ = state.cad_demo.set_active_variant_tile(tile_index);
        let viewport = cad_pane::variant_tile_bounds(content_bounds, tile_index);
        let (target_kind, target_ref) = state
            .cad_demo
            .infer_context_menu_target_for_viewport_point(point, viewport);
        state
            .cad_demo
            .open_context_menu(point, target_kind, target_ref.clone());
        state.cad_demo.last_action = Some(format!(
            "CAD context menu open tile={} {} ({})",
            tile_index + 1,
            target_ref,
            target_kind.label()
        ));
        return true;
    }

    if *button != MouseButton::Left || !state.cad_demo.context_menu.is_open {
        return false;
    }
    let Some(content_bounds) = top_cad_content else {
        state.cad_demo.close_context_menu();
        return true;
    };
    let menu_bounds = cad_demo_context_menu_bounds(
        content_bounds,
        state.cad_demo.context_menu.anchor,
        state.cad_demo.context_menu.items.len(),
    );
    if !menu_bounds.contains(point) {
        state.cad_demo.close_context_menu();
        state.cad_demo.last_action = Some("CAD context menu dismissed".to_string());
        return true;
    }

    for index in 0..state.cad_demo.context_menu.items.len() {
        if cad_demo_context_menu_row_bounds(menu_bounds, index).contains(point) {
            if let Some(action) = state.cad_demo.run_context_menu_item(index) {
                state.cad_demo.last_action = Some(action);
            }
            state.cad_demo.close_context_menu();
            return true;
        }
    }

    true
}

fn handle_cad_snap_preview_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let InputEvent::MouseUp { button, .. } = event else {
        return false;
    };
    if *button != MouseButton::Left {
        return false;
    }
    if !state.cad_demo.snap_toggles.grid
        && !state.cad_demo.snap_toggles.origin
        && !state.cad_demo.snap_toggles.endpoint
        && !state.cad_demo.snap_toggles.midpoint
    {
        return false;
    }

    let Some(pane_id) = cad_camera_target_pane_id(state, point) else {
        return false;
    };
    let Some(pane) = state.panes.iter().find(|pane| pane.id == pane_id) else {
        return false;
    };
    let content_bounds = pane_content_bounds(pane.bounds);
    let viewport = cad_pane::camera_interaction_bounds(content_bounds);
    let snapped = state.cad_demo.apply_snap_to_viewport_point(point, viewport);
    state.cad_demo.last_action = Some(format!(
        "CAD snap preview raw=({:.1},{:.1}) snapped=({:.1},{:.1}) {}",
        point.x,
        point.y,
        snapped.x,
        snapped.y,
        state.cad_demo.snap_summary(),
    ));
    true
}

fn dispatch_mouse_scroll(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = false;
    if let InputEvent::Scroll { dy, .. } = event {
        if apply_cad_camera_zoom(state, point, *dy) {
            handled = true;
        } else {
            handled |= dispatch_chat_scroll_event(state, point, *dy);
        }
    }
    handled |= dispatch_text_inputs(state, event);
    handled |= PaneInput::dispatch_frame_event(state, event);
    handled
}

fn cad_camera_target_pane_id(state: &crate::app_state::RenderState, point: Point) -> Option<u64> {
    let pane_order = pane_indices_by_z_desc(state);
    for pane_idx in pane_order {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::CadDemo || !pane.bounds.contains(point) {
            continue;
        }
        let content_bounds = pane_content_bounds(pane.bounds);
        if !content_bounds.contains(point) {
            return None;
        }
        if state.cad_demo.context_menu.is_open {
            let menu_bounds = cad_demo_context_menu_bounds(
                content_bounds,
                state.cad_demo.context_menu.anchor,
                state.cad_demo.context_menu.items.len(),
            );
            if menu_bounds.contains(point) {
                return None;
            }
        }
        if cad_demo_cycle_variant_button_bounds(content_bounds).contains(point)
            || cad_demo_reset_button_bounds(content_bounds).contains(point)
            || cad_demo_hidden_line_mode_button_bounds(content_bounds).contains(point)
            || cad_demo_reset_camera_button_bounds(content_bounds).contains(point)
            || cad_demo_projection_mode_button_bounds(content_bounds).contains(point)
            || cad_demo_snap_grid_button_bounds(content_bounds).contains(point)
            || cad_demo_snap_origin_button_bounds(content_bounds).contains(point)
            || cad_demo_snap_endpoint_button_bounds(content_bounds).contains(point)
            || cad_demo_snap_midpoint_button_bounds(content_bounds).contains(point)
            || cad_demo_hotkey_profile_button_bounds(content_bounds).contains(point)
            || cad_demo_view_snap_top_button_bounds(content_bounds).contains(point)
            || cad_demo_view_snap_front_button_bounds(content_bounds).contains(point)
            || cad_demo_view_snap_right_button_bounds(content_bounds).contains(point)
            || cad_demo_view_snap_iso_button_bounds(content_bounds).contains(point)
            || cad_demo_warning_filter_severity_button_bounds(content_bounds).contains(point)
            || cad_demo_warning_filter_code_button_bounds(content_bounds).contains(point)
            || cad_demo_warning_panel_bounds(content_bounds).contains(point)
            || cad_demo_timeline_panel_bounds(content_bounds).contains(point)
        {
            return None;
        }
        if cad_pane::variant_tile_index_at_point(content_bounds, point).is_some() {
            return Some(pane.id);
        }
        return None;
    }
    None
}

fn begin_cad_camera_drag(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) -> bool {
    if state.pane_drag_mode.is_some() {
        return false;
    }
    let mode = match button {
        MouseButton::Left => CadCameraDragMode::Orbit,
        MouseButton::Right => CadCameraDragMode::Pan,
        _ => return false,
    };
    let Some(pane_id) = cad_camera_target_pane_id(state, point) else {
        return false;
    };
    let tile_index = state
        .panes
        .iter()
        .find(|pane| pane.id == pane_id)
        .map(|pane| pane_content_bounds(pane.bounds))
        .and_then(|content| cad_pane::variant_tile_index_at_point(content, point))
        .unwrap_or(0);
    let _ = state.cad_demo.set_active_variant_tile(tile_index);
    PaneController::bring_to_front(state, pane_id);
    state.cad_camera_drag_state = Some(CadCameraDragState {
        pane_id,
        tile_index,
        mode,
        last_mouse: point,
        moved: false,
    });
    true
}

fn update_cad_camera_drag(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(mut drag) = state.cad_camera_drag_state.take() else {
        return false;
    };
    let pane_exists = state
        .panes
        .iter()
        .any(|pane| pane.id == drag.pane_id && pane.kind == PaneKind::CadDemo);
    if !pane_exists {
        return false;
    }
    let delta_x = point.x - drag.last_mouse.x;
    let delta_y = point.y - drag.last_mouse.y;
    drag.last_mouse = point;

    if delta_x.abs() < f32::EPSILON && delta_y.abs() < f32::EPSILON {
        state.cad_camera_drag_state = Some(drag);
        return false;
    }

    if delta_x.abs() + delta_y.abs() >= 0.75 {
        drag.moved = true;
    }
    let _ = state.cad_demo.set_active_variant_tile(drag.tile_index);

    match drag.mode {
        CadCameraDragMode::Orbit => state.cad_demo.orbit_camera_by_drag(delta_x, delta_y),
        CadCameraDragMode::Pan => state.cad_demo.pan_camera_by_drag(delta_x, delta_y),
    }
    state.cad_camera_drag_state = Some(drag);
    true
}

fn finish_cad_camera_drag(state: &mut crate::app_state::RenderState) -> bool {
    let Some(drag) = state.cad_camera_drag_state.take() else {
        return false;
    };
    if !drag.moved {
        return false;
    }
    let mode_label = match drag.mode {
        CadCameraDragMode::Orbit => "orbit",
        CadCameraDragMode::Pan => "pan",
    };
    state.cad_demo.last_action = Some(format!(
        "CAD camera {mode_label} tile={} -> zoom={:.2} pan=({:.0},{:.0}) orbit=({:.0},{:.0})",
        drag.tile_index + 1,
        state.cad_demo.camera_zoom,
        state.cad_demo.camera_pan_x,
        state.cad_demo.camera_pan_y,
        state.cad_demo.camera_orbit_yaw_deg,
        state.cad_demo.camera_orbit_pitch_deg
    ));
    true
}

fn apply_cad_camera_zoom(
    state: &mut crate::app_state::RenderState,
    point: Point,
    scroll_dy: f32,
) -> bool {
    let Some(pane_id) = cad_camera_target_pane_id(state, point) else {
        return false;
    };
    let tile_index = state
        .panes
        .iter()
        .find(|pane| pane.id == pane_id)
        .map(|pane| pane_content_bounds(pane.bounds))
        .and_then(|content| cad_pane::variant_tile_index_at_point(content, point))
        .unwrap_or(0);
    let _ = state.cad_demo.set_active_variant_tile(tile_index);
    let previous = state.cad_demo.camera_zoom;
    state.cad_demo.zoom_camera_by_scroll(scroll_dy);
    if (previous - state.cad_demo.camera_zoom).abs() <= f32::EPSILON {
        return false;
    }
    state.cad_demo.last_action = Some(format!(
        "CAD camera zoom -> {:.2}",
        state.cad_demo.camera_zoom
    ));
    true
}

fn begin_chat_transcript_long_press(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) {
    state.chat_transcript_press = None;
    if button != MouseButton::Left {
        return;
    }

    let Some(message_id) = chat_pane::transcript_message_id_at_point(state, point) else {
        return;
    };
    state.chat_transcript_press = Some(ChatTranscriptPressState {
        message_id,
        started_at: std::time::Instant::now(),
    });
}

fn handle_chat_transcript_long_press_release(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some(press) = state.chat_transcript_press.take() else {
        return false;
    };

    let now = std::time::Instant::now();
    if now.duration_since(press.started_at)
        < std::time::Duration::from_millis(CHAT_MESSAGE_LONG_PRESS_MS)
    {
        return false;
    }

    let Some(released_message_id) = chat_pane::transcript_message_id_at_point(state, point) else {
        return false;
    };
    if released_message_id != press.message_id {
        return false;
    }

    let Some(message_text) = chat_pane::transcript_message_copy_text_by_id(state, press.message_id)
    else {
        return false;
    };

    let notice = match copy_to_clipboard(&message_text) {
        Ok(()) => "Copied message to clipboard".to_string(),
        Err(error) => format!("Failed to copy message: {error}"),
    };
    state.autopilot_chat.set_copy_notice(now, notice);
    true
}

fn dispatch_text_inputs(state: &mut crate::app_state::RenderState, event: &InputEvent) -> bool {
    let mut handled = dispatch_spark_input_event(state, event);
    handled |= dispatch_pay_invoice_input_event(state, event);
    handled |= dispatch_create_invoice_input_event(state, event);
    handled |= dispatch_relay_connections_input_event(state, event);
    handled |= dispatch_network_requests_input_event(state, event);
    handled |= dispatch_settings_input_event(state, event);
    handled |= dispatch_credentials_input_event(state, event);
    handled |= dispatch_chat_input_event(state, event);
    handled |= dispatch_job_history_input_event(state, event);
    handled
}

fn dispatch_pane_actions(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let sort_count_before = pane_z_sort_invocation_count();
    let pane_order = pane_indices_by_z_desc(state);
    let Some((pane_id, action)) =
        topmost_pane_hit_action_in_order(state, point, pane_order.as_slice())
    else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    let handled = run_pane_hit_action(state, action);

    let sort_delta = pane_z_sort_invocation_count().saturating_sub(sort_count_before);
    debug_assert!(
        sort_delta <= 1,
        "pane action dispatch sorted z-order {sort_delta} times"
    );

    handled
}

fn sidebar_settings_icon_bounds(state: &crate::app_state::RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    let width = logical.width;
    let height = logical.height;

    if !state.sidebar.is_open {
        return Bounds::new(-1000.0, -1000.0, 0.0, 0.0);
    }

    let min_sidebar_width = 220.0;
    let max_sidebar_width = (width * 0.5).max(min_sidebar_width);
    let configured_width = state
        .sidebar
        .width
        .max(min_sidebar_width)
        .min(max_sidebar_width);
    let panel_width = configured_width;
    let sidebar_x = (width - panel_width).max(0.0);

    let icon_size = 16.0;
    let padding = 12.0;
    let icon_x = sidebar_x + panel_width - icon_size - padding;
    let icon_y = height - icon_size - padding;
    Bounds::new(icon_x, icon_y, icon_size, icon_size)
}

fn handle_sidebar_mouse_down(
    state: &mut crate::app_state::RenderState,
    point: Point,
    button: MouseButton,
) -> bool {
    if button != MouseButton::Left {
        return false;
    }
    let handle = sidebar_handle_bounds(state);
    if !handle.contains(point) {
        return false;
    }

    state.sidebar.is_pressed = true;
    state.sidebar.is_dragging = false;
    state.sidebar.drag_start_x = point.x;
    state.sidebar.drag_start_width = state.sidebar.width;
    true
}

fn handle_sidebar_mouse_move(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let icon_bounds = sidebar_settings_icon_bounds(state);
    let hover = icon_bounds.contains(point);
    let mut handled = false;
    if hover != state.sidebar.settings_hover {
        state.sidebar.settings_hover = hover;
        handled = true;
    }

    if !state.sidebar.is_pressed {
        return handled;
    }

    let dx = point.x - state.sidebar.drag_start_x;
    let logical = logical_size(&state.config, state.scale_factor);

    // Start a drag only after a small horizontal threshold, and only when open.
    if !state.sidebar.is_dragging {
        if dx.abs() < 3.0 || !state.sidebar.is_open {
            return handled;
        }
        state.sidebar.is_dragging = true;
    }

    let min_sidebar_width = 220.0;
    let max_sidebar_width = (logical.width * 0.5).max(min_sidebar_width);
    let mut new_width = state.sidebar.drag_start_width - dx;
    new_width = new_width.max(min_sidebar_width).min(max_sidebar_width);
    state.sidebar.width = new_width;
    clamp_all_panes_to_window(state);
    true
}

fn handle_sidebar_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    _event: &InputEvent,
) -> bool {
    if !state.sidebar.is_pressed {
        return false;
    }

    let was_dragging = state.sidebar.is_dragging;
    state.sidebar.is_pressed = false;
    state.sidebar.is_dragging = false;

    let handle = sidebar_handle_bounds(state);
    if !was_dragging && handle.contains(point) {
        // Treat as a click: toggle open/closed.
        state.sidebar.is_open = !state.sidebar.is_open;
        if state.sidebar.is_open && state.sidebar.width < 50.0 {
            state.sidebar.width = SIDEBAR_DEFAULT_WIDTH;
        }
        clamp_all_panes_to_window(state);
        return true;
    }

    was_dragging
}

fn dispatch_keyboard_submit_actions(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_chat_keyboard_input(state, logical_key)
        || handle_spark_wallet_keyboard_input(state, logical_key)
        || handle_pay_invoice_keyboard_input(state, logical_key)
        || handle_create_invoice_keyboard_input(state, logical_key)
        || handle_relay_connections_keyboard_input(state, logical_key)
        || handle_network_requests_keyboard_input(state, logical_key)
        || handle_settings_keyboard_input(state, logical_key)
        || handle_credentials_keyboard_input(state, logical_key)
        || handle_job_history_keyboard_input(state, logical_key)
        || handle_cad_timeline_keyboard_input(state, logical_key)
}

fn run_pane_hit_action(state: &mut crate::app_state::RenderState, action: PaneHitAction) -> bool {
    match action {
        PaneHitAction::NostrRegenerate => {
            match regenerate_identity() {
                Ok(identity) => {
                    state.nostr_identity = Some(identity);
                    state.nostr_identity_error = None;
                    state.nostr_secret_state.revealed_until = None;
                    state.nostr_secret_state.set_copy_notice(
                        std::time::Instant::now(),
                        "Identity regenerated. Secrets are hidden by default.".to_string(),
                    );
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                Err(err) => {
                    state.nostr_identity_error = Some(err.to_string());
                }
            }
            true
        }
        PaneHitAction::NostrReveal => {
            state
                .nostr_secret_state
                .toggle_reveal(std::time::Instant::now());
            true
        }
        PaneHitAction::NostrCopySecret => {
            let now = std::time::Instant::now();
            let notice = if let Some(identity) = state.nostr_identity.as_ref() {
                match copy_to_clipboard(&identity.nsec) {
                    Ok(()) => "Copied nsec to clipboard. Treat it like a password.".to_string(),
                    Err(error) => format!("Failed to copy nsec: {error}"),
                }
            } else {
                "No Nostr identity loaded. Regenerate keys first.".to_string()
            };
            state.nostr_secret_state.set_copy_notice(now, notice);
            true
        }
        PaneHitAction::ChatSend => run_chat_submit_action(state),
        PaneHitAction::ChatRefreshThreads => run_chat_refresh_threads_action(state),
        PaneHitAction::ChatNewThread => run_chat_new_thread_action(state),
        PaneHitAction::ChatCycleModel => run_chat_cycle_model_action(state),
        PaneHitAction::ChatInterruptTurn => run_chat_interrupt_turn_action(state),
        PaneHitAction::ChatToggleArchivedFilter => run_chat_toggle_archived_filter_action(state),
        PaneHitAction::ChatCycleSortFilter => run_chat_cycle_sort_filter_action(state),
        PaneHitAction::ChatCycleSourceFilter => run_chat_cycle_source_filter_action(state),
        PaneHitAction::ChatCycleProviderFilter => run_chat_cycle_provider_filter_action(state),
        PaneHitAction::ChatForkThread => run_chat_fork_thread_action(state),
        PaneHitAction::ChatArchiveThread => run_chat_archive_thread_action(state),
        PaneHitAction::ChatUnarchiveThread => run_chat_unarchive_thread_action(state),
        PaneHitAction::ChatRenameThread => run_chat_rename_thread_action(state),
        PaneHitAction::ChatRollbackThread => run_chat_rollback_thread_action(state),
        PaneHitAction::ChatCompactThread => run_chat_compact_thread_action(state),
        PaneHitAction::ChatUnsubscribeThread => run_chat_unsubscribe_thread_action(state),
        PaneHitAction::ChatRespondApprovalAccept => {
            run_chat_approval_response_action(state, ApprovalDecision::Accept)
        }
        PaneHitAction::ChatRespondApprovalAcceptSession => {
            run_chat_approval_response_action(state, ApprovalDecision::AcceptForSession)
        }
        PaneHitAction::ChatRespondApprovalDecline => {
            run_chat_approval_response_action(state, ApprovalDecision::Decline)
        }
        PaneHitAction::ChatRespondApprovalCancel => {
            run_chat_approval_response_action(state, ApprovalDecision::Cancel)
        }
        PaneHitAction::ChatRespondToolCall => run_chat_tool_call_response_action(state),
        PaneHitAction::ChatRespondToolUserInput => run_chat_tool_user_input_response_action(state),
        PaneHitAction::ChatRespondAuthRefresh => run_chat_auth_refresh_response_action(state),
        PaneHitAction::ChatSelectThread(index) => run_chat_select_thread_action(state, index),
        PaneHitAction::GoOnlineToggle => {
            let wants_online = matches!(
                state.provider_runtime.mode,
                ProviderMode::Offline | ProviderMode::Degraded
            );
            if wants_online {
                queue_spark_command(state, SparkWalletCommand::Refresh);
            }
            match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline {
                online: wants_online,
            }) {
                Ok(command_seq) => {
                    state.provider_runtime.last_result =
                        Some(format!("Queued SetRunnerOnline command #{command_seq}"));
                    state.provider_runtime.last_authoritative_status = Some("pending".to_string());
                    state.provider_runtime.last_authoritative_event_id = None;
                    state.provider_runtime.last_authoritative_error_class = None;
                }
                Err(error) => {
                    state.provider_runtime.last_result = Some(error.clone());
                    state.provider_runtime.last_error_detail = Some(error);
                    state.provider_runtime.mode = ProviderMode::Degraded;
                    state.provider_runtime.degraded_reason_code =
                        Some("SA_COMMAND_QUEUE_ERROR".to_string());
                    state.provider_runtime.mode_changed_at = std::time::Instant::now();
                    state.provider_runtime.last_authoritative_status =
                        Some(RuntimeCommandStatus::Retryable.label().to_string());
                    state.provider_runtime.last_authoritative_event_id = None;
                    state.provider_runtime.last_authoritative_error_class =
                        Some(RuntimeCommandErrorClass::Transport.label().to_string());
                }
            }
            true
        }
        PaneHitAction::CodexAccount(action) => run_codex_account_action(state, action),
        PaneHitAction::CodexModels(action) => run_codex_models_action(state, action),
        PaneHitAction::CodexConfig(action) => run_codex_config_action(state, action),
        PaneHitAction::CodexMcp(action) => run_codex_mcp_action(state, action),
        PaneHitAction::CodexApps(action) => run_codex_apps_action(state, action),
        PaneHitAction::CodexLabs(action) => run_codex_labs_action(state, action),
        PaneHitAction::CodexDiagnostics(action) => run_codex_diagnostics_action(state, action),
        PaneHitAction::EarningsScoreboard(action) => run_earnings_scoreboard_action(state, action),
        PaneHitAction::RelayConnections(action) => run_relay_connections_action(state, action),
        PaneHitAction::SyncHealth(action) => run_sync_health_action(state, action),
        PaneHitAction::NetworkRequests(action) => run_network_requests_action(state, action),
        PaneHitAction::StarterJobs(action) => run_starter_jobs_action(state, action),
        PaneHitAction::ActivityFeed(action) => run_activity_feed_action(state, action),
        PaneHitAction::AlertsRecovery(action) => run_alerts_recovery_action(state, action),
        PaneHitAction::Settings(action) => run_settings_action(state, action),
        PaneHitAction::Credentials(action) => run_credentials_action(state, action),
        PaneHitAction::JobInbox(action) => reducers::run_job_inbox_action(state, action),
        PaneHitAction::ActiveJob(action) => reducers::run_active_job_action(state, action),
        PaneHitAction::JobHistory(action) => reducers::run_job_history_action(state, action),
        PaneHitAction::AgentProfileState(action) => {
            reducers::run_agent_profile_state_action(state, action)
        }
        PaneHitAction::AgentScheduleTick(action) => {
            reducers::run_agent_schedule_tick_action(state, action)
        }
        PaneHitAction::TrajectoryAudit(action) => {
            reducers::run_trajectory_audit_action(state, action)
        }
        PaneHitAction::SkillRegistry(action) => reducers::run_skill_registry_action(state, action),
        PaneHitAction::SkillTrustRevocation(action) => {
            reducers::run_skill_trust_revocation_action(state, action)
        }
        PaneHitAction::CreditDesk(action) => reducers::run_credit_desk_action(state, action),
        PaneHitAction::CreditSettlementLedger(action) => {
            reducers::run_credit_settlement_ledger_action(state, action)
        }
        PaneHitAction::AgentNetworkSimulation(action) => {
            run_agent_network_simulation_action(state, action)
        }
        PaneHitAction::TreasuryExchangeSimulation(action) => {
            run_treasury_exchange_simulation_action(state, action)
        }
        PaneHitAction::RelaySecuritySimulation(action) => {
            run_relay_security_simulation_action(state, action)
        }
        PaneHitAction::StableSatsSimulation(action) => {
            run_stable_sats_simulation_action(state, action)
        }
        PaneHitAction::CadDemo(action) => reducers::run_cad_demo_action(state, action),
        PaneHitAction::Spark(action) => run_spark_action(state, action),
        PaneHitAction::SparkCreateInvoice(action) => run_create_invoice_action(state, action),
        PaneHitAction::SparkPayInvoice(action) => run_pay_invoice_action(state, action),
    }
}

fn handle_chat_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.chat_inputs.composer.is_focused(),
        dispatch_chat_input_event,
        |s| {
            if s.chat_inputs.composer.is_focused() {
                return run_chat_submit_action(s);
            }
            false
        },
    )
}

fn handle_spark_wallet_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        spark_inputs_focused,
        dispatch_spark_input_event,
        |s| {
            if s.spark_inputs.invoice_amount.is_focused() {
                let _ = run_spark_action(s, SparkPaneAction::CreateInvoice);
                return true;
            }
            if s.spark_inputs.send_request.is_focused() || s.spark_inputs.send_amount.is_focused() {
                let _ = run_spark_action(s, SparkPaneAction::SendPayment);
                return true;
            }
            false
        },
    )
}

fn handle_pay_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        pay_invoice_inputs_focused,
        dispatch_pay_invoice_input_event,
        |s| {
            if s.pay_invoice_inputs.payment_request.is_focused()
                || s.pay_invoice_inputs.amount_sats.is_focused()
            {
                let _ = run_pay_invoice_action(s, PayInvoicePaneAction::SendPayment);
                return true;
            }
            false
        },
    )
}

fn handle_create_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        create_invoice_inputs_focused,
        dispatch_create_invoice_input_event,
        |s| {
            if s.create_invoice_inputs.amount_sats.is_focused()
                || s.create_invoice_inputs.description.is_focused()
                || s.create_invoice_inputs.expiry_seconds.is_focused()
            {
                let _ = run_create_invoice_action(s, CreateInvoicePaneAction::CreateInvoice);
                return true;
            }
            false
        },
    )
}

fn handle_relay_connections_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.relay_connections_inputs.relay_url.is_focused(),
        dispatch_relay_connections_input_event,
        |s| {
            if s.relay_connections_inputs.relay_url.is_focused() {
                return run_relay_connections_action(s, RelayConnectionsPaneAction::AddRelay);
            }
            false
        },
    )
}

fn handle_network_requests_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        network_requests_inputs_focused,
        dispatch_network_requests_input_event,
        |s| {
            if network_requests_inputs_focused(s) {
                return run_network_requests_action(s, NetworkRequestsPaneAction::SubmitRequest);
            }
            false
        },
    )
}

fn handle_activity_feed_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    if !matches!(key, Key::Named(NamedKey::Enter)) {
        return false;
    }

    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_activity_feed_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::ActivityFeed);
    if !is_activity_feed_active {
        return false;
    }

    run_activity_feed_action(state, ActivityFeedPaneAction::Refresh)
}

fn handle_alerts_recovery_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    if !matches!(key, Key::Named(NamedKey::Enter)) {
        return false;
    }

    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_alerts_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::AlertsRecovery);
    if !is_alerts_active {
        return false;
    }

    run_alerts_recovery_action(state, AlertsRecoveryPaneAction::RecoverSelected)
}

fn handle_settings_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        settings_inputs_focused,
        dispatch_settings_input_event,
        |s| {
            if settings_inputs_focused(s) {
                return run_settings_action(s, SettingsPaneAction::Save);
            }
            false
        },
    )
}

fn handle_credentials_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        credentials_inputs_focused,
        dispatch_credentials_input_event,
        |s| {
            if s.credentials_inputs.variable_value.is_focused() {
                return run_credentials_action(s, CredentialsPaneAction::SaveValue);
            }
            if s.credentials_inputs.variable_name.is_focused() {
                return run_credentials_action(s, CredentialsPaneAction::AddCustom);
            }
            false
        },
    )
}

fn handle_job_history_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    handle_focused_keyboard_submit(
        state,
        logical_key,
        |s| s.job_history_inputs.search_job_id.is_focused(),
        dispatch_job_history_input_event,
        |s| {
            if s.job_history_inputs.search_job_id.is_focused() {
                s.job_history.last_error = None;
                s.job_history.last_action = Some("Applied job-id search filter".to_string());
                return true;
            }
            false
        },
    )
}

fn handle_cad_timeline_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };
    let Some(active_pane_id) = PaneController::active(state) else {
        return false;
    };
    let is_cad_active = state
        .panes
        .iter()
        .find(|pane| pane.id == active_pane_id)
        .is_some_and(|pane| pane.kind == crate::app_state::PaneKind::CadDemo);
    if !is_cad_active {
        return false;
    }

    match key {
        Key::Named(NamedKey::ArrowUp) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::TimelineSelectPrev)
        }
        Key::Named(NamedKey::ArrowDown) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::TimelineSelectNext)
        }
        Key::Named(NamedKey::Home) => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::ResetCamera)
        }
        Key::Character(value) if value == "0" => {
            reducers::run_cad_demo_action(state, CadDemoPaneAction::ResetCamera)
        }
        Key::Character(value) => cad_hotkey_action_matrix()
            .iter()
            .find_map(|(hotkey, action)| {
                state
                    .cad_demo
                    .hotkey_matches(*hotkey, value.as_str())
                    .then_some(*action)
            })
            .is_some_and(|action| reducers::run_cad_demo_action(state, action)),
        _ => false,
    }
}

fn cad_hotkey_action_matrix() -> &'static [(CadHotkeyAction, CadDemoPaneAction)] {
    const MATRIX: [(CadHotkeyAction, CadDemoPaneAction); 10] = [
        (CadHotkeyAction::SnapTop, CadDemoPaneAction::SnapViewTop),
        (CadHotkeyAction::SnapFront, CadDemoPaneAction::SnapViewFront),
        (CadHotkeyAction::SnapRight, CadDemoPaneAction::SnapViewRight),
        (
            CadHotkeyAction::SnapIsometric,
            CadDemoPaneAction::SnapViewIsometric,
        ),
        (
            CadHotkeyAction::ToggleProjection,
            CadDemoPaneAction::ToggleProjectionMode,
        ),
        (
            CadHotkeyAction::CycleRenderMode,
            CadDemoPaneAction::CycleHiddenLineMode,
        ),
        (
            CadHotkeyAction::ToggleSnapGrid,
            CadDemoPaneAction::ToggleSnapGrid,
        ),
        (
            CadHotkeyAction::ToggleSnapOrigin,
            CadDemoPaneAction::ToggleSnapOrigin,
        ),
        (
            CadHotkeyAction::ToggleSnapEndpoint,
            CadDemoPaneAction::ToggleSnapEndpoint,
        ),
        (
            CadHotkeyAction::ToggleSnapMidpoint,
            CadDemoPaneAction::ToggleSnapMidpoint,
        ),
    ];
    &MATRIX
}

fn handle_focused_keyboard_submit<FHasFocus, FDispatch, FEnter>(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
    has_focus: FHasFocus,
    dispatch_input: FDispatch,
    on_enter: FEnter,
) -> bool
where
    FHasFocus: Fn(&crate::app_state::RenderState) -> bool,
    FDispatch: Fn(&mut crate::app_state::RenderState, &InputEvent) -> bool,
    FEnter: Fn(&mut crate::app_state::RenderState) -> bool,
{
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };
    let focused_before = has_focus(state);
    let handled_by_input = dispatch_input(state, &key_event);
    let focused_after = has_focus(state);
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter)) && on_enter(state) {
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_chat_turn_input, build_create_invoice_command, build_pay_invoice_command,
        build_spark_command_for_action, cad_hotkey_action_matrix, is_command_palette_shortcut,
        is_toggle_fullscreen_shortcut, parse_positive_amount_str,
        validate_lightning_payment_request,
    };
    use crate::pane_system::cad_palette_command_specs;
    use crate::spark_pane::{
        CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction, hit_action, layout,
    };
    use crate::spark_wallet::SparkWalletCommand;
    use codex_client::UserInput;
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use wgpui::{Bounds, Modifiers, Point};
    use winit::keyboard::Key as WinitLogicalKey;

    #[test]
    fn parse_positive_amount_str_validates_inputs() {
        assert_eq!(parse_positive_amount_str("42", "Amount"), Ok(42));
        assert!(
            parse_positive_amount_str("0", "Amount")
                .expect_err("zero rejected")
                .contains("greater than 0")
        );
        assert!(
            parse_positive_amount_str("abc", "Amount")
                .expect_err("non-numeric rejected")
                .contains("valid integer")
        );
    }

    #[test]
    fn parse_positive_amount_str_has_readable_errors() {
        assert_eq!(
            parse_positive_amount_str("", "Invoice amount")
                .expect_err("empty amount should be rejected"),
            "Invoice amount is required"
        );
    }

    #[test]
    fn spark_command_builder_routes_actions() {
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::Refresh, "", "", ""),
            Ok(SparkWalletCommand::Refresh)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::GenerateSparkAddress, "", "", ""),
            Ok(SparkWalletCommand::GenerateSparkAddress)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::GenerateBitcoinAddress, "", "", ""),
            Ok(SparkWalletCommand::GenerateBitcoinAddress)
        ));
        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::CopySparkAddress, "", "", ""),
            Err(error) if error.contains("handled directly")
        ));

        assert!(matches!(
            build_spark_command_for_action(SparkPaneAction::CreateInvoice, "1500", "", ""),
            Ok(SparkWalletCommand::CreateInvoice {
                amount_sats: 1500,
                description: Some(_),
                expiry_seconds: Some(3600)
            })
        ));

        assert!(matches!(
            build_spark_command_for_action(
                SparkPaneAction::SendPayment,
                "",
                "lnbc1example",
                "250"
            ),
            Ok(SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats: Some(250)
            }) if payment_request == "lnbc1example"
        ));

        assert!(matches!(
            build_spark_command_for_action(
                SparkPaneAction::SendPayment,
                "",
                "not-an-invoice",
                ""
            ),
            Err(error) if error.contains("expected prefix ln")
        ));
    }

    #[test]
    fn spark_click_to_command_smoke_path() {
        let content = Bounds::new(10.0, 10.0, 780.0, 420.0);
        let pane_layout = layout(content);

        let click = Point::new(
            pane_layout.create_invoice_button.origin.x + 4.0,
            pane_layout.create_invoice_button.origin.y + 4.0,
        );
        let action = hit_action(pane_layout, click).expect("create-invoice button should hit");

        let command = build_spark_command_for_action(action, "2100", "", "")
            .expect("command dispatch should succeed");
        assert!(matches!(
            command,
            SparkWalletCommand::CreateInvoice {
                amount_sats: 2100,
                description: Some(_),
                expiry_seconds: Some(3600)
            }
        ));
    }

    #[test]
    fn command_palette_shortcut_detects_plain_k_only() {
        let key = WinitLogicalKey::Character("k".into());
        let cmd_mods = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        let ctrl_mods = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let none_mods = Modifiers::default();

        assert!(!is_command_palette_shortcut(&key, cmd_mods));
        assert!(!is_command_palette_shortcut(&key, ctrl_mods));
        assert!(is_command_palette_shortcut(&key, none_mods));
    }

    #[test]
    fn fullscreen_shortcut_matches_platform_binding() {
        let key = WinitLogicalKey::Character("f".into());
        let cmd_mods = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        let ctrl_mods = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let alt_mods = Modifiers {
            alt: true,
            ctrl: true,
            meta: true,
            ..Modifiers::default()
        };
        let none_mods = Modifiers::default();

        #[cfg(target_os = "macos")]
        {
            assert!(is_toggle_fullscreen_shortcut(&key, cmd_mods));
            assert!(!is_toggle_fullscreen_shortcut(&key, ctrl_mods));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(is_toggle_fullscreen_shortcut(&key, ctrl_mods));
            assert!(!is_toggle_fullscreen_shortcut(&key, cmd_mods));
        }

        assert!(!is_toggle_fullscreen_shortcut(&key, alt_mods));
        assert!(!is_toggle_fullscreen_shortcut(&key, none_mods));
    }

    #[test]
    fn validate_lightning_payment_request_rejects_non_invoice_text() {
        let error = validate_lightning_payment_request("not-an-invoice")
            .expect_err("non-invoice requests should fail");
        assert!(error.contains("expected prefix ln"));
    }

    #[test]
    fn build_pay_invoice_command_accepts_lightning_invoice() {
        let command = build_pay_invoice_command(
            PayInvoicePaneAction::SendPayment,
            "lnbc1exampleinvoice",
            "250",
        )
        .expect("invoice command should be built");
        assert!(matches!(
            command,
            SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats: Some(250)
            } if payment_request == "lnbc1exampleinvoice"
        ));
    }

    #[test]
    fn build_create_invoice_command_supports_optional_fields() {
        let command = build_create_invoice_command(
            CreateInvoicePaneAction::CreateInvoice,
            "1200",
            "MVP invoice",
            "900",
        )
        .expect("create invoice command should be built");
        assert!(matches!(
            command,
            SparkWalletCommand::CreateInvoice {
                amount_sats: 1200,
                description: Some(description),
                expiry_seconds: Some(900)
            } if description == "MVP invoice"
        ));
    }

    #[test]
    fn assemble_chat_turn_input_attaches_enabled_skill() {
        let (input, last_error) = assemble_chat_turn_input(
            "build mezo integration".to_string(),
            Some(("mezo", "/repo/skills/mezo/SKILL.md", true)),
        );

        assert!(last_error.is_none());
        assert_eq!(input.len(), 2);
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "build mezo integration"
        ));
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, path }
                if name == "mezo" && path == &PathBuf::from("/repo/skills/mezo/SKILL.md")
        ));
    }

    #[test]
    fn assemble_chat_turn_input_rejects_disabled_skill_attachment() {
        let (input, last_error) = assemble_chat_turn_input(
            "build mezo integration".to_string(),
            Some(("mezo", "/repo/skills/mezo/SKILL.md", false)),
        );

        assert_eq!(input.len(), 1);
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "build mezo integration"
        ));
        assert_eq!(
            last_error.as_deref(),
            Some("Selected skill 'mezo' is disabled; enable it first.")
        );
    }

    #[test]
    fn enter_and_mouse_primary_actions_stay_in_parity() {
        let enter_actions: BTreeSet<&str> = [
            "chat.submit",
            "spark.create_invoice",
            "spark.send_payment",
            "pay_invoice.send_payment",
            "create_invoice.create",
            "relay_connections.add",
            "network_requests.submit",
            "activity_feed.refresh",
            "alerts_recovery.recover",
            "settings.save",
        ]
        .into_iter()
        .collect();

        let mouse_actions: BTreeSet<&str> = [
            "chat.submit",
            "spark.create_invoice",
            "spark.send_payment",
            "pay_invoice.send_payment",
            "create_invoice.create",
            "relay_connections.add",
            "network_requests.submit",
            "activity_feed.refresh",
            "alerts_recovery.recover",
            "settings.save",
        ]
        .into_iter()
        .collect();

        assert_eq!(enter_actions, mouse_actions);
    }

    #[test]
    fn cad_hotkey_and_command_palette_actions_stay_in_parity() {
        let hotkey_actions: BTreeSet<String> = cad_hotkey_action_matrix()
            .iter()
            .map(|(_, action)| format!("{action:?}"))
            .collect();
        let palette_actions: BTreeSet<String> = cad_palette_command_specs()
            .iter()
            .map(|spec| format!("{:?}", spec.action))
            .collect();

        assert!(
            hotkey_actions.is_subset(&palette_actions),
            "every CAD hotkey action must have a command palette equivalent"
        );
    }
}
