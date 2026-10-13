//! `/rewind` (#11515): undo a turn's file changes and return to its prompt.
//!
//! As each turn begins, the checkout's changed files are read
//! ([`crate::shell_changes::Snapshot`]); as it ends, the files the turn
//! changed are kept with what they held before it
//! ([`crate::shell_changes::TurnRecord`]), beside the saved chat. Choosing a
//! message in `/rewind` puts back the files its turn and every later turn
//! changed, cuts the chat to just before the message, and leaves the
//! message in the input to edit and send again. A file changed outside the
//! chat since is never overwritten.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::shell_changes::{Restored, Snapshot, TurnRecord};
use crate::{App, Draft, Screen, live};

/// How many of the latest messages the picker lists, and how many turns'
/// files are kept.
pub(crate) const MAX_TURNS: usize = 20;

/// The most earlier file contents kept across a chat's turns.
const MAX_KEPT_BYTES: usize = 16 << 20;

/// The ending of the file beside a saved chat that keeps its turns' files.
const SAVED: &str = "rewind.json";

/// The checkout as a turn began, read off the input's thread.
pub(crate) struct Start {
    message: usize,
    snapshot: std::thread::JoinHandle<Option<Snapshot>>,
}

/// One message the picker offers.
#[derive(Debug)]
pub struct Choice {
    /// Which of the person's messages, counting from zero.
    pub message: usize,
    pub prompt: String,
    /// The files this message's turn changed.
    pub files: Vec<String>,
    /// Files this turn or a later one changed that cannot be put back.
    pub unrestorable: Vec<String>,
    /// Whether every file this turn and the later ones changed was seen.
    pub complete: bool,
}

impl Choice {
    /// Whether rewinding here leaves some files as they are now.
    #[must_use]
    pub fn partly(&self) -> bool {
        !self.unrestorable.is_empty() || !self.complete
    }
}

/// The latest messages, newest first.
pub struct Picker {
    pub choices: Vec<Choice>,
    pub selected: usize,
    pub page: usize,
}

pub enum Action {
    Continue,
    Close,
    Select(usize),
}

impl Picker {
    pub fn handle(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return Action::Continue;
        }
        let last = self.choices.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => return Action::Close,
            KeyCode::Enter if key.kind == KeyEventKind::Press => {
                if let Some(choice) = self.choices.get(self.selected) {
                    return Action::Select(choice.message);
                }
            }
            KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                self.selected = self.selected.saturating_add(1).min(last);
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.page.max(1)),
            KeyCode::PageDown => {
                self.selected = self.selected.saturating_add(self.page.max(1)).min(last);
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            _ => {}
        }
        Action::Continue
    }
}

/// Where the turns' files of the chat saved at `session` are kept: a file
/// of their own beside it, so the chat itself, which can be sent to the
/// account or published, never carries file contents.
fn beside(session: &Path) -> PathBuf {
    session.with_extension(SAVED)
}

/// Keeps `turns` beside the chat saved at `session`, readable by this
/// user only; none removes the file.
pub(crate) fn write(session: &Path, turns: &[TurnRecord]) {
    let path = beside(session);
    if turns.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let Ok(bytes) = serde_json::to_vec(turns) else {
        return;
    };
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(&path) {
        use std::io::Write;
        let _ = file.write_all(&bytes);
    }
}

/// The turns' files kept beside the chat saved at `session`, for the
/// messages `chat` still holds.
pub(crate) fn read(session: &Path, chat: &live::Chat) -> Vec<TurnRecord> {
    let users = chat.user_count();
    std::fs::read(beside(session))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<TurnRecord>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|turn| turn.message < users)
        .collect()
}

fn names(paths: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut text = paths
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > SHOWN {
        text.push_str(&format!(" and {} more", paths.len() - SHOWN));
    }
    text
}

fn files(count: usize) -> String {
    if count == 1 {
        "1 file".into()
    } else {
        format!("{count} files")
    }
}

