use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{PsionPluginClass, PsionPluginRouteLabel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PsionPluginBenchmarkFamily, PsionPluginHostNativeReferenceRunBundle};

/// Stable schema version for the first host-native plugin-conditioned capability matrix.
pub const PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_host_native_capability_matrix.v1";
/// Stable schema version for the first host-native plugin-conditioned served posture.
pub const PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_host_native_served_posture.v1";
/// Stable committed fixture ref for the host-native plugin-conditioned capability matrix.
pub const PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF: &str =
    "fixtures/psion/plugins/capability/psion_plugin_host_native_capability_matrix_v1.json";
/// Stable committed fixture ref for the host-native plugin-conditioned served posture.
pub const PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF: &str =
    "fixtures/psion/plugins/serve/psion_plugin_host_native_served_posture_v1.json";
/// Stable claim-boundary doc inherited by the host-native capability matrix.
pub const PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF: &str =
    "docs/PSION_PLUGIN_CLAIM_BOUNDARY_AND_CAPABILITY_POSTURE.md";
/// Stable served-evidence doc inherited by the host-native served posture.
pub const PSION_SERVED_EVIDENCE_DOC_REF: &str = "docs/PSION_SERVED_EVIDENCE.md";
/// Stable served-output claim doc inherited by the host-native served posture.
pub const PSION_SERVED_OUTPUT_CLAIMS_DOC_REF: &str = "docs/PSION_SERVED_OUTPUT_CLAIMS.md";
/// Stable human-readable capability matrix doc for the host-native publication.
pub const PSION_PLUGIN_HOST_NATIVE_CAPABILITY_DOC_REF: &str =
    "docs/PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_V1.md";

/// Posture published for one host-native plugin-conditioned capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginHostNativeCapabilityPosture {
    /// Region is supported and explicitly publication-backed in v1.
    Supported,
    /// Region or class exists in docs but is not yet proved end to end.
    NotYetProved,
    /// Region or class is outside the current bounded trained lane.
    Unsupported,
    /// Claim family is intentionally blocked rather than merely unproven.
    Blocked,
}

/// Claim class carried by one host-native plugin-conditioned capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginHostNativeCapabilityClaimClass {
    /// Bounded learned plugin-use region.
    PluginUseRegion,
    /// Whole plugin-class substrate posture.
    PluginClassBoundary,
    /// Publication or marketplace boundary.
    PublicationBoundary,
    /// Arbitrary software-capability boundary.
    SoftwareCapabilityBoundary,
}

/// Benchmark evidence bound to one capability row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeBenchmarkEvidence {
    /// Benchmark family that backs the row.
    pub benchmark_family: PsionPluginBenchmarkFamily,
    /// Shared host-native evaluation receipt identifier.
    pub evaluation_receipt_id: String,
    /// Shared host-native evaluation receipt digest.
    pub evaluation_receipt_digest: String,
    /// Eligible item count within the published boundary.
    pub eligible_item_count: u32,
    /// Explicit excluded item count outside the published boundary.
    pub out_of_scope_item_count: u32,
    /// Baseline score in basis points.
    pub baseline_score_bps: u32,
    /// Trained score in basis points.
    pub trained_score_bps: u32,
    /// Improvement over baseline in basis points.
    pub delta_bps: i32,
}

impl PsionPluginHostNativeBenchmarkEvidence {
    fn validate(
        &self,
        expected_receipt_id: &str,
        expected_receipt_digest: &str,
        field: &str,
    ) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
        check_string_match(
            self.evaluation_receipt_id.as_str(),
            expected_receipt_id,
            format!("{field}.evaluation_receipt_id").as_str(),
        )?;
        check_string_match(
            self.evaluation_receipt_digest.as_str(),
            expected_receipt_digest,
            format!("{field}.evaluation_receipt_digest").as_str(),
        )?;
        if self.baseline_score_bps > 10_000 || self.trained_score_bps > 10_000 {
            return Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                field: format!("{field}.score_bps"),
                expected: String::from("at most 10000"),
                actual: format!("{} / {}", self.baseline_score_bps, self.trained_score_bps),
            });
        }
        if self.delta_bps != self.trained_score_bps as i32 - self.baseline_score_bps as i32 {
            return Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                field: format!("{field}.delta_bps"),
                expected: (self.trained_score_bps as i32 - self.baseline_score_bps as i32)
                    .to_string(),
                actual: self.delta_bps.to_string(),
            });
        }
        Ok(())
    }
}

/// One explicit region or boundary row in the host-native capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeCapabilityRow {
    /// Stable capability-region or boundary identifier.
    pub region_id: String,
    /// Publication posture for the row.
    pub posture: PsionPluginHostNativeCapabilityPosture,
    /// Claim class for the row.
    pub claim_class: PsionPluginHostNativeCapabilityClaimClass,
    /// Plugin class bound by the row when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_class: Option<PsionPluginClass>,
    /// Route labels explicitly inside the row boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_labels: Vec<PsionPluginRouteLabel>,
    /// Learned plugin ids admitted by the row when applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admitted_plugin_ids: Vec<String>,
    /// Benchmark evidence for the row when the row is backed by benchmark deltas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_evidence: Option<PsionPluginHostNativeBenchmarkEvidence>,
    /// Short explanation of the row.
    pub detail: String,
}

/// Machine-readable capability matrix for the first host-native plugin-conditioned lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeCapabilityMatrix {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable matrix identifier.
    pub matrix_id: String,
    /// Stable matrix version.
    pub matrix_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Bound model-artifact identifier.
    pub model_artifact_id: String,
    /// Bound model-artifact digest.
    pub model_artifact_digest: String,
    /// Bound evaluation-receipt identifier.
    pub evaluation_receipt_id: String,
    /// Bound evaluation-receipt digest.
    pub evaluation_receipt_digest: String,
    /// Claim-boundary authority doc inherited by the matrix.
    pub claim_boundary_doc_ref: String,
    /// Served-evidence doc inherited by the matrix.
    pub served_evidence_doc_ref: String,
    /// Served-output claim doc inherited by the matrix.
    pub served_output_claim_doc_ref: String,
    /// Explicit published rows.
    pub rows: Vec<PsionPluginHostNativeCapabilityRow>,
    /// Short explanation of the matrix.
    pub summary: String,
    /// Stable digest over the matrix.
    pub matrix_digest: String,
}

