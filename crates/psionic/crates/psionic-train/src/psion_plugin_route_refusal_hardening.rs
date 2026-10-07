use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::PsionPluginRouteLabel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_psion_plugin_guest_plugin_benchmark_bundle,
    build_psion_plugin_refusal_request_structure_benchmark_bundle,
    build_psion_plugin_result_interpretation_benchmark_bundle,
    record_psion_plugin_host_native_capability_matrix,
    record_psion_plugin_host_native_served_posture, record_psion_plugin_mixed_capability_matrix,
    record_psion_plugin_mixed_served_posture, run_psion_plugin_host_native_reference_lane,
    run_psion_plugin_mixed_reference_lane, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkMetricKind, PsionPluginBenchmarkTaskContract,
    PsionPluginGuestPluginBenchmarkBundle, PsionPluginGuestPluginBenchmarkError,
    PsionPluginHostNativeCapabilityMatrix, PsionPluginHostNativeCapabilityMatrixError,
    PsionPluginHostNativeClaimSurface, PsionPluginHostNativeReferenceLaneError,
    PsionPluginHostNativeReferenceRunBundle, PsionPluginHostNativeServedPosture,
    PsionPluginMixedCapabilityMatrix, PsionPluginMixedCapabilityMatrixError,
    PsionPluginMixedClaimSurface, PsionPluginMixedReferenceLaneError,
    PsionPluginMixedReferenceRunBundle, PsionPluginMixedServedPosture,
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkError,
    PsionPluginResultInterpretationBenchmarkBundle, PsionPluginResultInterpretationBenchmarkError,
    PSION_PLUGIN_GUEST_PLUGIN_BENCHMARK_BUNDLE_REF, PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL,
    PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF,
    PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF, PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF,
    PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_REF, PSION_PLUGIN_MIXED_REFERENCE_COMPARISON_LABEL,
    PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF, PSION_PLUGIN_MIXED_SERVED_POSTURE_REF,
    PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
    PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
};

/// Stable schema version for the plugin route/refusal hardening bundle.
pub const PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_route_refusal_hardening_bundle.v1";
/// Stable committed bundle ref for the plugin route/refusal hardening tranche.
pub const PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_BUNDLE_REF: &str = "fixtures/psion/plugins/hardening/psion_plugin_route_refusal_hardening_v1/psion_plugin_route_refusal_hardening_bundle.json";
/// Stable human-readable hardening doc for the tranche.
pub const PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_DOC_REF: &str =
    "docs/PSION_PLUGIN_ROUTE_REFUSAL_HARDENING.md";
/// Stable follow-up operator audit ref for the host-native Google proof.
pub const PSION_PLUGIN_HOST_NATIVE_GOOGLE_AUDIT_REF: &str = "docs/audits/2026-03-22-openagentsgemini-first-google-host-native-plugin-conditioned-run-audit.md";
/// Stable follow-up operator audit ref for the mixed Google proof.
pub const PSION_PLUGIN_MIXED_GOOGLE_AUDIT_REF: &str =
    "docs/audits/2026-03-22-openagentsgemini-first-google-mixed-plugin-conditioned-run-audit.md";

const HOST_NATIVE_OVERDELEGATION_CASE_ID: &str = "host_native.overdelegation_answer_in_language";
const HOST_NATIVE_UNSUPPORTED_REFUSAL_CASE_ID: &str = "host_native.unsupported_capability_refusal";
const HOST_NATIVE_RECEIPT_BOUND_INTERPRETATION_CASE_ID: &str =
    "host_native.receipt_bound_result_interpretation";
const MIXED_GUEST_UNSUPPORTED_LOAD_CASE_ID: &str = "mixed.guest_unsupported_digest_load_refusal";
const MIXED_RECEIPT_BOUND_INTERPRETATION_CASE_ID: &str =
    "mixed.receipt_bound_result_interpretation";

/// Which learned lane the hardening row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginRouteRefusalHardeningLane {
    HostNativeReference,
    MixedReference,
}

/// Unified claim-surface vocabulary for the hardening tranche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginHardeningClaimSurface {
    LearnedJudgment,
    BenchmarkBackedCapabilityClaim,
    ExecutorBackedResult,
    SourceGroundedStatement,
    Verification,
    PluginPublication,
    PublicPluginUniversality,
    ArbitrarySoftwareCapability,
    HiddenExecutionWithoutRuntimeReceipt,
}

/// Explicit execution-implication failure shape tested by the tranche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginExecutionImplicationScenarioKind {
    OverdelegationAnswerInLanguage,
    UnsupportedCapabilityRefusal,
    ReceiptBoundResultInterpretation,
    GuestUnsupportedDigestLoadRefusal,
}

/// One frozen route/refusal regression row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginRouteRefusalRegressionRow {
    pub lane: PsionPluginRouteRefusalHardeningLane,
    pub lane_id: String,
    pub benchmark_family: PsionPluginBenchmarkFamily,
    pub reference_label: String,
    pub eligible_item_count: u32,
    pub out_of_scope_item_count: u32,
    pub reference_score_bps: u32,
    pub observed_score_bps: u32,
    pub frozen_min_score_bps: u32,
    pub regression_budget_bps: u32,
    pub detail: String,
}

/// One frozen overdelegation budget for a learned plugin lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginOverdelegationBudget {
    pub lane: PsionPluginRouteRefusalHardeningLane,
    pub lane_id: String,
    pub benchmark_package_id: String,
    pub benchmark_package_digest: String,
    pub benchmark_receipt_id: String,
    pub benchmark_receipt_digest: String,
    pub overdelegation_negative_item_ids: Vec<String>,
    pub overdelegation_negative_item_count: u32,
    pub reference_overdelegation_rejection_accuracy_bps: u32,
    pub observed_overdelegation_failure_bps: u32,
    pub max_allowed_overdelegation_failure_bps: u32,
    pub detail: String,
}

