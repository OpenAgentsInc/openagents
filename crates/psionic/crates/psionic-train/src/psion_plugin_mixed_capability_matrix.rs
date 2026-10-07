use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{PsionPluginClass, PsionPluginRouteLabel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionPluginBenchmarkFamily, PsionPluginBenchmarkMetricKind,
    PsionPluginGuestPluginBenchmarkBundle, PsionPluginMixedReferenceRunBundle,
    PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF, PSION_SERVED_EVIDENCE_DOC_REF,
    PSION_SERVED_OUTPUT_CLAIMS_DOC_REF,
};

/// Stable schema version for the mixed plugin-conditioned capability matrix.
pub const PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_mixed_capability_matrix.v2";
/// Stable schema version for the mixed plugin-conditioned served posture.
pub const PSION_PLUGIN_MIXED_SERVED_POSTURE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_mixed_served_posture.v2";
/// Stable committed fixture ref for the mixed plugin-conditioned capability matrix.
pub const PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_REF: &str =
    "fixtures/psion/plugins/capability/psion_plugin_mixed_capability_matrix_v2.json";
/// Stable committed fixture ref for the mixed plugin-conditioned served posture.
pub const PSION_PLUGIN_MIXED_SERVED_POSTURE_REF: &str =
    "fixtures/psion/plugins/serve/psion_plugin_mixed_served_posture_v2.json";
/// Stable human-readable capability matrix doc for the mixed publication.
pub const PSION_PLUGIN_MIXED_CAPABILITY_DOC_REF: &str =
    "docs/PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_V2.md";

/// Posture published for one mixed plugin-conditioned capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginMixedCapabilityPosture {
    Supported,
    Unsupported,
    Blocked,
}

/// Claim class carried by one mixed plugin-conditioned capability row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginMixedCapabilityClaimClass {
    PluginUseRegion,
    PluginClassBoundary,
    PublicationBoundary,
    SoftwareCapabilityBoundary,
}

/// Mixed-lane comparison evidence bound to one capability row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedBenchmarkEvidence {
    pub benchmark_family: PsionPluginBenchmarkFamily,
    pub evaluation_receipt_id: String,
    pub evaluation_receipt_digest: String,
    pub comparison_label: String,
    pub comparison_reference_run_bundle_ref: String,
    pub comparison_reference_run_bundle_digest: String,
    pub eligible_item_count: u32,
    pub out_of_scope_item_count: u32,
    pub host_native_reference_score_bps: u32,
    pub mixed_score_bps: u32,
    pub delta_vs_host_native_bps: i32,
}

impl PsionPluginMixedBenchmarkEvidence {
    fn validate(
        &self,
        run_bundle: &PsionPluginMixedReferenceRunBundle,
        field: &str,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        check_string_match(
            self.evaluation_receipt_id.as_str(),
            run_bundle.evaluation_receipt.receipt_id.as_str(),
            format!("{field}.evaluation_receipt_id").as_str(),
        )?;
        check_string_match(
            self.evaluation_receipt_digest.as_str(),
            run_bundle.evaluation_receipt.receipt_digest.as_str(),
            format!("{field}.evaluation_receipt_digest").as_str(),
        )?;
        check_string_match(
            self.comparison_label.as_str(),
            run_bundle.evaluation_receipt.comparison_label.as_str(),
            format!("{field}.comparison_label").as_str(),
        )?;
        check_string_match(
            self.comparison_reference_run_bundle_ref.as_str(),
            run_bundle
                .evaluation_receipt
                .comparison_reference_run_bundle_ref
                .as_str(),
            format!("{field}.comparison_reference_run_bundle_ref").as_str(),
        )?;
        check_string_match(
            self.comparison_reference_run_bundle_digest.as_str(),
            run_bundle
                .evaluation_receipt
                .comparison_reference_run_bundle_digest
                .as_str(),
            format!("{field}.comparison_reference_run_bundle_digest").as_str(),
        )?;
        if self.host_native_reference_score_bps > 10_000 || self.mixed_score_bps > 10_000 {
            return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
                field: format!("{field}.score_bps"),
                expected: String::from("at most 10000"),
                actual: format!(
                    "{} / {}",
                    self.host_native_reference_score_bps, self.mixed_score_bps
                ),
            });
        }
        if self.delta_vs_host_native_bps
            != self.mixed_score_bps as i32 - self.host_native_reference_score_bps as i32
        {
            return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
                field: format!("{field}.delta_vs_host_native_bps"),
                expected: (self.mixed_score_bps as i32 - self.host_native_reference_score_bps as i32)
                    .to_string(),
                actual: self.delta_vs_host_native_bps.to_string(),
            });
        }
        Ok(())
    }
}

/// Guest capability-boundary benchmark evidence bound to one capability row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestCapabilityMetricEvidence {
    pub package_id: String,
    pub package_digest: String,
    pub receipt_id: String,
    pub receipt_digest: String,
    pub metric_kind: PsionPluginBenchmarkMetricKind,
    pub value_bps: u32,
}

impl PsionPluginGuestCapabilityMetricEvidence {
    fn validate(
        &self,
        guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
        field: &str,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        check_string_match(
            self.package_id.as_str(),
            guest_benchmark_bundle.package.package_id.as_str(),
            format!("{field}.package_id").as_str(),
        )?;
        check_string_match(
            self.package_digest.as_str(),
            guest_benchmark_bundle.package.package_digest.as_str(),
            format!("{field}.package_digest").as_str(),
        )?;
        check_string_match(
            self.receipt_id.as_str(),
            guest_benchmark_bundle.receipt.receipt_id.as_str(),
            format!("{field}.receipt_id").as_str(),
        )?;
        check_string_match(
            self.receipt_digest.as_str(),
            guest_benchmark_bundle.receipt.receipt_digest.as_str(),
            format!("{field}.receipt_digest").as_str(),
        )?;
        validate_bps(self.value_bps, format!("{field}.value_bps").as_str())?;
        let expected_metric = guest_benchmark_bundle
            .receipt
            .observed_metrics
            .iter()
            .find(|metric| metric.kind == self.metric_kind)
            .ok_or_else(|| PsionPluginMixedCapabilityMatrixError::MissingField {
                field: format!("{field}.metric_kind"),
            })?;
        if self.value_bps != expected_metric.value_bps {
            return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
                field: format!("{field}.value_bps"),
                expected: expected_metric.value_bps.to_string(),
                actual: self.value_bps.to_string(),
            });
        }
        Ok(())
    }
}

