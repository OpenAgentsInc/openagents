//! The thread picker that `/resume` and Ctrl+T open.
//!
//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `crates/codegen/xai-grok-pager/src/views/session_picker.rs` (rows under a
//! header per project, this folder's first and the rest alphabetical; the
//! right-aligned "5m ago"; the fields an expanded row shows; a typed query
//! expanding every row it finds), `views/picker.rs` (`handle_picker_input`'s
//! keys, `render_picker_row`, the header rule, the " search: " bar and its
//! "/ to search" hint, the divider under it), `slash/commands/resume.rs`
//! (`/resume` opens the picker), and `app/session_title_resolve.rs` (a title
//! matches whole and case-insensitively; among duplicates a sole renamed one
//! wins; an ID always means the ID).
//!
//! Adapted: threads in place of sessions. A thread's group is its Coder
//! project; a chat with none sits under "Chats", right after this folder's
//! group. Within a group, rows keep the shared chat-list order (#10100). The
//! white ladder's intensities stand in for grok-build's theme grays, and an
//! expanded field's value is clipped rather than wrapped. Ctrl+N starts a
//! thread and Ctrl+A archives one, as the thread list did; grok-build's `d`
//! delete, `f` source filter, and Ctrl+W worktree have nothing to act on
//! here. `/resume` takes an ID, an ID prefix, or a title.

use std::collections::{BTreeSet, HashSet};

use coder_terminal::components::{cells, clip, sanitize};
use coder_terminal::{Colors, Intensity, Ladder, frame, rail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openagents_chat::basic_chats::Summary;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// The picker's title, on its top rule.
pub const TITLE: &str = "Resume thread";
/// The keys, on its bottom rule.
pub const HINT: &str =
    "↑/↓ nav · Enter select · e expand · y copy ID · Ctrl+N new · Ctrl+A archive · Esc close";
/// The group of threads with no Coder project.
pub const CHATS: &str = "Chats";
/// The widest the picker draws.
const WIDTH_MAX: u16 = 100;
/// Where an expanded row's fields start, and their label column.
const FIELD_INDENT: u16 = 4;
const FIELD_LABEL: usize = 12;

/// The picker's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picker {
    /// Every thread, in the shared chat-list order.
    pub rows: Vec<Summary>,
    /// The position in [`Picker::entries`], headers included.
    pub selected: usize,
    pub query: String,
    /// Keys type into the query.
    pub search: bool,
    /// No row shows selected: search was entered from the list's edge.
    pub hidden: bool,
    /// The threads whose fields show.
    pub expanded: BTreeSet<String>,
    /// This folder's name: its project's group comes first.
    pub here: String,
}

/// One position in the picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry<'a> {
    Header(&'a str),
    Row(&'a Summary),
}

/// What a key in the picker asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    Nothing,
    Open(String),
    New,
    Archive(String),
    /// Copy this thread's ID.
    Copy(String),
    Close,
}

impl Picker {
    /// The picker over `rows` (any order), this folder named `here`, the
    /// first row selected.
    pub fn new(rows: Vec<Summary>, here: &str) -> Self {
        let rows = openagents_chat_app::chat_list::search(&rows, "")
            .into_iter()
            .cloned()
            .collect();
        let mut picker = Self {
            rows,
            here: here.to_owned(),
            ..Self::default()
        };
        picker.selected = picker.first();
        picker
    }

    /// The picker narrowed to `query`, every row it finds expanded.
    pub fn with_query(mut self, query: &str) -> Self {
        self.query = query.to_owned();
        self.queried();
        self
    }

