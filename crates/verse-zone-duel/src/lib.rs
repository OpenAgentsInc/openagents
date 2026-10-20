//! The Prussian duel: a brother of the Teutonic Order against an Old
//! Prussian warrior on an open meadow, about 1240, during the Northern
//! Crusades. It is a one-on-one melee demo: directional sword-and-board
//! fighting with stamina, shield blocks and bashes, timed parries and
//! ripostes, feints, footwork, and armour by hit zone.
//!
//! [`rules`] holds the fight, [`rig`] the fighters' poses, [`hits`] the
//! blade sweeps through the `physics` crate, and [`ai`] the opponent's
//! choices; none of them draws. [`draw`] turns the fight into physically
//! lit meshes under Everglade's daylight, and [`hud`] into the screen's
//! bars, cues, and callouts.
//!
//! `verse` re-exports this crate as `zones::duel`. `docs/verse/duel.md`
//! has the controls, the rules, and the historical notes.

pub mod ai;
pub mod draw;
pub mod hits;
pub mod hud;
pub mod rig;
pub mod rules;
#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};
use verse_core::world::World;
use verse_pbr::mesh::Mesh;
use verse_world::social::controller::{InputState, PlayerController};

pub use rules::{Command, Dir, Fight, Outcome, Refusal, Side};

/// Half the walkable square, m.
pub const HALF_EXTENT: f32 = 60.0;
/// The way back to the plaza, behind the knight's start.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -16.0);
/// The seed of the first fight; each restart takes the next.
pub const SEED: u64 = 0x1240_D0E1;
/// The hour Everglade's sky is pinned to: late morning.
pub const HOUR: f32 = 10.5;

/// How far the camera's view turns from the knight's facing, radians: it
/// looks over his right shoulder, so the Prussian stands clear of him on
/// screen.
pub const CAMERA_TURN: f32 = 0.42;

/// How long an error line stays on screen, s.
const MESSAGE_SECONDS: f32 = 1.2;

/// The zone's state: the fight, the player's chosen direction, and the
/// error line.
pub struct Duel {
    fight: Fight,
    round: u64,
    message: Option<(&'static str, f32)>,
    /// Mouse travel not yet read as a direction, points.
    aim: Vec2,
    /// The direction the next blow takes.
    dir: Dir,
    shield: bool,
    rendered: Mesh,
}

impl Default for Duel {
    fn default() -> Self {
        Self::new()
    }
}

impl Duel {
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(SEED)
    }

