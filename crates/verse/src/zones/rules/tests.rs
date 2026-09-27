use super::*;
use std::collections::VecDeque;

struct FixedDice {
    faces: VecDeque<(u8, u8)>,
    calls: usize,
}

impl FixedDice {
    fn new(faces: &[(u8, u8)]) -> Self {
        Self {
            faces: faces.iter().copied().collect(),
            calls: 0,
        }
    }

    fn exhausted(&self) {
        assert!(self.faces.is_empty(), "unused dice: {:?}", self.faces);
    }
}

impl Dice for FixedDice {
    fn roll(&mut self, sides: u8) -> u8 {
        self.calls += 1;
        let (expected, face) = self.faces.pop_front().expect("unexpected die roll");
        assert_eq!(sides, expected);
        face
    }
}

fn encounter() -> Encounter {
    Encounter::new(&mut FixedDice::new(&[(20, 15), (20, 10)])).unwrap()
}

#[test]
fn attacks_override_totals_only_for_natural_one_and_twenty() {
    let mut dice = FixedDice::new(&[(20, 1), (20, 20), (20, 10)]);
    let miss = resolve_d20(&mut dice, TestKind::Attack, 100, 1, RollMode::Normal).unwrap();
    assert!(!miss.success);
    let critical = resolve_d20(&mut dice, TestKind::Attack, -100, 100, RollMode::Normal).unwrap();
    assert!(critical.success && critical.critical);
    let exact = resolve_d20(&mut dice, TestKind::Attack, 2, 12, RollMode::Normal).unwrap();
    assert!(exact.success && !exact.critical);
    dice.exhausted();
}

#[test]
fn checks_and_saves_do_not_have_attack_overrides() {
    for kind in [TestKind::AbilityCheck, TestKind::SavingThrow] {
        let mut dice = FixedDice::new(&[(20, 1), (20, 20)]);
        let one = resolve_d20(&mut dice, kind, 20, 10, RollMode::Normal).unwrap();
        assert!(one.success && !one.critical);
        let twenty = resolve_d20(&mut dice, kind, -5, 20, RollMode::Normal).unwrap();
        assert!(!twenty.success && !twenty.critical);
    }
}

#[test]
fn advantage_selects_one_die_and_cancels_disadvantage() {
    let mut dice = FixedDice::new(&[(20, 1), (20, 20), (20, 20), (20, 1), (20, 8)]);
    let advantage = resolve_d20(&mut dice, TestKind::Attack, 0, 10, RollMode::Advantage).unwrap();
    assert!(advantage.critical);
    assert_eq!(advantage.selected, 20);
    let disadvantage =
        resolve_d20(&mut dice, TestKind::Attack, 100, 1, RollMode::Disadvantage).unwrap();
    assert!(!disadvantage.success);
    assert_eq!(disadvantage.selected, 1);
    let mode = RollMode::from_sources(true, true);
    assert_eq!(mode, RollMode::Normal);
    resolve_d20(&mut dice, TestKind::Attack, 0, 8, mode).unwrap();
    dice.exhausted();
}

#[test]
fn critical_damage_doubles_dice_but_not_flat_modifier() {
    let mut dice = FixedDice::new(&[(6, 2), (6, 5), (10, 4), (10, 9)]);
    assert_eq!(roll_damage(&mut dice, 1, 6, 1, true).unwrap(), 8);
    assert_eq!(roll_damage(&mut dice, 1, 10, 0, true).unwrap(), 13);
    dice.exhausted();
}

#[test]
fn damage_dice_are_bounded_before_rolling() {
    let mut dice = FixedDice::new(&[]);
    for (count, sides) in [(0, 6), (17, 6), (1, 0), (1, 101)] {
        assert_eq!(
            roll_damage(&mut dice, count, sides, 0, true),
            Err(RuleError::InvalidDamageDice)
        );
    }
    assert_eq!(dice.calls, 0);
}

#[test]
fn dice_results_are_validated() {
    for face in [0, 21, 255] {
        assert_eq!(
            resolve_d20(
                &mut FixedDice::new(&[(20, face)]),
                TestKind::Attack,
                0,
                10,
                RollMode::Normal
            ),
            Err(RuleError::InvalidDie { sides: 20, face })
        );
    }
    assert_eq!(
        roll_damage(&mut FixedDice::new(&[(10, 11)]), 1, 10, 0, false),
        Err(RuleError::InvalidDie {
            sides: 10,
            face: 11
        })
    );
}

