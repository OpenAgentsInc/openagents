//! A Coder task's change as a reviewer reads it, and its publication
//! (NIP-HOST `task.review` and `task.publish`).
//!
//! A review names the exact revisions it shows: the base commit the task
//! started from, the worktree's `HEAD` commit, and the tree of the
//! worktree's content (tracked changes and new files, as Git would commit
//! them) when the review was read. The changed-file counts are Git's own
//! for that base and tree, so they stay complete when the diff text is cut.
//! A diff that was cut, or could not be read at all, says so
//! ([`Completeness`]); it is never shown as complete.
//!
//! A publication is a host-owned repository operation over a reviewed
//! head: it commits exactly the reviewed tree and pushes it (to the
//! repository's branch, or to a branch of its own with a draft pull
//! request), once. Its operation identity derives from the task and the
//! reviewed revisions, so a retry is the same operation: the host
//! reconciles an uncertain push by reading the remote, never by pushing
//! twice. A publication of a head the worktree has moved past is refused.
//! These types are the wire form only; the task owner makes them.
use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};

/// The most bytes of diff text a review carries over the wire.
pub const MAX_DIFF: usize = 32 * 1024;
/// The most file rows a review carries; [`TaskReview::files_total`] keeps
/// the true count.
pub const MAX_FILES: usize = 200;
/// The longest path a file row carries.
pub const MAX_PATH: usize = 512;
/// The largest encoded review outcome, inside one relay frame.
pub const MAX_REVIEW_BYTES: usize = 48 * 1024;
/// The longest note or reason a review or publication carries.
pub const MAX_NOTE: usize = 1024;
/// The longest link a publication carries.
pub const MAX_URL: usize = 512;

/// How a file changed between the base and the reviewed tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// Its type changed, such as a file becoming a link.
    TypeChanged,
}

/// One changed file and its line counts. A count is `None` when Git counts
/// no lines, as for a binary file: unknown, never zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCount {
    pub path: String,
    pub status: FileStatus,
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

/// Whether the diff text is the whole change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Completeness {
    /// The diff text is the whole change between the two revisions.
    Complete,
    /// The diff text stops at a line boundary after `shown` bytes. `total`
    /// is the whole diff's size when it was measured, `None` when the read
    /// stopped before its end. The file counts are still complete.
    Truncated { shown: u64, total: Option<u64> },
    /// The change could not be read; `reason` says why. Nothing about the
    /// change is known beyond what the review names.
    Unknown { reason: String },
}

/// What a reviewer reads: the change between two exact revisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskReview {
    pub task: String,
    /// The commit the task started from.
    pub base: String,
    /// The worktree's `HEAD` commit when the review was read.
    pub head_commit: String,
    /// The tree of the worktree's content when the review was read: the
    /// head a publication commits.
    pub head: String,
    /// The changed files, path order, at most [`MAX_FILES`].
    pub files: Vec<FileCount>,
    /// How many files changed in all.
    pub files_total: u64,
    /// Lines added and removed over every counted file.
    pub added: u64,
    pub removed: u64,
    /// Files whose lines Git does not count, such as binary files.
    pub uncounted: u64,
    /// The unified diff from `base` to `head`, as far as it was read.
    pub diff: String,
    pub completeness: Completeness,
    /// The last publication of this task, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<Publication>,
}

/// Where a publication lands, by the repository's policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Landing {
    /// Pushed onto the repository's branch, fast-forward only.
    Branch,
    /// Pushed to a branch of its own with a draft pull request.
    DraftPullRequest,
}

/// Where a publication is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishState {
    /// The commit is on the remote, and the pull request is open when the
    /// landing asks for one.
    Published,
    /// The commit is on the remote, but the pull request was not opened;
    /// `note` says why. Publishing again opens it without pushing again.
    Pushed,
    /// The push's result is unknown, such as a push that timed out.
    /// Publishing again reads the remote first and pushes only if the
    /// commit is not there.
    Uncertain,
    /// Nothing was published; `note` says why. A refusal changes nothing
    /// on the remote.
    Refused,
}

/// A publication of a reviewed head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    /// The operation's identity: 64 lowercase hex characters, derived from
    /// the task and the reviewed revisions.
    pub operation: String,
    pub task: String,
    /// The reviewed revisions: the review identity.
    pub base: String,
    pub head_commit: String,
    pub head: String,
    pub landing: Landing,
    pub state: PublishState,
    /// The remote branch pushed to, once chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit of the reviewed tree, once made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The pull request, or the commit on the forge, when there is a link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// What happened, in a sentence.
    pub note: String,
}

/// A Git object ID: 40 or 64 lowercase hex characters.
///
/// # Errors
/// Anything else.
pub fn revision(id: &str) -> Result<()> {
    if !matches!(id.len(), 40 | 64)
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return fail(Code::Malformed, "revision must be a Git object ID");
    }
    Ok(())
}

