//! Composer history is distinct from conversation snapshots and pending work.
use crate::{App, Draft};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    text: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    removed: bool,
    project: std::path::PathBuf,
    session: String,
}

#[derive(Default)]
pub(crate) struct History {
    entries: Vec<Entry>,
    walk: Vec<String>,
    index: Option<usize>,
    stash: Draft,
    loaded: bool,
    session: Option<String>,
    last_recorded: Option<String>,
}

impl History {
    pub(crate) fn reset(&mut self) {
        self.index = None;
        self.walk.clear();
        self.stash = Draft::default();
    }
}

impl App {
    fn history_path(&self) -> Option<std::path::PathBuf> {
        self.account_dir
            .as_ref()
            .map(|root| root.join("prompt-history.jsonl"))
    }

    fn prompt_session(&mut self) -> String {
        if let Some(session) = self.session_id() {
            return session.to_owned();
        }
        self.composer_history
            .session
            .get_or_insert_with(|| {
                format!(
                    "{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                )
            })
            .clone()
    }

    pub(crate) fn record_prompt(&mut self) {
        if self.replaying_prompt {
            return;
        }
        self.load_prompt_history();
        self.composer_history.reset();
        if self.draft.text.trim().is_empty() {
            return;
        }
        let session = self.prompt_session();
        let entry = Entry {
            text: self.draft.text.clone(),
            id: format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            removed: false,
            project: self.cwd.clone().unwrap_or_default(),
            session,
        };
        if let Some(path) = self.history_path() {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = options.open(path) {
                let _ = file.lock();
                if let Ok(mut bytes) = serde_json::to_vec(&entry) {
                    bytes.push(b'\n');
                    let _ = file.write_all(&bytes);
                }
            }
        }
        self.composer_history.last_recorded = Some(entry.id.clone());
        self.composer_history.entries.push(entry);
    }

    pub(crate) fn restore_unanswered_prompt(&mut self) {
        if self.active_delegation.is_some()
            || !self.live.partial.is_empty()
            || !self.draft.text.is_empty()
        {
            return;
        }
        let Some(crate::live::Entry::User(text)) = self.live.entries.last() else {
            return;
        };
        let text = text.clone();
        let Some(id) = self.composer_history.last_recorded.take() else {
            return;
        };
        if !self
            .composer_history
            .entries
            .iter()
            .any(|e| e.id == id && e.text == text)
        {
            return;
        }
        self.live.entries.pop();
        self.draft = Draft {
            cursor: text.len(),
            text,
        };
        if let Some(entry) = self
            .composer_history
            .entries
            .iter()
            .find(|e| e.id == id)
            .cloned()
        {
            let tombstone = Entry {
                removed: true,
                ..entry
            };
            if let Some(path) = self.history_path() {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) {
                    let _ = file.lock();
                    if let Ok(mut bytes) = serde_json::to_vec(&tombstone) {
                        bytes.push(b'\n');
                        let _ = file.write_all(&bytes);
                    }
                }
            }
            self.composer_history.entries.push(tombstone);
        }
        self.composer_history.reset();
    }

    fn load_prompt_history(&mut self) {
        if !self.composer_history.loaded {
            if let Some(path) = self.history_path() {
                if let Ok(text) = std::fs::read_to_string(path) {
                    self.composer_history.entries = text
                        .lines()
                        .filter_map(|line| serde_json::from_str(line).ok())
                        .collect();
                }
            }
            self.composer_history.loaded = true;
        }
    }

    fn start_history(&mut self) {
        self.load_prompt_history();
        let session = self.prompt_session();
        let project = self.cwd.clone().unwrap_or_default();
        let h = &mut self.composer_history;
        let project = &project;
        let session = &session;
        let removed: std::collections::HashSet<_> = h
            .entries
            .iter()
            .filter(|e| e.removed)
            .map(|e| e.id.as_str())
            .collect();
        let window: Vec<_> = h
            .entries
            .iter()
            .rev()
            .filter(|e| !e.removed && !removed.contains(e.id.as_str()) && &e.project == project)
            .take(1000)
            .collect();
        h.walk = [true, false]
            .into_iter()
            .flat_map(|current| {
                window
                    .iter()
                    .filter(move |e| &e.project == project && (&e.session == session) == current)
                    .map(|e| e.text.clone())
            })
            .take(1000)
            .collect();
        h.stash = if self.draft.text.trim().is_empty() {
            Draft::default()
        } else {
            self.draft.clone()
        };
    }

