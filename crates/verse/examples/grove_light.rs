//! The Grove's light: a set of views and frame times with one renderer.
//! Usage: grove_light OUT_DIR [--frames N]
//!
//! Installs the Grove from the committed, pinned Everglade pack and writes,
//! into `OUT_DIR`:
//!
//! - `spawn.png`: the field from the spawn.
//! - `tower.png`: the concrete tower from the dummies' side.
//! - `meteor_fall.png` and `meteor.png`: Meteor Swarm's meteors falling
//!   on the tower's side, and bursting on it.
//! - `bolt.png`: the Thunderbolt's flash on the tower.
//! - `breath.png`: the dragon breathing fire on the dummies.
//! - `fireball.png`, `wall.png`, `moonbeam.png`, `sunbeam.png`,
//!   `flame.png`, and `shapechange.png`: those spells mid-cast.
//!
//! Then it prints the frame times of two runs of `N` frames (240 by
//! default) at 60 Hz: walking the field at street level, and Meteor Swarm
//! cast again and again on the tower. Each prints the simulation and frame
//! building on the CPU, and the GPU render with its readback.
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::Vec3;
use verse::controller::InputState;
use verse::render::Offscreen;
use verse::runtime::{Action, WorldRuntime};
use verse::zones::grove::kit::{Land, Spell};
use verse::zones::grove::layout::TOWER;
use verse::zones::grove::{hotbar, slots};
use verse::zones::{self, Intent, everglade::demolition::meteor, everglade_pack};

const DT: f32 = 1.0 / 60.0;
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const ASPECT: f32 = WIDTH as f32 / HEIGHT as f32;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let mut frames = 240usize;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--frames" => {
                frames = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--frames takes a count")?;
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let fresh = || -> Result<WorldRuntime, String> {
        let mut runtime = WorldRuntime::new();
        runtime.install_grove(&pack);
        if runtime.zone != zones::ZoneId::Grove {
            return Err("The Grove did not install from the pinned pack".into());
        }
        runtime.settle_zone_light();
        runtime.apply(Action::Orbit { dx: 0.0, dy: 60.0 })?;
        run(&mut runtime, 0.5);
        Ok(runtime)
    };
    let mut runtime = fresh()?;
    let mut atlas = verse::ui::Atlas::new(16.0);
    hotbar::add_sprites(&mut atlas)?;
    let mut renderer = Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(zones::ZoneId::Grove),
    )?;
    let mut shot = |runtime: &WorldRuntime, name: &str| -> Result<(), String> {
        let pixels = renderer.render(
            runtime.view(ASPECT),
            &runtime.dynamic_mesh(),
            &verse::ui::UiBatch::default(),
        )?;
        write_png(&out.join(format!("{name}.png")), &pixels)
    };
    shot(&runtime, "spawn")?;

    // The tower from the south-east, its south and east faces in view.
    face_tower(&mut runtime)?;
    shot(&runtime, "tower")?;

    // Meteor Swarm on the tower's side, as the meteors burst.
    swarm(&mut runtime, Spell::MeteorSwarm, 6.5)?;
    run(&mut runtime, meteor::CAST + 0.55);
    shot(&runtime, "meteor_fall")?;
    run(&mut runtime, 0.6);
    shot(&runtime, "meteor")?;
    // The Thunderbolt on the whole tower, a few frames after it strikes.
    let mut runtime = fresh()?;
    face_tower(&mut runtime)?;
    swarm(&mut runtime, Spell::Thunderbolt, 14.0)?;
    run(&mut runtime, meteor::BOLT_CAST + 0.1);
    shot(&runtime, "bolt")?;

    // The spells at the straw dummy, from 8 m.
    for (name, spell, age) in [
        ("fireball", Spell::Fireball, 0.75),
        ("flame", Spell::ProduceFlame, 0.3),
        ("wall", Spell::WallOfFire, 1.0),
        ("moonbeam", Spell::Moonbeam, 1.0),
        ("sunbeam", Spell::Sunbeam, 0.4),
        ("shapechange", Spell::Shapechange, 0.6),
    ] {
        let mut runtime = fresh()?;
        runtime.set_spawn(Vec3::new(0.0, 0.0, -13.0), 0.0)?;
        run(&mut runtime, 0.2);
        cast(&mut runtime, spell)?;
        run(&mut runtime, age);
        shot(&runtime, name)?;
    }

    // The dragon's breath, as `grove_capture breath` frames it.
    let mut runtime = fresh()?;
    runtime.set_spawn(Vec3::new(0.0, 0.0, -14.0), 0.0)?;
    cast(&mut runtime, Spell::Shapechange)?;
    run(&mut runtime, 2.0);
    runtime.set_spawn(Vec3::new(0.0, 0.0, -14.0), 0.0)?;
    runtime.apply(Action::Orbit {
        dx: 120.0,
        dy: -20.0,
    })?;
    run(&mut runtime, 0.1);
    runtime.zone_intent(Intent::GroveSlot(13))?;
    run(&mut runtime, 1.0);
    shot(&runtime, "breath")?;

    // Frame times: a walk across the field at street level.
    let mut runtime = fresh()?;
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    let (cpu, gpu) = time(&mut runtime, &mut renderer, frames, |runtime, i| {
        if i % 120 == 0 {
            runtime.set_spawn(zones::grove::SPAWN, 0.0)?;
        }
        runtime.tick(&walk, DT);
        Ok(())
    })?;
    report("street: cpu (tick and frame)", cpu);
    report("street: gpu (render and readback)", gpu);

    // Frame times: Meteor Swarm again and again on the tower.
    face_tower(&mut runtime)?;
    let (cpu, gpu) = time(&mut runtime, &mut renderer, frames, |runtime, i| {
        if i % 180 == 0 {
            swarm(runtime, Spell::MeteorSwarm, 6.5 + (i / 180) as f32 * 3.0)?;
        }
        runtime.tick(&InputState::default(), DT);
        Ok(())
    })?;
    report("meteor swarm: cpu (tick and frame)", cpu);
    report("meteor swarm: gpu (render and readback)", gpu);
    Ok(())
}

