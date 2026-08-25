//! Typed facts about the git state of the mounted workspace, read straight
//! from the `.git` plumbing files — no `git` binary, no object store, no
//! packfiles, no zlib.
//!
//! What it reads, and what each read honestly gives:
//!
//! - `.git/HEAD` — the current branch, or the detached commit id.
//! - `.git/refs/heads/*` and `.git/packed-refs` — local branch tips.
//! - `.git/logs/HEAD` — the reflog, whose last N lines give recent commit
//!   ids, timestamps, and messages without touching the object store.
//! - `.git/index` (version 2 only) — tracked path count and per-entry
//!   `{path, size, mtime}`. Versions above 2 are reported, not misparsed:
//!   the header's entry count is still honest, the per-entry facts are
//!   declared unavailable.
//! - A bounded walk of the workdir (skipping `.git`) compared against the
//!   index by size and mtime — which makes `changed_candidates` exactly
//!   that: candidates. Without hashing content this is never a verdict,
//!   and the output says so (`comparison: "size_and_mtime_only"`).
//!
//! Everything skipped is named in `notes` rather than silently absorbed:
//! an unsupported index version, a truncated index read, a missing reflog,
//! branches known only from `packed-refs`, a walk that hit its ceiling.
//!
//! Follow-up (recorded here per the harvest issue): a gitoxide-backed
//! object walk and blame are the next capability step. `gix` was
//! deliberately not used for this plugin — its `wasm32-unknown-unknown`
//! story with fs/time features is a fight — so history here is
//! reflog-based and there is no blame.

use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, read_mounted_file_range, MountDirListing,
    Refusal, RefusalCode,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_MAX_LOG: usize = 15;
const MAX_LOG_CAP: usize = 100;
const DEFAULT_MAX_PATHS: usize = 20;
const MAX_PATHS_CAP: usize = 100;
const DEFAULT_MAX_WALK: usize = 5_000;
const MAX_WALK_CAP: usize = 20_000;

/// Mirror of the host's per-read bound; an index past it is read from the
/// front through the range import and reported as truncated.
const INDEX_READ_BOUND: u32 = 1_048_576;

const HEAD_PATH: &str = ".git/HEAD";
const HEAD_REF_PREFIX: &str = "ref: refs/heads/";
const REFS_HEADS_DIR: &str = ".git/refs/heads";
const PACKED_REFS_PATH: &str = ".git/packed-refs";
const HEAD_LOG_PATH: &str = ".git/logs/HEAD";
const INDEX_PATH: &str = ".git/index";

/// The fixed portion of a version-2 index entry, before the path.
const INDEX_ENTRY_FIXED: usize = 62;

#[derive(Deserialize)]
pub struct Input {
    /// Which facts to report: any of `head`, `branches`, `log`, `status`.
    /// All four when absent.
    #[serde(default)]
    pub facts: Option<Vec<String>>,
    /// Most reflog entries to return. Default 15, capped at 100.
    #[serde(default)]
    pub max_log: Option<usize>,
    /// Most paths listed per status category. Default 20, capped at 100.
    #[serde(default)]
    pub max_paths: Option<usize>,
    /// Most workdir files statted by the walk. Default 5000, capped at 20000.
    #[serde(default)]
    pub max_walk: Option<usize>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Head {
    /// The branch `HEAD` points at, when it points at one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit id `HEAD` holds directly, when it is detached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detached: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LogEntry {
    /// The commit id the reflog line moved to.
    pub id: String,
    /// The reflog timestamp, in milliseconds since the Unix epoch.
    pub at_ms: i64,
    pub message: String,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct PathFacts {
    /// How many paths fell in this category, before the listing bound.
    pub count: usize,
    /// The first paths, at most `max_paths`, sorted.
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Status {
    /// Tracked path count, as the index header declares it.
    pub tracked: usize,
    /// Tracked files whose size or mtime differs from the index. Candidates
    /// only: nothing here hashed content, so a stat difference is a reason
    /// to look, never a verdict.
    pub changed_candidates: PathFacts,
    /// Paths present in the workdir but absent from the index.
    pub untracked: PathFacts,
    /// Paths present in the index but absent from the workdir.
    pub missing: PathFacts,
    /// True when the walk hit its file ceiling or a listing was truncated,
    /// so the three categories above saw only part of the workdir.
    pub walk_truncated: bool,
    /// How tracked files were compared. Always `size_and_mtime_only`.
    pub comparison: &'static str,
}

#[derive(Debug, Default, Serialize)]
pub struct Output {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<Head>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<Vec<Branch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<Vec<LogEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// Everything honestly skipped or degraded, named.
    pub notes: Vec<String>,
}

/// The host capabilities the plugin uses, abstracted so the logic runs
/// under `cargo test` with a fake host and inside the WASM host unchanged.
pub trait Host {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal>;
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal>;
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal>;
}

struct RealHost;

impl Host for RealHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        list_mounted_dir(mount_index, path)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        read_mounted_file(path)
    }
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        read_mounted_file_range(path, offset, max_bytes)
    }
}

