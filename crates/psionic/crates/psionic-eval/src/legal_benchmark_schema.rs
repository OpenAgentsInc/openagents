//! Canonical legal benchmark schemas.
//!
//! These structs are the stable receipt and dataset shapes used to move legal
//! benchmark runs into training, replay, promotion, and audit systems. The
//! first version wraps the existing legal benchmark runner records instead of
//! replacing them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    ArtifactManifest, LegalBenchmarkAnswerIntegrityReport, Metadata, RunActor, RunRecord,
    ScoreReport, SourceArtifact, ToolCallRecord, run_record_digest, score_report_digest,
    stable_json_digest,
};

pub const LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkVisibility {
    Public,
    Private,
    Hidden,
    Synthetic,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalThinkingMode {
    Enabled,
    Disabled,
    Auto,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalModelActionKind {
    SystemMessage,
    UserMessage,
    AssistantMessage,
    ToolCall,
    ToolResult,
    RunnerEvent,
    JudgeEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalPromotionDecisionKind {
    Promote,
    Hold,
    Reject,
    Quarantine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalFailureClass {
    DidNotWriteRequiredFile,
    WroteWrongPath,
    WroteEmptyFile,
    WroteTooLong,
    WroteTooShort,
    FailedToUseSources,
    HallucinatedCitations,
    DidNotSubmit,
    ToolCallMalformed,
    InvalidJson,
    HarnessIntegrityFailure,
    ScorerUnavailable,
    Timeout,
    ModelRefusal,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalContentDigest {
    pub algorithm: String,
    pub value: String,
}

impl LegalContentDigest {
    pub fn sha256(value: impl Into<String>) -> Self {
        Self {
            algorithm: String::from("sha256"),
            value: value.into(),
        }
    }

    pub fn validate(&self, field: &'static str) -> Result<(), LegalSchemaError> {
        require_non_empty(field, self.value.as_str())?;
        match self.algorithm.as_str() {
            "sha256" | "blake3" => Ok(()),
            _ => Err(LegalSchemaError::Validation {
                field,
                detail: String::from("digest algorithm must be sha256 or blake3"),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalSourceDocument {
    pub schema_version: u16,
    pub document_id: String,
    pub relative_path: String,
    pub media_type: String,
    pub byte_size: u64,
    pub content_hash: LegalContentDigest,
    pub classification: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

impl LegalSourceDocument {
    pub fn from_source_artifact(artifact: &SourceArtifact) -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            document_id: artifact.artifact_id.clone(),
            relative_path: artifact.relative_path.clone(),
            media_type: artifact.media_type.clone(),
            byte_size: artifact.byte_size,
            content_hash: LegalContentDigest::sha256(artifact.sha256.clone()),
            classification: format!("{:?}", artifact.data_classification),
            provenance: artifact.provenance.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("source_document.schema_version", self.schema_version)?;
        require_non_empty("source_document.document_id", self.document_id.as_str())?;
        require_non_empty("source_document.relative_path", self.relative_path.as_str())?;
        self.content_hash.validate("source_document.content_hash")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalTaskSpec {
    pub schema_version: u16,
    pub task_id: String,
    pub task_version: String,
    pub benchmark_id: String,
    pub benchmark_visibility: LegalBenchmarkVisibility,
    pub title: String,
    pub source_documents: Vec<LegalSourceDocument>,
    pub required_answer_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRunSpec {
    pub schema_version: u16,
    pub run_id: String,
    pub task_id: String,
    pub task_version: String,
    pub benchmark_id: String,
    pub benchmark_visibility: LegalBenchmarkVisibility,
    pub base_model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    pub tokenizer_id: String,
    pub tokenizer_hash: LegalContentDigest,
    pub prompt_template_id: String,
    pub prompt_template_hash: LegalContentDigest,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub inference_settings: Metadata,
    pub thinking_mode: LegalThinkingMode,
    pub tool_list: Vec<String>,
    pub source_document_hashes: Vec<LegalContentDigest>,
    pub git_commit: String,
    pub git_dirty: bool,
    pub deterministic_replay_command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_summary: Option<Metadata>,
}

impl LegalRunSpec {
    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("run_spec.schema_version", self.schema_version)?;
        require_non_empty("run_spec.run_id", self.run_id.as_str())?;
        require_non_empty("run_spec.task_id", self.task_id.as_str())?;
        require_non_empty("run_spec.benchmark_id", self.benchmark_id.as_str())?;
        require_non_empty("run_spec.base_model_id", self.base_model_id.as_str())?;
        require_non_empty("run_spec.tokenizer_id", self.tokenizer_id.as_str())?;
        self.tokenizer_hash.validate("run_spec.tokenizer_hash")?;
        require_non_empty(
            "run_spec.prompt_template_id",
            self.prompt_template_id.as_str(),
        )?;
        self.prompt_template_hash
            .validate("run_spec.prompt_template_hash")?;
        require_non_empty("run_spec.git_commit", self.git_commit.as_str())?;
        if self.tool_list.is_empty() {
            return Err(LegalSchemaError::Validation {
                field: "run_spec.tool_list",
                detail: String::from("tool list must not be empty"),
            });
        }
        if self.source_document_hashes.is_empty() {
            return Err(LegalSchemaError::Validation {
                field: "run_spec.source_document_hashes",
                detail: String::from(
                    "at least one source document or input manifest hash is required",
                ),
            });
        }
        for hash in &self.source_document_hashes {
            hash.validate("run_spec.source_document_hashes")?;
        }
        if self.deterministic_replay_command.is_empty() {
            return Err(LegalSchemaError::Validation {
                field: "run_spec.deterministic_replay_command",
                detail: String::from("deterministic replay command must not be empty"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalToolCall {
    pub schema_version: u16,
    pub tool_call_id: String,
    pub tool_name: String,
    pub call_event_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_event_index: Option<u64>,
    pub input_hash: LegalContentDigest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<LegalContentDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    pub elapsed_ms: u64,
}

impl LegalToolCall {
    pub fn from_tool_call_record(call: &ToolCallRecord) -> Result<Self, LegalSchemaError> {
        Ok(Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            tool_call_id: call.tool_call_id.clone(),
            tool_name: call.tool_name.clone(),
            call_event_index: call.call_event_index,
            result_event_index: call.result_event_index,
            input_hash: LegalContentDigest::sha256(stable_json_digest(
                "psionic.legal_benchmark.canonical.tool_input.v1",
                &call.input,
            )?),
            output_hash: call
                .output
                .as_ref()
                .map(|output| {
                    stable_json_digest("psionic.legal_benchmark.canonical.tool_output.v1", output)
                        .map(LegalContentDigest::sha256)
                })
                .transpose()?,
            error_kind: call.error_kind.clone(),
            elapsed_ms: call.elapsed_ms,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalModelAction {
    pub schema_version: u16,
    pub action_id: String,
    pub action_index: u64,
    pub action_kind: LegalModelActionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<LegalContentDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalAnswerFile {
    pub schema_version: u16,
    pub relative_path: String,
    pub byte_size: u64,
    pub content_hash: LegalContentDigest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_score_hash: Option<LegalContentDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_score_hash: Option<LegalContentDigest>,
    pub creation_actor: RunActor,
    pub last_modifying_actor: RunActor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writer_tool_call_id: Option<String>,
    pub integrity_valid: bool,
}

impl LegalAnswerFile {
    pub fn from_output_artifact(
        artifact: &SourceArtifact,
        integrity: &LegalBenchmarkAnswerIntegrityReport,
    ) -> Self {
        let integrity_file = integrity
            .answer_files
            .iter()
            .find(|file| file.relative_path == artifact.relative_path);
        Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            relative_path: artifact.relative_path.clone(),
            byte_size: artifact.byte_size,
            content_hash: LegalContentDigest::sha256(artifact.sha256.clone()),
            pre_score_hash: integrity_file
                .and_then(|file| file.pre_score_hash.clone())
                .map(LegalContentDigest::sha256),
            post_score_hash: integrity_file
                .and_then(|file| file.post_score_hash.clone())
                .map(LegalContentDigest::sha256),
            creation_actor: integrity_file
                .map(|file| file.creation_actor)
                .unwrap_or(RunActor::Unknown),
            last_modifying_actor: integrity_file
                .map(|file| file.last_modifying_actor)
                .unwrap_or(RunActor::Unknown),
            writer_tool_call_id: integrity_file.and_then(|file| file.writer_tool_call_id.clone()),
            integrity_valid: integrity_file.map(|file| file.valid).unwrap_or(false),
        }
    }

    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("answer_file.schema_version", self.schema_version)?;
        require_non_empty("answer_file.relative_path", self.relative_path.as_str())?;
        self.content_hash.validate("answer_file.content_hash")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalScoreReceipt {
    pub schema_version: u16,
    pub score_report_id: String,
    pub scorer_version: String,
    pub score_report_hash: LegalContentDigest,
    pub all_pass: bool,
    pub criterion_pass_rate_bps: u32,
    pub document_coverage_bps: u32,
    pub criterion_count: u32,
    pub passed_criterion_count: u32,
    pub failure_diagnostics: Vec<String>,
}

impl LegalScoreReceipt {
    pub fn from_score_report(
        report: &ScoreReport,
        scorer_version: impl Into<String>,
    ) -> Result<Self, LegalSchemaError> {
        Ok(Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            score_report_id: report.score_report_id.clone(),
            scorer_version: scorer_version.into(),
            score_report_hash: LegalContentDigest::sha256(score_report_digest(report)?),
            all_pass: report.all_pass,
            criterion_pass_rate_bps: report.criterion_pass_rate_bps,
            document_coverage_bps: report.document_coverage_bps,
            criterion_count: u32::try_from(report.criterion_results.len()).unwrap_or(u32::MAX),
            passed_criterion_count: u32::try_from(
                report
                    .criterion_results
                    .iter()
                    .filter(|criterion| criterion.passed)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            failure_diagnostics: report.failure_diagnostics.clone(),
        })
    }

    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("score.schema_version", self.schema_version)?;
        require_non_empty("score.score_report_id", self.score_report_id.as_str())?;
        require_non_empty("score.scorer_version", self.scorer_version.as_str())?;
        self.score_report_hash.validate("score.score_report_hash")?;
        if self.criterion_pass_rate_bps > 10_000 || self.document_coverage_bps > 10_000 {
            return Err(LegalSchemaError::Validation {
                field: "score",
                detail: String::from("basis-point scores must be <= 10000"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalIntegrityReceipt {
    pub schema_version: u16,
    pub valid: bool,
    pub integrity_report_hash: LegalContentDigest,
    pub invalid_reasons: Vec<String>,
}

impl LegalIntegrityReceipt {
    pub fn from_answer_integrity(report: &LegalBenchmarkAnswerIntegrityReport) -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            valid: report.valid,
            integrity_report_hash: LegalContentDigest::sha256(report.report_hash.clone()),
            invalid_reasons: report.invalid_reasons.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("integrity.schema_version", self.schema_version)?;
        self.integrity_report_hash
            .validate("integrity.integrity_report_hash")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRunReceipt {
    pub schema_version: u16,
    pub receipt_id: String,
    pub created_at_rfc3339: String,
    pub run_spec: LegalRunSpec,
    pub terminal_state: String,
    pub model_output_transcript: Vec<LegalModelAction>,
    pub tool_calls: Vec<LegalToolCall>,
    pub answer_files: Vec<LegalAnswerFile>,
    pub score: LegalScoreReceipt,
    pub integrity: LegalIntegrityReceipt,
    pub run_record_hash: LegalContentDigest,
    pub output_artifact_manifest_hash: LegalContentDigest,
    pub artifact_paths_and_hashes: Vec<LegalArtifactRef>,
    pub wall_clock_timings: LegalWallClockTimings,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

impl LegalRunReceipt {
    pub fn from_existing_run(
        input: LegalRunReceiptFromExisting<'_>,
    ) -> Result<Self, LegalSchemaError> {
        let source_document_hashes = if input.source_documents.is_empty() {
            vec![LegalContentDigest::sha256(
                input.run_record.input_artifact_manifest_hash.clone(),
            )]
        } else {
            input
                .source_documents
                .iter()
                .map(|document| document.content_hash.clone())
                .collect()
        };
        let answer_files = input
            .output_manifest
            .artifacts
            .iter()
            .map(|artifact| LegalAnswerFile::from_output_artifact(artifact, input.answer_integrity))
            .collect::<Vec<_>>();
        let scorer_version = input
            .score_report
            .metadata
            .get("scorer_version")
            .and_then(Value::as_str)
            .unwrap_or("psionic-eval.legal_benchmark_evaluator.v1");
        let mut artifact_paths_and_hashes = input
            .source_documents
            .iter()
            .map(|document| LegalArtifactRef {
                relative_path: document.relative_path.clone(),
                content_hash: document.content_hash.clone(),
            })
            .collect::<Vec<_>>();
        artifact_paths_and_hashes.extend(input.output_manifest.artifacts.iter().map(|artifact| {
            LegalArtifactRef {
                relative_path: artifact.relative_path.clone(),
                content_hash: LegalContentDigest::sha256(artifact.sha256.clone()),
            }
        }));
        artifact_paths_and_hashes
            .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let receipt = Self {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            receipt_id: format!("legal.run_receipt.{}", input.run_record.run_id),
            created_at_rfc3339: input.created_at_rfc3339.to_string(),
            run_spec: LegalRunSpec {
                schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                run_id: input.run_record.run_id.clone(),
                task_id: input.run_record.task_id.clone(),
                task_version: input.run_record.task_version.clone(),
                benchmark_id: input.benchmark_id.to_string(),
                benchmark_visibility: input.benchmark_visibility,
                base_model_id: input.base_model_id.to_string(),
                adapter_id: input.adapter_id.map(str::to_owned),
                tokenizer_id: input.tokenizer_id.to_string(),
                tokenizer_hash: LegalContentDigest::sha256(input.tokenizer_hash.to_string()),
                prompt_template_id: input.prompt_template_id.to_string(),
                prompt_template_hash: LegalContentDigest::sha256(
                    input.prompt_template_hash.to_string(),
                ),
                inference_settings: input.inference_settings,
                thinking_mode: input.thinking_mode,
                tool_list: input.tool_list,
                source_document_hashes,
                git_commit: input.git_commit.to_string(),
                git_dirty: input.git_dirty,
                deterministic_replay_command: input.deterministic_replay_command,
                worker_id: input.worker_id.map(str::to_owned),
                hardware_summary: input.hardware_summary,
            },
            terminal_state: format!("{:?}", input.run_record.terminal_state),
            model_output_transcript: input
                .run_record
                .transcript
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    Ok(LegalModelAction {
                        schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                        action_id: format!("{}.event.{index}", input.run_record.run_id),
                        action_index: event.event_index,
                        action_kind: match event.event_kind {
                            crate::TranscriptEventKind::System => {
                                LegalModelActionKind::SystemMessage
                            }
                            crate::TranscriptEventKind::User => LegalModelActionKind::UserMessage,
                            crate::TranscriptEventKind::Assistant => {
                                LegalModelActionKind::AssistantMessage
                            }
                            crate::TranscriptEventKind::ToolCall => LegalModelActionKind::ToolCall,
                            crate::TranscriptEventKind::ToolResult => {
                                LegalModelActionKind::ToolResult
                            }
                            crate::TranscriptEventKind::Runner => LegalModelActionKind::RunnerEvent,
                            crate::TranscriptEventKind::Judge => LegalModelActionKind::JudgeEvent,
                        },
                        role: event.role.clone(),
                        content_hash: event
                            .content
                            .as_ref()
                            .map(|content| {
                                stable_json_digest(
                                    "psionic.legal_benchmark.canonical.transcript_content.v1",
                                    content,
                                )
                            })
                            .transpose()
                            .map(|digest| digest.map(LegalContentDigest::sha256))?,
                        tool_call_id: event
                            .payload
                            .as_ref()
                            .and_then(|payload| payload.get("tool_call_id"))
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        timestamp_ms: event.timestamp_ms,
                    })
                })
                .collect::<Result<Vec<_>, LegalSchemaError>>()?,
            tool_calls: input
                .run_record
                .tool_calls
                .iter()
                .map(LegalToolCall::from_tool_call_record)
                .collect::<Result<Vec<_>, _>>()?,
            answer_files,
            score: LegalScoreReceipt::from_score_report(input.score_report, scorer_version)?,
            integrity: LegalIntegrityReceipt::from_answer_integrity(input.answer_integrity),
            run_record_hash: LegalContentDigest::sha256(run_record_digest(input.run_record)?),
            output_artifact_manifest_hash: LegalContentDigest::sha256(
                input.run_record.output_artifact_manifest_hash.clone(),
            ),
            artifact_paths_and_hashes,
            wall_clock_timings: LegalWallClockTimings {
                wall_time_ms: input.run_record.metrics.wall_time_ms,
                model_turns: input.run_record.metrics.model_turns,
                tool_call_count: input.run_record.metrics.tool_call_count,
            },
            metadata: input.metadata,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_schema_version("run_receipt.schema_version", self.schema_version)?;
        require_non_empty("run_receipt.receipt_id", self.receipt_id.as_str())?;
        require_rfc3339_utc(
            "run_receipt.created_at_rfc3339",
            self.created_at_rfc3339.as_str(),
        )?;
        self.run_spec.validate()?;
        if self.answer_files.is_empty() {
            return Err(LegalSchemaError::Validation {
                field: "run_receipt.answer_files",
                detail: String::from("at least one answer file is required"),
            });
        }
        for answer_file in &self.answer_files {
            answer_file.validate()?;
        }
        self.score.validate()?;
        self.integrity.validate()?;
        self.run_record_hash
            .validate("run_receipt.run_record_hash")?;
        self.output_artifact_manifest_hash
            .validate("run_receipt.output_artifact_manifest_hash")?;
        if self.artifact_paths_and_hashes.is_empty() {
            return Err(LegalSchemaError::Validation {
                field: "run_receipt.artifact_paths_and_hashes",
                detail: String::from("artifact refs must not be empty"),
            });
        }
        for artifact in &self.artifact_paths_and_hashes {
            artifact.validate()?;
        }
        Ok(())
    }
}

pub struct LegalRunReceiptFromExisting<'a> {
    pub benchmark_id: &'a str,
    pub benchmark_visibility: LegalBenchmarkVisibility,
    pub base_model_id: &'a str,
    pub adapter_id: Option<&'a str>,
    pub tokenizer_id: &'a str,
    pub tokenizer_hash: &'a str,
    pub prompt_template_id: &'a str,
    pub prompt_template_hash: &'a str,
    pub inference_settings: Metadata,
    pub thinking_mode: LegalThinkingMode,
    pub tool_list: Vec<String>,
    pub source_documents: Vec<LegalSourceDocument>,
    pub run_record: &'a RunRecord,
    pub output_manifest: &'a ArtifactManifest,
    pub score_report: &'a ScoreReport,
    pub answer_integrity: &'a LegalBenchmarkAnswerIntegrityReport,
    pub git_commit: &'a str,
    pub git_dirty: bool,
    pub deterministic_replay_command: Vec<String>,
    pub worker_id: Option<&'a str>,
    pub hardware_summary: Option<Metadata>,
    pub created_at_rfc3339: &'a str,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalArtifactRef {
    pub relative_path: String,
    pub content_hash: LegalContentDigest,
}

impl LegalArtifactRef {
    pub fn validate(&self) -> Result<(), LegalSchemaError> {
        require_non_empty("artifact.relative_path", self.relative_path.as_str())?;
        self.content_hash.validate("artifact.content_hash")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalWallClockTimings {
    pub wall_time_ms: u64,
    pub model_turns: u32,
    pub tool_call_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalTrainingExample {
    pub schema_version: u16,
    pub example_id: String,
    pub run_receipt_hash: LegalContentDigest,
    pub prompt_hash: LegalContentDigest,
    pub answer_hashes: Vec<LegalContentDigest>,
    pub score_bps: u32,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBadRunExample {
    pub schema_version: u16,
    pub example_id: String,
    pub run_receipt_hash: LegalContentDigest,
    pub full_prompt: String,
    pub full_model_response: String,
    pub tool_call_transcript: Vec<LegalToolCall>,
    pub attempted_file_writes: Vec<LegalAttemptedFileWrite>,
    pub required_file_paths: Vec<String>,
    pub required_files: Vec<LegalRequiredFileStatus>,
    pub answer_files: Vec<LegalCapturedAnswerFile>,
    pub action_sequence: Vec<String>,
    pub stop_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scorer_feedback: Option<String>,
    pub integrity: LegalIntegrityReceipt,
    pub failure_class: LegalFailureClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_correction: Option<String>,
    pub training_eligible: bool,
    pub sft_eligible: bool,
    pub training_eligibility_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_malformed_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalAttemptedFileWrite {
    pub relative_path: String,
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<LegalContentDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_len: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRequiredFileStatus {
    pub relative_path: String,
    pub existed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<LegalContentDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_len: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalCapturedAnswerFile {
    pub relative_path: String,
    pub content: String,
    pub content_hash: LegalContentDigest,
    pub byte_len: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalPreferencePair {
    pub schema_version: u16,
    pub pair_id: String,
    pub chosen_example_id: String,
    pub rejected_example_id: String,
    pub label_source: String,
    pub preference_reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalRewardTrace {
    pub schema_version: u16,
    pub reward_trace_id: String,
    pub run_receipt_hash: LegalContentDigest,
    pub total_reward_bps: i32,
    pub components: BTreeMap<String, i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalDatasetManifest {
    pub schema_version: u16,
    pub dataset_id: String,
    pub examples: Vec<LegalTrainingExample>,
    pub bad_runs: Vec<LegalBadRunExample>,
    pub preference_pairs: Vec<LegalPreferencePair>,
    pub reward_traces: Vec<LegalRewardTrace>,
    pub content_hashes: Vec<LegalContentDigest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalAdapterManifest {
    pub schema_version: u16,
    pub adapter_id: String,
    pub base_model_id: String,
    pub tokenizer_id: String,
    pub tokenizer_hash: LegalContentDigest,
    pub dataset_manifest_hash: LegalContentDigest,
    pub adapter_hash: LegalContentDigest,
    pub adapter_format: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalModelCandidate {
    pub schema_version: u16,
    pub candidate_id: String,
    pub base_model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    pub run_receipt_hashes: Vec<LegalContentDigest>,
    pub score_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalPromotionDecision {
    pub schema_version: u16,
    pub decision_id: String,
    pub candidate_id: String,
    pub decision: LegalPromotionDecisionKind,
    pub reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_candidate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_receipt_hash: Option<LegalContentDigest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingJob {
    pub schema_version: u16,
    pub job_id: String,
    pub base_model_id: String,
    pub adapter_target_id: String,
    pub dataset_manifest_hash: LegalContentDigest,
    pub training_config_hash: LegalContentDigest,
    pub worker_count: u32,
    pub output_adapter_manifest_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PylonWorkerReceipt {
    pub schema_version: u16,
    pub worker_id: String,
    pub job_id: String,
    pub hardware_summary: Metadata,
    pub samples_seen: u64,
    pub checkpoint_hashes: Vec<LegalContentDigest>,
    pub started_at_rfc3339: String,
    pub finished_at_rfc3339: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicTrainingConfig {
    pub schema_version: u16,
    pub config_id: String,
    pub base_model_id: String,
    pub tokenizer_id: String,
    pub method: String,
    pub optimizer: String,
    pub max_steps: u64,
    pub random_seed: u64,
    pub hyperparameters: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicTrainingReceipt {
    pub schema_version: u16,
    pub receipt_id: String,
    pub job_id: String,
    pub training_config_hash: LegalContentDigest,
    pub dataset_manifest_hash: LegalContentDigest,
    pub adapter_manifest_hash: LegalContentDigest,
    pub worker_receipts: Vec<PylonWorkerReceipt>,
    pub metrics: Metadata,
    pub started_at_rfc3339: String,
    pub finished_at_rfc3339: String,
}

pub fn legal_run_receipt_digest(receipt: &LegalRunReceipt) -> Result<String, LegalSchemaError> {
    Ok(stable_json_digest(
        "psionic.legal_benchmark.canonical.run_receipt.v1",
        receipt,
    )?)
}

pub fn legal_run_receipt_canonical_json(
    receipt: &LegalRunReceipt,
) -> Result<Vec<u8>, LegalSchemaError> {
    receipt.validate()?;
    Ok(serde_json::to_vec(receipt)?)
}

#[derive(Debug, Error)]
pub enum LegalSchemaError {
    #[error("schema validation failed at {field}: {detail}")]
    Validation { field: &'static str, detail: String },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

fn require_schema_version(field: &'static str, version: u16) -> Result<(), LegalSchemaError> {
    if version == LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(LegalSchemaError::Validation {
            field,
            detail: format!(
                "expected schema version {LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION}, got {version}"
            ),
        })
    }
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), LegalSchemaError> {
    if value.trim().is_empty() {
        Err(LegalSchemaError::Validation {
            field,
            detail: String::from("value must not be empty"),
        })
    } else {
        Ok(())
    }
}

fn require_rfc3339_utc(field: &'static str, value: &str) -> Result<(), LegalSchemaError> {
    require_non_empty(field, value)?;
    if value.ends_with('Z') && value.contains('T') {
        Ok(())
    } else {
        Err(LegalSchemaError::Validation {
            field,
            detail: String::from(
                "timestamp must use RFC3339 UTC form, for example 2026-05-20T00:00:00Z",
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, ArtifactManifestRole, BenchmarkTaskSpec, CriterionResult, CriterionSpec,
        CriterionVerdict, DataClassification, DeliverableKind, DeliverableSpec, JudgeMode,
        JudgePolicy, RunMetrics, RunTerminalState, ToolPolicy, TranscriptEvent,
        TranscriptEventKind, artifact_from_file, build_output_artifact_manifest,
    };
    use std::fs;

    fn source_hash() -> LegalContentDigest {
        LegalContentDigest::sha256(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
    }

    fn answer_hash() -> LegalContentDigest {
        LegalContentDigest::sha256(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
    }

    fn minimal_receipt() -> LegalRunReceipt {
        LegalRunReceipt {
            schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
            receipt_id: String::from("legal.run_receipt.run.schema"),
            created_at_rfc3339: String::from("2026-05-20T00:00:00Z"),
            run_spec: LegalRunSpec {
                schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                run_id: String::from("run.schema"),
                task_id: String::from("task.schema"),
                task_version: String::from("v1"),
                benchmark_id: String::from("harvey"),
                benchmark_visibility: LegalBenchmarkVisibility::Public,
                base_model_id: String::from("qwen3.5-4b"),
                adapter_id: Some(String::from("adapter.legal.001")),
                tokenizer_id: String::from("qwen-tokenizer"),
                tokenizer_hash: source_hash(),
                prompt_template_id: String::from("legal.autopilot.v1"),
                prompt_template_hash: source_hash(),
                inference_settings: Metadata::new(),
                thinking_mode: LegalThinkingMode::Disabled,
                tool_list: vec![String::from("write")],
                source_document_hashes: vec![source_hash()],
                git_commit: String::from("abcdef1"),
                git_dirty: false,
                deterministic_replay_command: vec![
                    String::from("cargo"),
                    String::from("run"),
                    String::from("-p"),
                    String::from("psionic-eval"),
                ],
                worker_id: Some(String::from("worker.local")),
                hardware_summary: None,
            },
            terminal_state: String::from("submitted"),
            model_output_transcript: Vec::new(),
            tool_calls: Vec::new(),
            answer_files: vec![LegalAnswerFile {
                schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                relative_path: String::from("memo.md"),
                byte_size: 12,
                content_hash: answer_hash(),
                pre_score_hash: Some(answer_hash()),
                post_score_hash: Some(answer_hash()),
                creation_actor: RunActor::Model,
                last_modifying_actor: RunActor::Model,
                writer_tool_call_id: Some(String::from("call.write.memo")),
                integrity_valid: true,
            }],
            score: LegalScoreReceipt {
                schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                score_report_id: String::from("score.run.schema"),
                scorer_version: String::from("psionic-eval.legal_benchmark_evaluator.v1"),
                score_report_hash: source_hash(),
                all_pass: true,
                criterion_pass_rate_bps: 10_000,
                document_coverage_bps: 10_000,
                criterion_count: 1,
                passed_criterion_count: 1,
                failure_diagnostics: Vec::new(),
            },
            integrity: LegalIntegrityReceipt {
                schema_version: LEGAL_BENCHMARK_CANONICAL_SCHEMA_VERSION,
                valid: true,
                integrity_report_hash: source_hash(),
                invalid_reasons: Vec::new(),
            },
            run_record_hash: source_hash(),
            output_artifact_manifest_hash: source_hash(),
            artifact_paths_and_hashes: vec![LegalArtifactRef {
                relative_path: String::from("memo.md"),
                content_hash: answer_hash(),
            }],
            wall_clock_timings: LegalWallClockTimings {
                wall_time_ms: 100,
                model_turns: 1,
                tool_call_count: 1,
            },
            metadata: Metadata::new(),
        }
    }

    #[test]
    fn legal_benchmark_schema_validates_new_run_receipt() {
        let receipt = minimal_receipt();
        receipt.validate().expect("valid receipt");
        let digest = legal_run_receipt_digest(&receipt).expect("digest");
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn legal_benchmark_schema_missing_visibility_fails() {
        let mut value = serde_json::to_value(minimal_receipt()).expect("json");
        value
            .get_mut("run_spec")
            .and_then(Value::as_object_mut)
            .expect("run_spec")
            .remove("benchmark_visibility");
        let error = serde_json::from_value::<LegalRunReceipt>(value).expect_err("missing field");
        assert!(error.to_string().contains("benchmark_visibility"));
    }

    #[test]
    fn legal_benchmark_schema_missing_answer_hash_fails() {
        let mut value = serde_json::to_value(minimal_receipt()).expect("json");
        value
            .get_mut("answer_files")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.first_mut())
            .and_then(Value::as_object_mut)
            .expect("answer file")
            .remove("content_hash");
        let error = serde_json::from_value::<LegalRunReceipt>(value).expect_err("missing field");
        assert!(error.to_string().contains("content_hash"));
    }

    #[test]
    fn legal_benchmark_schema_missing_scorer_version_fails() {
        let mut value = serde_json::to_value(minimal_receipt()).expect("json");
        value
            .get_mut("score")
            .and_then(Value::as_object_mut)
            .expect("score")
            .remove("scorer_version");
        let error = serde_json::from_value::<LegalRunReceipt>(value).expect_err("missing field");
        assert!(error.to_string().contains("scorer_version"));
    }

    #[test]
    fn legal_benchmark_schema_serialization_is_deterministic() {
        let receipt = minimal_receipt();
        let first = legal_run_receipt_canonical_json(&receipt).expect("first");
        let second = legal_run_receipt_canonical_json(&receipt).expect("second");
        assert_eq!(first, second);
        assert_eq!(
            legal_run_receipt_digest(&receipt).expect("digest one"),
            legal_run_receipt_digest(&receipt).expect("digest two")
        );
    }

    #[test]
    fn legal_benchmark_schema_wraps_existing_run_records() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("output");
        fs::create_dir_all(&output_root).expect("output dir");
        let content = "# Memo\n\nModel answer.\n";
        fs::write(output_root.join("memo.md"), content).expect("memo");
        let artifact = artifact_from_file(
            "artifact.output.memo",
            ArtifactKind::GeneratedDeliverable,
            &output_root,
            output_root.join("memo.md"),
            DataClassification::BenchmarkConfidential,
            Some(String::from("model")),
        )
        .expect("artifact");
        let output_manifest = build_output_artifact_manifest(
            "legal.schema.task",
            "v1",
            "run.schema.wrap",
            vec![artifact],
        );
        assert_eq!(output_manifest.manifest_role, ArtifactManifestRole::Output);
        let task = BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.schema.task"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("review"),
            title: String::from("Schema task"),
            instructions: String::from("Write memo."),
            work_type: String::from("review"),
            tags: Vec::new(),
            source_artifacts: Vec::new(),
            deliverables: vec![DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Memo"),
                required: true,
            }],
            criteria: vec![CriterionSpec {
                criterion_id: String::from("criterion.memo"),
                criterion_kind: crate::CriterionKind::DeliverableValidation,
                description: String::from("Memo exists"),
                weight_bps: Some(10_000),
                deliverable_ids: vec![String::from("memo")],
                source_artifact_ids: Vec::new(),
            }],
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
                max_turns: 1,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: Metadata::new(),
        };
        let run_record = RunRecord {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_id: String::from("run.schema.wrap"),
            task_id: task.task_id.clone(),
            task_version: task.task_version.clone(),
            input_artifact_manifest_hash: source_hash().value,
            run_config_hash: source_hash().value,
            output_artifact_manifest_hash: crate::artifact_manifest_digest(&output_manifest)
                .expect("manifest hash"),
            terminal_state: RunTerminalState::Submitted,
            transcript: vec![TranscriptEvent {
                event_index: 0,
                event_kind: TranscriptEventKind::Assistant,
                role: Some(String::from("assistant")),
                content: Some(String::from("submitted")),
                payload: None,
                timestamp_ms: 1,
            }],
            tool_calls: Vec::new(),
            metrics: RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 1,
                output_tokens: 1,
                wall_time_ms: 10,
                estimated_cost_micro_usd: 1,
            },
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            metadata: Metadata::new(),
        };
        let score_report = ScoreReport {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            score_report_id: String::from("score.run.schema.wrap"),
            run_id: run_record.run_id.clone(),
            task_id: run_record.task_id.clone(),
            task_version: run_record.task_version.clone(),
            run_record_hash: crate::run_record_digest(&run_record).expect("run hash"),
            output_artifact_manifest_hash: run_record.output_artifact_manifest_hash.clone(),
            all_pass: true,
            criterion_pass_rate_bps: 10_000,
            criterion_results: vec![CriterionResult {
                criterion_id: String::from("criterion.memo"),
                passed: true,
                verdict: CriterionVerdict::Pass,
                reasoning: String::from("present"),
                evidence_refs: vec![String::from("memo.md")],
                judge_model: String::from("mock"),
                judge_prompt_hash: String::from("prompt-hash"),
                raw_response_hash: String::from("response-hash"),
                confidence_bps: None,
                judge_latency_ms: Some(0),
                judge_cost_micro_usd: Some(0),
            }],
            metrics: run_record.metrics.clone(),
            document_coverage_bps: 10_000,
            failure_diagnostics: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            failure_comparisons: Vec::new(),
            metadata: Metadata::new(),
        };
        let mut answer_integrity = LegalBenchmarkAnswerIntegrityReport::default();
        answer_integrity.valid = true;
        answer_integrity.report_hash = source_hash().value;
        answer_integrity.answer_files = vec![crate::AnswerFileIntegrityReceipt {
            relative_path: String::from("memo.md"),
            required_by_task: true,
            declared_in_manifest: true,
            exists: true,
            creation_actor: RunActor::Model,
            last_modifying_actor: RunActor::Model,
            writer_tool_call_id: Some(String::from("call.write.memo")),
            pre_score_hash: Some(output_manifest.artifacts[0].sha256.clone()),
            post_score_hash: Some(output_manifest.artifacts[0].sha256.clone()),
            byte_size: Some(output_manifest.artifacts[0].byte_size),
            mtime_ms: Some(1),
            valid: true,
            failure_reasons: Vec::new(),
        }];
        answer_integrity.invalid_reasons = Vec::new();

        let receipt = LegalRunReceipt::from_existing_run(LegalRunReceiptFromExisting {
            benchmark_id: "harvey",
            benchmark_visibility: LegalBenchmarkVisibility::Public,
            base_model_id: "qwen3.5-4b",
            adapter_id: None,
            tokenizer_id: "qwen-tokenizer",
            tokenizer_hash: source_hash().value.as_str(),
            prompt_template_id: "legal.autopilot.v1",
            prompt_template_hash: source_hash().value.as_str(),
            inference_settings: Metadata::new(),
            thinking_mode: LegalThinkingMode::Disabled,
            tool_list: vec![String::from("write")],
            source_documents: Vec::new(),
            run_record: &run_record,
            output_manifest: &output_manifest,
            score_report: &score_report,
            answer_integrity: &answer_integrity,
            git_commit: "abcdef1",
            git_dirty: false,
            deterministic_replay_command: vec![String::from("cargo"), String::from("test")],
            worker_id: Some("worker.local"),
            hardware_summary: None,
            created_at_rfc3339: "2026-05-20T00:00:00Z",
            metadata: Metadata::new(),
        })
        .expect("receipt");

        assert_eq!(receipt.run_spec.benchmark_id, "harvey");
        assert_eq!(receipt.answer_files[0].creation_actor, RunActor::Model);
        receipt.validate().expect("valid wrapped receipt");
    }
}