impl PsionPluginHostNativeCapabilityMatrix {
    /// Writes the capability matrix to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
        write_json_file(self, output_path)
    }

    /// Validates the capability matrix against the committed host-native reference bundle.
    pub fn validate_against_run_bundle(
        &self,
        run_bundle: &PsionPluginHostNativeReferenceRunBundle,
    ) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_SCHEMA_VERSION,
            "psion_plugin_host_native_capability_matrix.schema_version",
        )?;
        check_string_match(
            self.matrix_id.as_str(),
            "psion_plugin_host_native_capability_matrix",
            "psion_plugin_host_native_capability_matrix.matrix_id",
        )?;
        check_string_match(
            self.matrix_version.as_str(),
            "v1",
            "psion_plugin_host_native_capability_matrix.matrix_version",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            run_bundle.lane_id.as_str(),
            "psion_plugin_host_native_capability_matrix.lane_id",
        )?;
        check_string_match(
            self.model_artifact_id.as_str(),
            run_bundle.model_artifact.artifact_id.as_str(),
            "psion_plugin_host_native_capability_matrix.model_artifact_id",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            run_bundle.model_artifact.artifact_digest.as_str(),
            "psion_plugin_host_native_capability_matrix.model_artifact_digest",
        )?;
        check_string_match(
            self.evaluation_receipt_id.as_str(),
            run_bundle.evaluation_receipt.receipt_id.as_str(),
            "psion_plugin_host_native_capability_matrix.evaluation_receipt_id",
        )?;
        check_string_match(
            self.evaluation_receipt_digest.as_str(),
            run_bundle.evaluation_receipt.receipt_digest.as_str(),
            "psion_plugin_host_native_capability_matrix.evaluation_receipt_digest",
        )?;
        check_string_match(
            self.claim_boundary_doc_ref.as_str(),
            PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF,
            "psion_plugin_host_native_capability_matrix.claim_boundary_doc_ref",
        )?;
        check_string_match(
            self.served_evidence_doc_ref.as_str(),
            PSION_SERVED_EVIDENCE_DOC_REF,
            "psion_plugin_host_native_capability_matrix.served_evidence_doc_ref",
        )?;
        check_string_match(
            self.served_output_claim_doc_ref.as_str(),
            PSION_SERVED_OUTPUT_CLAIMS_DOC_REF,
            "psion_plugin_host_native_capability_matrix.served_output_claim_doc_ref",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_host_native_capability_matrix.summary",
        )?;
        if self.rows.is_empty() {
            return Err(PsionPluginHostNativeCapabilityMatrixError::MissingField {
                field: String::from("psion_plugin_host_native_capability_matrix.rows"),
            });
        }
        let mut seen_region_ids = BTreeSet::new();
        let learned_plugin_ids =
            sorted_unique_strings(run_bundle.model_artifact.learned_plugin_ids.as_slice());
        let mut supported_region_count = 0_u32;
        let mut saw_networked_substrate_proved_but_out_of_v1 = false;
        let mut saw_secret_backed_unsupported = false;
        let mut saw_guest_artifact_unsupported = false;
        let mut saw_publication_blocked = false;
        let mut saw_universality_blocked = false;
        let mut saw_arbitrary_software_blocked = false;
        let mut saw_multi_call_gap = false;
        for (index, row) in self.rows.iter().enumerate() {
            let field = format!("psion_plugin_host_native_capability_matrix.rows[{index}]");
            ensure_nonempty(
                row.region_id.as_str(),
                format!("{field}.region_id").as_str(),
            )?;
            ensure_nonempty(row.detail.as_str(), format!("{field}.detail").as_str())?;
            if !seen_region_ids.insert(row.region_id.as_str()) {
                return Err(PsionPluginHostNativeCapabilityMatrixError::DuplicateValue {
                    field: String::from(
                        "psion_plugin_host_native_capability_matrix.rows.region_id",
                    ),
                    value: row.region_id.clone(),
                });
            }
            reject_duplicate_strings(
                row.admitted_plugin_ids.as_slice(),
                format!("{field}.admitted_plugin_ids").as_str(),
            )?;
            if row
                .admitted_plugin_ids
                .iter()
                .any(|plugin_id| plugin_id == "plugin.http.fetch_text")
            {
                return Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                    field: format!("{field}.admitted_plugin_ids"),
                    expected: String::from("no networked plugin ids in v1"),
                    actual: format!("{:?}", row.admitted_plugin_ids),
                });
            }
            if let Some(evidence) = &row.benchmark_evidence {
                evidence.validate(
                    self.evaluation_receipt_id.as_str(),
                    self.evaluation_receipt_digest.as_str(),
                    format!("{field}.benchmark_evidence").as_str(),
                )?;
            }
            match row.posture {
                PsionPluginHostNativeCapabilityPosture::Supported => {
                    supported_region_count += 1;
                    check_option_match(
                        row.plugin_class,
                        Some(PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic),
                        format!("{field}.plugin_class").as_str(),
                    )?;
                    if row.benchmark_evidence.is_none() {
                        return Err(PsionPluginHostNativeCapabilityMatrixError::MissingField {
                            field: format!("{field}.benchmark_evidence"),
                        });
                    }
                    let evidence = row.benchmark_evidence.as_ref().expect("checked is_some");
                    if evidence.eligible_item_count == 0 || evidence.delta_bps <= 0 {
                        return Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                            field: format!("{field}.benchmark_evidence"),
                            expected: String::from(
                                "eligible_item_count > 0 and positive delta for supported row",
                            ),
                            actual: format!(
                                "eligible={} delta={}",
                                evidence.eligible_item_count, evidence.delta_bps
                            ),
                        });
                    }
                    if sorted_unique_strings(row.admitted_plugin_ids.as_slice())
                        != learned_plugin_ids
                    {
                        return Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                            field: format!("{field}.admitted_plugin_ids"),
                            expected: format!("{learned_plugin_ids:?}"),
                            actual: format!("{:?}", row.admitted_plugin_ids),
                        });
                    }
                }
                PsionPluginHostNativeCapabilityPosture::Unsupported => {
                    if row.plugin_class == Some(PsionPluginClass::HostNativeNetworkedReadOnly) {
                        saw_networked_substrate_proved_but_out_of_v1 = true;
                    }
                    if row.region_id
                        == "host_native_capability_free_local_deterministic.sequencing_multi_call"
                    {
                        saw_multi_call_gap = true;
                    }
                    if row.plugin_class == Some(PsionPluginClass::HostNativeSecretBackedOrStateful)
                    {
                        saw_secret_backed_unsupported = true;
                    }
                    if row.plugin_class == Some(PsionPluginClass::GuestArtifactDigestBound) {
                        saw_guest_artifact_unsupported = true;
                    }
                }
                PsionPluginHostNativeCapabilityPosture::Blocked => {
                    if row.region_id == "plugin_publication_or_marketplace" {
                        saw_publication_blocked = true;
                    }
                    if row.region_id == "public_plugin_universality" {
                        saw_universality_blocked = true;
                    }
                    if row.region_id == "arbitrary_software_capability" {
                        saw_arbitrary_software_blocked = true;
                    }
                }
                PsionPluginHostNativeCapabilityPosture::NotYetProved => {}
            }
        }
        if supported_region_count == 0 {
            return Err(PsionPluginHostNativeCapabilityMatrixError::MissingField {
                field: String::from("psion_plugin_host_native_capability_matrix.supported_rows"),
            });
        }
        ensure_bool_true(
            saw_networked_substrate_proved_but_out_of_v1,
            "psion_plugin_host_native_capability_matrix.networked_read_only_outside_supported_v1",
        )?;
        ensure_bool_true(
            saw_multi_call_gap,
            "psion_plugin_host_native_capability_matrix.multi_call_gap_explicit",
        )?;
        ensure_bool_true(
            saw_secret_backed_unsupported,
            "psion_plugin_host_native_capability_matrix.secret_backed_unsupported",
        )?;
        ensure_bool_true(
            saw_guest_artifact_unsupported,
            "psion_plugin_host_native_capability_matrix.guest_artifact_unsupported",
        )?;
        ensure_bool_true(
            saw_publication_blocked,
            "psion_plugin_host_native_capability_matrix.plugin_publication_blocked",
        )?;
        ensure_bool_true(
            saw_universality_blocked,
            "psion_plugin_host_native_capability_matrix.public_plugin_universality_blocked",
        )?;
        ensure_bool_true(
            saw_arbitrary_software_blocked,
            "psion_plugin_host_native_capability_matrix.arbitrary_software_capability_blocked",
        )?;
        if self.matrix_digest != stable_capability_matrix_digest(self) {
            return Err(PsionPluginHostNativeCapabilityMatrixError::DigestMismatch {
                kind: String::from("psion_plugin_host_native_capability_matrix"),
            });
        }
        Ok(())
    }
}

