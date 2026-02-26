use nostr::regenerate_identity;
use wgpui::clipboard::copy_to_clipboard;
use wgpui::{Bounds, Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};

use crate::app_state::{App, ProviderMode};
use crate::hotbar::{
    HOTBAR_SLOT_NOSTR_IDENTITY, HOTBAR_SLOT_SPARK_WALLET, activate_hotbar_slot,
    hotbar_slot_for_key, process_hotbar_clicks,
};
use crate::pane_system::{
    ActiveJobPaneAction, EarningsScoreboardPaneAction, JobInboxPaneAction, PaneController,
    PaneInput, dispatch_chat_input_event, dispatch_create_invoice_input_event,
    dispatch_job_history_input_event, dispatch_pay_invoice_input_event, dispatch_spark_input_event,
    topmost_active_job_action_hit, topmost_chat_send_hit, topmost_create_invoice_action_hit,
    topmost_earnings_scoreboard_action_hit, topmost_go_online_toggle_hit,
    topmost_job_history_action_hit, topmost_job_inbox_action_hit, topmost_nostr_copy_secret_hit,
    topmost_nostr_regenerate_hit, topmost_nostr_reveal_hit, topmost_pay_invoice_action_hit,
    topmost_spark_action_hit,
};
use crate::render::{logical_size, render_frame};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;

const COMMAND_OPEN_AUTOPILOT_CHAT: &str = "pane.autopilot_chat";
const COMMAND_OPEN_GO_ONLINE: &str = "pane.go_online";
const COMMAND_OPEN_PROVIDER_STATUS: &str = "pane.provider_status";
const COMMAND_OPEN_EARNINGS_SCOREBOARD: &str = "pane.earnings_scoreboard";
const COMMAND_OPEN_JOB_INBOX: &str = "pane.job_inbox";
const COMMAND_OPEN_ACTIVE_JOB: &str = "pane.active_job";
const COMMAND_OPEN_JOB_HISTORY: &str = "pane.job_history";
const COMMAND_OPEN_IDENTITY_KEYS: &str = "pane.identity_keys";
const COMMAND_OPEN_WALLET: &str = "pane.wallet";
const COMMAND_OPEN_PAY_INVOICE: &str = "pane.pay_invoice";
const COMMAND_OPEN_CREATE_INVOICE: &str = "pane.create_invoice";

