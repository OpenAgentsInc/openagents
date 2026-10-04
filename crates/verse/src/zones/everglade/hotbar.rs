//! Everglade's hotbar, drawn as the chamber's action bar is: a beveled tray
//! of game-icons.net art with a number key on each slot and a cooldown
//! sector over a spell that is not ready. Levitate, Up, and Down come first,
//! then the spells that need no enemy ([`super::spells`]). The icons are
//! sprites in the HUD's atlas, added by [`add_sprites`].

use super::super::Intent;
use crate::ui::{Atlas, UiBatch};

/// How many slots the bar has.
pub const COUNT: usize = 7;

/// One slot: the intent it sends and its icon sprite. Number keys 1 to 7
/// press them in order.
pub const SLOTS: [(Intent, &str); COUNT] = [
    (Intent::Levitate, "levitate-icon"),
    (Intent::Rise, "rise-icon"),
    (Intent::Lower, "descend-icon"),
    (Intent::FeatherFall, "feather-fall-icon"),
    (Intent::WallOfStone, "wall-of-stone-icon"),
    (Intent::WindWall, "wind-wall-icon"),
    (Intent::ReverseGravity, "reverse-gravity-icon"),
];

/// Whether a slot can be used now, whether its toggle or spell is on, and
/// the fraction of its cooldown left.
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
fn unit(size: [f32; 2]) -> f32 {
    (size[1] / 768.0).max(1.0)
}

/// Adds every slot's icon to `atlas`.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for (_, key) in SLOTS {
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// The tray's frame in logical points for a screen of `size`: centered at
/// the bottom as the chamber's bar is, raised `bottom` points more (to clear
/// a phone's sticks).
#[must_use]
pub fn frame(size: [f32; 2], bottom: f32) -> [f32; 4] {
    let u = unit(size);
    let width = (2.0 * PAD + STEP * (SLOTS.len() as f32 - 1.0) + ICON) * u;
    let height = (ICON + 2.0 * PAD) * u;
    [
        (size[0] - width) * 0.5,
        size[1] - bottom - MARGIN * u - height,
        width,
        height,
    ]
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

/// The intent under `point`, if any.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<Intent> {
    let frame = frame(size, bottom);
    SLOTS.iter().enumerate().find_map(|(index, (intent, _))| {
        let ([x, y], icon) = slot_at(size, frame, index);
        (point[0] >= x && point[0] <= x + icon && point[1] >= y && point[1] <= y + icon)
            .then_some(*intent)
    })
}

/// Draws the tray with `slots` (in [`SLOTS`] order) into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, slots: &[Slot; COUNT]) {
    let frame = frame(size, bottom);
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
    for (index, ((_, key), slot)) in SLOTS.iter().zip(slots).enumerate() {
        let ([x, y], icon) = slot_at(size, frame, index);
        let tint = if slot.enabled {
            [1.0; 4]
        } else {
            [0.32, 0.32, 0.32, 1.0]
        };
        ui.image_region(atlas, key, [x, y, icon, icon], [0.0, 1.0, 0.0, 1.0], tint);
        ui.cooldown(atlas, [x, y, icon], slot.cooldown);
        let (edge, width) = if slot.active {
            ([0.98, 0.82, 0.38, 1.0], 2.0)
        } else {
            ([0.55, 0.45, 0.28, 1.0], 1.0)
        };
        ui.frame(atlas, x - 1.0, y - 1.0, icon + 2.0, icon + 2.0, width, edge);
        let key = (index + 1).to_string();
        ui.text(
            atlas,
            x + icon - 3.0 - atlas.advance * key.len() as f32,
            y + 1.0,
            &key,
            [0.75, 0.75, 0.75, 1.0],
        );
    }
}
