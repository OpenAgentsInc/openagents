//! The duel's rules: directional sword-and-board melee in real time.
//!
//! Both fighters run the same rules; only who chooses their commands
//! differs (the player's input or [`crate::ai`]). A blow is a timed motion,
//! a windup, a release, and a recovery, and the release moves a real blade
//! through space ([`crate::rig`]). Whatever that blade sweeps through first
//! ([`crate::hits`]) decides what happens: a shield blocks, a sword raised in
//! time parries, and a body zone takes the blow under its armour.
//!
//! Dice follow `docs/verse/combat-model.md` where they fit: an armour save
//! (a d20 against the blow's quality) decides whether mail, padding, or a
//! helm turns a blow, and the weapon's dice set the damage. Where geometry
//! decides instead of a die, such as whether a blow lands at all, the zone's
//! documentation says so (`docs/verse/duel.md`).
//!
//! Nothing here draws or reads input. [`Fight::tick`] steps at a fixed
//! 240 Hz, so a seed and the same commands replay the same fight.

use std::collections::VecDeque;
use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use verse_world::spells::dice::Dice;

use crate::ai::Brain;
use crate::hits::{self, Struck};
use crate::rig::{self, Rig};

/// The fixed simulation step, s.
pub const STEP: f32 = 1.0 / 240.0;
/// Radius of the trampled ground the fight stays on, m.
pub const GROUND_RADIUS: f32 = 13.0;
/// Where each fighter starts: the knight at the south, facing north.
pub const PLAYER_START: Vec3 = Vec3::new(0.0, 0.0, -3.2);
pub const ENEMY_START: Vec3 = Vec3::new(0.0, 0.0, 3.2);
/// How close two bodies may stand, center to center, m.
pub const BODY_GAP: f32 = 0.72;
/// The most stamina either fighter has.
pub const MAX_STAMINA: f32 = 100.0;
/// How long a callout over a hit stays up, s.
pub const CALLOUT_LIFE: f32 = 2.2;
/// Log lines the fight keeps.
pub const LOG_LINES: usize = 5;

/// When a parry catches a blow: from `PARRY_OPEN` to `PARRY_CLOSE` after
/// the parry starts, s. The whole motion takes `PARRY_TOTAL`.
pub const PARRY_OPEN: f32 = 0.06;
pub const PARRY_CLOSE: f32 = 0.26;
pub const PARRY_TOTAL: f32 = 0.55;
/// A parry turns blows that come from within this angle of the fighter's
/// facing, radians.
pub const PARRY_ARC: f32 = 1.35;
/// How long a riposte stays open after a parry, s, and what it gives:
/// a quicker windup, more damage, and a better chance to beat armour.
pub const RIPOSTE_WINDOW: f32 = 0.85;
pub const RIPOSTE_WINDUP: f32 = 0.6;
pub const RIPOSTE_DAMAGE: f32 = 1.25;
pub const RIPOSTE_EDGE: i32 = 2;

/// A shield bash: windup, the moment it can land, and the whole motion, s.
pub const BASH_LAND: (f32, f32) = (0.18, 0.32);
pub const BASH_TOTAL: f32 = 0.62;
/// A quick step: how long it takes, s, and how fast it carries, m/s.
pub const STEP_TIME: f32 = 0.3;
pub const STEP_SPEED: f32 = 4.2;

/// Stamina costs.
pub const FEINT_COST: f32 = 12.0;
pub const CANCEL_COST: f32 = 8.0;
pub const PARRY_COST: f32 = 6.0;
pub const BASH_COST: f32 = 15.0;
pub const STEP_COST: f32 = 12.0;
/// Below this stamina a fighter is tired: slower blows and less force.
pub const TIRED: f32 = 25.0;
/// Stamina returns this long after the last effort, s.
pub const REGEN_DELAY: f32 = 0.6;

/// Which fighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Player,
    Enemy,
}

impl Side {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Player => Self::Enemy,
            Self::Enemy => Self::Player,
        }
    }
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Player => 0,
            Self::Enemy => 1,
        }
    }
}

/// Who a fighter is: what they wear and carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kit {
    /// A brother of the Teutonic Order: great helm, mail hauberk over a
    /// padded gambeson, mail sleeves and chausses, white surcoat with the
    /// black cross, arming sword, and heater shield.
    Knight,
    /// An Old Prussian warrior: a riveted spangenhelm, a short mail shirt
    /// over a padded wool coat, wool trousers and leg wraps, a bearded axe,
    /// and a round linden shield.
    Prussian,
}

impl Kit {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Knight => "Brother Konrad",
            Self::Prussian => "Skurdo",
        }
    }
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Knight => "Teutonic knight",
            Self::Prussian => "Prussian warrior",
        }
    }
    #[must_use]
    pub const fn weapon(self) -> Weapon {
        match self {
            Self::Knight => Weapon::ARMING_SWORD,
            Self::Prussian => Weapon::BEARDED_AXE,
        }
    }
    /// Walking speeds forward, back, and sideways, m/s, and how fast the
    /// fighter can change them, m/s²: the knight carries more iron.
    #[must_use]
    pub const fn footwork(self) -> Footwork {
        match self {
            Self::Knight => Footwork {
                forward: 2.0,
                back: 1.5,
                side: 1.6,
                accel: 7.0,
                turn: 4.5,
            },
            Self::Prussian => Footwork {
                forward: 2.3,
                back: 1.8,
                side: 1.9,
                accel: 9.0,
                turn: 5.0,
            },
        }
    }
    /// Stamina back per second at rest, and spent per second holding the
    /// shield up.
    #[must_use]
    pub const fn wind(self) -> (f32, f32) {
        match self {
            Self::Knight => (17.0, 3.5),
            Self::Prussian => (21.0, 3.0),
        }
    }
    #[must_use]
    pub const fn health(self) -> f32 {
        80.0
    }
}

/// How a fighter moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footwork {
    pub forward: f32,
    pub back: f32,
    pub side: f32,
    pub accel: f32,
    /// How fast they turn to face their opponent in guard, rad/s.
    pub turn: f32,
}

/// The four directions of attack, named for where the blow comes from as
/// the attacker sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    /// A downward cut from above the head.
    Overhead,
    /// A cut from the attacker's left: a backhand for a right-handed
    /// fighter, which lands on the defender's right, the side the shield
    /// must reach across to cover.
    Left,
    /// A cut from the attacker's right: the forehand, which lands on the
    /// defender's shield side.
    Right,
    /// A straight thrust with the point.
    Thrust,
}

