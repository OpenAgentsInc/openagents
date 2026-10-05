//! Hover cards for hotbar slots: a spell's name, one sentence on what it
//! does, and a line of small details such as its key, mana, and cooldown.
//!
//! The card reimplements Zeron's frosted tooltip chip
//! (`crates/ui/src/settings/widgets.rs` `TextTooltip` and
//! `crates/ui/src/change_requests.rs` `ChangeRequestTooltip`, studied at
//! Zeron `9e1a1115`) for Verse's UI batch: no arrow, a width cap (Zeron's
//! 320 points for 11-point text, widened for the HUD's 14-point font), 9
//! points of side padding and 7 of top and bottom, 3 points between
//! lines, a 1-point border with softened corners and a small drop shadow,
//! a title brighter than the body, and a 350 ms hover delay. Zeron draws
//! its chip in neutral grays; this one takes Everglade's bronze tray
//! colors, so the zones never show a black-and-white box. The desktop
//! app's port (`rust-native-desktop`'s window) places a chip 6 points over
//! its control and clamps it to the window, and this card does the same,
//! dropping below the control when there is no room above.
//!
//! A hotbar keeps each slot's text as data beside the slot ([`Tip`]), so a
//! new slot gets its card by adding its own line.

use crate::ui::{Atlas, UiBatch};

/// How long the pointer rests on a slot before its card shows, s (Zeron's
/// `tooltip_show_delay`).
pub const HOVER_DELAY: f32 = 0.35;
/// How long a touch holds a slot before its card shows, s. A shorter touch
/// is a tap that uses the slot.
pub const LONG_PRESS: f32 = 0.45;
/// After a card hides, how long another slot's card shows without the
/// delay, s, so sweeping along a bar reads each slot at once.
const WARM: f32 = 0.3;

/// The widest card, in logical points.
pub const MAX_WIDTH: f32 = 360.0;
/// Padding inside the border, points.
const PAD_X: f32 = 9.0;
const PAD_Y: f32 = 7.0;
/// Space between lines, points.
const GAP: f32 = 3.0;
/// Space between the card and its slot, points.
pub const OFFSET: f32 = 6.0;
/// The nearest a card comes to a screen edge, points.
pub const MARGIN: f32 = 8.0;
/// How far the drop shadow falls, points.
const SHADOW: f32 = 2.0;

/// Everglade's tooltip palette: the hotbar tray's bronze and dark walnut,
/// a gold name, parchment text, and colored details.
pub mod palette {
    /// The card's fill, the tray's dark walnut well.
    pub const SURFACE: [f32; 4] = [0.04, 0.03, 0.018, 0.95];
    /// The border, the tray slots' bronze edge.
    pub const BORDER: [f32; 4] = [0.55, 0.45, 0.28, 1.0];
    /// The drop shadow under the card.
    pub const SHADOW: [f32; 4] = [0.05, 0.03, 0.012, 0.45];
    /// The spell's name: the gold of a lit slot's edge.
    pub const TITLE: [f32; 4] = [0.98, 0.82, 0.38, 1.0];
    /// The sentence on what it does.
    pub const BODY: [f32; 4] = [0.88, 0.82, 0.68, 1.0];
    /// A key or control.
    pub const KEY: [f32; 4] = [0.78, 0.68, 0.48, 1.0];
    /// Mana, a pale blue.
    pub const MANA: [f32; 4] = [0.62, 0.78, 1.0, 1.0];
    /// A cooldown or duration, a leaf green.
    pub const TIME: [f32; 4] = [0.64, 0.8, 0.5, 1.0];
    /// A rule such as concentration, a muted violet.
    pub const RULE: [f32; 4] = [0.8, 0.7, 0.95, 1.0];
    /// The dot between details.
    pub const SEPARATOR: [f32; 4] = [0.46, 0.39, 0.24, 1.0];
}

/// A slot's name and its one sentence, kept beside the slot's definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tip {
    pub name: &'static str,
    pub text: &'static str,
}

