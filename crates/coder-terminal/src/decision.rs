//! A decision's evidence: what the call decided, where the data went,
//! and what it cost.
//!
//! A turn that asks a model to judge — which program a request wants,
//! whether a draft may run commands — leaves a record behind: the profile
//! that carried the call, the destination the data went to, the function
//! or program it selected, the answer or refusal it produced, who
//! answered, how long it took, and what it cost. [`DecisionView`] is the
//! caller's half of that record, with two presentations:
//! [`lines`][DecisionView::lines] is the one-line summary a transcript
//! can carry, and [`expanded`][DecisionView::expanded] is the full detail
//! the summary stands in for.
//!
//! Three rules outrank every layout choice in here:
//!
//! - **An unknown renders as `unknown`.** A cost nobody measured is not a
//!   zero, a model that cannot be named is not a model, and an outcome
//!   nobody recorded is not an answer. The view says `unknown` in the
//!   record's own word rather than smoothing the gap into something that
//!   looks measured.
//! - **The four outcomes are different rows.** An answer with its
//!   probability, a refusal with its code, an unavailable call, and an
//!   unrecorded outcome are never smudged into one "no answer" — each
//!   keeps its own word and its own step on the ladder.
//! - **A simulated view can never pass for metered work.** [`Origin`] is
//!   part of the model and rides both presentations: a demonstration
//!   recedes to the faintest step, a measurement stays present.
//!
//! The view is pure: it reads nothing, writes nothing, and holds no
//! style. A [`Line`] is spans of text and the [`Intensity`] step each
//! draws at; which amber a step burns is the caller's
//! [`Ladder`][crate::Ladder]'s to say, so the same lines serve a
//! truecolor terminal, a 256-color one, and `NO_COLOR` alike.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Intensity, wrap_rows};

/// The mark a cut leaves, so a narrowed view shows that it narrowed.
const ELLIPSIS: &str = "...";

/// The cells a field's name takes in the expanded view.
const LABEL: usize = 13;

/// Below this width an expanded field stacks its name over its value
/// rather than sharing a line neither fits on.
const STACKED: usize = LABEL + 4;

/// Where a decision's evidence came from.
///
/// The distinction is not cosmetic: a simulated view demonstrates the
/// layout with invented numbers, and it must never read as a call that
/// actually ran. `Simulated` is the default because an unmarked view is
/// safer mistaken for a demo than for a measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    /// An example: the numbers were invented to show the view.
    #[default]
    Simulated,
    /// A call that ran: the numbers were measured.
    Metered,
}

impl Origin {
    /// The word the view carries.
    pub const fn word(self) -> &'static str {
        match self {
            Origin::Simulated => "simulated",
            Origin::Metered => "metered",
        }
    }

    /// The step the word draws at. A simulation recedes to the faintest
    /// tone — it is a demonstration, and it looks like one.
    pub const fn tone(self) -> Intensity {
        match self {
            Origin::Simulated => Intensity::Quarter,
            Origin::Metered => Intensity::Half,
        }
    }
}

/// What the decision produced.
///
/// Four states, four words, four tones — never one "no answer". An
/// answered call is a measurement with its probability; a refusal is the
/// door's own answer and belongs in the record; `unavailable` means the
/// call produced nothing to record; `unknown` means nobody recorded what
/// happened at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Outcome {
    /// The door answered: the original result and the probability it
    /// carried.
    Answered {
        /// The result the decision returned.
        answer: String,
        /// The probability the answer carried, `0.0` to `1.0`.
        probability: f64,
    },
    /// The door refused, with the typed refusal code it gave.
    Refused {
        /// The refusal code, as the door spelled it.
        code: String,
    },
    /// The call produced no answer — not a refusal, a silence.
    Unavailable,
    /// The outcome was never recorded.
    #[default]
    Unknown,
}

impl Outcome {
    /// The outcome's own word.
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Answered { .. } => "answered",
            Outcome::Refused { .. } => "refused",
            Outcome::Unavailable => "unavailable",
            Outcome::Unknown => "unknown",
        }
    }
}

