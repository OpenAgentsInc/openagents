//! The gesture layer: two hands in body space become the fight's actions.
//!
//! Every input reaches the fight through here, as [`HandPose`]s, so the
//! same rules read a camera's hands and a test's made-up ones.
//!
//! - **Shield (left hand).** The shield is up while the left hand is at
//!   chest height or above; it covers the direction from the chest to the
//!   hand. Pushing the raised shield forward fast is a bash.
//! - **Weapon (right hand).** A swing starts when the hand moves faster than
//!   [`SWING_START`] and ends when it slows below [`SWING_END`]. The blade's
//!   tip path, the arc's length, and the top speed go to the fight.
//! - **Potion.** Close the left hand at the left hip (or reach below the
//!   picture on the left and come back closed) to take a potion from the
//!   belt; the shield drops while the hand holds it. Hold it at the mouth
//!   for [`DRINK_HOLD`] to drink. Open the hand anywhere else to put it back.
//! - **Sheathe and draw.** The sword hangs on the back. Reach over the right
//!   shoulder and open the hand to sheathe it; reach there and close the
//!   hand to draw it.

use glam::Vec3;

use crate::body::{Grip, HandPose};

/// The chest, where the shield's cover and an attack's line start, m.
pub const CHEST: Vec3 = Vec3::new(0.0, 1.35, 0.0);
/// The mouth, m.
pub const MOUTH: Vec3 = Vec3::new(0.0, 1.62, 0.15);

/// The left hand raises the shield at this height and lowers it below
/// [`SHIELD_LOWER`], m.
pub const SHIELD_RAISE: f32 = 1.22;
pub const SHIELD_LOWER: f32 = 1.12;

/// A swing starts above this hand speed, m/s.
pub const SWING_START: f32 = 1.4;
/// And ends below this one.
pub const SWING_END: f32 = 0.55;
/// The shortest arc that counts as a swing, m of hand travel.
pub const SWING_MIN_ARC: f32 = 0.22;
/// A swing longer than this is a wave, not a blow, s.
pub const SWING_MAX: f64 = 0.9;
/// The blade's length from the palm to its tip, m.
pub const BLADE: f32 = 0.9;

/// A raised shield pushed forward this fast bashes, m/s.
pub const BASH_FORWARD: f32 = 0.9;
/// Or swept across this fast toward the middle.
pub const BASH_ACROSS: f32 = 1.8;
/// Between two bashes, s.
pub const BASH_COOLDOWN: f64 = 1.2;

/// The left hip's belt pouch: beyond this `x` to the left and below this
/// height, m.
pub const BELT_X: f32 = 0.02;
pub const BELT_Y: f32 = 1.05;
/// A left hand that leaves the picture below this height and comes back
/// closed within [`BELT_RETURN`] has taken a potion, m and s.
pub const BELT_LOST_Y: f32 = 1.15;
pub const BELT_RETURN: f64 = 1.5;
/// The mouth's reach: a held potion this close to [`MOUTH`] in the face's
/// plane is at the lips, m.
pub const MOUTH_REACH: f32 = 0.24;
/// How long the potion stays at the mouth to drink it, s.
pub const DRINK_HOLD: f64 = 0.45;

/// The scabbard over the right shoulder: a right hand above this height
/// and beyond this `x` to the right, m.
pub const SHOULDER_Y: f32 = 1.82;
pub const SHOULDER_X: f32 = -0.08;
/// An open hand held there this long sheathes the sword, s.
pub const SHEATHE_HOLD: f64 = 0.5;

/// One swing of the weapon hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Swing {
    /// The blade tip's path in body space, oldest first.
    pub path: Vec<Vec3>,
    /// The hand's top speed, m/s.
    pub peak: f32,
    /// How far the hand travelled, m.
    pub arc: f32,
    /// How long the swing took, s.
    pub seconds: f64,
}

/// What the hands did this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Swing(Swing),
    /// The shield went up or down.
    Shield(bool),
    /// A raised shield pushed into a bash, at this speed.
    Bash(f32),
    /// The left hand took a potion from the belt.
    PotionTaken,
    /// The left hand put it back without drinking.
    PotionReturned,
    /// The potion reached the mouth and was drunk.
    Drink,
    Sheathe,
    Draw,
}

/// The potion hand's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Potion {
    /// No potion in hand.
    Belt,
    /// Held, and at the mouth since this time when it is.
    Held { at_mouth: Option<f64> },
}