    /// The headers and rows, in order.
    pub fn entries(&self) -> Vec<Entry<'_>> {
        let mut groups: Vec<(&str, Vec<&Summary>)> = Vec::new();
        for row in found(&self.rows, &self.query) {
            let label = group(row);
            match groups.iter_mut().find(|(name, _)| *name == label) {
                Some((_, members)) => members.push(row),
                None => groups.push((label, vec![row])),
            }
        }
        groups.sort_by(|a, b| a.0.cmp(b.0));
        if let Some(at) = groups.iter().position(|(name, _)| *name == CHATS) {
            let chats = groups.remove(at);
            groups.insert(0, chats);
        }
        if let Some(at) = groups.iter().position(|(name, _)| *name == self.here) {
            let here = groups.remove(at);
            groups.insert(0, here);
        }
        groups
            .into_iter()
            .flat_map(|(name, members)| {
                std::iter::once(Entry::Header(name)).chain(members.into_iter().map(Entry::Row))
            })
            .collect()
    }

    /// The selected thread.
    pub fn current(&self) -> Option<&Summary> {
        match self.entries().get(self.selected) {
            Some(Entry::Row(row)) => Some(row),
            _ => None,
        }
    }

    fn first(&self) -> usize {
        self.entries()
            .iter()
            .position(|entry| matches!(entry, Entry::Row(_)))
            .unwrap_or(0)
    }

    fn last(&self) -> usize {
        self.entries()
            .iter()
            .rposition(|entry| matches!(entry, Entry::Row(_)))
            .unwrap_or(0)
    }

    /// The query changed: the first row selected, and every row it finds
    /// expanded (none with no query).
    fn queried(&mut self) {
        self.expanded.clear();
        if !self.query.trim().is_empty() {
            self.expanded = found(&self.rows, &self.query)
                .into_iter()
                .map(|row| row.id.clone())
                .collect();
        }
        self.selected = self.first();
        self.hidden = false;
    }

    /// Move to the next row down (`down`) or up, past headers.
    fn step(&mut self, down: bool) {
        let entries = self.entries();
        let mut at = self.selected;
        loop {
            at = if down {
                at + 1
            } else {
                match at.checked_sub(1) {
                    Some(at) => at,
                    None => return,
                }
            };
            match entries.get(at) {
                Some(Entry::Row(_)) => {
                    self.selected = at;
                    self.hidden = false;
                    return;
                }
                Some(Entry::Header(_)) => {}
                None => return,
            }
        }
    }

    fn expand(&mut self, open: bool) {
        let Some(id) = self.current().map(|row| row.id.clone()) else {
            return;
        };
        // `e` and Right toggle, as grok-build's expand does; Left and `E`
        // only collapse.
        if open && !self.expanded.remove(&id) {
            self.expanded.insert(id);
        } else if !open {
            self.expanded.remove(&id);
        }
    }

    /// One key.
    pub fn key(&mut self, key: &KeyEvent) -> Picked {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('n') => Picked::New,
                KeyCode::Char('a') => self
                    .current()
                    .map_or(Picked::Nothing, |row| Picked::Archive(row.id.clone())),
                _ => Picked::Nothing,
            };
        }
        let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if self.search {
            match key.code {
                KeyCode::Esc => {
                    self.search = false;
                    self.hidden = false;
                    return Picked::Nothing;
                }
                KeyCode::Up => {
                    self.search = false;
                    self.hidden = false;
                    self.selected = self.last();
                    return Picked::Nothing;
                }
                KeyCode::Down => {
                    self.search = false;
                    self.hidden = false;
                    self.selected = self.first();
                    return Picked::Nothing;
                }
                KeyCode::Enter => {
                    self.search = false;
                    self.hidden = false;
                }
                KeyCode::Right if self.query.is_empty() => {
                    self.expand(true);
                    return Picked::Nothing;
                }
                KeyCode::Left if self.query.is_empty() => {
                    self.expand(false);
                    return Picked::Nothing;
                }
                KeyCode::Backspace => {
                    if self.query.pop().is_some() {
                        self.queried();
                    }
                    return Picked::Nothing;
                }
                KeyCode::Char(c) if plain => {
                    self.query.push(c);
                    self.queried();
                    return Picked::Nothing;
                }
                _ => return Picked::Nothing,
            }
        }
        let entries_have_rows = self
            .entries()
            .iter()
            .any(|entry| matches!(entry, Entry::Row(_)));
        match key.code {
            KeyCode::Esc => {
                if self.query.is_empty() {
                    return Picked::Close;
                }
                self.query.clear();
                self.queried();
            }
            KeyCode::Enter => {
                if let Some(row) = self.current() {
                    return Picked::Open(row.id.clone());
                }
                // Nothing found: an ID typed whole opens that thread.
                let typed = self.query.trim();
                if !entries_have_rows && openagents_chat::client::thread_id(typed) {
                    return Picked::Open(typed.to_owned());
                }
            }
            KeyCode::Down | KeyCode::Char('j') if plain => {
                if !entries_have_rows || self.selected == self.last() {
                    self.search = true;
                    self.hidden = true;
                } else {
                    self.step(true);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if plain => {
                if !entries_have_rows || self.selected == self.first() {
                    self.search = true;
                    self.hidden = true;
                } else {
                    self.step(false);
                }
            }
            KeyCode::Char('y') if key.modifiers.is_empty() => {
                if let Some(row) = self.current() {
                    return Picked::Copy(row.id.clone());
                }
            }
            KeyCode::Char('e') | KeyCode::Right if key.modifiers.is_empty() => self.expand(true),
            KeyCode::Char('E') | KeyCode::Left => self.expand(false),
            KeyCode::Char('/') if key.modifiers.is_empty() => self.search = true,
            KeyCode::Char(c) if plain => {
                self.query.push(c);
                self.search = true;
                self.queried();
            }
            _ => {}
        }
        Picked::Nothing
    }

    /// Draws the picker centered over `area`. `open` is the open thread;
    /// `now` is Unix seconds.
    pub fn render(&self, area: Rect, buf: &mut Buffer, ladder: Ladder, open: &str, now: u64) {
        let area = area.intersection(buf.area);
        let entries = self.entries();
        let width = area.width.saturating_sub(4).min(WIDTH_MAX);
        let inside = width.saturating_sub(2);
        let lines = self.lines(&entries, inside, ladder, open, now);
        // The rules, the search bar and its divider, then the rows.
        let wanted = u16::try_from(lines.len().max(1) + 4).unwrap_or(u16::MAX);
        let height = area.height.saturating_sub(2).min(wanted);
        let bounds = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        if bounds.is_empty() {
            return;
        }
        let field = Style::new().bg(ladder.background());
        for y in bounds.top()..bounds.bottom() {
            for x in bounds.left()..bounds.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(field);
            }
        }
        let rule = ladder.style(Intensity::Quarter).bg(ladder.background());
        frame(bounds, buf, rule);
        if bounds.height < 3 || bounds.width < 6 {
            return;
        }
        let room = usize::from(bounds.width).saturating_sub(6);
        rail(
            bounds,
            buf,
            0,
            Some((&clip(TITLE, room), ladder.style(Intensity::Full))),
            None,
        );
        rail(
            bounds,
            buf,
            bounds.height - 1,
            Some((&clip(HINT, room), ladder.style(Intensity::Half))),
            None,
        );
        let x = bounds.left() + 1;
        let mut y = bounds.top() + 1;
        let bottom = bounds.bottom() - 1;
        // The search bar.
        if self.search || !self.query.is_empty() {
            let label = " search: ";
            buf.set_stringn(
                x,
                y,
                label,
                usize::from(inside),
                ladder.style(Intensity::Half),
            );
            let at = x + cells(label) as u16;
            let room = usize::from(inside).saturating_sub(cells(label) + 1);
            let query = clip(&sanitize(&self.query), room);
            buf.set_stringn(at, y, &query, room, ladder.style(Intensity::Full));
            if self.search {
                let cursor = at + cells(&query) as u16;
                if cursor < bounds.right() - 1 {
                    buf[(cursor, y)].set_style(
                        ladder
                            .style(Intensity::Full)
                            .add_modifier(Modifier::REVERSED),
                    );
                }
            }
        } else {
            buf.set_stringn(
                x,
                y,
                " / to search",
                usize::from(inside),
                ladder.style(Intensity::Quarter),
            );
        }
        y += 1;
        if y >= bottom {
            return;
        }
        buf.set_stringn(
            x,
            y,
            "─".repeat(usize::from(inside)),
            usize::from(inside),
            rule,
        );
        y += 1;
        let visible = usize::from(bottom.saturating_sub(y));
        if visible == 0 {
            return;
        }
        if lines.is_empty() {
            let empty = if self.query.is_empty() {
                "No threads yet. Press Ctrl+N for a new one."
            } else {
                "No threads match."
            };
            buf.set_stringn(
                x + 1,
                y,
                clip(empty, usize::from(inside).saturating_sub(2)),
                usize::from(inside),
                ladder.style(Intensity::Half),
            );
            return;
        }
        // Keep the selected row, and its fields when they fit, in view.
        let (start, end) = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.entry == Some(self.selected))
            .fold((None, 0), |(start, _), (at, _)| {
                (start.or(Some(at)), at + 1)
            });
        let start = start.unwrap_or(0);
        let first = end.saturating_sub(visible).min(start);
        for (offset, line) in lines.iter().skip(first).take(visible).enumerate() {
            let row_y = y + offset as u16;
            if line.band {
                let mut band = Style::new().bg(ladder.selection());
                if ladder.colors() == Colors::None {
                    band = band.add_modifier(Modifier::REVERSED);
                }
                for cx in x..x + inside {
                    buf[(cx, row_y)].set_style(band);
                }
            }
            let mut at = x;
            for (text, style) in &line.spans {
                let room = usize::from((x + inside).saturating_sub(at));
                if room == 0 {
                    break;
                }
                if text.is_empty() {
                    continue;
                }
                let style = if line.band {
                    style.bg(ladder.selection())
                } else {
                    *style
                };
                buf.set_stringn(at, row_y, text, room, style);
                at += (cells(text) as u16).min(room as u16);
            }
        }
    }

    /// Every drawn line under the divider, `width` cells wide.
    fn lines(
        &self,
        entries: &[Entry<'_>],
        width: u16,
        ladder: Ladder,
        open: &str,
        now: u64,
    ) -> Vec<Drawn> {
        let width = usize::from(width);
        let mut lines = Vec::new();
        for (at, entry) in entries.iter().enumerate() {
            match entry {
                Entry::Header(name) => {
                    let title = format!(" {} ", clip(&sanitize(name), width.saturating_sub(2)));
                    let rest = width.saturating_sub(cells(&title));
                    lines.push(Drawn {
                        entry: None,
                        band: false,
                        spans: vec![
                            (
                                title,
                                ladder.style(Intensity::Half).add_modifier(Modifier::BOLD),
                            ),
                            ("─".repeat(rest), ladder.style(Intensity::Quarter)),
                        ],
                    });
                }
                Entry::Row(row) => {
                    let selected = at == self.selected && !self.hidden;
                    let expanded = self.expanded.contains(&row.id);
                    lines.push(row_line(
                        at, row, selected, expanded, width, ladder, open, now,
                    ));
                    if expanded {
                        for (label, value) in fields(row) {
                            let lead = format!(
                                "{}{label:<FIELD_LABEL$} ",
                                " ".repeat(usize::from(FIELD_INDENT))
                            );
                            let room = width.saturating_sub(cells(&lead));
                            lines.push(Drawn {
                                entry: Some(at),
                                band: false,
                                spans: vec![
                                    (lead, ladder.style(Intensity::Half)),
                                    (
                                        clip(&sanitize(&value), room),
                                        ladder.style(Intensity::ThreeQuarters),
                                    ),
                                ],
                            });
                        }
                    }
                }
            }
        }
        lines
    }
}

