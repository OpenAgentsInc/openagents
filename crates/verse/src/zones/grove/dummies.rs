//! The Grove's training dummies: where each stands, its health, armor, and
//! resistances, and how spells move and hold it.
//!
//! The table is the druid demo's (`docs/verse/druid-demo.md`, World): three
//! straw dummies at 10, 20, and 30 m from the spawn, an armored one at
//! 15 m, a fire-warded one at 25 m, a big one among them, and a flying
//! target 5.5 m up on a post. Armor class answers spell attack rolls, save
//! modifiers answer saving throws, and a resistance halves its damage
//! type, as in the SRD. Spells leave timed conditions on a dummy
//! ([`Condition`]), and repeated hard control within fifteen seconds
//! diminishes, as the combat model says. A dummy that nothing touches for
//! ten seconds stands back up at full health where it started.

use super::kit::{Ability, Damage};
use crate::zones::everglade::height;
use glam::{DVec3, Vec3};
use verse_world::reverse_gravity as reverse;

/// Seconds a dummy waits untouched before it resets.
pub const RESET_AFTER: f32 = 10.0;
/// A dummy's height at scale one, m, about the pack's model's.
pub const HEIGHT: f32 = 1.8;
/// Half the footprint a dummy blocks at scale one, m.
const HALF: f32 = 0.3;
/// How long a push slides, s.
const SLIDE: f32 = 0.35;
/// Window in which repeated control halves its duration, s.
const DIMINISH: f32 = 15.0;
/// Gravity on a dummy, m/s².
const GRAVITY: f64 = 9.81;
/// The longest step of a dummy's vertical motion, s.
const SUBSTEP: f64 = 1.0 / 120.0;
/// How high the flying target's post holds it, m: out of a druid's
/// reach on foot, in reach of the eagle, the dragon, and ranged spells.
pub const POST: f32 = 5.5;

/// A kind of dummy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Straw,
    Armored,
    Warded,
    Big,
    Flying,
}

impl Kind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Straw => "Straw Dummy",
            Self::Armored => "Armored Dummy",
            Self::Warded => "Warded Dummy",
            Self::Big => "Big Dummy",
            Self::Flying => "Flying Target",
        }
    }

    /// Health at full.
    #[must_use]
    pub const fn max_hp(self) -> f32 {
        match self {
            Self::Straw => 100.0,
            Self::Armored => 200.0,
            Self::Warded => 150.0,
            Self::Big => 300.0,
            Self::Flying => 60.0,
        }
    }

    /// The SRD-style armor class, which sets physical mitigation.
    #[must_use]
    pub const fn armor_class(self) -> i32 {
        match self {
            Self::Straw => 10,
            Self::Armored => 18,
            Self::Warded => 13,
            Self::Big => 15,
            Self::Flying => 12,
        }
    }

    /// The model's scale.
    #[must_use]
    pub const fn scale(self) -> f32 {
        match self {
            Self::Big => 1.7,
            Self::Flying => 0.8,
            _ => 1.0,
        }
    }

    /// Its modifier to a saving throw of `ability`: heavy dummies stand
    /// firm, the armored one is slow to dodge, and the flying target is
    /// light.
    #[must_use]
    pub const fn save(self, ability: Ability) -> i32 {
        match (self, ability) {
            (Self::Armored, Ability::Strength | Ability::Constitution) => 2,
            (Self::Armored, Ability::Dexterity) => -1,
            (Self::Warded, Ability::Constitution) => 1,
            (Self::Big, Ability::Strength | Ability::Constitution) => 3,
            (Self::Big, Ability::Dexterity) => -2,
            (Self::Flying, Ability::Dexterity) => 2,
            _ => 0,
        }
    }

    /// The multiplier damage of `kind` takes: one half for a resistance.
    /// The armored dummy resists piercing and slashing; the warded one
    /// resists fire.
    #[must_use]
    pub fn multiplier(self, kind: Damage) -> f32 {
        match (self, kind) {
            (Self::Warded, Damage::Fire) | (Self::Armored, Damage::Piercing | Damage::Slashing) => {
                0.5
            }
            _ => 1.0,
        }
    }
}

/// A timed condition on a dummy: the combat model's debuffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    /// Rooted in place: pushes and lifts don't move it.
    Restrained,
    /// Knocked down.
    Prone,
    Blinded,
    /// Takes poison damage each second.
    Poisoned,
    /// Faerie Fire's outline: it takes a fifth more damage.
    Outlined,
    /// A harmless beast, drawn small.
    Polymorphed,
    Paralyzed,
    /// Ends when it takes damage.
    Asleep,
    Slowed,
    /// Starry Wisp's light: it can't hide.
    Starlit,
    /// On fire: it takes fire damage each second.
    Burning,
    /// Cowering from the dragon's roar.
    Frightened,
}

