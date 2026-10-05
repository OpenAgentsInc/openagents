//! The demolition yard's hotbar, drawn as Everglade's is: the beveled
//! tray of game-icons.net art with the sledgehammer on `1`, Meteor Swarm
//! on `2`, and Rebuild on `R`. Over the tray sit the caster's blue mana
//! bar as the Grove draws it, a bar that fills as the cottages come down,
//! the yard's name, and a line of help, all in the glade's warm colors.
//! A cast bar in fire orange rises over the tray while Meteor Swarm
//! casts. Space still jumps, and the return portal still leads back to
//! the plaza.

use super::meteor::{self, MAX_MANA};
use crate::tooltip::{self, Card, Tip, palette};
use crate::ui::{Atlas, UiBatch};
use crate::zones::Intent;
use crate::zones::everglade::hotbar::{self as tray, Slot};

/// Each slot: the intent it sends, its icon sprite, and its key's label.
pub const SLOTS: [(Intent, &str, &str); 3] = [
    (Intent::Swing, "sledgehammer-icon", "1"),
    (Intent::MeteorSwarm, "meteor-swarm-icon", "2"),
    (Intent::Rebuild, "rebuild-icon", "R"),
];

/// Each slot's card ([`crate::tooltip`]), in [`SLOTS`] order.
pub const TIPS: [Tip; 3] = [
    Tip::new(
        "Sledgehammer",
        "Swing two-handed at the cottage piece ahead: each blow cracks it, and at zero hit points it breaks and drops what it held up.",
    ),
    Tip::new("Meteor Swarm", meteor::TOOLTIP),
    Tip::new(
        "Rebuild",
        "Stands both cottages back up whole, refills your mana, and readies Meteor Swarm.",
    ),
];

/// The line of help over the bar for Meteor Swarm's `status`.
#[must_use]
pub fn help(status: &meteor::Status) -> &'static str {
    if status.targeting {
        "Click the ground to call down Meteor Swarm · right click or Esc cancels"
    } else if status.casting.is_some() {
        "Casting Meteor Swarm · moving or Esc stops it"
    } else {
        "Click or 1 swings the sledgehammer · 2 Meteor Swarm · R rebuilds"
    }
}

/// What the bar shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    /// Whether a swing is under way.
    pub swinging: bool,
    /// Meteor Swarm's mana, readiness, targeting, cast, and cooldown.
    pub swarm: meteor::Status,
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
/// The mana bar's well and fill and its label, as the Grove's.
const MANA_WELL: [f32; 4] = [0.05, 0.06, 0.16, 0.96];
const MANA: [f32; 4] = [0.22, 0.48, 1.0, 1.0];
const MANA_TEXT: [f32; 4] = [0.62, 0.78, 1.0, 1.0];
/// The cast bar's fill and its label.
const CAST: [f32; 4] = [1.0, 0.5, 0.12, 1.0];
const CAST_TEXT: [f32; 4] = [1.0, 0.86, 0.6, 1.0];
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
    let card = Card::of(tip).detail(keys, palette::KEY);
    Some(if SLOTS[index].0 == Intent::MeteorSwarm {
        card.detail(format!("{:.0} mana", meteor::COST), palette::MANA)
            .detail(format!("{:.0} s cooldown", meteor::COOLDOWN), palette::TIME)
            .detail(format!("{} s cast", meteor::CAST), palette::TIME)
            .detail(format!("{:.0} m range", meteor::RANGE), palette::RULE)
    } else {
        card
    })
}

/// The top of the line of help on a screen of `size`, the tray raised
/// `bottom` points: what a card stays above.
#[must_use]
pub fn help_top(atlas: &Atlas, size: [f32; 2], bottom: f32) -> f32 {
    let [_, top, _, _] = tray::frame_of(size, bottom, SLOTS.len());
    // The mana bar and its label, the bar of pieces down, then the name
    // and the help, as [`draw`] stacks them.
    top - (2.0 * BAR[1] + 10.0) * tray::unit(size) - 3.0 * atlas.line - 4.0
}

/// Draws slot `index`'s card above the tray, its bar, and its help into
/// `ui`, kept on screen.
pub fn draw_tip(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, index: usize) {
    let Some(card) = card(index) else {
        return;
    };
    let [x, _, w, _] = tray::slot_rect_of(size, bottom, SLOTS.len(), index);
    let [_, top, _, height] = tray::frame_of(size, bottom, SLOTS.len());
    let help = help_top(atlas, size, bottom);
    tooltip::draw(ui, atlas, &card, [x, help, w, top + height - help], size);
}

/// The intent a key sends, by its `KeyboardEvent.code` name: `Digit1`
/// swings, `Digit2` aims Meteor Swarm, and `KeyR` rebuilds.
#[must_use]
pub fn key(code: &str) -> Option<Intent> {
    match code {
        "Digit1" => Some(Intent::Swing),
        "Digit2" => Some(Intent::MeteorSwarm),
        "KeyR" => Some(Intent::Rebuild),
        _ => None,
    }
}

