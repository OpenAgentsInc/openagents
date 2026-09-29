//! The chrome around a body, and the whole slide as one grid.
//!
//! The body sits in the safe area, centered in the rows it does not fill.
//! The foot carries one hairline across the canvas, filled to the deck's
//! progress, with the slide's place in the deck sitting on the rule at the
//! right the way a badge sits on a rule.

use crate::canvas::{Canvas, PAD_COLS, PAD_ROWS};
use crate::grid::{Grid, PressId, Style};
use crate::layouts;
use crate::prose;
use crate::slide::{Deck, Layout};
use coder_ui::theme::Intensity;

/// The cells between the progress rule and the slide's number.
const FOOT_GAP: usize = 1;

/// The slide at `index` of `deck`, laid out whole on `canvas`: the grid
/// the window paints, the snapshots hold, and the text export prints.
pub fn slide_grid(deck: &Deck, index: usize, canvas: Canvas) -> Grid {
    let mut grid = Grid::new(canvas.cells, canvas.rows);
    let Some(slide) = deck.slide(index) else {
        return grid;
    };
    let body = layouts::body(slide, canvas);
    let note = slide.note.clone().unwrap_or_default();
    let width = canvas.body_cells();
    // A banner centers its wordmark and its lead, so its note centers too,
    // laid out at its own length; every other layout keeps the note on the
    // body's left edge at the body's width.
    let (left, measure) = if slide.layout == Some(Layout::Banner) {
        let length = note.chars().count().min(width);
        (PAD_COLS + (width - length) / 2, length.max(1))
    } else {
        (PAD_COLS, width)
    };
    let laid = prose::text(&note, Style::at(Intensity::Half), measure);
    let note_rows = if note.is_empty() {
        0
    } else {
        laid.height() + 1
    };
    let room = canvas.body_rows();
    let top = PAD_ROWS + room.saturating_sub(body.height() + note_rows) / 2;
    grid.blit(PAD_COLS, top, &body);
    if !note.is_empty() {
        grid.blit(left, top + body.height() + 1, &laid);
    }
    grid.truncate(canvas.rows);
    grid.grow(canvas.rows);
    foot(&mut grid, canvas, index, deck.len());
    grid
}

/// The foot: the progress rule, and the slide's place on it.
fn foot(grid: &mut Grid, canvas: Canvas, index: usize, total: usize) {
    let row = canvas.foot_row();
    let place = format!(" {} / {} ", index + 1, total.max(1));
    let width = canvas.cells;
    grid.hrule(0, row, width, Style::at(Intensity::Quarter), false);
    let done = ((index + 1) * width) / total.max(1);
    if done > 0 {
        grid.hrule(0, row, done, Style::at(Intensity::Half), false);
    }
    let at = width.saturating_sub(place.chars().count() + FOOT_GAP);
    // The place sits on the rule the way a badge does: the rule stops
    // either side of it.
    grid.put_str(at, row, &place, Style::at(Intensity::Half));
}

/// The presenter's note for the slide at `index`, wrapped at `width`
/// cells at half intensity, or an empty grid when the slide carries none.
pub fn notes_grid(deck: &Deck, index: usize, width: usize) -> Grid {
    let text = deck
        .slide(index)
        .map(|slide| slide.note_text())
        .unwrap_or_default();
    if text.is_empty() {
        return prose::text("No presenter note.", Style::at(Intensity::Quarter), width);
    }
    prose::text(&text, Style::at(Intensity::Half), width)
}

/// The columns the overview lays its cards out in.
const OVERVIEW_COLUMNS: usize = 3;

/// The rows one card takes, its frame and the gap under it.
const CARD_ROWS: usize = 3;

