use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    invoke_psion_plugin_guest_artifact_json_packet, invoke_text_stats_json_packet,
    reference_psion_plugin_guest_artifact_bytes, reference_psion_plugin_guest_artifact_manifest,
    weighted_controller_admissible_user_added_starter_plugin_registrations,
    StarterPluginAuthoringClass, StarterPluginOriginClass, StarterPluginRegistration,
    TextStatsConfig, TextStatsRequest, PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID,
    STARTER_PLUGIN_GUEST_ECHO_ID, STARTER_PLUGIN_TEXT_STATS_ID,
    STARTER_PLUGIN_TEXT_STATS_INPUT_SCHEMA_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
};

pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_v1/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle.json";
pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_RUN_ROOT_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_v1";
const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID: &str =
    "tassadar.weighted_plugin.result_binding_contract.v1";
const TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID: &str =
    "tassadar.weighted_plugin.model_loop_return_profile.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID: &str =
    "tassadar.weighted_plugin.controller_trace_contract.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID: &str =
    "tassadar.weighted_plugin.control_trace_profile.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID: &str =
    "tassadar.weighted_plugin.controller_determinism_profile.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_EQUIVALENT_CHOICE_RELATION_ID: &str =
    "singleton_exact_control_trace.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_FAILURE_SEMANTICS_LATTICE_ID: &str =
    "tassadar.post_article.failure_semantics_lattice.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_TIME_SEMANTICS_CONTRACT_ID: &str =
    "tassadar.post_article.control_time_semantics.v1";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_INFORMATION_BOUNDARY_ID: &str =
    "tassadar.post_article.control_information_boundary.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControllerCaseOutcome {
    StopAfterSuccess,
    RetryThenStop,
    TerminalRefusalThenStop,
    ContinueAcrossPluginsThenStop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControlTokenKind {
    PluginSelect,
    ExportSelect,
    PacketEncode,
    InvocationCommit,
    ResultAccept,
    ResultRefusal,
    Continue,
    Retry,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerCaseRow {
    pub case_id: String,
    pub case_outcome: TassadarPostArticleWeightedPluginControllerCaseOutcome,
    pub model_version_id: String,
    pub plugin_chain_ids: Vec<String>,
    pub result_binding_case_ids: Vec<String>,
    pub determinism_class_id: String,
    pub sampling_policy_id: String,
    pub temperature_policy_id: String,
    pub randomness_control_id: String,
    pub external_signal_boundary_id: String,
    pub model_selects_plugin: bool,
    pub model_selects_export: bool,
    pub packet_arguments_model_owned: bool,
    pub sequencing_model_owned: bool,
    pub retry_model_owned: bool,
    pub stop_model_owned: bool,
    pub typed_refusal_returned_to_model_loop: bool,
    pub host_validates_only: bool,
    pub host_executes_only: bool,
    pub replay_stable: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControlTraceRow {
    pub case_id: String,
    pub step_index: u16,
    pub control_token_kind: TassadarPostArticleWeightedPluginControlTokenKind,
    pub decision_owner_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_schema_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_binding_case_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_reason_id: Option<String>,
    pub previous_control_state_digest: String,
    pub next_control_state_id: String,
    pub next_control_state_digest: String,
    pub equivalent_choice_relation_id: String,
    pub failure_semantics_lattice_id: String,
    pub time_semantics_contract_id: String,
    pub information_boundary_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginHostNegativeRow {
    pub check_id: String,
    pub negative_class_id: String,
    pub attack_family_id: String,
    pub green: bool,
    pub typed_refusal_reason_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginStarterPluginAdmissionRow {
    pub plugin_id: String,
    pub tool_name: String,
    pub authoring_class_id: String,
    pub origin_class_id: String,
    pub catalog_entry_id: String,
    pub derived_from_shared_registration: bool,
    pub derived_from_catalog_exposure: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub host_owned_runtime_api_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub control_trace_contract_id: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub equivalent_choice_relation_id: String,
    pub failure_semantics_lattice_id: String,
    pub time_semantics_contract_id: String,
    pub information_boundary_id: String,
    pub starter_plugin_admission_rows:
        Vec<TassadarPostArticleWeightedPluginStarterPluginAdmissionRow>,
    pub controller_case_rows: Vec<TassadarPostArticleWeightedPluginControllerCaseRow>,
    pub control_trace_rows: Vec<TassadarPostArticleWeightedPluginControlTraceRow>,
    pub host_negative_rows: Vec<TassadarPostArticleWeightedPluginHostNegativeRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError {
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle(
) -> TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle {
    let starter_plugin_admission_rows =
        weighted_controller_admissible_user_added_starter_plugin_registrations()
            .into_iter()
            .map(starter_plugin_admission_row)
            .collect::<Vec<_>>();
    let text_stats_trace = weighted_text_stats_success_trace_fixture();
    let guest_echo_trace = weighted_guest_echo_success_trace_fixture();
    let controller_case_rows = vec![
        controller_case(
            "fetch_text_stop_after_success",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::StopAfterSuccess,
            &["plugin.http_fetch_text"],
            &["fetch_text_exact_binding"],
            true,
            "the model selects `plugin.http_fetch_text`, accepts the typed result packet, and stops without any host-authored continuation logic.",
        ),
        controller_case(
            "fetch_text_timeout_retry_then_stop",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::RetryThenStop,
            &["plugin.http_fetch_text"],
            &[
                "timeout_refusal_normalized_binding",
                "fetch_text_exact_binding",
            ],
            true,
            "the model sees a typed timeout refusal, emits one explicit retry, re-invokes the same plugin under the same deterministic control profile, and then stops after success.",
        ),
        controller_case(
            "html_extract_schema_refusal_then_stop",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::TerminalRefusalThenStop,
            &["plugin.html_extract"],
            &["html_extract_version_skew_blocked"],
            false,
            "the model sees a terminal schema-version-skew refusal from `plugin.html_extract` and stops rather than relying on hidden host repair.",
        ),
        controller_case(
            "fetch_then_extract_continue_then_stop",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::ContinueAcrossPluginsThenStop,
            &["plugin.http_fetch_text", "plugin.html_extract"],
            &[
                "fetch_text_exact_binding",
                "html_extract_backward_compatible_binding",
            ],
            false,
            "the model continues from `plugin.http_fetch_text` into `plugin.html_extract`, accepts both typed result packets, and then stops on the same deterministic controller trace.",
        ),
        controller_case(
            "text_stats_stop_after_success",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::StopAfterSuccess,
            &[STARTER_PLUGIN_TEXT_STATS_ID],
            &["text_stats_exact_binding"],
            true,
            "the model selects the user-added capability-free `plugin.text.stats` entry admitted from the shared starter-plugin registration and catalog path, accepts the typed result packet, and stops without host-authored continuation.",
        ),
        controller_case(
            "guest_echo_stop_after_success",
            TassadarPostArticleWeightedPluginControllerCaseOutcome::StopAfterSuccess,
            &[STARTER_PLUGIN_GUEST_ECHO_ID],
            &["guest_echo_exact_binding"],
            true,
            "the model selects the user-added digest-bound guest-artifact starter plugin admitted from the shared registration and catalog path, accepts the typed result packet, and stops without host-authored continuation.",
        ),
    ];

    let control_trace_rows = vec![
        trace_row(
            "fetch_text_stop_after_success",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            None,
            None,
            None,
            None,
            None,
            None,
            "select fetch-text plugin under the declared deterministic controller trace.",
        ),
        trace_row(
            "fetch_text_stop_after_success",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "select the declared packet export instead of falling back to a host default export.",
        ),
        trace_row(
            "fetch_text_stop_after_success",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_stop_after_success",
                "plugin.http.fetch_text.input.v1",
            )),
            None,
            None,
            None,
            "encode the packet arguments from model outputs under `packet.v1` without host-authored argument synthesis.",
        ),
        trace_row(
            "fetch_text_stop_after_success",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_stop_after_success",
                "plugin.http.fetch_text.input.v1",
            )),
            Some("receipt.fetch_text_stop_after_success.v1"),
            None,
            None,
            "the host validates and executes the declared call but does not choose the plugin or next step.",
        ),
        trace_row(
            "fetch_text_stop_after_success",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.fetch_text_stop_after_success.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the host returns one typed result packet to the model loop under the result-binding contract.",
        ),
        trace_row(
            "fetch_text_stop_after_success",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.fetch_text_stop_after_success.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the model stops after accepting the fetch-text result instead of relying on host queue or heuristic completion.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            None,
            None,
            None,
            None,
            None,
            None,
            "the retrying controller still begins with an explicit model-owned plugin selection.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the retrying controller keeps export choice explicit and model-owned.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_timeout_retry_then_stop.first",
                "plugin.http.fetch_text.input.v1",
            )),
            None,
            None,
            None,
            "the first attempt packet remains model-authored under the declared ABI.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_timeout_retry_then_stop.first",
                "plugin.http.fetch_text.input.v1",
            )),
            Some("receipt.fetch_text_timeout_retry_then_stop.first.v1"),
            None,
            None,
            "the host executes the first declared attempt under bounded runtime control.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultRefusal,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.fetch_text_timeout_retry_then_stop.first.v1"),
            Some("timeout_refusal_normalized_binding"),
            Some("runtime_timeout"),
            "the host returns one typed timeout refusal packet to the model loop instead of auto-retrying.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Retry,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.fetch_text_timeout_retry_then_stop.first.v1"),
            Some("timeout_refusal_normalized_binding"),
            Some("runtime_timeout"),
            "the model emits one explicit retry decision after seeing the typed timeout refusal.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            6,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_timeout_retry_then_stop.second",
                "plugin.http.fetch_text.input.v1",
            )),
            None,
            None,
            None,
            "the retried call uses a second explicit packet encoding rather than a hidden host replay mutation.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            7,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_text_timeout_retry_then_stop.second",
                "plugin.http.fetch_text.input.v1",
            )),
            Some("receipt.fetch_text_timeout_retry_then_stop.second.v1"),
            None,
            None,
            "the host executes the second declared attempt without changing the plan.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            8,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.fetch_text_timeout_retry_then_stop.second.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the host returns one success packet after the explicit retry path.",
        ),
        trace_row(
            "fetch_text_timeout_retry_then_stop",
            9,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.fetch_text_timeout_retry_then_stop.second.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the model stops after the successful retry instead of relying on host completion policy.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some("plugin.html_extract"),
            None,
            None,
            None,
            None,
            None,
            None,
            "the model selects the html-extract plugin explicitly.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the model selects the declared packet export explicitly.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            Some("plugin.html.extract.input.v1"),
            Some(packet_digest(
                "html_extract_schema_refusal_then_stop",
                "plugin.html.extract.input.v1",
            )),
            None,
            None,
            None,
            "the model encodes html-extract arguments under the canonical packet ABI.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            Some("plugin.html.extract.input.v1"),
            Some(packet_digest(
                "html_extract_schema_refusal_then_stop",
                "plugin.html.extract.input.v1",
            )),
            Some("receipt.html_extract_schema_refusal_then_stop.v1"),
            None,
            None,
            "the host validates the manifest and executes the declared html-extract call.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultRefusal,
            "host_runtime",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.html_extract_schema_refusal_then_stop.v1"),
            Some("html_extract_version_skew_blocked"),
            Some("model_plugin_schema_version_skew"),
            "the host returns one typed schema-version-skew refusal packet to the model loop.",
        ),
        trace_row(
            "html_extract_schema_refusal_then_stop",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.html_extract_schema_refusal_then_stop.v1"),
            Some("html_extract_version_skew_blocked"),
            Some("model_plugin_schema_version_skew"),
            "the model stops on the terminal schema refusal instead of asking the host to repair or redirect the call.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            None,
            None,
            None,
            None,
            None,
            None,
            "the model selects fetch-text as the first step in the two-plugin chain.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the model selects the first export explicitly.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_then_extract_continue_then_stop.fetch",
                "plugin.http.fetch_text.input.v1",
            )),
            None,
            None,
            None,
            "the model encodes the first packet for fetch-text.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            Some("plugin.http.fetch_text.input.v1"),
            Some(packet_digest(
                "fetch_then_extract_continue_then_stop.fetch",
                "plugin.http.fetch_text.input.v1",
            )),
            Some("receipt.fetch_then_extract_continue_then_stop.fetch.v1"),
            None,
            None,
            "the host executes the fetch-text call under the declared limits.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some("plugin.http_fetch_text"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.fetch_then_extract_continue_then_stop.fetch.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the host returns one typed fetch-text result packet to the model loop.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Continue,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.fetch_then_extract_continue_then_stop.fetch.v1"),
            Some("fetch_text_exact_binding"),
            None,
            "the model emits one explicit continue decision into html-extract instead of leaving chaining to the host.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            6,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some("plugin.html_extract"),
            None,
            None,
            None,
            None,
            None,
            None,
            "the model selects html-extract as the second step in the chain.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            7,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the model keeps second-step export choice explicit.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            8,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            Some("plugin.html.extract.input.v1"),
            Some(packet_digest(
                "fetch_then_extract_continue_then_stop.extract",
                "plugin.html.extract.input.v1",
            )),
            None,
            None,
            None,
            "the model encodes the second packet from the first result packet under the declared result-binding contract.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            9,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            Some("plugin.html.extract.input.v1"),
            Some(packet_digest(
                "fetch_then_extract_continue_then_stop.extract",
                "plugin.html.extract.input.v1",
            )),
            Some("receipt.fetch_then_extract_continue_then_stop.extract.v1"),
            None,
            None,
            "the host executes the second declared call without injecting a hidden planner step.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            10,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some("plugin.html_extract"),
            Some("handle_packet"),
            None,
            None,
            Some("receipt.fetch_then_extract_continue_then_stop.extract.v1"),
            Some("html_extract_backward_compatible_binding"),
            None,
            "the host returns one typed html-extract result packet to the model loop.",
        ),
        trace_row(
            "fetch_then_extract_continue_then_stop",
            11,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some("receipt.fetch_then_extract_continue_then_stop.extract.v1"),
            Some("html_extract_backward_compatible_binding"),
            None,
            "the model stops after the second accepted result instead of relying on hidden host workflow completion.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some(STARTER_PLUGIN_TEXT_STATS_ID),
            None,
            None,
            None,
            None,
            None,
            None,
            "the model selects the admitted user-added text-stats plugin from the shared starter-plugin registry instead of a host-curated one-off whitelist.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some(STARTER_PLUGIN_TEXT_STATS_ID),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the model keeps export choice explicit on the user-added plugin path.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some(STARTER_PLUGIN_TEXT_STATS_ID),
            Some("handle_packet"),
            Some(STARTER_PLUGIN_TEXT_STATS_INPUT_SCHEMA_ID),
            Some(text_stats_trace.packet_digest.clone()),
            None,
            None,
            None,
            "the model encodes one packet-local text-stats request under the canonical packet ABI without host-authored argument synthesis.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some(STARTER_PLUGIN_TEXT_STATS_ID),
            Some("handle_packet"),
            Some(STARTER_PLUGIN_TEXT_STATS_INPUT_SCHEMA_ID),
            Some(text_stats_trace.packet_digest.clone()),
            Some(text_stats_trace.receipt_id.as_str()),
            None,
            None,
            "the host validates and executes the declared user-added text-stats call without changing the selected plugin or next step.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some(STARTER_PLUGIN_TEXT_STATS_ID),
            Some("handle_packet"),
            None,
            None,
            Some(text_stats_trace.receipt_id.as_str()),
            Some("text_stats_exact_binding"),
            None,
            "the host returns one typed text-stats result packet to the model loop under the shared result-binding contract.",
        ),
        trace_row(
            "text_stats_stop_after_success",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some(text_stats_trace.receipt_id.as_str()),
            Some("text_stats_exact_binding"),
            None,
            "the model stops after accepting the user-added text-stats result instead of relying on hidden host chaining.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            0,
            TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect,
            "model_weights",
            Some(STARTER_PLUGIN_GUEST_ECHO_ID),
            None,
            None,
            None,
            None,
            None,
            None,
            "the model selects the admitted digest-bound guest-artifact starter plugin from the shared starter-plugin registry instead of a host-curated one-off exception.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            1,
            TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect,
            "model_weights",
            Some(STARTER_PLUGIN_GUEST_ECHO_ID),
            Some("handle_packet"),
            None,
            None,
            None,
            None,
            None,
            "the model keeps export choice explicit on the guest-artifact starter path as well.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            2,
            TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode,
            "model_weights",
            Some(STARTER_PLUGIN_GUEST_ECHO_ID),
            Some("handle_packet"),
            Some(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID),
            Some(guest_echo_trace.packet_digest.clone()),
            None,
            None,
            None,
            "the model encodes one bounded guest-artifact packet under the canonical packet ABI without host-authored argument synthesis.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            3,
            TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit,
            "host_runtime",
            Some(STARTER_PLUGIN_GUEST_ECHO_ID),
            Some("handle_packet"),
            Some(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID),
            Some(guest_echo_trace.packet_digest.clone()),
            Some(guest_echo_trace.receipt_id.as_str()),
            None,
            None,
            "the host validates and executes the declared guest-artifact call without changing the selected plugin or next step.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            4,
            TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept,
            "host_runtime",
            Some(STARTER_PLUGIN_GUEST_ECHO_ID),
            Some("handle_packet"),
            None,
            None,
            Some(guest_echo_trace.receipt_id.as_str()),
            Some("guest_echo_exact_binding"),
            None,
            "the host returns one typed guest-artifact result packet to the model loop under the shared result-binding contract.",
        ),
        trace_row(
            "guest_echo_stop_after_success",
            5,
            TassadarPostArticleWeightedPluginControlTokenKind::Stop,
            "model_weights",
            None,
            None,
            None,
            None,
            Some(guest_echo_trace.receipt_id.as_str()),
            Some("guest_echo_exact_binding"),
            None,
            "the model stops after accepting the guest-artifact result instead of relying on hidden host chaining.",
        ),
    ];

    let host_negative_rows = vec![
        negative_row(
            "hidden_host_side_sequencing_blocked",
            "hidden_host_side_sequencing",
            "planner_attack_blocked",
            "host_workflow_injection",
            "hidden host-side sequencing remains blocked instead of letting the runtime decide the next plugin step.",
        ),
        negative_row(
            "host_auto_retry_blocked",
            "host_auto_retry",
            "auto_retry_blocked",
            "host_retry_injection",
            "host auto-retry remains blocked so retry stays a model-owned decision.",
        ),
        negative_row(
            "fallback_export_selection_blocked",
            "fallback_export_selection",
            "fallback_export_selection_blocked",
            "export_fallback_injection",
            "fallback export selection remains blocked instead of redirecting to a host-preferred export.",
        ),
        negative_row(
            "heuristic_plugin_ranking_blocked",
            "heuristic_plugin_ranking",
            "heuristic_plugin_ranking_blocked",
            "ranking_attack",
            "heuristic plugin ranking remains blocked unless the ranking is explicit model-visible state.",
        ),
        negative_row(
            "schema_auto_repair_blocked",
            "schema_auto_repair",
            "schema_auto_repair_blocked",
            "schema_attack",
            "schema auto-repair remains blocked instead of repairing malformed calls or results after the model emits them.",
        ),
        negative_row(
            "cached_result_substitution_blocked",
            "cached_result_substitution",
            "cached_result_substitution_blocked",
            "cache_attack",
            "cached result substitution remains blocked instead of turning cache hits into hidden planner outcomes.",
        ),
        negative_row(
            "candidate_precomputation_blocked",
            "candidate_precomputation",
            "candidate_precomputation_blocked",
            "precomputation_attack",
            "candidate precomputation remains blocked instead of narrowing the controller trace before the model commits to one call.",
        ),
        negative_row(
            "hidden_topk_filtering_blocked",
            "hidden_topk_filtering",
            "hidden_topk_filtering_blocked",
            "ranking_attack",
            "hidden top-k filtering remains blocked instead of shrinking the model-visible choice set.",
        ),
        negative_row(
            "helper_substitution_blocked",
            "helper_substitution",
            "helper_substitution_blocked",
            "helper_attack",
            "helper substitution remains blocked instead of replacing the declared plugin with a nearby helper surface.",
        ),
        negative_row(
            "runtime_learning_or_policy_drift_blocked",
            "runtime_learning_or_policy_drift",
            "runtime_learning_or_policy_drift_blocked",
            "adaptation_attack",
            "runtime learning or policy drift remains blocked instead of letting the host gradually become the planner.",
        ),
    ];

    let mut bundle =
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle {
            schema_version: 1,
            bundle_id: String::from(
                "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.runtime_bundle.v1",
            ),
            host_owned_runtime_api_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
            ),
            invocation_receipt_profile_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
            ),
            result_binding_contract_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
            ),
            model_loop_return_profile_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
            ),
            control_trace_contract_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID,
            ),
            control_trace_profile_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID,
            ),
            determinism_profile_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
            ),
            equivalent_choice_relation_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_EQUIVALENT_CHOICE_RELATION_ID,
            ),
            failure_semantics_lattice_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_FAILURE_SEMANTICS_LATTICE_ID,
            ),
            time_semantics_contract_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_TIME_SEMANTICS_CONTRACT_ID,
            ),
            information_boundary_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_INFORMATION_BOUNDARY_ID,
            ),
            starter_plugin_admission_rows,
            controller_case_rows,
            control_trace_rows,
            host_negative_rows,
            claim_boundary: String::from(
                "this runtime bundle freezes the canonical post-article weighted plugin controller trace above the result-binding contract, host-owned runtime API, and invocation-receipt layer. It keeps plugin selection, export selection, packet encoding, multi-step continuation, retry, typed refusal return, stop conditions, and bounded user-added capability-free starter-plugin admission from the shared registration and catalog path machine-readable while making host execution-only steps explicit and keeping publication, served/public universality, and arbitrary software capability blocked.",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
    bundle.summary = format!(
        "Post-article weighted plugin controller bundle covers starter_plugin_admission_rows={}, controller_case_rows={}, control_trace_rows={}, host_negative_rows={}.",
        bundle.starter_plugin_admission_rows.len(),
        bundle.controller_case_rows.len(),
        bundle.control_trace_rows.len(),
        bundle.host_negative_rows.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
    )
}

