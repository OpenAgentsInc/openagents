//! Corpus inventory and qualification for CoderBench.
//!
//! Walks the local trace stores, converts what it can to ATIF, and writes a
//! deterministic inventory with counted exclusions. Nothing is forced: a session
//! that does not convert or does not qualify is recorded with its reason.
//!
//! The engine is bounded by design. Discovery uses capped depth, scan entry, and
//! per-store file limits. Each candidate is read once and released; converted ATIF
//! is held only long enough to summarize and digest. Files above the ingest cap are
//! excluded before any large read. This lets the command handle the real stores
//! (~2,548 Codex rollouts and ~1,327 Claude sessions on this machine) without
//! unbounded memory.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::errors::CliError;
use crate::gym::convert;
use crate::gym::schemas::{CORPUS_IMPORT_RECORD_SCHEMA, CorpusImportRecord};
use crate::trace::{self, DiscoveryBounds, TraceCandidate, TraceSourceKind};
use crate::trace_client::MAXIMUM_TRACE_BYTES;
use clap::{Args, Subcommand};
use serde_json::Value;

/// Schema id for the corpus inventory document.
pub const INVENTORY_SCHEMA: &str = "openagents.gym.corpus_inventory.v1";

/// Schema id for the qualification report.
pub const QUALIFY_SCHEMA: &str = "openagents.gym.corpus_qualify.v1";

/// Bounds used for a full inventory. Large enough for the real stores and small
/// enough to keep memory bounded.
pub const INVENTORY_BOUNDS: DiscoveryBounds = DiscoveryBounds {
    max_depth: 4,
    max_files_per_store: 12_000,
    max_scan_entries: 100_000,
};

/// One inventory row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryRow {
    pub source: String,
    pub path: PathBuf,
    pub digest: Option<String>,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    pub qualifies: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_because: Option<Vec<String>>,
}

/// Per-store counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreCount {
    pub source: String,
    pub matched: usize,
    pub listed: usize,
    pub qualified: usize,
    pub excluded: usize,
}

/// The inventory document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryDocument {
    pub schema: String,
    pub stores: Vec<StoreCount>,
    pub rows: Vec<InventoryRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<BTreeMap<String, usize>>,
}

/// A qualification report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifyReport {
    pub schema: String,
    pub total_rows: usize,
    pub excluded_rows: usize,
    pub qualified_rows: usize,
    pub by_reason: BTreeMap<String, usize>,
}

fn repo_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"github\.com/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)").unwrap())
}

/// Look for a repository hint in the content.
fn detect_repo_hint(text: &str) -> Option<String> {
    repo_regex()
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .map(|s| s.trim_end_matches(['.', ',', ';', '"', '\'']).to_string())
        .or_else(|| {
            // Fallback to the known first-domain repo if it is named anywhere.
            if text.contains("OpenAgentsInc/openagents") {
                Some("OpenAgentsInc/openagents".to_string())
            } else {
                None
            }
        })
}

/// A repo hint that points at a local user directory is treated as private.
fn is_private_repo_hint(hint: &str) -> bool {
    hint.starts_with('/')
        || hint.starts_with("~/")
        || hint.starts_with("C:\\")
        || hint.starts_with("\\\\")
}

