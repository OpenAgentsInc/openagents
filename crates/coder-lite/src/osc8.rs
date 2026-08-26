//! OSC 8 hyperlink emission.
//!
//! ratatui paints cells; it has no notion of a hyperlink. A terminal learns
//! that a run of cells is clickable from the OSC 8 escape sequence, which must
//! be written around the text as it is emitted. So coder-lite repaints just the
//! link runs after the frame is flushed: move the cursor to the run, wrap it in
//! `ESC ] 8 ; id=… ; URL ESC \`, write the same characters the frame already
//! shows, and close with an empty OSC 8.
//!
//! The characters written are read back out of the rendered buffer, never
//! synthesized. A hyperlink pass can therefore change what a cell *links to*,
//! but it can never change what a cell *says*.

use std::io::Write;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::transcript::ScreenLink;

/// One link run resolved to absolute terminal coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedLink {
    pub x: u16,
    pub y: u16,
    pub url: String,
    pub id: u32,
    /// Width of the run in cells.
    pub width: u16,
}

/// Place transcript-local links onto the screen.
///
/// `links` are addressed by row within the transcript's wrapped lines;
/// `scroll` is the first visible row and `area` the transcript rectangle.
/// Links scrolled out of view, or starting past the right edge, are dropped.
/// Runs are clipped to the area rather than allowed to spill.
pub fn place(links: &[ScreenLink], area: Rect, scroll: usize) -> Vec<PlacedLink> {
    let mut out = Vec::new();
    for link in links {
        if link.row < scroll {
            continue;
        }
        let rel = link.row - scroll;
        if rel >= area.height as usize {
            continue;
        }
        if link.col_start >= area.width as usize {
            continue;
        }
        let end = link.col_end.min(area.width as usize);
        if end <= link.col_start {
            continue;
        }
        out.push(PlacedLink {
            x: area.x + link.col_start as u16,
            y: area.y + rel as u16,
            url: link.url.clone(),
            id: link.id,
            width: (end - link.col_start) as u16,
        });
    }
    out
}

/// Escape sequences that re-paint `placed` as OSC 8 hyperlinks.
///
/// The text comes from `buf`, so this reproduces what is already on screen.
/// A run whose cells are not in the buffer is skipped.
///
/// URLs containing control characters, `ESC`, or the OSC terminators are
/// dropped: a URL that can close its own escape sequence could rewrite the
/// rest of the screen.
pub fn sequences(placed: &[PlacedLink], buf: &Buffer) -> String {
    let mut out = String::new();
    for link in placed {
        if !url_is_safe(&link.url) {
            continue;
        }
        let mut text = String::new();
        let mut ok = true;
        for dx in 0..link.width {
            match buf.cell((link.x + dx, link.y)) {
                Some(cell) => text.push_str(cell.symbol()),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok || text.is_empty() {
            continue;
        }
        // Cursor position is 1-based in CUP.
        out.push_str(&format!("\x1b[{};{}H", link.y + 1, link.x + 1));
        out.push_str(&format!("\x1b]8;id={};{}\x1b\\", link.id, link.url));
        out.push_str(&text);
        out.push_str("\x1b]8;;\x1b\\");
    }
    out
}

/// Reject URLs that could break out of the escape sequence.
fn url_is_safe(url: &str) -> bool {
    !url.is_empty()
        && url.len() <= 2048
        && !url
            .chars()
            .any(|c| c.is_control() || c == '\u{9c}' || c == '\u{7f}')
}

/// Write the hyperlink pass for one frame.
pub fn emit<W: Write>(w: &mut W, placed: &[PlacedLink], buf: &Buffer) -> std::io::Result<()> {
    let seq = sequences(placed, buf);
    if seq.is_empty() {
        return Ok(());
    }
    w.write_all(seq.as_bytes())?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    fn buffer_with(text: &str, width: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
        buf.set_line(0, 0, &Line::from(text.to_string()), width);
        buf
    }

    fn link(row: usize, col_start: usize, col_end: usize, url: &str) -> ScreenLink {
        ScreenLink {
            row,
            col_start,
            col_end,
            url: url.to_string(),
            id: 1,
        }
    }

    #[test]
    fn scrolled_out_links_are_dropped() {
        let area = Rect::new(0, 0, 40, 3);
        let links = vec![
            link(0, 0, 4, "https://a.test/"),
            link(9, 0, 4, "https://b.test/"),
        ];
        let placed = place(&links, area, 1);
        assert!(placed.is_empty(), "{placed:?}");
    }

    #[test]
    fn visible_links_get_screen_coordinates() {
        let area = Rect::new(2, 5, 40, 3);
        let placed = place(&[link(4, 3, 12, "https://a.test/")], area, 3);
        assert_eq!(placed.len(), 1);
        assert_eq!((placed[0].x, placed[0].y, placed[0].width), (5, 6, 9));
    }

    #[test]
    fn runs_are_clipped_to_the_area_width() {
        let area = Rect::new(0, 0, 10, 1);
        let placed = place(&[link(0, 6, 40, "https://a.test/")], area, 0);
        assert_eq!(placed[0].width, 4);
    }

    #[test]
    fn sequence_repaints_the_text_that_is_already_on_screen() {
        let buf = buffer_with("see Buildkite now", 20);
        let placed = place(&[link(0, 4, 13, "https://buildkite.com/")], buf.area, 0);
        let seq = sequences(&placed, &buf);
        assert!(seq.contains("\x1b]8;id=1;https://buildkite.com/\x1b\\"));
        assert!(seq.contains("Buildkite"));
        assert!(seq.ends_with("\x1b]8;;\x1b\\"));
        // Nothing beyond the run leaks into the repaint.
        assert!(!seq.contains("now"));
    }

    #[test]
    fn urls_carrying_escapes_are_refused() {
        let buf = buffer_with("click here", 20);
        let placed = place(
            &[link(0, 0, 5, "https://a.test/\x1b]8;;\x1b\\evil")],
            buf.area,
            0,
        );
        assert_eq!(sequences(&placed, &buf), "");
    }
}