impl Dir {
    pub const ALL: [Self; 4] = [Self::Overhead, Self::Left, Self::Right, Self::Thrust];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Overhead => "Overhead",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Thrust => "Thrust",
        }
    }

    /// The kind of blow this direction delivers.
    #[must_use]
    pub const fn blow(self) -> Blow {
        match self {
            Self::Thrust => Blow::Thrust,
            _ => Blow::Cut,
        }
    }

    /// The direction a mouse movement of `(dx, dy)` picks (screen axes,
    /// y down), or `None` while it is too small to read.
    #[must_use]
    pub fn from_motion(dx: f32, dy: f32) -> Option<Self> {
        if dx.hypot(dy) < AIM_THRESHOLD {
            return None;
        }
        Some(if dx.abs() > dy.abs() {
            if dx > 0.0 { Self::Right } else { Self::Left }
        } else if dy < 0.0 {
            Self::Overhead
        } else {
            Self::Thrust
        })
    }
}

/// How far the mouse must travel, in points, to pick a direction.
pub const AIM_THRESHOLD: f32 = 14.0;

/// What a blow does to flesh and armour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blow {
    Cut,
    Thrust,
}

/// A weapon's reach, dice, and weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    pub name: &'static str,
    /// From the grip to the point or the axe's edge, m.
    pub length: f32,
    /// Where along the length it strikes from, as fractions: a sword's
    /// whole blade, an axe's head only.
    pub strikes: (f32, f32),
    /// Damage dice: count, sides, and a flat bonus, for cuts and thrusts.
    pub cut: (u32, u32, i32),
    pub thrust: (u32, u32, i32),
    /// Added to the blow's quality against an armour save.
    pub edge: i32,
    /// How hard it is on a shield: stamina a block costs, times this.
    pub on_shield: f32,
    /// How far forward the body travels into a blow, m.
    pub lunge: f32,
    /// Windups and releases take this long, times the base.
    pub speed: f32,
}

impl Weapon {
    /// A single-handed arming sword of the 13th century, about 76 cm of
    /// blade: fine point, good edge.
    pub const ARMING_SWORD: Self = Self {
        name: "arming sword",
        length: 0.9,
        strikes: (0.35, 1.0),
        cut: (2, 6, 3),
        thrust: (2, 6, 4),
        edge: 2,
        on_shield: 1.0,
        lunge: 0.28,
        speed: 1.0,
    };
    /// A bearded axe on a 75 cm haft: it bites shields and lands hard, but
    /// its jab with the head's top is short and weak.
    pub const BEARDED_AXE: Self = Self {
        name: "bearded axe",
        length: 0.78,
        strikes: (0.78, 1.0),
        cut: (2, 6, 5),
        thrust: (1, 6, 2),
        edge: 1,
        on_shield: 1.4,
        lunge: 0.36,
        speed: 0.92,
    };
}

/// A blow's three phases, s, and what starting it costs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub windup: f32,
    pub release: f32,
    pub recover: f32,
    pub cost: f32,
    /// Stamina a shield block of this blow costs the defender.
    pub on_shield: f32,
}

/// The timing of `dir` with `weapon`.
#[must_use]
pub fn timing(dir: Dir, weapon: Weapon) -> Timing {
    let base = match dir {
        Dir::Overhead => Timing {
            windup: 0.55,
            release: 0.27,
            recover: 0.42,
            cost: 16.0,
            on_shield: 18.0,
        },
        Dir::Left | Dir::Right => Timing {
            windup: 0.46,
            release: 0.25,
            recover: 0.38,
            cost: 13.0,
            on_shield: 14.0,
        },
        Dir::Thrust => Timing {
            windup: 0.4,
            release: 0.2,
            recover: 0.36,
            cost: 11.0,
            on_shield: 10.0,
        },
    };
    Timing {
        windup: base.windup * weapon.speed,
        release: base.release * weapon.speed,
        ..base
    }
}

/// The body's hit zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    Head,
    Torso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl Zone {
    pub const ALL: [Self; 6] = [
        Self::Head,
        Self::Torso,
        Self::LeftArm,
        Self::RightArm,
        Self::LeftLeg,
        Self::RightLeg,
    ];
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Torso => "Body",
            Self::LeftArm => "Shield arm",
            Self::RightArm => "Weapon arm",
            Self::LeftLeg => "Left leg",
            Self::RightLeg => "Right leg",
        }
    }
    /// Damage times this when a blow gets through.
    #[must_use]
    pub const fn weight(self) -> f32 {
        match self {
            Self::Head => 1.6,
            Self::Torso => 1.0,
            Self::LeftArm | Self::RightArm => 0.6,
            Self::LeftLeg | Self::RightLeg => 0.7,
        }
    }
    /// How long a blow that gets through rocks the fighter, s.
    #[must_use]
    pub const fn stagger(self) -> f32 {
        match self {
            Self::Head => 0.6,
            Self::Torso => 0.35,
            Self::LeftArm | Self::RightArm => 0.2,
            Self::LeftLeg | Self::RightLeg => 0.3,
        }
    }
}

/// What covers one zone, and how well it does against cuts and thrusts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Armour {
    /// What the player reads: "mail", "the helm", "padding".
    pub name: &'static str,
    /// Added to the d20 armour save against a cut and a thrust. A save of
    /// 20 or more turns the blow.
    pub cut: i32,
    pub thrust: i32,
    /// What part of the damage still gets through when the save fails.
    pub through_cut: f32,
    pub through_thrust: f32,
}

impl Armour {
    pub const NONE: Self = Self {
        name: "cloth",
        cut: -20,
        thrust: -20,
        through_cut: 1.0,
        through_thrust: 1.0,
    };
    /// Riveted mail over a padded gambeson: it turns nearly every cut, but
    /// a good thrust can burst its rings.
    pub const MAIL_PADDED: Self = Self {
        name: "mail",
        cut: 13,
        thrust: 5,
        through_cut: 0.45,
        through_thrust: 0.75,
    };
    /// Mail alone, as on sleeves and chausses.
    pub const MAIL: Self = Self {
        name: "mail",
        cut: 11,
        thrust: 3,
        through_cut: 0.55,
        through_thrust: 0.85,
    };
    /// A flat-topped great helm over a mail coif: only its eye slits let a
    /// point in.
    pub const GREAT_HELM: Self = Self {
        name: "the helm",
        cut: 15,
        thrust: 9,
        through_cut: 0.35,
        through_thrust: 0.6,
    };
    /// A riveted spangenhelm with a nasal: the face below it is open.
    pub const SPANGENHELM: Self = Self {
        name: "the helmet",
        cut: 8,
        thrust: 4,
        through_cut: 0.6,
        through_thrust: 0.8,
    };
    /// A thick padded wool coat: fair against cuts, poor against points.
    pub const PADDED: Self = Self {
        name: "padding",
        cut: 5,
        thrust: 1,
        through_cut: 0.75,
        through_thrust: 0.95,
    };
    /// Wool trousers under wound leg wraps.
    pub const WRAPS: Self = Self {
        name: "leg wraps",
        cut: 2,
        thrust: -2,
        through_cut: 0.9,
        through_thrust: 1.0,
    };

    #[must_use]
    pub const fn save(self, blow: Blow) -> i32 {
        match blow {
            Blow::Cut => self.cut,
            Blow::Thrust => self.thrust,
        }
    }
    #[must_use]
    pub const fn through(self, blow: Blow) -> f32 {
        match blow {
            Blow::Cut => self.through_cut,
            Blow::Thrust => self.through_thrust,
        }
    }
}

