//! What a local run changed, at exact revisions, for a reviewer.
//!
//! A review names three revisions: the run's base commit, its worktree's
//! `HEAD`, and the tree of the worktree's content now (tracked changes and
//! new files, as `git add -A` would stage them). The tree is written
//! through a private index, so the worktree, its index, and its refs stay
//! as they were; only Git objects are added, which Git collects when
//! nothing names them. The counts and the diff both compare the base with
//! that tree, so they describe the same change, and a later read that finds
//! another tree is how a reader learns the view is stale.
//!
//! The diff is read from Git as it streams and kept up to a bound; the
//! rest is measured up to a larger bound and dropped, so a large change
//! never holds more than the bound in memory. A cut diff, and a diff Git
//! could not produce, say so ([`Completeness`]); neither looks complete.

use std::io::Read as _;
use std::path::Path;
use std::process::Stdio;

use coder_host::access::review::{
    self as wire, Completeness, FileCount, FileStatus, Publication, TaskReview,
};

use super::local;

/// How much of a diff past its bound is measured before the read stops.
const MEASURE_MAX: u64 = 16 * 1024 * 1024;
/// The most bytes of Git's error output a reason keeps.
const REASON_MAX: usize = 400;

/// The worktree's revisions now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    /// The worktree's `HEAD` commit.
    pub commit: String,
    /// The tree of the worktree's content.
    pub tree: String,
}

/// The revisions of `worktree` now: its `HEAD` and the tree of its
/// content, written through a private index.
///
/// # Errors
/// A plain sentence when Git cannot read the worktree.
pub fn head(worktree: &Path) -> Result<Head, String> {
    let commit = local::git_out(worktree, &["rev-parse", "--verify", "HEAD^{commit}"])
        .map_err(|why| format!("Git cannot read the worktree's HEAD: {}", clip(&why)))?
        .trim()
        .to_owned();
    let index = std::env::temp_dir().join(format!(
        "openagents-review-{}-{}.index",
        std::process::id(),
        nonce()
    ));
    let result = (|| {
        let with_index = |args: &[&str]| -> Result<String, String> {
            let output = local::git()
                .arg("-C")
                .arg(coder_boundary::plain_path(worktree))
                .env("GIT_INDEX_FILE", &index)
                .args(args)
                .output()
                .map_err(|_| "cannot run git".to_owned())?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
            }
        };
        with_index(&["read-tree", &commit])?;
        with_index(&["add", "-A", "--", "."])?;
        with_index(&["write-tree"])
    })();
    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(index.with_extension("index.lock"));
    let tree = result
        .map_err(|why| format!("Git cannot read the worktree's content: {}", clip(&why)))?
        .trim()
        .to_owned();
    wire::revision(&tree).map_err(|_| "Git named no tree for the worktree.".to_owned())?;
    Ok(Head { commit, tree })
}

/// What `worktree` changed since `base`, for `task`: exact revisions, the
/// changed files with their counts, and the diff up to `max` bytes.
///
/// # Errors
/// A plain sentence when the revisions themselves cannot be read, such as
/// a worktree that is gone. A diff that cannot be read after that is a
/// review whose completeness is unknown.
pub fn read(task: &str, worktree: &Path, base: &str, max: usize) -> Result<TaskReview, String> {
    let base = local::git_out(
        worktree,
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
    )
    .map_err(|why| format!("Git cannot find the task's base: {}", clip(&why)))?
    .trim()
    .to_owned();
    let now = head(worktree)?;
    let mut review = TaskReview {
        task: task.to_owned(),
        base: base.clone(),
        head_commit: now.commit.clone(),
        head: now.tree.clone(),
        files: Vec::new(),
        files_total: 0,
        added: 0,
        removed: 0,
        uncounted: 0,
        diff: String::new(),
        completeness: Completeness::Complete,
        publication: None,
    };
    match counts(worktree, &base, &now.tree) {
        Ok(files) => {
            review.files_total = files.len() as u64;
            for file in &files {
                match (file.added, file.removed) {
                    (Some(added), Some(removed)) => {
                        review.added += added;
                        review.removed += removed;
                    }
                    _ => review.uncounted += 1,
                }
            }
            review.files = files;
        }
        Err(why) => {
            review.completeness = Completeness::Unknown { reason: why };
            return Ok(review);
        }
    }
    match bounded_diff(worktree, &base, &now.tree, max) {
        Ok((diff, completeness)) => {
            review.diff = diff;
            review.completeness = completeness;
        }
        Err(why) => review.completeness = Completeness::Unknown { reason: why },
    }
    Ok(review)
}