/// True when the text is 40 hexadecimal characters.
fn is_sha40(text: &str) -> bool {
    text.len() == 40 && text.chars().all(|c| c.is_ascii_hexdigit())
}

fn read_string(host: &dyn Host, path: &str) -> Option<String> {
    host.read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The whole report, over any [`Host`].
pub fn git_facts(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    // `.git/HEAD` is the existence proof for a git repository; without it
    // there is nothing to report about.
    let head_text = match read_string(host, HEAD_PATH) {
        Some(text) => text,
        None => {
            return Err(Refusal::unsupported(
                "the workspace has no git repository at its root (no .git/HEAD)",
            ))
        }
    };

    let mut notes: Vec<String> = Vec::new();
    let selected = select_facts(input.facts.as_deref(), &mut notes);

    let max_log = input.max_log.unwrap_or(DEFAULT_MAX_LOG).clamp(1, MAX_LOG_CAP);
    let max_paths = input.max_paths.unwrap_or(DEFAULT_MAX_PATHS).clamp(1, MAX_PATHS_CAP);
    let max_walk = input.max_walk.unwrap_or(DEFAULT_MAX_WALK).clamp(1, MAX_WALK_CAP);

    let head = selected.head.then(|| parse_head(&head_text));
    let branches = selected.branches.then(|| read_branches(host, &mut notes));
    let log = selected.log.then(|| read_log(host, max_log, &mut notes));
    let status = selected
        .status
        .then(|| read_status(host, max_paths, max_walk, &mut notes));

    Ok(Output { head, branches, log, status, notes })
}

#[derive(Default)]
struct Selected {
    head: bool,
    branches: bool,
    log: bool,
    status: bool,
}

fn select_facts(facts: Option<&[String]>, notes: &mut Vec<String>) -> Selected {
    let Some(facts) = facts else {
        return Selected { head: true, branches: true, log: true, status: true };
    };
    let mut selected = Selected::default();
    for fact in facts {
        match fact.as_str() {
            "head" => selected.head = true,
            "branches" => selected.branches = true,
            "log" => selected.log = true,
            "status" => selected.status = true,
            other => notes.push(format!(
                "unknown fact `{other}` ignored; known facts are head, branches, log, status"
            )),
        }
    }
    selected
}

/// `.git/HEAD`: a symbolic ref names the branch, a bare 40-hex id is a
/// detached head. Anything else is neither, honestly.
fn parse_head(text: &str) -> Head {
    let first = text.lines().next().unwrap_or("").trim();
    if let Some(branch) = first.strip_prefix(HEAD_REF_PREFIX) {
        return Head { branch: Some(branch.trim().to_string()), detached: None };
    }
    if is_sha40(first) {
        return Head { branch: None, detached: Some(first.to_string()) };
    }
    Head::default()
}

/// Recursively collect the relative paths of all files under `path`.
/// Failures stop descent rather than failing the report.
fn collect_files(host: &dyn Host, path: &str, out: &mut Vec<String>) {
    let Ok(listing) = host.list(0, path) else { return };
    for entry in listing.entries {
        let child = format!("{path}/{}", entry.name);
        match entry.kind.as_str() {
            "file" => out.push(child),
            "dir" => collect_files(host, &child, out),
            _ => {}
        }
    }
}

/// Local branch tips: loose refs first-class, `packed-refs` behind them
/// (a loose ref shadows its packed twin, exactly as git resolves them).
fn read_branches(host: &dyn Host, notes: &mut Vec<String>) -> Vec<Branch> {
    let mut tips: BTreeMap<String, String> = BTreeMap::new();

    let mut packed_only = 0usize;
    if let Some(text) = read_string(host, PACKED_REFS_PATH) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (Some(id), Some(refname)) = (parts.next(), parts.next()) else { continue };
            if let Some(name) = refname.strip_prefix("refs/heads/") {
                if is_sha40(id) {
                    tips.insert(name.to_string(), id.to_string());
                    packed_only += 1;
                }
            }
        }
    }

    let mut loose_files = Vec::new();
    collect_files(host, REFS_HEADS_DIR, &mut loose_files);
    let mut loose_count = 0usize;
    for file in &loose_files {
        let name = file
            .strip_prefix(".git/refs/heads/")
            .unwrap_or(file)
            .to_string();
        let Some(text) = read_string(host, file) else { continue };
        let id = text.trim();
        if is_sha40(id) {
            tips.insert(name, id.to_string());
            loose_count += 1;
        }
    }

    if loose_count == 0 && packed_only > 0 {
        notes.push("branch tips come from packed-refs only; no loose refs under .git/refs/heads".to_string());
    }

    tips.into_iter().map(|(name, id)| Branch { name, id }).collect()
}

