//! Offline visual acceptance of the shared camera-collision step.
//! Usage: camera_capture OUT_DIR
//!
//! Renders the views where an orbit would otherwise stand behind a wall:
//!
//! - `crypt-spawn.png`: the crypt's first frame at its spawn, before any
//!   tick, with the door's wall behind the player.
//! - `everglade-wall.png`: the player with its back to a kit house's long
//!   wall in Everglade.
//! - `grid-wall.png`: the player with its back to one of the Grid Gym's low
//!   walls, which the camera sees over.
//! - `plaza-tower.png`: the player with its back to the plaza's tallest
//!   tower.
//!
//! It prints each view's eye and whether a solid holds it in.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::controller::{Footprint, InputState};
use verse::runtime::WorldRuntime;
use verse::zones::{self, everglade_pack};

const SIZE: (u32, u32) = (1280, 800);

fn main() -> Result<(), String> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;

    // The crypt's very first frame.
    let mut crypt = WorldRuntime::new();
    crypt.install_crypt(&pack);
    if crypt.zone != zones::ZoneId::Crypt {
        return Err("The crypt did not install".into());
    }
    shoot(&crypt, &out.join("crypt-spawn.png"))?;

    // Everglade, backed onto a house wall.
    let mut everglade = WorldRuntime::new();
    everglade.install_everglade(&pack);
    if everglade.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install".into());
    }
    let (wall, _) = zones::everglade::layout::city::kit_blocks()
        .into_iter()
        .find(|(f, top)| {
            let (wide, deep) = (f.max[0] - f.min[0], f.max[1] - f.min[1]);
            let ground = zones::everglade::height(0.5 * (f.min[0] + f.max[0]), f.max[1]);
            wide > 4.0 && deep < 1.0 && *top - ground > 2.5
        })
        .ok_or("No kit house has a long wall")?;
    back_onto(&mut everglade, &wall)?;
    shoot(&everglade, &out.join("everglade-wall.png"))?;

    // The Grid, backed onto a Gym wall.
    let mut grid = WorldRuntime::bare();
    let wall = grid.world.blockers[0];
    back_onto(&mut grid, &wall)?;
    shoot(&grid, &out.join("grid-wall.png"))?;

    // The plaza, backed onto its biggest tower.
    let mut plaza = WorldRuntime::new();
    let tower = plaza
        .world
        .blockers
        .iter()
        .copied()
        .filter(|f| f.max[0] - f.min[0] > 4.0 && f.max[1] - f.min[1] > 4.0)
        .max_by(|a, b| {
            let area = |f: &Footprint| (f.max[0] - f.min[0]) * (f.max[1] - f.min[1]);
            area(a).total_cmp(&area(b))
        })
        .ok_or("The plaza has no tower")?;
    back_onto(&mut plaza, &tower)?;
    shoot(&plaza, &out.join("plaza-tower.png"))?;
    Ok(())
}

/// Stands the player 0.6 m off `wall`'s far side, facing away from it, and
/// settles the camera for a second.
fn back_onto(runtime: &mut WorldRuntime, wall: &Footprint) -> Result<(), String> {
    let thin_z = wall.max[1] - wall.min[1] < wall.max[0] - wall.min[0];
    let (cx, cz) = (
        0.5 * (wall.min[0] + wall.max[0]),
        0.5 * (wall.min[1] + wall.max[1]),
    );
    let (at, yaw) = if thin_z {
        (Vec3::new(cx, 0.0, wall.max[1] + 0.6), 0.0)
    } else {
        (Vec3::new(wall.max[0] + 0.6, 0.0, cz), PI / 2.0)
    };
    runtime.set_spawn(at, yaw)?;
    for _ in 0..60 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    Ok(())
}

fn shoot(runtime: &WorldRuntime, path: &Path) -> Result<(), String> {
    let aspect = SIZE.0 as f32 / SIZE.1 as f32;
    let framing = runtime.framing();
    println!(
        "{}: player {:.2}, eye {:.2}, held in {}, avatar hidden {}",
        path.display(),
        runtime.player.pos,
        framing.eye,
        framing.limited,
        runtime.hides_avatar()
    );
    let atlas = verse::ui::Atlas::new(16.0);
    verse::render::capture_with_atmosphere(
        path,
        SIZE.0,
        SIZE.1,
        &runtime.world.mesh,
        runtime.view(aspect),
        &runtime.dynamic_mesh(),
        &verse::ui::UiBatch::default(),
        &atlas,
        runtime.atmosphere(),
    )
}
