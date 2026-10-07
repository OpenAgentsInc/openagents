//! Answer-file integrity checks for the legal benchmark runner and scorer.
//!
//! The runner may create directories, logs, manifests, and receipts. It must
//! not create or patch answer content. This module records that boundary in a
//! machine-readable report.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ArtifactManifest, BenchmarkTaskSpec, LegalBenchmarkPathRoot, ToolCallRecord,
    stable_json_digest,
};

pub const LEGAL_BENCHMARK_INTEGRITY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunActor {
    Model,
    Harness,
    Scorer,
    Human,
    Worker,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnswerFileIntegrityReceipt {
    pub relative_path: String,
    pub required_by_task: bool,
    pub declared_in_manifest: bool,
    pub exists: bool,
    pub creation_actor: RunActor,
    pub last_modifying_actor: RunActor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writer_tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_score_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_score_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime_ms: Option<u64>,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkAnswerIntegrityReport {
    pub schema_version: u16,
    pub valid: bool,
    pub checked_at_ms: u64,
    pub report_hash: String,
    pub answer_files: Vec<AnswerFileIntegrityReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_reasons: Vec<String>,
}

impl Default for LegalBenchmarkAnswerIntegrityReport {
    fn default() -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_INTEGRITY_SCHEMA_VERSION,
            valid: false,
            checked_at_ms: 0,
            report_hash: String::from("legacy_missing_answer_integrity"),
            answer_files: Vec::new(),
            invalid_reasons: vec![String::from("answer_integrity_missing_from_legacy_receipt")],
        }
    }
}

