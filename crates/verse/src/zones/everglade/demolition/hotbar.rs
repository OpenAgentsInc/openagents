//! The demolition yard's hotbar, drawn as Everglade's is: the beveled
//! tray of game-icons.net art with the sledgehammer on `1` and Rebuild on
//! `R`. Over the tray a bar fills as the cottages come down, under the
//! yard's name and a line of help, all in the glade's warm colors. Space
//! still jumps, and the return portal still leads back to the plaza.

use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};
use crate::zones::Intent;
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// Each slot: the intent it sends, its icon sprite, and its key's label.
pub const SLOTS: [(Intent, &str, &str); 2] = [
    (Intent::Swing, "sledgehammer-icon", "1"),
    (Intent::Rebuild, "rebuild-icon", "R"),
];

/// Each slot's card ([`crate::tooltip`]), in [`SLOTS`] order.
pub const TIPS: [Tip; 2] = [
    Tip::new(
        "Sledgehammer",
        "Swing two-handed at the cottage piece ahead: each blow cracks it, and at zero hit points it breaks and drops what it held up.",
    ),
    Tip::new("Rebuild", "Stands both cottages back up whole."),
];

/// The line of help over the bar.
pub const HELP: &str = "Click or 1 swings the sledgehammer · R rebuilds";

/// What the bar shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    /// Whether a swing is under way.
    pub swinging: bool,
    /// Pieces no longer standing, and every piece.
    pub down: usize,
    pub total: usize,
}

/// The parchment of the yard's name, the green of its help, and the
/// bar's frame, well, and fill.
const NAME: [f32; 4] = [0.98, 0.86, 0.6, 1.0];
const HINT: [f32; 4] = [0.74, 0.9, 0.56, 1.0];
const FRAME: [f32; 4] = [0.46, 0.39, 0.24, 1.0];
const WELL: [f32; 4] = [0.16, 0.08, 0.04, 0.94];
const FILL: [f32; 4] = [0.95, 0.52, 0.18, 1.0];
/// The bar's width and height, in the tray's units.
const BAR: [f32; 2] = [300.0, 10.0];

/// Adds every slot's icon that `atlas` lacks.
///
/// # Errors
///
/// Returns a message when an icon cannot be rasterized.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    for (_, key, _) in SLOTS {
        if atlas.sprites.contains_key(key) {
            continue;
        }
        let icon = icons::icon(key).ok_or_else(|| format!("no hotbar icon for {key}"))?;
        atlas.add_sprite(key, icons::SIZE, icons::SIZE, &icons::rasterize(icon)?)?;
    }
    Ok(())
}

/// The intent under `point` on a screen of `size`, the tray raised
/// `bottom` points, if any.
#[must_use]
pub fn hit(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<Intent> {
    tray::hit_of(point, size, bottom, SLOTS.len()).map(|index| SLOTS[index].0)
}

/// The index of the slot under `point`, if any.
#[must_use]
pub fn slot_under(point: [f32; 2], size: [f32; 2], bottom: f32) -> Option<usize> {
    tray::hit_of(point, size, bottom, SLOTS.len())
}

/// Slot `index`'s card: its name, its sentence, and its keys.
#[must_use]
pub fn card(index: usize) -> Option<Card> {
    let tip = *TIPS.get(index)?;
    let keys = if index == 0 {
        "Click or 1".to_string()
    } else {
        format!("Key {}", SLOTS[index].2)
    };
    Some(Card::of(tip).detail(keys, palette::KEY))
}

/// Draws slot `index`'s card above the tray, its bar, and its help into
/// `ui`, kept on screen.
pub fn draw_tip(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, index: usize) {
    let Some(card) = card(index) else {
        return;
    };
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, SLOTS.len(), index);
    let [_, top, _, height] = tray::frame_of(size, bottom, SLOTS.len());
    let help = top - (BAR[1] + 6.0) * tray::unit(size) - 2.0 * atlas.line - 4.0;
    tooltip::draw(ui, atlas, &card, [x, help, w, top + height - help], size);
}