/// One drawn line: its spans, whether it carries the selection band, and
/// the entry it belongs to.
struct Drawn {
    entry: Option<usize>,
    band: bool,
    spans: Vec<(String, Style)>,
}

/// A row as `render_picker_row` draws it: indent, the fold glyph, the
/// title, its badge, and the time right-aligned with a cell to spare.
#[allow(clippy::too_many_arguments)]
fn row_line(
    at: usize,
    row: &Summary,
    selected: bool,
    expanded: bool,
    width: usize,
    ladder: Ladder,
    open: &str,
    now: u64,
) -> Drawn {
    let meta = ladder.style(Intensity::Half);
    let label_style = if selected {
        ladder.style(Intensity::Full).add_modifier(Modifier::BOLD)
    } else {
        ladder.style(Intensity::ThreeQuarters)
    };
    let indent = "  ";
    let glyph = if expanded { "◆ " } else { "› " };
    let badge = badge(row, open);
    let prefix = cells(indent) + cells(glyph);
    let badge_width = if badge.is_empty() {
        0
    } else {
        cells(&badge) + 1
    };
    let content = width.saturating_sub(prefix + 1 + badge_width);
    let usable = content.saturating_sub(2);
    let right = clip(&time_ago(row.updated, now), usable / 2);
    let label = clip(&sanitize(&title(row)), usable.saturating_sub(cells(&right)));
    let gap = width.saturating_sub(prefix + cells(&label) + badge_width + cells(&right) + 1);
    let mut spans = vec![
        (indent.to_owned(), label_style),
        (glyph.to_owned(), meta),
        (label, label_style),
    ];
    if !badge.is_empty() {
        spans.push((format!(" {badge}"), meta));
    }
    spans.push((" ".repeat(gap), meta));
    spans.push((right, meta));
    Drawn {
        entry: Some(at),
        band: selected,
        spans,
    }
}

