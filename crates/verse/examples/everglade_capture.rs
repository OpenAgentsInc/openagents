//! Offline visual acceptance of Everglade with the shared renderer.
//! Usage: everglade_capture OUTPUT.png
//!
//! Enters Everglade from the plaza arch and renders the greybox glade from
//! the yard, looking over the station markers toward the workshop outline,
//! with the zone HUD.
use std::path::PathBuf;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones,
};

fn main() -> Result<(), String> {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output PNG path")?,
    );
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(glam::Vec3::new(-24.0, 0.0, -27.0), 0.0)?;
    runtime.zone_intent(zones::Intent::Enter)?;
    // Stand at the yard's south edge facing the hall, and look down over
    // the yard's markers from behind and above.
    runtime.set_spawn(glam::Vec3::new(-3.0, 0.0, -14.0), 0.0)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 80.0 })?;
    let idle = InputState::default();
    for _ in 0..10 {
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
