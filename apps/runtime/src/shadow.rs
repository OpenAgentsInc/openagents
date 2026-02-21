use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum ShadowError {
    #[error("shadow file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("shadow parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowSnapshotManifest {
    pub run_id: String,
    pub receipt_path: PathBuf,
    pub replay_path: PathBuf,
    pub summary_path: PathBuf,
    pub checkpoint_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowGatePolicy {
    pub max_warning_count: u64,
    pub block_on_critical: bool,
}

impl Default for ShadowGatePolicy {
    fn default() -> Self {
        Self {
            max_warning_count: 0,
            block_on_critical: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowParityReport {
    pub schema: String,
    pub generated_at: String,
    pub legacy_run_id: String,
    pub rust_run_id: String,
    pub diffs: Vec<ShadowDiff>,
    pub totals: ShadowDiffTotals,
    pub gate: ShadowGateResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowDiff {
    pub severity: DiffSeverity,
    pub field: String,
    pub legacy: String,
    pub rust: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffSeverity {
    Critical,
    Warning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowDiffTotals {
    pub critical: u64,
    pub warning: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShadowGateResult {
    pub decision: GateDecision,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    Allow,
    Block,
}

#[derive(Clone, Debug, Deserialize)]
struct ReceiptDoc {
    session_id: String,
    trajectory_hash: String,
    event_count: usize,
    last_seq: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct SummaryEnvelope {
    summary: SummaryDoc,
}

#[derive(Clone, Debug, Deserialize)]
struct SummaryDoc {
    status: String,
    last_seq: u64,
    event_count: u64,
    projection_hash: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckpointEnvelope {
    checkpoint: CheckpointDoc,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckpointDoc {
    last_seq: u64,
    last_event_type: String,
}

#[derive(Clone, Debug)]
struct ShadowSnapshot {
    run_id: String,
    session_id: String,
    trajectory_hash: String,
    receipt_event_count: usize,
    receipt_last_seq: u64,
    replay_hash: String,
    replay_runtime_event_count: usize,
    summary_status: String,
    summary_last_seq: u64,
    summary_event_count: u64,
    summary_projection_hash: String,
    checkpoint_last_seq: u64,
    checkpoint_last_event_type: String,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<ShadowSnapshotManifest, ShadowError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn generate_parity_report(
    legacy_manifest: &ShadowSnapshotManifest,
    rust_manifest: &ShadowSnapshotManifest,
    policy: &ShadowGatePolicy,
    legacy_manifest_path: impl AsRef<Path>,
    rust_manifest_path: impl AsRef<Path>,
) -> Result<ShadowParityReport, ShadowError> {
    let legacy = load_snapshot(legacy_manifest, legacy_manifest_path.as_ref())?;
    let rust = load_snapshot(rust_manifest, rust_manifest_path.as_ref())?;
    let diffs = compare_snapshots(&legacy, &rust);
    let totals = summarize_diffs(&diffs);
    let gate = evaluate_gate(&totals, policy);

    Ok(ShadowParityReport {
        schema: "openagents.runtime.shadow_parity.v1".to_string(),
        generated_at: Utc::now().to_rfc3339(),
        legacy_run_id: legacy.run_id,
        rust_run_id: rust.run_id,
        diffs,
        totals,
        gate,
    })
}

pub fn write_report(
    path: impl AsRef<Path>,
    report: &ShadowParityReport,
) -> Result<(), ShadowError> {
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn load_snapshot(
    manifest: &ShadowSnapshotManifest,
    manifest_path: &Path,
) -> Result<ShadowSnapshot, ShadowError> {
    let base = manifest_path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let receipt: ReceiptDoc = read_json(resolve_relative(&base, &manifest.receipt_path))?;
    let summary: SummaryEnvelope = read_json(resolve_relative(&base, &manifest.summary_path))?;
    let checkpoint: CheckpointEnvelope =
        read_json(resolve_relative(&base, &manifest.checkpoint_path))?;
    let replay_bytes = fs::read_to_string(resolve_relative(&base, &manifest.replay_path))?;
    let replay_runtime_event_count = replay_bytes
        .lines()
        .filter(|line| line.contains("\"type\":\"RuntimeEvent\""))
        .count();
    let replay_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(replay_bytes.as_bytes()))
    );

    Ok(ShadowSnapshot {
        run_id: manifest.run_id.clone(),
        session_id: receipt.session_id,
        trajectory_hash: receipt.trajectory_hash,
        receipt_event_count: receipt.event_count,
        receipt_last_seq: receipt.last_seq,
        replay_hash,
        replay_runtime_event_count,
        summary_status: summary.summary.status,
        summary_last_seq: summary.summary.last_seq,
        summary_event_count: summary.summary.event_count,
        summary_projection_hash: summary.summary.projection_hash,
        checkpoint_last_seq: checkpoint.checkpoint.last_seq,
        checkpoint_last_event_type: checkpoint.checkpoint.last_event_type,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, ShadowError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn resolve_relative(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn compare_snapshots(legacy: &ShadowSnapshot, rust: &ShadowSnapshot) -> Vec<ShadowDiff> {
    let mut diffs = Vec::new();
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "run_id",
        &legacy.run_id,
        &rust.run_id,
        "run identifiers diverged",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "session_id",
        &legacy.session_id,
        &rust.session_id,
        "receipt session IDs diverged",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "trajectory_hash",
        &legacy.trajectory_hash,
        &rust.trajectory_hash,
        "trajectory hash mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "summary_status",
        &legacy.summary_status,
        &rust.summary_status,
        "projected run status mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "summary_last_seq",
        &legacy.summary_last_seq.to_string(),
        &rust.summary_last_seq.to_string(),
        "projected last sequence mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Critical,
        "checkpoint_last_seq",
        &legacy.checkpoint_last_seq.to_string(),
        &rust.checkpoint_last_seq.to_string(),
        "checkpoint sequence mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "replay_hash",
        &legacy.replay_hash,
        &rust.replay_hash,
        "replay hash mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "replay_runtime_event_count",
        &legacy.replay_runtime_event_count.to_string(),
        &rust.replay_runtime_event_count.to_string(),
        "runtime replay event count mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "receipt_event_count",
        &legacy.receipt_event_count.to_string(),
        &rust.receipt_event_count.to_string(),
        "receipt event count mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "receipt_last_seq",
        &legacy.receipt_last_seq.to_string(),
        &rust.receipt_last_seq.to_string(),
        "receipt last sequence mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "summary_event_count",
        &legacy.summary_event_count.to_string(),
        &rust.summary_event_count.to_string(),
        "summary event count mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "summary_projection_hash",
        &legacy.summary_projection_hash,
        &rust.summary_projection_hash,
        "projection hash mismatch",
    );
    push_if_diff(
        &mut diffs,
        DiffSeverity::Warning,
        "checkpoint_last_event_type",
        &legacy.checkpoint_last_event_type,
        &rust.checkpoint_last_event_type,
        "checkpoint event type mismatch",
    );
    diffs
}

fn push_if_diff(
    diffs: &mut Vec<ShadowDiff>,
    severity: DiffSeverity,
    field: &str,
    legacy: &str,
    rust: &str,
    message: &str,
) {
    if legacy != rust {
        diffs.push(ShadowDiff {
            severity,
            field: field.to_string(),
            legacy: legacy.to_string(),
            rust: rust.to_string(),
            message: message.to_string(),
        });
    }
}

fn summarize_diffs(diffs: &[ShadowDiff]) -> ShadowDiffTotals {
    let critical = diffs
        .iter()
        .filter(|diff| diff.severity == DiffSeverity::Critical)
        .count() as u64;
    let warning = diffs
        .iter()
        .filter(|diff| diff.severity == DiffSeverity::Warning)
        .count() as u64;
    ShadowDiffTotals { critical, warning }
}

fn evaluate_gate(totals: &ShadowDiffTotals, policy: &ShadowGatePolicy) -> ShadowGateResult {
    if policy.block_on_critical && totals.critical > 0 {
        return ShadowGateResult {
            decision: GateDecision::Block,
            reason: "critical parity diffs detected".to_string(),
        };
    }
    if totals.warning > policy.max_warning_count {
        return ShadowGateResult {
            decision: GateDecision::Block,
            reason: format!(
                "warning parity diffs ({}) exceed policy threshold ({})",
                totals.warning, policy.max_warning_count
            ),
        };
    }
    ShadowGateResult {
        decision: GateDecision::Allow,
        reason: "parity within configured thresholds".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use anyhow::{Result, anyhow};
    use tempfile::tempdir;

    use super::{
        GateDecision, ShadowGatePolicy, ShadowSnapshotManifest, generate_parity_report,
        write_report,
    };

    #[test]
    fn shadow_report_blocks_on_critical_diffs() -> Result<()> {
        let dir = tempdir()?;
        let legacy_manifest = write_manifest_bundle(
            dir.path().join("legacy"),
            "run-1",
            "sha256:legacy",
            "running",
            2,
            "run.step.completed",
        )?;
        let rust_manifest = write_manifest_bundle(
            dir.path().join("rust"),
            "run-1",
            "sha256:rust",
            "running",
            2,
            "run.step.completed",
        )?;

        let report = generate_parity_report(
            &legacy_manifest.0,
            &rust_manifest.0,
            &ShadowGatePolicy::default(),
            &legacy_manifest.1,
            &rust_manifest.1,
        )?;
        if report.gate.decision != GateDecision::Block {
            return Err(anyhow!("expected gate decision=block for critical diff"));
        }
        Ok(())
    }

    #[test]
    fn shadow_report_allows_when_snapshots_match() -> Result<()> {
        let dir = tempdir()?;
        let legacy_manifest = write_manifest_bundle(
            dir.path().join("legacy"),
            "run-1",
            "sha256:same",
            "succeeded",
            2,
            "run.finished",
        )?;
        let rust_manifest = write_manifest_bundle(
            dir.path().join("rust"),
            "run-1",
            "sha256:same",
            "succeeded",
            2,
            "run.finished",
        )?;

        let report = generate_parity_report(
            &legacy_manifest.0,
            &rust_manifest.0,
            &ShadowGatePolicy::default(),
            &legacy_manifest.1,
            &rust_manifest.1,
        )?;
        if report.gate.decision != GateDecision::Allow {
            return Err(anyhow!(
                "expected gate decision=allow for matching snapshots"
            ));
        }

        let output_path = dir.path().join("parity-report.json");
        write_report(&output_path, &report)?;
        if !output_path.exists() {
            return Err(anyhow!("parity report file was not created"));
        }
        Ok(())
    }

    fn write_manifest_bundle(
        base: PathBuf,
        run_id: &str,
        trajectory_hash: &str,
        summary_status: &str,
        last_seq: u64,
        last_event_type: &str,
    ) -> Result<(ShadowSnapshotManifest, PathBuf)> {
        fs::create_dir_all(&base)?;
        let receipt_path = base.join("receipt.json");
        let replay_path = base.join("replay.jsonl");
        let summary_path = base.join("summary.json");
        let checkpoint_path = base.join("checkpoint.json");
        let manifest_path = base.join("manifest.json");

        fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "session_id": run_id,
                "trajectory_hash": trajectory_hash,
                "event_count": 2,
                "last_seq": last_seq
            }))?,
        )?;
        fs::write(
            &replay_path,
            r#"{"type":"ReplayHeader"}{"type":"SessionStart"}{"type":"RuntimeEvent"}{"type":"SessionEnd"}"#,
        )?;
        fs::write(
            &summary_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "summary": {
                    "status": summary_status,
                    "last_seq": last_seq,
                    "event_count": 2,
                    "projection_hash": "sha256:projection"
                }
            }))?,
        )?;
        fs::write(
            &checkpoint_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "checkpoint": {
                    "last_seq": last_seq,
                    "last_event_type": last_event_type
                }
            }))?,
        )?;

        let manifest = ShadowSnapshotManifest {
            run_id: run_id.to_string(),
            receipt_path: PathBuf::from("receipt.json"),
            replay_path: PathBuf::from("replay.jsonl"),
            summary_path: PathBuf::from("summary.json"),
            checkpoint_path: PathBuf::from("checkpoint.json"),
        };
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        Ok((manifest, manifest_path))
    }
}