/// What a rewind did, in plain words.
fn summary(done: &Restored) -> String {
    let mut parts = Vec::new();
    if !done.restored.is_empty() {
        parts.push(format!("Restored {}.", files(done.restored.len())));
    }
    if !done.skipped.is_empty() {
        parts.push(format!(
            "Left {} as is: changed after that turn.",
            names(&done.skipped)
        ));
    }
    if !done.unrestorable.is_empty() {
        parts.push(format!(
            "Could not restore {} (too large, binary, or unreadable).",
            names(&done.unrestorable)
        ));
    }
    if !done.failed.is_empty() {
        parts.push(format!("Could not write {}.", names(&done.failed)));
    }
    if parts.is_empty() {
        parts.push("No files to restore.".into());
    }
    parts.push("Your message is back in the input to edit and send again.".into());
    parts.join(" ")
}

impl App {
    fn rewind_blocked(&self) -> bool {
        self.live.busy
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
            || self.request.is_some()
            || self.delegations.iter().any(|child| child.running)
    }

    /// A turn of the main chat begins on its newest message: read the
    /// checkout as it is now, off the input's thread.
    pub(crate) fn begin_turn_record(&mut self) {
        if !self.rewind_records {
            return;
        }
        self.finish_turn_record();
        let Some(message) = self.live.user_count().checked_sub(1) else {
            return;
        };
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        self.live.turn_start = Some(Start {
            message,
            snapshot: std::thread::spawn(move || Snapshot::take(&cwd)),
        });
    }

    /// The main chat's turn ended: keep the files it changed.
    pub(crate) fn finish_turn_record(&mut self) {
        let Some(start) = self.live.turn_start.take() else {
            return;
        };
        let Ok(Some(snapshot)) = start.snapshot.join() else {
            return;
        };
        self.keep_turn(TurnRecord::finish(&snapshot, start.message));
    }

    pub(crate) fn keep_turn(&mut self, record: TurnRecord) {
        if record.files.is_empty() && record.complete {
            return;
        }
        let kept = |turns: &[TurnRecord]| turns.iter().map(TurnRecord::kept_bytes).sum::<usize>();
        let turns = &mut self.live.turns;
        turns.retain(|turn| turn.message != record.message);
        turns.push(record);
        while turns.len() > MAX_TURNS || (turns.len() > 1 && kept(turns) > MAX_KEPT_BYTES) {
            turns.remove(0);
        }
        self.live.turns_changed = true;
        self.history.dirty = true;
    }

    /// The picker's messages, newest first.
    pub(crate) fn rewind_choices(&self) -> Vec<Choice> {
        let prompts: Vec<&String> = self
            .live
            .entries
            .iter()
            .filter_map(|entry| match entry {
                live::Entry::User(text) => Some(text),
                _ => None,
            })
            .collect();
        (0..prompts.len())
            .rev()
            .take(MAX_TURNS)
            .map(|message| {
                let undone = || {
                    self.live
                        .turns
                        .iter()
                        .filter(move |turn| turn.message >= message)
                };
                let mut unrestorable: Vec<String> = Vec::new();
                for path in undone().flat_map(TurnRecord::unrestorable) {
                    if !unrestorable.iter().any(|known| known == path) {
                        unrestorable.push(path.to_owned());
                    }
                }
                Choice {
                    message,
                    prompt: prompts[message].clone(),
                    files: self
                        .live
                        .turns
                        .iter()
                        .filter(|turn| turn.message == message)
                        .flat_map(|turn| turn.files.iter().map(|file| file.path.clone()))
                        .collect(),
                    unrestorable,
                    complete: undone().all(|turn| turn.complete),
                }
            })
            .collect()
    }

    /// `/rewind`: open the picker of the latest messages.
    pub fn rewind(&mut self) -> bool {
        if self.rewind_blocked() {
            self.notice = Some("Stop the current work with Esc before rewinding.".into());
            return false;
        }
        let choices = self.rewind_choices();
        if choices.is_empty() {
            self.notice = Some("Nothing to rewind yet.".into());
            return false;
        }
        self.rewind_picker = Some(Picker {
            choices,
            selected: 0,
            page: 10,
        });
        self.model_picker = None;
        self.screen = Screen::Conversation;
        self.notice = None;
        true
    }

