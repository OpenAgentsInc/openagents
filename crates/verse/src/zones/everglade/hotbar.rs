//! Everglade's movement hotbar, drawn as the chamber's action bar is: a
//! beveled tray of game-icons.net art with a number key on each slot. The
//! icons are sprites in the HUD's atlas, added by [`add_sprites`].

use super::super::Intent;
use crate::ui::{Atlas, UiBatch};

/// One slot: the intent it sends and its icon sprite.
pub const SLOTS: [(Intent, &str); 5] = [
    (Intent::Jump, "jump-icon"),
    (Intent::Sprint, "sprint-icon"),
    (Intent::Levitate, "levitate-icon"),
    (Intent::Rise, "rise-icon"),
    (Intent::Lower, "descend-icon"),
];

/// Whether a slot can be used now, and whether its toggle is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub enabled: bool,
    pub active: bool,
}

const ICON: f32 = 40.0;
const STEP: f32 = 48.0;
const PAD: f32 = 8.0;

/// Adds every slot's icon to `atlas`.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for (_, key) in SLOTS {
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// The tray's frame in logical units for a screen of `size`, `bottom` above
/// the screen's bottom edge.
#[must_use]
pub fn frame(size: [f32; 2], bottom: f32) -> [f32; 4] {
    let width = 2.0 * PAD + STEP * (SLOTS.len() as f32 - 1.0) + ICON;
    let height = ICON + 2.0 * PAD;
    [
        (size[0] - width) * 0.5,
        size[1] - bottom - height,
        width,
        height,
    ]
}

fn slot_origin(frame: [f32; 4], index: usize) -> [f32; 2] {
    [frame[0] + PAD + STEP * index as f32, frame[1] + PAD]
}

/// The intent under `point`, if any.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<Intent> {
    let frame = frame(size, bottom);
    SLOTS.iter().enumerate().find_map(|(index, (intent, _))| {
        let [x, y] = slot_origin(frame, index);
        (point[0] >= x && point[0] <= x + ICON && point[1] >= y && point[1] <= y + ICON)
            .then_some(*intent)
    })
}

/// Draws the tray with `slots` (in [`SLOTS`] order) into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, slots: &[Slot; 5]) {
    let frame = frame(size, bottom);
    let [left, top, width, height] = frame;
    for (inset, color) in [
        (0.0, [0.08, 0.07, 0.06, 0.96]),
        (1.0, [0.46, 0.39, 0.24, 1.0]),
        (2.0, [0.19, 0.17, 0.13, 1.0]),
        (4.0, [0.035, 0.03, 0.025, 0.96]),
    ] {
        ui.rect(
            atlas,
            left + inset,
            top + inset,
            width - inset * 2.0,
            height - inset * 2.0,
            color,
        );
    }
    for (index, ((_, key), slot)) in SLOTS.iter().zip(slots).enumerate() {
        let [x, y] = slot_origin(frame, index);
        let tint = if slot.enabled {
            [1.0; 4]
        } else {
            [0.32, 0.32, 0.32, 1.0]
        };
        ui.image_region(atlas, key, [x, y, ICON, ICON], [0.0, 1.0, 0.0, 1.0], tint);
        let (edge, width) = if slot.active {
            ([0.98, 0.82, 0.38, 1.0], 2.0)
        } else {
            ([0.55, 0.45, 0.28, 1.0], 1.0)
        };
        ui.frame(atlas, x - 1.0, y - 1.0, ICON + 2.0, ICON + 2.0, width, edge);
        let key = (index + 1).to_string();
        ui.text(
            atlas,
            x + ICON - 3.0 - atlas.advance * key.len() as f32,
            y + 1.0,
            &key,
            [0.75, 0.75, 0.75, 1.0],
        );
    }
}
