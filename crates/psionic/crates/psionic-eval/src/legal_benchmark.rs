//! Legal-agent benchmark schema contracts.
//!
//! These types are the Psionic-owned task, artifact, run, transcript, and
//! scoring contracts for Harvey-compatible legal-agent benchmark work. The
//! structs intentionally keep upstream compatibility fields separate from the
//! immutable hashes and receipts Psionic and Autopilot consume.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current schema version for the owned legal benchmark contracts.
pub const LEGAL_BENCHMARK_SCHEMA_VERSION: u16 = 1;

/// Stable metadata map used across benchmark contracts.
pub type Metadata = BTreeMap<String, Value>;

/// Source compatibility information for a task imported from another suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCompatibility {
    /// Upstream suite name, for example `harvey_labs`.
    pub upstream_suite: String,
    /// Upstream repository commit or immutable release id.
    pub upstream_commit: String,
    /// Upstream task path relative to the suite root.
    pub upstream_task_path: String,
    /// Raw upstream ids or labels that should remain non-authoritative.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub upstream_fields: Metadata,
}

/// Benchmark task specification normalized into Psionic-owned shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkTaskSpec {
    /// Schema version for this task spec.
    pub schema_version: u16,
    /// Stable task id in the owned benchmark namespace.
    pub task_id: String,
    /// Immutable task version.
    pub task_version: String,
    /// Domain or corpus family, for example `legal`.
    pub domain: String,
    /// Practice area or benchmark lane.
    pub practice_area: String,
    /// Workflow class such as analyze, draft, review, or research.
    pub workflow: String,
    /// Human-readable title.
    pub title: String,
    /// Task instructions visible to the agent.
    pub instructions: String,
    /// Work type or original benchmark category.
    pub work_type: String,
    /// Search and reporting tags.
    pub tags: Vec<String>,
    /// Source documents and other immutable inputs.
    pub source_artifacts: Vec<SourceArtifact>,
    /// Required deliverables.
    pub deliverables: Vec<DeliverableSpec>,
    /// Scoring criteria.
    pub criteria: Vec<CriterionSpec>,
    /// Judge policy attached to this task.
    pub judge_policy: JudgePolicy,
    /// Tool policy attached to this task.
    pub tool_policy: ToolPolicy,
    /// Compatibility information when imported from an upstream suite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_compatibility: Option<SourceCompatibility>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Immutable manifest over source or output artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    /// Schema version for this artifact manifest.
    pub schema_version: u16,
    /// Stable manifest id.
    pub manifest_id: String,
    /// Task id this manifest belongs to.
    pub task_id: String,
    /// Task version this manifest belongs to.
    pub task_version: String,
    /// Whether this manifest describes inputs, outputs, or derived artifacts.
    pub manifest_role: ArtifactManifestRole,
    /// Artifacts included in the manifest.
    pub artifacts: Vec<SourceArtifact>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Comparison between two artifact manifests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactManifestComparison {
    /// Stable digest of the old manifest.
    pub old_manifest_hash: String,
    /// Stable digest of the new manifest.
    pub new_manifest_hash: String,
    /// Artifact ids that exist only in the new manifest.
    pub added_artifact_ids: Vec<String>,
    /// Artifact ids that exist only in the old manifest.
    pub removed_artifact_ids: Vec<String>,
    /// Artifact ids whose path, size, media type, classification, or digest changed.
    pub changed_artifact_ids: Vec<String>,
    /// Artifact ids that are unchanged.
    pub unchanged_artifact_ids: Vec<String>,
}

impl ArtifactManifestComparison {
    /// Returns true when the manifests have no effective artifact changes.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.added_artifact_ids.is_empty()
            && self.removed_artifact_ids.is_empty()
            && self.changed_artifact_ids.is_empty()
    }
}

/// Errors raised while creating file-backed artifacts or manifest comparisons.
#[derive(Debug, Error)]
pub enum ArtifactManifestError {
    /// File-system operation failed.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Relative path conversion failed.
    #[error("failed to compute relative path for {path} from root {root}")]
    RelativePath {
        /// Root path.
        root: PathBuf,
        /// Child path.
        path: PathBuf,
    },
    /// JSON digest calculation failed.
    #[error("failed to calculate manifest digest: {0}")]
    Digest(#[from] serde_json::Error),
}

/// Artifact manifest role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactManifestRole {
    /// Agent-visible input artifacts.
    Input,
    /// Agent-generated output artifacts.
    Output,
    /// Derived extraction or intermediate artifacts.
    Derived,
}

