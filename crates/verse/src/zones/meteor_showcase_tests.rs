//! The Meteor Showcase through the world runtime: its flag and no arch,
//! the caster's eight arcs breaking both houses, the debris at rest, the
//! rebuild, and the walk up the path ([`super::meteor_showcase`]).

use super::meteor_showcase::{self as showcase, DELAY, HOUR, RETURN_PORTAL, aim, houses};
use super::{Everglade, ZoneId, everglade, everglade_pack};
use crate::{controller::InputState, runtime::WorldRuntime, zones::Intent};
use everglade::demolition::meteor::CAST;
use everglade::demolition::{site::Status, town::Town};
use glam::Vec3;
use std::f32::consts::{PI, TAU};
use std::path::Path;

const DT: f32 = 1.0 / 60.0;

fn installed() -> WorldRuntime {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&path).unwrap();
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_showcase(&pack);
    assert_eq!(runtime.zone, ZoneId::MeteorShowcase);
    runtime
}

fn town(runtime: &WorldRuntime) -> &Town {
    runtime
        .zone_state
        .everglade
        .as_deref()
        .and_then(Everglade::town)
        .expect("the showcase has its houses' demolition")
}

fn run(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
    }
}

/// The town building each house is.
fn buildings(town: &Town) -> [usize; 2] {
    houses().map(|house| {
        let distance = |i: usize| {
            let ([cx, cz], _) = town.buildings()[i].rect;
            (house.center[0] - cx).hypot(house.center[1] - cz)
        };
        let index = (0..town.buildings().len())
            .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
            .expect("the lot has buildings");
        assert!(
            distance(index) < 3.0,
            "{} is one building: the nearest stands {} m off",
            house.name,
            distance(index)
        );
        index
    })
}

/// How many of `building`'s pieces stand, and how many it has.
fn standing(town: &Town, building: usize) -> (usize, usize) {
    let site = town.site();
    let mine: Vec<_> = site
        .specs()
        .iter()
        .zip(site.pieces())
        .filter(|(spec, _)| spec.building == building)
        .collect();
    let up = mine
        .iter()
        .filter(|(_, piece)| piece.status == Status::Standing)
        .count();
    (up, mine.len())
}

#[test]
fn the_showcase_is_reached_by_its_flag_and_no_arch() {
    assert_eq!(
        ZoneId::from_name("meteor showcase"),
        Some(ZoneId::MeteorShowcase)
    );
    assert_eq!(
        ZoneId::from_name("verse-meteor-showcase"),
        Some(ZoneId::MeteorShowcase)
    );
    for zone in ZoneId::ALL {
        assert!(
            zone.portals()
                .iter()
                .all(|(to, _)| *to != ZoneId::MeteorShowcase),
            "{} has an arch to the showcase",
            zone.label()
        );
    }
    assert_eq!(
        ZoneId::MeteorShowcase.portals(),
        vec![(ZoneId::Plaza, RETURN_PORTAL)]
    );
    assert!(ZoneId::MeteorShowcase.meteor_stage());
}