/// A second look at the original result.
///
/// A review states its own result whether it upheld or overturned the
/// first, and it always names who looked — a review nobody can name is
/// no review at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    /// The result the review reached.
    pub result: String,
    /// Who reviewed: a name, a key id, a gate — whatever identifies them.
    pub reviewer: String,
}

/// What one decision left behind, filled in by the caller.
///
/// Every field is shown by [`expanded`][DecisionView::expanded]; the
/// collapsed [`lines`][DecisionView::lines] keeps the origin, the
/// profile, the destination, the selection, and the headline outcome,
/// and marks whatever it had to cut with `...`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecisionView {
    /// Simulated or metered — carried on both presentations.
    pub origin: Origin,
    /// The active decision profile's name.
    pub profile: String,
    /// Where the call's data went: the endpoint or relay it addressed.
    pub destination: String,
    /// The function or program the decision selected.
    pub selection: String,
    /// What the decision produced.
    pub outcome: Outcome,
    /// A review of the original result, when one exists.
    pub review: Option<Review>,
    /// The model that actually answered — `None` when that cannot be
    /// named, which renders as `unknown`.
    pub model: Option<String>,
    /// How long the call took, when it was measured.
    pub latency_ms: Option<u64>,
    /// What the call cost, when it is known — `None` renders as
    /// `unknown`, never as a fabricated zero.
    pub cost: Option<f64>,
}

impl DecisionView {
    /// The collapsed summary: one line of profile, destination,
    /// selection, and the headline outcome behind the origin's word,
    /// clipped to `width` and marked with `...` where it had to cut.
    ///
    /// The cut is marked, never silent — and [`expanded`][Self::expanded]
    /// names the fields the summary dropped, so nothing the view knows
    /// is lost without a word.
    pub fn lines(&self, width: usize) -> Vec<Line> {
        let mut spans = Vec::new();
        for (index, (_, part)) in self.summary_parts().into_iter().enumerate() {
            if index > 0 {
                spans.push(Span::new("  ", Intensity::Half));
            }
            spans.extend(part);
        }
        vec![clip(spans, width)]
    }

    /// The full detail: every field the view holds, one field per line,
    /// wrapped rather than cut when the width runs short.
    ///
    /// Whatever [`lines`][Self::lines] had to drop at `width` is stated
    /// here twice — once because every field draws in full, and once in
    /// the `elided` row that names what the summary hid.
    pub fn expanded(&self, width: usize) -> Vec<Line> {
        let mut lines = vec![clip(
            vec![
                Span::new("decision", Intensity::Full),
                Span::new("  ", Intensity::Half),
                Span::new(self.origin.word(), self.origin.tone()),
            ],
            width,
        )];

        let (profile, tone) = shown(&self.profile, Intensity::ThreeQuarters);
        field(&mut lines, width, "profile", toned(&profile, tone));
        let (destination, tone) = shown(&self.destination, Intensity::Half);
        field(&mut lines, width, "destination", toned(&destination, tone));
        let (selection, tone) = shown(&self.selection, Intensity::ThreeQuarters);
        field(&mut lines, width, "selection", toned(&selection, tone));

        let mut outcome = Toned::default();
        for (text, intensity) in self.outcome_spans() {
            outcome.push(&text, intensity);
        }
        field(&mut lines, width, "outcome", outcome);

        if let Some(review) = &self.review {
            let mut reviewed = Toned::default();
            reviewed.push(&format!("\"{}\"", review.result), Intensity::ThreeQuarters);
            reviewed.push("  reviewer ", Intensity::Half);
            reviewed.push(&review.reviewer, Intensity::ThreeQuarters);
            field(&mut lines, width, "reviewed", reviewed);
        }

        let (model, tone) = match &self.model {
            Some(model) => shown(model, Intensity::ThreeQuarters),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "model", toned(&model, tone));

        let (latency, tone) = match self.latency_ms {
            Some(ms) => (format!("{ms} ms"), Intensity::ThreeQuarters),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "latency", toned(&latency, tone));

        let (cost, tone) = match self.cost {
            Some(cost) => (format!("${cost:.4}"), Intensity::ThreeQuarters),
            None => ("unknown".to_owned(), Intensity::Quarter),
        };
        field(&mut lines, width, "cost", toned(&cost, tone));

        let elided = self.elided(width);
        if !elided.is_empty() {
            field(
                &mut lines,
                width,
                "elided",
                toned(&elided.join(", "), Intensity::Quarter),
            );
        }
        lines
    }

