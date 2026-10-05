//! Copy mode (the prefix, then `[`) and scrollback search (the prefix,
//! then `/`): a keyboard cursor over the focused pane's scrollback and
//! screen, a selection it grows, and searches that move it, as tmux's copy
//! mode does with vi keys.

use winit::keyboard::{Key as Logical, KeyCode, NamedKey};

use super::layout::PaneId;
use super::select::{self, Point, Selection, Unit};
use super::{KeyIn, Overlay};

/// The help line while copy mode runs.
pub const HELP: &str = "copy mode: arrows or hjkl move · PgUp PgDn · g G top bottom · 0 $ line · w b words · v select · V lines · y or Enter copy · / ? search · n N next · q or Esc leave";

/// Copy mode's state.
#[derive(Clone, Debug)]
pub(super) struct Copy {
    pub pane: PaneId,
    pub cursor: Point,
    /// Where the selection started and what it grows by, once begun.
    anchor: Option<(Point, Unit)>,
    /// The search being typed, and whether it looks toward older lines.
    prompt: Option<(String, bool)>,
    /// The last search and its direction.
    query: Option<(String, bool)>,
    /// What the last search found or missed.
    note: Option<String>,
}

impl Copy {
    /// The header's state text.
    #[must_use]
    pub fn state(&self) -> String {
        if let Some((prompt, older)) = &self.prompt {
            return format!("search {}: {prompt}_", if *older { "up" } else { "down" });
        }
        let mut state = "copy mode".to_owned();
        if self.anchor.is_some() {
            state.push_str(" · selecting");
        }
        if let Some(note) = &self.note {
            state.push_str(" · ");
            state.push_str(note);
        }
        state
    }
}

