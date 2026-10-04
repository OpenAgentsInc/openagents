//! Line comments on a reviewed change, sent as one follow-up turn for
//! **Request changes**.
//!
//! Reimplemented from Zeron's review comments (public MIT zeronsh/zeron at
//! `9e1a1115`, `crates/ui/src/comments.rs` and `comment_ui.rs`): a comment
//! pins a body to one file and line on one side of the diff, and the set
//! folds into the next prompt as located bullets. Zeron anchors a comment to
//! the diff on screen; here each comment also records the exact base and
//! head revisions of the [`TaskReview`] it was written against. When the
//! computer reads a newer head, those comments are outdated: they stay in
//! the set for the person to see, edit, or remove, but a follow-up for the
//! new head never sends them, because their line numbers may name other
//! code.
//!
//! [`target`] maps a line of the diff pane's [`Document`] to the file and
//! line a comment cites. [`Comments::follow_up`] writes the turn's text;
//! [`Comments::sent`] drops what a delivered follow-up carried. The set has
//! no rendering; each platform draws it beside the ported diff pane.

use crate::changes::{Document, Kind};
use coder_host::access::review::TaskReview;

/// The most comments one set holds.
pub const MAX_COMMENTS: usize = 200;
/// The most characters of one comment's body.
pub const MAX_BODY: usize = 4_000;

/// The text a follow-up opens with when the person wrote no note.
pub const COMMENT_ONLY_TEXT: &str = "Address the review comments below.";
/// The line that introduces the located comments in a follow-up.
pub const COMMENT_BLOCK_HEADER: &str = "Comments on the diff (each cites the file and line it belongs to; L is a line number in the original file, R is a line number in the changed file):";

/// Which side of the diff a line number counts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The original file, at the review's base.
    Old,
    /// The changed file, at the review's head.
    New,
}

impl Side {
    /// The one-letter tag a follow-up cites: `L` or `R`.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Old => "L",
            Self::New => "R",
        }
    }
}

/// The file and line a comment is pinned to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The changed file's path.
    pub path: String,
    /// The path before a rename, when the diff renamed the file.
    pub old_path: Option<String>,
    pub side: Side,
    /// The 1-based line number on `side`.
    pub line: u32,
}

impl Target {
    /// The path `line` is valid in. An old-side line of a renamed file only
    /// exists under its earlier name.
    #[must_use]
    pub fn cite_path(&self) -> &str {
        match (self.side, &self.old_path) {
            (Side::Old, Some(old)) => old,
            _ => &self.path,
        }
    }

    /// `path:line`, as a follow-up and a comment card cite it.
    #[must_use]
    pub fn location(&self) -> String {
        format!("{}:{}", self.cite_path(), self.line)
    }
}

/// The comment target of the diff pane's line `index`: an added or
/// unchanged line cites the changed file, a removed line the original
/// file. Headers, hunk lines, and notes have no target.
#[must_use]
pub fn target(document: &Document, index: usize) -> Option<Target> {
    let lines = document.lines();
    let shown = lines.get(index)?;
    if !matches!(shown.kind, Kind::Add | Kind::Remove | Kind::Context) {
        return None;
    }
    let mut path = String::new();
    let mut old_path: Option<String> = None;
    let mut old = 0u32;
    let mut new = 0u32;
    let mut in_hunk = false;
    for line in &lines[..=index] {
        let text = line.text.as_str();
        match line.kind {
            Kind::File => {
                let (before, after) = header_paths(text);
                path = after;
                old_path = (!before.is_empty() && before != path).then_some(before);
                in_hunk = false;
            }
            Kind::Hunk => match hunk_starts(text) {
                Some((first_old, first_new)) => {
                    old = first_old;
                    new = first_new;
                    in_hunk = true;
                }
                None => in_hunk = false,
            },
            Kind::Meta => {
                if let Some(before) = text.strip_prefix("rename from ") {
                    let before = before.trim().to_owned();
                    old_path = (before != path).then_some(before);
                }
            }
            Kind::Add => new = new.saturating_add(1),
            Kind::Remove => old = old.saturating_add(1),
            Kind::Context => {
                old = old.saturating_add(1);
                new = new.saturating_add(1);
            }
        }
    }
    if !in_hunk || path.is_empty() {
        return None;
    }
    // Each counter now stands one past the line it last counted.
    let (side, line) = match shown.kind {
        Kind::Remove => (Side::Old, old.checked_sub(1)?),
        _ => (Side::New, new.checked_sub(1)?),
    };
    (line > 0).then_some(Target {
        path,
        old_path,
        side,
        line,
    })
}