/// Draws the tray, the mana bar, the bar of pieces down, the name and
/// help over them, and the cast bar while one runs into `ui`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], bottom: f32, bar: &Bar) {
    let sprites = SLOTS.map(|(_, sprite, key)| (sprite, key));
    let swarm = &bar.swarm;
    let slots = [
        Slot {
            enabled: true,
            active: bar.swinging,
            cooldown: 0.0,
        },
        Slot {
            enabled: swarm.ready || swarm.targeting,
            active: swarm.targeting || swarm.casting.is_some(),
            cooldown: swarm.cooldown,
        },
        Slot {
            enabled: true,
            active: false,
            cooldown: 0.0,
        },
    ];
    tray::draw_of(ui, atlas, size, bottom, &sprites, &slots);
    let [_, top, _, _] = tray::frame_of(size, bottom, SLOTS.len());
    let u = tray::unit(size);
    let [width, height] = BAR.map(|v| v * u);
    let left = (size[0] - width) * 0.5;
    let line = atlas.line;
    // The mana bar, labeled at its left, just over the tray.
    let mana_y = top - height - 4.0 * u;
    meter(
        ui,
        atlas,
        [left, mana_y, width, height],
        swarm.mana / MAX_MANA,
        MANA_WELL,
        MANA,
        u,
    );
    let label = format!("Mana {} / {}", swarm.mana.floor() as i32, MAX_MANA as i32);
    ui.text(
        atlas,
        left + 4.0 * u,
        mana_y - line - 2.0,
        &label,
        MANA_TEXT,
    );
    // The bar of pieces down, over the mana's label.
    let y = mana_y - line - 6.0 * u - height;
    let k = if bar.total == 0 {
        0.0
    } else {
        bar.down as f32 / bar.total as f32
    };
    meter(ui, atlas, [left, y, width, height], k, WELL, FILL, u);
    let name = format!(
        "Demolition yard · {} of {} pieces down",
        bar.down, bar.total
    );
    let centered = |text: &str| (size[0] - atlas.measure(text)) * 0.5;
    let help = help(swarm);
    ui.text(atlas, centered(&name), y - line - 2.0, &name, NAME);
    ui.text(atlas, centered(help), y - 2.0 * line - 4.0, help, HINT);
    draw_cast(ui, atlas, size, swarm);
}

/// Draws Meteor Swarm's cast bar into `ui` while a cast runs, well over
/// the tray as an MMO's is.
pub fn draw_cast(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], swarm: &meteor::Status) {
    let Some(k) = swarm.casting else {
        return;
    };
    let u = tray::unit(size);
    let cast_width = 240.0 * u;
    let cast_height = 14.0 * u;
    let cast_left = (size[0] - cast_width) * 0.5;
    let cast_y = size[1] * 0.68;
    meter(
        ui,
        atlas,
        [cast_left, cast_y, cast_width, cast_height],
        k,
        WELL,
        CAST,
        u,
    );
    let title = "Meteor Swarm";
    ui.text(
        atlas,
        (size[0] - atlas.measure(title)) * 0.5,
        cast_y - atlas.line - 2.0,
        title,
        CAST_TEXT,
    );
}

/// Draws Everglade's Meteor Swarm overlay into `ui`: while the spell aims
/// or casts, its line of help over a tray of `count` slots raised `bottom`
/// points, and its cast bar.
pub fn draw_town(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    count: usize,
    swarm: &meteor::Status,
) {
    if !swarm.targeting && swarm.casting.is_none() {
        return;
    }
    let [_, top, _, _] = tray::frame_of(size, bottom, count);
    let help = help(swarm);
    let x = (size[0] - atlas.measure(help)) * 0.5;
    ui.text(atlas, x, top - atlas.line - 4.0, help, HINT);
    draw_cast(ui, atlas, size, swarm);
}

/// A framed bar at `[left, top, width, height]` filled `k` of the way
/// with `fill` over `well`.
fn meter(
    ui: &mut UiBatch,
    atlas: &Atlas,
    [left, top, width, height]: [f32; 4],
    k: f32,
    well: [f32; 4],
    fill: [f32; 4],
    u: f32,
) {
    for (inset, color) in [(0.0, FRAME), (1.0, well)] {
        ui.rect(
            atlas,
            left + inset * u,
            top + inset * u,
            width - 2.0 * inset * u,
            height - 2.0 * inset * u,
            color,
        );
    }
    let k = k.clamp(0.0, 1.0);
    if k > 0.0 {
        ui.rect(
            atlas,
            left + u,
            top + u,
            (width - 2.0 * u) * k,
            height - 2.0 * u,
            fill,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_slots_send_the_yard_intents() {
        assert_eq!(key("Digit1"), Some(Intent::Swing));
        assert_eq!(key("Digit2"), Some(Intent::MeteorSwarm));
        assert_eq!(key("KeyR"), Some(Intent::Rebuild));
        assert_eq!(key("KeyP"), None);
        let size = [1280.0, 800.0];
        let [x, y, w, h] = tray::frame_of(size, 0.0, SLOTS.len());
        assert_eq!(
            hit([x + w * 0.2, y + h * 0.5], size, 0.0),
            Some(Intent::Swing)
        );
        assert_eq!(
            hit([x + w * 0.5, y + h * 0.5], size, 0.0),
            Some(Intent::MeteorSwarm)
        );
        assert_eq!(
            hit([x + w * 0.8, y + h * 0.5], size, 0.0),
            Some(Intent::Rebuild)
        );
        assert_eq!(hit([x - 10.0, y], size, 0.0), None);
    }

    #[test]
    fn each_slot_has_a_card_over_the_help() {
        let size = [1280.0, 800.0];
        let atlas = Atlas::new(14.0);
        let [_, top, ..] = tray::frame_of(size, 0.0, SLOTS.len());
        let help = help_top(&atlas, size, 0.0);
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
