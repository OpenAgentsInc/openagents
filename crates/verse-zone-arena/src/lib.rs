//! The Grid Arena: a robot duel on the Grid's floor. The player walks the
//! Striker with the shared WoW-style controller and fights the Brute, an
//! enemy robot that closes in, circles, fires plasma bolts, and slams the
//! ground. Four abilities sit on keys 1 to 4 (Laser, Missile Salvo, Shield,
//! and EMP Burst) under Verse's combat model: real time, a global cooldown,
//! per-ability cooldowns, and SRD-style dice rolled behind the scenes from
//! a seed (`docs/verse/combat-model.md`).
//!
//! [`rules`] holds the fight and [`ai`] the enemy's choices, neither of
//! which draws; [`draw`] turns the fight into the Grid's hidden-line
//! meshes and [`hud`] into the screen's bars, numbers, and action bar.
//!
//! `verse` re-exports this crate as `zones::arena`, so an edit here
//! recompiles this crate and what depends on it rather than all of Verse.

pub mod ai;
pub mod draw;
pub mod hud;
pub mod rules;
#[cfg(test)]
mod tests;

use glam::Vec3;
use verse_core::world::World;
use verse_pbr::mesh::Mesh;
use verse_world::social::controller::{InputState, PlayerController};

pub use rules::{Ability, Fight, Outcome, Refusal, Side};

/// Half the walkable square, m: the ring and the pylons outside it.
pub const HALF_EXTENT: f32 = 30.0;
/// The return portal, behind the player's start, past the ring.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -26.0);
/// The seed of the first fight; each restart takes the next.
pub const SEED: u64 = 0x0A12_E7A5;
/// The enemy's amber, as sRGB. The Grid itself draws in white light; the
/// Brute's lines, shots, and numbers take this color so the two robots read
/// apart at a glance.
pub const ENEMY_AMBER: u32 = 0xFFA41C;

/// [`ENEMY_AMBER`] in linear light, scaled by `k`.
#[must_use]
pub fn enemy_amber(k: f32) -> [f32; 3] {
    verse_gfx::palette::linear(ENEMY_AMBER).map(|c| c * k)
}

/// How long an error line stays on screen, s.
const MESSAGE_SECONDS: f32 = 1.4;

/// The zone's state: the fight, how many restarts, and the error line.
pub struct Arena {
    fight: Fight,
    round: u64,
    message: Option<(&'static str, f32)>,
    rendered: Mesh,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(SEED)
    }

    /// An arena whose first fight rolls from `seed`.
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        let mut arena = Self {
            fight: Fight::new(seed),
            round: 0,
            message: None,
            rendered: Mesh::default(),
        };
        arena.redraw();
        arena
    }

    /// Where the player arrives: the Striker's start, facing the Brute.
    #[must_use]
    pub fn spawn() -> Vec3 {
        rules::PLAYER_START
    }

    #[must_use]
    pub fn spawn_yaw() -> f32 {
        0.0
    }

    /// The ring, the pylons, and the start marks. The host lays them over
    /// the Grid's own floor.
    #[must_use]
    pub fn world() -> World {
        World {
            mesh: draw::world(),
            blockers: Vec::new(),
        }
    }

    #[must_use]
    pub fn fight(&self) -> &Fight {
        &self.fight
    }

    /// The fight, to change it, as a test or a capture does.
    pub fn fight_mut(&mut self) -> &mut Fight {
        &mut self.fight
    }

    /// How many times the fight has restarted.
    #[must_use]
    pub fn round(&self) -> u64 {
        self.round
    }

    /// The error line and when it goes, while one shows.
    #[must_use]
    pub fn message(&self) -> Option<(&'static str, f32)> {
        self.message.filter(|(_, until)| *until > self.fight.now)
    }

    /// Moves the player's robot with the shared controller; a stunned or
    /// destroyed robot stands still.
    pub fn move_player(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        dt: f32,
        bound: f32,
    ) {
        let held = self.fight.player.stunned(self.fight.now) || !self.fight.player.alive();
        let input = if held { InputState::default() } else { *input };
        player.update(&input, dt, &[], bound);
    }

    /// Advances the fight by `dt`: the player's pose goes in, and the
    /// knockback, the ring, and the enemy's body come back out to it.
    pub fn tick(&mut self, dt: f32, player: &mut PlayerController) {
        self.fight.sync_player(player.pos, player.yaw, player.speed);
        self.fight.tick(dt);
        player.pos.x = self.fight.player.pos.x;
        player.pos.z = self.fight.player.pos.z;
        self.redraw();
    }

    /// Fires the ability on `slot` (0-based). A refusal shows its line.
    ///
    /// # Errors
    ///
    /// The slot is empty or the ability cannot fire now.
    pub fn press(&mut self, slot: usize) -> Result<Ability, Refusal> {
        let Some(ability) = Ability::slot(slot) else {
            return Err(Refusal::NotReady);
        };
        match self.fight.press(ability) {
            Ok(()) => {
                self.redraw();
                Ok(ability)
            }
            Err(refusal) => {
                self.message = Some((refusal.message(), self.fight.now + MESSAGE_SECONDS));
                Err(refusal)
            }
        }
    }

    /// Starts a fresh fight with the next seed; the player goes back to the
    /// start, which the host applies.
    pub fn restart(&mut self) -> (Vec3, f32) {
        self.round += 1;
        let seed = SEED ^ self.round.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.fight = Fight::new(seed);
        self.message = None;
        self.redraw();
        (Self::spawn(), Self::spawn_yaw())
    }

    fn redraw(&mut self) {
        self.rendered = draw::dynamic(&self.fight);
    }

    /// This frame's robots, shots, and effects.
    #[must_use]
    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The zone panel's caption: the fight's state in a line or two.
    #[must_use]
    pub fn caption(&self) -> String {
        let f = &self.fight;
        let state = match f.outcome {
            Outcome::Fighting => format!(
                "You {:.0}/{:.0} · Brute {:.0}/{:.0}",
                f.player.hp.max(0.0),
                f.player.max_hp,
                f.enemy.hp.max(0.0),
                f.enemy.max_hp
            ),
            Outcome::Won => "You won. Press R to fight again.".into(),
            Outcome::Lost => "Your robot was destroyed. Press R to fight again.".into(),
        };
        format!("Grid Arena · {state}\n1 Laser · 2 Missiles · 3 Shield · 4 EMP · G leaves")
    }
}
