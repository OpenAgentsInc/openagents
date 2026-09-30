//! The OpenAgents deck, built from the desktop app's Rust Native pieces.
//!
//! A slide is a handful of [`rust_native::Node`]s (text, Markdown prose,
//! lists and tables, cards) laid out by the same row layout as the desktop
//! and mobile chats and painted by the desktop's transcript painter, with
//! the same fonts and the same colors ([`rust_native_desktop::Theme::openagents`]).
//! The window is [`rust_native_desktop::window`]. The deck keeps only its
//! slides and how a presenter moves through them; improving the shared
//! pieces improves the deck and the desktop app together.
//!
//! The modules, in the order a reader meets them:
//!
//! - [`slide`]: what a slide is, and the deck that holds them.
//! - [`script`]: the parser for the scripts under `decks/`, where the copy
//!   lives, one file a deck.
//! - [`compose`]: a slide as Rust Native nodes, and where each sits on the
//!   canvas.
//! - [`viewer`]: the presentation as a view another Rust Native app can
//!   embed: the keys, the overview, the notes, the black screen, and the
//!   painting, in whatever rectangle the host gives. [`decks`] lists the
//!   decks a host can open.
//! - [`present`]: the presentation as its own window, a thin
//!   [`rust_native_desktop::App`] around a [`Viewer`].
//! - [`snapshot`]: the golden outlines this crate's tests check.

pub mod compose;
pub mod present;
pub mod script;
pub mod slide;
pub mod snapshot;
pub mod viewer;

pub use compose::{Composed, compose};
pub use present::Presenter;
pub use slide::{Deck, Layout, Metric, Slide};
pub use viewer::{DeckEntry, Outcome, UnknownDeck, Viewer, decks};

#[cfg(test)]
mod tests {
    use super::*;
    use compose::{CONTENT_BOTTOM, MARGIN_X, MARGIN_Y, WIDTH};

    /// Every script parses, every slide names a layout and a source, no
    /// two slides in one deck share an id, and every image a slide shows
    /// is compiled in, decodes, and has alternative text.
    #[test]
    fn every_script_parses_into_named_slides() {
        for (name, source) in slide::SCRIPTS {
            let deck = script::parse(source).unwrap_or_else(|complaint| {
                panic!("the {name} script does not parse: {complaint}")
            });
            assert!(
                (1..=40).contains(&deck.len()),
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
                for image in slide.image.iter().chain(&slide.images) {
                    let bytes = slide::asset(&image.path).unwrap_or_else(|| {
                        panic!(
                            "{name}/{} shows {}, which is not in ASSETS",
                            slide.id, image.path
                        )
                    });
                    rust_native_desktop::image::Image::png(bytes)
                        .unwrap_or_else(|e| panic!("{name}/{}: {e}", slide.id));
                    assert!(!image.alt.is_empty(), "{name}/{} has no alt text", slide.id);
                }
            }
        }
    }

    /// Every part of every slide stays inside the canvas's margins (an image
    /// may use the full width), and nothing is so wide that it scrolls
    /// sideways.
    #[test]
    fn every_slide_fits_its_canvas() {
        for deck in Deck::all() {
            for (index, slide) in deck.slides.iter().enumerate() {
                let composed = compose(&deck, index);
                for part in &composed.parts {
                    let at = format!("{}/{} {}", deck.name, slide.id, part.kind);
                    assert!(!part.scrolls(), "{at} scrolls sideways");
                    // Text keeps the margins; an image may reach wider, as
                    // a gallery does, but never off the canvas.
                    let margin = if part.kind == "image" { 0.0 } else { MARGIN_X };
                    assert!(part.x >= margin - 0.5, "{at} starts at {}", part.x);
                    let right = part.x + part.natural_width();
                    assert!(right <= WIDTH - margin + 0.5, "{at} ends at {right}");
                    if part.kind == "foot" {
                        continue;
                    }
                    assert!(part.y >= MARGIN_Y - 0.5, "{at} starts at {}", part.y);
                    assert!(
                        part.y + part.height() <= CONTENT_BOTTOM + 0.5,
                        "{at} runs to {} of {CONTENT_BOTTOM}",
                        part.y + part.height()
                    );
                }
            }
        }
    }

    /// The deck paints white and gradations of white on the desktop's
    /// near-black, never a warm hue such as the amber it once had: every
    /// pixel of every slide is a cool or neutral gray.
    #[test]
    fn every_slide_is_painted_in_grays() {
        for deck in Deck::all() {
            let mut presenter = Presenter::new(deck.clone(), 0);
            for index in 0..deck.len() {
                // An image keeps its own colors; the test is of the deck's.
                if deck.slides[index].image.is_some() || !deck.slides[index].images.is_empty() {
                    continue;
                }
                presenter.key("Home", false);
                for _ in 0..index {
                    presenter.key("ArrowRight", false);
                }
                let frame = present::capture(&mut presenter, 400, 225);
                for pixel in frame.pixels.chunks_exact(4) {
                    let (r, g, b) = (pixel[0] as i32, pixel[1] as i32, pixel[2] as i32);
                    let spread = r.max(g).max(b) - r.min(g).min(b);
                    assert!(
                        spread <= 24 && b + 4 >= r,
                        "{}/{} paints {r},{g},{b}",
                        deck.name,
                        deck.slides[index].id
                    );
                }
            }
        }
    }

    /// Every slide of every deck matches its outline snapshot, and every
    /// snapshot belongs to a slide of the deck it is filed under.
    #[test]
    fn every_slide_matches_its_snapshot() {
        for deck in Deck::all() {
            let mut drawn = Vec::new();
            for (index, slide) in deck.slides.iter().enumerate() {
                snapshot::check(&deck.name, &slide.id, &compose(&deck, index).outline());
                drawn.push(slide.id.clone());
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