/// Convert a candidate to an inventory row.
fn process_candidate(
    candidate: &TraceCandidate,
) -> Result<InventoryRow, Box<dyn std::error::Error>> {
    let source = candidate.kind.as_str().to_string();
    let path = candidate.path.clone();
    let bytes = candidate.bytes;
    let mut reasons = Vec::new();

    // Conversion is through packaged converters only: the native ATIF reader for
    // exports, and the Claude/Codex converters in `gym::convert`. A store with
    // no converter is recorded as not redactable rather than guessed at.
    let (text, summary, digest) = match candidate.kind {
        TraceSourceKind::OpenagentsExport => {
            // Native files above the ingest cap are excluded before any large read.
            if bytes > MAXIMUM_TRACE_BYTES {
                reasons.push("excessive_length".to_string());
                (String::new(), None, None)
            } else {
                let text = fs::read_to_string(&path)?;
                let digest = format!("sha256:{:x}", Sha256::digest(text.as_bytes()));
                let summary = trace::summarize_trace_file(&path).ok();
                (text, summary, Some(digest))
            }
        }
        TraceSourceKind::ClaudeSession | TraceSourceKind::CodexSession => {
            // Converted stores stream line by line, so the pre-parse guard is
            // about conversion cost, not memory for the whole file — but a
            // multi-GB rollout is skipped before any read at all.
            if bytes > convert::MAX_SOURCE_BYTES {
                reasons.push("oversized_source".to_string());
                (String::new(), None, None)
            } else {
                match convert::convert_candidate(candidate.kind, &path) {
                    Some(Ok(document)) => {
                        let text = serde_json::to_string(&document)?;
                        let digest = format!("sha256:{:x}", Sha256::digest(text.as_bytes()));
                        let summary = Some(trace::summarize_trace_text(&path, &text));
                        (text, summary, Some(digest))
                    }
                    _ => {
                        reasons.push("not_redactable".to_string());
                        (String::new(), None, None)
                    }
                }
            }
        }
        _ => {
            reasons.push("not_redactable".to_string());
            (String::new(), None, None)
        }
    };

    if summary.as_ref().is_none_or(|s| s.format != "atif") && reasons.is_empty() {
        reasons.push("not_redactable".to_string());
    }

    let (steps, started_at, ended_at, model) = summary
        .as_ref()
        .map(|s| {
            (
                s.steps,
                s.first_timestamp.clone(),
                s.last_timestamp.clone(),
                s.agent_model.clone(),
            )
        })
        .unwrap_or((None, None, None, None));

    let repo_hint = if !text.is_empty() {
        detect_repo_hint(&text)
    } else {
        None
    };

    if steps.unwrap_or(0) < 10 {
        reasons.push("insufficient_substance".to_string());
    }
    if repo_hint.is_none() {
        reasons.push("no_coding_outcome".to_string());
    } else if let Some(hint) = repo_hint.as_ref()
        && is_private_repo_hint(hint)
    {
        reasons.push("private_repo".to_string());
    }

    let domain = repo_hint.clone();
    let qualifies = reasons.is_empty();
    let excluded_because = if qualifies { None } else { Some(reasons) };

    Ok(InventoryRow {
        source,
        path,
        digest,
        bytes,
        steps,
        started_at,
        ended_at,
        model,
        repo_hint,
        domain,
        qualifies,
        excluded_because,
    })
}

/// Build an inventory over the local stores and write it to `out`.
pub fn inventory(
    home: &Path,
    extra_paths: &[PathBuf],
    out: &Path,
    bounds: DiscoveryBounds,
) -> Result<InventoryDocument, Box<dyn std::error::Error>> {
    let mut specs = trace::default_trace_stores(home);
    for path in extra_paths {
        specs.push(trace::path_trace_store(path.clone()));
    }
    let (scans, candidates) = trace::discover(&specs, bounds);

    let mut counts: BTreeMap<String, StoreCount> = BTreeMap::new();
    for scan in &scans {
        counts.insert(
            scan.kind.as_str().to_string(),
            StoreCount {
                source: scan.kind.as_str().to_string(),
                matched: scan.matched,
                listed: scan.listed,
                qualified: 0,
                excluded: 0,
            },
        );
    }

    let mut rows = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let row = process_candidate(&candidate)?;
        if let Some(counter) = counts.get_mut(&row.source) {
            if row.qualifies {
                counter.qualified += 1;
            } else {
                counter.excluded += 1;
            }
        }
        rows.push(row);
    }

    rows.sort_by(|a, b| a.path.cmp(&b.path));

    let mut exclusion_counts = BTreeMap::new();
    for row in &rows {
        if let Some(reasons) = &row.excluded_because {
            for reason in reasons {
                *exclusion_counts.entry(reason.clone()).or_insert(0) += 1;
            }
        }
    }

    let stores = counts.into_values().collect();
    let document = InventoryDocument {
        schema: INVENTORY_SCHEMA.to_string(),
        stores,
        rows,
        counts: Some(exclusion_counts),
    };

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string(&document)?)?;
    Ok(document)
}

/// Read an inventory file and produce a qualification report.
pub fn qualify(path: &Path) -> Result<QualifyReport, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let doc: InventoryDocument = serde_json::from_str(&text)?;
    let mut by_reason = BTreeMap::new();
    let mut excluded_rows = 0;
    for row in &doc.rows {
        if let Some(reasons) = &row.excluded_because {
            excluded_rows += 1;
            for reason in reasons {
                *by_reason.entry(reason.clone()).or_insert(0) += 1;
            }
        }
    }
    Ok(QualifyReport {
        schema: QUALIFY_SCHEMA.to_string(),
        total_rows: doc.rows.len(),
        excluded_rows,
        qualified_rows: doc.rows.len() - excluded_rows,
        by_reason,
    })
}