    /// Undo the turn of the person's `message`th message and every later
    /// one: put their files back, cut the chat to just before the message,
    /// and leave the message in the input.
    pub fn rewind_to(&mut self, message: usize) -> bool {
        self.rewind_picker = None;
        if self.rewind_blocked() {
            self.notice = Some("Stop the current work with Esc before rewinding.".into());
            return false;
        }
        let Some((entry, prompt)) = self
            .live
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match entry {
                live::Entry::User(text) => Some((index, text.clone())),
                _ => None,
            })
            .nth(message)
        else {
            self.notice = Some("That message is no longer in this chat.".into());
            return false;
        };
        let mut undone: Vec<&TurnRecord> = self
            .live
            .turns
            .iter()
            .filter(|turn| turn.message >= message)
            .collect();
        undone.sort_by(|a, b| b.message.cmp(&a.message));
        let mut done = Restored::default();
        for turn in undone {
            done.undo(turn);
        }
        done.settle();
        self.select_agent(None);
        self.live.turns.retain(|turn| turn.message < message);
        self.live.turns_changed = true;
        self.live.entries.truncate(entry);
        self.live.partial.clear();
        self.live.partial_model = None;
        self.live.notice = None;
        self.live.stamp();
        self.live.cache = Default::default();
        // The Claude Code session read the messages cut here: the next
        // turn starts a new one from the chat as it is now.
        let users = self.live.user_count();
        if self
            .live
            .claude_session
            .as_ref()
            .is_some_and(|session| session.users > users)
        {
            self.live.claude_session = None;
        }
        self.live.work_started = self.live.work_started.min(users);
        self.draft = Draft {
            cursor: prompt.len(),
            text: prompt,
            ..Default::default()
        };
        self.composer = Default::default();
        self.composer_history.reset();
        self.screen = Screen::Conversation;
        self.history.dirty = true;
        self.scroll = u16::MAX;
        self.notice = Some(summary(&done));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_changes::{Content, FileRecord, Mark};

    #[test]
    fn the_summary_says_what_happened_in_plain_words() {
        let done = Restored {
            restored: vec!["a.rs".into(), "b.rs".into(), "c.rs".into()],
            skipped: vec!["notes.md".into()],
            unrestorable: vec!["logo.png".into()],
            failed: Vec::new(),
        };
        let text = summary(&done);
        assert!(
            text.starts_with("Restored 3 files. Left notes.md as is"),
            "{text}"
        );
        assert!(text.contains("Could not restore logo.png"), "{text}");
        assert_eq!(
            summary(&Restored::default()),
            "No files to restore. Your message is back in the input to edit and send again."
        );
    }

