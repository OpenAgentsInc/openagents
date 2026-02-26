use nostr::regenerate_identity;
use wgpui::{Component, InputEvent, Modifiers, MouseButton, Point};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app_state::App;
use crate::hotbar::{activate_hotbar_slot, hotbar_slot_for_key, process_hotbar_clicks};
use crate::pane_system::{
    active_pane_id, bring_pane_to_front_by_id, close_pane, cursor_icon_for_pointer,
    dispatch_pane_frame_event, handle_pane_mouse_down, handle_pane_mouse_up,
    topmost_nostr_regenerate_hit, update_drag,
};
use crate::render::render_frame;

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
                    modifiers: Modifiers::default(),
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
                        if !handled {
                            handled |= handle_pane_mouse_down(state, app.cursor_position, button);
                        }
                    } else {
                        handled |= handle_pane_mouse_down(state, app.cursor_position, button);
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
                    handled |= handle_nostr_regenerate_click(state, app.cursor_position);
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