/// The `a/` and `b/` paths of a `diff --git` header.
fn header_paths(header: &str) -> (String, String) {
    let rest = header.strip_prefix("diff --git ").unwrap_or(header);
    match rest.rsplit_once(" b/") {
        Some((before, after)) => (
            before
                .trim()
                .trim_start_matches("a/")
                .trim_matches('"')
                .to_owned(),
            after.trim().trim_matches('"').to_owned(),
        ),
        None => (String::new(), String::new()),
    }
}

/// The first old and new line numbers of `@@ -a,b +c,d @@`. A range that
/// starts at 0 holds no lines, so its next line is still line 1.
fn hunk_starts(header: &str) -> Option<(u32, u32)> {
    let mut parts = header.split_whitespace().skip(1);
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let start = |range: &str| -> Option<u32> {
        let first = range.split(',').next()?.parse::<u32>().ok()?;
        Some(first.max(1))
    };
    Some((start(old)?, start(new)?))
}

/// One comment, pinned to a line at the revisions it was written against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// Unique within its set; never reused.
    pub id: u64,
    pub target: Target,
    pub body: String,
    /// The reviewed base commit.
    pub base: String,
    /// The reviewed head tree.
    pub head: String,
}

impl Comment {
    /// Whether the comment was written against `review`'s revisions.
    #[must_use]
    pub fn is_current(&self, review: &TaskReview) -> bool {
        self.base == review.base && self.head == review.head
    }
}

/// Why the set refused a change or a follow-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The body is empty after trimming.
    EmptyBody,
    /// The body is longer than [`MAX_BODY`] characters.
    TooLong,
    /// The set already holds [`MAX_COMMENTS`] comments.
    TooMany,
    /// No comment has this id.
    NotFound,
    /// The follow-up has no note and no comment on the current revisions.
    Nothing,
}

/// A follow-up turn ready to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowUp {
    /// The turn's text.
    pub text: String,
    /// The comments it carries, for [`Comments::sent`].
    pub ids: Vec<u64>,
    /// Comments left out because they name older revisions.
    pub outdated: usize,
}

/// One review's line comments.
#[derive(Clone, Debug, Default)]
pub struct Comments {
    comments: Vec<Comment>,
    next: u64,
}

impl Comments {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every comment, in the order they were added.
    #[must_use]
    pub fn all(&self) -> &[Comment] {
        &self.comments
    }

