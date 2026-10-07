use super::*;
use physics::water::{WaterBody, WaterId, WaterSet};

/// A square pond 40 m across with its level at 0 and a flat bed 3 m down.
fn pond() -> &'static WaterSet {
    use std::sync::OnceLock;
    static POND: OnceLock<WaterSet> = OnceLock::new();
    POND.get_or_init(|| {
        WaterSet::new(
            vec![WaterBody::pond(
                WaterId(0),
                vec![
                    DVec2::new(-20.0, -20.0),
                    DVec2::new(20.0, -20.0),
                    DVec2::new(20.0, 20.0),
                    DVec2::new(-20.0, 20.0),
                ],
                0.0,
            )],
            4.0,
        )
    })
}

fn fleet() -> Fleet {
    let bed = [(DVec3::new(0.0, -3.5, 0.0), DVec3::new(30.0, 0.5, 30.0))];
    Fleet::new(
        pond(),
        &bed,
        &[(DVec2::ZERO, 0.0), (DVec2::new(8.0, 0.0), 0.0)],
    )
}

fn settle(f: &mut Fleet, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        f.tick(1.0 / 60.0);
    }
}

/// Empty, a boat draws about 0.12 m; with two aboard about 0.2 m, and it
/// floats level.
#[test]
fn a_boat_draws_about_a_hand_empty_and_more_with_two_aboard() {
    let mut f = fleet();
    settle(&mut f, 8.0);
    let empty = f.draft(0);
    assert!((empty - 0.12).abs() < 0.02, "{empty}");
    assert!(f.tilt(0) < 0.02);
    let side = Vec3::new(1.2, 0.0, 0.0);
    assert_eq!(f.board(0, "a", side, false), Ok(Seat::Rower));
    assert_eq!(f.board(0, "b", side, false), Ok(Seat::Passenger));
    assert_eq!(f.board(0, "c", side, false), Err(Refusal::Full));
    settle(&mut f, 8.0);
    let laden = f.draft(0);
    assert!((laden - 0.2).abs() < 0.03, "{laden}");
    assert!(f.tilt(0) < 0.05, "{}", f.tilt(0));
}

/// Boarding needs the boat within reach; from the water it takes a climb.
#[test]
fn boarding_from_land_is_at_once_and_from_the_water_takes_a_climb() {
    let mut f = fleet();
    settle(&mut f, 1.0);
    assert_eq!(
        f.board(0, "far", Vec3::new(5.0, 0.0, 0.0), false),
        Err(Refusal::TooFar)
    );
    assert_eq!(
        f.board(0, "swimmer", Vec3::new(1.5, -0.5, 0.0), true),
        Ok(Seat::Rower)
    );
    assert_eq!(f.seat_of("swimmer"), Some((0, Seat::Rower, false)));
    // No strokes while climbing.
    f.row(
        "swimmer",
        Intent {
            ahead: 1.0,
            turn: 0.0,
        },
    );
    settle(&mut f, 1.0);
    assert_eq!(f.seat_of("swimmer"), Some((0, Seat::Rower, false)));
    settle(&mut f, 0.6);
    assert_eq!(f.seat_of("swimmer"), Some((0, Seat::Rower, true)));
    assert!(
        f.take_events()
            .iter()
            .any(|e| matches!(e, Event::Boarded { who, .. } if who == "swimmer"))
    );
    assert_eq!(f.leave("swimmer"), Some((0, Seat::Rower)));
    assert_eq!(f.seat_of("swimmer"), None);
}

/// Rowing ahead tops out near 1.5 m/s; one oar turns the boat; rowing
/// astern brings it back.
#[test]
fn strokes_drive_the_boat_near_a_mile_and_a_half_a_second_and_one_oar_turns_it() {
    let mut f = fleet();
    settle(&mut f, 2.0);
    f.board(0, "a", Vec3::new(1.0, 0.0, 0.0), false).unwrap();
    f.row(
        "a",
        Intent {
            ahead: 1.0,
            turn: 0.0,
        },
    );
    let start = f.frame(0).0;
    let mut top: f64 = 0.0;
    for _ in 0..(14.0 * 60.0) as usize {
        f.tick(1.0 / 60.0);
        top = top.max(f.speed(0));
    }
    assert!((1.2..1.9).contains(&top), "top speed {top}");
    let ahead = f.frame(0).0 - start;
    assert!(ahead.z > 10.0 && ahead.x.abs() < 1.5, "{ahead}");
    assert!(f.boats[0].strokes >= 10);
    assert!(
        f.take_events()
            .iter()
            .any(|e| matches!(e, Event::Stroke { .. }))
    );
    // One oar: the heading swings.
    let yaw = f.yaw(0);
    f.row(
        "a",
        Intent {
            ahead: 0.0,
            turn: 1.0,
        },
    );
    settle(&mut f, 6.0);
    assert!((f.yaw(0) - yaw).abs() > 0.4, "{} → {}", yaw, f.yaw(0));
    // Astern, the other way.
    f.row(
        "a",
        Intent {
            ahead: -1.0,
            turn: 0.0,
        },
    );
    let before = f.frame(0).0;
    let heading = f.frame(0).1 * DVec3::Z;
    settle(&mut f, 8.0);
    assert!((f.frame(0).0 - before).dot(heading) < -2.0);
}

