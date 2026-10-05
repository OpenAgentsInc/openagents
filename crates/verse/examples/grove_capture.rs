//! Offline visual acceptance of the Grove with the shared renderer.
//! Usage: grove_capture OUTPUT.png [VIEW [ARGS]]
//!
//! Installs the Grove from the committed, pinned Everglade pack, as
//! `verse --grove` does after the download, and renders one view with the
//! Archdruid's four-row bar and the combat log:
//!
//! - `field` (the default): from the spawn over the meadow and its
//!   training dummies.
//! - `action`: the same view a moment after Web and Fire Bolt, with a
//!   bolt in flight, a rooted dummy, and a floating result.
//! - `tooltip`: the field with the pointer resting on Fireball's slot
//!   (Alt+8), so its card shows above the bar.
//! - `phone`: the field at a phone's 844 × 390, where the bar shows one row
//!   and its switcher.
//! - `thunder [AGE]`: Thunderwave mid-blast beside a straw dummy, `AGE`
//!   seconds (0.12 by default) after the cast.
//! - `spider`: the druid in Wild Shape as the Giant Spider, mid-bite on a
//!   straw dummy, with its Bite and Web on row 2.
//! - `spider-walk`: the Giant Spider walking across the meadow, seen from
//!   its side.
//! - `spell SLOT [AGE] [BACK] [SLOT...]`: casts the bar's slot `SLOT`
//!   (row × 12 + column) at the straw dummy from `BACK` m (8 by default),
//!   then any further slots, and renders `AGE` seconds (0.4 by default)
//!   later.
//! - `shapechange [AGE] [ORBIT]`: Shapechange's transformation `AGE`
//!   seconds (0.6 by default) after the cast, from 8 m short of the straw
//!   dummy, the camera orbited `ORBIT` points (220 by default) to see the
//!   druid's face.
//! - `dragon-fly [ORBIT]`: the dragon after taking off and flying across
//!   the meadow, seen from `ORBIT` points around (300 by default).
//! - `breath [AGE] [ORBIT]`: the dragon breathing fire on the dummies,
//!   `AGE` seconds (1.0 by default) after Fire Breath, from 9 m short of
//!   the straw dummy.
//! - `dragon SLOT [AGE] [ORBIT]`: the dragon `AGE` seconds after the
//!   bar's slot `SLOT`, such as Tail Sweep (14) or Roar (16).
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, Intent, everglade_pack, grove::hotbar},
};

