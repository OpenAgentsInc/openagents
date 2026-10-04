//! Offline visual acceptance of a Rust Native panel over the plaza.
//! Usage: panel_capture OUTPUT.png [transcript|changes] [SCALE]
//!
//! Renders the plaza at its spawn, 1280 by 800 points at SCALE pixels a
//! point (default 2), with the sample seat panel docked on the right,
//! showing its transcript (the default) or its diff. The panel is painted
//! by `rust-native-desktop` in the chat palette and composited over the
//! amber world exactly as the window draws it.
use std::path::PathBuf;
use verse::panels::{self, Intent, Tab};
use verse::runtime::WorldRuntime;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let tab = match args.next().as_deref() {
        None | Some("transcript") => Tab::Transcript,
        Some("changes") => Tab::Changes,
        Some(other) => return Err(format!("{other} is not transcript or changes")),
    };
    let scale: f32 = args.next().map_or(Ok(2.0), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    let (width, height) = ((1280.0 * scale) as u32, (800.0 * scale) as u32);
    let runtime = WorldRuntime::new();
    let mut panel = panels::sample();
    panel.apply(Intent::Show(tab));
    panel.set_focus(true);
    let image = panel.image([width, height], scale)?.clone();
    let atlas = verse::ui::Atlas::new(14.0 * scale);
    verse::render::capture_with_overlay(
        &output,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(width as f32 / height as f32),
        &runtime.dynamic_mesh(),
        &verse::ui::UiBatch::default(),
        &atlas,
        runtime.atmosphere(),
        Some(&image),
    )
}
