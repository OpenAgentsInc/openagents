//! Scrollback text selection for the coder transcript.
//!
//! Mouse capture means the terminal's own selection cannot reach this text,
//! so the transcript builds its own selection on top of the lines it already
//! renders. The model works in *screen* coordinates — absolute row, display
//! column — against the same wrapped `Line`s the last frame drew, because
//! that is what the mouse reports and what the highlight paints over.
//!
//! Copy time is where screen coordinates get translated back to text: each
//! selected row records its plain source line, so the copy is reconstructed
//! from content rather than scraped from styled spans, in logical order,
//! with the same line breaks the reader sees.
//!
//! Interaction follows terminal convention: press-drag-release sweeps, a
//! double click takes a word, a triple click takes the whole row. The
//! selection persists after release — scrollback is for reading, and a
//! highlight that vanishes the instant the button lifts reads as a glitch —
//! and is cleared by any click, scroll, or `Esc`.

use unicode_width::UnicodeWidthStr;

/// One rendered transcript row, recorded at draw time.
///
/// `plain` is the row's text with styles stripped; reconstructing the copy
/// from it keeps the clipboard byte-faithful to what is on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenRow {
    /// Absolute screen row (terminal coordinate, what a `MouseEvent` row is).
    pub screen_y: u16,
    /// First terminal column of the transcript body (the left edge of the
    /// text, after any gutter the renderer draws).
    pub screen_x: u16,
    /// The row's visible text, styles stripped.
    pub plain: String,
}

/// A point inside the recorded rows, resolved from a mouse position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row_index: usize,
    /// Character offset into that row's `plain` text.
    pub char_index: usize,
}

/// One selected row of the final selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRow {
    pub row_index: usize,
    /// Inclusive character range into the row's `plain` text.
    pub char_start: usize,
    /// Exclusive character end.
    pub char_end: usize,
}

impl SelectedRow {
    /// The selected substring of the row's source text.
    pub fn slice_of<'a>(&self, plain: &'a str) -> &'a str {
        let start = plain
            .char_indices()
            .nth(self.char_start)
            .map(|(byte, _)| byte)
            .unwrap_or(plain.len());
        let end = plain
            .char_indices()
            .nth(self.char_end)
            .map(|(byte, _)| byte)
            .unwrap_or(plain.len());
        plain.get(start..end).unwrap_or("")
    }
}

/// The current selection: an anchor, a head, and the gesture that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    anchor: SelectionPoint,
    head: SelectionPoint,
    /// Whether the anchor edge is inclusive. A word or row gesture selects
    /// the whole unit under the anchor; a drag selects half-open spans.
    pub kind: SelectionKind,
}

/// What produced the selection, which fixes its boundary semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// Character-precise sweep from a press-drag-release.
    Drag,
    /// Double click: the whole word under the anchor, extended word-wise.
    Word,
    /// Triple click: the whole row.
    Row,
}

impl Selection {
    fn new(anchor: SelectionPoint, head: SelectionPoint, kind: SelectionKind) -> Self {
        Self {
            anchor,
            head,
            kind,
        }
    }

