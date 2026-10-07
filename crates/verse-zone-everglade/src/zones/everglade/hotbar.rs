//! Everglade's hotbar, drawn as the chamber's action bar is: a beveled tray
//! of game-icons.net art with a number key on each slot. Levitate comes
//! first, held to rise, then the spells that need no enemy
//! ([`super::spells`]), none with a cooldown. Nothing on Everglade's bar
//! damages a building, a creature, or a player. Meteor Swarm, the
//! Thunderbolt, and the sledgehammer, which break buildings
//! ([`super::demolition::town`]), follow in [`SLOTS`] for the Meteor
//! Stress Test's castle, and for Everglade's town only in a
//! `dev-destruction` build with that test switched on
//! ([`DEV_DESTRUCTION`]), where the bar is [`DEV_ORDER`]: Meteor Swarm on
//! 1, the Thunderbolt on 2, and Levitate on 3. The runtime chooses which
//! slots show and in what order. The icons are sprites in the HUD's atlas, added by
//! [`add_sprites`]. Resting the pointer on a slot, or holding a touch on it,
//! shows its card ([`crate::tooltip`]) with the name and sentence kept in
//! [`SLOTS`].

use super::super::Intent;
use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};

/// How many slots the bar has: Levitate and the four utility spells.
pub const COUNT: usize = 5;

/// Whether this build carries the dev-only destruction slots.
pub const DEV_DESTRUCTION: bool = cfg!(feature = "dev-destruction");

/// How many slots the bar has where destruction is on: [`COUNT`], then
/// Meteor Swarm, the Thunderbolt, and the sledgehammer.
pub const FULL_COUNT: usize = COUNT + 3;

/// Everglade's bar where destruction is on, as indices into [`SLOTS`]:
/// Meteor Swarm, the Thunderbolt, Levitate, the four utility spells, and
/// the sledgehammer last.
pub const DEV_ORDER: [usize; FULL_COUNT] = [5, 6, 0, 1, 2, 3, 4, 7];

/// One slot: the intent it sends, its icon sprite, and its card's name and
/// sentence. Number keys press them in displayed order. The first
/// [`COUNT`] are Everglade's bar; the last three show only where
/// destruction is on.
pub const SLOTS: [(Intent, &str, Tip); FULL_COUNT] = [
    (
        Intent::Levitate,
        "levitate-icon",
        Tip::new(
            "Levitate",
            "Hold to rise up to 18 m over the ground and let go to hover there; tap while hovering to fall, or hold X to sink.",
        ),
    ),
    (
        Intent::FeatherFall,
        "feather-fall-icon",
        Tip::new(
            "Feather Fall",
            "Cast while falling to slow your descent to a gentle drift until you land; press again to end it.",
        ),
    ),
    (
        Intent::WindWall,
        "wind-wall-icon",
        Tip::new(
            "Wind Wall",
            "Raises another 30-foot wall of wind 4 m ahead whose updraft throws you upward when you walk into it.",
        ),
    ),
    (
        Intent::ReverseGravity,
        "reverse-gravity-icon",
        Tip::new(
            "Reverse Gravity",
            "Gravity flips in a 50-foot cylinder around you: you fall upward and hover near its top until you press again.",
        ),
    ),
    (
        Intent::WallOfStone,
        "wall-of-stone-icon",
        Tip::new(
            "Wall of Stone",
            "Raises another pair of granite panels 4 m ahead that block your way for ten minutes.",
        ),
    ),
    (
        Intent::MeteorSwarm,
        "meteor-swarm-icon",
        Tip::new(
            "Meteor Swarm",
            "Calls down six blazing meteors on a circle of ground you choose, blasting apart the town's buildings they reach; R restores the town.",
        ),
    ),
    (
        Intent::Thunderbolt,
        "thunderbolt-icon",
        Tip::new(
            "Thunderbolt",
            "Calls one huge bolt of lightning down on the wall or ground you choose, blasting a hole through the building it strikes; R restores the town.",
        ),
    ),
    (
        Intent::Swing,
        "sledgehammer-icon",
        Tip::new(
            "Sledgehammer",
            "Swing a two-handed sledgehammer at the building ahead: each blow cracks it, and at zero hit points it breaks and drops what it held up.",
        ),
    ),
];

