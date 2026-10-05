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
//!
//! Then the yard is rebuilt and Meteor Swarm called down on the west
//! cottage, seen from the south:
//!
//! - `target.png`: the targeting circle on the cottage's south front.
//! - `casting.png`: the cast bar, the circle pulsing, and the fire
//!   gathering over it.
//! - `meteors.png`: the meteors falling with their trails.
//! - `impact.png`: the first explosions.
//! - `blast.png`: every meteor down, the cottage blowing apart.
//! - `aftermath.png`: the ruin, the scorch marks, and the other cottage
//!   still standing, a few seconds later.
//!
//! `demolition_capture --tower OUTPUT_DIR` instead enters the Grove and
//! brings down its concrete tower: `tower_*.png` aims Meteor Swarm at the
//! tower's side, shows the crater, the top tipping over its hinge and
//! striking the ground, and the rubble; `bolt_*.png` calls the Thunderbolt
//! down on the rebuilt tower. It prints the frame times of the topple.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1).peekable();
    let tower = args.next_if(|a| a == "--tower").is_some();
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
    if tower {
        return tower_fall(&pack, &dir);
    }
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
    shot(&runtime, &atlas, &dir.join("debris.png"))?;
    meteor_swarm(&mut runtime, &atlas, &dir)
}

/// Rebuilds the yard and calls Meteor Swarm down on the west cottage.
fn meteor_swarm(
    runtime: &mut WorldRuntime,
    atlas: &verse::ui::Atlas,
    dir: &Path,
) -> Result<(), String> {
    use zones::everglade::demolition::meteor;
    let idle = InputState::default();
    // The slowest frame's simulation and dynamic mesh, s.
    let slowest = std::cell::Cell::new(0.0_f64);
    let tick = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / DT).round() as usize {
            let start = std::time::Instant::now();
            runtime.tick(&idle, DT);
            std::hint::black_box(runtime.dynamic_mesh());
            slowest.set(slowest.get().max(start.elapsed().as_secs_f64()));
        }
    };
    runtime.zone_intent(zones::Intent::Rebuild)?;
    runtime.set_spawn(glam::Vec3::new(-14.0, 0.0, -27.0), 0.5)?;
    runtime.apply(Action::Zoom { lines: -4.0 })?;
    // Undo the earlier views' orbits: the camera behind the player again,
    // pitched down a little so the sky shows over the cottages.
    runtime.apply(Action::Orbit {
        dx: 300.0,
        dy: -230.0,
    })?;
    tick(runtime, 0.1);
    runtime.zone_intent(zones::Intent::MeteorSwarm)?;
    // The cursor over the cottage's south front.
    let target = glam::Vec3::new(-6.0, 0.0, -12.5);
    let aspect = 1.6;
    let clip = runtime.view(aspect).view_proj * target.extend(1.0);
    let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
    if !runtime.demolition_aim(aspect, x, y) {
        return Err("The targeting circle found no ground".into());
    }
    tick(runtime, 0.3);
    shot(runtime, atlas, &dir.join("target.png"))?;
    if !runtime.demolition_confirm() {
        return Err("Meteor Swarm did not start its cast".into());
    }
    tick(runtime, meteor::CAST - 0.4);
    shot(runtime, atlas, &dir.join("casting.png"))?;
    tick(runtime, 0.4 + 0.55);
    // Looking up into the sky they fall from.
    runtime.apply(Action::Orbit {
        dx: 0.0,
        dy: -110.0,
    })?;
    tick(runtime, DT);
    shot(runtime, atlas, &dir.join("meteors.png"))?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 110.0 })?;
    tick(runtime, 0.32);
    shot(runtime, atlas, &dir.join("impact.png"))?;
    tick(runtime, 0.75);
    shot(runtime, atlas, &dir.join("blast.png"))?;
    eprintln!("{}", runtime.zone_snapshot(aspect).caption);
    tick(runtime, 4.0);
    eprintln!(
        "Slowest frame of the strike: {:.1} ms of simulation and dynamic mesh",
        slowest.get() * 1000.0
    );
    runtime.set_spawn(glam::Vec3::new(-14.0, 0.0, -26.0), 0.45)?;
    tick(runtime, 0.1);
    shot(runtime, atlas, &dir.join("aftermath.png"))
}

