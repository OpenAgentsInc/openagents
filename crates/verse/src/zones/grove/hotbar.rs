//! The Grove's hotbar: Everglade's icon tray with the druid's spells on
//! keys 1 to 9 and Long Rest on 0. The Grove has no mana and no cooldowns,
//! so the bar has no mana bar and no slot ever dims for one. Resting the
//! pointer on a slot shows its card ([`crate::tooltip`]) with the sentence
//! kept in [`SLOTS`] and the name and level from the kit.

use super::super::Intent;
use super::kit::Spell;
use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// How many slots the bar has.
pub const COUNT: usize = Spell::ALL.len();

/// Each slot's icon sprite and its card's sentence, in [`Spell::ALL`]
/// order. The card's name and level come from [`Spell::def`].
pub const SLOTS: [(&str, &str); COUNT] = [
    (
        "thunderwave-icon",
        "A 15-foot cube of thunder from you deals 2d8 and pushes dummies 10 feet; a Constitution save halves it and holds ground.",
    ),
    (
        "gust-of-wind-icon",
        "A 60-foot line of wind pushes each dummy in it 15 feet away unless it makes a Strength save.",
    ),
    (
        "wind-wall-icon",
        "Raises a wall of wind at the dummy ahead: 4d8 bludgeoning, half on a Strength save, and a failed save is thrown upward, as you are if you walk in.",
    ),
    (
        "wall-of-stone-icon",
        "Raises granite panels at the dummy ahead that shove it out past the wall and block the way.",
    ),
    (
        "reverse-gravity-icon",
        "Gravity flips in a 50-foot cylinder around you, so you and every dummy in it fall upward.",
    ),
    (
        "fire-bolt-icon",
        "Hurls a bolt at the dummy ahead: a spell attack for 4d10 fire, doubled on a natural 20.",
    ),
    (
        "fireball-icon",
        "Throws a bead that bursts on the dummy ahead for 8d6 fire in a 20-foot radius, half on a Dexterity save.",
    ),
    (
        "misty-step-icon",
        "Blink up to 30 feet forward, stopping just short of the dummy ahead.",
    ),
    (
        "web-icon",
        "Fills a 20-foot cube at the dummy ahead with webs that root each one failing a Dexterity save for 12 seconds.",
    ),
    (
        "long-rest-icon",
        "Ends your spells and stands the dummies back up, healed and home.",
    ),
];

/// Each slot's icon sprite, in [`Spell::ALL`] order.
pub const SPRITES: [&str; COUNT] = {
    let mut sprites = [""; COUNT];
    let mut i = 0;
    while i < COUNT {
        sprites[i] = SLOTS[i].0;
        i += 1;
    }
    sprites
};

/// Each slot's key label: 1 to 9, then 0.
pub const KEYS: [&str; COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// What the bar shows: each slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub slots: [Slot; COUNT],
}

/// The intent slot `index` sends.
#[must_use]
pub fn intent(index: usize) -> Option<Intent> {
    Spell::ALL.get(index).map(|s| s.intent())
}

/// The intent a number key (`1` to `9`, `0`) sends.
#[must_use]
pub fn key(digit: u8) -> Option<Intent> {
    match digit {
        1..=9 => intent(usize::from(digit) - 1),
        0 => intent(9),
        _ => None,
    }
}

/// Adds every slot's icon that `atlas` lacks, such as those Everglade's
/// hotbar has not added.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for key in SPRITES {
        if atlas.sprites.contains_key(key) {
            continue;
        }
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// The intent under `point`, if any.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<Intent> {
    tray::hit_of(point, size, bottom, COUNT).and_then(intent)
}

