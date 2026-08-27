//! Local agent-trace discovery, summary, and redaction.
//!
//! This is the Rust port of `packages/openagents-cli/src/trace-store.ts` and
//! `trace-command.ts`. It replaces a module that returned two hardcoded sessions
//! from `scan_foreign_sessions` and a `redact_trace` that swapped the *prefix* of a
//! secret — turning `sk-liveSECRET` into `[REDACTED_KEY]liveSECRET` — while writing
//! no file and reporting success. A redaction command that leaves the key in place
//! is worse than no command, because whoever ran it has been told the trace is safe.
//!
//! Nothing here invents a value. Discovery reports only files it actually stat'ed,
//! `summarize` reports only fields the document carries, and redaction reports
//! counts per category without ever echoing the matched text.

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

/// Which store a file was found in. It is a property of the directory the file was
/// found under, never inferred from the file's name or contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceSourceKind {
    OpenagentsExport,
    ClaudeSession,
    CodexSession,
    TracePath,
}

impl TraceSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenagentsExport => "openagents_export",
            Self::ClaudeSession => "claude_session",
            Self::CodexSession => "codex_session",
            Self::TracePath => "trace_path",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceStoreSpec {
    pub root: PathBuf,
    pub kind: TraceSourceKind,
    pub extensions: Vec<&'static str>,
}

/// One discovered file. Its only identifier is its absolute path: discovery derives
/// no session id, and `trace show` resolves by path, so inventing an id here would
/// add a lookup that has nothing behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCandidate {
    pub path: PathBuf,
    pub kind: TraceSourceKind,
    pub bytes: u64,
    pub modified_at: String,
}

/// What one store yielded, including what was refused and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceStoreScan {
    pub root: PathBuf,
    pub kind: TraceSourceKind,
    pub present: bool,
    pub matched: usize,
    pub listed: usize,
    pub skipped_symlinks: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DiscoveryBounds {
    pub max_depth: usize,
    pub max_files_per_store: usize,
    pub max_scan_entries: usize,
}

impl Default for DiscoveryBounds {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_files_per_store: 20,
            max_scan_entries: 5000,
        }
    }
}

/// The three stores a machine carries by default, in listing order.
pub fn default_trace_stores(home: &Path) -> Vec<TraceStoreSpec> {
    vec![
        TraceStoreSpec {
            root: home.join(".openagents").join("exports"),
            kind: TraceSourceKind::OpenagentsExport,
            extensions: vec![".json"],
        },
        TraceStoreSpec {
            root: home.join(".claude").join("projects"),
            kind: TraceSourceKind::ClaudeSession,
            extensions: vec![".jsonl"],
        },
        TraceStoreSpec {
            root: home.join(".codex").join("sessions"),
            kind: TraceSourceKind::CodexSession,
            extensions: vec![".jsonl"],
        },
    ]
}

/// A store the caller named, through `--path` or `OPENAGENTS_TRACE_PATHS`.
pub fn path_trace_store(root: PathBuf) -> TraceStoreSpec {
    TraceStoreSpec {
        root,
        kind: TraceSourceKind::TracePath,
        extensions: vec![".json", ".jsonl"],
    }
}

/// Extra stores from `OPENAGENTS_TRACE_PATHS`, a colon-separated list.
pub fn extra_path_stores() -> Vec<TraceStoreSpec> {
    std::env::var("OPENAGENTS_TRACE_PATHS")
        .unwrap_or_default()
        .split(':')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| path_trace_store(PathBuf::from(entry)))
        .collect()
}

fn matches_extension(name: &str, extensions: &[&str]) -> bool {
    extensions.iter().any(|ext| name.ends_with(ext))
}