#[test]
fn eight_meteors_on_distinct_arcs_break_both_houses_and_the_debris_rests() {
    let mut runtime = installed();
    let glade = runtime.zone_state.everglade.as_deref().unwrap();
    assert_eq!(glade.clock().pinned_hour(), Some(HOUR));
    let houses = buildings(town(&runtime));
    assert_ne!(houses[0], houses[1]);
    for building in houses {
        assert!(town(&runtime).buildings()[building].destructible());
    }
    // Nothing casts on its own.
    assert_eq!(town(&runtime).bombardment(), [0, 0]);
    run(&mut runtime, DELAY + CAST + 1.0);
    assert_eq!(town(&runtime).bombardment(), [0, 0]);
    assert_eq!(town(&runtime).swarm().meteors_left(), 0);
    // The film's staged caster: the cast gathers, then all eight set out
    // on arcs of their own.
    assert!(runtime.stage_meteor_showcase(DELAY));
    assert_eq!(town(&runtime).bombardment(), [1, 0]);
    run(&mut runtime, DELAY + CAST + 0.05);
    let arcs = town(&runtime).casters()[0].arcs();
    assert_eq!(arcs.len(), 8, "{arcs:?}");
    for (i, a) in arcs.iter().enumerate() {
        // None comes straight down.
        assert!(a.1 < 1.15 && a.1 > 0.2, "meteor {i} descends at {}", a.1);
        for b in &arcs[i + 1..] {
            let turn = (a.0 - b.0 + PI).rem_euclid(TAU) - PI;
            assert!(
                turn.abs() > 0.05 || (a.1 - b.1).abs() > 0.05,
                "two meteors share an arc: {a:?} {b:?}"
            );
            assert!((a.2 - b.2).abs() > 0.02, "two meteors set out together");
        }
    }
    let headings: Vec<f32> = arcs.iter().map(|a| a.0).collect();
    let spread = headings.iter().copied().fold(f32::MIN, f32::max)
        - headings.iter().copied().fold(f32::MAX, f32::min);
    assert!(spread > 1.2, "the arcs fan only {spread} radians");
    let descents: Vec<f32> = arcs.iter().map(|a| a.1).collect();
    let range = descents.iter().copied().fold(f32::MIN, f32::max)
        - descents.iter().copied().fold(f32::MAX, f32::min);
    assert!(range > 0.15, "the arcs descend alike: {descents:?}");
    // They land, and both houses come apart.
    let mut flying = 0;
    let mut remaining = 8;
    let mut lit_impacts = 0;
    for _ in 0..(5.0 / DT) as usize {
        runtime.tick(&InputState::default(), DT);
        flying = flying.max(town(&runtime).bombardment()[1]);
        let eye = runtime.view(16.0 / 9.0).eye;
        assert!(eye.y >= everglade::land(eye.x, eye.z) + 0.25);
        let left = town(&runtime).casters()[0].meteors_left();
        if left < remaining {
            lit_impacts += remaining - left;
            let lamps = runtime.dynamic_mesh().neon.unwrap().flash_lamps;
            assert_eq!(lamps.iter().filter(|lamp| lamp.lit()).count(), lit_impacts);
            assert!(
                lamps
                    .iter()
                    .filter(|lamp| lamp.lit())
                    .all(|lamp| { lamp.color[0] > lamp.color[2] && lamp.range >= 28.0 })
            );
            remaining = left;
        }
    }
    assert_eq!(flying, 8);
    assert_eq!(lit_impacts, 8, "every staged meteor starts direct light");
    assert_eq!(town(&runtime).bombardment()[1], 0, "every meteor landed");
    let sprites = runtime.dynamic_mesh().sprites;
    let smoke: Vec<_> = sprites.iter().filter(|sprite| sprite.scene_lit).collect();
    assert!(
        !smoke.is_empty(),
        "the ruins emit smoke that takes scene light"
    );
    assert!(
        smoke.iter().all(|sprite| {
            sprite.lit
                && sprite.additive < 1.0
                && sprite.density.is_finite()
                && sprite.density > 0.0
        }),
        "smoke keeps surface-color units, alpha blending, and positive optical density"
    );
    assert!(
        sprites
            .iter()
            .any(|sprite| sprite.additive == 1.0 && !sprite.scene_lit),
        "the ruins keep emissive fire or embers"
    );
    assert!(
        sprites
            .iter()
            .filter(|sprite| sprite.additive == 1.0)
            .all(|sprite| !sprite.scene_lit),
        "additive particles remain emissive"
    );
    run(&mut runtime, 10.0);
    assert!(
        runtime
            .dynamic_mesh()
            .neon
            .unwrap()
            .flash_lamps
            .iter()
            .all(|lamp| !lamp.lit())
    );
    let town_now = town(&runtime);
    for (building, house) in houses.into_iter().zip(showcase::houses()) {
        let (up, all) = standing(town_now, building);
        assert!(
            up * 2 < all,
            "{}: {up} of {all} pieces still stand",
            house.name
        );
    }
    // What fell lies on the ground: nothing under it, nothing still
    // sliding.
    let site = town_now.site();
    let mut resting = 0;
    for (body, _) in site.chunk_bodies() {
        let (at, velocity) = site.body_motion(body);
        assert!(at.y > -0.05, "a chunk sank to {}", at.y);
        assert!(velocity.length() < 2.0, "a chunk still moves at {velocity}");
        resting += 1;
    }
    for piece in site.pieces() {
        if piece.status == Status::Loose {
            let (at, velocity) = site.body_motion(piece.body);
            assert!(at.y > -1.3, "a loose piece sank to {}", at.y);
            assert!(velocity.length() < 2.0, "a loose piece still moves");
        }
    }
    assert!(resting > 0, "the houses left debris");
    // The caster waits for the rebuild, and casts again after it.
    runtime.zone_intent(Intent::Rebuild).unwrap();
    for building in houses {
        let (up, all) = standing(town(&runtime), building);
        assert_eq!(up, all);
    }
    run(&mut runtime, DELAY + CAST + 0.5);
    assert!(
        town(&runtime).bombardment()[1] > 0,
        "the caster casts again"
    );
}

