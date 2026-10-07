//! Offline pictures of Everglade's interactive water (`docs/verse/water.md`,
//! phase W6): wakes, foam trails, splashes, floating debris, and a rowed
//! boat.
//!
//! Usage: everglade_wake_capture OUTPUT_DIRECTORY
//!
//! Installs Everglade from the committed, pinned pack and renders with one
//! offscreen renderer that draws every few frames, so the ripple and foam
//! field builds up its trails as it does in play. Into `OUTPUT_DIRECTORY`:
//!
//! - `swim-wake.png`: the player swimming across Lantern Pond, with its
//!   Kelvin wake and foam trail behind it.
//! - `boat-jetty.png`: on Lantern Pond's jetty beside its rowboat.
//! - `boat-rowed.png`: the player rowing the boat out across the pond, its
//!   wake spreading behind.
//! - `boat-capsized.png`: the boat rolled past its limit, floating keel up
//!   with the player swimming beside it.
//! - `bridge-debris.png`: the footbridge broken, its planks floating down
//!   Glade Run with their splashes and wakes.
//!
//! It writes `capture.json`: each picture's place, the ripple field's
//! sources, the water's live effect particles, and the picture's SHA-256.
//! `VERSE_QUALITY` (`low`, `medium`, `high`) picks the tier.

use std::path::{Path, PathBuf};

use glam::Vec3;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse::{
    controller::InputState,
    render::Offscreen,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};
use verse_world::social::everglade_water::{self as ew, PONDS};
use verse_world::water::medium;

const DT: f32 = 1.0 / 60.0;
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
/// The renderer draws one frame in this many, stepping the ripple field.
const EVERY: usize = 2;

struct Shooter {
    off: Offscreen,
    atlas: verse::ui::Atlas,
    dir: PathBuf,
    records: Vec<Value>,
}

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
    let off = Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(runtime.zone),
    )?;
    let mut shooter = Shooter {
        off,
        atlas,
        dir,
        records: Vec::new(),
    };
    let stroke = InputState {
        forward: true,
        ..InputState::default()
    };

    // Swimming across Lantern Pond, seen from behind and above.
    let ([cx, cz], _) = PONDS[0];
    let float = ew::surface(cx, cz).unwrap() - medium::FLOAT_DEPTH as f32;
    runtime.set_spawn(
        Vec3::new(cx - 3.5, float, cz + 0.5),
        std::f32::consts::FRAC_PI_2,
    )?;
    // Under the dive pitch, so the stroke stays on the surface.
    camera(&mut runtime, 8.0, 0.45);
    shooter.run(&mut runtime, &stroke, 2.2)?;
    shooter.shot(&mut runtime, "swim-wake")?;

    // On the jetty beside Lantern Pond's boat.
    let boat = runtime
        .everglade_afloat_mut()
        .ok_or("Everglade has no boats")?
        .fleet
        .frame(0);
    let keel = boat.0.as_vec3();
    let jetty = jetty_by(&mut runtime, keel)?;
    let toward = keel - jetty;
    runtime.set_spawn(jetty, toward.x.atan2(toward.z))?;
    camera(&mut runtime, 6.0, 0.45);
    runtime.apply(Action::Orbit { dx: 60.0, dy: 0.0 })?;
    shooter.run(&mut runtime, &InputState::default(), 0.6)?;
    shooter.shot(&mut runtime, "boat-jetty")?;

    // Board and row out across the pond.
    let line = runtime.boat_interact().ok_or("no boat in reach")?;
    eprintln!("{line}");
    camera(&mut runtime, 11.0, 0.95);
    shooter.run(&mut runtime, &stroke, 2.8)?;
    shooter.shot(&mut runtime, "boat-rowed")?;

    // Rolled past its limit, it capsizes and throws the player in.
    if let Some(afloat) = runtime.everglade_afloat_mut() {
        afloat.fleet.roll(0, 12.0);
    }
    shooter.run(&mut runtime, &InputState::default(), 3.0)?;
    camera(&mut runtime, 6.5, 0.5);
    shooter.run(&mut runtime, &InputState::default(), 0.4)?;
    shooter.shot(&mut runtime, "boat-capsized")?;

    // The footbridge broken over Glade Run.
    let ([bx, bz], _) = zones::everglade::layout::BRIDGE;
    // From the bank downstream, looking back up the run at the bridge.
    let run = ew::run();
    let below = run.locate(bx, bz).along + 7.0;
    let [rx, rz] = run.point_at(below);
    let [tx, tz] = run.tangent_at(below);
    let off = run.half_at(below) + ew::BANK + 1.0;
    let bank = Vec3::new(rx + tz * off, 0.0, rz - tx * off);
    stand(&mut runtime, bank, Vec3::new(bx, 0.0, bz))?;
    shooter.run(&mut runtime, &InputState::default(), 0.3)?;
    break_bridge(&mut runtime)?;
    // The broken deck leaves the static cells: draw them again.
    shooter.off = Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &shooter.atlas,
        zones::atmosphere(runtime.zone),
    )?;
    camera(&mut runtime, 9.0, 0.7);
    shooter.run(&mut runtime, &InputState::default(), 12.0)?;
    shooter.shot(&mut runtime, "bridge-debris")?;

    let capture = json!({
        "schema": "openagents.verse.everglade_wake_capture.v1",
        "pack": everglade_pack::PACK_SHA256,
        "quality": std::env::var("VERSE_QUALITY").unwrap_or_else(|_| "auto".into()),
        "pictures": shooter.records,
    });
    std::fs::write(
        shooter.dir.join("capture.json"),
        serde_json::to_string_pretty(&capture).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())
}

