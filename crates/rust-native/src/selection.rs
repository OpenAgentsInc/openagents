//! Stable, renderer-independent selection over a transcript's display text.
//!
//! Copy preserves selected UTF-8 bytes, including trailing whitespace. It adds
//! one newline between paragraph fields and rows. A layout update may reorder
//! rows without changing a selection; changing an endpoint's prefix cancels it.

use crate::layout::Frame;
use unicode_segmentation::UnicodeSegmentation;

/// A resolved insertion point in the current frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub row: usize,
    pub text: usize,
    pub byte: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Anchor {
    key: String,
    text: usize,
    prefix: String,
}
impl Anchor {
    fn new(frame: &Frame, point: Position) -> Option<Self> {
        let row = frame.display(point.row)?;
        let text = row.texts.get(point.text)?;
        let prefix = text.get(..point.byte)?;
        if !is_boundary(text, point.byte) {
            return None;
        }
        Some(Self {
            key: row.key.clone(),
            text: point.text,
            prefix: prefix.into(),
        })
    }
    fn resolve(&self, frame: &Frame) -> Option<Position> {
        let row = frame.find(&self.key)?;
        let text = frame.display(row)?.texts.get(self.text)?;
        if !text.starts_with(&self.prefix) || !is_boundary(text, self.prefix.len()) {
            return None;
        }
        Some(Position {
            row,
            text: self.text,
            byte: self.prefix.len(),
        })
    }
}

/// Selection endpoints name stable rows rather than viewport indices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    anchor: Option<Anchor>,
    caret: Option<Anchor>,
}
impl Selection {
    pub fn clear(&mut self) {
        self.anchor = None;
        self.caret = None;
    }
    pub fn begin(&mut self, frame: &Frame, point: Position, extend: bool) {
        let Some(point) = Anchor::new(frame, point) else {
            return;
        };
        if !extend
            || self
                .anchor
                .as_ref()
                .and_then(|a| a.resolve(frame))
                .is_none()
        {
            self.anchor = Some(point.clone());
        }
        self.caret = Some(point);
    }
    pub fn extend(&mut self, frame: &Frame, point: Position) {
        if self.anchor.is_some() {
            self.caret = Anchor::new(frame, point);
        }
    }
    pub fn reconcile(&mut self, frame: &Frame) {
        if self.endpoints(frame).is_none() {
            self.clear();
        }
    }
    pub fn endpoints(&self, frame: &Frame) -> Option<(Position, Position)> {
        Some((
            self.anchor.as_ref()?.resolve(frame)?,
            self.caret.as_ref()?.resolve(frame)?,
        ))
    }
    pub fn ordered(&self, frame: &Frame) -> Option<(Position, Position)> {
        let (a, b) = self.endpoints(frame)?;
        Some(if a <= b { (a, b) } else { (b, a) })
    }
    pub fn collapsed(&self, frame: &Frame) -> bool {
        self.endpoints(frame).is_none_or(|(a, b)| a == b)
    }
    pub fn copy(&self, frame: &Frame) -> String {
        let Some((a, b)) = self.ordered(frame) else {
            return String::new();
        };
        if a == b {
            return String::new();
        }
        let mut pieces = Vec::new();
        for index in a.row..=b.row {
            let Some(row) = frame.display(index) else {
                continue;
            };
            for (field, text) in row.texts.iter().enumerate() {
                let here = (index, field);
                if here < (a.row, a.text) || here > (b.row, b.text) {
                    continue;
                }
                let start = if here == (a.row, a.text) { a.byte } else { 0 };
                let end = if here == (b.row, b.text) {
                    b.byte
                } else {
                    text.len()
                };
                if let Some(part) = text.get(start..end) {
                    pieces.push(part);
                }
            }
        }
        pieces.join("\n")
    }
}

fn is_boundary(text: &str, byte: usize) -> bool {
    byte == text.len() || text.grapheme_indices(true).any(|(at, _)| at == byte)
}

/// Valid editing and selection boundaries, including the end of the text.
pub fn grapheme_boundaries(text: &str) -> Vec<usize> {
    text.grapheme_indices(true)
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{TranscriptLayout, Update, testing::FixedMeasurer};
    use crate::{Element, Node, TextRole, style::Style};
    #[test]
    fn rejects_split_graphemes_and_changed_endpoint_prefixes() {
        let make = |value: &str| Node::<()> {
            key: "a".into(),
            style: Style::default(),
            element: Element::Text {
                value: value.into(),
                role: TextRole::Body,
            },
        };
        let mut layout = TranscriptLayout::new();
        let mut measurer = FixedMeasurer::default();
        layout
            .update(
                Update {
                    width: 300.0,
                    scale: 1.0,
                    order: Some(vec!["a".into()]),
                    rows: vec![make("é 👩‍💻 end")],
                    ..Update::default()
                },
                &mut measurer,
            )
            .unwrap();
        let frame = layout.frame();
        let mut selection = Selection::default();
        selection.begin(
            &frame,
            Position {
                row: 0,
                text: 0,
                byte: 1,
            },
            false,
        );
        assert!(selection.endpoints(&frame).is_none());
        selection.begin(
            &frame,
            Position {
                row: 0,
                text: 0,
                byte: 0,
            },
            false,
        );
        selection.extend(
            &frame,
            Position {
                row: 0,
                text: 0,
                byte: 3,
            },
        );
        assert_eq!(selection.copy(&frame), "é");
        layout
            .update(
                Update {
                    width: 300.0,
                    scale: 1.0,
                    rows: vec![make("changed")],
                    ..Update::default()
                },
                &mut measurer,
            )
            .unwrap();
        selection.reconcile(&layout.frame());
        assert!(selection.endpoints(&layout.frame()).is_none());
    }
}
