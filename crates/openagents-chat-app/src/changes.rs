//! The "What changed" card, its unified diff, and publishing the change.
//!
//! Reimplemented from Zeron's diff pane (public MIT zeronsh/zeron): one
//! unified view, virtualized by line, with paint-only syntax spans. Opening
//! the card shows the change. Nothing in the pane can edit it.
//!
//! A [`Reviewer`] holds one chat's change as the computer last read it
//! ([`TaskReview`], NIP-HOST `task.review` or the desktop's own runner):
//! the exact base and head revisions, Git's changed-file counts, and the
//! diff with its completeness, so a cut or unreadable diff never shows as
//! complete. While the card shows, the reviewer reads the change again now
//! and then; a read naming another head marks the shown view stale, keeps
//! it on screen, and offers **Refresh**, and a stale view cannot publish.
//! **Publish** is a typed request ([`Need::Publish`]) naming the reviewed
//! revisions, which the computer's task owner checks again before it
//! commits and pushes once; the card then links the commit or the pull
//! request. Each platform carries the reviewer's [`Need`]s over its own
//! transport and renders its [`Card`]; none of this is per platform.

use coder_host::access::review::{Completeness, Landing, Publication, PublishState, TaskReview};
use rust_native::syntax::{Highlighter, Span};
use std::time::{Duration, Instant};

/// The most lines one change document keeps.
pub const MAX_LINES: usize = 20_000;
/// The most bytes of diff text one document reads.
pub const MAX_BYTES: usize = 1024 * 1024;
/// The most bytes of one displayed line.
const MAX_LINE: usize = 2_000;
/// Visible lines drawn for one viewport, including one line of overscan.
const WINDOW_CAP: usize = 80;

/// What a displayed line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A `diff --git` file header.
    File,
    /// A hunk header.
    Hunk,
    /// An added line.
    Add,
    /// A removed line.
    Remove,
    /// An unchanged line.
    Context,
    /// A Git header or note that is not a changed line.
    Meta,
}

/// One displayed line. Spans are paint-only: filling them does not change
/// the text or the line count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    /// File extension the highlighter understands, such as `rs`.
    pub language: String,
    /// Foreground spans into `text`. `None` means not highlighted yet.
    pub spans: Option<Vec<Span>>,
}

/// A parsed unified diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    lines: Vec<Line>,
    files: usize,
    added: u32,
    removed: u32,
}

impl Document {
    /// How many lines the pane can scroll through.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the document has nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The lines, in file order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The card's second line: file count and added and removed counts.
    #[must_use]
    pub fn summary(&self) -> String {
        let files = match self.files {
            1 => "1 file".to_owned(),
            count => format!("{count} files"),
        };
        format!("{files}, +{}, −{}", self.added, self.removed)
    }

    /// The first visible line and how many lines fit in `viewport` points.
    ///
    /// `line` is the uniform line height. The result never walks the lines
    /// and never exceeds [`WINDOW_CAP`].
    #[must_use]
    pub fn window(&self, scroll: f32, viewport: f32, line: f32) -> (usize, usize) {
        let total = self.lines.len();
        if total == 0
            || !viewport.is_finite()
            || viewport <= 0.0
            || !line.is_finite()
            || line <= 0.0
        {
            return (0, 0);
        }
        let scroll = scroll.clamp(0.0, self.scroll_limit(viewport, line));
        let first = (scroll / line).floor() as usize;
        let count = ((viewport / line).ceil() as usize)
            .saturating_add(1)
            .clamp(1, WINDOW_CAP);
        let first = first.min(total - 1);
        (first, count.min(total - first))
    }

    /// How far the pane can scroll, in the same points as `viewport`.
    #[must_use]
    pub fn scroll_limit(&self, viewport: f32, line: f32) -> f32 {
        if !line.is_finite() || line <= 0.0 {
            return 0.0;
        }
        (self.lines.len() as f32 * line - viewport.max(0.0)).max(0.0)
    }

    /// Drops every line's syntax spans, so the next [`Document::ensure_spans`]
    /// colors them again, as after a change of scheme.
    pub fn clear_spans(&mut self) {
        for line in &mut self.lines {
            line.spans = None;
        }
    }