/// Shapechange's slot, Alt+3.
const SHAPECHANGE: u8 = 38;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "field".into());
    let rest: Vec<String> = args.collect();
    let number = |i: usize, default: f32| -> Result<f32, String> {
        rest.get(i)
            .map_or(Ok(default), |a| a.parse())
            .map_err(|e| format!("bad number: {e}"))
    };
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
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        let steps = (seconds / 0.01).round() as usize;
        for _ in 0..steps {
            runtime.tick(&InputState::default(), 0.01);
        }
    };
    match view.as_str() {
        "field" | "tooltip" | "phone" => {}
        "thunder" => {
            let age = number(0, 0.12)?;
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -7.5), 0.0)?;
            run(&mut runtime, 0.2);
            runtime.zone_intent(Intent::Thunderwave)?;
            run(&mut runtime, age);
        }
        "action" => {
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -11.0), 0.0)?;
            runtime.zone_intent(Intent::Web)?;
            run(&mut runtime, 1.1);
            runtime.zone_intent(Intent::Firebolt)?;
            run(&mut runtime, 0.15);
        }
        "spider" => {
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -8.0), 0.0)?;
            runtime.zone_intent(Intent::GroveSlot(3))?;
            run(&mut runtime, 0.8);
            runtime.zone_intent(Intent::GroveSlot(12))?;
            run(&mut runtime, 0.3);
        }
        "spider-walk" => {
            runtime.set_spawn(
                glam::Vec3::new(-6.0, 0.0, -12.0),
                std::f32::consts::FRAC_PI_2,
            )?;
            runtime.zone_intent(Intent::GroveSlot(3))?;
            let walk = InputState {
                forward: true,
                ..InputState::default()
            };
            for _ in 0..23 {
                runtime.tick(&walk, 0.05);
            }
            runtime.apply(Action::Orbit {
                dx: 260.0,
                dy: -20.0,
            })?;
            runtime.tick(&walk, 0.02);
        }
        "spell" => {
            let slot = number(0, 17.0)? as u8;
            let age = number(1, 0.4)?;
            let back = number(2, 8.0)?;
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -5.0 - back), 0.0)?;
            run(&mut runtime, 0.2);
            runtime.zone_intent(Intent::GroveSlot(slot))?;
            for extra in rest.iter().skip(3) {
                let extra: u8 = extra.parse().map_err(|e| format!("bad slot: {e}"))?;
                run(&mut runtime, 0.05);
                runtime.zone_intent(Intent::GroveSlot(extra))?;
            }
            run(&mut runtime, age);
        }
        "shapechange" => {
            let age = number(0, 0.6)?;
            let orbit = number(1, 220.0)?;
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -13.0), 0.0)?;
            run(&mut runtime, 0.2);
            runtime.apply(Action::Orbit { dx: orbit, dy: 0.0 })?;
            runtime.zone_intent(Intent::GroveSlot(SHAPECHANGE))?;
            run(&mut runtime, age);
        }
        "dragon-fly" => {
            let orbit = number(0, 300.0)?;
            runtime.set_spawn(glam::Vec3::new(-8.0, 0.0, -22.0), 0.5)?;
            runtime.zone_intent(Intent::GroveSlot(SHAPECHANGE))?;
            run(&mut runtime, 2.0);
            let up = InputState {
                jump: true,
                forward: true,
                ..InputState::default()
            };
            for _ in 0..120 {
                runtime.tick(&up, 0.01);
            }
            let ahead = InputState {
                forward: true,
                ..InputState::default()
            };
            for _ in 0..70 {
                runtime.tick(&ahead, 0.01);
            }
            runtime.apply(Action::Orbit {
                dx: orbit,
                dy: -40.0,
            })?;
            runtime.tick(&ahead, 0.01);
        }
        "breath" | "dragon" => {
            let (slot, rest_at) = if view == "breath" {
                (13, 0)
            } else {
                (number(0, 14.0)? as u8, 1)
            };
            let age = number(rest_at, 1.0)?;
            let orbit = number(rest_at + 1, 120.0)?;
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -14.0), 0.0)?;
            runtime.zone_intent(Intent::GroveSlot(SHAPECHANGE))?;
            run(&mut runtime, 2.0);
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -14.0), 0.0)?;
            runtime.apply(Action::Orbit {
                dx: orbit,
                dy: -20.0,
            })?;
            run(&mut runtime, 0.1);
            runtime.zone_intent(Intent::GroveSlot(slot))?;
            run(&mut runtime, age);
        }
        other => {
            return Err(format!(
                "unknown view `{other}`; use field, action, tooltip, phone, thunder, spider, spider-walk, spell, shapechange, dragon-fly, breath, or dragon"
            ));
        }
    }
    let (width, height) = if view == "phone" {
        (844, 390)
    } else {
        (1280, 800)
    };
    let size = [width as f32, height as f32];
    let mut atlas = verse::ui::Atlas::new(16.0);
    hotbar::add_sprites(&mut atlas)?;
    eprintln!("{}", runtime.zone_snapshot(1.6).caption);
    let mut ui = verse::ui::UiBatch::default();
    if let Some(bar) = runtime.grove_bar() {
        let layout = hotbar::Layout::for_screen(size, 0);
        hotbar::draw(&mut ui, &atlas, size, 14.0, layout, &bar);
        if let Some((status, lines)) = runtime.grove_log() {
            hotbar::draw_log(&mut ui, &atlas, size, 14.0, layout, &status, &lines);
        }
        if view == "tooltip" {
            // A simulated hover on Fireball, Alt+8.
            hotbar::draw_tip(&mut ui, &atlas, size, 14.0, layout, &bar, 43);
        }
    }
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(width as f32 / height as f32),
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
