//! Offline visual acceptance of Everglade's destructible town.
//! Usage: town_capture OUTPUT_DIR
//!
//! Installs Everglade from the committed, pinned pack, settles its light,
//! and calls Meteor Swarm down on Main Street's café, seen from the street,
//! rendering with Everglade's hotbar into `OUTPUT_DIR`:
//!
//! - `before.png`: the shops on Main Street, whole.
//! - `target.png`: the targeting circle on the café's front.
//! - `casting.png`: the cast bar and the fire gathering over the circle.
//! - `impact.png`: the first explosions.
//! - `blast.png`: every meteor down, the café blowing apart.
//! - `aftermath.png`: the ruin and the scorch marks, a few seconds later.
//! - `hammer.png`: the sledgehammer swung at the bakery's front.
//! - `restored.png`: the street after `R` restores the town.
//!
//! It prints the median and slowest frame's simulation and dynamic mesh
//! time, and how many figure vertices the ruin draws.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

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
    let idle = InputState::default();
    let frames = std::cell::RefCell::new(Vec::new());
    let tick = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / DT).round().max(1.0) as usize {
            let start = std::time::Instant::now();
            runtime.tick(&idle, DT);
            std::hint::black_box(runtime.dynamic_mesh());
            frames.borrow_mut().push(start.elapsed().as_secs_f64());
        }
    };
    // On Main Street, south-west of the café, looking at its front.
    let ([cx, cz], [_, hz]) = zones::everglade::layout::SHOPS[1];
    let front = glam::Vec3::new(cx, 0.0, cz - hz - 0.4);
    let stand = glam::Vec3::new(cx - 9.0, 0.0, cz - hz - 17.0);
    let yaw = (front.x - stand.x).atan2(front.z - stand.z);
    runtime.set_spawn(stand, yaw)?;
    runtime.apply(Action::Zoom { lines: -3.0 })?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: -60.0 })?;
    tick(&mut runtime, 0.2);
    shot(&runtime, &atlas, &dir.join("before.png"))?;
    runtime.zone_intent(zones::Intent::MeteorSwarm)?;
    let aspect = 1.6;
    let clip = runtime.view(aspect).view_proj * front.extend(1.0);
    let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
    if !runtime.demolition_aim(aspect, x, y) {
        return Err("The targeting circle found no ground".into());
    }
    tick(&mut runtime, 0.3);
    shot(&runtime, &atlas, &dir.join("target.png"))?;
    if !runtime.demolition_confirm() {
        return Err("Meteor Swarm did not start its cast".into());
    }
    let cast = zones::everglade::demolition::meteor::CAST;
    tick(&mut runtime, cast - 0.4);
    shot(&runtime, &atlas, &dir.join("casting.png"))?;
    tick(&mut runtime, 0.4 + 0.87);
    shot(&runtime, &atlas, &dir.join("impact.png"))?;
    tick(&mut runtime, 0.75);
    shot(&runtime, &atlas, &dir.join("blast.png"))?;
    tick(&mut runtime, 4.0);
    shot(&runtime, &atlas, &dir.join("aftermath.png"))?;
    let figure = runtime
        .dynamic_mesh()
        .figure
        .map_or(0, |f| f.vertices.len());
    eprintln!("Figure vertices after the strike: {figure}");
    // The sledgehammer at the bakery's front, seen from the side.
    let ([bx, bz], [_, bhz]) = zones::everglade::layout::SHOPS[0];
    runtime.set_spawn(glam::Vec3::new(bx + 1.0, 0.0, bz - bhz - 1.05), 0.0)?;
    runtime.apply(Action::Orbit {
        dx: -150.0,
        dy: 20.0,
    })?;
    for _ in 0..2 {
        runtime.zone_intent(zones::Intent::Swing)?;
        tick(&mut runtime, 1.1);
    }
    runtime.zone_intent(zones::Intent::Swing)?;
    tick(&mut runtime, 0.32);
    shot(&runtime, &atlas, &dir.join("hammer.png"))?;
    tick(&mut runtime, 1.0);
    shot(&runtime, &atlas, &dir.join("cracked.png"))?;
    let mut times = frames.borrow().clone();
    times.sort_by(f64::total_cmp);
    eprintln!(
        "Frames: median {:.2} ms, slowest {:.1} ms of simulation and dynamic mesh",
        times[times.len() / 2] * 1000.0,
        times.last().copied().unwrap_or(0.0) * 1000.0
    );
    runtime.zone_intent(zones::Intent::Rebuild)?;
    runtime.set_spawn(stand, yaw)?;
    runtime.apply(Action::Orbit { dx: 260.0, dy: 0.0 })?;
    tick(&mut runtime, 0.2);
    shot(&runtime, &atlas, &dir.join("restored.png"))
}

fn shot(runtime: &WorldRuntime, atlas: &verse::ui::Atlas, path: &Path) -> Result<(), String> {
    let (width, height) = (1280, 800);
    let size = [width as f32, height as f32];
    let mut ui = verse::ui::UiBatch::default();
    if let Some(slots) = runtime.everglade_hotbar() {
        zones::everglade::hotbar::draw(&mut ui, atlas, size, 14.0, &slots);
    }
    if let Some(swarm) = runtime.everglade_swarm() {
        zones::everglade::demolition::hotbar::draw_town(
            &mut ui,
            atlas,
            size,
            14.0,
            zones::everglade::hotbar::COUNT,
            &swarm,
        );
    }
    verse::render::capture_with_atmosphere(
        path,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(1.6),
        &runtime.dynamic_mesh(),
        &ui,
        atlas,
        zones::atmosphere(runtime.zone),
    )
}
