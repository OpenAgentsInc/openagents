use std::collections::{BTreeMap, BTreeSet};

use psionic_data::PsionArtifactLineageManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PsionBenchmarkPackageContract, PsionBenchmarkPackageFamily, PsionBenchmarkTaskContract};

/// Stable schema version for the Psion refusal-calibration receipt.
pub const PSION_REFUSAL_CALIBRATION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.refusal_calibration_receipt.v1";
/// Stable schema version for the served capability-matrix artifact.
pub const PSION_CAPABILITY_MATRIX_SCHEMA_VERSION: &str = "psion.capability_matrix.v1";

/// Minimal posture view needed from the Psion capability matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCapabilityRegionPostureView {
    Supported,
    RouteRequired,
    RefusalRequired,
    Unsupported,
}

/// Minimal region view needed from the Psion capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityRegionView {
    /// Stable capability-region id.
    pub region_id: String,
    /// Published posture for the region.
    pub posture: PsionCapabilityRegionPostureView,
    /// Typed refusal reasons published for the region.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<String>,
}

/// Minimal capability-matrix view used by refusal calibration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCapabilityMatrixView {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable matrix id.
    pub matrix_id: String,
    /// Stable matrix version.
    pub matrix_version: String,
    /// Published regions.
    pub regions: Vec<PsionCapabilityRegionView>,
}