/// Stands south-east of the tower, facing it, the camera looking up its
/// side.
fn face_tower(runtime: &mut WorldRuntime) -> Result<(), String> {
    let stand = Vec3::new(TOWER[0] - 4.0, 0.0, TOWER[1] - 15.0);
    runtime.set_spawn(stand, 4.0_f32.atan2(15.0))?;
    runtime.apply(Action::Zoom { lines: -4.0 })?;
    runtime.apply(Action::Orbit {
        dx: 0.0,
        dy: -120.0,
    })?;
    run(runtime, 0.1);
    Ok(())
}

fn run(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
    }
}

fn cast(runtime: &mut WorldRuntime, spell: Spell) -> Result<(), String> {
    let slot = slots::slot_of(spell, None, Land::Arid).ok_or("the spell is on the bar")?;
    runtime.zone_intent(Intent::GroveSlot(slot as u8))
}

/// Aims `spell` at the tower's south face, `level` m up, and calls it down.
fn swarm(runtime: &mut WorldRuntime, spell: Spell, level: f32) -> Result<(), String> {
    cast(runtime, spell)?;
    let side = Vec3::new(TOWER[0] + 0.3, level, TOWER[1] - 2.8);
    let clip = runtime.view(ASPECT).view_proj * side.extend(1.0);
    let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
    if !runtime.demolition_aim(ASPECT, x, y) {
        return Err("The ring found nothing under the cursor".into());
    }
    if !runtime.demolition_confirm() {
        return Err(format!("{spell:?} did not start"));
    }
    Ok(())
}

/// Runs `frames` frames of `step` and renders each, returning the CPU and
/// GPU times, ms.
fn time(
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    frames: usize,
    mut step: impl FnMut(&mut WorldRuntime, usize) -> Result<(), String>,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let ui = verse::ui::UiBatch::default();
    let mut cpu = Vec::with_capacity(frames);
    let mut gpu = Vec::with_capacity(frames);
    for i in 0..frames {
        let started = Instant::now();
        step(runtime, i)?;
        let view = runtime.view(ASPECT);
        let dynamic = runtime.dynamic_mesh();
        let built = Instant::now();
        renderer.render(view, &dynamic, &ui)?;
        let done = Instant::now();
        cpu.push((built - started).as_secs_f64() * 1e3);
        gpu.push((done - built).as_secs_f64() * 1e3);
    }
    Ok((cpu, gpu))
}

fn report(name: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    println!(
        "{name}: median {:.2} ms, p95 {:.2} ms, max {:.2} ms over {} frames",
        at(0.5),
        at(0.95),
        at(1.0),
        samples.len()
    );
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
