//! The duel's rules, deterministic from a seed.

use glam::Vec3;

use crate::rules::*;

/// A fight with both fighters under the test's command, standing `gap`
/// apart and facing each other.
pub(crate) fn staged(gap: f32) -> Fight {
    let mut fight = Fight::new(7);
    fight.ai = false;
    fight.hold_reads = true;
    fight.fighters[0].pos = Vec3::new(0.0, 0.0, -gap * 0.5);
    fight.fighters[1].pos = Vec3::new(0.0, 0.0, gap * 0.5);
    fight.fighters[0].yaw = 0.0;
    fight.fighters[1].yaw = std::f32::consts::PI;
    fight
}

/// Sets `side`'s shield fully up and covering against `dir`.
pub(crate) fn guard_against(fight: &mut Fight, side: Side, dir: Option<Dir>) {
    fight.steer(
        side,
        Steer {
            shield: true,
            ..Steer::default()
        },
    );
    for _ in 0..240 {
        // The other fighter's windup is what the shield reads; hold the
        // read by hand instead.
        let f = fight.fighter_mut(side);
        f.read = dir;
        f.raise = 1.0;
        fight.step();
    }
}

/// Lets `side` swing `dir` from guard and returns what happened.
pub(crate) fn swing(fight: &mut Fight, side: Side, dir: Dir) -> Vec<Event> {
    let start = fight.events.len();
    fight.command(side, Command::Attack(dir)).unwrap();
    let defender = side.other();
    let read = fight.fighters[defender.index()].read;
    for _ in 0..400 {
        // Keep the defender's guard as staged.
        if read.is_some() {
            fight.fighter_mut(defender).read = read;
        }
        fight.step();
        if !matches!(
            fight.fighters[side.index()].action,
            Action::Windup { .. } | Action::Release { .. }
        ) {
            break;
        }
    }
    fight.events[start..].to_vec()
}

/// Steps from `side`'s attack until the blow meets something; returns
/// the steps it took.
fn steps_to_contact(gap: f32, side: Side, dir: Dir) -> usize {
    let mut fight = staged(gap);
    fight.command(side, Command::Attack(dir)).unwrap();
    for i in 1..400 {
        fight.step();
        if fight
            .events
            .iter()
            .any(|e| matches!(e, Event::Hit { .. } | Event::Blocked { .. }))
        {
            return i;
        }
    }
    panic!("the blow never landed");
}

fn first(events: &[Event]) -> Event {
    *events
        .iter()
        .find(|e| !matches!(e, Event::Feint { .. }))
        .expect("something happened")
}

#[test]
fn a_raised_shield_turned_to_the_blow_blocks_every_direction() {
    for side in [Side::Player, Side::Enemy] {
        for dir in Dir::ALL {
            for gap in [1.4_f32, 1.6] {
                let mut fight = staged(gap);
                guard_against(&mut fight, side.other(), Some(dir));
                let event = first(&swing(&mut fight, side, dir));
                assert!(
                    matches!(event, Event::Blocked { .. }),
                    "{side:?} {dir:?} at {gap}: {event:?}"
                );
            }
        }
    }
}

#[test]
fn a_shield_on_the_wrong_side_leaves_the_other_open() {
    // The backhand lands on the weapon side while the shield covers the
    // shield side, and the forehand the other way round.
    for (dir, wrong) in [
        (Dir::Left, Dir::Right),
        (Dir::Right, Dir::Left),
        (Dir::Thrust, Dir::Overhead),
    ] {
        let mut fight = staged(1.5);
        guard_against(&mut fight, Side::Enemy, Some(wrong));
        let event = first(&swing(&mut fight, Side::Player, dir));
        assert!(matches!(event, Event::Hit { .. }), "{dir:?}: {event:?}");
    }
}