#[derive(Clone, Debug)]
struct SwingTrack {
    start: f64,
    last: Vec3,
    path: Vec<Vec3>,
    peak: f32,
    arc: f32,
}

/// The gesture layer's state.
#[derive(Clone, Debug)]
pub struct Gestures {
    /// Whether the sword is in hand.
    pub drawn: bool,
    /// Whether the shield is up.
    pub shield_up: bool,
    pub potion: Potion,
    /// Potions left in the belt.
    pub potions: u32,
    swing: Option<SwingTrack>,
    sheathe_since: Option<f64>,
    bash_at: f64,
    /// Where the left hand was last seen, and when.
    left_last: Option<(f64, Vec3)>,
}

/// The tip of a blade held in `hand`.
#[must_use]
pub fn blade_tip(hand: &HandPose) -> Vec3 {
    hand.pos + hand.up * BLADE
}

/// The direction the raised shield covers: from the chest through the
/// shield.
#[must_use]
pub fn shield_cover(hand: &HandPose) -> Vec3 {
    (hand.pos - CHEST).normalize_or(Vec3::Z)
}

/// Whether a raised shield at `cover` stops a blow arriving along
/// `incoming` (from the chest toward where the blow lands): the angle
/// between them is within `cone` radians.
#[must_use]
pub fn shield_blocks(cover: Vec3, incoming: Vec3, cone: f32) -> bool {
    let (a, b) = (cover.normalize_or_zero(), incoming.normalize_or_zero());
    if a == Vec3::ZERO || b == Vec3::ZERO {
        return false;
    }
    a.dot(b).clamp(-1.0, 1.0).acos() <= cone
}

/// Whether a hand at `pos` is at the mouth.
#[must_use]
pub fn at_mouth(pos: Vec3) -> bool {
    let d = pos - MOUTH;
    d.x.hypot(d.y) <= MOUTH_REACH
}

/// Whether a left hand at `pos` is at the belt pouch.
#[must_use]
pub fn at_belt(pos: Vec3) -> bool {
    pos.x >= BELT_X && pos.y <= BELT_Y
}

/// Whether a right hand at `pos` is over the shoulder at the scabbard.
#[must_use]
pub fn at_scabbard(pos: Vec3) -> bool {
    pos.y >= SHOULDER_Y && pos.x <= SHOULDER_X
}

impl Default for Gestures {
    fn default() -> Self {
        Self::new(3)
    }
}

impl Gestures {
    /// Sword in hand, shield down, `potions` in the belt.
    #[must_use]
    pub fn new(potions: u32) -> Self {
        Self {
            drawn: true,
            shield_up: false,
            potion: Potion::Belt,
            potions,
            swing: None,
            sheathe_since: None,
            bash_at: f64::NEG_INFINITY,
            left_last: None,
        }
    }

    /// Whether a swing is under way.
    #[must_use]
    pub fn swinging(&self) -> bool {
        self.swing.is_some()
    }

    /// How far through the drink the potion is, 0 to 1.
    #[must_use]
    pub fn drink_progress(&self, t: f64) -> f32 {
        match self.potion {
            Potion::Held {
                at_mouth: Some(since),
            } => ((t - since) / DRINK_HOLD).clamp(0.0, 1.0) as f32,
            _ => 0.0,
        }
    }

    /// How far through the sheathe hold the hand is, 0 to 1.
    #[must_use]
    pub fn sheathe_progress(&self, t: f64) -> f32 {
        self.sheathe_since
            .map_or(0.0, |s| ((t - s) / SHEATHE_HOLD).clamp(0.0, 1.0) as f32)
    }

    /// Reads one frame of both hands at `t` seconds; `None` is a hand the
    /// camera doesn't see.
    pub fn update(
        &mut self,
        left: Option<&HandPose>,
        right: Option<&HandPose>,
        t: f64,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        self.left_hand(left, t, &mut events);
        self.right_hand(right, t, &mut events);
        events
    }