/// The intent a key sends, by its `KeyboardEvent.code` name: `Digit1`
/// swings, and `KeyR` rebuilds.
#[must_use]
pub fn key(code: &str) -> Option<Intent> {
    match code {
        "Digit1" => Some(Intent::Swing),
        "KeyR" => Some(Intent::Rebuild),
        _ => None,
    }
}

/// Draws the tray, the bar of pieces down, and the name and help over it
/// into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, bar: &Bar) {
    let sprites = SLOTS.map(|(_, sprite, key)| (sprite, key));
    let slots = [0, 1].map(|i| Slot {
        enabled: true,
        active: i == 0 && bar.swinging,
        cooldown: 0.0,
    });
    tray::draw_of(ui, atlas, size, bottom, &sprites, &slots);
    let [_, top, _, _] = tray::frame_of(size, bottom, SLOTS.len());
    let u = tray::unit(size);
    let [width, height] = BAR.map(|v| v * u);
    let left = (size[0] - width) * 0.5;
    let y = top - height - 6.0 * u;
    for (inset, color) in [(0.0, FRAME), (1.0, WELL)] {
        ui.rect(
            atlas,
            left + inset * u,
            y + inset * u,
            width - 2.0 * inset * u,
            height - 2.0 * inset * u,
            color,
        );
    }
    let k = if bar.total == 0 {
        0.0
    } else {
        (bar.down as f32 / bar.total as f32).clamp(0.0, 1.0)
    };
    if k > 0.0 {
        ui.rect(
            atlas,
            left + u,
            y + u,
            (width - 2.0 * u) * k,
            height - 2.0 * u,
            FILL,
        );
    }
    let name = format!(
        "Demolition yard · {} of {} pieces down",
        bar.down, bar.total
    );
    let centered = |text: &str| (size[0] - atlas.measure(text)) * 0.5;
    let line = atlas.line;
    ui.text(atlas, centered(&name), y - line - 2.0, &name, NAME);
    ui.text(atlas, centered(HELP), y - 2.0 * line - 4.0, HELP, HINT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_slots_send_the_yard_intents() {
        assert_eq!(key("Digit1"), Some(Intent::Swing));
        assert_eq!(key("KeyR"), Some(Intent::Rebuild));
        assert_eq!(key("KeyP"), None);
        let size = [1280.0, 800.0];
        let [x, y, w, h] = tray::frame_of(size, 0.0, SLOTS.len());
        assert_eq!(
            hit([x + w * 0.3, y + h * 0.5], size, 0.0),
            Some(Intent::Swing)
        );
        assert_eq!(
            hit([x + w * 0.7, y + h * 0.5], size, 0.0),
            Some(Intent::Rebuild)
        );
        assert_eq!(hit([x - 10.0, y], size, 0.0), None);
    }

    #[test]
    fn each_slot_has_a_card_over_the_help() {
        let size = [1280.0, 800.0];
        let atlas = Atlas::new(14.0);
        let [_, top, ..] = tray::frame_of(size, 0.0, SLOTS.len());
        let help = top - (BAR[1] + 6.0) * tray::unit(size) - 2.0 * atlas.line - 4.0;
        for index in 0..SLOTS.len() {
            let [x, y, w, h] = tray::slot_rect_of(size, 0.0, SLOTS.len(), index);
            assert_eq!(
                slot_under([x + w * 0.5, y + h * 0.5], size, 0.0),
                Some(index)
            );
            let card = card(index).expect("a card");
            assert_eq!(card.title, TIPS[index].name);
            let mut ui = UiBatch::default();
            draw_tip(&mut ui, &atlas, size, 0.0, index);
            assert!(!ui.vertices.is_empty());
            let [_, cy, _, ch] =
                tooltip::layout(&atlas, &card, [x, help, w, top + h - help], size).rect;
            assert!(cy + ch <= help);
        }
        assert!(card(SLOTS.len()).is_none());
    }
}