    /// The selected ranges, ordered from the earliest row.
    ///
    /// `plain_rows` must be the same slice the selection was built against.
    pub fn selected_rows(&self, plain_rows: &[ScreenRow]) -> Vec<SelectedRow> {
        // Order the endpoints once, so a backward drag yields the same text
        // as the forward one and per-row clamping below stays simple.
        let (first_point, last_point) = if (self.anchor.row_index, self.anchor.char_index)
            <= (self.head.row_index, self.head.char_index)
        {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        let (first, first_start) = self.edge_at(plain_rows, first_point, true);
        let (last, last_end) = self.edge_at(plain_rows, last_point, false);
        let mut rows = Vec::new();
        for row_index in first..=last {
            let Some(row) = plain_rows.get(row_index) else {
                continue;
            };
            let char_count = row.plain.chars().count();
            let start = if row_index == first {
                first_start.min(char_count)
            } else {
                0
            };
            let end = if row_index == last {
                last_end.min(char_count)
            } else {
                char_count
            };
            if start < end {
                rows.push(SelectedRow {
                    row_index,
                    char_start: start,
                    char_end: end,
                });
            }
        }
        rows
    }

    /// One end of the span: the row it lands on and the character offset
    /// along it. Row gestures span the whole row; drag and word gestures
    /// carry already-snapped character bounds.
    fn edge_at(
        &self,
        plain_rows: &[ScreenRow],
        at: SelectionPoint,
        is_start: bool,
    ) -> (usize, usize) {
        let row_index = at.row_index;
        let Some(row) = plain_rows.get(row_index) else {
            return (row_index, 0);
        };
        let char_count = row.plain.chars().count();
        let value = match self.kind {
            SelectionKind::Row => {
                if is_start {
                    0
                } else {
                    char_count
                }
            }
            // The head of a drag includes the cell under the cursor, which
            // is what terminal selections do; word bounds are already
            // exclusive.
            SelectionKind::Drag if !is_start => at.char_index.saturating_add(1).min(char_count),
            SelectionKind::Drag | SelectionKind::Word => at.char_index.min(char_count),
        };
        (row_index, value)
    }
}

/// Snap a character index to the start or end of the word containing it.
///
/// A bound only walks through word characters, so a click on whitespace
/// yields an empty span — the caller treats that as no word to take — and a
/// click mid-word stays inside that word.
fn word_bound(text: &str, index: usize, start: bool) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let index = index.min(chars.len());
    let is_word = |c: char| !c.is_whitespace() && c != '·' && c != '⏺' && c != '○' && c != '↔';
    if start {
        let mut i = index;
        while i > 0 && is_word(chars[i - 1]) {
            i -= 1;
        }
        i
    } else {
        let mut i = index;
        while i < chars.len() && is_word(chars[i]) {
            i += 1;
        }
        i
    }
}

/// Click timing for double- and triple-click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ClickStage {
    #[default]
    None,
    Single,
    Double,
    Triple,
}

/// The selection state machine, held by the UI between frames.
#[derive(Debug, Default)]
pub struct SelectionState {
    /// Rows the last frame drew, in screen coordinates.
    rows: Vec<ScreenRow>,
    selection: Option<Selection>,
    /// A drag in progress: the anchor is fixed, the head follows the mouse.
    dragging: bool,
    /// Where the drag started: the mouse cell and the selection point it
    /// resolved to. Held so the drag threshold can compare raw cells while
    /// the selection keeps the snapped point.
    drag_anchor: Option<(u16, u16, SelectionPoint)>,
    click_stage: ClickStage,
    last_click: Option<(u16, u16, std::time::Instant)>,
}

/// Double/triple clicks must land within this window, and on the same cell,
/// to count as one gesture continuing.
const MULTI_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);
/// A press must travel this many rows/columns before it is a drag, so a
/// shaky click does not destroy the selection it was meant to keep.
const DRAG_THRESHOLD: i32 = 2;

impl SelectionState {
    /// Record the rows the frame just drew. Called once per frame.
    pub fn observe_rows(&mut self, rows: Vec<ScreenRow>) {
        self.rows = rows;
    }

    /// The rows recorded at the last frame.
    pub fn rows(&self) -> &[ScreenRow] {
        &self.rows
    }

    /// The active selection, if any.
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Whether a drag is mid-flight.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Feed one mouse event. Returns `Some(())` when the event changed the
    /// selection (the caller must redraw), `None` otherwise.
    pub fn handle_mouse(
        &mut self,
        column: u16,
        row: u16,
        event_kind: crossterm::event::MouseEventKind,
    ) -> Option<()> {
        match event_kind {
            crossterm::event::MouseEventKind::Down(_) => self.on_press(column, row),
            crossterm::event::MouseEventKind::Drag(_) => self.on_drag(column, row),
            crossterm::event::MouseEventKind::Up(_) => self.on_release(column, row),
            _ => None,
        }
    }

