//! The Grove's hotbar: the Archdruid's four rows of Everglade's icon tray
//! ([`super::slots`]). Row 1 is on keys `1` to `9`, `0`, `-`, and `=`, and
//! `Shift`, `Ctrl`, and `Alt` with the same keys reach rows 2, 3, and 4. On
//! a wide screen all four rows stack, row 1 at the bottom; a phone shows
//! one row with a row switcher at its end. The Grove has no mana and no
//! cooldowns, so the bar has no mana bar and no slot ever dims for one.
//! Resting the pointer on a slot shows its card ([`crate::tooltip`]): the
//! sentence from [`slots::info`] and the numbers from [`Spell::def`].
//!
//! A slot sends [`Intent::GroveSlot`], and the Grove casts what the slot
//! holds when the press arrives.

use super::super::Intent;
use super::dummies::Condition;
use super::kit::{Area, Damage, Delivery, Spell};
use super::shape::Form;
use super::slots::{self, COLUMNS, COUNT, ROWS};
use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// Each column's key label.
pub const KEYS: [&str; COLUMNS] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="];
/// Each row's modifier: none, Shift, Ctrl, and Alt.
pub const MODIFIERS: [&str; ROWS] = ["", "Shift", "Ctrl", "Alt"];
/// The short prefix on a slot's key label for each row.
const PREFIX: [&str; ROWS] = ["", "S", "C", "A"];
/// The row switcher's sprite, at the end of a compact bar.
pub const SWITCHER: &str = "rise-icon";
/// Space between stacked rows, in the tray's units.
const GAP: f32 = 4.0;

/// What the bar shows: each slot's ability, if any, and state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub spells: [Option<Spell>; COUNT],
    pub slots: [Slot; COUNT],
}

/// How the bar is laid out on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// All four rows, stacked, row 1 at the bottom.
    Full,
    /// One row and a switcher, for a narrow or short screen.
    Compact { row: usize },
}

impl Layout {
    /// The layout for a screen of `size` logical points showing `row` when
    /// compact: compact below 640 points either way.
    #[must_use]
    pub fn for_screen(size: [f32; 2], row: usize) -> Self {
        if size[0] < 640.0 || size[1] < 640.0 {
            Self::Compact { row: row % ROWS }
        } else {
            Self::Full
        }
    }

    /// The rows shown, bottom first.
    fn rows(self) -> Vec<usize> {
        match self {
            Self::Full => (0..ROWS).collect(),
            Self::Compact { row } => vec![row % ROWS],
        }
    }

    /// Slots in each shown row's tray: a compact row adds the switcher.
    fn width(self) -> usize {
        match self {
            Self::Full => COLUMNS,
            Self::Compact { .. } => COLUMNS + 1,
        }
    }
}

/// What a point on the bar presses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// The slot at this index, `row * 12 + column`.
    Slot(usize),
    /// A compact bar's switcher, which shows the next row.
    Switch,
}

/// The intent slot `index` sends.
#[must_use]
pub fn intent(index: usize) -> Option<Intent> {
    (index < COUNT).then(|| Intent::GroveSlot(index as u8))
}

/// The column a key selects: `1` to `9`, `0`, `-`, and `=`.
#[must_use]
pub fn key_column(key: char) -> Option<usize> {
    match key {
        '1'..='9' => Some(key as usize - '1' as usize),
        '0' => Some(9),
        '-' => Some(10),
        '=' => Some(11),
        _ => None,
    }
}

/// The row the held modifiers select: Alt over Ctrl over Shift.
#[must_use]
pub const fn row_of(shift: bool, ctrl: bool, alt: bool) -> usize {
    if alt {
        3
    } else if ctrl {
        2
    } else if shift {
        1
    } else {
        0
    }
}

/// The intent a key sends in `row`.
#[must_use]
pub fn key(key: char, row: usize) -> Option<Intent> {
    let column = key_column(key)?;
    (row < ROWS).then(|| intent(row * COLUMNS + column))?
}

/// Adds every ability's icon that `atlas` lacks, such as those Everglade's
/// hotbar has not added.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    let keys = Spell::ALL.map(|spell| slots::info(spell).0);
    for key in keys.into_iter().chain([SWITCHER]) {
        if atlas.sprites.contains_key(key) {
            continue;
        }
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// The bottom of the tray for the `shown`th row from the bottom.
fn row_bottom(size: [f32; 2], bottom: f32, layout: Layout, shown: usize) -> f32 {
    let height = tray::frame_of(size, bottom, layout.width())[3];
    bottom + shown as f32 * (height + GAP * tray::unit(size))
}

/// What `point` presses on the bar, if anything.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32, layout: Layout) -> Option<Hit> {
    for (shown, row) in layout.rows().into_iter().enumerate() {
        let b = row_bottom(size, bottom, layout, shown);
        if let Some(column) = tray::hit_of(point, size, b, layout.width()) {
            return Some(if column == COLUMNS {
                Hit::Switch
            } else {
                Hit::Slot(row * COLUMNS + column)
            });
        }
    }
    None
}

