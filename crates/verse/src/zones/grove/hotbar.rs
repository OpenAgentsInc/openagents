//! The Grove's hotbar: Everglade's icon tray with the druid's spells on
//! keys 1 to 9, Wild Shape: Giant Spider on 0, Return to Form on `-`, and
//! Long Rest on `=`. In the spider's shape its Bite and Web take the first
//! two slots ([`super::shape::Form::attacks`]). The Grove has no mana and
//! no cooldowns, so the bar has no mana bar and no slot ever dims for one.
//! Resting the pointer on a slot shows its card ([`crate::tooltip`]) with
//! the sentence kept in [`info`] and the name and level from the kit.
//!
//! A slot sends [`Intent::GroveSlot`], and the Grove casts what the slot
//! holds when the press arrives.

use super::super::Intent;
use super::kit::Spell;
use super::shape::Form;
use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// How many slots the bar has.
pub const COUNT: usize = 12;

/// The bar in the druid's own shape.
pub const ROW: [Spell; COUNT] = [
    Spell::Thunderwave,
    Spell::GustOfWind,
    Spell::WindWall,
    Spell::WallOfStone,
    Spell::ReverseGravity,
    Spell::FireBolt,
    Spell::Fireball,
    Spell::MistyStep,
    Spell::Web,
    Spell::WildShapeSpider,
    Spell::ReturnToForm,
    Spell::LongRest,
];

/// Each slot's key label.
pub const KEYS: [&str; COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="];

/// What slot `index` casts while the druid wears `form`.
#[must_use]
pub fn spell(index: usize, form: Option<Form>) -> Option<Spell> {
    match form {
        Some(form) if index < 2 => Some(form.attacks()[index]),
        _ => ROW.get(index).copied(),
    }
}

/// A spell's icon sprite and its card's sentence. The card's name and
/// level come from [`Spell::def`].
#[must_use]
pub const fn info(spell: Spell) -> (&'static str, &'static str) {
    match spell {
        Spell::Thunderwave => (
            "thunderwave-icon",
            "A 15-foot cube of thunder from you deals 2d8 and pushes dummies 10 feet; a Constitution save halves it and holds ground.",
        ),
        Spell::GustOfWind => (
            "gust-of-wind-icon",
            "A 60-foot line of wind pushes each dummy in it 15 feet away unless it makes a Strength save.",
        ),
        Spell::WindWall => (
            "wind-wall-icon",
            "Raises a wall of wind at the dummy ahead: 4d8 bludgeoning, half on a Strength save, and a failed save is thrown upward, as you are if you walk in.",
        ),
        Spell::WallOfStone => (
            "wall-of-stone-icon",
            "Raises granite panels at the dummy ahead that shove it out past the wall and block the way.",
        ),
        Spell::ReverseGravity => (
            "reverse-gravity-icon",
            "Gravity flips in a 50-foot cylinder around you, so you and every dummy in it fall upward.",
        ),
        Spell::FireBolt => (
            "fire-bolt-icon",
            "Hurls a bolt at the dummy ahead: a spell attack for 4d10 fire, doubled on a natural 20.",
        ),
        Spell::Fireball => (
            "fireball-icon",
            "Throws a bead that bursts on the dummy ahead for 8d6 fire in a 20-foot radius, half on a Dexterity save.",
        ),
        Spell::MistyStep => (
            "misty-step-icon",
            "Blink up to 30 feet forward, stopping just short of the dummy ahead.",
        ),
        Spell::Web => (
            "web-icon",
            "Fills a 20-foot cube at the dummy ahead with webs that root each one failing a Dexterity save for 12 seconds.",
        ),
        Spell::WildShapeSpider => (
            "wild-shape-spider-icon",
            "Become a Giant Spider, a quarter faster than you run, with its bite and web on the bar; your spells still cast.",
        ),
        Spell::ReturnToForm => (
            "return-to-form-icon",
            "Drop the beast's shape and stand as the druid again.",
        ),
        Spell::LongRest => (
            "long-rest-icon",
            "Ends your spells and your beast's shape and stands the dummies back up, healed and home.",
        ),
        Spell::Bite => (
            "spider-bite-icon",
            "Bite the dummy at your fangs: a +5 attack for 1d8 + 3 piercing and 2d6 poison.",
        ),
        Spell::SpiderWeb => (
            "spider-web-icon",
            "Spit a web at a dummy up to 60 feet away: a +5 attack that roots it for 6 seconds.",
        ),
    }
}

/// What the bar shows: each slot's spell and state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub spells: [Spell; COUNT],
    pub slots: [Slot; COUNT],
}

/// The intent slot `index` sends.
#[must_use]
pub fn intent(index: usize) -> Option<Intent> {
    (index < COUNT).then(|| Intent::GroveSlot(index as u8))
}

/// The slot a key selects: `1` to `9`, `0`, `-`, and `=`.
#[must_use]
pub fn key_slot(key: char) -> Option<usize> {
    match key {
        '1'..='9' => Some(key as usize - '1' as usize),
        '0' => Some(9),
        '-' => Some(10),
        '=' => Some(11),
        _ => None,
    }
}

/// The intent a key sends: `1` to `9`, `0`, `-`, and `=`.
#[must_use]
pub fn key(key: char) -> Option<Intent> {
    key_slot(key).and_then(intent)
}

/// Adds every slot's icon that `atlas` lacks, such as those Everglade's
/// hotbar has not added.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for spell in Spell::ALL {
        let key = info(spell).0;
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