/// One source or generated artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtifact {
    /// Stable artifact id within a task or run.
    pub artifact_id: String,
    /// Artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Path relative to the task, run, or manifest root.
    pub relative_path: String,
    /// Original file name if different from the relative path leaf.
    pub original_filename: String,
    /// Media type such as `application/pdf`.
    pub media_type: String,
    /// Byte length.
    pub byte_size: u64,
    /// SHA-256 digest of the artifact bytes.
    pub sha256: String,
    /// Data classification for execution and reporting.
    pub data_classification: DataClassification,
    /// Optional provenance string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

/// Artifact kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Original benchmark source document.
    SourceDocument,
    /// Extracted text or structured representation.
    ExtractedText,
    /// Agent-generated deliverable.
    GeneratedDeliverable,
    /// Agent transcript.
    Transcript,
    /// Score report.
    ScoreReport,
    /// Comparison report.
    ComparisonReport,
    /// Scratch or intermediate artifact.
    Scratch,
}

/// Data classification for benchmark artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    /// Public reference corpus data.
    PublicReference,
    /// Benchmark-confidential data.
    BenchmarkConfidential,
    /// User-confidential data.
    UserConfidential,
    /// Restricted data.
    Restricted,
    /// Unknown classification.
    Unknown,
}

/// Required deliverable specification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverableSpec {
    /// Stable deliverable id.
    pub deliverable_id: String,
    /// Deliverable kind.
    pub deliverable_kind: DeliverableKind,
    /// Required relative output path or path pattern.
    pub required_path: String,
    /// Human-readable description.
    pub description: String,
    /// Whether the deliverable must exist for scoring.
    pub required: bool,
}

/// Deliverable kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverableKind {
    /// Plain text deliverable.
    Text,
    /// Markdown deliverable.
    Markdown,
    /// Word-processing document.
    Docx,
    /// Spreadsheet.
    Xlsx,
    /// PDF deliverable.
    Pdf,
    /// JSON deliverable.
    Json,
    /// Directory of output files.
    Directory,
    /// Other deliverable type.
    Other,
}

/// One scoring criterion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionSpec {
    /// Stable criterion id.
    pub criterion_id: String,
    /// Criterion kind.
    pub criterion_kind: CriterionKind,
    /// Criterion text.
    pub description: String,
    /// Weight in basis points, if the suite uses weighted scoring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_bps: Option<u32>,
    /// Deliverables this criterion applies to.
    pub deliverable_ids: Vec<String>,
    /// Source artifacts this criterion expects to be considered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_artifact_ids: Vec<String>,
}

/// Criterion family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionKind {
    /// Completeness requirement.
    Completeness,
    /// Legal reasoning requirement.
    LegalReasoning,
    /// Factual accuracy requirement.
    FactualAccuracy,
    /// Citation or evidence requirement.
    CitationEvidence,
    /// Output formatting requirement.
    Formatting,
    /// Deliverable presence or readability requirement.
    DeliverableValidation,
    /// Other criterion kind.
    Other,
}

/// Judge policy for scoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgePolicy {
    /// Judge mode.
    pub mode: JudgeMode,
    /// Provider or local judge route.
    pub provider: String,
    /// Judge model id.
    pub model: String,
    /// Stable judge prompt template id.
    pub prompt_template_id: String,
    /// Stable hash of the prompt template.
    pub prompt_template_hash: String,
    /// Whether all criteria must pass for task success.
    pub all_pass_required: bool,
    /// Number of judge samples.
    pub sample_count: u16,
}

/// Judge mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeMode {
    /// Deterministic evaluator only.
    Deterministic,
    /// Single LLM judge.
    Llm,
    /// Multiple LLM judges.
    MultiJudge,
    /// Human reviewer.
    Human,
}

/// Tool policy for agent execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Allowed benchmark tools.
    pub allowed_tools: Vec<String>,
    /// Whether network access is allowed.
    pub network_allowed: bool,
    /// Whether source artifacts must be mounted read-only.
    pub source_artifacts_read_only: bool,
    /// Maximum model-tool turns.
    pub max_turns: u32,
    /// Maximum wall time in seconds.
    pub max_wall_time_seconds: u64,
}

