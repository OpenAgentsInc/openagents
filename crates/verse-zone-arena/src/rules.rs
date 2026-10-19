//! The duel's rules under Verse's combat model (`docs/verse/combat-model.md`):
//! real time, a 1 s global cooldown, per-ability cooldowns, shields, and
//! stuns with diminishing returns, with SRD-style dice rolled behind the
//! scenes from a seed so a fight replays exactly.
//!
//! Nothing here draws or reads input. [`Fight`] owns both robots, the
//! projectiles in flight, the floating numbers, and the short log; the zone
//! adapter moves the player's robot from the shared controller and hands
//! its pose in with [`Fight::sync_player`].

use std::collections::VecDeque;

use glam::Vec3;
use serde::Serialize;
use verse_world::spells::dice::Dice;

/// The global cooldown every on-GCD ability starts, s.
pub const GCD: f32 = 1.0;
/// Radius of the fighting floor, m. Neither robot leaves it.
pub const ARENA_RADIUS: f32 = 20.0;
/// The player's robot's collision radius, m.
pub const PLAYER_RADIUS: f32 = 0.6;
/// The enemy robot's collision radius, m.
pub const ENEMY_RADIUS: f32 = 1.0;
/// Height of a robot's center of mass above its feet, where shots aim, m.
pub const CENTER: f32 = 1.4;
/// How long a floating number rises before it fades out, s.
pub const FLOATER_LIFE: f32 = 1.3;
/// A repeated stun within this many seconds is shortened (combat model).
pub const DIMINISH_WINDOW: f32 = 15.0;
/// Log lines the fight keeps.
pub const LOG_LINES: usize = 5;
/// The enemy holds its fire this long after a fight starts, s.
pub const OPENING_GRACE: f32 = 1.5;

/// Where each robot starts: the player at the south end facing north, the
/// enemy at the north end facing back.
pub const PLAYER_START: Vec3 = Vec3::new(0.0, 0.0, -10.0);
pub const ENEMY_START: Vec3 = Vec3::new(0.0, 0.0, 10.0);

/// Which robot something belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
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
}

/// The player's four abilities, on keys 1 to 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Ability {
    /// An instant beam: a ranged attack roll, 2d10 radiant on a hit.
    Laser,
    /// Four homing missiles; each bursts for 2d6 force in 2.5 m, Dex save
    /// for half.
    Missiles,
    /// Absorbs the next 35 damage for 6 s. Off the global cooldown.
    Shield,
    /// A 6 m burst around the robot: 3d6 lightning, a Con save, and on a
    /// failure a 2 s stun and a 7 m knockback.
    Emp,
}

impl Ability {
    pub const ALL: [Self; 4] = [Self::Laser, Self::Missiles, Self::Shield, Self::Emp];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Laser => "Laser",
            Self::Missiles => "Missile Salvo",
            Self::Shield => "Shield",
            Self::Emp => "EMP Burst",
        }
    }

    /// The short label the action bar fits under the slot's key.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Laser => "LASER",
            Self::Missiles => "MISSILE",
            Self::Shield => "SHIELD",
            Self::Emp => "EMP",
        }
    }

    /// The slot's key.
    #[must_use]
    pub const fn key(self) -> char {
        match self {
            Self::Laser => '1',
            Self::Missiles => '2',
            Self::Shield => '3',
            Self::Emp => '4',
        }
    }

    /// The ability on slot `index`, 0-based.
    #[must_use]
    pub fn slot(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    /// The ability's own cooldown, s. The Laser has none beyond the global
    /// cooldown, as a cantrip.
    #[must_use]
    pub const fn cooldown(self) -> f32 {
        match self {
            Self::Laser => 0.0,
            Self::Missiles => 8.0,
            Self::Shield => 15.0,
            Self::Emp => 12.0,
        }
    }

    #[must_use]
    pub const fn on_gcd(self) -> bool {
        !matches!(self, Self::Shield)
    }

    /// How close the enemy must be, m; zero for an ability with no target.
    #[must_use]
    pub const fn range(self) -> f32 {
        match self {
            Self::Laser => 30.0,
            Self::Missiles => 36.0,
            Self::Shield => 0.0,
            Self::Emp => 6.0,
        }
    }

    /// The tooltip line: dice, range, and cooldown.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Laser => "2d10 radiant (2 to 20), 30 m, instant",
            Self::Missiles => "4 missiles, 2d6 force each in 2.5 m, 36 m, 8 s",
            Self::Shield => "Absorbs 35 damage for 6 s, 15 s",
            Self::Emp => "3d6 lightning in 6 m, stun 2 s and knockback, 12 s",
        }
    }
}