/// Walk one store breadth-first within `bounds`, newest file first.
///
/// Symlinks are never followed and always counted: a symlinked root is refused
/// outright, and a symlinked entry is skipped, so a planted link can neither escape
/// the store nor spin the walk in a loop.
pub fn scan_store(
    spec: &TraceStoreSpec,
    bounds: DiscoveryBounds,
) -> (TraceStoreScan, Vec<TraceCandidate>) {
    let root_meta = fs::symlink_metadata(&spec.root).ok();
    let root_is_symlink = root_meta
        .as_ref()
        .is_some_and(|m| m.file_type().is_symlink());
    let usable = root_meta.as_ref().is_some_and(|m| m.is_dir()) && !root_is_symlink;

    if !usable {
        return (
            TraceStoreScan {
                root: spec.root.clone(),
                kind: spec.kind,
                present: false,
                matched: 0,
                listed: 0,
                skipped_symlinks: usize::from(root_is_symlink),
                truncated: false,
            },
            Vec::new(),
        );
    }

    let mut found: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut skipped_symlinks = 0usize;
    let mut visited = 0usize;
    let mut truncated = false;
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(spec.root.clone(), 0usize)]);

    'walk: while let Some((directory, depth)) = queue.pop_front() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            // An unreadable directory is skipped, not reported as a file.
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited >= bounds.max_scan_entries {
                truncated = true;
                break 'walk;
            }
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                skipped_symlinks += 1;
                continue;
            }
            if meta.is_dir() {
                if depth < bounds.max_depth {
                    queue.push_back((path, depth + 1));
                }
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if meta.is_file() && matches_extension(&name, &spec.extensions) {
                let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                found.push((path, meta.len(), mtime));
            }
        }
    }

    let matched = found.len();
    found.sort_by(|a, b| b.2.cmp(&a.2));
    let candidates: Vec<TraceCandidate> = found
        .into_iter()
        .take(bounds.max_files_per_store)
        .map(|(path, bytes, mtime)| TraceCandidate {
            path,
            kind: spec.kind,
            bytes,
            modified_at: iso8601_utc(mtime),
        })
        .collect();

    (
        TraceStoreScan {
            root: spec.root.clone(),
            kind: spec.kind,
            present: true,
            matched,
            listed: candidates.len(),
            skipped_symlinks,
            truncated,
        },
        candidates,
    )
}

/// Scan every store and merge the candidates, newest first across all of them.
pub fn discover(
    specs: &[TraceStoreSpec],
    bounds: DiscoveryBounds,
) -> (Vec<TraceStoreScan>, Vec<TraceCandidate>) {
    let mut scans = Vec::with_capacity(specs.len());
    let mut candidates = Vec::new();
    for spec in specs {
        let (scan, mut found) = scan_store(spec, bounds);
        scans.push(scan);
        candidates.append(&mut found);
    }
    candidates.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    (scans, candidates)
}

/// Format a `SystemTime` as `2026-08-26T05:44:44.859Z`, matching the ISO strings the
/// TypeScript CLI reports so the two listings can be compared line for line.
fn iso8601_utc(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs() as i64;
    let millis = duration.subsec_millis();

    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
        millis
    )
}

/// Howard Hinnant's `civil_from_days`: days since the Unix epoch to a civil date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSummary {
    pub path: PathBuf,
    /// `atif`, `jsonl`, or `unknown`. Detected from the content, then the extension.
    pub format: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps_by_source: Option<BTreeMap<String, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_timestamp: Option<String>,
}