/// One explicit no-implicit-execution case retained by the hardening tranche.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginExecutionImplicationCase {
    pub case_id: String,
    pub lane: PsionPluginRouteRefusalHardeningLane,
    pub lane_id: String,
    pub source_bundle_ref: String,
    pub source_item_id: String,
    pub scenario_kind: PsionPluginExecutionImplicationScenarioKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_route_label: Option<PsionPluginRouteLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_supported_claim_surfaces: Vec<PsionPluginHardeningClaimSurface>,
    pub required_blocked_claim_surfaces: Vec<PsionPluginHardeningClaimSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_typed_refusal_reasons: Vec<String>,
    pub execution_evidence_required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_receipt_refs: Vec<String>,
    pub forbid_unseen_execution_claims: bool,
    pub required_policy_phrase: String,
    pub observed_pass: bool,
    pub detail: String,
}

/// Full bounded output bundle for the plugin route/refusal hardening tranche.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginRouteRefusalHardeningBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub hardening_doc_ref: String,
    pub host_native_run_bundle_ref: String,
    pub host_native_run_bundle_digest: String,
    pub host_native_capability_matrix_ref: String,
    pub host_native_capability_matrix_digest: String,
    pub host_native_served_posture_ref: String,
    pub host_native_served_posture_digest: String,
    pub mixed_run_bundle_ref: String,
    pub mixed_run_bundle_digest: String,
    pub mixed_capability_matrix_ref: String,
    pub mixed_capability_matrix_digest: String,
    pub mixed_served_posture_ref: String,
    pub mixed_served_posture_digest: String,
    pub refusal_benchmark_bundle_ref: String,
    pub refusal_benchmark_bundle_digest: String,
    pub result_interpretation_benchmark_bundle_ref: String,
    pub result_interpretation_benchmark_bundle_digest: String,
    pub guest_benchmark_bundle_ref: String,
    pub guest_benchmark_bundle_digest: String,
    pub host_native_google_audit_ref: String,
    pub mixed_google_audit_ref: String,
    pub regression_rows: Vec<PsionPluginRouteRefusalRegressionRow>,
    pub overdelegation_budgets: Vec<PsionPluginOverdelegationBudget>,
    pub execution_implication_cases: Vec<PsionPluginExecutionImplicationCase>,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionPluginRouteRefusalHardeningBundle {
    /// Writes the bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginRouteRefusalHardeningError> {
        write_json_file(self, output_path)
    }

    fn validate_against_inputs(
        &self,
        host_native_run_bundle: &PsionPluginHostNativeReferenceRunBundle,
        host_native_matrix: &PsionPluginHostNativeCapabilityMatrix,
        host_native_posture: &PsionPluginHostNativeServedPosture,
        mixed_run_bundle: &PsionPluginMixedReferenceRunBundle,
        mixed_matrix: &PsionPluginMixedCapabilityMatrix,
        mixed_posture: &PsionPluginMixedServedPosture,
        refusal_bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
        interpretation_bundle: &PsionPluginResultInterpretationBenchmarkBundle,
        guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
    ) -> Result<(), PsionPluginRouteRefusalHardeningError> {
        host_native_matrix.validate_against_run_bundle(host_native_run_bundle)?;
        host_native_posture
            .validate_against_matrix_and_run_bundle(host_native_matrix, host_native_run_bundle)?;
        mixed_matrix.validate_against_inputs(mixed_run_bundle, guest_benchmark_bundle)?;
        mixed_posture.validate_against_inputs(
            mixed_matrix,
            mixed_run_bundle,
            guest_benchmark_bundle,
        )?;
        refusal_bundle.validate_against_contamination(
            &psionic_data::build_psion_plugin_contamination_bundle()?,
        )?;
        interpretation_bundle.validate_against_contamination(
            &psionic_data::build_psion_plugin_contamination_bundle()?,
        )?;
        guest_benchmark_bundle.validate_against_contamination(
            &psionic_data::build_psion_plugin_mixed_contamination_bundle()?,
        )?;

        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_BUNDLE_SCHEMA_VERSION,
            "psion_plugin_route_refusal_hardening_bundle.schema_version",
        )?;
        check_string_match(
            self.bundle_id.as_str(),
            "bundle.psion.plugin_route_refusal_hardening.v1",
            "psion_plugin_route_refusal_hardening_bundle.bundle_id",
        )?;
        check_string_match(
            self.hardening_doc_ref.as_str(),
            PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_DOC_REF,
            "psion_plugin_route_refusal_hardening_bundle.hardening_doc_ref",
        )?;
        check_string_match(
            self.host_native_run_bundle_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF,
            "psion_plugin_route_refusal_hardening_bundle.host_native_run_bundle_ref",
        )?;
        check_string_match(
            self.host_native_run_bundle_digest.as_str(),
            host_native_run_bundle.bundle_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.host_native_run_bundle_digest",
        )?;
        check_string_match(
            self.host_native_capability_matrix_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF,
            "psion_plugin_route_refusal_hardening_bundle.host_native_capability_matrix_ref",
        )?;
        check_string_match(
            self.host_native_capability_matrix_digest.as_str(),
            host_native_matrix.matrix_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.host_native_capability_matrix_digest",
        )?;
        check_string_match(
            self.host_native_served_posture_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF,
            "psion_plugin_route_refusal_hardening_bundle.host_native_served_posture_ref",
        )?;
        check_string_match(
            self.host_native_served_posture_digest.as_str(),
            host_native_posture.posture_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.host_native_served_posture_digest",
        )?;
        check_string_match(
            self.mixed_run_bundle_ref.as_str(),
            PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF,
            "psion_plugin_route_refusal_hardening_bundle.mixed_run_bundle_ref",
        )?;
        check_string_match(
            self.mixed_run_bundle_digest.as_str(),
            mixed_run_bundle.bundle_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.mixed_run_bundle_digest",
        )?;
        check_string_match(
            self.mixed_capability_matrix_ref.as_str(),
            PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_REF,
            "psion_plugin_route_refusal_hardening_bundle.mixed_capability_matrix_ref",
        )?;
        check_string_match(
            self.mixed_capability_matrix_digest.as_str(),
            mixed_matrix.matrix_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.mixed_capability_matrix_digest",
        )?;
        check_string_match(
            self.mixed_served_posture_ref.as_str(),
            PSION_PLUGIN_MIXED_SERVED_POSTURE_REF,
            "psion_plugin_route_refusal_hardening_bundle.mixed_served_posture_ref",
        )?;
        check_string_match(
            self.mixed_served_posture_digest.as_str(),
            mixed_posture.posture_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.mixed_served_posture_digest",
        )?;
        check_string_match(
            self.refusal_benchmark_bundle_ref.as_str(),
            PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
            "psion_plugin_route_refusal_hardening_bundle.refusal_benchmark_bundle_ref",
        )?;
        check_string_match(
            self.refusal_benchmark_bundle_digest.as_str(),
            refusal_bundle.bundle_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.refusal_benchmark_bundle_digest",
        )?;
        check_string_match(
            self.result_interpretation_benchmark_bundle_ref.as_str(),
            PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
            "psion_plugin_route_refusal_hardening_bundle.result_interpretation_benchmark_bundle_ref",
        )?;
        check_string_match(
            self.result_interpretation_benchmark_bundle_digest.as_str(),
            interpretation_bundle.bundle_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.result_interpretation_benchmark_bundle_digest",
        )?;
        check_string_match(
            self.guest_benchmark_bundle_ref.as_str(),
            PSION_PLUGIN_GUEST_PLUGIN_BENCHMARK_BUNDLE_REF,
            "psion_plugin_route_refusal_hardening_bundle.guest_benchmark_bundle_ref",
        )?;
        check_string_match(
            self.guest_benchmark_bundle_digest.as_str(),
            guest_benchmark_bundle.bundle_digest.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.guest_benchmark_bundle_digest",
        )?;
        check_string_match(
            self.host_native_google_audit_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_GOOGLE_AUDIT_REF,
            "psion_plugin_route_refusal_hardening_bundle.host_native_google_audit_ref",
        )?;
        check_string_match(
            self.mixed_google_audit_ref.as_str(),
            PSION_PLUGIN_MIXED_GOOGLE_AUDIT_REF,
            "psion_plugin_route_refusal_hardening_bundle.mixed_google_audit_ref",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_route_refusal_hardening_bundle.summary",
        )?;
        if self.regression_rows.is_empty() {
            return Err(PsionPluginRouteRefusalHardeningError::MissingField {
                field: String::from("psion_plugin_route_refusal_hardening_bundle.regression_rows"),
            });
        }
        if self.overdelegation_budgets.is_empty() {
            return Err(PsionPluginRouteRefusalHardeningError::MissingField {
                field: String::from(
                    "psion_plugin_route_refusal_hardening_bundle.overdelegation_budgets",
                ),
            });
        }
        if self.execution_implication_cases.is_empty() {
            return Err(PsionPluginRouteRefusalHardeningError::MissingField {
                field: String::from(
                    "psion_plugin_route_refusal_hardening_bundle.execution_implication_cases",
                ),
            });
        }

        let expected_rows = regression_rows(host_native_run_bundle, mixed_run_bundle);
        check_debug_slice_match(
            self.regression_rows.as_slice(),
            expected_rows.as_slice(),
            "psion_plugin_route_refusal_hardening_bundle.regression_rows",
        )?;
        let expected_budgets =
            overdelegation_budgets(host_native_run_bundle, mixed_run_bundle, refusal_bundle)?;
        check_debug_slice_match(
            self.overdelegation_budgets.as_slice(),
            expected_budgets.as_slice(),
            "psion_plugin_route_refusal_hardening_bundle.overdelegation_budgets",
        )?;
        let expected_cases = execution_implication_cases(
            host_native_posture,
            mixed_posture,
            refusal_bundle,
            interpretation_bundle,
            guest_benchmark_bundle,
        )?;
        check_debug_slice_match(
            self.execution_implication_cases.as_slice(),
            expected_cases.as_slice(),
            "psion_plugin_route_refusal_hardening_bundle.execution_implication_cases",
        )?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginRouteRefusalHardeningError::DigestMismatch {
                kind: String::from("psion_plugin_route_refusal_hardening_bundle"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginRouteRefusalHardeningError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("unknown benchmark family `{benchmark_family:?}` in a hardening row")]
    UnknownBenchmarkFamily {
        benchmark_family: PsionPluginBenchmarkFamily,
    },
    #[error("unknown benchmark metric `{metric_kind:?}` in the hardening tranche")]
    UnknownBenchmarkMetric {
        metric_kind: PsionPluginBenchmarkMetricKind,
    },
    #[error("unknown benchmark item `{item_id}` in `{bundle_ref}`")]
    UnknownBenchmarkItem { bundle_ref: String, item_id: String },
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error(transparent)]
    HostNativeReference(#[from] PsionPluginHostNativeReferenceLaneError),
    #[error(transparent)]
    MixedReference(#[from] PsionPluginMixedReferenceLaneError),
    #[error(transparent)]
    HostNativeCapabilityMatrix(#[from] PsionPluginHostNativeCapabilityMatrixError),
    #[error(transparent)]
    MixedCapabilityMatrix(#[from] PsionPluginMixedCapabilityMatrixError),
    #[error(transparent)]
    RefusalBenchmark(#[from] PsionPluginRefusalRequestStructureBenchmarkError),
    #[error(transparent)]
    InterpretationBenchmark(#[from] PsionPluginResultInterpretationBenchmarkError),
    #[error(transparent)]
    GuestBenchmark(#[from] PsionPluginGuestPluginBenchmarkError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Contamination(#[from] psionic_data::PsionPluginContaminationError),
}

/// Returns the canonical output path for the committed hardening bundle.
#[must_use]
pub fn psion_plugin_route_refusal_hardening_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_BUNDLE_REF)
}

/// Builds the canonical route/refusal hardening bundle from committed tranche artifacts.
pub fn build_psion_plugin_route_refusal_hardening_bundle(
) -> Result<PsionPluginRouteRefusalHardeningBundle, PsionPluginRouteRefusalHardeningError> {
    let host_native_run_bundle = run_psion_plugin_host_native_reference_lane()?;
    let host_native_matrix =
        record_psion_plugin_host_native_capability_matrix(&host_native_run_bundle)?;
    let host_native_posture = record_psion_plugin_host_native_served_posture(
        &host_native_matrix,
        &host_native_run_bundle,
    )?;
    let mixed_run_bundle = run_psion_plugin_mixed_reference_lane()?;
    let guest_benchmark_bundle = build_psion_plugin_guest_plugin_benchmark_bundle()?;
    let mixed_matrix =
        record_psion_plugin_mixed_capability_matrix(&mixed_run_bundle, &guest_benchmark_bundle)?;
    let mixed_posture = record_psion_plugin_mixed_served_posture(
        &mixed_matrix,
        &mixed_run_bundle,
        &guest_benchmark_bundle,
    )?;
    let refusal_bundle = build_psion_plugin_refusal_request_structure_benchmark_bundle()?;
    let interpretation_bundle = build_psion_plugin_result_interpretation_benchmark_bundle()?;
    let regression_rows = regression_rows(&host_native_run_bundle, &mixed_run_bundle);
    let overdelegation_budgets =
        overdelegation_budgets(&host_native_run_bundle, &mixed_run_bundle, &refusal_bundle)?;
    let execution_implication_cases = execution_implication_cases(
        &host_native_posture,
        &mixed_posture,
        &refusal_bundle,
        &interpretation_bundle,
        &guest_benchmark_bundle,
    )?;
    let mut bundle = PsionPluginRouteRefusalHardeningBundle {
        schema_version: String::from(PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("bundle.psion.plugin_route_refusal_hardening.v1"),
        hardening_doc_ref: String::from(PSION_PLUGIN_ROUTE_REFUSAL_HARDENING_DOC_REF),
        host_native_run_bundle_ref: String::from(PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF),
        host_native_run_bundle_digest: host_native_run_bundle.bundle_digest.clone(),
        host_native_capability_matrix_ref: String::from(
            PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF,
        ),
        host_native_capability_matrix_digest: host_native_matrix.matrix_digest.clone(),
        host_native_served_posture_ref: String::from(PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF),
        host_native_served_posture_digest: host_native_posture.posture_digest.clone(),
        mixed_run_bundle_ref: String::from(PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF),
        mixed_run_bundle_digest: mixed_run_bundle.bundle_digest.clone(),
        mixed_capability_matrix_ref: String::from(PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_REF),
        mixed_capability_matrix_digest: mixed_matrix.matrix_digest.clone(),
        mixed_served_posture_ref: String::from(PSION_PLUGIN_MIXED_SERVED_POSTURE_REF),
        mixed_served_posture_digest: mixed_posture.posture_digest.clone(),
        refusal_benchmark_bundle_ref: String::from(
            PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
        ),
        refusal_benchmark_bundle_digest: refusal_bundle.bundle_digest.clone(),
        result_interpretation_benchmark_bundle_ref: String::from(
            PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
        ),
        result_interpretation_benchmark_bundle_digest: interpretation_bundle.bundle_digest.clone(),
        guest_benchmark_bundle_ref: String::from(PSION_PLUGIN_GUEST_PLUGIN_BENCHMARK_BUNDLE_REF),
        guest_benchmark_bundle_digest: guest_benchmark_bundle.bundle_digest.clone(),
        host_native_google_audit_ref: String::from(PSION_PLUGIN_HOST_NATIVE_GOOGLE_AUDIT_REF),
        mixed_google_audit_ref: String::from(PSION_PLUGIN_MIXED_GOOGLE_AUDIT_REF),
        regression_rows,
        overdelegation_budgets,
        execution_implication_cases,
        summary: String::from(
            "The plugin route/refusal hardening tranche freezes the current host-native and mixed learned-lane route/refusal scores, the zero-bps overdelegation failure budget, and explicit no-implicit-execution cases that later operator or cluster decisions must cite instead of narrative-only confidence.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_inputs(
        &host_native_run_bundle,
        &host_native_matrix,
        &host_native_posture,
        &mixed_run_bundle,
        &mixed_matrix,
        &mixed_posture,
        &refusal_bundle,
        &interpretation_bundle,
        &guest_benchmark_bundle,
    )?;
    Ok(bundle)
}

/// Builds and writes the canonical route/refusal hardening bundle.
pub fn write_psion_plugin_route_refusal_hardening_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginRouteRefusalHardeningBundle, PsionPluginRouteRefusalHardeningError> {
    let bundle = build_psion_plugin_route_refusal_hardening_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

fn regression_rows(
    host_native_run_bundle: &PsionPluginHostNativeReferenceRunBundle,
    mixed_run_bundle: &PsionPluginMixedReferenceRunBundle,
) -> Vec<PsionPluginRouteRefusalRegressionRow> {
    let mut rows = Vec::new();
    for family in [
        PsionPluginBenchmarkFamily::DiscoverySelection,
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        PsionPluginBenchmarkFamily::ResultInterpretation,
    ] {
        if let Some(row) = host_native_run_bundle
            .evaluation_receipt
            .benchmark_deltas
            .iter()
            .find(|row| row.benchmark_family == family)
        {
            rows.push(PsionPluginRouteRefusalRegressionRow {
                lane: PsionPluginRouteRefusalHardeningLane::HostNativeReference,
                lane_id: host_native_run_bundle.lane_id.clone(),
                benchmark_family: family,
                reference_label: String::from(PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL),
                eligible_item_count: row.eligible_item_count,
                out_of_scope_item_count: row.out_of_scope_item_count,
                reference_score_bps: row.baseline_score_bps,
                observed_score_bps: row.trained_score_bps,
                frozen_min_score_bps: row.trained_score_bps,
                regression_budget_bps: 0,
                detail: format!(
                    "Host-native {:?} is now frozen at its current trained score so later route/refusal claims cannot drift below the current proved-class-only baseline without an explicit regression receipt.",
                    family
                ),
            });
        }
    }
    for family in [
        PsionPluginBenchmarkFamily::DiscoverySelection,
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        PsionPluginBenchmarkFamily::ResultInterpretation,
    ] {
        if let Some(row) = mixed_run_bundle
            .evaluation_receipt
            .benchmark_comparisons
            .iter()
            .find(|row| row.benchmark_family == family)
        {
            rows.push(PsionPluginRouteRefusalRegressionRow {
                lane: PsionPluginRouteRefusalHardeningLane::MixedReference,
                lane_id: mixed_run_bundle.lane_id.clone(),
                benchmark_family: family,
                reference_label: String::from(PSION_PLUGIN_MIXED_REFERENCE_COMPARISON_LABEL),
                eligible_item_count: row.eligible_item_count,
                out_of_scope_item_count: row.out_of_scope_item_count,
                reference_score_bps: row.host_native_reference_score_bps,
                observed_score_bps: row.mixed_score_bps,
                frozen_min_score_bps: row.mixed_score_bps,
                regression_budget_bps: 0,
                detail: format!(
                    "Mixed {:?} is now frozen at its current score relative to the committed host-native reference lane, so later guest-artifact or networked expansions have to cite an explicit regression surface instead of quietly widening.",
                    family
                ),
            });
        }
    }
    rows
}

fn overdelegation_budgets(
    host_native_run_bundle: &PsionPluginHostNativeReferenceRunBundle,
    mixed_run_bundle: &PsionPluginMixedReferenceRunBundle,
    refusal_bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
) -> Result<Vec<PsionPluginOverdelegationBudget>, PsionPluginRouteRefusalHardeningError> {
    let overdelegation_metric = find_metric(
        &refusal_bundle.receipt.observed_metrics,
        PsionPluginBenchmarkMetricKind::OverdelegationRejectionAccuracyBps,
    )?;
    let mut overdelegation_negative_item_ids = refusal_bundle
        .package
        .items
        .iter()
        .filter_map(|item| match &item.task {
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(task)
                if task.overdelegation_negative =>
            {
                Some(item.item_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    overdelegation_negative_item_ids.sort();
    let observed_overdelegation_failure_bps = 10_000 - overdelegation_metric.value_bps;
    Ok(vec![
        PsionPluginOverdelegationBudget {
            lane: PsionPluginRouteRefusalHardeningLane::HostNativeReference,
            lane_id: host_native_run_bundle.lane_id.clone(),
            benchmark_package_id: refusal_bundle.package.package_id.clone(),
            benchmark_package_digest: refusal_bundle.package.package_digest.clone(),
            benchmark_receipt_id: refusal_bundle.receipt.receipt_id.clone(),
            benchmark_receipt_digest: refusal_bundle.receipt.receipt_digest.clone(),
            overdelegation_negative_item_ids: overdelegation_negative_item_ids.clone(),
            overdelegation_negative_item_count: overdelegation_negative_item_ids.len() as u32,
            reference_overdelegation_rejection_accuracy_bps: overdelegation_metric.value_bps,
            observed_overdelegation_failure_bps,
            max_allowed_overdelegation_failure_bps: 0,
            detail: String::from(
                "The host-native learned lane freezes a zero-bps overdelegation failure budget against the current authored negative cases, so future route hardening must fail closed if those direct-answer or unsupported-capability negatives slip.",
            ),
        },
        PsionPluginOverdelegationBudget {
            lane: PsionPluginRouteRefusalHardeningLane::MixedReference,
            lane_id: mixed_run_bundle.lane_id.clone(),
            benchmark_package_id: refusal_bundle.package.package_id.clone(),
            benchmark_package_digest: refusal_bundle.package.package_digest.clone(),
            benchmark_receipt_id: refusal_bundle.receipt.receipt_id.clone(),
            benchmark_receipt_digest: refusal_bundle.receipt.receipt_digest.clone(),
            overdelegation_negative_item_ids,
            overdelegation_negative_item_count: 2,
            reference_overdelegation_rejection_accuracy_bps: overdelegation_metric.value_bps,
            observed_overdelegation_failure_bps,
            max_allowed_overdelegation_failure_bps: 0,
            detail: String::from(
                "The mixed learned lane inherits the same zero-bps overdelegation failure budget so guest-artifact breadth does not silently buy broader overdelegation tolerance.",
            ),
        },
    ])
}

fn execution_implication_cases(
    host_native_posture: &PsionPluginHostNativeServedPosture,
    mixed_posture: &PsionPluginMixedServedPosture,
    refusal_bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
    interpretation_bundle: &PsionPluginResultInterpretationBenchmarkBundle,
    guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
) -> Result<Vec<PsionPluginExecutionImplicationCase>, PsionPluginRouteRefusalHardeningError> {
    let host_native_overdelegation = refusal_item(
        refusal_bundle,
        "plugin_overdelegation_answer_in_language_v1",
    )?;
    let host_native_unsupported =
        refusal_item(refusal_bundle, "plugin_refusal_unsupported_capability_v1")?;
    let host_native_interpretation = interpretation_item(
        interpretation_bundle,
        "plugin_result_interpretation_fetch_refusal_v1",
    )?;
    let mixed_guest_unsupported = guest_item(
        guest_benchmark_bundle,
        "guest_plugin_unsupported_digest_load_claim_v1",
    )?;
    let mixed_interpretation = interpretation_item(
        interpretation_bundle,
        "plugin_result_interpretation_fetch_refusal_v1",
    )?;

    let host_supported = host_native_posture
        .supported_claim_surfaces
        .iter()
        .copied()
        .map(map_host_native_claim_surface)
        .collect::<BTreeSet<_>>();
    let host_blocked = host_native_posture
        .blocked_claim_surfaces
        .iter()
        .copied()
        .map(map_host_native_claim_surface)
        .collect::<BTreeSet<_>>();
    let mixed_supported = mixed_posture
        .supported_claim_surfaces
        .iter()
        .copied()
        .map(map_mixed_claim_surface)
        .collect::<BTreeSet<_>>();
    let mixed_blocked = mixed_posture
        .blocked_claim_surfaces
        .iter()
        .copied()
        .map(map_mixed_claim_surface)
        .collect::<BTreeSet<_>>();

    Ok(vec![
        PsionPluginExecutionImplicationCase {
            case_id: String::from(HOST_NATIVE_OVERDELEGATION_CASE_ID),
            lane: PsionPluginRouteRefusalHardeningLane::HostNativeReference,
            lane_id: host_native_posture.lane_id.clone(),
            source_bundle_ref: String::from(
                PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
            ),
            source_item_id: host_native_overdelegation.item_id.clone(),
            scenario_kind:
                PsionPluginExecutionImplicationScenarioKind::OverdelegationAnswerInLanguage,
            expected_route_label: Some(PsionPluginRouteLabel::AnswerInLanguage),
            required_supported_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::LearnedJudgment,
            ],
            required_blocked_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
            ],
            required_typed_refusal_reasons: vec![],
            execution_evidence_required: host_native_overdelegation
                .receipt_posture
                .execution_evidence_required,
            required_receipt_refs: host_native_overdelegation
                .receipt_posture
                .required_receipt_refs
                .clone(),
            forbid_unseen_execution_claims: host_native_overdelegation
                .receipt_posture
                .forbid_unseen_execution_claims,
            required_policy_phrase: String::from("explicit runtime receipt refs"),
            observed_pass: host_supported
                .contains(&PsionPluginHardeningClaimSurface::LearnedJudgment)
                && host_blocked.contains(
                    &PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
                )
                && host_native_overdelegation
                    .receipt_posture
                    .forbid_unseen_execution_claims
                && host_native_posture
                    .execution_backed_statement_policy
                    .contains("explicit runtime receipt refs"),
            detail: String::from(
                "The host-native overdelegation negative must stay a plain learned-language answer while still blocking any hidden execution implication.",
            ),
        },
        PsionPluginExecutionImplicationCase {
            case_id: String::from(HOST_NATIVE_UNSUPPORTED_REFUSAL_CASE_ID),
            lane: PsionPluginRouteRefusalHardeningLane::HostNativeReference,
            lane_id: host_native_posture.lane_id.clone(),
            source_bundle_ref: String::from(
                PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
            ),
            source_item_id: host_native_unsupported.item_id.clone(),
            scenario_kind:
                PsionPluginExecutionImplicationScenarioKind::UnsupportedCapabilityRefusal,
            expected_route_label: Some(PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability),
            required_supported_claim_surfaces: vec![],
            required_blocked_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
            ],
            required_typed_refusal_reasons: vec![String::from(
                "plugin_capability_outside_admitted_set",
            )],
            execution_evidence_required: host_native_unsupported
                .receipt_posture
                .execution_evidence_required,
            required_receipt_refs: host_native_unsupported
                .receipt_posture
                .required_receipt_refs
                .clone(),
            forbid_unseen_execution_claims: host_native_unsupported
                .receipt_posture
                .forbid_unseen_execution_claims,
            required_policy_phrase: String::from("explicit runtime receipt refs"),
            observed_pass: host_blocked
                .contains(&PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt)
                && host_native_posture
                    .typed_refusal_reasons
                    .contains(&String::from("plugin_capability_outside_admitted_set"))
                && host_native_unsupported
                    .receipt_posture
                    .forbid_unseen_execution_claims
                && host_native_posture
                    .execution_backed_statement_policy
                    .contains("explicit runtime receipt refs"),
            detail: String::from(
                "The host-native unsupported-capability refusal must stay an explicit refusal reason without implying hidden fallback execution or unseen plugin retries.",
            ),
        },
        PsionPluginExecutionImplicationCase {
            case_id: String::from(HOST_NATIVE_RECEIPT_BOUND_INTERPRETATION_CASE_ID),
            lane: PsionPluginRouteRefusalHardeningLane::HostNativeReference,
            lane_id: host_native_posture.lane_id.clone(),
            source_bundle_ref: String::from(
                PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
            ),
            source_item_id: host_native_interpretation.item_id.clone(),
            scenario_kind:
                PsionPluginExecutionImplicationScenarioKind::ReceiptBoundResultInterpretation,
            expected_route_label: None,
            required_supported_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::ExecutorBackedResult,
            ],
            required_blocked_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
            ],
            required_typed_refusal_reasons: vec![],
            execution_evidence_required: host_native_interpretation
                .receipt_posture
                .execution_evidence_required,
            required_receipt_refs: host_native_interpretation
                .receipt_posture
                .required_receipt_refs
                .clone(),
            forbid_unseen_execution_claims: host_native_interpretation
                .receipt_posture
                .forbid_unseen_execution_claims,
            required_policy_phrase: String::from("explicit runtime receipt refs"),
            observed_pass: host_supported
                .contains(&PsionPluginHardeningClaimSurface::ExecutorBackedResult)
                && host_blocked.contains(
                    &PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
                )
                && host_native_interpretation
                    .receipt_posture
                    .execution_evidence_required
                && !host_native_interpretation
                    .receipt_posture
                    .required_receipt_refs
                    .is_empty()
                && host_native_interpretation
                    .receipt_posture
                    .forbid_unseen_execution_claims
                && host_native_posture
                    .execution_backed_statement_policy
                    .contains("explicit runtime receipt refs"),
            detail: String::from(
                "The host-native result-interpretation case may speak about an executor-backed result only when the cited runtime receipts stay explicit; otherwise it must not imply hidden fetch success or hidden retries.",
            ),
        },
        PsionPluginExecutionImplicationCase {
            case_id: String::from(MIXED_GUEST_UNSUPPORTED_LOAD_CASE_ID),
            lane: PsionPluginRouteRefusalHardeningLane::MixedReference,
            lane_id: mixed_posture.lane_id.clone(),
            source_bundle_ref: String::from(PSION_PLUGIN_GUEST_PLUGIN_BENCHMARK_BUNDLE_REF),
            source_item_id: mixed_guest_unsupported.item_id.clone(),
            scenario_kind:
                PsionPluginExecutionImplicationScenarioKind::GuestUnsupportedDigestLoadRefusal,
            expected_route_label: Some(PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability),
            required_supported_claim_surfaces: vec![],
            required_blocked_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::PluginPublication,
                PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
            ],
            required_typed_refusal_reasons: vec![String::from("guest_artifact_load_refusal")],
            execution_evidence_required: mixed_guest_unsupported
                .receipt_posture
                .execution_evidence_required,
            required_receipt_refs: mixed_guest_unsupported
                .receipt_posture
                .required_receipt_refs
                .clone(),
            forbid_unseen_execution_claims: mixed_guest_unsupported
                .receipt_posture
                .forbid_unseen_execution_claims,
            required_policy_phrase: String::from("do not imply hidden guest execution"),
            observed_pass: mixed_blocked
                .contains(&PsionPluginHardeningClaimSurface::PluginPublication)
                && mixed_blocked.contains(
                    &PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
                )
                && mixed_posture
                    .typed_refusal_reasons
                    .contains(&String::from("guest_artifact_load_refusal"))
                && mixed_guest_unsupported
                    .receipt_posture
                    .forbid_unseen_execution_claims
                && mixed_posture
                    .execution_backed_statement_policy
                    .contains("do not imply hidden guest execution"),
            detail: String::from(
                "The mixed lane must refuse unsupported guest-digest loading without implying generic guest execution, silent publication, or hidden fallback behavior.",
            ),
        },
        PsionPluginExecutionImplicationCase {
            case_id: String::from(MIXED_RECEIPT_BOUND_INTERPRETATION_CASE_ID),
            lane: PsionPluginRouteRefusalHardeningLane::MixedReference,
            lane_id: mixed_posture.lane_id.clone(),
            source_bundle_ref: String::from(
                PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
            ),
            source_item_id: mixed_interpretation.item_id.clone(),
            scenario_kind:
                PsionPluginExecutionImplicationScenarioKind::ReceiptBoundResultInterpretation,
            expected_route_label: None,
            required_supported_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::ExecutorBackedResult,
            ],
            required_blocked_claim_surfaces: vec![
                PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
            ],
            required_typed_refusal_reasons: vec![],
            execution_evidence_required: mixed_interpretation
                .receipt_posture
                .execution_evidence_required,
            required_receipt_refs: mixed_interpretation
                .receipt_posture
                .required_receipt_refs
                .clone(),
            forbid_unseen_execution_claims: mixed_interpretation
                .receipt_posture
                .forbid_unseen_execution_claims,
            required_policy_phrase: String::from("explicit runtime receipt refs"),
            observed_pass: mixed_supported
                .contains(&PsionPluginHardeningClaimSurface::ExecutorBackedResult)
                && mixed_blocked.contains(
                    &PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
                )
                && mixed_interpretation
                    .receipt_posture
                    .execution_evidence_required
                && !mixed_interpretation
                    .receipt_posture
                    .required_receipt_refs
                    .is_empty()
                && mixed_interpretation
                    .receipt_posture
                    .forbid_unseen_execution_claims
                && mixed_posture
                    .execution_backed_statement_policy
                    .contains("explicit runtime receipt refs"),
            detail: String::from(
                "The mixed lane keeps the same receipt-bound interpretation rule: executor-backed statements require explicit runtime receipt refs and cannot be widened into hidden execution claims.",
            ),
        },
    ])
}

fn refusal_item<'a>(
    bundle: &'a PsionPluginRefusalRequestStructureBenchmarkBundle,
    item_id: &str,
) -> Result<&'a crate::PsionPluginBenchmarkItem, PsionPluginRouteRefusalHardeningError> {
    bundle
        .package
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .ok_or_else(
            || PsionPluginRouteRefusalHardeningError::UnknownBenchmarkItem {
                bundle_ref: String::from(
                    PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
                ),
                item_id: String::from(item_id),
            },
        )
}

fn interpretation_item<'a>(
    bundle: &'a PsionPluginResultInterpretationBenchmarkBundle,
    item_id: &str,
) -> Result<&'a crate::PsionPluginBenchmarkItem, PsionPluginRouteRefusalHardeningError> {
    bundle
        .package
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .ok_or_else(
            || PsionPluginRouteRefusalHardeningError::UnknownBenchmarkItem {
                bundle_ref: String::from(PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF),
                item_id: String::from(item_id),
            },
        )
}