/// The player's attack bonus, the save DC of its abilities, and its armor.
pub const PLAYER_ATTACK: i32 = 7;
pub const PLAYER_DC: i32 = 15;
pub const PLAYER_AC: i32 = 15;
pub const PLAYER_HP: f32 = 120.0;
pub const PLAYER_DEX_SAVE: i32 = 3;
/// The enemy's numbers.
pub const ENEMY_ATTACK: i32 = 5;
pub const ENEMY_DC: i32 = 13;
pub const ENEMY_AC: i32 = 14;
pub const ENEMY_HP: f32 = 160.0;
pub const ENEMY_DEX_SAVE: i32 = 1;
pub const ENEMY_CON_SAVE: i32 = 3;

/// The shield's strength and length.
pub const SHIELD_ABSORB: f32 = 35.0;
pub const SHIELD_SECONDS: f32 = 6.0;
/// The EMP's reach, stun, and knockbacks on a failed and a passed save.
pub const EMP_RADIUS: f32 = 6.0;
pub const EMP_STUN: f32 = 2.0;
pub const EMP_PUSH: f32 = 7.0;
pub const EMP_PUSH_SAVED: f32 = 3.0;
/// Missiles in a salvo, their blast radius, and their speeds.
pub const SALVO: usize = 4;
pub const MISSILE_BLAST: f32 = 2.5;
pub const MISSILE_SPEED: f32 = 18.0;
pub const MISSILE_TURN: f32 = 40.0;
/// The enemy's plasma bolt: speed, how near it must pass, and its range.
pub const BOLT_SPEED: f32 = 16.0;
pub const BOLT_REACH: f32 = 1.0;
pub const BOLT_RANGE: f32 = 28.0;
pub const BOLT_COOLDOWN: f32 = 1.6;
/// The enemy's ground slam: reach, wind-up, cooldown, knockdown.
pub const SLAM_REACH: f32 = 5.0;
pub const SLAM_WINDUP: f32 = 0.8;
pub const SLAM_COOLDOWN: f32 = 7.0;
pub const SLAM_PUSH: f32 = 6.0;
pub const SLAM_KNOCKDOWN: f32 = 1.0;
/// The enemy's one emergency shield below half health.
pub const OVERDRIVE_ABSORB: f32 = 25.0;
/// How fast a knockback's speed decays, 1/s. A push of `d` meters starts at
/// `d * PUSH_DECAY` m/s.
pub const PUSH_DECAY: f32 = 6.0;

/// One robot's combat state.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Robot {
    pub side: Side,
    /// Feet, m.
    pub pos: Vec3,
    /// Heading about +Y; zero faces +Z.
    pub yaw: f32,
    /// Ground speed this frame, m/s, for the walk.
    pub speed: f32,
    /// Distance walked, m, which phases the legs.
    pub stride: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub shield: f32,
    pub shield_until: f32,
    pub stunned_until: f32,
    /// A knockback in progress, m/s along the ground.
    pub push: Vec3,
    /// Stuns taken inside the diminishing window, and when the last began.
    pub stuns: u32,
    pub last_stun: f32,
    /// When the robot last took damage, for its hit flash.
    pub hit_at: f32,
}

impl Robot {
    #[must_use]
    pub fn new(side: Side) -> Self {
        let (pos, yaw, hp) = match side {
            Side::Player => (PLAYER_START, 0.0, PLAYER_HP),
            Side::Enemy => (ENEMY_START, std::f32::consts::PI, ENEMY_HP),
        };
        Self {
            side,
            pos,
            yaw,
            speed: 0.0,
            stride: 0.0,
            hp,
            max_hp: hp,
            shield: 0.0,
            shield_until: 0.0,
            stunned_until: 0.0,
            push: Vec3::ZERO,
            stuns: 0,
            last_stun: f32::NEG_INFINITY,
            hit_at: f32::NEG_INFINITY,
        }
    }