    /// The collapsed line's fields, in order: origin first, then
    /// profile, destination, selection, and the headline outcome.
    fn summary_parts(&self) -> Vec<(&'static str, Vec<Span>)> {
        let single = |value: &str, tone: Intensity| {
            let (text, tone) = shown(value, tone);
            vec![Span::new(text, tone)]
        };
        vec![
            (
                "origin",
                vec![Span::new(self.origin.word(), self.origin.tone())],
            ),
            ("profile", single(&self.profile, Intensity::ThreeQuarters)),
            ("destination", single(&self.destination, Intensity::Half)),
            (
                "selection",
                single(&self.selection, Intensity::ThreeQuarters),
            ),
            (
                "outcome",
                self.outcome_spans()
                    .into_iter()
                    .map(|(text, tone)| Span::new(text, tone))
                    .collect(),
            ),
        ]
    }

    /// The outcome's spans: its own word first, then the detail it
    /// carries, each at its own step.
    fn outcome_spans(&self) -> Vec<(String, Intensity)> {
        match &self.outcome {
            Outcome::Answered {
                answer,
                probability,
            } => vec![
                ("answered ".to_owned(), Intensity::Half),
                (format!("\"{answer}\""), Intensity::ThreeQuarters),
                ("  ".to_owned(), Intensity::Half),
                (format!("p={probability:.2}"), step(*probability)),
            ],
            Outcome::Refused { code } => vec![
                ("refused ".to_owned(), Intensity::Full),
                (code.clone(), Intensity::ThreeQuarters),
            ],
            Outcome::Unavailable => vec![
                ("unavailable".to_owned(), Intensity::Half),
                ("  no answer returned".to_owned(), Intensity::Quarter),
            ],
            Outcome::Unknown => vec![
                ("unknown".to_owned(), Intensity::Quarter),
                ("  not recorded".to_owned(), Intensity::Quarter),
            ],
        }
    }

    /// The fields the collapsed line drops at `width`, by name. An empty
    /// list means the summary fit whole.
    fn elided(&self, width: usize) -> Vec<&'static str> {
        let parts = self.summary_parts();
        let text: usize = parts
            .iter()
            .flat_map(|(_, spans)| spans.iter())
            .map(|span| span.text.width())
            .sum();
        let separators = 2 * parts.len().saturating_sub(1);
        if text + separators <= width {
            return Vec::new();
        }
        // A field is dropped when none of its text survives the budget
        // the `...` marker leaves.
        let budget = width.saturating_sub(ELLIPSIS.len());
        let mut used = 0;
        let mut elided = Vec::new();
        for (index, (name, spans)) in parts.iter().enumerate() {
            let before = if index == 0 { 0 } else { 2 };
            let room = budget.saturating_sub(used + before);
            let part: usize = spans.iter().map(|span| span.text.width()).sum();
            if part.min(room) == 0 {
                elided.push(*name);
            }
            used += before + part;
        }
        elided
    }
}

/// A run of text at one step of the ladder — the smallest unit the view
/// composes.
///
/// Styling is the caller's job: the span names an [`Intensity`], never a
/// color, so a `NO_COLOR` terminal and a truecolor one read the same
/// lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// The text.
    pub text: String,
    /// The step it draws at.
    pub intensity: Intensity,
}

impl Span {
    /// A span of `text` at `intensity`.
    pub fn new(text: impl Into<String>, intensity: Intensity) -> Self {
        Self {
            text: text.into(),
            intensity,
        }
    }
}

/// One rendered line: the spans, left to right.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    /// The spans, in order.
    pub spans: Vec<Span>,
}

impl Line {
    /// The cells the line draws across.
    pub fn width(&self) -> usize {
        self.spans.iter().map(|span| span.text.width()).sum()
    }

