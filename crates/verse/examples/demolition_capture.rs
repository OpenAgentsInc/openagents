//! Offline visual acceptance of the demolition yard (`verse --demolition`).
//! Usage: demolition_capture OUTPUT_DIR [SECONDS]
//!
//! Installs Everglade from the committed, pinned pack as the demolition
//! yard and renders, with the yard's hotbar, into `OUTPUT_DIR`:
//!
//! - `before.png`: the west cottage before anything is hit, the character
//!   carrying the sledgehammer.
//! - `swing.png`: the character's chop into the cottage's south front, at
//!   the blow, seen from the side.
//! - `cracked.png`: the south front after that blow, its number in the
//!   air.
//! - `after.png`: the cottage after the player walks along its west wall
//!   and south front, swinging at each section until it breaks, and the
//!   debris falls for `SECONDS` (two by default).
//! - `debris.png`: the same debris up close.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let seconds: f32 = args
        .next()
        .map(|v| {
            v.parse()
                .map_err(|_| format!("SECONDS is a number, got {v}"))
        })
        .transpose()?
        .unwrap_or(2.0);
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
    runtime.set_demolition(true);
    runtime.install_everglade(&pack);
    if !runtime.in_demolition() {
        return Err("The demolition yard did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::everglade::demolition::hotbar::add_sprites(&mut atlas)?;
    let idle = InputState::default();
    let tick = |runtime: &mut WorldRuntime, frames: usize| {
        for _ in 0..frames {
            runtime.tick(&idle, DT);
        }
    };
    // The view: south-west of the west cottage, looking at its corner.
    let view = |runtime: &mut WorldRuntime| -> Result<(), String> {
        runtime.set_spawn(glam::Vec3::new(-13.5, 0.0, -18.5), 0.55)?;
        runtime.apply(Action::Orbit { dx: 0.0, dy: 40.0 })?;
        tick(runtime, 3);
        Ok(())
    };
    view(&mut runtime)?;
    shot(&runtime, &atlas, &dir.join("before.png"))?;
    // The carried hammer up close, from the character's right.
    runtime.set_spawn(glam::Vec3::new(4.0, 0.0, -24.0), 0.0)?;
    runtime.apply(Action::Orbit {
        dx: -300.0,
        dy: 10.0,
    })?;
    runtime.apply(Action::Zoom { lines: 6.0 })?;
    tick(&mut runtime, 3);
    shot(&runtime, &atlas, &dir.join("carry.png"))?;
    // One blow to the plain section of the south front, seen from the
    // side up close: the wind-up, the blow, and the follow-through.
    runtime.set_spawn(glam::Vec3::new(-5.0, 0.0, -13.95), 0.0)?;
    runtime.zone_intent(zones::Intent::Swing)?;
    for (frames, name) in [(8, "windup.png"), (10, "swing.png"), (12, "follow.png")] {
        tick(&mut runtime, frames);
        shot(&runtime, &atlas, &dir.join(name))?;
    }
    runtime.apply(Action::Zoom { lines: -6.0 })?;
    tick(&mut runtime, 12);
    eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    runtime.set_spawn(glam::Vec3::new(-5.0, 0.0, -18.0), 0.25)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 40.0 })?;
    tick(&mut runtime, 3);
    shot(&runtime, &atlas, &dir.join("cracked.png"))?;
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
            tick(&mut runtime, 60);
        }
        eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    }
    tick(&mut runtime, (seconds / DT) as usize);
    view(&mut runtime)?;
    shot(&runtime, &atlas, &dir.join("after.png"))?;
    runtime.set_spawn(glam::Vec3::new(-8.5, 0.0, -16.5), 0.6)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 60.0 })?;
    tick(&mut runtime, 3);
    shot(&runtime, &atlas, &dir.join("debris.png"))
}

fn shot(runtime: &WorldRuntime, atlas: &verse::ui::Atlas, path: &Path) -> Result<(), String> {
    let (width, height) = (1280, 800);
    let mut ui = verse::ui::UiBatch::default();
    if let Some(bar) = runtime.demolition_bar() {
        zones::everglade::demolition::hotbar::draw(
            &mut ui,
            atlas,
            [width as f32, height as f32],
            0.0,
            &bar,
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