/// Evidence attached to one mixed capability row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence_kind", rename_all = "snake_case")]
pub enum PsionPluginMixedCapabilityEvidence {
    MixedBenchmark(PsionPluginMixedBenchmarkEvidence),
    GuestCapabilityMetric(PsionPluginGuestCapabilityMetricEvidence),
}

impl PsionPluginMixedCapabilityEvidence {
    fn validate(
        &self,
        run_bundle: &PsionPluginMixedReferenceRunBundle,
        guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
        field: &str,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        match self {
            Self::MixedBenchmark(evidence) => evidence.validate(run_bundle, field),
            Self::GuestCapabilityMetric(evidence) => {
                evidence.validate(guest_benchmark_bundle, field)
            }
        }
    }
}

/// One explicit region or boundary row in the mixed capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedCapabilityRow {
    pub region_id: String,
    pub posture: PsionPluginMixedCapabilityPosture,
    pub claim_class: PsionPluginMixedCapabilityClaimClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_class: Option<PsionPluginClass>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_labels: Vec<PsionPluginRouteLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admitted_plugin_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<PsionPluginMixedCapabilityEvidence>,
    pub detail: String,
}

/// Machine-readable capability matrix for the mixed plugin-conditioned lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedCapabilityMatrix {
    pub schema_version: String,
    pub matrix_id: String,
    pub matrix_version: String,
    pub lane_id: String,
    pub model_artifact_id: String,
    pub model_artifact_digest: String,
    pub mixed_evaluation_receipt_id: String,
    pub mixed_evaluation_receipt_digest: String,
    pub guest_benchmark_package_id: String,
    pub guest_benchmark_package_digest: String,
    pub guest_benchmark_receipt_id: String,
    pub guest_benchmark_receipt_digest: String,
    pub claim_boundary_doc_ref: String,
    pub served_evidence_doc_ref: String,
    pub served_output_claim_doc_ref: String,
    pub rows: Vec<PsionPluginMixedCapabilityRow>,
    pub summary: String,
    pub matrix_digest: String,
}