/// Run configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunConfig {
    /// Schema version for this run config.
    pub schema_version: u16,
    /// Stable run config id.
    pub run_config_id: String,
    /// Provider route.
    pub provider: String,
    /// Model id.
    pub model: String,
    /// Agent protocol or module version id.
    pub agent_protocol_version: String,
    /// Tool policy for this run.
    pub tool_policy: ToolPolicy,
    /// Judge policy for this run.
    pub judge_policy: JudgePolicy,
    /// Optional random seed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random_seed: Option<u64>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Complete record for one benchmark run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Schema version for this run record.
    pub schema_version: u16,
    /// Stable run id.
    pub run_id: String,
    /// Task id.
    pub task_id: String,
    /// Task version.
    pub task_version: String,
    /// SHA-256 hash of the input artifact manifest.
    pub input_artifact_manifest_hash: String,
    /// SHA-256 hash of the run config.
    pub run_config_hash: String,
    /// SHA-256 hash of the output artifact manifest.
    pub output_artifact_manifest_hash: String,
    /// Terminal state for the run.
    pub terminal_state: RunTerminalState,
    /// Ordered transcript events.
    pub transcript: Vec<TranscriptEvent>,
    /// Tool calls extracted from the transcript.
    pub tool_calls: Vec<ToolCallRecord>,
    /// Aggregate run metrics.
    pub metrics: RunMetrics,
    /// Extraction receipt ids or hashes retained by this run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_receipt_refs: Vec<String>,
    /// Criterion-adjacent coverage state captured during the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_snapshot: Option<CoverageSnapshot>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Run terminal state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTerminalState {
    /// Agent submitted outputs.
    Submitted,
    /// Agent stopped without tool calls.
    NoToolCalls,
    /// Maximum turn count reached.
    MaxTurns,
    /// Maximum model token budget reached.
    MaxTokens,
    /// Context limit reached.
    ContextOverflow,
    /// Provider failure ended the run.
    ProviderFailure,
    /// Sandbox failure ended the run.
    SandboxFailure,
    /// Policy failure ended the run.
    PolicyFailure,
    /// Internal runner error ended the run.
    InternalError,
}

/// One transcript event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    /// Monotonic event index.
    pub event_index: u64,
    /// Event kind.
    pub event_kind: TranscriptEventKind,
    /// Optional role string such as `system`, `user`, `assistant`, or `tool`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Text content when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Structured JSON payload when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// Timestamp in milliseconds from Unix epoch.
    pub timestamp_ms: u64,
}

/// Transcript event kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptEventKind {
    /// System prompt or policy event.
    System,
    /// User task message.
    User,
    /// Model assistant message.
    Assistant,
    /// Tool call emitted by the model.
    ToolCall,
    /// Tool result returned to the model.
    ToolResult,
    /// Runner event.
    Runner,
    /// Judge event.
    Judge,
}

/// One tool call record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Event index of the call.
    pub call_event_index: u64,
    /// Event index of the result, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_event_index: Option<u64>,
    /// Tool input payload.
    pub input: Value,
    /// Tool output payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    /// Structured error kind when the call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
}

/// Aggregate run metrics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Number of model turns.
    pub model_turns: u32,
    /// Number of tool calls.
    pub tool_call_count: u32,
    /// Prompt/input tokens.
    pub input_tokens: u64,
    /// Completion/output tokens.
    pub output_tokens: u64,
    /// Total wall time in milliseconds.
    pub wall_time_ms: u64,
    /// Estimated cost in microdollars.
    pub estimated_cost_micro_usd: u64,
}

/// Policy mode for agent-visible coverage guidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageMode {
    /// No rubric-derived hints are visible to the running agent.
    Integrity,
    /// Policy-approved derived checklist items may be visible for training.
    HillClimb,
}

/// One policy-approved derived checklist item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedChecklistItem {
    /// Stable checklist item id.
    pub item_id: String,
    /// Agent-visible checklist text.
    pub prompt: String,
    /// Policy or artifact source for this derived item.
    pub source: String,
    /// Whether the item is allowed in model-visible training prompts.
    pub agent_visible: bool,
}

/// Checklist payload that can be safely rendered to the running agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentVisibleChecklist {
    /// Coverage mode used to construct the checklist.
    pub mode: CoverageMode,
    /// Policy-approved items visible to the agent.
    pub items: Vec<DerivedChecklistItem>,
    /// Whether hidden rubric criteria were exposed.
    pub hidden_criteria_visible: bool,
}