fn str_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Read a trace file and report only what it actually carries.
///
/// A file that is not an ATIF document is reported as what it is — a line-delimited
/// log, or an unknown format — rather than being given invented step counts.
pub fn summarize_trace_file(path: &Path) -> std::io::Result<TraceSummary> {
    let text = fs::read_to_string(path)?;
    let bytes = text.len() as u64;

    let document = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .filter(|v| v.is_object());
    let steps = document
        .as_ref()
        .and_then(|d| d.get("steps"))
        .and_then(|v| v.as_array());

    let base = TraceSummary {
        path: path.to_path_buf(),
        format: "unknown".to_string(),
        bytes,
        lines: None,
        schema_version: None,
        session_id: None,
        agent_name: None,
        agent_model: None,
        steps: None,
        steps_by_source: None,
        models: None,
        tool_calls: None,
        total_prompt_tokens: None,
        total_completion_tokens: None,
        first_timestamp: None,
        last_timestamp: None,
    };

    let (Some(document), Some(steps)) = (document.as_ref(), steps) else {
        if path.to_string_lossy().ends_with(".jsonl") {
            return Ok(TraceSummary {
                format: "jsonl".to_string(),
                lines: Some(text.lines().filter(|l| !l.trim().is_empty()).count()),
                ..base
            });
        }
        return Ok(base);
    };

    let mut steps_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut models: BTreeSet<String> = BTreeSet::new();
    let mut tool_calls = 0usize;
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut saw_tokens = false;

    for step in steps.iter().filter(|s| s.is_object()) {
        let source = str_field(step, "source").unwrap_or_else(|| "unknown".to_string());
        *steps_by_source.entry(source).or_insert(0) += 1;
        if let Some(model) = str_field(step, "model_name") {
            models.insert(model);
        }
        if let Some(calls) = step.get("tool_calls").and_then(|v| v.as_array()) {
            tool_calls += calls.len();
        }
        if let Some(metrics) = step.get("metrics") {
            if let Some(n) = metrics.get("prompt_tokens").and_then(|v| v.as_u64()) {
                prompt_tokens += n;
                saw_tokens = true;
            }
            if let Some(n) = metrics.get("completion_tokens").and_then(|v| v.as_u64()) {
                completion_tokens += n;
                saw_tokens = true;
            }
        }
    }

    let final_metrics = document.get("final_metrics");
    let final_prompt = final_metrics
        .and_then(|m| m.get("total_prompt_tokens"))
        .and_then(|v| v.as_u64());
    let final_completion = final_metrics
        .and_then(|m| m.get("total_completion_tokens"))
        .and_then(|v| v.as_u64());

    // The document's own totals win; the per-step sums are a fallback, and when
    // neither exists the fields are omitted rather than reported as zero.
    let (total_prompt_tokens, total_completion_tokens) =
        if final_prompt.is_some() || final_completion.is_some() {
            (final_prompt, final_completion)
        } else if saw_tokens {
            (Some(prompt_tokens), Some(completion_tokens))
        } else {
            (None, None)
        };

    let agent = document.get("agent");
    Ok(TraceSummary {
        format: "atif".to_string(),
        schema_version: str_field(document, "schema_version"),
        session_id: str_field(document, "session_id"),
        agent_name: agent.and_then(|a| str_field(a, "name")),
        agent_model: agent.and_then(|a| str_field(a, "model_name")),
        steps: Some(steps.len()),
        steps_by_source: Some(steps_by_source),
        models: Some(models.into_iter().collect()),
        tool_calls: Some(tool_calls),
        total_prompt_tokens,
        total_completion_tokens,
        first_timestamp: steps.first().and_then(|s| str_field(s, "timestamp")),
        last_timestamp: steps.last().and_then(|s| str_field(s, "timestamp")),
        ..base
    })
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

/// Where a redacted copy is written. `.jsonl` is checked first.
pub fn redacted_path_for(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(stem) = text.strip_suffix(".jsonl") {
        PathBuf::from(format!("{}.redacted.jsonl", stem))
    } else if let Some(stem) = text.strip_suffix(".json") {
        PathBuf::from(format!("{}.redacted.json", stem))
    } else {
        PathBuf::from(format!("{}.redacted.json", text))
    }
}

/// True when the path is itself a redacted copy.
pub fn is_redacted_copy(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.ends_with(".redacted.json") || text.ends_with(".redacted.jsonl")
}

/// What redaction removed, counted by category. Never the matched text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redaction {
    pub text: String,
    pub counts: BTreeMap<String, usize>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub counts: BTreeMap<String, usize>,
    pub total: usize,
    /// `None` when the input was not JSON to begin with, so re-parsing proves nothing.
    pub valid_json: Option<bool>,
}

/// The number of consecutive BIP-39 words a run must reach before it is treated as a
/// seed phrase. Below this, the rule declines and the prose is left alone.
const MIN_SEED_WORDS: usize = 12;

