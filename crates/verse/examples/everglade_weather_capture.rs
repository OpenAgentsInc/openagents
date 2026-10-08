//! Offline pictures of Everglade's weather (`docs/verse/water.md`, phase
//! W9): the same views dry, during rain, and after rain.
//!
//! Usage: everglade_weather_capture OUTPUT_DIRECTORY
//!
//! Installs Everglade from the committed, pinned pack, fixes the world tick,
//! and pins the weather schedule (the development control), so each
//! picture is the same on every run:
//!
//! - `dry`: clear for all time.
//! - `rain`: clear until 20 minutes before the tick, then rain: the ground
//!   soaked, the puddles full, streaks and splash-back around the camera,
//!   rain ripples on the water, and the ponds risen.
//! - `after`: rain from 40 to 6 minutes before the tick, then clear: wet
//!   ground drying, puddles still standing, no rain falling.
//!
//! Each state is drawn from two places: `pond`, across Lantern Pond from
//! its bank, and `street`, at the town's spawn among its houses. Into
//! `OUTPUT_DIRECTORY`: `{state}-{place}.png` and `capture.json` with each
//! picture's weather, ground, rise, particle counts, ripple sources, and
//! SHA-256. `VERSE_QUALITY` (`low`, `medium`, `high`) picks the tier.

use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::Vec3;
use physics::water::weather::{Schedule, State};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse::{
    controller::InputState,
    render::Offscreen,
    runtime::WorldRuntime,
    zones::{self, everglade_pack},
};
use verse_world::social::everglade_water::{self as ew, PONDS};