/// A thread's title, or "New thread" before it has one.
pub fn title(row: &Summary) -> String {
    if row.title.trim().is_empty() {
        "New thread".to_owned()
    } else {
        row.title.clone()
    }
}

/// What sits right after a row's title: open now, pinned, Coder, archived.
fn badge(row: &Summary, open: &str) -> String {
    let mut parts = Vec::new();
    if row.id == open {
        parts.push("open");
    }
    if row.pinned {
        parts.push("pinned");
    }
    if row.coder.is_some() {
        parts.push("Coder");
    }
    if row.archived {
        parts.push("archived");
    }
    parts.join(" · ")
}

/// An expanded row's fields.
fn fields(row: &Summary) -> Vec<(&'static str, String)> {
    let mut fields = vec![("ID", row.id.clone())];
    if let Some(coder) = &row.coder {
        if let Some(project) = &coder.project {
            fields.push(("Project", project.clone()));
        }
        fields.push(("Coder task", coder.task.clone()));
        fields.push(("Host", coder.host.clone()));
    }
    fields.push(("Created", when(row.started)));
    fields.push(("Updated", when(row.updated)));
    fields
}

/// The group a thread sits under: its Coder project, else [`CHATS`].
fn group(row: &Summary) -> &str {
    row.coder
        .as_ref()
        .and_then(|coder| coder.project.as_deref())
        .filter(|project| !project.trim().is_empty())
        .unwrap_or(CHATS)
}