impl Shooter {
    /// Ticks `seconds` with `input`, drawing one frame in [`EVERY`].
    fn run(
        &mut self,
        runtime: &mut WorldRuntime,
        input: &InputState,
        seconds: f32,
    ) -> Result<(), String> {
        let ui = verse::ui::UiBatch::default();
        for i in 0..(seconds / DT).round().max(1.0) as usize {
            runtime.tick(input, DT);
            if i % EVERY == 0 {
                self.off
                    .render(runtime.view(1.6), &runtime.dynamic_mesh(), &ui)?;
            }
        }
        Ok(())
    }

    /// Draws the frame to `dir/name.png` and records it.
    fn shot(&mut self, runtime: &mut WorldRuntime, name: &str) -> Result<(), String> {
        runtime.tick(&InputState::default(), DT);
        let size = [WIDTH as f32, HEIGHT as f32];
        let mut ui = verse::ui::UiBatch::default();
        if let Some(slots) = runtime.everglade_hotbar() {
            zones::everglade::hotbar::draw(&mut ui, &self.atlas, size, 14.0, &slots);
        }
        let dynamic = runtime.dynamic_mesh();
        let pixels = self.off.render(runtime.view(1.6), &dynamic, &ui)?;
        let path = self.dir.join(format!("{name}.png"));
        let file = std::fs::File::create(&path)
            .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&pixels))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let water = dynamic.neon.as_ref().and_then(|n| n.water);
        let sources = water.map_or(0, |w| w.sources().len());
        let hulls = water.map_or(0, |w| w.hull_count);
        let glade = runtime.everglade_zone_mut();
        let particles = glade
            .as_ref()
            .and_then(|g| g.water_fx())
            .map_or(0, |fx| fx.len());
        let boat = glade.and_then(|g| g.afloat()).map(|a| {
            let (keel, _) = a.fleet.frame(0);
            json!({
                "keel": keel.to_array(),
                "state": a.fleet.boats[0].state.name(),
                "speed": a.fleet.speed(0),
                "draft": a.fleet.draft(0),
            })
        });
        let p = runtime.player.pos;
        eprintln!(
            "{name}: player at {p}, {sources} ripple sources, {particles} water particles, {hulls} hulls"
        );
        self.records.push(json!({
            "name": name,
            "file": format!("{name}.png"),
            "player": p.to_array(),
            "ripple_sources": sources,
            "water_particles": particles,
            "hulls": hulls,
            "boat": boat,
            "sha256": digest,
        }));
        Ok(())
    }
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
    runtime.set_spawn(Vec3::new(at.x, ground, at.z), yaw)?;
    runtime.camera = verse::camera::FollowCamera::default();
    Ok(())
}

/// The point on the jetty deck nearest the boat at `keel`.
fn jetty_by(runtime: &mut WorldRuntime, keel: Vec3) -> Result<Vec3, String> {
    let glade = runtime
        .everglade_zone_mut()
        .ok_or("Everglade is not installed")?;
    let solids = glade.solids();
    let top = ew::surface(keel.x, keel.z).ok_or("the boat is not on the water")?;
    let dock = zones::everglade::layout::placements()
        .into_iter()
        .filter(|p| p.model == "generated/dock")
        .min_by(|p, q| {
            let d = |p: &zones::everglade::layout::Placement| {
                (p.at[0] - keel.x).hypot(p.at[1] - keel.z)
            };
            d(p).total_cmp(&d(q))
        })
        .ok_or("no jetty")?;
    let along = verse::controller::forward(dock.yaw);
    (-30..=30)
        .map(|i| {
            let x = dock.at[0] + along.x * i as f32 * 0.1;
            let z = dock.at[1] + along.z * i as f32 * 0.1;
            Vec3::new(x, solids.floor(x, z, top + 1.6), z)
        })
        .filter(|p| p.y > top + 0.2)
        .min_by(|p, q| {
            let d = |p: &Vec3| (p.x - keel.x).hypot(p.z - keel.z);
            d(p).total_cmp(&d(q))
        })
        .ok_or_else(|| "no deck by the boat".to_owned())
}

/// Breaks every piece of Glade Run's footbridge, as a meteor would.
fn break_bridge(runtime: &mut WorldRuntime) -> Result<(), String> {
    let ([bx, bz], _) = zones::everglade::layout::BRIDGE;
    let glade = runtime
        .everglade_zone_mut()
        .ok_or("Everglade is not installed")?;
    let town = glade.town_mut().ok_or("the town is not raised")?;
    let bridge = town
        .buildings()
        .iter()
        .position(|b| {
            let ([cx, cz], [hx, hz]) = b.rect;
            (bx - cx).abs() <= hx + 0.5 && (bz - cz).abs() <= hz + 0.5 && b.destructible()
        })
        .ok_or("the footbridge is not a building")?;
    town.blast(
        Vec3::new(bx, zones::everglade::height(bx, bz) + 0.6, bz),
        3.0,
        0,
        Vec3::Z,
    );
    let doomed: Vec<usize> = town
        .refs()
        .iter()
        .enumerate()
        .filter(|(_, (b, _))| *b == bridge)
        .map(|(i, _)| i)
        .collect();
    for i in doomed {
        let at = town.site().specs()[i].center;
        town.site_mut().damage(i, 100_000, at, glam::DVec3::Y * 0.2);
    }
    Ok(())
}