    #[must_use]
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    #[must_use]
    pub fn stunned(&self, now: f32) -> bool {
        now < self.stunned_until
    }

    /// The shield still holding at `now`.
    #[must_use]
    pub fn shield_at(&self, now: f32) -> f32 {
        if now < self.shield_until {
            self.shield
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::Y * CENTER
    }

    #[must_use]
    pub fn radius(&self) -> f32 {
        match self.side {
            Side::Player => PLAYER_RADIUS,
            Side::Enemy => ENEMY_RADIUS,
        }
    }

    #[must_use]
    pub fn armor(&self) -> i32 {
        match self.side {
            Side::Player => PLAYER_AC,
            Side::Enemy => ENEMY_AC,
        }
    }

    #[must_use]
    pub fn dex_save(&self) -> i32 {
        match self.side {
            Side::Player => PLAYER_DEX_SAVE,
            Side::Enemy => ENEMY_DEX_SAVE,
        }
    }

    #[must_use]
    pub fn con_save(&self) -> i32 {
        match self.side {
            Side::Player => 2,
            Side::Enemy => ENEMY_CON_SAVE,
        }
    }

    /// Raises a shield of `amount` for `seconds`, replacing a weaker one.
    pub fn raise_shield(&mut self, amount: f32, seconds: f32, now: f32) {
        self.shield = self.shield_at(now).max(amount);
        self.shield_until = now + seconds;
    }

    /// Applies `amount` damage, the shield first. Returns what reached
    /// health and what the shield absorbed.
    pub fn damage(&mut self, amount: f32, now: f32) -> (f32, f32) {
        let amount = amount.max(0.0);
        let shield = self.shield_at(now);
        let absorbed = shield.min(amount);
        self.shield = shield - absorbed;
        let taken = (amount - absorbed).min(self.hp.max(0.0));
        self.hp -= taken;
        if amount > 0.0 {
            self.hit_at = now;
        }
        (taken, absorbed)
    }

    /// Stuns for `seconds`, shortened by diminishing returns: each repeat
    /// within [`DIMINISH_WINDOW`] halves it, and the third is immune.
    /// Returns the seconds applied, zero when immune.
    pub fn stun(&mut self, seconds: f32, now: f32) -> f32 {
        if now - self.last_stun > DIMINISH_WINDOW {
            self.stuns = 0;
        }
        if self.stuns >= 2 {
            return 0.0;
        }
        let applied = seconds * 0.5_f32.powi(self.stuns as i32);
        self.stuns += 1;
        self.last_stun = now;
        self.stunned_until = self.stunned_until.max(now + applied);
        applied
    }

    /// Starts a knockback of `distance` meters along `direction`.
    pub fn knock(&mut self, direction: Vec3, distance: f32) {
        let flat = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
        self.push += flat * distance * PUSH_DECAY;
    }

    /// Advances the knockback by `dt` and keeps the robot on the floor.
    pub fn slide(&mut self, dt: f32) {
        self.pos += self.push * dt;
        self.push *= (-PUSH_DECAY * dt).exp();
        if self.push.length() < 0.05 {
            self.push = Vec3::ZERO;
        }
        self.keep_on_floor();
    }

    /// Clamps the robot inside the arena's ring.
    pub fn keep_on_floor(&mut self) {
        let flat = Vec3::new(self.pos.x, 0.0, self.pos.z);
        let limit = ARENA_RADIUS - self.radius();
        if flat.length() > limit {
            let kept = flat.normalize() * limit;
            self.pos.x = kept.x;
            self.pos.z = kept.z;
        }
    }
}

/// What a projectile is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ShotKind {
    /// The player's homing missile.
    Missile,
    /// The enemy's plasma bolt, flying straight.
    Bolt,
}

/// A projectile in flight.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shot {
    pub kind: ShotKind,
    pub from: Side,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    /// A missile waits this long before it homes, so a salvo fans out.
    pub delay: f32,
}

/// A brief effect the scene draws.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Effect {
    /// The laser's beam, from the gun to where it landed.
    Beam { from: Vec3, to: Vec3, hit: bool },
    /// A missile's or a bolt's burst.
    Blast { at: Vec3, radius: f32, side: Side },
    /// The EMP's ring around the player's robot.
    Pulse { at: Vec3, radius: f32 },
    /// The enemy's slam on the floor.
    Slam { at: Vec3, radius: f32 },
}

