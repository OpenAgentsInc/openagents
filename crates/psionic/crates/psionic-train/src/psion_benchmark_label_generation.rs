use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    PsionArtifactLineageManifest, PsionExclusionManifest, PsionSourceLifecycleError,
    PsionSourceLifecycleManifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionBenchmarkExactLabelGrader, PsionBenchmarkExactRefusalGrader,
    PsionBenchmarkExactRouteGrader, PsionBenchmarkGraderInterface, PsionBenchmarkPackageContract,
    PsionBenchmarkPackageError, PsionBenchmarkPackageFamily, PsionBenchmarkRubricGrader,
    PsionRouteClass,
};

/// Stable schema version for one Psion benchmark label-generation receipt.
pub const PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.benchmark_label_generation_receipt.v1";
/// Stable schema version for one Psion benchmark label-generation receipt set.
pub const PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SET_SCHEMA_VERSION: &str =
    "psion.benchmark_label_generation_receipt_set.v1";

/// Label-generation mode for one benchmark item or package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionBenchmarkLabelGenerationMode {
    /// Labels are produced from exact CPU-reference or equivalent exact truth.
    Exact,
    /// Labels are produced from a rubric-backed human-judgment surface.
    RubricBacked,
    /// The package mixes exact and rubric-backed item receipts.
    Hybrid,
}

/// Versioned label-generation logic binding for one benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkLabelLogicBinding {
    /// Stable label-generation logic identifier.
    pub logic_id: String,
    /// Stable logic version.
    pub logic_version: String,
    /// Stable generator or procedure reference.
    pub generator_ref: String,
    /// Short explanation of the logic binding.
    pub detail: String,
}

impl PsionBenchmarkLabelLogicBinding {
    fn validate(&self) -> Result<(), PsionBenchmarkLabelGenerationError> {
        ensure_nonempty(
            self.logic_id.as_str(),
            "psion_benchmark_label_logic.logic_id",
        )?;
        ensure_nonempty(
            self.logic_version.as_str(),
            "psion_benchmark_label_logic.logic_version",
        )?;
        ensure_nonempty(
            self.generator_ref.as_str(),
            "psion_benchmark_label_logic.generator_ref",
        )?;
        ensure_nonempty(self.detail.as_str(), "psion_benchmark_label_logic.detail")?;
        Ok(())
    }
}

/// Exact-truth binding for one exact benchmark label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "truth_kind", rename_all = "snake_case")]
pub enum PsionBenchmarkExactTruthBinding {
    /// CPU-reference truth for exact-label benchmark items.
    CpuReferenceLabel {
        /// Stable runtime or executor reference.
        runtime_ref: String,
        /// Stable truth artifact reference.
        truth_artifact_ref: String,
        /// Stable truth artifact digest.
        truth_artifact_digest: String,
        /// Stable label namespace.
        label_namespace: String,
        /// Accepted labels.
        accepted_labels: Vec<String>,
        /// Short explanation of the exact truth.
        detail: String,
    },
    /// Equivalent exact truth when CPU-reference execution is not the right anchor.
    EquivalentExactLabel {
        /// Stable exact-truth reference.
        truth_ref: String,
        /// Stable truth artifact digest.
        truth_artifact_digest: String,
        /// Stable label namespace.
        label_namespace: String,
        /// Accepted labels.
        accepted_labels: Vec<String>,
        /// Short explanation of the exact truth.
        detail: String,
    },
    /// Exact policy truth for route-selection benchmark items.
    RoutePolicy {
        /// Stable route-boundary reference.
        truth_ref: String,
        /// Stable truth artifact digest.
        truth_artifact_digest: String,
        /// Expected route.
        expected_route: PsionRouteClass,
        /// Short explanation of the exact truth.
        detail: String,
    },
    /// Exact policy truth for refusal benchmark items.
    RefusalPolicy {
        /// Stable refusal-boundary reference.
        truth_ref: String,
        /// Stable truth artifact digest.
        truth_artifact_digest: String,
        /// Accepted refusal reason codes.
        accepted_reason_codes: Vec<String>,
        /// Short explanation of the exact truth.
        detail: String,
    },
}

impl PsionBenchmarkExactTruthBinding {
    fn validate(&self) -> Result<(), PsionBenchmarkLabelGenerationError> {
        match self {
            Self::CpuReferenceLabel {
                runtime_ref,
                truth_artifact_ref,
                truth_artifact_digest,
                label_namespace,
                accepted_labels,
                detail,
            } => {
                ensure_nonempty(runtime_ref.as_str(), "cpu_reference_label.runtime_ref")?;
                ensure_nonempty(
                    truth_artifact_ref.as_str(),
                    "cpu_reference_label.truth_artifact_ref",
                )?;
                ensure_nonempty(
                    truth_artifact_digest.as_str(),
                    "cpu_reference_label.truth_artifact_digest",
                )?;
                ensure_nonempty(
                    label_namespace.as_str(),
                    "cpu_reference_label.label_namespace",
                )?;
                ensure_nonempty(detail.as_str(), "cpu_reference_label.detail")?;
                require_nonempty_unique_strings(
                    accepted_labels.as_slice(),
                    "cpu_reference_label.accepted_labels",
                )?;
            }
            Self::EquivalentExactLabel {
                truth_ref,
                truth_artifact_digest,
                label_namespace,
                accepted_labels,
                detail,
            } => {
                ensure_nonempty(truth_ref.as_str(), "equivalent_exact_label.truth_ref")?;
                ensure_nonempty(
                    truth_artifact_digest.as_str(),
                    "equivalent_exact_label.truth_artifact_digest",
                )?;
                ensure_nonempty(
                    label_namespace.as_str(),
                    "equivalent_exact_label.label_namespace",
                )?;
                ensure_nonempty(detail.as_str(), "equivalent_exact_label.detail")?;
                require_nonempty_unique_strings(
                    accepted_labels.as_slice(),
                    "equivalent_exact_label.accepted_labels",
                )?;
            }
            Self::RoutePolicy {
                truth_ref,
                truth_artifact_digest,
                detail,
                ..
            } => {
                ensure_nonempty(truth_ref.as_str(), "route_policy.truth_ref")?;
                ensure_nonempty(
                    truth_artifact_digest.as_str(),
                    "route_policy.truth_artifact_digest",
                )?;
                ensure_nonempty(detail.as_str(), "route_policy.detail")?;
            }
            Self::RefusalPolicy {
                truth_ref,
                truth_artifact_digest,
                accepted_reason_codes,
                detail,
            } => {
                ensure_nonempty(truth_ref.as_str(), "refusal_policy.truth_ref")?;
                ensure_nonempty(
                    truth_artifact_digest.as_str(),
                    "refusal_policy.truth_artifact_digest",
                )?;
                ensure_nonempty(detail.as_str(), "refusal_policy.detail")?;
                require_nonempty_unique_strings(
                    accepted_reason_codes.as_slice(),
                    "refusal_policy.accepted_reason_codes",
                )?;
            }
        }
        Ok(())
    }