fn note(text: &str) -> Result<()> {
    if text.len() > MAX_NOTE {
        return fail(Code::Bounds, "note exceeds its bound");
    }
    Ok(())
}

impl TaskReview {
    /// Check the review's own bounds.
    ///
    /// # Errors
    /// A field outside its bound or form.
    pub fn validate(&self) -> Result<()> {
        crate::protocol::identity(&self.task).map_err(crate::Error::from)?;
        revision(&self.base)?;
        revision(&self.head_commit)?;
        revision(&self.head)?;
        if self.files.len() > MAX_FILES || (self.files.len() as u64) > self.files_total {
            return fail(Code::Bounds, "too many file rows");
        }
        for file in &self.files {
            if file.path.is_empty()
                || file.path.len() > MAX_PATH
                || file.path.chars().any(char::is_control)
            {
                return fail(Code::Bounds, "file path exceeds its bound");
            }
        }
        if self.diff.len() > MAX_DIFF {
            return fail(Code::Bounds, "diff exceeds its bound");
        }
        match &self.completeness {
            Completeness::Complete => {}
            Completeness::Truncated { shown, total } => {
                if *shown != self.diff.len() as u64 || total.is_some_and(|total| total < *shown) {
                    return fail(Code::Malformed, "truncation does not match the diff");
                }
            }
            Completeness::Unknown { reason } => note(reason)?,
        }
        if let Some(publication) = &self.publication {
            publication.validate()?;
            if publication.task != self.task {
                return fail(Code::Malformed, "publication names another task");
            }
        }
        Ok(())
    }

    /// Whether `other` shows the same revisions.
    #[must_use]
    pub fn same_head(&self, other: &TaskReview) -> bool {
        self.base == other.base && self.head_commit == other.head_commit && self.head == other.head
    }
}

impl Publication {
    /// Check the publication's own bounds.
    ///
    /// # Errors
    /// A field outside its bound or form.
    pub fn validate(&self) -> Result<()> {
        crate::protocol::identity(&self.operation).map_err(crate::Error::from)?;
        crate::protocol::identity(&self.task).map_err(crate::Error::from)?;
        revision(&self.base)?;
        revision(&self.head_commit)?;
        revision(&self.head)?;
        if let Some(commit) = &self.commit {
            revision(commit)?;
        }
        if let Some(branch) = &self.branch
            && (branch.is_empty() || branch.len() > 255 || branch.chars().any(char::is_control))
        {
            return fail(Code::Bounds, "branch exceeds its bound");
        }
        if let Some(url) = &self.url
            && (url.len() > MAX_URL || !url.starts_with("https://") || !url.is_ascii())
        {
            return fail(Code::Malformed, "publication link must be an https URL");
        }
        note(&self.note)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review() -> TaskReview {
        TaskReview {
            task: "a".repeat(64),
            base: "b".repeat(40),
            head_commit: "b".repeat(40),
            head: "c".repeat(40),
            files: vec![FileCount {
                path: "src/a.rs".into(),
                status: FileStatus::Modified,
                added: Some(2),
                removed: Some(1),
            }],
            files_total: 1,
            added: 2,
            removed: 1,
            uncounted: 0,
            diff: "diff --git a/src/a.rs b/src/a.rs\n".into(),
            completeness: Completeness::Complete,
            publication: None,
        }
    }

    #[test]
    fn a_review_names_exact_revisions_and_honest_truncation() {
        let mut review = review();
        review.validate().unwrap();
        review.completeness = Completeness::Truncated {
            shown: review.diff.len() as u64,
            total: Some(4_000),
        };
        review.validate().unwrap();
        // A truncation that does not match the text it describes refuses.
        review.completeness = Completeness::Truncated {
            shown: 3,
            total: None,
        };
        assert!(review.validate().is_err());
        let mut short = self::tests::review();
        short.head = "c".repeat(12);
        assert!(short.validate().is_err());
        let mut many = self::tests::review();
        many.files_total = 0;
        assert!(many.validate().is_err());
    }

    #[test]
    fn a_publication_link_is_https_only() {
        let mut publication = Publication {
            operation: "d".repeat(64),
            task: "a".repeat(64),
            base: "b".repeat(40),
            head_commit: "b".repeat(40),
            head: "c".repeat(40),
            landing: Landing::DraftPullRequest,
            state: PublishState::Published,
            branch: Some("coder/review-aaaaaaaa".into()),
            commit: Some("e".repeat(40)),
            url: Some("https://github.com/o/r/pull/1".into()),
            note: "Opened a draft pull request.".into(),
        };
        publication.validate().unwrap();
        publication.url = Some("javascript:alert(1)".into());
        assert!(publication.validate().is_err());
        let text = serde_json::to_string(&PublishState::Uncertain).unwrap();
        assert_eq!(text, "\"uncertain\"");
    }
}