/// One parsed reflog line.
struct ReflogLine {
    new: String,
    timestamp: i64,
    message: String,
}

/// A reflog line: `old-id new-id author <email> timestamp tz\tmessage`.
/// Only the first tab separates the header from the message, so a message
/// containing tabs survives whole.
fn parse_reflog_line(line: &str) -> Option<ReflogLine> {
    if line.is_empty() {
        return None;
    }
    let (prefix, message) = match line.find('\t') {
        Some(at) => (&line[..at], &line[at + 1..]),
        None => (line, ""),
    };
    let tokens: Vec<&str> = prefix.split_whitespace().collect();
    if tokens.len() < 6 {
        return None;
    }
    let old = tokens.first()?;
    let new = tokens.get(1)?;
    if !is_sha40(old) || !is_sha40(new) {
        return None;
    }
    let tz = tokens.last()?;
    if !tz.starts_with('+') && !tz.starts_with('-') {
        return None;
    }
    let timestamp = tokens.get(tokens.len() - 2)?.parse::<i64>().ok()?;
    Some(ReflogLine {
        new: (*new).to_string(),
        timestamp,
        message: message.trim_end_matches(['\n', '\r']).to_string(),
    })
}

/// Recent history from `.git/logs/HEAD`: the last `max_log` entries,
/// newest first. Reflog-based, so it reaches only as far back as the
/// reflog does — and its absence is a note, not an error.
fn read_log(host: &dyn Host, max_log: usize, notes: &mut Vec<String>) -> Vec<LogEntry> {
    let Some(text) = read_string(host, HEAD_LOG_PATH) else {
        notes.push("no reflog at .git/logs/HEAD; log is empty".to_string());
        return Vec::new();
    };
    let parsed: Vec<ReflogLine> = text.lines().filter_map(parse_reflog_line).collect();
    let start = parsed.len().saturating_sub(max_log);
    parsed[start..]
        .iter()
        .rev()
        .map(|line| LogEntry {
            id: line.new.clone(),
            at_ms: line.timestamp * 1000,
            message: line.message.clone(),
        })
        .collect()
}

/// One tracked path as the index records it.
#[derive(Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub path: String,
    pub size: u64,
    pub mtime_ms: i64,
}

