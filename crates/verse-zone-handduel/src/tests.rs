use glam::Vec3;
use openagents_web_hands::Mapping;
use openagents_web_hands::synth::{self, Pose};
use openagents_web_hands::track::{RawFrame, RawHand};

use crate::body::{Body, Grip, HandPose, Side, reach};
use crate::gesture::{
    BELT_Y, CHEST, DRINK_HOLD, Event, Gestures, MOUTH, Potion, SHEATHE_HOLD, Swing, shield_blocks,
};
use crate::rules::{
    BLOCK_CONE, Blow, BlowResult, Fight, Outcome, PLAYER_REACH, Phase, SwingResult, WARDEN_REACH,
    WINDUP,
};
use crate::{HandDuel, hud};

const FRAME: f64 = 1.0 / 30.0;

fn still(pos: Vec3, grip: Grip) -> HandPose {
    HandPose::at(pos, grip)
}

/// A hand moving from `from` to `to` over `seconds`, sampled at 30 Hz,
/// with the velocity each sample would read.
fn path(from: Vec3, to: Vec3, seconds: f64, grip: Grip) -> Vec<HandPose> {
    let n = (seconds / FRAME).round().max(1.0) as usize;
    let vel = (to - from) / seconds as f32;
    (0..=n)
        .map(|i| HandPose {
            pos: from.lerp(to, i as f32 / n as f32),
            up: Vec3::Y,
            vel,
            grip,
        })
        .collect()
}

/// Feeds right-hand poses, then a still hand, and returns the events.
fn right_only(g: &mut Gestures, poses: &[HandPose], t: &mut f64) -> Vec<Event> {
    let mut events = Vec::new();
    for pose in poses {
        *t += FRAME;
        events.extend(g.update(None, Some(pose), *t));
    }
    let rest = HandPose {
        vel: Vec3::ZERO,
        ..*poses.last().unwrap()
    };
    for _ in 0..3 {
        *t += FRAME;
        events.extend(g.update(None, Some(&rest), *t));
    }
    events
}

fn left_only(g: &mut Gestures, pose: HandPose, seconds: f64, t: &mut f64) -> Vec<Event> {
    let mut events = Vec::new();
    let n = (seconds / FRAME).ceil() as usize;
    for _ in 0..n.max(1) {
        *t += FRAME;
        events.extend(g.update(Some(&pose), None, *t));
    }
    events
}

// --- The shield's angle check ---

#[test]
fn a_shield_blocks_only_blows_along_the_line_it_covers() {
    let left_high = Vec3::new(0.5, 1.45, 0.4) - CHEST;
    assert!(shield_blocks(left_high, Blow::Left.line(), BLOCK_CONE));
    assert!(!shield_blocks(left_high, Blow::Right.line(), BLOCK_CONE));
    assert!(!shield_blocks(left_high, Blow::Overhead.line(), BLOCK_CONE));

    let overhead = Vec3::new(0.0, 1.95, 0.45) - CHEST;
    assert!(shield_blocks(overhead, Blow::Overhead.line(), BLOCK_CONE));
    assert!(!shield_blocks(overhead, Blow::Thrust.line(), BLOCK_CONE));

    let middle = Vec3::new(0.0, 1.3, 0.6) - CHEST;
    assert!(shield_blocks(middle, Blow::Thrust.line(), BLOCK_CONE));

    // Just inside and just outside the cone.
    let line = Blow::Thrust.line().normalize();
    let side = line.cross(Vec3::Y).normalize();
    let tilt = |a: f32| line * a.cos() + side * a.sin();
    assert!(shield_blocks(tilt(BLOCK_CONE - 0.02), line, BLOCK_CONE));
    assert!(!shield_blocks(tilt(BLOCK_CONE + 0.02), line, BLOCK_CONE));
    assert!(!shield_blocks(Vec3::ZERO, line, BLOCK_CONE));
}

