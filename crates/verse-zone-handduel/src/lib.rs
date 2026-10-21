//! The Hand Duel: a sword-and-shield fight played with your hands in front
//! of a camera. It is the hand-tracked companion to the Prussian duel
//! (`verse-zone-duel`): the left hand holds a round shield, the right a
//! sword, and a potion belt hangs at the left hip. The Barrow Warden, a
//! skeletal revenant with a hooked glaive, walks out of his barrow and
//! swings at you with a long, readable windup.
//!
//! The browser's hand landmarker sends each camera frame's points to
//! [`HandDuel::hand_frame`]. [`body`] places both hands in the player's body,
//! [`gesture`] reads them as blocks, swings, bashes, potions, and the
//! scabbard, and [`rules`] resolves the fight with the combat model's dice
//! behind the scenes (`docs/verse/combat-model.md`). [`draw`] and [`hud`]
//! show it in the Grid's line style.
//!
//! `verse` re-exports this crate as `zones::hand_duel`.
//! `docs/verse/hand-duel.md` has the gestures and the rules.

pub mod body;
pub mod draw;
pub mod gesture;
pub mod hud;
pub mod rules;
#[cfg(test)]
mod tests;

use glam::{Mat4, Vec3};
use openagents_web_hands::Landmark;
use openagents_web_hands::track::RawFrame;
use verse_core::world::World;
use verse_pbr::mesh::Mesh;

pub use body::{Grip, HandPose, Side};
pub use gesture::{Event, Gestures, Swing};
pub use rules::{Blow, BlowResult, Fight, Outcome, SwingResult};

/// Half the walkable square, m.
pub const HALF_EXTENT: f32 = 30.0;
/// The way back to the plaza, behind the player.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -14.0);
/// The seed of the first fight; each new fight takes the next.
pub const SEED: u64 = 0x4A9D_D0E1;
/// The Warden's amber, as sRGB: the Grid Arena's enemy color.
pub const ENEMY_AMBER: u32 = 0xFFA41C;
/// The eye: a little behind and above the body's origin, so both hands
/// stay in view in front of it.
pub const EYE: Vec3 = Vec3::new(0.0, 1.62, -0.45);
/// Where the eye looks: the Warden's chest at his reach.
pub const LOOK: Vec3 = Vec3::new(0.0, 1.3, 3.0);
/// The first-person view's vertical field of view, radians.
pub const FOV_Y: f32 = 1.25;
/// How many trail points a swing keeps.
const TRAIL: usize = 14;

/// [`ENEMY_AMBER`] in linear light, scaled by `k`.
#[must_use]
pub fn enemy_amber(k: f32) -> [f32; 3] {
    verse_gfx::palette::linear(ENEMY_AMBER).map(|c| c * k)
}

/// The zone's state.
pub struct HandDuel {
    fight: Fight,
    body: body::Body,
    gestures: Gestures,
    round: u64,
    /// The last hand frame's time, s, and the camera picture's aspect.
    hand_time: f64,
    aspect: f32,
    /// Whether the page's hand tracking is on.
    tracking: bool,
    /// The blade tip's recent path.
    trail: Vec<Vec3>,
    /// The last swing's result, for the screen.
    pub last_swing: Option<SwingResult>,
    rendered: Mesh,
}

impl Default for HandDuel {
    fn default() -> Self {
        Self::new()
    }
}