impl PsionCapabilityMatrixView {
    /// Validates the capability-matrix subset used by refusal calibration.
    pub fn validate(&self) -> Result<(), PsionRefusalCalibrationError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_capability_matrix_view.schema_version",
        )?;
        if self.schema_version != PSION_CAPABILITY_MATRIX_SCHEMA_VERSION {
            return Err(PsionRefusalCalibrationError::SchemaVersionMismatch {
                expected: String::from(PSION_CAPABILITY_MATRIX_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.matrix_id.as_str(), "psion_capability_matrix_view.matrix_id")?;
        ensure_nonempty(
            self.matrix_version.as_str(),
            "psion_capability_matrix_view.matrix_version",
        )?;
        if self.regions.is_empty() {
            return Err(PsionRefusalCalibrationError::MissingField {
                field: String::from("psion_capability_matrix_view.regions"),
            });
        }
        let mut seen_regions = BTreeSet::new();
        for region in &self.regions {
            ensure_nonempty(
                region.region_id.as_str(),
                "psion_capability_matrix_view.regions[].region_id",
            )?;
            if !seen_regions.insert(region.region_id.as_str()) {
                return Err(PsionRefusalCalibrationError::DuplicateCapabilityRegion {
                    region_id: region.region_id.clone(),
                });
            }
            match region.posture {
                PsionCapabilityRegionPostureView::Supported
                | PsionCapabilityRegionPostureView::RouteRequired => {
                    if !region.refusal_reasons.is_empty() {
                        return Err(PsionRefusalCalibrationError::FieldMismatch {
                            field: format!(
                                "psion_capability_matrix_view.regions[{}].refusal_reasons",
                                region.region_id
                            ),
                            expected: String::from("empty"),
                            actual: String::from("nonempty"),
                        });
                    }
                }
                PsionCapabilityRegionPostureView::RefusalRequired
                | PsionCapabilityRegionPostureView::Unsupported => {
                    if region.refusal_reasons.is_empty() {
                        return Err(PsionRefusalCalibrationError::MissingField {
                            field: format!(
                                "psion_capability_matrix_view.regions[{}].refusal_reasons",
                                region.region_id
                            ),
                        });
                    }
                    let mut reasons = BTreeSet::new();
                    for reason in &region.refusal_reasons {
                        ensure_nonempty(
                            reason.as_str(),
                            "psion_capability_matrix_view.regions[].refusal_reasons[]",
                        )?;
                        if !reasons.insert(reason.as_str()) {
                            return Err(PsionRefusalCalibrationError::DuplicateRefusalReason {
                                reason_code: reason.clone(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn region(&self, region_id: &str) -> Option<&PsionCapabilityRegionView> {
        self.regions.iter().find(|region| region.region_id == region_id)
    }
}

/// One measured refusal-calibration row tied to one refusal benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRefusalCalibrationRow {
    /// Stable benchmark item id.
    pub item_id: String,
    /// Capability region the row calibrates against.
    pub capability_region_id: String,
    /// Expected refusal reason code for the item.
    pub expected_reason_code: String,
    /// Observed refusal accuracy for the unsupported request.
    pub observed_refusal_accuracy_bps: u32,
    /// Observed exact reason-code match rate.
    pub reason_code_match_bps: u32,
    /// Stable evidence ref for the unsupported region.
    pub unsupported_region_evidence_ref: String,
    /// Short explanation of the row.
    pub detail: String,
}

/// One refusal-calibration receipt for the canonical unsupported-request refusal package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRefusalCalibrationReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable refusal benchmark package id.
    pub package_id: String,
    /// Stable refusal benchmark package digest.
    pub package_digest: String,
    /// Capability-matrix id used for calibration.
    pub capability_matrix_id: String,
    /// Capability-matrix version used for calibration.
    pub capability_matrix_version: String,
    /// Per-probe refusal rows.
    pub rows: Vec<PsionRefusalCalibrationRow>,
    /// Aggregate refusal accuracy across unsupported requests.
    pub aggregate_unsupported_request_refusal_bps: u32,
    /// Aggregate reason-code match rate across unsupported requests.
    pub aggregate_reason_code_match_bps: u32,
    /// Supported-control over-refusal rate carried alongside the unsupported rows.
    pub supported_control_overrefusal_bps: u32,
    /// Regression against the last accepted refusal baseline.
    pub refusal_regression_bps: u32,
    /// Short explanation of the receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionRefusalCalibrationReceipt {
    /// Validates the receipt shape and digest without external context.
    pub fn validate(&self) -> Result<(), PsionRefusalCalibrationError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_refusal_calibration_receipt.schema_version",
        )?;
        if self.schema_version != PSION_REFUSAL_CALIBRATION_RECEIPT_SCHEMA_VERSION {
            return Err(PsionRefusalCalibrationError::SchemaVersionMismatch {
                expected: String::from(PSION_REFUSAL_CALIBRATION_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "psion_refusal_calibration_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.package_id.as_str(),
            "psion_refusal_calibration_receipt.package_id",
        )?;
        ensure_nonempty(
            self.package_digest.as_str(),
            "psion_refusal_calibration_receipt.package_digest",
        )?;
        ensure_nonempty(
            self.capability_matrix_id.as_str(),
            "psion_refusal_calibration_receipt.capability_matrix_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_version.as_str(),
            "psion_refusal_calibration_receipt.capability_matrix_version",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_refusal_calibration_receipt.summary",
        )?;
        if self.rows.is_empty() {
            return Err(PsionRefusalCalibrationError::MissingField {
                field: String::from("psion_refusal_calibration_receipt.rows"),
            });
        }
        let mut seen_items = BTreeSet::new();
        let mut refusal_sum = 0_u32;
        let mut reason_code_sum = 0_u32;
        for row in &self.rows {
            ensure_nonempty(
                row.item_id.as_str(),
                "psion_refusal_calibration_receipt.rows[].item_id",
            )?;
            ensure_nonempty(
                row.capability_region_id.as_str(),
                "psion_refusal_calibration_receipt.rows[].capability_region_id",
            )?;
            ensure_nonempty(
                row.expected_reason_code.as_str(),
                "psion_refusal_calibration_receipt.rows[].expected_reason_code",
            )?;
            ensure_nonempty(
                row.unsupported_region_evidence_ref.as_str(),
                "psion_refusal_calibration_receipt.rows[].unsupported_region_evidence_ref",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "psion_refusal_calibration_receipt.rows[].detail",
            )?;
            validate_bps(
                row.observed_refusal_accuracy_bps,
                "psion_refusal_calibration_receipt.rows[].observed_refusal_accuracy_bps",
            )?;
            validate_bps(
                row.reason_code_match_bps,
                "psion_refusal_calibration_receipt.rows[].reason_code_match_bps",
            )?;
            if !seen_items.insert(row.item_id.as_str()) {
                return Err(PsionRefusalCalibrationError::DuplicateRefusalItem {
                    item_id: row.item_id.clone(),
                });
            }
            refusal_sum = refusal_sum.saturating_add(row.observed_refusal_accuracy_bps);
            reason_code_sum = reason_code_sum.saturating_add(row.reason_code_match_bps);
        }
        validate_bps(
            self.supported_control_overrefusal_bps,
            "psion_refusal_calibration_receipt.supported_control_overrefusal_bps",
        )?;
        validate_bps(
            self.refusal_regression_bps,
            "psion_refusal_calibration_receipt.refusal_regression_bps",
        )?;

        let expected_refusal = refusal_sum / (self.rows.len() as u32).max(1);
        if self.aggregate_unsupported_request_refusal_bps != expected_refusal {
            return Err(PsionRefusalCalibrationError::FieldMismatch {
                field: String::from(
                    "psion_refusal_calibration_receipt.aggregate_unsupported_request_refusal_bps",
                ),
                expected: expected_refusal.to_string(),
                actual: self.aggregate_unsupported_request_refusal_bps.to_string(),
            });
        }
        let expected_reason_match = reason_code_sum / (self.rows.len() as u32).max(1);
        if self.aggregate_reason_code_match_bps != expected_reason_match {
            return Err(PsionRefusalCalibrationError::FieldMismatch {
                field: String::from(
                    "psion_refusal_calibration_receipt.aggregate_reason_code_match_bps",
                ),
                expected: expected_reason_match.to_string(),
                actual: self.aggregate_reason_code_match_bps.to_string(),
            });
        }
        if self.receipt_digest != stable_refusal_calibration_receipt_digest(self) {
            return Err(PsionRefusalCalibrationError::DigestMismatch {
                kind: String::from("psion_refusal_calibration_receipt"),
            });
        }
        Ok(())
    }

    /// Validates the receipt against the refusal package, capability matrix, and artifact lineage.
    pub fn validate_against_package_and_matrix(
        &self,
        package: &PsionBenchmarkPackageContract,
        capability_matrix: &PsionCapabilityMatrixView,
        artifact_lineage: &PsionArtifactLineageManifest,
    ) -> Result<(), PsionRefusalCalibrationError> {
        self.validate()?;
        capability_matrix.validate()?;
        check_string_match(
            self.package_id.as_str(),
            package.package_id.as_str(),
            "psion_refusal_calibration_receipt.package_id",
        )?;
        check_string_match(
            self.package_digest.as_str(),
            package.package_digest.as_str(),
            "psion_refusal_calibration_receipt.package_digest",
        )?;
        if package.package_family != PsionBenchmarkPackageFamily::RefusalEvaluation {
            return Err(PsionRefusalCalibrationError::FieldMismatch {
                field: String::from("psion_refusal_calibration_receipt.package_family"),
                expected: String::from("RefusalEvaluation"),
                actual: format!("{:?}", package.package_family),
            });
        }
        check_string_match(
            self.capability_matrix_id.as_str(),
            capability_matrix.matrix_id.as_str(),
            "psion_refusal_calibration_receipt.capability_matrix_id",
        )?;
        check_string_match(
            self.capability_matrix_version.as_str(),
            capability_matrix.matrix_version.as_str(),
            "psion_refusal_calibration_receipt.capability_matrix_version",
        )?;
        if self.rows.len() != package.items.len() {
            return Err(PsionRefusalCalibrationError::FieldMismatch {
                field: String::from("psion_refusal_calibration_receipt.rows"),
                expected: package.items.len().to_string(),
                actual: self.rows.len().to_string(),
            });
        }

        let lineage_row = artifact_lineage
            .benchmark_artifacts
            .iter()
            .find(|artifact| artifact.benchmark_id == package.package_id)
            .ok_or_else(
                || PsionRefusalCalibrationError::UnknownBenchmarkArtifactLineage {
                    package_id: package.package_id.clone(),
                },
            )?;
        check_string_match(
            lineage_row.benchmark_digest.as_str(),
            package.package_digest.as_str(),
            "psion_refusal_calibration_receipt.lineage_row.benchmark_digest",
        )?;

        let package_items = package
            .items
            .iter()
            .map(|item| {
                let (
                    expected_reason_code,
                    capability_region_id,
                    unsupported_region_evidence_ref,
                    claim_boundary_required,
                ) = match &item.task {
                    PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code,
                        capability_region_id,
                        unsupported_region_evidence_ref,
                        claim_boundary_required,
                        ..
                    } => (
                        expected_reason_code.as_str(),
                        capability_region_id.as_str(),
                        unsupported_region_evidence_ref.as_str(),
                        *claim_boundary_required,
                    ),
                    _ => unreachable!("refusal package should only contain refusal tasks"),
                };
                (
                    item.item_id.as_str(),
                    (
                        expected_reason_code,
                        capability_region_id,
                        unsupported_region_evidence_ref,
                        claim_boundary_required,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();

        for row in &self.rows {
            let (
                expected_reason_code,
                capability_region_id,
                unsupported_region_evidence_ref,
                claim_boundary_required,
            ) = package_items.get(row.item_id.as_str()).copied().ok_or_else(|| {
                PsionRefusalCalibrationError::UnknownRefusalPackageItem {
                    package_id: package.package_id.clone(),
                    item_id: row.item_id.clone(),
                }
            })?;
            if !claim_boundary_required {
                return Err(PsionRefusalCalibrationError::FieldMismatch {
                    field: format!(
                        "psion_refusal_calibration_receipt.rows[{}].claim_boundary_required",
                        row.item_id
                    ),
                    expected: String::from("true"),
                    actual: String::from("false"),
                });
            }
            check_string_match(
                row.expected_reason_code.as_str(),
                expected_reason_code,
                format!(
                    "psion_refusal_calibration_receipt.rows[{}].expected_reason_code",
                    row.item_id
                )
                .as_str(),
            )?;
            check_string_match(
                row.capability_region_id.as_str(),
                capability_region_id,
                format!(
                    "psion_refusal_calibration_receipt.rows[{}].capability_region_id",
                    row.item_id
                )
                .as_str(),
            )?;
            check_string_match(
                row.unsupported_region_evidence_ref.as_str(),
                unsupported_region_evidence_ref,
                format!(
                    "psion_refusal_calibration_receipt.rows[{}].unsupported_region_evidence_ref",
                    row.item_id
                )
                .as_str(),
            )?;
            let region = capability_matrix
                .region(row.capability_region_id.as_str())
                .ok_or_else(|| PsionRefusalCalibrationError::UnknownCapabilityRegion {
                    region_id: row.capability_region_id.clone(),
                })?;
            match region.posture {
                PsionCapabilityRegionPostureView::RefusalRequired
                | PsionCapabilityRegionPostureView::Unsupported => {}
                posture => {
                    return Err(PsionRefusalCalibrationError::InvalidCapabilityRegionPosture {
                        region_id: row.capability_region_id.clone(),
                        posture: format!("{posture:?}"),
                    });
                }
            }
            if !region
                .refusal_reasons
                .iter()
                .any(|reason| reason == row.expected_reason_code.as_str())
            {
                return Err(PsionRefusalCalibrationError::FieldMismatch {
                    field: format!(
                        "psion_refusal_calibration_receipt.rows[{}].expected_reason_code",
                        row.item_id
                    ),
                    expected: region.refusal_reasons.join(","),
                    actual: row.expected_reason_code.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Records one refusal-calibration receipt for the canonical unsupported-request refusal package.
pub fn record_psion_refusal_calibration_receipt(
    receipt_id: impl Into<String>,
    package: &PsionBenchmarkPackageContract,
    capability_matrix: &PsionCapabilityMatrixView,
    mut rows: Vec<PsionRefusalCalibrationRow>,
    supported_control_overrefusal_bps: u32,
    refusal_regression_bps: u32,
    summary: impl Into<String>,
    artifact_lineage: &PsionArtifactLineageManifest,
) -> Result<PsionRefusalCalibrationReceipt, PsionRefusalCalibrationError> {
    rows.sort_by(|left, right| left.item_id.cmp(&right.item_id));
    let aggregate_unsupported_request_refusal_bps =
        rows.iter().map(|row| row.observed_refusal_accuracy_bps).sum::<u32>()
            / (rows.len() as u32).max(1);
    let aggregate_reason_code_match_bps =
        rows.iter().map(|row| row.reason_code_match_bps).sum::<u32>()
            / (rows.len() as u32).max(1);
    let mut receipt = PsionRefusalCalibrationReceipt {
        schema_version: String::from(PSION_REFUSAL_CALIBRATION_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        package_id: package.package_id.clone(),
        package_digest: package.package_digest.clone(),
        capability_matrix_id: capability_matrix.matrix_id.clone(),
        capability_matrix_version: capability_matrix.matrix_version.clone(),
        rows,
        aggregate_unsupported_request_refusal_bps,
        aggregate_reason_code_match_bps,
        supported_control_overrefusal_bps,
        refusal_regression_bps,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_refusal_calibration_receipt_digest(&receipt);
    receipt.validate_against_package_and_matrix(package, capability_matrix, artifact_lineage)?;
    Ok(receipt)
}

/// Error returned by refusal-calibration validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionRefusalCalibrationError {
    /// One required field was empty or missing.
    #[error("Psion refusal calibration field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One field diverged from the required value.
    #[error("Psion refusal calibration field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field path.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// One basis-point field was outside the valid range.
    #[error("Psion refusal calibration field `{field}` must stay within 0..=10000, found `{actual}`")]
    InvalidBps {
        /// Field path.
        field: String,
        /// Actual value.
        actual: u32,
    },
    /// The schema version drifted from the current contract.
    #[error("Psion refusal calibration expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The receipt repeated one benchmark item.
    #[error("Psion refusal calibration repeated refusal item `{item_id}`")]
    DuplicateRefusalItem {
        /// Repeated item id.
        item_id: String,
    },
    /// The capability-matrix view repeated one region.
    #[error("Psion refusal calibration repeated capability region `{region_id}`")]
    DuplicateCapabilityRegion {
        /// Repeated region id.
        region_id: String,
    },
    /// The capability-matrix view repeated one refusal reason.
    #[error("Psion refusal calibration repeated refusal reason `{reason_code}`")]
    DuplicateRefusalReason {
        /// Repeated reason code.
        reason_code: String,
    },
    /// One row referenced an unknown package item.
    #[error("Psion refusal calibration package `{package_id}` is missing item `{item_id}`")]
    UnknownRefusalPackageItem {
        /// Package id.
        package_id: String,
        /// Item id.
        item_id: String,
    },
    /// The artifact-lineage manifest omitted the refusal benchmark artifact.
    #[error("Psion refusal calibration missing artifact lineage for package `{package_id}`")]
    UnknownBenchmarkArtifactLineage {
        /// Package id.
        package_id: String,
    },
    /// One row referenced an unknown capability region.
    #[error("Psion refusal calibration referenced unknown capability region `{region_id}`")]
    UnknownCapabilityRegion {
        /// Capability region id.
        region_id: String,
    },
    /// One referenced capability region was not a refusal or unsupported posture.
    #[error("Psion refusal calibration region `{region_id}` has unsupported posture `{posture}` for refusal benchmarking")]
    InvalidCapabilityRegionPosture {
        /// Region id.
        region_id: String,
        /// Actual posture.
        posture: String,
    },
    /// The receipt digest drifted from the canonical payload.
    #[error("Psion refusal calibration digest drifted for `{kind}`")]
    DigestMismatch {
        /// Artifact kind.
        kind: String,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionRefusalCalibrationError> {
    if value.trim().is_empty() {
        return Err(PsionRefusalCalibrationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionRefusalCalibrationError> {
    if value > 10_000 {
        return Err(PsionRefusalCalibrationError::InvalidBps {
            field: String::from(field),
            actual: value,
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionRefusalCalibrationError> {
    ensure_nonempty(actual, field)?;
    ensure_nonempty(expected, field)?;
    if actual != expected {
        return Err(PsionRefusalCalibrationError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn stable_refusal_calibration_receipt_digest(
    receipt: &PsionRefusalCalibrationReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_refusal_calibration_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.capability_matrix_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.capability_matrix_version.as_bytes());
    for row in &receipt.rows {
        hasher.update(b"|row|");
        hasher.update(row.item_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.capability_region_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.expected_reason_code.as_bytes());
        hasher.update(b"|");
        hasher.update(row.observed_refusal_accuracy_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.reason_code_match_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.unsupported_region_evidence_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(row.detail.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_unsupported_request_refusal_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_reason_code_match_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .supported_control_overrefusal_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.refusal_regression_bps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        record_psion_refusal_calibration_receipt, PsionCapabilityMatrixView,
        PsionRefusalCalibrationError, PsionRefusalCalibrationRow,
    };
    use crate::PsionBenchmarkCatalog;
    use psionic_data::PsionArtifactLineageManifest;

    fn benchmark_catalog() -> PsionBenchmarkCatalog {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"
        ))
        .expect("benchmark catalog fixture should parse")
    }

    fn capability_matrix() -> PsionCapabilityMatrixView {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/capability/psion_capability_matrix_v1.json"
        ))
        .expect("capability matrix fixture should parse")
    }

    fn artifact_lineage() -> PsionArtifactLineageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"
        ))
        .expect("artifact lineage fixture should parse")
    }

    fn refusal_package() -> crate::PsionBenchmarkPackageContract {
        benchmark_catalog()
            .packages
            .into_iter()
            .find(|package| package.package_id == "psion_unsupported_request_refusal_benchmark_v1")
            .expect("refusal package should exist")
    }

    #[test]
    fn refusal_calibration_fixture_validates() -> Result<(), Box<dyn std::error::Error>> {
        let receipt: crate::PsionRefusalCalibrationReceipt = serde_json::from_str(include_str!(
            "../../../fixtures/psion/refusal/psion_refusal_calibration_receipt_v1.json"
        ))?;
        receipt.validate_against_package_and_matrix(
            &refusal_package(),
            &capability_matrix(),
            &artifact_lineage(),
        )?;
        Ok(())
    }

    #[test]
    fn refusal_calibration_rejects_route_required_region() -> Result<(), Box<dyn std::error::Error>>
    {
        let package = refusal_package();
        let mut matrix = capability_matrix();
        let exactness_region = matrix
            .regions
            .iter_mut()
            .find(|region| {
                region.region_id == "unsupported_exact_execution_without_executor_surface"
            })
            .expect("exactness refusal region should exist");
        exactness_region.posture = super::PsionCapabilityRegionPostureView::RouteRequired;
        exactness_region.refusal_reasons.clear();

        let error = record_psion_refusal_calibration_receipt(
            "psion-refusal-calibration-invalid-region-v1",
            &package,
            &matrix,
            vec![
                PsionRefusalCalibrationRow {
                    item_id: String::from("refusal-case-exactness"),
                    capability_region_id: String::from(
                        "unsupported_exact_execution_without_executor_surface",
                    ),
                    expected_reason_code: String::from("unsupported_exactness_request"),
                    observed_refusal_accuracy_bps: 9950,
                    reason_code_match_bps: 10000,
                    unsupported_region_evidence_ref: String::from(
                        "evidence://psion/refusal/exactness-without-executor",
                    ),
                    detail: String::from("invalid exactness row"),
                },
                PsionRefusalCalibrationRow {
                    item_id: String::from("refusal-case-missing-constraints"),
                    capability_region_id: String::from(
                        "underspecified_design_without_required_constraints",
                    ),
                    expected_reason_code: String::from("missing_required_constraints"),
                    observed_refusal_accuracy_bps: 9890,
                    reason_code_match_bps: 9940,
                    unsupported_region_evidence_ref: String::from(
                        "evidence://psion/refusal/missing-required-constraints",
                    ),
                    detail: String::from("missing constraints row"),
                },
                PsionRefusalCalibrationRow {
                    item_id: String::from("refusal-case-over-context"),
                    capability_region_id: String::from("over_context_envelope_requests"),
                    expected_reason_code: String::from("unsupported_context_length"),
                    observed_refusal_accuracy_bps: 9940,
                    reason_code_match_bps: 9980,
                    unsupported_region_evidence_ref: String::from(
                        "evidence://psion/refusal/over-context-envelope",
                    ),
                    detail: String::from("over context row"),
                },
                PsionRefusalCalibrationRow {
                    item_id: String::from("refusal-case-freshness"),
                    capability_region_id: String::from(
                        "freshness_or_run_artifact_dependent_requests",
                    ),
                    expected_reason_code: String::from(
                        "currentness_or_run_artifact_dependency",
                    ),
                    observed_refusal_accuracy_bps: 9910,
                    reason_code_match_bps: 9950,
                    unsupported_region_evidence_ref: String::from(
                        "evidence://psion/refusal/currentness-or-hidden-artifact",
                    ),
                    detail: String::from("freshness row"),
                },
                PsionRefusalCalibrationRow {
                    item_id: String::from("refusal-case-open-ended"),
                    capability_region_id: String::from("open_ended_general_assistant_chat"),
                    expected_reason_code: String::from(
                        "open_ended_general_assistant_unsupported",
                    ),
                    observed_refusal_accuracy_bps: 9910,
                    reason_code_match_bps: 9930,
                    unsupported_region_evidence_ref: String::from(
                        "evidence://psion/refusal/open-ended-assistant",
                    ),
                    detail: String::from("open ended row"),
                },
            ],
            900,
            60,
            "invalid refusal calibration receipt",
            &artifact_lineage(),
        )
        .expect_err("route-required exactness region should be rejected");
        assert!(matches!(
            error,
            PsionRefusalCalibrationError::FieldMismatch { .. }
                | PsionRefusalCalibrationError::InvalidCapabilityRegionPosture { .. }
        ));
        Ok(())
    }
}
