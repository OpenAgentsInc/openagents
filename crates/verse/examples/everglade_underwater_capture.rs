//! Offline pictures of Everglade from under its water
//! (`docs/verse/water.md`, phase W7).
//!
//! Usage: everglade_underwater_capture OUTPUT_DIRECTORY
//!
//! Installs Everglade from the committed, pinned pack, settles its light,
//! puts the player in Lantern Pond, and renders from fixed eyes in first
//! person into `OUTPUT_DIRECTORY`:
//!
//! - `waterline.png`: the eye exactly at the pond's surface, the view split
//!   per pixel into the air above and the water below.
//! - `snell.png`: from the pond's bed, looking up at the surface: Snell's
//!   window, and total internal reflection outside it.
//! - `bed.png`: under the surface, looking across the bed: fog, caustics,
//!   motes, and the sun's shafts.
//! - `jetty.png`: under the surface, looking at the jetty's posts and the
//!   moored rowboat: caustics on the posts.
//! - `swimmer.png`: the player diving, seen from under the water: caustics
//!   on a submerged character, and `swimmer-no-caustics.png` the same
//!   without them.
//! - `above.png`: the pond's shallows from over its bank: caustics on the
//!   bed through the surface.
//!
//! It writes `capture.json`: each picture's eye, the surface over it, and
//! the picture's SHA-256. `VERSE_QUALITY` (`low`, `medium`, `high`) picks
//! the tier.

use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse::{
    controller::InputState,
    render::View,
    runtime::WorldRuntime,
    zones::{self, everglade_pack},
};
use verse_world::social::everglade_water::{self as ew, PONDS};