impl HandDuel {
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(SEED)
    }

    /// A duel whose first fight rolls from `seed`.
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        let mut duel = Self {
            fight: Fight::new(seed),
            body: body::Body::default(),
            gestures: Gestures::default(),
            round: 0,
            hand_time: 0.0,
            aspect: 4.0 / 3.0,
            tracking: false,
            trail: Vec::new(),
            last_swing: None,
            rendered: Mesh::default(),
        };
        duel.redraw();
        duel
    }

    /// Where the player stands: the origin, facing the barrow.
    #[must_use]
    pub fn spawn() -> Vec3 {
        Vec3::ZERO
    }

    #[must_use]
    pub fn spawn_yaw() -> f32 {
        0.0
    }

    /// The barrow field.
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
    pub fn gestures(&self) -> &Gestures {
        &self.gestures
    }

    #[must_use]
    pub fn round(&self) -> u64 {
        self.round
    }

    /// The hand on `side`, while the camera sees it.
    #[must_use]
    pub fn hand(&self, side: Side) -> Option<HandPose> {
        self.body.hand(side)
    }

    /// The hand's smoothed points, for drawing its bones.
    #[must_use]
    pub fn points(&self, side: Side) -> Option<[Landmark; 21]> {
        self.body.points(side)
    }

    #[must_use]
    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    #[must_use]
    pub fn hand_time(&self) -> f64 {
        self.hand_time
    }

    /// Whether the page's hand tracking is on.
    #[must_use]
    pub fn tracking(&self) -> bool {
        self.tracking
    }

    /// Hand frames read since the zone opened.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.body.frames
    }

    #[must_use]
    pub fn trail(&self) -> &[Vec3] {
        &self.trail
    }

    /// The page turned hand tracking on or off.
    pub fn set_tracking(&mut self, on: bool) {
        self.tracking = on;
        if !on {
            self.body.clear();
            let events = self.gestures.update(None, None, self.hand_time);
            self.apply(events);
        }
    }

    /// One frame of the landmarker's points, packed as the page sends them
    /// (`openagents_web_hands::RawFrame`), at `t` seconds.
    pub fn hand_frame(&mut self, data: &[f32], t: f64) {
        let Some(frame) = RawFrame::decode(data) else {
            return;
        };
        self.read(&frame, t);
    }

    /// One decoded frame at `t` seconds.
    pub fn read(&mut self, frame: &RawFrame, t: f64) {
        self.tracking = true;
        self.hand_time = t;
        self.aspect = frame.aspect;
        self.body.frame(frame, t);
        self.hands(t);
    }

    /// Body-space hands straight into the gesture layer, at `t` seconds:
    /// what a camera frame becomes, for tests and captures.
    pub fn poses(&mut self, left: Option<HandPose>, right: Option<HandPose>, t: f64) {
        self.hand_time = t;
        let events = self.gestures.update(left.as_ref(), right.as_ref(), t);
        self.track_trail(right.as_ref());
        self.guard(left.as_ref());
        self.apply(events);
    }

    fn hands(&mut self, t: f64) {
        let left = self.body.hand(Side::Left);
        let right = self.body.hand(Side::Right);
        let events = self.gestures.update(left.as_ref(), right.as_ref(), t);
        self.track_trail(right.as_ref());
        self.guard(left.as_ref());
        self.apply(events);
    }

    fn track_trail(&mut self, right: Option<&HandPose>) {
        match right {
            Some(hand) if self.gestures.swinging() => {
                self.trail.push(gesture::blade_tip(hand));
                if self.trail.len() > TRAIL {
                    self.trail.remove(0);
                }
            }
            _ => {
                if !self.trail.is_empty() {
                    self.trail.remove(0);
                }
            }
        }
    }

    fn guard(&mut self, left: Option<&HandPose>) {
        let guard = left
            .filter(|_| self.gestures.shield_up)
            .map(|hand| (gesture::shield_cover(hand), hand.pos));
        self.fight.set_guard(guard);
    }

    fn apply(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Swing(swing) => self.last_swing = Some(self.fight.swing(&swing)),
                Event::Bash(_) => {
                    let _ = self.fight.bash();
                }
                Event::Drink => {
                    let _ = self.fight.drink();
                }
                // Putting the sword away after a fight starts the next.
                Event::Sheathe if self.fight.outcome != Outcome::Fighting => self.next_fight(),
                Event::Shield(_)
                | Event::PotionTaken
                | Event::PotionReturned
                | Event::Sheathe
                | Event::Draw => {}
            }
        }
    }

    /// A fresh fight with the next seed and a full belt. The sword stays
    /// sheathed until the player draws it.
    pub fn next_fight(&mut self) {
        self.round += 1;
        let seed = SEED ^ self.round.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.fight = Fight::new(seed);
        let drawn = self.gestures.drawn;
        self.gestures = Gestures::default();
        self.gestures.drawn = drawn;
        self.last_swing = None;
        self.redraw();
    }

    /// Advances the fight by `dt` and redraws it.
    pub fn tick(&mut self, dt: f32) {
        self.fight.tick(dt);
        self.redraw();
    }

    fn redraw(&mut self) {
        self.rendered = draw::dynamic(self);
    }

    /// This frame's moving parts.
    #[must_use]
    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// A small jolt when a blow lands on the player.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        let left = self.fight.jolt_until - self.fight.now;
        if left <= 0.0 {
            return Vec3::ZERO;
        }
        let k = left * 0.16;
        let t = self.fight.now * 70.0;
        Vec3::new(t.sin() * k, (t * 1.3).cos() * k, 0.0)
    }

    /// The first-person view at `aspect`, and the eye, with the player at
    /// `origin`.
    #[must_use]
    pub fn view(&self, aspect: f32, origin: Vec3) -> (Mat4, Vec3) {
        let eye = origin + EYE + self.shake();
        let look = origin + LOOK;
        let view = Mat4::look_at_rh(eye, look, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y, aspect.clamp(0.2, 5.0), 0.05, 2000.0);
        (proj * view, eye)
    }

    /// The zone panel's caption.
    #[must_use]
    pub fn caption(&self) -> String {
        let f = &self.fight;
        let state = match f.outcome {
            Outcome::Fighting => format!(
                "You {:.0}/{:.0} · Warden {:.0}/{:.0}",
                f.player.hp, f.player.max_hp, f.warden.hp, f.warden.max_hp
            ),
            Outcome::Won => "The Warden falls. Sheathe your sword to fight again.".into(),
            Outcome::Lost => "You fell. Sheathe your sword to fight again.".into(),
        };
        format!("Hand Duel · {state}")
    }
}
