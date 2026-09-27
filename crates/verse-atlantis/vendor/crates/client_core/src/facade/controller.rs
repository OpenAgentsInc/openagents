//! Read-only controller façade consumed by the renderer.

use ecs_core::components::{CameraPose, ControllerMode, InputCommand, InputProfile};

#[derive(Default, Clone, Debug)]
pub struct ControllerState {
    pub profile: InputProfile,
    pub mode: ControllerMode,
    pub camera: CameraPose,
    pub reticle_world: glam::Vec3,
    pub in_ui_capture: bool,
}

impl ControllerState {
    #[inline]
    #[must_use]
    pub fn camera_pose(&self) -> CameraPose {
        self.camera
    }
    #[inline]
    #[must_use]
    pub fn reticle_world(&self) -> glam::Vec3 {
        self.reticle_world
    }
    #[inline]
    #[must_use]
    pub fn mode(&self) -> ControllerMode {
        self.mode
    }
    #[inline]
    #[must_use]
    pub fn profile(&self) -> InputProfile {
        self.profile
    }
}

#[derive(Default, Clone, Debug)]
pub struct InputQueue {
    cmds: Vec<InputCommand>,
}

impl InputQueue {
    pub fn push(&mut self, c: InputCommand) {
        self.cmds.push(c);
    }
    pub fn drain(&mut self) -> impl Iterator<Item = InputCommand> + '_ {
        self.cmds.drain(..)
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_roundtrip() {
        let mut q = InputQueue::default();
        q.push(InputCommand::AtWillLMB);
        q.push(InputCommand::Dodge);
        let v: Vec<_> = q.drain().collect();
        assert_eq!(v.len(), 2);
        assert!(q.is_empty());
    }
}
