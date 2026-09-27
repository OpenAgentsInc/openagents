//! Authoritative actor store and basic types (pre-ECS bridge).

use glam::Vec3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Wizard,
    Zombie,
    Boss,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Faction {
    Pc,
    Wizards,
    Undead,
    Neutral,
}

/// Preferred terminology in docs and new code paths.
// Faction marks allegiance; used for target selection and hostility checks.

#[derive(Copy, Clone, Debug)]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}
impl Health {
    #[inline]
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
    #[inline]
    pub fn clamp(&mut self) {
        if self.hp > self.max {
            self.hp = self.max;
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Transform {
    pub pos: Vec3,
    pub yaw: f32,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
    pub faction: Faction,
    pub tr: Transform,
    pub hp: Health,
}

// Legacy ActorStore removed; ECS world is authoritative.
