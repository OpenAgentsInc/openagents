//! The issue-flow evaluation set: past issues from this repository, each
//! pinned to the commit before its fix and graded like a mini-task.
//!
//! Every change to the issue flow ([`crate::issue_turn`]) used to be fitted
//! on one issue, #9597. This set measures a change on more than one. Each
//! entry freezes an issue's title and body as they were before the fix,
//! pins the base commit (the fix's parent) and the fix, and names a grader:
//! the tests the real fix added or changed, run against the candidate, and
//! checks for the issue's stated deliverables. The set lives in
//! `crates/coder-one/issues-eval/`:
//!
//! ```text
//! manifest.json            the split and every file's SHA-256
//! entries/<id>.json        one entry: the issue, the commits, the checks
//! hidden/<file>            test code a check places in the candidate
//! ```
//!
//! [`run::run`] works an entry through the issue flow in a scratch clone
//! at the base commit, publishing nothing, then grades the result;
//! [`grade::grade`] grades any checkout, which is how the set proves its
//! graders discriminate: the base fails and the real fix passes.

pub mod cli;
pub mod grade;
pub mod run;
pub mod sealed;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The set manifest's schema.
pub const SET_SCHEMA: &str = "openagents.coder-one.issue-eval-set.v1";
/// One entry's schema.
pub const ENTRY_SCHEMA: &str = "openagents.coder-one.issue-eval-entry.v1";

/// The set's directory in this repository.
#[must_use]
pub fn default_set_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("issues-eval")
}

/// Which part of the set an entry belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    /// Entries an issue-flow change may be developed and tuned on.
    Development,
    /// Entries only for confirming a change after it is chosen.
    HeldOut,
}

impl Part {
    /// The part's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Part::Development => "development",
            Part::HeldOut => "held-out",
        }
    }
}

/// The issue as it was before the fix.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrozenIssue {
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub created_at: String,
    /// How the text is known to predate the fix.
    pub frozen: String,
}

/// The real fix.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fix {
    /// The commits that fixed the issue, oldest first.
    pub commits: Vec<String>,
    /// The commit that holds the whole fix; graders must pass there.
    pub head: String,
}

/// One grader check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    /// `fix_tests` for the tests the real fix added or changed, or
    /// `deliverables` for what the issue says must exist or change.
    pub group: String,
    #[serde(flatten)]
    pub kind: Kind,
    /// What the check stands for, in a sentence.
    #[serde(default)]
    pub note: String,
}

/// What a check does.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// Every changed path matches one of `patterns`.
    OnlyChanged { patterns: Vec<String> },
    /// Some changed path matches `pattern`.
    Changed { pattern: String },
    /// `path` exists and matches `pattern`.
    Contains { path: String, pattern: String },
    /// `path` exists and does not match `pattern`.
    Lacks { path: String, pattern: String },
    /// Some changed file whose path matches `paths` matches `pattern`.
    ChangedContains { paths: String, pattern: String },
    /// Every relative Markdown link in `path` points at a file that
    /// exists, and at a heading that file has.
    Links { path: String },
    /// `run` exits 0 under `sh -c` in the checkout within
    /// `timeout_secs`, after `files` are placed and `inject` is spliced
    /// in; both are undone afterwards. With `tests`, the output must also
    /// report at least one passing test and no failing one. Every regex
    /// in `expect` must match the output.
    Command {
        run: String,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
        #[serde(default)]
        tests: bool,
        #[serde(default)]
        expect: Vec<String>,
        /// Checkout path to set file under `hidden/`.
        #[serde(default)]
        files: std::collections::BTreeMap<String, String>,
        #[serde(default)]
        inject: Option<Inject>,
    },
}

/// Test code spliced into a source file for one check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inject {
    /// The source file in the checkout.
    pub path: String,
    /// The snippet, under `hidden/`.
    pub snippet: String,
    /// The line the snippet goes after, trimmed, such as `mod tests {`.
    /// When no line matches, the snippet is appended in a test module of
    /// its own that uses `super::*`.
    pub after: String,
}

fn default_timeout() -> u64 {
    1_800
}

/// One entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub schema: String,
    pub id: String,
    /// `docs`, `gym-view`, `rust-behavior`, or `cli`, joined with `+`
    /// when an issue spans two.
    pub category: String,
    pub issue: FrozenIssue,
    /// The fix's parent: the scratch clone starts here.
    pub base: String,
    pub fix: Fix,
    pub checks: Vec<Check>,
    /// Anything a reader of results should know about the entry.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Entry {
    /// The request the issue flow works: the issue as a heading and its
    /// body. The issue's URL is left out, so the loop doesn't read the
    /// closed issue and the fix it links.
    #[must_use]
    pub fn request(&self) -> String {
        format!(
            "# Issue #{}: {}\n\n{}",
            self.issue.number,
            self.issue.title,
            self.issue.body.trim()
        )
    }
}

