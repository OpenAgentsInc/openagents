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

use crate::trace::{self, DiscoveryBounds, TraceCandidate, TraceSourceKind};
use crate::trace_client::MAXIMUM_TRACE_BYTES;

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
        .map(|s| {
            s.trim_end_matches(|c: char| c == '.' || c == ',' || c == ';' || c == '"' || c == '\'')
                .to_string()
        })
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

    // Files above the ingest cap are excluded before any large read.
    if bytes > MAXIMUM_TRACE_BYTES {
        reasons.push("excessive_length".to_string());
        return Ok(InventoryRow {
            source,
            path,
            digest: None,
            bytes,
            steps: None,
            started_at: None,
            ended_at: None,
            model: None,
            repo_hint: None,
            domain: None,
            qualifies: false,
            excluded_because: Some(reasons),
        });
    }

    // Conversion is through packaged converters only. The only packaged converter
    // currently wired is the native ATIF reader; other stores are unconverted and
    // recorded as not redactable.
    let (text, summary, digest) = match candidate.kind {
        TraceSourceKind::OpenagentsExport => {
            let text = fs::read_to_string(&path)?;
            let digest = format!("sha256:{:x}", Sha256::digest(text.as_bytes()));
            let summary = trace::summarize_trace_file(&path).ok();
            (text, summary, Some(digest))
        }
        _ => {
            reasons.push("not_redactable".to_string());
            (String::new(), None, None)
        }
    };

    if !summary.as_ref().is_some_and(|s| s.format == "atif") && reasons.is_empty() {
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
    } else if is_private_repo_hint(repo_hint.as_ref().unwrap()) {
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