fn guest_item<'a>(
    bundle: &'a PsionPluginGuestPluginBenchmarkBundle,
    item_id: &str,
) -> Result<&'a crate::PsionPluginBenchmarkItem, PsionPluginRouteRefusalHardeningError> {
    bundle
        .package
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .ok_or_else(
            || PsionPluginRouteRefusalHardeningError::UnknownBenchmarkItem {
                bundle_ref: String::from(PSION_PLUGIN_GUEST_PLUGIN_BENCHMARK_BUNDLE_REF),
                item_id: String::from(item_id),
            },
        )
}

fn find_metric<'a>(
    metrics: &'a [crate::PsionPluginObservedMetric],
    metric_kind: PsionPluginBenchmarkMetricKind,
) -> Result<&'a crate::PsionPluginObservedMetric, PsionPluginRouteRefusalHardeningError> {
    metrics
        .iter()
        .find(|metric| metric.kind == metric_kind)
        .ok_or(PsionPluginRouteRefusalHardeningError::UnknownBenchmarkMetric { metric_kind })
}

fn map_host_native_claim_surface(
    surface: PsionPluginHostNativeClaimSurface,
) -> PsionPluginHardeningClaimSurface {
    match surface {
        PsionPluginHostNativeClaimSurface::LearnedJudgment => {
            PsionPluginHardeningClaimSurface::LearnedJudgment
        }
        PsionPluginHostNativeClaimSurface::BenchmarkBackedCapabilityClaim => {
            PsionPluginHardeningClaimSurface::BenchmarkBackedCapabilityClaim
        }
        PsionPluginHostNativeClaimSurface::ExecutorBackedResult => {
            PsionPluginHardeningClaimSurface::ExecutorBackedResult
        }
        PsionPluginHostNativeClaimSurface::SourceGroundedStatement => {
            PsionPluginHardeningClaimSurface::SourceGroundedStatement
        }
        PsionPluginHostNativeClaimSurface::Verification => {
            PsionPluginHardeningClaimSurface::Verification
        }
        PsionPluginHostNativeClaimSurface::PluginPublication => {
            PsionPluginHardeningClaimSurface::PluginPublication
        }
        PsionPluginHostNativeClaimSurface::PublicPluginUniversality => {
            PsionPluginHardeningClaimSurface::PublicPluginUniversality
        }
        PsionPluginHostNativeClaimSurface::ArbitrarySoftwareCapability => {
            PsionPluginHardeningClaimSurface::ArbitrarySoftwareCapability
        }
        PsionPluginHostNativeClaimSurface::HiddenExecutionWithoutRuntimeReceipt => {
            PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt
        }
    }
}