    fn on_press(&mut self, column: u16, row: u16) -> Option<()> {
        let point = self.point_at(column, row)?;
        let now = std::time::Instant::now();
        let same_cell = self
            .last_click
            .is_some_and(|(c, r, _)| c == column && r == row);
        let within_window = self
            .last_click
            .is_some_and(|(_, _, at)| now.duration_since(at) <= MULTI_CLICK_WINDOW);
        let stage = if same_cell && within_window {
            match self.click_stage {
                ClickStage::Single => ClickStage::Double,
                ClickStage::Double | ClickStage::Triple => ClickStage::Triple,
                ClickStage::None => ClickStage::Single,
            }
        } else {
            ClickStage::Single
        };
        self.last_click = Some((column, row, now));
        self.click_stage = stage;
        match stage {
            ClickStage::Double => {
                // Double click: take the word under the cursor.
                self.dragging = false;
                self.drag_anchor = None;
                let plain = self.rows[point.row_index].plain.clone();
                let anchor = SelectionPoint {
                    row_index: point.row_index,
                    char_index: word_bound(&plain, point.char_index, true),
                };
                let head = SelectionPoint {
                    row_index: point.row_index,
                    char_index: word_bound(&plain, point.char_index, false),
                };
                if anchor.char_index < head.char_index {
                    self.selection = Some(Selection::new(anchor, head, SelectionKind::Word));
                    Some(())
                } else {
                    // The click landed on whitespace or a gutter glyph: no
                    // word to take, and the old selection is already gone.
                    None
                }
            }
            ClickStage::Triple => {
                // Triple click: the whole row.
                self.dragging = false;
                self.drag_anchor = None;
                let char_count = self.rows[point.row_index].plain.chars().count();
                self.selection = Some(Selection::new(
                    SelectionPoint {
                        row_index: point.row_index,
                        char_index: 0,
                    },
                    SelectionPoint {
                        row_index: point.row_index,
                        char_index: char_count,
                    },
                    SelectionKind::Row,
                ));
                Some(())
            }
            ClickStage::Single | ClickStage::None => {
                // First press of a gesture: collapse the old selection and
                // arm a drag. The drag threshold keeps a plain click from
                // being treated as a sweep.
                self.dragging = true;
                self.selection = None;
                self.drag_anchor = Some((column, row, point));
                Some(())
            }
        }
    }

    fn on_drag(&mut self, column: u16, row: u16) -> Option<()> {
        if !self.dragging {
            return None;
        }
        let (anchor_col, anchor_row, anchor_point) = self.drag_anchor?;
        // Below the threshold, keep waiting: this is still "a click".
        let moved =
            (column as i32 - anchor_col as i32).abs() + (row as i32 - anchor_row as i32).abs();
        if moved < DRAG_THRESHOLD && self.selection.is_none() {
            return None;
        }
        let head = self.point_at(column, row)?;
        self.selection = Some(Selection::new(
            anchor_point,
            head,
            SelectionKind::Drag,
        ));
        Some(())
    }

    fn on_release(&mut self, column: u16, row: u16) -> Option<()> {
        self.dragging = false;
        self.drag_anchor = None;
        let _ = (column, row);
        None
    }

    /// Clear the selection. Any click, scroll, or Esc does this.
    pub fn clear(&mut self) {
        self.selection = None;
        self.dragging = false;
        self.drag_anchor = None;
    }

    /// Resolve a mouse position to a point in the recorded rows.
    fn point_at(&self, column: u16, row: u16) -> Option<SelectionPoint> {
        let row_index = self
            .rows
            .iter()
            .position(|candidate| candidate.screen_y == row)?;
        let row = &self.rows[row_index];
        // The transcript body may start right of column zero.
        let offset = column.saturating_sub(row.screen_x) as usize;
        // Map a display column to a character index through the width table:
        // the first character whose start column reaches the click is the hit.
        let mut display = 0usize;
        for (byte, ch) in row.plain.char_indices() {
            if display >= offset {
                return Some(SelectionPoint {
                    row_index,
                    char_index: byte_to_char_index(&row.plain, byte),
                });
            }
            display += UnicodeWidthStr::width(ch.to_string().as_str());
        }
        // Clicking past the end of a short row selects to its end.
        Some(SelectionPoint {
            row_index,
            char_index: row.plain.chars().count(),
        })
    }