#[test]
fn the_shield_covers_a_frontal_arc_and_not_the_flank() {
    // The shield reaches only so far across the body: a blow from behind
    // the weapon shoulder gets past it.
    assert!(COVER_MIN > -1.0 && COVER_MAX < 1.6);
    let mut fight = staged(1.5);
    fight.hold_facing = true;
    // The Prussian stands at the knight's weapon side and a little behind.
    let a = -2.0_f32;
    let knight = fight.fighters[0].pos;
    fight.fighters[1].pos = knight + Vec3::new(-a.sin(), 0.0, a.cos()) * 1.4;
    fight.fighters[1].yaw = (knight - fight.fighters[1].pos)
        .x
        .atan2((knight - fight.fighters[1].pos).z);
    guard_against(&mut fight, Side::Player, Some(Dir::Right));
    let event = first(&swing(&mut fight, Side::Enemy, Dir::Right));
    assert!(
        matches!(
            event,
            Event::Hit {
                on: Side::Player,
                ..
            }
        ),
        "{event:?}"
    );
    // Face on, the same blow is blocked.
    let mut fight = staged(1.5);
    guard_against(&mut fight, Side::Player, Some(Dir::Right));
    let event = first(&swing(&mut fight, Side::Enemy, Dir::Right));
    assert!(matches!(event, Event::Blocked { .. }), "{event:?}");
}

#[test]
fn a_lowered_shield_lets_a_thrust_reach_the_body() {
    let mut fight = staged(1.5);
    let event = first(&swing(&mut fight, Side::Player, Dir::Thrust));
    assert!(
        matches!(
            event,
            Event::Hit {
                on: Side::Enemy,
                zone: Zone::Torso,
                blow: Blow::Thrust,
                ..
            }
        ),
        "{event:?}"
    );
}

#[test]
fn out_of_measure_a_blow_misses() {
    let mut fight = staged(2.3);
    let event = first(&swing(&mut fight, Side::Player, Dir::Overhead));
    assert_eq!(event, Event::Missed { by: Side::Player });
    assert!(matches!(
        fight.player().action,
        Action::Recover {
            why: After::Missed,
            ..
        }
    ));
}

#[test]
fn a_parry_inside_its_window_turns_the_blow_and_opens_a_riposte() {
    let contact = steps_to_contact(1.5, Side::Enemy, Dir::Right);
    let window = (PARRY_OPEN + PARRY_CLOSE) * 0.5;
    let early = PARRY_CLOSE + 0.08;
    let late = PARRY_OPEN * 0.5;
    for (lead, parried) in [(window, true), (early, false), (late, false)] {
        let mut fight = staged(1.5);
        fight
            .command(Side::Enemy, Command::Attack(Dir::Right))
            .unwrap();
        let at = contact - (lead / STEP).round() as usize;
        for i in 1..400 {
            if i == at {
                fight.command(Side::Player, Command::Parry).unwrap();
            }
            fight.step();
            if fight
                .events
                .iter()
                .any(|e| !matches!(e, Event::Feint { .. }))
            {
                break;
            }
        }
        let event = first(&fight.events);
        assert_eq!(
            matches!(
                event,
                Event::Parried {
                    by: Side::Player,
                    ..
                }
            ),
            parried,
            "parry {lead} s before the blow: {event:?}"
        );
        if parried {
            assert!(fight.now < fight.player().riposte_until);
            assert!(matches!(
                fight.enemy().action,
                Action::Recover {
                    why: After::Parried,
                    ..
                }
            ));
            // The riposte winds up quicker.
            let quick = fight.player().windup(Dir::Thrust, fight.now);
            assert!(quick < timing(Dir::Thrust, Weapon::ARMING_SWORD).windup * 0.7);
        }
    }
}

#[test]
fn a_parry_does_not_turn_a_blow_from_behind() {
    let contact = steps_to_contact(1.5, Side::Enemy, Dir::Right);
    let mut fight = staged(1.5);
    fight.hold_facing = true;
    fight.fighters[0].yaw = std::f32::consts::PI;
    fight
        .command(Side::Enemy, Command::Attack(Dir::Right))
        .unwrap();
    let at = contact - ((PARRY_OPEN + PARRY_CLOSE) * 0.5 / STEP).round() as usize;
    for i in 1..400 {
        if i == at {
            fight.command(Side::Player, Command::Parry).unwrap();
        }
        fight.step();
    }
    assert!(
        !fight
            .events
            .iter()
            .any(|e| matches!(e, Event::Parried { .. }))
    );
}