const DT: f32 = 1.0 / 60.0;
const SIZE: (u32, u32) = (1280, 800);

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
    let atlas = verse::ui::Atlas::new(16.0);
    let mut records = Vec::new();

    // Lantern Pond: its center, its level, and the jetty's side.
    let ([cx, cz], r) = PONDS[0];
    let level = ew::surface(cx, cz).ok_or("Lantern Pond has no surface")?;
    let jetty = -2.35_f32;
    let (jx, jz) = (cx + jetty.cos() * r, cz + jetty.sin() * r);

    // The player swims at the pond's middle, out of the views but in the
    // water, so the zone's swimming state runs.
    let float = level - verse_world::water::FLOAT_DEPTH as f32;
    runtime.set_spawn(Vec3::new(cx + 2.0, float, cz + 2.0), 0.0)?;
    tick(&mut runtime, 0.5);

    // The eye exactly at the surface, looking across toward the jetty.
    let from = Vec3::new(cx + 3.5, 0.0, cz + 2.5);
    let toward = Vec3::new(jx, 0.0, jz);
    let eye = from.with_y(surface_at(&runtime, from));
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "waterline",
        eye,
        toward.with_y(eye.y - 0.05),
    )?);

    // From near the bed, looking up at the surface.
    let eye = Vec3::new(cx - 1.0, level - 2.2, cz + 0.5);
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "snell",
        eye,
        eye + Vec3::new(0.6, 2.0, -0.9),
    )?);

    // Across the bed toward the shallows and the jetty.
    let eye = Vec3::new(cx + 2.5, level - 1.0, cz + 2.0);
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "bed",
        eye,
        Vec3::new(jx, level - 1.9, jz),
    )?);

    // Toward the jetty from under the water: its posts and the moored
    // rowboat, and their shadows on the bed.
    let along = |d: f32| Vec3::new(cx + jetty.cos() * d, 0.0, cz + jetty.sin() * d);
    let eye = along(r - 7.0).with_y(level - 0.8);
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "jetty",
        eye,
        along(r).with_y(level - 0.4),
    )?);

    // The player diving under the surface, seen with the sun behind the
    // eye.
    let sun = runtime
        .dynamic_mesh()
        .neon
        .and_then(|n| n.key)
        .map_or(Vec3::Y, |k| k.dir);
    let behind = Vec3::new(sun.x, 0.0, sun.z).normalize_or(Vec3::X);
    let diver = Vec3::new(cx + 0.5, level - 1.9, cz - 0.5);
    runtime.set_spawn(diver, 0.8)?;
    tick(&mut runtime, DT);
    let side = Vec3::new(-behind.z, 0.0, behind.x);
    let eye = diver + behind * 1.0 + side * 0.9 + Vec3::Y * 1.45;
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "swimmer",
        eye,
        diver + Vec3::Y * 0.6,
    )?);
    // The same without caustics, for the difference they make on the
    // character.
    records.push(shot_with(
        &runtime,
        &atlas,
        &dir,
        "swimmer-no-caustics",
        eye,
        diver + Vec3::Y * 0.6,
        Some(0.0),
    )?);

    // The pond from over its water, the bed lit through the surface.
    runtime.set_spawn(Vec3::new(cx - 2.0, float, cz - 2.0), 0.0)?;
    tick(&mut runtime, DT);
    let eye = Vec3::new(cx + 1.0, level + 3.5, cz + 1.0);
    records.push(shot(
        &runtime,
        &atlas,
        &dir,
        "above",
        eye,
        Vec3::new(cx + 4.0, level - 1.5, cz + 3.5),
    )?);

    let capture = json!({
        "schema": "openagents.verse.everglade_underwater_capture.v1",
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

fn tick(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round().max(1.0) as usize {
        runtime.tick(&InputState::default(), DT);
    }
}

/// The drawn surface's height over `at`: the zone's water seen from there.
fn surface_at(runtime: &WorldRuntime, at: Vec3) -> f32 {
    let mut mesh = runtime.dynamic_mesh();
    let Some(water) = mesh.neon.as_mut().and_then(|n| n.water.as_mut()) else {
        return ew::surface(at.x, at.z).unwrap_or(at.y);
    };
    zones::everglade::water::see_from(water, at);
    water
        .eye
        .map_or_else(|| ew::surface(at.x, at.z).unwrap_or(at.y), |s| s.height)
}

/// Renders the frame from `eye` toward `target` in first person to
/// `dir/name.png` and returns its record.
fn shot(
    runtime: &WorldRuntime,
    atlas: &verse::ui::Atlas,
    dir: &Path,
    name: &str,
    eye: Vec3,
    target: Vec3,
) -> Result<Value, String> {
    shot_with(runtime, atlas, dir, name, eye, target, None)
}

/// As [`shot`], with the caustics' strength replaced by `caustics` when
/// given.
fn shot_with(
    runtime: &WorldRuntime,
    atlas: &verse::ui::Atlas,
    dir: &Path,
    name: &str,
    eye: Vec3,
    target: Vec3,
    caustics: Option<f32>,
) -> Result<Value, String> {
    let (width, height) = SIZE;
    let aspect = width as f32 / height as f32;
    let view = View {
        view_proj: Mat4::perspective_rh(
            verse::camera::FOV_Y,
            aspect,
            verse::camera::FIRST_PERSON_NEAR,
            verse::camera::FAR,
        ) * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    };
    // The frame's water seen from this eye: the surface over it and the
    // motes around it, as the runtime does for its own eye.
    let mut dynamic = runtime.dynamic_mesh();
    let mut surface = None;
    if let Some(water) = dynamic.neon.as_mut().and_then(|n| n.water.as_mut()) {
        zones::everglade::water::see_from(water, eye);
        if let Some(c) = caustics {
            water.caustics = c;
        }
        surface = water.eye.map(|s| s.height);
        let motes = zones::everglade::water::motes(water, eye);
        dynamic.sprites.extend(motes);
    }
    let path = dir.join(format!("{name}.png"));
    verse::render::capture_with_atmosphere(
        &path,
        width,
        height,
        &runtime.world.mesh,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        atlas,
        zones::atmosphere(runtime.zone),
    )?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    eprintln!("{name}: eye {eye}, surface {surface:?}");
    Ok(json!({
        "name": name,
        "file": format!("{name}.png"),
        "sha256": digest,
        "eye": [eye.x, eye.y, eye.z],
        "target": [target.x, target.y, target.z],
        "surface": surface,
    }))
}