/// An effect and when it began.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Flash {
    pub effect: Effect,
    pub at: f32,
}

impl Flash {
    /// How long each effect lasts, s.
    #[must_use]
    pub const fn life(&self) -> f32 {
        match self.effect {
            Effect::Beam { .. } => 0.18,
            Effect::Blast { .. } => 0.45,
            Effect::Pulse { .. } => 0.5,
            Effect::Slam { .. } => 0.6,
        }
    }
}

/// A floating number or word over a robot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Floater {
    /// Who it is over: the robot that took the damage or the effect.
    pub over: Side,
    pub at: Vec3,
    pub text: String,
    pub crit: bool,
    /// Healing or shield, drawn apart from damage.
    pub good: bool,
    pub born: f32,
}

/// How the fight stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Outcome {
    Fighting,
    Won,
    Lost,
}

/// Why an ability did not fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Refusal {
    /// The fight is over; R starts another.
    Over,
    Stunned,
    /// The global cooldown is running.
    Busy,
    /// The ability's own cooldown is running.
    NotReady,
    OutOfRange,
}

impl Refusal {
    /// What the screen says, as an MMO's error line does.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Over => "The fight is over. Press R to fight again.",
            Self::Stunned => "You can't do that while stunned",
            Self::Busy => "Not ready yet",
            Self::NotReady => "That ability isn't ready yet",
            Self::OutOfRange => "Out of range",
        }
    }
}

/// The enemy's slam while it winds up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Windup {
    pub started: f32,
}

/// The enemy's own state beyond its robot: cooldowns, the slam it winds
/// up, which way it circles, and whether it spent its emergency shield.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Brain {
    pub bolt_ready: f32,
    pub slam_ready: f32,
    pub windup: Option<Windup>,
    /// +1 circles counterclockwise, -1 clockwise.
    pub circle: f32,
    pub circle_until: f32,
    pub overdrive: bool,
}

impl Brain {
    fn new() -> Self {
        Self {
            bolt_ready: OPENING_GRACE,
            slam_ready: OPENING_GRACE + 2.0,
            windup: None,
            circle: 1.0,
            circle_until: 3.0,
            overdrive: false,
        }
    }
}

/// The duel: two robots, their shots, and everything that rolls.
#[derive(Clone, Debug, Serialize)]
pub struct Fight {
    pub seed: u64,
    #[serde(skip)]
    dice: Dice,
    /// Seconds since the fight began.
    pub now: f32,
    pub player: Robot,
    pub enemy: Robot,
    pub brain: Brain,
    /// When the global cooldown ends.
    pub gcd_until: f32,
    /// When each of the player's abilities is ready again, by slot.
    pub ready_at: [f32; 4],
    pub shots: Vec<Shot>,
    pub flashes: Vec<Flash>,
    pub floaters: Vec<Floater>,
    pub log: VecDeque<String>,
    pub outcome: Outcome,
    /// Damage each side dealt, for the end screen.
    pub dealt: [f32; 2],
}

/// A ranged attack roll's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub roll: u32,
    pub hit: bool,
    pub crit: bool,
}