#[test]
fn mail_turns_cuts_far_more_often_than_thrusts() {
    let edge = Weapon::ARMING_SWORD.edge;
    for armour in [Armour::MAIL_PADDED, Armour::MAIL] {
        let cut = turn_chance(armour, Blow::Cut, edge);
        let thrust = turn_chance(armour, Blow::Thrust, edge);
        assert!(
            cut >= thrust + 0.35,
            "{}: cut {cut} thrust {thrust}",
            armour.name
        );
        assert!(armour.through_cut < armour.through_thrust);
    }
    for armour in [
        Armour::GREAT_HELM,
        Armour::SPANGENHELM,
        Armour::PADDED,
        Armour::WRAPS,
    ] {
        assert!(turn_chance(armour, Blow::Cut, edge) > turn_chance(armour, Blow::Thrust, edge));
    }
    // The great helm guards better than the spangenhelm, and padding is
    // poor against a point.
    let helm = |a: Armour| turn_chance(a, Blow::Cut, edge);
    assert!(helm(Armour::GREAT_HELM) > helm(Armour::SPANGENHELM));
    assert!(turn_chance(Armour::PADDED, Blow::Thrust, edge) < 0.15);
    assert_eq!(turn_chance(Armour::NONE, Blow::Cut, edge), 0.0);
    // A natural 1 always finds a gap.
    assert!(turn_chance(Armour::GREAT_HELM, Blow::Cut, -20) <= 0.95);
    // By zone: the knight's limbs are mail, the Prussian's forearms only
    // padding, his legs wrapped wool.
    assert_eq!(armour(Kit::Knight, Zone::Head), Armour::GREAT_HELM);
    assert_eq!(armour(Kit::Knight, Zone::LeftLeg), Armour::MAIL);
    assert_eq!(armour(Kit::Prussian, Zone::RightArm), Armour::PADDED);
    assert_eq!(armour(Kit::Prussian, Zone::LeftLeg), Armour::WRAPS);
    // The rolled saves follow the chance.
    let mut dice = verse_world::spells::dice::Dice::new(99);
    let mut turned = [0_u32; 2];
    for _ in 0..4000 {
        for (i, blow) in [Blow::Cut, Blow::Thrust].into_iter().enumerate() {
            let save = dice.save(
                1,
                "Armour",
                Armour::MAIL_PADDED.save(blow) - edge,
                ARMOUR_DC,
            );
            if save.success && save.roll != 1 {
                turned[i] += 1;
            }
        }
    }
    let rate = |n: u32| n as f32 / 4000.0;
    assert!((rate(turned[0]) - turn_chance(Armour::MAIL_PADDED, Blow::Cut, edge)).abs() < 0.03);
    assert!((rate(turned[1]) - turn_chance(Armour::MAIL_PADDED, Blow::Thrust, edge)).abs() < 0.03);
}

#[test]
fn damage_and_bleeding_differ_by_zone() {
    assert!(Zone::Head.weight() > Zone::Torso.weight());
    assert!(Zone::Torso.weight() > Zone::LeftArm.weight());
    assert!(Zone::Head.stagger() > Zone::LeftArm.stagger());
    assert!(bleed(Zone::Torso, Blow::Thrust).0 > bleed(Zone::Torso, Blow::Cut).0);
    assert!(bleed(Zone::Torso, Blow::Thrust).0 > bleed(Zone::LeftArm, Blow::Thrust).0);
    // A thrust that gets into the body bleeds and staggers.
    let mut fight = staged(1.5);
    let mut events = Vec::new();
    for _ in 0..20 {
        events = swing(&mut fight, Side::Player, Dir::Thrust);
        if events
            .iter()
            .any(|e| matches!(e, Event::Hit { turned: false, .. }))
        {
            break;
        }
        for _ in 0..240 {
            fight.step();
        }
    }
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Hit {
            turned: false,
            zone: Zone::Torso,
            ..
        }
    )));
    assert!(fight.enemy().bleeding(fight.now) > 0.0);
    let hp = fight.enemy().hp;
    for _ in 0..240 {
        fight.step();
    }
    assert!(fight.enemy().hp < hp, "bleeding takes health");
}

