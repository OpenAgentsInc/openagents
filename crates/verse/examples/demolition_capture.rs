//! Offline visual acceptance of the demolition yard (`verse --demolition`).
//! Usage: demolition_capture BEFORE.png CRACKED.png AFTER.png [SECONDS]
//!
//! Installs Everglade from the committed, pinned pack as the demolition
//! yard and renders the west cottage before anything is hit, then after
//! one blow to its south front. It then walks the player along the
//! cottage's west wall and its south front, swinging the sledgehammer at
//! each section until it breaks, lets the debris fall for `SECONDS` (two
//! by default), and renders the same view.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let before = PathBuf::from(args.next().ok_or("Expected the before PNG path")?);
    let cracked = PathBuf::from(args.next().ok_or("Expected the cracked PNG path")?);
    let after = PathBuf::from(args.next().ok_or("Expected the after PNG path")?);
    let seconds: f32 = args
        .next()
        .map(|v| {
            v.parse()
                .map_err(|_| format!("SECONDS is a number, got {v}"))
        })
        .transpose()?
        .unwrap_or(2.0);
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
    runtime.set_demolition(true);
    runtime.install_everglade(&pack);
    if !runtime.in_demolition() {
        return Err("The demolition yard did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    let idle = InputState::default();
    // The view: south-west of the west cottage, looking at its corner.
    let view = |runtime: &mut WorldRuntime| -> Result<(), String> {
        runtime.set_spawn(glam::Vec3::new(-13.5, 0.0, -18.5), 0.55)?;
        runtime.apply(Action::Orbit { dx: 0.0, dy: 40.0 })?;
        for _ in 0..3 {
            runtime.tick(&idle, DT);
        }
        Ok(())
    };
    view(&mut runtime)?;
    shot(&runtime, &before)?;
    // One blow to the plain section of the south front, and the swing's
    // follow-through.
    runtime.set_spawn(glam::Vec3::new(-5.0, 0.0, -13.95), 0.0)?;
    runtime.zone_intent(zones::Intent::Swing)?;
    for _ in 0..30 {
        runtime.tick(&idle, DT);
    }
    eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    runtime.set_spawn(glam::Vec3::new(-5.0, 0.0, -18.0), 0.25)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 40.0 })?;
    for _ in 0..3 {
        runtime.tick(&idle, DT);
    }
    shot(&runtime, &cracked)?;
    // West wall sections, from the south, then the south front's east half.
    let mut targets: Vec<(glam::Vec3, f32)> = (0..5)
        .map(|i| {
            (
                glam::Vec3::new(-10.95, 0.0, -12.0 + 2.0 * i as f32),
                std::f32::consts::FRAC_PI_2,
            )
        })
        .collect();
    targets.extend([-3.0_f32, -5.0].map(|x| (glam::Vec3::new(x, 0.0, -13.95), 0.0)));
    for (at, yaw) in targets {
        for _ in 0..4 {
            runtime.set_spawn(at, yaw)?;
            runtime.zone_intent(zones::Intent::Swing)?;
            for _ in 0..48 {
                runtime.tick(&idle, DT);
            }
        }
        eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    }
    for _ in 0..(seconds / DT) as usize {
        runtime.tick(&idle, DT);
    }
    view(&mut runtime)?;
    shot(&runtime, &after)
}

fn shot(runtime: &WorldRuntime, path: &Path) -> Result<(), String> {
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    verse::render::capture_with_atmosphere(
        path,
        1280,
        800,
        &runtime.world.mesh,
        runtime.view(1.6),
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
