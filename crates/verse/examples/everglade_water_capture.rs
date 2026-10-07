//! Offline pictures of Everglade's water (`docs/verse/water.md`, phase W3).
//!
//! Usage: everglade_water_capture OUTPUT_DIRECTORY
//!
//! Installs Everglade from the committed, pinned pack, settles its light,
//! and renders with Everglade's hotbar and breath bar into
//! `OUTPUT_DIRECTORY`:
//!
//! - `lantern.png`, `reed.png`, `thinking.png`, `fern.png`: each pond from
//!   its bank, its water drawn by the shared water shader over its carved
//!   bowl.
//! - `weir.png`: Glade Run's weir from downstream: the stones, the falling
//!   sheet, the plunge pool's foam, and the mist.
//! - `wading.png`: the player wading across Glade Run.
//! - `swimming.png`: the player swimming in Lantern Pond.
//! - `diving.png`: in first person under Lantern Pond's surface, with the
//!   breath bar.
//! - `exhausted.png`: out of breath on Lantern Pond's bed, with the
//!   Exhaustion debuff beside the bar.
//!
//! It writes `capture.json`: each picture's place, the player's medium and
//! breath there, and the picture's SHA-256. `VERSE_QUALITY` (`low`,
//! `medium`, `high`) picks the tier.

use std::path::{Path, PathBuf};

use glam::Vec3;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};
use verse_world::social::everglade_water::{self as ew, PONDS};
use verse_world::water::medium;

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), String> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let mut runtime = WorldRuntime::new();
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::everglade::hotbar::add_sprites(&mut atlas)?;
    let mut records = Vec::new();

    // Each pond with the player swimming in it, looking across the
    // water toward a bank, the camera close behind over the water.
    let ponds = [
        ("lantern", 0, -1.75_f32),
        ("reed", 1, 0.2),
        ("thinking", 2, -0.6),
        ("fern", 3, -2.3),
    ];
    for (name, k, angle) in ponds {
        let ([cx, cz], _) = PONDS[k];
        let (dx, dz) = (angle.cos(), angle.sin());
        let float = ew::surface(cx, cz).unwrap() - medium::FLOAT_DEPTH as f32;
        runtime.set_spawn(Vec3::new(cx + dx, float, cz + dz), (-dx).atan2(-dz))?;
        frame_camera(&mut runtime, 5.5, 0.42);
        tick(&mut runtime, &InputState::default(), 0.3);
        records.push(shot(&runtime, &atlas, &dir, name)?);
    }

    // The weir from downstream: wading in the run below the plunge pool,
    // looking up at the stones, the falling sheet, and the foam.
    let run = ew::run();
    let along = run.weir + ew::POOL_BELOW + ew::POOL_RADIUS + 1.8;
    let [x, z] = run.point_at(along);
    let [wx, wz] = run.point_at(run.weir);
    stand(&mut runtime, Vec3::new(x, 0.0, z), Vec3::new(wx, 0.0, wz))?;
    frame_camera(&mut runtime, 6.0, 0.3);
    tick(&mut runtime, &InputState::default(), 0.5);
    records.push(shot(&runtime, &atlas, &dir, "weir")?);

    // Wading across the run, seen from the side.
    let along = 30.0;
    let [x, z] = run.point_at(along);
    let [tx, tz] = run.tangent_at(along);
    let (nx, nz) = (-tz, tx);
    let off = run.half_at(along) + 1.6;
    stand(
        &mut runtime,
        Vec3::new(x - nx * off, 0.0, z - nz * off),
        Vec3::new(x + nx * off, 0.0, z + nz * off),
    )?;
    frame_camera(&mut runtime, 6.5, 0.35);
    runtime.apply(Action::Orbit { dx: 70.0, dy: 0.0 })?;
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    tick(&mut runtime, &walk, 0.55);
    records.push(shot(&runtime, &atlas, &dir, "wading")?);

    // Swimming in Lantern Pond.
    let ([cx, cz], _) = PONDS[0];
    let float = ew::surface(cx, cz).unwrap() - medium::FLOAT_DEPTH as f32;
    runtime.set_spawn(Vec3::new(cx - 2.5, float, cz - 1.0), 0.6)?;
    runtime.apply(Action::Orbit {
        dx: 120.0,
        dy: 20.0,
    })?;
    tick(&mut runtime, &walk, 0.8);
    records.push(shot(&runtime, &atlas, &dir, "swimming")?);

    // Under the surface in first person, then out of breath on the bed.
    runtime.set_spawn(Vec3::new(cx, float, cz), 0.0)?;
    runtime.apply(Action::Zoom { lines: 40.0 })?;
    // Look down past the dive pitch and stroke down, circling.
    runtime.apply(Action::Orbit { dx: 0.0, dy: 400.0 })?;
    let dive = InputState {
        forward: true,
        left: true,
        ..InputState::default()
    };
    tick(&mut runtime, &dive, 2.0);
    runtime.apply(Action::Orbit {
        dx: 0.0,
        dy: -330.0,
    })?;
    tick(&mut runtime, &InputState::default(), 0.05);
    records.push(shot(&runtime, &atlas, &dir, "diving")?);
    runtime.apply(Action::Orbit { dx: 0.0, dy: 330.0 })?;
    // Breath runs out after two minutes; two Exhaustion levels follow.
    let limit = runtime.everglade_breath().map_or(120.0, |b| b.limit);
    tick(&mut runtime, &dive, limit + 13.0);
    runtime.apply(Action::Orbit {
        dx: 0.0,
        dy: -330.0,
    })?;
    tick(&mut runtime, &InputState::default(), 0.05);
    records.push(shot(&runtime, &atlas, &dir, "exhausted")?);

    let capture = json!({
        "schema": "openagents.verse.everglade_water_capture.v1",
        "pack": everglade_pack::PACK_SHA256,
        "quality": std::env::var("VERSE_QUALITY").unwrap_or_else(|_| "auto".into()),
        "pictures": records,
    });
    std::fs::write(
        dir.join("capture.json"),
        serde_json::to_string_pretty(&capture).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())
}