pub fn handle_window_event(app: &mut App, event_loop: &ActiveEventLoop, event: WindowEvent) {
    let Some(state) = &mut app.state else {
        return;
    };

    if drain_spark_worker_updates(state) {
        state.window.request_redraw();
    }
    if state.nostr_secret_state.expire(std::time::Instant::now()) {
        state.window.request_redraw();
    }
    let now = std::time::Instant::now();
    let blockers = state.provider_blockers();
    if state.provider_runtime.tick(now, blockers.as_slice()) {
        state.window.request_redraw();
    }
    if state.autopilot_chat.tick(now) {
        state.window.request_redraw();
    }
    refresh_earnings_scoreboard(state, now);

    match event {
        WindowEvent::CloseRequested => {
            let _ = state.spark_worker.cancel_pending();
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

            let mut needs_redraw = false;
            if PaneController::update_drag(state, app.cursor_position) {
                needs_redraw = true;
            }

            let pane_move_event = InputEvent::MouseMove {
                x: app.cursor_position.x,
                y: app.cursor_position.y,
            };
            if PaneInput::dispatch_frame_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_spark_input_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_pay_invoice_input_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_create_invoice_input_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_chat_input_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_job_history_input_event(state, &pane_move_event) {
                needs_redraw = true;
            }

            if state
                .hotbar
                .event(
                    &pane_move_event,
                    state.hotbar_bounds,
                    &mut state.event_context,
                )
                .is_handled()
            {
                needs_redraw = true;
            }

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
                    let mut handled = false;
                    if state.hotbar_bounds.contains(app.cursor_position) {
                        handled |= state
                            .hotbar
                            .event(&input, state.hotbar_bounds, &mut state.event_context)
                            .is_handled();
                        handled |= process_hotbar_clicks(state);
                        handled |= dispatch_spark_input_event(state, &input);
                        handled |= dispatch_pay_invoice_input_event(state, &input);
                        handled |= dispatch_create_invoice_input_event(state, &input);
                        handled |= dispatch_chat_input_event(state, &input);
                        handled |= dispatch_job_history_input_event(state, &input);
                        if !handled {
                            handled |=
                                PaneInput::handle_mouse_down(state, app.cursor_position, button);
                        }
                    } else {
                        handled |= PaneInput::handle_mouse_down(state, app.cursor_position, button);
                        handled |= dispatch_spark_input_event(state, &input);
                        handled |= dispatch_pay_invoice_input_event(state, &input);
                        handled |= dispatch_create_invoice_input_event(state, &input);
                        handled |= dispatch_chat_input_event(state, &input);
                        handled |= dispatch_job_history_input_event(state, &input);
                        handled |= state
                            .hotbar
                            .event(&input, state.hotbar_bounds, &mut state.event_context)
                            .is_handled();
                        handled |= process_hotbar_clicks(state);
                    }

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
                ElementState::Released => {
                    let mut handled = PaneInput::handle_mouse_up(state, &input);
                    handled |= dispatch_spark_input_event(state, &input);
                    handled |= dispatch_pay_invoice_input_event(state, &input);
                    handled |= dispatch_create_invoice_input_event(state, &input);
                    handled |= dispatch_chat_input_event(state, &input);
                    handled |= dispatch_job_history_input_event(state, &input);
                    handled |= handle_nostr_regenerate_click(state, app.cursor_position);
                    handled |= handle_nostr_reveal_click(state, app.cursor_position);
                    handled |= handle_nostr_copy_click(state, app.cursor_position);
                    handled |= handle_spark_action_click(state, app.cursor_position);
                    handled |= handle_pay_invoice_action_click(state, app.cursor_position);
                    handled |= handle_create_invoice_action_click(state, app.cursor_position);
                    handled |= handle_chat_send_click(state, app.cursor_position);
                    handled |= handle_go_online_toggle_click(state, app.cursor_position);
                    handled |= handle_earnings_scoreboard_action_click(state, app.cursor_position);
                    handled |= handle_job_inbox_action_click(state, app.cursor_position);
                    handled |= handle_active_job_action_click(state, app.cursor_position);
                    handled |= handle_job_history_action_click(state, app.cursor_position);
                    handled |= state
                        .hotbar
                        .event(&input, state.hotbar_bounds, &mut state.event_context)
                        .is_handled();
                    handled |= process_hotbar_clicks(state);

                    state
                        .window
                        .set_cursor(PaneInput::cursor_icon(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed
                && is_command_palette_shortcut(&event.logical_key, state.input_modifiers)
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

            if handle_chat_keyboard_input(state, &event.logical_key)
                || handle_spark_wallet_keyboard_input(state, &event.logical_key)
                || handle_pay_invoice_keyboard_input(state, &event.logical_key)
                || handle_create_invoice_keyboard_input(state, &event.logical_key)
                || handle_job_history_keyboard_input(state, &event.logical_key)
            {
                state.window.request_redraw();
                return;
            }

            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(pane_id) = PaneController::active(state) {
                        PaneController::close(state, pane_id);
                        state.window.request_redraw();
                    }
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

fn handle_nostr_regenerate_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(pane_id) = topmost_nostr_regenerate_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    match regenerate_identity() {
        Ok(identity) => {
            state.nostr_identity = Some(identity);
            state.nostr_identity_error = None;
        }
        Err(err) => {
            state.nostr_identity_error = Some(err.to_string());
        }
    }
    true
}

fn handle_nostr_reveal_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(pane_id) = topmost_nostr_reveal_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    state
        .nostr_secret_state
        .toggle_reveal(std::time::Instant::now());
    true
}

fn handle_nostr_copy_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(pane_id) = topmost_nostr_copy_secret_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
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

fn handle_spark_action_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some((pane_id, action)) = topmost_spark_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_spark_action(state, action)
}

fn handle_pay_invoice_action_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some((pane_id, action)) = topmost_pay_invoice_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_pay_invoice_action(state, action)
}

fn handle_create_invoice_action_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some((pane_id, action)) = topmost_create_invoice_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_create_invoice_action(state, action)
}

fn handle_chat_send_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(pane_id) = topmost_chat_send_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_chat_submit_action(state)
}

