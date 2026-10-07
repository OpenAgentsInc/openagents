use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    build_starter_plugin_tool_bridge_bundle, starter_plugin_registration_by_tool_name,
    tassadar_post_article_starter_plugin_workflow_controller_bundle_path,
    StarterPluginAuthoringClass, StarterPluginOriginClass,
    StarterPluginProjectedToolResultEnvelope, StarterPluginToolBridgeBundle,
    StarterPluginWorkflowCase, StarterPluginWorkflowControllerBundle,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_BUNDLE_REF: &str = "fixtures/tassadar/datasets/tassadar_multi_plugin_trace_corpus_v1/tassadar_multi_plugin_trace_corpus_bundle.json";
pub const TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_RUN_ROOT_REF: &str =
    "fixtures/tassadar/datasets/tassadar_multi_plugin_trace_corpus_v1";

const ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_router_plugin_tool_loop_pilot_v1/tassadar_post_article_router_plugin_tool_loop_pilot_bundle.json";
const APPLE_FM_PLUGIN_SESSION_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_apple_fm_plugin_session_pilot_v1/tassadar_post_article_apple_fm_plugin_session_pilot_bundle.json";
const WEIGHTED_CONTROLLER_REPORT_REF: &str = "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report.json";

const TRACE_RECORD_SCHEMA_ID: &str = "psionic.tassadar.multi_plugin_trace_record.v1";
const PARITY_MATRIX_SCHEMA_ID: &str = "psionic.tassadar.multi_plugin_parity_matrix.v1";
const BOOTSTRAP_CONTRACT_ID: &str = "psionic.tassadar.multi_plugin_training_bootstrap.v1";

const LANE_DETERMINISTIC_WORKFLOW: &str = "deterministic_workflow";
const LANE_ROUTER_RESPONSES: &str = "router_responses";
const LANE_APPLE_FM_SESSION: &str = "apple_fm_session";

const WORKFLOW_WEB_CONTENT_SUCCESS: &str = "starter_plugin.web_content_success.v1";
const WORKFLOW_FETCH_REFUSAL: &str = "starter_plugin.fetch_refusal.v1";
const WORKFLOW_GUEST_ARTIFACT_SUCCESS: &str = "starter_plugin.guest_artifact_success.v1";

const STOP_COMPLETED_SUCCESS: &str = "workflow_stop.completed_success";
const STOP_TYPED_REFUSAL: &str = "workflow_stop.typed_refusal";
const STOP_GUEST_ARTIFACT_COMPLETED: &str = "workflow_stop.guest_artifact_completed_success";

