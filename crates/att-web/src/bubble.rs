//! The scene's bubbles: what each party sees, over its head.
//!
//! You get a speech bubble in plain words. The relay and the sealed
//! provider get an event bubble: a Nostr event pretty-printed as JSON, the
//! field that matters in bold, and long strings cut in the middle until
//! tapped. This module is pure (lines of styled spans, and where the
//! bubbles go); `show` turns it into DOM.

use serde_json::Value;

use crate::steps::middle_cut;

/// Whose bubble.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Party {
    You,
    Relay,
    Provider,
}

impl Party {
    pub const ALL: [Self; 3] = [Self::You, Self::Relay, Self::Provider];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The `data-party` value.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Relay => "relay",
            Self::Provider => "provider",
        }
    }
}

/// How a span is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Braces, brackets, commas, colons.
    Punct,
    /// An object's key.
    Key,
    /// A string, number, or other value.
    Value,
}

/// One run of text in a line of an event bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub look: Look,
    /// Drawn bold: the value of the field the bubble is about.
    pub bold: bool,
    /// The whole text, when `text` is cut in the middle.
    pub full: Option<String>,
}

/// Strings longer than this are cut in the middle until tapped.
pub const CUT_OVER: usize = 26;
/// How long a cut string is.
pub const CUT_TO: usize = 19;

/// The order a Nostr event's fields are shown in; others follow.
const ORDER: [&str; 7] = [
    "id",
    "pubkey",
    "created_at",
    "kind",
    "tags",
    "content",
    "sig",
];

fn span(text: impl Into<String>, look: Look) -> Span {
    Span {
        text: text.into(),
        look,
        bold: false,
        full: None,
    }
}

/// Bold strings up to this many characters are shown whole.
const BOLD_WHOLE: usize = 240;

/// A scalar as JSON, cut in the middle when it is long.
fn scalar(value: &Value, bold: bool) -> Span {
    let full = value.to_string();
    let cut = match value {
        // Bold text is what the bubble is about: the decrypted question
        // stays whole; only long ciphertext is cut.
        Value::String(s) if bold && s.chars().count() <= BOLD_WHOLE => None,
        Value::String(s) => middle_cut(s, CUT_OVER).and_then(|_| middle_cut(s, CUT_TO)),
        _ => None,
    };
    match cut {
        Some(cut) => Span {
            text: format!("\"{cut}\""),
            look: Look::Value,
            bold,
            full: Some(full),
        },
        None => Span {
            text: full,
            look: Look::Value,
            bold,
            full: None,
        },
    }
}

fn is_scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// Writes `value` into `lines`, continuing the current last line.
fn write(lines: &mut Vec<Vec<Span>>, value: &Value, indent: usize, bold: bool, bold_key: &str) {
    let pad = "  ".repeat(indent);
    match value {
        Value::Object(map) => {
            lines
                .last_mut()
                .expect("a line")
                .push(span("{", Look::Punct));
            let mut keys: Vec<&String> = ORDER
                .iter()
                .filter_map(|k| map.get_key_value(*k).map(|(k, _)| k))
                .collect();
            keys.extend(map.keys().filter(|k| !ORDER.contains(&k.as_str())));
            for (i, key) in keys.iter().enumerate() {
                let mut line = vec![
                    span(format!("{pad}  "), Look::Punct),
                    span(format!("\"{key}\""), Look::Key),
                    span(": ", Look::Punct),
                ];
                let child = &map[key.as_str()];
                let child_bold = bold || key.as_str() == bold_key;
                if is_scalar(child) {
                    line.push(scalar(child, child_bold));
                    lines.push(line);
                } else {
                    lines.push(line);
                    write(lines, child, indent + 1, child_bold, bold_key);
                }
                if i + 1 < keys.len() {
                    lines
                        .last_mut()
                        .expect("a line")
                        .push(span(",", Look::Punct));
                }
            }
            lines.push(vec![span(format!("{pad}}}"), Look::Punct)]);
        }
        Value::Array(items) if items.iter().all(is_scalar) => {
            // A short array of values, such as one tag, on one line.
            let line = lines.last_mut().expect("a line");
            line.push(span("[", Look::Punct));
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    line.push(span(", ", Look::Punct));
                }
                line.push(scalar(item, bold));
            }
            line.push(span("]", Look::Punct));
        }
        Value::Array(items) => {
            lines
                .last_mut()
                .expect("a line")
                .push(span("[", Look::Punct));
            for (i, item) in items.iter().enumerate() {
                lines.push(vec![span(format!("{pad}  "), Look::Punct)]);
                write(lines, item, indent + 1, bold, bold_key);
                if i + 1 < items.len() {
                    lines
                        .last_mut()
                        .expect("a line")
                        .push(span(",", Look::Punct));
                }
            }
            lines.push(vec![span(format!("{pad}]"), Look::Punct)]);
        }
        scalar_value => lines
            .last_mut()
            .expect("a line")
            .push(scalar(scalar_value, bold)),
    }
}

/// `event` pretty-printed as JSON lines, Nostr fields first, with the
/// value of the field named `bold` in bold.
#[must_use]
pub fn event_lines(event: &Value, bold: &str) -> Vec<Vec<Span>> {
    let mut lines = vec![Vec::new()];
    write(&mut lines, event, 0, false, bold);
    lines.retain(|line| !line.is_empty());
    lines
}

/// A bubble to place: where its party is (its tail's tip, in stage
/// pixels) and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    pub x: f64,
    /// The bottom of the bubble should sit here: just over the party's
    /// label.
    pub bottom: f64,
    pub width: f64,
    pub height: f64,
}