    /// Fill syntax spans for the visible lines. Headers stay plain.
    pub fn ensure_spans(&mut self, first: usize, count: usize, highlighter: &Highlighter) {
        let end = first.saturating_add(count).min(self.lines.len());
        let start = first.min(self.lines.len());
        for line in &mut self.lines[start..end] {
            if line.spans.is_some() {
                continue;
            }
            line.spans = Some(highlight(line, highlighter));
        }
    }
}

/// Parse a unified diff. Text past the line or byte bound is omitted, and
/// the document says so on its last line.
#[must_use]
pub fn parse(diff: &str) -> Document {
    let mut doc = Document {
        lines: Vec::new(),
        files: 0,
        added: 0,
        removed: 0,
    };
    let mut language = String::new();
    let mut bytes = 0usize;
    let mut cut = false;
    for line in diff.split_inclusive('\n') {
        bytes = bytes.saturating_add(line.len());
        if doc.lines.len() >= MAX_LINES || bytes > MAX_BYTES {
            cut = true;
            break;
        }
        let raw = line.trim_end_matches(['\n', '\r']);
        if raw.is_empty() && doc.lines.is_empty() {
            continue;
        }
        let kind = classify(raw);
        if kind == Kind::File {
            doc.files += 1;
            language = language_of(path_of(raw));
        } else if let Some(path) = raw.strip_prefix("+++ b/") {
            language = language_of(path);
        } else if let Some(path) = raw.strip_prefix("+++ ") {
            language = language_of(path.trim_start_matches("b/"));
        }
        match kind {
            Kind::Add => doc.added = doc.added.saturating_add(1),
            Kind::Remove => doc.removed = doc.removed.saturating_add(1),
            _ => {}
        }
        let shown = clip(raw);
        let highlight_later = matches!(kind, Kind::Add | Kind::Remove | Kind::Context);
        doc.lines.push(Line {
            kind,
            text: shown,
            language: if highlight_later {
                language.clone()
            } else {
                String::new()
            },
            spans: if highlight_later {
                None
            } else {
                Some(Vec::new())
            },
        });
    }
    if cut {
        doc.lines.push(Line {
            kind: Kind::Meta,
            text: "The rest of the changes are not shown.".into(),
            language: String::new(),
            spans: Some(Vec::new()),
        });
    }
    doc
}

/// The unified diff at the end of `text`, when `text` contains one.
#[must_use]
pub fn extract(text: &str) -> Option<&str> {
    let start = text
        .rfind("\ndiff --git ")
        .map(|index| index + 1)
        .or_else(|| text.starts_with("diff --git ").then_some(0))?;
    Some(&text[start..])
}

fn classify(line: &str) -> Kind {
    if line.starts_with("diff --git ") {
        Kind::File
    } else if line.starts_with("@@") {
        Kind::Hunk
    } else if line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("index ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("rename ")
        || line.starts_with("similarity ")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
        || line.starts_with("Binary files")
        || line.starts_with('\\')
    {
        Kind::Meta
    } else if line.starts_with('+') {
        Kind::Add
    } else if line.starts_with('-') {
        Kind::Remove
    } else if line.starts_with(' ') {
        Kind::Context
    } else {
        Kind::Meta
    }
}

fn path_of(header: &str) -> &str {
    header
        .rsplit_once(" b/")
        .map(|(_, path)| path.trim().trim_matches('"'))
        .unwrap_or("")
}

fn language_of(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_owned())
        .unwrap_or_default()
}

fn clip(line: &str) -> String {
    if line.len() <= MAX_LINE {
        return line.to_owned();
    }
    let mut end = MAX_LINE;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let mut shown = line[..end].to_owned();
    shown.push('…');
    shown
}

fn highlight(line: &Line, highlighter: &Highlighter) -> Vec<Span> {
    if line.language.is_empty() || line.text.is_empty() {
        return Vec::new();
    }
    let prefix = usize::from(u8::from(matches!(
        line.text.as_bytes().first(),
        Some(b'+' | b'-' | b' ')
    )));
    if prefix >= line.text.len() {
        return Vec::new();
    }
    let code = &line.text[prefix..];
    let mut spans = highlighter.spans(&line.language, code);
    for span in &mut spans {
        span.start = span.start.saturating_add(prefix).min(line.text.len());
        span.end = span.end.saturating_add(prefix).min(line.text.len());
    }
    spans.retain(|span| span.end > span.start);
    spans
}

/// How often an open change is read again to notice a moved worktree.
pub const CHECK_EVERY: Duration = Duration::from_secs(10);

