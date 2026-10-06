//! Fixed-cell presentation for terminal runs mounted on phone GPU surfaces.
//! The linked host owns the shell; this module only lays out and draws text.

use coder_vt::Cell;
use unicode_width::UnicodeWidthChar;
use verse_gfx::ui::{Atlas, UiBatch};

/// Clip a semantic terminal run to whole cells. Combining marks remain with
/// their base, and a wide character never occupies half of the last cell.
#[must_use]
pub fn cells(text: &str, columns: usize) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut used = 0;
    for ch in text.chars() {
        let width = ch.width().unwrap_or(0).min(2);
        if width == 0 {
            if let Some(cell) = cells.last_mut() {
                cell.combining.push(ch);
            }
            continue;
        }
        if used + width > columns {
            break;
        }
        cells.push(Cell {
            ch,
            width: width as u8,
            ..Cell::default()
        });
        used += width;
    }
    cells
}

#[must_use]
pub fn columns(cells: &[Cell]) -> usize {
    cells.iter().map(|cell| usize::from(cell.width)).sum()
}

/// Draw with the same box, wide-glyph, and combining-mark renderer as the
/// desktop terminal. The caller supplies its admitted run's color.
pub fn draw(batch: &mut UiBatch, atlas: &Atlas, origin: [f32; 2], cells: &[Cell], color: [f32; 4]) {
    let mut x = origin[0];
    for cell in cells {
        crate::draw::glyph(batch, atlas, x, origin[1], cell, color);
        x += f32::from(cell.width) * atlas.advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_and_combining_text_keeps_terminal_columns_and_clips_whole_glyphs() {
        let row = cells("a\u{301}界b", 4);
        assert_eq!(columns(&row), 4);
        assert_eq!(row[0].combining, ['\u{301}']);
        assert_eq!(row[1].width, 2);
        assert_eq!(cells("界x", 1).len(), 0);
        assert_eq!(columns(&cells("a界x", 3)), 3);
    }
}