    fn validate_against_grader(
        &self,
        grader: &PsionBenchmarkGraderInterface,
        item_id: &str,
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        match (self, grader) {
            (
                Self::CpuReferenceLabel {
                    label_namespace,
                    accepted_labels,
                    ..
                },
                PsionBenchmarkGraderInterface::ExactLabel(PsionBenchmarkExactLabelGrader {
                    label_namespace: expected_namespace,
                    accepted_labels: expected_labels,
                    ..
                }),
            )
            | (
                Self::EquivalentExactLabel {
                    label_namespace,
                    accepted_labels,
                    ..
                },
                PsionBenchmarkGraderInterface::ExactLabel(PsionBenchmarkExactLabelGrader {
                    label_namespace: expected_namespace,
                    accepted_labels: expected_labels,
                    ..
                }),
            ) => {
                check_string_match(
                    label_namespace.as_str(),
                    expected_namespace.as_str(),
                    &format!(
                        "psion_benchmark_item_label_generation[{item_id}].exact_truth.label_namespace"
                    ),
                )?;
                check_string_set_match(
                    accepted_labels.as_slice(),
                    expected_labels.as_slice(),
                    &format!(
                        "psion_benchmark_item_label_generation[{item_id}].exact_truth.accepted_labels"
                    ),
                )?;
            }
            (
                Self::RoutePolicy { expected_route, .. },
                PsionBenchmarkGraderInterface::ExactRoute(PsionBenchmarkExactRouteGrader {
                    expected_route: expected,
                    ..
                }),
            ) => {
                if expected_route != expected {
                    return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                        field: format!(
                            "psion_benchmark_item_label_generation[{item_id}].exact_truth.expected_route"
                        ),
                        expected: format!("{expected:?}"),
                        actual: format!("{expected_route:?}"),
                    });
                }
            }
            (
                Self::RefusalPolicy {
                    accepted_reason_codes,
                    ..
                },
                PsionBenchmarkGraderInterface::ExactRefusal(PsionBenchmarkExactRefusalGrader {
                    accepted_reason_codes: expected_codes,
                    ..
                }),
            ) => {
                check_string_set_match(
                    accepted_reason_codes.as_slice(),
                    expected_codes.as_slice(),
                    &format!(
                        "psion_benchmark_item_label_generation[{item_id}].exact_truth.accepted_reason_codes"
                    ),
                )?;
            }
            _ => {
                return Err(PsionBenchmarkLabelGenerationError::GraderTruthMismatch {
                    item_id: item_id.to_string(),
                    grader_kind: grader_kind_label(grader).to_string(),
                    truth_kind: exact_truth_kind_label(self).to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Versioned rubric binding for one rubric-backed benchmark label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkRubricVersionBinding {
    /// Stable rubric reference.
    pub rubric_ref: String,
    /// Stable rubric version.
    pub rubric_version: String,
    /// Stable reviewer-guidance reference.
    pub reviewer_guidance_ref: String,
    /// Short explanation of the rubric binding.
    pub detail: String,
}

impl PsionBenchmarkRubricVersionBinding {
    fn validate_against_grader(
        &self,
        grader: &PsionBenchmarkGraderInterface,
        item_id: &str,
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        ensure_nonempty(
            self.rubric_ref.as_str(),
            "psion_benchmark_rubric_binding.rubric_ref",
        )?;
        ensure_nonempty(
            self.rubric_version.as_str(),
            "psion_benchmark_rubric_binding.rubric_version",
        )?;
        ensure_nonempty(
            self.reviewer_guidance_ref.as_str(),
            "psion_benchmark_rubric_binding.reviewer_guidance_ref",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_benchmark_rubric_binding.detail",
        )?;
        match grader {
            PsionBenchmarkGraderInterface::RubricScore(PsionBenchmarkRubricGrader {
                rubric_ref,
                ..
            }) => {
                check_string_match(
                    self.rubric_ref.as_str(),
                    rubric_ref.as_str(),
                    &format!(
                        "psion_benchmark_item_label_generation[{item_id}].rubric_binding.rubric_ref"
                    ),
                )?;
                Ok(())
            }
            _ => Err(
                PsionBenchmarkLabelGenerationError::RubricBindingRequiresRubricGrader {
                    item_id: item_id.to_string(),
                    grader_kind: grader_kind_label(grader).to_string(),
                },
            ),
        }
    }
}

/// Derived-data lineage preserved for one generated benchmark item and label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkDerivedDataLineage {
    /// Stable digest over the generated benchmark item payload.
    pub generated_item_digest: String,
    /// Stable digest over the generated label payload.
    pub generated_label_digest: String,
    /// Parent reviewed sources that produced the derived item or label.
    pub parent_source_ids: Vec<String>,
    /// Stable parent-artifact references when generation passed through another artifact.
    pub parent_artifact_refs: Vec<String>,
    /// Stable generator references used to derive the item or label.
    pub generator_refs: Vec<String>,
    /// Whether the item or label is explicitly derived from the declared parent sources.
    pub derived_from_parent_sources: bool,
    /// Whether the item or label was bound to contamination review.
    pub contamination_review_bound: bool,
    /// Short explanation of the lineage.
    pub detail: String,
}

impl PsionBenchmarkDerivedDataLineage {
    fn validate_against_item(
        &self,
        package: &PsionBenchmarkPackageContract,
        item_id: &str,
        expected_source_ids: &[String],
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        ensure_nonempty(
            self.generated_item_digest.as_str(),
            "psion_benchmark_derived_lineage.generated_item_digest",
        )?;
        ensure_nonempty(
            self.generated_label_digest.as_str(),
            "psion_benchmark_derived_lineage.generated_label_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_benchmark_derived_lineage.detail",
        )?;
        require_nonempty_unique_strings(
            self.parent_source_ids.as_slice(),
            "psion_benchmark_derived_lineage.parent_source_ids",
        )?;
        reject_duplicate_strings(
            self.parent_artifact_refs.as_slice(),
            "psion_benchmark_derived_lineage.parent_artifact_refs",
        )?;
        reject_duplicate_strings(
            self.generator_refs.as_slice(),
            "psion_benchmark_derived_lineage.generator_refs",
        )?;
        if !self.derived_from_parent_sources {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: format!(
                    "psion_benchmark_item_label_generation[{item_id}].derived_data_lineage.derived_from_parent_sources"
                ),
                expected: String::from("true"),
                actual: String::from("false"),
            });
        }
        if !self.contamination_review_bound {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: format!(
                    "psion_benchmark_item_label_generation[{item_id}].derived_data_lineage.contamination_review_bound"
                ),
                expected: String::from("true"),
                actual: String::from("false"),
            });
        }
        check_string_set_match(
            self.parent_source_ids.as_slice(),
            expected_source_ids,
            &format!(
                "psion_benchmark_item_label_generation[{item_id}].derived_data_lineage.parent_source_ids"
            ),
        )?;
        let package_source_ids = package
            .contamination_inputs
            .benchmark_source_ids
            .iter()
            .chain(package.contamination_inputs.held_out_source_ids.iter())
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for source_id in &self.parent_source_ids {
            if !package_source_ids.contains(source_id.as_str()) {
                return Err(PsionBenchmarkLabelGenerationError::UnknownPackageSource {
                    package_id: package.package_id.clone(),
                    source_id: source_id.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Label-generation receipt for one benchmark item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkItemLabelGenerationReceipt {
    /// Stable benchmark item id.
    pub item_id: String,
    /// Stable grader id.
    pub grader_id: String,
    /// Label-generation mode for the item.
    pub generation_mode: PsionBenchmarkLabelGenerationMode,
    /// Versioned label-generation logic.
    pub label_logic: PsionBenchmarkLabelLogicBinding,
    /// Exact truth binding when the item uses exact generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_truth: Option<PsionBenchmarkExactTruthBinding>,
    /// Rubric version binding when the item uses rubric-backed generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubric_binding: Option<PsionBenchmarkRubricVersionBinding>,
    /// Derived-data lineage for the generated item and label.
    pub derived_data_lineage: PsionBenchmarkDerivedDataLineage,
    /// Short explanation of the item receipt.
    pub detail: String,
}

impl PsionBenchmarkItemLabelGenerationReceipt {
    fn validate_against_package_item(
        &self,
        package: &PsionBenchmarkPackageContract,
        package_item: &crate::PsionBenchmarkItem,
        grader: &PsionBenchmarkGraderInterface,
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        ensure_nonempty(
            self.item_id.as_str(),
            "psion_benchmark_item_label_generation.item_id",
        )?;
        ensure_nonempty(
            self.grader_id.as_str(),
            "psion_benchmark_item_label_generation.grader_id",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_benchmark_item_label_generation.detail",
        )?;
        check_string_match(
            self.item_id.as_str(),
            package_item.item_id.as_str(),
            "psion_benchmark_item_label_generation.item_id",
        )?;
        check_string_match(
            self.grader_id.as_str(),
            package_item.grader_id.as_str(),
            "psion_benchmark_item_label_generation.grader_id",
        )?;
        self.label_logic.validate()?;
        self.derived_data_lineage.validate_against_item(
            package,
            package_item.item_id.as_str(),
            package_item.source_ids.as_slice(),
        )?;

        match grader {
            PsionBenchmarkGraderInterface::ExactLabel(_)
            | PsionBenchmarkGraderInterface::ExactRoute(_)
            | PsionBenchmarkGraderInterface::ExactRefusal(_) => {
                if self.generation_mode != PsionBenchmarkLabelGenerationMode::Exact {
                    return Err(
                        PsionBenchmarkLabelGenerationError::GraderGenerationMismatch {
                            item_id: package_item.item_id.clone(),
                            grader_kind: grader_kind_label(grader).to_string(),
                            generation_mode: self.generation_mode,
                        },
                    );
                }
                let exact_truth = self.exact_truth.as_ref().ok_or_else(|| {
                    PsionBenchmarkLabelGenerationError::MissingExactTruth {
                        item_id: package_item.item_id.clone(),
                    }
                })?;
                if self.rubric_binding.is_some() {
                    return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                        field: format!(
                            "psion_benchmark_item_label_generation[{}].rubric_binding",
                            package_item.item_id
                        ),
                        expected: String::from("none"),
                        actual: String::from("present"),
                    });
                }
                exact_truth.validate()?;
                exact_truth.validate_against_grader(grader, package_item.item_id.as_str())?;
            }
            PsionBenchmarkGraderInterface::RubricScore(_) => {
                if self.generation_mode != PsionBenchmarkLabelGenerationMode::RubricBacked {
                    return Err(
                        PsionBenchmarkLabelGenerationError::GraderGenerationMismatch {
                            item_id: package_item.item_id.clone(),
                            grader_kind: grader_kind_label(grader).to_string(),
                            generation_mode: self.generation_mode,
                        },
                    );
                }
                if self.exact_truth.is_some() {
                    return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                        field: format!(
                            "psion_benchmark_item_label_generation[{}].exact_truth",
                            package_item.item_id
                        ),
                        expected: String::from("none"),
                        actual: String::from("present"),
                    });
                }
                let rubric_binding = self.rubric_binding.as_ref().ok_or_else(|| {
                    PsionBenchmarkLabelGenerationError::MissingRubricBinding {
                        item_id: package_item.item_id.clone(),
                    }
                })?;
                rubric_binding.validate_against_grader(grader, package_item.item_id.as_str())?;
            }
        }
        Ok(())
    }
}

/// Label-generation receipt for one benchmark package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkLabelGenerationReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable package id.
    pub package_id: String,
    /// Stable package digest.
    pub package_digest: String,
    /// Package family covered by the receipt.
    pub package_family: PsionBenchmarkPackageFamily,
    /// Stable contamination-input digest.
    pub contamination_input_digest: String,
    /// Derived package-level generation mode.
    pub generation_mode: PsionBenchmarkLabelGenerationMode,
    /// Item-level label-generation receipts.
    pub item_receipts: Vec<PsionBenchmarkItemLabelGenerationReceipt>,
    /// Short explanation of the receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionBenchmarkLabelGenerationReceipt {
    /// Validates the receipt against one benchmark package and the benchmark lineage manifest.
    pub fn validate_against_package(
        &self,
        package: &PsionBenchmarkPackageContract,
        artifact_lineage: &PsionArtifactLineageManifest,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        package.validate_against_context(lifecycle, exclusion)?;
        artifact_lineage.validate_against_lifecycle(lifecycle)?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_label_generation_receipt.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SCHEMA_VERSION {
            return Err(PsionBenchmarkLabelGenerationError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "psion_benchmark_label_generation_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.package_id.as_str(),
            "psion_benchmark_label_generation_receipt.package_id",
        )?;
        ensure_nonempty(
            self.package_digest.as_str(),
            "psion_benchmark_label_generation_receipt.package_digest",
        )?;
        ensure_nonempty(
            self.contamination_input_digest.as_str(),
            "psion_benchmark_label_generation_receipt.contamination_input_digest",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_benchmark_label_generation_receipt.summary",
        )?;
        check_string_match(
            self.package_id.as_str(),
            package.package_id.as_str(),
            "psion_benchmark_label_generation_receipt.package_id",
        )?;
        check_string_match(
            self.package_digest.as_str(),
            package.package_digest.as_str(),
            "psion_benchmark_label_generation_receipt.package_digest",
        )?;
        check_string_match(
            self.contamination_input_digest.as_str(),
            package
                .contamination_inputs
                .contamination_input_digest
                .as_str(),
            "psion_benchmark_label_generation_receipt.contamination_input_digest",
        )?;
        if self.package_family != package.package_family {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: String::from("psion_benchmark_label_generation_receipt.package_family"),
                expected: format!("{:?}", package.package_family),
                actual: format!("{:?}", self.package_family),
            });
        }
        if self.item_receipts.len() != package.items.len() {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: String::from("psion_benchmark_label_generation_receipt.item_receipts"),
                expected: package.items.len().to_string(),
                actual: self.item_receipts.len().to_string(),
            });
        }

        let lineage_row = artifact_lineage
            .benchmark_artifacts
            .iter()
            .find(|artifact| artifact.benchmark_id == package.package_id)
            .ok_or_else(
                || PsionBenchmarkLabelGenerationError::UnknownBenchmarkArtifactLineage {
                    package_id: package.package_id.clone(),
                },
            )?;
        check_string_match(
            lineage_row.benchmark_digest.as_str(),
            package.package_digest.as_str(),
            "psion_benchmark_label_generation_receipt.lineage_row.benchmark_digest",
        )?;

        let package_items = package
            .items
            .iter()
            .map(|item| (item.item_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let graders = package
            .grader_interfaces
            .iter()
            .map(|grader| (grader_id(grader), grader))
            .collect::<BTreeMap<_, _>>();
        let mut seen_item_ids = BTreeSet::new();
        let mut all_parent_source_ids = BTreeSet::new();
        let mut observed_modes = BTreeSet::new();
        for item_receipt in &self.item_receipts {
            if !seen_item_ids.insert(item_receipt.item_id.as_str()) {
                return Err(PsionBenchmarkLabelGenerationError::DuplicateItemReceipt {
                    item_id: item_receipt.item_id.clone(),
                });
            }
            let package_item = package_items
                .get(item_receipt.item_id.as_str())
                .ok_or_else(|| PsionBenchmarkLabelGenerationError::UnknownPackageItem {
                    package_id: package.package_id.clone(),
                    item_id: item_receipt.item_id.clone(),
                })?;
            let grader = graders
                .get(item_receipt.grader_id.as_str())
                .ok_or_else(
                    || PsionBenchmarkLabelGenerationError::UnknownPackageGrader {
                        package_id: package.package_id.clone(),
                        grader_id: item_receipt.grader_id.clone(),
                    },
                )?;
            item_receipt.validate_against_package_item(package, package_item, grader)?;
            observed_modes.insert(item_receipt.generation_mode);
            all_parent_source_ids.extend(
                item_receipt
                    .derived_data_lineage
                    .parent_source_ids
                    .iter()
                    .map(String::as_str),
            );
        }
        let expected_generation_mode =
            derive_package_generation_mode(observed_modes.iter().copied());
        if self.generation_mode != expected_generation_mode {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: String::from("psion_benchmark_label_generation_receipt.generation_mode"),
                expected: format!("{expected_generation_mode:?}"),
                actual: format!("{:?}", self.generation_mode),
            });
        }

        check_string_set_match(
            &all_parent_source_ids
                .iter()
                .map(|source_id| String::from(*source_id))
                .collect::<Vec<_>>(),
            lineage_row.source_ids.as_slice(),
            "psion_benchmark_label_generation_receipt.lineage_row.source_ids",
        )?;

        if self.receipt_digest != stable_label_generation_receipt_digest(self) {
            return Err(PsionBenchmarkLabelGenerationError::DigestMismatch {
                kind: String::from("psion_benchmark_label_generation_receipt"),
            });
        }
        Ok(())
    }
}