/// The card for `spell` on slot `index`: the spell's name and sentence,
/// its key, and its level. The Grove has no mana and no cooldowns, so the
/// card names none.
#[must_use]
pub fn card_of(spell: Spell, index: usize) -> Card {
    let def = spell.def();
    let card = Card::of(Tip::new(def.label, info(spell).1));
    let card = match KEYS.get(index) {
        Some(key) => card.detail(format!("Key {key}"), palette::KEY),
        None => card,
    };
    let card = if spell == Spell::LongRest {
        card.detail("Demo control", palette::RULE)
    } else if spell.beast() {
        card.detail("Giant Spider", palette::RULE)
    } else if !spell.spell() {
        card.detail("Wild Shape", palette::RULE)
    } else if def.level == 0 {
        card.detail("Cantrip", palette::RULE)
    } else {
        card.detail(format!("Level {}", def.level), palette::RULE)
    };
    if spell.glade().is_some() {
        card.detail("Concentration", palette::RULE)
    } else {
        card
    }
}

/// Slot `index`'s card on `bar`.
#[must_use]
pub fn card(bar: &Bar, index: usize) -> Option<Card> {
    Some(card_of(*bar.spells.get(index)?, index))
}

/// Draws slot `index`'s card above the tray into `ui`, kept on screen.
pub fn draw_tip(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    bar: &Bar,
    index: usize,
) {
    let Some(card) = card(bar, index) else {
        return;
    };
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, COUNT, index);
    let [_, top, _, height] = tray::frame_of(size, bottom, COUNT);
    tooltip::draw(ui, atlas, &card, [x, top, w, height], size);
}

/// Draws the tray into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, bar: &Bar) {
    let sprites: Vec<(&str, &str)> = bar.spells.iter().map(|s| info(*s).0).zip(KEYS).collect();
    tray::draw_of(ui, atlas, size, bottom, &sprites, &bar.slots);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: [f32; 2] = [1280.0, 800.0];

    fn bar(form: Option<Form>) -> Bar {
        Bar {
            spells: std::array::from_fn(|i| spell(i, form).unwrap()),
            slots: [Slot {
                enabled: true,
                active: false,
                cooldown: 0.0,
            }; COUNT],
        }
    }

    #[test]
    fn the_pointer_finds_each_slot_and_its_card_names_the_spell() {
        let bar = bar(None);
        for (index, spell) in ROW.into_iter().enumerate() {
            let [x, y, w, h] = tray::slot_rect_of(SIZE, 14.0, COUNT, index);
            let at = [x + w * 0.5, y + h * 0.5];
            assert_eq!(slot_under(at, SIZE, 14.0), Some(index));
            assert_eq!(hit(at, SIZE, 14.0), Some(Intent::GroveSlot(index as u8)));
            assert_eq!(slot_under([x - 2.0, y + h * 0.5], SIZE, 14.0), None);
            let card = card(&bar, index).expect("a card");
            assert_eq!(card.title, spell.def().label);
            assert_eq!(card.details[0].0, format!("Key {}", KEYS[index]));
        }
        assert!(
            card(&bar, 0)
                .unwrap()
                .details
                .iter()
                .any(|(t, _)| t == "Level 1")
        );
        assert!(
            card(&bar, 5)
                .unwrap()
                .details
                .iter()
                .any(|(t, _)| t == "Cantrip")
        );
        assert!(
            card(&bar, 2)
                .unwrap()
                .details
                .iter()
                .any(|(t, _)| t == "Concentration")
        );
        // The Grove has no mana and no cooldowns, and no card claims any.
        for spell in Spell::ALL {
            let card = card_of(spell, 0);
            let text = info(spell).1;
            assert!(text.ends_with('.') && !text.trim_end_matches('.').contains(". "));
            let words: Vec<String> = card
                .details
                .iter()
                .map(|(t, _)| t.to_lowercase())
                .chain([card.title.to_lowercase(), text.to_lowercase()])
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
    fn keys_reach_every_slot_and_the_spider_puts_its_attacks_first() {
        for (index, label) in KEYS.into_iter().enumerate() {
            let key = label.chars().next().unwrap();
            assert_eq!(key_slot(key), Some(index));
            assert_eq!(super::key(key), Some(Intent::GroveSlot(index as u8)));
        }
        assert_eq!(key_slot('a'), None);
        let spider = bar(Some(Form::GiantSpider));
        assert_eq!(spider.spells[0], Spell::Bite);
        assert_eq!(spider.spells[1], Spell::SpiderWeb);
        assert_eq!(spider.spells[2..], ROW[2..]);
        assert!(
            card(&spider, 0)
                .unwrap()
                .details
                .iter()
                .any(|(t, _)| t == "Giant Spider")
        );
    }

    #[test]
    fn a_card_clears_the_tray_and_the_screen_edge() {
        let atlas = Atlas::new(14.0);
        let bar = bar(None);
        let [_, top, _, height] = tray::frame_of(SIZE, 14.0, COUNT);
        for index in 0..COUNT {
            let mut ui = UiBatch::default();
            let [x, _, w, _] = tray::slot_rect_of(SIZE, 14.0, COUNT, index);
            let anchor = [x, top, w, height];
            let rect = tooltip::draw(&mut ui, &atlas, &card(&bar, index).unwrap(), anchor, SIZE);
            assert!(rect[1] + rect[3] <= top, "slot {index}");
            assert!(rect[0] >= tooltip::MARGIN);
            assert!(rect[0] + rect[2] <= SIZE[0] - tooltip::MARGIN);
        }
    }
}