impl Overlay {
    /// Enters copy mode on the focused pane, its cursor where the
    /// program's is; with `search`, a search prompt opens at once.
    pub(super) fn enter_copy(&mut self, search: bool) {
        let Some(id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get(&id) else {
            return;
        };
        let vt = &pane.session.vt;
        let (row, col) = vt.cursor();
        let cursor = Point {
            line: select::absolute(vt, select::top(vt, pane.scroll) + row.min(vt.rows() - 1)),
            col,
        };
        self.copy = Some(Copy {
            pane: id,
            cursor,
            anchor: None,
            prompt: search.then(|| (String::new(), true)),
            query: None,
            note: None,
        });
    }

    /// Handles a key in copy mode.
    pub(super) fn copy_key(&mut self, key: &KeyIn) {
        let Some(mut copy) = self.copy.take() else {
            return;
        };
        let typed = match &key.logical {
            Logical::Character(s) => s.chars().next(),
            _ => None,
        };
        let named = match &key.logical {
            Logical::Named(named) => Some(*named),
            _ => None,
        };
        if let Some((mut prompt, older)) = copy.prompt.take() {
            match named {
                Some(NamedKey::Enter) => {
                    copy.query = Some((prompt.clone(), older));
                    self.copy = Some(copy);
                    self.search_again(false);
                    return;
                }
                Some(NamedKey::Escape) => {}
                Some(NamedKey::Backspace) => {
                    prompt.pop();
                    copy.prompt = Some((prompt, older));
                }
                _ => {
                    if let Some(text) = key.text.as_deref().filter(|t| !t.is_empty()) {
                        prompt.extend(text.chars().filter(|c| !c.is_control()));
                    }
                    copy.prompt = Some((prompt, older));
                }
            }
            self.copy = Some(copy);
            return;
        }
        copy.note = None;
        let Some(pane) = self.panes.get(&copy.pane) else {
            return;
        };
        let vt = &pane.session.vt;
        let first = vt.history_dropped();
        let last = first + select::total(vt) as u64 - 1;
        let cols = vt.cols();
        let half = (vt.rows() / 2).max(1) as u64;
        let row_of = |line: u64| select::index(vt, line).and_then(|i| vt.line(i));
        let mut cursor = copy.cursor;
        let shift = self.mods.shift_key();
        match (named, typed, key.code) {
            (Some(NamedKey::Escape), ..) | (_, Some('q'), _) => {
                self.leave_copy(copy);
                return;
            }
            (Some(NamedKey::Enter), ..) | (_, Some('y'), _) => {
                self.copy = Some(copy);
                self.copy_selection();
                if let Some(copy) = self.copy.take() {
                    self.leave_copy(copy);
                }
                return;
            }
            (Some(NamedKey::ArrowLeft), ..) | (_, Some('h'), _) => {
                cursor.col = cursor.col.saturating_sub(1);
            }
            (Some(NamedKey::ArrowRight), ..) | (_, Some('l'), _) => {
                cursor.col = (cursor.col + 1).min(cols - 1);
            }
            (Some(NamedKey::ArrowUp), ..) | (_, Some('k'), _) => {
                cursor.line = cursor.line.saturating_sub(1).max(first);
            }
            (Some(NamedKey::ArrowDown), ..) | (_, Some('j'), _) => {
                cursor.line = (cursor.line + 1).min(last);
            }
            (Some(NamedKey::PageUp), ..) => {
                cursor.line = cursor.line.saturating_sub(half).max(first);
            }
            (Some(NamedKey::PageDown), ..) => {
                cursor.line = (cursor.line + half).min(last);
            }
            (_, Some('g'), _) => {
                cursor = Point {
                    line: first,
                    col: 0,
                }
            }
            (_, Some('G'), _) => cursor = Point { line: last, col: 0 },
            (Some(NamedKey::Home), ..) | (_, Some('0' | '^'), _) => cursor.col = 0,
            (Some(NamedKey::End), ..) | (_, Some('$'), _) => {
                cursor.col = row_of(cursor.line)
                    .map_or(0, |r| r.text().chars().count().max(1) - 1)
                    .min(cols - 1);
            }
            (_, Some('w'), _) => cursor = next_word(vt, cursor, last),
            (_, Some('b'), _) => cursor = previous_word(vt, cursor, first),
            (Some(NamedKey::Space), ..) | (_, Some('v'), _) => {
                copy.anchor = match copy.anchor {
                    Some(_) => None,
                    None => Some((cursor, Unit::Char)),
                };
            }
            (_, Some('V'), _) => {
                copy.anchor = match copy.anchor {
                    Some((_, Unit::Line)) => None,
                    _ => Some((cursor, Unit::Line)),
                };
            }
            (_, Some('/'), _) => copy.prompt = Some((String::new(), true)),
            (_, Some('?'), _) => copy.prompt = Some((String::new(), false)),
            (_, Some('n'), _) => {
                self.copy = Some(copy);
                self.search_again(false);
                return;
            }
            (_, Some('N'), _) => {
                self.copy = Some(copy);
                self.search_again(true);
                return;
            }
            (None, None, KeyCode::KeyN) if shift => {
                self.copy = Some(copy);
                self.search_again(true);
                return;
            }
            _ => {}
        }
        copy.cursor = cursor;
        self.copy = Some(copy);
        self.follow_copy();
    }

    fn leave_copy(&mut self, copy: Copy) {
        if let Some(pane) = self.panes.get_mut(&copy.pane) {
            pane.scroll = 0;
        }
    }

    /// Repeats the last search, the other way with `reverse`, moving the
    /// cursor to the match and selecting it.
    fn search_again(&mut self, reverse: bool) {
        let Some(mut copy) = self.copy.take() else {
            return;
        };
        let Some((query, older)) = copy.query.clone() else {
            self.copy = Some(copy);
            return;
        };
        let older = older != reverse;
        if let Some(pane) = self.panes.get(&copy.pane) {
            match select::search(&pane.session.vt, &query, copy.cursor, older) {
                Some((start, end)) => {
                    copy.cursor = start;
                    copy.anchor = None;
                    copy.note = Some(format!("found {query}"));
                    self.copy = Some(copy);
                    if let Some(pane) = self
                        .panes
                        .get_mut(&self.copy.as_ref().map_or(0, |c| c.pane))
                    {
                        pane.selection = Some(Selection {
                            anchor: start,
                            head: end,
                            unit: Unit::Char,
                        });
                    }
                    self.follow_copy_scroll();
                    return;
                }
                None => copy.note = Some(format!("no more {query}")),
            }
        }
        self.copy = Some(copy);
    }

    /// Puts the copy cursor's selection on its pane and scrolls the pane
    /// so the cursor shows.
    fn follow_copy(&mut self) {
        let Some(copy) = &self.copy else {
            return;
        };
        let (id, cursor, anchor) = (copy.pane, copy.cursor, copy.anchor);
        if let Some(pane) = self.panes.get_mut(&id) {
            pane.selection = anchor.map(|(anchor, unit)| Selection {
                anchor,
                head: cursor,
                unit,
            });
        }
        self.follow_copy_scroll();
    }

    fn follow_copy_scroll(&mut self) {
        let Some(copy) = &self.copy else {
            return;
        };
        let (id, cursor) = (copy.pane, copy.cursor);
        let Some(pane) = self.panes.get_mut(&id) else {
            return;
        };
        let vt = &pane.session.vt;
        let Some(index) = select::index(vt, cursor.line) else {
            return;
        };
        let top = select::top(vt, pane.scroll);
        let back = vt.scrollback_len();
        if index < top {
            pane.scroll = back - index;
        } else if index >= top + vt.rows() {
            pane.scroll = back.saturating_sub(index + 1 - vt.rows());
        }
    }
}

/// The start of the next word after `at`, across lines.
fn next_word(vt: &coder_vt::Terminal, at: Point, last: u64) -> Point {
    let mut point = at;
    let mut in_word = char_at(vt, point).is_some_and(select::word_char);
    loop {
        point.col += 1;
        if point.col >= vt.cols() {
            if point.line >= last {
                return at;
            }
            point = Point {
                line: point.line + 1,
                col: 0,
            };
            in_word = false;
        }
        let word = char_at(vt, point).is_some_and(select::word_char);
        if word && !in_word {
            return point;
        }
        in_word = word;
    }
}

/// The start of the word before `at`, across lines.
fn previous_word(vt: &coder_vt::Terminal, at: Point, first: u64) -> Point {
    let mut point = at;
    // Step back over blanks, then to the start of the word.
    loop {
        if point.col == 0 {
            if point.line <= first {
                return point;
            }
            point = Point {
                line: point.line - 1,
                col: vt.cols() - 1,
            };
        } else {
            point.col -= 1;
        }
        if char_at(vt, point).is_some_and(select::word_char) {
            break;
        }
    }
    while point.col > 0
        && char_at(
            vt,
            Point {
                line: point.line,
                col: point.col - 1,
            },
        )
        .is_some_and(select::word_char)
    {
        point.col -= 1;
    }
    point
}

fn char_at(vt: &coder_vt::Terminal, at: Point) -> Option<char> {
    let row = vt.line(select::index(vt, at.line)?)?;
    row.cells.get(at.col).map(|c| c.ch)
}