/// Where a bubble goes: its top-left corner, and how far along its bottom
/// edge the tail sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub left: f64,
    pub top: f64,
    pub tail: f64,
}

/// The gap kept between bubbles and from the stage's edges.
pub const GAP: f64 = 8.0;

/// A box on the stage a bubble must not cover: a party's label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Places bubbles over their parties inside a stage `width` by `height`,
/// side by side without overlapping each other, raised clear of the
/// labels in `keep_clear`, each with its tail pointing at its party.
/// Returns them in the order given.
#[must_use]
pub fn arrange(wants: &[Want], keep_clear: &[Rect], width: f64, height: f64) -> Vec<Placed> {
    let mut order: Vec<usize> = (0..wants.len()).collect();
    order.sort_by(|a, b| wants[*a].x.total_cmp(&wants[*b].x));
    let mut lefts = vec![0.0; wants.len()];
    let mut edge = GAP;
    for &i in &order {
        let w = &wants[i];
        let left = (w.x - w.width / 2.0).max(edge);
        lefts[i] = left;
        edge = left + w.width + GAP;
    }
    // Pushed past the right edge: shift back, keeping the order.
    let mut limit = width - GAP;
    for &i in order.iter().rev() {
        let w = &wants[i];
        if lefts[i] + w.width > limit {
            lefts[i] = (limit - w.width).max(GAP);
        }
        limit = lefts[i] - GAP;
    }
    wants
        .iter()
        .zip(lefts)
        .map(|(w, left)| {
            // Raise the bubble over any label it would cover.
            let mut bottom = w.bottom;
            let right = left + w.width;
            for _ in 0..keep_clear.len() {
                let covered = keep_clear.iter().find(|r| {
                    r.left < right
                        && r.right > left
                        && r.top < bottom
                        && r.bottom > bottom - w.height
                });
                match covered {
                    Some(r) => bottom = r.top - GAP,
                    None => break,
                }
            }
            let top = (bottom - w.height).clamp(GAP, (height - w.height - GAP).max(GAP));
            Placed {
                left,
                top,
                tail: (w.x - left).clamp(14.0, (w.width - 14.0).max(14.0)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text(lines: &[Vec<Span>]) -> String {
        lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn events_print_in_nostr_order_with_content_bold_and_long_strings_cut() {
        let event = json!({
            "sig": "f".repeat(128),
            "content": "A".repeat(600),
            "tags": [["p", "b".repeat(64)], ["requires", "openagents.attested.v1"]],
            "kind": 25910,
            "created_at": 1_760_112_345,
            "pubkey": "a".repeat(64),
            "id": "c".repeat(64),
        });
        let lines = event_lines(&event, "content");
        let shown = text(&lines);
        let first: Vec<&str> = shown.lines().collect();
        assert_eq!(first[0], "{");
        assert!(first[1].starts_with("  \"id\": \"ccc"));
        assert!(first[3].contains("\"created_at\": 1760112345,"));
        assert!(first[4].contains("\"kind\": 25910,"));
        assert!(shown.contains("    [\"p\", \"bbbbbbbbb\u{2026}bbbbbbbbb\"],"));
        assert!(shown.contains("[\"requires\", \"openagents.attested.v1\"]"));
        assert_eq!(*first.last().unwrap(), "}");
        // Only the content is bold, and it is cut but keeps its whole text.
        let bold: Vec<&Span> = lines.iter().flatten().filter(|s| s.bold).collect();
        assert_eq!(bold.len(), 1);
        assert_eq!(
            bold[0].full.as_deref(),
            Some(format!("\"{}\"", "A".repeat(600)).as_str())
        );
        assert!(bold[0].text.chars().count() <= CUT_TO + 2);
        // Short strings are not cut.
        assert!(
            lines
                .iter()
                .flatten()
                .all(|s| s.full.is_none() || s.text.contains('\u{2026}'))
        );
    }

    #[test]
    fn bubbles_sit_over_their_parties_without_overlapping() {
        let want = |x: f64| Want {
            x,
            bottom: 300.0,
            width: 320.0,
            height: 200.0,
        };
        let placed = arrange(&[want(200.0), want(520.0), want(900.0)], &[], 1200.0, 900.0);
        for pair in placed.windows(2) {
            assert!(pair[0].left + 320.0 + GAP <= pair[1].left + 1e-6);
        }
        assert!(placed[2].left + 320.0 <= 1200.0 - GAP + 1e-6);
        assert!((placed[0].top - 100.0).abs() < 1e-6);
        // Each tail points at its party.
        for (p, x) in placed.iter().zip([200.0, 520.0, 900.0]) {
            assert!((p.left + p.tail - x).abs() < 1e-6 || p.tail == 14.0 || p.tail == 306.0);
        }
        // Crowded at the right edge: shifted back, still in order.
        let placed = arrange(&[want(1100.0), want(1150.0)], &[], 1200.0, 900.0);
        assert!(placed[1].left + 320.0 <= 1200.0 - GAP + 1e-6);
        assert!(placed[0].left + 320.0 + GAP <= placed[1].left + 1e-6);
        // Too tall for the room above: kept inside the stage.
        let tall = Want {
            x: 300.0,
            bottom: 50.0,
            width: 320.0,
            height: 200.0,
        };
        assert_eq!(arrange(&[tall], &[], 1200.0, 900.0)[0].top, GAP);
        // A label in the way: the bubble rises over it.
        let label = Rect {
            left: 400.0,
            top: 250.0,
            right: 600.0,
            bottom: 290.0,
        };
        let placed = arrange(&[want(520.0)], &[label], 1200.0, 900.0)[0];
        assert!(placed.top + 200.0 <= label.top - GAP + 1e-6);
    }
}
