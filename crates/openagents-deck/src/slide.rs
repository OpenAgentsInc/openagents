//! What a slide is, and the deck that holds them.
//!
//! The copy lives under `decks/`, one Markdown file a deck, and
//! [`Deck::named`] parses the one a presenter asks for, so changing a
//! sentence changes a Markdown file rather than a layout. A slide names one
//! [`Layout`], which decides how its parts draw, and carries the source its
//! facts come from.

use crate::script;

/// The nine shapes a slide draws in. The Coder deck's ninth, `live`,
/// draws Coder's own product screens and is not carried over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// The wordmark in the block face, over one line.
    Banner,
    /// The title in ordinary type set several times larger than the body,
    /// centered, with a lead under it: the opening slide.
    Title,
    /// One sentence, centered, alone on the canvas.
    Statement,
    /// A title and up to five items, each a lead phrase and its
    /// continuation.
    Points,
    /// Two to four numbers side by side, each over its label.
    Metrics,
    /// A table of rows against columns, the first column at full intensity.
    Compare,
    /// Boxes joined left to right: the stages of a run.
    Flow,
    /// A framed passage over its attribution.
    Quote,
    /// Labeled facts on the left and prose on the right.
    Ask,
    /// One image, centered and as large as fits without losing sharpness,
    /// under the kicker and title when the slide has them.
    Image,
    /// Several images clustered in a centered grid, two to a row, each as
    /// large as its cell allows: a slide's `![alt](path)` lines, in order.
    Gallery,
}

impl Layout {
    /// The layout a script's `layout:` line names.
    pub fn named(name: &str) -> Option<Layout> {
        match name {
            "banner" => Some(Layout::Banner),
            "title" => Some(Layout::Title),
            "statement" => Some(Layout::Statement),
            "points" => Some(Layout::Points),
            "metrics" => Some(Layout::Metrics),
            "compare" => Some(Layout::Compare),
            "flow" => Some(Layout::Flow),
            "quote" => Some(Layout::Quote),
            "ask" => Some(Layout::Ask),
            "image" => Some(Layout::Image),
            "gallery" => Some(Layout::Gallery),
            _ => None,
        }
    }

    /// The name the script uses.
    pub fn name(&self) -> &'static str {
        match self {
            Layout::Banner => "banner",
            Layout::Title => "title",
            Layout::Statement => "statement",
            Layout::Points => "points",
            Layout::Metrics => "metrics",
            Layout::Compare => "compare",
            Layout::Flow => "flow",
            Layout::Quote => "quote",
            Layout::Ask => "ask",
            Layout::Image => "image",
            Layout::Gallery => "gallery",
        }
    }
}

/// The mark a metric with no value yet draws, so an empty cell reads as
/// empty rather than as a number.
pub const UNFILLED: &str = "—";

/// One number over its label.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Metric {
    pub value: String,
    pub label: String,
}

impl Metric {
    /// Whether the value is still to be filled in.
    pub fn is_unfilled(&self) -> bool {
        self.value.trim().is_empty() || self.value.trim() == UNFILLED
    }

    /// The value as it draws: the mark when nothing is filled in.
    pub fn shown(&self) -> &str {
        if self.is_unfilled() {
            UNFILLED
        } else {
            self.value.trim()
        }
    }
}

/// One row of a comparison: its label and one cell per column.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    pub label: String,
    pub cells: Vec<String>,
}

/// The source that owes a slide its facts. `owner` means the facts are not
/// in this repository and the owner fills them in.
pub const OWNER: &str = "owner";

/// One slide.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Slide {
    pub id: String,
    pub layout: Option<Layout>,
    pub title: Option<String>,
    pub lead: Option<String>,
    /// A short label over the title, at half intensity: the part of the
    /// talk the slide belongs to.
    pub kicker: Option<String>,
    pub source: Option<String>,
    pub note: Option<String>,
    pub body: String,
    pub metrics: Vec<Metric>,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub steps: Vec<String>,
    pub notes: Vec<String>,
    /// The image an image slide shows.
    pub image: Option<SlideImage>,
    /// The images a gallery slide clusters, in order.
    pub images: Vec<SlideImage>,
    /// How many times its native size an image slide's image may grow,
    /// when the slide has room (`scale: 2`); `None` is 1, never enlarged.
    pub scale: Option<u8>,
}

/// An image a slide shows: `![alt](path)` in the script, on a line by
/// itself. The path names a file under `decks/`, compiled in through
/// [`ASSETS`]; the alternative text is what the slide says to a reader
/// who can't see it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SlideImage {
    pub alt: String,
    pub path: String,
}

/// The files the scripts may show, by their path under `decks/`.
pub const ASSETS: &[(&str, &[u8])] = &[
    (
        "assets/important.png",
        include_bytes!("../decks/assets/important.png"),
    ),
    (
        "assets/tweet-tibo-open.png",
        include_bytes!("../decks/assets/tweet-tibo-open.png"),
    ),
    (
        "assets/tweet-david-lamond.png",
        include_bytes!("../decks/assets/tweet-david-lamond.png"),
    ),
    (
        "assets/tweet-zach.png",
        include_bytes!("../decks/assets/tweet-zach.png"),
    ),
    (
        "assets/tweet-sami.png",
        include_bytes!("../decks/assets/tweet-sami.png"),
    ),
    (
        "assets/tweet-demetrius-taylor.png",
        include_bytes!("../decks/assets/tweet-demetrius-taylor.png"),
    ),
    (
        "assets/tweet-monet.png",
        include_bytes!("../decks/assets/tweet-monet.png"),
    ),
    (
        "assets/marketplace.png",
        include_bytes!("../decks/assets/marketplace.png"),
    ),
    (
        "assets/ethan1.png",
        include_bytes!("../decks/assets/ethan1.png"),
    ),
];

