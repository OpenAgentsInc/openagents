//! Offline visual acceptance of the Physics Lab with the shared renderer.
//! Usage: lab_capture OUTPUT.png [SCENARIO 1-9] [SECONDS]
//!
//! Enters the lab from the plaza arch, presses **+** on the scenario knob to
//! reach SCENARIO, runs SECONDS of simulated time (default 2.5), and renders
//! the stage from beside it with the zone HUD.
use std::path::PathBuf;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones,
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let scenario: usize = args
        .next()
        .map_or(Ok(1), |s| s.parse())
        .map_err(|_| "SCENARIO must be a number from 1 to 9")?;
    let seconds: f32 = args
        .next()
        .map_or(Ok(2.5), |s| s.parse())
        .map_err(|_| "SECONDS must be a number")?;
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -25.0), 0.0)?;
    runtime.zone_intent(zones::Intent::Enter)?;
    for _ in 1..scenario.clamp(1, 9) {
        runtime.zone_intent(zones::Intent::Increase)?;
    }
    let idle = InputState::default();
    // Stand beside the stage so the character does not hide it, and look
    // across the stage from above.
    runtime.set_spawn(glam::Vec3::new(4.8, 0.0, 0.5), 0.0)?;
    runtime.apply(Action::Orbit {
        dx: 110.0,
        dy: 90.0,
    })?;
    runtime.apply(Action::Zoom { lines: 2.0 })?;
    for _ in 0..(seconds / 0.05).round() as usize {
        runtime.tick(&idle, 0.05);
    }
    let atlas = verse::ui::Atlas::new(16.0);
    let mut hud = zones::hud::Hud::default();
    hud.set_bottom_clearance(0.0)?;
    let snapshot = runtime.zone_snapshot(1.6);
    eprintln!("{}", snapshot.caption);
    let ui = hud.draw(&atlas, &hud.snapshot([1280.0, 800.0], &snapshot, true), 1.0);
    verse::render::capture_with_atmosphere(
        &output,
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