struct RedactionRule {
    category: &'static str,
    pattern: Regex,
    replacement: &'static str,
    /// Rules whose decision needs more than the pattern. Returning the match
    /// unchanged means the rule declined, and a decline is not counted.
    resolve: Option<fn(&str) -> String>,
    /// The bare `NAME=value` rule, which must not re-match the marker the quoted
    /// rules just wrote. Stands in for the TypeScript `(?!\[REDACTED)` lookahead.
    declines_redacted_capture: bool,
}

/// Every rule, in the order they run.
///
/// Order is load-bearing: the specific shapes run before the broad ones so a bearer
/// token is counted as `bearer_token` rather than swallowed by `env_value`, and each
/// rule sees the previous rule's substitutions.
///
/// These patterns are a third statement of the ones in
/// `packages/atif/src/redaction.ts`, which is authoritative, and a third statement is
/// what let `oa_pat_` and `smct_` leak in the first place. This crate cannot import
/// the TypeScript list, so the coupling is a TEST instead: `tests/trace_test.rs` reads
/// `fixtures/redaction/planted-secrets.json` — the same file both TypeScript paths
/// assert against — and fails when a planted credential body survives redaction here.
/// A token family added to ATIF gets a planted secret, and the planted secret fails
/// this crate until the rule below exists.
///
/// The CATEGORY names are this CLI's own and do not have to match ATIF's; only the
/// removal is a shared contract.
fn redaction_rules(home: &str) -> Vec<RedactionRule> {
    let mut rules = vec![
        RedactionRule {
            category: "seed_phrase",
            pattern: Regex::new(r"\b(?:[a-z]{3,8} ){11}[a-z]{3,8}(?:(?: [a-z]{3,8}){3})*\b").unwrap(),
            replacement: "[REDACTED:seed_phrase]",
            resolve: Some(resolve_seed_phrase),
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "private_key",
            pattern: Regex::new(
                r"\b(?:nsec1[02-9ac-hj-np-z]{50,}|(?:xprv|yprv|zprv|tprv|uprv|vprv)[1-9A-HJ-NP-Za-km-z]{50,})\b",
            )
            .unwrap(),
            replacement: "[REDACTED:private_key]",
            resolve: None,
            declines_redacted_capture: false,
        },
        // The PEM block form. It spans lines, so it needs `(?s)`, and it runs
        // before every line-oriented rule below so nothing chops it in half.
        RedactionRule {
            category: "private_key",
            pattern: Regex::new(
                r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
            )
            .unwrap(),
            replacement: "[REDACTED:private_key]",
            resolve: None,
            declines_redacted_capture: false,
        },
        // Payment and wallet material: an invoice or offer someone can pay, and
        // an extended PUBLIC key, which is not a spending secret but reveals
        // every address a wallet will ever use.
        RedactionRule {
            category: "wallet_or_payment",
            pattern: Regex::new(
                r"(?i)\b(?:lnbc[0-9][a-z0-9]{20,}|lntb[0-9][a-z0-9]{20,}|lno1[a-z0-9]{20,}|bc1[a-z0-9]{20,}|(?:xpub|ypub|zpub|tpub)[1-9A-HJ-NP-Za-km-z]{20,})\b",
            )
            .unwrap(),
            replacement: "[REDACTED:wallet_or_payment]",
            resolve: None,
            declines_redacted_capture: false,
        },
        // A path into a `.secrets/` directory. The file name alone says which
        // credential lives there, and the home-path rule below rewrites only the
        // `/Users/<name>` prefix, so it would leave the rest of the path standing.
        RedactionRule {
            category: "secrets_path",
            pattern: Regex::new(r#"(?:\.{1,2}/)?\.secrets/[^\s"'`)<>]+"#).unwrap(),
            replacement: "[REDACTED:secrets_path]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "bearer_token",
            pattern: Regex::new(r"\b[Bb]earer\s+[A-Za-z0-9._~+/=-]{8,}").unwrap(),
            replacement: "Bearer [REDACTED:bearer_token]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "api_key",
            pattern: Regex::new(
                r"\b(?:sk-[A-Za-z0-9_-]{16,}|(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{8,}|gh[pousr]_[A-Za-z0-9]{16,}|github_pat_[A-Za-z0-9_]{20,}|glpat-[A-Za-z0-9_-]{16,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{20,})\b",
            )
            .unwrap(),
            replacement: "[REDACTED:api_key]",
            resolve: None,
            declines_redacted_capture: false,
        },
        // OpenAgents' own token family. `trace-command.ts` does not carry these —
        // its `api_key` rule stops at the third-party prefixes — so `oa_pat_…`
        // survived a redaction that claimed to have run. The patterns are the
        // authoritative ones from `packages/atif/src/redaction.ts`. Redacting more
        // than the TypeScript rules is always safe here; redacting less is the
        // failure mode this whole module exists to close.
        RedactionRule {
            category: "oa_agent_token",
            pattern: Regex::new(r"\boa_agent_[A-Za-z0-9_-]{6,}\b").unwrap(),
            replacement: "[REDACTED:oa_agent_token]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "oa_token",
            pattern: Regex::new(
                r"\b(?:oa_(?:live|test|sk|key|secret|tok|token|pat)?_?[A-Za-z0-9]{12,}|oa-x-[A-Za-z0-9_-]{4,}|smct_[A-Za-z0-9_-]{6,})\b",
            )
            .unwrap(),
            replacement: "[REDACTED:oa_token]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "jwt",
            pattern: Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b").unwrap(),
            replacement: "[REDACTED:jwt]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "secret_field",
            pattern: Regex::new(
                r#"(?i)("[\w.-]*(?:token|secret|password|passwd|api[_-]?key|credential|private[_-]?key)[\w.-]*"\s*:\s*)"(?:[^"\\]|\\.)*""#,
            )
            .unwrap(),
            replacement: "$1\"[REDACTED:secret_field]\"",
            resolve: None,
            declines_redacted_capture: false,
        },
        // The quoted form. The TypeScript rule uses a backreference to pair the
        // quote character; `regex` has none, so the two quote styles are two rules
        // that cannot cross-match — which is exactly what the backreference enforced.
        RedactionRule {
            category: "env_value",
            pattern: Regex::new("\\b([A-Z][A-Z0-9_]{2,})=(\"[^\"\n]{4,}?\")").unwrap(),
            replacement: "$1=[REDACTED:env_value]",
            resolve: None,
            declines_redacted_capture: false,
        },
        RedactionRule {
            category: "env_value",
            pattern: Regex::new(r"\b([A-Z][A-Z0-9_]{2,})=('[^'\n]{4,}?')").unwrap(),
            replacement: "$1=[REDACTED:env_value]",
            resolve: None,
            declines_redacted_capture: false,
        },
        // The bare form. `already_redacted` below stands in for the TypeScript
        // `(?!\[REDACTED)` lookahead, so this rule cannot re-match the marker the
        // two rules above just wrote.
        RedactionRule {
            category: "env_value",
            pattern: Regex::new("\\b([A-Z][A-Z0-9_]{2,})=([^\\s\"'`\\\\,}]{4,})").unwrap(),
            replacement: "$1=[REDACTED:env_value]",
            resolve: None,
            declines_redacted_capture: true,
        },
    ];

    if !home.is_empty() {
        rules.push(RedactionRule {
            category: "home_path",
            pattern: Regex::new(&regex::escape(home)).unwrap(),
            replacement: "~",
            resolve: None,
            declines_redacted_capture: false,
        });
    }
    rules.push(RedactionRule {
        category: "home_path",
        pattern: Regex::new(r"(?:/Users|/home)/[A-Za-z0-9._-]+").unwrap(),
        replacement: "~",
        resolve: None,
        declines_redacted_capture: false,
    });
    rules
}

/// Decide whether a run of lowercase words is really a seed phrase.
///
/// A 12-word run of English prose matches the shape, so the shape alone is not
/// enough. The rule finds the longest run of consecutive words that are all in the
/// BIP-39 English list and declines unless that run reaches [`MIN_SEED_WORDS`],
/// which keeps surrounding prose intact.
fn resolve_seed_phrase(matched: &str) -> String {
    let words: Vec<&str> = matched.split(' ').collect();
    let wordlist = bip39::Language::English.word_list();

    let mut best_start = 0usize;
    let mut best_length = 0usize;
    let mut run_start = 0usize;
    let mut run_length = 0usize;
    for (index, word) in words.iter().enumerate() {
        if wordlist.contains(word) {
            if run_length == 0 {
                run_start = index;
            }
            run_length += 1;
            if run_length > best_length {
                best_length = run_length;
                best_start = run_start;
            }
        } else {
            run_length = 0;
        }
    }

    if best_length < MIN_SEED_WORDS {
        return matched.to_string();
    }

    [
        words[..best_start].join(" "),
        "[REDACTED:seed_phrase]".to_string(),
        words[best_start + best_length..].join(" "),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// True when a bare env value is already a redaction marker.
fn already_redacted(value: &str) -> bool {
    value.starts_with("[REDACTED")
}

/// Apply every rule in order and report what each removed.
///
/// The returned text is what gets written; the counts are what gets printed. The
/// matched text appears in neither.
pub fn redact_text(input: &str, home: &str) -> Redaction {
    let mut output = input.to_string();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0usize;

    for rule in redaction_rules(home) {
        let mut matched = 0usize;
        let replaced = rule.pattern.replace_all(&output, |caps: &Captures| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or_default();

            if let Some(resolve) = rule.resolve {
                let resolved = resolve(whole);
                if resolved == whole {
                    return whole.to_string();
                }
                matched += 1;
                return resolved;
            }

            // Stand-in for the `(?!\[REDACTED)` lookahead.
            if rule.declines_redacted_capture
                && caps.get(2).is_some_and(|m| already_redacted(m.as_str()))
            {
                return whole.to_string();
            }

            matched += 1;
            expand_single_digit(rule.replacement, caps)
        });
        output = replaced.into_owned();

        if matched > 0 {
            *counts.entry(rule.category.to_string()).or_insert(0) += matched;
            total += matched;
        }
    }

    Redaction {
        text: output,
        counts,
        total,
    }
}

/// Expand `$1`..`$9` in a replacement, exactly as the TypeScript does: a single
/// digit, and a missing capture becomes the empty string.
fn expand_single_digit(replacement: &str, caps: &Captures) -> String {
    let mut out = String::with_capacity(replacement.len());
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
                chars.next();
                if let Some(capture) = caps.get(digit as usize) {
                    out.push_str(capture.as_str());
                }
                continue;
            }
        }
        out.push(ch);
    }
    out
}

