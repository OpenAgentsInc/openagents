//! `/resume`: recent Claude Code and Codex sessions from their own local stores.
//!
//! This is the Rust port of `packages/openagents-cli/src/coder-foreign-resume.ts`,
//! the picker half of the foreign-session feature. The scanner half is the
//! `packet-v0` WebAssembly guest at `plugins/foreign-sessions`, run under the
//! host in [`crate::plugins`]: read-only mounts over `~/.claude` and `~/.codex`,
//! a pinned digest, a memory ceiling and a deadline. This module builds the
//! bounded scan packet, interprets the metadata-only answer, renders a numbered
//! list, and prints the command that resumes one session in the tool that owns
//! it.
//!
//! # What is surfaced is the session on disk
//!
//! The scanner reports metadata, not a transcript, and this module never
//! reconstructs one. Before a resume command is printed, the reported path is
//! resolved against the declared mount roots and the file is opened here, in
//! the host, and the session id the scanner reported is looked for in the
//! file's own leading records. A session whose file is not there, cannot be
//! read, or does not carry the reported id is **refused by name with the
//! path** — not rendered from the scan alone, and never replaced by an empty or
//! invented session. That is the whole point: this crate has previously shipped
//! a hardcoded identity seed, an invented forum board, and two fabricated trace
//! sessions, and every one of them reached a user.
//!
//! The one case where the id does not come from the records is a file over the
//! host's per-file read bound. There the scanner takes the id from the file
//! name, and this module says so on the line it prints rather than implying a
//! read that did not happen.
//!
//! # What is surfaced is redacted
//!
//! These are other agents' session stores, and a working directory or a file
//! path read out of one is untrusted content. Every foreign-derived string goes
//! through [`crate::trace::redact_text`] — the rules this CLI already shares
//! with `packages/atif/src/redaction.ts` — before it reaches the transcript.
//!
//! Redaction and a runnable command pull in opposite directions, so the rule is
//! explicit rather than split the difference: a working directory whose only
//! redaction is the home-path rewrite is rebuilt as `"$HOME/..."`, which a shell
//! expands back to exactly the directory the file recorded. Anything else
//! removed from the path means no command is printed at all, with the categories
//! named. A command that would `cd` somewhere other than where the session ran
//! is worse than no command.
//!
//! # Shell safety
//!
//! The TypeScript renders `cd "${cwd}" && claude --resume ${id}` from fields
//! read straight out of a foreign file. A session file that records a cwd of
//! `"; rm -rf ~; #` produces a line that does that when pasted. Both fields are
//! checked here before they are interpolated, and a value that is not safe to
//! quote is refused by name.

use crate::plugins::{self, Approval, CatalogEntry, LoadedPlugin};
use crate::trace::redact_text;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

/// How far back the picker looks, in days, unless the caller says otherwise.
pub const DEFAULT_MAX_AGE_DAYS: f64 = 30.0;
/// How many sessions the picker asks for, unless the caller says otherwise.
pub const DEFAULT_PICKER_LIMIT: usize = 10;
/// The catalog name of the scanner this picker drives.
pub const SCANNER_NAME: &str = "foreign_sessions";

/// Leading records inspected when confirming a scanner-reported session id.
///
/// The same bound the guest uses (`META_SCAN_LINES`), so a file whose id the
/// scanner found is a file whose id this confirms.
const VERIFY_SCAN_LINES: usize = 20;

/// Most bytes read back when confirming a session id. The guest's own per-file
/// bound is 1 MiB and it gives up past that; there is no reason to read more
/// here than the side being checked could have seen.
const VERIFY_READ_BYTES: u64 = 1024 * 1024;

// ───────────────────────────────────────────────────────────── the scan answer

/// Which foreign tool owns a session. Only the two the manifest mounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignSource {
    Claude,
    Codex,
}