/// Per-document coverage state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentCoverageEntry {
    /// Source artifact id when known.
    pub artifact_id: String,
    /// Relative document path.
    pub relative_path: String,
    /// Whether the agent discovered the document.
    pub discovered: bool,
    /// Whether the agent read the document.
    pub read: bool,
    /// Whether extracted text was used instead of raw bytes.
    pub used_extracted_text: bool,
}

/// One extracted fact or candidate fact trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactCoverageEntry {
    /// Stable fact id.
    pub fact_id: String,
    /// Source artifact id or relative path.
    pub source_ref: String,
    /// Hash of the extracted text span or summary.
    pub text_hash: String,
    /// Extraction mechanism.
    pub method: String,
}

/// One evidence reference captured by a tool or self-check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCoverageEntry {
    /// Stable evidence id.
    pub evidence_id: String,
    /// Source artifact id or relative path.
    pub source_ref: String,
    /// Optional line number or page label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// Hash of the evidence span.
    pub span_hash: String,
}

/// Deliverable drafting coverage state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverableSectionCoverage {
    /// Deliverable id when matched to the task spec.
    pub deliverable_id: String,
    /// Output or workspace path touched by the agent.
    pub relative_path: String,
    /// Section or draft id.
    pub section_id: String,
    /// Whether the section was drafted or edited.
    pub drafted: bool,
}

/// Validation coverage state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCoverageEntry {
    /// Stable validation id.
    pub validation_id: String,
    /// Validation target such as a deliverable id or path.
    pub target_ref: String,
    /// Whether validation passed.
    pub passed: bool,
    /// Human-readable validation summary.
    pub detail: String,
}

/// Self-check coverage state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfCheckCoverageEntry {
    /// Stable self-check id.
    pub self_check_id: String,
    /// Area being checked.
    pub area: String,
    /// Self-check outcome.
    pub outcome: String,
}

/// Criterion-adjacent coverage snapshot captured without exposing hidden rubric
/// criteria to the model in integrity mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSnapshot {
    /// Schema version for coverage snapshots.
    pub schema_version: u16,
    /// Policy mode used for the run.
    pub mode: CoverageMode,
    /// Whether hidden criteria were included in model-visible messages.
    pub hidden_criteria_visible: bool,
    /// Agent-visible derived checklist items.
    pub derived_checklist_items: Vec<DerivedChecklistItem>,
    /// Documents discovered or read by the agent.
    pub documents: Vec<DocumentCoverageEntry>,
    /// Facts extracted from source documents.
    pub facts: Vec<FactCoverageEntry>,
    /// Evidence spans captured during the run.
    pub evidence_refs: Vec<EvidenceCoverageEntry>,
    /// Deliverable sections drafted or edited.
    pub deliverable_sections: Vec<DeliverableSectionCoverage>,
    /// Validations run before scoring.
    pub validations: Vec<ValidationCoverageEntry>,
    /// Agent self-check outcomes.
    pub self_checks: Vec<SelfCheckCoverageEntry>,
}

/// Failure class after missed criteria are compared to coverage state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionFailureClass {
    /// The agent did not discover or read required source material.
    CoverageGap,
    /// The agent read source material but did not extract usable facts/evidence.
    ExtractionGap,
    /// The agent extracted relevant material but did not draft the target output.
    DraftingGap,
    /// The agent had apparent coverage but reasoned or applied law incorrectly.
    ReasoningGap,
    /// The criterion was not a failure.
    Passed,
}

/// Post-judge comparison between a missed criterion and run coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionCoverageFailure {
    /// Criterion id.
    pub criterion_id: String,
    /// Failure classification.
    pub failure_class: CriterionFailureClass,
    /// Source artifacts that were expected but not read.
    pub missing_source_artifact_ids: Vec<String>,
    /// Deliverables that were expected but not drafted.
    pub missing_deliverable_ids: Vec<String>,
    /// Evidence ids considered for this comparison.
    pub evidence_refs: Vec<String>,
    /// Human-readable diagnostic.
    pub diagnostic: String,
}