/// Visible claim surface allowed or blocked by the host-native served posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginHostNativeClaimSurface {
    /// Learned answer without execution implication.
    LearnedJudgment,
    /// Benchmark-backed capability claim inside the published matrix.
    BenchmarkBackedCapabilityClaim,
    /// Executor-backed result with explicit runtime receipts.
    ExecutorBackedResult,
    /// Source-grounded statement.
    SourceGroundedStatement,
    /// Verification or proof implication.
    Verification,
    /// Plugin publication or marketplace claim.
    PluginPublication,
    /// Public plugin universality claim.
    PublicPluginUniversality,
    /// Arbitrary software-capability claim.
    ArbitrarySoftwareCapability,
    /// Hidden execution claim without runtime receipts.
    HiddenExecutionWithoutRuntimeReceipt,
}

/// Machine-readable served posture for the first host-native plugin-conditioned lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeServedPosture {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable posture identifier.
    pub posture_id: String,
    /// Stable posture version.
    pub posture_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Bound capability-matrix identifier.
    pub capability_matrix_id: String,
    /// Bound capability-matrix version.
    pub capability_matrix_version: String,
    /// Bound capability-matrix digest.
    pub capability_matrix_digest: String,
    /// Bound model-artifact identifier.
    pub model_artifact_id: String,
    /// Bound model-artifact digest.
    pub model_artifact_digest: String,
    /// Bound evaluation-receipt identifier.
    pub evaluation_receipt_id: String,
    /// Bound evaluation-receipt digest.
    pub evaluation_receipt_digest: String,
    /// Human-readable capability doc carried by the posture.
    pub capability_doc_ref: String,
    /// Served-evidence doc inherited by the posture.
    pub served_evidence_doc_ref: String,
    /// Served-output claim doc inherited by the posture.
    pub served_output_claim_doc_ref: String,
    /// Current visibility posture for the lane.
    pub visibility_posture: String,
    /// Route labels the lane may show in served behavior.
    pub supported_route_labels: Vec<PsionPluginRouteLabel>,
    /// Claim surfaces allowed for the lane.
    pub supported_claim_surfaces: Vec<PsionPluginHostNativeClaimSurface>,
    /// Claim surfaces explicitly blocked for the lane.
    pub blocked_claim_surfaces: Vec<PsionPluginHostNativeClaimSurface>,
    /// Plugin classes explicitly not yet proved at service time.
    pub not_yet_proved_plugin_classes: Vec<PsionPluginClass>,
    /// Plugin classes explicitly unsupported at service time.
    pub unsupported_plugin_classes: Vec<PsionPluginClass>,
    /// Typed refusal reasons this posture keeps explicit.
    pub typed_refusal_reasons: Vec<String>,
    /// Execution-backed statement policy for the lane.
    pub execution_backed_statement_policy: String,
    /// Benchmark-backed statement policy for the lane.
    pub benchmark_backed_statement_policy: String,
    /// Short explanation of the served posture.
    pub summary: String,
    /// Stable digest over the posture.
    pub posture_digest: String,
}

