//! Guest plugin that scans a mounted `.git` directory for unreachable
//! commits and stash entries.
//!
//! It reads Git state directly: `HEAD`, loose refs, `packed-refs`, reflogs,
//! and loose objects. It does not shell out, does not call a `git` binary,
//! and does not parse packfiles. Output is a deterministic JSON value with
//! the current branch or detached HEAD, stash entries, and a bounded list
//! of lost commit candidates.

use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, MountDirListing, Refusal,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const DEFAULT_MAX_LOST_COMMITS: usize = 50;
const MAX_LOST_COMMITS_CAP: usize = 100;
const ZERO_SHA: &str = "0000000000000000000000000000000000000000";
const HEAD_REF_PREFIX: &str = "ref: refs/heads/";
const REFS_DIR: &str = ".git/refs/heads";
const HEAD_PATH: &str = ".git/HEAD";
const PACKED_REFS_PATH: &str = ".git/packed-refs";
const HEAD_LOG_PATH: &str = ".git/logs/HEAD";
const HEADS_LOG_DIR: &str = ".git/logs/refs/heads";
const STASH_LOG_PATH: &str = ".git/logs/refs/stash";
const STASH_REF_PATH: &str = ".git/refs/stash";
const OBJECTS_DIR: &str = ".git/objects";
const GIT_DIR_ENTRY: &str = ".git";

