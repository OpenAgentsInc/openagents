//! Map button presses to `InputCommand` events when in appropriate mode/profile.

use crate::facade::controller::{ControllerState, InputQueue};
use ecs_core::components::{ControllerMode, InputCommand};
use tracing::{debug, info};

#[derive(Default, Clone, Copy, Debug)]
pub struct ButtonSnapshot {
    pub lmb_pressed: bool,
    pub rmb_pressed: bool,
    pub q_pressed: bool,
    pub e_pressed: bool,
    pub r_pressed: bool,
    pub shift_pressed: bool,
    pub tab_pressed: bool,
}

#[derive(Clone, Debug)]
pub struct Bindings {
    pub lmb: InputCommand,
    pub rmb: InputCommand,
    pub q: InputCommand,
    pub e: InputCommand,
    pub r: InputCommand,
    pub shift: InputCommand,
    pub tab: InputCommand,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            lmb: InputCommand::AtWillLMB,
            rmb: InputCommand::AtWillRMB,
            q: InputCommand::EncounterQ,
            e: InputCommand::EncounterE,
            r: InputCommand::EncounterR,
            shift: InputCommand::Dodge,
            tab: InputCommand::ClassMechanic,
        }
    }
}

pub fn handle_buttons(
    binds: &Bindings,
    state: &ControllerState,
    input: &ButtonSnapshot,
    out: &mut InputQueue,
) {
    let in_action = matches!(state.mode, ControllerMode::Mouselook);
    if !in_action {
        return;
    }
    let mut pushed = 0usize;
    if input.lmb_pressed {
        out.push(binds.lmb.clone());
        pushed += 1;
    }
    if input.rmb_pressed {
        out.push(binds.rmb.clone());
        pushed += 1;
    }
    if input.q_pressed {
        out.push(binds.q.clone());
        pushed += 1;
    }
    if input.e_pressed {
        out.push(binds.e.clone());
        pushed += 1;
    }
    if input.r_pressed {
        out.push(binds.r.clone());
        pushed += 1;
    }
    if input.shift_pressed {
        out.push(binds.shift.clone());
        pushed += 1;
    }
    if input.tab_pressed {
        out.push(binds.tab.clone());
        pushed += 1;
    }
    if pushed > 0 {
        info!(target: "controls", mode=?state.mode, profile=?state.profile, pressed=?pushed, "input commands enqueued");
        debug!(target: "controls", lmb=input.lmb_pressed, rmb=input.rmb_pressed, q=input.q_pressed, e=input.e_pressed, r=input.r_pressed, shift=input.shift_pressed, tab=input.tab_pressed, "bindings snapshot");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecs_core::InputProfile;
    #[test]
    fn pressed_buttons_emit_commands_in_mouselook() {
        let binds = Bindings::default();
        let state = ControllerState {
            profile: InputProfile::ActionCombat,
            mode: ControllerMode::Mouselook,
            ..Default::default()
        };
        let input = ButtonSnapshot {
            lmb_pressed: true,
            q_pressed: true,
            shift_pressed: true,
            ..Default::default()
        };
        let mut out = InputQueue::default();
        handle_buttons(&binds, &state, &input, &mut out);
        let cmds: Vec<_> = out.drain().collect();
        assert!(cmds.contains(&InputCommand::AtWillLMB));
        assert!(cmds.contains(&InputCommand::EncounterQ));
        assert!(cmds.contains(&InputCommand::Dodge));
    }

    #[test]
    fn rmb_emits_secondary_command() {
        let binds = Bindings::default();
        let state = ControllerState {
            mode: ControllerMode::Mouselook,
            ..Default::default()
        };
        let input = ButtonSnapshot {
            rmb_pressed: true,
            ..Default::default()
        };
        let mut out = InputQueue::default();
        handle_buttons(&binds, &state, &input, &mut out);
        let cmds: Vec<_> = out.drain().collect();
        assert_eq!(cmds, vec![InputCommand::AtWillRMB]);
    }
}