/// The slots shown: the first `count` of [`SLOTS`], at most all of them.
#[must_use]
pub fn shown(count: usize) -> &'static [(Intent, &'static str, Tip)] {
    &SLOTS[..count.min(FULL_COUNT)]
}

/// The identity order of a bar of `count` slots: each shown slot in
/// [`SLOTS`] order.
#[must_use]
pub fn in_order(count: usize) -> Vec<usize> {
    (0..shown(count).len()).collect()
}

/// The intent of the slot that number key `n` presses in a tray whose
/// displayed slot `i` is `SLOTS[order[i]]`.
#[must_use]
pub fn key_intent(n: usize, order: &[usize]) -> Option<Intent> {
    let index = *order.get(n.checked_sub(1)?)?;
    SLOTS.get(index).map(|(intent, ..)| *intent)
}

/// Whether a slot can be used now, whether its toggle or spell is on, and
/// the fraction of its cooldown left (always 0 in Everglade; the Grove's
/// bar shares this type).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slot {
    pub enabled: bool,
    pub active: bool,
    pub cooldown: f32,
}

/// The chamber bar's units (`imported::overlay`): 36-unit icons 42 apart in
/// a tray 52 tall whose bottom sits 8 above the screen's, all scaled by the
/// screen's height over 768.
const ICON: f32 = 36.0;
const STEP: f32 = 42.0;
const PAD: f32 = 8.0;
const MARGIN: f32 = 8.0;

/// Units to logical points: the chamber's height over 768, never smaller
/// than one point a unit.
pub fn unit(size: [f32; 2]) -> f32 {
    (size[1] / 768.0).max(1.0)
}

/// Adds every slot's icon to `atlas`.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for (_, key, _) in SLOTS {
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    // The Exhaustion debuff beside the breath bar.
    super::water::add_sprites(atlas)
}

/// The tray's frame in logical points for a screen of `size` and a bar of
/// [`COUNT`] slots: centered at the bottom as the chamber's bar is, raised
/// `bottom` points more (to clear a phone's sticks). [`frame_of`] lays out
/// a bar of another length.
#[must_use]
pub fn frame(size: [f32; 2], bottom: f32) -> [f32; 4] {
    frame_of(size, bottom, COUNT)
}

/// The frame of a tray of `count` slots, as [`frame`] lays it out.
#[must_use]
pub fn frame_of(size: [f32; 2], bottom: f32, count: usize) -> [f32; 4] {
    let u = unit(size);
    let width = (2.0 * PAD + STEP * (count.max(1) as f32 - 1.0) + ICON) * u;
    let height = (ICON + 2.0 * PAD) * u;
    [
        (size[0] - width) * 0.5,
        size[1] - bottom - MARGIN * u - height,
        width,
        height,
    ]
}

/// Slot `index`'s icon as left, top, width, and height, in a tray of
/// `count` slots.
#[must_use]
pub fn slot_rect_of(size: [f32; 2], bottom: f32, count: usize, index: usize) -> [f32; 4] {
    let ([x, y], icon) = slot_at(size, frame_of(size, bottom, count), index);
    [x, y, icon, icon]
}

/// The index of the slot under `point` in a bar of `count` slots, if any.
#[must_use]
pub fn slot_under(point: [f32; 2], size: [f32; 2], bottom: f32, count: usize) -> Option<usize> {
    hit_of(point, size, bottom, shown(count).len())
}

/// Slot `index`'s card: its name and sentence, its keys, and for a spell
/// that it has no cooldown.
#[must_use]
pub fn card(index: usize) -> Option<Card> {
    card_with_key(index, index)
}

fn card_with_key(index: usize, displayed: usize) -> Option<Card> {
    let (intent, _, tip) = SLOTS.get(index)?;
    let key = if *intent == Intent::Levitate {
        format!("Hold {} or L", displayed + 1)
    } else {
        format!("Key {}", displayed + 1)
    };
    let mut card = Card::of(*tip).detail(key, palette::KEY);
    if super::spells::Spell::of(*intent).is_some() {
        card = card.detail("No cooldown", palette::TIME);
    }
    if *intent == Intent::MeteorSwarm {
        use super::demolition::meteor;
        card = card
            .detail("No mana, no cooldown", palette::MANA)
            .detail(format!("{} s cast", meteor::CAST), palette::TIME)
            .detail(format!("{:.0} m range", meteor::RANGE), palette::RULE);
    }
    if *intent == Intent::Thunderbolt {
        use super::demolition::meteor;
        card = card
            .detail("No mana, no cooldown", palette::MANA)
            .detail(format!("{} s cast", meteor::BOLT_CAST), palette::TIME)
            .detail(format!("{:.0} m range", meteor::RANGE), palette::RULE);
    }
    Some(card)
}

