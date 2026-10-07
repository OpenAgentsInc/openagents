//! The Water Lab as a zone: entry, and every hotbar slot casting its own
//! spell, by key or by click, without ever leaving the zone.

use super::everglade_pack::{self, ZonePack};
use super::water::{self, Slot};
use super::{LoadState, ZoneId};
use crate::controller::InputState;
use crate::runtime::WorldRuntime;
use std::path::Path;
use std::sync::OnceLock;

fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                everglade_pack::PACK_SHA256,
                everglade_pack::PACK_EXTENSION
            ));
        ZonePack::load_local(&path).expect("the committed pack loads")
    })
}

fn lab() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime.install_water_lab(pack());
    assert_eq!(runtime.zone, ZoneId::WaterLab);
    assert_eq!(runtime.zone_load_state(), LoadState::Idle);
    runtime
}

/// Each slot, plain and with Shift, casts the Water Lab's own spell, its
/// state shows on the bar, and the player stays in the lab.
#[test]
fn every_slot_casts_its_spell_and_stays_in_the_lab() {
    let mut runtime = lab();
    let idle = InputState::default();
    for (index, slot) in Slot::ALL.iter().enumerate() {
        for shift in [false, true, false] {
            let line = runtime
                .water_press(index, shift)
                .unwrap_or_else(|e| panic!("{slot:?}: {e}"));
            assert!(!line.is_empty());
            for _ in 0..10 {
                runtime.tick(&idle, 0.02);
            }
            assert_eq!(runtime.zone, ZoneId::WaterLab, "{slot:?} shift {shift}");
            assert!(runtime.water_lab().is_some());
        }
    }
    // Slot 5 is Water Breathing: a press turns it on, another off.
    let breathing = |r: &WorldRuntime| r.water_lab().unwrap().spells.breathing;
    let before = breathing(&runtime);
    runtime.water_press(4, false).unwrap();
    assert_ne!(breathing(&runtime), before);
    assert_eq!(runtime.zone, ZoneId::WaterLab);
    // The bar shows every slot, each with a card.
    let bar = runtime.water_bar().unwrap();
    assert_eq!(bar.len(), water::hotbar::COUNT);
    for index in 0..bar.len() {
        assert!(water::hotbar::card(index).is_some());
    }
    // A slot past the bar is refused, and still leaves nobody.
    assert!(runtime.water_press(water::hotbar::COUNT, false).is_err());
    assert_eq!(runtime.zone, ZoneId::WaterLab);
}

/// A click on each slot's icon finds that slot, as the app's click path
/// asks before the zone panel behind the tray.
#[test]
fn a_click_on_each_slot_presses_it() {
    let mut runtime = lab();
    let size = [1280.0, 800.0];
    let bottom = 14.0;
    for index in 0..water::hotbar::COUNT {
        let [x, y, w, h] =
            super::everglade::hotbar::slot_rect_of(size, bottom, water::hotbar::COUNT, index);
        let hit = water::hotbar::slot_under([x + w * 0.5, y + h * 0.5], size, bottom);
        assert_eq!(hit, Some(index));
        runtime.water_press(hit.unwrap(), false).unwrap();
        assert_eq!(runtime.zone, ZoneId::WaterLab);
    }
}

/// The Water Orb's slot acts on press and release through the runtime, as
/// the app's key and click paths call it: holding grows an orb, letting go
/// throws it, and Shift as it is let go sets it hovering. The dummies stand
/// in the character's figure, and a Thunderbolt strikes.
#[test]
fn the_orb_grows_on_press_and_flies_on_release() {
    let mut runtime = lab();
    let idle = InputState::default();
    let orb = water::hotbar::ORB;
    assert_eq!(Slot::ALL[orb], Slot::WaterOrb);
    runtime.water_press(orb, false).unwrap();
    for _ in 0..60 {
        runtime.tick(&idle, 1.0 / 60.0);
    }
    let size = runtime.water_lab().unwrap().forming().unwrap().radius;
    assert!(size > 1.0, "{size}");
    // No aim from a pointer: it flies ahead.
    runtime.water_aim(1.6, None);
    let line = runtime.water_release(orb, false).unwrap().unwrap();
    assert!(line.contains("hurl"), "{line}");
    assert!(runtime.water_lab().unwrap().forming().is_none());
    // Another, held in place.
    runtime.water_press(orb, false).unwrap();
    for _ in 0..30 {
        runtime.tick(&idle, 1.0 / 60.0);
    }
    runtime.water_release(orb, true).unwrap().unwrap();
    assert!(
        runtime
            .water_lab()
            .unwrap()
            .orbs
            .iter()
            .any(|o| o.hovering())
    );
    // Letting go of another slot does nothing.
    assert_eq!(runtime.water_release(0, false).unwrap(), None);
    // The dummies join the character's figure, and the orbs' water draws.
    let mesh = runtime.dynamic_mesh();
    assert!(!mesh.liquid.is_empty());
    let figure = mesh.figure.expect("a figure");
    let cast = runtime
        .water_lab()
        .unwrap()
        .figure(None)
        .map_or(0, |(_, start)| start);
    assert!(figure.vertices.len() > cast, "the dummies draw");
    // The Thunderbolt strikes ahead.
    let line = runtime.water_press(6, false).unwrap();
    assert!(line.starts_with("Thunderbolt"), "{line}");
}