    /// Whether the set holds no comment.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    /// The comments pinned to `target`'s file, side, and line.
    pub fn at<'a>(&'a self, target: &'a Target) -> impl Iterator<Item = &'a Comment> + 'a {
        self.comments.iter().filter(move |comment| {
            comment.target.path == target.path
                && comment.target.side == target.side
                && comment.target.line == target.line
        })
    }

    /// The comments written against revisions other than `review`'s.
    #[must_use]
    pub fn outdated(&self, review: &TaskReview) -> usize {
        self.comments
            .iter()
            .filter(|comment| !comment.is_current(review))
            .count()
    }

    /// Pin `body` to `target` at `review`'s revisions, and return its id.
    ///
    /// # Errors
    ///
    /// [`Refusal::EmptyBody`], [`Refusal::TooLong`], or
    /// [`Refusal::TooMany`].
    pub fn add(&mut self, review: &TaskReview, target: Target, body: &str) -> Result<u64, Refusal> {
        let body = checked(body)?;
        if self.comments.len() >= MAX_COMMENTS {
            return Err(Refusal::TooMany);
        }
        let id = self.next;
        self.next += 1;
        self.comments.push(Comment {
            id,
            target,
            body,
            base: review.base.clone(),
            head: review.head.clone(),
        });
        Ok(id)
    }

    /// Replace comment `id`'s body. The comment keeps its line and its
    /// revisions: editing an outdated comment does not make it current.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFound`], [`Refusal::EmptyBody`], or
    /// [`Refusal::TooLong`].
    pub fn edit(&mut self, id: u64, body: &str) -> Result<(), Refusal> {
        let body = checked(body)?;
        let comment = self
            .comments
            .iter_mut()
            .find(|comment| comment.id == id)
            .ok_or(Refusal::NotFound)?;
        comment.body = body;
        Ok(())
    }

    /// Remove comment `id`. Returns whether it was there.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.comments.len();
        self.comments.retain(|comment| comment.id != id);
        self.comments.len() != before
    }

    /// Remove every comment written against revisions other than
    /// `review`'s, and return how many went.
    pub fn discard_outdated(&mut self, review: &TaskReview) -> usize {
        let before = self.comments.len();
        self.comments.retain(|comment| comment.is_current(review));
        before - self.comments.len()
    }

    /// The **Request changes** turn for `review`: the person's `note`, then
    /// each current comment as a located bullet, in file and line order.
    ///
    /// # Errors
    ///
    /// [`Refusal::Nothing`] when `note` is blank and no comment names
    /// `review`'s revisions.
    pub fn follow_up(&self, review: &TaskReview, note: &str) -> Result<FollowUp, Refusal> {
        let note = note.trim();
        let mut current: Vec<&Comment> = self
            .comments
            .iter()
            .filter(|comment| comment.is_current(review))
            .collect();
        if note.is_empty() && current.is_empty() {
            return Err(Refusal::Nothing);
        }
        current.sort_by(|a, b| {
            (a.target.cite_path(), a.target.line, a.target.side, a.id).cmp(&(
                b.target.cite_path(),
                b.target.line,
                b.target.side,
                b.id,
            ))
        });
        let mut text = format!(
            "Changes requested on the review of base {} and head {}.\n\n",
            review.base, review.head
        );
        text.push_str(if note.is_empty() {
            COMMENT_ONLY_TEXT
        } else {
            note
        });
        if !current.is_empty() {
            text.push_str("\n\n");
            text.push_str(COMMENT_BLOCK_HEADER);
            for comment in &current {
                text.push_str(&format!(
                    "\n- {} ({}): {}",
                    comment.target.location(),
                    comment.target.side.tag(),
                    comment.body.replace('\n', "\n  ")
                ));
            }
        }
        Ok(FollowUp {
            text,
            ids: current.iter().map(|comment| comment.id).collect(),
            outdated: self.comments.len() - current.len(),
        })
    }

    /// Drop the comments a delivered follow-up carried. A comment edited
    /// after the follow-up was written is dropped too, as it was sent.
    pub fn sent(&mut self, follow_up: &FollowUp) {
        self.comments
            .retain(|comment| !follow_up.ids.contains(&comment.id));
    }
}

