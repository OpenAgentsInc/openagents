//! Cursor/mode toggle logic.

use crate::facade::controller::ControllerState;
use ecs_core::components::ControllerMode;
use tracing::info;

#[derive(Clone, Copy, Debug, Default)]
pub struct UiFocus {
    pub chat_open: bool,
    pub menu_open: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum CursorEvent {
    Toggle,
    MouseRight(bool),
    Hold(bool),
}

pub enum HostEvent {
    PointerLockRequest(bool),
}

pub fn handle_cursor_event(
    state: &mut ControllerState,
    ui: &UiFocus,
    ev: CursorEvent,
    out: &mut Vec<HostEvent>,
) {
    match ev {
        CursorEvent::Toggle => {
            if ui.chat_open || ui.menu_open {
                return;
            }
            let prev = state.mode;
            state.mode = match state.mode {
                ControllerMode::Mouselook => ControllerMode::Cursor,
                ControllerMode::Cursor => ControllerMode::Mouselook,
            };
            info!(target: "controls", from = ?prev, to = ?state.mode, reason = "alt_toggle");
            out.push(HostEvent::PointerLockRequest(
                state.mode == ControllerMode::Mouselook,
            ));
        }
        CursorEvent::MouseRight(down) => {
            // Classic fallback: only in ClassicCursor profile
            if state.profile == ecs_core::components::InputProfile::ClassicCursor {
                if down && state.mode == ControllerMode::Cursor {
                    state.mode = ControllerMode::Mouselook;
                    info!(target: "controls", to = ?state.mode, reason = "rmb_hold_begin");
                    out.push(HostEvent::PointerLockRequest(true));
                } else if !down && state.mode == ControllerMode::Mouselook {
                    state.mode = ControllerMode::Cursor;
                    info!(target: "controls", to = ?state.mode, reason = "rmb_hold_end");
                    out.push(HostEvent::PointerLockRequest(false));
                }
            }
        }
        CursorEvent::Hold(down) => {
            if ui.chat_open || ui.menu_open {
                return;
            }
            if down {
                state.mode = ControllerMode::Cursor;
                info!(target: "controls", to = ?state.mode, reason = "alt_hold_begin");
                out.push(HostEvent::PointerLockRequest(false));
            } else {
                state.mode = ControllerMode::Mouselook;
                info!(target: "controls", to = ?state.mode, reason = "alt_hold_end");
                out.push(HostEvent::PointerLockRequest(true));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_switches_and_requests_lock() {
        let mut s = ControllerState {
            mode: ControllerMode::Cursor,
            ..Default::default()
        };
        let mut ev = Vec::new();
        handle_cursor_event(&mut s, &UiFocus::default(), CursorEvent::Toggle, &mut ev);
        assert_eq!(s.mode, ControllerMode::Mouselook);
        assert!(!ev.is_empty());
    }
}