const DEFAULT_VISIBILITY: &str = "ledger";
const MAX_BATCH: usize = 100;

/// Arguments for `openagents gym corpus`.
#[derive(Args, Debug)]
pub struct CorpusArgs {
    #[command(subcommand)]
    pub action: CorpusAction,
}

#[derive(Subcommand, Debug)]
pub enum CorpusAction {
    /// Convert, redact, tripwire, and upload qualifying inventory rows.
    Import {
        /// Inventory file from `gym inventory` / `gym corpus qualify`.
        inventory: PathBuf,
        /// Visibility for the batch. Default `ledger`. `dark` and `glass` refused.
        #[arg(long)]
        visibility: Option<String>,
        /// Ledger path. Default `docs/coderbench/corpus.jsonl`.
        #[arg(long)]
        ledger: Option<PathBuf>,
        /// Redact and tripwire without uploading or writing the ledger.
        #[arg(long)]
        dry_run: bool,
        /// Seconds to wait between uploads. Defaults to 30 for batches over
        /// 10 (about 120 traces/hour); 0 disables pacing.
        #[arg(long)]
        pace: Option<u64>,
    },
    /// Summarize the corpus ledger: imported vs still pending in an inventory.
    Status {
        #[arg(long)]
        ledger: Option<PathBuf>,
        /// Optional inventory to count pending qualified rows.
        #[arg(long)]
        inventory: Option<PathBuf>,
    },
    /// Re-hash local files against ledger digests.
    Verify {
        /// Digests to check. Empty means every ledger row.
        digest: Vec<String>,
        #[arg(long)]
        ledger: Option<PathBuf>,
    },
}

#[derive(Debug, Clone)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
    pub pending: usize,
    pub visibility: String,
}

/// Refuse batch visibilities the corpus cannot hold.
pub fn refuse_batch_visibility(visibility: &str) -> Option<String> {
    match visibility {
        "dark" => Some(
            "dark is refused outright: an unpublishable trace has no corpus value (visibility ladder: ledger default, pulse per-trace, glass never a batch flag)".to_string(),
        ),
        "glass" => Some(
            "glass is refused as a batch flag: a glass trace is a per-trace human decision, never a corpus default".to_string(),
        ),
        _ => None,
    }
}

/// Scan redacted text for leftover secret-shaped material. A catch halts the batch.
pub fn tripwire_findings(text: &str) -> Vec<String> {
    let mut findings = Vec::new();
    if text.contains("BEGIN PRIVATE KEY") || text.contains("BEGIN RSA PRIVATE KEY") {
        findings.push("private_key".to_string());
    }
    // A redacted assignment keeps its name and masks its value, so the name
    // alone is not a leak: only a value that survived redaction halts the
    // batch. The regex crate has no lookahead, so the value is inspected by
    // hand.
    for name in ["OPENAGENTS_TOKEN=", "OPENAI_API_KEY="] {
        let mut from = 0;
        while let Some(at) = text[from..].find(name) {
            let value = &text[from + at + name.len()..];
            if !value.starts_with("[REDACTED") && !value.trim_start().is_empty() {
                findings.push("env_secret".to_string());
                break;
            }
            from += at + name.len();
        }
        if findings.last().map(String::as_str) == Some("env_secret") {
            break;
        }
    }
    let key = Regex::new(r"sk-[A-Za-z0-9]{16,}").unwrap();
    if key.is_match(text) {
        findings.push("provider_key".to_string());
    }
    findings
}

pub fn default_ledger_path() -> Result<PathBuf, CliError> {
    Ok(repo_root()?.join("docs/coderbench/corpus.jsonl"))
}

fn repo_root() -> Result<PathBuf, CliError> {
    let mut dir = std::env::current_dir().map_err(|e| CliError::Configuration(e.to_string()))?;
    loop {
        if dir.join("docs").join("coderbench").is_dir() || dir.join("bench").join("suites").is_dir()
        {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => {
                return Err(CliError::Configuration(
                    "could not find docs/coderbench from the current directory".into(),
                ));
            }
        }
    }
}

