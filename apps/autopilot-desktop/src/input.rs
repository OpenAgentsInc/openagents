use codex_client::{
    ApprovalDecision, AppsListParams, ChatgptAuthTokensRefreshResponse,
    CollaborationModeListParams, CommandExecParams, CommandExecutionRequestApprovalResponse,
    ConfigBatchWriteParams, ConfigEdit, ConfigReadParams, ConfigValueWriteParams,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse, ExperimentalFeatureListParams,
    ExternalAgentConfigDetectParams, ExternalAgentConfigImportParams,
    FileChangeRequestApprovalResponse, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, GetAccountParams,
    HazelnutScope, ListMcpServerStatusParams, LoginAccountParams, McpServerOauthLoginParams,
    MergeStrategy, ModelListParams, ProductSurface, ReviewDelivery, ReviewStartParams,
    ReviewTarget, SkillsRemoteReadParams, SkillsRemoteWriteParams, ThreadArchiveParams,
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
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};
use winit::window::Fullscreen;

use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, AlertDomain, App, ChatTranscriptPressState,
    JobInboxNetworkRequest, JobInboxValidation, NetworkRequestSubmission, ProviderMode,
};
use crate::hotbar::{
    HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET, activate_hotbar_slot,
    hotbar_slot_for_key, process_hotbar_clicks,
};
use crate::pane_registry::pane_spec_by_command_id;
use crate::pane_system::{
    ActivityFeedPaneAction, AgentNetworkSimulationPaneAction, AlertsRecoveryPaneAction,
    CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction, CodexDiagnosticsPaneAction,
    CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction, CodexRemoteSkillsPaneAction,
    CredentialsPaneAction, EarningsScoreboardPaneAction, NetworkRequestsPaneAction, PaneController,
    PaneHitAction, PaneInput, RelayConnectionsPaneAction, RelaySecuritySimulationPaneAction,
    SIDEBAR_DEFAULT_WIDTH, SettingsPaneAction, StarterJobsPaneAction, SyncHealthPaneAction,
    TreasuryExchangeSimulationPaneAction, clamp_all_panes_to_window, dispatch_chat_input_event,
    dispatch_chat_scroll_event, dispatch_create_invoice_input_event,
    dispatch_credentials_input_event, dispatch_job_history_input_event,
    dispatch_network_requests_input_event, dispatch_pay_invoice_input_event,
    dispatch_relay_connections_input_event, dispatch_settings_input_event,
    dispatch_spark_input_event, pane_indices_by_z_desc, pane_z_sort_invocation_count,
    topmost_pane_hit_action_in_order,
};
use crate::panes::chat as chat_pane;
use crate::render::{
    logical_size, render_frame, sidebar_go_online_button_bounds, sidebar_handle_bounds,
};
use crate::runtime_lanes::{
    AcCreditCommand, RuntimeCommandErrorClass, RuntimeCommandResponse, RuntimeCommandStatus,
    RuntimeLane, SaLifecycleCommand, SklDiscoveryTrustCommand,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;

mod actions;
mod reducers;
use actions::*;

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
        if go_online_bounds.size.width > 0.0 && go_online_bounds.contains(point) {
            if run_pane_hit_action(state, PaneHitAction::GoOnlineToggle) {
                return true;
            }
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

    handled
}

fn dispatch_mouse_up(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = handle_chat_transcript_long_press_release(state, point);
    handled |= handle_sidebar_mouse_up(state, point, event);
    handled |= PaneInput::handle_mouse_up(state, event);
    handled |= dispatch_text_inputs(state, event);
    handled |= dispatch_pane_actions(state, point);
    handled |= state
        .hotbar
        .event(event, state.hotbar_bounds, &mut state.event_context)
        .is_handled();
    handled |= process_hotbar_clicks(state);
    handled
}

fn dispatch_mouse_scroll(
    state: &mut crate::app_state::RenderState,
    point: Point,
    event: &InputEvent,
) -> bool {
    let mut handled = false;
    if let InputEvent::Scroll { dy, .. } = event {
        handled |= dispatch_chat_scroll_event(state, point, *dy);
    }
    handled |= dispatch_text_inputs(state, event);
    handled |= PaneInput::dispatch_frame_event(state, event);
    handled
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
        PaneHitAction::CodexRemoteSkills(action) => run_codex_remote_skills_action(state, action),
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

fn spark_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.spark_inputs.invoice_amount.is_focused()
        || state.spark_inputs.send_request.is_focused()
        || state.spark_inputs.send_amount.is_focused()
}

fn pay_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.pay_invoice_inputs.payment_request.is_focused()
        || state.pay_invoice_inputs.amount_sats.is_focused()
}

