//! The OpenAgents deck: slides laid out on a cell grid.
//!
//! Ported from the Coder repository's `coder-deck`. A slide is a
//! [`Grid`](grid::Grid) of cells on one fixed canvas, 108 cells by 28 rows,
//! which is 16:9 at JetBrains Mono's metrics. The window paints that grid,
//! the golden snapshots under `snapshots/` hold it, and
//! `openagents-deck --text` prints it.
//!
//! The modules, in the order a reader meets them:
//!
//! - [`canvas`]: the canvas and its safe area.
//! - [`grid`]: the cell grid, ported from Coder's component core.
//! - [`slide`]: what a slide is, and the deck that holds them.
//! - [`script`]: the parser for the scripts under `decks/`, where the copy
//!   lives, one file a deck.
//! - [`prose`]: the Markdown a slide's body may hold.
//! - [`banner`]: the block face the title slide and big numbers draw in.
//! - [`layouts`]: one body grid per layout.
//! - [`frame`]: the chrome around a body, the overview, and the notes.
//! - [`reveal`]: the arrival, as a transform over a finished grid.
//! - [`paint`]: the grid in pixels, for the window and the PNG capture.
//! - [`snapshot`]: the golden-snapshot check this crate's tests run.

pub mod banner;
pub mod canvas;
pub mod frame;
pub mod grid;
pub mod layouts;
pub mod paint;
pub mod prose;
pub mod reveal;
pub mod script;
pub mod slide;
pub mod snapshot;

pub use canvas::Canvas;
pub use frame::{notes_grid, overview, overview_press, slide_grid};
pub use grid::Grid;
pub use reveal::{glyphs, reveal};
pub use slide::{Deck, Layout, Metric, Slide};

#[cfg(test)]
mod tests {
    use super::*;
    use canvas::{PAD_COLS, PAD_ROWS};

    /// The slides of one deck, laid out whole at the canvas.
    fn slides(deck: &Deck) -> Vec<(String, Grid)> {
        (0..deck.len())
            .map(|index| {
                let id = deck.slide(index).expect("the slide").id.clone();
                (id, slide_grid(deck, index, Canvas::DEFAULT, 1.0))
            })
            .collect()
    }

    /// Every script parses, every slide names a layout and a source, and
    /// no two slides in one deck share an id.
    #[test]
    fn every_script_parses_into_named_slides() {
        for (name, source) in slide::SCRIPTS {
            let deck = script::parse(source).unwrap_or_else(|complaint| {
                panic!("the {name} script does not parse: {complaint}")
            });
            assert!(
                (12..=20).contains(&deck.len()),
                "the {name} deck holds {} slides",
                deck.len()
            );
            let mut ids: Vec<&str> = deck.slides.iter().map(|slide| slide.id.as_str()).collect();
            ids.sort_unstable();
            let count = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), count, "two slides in {name} share an id");
            for slide in &deck.slides {
                assert!(
                    slide.layout.is_some(),
                    "{name}/{} names no layout",
                    slide.id
                );
                assert!(
                    slide.source.is_some(),
                    "{name}/{} names no source",
                    slide.id
                );
                assert!(
                    !slide.notes.is_empty(),
                    "{name}/{} carries no notes",
                    slide.id
                );
            }
        }
    }

    /// Every slide of every deck draws inside the safe area: nothing in
    /// the padding columns, nothing above the first body row, and nothing
    /// in the padding rows above the foot. The foot is the one row that
    /// runs the whole width.
    #[test]
    fn every_slide_stays_inside_the_safe_area() {
        let canvas = Canvas::DEFAULT;
        for deck in Deck::all() {
            for (id, grid) in slides(&deck) {
                assert_eq!(grid.height(), canvas.rows, "{}/{id} grew", deck.name);
                for row in 0..canvas.foot_row() {
                    for col in 0..canvas.cells {
                        let inside = (PAD_COLS..canvas.cells - PAD_COLS).contains(&col)
                            && (PAD_ROWS..canvas.rows - PAD_ROWS).contains(&row);
                        if inside {
                            continue;
                        }
                        let blank = grid.get(col, row).is_none_or(grid::Cell::is_blank);
                        assert!(
                            blank,
                            "{}/{id} draws at {col},{row}, outside the safe area",
                            deck.name
                        );
                    }
                }
            }
        }
    }

    /// Every slide keeps to one full-intensity element: something draws
    /// at full, and on a titled slide either the title does or the body
    /// does, never both.
    #[test]
    fn every_slide_keeps_one_full_element() {
        use coder_ui::theme::Intensity;
        for deck in Deck::all() {
            for slide in &deck.slides {
                let body = layouts::body(slide, Canvas::DEFAULT);
                assert!(
                    layouts::has_full(&body),
                    "{}/{} has nothing at full intensity",
                    deck.name,
                    slide.id
                );
                let titled = slide.title.is_some()
                    && !matches!(
                        slide.layout(),
                        Layout::Banner | Layout::Quote | Layout::Statement
                    );
                if !titled {
                    continue;
                }
                let title_row = usize::from(slide.kicker.is_some());
                let title_full = body
                    .get(0, title_row)
                    .is_some_and(|cell| cell.style.intensity == Intensity::Full);
                let rest_full = body.rows().enumerate().any(|(row, cells)| {
                    row != title_row
                        && cells
                            .iter()
                            .any(|cell| !cell.is_blank() && cell.style.intensity == Intensity::Full)
                });
                assert!(
                    title_full != rest_full,
                    "{}/{} draws its title and its body at full intensity",
                    deck.name,
                    slide.id
                );
            }
        }
    }

    /// Every slide of every deck matches its snapshot, and every snapshot
    /// belongs to a slide of the deck it is filed under.
    #[test]
    fn every_slide_matches_its_snapshot() {
        for deck in Deck::all() {
            let mut drawn = Vec::new();
            for (id, grid) in slides(&deck) {
                snapshot::check(&deck.name, &id, &grid);
                drawn.push(id);
            }
            if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
                continue;
            }
            for name in snapshot::names(&deck.name) {
                assert!(
                    drawn.contains(&name),
                    "the {}/{name} snapshot belongs to no slide",
                    deck.name
                );
            }
        }
    }
}