impl Tip {
    #[must_use]
    pub const fn new(name: &'static str, text: &'static str) -> Self {
        Self { name, text }
    }
}

/// What a card shows: a title, a body, and colored details on the last
/// line, joined with dots.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub title: String,
    pub body: String,
    pub details: Vec<(String, [f32; 4])>,
}

impl Card {
    /// A card for `tip` with no details yet.
    #[must_use]
    pub fn of(tip: Tip) -> Self {
        Self {
            title: tip.name.into(),
            body: tip.text.into(),
            details: Vec::new(),
        }
    }

    /// Adds one detail in `color`.
    #[must_use]
    pub fn detail(mut self, text: impl Into<String>, color: [f32; 4]) -> Self {
        self.details.push((text.into(), color));
        self
    }
}

/// Which slot the pointer rests on and since when, for the hover delay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dwell {
    slot: Option<(usize, f32)>,
    /// When the last shown card hid, s.
    hid_at: Option<f32>,
}

impl Dwell {
    /// Records that the pointer is over `slot` (or none) at `now` seconds
    /// and returns the slot whose card shows.
    pub fn update(&mut self, slot: Option<usize>, now: f32) -> Option<usize> {
        let shown = self.shown(now);
        match (slot, self.slot) {
            (Some(index), Some((current, _))) if index == current => {}
            (Some(index), _) => {
                // Moving straight from one card to the next skips the delay.
                let warm = shown.is_some() || self.hid_at.is_some_and(|at| now - at <= WARM);
                let since = if warm { now - HOVER_DELAY } else { now };
                self.slot = Some((index, since));
            }
            (None, _) => {
                if shown.is_some() {
                    self.hid_at = Some(now);
                }
                self.slot = None;
            }
        }
        self.shown(now)
    }

    /// The slot whose card shows at `now`, if the pointer has rested long
    /// enough.
    #[must_use]
    pub fn shown(&self, now: f32) -> Option<usize> {
        self.slot
            .filter(|&(_, since)| now - since >= HOVER_DELAY - 1e-4)
            .map(|(index, _)| index)
    }
}

/// Whether a touch held for `held` seconds is a long press, which shows the
/// card instead of using the slot.
#[must_use]
pub fn long_press(held: f32) -> bool {
    held >= LONG_PRESS
}

/// A card laid out for drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Left, top, width, and height, points.
    pub rect: [f32; 4],
    pub title: Vec<String>,
    pub body: Vec<String>,
    /// Each details line: its pieces and their colors.
    pub details: Vec<Vec<(String, [f32; 4])>>,
}

/// The card's size for `card`, and its wrapped lines, before placement.
fn measure(atlas: &Atlas, card: &Card, screen: [f32; 2]) -> Layout {
    let max = MAX_WIDTH.min(screen[0] - 2.0 * MARGIN).max(4.0 * PAD_X);
    let inner = max - 2.0 * PAD_X;
    let title = atlas.wrap(&card.title, inner);
    let body = if card.body.is_empty() {
        Vec::new()
    } else {
        atlas.wrap(&card.body, inner)
    };
    let dot = " · ";
    let mut details: Vec<Vec<(String, [f32; 4])>> = Vec::new();
    let mut width = 0.0_f32;
    for (text, color) in &card.details {
        let piece = atlas.measure(text);
        let fits = details.last().is_some_and(|line| {
            let used: f32 = line.iter().map(|(t, _)| atlas.measure(t)).sum();
            used + atlas.measure(dot) + piece <= inner
        });
        if fits {
            let line = details.last_mut().expect("a details line");
            line.push((dot.into(), palette::SEPARATOR));
            line.push((text.clone(), *color));
        } else {
            details.push(vec![(text.clone(), *color)]);
        }
    }
    for line in title.iter().chain(&body) {
        width = width.max(atlas.measure(line));
    }
    for line in &details {
        width = width.max(line.iter().map(|(t, _)| atlas.measure(t)).sum());
    }
    let lines = title.len() + body.len() + details.len();
    let height = lines as f32 * atlas.line + lines.saturating_sub(1) as f32 * GAP;
    Layout {
        rect: [
            0.0,
            0.0,
            (width.min(inner) + 2.0 * PAD_X).ceil(),
            (height + 2.0 * PAD_Y).ceil(),
        ],
        title,
        body,
        details,
    }
}