/// What `kit` wears on `zone`.
#[must_use]
pub const fn armour(kit: Kit, zone: Zone) -> Armour {
    match (kit, zone) {
        (Kit::Knight, Zone::Head) => Armour::GREAT_HELM,
        (Kit::Knight, Zone::Torso) => Armour::MAIL_PADDED,
        (Kit::Knight, _) => Armour::MAIL,
        (Kit::Prussian, Zone::Head) => Armour::SPANGENHELM,
        (Kit::Prussian, Zone::Torso) => Armour::MAIL_PADDED,
        (Kit::Prussian, Zone::LeftArm | Zone::RightArm) => Armour::PADDED,
        (Kit::Prussian, _) => Armour::WRAPS,
    }
}

/// The armour save's target: a d20 plus the armour's bonus at or above
/// this turns the blow.
pub const ARMOUR_DC: i32 = 20;

/// The chance `armour` turns a `blow` of `quality` (the weapon's edge and
/// any riposte bonus): a d20 plus the armour's bonus, less the quality,
/// at or above [`ARMOUR_DC`], except on a natural 1, which finds a gap.
#[must_use]
pub fn turn_chance(armour: Armour, blow: Blow, quality: i32) -> f32 {
    let need = ARMOUR_DC - (armour.save(blow) - quality);
    // Faces 2 to 20 can turn it; a 1 never does.
    let faces = (21 - need.max(2)).clamp(0, 19);
    faces as f32 / 20.0
}

/// Bleeding a blow that gets through starts: hit points per second and how
/// long, by zone and blow.
#[must_use]
pub const fn bleed(zone: Zone, blow: Blow) -> (f32, f32) {
    match (zone, blow) {
        (Zone::Torso, Blow::Thrust) => (1.6, 6.0),
        (Zone::Torso, Blow::Cut) => (1.0, 5.0),
        (Zone::Head, _) => (1.2, 5.0),
        (Zone::LeftLeg | Zone::RightLeg, _) => (0.9, 5.0),
        (_, Blow::Thrust) => (0.6, 4.0),
        _ => (0.7, 4.0),
    }
}

/// What a fighter is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Standing in guard, free to act.
    Guard,
    /// Drawing back for a blow in `dir`; `dur` is the whole windup. A
    /// feint changes `dir` and restarts part of it.
    Windup { dir: Dir, dur: f32, feints: u8 },
    /// The blow in motion; `dur` is its whole release.
    Release { dir: Dir, dur: f32 },
    /// Coming back to guard after a blow, from release progress `from`.
    Recover {
        dir: Dir,
        from: f32,
        dur: f32,
        why: After,
    },
    /// Meeting a blow with the blade.
    Parry,
    /// Driving the shield forward.
    Bash { landed: bool },
    /// A quick step in `dir` (x right, y forward, a unit vector).
    Step { dir: Vec2 },
    /// Rocked off balance and unable to act for `dur`.
    Stagger { dur: f32 },
    /// Fallen.
    Down,
}

/// Why a blow ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Missed,
    Landed,
    Blocked,
    Parried,
}

impl After {
    /// Recovery after this ending, times the blow's own.
    #[must_use]
    pub const fn factor(self) -> f32 {
        match self {
            Self::Missed => 1.45,
            Self::Landed => 1.0,
            Self::Blocked => 1.25,
            Self::Parried => 2.0,
        }
    }
}

/// One bleeding wound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bleed {
    pub rate: f32,
    pub until: f32,
}

/// What a fighter is told to do this step by the player or the AI.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Steer {
    /// Forward (1) or back (-1), toward the opponent.
    pub ahead: f32,
    /// Right (1) or left (-1), circling.
    pub side: f32,
    /// The shield is held up.
    pub shield: bool,
}

/// One-off commands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Attack(Dir),
    Parry,
    Bash,
    /// A quick step: x right, y forward.
    Step(Vec2),
    /// Pulls a windup back into guard.
    Cancel,
}

/// Why a command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    Tired,
    ShieldDown,
    Over,
}

impl Refusal {
    /// The line the screen shows.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Busy => "Not yet",
            Self::Tired => "Too tired",
            Self::ShieldDown => "Your shield arm needs a moment",
            Self::Over => "The fight is over. Press R to fight again",
        }
    }
}

/// One fighter.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub side: Side,
    pub kit: Kit,
    /// Feet, m.
    pub pos: Vec3,
    /// Ground velocity, m/s.
    pub vel: Vec3,
    /// Facing, radians: forward is `(sin yaw, 0, cos yaw)`.
    pub yaw: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub stamina: f32,
    pub action: Action,
    /// When the current action started, fight time, s.
    pub since: f32,
    pub steer: Steer,
    /// How far the shield is raised, 0 to 1.
    pub raise: f32,
    /// Where the raised shield covers: radians toward the shield side
    /// (left) when positive, toward the weapon side when negative.
    pub cover: f32,
    /// How high it covers, 0 at the chest to 1 over the head.
    pub high: f32,
    /// The opponent's blow this fighter has seen: its direction and when
    /// they saw it. The shield follows it.
    pub read: Option<Dir>,
    pub wounds_arm: u8,
    pub wounds_leg: u8,
    pub bleeds: Vec<Bleed>,
    /// The last time they spent stamina, for its return.
    pub exerted: f32,
    pub hit_at: f32,
    pub riposte_until: f32,
    pub shield_locked_until: f32,
    /// Distance walked, m, for the stride.
    pub stride: f32,
    /// Damage dealt over the fight.
    pub dealt: f32,
}

impl Fighter {
    #[must_use]
    pub fn new(side: Side, kit: Kit, pos: Vec3, yaw: f32) -> Self {
        Self {
            side,
            kit,
            pos,
            vel: Vec3::ZERO,
            yaw,
            hp: kit.health(),
            max_hp: kit.health(),
            stamina: MAX_STAMINA,
            action: Action::Guard,
            since: 0.0,
            steer: Steer::default(),
            raise: 0.0,
            cover: 0.15,
            high: 0.0,
            read: None,
            wounds_arm: 0,
            wounds_leg: 0,
            bleeds: Vec::new(),
            exerted: -10.0,
            hit_at: -10.0,
            riposte_until: -10.0,
            shield_locked_until: -10.0,
            stride: 0.0,
            dealt: 0.0,
        }
    }

    #[must_use]
    pub fn alive(&self) -> bool {
        self.action != Action::Down
    }

    #[must_use]
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    #[must_use]
    pub fn right(&self) -> Vec3 {
        Vec3::new(-self.yaw.cos(), 0.0, self.yaw.sin())
    }

    #[must_use]
    pub fn tired(&self) -> bool {
        self.stamina < TIRED
    }

    /// Seconds into the current action.
    #[must_use]
    pub fn elapsed(&self, now: f32) -> f32 {
        (now - self.since).max(0.0)
    }