/// Rolled past the limit, a boat capsizes and its occupants fall in; a
/// swimmer beside it rights it.
#[test]
fn rolling_past_the_limit_capsizes_and_a_swimmer_rights_it() {
    let mut f = fleet();
    settle(&mut f, 2.0);
    f.board(0, "a", Vec3::new(1.0, 0.0, 0.0), false).unwrap();
    f.board(0, "b", Vec3::new(1.0, 0.0, 0.0), false).unwrap();
    // A small shove only rocks it.
    f.roll(0, 0.8);
    settle(&mut f, 4.0);
    assert_eq!(f.boats[0].state, State::Upright);
    f.take_events();
    f.roll(0, 12.0);
    settle(&mut f, 3.0);
    assert_eq!(f.boats[0].state, State::Capsized);
    let events = f.take_events();
    let dumped = events
        .iter()
        .find_map(|e| match e {
            Event::Capsized { dumped, .. } => Some(dumped.clone()),
            _ => None,
        })
        .expect("a capsize event");
    assert_eq!(dumped, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(f.seat_of("a"), None);
    // It floats keel up.
    settle(&mut f, 6.0);
    assert!(f.tilt(0) > 2.5, "{}", f.tilt(0));
    assert_eq!(
        f.board(0, "a", Vec3::new(1.0, 0.0, 0.0), true),
        Err(Refusal::NotUpright)
    );
    assert_eq!(f.right(0, Vec3::new(9.0, 0.0, 9.0)), Err(Refusal::TooFar));
    let (keel, _) = f.frame(0);
    f.right(0, keel.as_vec3() + Vec3::new(1.2, 0.0, 0.0))
        .unwrap();
    settle(&mut f, 5.0);
    assert_eq!(f.boats[0].state, State::Upright);
    assert!(f.tilt(0) < 0.05, "{}", f.tilt(0));
    assert!((f.draft(0) - 0.12).abs() < 0.03);
}

/// Breaking a boat drops its occupants in and floats its planks.
#[test]
fn breaking_a_boat_floats_its_planks() {
    let mut f = fleet();
    settle(&mut f, 2.0);
    f.board(0, "a", Vec3::new(1.0, 0.0, 0.0), false).unwrap();
    let (keel, _) = f.frame(0);
    assert!(!f.strike(keel.as_vec3(), 1.0, 5).contains(&0));
    assert_eq!(f.strike(keel.as_vec3(), 1.0, HIT_POINTS), vec![0]);
    assert_eq!(f.boats[0].state, State::Broken);
    assert_eq!(f.seat_of("a"), None);
    assert!(
        f.take_events()
            .iter()
            .any(|e| matches!(e, Event::Broken { dumped, .. } if dumped == &vec!["a".to_owned()]))
    );
    settle(&mut f, 12.0);
    assert_eq!(f.boats[0].planks.len(), 7);
    for &p in &f.boats[0].planks {
        let (pos, _) = f.plank_pose(p);
        assert!(pos.y > -0.15 && pos.y < 0.1, "a plank at {pos}");
    }
    // The other boat is untouched.
    assert_eq!(f.boats[1].state, State::Upright);
    f.reset();
    settle(&mut f, 3.0);
    assert_eq!(f.boats[0].state, State::Upright);
    assert!(f.boats[0].planks.is_empty());
}

/// The host's reports carry its boat to another client within one report:
/// a second client that follows them holds the same pose, and a passenger
/// there sees the capsize.
#[test]
fn a_second_client_follows_the_host_s_reports() {
    let mut host = fleet();
    let mut other = fleet();
    settle(&mut host, 1.0);
    settle(&mut other, 1.0);
    host.board(0, "host", Vec3::new(1.0, 0.0, 0.0), false)
        .unwrap();
    other
        .board(0, "guest", Vec3::new(1.0, 0.0, 0.0), false)
        .unwrap();
    // The guest took the rower's seat locally, but the host's report wins
    // once the host rows: the guest sees itself as the boat's passenger.
    host.row(
        "host",
        Intent {
            ahead: 1.0,
            turn: 0.3,
        },
    );
    for _ in 0..600 {
        host.tick(1.0 / 60.0);
        other.tick(1.0 / 60.0);
        for report in host.reports("host") {
            assert!(other.receive("host", &report));
        }
        let (a, ra) = host.frame(0);
        let (b, rb) = other.frame(0);
        // Within one report interval of motion.
        assert!(a.distance(b) < 0.05, "{a} vs {b}");
        assert!(ra.angle_between(rb) < 0.05);
    }
    assert!(other.reports("guest").is_empty());
    // An older stamp is refused.
    let mut stale = host.report(0);
    stale.stamp = [0, 0];
    assert!(!other.receive("host", &stale));
    host.roll(0, 12.0);
    for _ in 0..240 {
        host.tick(1.0 / 60.0);
        other.tick(1.0 / 60.0);
        for report in host.reports("host") {
            other.receive("host", &report);
        }
    }
    assert_eq!(other.boats[0].state, State::Capsized);
}