/// The bytes of the asset at `path` under `decks/`, when it is compiled in.
pub fn asset(path: &str) -> Option<&'static [u8]> {
    ASSETS
        .iter()
        .find(|(known, _)| *known == path)
        .map(|(_, bytes)| *bytes)
}

impl Slide {
    /// The layout the slide draws in, or [`Layout::Statement`] for a slide
    /// whose script named none.
    pub fn layout(&self) -> Layout {
        self.layout.unwrap_or(Layout::Statement)
    }

    /// Whether the slide is waiting on facts: the owner owes it a source,
    /// or one of its metrics has no value.
    pub fn is_unfilled(&self) -> bool {
        self.source.as_deref() == Some(OWNER)
            || self.metrics.iter().any(Metric::is_unfilled)
            || self.rows.iter().any(|row| {
                row.cells
                    .iter()
                    .any(|cell| cell.trim().is_empty() || cell.trim() == UNFILLED)
            })
    }

    /// The presenter's note, one line per entry.
    pub fn note_text(&self) -> String {
        self.notes.join("\n")
    }
}

/// The deck: its name, and the slides in the order they present.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Deck {
    /// The name the script is filed under, which is also the directory its
    /// snapshots live in.
    pub name: String,
    pub slides: Vec<Slide>,
}

/// The scripts the repository ships, by name. The first is the one the
/// window opens without `--deck`.
pub const SCRIPTS: &[(&str, &str)] = &[
    (
        "three-devdays-later",
        include_str!("../decks/three-devdays-later.md"),
    ),
    (
        "test-time-capabilities",
        include_str!("../decks/test-time-capabilities.md"),
    ),
];

/// The deck the window opens when none is named.
pub const DEFAULT: &str = SCRIPTS[0].0;

impl Deck {
    /// The deck the window opens when none is named.
    pub fn load() -> Deck {
        Deck::named(DEFAULT).unwrap_or_default()
    }

    /// The deck filed under `name`, or none when the repository ships no
    /// script by that name.
    pub fn named(name: &str) -> Option<Deck> {
        let (_, source) = SCRIPTS.iter().find(|(known, _)| *known == name)?;
        Some(Deck::parsed(name, source))
    }

    /// The names a presenter may ask for, in the order they are filed.
    pub fn names() -> Vec<&'static str> {
        SCRIPTS.iter().map(|(name, _)| *name).collect()
    }

    /// Every deck the repository ships.
    pub fn all() -> Vec<Deck> {
        SCRIPTS
            .iter()
            .map(|(name, source)| Deck::parsed(name, source))
            .collect()
    }

    /// The deck `source` describes, filed under `name`.
    fn parsed(name: &str, source: &str) -> Deck {
        match script::parse(source) {
            Ok(mut deck) => {
                deck.name = name.to_string();
                deck
            }
            // The script is compiled in and a test parses it, so a
            // malformed one fails the build rather than the presentation.
            Err(complaint) => Deck {
                name: name.to_string(),
                slides: vec![Slide {
                    id: "script".to_string(),
                    layout: Some(Layout::Statement),
                    body: complaint,
                    ..Slide::default()
                }],
            },
        }
    }

    /// How many slides there are.
    /// The deck's title: the first slide's title, or the deck's name when
    /// that slide carries none. The window takes it as its title.
    pub fn title(&self) -> String {
        self.slides
            .first()
            .and_then(|slide| slide.title.clone())
            .unwrap_or_else(|| self.name.clone())
    }

    pub fn len(&self) -> usize {
        self.slides.len()
    }

    /// Whether the deck holds no slide.
    pub fn is_empty(&self) -> bool {
        self.slides.is_empty()
    }

    /// The slide at `index`, when there is one.
    pub fn slide(&self, index: usize) -> Option<&Slide> {
        self.slides.get(index)
    }

    /// The slides still waiting on facts, with their place in the deck.
    pub fn unfilled(&self) -> Vec<(usize, &Slide)> {
        self.slides
            .iter()
            .enumerate()
            .filter(|(_, slide)| slide.is_unfilled())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every layout name round-trips.
    #[test]
    fn a_layout_name_round_trips() {
        for layout in [
            Layout::Banner,
            Layout::Statement,
            Layout::Points,
            Layout::Metrics,
            Layout::Compare,
            Layout::Flow,
            Layout::Quote,
            Layout::Ask,
            Layout::Image,
            Layout::Gallery,
        ] {
            assert_eq!(Layout::named(layout.name()), Some(layout));
        }
    }

    /// Every shipped script parses under its own name, the default is the
    /// first, and an unknown name is refused.
    #[test]
    fn a_deck_is_found_by_name() {
        for name in Deck::names() {
            let deck = Deck::named(name).expect("the deck is filed");
            assert_eq!(deck.name, name);
            assert!(deck.slides.iter().all(|slide| slide.id != "script"));
        }
        assert_eq!(Deck::load().name, DEFAULT);
        assert!(Deck::named("nothing").is_none());
        assert_eq!(Deck::all().len(), SCRIPTS.len());
    }

    /// A metric with no value reports itself unfilled and draws the mark.
    #[test]
    fn an_empty_metric_is_unfilled() {
        let empty = Metric {
            value: "  ".to_string(),
            label: "runs".to_string(),
        };
        assert!(empty.is_unfilled());
        assert_eq!(empty.shown(), UNFILLED);
        let filled = Metric {
            value: "292".to_string(),
            label: "tests".to_string(),
        };
        assert!(!filled.is_unfilled());
        assert_eq!(filled.shown(), "292");
    }
}
