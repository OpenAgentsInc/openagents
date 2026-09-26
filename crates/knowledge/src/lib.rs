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
pub mod evidence;
pub mod front;
pub mod harvest;
pub mod lint;
pub mod remote;
pub mod search;
pub mod transfer;
mod write;
pub mod xp;

pub use write::{archive, date, pending, set_evidence, set_status, template, today, version_path};

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use front::Value;

/// What an entry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A standard definition, its variants, and how to tell them apart.
    Method,
    /// An input or state that breaks common code.
    EdgeCase,
    /// A mistake agents make, how to notice it, and how to avoid it.
    Slip,
    /// How a class of environment behaves.
    Environment,
    /// How to use a command or library correctly.
    Tool,
}

impl Kind {
    /// The kind a name like `edge-case` names.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "method" => Kind::Method,
            "edge-case" => Kind::EdgeCase,
            "slip" => Kind::Slip,
            "environment" => Kind::Environment,
            "tool" => Kind::Tool,
            _ => return None,
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Method => "method",
            Kind::EdgeCase => "edge-case",
            Kind::Slip => "slip",
            Kind::Environment => "environment",
            Kind::Tool => "tool",
        })
    }
}

/// How far an entry is trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Not yet reviewed or measured; shown only when candidates are asked for.
    Candidate,
    /// Shown by default.
    Admitted,
    /// Wrong or harmful; never shown, kept so earlier runs can be explained.
    Withdrawn,
}

impl Status {
    /// The status a name like `admitted` names.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "candidate" => Status::Candidate,
            "admitted" => Status::Admitted,
            "withdrawn" => Status::Withdrawn,
            _ => return None,
        })
    }

    /// Whether an entry with this status is shown, given whether candidates
    /// are.
    #[must_use]
    pub fn shown(self, candidates: bool) -> bool {
        match self {
            Status::Admitted => true,
            Status::Candidate => candidates,
            Status::Withdrawn => false,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Status::Candidate => "candidate",
            Status::Admitted => "admitted",
            Status::Withdrawn => "withdrawn",
        })
    }
}

/// One entry.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub version: u32,
    pub kind: Kind,
    pub title: String,
    /// One or two sentences: what the entry is about and when it applies.
    pub summary: String,
    pub tags: Vec<String>,
    pub applies_when: String,
    pub status: Status,
    pub author: String,
    /// Run IDs the entry was written from, or `reference`.
    pub written_from: Vec<String>,
    /// Sources for its definitions.
    pub cites: Vec<String>,
    /// Admission records, one line each: reviews, measurements, demotions,
    /// and withdrawals.
    pub evidence: Vec<String>,
    /// The Markdown after the front matter.
    pub body: String,
    /// `sha256:` and the hex SHA-256 of the whole file.
    pub digest: String,
}

/// The front-matter keys an entry may have.
const KEYS: &[&str] = &[
    "id",
    "version",
    "kind",
    "title",
    "summary",
    "tags",
    "applies_when",
    "status",
    "author",
    "provenance",
    "evidence",
];

/// `sha256:` and the hex SHA-256 of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Whether `id` is lowercase letters, digits, dots, and hyphens, starting
/// with a letter.
pub(crate) fn valid_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

impl Entry {
    /// Parses and validates one entry file.
    ///
    /// # Errors
    ///
    /// Malformed front matter, a missing or empty required field, an
    /// unknown key, kind, or status, or an empty body.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (front, body) = front::split(text)?;
        let fields = front::parse(front)?;
        if let Some((key, _)) = fields.iter().find(|(k, _)| !KEYS.contains(&k.as_str())) {
            return Err(format!("unknown key `{key}`"));
        }
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let text_of = |key: &str| -> Result<String, String> {
            let value = get(key)
                .and_then(Value::text)
                .map(str::trim)
                .unwrap_or_default();
            if value.is_empty() {
                return Err(format!("`{key}` is missing or empty"));
            }
            Ok(value.to_string())
        };
        let list_of = |value: Option<&Value>, key: &str| -> Result<Vec<String>, String> {
            match value {
                None => Ok(Vec::new()),
                Some(value) => value.list().ok_or(format!("`{key}` must be a list")),
            }
        };
        let id = text_of("id")?;
        if !valid_id(&id) {
            return Err(format!(
                "the id `{id}` must be lowercase letters, digits, dots, and hyphens"
            ));
        }
        let version = text_of("version")?
            .parse::<u32>()
            .map_err(|_| "`version` must be a whole number".to_string())?;
        let kind = text_of("kind")?;
        let kind = Kind::parse(&kind).ok_or(format!(
            "unknown kind `{kind}`: use method, edge-case, slip, environment, or tool"
        ))?;
        let status = text_of("status")?;
        let status = Status::parse(&status).ok_or(format!(
            "unknown status `{status}`: use candidate, admitted, or withdrawn"
        ))?;
        let (written_from, cites) = match get("provenance") {
            Some(Value::Map(map)) => {
                let find = |key: &str| map.iter().find(|(k, _)| k == key).map(|(_, v)| v);
                (
                    list_of(find("written_from"), "provenance.written_from")?,
                    list_of(find("cites"), "provenance.cites")?,
                )
            }
            None => (Vec::new(), Vec::new()),
            Some(_) => return Err("`provenance` must hold written_from and cites".to_string()),
        };
        let body = body.trim().to_string();
        if body.is_empty() {
            return Err("the entry has no body after the front matter".to_string());
        }
        Ok(Entry {
            id,
            version,
            kind,
            title: text_of("title")?,
            summary: text_of("summary")?,
            tags: list_of(get("tags"), "tags")?,
            applies_when: text_of("applies_when")?,
            status,
            author: text_of("author")?,
            written_from,
            cites,
            evidence: list_of(get("evidence"), "evidence")?,
            body,
            digest: digest(text.as_bytes()),
        })
    }

    /// The text retrieval matches on: the title, summary, tags, and
    /// `applies_when`.
    #[must_use]
    pub fn search_text(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}",
            self.title,
            self.summary,
            self.tags.join(", "),
            self.applies_when
        )
    }

    /// The first 12 hex digits of the digest, for display.
    #[must_use]
    pub fn short_digest(&self) -> &str {
        let hex = self.digest.trim_start_matches("sha256:");
        &hex[..hex.len().min(12)]
    }
}

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

/// The knowledge directory: `OPENAGENTS_KNOWLEDGE`, or else `knowledge/`
/// in the checkout this binary was built from.
#[must_use]
pub fn default_dir() -> PathBuf {
    match std::env::var_os(DIR_VAR) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../knowledge");
            dir.canonicalize().unwrap_or(dir)
        }
    }
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