/// The index as far as it could honestly be parsed.
struct ParsedIndex {
    /// Entry count as the header declares it, readable for every version.
    declared: usize,
    entries: Vec<IndexEntry>,
    /// True when every declared entry was parsed; false leaves the
    /// classification below partial, and a note says why.
    complete: bool,
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

/// Parse a version-2 index, whole entries only. `input_truncated` says the
/// bytes are a prefix of the real file, so running out of bytes is expected
/// there and named `index_truncated`; anywhere else it is a malformed file.
fn parse_index(bytes: &[u8], input_truncated: bool, notes: &mut Vec<String>) -> ParsedIndex {
    if bytes.len() < 12 || &bytes[0..4] != b"DIRC" {
        notes.push("the index has no DIRC header; treating it as unreadable".to_string());
        return ParsedIndex { declared: 0, entries: Vec::new(), complete: false };
    }
    let version = be32(bytes, 4);
    let declared = be32(bytes, 8) as usize;
    if version != 2 {
        notes.push(format!(
            "index_version_unsupported: the index is version {version} and only version 2 is parsed; the tracked count is the header's, per-entry paths and stats are unavailable"
        ));
        return ParsedIndex { declared, entries: Vec::new(), complete: false };
    }

    let mut entries = Vec::new();
    let mut offset = 12usize;
    let mut complete = true;
    for _ in 0..declared {
        // The fixed 62 bytes: stat data, object id, flags.
        if offset + INDEX_ENTRY_FIXED > bytes.len() {
            complete = false;
            break;
        }
        let mtime_sec = i64::from(be32(bytes, offset + 8));
        let mtime_nsec = i64::from(be32(bytes, offset + 12));
        let size = u64::from(be32(bytes, offset + 36));
        let flags = be16(bytes, offset + 60);
        if flags & 0x4000 != 0 {
            // The extended bit belongs to version 3; in a version-2 header
            // it means the file is not what it claims. Stop, do not guess.
            notes.push(
                "an index entry carries extended flags a version-2 index cannot have; stopping at the entries already parsed".to_string(),
            );
            complete = false;
            break;
        }
        let name_at = offset + INDEX_ENTRY_FIXED;
        let name_len_field = (flags & 0x0FFF) as usize;
        let name_len = if name_len_field < 0x0FFF {
            name_len_field
        } else {
            // Names of 4095 bytes or more store 0xFFF; the real length is
            // up to the NUL terminator.
            match bytes[name_at..].iter().position(|b| *b == 0) {
                Some(at) => at,
                None => {
                    complete = false;
                    break;
                }
            }
        };
        // Whole entries only: the fixed part, the name, and the NUL
        // padding to the next 8-byte boundary must all be present.
        let entry_len = (INDEX_ENTRY_FIXED + name_len + 8) & !7;
        if offset + entry_len > bytes.len() || name_at + name_len > bytes.len() {
            complete = false;
            break;
        }
        let path = String::from_utf8_lossy(&bytes[name_at..name_at + name_len]).into_owned();
        entries.push(IndexEntry {
            path,
            size,
            mtime_ms: mtime_sec * 1000 + mtime_nsec / 1_000_000,
        });
        offset += entry_len;
    }

    if !complete && entries.len() < declared {
        if input_truncated {
            notes.push(format!(
                "index_truncated: the index exceeds the read bound; {} of {declared} entries were parsed from its first bytes, whole entries only",
                entries.len()
            ));
        } else {
            notes.push(format!(
                "the index ends mid-entry; {} of {declared} declared entries were parsed",
                entries.len()
            ));
        }
    }

    ParsedIndex { declared, entries, complete }
}

/// The index bytes: whole when the file fits the host's bound, the first
/// bound's worth through the range import when it does not. The second
/// value says which.
fn load_index(host: &dyn Host, notes: &mut Vec<String>) -> Option<(Vec<u8>, bool)> {
    match host.read(INDEX_PATH) {
        Ok(bytes) => Some((bytes, false)),
        Err(refusal) if refusal.code == RefusalCode::FileTooLarge => {
            match host.read_range(INDEX_PATH, 0, INDEX_READ_BOUND) {
                Ok(bytes) => Some((bytes, true)),
                Err(_) => {
                    notes.push("the index exceeds the read bound and the range read failed; status has no index data".to_string());
                    None
                }
            }
        }
        Err(_) => {
            notes.push("no .git/index; the index is treated as empty (a repository before its first `git add` has none)".to_string());
            None
        }
    }
}

/// One statted workdir file.
struct WalkedFile {
    path: String,
    size: u64,
    mtime_ms: i64,
}

struct Walker<'a> {
    host: &'a dyn Host,
    budget: usize,
    truncated: bool,
    files: Vec<WalkedFile>,
}