pub fn read_ledger(path: &Path) -> Result<Vec<CorpusImportRecord>, CliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", path.display())))?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: CorpusImportRecord = serde_json::from_str(line).map_err(|e| {
            CliError::Input(format!(
                "{} line {} is not a ledger row: {e}",
                path.display(),
                i + 1
            ))
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn append_ledger(path: &Path, row: &CorpusImportRecord) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::Output(format!("could not create {}: {e}", parent.display())))?;
    }
    let mut line = serde_json::to_string(row).map_err(|e| CliError::Output(e.to_string()))?;
    line.push('\n');
    use std::io::Write;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| CliError::Output(format!("could not append {}: {e}", path.display())))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub digest: String,
    pub document: Value,
    pub source: String,
    pub domain: String,
    /// The local file the document came from, kept for `corpus verify`.
    pub source_path: PathBuf,
    /// Digest of the pre-redaction local text (native or converted), which is
    /// what re-hashing the source can actually reproduce later.
    pub source_digest: String,
}

/// Produce the uploadable text for one inventory row: the file itself for
/// native ATIF, the converted document for Claude and Codex sessions. This is
/// the single dispatch `prepare_import` and `verify_ledger` both use, so the
/// digest recorded at import time and the digest recomputed at verify time can
/// only be the same function of the same bytes.
fn source_text_for(source: &str, path: &Path) -> Result<String, CliError> {
    let kind = match source {
        "claude_session" => Some(TraceSourceKind::ClaudeSession),
        "codex_session" => Some(TraceSourceKind::CodexSession),
        _ => None,
    };
    match kind.and_then(|kind| convert::convert_candidate(kind, path)) {
        Some(Ok(document)) => serde_json::to_string(&document)
            .map_err(|e| CliError::Input(format!("could not serialize {}: {e}", path.display()))),
        Some(Err(error)) => Err(CliError::Input(format!(
            "could not convert {}: {error}",
            path.display()
        ))),
        None => fs::read_to_string(path)
            .map_err(|e| CliError::Input(format!("could not read {}: {e}", path.display()))),
    }
}

/// Redact and tripwire qualifying rows. Does not upload.
pub fn prepare_import(
    inventory: &Path,
    visibility: &str,
    ledger: &Path,
    home: &str,
) -> Result<(Vec<PreparedImport>, usize, usize), CliError> {
    if let Some(reason) = refuse_batch_visibility(visibility) {
        return Err(CliError::Input(reason));
    }
    if visibility != "ledger" && visibility != "pulse" {
        return Err(CliError::Input(format!(
            "corpus import visibility must be ledger or pulse; got {visibility}"
        )));
    }

    let text = fs::read_to_string(inventory)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", inventory.display())))?;
    let doc: InventoryDocument = serde_json::from_str(&text).map_err(|e| {
        CliError::Input(format!(
            "{} is not an inventory document: {e}",
            inventory.display()
        ))
    })?;

    let existing = read_ledger(ledger)?;
    let known: std::collections::BTreeSet<String> =
        existing.into_iter().map(|r| r.digest).collect();

    let qualified: Vec<&InventoryRow> = doc.rows.iter().filter(|r| r.qualifies).collect();
    let mut prepared = Vec::new();
    let mut skipped = 0usize;
    let mut pending = 0usize;

    for row in qualified.into_iter().take(MAX_BATCH) {
        let digest = match &row.digest {
            Some(d) => d.clone(),
            None => {
                pending += 1;
                continue;
            }
        };
        if known.contains(&digest) {
            skipped += 1;
            continue;
        }
        let source_text = source_text_for(&row.source, &row.path)?;
        let source_digest = format!("sha256:{:x}", Sha256::digest(source_text.as_bytes()));
        if source_digest != digest {
            // A source still being written — a live session, most often — is
            // not a reason to halt everyone else's import: skip it and let a
            // later inventory pick it up settled.
            eprintln!(
                "skipping {}: drifted since the inventory was written",
                row.path.display()
            );
            skipped += 1;
            continue;
        }
        let redacted = crate::trace::redact_text(&source_text, home);
        let findings = tripwire_findings(&redacted.text);
        if !findings.is_empty() {
            // A trace redaction could not clean never uploads, and no flag can
            // make it. It is excluded aloud with its reason, and the rest of
            // the batch — clean traces — proceeds; halting them all made bulk
            // import a whack-a-mole and protected nothing extra.
            eprintln!(
                "excluding {} ({digest}): tripwire {}",
                row.path.display(),
                findings.join(", ")
            );
            skipped += 1;
            continue;
        }
        let document: Value = match serde_json::from_str(&redacted.text) {
            Ok(document) => document,
            Err(e) => {
                // A conversion that did not yield clean JSON — a truncation
                // seam that split a string, a malformed source — skips its
                // one row rather than halting everyone else's import.
                eprintln!(
                    "skipping {}: redacted output is not JSON: {e}",
                    row.path.display()
                );
                skipped += 1;
                continue;
            }
        };
        prepared.push(PreparedImport {
            digest,
            document,
            source: row.source.clone(),
            domain: row.domain.clone().unwrap_or_else(|| "unknown".into()),
            source_path: row.path.clone(),
            source_digest,
        });
    }

    Ok((prepared, skipped, pending))
}