fn handle_go_online_toggle_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some(pane_id) = topmost_go_online_toggle_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    if state.provider_runtime.mode == ProviderMode::Offline {
        queue_spark_command(state, SparkWalletCommand::Refresh);
    }
    let blockers = state.provider_blockers();
    state
        .provider_runtime
        .toggle_online(std::time::Instant::now(), blockers.as_slice());
    true
}

fn handle_earnings_scoreboard_action_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some((pane_id, action)) = topmost_earnings_scoreboard_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_earnings_scoreboard_action(state, action)
}

fn handle_job_inbox_action_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some((pane_id, action)) = topmost_job_inbox_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_job_inbox_action(state, action)
}

fn handle_active_job_action_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some((pane_id, action)) = topmost_active_job_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_active_job_action(state, action)
}

fn handle_job_history_action_click(
    state: &mut crate::app_state::RenderState,
    point: Point,
) -> bool {
    let Some((pane_id, action)) = topmost_job_history_action_hit(state, point) else {
        return false;
    };

    PaneController::bring_to_front(state, pane_id);
    run_job_history_action(state, action)
}

fn handle_chat_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };
    let focused_before = state.chat_inputs.composer.is_focused();
    let handled_by_input = dispatch_chat_input_event(state, &key_event);
    let focused_after = state.chat_inputs.composer.is_focused();
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter)) && state.chat_inputs.composer.is_focused() {
        return run_chat_submit_action(state);
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

fn handle_spark_wallet_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };

    let focused_before = spark_inputs_focused(state);
    let handled_by_input = dispatch_spark_input_event(state, &key_event);
    let focused_after = spark_inputs_focused(state);
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter))
        && (state.spark_inputs.invoice_amount.is_focused()
            || state.spark_inputs.send_request.is_focused()
            || state.spark_inputs.send_amount.is_focused())
    {
        if state.spark_inputs.invoice_amount.is_focused() {
            let _ = run_spark_action(state, SparkPaneAction::CreateInvoice);
        } else {
            let _ = run_spark_action(state, SparkPaneAction::SendPayment);
        }
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

fn handle_pay_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };

    let focused_before = pay_invoice_inputs_focused(state);
    let handled_by_input = dispatch_pay_invoice_input_event(state, &key_event);
    let focused_after = pay_invoice_inputs_focused(state);
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter))
        && (state.pay_invoice_inputs.payment_request.is_focused()
            || state.pay_invoice_inputs.amount_sats.is_focused())
    {
        let _ = run_pay_invoice_action(state, PayInvoicePaneAction::SendPayment);
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

fn handle_create_invoice_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };

    let focused_before = create_invoice_inputs_focused(state);
    let handled_by_input = dispatch_create_invoice_input_event(state, &key_event);
    let focused_after = create_invoice_inputs_focused(state);
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter))
        && (state.create_invoice_inputs.amount_sats.is_focused()
            || state.create_invoice_inputs.description.is_focused()
            || state.create_invoice_inputs.expiry_seconds.is_focused())
    {
        let _ = run_create_invoice_action(state, CreateInvoicePaneAction::CreateInvoice);
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

fn handle_job_history_keyboard_input(
    state: &mut crate::app_state::RenderState,
    logical_key: &WinitLogicalKey,
) -> bool {
    let Some(key) = map_winit_key(logical_key) else {
        return false;
    };

    let key_event = InputEvent::KeyDown {
        key: key.clone(),
        modifiers: state.input_modifiers,
    };

    let focused_before = state.job_history_inputs.search_job_id.is_focused();
    let handled_by_input = dispatch_job_history_input_event(state, &key_event);
    let focused_after = state.job_history_inputs.search_job_id.is_focused();
    let focus_active = focused_before || focused_after;

    if matches!(key, Key::Named(NamedKey::Enter))
        && state.job_history_inputs.search_job_id.is_focused()
    {
        state.job_history.last_error = None;
        state.job_history.last_action = Some("Applied job-id search filter".to_string());
        return true;
    }

    if focus_active {
        return handled_by_input;
    }

    false
}

fn run_chat_submit_action(state: &mut crate::app_state::RenderState) -> bool {
    let prompt = state.chat_inputs.composer.get_value().trim().to_string();
    if prompt.is_empty() {
        state.autopilot_chat.last_error = Some("Prompt cannot be empty".to_string());
        return true;
    }

    state.chat_inputs.composer.set_value(String::new());
    state
        .autopilot_chat
        .submit_prompt(std::time::Instant::now(), prompt);
    true
}

fn run_earnings_scoreboard_action(
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

fn run_job_inbox_action(
    state: &mut crate::app_state::RenderState,
    action: JobInboxPaneAction,
) -> bool {
    match action {
        JobInboxPaneAction::SelectRow(index) => {
            if !state.job_inbox.select_by_index(index) {
                state.job_inbox.last_error = Some("Request row out of range".to_string());
                state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
            } else {
                state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
            }
            true
        }
        JobInboxPaneAction::AcceptSelected => {
            match state
                .job_inbox
                .decide_selected(true, "validated + queued for runtime")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_add(1);
                    state.provider_runtime.last_result =
                        Some(format!("runtime accepted request {request_id}"));
                    let selected_request = state
                        .job_inbox
                        .requests
                        .iter()
                        .find(|request| request.request_id == request_id)
                        .cloned();
                    if let Some(request) = selected_request.as_ref() {
                        state.active_job.start_from_request(request);
                        PaneController::create_active_job(state);
                    }
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
        JobInboxPaneAction::RejectSelected => {
            match state
                .job_inbox
                .decide_selected(false, "failed policy preflight")
            {
                Ok(request_id) => {
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Ready;
                    state.provider_runtime.last_result =
                        Some(format!("runtime rejected request {request_id}"));
                }
                Err(error) => {
                    state.job_inbox.last_error = Some(error);
                    state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
                }
            }
            true
        }
    }
}

fn run_active_job_action(
    state: &mut crate::app_state::RenderState,
    action: ActiveJobPaneAction,
) -> bool {
    let now = std::time::Instant::now();
    match action {
        ActiveJobPaneAction::AdvanceStage => {
            if let Ok(stage) = state.active_job.advance_stage() {
                state.provider_runtime.last_result =
                    Some(format!("active job advanced to {}", stage.label()));
                if stage == crate::app_state::JobLifecycleStage::Paid {
                    state.provider_runtime.queue_depth =
                        state.provider_runtime.queue_depth.saturating_sub(1);
                    state.provider_runtime.last_completed_job_at = Some(now);
                    if let Some(job) = state.active_job.job.as_ref() {
                        state.job_history.record_from_active_job(
                            job,
                            crate::app_state::JobHistoryStatus::Succeeded,
                        );
                    }
                }
            }
            refresh_earnings_scoreboard(state, now);
            true
        }
        ActiveJobPaneAction::AbortJob => {
            if state
                .active_job
                .abort_job("operator requested abort")
                .is_ok()
            {
                state.provider_runtime.last_result = Some("active job aborted".to_string());
                state.provider_runtime.queue_depth =
                    state.provider_runtime.queue_depth.saturating_sub(1);
                state.provider_runtime.last_completed_job_at = Some(now);
                if let Some(job) = state.active_job.job.as_ref() {
                    state
                        .job_history
                        .record_from_active_job(job, crate::app_state::JobHistoryStatus::Failed);
                }
            }
            refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

fn run_job_history_action(
    state: &mut crate::app_state::RenderState,
    action: crate::pane_system::JobHistoryPaneAction,
) -> bool {
    let now = std::time::Instant::now();
    match action {
        crate::pane_system::JobHistoryPaneAction::CycleStatusFilter => {
            state.job_history.cycle_status_filter();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::CycleTimeRange => {
            state.job_history.cycle_time_range();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::PreviousPage => {
            state.job_history.previous_page();
            refresh_earnings_scoreboard(state, now);
            true
        }
        crate::pane_system::JobHistoryPaneAction::NextPage => {
            state.job_history.next_page();
            refresh_earnings_scoreboard(state, now);
            true
        }
    }
}

fn refresh_earnings_scoreboard(state: &mut crate::app_state::RenderState, now: std::time::Instant) {
    state.earnings_scoreboard.refresh_from_sources(
        now,
        &state.provider_runtime,
        &state.job_history,
        &state.spark_wallet,
    );
}

fn run_spark_action(state: &mut crate::app_state::RenderState, action: SparkPaneAction) -> bool {
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

fn run_pay_invoice_action(
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

fn run_create_invoice_action(
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

fn build_spark_command_for_action(
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
            let request = send_request.trim().to_string();
            if request.is_empty() {
                return Err("Payment request cannot be empty".to_string());
            }

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

fn build_pay_invoice_command(
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

fn build_create_invoice_command(
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

fn validate_lightning_payment_request(raw: &str) -> Result<String, String> {
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

fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_optional_positive_amount_str(raw: &str, label: &str) -> Result<Option<u64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_positive_amount_str(trimmed, label).map(Some)
}

fn queue_spark_command(state: &mut crate::app_state::RenderState, command: SparkWalletCommand) {
    state.spark_wallet.last_error = None;
    if let Err(error) = state.spark_worker.enqueue(command) {
        state.spark_wallet.last_error = Some(error);
    }
}

fn drain_spark_worker_updates(state: &mut crate::app_state::RenderState) -> bool {
    let previous_invoice = state.spark_wallet.last_invoice.clone();
    if !state.spark_worker.drain_updates(&mut state.spark_wallet) {
        return false;
    }

    if state.spark_wallet.last_invoice != previous_invoice
        && state
            .spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created invoice"))
        && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
    {
        let invoice = invoice.to_string();
        state.spark_inputs.send_request.set_value(invoice.clone());
        state.pay_invoice_inputs.payment_request.set_value(invoice);
    }

    refresh_earnings_scoreboard(state, std::time::Instant::now());
    true
}

fn parse_positive_amount_str(raw: &str, label: &str) -> Result<u64, String> {
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
        match action.as_str() {
            COMMAND_OPEN_AUTOPILOT_CHAT => {
                PaneController::create_autopilot_chat(state);
                changed = true;
            }
            COMMAND_OPEN_GO_ONLINE => {
                PaneController::create_go_online(state);
                changed = true;
            }
            COMMAND_OPEN_PROVIDER_STATUS => {
                PaneController::create_provider_status(state);
                changed = true;
            }
            COMMAND_OPEN_EARNINGS_SCOREBOARD => {
                PaneController::create_earnings_scoreboard(state);
                refresh_earnings_scoreboard(state, std::time::Instant::now());
                changed = true;
            }
            COMMAND_OPEN_JOB_INBOX => {
                PaneController::create_job_inbox(state);
                changed = true;
            }
            COMMAND_OPEN_ACTIVE_JOB => {
                PaneController::create_active_job(state);
                changed = true;
            }
            COMMAND_OPEN_JOB_HISTORY => {
                PaneController::create_job_history(state);
                changed = true;
            }
            COMMAND_OPEN_IDENTITY_KEYS => {
                activate_hotbar_slot(state, HOTBAR_SLOT_NOSTR_IDENTITY);
                changed = true;
            }
            COMMAND_OPEN_WALLET => {
                activate_hotbar_slot(state, HOTBAR_SLOT_SPARK_WALLET);
                changed = true;
            }
            COMMAND_OPEN_PAY_INVOICE => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkPayInvoice);
                PaneController::create_pay_invoice(state);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            COMMAND_OPEN_CREATE_INVOICE => {
                let was_open = state
                    .panes
                    .iter()
                    .any(|pane| pane.kind == crate::app_state::PaneKind::SparkCreateInvoice);
                PaneController::create_create_invoice(state);
                if !was_open {
                    queue_spark_command(state, SparkWalletCommand::Refresh);
                }
                changed = true;
            }
            _ => {}
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_invoice_command, build_pay_invoice_command, build_spark_command_for_action,
        is_command_palette_shortcut, parse_positive_amount_str, validate_lightning_payment_request,
    };
    use crate::spark_pane::{
        CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction, hit_action, layout,
    };
    use crate::spark_wallet::SparkWalletCommand;
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
}