/// Receipt set spanning the canonical benchmark catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionBenchmarkLabelGenerationReceiptSet {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt-set id.
    pub receipt_set_id: String,
    /// Bound catalog digest.
    pub catalog_digest: String,
    /// Bound lineage schema version.
    pub lineage_schema_version: String,
    /// Package receipts in deterministic order.
    pub receipts: Vec<PsionBenchmarkLabelGenerationReceipt>,
    /// Short explanation of the receipt set.
    pub summary: String,
    /// Stable digest over the receipt set.
    pub receipt_set_digest: String,
}

impl PsionBenchmarkLabelGenerationReceiptSet {
    /// Validates the receipt set against the benchmark catalog and lineage manifest.
    pub fn validate_against_catalog(
        &self,
        catalog: &crate::PsionBenchmarkCatalog,
        artifact_lineage: &PsionArtifactLineageManifest,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
    ) -> Result<(), PsionBenchmarkLabelGenerationError> {
        catalog.validate_against_context(lifecycle, exclusion)?;
        artifact_lineage.validate_against_lifecycle(lifecycle)?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_benchmark_label_generation_receipt_set.schema_version",
        )?;
        if self.schema_version != PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SET_SCHEMA_VERSION {
            return Err(PsionBenchmarkLabelGenerationError::SchemaVersionMismatch {
                expected: String::from(PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SET_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_set_id.as_str(),
            "psion_benchmark_label_generation_receipt_set.receipt_set_id",
        )?;
        ensure_nonempty(
            self.catalog_digest.as_str(),
            "psion_benchmark_label_generation_receipt_set.catalog_digest",
        )?;
        ensure_nonempty(
            self.lineage_schema_version.as_str(),
            "psion_benchmark_label_generation_receipt_set.lineage_schema_version",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_benchmark_label_generation_receipt_set.summary",
        )?;
        check_string_match(
            self.catalog_digest.as_str(),
            catalog.catalog_digest.as_str(),
            "psion_benchmark_label_generation_receipt_set.catalog_digest",
        )?;
        check_string_match(
            self.lineage_schema_version.as_str(),
            artifact_lineage.schema_version.as_str(),
            "psion_benchmark_label_generation_receipt_set.lineage_schema_version",
        )?;
        if self.receipts.len() != catalog.packages.len() {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: String::from("psion_benchmark_label_generation_receipt_set.receipts"),
                expected: catalog.packages.len().to_string(),
                actual: self.receipts.len().to_string(),
            });
        }

        let mut seen_package_ids = BTreeSet::new();
        for receipt in &self.receipts {
            if !seen_package_ids.insert(receipt.package_id.as_str()) {
                return Err(
                    PsionBenchmarkLabelGenerationError::DuplicatePackageReceipt {
                        package_id: receipt.package_id.clone(),
                    },
                );
            }
            let package = catalog
                .packages
                .iter()
                .find(|package| package.package_id == receipt.package_id)
                .ok_or_else(
                    || PsionBenchmarkLabelGenerationError::UnknownReceiptPackage {
                        package_id: receipt.package_id.clone(),
                    },
                )?;
            receipt.validate_against_package(package, artifact_lineage, lifecycle, exclusion)?;
        }
        if self.receipt_set_digest != stable_label_generation_receipt_set_digest(self) {
            return Err(PsionBenchmarkLabelGenerationError::DigestMismatch {
                kind: String::from("psion_benchmark_label_generation_receipt_set"),
            });
        }
        Ok(())
    }
}