/// Result for one criterion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CriterionResult {
    /// Criterion id.
    pub criterion_id: String,
    /// Whether the criterion passed.
    pub passed: bool,
    /// Verdict family.
    pub verdict: CriterionVerdict,
    /// Judge or deterministic reasoning summary.
    pub reasoning: String,
    /// Referenced output or source evidence ids.
    pub evidence_refs: Vec<String>,
    /// Judge model used for this result.
    pub judge_model: String,
    /// Judge prompt hash.
    pub judge_prompt_hash: String,
    /// Raw judge response hash.
    pub raw_response_hash: String,
    /// Optional judge confidence in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_bps: Option<u16>,
    /// Judge latency for this criterion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge_latency_ms: Option<u64>,
    /// Estimated criterion judging cost in microdollars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge_cost_micro_usd: Option<u64>,
}

/// Criterion verdict family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionVerdict {
    /// Criterion passed.
    Pass,
    /// Criterion failed.
    Fail,
    /// Judge result was ambiguous.
    Ambiguous,
    /// Criterion was not evaluated.
    NotEvaluated,
}

/// Score report for one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreReport {
    /// Schema version for this score report.
    pub schema_version: u16,
    /// Stable score report id.
    pub score_report_id: String,
    /// Run id.
    pub run_id: String,
    /// Task id.
    pub task_id: String,
    /// Task version.
    pub task_version: String,
    /// Run record hash.
    pub run_record_hash: String,
    /// Output artifact manifest hash.
    pub output_artifact_manifest_hash: String,
    /// Whether every criterion passed.
    pub all_pass: bool,
    /// Criterion pass rate in basis points.
    pub criterion_pass_rate_bps: u32,
    /// Criterion results.
    pub criterion_results: Vec<CriterionResult>,
    /// Run metrics copied into the report.
    pub metrics: RunMetrics,
    /// Output/source document coverage in basis points.
    #[serde(default)]
    pub document_coverage_bps: u32,
    /// Human-readable failure diagnostics retained for reporting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_diagnostics: Vec<String>,
    /// Extraction receipt ids or hashes considered by the scorer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_receipt_refs: Vec<String>,
    /// Coverage snapshot considered by the scorer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_snapshot: Option<CoverageSnapshot>,
    /// Post-judge failure classifications derived from coverage state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_comparisons: Vec<CriterionCoverageFailure>,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Comparison between two or more score reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComparisonReport {
    /// Schema version for this comparison report.
    pub schema_version: u16,
    /// Stable comparison report id.
    pub comparison_report_id: String,
    /// Suite or campaign id.
    pub comparison_scope: String,
    /// Baseline score report hash.
    pub baseline_score_report_hash: String,
    /// Candidate score report hash.
    pub candidate_score_report_hash: String,
    /// All-pass delta in basis points.
    pub all_pass_delta_bps: i32,
    /// Criterion pass-rate delta in basis points.
    pub criterion_pass_rate_delta_bps: i32,
    /// Cost delta in microdollars.
    pub estimated_cost_delta_micro_usd: i64,
    /// Latency delta in milliseconds.
    pub wall_time_delta_ms: i64,
    /// Additional owned metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Computes a stable SHA-256 digest for a serializable value under a namespace.
pub fn stable_json_digest<T>(namespace: &str, value: &T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value)?);
    Ok(hex::encode(hasher.finalize()))
}

/// Computes a stable task-spec digest.
pub fn task_spec_digest(task_spec: &BenchmarkTaskSpec) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.task_spec.v1", task_spec)
}

/// Computes a stable artifact-manifest digest.
pub fn artifact_manifest_digest(
    artifact_manifest: &ArtifactManifest,
) -> Result<String, serde_json::Error> {
    stable_json_digest(
        "psionic.legal_benchmark.artifact_manifest.v1",
        artifact_manifest,
    )
}

/// Computes a stable run-config digest.
pub fn run_config_digest(run_config: &RunConfig) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.run_config.v1", run_config)
}

/// Computes a stable run-record digest.
pub fn run_record_digest(run_record: &RunRecord) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.run_record.v1", run_record)
}

/// Computes a stable transcript digest.
pub fn transcript_digest(transcript: &[TranscriptEvent]) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.transcript.v1", &transcript)
}

/// Computes a stable score-report digest.
pub fn score_report_digest(score_report: &ScoreReport) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.score_report.v1", score_report)
}

/// Computes a stable comparison-report digest.
pub fn comparison_report_digest(
    comparison_report: &ComparisonReport,
) -> Result<String, serde_json::Error> {
    stable_json_digest(
        "psionic.legal_benchmark.comparison_report.v1",
        comparison_report,
    )
}