impl Fight {
    /// A fresh duel whose dice start from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            dice: Dice::new(seed),
            now: 0.0,
            player: Robot::new(Side::Player),
            enemy: Robot::new(Side::Enemy),
            brain: Brain::new(),
            gcd_until: 0.0,
            ready_at: [0.0; 4],
            shots: Vec::new(),
            flashes: Vec::new(),
            floaters: Vec::new(),
            log: VecDeque::new(),
            outcome: Outcome::Fighting,
            dealt: [0.0; 2],
        }
    }

    /// The dice, so a test can force a save.
    pub fn dice_mut(&mut self) -> &mut Dice {
        &mut self.dice
    }

    #[must_use]
    pub fn robot(&self, side: Side) -> &Robot {
        match side {
            Side::Player => &self.player,
            Side::Enemy => &self.enemy,
        }
    }

    fn robot_mut(&mut self, side: Side) -> &mut Robot {
        match side {
            Side::Player => &mut self.player,
            Side::Enemy => &mut self.enemy,
        }
    }

    /// The flat distance between the robots, m.
    #[must_use]
    pub fn distance(&self) -> f32 {
        let d = self.enemy.pos - self.player.pos;
        d.x.hypot(d.z)
    }

    /// Seconds before `ability` can fire, counting the global cooldown.
    #[must_use]
    pub fn remaining(&self, ability: Ability) -> f32 {
        let own = self.ready_at[ability as usize] - self.now;
        let gcd = if ability.on_gcd() {
            self.gcd_until - self.now
        } else {
            0.0
        };
        own.max(gcd).max(0.0)
    }

    /// Whether the slot's remaining time is the global cooldown alone.
    #[must_use]
    pub fn on_global(&self, ability: Ability) -> bool {
        ability.on_gcd()
            && self.gcd_until > self.now
            && self.ready_at[ability as usize] <= self.gcd_until
    }

    /// Whether `ability` would fire now, and why not.
    ///
    /// # Errors
    ///
    /// The reason the ability cannot fire.
    pub fn usable(&self, ability: Ability) -> Result<(), Refusal> {
        if self.outcome != Outcome::Fighting {
            return Err(Refusal::Over);
        }
        if self.player.stunned(self.now) {
            return Err(Refusal::Stunned);
        }
        if self.ready_at[ability as usize] > self.now {
            return Err(Refusal::NotReady);
        }
        if ability.on_gcd() && self.gcd_until > self.now {
            return Err(Refusal::Busy);
        }
        if ability.range() > 0.0 && self.distance() > ability.range() {
            return Err(Refusal::OutOfRange);
        }
        Ok(())
    }

    /// Hands in the player's robot's pose from the shared controller.
    pub fn sync_player(&mut self, pos: Vec3, yaw: f32, speed: f32) {
        let moved = Vec3::new(pos.x - self.player.pos.x, 0.0, pos.z - self.player.pos.z).length();
        self.player.stride += moved.min(2.0);
        self.player.pos = pos;
        self.player.yaw = yaw;
        self.player.speed = speed;
    }

    /// Fires `ability` from the player's robot at the enemy.
    ///
    /// # Errors
    ///
    /// The reason the ability cannot fire; nothing changes.
    pub fn press(&mut self, ability: Ability) -> Result<(), Refusal> {
        self.usable(ability)?;
        let now = self.now;
        if ability.on_gcd() {
            self.gcd_until = now + GCD;
        }
        self.ready_at[ability as usize] = now + ability.cooldown();
        match ability {
            Ability::Laser => self.laser(),
            Ability::Missiles => self.salvo(),
            Ability::Shield => {
                self.player.raise_shield(SHIELD_ABSORB, SHIELD_SECONDS, now);
                self.float(Side::Player, "Shield".into(), false, true);
                self.note("You raise your shield");
            }
            Ability::Emp => self.emp(),
        }
        Ok(())
    }

    /// A d20 plus `bonus` against `armor`: a natural 20 always hits and
    /// crits, a natural 1 always misses.
    pub fn attack_roll(&mut self, bonus: i32, armor: i32) -> Attack {
        let roll = self.dice.roll(20);
        let crit = roll == 20;
        let hit = crit || (roll != 1 && roll as i32 + bonus >= armor);
        Attack { roll, hit, crit }
    }

    /// `count` dice of `sides` faces, doubled on a critical hit.
    pub fn damage_roll(&mut self, count: u32, sides: u32, crit: bool) -> u32 {
        self.dice.sum(if crit { count * 2 } else { count }, sides)
    }

    fn gun(robot: &Robot) -> Vec3 {
        let forward = verse_world::social::controller::forward(robot.yaw);
        let right = forward.cross(Vec3::Y);
        robot.pos + Vec3::Y * 1.55 + forward * 0.7 + right * 0.55
    }

    fn laser(&mut self) {
        let from = Self::gun(&self.player);
        let attack = self.attack_roll(PLAYER_ATTACK, ENEMY_AC);
        let target = self.enemy.center();
        if attack.hit {
            let amount = self.damage_roll(2, 10, attack.crit) as f32;
            self.flash(Effect::Beam {
                from,
                to: target,
                hit: true,
            });
            self.hit(Side::Enemy, amount, attack.crit, "Laser");
        } else {
            // A miss scorches the floor beside the target.
            let forward = (target - from).normalize_or_zero();
            let beside = forward.cross(Vec3::Y).normalize_or_zero() * 1.6;
            self.flash(Effect::Beam {
                from,
                to: target + beside - Vec3::Y * 1.2,
                hit: false,
            });
            self.float(Side::Enemy, "Miss".into(), false, false);
            self.note("Your laser misses");
        }
    }

    fn salvo(&mut self) {
        let robot = &self.player;
        let forward = verse_world::social::controller::forward(robot.yaw);
        let right = forward.cross(Vec3::Y);
        let launch = robot.pos + Vec3::Y * 2.3;
        for i in 0..SALVO {
            let spread = i as f32 / (SALVO - 1) as f32 * 2.0 - 1.0;
            self.shots.push(Shot {
                kind: ShotKind::Missile,
                from: Side::Player,
                pos: launch + right * spread * 0.5,
                vel: forward * 5.0 + Vec3::Y * 7.0 + right * spread * 5.0,
                age: 0.0,
                delay: 0.25 + 0.06 * i as f32,
            });
        }
        self.note("You fire a missile salvo");
    }

    fn emp(&mut self) {
        let at = self.player.pos;
        self.flash(Effect::Pulse {
            at,
            radius: EMP_RADIUS,
        });
        if self.distance() > EMP_RADIUS {
            return;
        }
        let save = self.dice.save(2, "Constitution", ENEMY_CON_SAVE, PLAYER_DC);
        let rolled = self.damage_roll(3, 6, false) as f32;
        let away = self.enemy.pos - self.player.pos;
        if save.success {
            self.enemy.knock(away, EMP_PUSH_SAVED);
            self.hit(Side::Enemy, (rolled / 2.0).floor(), false, "EMP Burst");
            self.float(Side::Enemy, "Resisted".into(), false, false);
        } else {
            self.enemy.knock(away, EMP_PUSH);
            self.hit(Side::Enemy, rolled, false, "EMP Burst");
            let stunned = self.enemy.stun(EMP_STUN, self.now);
            if stunned > 0.0 {
                // A stun breaks the slam it winds up.
                if self.brain.windup.take().is_some() {
                    self.float(Side::Enemy, "Interrupted".into(), false, false);
                }
                self.float(Side::Enemy, "Stunned".into(), false, false);
                self.note(&format!("The enemy is stunned for {stunned:.1} s"));
            } else {
                self.float(Side::Enemy, "Immune".into(), false, false);
            }
        }
    }

    /// Deals `amount` to `side`, through its shield, with the floating
    /// number and the log line.
    pub fn hit(&mut self, side: Side, amount: f32, crit: bool, source: &str) {
        let now = self.now;
        let (taken, absorbed) = self.robot_mut(side).damage(amount, now);
        let dealer = side.other() as usize;
        self.dealt[dealer] += taken + absorbed;
        if taken > 0.0 || absorbed == 0.0 {
            let text = if crit {
                format!("{taken:.0}!")
            } else {
                format!("{taken:.0}")
            };
            self.float(side, text, crit, false);
        }
        if absorbed > 0.0 {
            self.float(side, format!("Absorbed {absorbed:.0}"), false, true);
        }
        let crit_word = if crit { " critical" } else { "" };
        let (whom, whose) = match side {
            Side::Enemy => ("the enemy", "the enemy's"),
            Side::Player => ("you", "your"),
        };
        let line = if taken == 0.0 && absorbed > 0.0 {
            format!("{source}{crit_word} hits {whose} shield for {absorbed:.0}")
        } else {
            format!("{source}{crit_word} hits {whom} for {taken:.0}")
        };
        self.note(&line);
        self.settle();
    }

    fn settle(&mut self) {
        if self.outcome != Outcome::Fighting {
            return;
        }
        if !self.enemy.alive() {
            self.outcome = Outcome::Won;
            self.shots.clear();
            self.brain.windup = None;
            self.note("The enemy robot is destroyed");
        } else if !self.player.alive() {
            self.outcome = Outcome::Lost;
            self.shots.clear();
            self.brain.windup = None;
            self.note("Your robot is destroyed");
        }
    }

    fn float(&mut self, over: Side, text: String, crit: bool, good: bool) {
        let robot = self.robot(over);
        // Numbers in one moment fan out a little, so they don't stack.
        let n = self
            .floaters
            .iter()
            .filter(|f| f.over == over && self.now - f.born < 0.3)
            .count();
        let at = robot.pos + Vec3::Y * (3.2 + 0.35 * n as f32);
        self.floaters.push(Floater {
            over,
            at,
            text,
            crit,
            good,
            born: self.now,
        });
    }

    fn flash(&mut self, effect: Effect) {
        self.flashes.push(Flash {
            effect,
            at: self.now,
        });
    }

    fn note(&mut self, line: &str) {
        self.log.push_back(line.to_owned());
        while self.log.len() > LOG_LINES {
            self.log.pop_front();
        }
    }

    /// Advances the fight by `dt` seconds: knockbacks, the enemy's moves and
    /// attacks, every projectile, and the effects' and numbers' lifetimes.
    pub fn tick(&mut self, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        if dt == 0.0 {
            return;
        }
        self.now += dt;
        self.player.slide(dt);
        self.enemy.slide(dt);
        if self.outcome == Outcome::Fighting {
            crate::ai::think(self, dt);
            self.fly(dt);
        }
        self.separate();
        let now = self.now;
        self.flashes.retain(|f| now - f.at < f.life());
        self.floaters.retain(|f| now - f.born < FLOATER_LIFE);
    }

    /// Keeps the robots from standing in each other: the heavier enemy
    /// holds its ground and the player's robot is pushed clear.
    fn separate(&mut self) {
        let reach = PLAYER_RADIUS + ENEMY_RADIUS;
        let d = Vec3::new(
            self.player.pos.x - self.enemy.pos.x,
            0.0,
            self.player.pos.z - self.enemy.pos.z,
        );
        let distance = d.length();
        if distance < reach {
            let away = if distance > 1e-4 {
                d / distance
            } else {
                -Vec3::Z
            };
            let y = self.player.pos.y;
            self.player.pos = self.enemy.pos.with_y(y) + away * reach;
            self.player.keep_on_floor();
            // Pinned against the ring, the enemy steps aside instead.
            let now = Vec3::new(
                self.player.pos.x - self.enemy.pos.x,
                0.0,
                self.player.pos.z - self.enemy.pos.z,
            );
            if now.length() < reach - 0.01 {
                self.enemy.pos = self.player.pos.with_y(0.0) - away * reach;
                self.enemy.keep_on_floor();
            }
        }
    }

    /// Fires the enemy's plasma bolt at where the player stands now.
    pub(crate) fn fire_bolt(&mut self) {
        let from = Self::gun(&self.enemy) + Vec3::Y * 0.2;
        let aim = (self.player.center() - from).normalize_or_zero();
        self.shots.push(Shot {
            kind: ShotKind::Bolt,
            from: Side::Enemy,
            pos: from,
            vel: aim * BOLT_SPEED,
            age: 0.0,
            delay: 0.0,
        });
    }

    /// The slam lands: a Dex save for half, and on a failure a knockdown
    /// and a knockback.
    pub(crate) fn land_slam(&mut self) {
        let at = self.enemy.pos;
        self.flash(Effect::Slam {
            at,
            radius: SLAM_REACH,
        });
        if self.distance() > SLAM_REACH + PLAYER_RADIUS {
            self.float(Side::Player, "Miss".into(), false, false);
            self.note("The enemy's slam misses");
            return;
        }
        let save = self.dice.save(1, "Dexterity", PLAYER_DEX_SAVE, ENEMY_DC);
        let rolled = self.damage_roll(4, 6, false) as f32;
        let away = self.player.pos - self.enemy.pos;
        if save.success {
            self.hit(Side::Player, (rolled / 2.0).floor(), false, "Ground Slam");
            self.float(Side::Player, "Resisted".into(), false, false);
        } else {
            self.player.knock(away, SLAM_PUSH);
            self.hit(Side::Player, rolled, false, "Ground Slam");
            if self.player.alive() && self.player.stun(SLAM_KNOCKDOWN, self.now) > 0.0 {
                self.float(Side::Player, "Knocked down".into(), false, false);
            }
        }
    }

    /// The enemy's one emergency shield.
    pub(crate) fn overdrive(&mut self) {
        self.brain.overdrive = true;
        self.enemy
            .raise_shield(OVERDRIVE_ABSORB, SHIELD_SECONDS, self.now);
        self.float(Side::Enemy, "Shield".into(), false, true);
        self.note("The enemy raises a shield");
    }

    fn fly(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        let mut landed = Vec::new();
        for shot in &mut shots {
            shot.age += dt;
            let target = self.robot(shot.from.other()).center();
            let before = shot.pos;
            match shot.kind {
                ShotKind::Missile => {
                    if shot.age >= shot.delay {
                        let want = (target - shot.pos).normalize_or_zero() * MISSILE_SPEED;
                        let change = want - shot.vel;
                        let most = MISSILE_TURN * dt;
                        shot.vel += if change.length() > most {
                            change.normalize() * most
                        } else {
                            change
                        };
                    } else {
                        // The launch climbs and slows before it turns.
                        shot.vel *= (-2.0 * dt).exp();
                    }
                    shot.pos += shot.vel * dt;
                    let near = closest(before, shot.pos, target).distance(target);
                    if near < 0.9 || shot.pos.y < 0.05 || shot.age > 5.0 {
                        let at = if near < 0.9 {
                            target
                        } else {
                            shot.pos.with_y(shot.pos.y.max(0.05))
                        };
                        landed.push((shot.kind, shot.from, at, true));
                        shot.age = f32::INFINITY;
                    }
                }
                ShotKind::Bolt => {
                    shot.pos += shot.vel * dt;
                    let near = closest(before, shot.pos, target).distance(target);
                    if near < BOLT_REACH {
                        landed.push((shot.kind, shot.from, target, true));
                        shot.age = f32::INFINITY;
                    } else if shot.age * BOLT_SPEED > BOLT_RANGE + 6.0 || shot.pos.y < 0.0 {
                        // It flew past: the player dodged it.
                        shot.age = f32::INFINITY;
                        landed.push((shot.kind, shot.from, shot.pos, false));
                    }
                }
            }
        }
        shots.retain(|s| s.age.is_finite());
        self.shots = shots;
        for (kind, from, at, close) in landed {
            if self.outcome != Outcome::Fighting {
                break;
            }
            match kind {
                ShotKind::Missile => self.missile_burst(at),
                ShotKind::Bolt if close => self.bolt_hit(at),
                ShotKind::Bolt => {
                    let _ = from;
                    self.float(Side::Player, "Dodged".into(), false, false);
                }
            }
        }
    }

    fn missile_burst(&mut self, at: Vec3) {
        self.flash(Effect::Blast {
            at,
            radius: MISSILE_BLAST,
            side: Side::Player,
        });
        let enemy = &self.enemy;
        if enemy.center().distance(at) > MISSILE_BLAST + enemy.radius() {
            return;
        }
        let save = self.dice.save(2, "Dexterity", ENEMY_DEX_SAVE, PLAYER_DC);
        let rolled = self.damage_roll(2, 6, false) as f32;
        let amount = if save.success {
            (rolled / 2.0).floor()
        } else {
            rolled
        };
        self.hit(Side::Enemy, amount, false, "Missile");
    }

    fn bolt_hit(&mut self, at: Vec3) {
        self.flash(Effect::Blast {
            at,
            radius: 1.2,
            side: Side::Enemy,
        });
        let attack = self.attack_roll(ENEMY_ATTACK, PLAYER_AC);
        if attack.hit {
            let amount = self.damage_roll(2, 8, attack.crit) as f32 + 2.0;
            self.hit(Side::Player, amount, attack.crit, "Plasma bolt");
        } else {
            self.float(Side::Player, "Miss".into(), false, false);
            self.note("The enemy's bolt misses");
        }
    }
}

/// The point on segment `a`–`b` nearest `p`.
fn closest(a: Vec3, b: Vec3, p: Vec3) -> Vec3 {
    let ab = b - a;
    let length = ab.length_squared();
    if length <= f32::EPSILON {
        return a;
    }
    a + ab * ((p - a).dot(ab) / length).clamp(0.0, 1.0)
}