    /// The blow being wound up or released, and its progress, 0 to 1.
    #[must_use]
    pub fn threat(&self, now: f32) -> Option<(Dir, bool, f32)> {
        let t = self.elapsed(now);
        match self.action {
            Action::Windup { dir, dur, .. } => Some((dir, false, (t / dur).min(1.0))),
            Action::Release { dir, dur } => Some((dir, true, (t / dur).min(1.0))),
            _ => None,
        }
    }

    /// Whether the parry's blade is up to catch a blow now.
    #[must_use]
    pub fn parrying(&self, now: f32) -> bool {
        self.action == Action::Parry && (PARRY_OPEN..=PARRY_CLOSE).contains(&self.elapsed(now))
    }

    /// Whether the shield is up as a guard rather than hanging at the side.
    #[must_use]
    pub fn guarding(&self) -> bool {
        self.raise > 0.6
    }

    /// The hit points lost per second to bleeding now.
    #[must_use]
    pub fn bleeding(&self, now: f32) -> f32 {
        self.bleeds
            .iter()
            .filter(|b| b.until > now)
            .map(|b| b.rate)
            .sum()
    }

    fn set(&mut self, action: Action, now: f32) {
        self.action = action;
        self.since = now;
    }

    fn spend(&mut self, cost: f32, now: f32) {
        self.stamina = (self.stamina - cost).max(0.0);
        self.exerted = now;
    }

    /// A windup's length for `dir` now: slower when tired, quicker in a
    /// riposte.
    #[must_use]
    pub fn windup(&self, dir: Dir, now: f32) -> f32 {
        let mut d = timing(dir, self.kit.weapon()).windup;
        if self.tired() {
            d *= 1.2;
        }
        if now < self.riposte_until {
            d *= RIPOSTE_WINDUP;
        }
        d
    }

    /// Whether the fighter can act from what they are doing now.
    fn free(&self, now: f32) -> bool {
        match self.action {
            Action::Guard => true,
            // The last part of a recovery flows into the next motion.
            Action::Recover { dur, .. } => self.elapsed(now) > dur * 0.75,
            _ => false,
        }
    }
}

/// How the fight stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Fighting,
    Won,
    Lost,
}

/// How a callout reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Good for the player.
    Good,
    /// Bad for the player.
    Bad,
    Plain,
}

/// A short line over where something happened.
#[derive(Clone, Debug, PartialEq)]
pub struct Callout {
    pub text: String,
    pub at: Vec3,
    pub born: f32,
    pub tone: Tone,
    pub big: bool,
}

/// What a contact looked like, for the flash drawn there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spark {
    /// Steel on steel or on mail.
    Steel,
    /// A blade biting a wooden shield.
    Wood,
    /// A blow that got through.
    Flesh,
}

/// A flash where a blow met something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flash {
    pub at: Vec3,
    pub born: f32,
    pub kind: Spark,
    pub strength: f32,
}

/// How long a flash lasts, s.
pub const FLASH_LIFE: f32 = 0.28;

/// Something that happened, for the log, the tests, and the film.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Blocked {
        by: Side,
        dir: Dir,
    },
    GuardBroken {
        by: Side,
    },
    Parried {
        by: Side,
        dir: Dir,
    },
    Hit {
        on: Side,
        zone: Zone,
        blow: Blow,
        turned: bool,
        damage: f32,
    },
    Missed {
        by: Side,
    },
    Feint {
        by: Side,
        to: Dir,
    },
    Bashed {
        on: Side,
        guarded: bool,
    },
    Fell {
        side: Side,
    },
}

/// The duel.
#[derive(Clone, Debug)]
pub struct Fight {
    pub now: f32,
    pub fighters: [Fighter; 2],
    pub outcome: Outcome,
    pub dice: Dice,
    pub brain: Brain,
    /// Whether Skurdo chooses for himself; a test turns him off to command
    /// both fighters.
    pub ai: bool,
    /// Holds each fighter's read of the other's blow where it is set, as a
    /// test does to stage a guard on the wrong side.
    pub hold_reads: bool,
    /// Holds each fighter's facing, as a test does to stage a blow from
    /// the flank.
    pub hold_facing: bool,
    pub callouts: Vec<Callout>,
    pub flashes: Vec<Flash>,
    pub log: VecDeque<String>,
    pub events: Vec<Event>,
    /// Hit-stop left, s: the fight holds still for a heartbeat on a
    /// heavy blow.
    pub freeze: f32,
    /// Camera shake: its strength and when it started.
    pub shake: (f32, f32),
    /// Each fighter's pose at the last step.
    pub rigs: [Rig; 2],
    /// Time not yet stepped, s.
    pending: f32,
    /// When each fighter's read of the other's blow lands: the direction
    /// and the time they take it in.
    reads: [Option<(Dir, f32)>; 2],
}

/// How long the player's shield takes to follow a blow they can see
/// coming, s: the arm's own reaction, short because the player chose to
/// hold the shield up.
pub const PLAYER_READ: f32 = 0.1;

