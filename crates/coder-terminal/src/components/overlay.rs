//! A centered framed list over the screen: threads, plugins, help.
//!
//! The overlay draws into a [`Buffer`] over whatever the screen already
//! holds: it clears its box to the ladder's background, draws the hairline
//! with the title and the key hints set into its rules (clipped with "…"
//! when a rule cannot hold them), and keeps the selected row in view.
//! Under `NO_COLOR` there is no selection tint, so the selected row
//! reverses instead.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::{cells, clip, sanitize};
use crate::{Colors, Intensity, Ladder, frame, rail};

/// The widest an overlay draws, in cells.
pub const OVERLAY_WIDTH_MAX: u16 = 90;

/// One row of a list overlay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub detail: String,
}

/// A centered framed list over the screen: threads, plugins, help.
pub struct ListOverlay<'a> {
    pub title: &'a str,
    pub items: &'a [Item],
    pub selected: usize,
    /// Key hints on the bottom rule, e.g. "Enter open · n new · a archive ·
    /// Esc close".
    pub hint: &'a str,
    /// Shown when `items` is empty.
    pub empty: &'a str,
    pub ladder: Ladder,
}

impl ListOverlay<'_> {
    /// The box the overlay takes in `area`: `min(area.width - 4, 90)` wide
    /// and `min(area.height - 2, rows + 4)` tall, centered.
    pub fn bounds(&self, area: Rect) -> Rect {
        let rows = self.items.len().max(1);
        let width = area.width.saturating_sub(4).min(OVERLAY_WIDTH_MAX);
        let height = area
            .height
            .saturating_sub(2)
            .min(u16::try_from(rows + 4).unwrap_or(u16::MAX));
        Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        }
    }

    /// The first item shown when `visible` rows are on screen, so that
    /// `selected` is always among them.
    pub fn scroll(&self, visible: usize) -> usize {
        if visible == 0 || self.items.is_empty() {
            return 0;
        }
        let selected = self.selected.min(self.items.len() - 1);
        (selected + 1).saturating_sub(visible)
    }

    /// Draws into `buf`: clears a box of `min(area.width-4, 90)` x
    /// `min(area.height-2, items+4)` centered in `area` to the ladder's
    /// background, a hairline frame (Quarter) with the title (Full) on the
    /// top rule and the hint (Half) on the bottom rule; each item one row,
    /// "› label  detail" for the selected row (label Full on
    /// `ladder.selection()`, and reversed under `Colors::None`, which has
    /// no tint), "  label  detail" otherwise (label ThreeQuarters, detail
    /// Half, right-aligned when it fits, else after two spaces and clipped
    /// with "…"). Scrolls so `selected` is always visible. Never panics on
    /// tiny areas.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        let bounds = self.bounds(area);
        if bounds.is_empty() {
            return;
        }
        let ladder = self.ladder;
        let field = Style::new().bg(ladder.background());
        for y in bounds.top()..bounds.bottom() {
            for x in bounds.left()..bounds.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(field);
            }
        }
        frame(
            bounds,
            buf,
            ladder.style(Intensity::Quarter).bg(ladder.background()),
        );
        if bounds.height < 3 || bounds.width < 5 {
            return;
        }
        // A rail takes its text plus a space each side, inside a corner and
        // a rule cell at each end; text longer than that is clipped.
        let rule_room = usize::from(bounds.width).saturating_sub(6);
        let title = clip(&sanitize(self.title), rule_room);
        let hint = clip(&sanitize(self.hint), rule_room);
        rail(
            bounds,
            buf,
            0,
            Some((&title, ladder.style(Intensity::Full))),
            None,
        );
        rail(
            bounds,
            buf,
            bounds.height - 1,
            Some((&hint, ladder.style(Intensity::Half))),
            None,
        );

        // A blank row of padding under the title and over the hint, when
        // the box has room for it.
        let pad = u16::from(bounds.height >= 5);
        let top = bounds.top() + 1 + pad;
        let visible = usize::from(bounds.height - 2 - 2 * pad);
        // The text runs from two cells inside the left wall to two inside
        // the right; the selection band covers the whole inside.
        let left = bounds.left() + 2;
        let room = usize::from(bounds.width - 4);

        if self.items.is_empty() {
            if visible > 0 {
                let text = clip(&sanitize(self.empty), room);
                buf.set_string(left, top, text, ladder.style(Intensity::Half));
            }
            return;
        }

        let first = self.scroll(visible);
        let selected = self.selected.min(self.items.len() - 1);
        for (offset, item) in self.items.iter().skip(first).take(visible).enumerate() {
            let y = top + offset as u16;
            let chosen = first + offset == selected;
            let band = if chosen {
                let band = Style::new().bg(ladder.selection());
                if ladder.colors() == Colors::None {
                    band.add_modifier(Modifier::REVERSED)
                } else {
                    band
                }
            } else {
                field
            };
            for x in bounds.left() + 1..bounds.right() - 1 {
                buf[(x, y)].set_style(band);
            }
            let (lead, label_at) = if chosen {
                ("› ", Intensity::Full)
            } else {
                ("  ", Intensity::ThreeQuarters)
            };
            buf.set_stringn(left, y, lead, room, ladder.style(label_at));
            let room = room.saturating_sub(2);
            let label = clip(&sanitize(&item.label), room);
            let label_width = cells(&label);
            let x = left + 2;
            buf.set_stringn(x, y, &label, room, ladder.style(label_at));
            let detail = sanitize(&item.detail);
            let detail_width = cells(&detail);
            if detail.is_empty() {
                continue;
            }
            let detail_at = ladder.style(Intensity::Half);
            if label_width + 2 + detail_width <= room {
                let at = x + (room - detail_width) as u16;
                buf.set_string(at, y, &detail, detail_at);
            } else if room > label_width + 3 {
                let detail = clip(&detail, room - label_width - 2);
                buf.set_string(x + label_width as u16 + 2, y, &detail, detail_at);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item {
                label: format!("item {i}"),
                detail: format!("{i}m ago"),
            })
            .collect()
    }

    #[test]
    fn tiny_areas_never_panic() {
        let items = items(5);
        for colors in [Colors::True, Colors::Indexed, Colors::None] {
            for (width, height) in [(0, 0), (1, 1), (4, 2), (5, 3), (10, 3), (6, 5), (10, 4)] {
                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                for selected in [0, 3, 99] {
                    ListOverlay {
                        title: "threads",
                        items: &items,
                        selected,
                        hint: "Esc close",
                        empty: "none",
                        ladder: Ladder::new(colors),
                    }
                    .render(area, &mut buf);
                }
            }
        }
    }

    #[test]
    fn an_area_outside_the_buffer_draws_nothing() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
        let items = items(2);
        ListOverlay {
            title: "t",
            items: &items,
            selected: 0,
            hint: "",
            empty: "",
            ladder: Ladder::default(),
        }
        .render(Rect::new(40, 40, 30, 10), &mut buf);
    }

    #[test]
    fn the_selection_stays_in_view() {
        let items = items(50);
        let overlay = ListOverlay {
            title: "t",
            items: &items,
            selected: 30,
            hint: "",
            empty: "",
            ladder: Ladder::default(),
        };
        let first = overlay.scroll(10);
        assert!((first..first + 10).contains(&30));
        assert_eq!(overlay.scroll(100), 0);
    }
}