#[test]
fn stamina_drains_on_blows_and_the_shield_and_returns_at_rest() {
    let mut fight = staged(3.0);
    let full = fight.player().stamina;
    fight
        .command(Side::Player, Command::Attack(Dir::Overhead))
        .unwrap();
    let cost = timing(Dir::Overhead, Weapon::ARMING_SWORD).cost;
    assert!((fight.player().stamina - (full - cost)).abs() < 1e-3);
    // Holding the shield up drains it.
    let mut fight = staged(3.0);
    fight.steer(
        Side::Player,
        Steer {
            shield: true,
            ..Steer::default()
        },
    );
    for _ in 0..480 {
        fight.step();
    }
    let (_, drain) = Kit::Knight.wind();
    assert!(
        fight.player().stamina < full - drain * 1.5,
        "{}",
        fight.player().stamina
    );
    // Lowered, it comes back after a short pause.
    fight.steer(Side::Player, Steer::default());
    fight.fighters[0].stamina = 40.0;
    let low = fight.player().stamina;
    for _ in 0..(REGEN_DELAY / STEP) as usize {
        fight.step();
    }
    let paused = fight.player().stamina;
    assert!(paused <= low + 1.0);
    for _ in 0..240 {
        fight.step();
    }
    assert!(fight.player().stamina > paused + 10.0);
}

#[test]
fn a_block_costs_stamina_and_pushes_back_and_a_heavy_blow_breaks_a_tired_guard() {
    let mut fight = staged(1.5);
    guard_against(&mut fight, Side::Player, Some(Dir::Overhead));
    let before = fight.player().stamina;
    let pos = fight.player().pos;
    let event = first(&swing(&mut fight, Side::Enemy, Dir::Overhead));
    assert!(matches!(
        event,
        Event::Blocked {
            by: Side::Player,
            ..
        }
    ));
    assert!(fight.player().stamina < before - 15.0);
    for _ in 0..60 {
        fight.step();
    }
    assert!(
        fight.player().pos.z < pos.z - 0.05,
        "the blow drives him back"
    );
    // The same blow on a spent guard breaks it.
    let mut fight = staged(1.5);
    guard_against(&mut fight, Side::Player, Some(Dir::Overhead));
    fight.fighters[0].stamina = 8.0;
    let events = swing(&mut fight, Side::Enemy, Dir::Overhead);
    assert!(
        events.contains(&Event::GuardBroken { by: Side::Enemy }),
        "{events:?}"
    );
    assert!(matches!(fight.player().action, Action::Stagger { .. }));
    assert!(fight.command(Side::Player, Command::Bash).is_err());
}

#[test]
fn a_feint_changes_the_blow_for_a_stamina_cost() {
    let mut fight = staged(1.5);
    fight
        .command(Side::Player, Command::Attack(Dir::Right))
        .unwrap();
    for _ in 0..40 {
        fight.step();
    }
    let before = fight.player().stamina;
    fight
        .command(Side::Player, Command::Attack(Dir::Left))
        .unwrap();
    assert!((before - fight.player().stamina - FEINT_COST).abs() < 0.5);
    assert!(fight.events.contains(&Event::Feint {
        by: Side::Player,
        to: Dir::Left
    }));
    assert!(matches!(
        fight.player().action,
        Action::Windup {
            dir: Dir::Left,
            feints: 1,
            ..
        }
    ));
    // Repeating the same direction is not a feint, and a tired fighter
    // cannot feint.
    assert_eq!(
        fight.command(Side::Player, Command::Attack(Dir::Left)),
        Err(Refusal::Busy)
    );
    fight.fighters[0].stamina = FEINT_COST - 1.0;
    assert_eq!(
        fight.command(Side::Player, Command::Attack(Dir::Overhead)),
        Err(Refusal::Tired)
    );
    // Cancelling a windup costs less than a feint and returns to guard.
    fight.fighters[0].stamina = 50.0;
    fight.command(Side::Player, Command::Cancel).unwrap();
    assert_eq!(fight.player().action, Action::Guard);
    assert!((fight.player().stamina - (50.0 - CANCEL_COST)).abs() < 1e-3);
}