impl PsionPluginHostNativeServedPosture {
    /// Writes the served posture to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
        write_json_file(self, output_path)
    }

    /// Validates the served posture against the published matrix and run bundle.
    pub fn validate_against_matrix_and_run_bundle(
        &self,
        matrix: &PsionPluginHostNativeCapabilityMatrix,
        run_bundle: &PsionPluginHostNativeReferenceRunBundle,
    ) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
        matrix.validate_against_run_bundle(run_bundle)?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_SCHEMA_VERSION,
            "psion_plugin_host_native_served_posture.schema_version",
        )?;
        check_string_match(
            self.posture_id.as_str(),
            "psion_plugin_host_native_served_posture",
            "psion_plugin_host_native_served_posture.posture_id",
        )?;
        check_string_match(
            self.posture_version.as_str(),
            "v1",
            "psion_plugin_host_native_served_posture.posture_version",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            run_bundle.lane_id.as_str(),
            "psion_plugin_host_native_served_posture.lane_id",
        )?;
        check_string_match(
            self.capability_matrix_id.as_str(),
            matrix.matrix_id.as_str(),
            "psion_plugin_host_native_served_posture.capability_matrix_id",
        )?;
        check_string_match(
            self.capability_matrix_version.as_str(),
            matrix.matrix_version.as_str(),
            "psion_plugin_host_native_served_posture.capability_matrix_version",
        )?;
        check_string_match(
            self.capability_matrix_digest.as_str(),
            matrix.matrix_digest.as_str(),
            "psion_plugin_host_native_served_posture.capability_matrix_digest",
        )?;
        check_string_match(
            self.model_artifact_id.as_str(),
            run_bundle.model_artifact.artifact_id.as_str(),
            "psion_plugin_host_native_served_posture.model_artifact_id",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            run_bundle.model_artifact.artifact_digest.as_str(),
            "psion_plugin_host_native_served_posture.model_artifact_digest",
        )?;
        check_string_match(
            self.evaluation_receipt_id.as_str(),
            run_bundle.evaluation_receipt.receipt_id.as_str(),
            "psion_plugin_host_native_served_posture.evaluation_receipt_id",
        )?;
        check_string_match(
            self.evaluation_receipt_digest.as_str(),
            run_bundle.evaluation_receipt.receipt_digest.as_str(),
            "psion_plugin_host_native_served_posture.evaluation_receipt_digest",
        )?;
        check_string_match(
            self.capability_doc_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_CAPABILITY_DOC_REF,
            "psion_plugin_host_native_served_posture.capability_doc_ref",
        )?;
        check_string_match(
            self.served_evidence_doc_ref.as_str(),
            PSION_SERVED_EVIDENCE_DOC_REF,
            "psion_plugin_host_native_served_posture.served_evidence_doc_ref",
        )?;
        check_string_match(
            self.served_output_claim_doc_ref.as_str(),
            PSION_SERVED_OUTPUT_CLAIMS_DOC_REF,
            "psion_plugin_host_native_served_posture.served_output_claim_doc_ref",
        )?;
        check_string_match(
            self.visibility_posture.as_str(),
            "operator_internal_only",
            "psion_plugin_host_native_served_posture.visibility_posture",
        )?;
        check_debug_slice_match(
            self.supported_route_labels.as_slice(),
            required_route_labels().as_slice(),
            "psion_plugin_host_native_served_posture.supported_route_labels",
        )?;
        check_debug_slice_match(
            self.supported_claim_surfaces.as_slice(),
            supported_claim_surfaces().as_slice(),
            "psion_plugin_host_native_served_posture.supported_claim_surfaces",
        )?;
        check_debug_slice_match(
            self.blocked_claim_surfaces.as_slice(),
            blocked_claim_surfaces().as_slice(),
            "psion_plugin_host_native_served_posture.blocked_claim_surfaces",
        )?;
        check_debug_slice_match(
            self.not_yet_proved_plugin_classes.as_slice(),
            [].as_slice(),
            "psion_plugin_host_native_served_posture.not_yet_proved_plugin_classes",
        )?;
        check_debug_slice_match(
            self.unsupported_plugin_classes.as_slice(),
            [
                PsionPluginClass::HostNativeNetworkedReadOnly,
                PsionPluginClass::HostNativeSecretBackedOrStateful,
                PsionPluginClass::GuestArtifactDigestBound,
            ]
            .as_slice(),
            "psion_plugin_host_native_served_posture.unsupported_plugin_classes",
        )?;
        reject_duplicate_strings(
            self.typed_refusal_reasons.as_slice(),
            "psion_plugin_host_native_served_posture.typed_refusal_reasons",
        )?;
        check_debug_slice_match(
            self.typed_refusal_reasons.as_slice(),
            required_typed_refusal_reasons().as_slice(),
            "psion_plugin_host_native_served_posture.typed_refusal_reasons",
        )?;
        check_string_match(
            self.execution_backed_statement_policy.as_str(),
            "executor_backed_result requires explicit runtime receipt refs; otherwise outputs remain learned_judgment or benchmark_backed_capability_claim only",
            "psion_plugin_host_native_served_posture.execution_backed_statement_policy",
        )?;
        check_string_match(
            self.benchmark_backed_statement_policy.as_str(),
            "benchmark_backed_capability_claim may cite only supported host-native rows published in the capability matrix and may not widen plugin class or publication posture",
            "psion_plugin_host_native_served_posture.benchmark_backed_statement_policy",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_host_native_served_posture.summary",
        )?;
        if self.posture_digest != stable_served_posture_digest(self) {
            return Err(PsionPluginHostNativeCapabilityMatrixError::DigestMismatch {
                kind: String::from("psion_plugin_host_native_served_posture"),
            });
        }
        Ok(())
    }
}