const DT: f32 = 1.0 / 60.0;
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
/// The world tick every picture is sampled at.
const TICK: u64 = 1_000_000 * physics::water::TICK_HZ;
const MINUTE: u64 = 60 * physics::water::TICK_HZ;

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
    let quality = std::env::var("VERSE_QUALITY").unwrap_or_else(|_| "auto".into());
    let tier = match quality.as_str() {
        "low" => verse_engine::quality::Tier::Low,
        "medium" => verse_engine::quality::Tier::Medium,
        _ => verse_engine::quality::Tier::High,
    };
    let atlas = verse::ui::Atlas::new(16.0);
    let mut off = Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(runtime.zone),
    )?;
    let mut records: Vec<Value> = Vec::new();
    let states: [(&str, Schedule); 3] = {
        let base = Schedule::new(
            zones::everglade::weather::CLIMATE,
            zones::everglade::weather::SEED,
        )
        .pinned(State::Clear);
        let mut rain = base.clone();
        rain.pin(TICK - 20 * MINUTE, State::Rain);
        let mut after = base.clone();
        after.pin(TICK - 40 * MINUTE, State::Rain);
        after.pin(TICK - 6 * MINUTE, State::Clear);
        [("dry", base), ("rain", rain), ("after", after)]
    };
    for (state, schedule) in states {
        {
            let glade = runtime
                .everglade_zone_mut()
                .ok_or("Everglade is not installed")?;
            glade.set_water_tier(tier);
            let sky = glade.sky_mut().ok_or("Everglade has no weather")?;
            sky.set_schedule(schedule);
            sky.fix(Some(TICK));
        }
        for place in ["pond", "street", "workshop", "house", "porch", "agora", "arcade", "civic"] {
            match place {
                "pond" => {
                    let ([cx, cz], r) = PONDS[0];
                    let at = Vec3::new(cx - r - 4.5, 0.0, cz + 3.0);
                    stand(&mut runtime, at, Vec3::new(cx + 2.0, 0.0, cz - 1.0))?;
                    camera(&mut runtime, 4.5, 0.32);
                }
                "workshop" => {
                    runtime.set_spawn(Vec3::new(0.0, 0.1, 6.0), 0.0)?;
                    camera(&mut runtime, 1.0, 0.15);
                }
                "house" | "porch" | "agora" | "arcade" | "civic" => {
                    use zones::everglade::layout::{estate, agora, civic};
                    let (building, local, floor) = match place {
                        "house" => (estate::OWNERS_HOUSE, [0.0,-18.4], estate::FLOOR),
                        "porch" => (estate::OWNERS_HOUSE, [0.0,-10.0], estate::FLOOR),
                        "agora" => (agora::AGORA, [0.0,-15.0], agora::FLOOR),
                        "arcade" => (agora::AGORA, [12.0,-15.0], agora::FLOOR),
                        _ => (civic::CIVIC, [0.0,-19.6], civic::FLOOR),
                    };
                    let [x,z] = building.world(local);
                    runtime.set_spawn(Vec3::new(x, zones::everglade::height(x,z)+floor, z), building.yaw)?;
                    camera(&mut runtime, 1.0, 0.15);
                }
                _ => {
                    let at = zones::everglade::Everglade::spawn();
                    runtime.set_spawn(at, zones::everglade::Everglade::spawn_yaw())?;
                    camera(&mut runtime, 5.0, 0.3);
                }
            }
            // Long enough for the streaks to fill the air and the ripple
            // field to take the drops.
            let ui = verse::ui::UiBatch::default();
            for i in 0..150 {
                runtime.tick(&InputState::default(), DT);
                if i % 3 == 0 {
                    off.render(runtime.view(1.6), &runtime.dynamic_mesh(), &ui)?;
                }
            }
            runtime.tick(&InputState::default(), DT);
            let dynamic = runtime.dynamic_mesh();
            let started = Instant::now();
            let pixels = off.render(runtime.view(1.6), &dynamic, &ui)?;
            let frame_ms = started.elapsed().as_secs_f64() * 1e3;
            let name = format!("{state}-{place}");
            let path = dir.join(format!("{name}.png"));
            write_png(&path, &pixels)?;
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let digest: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let water = dynamic.neon.as_ref().and_then(|n| n.water);
            let sources = water.map_or(0, |w| w.sources().len());
            let runtime_eye = runtime.view(1.6).eye;
            let glade = runtime
                .everglade_zone_mut()
                .ok_or("Everglade is not installed")?;
            let sky = glade.sky().ok_or("Everglade has no weather")?.clone();
            let rain_particles = glade.rainfall().map_or(0, |r| r.particles.len());
            let mut visible_rain = Vec::new();
            if let Some(rain) = glade.rainfall() {
                rain.draw_covered(&mut visible_rain, runtime_eye, |p| glade.solids().rain_open(p));
            }
            let rain_budget = glade.rainfall().map_or(0, |r| r.budget());
            let water_particles = glade.water_fx().map_or(0, |fx| fx.len());
            let wet_character = glade.character_wetness();
            let w = sky.weather;
            eprintln!(
                "{name}: {} rain {:.2} wet {:.2} puddles {:.2} rise {:.3} m, {rain_particles}/{rain_budget} rain particles, {sources} ripple sources",
                zones::everglade::weather::name(w.state),
                w.rain,
                sky.ground.wet,
                sky.ground.puddles,
                sky.rise
            );
            records.push(json!({
                "name": name,
                "file": format!("{name}.png"),
                "state": w.state.name(),
                "rain": w.rain,
                "wind": [w.wind.x, w.wind.y],
                "cloud": w.cloud,
                "fog": w.fog,
                "heavy_precipitation": w.heavy_precipitation(),
                "ground_wet": sky.ground.wet,
                "puddles": sky.ground.puddles,
                "rise_m": sky.rise,
                "pond_surface_m": ew::surface(PONDS[0].0[0], PONDS[0].0[1]),
                "pond_rest_m": ew::pond_level(0),
                "rain_particles": rain_particles,
                "rain_particle_budget": rain_budget,
                "visible_rain_particles": visible_rain.len(),
                "camera_under_cover": !glade.solids().rain_open(runtime_eye),
                "water_particles": water_particles,
                "ripple_sources": sources,
                "character_wetness": wet_character,
                "offscreen_frame_ms": frame_ms,
                "sha256": digest,
            }));
        }
    }
    let capture = json!({
        "schema": "openagents.verse.everglade_weather_capture.v1",
        "pack": everglade_pack::PACK_SHA256,
        "quality": quality,
        "tick": TICK,
        "pictures": records,
    });
    std::fs::write(
        dir.join("capture.json"),
        serde_json::to_string_pretty(&capture).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Puts the camera `distance` meters behind the player at `pitch`.
fn camera(runtime: &mut WorldRuntime, distance: f32, pitch: f32) {
    runtime.camera = verse::camera::FollowCamera::default();
    runtime.camera.distance = distance;
    runtime.camera.pitch = pitch;
}

/// Stands the player at `at` on the ground facing `toward`.
fn stand(runtime: &mut WorldRuntime, at: Vec3, toward: Vec3) -> Result<(), String> {
    let ground = zones::everglade::height(at.x, at.z);
    let yaw = (toward.x - at.x).atan2(toward.z - at.z);
    runtime.set_spawn(Vec3::new(at.x, ground, at.z), yaw)
}