/// Draws displayed slot `index`'s card over a tray of `count` slots in
/// [`SLOTS`] order into `ui`, kept on screen.
pub fn draw_tip(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    count: usize,
    index: usize,
) {
    draw_tip_ordered(ui, atlas, size, bottom, index, &in_order(count));
}

/// Draws a card with its displayed key in a reordered tray, whose
/// displayed slot `i` is `SLOTS[order[i]]`.
pub fn draw_tip_ordered(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    index: usize,
    order: &[usize],
) {
    let count = order.len().min(FULL_COUNT);
    if index >= count {
        return;
    }
    let Some(card) = card_with_key(order[index], index) else {
        return;
    };
    let [x, _, w, _] = slot_rect_of(size, bottom, count, index);
    let [_, top, _, height] = frame_of(size, bottom, count);
    tooltip::draw(ui, atlas, &card, [x, top, w, height], size);
}

/// A slot's top-left corner and the icon's edge length.
fn slot_at(size: [f32; 2], frame: [f32; 4], index: usize) -> ([f32; 2], f32) {
    let u = unit(size);
    (
        [
            frame[0] + (PAD + STEP * index as f32) * u,
            frame[1] + PAD * u,
        ],
        ICON * u,
    )
}

/// The intent under `point` in a bar of `count` slots in [`SLOTS`] order,
/// if any.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32, count: usize) -> Option<Intent> {
    hit_ordered(point, size, bottom, &in_order(count))
}

/// The intent under `point` in a tray whose displayed slot `i` is
/// `SLOTS[order[i]]`, if any.
#[must_use]
pub fn hit_ordered(
    point: [f32; 2],
    size: [f32; 2],
    bottom: f32,
    order: &[usize],
) -> Option<Intent> {
    let index = hit_of(point, size, bottom, order.len().min(FULL_COUNT))?;
    SLOTS.get(order[index]).map(|(intent, ..)| *intent)
}

/// The index of the slot under `point` in a tray of `count` slots.
#[must_use]
pub fn hit_of(point: [f32; 2], size: [f32; 2], bottom: f32, count: usize) -> Option<usize> {
    let frame = frame_of(size, bottom, count);
    (0..count).find(|&index| {
        let ([x, y], icon) = slot_at(size, frame, index);
        point[0] >= x && point[0] <= x + icon && point[1] >= y && point[1] <= y + icon
    })
}

/// Draws the tray with `slots` (in [`SLOTS`] order, one per shown slot)
/// into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, slots: &[Slot]) {
    draw_ordered(ui, atlas, size, bottom, slots, &in_order(slots.len()));
}

/// Draws a tray with its icons in `order` (displayed slot `i` is
/// `SLOTS[order[i]]`) and `slots`' states in displayed order.
pub fn draw_ordered(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    slots: &[Slot],
    order: &[usize],
) {
    let order = &order[..order.len().min(slots.len()).min(FULL_COUNT)];
    let keys: Vec<String> = (1..=order.len()).map(|n| n.to_string()).collect();
    let sprites: Vec<(&str, &str)> = order
        .iter()
        .map(|&index| &SLOTS[index])
        .zip(&keys)
        .map(|((_, sprite, _), key)| (*sprite, key.as_str()))
        .collect();
    draw_of(ui, atlas, size, bottom, &sprites, slots);
}

