//! The shared knowledge base: entries an agent can search while it works.
//!
//! An entry is one Markdown file with YAML front matter under `knowledge/`
//! at the repository root: a method's definition, an edge case, a common
//! mistake, how an environment behaves, or how to use a tool.
//! `docs/coder/design/knowledge-base.md` is the design, and issue #9670
//! tracks it.
//!
//! [`Base`] loads and validates the entries, [`lint`] checks that none
//! names or quotes a benchmark task, and [`search`] finds the entries that
//! bear on a query, by BM25 over each entry's title, summary, tags, and
//! `applies_when`, combined with cosine similarity over embeddings from
//! `crates/openrouter` when a key is available. [`harvest`] proposes
//! entries from a finished run, [`evidence`] measures entries from recorded
//! runs and writes NIP-EVAL reports, and [`remote`] turns entries into
//! NIP-KB events, accepts synced ones, and applies the reader's trust.
//! [`xp`] derives the XP ledger from NIP-XP awards over those entries.
//! [`cli`] is the `kb` command. Nothing here runs an agent's loop or opens
//! a relay connection.

pub mod cli;
pub mod codebase;
pub mod evidence;
pub mod harvest;
pub mod lint;
pub mod private;
pub mod product;
pub mod remote;
pub mod search;
pub mod snapshot;
pub mod study;
pub mod transfer;
mod write;

pub use write::{archive, date, pending, set_evidence, set_status, template, today, version_path};

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use xp_ledger as xp;
pub use xp_ledger::entry::{Entry, Kind, Status, digest, valid_id};
pub use xp_ledger::front;

/// A problem with one file or entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Problem {
    /// The file or entry ID.
    pub at: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

/// A set of entries.
#[derive(Clone, Debug, Default)]
pub struct Base {
    pub entries: Vec<Entry>,
}

impl Base {
    /// Reads every `.md` file in `dir`, in name order, and returns the
    /// entries that parse and a problem for each file that doesn't, for
    /// a repeated ID, and for a file not named after its ID.
    #[must_use]
    pub fn read(dir: &Path) -> (Vec<Entry>, Vec<Problem>) {
        let mut entries: Vec<Entry> = Vec::new();
        let mut problems = Vec::new();
        let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(listing) => listing
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect(),
            Err(error) => {
                problems.push(Problem {
                    at: dir.display().to_string(),
                    message: format!("can't read the directory: {error}"),
                });
                return (entries, problems);
            }
        };
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name == "README.md" {
                continue;
            }
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| format!("can't read the file: {e}"))
                .and_then(|text| Entry::parse(&text));
            match parsed {
                Ok(entry) => {
                    if name != format!("{}.md", entry.id) {
                        problems.push(Problem {
                            at: name.clone(),
                            message: format!("the file must be named {}.md", entry.id),
                        });
                    }
                    if entries.iter().any(|e| e.id == entry.id) {
                        problems.push(Problem {
                            at: name,
                            message: format!("the id {} is used by another file", entry.id),
                        });
                        continue;
                    }
                    entries.push(entry);
                }
                Err(message) => problems.push(Problem { at: name, message }),
            }
        }
        (entries, problems)
    }

    /// The entries in `dir` whose status is shown, given whether candidates
    /// are.
    ///
    /// # Errors
    ///
    /// Any problem [`Base::read`] finds.
    pub fn load(dir: &Path, candidates: bool) -> Result<Self, String> {
        let (entries, problems) = Base::read(dir);
        if let Some(problem) = problems.first() {
            return Err(format!(
                "the knowledge base in {} has {} problems; the first is {problem}",
                dir.display(),
                problems.len()
            ));
        }
        Ok(Base {
            entries: entries
                .into_iter()
                .filter(|e| e.status.shown(candidates))
                .collect(),
        })
    }

    /// The entry with `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// The variable that names the knowledge directory.
pub const DIR_VAR: &str = "OPENAGENTS_KNOWLEDGE";

include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

/// The knowledge directory: `OPENAGENTS_KNOWLEDGE`, or the user's cache.
#[must_use]
pub fn default_dir() -> PathBuf {
    match std::env::var_os(DIR_VAR) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".openagents/knowledge/entries"))
            .unwrap_or_else(|| PathBuf::from(".openagents/knowledge/entries")),
    }
}

/// Copies bundled entries to a writable cache without replacing local edits
/// or withdrawals. Explicit directories are never seeded.
pub fn seed_bundled(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    for (name, text) in BUNDLED {
        use std::io::Write;
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => file
                .write_all(text.as_bytes())
                .map_err(|e| format!("can't write {}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("can't write {}: {e}", path.display())),
        }
    }
    Ok(())
}

/// `~/.openagents/knowledge/embeddings.json`, where entry embeddings are
/// cached by digest.
#[must_use]
pub fn default_cache() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/knowledge/embeddings.json"))
}

#[cfg(test)]
mod tests;
