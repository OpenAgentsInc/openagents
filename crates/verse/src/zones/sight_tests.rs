//! The shared camera-collision step in each zone: the eye never starts
//! behind a wall, stays in front of the one at the player's back, and eases
//! back out once the view clears.

use super::crypt::{self, HALF_X, HALF_Z};
use super::everglade_pack::{self, ZonePack};
use super::{ZoneId, everglade};
use crate::controller::{Footprint, InputState};
use crate::runtime::WorldRuntime;
use glam::Vec3;
use std::f32::consts::PI;
use std::path::Path;
use std::sync::OnceLock;

fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                everglade_pack::PACK_SHA256,
                everglade_pack::PACK_EXTENSION
            ));
        ZonePack::load_local(&path).expect("the committed pack loads")
    })
}

#[test]
fn the_crypt_spawns_with_the_camera_inside_the_hall() {
    let mut runtime = WorldRuntime::new();
    runtime.install_crypt(pack());
    assert_eq!(runtime.zone, ZoneId::Crypt);
    // The first frame, before any tick: the orbit would stand outside the
    // door's wall, so the eye is pulled in front of it.
    let framing = runtime.framing();
    assert!(framing.limited, "{framing:?}");
    for eye in [framing.eye, runtime.view(1.6).eye] {
        assert!(
            eye.z < HALF_Z - 0.1 && eye.z > runtime.player.pos.z,
            "{eye}"
        );
        assert!(eye.x.abs() < HALF_X - 0.1, "{eye}");
        assert!(eye.y > 0.1 && eye.y < crypt::vault_height(eye.x), "{eye}");
    }
    // And it stays there as the player stands.
    for _ in 0..30 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
        let eye = runtime.view(1.6).eye;
        assert!(eye.z < HALF_Z - 0.1, "{eye}");
    }
}

#[test]
fn a_crypt_pillar_between_pulls_the_camera_in_and_it_eases_back_out() {
    let mut runtime = WorldRuntime::new();
    runtime.install_crypt(pack());
    // Beside the east pillars, facing west, with a pillar behind.
    let behind = Vec3::new(crypt::PILLAR_X - 2.3, 0.0, crypt::PILLARS_Z[2]);
    runtime.set_spawn(behind, -PI / 2.0).unwrap();
    runtime.camera.distance = 6.0;
    runtime.tick(&InputState::default(), 1.0 / 60.0);
    let held = runtime.view(1.6).eye;
    assert!(held.x < crypt::PILLAR_X - 0.3, "{held}");
    // Turning the orbit away from the pillar, toward the hall, clears the
    // view: the eye eases out over several frames.
    runtime.camera.yaw_offset = PI;
    let mut last = runtime.view(1.6).eye.distance(runtime.player.pos);
    let mut frames = 0;
    while runtime.framing().limited && frames < 600 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
        let now = runtime.view(1.6).eye.distance(runtime.player.pos);
        assert!(now + 1e-4 >= last && now - last < 0.5, "{last} to {now}");
        last = now;
        frames += 1;
    }
    assert!(frames > 3 && frames < 600, "{frames}");
}

/// A kit house's long wall in Everglade: its footprint and top.
fn house_wall() -> (Footprint, f32) {
    everglade::layout::city::kit_blocks()
        .into_iter()
        .find(|(f, top)| {
            let (wide, deep) = (f.max[0] - f.min[0], f.max[1] - f.min[1]);
            let ground = everglade::height(0.5 * (f.min[0] + f.max[0]), f.max[1]);
            wide > 4.0 && deep < 1.0 && *top - ground > 2.5
        })
        .expect("a kit house has a long wall")
}

#[test]
fn backing_onto_an_everglade_house_wall_keeps_the_camera_in_front() {
    let mut runtime = WorldRuntime::new();
    runtime.install_everglade(pack());
    assert_eq!(runtime.zone, ZoneId::Everglade);
    let (wall, _) = house_wall();
    // Just off the wall, facing away: the wall is at the back.
    let (at, yaw, in_front) = backed_onto(&wall);
    runtime.set_spawn(at, yaw).unwrap();
    for frame in 0..10 {
        let eye = runtime.view(1.5).eye;
        assert!(in_front(eye), "frame {frame}: {eye} behind {wall:?}");
        assert!(eye.y > everglade::height(eye.x, eye.z), "{eye}");
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    // That close, the avatar is not drawn over the whole view.
    assert!(runtime.framing().close && runtime.hides_avatar());
}

/// Where to stand 0.6 m off `wall`'s far side along its thin axis, facing
/// away from it, and whether an eye is still on that side.
fn backed_onto(wall: &Footprint) -> (Vec3, f32, impl Fn(Vec3) -> bool) {
    let wall = *wall;
    let thin_z = wall.max[1] - wall.min[1] < wall.max[0] - wall.min[0];
    let (cx, cz) = (
        0.5 * (wall.min[0] + wall.max[0]),
        0.5 * (wall.min[1] + wall.max[1]),
    );
    let (at, yaw) = if thin_z {
        (Vec3::new(cx, 0.0, wall.max[1] + 0.6), 0.0)
    } else {
        (Vec3::new(wall.max[0] + 0.6, 0.0, cz), PI / 2.0)
    };
    let in_front = move |eye: Vec3| {
        if thin_z {
            eye.z > wall.max[1]
        } else {
            eye.x > wall.max[0]
        }
    };
    (at, yaw, in_front)
}

#[test]
fn the_grid_workstations_leave_the_spawn_camera_free_and_the_plaza_towers_do_not() {
    let mut grid = WorldRuntime::bare();
    assert_eq!(
        grid.world.blockers.len(),
        crate::grid_workstation::SITES.len()
    );
    grid.tick(&InputState::default(), 1.0 / 60.0);
    let framing = grid.framing();
    assert!(!framing.limited, "{framing:?}");
    // The furnished plaza: the tallest blocker's top comes from what
    // stands on it, and backing onto it holds the eye in front.
    let mut plaza = WorldRuntime::new();
    plaza.tick(&InputState::default(), 1.0 / 60.0);
    let tops = plaza.zone_state.sight_tops.2.clone();
    assert_eq!(tops.len(), plaza.world.blockers.len());
    let (index, tower) = plaza
        .world
        .blockers
        .iter()
        .enumerate()
        .filter(|(_, f)| f.max[0] - f.min[0] > 4.0 && f.max[1] - f.min[1] > 4.0)
        .max_by(|a, b| tops[a.0].total_cmp(&tops[b.0]))
        .map(|(i, f)| (i, *f))
        .expect("the plaza has a tower");
    assert!(
        tops[index] > 8.0 && tops[index].is_finite(),
        "{}",
        tops[index]
    );
    let (at, yaw, in_front) = backed_onto(&tower);
    plaza.set_spawn(at, yaw).unwrap();
    plaza.tick(&InputState::default(), 1.0 / 60.0);
    let eye = plaza.view(1.5).eye;
    assert!(in_front(eye), "{eye} behind {tower:?}");
}

#[test]
fn lagrange_frames_without_blockers() {
    let runtime = WorldRuntime::new();
    // Lagrange flies free; it does not pull the camera in on open ground.
    let mut lagrange = WorldRuntime::new();
    lagrange.zone = ZoneId::Lagrange1;
    assert!(lagrange.framing().eye.is_finite());
    assert!(!runtime.framing().limited);
}
