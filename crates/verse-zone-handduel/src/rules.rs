//! The fight: the player against the Barrow Warden, a tall revenant with a
//! hooked glaive. Real time, with the dice rolled behind the scenes from a
//! seed (`docs/verse/combat-model.md`).
//!
//! The player stands at the origin facing `+z`, with `+x` to the left;
//! body space and the world share axes, so a hand's body position is where it is drawn. The Warden
//! walks in to his reach, waits, winds up a blow from one of four
//! directions, and strikes. The windup is the tell: a raised shield that
//! covers the blow's line within [`BLOCK_CONE`] stops it. The player's
//! swings land when the blade's path crosses the Warden's body in reach.

use glam::Vec3;
use verse_world::spells::dice::Dice;

use crate::gesture::{CHEST, Swing, shield_blocks};

/// The player's health and armour class.
pub const PLAYER_HP: f32 = 50.0;
pub const PLAYER_AC: i32 = 13;
/// The Warden's health and armour class.
pub const WARDEN_HP: f32 = 70.0;
pub const WARDEN_AC: i32 = 13;
/// The Warden's guard while he waits adds this to his armour class.
pub const GUARD_AC: i32 = 2;
/// The Warden's attack bonus.
pub const WARDEN_ATTACK: i32 = 6;
/// The player's attack bonus before swing speed.
pub const PLAYER_ATTACK: i32 = 4;

/// Where the Warden starts, m in front of the player.
pub const WARDEN_START: f32 = 9.0;
/// He stops this far away to fight, m.
pub const WARDEN_REACH: f32 = 1.9;
/// The player's sword reaches this far, m.
pub const PLAYER_REACH: f32 = 2.4;
/// His walking speed, m/s.
pub const WALK: f32 = 1.4;

/// How long the Warden's windup shows before the blow lands, s.
pub const WINDUP: f32 = 1.1;
/// His recovery after a blow, and after a blocked one, s.
pub const RECOVER: f32 = 0.9;
pub const RECOVER_BLOCKED: f32 = 1.4;
/// A stagger from a heavy hit or a bash, s.
pub const STAGGER: f32 = 0.8;
pub const STAGGER_BASH: f32 = 1.3;
/// His wait between blows, s.
pub const READY_MIN: f32 = 0.6;
pub const READY_MAX: f32 = 1.5;
/// A hit this heavy during his windup interrupts it.
pub const INTERRUPT_DAMAGE: f32 = 7.0;
/// The widest angle between the shield's cover and a blow's line that
/// still blocks, radians (40 degrees).
pub const BLOCK_CONE: f32 = 0.698;

/// The Warden's body in body space, as the blade sees it from the front:
/// `x` from `-BODY_HALF` to `BODY_HALF`, `y` from `BODY_LOW` to `BODY_HIGH`.
pub const BODY_HALF: f32 = 0.55;
pub const BODY_LOW: f32 = 0.6;
pub const BODY_HIGH: f32 = 2.4;
/// The least path inside the body that is a contact, m.
pub const MIN_CONTACT: f32 = 0.12;

/// How long a floating number lives, s.
pub const FLOATER_LIFE: f32 = 1.3;
/// How long a spark lives, s.
pub const SPARK_LIFE: f32 = 0.35;

/// Where a blow comes from, as the player sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    /// Down onto the head.
    Overhead,
    /// From the player's left.
    Left,
    /// From the player's right.
    Right,
    /// Straight at the chest.
    Thrust,
}

impl Blow {
    pub const ALL: [Blow; 4] = [Blow::Overhead, Blow::Left, Blow::Right, Blow::Thrust];

    /// Where the blow lands on the player, in body space.
    #[must_use]
    pub fn impact(self) -> Vec3 {
        match self {
            Blow::Overhead => Vec3::new(0.0, 1.95, 0.45),
            Blow::Left => Vec3::new(0.55, 1.4, 0.4),
            Blow::Right => Vec3::new(-0.55, 1.4, 0.4),
            Blow::Thrust => Vec3::new(0.0, 1.25, 0.6),
        }
    }

    /// The line the shield has to cover: from the chest to the impact.
    #[must_use]
    pub fn line(self) -> Vec3 {
        self.impact() - CHEST
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Blow::Overhead => "Overhead",
            Blow::Left => "From the left",
            Blow::Right => "From the right",
            Blow::Thrust => "Thrust",
        }
    }
}

