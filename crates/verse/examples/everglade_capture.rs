//! Offline visual acceptance of Everglade with the shared renderer.
//! Usage: everglade_capture OUTPUT.png [approach|yard|hall]
//!
//! Installs Everglade from the committed, pinned pack, as a portal entry
//! does after the download, and renders one of three views with the zone
//! HUD:
//!
//! - `approach` (the default): from the stepping stones near the return
//!   portal, up the path through the gate toward the workshop.
//! - `yard`: from above the yard's south edge, over the Task Wall, the
//!   proving ring, and the podium to the hall's facade.
//! - `hall`: inside the hall, over the desks and their monitors toward the
//!   gallery and the hearth.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "approach".into());
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
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    let (at, yaw, tilt) = match view.as_str() {
        "approach" => (glam::Vec3::new(0.0, 0.0, -29.0), 0.0, 0.0),
        "yard" => (glam::Vec3::new(-3.0, 0.0, -15.0), 0.25, 80.0),
        // At the desks station; the camera stays inside, by the doors.
        "hall" => (glam::Vec3::new(0.0, 0.0, 5.0), 0.0, 20.0),
        other => {
            return Err(format!(
                "unknown view `{other}`; use approach, yard, or hall"
            ));
        }
    };
    runtime.set_spawn(at, yaw)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: tilt })?;
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