/// Draws a tray of `sprites`, each an icon sprite and its key's label,
/// with `slots` in the same order, into `ui`.
pub fn draw_of(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    sprites: &[(&str, &str)],
    slots: &[Slot],
) {
    let frame = frame_of(size, bottom, sprites.len());
    let [left, top, width, height] = frame;
    let u = unit(size);
    for (inset, color) in [
        (0.0, [0.08, 0.07, 0.06, 0.96]),
        (1.0, [0.46, 0.39, 0.24, 1.0]),
        (2.0, [0.19, 0.17, 0.13, 1.0]),
        (4.0, [0.035, 0.03, 0.025, 0.96]),
    ] {
        ui.rect(
            atlas,
            left + inset * u,
            top + inset * u,
            width - inset * u * 2.0,
            height - inset * u * 2.0,
            color,
        );
    }
    for (index, ((sprite, key), slot)) in sprites.iter().zip(slots).enumerate() {
        let ([x, y], icon) = slot_at(size, frame, index);
        let tint = if slot.enabled {
            [1.0; 4]
        } else {
            [0.32, 0.32, 0.32, 1.0]
        };
        ui.image_region(
            atlas,
            sprite,
            [x, y, icon, icon],
            [0.0, 1.0, 0.0, 1.0],
            tint,
        );
        ui.cooldown(atlas, [x, y, icon], slot.cooldown);
        let (edge, width) = if slot.active {
            ([0.98, 0.82, 0.38, 1.0], 2.0)
        } else {
            ([0.55, 0.45, 0.28, 1.0], 1.0)
        };
        ui.frame(atlas, x - 1.0, y - 1.0, icon + 2.0, icon + 2.0, width, edge);
        ui.text(
            atlas,
            x + icon - 3.0 - atlas.advance * key.len() as f32,
            y + 1.0,
            key,
            [0.75, 0.75, 0.75, 1.0],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::demolition::meteor;
    use super::*;

    const DESKTOP: [f32; 2] = [1280.0, 800.0];
    const PHONE: [f32; 2] = [390.0, 844.0];

    fn center([x, y, w, h]: [f32; 4]) -> [f32; 2] {
        [x + w * 0.5, y + h * 0.5]
    }

    #[test]
    fn the_pointer_finds_each_slot_and_none_between_them() {
        for (size, bottom) in [(DESKTOP, 14.0), (PHONE, 120.0)] {
            for index in 0..COUNT {
                let rect = slot_rect_of(size, bottom, COUNT, index);
                assert_eq!(slot_under(center(rect), size, bottom, COUNT), Some(index));
                assert_eq!(hit(center(rect), size, bottom, COUNT), Some(SLOTS[index].0));
                // Just past the icon's right edge is the gap to the next.
                let gap = [rect[0] + rect[2] + 1.0, rect[1] + rect[3] * 0.5];
                assert_eq!(slot_under(gap, size, bottom, COUNT), None);
            }
            let [left, top, width, _] = frame(size, bottom);
            assert_eq!(
                slot_under([left + width * 0.5, top - 4.0], size, bottom, COUNT),
                None
            );
        }
    }

    #[test]
    fn every_slot_has_a_name_and_one_sentence() {
        for (index, (intent, _, tip)) in SLOTS.iter().enumerate() {
            assert!(!tip.name.is_empty(), "{intent:?}");
            assert!(tip.text.ends_with('.'), "{}", tip.name);
            assert!(
                !tip.text.trim_end_matches('.').contains(". "),
                "{} has more than one sentence",
                tip.name
            );
            let card = card(index).expect("a card");
            assert_eq!(card.title, tip.name);
            assert!(card.details[0].0.contains(&(index + 1).to_string()));
        }
        assert!(card(FULL_COUNT).is_none());
        // Wind Wall's card says what walking into it does now.
        let wind = SLOTS
            .iter()
            .position(|(intent, ..)| *intent == Intent::WindWall)
            .unwrap();
        assert!(SLOTS[wind].2.text.contains("throws you upward"));
    }

    #[test]
    fn the_bar_is_movement_and_utility_spells_on_keys_1_to_5() {
        let bar: Vec<Intent> = shown(COUNT).iter().map(|(i, ..)| *i).collect();
        assert_eq!(
            bar,
            [
                Intent::Levitate,
                Intent::FeatherFall,
                Intent::WindWall,
                Intent::ReverseGravity,
                Intent::WallOfStone,
            ]
        );
        for (n, intent) in bar.iter().enumerate() {
            assert_eq!(key_intent(n + 1, &in_order(COUNT)), Some(*intent));
        }
        assert_eq!(key_intent(0, &in_order(COUNT)), None);
        assert_eq!(key_intent(6, &in_order(COUNT)), None);
        assert_eq!(key_intent(7, &in_order(COUNT)), None);
        assert!(
            bar.iter()
                .all(|i| !matches!(i, Intent::MeteorSwarm | Intent::Thunderbolt | Intent::Swing)),
            "Everglade's bar has no offensive slot"
        );
        assert_eq!(shown(FULL_COUNT + 3).len(), FULL_COUNT);
    }

    #[test]
    fn the_dev_bar_puts_meteor_swarm_on_1_thunderbolt_on_2_and_levitate_on_3() {
        let bar: Vec<Intent> = DEV_ORDER.iter().map(|&i| SLOTS[i].0).collect();
        assert_eq!(
            bar,
            [
                Intent::MeteorSwarm,
                Intent::Thunderbolt,
                Intent::Levitate,
                Intent::FeatherFall,
                Intent::WindWall,
                Intent::ReverseGravity,
                Intent::WallOfStone,
                Intent::Swing,
            ]
        );
        // Every slot shows once.
        let mut sorted = DEV_ORDER;
        sorted.sort_unstable();
        assert_eq!(sorted.to_vec(), in_order(FULL_COUNT));
        // Number keys follow the displayed order.
        for (n, intent) in bar.iter().enumerate() {
            assert_eq!(key_intent(n + 1, &DEV_ORDER), Some(*intent));
        }
        assert_eq!(key_intent(9, &DEV_ORDER), None);
        // Each card shows its displayed key; Levitate's says to hold 3.
        for (displayed, &index) in DEV_ORDER.iter().enumerate() {
            let card = card_with_key(index, displayed).expect("a card");
            assert_eq!(card.title, SLOTS[index].2.name);
            assert!(card.details[0].0.contains(&(displayed + 1).to_string()));
        }
        let levitate = card_with_key(0, 2).unwrap();
        assert_eq!(levitate.details[0].0, "Hold 3 or L");
        // Meteor Swarm and the Thunderbolt carry their cast and range.
        for (index, cast) in [(5, meteor::CAST), (6, meteor::BOLT_CAST)] {
            let card = card_with_key(index, 0).unwrap();
            let details: Vec<&str> = card.details.iter().map(|(d, _)| d.as_str()).collect();
            assert!(details.contains(&"No mana, no cooldown"), "{details:?}");
            assert!(details.contains(&format!("{cast} s cast").as_str()));
            assert!(details.iter().any(|d| d.ends_with("m range")));
            assert!(SLOTS[index].2.text.contains("R restores the town"));
        }
    }

    #[test]
    fn the_dev_bar_and_its_cards_fit_a_phone_screen() {
        let atlas = Atlas::new(14.0);
        let bottom = 120.0;
        let [left, top, width, _] = frame_of(PHONE, bottom, FULL_COUNT);
        assert!(left >= 0.0 && left + width <= PHONE[0], "{left} {width}");
        for displayed in 0..FULL_COUNT {
            let mut ui = UiBatch::default();
            let card = card_with_key(DEV_ORDER[displayed], displayed).unwrap();
            let [x, _, w, _] = slot_rect_of(PHONE, bottom, FULL_COUNT, displayed);
            let [cx, cy, cw, ch] = tooltip::draw(&mut ui, &atlas, &card, [x, top, w, 1.0], PHONE);
            assert!(cx >= tooltip::MARGIN - 0.01, "slot {displayed} at {cx}");
            assert!(
                cx + cw <= PHONE[0] - tooltip::MARGIN + 0.01,
                "slot {displayed}"
            );
            assert!(cy >= 0.0 && cy + ch <= top, "above the tray");
        }
    }

    #[test]
    fn the_end_slots_cards_stay_on_a_phone_screen() {
        let atlas = Atlas::new(14.0);
        let bottom = 120.0;
        let [_, top, ..] = frame(PHONE, bottom);
        for index in [0, COUNT - 1] {
            let mut ui = UiBatch::default();
            let card = card(index).unwrap();
            let [x, _, w, _] = slot_rect_of(PHONE, bottom, COUNT, index);
            let [cx, cy, cw, ch] = tooltip::draw(&mut ui, &atlas, &card, [x, top, w, 1.0], PHONE);
            assert!(cx >= tooltip::MARGIN, "slot {index} at {cx}");
            assert!(cx + cw <= PHONE[0] - tooltip::MARGIN + 0.01, "slot {index}");
            assert!(cy >= 0.0 && cy + ch <= top, "above the tray");
        }
    }
}