/// Where a card of `size` goes for a slot at `anchor` (left, top, width,
/// height) on a screen of `screen`, all in points: centered over the slot
/// [`OFFSET`] above it, kept [`MARGIN`] inside the screen's sides, and
/// below the slot when it does not fit above.
#[must_use]
pub fn place(anchor: [f32; 4], size: [f32; 2], screen: [f32; 2]) -> [f32; 2] {
    let [ax, ay, aw, ah] = anchor;
    let clamp = |value: f32, low: f32, high: f32| value.max(low).min(high.max(low));
    let x = clamp(
        ax + aw * 0.5 - size[0] * 0.5,
        MARGIN,
        screen[0] - MARGIN - size[0],
    );
    let above = ay - OFFSET - size[1];
    let y = if above >= MARGIN {
        above
    } else {
        clamp(ay + ah + OFFSET, MARGIN, screen[1] - MARGIN - size[1])
    };
    [x, y]
}

/// Lays `card` out for the slot at `anchor` on a screen of `screen`.
#[must_use]
pub fn layout(atlas: &Atlas, card: &Card, anchor: [f32; 4], screen: [f32; 2]) -> Layout {
    let mut layout = measure(atlas, card, screen);
    let [_, _, w, h] = layout.rect;
    let [x, y] = place(anchor, [w, h], screen);
    layout.rect = [x, y, w, h];
    layout
}

/// A rectangle with its corners cut by `r` points, the batch's stand-in
/// for Zeron's 6-point radius.
fn notched(ui: &mut UiBatch, atlas: &Atlas, [x, y, w, h]: [f32; 4], r: f32, color: [f32; 4]) {
    ui.rect(atlas, x + r, y, w - 2.0 * r, h, color);
    ui.rect(atlas, x, y + r, r, h - 2.0 * r, color);
    ui.rect(atlas, x + w - r, y + r, r, h - 2.0 * r, color);
}