#[test]
fn seeded_dice_are_reproducible_and_within_bounds() {
    let mut left = SeededDice::new(5);
    let mut right = SeededDice::new(5);
    for _ in 0..1_000 {
        for sides in [6, 10, 20] {
            let a = left.roll(sides);
            assert!((1..=sides).contains(&a));
            assert_eq!(a, right.roll(sides));
        }
    }
    assert_eq!(left.roll(0), 0);
}

#[test]
fn fortitude_uses_damage_taken_and_constitution_modifier() {
    let mut dice = FixedDice::new(&[(20, 8), (20, 7)]);
    let save = damage_zombie(&mut dice, 1, 6, DamageType::Fire, false).unwrap();
    assert_eq!(save.hp, 1);
    assert_eq!(save.fortitude.unwrap().target, 11);
    assert_eq!(save.fortitude.unwrap().total, 11);
    let fail = damage_zombie(&mut dice, 1, 6, DamageType::Fire, false).unwrap();
    assert_eq!(fail.hp, 0);
    dice.exhausted();
}

#[test]
fn radiant_critical_and_nonlethal_damage_skip_fortitude() {
    let mut dice = FixedDice::new(&[]);
    for (damage_type, critical) in [(DamageType::Radiant, false), (DamageType::Fire, true)] {
        let result = damage_zombie(&mut dice, 2, 6, damage_type, critical).unwrap();
        assert_eq!(result.hp, 0);
        assert!(result.fortitude.is_none());
    }
    assert_eq!(
        damage_zombie(&mut dice, 22, 6, DamageType::Fire, false)
            .unwrap()
            .hp,
        16
    );
    assert_eq!(
        damage_zombie(&mut dice, 0, 6, DamageType::Fire, false)
            .unwrap()
            .hp,
        0
    );
    assert_eq!(dice.calls, 0);
}

#[test]
fn natural_twenty_does_not_save_zombie_against_impossible_dc() {
    let result = damage_zombie(
        &mut FixedDice::new(&[(20, 20)]),
        1,
        19,
        DamageType::Fire,
        false,
    )
    .unwrap();
    assert_eq!(result.hp, 0);
    assert_eq!(result.fortitude.unwrap().target, 24);
}

#[test]
fn initial_stats_and_tied_initiative_use_declared_demo_policy() {
    let state = Encounter::new(&mut FixedDice::new(&[(20, 8), (20, 12)]))
        .unwrap()
        .snapshot();
    assert_eq!(state.ruleset, RULESET_ID);
    assert_eq!(state.initiative, [10, 10]);
    assert_eq!(state.first, ActorId::Wizard);
    assert_eq!(state.round, 1);
    assert_eq!(state.turn, Turn::Wizard);
    assert_eq!((state.wizard.hp, state.wizard.armor_class), (12, 12));
    assert_eq!((state.zombie.hp, state.zombie.armor_class), (22, 8));
    assert_eq!(state.movement_remaining_m, WIZARD_SPEED_M);
}

#[test]
fn zombie_winning_initiative_acts_once_before_player() {
    let mut dice = FixedDice::new(&[(20, 1), (20, 20), (20, 12), (6, 3)]);
    let state = Encounter::new(&mut dice).unwrap().snapshot();
    assert_eq!(state.first, ActorId::Zombie);
    assert_eq!(state.round, 1);
    assert_eq!(state.wizard.hp, 8);
    assert!((state.zombie.position[1] + SLAM_REACH_M).abs() < EPSILON_M);
    assert_eq!(state.turn, Turn::Wizard);
    dice.exhausted();
}

#[test]
fn cast_spends_one_action_and_requires_end_turn_for_npc_response() {
    let mut encounter = encounter();
    let mut dice = FixedDice::new(&[(20, 10), (10, 6), (20, 1)]);
    encounter.cast(&mut dice).unwrap();
    let state = encounter.snapshot();
    assert_eq!(state.wizard.hp, 12);
    assert_eq!(state.zombie.hp, 16);
    assert!(!state.action_available);
    assert_eq!(encounter.cast(&mut dice), Err(RuleError::ActionSpent));
    assert_eq!(encounter.snapshot(), state);
    encounter.end_turn(&mut dice).unwrap();
    let next = encounter.snapshot();
    assert_eq!(next.round, 2);
    assert!(next.action_available);
    assert_eq!(next.movement_remaining_m, WIZARD_SPEED_M);
    assert!((next.zombie.position[1] + SLAM_REACH_M).abs() < EPSILON_M);
    assert_eq!(next.wizard.hp, 12);
    dice.exhausted();
}