    /// Reconstruct the copied text.
    ///
    /// Word and row selections join their rows with newlines. A drag across
    /// rows also joins with newlines: the transcript is line-oriented, and
    /// every row the renderer drew is a line the reader sees.
    pub fn copy_text(&self) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let mut text = String::new();
        for selected in selection.selected_rows(&self.rows) {
            let row = &self.rows[selected.row_index];
            let piece = selected.slice_of(&row.plain);
            text.push_str(piece);
            text.push('\n');
        }
        // One trailing newline is layout, not content.
        while text.ends_with('\n') {
            text.pop();
        }
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// Byte offset to character offset, for a string whose prefixes are ASCII
/// enough that `char_indices` counts stay stable.
fn byte_to_char_index(text: &str, byte: usize) -> usize {
    text.char_indices()
        .take_while(|(b, _)| *b < byte)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(screen_y: u16, plain: &str) -> ScreenRow {
        ScreenRow {
            screen_y,
            screen_x: 0,
            plain: plain.to_string(),
        }
    }

    fn state(rows: Vec<ScreenRow>) -> SelectionState {
        let mut state = SelectionState::default();
        state.observe_rows(rows);
        state
    }

    fn drag(state: &mut SelectionState, from: (u16, u16), to: (u16, u16)) {
        state.handle_mouse(from.0, from.1, crossterm::event::MouseEventKind::Down(
            crossterm::event::MouseButton::Left,
        ));
        state.handle_mouse(
            to.0,
            to.1,
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            to.0,
            to.1,
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        );
    }

    #[test]
    fn drag_across_rows_copies_logical_lines() {
        let mut state = state(vec![
            row(0, "first line of output"),
            row(1, "second line here"),
            row(2, "third line"),
        ]);
        drag(&mut state, (0, 0), (10, 1));
        assert_eq!(
            state.copy_text().as_deref(),
            Some("first line of output\nsecond line")
        );
    }

    #[test]
    fn partial_drag_stops_at_the_head_column() {
        let mut state = state(vec![row(0, "copy this text"), row(1, "not this")]);
        drag(&mut state, (5, 0), (8, 0));
        assert_eq!(state.copy_text().as_deref(), Some("this"));
    }

    #[test]
    fn a_click_clears_and_does_not_copy() {
        let mut state = state(vec![row(0, "some text")]);
        drag(&mut state, (0, 0), (3, 0));
        assert!(state.copy_text().is_some());
        // A plain press with no drag leaves nothing selected.
        state.handle_mouse(
            1,
            0,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            1,
            0,
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        );
        assert!(state.copy_text().is_none());
    }

    #[test]
    fn double_click_selects_a_word() {
        let mut state = state(vec![row(0, "select the word here")]);
        state.handle_mouse(
            11,
            0,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            11,
            0,
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            11,
            0,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        );
        assert_eq!(state.copy_text().as_deref(), Some("word"));
    }

    #[test]
    fn triple_click_selects_the_row() {
        let mut state = state(vec![row(0, "whole line please"), row(1, "not me")]);
        for _ in 0..3 {
            state.handle_mouse(
                3,
                0,
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            );
            state.handle_mouse(
                3,
                0,
                crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
            );
        }
        assert_eq!(state.copy_text().as_deref(), Some("whole line please"));
    }

    #[test]
    fn clicks_far_apart_do_not_double() {
        let mut state = state(vec![row(0, "word one and word two")]);
        state.handle_mouse(
            2,
            0,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            2,
            0,
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        );
        state.handle_mouse(
            16,
            0,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        );
        assert!(state.selection().is_none(), "a fresh press clears");
    }

    #[test]
    fn clear_empties_the_selection() {
        let mut state = state(vec![row(0, "text")]);
        drag(&mut state, (0, 0), (4, 0));
        assert!(state.copy_text().is_some());
        state.clear();
        assert!(state.copy_text().is_none());
    }

    #[test]
    fn clicking_past_the_end_selects_to_the_end() {
        let mut state = state(vec![row(0, "short"), row(1, "longer row")]);
        drag(&mut state, (0, 0), (40, 1));
        assert_eq!(
            state.copy_text().as_deref(),
            Some("short\nlonger row")
        );
    }

    #[test]
    fn wide_characters_map_columns_correctly() {
        // CJK characters are two columns wide.
        let mut state = state(vec![row(0, "日本語 text")]);
        drag(&mut state, (0, 0), (7, 0));
        assert_eq!(state.copy_text().as_deref(), Some("日本語 t"));
    }

    #[test]
    fn drag_backward_selects_the_same_text() {
        let mut state = state(vec![row(0, "abcdefgh")]);
        drag(&mut state, (6, 0), (2, 0));
        // The cell under the cursor is included in both directions, so a
        // backward sweep copies exactly what the forward one does.
        assert_eq!(state.copy_text().as_deref(), Some("cdefg"));
    }
}
