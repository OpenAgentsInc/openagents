//! Captures the standalone castle after a chosen number of simulated seconds.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{self, everglade_pack},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let age: f32 = args
        .next()
        .unwrap_or_else(|| "4.2".into())
        .parse()
        .map_err(|e| format!("Invalid age: {e}"))?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&path)?;
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_stress_test(&pack);
    if runtime.zone != zones::ZoneId::MeteorStressTest {
        return Err("Meteor Stress Test failed to install".into());
    }
    for _ in 0..(age * 60.0) as usize {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::everglade::hotbar::add_sprites(&mut atlas)?;
    let mut ui = verse::ui::UiBatch::default();
    if let Some(bar) = runtime.everglade_hotbar() {
        zones::everglade::hotbar::draw(&mut ui, &atlas, [1280.0, 800.0], 14.0, &bar);
    }
    ui.text(
        &atlas,
        20.0,
        20.0,
        "Meteor Stress Test · 5 casters · R: rebuild · 6: Meteor Swarm",
        [1.0, 0.8, 0.4, 1.0],
    );
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