impl PsionPluginMixedCapabilityMatrix {
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        write_json_file(self, output_path)
    }

    pub fn validate_against_inputs(
        &self,
        run_bundle: &PsionPluginMixedReferenceRunBundle,
        guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_SCHEMA_VERSION,
            "psion_plugin_mixed_capability_matrix.schema_version",
        )?;
        check_string_match(
            self.matrix_id.as_str(),
            "psion_plugin_mixed_capability_matrix",
            "psion_plugin_mixed_capability_matrix.matrix_id",
        )?;
        check_string_match(
            self.matrix_version.as_str(),
            "v2",
            "psion_plugin_mixed_capability_matrix.matrix_version",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            run_bundle.lane_id.as_str(),
            "psion_plugin_mixed_capability_matrix.lane_id",
        )?;
        check_string_match(
            self.model_artifact_id.as_str(),
            run_bundle.model_artifact.artifact_id.as_str(),
            "psion_plugin_mixed_capability_matrix.model_artifact_id",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            run_bundle.model_artifact.artifact_digest.as_str(),
            "psion_plugin_mixed_capability_matrix.model_artifact_digest",
        )?;
        check_string_match(
            self.mixed_evaluation_receipt_id.as_str(),
            run_bundle.evaluation_receipt.receipt_id.as_str(),
            "psion_plugin_mixed_capability_matrix.mixed_evaluation_receipt_id",
        )?;
        check_string_match(
            self.mixed_evaluation_receipt_digest.as_str(),
            run_bundle.evaluation_receipt.receipt_digest.as_str(),
            "psion_plugin_mixed_capability_matrix.mixed_evaluation_receipt_digest",
        )?;
        check_string_match(
            self.guest_benchmark_package_id.as_str(),
            guest_benchmark_bundle.package.package_id.as_str(),
            "psion_plugin_mixed_capability_matrix.guest_benchmark_package_id",
        )?;
        check_string_match(
            self.guest_benchmark_package_digest.as_str(),
            guest_benchmark_bundle.package.package_digest.as_str(),
            "psion_plugin_mixed_capability_matrix.guest_benchmark_package_digest",
        )?;
        check_string_match(
            self.guest_benchmark_receipt_id.as_str(),
            guest_benchmark_bundle.receipt.receipt_id.as_str(),
            "psion_plugin_mixed_capability_matrix.guest_benchmark_receipt_id",
        )?;
        check_string_match(
            self.guest_benchmark_receipt_digest.as_str(),
            guest_benchmark_bundle.receipt.receipt_digest.as_str(),
            "psion_plugin_mixed_capability_matrix.guest_benchmark_receipt_digest",
        )?;
        check_string_match(
            self.claim_boundary_doc_ref.as_str(),
            PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF,
            "psion_plugin_mixed_capability_matrix.claim_boundary_doc_ref",
        )?;
        check_string_match(
            self.served_evidence_doc_ref.as_str(),
            PSION_SERVED_EVIDENCE_DOC_REF,
            "psion_plugin_mixed_capability_matrix.served_evidence_doc_ref",
        )?;
        check_string_match(
            self.served_output_claim_doc_ref.as_str(),
            PSION_SERVED_OUTPUT_CLAIMS_DOC_REF,
            "psion_plugin_mixed_capability_matrix.served_output_claim_doc_ref",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_mixed_capability_matrix.summary",
        )?;
        if self.rows.is_empty() {
            return Err(PsionPluginMixedCapabilityMatrixError::MissingField {
                field: String::from("psion_plugin_mixed_capability_matrix.rows"),
            });
        }
        let mut seen_region_ids = BTreeSet::new();
        let host_native_plugin_ids = host_native_plugin_ids(run_bundle);
        let guest_plugin_ids = guest_plugin_ids(run_bundle);
        let mut supported_region_count = 0_u32;
        let mut saw_local_class_supported = false;
        let mut saw_networked_class_supported = false;
        let mut saw_guest_class_supported = false;
        let mut saw_secret_backed_unsupported = false;
        let mut saw_guest_loading_blocked = false;
        let mut saw_publication_blocked = false;
        let mut saw_universality_blocked = false;
        let mut saw_arbitrary_software_blocked = false;
        for (index, row) in self.rows.iter().enumerate() {
            let field = format!("psion_plugin_mixed_capability_matrix.rows[{index}]");
            ensure_nonempty(row.region_id.as_str(), format!("{field}.region_id").as_str())?;
            ensure_nonempty(row.detail.as_str(), format!("{field}.detail").as_str())?;
            if !seen_region_ids.insert(row.region_id.as_str()) {
                return Err(PsionPluginMixedCapabilityMatrixError::DuplicateValue {
                    field: String::from("psion_plugin_mixed_capability_matrix.rows.region_id"),
                    value: row.region_id.clone(),
                });
            }
            reject_duplicate_strings(
                row.admitted_plugin_ids.as_slice(),
                format!("{field}.admitted_plugin_ids").as_str(),
            )?;
            if let Some(evidence) = &row.evidence {
                evidence.validate(run_bundle, guest_benchmark_bundle, format!("{field}.evidence").as_str())?;
            }
            match row.region_id.as_str() {
                "host_native_capability_free_local_deterministic" => {
                    saw_local_class_supported = row.posture == PsionPluginMixedCapabilityPosture::Supported
                        && row.plugin_class == Some(PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic);
                }
                "host_native_networked_read_only" => {
                    saw_networked_class_supported = row.posture == PsionPluginMixedCapabilityPosture::Supported
                        && row.plugin_class == Some(PsionPluginClass::HostNativeNetworkedReadOnly);
                }
                "guest_artifact_digest_bound" => {
                    saw_guest_class_supported = row.posture == PsionPluginMixedCapabilityPosture::Supported
                        && row.plugin_class == Some(PsionPluginClass::GuestArtifactDigestBound);
                }
                "host_native_secret_backed_or_stateful" => {
                    saw_secret_backed_unsupported = row.posture
                        == PsionPluginMixedCapabilityPosture::Unsupported
                        && row.plugin_class == Some(PsionPluginClass::HostNativeSecretBackedOrStateful);
                }
                "guest_artifact_generic_loading_or_unadmitted_digest" => {
                    saw_guest_loading_blocked =
                        row.posture == PsionPluginMixedCapabilityPosture::Blocked;
                }
                "plugin_publication_or_marketplace" => {
                    saw_publication_blocked =
                        row.posture == PsionPluginMixedCapabilityPosture::Blocked;
                }
                "public_plugin_universality" => {
                    saw_universality_blocked =
                        row.posture == PsionPluginMixedCapabilityPosture::Blocked;
                }
                "arbitrary_software_capability" => {
                    saw_arbitrary_software_blocked =
                        row.posture == PsionPluginMixedCapabilityPosture::Blocked;
                }
                _ => {}
            }
            if row.posture == PsionPluginMixedCapabilityPosture::Supported {
                supported_region_count += 1;
            }
            match row.region_id.as_str() {
                "host_native_mixed.discovery_selection"
                | "host_native_mixed.argument_construction"
                | "host_native_mixed.sequencing_multi_call"
                | "host_native_mixed.refusal_request_structure"
                | "host_native_mixed.result_interpretation" => {
                    check_debug_slice_match(
                        sorted_unique_strings(row.admitted_plugin_ids.as_slice()).as_slice(),
                        host_native_plugin_ids.as_slice(),
                        format!("{field}.admitted_plugin_ids").as_str(),
                    )?;
                    if !matches!(row.evidence, Some(PsionPluginMixedCapabilityEvidence::MixedBenchmark(_)))
                    {
                        return Err(PsionPluginMixedCapabilityMatrixError::MissingField {
                            field: format!("{field}.evidence"),
                        });
                    }
                }
                "guest_artifact_digest_bound" | "guest_artifact_digest_bound.admitted_use" => {
                    check_debug_slice_match(
                        sorted_unique_strings(row.admitted_plugin_ids.as_slice()).as_slice(),
                        guest_plugin_ids.as_slice(),
                        format!("{field}.admitted_plugin_ids").as_str(),
                    )?;
                }
                _ => {}
            }
        }
        ensure_bool_true(
            supported_region_count > 0,
            "psion_plugin_mixed_capability_matrix.supported_rows",
        )?;
        ensure_bool_true(
            saw_local_class_supported,
            "psion_plugin_mixed_capability_matrix.local_class_supported",
        )?;
        ensure_bool_true(
            saw_networked_class_supported,
            "psion_plugin_mixed_capability_matrix.networked_class_supported",
        )?;
        ensure_bool_true(
            saw_guest_class_supported,
            "psion_plugin_mixed_capability_matrix.guest_class_supported",
        )?;
        ensure_bool_true(
            saw_secret_backed_unsupported,
            "psion_plugin_mixed_capability_matrix.secret_backed_unsupported",
        )?;
        ensure_bool_true(
            saw_guest_loading_blocked,
            "psion_plugin_mixed_capability_matrix.guest_loading_blocked",
        )?;
        ensure_bool_true(
            saw_publication_blocked,
            "psion_plugin_mixed_capability_matrix.publication_blocked",
        )?;
        ensure_bool_true(
            saw_universality_blocked,
            "psion_plugin_mixed_capability_matrix.universality_blocked",
        )?;
        ensure_bool_true(
            saw_arbitrary_software_blocked,
            "psion_plugin_mixed_capability_matrix.arbitrary_software_blocked",
        )?;
        if self.matrix_digest != stable_capability_matrix_digest(self) {
            return Err(PsionPluginMixedCapabilityMatrixError::DigestMismatch {
                kind: String::from("psion_plugin_mixed_capability_matrix"),
            });
        }
        Ok(())
    }
}

/// Visible claim surface allowed or blocked by the mixed served posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginMixedClaimSurface {
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

