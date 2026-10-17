//! The Lab row (#11884): `lab-ui`'s Rust Native card for a saved chat's Lab
//! card, laid out for the phone.
//!
//! The card's text, notes, and open button come from `lab_ui::card`, the same
//! view the desktop and the web draw. Its one surface becomes a
//! [`WidgetKind::LabMedia`] rect that the platform fills with the cell for the
//! card's standard view. The row adds a note control beside the open button.

use std::sync::Arc;

use lab_kit::{Card, View};
use openagents_text::WhiteSpace;
use rust_native::{Element, TextRole};

use super::display::{ColorRole, DisplayBuilder, WidgetKind};
use super::markdown::{Ctx, PText, Px, place_text, prepare_plain};
use super::style::{Family, TYPE, Weight};

/// A shown card and the space its document lives in.
#[derive(Debug, Clone, PartialEq)]
pub struct LabCardInput {
    pub space: String,
    pub card: Card,
}

/// The standard view a Lab card's media cell draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LabMediaView {
    Video,
    Image,
    Compare,
    Model3d,
    Playable,
}

impl LabMediaView {
    fn of(view: View) -> Option<Self> {
        Some(match view.drawn() {
            View::Video => Self::Video,
            View::Image => Self::Image,
            View::Compare => Self::Compare,
            View::Model3d => Self::Model3d,
            View::Playable => Self::Playable,
            View::Report | View::None | View::Unknown => return None,
        })
    }
}

pub(crate) enum Piece {
    Text(PText),
    Media(LabMediaView),
}

pub(crate) struct LabRow {
    /// The row's opaque `lab-card:` reference.
    pub path: String,
    pub pieces: Vec<Piece>,
    /// A frame strip under the picture.
    pub strip: bool,
    pub open: Option<PText>,
    pub note: PText,
    pub versions: PText,
}

const STRIP: f32 = 44.0;
const BUTTON: f32 = 34.0;
const GAP: f32 = 8.0;
const PAD: f32 = 14.0;

/// Prepares a Lab row from `lab-ui`'s card. `None` when the card can't be
/// drawn, so the row falls back to its title.
pub(crate) fn prepare(ctx: &mut Ctx, path: &str, input: &LabCardInput) -> Option<LabRow> {
    // A live key holds a file path, which isn't a valid view instance.
    let instance = if path.starts_with(LIVE_PREFIX) {
        format!("{LIVE_PREFIX}{:016x}", super::rows::row_key(path))
    } else {
        path.to_owned()
    };
    let projection = lab_ui::card(&input.card, &input.space, &instance, 1).ok()?;
    let view = projection.view.view();
    let Element::Stack { children, .. } = &view.root.element else {
        return None;
    };
    let (body_size, body_lh) = TYPE.body;
    let (head_size, head_lh) = TYPE.h[2];
    let (small_size, small_lh) = TYPE.small;
    let mut pieces = Vec::new();
    let mut open = None;
    for node in children {
        match &node.element {
            Element::Text { value, role } => {
                let (family, weight, size, lh, color) = match role {
                    TextRole::Heading => (
                        Family::Sans,
                        Weight::Semibold,
                        head_size,
                        head_lh,
                        ColorRole::Text,
                    ),
                    TextRole::Status => (
                        Family::Sans,
                        Weight::Medium,
                        small_size,
                        small_lh,
                        ColorRole::TextSecondary,
                    ),
                    _ => (
                        Family::Sans,
                        Weight::Regular,
                        body_size,
                        body_lh,
                        ColorRole::Text,
                    ),
                };
                let style = ctx.typo.style(family, weight, false, size);
                let lh = ctx.typo.px(lh);
                pieces.push(Piece::Text(prepare_plain(
                    ctx,
                    value,
                    style,
                    lh,
                    color,
                    WhiteSpace::Normal,
                )));
            }
            Element::Surface { .. } => {
                if let Some(view) = LabMediaView::of(input.card.view) {
                    pieces.push(Piece::Media(view));
                }
            }
            Element::Button { label, .. } => {
                open = Some(button_text(ctx, label));
            }
            _ => {}
        }
    }
    let strip = !input.card.strip.is_empty()
        && matches!(input.card.view.drawn(), View::Image | View::Video);
    let note = button_text(ctx, "Add a note");
    let versions = button_text(ctx, "Versions");
    Some(LabRow {
        path: path.to_owned(),
        pieces,
        strip,
        open,
        note,
        versions,
    })
}