/// The overview: one card a slide, the current one at full intensity. Each
/// card's cells carry the press id of its slide's place in the deck, so a
/// click on a card opens that slide.
pub fn overview(deck: &Deck, current: usize, canvas: Canvas) -> Grid {
    let mut grid = Grid::new(canvas.cells, canvas.rows);
    let card = canvas.body_cells() / OVERVIEW_COLUMNS;
    for (index, slide) in deck.slides.iter().enumerate() {
        let column = index % OVERVIEW_COLUMNS;
        let row = index / OVERVIEW_COLUMNS;
        let left = PAD_COLS + column * card;
        let top = 1 + row * CARD_ROWS;
        if top + 3 > canvas.rows - 1 {
            break;
        }
        let here = index == current;
        let press = Some(PressId(index as u32));
        let style = Style::at(if here {
            Intensity::Full
        } else {
            Intensity::Quarter
        })
        .press(press);
        grid.frame(left, top, card - 1, 3, style, false);
        let title = slide
            .title
            .clone()
            .or_else(|| slide.lead.clone())
            .or_else(|| {
                let words: Vec<&str> = slide.body.split_whitespace().collect();
                (!words.is_empty()).then(|| words.join(" "))
            })
            .unwrap_or_else(|| slide.id.clone());
        let label = format!("{:>2} {}", index + 1, title);
        let label = crop(&label, card.saturating_sub(5));
        grid.put_str(
            left + 2,
            top + 1,
            &label,
            Style::at(if here {
                Intensity::Full
            } else {
                Intensity::Half
            })
            .press(press),
        );
    }
    grid
}

/// The slide a press on the overview opens, when the cell at `col`, `row`
/// belongs to a card.
pub fn overview_press(grid: &Grid, col: usize, row: usize) -> Option<usize> {
    grid.get(col, row)
        .and_then(|cell| cell.style.press)
        .map(|PressId(id)| id as usize)
        .or_else(|| {
            // A press inside a card's frame lands on blank cells, which carry
            // no id; find the card whose bounds hold the cell.
            grid.press_ids().into_iter().find_map(|id| {
                let (cols, rows) = grid.press_bounds(id)?;
                (cols.contains(&col) && rows.contains(&row)).then_some(id.0 as usize)
            })
        })
}

/// `text` cut to `room` cells.
fn crop(text: &str, room: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= room || room == 0 {
        return text.to_string();
    }
    let mut cut: String = chars[..room.saturating_sub(1)].iter().collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script;

    fn deck() -> Deck {
        script::parse(
            "layout: banner\nid: title\ntitle: TTC\nlead: The one line\nnotes: Say it\n\n---\n\n\
             layout: statement\nid: sentence\n\nOne sentence.\n",
        )
        .expect("the script parses")
    }

    /// A slide fills the canvas, keeps its body inside the safe area, and
    /// carries its place in the foot.
    #[test]
    fn a_slide_fills_the_canvas_and_names_its_place() {
        let grid = slide_grid(&deck(), 0, Canvas::DEFAULT);
        assert_eq!(grid.width(), Canvas::DEFAULT.cells);
        assert_eq!(grid.height(), Canvas::DEFAULT.rows);
        let foot: String = grid
            .runs(Canvas::DEFAULT.foot_row())
            .iter()
            .map(|run| run.text.clone())
            .collect();
        assert!(foot.contains("1 / 2"), "the foot reads {foot}");
    }

    /// The overview draws a card a slide, and a press inside a card opens
    /// its slide.
    #[test]
    fn the_overview_draws_a_card_a_slide() {
        let deck = deck();
        let grid = overview(&deck, 1, Canvas::DEFAULT);
        assert_eq!(grid.press_ids().len(), 2);
        let (cols, rows) = grid.press_bounds(PressId(1)).expect("the second card");
        assert_eq!(
            overview_press(&grid, cols.start + 1, rows.start + 1),
            Some(1)
        );
        assert!(grid.to_text().contains("TTC"));
    }

    /// The notes grid holds the presenter's note.
    #[test]
    fn the_notes_hold_the_presenters_note() {
        assert!(notes_grid(&deck(), 0, 40).to_text().contains("Say it"));
        assert!(
            notes_grid(&deck(), 1, 40)
                .to_text()
                .contains("No presenter note")
        );
    }
}