#[test]
fn the_player_walks_up_the_path_to_a_door() {
    let mut runtime = installed();
    let [west, _] = houses();
    let (outside, _) = west.door_points();
    let mut closest = f32::INFINITY;
    for _ in 0..(30.0 / DT) as usize {
        let at = runtime.player.pos;
        let to = Vec3::new(outside[0] - at.x, 0.0, outside[1] - at.z);
        if to.length() < 0.6 {
            break;
        }
        runtime.player.yaw = to.x.atan2(to.z);
        let input = InputState {
            forward: true,
            ..InputState::default()
        };
        runtime.tick(&input, DT);
        let p = runtime.player.pos;
        assert!(
            p.y >= everglade::land(p.x, p.z) - 0.01,
            "below ground at {p}"
        );
        closest = closest.min((p.x - outside[0]).hypot(p.z - outside[1]));
    }
    assert!(closest < 1.0, "the walk stopped {closest} m from the door");
}

#[test]
fn the_players_meteor_swarm_calls_down_eight_and_nothing_else_casts() {
    let mut runtime = installed();
    run(&mut runtime, 8.0);
    assert_eq!(town(&runtime).bombardment(), [0, 0]);
    runtime.zone_intent(Intent::Rebuild).unwrap();
    run(&mut runtime, 8.0);
    assert_eq!(town(&runtime).bombardment(), [0, 0], "R casts nothing");
    // Key 1, the ring on the ground between the houses, and a click.
    runtime.zone_intent(Intent::MeteorSwarm).unwrap();
    let aspect = 16.0 / 9.0;
    let clip = runtime.view(aspect).view_proj * aim().extend(1.0);
    let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
    assert!(
        runtime.demolition_aim(aspect, x, y),
        "the ring finds the lot"
    );
    assert!(runtime.demolition_confirm());
    run(&mut runtime, CAST + 0.05);
    assert_eq!(town(&runtime).swarm().meteors_left(), 8);
}

#[test]
fn broken_chunks_draw_as_gpu_instances_and_rubble_at_rest_merges() {
    let vertices = |set: &crate::pbr::textured::Instances| {
        let counts = set.mesh_vertices();
        set.records
            .iter()
            .map(|r| counts[r.mesh as usize])
            .sum::<usize>()
    };
    let mut runtime = installed();
    assert!(runtime.stage_meteor_showcase(DELAY));
    run(&mut runtime, DELAY + CAST + 2.0);
    // Mid-swarm the broken chunks draw as instances of meshes uploaded
    // once; the pool poses only what is damaged or loose but whole.
    let profile = town(&runtime).profile();
    assert!(profile.chunks > 100, "{} chunks", profile.chunks);
    let mid = runtime.dynamic_mesh().instances;
    assert!(!mid.is_empty() && mid.len() <= crate::pbr::textured::INSTANCE_SETS);
    for set in &mid {
        set.validate().unwrap();
    }
    assert_eq!(mid[0].records.len(), profile.instances);
    let instanced: usize = mid.iter().map(vertices).sum();
    assert!(
        instanced > profile.posed_vertices,
        "{instanced} instanced vertices, {} posed",
        profile.posed_vertices
    );
    // Long after, the rubble at rest is one merged mesh a material, drawn
    // in a few draws, and fewer chunks are left to instance.
    run(&mut runtime, 12.0);
    let late = runtime.dynamic_mesh().instances;
    for set in &late {
        set.validate().unwrap();
    }
    let merged = late
        .iter()
        .find(|set| {
            set.records
                .iter()
                .all(|r| r.transform == glam::Mat4::IDENTITY)
        })
        .expect("the rubble at rest is merged");
    assert!(
        merged.records.len() <= 16,
        "{} merged meshes",
        merged.records.len()
    );
    assert!(vertices(merged) > instanced / 2);
    let late_profile = town(&runtime).profile();
    assert!(
        late_profile.instances < late_profile.chunks,
        "{} chunk parts still instanced of {} chunks",
        late_profile.instances,
        late_profile.chunks
    );
    // Restoring the houses leaves nothing to instance or merge.
    runtime.zone_intent(Intent::Rebuild).unwrap();
    runtime.tick(&InputState::default(), DT);
    assert!(runtime.dynamic_mesh().instances.is_empty());
}
