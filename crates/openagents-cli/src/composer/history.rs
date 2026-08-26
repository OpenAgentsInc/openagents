//! What you typed last time, and the time before that.
//!
//! The composer starts empty every turn, which means a prompt you want to
//! adjust by one word has to be typed again from the beginning. This is the
//! ring that Up and Down walk, and the file it survives a restart in.
//!
//! Two rules keep the walk predictable.
//!
//! The **draft is not lost**. Walking back from a half-typed line and then
//! forward again returns that line, rather than an empty composer. The draft
//! is captured on the first Up of a run and restored when Down walks past the
//! newest entry.
//!
//! **A repeat is not recorded twice in a row.** Sending the same prompt three
//! times leaves one entry, so the walk is over distinct prompts.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// How many prompts are kept, in memory and on disk.
pub const CAPACITY: usize = 500;

/// A prompt that spans lines is stored on one line, with its newlines escaped,
/// so the file stays one-entry-per-line and can be read by anything.
const ESCAPED_NEWLINE: &str = "\\n";

#[derive(Debug, Default)]
pub struct History {
    /// Oldest first.
    entries: Vec<String>,
    /// How far back the reader has walked. `None` means "at the draft".
    walk: Option<usize>,
    /// What was in the composer when the walk started.
    draft: Option<String>,
    path: Option<PathBuf>,
}

impl History {
    /// An in-memory history that outlives nothing. What the tests use, and
    /// what a session with no writable home falls back to.
    pub fn new() -> Self {
        Self::default()
    }

    /// The file prompts are kept in between sessions.
    pub fn default_path() -> PathBuf {
        crate::auth::config_directory().join("coder-history")
    }

    /// Read `path`, keeping the last [`CAPACITY`] entries.
    ///
    /// A missing or unreadable file is an empty history, not an error: a
    /// session must open whether or not the reader has one.
    pub fn load(path: PathBuf) -> Self {
        let mut entries = std::fs::read_to_string(&path)
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(unescape)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        if entries.len() > CAPACITY {
            entries.drain(..entries.len() - CAPACITY);
        }
        Self {
            entries,
            walk: None,
            draft: None,
            path: Some(path),
        }
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Remember a prompt that was sent, and append it to the file.
    pub fn record(&mut self, prompt: &str) {
        self.walk = None;
        self.draft = None;
        let prompt = prompt.trim_end();
        if prompt.is_empty() {
            return;
        }
        if self.entries.last().map(String::as_str) == Some(prompt) {
            return;
        }
        self.entries.push(prompt.to_string());
        if self.entries.len() > CAPACITY {
            let excess = self.entries.len() - CAPACITY;
            self.entries.drain(..excess);
        }
        if let Some(path) = &self.path {
            append(path, prompt);
        }
    }

    /// The previous prompt, given what is in the composer now.
    ///
    /// `None` when there is nothing further back, which is what lets the
    /// caller decide what Up means at the end of the ring.
    pub fn previous(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let next = match self.walk {
            None => {
                self.draft = Some(current.to_string());
                self.entries.len() - 1
            }
            Some(0) => return None,
            Some(index) => index - 1,
        };
        self.walk = Some(next);
        self.entries.get(next).cloned()
    }

    /// The next prompt forward, or the draft once the walk runs out.
    ///
    /// Named `forward` rather than `next` so it cannot be mistaken for an
    /// iterator's: this walks a cursor the caller does not own.
    pub fn forward(&mut self) -> Option<String> {
        let index = self.walk?;
        if index + 1 < self.entries.len() {
            self.walk = Some(index + 1);
            return self.entries.get(index + 1).cloned();
        }
        self.walk = None;
        Some(self.draft.take().unwrap_or_default())
    }

    /// Abandon the walk. Called when the composer is edited, so the next Up
    /// starts again from what is now in it.
    pub fn stop_walking(&mut self) {
        self.walk = None;
        self.draft = None;
    }

    pub fn walking(&self) -> bool {
        self.walk.is_some()
    }
}

fn append(path: &Path, prompt: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // A history that cannot be written is not worth interrupting a session
    // for. The prompt still went out; only its memory was lost.
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", escape(prompt));
    }
}