    #[test]
    fn a_turn_with_a_binary_file_is_offered_as_partly_restorable() {
        let mut app = App::default();
        for text in ["first", "second"] {
            app.live.entries.push(live::Entry::User(text.into()));
            app.live.entries.push(live::Entry::Assistant {
                text: "done".into(),
                model: None,
                elapsed_ms: None,
            });
        }
        app.keep_turn(TurnRecord {
            message: 1,
            root: "/nowhere".into(),
            files: vec![
                FileRecord {
                    path: "logo.png".into(),
                    before: Content::Absent,
                    after: Mark::Opaque,
                },
                FileRecord {
                    path: "a.rs".into(),
                    before: Content::Text("one\n".into()),
                    after: Mark::Absent,
                },
            ],
            complete: true,
        });
        let choices = app.rewind_choices();
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].prompt, "second");
        assert_eq!(choices[0].files, ["logo.png", "a.rs"]);
        assert!(choices[0].partly());
        assert_eq!(choices[0].unrestorable, ["logo.png"]);
        // Rewinding to the first message undoes the second turn too.
        assert_eq!(choices[1].prompt, "first");
        assert!(choices[1].files.is_empty());
        assert!(choices[1].partly());
    }

    /// After a rewind the chat ends just before the chosen message, the
    /// message is in the input, and the files are back.
    #[test]
    fn rewinding_cuts_the_chat_and_puts_the_prompt_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join("a.rs"), "turn two\n").unwrap();
        std::fs::write(root.join("new.rs"), "made by turn two\n").unwrap();
        let mut app = App::default();
        for text in ["first", "second", "third"] {
            app.live.entries.push(live::Entry::User(text.into()));
            app.live.entries.push(live::Entry::Assistant {
                text: "done".into(),
                model: None,
                elapsed_ms: None,
            });
        }
        app.live.stamp();
        app.live.claude_session = Some(live::ClaudeSession {
            id: "s".into(),
            users: 3,
        });
        let mark = |text: &str| Mark::of(&Content::Text(text.into()));
        app.keep_turn(TurnRecord {
            message: 0,
            root: root.clone(),
            files: vec![FileRecord {
                path: "kept.rs".into(),
                before: Content::Absent,
                after: Mark::Absent,
            }],
            complete: true,
        });
        app.keep_turn(TurnRecord {
            message: 1,
            root: root.clone(),
            files: vec![
                FileRecord {
                    path: "a.rs".into(),
                    before: Content::Text("turn one\n".into()),
                    after: mark("turn two\n"),
                },
                FileRecord {
                    path: "new.rs".into(),
                    before: Content::Absent,
                    after: mark("made by turn two\n"),
                },
            ],
            complete: true,
        });
        assert!(app.rewind());
        assert_eq!(app.rewind_picker.as_ref().unwrap().choices.len(), 3);
        assert!(app.rewind_to(1));
        assert!(app.rewind_picker.is_none());
        assert_eq!(app.live.entries.len(), 2);
        assert_eq!(app.live.stamps.len(), 2);
        assert!(matches!(&app.live.entries[0], live::Entry::User(t) if t == "first"));
        assert_eq!(app.draft.text, "second");
        assert_eq!(app.draft.cursor, "second".len());
        assert_eq!(app.live.claude_session, None);
        assert_eq!(app.live.turns.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("a.rs")).unwrap(),
            "turn one\n"
        );
        assert!(!root.join("new.rs").exists());
        let notice = app.notice.clone().unwrap();
        assert!(notice.starts_with("Restored 2 files."), "{notice}");
        assert!(oa_copy::violations(&notice, &[]).is_empty(), "{notice}");
    }

    /// The turns' files are kept beside the saved chat, never in it, and
    /// come back for the messages the chat still holds.
    #[test]
    fn kept_turns_survive_a_save_beside_the_chat() {
        let dir = tempfile::tempdir().unwrap();
        let session = dir.path().join("20261013T000000Z-abc.atif.json");
        let turn = |message| TurnRecord {
            message,
            root: "/repo".into(),
            files: vec![FileRecord {
                path: "a.rs".into(),
                before: Content::Text("one\n".into()),
                after: Mark::of(&Content::Text("two\n".into())),
            }],
            complete: true,
        };
        let kept = dir.path().join("20261013T000000Z-abc.atif.rewind.json");
        write(&session, &[turn(0), turn(3)]);
        assert!(kept.exists());
        let mut chat = live::Chat::default();
        chat.entries.push(live::Entry::User("first".into()));
        // A message the saved chat no longer holds is dropped.
        assert_eq!(read(&session, &chat), [turn(0)]);
        write(&session, &[]);
        assert!(read(&session, &chat).is_empty());
        assert!(!kept.exists());
    }

    #[test]
    fn rewinding_waits_for_the_current_work() {
        let mut app = App::default();
        app.live.entries.push(live::Entry::User("first".into()));
        app.live.busy = true;
        assert!(!app.rewind());
        assert!(!app.rewind_to(0));
        assert_eq!(app.live.entries.len(), 1);
        assert!(app.notice.as_deref().unwrap().contains("Esc"));
    }
}
