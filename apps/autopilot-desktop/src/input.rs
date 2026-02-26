use nostr::regenerate_identity;
use wgpui::{Component, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{
    Key as WinitLogicalKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey,
};

use crate::app_state::App;
use crate::hotbar::{activate_hotbar_slot, hotbar_slot_for_key, process_hotbar_clicks};
use crate::pane_system::{
    active_pane_id, bring_pane_to_front_by_id, close_pane, cursor_icon_for_pointer,
    dispatch_pane_frame_event, dispatch_spark_input_event, handle_pane_mouse_down,
    handle_pane_mouse_up, topmost_nostr_regenerate_hit, topmost_spark_action_hit, update_drag,
};
use crate::render::render_frame;
use crate::spark_pane::SparkPaneAction;

pub fn handle_window_event(app: &mut App, event_loop: &ActiveEventLoop, event: WindowEvent) {
    let Some(state) = &mut app.state else {
        return;
    };

    match event {
        WindowEvent::CloseRequested => event_loop.exit(),
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

            let mut needs_redraw = false;
            if update_drag(state, app.cursor_position) {
                needs_redraw = true;
            }

            let pane_move_event = InputEvent::MouseMove {
                x: app.cursor_position.x,
                y: app.cursor_position.y,
            };
            if dispatch_pane_frame_event(state, &pane_move_event) {
                needs_redraw = true;
            }
            if dispatch_spark_input_event(state, &pane_move_event) {
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
                .set_cursor(cursor_icon_for_pointer(state, app.cursor_position));

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
                        if !handled {
                            handled |= handle_pane_mouse_down(state, app.cursor_position, button);
                        }
                    } else {
                        handled |= handle_pane_mouse_down(state, app.cursor_position, button);
                        handled |= dispatch_spark_input_event(state, &input);
                        handled |= state
                            .hotbar
                            .event(&input, state.hotbar_bounds, &mut state.event_context)
                            .is_handled();
                        handled |= process_hotbar_clicks(state);
                    }

                    state
                        .window
                        .set_cursor(cursor_icon_for_pointer(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
                ElementState::Released => {
                    let mut handled = handle_pane_mouse_up(state, &input);
                    handled |= dispatch_spark_input_event(state, &input);
                    handled |= handle_nostr_regenerate_click(state, app.cursor_position);
                    handled |= handle_spark_action_click(state, app.cursor_position);
                    handled |= state
                        .hotbar
                        .event(&input, state.hotbar_bounds, &mut state.event_context)
                        .is_handled();
                    handled |= process_hotbar_clicks(state);

                    state
                        .window
                        .set_cursor(cursor_icon_for_pointer(state, app.cursor_position));
                    if handled {
                        state.window.request_redraw();
                    }
                }
            }
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }

            if handle_spark_keyboard_input(state, &event.logical_key) {
                state.window.request_redraw();
                return;
            }

            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(pane_id) = active_pane_id(state) {
                        close_pane(state, pane_id);
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
            if flashing_now || state.hotbar_flash_was_active {
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

    bring_pane_to_front_by_id(state, pane_id);
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

fn handle_spark_action_click(state: &mut crate::app_state::RenderState, point: Point) -> bool {
    let Some((pane_id, action)) = topmost_spark_action_hit(state, point) else {
        return false;
    };

    bring_pane_to_front_by_id(state, pane_id);
    run_spark_action(state, action)
}

fn handle_spark_keyboard_input(
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

fn run_spark_action(state: &mut crate::app_state::RenderState, action: SparkPaneAction) -> bool {
    match action {
        SparkPaneAction::Refresh => {
            state.spark_wallet.refresh(&state.async_runtime);
        }
        SparkPaneAction::GenerateSparkAddress => {
            state
                .spark_wallet
                .request_spark_address(&state.async_runtime);
        }
        SparkPaneAction::GenerateBitcoinAddress => {
            state
                .spark_wallet
                .request_bitcoin_address(&state.async_runtime);
        }
        SparkPaneAction::CreateInvoice => {
            let amount = parse_positive_amount(
                state.spark_inputs.invoice_amount.get_value(),
                "Invoice amount",
                &mut state.spark_wallet.last_error,
            );
            let Some(amount) = amount else {
                return true;
            };
            if let Some(invoice) = state
                .spark_wallet
                .create_invoice(&state.async_runtime, amount)
            {
                state.spark_inputs.send_request.set_value(invoice);
            }
        }
        SparkPaneAction::SendPayment => {
            let request = state
                .spark_inputs
                .send_request
                .get_value()
                .trim()
                .to_string();
            if request.is_empty() {
                state.spark_wallet.last_error = Some("Payment request cannot be empty".to_string());
                return true;
            }

            let amount_text = state.spark_inputs.send_amount.get_value().trim();
            let amount = if amount_text.is_empty() {
                None
            } else {
                let parsed = parse_positive_amount(
                    amount_text,
                    "Send amount",
                    &mut state.spark_wallet.last_error,
                );
                let Some(value) = parsed else {
                    return true;
                };
                Some(value)
            };

            let _ = state
                .spark_wallet
                .send_payment(&state.async_runtime, &request, amount);
        }
    }
    true
}

fn parse_positive_amount(raw: &str, label: &str, error_slot: &mut Option<String>) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        *error_slot = Some(format!("{label} is required"));
        return None;
    }

    match trimmed.parse::<u64>() {
        Ok(value) if value > 0 => Some(value),
        Ok(_) => {
            *error_slot = Some(format!("{label} must be greater than 0"));
            None
        }
        Err(error) => {
            *error_slot = Some(format!("{label} must be a valid integer: {error}"));
            None
        }
    }
}

fn spark_inputs_focused(state: &crate::app_state::RenderState) -> bool {
    state.spark_inputs.invoice_amount.is_focused()
        || state.spark_inputs.send_request.is_focused()
        || state.spark_inputs.send_amount.is_focused()
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