const TOOL_TEXT_URL_EXTRACT: &str = "plugin_text_url_extract";
const TOOL_HTTP_FETCH_TEXT: &str = "plugin_http_fetch_text";
const TOOL_HTML_EXTRACT_READABLE: &str = "plugin_html_extract_readable";
const TOOL_FEED_RSS_ATOM_PARSE: &str = "plugin_feed_rss_atom_parse";
const TOOL_GUEST_ECHO: &str = "plugin_example_echo_guest";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginSourceBundleRow {
    pub lane_id: String,
    pub bundle_ref: String,
    pub bundle_id: String,
    pub bundle_digest: String,
    pub case_count: u16,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginProjectedToolSchemaRow {
    pub tool_name: String,
    pub plugin_id: String,
    pub authoring_class_id: String,
    pub origin_class_id: String,
    pub description: String,
    pub arguments_schema: Value,
    pub arguments_schema_digest: String,
    pub result_schema_id: String,
    pub refusal_schema_ids: Vec<String>,
    pub replay_class_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginControllerDecisionRow {
    pub decision_index: u16,
    pub decision_source: String,
    pub decision_kind: String,
    pub decision_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarMultiPluginTraceStepRow {
    pub step_index: u16,
    pub decision_ref: String,
    pub result_ref: String,
    pub tool_name: String,
    pub plugin_id: String,
    pub authoring_class_id: String,
    pub origin_class_id: String,
    pub arguments: Value,
    pub projected_result: StarterPluginProjectedToolResultEnvelope,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginClassExpansionRow {
    pub authoring_class_id: String,
    pub origin_class_id: String,
    pub plugin_ids: Vec<String>,
    pub tool_names: Vec<String>,
    pub workflow_case_ids: Vec<String>,
    pub lane_ids: Vec<String>,
    pub trace_record_ids: Vec<String>,
    pub weighted_controller_report_ref: String,
    pub weighted_controller_report_id: String,
    pub weighted_controller_runtime_bundle_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarMultiPluginTraceRecord {
    pub record_schema_id: String,
    pub record_id: String,
    pub workflow_case_id: String,
    pub source_case_id: String,
    pub lane_id: String,
    pub source_bundle_ref: String,
    pub source_bundle_id: String,
    pub source_bundle_digest: String,
    pub directive_text: String,
    pub admissible_tool_names: Vec<String>,
    pub projected_tool_schema_rows: Vec<TassadarMultiPluginProjectedToolSchemaRow>,
    pub controller_decision_rows: Vec<TassadarMultiPluginControllerDecisionRow>,
    pub step_rows: Vec<TassadarMultiPluginTraceStepRow>,
    pub normalized_stop_condition_id: String,
    pub source_stop_condition_id: String,
    pub final_message_text: String,
    pub typed_refusal_preserved: bool,
    pub detail: String,
    pub record_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginToolSchemaParityRow {
    pub tool_name: String,
    pub plugin_id: String,
    pub canonical_arguments_schema_digest: String,
    pub router_bundle_arguments_schema_digest: String,
    pub apple_fm_bundle_arguments_schema_digest: String,
    pub stable_across_lanes: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginWorkflowStepParityRow {
    pub step_index: u16,
    pub lanes_present: Vec<String>,
    pub tool_name_by_lane: BTreeMap<String, String>,
    pub status_by_lane: BTreeMap<String, String>,
    pub arguments_digest_by_lane: BTreeMap<String, String>,
    pub payload_digest_by_lane: BTreeMap<String, String>,
    pub receipt_id_by_lane: BTreeMap<String, String>,
    pub agreement_class: String,
    pub disagreement_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginWorkflowDisagreementRow {
    pub workflow_case_id: String,
    pub scope_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_index: Option<u16>,
    pub disagreement_kind: String,
    pub detail: String,
    pub lane_values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginWorkflowParityRow {
    pub workflow_case_id: String,
    pub lane_record_ids: BTreeMap<String, String>,
    pub directive_digest_by_lane: BTreeMap<String, String>,
    pub normalized_stop_condition_id: String,
    pub source_stop_condition_id_by_lane: BTreeMap<String, String>,
    pub step_parity_rows: Vec<TassadarMultiPluginWorkflowStepParityRow>,
    pub disagreement_rows: Vec<TassadarMultiPluginWorkflowDisagreementRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginParityMatrix {
    pub schema_id: String,
    pub tool_schema_parity_rows: Vec<TassadarMultiPluginToolSchemaParityRow>,
    pub workflow_parity_rows: Vec<TassadarMultiPluginWorkflowParityRow>,
    pub explicit_disagreement_count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiPluginTrainingBootstrapContract {
    pub contract_id: String,
    pub target_deferred_issue_id: String,
    pub trace_record_schema_id: String,
    pub admitted_controller_lanes: Vec<String>,
    pub admitted_workflow_case_ids: Vec<String>,
    pub requires_receipt_identity: bool,
    pub preserves_disagreement_rows: bool,
    pub bootstrap_ready: bool,
    pub bootstrap_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarMultiPluginTraceCorpusBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub source_bundle_rows: Vec<TassadarMultiPluginSourceBundleRow>,
    pub projected_tool_schema_rows: Vec<TassadarMultiPluginProjectedToolSchemaRow>,
    pub trace_records: Vec<TassadarMultiPluginTraceRecord>,
    pub class_expansion_rows: Vec<TassadarMultiPluginClassExpansionRow>,
    pub parity_matrix: TassadarMultiPluginParityMatrix,
    pub bootstrap_contract: TassadarMultiPluginTrainingBootstrapContract,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarMultiPluginTraceCorpusError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("unknown workflow source case `{case_id}`")]
    UnknownWorkflowCase { case_id: String },
    #[error("workflow step `{step_index}` in `{case_id}` is missing a required fetch predecessor")]
    MissingFetchPredecessor { case_id: String, step_index: u16 },
    #[error("tool parity could not find committed digest for `{lane_id}` `{tool_name}`")]
    MissingToolDigest { lane_id: String, tool_name: String },
    #[error("trace corpus could not find projected tool schema for `{tool_name}`")]
    MissingProjectedToolSchema { tool_name: String },
    #[error("trace corpus could not find runtime registration for `{tool_name}`")]
    MissingRuntimeRegistration { tool_name: String },
    #[error("workflow parity could not find all expected lane records for `{workflow_case_id}`")]
    MissingWorkflowLane { workflow_case_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize)]
struct RouterPilotBundle {
    bundle_id: String,
    bundle_digest: String,
    claim_boundary: String,
    tool_definition_rows: Vec<RouterToolDefinitionRow>,
    case_rows: Vec<RouterPilotCase>,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterToolDefinitionRow {
    tool_name: String,
    parameters_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterPilotCase {
    case_id: String,
    directive: String,
    tool_loop_outcome: RouterToolLoopOutcome,
    typed_refusal_preserved: bool,
    detail: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterToolLoopOutcome {
    termination_reason: String,
    final_message: RouterMessage,
    steps: Vec<RouterToolLoopStep>,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterMessage {
    content: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterToolLoopStep {
    step_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assistant_message: Option<RouterMessage>,
    #[serde(default)]
    tool_calls: Vec<RouterToolCall>,
    #[serde(default)]
    tool_results: Vec<RouterToolResult>,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterToolCall {
    id: String,
    name: String,
    arguments: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct RouterToolResult {
    tool_call_id: String,
    tool_name: String,
    structured: StarterPluginProjectedToolResultEnvelope,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmPilotBundle {
    bundle_id: String,
    bundle_digest: String,
    claim_boundary: String,
    tool_definition_rows: Vec<AppleFmToolDefinitionRow>,
    case_rows: Vec<AppleFmPilotCase>,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmToolDefinitionRow {
    tool_name: String,
    arguments_schema_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmPilotCase {
    case_id: String,
    directive: String,
    transcript: AppleFmTranscript,
    step_rows: Vec<AppleFmPilotStep>,
    typed_refusal_preserved: bool,
    detail: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmPilotStep {
    step_index: usize,
    tool_name: String,
    arguments: Value,
    projected_result: StarterPluginProjectedToolResultEnvelope,
    transcript_call_entry_id: String,
    transcript_tool_entry_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmTranscript {
    transcript: AppleFmTranscriptPayload,
}

#[derive(Clone, Debug, Deserialize)]
struct WeightedControllerReport {
    report_id: String,
    runtime_bundle: WeightedControllerRuntimeBundle,
}

#[derive(Clone, Debug, Deserialize)]
struct WeightedControllerRuntimeBundle {
    bundle_id: String,
    starter_plugin_admission_rows: Vec<WeightedControllerStarterPluginAdmissionRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct WeightedControllerStarterPluginAdmissionRow {
    plugin_id: String,
    tool_name: String,
    authoring_class_id: String,
    origin_class_id: String,
    detail: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmTranscriptPayload {
    entries: Vec<AppleFmTranscriptEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmTranscriptEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default)]
    contents: Vec<AppleFmTranscriptContent>,
}

#[derive(Clone, Debug, Deserialize)]
struct AppleFmTranscriptContent {
    #[serde(rename = "type")]
    _content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[must_use]
pub fn tassadar_multi_plugin_trace_corpus_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_BUNDLE_REF)
}

pub fn build_tassadar_multi_plugin_trace_corpus_bundle(
) -> Result<TassadarMultiPluginTraceCorpusBundle, TassadarMultiPluginTraceCorpusError> {
    let bridge_bundle = build_starter_plugin_tool_bridge_bundle();

    let deterministic_bundle: StarterPluginWorkflowControllerBundle =
        read_json(tassadar_post_article_starter_plugin_workflow_controller_bundle_path())?;
    let router_bundle: RouterPilotBundle =
        read_json(repo_root().join(ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF))?;
    let apple_bundle: AppleFmPilotBundle =
        read_json(repo_root().join(APPLE_FM_PLUGIN_SESSION_BUNDLE_REF))?;
    let weighted_controller_report: WeightedControllerReport =
        read_json(repo_root().join(WEIGHTED_CONTROLLER_REPORT_REF))?;

    let source_bundle_rows = vec![
        TassadarMultiPluginSourceBundleRow {
            lane_id: String::from(LANE_DETERMINISTIC_WORKFLOW),
            bundle_ref: String::from(
                psionic_runtime::TASSADAR_POST_ARTICLE_STARTER_PLUGIN_WORKFLOW_CONTROLLER_BUNDLE_REF,
            ),
            bundle_id: deterministic_bundle.bundle_id.clone(),
            bundle_digest: deterministic_bundle.bundle_digest.clone(),
            case_count: deterministic_bundle.case_rows.len() as u16,
            claim_boundary: deterministic_bundle.claim_boundary.clone(),
        },
        TassadarMultiPluginSourceBundleRow {
            lane_id: String::from(LANE_ROUTER_RESPONSES),
            bundle_ref: String::from(ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF),
            bundle_id: router_bundle.bundle_id.clone(),
            bundle_digest: router_bundle.bundle_digest.clone(),
            case_count: router_bundle.case_rows.len() as u16,
            claim_boundary: router_bundle.claim_boundary.clone(),
        },
        TassadarMultiPluginSourceBundleRow {
            lane_id: String::from(LANE_APPLE_FM_SESSION),
            bundle_ref: String::from(APPLE_FM_PLUGIN_SESSION_BUNDLE_REF),
            bundle_id: apple_bundle.bundle_id.clone(),
            bundle_digest: apple_bundle.bundle_digest.clone(),
            case_count: apple_bundle.case_rows.len() as u16,
            claim_boundary: apple_bundle.claim_boundary.clone(),
        },
    ];

    let mut trace_records = Vec::new();
    trace_records.extend(build_deterministic_trace_records(
        &deterministic_bundle,
        &bridge_bundle,
    )?);
    trace_records.extend(build_router_trace_records(&router_bundle, &bridge_bundle)?);
    trace_records.extend(build_apple_fm_trace_records(&apple_bundle, &bridge_bundle)?);
    trace_records.sort_by(|left, right| left.record_id.cmp(&right.record_id));
    let projected_tool_schema_rows = projected_tool_schema_rows_from_trace_records(&trace_records);
    let class_expansion_rows =
        build_class_expansion_rows(&weighted_controller_report, trace_records.as_slice());

    let parity_matrix = build_parity_matrix(
        &bridge_bundle,
        &router_bundle,
        &apple_bundle,
        trace_records.as_slice(),
    )?;
    let bootstrap_contract = build_bootstrap_contract(&trace_records, &parity_matrix);
    let mut bundle = TassadarMultiPluginTraceCorpusBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.multi_plugin_trace_corpus.bundle.v1"),
        source_bundle_rows,
        projected_tool_schema_rows,
        trace_records,
        class_expansion_rows,
        parity_matrix,
        bootstrap_contract,
        claim_boundary: String::from(
            "this bundle freezes a repo-owned multi-plugin trace corpus plus an explicit parity matrix across deterministic, router-owned, and local Apple FM controller lanes. It preserves the three-lane host-native workflow families and one separate deterministic-only digest-bound guest-artifact workflow without collapsing them into one false controller-equivalence story. It is bootstrap input to later TAS-204 weighted-controller work and does not claim weighted-controller closure, served-model equivalence, or canonical Tassadar proof closure.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "multi-plugin trace corpus freezes source_bundles={}, trace_records={}, class_expansion_rows={}, workflow_parity_rows={}, and explicit_disagreements={}.",
        bundle.source_bundle_rows.len(),
        bundle.trace_records.len(),
        bundle.class_expansion_rows.len(),
        bundle.parity_matrix.workflow_parity_rows.len(),
        bundle.parity_matrix.explicit_disagreement_count,
    );
    bundle.bundle_digest = stable_digest(b"tassadar_multi_plugin_trace_corpus_bundle|", &bundle);
    Ok(bundle)
}

pub fn write_tassadar_multi_plugin_trace_corpus_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMultiPluginTraceCorpusBundle, TassadarMultiPluginTraceCorpusError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMultiPluginTraceCorpusError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_multi_plugin_trace_corpus_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMultiPluginTraceCorpusError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_multi_plugin_trace_corpus_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarMultiPluginTraceCorpusBundle, TassadarMultiPluginTraceCorpusError> {
    read_json(path)
}

fn projected_tool_schema_rows_from_trace_records(
    trace_records: &[TassadarMultiPluginTraceRecord],
) -> Vec<TassadarMultiPluginProjectedToolSchemaRow> {
    let mut rows = BTreeMap::new();
    for record in trace_records {
        for row in &record.projected_tool_schema_rows {
            rows.entry(row.tool_name.clone())
                .or_insert_with(|| row.clone());
        }
    }
    rows.into_values().collect()
}

fn build_class_expansion_rows(
    weighted_controller_report: &WeightedControllerReport,
    trace_records: &[TassadarMultiPluginTraceRecord],
) -> Vec<TassadarMultiPluginClassExpansionRow> {
    let mut rows = weighted_controller_report
        .runtime_bundle
        .starter_plugin_admission_rows
        .iter()
        .filter(|row| {
            matches!(
                row.authoring_class_id.as_str(),
                "networked_read_only" | "guest_artifact_digest_bound"
            )
        })
        .map(|admission| {
            let matching_records = trace_records
                .iter()
                .filter(|record| {
                    record
                        .step_rows
                        .iter()
                        .any(|step| step.tool_name == admission.tool_name)
                })
                .collect::<Vec<_>>();
            let mut workflow_case_ids = matching_records
                .iter()
                .map(|record| record.workflow_case_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            workflow_case_ids.sort();
            let mut lane_ids = matching_records
                .iter()
                .map(|record| record.lane_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            lane_ids.sort();
            let mut trace_record_ids = matching_records
                .iter()
                .map(|record| record.record_id.clone())
                .collect::<Vec<_>>();
            trace_record_ids.sort();
            let detail = match admission.authoring_class_id.as_str() {
                "networked_read_only" => String::from(
                    "The networked_read_only class now has explicit corpus growth only because the admitted fetch plugin landed in the weighted-controller lane and appears across the shared host-native workflow families.",
                ),
                "guest_artifact_digest_bound" => String::from(
                    "The guest_artifact_digest_bound class now has explicit corpus growth only because the admitted digest-bound guest plugin landed in the weighted-controller lane and appears as a separate deterministic-only workflow without false three-lane parity.",
                ),
                _ => admission.detail.clone(),
            };
            TassadarMultiPluginClassExpansionRow {
                authoring_class_id: admission.authoring_class_id.clone(),
                origin_class_id: admission.origin_class_id.clone(),
                plugin_ids: vec![admission.plugin_id.clone()],
                tool_names: vec![admission.tool_name.clone()],
                workflow_case_ids,
                lane_ids,
                trace_record_ids,
                weighted_controller_report_ref: String::from(WEIGHTED_CONTROLLER_REPORT_REF),
                weighted_controller_report_id: weighted_controller_report.report_id.clone(),
                weighted_controller_runtime_bundle_id: weighted_controller_report
                    .runtime_bundle
                    .bundle_id
                    .clone(),
                detail,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.authoring_class_id.cmp(&right.authoring_class_id));
    rows
}

fn projected_tool_schema_rows_for_requested_tool_names(
    bridge_bundle: &StarterPluginToolBridgeBundle,
    tool_names: &BTreeSet<String>,
) -> Result<Vec<TassadarMultiPluginProjectedToolSchemaRow>, TassadarMultiPluginTraceCorpusError> {
    let mut rows = tool_names
        .iter()
        .map(|tool_name| {
            let row = bridge_bundle
                .projection_rows
                .iter()
                .find(|row| row.tool_name == *tool_name)
                .ok_or_else(
                    || TassadarMultiPluginTraceCorpusError::MissingProjectedToolSchema {
                        tool_name: tool_name.clone(),
                    },
                )?;
            let registration =
                starter_plugin_registration_by_tool_name(tool_name).ok_or_else(|| {
                    TassadarMultiPluginTraceCorpusError::MissingRuntimeRegistration {
                        tool_name: tool_name.clone(),
                    }
                })?;
            Ok(TassadarMultiPluginProjectedToolSchemaRow {
                tool_name: row.tool_name.clone(),
                plugin_id: row.plugin_id.clone(),
                authoring_class_id: authoring_class_id(registration.authoring_class),
                origin_class_id: origin_class_id(registration.origin_class),
                description: row.deterministic_projection.description.clone(),
                arguments_schema: row.deterministic_projection.arguments_schema.clone(),
                arguments_schema_digest: row.arguments_schema_digest.clone(),
                result_schema_id: row.result_schema_id.clone(),
                refusal_schema_ids: row.refusal_schema_ids.clone(),
                replay_class_id: row.replay_class_id.clone(),
            })
        })
        .collect::<Result<Vec<_>, TassadarMultiPluginTraceCorpusError>>()?;
    rows.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
    Ok(rows)
}

fn build_deterministic_trace_records(
    bundle: &StarterPluginWorkflowControllerBundle,
    bridge_bundle: &StarterPluginToolBridgeBundle,
) -> Result<Vec<TassadarMultiPluginTraceRecord>, TassadarMultiPluginTraceCorpusError> {
    bundle
        .case_rows
        .iter()
        .map(|case| {
            let workflow_case_id = normalized_workflow_case_id(case.case_id.as_str())?;
            let normalized_stop_condition_id = normalized_stop_condition_id(case.case_id.as_str())?;
            let admissible_tool_names = case
                .step_rows
                .iter()
                .map(|step| step.tool_name.clone())
                .collect::<BTreeSet<_>>();
            let projected_tool_schema_rows = projected_tool_schema_rows_for_requested_tool_names(
                bridge_bundle,
                &admissible_tool_names,
            )?;
            let admissible_tool_names = admissible_tool_names.into_iter().collect::<Vec<_>>();
            let mut decision_rows = Vec::new();
            let mut step_rows = Vec::new();
            for step in &case.step_rows {
                let decision_ref = format!("deterministic.tool_call.{}", step.step_index);
                decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                    decision_index: decision_rows.len() as u16,
                    decision_source: String::from("host_owned"),
                    decision_kind: String::from("tool_call"),
                    decision_ref: decision_ref.clone(),
                    tool_name: Some(step.tool_name.clone()),
                    detail: step.detail.clone(),
                });
                step_rows.push(TassadarMultiPluginTraceStepRow {
                    step_index: step.step_index,
                    decision_ref,
                    result_ref: format!("deterministic.tool_result.{}", step.step_index),
                    tool_name: step.tool_name.clone(),
                    plugin_id: step.plugin_id.clone(),
                    authoring_class_id: authoring_class_id_for_tool_name(step.tool_name.as_str())?,
                    origin_class_id: origin_class_id_for_tool_name(step.tool_name.as_str())?,
                    arguments: deterministic_step_arguments(case, step.step_index)?,
                    projected_result: step.projected_result.clone(),
                    detail: step.detail.clone(),
                });
            }
            for decision in &case.decision_rows {
                decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                    decision_index: decision_rows.len() as u16,
                    decision_source: String::from("host_owned"),
                    decision_kind: decision.decision_kind.clone(),
                    decision_ref: format!("deterministic.decision.{}", decision.decision_index),
                    tool_name: None,
                    detail: decision.detail.clone(),
                });
            }
            let stop_detail = case
                .decision_rows
                .iter()
                .find(|row| row.decision_kind == "stop_condition")
                .map(|row| row.detail.clone())
                .unwrap_or_else(|| case.detail.clone());
            let final_message_text = match normalized_stop_condition_id {
                STOP_COMPLETED_SUCCESS => String::from(
                    "The deterministic controller completed after exhausting the extracted URL set.",
                ),
                STOP_TYPED_REFUSAL => {
                    String::from("The deterministic controller stopped after a typed plugin refusal.")
                }
                STOP_GUEST_ARTIFACT_COMPLETED => String::from(
                    "The deterministic controller completed after the digest-bound guest-artifact echo succeeded.",
                ),
                _ => case.detail.clone(),
            };
            Ok(finalize_trace_record(TassadarMultiPluginTraceRecord {
                record_schema_id: String::from(TRACE_RECORD_SCHEMA_ID),
                record_id: format!(
                    "trace_record.{}.{}.v1",
                    workflow_case_id, LANE_DETERMINISTIC_WORKFLOW
                ),
                workflow_case_id: String::from(workflow_case_id),
                source_case_id: case.case_id.clone(),
                lane_id: String::from(LANE_DETERMINISTIC_WORKFLOW),
                source_bundle_ref: String::from(
                    psionic_runtime::TASSADAR_POST_ARTICLE_STARTER_PLUGIN_WORKFLOW_CONTROLLER_BUNDLE_REF,
                ),
                source_bundle_id: bundle.bundle_id.clone(),
                source_bundle_digest: bundle.bundle_digest.clone(),
                directive_text: case.directive_text.clone(),
                admissible_tool_names,
                projected_tool_schema_rows,
                controller_decision_rows: decision_rows,
                step_rows,
                normalized_stop_condition_id: String::from(normalized_stop_condition_id),
                source_stop_condition_id: case.stop_condition_id.clone(),
                final_message_text,
                typed_refusal_preserved: !case.refusal_rows.is_empty(),
                detail: format!("{} stop_detail={stop_detail}", case.detail),
                record_digest: String::new(),
            }))
        })
        .collect()
}

fn deterministic_step_arguments(
    case: &StarterPluginWorkflowCase,
    step_index: u16,
) -> Result<Value, TassadarMultiPluginTraceCorpusError> {
    let step = case
        .step_rows
        .iter()
        .find(|row| row.step_index == step_index)
        .expect("step should exist");
    match step.tool_name.as_str() {
        TOOL_TEXT_URL_EXTRACT => Ok(serde_json::json!({ "text": case.directive_text })),
        TOOL_HTTP_FETCH_TEXT => Ok(serde_json::json!({ "url": step.subject_id })),
        TOOL_HTML_EXTRACT_READABLE => {
            let fetch_payload =
                preceding_fetch_payload(case, step_index, step.subject_id.as_str())?;
            Ok(serde_json::json!({
                "source_url": step.subject_id,
                "content_type": fetch_payload
                    .get("content_type")
                    .cloned()
                    .unwrap_or(Value::Null),
                "body_text": fetch_payload.get("body_text").cloned().unwrap_or(Value::Null),
            }))
        }
        TOOL_FEED_RSS_ATOM_PARSE => {
            let fetch_payload =
                preceding_fetch_payload(case, step_index, step.subject_id.as_str())?;
            Ok(serde_json::json!({
                "source_url": step.subject_id,
                "content_type": fetch_payload
                    .get("content_type")
                    .cloned()
                    .unwrap_or(Value::Null),
                "feed_text": fetch_payload.get("body_text").cloned().unwrap_or(Value::Null),
            }))
        }
        TOOL_GUEST_ECHO => Ok(step.projected_result.structured_payload.clone()),
        _ => Ok(Value::Null),
    }
}

fn preceding_fetch_payload(
    case: &StarterPluginWorkflowCase,
    step_index: u16,
    subject_id: &str,
) -> Result<Value, TassadarMultiPluginTraceCorpusError> {
    case.step_rows
        .iter()
        .rev()
        .find(|row| {
            row.step_index < step_index
                && row.tool_name == TOOL_HTTP_FETCH_TEXT
                && row.subject_id == subject_id
        })
        .map(|row| row.projected_result.structured_payload.clone())
        .ok_or_else(
            || TassadarMultiPluginTraceCorpusError::MissingFetchPredecessor {
                case_id: case.case_id.clone(),
                step_index,
            },
        )
}

fn build_router_trace_records(
    bundle: &RouterPilotBundle,
    bridge_bundle: &StarterPluginToolBridgeBundle,
) -> Result<Vec<TassadarMultiPluginTraceRecord>, TassadarMultiPluginTraceCorpusError> {
    bundle
        .case_rows
        .iter()
        .map(|case| {
            let workflow_case_id = normalized_workflow_case_id(case.case_id.as_str())?;
            let normalized_stop_condition_id = normalized_stop_condition_id(case.case_id.as_str())?;
            let admissible_tool_names = case
                .tool_loop_outcome
                .steps
                .iter()
                .filter_map(|step| {
                    step.tool_results
                        .first()
                        .map(|result| result.tool_name.clone())
                })
                .collect::<BTreeSet<_>>();
            let projected_tool_schema_rows = projected_tool_schema_rows_for_requested_tool_names(
                bridge_bundle,
                &admissible_tool_names,
            )?;
            let admissible_tool_names = admissible_tool_names.into_iter().collect::<Vec<_>>();
            let mut decision_rows = Vec::new();
            let mut step_rows = Vec::new();
            for step in &case.tool_loop_outcome.steps {
                if step.tool_results.is_empty() {
                    continue;
                }
                let tool_call = step
                    .tool_calls
                    .first()
                    .expect("router tool-result step should carry tool call");
                let tool_result = step
                    .tool_results
                    .first()
                    .expect("router tool-result step should carry tool result");
                let detail = step
                    .assistant_message
                    .as_ref()
                    .map(|message| message.content.clone())
                    .unwrap_or_else(|| {
                        String::from(
                            "the served seed response issued the first router-owned tool call.",
                        )
                    });
                decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                    decision_index: decision_rows.len() as u16,
                    decision_source: String::from("router_assistant"),
                    decision_kind: String::from("tool_call"),
                    decision_ref: tool_call.id.clone(),
                    tool_name: Some(tool_call.name.clone()),
                    detail: detail.clone(),
                });
                step_rows.push(TassadarMultiPluginTraceStepRow {
                    step_index: step.step_index as u16,
                    decision_ref: tool_call.id.clone(),
                    result_ref: tool_result.tool_call_id.clone(),
                    tool_name: tool_result.tool_name.clone(),
                    plugin_id: tool_result.structured.plugin_id.clone(),
                    authoring_class_id: authoring_class_id_for_tool_name(
                        tool_result.tool_name.as_str(),
                    )?,
                    origin_class_id: origin_class_id_for_tool_name(tool_result.tool_name.as_str())?,
                    arguments: tool_call.arguments.clone(),
                    projected_result: tool_result.structured.clone(),
                    detail,
                });
            }
            decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                decision_index: decision_rows.len() as u16,
                decision_source: String::from("router_assistant"),
                decision_kind: String::from("final_message"),
                decision_ref: format!("router.final_message.{}", case.case_id),
                tool_name: None,
                detail: case.tool_loop_outcome.final_message.content.clone(),
            });
            Ok(finalize_trace_record(TassadarMultiPluginTraceRecord {
                record_schema_id: String::from(TRACE_RECORD_SCHEMA_ID),
                record_id: format!(
                    "trace_record.{}.{}.v1",
                    workflow_case_id, LANE_ROUTER_RESPONSES
                ),
                workflow_case_id: String::from(workflow_case_id),
                source_case_id: case.case_id.clone(),
                lane_id: String::from(LANE_ROUTER_RESPONSES),
                source_bundle_ref: String::from(ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF),
                source_bundle_id: bundle.bundle_id.clone(),
                source_bundle_digest: bundle.bundle_digest.clone(),
                directive_text: case.directive.clone(),
                admissible_tool_names,
                projected_tool_schema_rows,
                controller_decision_rows: decision_rows,
                step_rows,
                normalized_stop_condition_id: String::from(normalized_stop_condition_id),
                source_stop_condition_id: format!(
                    "router.termination.{}",
                    case.tool_loop_outcome.termination_reason
                ),
                final_message_text: case.tool_loop_outcome.final_message.content.clone(),
                typed_refusal_preserved: case.typed_refusal_preserved,
                detail: case.detail.clone(),
                record_digest: String::new(),
            }))
        })
        .collect()
}

fn build_apple_fm_trace_records(
    bundle: &AppleFmPilotBundle,
    bridge_bundle: &StarterPluginToolBridgeBundle,
) -> Result<Vec<TassadarMultiPluginTraceRecord>, TassadarMultiPluginTraceCorpusError> {
    bundle
        .case_rows
        .iter()
        .map(|case| {
            let workflow_case_id = normalized_workflow_case_id(case.case_id.as_str())?;
            let normalized_stop_condition_id = normalized_stop_condition_id(case.case_id.as_str())?;
            let admissible_tool_names = case
                .step_rows
                .iter()
                .map(|step| step.tool_name.clone())
                .collect::<BTreeSet<_>>();
            let projected_tool_schema_rows = projected_tool_schema_rows_for_requested_tool_names(
                bridge_bundle,
                &admissible_tool_names,
            )?;
            let admissible_tool_names = admissible_tool_names.into_iter().collect::<Vec<_>>();
            let transcript_text_by_id = apple_fm_transcript_text_by_id(&case.transcript);
            let mut decision_rows = Vec::new();
            let mut step_rows = Vec::new();
            for step in &case.step_rows {
                let detail = transcript_text_by_id
                    .get(step.transcript_call_entry_id.as_str())
                    .cloned()
                    .unwrap_or_else(|| {
                        String::from("the local Apple FM session issued one bounded tool call.")
                    });
                decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                    decision_index: decision_rows.len() as u16,
                    decision_source: String::from("apple_fm_assistant"),
                    decision_kind: String::from("tool_call"),
                    decision_ref: step.transcript_call_entry_id.clone(),
                    tool_name: Some(step.tool_name.clone()),
                    detail: detail.clone(),
                });
                step_rows.push(TassadarMultiPluginTraceStepRow {
                    step_index: step.step_index as u16,
                    decision_ref: step.transcript_call_entry_id.clone(),
                    result_ref: step.transcript_tool_entry_id.clone(),
                    tool_name: step.tool_name.clone(),
                    plugin_id: step.projected_result.plugin_id.clone(),
                    authoring_class_id: authoring_class_id_for_tool_name(step.tool_name.as_str())?,
                    origin_class_id: origin_class_id_for_tool_name(step.tool_name.as_str())?,
                    arguments: step.arguments.clone(),
                    projected_result: step.projected_result.clone(),
                    detail,
                });
            }
            let final_message_text = case
                .transcript
                .transcript
                .entries
                .last()
                .and_then(transcript_entry_text)
                .unwrap_or_else(|| case.detail.clone());
            decision_rows.push(TassadarMultiPluginControllerDecisionRow {
                decision_index: decision_rows.len() as u16,
                decision_source: String::from("apple_fm_assistant"),
                decision_kind: String::from("final_message"),
                decision_ref: format!("apple_fm.final_message.{}", case.case_id),
                tool_name: None,
                detail: final_message_text.clone(),
            });
            Ok(finalize_trace_record(TassadarMultiPluginTraceRecord {
                record_schema_id: String::from(TRACE_RECORD_SCHEMA_ID),
                record_id: format!(
                    "trace_record.{}.{}.v1",
                    workflow_case_id, LANE_APPLE_FM_SESSION
                ),
                workflow_case_id: String::from(workflow_case_id),
                source_case_id: case.case_id.clone(),
                lane_id: String::from(LANE_APPLE_FM_SESSION),
                source_bundle_ref: String::from(APPLE_FM_PLUGIN_SESSION_BUNDLE_REF),
                source_bundle_id: bundle.bundle_id.clone(),
                source_bundle_digest: bundle.bundle_digest.clone(),
                directive_text: case.directive.clone(),
                admissible_tool_names,
                projected_tool_schema_rows,
                controller_decision_rows: decision_rows,
                step_rows,
                normalized_stop_condition_id: String::from(normalized_stop_condition_id),
                source_stop_condition_id: format!(
                    "apple_fm.session.{}",
                    if case.typed_refusal_preserved {
                        "typed_refusal_completed"
                    } else {
                        "completed"
                    }
                ),
                final_message_text,
                typed_refusal_preserved: case.typed_refusal_preserved,
                detail: case.detail.clone(),
                record_digest: String::new(),
            }))
        })
        .collect()
}

fn apple_fm_transcript_text_by_id(transcript: &AppleFmTranscript) -> BTreeMap<String, String> {
    transcript
        .transcript
        .entries
        .iter()
        .filter_map(|entry| {
            entry
                .id
                .as_ref()
                .and_then(|id| transcript_entry_text(entry).map(|text| (id.clone(), text)))
        })
        .collect()
}

fn transcript_entry_text(entry: &AppleFmTranscriptEntry) -> Option<String> {
    entry
        .contents
        .iter()
        .find_map(|content| content.text.clone())
}

fn build_parity_matrix(
    bridge_bundle: &StarterPluginToolBridgeBundle,
    router_bundle: &RouterPilotBundle,
    apple_bundle: &AppleFmPilotBundle,
    trace_records: &[TassadarMultiPluginTraceRecord],
) -> Result<TassadarMultiPluginParityMatrix, TassadarMultiPluginTraceCorpusError> {
    let tool_schema_parity_rows =
        build_tool_schema_parity_rows(bridge_bundle, router_bundle, apple_bundle)?;
    let workflow_parity_rows = build_workflow_parity_rows(trace_records)?;
    let explicit_disagreement_count = workflow_parity_rows
        .iter()
        .map(|row| row.disagreement_rows.len() as u16)
        .sum();
    Ok(TassadarMultiPluginParityMatrix {
        schema_id: String::from(PARITY_MATRIX_SCHEMA_ID),
        tool_schema_parity_rows,
        workflow_parity_rows,
        explicit_disagreement_count,
    })
}

fn build_tool_schema_parity_rows(
    bridge_bundle: &StarterPluginToolBridgeBundle,
    router_bundle: &RouterPilotBundle,
    apple_bundle: &AppleFmPilotBundle,
) -> Result<Vec<TassadarMultiPluginToolSchemaParityRow>, TassadarMultiPluginTraceCorpusError> {
    let router_digests = router_bundle
        .tool_definition_rows
        .iter()
        .map(|row| (row.tool_name.as_str(), row.parameters_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    let apple_digests = apple_bundle
        .tool_definition_rows
        .iter()
        .map(|row| (row.tool_name.as_str(), row.arguments_schema_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut rows = bridge_bundle
        .projection_rows
        .iter()
        .filter(|row| {
            router_digests.contains_key(row.tool_name.as_str())
                && apple_digests.contains_key(row.tool_name.as_str())
        })
        .map(|row| {
            let router_digest = router_digests.get(row.tool_name.as_str()).ok_or_else(|| {
                TassadarMultiPluginTraceCorpusError::MissingToolDigest {
                    lane_id: String::from(LANE_ROUTER_RESPONSES),
                    tool_name: row.tool_name.clone(),
                }
            })?;
            let apple_digest = apple_digests.get(row.tool_name.as_str()).ok_or_else(|| {
                TassadarMultiPluginTraceCorpusError::MissingToolDigest {
                    lane_id: String::from(LANE_APPLE_FM_SESSION),
                    tool_name: row.tool_name.clone(),
                }
            })?;
            let stable_across_lanes = row.arguments_schema_digest == *router_digest
                && row.arguments_schema_digest == *apple_digest;
            Ok(TassadarMultiPluginToolSchemaParityRow {
                tool_name: row.tool_name.clone(),
                plugin_id: row.plugin_id.clone(),
                canonical_arguments_schema_digest: row.arguments_schema_digest.clone(),
                router_bundle_arguments_schema_digest: String::from(*router_digest),
                apple_fm_bundle_arguments_schema_digest: String::from(*apple_digest),
                stable_across_lanes,
                detail: if stable_across_lanes {
                    String::from(
                        "the shared bridge, router bundle, and Apple FM bundle all keep the same arguments-schema digest.",
                    )
                } else {
                    String::from(
                        "the committed controller bundles drift on tool-schema digest and must not be collapsed into synthetic agreement.",
                    )
                },
            })
        })
        .collect::<Result<Vec<_>, TassadarMultiPluginTraceCorpusError>>()?;
    rows.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
    Ok(rows)
}

fn build_workflow_parity_rows(
    trace_records: &[TassadarMultiPluginTraceRecord],
) -> Result<Vec<TassadarMultiPluginWorkflowParityRow>, TassadarMultiPluginTraceCorpusError> {
    let mut grouped = BTreeMap::<&str, Vec<&TassadarMultiPluginTraceRecord>>::new();
    for record in trace_records {
        grouped
            .entry(record.workflow_case_id.as_str())
            .or_default()
            .push(record);
    }
    let mut rows = Vec::new();
    for (workflow_case_id, group) in grouped {
        let expected_lanes = expected_lane_ids_for_workflow_case(workflow_case_id)?;
        if group.len() != expected_lanes.len()
            || expected_lanes.iter().any(|lane_id| {
                !group
                    .iter()
                    .any(|record| record.lane_id.as_str() == *lane_id)
            })
        {
            return Err(TassadarMultiPluginTraceCorpusError::MissingWorkflowLane {
                workflow_case_id: workflow_case_id.to_string(),
            });
        }
        let mut group = group;
        group.sort_by(|left, right| left.lane_id.cmp(&right.lane_id));
        let lane_record_ids = group
            .iter()
            .map(|record| (record.lane_id.clone(), record.record_id.clone()))
            .collect::<BTreeMap<_, _>>();
        let directive_digest_by_lane = group
            .iter()
            .map(|record| {
                (
                    record.lane_id.clone(),
                    stable_digest(b"tassadar_multi_plugin_directive|", &record.directive_text),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let source_stop_condition_id_by_lane = group
            .iter()
            .map(|record| {
                (
                    record.lane_id.clone(),
                    record.source_stop_condition_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let normalized_stop_condition_id = group[0].normalized_stop_condition_id.clone();
        let max_steps = group
            .iter()
            .map(|record| record.step_rows.len() as u16)
            .max()
            .unwrap_or(0);
        let mut step_parity_rows = Vec::new();
        let mut disagreement_rows = Vec::new();
        if distinct_count(directive_digest_by_lane.values()) > 1 {
            disagreement_rows.push(TassadarMultiPluginWorkflowDisagreementRow {
                workflow_case_id: workflow_case_id.to_string(),
                scope_kind: String::from("directive"),
                step_index: None,
                disagreement_kind: String::from("directive_text_drift"),
                detail: String::from(
                    "directive text differs across controller lanes and is retained explicitly.",
                ),
                lane_values: directive_digest_by_lane.clone(),
            });
        }
        if distinct_count(source_stop_condition_id_by_lane.values()) > 1 {
            disagreement_rows.push(TassadarMultiPluginWorkflowDisagreementRow {
                workflow_case_id: workflow_case_id.to_string(),
                scope_kind: String::from("stop_condition"),
                step_index: None,
                disagreement_kind: String::from("source_stop_condition_drift"),
                detail: String::from(
                    "source stop-condition identifiers differ across controller lanes and are retained explicitly.",
                ),
                lane_values: source_stop_condition_id_by_lane.clone(),
            });
        }
        for step_index in 0..max_steps {
            let mut lanes_present = Vec::new();
            let mut tool_name_by_lane = BTreeMap::new();
            let mut status_by_lane = BTreeMap::new();
            let mut arguments_digest_by_lane = BTreeMap::new();
            let mut payload_digest_by_lane = BTreeMap::new();
            let mut receipt_id_by_lane = BTreeMap::new();
            for record in &group {
                if let Some(step) = record
                    .step_rows
                    .iter()
                    .find(|row| row.step_index == step_index)
                {
                    lanes_present.push(record.lane_id.clone());
                    let _ =
                        tool_name_by_lane.insert(record.lane_id.clone(), step.tool_name.clone());
                    let _ = status_by_lane.insert(
                        record.lane_id.clone(),
                        invocation_status_label(step.projected_result.status),
                    );
                    let _ = arguments_digest_by_lane.insert(
                        record.lane_id.clone(),
                        stable_digest(b"tassadar_multi_plugin_step_arguments|", &step.arguments),
                    );
                    let _ = payload_digest_by_lane.insert(
                        record.lane_id.clone(),
                        stable_digest(
                            b"tassadar_multi_plugin_step_payload|",
                            &step.projected_result.structured_payload,
                        ),
                    );
                    let _ = receipt_id_by_lane.insert(
                        record.lane_id.clone(),
                        step.projected_result.plugin_receipt.receipt_id.clone(),
                    );
                }
            }
            let mut disagreement_reasons = Vec::new();
            if lanes_present.len() != group.len() {
                disagreement_reasons.push(String::from("lane_missing"));
            }
            if distinct_count(tool_name_by_lane.values()) > 1 {
                disagreement_reasons.push(String::from("tool_name_drift"));
            }
            if distinct_count(status_by_lane.values()) > 1 {
                disagreement_reasons.push(String::from("status_drift"));
            }
            if distinct_count(arguments_digest_by_lane.values()) > 1 {
                disagreement_reasons.push(String::from("arguments_drift"));
            }
            if distinct_count(payload_digest_by_lane.values()) > 1 {
                disagreement_reasons.push(String::from("payload_drift"));
            }
            if distinct_count(receipt_id_by_lane.values()) > 1 {
                disagreement_reasons.push(String::from("receipt_identity_drift"));
            }
            let agreement_class = if disagreement_reasons.is_empty() {
                "exact_match"
            } else if disagreement_reasons.iter().all(|reason| {
                matches!(
                    reason.as_str(),
                    "arguments_drift" | "payload_drift" | "receipt_identity_drift"
                )
            }) {
                "digest_drift"
            } else {
                "lane_disagreement"
            };
            if !disagreement_reasons.is_empty() {
                for reason in &disagreement_reasons {
                    disagreement_rows.push(TassadarMultiPluginWorkflowDisagreementRow {
                        workflow_case_id: workflow_case_id.to_string(),
                        scope_kind: String::from("step"),
                        step_index: Some(step_index),
                        disagreement_kind: reason.clone(),
                        detail: format!(
                            "step {step_index} keeps `{reason}` explicit instead of smoothing it into synthetic consensus."
                        ),
                        lane_values: match reason.as_str() {
                            "tool_name_drift" => tool_name_by_lane.clone(),
                            "status_drift" => status_by_lane.clone(),
                            "arguments_drift" => arguments_digest_by_lane.clone(),
                            "payload_drift" => payload_digest_by_lane.clone(),
                            "receipt_identity_drift" => receipt_id_by_lane.clone(),
                            _ => BTreeMap::new(),
                        },
                    });
                }
            }
            step_parity_rows.push(TassadarMultiPluginWorkflowStepParityRow {
                step_index,
                lanes_present,
                tool_name_by_lane,
                status_by_lane,
                arguments_digest_by_lane,
                payload_digest_by_lane,
                receipt_id_by_lane,
                agreement_class: String::from(agreement_class),
                disagreement_reasons,
            });
        }
        rows.push(TassadarMultiPluginWorkflowParityRow {
            workflow_case_id: workflow_case_id.to_string(),
            lane_record_ids,
            directive_digest_by_lane,
            normalized_stop_condition_id,
            source_stop_condition_id_by_lane,
            step_parity_rows,
            disagreement_rows,
        });
    }
    rows.sort_by(|left, right| left.workflow_case_id.cmp(&right.workflow_case_id));
    Ok(rows)
}

fn build_bootstrap_contract(
    trace_records: &[TassadarMultiPluginTraceRecord],
    parity_matrix: &TassadarMultiPluginParityMatrix,
) -> TassadarMultiPluginTrainingBootstrapContract {
    let admitted_controller_lanes = trace_records
        .iter()
        .map(|record| record.lane_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let admitted_workflow_case_ids = trace_records
        .iter()
        .map(|record| record.workflow_case_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    TassadarMultiPluginTrainingBootstrapContract {
        contract_id: String::from(BOOTSTRAP_CONTRACT_ID),
        target_deferred_issue_id: String::from("TAS-204"),
        trace_record_schema_id: String::from(TRACE_RECORD_SCHEMA_ID),
        admitted_controller_lanes,
        admitted_workflow_case_ids,
        requires_receipt_identity: true,
        preserves_disagreement_rows: true,
        bootstrap_ready: trace_records.len() == 7
            && parity_matrix.workflow_parity_rows.len() == 3
            && parity_matrix.explicit_disagreement_count > 0,
        bootstrap_boundary: String::from(
            "later weighted-controller and training-derivation work may bootstrap from these records only if they preserve plugin receipt identity, keep disagreement rows explicit, and keep the single-lane guest-artifact proof separate from the three-lane host-native parity families instead of collapsing them into a false proof of controller closure.",
        ),
    }
}

fn finalize_trace_record(
    mut record: TassadarMultiPluginTraceRecord,
) -> TassadarMultiPluginTraceRecord {
    record.record_digest = stable_digest(b"tassadar_multi_plugin_trace_record|", &record);
    record
}

fn normalized_workflow_case_id(
    source_case_id: &str,
) -> Result<&'static str, TassadarMultiPluginTraceCorpusError> {
    match source_case_id {
        "web_content_intake_success"
        | "router_plugin_tool_loop_success"
        | "apple_fm_plugin_session_success" => Ok(WORKFLOW_WEB_CONTENT_SUCCESS),
        "web_content_intake_fetch_refusal"
        | "router_plugin_tool_loop_fetch_refusal"
        | "apple_fm_plugin_session_fetch_refusal" => Ok(WORKFLOW_FETCH_REFUSAL),
        "guest_artifact_echo_success" => Ok(WORKFLOW_GUEST_ARTIFACT_SUCCESS),
        _ => Err(TassadarMultiPluginTraceCorpusError::UnknownWorkflowCase {
            case_id: source_case_id.to_string(),
        }),
    }
}

fn normalized_stop_condition_id(
    source_case_id: &str,
) -> Result<&'static str, TassadarMultiPluginTraceCorpusError> {
    match normalized_workflow_case_id(source_case_id)? {
        WORKFLOW_WEB_CONTENT_SUCCESS => Ok(STOP_COMPLETED_SUCCESS),
        WORKFLOW_FETCH_REFUSAL => Ok(STOP_TYPED_REFUSAL),
        WORKFLOW_GUEST_ARTIFACT_SUCCESS => Ok(STOP_GUEST_ARTIFACT_COMPLETED),
        _ => Err(TassadarMultiPluginTraceCorpusError::UnknownWorkflowCase {
            case_id: source_case_id.to_string(),
        }),
    }
}

fn expected_lane_ids_for_workflow_case(
    workflow_case_id: &str,
) -> Result<&'static [&'static str], TassadarMultiPluginTraceCorpusError> {
    match workflow_case_id {
        WORKFLOW_WEB_CONTENT_SUCCESS | WORKFLOW_FETCH_REFUSAL => Ok(&[
            LANE_APPLE_FM_SESSION,
            LANE_DETERMINISTIC_WORKFLOW,
            LANE_ROUTER_RESPONSES,
        ]),
        WORKFLOW_GUEST_ARTIFACT_SUCCESS => Ok(&[LANE_DETERMINISTIC_WORKFLOW]),
        _ => Err(TassadarMultiPluginTraceCorpusError::UnknownWorkflowCase {
            case_id: String::from(workflow_case_id),
        }),
    }
}

fn authoring_class_id_for_tool_name(
    tool_name: &str,
) -> Result<String, TassadarMultiPluginTraceCorpusError> {
    let registration = starter_plugin_registration_by_tool_name(tool_name).ok_or_else(|| {
        TassadarMultiPluginTraceCorpusError::MissingRuntimeRegistration {
            tool_name: String::from(tool_name),
        }
    })?;
    Ok(authoring_class_id(registration.authoring_class))
}

fn origin_class_id_for_tool_name(
    tool_name: &str,
) -> Result<String, TassadarMultiPluginTraceCorpusError> {
    let registration = starter_plugin_registration_by_tool_name(tool_name).ok_or_else(|| {
        TassadarMultiPluginTraceCorpusError::MissingRuntimeRegistration {
            tool_name: String::from(tool_name),
        }
    })?;
    Ok(origin_class_id(registration.origin_class))
}

fn authoring_class_id(authoring_class: StarterPluginAuthoringClass) -> String {
    String::from(match authoring_class {
        StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic => {
            "capability_free_local_deterministic"
        }
        StarterPluginAuthoringClass::NetworkedReadOnly => "networked_read_only",
        StarterPluginAuthoringClass::GuestArtifactDigestBound => "guest_artifact_digest_bound",
    })
}

fn origin_class_id(origin_class: StarterPluginOriginClass) -> String {
    String::from(match origin_class {
        StarterPluginOriginClass::OperatorBuiltin => "operator_builtin",
        StarterPluginOriginClass::UserAdded => "user_added",
    })
}

fn invocation_status_label(status: psionic_runtime::StarterPluginInvocationStatus) -> String {
    match status {
        psionic_runtime::StarterPluginInvocationStatus::Success => String::from("success"),
        psionic_runtime::StarterPluginInvocationStatus::Refusal => String::from("refusal"),
    }
}

fn distinct_count<'a>(values: impl Iterator<Item = &'a String>) -> usize {
    values.collect::<BTreeSet<_>>().len()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-data crate dir")
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarMultiPluginTraceCorpusError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarMultiPluginTraceCorpusError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarMultiPluginTraceCorpusError::Decode {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_multi_plugin_trace_corpus_bundle,
        load_tassadar_multi_plugin_trace_corpus_bundle,
        tassadar_multi_plugin_trace_corpus_bundle_path,
        write_tassadar_multi_plugin_trace_corpus_bundle, APPLE_FM_PLUGIN_SESSION_BUNDLE_REF,
        BOOTSTRAP_CONTRACT_ID, PARITY_MATRIX_SCHEMA_ID, ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF,
        TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_BUNDLE_REF, TRACE_RECORD_SCHEMA_ID,
        WORKFLOW_FETCH_REFUSAL, WORKFLOW_GUEST_ARTIFACT_SUCCESS, WORKFLOW_WEB_CONTENT_SUCCESS,
    };

    #[test]
    fn multi_plugin_trace_corpus_bundle_covers_all_lanes_and_workflows(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_multi_plugin_trace_corpus_bundle()?;

        assert_eq!(bundle.source_bundle_rows.len(), 3);
        assert_eq!(bundle.projected_tool_schema_rows.len(), 6);
        assert_eq!(bundle.trace_records.len(), 7);
        assert_eq!(bundle.class_expansion_rows.len(), 2);
        assert_eq!(bundle.parity_matrix.schema_id, PARITY_MATRIX_SCHEMA_ID);
        assert_eq!(bundle.parity_matrix.workflow_parity_rows.len(), 3);
        assert_eq!(bundle.bootstrap_contract.contract_id, BOOTSTRAP_CONTRACT_ID);
        assert!(bundle.bootstrap_contract.bootstrap_ready);
        assert_eq!(
            bundle.bootstrap_contract.trace_record_schema_id,
            TRACE_RECORD_SCHEMA_ID
        );
        assert!(bundle
            .bootstrap_contract
            .admitted_workflow_case_ids
            .contains(&String::from(WORKFLOW_WEB_CONTENT_SUCCESS)));
        assert!(bundle
            .bootstrap_contract
            .admitted_workflow_case_ids
            .contains(&String::from(WORKFLOW_FETCH_REFUSAL)));
        assert!(bundle
            .bootstrap_contract
            .admitted_workflow_case_ids
            .contains(&String::from(WORKFLOW_GUEST_ARTIFACT_SUCCESS)));
        assert!(bundle
            .source_bundle_rows
            .iter()
            .any(|row| { row.bundle_ref == ROUTER_PLUGIN_TOOL_LOOP_BUNDLE_REF }));
        assert!(bundle
            .source_bundle_rows
            .iter()
            .any(|row| { row.bundle_ref == APPLE_FM_PLUGIN_SESSION_BUNDLE_REF }));
        assert!(bundle
            .class_expansion_rows
            .iter()
            .any(|row| row.authoring_class_id == "networked_read_only"
                && row.workflow_case_ids
                    == vec![
                        String::from(WORKFLOW_FETCH_REFUSAL),
                        String::from(WORKFLOW_WEB_CONTENT_SUCCESS)
                    ]));
        assert!(bundle
            .class_expansion_rows
            .iter()
            .any(
                |row| row.authoring_class_id == "guest_artifact_digest_bound"
                    && row.workflow_case_ids == vec![String::from(WORKFLOW_GUEST_ARTIFACT_SUCCESS)]
            ));
        Ok(())
    }

    #[test]
    fn multi_plugin_trace_corpus_bundle_keeps_disagreement_rows_explicit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_multi_plugin_trace_corpus_bundle()?;
        let success = bundle
            .parity_matrix
            .workflow_parity_rows
            .iter()
            .find(|row| row.workflow_case_id == WORKFLOW_WEB_CONTENT_SUCCESS)
            .expect("success workflow");

        assert!(success
            .disagreement_rows
            .iter()
            .any(|row| row.disagreement_kind == "directive_text_drift"));
        assert!(success.step_parity_rows.iter().any(|row| {
            row.agreement_class == "digest_drift"
                && row
                    .disagreement_reasons
                    .contains(&String::from("payload_drift"))
        }));
        let guest = bundle
            .parity_matrix
            .workflow_parity_rows
            .iter()
            .find(|row| row.workflow_case_id == WORKFLOW_GUEST_ARTIFACT_SUCCESS)
            .expect("guest workflow");
        assert_eq!(
            guest.lane_record_ids.keys().cloned().collect::<Vec<_>>(),
            vec![String::from("deterministic_workflow")]
        );
        assert!(guest.disagreement_rows.is_empty());
        assert_eq!(guest.step_parity_rows.len(), 1);
        let guest_step = bundle
            .trace_records
            .iter()
            .find(|record| record.workflow_case_id == WORKFLOW_GUEST_ARTIFACT_SUCCESS)
            .expect("guest trace record")
            .step_rows
            .first()
            .expect("guest step");
        assert_eq!(guest_step.authoring_class_id, "guest_artifact_digest_bound");
        assert_eq!(guest_step.origin_class_id, "user_added");
        Ok(())
    }

    #[test]
    fn multi_plugin_trace_corpus_bundle_writes_and_loads() -> Result<(), Box<dyn std::error::Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let output_path = tempdir
            .path()
            .join("tassadar_multi_plugin_trace_corpus_bundle.json");
        let written = write_tassadar_multi_plugin_trace_corpus_bundle(&output_path)?;
        let loaded = load_tassadar_multi_plugin_trace_corpus_bundle(&output_path)?;

        assert_eq!(written, loaded);
        Ok(())
    }

    #[test]
    fn multi_plugin_trace_corpus_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_multi_plugin_trace_corpus_bundle_path();
        assert!(path.ends_with(TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_BUNDLE_REF));
    }
}
