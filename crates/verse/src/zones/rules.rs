//! A local, turn-based SRD 5.1 encounter subset, reimplemented from the reference.
//!
//! This module is independent of rendering, wall clocks, networks, and assets.
//! See `docs/verse/zone-rules.md` and `docs/verse/SRD-5.1-NOTICE.md` for scope,
//! sources, attribution, and the demo's explicit departures from a full game.

use serde::Serialize;
use std::fmt;

#[cfg(test)]
#[path = "rules/tests.rs"]
mod tests;

pub const RULESET_ID: &str = "srd-5.1-encounter-v1";
pub const METERS_PER_FOOT: f32 = 0.3048;
pub const WIZARD_SPEED_M: f32 = 30.0 * METERS_PER_FOOT;
pub const ZOMBIE_SPEED_M: f32 = 20.0 * METERS_PER_FOOT;
pub const SLAM_REACH_M: f32 = 5.0 * METERS_PER_FOOT;
pub const FIRE_BOLT_RANGE_M: f32 = 120.0 * METERS_PER_FOOT;
/// The curated encounter stays within its obstacle-free clearing.
pub const ARENA_RADIUS_M: f32 = 10.0;
const MAX_ROUNDS: u32 = 1_000;
const POSITION_LIMIT_M: f32 = 128.0;
const EPSILON_M: f32 = 0.000_01;

/// Supplies a die face. Every consumer validates the result before using it.
pub trait Dice {
    fn roll(&mut self, sides: u8) -> u8;
}

/// Reproducible local demo dice. These are not a multiplayer fairness protocol.
#[derive(Clone, Debug)]
pub struct SeededDice {
    state: u64,
}

impl SeededDice {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut n = self.state;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        n ^ (n >> 31)
    }
}