/// The index of the slot under `point`, if any.
#[must_use]
pub fn slot_under(point: [f32; 2], size: [f32; 2], bottom: f32, layout: Layout) -> Option<usize> {
    match hit(point, size, bottom, layout)? {
        Hit::Slot(index) => Some(index),
        Hit::Switch => None,
    }
}

/// The top of the bar's highest row, in logical points from the top.
#[must_use]
pub fn top(size: [f32; 2], bottom: f32, layout: Layout) -> f32 {
    let shown = layout.rows().len().saturating_sub(1);
    tray::frame_of(
        size,
        row_bottom(size, bottom, layout, shown),
        layout.width(),
    )[1]
}

/// A key label for slot `index`: its modifier's initial and its key.
#[must_use]
pub fn key_label(index: usize) -> String {
    let (row, column) = (index / COLUMNS, index % COLUMNS);
    format!(
        "{}{}",
        PREFIX.get(row).copied().unwrap_or(""),
        KEYS.get(column).copied().unwrap_or("")
    )
}

/// The dice's range as "low to high" for `count` dice of `sides` plus
/// `bonus`.
fn range_of(count: u32, sides: u32, bonus: i32) -> String {
    let low = count as i32 + bonus;
    let high = (count * sides) as i32 + bonus;
    format!("{low} to {high}")
}

/// The card for `spell` on slot `index`: its name and sentence, its key,
/// its level, and its range, area, damage range, save or attack, and
/// condition. The Grove has no mana and no cooldowns, so the card names
/// none.
#[must_use]
pub fn card_of(spell: Spell, index: usize) -> Card {
    let def = spell.def();
    let row = index / COLUMNS;
    let key = match MODIFIERS.get(row).copied().unwrap_or("") {
        "" => format!("Key {}", KEYS[index % COLUMNS]),
        modifier => format!("{modifier}+{}", KEYS[index % COLUMNS]),
    };
    let mut card = Card::of(Tip::new(def.label, slots::info(spell).1)).detail(key, palette::KEY);
    card = if spell == Spell::LongRest || spell == Spell::ChooseLand {
        card.detail("Demo control", palette::RULE)
    } else if Form::Dragon.attacks().contains(&spell) {
        card.detail("Dragon attack", palette::RULE)
    } else if spell.beast() {
        card.detail("Beast attack", palette::RULE)
    } else if spell.shape() && spell != Spell::Shapechange {
        card.detail("Wild Shape", palette::RULE)
    } else if def.level == 0 {
        card.detail("Cantrip", palette::RULE)
    } else {
        card.detail(format!("Level {}", def.level), palette::RULE)
    };
    if def.range > 0.0 {
        card = card.detail(format!("Range {:.0} m", def.range), palette::RULE);
    }
    let area = match def.area {
        Area::Burst(r) => Some(format!("{:.1} m radius", r)),
        Area::Zone(r) => Some(format!("{:.1} m radius, lasting", r)),
        Area::Cone(l) => Some(format!("{l:.1} m cone")),
        Area::Line(l, w) => Some(format!("{l:.0} m by {w:.1} m line")),
        Area::Wall(l) => Some(format!("{l:.0} m wall")),
        Area::Bolt | Area::Single | Area::Caster => None,
    };
    if let Some(area) = area {
        card = card.detail(area, palette::RULE);
    }
    let (count, sides) = def.dice;
    if count > 0 {
        let word = def.kind.word();
        let text = if def.kind == Damage::Healing {
            format!("Heals {}", range_of(count, sides, def.bonus))
        } else {
            match def.extra {
                Some((n, s, other)) => format!(
                    "{} {word} and {} {}",
                    range_of(count, sides, def.bonus),
                    range_of(n, s, 0),
                    other.word()
                ),
                None => format!("{} {word}", range_of(count, sides, def.bonus)),
            }
        };
        card = card.detail(text, palette::KEY);
    }
    let roll = match def.delivery {
        Delivery::Attack { bonus } if !spell.shape() => Some(format!("Attack +{bonus}")),
        Delivery::Save { ability, half } => Some(format!(
            "{} save{}",
            ability.name(),
            if half { ", half" } else { "" }
        )),
        _ => None,
    };
    if let Some(roll) = roll {
        card = card.detail(roll, palette::RULE);
    }
    if let Some((condition, seconds)) = def.rider {
        card = card.detail(condition_text(condition, seconds), palette::RULE);
    }
    if def.concentration || spell.glade().is_some() {
        card = card.detail("Concentration", palette::RULE);
    }
    card
}