pub fn write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn controller_case(
    case_id: &str,
    case_outcome: TassadarPostArticleWeightedPluginControllerCaseOutcome,
    plugin_chain_ids: &[&str],
    result_binding_case_ids: &[&str],
    retry_model_owned: bool,
    detail: &str,
) -> TassadarPostArticleWeightedPluginControllerCaseRow {
    TassadarPostArticleWeightedPluginControllerCaseRow {
        case_id: String::from(case_id),
        case_outcome,
        model_version_id: String::from("tassadar.weighted_plugin_controller.schema_set.v1"),
        plugin_chain_ids: plugin_chain_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        result_binding_case_ids: result_binding_case_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        determinism_class_id: String::from("strict_deterministic"),
        sampling_policy_id: String::from("sampling_policy.greedy_single_path.v1"),
        temperature_policy_id: String::from("temperature.fixed_zero.v1"),
        randomness_control_id: String::from("randomness.disallowed.v1"),
        external_signal_boundary_id: String::from(
            "tassadar.weighted_plugin.external_signal_boundary.v1",
        ),
        model_selects_plugin: true,
        model_selects_export: true,
        packet_arguments_model_owned: true,
        sequencing_model_owned: true,
        retry_model_owned,
        stop_model_owned: true,
        typed_refusal_returned_to_model_loop: true,
        host_validates_only: true,
        host_executes_only: true,
        replay_stable: true,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn trace_row(
    case_id: &str,
    step_index: u16,
    control_token_kind: TassadarPostArticleWeightedPluginControlTokenKind,
    decision_owner_id: &str,
    plugin_id: Option<&str>,
    export_name: Option<&str>,
    packet_schema_id: Option<&str>,
    packet_digest: Option<String>,
    invocation_receipt_id: Option<&str>,
    result_binding_case_id: Option<&str>,
    typed_refusal_reason_id: Option<&str>,
    detail: &str,
) -> TassadarPostArticleWeightedPluginControlTraceRow {
    let previous_control_state_digest = synthetic_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_prev_state|",
        &(case_id, step_index.saturating_sub(1), decision_owner_id),
    );
    let next_control_state_id = format!("controller_state.{case_id}.{step_index}.v1");
    let next_control_state_digest = synthetic_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_next_state|",
        &(
            case_id,
            step_index,
            decision_owner_id,
            plugin_id,
            export_name,
            packet_schema_id,
            invocation_receipt_id,
            result_binding_case_id,
            typed_refusal_reason_id,
        ),
    );
    TassadarPostArticleWeightedPluginControlTraceRow {
        case_id: String::from(case_id),
        step_index,
        control_token_kind,
        decision_owner_id: String::from(decision_owner_id),
        plugin_id: plugin_id.map(String::from),
        export_name: export_name.map(String::from),
        packet_schema_id: packet_schema_id.map(String::from),
        packet_digest,
        invocation_receipt_id: invocation_receipt_id.map(String::from),
        result_binding_case_id: result_binding_case_id.map(String::from),
        typed_refusal_reason_id: typed_refusal_reason_id.map(String::from),
        previous_control_state_digest,
        next_control_state_id,
        next_control_state_digest,
        equivalent_choice_relation_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_EQUIVALENT_CHOICE_RELATION_ID,
        ),
        failure_semantics_lattice_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_FAILURE_SEMANTICS_LATTICE_ID,
        ),
        time_semantics_contract_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_TIME_SEMANTICS_CONTRACT_ID,
        ),
        information_boundary_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_INFORMATION_BOUNDARY_ID,
        ),
        detail: String::from(detail),
    }
}