    /// The line's text with the spans joined — what a test or a pipe
    /// reads.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// A field's value: its text and the step each run of it draws at.
///
/// The shape [`crate::Marked`] gives rendered prose, with steps in place
/// of marks — a quiet word, an answer, and a probability can ride one
/// line at three tones and still wrap together.
#[derive(Clone, Debug, Default)]
struct Toned {
    text: String,
    runs: Vec<(Range<usize>, Intensity)>,
}

impl Toned {
    /// Appends `text` at `intensity`, joining the last run when the step
    /// matches.
    fn push(&mut self, text: &str, intensity: Intensity) {
        if text.is_empty() {
            return;
        }
        if let Some((range, held)) = self.runs.last_mut()
            && *held == intensity
        {
            range.end += text.len();
            self.text.push_str(text);
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.runs.push((start..self.text.len(), intensity));
    }

    /// The spans covering `bytes`, clipped to it — what one wrapped row
    /// of this value draws.
    fn runs_in(&self, bytes: Range<usize>) -> Vec<Span> {
        self.runs
            .iter()
            .filter_map(|(range, intensity)| {
                let start = range.start.max(bytes.start);
                let end = range.end.min(bytes.end);
                (start < end).then(|| Span::new(&self.text[start..end], *intensity))
            })
            .collect()
    }
}

/// A single-tone value.
fn toned(text: &str, intensity: Intensity) -> Toned {
    let mut value = Toned::default();
    value.push(text, intensity);
    value
}

/// One field in the expanded view: `name` quiet on the left, `value`
/// beside it or under it, wrapped when the width runs short so nothing
/// is cut.
fn field(lines: &mut Vec<Line>, width: usize, name: &str, value: Toned) {
    if width >= STACKED {
        let inner = width - LABEL;
        for (index, range) in wrap_rows(&value.text, inner).into_iter().enumerate() {
            let lead = if index == 0 {
                left(name, LABEL)
            } else {
                " ".repeat(LABEL)
            };
            let mut spans = vec![Span::new(lead, Intensity::Half)];
            spans.extend(value.runs_in(range));
            lines.push(Line { spans });
        }
    } else {
        lines.push(Line {
            spans: vec![Span::new(clip_text(name, width), Intensity::Half)],
        });
        let inner = width.saturating_sub(2).max(1);
        for range in wrap_rows(&value.text, inner) {
            let mut spans = vec![Span::new("  ", Intensity::Half)];
            spans.extend(value.runs_in(range));
            lines.push(Line { spans });
        }
    }
}

/// A name the caller filled, or `unknown` at the faintest step when the
/// field was left blank — a blank field is a field nobody filled, and
/// the view says so.
fn shown(value: &str, tone: Intensity) -> (String, Intensity) {
    if value.trim().is_empty() {
        ("unknown".to_owned(), Intensity::Quarter)
    } else {
        (value.to_owned(), tone)
    }
}

/// The step a probability draws at: the surer the answer, the brighter
/// the number, with the ladder's quarters as the thresholds.
fn step(probability: f64) -> Intensity {
    let probability = if probability.is_finite() {
        probability.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let index = (probability * Intensity::ALL.len() as f64) as usize;
    Intensity::ALL[index.min(Intensity::ALL.len() - 1)]
}

/// Cuts spans at `width` cells. Whatever is dropped ends with `...`, so
/// a cut is always marked, never silent.
fn clip(spans: Vec<Span>, width: usize) -> Line {
    let total: usize = spans.iter().map(|span| span.text.width()).sum();
    if total <= width {
        return Line { spans };
    }
    let budget = width.saturating_sub(ELLIPSIS.len());
    let mut kept = Vec::new();
    let mut used = 0;
    for span in spans {
        let room = budget.saturating_sub(used);
        if room == 0 {
            break;
        }
        if span.text.width() <= room {
            used += span.text.width();
            kept.push(span);
            continue;
        }
        let mut taken = String::new();
        let mut cells = 0;
        for grapheme in span.text.graphemes(true) {
            cells += grapheme.width();
            if cells > room {
                break;
            }
            taken.push_str(grapheme);
        }
        if !taken.is_empty() {
            kept.push(Span::new(taken, span.intensity));
        }
        break;
    }
    let marker = &ELLIPSIS[..width.min(ELLIPSIS.len())];
    if !marker.is_empty() {
        kept.push(Span::new(marker, Intensity::Quarter));
    }
    Line { spans: kept }
}

/// Cuts a bare string at `width` cells, with the same `...` mark.
fn clip_text(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width <= ELLIPSIS.len() {
        return ELLIPSIS[..width].to_owned();
    }
    let room = width - ELLIPSIS.len();
    let mut taken = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        cells += grapheme.width();
        if cells > room {
            break;
        }
        taken.push_str(grapheme);
    }
    taken.push_str(ELLIPSIS);
    taken
}

/// Pads `text` to `width` cells on the right, so a column of values
/// lines up under itself.
fn left(text: &str, width: usize) -> String {
    let taken = text.width();
    if taken >= width {
        clip_text(text, width)
    } else {
        format!("{text}{}", " ".repeat(width - taken))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A metered view with every field filled.
    fn metered_view() -> DecisionView {
        DecisionView {
            origin: Origin::Metered,
            profile: "local".to_owned(),
            destination: "http://127.0.0.1:8080/v1/systemone".to_owned(),
            selection: "classify".to_owned(),
            outcome: Outcome::Answered {
                answer: "chat".to_owned(),
                probability: 0.97,
            },
            review: Some(Review {
                result: "chat".to_owned(),
                reviewer: "gate:probability-v2".to_owned(),
            }),
            model: Some("kev-0.3".to_owned()),
            latency_ms: Some(214),
            cost: Some(0.0021),
        }
    }

    /// The expanded view as one string, for substring checks.
    fn expanded_text(view: &DecisionView, width: usize) -> String {
        view.expanded(width)
            .iter()
            .map(Line::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The spans of the expanded view's `outcome` row.
    fn outcome_spans(view: &DecisionView) -> Vec<Span> {
        view.expanded(80)
            .into_iter()
            .find(|line| line.text().starts_with("outcome"))
            .expect("an outcome row")
            .spans
    }

    #[test]
    fn the_collapsed_line_is_one_line_with_the_headline() {
        let lines = metered_view().lines(120);
        assert_eq!(lines.len(), 1);
        let text = lines[0].text();
        for part in [
            "metered",
            "local",
            "http://127.0.0.1:8080/v1/systemone",
            "classify",
            "answered",
            "p=0.97",
        ] {
            assert!(text.contains(part), "{part} missing from {text}");
        }
    }

    #[test]
    fn the_collapsed_line_marks_a_cut() {
        let lines = metered_view().lines(24);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].width() <= 24, "{} cells", lines[0].width());
        assert!(lines[0].text().ends_with("..."), "{}", lines[0].text());
        // Even an empty width still yields the one summary line.
        assert_eq!(metered_view().lines(0).len(), 1);
    }

    #[test]
    fn the_expanded_view_shows_every_field() {
        let text = expanded_text(&metered_view(), 80);
        for part in [
            "metered",
            "local",
            "http://127.0.0.1:8080/v1/systemone",
            "classify",
            "answered",
            "\"chat\"",
            "p=0.97",
            "reviewer gate:probability-v2",
            "kev-0.3",
            "214 ms",
            "$0.0021",
        ] {
            assert!(text.contains(part), "{part} missing from\n{text}");
        }
    }

    #[test]
    fn the_four_outcomes_are_different_rows() {
        let mut view = metered_view();
        for (outcome, word) in [
            (
                Outcome::Answered {
                    answer: "chat".to_owned(),
                    probability: 0.97,
                },
                "answered",
            ),
            (
                Outcome::Refused {
                    code: "quota_exceeded".to_owned(),
                },
                "refused",
            ),
            (Outcome::Unavailable, "unavailable"),
            (Outcome::Unknown, "unknown"),
        ] {
            view.outcome = outcome;
            let row = expanded_text(&view, 80)
                .lines()
                .find(|line| line.starts_with("outcome"))
                .expect("an outcome row")
                .to_owned();
            assert!(row.contains(word), "{row}");
            assert!(row.contains(view.outcome.word()), "{row}");
            for other in ["answered", "refused", "unavailable", "unknown"] {
                if other != word {
                    assert!(!row.contains(other), "{other} in {row}");
                }
            }
        }
    }

    #[test]
    fn a_refusal_is_styled_apart_from_an_answer() {
        let mut view = metered_view();
        view.outcome = Outcome::Refused {
            code: "quota_exceeded".to_owned(),
        };
        let refused = outcome_spans(&view);
        assert!(
            refused
                .iter()
                .any(|span| span.text.starts_with("refused") && span.intensity == Intensity::Full),
            "a refusal's word is loud: {refused:?}"
        );

        view.outcome = Outcome::Answered {
            answer: "chat".to_owned(),
            probability: 0.97,
        };
        let answered = outcome_spans(&view);
        assert!(
            answered
                .iter()
                .any(|span| span.text.starts_with("answered") && span.intensity == Intensity::Half),
            "an answer's word stays quiet: {answered:?}"
        );
    }

    #[test]
    fn probability_rides_the_ladder() {
        let mut view = metered_view();
        let tone_of = |view: &mut DecisionView, probability: f64| {
            view.outcome = Outcome::Answered {
                answer: "chat".to_owned(),
                probability,
            };
            outcome_spans(view)
                .into_iter()
                .find(|span| span.text.contains("p="))
                .expect("a probability span")
                .intensity
        };
        assert_eq!(tone_of(&mut view, 0.97), Intensity::Full);
        assert_eq!(tone_of(&mut view, 0.60), Intensity::ThreeQuarters);
        assert_eq!(tone_of(&mut view, 0.30), Intensity::Half);
        assert_eq!(tone_of(&mut view, 0.10), Intensity::Quarter);
    }

    #[test]
    fn an_unmeasured_cost_is_unknown_not_zero() {
        let mut view = metered_view();
        view.cost = None;
        let cost = expanded_text(&view, 80)
            .lines()
            .find(|line| line.starts_with("cost"))
            .expect("a cost row")
            .to_owned();
        assert!(cost.contains("unknown"), "{cost}");
        assert!(!cost.contains("$0"), "{cost}");
        assert!(!cost.contains("0.0"), "{cost}");
    }

    #[test]
    fn a_simulated_view_cannot_pass_for_metered() {
        let mut view = metered_view();
        view.origin = Origin::Simulated;

        let collapsed = view.lines(120);
        assert_eq!(collapsed[0].spans[0].text, "simulated");
        assert_eq!(collapsed[0].spans[0].intensity, Intensity::Quarter);

        let header = view.expanded(120)[0].clone();
        assert!(header.text().contains("simulated"), "{}", header.text());

        // A measurement names itself too, and at a different step —
        // the two read differently with or without color.
        let metered = metered_view().lines(120);
        assert_eq!(metered[0].spans[0].text, "metered");
        assert_eq!(metered[0].spans[0].intensity, Intensity::Half);
    }

    #[test]
    fn the_detail_names_what_the_summary_dropped() {
        // A wide summary fits whole, and no elided row appears.
        let wide = expanded_text(&metered_view(), 120);
        assert!(!wide.contains("elided"), "{wide}");

        // At 24 cells the summary keeps origin, profile, and part of the
        // destination; the rest is named in the detail.
        let text = expanded_text(&metered_view(), 24);
        let at = text.find("elided").expect("an elided row");
        let after = &text[at..];
        assert!(after.contains("selection"), "{after}");
        assert!(after.contains("outcome"), "{after}");
    }

    #[test]
    fn a_narrow_terminal_still_gets_ordered_lines() {
        let lines = metered_view().expanded(20);
        assert!(lines.len() > 5, "{} lines", lines.len());
        for line in &lines {
            assert!(
                line.width() <= 20,
                "{} is {} cells",
                line.text(),
                line.width()
            );
        }
        // Every field still shows, in the order the model carries them.
        let text = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        let mut at = 0;
        for name in [
            "profile",
            "destination",
            "selection",
            "outcome",
            "reviewed",
            "model",
            "latency",
            "cost",
        ] {
            let found = text[at..].find(name).map(|index| at + index);
            assert!(found.is_some(), "{name} missing after {at} in\n{text}");
            at = found.unwrap() + name.len();
        }
    }
}