/// What the Warden is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Approach,
    /// On guard until this time.
    Ready {
        until: f32,
    },
    /// Winding up `blow` since `start`.
    Windup {
        blow: Blow,
        start: f32,
    },
    Recover {
        until: f32,
    },
    Stagger {
        until: f32,
    },
    Dead {
        at: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Warden {
    pub hp: f32,
    pub max_hp: f32,
    /// How far in front of the player he stands, m.
    pub dist: f32,
    pub phase: Phase,
    pub hit_at: f32,
    /// His last blow and when it landed, for the drawing.
    pub struck: Option<(Blow, f32)>,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub hp: f32,
    pub max_hp: f32,
    pub hit_at: f32,
    pub healed_at: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Fighting,
    Won,
    Lost,
}

/// A floating word or number's kind, which sets its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Float {
    /// Damage the player dealt.
    Dealt,
    /// Damage the player took.
    Taken,
    Heal,
    /// A word: "Blocked", "Miss", "Out of reach".
    Word,
}

#[derive(Clone, Debug)]
pub struct Floater {
    pub text: String,
    pub at: Vec3,
    pub born: f32,
    pub kind: Float,
    pub crit: bool,
}

/// A burst at a contact point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    pub at: Vec3,
    pub born: f32,
    /// A shield stop rather than a cut.
    pub blocked: bool,
}

/// What a swing did, for the tests and the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwingResult {
    OutOfReach,
    /// The blade didn't cross him.
    NoContact,
    Miss,
    Hit {
        damage: f32,
        crit: bool,
    },
}

/// What one of the Warden's blows did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlowResult {
    Blocked,
    Miss,
    Hit { damage: f32, crit: bool },
}

/// The fight's whole state.
pub struct Fight {
    pub now: f32,
    pub dice: Dice,
    pub player: Player,
    pub warden: Warden,
    pub outcome: Outcome,
    pub floaters: Vec<Floater>,
    pub sparks: Vec<Spark>,
    /// The shield's cover direction while it is up.
    pub guard: Option<Vec3>,
    /// The shield's center while it is up, for the sparks.
    pub guard_at: Option<Vec3>,
    /// The last blow's result, for the tests.
    pub last_blow: Option<BlowResult>,
    last_dir: Option<Blow>,
    /// The camera jolts until this time.
    pub jolt_until: f32,
}