fn condition_text(condition: Condition, seconds: f32) -> String {
    let name = condition.name();
    let mut first = name.chars();
    let name = first
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + first.as_str())
        .unwrap_or_default();
    format!("{name} {seconds:.1} s")
}

/// Slot `index`'s card on `bar`, if it holds an ability.
#[must_use]
pub fn card(bar: &Bar, index: usize) -> Option<Card> {
    Some(card_of((*bar.spells.get(index)?)?, index))
}

/// Draws slot `index`'s card above the bar into `ui`, kept on screen.
pub fn draw_tip(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    layout: Layout,
    bar: &Bar,
    index: usize,
) {
    let Some(card) = card(bar, index) else {
        return;
    };
    let column = index % COLUMNS;
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, layout.width(), column);
    let top = top(size, bottom, layout);
    tooltip::draw(ui, atlas, &card, [x, top, w, 1.0], size);
}

/// Draws the combat log at the screen's lower left into `ui`: `status`
/// (the land and the form) over the newest `lines`, oldest first. A short
/// screen shows the newest four. Beside a bar that leaves too little room,
/// the log sits above it instead.
pub fn draw_log(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    layout: Layout,
    status: &str,
    lines: &[String],
) {
    let keep = if size[1] < 640.0 { 4 } else { lines.len() };
    let lines = &lines[lines.len().saturating_sub(keep)..];
    let longest = lines
        .iter()
        .map(String::len)
        .chain([status.len()])
        .max()
        .unwrap_or(0) as f32;
    let tray = tray::frame_of(size, bottom, layout.width())[2];
    let beside = (size[0] - tray) / 2.0 - 24.0;
    let above = beside < 260.0;
    let room = if above { size[0] * 0.6 } else { beside };
    let chars = longest.min((room / atlas.advance).max(16.0));
    // A line too long for the box wraps at a space onto the next row.
    let rows: Vec<(String, f32)> = {
        let count = lines.len();
        let mut rows = Vec::new();
        for (k, line) in lines.iter().enumerate() {
            // Older lines fade.
            let alpha = 1.0 - 0.55 * (count - 1 - k) as f32 / count.max(1) as f32;
            for row in wrap(line, chars as usize) {
                rows.push((row, alpha));
            }
        }
        rows
    };
    let pad = 8.0;
    let width = chars * atlas.advance + pad * 2.0;
    let height = (rows.len() + 1) as f32 * atlas.line + pad * 2.0;
    let x = 12.0;
    let y = if above {
        top(size, bottom, layout) - 8.0 - height
    } else {
        size[1] - 12.0 - height
    };
    ui.rect(atlas, x, y, width, height, [0.03, 0.03, 0.025, 0.72]);
    let clip = |text: &str| -> String {
        if text.len() as f32 <= chars {
            text.to_owned()
        } else {
            let mut cut: String = text.chars().take(chars as usize - 1).collect();
            cut.push('~');
            cut
        }
    };
    ui.text(
        atlas,
        x + pad,
        y + pad,
        &clip(status),
        [0.98, 0.85, 0.5, 1.0],
    );
    for (k, (row, alpha)) in rows.iter().enumerate() {
        ui.text(
            atlas,
            x + pad,
            y + pad + (k + 1) as f32 * atlas.line,
            &clip(row),
            [0.92, 0.92, 0.88, *alpha],
        );
    }
}

/// `line` in rows of at most `width` characters, broken at spaces where it
/// can be; a continued row is indented two spaces.
fn wrap(line: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in line.split(' ') {
        let indent = if rows.is_empty() { 0 } else { 2 };
        if !row.is_empty() && row.chars().count() + 1 + word.chars().count() + indent > width {
            rows.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
    }
    rows.push(row);
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| if i == 0 { r } else { format!("  {r}") })
        .collect()
}