impl Condition {
    /// Every condition, in the order its tag shows.
    pub const ALL: [Self; 12] = [
        Self::Restrained,
        Self::Prone,
        Self::Paralyzed,
        Self::Asleep,
        Self::Polymorphed,
        Self::Blinded,
        Self::Poisoned,
        Self::Outlined,
        Self::Frightened,
        Self::Burning,
        Self::Slowed,
        Self::Starlit,
    ];

    /// Its name in a combat line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Restrained => "rooted",
            Self::Prone => "knocked down",
            Self::Blinded => "blinded",
            Self::Poisoned => "poisoned",
            Self::Outlined => "outlined",
            Self::Polymorphed => "polymorphed",
            Self::Paralyzed => "paralyzed",
            Self::Asleep => "asleep",
            Self::Slowed => "slowed",
            Self::Starlit => "starlit",
            Self::Burning => "burning",
            Self::Frightened => "frightened",
        }
    }

    /// The short tag over the health bar.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Restrained => "ROOT",
            Self::Prone => "DOWN",
            Self::Blinded => "BLIND",
            Self::Poisoned => "POISON",
            Self::Outlined => "FAERIE",
            Self::Polymorphed => "POLY",
            Self::Paralyzed => "HELD",
            Self::Asleep => "SLEEP",
            Self::Slowed => "SLOW",
            Self::Starlit => "STAR",
            Self::Burning => "BURN",
            Self::Frightened => "FEAR",
        }
    }

    /// The tag's color, linear RGB.
    #[must_use]
    pub const fn color(self) -> [f32; 3] {
        match self {
            Self::Restrained => [0.7, 0.45, 1.0],
            Self::Prone => [0.95, 0.75, 0.4],
            Self::Blinded => [1.0, 0.95, 0.6],
            Self::Poisoned => [0.55, 1.0, 0.3],
            Self::Outlined => [0.85, 0.55, 1.0],
            Self::Polymorphed => [0.6, 1.0, 0.8],
            Self::Paralyzed => [1.0, 0.5, 0.5],
            Self::Asleep => [0.6, 0.7, 1.0],
            Self::Slowed => [0.6, 0.9, 1.0],
            Self::Starlit => [1.0, 0.95, 0.75],
            Self::Burning => [1.0, 0.5, 0.15],
            Self::Frightened => [0.95, 0.6, 0.85],
        }
    }

    /// Whether it is hard control, which diminishes when repeated.
    #[must_use]
    pub const fn hard(self) -> bool {
        matches!(
            self,
            Self::Restrained | Self::Prone | Self::Paralyzed | Self::Asleep | Self::Polymorphed
        )
    }
}

/// How much more damage an outlined dummy takes.
pub const OUTLINED: f32 = 1.2;

/// The field's dummies: kind and where each stands, x and z, m. The spawn
/// is at (0, -15) facing +z.
pub const FIELD: [(Kind, [f32; 2]); 7] = [
    (Kind::Straw, [0.0, -5.0]),
    (Kind::Straw, [-5.0, 4.4]),
    (Kind::Straw, [3.0, 14.85]),
    (Kind::Armored, [-7.0, -1.7]),
    (Kind::Warded, [8.0, 8.7]),
    (Kind::Big, [-1.5, 7.0]),
    (Kind::Flying, [9.0, -4.0]),
];

/// A push in progress: from where to where, and how far along, s.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slide {
    from: Vec3,
    to: Vec3,
    t: f32,
}

/// One dummy.
#[derive(Clone, Debug, PartialEq)]
pub struct Dummy {
    pub kind: Kind,
    /// Where it stands at rest: its feet, m.
    pub home: Vec3,
    /// Its feet now, m.
    pub pos: Vec3,
    /// Which way it faces, as the controller's yaw.
    pub yaw: f32,
    pub hp: f32,
    vertical_speed: f64,
    slide: Option<Slide>,
    /// Rooted until this time, s.
    rooted_until: f32,
    /// When control last took hold, for diminishing returns, s.
    controls: Vec<f32>,
    /// Each condition on it but Restrained (`rooted_until`), and when it
    /// ends, s.
    conditions: Vec<(Condition, f32)>,
    /// When anything last touched it, s.
    touched: f32,
    /// When it was last hit, for the wobble, s.
    pub hit_at: f32,
}

