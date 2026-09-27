//! Offline visual acceptance of the Lagrange 1 station with the shared renderer.
//! Usage: lagrange_capture OUTPUT.png [spawn|jig|carry|sun]
use std::path::PathBuf;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones,
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "spawn".into());
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(glam::Vec3::new(12.0, 0.0, 9.0), 0.0)?;
    runtime.zone_intent(zones::Intent::Enter)?;
    let idle = InputState::default();
    match view.as_str() {
        "jig" | "carry" => {
            let forward = InputState {
                forward: true,
                ..Default::default()
            };
            runtime.apply(Action::Orbit { dx: 0.0, dy: 120.0 })?;
            for _ in 0..(14 * 20) {
                runtime.tick(&forward, 0.05);
            }
            if view == "carry" {
                runtime
                    .navigate_to([-9.5, -4.0])
                    .map_err(|e| e.to_string())?;
                for _ in 0..(60 * 20) {
                    runtime.tick(&idle, 0.05);
                }
                runtime.player.yaw = -std::f32::consts::FRAC_PI_2;
                runtime.tick(&idle, 0.05);
                let _ = runtime.zone_intent(zones::Intent::Grab);
            }
            runtime.apply(Action::Orbit {
                dx: 250.0,
                dy: -60.0,
            })?;
            runtime.apply(Action::Zoom { lines: -6.0 })?;
        }
        "sun" | "earth" => {
            runtime
                .navigate_to([45.0, 30.0])
                .map_err(|e| e.to_string())?;
            for _ in 0..(80 * 20) {
                runtime.tick(&idle, 0.05);
            }
            runtime.player.yaw = if view == "sun" {
                std::f32::consts::PI
            } else {
                0.0
            };
            runtime.apply(Action::Look { dx: 0.0, dy: -40.0 })?;
            runtime.apply(Action::Zoom { lines: 8.0 })?;
        }
        _ => runtime.apply(Action::Orbit { dx: 90.0, dy: 0.0 })?,
    }
    for _ in 0..4 {
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
