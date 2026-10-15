//! Offscreen acceptance for Snap Station in the desktop world runtime.
//! Usage: snap_station_capture DIRECTORY. Builds job one through generic
//! inputs, captures the native PBR view and HUD, and verifies its replay.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{self, Intent},
};
use verse_game::{CommunityGame, GameInput};
fn shot(world: &WorldRuntime, path: &Path) -> Result<(), String> {
    let atlas = verse::ui::Atlas::new(16.);
    let hud = zones::hud::Hud::default();
    let snapshot = hud.snapshot([1280., 800.], &world.zone_snapshot(1.6), true);
    let ui = hud.draw(&atlas, &snapshot, 1.);
    verse::render::capture_with_atmosphere(
        path,
        1280,
        800,
        &world.world.mesh,
        world.view(1.6),
        &world.dynamic_mesh(),
        &ui,
        &atlas,
        world.atmosphere(),
    )
}
fn press(world: &mut WorldRuntime, input: GameInput) {
    world.snap_station_mut().unwrap().input(input);
    world.tick(&InputState::default(), 1. / 60.);
}
fn main() -> Result<(), String> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("Choose a capture folder")?);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut world = WorldRuntime::new();
    world.install_lagrange();
    let gate = zones::snap::arch();
    world.set_spawn(gate.at + glam::Vec3::Z * 3., 0.)?;
    world.set_shot(Some((
        gate.at + glam::Vec3::new(0., 3., 11.),
        gate.at + glam::Vec3::Y * 2.,
    )));
    shot(&world, &out.join("arch.png"))?;
    world.set_shot(None);
    world.set_spawn(gate.at, 0.)?;
    world.zone_intent(Intent::Interact)?;
    shot(&world, &out.join("building.png"))?;
    use GameInput::{Left, Right, Turn, Use};
    for input in [
        Use, Turn, Turn, Turn, Use, Use, Right, Right, Use, Use, Right, Right, Use,
    ] {
        press(&mut world, input);
    }
    shot(&world, &out.join("built.png"))?;
    for input in [Left, Use] {
        press(&mut world, input);
    }
    for _ in 0..10_000 {
        if world.snap_station().unwrap().game.outcome().is_some() {
            break;
        }
        world.tick(&InputState::default(), 1. / 60.);
    }
    let station = world.snap_station().unwrap();
    if station.game.outcome() != Some(verse_game::Outcome::Won) {
        return Err("The native run did not finish".into());
    }
    station
        .replay
        .verify(&mut snap_web::zone::SnapGame::new(snap_rules::level::job(
            1,
        )))?;
    std::fs::write(out.join("run.txt"), station.replay.encode()).map_err(|e| e.to_string())?;
    shot(&world, &out.join("won.png"))?;
    world.zone_intent(Intent::Return)?;
    if world.snap_station().is_some() {
        return Err("The game did not close".into());
    }
    println!(
        "Played job one through CommunityGame, won, verified its rotated build replay, and returned to Lagrange 1."
    );
    Ok(())
}