    pub(crate) fn composer_arrow(&mut self, up: bool) {
        if self.footer_focused {
            let count = if self.mode == crate::Mode::Demo {
                crate::agents::DEMOS.len()
            } else {
                self.delegations.len()
            };
            if up {
                if let Some(next) = self.selected_agent.and_then(|i| i.checked_sub(1)) {
                    self.select_agent(Some(next));
                } else {
                    self.footer_focused = false;
                    self.select_agent(None);
                }
            } else if count > 0 {
                self.select_agent(Some(
                    self.selected_agent.map_or(0, |i| (i + 1).min(count - 1)),
                ));
            }
            return;
        }
        if self.draft.vertical(self.composer_width.max(1), up) {
            return;
        }
        if up {
            if self.restore_queued_prompts() {
                return;
            }
            if self.composer_history.index.is_none() {
                self.start_history();
            }
            let h = &mut self.composer_history;
            let next = h.index.map_or(0, |i| i + 1);
            if let Some(text) = h.walk.get(next) {
                self.draft = Draft {
                    text: text.clone(),
                    cursor: 0,
                };
                h.index = Some(next);
            }
        } else if let Some(index) = self.composer_history.index {
            let h = &mut self.composer_history;
            if index == 0 {
                self.draft = std::mem::take(&mut h.stash);
                self.draft.cursor = self.draft.text.len();
                h.reset();
            } else {
                let text = h.walk[index - 1].clone();
                self.draft = Draft {
                    cursor: text.len(),
                    text,
                };
                h.index = Some(index - 1);
            }
        } else if !self.delegations.is_empty() || self.mode == crate::Mode::Demo {
            self.footer_focused = true;
            self.select_agent(Some(0));
        }
    }
}