/// Records one benchmark label-generation receipt.
pub fn record_psion_benchmark_label_generation_receipt(
    receipt_id: impl Into<String>,
    package: &PsionBenchmarkPackageContract,
    item_receipts: Vec<PsionBenchmarkItemLabelGenerationReceipt>,
    summary: impl Into<String>,
    artifact_lineage: &PsionArtifactLineageManifest,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
) -> Result<PsionBenchmarkLabelGenerationReceipt, PsionBenchmarkLabelGenerationError> {
    let observed_modes = item_receipts
        .iter()
        .map(|receipt| receipt.generation_mode)
        .collect::<BTreeSet<_>>();
    let mut receipt = PsionBenchmarkLabelGenerationReceipt {
        schema_version: String::from(PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        package_id: package.package_id.clone(),
        package_digest: package.package_digest.clone(),
        package_family: package.package_family,
        contamination_input_digest: package
            .contamination_inputs
            .contamination_input_digest
            .clone(),
        generation_mode: derive_package_generation_mode(observed_modes.iter().copied()),
        item_receipts,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_label_generation_receipt_digest(&receipt);
    receipt.validate_against_package(package, artifact_lineage, lifecycle, exclusion)?;
    Ok(receipt)
}

/// Records one receipt set over the benchmark catalog.
pub fn record_psion_benchmark_label_generation_receipt_set(
    receipt_set_id: impl Into<String>,
    catalog: &crate::PsionBenchmarkCatalog,
    artifact_lineage: &PsionArtifactLineageManifest,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
    receipts: Vec<PsionBenchmarkLabelGenerationReceipt>,
    summary: impl Into<String>,
) -> Result<PsionBenchmarkLabelGenerationReceiptSet, PsionBenchmarkLabelGenerationError> {
    let mut receipt_set = PsionBenchmarkLabelGenerationReceiptSet {
        schema_version: String::from(PSION_BENCHMARK_LABEL_GENERATION_RECEIPT_SET_SCHEMA_VERSION),
        receipt_set_id: receipt_set_id.into(),
        catalog_digest: catalog.catalog_digest.clone(),
        lineage_schema_version: artifact_lineage.schema_version.clone(),
        receipts,
        summary: summary.into(),
        receipt_set_digest: String::new(),
    };
    receipt_set.receipt_set_digest = stable_label_generation_receipt_set_digest(&receipt_set);
    receipt_set.validate_against_catalog(catalog, artifact_lineage, lifecycle, exclusion)?;
    Ok(receipt_set)
}

fn derive_package_generation_mode(
    observed_modes: impl IntoIterator<Item = PsionBenchmarkLabelGenerationMode>,
) -> PsionBenchmarkLabelGenerationMode {
    let observed_modes = observed_modes.into_iter().collect::<BTreeSet<_>>();
    if observed_modes.len() == 1 {
        *observed_modes
            .iter()
            .next()
            .unwrap_or(&PsionBenchmarkLabelGenerationMode::Hybrid)
    } else {
        PsionBenchmarkLabelGenerationMode::Hybrid
    }
}

fn stable_label_generation_receipt_digest(
    receipt: &PsionBenchmarkLabelGenerationReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_label_generation_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.package_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.package_family).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.contamination_input_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.generation_mode).as_bytes());
    for item_receipt in &receipt.item_receipts {
        hasher.update(b"|item|");
        hasher.update(item_receipt.item_id.as_bytes());
        hasher.update(b"|");
        hasher.update(item_receipt.grader_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", item_receipt.generation_mode).as_bytes());
        hasher.update(b"|");
        hasher.update(item_receipt.label_logic.logic_id.as_bytes());
        hasher.update(b"|");
        hasher.update(item_receipt.label_logic.logic_version.as_bytes());
        hasher.update(b"|");
        hasher.update(item_receipt.label_logic.generator_ref.as_bytes());
        if let Some(exact_truth) = &item_receipt.exact_truth {
            hasher.update(b"|exact_truth|");
            hasher.update(exact_truth_kind_label(exact_truth).as_bytes());
            match exact_truth {
                PsionBenchmarkExactTruthBinding::CpuReferenceLabel {
                    runtime_ref,
                    truth_artifact_ref,
                    truth_artifact_digest,
                    label_namespace,
                    accepted_labels,
                    ..
                } => {
                    hasher.update(runtime_ref.as_bytes());
                    hasher.update(b"|");
                    hasher.update(truth_artifact_ref.as_bytes());
                    hasher.update(b"|");
                    hasher.update(truth_artifact_digest.as_bytes());
                    hasher.update(b"|");
                    hasher.update(label_namespace.as_bytes());
                    for label in accepted_labels {
                        hasher.update(b"|label|");
                        hasher.update(label.as_bytes());
                    }
                }
                PsionBenchmarkExactTruthBinding::EquivalentExactLabel {
                    truth_ref,
                    truth_artifact_digest,
                    label_namespace,
                    accepted_labels,
                    ..
                } => {
                    hasher.update(truth_ref.as_bytes());
                    hasher.update(b"|");
                    hasher.update(truth_artifact_digest.as_bytes());
                    hasher.update(b"|");
                    hasher.update(label_namespace.as_bytes());
                    for label in accepted_labels {
                        hasher.update(b"|label|");
                        hasher.update(label.as_bytes());
                    }
                }
                PsionBenchmarkExactTruthBinding::RoutePolicy {
                    truth_ref,
                    truth_artifact_digest,
                    expected_route,
                    ..
                } => {
                    hasher.update(truth_ref.as_bytes());
                    hasher.update(b"|");
                    hasher.update(truth_artifact_digest.as_bytes());
                    hasher.update(b"|");
                    hasher.update(format!("{expected_route:?}").as_bytes());
                }
                PsionBenchmarkExactTruthBinding::RefusalPolicy {
                    truth_ref,
                    truth_artifact_digest,
                    accepted_reason_codes,
                    ..
                } => {
                    hasher.update(truth_ref.as_bytes());
                    hasher.update(b"|");
                    hasher.update(truth_artifact_digest.as_bytes());
                    for code in accepted_reason_codes {
                        hasher.update(b"|reason|");
                        hasher.update(code.as_bytes());
                    }
                }
            }
        }
        if let Some(rubric_binding) = &item_receipt.rubric_binding {
            hasher.update(b"|rubric|");
            hasher.update(rubric_binding.rubric_ref.as_bytes());
            hasher.update(b"|");
            hasher.update(rubric_binding.rubric_version.as_bytes());
            hasher.update(b"|");
            hasher.update(rubric_binding.reviewer_guidance_ref.as_bytes());
        }
        hasher.update(b"|lineage|");
        hasher.update(
            item_receipt
                .derived_data_lineage
                .generated_item_digest
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            item_receipt
                .derived_data_lineage
                .generated_label_digest
                .as_bytes(),
        );
        for source_id in &item_receipt.derived_data_lineage.parent_source_ids {
            hasher.update(b"|source|");
            hasher.update(source_id.as_bytes());
        }
        for artifact_ref in &item_receipt.derived_data_lineage.parent_artifact_refs {
            hasher.update(b"|artifact|");
            hasher.update(artifact_ref.as_bytes());
        }
        for generator_ref in &item_receipt.derived_data_lineage.generator_refs {
            hasher.update(b"|generator|");
            hasher.update(generator_ref.as_bytes());
        }
    }
    hasher.update(b"|summary|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_label_generation_receipt_set_digest(
    receipt_set: &PsionBenchmarkLabelGenerationReceiptSet,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_benchmark_label_generation_receipt_set|");
    hasher.update(receipt_set.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt_set.receipt_set_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt_set.catalog_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt_set.lineage_schema_version.as_bytes());
    for receipt in &receipt_set.receipts {
        hasher.update(b"|receipt|");
        hasher.update(receipt.receipt_digest.as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(receipt_set.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionBenchmarkLabelGenerationError> {
    if value.trim().is_empty() {
        return Err(PsionBenchmarkLabelGenerationError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionBenchmarkLabelGenerationError> {
    if actual != expected {
        return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn check_string_set_match(
    actual: &[String],
    expected: &[String],
    field: &str,
) -> Result<(), PsionBenchmarkLabelGenerationError> {
    let actual = actual.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
            field: field.to_string(),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionBenchmarkLabelGenerationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(PsionBenchmarkLabelGenerationError::FieldMismatch {
                field: field.to_string(),
                expected: String::from("unique values"),
                actual: value.clone(),
            });
        }
    }
    Ok(())
}

fn require_nonempty_unique_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionBenchmarkLabelGenerationError> {
    if values.is_empty() {
        return Err(PsionBenchmarkLabelGenerationError::MissingField {
            field: field.to_string(),
        });
    }
    reject_duplicate_strings(values, field)
}

fn grader_kind_label(grader: &PsionBenchmarkGraderInterface) -> &'static str {
    match grader {
        PsionBenchmarkGraderInterface::ExactLabel(_) => "exact_label",
        PsionBenchmarkGraderInterface::RubricScore(_) => "rubric_score",
        PsionBenchmarkGraderInterface::ExactRoute(_) => "exact_route",
        PsionBenchmarkGraderInterface::ExactRefusal(_) => "exact_refusal",
    }
}

fn grader_id(grader: &PsionBenchmarkGraderInterface) -> &str {
    match grader {
        PsionBenchmarkGraderInterface::ExactLabel(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::RubricScore(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::ExactRoute(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::ExactRefusal(grader) => grader.grader_id.as_str(),
    }
}

fn exact_truth_kind_label(binding: &PsionBenchmarkExactTruthBinding) -> &'static str {
    match binding {
        PsionBenchmarkExactTruthBinding::CpuReferenceLabel { .. } => "cpu_reference_label",
        PsionBenchmarkExactTruthBinding::EquivalentExactLabel { .. } => "equivalent_exact_label",
        PsionBenchmarkExactTruthBinding::RoutePolicy { .. } => "route_policy",
        PsionBenchmarkExactTruthBinding::RefusalPolicy { .. } => "refusal_policy",
    }
}

/// Validation failures for the Psion benchmark label-generation layer.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionBenchmarkLabelGenerationError {
    /// One required field was missing.
    #[error("Psion benchmark label-generation contract is missing `{field}`")]
    MissingField {
        /// Missing field.
        field: String,
    },
    /// One field drifted from the required value.
    #[error("Psion benchmark label-generation field mismatch on `{field}`: expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// One schema version drifted.
    #[error(
        "Psion benchmark label-generation schema mismatch: expected `{expected}`, found `{actual}`"
    )]
    SchemaVersionMismatch {
        /// Expected schema.
        expected: String,
        /// Actual schema.
        actual: String,
    },
    /// One receipt repeated a package id.
    #[error("Psion benchmark label-generation receipt set repeats package `{package_id}`")]
    DuplicatePackageReceipt {
        /// Package id.
        package_id: String,
    },
    /// One package receipt repeated an item id.
    #[error("Psion benchmark label-generation receipt repeats item `{item_id}`")]
    DuplicateItemReceipt {
        /// Item id.
        item_id: String,
    },
    /// One receipt referenced a package outside the catalog.
    #[error("Psion benchmark label-generation receipt references unknown package `{package_id}`")]
    UnknownReceiptPackage {
        /// Package id.
        package_id: String,
    },
    /// One receipt referenced an item outside its package contract.
    #[error("Psion benchmark label-generation receipt for package `{package_id}` references unknown item `{item_id}`")]
    UnknownPackageItem {
        /// Package id.
        package_id: String,
        /// Item id.
        item_id: String,
    },
    /// One receipt referenced a grader outside its package contract.
    #[error("Psion benchmark label-generation receipt for package `{package_id}` references unknown grader `{grader_id}`")]
    UnknownPackageGrader {
        /// Package id.
        package_id: String,
        /// Grader id.
        grader_id: String,
    },
    /// The benchmark lineage manifest did not contain the package artifact.
    #[error("Psion benchmark lineage manifest is missing package `{package_id}`")]
    UnknownBenchmarkArtifactLineage {
        /// Package id.
        package_id: String,
    },
    /// One item used a generation mode incompatible with its package grader.
    #[error("Psion benchmark item `{item_id}` uses generation mode `{generation_mode:?}` incompatible with grader `{grader_kind}`")]
    GraderGenerationMismatch {
        /// Item id.
        item_id: String,
        /// Grader kind.
        grader_kind: String,
        /// Generation mode.
        generation_mode: PsionBenchmarkLabelGenerationMode,
    },
    /// One exact-truth binding did not match the package grader.
    #[error("Psion benchmark item `{item_id}` exact truth `{truth_kind}` does not match grader `{grader_kind}`")]
    GraderTruthMismatch {
        /// Item id.
        item_id: String,
        /// Grader kind.
        grader_kind: String,
        /// Exact truth kind.
        truth_kind: String,
    },
    /// One rubric binding appeared on a non-rubric grader.
    #[error("Psion benchmark item `{item_id}` rubric binding requires a rubric grader, found `{grader_kind}`")]
    RubricBindingRequiresRubricGrader {
        /// Item id.
        item_id: String,
        /// Grader kind.
        grader_kind: String,
    },
    /// One exact-truth binding was required but missing.
    #[error("Psion benchmark item `{item_id}` is missing exact truth")]
    MissingExactTruth {
        /// Item id.
        item_id: String,
    },
    /// One rubric binding was required but missing.
    #[error("Psion benchmark item `{item_id}` is missing a rubric binding")]
    MissingRubricBinding {
        /// Item id.
        item_id: String,
    },
    /// One derived lineage row referenced a source outside the package.
    #[error("Psion benchmark package `{package_id}` references source `{source_id}` outside its contamination inputs")]
    UnknownPackageSource {
        /// Package id.
        package_id: String,
        /// Source id.
        source_id: String,
    },
    /// One digest drifted from the canonical payload.
    #[error("Psion benchmark label-generation digest drifted for `{kind}`")]
    DigestMismatch {
        /// Artifact kind.
        kind: String,
    },
    /// Benchmark package validation failed underneath the label-generation layer.
    #[error(transparent)]
    BenchmarkPackage(#[from] PsionBenchmarkPackageError),
    /// Source lifecycle validation failed underneath the label-generation layer.
    #[error(transparent)]
    SourceLifecycle(#[from] PsionSourceLifecycleError),
}

#[cfg(test)]
mod tests {
    use super::{
        record_psion_benchmark_label_generation_receipt,
        record_psion_benchmark_label_generation_receipt_set, PsionBenchmarkDerivedDataLineage,
        PsionBenchmarkExactTruthBinding, PsionBenchmarkItemLabelGenerationReceipt,
        PsionBenchmarkLabelGenerationError, PsionBenchmarkLabelGenerationMode,
        PsionBenchmarkLabelLogicBinding, PsionBenchmarkRubricVersionBinding,
    };
    use psionic_data::{
        PsionArtifactLineageManifest, PsionExclusionManifest, PsionSourceLifecycleManifest,
    };

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle manifest should parse")
    }

    fn exclusion_manifest() -> PsionExclusionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/isolation/psion_exclusion_manifest_v1.json"
        ))
        .expect("exclusion manifest should parse")
    }

    fn artifact_lineage_manifest() -> PsionArtifactLineageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"
        ))
        .expect("artifact lineage manifest should parse")
    }

    fn benchmark_catalog() -> crate::PsionBenchmarkCatalog {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"
        ))
        .expect("benchmark catalog should parse")
    }

    fn base_item_receipt(
        package: &crate::PsionBenchmarkPackageContract,
        item_id: &str,
    ) -> PsionBenchmarkItemLabelGenerationReceipt {
        let item = package
            .items
            .iter()
            .find(|item| item.item_id == item_id)
            .expect("package item should exist");
        let grader = package
            .grader_interfaces
            .iter()
            .find(|grader| super::grader_id(grader) == item.grader_id)
            .expect("grader should exist");
        match grader {
            crate::PsionBenchmarkGraderInterface::RubricScore(rubric) => {
                PsionBenchmarkItemLabelGenerationReceipt {
                    item_id: item.item_id.clone(),
                    grader_id: item.grader_id.clone(),
                    generation_mode: PsionBenchmarkLabelGenerationMode::RubricBacked,
                    label_logic: PsionBenchmarkLabelLogicBinding {
                        logic_id: format!("{}-labelgen", item.item_id),
                        logic_version: String::from("v1"),
                        generator_ref: String::from("generator://psion/benchmark/rubric-assembly-v1"),
                        detail: String::from(
                            "Rubric-backed labels preserve the generator and rubric version explicitly.",
                        ),
                    },
                    exact_truth: None,
                    rubric_binding: Some(PsionBenchmarkRubricVersionBinding {
                        rubric_ref: rubric.rubric_ref.clone(),
                        rubric_version: String::from("2026.03.22"),
                        reviewer_guidance_ref: String::from(
                            "guidance://psion/benchmark/rubric/reasoning-v1",
                        ),
                        detail: String::from(
                            "Rubric-backed items pin the rubric reference, version, and reviewer guidance.",
                        ),
                    }),
                    derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                        generated_item_digest: format!("derived-item-{item_id}"),
                        generated_label_digest: format!("derived-label-{item_id}"),
                        parent_source_ids: item.source_ids.clone(),
                        parent_artifact_refs: vec![String::from("artifact://psion/benchmark/source-pack-v1")],
                        generator_refs: vec![String::from("generator://psion/benchmark/rubric-assembly-v1")],
                        derived_from_parent_sources: true,
                        contamination_review_bound: true,
                        detail: String::from(
                            "Derived lineage keeps the parent source ids and the generator reference explicit.",
                        ),
                    },
                    detail: String::from(
                        "Rubric-backed benchmark item records a rubric version and derived-data lineage.",
                    ),
                }
            }
            crate::PsionBenchmarkGraderInterface::ExactLabel(grader) => {
                PsionBenchmarkItemLabelGenerationReceipt {
                    item_id: item.item_id.clone(),
                    grader_id: item.grader_id.clone(),
                    generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                    label_logic: PsionBenchmarkLabelLogicBinding {
                        logic_id: format!("{}-labelgen", item.item_id),
                        logic_version: String::from("v1"),
                        generator_ref: String::from("generator://psion/benchmark/exact-label-v1"),
                        detail: String::from(
                            "Exact labels pin the exact-label generation procedure version.",
                        ),
                    },
                    exact_truth: Some(PsionBenchmarkExactTruthBinding::EquivalentExactLabel {
                        truth_ref: String::from("truth://psion/benchmark/spec-extract-v1"),
                        truth_artifact_digest: format!("truth-digest-{item_id}"),
                        label_namespace: grader.label_namespace.clone(),
                        accepted_labels: grader.accepted_labels.clone(),
                        detail: String::from(
                            "Equivalent exact truth keeps the extracted exact label set versioned.",
                        ),
                    }),
                    rubric_binding: None,
                    derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                        generated_item_digest: format!("derived-item-{item_id}"),
                        generated_label_digest: format!("derived-label-{item_id}"),
                        parent_source_ids: item.source_ids.clone(),
                        parent_artifact_refs: vec![String::from("artifact://psion/benchmark/source-pack-v1")],
                        generator_refs: vec![String::from("generator://psion/benchmark/exact-label-v1")],
                        derived_from_parent_sources: true,
                        contamination_review_bound: true,
                        detail: String::from(
                            "Derived lineage keeps exact-label items tied to their parent sources and generator.",
                        ),
                    },
                    detail: String::from(
                        "Exact benchmark item records equivalent exact truth and derived-data lineage.",
                    ),
                }
            }
            crate::PsionBenchmarkGraderInterface::ExactRoute(grader) => {
                PsionBenchmarkItemLabelGenerationReceipt {
                    item_id: item.item_id.clone(),
                    grader_id: item.grader_id.clone(),
                    generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                    label_logic: PsionBenchmarkLabelLogicBinding {
                        logic_id: format!("{}-labelgen", item.item_id),
                        logic_version: String::from("v1"),
                        generator_ref: String::from("generator://psion/benchmark/route-policy-v1"),
                        detail: String::from("Exact route labels pin the route-policy version."),
                    },
                    exact_truth: Some(PsionBenchmarkExactTruthBinding::RoutePolicy {
                        truth_ref: String::from("route://psion/exactness_boundary"),
                        truth_artifact_digest: format!("truth-digest-{item_id}"),
                        expected_route: grader.expected_route,
                        detail: String::from(
                            "Route labels are produced from an exact policy boundary.",
                        ),
                    }),
                    rubric_binding: None,
                    derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                        generated_item_digest: format!("derived-item-{item_id}"),
                        generated_label_digest: format!("derived-label-{item_id}"),
                        parent_source_ids: item.source_ids.clone(),
                        parent_artifact_refs: vec![String::from("artifact://psion/benchmark/source-pack-v1")],
                        generator_refs: vec![String::from("generator://psion/benchmark/route-policy-v1")],
                        derived_from_parent_sources: true,
                        contamination_review_bound: true,
                        detail: String::from(
                            "Route lineage keeps the parent sources and policy generator explicit.",
                        ),
                    },
                    detail: String::from("Exact route item records route-policy truth and lineage."),
                }
            }
            crate::PsionBenchmarkGraderInterface::ExactRefusal(grader) => {
                PsionBenchmarkItemLabelGenerationReceipt {
                    item_id: item.item_id.clone(),
                    grader_id: item.grader_id.clone(),
                    generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                    label_logic: PsionBenchmarkLabelLogicBinding {
                        logic_id: format!("{}-labelgen", item.item_id),
                        logic_version: String::from("v1"),
                        generator_ref: String::from("generator://psion/benchmark/refusal-policy-v1"),
                        detail: String::from("Exact refusal labels pin the refusal-policy version."),
                    },
                    exact_truth: Some(PsionBenchmarkExactTruthBinding::RefusalPolicy {
                        truth_ref: String::from("refusal://psion/benchmark/boundary-v1"),
                        truth_artifact_digest: format!("truth-digest-{item_id}"),
                        accepted_reason_codes: grader.accepted_reason_codes.clone(),
                        detail: String::from(
                            "Refusal labels are produced from an exact refusal policy boundary.",
                        ),
                    }),
                    rubric_binding: None,
                    derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                        generated_item_digest: format!("derived-item-{item_id}"),
                        generated_label_digest: format!("derived-label-{item_id}"),
                        parent_source_ids: item.source_ids.clone(),
                        parent_artifact_refs: vec![String::from("artifact://psion/benchmark/source-pack-v1")],
                        generator_refs: vec![String::from("generator://psion/benchmark/refusal-policy-v1")],
                        derived_from_parent_sources: true,
                        contamination_review_bound: true,
                        detail: String::from(
                            "Refusal lineage keeps the parent sources and refusal generator explicit.",
                        ),
                    },
                    detail: String::from(
                        "Exact refusal item records refusal-policy truth and lineage.",
                    ),
                }
            }
        }
    }

    fn receipt_for_package(
        package_id: &str,
    ) -> Result<super::PsionBenchmarkLabelGenerationReceipt, PsionBenchmarkLabelGenerationError>
    {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let artifact_lineage = artifact_lineage_manifest();
        let catalog = benchmark_catalog();
        let package = catalog
            .packages
            .iter()
            .find(|package| package.package_id == package_id)
            .expect("package should exist");
        let item_receipts = package
            .items
            .iter()
            .map(|item| base_item_receipt(package, item.item_id.as_str()))
            .collect::<Vec<_>>();
        record_psion_benchmark_label_generation_receipt(
            format!("{package_id}-labelgen-receipt-v1"),
            package,
            item_receipts,
            format!("Label-generation receipt for `{package_id}`."),
            &artifact_lineage,
            &lifecycle,
            &exclusion,
        )
    }

    #[test]
    fn label_generation_receipt_set_validates_against_catalog() {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let artifact_lineage = artifact_lineage_manifest();
        let catalog = benchmark_catalog();
        let receipts = catalog
            .packages
            .iter()
            .map(|package| {
                receipt_for_package(package.package_id.as_str())
                    .expect("package receipt should build")
            })
            .collect::<Vec<_>>();
        let receipt_set = record_psion_benchmark_label_generation_receipt_set(
            "psion-benchmark-label-generation-receipt-set-v1",
            &catalog,
            &artifact_lineage,
            &lifecycle,
            &exclusion,
            receipts,
            "Canonical receipt set proving benchmark label generation stays bound to exact truth, rubric versions, and derived-data lineage across the main Psion benchmark families.",
        )
        .expect("receipt set should validate");
        assert_eq!(
            receipt_set.receipts.len(),
            catalog.packages.len(),
            "one label-generation receipt should exist per benchmark package"
        );
        let engineering = receipt_set
            .receipts
            .iter()
            .find(|receipt| receipt.package_id == "psion_engineering_spec_benchmark_v1")
            .expect("engineering receipt should exist");
        assert_eq!(
            engineering.generation_mode,
            PsionBenchmarkLabelGenerationMode::Hybrid,
            "engineering package should prove the mixed exact-plus-rubric package posture"
        );
    }

    #[test]
    fn exact_items_require_exact_truth() {
        let catalog = benchmark_catalog();
        let package = catalog
            .packages
            .iter()
            .find(|package| package.package_id == "psion_normative_spec_benchmark_v1")
            .expect("normative package should exist");
        let mut item_receipts = package
            .items
            .iter()
            .map(|item| base_item_receipt(package, item.item_id.as_str()))
            .collect::<Vec<_>>();
        let item_receipt = item_receipts
            .iter_mut()
            .find(|item| item.item_id == "spec-case-definition")
            .expect("definition item receipt should exist");
        item_receipt.exact_truth = None;
        let error = record_psion_benchmark_label_generation_receipt(
            "psion-normative-labelgen-receipt-v1",
            package,
            item_receipts,
            "Normative label-generation receipt.",
            &artifact_lineage_manifest(),
            &lifecycle_manifest(),
            &exclusion_manifest(),
        )
        .expect_err("exact item without exact truth should fail");
        assert!(matches!(
            error,
            PsionBenchmarkLabelGenerationError::MissingExactTruth { .. }
        ));
    }

    #[test]
    fn derived_lineage_sources_must_match_package_item_sources() {
        let catalog = benchmark_catalog();
        let package = catalog
            .packages
            .iter()
            .find(|package| package.package_id == "psion_route_benchmark_v1")
            .expect("route package should exist");
        let mut item_receipt = base_item_receipt(package, "route-case-answer");
        item_receipt.derived_data_lineage.parent_source_ids =
            vec![String::from("wasm_core_spec_release_2")];
        let error = record_psion_benchmark_label_generation_receipt(
            "psion-route-labelgen-receipt-v1",
            package,
            vec![item_receipt],
            "Route label-generation receipt.",
            &artifact_lineage_manifest(),
            &lifecycle_manifest(),
            &exclusion_manifest(),
        )
        .expect_err("lineage source drift should fail");
        assert!(matches!(
            error,
            PsionBenchmarkLabelGenerationError::FieldMismatch { .. }
        ));
    }
}