/// Machine-readable served posture for the mixed plugin-conditioned lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedServedPosture {
    pub schema_version: String,
    pub posture_id: String,
    pub posture_version: String,
    pub lane_id: String,
    pub capability_matrix_id: String,
    pub capability_matrix_version: String,
    pub capability_matrix_digest: String,
    pub model_artifact_id: String,
    pub model_artifact_digest: String,
    pub mixed_evaluation_receipt_id: String,
    pub mixed_evaluation_receipt_digest: String,
    pub guest_benchmark_receipt_id: String,
    pub guest_benchmark_receipt_digest: String,
    pub capability_doc_ref: String,
    pub served_evidence_doc_ref: String,
    pub served_output_claim_doc_ref: String,
    pub visibility_posture: String,
    pub supported_route_labels: Vec<PsionPluginRouteLabel>,
    pub supported_claim_surfaces: Vec<PsionPluginMixedClaimSurface>,
    pub blocked_claim_surfaces: Vec<PsionPluginMixedClaimSurface>,
    pub not_yet_proved_plugin_classes: Vec<PsionPluginClass>,
    pub unsupported_plugin_classes: Vec<PsionPluginClass>,
    pub typed_refusal_reasons: Vec<String>,
    pub execution_backed_statement_policy: String,
    pub benchmark_backed_statement_policy: String,
    pub summary: String,
    pub posture_digest: String,
}

impl PsionPluginMixedServedPosture {
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        write_json_file(self, output_path)
    }

    pub fn validate_against_inputs(
        &self,
        matrix: &PsionPluginMixedCapabilityMatrix,
        run_bundle: &PsionPluginMixedReferenceRunBundle,
        guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
    ) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
        matrix.validate_against_inputs(run_bundle, guest_benchmark_bundle)?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_MIXED_SERVED_POSTURE_SCHEMA_VERSION,
            "psion_plugin_mixed_served_posture.schema_version",
        )?;
        check_string_match(
            self.posture_id.as_str(),
            "psion_plugin_mixed_served_posture",
            "psion_plugin_mixed_served_posture.posture_id",
        )?;
        check_string_match(
            self.posture_version.as_str(),
            "v2",
            "psion_plugin_mixed_served_posture.posture_version",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            run_bundle.lane_id.as_str(),
            "psion_plugin_mixed_served_posture.lane_id",
        )?;
        check_string_match(
            self.capability_matrix_id.as_str(),
            matrix.matrix_id.as_str(),
            "psion_plugin_mixed_served_posture.capability_matrix_id",
        )?;
        check_string_match(
            self.capability_matrix_version.as_str(),
            matrix.matrix_version.as_str(),
            "psion_plugin_mixed_served_posture.capability_matrix_version",
        )?;
        check_string_match(
            self.capability_matrix_digest.as_str(),
            matrix.matrix_digest.as_str(),
            "psion_plugin_mixed_served_posture.capability_matrix_digest",
        )?;
        check_string_match(
            self.model_artifact_id.as_str(),
            run_bundle.model_artifact.artifact_id.as_str(),
            "psion_plugin_mixed_served_posture.model_artifact_id",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            run_bundle.model_artifact.artifact_digest.as_str(),
            "psion_plugin_mixed_served_posture.model_artifact_digest",
        )?;
        check_string_match(
            self.mixed_evaluation_receipt_id.as_str(),
            run_bundle.evaluation_receipt.receipt_id.as_str(),
            "psion_plugin_mixed_served_posture.mixed_evaluation_receipt_id",
        )?;
        check_string_match(
            self.mixed_evaluation_receipt_digest.as_str(),
            run_bundle.evaluation_receipt.receipt_digest.as_str(),
            "psion_plugin_mixed_served_posture.mixed_evaluation_receipt_digest",
        )?;
        check_string_match(
            self.guest_benchmark_receipt_id.as_str(),
            guest_benchmark_bundle.receipt.receipt_id.as_str(),
            "psion_plugin_mixed_served_posture.guest_benchmark_receipt_id",
        )?;
        check_string_match(
            self.guest_benchmark_receipt_digest.as_str(),
            guest_benchmark_bundle.receipt.receipt_digest.as_str(),
            "psion_plugin_mixed_served_posture.guest_benchmark_receipt_digest",
        )?;
        check_string_match(
            self.capability_doc_ref.as_str(),
            PSION_PLUGIN_MIXED_CAPABILITY_DOC_REF,
            "psion_plugin_mixed_served_posture.capability_doc_ref",
        )?;
        check_string_match(
            self.served_evidence_doc_ref.as_str(),
            PSION_SERVED_EVIDENCE_DOC_REF,
            "psion_plugin_mixed_served_posture.served_evidence_doc_ref",
        )?;
        check_string_match(
            self.served_output_claim_doc_ref.as_str(),
            PSION_SERVED_OUTPUT_CLAIMS_DOC_REF,
            "psion_plugin_mixed_served_posture.served_output_claim_doc_ref",
        )?;
        check_string_match(
            self.visibility_posture.as_str(),
            "operator_internal_only",
            "psion_plugin_mixed_served_posture.visibility_posture",
        )?;
        check_debug_slice_match(
            self.supported_route_labels.as_slice(),
            required_route_labels().as_slice(),
            "psion_plugin_mixed_served_posture.supported_route_labels",
        )?;
        check_debug_slice_match(
            self.supported_claim_surfaces.as_slice(),
            supported_claim_surfaces().as_slice(),
            "psion_plugin_mixed_served_posture.supported_claim_surfaces",
        )?;
        check_debug_slice_match(
            self.blocked_claim_surfaces.as_slice(),
            blocked_claim_surfaces().as_slice(),
            "psion_plugin_mixed_served_posture.blocked_claim_surfaces",
        )?;
        check_debug_slice_match(
            self.not_yet_proved_plugin_classes.as_slice(),
            [].as_slice(),
            "psion_plugin_mixed_served_posture.not_yet_proved_plugin_classes",
        )?;
        check_debug_slice_match(
            self.unsupported_plugin_classes.as_slice(),
            [PsionPluginClass::HostNativeSecretBackedOrStateful].as_slice(),
            "psion_plugin_mixed_served_posture.unsupported_plugin_classes",
        )?;
        reject_duplicate_strings(
            self.typed_refusal_reasons.as_slice(),
            "psion_plugin_mixed_served_posture.typed_refusal_reasons",
        )?;
        check_debug_slice_match(
            self.typed_refusal_reasons.as_slice(),
            required_typed_refusal_reasons().as_slice(),
            "psion_plugin_mixed_served_posture.typed_refusal_reasons",
        )?;
        check_string_match(
            self.execution_backed_statement_policy.as_str(),
            "executor_backed_result requires explicit runtime receipt refs; bounded guest capability rows do not imply hidden guest execution without those receipts",
            "psion_plugin_mixed_served_posture.execution_backed_statement_policy",
        )?;
        check_string_match(
            self.benchmark_backed_statement_policy.as_str(),
            "benchmark_backed_capability_claim may cite supported host-native mixed rows and the bounded guest admitted-use row only; blocked guest loading, publication, universality, and arbitrary-binary rows remain refusal-boundary statements",
            "psion_plugin_mixed_served_posture.benchmark_backed_statement_policy",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_mixed_served_posture.summary",
        )?;
        if self.posture_digest != stable_served_posture_digest(self) {
            return Err(PsionPluginMixedCapabilityMatrixError::DigestMismatch {
                kind: String::from("psion_plugin_mixed_served_posture"),
            });
        }
        Ok(())
    }
}