impl Fight {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            now: 0.0,
            dice: Dice::new(seed),
            player: Player {
                hp: PLAYER_HP,
                max_hp: PLAYER_HP,
                hit_at: f32::NEG_INFINITY,
                healed_at: f32::NEG_INFINITY,
            },
            warden: Warden {
                hp: WARDEN_HP,
                max_hp: WARDEN_HP,
                dist: WARDEN_START,
                phase: Phase::Approach,
                hit_at: f32::NEG_INFINITY,
                struck: None,
            },
            outcome: Outcome::Fighting,
            floaters: Vec::new(),
            sparks: Vec::new(),
            guard: None,
            guard_at: None,
            last_blow: None,
            last_dir: None,
            jolt_until: f32::NEG_INFINITY,
        }
    }

    /// The Warden's position in the world.
    #[must_use]
    pub fn warden_pos(&self) -> Vec3 {
        Vec3::new(0.0, 0.0, self.warden.dist)
    }

    /// The shield, as the hands hold it now: its cover direction and center
    /// while it is up.
    pub fn set_guard(&mut self, guard: Option<(Vec3, Vec3)>) {
        self.guard = guard.map(|(cover, _)| cover);
        self.guard_at = guard.map(|(_, at)| at);
    }

    /// How far through his windup the Warden is, with the blow.
    #[must_use]
    pub fn telegraph(&self) -> Option<(Blow, f32)> {
        match self.warden.phase {
            Phase::Windup { blow, start } => Some((blow, ((self.now - start) / WINDUP).min(1.0))),
            _ => None,
        }
    }

    fn float(&mut self, text: impl Into<String>, at: Vec3, kind: Float, crit: bool) {
        self.floaters.push(Floater {
            text: text.into(),
            at,
            born: self.now,
            kind,
            crit,
        });
    }

    fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        let r = self.dice.roll(1000) as f32 / 1000.0;
        lo + (hi - lo) * r
    }

    /// Advances the fight by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.now += dt;
        let now = self.now;
        self.floaters.retain(|f| now - f.born < FLOATER_LIFE);
        self.sparks.retain(|s| now - s.born < SPARK_LIFE);
        if self.outcome != Outcome::Fighting {
            return;
        }
        match self.warden.phase {
            Phase::Approach => {
                self.warden.dist = (self.warden.dist - WALK * dt).max(WARDEN_REACH);
                if self.warden.dist <= WARDEN_REACH + 1e-3 {
                    let wait = self.uniform(READY_MIN, READY_MAX);
                    self.warden.phase = Phase::Ready { until: now + wait };
                }
            }
            Phase::Ready { until } => {
                if now >= until {
                    let blow = self.pick_blow();
                    self.warden.phase = Phase::Windup { blow, start: now };
                }
            }
            Phase::Windup { blow, start } => {
                if now - start >= WINDUP {
                    self.strike(blow);
                }
            }
            Phase::Recover { until } | Phase::Stagger { until } => {
                if now >= until {
                    self.warden.phase = if self.warden.dist > WARDEN_REACH + 0.05 {
                        Phase::Approach
                    } else {
                        let wait = self.uniform(READY_MIN, READY_MAX);
                        Phase::Ready { until: now + wait }
                    };
                }
            }
            Phase::Dead { .. } => {}
        }
    }

    /// A blow, never the same direction three times running.
    fn pick_blow(&mut self) -> Blow {
        let mut blow = Blow::ALL[(self.dice.roll(4) - 1) as usize];
        if Some(blow) == self.last_dir && self.dice.roll(2) == 1 {
            blow = Blow::ALL[(self.dice.roll(4) - 1) as usize];
        }
        self.last_dir = Some(blow);
        blow
    }

    /// The Warden's blow lands: the shield stops it, or the dice decide.
    pub fn strike(&mut self, blow: Blow) -> BlowResult {
        let now = self.now;
        self.warden.struck = Some((blow, now));
        let at = blow.impact();
        let result = if self
            .guard
            .is_some_and(|cover| shield_blocks(cover, blow.line(), BLOCK_CONE))
        {
            let spark_at = self.guard_at.unwrap_or(at);
            self.sparks.push(Spark {
                at: spark_at,
                born: now,
                blocked: true,
            });
            self.float("Blocked", spark_at + Vec3::Y * 0.2, Float::Word, false);
            self.warden.phase = Phase::Recover {
                until: now + RECOVER_BLOCKED,
            };
            BlowResult::Blocked
        } else {
            let roll = self.dice.roll(20) as i32;
            self.warden.phase = Phase::Recover {
                until: now + RECOVER,
            };
            if roll == 1 || (roll != 20 && roll + WARDEN_ATTACK < PLAYER_AC) {
                self.float("Miss", at, Float::Word, false);
                BlowResult::Miss
            } else {
                let crit = roll == 20;
                let dice = if crit { 2 } else { 1 };
                let damage = (self.dice.sum(dice, 10) + 3) as f32;
                self.player.hp = (self.player.hp - damage).max(0.0);
                self.player.hit_at = now;
                self.jolt_until = now + 0.25;
                self.float(format!("-{damage:.0}"), at, Float::Taken, crit);
                if self.player.hp <= 0.0 {
                    self.outcome = Outcome::Lost;
                }
                BlowResult::Hit { damage, crit }
            }
        };
        self.last_blow = Some(result);
        result
    }

    /// How far the blade's path runs inside the Warden's body, m, and where
    /// it first enters.
    #[must_use]
    pub fn contact(path: &[Vec3]) -> (f32, Option<Vec3>) {
        let inside = |p: Vec3| {
            (-BODY_HALF..=BODY_HALF).contains(&p.x) && (BODY_LOW..=BODY_HIGH).contains(&p.y)
        };
        let mut length = 0.0;
        let mut first = None;
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            // Sample the segment so a fast cut across the body counts.
            const STEPS: usize = 8;
            let seg = a.distance(b) / STEPS as f32;
            for i in 0..STEPS {
                let p = a.lerp(b, (i as f32 + 0.5) / STEPS as f32);
                if inside(p) {
                    length += seg;
                    first.get_or_insert(p);
                }
            }
        }
        (length, first)
    }

    /// The player's swing lands, or doesn't.
    pub fn swing(&mut self, swing: &Swing) -> SwingResult {
        if self.outcome != Outcome::Fighting || matches!(self.warden.phase, Phase::Dead { .. }) {
            return SwingResult::NoContact;
        }
        let now = self.now;
        let face = self.warden_pos() + Vec3::Y * 1.5 - Vec3::Z * 0.3;
        if self.warden.dist > PLAYER_REACH {
            if self.warden.dist < PLAYER_REACH + 3.0 {
                self.float("Out of reach", face, Float::Word, false);
            }
            return SwingResult::OutOfReach;
        }
        let (inside, entry) = Self::contact(&swing.path);
        if inside < MIN_CONTACT {
            return SwingResult::NoContact;
        }
        // Where the blade meets him, on his front.
        let entry = entry.unwrap_or(Vec3::Y * 1.4);
        let at = Vec3::new(entry.x, entry.y.min(2.2), self.warden.dist - 0.25);
        // Faster swings land more often and harder.
        let speed_bonus = ((swing.peak - crate::gesture::SWING_START) / 1.0)
            .floor()
            .clamp(0.0, 4.0) as i32;
        let guard = if matches!(self.warden.phase, Phase::Ready { .. }) {
            GUARD_AC
        } else {
            0
        };
        let roll = self.dice.roll(20) as i32;
        if roll == 1 || (roll != 20 && roll + PLAYER_ATTACK + speed_bonus < WARDEN_AC + guard) {
            self.sparks.push(Spark {
                at,
                born: now,
                blocked: true,
            });
            self.float("Miss", at, Float::Word, false);
            return SwingResult::Miss;
        }
        let crit = roll == 20;
        let dice = if crit { 2 } else { 1 };
        let base = (self.dice.sum(dice, 8) + 2) as f32;
        let speed = (0.7 + 0.25 * (swing.peak - crate::gesture::SWING_START)).clamp(0.7, 1.6);
        let quality = (0.8 + 0.4 * (inside / swing.arc.max(0.1))).clamp(0.8, 1.2);
        let damage = (base * speed * quality).round().max(1.0);
        self.warden.hp = (self.warden.hp - damage).max(0.0);
        self.warden.hit_at = now;
        self.sparks.push(Spark {
            at,
            born: now,
            blocked: false,
        });
        self.float(format!("{damage:.0}"), at, Float::Dealt, crit);
        if self.warden.hp <= 0.0 {
            self.warden.phase = Phase::Dead { at: now };
            self.outcome = Outcome::Won;
        } else if matches!(self.warden.phase, Phase::Windup { .. }) && damage >= INTERRUPT_DAMAGE {
            self.warden.phase = Phase::Stagger {
                until: now + STAGGER,
            };
            self.float("Interrupted", face, Float::Word, false);
        }
        SwingResult::Hit { damage, crit }
    }

    /// A shield bash: interrupts a windup in reach, else shoves him back.
    pub fn bash(&mut self) -> bool {
        if self.outcome != Outcome::Fighting || self.warden.dist > PLAYER_REACH {
            return false;
        }
        let now = self.now;
        let face = self.warden_pos() + Vec3::Y * 1.5 - Vec3::Z * 0.3;
        if matches!(self.warden.phase, Phase::Dead { .. }) {
            return false;
        }
        let windup = matches!(self.warden.phase, Phase::Windup { .. });
        let damage = self.dice.roll(4) as f32;
        self.warden.hp = (self.warden.hp - damage).max(0.0);
        self.warden.hit_at = now;
        self.warden.dist = (self.warden.dist + 0.7).min(WARDEN_START);
        self.warden.phase = Phase::Stagger {
            until: now + if windup { STAGGER_BASH } else { STAGGER * 0.6 },
        };
        self.float(
            if windup { "Bashed" } else { "Shoved" },
            face,
            Float::Word,
            false,
        );
        if self.warden.hp <= 0.0 {
            self.warden.phase = Phase::Dead { at: now };
            self.outcome = Outcome::Won;
        }
        windup
    }

    /// A potion drunk: 2d4 + 4 back.
    pub fn drink(&mut self) -> f32 {
        if self.outcome == Outcome::Lost {
            return 0.0;
        }
        let heal = (self.dice.sum(2, 4) + 4) as f32;
        let before = self.player.hp;
        self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
        self.player.healed_at = self.now;
        let gained = self.player.hp - before;
        self.float(
            format!("+{gained:.0}"),
            crate::gesture::MOUTH + Vec3::Z * 0.4,
            Float::Heal,
            false,
        );
        gained
    }
}