/// What a platform must do for a [`Reviewer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    /// Read the task's change (`task.review`).
    Read { task: String },
    /// Publish the reviewed change (`task.publish`).
    Publish {
        task: String,
        base: String,
        head_commit: String,
        head: String,
    },
}

/// Why a read gave no review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    /// The computer reviews no change for this task, as an older host or a
    /// task without a worktree of its own.
    Unsupported,
    /// The read failed; `0` says why, for the person.
    Failed(String),
}

/// A control on the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
    /// Open the diff pane.
    Open,
    /// Show the change as it is now, after a stale view.
    Refresh,
    /// Publish the reviewed change.
    Publish,
}

/// How a card line reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    /// Something the reader must know before acting on the change.
    Warning,
}

/// One line of the card under its summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// A stable key suffix, such as `stale`.
    pub key: &'static str,
    pub text: String,
    pub tone: Tone,
}

/// The card a platform renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// File count and line counts, such as `2 files, +5, −1`.
    pub summary: String,
    /// The exact revisions, such as `Base 1a2b3c4d5e · head 6f7e8d9c0b`,
    /// or `None` when the computer did not name them.
    pub revisions: Option<String>,
    pub notes: Vec<Note>,
    /// The controls, in order, with their labels.
    pub actions: Vec<(CardAction, &'static str)>,
    /// The published commit or pull request, as a label and an `https`
    /// link.
    pub link: Option<(String, String)>,
}

/// A change as one chat shows it.
#[derive(Clone, Debug)]
struct Shown {
    review: TaskReview,
    document: Document,
}

/// One chat's change: what the card and the pane show, whether that view
/// is stale, and its publication.
#[derive(Clone, Debug, Default)]
pub struct Reviewer {
    task: Option<String>,
    shown: Option<Shown>,
    /// A newer read naming another head: the shown view is stale.
    newer: Option<TaskReview>,
    /// A diff read from the transcript, with no revisions, from a computer
    /// that reviews no change.
    legacy: Option<Document>,
    unsupported: bool,
    reading: bool,
    publishing: bool,
    want_publish: bool,
    /// When the last read ended; `None` reads at the next tick.
    read_at: Option<Instant>,
    error: Option<String>,
    revision: u64,
}

impl Reviewer {
    /// A reviewer for no task yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A reviewer showing `review`, already read, as a fixture or a
    /// review carried in from elsewhere does.
    #[must_use]
    pub fn showing(review: TaskReview, now: Instant) -> Self {
        let mut reviewer = Self::new();
        reviewer.reset(Some(&review.task));
        reviewer.read_at = Some(now);
        reviewer.shown = Some(Shown {
            document: parse(&review.diff),
            review,
        });
        reviewer
    }

    /// Changes each time what the card shows may have.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Follow `task`'s change, from nothing when it is another task or a
    /// new turn of the same one.
    pub fn reset(&mut self, task: Option<&str>) {
        let keep_unsupported = self.unsupported && self.task.as_deref() == task;
        *self = Self {
            task: task.map(str::to_owned),
            unsupported: keep_unsupported,
            revision: self.revision + 1,
            ..Self::default()
        };
    }

    /// The task followed.
    #[must_use]
    pub fn task(&self) -> Option<&str> {
        self.task.as_deref()
    }

    /// What to do now. `ended` says the task's turn ended with a result,
    /// so its change is final; `open` says the card or the pane shows, so
    /// a moved worktree should be noticed.
    pub fn tick(&mut self, now: Instant, ended: bool, open: bool) -> Option<Need> {
        let task = self.task.clone()?;
        if !ended || self.reading || self.publishing {
            return None;
        }
        if self.want_publish {
            self.want_publish = false;
            let shown = &self.shown.as_ref()?.review;
            self.publishing = true;
            self.revision += 1;
            return Some(Need::Publish {
                task,
                base: shown.base.clone(),
                head_commit: shown.head_commit.clone(),
                head: shown.head.clone(),
            });
        }
        if self.unsupported {
            return None;
        }
        let due = match self.read_at {
            None => true,
            Some(at) => {
                open && self.shown.is_some() && now.saturating_duration_since(at) >= CHECK_EVERY
            }
        };
        if !due {
            return None;
        }
        self.reading = true;
        Some(Need::Read { task })
    }