    fn left_hand(&mut self, left: Option<&HandPose>, t: f64, events: &mut Vec<Event>) {
        let Some(hand) = left else {
            // Out of sight, the shield hangs at the side; a held potion
            // stays held.
            self.set_shield(false, events);
            return;
        };
        // Back in sight after reaching low on the left: the hand went to the
        // belt below the picture.
        let from_below = self.left_last.is_some_and(|(seen, pos)| {
            t - seen > crate::body::LOST_AFTER * 0.5
                && t - seen <= BELT_RETURN
                && pos.y <= BELT_LOST_Y
                && pos.x >= BELT_X - 0.15
        });
        self.left_last = Some((t, hand.pos));

        match self.potion {
            Potion::Belt => {
                let reached = at_belt(hand.pos) || from_below;
                if reached && hand.grip == Grip::Closed && self.potions > 0 {
                    self.potion = Potion::Held { at_mouth: None };
                    events.push(Event::PotionTaken);
                }
            }
            Potion::Held { at_mouth: since } => {
                if at_mouth(hand.pos) {
                    let since = since.unwrap_or(t);
                    if t - since >= DRINK_HOLD {
                        self.potions = self.potions.saturating_sub(1);
                        self.potion = Potion::Belt;
                        events.push(Event::Drink);
                    } else {
                        self.potion = Potion::Held {
                            at_mouth: Some(since),
                        };
                    }
                } else if hand.grip == Grip::Open {
                    self.potion = Potion::Belt;
                    events.push(Event::PotionReturned);
                } else {
                    self.potion = Potion::Held { at_mouth: None };
                }
            }
        }

        // The shield: up at chest height with no potion in the hand.
        let holding = matches!(self.potion, Potion::Held { .. });
        let up = !holding
            && if self.shield_up {
                hand.pos.y >= SHIELD_LOWER
            } else {
                hand.pos.y >= SHIELD_RAISE
            };
        self.set_shield(up, events);

        if self.shield_up && t - self.bash_at >= BASH_COOLDOWN {
            let speed = hand.vel.length();
            if hand.vel.z >= BASH_FORWARD || (-hand.vel.x >= BASH_ACROSS && speed >= BASH_ACROSS) {
                self.bash_at = t;
                events.push(Event::Bash(speed));
            }
        }
    }

    fn set_shield(&mut self, up: bool, events: &mut Vec<Event>) {
        if up != self.shield_up {
            self.shield_up = up;
            events.push(Event::Shield(up));
        }
    }

    fn right_hand(&mut self, right: Option<&HandPose>, t: f64, events: &mut Vec<Event>) {
        let Some(hand) = right else {
            // A fast blow can carry the hand out of the picture: it still
            // lands with the path it had.
            if let Some(track) = self.swing.take()
                && track.arc >= SWING_MIN_ARC
                && track.path.len() >= 2
            {
                events.push(Event::Swing(Swing {
                    path: track.path,
                    peak: track.peak,
                    arc: track.arc,
                    seconds: t - track.start,
                }));
            }
            self.sheathe_since = None;
            return;
        };
        // Sheathe and draw at the scabbard over the right shoulder.
        if at_scabbard(hand.pos) {
            if self.drawn && hand.grip == Grip::Open {
                let since = *self.sheathe_since.get_or_insert(t);
                if t - since >= SHEATHE_HOLD {
                    self.drawn = false;
                    self.sheathe_since = None;
                    self.swing = None;
                    events.push(Event::Sheathe);
                    return;
                }
            } else {
                self.sheathe_since = None;
            }
            if !self.drawn && hand.grip == Grip::Closed {
                self.drawn = true;
                events.push(Event::Draw);
                return;
            }
        } else {
            self.sheathe_since = None;
        }
        if !self.drawn {
            self.swing = None;
            return;
        }

        let speed = hand.vel.length();
        let tip = blade_tip(hand);
        match &mut self.swing {
            None => {
                if speed >= SWING_START {
                    // The swing began a frame's travel back.
                    let back = hand.pos - hand.vel * (1.0 / 30.0);
                    self.swing = Some(SwingTrack {
                        start: t,
                        last: hand.pos,
                        path: vec![back + hand.up * BLADE, tip],
                        peak: speed,
                        arc: hand.pos.distance(back),
                    });
                }
            }
            Some(track) => {
                track.arc += hand.pos.distance(track.last);
                track.last = hand.pos;
                track.peak = track.peak.max(speed);
                track.path.push(tip);
                let seconds = t - track.start;
                if seconds > SWING_MAX {
                    self.swing = None;
                } else if speed < SWING_END {
                    let done = self.swing.take();
                    if let Some(track) = done
                        && track.arc >= SWING_MIN_ARC
                    {
                        events.push(Event::Swing(Swing {
                            path: track.path,
                            peak: track.peak,
                            arc: track.arc,
                            seconds,
                        }));
                    }
                }
            }
        }
    }
}
