//! Everglade's rowboats in play: boarding from a jetty and from the water,
//! rowing across Lantern Pond and back, leaving onto the jetty, the wake,
//! and the pads.

use super::*;
use crate::controller::{InputState, PlayerController};
use crate::zones::everglade::layout;
use crate::zones::everglade::tests::pack;
use std::sync::OnceLock;
use verse_world::rowboat::{Seat, State};

const DT: f32 = 1.0 / 60.0;

fn solids() -> &'static Solids {
    static SOLIDS: OnceLock<Solids> = OnceLock::new();
    SOLIDS.get_or_init(|| {
        super::super::solids::build(pack(), &layout::placements()).expect("the solids build")
    })
}

fn afloat() -> Afloat {
    Afloat::new(pack(), &layout::floats()).expect("the boats load")
}

/// The point on the jetty beside boat `k` nearest its hull, standing on
/// the deck.
fn on_the_jetty(a: &Afloat, k: usize) -> Vec3 {
    let (keel, _) = a.fleet.frame(k);
    let keel = keel.as_vec3();
    let dock = layout::placements()
        .into_iter()
        .filter(|p| p.model == "generated/dock")
        .min_by(|p, q| {
            let d = |p: &Placement| Vec2::from(p.at).distance(Vec2::new(keel.x, keel.z));
            d(p).total_cmp(&d(q))
        })
        .expect("a jetty");
    let top = ew::surface(keel.x, keel.z).unwrap();
    // Along the jetty's length, the deck point nearest the boat.
    let along = crate::controller::forward(dock.yaw);
    (-30..=30)
        .map(|i| {
            let p = Vec2::from(dock.at) + Vec2::new(along.x, along.z) * (i as f32 * 0.1);
            let floor = solids().floor(p.x, p.y, top + 1.6);
            Vec3::new(p.x, floor, p.y)
        })
        .filter(|p| p.y > top + 0.2)
        .min_by(|p, q| a.fleet.reach(k, *p).total_cmp(&a.fleet.reach(k, *q)))
        .expect("a deck by the boat")
}

/// Steers boat `k` toward `target` for at most `seconds`: the keys a
/// rower would hold. Returns whether it got within `close` m.
fn row_to(
    a: &mut Afloat,
    player: &mut PlayerController,
    k: usize,
    target: Vec2,
    close: f32,
    seconds: f32,
) -> bool {
    for _ in 0..(seconds / DT) as usize {
        let (keel, rot) = a.fleet.frame(k);
        let at = Vec2::new(keel.x as f32, keel.z as f32);
        if at.distance(target) < close {
            return true;
        }
        let ahead = (rot * glam::DVec3::Z).as_vec3();
        let heading = Vec2::new(ahead.x, ahead.z);
        let want = (target - at).normalize_or_zero();
        let turn = heading.perp_dot(want);
        let input = InputState {
            forward: heading.dot(want) > 0.6,
            // With +z ahead, +x lies to the left.
            left: turn < -0.15,
            right: turn > 0.15,
            ..InputState::default()
        };
        assert!(a.ride(player, &input));
        a.tick(DT);
    }
    false
}