fn map_mixed_claim_surface(
    surface: PsionPluginMixedClaimSurface,
) -> PsionPluginHardeningClaimSurface {
    match surface {
        PsionPluginMixedClaimSurface::LearnedJudgment => {
            PsionPluginHardeningClaimSurface::LearnedJudgment
        }
        PsionPluginMixedClaimSurface::BenchmarkBackedCapabilityClaim => {
            PsionPluginHardeningClaimSurface::BenchmarkBackedCapabilityClaim
        }
        PsionPluginMixedClaimSurface::ExecutorBackedResult => {
            PsionPluginHardeningClaimSurface::ExecutorBackedResult
        }
        PsionPluginMixedClaimSurface::SourceGroundedStatement => {
            PsionPluginHardeningClaimSurface::SourceGroundedStatement
        }
        PsionPluginMixedClaimSurface::Verification => {
            PsionPluginHardeningClaimSurface::Verification
        }
        PsionPluginMixedClaimSurface::PluginPublication => {
            PsionPluginHardeningClaimSurface::PluginPublication
        }
        PsionPluginMixedClaimSurface::PublicPluginUniversality => {
            PsionPluginHardeningClaimSurface::PublicPluginUniversality
        }
        PsionPluginMixedClaimSurface::ArbitrarySoftwareCapability => {
            PsionPluginHardeningClaimSurface::ArbitrarySoftwareCapability
        }
        PsionPluginMixedClaimSurface::HiddenExecutionWithoutRuntimeReceipt => {
            PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

fn write_json_file<T: Serialize>(
    value: &T,
    output_path: impl AsRef<Path>,
) -> Result<(), PsionPluginRouteRefusalHardeningError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginRouteRefusalHardeningError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        PsionPluginRouteRefusalHardeningError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginRouteRefusalHardeningError> {
    if actual != expected {
        return Err(PsionPluginRouteRefusalHardeningError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginRouteRefusalHardeningError> {
    if value.is_empty() {
        return Err(PsionPluginRouteRefusalHardeningError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_debug_slice_match<T: std::fmt::Debug + PartialEq>(
    actual: &[T],
    expected: &[T],
    field: &str,
) -> Result<(), PsionPluginRouteRefusalHardeningError> {
    if actual != expected {
        return Err(PsionPluginRouteRefusalHardeningError::FieldMismatch {
            field: String::from(field),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn stable_bundle_digest(bundle: &PsionPluginRouteRefusalHardeningBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_route_refusal_hardening_bundle|");
    hasher.update(
        serde_json::to_vec(&canonical)
            .expect("route/refusal hardening bundle serialization should succeed"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_psion_plugin_route_refusal_hardening_bundle,
        write_psion_plugin_route_refusal_hardening_bundle, PsionPluginHardeningClaimSurface,
        PsionPluginRouteRefusalHardeningLane,
    };

    #[test]
    fn route_refusal_hardening_bundle_builds_and_freezes_zero_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_route_refusal_hardening_bundle()?;
        assert_eq!(bundle.regression_rows.len(), 9);
        assert_eq!(bundle.overdelegation_budgets.len(), 2);
        assert!(bundle
            .overdelegation_budgets
            .iter()
            .all(|budget| budget.max_allowed_overdelegation_failure_bps == 0));
        assert!(bundle
            .execution_implication_cases
            .iter()
            .all(|case| case.observed_pass));
        Ok(())
    }

    #[test]
    fn route_refusal_hardening_bundle_keeps_hidden_execution_blocked(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_route_refusal_hardening_bundle()?;
        let host_case = bundle
            .execution_implication_cases
            .iter()
            .find(|case| case.lane == PsionPluginRouteRefusalHardeningLane::HostNativeReference)
            .expect("host-native case should exist");
        assert!(host_case
            .required_blocked_claim_surfaces
            .contains(&PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt));
        let mixed_case = bundle
            .execution_implication_cases
            .iter()
            .find(|case| case.lane == PsionPluginRouteRefusalHardeningLane::MixedReference)
            .expect("mixed case should exist");
        assert!(mixed_case
            .required_blocked_claim_surfaces
            .contains(&PsionPluginHardeningClaimSurface::HiddenExecutionWithoutRuntimeReceipt));
        Ok(())
    }

    #[test]
    fn route_refusal_hardening_bundle_writes_to_disk() -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let output_path = tempdir
            .path()
            .join("psion_plugin_route_refusal_hardening_bundle.json");
        let bundle = write_psion_plugin_route_refusal_hardening_bundle(&output_path)?;
        let written = std::fs::read_to_string(output_path)?;
        assert!(written.contains(bundle.bundle_id.as_str()));
        Ok(())
    }
}