    /// The answer to a [`Need::Read`].
    pub fn read(&mut self, result: Result<TaskReview, ReadFailure>, now: Instant) {
        self.reading = false;
        self.read_at = Some(now);
        self.revision += 1;
        match result {
            Ok(review) if Some(review.task.as_str()) == self.task.as_deref() => {
                self.error = None;
                match &mut self.shown {
                    None => {
                        self.shown = Some(Shown {
                            document: parse(&review.diff),
                            review,
                        });
                    }
                    Some(shown) if shown.review.same_head(&review) => {
                        // The same change: only its publication may be new.
                        shown.review.publication = review.publication;
                        self.newer = None;
                    }
                    Some(_) => self.newer = Some(review),
                }
            }
            Ok(_) => self.error = Some("The computer answered for another task.".into()),
            Err(ReadFailure::Unsupported) => self.unsupported = true,
            Err(ReadFailure::Failed(why)) => self.error = Some(why),
        }
    }

    /// Show a diff found in the task's transcript, for a computer that
    /// reviews no change itself. It names no revisions and cannot publish.
    pub fn set_legacy(&mut self, diff: Option<&str>) {
        let next = diff.map(parse);
        if next != self.legacy {
            self.legacy = next;
            self.revision += 1;
        }
    }

    /// Whether the computer reviews no change for this task.
    #[must_use]
    pub fn unsupported(&self) -> bool {
        self.unsupported
    }

    /// Whether a newer read named another head than the one shown.
    #[must_use]
    pub fn stale(&self) -> bool {
        self.newer.is_some()
    }

    /// Show the newer read in place of the stale view.
    pub fn refresh(&mut self) {
        if let Some(review) = self.newer.take() {
            self.shown = Some(Shown {
                document: parse(&review.diff),
                review,
            });
            self.revision += 1;
        } else {
            // Read again now.
            self.read_at = None;
        }
    }

    /// The change shown, when the computer named its revisions.
    #[must_use]
    pub fn review(&self) -> Option<&TaskReview> {
        self.shown.as_ref().map(|shown| &shown.review)
    }

    /// The shown diff's lines.
    #[must_use]
    pub fn document(&self) -> Option<&Document> {
        self.shown
            .as_ref()
            .map(|shown| &shown.document)
            .or(self.legacy.as_ref())
    }

    /// The shown diff's lines, to fill syntax spans.
    pub fn document_mut(&mut self) -> Option<&mut Document> {
        match &mut self.shown {
            Some(shown) => Some(&mut shown.document),
            None => self.legacy.as_mut(),
        }
    }