fn create_invoice_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.create_invoice_inputs.amount_sats.is_focused()
        || state.create_invoice_inputs.description.is_focused()
        || state.create_invoice_inputs.expiry_seconds.is_focused()
}

fn network_requests_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.network_requests_inputs.request_type.is_focused()
        || state.network_requests_inputs.payload.is_focused()
        || state.network_requests_inputs.skill_scope_id.is_focused()
        || state
            .network_requests_inputs
            .credit_envelope_ref
            .is_focused()
        || state.network_requests_inputs.budget_sats.is_focused()
        || state.network_requests_inputs.timeout_seconds.is_focused()
}

fn settings_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.settings_inputs.relay_url.is_focused()
        || state.settings_inputs.wallet_default_send_sats.is_focused()
        || state.settings_inputs.provider_max_queue_depth.is_focused()
}

fn credentials_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.credentials_inputs.variable_name.is_focused()
        || state.credentials_inputs.variable_value.is_focused()
}

fn any_text_input_focused(state: &crate::app_state::RenderState) -> bool {
    state.chat_inputs.composer.is_focused()
        || spark_inputs_focused(state)
        || pay_invoice_inputs_focused(state)
        || create_invoice_inputs_focused(state)
        || network_requests_inputs_focused(state)
        || settings_inputs_focused(state)
        || credentials_inputs_focused(state)
        || state.relay_connections_inputs.relay_url.is_focused()
        || state.job_history_inputs.search_job_id.is_focused()
}

fn blur_non_chat_text_inputs(state: &mut crate::app_state::RenderState) {
    state.spark_inputs.invoice_amount.blur();
    state.spark_inputs.send_request.blur();
    state.spark_inputs.send_amount.blur();
    state.pay_invoice_inputs.payment_request.blur();
    state.pay_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.amount_sats.blur();
    state.create_invoice_inputs.description.blur();
    state.create_invoice_inputs.expiry_seconds.blur();
    state.relay_connections_inputs.relay_url.blur();
    state.network_requests_inputs.request_type.blur();
    state.network_requests_inputs.payload.blur();
    state.network_requests_inputs.skill_scope_id.blur();
    state.network_requests_inputs.credit_envelope_ref.blur();
    state.network_requests_inputs.budget_sats.blur();
    state.network_requests_inputs.timeout_seconds.blur();
    state.settings_inputs.relay_url.blur();
    state.settings_inputs.wallet_default_send_sats.blur();
    state.settings_inputs.provider_max_queue_depth.blur();
    state.credentials_inputs.variable_name.blur();
    state.credentials_inputs.variable_value.blur();
    state.job_history_inputs.search_job_id.blur();
}

fn focus_chat_composer(state: &mut crate::app_state::RenderState) {
    blur_non_chat_text_inputs(state);
    state.chat_inputs.composer.focus();
}

fn map_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

fn map_winit_key(logical_key: &WinitLogicalKey) -> Option<Key> {
    match logical_key {
        WinitLogicalKey::Character(text) => Some(Key::Character(text.to_string())),
        WinitLogicalKey::Named(named) => Some(Key::Named(map_named_key(*named))),
        _ => None,
    }
}

fn map_named_key(named: WinitNamedKey) -> NamedKey {
    match named {
        WinitNamedKey::Enter => NamedKey::Enter,
        WinitNamedKey::Escape => NamedKey::Escape,
        WinitNamedKey::Backspace => NamedKey::Backspace,
        WinitNamedKey::Delete => NamedKey::Delete,
        WinitNamedKey::Tab => NamedKey::Tab,
        WinitNamedKey::Space => NamedKey::Space,
        WinitNamedKey::Home => NamedKey::Home,
        WinitNamedKey::End => NamedKey::End,
        WinitNamedKey::PageUp => NamedKey::PageUp,
        WinitNamedKey::PageDown => NamedKey::PageDown,
        WinitNamedKey::ArrowUp => NamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => NamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => NamedKey::ArrowRight,
        _ => NamedKey::Unidentified,
    }
}