/// The screen point of `at` in a view of `aspect`.
fn screen(runtime: &WorldRuntime, aspect: f32, at: glam::Vec3) -> (f32, f32) {
    let clip = runtime.view(aspect).view_proj * at.extend(1.0);
    (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w)
}

/// The Grove's tower: Meteor Swarm on its side until its top topples,
/// then the Thunderbolt on the rebuilt tower.
fn tower_fall(pack: &everglade_pack::ZonePack, dir: &Path) -> Result<(), String> {
    use zones::everglade::demolition::meteor;
    use zones::grove::layout::TOWER;
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(pack);
    if runtime.zone != zones::ZoneId::Grove {
        return Err("The Grove did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::grove::hotbar::add_sprites(&mut atlas)?;
    let aspect = 1.6;
    let idle = InputState::default();
    let mut times: Vec<f64> = Vec::new();
    let tick = |runtime: &mut WorldRuntime, seconds: f32, times: &mut Vec<f64>| {
        for _ in 0..(seconds / DT).round() as usize {
            let start = std::time::Instant::now();
            runtime.tick(&idle, DT);
            std::hint::black_box(runtime.dynamic_mesh());
            if runtime.toppling().is_some_and(|(t, _)| t > 0) {
                times.push(start.elapsed().as_secs_f64());
            }
        }
    };
    let slot = |spell| {
        zones::grove::slots::slot_of(spell, None, zones::grove::kit::Land::Arid)
            .map(|s| zones::Intent::GroveSlot(s as u8))
            .ok_or("the spell is on the bar")
    };
    // South-east of the tower, looking at its south and east faces, so
    // its top falls across the view.
    let stand = glam::Vec3::new(TOWER[0] + 22.0, 0.0, TOWER[1] - 22.0);
    let facing = (-22.0_f32).atan2(22.0);
    runtime.set_spawn(stand, facing)?;
    runtime.apply(Action::Zoom { lines: -10.0 })?;
    runtime.apply(Action::Orbit {
        dx: 0.0,
        dy: -140.0,
    })?;
    tick(&mut runtime, 0.1, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("tower_before.png"))?;
    let mut casts = 0;
    let mut level = 6.5;
    while runtime.toppling().is_some_and(|(t, _)| t == 0) && casts < 4 {
        runtime.zone_intent(slot(zones::grove::kit::Spell::MeteorSwarm)?)?;
        let side = glam::Vec3::new(TOWER[0] + 0.3, level, TOWER[1] - 2.8);
        let (x, y) = screen(&runtime, aspect, side);
        if !runtime.demolition_aim(aspect, x, y) {
            return Err("The ring found nothing under the cursor".into());
        }
        tick(&mut runtime, 0.2, &mut times);
        if casts == 0 {
            shot_grove_bar(&runtime, &atlas, &dir.join("tower_aim.png"))?;
        }
        if !runtime.demolition_confirm() {
            return Err("Meteor Swarm did not start".into());
        }
        tick(&mut runtime, meteor::CAST - 0.3, &mut times);
        if casts == 0 {
            shot_grove_bar(&runtime, &atlas, &dir.join("tower_casting.png"))?;
        }
        tick(&mut runtime, 0.75, &mut times);
        if casts == 0 {
            shot_grove(&runtime, &atlas, &dir.join("tower_meteors.png"))?;
        }
        tick(&mut runtime, 0.45, &mut times);
        if casts == 0 {
            shot_grove(&runtime, &atlas, &dir.join("tower_impact.png"))?;
        }
        tick(&mut runtime, 0.4, &mut times);
        casts += 1;
        level -= 1.2;
        eprintln!("after cast {casts}: {:?}", runtime.toppling());
    }
    shot_grove(&runtime, &atlas, &dir.join("tower_crater.png"))?;
    for k in 0..12 {
        tick(&mut runtime, 0.25, &mut times);
        shot_grove(&runtime, &atlas, &dir.join(format!("tower_fall_{k}.png")))?;
        if runtime.toppling().is_some_and(|(t, _)| t == 0) {
            break;
        }
    }
    tick(&mut runtime, 0.15, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("tower_crash.png"))?;
    tick(&mut runtime, 0.8, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("tower_dust.png"))?;
    tick(&mut runtime, 5.0, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("tower_aftermath.png"))?;
    runtime.set_spawn(glam::Vec3::new(TOWER[0] - 14.0, 0.0, TOWER[1] - 34.0), 0.35)?;
    tick(&mut runtime, 0.1, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("tower_rubble.png"))?;
    if !times.is_empty() {
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let mut sorted = times.clone();
        sorted.sort_by(f64::total_cmp);
        eprintln!(
            "Topple: {} frames, simulation and dynamic mesh {:.2} ms mean, {:.2} ms at the 95th percentile, {:.2} ms slowest",
            times.len(),
            mean * 1e3,
            sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)] * 1e3,
            sorted[sorted.len() - 1] * 1e3
        );
    }
    eprintln!("chunks: {:?}", runtime.toppling());
    // The Thunderbolt on the rebuilt tower.
    runtime.zone_intent(zones::Intent::Rebuild)?;
    runtime.set_spawn(stand, facing)?;
    tick(&mut runtime, 0.1, &mut times);
    runtime.zone_intent(slot(zones::grove::kit::Spell::Thunderbolt)?)?;
    let side = glam::Vec3::new(TOWER[0] - 0.5, 14.0, TOWER[1] - 2.8);
    let (x, y) = screen(&runtime, aspect, side);
    if !runtime.demolition_aim(aspect, x, y) {
        return Err("The ring found nothing under the cursor".into());
    }
    tick(&mut runtime, 0.2, &mut times);
    shot_grove_bar(&runtime, &atlas, &dir.join("bolt_aim.png"))?;
    runtime.demolition_confirm();
    tick(&mut runtime, meteor::BOLT_CAST + 0.12, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("bolt_strike.png"))?;
    tick(&mut runtime, 0.15, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("bolt_flash.png"))?;
    tick(&mut runtime, 1.5, &mut times);
    shot_grove(&runtime, &atlas, &dir.join("bolt_crater.png"))
}

/// The view with the Grove's bar and the spell's help and cast bar.
fn shot_grove_bar(
    runtime: &WorldRuntime,
    atlas: &verse::ui::Atlas,
    path: &Path,
) -> Result<(), String> {
    grove_view(runtime, atlas, path, true)
}

/// The view alone, the bar hidden so the tower shows whole.
fn shot_grove(runtime: &WorldRuntime, atlas: &verse::ui::Atlas, path: &Path) -> Result<(), String> {
    grove_view(runtime, atlas, path, false)
}

fn grove_view(
    runtime: &WorldRuntime,
    atlas: &verse::ui::Atlas,
    path: &Path,
    bar: bool,
) -> Result<(), String> {
    let (width, height) = (1280, 800);
    let size = [width as f32, height as f32];
    let mut ui = verse::ui::UiBatch::default();
    if let Some(bar) = runtime.grove_bar().filter(|_| bar) {
        let layout = zones::grove::hotbar::Layout::for_screen(size, 0);
        zones::grove::hotbar::draw(&mut ui, atlas, size, 0.0, layout, &bar);
    }
    if let Some(swarm) = runtime.grove_swarm() {
        zones::everglade::demolition::hotbar::draw_aim(&mut ui, atlas, size, &swarm);
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
