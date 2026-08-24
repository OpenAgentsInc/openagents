//! Foreign coding-agent session discovery, as a `packet-v0` guest plugin.
//!
//! The scanner half of OpenAgentsInc/openagents.com#198: given read-only
//! mounts over `~/.claude` (mount 0) and `~/.codex` (mount 1), report
//! recent session *metadata* — source, session id, working directory,
//! mtime, size, record count — and nothing else. Resuming a session is
//! deliberately not here; this plugin only says what exists.
//!
//! Foreign state is untrusted input, so the posture is the issue's:
//! read-only through the host's confined capability imports, bounded
//! everywhere (listing entries, per-file bytes, file reads per run,
//! directory listings per run, candidates, results), and fail-soft — a
//! missing directory contributes nothing, a malformed or unreadable file
//! is skipped and counted, a file over the host's per-file read bound is
//! reported from listing metadata alone and marked `metadata_truncated`.
//!
//! The guest has no clock on `wasm32-unknown-unknown`, so the age cutoff
//! runs against `now_ms` when the caller provides it, and otherwise
//! against the newest mtime the scan observed.

use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, MountDirListing, Refusal, RefusalCode,
};
use serde::{Deserialize, Serialize};

/// Mount indices, fixed by the order `manifest.json` declares the mounts.
const CLAUDE_MOUNT: u32 = 0;
const CODEX_MOUNT: u32 = 1;

const DEFAULT_MAX_AGE_DAYS: f64 = 30.0;
const DEFAULT_LIMIT: usize = 50;
/// Hard cap on `limit`; asking for more is answered with this many.
const LIMIT_CAP: usize = 50;
/// How many leading JSONL lines may be inspected for session metadata.
const META_SCAN_LINES: usize = 20;
/// File reads per invocation, across both sources.
const MAX_FILE_READS: usize = 200;
/// Directory listings per invocation, across both sources.
const MAX_DIR_LISTS: usize = 1500;
/// Candidate files held before sorting; beyond this the scan reports itself
/// truncated rather than growing without bound.
const MAX_CANDIDATES: usize = 5000;
const MS_PER_DAY: f64 = 86_400_000.0;

#[derive(Deserialize)]
pub struct Input {
    /// Which stores to scan; both when absent.
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// Substring the session's working directory must contain.
    #[serde(default)]
    pub cwd_filter: Option<String>,
    /// Sessions older than this are not reported. Default 30.
    #[serde(default)]
    pub max_age_days: Option<f64>,
    /// Most sessions to report, newest first. Default 50, capped at 50.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Milliseconds since the Unix epoch, for the age cutoff. The sandbox
    /// has no clock; when absent, the newest observed mtime stands in.
    #[serde(default)]
    pub now_ms: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Session {
    pub source: &'static str,
    pub session_id: String,
    /// Path relative to the source's mount root (`~/.claude` or `~/.codex`).
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Claude only: the encoded project directory the session file sits in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    pub mtime_ms: i64,
    pub size_bytes: u64,
    /// JSONL records in the file, when the file was small enough to read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_count: Option<usize>,
    /// True when the file exceeds the host's per-file read bound, so only
    /// the directory listing's metadata is known.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub metadata_truncated: bool,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Skipped {
    /// Readable files whose leading records held no usable metadata.
    pub malformed: usize,
    /// Files the host refused to read for any reason but size.
    pub unreadable: usize,
    /// Symlinked entries, which the host would refuse to follow.
    pub symlinked: usize,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub sessions: Vec<Session>,
    pub scanned_dirs: usize,
    pub scanned_files: usize,
    pub skipped: Skipped,
    /// Files reported from listing metadata alone (over the read bound).
    pub oversized: usize,
    /// Sources whose store was not present under its mount.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_sources: Vec<&'static str>,
    /// True when any directory listing hit the host's entry bound, or the
    /// scan hit its own listing/candidate bounds; the picture may be partial.
    pub scan_truncated: bool,
    /// True when the per-invocation file-read budget ran out before every
    /// surviving candidate could be inspected.
    pub read_budget_exhausted: bool,
}

/// The two host capabilities the scanner uses, as a seam so the scan logic
/// runs under `cargo test` against a fake host as well as inside the WASM
/// sandbox against the real one.
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

/// One file the listing pass found, before its bytes are inspected.
struct Candidate {
    source: &'static str,
    mount: u32,
    path: String,
    file_name: String,
    project_dir: Option<String>,
    mtime_ms: i64,
    size_bytes: u64,
}

/// Encode a string the way Claude Code encodes a cwd into a project
/// directory name: every character outside `[A-Za-z0-9]` becomes `-`.
pub fn dashed(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Claude session metadata from the file's leading records: the first
/// `cwd` and `sessionId` seen in the first [`META_SCAN_LINES`] lines, plus
/// the record count. `None` when no line yields a cwd.
pub fn claude_meta(bytes: &[u8]) -> Option<(String, Option<String>, usize)> {
    let text = String::from_utf8_lossy(bytes);
    let record_count = text.lines().count();
    let mut cwd = None;
    let mut session_id = None;
    for line in text.lines().take(META_SCAN_LINES) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if cwd.is_none() {
            if let Some(dir) = value.get("cwd").and_then(|v| v.as_str()) {
                cwd = Some(dir.to_string());
            }
        }
        if session_id.is_none() {
            if let Some(id) = value.get("sessionId").and_then(|v| v.as_str()) {
                session_id = Some(id.to_string());
            }
        }
        if cwd.is_some() && session_id.is_some() {
            break;
        }
    }
    cwd.map(|dir| (dir, session_id, record_count))
}

/// Codex rollout metadata from the first line's `session_meta` record:
/// `(cwd, session id, record count)`. `None` when the first line is not a
/// well-formed `session_meta` with a `cwd`.
pub fn codex_meta(bytes: &[u8]) -> Option<(String, Option<String>, usize)> {
    let text = String::from_utf8_lossy(bytes);
    let record_count = text.lines().count();
    let first = text.lines().next()?;
    let value = serde_json::from_str::<serde_json::Value>(first).ok()?;
    if value.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return None;
    }
    let payload = value.get("payload")?;
    let cwd = payload.get("cwd").and_then(|v| v.as_str())?.to_string();
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some((cwd, id, record_count))
}