/// Draws the bar into `ui`.
pub fn draw(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    layout: Layout,
    bar: &Bar,
) {
    for (shown, row) in layout.rows().into_iter().enumerate() {
        let b = row_bottom(size, bottom, layout, shown);
        let labels: Vec<String> = (0..COLUMNS).map(|c| key_label(row * COLUMNS + c)).collect();
        let mut sprites: Vec<(&str, &str)> = (0..COLUMNS)
            .map(|c| {
                let sprite = bar.spells[row * COLUMNS + c].map_or("", |s| slots::info(s).0);
                (sprite, labels[c].as_str())
            })
            .collect();
        let mut states: Vec<Slot> = bar.slots[row * COLUMNS..(row + 1) * COLUMNS].to_vec();
        let switch = format!("{}/{ROWS}", row + 1);
        if let Layout::Compact { .. } = layout {
            sprites.push((SWITCHER, switch.as_str()));
            states.push(Slot {
                enabled: true,
                active: false,
                cooldown: 0.0,
            });
        }
        tray::draw_of(ui, atlas, size, b, &sprites, &states);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::grove::kit::Land;
    use crate::zones::grove::shape::Form;

    const SIZE: [f32; 2] = [1280.0, 800.0];

    fn bar(form: Option<Form>, land: Land) -> Bar {
        Bar {
            spells: std::array::from_fn(|i| slots::spell(i, form, land)),
            slots: [Slot {
                enabled: true,
                active: false,
                cooldown: 0.0,
            }; COUNT],
        }
    }

    #[test]
    fn the_pointer_finds_each_slot_of_every_row_and_its_card_names_the_spell() {
        let bar = bar(None, Land::Arid);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                let b = row_bottom(SIZE, 14.0, Layout::Full, row);
                let [x, y, w, h] = tray::slot_rect_of(SIZE, b, COLUMNS, column);
                let at = [x + w * 0.5, y + h * 0.5];
                assert_eq!(hit(at, SIZE, 14.0, Layout::Full), Some(Hit::Slot(index)));
                let Some(spell) = bar.spells[index] else {
                    assert!(card(&bar, index).is_none());
                    continue;
                };
                let card = card(&bar, index).expect("a card");
                assert_eq!(card.title, spell.def().label);
            }
        }
        assert_eq!(key_label(0), "1");
        assert_eq!(key_label(13), "S2");
        assert_eq!(key_label(35), "C=");
        assert_eq!(key_label(40), "A5");
        // The cards say what the numbers are, with no mana or cooldown.
        let fireball = card_of(Spell::Fireball, 43);
        assert!(fireball.details.iter().any(|(t, _)| t == "8 to 48 fire"));
        assert!(fireball.details.iter().any(|(t, _)| t == "Alt+8"));
        for spell in Spell::ALL {
            let card = card_of(spell, 0);
            let text = slots::info(spell).1;
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
    fn modifiers_choose_the_row_and_a_compact_bar_switches_rows() {
        assert_eq!(
            key('1', row_of(false, false, false)),
            Some(Intent::GroveSlot(0))
        );
        assert_eq!(
            key('2', row_of(true, false, false)),
            Some(Intent::GroveSlot(13))
        );
        assert_eq!(
            key('=', row_of(false, true, false)),
            Some(Intent::GroveSlot(35))
        );
        assert_eq!(
            key('5', row_of(true, true, true)),
            Some(Intent::GroveSlot(40))
        );
        assert_eq!(key('a', 0), None);
        let phone = [800.0, 390.0];
        let layout = Layout::for_screen(phone, 2);
        assert_eq!(layout, Layout::Compact { row: 2 });
        let [x, y, w, h] = tray::slot_rect_of(phone, 0.0, COLUMNS + 1, COLUMNS);
        assert_eq!(
            hit([x + w / 2.0, y + h / 2.0], phone, 0.0, layout),
            Some(Hit::Switch)
        );
        let [x, y, w, h] = tray::slot_rect_of(phone, 0.0, COLUMNS + 1, 0);
        assert_eq!(
            hit([x + w / 2.0, y + h / 2.0], phone, 0.0, layout),
            Some(Hit::Slot(24))
        );
        assert_eq!(Layout::for_screen(SIZE, 2), Layout::Full);
    }

    #[test]
    fn a_card_clears_the_bar_and_the_screen_edge() {
        let atlas = Atlas::new(14.0);
        let bar = bar(None, Land::Polar);
        let top = top(SIZE, 14.0, Layout::Full);
        for index in 0..COUNT {
            let Some(card) = card(&bar, index) else {
                continue;
            };
            let mut ui = UiBatch::default();
            let [x, _, w, _] = tray::slot_rect_of(SIZE, 14.0, COLUMNS, index % COLUMNS);
            let rect = tooltip::draw(&mut ui, &atlas, &card, [x, top, w, 1.0], SIZE);
            assert!(rect[1] + rect[3] <= top + 1.0, "slot {index}");
            assert!(rect[0] >= tooltip::MARGIN);
            assert!(rect[0] + rect[2] <= SIZE[0] - tooltip::MARGIN);
        }
    }
}