    /// Whether there is a change to show.
    #[must_use]
    pub fn has_change(&self) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|shown| shown.review.files_total > 0)
            || self.legacy.as_ref().is_some_and(|doc| !doc.is_empty())
    }

    /// Whether **Publish** may be offered: a current view of a change the
    /// computer named, not yet published, with no publish in flight.
    #[must_use]
    pub fn can_publish(&self) -> bool {
        let Some(shown) = &self.shown else {
            return false;
        };
        !self.stale()
            && !self.publishing
            && shown.review.files_total > 0
            && !matches!(shown.review.completeness, Completeness::Unknown { .. })
            && self
                .publication()
                .is_none_or(|p| p.state != PublishState::Published)
    }

    /// Ask to publish the shown change. Refused, changing nothing, when
    /// [`Reviewer::can_publish`] is false.
    pub fn publish(&mut self) -> bool {
        if !self.can_publish() {
            if self.stale() {
                self.error =
                    Some("The change moved since this view. Refresh, then review it again.".into());
                self.revision += 1;
            }
            return false;
        }
        self.want_publish = true;
        self.error = None;
        self.revision += 1;
        true
    }

    /// The answer to a [`Need::Publish`]. A refusal reads the change again
    /// at once, so a moved worktree shows as stale.
    pub fn published(&mut self, result: Result<Publication, String>) {
        self.publishing = false;
        self.revision += 1;
        match result {
            Ok(publication) => {
                if publication.state == PublishState::Refused {
                    self.read_at = None;
                }
                self.error = None;
                if let Some(shown) = self.shown.as_mut().filter(|shown| {
                    shown.review.task == publication.task
                        && shown.review.base == publication.base
                        && shown.review.head_commit == publication.head_commit
                        && shown.review.head == publication.head
                }) {
                    shown.review.publication = Some(publication);
                }
            }
            Err(why) => self.error = Some(why),
        }
    }

    /// The shown change's publication, when it was published or attempted.
    #[must_use]
    pub fn publication(&self) -> Option<&Publication> {
        let review = &self.shown.as_ref()?.review;
        review.publication.as_ref().filter(|p| {
            p.base == review.base && p.head_commit == review.head_commit && p.head == review.head
        })
    }

    /// The card, when there is a change to show. `allowed` says this device
    /// may publish on the computer (the owner, or a grant with `operate`);
    /// without it no **Publish** control shows.
    #[must_use]
    pub fn card(&self, allowed: bool) -> Option<Card> {
        if !self.has_change() {
            return None;
        }
        let mut notes = Vec::new();
        let mut actions = vec![(CardAction::Open, "What changed")];
        let mut link = None;
        let (summary, revisions) = match &self.shown {
            Some(shown) => {
                let review = &shown.review;
                let files = match review.files_total {
                    1 => "1 file".to_owned(),
                    count => format!("{count} files"),
                };
                let mut summary = format!("{files}, +{}, −{}", review.added, review.removed);
                if review.uncounted > 0 {
                    summary.push_str(&format!(", {} not counted", review.uncounted));
                }
                let mut revisions = format!(
                    "Base {} · head {}",
                    short(&review.base),
                    short(&review.head)
                );
                if review.head_commit != review.base {
                    revisions.push_str(&format!(" on {}", short(&review.head_commit)));
                }
                match &review.completeness {
                    Completeness::Complete => {}
                    Completeness::Truncated { shown, total } => notes.push(Note {
                        key: "truncated",
                        text: match total {
                            Some(total) => format!(
                                "The diff shows its first {} of {}. The counts cover every file.",
                                size(*shown),
                                size(*total)
                            ),
                            None => format!(
                                "The diff shows its first {}; the rest was not read. The counts cover every file.",
                                size(*shown)
                            ),
                        },
                        tone: Tone::Warning,
                    }),
                    Completeness::Unknown { reason } => notes.push(Note {
                        key: "unknown",
                        text: format!("The diff could not be read: {reason}"),
                        tone: Tone::Warning,
                    }),
                }
                if (review.files.len() as u64) < review.files_total {
                    notes.push(Note {
                        key: "listed",
                        text: format!(
                            "{} of {} files are listed.",
                            review.files.len(),
                            review.files_total
                        ),
                        tone: Tone::Plain,
                    });
                }
                (summary, Some(revisions))
            }
            None => {
                let summary = self
                    .legacy
                    .as_ref()
                    .map(Document::summary)
                    .unwrap_or_default();
                notes.push(Note {
                    key: "unnamed",
                    text: "This computer did not name the revisions this diff compares, so it cannot be published from here."
                        .into(),
                    tone: Tone::Warning,
                });
                (summary, None)
            }
        };
        if self.stale() {
            notes.push(Note {
                key: "stale",
                text: "The worktree changed since this view. Refresh to review the current change."
                    .into(),
                tone: Tone::Warning,
            });
            actions.push((CardAction::Refresh, "Refresh"));
        }
        if self.publishing {
            notes.push(Note {
                key: "publishing",
                text: "Publishing the reviewed change…".into(),
                tone: Tone::Plain,
            });
        }
        if let Some(publication) = self.publication() {
            let commit = publication.commit.as_deref().map(short).unwrap_or_default();
            let branch = publication.branch.clone().unwrap_or_default();
            match publication.state {
                PublishState::Published => {
                    let label = match publication.landing {
                        Landing::DraftPullRequest => "Draft pull request".to_owned(),
                        Landing::Branch => format!("Commit {commit} on {branch}"),
                    };
                    match &publication.url {
                        Some(url) => link = Some((label, url.clone())),
                        None => notes.push(Note {
                            key: "published",
                            text: format!("Published {commit} on {branch}."),
                            tone: Tone::Plain,
                        }),
                    }
                }
                PublishState::Pushed | PublishState::Uncertain | PublishState::Refused => {
                    notes.push(Note {
                        key: "publication",
                        text: publication.note.clone(),
                        tone: Tone::Warning,
                    });
                }
            }
        }
        if let Some(error) = &self.error {
            notes.push(Note {
                key: "error",
                text: error.clone(),
                tone: Tone::Warning,
            });
        }
        if allowed && self.can_publish() {
            let label = match self.publication().map(|p| p.state) {
                Some(PublishState::Pushed | PublishState::Uncertain) => "Finish publishing",
                _ => "Publish",
            };
            actions.push((CardAction::Publish, label));
        }
        Some(Card {
            summary,
            revisions,
            notes,
            actions,
            link,
        })
    }
}