fn toggle_command_palette(state: &mut crate::app_state::RenderState) {
    if state.command_palette.is_open() {
        state.command_palette.close();
    } else {
        state.command_palette.open();
    }
}

fn toggle_window_fullscreen(state: &mut crate::app_state::RenderState) {
    let next_state = if state.window.fullscreen().is_some() {
        None
    } else {
        Some(Fullscreen::Borderless(None))
    };
    state.window.set_fullscreen(next_state);
}

fn command_palette_bounds(state: &crate::app_state::RenderState) -> Bounds {
    let logical = logical_size(&state.config, state.scale_factor);
    Bounds::new(0.0, 0.0, logical.width, logical.height)
}

fn is_command_palette_shortcut(logical_key: &WinitLogicalKey, modifiers: Modifiers) -> bool {
    let is_k = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("k"),
        _ => false,
    };

    is_k && !modifiers.meta && !modifiers.ctrl && !modifiers.alt
}

fn is_toggle_fullscreen_shortcut(logical_key: &WinitLogicalKey, modifiers: Modifiers) -> bool {
    let is_f = match logical_key {
        WinitLogicalKey::Character(value) => value.eq_ignore_ascii_case("f"),
        _ => false,
    };
    if !is_f || modifiers.alt {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        modifiers.meta && !modifiers.ctrl
    }

    #[cfg(not(target_os = "macos"))]
    {
        modifiers.ctrl && !modifiers.meta
    }
}

fn dispatch_command_palette_actions(state: &mut crate::app_state::RenderState) -> bool {
    let action_ids: Vec<String> = {
        let mut queue = state.command_palette_actions.borrow_mut();
        queue.drain(..).collect()
    };
    if action_ids.is_empty() {
        return false;
    }

    let mut changed = false;
    for action in action_ids {
        let Some(spec) = pane_spec_by_command_id(&action) else {
            continue;
        };

        match spec.kind {
            crate::app_state::PaneKind::EarningsScoreboard => {
                let _ = PaneController::create_for_kind(state, spec.kind);
                refresh_earnings_scoreboard(state, std::time::Instant::now());
                changed = true;
            }
            crate::app_state::PaneKind::SyncHealth => {
                let _ = PaneController::create_for_kind(state, spec.kind);
                refresh_sync_health(state);
                changed = true;
            }
            crate::app_state::PaneKind::ActivityFeed => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::ActivityFeed);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    state
                        .activity_feed
                        .record_refresh(build_activity_feed_snapshot_events(state));
                }
                changed = true;
            }
            crate::app_state::PaneKind::AlertsRecovery => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::AlertsRecovery);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    state.alerts_recovery.last_error = None;
                    state.alerts_recovery.load_state = crate::app_state::PaneLoadState::Ready;
                    state.alerts_recovery.last_action =
                        Some("Alerts lane opened for active incident triage".to_string());
                }
                changed = true;
            }
            crate::app_state::PaneKind::NostrIdentity => {
                activate_hotbar_slot(state, HOTBAR_SLOT_NOSTR_IDENTITY);
                changed = true;
            }
            crate::app_state::PaneKind::SparkWallet => {
                activate_hotbar_slot(state, HOTBAR_SLOT_SPARK_WALLET);
                changed = true;
            }
            crate::app_state::PaneKind::SparkPayInvoice => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkPayInvoice);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            crate::app_state::PaneKind::SparkCreateInvoice => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkCreateInvoice);
                let _ = PaneController::create_for_kind(state, spec.kind);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            kind => {
                let _ = PaneController::create_for_kind(state, kind);
                changed = true;
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_chat_turn_input, build_create_invoice_command, build_pay_invoice_command,
        build_spark_command_for_action, is_command_palette_shortcut, is_toggle_fullscreen_shortcut,
        parse_positive_amount_str, validate_lightning_payment_request,
    };
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
}
