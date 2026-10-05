//! Offline visual acceptance of aiming a strike through a hole.
//! Usage: aim_capture OUTPUT_DIR [grove|town]
//!
//! Blows a hole in a wall with a strike, as a player does, then aims
//! from the same spot through the hole at the inside of the far wall, and
//! renders into `OUTPUT_DIR`:
//!
//! - `PLACE-hole.png`: the hole, after the first strike.
//! - `PLACE-aim.png`: the ring on the far wall's inner face, seen through
//!   the hole.
//! - `PLACE-strike.png`: the next strike bursting on that face.
//!
//! `grove` (the default) cuts a crater in the south face of the Grove's
//! solid concrete tower with a Thunderbolt and aims at the blocks inside it; `town` blasts the south wall of
//! Everglade's lane cottage with Meteor Swarm.
use glam::Vec3;
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, Intent, everglade_pack},
};

const DT: f32 = 1.0 / 60.0;
const ASPECT: f32 = 1.6;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let place = args.next().unwrap_or_else(|| "grove".into());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
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
    // The wall's face, the point through the hole on the far wall, where
    // the caster stands, and the slot that aims the strike.
    let (face, far, stand, aim): (Vec3, Vec3, Vec3, Intent) = match place.as_str() {
        "grove" => {
            runtime.install_grove(&pack);
            let [x, z] = zones::grove::layout::TOWER;
            let slot = zones::grove::slots::slot_of(
                zones::grove::kit::Spell::Thunderbolt,
                None,
                zones::grove::kit::Land::Arid,
            )
            .ok_or("Meteor Swarm is on the bar")?;
            (
                Vec3::new(x, 6.0, z - 2.8),
                Vec3::new(x, 6.2, z + 1.0),
                Vec3::new(x - 2.0, 0.0, z - 12.0),
                Intent::GroveSlot(slot as u8),
            )
        }
        "town" => {
            runtime.install_everglade(&pack);
            let ([cx, cz], [_, hz]) = zones::everglade::layout::COTTAGE;
            let base = zones::everglade::height(cx, cz);
            (
                Vec3::new(cx, base + 1.6, cz - hz),
                Vec3::new(cx + 0.5, base + 1.6, cz + hz),
                Vec3::new(cx - 2.0, 0.0, cz - hz - 11.0),
                Intent::MeteorSwarm,
            )
        }
        other => return Err(format!("unknown place `{other}`; use grove or town")),
    };
    runtime.settle_zone_light();
    let yaw = (face.x - stand.x).atan2(face.z - stand.z);
    runtime.set_spawn(stand, yaw)?;
    runtime.apply(Action::Zoom { lines: -3.0 })?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: -40.0 })?;
    let tick = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / DT).round().max(1.0) as usize {
            runtime.tick(&InputState::default(), DT);
        }
    };
    tick(&mut runtime, 0.2);
    let strike = |runtime: &mut WorldRuntime, at: Vec3| -> Result<(), String> {
        runtime.zone_intent(aim)?;
        let clip = runtime.view(ASPECT).view_proj * at.extend(1.0);
        let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
        if !runtime.demolition_aim(ASPECT, x, y) {
            return Err("The ring found no surface".into());
        }
        Ok(())
    };
    strike(&mut runtime, face)?;
    if !runtime.demolition_confirm() {
        return Err("The strike did not start".into());
    }
    tick(&mut runtime, 8.0);
    shot(&runtime, &dir.join(format!("{place}-hole.png")))?;
    strike(&mut runtime, far)?;
    tick(&mut runtime, 0.3);
    shot(&runtime, &dir.join(format!("{place}-aim.png")))?;
    if !runtime.demolition_confirm() {
        return Err("The strike did not start again".into());
    }
    tick(
        &mut runtime,
        zones::everglade::demolition::meteor::CAST + 1.25,
    );
    shot(&runtime, &dir.join(format!("{place}-strike.png")))
}

fn shot(runtime: &WorldRuntime, path: &Path) -> Result<(), String> {
    let (width, height) = (1280, 800);
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    verse::render::capture_with_atmosphere(
        path,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(ASPECT),
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