impl Fight {
    /// A fresh duel whose dice start from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let player = Fighter::new(Side::Player, Kit::Knight, PLAYER_START, 0.0);
        let enemy = Fighter::new(Side::Enemy, Kit::Prussian, ENEMY_START, PI);
        let rigs = [rig::rig(&player, 0.0), rig::rig(&enemy, 0.0)];
        let mut fight = Self {
            now: 0.0,
            fighters: [player, enemy],
            outcome: Outcome::Fighting,
            dice: Dice::new(seed),
            brain: Brain::new(seed ^ 0xB4A1_7E5D),
            ai: true,
            hold_reads: false,
            hold_facing: false,
            callouts: Vec::new(),
            flashes: Vec::new(),
            log: VecDeque::new(),
            events: Vec::new(),
            freeze: 0.0,
            shake: (0.0, -10.0),
            rigs,
            pending: 0.0,
            reads: [None, None],
        };
        fight.note(
            "Konrad of the Order faces Skurdo of Sambia. Raise your shield and watch his axe.",
        );
        fight
    }

    #[must_use]
    pub fn player(&self) -> &Fighter {
        &self.fighters[0]
    }
    #[must_use]
    pub fn enemy(&self) -> &Fighter {
        &self.fighters[1]
    }
    pub fn fighter_mut(&mut self, side: Side) -> &mut Fighter {
        &mut self.fighters[side.index()]
    }

    fn note(&mut self, line: impl Into<String>) {
        self.log.push_back(line.into());
        while self.log.len() > LOG_LINES {
            self.log.pop_front();
        }
    }

    fn callout(&mut self, text: impl Into<String>, at: Vec3, tone: Tone, big: bool) {
        self.callouts.push(Callout {
            text: text.into(),
            at,
            born: self.now,
            tone,
            big,
        });
    }

    fn flash(&mut self, at: Vec3, kind: Spark, strength: f32) {
        self.flashes.push(Flash {
            at,
            born: self.now,
            kind,
            strength,
        });
    }

    /// A heavy moment: a short hold and a jolt of the camera.
    fn heavy(&mut self, freeze: f32, shake: f32) {
        self.freeze = self.freeze.max(freeze);
        if shake >= self.shake_now() {
            self.shake = (shake, self.now);
        }
    }

    /// The camera shake's strength now, m.
    #[must_use]
    pub fn shake_now(&self) -> f32 {
        let (amp, at) = self.shake;
        let t = self.now - at;
        if t < 0.0 { 0.0 } else { amp * (-t * 9.0).exp() }
    }

    /// Steers `side` for the coming steps.
    pub fn steer(&mut self, side: Side, steer: Steer) {
        self.fighters[side.index()].steer = steer;
    }

    /// Gives `side` a command.
    ///
    /// # Errors
    ///
    /// The fighter is busy, tired, or the fight is over.
    pub fn command(&mut self, side: Side, command: Command) -> Result<(), Refusal> {
        if self.outcome != Outcome::Fighting {
            return Err(Refusal::Over);
        }
        let now = self.now;
        let f = &mut self.fighters[side.index()];
        if !f.alive() {
            return Err(Refusal::Over);
        }
        match command {
            Command::Attack(dir) => {
                let cost = timing(dir, f.kit.weapon()).cost;
                match f.action {
                    // Changing direction in the windup is a feint.
                    Action::Windup {
                        dir: old,
                        dur,
                        feints,
                    } if old != dir => {
                        if f.stamina < FEINT_COST {
                            return Err(Refusal::Tired);
                        }
                        f.spend(FEINT_COST, now);
                        let windup = f.windup(dir, now);
                        // The new blow keeps half of the old one's
                        // progress: a feint is quick, not free.
                        let kept = (f.elapsed(now) / dur).min(1.0) * 0.5;
                        f.set(
                            Action::Windup {
                                dir,
                                dur: windup,
                                feints: feints.saturating_add(1),
                            },
                            now - kept * windup,
                        );
                        self.events.push(Event::Feint { by: side, to: dir });
                        Ok(())
                    }
                    Action::Windup { .. } => Err(Refusal::Busy),
                    _ if !f.free(now) => Err(Refusal::Busy),
                    _ if f.stamina < cost.min(TIRED * 0.4) || f.stamina <= 0.0 => {
                        Err(Refusal::Tired)
                    }
                    _ => {
                        f.spend(cost, now);
                        let dur = f.windup(dir, now);
                        f.set(
                            Action::Windup {
                                dir,
                                dur,
                                feints: 0,
                            },
                            now,
                        );
                        Ok(())
                    }
                }
            }
            Command::Cancel => match f.action {
                Action::Windup { .. } => {
                    if f.stamina < CANCEL_COST {
                        return Err(Refusal::Tired);
                    }
                    f.spend(CANCEL_COST, now);
                    f.set(Action::Guard, now);
                    Ok(())
                }
                _ => Err(Refusal::Busy),
            },
            Command::Parry => {
                if !f.free(now) {
                    return Err(Refusal::Busy);
                }
                if f.stamina < PARRY_COST {
                    return Err(Refusal::Tired);
                }
                f.spend(PARRY_COST, now);
                f.set(Action::Parry, now);
                Ok(())
            }
            Command::Bash => {
                if !f.free(now) {
                    return Err(Refusal::Busy);
                }
                if now < f.shield_locked_until {
                    return Err(Refusal::ShieldDown);
                }
                if f.stamina < BASH_COST {
                    return Err(Refusal::Tired);
                }
                f.spend(BASH_COST, now);
                f.set(Action::Bash { landed: false }, now);
                Ok(())
            }
            Command::Step(dir) => {
                if !f.free(now) {
                    return Err(Refusal::Busy);
                }
                if f.stamina < STEP_COST {
                    return Err(Refusal::Tired);
                }
                let dir = if dir.length_squared() < 1e-4 {
                    Vec2::new(0.0, -1.0)
                } else {
                    dir.normalize()
                };
                f.spend(STEP_COST, now);
                f.vel = (f.right() * dir.x + f.forward() * dir.y) * STEP_SPEED;
                f.set(Action::Step { dir }, now);
                Ok(())
            }
        }
    }

    /// Advances the fight by `dt` of wall time, in fixed steps, holding
    /// still through a hit-stop.
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let mut dt = dt.min(0.1);
        if self.freeze > 0.0 {
            let held = self.freeze.min(dt);
            self.freeze -= held;
            dt -= held;
        }
        self.pending += dt;
        while self.pending >= STEP {
            self.pending -= STEP;
            self.step();
        }
        let now = self.now;
        self.callouts.retain(|c| now - c.born < CALLOUT_LIFE);
        self.flashes.retain(|f| now - f.born < FLASH_LIFE);
    }

    /// One fixed step of both fighters, the blades, and the AI.
    pub fn step(&mut self) {
        let h = STEP;
        if self.ai && self.outcome == Outcome::Fighting {
            crate::ai::think(self);
        }
        self.now += h;
        let now = self.now;
        for side in [Side::Player, Side::Enemy] {
            if !self.hold_reads {
                self.perceive(side);
            }
            let yaw = self.fighters[side.index()].yaw;
            self.advance(side, h);
            if self.hold_facing {
                self.fighters[side.index()].yaw = yaw;
            }
        }
        self.separate();
        let previous = self.rigs.clone();
        self.rigs = [
            rig::rig(&self.fighters[0], now),
            rig::rig(&self.fighters[1], now),
        ];
        if self.outcome == Outcome::Fighting {
            for side in [Side::Player, Side::Enemy] {
                self.strike(side, &previous[side.index()]);
                self.bash(side);
            }
        }
        for side in [Side::Player, Side::Enemy] {
            self.bleed_out(side, h);
        }
    }

    /// Updates what `side` has seen of the other's blow: the shield follows
    /// a read once the reaction time has passed.
    fn perceive(&mut self, side: Side) {
        let now = self.now;
        let threat = self.fighters[side.other().index()]
            .threat(now)
            .map(|(dir, ..)| dir);
        let i = side.index();
        match (threat, self.reads[i]) {
            (None, _) => {
                self.reads[i] = None;
                self.fighters[i].read = None;
            }
            (Some(dir), Some((seen, at))) if seen == dir => {
                if now >= at {
                    self.fighters[i].read = Some(dir);
                }
            }
            (Some(dir), _) => {
                let delay = match side {
                    Side::Player => PLAYER_READ,
                    Side::Enemy => self.brain.reaction(),
                };
                self.reads[i] = Some((dir, now + delay));
            }
        }
    }

    /// Moves `side` one step: its action's clock, its shield, its stamina,
    /// its feet, and its facing.
    fn advance(&mut self, side: Side, h: f32) {
        let now = self.now;
        let other = self.fighters[side.other().index()].pos;
        let f = &mut self.fighters[side.index()];
        let weapon = f.kit.weapon();
        let t = f.elapsed(now);
        // The action's clock.
        match f.action {
            Action::Windup { dir, dur, feints } if t >= dur => {
                let release = timing(dir, weapon).release;
                // Stepping into the blow: the body's weight goes with it.
                let lunge = weapon.lunge / release * 1.15;
                let fwd = f.forward();
                f.vel = f.vel * 0.4 + fwd * lunge;
                f.set(Action::Release { dir, dur: release }, now);
                let _ = feints;
            }
            Action::Release { dir, dur } if t >= dur => {
                let recover = timing(dir, weapon).recover * After::Missed.factor();
                f.set(
                    Action::Recover {
                        dir,
                        from: 1.0,
                        dur: recover,
                        why: After::Missed,
                    },
                    now,
                );
                self.events.push(Event::Missed { by: side });
            }
            Action::Recover { dur, .. } | Action::Stagger { dur } if t >= dur => {
                f.set(Action::Guard, now);
            }
            Action::Parry if t >= PARRY_TOTAL => f.set(Action::Guard, now),
            Action::Bash { .. } if t >= BASH_TOTAL => f.set(Action::Guard, now),
            Action::Step { .. } if t >= STEP_TIME => f.set(Action::Guard, now),
            _ => {}
        }
        let f = &mut self.fighters[side.index()];
        let alive = f.alive();
        // The shield: up while held and the arm is able, half down in the
        // swing, forward in a bash.
        let able = alive && now >= f.shield_locked_until && f.stamina > 0.0;
        let want = match f.action {
            Action::Bash { .. } => 1.0,
            Action::Stagger { .. } | Action::Down => 0.0,
            Action::Release { .. } => {
                if f.steer.shield && able {
                    0.4
                } else {
                    0.0
                }
            }
            _ if f.steer.shield && able => 1.0,
            _ => 0.0,
        };
        let rate = if want > f.raise { 5.5 } else { 4.0 };
        f.raise += (want - f.raise).clamp(-rate * h, rate * h);
        // Where it covers: toward the blow the fighter has read, or a
        // little to the shield side when nothing comes. It also turns to
        // face an opponent who stands off to one side.
        let to = other - f.pos;
        let bearing = {
            let fwd = f.forward();
            let r = f.right();
            // Positive when the opponent stands to the fighter's left.
            (-to.dot(r)).atan2(to.dot(fwd).max(1e-3))
        };
        let (cover, high) = match f.read {
            Some(Dir::Right) => (0.8, 0.25),
            Some(Dir::Left) => (-0.75, 0.25),
            Some(Dir::Overhead) => (0.1, 1.0),
            Some(Dir::Thrust) => (0.05, 0.15),
            None => (0.15, 0.1),
        };
        let cover = (cover + bearing).clamp(COVER_MIN, COVER_MAX);
        // The shield swings out to its own side faster than it reaches
        // across the body.
        let rate = if cover > f.cover { 7.0 } else { 3.6 };
        f.cover += (cover - f.cover).clamp(-rate * h, rate * h);
        f.high += (high - f.high).clamp(-5.0 * h, 5.0 * h);
        // Stamina: spent holding the shield up, back at rest.
        let (regen, drain) = f.kit.wind();
        if f.guarding() && alive {
            f.stamina = (f.stamina - drain * h).max(0.0);
            f.exerted = now;
            if f.stamina <= 0.0 && f.steer.shield {
                f.shield_locked_until = now + 0.8;
            }
        } else if alive
            && now - f.exerted > REGEN_DELAY
            && matches!(f.action, Action::Guard | Action::Recover { .. })
        {
            let moving = f.vel.length() > 1.0;
            let k = if moving { 0.6 } else { 1.0 };
            f.stamina = (f.stamina + regen * k * h).min(MAX_STAMINA);
        }
        // The feet.
        let foot = f.kit.footwork();
        let pace = match f.action {
            Action::Guard => 1.0,
            Action::Windup { .. } => 0.75,
            Action::Release { .. } => 0.0,
            Action::Recover { .. } => 0.7,
            Action::Parry | Action::Bash { .. } => 0.5,
            Action::Step { .. } | Action::Stagger { .. } | Action::Down => 0.0,
        } * if f.guarding() { 0.8 } else { 1.0 }
            * (1.0 - 0.15 * f32::from(f.wounds_leg.min(2)));
        let controlled = matches!(
            f.action,
            Action::Guard
                | Action::Windup { .. }
                | Action::Recover { .. }
                | Action::Parry
                | Action::Bash { .. }
        );
        if controlled && alive {
            let ahead = f.steer.ahead.clamp(-1.0, 1.0);
            let sideways = f.steer.side.clamp(-1.0, 1.0);
            let along = if ahead >= 0.0 {
                ahead * foot.forward
            } else {
                ahead * foot.back
            };
            let mut want = f.forward() * along + f.right() * sideways * foot.side;
            if want.length() > foot.forward {
                want = want.normalize() * foot.forward;
            }
            let want = want * pace;
            let dv = want - f.vel;
            let max = foot.accel * h;
            f.vel += if dv.length() > max {
                dv.normalize() * max
            } else {
                dv
            };
        } else {
            // A lunge, a step, a stagger, or a fall carries on and slows.
            let k = match f.action {
                Action::Release { .. } => 3.5,
                Action::Step { .. } => 4.0,
                _ => 6.0,
            };
            f.vel *= (-k * h).exp();
        }
        f.pos += f.vel * h;
        f.pos.y = 0.0;
        let flat = Vec2::new(f.pos.x, f.pos.z);
        if flat.length() > GROUND_RADIUS {
            let back = flat.normalize() * GROUND_RADIUS;
            f.pos.x = back.x;
            f.pos.z = back.y;
        }
        f.stride += f.vel.length() * h;
        // Facing: they keep their eyes on each other, turning at a human
        // rate, slowly while a blow is in motion.
        let turn = match f.action {
            Action::Guard | Action::Parry | Action::Step { .. } => foot.turn,
            Action::Windup { .. } | Action::Bash { .. } => foot.turn * 0.7,
            Action::Release { .. } => 1.6,
            Action::Recover { .. } => foot.turn * 0.5,
            Action::Stagger { .. } => 0.8,
            Action::Down => 0.0,
        };
        if to.length_squared() > 1e-4 {
            let want = to.x.atan2(to.z);
            let mut d = want - f.yaw;
            while d > PI {
                d -= 2.0 * PI;
            }
            while d < -PI {
                d += 2.0 * PI;
            }
            f.yaw += d.clamp(-turn * h, turn * h);
        }
    }

    /// Keeps the two bodies apart.
    fn separate(&mut self) {
        let [a, b] = &mut self.fighters;
        let d = Vec2::new(b.pos.x - a.pos.x, b.pos.z - a.pos.z);
        let len = d.length();
        if len < BODY_GAP && len > 1e-5 {
            let push = (BODY_GAP - len) * 0.5;
            let n = d / len;
            a.pos.x -= n.x * push;
            a.pos.z -= n.y * push;
            b.pos.x += n.x * push;
            b.pos.z += n.y * push;
        }
    }

    /// Sweeps `side`'s blade from where it was to where it is now, and
    /// settles what it met first.
    fn strike(&mut self, side: Side, previous: &Rig) {
        let now = self.now;
        let attacker = &self.fighters[side.index()];
        let Action::Release { dir, dur } = attacker.action else {
            return;
        };
        let progress = (attacker.elapsed(now) / dur).min(1.0);
        let weapon = attacker.kit.weapon();
        let defender = &self.fighters[side.other().index()];
        if !defender.alive() {
            return;
        }
        let rig_d = &self.rigs[side.other().index()];
        let Some(contact) = hits::sweep(
            rig_d,
            defender.kit,
            previous,
            &self.rigs[side.index()],
            weapon,
        ) else {
            return;
        };
        self.resolve(side, dir, progress, contact);
    }

    fn resolve(&mut self, side: Side, dir: Dir, progress: f32, contact: hits::Contact) {
        let now = self.now;
        let (ai, di) = (side.index(), side.other().index());
        let weapon = self.fighters[ai].kit.weapon();
        let base = timing(dir, weapon);
        let to_attacker = self.fighters[ai].pos - self.fighters[di].pos;
        let defender_fwd = self.fighters[di].forward();
        let off = to_attacker
            .with_y(0.0)
            .normalize_or_zero()
            .dot(defender_fwd)
            .clamp(-1.0, 1.0)
            .acos();
        let push = (self.fighters[di].pos - self.fighters[ai].pos)
            .with_y(0.0)
            .normalize_or_zero();
        let mine = side == Side::Player;
        let tone_for = |good_for_attacker: bool| {
            if good_for_attacker == mine {
                Tone::Good
            } else {
                Tone::Bad
            }
        };
        let riposte = now < self.fighters[ai].riposte_until;
        // A blade raised in time meets the blow first.
        if self.fighters[di].parrying(now) && off <= PARRY_ARC {
            let a = &mut self.fighters[ai];
            a.set(
                Action::Recover {
                    dir,
                    from: progress,
                    dur: base.recover * After::Parried.factor(),
                    why: After::Parried,
                },
                now,
            );
            a.spend(10.0, now);
            a.vel = -push * 1.2;
            let d = &mut self.fighters[di];
            d.stamina = (d.stamina + PARRY_COST).min(MAX_STAMINA);
            d.riposte_until = now + RIPOSTE_WINDOW;
            d.set(Action::Guard, now);
            self.events.push(Event::Parried {
                by: side.other(),
                dir,
            });
            self.flash(contact.at, Spark::Steel, 1.0);
            self.callout("Parried!", contact.at, tone_for(false), true);
            self.note(if mine {
                "Skurdo parried you. Watch for his answer.".to_owned()
            } else {
                "You parried. Strike back now.".to_owned()
            });
            self.heavy(0.07, 0.05);
            return;
        }
        match contact.struck {
            Struck::Shield => {
                let raised = self.fighters[di].guarding();
                let mut cost = base.on_shield * weapon.on_shield;
                if !raised {
                    cost *= 1.6;
                }
                if riposte {
                    cost *= 1.2;
                }
                if self.fighters[ai].tired() {
                    cost *= 0.85;
                }
                let broken = self.fighters[di].stamina < cost;
                let a = &mut self.fighters[ai];
                a.set(
                    Action::Recover {
                        dir,
                        from: progress,
                        dur: base.recover * After::Blocked.factor(),
                        why: After::Blocked,
                    },
                    now,
                );
                a.vel *= 0.2;
                let d = &mut self.fighters[di];
                if broken {
                    d.stamina = 0.0;
                    d.exerted = now;
                    d.set(Action::Stagger { dur: 0.9 }, now);
                    d.shield_locked_until = now + 1.3;
                    d.vel = push * 1.8;
                    self.events.push(Event::GuardBroken { by: side });
                    self.flash(contact.at, Spark::Wood, 1.0);
                    self.callout("Guard broken!", contact.at, tone_for(true), true);
                    self.note(if mine {
                        "You broke his guard.".to_owned()
                    } else {
                        "Your guard broke. You are open.".to_owned()
                    });
                    self.heavy(0.1, 0.12);
                } else {
                    d.spend(cost, now);
                    d.vel += push * (0.9 + cost * 0.04);
                    self.events.push(Event::Blocked {
                        by: side.other(),
                        dir,
                    });
                    self.flash(contact.at, Spark::Wood, 0.6);
                    let text = if raised {
                        "Blocked"
                    } else {
                        "Caught on the shield"
                    };
                    self.callout(text, contact.at, tone_for(false), false);
                    if dir == Dir::Overhead || weapon.on_shield > 1.2 {
                        self.heavy(0.05, 0.04);
                    }
                }
            }
            Struck::Body(zone) => {
                self.wound(side, dir, zone, contact.at, progress, riposte);
            }
        }
    }

    /// A blow that reached a body zone: the armour save, the damage, and
    /// what the wound does.
    fn wound(&mut self, side: Side, dir: Dir, zone: Zone, at: Vec3, progress: f32, riposte: bool) {
        let now = self.now;
        let (ai, di) = (side.index(), side.other().index());
        let weapon = self.fighters[ai].kit.weapon();
        let base = timing(dir, weapon);
        let blow = dir.blow();
        let kit = self.fighters[di].kit;
        let armour = armour(kit, zone);
        // The blow's force: the body's speed into it, and a tired arm.
        let closing = self.fighters[ai]
            .vel
            .dot((self.fighters[di].pos - self.fighters[ai].pos).normalize_or_zero())
            .max(0.0);
        let momentum = 1.0 + 0.12 * closing.min(3.0);
        let mut quality = weapon.edge + if riposte { RIPOSTE_EDGE } else { 0 };
        if self.fighters[ai].tired() {
            quality -= 2;
        }
        // Behind the scenes: the armour save. A natural 1 means the blow
        // found a gap; a total of 20 or more turns it.
        let save = self.dice.save(
            di as u64 + 1,
            "Armour",
            armour.save(blow) - quality,
            ARMOUR_DC,
        );
        let gap = save.roll == 1;
        let turned = !gap && save.success;
        let (count, sides, bonus) = match blow {
            Blow::Cut => weapon.cut,
            Blow::Thrust => weapon.thrust,
        };
        let rolled = (self.dice.sum(count, sides) as i32 + bonus).max(1) as f32;
        let arm = 1.0 - 0.12 * f32::from(self.fighters[ai].wounds_arm.min(2));
        let mut force = rolled * momentum * arm * if riposte { RIPOSTE_DAMAGE } else { 1.0 };
        if self.fighters[ai].tired() {
            force *= 0.85;
        }
        let damage = if turned {
            // The armour held: what reaches the body is the blow's weight.
            let blunt = match (zone, blow) {
                (Zone::Head, Blow::Cut) => 3.0 + force * 0.15,
                (_, Blow::Cut) => 1.0 + force * 0.1,
                _ => 0.5 + force * 0.05,
            };
            blunt.round()
        } else {
            let through = if gap { 1.0 } else { armour.through(blow) };
            (force * through * zone.weight()).round().max(1.0)
        };
        // The attacker's blow ends here and they recover.
        let a = &mut self.fighters[ai];
        a.set(
            Action::Recover {
                dir,
                from: progress,
                dur: base.recover * After::Landed.factor(),
                why: After::Landed,
            },
            now,
        );
        a.vel *= 0.3;
        a.dealt += damage;
        let d = &mut self.fighters[di];
        d.hp -= damage;
        d.hit_at = now;
        let push = (d.pos - self.fighters[ai].pos)
            .with_y(0.0)
            .normalize_or_zero();
        let d = &mut self.fighters[di];
        let mut stagger = 0.0;
        if turned {
            if zone == Zone::Head && blow == Blow::Cut {
                // A ringing blow to the helm.
                stagger = 0.35;
            }
        } else {
            stagger = zone.stagger();
            let (rate, time) = bleed(zone, blow);
            let deep = if gap { 1.5 } else { 1.0 };
            d.bleeds.push(Bleed {
                rate: rate * deep,
                until: now + time,
            });
            match zone {
                Zone::RightArm => d.wounds_arm = d.wounds_arm.saturating_add(1),
                Zone::LeftLeg | Zone::RightLeg => d.wounds_leg = d.wounds_leg.saturating_add(1),
                _ => {}
            }
        }
        if stagger > 0.0 && d.alive() && !matches!(d.action, Action::Stagger { .. } | Action::Down)
        {
            d.set(Action::Stagger { dur: stagger }, now);
        }
        d.vel += push * if turned { 0.6 } else { 1.2 };
        self.events.push(Event::Hit {
            on: side.other(),
            zone,
            blow,
            turned,
            damage,
        });
        let text = callout_text(kit, zone, blow, turned, gap, armour);
        let mine = side == Side::Player;
        let good = mine;
        self.flash(
            at,
            if turned { Spark::Steel } else { Spark::Flesh },
            if turned { 0.7 } else { 1.0 },
        );
        self.callout(
            format!("{text}  -{damage:.0}"),
            at,
            if good { Tone::Good } else { Tone::Bad },
            !turned,
        );
        let who = if mine { "Your" } else { "His" };
        self.note(format!(
            "{who} {} — {text} ({damage:.0})",
            dir.name().to_lowercase()
        ));
        if !turned || damage >= 10.0 || zone == Zone::Head {
            self.heavy(0.09, if turned { 0.07 } else { 0.13 });
        }
        if self.fighters[di].hp <= 0.0 {
            self.fall(side.other());
        }
    }

    /// The shield bash in its landing moment.
    fn bash(&mut self, side: Side) {
        let now = self.now;
        let (ai, di) = (side.index(), side.other().index());
        let a = &self.fighters[ai];
        let Action::Bash { landed: false } = a.action else {
            return;
        };
        let t = a.elapsed(now);
        if !(BASH_LAND.0..=BASH_LAND.1).contains(&t) {
            return;
        }
        let Some(at) = hits::bash(&self.rigs[ai], &self.rigs[di], self.fighters[di].kit) else {
            return;
        };
        let push = (self.fighters[di].pos - self.fighters[ai].pos)
            .with_y(0.0)
            .normalize_or_zero();
        self.fighters[ai].action = Action::Bash { landed: true };
        let d = &mut self.fighters[di];
        let guarded = d.guarding() && !matches!(d.action, Action::Parry);
        if guarded {
            d.spend(14.0, now);
            d.vel += push * 2.0;
            if d.stamina <= 0.0 {
                d.set(Action::Stagger { dur: 0.7 }, now);
                d.shield_locked_until = now + 1.0;
            }
        } else {
            d.set(Action::Stagger { dur: 0.65 }, now);
            d.vel = push * 2.4;
            d.spend(6.0, now);
        }
        self.events.push(Event::Bashed {
            on: side.other(),
            guarded,
        });
        self.flash(at, Spark::Wood, 0.8);
        let mine = side == Side::Player;
        self.callout(
            if guarded {
                "Shield on shield"
            } else {
                "Shield bash!"
            },
            at,
            if mine { Tone::Good } else { Tone::Bad },
            !guarded,
        );
        if !guarded {
            self.note(if mine {
                "Your bash knocked him back."
            } else {
                "His bash knocked you back."
            });
        }
        self.heavy(0.06, 0.08);
    }

    fn bleed_out(&mut self, side: Side, h: f32) {
        let now = self.now;
        let f = &mut self.fighters[side.index()];
        if !f.alive() {
            return;
        }
        let rate = f.bleeding(now);
        f.bleeds.retain(|b| b.until > now);
        if rate > 0.0 {
            f.hp -= rate * h;
            if f.hp <= 0.0 {
                self.fall(side);
            }
        }
    }

    fn fall(&mut self, side: Side) {
        let now = self.now;
        let f = &mut self.fighters[side.index()];
        if !f.alive() {
            return;
        }
        f.hp = f.hp.min(0.0);
        f.set(Action::Down, now);
        f.raise = 0.0;
        self.events.push(Event::Fell { side });
        if self.outcome == Outcome::Fighting {
            self.outcome = match side {
                Side::Enemy => Outcome::Won,
                Side::Player => Outcome::Lost,
            };
            self.note(match side {
                Side::Enemy => "Skurdo falls. The field is yours.",
                Side::Player => "You fall. Skurdo stands over you.",
            });
            self.heavy(0.12, 0.1);
        }
    }
}

