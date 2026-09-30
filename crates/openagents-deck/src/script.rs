//! The parser for a script under `decks/`, where a deck's copy lives.
//!
//! A line of three dashes separates two slides. Inside a slide, a line that
//! opens with one of the keys below is a directive; every other line is the
//! slide's body, which the layouts that take prose lay out through
//! [`crate::prose`]. The format is line oriented on purpose: a
//! reviewer reads the copy as Markdown, and the style gate reads it as
//! prose, without a front matter dialect in the way.
//!
//! ```text
//! layout: points
//! id: gate
//! title: Work that fails the checks never lands
//!
//! - **The gate runs before delivery.** Formatting, lint, and the tests.
//!
//! notes: Name the runs that failed the gate this month.
//! ```

use crate::slide::{Deck, Layout, Metric, Row, Slide, SlideImage};

/// The line that separates two slides.
const BREAK: &str = "---";

/// The deck `source` describes, with no name yet, or what is wrong with it.
pub fn parse(source: &str) -> Result<Deck, String> {
    let mut slides = Vec::new();
    for (number, part) in source.split(&format!("\n{BREAK}\n")).enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        slides.push(slide(part, number + 1)?);
    }
    if slides.is_empty() {
        return Err("the script holds no slide".to_string());
    }
    Ok(Deck {
        name: String::new(),
        slides,
    })
}

/// One slide, from the text between two breaks.
fn slide(text: &str, number: usize) -> Result<Slide, String> {
    let mut slide = Slide::default();
    let mut body = String::new();
    for line in text.lines() {
        match directive(line) {
            Some(("layout", value)) => {
                slide.layout = Some(
                    Layout::named(value)
                        .ok_or_else(|| format!("slide {number} names no layout: {value}"))?,
                );
            }
            Some(("id", value)) => slide.id = value.to_string(),
            Some(("title", value)) => slide.title = Some(value.to_string()),
            Some(("lead", value)) => slide.lead = Some(value.to_string()),
            Some(("source", value)) => slide.source = Some(value.to_string()),
            Some(("note", value)) => slide.note = Some(value.to_string()),
            Some(("kicker", value)) => slide.kicker = Some(value.to_string()),
            Some(("metric", value)) => slide.metrics.push(metric(value)),
            Some(("column", value)) => slide.columns.push(value.to_string()),
            Some(("row", value)) => slide.rows.push(row(value)),
            Some(("step", value)) => slide.steps.push(value.to_string()),
            Some(("notes", value)) => slide.notes.push(value.to_string()),
            _ => {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    if slide.id.is_empty() {
        return Err(format!("slide {number} carries no id"));
    }
    slide.image = image(body.trim());
    if slide.layout.is_none() && slide.image.is_some() {
        slide.layout = Some(Layout::Image);
    }
    if slide.layout == Some(Layout::Image) && slide.image.is_none() {
        return Err(format!(
            "slide {number} is an image slide with no ![alt](path) line"
        ));
    }
    if slide.layout.is_none() {
        return Err(format!("slide {number} names no layout"));
    }
    slide.body = body.trim().to_string();
    Ok(slide)
}

/// The keys a line may open with.
const KEYS: &[&str] = &[
    "layout", "id", "title", "lead", "kicker", "source", "note", "metric", "column", "row", "step",
    "notes",
];

/// The key and the value of a directive line, when the line is one. A key
/// holds no space, so a sentence with a colon in it stays in the body.
fn directive(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    if key.chars().any(char::is_whitespace) {
        return None;
    }
    KEYS.contains(&key).then(|| (key, value.trim()))
}

/// The image a body shows, when the body is one `![alt](path)` line and
/// nothing else.
fn image(body: &str) -> Option<SlideImage> {
    if body.lines().count() != 1 {
        return None;
    }
    let rest = body.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let path = rest.strip_suffix(')')?;
    (!path.is_empty() && !path.contains(char::is_whitespace)).then(|| SlideImage {
        alt: alt.trim().to_string(),
        path: path.to_string(),
    })
}

/// A metric line: the value, a vertical bar, and the label.
fn metric(value: &str) -> Metric {
    match value.split_once('|') {
        Some((number, label)) => Metric {
            value: number.trim().to_string(),
            label: label.trim().to_string(),
        },
        None => Metric {
            value: String::new(),
            label: value.trim().to_string(),
        },
    }
}

/// A row line: the label and one cell per column, separated by bars.
fn row(value: &str) -> Row {
    let mut parts = value.split('|').map(|part| part.trim().to_string());
    let label = parts.next().unwrap_or_default();
    Row {
        label,
        cells: parts.collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script of two slides parses into two slides, with the directives
    /// read and the body kept as Markdown.
    #[test]
    fn a_script_parses_into_slides() {
        let deck = parse(
            "layout: banner\nid: title\nlead: The one line\n\n---\n\n\
             layout: points\nid: gate\ntitle: The gate\n\n\
             - **One.** The first item\n- **Two.** The second\n\n\
             notes: Say the number\n",
        )
        .expect("the script parses");
        assert_eq!(deck.len(), 2);
        assert_eq!(deck.slides[0].layout(), Layout::Banner);
        assert_eq!(deck.slides[0].lead.as_deref(), Some("The one line"));
        assert_eq!(deck.slides[1].id, "gate");
        assert!(deck.slides[1].body.starts_with("- **One.**"));
        assert_eq!(deck.slides[1].note_text(), "Say the number");
    }

    /// A metric line splits on the bar, and a row keeps one cell per
    /// column.
    #[test]
    fn a_metric_and_a_row_split_on_the_bar() {
        let deck = parse(
            "layout: metrics\nid: bench\nmetric: 292 | tests\nmetric:  | runs\n\n---\n\n\
             layout: compare\nid: field\ncolumn: Coder\ncolumn: The rest\n\
             row: Gates its own work | yes | no\n",
        )
        .expect("the script parses");
        assert_eq!(deck.slides[0].metrics[0].value, "292");
        assert_eq!(deck.slides[0].metrics[1].label, "runs");
        assert!(deck.slides[0].metrics[1].is_unfilled());
        assert_eq!(deck.slides[1].rows[0].cells, vec!["yes", "no"]);
    }

    /// A slide with no id and a slide with no layout are both refused.
    #[test]
    fn a_slide_without_an_id_or_a_layout_is_refused() {
        assert!(parse("layout: banner\n").is_err());
        assert!(parse("id: title\n").is_err());
        assert!(parse("layout: nothing\nid: title\n").is_err());
    }

    /// A lone image line makes an image slide, with or without `layout:`;
    /// an image slide without one is refused.
    #[test]
    fn a_lone_image_makes_an_image_slide() {
        let deck = parse("id: a\n\n![A post](assets/a.png)\n").expect("the script parses");
        assert_eq!(deck.slides[0].layout(), Layout::Image);
        let image = deck.slides[0].image.clone().expect("the image");
        assert_eq!(
            (image.alt.as_str(), image.path.as_str()),
            ("A post", "assets/a.png")
        );
        assert!(parse("layout: image\nid: a\n\nNo image.\n").is_err());
        let prose = parse("layout: points\nid: a\n\n![a](b.png) and words\n").unwrap();
        assert!(prose.slides[0].image.is_none());
    }

    /// A line that only holds a colon, such as prose or a Markdown link,
    /// stays in the body.
    #[test]
    fn prose_with_a_colon_stays_in_the_body() {
        let deck = parse("layout: statement\nid: one\n\nThe rule: one thing at a time.\n")
            .expect("the script parses");
        assert_eq!(deck.slides[0].body, "The rule: one thing at a time.");
    }
}