#[test]
fn close_range_fire_bolt_uses_disadvantage() {
    let mut encounter = encounter();
    let mut dice = FixedDice::new(&[(20, 1), (20, 20), (20, 2)]);
    encounter.end_turn(&mut dice).unwrap();
    encounter.cast(&mut dice).unwrap();
    let state = encounter.snapshot();
    assert_eq!(state.zombie.hp, 22);
    let report = state.last_attack.unwrap();
    assert_eq!(report.roll.mode, RollMode::Disadvantage);
    assert_eq!(report.roll.selected, 2);
    assert!(!report.roll.critical);
    dice.exhausted();
}

#[test]
fn critical_fire_bolt_can_end_encounter_without_fortitude() {
    let mut encounter = encounter();
    encounter.state.zombie.hp = 15;
    let mut dice = FixedDice::new(&[(20, 20), (10, 8), (10, 9)]);
    encounter.cast(&mut dice).unwrap();
    let won = encounter.snapshot();
    assert_eq!(won.zombie.hp, 0);
    assert_eq!(won.status, EncounterStatus::Won);
    assert_eq!(won.turn, Turn::Finished);
    assert_eq!(won.last_attack.as_ref().unwrap().damage, 17);
    assert!(won.last_attack.as_ref().unwrap().fortitude.is_none());
    assert_eq!(
        encounter.end_turn(&mut dice),
        Err(RuleError::EncounterFinished)
    );
    assert_eq!(encounter.cast(&mut dice), Err(RuleError::EncounterFinished));
    assert_eq!(
        encounter.move_wizard_to([1.0, 0.0]),
        Err(RuleError::EncounterFinished)
    );
    assert_eq!(encounter.snapshot(), won);
    dice.exhausted();
}

#[test]
fn lethal_slam_finishes_without_starting_another_round() {
    let mut encounter = encounter();
    let mut dice = FixedDice::new(&[(20, 20), (6, 6), (6, 6)]);
    encounter.end_turn(&mut dice).unwrap();
    let state = encounter.snapshot();
    assert_eq!(state.wizard.hp, 0);
    assert_eq!(state.status, EncounterStatus::Lost);
    assert_eq!(state.round, 1);
    assert!(!state.action_available);
    assert_eq!(state.last_attack.unwrap().damage, 13);
    dice.exhausted();
}

#[test]
fn movement_budget_charges_each_segment_and_refreshes_on_turn() {
    let mut encounter = encounter();
    encounter.move_wizard_to([3.0, 0.0]).unwrap();
    encounter.move_wizard_to([0.0, 0.0]).unwrap();
    assert!((encounter.snapshot().movement_remaining_m - (WIZARD_SPEED_M - 6.0)).abs() < EPSILON_M);
    let before = encounter.snapshot();
    assert_eq!(
        encounter.move_wizard_to([4.0, 0.0]),
        Err(RuleError::MovementSpent)
    );
    assert_eq!(encounter.snapshot(), before);
    encounter.end_turn(&mut FixedDice::new(&[(20, 1)])).unwrap();
    assert_eq!(encounter.snapshot().movement_remaining_m, WIZARD_SPEED_M);
}

#[test]
fn zombie_movement_is_bounded_and_no_attack_occurs_outside_reach() {
    let mut encounter = encounter();
    encounter.move_wizard_to([0.0, 8.0]).unwrap();
    let before = encounter.snapshot();
    let mut dice = FixedDice::new(&[]);
    encounter.end_turn(&mut dice).unwrap();
    let after = encounter.snapshot();
    assert!(
        (distance(before.zombie.position, after.zombie.position) - ZOMBIE_SPEED_M).abs()
            < EPSILON_M
    );
    assert_eq!(after.wizard.hp, 12);
    assert_eq!(after.round, 2);
    assert!(after.last_attack.is_none());
    assert_eq!(dice.calls, 0);
}

