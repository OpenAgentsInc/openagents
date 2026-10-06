//! Offline captures of the Grove's tower coming down to the ground.
//! Usage: sky_capture OUTPUT_DIR
//!
//! - `stump_*.png`: Meteor Swarm undercuts the tower until its top
//!   topples, the wreck settles on the stump, then Thunderbolts blow the
//!   stump's foot out; nothing may stay in the air where it stood.
//! - `dragon_*.png`: the dragon walks into the tower's south face, seen
//!   from the side so its head shows against the wall, then breathes fire
//!   at point-blank range; the near wall takes it.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack, grove::kit::Spell, grove::layout::TOWER},
};

const DT: f32 = 1.0 / 60.0;
/// Shapechange's slot, Alt+3.
const SHAPECHANGE: u8 = 38;
/// Fire Breath's slot on the dragon's row.
const BREATH: u8 = 13;

fn main() -> Result<(), String> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    stump(&pack, &dir)?;
    dragon(&pack, &dir)
}

fn grove(pack: &everglade_pack::ZonePack) -> Result<WorldRuntime, String> {
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(pack);
    if runtime.zone != zones::ZoneId::Grove {
        return Err("The Grove did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    Ok(runtime)
}

fn tick(runtime: &mut WorldRuntime, seconds: f32, input: &InputState) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(input, DT);
    }
}

fn idle(runtime: &mut WorldRuntime, seconds: f32) {
    tick(runtime, seconds, &InputState::default());
}

/// The screen point of `at` in a view of `aspect`.
fn screen(runtime: &WorldRuntime, aspect: f32, at: glam::Vec3) -> (f32, f32) {
    let clip = runtime.view(aspect).view_proj * at.extend(1.0);
    (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w)
}

fn slot(spell: Spell) -> Result<zones::Intent, String> {
    zones::grove::slots::slot_of(spell, None, zones::grove::kit::Land::Arid)
        .map(|s| zones::Intent::GroveSlot(s as u8))
        .ok_or_else(|| "the spell is on the bar".into())
}

/// Casts `spell`, Meteor Swarm or the Thunderbolt, at `at` on the screen.
fn strike(runtime: &mut WorldRuntime, spell: Spell, at: glam::Vec3) -> Result<(), String> {
    runtime.zone_intent(slot(spell)?)?;
    let (x, y) = screen(runtime, 1.6, at);
    if !runtime.demolition_aim(1.6, x, y) {
        return Err(format!("The ring found nothing at {at}"));
    }
    idle(runtime, 0.2);
    if !runtime.demolition_confirm() {
        return Err("The strike did not start".into());
    }
    Ok(())
}

fn stump(pack: &everglade_pack::ZonePack, dir: &Path) -> Result<(), String> {
    use zones::everglade::demolition::meteor;
    let mut runtime = grove(pack)?;
    // South-west of the tower, looking at its south and west faces.
    let stand = glam::Vec3::new(TOWER[0] - 16.0, 0.0, TOWER[1] - 14.0);
    runtime.set_spawn(stand, 16.0_f32.atan2(14.0))?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: -60.0 })?;
    idle(&mut runtime, 0.1);
    shot(&runtime, &dir.join("stump_0_before.png"))?;
    let mut level = 8.6;
    let mut casts = 0;
    while runtime.toppling().is_some_and(|(t, _)| t == 0) && casts < 5 {
        let side = glam::Vec3::new(TOWER[0] + 0.3, level, TOWER[1] - 2.8);
        strike(&mut runtime, Spell::MeteorSwarm, side)?;
        idle(&mut runtime, meteor::CAST + 1.6);
        level -= 1.2;
        casts += 1;
    }
    idle(&mut runtime, 10.0);
    shot(&runtime, &dir.join("stump_1_settled.png"))?;
    for y in [1.5, 3.0, 4.5, 2.0] {
        let west = glam::Vec3::new(TOWER[0] - 2.8, y, TOWER[1] + 0.5);
        strike(&mut runtime, Spell::Thunderbolt, west)?;
        idle(&mut runtime, meteor::BOLT_CAST + 0.8);
    }
    for (k, wait) in [0.5f32, 1.5, 3.0, 5.0].into_iter().enumerate() {
        idle(&mut runtime, wait);
        shot(&runtime, &dir.join(format!("stump_{}_after.png", k + 2)))?;
    }
    eprintln!("stump: {:?}", runtime.toppling());
    Ok(())
}

fn dragon(pack: &everglade_pack::ZonePack, dir: &Path) -> Result<(), String> {
    let mut runtime = grove(pack)?;
    runtime.set_spawn(glam::Vec3::new(TOWER[0], 0.0, TOWER[1] - 14.0), 0.0)?;
    runtime.zone_intent(zones::Intent::GroveSlot(SHAPECHANGE))?;
    idle(&mut runtime, 2.0);
    let ahead = InputState {
        forward: true,
        ..InputState::default()
    };
    tick(&mut runtime, 4.0, &ahead);
    eprintln!("dragon pressed at {}", runtime.player.pos);
    // From the side, the head against the wall.
    runtime.apply(Action::Orbit {
        dx: -260.0,
        dy: 0.0,
    })?;
    runtime.apply(Action::Zoom { lines: -6.0 })?;
    idle(&mut runtime, 0.1);
    shot(&runtime, &dir.join("dragon_0_pressed.png"))?;
    runtime.zone_intent(zones::Intent::GroveSlot(BREATH))?;
    idle(&mut runtime, 1.0);
    shot(&runtime, &dir.join("dragon_1_breath.png"))?;
    idle(&mut runtime, 2.0);
    shot(&runtime, &dir.join("dragon_2_after.png"))?;
    // From the north, the far wall.
    runtime.set_spawn(glam::Vec3::new(TOWER[0] + 3.0, 0.0, TOWER[1] + 16.0), 3.0)?;
    idle(&mut runtime, 0.1);
    shot(&runtime, &dir.join("dragon_3_far_side.png"))
}

fn shot(runtime: &WorldRuntime, path: &Path) -> Result<(), String> {
    let (width, height) = (1280, 800);
    let atlas = verse::ui::Atlas::new(16.0);
    verse::render::capture_with_atmosphere(
        path,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(1.6),
        &runtime.dynamic_mesh(),
        &verse::ui::UiBatch::default(),
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
