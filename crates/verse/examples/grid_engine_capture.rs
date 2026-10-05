//! Renders the Grid from its pinned engine pack through the engine renderer
//! and compares the frame with the legacy line pass.
//! Usage: grid_engine_capture OUTPUT.png [LEGACY.png] [IDLE_SECONDS]
//!
//! Starts the bare world at its spawn, waits IDLE_SECONDS (default 0.5),
//! and renders the same portrait phone frame `bare_capture` renders: the
//! engine frame to OUTPUT.png, the legacy frame to LEGACY.png (default
//! beside OUTPUT.png). Prints the fraction of pixels that differ.
use std::path::PathBuf;

use verse::grid_pack;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let legacy = args
        .next()
        .map_or_else(|| output.with_extension("legacy.png"), PathBuf::from);
    let idle: f32 = args.next().map_or(Ok(0.5), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    let mut runtime = verse::runtime::WorldRuntime::bare();
    let frame = 1.0 / 60.0;
    for _ in 0..(idle / frame).round() as usize {
        runtime.tick(&verse::controller::InputState::default(), frame);
    }
    let (width, height) = (590u32, 1280u32);
    let aspect = width as f32 / height as f32;
    let atlas = verse::ui::Atlas::new(16.0);
    verse::render::capture_with_atmosphere(
        &legacy,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(aspect),
        &runtime.dynamic_mesh(),
        &verse::ui::UiBatch::default(),
        &atlas,
        runtime.atmosphere(),
    )?;
    let comparison = grid_pack::capture::Comparison::render(&runtime, width, height, &atlas)?;
    comparison.write(&output)?;
    let legacy_pixels = grid_pack::capture::read_png(&legacy)?;
    let mismatch = grid_pack::capture::mismatch(&comparison.pixels, &legacy_pixels);
    println!("mismatch {mismatch:.4} of pixels differ from the legacy frame");
    println!("wrote {} and {}", output.display(), legacy.display());
    Ok(())
}
