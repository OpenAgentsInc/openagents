//! The serialized queue for single-digest artifacts (#10763).
//!
//! Some files in the repository are one digest over many sources: the
//! Everglade pack, the Grid pack's fingerprint. Two agents that each change
//! a source and repin race, and the second push has to rebase and repin
//! again. The queue serializes those changes the way a merge queue does:
//!
//! 1. An agent submits a branch ([`submit`]). The submission is a record,
//!    `artifacts/<name>/queue/<id>.json` under the lease root, and a ref,
//!    `refs/artifact-queue/<name>/<id>`, that keeps the commit alive.
//! 2. Whoever takes the exclusive `artifact/<name>` lease runs the queue
//!    ([`run`]): it fetches `origin/main` into a scratch worktree, applies
//!    every pending change in order with the artifact's pinned files left
//!    out, regenerates the pin once for the batch, runs the artifact's
//!    check, commits the repin, and pushes.
//! 3. A change that conflicts or fails the check is rejected with its
//!    reason, and the rest of the batch continues.
//!
//! The registry, one `artifacts/<name>.json` per artifact at the top of the
//! repository ([`Artifact`]), names each artifact's regenerate and check
//! commands, its pinned files, and the lines of source code that hold its
//! pin. `docs/coder/runtime/artifact-queue.md` is the operator's page.

mod pin;
mod run;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use pin::{apply_pin, reset_pin};
pub use run::{Options, Outcome, git_common_dir, run, submit};

/// The schema of a registry entry, [`Artifact`].
pub const ARTIFACT_SCHEMA: &str = "openagents.artifact.v1";
/// The schema of a queue record, [`Submission`].
pub const SUBMISSION_SCHEMA: &str = "openagents.artifact.submission.v1";
/// The registry directory at the top of the repository.
pub const REGISTRY_DIR: &str = "artifacts";
/// Where the queue keeps each submission's commit alive.
pub const REF_PREFIX: &str = "refs/artifact-queue";

/// One queue-managed artifact, as `artifacts/<name>.json` declares it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// [`ARTIFACT_SCHEMA`].
    pub schema: String,
    /// The artifact's name, the file's stem, and the lease's
    /// `artifact/<name>`.
    pub name: String,
    /// What it is, for people.
    pub description: String,
    /// The shell command, run at the top of the repository, that rebuilds
    /// the artifact from its sources and prints the pin lines.
    pub regenerate: String,
    /// The shell command that fails unless the sources build the pinned
    /// artifact.
    pub check: String,
    /// Whether the two commands run under a `build` lease.
    #[serde(default)]
    pub build_lease: bool,
    /// Path patterns (`*` matches within one path segment) of files the
    /// regenerate command writes. A submitted change's edits to them are
    /// left out; the queue writes them once per batch.
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Source lines that hold the pin, when the pin lives in code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pin>,
    /// The repin commit's subject; `{changes}` becomes the changes'
    /// summaries.
    pub message: String,
}

/// The lines of one source file that hold an artifact's pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The file, relative to the top of the repository.
    pub file: String,
    /// Line prefixes, such as `pub const PACK_SHA256: &str = `. The
    /// regenerate command prints one line with each prefix, and that line
    /// replaces the file's line with the same prefix.
    pub lines: Vec<String>,
    /// Where earlier digests are kept, when the file keeps them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<History>,
}

/// A list of earlier digests in a pin file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// The pin line whose quoted value moves into the history when it
    /// changes; one of [`Pin::lines`].
    pub from: String,
    /// The line after which the earlier value is inserted.
    pub after: String,
    /// The prefix of the line that ends the list.
    pub end: String,
}

impl Artifact {
    /// Reads `artifacts/<name>.json` under `top`.
    ///
    /// # Errors
    /// A sentence when the file is missing, unreadable, or not an entry
    /// for `name`.
    pub fn load(top: &Path, name: &str) -> Result<Artifact, String> {
        if !crate::resource::valid_name(name) {
            return Err(format!(
                "`{name}` is not an artifact name; use letters, digits, `.`, `_`, and `-`"
            ));
        }
        let path = top.join(REGISTRY_DIR).join(format!("{name}.json"));
        let bytes = std::fs::read(&path).map_err(|error| {
            format!(
                "{name} is not a queue-managed artifact here ({}: {error}); the registry is {REGISTRY_DIR}/",
                path.display()
            )
        })?;
        let artifact: Artifact = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if artifact.schema != ARTIFACT_SCHEMA {
            return Err(format!(
                "{}: the schema is {}, not {ARTIFACT_SCHEMA}",
                path.display(),
                artifact.schema
            ));
        }
        if artifact.name != name {
            return Err(format!(
                "{}: it names {}, not {name}",
                path.display(),
                artifact.name
            ));
        }
        if let Some(pin) = &artifact.pin
            && let Some(history) = &pin.history
            && !pin.lines.contains(&history.from)
        {
            return Err(format!(
                "{}: the history's `from` is not one of the pin's lines",
                path.display()
            ));
        }
        Ok(artifact)
    }