impl ForeignSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    /// The binary and verb that resume a session in this tool.
    pub fn resume_verb(self) -> &'static str {
        match self {
            Self::Claude => "claude --resume",
            Self::Codex => "codex resume",
        }
    }
}

/// One session the scanner reported. Metadata only; no transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignSession {
    pub source: ForeignSource,
    pub session_id: String,
    /// Relative to the store's mount root, as the guest reports it.
    pub path: String,
    pub cwd: Option<String>,
    pub project_dir: Option<String>,
    pub mtime_ms: i64,
    pub size_bytes: u64,
    pub record_count: Option<usize>,
    /// The file was over the host's read bound, so only its listing metadata
    /// is known — including the session id, which came from the file name.
    pub metadata_truncated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Skipped {
    pub malformed: usize,
    pub unreadable: usize,
    pub symlinked: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignScanOutput {
    pub sessions: Vec<ForeignSession>,
    pub scanned_dirs: usize,
    pub scanned_files: usize,
    pub skipped: Skipped,
    pub oversized: usize,
    pub missing_sources: Vec<String>,
    pub scan_truncated: bool,
    pub read_budget_exhausted: bool,
    /// Rows whose `source` this picker cannot resume, counted by name.
    ///
    /// The TypeScript drops these silently. A dropped row that nothing accounts
    /// for is a listing that quietly disagrees with the store it scanned, so
    /// they are counted and reported instead.
    pub unsupported_sources: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignScanRefusal {
    pub code: String,
    pub reason: String,
}

/// What one `/resume` turn is answered against.
#[derive(Debug, Clone)]
pub struct ForeignResumeDeps {
    pub now_ms: i64,
    /// The session's working directory, used as the scanner's cwd filter.
    pub cwd: String,
    /// `None` lists; `Some(n)` describes the nth listed session.
    pub selection: Option<usize>,
    /// The invoking user's home, for the redaction rules.
    pub home: String,
    /// The store roots the scanner was mounted on, in declaration order. A
    /// reported relative path is resolved against these and nothing else.
    pub mount_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct ForeignResumeOptions {
    pub max_age_days: Option<f64>,
    pub limit: Option<usize>,
}

// ───────────────────────────────────────────────────────────────── the parsing

fn as_str(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn as_i64(value: Option<&Value>) -> i64 {
    value.and_then(Value::as_f64).map_or(0, |n| n as i64)
}

fn as_usize(value: Option<&Value>) -> usize {
    value
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map_or(0, |n| n as usize)
}

fn optional_string(record: &Value, key: &str) -> Option<String> {
    match record.get(key) {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_str().unwrap_or_default().to_string()),
    }
}

fn optional_usize(record: &Value, key: &str) -> Option<usize> {
    match record.get(key) {
        None | Some(Value::Null) => None,
        Some(value) => Some(as_usize(Some(value))),
    }
}

/// Read one session row. `Err(name)` is a row whose source this cannot resume.
fn parse_session(value: &Value) -> Result<ForeignSession, String> {
    if !value.is_object() {
        return Err(String::new());
    }
    let raw_source = as_str(value.get("source"));
    let Some(source) = ForeignSource::parse(&raw_source) else {
        return Err(raw_source);
    };
    Ok(ForeignSession {
        source,
        session_id: as_str(value.get("session_id")),
        path: as_str(value.get("path")),
        cwd: optional_string(value, "cwd"),
        project_dir: optional_string(value, "project_dir"),
        mtime_ms: as_i64(value.get("mtime_ms")),
        size_bytes: as_i64(value.get("size_bytes")).max(0) as u64,
        record_count: optional_usize(value, "record_count"),
        metadata_truncated: value.get("metadata_truncated") == Some(&Value::Bool(true)),
    })
}

fn parse_scan_output(value: &Value) -> ForeignScanOutput {
    let empty = Vec::new();
    let rows = value
        .get("sessions")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    let mut sessions = Vec::new();
    let mut unsupported_sources: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        match parse_session(row) {
            Ok(session) => sessions.push(session),
            Err(name) if name.is_empty() => {}
            Err(name) => *unsupported_sources.entry(name).or_insert(0) += 1,
        }
    }

    let skipped = value.get("skipped").cloned().unwrap_or(Value::Null);
    let missing_sources = value
        .get("missing_sources")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    ForeignScanOutput {
        sessions,
        scanned_dirs: as_usize(value.get("scanned_dirs")),
        scanned_files: as_usize(value.get("scanned_files")),
        skipped: Skipped {
            malformed: as_usize(skipped.get("malformed")),
            unreadable: as_usize(skipped.get("unreadable")),
            symlinked: as_usize(skipped.get("symlinked")),
        },
        oversized: as_usize(value.get("oversized")),
        missing_sources,
        scan_truncated: value.get("scan_truncated") == Some(&Value::Bool(true)),
        read_budget_exhausted: value.get("read_budget_exhausted") == Some(&Value::Bool(true)),
        unsupported_sources,
    }
}

/// What one invocation came back as.
#[derive(Debug, Clone)]
pub enum ScanResult {
    Ok(Box<ForeignScanOutput>),
    Refusal(ForeignScanRefusal),
    Error(String),
}

/// Sort a raw packet into the three shapes a caller can act on.
pub fn normalize_scan_result(value: &Value) -> ScanResult {
    if !value.is_object() {
        return ScanResult::Error("The scanner returned an unrecognised packet.".to_string());
    }
    if let Some(refusal) = value.get("refusal").filter(|v| !v.is_null()) {
        let code = as_str(refusal.get("code"));
        let reason = as_str(refusal.get("reason"));
        if !code.is_empty() && !reason.is_empty() {
            return ScanResult::Refusal(ForeignScanRefusal { code, reason });
        }
        return ScanResult::Error("The scanner returned a malformed refusal.".to_string());
    }
    if let Some(ok) = value.get("ok").filter(|v| !v.is_null()) {
        return ScanResult::Ok(Box::new(parse_scan_output(ok)));
    }
    ScanResult::Error("The scanner returned an unrecognised packet.".to_string())
}

/// The packet the guest's `interface.input` schema describes.
pub fn build_packet(deps: &ForeignResumeDeps, options: &ForeignResumeOptions) -> Value {
    json!({
        "now_ms": deps.now_ms,
        "cwd_filter": deps.cwd,
        "max_age_days": options.max_age_days.unwrap_or(DEFAULT_MAX_AGE_DAYS),
        "limit": options.limit.unwrap_or(DEFAULT_PICKER_LIMIT),
    })
}

// ───────────────────────────────────────────────────────────────── the rendering

/// `5 days ago`, `3 hours ago`, or `just now`.
pub fn format_age(mtime_ms: i64, now_ms: i64) -> String {
    let diff = (now_ms - mtime_ms).max(0);
    let days = diff / DAY_MS;
    if days >= 1 {
        return format!("{days} day{} ago", if days == 1 { "" } else { "s" });
    }
    let hours = diff / HOUR_MS;
    if hours >= 1 {
        return format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" });
    }
    "just now".to_string()
}

/// Run one string from a foreign store through the shared redaction rules.
fn hide(value: &str, home: &str) -> String {
    redact_text(value, home).text
}

/// A recorded working directory, as it is shown to a reader.
///
/// Redacted first. Then, if what is left could not go inside a quoted shell
/// word, it is shown as an escaped literal — `"/tmp/x\"; rm -rf ~; #"` rather
/// than the bare bytes. The exact recorded value is still on the screen; it
/// just cannot be mistaken for something to run. A session file's `cwd` is
/// written by whatever agent owned that session, and the line above it says
/// `cd`.
fn show_cwd(value: &str, home: &str) -> String {
    let hidden = hide(value, home);
    if quotable(&hidden) {
        hidden
    } else {
        format!("{hidden:?}")
    }
}

/// True when a value can be put inside a double-quoted shell word and mean
/// itself. Deliberately narrow: the input is a field from someone else's file.
fn quotable(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\\' | '$' | '`' | '\n' | '\r'))
}

/// True when a value is shaped like a session id: what a UUID or a file stem
/// is made of, and nothing a shell would look at twice.
fn id_shaped(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// How a working directory can appear inside a resume command.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ShellCwd {
    /// The session recorded no working directory.
    Unknown,
    /// Nothing was redacted; the literal path is quoted.
    Literal(String),
    /// Only the home rewrite applied, and `$HOME` puts it back exactly.
    Home(String),
    /// Redaction removed these categories, so no path can be printed.
    Redacted(Vec<String>),
    /// The recorded path cannot be safely quoted.
    Unquotable,
}

/// Decide how — or whether — a recorded working directory can be printed into a
/// command the reader is invited to run.
fn shell_cwd(cwd: Option<&str>, home: &str) -> ShellCwd {
    let Some(cwd) = cwd.filter(|value| !value.is_empty()) else {
        return ShellCwd::Unknown;
    };
    if !quotable(cwd) {
        return ShellCwd::Unquotable;
    }
    let redaction = redact_text(cwd, home);
    if redaction.total == 0 {
        return ShellCwd::Literal(cwd.to_string());
    }
    let only_home = redaction
        .counts
        .keys()
        .all(|category| category == "home_path");
    // `$HOME` is accepted only when the rewrite is a leading one, there is
    // exactly one of them, and expanding it back yields the recorded path
    // character for character. Anything less is a guess about where a session
    // ran, and this prints no guesses. `strip_prefix` and not `text[1..]`:
    // `/opt/Users/ada/x` also redacts to a string with a `~` in it, and slicing
    // by one byte would both mangle that path and split a leading multi-byte
    // character in half.
    if only_home
        && !home.is_empty()
        && let Some(rest) = redaction.text.strip_prefix('~')
    {
        return ShellCwd::Home(format!("$HOME{rest}"));
    }
    ShellCwd::Redacted(redaction.counts.keys().cloned().collect())
}

fn describe_session(session: &ForeignSession, now_ms: i64, home: &str) -> String {
    let age = format_age(session.mtime_ms, now_ms);
    let records = match session.record_count {
        None => "metadata only".to_string(),
        Some(count) => format!("{count} records"),
    };
    let truncated = if session.metadata_truncated {
        " · truncated"
    } else {
        ""
    };
    let cwd = match session.cwd.as_deref().filter(|value| !value.is_empty()) {
        Some(value) => show_cwd(value, home),
        None => "(cwd unknown)".to_string(),
    };
    format!(
        "{:<6}  {}  {}  {}  {}{}",
        session.source.as_str(),
        hide(&session.session_id, home),
        cwd,
        age,
        records,
        truncated
    )
}

fn scan_notes(output: &ForeignScanOutput) -> Vec<String> {
    let mut notes = Vec::new();
    if output.scan_truncated {
        notes.push("The scan hit a bound and may be partial.".to_string());
    }
    if output.read_budget_exhausted {
        notes.push(
            "The file-read budget was exhausted; some sessions may be metadata-only.".to_string(),
        );
    }
    for (name, count) in &output.unsupported_sources {
        notes.push(format!(
            "{count} session{} from `{name}` {} left out: this picker resumes `claude` and `codex` only.",
            if *count == 1 { "" } else { "s" },
            if *count == 1 { "was" } else { "were" }
        ));
    }
    notes
}

fn describe_list(output: &ForeignScanOutput, deps: &ForeignResumeDeps) -> String {
    let header = format!(
        "Recent foreign sessions for this directory ({}):",
        hide(&deps.cwd, &deps.home)
    );

    if output.sessions.is_empty() {
        let mut reasons = Vec::new();
        if !output.missing_sources.is_empty() {
            reasons.push(format!(
                "the scanner could not read the {} state store",
                output.missing_sources.join(" or ")
            ));
        }
        if output.scan_truncated {
            reasons.push("the scan was truncated".to_string());
        }
        if output.read_budget_exhausted {
            reasons.push("the file-read budget was exhausted".to_string());
        }
        let reason = if reasons.is_empty() {
            String::new()
        } else {
            format!(" ({})", reasons.join("; "))
        };
        let notes = scan_notes(output);
        let tail = if notes.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", notes.join(" "))
        };
        return format!("{header}\n\nNo recent foreign sessions were found{reason}.{tail}");
    }

    let lines = output
        .sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            format!(
                "  {:>2}. {}",
                index + 1,
                describe_session(session, deps.now_ms, &deps.home)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let notes = scan_notes(output);
    let note = if notes.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", notes.join(" "))
    };

    format!(
        "{header}\n\n{lines}\n\nRun /resume <number> to see the resume command for that session.{note}"
    )
}

// ───────────────────────────────────────────────── proving the file is there

/// What confirming a reported session against its file on disk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnDisk {
    /// The file exists and its own leading records carry the reported id.
    Confirmed { path: PathBuf },
    /// The file exists and the scanner took the id from its name, because the
    /// file is over the host's per-file read bound.
    FromFileName { path: PathBuf },
    /// The file exists and does not carry the reported id.
    Mismatch { path: PathBuf },
    /// No declared store root holds the reported path.
    Missing {
        relative: String,
        roots: Vec<PathBuf>,
    },
    /// The file is there and could not be read.
    Unreadable { path: PathBuf, error: String },
}

/// Join a scanner-reported relative path onto a mount root without letting it
/// leave. `..`, absolute paths, and empty components are refused outright — the
/// path came out of a file this process does not control.
fn under_root(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.starts_with('/') {
        return None;
    }
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// Read the leading records of a session file and say whether the reported id
/// is in them.
///
/// Claude records it as `sessionId` on a top-level record; Codex records it as
/// `payload.id` on the first `session_meta` line. Both are read here rather
/// than taken from the scanner, because taking it from the scanner would prove
/// only that the scanner is self-consistent.
fn file_carries_id(path: &Path, session_id: &str) -> std::io::Result<bool> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; VERIFY_READ_BYTES as usize];
    let mut filled = 0usize;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            got => filled += got,
        }
    }
    buffer.truncate(filled);
    let text = String::from_utf8_lossy(&buffer);

    for line in text.lines().take(VERIFY_SCAN_LINES) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("sessionId").and_then(Value::as_str) == Some(session_id) {
            return Ok(true);
        }
        if value.get("type").and_then(Value::as_str) == Some("session_meta")
            && value
                .get("payload")
                .and_then(|p| p.get("id"))
                .and_then(Value::as_str)
                == Some(session_id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Resolve a reported session to the file it names and confirm what is in it.
pub fn confirm_on_disk(session: &ForeignSession, roots: &[PathBuf]) -> OnDisk {
    let found = roots
        .iter()
        .filter_map(|root| under_root(root, &session.path))
        .find(|candidate| candidate.is_file());

    let Some(path) = found else {
        return OnDisk::Missing {
            relative: session.path.clone(),
            roots: roots.to_vec(),
        };
    };

    // Over the read bound the guest never opened the file; the id is the file
    // stem. Confirm exactly that claim and no more.
    if session.metadata_truncated {
        let stem = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = stem.strip_suffix(".jsonl").unwrap_or(&stem);
        return if stem == session.session_id {
            OnDisk::FromFileName { path }
        } else {
            OnDisk::Mismatch { path }
        };
    }

    match file_carries_id(&path, &session.session_id) {
        Ok(true) => OnDisk::Confirmed { path },
        Ok(false) => OnDisk::Mismatch { path },
        Err(error) => OnDisk::Unreadable {
            path,
            error: error.to_string(),
        },
    }
}

fn describe_selection(session: &ForeignSession, deps: &ForeignResumeDeps) -> String {
    let home = deps.home.as_str();
    let on_disk = confirm_on_disk(session, &deps.mount_roots);

    let (path, provenance) = match &on_disk {
        OnDisk::Confirmed { path } => (
            path.clone(),
            "the session id was read back out of this file's own records",
        ),
        OnDisk::FromFileName { path } => (
            path.clone(),
            "the file is over the scanner's read bound, so the session id is its file name and \
             nothing was read from inside it",
        ),
        OnDisk::Missing { relative, roots } => {
            let tried = roots
                .iter()
                .map(|root| hide(&root.to_string_lossy(), home))
                .collect::<Vec<_>>()
                .join(", ");
            let tried = if tried.is_empty() {
                "no store root was mounted".to_string()
            } else {
                tried
            };
            return format!(
                "The scanner reported {}, and no mounted store holds it (tried: {tried}).\n\n\
                 Nothing is resumed from a path that is not there.",
                hide(relative, home)
            );
        }
        OnDisk::Mismatch { path } => {
            return format!(
                "{} does not carry the session id the scanner reported ({}).\n\n\
                 No resume command is printed for a session this file does not hold.",
                hide(&path.to_string_lossy(), home),
                hide(&session.session_id, home)
            );
        }
        OnDisk::Unreadable { path, error } => {
            return format!(
                "{} could not be read: {error}.\n\n\
                 No resume command is printed for a session that could not be confirmed.",
                hide(&path.to_string_lossy(), home)
            );
        }
    };

    let age = format_age(session.mtime_ms, deps.now_ms);
    let mut lines = vec![
        "Resume context:".to_string(),
        format!("  source:      {}", session.source.as_str()),
        format!("  session id:  {}", hide(&session.session_id, home)),
        format!("  file:        {}", hide(&path.to_string_lossy(), home)),
        format!(
            "  cwd:         {}",
            match session.cwd.as_deref().filter(|value| !value.is_empty()) {
                Some(value) => show_cwd(value, home),
                None => "(unknown)".to_string(),
            }
        ),
        format!("  age:         {age}"),
        format!(
            "  records:     {}",
            match session.record_count {
                Some(count) => count.to_string(),
                None => "(unknown)".to_string(),
            }
        ),
    ];
    if session.metadata_truncated {
        lines.push("  metadata:    truncated".to_string());
    }
    lines.push(format!("  confirmed:   {provenance}"));
    lines.push(String::new());

    if !id_shaped(&session.session_id) {
        lines.push(
            "The recorded session id is not shaped like one, so it is not put on a command line."
                .to_string(),
        );
        return lines.join("\n");
    }

    let verb = session.source.resume_verb();
    let id = &session.session_id;
    match shell_cwd(session.cwd.as_deref(), home) {
        ShellCwd::Literal(cwd) => {
            lines.push("Run this to resume in the foreign tool:".to_string());
            lines.push(format!("  cd \"{cwd}\" && {verb} {id}"));
        }
        ShellCwd::Home(cwd) => {
            lines.push("Run this to resume in the foreign tool:".to_string());
            lines.push(format!("  cd \"{cwd}\" && {verb} {id}"));
        }
        ShellCwd::Unknown => {
            lines.push(
                "The session recorded no working directory, so run this from wherever it ran:"
                    .to_string(),
            );
            lines.push(format!("  {verb} {id}"));
        }
        ShellCwd::Redacted(categories) => {
            lines.push(format!(
                "The recorded working directory carries material the redaction rules remove ({}), \
                 so no `cd` is printed. Resume from the directory the session file above sits under:",
                categories.join(", ")
            ));
            lines.push(format!("  {verb} {id}"));
        }
        ShellCwd::Unquotable => {
            lines.push(
                "The recorded working directory cannot be safely quoted into a shell command, so \
                 no `cd` is printed. Resume from the directory the session file above sits under:"
                    .to_string(),
            );
            lines.push(format!("  {verb} {id}"));
        }
    }

    lines.join("\n")
}

// ─────────────────────────────────────────────────────────────────── the turn

/// The seam the scanner is reached through. A test stands a fake here; the real
/// call is [`scanner_invoke`].
pub type ForeignResumeInvoke<'a> = &'a dyn Fn(&Value) -> Result<Value, String>;

/// Run one `/resume` turn and return the single notice to put on the transcript.
pub fn run_foreign_resume(
    deps: &ForeignResumeDeps,
    invoke: ForeignResumeInvoke<'_>,
    options: &ForeignResumeOptions,
) -> String {
    let packet = build_packet(deps, options);

    let raw = match invoke(&packet) {
        Ok(value) => value,
        Err(error) => return format!("The scanner could not run: {error}"),
    };

    let output = match normalize_scan_result(&raw) {
        ScanResult::Error(message) => return message,
        ScanResult::Refusal(refusal) => {
            return format!("The scanner refused ({}): {}", refusal.code, refusal.reason);
        }
        ScanResult::Ok(output) => *output,
    };

    let Some(selection) = deps.selection else {
        return describe_list(&output, deps);
    };

    if selection < 1 || selection > output.sessions.len() {
        let hint = if output.sessions.is_empty() {
            String::new()
        } else {
            format!(" Choose a number from 1 to {}.", output.sessions.len())
        };
        return format!(
            "There is no session at {selection}.{hint}\n\n{}",
            describe_list(&output, deps)
        );
    }

    describe_selection(&output.sessions[selection - 1], deps)
}

// ──────────────────────────────────────────────────────── driving the real one

/// Find the scanner in the plugin catalog and load it.
///
/// Every failure names the capability and where it was looked for, because the
/// alternative — an empty listing — is indistinguishable from a machine with no
/// foreign sessions on it.
pub fn load_scanner(from: &Path) -> Result<LoadedPlugin, String> {
    let catalog = plugins::discover_catalog(from);
    let Some(entry) = catalog
        .iter()
        .find(|entry: &&CatalogEntry| entry.name == SCANNER_NAME)
    else {
        return Err(format!(
            "The `{SCANNER_NAME}` capability is not installed: no `plugins/*/manifest.json` \
             declaring it was found from {} upward. Nothing was scanned.",
            from.display()
        ));
    };

    // Typing `/resume` is the operator action the mount tier asks for, and the
    // notice says which directories were read.
    Approval {
        mounts_allowed: true,
    }
    .check(entry)
    .map_err(|refusal| {
        format!("The `{SCANNER_NAME}` capability would not load {refusal}. Nothing was scanned.")
    })?;

    plugins::load_plugin(&entry.manifest_path, from).map_err(|refusal| {
        format!(
            "The `{SCANNER_NAME}` capability at {} would not load {refusal}. Nothing was scanned.",
            entry.manifest_path.display()
        )
    })
}

/// The real seam: one blocking `packet-v0` invocation of the loaded scanner.
pub fn scanner_invoke(plugin: &LoadedPlugin) -> impl Fn(&Value) -> Result<Value, String> + '_ {
    move |packet: &Value| {
        let bytes = serde_json::to_vec(packet).map_err(|error| error.to_string())?;
        let answer = plugins::invoke(plugin, &bytes).map_err(|refusal| refusal.to_string())?;
        serde_json::from_slice(&answer)
            .map_err(|error| format!("the scanner's answer was not JSON: {error}"))
    }
}

/// One whole `/resume` turn against the machine's real stores.
///
/// Blocking: the wasm invocation is synchronous. Call it off the UI thread.
pub fn foreign_resume_turn(cwd: &Path, home: &Path, selection: Option<usize>) -> String {
    let plugin = match load_scanner(cwd) {
        Ok(plugin) => plugin,
        Err(message) => return message,
    };
    let deps = ForeignResumeDeps {
        now_ms: now_ms(),
        cwd: cwd.to_string_lossy().into_owned(),
        selection,
        home: home.to_string_lossy().into_owned(),
        mount_roots: plugin.mounts.clone(),
    };
    let invoke = scanner_invoke(&plugin);
    let body = run_foreign_resume(&deps, &invoke, &ForeignResumeOptions::default());

    let roots = plugin
        .mounts
        .iter()
        .map(|root| hide(&root.to_string_lossy(), &deps.home))
        .collect::<Vec<_>>()
        .join(", ");
    if roots.is_empty() {
        body
    } else {
        format!("{body}\n\nRead read-only from: {roots}.")
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_the_way_the_typescript_reports_them() {
        let now = 1_000_000_000_000i64;
        assert_eq!(format_age(now - 5 * DAY_MS, now), "5 days ago");
        assert_eq!(format_age(now - DAY_MS, now), "1 day ago");
        assert_eq!(format_age(now - 3 * HOUR_MS, now), "3 hours ago");
        assert_eq!(format_age(now - HOUR_MS, now), "1 hour ago");
        assert_eq!(format_age(now - 1000, now), "just now");
        // A clock that moved backwards is not a negative age.
        assert_eq!(format_age(now + DAY_MS, now), "just now");
    }

    #[test]
    fn a_reported_path_cannot_climb_out_of_its_mount() {
        let root = Path::new("/store");
        assert_eq!(
            under_root(root, "projects/a.jsonl"),
            Some(PathBuf::from("/store/projects/a.jsonl"))
        );
        assert_eq!(under_root(root, "../../etc/passwd"), None);
        assert_eq!(under_root(root, "projects/../../etc/passwd"), None);
        assert_eq!(under_root(root, "/etc/passwd"), None);
        assert_eq!(under_root(root, ""), None);
    }

    #[test]
    fn a_working_directory_becomes_a_command_only_when_it_round_trips() {
        let home = "/Users/ada";
        assert_eq!(
            shell_cwd(Some("/Users/ada/work"), home),
            ShellCwd::Home("$HOME/work".to_string())
        );
        // Nothing to hide: printed as it stands.
        assert_eq!(
            shell_cwd(Some("/srv/build"), home),
            ShellCwd::Literal("/srv/build".to_string())
        );
        assert_eq!(shell_cwd(None, home), ShellCwd::Unknown);
        assert_eq!(shell_cwd(Some(""), home), ShellCwd::Unknown);
        // A shell metacharacter never reaches a command line.
        assert_eq!(
            shell_cwd(Some("/tmp/x\"; rm -rf ~; #"), home),
            ShellCwd::Unquotable
        );
        assert_eq!(shell_cwd(Some("/tmp/$(id)"), home), ShellCwd::Unquotable);
        // A home rewrite that is not the leading one cannot be put back with
        // `$HOME`, and a byte-index slice would have produced `$HOMEopt~/x`.
        assert!(matches!(
            shell_cwd(Some("/opt/Users/ada/x"), home),
            ShellCwd::Redacted(_)
        ));
        // The same slice would have split this leading character in half.
        assert!(matches!(
            shell_cwd(Some("é/Users/ada/x"), home),
            ShellCwd::Redacted(_)
        ));
        // A path the rules gut is refused rather than half-printed.
        match shell_cwd(Some("/srv/.secrets/tailnet.env"), home) {
            ShellCwd::Redacted(categories) => {
                assert!(
                    categories.iter().any(|c| c == "secrets_path"),
                    "{categories:?}"
                )
            }
            other => panic!("a secrets path must not become a command: {other:?}"),
        }
    }
}