/// Redact a trace file and write the redacted copy beside it.
///
/// The write is the point: the command this replaces printed a size and dropped the
/// result, so a caller who ran it before sharing a trace had been told the trace was
/// safe while the original still held the key.
pub fn redact_trace_file(path: &Path, home: &str) -> std::io::Result<RedactionResult> {
    let text = fs::read_to_string(path)?;
    let parsed_before = serde_json::from_str::<serde_json::Value>(&text).is_ok();

    let redaction = redact_text(&text, home);
    let output = redacted_path_for(path);
    fs::write(&output, &redaction.text)?;

    let valid_json = if parsed_before {
        Some(serde_json::from_str::<serde_json::Value>(&redaction.text).is_ok())
    } else {
        None
    };

    Ok(RedactionResult {
        input: path.to_path_buf(),
        output,
        counts: redaction.counts,
        total: redaction.total,
        valid_json,
    })
}

/// Resolve a `trace show` / `trace redact` argument to a real file.
///
/// A path, or a bare file name inside `~/.openagents/exports`. An argument that
/// resolves to nothing is refused: there is no session-id lookup behind this, and
/// pretending otherwise is how the old `trace show` accepted any id at all.
pub fn resolve_trace_argument(value: &str, home: &Path) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(value);
    let direct = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(candidate)
    };
    if direct.exists() {
        return Ok(direct);
    }

    if !value.contains('/') {
        let in_exports = home.join(".openagents").join("exports").join(value);
        if in_exports.exists() {
            return Ok(in_exports);
        }
    }

    Err(format!(
        "No trace file exists at {}, and ~/.openagents/exports has no file by that name. \
         Run `oa trace list` to see what is discoverable.",
        value
    ))
}
