//! The duel's rules, replayed from fixed seeds and fixed frames.

use glam::Vec3;

use crate::Arena;
use crate::rules::{
    Ability, EMP_STUN, ENEMY_HP, Fight, GCD, Outcome, PLAYER_START, Refusal, Robot, SHIELD_ABSORB,
    SHIELD_SECONDS, Side,
};

const FRAME: f32 = 1.0 / 60.0;

fn run(fight: &mut Fight, seconds: f32) {
    for _ in 0..(seconds / FRAME).round() as usize {
        fight.tick(FRAME);
    }
}

/// A fight with the enemy standing `distance` meters in front of the player.
fn facing(distance: f32) -> Fight {
    let mut fight = Fight::new(7);
    fight.enemy.pos = PLAYER_START + Vec3::Z * distance;
    fight
}

#[test]
fn the_same_seed_and_presses_replay_the_same_fight() {
    let script = |seed| {
        let mut fight = Fight::new(seed);
        for step in 0..900 {
            if step % 70 == 0 {
                let _ = fight.press(Ability::Laser);
            }
            if step == 200 {
                let _ = fight.press(Ability::Missiles);
            }
            fight.tick(FRAME);
        }
        (
            fight.player.hp,
            fight.enemy.hp,
            fight.log.clone(),
            fight.enemy.pos,
        )
    };
    assert_eq!(script(11), script(11));
    assert_ne!(script(11).2, script(12).2);
}

#[test]
fn the_global_cooldown_holds_the_next_ability_for_one_second() {
    let mut fight = facing(10.0);
    assert_eq!(fight.press(Ability::Laser), Ok(()));
    assert_eq!(fight.press(Ability::Laser), Err(Refusal::Busy));
    assert_eq!(fight.press(Ability::Emp), Err(Refusal::Busy));
    // The shield is off the global cooldown.
    assert_eq!(fight.press(Ability::Shield), Ok(()));
    fight.now += GCD - 0.01;
    assert_eq!(fight.press(Ability::Laser), Err(Refusal::Busy));
    fight.now += 0.02;
    assert_eq!(fight.press(Ability::Laser), Ok(()));
}

#[test]
fn each_ability_waits_out_its_own_cooldown() {
    let mut fight = facing(10.0);
    assert_eq!(fight.press(Ability::Missiles), Ok(()));
    fight.now += 1.5;
    assert_eq!(fight.press(Ability::Missiles), Err(Refusal::NotReady));
    assert!((fight.remaining(Ability::Missiles) - 6.5).abs() < 1e-4);
    fight.now += 6.5;
    assert_eq!(fight.press(Ability::Missiles), Ok(()));
    assert_eq!(fight.press(Ability::Shield), Ok(()));
    fight.now += 14.9;
    assert_eq!(fight.press(Ability::Shield), Err(Refusal::NotReady));
    fight.now += 0.2;
    assert_eq!(fight.press(Ability::Shield), Ok(()));
}

#[test]
fn abilities_refuse_a_target_out_of_range() {
    let mut fight = facing(31.0);
    assert_eq!(fight.press(Ability::Laser), Err(Refusal::OutOfRange));
    assert_eq!(fight.press(Ability::Missiles), Ok(()));
    let mut fight = facing(6.5);
    assert_eq!(fight.press(Ability::Emp), Err(Refusal::OutOfRange));
    // A refusal costs nothing: no cooldown started.
    assert_eq!(fight.gcd_until, 0.0);
    assert_eq!(fight.press(Ability::Laser), Ok(()));
}

#[test]
fn the_shield_absorbs_damage_until_it_breaks_or_expires() {
    let mut fight = facing(10.0);
    fight.press(Ability::Shield).unwrap();
    let hp = fight.player.hp;
    fight.hit(Side::Player, 20.0, false, "Test");
    assert_eq!(fight.player.hp, hp);
    assert_eq!(fight.player.shield_at(fight.now), SHIELD_ABSORB - 20.0);
    fight.hit(Side::Player, 30.0, false, "Test");
    assert_eq!(fight.player.hp, hp - (30.0 - (SHIELD_ABSORB - 20.0)));
    assert_eq!(fight.player.shield_at(fight.now), 0.0);
    // A fresh shield lapses after its time.
    let mut robot = Robot::new(Side::Player);
    robot.raise_shield(SHIELD_ABSORB, SHIELD_SECONDS, 0.0);
    assert_eq!(robot.shield_at(SHIELD_SECONDS - 0.01), SHIELD_ABSORB);
    assert_eq!(robot.shield_at(SHIELD_SECONDS), 0.0);
    let (taken, absorbed) = robot.damage(10.0, SHIELD_SECONDS + 1.0);
    assert_eq!((taken, absorbed), (10.0, 0.0));
}