/// Builds a deterministic input artifact manifest from a task spec.
#[must_use]
pub fn build_input_artifact_manifest(task_spec: &BenchmarkTaskSpec) -> ArtifactManifest {
    let mut artifacts = task_spec.source_artifacts.clone();
    sort_artifacts(&mut artifacts);
    ArtifactManifest {
        schema_version: LEGAL_BENCHMARK_SCHEMA_VERSION,
        manifest_id: format!(
            "manifest.input.{}.{}",
            task_spec.task_id, task_spec.task_version
        ),
        task_id: task_spec.task_id.clone(),
        task_version: task_spec.task_version.clone(),
        manifest_role: ArtifactManifestRole::Input,
        artifacts,
        metadata: BTreeMap::new(),
    }
}

/// Builds a deterministic output artifact manifest from generated artifacts.
#[must_use]
pub fn build_output_artifact_manifest(
    task_id: impl Into<String>,
    task_version: impl Into<String>,
    run_id: impl Into<String>,
    mut artifacts: Vec<SourceArtifact>,
) -> ArtifactManifest {
    let task_id = task_id.into();
    let task_version = task_version.into();
    let run_id = run_id.into();
    sort_artifacts(&mut artifacts);
    ArtifactManifest {
        schema_version: LEGAL_BENCHMARK_SCHEMA_VERSION,
        manifest_id: format!("manifest.output.{task_id}.{task_version}.{run_id}"),
        task_id,
        task_version,
        manifest_role: ArtifactManifestRole::Output,
        artifacts,
        metadata: BTreeMap::new(),
    }
}

/// Builds a deterministic derived artifact manifest from extraction or
/// intermediate artifacts.
#[must_use]
pub fn build_derived_artifact_manifest(
    task_id: impl Into<String>,
    task_version: impl Into<String>,
    manifest_id_suffix: impl Into<String>,
    mut artifacts: Vec<SourceArtifact>,
) -> ArtifactManifest {
    let task_id = task_id.into();
    let task_version = task_version.into();
    let manifest_id_suffix = manifest_id_suffix.into();
    sort_artifacts(&mut artifacts);
    ArtifactManifest {
        schema_version: LEGAL_BENCHMARK_SCHEMA_VERSION,
        manifest_id: format!("manifest.derived.{task_id}.{task_version}.{manifest_id_suffix}"),
        task_id,
        task_version,
        manifest_role: ArtifactManifestRole::Derived,
        artifacts,
        metadata: BTreeMap::new(),
    }
}

/// Creates a file-backed artifact with SHA-256, byte length, relative path, and
/// media type.
pub fn artifact_from_file(
    artifact_id: impl Into<String>,
    artifact_kind: ArtifactKind,
    root: impl AsRef<Path>,
    path: impl AsRef<Path>,
    data_classification: DataClassification,
    provenance: Option<String>,
) -> Result<SourceArtifact, ArtifactManifestError> {
    let root = root.as_ref();
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| ArtifactManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let relative_path = path
        .strip_prefix(root)
        .map_err(|_| ArtifactManifestError::RelativePath {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        })?
        .to_string_lossy()
        .to_string();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let original_filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact")
        .to_owned();

    Ok(SourceArtifact {
        artifact_id: artifact_id.into(),
        artifact_kind,
        relative_path: relative_path.clone(),
        original_filename,
        media_type: infer_media_type(&relative_path, &bytes).to_owned(),
        byte_size: bytes.len().try_into().unwrap_or(u64::MAX),
        sha256: hex::encode(hasher.finalize()),
        data_classification,
        provenance,
    })
}

/// Compares two manifests by artifact id and stable artifact identity fields.
pub fn compare_artifact_manifests(
    old_manifest: &ArtifactManifest,
    new_manifest: &ArtifactManifest,
) -> Result<ArtifactManifestComparison, ArtifactManifestError> {
    let old_manifest_hash = artifact_manifest_digest(old_manifest)?;
    let new_manifest_hash = artifact_manifest_digest(new_manifest)?;
    let old_by_id = old_manifest
        .artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    let new_by_id = new_manifest
        .artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.clone(), artifact))
        .collect::<BTreeMap<_, _>>();

    let mut added_artifact_ids = Vec::new();
    let mut removed_artifact_ids = Vec::new();
    let mut changed_artifact_ids = Vec::new();
    let mut unchanged_artifact_ids = Vec::new();

    for artifact_id in old_by_id.keys() {
        if !new_by_id.contains_key(artifact_id) {
            removed_artifact_ids.push(artifact_id.clone());
        }
    }
    for (artifact_id, new_artifact) in &new_by_id {
        match old_by_id.get(artifact_id) {
            Some(old_artifact) if artifact_identity_changed(old_artifact, new_artifact) => {
                changed_artifact_ids.push(artifact_id.clone());
            }
            Some(_) => unchanged_artifact_ids.push(artifact_id.clone()),
            None => added_artifact_ids.push(artifact_id.clone()),
        }
    }

    Ok(ArtifactManifestComparison {
        old_manifest_hash,
        new_manifest_hash,
        added_artifact_ids,
        removed_artifact_ids,
        changed_artifact_ids,
        unchanged_artifact_ids,
    })
}