pub fn record_psion_plugin_mixed_capability_matrix(
    run_bundle: &PsionPluginMixedReferenceRunBundle,
    guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
) -> Result<PsionPluginMixedCapabilityMatrix, PsionPluginMixedCapabilityMatrixError> {
    let host_native_plugin_ids = host_native_plugin_ids(run_bundle);
    let local_plugin_ids = local_plugin_ids(run_bundle);
    let networked_plugin_ids = networked_plugin_ids(run_bundle);
    let guest_plugin_ids = guest_plugin_ids(run_bundle);
    let rows = vec![
        supported_host_native_region(
            "host_native_mixed.discovery_selection",
            "The mixed lane may route across the admitted host-native plugin set, including the bounded networked_read_only fetch plugin, without flattening guest-artifact boundaries into generic plugin support.",
            vec![
                PsionPluginRouteLabel::AnswerInLanguage,
                PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
            ],
            host_native_plugin_ids.clone(),
            mixed_benchmark_evidence(PsionPluginBenchmarkFamily::DiscoverySelection, run_bundle)?,
        ),
        supported_host_native_region(
            "host_native_mixed.argument_construction",
            "The mixed lane may construct typed arguments or request missing structure across the admitted host-native plugin set, including the bounded networked_read_only fetch plugin.",
            vec![
                PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
            ],
            host_native_plugin_ids.clone(),
            mixed_benchmark_evidence(PsionPluginBenchmarkFamily::ArgumentConstruction, run_bundle)?,
        ),
        supported_host_native_region(
            "host_native_mixed.sequencing_multi_call",
            "The mixed lane may now publish bounded multi-call sequencing over the admitted host-native plugin set because the mixed comparison receipt closes the prior zero-eligible-item gap.",
            vec![PsionPluginRouteLabel::DelegateToAdmittedPlugin],
            host_native_plugin_ids.clone(),
            mixed_benchmark_evidence(PsionPluginBenchmarkFamily::SequencingMultiCall, run_bundle)?,
        ),
        supported_host_native_region(
            "host_native_mixed.refusal_request_structure",
            "The mixed lane may request missing structure, refuse unsupported capability, and reject overdelegation across the admitted host-native plugin set while keeping publication and arbitrary-loading claims blocked.",
            vec![
                PsionPluginRouteLabel::AnswerInLanguage,
                PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
            ],
            host_native_plugin_ids.clone(),
            mixed_benchmark_evidence(
                PsionPluginBenchmarkFamily::RefusalRequestStructure,
                run_bundle,
            )?,
        ),
        supported_host_native_region(
            "host_native_mixed.result_interpretation",
            "The mixed lane may continue in language over receipt-backed host-native results across the admitted host-native plugin set without implying hidden execution.",
            vec![PsionPluginRouteLabel::AnswerInLanguage],
            host_native_plugin_ids.clone(),
            mixed_benchmark_evidence(PsionPluginBenchmarkFamily::ResultInterpretation, run_bundle)?,
        ),
        supported_class_row(
            "host_native_capability_free_local_deterministic",
            PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic,
            local_plugin_ids,
            "The local-deterministic host-native class remains inside the mixed supported publication.",
        ),
        supported_class_row(
            "host_native_networked_read_only",
            PsionPluginClass::HostNativeNetworkedReadOnly,
            networked_plugin_ids,
            "The bounded networked_read_only class now moves from substrate-proved to supported in the mixed learned publication.",
        ),
        supported_class_row(
            "guest_artifact_digest_bound",
            PsionPluginClass::GuestArtifactDigestBound,
            guest_plugin_ids.clone(),
            "The bounded guest-artifact class is supported only for the one admitted digest-bound plugin and only inside the mixed operator-internal publication.",
        ),
        PsionPluginMixedCapabilityRow {
            region_id: String::from("guest_artifact_digest_bound.admitted_use"),
            posture: PsionPluginMixedCapabilityPosture::Supported,
            claim_class: PsionPluginMixedCapabilityClaimClass::PluginUseRegion,
            plugin_class: Some(PsionPluginClass::GuestArtifactDigestBound),
            route_labels: vec![PsionPluginRouteLabel::DelegateToAdmittedPlugin],
            admitted_plugin_ids: guest_plugin_ids,
            evidence: Some(PsionPluginMixedCapabilityEvidence::GuestCapabilityMetric(
                guest_metric_evidence(
                    PsionPluginBenchmarkMetricKind::GuestPluginAdmittedUseAccuracyBps,
                    guest_benchmark_bundle,
                )?,
            )),
            detail: String::from(
                "The mixed publication supports one bounded guest-artifact use region: delegate only to the admitted digest-bound echo guest plugin while keeping internal-only and publication-blocked posture explicit.",
            ),
        },
        PsionPluginMixedCapabilityRow {
            region_id: String::from("guest_artifact_generic_loading_or_unadmitted_digest"),
            posture: PsionPluginMixedCapabilityPosture::Blocked,
            claim_class: PsionPluginMixedCapabilityClaimClass::PluginClassBoundary,
            plugin_class: Some(PsionPluginClass::GuestArtifactDigestBound),
            route_labels: vec![PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability],
            admitted_plugin_ids: vec![],
            evidence: Some(PsionPluginMixedCapabilityEvidence::GuestCapabilityMetric(
                guest_metric_evidence(
                    PsionPluginBenchmarkMetricKind::GuestPluginUnsupportedLoadRefusalAccuracyBps,
                    guest_benchmark_bundle,
                )?,
            )),
            detail: String::from(
                "The mixed publication keeps generic guest loading and unadmitted digest claims blocked rather than implying arbitrary guest-artifact admission.",
            ),
        },
        PsionPluginMixedCapabilityRow {
            region_id: String::from("host_native_secret_backed_or_stateful"),
            posture: PsionPluginMixedCapabilityPosture::Unsupported,
            claim_class: PsionPluginMixedCapabilityClaimClass::PluginClassBoundary,
            plugin_class: Some(PsionPluginClass::HostNativeSecretBackedOrStateful),
            route_labels: vec![],
            admitted_plugin_ids: vec![],
            evidence: None,
            detail: String::from(
                "Secret-backed and stateful host-native plugins remain outside the mixed publication and are not implied by the mixed learned lane.",
            ),
        },
        blocked_boundary_row(
            "plugin_publication_or_marketplace",
            PsionPluginMixedCapabilityClaimClass::PublicationBoundary,
            guest_metric_evidence(
                PsionPluginBenchmarkMetricKind::GuestPluginPublicationBoundaryAccuracyBps,
                guest_benchmark_bundle,
            )?,
            "The mixed publication still blocks plugin publication, marketplace, and external catalog claims for the guest lane.",
        ),
        blocked_boundary_row(
            "public_plugin_universality",
            PsionPluginMixedCapabilityClaimClass::PublicationBoundary,
            guest_metric_evidence(
                PsionPluginBenchmarkMetricKind::GuestPluginServedUniversalityBoundaryAccuracyBps,
                guest_benchmark_bundle,
            )?,
            "The mixed publication still blocks served/public plugin universality claims.",
        ),
        blocked_boundary_row(
            "arbitrary_software_capability",
            PsionPluginMixedCapabilityClaimClass::SoftwareCapabilityBoundary,
            guest_metric_evidence(
                PsionPluginBenchmarkMetricKind::GuestPluginArbitraryBinaryBoundaryAccuracyBps,
                guest_benchmark_bundle,
            )?,
            "The mixed publication does not imply arbitrary binary loading or arbitrary software capability.",
        ),
    ];
    let mut matrix = PsionPluginMixedCapabilityMatrix {
        schema_version: String::from(PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_SCHEMA_VERSION),
        matrix_id: String::from("psion_plugin_mixed_capability_matrix"),
        matrix_version: String::from("v2"),
        lane_id: run_bundle.lane_id.clone(),
        model_artifact_id: run_bundle.model_artifact.artifact_id.clone(),
        model_artifact_digest: run_bundle.model_artifact.artifact_digest.clone(),
        mixed_evaluation_receipt_id: run_bundle.evaluation_receipt.receipt_id.clone(),
        mixed_evaluation_receipt_digest: run_bundle.evaluation_receipt.receipt_digest.clone(),
        guest_benchmark_package_id: guest_benchmark_bundle.package.package_id.clone(),
        guest_benchmark_package_digest: guest_benchmark_bundle.package.package_digest.clone(),
        guest_benchmark_receipt_id: guest_benchmark_bundle.receipt.receipt_id.clone(),
        guest_benchmark_receipt_digest: guest_benchmark_bundle.receipt.receipt_digest.clone(),
        claim_boundary_doc_ref: String::from(PSION_PLUGIN_CLAIM_BOUNDARY_DOC_REF),
        served_evidence_doc_ref: String::from(PSION_SERVED_EVIDENCE_DOC_REF),
        served_output_claim_doc_ref: String::from(PSION_SERVED_OUTPUT_CLAIMS_DOC_REF),
        rows,
        summary: String::from(
            "The mixed capability matrix publishes supported host-native regions over the admitted mixed host-native set, one bounded guest admitted-use region, and benchmark-backed blocked rows for generic guest loading, publication, universality, and arbitrary-binary overclaims.",
        ),
        matrix_digest: String::new(),
    };
    matrix.rows.sort_by(|left, right| left.region_id.cmp(&right.region_id));
    matrix.matrix_digest = stable_capability_matrix_digest(&matrix);
    matrix.validate_against_inputs(run_bundle, guest_benchmark_bundle)?;
    Ok(matrix)
}