fn negative_row(
    check_id: &str,
    negative_class_id: &str,
    typed_refusal_reason_id: &str,
    attack_family_id: &str,
    detail: &str,
) -> TassadarPostArticleWeightedPluginHostNegativeRow {
    TassadarPostArticleWeightedPluginHostNegativeRow {
        check_id: String::from(check_id),
        negative_class_id: String::from(negative_class_id),
        attack_family_id: String::from(attack_family_id),
        green: true,
        typed_refusal_reason_id: String::from(typed_refusal_reason_id),
        detail: String::from(detail),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WeightedTextStatsTraceFixture {
    packet_digest: String,
    receipt_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WeightedGuestEchoTraceFixture {
    packet_digest: String,
    receipt_id: String,
}

fn starter_plugin_admission_row(
    registration: &StarterPluginRegistration,
) -> TassadarPostArticleWeightedPluginStarterPluginAdmissionRow {
    let catalog = registration
        .catalog
        .expect("weighted controller admission requires catalog metadata");
    let authoring_class_id = authoring_class_id(registration.authoring_class);
    TassadarPostArticleWeightedPluginStarterPluginAdmissionRow {
        plugin_id: String::from(registration.plugin_id),
        tool_name: String::from(registration.tool_name),
        authoring_class_id: authoring_class_id.clone(),
        origin_class_id: origin_class_id(registration.origin_class),
        catalog_entry_id: String::from(catalog.catalog_entry_id),
        derived_from_shared_registration: true,
        derived_from_catalog_exposure: registration.catalog_exposed,
        detail: format!(
            "`{}` is admitted to the canonical weighted controller lane because it is a shared-registry user-added starter plugin with authoring class `{}`, bridge exposure, and catalog entry `{}`.",
            registration.plugin_id,
            authoring_class_id,
            catalog.catalog_entry_id,
        ),
    }
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

fn weighted_text_stats_success_trace_fixture() -> WeightedTextStatsTraceFixture {
    let packet = serde_json::to_vec(&TextStatsRequest {
        text: String::from(
            "Weighted controller runtime proof surface.\nA user-added starter plugin is admitted here.",
        ),
    })
    .expect("serialize weighted text-stats request");
    let outcome = invoke_text_stats_json_packet("json", &packet, &TextStatsConfig::default());
    assert!(
        outcome.refusal.is_none() && outcome.response.is_some(),
        "weighted text-stats trace fixture must stay successful",
    );
    WeightedTextStatsTraceFixture {
        packet_digest: outcome.receipt.input_packet_digest.clone(),
        receipt_id: outcome.receipt.receipt_id,
    }
}

fn weighted_guest_echo_success_trace_fixture() -> WeightedGuestEchoTraceFixture {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
    let packet = serde_json::to_vec(&serde_json::json!({
        "text": "Weighted guest-artifact controller proof."
    }))
    .expect("serialize weighted guest-artifact request");
    let outcome = invoke_psion_plugin_guest_artifact_json_packet(
        "weighted_guest_echo_success",
        &manifest,
        artifact_bytes.as_slice(),
        packet.as_slice(),
    )
    .expect("invoke guest-artifact trace fixture");
    assert!(
        outcome.projected_result.status == crate::StarterPluginInvocationStatus::Success,
        "weighted guest-artifact trace fixture must stay successful",
    );
    WeightedGuestEchoTraceFixture {
        packet_digest: outcome.projected_result.plugin_receipt.input_packet_digest.clone(),
        receipt_id: outcome.projected_result.plugin_receipt.receipt_id,
    }
}

fn packet_digest(case_id: &str, packet_schema_id: &str) -> String {
    synthetic_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_packet|",
        &(case_id, packet_schema_id),
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn synthetic_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    stable_digest(prefix, value)
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError>
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle,
        read_repo_json,
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle_path,
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle,
        TassadarPostArticleWeightedPluginControlTokenKind,
        TassadarPostArticleWeightedPluginControllerCaseOutcome,
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
    };

    #[test]
    fn post_article_weighted_plugin_controller_bundle_covers_declared_rows() {
        let bundle =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle();

        assert_eq!(
            bundle.bundle_id,
            "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.runtime_bundle.v1"
        );
        assert_eq!(bundle.starter_plugin_admission_rows.len(), 3);
        assert_eq!(bundle.controller_case_rows.len(), 6);
        assert_eq!(bundle.control_trace_rows.len(), 46);
        assert_eq!(bundle.host_negative_rows.len(), 10);
        assert!(bundle.starter_plugin_admission_rows.iter().any(|row| {
            row.plugin_id == "plugin.text.stats"
                && row.derived_from_shared_registration
                && row.derived_from_catalog_exposure
        }));
        assert!(bundle.starter_plugin_admission_rows.iter().any(|row| {
            row.plugin_id == "plugin.http.fetch_text"
                && row.authoring_class_id == "networked_read_only"
                && row.origin_class_id == "user_added"
                && row.derived_from_shared_registration
                && row.derived_from_catalog_exposure
        }));
        assert!(bundle.starter_plugin_admission_rows.iter().any(|row| {
            row.plugin_id == "plugin.example.echo_guest"
                && row.authoring_class_id == "guest_artifact_digest_bound"
                && row.origin_class_id == "user_added"
                && row.derived_from_shared_registration
                && row.derived_from_catalog_exposure
        }));
        assert!(bundle.controller_case_rows.iter().all(|row| {
            row.model_selects_plugin
                && row.model_selects_export
                && row.packet_arguments_model_owned
                && row.sequencing_model_owned
                && row.stop_model_owned
                && row.host_validates_only
                && row.host_executes_only
                && row.replay_stable
        }));
        assert!(bundle.control_trace_rows.iter().any(|row| {
            row.control_token_kind == TassadarPostArticleWeightedPluginControlTokenKind::Retry
                && row.decision_owner_id == "model_weights"
        }));
        assert!(bundle.control_trace_rows.iter().any(|row| {
            row.case_id == "text_stats_stop_after_success"
                && row.plugin_id.as_deref() == Some("plugin.text.stats")
        }));
        assert!(bundle.control_trace_rows.iter().any(|row| {
            row.case_id == "guest_echo_stop_after_success"
                && row.plugin_id.as_deref() == Some("plugin.example.echo_guest")
        }));
        assert!(bundle.controller_case_rows.iter().any(|row| {
            row.case_outcome
                == TassadarPostArticleWeightedPluginControllerCaseOutcome::ContinueAcrossPluginsThenStop
                && row.plugin_chain_ids.len() == 2
        }));
        assert!(bundle.host_negative_rows.iter().all(|row| row.green));
    }

    #[test]
    fn post_article_weighted_plugin_controller_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle();
        let committed: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle =
            read_repo_json(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle.json"
            )
        );
    }

    #[test]
    fn write_post_article_weighted_plugin_controller_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle.json",
        );
        let written =
            write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle(
                &output_path,
            )
            .expect("write bundle");
        let persisted: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
