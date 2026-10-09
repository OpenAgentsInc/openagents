//! Composer history is distinct from conversation snapshots and pending work.
use crate::{App, Draft};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    text: String,
    #[serde(default)]
    composer: crate::composer_state::ComposerState,
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
    walk: Vec<Entry>,
    stash_composer: crate::composer_state::ComposerState,
    bash_only: bool,
    recall_up: bool,
    applied: Option<(usize, String, bool)>,
    index: Option<usize>,
    stash: Draft,
    load: Option<std::sync::Arc<DiskLoad>>,
    navigating: bool,
    session: Option<String>,
    last_recorded: Option<String>,
}

impl History {
    pub(crate) fn reset(&mut self) {
        self.applied = None;
        self.index = None;
        self.navigating = false;
        self.load = None;
        self.walk.clear();
        self.stash = Draft::default();
        self.stash_composer = Default::default();
    }
}

#[derive(Default)]
struct DiskLoad {
    pages: std::sync::Mutex<Vec<Vec<Entry>>>,
    done: std::sync::atomic::AtomicBool,
}

type LoadKey = (std::path::PathBuf, std::path::PathBuf, String, bool);
fn shared_load(
    path: std::path::PathBuf,
    project: std::path::PathBuf,
    session: String,
    bash: bool,
) -> std::sync::Arc<DiskLoad> {
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    static LOADS: OnceLock<Mutex<std::collections::HashMap<LoadKey, Weak<DiskLoad>>>> =
        OnceLock::new();
    let mut loads = LOADS.get_or_init(Default::default).lock().unwrap();
    let key = (path.clone(), project.clone(), session.clone(), bash);
    if let Some(load) = loads.get(&key).and_then(Weak::upgrade) {
        return load;
    }
    loads.retain(|_, load| load.strong_count() > 0);
    let load = Arc::new(DiskLoad::default());
    loads.insert(key, Arc::downgrade(&load));
    let worker = load.clone();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut entries = std::collections::VecDeque::new();
        let mut removed = std::collections::HashSet::new();
        if let Ok(file) = std::fs::File::open(path) {
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(mut entry) = serde_json::from_str::<Entry>(&line) {
                    if let Some(text) = entry.text.strip_prefix('!') {
                        entry.text = text.to_owned();
                        entry.composer.mode = crate::composer_state::InputMode::Bash;
                    }
                    if entry.project != project {
                        continue;
                    }
                    if entry.removed {
                        removed.insert(entry.id.clone());
                    }
                    entries.push_back(entry);
                    if entries.len() > 2000 {
                        entries.pop_front();
                    }
                }
            }
        }
        let window = entries
            .into_iter()
            .rev()
            .filter(|e| !e.removed && !removed.contains(&e.id))
            .take(1000)
            .collect::<Vec<_>>();
        let ordered = [true, false]
            .into_iter()
            .flat_map(|current| {
                window
                    .iter()
                    .filter({
                        let session = &session;
                        move |e| (e.session == *session) == current
                    })
                    .filter(|e| !bash || e.composer.mode == crate::composer_state::InputMode::Bash)
                    .cloned()
            })
            .collect::<Vec<_>>();
        for page in ordered.chunks(10) {
            worker.pages.lock().unwrap().push(page.to_vec());
        }
        worker
            .done
            .store(true, std::sync::atomic::Ordering::Release);
    });
    load
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
        self.composer_history.reset();
        if self.draft.text.trim().is_empty() && self.composer.images.is_empty() {
            return;
        }
        let session = self.prompt_session();
        let entry = Entry {
            text: self.draft.text.clone(),
            composer: self.composer.clone(),
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
        let mut text = text.clone();
        let Some(id) = self.composer_history.last_recorded.take() else {
            return;
        };
        if !self
            .composer_history
            .entries
            .iter()
            .any(|e| e.id == id && (e.text == text || format!("!{}", e.text) == text))
        {
            return;
        }
        let restored = self
            .composer_history
            .entries
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .composer
            .clone();
        if restored.mode == crate::composer_state::InputMode::Bash {
            text = text.strip_prefix('!').unwrap_or(&text).to_owned();
        }
        self.composer = restored;
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

    pub(crate) fn poll_prompt_history(&mut self) {
        if !self.composer_history.navigating {
            return;
        }
        let Some(load) = self.composer_history.load.clone() else {
            return;
        };
        let disk = load
            .pages
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        let session = self.prompt_session();
        let project = self.cwd.clone().unwrap_or_default();
        let h = &mut self.composer_history;
        let removed = h
            .entries
            .iter()
            .filter(|e| e.removed)
            .map(|e| e.id.clone())
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        let mut entries = h
            .entries
            .iter()
            .rev()
            .chain(disk.iter())
            .filter(|e| {
                !e.removed
                    && !removed.contains(&e.id)
                    && e.project == project
                    && (!h.bash_only || e.composer.mode == crate::composer_state::InputMode::Bash)
                    && seen.insert(e.id.clone())
            })
            .cloned()
            .collect::<Vec<_>>();
        entries.truncate(1000);
        entries.sort_by_key(|e| e.session != session);
        h.walk = entries;
        if let Some(mut index) = h.index {
            if load.done.load(std::sync::atomic::Ordering::Acquire) {
                if h.walk.is_empty() {
                    h.index = None;
                    return;
                }
                index = index.min(h.walk.len() - 1);
                h.index = Some(index);
            }
            if let Some(entry) = h.walk.get(index) {
                let applied = (index, entry.id.clone(), h.recall_up);
                if h.applied.as_ref() == Some(&applied) {
                    return;
                }
                h.applied = Some(applied);
                self.composer = entry.composer.clone();
                self.draft = Draft {
                    text: entry.text.clone(),
                    cursor: if h.recall_up { 0 } else { entry.text.len() },
                };
            }
        }
    }

    fn start_history(&mut self) {
        let session = self.prompt_session();
        let project = self.cwd.clone().unwrap_or_default();
        let h = &mut self.composer_history;
        h.bash_only = self.composer.mode == crate::composer_state::InputMode::Bash;
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
                    .filter(|e| {
                        !h.bash_only || e.composer.mode == crate::composer_state::InputMode::Bash
                    })
                    .map(|e| (*e).clone())
            })
            .take(1000)
            .collect();
        h.navigating = true;
        h.load = self.account_dir.as_ref().map(|root| {
            shared_load(
                root.join("prompt-history.jsonl"),
                project.clone(),
                session.clone(),
                h.bash_only,
            )
        });
        h.stash_composer = self.composer.clone();
        h.stash = if self.draft.text.trim().is_empty() && self.composer.images.is_empty() {
            Draft::default()
        } else {
            self.draft.clone()
        };
    }

    pub(crate) fn composer_arrow(&mut self, up: bool) {
        // An open agent conversation is a selected footer row, so the footer
        // owns Up/Down there: Down opens the next agent and Up the previous
        // one, back to the main conversation.
        if self.footer_focused || self.selected_agent.is_some() {
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
        // Before the first frame the composer width is unknown (0): move by
        // logical lines only rather than wrapping at one column.
        let width = match self.composer_width {
            0 => u16::MAX,
            width => width,
        };
        if self.draft.vertical(width, up) {
            return;
        }
        if up {
            if self.restore_queued_prompts() {
                return;
            }
            if !self.composer_history.navigating {
                self.start_history();
            }
            let h = &mut self.composer_history;
            let next = h.index.map_or(0, |i| i + 1);
            h.recall_up = true;
            if h.load.is_some() {
                h.index = Some(next);
            }
            if let Some(entry) = h.walk.get(next) {
                let text = &entry.text;
                self.composer = entry.composer.clone();
                self.draft = Draft {
                    text: text.clone(),
                    cursor: 0,
                };
                h.index = Some(next);
                h.applied = Some((next, entry.id.clone(), true));
            }
            self.poll_prompt_history();
        } else if let Some(index) = self.composer_history.index {
            let h = &mut self.composer_history;
            if index == 0 {
                self.composer = std::mem::take(&mut h.stash_composer);
                self.draft = std::mem::take(&mut h.stash);
                self.draft.cursor = self.draft.text.len();
                h.reset();
            } else {
                h.recall_up = false;
                h.index = Some(index - 1);
                let Some(entry) = h.walk.get(index - 1).cloned() else {
                    return;
                };
                self.composer = entry.composer;
                let text = entry.text;
                self.draft = Draft {
                    cursor: text.len(),
                    text,
                };
                h.index = Some(index - 1);
                h.applied = Some((index - 1, entry.id, false));
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
    fn wait_history(app: &mut App) {
        for _ in 0..1000 {
            app.poll_prompt_history();
            if app
                .composer_history
                .load
                .as_ref()
                .is_none_or(|l| l.done.load(std::sync::atomic::Ordering::Acquire))
            {
                app.poll_prompt_history();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("history load timed out");
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
            composer: Default::default(),
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
        wait_history(&mut c);
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
    #[test]
    fn bash_filter_and_rich_draft_restore() {
        use crate::composer_state::InputMode;
        let mut a = app();
        record(&mut a, "normal");
        a.composer.mode = InputMode::Bash;
        record(&mut a, "pwd");
        a.composer = Default::default();
        record(&mut a, "new normal");
        a.composer.mode = InputMode::Bash;
        a.composer.pasted.push("paste".into());
        a.attach_image("data:image/png;base64,AA".into(), Some("existing".into()));
        a.draft.text = "draft".into();
        let saved = a.composer.clone();
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "pwd");
        assert_eq!(a.composer.mode, InputMode::Bash);
        a.composer_arrow(true);
        assert_eq!(a.draft.text, "pwd");
        a.composer_arrow(false);
        assert_eq!(a.draft.text, "draft");
        assert_eq!(a.composer, saved);
        assert_eq!(a.draft.cursor, 5);
    }

    #[test]
    fn asynchronous_pages_share_load_and_preserve_rapid_intent() {
        let root = tempfile::tempdir().unwrap();
        let mut writer = app();
        writer.account_dir = Some(root.path().to_owned());
        for i in 0..25 {
            record(&mut writer, &format!("entry {i}"));
        }
        let mut reader = app();
        reader.account_dir = writer.account_dir.clone();
        reader.draft.text = "live draft".into();
        reader.draft.cursor = 0;
        reader.start_history();
        let load = reader.composer_history.load.clone().unwrap();
        let shared = shared_load(
            root.path().join("prompt-history.jsonl"),
            Default::default(),
            reader.prompt_session(),
            false,
        );
        assert!(std::sync::Arc::ptr_eq(&load, &shared));
        // Issue all arrow intents while the renderer has not polled the load.
        for _ in 0..12 {
            reader.composer_arrow(true);
        }
        wait_history(&mut reader);
        assert_eq!(reader.draft.text, "entry 13");
        assert!(load.pages.lock().unwrap().iter().all(|p| p.len() <= 10));
        reader.draft.cursor = 3;
        reader.poll_prompt_history();
        assert_eq!(reader.draft.cursor, 3);
        reader.draft.cursor = reader.draft.text.len();
        for _ in 0..12 {
            reader.composer_arrow(false);
        }
        assert_eq!(reader.draft.text, "live draft");
        reader.poll_prompt_history();
        assert_eq!(reader.draft.text, "live draft");
    }
    #[test]
    fn returning_to_draft_cancels_pending_disk_recall() {
        let mut a = app();
        a.draft.text = "live".into();
        a.start_history();
        let pending = std::sync::Arc::new(DiskLoad::default());
        a.composer_history.load = Some(pending.clone());
        a.composer_arrow(true);
        assert_eq!(a.composer_history.index, Some(0));
        a.composer_arrow(false);
        assert_eq!(a.draft.text, "live");
        pending
            .done
            .store(true, std::sync::atomic::Ordering::Release);
        a.poll_prompt_history();
        assert_eq!(a.draft.text, "live");
        assert!(!a.composer_history.navigating);
    }
}
