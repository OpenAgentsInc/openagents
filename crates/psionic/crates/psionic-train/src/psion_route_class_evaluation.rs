use std::collections::{BTreeMap, BTreeSet};

use psionic_data::PsionArtifactLineageManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PsionBenchmarkPackageContract, PsionBenchmarkPackageFamily, PsionRouteClass};

/// Stable schema version for the Psion route-class evaluation receipt.
pub const PSION_ROUTE_CLASS_EVALUATION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.route_class_evaluation_receipt.v1";

/// One measured route-class row tied to one benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRouteClassEvaluationRow {
    /// Stable benchmark item id.
    pub item_id: String,
    /// Route class the item evaluates.
    pub route_class: PsionRouteClass,
    /// Observed route-selection accuracy for that class.
    pub observed_route_accuracy_bps: u32,
    /// Wrongly delegated rate when the item should have stayed in the learned lane.
    pub false_positive_delegation_bps: u32,
    /// Missed-delegation rate when the item should have gone to the exact executor.
    pub false_negative_delegation_bps: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// One route-class evaluation receipt for the canonical Psion route benchmark package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRouteClassEvaluationReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable benchmark package id.
    pub package_id: String,
    /// Stable benchmark package digest.
    pub package_digest: String,
    /// Per-class route rows.
    pub rows: Vec<PsionRouteClassEvaluationRow>,
    /// Aggregate route-selection accuracy across the route classes.
    pub aggregate_route_selection_accuracy_bps: u32,
    /// Aggregate false-positive delegation rate across non-delegation classes.
    pub aggregate_false_positive_delegation_bps: u32,
    /// Aggregate false-negative delegation rate on the delegation class.
    pub aggregate_false_negative_delegation_bps: u32,
    /// Short explanation of the receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionRouteClassEvaluationReceipt {
    /// Validates the receipt against the canonical route package and artifact lineage.
    pub fn validate_against_package(
        &self,
        package: &PsionBenchmarkPackageContract,
        artifact_lineage: &PsionArtifactLineageManifest,
    ) -> Result<(), PsionRouteClassEvaluationError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_route_class_evaluation_receipt.schema_version",
        )?;
        if self.schema_version != PSION_ROUTE_CLASS_EVALUATION_RECEIPT_SCHEMA_VERSION {
            return Err(PsionRouteClassEvaluationError::SchemaVersionMismatch {
                expected: String::from(PSION_ROUTE_CLASS_EVALUATION_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "psion_route_class_evaluation_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_route_class_evaluation_receipt.summary",
        )?;
        check_string_match(
            self.package_id.as_str(),
            package.package_id.as_str(),
            "psion_route_class_evaluation_receipt.package_id",
        )?;
        check_string_match(
            self.package_digest.as_str(),
            package.package_digest.as_str(),
            "psion_route_class_evaluation_receipt.package_digest",
        )?;
        if package.package_family != PsionBenchmarkPackageFamily::RouteEvaluation {
            return Err(PsionRouteClassEvaluationError::FieldMismatch {
                field: String::from("psion_route_class_evaluation_receipt.package_family"),
                expected: String::from("RouteEvaluation"),
                actual: format!("{:?}", package.package_family),
            });
        }
        if self.rows.len() != package.items.len() {
            return Err(PsionRouteClassEvaluationError::FieldMismatch {
                field: String::from("psion_route_class_evaluation_receipt.rows"),
                expected: package.items.len().to_string(),
                actual: self.rows.len().to_string(),
            });
        }

        let lineage_row = artifact_lineage
            .benchmark_artifacts
            .iter()
            .find(|artifact| artifact.benchmark_id == package.package_id)
            .ok_or_else(
                || PsionRouteClassEvaluationError::UnknownBenchmarkArtifactLineage {
                    package_id: package.package_id.clone(),
                },
            )?;
        check_string_match(
            lineage_row.benchmark_digest.as_str(),
            package.package_digest.as_str(),
            "psion_route_class_evaluation_receipt.lineage_row.benchmark_digest",
        )?;

        let package_items = package
            .items
            .iter()
            .map(|item| {
                let route_class = match &item.task {
                    crate::PsionBenchmarkTaskContract::RouteEvaluation { route_class, .. } => {
                        *route_class
                    }
                    _ => unreachable!("route package should only contain route tasks"),
                };
                (item.item_id.as_str(), route_class)
            })
            .collect::<BTreeMap<_, _>>();

        let mut seen_items = BTreeSet::new();
        let mut seen_classes = BTreeSet::new();
        let mut accuracy_sum = 0_u32;
        let mut false_positive_sum = 0_u32;
        let mut non_delegate_count = 0_u32;
        let mut delegate_false_negative = None;
        for row in &self.rows {
            ensure_nonempty(
                row.item_id.as_str(),
                "psion_route_class_evaluation_receipt.rows[].item_id",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "psion_route_class_evaluation_receipt.rows[].detail",
            )?;
            validate_bps(
                row.observed_route_accuracy_bps,
                "psion_route_class_evaluation_receipt.rows[].observed_route_accuracy_bps",
            )?;
            validate_bps(
                row.false_positive_delegation_bps,
                "psion_route_class_evaluation_receipt.rows[].false_positive_delegation_bps",
            )?;
            validate_bps(
                row.false_negative_delegation_bps,
                "psion_route_class_evaluation_receipt.rows[].false_negative_delegation_bps",
            )?;
            if !seen_items.insert(row.item_id.as_str()) {
                return Err(PsionRouteClassEvaluationError::DuplicateRouteItem {
                    item_id: row.item_id.clone(),
                });
            }
            let expected_class = package_items.get(row.item_id.as_str()).ok_or_else(|| {
                PsionRouteClassEvaluationError::UnknownRoutePackageItem {
                    package_id: package.package_id.clone(),
                    item_id: row.item_id.clone(),
                }
            })?;
            if row.route_class != *expected_class {
                return Err(PsionRouteClassEvaluationError::FieldMismatch {
                    field: format!(
                        "psion_route_class_evaluation_receipt.rows[{}].route_class",
                        row.item_id
                    ),
                    expected: format!("{expected_class:?}"),
                    actual: format!("{:?}", row.route_class),
                });
            }
            seen_classes.insert(row.route_class);
            accuracy_sum = accuracy_sum.saturating_add(row.observed_route_accuracy_bps);

            match row.route_class {
                PsionRouteClass::DelegateToExactExecutor => {
                    if row.false_positive_delegation_bps != 0 {
                        return Err(PsionRouteClassEvaluationError::FieldMismatch {
                            field: format!(
                                "psion_route_class_evaluation_receipt.rows[{}].false_positive_delegation_bps",
                                row.item_id
                            ),
                            expected: String::from("0"),
                            actual: row.false_positive_delegation_bps.to_string(),
                        });
                    }
                    delegate_false_negative = Some(row.false_negative_delegation_bps);
                }
                _ => {
                    if row.false_negative_delegation_bps != 0 {
                        return Err(PsionRouteClassEvaluationError::FieldMismatch {
                            field: format!(
                                "psion_route_class_evaluation_receipt.rows[{}].false_negative_delegation_bps",
                                row.item_id
                            ),
                            expected: String::from("0"),
                            actual: row.false_negative_delegation_bps.to_string(),
                        });
                    }
                    false_positive_sum =
                        false_positive_sum.saturating_add(row.false_positive_delegation_bps);
                    non_delegate_count = non_delegate_count.saturating_add(1);
                }
            }
        }
        for route_class in PsionRouteClass::required_classes() {
            if !seen_classes.contains(&route_class) {
                return Err(PsionRouteClassEvaluationError::MissingRouteClass {
                    route_class: format!("{route_class:?}"),
                });
            }
        }
        let expected_accuracy = accuracy_sum / (self.rows.len() as u32).max(1);
        if self.aggregate_route_selection_accuracy_bps != expected_accuracy {
            return Err(PsionRouteClassEvaluationError::FieldMismatch {
                field: String::from(
                    "psion_route_class_evaluation_receipt.aggregate_route_selection_accuracy_bps",
                ),
                expected: expected_accuracy.to_string(),
                actual: self.aggregate_route_selection_accuracy_bps.to_string(),
            });
        }
        let expected_false_positive = false_positive_sum / non_delegate_count.max(1);
        if self.aggregate_false_positive_delegation_bps != expected_false_positive {
            return Err(PsionRouteClassEvaluationError::FieldMismatch {
                field: String::from(
                    "psion_route_class_evaluation_receipt.aggregate_false_positive_delegation_bps",
                ),
                expected: expected_false_positive.to_string(),
                actual: self.aggregate_false_positive_delegation_bps.to_string(),
            });
        }
        let expected_false_negative = delegate_false_negative.unwrap_or_default();
        if self.aggregate_false_negative_delegation_bps != expected_false_negative {
            return Err(PsionRouteClassEvaluationError::FieldMismatch {
                field: String::from(
                    "psion_route_class_evaluation_receipt.aggregate_false_negative_delegation_bps",
                ),
                expected: expected_false_negative.to_string(),
                actual: self.aggregate_false_negative_delegation_bps.to_string(),
            });
        }
        if self.receipt_digest != stable_route_class_evaluation_receipt_digest(self) {
            return Err(PsionRouteClassEvaluationError::DigestMismatch {
                kind: String::from("psion_route_class_evaluation_receipt"),
            });
        }
        Ok(())
    }
}

/// Records one route-class evaluation receipt after validating it.
pub fn record_psion_route_class_evaluation_receipt(
    receipt_id: impl Into<String>,
    package: &PsionBenchmarkPackageContract,
    rows: Vec<PsionRouteClassEvaluationRow>,
    summary: impl Into<String>,
    artifact_lineage: &PsionArtifactLineageManifest,
) -> Result<PsionRouteClassEvaluationReceipt, PsionRouteClassEvaluationError> {
    let mut receipt = PsionRouteClassEvaluationReceipt {
        schema_version: String::from(PSION_ROUTE_CLASS_EVALUATION_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        package_id: package.package_id.clone(),
        package_digest: package.package_digest.clone(),
        rows,
        aggregate_route_selection_accuracy_bps: 0,
        aggregate_false_positive_delegation_bps: 0,
        aggregate_false_negative_delegation_bps: 0,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.aggregate_route_selection_accuracy_bps = receipt
        .rows
        .iter()
        .map(|row| row.observed_route_accuracy_bps)
        .sum::<u32>()
        / (receipt.rows.len() as u32).max(1);
    let non_delegate_rows = receipt
        .rows
        .iter()
        .filter(|row| row.route_class != PsionRouteClass::DelegateToExactExecutor)
        .collect::<Vec<_>>();
    receipt.aggregate_false_positive_delegation_bps = non_delegate_rows
        .iter()
        .map(|row| row.false_positive_delegation_bps)
        .sum::<u32>()
        / (non_delegate_rows.len() as u32).max(1);
    receipt.aggregate_false_negative_delegation_bps = receipt
        .rows
        .iter()
        .find(|row| row.route_class == PsionRouteClass::DelegateToExactExecutor)
        .map(|row| row.false_negative_delegation_bps)
        .unwrap_or_default();
    receipt.receipt_digest = stable_route_class_evaluation_receipt_digest(&receipt);
    receipt.validate_against_package(package, artifact_lineage)?;
    Ok(receipt)
}

/// Errors surfaced while validating or recording a route-class evaluation receipt.
#[derive(Debug, Error)]
pub enum PsionRouteClassEvaluationError {
    /// One required field was missing or empty.
    #[error("Psion route-class evaluation receipt is missing required field `{field}`")]
    MissingField {
        /// Field label.
        field: String,
    },
    /// One bps value was out of range.
    #[error(
        "Psion route-class evaluation receipt field `{field}` has invalid bps value `{value}`"
    )]
    InvalidBps {
        /// Field label.
        field: String,
        /// Invalid value.
        value: u32,
    },
    /// One field disagreed with the canonical package or derived aggregate.
    #[error(
        "Psion route-class evaluation receipt field `{field}` expected `{expected}`, found `{actual}`"
    )]
    FieldMismatch {
        /// Field label.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// The schema version drifted from the canonical version.
    #[error("Psion route-class evaluation receipt expected schema `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One row referenced an unknown item.
    #[error("Psion route package `{package_id}` is missing item `{item_id}`")]
    UnknownRoutePackageItem {
        /// Package id.
        package_id: String,
        /// Item id.
        item_id: String,
    },
    /// One row repeated the same item id.
    #[error("Psion route-class evaluation receipt repeated item `{item_id}`")]
    DuplicateRouteItem {
        /// Item id.
        item_id: String,
    },
    /// One required route class was not present in the receipt.
    #[error("Psion route-class evaluation receipt is missing route class `{route_class}`")]
    MissingRouteClass {
        /// Missing route class.
        route_class: String,
    },
    /// The route package was missing from artifact lineage.
    #[error("Psion route package `{package_id}` is missing a benchmark-artifact lineage row")]
    UnknownBenchmarkArtifactLineage {
        /// Package id.
        package_id: String,
    },
    /// The receipt digest no longer matches the payload.
    #[error("Psion route-class evaluation digest drifted for `{kind}`")]
    DigestMismatch {
        /// Artifact kind.
        kind: String,
    },
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionRouteClassEvaluationError> {
    if value.trim().is_empty() {
        return Err(PsionRouteClassEvaluationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionRouteClassEvaluationError> {
    if value > 10_000 {
        return Err(PsionRouteClassEvaluationError::InvalidBps {
            field: String::from(field),
            value,
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionRouteClassEvaluationError> {
    if actual != expected {
        return Err(PsionRouteClassEvaluationError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn stable_route_class_evaluation_receipt_digest(
    receipt: &PsionRouteClassEvaluationReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_route_class_evaluation_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_digest.as_bytes());
    for row in &receipt.rows {
        hasher.update(b"|row|");
        hasher.update(row.item_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", row.route_class).as_bytes());
        hasher.update(b"|");
        hasher.update(row.observed_route_accuracy_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.false_positive_delegation_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.false_negative_delegation_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.detail.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_route_selection_accuracy_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_false_positive_delegation_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .aggregate_false_negative_delegation_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_lineage_manifest() -> PsionArtifactLineageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"
        ))
        .expect("artifact lineage manifest should parse")
    }

    fn route_package() -> PsionBenchmarkPackageContract {
        let catalog: crate::PsionBenchmarkCatalog = serde_json::from_str(include_str!(
            "../../../fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"
        ))
        .expect("benchmark catalog should parse");
        catalog
            .packages
            .into_iter()
            .find(|package| package.package_id == "psion_route_benchmark_v1")
            .expect("route package should exist")
    }

    fn route_receipt() -> PsionRouteClassEvaluationReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/route/psion_route_class_evaluation_receipt_v1.json"
        ))
        .expect("route-class evaluation receipt should parse")
    }

    #[test]
    fn route_class_receipt_validates_against_route_package() {
        route_receipt()
            .validate_against_package(&route_package(), &artifact_lineage_manifest())
            .expect("route-class receipt should validate");
    }

    #[test]
    fn route_class_receipt_requires_all_route_classes() {
        let mut receipt = route_receipt();
        receipt.rows.pop();
        receipt.aggregate_route_selection_accuracy_bps = receipt
            .rows
            .iter()
            .map(|row| row.observed_route_accuracy_bps)
            .sum::<u32>()
            / (receipt.rows.len() as u32).max(1);
        receipt.aggregate_false_positive_delegation_bps = receipt
            .rows
            .iter()
            .filter(|row| row.route_class != PsionRouteClass::DelegateToExactExecutor)
            .map(|row| row.false_positive_delegation_bps)
            .sum::<u32>()
            / (receipt
                .rows
                .iter()
                .filter(|row| row.route_class != PsionRouteClass::DelegateToExactExecutor)
                .count() as u32)
                .max(1);
        receipt.aggregate_false_negative_delegation_bps = receipt
            .rows
            .iter()
            .find(|row| row.route_class == PsionRouteClass::DelegateToExactExecutor)
            .map(|row| row.false_negative_delegation_bps)
            .unwrap_or_default();
        receipt.receipt_digest = stable_route_class_evaluation_receipt_digest(&receipt);
        let error = receipt
            .validate_against_package(&route_package(), &artifact_lineage_manifest())
            .expect_err("receipt should require every route class");
        assert!(matches!(
            error,
            PsionRouteClassEvaluationError::FieldMismatch { .. }
                | PsionRouteClassEvaluationError::MissingRouteClass { .. }
        ));
    }

    #[test]
    fn non_delegate_rows_cannot_report_false_negative_delegation() {
        let mut receipt = route_receipt();
        let row = receipt
            .rows
            .iter_mut()
            .find(|row| row.route_class == PsionRouteClass::AnswerInLanguage)
            .expect("answer row should exist");
        row.false_negative_delegation_bps = 120;
        receipt.receipt_digest = stable_route_class_evaluation_receipt_digest(&receipt);
        let error = receipt
            .validate_against_package(&route_package(), &artifact_lineage_manifest())
            .expect_err("non-delegate rows should not report false-negative delegation");
        assert!(matches!(
            error,
            PsionRouteClassEvaluationError::FieldMismatch { .. }
        ));
    }
}