/// Builds the first host-native plugin-conditioned capability matrix from the committed run bundle.
pub fn record_psion_plugin_host_native_capability_matrix(
    run_bundle: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginHostNativeCapabilityMatrix, PsionPluginHostNativeCapabilityMatrixError> {
    let learned_plugin_ids =
        sorted_unique_strings(run_bundle.model_artifact.learned_plugin_ids.as_slice());
    let evaluation_receipt = &run_bundle.evaluation_receipt;
    let mut rows = vec![
        supported_row(
            "host_native_capability_free_local_deterministic.discovery_selection",
            "The lane may distinguish direct answer, admitted-plugin delegation, and unsupported-plugin refusal for the three proved local-deterministic plugins only.",
            discovery_selection_routes(),
            learned_plugin_ids.clone(),
            benchmark_evidence(PsionPluginBenchmarkFamily::DiscoverySelection, evaluation_receipt)?,
        ),
        supported_row(
            "host_native_capability_free_local_deterministic.argument_construction",
            "The lane may plan typed arguments or request missing structure for admitted local-deterministic plugins when the schema boundary is already explicit.",
            vec![
                PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
            ],
            learned_plugin_ids.clone(),
            benchmark_evidence(
                PsionPluginBenchmarkFamily::ArgumentConstruction,
                evaluation_receipt,
            )?,
        ),
        supported_row(
            "host_native_capability_free_local_deterministic.refusal_request_structure",
            "The lane may request missing structure, refuse unsupported plugin capability, and reject overdelegation inside the proved host-native boundary.",
            vec![
                PsionPluginRouteLabel::AnswerInLanguage,
                PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
            ],
            learned_plugin_ids.clone(),
            benchmark_evidence(
                PsionPluginBenchmarkFamily::RefusalRequestStructure,
                evaluation_receipt,
            )?,
        ),
        supported_row(
            "host_native_capability_free_local_deterministic.result_interpretation",
            "The lane may continue in language over receipt-backed results or typed refusals from the admitted local-deterministic plugins without inventing hidden retries or unseen execution.",
            vec![PsionPluginRouteLabel::AnswerInLanguage],
            learned_plugin_ids.clone(),
            benchmark_evidence(
                PsionPluginBenchmarkFamily::ResultInterpretation,
                evaluation_receipt,
            )?,
        ),
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("host_native_networked_read_only.substrate_proved_outside_v1"),
            posture: PsionPluginHostNativeCapabilityPosture::Unsupported,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PluginClassBoundary,
            plugin_class: Some(PsionPluginClass::HostNativeNetworkedReadOnly),
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "The networked_read_only class now has one bounded user-authored substrate proof, but it remains outside the first trained host-native capability publication until a later learned lane explicitly trains and evaluates it.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from(
                "host_native_capability_free_local_deterministic.sequencing_multi_call",
            ),
            posture: PsionPluginHostNativeCapabilityPosture::Unsupported,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PluginUseRegion,
            plugin_class: Some(PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic),
            route_labels: vec![PsionPluginRouteLabel::DelegateToAdmittedPlugin],
            admitted_plugin_ids: learned_plugin_ids.clone(),
            benchmark_evidence: Some(benchmark_evidence(
                PsionPluginBenchmarkFamily::SequencingMultiCall,
                evaluation_receipt,
            )?),
            detail: String::from(
                "The first host-native publication keeps multi-call sequencing outside supported v1 because the current bounded benchmark receipt has zero eligible in-boundary items.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("host_native_secret_backed_or_stateful"),
            posture: PsionPluginHostNativeCapabilityPosture::Unsupported,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PluginClassBoundary,
            plugin_class: Some(PsionPluginClass::HostNativeSecretBackedOrStateful),
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "Secret-backed and stateful host-native plugins remain later bounded substrate work and are not part of the first learned publication.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("guest_artifact_digest_bound"),
            posture: PsionPluginHostNativeCapabilityPosture::Unsupported,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PluginClassBoundary,
            plugin_class: Some(PsionPluginClass::GuestArtifactDigestBound),
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "Guest-artifact plugin support remains a later separate bounded lane and is not present-tense truth for the first host-native learned publication.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("plugin_publication_or_marketplace"),
            posture: PsionPluginHostNativeCapabilityPosture::Blocked,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PublicationBoundary,
            plugin_class: None,
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "The trained lane does not imply plugin publication, starter-plugin marketplace closure, or product-level plugin admission rights.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("public_plugin_universality"),
            posture: PsionPluginHostNativeCapabilityPosture::Blocked,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::PublicationBoundary,
            plugin_class: None,
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "The trained lane does not imply public plugin universality or broad user-plugin closure beyond the current bounded operator-internal matrix.",
            ),
        },
        PsionPluginHostNativeCapabilityRow {
            region_id: String::from("arbitrary_software_capability"),
            posture: PsionPluginHostNativeCapabilityPosture::Blocked,
            claim_class: PsionPluginHostNativeCapabilityClaimClass::SoftwareCapabilityBoundary,
            plugin_class: None,
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            benchmark_evidence: None,
            detail: String::from(
                "The trained lane does not imply arbitrary software capability, arbitrary binary loading, or execution-in-the-weights claims.",
            ),
        },
    ];
    sort_rows(&mut rows);
    let mut matrix = PsionPluginHostNativeCapabilityMatrix {
        schema_version: String::from(PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_SCHEMA_VERSION),
        matrix_id: String::from("psion_plugin_host_native_capability_matrix"),
        matrix_version: String::from("v1"),
        lane_id: run_bundle.lane_id.clone(),
        model_artifact_id: run_bundle.model_artifact.artifact_id.clone(),
        model_artifact_digest: run_bundle.model_artifact.artifact_digest.clone(),
        evaluation_receipt_id: run_bundle.evaluation_receipt.receipt_id.clone(),
        evaluation_receipt_digest: run_bundle.evaluation_receipt.receipt_digest.clone(),
        claim_boundary_doc_ref: String::from(PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF),
        served_evidence_doc_ref: String::from(PSION_SERVED_EVIDENCE_DOC_REF),
        served_output_claim_doc_ref: String::from(PSION_SERVED_OUTPUT_CLAIMS_DOC_REF),
        rows,
        summary: String::from(
            "The first host-native plugin-conditioned capability matrix publishes only the proved local-deterministic plugin-use regions, keeps the newly proved networked_read_only substrate outside supported v1, and blocks publication or arbitrary software overread.",
        ),
        matrix_digest: String::new(),
    };
    matrix.matrix_digest = stable_capability_matrix_digest(&matrix);
    matrix.validate_against_run_bundle(run_bundle)?;
    Ok(matrix)
}