fn artifact_identity_changed(old_artifact: &SourceArtifact, new_artifact: &SourceArtifact) -> bool {
    old_artifact.artifact_kind != new_artifact.artifact_kind
        || old_artifact.relative_path != new_artifact.relative_path
        || old_artifact.media_type != new_artifact.media_type
        || old_artifact.byte_size != new_artifact.byte_size
        || old_artifact.sha256 != new_artifact.sha256
        || old_artifact.data_classification != new_artifact.data_classification
}

fn sort_artifacts(artifacts: &mut [SourceArtifact]) {
    artifacts.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
    });
}

fn infer_media_type(path: &str, bytes: &[u8]) -> &'static str {
    match file_extension(path).as_deref() {
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("eml") => "message/rfc822",
        Some("json") => "application/json",
        Some("md" | "markdown") => "text/markdown",
        Some("pdf") => "application/pdf",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("txt") => "text/plain",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ if std::str::from_utf8(bytes).is_ok() => "text/plain",
        _ => "application/octet-stream",
    }
}

fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct MinimalTaskBundle {
        task_spec: BenchmarkTaskSpec,
        input_manifest: ArtifactManifest,
        output_manifest: ArtifactManifest,
        run_config: RunConfig,
        run_record: RunRecord,
        score_report: ScoreReport,
        comparison_report: ComparisonReport,
    }

    fn fixture_bundle() -> MinimalTaskBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/minimal_task_bundle.json"
        ))
        .expect("minimal legal benchmark fixture parses")
    }

    #[test]
    fn minimal_fixture_round_trips_all_core_schemas() {
        let fixture = fixture_bundle();
        let encoded = serde_json::to_string_pretty(&fixture).expect("fixture serializes");
        let decoded: MinimalTaskBundle =
            serde_json::from_str(&encoded).expect("fixture deserializes");
        assert_eq!(fixture, decoded);
    }

    #[test]
    fn stable_hash_helpers_are_deterministic() {
        let fixture = fixture_bundle();
        let task_digest_a = task_spec_digest(&fixture.task_spec).expect("task digest");
        let task_digest_b = task_spec_digest(&fixture.task_spec).expect("task digest repeat");
        assert_eq!(task_digest_a, task_digest_b);
        assert_eq!(task_digest_a.len(), 64);

        let input_digest_a =
            artifact_manifest_digest(&fixture.input_manifest).expect("input manifest digest");
        let input_digest_b =
            artifact_manifest_digest(&fixture.input_manifest).expect("input manifest repeat");
        assert_eq!(input_digest_a, input_digest_b);
        assert_eq!(input_digest_a.len(), 64);

        let output_digest =
            artifact_manifest_digest(&fixture.output_manifest).expect("output manifest digest");
        let run_config_hash = run_config_digest(&fixture.run_config).expect("run config digest");
        let run_record_hash = run_record_digest(&fixture.run_record).expect("run record digest");
        let transcript_hash =
            transcript_digest(&fixture.run_record.transcript).expect("transcript digest");
        let score_hash = score_report_digest(&fixture.score_report).expect("score digest");
        let comparison_hash =
            comparison_report_digest(&fixture.comparison_report).expect("comparison digest");

        assert_eq!(output_digest.len(), 64);
        assert_eq!(run_config_hash.len(), 64);
        assert_eq!(run_record_hash.len(), 64);
        assert_eq!(transcript_hash.len(), 64);
        assert_eq!(score_hash.len(), 64);
        assert_eq!(comparison_hash.len(), 64);
    }

    #[test]
    fn input_manifest_builder_is_deterministic() {
        let fixture = fixture_bundle();
        let mut reversed_task = fixture.task_spec.clone();
        reversed_task.source_artifacts.reverse();

        let manifest_a = build_input_artifact_manifest(&fixture.task_spec);
        let manifest_b = build_input_artifact_manifest(&reversed_task);

        assert_eq!(manifest_a, manifest_b);
        assert_eq!(manifest_a.manifest_role, ArtifactManifestRole::Input);
        assert_eq!(
            manifest_a.artifacts.len(),
            fixture.task_spec.source_artifacts.len()
        );
        assert_eq!(
            artifact_manifest_digest(&manifest_a).expect("manifest digest"),
            artifact_manifest_digest(&manifest_b).expect("manifest digest repeat")
        );
    }

    #[test]
    fn output_manifest_builder_sorts_generated_artifacts() {
        let fixture = fixture_bundle();
        let artifact = fixture.output_manifest.artifacts[0].clone();
        let manifest = build_output_artifact_manifest(
            fixture.task_spec.task_id,
            fixture.task_spec.task_version,
            "run.fixture",
            vec![artifact.clone()],
        );

        assert_eq!(manifest.manifest_role, ArtifactManifestRole::Output);
        assert_eq!(manifest.artifacts, vec![artifact]);
        assert!(manifest.manifest_id.contains("manifest.output."));
        assert_eq!(
            artifact_manifest_digest(&manifest)
                .expect("output manifest digest")
                .len(),
            64
        );
    }

    #[test]
    fn file_artifact_records_hash_size_and_media_type() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/legal_benchmark/harvey_compatibility_sample/tasks/corporate/sample-contract-review",
        );
        let path = root.join("documents/service-agreement.txt");
        let artifact = artifact_from_file(
            "artifact.source.service_agreement",
            ArtifactKind::SourceDocument,
            &root,
            &path,
            DataClassification::PublicReference,
            Some(String::from("fixture")),
        )
        .expect("file artifact");

        assert_eq!(artifact.relative_path, "documents/service-agreement.txt");
        assert_eq!(artifact.media_type, "text/plain");
        assert_eq!(artifact.byte_size, 128);
        assert_eq!(artifact.sha256.len(), 64);
    }

    #[test]
    fn manifest_comparison_detects_changed_inputs() {
        let fixture = fixture_bundle();
        let old_manifest = build_input_artifact_manifest(&fixture.task_spec);
        let mut changed_task = fixture.task_spec;
        changed_task.source_artifacts[0].sha256 =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");
        let new_manifest = build_input_artifact_manifest(&changed_task);

        let comparison =
            compare_artifact_manifests(&old_manifest, &new_manifest).expect("comparison");

        assert!(!comparison.is_unchanged());
        assert_eq!(
            comparison.changed_artifact_ids,
            vec![String::from("artifact.source.service_agreement")]
        );
        assert!(comparison.added_artifact_ids.is_empty());
        assert!(comparison.removed_artifact_ids.is_empty());
        assert_eq!(comparison.old_manifest_hash.len(), 64);
        assert_eq!(comparison.new_manifest_hash.len(), 64);
        assert_ne!(comparison.old_manifest_hash, comparison.new_manifest_hash);
    }

    #[test]
    fn run_record_requires_identity_and_manifest_fields() {
        let missing_required = serde_json::json!({
            "schema_version": LEGAL_BENCHMARK_SCHEMA_VERSION,
            "run_id": "run.missing-fields",
            "terminal_state": "submitted",
            "transcript": [],
            "tool_calls": [],
            "metrics": {
                "model_turns": 0,
                "tool_call_count": 0,
                "input_tokens": 0,
                "output_tokens": 0,
                "wall_time_ms": 0,
                "estimated_cost_micro_usd": 0
            }
        });

        let error = serde_json::from_value::<RunRecord>(missing_required)
            .expect_err("required run record fields are enforced");
        let message = error.to_string();
        assert!(
            message.contains("task_id")
                || message.contains("task_version")
                || message.contains("input_artifact_manifest_hash")
                || message.contains("run_config_hash")
                || message.contains("output_artifact_manifest_hash")
        );
    }

    #[test]
    fn task_spec_requires_task_identity() {
        let error = serde_json::from_value::<BenchmarkTaskSpec>(serde_json::json!({
            "schema_version": LEGAL_BENCHMARK_SCHEMA_VERSION
        }))
        .expect_err("required task spec fields are enforced");
        let message = error.to_string();
        assert!(message.contains("task_id"));
    }
}