#[test]
fn repeated_stuns_diminish_then_stop() {
    let mut robot = Robot::new(Side::Enemy);
    assert_eq!(robot.stun(2.0, 0.0), 2.0);
    assert_eq!(robot.stun(2.0, 5.0), 1.0);
    assert_eq!(robot.stun(2.0, 10.0), 0.0);
    assert!(!robot.stunned(10.5));
    // After the window, the full stun lands again.
    assert_eq!(robot.stun(2.0, 26.0), 2.0);
    assert!(robot.stunned(27.9) && !robot.stunned(28.1));
}

#[test]
fn a_failed_save_against_the_emp_stuns_and_knocks_back() {
    let mut fight = facing(4.0);
    fight.dice_mut().force_save(2, 1).unwrap();
    let hp = fight.enemy.hp;
    fight.press(Ability::Emp).unwrap();
    assert!(fight.enemy.hp < hp, "3d6 lightning lands");
    assert!(fight.enemy.stunned(fight.now));
    assert!((fight.enemy.stunned_until - fight.now - EMP_STUN).abs() < 1e-4);
    let before = fight.distance();
    run(&mut fight, 1.0);
    assert!(fight.distance() > before + 5.0, "knocked back about 7 m");
    // A stunned enemy fires nothing and stands where it landed.
    assert!(fight.shots.iter().all(|s| s.from == Side::Player));
    let at = fight.enemy.pos;
    run(&mut fight, 0.5);
    assert!(fight.enemy.pos.distance(at) < 0.05);
}

#[test]
fn a_passed_save_halves_the_emp_and_skips_the_stun() {
    let mut fight = facing(4.0);
    fight.dice_mut().force_save(2, 20).unwrap();
    fight.press(Ability::Emp).unwrap();
    assert!(!fight.enemy.stunned(fight.now));
    assert!(fight.enemy.hp >= ENEMY_HP - 9.0, "half of 3d6 is at most 9");
    assert!(fight.floaters.iter().any(|f| f.text == "Resisted"));
}

#[test]
fn a_stun_breaks_the_slam_it_winds_up() {
    let mut fight = facing(3.0);
    fight.now = 5.0;
    fight.brain.slam_ready = 0.0;
    fight.tick(FRAME);
    assert!(fight.brain.windup.is_some(), "the enemy winds up a slam");
    fight.dice_mut().force_save(2, 1).unwrap();
    fight.press(Ability::Emp).unwrap();
    assert!(fight.brain.windup.is_none());
    assert!(fight.floaters.iter().any(|f| f.text == "Interrupted"));
    let hp = fight.player.hp;
    run(&mut fight, 1.0);
    assert_eq!(fight.player.hp, hp, "the broken slam never lands");
}

#[test]
fn missiles_take_time_to_reach_the_target() {
    let mut fight = facing(20.0);
    // Hold the enemy still so only the missiles move.
    fight.enemy.stun(30.0, 0.0);
    fight.press(Ability::Missiles).unwrap();
    assert_eq!(fight.shots.len(), 4);
    assert_eq!(fight.enemy.hp, ENEMY_HP);
    run(&mut fight, 0.3);
    assert_eq!(fight.enemy.hp, ENEMY_HP, "still in flight");
    run(&mut fight, 3.5);
    assert!(fight.shots.is_empty());
    assert!(fight.enemy.hp < ENEMY_HP, "the salvo landed");
}

#[test]
fn critical_hits_roll_twice_the_dice() {
    let mut fight = Fight::new(3);
    let normal: Vec<u32> = (0..400).map(|_| fight.damage_roll(2, 10, false)).collect();
    let crit: Vec<u32> = (0..400).map(|_| fight.damage_roll(2, 10, true)).collect();
    assert!(normal.iter().all(|d| (2..=20).contains(d)));
    assert!(crit.iter().all(|d| (4..=40).contains(d)));
    assert!(crit.iter().any(|d| *d > 20));
}