/// Builds the first host-native plugin-conditioned served posture from the published matrix.
pub fn record_psion_plugin_host_native_served_posture(
    matrix: &PsionPluginHostNativeCapabilityMatrix,
    run_bundle: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginHostNativeServedPosture, PsionPluginHostNativeCapabilityMatrixError> {
    let mut posture = PsionPluginHostNativeServedPosture {
        schema_version: String::from(PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_SCHEMA_VERSION),
        posture_id: String::from("psion_plugin_host_native_served_posture"),
        posture_version: String::from("v1"),
        lane_id: run_bundle.lane_id.clone(),
        capability_matrix_id: matrix.matrix_id.clone(),
        capability_matrix_version: matrix.matrix_version.clone(),
        capability_matrix_digest: matrix.matrix_digest.clone(),
        model_artifact_id: run_bundle.model_artifact.artifact_id.clone(),
        model_artifact_digest: run_bundle.model_artifact.artifact_digest.clone(),
        evaluation_receipt_id: run_bundle.evaluation_receipt.receipt_id.clone(),
        evaluation_receipt_digest: run_bundle.evaluation_receipt.receipt_digest.clone(),
        capability_doc_ref: String::from(PSION_PLUGIN_HOST_NATIVE_CAPABILITY_DOC_REF),
        served_evidence_doc_ref: String::from(PSION_SERVED_EVIDENCE_DOC_REF),
        served_output_claim_doc_ref: String::from(PSION_SERVED_OUTPUT_CLAIMS_DOC_REF),
        visibility_posture: String::from("operator_internal_only"),
        supported_route_labels: required_route_labels(),
        supported_claim_surfaces: supported_claim_surfaces(),
        blocked_claim_surfaces: blocked_claim_surfaces(),
        not_yet_proved_plugin_classes: vec![],
        unsupported_plugin_classes: vec![
            PsionPluginClass::HostNativeNetworkedReadOnly,
            PsionPluginClass::HostNativeSecretBackedOrStateful,
            PsionPluginClass::GuestArtifactDigestBound,
        ],
        typed_refusal_reasons: required_typed_refusal_reasons(),
        execution_backed_statement_policy: String::from(
            "executor_backed_result requires explicit runtime receipt refs; otherwise outputs remain learned_judgment or benchmark_backed_capability_claim only",
        ),
        benchmark_backed_statement_policy: String::from(
            "benchmark_backed_capability_claim may cite only supported host-native rows published in the capability matrix and may not widen plugin class or publication posture",
        ),
        summary: String::from(
            "The first host-native served posture freezes an operator-internal learned-versus-executor statement policy for the bounded local-deterministic plugin lane, keeps the now-proved networked_read_only substrate outside the supported v1 learned surface, and leaves secret-backed, stateful, guest-artifact, publication, and arbitrary software claims out of scope.",
        ),
        posture_digest: String::new(),
    };
    posture.posture_digest = stable_served_posture_digest(&posture);
    posture.validate_against_matrix_and_run_bundle(matrix, run_bundle)?;
    Ok(posture)
}

/// Stable output path for the committed host-native capability matrix fixture.
#[must_use]
pub fn psion_plugin_host_native_capability_matrix_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF)
}

/// Stable output path for the committed host-native served posture fixture.
#[must_use]
pub fn psion_plugin_host_native_served_posture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF)
}

fn supported_row(
    region_id: &str,
    detail: &str,
    route_labels: Vec<PsionPluginRouteLabel>,
    admitted_plugin_ids: Vec<String>,
    benchmark_evidence: PsionPluginHostNativeBenchmarkEvidence,
) -> PsionPluginHostNativeCapabilityRow {
    PsionPluginHostNativeCapabilityRow {
        region_id: String::from(region_id),
        posture: PsionPluginHostNativeCapabilityPosture::Supported,
        claim_class: PsionPluginHostNativeCapabilityClaimClass::PluginUseRegion,
        plugin_class: Some(PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic),
        route_labels,
        admitted_plugin_ids,
        benchmark_evidence: Some(benchmark_evidence),
        detail: String::from(detail),
    }
}

fn benchmark_evidence(
    family: PsionPluginBenchmarkFamily,
    receipt: &crate::PsionPluginHostNativeEvaluationReceipt,
) -> Result<PsionPluginHostNativeBenchmarkEvidence, PsionPluginHostNativeCapabilityMatrixError> {
    let row = receipt
        .benchmark_deltas
        .iter()
        .find(|row| row.benchmark_family == family)
        .ok_or_else(
            || PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
                field: String::from("psion_plugin_host_native_capability_matrix.benchmark_family"),
                expected: format!("{family:?}"),
                actual: String::from("missing from host-native evaluation receipt"),
            },
        )?;
    Ok(PsionPluginHostNativeBenchmarkEvidence {
        benchmark_family: family,
        evaluation_receipt_id: receipt.receipt_id.clone(),
        evaluation_receipt_digest: receipt.receipt_digest.clone(),
        eligible_item_count: row.eligible_item_count,
        out_of_scope_item_count: row.out_of_scope_item_count,
        baseline_score_bps: row.baseline_score_bps,
        trained_score_bps: row.trained_score_bps,
        delta_bps: row.delta_bps,
    })
}

