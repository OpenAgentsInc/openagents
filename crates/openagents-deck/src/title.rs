//! The opening slide's title, set large.
//!
//! The grid has one type size, and the old opening drew its headline out of
//! cells in a block face, which reads badly at a distance. A [`Layout::Title`]
//! slide keeps its title in the grid at body size (so the text export and
//! the overview show it), and the painter sets it again here in the same
//! face at a whole multiple of the body size, over the rows the layout
//! reserved for it. The window and the PNG capture both call [`paint`]
//! after painting the grid, so the screenshot is the frame the window shows.

use crate::grid::{Grid, Style};
use crate::layouts::title_scale;
use crate::paint::{FIELD, Frame, Painter};
use crate::slide::{Layout, Slide};
use coder_ui::theme::Intensity;

/// Where a title slide's title sits in `grid`: the column it starts at,
/// its row, and the scale it draws at; `None` for any other slide or when
/// the title isn't in the grid.
pub fn locate(slide: &Slide, grid: &Grid) -> Option<(usize, usize, usize)> {
    if slide.layout() != Layout::Title {
        return None;
    }
    let title: Vec<char> = slide.title.as_deref()?.chars().collect();
    if title.is_empty() {
        return None;
    }
    let scale = title_scale(slide.title.as_deref()?, grid.width());
    for (row, cells) in grid.rows().enumerate() {
        let glyphs: Vec<char> = cells.iter().map(|cell| cell.glyph).collect();
        if let Some(col) = glyphs
            .windows(title.len())
            .position(|window| window == title.as_slice())
        {
            return Some((col, row, scale));
        }
    }
    None
}

/// Sets `slide`'s title large over `frame`, which already holds `grid`
/// painted by `painter` with its top-left corner at `x`, `y`. Any other
/// slide is left as painted.
pub fn paint(frame: &mut Frame, painter: &Painter, slide: &Slide, grid: &Grid, x: f32, y: f32) {
    let Some((col, row, scale)) = locate(slide, grid) else {
        return;
    };
    let title = slide.title.clone().unwrap_or_default();
    let length = title.chars().count();
    let (cw, ch) = (painter.cell_width(), painter.cell_height());
    // Erase the body-size title.
    let x0 = (x + col as f32 * cw).round() as i64;
    let x1 = (x + (col + length) as f32 * cw).round() as i64;
    let y0 = (y + row as f32 * ch).round() as i64;
    let y1 = (y + (row + 1) as f32 * ch).round() as i64;
    frame.fill(x0, y0, x1, y1, FIELD);
    // The block starts scale / 2 rows above the row the layout put the
    // title on, as `title_body` placed it.
    let block = row.saturating_sub(scale / 2);
    let mut large = Painter::new(painter.size() * scale as f32);
    let mut line = Grid::new(length, 1);
    line.put_str(0, 0, &title, Style::at(Intensity::Full));
    let width = length as f32 * large.cell_width();
    let canvas = grid.width() as f32 * cw;
    let left = x + ((canvas - width) / 2.0).round();
    let top = y + block as f32 * ch;
    large.paint(frame, &line, left, top);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use crate::script;
    use crate::slide::Deck;

    fn deck() -> Deck {
        script::parse(
            "layout: title\nid: title\ntitle: Test-Time Capabilities\nlead: One line\n\n---\n\n\
             layout: statement\nid: sentence\n\nOne sentence.\n",
        )
        .expect("the script parses")
    }

    #[test]
    fn a_title_slide_is_found_at_its_scale_and_other_slides_are_not() {
        let deck = deck();
        let grid = crate::slide_grid(&deck, 0, Canvas::DEFAULT);
        let (col, row, scale) = locate(deck.slide(0).unwrap(), &grid).expect("the title");
        assert_eq!(scale, 4);
        assert!(col > 0 && row > 0);
        let other = crate::slide_grid(&deck, 1, Canvas::DEFAULT);
        assert!(locate(deck.slide(1).unwrap(), &other).is_none());
    }

    #[test]
    fn the_large_title_is_painted_over_the_erased_small_one() {
        let deck = deck();
        let canvas = Canvas::DEFAULT;
        let grid = crate::slide_grid(&deck, 0, canvas);
        let mut painter = Painter::new(12.0);
        let (w, h) = painter.extent(canvas.cells, canvas.rows);
        let mut frame = Frame::new(w as usize, h as usize, FIELD);
        painter.paint(&mut frame, &grid, 0.0, 0.0);
        let lit = |frame: &Frame| frame.pixels.chunks(4).filter(|p| p[0] > 128).count();
        let before = lit(&frame);
        paint(
            &mut frame,
            &painter,
            deck.slide(0).unwrap(),
            &grid,
            0.0,
            0.0,
        );
        let after = lit(&frame);
        assert!(
            after > before * 4,
            "the large title lights many more pixels: {before} -> {after}"
        );
    }
}