/// [`read`] for the wire: the diff and the file rows shortened, in that
/// order, until the outcome fits a relay frame, and the task's last
/// publication attached.
///
/// # Errors
/// As [`read`].
pub fn read_for_wire(
    store: &Path,
    task: &str,
    worktree: &Path,
    base: &str,
) -> Result<TaskReview, String> {
    let mut review = read(task, worktree, base, wire::MAX_DIFF)?;
    review.publication = super::publish::last(store, task);
    fit(&mut review);
    Ok(review)
}

/// Shorten `review` until its encoding fits [`wire::MAX_REVIEW_BYTES`]:
/// the diff first, halved at a line each time and marked cut, then the
/// file rows past the first ones. The counts stay whole.
pub fn fit(review: &mut TaskReview) {
    if review.diff.len() > wire::MAX_DIFF {
        cut(review, wire::MAX_DIFF);
    }
    if review.files.len() > wire::MAX_FILES {
        review.files.truncate(wire::MAX_FILES);
    }
    for _ in 0..64 {
        let size = serde_json::to_vec(review).map_or(usize::MAX, |bytes| bytes.len());
        if size <= wire::MAX_REVIEW_BYTES {
            return;
        }
        if !review.diff.is_empty() {
            cut(review, review.diff.len() / 2);
        } else if !review.files.is_empty() {
            let keep = review.files.len() / 2;
            review.files.truncate(keep);
        } else {
            return;
        }
    }
}

fn cut(review: &mut TaskReview, max: usize) {
    let total = match &review.completeness {
        Completeness::Complete => Some(review.diff.len() as u64),
        Completeness::Truncated { total, .. } => *total,
        Completeness::Unknown { .. } => return,
    };
    let end = line_end(review.diff.as_bytes(), max);
    review.diff.truncate(end);
    review.completeness = Completeness::Truncated {
        shown: review.diff.len() as u64,
        total,
    };
}

