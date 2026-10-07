//! The Water Lab's hotbar: its six slots in Everglade's tray
//! (`everglade::hotbar::draw_of`), keys `1` to `5` and `B`, and the
//! standard hover card ([`verse_core::tooltip`]) for each slot: the spell's
//! name, what it does after the SRD, and its controls.
//!
//! A click on a slot presses it ([`slot_under`]); the app checks the tray
//! before anything else under the pointer, so a click on the bar never
//! reaches the zone panel behind it.

use verse_core::tooltip::{self, Card, Tip, palette};
use verse_gfx::ui::{Atlas, UiBatch};
use verse_zone_everglade::zones::everglade::hotbar::{self as tray, Slot as TraySlot};

use crate::spells::Slot;

/// How many slots the bar holds.
pub const COUNT: usize = Slot::ALL.len();
/// Each slot's key label.
pub const KEYS: [&str; COUNT] = ["1", "2", "3", "4", "5", "B"];

/// Each slot's name and one sentence on what it does.
#[must_use]
pub fn tip(slot: Slot) -> Tip {
    match slot {
        Slot::WaterWalk => Tip::new(
            "Water Walk",
            "You walk and run on the water's surface as if it were solid ground.",
        ),
        Slot::ControlWater => Tip::new(
            "Control Water",
            "You flood, part, redirect, or whirl the water in a 100-foot cube ahead.",
        ),
        Slot::CreateWater => Tip::new(
            "Create or Destroy Water",
            "Rain falls on a 30-foot cube ahead, or the water there lifts away as vapor.",
        ),
        Slot::SleetStorm => Tip::new(
            "Sleet Storm",
            "Freezing rain fills a 40-foot cylinder ahead and ices the water into slick ground.",
        ),
        Slot::WaterBreathing => Tip::new(
            "Water Breathing",
            "You breathe under water, so you can dive and walk the bed.",
        ),
        Slot::Drop => Tip::new(
            "Drop a float",
            "A crate, a barrel, or a plank falls into the water ahead and floats.",
        ),
    }
}

/// Slot `index`'s hover card: its name, sentence, keys, and rules.
#[must_use]
pub fn card(index: usize) -> Option<Card> {
    let slot = *Slot::ALL.get(index)?;
    let key = KEYS[index];
    let card = Card::of(tip(slot));
    Some(match slot {
        Slot::WaterWalk => card
            .detail(format!("Key {key} turns it on and off"), palette::KEY)
            .detail("1 hour", palette::TIME)
            .detail("No cooldown", palette::TIME),
        Slot::ControlWater => card
            .detail(format!("Key {key} casts it, Flood first"), palette::KEY)
            .detail(format!("Press {key} again for the next mode"), palette::KEY)
            .detail(format!("Shift+{key} ends it"), palette::KEY)
            .detail("Flood, Part, Redirect, Whirlpool", palette::RULE)
            .detail("Concentration, up to 10 minutes", palette::RULE),
        Slot::CreateWater => card
            .detail(format!("Key {key} creates rain"), palette::KEY)
            .detail(format!("Shift+{key} destroys water"), palette::KEY)
            .detail("Instantaneous", palette::TIME),
        Slot::SleetStorm => card
            .detail(
                format!("Key {key} casts it; press again to end it"),
                palette::KEY,
            )
            .detail("Concentration, up to 1 minute", palette::RULE)
            .detail("The ice thaws when it ends", palette::TIME),
        Slot::WaterBreathing => card
            .detail(format!("Key {key} turns it on and off"), palette::KEY)
            .detail("24 hours", palette::TIME)
            .detail("Without it, a breath lasts 1 + Con minutes", palette::RULE),
        Slot::Drop => card
            .detail(format!("Key {key} or 6"), palette::KEY)
            .detail("No cooldown", palette::TIME),
    })
}