#[test]
fn the_left_hand_raises_and_lowers_the_shield_with_hysteresis() {
    let mut g = Gestures::default();
    let mut t = 0.0;
    let low = left_only(
        &mut g,
        still(Vec3::new(0.3, 1.0, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    assert!(low.is_empty() && !g.shield_up);
    let up = left_only(
        &mut g,
        still(Vec3::new(0.3, 1.3, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    assert_eq!(up, vec![Event::Shield(true)]);
    // Between the two heights it stays up.
    left_only(
        &mut g,
        still(Vec3::new(0.3, 1.16, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    assert!(g.shield_up);
    let down = left_only(
        &mut g,
        still(Vec3::new(0.3, 1.05, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    assert_eq!(down, vec![Event::Shield(false)]);
    // Out of sight, it hangs at the side.
    left_only(
        &mut g,
        still(Vec3::new(0.3, 1.4, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    assert!(g.shield_up);
    assert_eq!(g.update(None, None, t + FRAME), vec![Event::Shield(false)]);
}

#[test]
fn a_raised_shield_stops_the_wardens_blow_and_a_wrong_one_does_not() {
    let mut fight = Fight::new(7);
    let shield = Vec3::new(0.5, 1.45, 0.4);
    fight.set_guard(Some((shield - CHEST, shield)));
    assert_eq!(fight.strike(Blow::Left), BlowResult::Blocked);
    assert_eq!(fight.player.hp, fight.player.max_hp);
    assert!(matches!(fight.warden.phase, Phase::Recover { .. }));

    // The same shield against a blow from the right: the dice decide.
    let mut hits = 0;
    for seed in 0..40 {
        let mut fight = Fight::new(seed);
        fight.set_guard(Some((shield - CHEST, shield)));
        match fight.strike(Blow::Right) {
            BlowResult::Blocked => panic!("the shield faces the other way"),
            BlowResult::Hit { damage, .. } => {
                hits += 1;
                assert!((4.0..=26.0).contains(&damage), "{damage}");
                assert!(fight.player.hp < fight.player.max_hp);
            }
            BlowResult::Miss => assert_eq!(fight.player.hp, fight.player.max_hp),
        }
    }
    assert!(hits > 20 && hits < 40, "{hits}");
}

// --- Swing detection ---

#[test]
fn a_fast_arc_is_a_swing_and_a_slow_one_is_not() {
    let mut g = Gestures::default();
    let mut t = 0.0;
    let fast = path(
        Vec3::new(-0.6, 1.5, 0.4),
        Vec3::new(0.5, 1.2, 0.5),
        0.3,
        Grip::Closed,
    );
    let events = right_only(&mut g, &fast, &mut t);
    let swings: Vec<&Swing> = events
        .iter()
        .filter_map(|e| match e {
            Event::Swing(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(swings.len(), 1, "{events:?}");
    let swing = swings[0];
    assert!(swing.peak > 3.0, "{}", swing.peak);
    assert!(swing.arc > 0.9, "{}", swing.arc);
    assert!(swing.path.len() >= 8);
    // The tip path runs from the right (`-x`) to the left (`+x`).
    assert!(swing.path[0].x < swing.path.last().unwrap().x);

    let slow = path(
        Vec3::new(-0.6, 1.5, 0.4),
        Vec3::new(0.5, 1.2, 0.5),
        1.5,
        Grip::Closed,
    );
    let events = right_only(&mut g, &slow, &mut t);
    assert!(!events.iter().any(|e| matches!(e, Event::Swing(_))));

    // A flick too short to be a blow.
    let flick = path(
        Vec3::new(-0.2, 1.4, 0.4),
        Vec3::new(-0.1, 1.4, 0.4),
        0.05,
        Grip::Closed,
    );
    let events = right_only(&mut g, &flick, &mut t);
    assert!(!events.iter().any(|e| matches!(e, Event::Swing(_))));
}

#[test]
fn a_sheathed_sword_does_not_swing() {
    let mut g = Gestures::default();
    g.drawn = false;
    let mut t = 0.0;
    let fast = path(
        Vec3::new(-0.6, 1.5, 0.4),
        Vec3::new(0.5, 1.2, 0.5),
        0.3,
        Grip::Closed,
    );
    assert!(right_only(&mut g, &fast, &mut t).is_empty());
}

#[test]
fn a_blow_that_carries_the_hand_out_of_view_still_lands() {
    let mut g = Gestures::default();
    let mut t = 0.0;
    let fast = path(
        Vec3::new(-0.6, 1.6, 0.4),
        Vec3::new(0.0, 1.1, 0.5),
        0.2,
        Grip::Closed,
    );
    for pose in &fast {
        t += FRAME;
        assert!(g.update(None, Some(pose), t).is_empty());
    }
    let events = g.update(None, None, t + FRAME);
    assert!(matches!(events.as_slice(), [Event::Swing(_)]), "{events:?}");
}

// --- Potion to mouth ---

#[test]
fn a_potion_comes_from_the_belt_to_the_mouth_and_is_drunk() {
    let mut g = Gestures::new(3);
    let mut t = 0.0;
    let belt = Vec3::new(0.3, BELT_Y - 0.1, 0.3);
    // An open hand at the belt takes nothing.
    assert!(left_only(&mut g, still(belt, Grip::Open), 0.1, &mut t).is_empty());
    let took = left_only(&mut g, still(belt, Grip::Closed), 0.1, &mut t);
    assert_eq!(took, vec![Event::PotionTaken]);
    assert!(matches!(g.potion, Potion::Held { .. }));

    // Raised with a potion in hand, the shield stays down.
    let chest = Vec3::new(0.3, 1.4, 0.3);
    left_only(&mut g, still(chest, Grip::Closed), 0.1, &mut t);
    assert!(!g.shield_up);

    // At the mouth, not long enough.
    let mouth = MOUTH + Vec3::new(-0.05, -0.05, 0.0);
    left_only(&mut g, still(mouth, Grip::Closed), DRINK_HOLD * 0.5, &mut t);
    assert_eq!(g.potions, 3);
    assert!(g.drink_progress(t) > 0.2 && g.drink_progress(t) < 1.0);
    let drank = left_only(&mut g, still(mouth, Grip::Closed), DRINK_HOLD, &mut t);
    // Drunk, the hand at the face holds the shield up again.
    assert_eq!(drank, vec![Event::Drink, Event::Shield(true)]);
    assert_eq!(g.potions, 2);
    assert_eq!(g.potion, Potion::Belt);
}

#[test]
fn an_open_hand_puts_the_potion_back_and_an_empty_belt_gives_none() {
    let mut g = Gestures::new(1);
    let mut t = 0.0;
    let belt = Vec3::new(0.3, BELT_Y - 0.1, 0.3);
    left_only(&mut g, still(belt, Grip::Closed), 0.1, &mut t);
    let back = left_only(
        &mut g,
        still(Vec3::new(0.3, 1.3, 0.3), Grip::Open),
        0.1,
        &mut t,
    );
    assert_eq!(back[0], Event::PotionReturned);
    assert_eq!(g.potions, 1);

    let mut empty = Gestures::new(0);
    assert!(left_only(&mut empty, still(belt, Grip::Closed), 0.2, &mut t).is_empty());
}

#[test]
fn reaching_below_the_picture_on_the_left_takes_a_potion() {
    let mut g = Gestures::new(2);
    let mut t = 0.0;
    left_only(
        &mut g,
        still(Vec3::new(0.3, 1.1, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    // Gone for half a second, below the picture.
    t += 0.5;
    assert!(g.update(None, None, t).is_empty());
    let back = left_only(
        &mut g,
        still(Vec3::new(0.3, 1.2, 0.3), Grip::Closed),
        0.05,
        &mut t,
    );
    assert_eq!(back, vec![Event::PotionTaken]);
}

// --- Sheathe, draw, and bash ---

#[test]
fn the_sword_sheathes_with_an_open_hand_over_the_shoulder_and_draws_with_a_fist() {
    let mut g = Gestures::default();
    let mut t = 0.0;
    let shoulder = Vec3::new(-0.3, 2.0, 0.2);
    let mut events = Vec::new();
    for _ in 0..((SHEATHE_HOLD / FRAME) as usize + 3) {
        t += FRAME;
        events.extend(g.update(None, Some(&still(shoulder, Grip::Open)), t));
    }
    assert_eq!(events, vec![Event::Sheathe]);
    assert!(!g.drawn);
    t += FRAME;
    let drew = g.update(None, Some(&still(shoulder, Grip::Closed)), t);
    assert_eq!(drew, vec![Event::Draw]);
    assert!(g.drawn);
    // A fist over the shoulder while drawn is just a raised sword.
    t += FRAME;
    assert!(
        g.update(None, Some(&still(shoulder, Grip::Closed)), t)
            .is_empty()
    );
}

#[test]
fn a_raised_shield_pushed_forward_bashes_once_per_cooldown() {
    let mut g = Gestures::default();
    let mut t = 0.0;
    left_only(
        &mut g,
        still(Vec3::new(0.3, 1.4, 0.3), Grip::Loose),
        0.1,
        &mut t,
    );
    let push = HandPose {
        vel: Vec3::new(-0.2, 0.0, 1.4),
        ..still(Vec3::new(0.25, 1.4, 0.5), Grip::Loose)
    };
    let events = left_only(&mut g, push, 0.2, &mut t);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Bash(_)))
            .count(),
        1
    );
    // A lowered shield pushed forward is no bash.
    let mut g = Gestures::default();
    let low = HandPose {
        vel: Vec3::new(0.0, 0.0, 1.4),
        ..still(Vec3::new(0.25, 1.0, 0.5), Grip::Loose)
    };
    assert!(left_only(&mut g, low, 0.2, &mut t).is_empty());
}

// --- Combat outcomes ---

fn cut_through_body() -> Swing {
    Swing {
        path: (0..10)
            .map(|i| Vec3::new(-0.8 + i as f32 * 0.18, 1.5, 0.9))
            .collect(),
        peak: 3.0,
        arc: 1.6,
        seconds: 0.3,
    }
}

fn in_reach(seed: u64) -> Fight {
    let mut fight = Fight::new(seed);
    fight.warden.dist = WARDEN_REACH;
    fight.warden.phase = Phase::Recover { until: 10.0 };
    fight
}

#[test]
fn a_swing_out_of_reach_or_beside_him_does_nothing() {
    let mut fight = Fight::new(1);
    assert!(fight.warden.dist > PLAYER_REACH);
    assert_eq!(fight.swing(&cut_through_body()), SwingResult::OutOfReach);

    let mut fight = in_reach(1);
    let wide = Swing {
        path: (0..10)
            .map(|i| Vec3::new(-1.2 - i as f32 * 0.05, 1.5, 0.9))
            .collect(),
        ..cut_through_body()
    };
    assert_eq!(fight.swing(&wide), SwingResult::NoContact);
    assert_eq!(fight.warden.hp, fight.warden.max_hp);
}

#[test]
fn swings_through_him_hit_or_miss_by_the_dice_and_faster_ones_hit_harder() {
    let (mut slow_total, mut fast_total) = (0.0, 0.0);
    let (mut slow_hits, mut fast_hits) = (0, 0);
    for seed in 0..200 {
        let mut fight = in_reach(seed);
        let slow = Swing {
            peak: 1.5,
            ..cut_through_body()
        };
        if let SwingResult::Hit { damage, .. } = fight.swing(&slow) {
            slow_hits += 1;
            slow_total += damage;
            assert!(fight.warden.hp < fight.warden.max_hp);
        }
        let mut fight = in_reach(seed);
        let fast = Swing {
            peak: 5.0,
            ..cut_through_body()
        };
        if let SwingResult::Hit { damage, .. } = fight.swing(&fast) {
            fast_hits += 1;
            fast_total += damage;
        }
    }
    assert!(slow_hits > 40 && slow_hits < 200, "{slow_hits}");
    assert!(fast_hits > slow_hits, "{fast_hits} vs {slow_hits}");
    assert!(
        fast_total / fast_hits as f32 > 1.5 * slow_total / slow_hits as f32,
        "faster swings hit harder"
    );
}

#[test]
fn the_same_seed_replays_the_same_fight() {
    let run = |seed| {
        let mut fight = in_reach(seed);
        (0..5)
            .map(|_| fight.swing(&cut_through_body()))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(42), run(42));
}

#[test]
fn a_heavy_hit_during_his_windup_interrupts_it() {
    for seed in 0..100 {
        let mut fight = in_reach(seed);
        fight.warden.phase = Phase::Windup {
            blow: Blow::Overhead,
            start: fight.now,
        };
        let swing = Swing {
            peak: 5.5,
            ..cut_through_body()
        };
        if let SwingResult::Hit { damage, .. } = fight.swing(&swing)
            && damage >= crate::rules::INTERRUPT_DAMAGE
        {
            assert!(matches!(fight.warden.phase, Phase::Stagger { .. }));
            return;
        }
    }
    panic!("no seed interrupted the windup");
}

#[test]
fn a_bash_during_a_windup_staggers_him_and_shoves_him_back() {
    let mut fight = in_reach(3);
    fight.warden.phase = Phase::Windup {
        blow: Blow::Left,
        start: 0.0,
    };
    assert!(fight.bash());
    assert!(matches!(fight.warden.phase, Phase::Stagger { .. }));
    assert!(fight.warden.dist > WARDEN_REACH);
    // Out of reach, a bash does nothing.
    let mut far = Fight::new(3);
    assert!(!far.bash());
}

#[test]
fn a_potion_heals_but_not_past_full() {
    let mut fight = Fight::new(5);
    fight.player.hp = 20.0;
    let gained = fight.drink();
    assert!((6.0..=12.0).contains(&gained), "{gained}");
    fight.player.hp = fight.player.max_hp - 1.0;
    assert_eq!(fight.drink(), 1.0);
}

#[test]
fn the_warden_walks_in_winds_up_and_strikes_on_his_own() {
    let mut fight = Fight::new(11);
    let mut wound = None;
    for _ in 0..(20.0 / 0.02) as usize {
        fight.tick(0.02);
        if let Phase::Windup { blow, .. } = fight.warden.phase {
            wound.get_or_insert(blow);
        }
        if fight.last_blow.is_some() {
            break;
        }
    }
    assert!(wound.is_some(), "he never wound up");
    assert!(fight.last_blow.is_some(), "he never struck");
    assert!((fight.warden.dist - WARDEN_REACH).abs() < 1e-3);
}

#[test]
fn killing_him_wins_and_sheathing_starts_the_next_fight() {
    let mut duel = HandDuel::new();
    duel.fight_mut().warden.hp = 1.0;
    duel.fight_mut().warden.dist = WARDEN_REACH;
    for seed in 0..50 {
        if duel.fight().outcome == Outcome::Won {
            break;
        }
        *duel.fight_mut() = {
            let mut f = in_reach(seed);
            f.warden.hp = 1.0;
            f
        };
        let _ = duel.fight_mut().swing(&cut_through_body());
    }
    assert_eq!(duel.fight().outcome, Outcome::Won);
    let shoulder = Vec3::new(-0.3, 2.0, 0.2);
    let mut t = 0.0;
    for _ in 0..((SHEATHE_HOLD / FRAME) as usize + 3) {
        t += FRAME;
        duel.poses(None, Some(still(shoulder, Grip::Open)), t);
    }
    assert_eq!(duel.round(), 1);
    assert_eq!(duel.fight().outcome, Outcome::Fighting);
    assert!(!duel.gestures().drawn);
}

#[test]
fn the_zone_blocks_a_telegraphed_blow_from_the_raised_left_hand() {
    let mut duel = HandDuel::new();
    duel.fight_mut().warden.dist = WARDEN_REACH;
    duel.fight_mut().warden.phase = Phase::Windup {
        blow: Blow::Overhead,
        start: 0.0,
    };
    let raised = still(Vec3::new(0.05, 1.95, 0.4), Grip::Loose);
    let mut t = 0.0;
    for _ in 0..3 {
        t += FRAME;
        duel.poses(Some(raised), None, t);
    }
    for _ in 0..((WINDUP + 0.2) / 0.02) as usize {
        duel.tick(0.02);
    }
    assert_eq!(duel.fight().last_blow, Some(BlowResult::Blocked));
}

// --- From the camera's points to the body ---

fn frame(hands: &[(Pose, f32, f32)]) -> RawFrame {
    let aspect = 4.0 / 3.0;
    // An identity mapping: page fractions are mirrored frame fractions.
    let map = Mapping {
        side: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    RawFrame {
        aspect,
        hands: hands
            .iter()
            .map(|(pose, x, y)| RawHand {
                score: 1.0,
                landmarks: synth::hand(*pose, 1.0 - x, *y, aspect, &map),
            })
            .collect(),
    }
}

#[test]
fn the_hand_on_the_pictures_left_is_the_right_hand_and_height_reads_up() {
    let mut body = Body::default();
    let mut t = 0.0;
    for _ in 0..6 {
        t += FRAME;
        // Frame x: 0.25 is the person's right, 0.75 the left; the right hand
        // is higher.
        body.frame(
            &frame(&[(Pose::Fist, 0.25, 0.3), (Pose::Open, 0.75, 0.6)]),
            t,
        );
    }
    let right = body.hand(Side::Right).expect("right hand");
    let left = body.hand(Side::Left).expect("left hand");
    assert!(right.pos.x < -0.2, "{right:?}");
    assert!(left.pos.x > 0.2, "{left:?}");
    assert!(right.pos.y > left.pos.y);
    assert_eq!(right.grip, Grip::Closed);
    assert_eq!(left.grip, Grip::Open);

    // Hands that cross keep who they are.
    for i in 0..30 {
        t += FRAME;
        let k = i as f32 / 29.0 * 0.2;
        body.frame(
            &frame(&[(Pose::Fist, 0.25 + k, 0.3), (Pose::Open, 0.75 - k, 0.6)]),
            t,
        );
    }
    assert_eq!(body.hand(Side::Right).unwrap().grip, Grip::Closed);

    // Gone for a while, gone.
    body.frame(&frame(&[]), t + 1.0);
    assert!(body.hand(Side::Right).is_none() && body.hand(Side::Left).is_none());
}

#[test]
fn a_hand_that_looks_bigger_reaches_further_and_moving_reads_as_velocity() {
    assert!(reach(0.3) > reach(0.2));
    assert!(reach(0.2) > reach(0.12));
    assert!(reach(5.0) <= crate::body::REACH_MAX);

    let mut body = Body::default();
    let mut t = 0.0;
    for i in 0..10 {
        t += FRAME;
        let x = 0.2 + i as f32 * 0.03;
        body.frame(&frame(&[(Pose::Fist, x, 0.4)]), t);
    }
    let hand = body.hand(Side::Right).expect("a hand");
    // Moving toward the picture's right is moving to the person's left.
    assert!(hand.vel.x > 0.5, "{hand:?}");
}

#[test]
fn camera_frames_drive_the_zone_through_the_same_gestures() {
    let mut duel = HandDuel::new();
    let mut t = 0.0;
    for _ in 0..6 {
        t += FRAME;
        let data = frame(&[(Pose::Open, 0.7, 0.3)]).encode();
        duel.hand_frame(&data, t);
    }
    assert!(duel.tracking());
    assert!(duel.hand(Side::Left).is_some());
    assert!(
        duel.gestures().shield_up,
        "a hand high on the left raises it"
    );
    assert!(duel.fight().guard.is_some());
}

#[test]
fn the_screens_words_are_plain() {
    let mut lines: Vec<String> = hud::GESTURES.iter().map(|s| (*s).to_owned()).collect();
    lines.push(hud::START.to_owned());
    lines.extend(Blow::ALL.iter().map(|b| b.word().to_owned()));
    lines.push(HandDuel::new().caption());
    for line in lines {
        assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
    }
}