/// The index of the slot under `point`, if any.
#[must_use]
pub fn slot_under(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<usize> {
    tray::hit_of(point, size, bottom, COUNT)
}

/// Slot `index`'s card: the spell's name and sentence, its key, and its
/// level. The Grove has no mana and no cooldowns, so the card names none.
#[must_use]
pub fn card(index: usize) -> Option<Card> {
    let spell = *Spell::ALL.get(index)?;
    let def = spell.def();
    let card = Card::of(Tip::new(def.label, SLOTS[index].1))
        .detail(format!("Key {}", KEYS[index]), palette::KEY);
    let card = if spell == Spell::LongRest {
        card.detail("Demo control", palette::RULE)
    } else if def.level == 0 {
        card.detail("Cantrip", palette::RULE)
    } else {
        card.detail(format!("Level {}", def.level), palette::RULE)
    };
    Some(if spell.glade().is_some() {
        card.detail("Concentration", palette::RULE)
    } else {
        card
    })
}

/// Draws slot `index`'s card above the tray into `ui`, kept on screen.
pub fn draw_tip(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, index: usize) {
    let Some(card) = card(index) else {
        return;
    };
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, COUNT, index);
    let [_, top, _, height] = tray::frame_of(size, bottom, COUNT);
    tooltip::draw(ui, atlas, &card, [x, top, w, height], size);
}

/// Draws the tray into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, bar: &Bar) {
    let sprites: Vec<(&str, &str)> = SPRITES.iter().copied().zip(KEYS).collect();
    tray::draw_of(ui, atlas, size, bottom, &sprites, &bar.slots);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: [f32; 2] = [1280.0, 800.0];

    #[test]
    fn the_pointer_finds_each_slot_and_its_card_names_the_spell() {
        for (index, spell) in Spell::ALL.into_iter().enumerate() {
            let [x, y, w, h] = tray::slot_rect_of(SIZE, 14.0, COUNT, index);
            let at = [x + w * 0.5, y + h * 0.5];
            assert_eq!(slot_under(at, SIZE, 14.0), Some(index));
            assert_eq!(hit(at, SIZE, 14.0), Some(spell.intent()));
            assert_eq!(slot_under([x - 2.0, y + h * 0.5], SIZE, 14.0), None);
            let card = card(index).expect("a card");
            assert_eq!(card.title, spell.def().label);
            let text = SLOTS[index].1;
            assert!(text.ends_with('.') && !text.trim_end_matches('.').contains(". "));
            assert_eq!(card.details[0].0, format!("Key {}", KEYS[index]));
        }
        let thunderwave = card(0).unwrap();
        assert!(thunderwave.details.iter().any(|(t, _)| t == "Level 1"));
        assert!(card(5).unwrap().details.iter().any(|(t, _)| t == "Cantrip"));
        assert!(
            card(2)
                .unwrap()
                .details
                .iter()
                .any(|(t, _)| t == "Concentration")
        );
        // The Grove has no mana and no cooldowns, and no card claims any.
        for index in 0..COUNT {
            let card = card(index).unwrap();
            let words: Vec<String> = card
                .details
                .iter()
                .map(|(t, _)| t.to_lowercase())
                .chain([card.title.to_lowercase(), SLOTS[index].1.to_lowercase()])
                .collect();
            assert!(
                words
                    .iter()
                    .all(|w| !w.contains("mana") && !w.contains("cooldown")),
                "{words:?}"
            );
        }
    }

    #[test]
    fn a_card_clears_the_tray_and_the_screen_edge() {
        let atlas = Atlas::new(14.0);
        let [_, top, _, height] = tray::frame_of(SIZE, 14.0, COUNT);
        for index in 0..COUNT {
            let mut ui = UiBatch::default();
            let [x, _, w, _] = tray::slot_rect_of(SIZE, 14.0, COUNT, index);
            let anchor = [x, top, w, height];
            let rect = tooltip::draw(&mut ui, &atlas, &card(index).unwrap(), anchor, SIZE);
            assert!(rect[1] + rect[3] <= top, "slot {index}");
            assert!(rect[0] >= tooltip::MARGIN);
            assert!(rect[0] + rect[2] <= SIZE[0] - tooltip::MARGIN);
        }
    }
}