fn button_text(ctx: &mut Ctx, label: &str) -> PText {
    let (size, lh) = TYPE.small;
    let style = ctx.typo.style(Family::Sans, Weight::Medium, false, size);
    let lh = ctx.typo.px(lh);
    prepare_plain(ctx, label, style, lh, ColorRole::Accent, WhiteSpace::Normal)
}

/// The media cell's height at column width `w`.
fn media_height(view: LabMediaView, strip: bool, px: Px, w: f32) -> f32 {
    let screen = match view {
        // Two sides next to each other, each with its version line.
        LabMediaView::Compare => ((w - px.v(GAP)) / 2.0) * 9.0 / 16.0 + px.v(TYPE.small.1),
        _ => w * 9.0 / 16.0,
    };
    screen + if strip { px.v(GAP + STRIP) } else { 0.0 }
}

/// Lays out (and optionally paints) the card at column `x`, `top`, width `cw`.
pub(crate) fn place(
    row: &LabRow,
    px: Px,
    x: f32,
    top: f32,
    cw: f32,
    mut out: Option<&mut DisplayBuilder>,
) -> f32 {
    let pad = px.v(PAD);
    let inner = (cw - pad * 2.0).max(20.0);
    let mut y = top + pad;
    let start = out.as_deref().map(|o| o.boxes.len());
    for (i, piece) in row.pieces.iter().enumerate() {
        if i > 0 {
            y += px.v(GAP);
        }
        match piece {
            Piece::Text(t) => y += place_text(t, x + pad, y, inner, out.as_deref_mut()),
            Piece::Media(view) => {
                let h = media_height(*view, row.strip, px, inner);
                if let Some(out) = out.as_deref_mut() {
                    out.fill(x + pad, y, inner, h, px.v(10.0), ColorRole::CodeBackground);
                    out.widget(
                        WidgetKind::LabMedia {
                            card: row.path.clone(),
                            view: *view,
                        },
                        (x + pad, y, inner, h),
                        None,
                    );
                }
                y += h;
            }
        }
    }
    y += px.v(GAP + 2.0);
    let mut bx = x + pad;
    let bh = px.v(BUTTON);
    let mut buttons: Vec<(&PText, WidgetKind)> = Vec::new();
    if let Some(open) = &row.open {
        buttons.push((
            open,
            WidgetKind::LabOpen {
                card: row.path.clone(),
            },
        ));
    }
    buttons.push((
        &row.note,
        WidgetKind::LabNote {
            card: row.path.clone(),
        },
    ));
    buttons.push((
        &row.versions,
        WidgetKind::LabVersions {
            card: row.path.clone(),
        },
    ));
    for (i, (t, kind)) in buttons.into_iter().enumerate() {
        let bw = t.p.max_content_width().ceil() + px.v(24.0);
        // A button that doesn't fit beside the last one starts a new line.
        if i > 0 && bx + bw > x + pad + inner {
            bx = x + pad;
            y += bh + px.v(GAP);
        }
        if let Some(out) = out.as_deref_mut() {
            out.fill(bx, y, bw, bh, bh / 2.0, ColorRole::ChipBackground);
            place_text(
                t,
                bx + px.v(12.0),
                y + (bh - t.lh) / 2.0,
                bw,
                Some(&mut *out),
            );
            out.widget(kind, (bx, y, bw, bh), None);
        }
        bx += bw + px.v(GAP);
    }
    y += bh + pad;
    let h = y - top;
    if let (Some(out), Some(start)) = (out, start) {
        // The card's frame sits under everything drawn above.
        out.boxes.insert(
            start,
            super::display::BoxPrim {
                x,
                y: top,
                w: cw,
                h,
                radius: px.v(14.0),
                style: super::display::BoxStyle::Hairline,
                color: ColorRole::CodeBorder,
                scroller: None,
            },
        );
    }
    h
}

/// The key a live session's Lab card row goes by (#11948): its visual's
/// path under `lab-live:`, so it never collides with a saved `lab-card:`.
pub fn live_key(visual_path: &str) -> String {
    format!("{LIVE_PREFIX}{visual_path}")
}

pub const LIVE_PREFIX: &str = "lab-live:";

/// Cards for a transcript, by row path (`Arc` so inputs stay cheap to clone).
pub type LabCards = Arc<std::collections::HashMap<String, LabCardInput>>;
