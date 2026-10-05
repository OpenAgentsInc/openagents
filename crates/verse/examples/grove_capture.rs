//! Offline visual acceptance of the Grove with the shared renderer.
//! Usage: grove_capture OUTPUT.png [field|action|tooltip]
//!
//! Installs the Grove from the committed, pinned Everglade pack, as
//! `verse --grove` does after the download, and renders one view with its
//! hotbar:
//!
//! - `field` (the default): from the spawn over the meadow and its
//!   training dummies.
//! - `action`: the same view a moment after Web and Fire Bolt, with a
//!   bolt in flight, a rooted dummy, and a floating result.
//! - `tooltip`: the field with the pointer resting on the hotbar's
//!   Fireball slot, so its card shows over the mana bar.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, Intent, everglade_pack},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "field".into());
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
    runtime.install_grove(&pack);
    if runtime.zone != zones::ZoneId::Grove {
        return Err("The Grove did not install from the pinned pack".into());
    }
    runtime.settle_zone_light();
    runtime.apply(Action::Orbit { dx: 0.0, dy: 60.0 })?;
    let idle = InputState::default();
    for _ in 0..10 {
        runtime.tick(&idle, 0.05);
    }
    match view.as_str() {
        "field" | "tooltip" => {}
        "action" => {
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -11.0), 0.0)?;
            runtime.zone_intent(Intent::Web)?;
            for _ in 0..22 {
                runtime.tick(&idle, 0.05);
            }
            runtime.zone_intent(Intent::Firebolt)?;
            for _ in 0..3 {
                runtime.tick(&idle, 0.05);
            }
        }
        other => {
            return Err(format!(
                "unknown view `{other}`; use field, action, or tooltip"
            ));
        }
    }
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::grove::hotbar::add_sprites(&mut atlas)?;
    eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    let mut ui = verse::ui::UiBatch::default();
    if let Some(bar) = runtime.grove_bar() {
        zones::grove::hotbar::draw(&mut ui, &atlas, [1280.0, 800.0], 14.0, &bar);
        if view == "tooltip" {
            // A simulated hover on Fireball, the seventh slot.
            zones::grove::hotbar::draw_tip(&mut ui, &atlas, [1280.0, 800.0], 14.0, 6);
        }
    }
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