#[derive(Deserialize)]
pub struct Input {
    /// Maximum number of lost commits to report. Default 50, capped at 100.
    #[serde(default)]
    pub max_lost_commits: Option<usize>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Head {
    /// The current branch name, or `null` when HEAD is detached or unknown.
    pub branch: Option<String>,
    /// The resolved SHA of HEAD, or `null` when it cannot be read.
    pub sha: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StashEntry {
    pub selector: String,
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LostCommit {
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub timestamp: i64,
    pub packed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_date: Option<i64>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Summary {
    pub total_lost_candidates: usize,
    pub stashes_count: usize,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Output {
    pub head: Head,
    pub stash_entries: Vec<StashEntry>,
    pub lost_commits: Vec<LostCommit>,
    pub summary: Summary,
}

/// The two host capabilities the scanner uses, abstracted so the core logic
/// runs under `cargo test` with a fake host and inside the WASM host.
pub trait Host {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal>;
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal>;
}

struct RealHost;

impl Host for RealHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        list_mounted_dir(mount_index, path)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        read_mounted_file(path)
    }
}

/// A one-line helper so failures are not thrown until the call site.
fn read_string(host: &dyn Host, path: &str) -> Option<String> {
    host.read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// True when the text is 40 hexadecimal characters.
fn is_sha40(text: &str) -> bool {
    text.len() == 40 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Trim newlines and trailing whitespace from Git ref contents.
fn trim_ref(text: &str) -> &str {
    text.trim_end_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t')
}

/// Resolve `HEAD` into a branch name and/or a SHA.
fn read_head(host: &dyn Host) -> Head {
    let text = match read_string(host, HEAD_PATH) {
        Some(t) => t,
        None => return Head::default(),
    };
    let first = text.lines().next().unwrap_or("").trim();
    if first.starts_with(HEAD_REF_PREFIX) {
        let branch = first[HEAD_REF_PREFIX.len()..].trim().to_string();
        let ref_path = format!(".git/refs/heads/{branch}");
        let sha = read_string(host, &ref_path)
            .as_deref()
            .map(trim_ref)
            .filter(|s| is_sha40(s))
            .map(str::to_string);
        Head {
            branch: Some(branch),
            sha,
        }
    } else {
        let sha = trim_ref(first);
        if is_sha40(sha) {
            Head {
                branch: None,
                sha: Some(sha.to_string()),
            }
        } else {
            Head::default()
        }
    }
}

/// Recursively list a directory and collect relative paths to all files
/// under it. Uses the host's confined listing import; failures stop descent.
fn collect_files_recursive(
    host: &dyn Host,
    mount_index: u32,
    path: &str,
    out: &mut Vec<String>,
) {
    let listing = match host.list(mount_index, path) {
        Ok(l) => l,
        Err(_) => return,
    };
    for entry in listing.entries {
        let child = if path.is_empty() {
            entry.name
        } else {
            format!("{path}/{}", entry.name)
        };
        match entry.kind.as_str() {
            "file" => out.push(child),
            "dir" => collect_files_recursive(host, mount_index, &child, out),
            _ => {}
        }
    }
}

/// Read every loose ref under `.git/refs/heads/` and return `(refname, sha)`.
fn read_loose_refs(host: &dyn Host) -> Vec<(String, String)> {
    let mut files = Vec::new();
    collect_files_recursive(host, 0, REFS_DIR, &mut files);
    let mut out = Vec::new();
    for file in files {
        let name = file.strip_prefix(".git/refs/heads/").unwrap_or(&file).to_string();
        let sha = match read_string(host, &file) {
            Some(text) => trim_ref(&text).to_string(),
            None => continue,
        };
        if is_sha40(&sha) {
            out.push((name, sha));
        }
    }
    out
}

/// Parse `packed-refs` for current ref tips. Skip comments and `^` peeled lines.
fn read_packed_refs(host: &dyn Host) -> Vec<(String, String)> {
    let text = match read_string(host, PACKED_REFS_PATH) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let sha = parts.next().unwrap_or("");
        let refname = parts.next().unwrap_or("");
        if is_sha40(sha) && !refname.is_empty() {
            out.push((refname.to_string(), sha.to_string()));
        }
    }
    out
}

/// One raw reflog entry after parsing.
#[derive(Debug, Clone)]
struct ReflogEntry {
    old: String,
    new: String,
    timestamp: i64,
    message: String,
}

/// Parse a single reflog line into old/new SHA, timestamp, and message.
fn parse_reflog_line(line: &str) -> Option<ReflogEntry> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (prefix, message) = match line.find('\t') {
        Some(idx) => (&line[..idx], &line[idx + 1..]),
        None => (line, ""),
    };
    let prefix = prefix.trim_end();
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
    let time_token = tokens.get(tokens.len() - 2)?;
    let timestamp = time_token.parse::<i64>().ok()?;
    Some(ReflogEntry {
        old: old.to_string(),
        new: new.to_string(),
        timestamp,
        message: message.trim_end_matches(|c: char| c == '\n' || c == '\r').to_string(),
    })
}

/// Read a reflog file and parse every line that matches the expected shape.
fn read_reflog(host: &dyn Host, path: &str) -> Vec<ReflogEntry> {
    let text = match read_string(host, path) {
        Some(t) => t,
        None => return Vec::new(),
    };
    text.lines().filter_map(parse_reflog_line).collect()
}

/// List the `.git/logs/refs/heads/` directory and read each branch reflog.
fn read_branch_reflogs(host: &dyn Host) -> Vec<ReflogEntry> {
    let listing = match host.list(0, HEADS_LOG_DIR) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in listing.entries {
        if entry.kind != "file" {
            continue;
        }
        let path = format!("{HEADS_LOG_DIR}/{}", entry.name);
        out.extend(read_reflog(host, &path));
    }
    out
}

/// Stash entries from `logs/refs/stash`, falling back to `refs/stash`.
fn read_stash(host: &dyn Host) -> Vec<StashEntry> {
    let mut out = Vec::new();
    let mut index = 0i64;
    for entry in read_reflog(host, STASH_LOG_PATH) {
        out.push(StashEntry {
            selector: format!("stash@{{{index}}}"),
            sha: entry.new.clone(),
            message: if entry.message.is_empty() { None } else { Some(entry.message) },
            timestamp: entry.timestamp,
        });
        index += 1;
    }
    if out.is_empty() {
        if let Some(text) = read_string(host, STASH_REF_PATH) {
            let sha = trim_ref(&text).to_string();
            if is_sha40(&sha) {
                out.push(StashEntry {
                    selector: "stash@{0}".to_string(),
                    sha,
                    message: None,
                    timestamp: 0,
                });
            }
        }
    }
    out
}

/// Read a loose object from `.git/objects/<first-2-hex>/<remaining-38-hex>`.
fn read_loose_object(host: &dyn Host, sha: &str) -> Option<Vec<u8>> {
    let prefix = &sha[..2];
    let suffix = &sha[2..];
    let path = format!("{OBJECTS_DIR}/{prefix}/{suffix}");
    host.read(&path).ok()
}

/// Decompress and parse a Git commit object. Returns `(subject, author, author_date)`.
fn parse_commit_object(bytes: &[u8]) -> Option<(String, String, i64)> {
    let inflated = miniz_oxide::inflate::decompress_to_vec_zlib(bytes).ok()?;
    let text = String::from_utf8_lossy(&inflated);
    let null_pos = text.find('\0')?;
    let header = &text[..null_pos];
    if !header.starts_with("commit ") {
        return None;
    }
    let body = &text[null_pos + 1..];
    let mut author: Option<String> = None;
    let mut author_date: Option<i64> = None;
    let mut message_start: Option<usize> = None;
    for (i, line) in body.lines().enumerate() {
        if line.starts_with("author ") {
            if let Some(rest) = line.strip_prefix("author ") {
                author = Some(parse_author_name(rest));
                author_date = parse_author_date(rest);
            }
        }
        if line.is_empty() {
            message_start = Some(body.lines().take(i + 1).map(|l| l.len() + 1).sum::<usize>());
            break;
        }
    }
    let message = if let Some(start) = message_start {
        body.get(start..).unwrap_or("").to_string()
    } else {
        body.to_string()
    };
    let subject = message.trim().lines().next().unwrap_or("").to_string();
    Some((subject, author.unwrap_or_default(), author_date.unwrap_or(0)))
}

/// Extract the human-readable author name and email from an `author` line.
fn parse_author_name(line: &str) -> String {
    let email_end = match line.rfind('>') {
        Some(idx) => idx,
        None => return line.to_string(),
    };
    let before = &line[..=email_end];
    before.to_string()
}

/// Extract the integer timestamp from an `author` line.
fn parse_author_date(line: &str) -> Option<i64> {
    let email_end = line.rfind('>')?;
    let tail = &line[email_end + 1..];
    let mut parts = tail.split_whitespace();
    parts.next()?.parse::<i64>().ok()
}

/// Determine whether the `.git` directory exists on the mount.
fn git_dir_present(host: &dyn Host) -> bool {
    host.list(0, ".").ok().map_or(false, |l| {
        l.entries.iter().any(|e| e.name == GIT_DIR_ENTRY && e.kind == "dir")
    })
}

/// The whole scan, over any [`Host`]. Fail-soft: a missing `.git` or file
/// contributes an empty or partial result rather than a refusal.
pub fn scan(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    if !git_dir_present(host) {
        return Ok(Output::default());
    }

    let head = read_head(host);
    let max = input
        .max_lost_commits
        .unwrap_or(DEFAULT_MAX_LOST_COMMITS)
        .clamp(1, MAX_LOST_COMMITS_CAP);

    // Current ref tips: these SHAs are reachable and NOT lost.
    let mut tips: HashSet<String> = HashSet::new();
    if let Some(ref sha) = head.sha {
        tips.insert(sha.clone());
    }
    for (_, sha) in read_loose_refs(host) {
        tips.insert(sha);
    }
    for (refname, sha) in read_packed_refs(host) {
        tips.insert(sha);
        // A detached HEAD may be recorded as `HEAD` in packed-refs.
        if refname == "HEAD" && head.sha.is_none() {
            // already covered by tip set, no branch.
        }
    }

    // Collect reflog candidates and remember the latest message/timestamp.
    let mut candidate_actions: HashMap<String, (i64, String)> = HashMap::new();
    for entry in read_reflog(host, HEAD_LOG_PATH).into_iter().chain(read_branch_reflogs(host)) {
        let timestamp = entry.timestamp;
        let message = &entry.message;
        for sha in [entry.old, entry.new] {
            if sha == ZERO_SHA {
                continue;
            }
            update_candidate(&mut candidate_actions, sha, timestamp, message);
        }
    }

    let stash = read_stash(host);
    let mut lost: Vec<LostCommit> = Vec::new();
    for (sha, (timestamp, action)) in candidate_actions {
        if tips.contains(&sha) {
            continue;
        }
        if let Some(bytes) = read_loose_object(host, &sha) {
            if let Some((subject, author, author_date)) = parse_commit_object(&bytes) {
                lost.push(LostCommit {
                    sha,
                    action: if action.is_empty() { None } else { Some(action) },
                    timestamp,
                    packed: false,
                    subject: if subject.is_empty() { None } else { Some(subject) },
                    author: if author.is_empty() { None } else { Some(author) },
                    author_date: if author_date == 0 { None } else { Some(author_date) },
                });
            } else {
                // Unparsable object bytes: report as packed so as not to fail.
                lost.push(LostCommit {
                    sha,
                    action: if action.is_empty() { None } else { Some(action) },
                    timestamp,
                    packed: true,
                    subject: None,
                    author: None,
                    author_date: None,
                });
            }
        } else {
            lost.push(LostCommit {
                sha,
                action: if action.is_empty() { None } else { Some(action) },
                timestamp,
                packed: true,
                subject: None,
                author: None,
                author_date: None,
            });
        }
    }

    // Stable sort: timestamp descending, then sha ascending.
    lost.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then(a.sha.cmp(&b.sha)));
    let total_lost_candidates = lost.len();
    lost.truncate(max);

    let summary = Summary {
        total_lost_candidates,
        stashes_count: stash.len(),
    };

    Ok(Output {
        head,
        stash_entries: stash,
        lost_commits: lost,
        summary,
    })
}

fn update_candidate(map: &mut HashMap<String, (i64, String)>, sha: String, timestamp: i64, message: &str) {
    map.entry(sha)
        .and_modify(|(t, m)| {
            if timestamp > *t {
                *t = timestamp;
                *m = message.to_string();
            }
        })
        .or_insert((timestamp, message.to_string()));
}

fn handle(input: Input) -> Result<Output, Refusal> {
    scan(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