/// A loaded, digest-checked set.
#[derive(Clone, Debug)]
pub struct Set {
    pub dir: PathBuf,
    /// The set's digest: SHA-256 over every file's name and digest.
    pub digest: String,
    pub entries: Vec<(Entry, Part, String)>,
}

impl Set {
    /// The entry `id`, its part, and its digest.
    ///
    /// # Errors
    ///
    /// Returns a message naming the known entries when none matches.
    pub fn find(&self, id: &str) -> Result<&(Entry, Part, String), String> {
        self.entries
            .iter()
            .find(|(entry, _, _)| {
                entry.id == id || entry.issue.number.to_string() == id.trim_start_matches('#')
            })
            .ok_or_else(|| {
                format!(
                    "no entry {id}; known: {}",
                    self.entries
                        .iter()
                        .map(|(entry, _, _)| entry.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    /// The path of a file under `hidden/`.
    #[must_use]
    pub fn hidden(&self, name: &str) -> PathBuf {
        self.dir.join("hidden").join(name)
    }
}

/// The SHA-256 of `bytes`, in hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Every file the manifest digests: entries and hidden files, relative to
/// the set's directory, sorted.
fn files(dir: &Path) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    for sub in ["entries", "hidden"] {
        let read = std::fs::read_dir(dir.join(sub))
            .map_err(|error| format!("cannot read {}/{sub}: {error}", dir.display()))?;
        for item in read.flatten() {
            if item.path().is_file() {
                found.push(format!("{sub}/{}", item.file_name().to_string_lossy()));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The digest of every file under `dir`, and the set's digest over them.
///
/// # Errors
///
/// Returns a message when a file can't be read.
pub fn digests(dir: &Path) -> Result<(Vec<(String, String)>, String), String> {
    let mut pairs = Vec::new();
    for name in files(dir)? {
        let bytes = std::fs::read(dir.join(&name))
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        pairs.push((name, sha256(&bytes)));
    }
    let listing: String = pairs
        .iter()
        .map(|(name, digest)| format!("{name} {digest}\n"))
        .collect();
    let digest = sha256(listing.as_bytes());
    Ok((pairs, digest))
}

/// Loads the set under `dir` and checks every digest the manifest records.
///
/// # Errors
///
/// Returns a message when the manifest is missing or malformed, a file
/// isn't the one the manifest digested, or an entry is in no part.
pub fn load(dir: &Path) -> Result<Set, String> {
    let path = dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    if manifest["schema"].as_str() != Some(SET_SCHEMA) {
        return Err(format!("{} is not a {SET_SCHEMA} manifest", path.display()));
    }
    let (pairs, digest) = digests(dir)?;
    let recorded = manifest["files"].as_object().cloned().unwrap_or_default();
    for (name, sha) in &pairs {
        match recorded.get(name).and_then(Value::as_str) {
            Some(want) if want == sha => {}
            Some(_) => {
                return Err(format!(
                    "{name} changed since the manifest was sealed; run `coder-one issue-eval seal` \
                     only when the change is meant, and never after measuring on held-out entries"
                ));
            }
            None => return Err(format!("{name} is not in the manifest")),
        }
    }
    if let Some(missing) = recorded
        .keys()
        .find(|name| !pairs.iter().any(|(n, _)| n == *name))
    {
        return Err(format!("the manifest names {missing}, which is missing"));
    }
    if manifest["digest"].as_str() != Some(digest.as_str()) {
        return Err("the manifest's set digest doesn't match its files".to_string());
    }
    let part_of = |id: &str| -> Option<Part> {
        let listed = |key: &str| {
            manifest["split"][key]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(id)))
        };
        match (listed("development"), listed("held-out")) {
            (true, false) => Some(Part::Development),
            (false, true) => Some(Part::HeldOut),
            _ => None,
        }
    };
    let mut entries = Vec::new();
    for (name, sha) in pairs.iter().filter(|(n, _)| n.starts_with("entries/")) {
        let text = std::fs::read_to_string(dir.join(name))
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        let entry: Entry =
            serde_json::from_str(&text).map_err(|error| format!("{name}: {error}"))?;
        if entry.schema != ENTRY_SCHEMA {
            return Err(format!("{name} is not a {ENTRY_SCHEMA} entry"));
        }
        let part = part_of(&entry.id)
            .ok_or_else(|| format!("{} is in neither part, or in both", entry.id))?;
        entries.push((entry, part, sha.clone()));
    }
    Ok(Set {
        dir: dir.to_path_buf(),
        digest,
        entries,
    })
}

/// Rewrites the manifest's digests for the files under `dir`, keeping its
/// split. Returns the new set digest.
///
/// # Errors
///
/// Returns a message when the manifest can't be read or written.
pub fn seal(dir: &Path) -> Result<String, String> {
    let path = dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    let (pairs, digest) = digests(dir)?;
    manifest["files"] = pairs
        .iter()
        .map(|(name, sha)| (name.clone(), json!(sha)))
        .collect::<serde_json::Map<_, _>>()
        .into();
    manifest["digest"] = json!(digest);
    let mut text = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
    text.push('\n');
    std::fs::write(&path, text)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(digest)
}

/// Runs `program` in `dir` and returns its standard output.
pub(crate) fn run_in(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Makes `dir` a fresh repository at `commit`, fetched from `source` one
/// commit deep, so the checkout holds no later history: `git log` can't
/// show the fix. With `then`, that commit is fetched and checked out
/// too, and the base's tree stays available to diff against.
///
/// # Errors
///
/// Returns a message when `git` fails.
pub fn checkout(source: &Path, commit: &str, then: Option<&str>, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    let url = format!(
        "file://{}",
        source
            .canonicalize()
            .map_err(|error| format!("cannot resolve {}: {error}", source.display()))?
            .display()
    );
    run_in(dir, "git", &["init", "-q"])?;
    run_in(dir, "git", &["fetch", "-q", "--depth=1", &url, commit])?;
    run_in(dir, "git", &["checkout", "-q", "--detach", commit])?;
    if let Some(then) = then {
        run_in(dir, "git", &["fetch", "-q", "--depth=1", &url, then])?;
        run_in(dir, "git", &["checkout", "-q", "--detach", then])?;
    }
    Ok(())
}

/// The top of the git checkout that holds `dir`.
///
/// # Errors
///
/// Returns a message when `dir` isn't in a git checkout.
pub fn source_of(dir: &Path) -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        run_in(dir, "git", &["rev-parse", "--show-toplevel"])?.trim(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_in_this_repository_loads_with_its_digests_and_split() {
        let set = load(&default_set_dir()).unwrap();
        assert!(
            (6..=10).contains(&set.entries.len()),
            "{}",
            set.entries.len()
        );
        let held: Vec<&Entry> = set
            .entries
            .iter()
            .filter(|(_, part, _)| *part == Part::HeldOut)
            .map(|(entry, _, _)| entry)
            .collect();
        assert!(!held.is_empty());
        // #9597 was tuned on for 14 attempts: it may only be development.
        assert!(held.iter().all(|entry| entry.issue.number != 9597));
        for category in ["docs", "gym-view", "rust-behavior", "cli"] {
            assert!(
                set.entries
                    .iter()
                    .any(|(entry, _, _)| entry.category.split('+').any(|c| c == category)),
                "no {category} entry"
            );
        }
        for (entry, _, _) in &set.entries {
            assert_eq!(entry.base.len(), 40, "{}", entry.id);
            assert_eq!(entry.fix.head.len(), 40, "{}", entry.id);
            assert!(!entry.fix.commits.is_empty(), "{}", entry.id);
            assert!(!entry.checks.is_empty(), "{}", entry.id);
            assert!(
                !entry
                    .request()
                    .contains("https://github.com/OpenAgentsInc/openagents/issues/")
            );
            for check in &entry.checks {
                assert!(
                    ["fix_tests", "deliverables"].contains(&check.group.as_str()),
                    "{}: {}",
                    entry.id,
                    check.group
                );
                if let Kind::Command { files, inject, .. } = &check.kind {
                    for hidden in files.values() {
                        assert!(set.hidden(hidden).is_file(), "{hidden}");
                    }
                    if let Some(inject) = inject {
                        assert!(set.hidden(&inject.snippet).is_file(), "{}", inject.snippet);
                    }
                }
            }
        }
    }

    #[test]
    fn a_changed_file_breaks_the_seal() {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["entries", "hidden"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        std::fs::write(dir.path().join("hidden/a.rs"), "// a\n").unwrap();
        std::fs::write(
            dir.path().join("manifest.json"),
            json!({ "schema": SET_SCHEMA, "split": { "development": [], "held-out": [] } })
                .to_string(),
        )
        .unwrap();
        seal(dir.path()).unwrap();
        assert!(load(dir.path()).unwrap().entries.is_empty());
        std::fs::write(dir.path().join("hidden/a.rs"), "// b\n").unwrap();
        let error = load(dir.path()).unwrap_err();
        assert!(
            error.contains("changed since the manifest was sealed"),
            "{error}"
        );
    }
}