/// The slot under `point` on a screen of `size` logical points, with the
/// tray's bottom `bottom` points up, if any.
#[must_use]
pub fn slot_under(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<usize> {
    tray::hit_of(point, size, bottom, COUNT)
}

/// The tray's frame: left, top, width, and height, logical points.
#[must_use]
pub fn frame(size: [f32; 2], bottom: f32) -> [f32; 4] {
    tray::frame_of(size, bottom, COUNT)
}

/// The slot a key presses, as a slot index: `1` to `5`, then `6` or `B`.
#[must_use]
pub fn key(key: char) -> Option<usize> {
    match key {
        '1'..='6' => Some(key as usize - '1' as usize),
        'b' | 'B' => Some(5),
        _ => None,
    }
}

/// Adds every slot's icon to `atlas`, skipping those already there.
///
/// # Errors
/// Names a slot whose icon is missing or does not rasterize.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use verse_content::compiler::icons;
    for slot in Slot::ALL {
        let key = slot.icon();
        if atlas.sprites.contains_key(key) {
            continue;
        }
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// Draws the bar with each slot's state.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, slots: &[TraySlot]) {
    let sprites: Vec<(&str, &str)> = Slot::ALL
        .iter()
        .zip(KEYS)
        .map(|(slot, key)| (slot.icon(), key))
        .collect();
    tray::draw_of(ui, atlas, size, bottom, &sprites, slots);
}

/// Draws slot `index`'s card over the bar, kept on screen.
pub fn draw_tip(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, index: usize) {
    let Some(card) = card(index) else {
        return;
    };
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, COUNT, index);
    let [_, top, _, height] = frame(size, bottom);
    tooltip::draw(ui, atlas, &card, [x, top, w, height], size);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENS: [[f32; 2]; 2] = [[1280.0, 800.0], [390.0, 844.0]];

    /// Every slot has a hover card with a name, one sentence, and its key.
    #[test]
    fn every_slot_has_a_card() {
        for (index, slot) in Slot::ALL.iter().enumerate() {
            let card = card(index).expect("a card");
            assert_eq!(card.title, tip(*slot).name);
            assert!(card.body.ends_with('.'), "{}", card.title);
            assert!(!card.body.trim_end_matches('.').contains(". "));
            assert!(
                card.details
                    .iter()
                    .any(|(text, _)| text.contains(KEYS[index])),
                "{} names no key",
                card.title
            );
        }
        assert!(card(COUNT).is_none());
        let control = card(1).unwrap();
        assert!(
            control
                .details
                .iter()
                .any(|(t, _)| t.contains("again for the next mode"))
        );
        assert!(
            control
                .details
                .iter()
                .any(|(t, _)| t.contains("Shift+2 ends it"))
        );
    }

    /// The pointer finds each slot on the bar, and nothing between them or
    /// above the tray.
    #[test]
    fn the_pointer_finds_each_slot() {
        for size in SCREENS {
            for bottom in [14.0, 120.0] {
                for index in 0..COUNT {
                    let [x, y, w, h] = tray::slot_rect_of(size, bottom, COUNT, index);
                    let center = [x + w * 0.5, y + h * 0.5];
                    assert_eq!(slot_under(center, size, bottom), Some(index));
                    assert_eq!(slot_under([x + w + 1.0, center[1]], size, bottom), None);
                }
                let [left, top, width, _] = frame(size, bottom);
                assert_eq!(
                    slot_under([left + width * 0.5, top - 4.0], size, bottom),
                    None
                );
            }
        }
    }

    #[test]
    fn every_slot_has_an_icon() {
        for slot in Slot::ALL {
            assert!(
                verse_content::compiler::icons::icon(slot.icon()).is_some(),
                "{}",
                slot.icon()
            );
        }
    }

    #[test]
    fn keys_press_their_slots() {
        for (index, key) in ['1', '2', '3', '4', '5', 'b'].into_iter().enumerate() {
            assert_eq!(super::key(key), Some(index));
        }
        assert_eq!(super::key('6'), Some(5));
        assert_eq!(super::key('7'), None);
    }
}