#[derive(Clone, Debug)]
pub struct BenchmarkIntegrityGuard {
    output_root: PathBuf,
    task_spec: BenchmarkTaskSpec,
    output_manifest: ArtifactManifest,
    tool_calls: Vec<ToolCallRecord>,
    pre_score_snapshots: BTreeMap<String, AnswerFileSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnswerFileSnapshot {
    exists: bool,
    sha256: Option<String>,
    byte_size: Option<u64>,
    mtime_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelWriteEvidence {
    tool_call_id: String,
    after_hash: Option<String>,
}

#[derive(Debug, Error)]
pub enum BenchmarkIntegrityError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid output-relative answer path `{0}`")]
    InvalidRelativePath(String),
    #[error("failed to calculate integrity digest: {0}")]
    Digest(#[from] serde_json::Error),
}

impl BenchmarkIntegrityGuard {
    pub fn before_scoring(
        task_spec: &BenchmarkTaskSpec,
        output_manifest: &ArtifactManifest,
        output_root: impl AsRef<Path>,
        tool_calls: &[ToolCallRecord],
    ) -> Result<Self, BenchmarkIntegrityError> {
        let output_root = output_root.as_ref().to_path_buf();
        let mut pre_score_snapshots = BTreeMap::new();
        for (relative_path, _) in answer_file_paths(task_spec, output_manifest) {
            pre_score_snapshots.insert(
                relative_path.clone(),
                snapshot_answer_file(&output_root, relative_path.as_str())?,
            );
        }
        Ok(Self {
            output_root,
            task_spec: task_spec.clone(),
            output_manifest: output_manifest.clone(),
            tool_calls: tool_calls.to_vec(),
            pre_score_snapshots,
        })
    }

    pub fn finalize_after_scoring(
        &self,
    ) -> Result<LegalBenchmarkAnswerIntegrityReport, BenchmarkIntegrityError> {
        build_answer_integrity_report_from_snapshots(
            &self.task_spec,
            &self.output_manifest,
            &self.output_root,
            &self.tool_calls,
            &self.pre_score_snapshots,
        )
    }
}

pub fn build_answer_integrity_report(
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &ArtifactManifest,
    output_root: impl AsRef<Path>,
    tool_calls: &[ToolCallRecord],
) -> Result<LegalBenchmarkAnswerIntegrityReport, BenchmarkIntegrityError> {
    let guard = BenchmarkIntegrityGuard::before_scoring(
        task_spec,
        output_manifest,
        output_root,
        tool_calls,
    )?;
    guard.finalize_after_scoring()
}

fn build_answer_integrity_report_from_snapshots(
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &ArtifactManifest,
    output_root: &Path,
    tool_calls: &[ToolCallRecord],
    pre_score_snapshots: &BTreeMap<String, AnswerFileSnapshot>,
) -> Result<LegalBenchmarkAnswerIntegrityReport, BenchmarkIntegrityError> {
    let paths = answer_file_paths(task_spec, output_manifest);
    let mut answer_files = Vec::new();
    let mut invalid_reasons = Vec::new();

    for (relative_path, path_role) in paths {
        let pre_snapshot =
            pre_score_snapshots
                .get(&relative_path)
                .cloned()
                .unwrap_or(AnswerFileSnapshot {
                    exists: false,
                    sha256: None,
                    byte_size: None,
                    mtime_ms: None,
                });
        let post_snapshot = snapshot_answer_file(output_root, relative_path.as_str())?;
        let write_evidence = latest_model_write_for_path(tool_calls, relative_path.as_str());
        let mut failure_reasons = Vec::new();

        if path_role.required_by_task && !post_snapshot.exists {
            failure_reasons.push(String::from("required_answer_file_missing"));
        }
        if path_role.declared_in_manifest && !post_snapshot.exists {
            failure_reasons.push(String::from("manifest_answer_file_missing"));
        }
        if post_snapshot.exists && write_evidence.is_none() {
            failure_reasons.push(String::from("answer_file_has_no_model_write"));
        }
        if pre_snapshot.sha256 != post_snapshot.sha256 {
            failure_reasons.push(String::from("answer_file_changed_during_scoring"));
        }
        if let Some(evidence) = &write_evidence {
            match (&evidence.after_hash, &post_snapshot.sha256) {
                (Some(expected), Some(actual)) if expected == actual => {}
                (Some(_), Some(_)) => failure_reasons.push(String::from(
                    "actual_hash_does_not_match_model_write_after_hash",
                )),
                (None, _) => failure_reasons.push(String::from("model_write_missing_after_hash")),
                (_, None) => {}
            }
        }

        let valid = failure_reasons.is_empty();
        if !valid {
            invalid_reasons.extend(
                failure_reasons
                    .iter()
                    .map(|reason| format!("{relative_path}:{reason}")),
            );
        }
        let actor = if write_evidence.is_some() && valid {
            RunActor::Model
        } else if write_evidence.is_some() {
            RunActor::Unknown
        } else if post_snapshot.exists {
            RunActor::Harness
        } else {
            RunActor::Unknown
        };
        answer_files.push(AnswerFileIntegrityReceipt {
            relative_path,
            required_by_task: path_role.required_by_task,
            declared_in_manifest: path_role.declared_in_manifest,
            exists: post_snapshot.exists,
            creation_actor: actor,
            last_modifying_actor: actor,
            writer_tool_call_id: write_evidence.map(|evidence| evidence.tool_call_id),
            pre_score_hash: pre_snapshot.sha256,
            post_score_hash: post_snapshot.sha256,
            byte_size: post_snapshot.byte_size,
            mtime_ms: post_snapshot.mtime_ms,
            valid,
            failure_reasons,
        });
    }

    let valid = invalid_reasons.is_empty();
    let checked_at_ms = now_ms();
    let report_hash = stable_json_digest(
        "psionic.legal_benchmark.answer_integrity.report.v1",
        &json!({
            "schema_version": LEGAL_BENCHMARK_INTEGRITY_SCHEMA_VERSION,
            "valid": valid,
            "answer_files": answer_files,
            "invalid_reasons": invalid_reasons,
        }),
    )?;
    Ok(LegalBenchmarkAnswerIntegrityReport {
        schema_version: LEGAL_BENCHMARK_INTEGRITY_SCHEMA_VERSION,
        valid,
        checked_at_ms,
        report_hash,
        answer_files,
        invalid_reasons,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AnswerPathRole {
    required_by_task: bool,
    declared_in_manifest: bool,
}

fn answer_file_paths(
    task_spec: &BenchmarkTaskSpec,
    output_manifest: &ArtifactManifest,
) -> BTreeMap<String, AnswerPathRole> {
    let mut paths = BTreeMap::new();
    for deliverable in task_spec.deliverables.iter().filter(|d| d.required) {
        paths.insert(
            deliverable.required_path.clone(),
            AnswerPathRole {
                required_by_task: true,
                declared_in_manifest: false,
            },
        );
    }
    for artifact in &output_manifest.artifacts {
        paths
            .entry(artifact.relative_path.clone())
            .and_modify(|role| role.declared_in_manifest = true)
            .or_insert(AnswerPathRole {
                required_by_task: false,
                declared_in_manifest: true,
            });
    }
    paths
}

fn snapshot_answer_file(
    output_root: &Path,
    relative_path: &str,
) -> Result<AnswerFileSnapshot, BenchmarkIntegrityError> {
    let path = output_file_path(output_root, relative_path)?;
    if !path.exists() {
        return Ok(AnswerFileSnapshot {
            exists: false,
            sha256: None,
            byte_size: None,
            mtime_ms: None,
        });
    }
    let metadata = fs::metadata(&path).map_err(|source| BenchmarkIntegrityError::Io {
        path: path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Ok(AnswerFileSnapshot {
            exists: false,
            sha256: None,
            byte_size: None,
            mtime_ms: metadata.modified().ok().and_then(system_time_ms),
        });
    }
    let bytes = fs::read(&path).map_err(|source| BenchmarkIntegrityError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(AnswerFileSnapshot {
        exists: true,
        sha256: Some(sha256_hex(&bytes)),
        byte_size: Some(metadata.len()),
        mtime_ms: metadata.modified().ok().and_then(system_time_ms),
    })
}

fn latest_model_write_for_path(
    tool_calls: &[ToolCallRecord],
    relative_path: &str,
) -> Option<ModelWriteEvidence> {
    tool_calls
        .iter()
        .rev()
        .find_map(|call| model_write_evidence(call, relative_path))
}

fn model_write_evidence(call: &ToolCallRecord, relative_path: &str) -> Option<ModelWriteEvidence> {
    if call.error_kind.is_some() || !matches!(call.tool_name.as_str(), "write" | "edit") {
        return None;
    }
    let input = tool_payload(&call.input)?;
    if path_root(input)? != LegalBenchmarkPathRoot::Output {
        return None;
    }
    if input
        .get("relative_path")
        .and_then(Value::as_str)
        .is_some_and(|path| path == relative_path)
    {
        Some(ModelWriteEvidence {
            tool_call_id: call.tool_call_id.clone(),
            after_hash: tool_after_hash(call),
        })
    } else {
        None
    }
}

fn tool_payload(value: &Value) -> Option<&Value> {
    value.get("input").unwrap_or(value).as_object()?;
    Some(value.get("input").unwrap_or(value))
}

fn path_root(value: &Value) -> Option<LegalBenchmarkPathRoot> {
    match value.get("root").and_then(Value::as_str)? {
        "output" => Some(LegalBenchmarkPathRoot::Output),
        "workspace" => Some(LegalBenchmarkPathRoot::Workspace),
        "documents" => Some(LegalBenchmarkPathRoot::Documents),
        _ => None,
    }
}

fn tool_after_hash(call: &ToolCallRecord) -> Option<String> {
    let output = call.output.as_ref()?.get("output")?;
    output
        .get("after_hash")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn output_file_path(
    output_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, BenchmarkIntegrityError> {
    let path = Path::new(relative_path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err(BenchmarkIntegrityError::InvalidRelativePath(
            relative_path.to_owned(),
        ));
    }
    Ok(output_root.join(path))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn system_time_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn now_ms() -> u64 {
    system_time_ms(SystemTime::now()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, ArtifactManifestRole, DataClassification, DeliverableKind, DeliverableSpec,
        JudgeMode, JudgePolicy, ToolPolicy, artifact_from_file, build_output_artifact_manifest,
    };
    use serde_json::json;

    fn task() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.integrity"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("draft"),
            title: String::from("Integrity task"),
            instructions: String::from("Write the memo."),
            work_type: String::from("draft"),
            tags: Vec::new(),
            source_artifacts: Vec::new(),
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Memo"),
                required: true,
            }],
            criteria: Vec::new(),
            judge_policy: JudgePolicy {
                mode: JudgeMode::Deterministic,
                provider: String::from("mock"),
                model: String::from("mock"),
                prompt_template_id: String::from("judge"),
                prompt_template_hash: String::from("hash"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("write")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 2,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: BTreeMap::new(),
        }
    }

    fn write_call(relative_path: &str, content: &str) -> ToolCallRecord {
        let after_hash = sha256_hex(content.as_bytes());
        ToolCallRecord {
            tool_call_id: String::from("call.write.memo"),
            tool_name: String::from("write"),
            call_event_index: 1,
            result_event_index: Some(2),
            input: json!({
                "tool": "write",
                "input": {
                    "root": "output",
                    "relative_path": relative_path,
                    "content": content,
                    "overwrite": true
                }
            }),
            output: Some(json!({
                "tool": "write",
                "output": {
                    "relative_path": relative_path,
                    "bytes_written": content.len(),
                    "after_hash": after_hash
                }
            })),
            error_kind: None,
            elapsed_ms: 1,
        }
    }

    fn output_manifest(root: &Path, content: &str) -> ArtifactManifest {
        let path = root.join("memo.md");
        fs::write(&path, content).expect("write memo");
        let artifact = artifact_from_file(
            "artifact.output.memo",
            ArtifactKind::GeneratedDeliverable,
            root,
            &path,
            DataClassification::BenchmarkConfidential,
            Some(String::from("model")),
        )
        .expect("artifact");
        let manifest =
            build_output_artifact_manifest("legal.integrity", "v1", "run", vec![artifact]);
        assert_eq!(manifest.manifest_role, ArtifactManifestRole::Output);
        manifest
    }

    #[test]
    fn benchmark_integrity_accepts_model_written_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let content = "# Memo\n\nModel wrote this.\n";
        let manifest = output_manifest(root, content);
        let report = build_answer_integrity_report(
            &task(),
            &manifest,
            root,
            &[write_call("memo.md", content)],
        )
        .expect("report");

        assert!(report.valid);
        assert_eq!(report.answer_files.len(), 1);
        assert_eq!(report.answer_files[0].creation_actor, RunActor::Model);
        assert_eq!(report.answer_files[0].last_modifying_actor, RunActor::Model);
        assert_eq!(
            report.answer_files[0].pre_score_hash,
            report.answer_files[0].post_score_hash
        );
    }

    #[test]
    fn benchmark_integrity_rejects_missing_required_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = build_output_artifact_manifest("legal.integrity", "v1", "run", Vec::new());
        let report =
            build_answer_integrity_report(&task(), &manifest, temp.path(), &[]).expect("report");

        assert!(!report.valid);
        assert!(
            report
                .invalid_reasons
                .iter()
                .any(|reason| reason.contains("required_answer_file_missing"))
        );
    }

    #[test]
    fn benchmark_integrity_rejects_harness_created_output_without_model_write() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = output_manifest(temp.path(), "# Memo\n\nHarness wrote this.\n");
        let report =
            build_answer_integrity_report(&task(), &manifest, temp.path(), &[]).expect("report");

        assert!(!report.valid);
        assert_eq!(report.answer_files[0].creation_actor, RunActor::Harness);
        assert!(
            report
                .invalid_reasons
                .iter()
                .any(|reason| reason.contains("answer_file_has_no_model_write"))
        );
    }

    #[test]
    fn benchmark_integrity_rejects_post_score_hash_drift() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let content = "# Memo\n\nBefore score.\n";
        let manifest = output_manifest(root, content);
        let call = write_call("memo.md", content);
        let guard = BenchmarkIntegrityGuard::before_scoring(&task(), &manifest, root, &[call])
            .expect("guard");

        fs::write(root.join("memo.md"), "# Memo\n\nScorer patched this.\n").expect("mutate");
        let report = guard.finalize_after_scoring().expect("report");

        assert!(!report.valid);
        assert!(
            report
                .invalid_reasons
                .iter()
                .any(|reason| reason.contains("answer_file_changed_during_scoring"))
        );
    }

    #[test]
    fn benchmark_integrity_rejects_legacy_marker_style_harness_mutation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let model_content = "# Memo\n\nModel draft.\n";
        let mutated_content = "# Memo\n\nModel draft.\n\nC-001 C-002 C-003\n";
        let manifest = output_manifest(root, mutated_content);
        let report = build_answer_integrity_report(
            &task(),
            &manifest,
            root,
            &[write_call("memo.md", model_content)],
        )
        .expect("report");

        assert!(!report.valid);
        assert!(
            report
                .invalid_reasons
                .iter()
                .any(|reason| reason.contains("actual_hash_does_not_match_model_write_after_hash"))
        );
    }
}