#[test]
fn a_player_boards_from_the_jetty_rows_across_lantern_pond_and_back_and_leaves_onto_the_jetty() {
    let mut a = afloat();
    assert_eq!(a.fleet.boats.len(), 3);
    assert!(a.pads().len() >= 8);
    for _ in 0..120 {
        a.tick(DT);
    }
    // Afloat at its draft by the jetty.
    assert!(
        (a.fleet.draft(0) - 0.12).abs() < 0.03,
        "{}",
        a.fleet.draft(0)
    );
    let jetty = on_the_jetty(&a, 0);
    let mut player = PlayerController::new(jetty, 0.0);
    player.set_surface_height(jetty.y);
    let line = a
        .interact(&mut player, false, solids())
        .expect("a boat in reach");
    assert_eq!(line, "You take the oars");
    assert_eq!(a.seat(), Some((0, Seat::Rower, true)));
    let start = {
        let (keel, _) = a.fleet.frame(0);
        Vec2::new(keel.x as f32, keel.z as f32)
    };
    // Across: to the far side of the pond's middle.
    let ([cx, cz], _) = ew::PONDS[0];
    let middle = Vec2::new(cx, cz);
    let far = middle + (middle - start).normalize() * 3.0;
    assert!(row_to(&mut a, &mut player, 0, far, 1.0, 40.0), "across");
    let across = {
        let (keel, _) = a.fleet.frame(0);
        Vec2::new(keel.x as f32, keel.z as f32).distance(start)
    };
    assert!(across > 5.0, "rowed {across} m");
    // A rowed boat leaves a wake: a moving source for the ripple field.
    let wake = a
        .sources()
        .into_iter()
        .map(|s| Vec2::from(s.velocity).length())
        .fold(0.0, f32::max);
    assert!(wake > verse_pbr::water::ripple::WAKE_SPEED, "{wake}");
    // And back to the jetty.
    let jetty_at = Vec2::new(jetty.x, jetty.z);
    assert!(
        row_to(&mut a, &mut player, 0, start, 1.2, 60.0),
        "back by the jetty"
    );
    // Coast to rest, then step out onto the deck.
    let idle = InputState::default();
    for _ in 0..120 {
        a.ride(&mut player, &idle);
        a.tick(DT);
    }
    let line = a.interact(&mut player, false, solids()).unwrap();
    assert_eq!(line, "You step out of the rowboat");
    assert_eq!(a.seat(), None);
    let top = ew::surface(player.pos.x, player.pos.z);
    assert!(
        top.is_none_or(|t| player.pos.y > t),
        "on the deck or the bank, not in the water: {}",
        player.pos
    );
    assert!(Vec2::new(player.pos.x, player.pos.z).distance(jetty_at) < 4.0);
}

#[test]
fn a_swimmer_climbs_in_and_a_second_player_takes_the_passenger_s_seat() {
    let mut a = afloat();
    for _ in 0..60 {
        a.tick(DT);
    }
    let (keel, rot) = a.fleet.frame(1);
    let side = (rot * glam::DVec3::new(1.2, 0.0, 0.0)).as_vec3();
    let p = keel.as_vec3() + side;
    let top = ew::surface(p.x, p.z).unwrap();
    let mut swimmer = PlayerController::new(Vec3::new(p.x, top - 1.3, p.z), 0.0);
    let line = a.interact(&mut swimmer, true, solids()).unwrap();
    assert_eq!(line, "You climb into the rowboat");
    assert_eq!(a.seat(), Some((1, Seat::Rower, false)));
    for _ in 0..(1.6 / DT) as usize {
        assert!(a.ride(&mut swimmer, &InputState::default()));
        a.tick(DT);
    }
    assert_eq!(a.seat(), Some((1, Seat::Rower, true)));
    // A second player boards as the passenger.
    let at = keel.as_vec3() - side;
    assert_eq!(a.fleet.board(1, "friend", at, true), Ok(Seat::Passenger));
    // Leaving with no deck in reach drops the player in the water.
    let line = a.interact(&mut swimmer, false, solids()).unwrap();
    assert!(
        line == "You slip over the side into the water" || line == "You step out of the rowboat",
        "{line}"
    );
}

#[test]
fn a_capsize_dumps_the_player_and_lily_pads_bob_as_a_boat_passes() {
    let mut a = afloat();
    for _ in 0..60 {
        a.tick(DT);
    }
    let pad = a.pads()[0];
    let rest = a.item_pose(Item::Pad { pad: 0 }).w_axis.y;
    a.set_movers(vec![(pad.at + Vec2::X * 0.8, 1.5)]);
    let mut heights = Vec::new();
    for _ in 0..60 {
        a.tick(DT);
        heights.push(a.item_pose(Item::Pad { pad: 0 }).w_axis.y);
    }
    let swing = heights.iter().fold(0.0_f32, |m, h| m.max((h - rest).abs()));
    assert!(swing > 0.01, "the pad bobs {swing} m");
    // Capsized with the player aboard: it is dumped in the water.
    let jetty = on_the_jetty(&a, 0);
    let mut player = PlayerController::new(jetty, 0.0);
    a.interact(&mut player, false, solids()).unwrap();
    a.fleet.roll(0, 12.0);
    for _ in 0..180 {
        a.ride(&mut player, &InputState::default());
        a.tick(DT);
    }
    assert_eq!(a.fleet.boats[0].state, State::Capsized);
    assert_eq!(a.seat(), None);
    let k = a.take_dump().expect("the boat dumped the player");
    a.drop_in(k, &mut player);
    let top = ew::surface(player.pos.x, player.pos.z).expect("in the water");
    assert!(player.pos.y < top);
    assert!(a.hulls().len() == 2, "a capsized hull masks no water");
}
