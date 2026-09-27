//! Minimal ECS scaffolding for scene organization.

use glam::{Mat4, Quat, Vec3};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity(u32);

#[derive(Copy, Clone, Debug)]
pub enum RenderKind {
    Wizard,
    Ruins,
}

#[derive(Copy, Clone, Debug)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

pub struct World {
    next_id: u32,
    pub ids: Vec<Entity>,
    pub transforms: Vec<Transform>,
    pub kinds: Vec<RenderKind>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ids: Vec::new(),
            transforms: Vec::new(),
            kinds: Vec::new(),
        }
    }
    pub fn spawn(&mut self, t: Transform, k: RenderKind) -> Entity {
        let e = Entity(self.next_id);
        self.next_id += 1;
        self.ids.push(e);
        self.transforms.push(t);
        self.kinds.push(k);
        e
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
pub mod components;
pub use components::*;
pub mod parse;