/// Draws `card` for the slot at `anchor` on a screen of `screen` into `ui`,
/// all in points. Returns where it went.
pub fn draw(
    ui: &mut UiBatch,
    atlas: &Atlas,
    card: &Card,
    anchor: [f32; 4],
    screen: [f32; 2],
) -> [f32; 4] {
    let layout = layout(atlas, card, anchor, screen);
    let [x, y, w, h] = layout.rect;
    notched(
        ui,
        atlas,
        [x - 1.0, y + SHADOW - 1.0, w + 2.0, h + 2.0],
        3.0,
        palette::SHADOW,
    );
    notched(ui, atlas, [x, y, w, h], 2.0, palette::BORDER);
    notched(
        ui,
        atlas,
        [x + 1.0, y + 1.0, w - 2.0, h - 2.0],
        1.0,
        palette::SURFACE,
    );
    let mut line_y = y + PAD_Y;
    let left = x + PAD_X;
    for line in &layout.title {
        // Drawn twice a point apart: the atlas has one weight, and the name
        // reads bold over the body.
        ui.text(atlas, left, line_y, line, palette::TITLE);
        ui.text(atlas, left + 1.0, line_y, line, palette::TITLE);
        line_y += atlas.line + GAP;
    }
    for line in &layout.body {
        ui.text(atlas, left, line_y, line, palette::BODY);
        line_y += atlas.line + GAP;
    }
    for line in &layout.details {
        let mut pen = left;
        for (text, color) in line {
            pen += ui.text(atlas, pen, line_y, text, *color);
        }
        line_y += atlas.line + GAP;
    }
    layout.rect
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: [f32; 2] = [1280.0, 800.0];

    fn card() -> Card {
        Card::of(Tip::new(
            "Wind Wall",
            "Raises a wall of wind ahead whose updraft throws you upward when you walk into it.",
        ))
        .detail("Key 6", palette::KEY)
        .detail("6 s cooldown", palette::TIME)
    }

    #[test]
    fn a_card_sits_centered_above_its_slot() {
        let anchor = [620.0, 700.0, 40.0, 40.0];
        let [x, y] = place(anchor, [200.0, 60.0], SCREEN);
        assert_eq!(x, 540.0);
        assert_eq!(y, 700.0 - OFFSET - 60.0);
    }

    #[test]
    fn a_card_stays_inside_the_left_and_right_edges() {
        let [x, _] = place([0.0, 700.0, 36.0, 36.0], [300.0, 60.0], SCREEN);
        assert_eq!(x, MARGIN);
        let [x, _] = place([1260.0, 700.0, 20.0, 20.0], [300.0, 60.0], SCREEN);
        assert_eq!(x, SCREEN[0] - MARGIN - 300.0);
    }

    #[test]
    fn a_card_drops_below_a_slot_at_the_top_edge() {
        let [_, y] = place([600.0, 10.0, 36.0, 36.0], [200.0, 60.0], SCREEN);
        assert_eq!(y, 10.0 + 36.0 + OFFSET);
        // On a screen too short for either, it keeps to the top margin.
        let [_, y] = place([0.0, 10.0, 36.0, 36.0], [200.0, 60.0], [400.0, 70.0]);
        assert_eq!(y, MARGIN);
    }

    #[test]
    fn a_card_wraps_to_its_width_and_a_narrow_screen() {
        let atlas = Atlas::new(14.0);
        let wide = layout(&atlas, &card(), [600.0, 700.0, 36.0, 36.0], SCREEN);
        assert!(wide.rect[2] <= MAX_WIDTH);
        assert_eq!(wide.title, vec!["Wind Wall".to_string()]);
        assert!(wide.body.len() >= 2, "{:?}", wide.body);
        assert_eq!(wide.details.len(), 1);
        let narrow = layout(&atlas, &card(), [100.0, 500.0, 36.0, 36.0], [200.0, 600.0]);
        assert!(narrow.rect[2] <= 200.0 - 2.0 * MARGIN);
        assert!(narrow.rect[0] >= MARGIN);
        assert!(narrow.body.len() > wide.body.len());
    }

    #[test]
    fn the_card_draws_in_color_never_gray() {
        let atlas = Atlas::new(14.0);
        let mut ui = UiBatch::default();
        let rect = draw(&mut ui, &atlas, &card(), [600.0, 700.0, 36.0, 36.0], SCREEN);
        assert!(rect[1] + rect[3] <= 700.0);
        assert!(!ui.vertices.is_empty());
        for vertex in &ui.vertices {
            let [r, g, b, _] = vertex.color;
            assert!(
                (r - g).abs() > 0.01 || (g - b).abs() > 0.01,
                "a gray vertex {:?}",
                vertex.color
            );
        }
    }

    #[test]
    fn hovering_waits_for_the_delay_and_sweeping_skips_it() {
        let mut dwell = Dwell::default();
        assert_eq!(dwell.update(Some(2), 0.0), None);
        assert_eq!(dwell.update(Some(2), 0.2), None);
        assert_eq!(dwell.update(Some(2), 0.36), Some(2));
        // The next slot shows at once while a card is up.
        assert_eq!(dwell.update(Some(3), 0.4), Some(3));
        // In the gap between slots the card hides, and the next slot
        // still shows at once.
        assert_eq!(dwell.update(None, 0.45), None);
        assert_eq!(dwell.update(Some(4), 0.5), Some(4));
        // Away for a while, the delay applies again.
        assert_eq!(dwell.update(None, 0.6), None);
        assert_eq!(dwell.update(Some(1), 2.0), None);
        assert_eq!(dwell.update(Some(1), 2.4), Some(1));
    }

    #[test]
    fn a_long_press_is_held_past_its_threshold() {
        assert!(!long_press(0.1));
        assert!(long_press(LONG_PRESS));
    }
}