impl Draft {
    /// Resolve movement against exactly the same wrapped layout as the renderer.
    fn vertical(&mut self, width: u16, up: bool) -> bool {
        let (_, (column, row)) = self.wrapped(width);
        let target = if up {
            row.checked_sub(1)
        } else {
            row.checked_add(1)
        };
        let Some(target) = target else {
            return false;
        };
        let mut best = None;
        let original = self.cursor;
        use unicode_segmentation::UnicodeSegmentation;
        for offset in self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(self.text.len()))
        {
            self.cursor = offset;
            let (_, (col, r)) = self.wrapped(width);
            if r == target {
                let distance = column.abs_diff(col);
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((offset, distance));
                }
            }
        }
        self.cursor = best.map_or(original, |(offset, _)| offset);
        best.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::default();
        app.set_mode(crate::Mode::Live);
        app.composer_width = 80;
        app
    }
    fn record(app: &mut App, text: &str) {
        app.draft = Draft {
            text: text.into(),
            cursor: text.len(),
        };
        app.record_prompt();
        app.draft = Draft::default();
    }
    #[test]
    fn history_cursor_direction_and_draft_restoration() {
        let mut a = app();
        record(&mut a, "first");
        record(&mut a, "second");
        a.draft = Draft {
            text: "draft".into(),
            cursor: 2,
        };
        a.composer_arrow(true);
        assert_eq!((a.draft.text.as_str(), a.draft.cursor), ("second", 0));
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "first");
        a.composer_arrow(false);
        assert_eq!((a.draft.text.as_str(), a.draft.cursor), ("second", 6));
        a.composer_arrow(false);
        assert_eq!((a.draft.text.as_str(), a.draft.cursor), ("draft", 5));
        assert!(!a.footer_focused);
    }
    #[test]
    fn multiline_motion_wins_and_queue_cannot_execute_after_retrieval() {
        let mut a = app();
        a.submit("active", std::path::Path::new("."));
        a.submit("queued one", std::path::Path::new("."));
        a.submit("queued two", std::path::Path::new("."));
        let slots = a.prompt_inbox.lock().unwrap().clone();
        a.draft = Draft {
            text: "one\ntwo".into(),
            cursor: 6,
        };
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "one\ntwo");
        assert_eq!(a.queued_prompts.len(), 2);
        let cursor = a.draft.cursor;
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "queued one\nqueued two\none\ntwo");
        assert_eq!(a.draft.cursor, "queued one\nqueued two\n".len() + cursor);
        assert!(a.queued_prompts.is_empty());
        assert!(slots.iter().all(|s| s.lock().unwrap().is_none()));
        assert!(a.live.busy);
    }
    #[test]
    fn wraps_and_unicode_have_valid_cursor_boundaries() {
        let mut d = Draft {
            text: "日本語\n🦀abc".into(),
            cursor: 0,
        };
        assert!(d.vertical(4, false));
        assert!(d.text.is_char_boundary(d.cursor));
        assert!(d.vertical(4, true));
        assert!(!d.vertical(4, true));
    }
    #[test]
    fn whitespace_draft_is_not_restored() {
        let mut a = app();
        record(&mut a, "past");
        a.draft = Draft {
            text: "  ".into(),
            cursor: 0,
        };
        a.composer_arrow(true);
        a.composer_arrow(false);
        assert!(a.draft.text.is_empty());
    }
    #[test]
    fn current_session_history_precedes_newer_other_sessions() {
        let mut a = app();
        record(&mut a, "mine");
        a.composer_history.entries.push(Entry {
            text: "other".into(),
            project: Default::default(),
            session: "other-session".into(),
            id: "other".into(),
            removed: false,
        });
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "mine");
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "other");
    }
    #[test]
    fn persisted_history_and_interrupt_tombstone_survive_restart() {
        let root = tempfile::tempdir().unwrap();
        let mut a = app();
        a.account_dir = Some(root.path().to_owned());
        a.submit("restore me", std::path::Path::new("."));
        a.restore_unanswered_prompt();
        a.restore_unanswered_prompt();
        assert_eq!(a.draft.text, "restore me");
        assert!(a.live.entries.is_empty());
        let mut b = app();
        b.account_dir = Some(root.path().to_owned());
        b.cwd = Some(".".into());
        b.composer_arrow(true);
        assert!(b.draft.text.is_empty());
        record(&mut b, "retained");
        let mut c = app();
        c.account_dir = Some(root.path().to_owned());
        c.cwd = Some(".".into());
        c.composer_arrow(true);
        assert_eq!(c.draft.text, "retained");
    }
    #[test]
    fn double_escape_saves_and_clears_draft() {
        let mut a = app();
        a.draft = Draft {
            text: "unsent".into(),
            cursor: 6,
        };
        let escape = || {
            crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ))
        };
        a.handle(escape());
        assert_eq!(a.draft.text, "unsent");
        a.handle(escape());
        assert!(a.draft.text.is_empty());
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "unsent");
    }
    #[test]
    fn multiple_suggestions_own_arrows_and_queue_wins_over_history() {
        let mut a = app();
        record(&mut a, "history");
        a.submit("active", std::path::Path::new("."));
        a.submit("pending", std::path::Path::new("."));
        a.draft = Draft {
            text: "/".into(),
            cursor: 1,
        };
        let key = |code| {
            crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                code,
                crossterm::event::KeyModifiers::NONE,
            ))
        };
        a.handle(key(crossterm::event::KeyCode::Up));
        assert_eq!(a.draft.text, "/");
        assert_eq!(a.queued_prompts.len(), 1);
        a.draft = Draft::default();
        a.handle(key(crossterm::event::KeyCode::Up));
        assert_eq!(a.draft.text, "pending");
        a.handle(key(crossterm::event::KeyCode::Down));
        assert!(a.queued_prompts.is_empty());
    }
    #[test]
    fn restoration_and_footer_entry_are_separate_presses() {
        let mut a = App::default();
        a.mode = crate::Mode::Demo;
        a.composer_width = 80;
        record(&mut a, "past");
        a.draft = Draft {
            text: "draft".into(),
            cursor: 0,
        };
        a.composer_arrow(true);
        a.composer_arrow(false);
        assert_eq!(a.draft.text, "draft");
        assert!(!a.footer_focused);
        a.composer_arrow(false);
        assert!(a.footer_focused);
        a.composer_arrow(true);
        assert!(!a.footer_focused);
        assert_eq!(a.draft.text, "draft");
    }
}