/// The rows `query` finds, in list order: the shared search (title and
/// project), and an ID that starts with it.
fn found<'a>(rows: &'a [Summary], query: &str) -> Vec<&'a Summary> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return rows.iter().collect();
    }
    let hits: HashSet<&str> = openagents_chat_app::chat_list::search(rows, query)
        .into_iter()
        .map(|row| row.id.as_str())
        .collect();
    rows.iter()
        .filter(|row| hits.contains(row.id.as_str()) || row.id.starts_with(&needle))
        .collect()
}

/// The thread `/resume ARG` or `--resume ARG` names: a whole ID; else one
/// title equal to it, ignoring case (among several, the sole renamed one);
/// else the one ID that starts with it. `None` when nothing or more than
/// one thing matches: the picker then opens narrowed to it.
pub fn resolve<'a>(rows: &'a [Summary], arg: &str) -> Option<&'a Summary> {
    let arg = arg.trim();
    if arg.is_empty() {
        return None;
    }
    if openagents_chat::client::thread_id(arg) {
        return rows.iter().find(|row| row.id == arg);
    }
    let needle = arg.to_lowercase();
    let titled: Vec<&Summary> = rows
        .iter()
        .filter(|row| row.title.trim().to_lowercase() == needle)
        .collect();
    match titled.as_slice() {
        [only] => return Some(only),
        [] => {}
        many => {
            let named: Vec<&&Summary> = many.iter().filter(|row| row.named).collect();
            return match named.as_slice() {
                [only] => Some(only),
                _ => None,
            };
        }
    }
    let prefixed: Vec<&Summary> = rows
        .iter()
        .filter(|row| row.id.starts_with(&needle))
        .collect();
    match prefixed.as_slice() {
        [only] => Some(only),
        _ => None,
    }
}

/// "just now", "5m ago", "3h ago", "2d ago", "4mo ago", right-aligned in
/// eight cells (grok-build's `format_time_ago`).
pub fn time_ago(then: u64, now: u64) -> String {
    let minutes = now.saturating_sub(then) / 60;
    let raw = if minutes < 1 {
        "just now".to_owned()
    } else if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 24 * 60 {
        format!("{}h ago", minutes / 60)
    } else if minutes < 30 * 24 * 60 {
        format!("{}d ago", minutes / (24 * 60))
    } else {
        format!("{}mo ago", minutes / (30 * 24 * 60))
    };
    format!("{raw:>8}")
}

/// "Oct 02, 15:04 UTC" for Unix seconds.
fn when(at: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = i64::try_from(at / 86_400).unwrap_or(0);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let second = at % 86_400;
    format!(
        "{} {day:02}, {:02}:{:02} UTC",
        MONTHS[usize::try_from(month - 1).unwrap_or(0)],
        second / 3_600,
        (second % 3_600) / 60
    )
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