#[test]
fn attack_rolls_follow_the_d20_rules() {
    let mut fight = Fight::new(5);
    for _ in 0..500 {
        let attack = fight.attack_roll(7, 14);
        assert!((1..=20).contains(&attack.roll));
        assert_eq!(attack.crit, attack.roll == 20);
        let expected = attack.roll == 20 || (attack.roll != 1 && attack.roll as i32 + 7 >= 14);
        assert_eq!(attack.hit, expected);
    }
}

#[test]
fn the_enemy_closes_in_and_fires() {
    let mut fight = facing(26.0);
    let start = fight.distance();
    run(&mut fight, 2.5);
    assert!(fight.distance() < start - 3.0, "it approaches");
    run(&mut fight, 20.0);
    assert!(
        fight.player.hp < fight.player.max_hp,
        "its attacks land on a still player"
    );
    assert!(
        fight
            .log
            .iter()
            .any(|l| l.contains("Plasma bolt") || l.contains("bolt misses") || l.contains("Slam"))
            || fight.floaters.iter().any(|f| f.over == Side::Player)
    );
}

#[test]
fn destroying_the_enemy_wins_and_restart_begins_again() {
    let mut arena = Arena::new();
    arena.fight_mut().hit(Side::Enemy, 1_000.0, false, "Test");
    assert_eq!(arena.fight().outcome, Outcome::Won);
    assert_eq!(arena.press(0), Err(Refusal::Over));
    assert!(arena.message().is_some());
    let (spawn, _) = arena.restart();
    assert_eq!(spawn, PLAYER_START);
    assert_eq!(arena.round(), 1);
    assert_eq!(arena.fight().outcome, Outcome::Fighting);
    assert_eq!(arena.fight().enemy.hp, ENEMY_HP);
    assert!(arena.message().is_none());
}

#[test]
fn losing_stops_the_fight_and_the_player() {
    let mut arena = Arena::new();
    arena.fight_mut().hit(Side::Player, 1_000.0, false, "Test");
    assert_eq!(arena.fight().outcome, Outcome::Lost);
    let mut player = verse_world::social::controller::PlayerController::new(PLAYER_START, 0.0);
    let walk = verse_world::social::controller::InputState {
        forward: true,
        ..Default::default()
    };
    arena.move_player(&mut player, &walk, 0.5, crate::HALF_EXTENT);
    assert_eq!(player.pos, PLAYER_START);
}

#[test]
fn the_robots_never_overlap_or_leave_the_ring() {
    let mut arena = Arena::new();
    let mut player = verse_world::social::controller::PlayerController::new(PLAYER_START, 0.0);
    let walk = verse_world::social::controller::InputState {
        forward: true,
        ..Default::default()
    };
    for _ in 0..1200 {
        arena.move_player(&mut player, &walk, FRAME, crate::HALF_EXTENT);
        arena.tick(FRAME, &mut player);
        let f = arena.fight();
        assert!(f.distance() >= 1.59, "overlap at {}", f.distance());
        for robot in [&f.player, &f.enemy] {
            assert!(robot.pos.with_y(0.0).length() <= crate::rules::ARENA_RADIUS + 1e-3);
        }
    }
}

#[test]
fn the_screen_text_is_plain() {
    let mut text = vec![
        crate::hud::CONTROLS.to_owned(),
        crate::hud::CONTROLS_WEB.to_owned(),
        Arena::new().caption(),
    ];
    for refusal in [
        Refusal::Over,
        Refusal::Stunned,
        Refusal::Busy,
        Refusal::NotReady,
        Refusal::OutOfRange,
    ] {
        text.push(refusal.message().to_owned());
    }
    for ability in Ability::ALL {
        text.push(ability.name().to_owned());
        text.push(ability.summary().to_owned());
    }
    for line in text {
        assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
    }
}

#[test]
fn the_scene_draws_both_robots_and_the_ring() {
    let arena = Arena::new();
    let mesh = arena.dynamic();
    assert!(mesh.faces.len() > 300 && mesh.lines.len() > 300);
    let world = Arena::world();
    assert!(world.mesh.lines.len() > 256);
}