#[test]
fn a_late_feint_beats_a_shield_that_read_the_first_blow() {
    // Skurdo reads the forehand and turns his shield to it; the knight
    // switches to the backhand after the read lands.
    let mut fight = staged(1.5);
    fight.hold_reads = false;
    fight.steer(
        Side::Enemy,
        Steer {
            shield: true,
            ..Steer::default()
        },
    );
    for _ in 0..120 {
        fight.step();
    }
    fight
        .command(Side::Player, Command::Attack(Dir::Right))
        .unwrap();
    let mut switched = false;
    for _ in 0..400 {
        fight.step();
        if !switched && fight.enemy().read == Some(Dir::Right) && fight.enemy().cover > 0.6 {
            fight
                .command(Side::Player, Command::Attack(Dir::Left))
                .unwrap();
            switched = true;
        }
        if fight
            .events
            .iter()
            .any(|e| !matches!(e, Event::Feint { .. }))
        {
            break;
        }
    }
    assert!(switched);
    let event = first(&fight.events);
    assert!(
        matches!(
            event,
            Event::Hit {
                on: Side::Enemy,
                ..
            }
        ),
        "{event:?}"
    );
}

#[test]
fn a_bash_staggers_an_open_fighter_and_only_pushes_a_shield() {
    let mut fight = staged(1.1);
    fight.command(Side::Player, Command::Bash).unwrap();
    for _ in 0..200 {
        fight.step();
    }
    assert!(
        fight.events.contains(&Event::Bashed {
            on: Side::Enemy,
            guarded: false
        }),
        "{:?}",
        fight.events
    );
    let mut fight = staged(1.1);
    guard_against(&mut fight, Side::Enemy, None);
    let before = fight.enemy().stamina;
    fight.command(Side::Player, Command::Bash).unwrap();
    for _ in 0..200 {
        fight.step();
    }
    assert!(
        fight.events.contains(&Event::Bashed {
            on: Side::Enemy,
            guarded: true
        }),
        "{:?}",
        fight.events
    );
    assert!(fight.enemy().stamina < before - 10.0);
}

#[test]
fn a_quick_step_back_carries_the_fighter_out_of_reach() {
    let mut fight = staged(1.5);
    let z = fight.player().pos.z;
    fight
        .command(Side::Player, Command::Step(glam::Vec2::new(0.0, -1.0)))
        .unwrap();
    for _ in 0..(STEP_TIME / STEP) as usize {
        fight.step();
    }
    assert!(fight.player().pos.z < z - 0.6, "{}", fight.player().pos.z);
    assert!(fight.player().stamina < MAX_STAMINA - STEP_COST + 0.5);
}