pub fn record_psion_plugin_mixed_served_posture(
    matrix: &PsionPluginMixedCapabilityMatrix,
    run_bundle: &PsionPluginMixedReferenceRunBundle,
    guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
) -> Result<PsionPluginMixedServedPosture, PsionPluginMixedCapabilityMatrixError> {
    let mut posture = PsionPluginMixedServedPosture {
        schema_version: String::from(PSION_PLUGIN_MIXED_SERVED_POSTURE_SCHEMA_VERSION),
        posture_id: String::from("psion_plugin_mixed_served_posture"),
        posture_version: String::from("v2"),
        lane_id: run_bundle.lane_id.clone(),
        capability_matrix_id: matrix.matrix_id.clone(),
        capability_matrix_version: matrix.matrix_version.clone(),
        capability_matrix_digest: matrix.matrix_digest.clone(),
        model_artifact_id: run_bundle.model_artifact.artifact_id.clone(),
        model_artifact_digest: run_bundle.model_artifact.artifact_digest.clone(),
        mixed_evaluation_receipt_id: run_bundle.evaluation_receipt.receipt_id.clone(),
        mixed_evaluation_receipt_digest: run_bundle.evaluation_receipt.receipt_digest.clone(),
        guest_benchmark_receipt_id: guest_benchmark_bundle.receipt.receipt_id.clone(),
        guest_benchmark_receipt_digest: guest_benchmark_bundle.receipt.receipt_digest.clone(),
        capability_doc_ref: String::from(PSION_PLUGIN_MIXED_CAPABILITY_DOC_REF),
        served_evidence_doc_ref: String::from(PSION_SERVED_EVIDENCE_DOC_REF),
        served_output_claim_doc_ref: String::from(PSION_SERVED_OUTPUT_CLAIMS_DOC_REF),
        visibility_posture: String::from("operator_internal_only"),
        supported_route_labels: required_route_labels(),
        supported_claim_surfaces: supported_claim_surfaces(),
        blocked_claim_surfaces: blocked_claim_surfaces(),
        not_yet_proved_plugin_classes: vec![],
        unsupported_plugin_classes: vec![PsionPluginClass::HostNativeSecretBackedOrStateful],
        typed_refusal_reasons: required_typed_refusal_reasons(),
        execution_backed_statement_policy: String::from(
            "executor_backed_result requires explicit runtime receipt refs; bounded guest capability rows do not imply hidden guest execution without those receipts",
        ),
        benchmark_backed_statement_policy: String::from(
            "benchmark_backed_capability_claim may cite supported host-native mixed rows and the bounded guest admitted-use row only; blocked guest loading, publication, universality, and arbitrary-binary rows remain refusal-boundary statements",
        ),
        summary: String::from(
            "The mixed served posture keeps the lane operator-internal, allows learned_judgment and benchmark_backed_capability_claim across the published mixed matrix, and keeps guest loading/publication/universality/arbitrary-binary overclaims blocked unless runtime receipts make a narrower executor-backed result explicit.",
        ),
        posture_digest: String::new(),
    };
    posture.posture_digest = stable_served_posture_digest(&posture);
    posture.validate_against_inputs(matrix, run_bundle, guest_benchmark_bundle)?;
    Ok(posture)
}