pub fn record_import(
    ledger: &Path,
    visibility: &str,
    prepared: &PreparedImport,
    uuid: String,
    stored_digest: String,
) -> Result<CorpusImportRecord, CliError> {
    let record = CorpusImportRecord {
        schema: CORPUS_IMPORT_RECORD_SCHEMA.to_string(),
        digest: stored_digest,
        trace_uuid: uuid,
        source: prepared.source.clone(),
        domain: prepared.domain.clone(),
        visibility: visibility.to_string(),
        recorded_at: crate::computer::now_iso8601(),
        batch_id: None,
        source_path: Some(prepared.source_path.clone()),
        source_digest: Some(prepared.source_digest.clone()),
    };
    append_ledger(ledger, &record)?;
    Ok(record)
}

pub fn corpus_status(ledger: &Path, inventory: Option<&Path>) -> Result<(usize, usize), CliError> {
    let imported = read_ledger(ledger)?.len();
    let pending = if let Some(path) = inventory {
        let text = fs::read_to_string(path)
            .map_err(|e| CliError::Input(format!("could not read {}: {e}", path.display())))?;
        let doc: InventoryDocument = serde_json::from_str(&text).map_err(|e| {
            CliError::Input(format!(
                "{} is not an inventory document: {e}",
                path.display()
            ))
        })?;
        let known: std::collections::BTreeSet<String> =
            read_ledger(ledger)?.into_iter().map(|r| r.digest).collect();
        doc.rows
            .iter()
            .filter(|r| r.qualifies)
            .filter(|r| match &r.digest {
                Some(d) => !known.contains(d),
                None => true,
            })
            .count()
    } else {
        0
    };
    Ok((imported, pending))
}

/// What one `corpus verify` pass actually established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Rows whose local source re-hashed to the recorded digest.
    pub verified: usize,
    /// Rows that could not be checked locally: no source path recorded (rows
    /// imported before the ledger carried one), or no source digest to compare.
    pub unverifiable: usize,
    /// Every disagreement found, one sentence per row.
    pub drifts: Vec<String>,
}

/// Re-hash each ledger row's local source and compare it with the digest the
/// ledger recorded at import time.
///
/// The check is local-only and says so: the server exposes `POST /api/v1/traces`
/// and no lookup route (`GET /api/v1/traces/:id` does not exist — see
/// `trace_client.rs`), so there is nothing cheap to ask it. What can be proven
/// here is that the file a ledger row points at still converts and hashes to
/// the bytes that were imported; a missing file, a changed file, and a row too
/// old to carry a source path are each reported as what they are.
pub fn verify_ledger(ledger: &Path, digests: &[String]) -> Result<VerifyReport, CliError> {
    let rows = read_ledger(ledger)?;
    let wanted: std::collections::BTreeSet<String> = if digests.is_empty() {
        rows.iter().map(|r| r.digest.clone()).collect()
    } else {
        digests.iter().cloned().collect()
    };
    let mut drifts = Vec::new();
    let mut verified = 0usize;
    let mut unverifiable = 0usize;
    for row in &rows {
        if !wanted.contains(&row.digest) {
            continue;
        }
        if !row.digest.starts_with("sha256:") && !row.digest.is_empty() {
            drifts.push(format!("{}: digest is not sha256-prefixed", row.trace_uuid));
        }
        let (Some(path), Some(expected)) = (&row.source_path, &row.source_digest) else {
            unverifiable += 1;
            continue;
        };
        if !path.exists() {
            drifts.push(format!(
                "{}: source {} no longer exists",
                row.trace_uuid,
                path.display()
            ));
            continue;
        }
        match source_text_for(&row.source, path) {
            Ok(text) => {
                let recomputed = format!("sha256:{:x}", Sha256::digest(text.as_bytes()));
                if recomputed == *expected {
                    verified += 1;
                } else {
                    drifts.push(format!(
                        "{}: source {} drifted — ledger recorded {expected}, \
                         re-hash yields {recomputed}",
                        row.trace_uuid,
                        path.display()
                    ));
                }
            }
            Err(error) => drifts.push(format!(
                "{}: source {} no longer converts: {error}",
                row.trace_uuid,
                path.display()
            )),
        }
    }
    // Induced drift: a requested digest that is not in the ledger.
    for digest in &wanted {
        if !rows.iter().any(|r| r.digest == *digest) {
            drifts.push(format!(
                "{digest}: named in verify but absent from the ledger"
            ));
        }
    }
    Ok(VerifyReport {
        verified,
        unverifiable,
        drifts,
    })
}

