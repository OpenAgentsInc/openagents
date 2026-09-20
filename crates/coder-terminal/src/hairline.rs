//! The hairline frame, and the text that rides its rules.
//!
//! Every terminal in this repository draws the same box: a one-cell rule
//! around the content, and short labels set into the top and bottom rules
//! rather than above or below them. [`frame`] draws the box and [`rail`]
//! writes the labels, so a composer and a table of verdicts share one
//! drawing rather than two that drift.
//!
//! Both take a ratatui [`Buffer`] and a [`Rect`], and both take a style per
//! call: the frame is the geometry, and which step of the amber ladder it
//! burns at is the caller's decision. Neither touches a cell inside the
//! box, so what sits in the gutter between the wall and the text — a
//! selection cursor, say — belongs to whoever draws the content.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

/// The hairline: `─` rules top and bottom, `│` walls, corners `┌┐└┘`.
///
/// An area too small to hold a rule and two corners is left alone rather
/// than half drawn.
pub fn frame(area: Rect, buf: &mut Buffer, style: Style) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let (top, bottom) = (area.top(), area.bottom() - 1);
    let (left, right) = (area.left(), area.right() - 1);
    for x in left + 1..right {
        buf[(x, top)].set_char('─').set_style(style);
        buf[(x, bottom)].set_char('─').set_style(style);
    }
    for y in top + 1..bottom {
        buf[(left, y)].set_char('│').set_style(style);
        buf[(right, y)].set_char('│').set_style(style);
    }
    buf[(left, top)].set_char('┌').set_style(style);
    buf[(right, top)].set_char('┐').set_style(style);
    buf[(left, bottom)].set_char('└').set_style(style);
    buf[(right, bottom)].set_char('┘').set_style(style);
}

/// Writes `left` and `right` into the rule at `offset`, each padded with a
/// space on both sides so the text never touches a corner.
///
/// A rail wider than the room left is dropped rather than cut: the left
/// rail is written first and takes what it needs, and the right rail draws
/// only if what remains holds it whole. Each side carries its own style,
/// because the two rails often say things of different weight — a view's
/// name against a chain that failed to verify.
pub fn rail(
    area: Rect,
    buf: &mut Buffer,
    offset: u16,
    left: Option<(&str, Style)>,
    right: Option<(&str, Style)>,
) {
    let y = area.top() + offset;
    if y >= buf.area.bottom() || area.width < 6 {
        return;
    }
    // The room the two rails share: the width less the corners and the rule
    // cell inside each.
    let mut room = usize::from(area.width) - 4;
    if let Some((text, style)) = left {
        let named = format!(" {text} ");
        let taken = named.width();
        if taken <= room {
            room -= taken;
            buf.set_string(area.left() + 2, y, &named, style);
        }
    }
    if let Some((text, style)) = right {
        let named = format!(" {text} ");
        let taken = named.width();
        if taken <= room {
            buf.set_string(area.right() - 2 - taken as u16, y, &named, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn text(buf: &Buffer) -> Vec<String> {
        let area = buf.area;
        (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buf[(x, y)].symbol().to_owned())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_frame_draws_its_rules_and_its_corners() {
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        frame(area, &mut buf, Style::new());
        assert_eq!(text(&buf), ["┌────┐", "│    │", "└────┘"]);
    }

    #[test]
    fn an_area_too_small_for_a_frame_is_left_alone() {
        for (width, height) in [(0, 0), (1, 3), (3, 1), (1, 1)] {
            let area = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(area);
            frame(area, &mut buf, Style::new());
            assert!(text(&buf).iter().all(|line| line.trim().is_empty()));
        }
    }

    #[test]
    fn each_rail_carries_its_own_style() {
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        frame(area, &mut buf, Style::new());
        let loud = Style::new().add_modifier(Modifier::BOLD);
        let quiet = Style::new().fg(Color::Indexed(94));
        rail(
            area,
            &mut buf,
            0,
            Some(("left", quiet)),
            Some(("right", loud)),
        );
        assert_eq!(text(&buf)[0], "┌─ left ─── right ─┐");
        assert_eq!(buf[(3u16, 0u16)].fg, Color::Indexed(94));
        assert!(buf[(14u16, 0u16)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn a_rail_the_rule_cannot_hold_whole_is_left_out() {
        let area = Rect::new(0, 0, 12, 3);
        let mut buf = Buffer::empty(area);
        frame(area, &mut buf, Style::new());
        rail(
            area,
            &mut buf,
            0,
            Some(("a name far too long", Style::new())),
            Some(("fits", Style::new())),
        );
        let line = &text(&buf)[0];
        assert!(!line.contains("name"), "{line}");
        assert!(line.contains(" fits "), "{line}");
    }

    #[test]
    fn a_rail_outside_the_buffer_draws_nothing() {
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        rail(area, &mut buf, 9, Some(("gone", Style::new())), None);
        assert!(text(&buf).iter().all(|line| line.trim().is_empty()));
    }
}