pub fn psion_plugin_mixed_capability_matrix_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_REF)
}

pub fn psion_plugin_mixed_served_posture_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_MIXED_SERVED_POSTURE_REF)
}

#[derive(Debug, Error)]
pub enum PsionPluginMixedCapabilityMatrixError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn supported_host_native_region(
    region_id: &str,
    detail: &str,
    route_labels: Vec<PsionPluginRouteLabel>,
    admitted_plugin_ids: Vec<String>,
    evidence: PsionPluginMixedBenchmarkEvidence,
) -> PsionPluginMixedCapabilityRow {
    PsionPluginMixedCapabilityRow {
        region_id: String::from(region_id),
        posture: PsionPluginMixedCapabilityPosture::Supported,
        claim_class: PsionPluginMixedCapabilityClaimClass::PluginUseRegion,
        plugin_class: None,
        route_labels,
        admitted_plugin_ids,
        evidence: Some(PsionPluginMixedCapabilityEvidence::MixedBenchmark(evidence)),
        detail: String::from(detail),
    }
}

fn supported_class_row(
    region_id: &str,
    plugin_class: PsionPluginClass,
    admitted_plugin_ids: Vec<String>,
    detail: &str,
) -> PsionPluginMixedCapabilityRow {
    PsionPluginMixedCapabilityRow {
        region_id: String::from(region_id),
        posture: PsionPluginMixedCapabilityPosture::Supported,
        claim_class: PsionPluginMixedCapabilityClaimClass::PluginClassBoundary,
        plugin_class: Some(plugin_class),
        route_labels: vec![],
        admitted_plugin_ids,
        evidence: None,
        detail: String::from(detail),
    }
}

fn blocked_boundary_row(
    region_id: &str,
    claim_class: PsionPluginMixedCapabilityClaimClass,
    evidence: PsionPluginGuestCapabilityMetricEvidence,
    detail: &str,
) -> PsionPluginMixedCapabilityRow {
    PsionPluginMixedCapabilityRow {
        region_id: String::from(region_id),
        posture: PsionPluginMixedCapabilityPosture::Blocked,
        claim_class,
        plugin_class: None,
        route_labels: vec![],
        admitted_plugin_ids: vec![],
        evidence: Some(PsionPluginMixedCapabilityEvidence::GuestCapabilityMetric(evidence)),
        detail: String::from(detail),
    }
}