#[test]
fn reaction_times_stay_within_human_bounds() {
    let mut brain = crate::ai::Brain::new(5);
    let samples: Vec<f32> = (0..2000).map(|_| brain.reaction()).collect();
    let min = samples.iter().copied().fold(f32::MAX, f32::min);
    let max = samples.iter().copied().fold(f32::MIN, f32::max);
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    assert!(min >= crate::ai::REACTION_MIN && max <= crate::ai::REACTION_MAX);
    assert!(max - min > 0.12, "they vary");
    assert!((0.25..0.33).contains(&mean), "{mean}");
    // In a fight, his shield turns to a blow no sooner than he can react.
    for seed in 0..12 {
        let mut fight = Fight::new(seed);
        fight.fighters[0].pos = Vec3::new(0.0, 0.0, -0.8);
        fight.fighters[1].pos = Vec3::new(0.0, 0.0, 0.8);
        fight.brain.next_attack = 100.0;
        for _ in 0..60 {
            fight.step();
        }
        let start = fight.now;
        fight
            .command(Side::Player, Command::Attack(Dir::Overhead))
            .unwrap();
        let mut seen = None;
        for _ in 0..240 {
            fight.step();
            if seen.is_none() && fight.enemy().read == Some(Dir::Overhead) {
                seen = Some(fight.now - start);
            }
        }
        let seen = seen.expect("he read the blow");
        assert!(
            (crate::ai::REACTION_MIN - STEP..=crate::ai::REACTION_MAX + 2.0 * STEP).contains(&seen),
            "seed {seed}: {seen}"
        );
    }
}

#[test]
fn skurdo_answers_blows_presses_and_wins_against_a_knight_who_does_nothing() {
    let mut fight = Fight::new(3);
    for _ in 0..(90.0 / STEP) as usize {
        fight.step();
        if fight.outcome != Outcome::Fighting {
            break;
        }
    }
    assert_eq!(fight.outcome, Outcome::Lost, "{:?}", fight.log);
    assert!(fight.player().hp <= 0.0);
}

#[test]
fn skurdo_blocks_or_parries_most_blows_he_sees_coming() {
    let mut fight = Fight::new(11);
    fight.brain.next_attack = 1000.0;
    let mut swings = 0;
    for _ in 0..(40.0 / STEP) as usize {
        // The knight walks in and swings forehands whenever he can.
        fight.steer(
            Side::Player,
            Steer {
                ahead: if gap(&fight) > 1.5 { 1.0 } else { 0.0 },
                ..Steer::default()
            },
        );
        if gap(&fight) < 1.7
            && fight
                .command(Side::Player, Command::Attack(Dir::Right))
                .is_ok()
        {
            swings += 1;
        }
        fight.brain.next_attack = 1000.0;
        fight.step();
        if fight.outcome != Outcome::Fighting {
            break;
        }
    }
    let defended = fight
        .events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Blocked {
                    by: Side::Enemy,
                    ..
                } | Event::Parried {
                    by: Side::Enemy,
                    ..
                }
            )
        })
        .count();
    let hits = fight
        .events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Hit {
                    on: Side::Enemy,
                    ..
                }
            )
        })
        .count();
    assert!(swings >= 8, "{swings}");
    assert!(defended > hits, "defended {defended}, hit {hits}");
}

fn gap(fight: &Fight) -> f32 {
    let (a, b) = (fight.player().pos, fight.enemy().pos);
    (a - b).with_y(0.0).length()
}

#[test]
fn a_seed_and_the_same_commands_replay_the_same_fight() {
    let run = || {
        let mut fight = Fight::new(21);
        for i in 0..(20.0 / STEP) as usize {
            if i % 300 == 0 {
                let _ = fight.command(Side::Player, Command::Attack(Dir::ALL[(i / 300) % 4]));
            }
            fight.steer(
                Side::Player,
                Steer {
                    ahead: 0.5,
                    side: 0.0,
                    shield: i % 600 > 300,
                },
            );
            fight.step();
        }
        (fight.events.clone(), fight.player().hp, fight.enemy().hp)
    };
    assert_eq!(run(), run());
}

#[test]
fn hit_stop_holds_the_fight_still_and_the_camera_shakes() {
    let mut fight = staged(1.5);
    let _ = swing(&mut fight, Side::Enemy, Dir::Overhead);
    assert!(fight.shake_now() > 0.0 || fight.freeze > 0.0);
    let mut fight = staged(1.5);
    fight.freeze = 0.08;
    let now = fight.now;
    fight.tick(0.05);
    assert_eq!(fight.now, now, "frozen");
    fight.tick(0.05);
    assert!(fight.now > now);
}

