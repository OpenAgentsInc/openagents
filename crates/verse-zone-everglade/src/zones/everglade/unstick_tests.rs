//! A walker sweeps the owner's house and the Civic Hall on their real
//! collision: their stairs, podiums, porticos, doorways, and rooms have no
//! pit below the visual floor, and every square a walker reaches leads
//! back to the foot of the stair. A trapped player is moved out.

use super::{self as unstick, CELL, Watch, drop_to, reachable, step_to};
use crate::zones::everglade::height;
use crate::zones::everglade::layout::{self, generated::Instance};
use crate::zones::everglade::solids::STEP;
use crate::zones::everglade::solids::{self, Solids};
use crate::zones::everglade::tests::pack;
use glam::Vec3;
use std::collections::{HashSet, VecDeque};
use std::sync::OnceLock;

fn solids() -> &'static Solids {
    static SOLIDS: OnceLock<Solids> = OnceLock::new();
    SOLIDS.get_or_init(|| solids::build(pack(), &layout::placements()).expect("the solids build"))
}

fn instance(name: &str) -> Instance {
    layout::generated()
        .into_iter()
        .find(|i| i.model.name == name)
        .unwrap_or_else(|| panic!("{name} is placed"))
}

/// A world point in `instance`'s frame, x and z.
fn to_local(instance: &Instance, x: f32, z: f32) -> [f32; 2] {
    let back = glam::Quat::from_rotation_y(-instance.yaw)
        * Vec3::new(x - instance.at[0], 0.0, z - instance.at[1]);
    [back.x / instance.scale, back.z / instance.scale]
}

/// A point in `instance`'s frame, on the floor its feet reach from
/// `feet` over the base.
fn standing(instance: &Instance, local: [f32; 2], feet: f32) -> Vec3 {
    let [x, z] = instance.world(local);
    let base = height(instance.at[0], instance.at[1]);
    Vec3::new(x, solids().floor(x, z, base + feet), z)
}

/// Walks `name` from the foot of its stair on a grid: every square a
/// walker reaches inside `region` (its frame's min x, max x, min z, max z)
/// leads back to the foot, and every square reached over `floors` (each a
/// rectangle in its frame and the floor's height over the base) stands on
/// that floor, not in a gap below it. Returns the offenders, in its frame.
fn sweep(name: &str, region: [f32; 4], floors: &[([f32; 4], f32)]) -> Vec<String> {
    let house = instance(name);
    let [fx, fz] = house.world(house.model.front);
    let start = Vec3::new(fx, solids().floor(fx, fz, height(fx, fz)), fz);
    let base = height(house.at[0], house.at[1]);
    let within = |x: f32, z: f32| {
        let [u, v] = to_local(&house, x, z);
        u >= region[0] && u <= region[1] && v >= region[2] && v <= region[3]
    };
    let reached = reachable(solids(), start, within);
    assert!(
        reached.len() > 1000,
        "{name}: only {} squares",
        reached.len()
    );
    let at = |k: [i32; 2]| (start.x + k[0] as f32 * CELL, start.z + k[1] as f32 * CELL);
    // The squares that lead back to the foot, walking backward from it.
    let mut out_of = HashSet::from([[0, 0]]);
    let mut queue = VecDeque::from([[0, 0]]);
    while let Some(k) = queue.pop_front() {
        let (x, z) = at(k);
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let p = [k[0] + d[0], k[1] + d[1]];
            let Some(&feet) = reached.get(&p) else {
                continue;
            };
            if out_of.contains(&p) {
                continue;
            }
            if step_to(solids(), x, z, feet).is_some_and(|f| (f - reached[&k]).abs() < 0.05) {
                out_of.insert(p);
                queue.push_back(p);
            }
        }
    }
    let mut out = Vec::new();
    let mut fell = HashSet::new();
    for (k, &feet) in &reached {
        let (x, z) = at(*k);
        let [u, v] = to_local(&house, x, z);
        if !out_of.contains(k) {
            out.push(format!("trapped at {u:.2}, {v:.2}, {:.2} up", feet - base));
        }
        // A fall into a gap it can't walk or jump out of: a slot below a
        // floor.
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let q = [k[0] + d[0], k[1] + d[1]];
            let (qx, qz) = at(q);
            if !within(qx, qz) || !fell.insert(q) {
                continue;
            }
            if let Some(floor) = drop_to(solids(), qx, qz, feet)
                && floor < feet - STEP
                && unstick::blocked(solids(), qx, qz, floor)
                && unstick::trapped(solids(), Vec3::new(qx, floor, qz))
            {
                let [u, v] = to_local(&house, qx, qz);
                out.push(format!(
                    "falls into a gap at {u:.2}, {v:.2}, {:.2} up",
                    floor - base
                ));
            }
        }
        for &([x0, x1, z0, z1], floor) in floors {
            if u >= x0 && u <= x1 && v >= z0 && v <= z1 && feet < base + floor - 0.1 {
                out.push(format!(
                    "below the floor at {u:.2}, {v:.2}, {:.2} up",
                    feet - base
                ));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_house_and_the_civic_hall_have_no_pits_and_every_floor_leads_back_out() {
    // The owner's house: the portico, the doorway, and the great room on
    // the podium, 1.6 m up.
    let house = sweep(
        "generated/greco_house",
        [-11.5, 11.5, -26.5, 2.0],
        &[([-10.4, 10.4, -25.2, -7.9], 1.6)],
    );
    // The Civic Hall: the portico and the chamber on its podium.
    let hall = sweep(
        "generated/civic_hall",
        [-19.0, 19.0, -31.0, 2.0],
        &[([-8.6, 8.6, -8.4, -4.9], 1.92)],
    );
    assert!(
        house.is_empty() && hall.is_empty(),
        "house: {house:#?}\nhall: {hall:#?}"
    );
}

#[test]
fn the_doorway_threshold_is_floor_and_the_portico_walks_into_the_great_room() {
    let house = instance("generated/greco_house");
    let base = height(house.at[0], house.at[1]);
    for x in [-0.75, -0.25, 0.25, 0.75] {
        for z in [-11.6, -11.75, -11.9, -12.0] {
            let p = standing(&house, [x, z], 1.6);
            assert!(p.y > base + 1.55, "{x}, {z}: {:.2} up", p.y - base);
            assert!(!unstick::trapped(solids(), p), "{x}, {z}");
        }
    }
}

#[test]
fn a_player_in_a_pit_is_moved_to_the_nearest_free_floor() {
    // Under the portico's floor, inside the podium: walled in on every
    // side by the floor's edges a podium's height up.
    let house = instance("generated/greco_house");
    let base = height(house.at[0], house.at[1]);
    let [x, z] = house.world([0.0, -10.0]);
    let pit = Vec3::new(x, base, z);
    assert!(unstick::trapped(solids(), pit));
    let free = unstick::rescue(solids(), pit).expect("a free floor nearby");
    assert!(free.y > base + 1.5, "{:.2} up", free.y - base);
    assert!(free.distance(pit) < 3.0, "{free:?}");
    // The watch moves a player held in place there after a second's press,
    // and never one walking free on the portico.
    let mut watch = Watch::default();
    let mut moved = None;
    for _ in 0..80 {
        moved = moved.or(watch.after_step(solids(), pit, true, true, 1.0 / 60.0));
    }
    assert_eq!(moved, Some(free));
    let portico = standing(&house, [0.0, -10.0], 1.6);
    let mut watch = Watch::default();
    for _ in 0..80 {
        assert_eq!(
            watch.after_step(solids(), portico, true, true, 1.0 / 60.0),
            None
        );
    }
}
