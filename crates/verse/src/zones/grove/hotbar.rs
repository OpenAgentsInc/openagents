//! The Grove's hotbar: Everglade's icon tray with the druid's spells on
//! keys 1 to 9 and Long Rest on 0, and a mana bar above it.

use super::super::Intent;
use super::kit::{MAX_MANA, Spell};
use crate::ui::{Atlas, UiBatch};
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// How many slots the bar has.
pub const COUNT: usize = Spell::ALL.len();

/// Each slot's icon sprite, in [`Spell::ALL`] order.
pub const SPRITES: [&str; COUNT] = [
    "thunderwave-icon",
    "gust-of-wind-icon",
    "wind-wall-icon",
    "wall-of-stone-icon",
    "reverse-gravity-icon",
    "fire-bolt-icon",
    "fireball-icon",
    "misty-step-icon",
    "web-icon",
    "long-rest-icon",
];

/// Each slot's key label: 1 to 9, then 0.
pub const KEYS: [&str; COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// What the bar shows: each slot, and the druid's mana.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub slots: [Slot; COUNT],
    pub mana: f32,
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

/// Draws the tray and the mana bar over it into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, bar: &Bar) {
    let sprites: Vec<(&str, &str)> = SPRITES.iter().copied().zip(KEYS).collect();
    tray::draw_of(ui, atlas, size, bottom, &sprites, &bar.slots);
    let [left, top, width, _] = tray::frame_of(size, bottom, COUNT);
    let u = tray::unit(size);
    let height = 10.0 * u;
    let y = top - height - 4.0 * u;
    for (inset, color) in [
        (0.0, [0.46, 0.39, 0.24, 1.0]),
        (1.0, [0.05, 0.06, 0.16, 0.96]),
    ] {
        ui.rect(
            atlas,
            left + inset * u,
            y + inset * u,
            width - 2.0 * inset * u,
            height - 2.0 * inset * u,
            color,
        );
    }
    let k = (bar.mana / MAX_MANA).clamp(0.0, 1.0);
    if k > 0.0 {
        ui.rect(
            atlas,
            left + u,
            y + u,
            (width - 2.0 * u) * k,
            height - 2.0 * u,
            [0.22, 0.48, 1.0, 1.0],
        );
    }
    let label = format!("Mana {} / {}", bar.mana.floor() as i32, MAX_MANA as i32);
    ui.text(
        atlas,
        left + 4.0 * u,
        y - atlas.line - 2.0,
        &label,
        [0.62, 0.78, 1.0, 1.0],
    );
}
