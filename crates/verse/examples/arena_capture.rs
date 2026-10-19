//! Offline visual acceptance of the Grid Arena with the shared renderer.
//! Usage: arena_capture OUTPUT.png [SECONDS]
//!
//! Enters the arena as `verse --arena` does, lets the Brute close in for
//! SECONDS (default 2.6) while the Striker fires its Laser, raises its
//! Shield, and launches a Missile Salvo, then renders the duel mid-flight
//! with the arena's HUD: both health bars, the floating numbers, and the
//! action bar with its cooldowns.
use std::path::PathBuf;
use verse::{controller::InputState, runtime::WorldRuntime, zones};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let seconds: f32 = args
        .next()
        .map_or(Ok(2.6), |s| s.parse())
        .map_err(|_| "SECONDS must be a number")?;
    let mut runtime = WorldRuntime::new();
    runtime.enter_arena()?;
    let idle = InputState::default();
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    let frame = 1.0 / 60.0;
    let frames = (seconds / frame).round() as usize;
    for i in 0..frames {
        let input = if i < frames / 2 { &walk } else { &idle };
        match i {
            40 => {
                let _ = runtime.arena_press(0);
            }
            41 => {
                let _ = runtime.arena_press(2);
            }
            110 => {
                let _ = runtime.arena_press(1);
            }
            _ => {}
        }
        runtime.tick(input, frame);
    }
    let (width, height) = (1600u32, 1000u32);
    let aspect = width as f32 / height as f32;
    let atlas = verse::ui::Atlas::new(16.0);
    let mut ui = verse::ui::UiBatch::default();
    let view = runtime.view(aspect);
    let arena = runtime.arena().ok_or("The arena did not open")?;
    zones::arena::hud::draw(
        &mut ui,
        &atlas,
        [width as f32, height as f32],
        view.view_proj,
        arena,
        zones::arena::hud::CONTROLS,
    );
    eprintln!("{}", runtime.zone_snapshot(aspect).caption);
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &runtime.world.mesh,
        view,
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