/// A session file's stem: the name without its `.jsonl` suffix.
fn stem(name: &str) -> String {
    name.strip_suffix(".jsonl").unwrap_or(name).to_string()
}

/// The whole scan, over any [`Host`]. Total: every path returns an output.
pub fn scan(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let sources = match &input.sources {
        None => vec!["claude", "codex"],
        Some(named) => {
            let mut sources = Vec::new();
            for name in named {
                match name.as_str() {
                    "claude" => sources.push("claude"),
                    "codex" => sources.push("codex"),
                    other => {
                        return Err(Refusal::unsupported(format!(
                            "unknown source `{other}`; this scanner knows `claude` and `codex`"
                        )))
                    }
                }
            }
            sources
        }
    };
    let max_age_days = input
        .max_age_days
        .filter(|days| days.is_finite() && *days > 0.0)
        .unwrap_or(DEFAULT_MAX_AGE_DAYS);
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, LIMIT_CAP);

    let mut out = Output {
        sessions: Vec::new(),
        scanned_dirs: 0,
        scanned_files: 0,
        skipped: Skipped::default(),
        oversized: 0,
        missing_sources: Vec::new(),
        scan_truncated: false,
        read_budget_exhausted: false,
    };
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut dir_lists = 0usize;

    // A listing whose store directory is absent means the source is not on
    // this machine; any other listing failure also fails soft.
    let mut list = |out: &mut Output,
                    mount: u32,
                    path: &str|
     -> Option<MountDirListing> {
        if dir_lists >= MAX_DIR_LISTS {
            out.scan_truncated = true;
            return None;
        }
        dir_lists += 1;
        match host.list(mount, path) {
            Ok(listing) => {
                out.scanned_dirs += 1;
                if listing.truncated {
                    out.scan_truncated = true;
                }
                Some(listing)
            }
            Err(_) => None,
        }
    };

    let push = |out: &mut Output, candidates: &mut Vec<Candidate>, candidate: Candidate| {
        out.scanned_files += 1;
        if candidates.len() < MAX_CANDIDATES {
            candidates.push(candidate);
        } else {
            out.scan_truncated = true;
        }
    };

    for source in &sources {
        match *source {
            "claude" => {
                // ~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl
                let Some(projects) = list(&mut out, CLAUDE_MOUNT, "projects") else {
                    out.missing_sources.push("claude");
                    continue;
                };
                for project in &projects.entries {
                    match project.kind.as_str() {
                        "dir" => {}
                        "symlink" => {
                            out.skipped.symlinked += 1;
                            continue;
                        }
                        _ => continue,
                    }
                    let dir_path = format!("projects/{}", project.name);
                    let Some(files) = list(&mut out, CLAUDE_MOUNT, &dir_path) else {
                        continue;
                    };
                    for file in &files.entries {
                        if file.kind == "symlink" {
                            out.skipped.symlinked += 1;
                            continue;
                        }
                        if file.kind != "file" || !file.name.ends_with(".jsonl") {
                            continue;
                        }
                        push(
                            &mut out,
                            &mut candidates,
                            Candidate {
                                source: "claude",
                                mount: CLAUDE_MOUNT,
                                path: format!("{dir_path}/{}", file.name),
                                file_name: file.name.clone(),
                                project_dir: Some(project.name.clone()),
                                mtime_ms: file.mtime_ms,
                                size_bytes: file.size,
                            },
                        );
                    }
                }
            }
            "codex" => {
                // ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl. The
                // `state_*.sqlite` index beside it is out of scope for this
                // slice; see the plugin README.
                let Some(years) = list(&mut out, CODEX_MOUNT, "sessions") else {
                    out.missing_sources.push("codex");
                    continue;
                };
                for year in dirs_of(&years, &mut out.skipped) {
                    let year_path = format!("sessions/{year}");
                    let Some(months) = list(&mut out, CODEX_MOUNT, &year_path) else {
                        continue;
                    };
                    for month in dirs_of(&months, &mut out.skipped) {
                        let month_path = format!("{year_path}/{month}");
                        let Some(days) = list(&mut out, CODEX_MOUNT, &month_path) else {
                            continue;
                        };
                        for day in dirs_of(&days, &mut out.skipped) {
                            let day_path = format!("{month_path}/{day}");
                            let Some(files) = list(&mut out, CODEX_MOUNT, &day_path) else {
                                continue;
                            };
                            for file in &files.entries {
                                if file.kind == "symlink" {
                                    out.skipped.symlinked += 1;
                                    continue;
                                }
                                if file.kind != "file"
                                    || !file.name.starts_with("rollout-")
                                    || !file.name.ends_with(".jsonl")
                                {
                                    continue;
                                }
                                push(
                                    &mut out,
                                    &mut candidates,
                                    Candidate {
                                        source: "codex",
                                        mount: CODEX_MOUNT,
                                        path: format!("{day_path}/{}", file.name),
                                        file_name: file.name.clone(),
                                        project_dir: None,
                                        mtime_ms: file.mtime_ms,
                                        size_bytes: file.size,
                                    },
                                );
                            }
                        }
                    }
                }
            }
            _ => unreachable!("sources were validated above"),
        }
    }

    // The age cutoff: `now` is the caller's clock, or the newest thing seen.
    let now_ms = input
        .now_ms
        .or_else(|| candidates.iter().map(|c| c.mtime_ms).max())
        .unwrap_or(0);
    let cutoff_ms = now_ms - (max_age_days * MS_PER_DAY) as i64;
    candidates.retain(|c| c.mtime_ms >= cutoff_ms);
    candidates.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms).then(a.path.cmp(&b.path)));

    let dashed_filter = input.cwd_filter.as_deref().map(dashed);
    let mut reads = 0usize;

    for candidate in &candidates {
        if out.sessions.len() >= limit {
            break;
        }
        // Claude's project directory name encodes the cwd, so a filter can
        // rule a candidate out before spending a read on it.
        if let (Some(filter), Some(project_dir)) = (&dashed_filter, &candidate.project_dir) {
            if !dashed(project_dir).contains(filter.as_str()) {
                continue;
            }
        }
        if reads >= MAX_FILE_READS {
            out.read_budget_exhausted = true;
            break;
        }
        reads += 1;
        // `read` addresses the mounts in declaration order; the full
        // relative path (projects/... vs sessions/...) exists in exactly
        // one of them. `candidate.mount` records intent for the reader.
        let _ = candidate.mount;
        match host.read(&candidate.path) {
            Ok(bytes) => {
                let meta = match candidate.source {
                    "claude" => claude_meta(&bytes),
                    _ => codex_meta(&bytes),
                };
                let Some((cwd, session_id, record_count)) = meta else {
                    out.skipped.malformed += 1;
                    continue;
                };
                if let Some(filter) = input.cwd_filter.as_deref() {
                    if !cwd.contains(filter) && !dashed(&cwd).contains(&dashed(filter)) {
                        continue;
                    }
                }
                out.sessions.push(Session {
                    source: candidate.source,
                    session_id: session_id.unwrap_or_else(|| stem(&candidate.file_name)),
                    path: candidate.path.clone(),
                    cwd: Some(cwd),
                    project_dir: candidate.project_dir.clone(),
                    mtime_ms: candidate.mtime_ms,
                    size_bytes: candidate.size_bytes,
                    record_count: Some(record_count),
                    metadata_truncated: false,
                });
            }
            Err(refusal) if refusal.code == RefusalCode::FileTooLarge => {
                out.oversized += 1;
                // Only the listing's metadata is known. With a cwd filter, a
                // Claude candidate already passed the project-name prefilter;
                // a Codex candidate's cwd is unknowable here, so the filter
                // excludes it rather than guessing.
                if dashed_filter.is_some() && candidate.project_dir.is_none() {
                    continue;
                }
                out.sessions.push(Session {
                    source: candidate.source,
                    session_id: stem(&candidate.file_name),
                    path: candidate.path.clone(),
                    cwd: None,
                    project_dir: candidate.project_dir.clone(),
                    mtime_ms: candidate.mtime_ms,
                    size_bytes: candidate.size_bytes,
                    record_count: None,
                    metadata_truncated: true,
                });
            }
            Err(_) => {
                out.skipped.unreadable += 1;
            }
        }
    }

    Ok(out)
}

/// Directory names in a listing, counting symlinks as skipped.
fn dirs_of<'l>(listing: &'l MountDirListing, skipped: &mut Skipped) -> Vec<&'l str> {
    let mut dirs = Vec::new();
    for entry in &listing.entries {
        match entry.kind.as_str() {
            "dir" => dirs.push(entry.name.as_str()),
            "symlink" => skipped.symlinked += 1,
            _ => {}
        }
    }
    dirs
}

fn handle(input: Input) -> Result<Output, Refusal> {
    scan(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