#[test]
fn what_the_player_reads_is_plain() {
    let mut text: Vec<String> = Vec::new();
    for kit in [Kit::Knight, Kit::Prussian] {
        for zone in Zone::ALL {
            for blow in [Blow::Cut, Blow::Thrust] {
                for (turned, gap) in [(true, false), (false, false), (false, true)] {
                    text.push(callout_text(
                        kit,
                        zone,
                        blow,
                        turned,
                        gap,
                        armour(kit, zone),
                    ));
                }
            }
        }
        text.push(kit.name().into());
        text.push(kit.title().into());
    }
    for refusal in [
        Refusal::Busy,
        Refusal::Tired,
        Refusal::ShieldDown,
        Refusal::Over,
    ] {
        text.push(refusal.message().into());
    }
    text.push(crate::hud::CONTROLS.into());
    let mut fight = Fight::new(3);
    for _ in 0..(90.0 / STEP) as usize {
        fight.step();
    }
    text.extend(fight.log.iter().cloned());
    text.extend(fight.callouts.iter().map(|c| c.text.clone()));
    text.push(crate::Duel::new().caption());
    for line in text {
        assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
    }
}

#[test]
fn the_meadow_and_both_fighters_draw_lit() {
    let duel = crate::Duel::new();
    let mesh = duel.dynamic();
    assert!(mesh.lit.len() > 3000, "{}", mesh.lit.len());
    assert!(mesh.lit.len() % 3 == 0);
    let world = crate::Duel::world();
    assert!(world.mesh.lit.len() > 100_000);
    assert!(
        world
            .mesh
            .lit
            .iter()
            .all(|v| v.pos.iter().all(|c| c.is_finite()))
    );
    assert!(
        mesh.lit
            .iter()
            .all(|v| v.normal.iter().all(|c| c.is_finite()))
    );
}

#[test]
#[ignore = "a fight's pacing, by eye"]
fn pacing() {
    for seed in 0..6 {
        let mut fight = Fight::new(seed);
        let mut t = 0;
        while fight.outcome == Outcome::Fighting && t < (180.0 / STEP) as usize {
            t += 1;
            let threat = fight.enemy().threat(fight.now).is_some();
            let d = gap(&fight);
            let ahead = if d > 1.6 {
                1.0
            } else if d < 1.2 {
                -1.0
            } else {
                0.0
            };
            fight.steer(
                Side::Player,
                Steer {
                    ahead,
                    side: 0.0,
                    shield: threat,
                },
            );
            let open = matches!(
                fight.enemy().action,
                Action::Recover { .. } | Action::Stagger { .. }
            );
            if !threat && (open || t % 200 == 0) && d < 1.7 {
                let dir = if open {
                    Dir::Thrust
                } else {
                    Dir::ALL[(t / 200) % 4]
                };
                let _ = fight.command(Side::Player, Command::Attack(dir));
            }
            fight.step();
        }
        let count = |f: &dyn Fn(&Event) -> bool| fight.events.iter().filter(|e| f(e)).count();
        println!(
            "seed {seed}: {:?} after {:.0} s, you {:.0} him {:.0}; his hits {} yours {}, blocks by him {} by you {}, parries {} feints {} misses {} {}",
            fight.outcome,
            fight.now,
            fight.player().hp,
            fight.enemy().hp,
            count(&|e| matches!(
                e,
                Event::Hit {
                    on: Side::Player,
                    ..
                }
            )),
            count(&|e| matches!(
                e,
                Event::Hit {
                    on: Side::Enemy,
                    ..
                }
            )),
            count(&|e| matches!(
                e,
                Event::Blocked {
                    by: Side::Enemy,
                    ..
                }
            )),
            count(&|e| matches!(
                e,
                Event::Blocked {
                    by: Side::Player,
                    ..
                }
            )),
            count(&|e| matches!(e, Event::Parried { .. })),
            count(&|e| matches!(e, Event::Feint { .. })),
            count(&|e| matches!(e, Event::Missed { by: Side::Player })),
            count(&|e| matches!(e, Event::Missed { by: Side::Enemy })),
        );
    }
}