fn discovery_selection_routes() -> Vec<PsionPluginRouteLabel> {
    vec![
        PsionPluginRouteLabel::AnswerInLanguage,
        PsionPluginRouteLabel::DelegateToAdmittedPlugin,
        PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
    ]
}

fn required_route_labels() -> Vec<PsionPluginRouteLabel> {
    vec![
        PsionPluginRouteLabel::AnswerInLanguage,
        PsionPluginRouteLabel::DelegateToAdmittedPlugin,
        PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
        PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
    ]
}

fn supported_claim_surfaces() -> Vec<PsionPluginHostNativeClaimSurface> {
    vec![
        PsionPluginHostNativeClaimSurface::LearnedJudgment,
        PsionPluginHostNativeClaimSurface::BenchmarkBackedCapabilityClaim,
        PsionPluginHostNativeClaimSurface::ExecutorBackedResult,
    ]
}

fn blocked_claim_surfaces() -> Vec<PsionPluginHostNativeClaimSurface> {
    vec![
        PsionPluginHostNativeClaimSurface::SourceGroundedStatement,
        PsionPluginHostNativeClaimSurface::Verification,
        PsionPluginHostNativeClaimSurface::PluginPublication,
        PsionPluginHostNativeClaimSurface::PublicPluginUniversality,
        PsionPluginHostNativeClaimSurface::ArbitrarySoftwareCapability,
        PsionPluginHostNativeClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
    ]
}

fn required_typed_refusal_reasons() -> Vec<String> {
    vec![
        String::from("plugin_class_not_yet_proved"),
        String::from("plugin_capability_outside_admitted_set"),
        String::from("missing_required_structured_input"),
        String::from("publication_or_arbitrary_loading_claim_blocked"),
        String::from("secret_backed_or_stateful_class_not_enabled"),
    ]
}

fn sort_rows(rows: &mut [PsionPluginHostNativeCapabilityRow]) {
    rows.sort_by(|left, right| left.region_id.cmp(&right.region_id));
}