pub async fn run_corpus(
    args: CorpusArgs,
    api_base: &str,
    token: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    match args.action {
        CorpusAction::Import {
            inventory,
            visibility,
            ledger,
            dry_run,
            pace,
        } => {
            let visibility = visibility.unwrap_or_else(|| DEFAULT_VISIBILITY.to_string());
            let ledger = match ledger {
                Some(p) => p,
                None => default_ledger_path()?,
            };
            let home = std::env::var("HOME").unwrap_or_default();
            let (prepared, skipped, mut pending) =
                prepare_import(&inventory, &visibility, &ledger, &home)?;
            // The spec promises roughly 120 uploads an hour, so a batch worth
            // pacing gets 30 seconds between uploads unless the caller says
            // otherwise. `--pace 0` disables it.
            let pace_seconds = pace.unwrap_or(if prepared.len() > 10 { 30 } else { 0 });
            let mut imported = 0usize;
            if dry_run {
                pending += prepared.len();
            } else {
                let client = crate::trace_client::TraceClient::new(api_base, token);
                for row in &prepared {
                    if imported > 0 && pace_seconds > 0 {
                        tokio::time::sleep(std::time::Duration::from_secs(pace_seconds)).await;
                    }
                    let stored = client
                        .upload(&row.document, &visibility, None)
                        .await
                        .map_err(|e| CliError::Network(e.to_string()))?;
                    let record =
                        record_import(&ledger, &visibility, row, stored.id, stored.digest)?;
                    if !json {
                        crate::gym::views::emit_lines(
                            &crate::gym::views::render_corpus_import_record(&record),
                        );
                    }
                    imported += 1;
                }
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "schema": CORPUS_IMPORT_RECORD_SCHEMA,
                        "imported": imported,
                        "skipped": skipped,
                        "pending": pending,
                        "visibility": visibility,
                        "ledger": ledger,
                    }))
                    .unwrap()
                );
            } else {
                println!(
                    "imported={imported} skipped={skipped} pending={pending} visibility={visibility} ledger={}",
                    ledger.display()
                );
            }
            Ok(())
        }
        CorpusAction::Status { ledger, inventory } => {
            let ledger = match ledger {
                Some(p) => p,
                None => default_ledger_path()?,
            };
            let (imported, pending) = corpus_status(&ledger, inventory.as_deref())?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "imported": imported,
                        "pending": pending,
                        "ledger": ledger,
                    }))
                    .unwrap()
                );
            } else {
                println!(
                    "imported={} pending={} ledger={}",
                    imported,
                    pending,
                    ledger.display()
                );
            }
            Ok(())
        }
        CorpusAction::Verify { digest, ledger } => {
            let ledger = match ledger {
                Some(p) => p,
                None => default_ledger_path()?,
            };
            let report = verify_ledger(&ledger, &digest)?;
            if !report.drifts.is_empty() {
                return Err(CliError::Input(format!(
                    "corpus verify found {} drift(s): {}",
                    report.drifts.len(),
                    report.drifts.join("; ")
                )));
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "verified": report.verified,
                        "unverifiable": report.unverifiable,
                        "server_checked": false,
                        "ledger": ledger,
                    }))
                    .unwrap()
                );
            } else {
                println!(
                    "verified {} row(s) against local sources ({} unverifiable: no source \
                     path recorded); server has no trace lookup route, so this check is \
                     local-only — ledger {}",
                    report.verified,
                    report.unverifiable,
                    ledger.display()
                );
            }
            Ok(())
        }
    }
}