fn checked(body: &str) -> Result<String, Refusal> {
    let body = body.trim();
    if body.is_empty() {
        return Err(Refusal::EmptyBody);
    }
    if body.chars().count() > MAX_BODY {
        return Err(Refusal::TooLong);
    }
    Ok(body.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changes::parse;
    use coder_host::access::review::Completeness;

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -3,3 +3,4 @@ fn top() {}\n fn keep() {}\n-fn old() {}\n+fn answer() {}\n+fn extra() {}\n fn tail() {}\ndiff --git a/old_name.rs b/new_name.rs\nsimilarity index 90%\nrename from old_name.rs\nrename to new_name.rs\n--- a/old_name.rs\n+++ b/new_name.rs\n@@ -7 +7 @@\n-gone\n+here\ndiff --git a/fresh.md b/fresh.md\nnew file mode 100644\n--- /dev/null\n+++ b/fresh.md\n@@ -0,0 +1,2 @@\n+one\n+two\n";

    fn review(head: char) -> TaskReview {
        TaskReview {
            task: "a".repeat(64),
            base: "b".repeat(40),
            head_commit: "b".repeat(40),
            head: head.to_string().repeat(40),
            files: Vec::new(),
            files_total: 3,
            added: 5,
            removed: 2,
            uncounted: 0,
            diff: DIFF.into(),
            completeness: Completeness::Complete,
            publication: None,
        }
    }

    fn index_of(document: &Document, text: &str) -> usize {
        document
            .lines()
            .iter()
            .position(|line| line.text == text)
            .expect("line")
    }

    fn at(text: &str) -> Target {
        let document = parse(DIFF);
        target(&document, index_of(&document, text)).expect("target")
    }

    #[test]
    fn diff_lines_map_to_the_file_and_line_they_belong_to() {
        let document = parse(DIFF);
        let keep = at(" fn keep() {}");
        assert_eq!(
            (keep.path.as_str(), keep.side, keep.line),
            ("src/a.rs", Side::New, 3)
        );
        let old = at("-fn old() {}");
        assert_eq!((old.side, old.line), (Side::Old, 4));
        let answer = at("+fn answer() {}");
        assert_eq!((answer.side, answer.line), (Side::New, 4));
        let extra = at("+fn extra() {}");
        assert_eq!((extra.side, extra.line), (Side::New, 5));
        let tail = at(" fn tail() {}");
        assert_eq!((tail.side, tail.line), (Side::New, 6));
        let two = at("+two");
        assert_eq!(
            (two.path.as_str(), two.old_path, two.line),
            ("fresh.md", None, 2)
        );
        for header in [
            "diff --git a/src/a.rs b/src/a.rs",
            "--- a/src/a.rs",
            "@@ -3,3 +3,4 @@ fn top() {}",
            "rename from old_name.rs",
        ] {
            assert_eq!(target(&document, index_of(&document, header)), None);
        }
        assert_eq!(target(&document, document.len()), None);
    }

    #[test]
    fn a_renamed_file_cites_the_name_each_side_lives_under() {
        let gone = at("-gone");
        assert_eq!(gone.path, "new_name.rs");
        assert_eq!(gone.old_path.as_deref(), Some("old_name.rs"));
        assert_eq!(gone.location(), "old_name.rs:7");
        assert_eq!(at("+here").location(), "new_name.rs:7");
    }

    #[test]
    fn comments_add_edit_and_remove() {
        let review = review('c');
        let mut comments = Comments::new();
        let first = comments
            .add(&review, at("+fn answer() {}"), "  early-return here \n")
            .unwrap();
        let second = comments
            .add(&review, at("-fn old() {}"), "why was this dropped?")
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(comments.all()[0].body, "early-return here");
        assert_eq!(comments.all()[0].head, review.head);
        assert_eq!(comments.at(&at("+fn answer() {}")).count(), 1);
        assert_eq!(comments.at(&at("+fn extra() {}")).count(), 0);

        comments.edit(first, "return early").unwrap();
        assert_eq!(comments.all()[0].body, "return early");
        assert_eq!(comments.edit(first, "  "), Err(Refusal::EmptyBody));
        assert_eq!(comments.all()[0].body, "return early");
        assert_eq!(comments.edit(99, "x"), Err(Refusal::NotFound));

        assert!(comments.remove(second));
        assert!(!comments.remove(second));
        assert_eq!(comments.all().len(), 1);
        let third = comments.add(&review, at("+two"), "fine").unwrap();
        assert!(third > second, "an id is never reused");
    }

    #[test]
    fn the_set_refuses_blank_long_and_excess_comments() {
        let review = review('c');
        let mut comments = Comments::new();
        assert_eq!(
            comments.add(&review, at("+two"), " \n\t"),
            Err(Refusal::EmptyBody)
        );
        assert_eq!(
            comments.add(&review, at("+two"), &"é".repeat(MAX_BODY + 1)),
            Err(Refusal::TooLong)
        );
        assert!(
            comments
                .add(&review, at("+two"), &"é".repeat(MAX_BODY))
                .is_ok()
        );
        for _ in 1..MAX_COMMENTS {
            comments.add(&review, at("+two"), "x").unwrap();
        }
        assert_eq!(
            comments.add(&review, at("+two"), "x"),
            Err(Refusal::TooMany)
        );
        assert!(!comments.is_empty());
    }

    #[test]
    fn the_follow_up_carries_the_note_revisions_and_located_comments() {
        let review = review('c');
        let mut comments = Comments::new();
        comments
            .add(&review, at("+here"), "rename the binding\nand its callers")
            .unwrap();
        comments
            .add(&review, at("+fn answer() {}"), "early-return here")
            .unwrap();
        comments
            .add(&review, at("-gone"), "why was this dropped?")
            .unwrap();
        let follow_up = comments.follow_up(&review, "  Tighten this up.  ").unwrap();
        let expected = format!(
            "Changes requested on the review of base {} and head {}.\n\nTighten this up.\n\n{COMMENT_BLOCK_HEADER}\n- new_name.rs:7 (R): rename the binding\n  and its callers\n- old_name.rs:7 (L): why was this dropped?\n- src/a.rs:4 (R): early-return here",
            "b".repeat(40),
            "c".repeat(40),
        );
        assert_eq!(follow_up.text, expected);
        assert_eq!(follow_up.ids.len(), 3);
        assert_eq!(follow_up.outdated, 0);
    }

    #[test]
    fn a_follow_up_needs_a_note_or_a_comment() {
        let review = review('c');
        let mut comments = Comments::new();
        assert_eq!(comments.follow_up(&review, "  "), Err(Refusal::Nothing));
        let note_only = comments.follow_up(&review, "Add tests.").unwrap();
        assert!(note_only.text.ends_with("\n\nAdd tests."));
        assert!(!note_only.text.contains(COMMENT_BLOCK_HEADER));
        comments.add(&review, at("+two"), "fix").unwrap();
        let comment_only = comments.follow_up(&review, "").unwrap();
        assert!(comment_only.text.contains(&format!(
            "\n\n{COMMENT_ONLY_TEXT}\n\n{COMMENT_BLOCK_HEADER}\n- fresh.md:2 (R): fix"
        )));
    }

    #[test]
    fn comments_on_an_older_head_are_outdated_and_never_sent() {
        let older = review('c');
        let newer = review('d');
        let mut comments = Comments::new();
        let stale = comments.add(&older, at("+two"), "on the old head").unwrap();
        assert!(comments.all()[0].is_current(&older));
        assert!(!comments.all()[0].is_current(&newer));
        assert_eq!(comments.outdated(&newer), 1);
        assert_eq!(comments.follow_up(&newer, ""), Err(Refusal::Nothing));

        comments.edit(stale, "edited, still old").unwrap();
        assert_eq!(comments.outdated(&newer), 1, "editing does not re-anchor");

        let fresh = comments
            .add(&newer, at("+here"), "on the new head")
            .unwrap();
        let follow_up = comments.follow_up(&newer, "").unwrap();
        assert_eq!(follow_up.ids, vec![fresh]);
        assert_eq!(follow_up.outdated, 1);
        assert!(follow_up.text.contains(&"d".repeat(40)));
        assert!(!follow_up.text.contains("still old"));

        assert_eq!(comments.discard_outdated(&newer), 1);
        assert_eq!(comments.all().len(), 1);
    }

    #[test]
    fn a_delivered_follow_up_drops_only_what_it_carried() {
        let older = review('c');
        let newer = review('d');
        let mut comments = Comments::new();
        let kept = comments.add(&older, at("+two"), "older").unwrap();
        comments.add(&newer, at("+here"), "sent").unwrap();
        let follow_up = comments.follow_up(&newer, "").unwrap();
        let late = comments.add(&newer, at("+one"), "after writing").unwrap();
        comments.sent(&follow_up);
        let left: Vec<u64> = comments.all().iter().map(|comment| comment.id).collect();
        assert_eq!(left, vec![kept, late]);
    }
}