    /// Whether `path` is one of the artifact's pinned files.
    #[must_use]
    pub fn pinned(&self, path: &str) -> bool {
        self.pinned.iter().any(|pattern| matches(pattern, path))
    }
}

/// Whether `path` matches `pattern`, where `*` matches any run of
/// characters within one path segment and `?` one character.
#[must_use]
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    pattern.len() == path.len()
        && pattern
            .iter()
            .zip(&path)
            .all(|(pattern, part)| segment(pattern.as_bytes(), part.as_bytes()))
}

fn segment(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|skip| segment(rest, &text[skip..])),
        Some((b'?', rest)) => !text.is_empty() && segment(rest, &text[1..]),
        Some((byte, rest)) => text.first() == Some(byte) && segment(rest, &text[1..]),
    }
}

/// Where a submission stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Waiting for the queue to apply it.
    Pending,
    /// Applied, repinned, checked, and pushed.
    Landed,
    /// Sent back to its submitter with a reason.
    Rejected,
}

impl Status {
    /// The status as the records spell it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Landed => "landed",
            Status::Rejected => "rejected",
        }
    }
}

/// One change submitted to an artifact's queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    /// [`SUBMISSION_SCHEMA`].
    pub schema: String,
    /// The submission's identifier, which sorts in submission order.
    pub id: String,
    /// The artifact's name.
    pub artifact: String,
    /// The submitter's Git directory (the common directory of its
    /// worktrees).
    pub repository: PathBuf,
    /// The branch as submitted.
    pub branch: String,
    /// The commit the branch named when it was submitted.
    pub commit: String,
    /// The ref in the submitter's repository that keeps the commit alive.
    pub git_ref: String,
    /// A few words for the repin commit: what the change adds.
    pub summary: String,
    /// The submitting agent session.
    pub session: String,
    /// When it was submitted, in Unix milliseconds.
    pub submitted_at_ms: u64,
    /// Where it stands.
    pub status: Status,
    /// Why it was rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The commit on `main` that carries it, once landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed: Option<String>,
    /// When it landed or was rejected, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<u64>,
}

/// One artifact's queue under a lease root:
/// `artifacts/<name>/queue/<id>.json`.
#[derive(Clone, Debug)]
pub struct Queue {
    dir: PathBuf,
    name: String,
}

impl Queue {
    /// The queue of artifact `name` under the lease root `root`.
    #[must_use]
    pub fn new(root: &Path, name: &str) -> Queue {
        crate::refuse_real_home(root);
        Queue {
            dir: root.join(REGISTRY_DIR).join(name),
            name: name.to_owned(),
        }
    }

    /// The artifact's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The artifact's directory under the lease root, which holds the
    /// queue, the scratch worktree, and its build output.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn records(&self) -> PathBuf {
        self.dir.join("queue")
    }

    /// Writes `submission`.
    ///
    /// # Errors
    /// The record can't be written.
    pub fn save(&self, submission: &Submission) -> Result<(), String> {
        let dir = self.records();
        std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        let path = dir.join(format!("{}.json", submission.id));
        let bytes = serde_json::to_vec_pretty(submission).map_err(|error| error.to_string())?;
        crate::table::write_atomic(&path, &bytes)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Every submission, oldest first.
    ///
    /// # Errors
    /// The queue directory or a record can't be read.
    pub fn list(&self) -> Result<Vec<Submission>, String> {
        let dir = self.records();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("{}: {error}", dir.display())),
        };
        let mut all = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let submission: Submission = serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            all.push(submission);
        }
        all.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(all)
    }

    /// The pending submissions, oldest first.
    ///
    /// # Errors
    /// As [`Queue::list`].
    pub fn pending(&self) -> Result<Vec<Submission>, String> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|submission| submission.status == Status::Pending)
            .collect())
    }

    /// One submission by identifier.
    ///
    /// # Errors
    /// As [`Queue::list`].
    pub fn get(&self, id: &str) -> Result<Option<Submission>, String> {
        Ok(self
            .list()?
            .into_iter()
            .find(|submission| submission.id == id))
    }
}

/// Every artifact queue under the lease root, by name.
///
/// # Errors
/// The artifacts directory can't be read.
pub fn queues(root: &Path) -> Result<Vec<Queue>, String> {
    crate::refuse_real_home(root);
    let dir = root.join(REGISTRY_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().join("queue").is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    names.sort();
    Ok(names.iter().map(|name| Queue::new(root, name)).collect())
}

/// A new submission identifier: nanoseconds since the epoch and this
/// process, so identifiers sort in submission order.
#[must_use]
pub fn new_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{nanos:020}-{}-{n}", std::process::id())
}

/// Joins summaries as a sentence lists them: `a`, `a and b`, `a, b, and c`.
#[must_use]
pub fn join_summaries(summaries: &[String]) -> String {
    match summaries {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests;