fn stable_capability_matrix_digest(matrix: &PsionPluginHostNativeCapabilityMatrix) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_host_native_capability_matrix|");
    hasher.update(matrix.schema_version.as_bytes());
    hasher.update(matrix.matrix_id.as_bytes());
    hasher.update(matrix.matrix_version.as_bytes());
    hasher.update(matrix.lane_id.as_bytes());
    hasher.update(matrix.model_artifact_id.as_bytes());
    hasher.update(matrix.model_artifact_digest.as_bytes());
    hasher.update(matrix.evaluation_receipt_id.as_bytes());
    hasher.update(matrix.evaluation_receipt_digest.as_bytes());
    hasher.update(matrix.claim_boundary_doc_ref.as_bytes());
    hasher.update(matrix.served_evidence_doc_ref.as_bytes());
    hasher.update(matrix.served_output_claim_doc_ref.as_bytes());
    for row in &matrix.rows {
        hasher.update(
            serde_json::to_vec(row)
                .expect("host-native capability row serialization should succeed"),
        );
    }
    hasher.update(matrix.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_served_posture_digest(posture: &PsionPluginHostNativeServedPosture) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_host_native_served_posture|");
    hasher.update(posture.schema_version.as_bytes());
    hasher.update(posture.posture_id.as_bytes());
    hasher.update(posture.posture_version.as_bytes());
    hasher.update(posture.lane_id.as_bytes());
    hasher.update(posture.capability_matrix_id.as_bytes());
    hasher.update(posture.capability_matrix_version.as_bytes());
    hasher.update(posture.capability_matrix_digest.as_bytes());
    hasher.update(posture.model_artifact_id.as_bytes());
    hasher.update(posture.model_artifact_digest.as_bytes());
    hasher.update(posture.evaluation_receipt_id.as_bytes());
    hasher.update(posture.evaluation_receipt_digest.as_bytes());
    hasher.update(posture.capability_doc_ref.as_bytes());
    hasher.update(posture.served_evidence_doc_ref.as_bytes());
    hasher.update(posture.served_output_claim_doc_ref.as_bytes());
    hasher.update(posture.visibility_posture.as_bytes());
    for route in &posture.supported_route_labels {
        hasher.update(format!("{route:?}").as_bytes());
    }
    for surface in &posture.supported_claim_surfaces {
        hasher.update(format!("{surface:?}").as_bytes());
    }
    for surface in &posture.blocked_claim_surfaces {
        hasher.update(format!("{surface:?}").as_bytes());
    }
    for plugin_class in &posture.not_yet_proved_plugin_classes {
        hasher.update(format!("{plugin_class:?}").as_bytes());
    }
    for plugin_class in &posture.unsupported_plugin_classes {
        hasher.update(format!("{plugin_class:?}").as_bytes());
    }
    for reason in &posture.typed_refusal_reasons {
        hasher.update(reason.as_bytes());
    }
    hasher.update(posture.execution_backed_statement_policy.as_bytes());
    hasher.update(posture.benchmark_backed_statement_policy.as_bytes());
    hasher.update(posture.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sorted_unique_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn write_json_file<T: Serialize>(
    value: &T,
    output_path: impl AsRef<Path>,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginHostNativeCapabilityMatrixError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        PsionPluginHostNativeCapabilityMatrixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    if value.trim().is_empty() {
        return Err(PsionPluginHostNativeCapabilityMatrixError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn ensure_bool_true(
    value: bool,
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    if value {
        Ok(())
    } else {
        Err(PsionPluginHostNativeCapabilityMatrixError::MissingField {
            field: field.to_string(),
        })
    }
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    if actual == expected {
        Ok(())
    } else {
        Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn check_option_match<T: std::fmt::Debug + PartialEq>(
    actual: Option<T>,
    expected: Option<T>,
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    if actual == expected {
        Ok(())
    } else {
        Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
            field: field.to_string(),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}

fn check_debug_slice_match<T: std::fmt::Debug + PartialEq>(
    actual: &[T],
    expected: &[T],
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    if actual == expected {
        Ok(())
    } else {
        Err(PsionPluginHostNativeCapabilityMatrixError::FieldMismatch {
            field: field.to_string(),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginHostNativeCapabilityMatrixError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginHostNativeCapabilityMatrixError::DuplicateValue {
                field: field.to_string(),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

/// Errors returned by the host-native capability matrix and served posture builders.
#[derive(Debug, Error)]
pub enum PsionPluginHostNativeCapabilityMatrixError {
    /// One required field was missing or false.
    #[error("Psion plugin host-native capability publication is missing `{field}`")]
    MissingField {
        /// Missing field path.
        field: String,
    },
    /// One field drifted from the required value.
    #[error(
        "Psion plugin host-native capability publication field `{field}` expected `{expected}` but found `{actual}`"
    )]
    FieldMismatch {
        /// Drifted field path.
        field: String,
        /// Required value.
        expected: String,
        /// Observed value.
        actual: String,
    },
    /// One repeated value appeared where uniqueness is required.
    #[error("Psion plugin host-native capability publication repeated `{value}` in `{field}`")]
    DuplicateValue {
        /// Field path carrying the duplicate.
        field: String,
        /// Repeated value.
        value: String,
    },
    /// Stable digest drifted from the computed value.
    #[error("Psion plugin host-native capability publication digest drifted for `{kind}`")]
    DigestMismatch {
        /// Digest kind that drifted.
        kind: String,
    },
    /// Failed to create the parent directory for one fixture.
    #[error("failed to create capability-publication directory `{path}`: {error}")]
    CreateDir {
        /// Parent directory path.
        path: String,
        /// I/O error.
        error: std::io::Error,
    },
    /// Failed to write one fixture.
    #[error("failed to write capability-publication fixture `{path}`: {error}")]
    Write {
        /// Output file path.
        path: String,
        /// I/O error.
        error: std::io::Error,
    },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        psion_plugin_host_native_capability_matrix_path,
        psion_plugin_host_native_served_posture_path,
        record_psion_plugin_host_native_capability_matrix,
        record_psion_plugin_host_native_served_posture, PsionPluginHostNativeCapabilityMatrix,
        PsionPluginHostNativeCapabilityPosture, PsionPluginHostNativeServedPosture,
        PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF,
        PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF,
    };
    use crate::{
        psion_plugin_host_native_reference_run_bundle_path, PsionPluginHostNativeReferenceRunBundle,
    };

    fn run_bundle() -> PsionPluginHostNativeReferenceRunBundle {
        serde_json::from_slice(
            &std::fs::read(psion_plugin_host_native_reference_run_bundle_path())
                .expect("committed host-native run bundle fixture should exist"),
        )
        .expect("committed host-native run bundle fixture should parse")
    }

    #[test]
    fn host_native_capability_matrix_validates_against_run_bundle() -> Result<(), Box<dyn Error>> {
        let run_bundle = run_bundle();
        let matrix = record_psion_plugin_host_native_capability_matrix(&run_bundle)?;
        matrix.validate_against_run_bundle(&run_bundle)?;
        Ok(())
    }

    #[test]
    fn host_native_served_posture_validates_against_matrix_and_run_bundle(
    ) -> Result<(), Box<dyn Error>> {
        let run_bundle = run_bundle();
        let matrix = record_psion_plugin_host_native_capability_matrix(&run_bundle)?;
        let posture = record_psion_plugin_host_native_served_posture(&matrix, &run_bundle)?;
        posture.validate_against_matrix_and_run_bundle(&matrix, &run_bundle)?;
        Ok(())
    }

    #[test]
    fn committed_host_native_capability_publication_fixtures_validate() -> Result<(), Box<dyn Error>>
    {
        let run_bundle = run_bundle();
        let matrix: PsionPluginHostNativeCapabilityMatrix = serde_json::from_slice(
            &std::fs::read(psion_plugin_host_native_capability_matrix_path())?,
        )?;
        let posture: PsionPluginHostNativeServedPosture = serde_json::from_slice(&std::fs::read(
            psion_plugin_host_native_served_posture_path(),
        )?)?;
        matrix.validate_against_run_bundle(&run_bundle)?;
        posture.validate_against_matrix_and_run_bundle(&matrix, &run_bundle)?;
        Ok(())
    }

    #[test]
    fn matrix_keeps_networked_and_fetch_text_out_of_supported_surface() -> Result<(), Box<dyn Error>>
    {
        let run_bundle = run_bundle();
        let matrix = record_psion_plugin_host_native_capability_matrix(&run_bundle)?;
        assert!(matrix.rows.iter().any(|row| {
            row.posture == PsionPluginHostNativeCapabilityPosture::Unsupported
                && row.region_id == "host_native_networked_read_only.substrate_proved_outside_v1"
        }));
        assert!(matrix.rows.iter().all(|row| {
            row.admitted_plugin_ids
                .iter()
                .all(|plugin_id| plugin_id != "plugin.http.fetch_text")
        }));
        Ok(())
    }

    #[test]
    fn fixture_refs_match_committed_paths() {
        assert!(psion_plugin_host_native_capability_matrix_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_REF));
        assert!(psion_plugin_host_native_served_posture_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_HOST_NATIVE_SERVED_POSTURE_REF));
    }
}