/// The furthest the raised shield reaches across to the weapon side, and
/// out to its own side, radians.
pub const COVER_MIN: f32 = -0.8;
pub const COVER_MAX: f32 = 1.3;

/// What a callout over a hit says.
#[must_use]
pub fn callout_text(
    kit: Kit,
    zone: Zone,
    blow: Blow,
    turned: bool,
    gap: bool,
    armour: Armour,
) -> String {
    let kind = match blow {
        Blow::Cut => "cut",
        Blow::Thrust => "thrust",
    };
    let what = if turned {
        format!("{} turned the {kind}", capital(armour.name))
    } else if gap {
        match (kit, zone) {
            (Kit::Knight, Zone::Head) => "through the eye slit".to_owned(),
            (_, Zone::Head) => "under the helmet's rim".to_owned(),
            _ => format!("found a gap in the {}", armour.name),
        }
    } else {
        match (armour.name, blow) {
            ("mail", Blow::Thrust) => "the point burst the rings".to_owned(),
            ("mail", Blow::Cut) => "the cut bit through the rings".to_owned(),
            ("the helm" | "the helmet", _) => format!("a hard {kind} through {}", armour.name),
            (name, _) => format!("the {kind} got through the {name}"),
        }
    };
    format!("{} — {what}", zone.name())
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(c).collect::<String>()
    })
}