/// The first ten characters of a revision.
fn short(id: &str) -> &str {
    &id[..10.min(id.len())]
}

/// A byte count for a person: `512 bytes`, `48 KB`, `1.5 MB`.
fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn sample(lines: usize) -> String {
        let mut diff = String::from(
            "diff --git a/src/answer.rs b/src/answer.rs\n--- a/src/answer.rs\n+++ b/src/answer.rs\n@@ -1,1 +1,4996 @@\n fn keep() {}\n",
        );
        let body = lines.saturating_sub(5);
        for index in 0..body {
            diff.push_str(&format!("+fn line_{index}() {{ return {index}; }}\n"));
        }
        diff
    }

    #[test]
    fn a_unified_diff_counts_files_and_changed_lines() {
        let doc = parse(
            "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1,2 @@\n-fn old() {}\n+fn answer() { return 1; }\n+fn extra() {}\n\ndiff --git a/notes.md b/notes.md\n--- a/notes.md\n+++ b/notes.md\n@@ -1 +0,0 @@\n-gone\n",
        );
        assert_eq!(doc.files, 2);
        assert_eq!((doc.added, doc.removed), (2, 2));
        assert!(doc.lines().iter().any(|line| line.kind == Kind::Hunk));
        assert_eq!(doc.summary(), "2 files, +2, −2");
        assert!(doc.lines().iter().all(|line| match line.kind {
            Kind::Add | Kind::Remove | Kind::Context => line.spans.is_none(),
            _ => line.spans.as_ref().is_some_and(Vec::is_empty),
        }));
    }

    #[test]
    fn syntax_spans_color_a_line_without_changing_it() {
        let mut doc = parse(
            "diff --git a/src/answer.rs b/src/answer.rs\n+++ b/src/answer.rs\n@@ -0,0 +1 @@\n+fn answer() { return 1; }\n",
        );
        let before: Vec<_> = doc.lines().iter().map(|line| line.text.clone()).collect();
        doc.ensure_spans(0, doc.len(), &Highlighter::default());
        let add = doc
            .lines()
            .iter()
            .find(|line| line.kind == Kind::Add)
            .expect("added line");
        assert_eq!(
            doc.lines()
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            before.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let spans = add.spans.as_ref().expect("spans");
        assert!(spans.iter().any(|span| span.end > span.start));
        assert!(spans.iter().all(|span| span.end <= add.text.len()));
    }

    #[test]
    fn a_five_thousand_line_diff_windows_without_walking_every_line() {
        let doc = parse(&sample(5_000));
        assert_eq!(doc.len(), 5_000);
        assert_eq!(doc.summary(), "1 file, +4995, −0");
        let (first, count) = doc.window(0.0, 400.0, 18.0);
        assert_eq!(first, 0);
        assert!(count <= WINDOW_CAP);
        assert!(count < 40);
        let started = Instant::now();
        let mut seen = 0usize;
        for step in 0..200 {
            let (next, shown) = doc.window(step as f32 * 18.0, 400.0, 18.0);
            assert!(shown <= WINDOW_CAP);
            assert!(shown < doc.len());
            seen = next;
        }
        assert!(seen > 100);
        assert!(started.elapsed().as_millis() < 50);
        let end = doc.scroll_limit(400.0, 18.0);
        let (first, count) = doc.window(end, 400.0, 18.0);
        assert_eq!(first + count, doc.len());
        assert!(doc.lines()[first + count - 1].text.contains("line_"));
    }

    #[test]
    fn extract_keeps_the_last_unified_diff_in_a_result() {
        let text = "Coder finished.\ndiff --git a/old b/old\n+old\n\nAnd then:\ndiff --git a/src/a.rs b/src/a.rs\n+fn answer() {}\n";
        let found = extract(text).expect("diff");
        assert!(found.starts_with("diff --git a/src/a.rs"));
        assert_eq!(parse(found).added, 1);
        assert!(extract("no changes here").is_none());
    }

    #[test]
    fn a_line_past_the_bound_is_cut_on_a_character_boundary() {
        let long = format!("+{}\n", "é".repeat(MAX_LINE));
        let diff = format!("diff --git a/a.txt b/a.txt\n{long}");
        let doc = parse(&diff);
        let line = doc
            .lines()
            .iter()
            .find(|line| line.kind == Kind::Add)
            .unwrap();
        assert!(line.text.ends_with('…'));
        assert!(line.text.is_char_boundary(line.text.len()));
        assert!(line.text.len() <= MAX_LINE + '…'.len_utf8());
    }

    fn wire(head: char, diff: &str, completeness: Completeness) -> TaskReview {
        TaskReview {
            task: "a".repeat(64),
            base: "b".repeat(40),
            head_commit: "b".repeat(40),
            head: head.to_string().repeat(40),
            files: vec![
                coder_host::access::review::FileCount {
                    path: "src/a.rs".into(),
                    status: coder_host::access::review::FileStatus::Modified,
                    added: Some(2),
                    removed: Some(1),
                },
                coder_host::access::review::FileCount {
                    path: "logo.png".into(),
                    status: coder_host::access::review::FileStatus::Added,
                    added: None,
                    removed: None,
                },
            ],
            files_total: 2,
            added: 2,
            removed: 1,
            uncounted: 1,
            diff: diff.into(),
            completeness,
            publication: None,
        }
    }

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1,2 @@\n-fn old() {}\n+fn answer() {}\n+fn extra() {}\n";

    fn reviewer() -> (Reviewer, Instant) {
        let now = Instant::now();
        let mut reviewer = Reviewer::new();
        reviewer.reset(Some(&"a".repeat(64)));
        assert_eq!(
            reviewer.tick(now, false, false),
            None,
            "nothing until the turn ends"
        );
        assert_eq!(
            reviewer.tick(now, true, false),
            Some(Need::Read {
                task: "a".repeat(64)
            })
        );
        assert_eq!(reviewer.tick(now, true, true), None, "one read at a time");
        reviewer.read(Ok(wire('c', DIFF, Completeness::Complete)), now);
        (reviewer, now)
    }

    #[test]
    fn the_card_names_exact_revisions_and_whole_counts() {
        let (reviewer, _) = reviewer();
        let card = reviewer.card(true).expect("a card");
        assert_eq!(card.summary, "2 files, +2, −1, 1 not counted");
        assert_eq!(
            card.revisions.as_deref(),
            Some("Base bbbbbbbbbb · head cccccccccc")
        );
        assert!(card.notes.is_empty(), "{:?}", card.notes);
        assert_eq!(
            card.actions,
            vec![
                (CardAction::Open, "What changed"),
                (CardAction::Publish, "Publish")
            ]
        );
        // Without the right to operate, nothing offers to publish.
        assert_eq!(reviewer.card(false).unwrap().actions.len(), 1);
        assert_eq!(reviewer.document().unwrap().summary(), "1 file, +2, −1");
    }

    #[test]
    fn a_cut_or_unreadable_diff_never_reads_as_complete() {
        let now = Instant::now();
        let mut reviewer = Reviewer::new();
        reviewer.reset(Some(&"a".repeat(64)));
        reviewer.tick(now, true, false);
        reviewer.read(
            Ok(wire(
                'c',
                DIFF,
                Completeness::Truncated {
                    shown: DIFF.len() as u64,
                    total: Some(300_000),
                },
            )),
            now,
        );
        let card = reviewer.card(true).unwrap();
        assert!(card.notes.iter().any(|note| note.key == "truncated"
            && note.text.contains("of 293 KB")
            && note.tone == Tone::Warning));
        reviewer.reset(Some(&"a".repeat(64)));
        reviewer.tick(now, true, false);
        reviewer.read(
            Ok(wire(
                'c',
                "",
                Completeness::Unknown {
                    reason: "Git could not write the diff".into(),
                },
            )),
            now,
        );
        let card = reviewer.card(true).unwrap();
        assert!(card.notes.iter().any(|note| note.key == "unknown"));
        assert!(!reviewer.can_publish(), "an unread change does not publish");
        // A computer that reviews nothing shows a transcript diff with no
        // revisions and no publish.
        let mut legacy = Reviewer::new();
        legacy.reset(Some(&"a".repeat(64)));
        legacy.tick(now, true, false);
        legacy.read(Err(ReadFailure::Unsupported), now);
        assert_eq!(legacy.tick(now + CHECK_EVERY * 2, true, true), None);
        legacy.set_legacy(Some(DIFF));
        let card = legacy.card(true).unwrap();
        assert_eq!(card.revisions, None);
        assert!(card.notes.iter().any(|note| note.key == "unnamed"));
        assert_eq!(card.actions, vec![(CardAction::Open, "What changed")]);
    }

    #[test]
    fn a_moved_head_makes_the_view_stale_and_refuses_to_publish_it() {
        let (mut reviewer, now) = reviewer();
        // Closed, the card is not read again.
        assert_eq!(reviewer.tick(now + CHECK_EVERY, true, false), None);
        let later = now + CHECK_EVERY;
        assert_eq!(
            reviewer.tick(later, true, true),
            Some(Need::Read {
                task: "a".repeat(64)
            })
        );
        reviewer.read(Ok(wire('d', DIFF, Completeness::Complete)), later);
        assert!(reviewer.stale());
        // The shown view stays until the person refreshes.
        assert_eq!(reviewer.review().unwrap().head, "c".repeat(40));
        let card = reviewer.card(true).unwrap();
        assert!(card.notes.iter().any(|note| note.key == "stale"));
        assert!(card.actions.contains(&(CardAction::Refresh, "Refresh")));
        assert!(
            !card
                .actions
                .iter()
                .any(|(action, _)| *action == CardAction::Publish)
        );
        assert!(!reviewer.publish(), "a stale view does not publish");
        assert_eq!(reviewer.tick(later, true, true), None);
        reviewer.refresh();
        assert!(!reviewer.stale());
        assert_eq!(reviewer.review().unwrap().head, "d".repeat(40));
        assert!(reviewer.publish());
        assert_eq!(
            reviewer.tick(later, true, true),
            Some(Need::Publish {
                task: "a".repeat(64),
                base: "b".repeat(40),
                head_commit: "b".repeat(40),
                head: "d".repeat(40),
            })
        );
        // While it publishes, nothing else is asked and no second publish.
        assert_eq!(reviewer.tick(later + CHECK_EVERY, true, true), None);
        assert!(!reviewer.publish());
    }

    #[test]
    fn a_published_change_links_its_pull_request_and_an_uncertain_one_offers_to_finish() {
        let (mut reviewer, now) = reviewer();
        assert!(reviewer.publish());
        let Some(Need::Publish {
            task,
            base,
            head_commit,
            head,
        }) = reviewer.tick(now, true, true)
        else {
            panic!("a publish");
        };
        let publication = |state, url: Option<&str>| Publication {
            operation: "e".repeat(64),
            task: task.clone(),
            base: base.clone(),
            head_commit: head_commit.clone(),
            head: head.clone(),
            landing: Landing::DraftPullRequest,
            state,
            branch: Some("coder/review-aaaaaaaa-eeeeeeee".into()),
            commit: Some("f".repeat(40)),
            url: url.map(str::to_owned),
            note: "Not sure the push of ffffffffff reached the remote.".into(),
        };
        reviewer.published(Ok(publication(PublishState::Uncertain, None)));
        let card = reviewer.card(true).unwrap();
        assert!(card.notes.iter().any(|note| note.key == "publication"));
        assert!(
            card.actions
                .contains(&(CardAction::Publish, "Finish publishing"))
        );
        assert!(reviewer.publish());
        reviewer.tick(now, true, true).unwrap();
        reviewer.published(Ok(publication(
            PublishState::Published,
            Some("https://github.com/o/r/pull/7"),
        )));
        let card = reviewer.card(true).unwrap();
        assert_eq!(
            card.link,
            Some((
                "Draft pull request".into(),
                "https://github.com/o/r/pull/7".into()
            ))
        );
        assert!(
            !card
                .actions
                .iter()
                .any(|(action, _)| *action == CardAction::Publish)
        );
        // A read of the same head keeps the link; the host records it too.
        let mut same = wire('c', DIFF, Completeness::Complete);
        same.publication = reviewer.publication().cloned();
        reviewer.read(Ok(same), now + CHECK_EVERY);
        assert!(reviewer.card(true).unwrap().link.is_some());
        // A transport failure says so and allows another try.
        let (mut failing, now) = self::tests::reviewer();
        failing.publish();
        failing.tick(now, true, true).unwrap();
        failing.published(Err("The computer is offline.".into()));
        let card = failing.card(true).unwrap();
        assert!(card.notes.iter().any(|note| note.key == "error"));
        assert!(card.actions.contains(&(CardAction::Publish, "Publish")));
    }
}