/// Puts the camera `distance` meters behind the player at `pitch`.
fn frame_camera(runtime: &mut WorldRuntime, distance: f32, pitch: f32) {
    runtime.camera = verse::camera::FollowCamera::default();
    runtime.camera.distance = distance;
    runtime.camera.pitch = pitch;
}

/// Stands the player at `at` on the ground facing `toward`.
fn stand(runtime: &mut WorldRuntime, at: Vec3, toward: Vec3) -> Result<(), String> {
    let ground = zones::everglade::height(at.x, at.z);
    let yaw = (toward.x - at.x).atan2(toward.z - at.z);
    runtime.set_spawn(Vec3::new(at.x, ground, at.z), yaw)?;
    runtime.camera = verse::camera::FollowCamera::default();
    Ok(())
}

fn tick(runtime: &mut WorldRuntime, input: &InputState, seconds: f32) {
    for _ in 0..(seconds / DT).round().max(1.0) as usize {
        runtime.tick(input, DT);
    }
}

/// Renders the frame to `dir/name.png` and returns its record.
fn shot(
    runtime: &WorldRuntime,
    atlas: &verse::ui::Atlas,
    dir: &Path,
    name: &str,
) -> Result<Value, String> {
    let (width, height) = (1280, 800);
    let size = [width as f32, height as f32];
    let mut ui = verse::ui::UiBatch::default();
    if let Some(slots) = runtime.everglade_hotbar() {
        zones::everglade::hotbar::draw(&mut ui, atlas, size, 14.0, &slots);
    }
    let breath = runtime.everglade_breath();
    if let Some(bar) = &breath {
        zones::everglade::water::draw_breath(&mut ui, atlas, size, 14.0, bar);
    }
    let path = dir.join(format!("{name}.png"));
    verse::render::capture_with_atmosphere(
        &path,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(1.6),
        &runtime.dynamic_mesh(),
        &ui,
        atlas,
        zones::atmosphere(runtime.zone),
    )?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let p = runtime.player.pos;
    let top = ew::surface(p.x, p.z);
    let bed = zones::everglade::height(p.x, p.z);
    let medium = medium::classify(top.map(f64::from), f64::from(bed), f64::from(p.y));
    let eye = runtime.view(1.6).eye;
    eprintln!("{name}: player at {p}, {}, eye {eye}", medium.name());
    Ok(json!({
        "name": name,
        "file": format!("{name}.png"),
        "sha256": digest,
        "player": [p.x, p.y, p.z],
        "eye": [eye.x, eye.y, eye.z],
        "surface": top,
        "bed": bed,
        "medium": medium.name(),
        "breath": breath.map(|b| json!({
            "left": b.left,
            "limit": b.limit,
            "exhaustion": b.levels,
            "under": b.under,
        })),
    }))
}