    /// A duel whose first fight rolls from `seed`.
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        let mut duel = Self {
            fight: Fight::new(seed),
            round: 0,
            message: None,
            aim: Vec2::ZERO,
            dir: Dir::Right,
            shield: false,
            rendered: Mesh::default(),
        };
        duel.redraw();
        duel
    }

    /// Where the player arrives: the knight's start, facing the Prussian.
    #[must_use]
    pub fn spawn() -> Vec3 {
        rules::PLAYER_START
    }

    #[must_use]
    pub fn spawn_yaw() -> f32 {
        0.0
    }

    /// The meadow: ground, grass, and the far props.
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

    #[must_use]
    pub fn round(&self) -> u64 {
        self.round
    }

    /// The direction the next blow takes.
    #[must_use]
    pub fn aim_dir(&self) -> Dir {
        self.dir
    }

    /// Whether the player holds the shield up.
    #[must_use]
    pub fn shield_held(&self) -> bool {
        self.shield
    }

    /// The error line and when it goes, while one shows.
    #[must_use]
    pub fn message(&self) -> Option<(&'static str, f32)> {
        self.message.filter(|(_, until)| *until > self.fight.now)
    }

    /// Reads the movement keys as footwork: W and S step toward and away,
    /// A and D (and Q and E) circle. The knight always faces his opponent.
    pub fn move_player(&mut self, input: &InputState) {
        let ahead = f32::from(i8::from(input.forward) - i8::from(input.backward));
        let side = f32::from(
            i8::from(input.right || input.strafe_right) - i8::from(input.left || input.strafe_left),
        );
        self.fight.steer(
            Side::Player,
            rules::Steer {
                ahead,
                side,
                shield: self.shield,
            },
        );
    }

    /// Holds the shield up or lets it down.
    pub fn set_shield(&mut self, up: bool) {
        self.shield = up;
        let steer = self.fight.player().steer;
        self.fight.steer(
            Side::Player,
            rules::Steer {
                shield: up,
                ..steer
            },
        );
    }

    /// Mouse travel, in points: a clear movement picks the next blow's
    /// direction.
    pub fn aim(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.aim += Vec2::new(dx, dy);
        if let Some(dir) = Dir::from_motion(self.aim.x, self.aim.y) {
            self.dir = dir;
            self.aim = Vec2::ZERO;
        }
    }

    /// Picks the next blow's direction outright, as a key does.
    pub fn choose(&mut self, dir: Dir) {
        self.dir = dir;
        self.aim = Vec2::ZERO;
    }

    fn refuse(&mut self, result: Result<(), Refusal>) -> Result<(), Refusal> {
        if let Err(refusal) = result
            && refusal != Refusal::Busy
        {
            self.message = Some((refusal.message(), self.fight.now + MESSAGE_SECONDS));
        }
        result
    }

    /// Strikes in `dir`, or in the chosen direction. In a windup, a new
    /// direction is a feint.
    ///
    /// # Errors
    ///
    /// The knight is busy or too tired.
    pub fn attack(&mut self, dir: Option<Dir>) -> Result<(), Refusal> {
        if let Some(dir) = dir {
            self.choose(dir);
        }
        let result = self.fight.command(Side::Player, Command::Attack(self.dir));
        self.refuse(result)
    }

    /// Raises the sword to parry.
    ///
    /// # Errors
    ///
    /// The knight is busy or too tired.
    pub fn parry(&mut self) -> Result<(), Refusal> {
        let result = self.fight.command(Side::Player, Command::Parry);
        self.refuse(result)
    }

    /// Drives the shield into the opponent.
    ///
    /// # Errors
    ///
    /// The knight is busy, his shield arm is spent, or he is too tired.
    pub fn bash(&mut self) -> Result<(), Refusal> {
        let result = self.fight.command(Side::Player, Command::Bash);
        self.refuse(result)
    }

    /// Pulls a windup back into guard.
    ///
    /// # Errors
    ///
    /// No windup to pull back, or too tired.
    pub fn cancel(&mut self) -> Result<(), Refusal> {
        let result = self.fight.command(Side::Player, Command::Cancel);
        self.refuse(result)
    }

    /// A quick step the way the movement keys point, or back when none
    /// are held.
    ///
    /// # Errors
    ///
    /// The knight is busy or too tired.
    pub fn dodge(&mut self, input: &InputState) -> Result<(), Refusal> {
        let ahead = f32::from(i8::from(input.forward) - i8::from(input.backward));
        let side = f32::from(
            i8::from(input.right || input.strafe_right) - i8::from(input.left || input.strafe_left),
        );
        let result = self
            .fight
            .command(Side::Player, Command::Step(Vec2::new(side, ahead)));
        self.refuse(result)
    }

    /// Advances the fight by `dt` and writes the knight's pose back to the
    /// shared controller, which the camera follows.
    pub fn tick(&mut self, dt: f32, player: &mut PlayerController) {
        self.fight.tick(dt);
        let f = self.fight.player();
        player.pos = f.pos;
        // The knight's own facing lives in the fight; the shared
        // controller's yaw is what the camera follows.
        player.yaw = f.yaw + CAMERA_TURN;
        player.speed = f.vel.length();
        self.aim *= (-dt.max(0.0) * 6.0).exp();
        self.redraw();
    }

    /// Starts a fresh fight with the next seed; the knight goes back to
    /// his start, which the host applies.
    pub fn restart(&mut self) -> (Vec3, f32) {
        self.round += 1;
        let seed = SEED ^ self.round.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.fight = Fight::new(seed);
        self.message = None;
        self.shield = false;
        self.redraw();
        (Self::spawn(), Self::spawn_yaw())
    }

    fn redraw(&mut self) {
        self.rendered = draw::dynamic(&self.fight);
    }

    /// This frame's fighters and flashes.
    #[must_use]
    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The camera's jolt this frame, m.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        let amp = self.fight.shake_now();
        if amp <= 1e-4 {
            return Vec3::ZERO;
        }
        let t = self.fight.now * 60.0;
        Vec3::new(
            (t * 1.7).sin(),
            (t * 2.3 + 1.0).sin() * 0.7,
            (t * 1.3 + 2.0).sin(),
        ) * amp
    }

    /// The zone panel's caption.
    #[must_use]
    pub fn caption(&self) -> String {
        let f = &self.fight;
        let state = match f.outcome {
            Outcome::Fighting => format!(
                "You {:.0} · Skurdo {:.0}",
                f.player().hp.max(0.0),
                f.enemy().hp.max(0.0)
            ),
            Outcome::Won => "You won. Press R to fight again.".into(),
            Outcome::Lost => "You fell. Press R to fight again.".into(),
        };
        format!("Prussian Duel, 1240 · {state}\nG leaves")
    }
}