fn escape(prompt: &str) -> String {
    prompt.replace('\\', "\\\\").replace('\n', ESCAPED_NEWLINE)
}

fn unescape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(prompts: &[&str]) -> History {
        let mut history = History::new();
        for prompt in prompts {
            history.record(prompt);
        }
        history
    }

    #[test]
    fn up_walks_back_through_what_was_sent() {
        let mut history = with(&["first", "second", "third"]);
        assert_eq!(history.previous("").as_deref(), Some("third"));
        assert_eq!(history.previous("").as_deref(), Some("second"));
        assert_eq!(history.previous("").as_deref(), Some("first"));
        assert_eq!(history.previous(""), None, "the walk ran off the end");
    }

    #[test]
    fn down_walks_forward_and_gives_the_draft_back() {
        let mut history = with(&["first", "second"]);
        assert_eq!(history.previous("half typed").as_deref(), Some("second"));
        assert_eq!(history.previous("").as_deref(), Some("first"));
        assert_eq!(history.forward().as_deref(), Some("second"));
        assert_eq!(
            history.forward().as_deref(),
            Some("half typed"),
            "walking forward past the newest entry lost the draft"
        );
        assert!(!history.walking());
    }

    #[test]
    fn down_before_any_walk_does_nothing() {
        let mut history = with(&["only"]);
        assert_eq!(history.forward(), None);
    }

    #[test]
    fn an_empty_history_has_nothing_to_walk() {
        let mut history = History::new();
        assert_eq!(history.previous("draft"), None);
        assert!(!history.walking());
    }

    #[test]
    fn the_same_prompt_twice_running_is_recorded_once() {
        let history = with(&["same", "same", "other", "same"]);
        assert_eq!(history.entries(), ["same", "other", "same"]);
    }

    #[test]
    fn an_empty_prompt_is_not_recorded() {
        let history = with(&["", "   ", "real"]);
        assert_eq!(history.entries(), ["real"]);
    }

    #[test]
    fn editing_after_a_walk_starts_the_next_walk_from_the_new_text() {
        let mut history = with(&["one", "two"]);
        assert_eq!(history.previous("").as_deref(), Some("two"));
        history.stop_walking();
        assert_eq!(history.previous("edited").as_deref(), Some("two"));
        assert_eq!(history.previous("").as_deref(), Some("one"));
        assert_eq!(history.forward().as_deref(), Some("two"));
        assert_eq!(history.forward().as_deref(), Some("edited"));
    }

    #[test]
    fn a_history_survives_a_restart() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("nested").join("coder-history");

        let mut first = History::load(path.clone());
        first.record("what changed today");
        first.record("run the tests");

        let mut second = History::load(path);
        assert_eq!(second.entries(), ["what changed today", "run the tests"]);
        assert_eq!(second.previous("").as_deref(), Some("run the tests"));
    }

    #[test]
    fn a_multi_line_prompt_comes_back_with_its_lines() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("coder-history");
        History::load(path.clone()).record("first line\nsecond line");
        assert_eq!(
            History::load(path).entries(),
            ["first line\nsecond line".to_string()]
        );
    }

    #[test]
    fn a_backslash_in_a_prompt_is_not_read_back_as_a_newline() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("coder-history");
        History::load(path.clone()).record("grep '\\n' src");
        assert_eq!(
            History::load(path).entries(),
            ["grep '\\n' src".to_string()]
        );
    }

    #[test]
    fn a_missing_file_is_an_empty_history_rather_than_a_failure() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let history = History::load(dir.path().join("was-never-written"));
        assert!(history.entries().is_empty());
    }

    #[test]
    fn only_the_last_entries_are_kept() {
        let mut history = History::new();
        for n in 0..CAPACITY + 20 {
            history.record(&format!("prompt {n}"));
        }
        assert_eq!(history.entries().len(), CAPACITY);
        assert_eq!(history.entries()[0], format!("prompt {}", 20));
    }
}