fn mixed_benchmark_evidence(
    family: PsionPluginBenchmarkFamily,
    run_bundle: &PsionPluginMixedReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkEvidence, PsionPluginMixedCapabilityMatrixError> {
    let row = run_bundle
        .evaluation_receipt
        .benchmark_comparisons
        .iter()
        .find(|row| row.benchmark_family == family)
        .ok_or_else(|| PsionPluginMixedCapabilityMatrixError::MissingField {
            field: format!("mixed_evaluation_receipt.benchmark_comparisons.{family:?}"),
        })?;
    Ok(PsionPluginMixedBenchmarkEvidence {
        benchmark_family: family,
        evaluation_receipt_id: run_bundle.evaluation_receipt.receipt_id.clone(),
        evaluation_receipt_digest: run_bundle.evaluation_receipt.receipt_digest.clone(),
        comparison_label: run_bundle.evaluation_receipt.comparison_label.clone(),
        comparison_reference_run_bundle_ref: run_bundle
            .evaluation_receipt
            .comparison_reference_run_bundle_ref
            .clone(),
        comparison_reference_run_bundle_digest: run_bundle
            .evaluation_receipt
            .comparison_reference_run_bundle_digest
            .clone(),
        eligible_item_count: row.eligible_item_count,
        out_of_scope_item_count: row.out_of_scope_item_count,
        host_native_reference_score_bps: row.host_native_reference_score_bps,
        mixed_score_bps: row.mixed_score_bps,
        delta_vs_host_native_bps: row.delta_vs_host_native_bps,
    })
}

fn guest_metric_evidence(
    metric_kind: PsionPluginBenchmarkMetricKind,
    guest_benchmark_bundle: &PsionPluginGuestPluginBenchmarkBundle,
) -> Result<PsionPluginGuestCapabilityMetricEvidence, PsionPluginMixedCapabilityMatrixError> {
    let metric = guest_benchmark_bundle
        .receipt
        .observed_metrics
        .iter()
        .find(|metric| metric.kind == metric_kind)
        .ok_or_else(|| PsionPluginMixedCapabilityMatrixError::MissingField {
            field: format!("guest_benchmark_receipt.observed_metrics.{metric_kind:?}"),
        })?;
    Ok(PsionPluginGuestCapabilityMetricEvidence {
        package_id: guest_benchmark_bundle.package.package_id.clone(),
        package_digest: guest_benchmark_bundle.package.package_digest.clone(),
        receipt_id: guest_benchmark_bundle.receipt.receipt_id.clone(),
        receipt_digest: guest_benchmark_bundle.receipt.receipt_digest.clone(),
        metric_kind,
        value_bps: metric.value_bps,
    })
}

fn host_native_plugin_ids(run_bundle: &PsionPluginMixedReferenceRunBundle) -> Vec<String> {
    let mut ids = run_bundle
        .model_artifact
        .learned_plugin_ids
        .iter()
        .filter(|plugin_id| plugin_id.as_str() != "plugin.example.echo_guest")
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn local_plugin_ids(run_bundle: &PsionPluginMixedReferenceRunBundle) -> Vec<String> {
    let mut ids = run_bundle
        .model_artifact
        .learned_plugin_ids
        .iter()
        .filter(|plugin_id| plugin_id.as_str() != "plugin.example.echo_guest")
        .filter(|plugin_id| plugin_id.as_str() != "plugin.http.fetch_text")
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn networked_plugin_ids(run_bundle: &PsionPluginMixedReferenceRunBundle) -> Vec<String> {
    let mut ids = run_bundle
        .model_artifact
        .learned_plugin_ids
        .iter()
        .filter(|plugin_id| plugin_id.as_str() == "plugin.http.fetch_text")
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn guest_plugin_ids(run_bundle: &PsionPluginMixedReferenceRunBundle) -> Vec<String> {
    let mut ids = run_bundle
        .model_artifact
        .learned_plugin_ids
        .iter()
        .filter(|plugin_id| plugin_id.as_str() == "plugin.example.echo_guest")
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn required_route_labels() -> Vec<PsionPluginRouteLabel> {
    vec![
        PsionPluginRouteLabel::AnswerInLanguage,
        PsionPluginRouteLabel::DelegateToAdmittedPlugin,
        PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
        PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
    ]
}

fn supported_claim_surfaces() -> Vec<PsionPluginMixedClaimSurface> {
    vec![
        PsionPluginMixedClaimSurface::LearnedJudgment,
        PsionPluginMixedClaimSurface::BenchmarkBackedCapabilityClaim,
        PsionPluginMixedClaimSurface::ExecutorBackedResult,
    ]
}

fn blocked_claim_surfaces() -> Vec<PsionPluginMixedClaimSurface> {
    vec![
        PsionPluginMixedClaimSurface::SourceGroundedStatement,
        PsionPluginMixedClaimSurface::Verification,
        PsionPluginMixedClaimSurface::PluginPublication,
        PsionPluginMixedClaimSurface::PublicPluginUniversality,
        PsionPluginMixedClaimSurface::ArbitrarySoftwareCapability,
        PsionPluginMixedClaimSurface::HiddenExecutionWithoutRuntimeReceipt,
    ]
}

fn required_typed_refusal_reasons() -> Vec<String> {
    vec![
        String::from("plugin_capability_outside_admitted_set"),
        String::from("missing_required_structured_input"),
        String::from("publication_or_arbitrary_loading_claim_blocked"),
        String::from("secret_backed_or_stateful_class_not_enabled"),
        String::from("guest_artifact_load_refusal"),
    ]
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
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginMixedCapabilityMatrixError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        PsionPluginMixedCapabilityMatrixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn stable_capability_matrix_digest(matrix: &PsionPluginMixedCapabilityMatrix) -> String {
    let mut canonical = matrix.clone();
    canonical.matrix_digest.clear();
    stable_digest(b"psion_plugin_mixed_capability_matrix|", &canonical)
}

fn stable_served_posture_digest(posture: &PsionPluginMixedServedPosture) -> String {
    let mut canonical = posture.clone();
    canonical.posture_digest.clear();
    stable_digest(b"psion_plugin_mixed_served_posture|", &canonical)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    if value.trim().is_empty() {
        return Err(PsionPluginMixedCapabilityMatrixError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(
    value: u32,
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    if value > 10_000 {
        return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
            field: String::from(field),
            expected: String::from("at most 10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn ensure_bool_true(
    value: bool,
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    if !value {
        return Err(PsionPluginMixedCapabilityMatrixError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    if actual != expected {
        return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn check_debug_slice_match<T: std::fmt::Debug + PartialEq>(
    actual: &[T],
    expected: &[T],
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    if actual != expected {
        return Err(PsionPluginMixedCapabilityMatrixError::FieldMismatch {
            field: String::from(field),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginMixedCapabilityMatrixError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginMixedCapabilityMatrixError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn sorted_unique_strings(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::{record_psion_plugin_mixed_capability_matrix, record_psion_plugin_mixed_served_posture};
    use crate::{
        build_psion_plugin_guest_plugin_benchmark_bundle, run_psion_plugin_mixed_reference_lane,
    };

    #[test]
    fn mixed_capability_matrix_and_posture_validate() -> Result<(), Box<dyn std::error::Error>> {
        let run_bundle = run_psion_plugin_mixed_reference_lane()?;
        let guest_benchmark_bundle = build_psion_plugin_guest_plugin_benchmark_bundle()?;
        let matrix = record_psion_plugin_mixed_capability_matrix(&run_bundle, &guest_benchmark_bundle)?;
        let posture =
            record_psion_plugin_mixed_served_posture(&matrix, &run_bundle, &guest_benchmark_bundle)?;
        matrix.validate_against_inputs(&run_bundle, &guest_benchmark_bundle)?;
        posture.validate_against_inputs(&matrix, &run_bundle, &guest_benchmark_bundle)?;
        assert!(matrix
            .rows
            .iter()
            .any(|row| row.region_id == "guest_artifact_digest_bound.admitted_use"));
        assert!(matrix
            .rows
            .iter()
            .any(|row| row.region_id == "host_native_networked_read_only"));
        Ok(())
    }
}