impl Walker<'_> {
    /// Bounded recursive walk from the mount root, skipping `.git`.
    /// Entries the host cannot list stop descent, not the walk.
    fn walk(&mut self, path: &str) {
        if self.truncated {
            return;
        }
        let Ok(listing) = self.host.list(0, path) else { return };
        if listing.truncated {
            self.truncated = true;
        }
        for entry in listing.entries {
            if self.truncated {
                return;
            }
            if entry.name == ".git" {
                continue;
            }
            let child = if path.is_empty() {
                entry.name.clone()
            } else {
                format!("{path}/{}", entry.name)
            };
            match entry.kind.as_str() {
                "file" => {
                    if self.files.len() >= self.budget {
                        self.truncated = true;
                        return;
                    }
                    self.files.push(WalkedFile {
                        path: child,
                        size: entry.size,
                        mtime_ms: entry.mtime_ms,
                    });
                }
                "dir" => self.walk(&child),
                _ => {}
            }
        }
    }
}

/// Bound a sorted path set to `max_paths` listed paths plus the full count.
fn path_facts(paths: BTreeSet<String>, max_paths: usize) -> PathFacts {
    let count = paths.len();
    PathFacts { count, paths: paths.into_iter().take(max_paths).collect() }
}

/// The status report: index versus a bounded workdir walk, compared by
/// size and mtime only.
fn read_status(
    host: &dyn Host,
    max_paths: usize,
    max_walk: usize,
    notes: &mut Vec<String>,
) -> Status {
    let parsed = match load_index(host, notes) {
        Some((bytes, input_truncated)) => parse_index(&bytes, input_truncated, notes),
        // A missing index is an empty, complete index: everything on disk
        // is honestly untracked.
        None => ParsedIndex { declared: 0, entries: Vec::new(), complete: true },
    };

    // No entries and no proof of completeness (an unsupported version, a
    // header that would not parse): classifying the workdir against an
    // unknown index would call every tracked file untracked. Refuse the
    // comparison instead; the note above already says why.
    if parsed.entries.is_empty() && !parsed.complete {
        notes.push(
            "changed_candidates, untracked, and missing were not computed: the index's entries could not be read".to_string(),
        );
        return Status {
            tracked: parsed.declared,
            changed_candidates: PathFacts::default(),
            untracked: PathFacts::default(),
            missing: PathFacts::default(),
            walk_truncated: false,
            comparison: "size_and_mtime_only",
        };
    }

    let mut walker = Walker { host, budget: max_walk, truncated: false, files: Vec::new() };
    walker.walk("");

    let index: BTreeMap<&str, &IndexEntry> =
        parsed.entries.iter().map(|entry| (entry.path.as_str(), entry)).collect();

    let mut changed: BTreeSet<String> = BTreeSet::new();
    let mut untracked: BTreeSet<String> = BTreeSet::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for file in &walker.files {
        match index.get(file.path.as_str()) {
            Some(entry) => {
                seen.insert(entry.path.as_str());
                if entry.size != file.size || entry.mtime_ms != file.mtime_ms {
                    changed.insert(file.path.clone());
                }
            }
            None => {
                untracked.insert(file.path.clone());
            }
        }
    }

    let missing: BTreeSet<String> = index
        .keys()
        .filter(|path| !seen.contains(**path))
        .map(|path| (*path).to_string())
        .collect();

    if walker.truncated {
        notes.push(format!(
            "the workdir walk stopped at {} files, so untracked and missing reflect only the part walked",
            walker.files.len()
        ));
    }
    if !parsed.complete && !parsed.entries.is_empty() {
        notes.push(
            "the comparison ran against the entries that were parsed; files tracked past the truncation point would show as untracked".to_string(),
        );
    }

    Status {
        tracked: parsed.declared,
        changed_candidates: path_facts(changed, max_paths),
        untracked: path_facts(untracked, max_paths),
        missing: path_facts(missing, max_paths),
        walk_truncated: walker.truncated,
        comparison: "size_and_mtime_only",
    }
}

fn handle(input: Input) -> Result<Output, Refusal> {
    git_facts(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