impl Dice for SeededDice {
    fn roll(&mut self, sides: u8) -> u8 {
        if sides < 2 {
            return 0;
        }
        let sides = u64::from(sides);
        let threshold = sides.wrapping_neg() % sides;
        loop {
            let n = self.next();
            if n >= threshold {
                return (n % sides + 1) as u8;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RollMode {
    Normal,
    Advantage,
    Disadvantage,
}

impl RollMode {
    /// Multiple sources do not stack; any advantage and disadvantage cancel.
    pub const fn from_sources(advantage: bool, disadvantage: bool) -> Self {
        match (advantage, disadvantage) {
            (true, false) => Self::Advantage,
            (false, true) => Self::Disadvantage,
            _ => Self::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    Attack,
    AbilityCheck,
    SavingThrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct D20Result {
    pub first: u8,
    pub second: Option<u8>,
    pub selected: u8,
    pub modifier: i16,
    pub total: i32,
    pub target: i16,
    pub success: bool,
    pub critical: bool,
    pub mode: RollMode,
}

pub fn resolve_d20(
    dice: &mut impl Dice,
    kind: TestKind,
    modifier: i16,
    target: i16,
    mode: RollMode,
) -> Result<D20Result, RuleError> {
    let first = die(dice, 20)?;
    let second = if mode == RollMode::Normal {
        None
    } else {
        Some(die(dice, 20)?)
    };
    let selected = match (mode, second) {
        (RollMode::Advantage, Some(second)) => first.max(second),
        (RollMode::Disadvantage, Some(second)) => first.min(second),
        _ => first,
    };
    let total = i32::from(selected) + i32::from(modifier);
    let attack = kind == TestKind::Attack;
    let success = if attack && selected == 1 {
        false
    } else if attack && selected == 20 {
        true
    } else {
        total >= i32::from(target)
    };
    Ok(D20Result {
        first,
        second,
        selected,
        modifier,
        total,
        target,
        success,
        critical: attack && selected == 20,
        mode,
    })
}

/// Critical hits roll twice the damage dice and add the flat modifier once.
pub fn roll_damage(
    dice: &mut impl Dice,
    count: u8,
    sides: u8,
    flat: i16,
    critical: bool,
) -> Result<u16, RuleError> {
    if !(1..=16).contains(&count) || !(2..=100).contains(&sides) {
        return Err(RuleError::InvalidDamageDice);
    }
    let count = if critical { count * 2 } else { count };
    let mut damage = i32::from(flat);
    for _ in 0..count {
        damage += i32::from(die(dice, sides)?);
    }
    Ok(damage.max(0) as u16)
}

fn die(dice: &mut impl Dice, sides: u8) -> Result<u8, RuleError> {
    let face = dice.roll(sides);
    if !(1..=sides).contains(&face) {
        return Err(RuleError::InvalidDie { sides, face });
    }
    Ok(face)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Fire,
    Bludgeoning,
    Radiant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DamageResult {
    pub hp: u16,
    pub damage: u16,
    pub fortitude: Option<D20Result>,
}

/// The Zombie's Constitution +3 save applies only to a noncritical,
/// nonradiant hit that reduces its positive hit points to zero.
pub fn damage_zombie(
    dice: &mut impl Dice,
    hp: u16,
    damage: u16,
    damage_type: DamageType,
    critical: bool,
) -> Result<DamageResult, RuleError> {
    if hp > 22 || damage > 3_200 {
        return Err(RuleError::InvalidDamage);
    }
    let mut remaining = hp.saturating_sub(damage);
    let fortitude = if hp > 0 && remaining == 0 && damage_type != DamageType::Radiant && !critical {
        let save = resolve_d20(
            dice,
            TestKind::SavingThrow,
            3,
            (5 + damage) as i16,
            RollMode::Normal,
        )?;
        if save.success {
            remaining = 1;
        }
        Some(save)
    } else {
        None
    };
    Ok(DamageResult {
        hp: remaining,
        damage,
        fortitude,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorId {
    Wizard,
    Zombie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Turn {
    Wizard,
    Zombie,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterStatus {
    Active,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ActorSnapshot {
    /// Local encounter XZ coordinates in meters; the host supplies its origin.
    pub position: [f32; 2],
    pub hp: u16,
    pub max_hp: u16,
    pub armor_class: i16,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttackReport {
    pub actor: ActorId,
    pub roll: D20Result,
    pub damage: u16,
    pub target_hp: u16,
    pub fortitude: Option<D20Result>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EncounterSnapshot {
    pub ruleset: &'static str,
    pub wizard: ActorSnapshot,
    pub zombie: ActorSnapshot,
    /// Wizard, then zombie, irrespective of the resulting turn order.
    pub initiative: [i32; 2],
    pub first: ActorId,
    pub round: u32,
    pub turn: Turn,
    pub status: EncounterStatus,
    pub action_available: bool,
    pub movement_remaining_m: f32,
    pub last_attack: Option<AttackReport>,
    pub last_notice: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Encounter {
    state: EncounterSnapshot,
}

impl Encounter {
    /// Starts a fresh local encounter. A zombie that wins initiative takes its
    /// first turn here, before control passes to the wizard.
    pub fn new(dice: &mut impl Dice) -> Result<Self, RuleError> {
        let wizard_initiative = i32::from(die(dice, 20)?) + 2;
        let zombie_initiative = i32::from(die(dice, 20)?) - 2;
        let wizard_first = wizard_initiative >= zombie_initiative;
        let mut encounter = Self {
            state: EncounterSnapshot {
                ruleset: RULESET_ID,
                wizard: ActorSnapshot {
                    position: [0.0, 0.0],
                    hp: 12,
                    max_hp: 12,
                    armor_class: 12,
                },
                zombie: ActorSnapshot {
                    position: [0.0, -25.0 * METERS_PER_FOOT],
                    hp: 22,
                    max_hp: 22,
                    armor_class: 8,
                },
                initiative: [wizard_initiative, zombie_initiative],
                first: if wizard_first {
                    ActorId::Wizard
                } else {
                    ActorId::Zombie
                },
                round: 1,
                turn: Turn::Wizard,
                status: EncounterStatus::Active,
                action_available: true,
                movement_remaining_m: WIZARD_SPEED_M,
                last_attack: None,
                last_notice: format!(
                    "Initiative {wizard_initiative} / {zombie_initiative}. Your turn."
                ),
                revision: 0,
            },
        };
        if !wizard_first {
            encounter.zombie_turn(dice)?;
            encounter.begin_wizard_turn();
        }
        Ok(encounter)
    }

    pub fn snapshot(&self) -> EncounterSnapshot {
        self.state.clone()
    }

    /// Casts in a clear encounter arena. Hosts with intervening geometry must
    /// use `cast_with_visibility` after checking the shared collision geometry.
    pub fn cast(&mut self, dice: &mut impl Dice) -> Result<(), RuleError> {
        self.cast_with_visibility(dice, true)
    }

    pub fn cast_with_visibility(
        &mut self,
        dice: &mut impl Dice,
        line_of_sight: bool,
    ) -> Result<(), RuleError> {
        self.require_wizard_turn()?;
        if !self.state.action_available {
            return Err(RuleError::ActionSpent);
        }
        if !line_of_sight {
            return Err(RuleError::BlockedTarget);
        }
        let distance = self.distance();
        if distance > FIRE_BOLT_RANGE_M + EPSILON_M {
            return Err(RuleError::OutOfRange);
        }
        let mut next = self.clone();
        let mode = if distance <= SLAM_REACH_M + EPSILON_M {
            RollMode::Disadvantage
        } else {
            RollMode::Normal
        };
        let roll = resolve_d20(dice, TestKind::Attack, 5, 8, mode)?;
        let damage = if roll.success {
            roll_damage(dice, 1, 10, 0, roll.critical)?
        } else {
            0
        };
        let applied = damage_zombie(
            dice,
            next.state.zombie.hp,
            damage,
            DamageType::Fire,
            roll.critical,
        )?;
        next.state.zombie.hp = applied.hp;
        next.state.action_available = false;
        next.state.last_notice = if roll.success {
            format!(
                "Fire Bolt: {damage} fire{}.",
                if roll.critical { " (critical)" } else { "" }
            )
        } else {
            "Fire Bolt missed. End your turn.".into()
        };
        if applied.fortitude.is_some_and(|save| save.success) {
            next.state.last_notice.push_str(" Undead Fortitude: 1 HP.");
        }
        next.state.last_attack = Some(AttackReport {
            actor: ActorId::Wizard,
            roll,
            damage,
            target_hp: applied.hp,
            fortitude: applied.fortitude,
        });
        if applied.hp == 0 {
            next.finish(EncounterStatus::Won);
        }
        next.state.revision = next.state.revision.saturating_add(1);
        *self = next;
        Ok(())
    }

    /// Ends the wizard's turn and resolves exactly one zombie turn. Waiting,
    /// pausing, or rendering frames never invokes this operation implicitly.
    pub fn end_turn(&mut self, dice: &mut impl Dice) -> Result<(), RuleError> {
        self.require_wizard_turn()?;
        if self.state.round >= MAX_ROUNDS {
            return Err(RuleError::RoundLimit);
        }
        let mut next = self.clone();
        if next.state.first == ActorId::Zombie {
            next.state.round += 1;
        }
        next.zombie_turn(dice)?;
        if next.state.first == ActorId::Wizard && next.state.status == EncounterStatus::Active {
            next.state.round += 1;
        }
        next.begin_wizard_turn();
        next.state.revision = next.state.revision.saturating_add(1);
        *self = next;
        Ok(())
    }

    /// Admits an already collision-checked destination, charging actual path
    /// segments rather than the straight-line distance to a remote waypoint.
    pub fn move_wizard_to(&mut self, position: [f32; 2]) -> Result<(), RuleError> {
        self.require_wizard_turn()?;
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > POSITION_LIMIT_M)
        {
            return Err(RuleError::InvalidPosition);
        }
        if position[0].hypot(position[1]) > ARENA_RADIUS_M + EPSILON_M {
            return Err(RuleError::OutsideArena);
        }
        let travelled = distance(self.state.wizard.position, position);
        if travelled > self.state.movement_remaining_m + EPSILON_M {
            return Err(RuleError::MovementSpent);
        }
        if segment_distance(
            self.state.wizard.position,
            position,
            self.state.zombie.position,
        ) < 0.5
        {
            return Err(RuleError::OccupiedPosition);
        }
        self.state.wizard.position = position;
        self.state.movement_remaining_m = (self.state.movement_remaining_m - travelled).max(0.0);
        self.state.revision = self.state.revision.saturating_add(1);
        Ok(())
    }

    pub fn reset(&mut self, dice: &mut impl Dice) -> Result<(), RuleError> {
        let mut fresh = Self::new(dice)?;
        fresh.state.revision = self.state.revision.saturating_add(1);
        *self = fresh;
        Ok(())
    }

    fn require_wizard_turn(&self) -> Result<(), RuleError> {
        if self.state.status != EncounterStatus::Active {
            return Err(RuleError::EncounterFinished);
        }
        if self.state.turn != Turn::Wizard {
            return Err(RuleError::NotYourTurn);
        }
        Ok(())
    }

    fn distance(&self) -> f32 {
        distance(self.state.wizard.position, self.state.zombie.position)
    }

    fn zombie_turn(&mut self, dice: &mut impl Dice) -> Result<(), RuleError> {
        self.state.turn = Turn::Zombie;
        self.state.action_available = false;
        self.state.movement_remaining_m = 0.0;
        let initial_distance = self.distance();
        if initial_distance > SLAM_REACH_M {
            let step = (initial_distance - SLAM_REACH_M).min(ZOMBIE_SPEED_M);
            for axis in 0..2 {
                self.state.zombie.position[axis] += (self.state.wizard.position[axis]
                    - self.state.zombie.position[axis])
                    / initial_distance
                    * step;
            }
        }
        if self.distance() > SLAM_REACH_M + EPSILON_M {
            self.state.last_attack = None;
            self.state.last_notice = "Zombie approaches. Your turn.".into();
            return Ok(());
        }
        let roll = resolve_d20(dice, TestKind::Attack, 3, 12, RollMode::Normal)?;
        let damage = if roll.success {
            roll_damage(dice, 1, 6, 1, roll.critical)?
        } else {
            0
        };
        self.state.wizard.hp = self.state.wizard.hp.saturating_sub(damage);
        self.state.last_notice = if roll.success {
            format!("Zombie Slam: {damage} damage. Your turn.")
        } else {
            "Zombie Slam missed. Your turn.".into()
        };
        self.state.last_attack = Some(AttackReport {
            actor: ActorId::Zombie,
            roll,
            damage,
            target_hp: self.state.wizard.hp,
            fortitude: None,
        });
        if self.state.wizard.hp == 0 {
            self.finish(EncounterStatus::Lost);
        }
        Ok(())
    }

    fn begin_wizard_turn(&mut self) {
        if self.state.status == EncounterStatus::Active {
            self.state.turn = Turn::Wizard;
            self.state.action_available = true;
            self.state.movement_remaining_m = WIZARD_SPEED_M;
        }
    }

    fn finish(&mut self, status: EncounterStatus) {
        self.state.status = status;
        self.state.turn = Turn::Finished;
        self.state.action_available = false;
        self.state.movement_remaining_m = 0.0;
        self.state.last_notice = if status == EncounterStatus::Won {
            "Zombie defeated. Reset to play again.".into()
        } else {
            "Wizard defeated. Reset to play again.".into()
        };
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn segment_distance(a: [f32; 2], b: [f32; 2], point: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let length_squared = ab[0] * ab[0] + ab[1] * ab[1];
    if length_squared <= f32::EPSILON {
        return distance(a, point);
    }
    let fraction =
        (((point[0] - a[0]) * ab[0] + (point[1] - a[1]) * ab[1]) / length_squared).clamp(0.0, 1.0);
    distance([a[0] + ab[0] * fraction, a[1] + ab[1] * fraction], point)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleError {
    InvalidDie { sides: u8, face: u8 },
    InvalidDamageDice,
    InvalidDamage,
    InvalidPosition,
    OccupiedPosition,
    OutsideArena,
    EncounterFinished,
    NotYourTurn,
    ActionSpent,
    MovementSpent,
    OutOfRange,
    BlockedTarget,
    RoundLimit,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDie { sides, face } => write!(f, "Invalid d{sides} result: {face}."),
            Self::InvalidDamageDice => f.write_str("Unsupported damage dice."),
            Self::InvalidDamage => f.write_str("Damage is outside the encounter limits."),
            Self::InvalidPosition => f.write_str("Position is outside the encounter."),
            Self::OccupiedPosition => f.write_str("The zombie occupies that position."),
            Self::OutsideArena => f.write_str("Stay inside the encounter circle."),
            Self::EncounterFinished => f.write_str("Encounter complete. Reset to play again."),
            Self::NotYourTurn => f.write_str("Wait for your turn."),
            Self::ActionSpent => f.write_str("Action spent. End your turn."),
            Self::MovementSpent => f.write_str("Movement spent. End your turn."),
            Self::OutOfRange => f.write_str("Target is beyond Fire Bolt range."),
            Self::BlockedTarget => f.write_str("Target is blocked."),
            Self::RoundLimit => f.write_str("Encounter round limit reached. Reset to play again."),
        }
    }
}

impl std::error::Error for RuleError {}