#[test]
fn rejected_commands_leave_state_and_dice_unchanged() {
    let mut encounter = encounter();
    let before = encounter.snapshot();
    let mut dice = FixedDice::new(&[]);
    assert_eq!(
        encounter.cast_with_visibility(&mut dice, false),
        Err(RuleError::BlockedTarget)
    );
    for position in [[f32::NAN, 0.0], [0.0, f32::INFINITY], [129.0, 0.0]] {
        assert_eq!(
            encounter.move_wizard_to(position),
            Err(RuleError::InvalidPosition)
        );
    }
    assert_eq!(
        encounter.move_wizard_to([0.0, -9.0]),
        Err(RuleError::OccupiedPosition)
    );
    assert_eq!(encounter.snapshot(), before);
    assert_eq!(dice.calls, 0);
}

#[test]
fn failed_damage_roll_does_not_spend_action_or_hp() {
    let mut encounter = encounter();
    let before = encounter.snapshot();
    assert_eq!(
        encounter.cast(&mut FixedDice::new(&[(20, 20), (10, 5), (10, 0)])),
        Err(RuleError::InvalidDie { sides: 10, face: 0 })
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn failed_npc_roll_does_not_move_zombie_or_advance_turn() {
    let mut encounter = encounter();
    let before = encounter.snapshot();
    assert_eq!(
        encounter.end_turn(&mut FixedDice::new(&[(20, 0)])),
        Err(RuleError::InvalidDie { sides: 20, face: 0 })
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn failed_fortitude_roll_does_not_apply_any_damage() {
    let mut encounter = encounter();
    encounter.state.zombie.hp = 2;
    let before = encounter.snapshot();
    assert_eq!(
        encounter.cast(&mut FixedDice::new(&[(20, 10), (10, 5), (20, 21)])),
        Err(RuleError::InvalidDie {
            sides: 20,
            face: 21
        })
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn out_of_range_cast_preserves_action() {
    let mut encounter = encounter();
    encounter.state.zombie.position = [0.0, -FIRE_BOLT_RANGE_M - 0.1];
    let before = encounter.snapshot();
    assert_eq!(
        encounter.cast(&mut FixedDice::new(&[])),
        Err(RuleError::OutOfRange)
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn reset_restores_stats_positions_and_rerolls_initiative() {
    let mut encounter = encounter();
    encounter
        .cast(&mut FixedDice::new(&[(20, 10), (10, 4)]))
        .unwrap();
    encounter.move_wizard_to([1.0, 1.0]).unwrap();
    let revision = encounter.snapshot().revision;
    encounter
        .reset(&mut FixedDice::new(&[(20, 20), (20, 1)]))
        .unwrap();
    let state = encounter.snapshot();
    assert_eq!(state.initiative, [22, -1]);
    assert_eq!(state.wizard.position, [0.0, 0.0]);
    assert_eq!(state.wizard.hp, 12);
    assert_eq!(state.zombie.hp, 22);
    assert_eq!(state.round, 1);
    assert_eq!(state.status, EncounterStatus::Active);
    assert!(state.action_available);
    assert_eq!(state.movement_remaining_m, WIZARD_SPEED_M);
    assert!(state.last_attack.is_none());
    assert_eq!(state.revision, revision + 1);
}

#[test]
fn failed_reset_keeps_prior_encounter() {
    let mut encounter = encounter();
    let before = encounter.snapshot();
    assert_eq!(
        encounter.reset(&mut FixedDice::new(&[(20, 0)])),
        Err(RuleError::InvalidDie { sides: 20, face: 0 })
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn round_limit_refuses_without_another_npc_action() {
    let mut encounter = encounter();
    encounter.state.round = MAX_ROUNDS;
    let before = encounter.snapshot();
    assert_eq!(
        encounter.end_turn(&mut FixedDice::new(&[])),
        Err(RuleError::RoundLimit)
    );
    assert_eq!(encounter.snapshot(), before);
}

#[test]
fn encounter_cannot_walk_beyond_its_clear_arena() {
    let mut encounter = encounter();
    encounter.move_wizard_to([0.0, 8.0]).unwrap();
    encounter.end_turn(&mut FixedDice::new(&[])).unwrap();
    let before = encounter.snapshot();
    assert_eq!(
        encounter.move_wizard_to([0.0, ARENA_RADIUS_M + 0.01]),
        Err(RuleError::OutsideArena)
    );
    assert_eq!(encounter.snapshot(), before);
    encounter.move_wizard_to([0.0, ARENA_RADIUS_M]).unwrap();
}