impl Dummy {
    /// A dummy of `kind` standing at `at`.
    #[must_use]
    pub fn new(kind: Kind, at: [f32; 2]) -> Self {
        let lift = if kind == Kind::Flying { POST } else { 0.0 };
        let home = Vec3::new(at[0], height(at[0], at[1]) + lift, at[1]);
        Self {
            kind,
            home,
            pos: home,
            // They face the spawn.
            yaw: (super::SPAWN.x - at[0]).atan2(super::SPAWN.z - at[1]),
            hp: kind.max_hp(),
            vertical_speed: 0.0,
            slide: None,
            rooted_until: f32::NEG_INFINITY,
            controls: Vec::new(),
            conditions: Vec::new(),
            touched: f32::NEG_INFINITY,
            hit_at: f32::NEG_INFINITY,
        }
    }

    /// The whole field, at rest.
    #[must_use]
    pub fn field() -> Vec<Self> {
        FIELD
            .iter()
            .map(|&(kind, at)| Self::new(kind, at))
            .collect()
    }

    /// The middle of its body, m.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::Y * (HEIGHT * 0.5 * self.kind.scale())
    }

    /// The top of its head, m.
    #[must_use]
    pub fn top(&self) -> f32 {
        self.pos.y + HEIGHT * self.kind.scale()
    }

    /// Whether its post holds it in place.
    #[must_use]
    pub fn anchored(&self) -> bool {
        self.kind == Kind::Flying
    }

    /// Whether a web holds it at `now`.
    #[must_use]
    pub fn rooted(&self, now: f32) -> bool {
        now < self.rooted_until
    }

    /// Whether it is down at zero health.
    #[must_use]
    pub fn down(&self) -> bool {
        self.hp <= 0.0
    }

    /// Its health as a fraction of full.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        (self.hp / self.kind.max_hp()).clamp(0.0, 1.0)
    }

    /// Deals `amount` of `kind` damage at `now` after resistance and
    /// Faerie Fire's outline, and returns what landed, rounded down. Damage
    /// wakes a sleeping dummy.
    pub fn damage(&mut self, amount: f32, kind: Damage, now: f32) -> i32 {
        let outlined = if self.has(Condition::Outlined, now) {
            OUTLINED
        } else {
            1.0
        };
        let dealt = (amount * self.kind.multiplier(kind) * outlined)
            .floor()
            .max(0.0);
        self.hp = (self.hp - dealt).max(0.0);
        self.touched = now;
        self.hit_at = now;
        if dealt > 0.0 {
            self.conditions.retain(|(c, _)| *c != Condition::Asleep);
        }
        dealt as i32
    }

    /// Restores `amount` hit points at `now`, up to full, and returns how
    /// many it restored.
    pub fn heal(&mut self, amount: f32, now: f32) -> i32 {
        let before = self.hp;
        self.hp = (self.hp + amount.max(0.0).floor()).min(self.kind.max_hp());
        self.touched = now;
        (self.hp - before) as i32
    }

    /// Whether `condition` holds it at `now`.
    #[must_use]
    pub fn has(&self, condition: Condition, now: f32) -> bool {
        match condition {
            Condition::Restrained => self.rooted(now),
            _ => self
                .conditions
                .iter()
                .any(|(c, until)| *c == condition && now < *until),
        }
    }

    /// Each condition on it at `now` and the seconds it has left, in
    /// [`Condition::ALL`] order.
    #[must_use]
    pub fn conditions(&self, now: f32) -> Vec<(Condition, f32)> {
        Condition::ALL
            .into_iter()
            .filter_map(|c| {
                let until = match c {
                    Condition::Restrained => self.rooted_until,
                    _ => self
                        .conditions
                        .iter()
                        .filter(|(k, _)| *k == c)
                        .map(|(_, u)| *u)
                        .fold(f32::NEG_INFINITY, f32::max),
                };
                (now < until).then_some((c, until - now))
            })
            .collect()
    }

    /// Puts `condition` on it for `seconds` at `now`. Hard control is
    /// halved for each hard control in the last fifteen seconds, and the
    /// third within them does nothing. Returns the duration applied.
    pub fn afflict(&mut self, condition: Condition, seconds: f32, now: f32) -> f32 {
        if condition == Condition::Restrained {
            return self.root(seconds, now);
        }
        self.touched = now;
        let duration = if condition.hard() {
            self.diminished(seconds, now)
        } else {
            seconds
        };
        if duration > 0.0 {
            match self.conditions.iter_mut().find(|(c, _)| *c == condition) {
                Some((_, until)) => *until = until.max(now + duration),
                None => self.conditions.push((condition, now + duration)),
            }
        }
        duration
    }

    /// `seconds` after diminishing returns at `now`, recording the control
    /// when it takes hold.
    fn diminished(&mut self, seconds: f32, now: f32) -> f32 {
        self.controls.retain(|&at| now - at < DIMINISH);
        let duration = match self.controls.len() {
            0 => seconds,
            1 => seconds * 0.5,
            _ => 0.0,
        };
        if duration > 0.0 {
            self.controls.push(now);
        }
        duration
    }

    /// Pushes it `distance` m along the horizontal `direction`, unless a
    /// web or its post holds it. Returns whether it moved.
    pub fn push(&mut self, direction: Vec3, distance: f32, now: f32) -> bool {
        self.touched = now;
        let flat = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
        if self.anchored() || self.rooted(now) || flat == Vec3::ZERO {
            return false;
        }
        let mut to = self.pos + flat * distance;
        // Pushes stop at the meadow's edge, where the ground rises.
        let r = to.x.hypot(to.z);
        if r > super::MEADOW_RADIUS {
            to.x *= super::MEADOW_RADIUS / r;
            to.z *= super::MEADOW_RADIUS / r;
        }
        self.slide = Some(Slide {
            from: self.pos,
            to,
            t: 0.0,
        });
        true
    }

    /// Throws it upward at `speed` m/s, unless something holds it.
    pub fn lift(&mut self, speed: f32, now: f32) -> bool {
        self.touched = now;
        if self.anchored() || self.rooted(now) {
            return false;
        }
        self.vertical_speed = self.vertical_speed.max(f64::from(speed));
        true
    }

    /// Roots it for `seconds` at `now`, halved for each root in the last
    /// fifteen seconds; the third within them does nothing. Returns the
    /// duration applied.
    pub fn root(&mut self, seconds: f32, now: f32) -> f32 {
        self.touched = now;
        let duration = self.diminished(seconds, now);
        if duration > 0.0 {
            self.rooted_until = self.rooted_until.max(now + duration);
            self.slide = None;
        }
        duration
    }

    /// Back at its home, at full health, with nothing on it.
    pub fn reset(&mut self) {
        *self = Self::new(self.kind, [self.home.x, self.home.z]);
    }

    /// The footprint it blocks walking with, and its top, m.
    #[must_use]
    pub fn block(&self) -> (crate::controller::Footprint, f32) {
        let half = HALF * self.kind.scale();
        (
            crate::controller::Footprint {
                min: [self.pos.x - half, self.pos.z - half],
                max: [self.pos.x + half, self.pos.z + half],
            },
            self.top(),
        )
    }

    /// Advances its motion `dt` seconds at `now`: a push slides it, and it
    /// falls, or falls upward inside `gravity`'s cylinder, as the player
    /// does. Untouched for [`RESET_AFTER`], it resets.
    pub fn tick(&mut self, dt: f32, now: f32, gravity: Option<&reverse::Gravity>) {
        self.conditions.retain(|(_, until)| now < *until);
        let rooted = self.rooted(now);
        if let Some(slide) = &mut self.slide {
            slide.t += dt;
            let k = (slide.t / SLIDE).clamp(0.0, 1.0);
            // Eases out, as a shove that drags to a stop.
            let eased = 1.0 - (1.0 - k) * (1.0 - k);
            let at = slide.from.lerp(slide.to, eased);
            self.pos.x = at.x;
            self.pos.z = at.z;
            if k >= 1.0 {
                self.slide = None;
            }
        }
        let ground = if self.anchored() {
            self.home.y
        } else {
            height(self.pos.x, self.pos.z)
        };
        let lifted = gravity.is_some_and(|g| {
            g.active
                && g.cylinder.contains(DVec3::new(
                    f64::from(self.pos.x),
                    f64::from(self.pos.y),
                    f64::from(self.pos.z),
                ))
        });
        if self.anchored() {
            // The post holds it at its height, whatever gravity does.
            self.pos.y = self.home.y;
            self.vertical_speed = 0.0;
        } else {
            // A root holds a dummy's feet, not its fall: one rooted in the
            // air falls under ordinary gravity rather than hanging there,
            // and Reverse Gravity can't lift it.
            let gravity = if rooted { None } else { gravity };
            let steps = (f64::from(dt) / SUBSTEP).ceil().max(1.0);
            let h = f64::from(dt) / steps;
            let (mut y, mut v) = (f64::from(self.pos.y), self.vertical_speed);
            for _ in 0..steps as usize {
                let a = gravity.map_or(-GRAVITY, |g| {
                    reverse::creature_accel(
                        g,
                        DVec3::new(f64::from(self.pos.x), y, f64::from(self.pos.z)),
                        v,
                    )
                });
                v += a * h;
                y += v * h;
                if y <= f64::from(ground) {
                    y = f64::from(ground);
                    v = v.max(0.0);
                }
            }
            self.pos.y = y as f32;
            self.vertical_speed = v;
        }
        if lifted {
            self.touched = now;
        }
        let moved = self.pos.distance(self.home) > 0.01;
        let hurt = self.hp < self.kind.max_hp();
        let held = rooted || !self.conditions.is_empty();
        if (moved || hurt || held) && now - self.touched >= RESET_AFTER {
            self.reset();
        }
    }
}