/// The changed files between `base` and `tree`, path order.
fn counts(worktree: &Path, base: &str, tree: &str) -> Result<Vec<FileCount>, String> {
    let statuses = local::git_out(
        worktree,
        &[
            "-c",
            "core.quotepath=off",
            "diff",
            "--name-status",
            "--no-renames",
            "-z",
            base,
            tree,
        ],
    )
    .map_err(|why| format!("Git cannot list the changed files: {}", clip(&why)))?;
    let mut status = std::collections::BTreeMap::new();
    let mut fields = statuses.split('\0').filter(|field| !field.is_empty());
    while let (Some(code), Some(path)) = (fields.next(), fields.next()) {
        let kind = match code.as_bytes().first() {
            Some(b'A') => FileStatus::Added,
            Some(b'D') => FileStatus::Deleted,
            Some(b'T') => FileStatus::TypeChanged,
            _ => FileStatus::Modified,
        };
        status.insert(path.to_owned(), kind);
    }
    let numstat = local::git_out(
        worktree,
        &[
            "-c",
            "core.quotepath=off",
            "diff",
            "--numstat",
            "--no-renames",
            "-z",
            base,
            tree,
        ],
    )
    .map_err(|why| format!("Git cannot count the changed lines: {}", clip(&why)))?;
    let mut files = Vec::new();
    for record in numstat.split('\0').filter(|record| !record.is_empty()) {
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        files.push(FileCount {
            path: path.to_owned(),
            status: status.get(path).copied().unwrap_or(FileStatus::Modified),
            // Git writes `-` for a file whose lines it does not count.
            added: added.parse().ok(),
            removed: removed.parse().ok(),
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// The most bytes of a diff read to give a result's files their patches.
const PATCH_BYTES: usize = 512 * 1024;

/// The unified diff from `base` to `to` in `worktree`, or to the
/// worktree's content now (new files included) when `to` is `None`,
/// bounded, for [`coder_events::attach_patches`]. Empty when Git cannot
/// produce it.
///
/// [`coder_events::attach_patches`]: openagents_chat::coder_events::attach_patches
pub(crate) fn patch(worktree: &Path, base: &str, to: Option<&str>) -> String {
    let tree = match to {
        Some(to) => to.to_owned(),
        None => match head(worktree) {
            Ok(head) => head.tree,
            Err(_) => return String::new(),
        },
    };
    bounded_diff(worktree, base, &tree, PATCH_BYTES)
        .map(|(diff, _)| diff)
        .unwrap_or_default()
}

/// The unified diff from `base` to `tree`, kept up to `max` bytes as it
/// streams and cut at a line.
fn bounded_diff(
    worktree: &Path,
    base: &str,
    tree: &str,
    max: usize,
) -> Result<(String, Completeness), String> {
    let mut child = local::git()
        .arg("-C")
        .arg(coder_boundary::plain_path(worktree))
        .args([
            "-c",
            "core.quotepath=off",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            base,
            tree,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "cannot run git".to_owned())?;
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stderr.read(&mut chunk) {
            if read == 0 {
                break;
            }
            if kept.len() < 4096 {
                kept.extend_from_slice(&chunk[..read]);
            }
        }
        String::from_utf8_lossy(&kept).trim().to_owned()
    });
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut kept: Vec<u8> = Vec::with_capacity(max.min(1024 * 1024) + 1);
    let mut total: u64 = 0;
    let mut chunk = [0u8; 64 * 1024];
    let mut stopped = false;
    loop {
        let read = match stdout.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                stopped = true;
                break;
            }
        };
        total += read as u64;
        let room = (max + 1).saturating_sub(kept.len());
        kept.extend_from_slice(&chunk[..read.min(room)]);
        if total > MEASURE_MAX {
            stopped = true;
            break;
        }
    }
    drop(stdout);
    if stopped {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|_| "Git did not finish the diff.".to_owned())?;
    let errors = errors.join().unwrap_or_default();
    if !stopped && !status.success() {
        return Err(format!("Git could not write the diff: {}", clip(&errors)));
    }
    let whole = !stopped && total as usize <= max && kept.len() as u64 == total;
    if whole {
        let text = String::from_utf8_lossy(&kept).into_owned();
        if text.len() <= max {
            return Ok((text, Completeness::Complete));
        }
    }
    let end = line_end(&kept, max);
    let mut text = String::from_utf8_lossy(&kept[..end]).into_owned();
    // Replacement characters can lengthen the text; cut again at a line.
    if text.len() > max {
        let again = line_end(text.as_bytes(), max);
        text.truncate(again);
    }
    Ok((
        text.clone(),
        Completeness::Truncated {
            shown: text.len() as u64,
            total: (!stopped).then_some(total),
        },
    ))
}

/// The end of the last whole line within `max` bytes of `bytes`, or 0.
fn line_end(bytes: &[u8], max: usize) -> usize {
    let limit = max.min(bytes.len());
    if limit == bytes.len() && bytes.last() == Some(&b'\n') {
        return limit;
    }
    bytes[..limit]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1)
}

fn clip(text: &str) -> String {
    let mut end = text.len().min(REASON_MAX);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn nonce() -> String {
    let bytes: [u8; 8] = secp256k1::rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether `publication` is of the review now shown: the same revisions.
#[must_use]
pub fn publishes(publication: &Publication, review: &TaskReview) -> bool {
    publication.base == review.base
        && publication.head_commit == review.head_commit
        && publication.head == review.head
}
