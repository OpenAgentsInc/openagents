//! Offline visual acceptance using the same verified pack and renderer as mobile.
use std::path::PathBuf;
use verse::{controller::InputState, runtime::WorldRuntime, zones};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let pack = PathBuf::from(args.next().ok_or("Expected a Ruins pack path")?);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let mut runtime = WorldRuntime::new();
    runtime.install_ruins(zones::assets::LoadedAssets::load_local(&pack)?);
    for _ in 0..24 {
        runtime.tick(&InputState::default(), 0.05);
    }
    let atlas = verse::ui::Atlas::new(16.0);
    let mut hud = zones::hud::Hud::default();
    hud.set_bottom_clearance(0.0)?;
    let snapshot = runtime.zone_snapshot(1.6);
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
