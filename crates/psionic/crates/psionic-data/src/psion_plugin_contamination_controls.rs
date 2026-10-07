use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetSplitKind, PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError,
    PsionPluginControllerSurface, PsionPluginTrainingDerivationBundle,
    PsionPluginTrainingDerivationError,
    build_psion_plugin_conditioned_dataset_bundle_from_derivation,
    build_psion_plugin_training_derivation_bundle,
};

/// Stable schema version for the first plugin-aware contamination bundle.
pub const PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_contamination_bundle.v1";
/// Stable committed output ref for the first contamination bundle.
pub const PSION_PLUGIN_CONTAMINATION_BUNDLE_REF: &str = "fixtures/psion/plugins/datasets/psion_plugin_contamination_controls_v1/psion_plugin_contamination_bundle.json";
/// Stable committed output ref for the first mixed contamination bundle.
pub const PSION_PLUGIN_MIXED_CONTAMINATION_BUNDLE_REF: &str = "fixtures/psion/plugins/datasets/psion_plugin_mixed_contamination_controls_v1/psion_plugin_mixed_contamination_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginContaminationItemKind {
    /// Train split record used for SFT.
    SftTrainRecord,
    /// Held-out split record reserved for eval and benchmark construction.
    HeldOutEvalRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PsionPluginTraceSourceRef {
    /// Stable source bundle ref.
    pub source_bundle_ref: String,
    /// Stable source bundle id.
    pub source_bundle_id: String,
    /// Stable source bundle digest.
    pub source_bundle_digest: String,
    /// Stable source case id inside the bundle.
    pub source_case_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginParentLineageRow {
    /// Stable lineage row identifier.
    pub lineage_id: String,
    /// Stable item ref inside the downstream split surface.
    pub item_ref: String,
    /// Item kind represented by the row.
    pub item_kind: PsionPluginContaminationItemKind,
    /// Dataset split represented by the row.
    pub split_kind: DatasetSplitKind,
    /// Stable parent training record id.
    pub training_record_id: String,
    /// Stable parent training record digest.
    pub training_record_digest: String,
    /// Stable workflow case id for contamination review.
    pub workflow_case_id: String,
    /// Controller surface that produced or anchored the row.
    pub controller_surface: PsionPluginControllerSurface,
    /// Source trace identity preserved from the parent record.
    pub source_trace: PsionPluginTraceSourceRef,
    /// Receipt refs preserved from the parent record.
    pub receipt_refs: Vec<String>,
    /// Receipt digests preserved from the parent record.
    pub receipt_digests: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginTraceDisjointGroup {
    /// Stable split kind for the group.
    pub split_kind: DatasetSplitKind,
    /// Stable workflow case ids represented in the group.
    pub workflow_case_ids: Vec<String>,
    /// Stable source case ids represented in the group.
    pub source_case_ids: Vec<String>,
    /// Stable receipt refs represented in the group.
    pub receipt_refs: Vec<String>,
    /// Short explanation of the disjointness boundary.
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginExclusionManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Source derivation bundle ref.
    pub source_derivation_bundle_ref: String,
    /// Source derivation bundle digest.
    pub source_derivation_bundle_digest: String,
    /// Stable dataset identity that this manifest protects.
    pub dataset_identity: String,
    /// Held-out trace sources excluded from training.
    pub training_excluded_trace_sources: Vec<PsionPluginTraceSourceRef>,
    /// Train trace sources excluded from benchmark and held-out packaging.
    pub benchmark_excluded_trace_sources: Vec<PsionPluginTraceSourceRef>,
    /// Held-out receipt refs excluded from training.
    pub training_excluded_receipt_refs: Vec<String>,
    /// Train receipt refs excluded from held-out benchmark packaging.
    pub benchmark_excluded_receipt_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginContaminationBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Source derivation bundle ref.
    pub source_derivation_bundle_ref: String,
    /// Source derivation bundle digest.
    pub source_derivation_bundle_digest: String,
    /// Source dataset bundle ref.
    pub dataset_bundle_ref: String,
    /// Source dataset bundle digest.
    pub dataset_bundle_digest: String,
    /// Stable dataset identity protected by the bundle.
    pub dataset_identity: String,
    /// Parent-lineage rows across SFT and held-out eval items.
    pub parent_lineage_rows: Vec<PsionPluginParentLineageRow>,
    /// Split-scoped trace-disjoint groups.
    pub trace_disjoint_groups: Vec<PsionPluginTraceDisjointGroup>,
    /// Explicit plugin-aware exclusion manifest.
    pub exclusion_manifest: PsionPluginExclusionManifest,
    /// Plain-language claim boundary for the bundle.
    pub claim_boundary: String,
    /// Short machine-readable summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPluginContaminationBundle {
    /// Writes the contamination bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginContaminationError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginContaminationError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginContaminationError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the contamination bundle.
    pub fn validate(&self) -> Result<(), PsionPluginContaminationError> {
        if self.schema_version != PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION {
            return Err(PsionPluginContaminationError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.source_derivation_bundle_ref.as_str(),
            "contamination_bundle.source_derivation_bundle_ref",
        )?;
        ensure_nonempty(
            self.source_derivation_bundle_digest.as_str(),
            "contamination_bundle.source_derivation_bundle_digest",
        )?;
        ensure_nonempty(
            self.dataset_bundle_ref.as_str(),
            "contamination_bundle.dataset_bundle_ref",
        )?;
        ensure_nonempty(
            self.dataset_bundle_digest.as_str(),
            "contamination_bundle.dataset_bundle_digest",
        )?;
        ensure_nonempty(
            self.dataset_identity.as_str(),
            "contamination_bundle.dataset_identity",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "contamination_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.summary.as_str(), "contamination_bundle.summary")?;
        ensure_nonempty(
            self.bundle_digest.as_str(),
            "contamination_bundle.bundle_digest",
        )?;
        if self.parent_lineage_rows.is_empty() {
            return Err(PsionPluginContaminationError::MissingField {
                field: String::from("contamination_bundle.parent_lineage_rows"),
            });
        }
        if self.trace_disjoint_groups.is_empty() {
            return Err(PsionPluginContaminationError::MissingField {
                field: String::from("contamination_bundle.trace_disjoint_groups"),
            });
        }

        let mut seen_lineage_ids = BTreeSet::new();
        let mut seen_item_refs = BTreeSet::new();
        for row in &self.parent_lineage_rows {
            validate_lineage_row(row)?;
            if !seen_lineage_ids.insert(row.lineage_id.as_str()) {
                return Err(PsionPluginContaminationError::DuplicateLineageId {
                    lineage_id: row.lineage_id.clone(),
                });
            }
            if !seen_item_refs.insert(row.item_ref.as_str()) {
                return Err(PsionPluginContaminationError::DuplicateItemRef {
                    item_ref: row.item_ref.clone(),
                });
            }
        }

        let train_rows = self
            .parent_lineage_rows
            .iter()
            .filter(|row| row.split_kind == DatasetSplitKind::Train)
            .collect::<Vec<_>>();
        if train_rows.is_empty() {
            return Err(PsionPluginContaminationError::MissingField {
                field: String::from("contamination_bundle.train_rows"),
            });
        }
        let held_out_rows = self
            .parent_lineage_rows
            .iter()
            .filter(|row| row.split_kind == DatasetSplitKind::HeldOut)
            .collect::<Vec<_>>();
        if held_out_rows.is_empty() {
            return Err(PsionPluginContaminationError::MissingField {
                field: String::from("contamination_bundle.held_out_rows"),
            });
        }
        if train_rows
            .iter()
            .any(|row| row.item_kind != PsionPluginContaminationItemKind::SftTrainRecord)
        {
            return Err(PsionPluginContaminationError::SplitKindItemKindMismatch {
                split_kind: String::from("train"),
                expected_item_kind: String::from("sft_train_record"),
            });
        }
        if held_out_rows
            .iter()
            .any(|row| row.item_kind != PsionPluginContaminationItemKind::HeldOutEvalRecord)
        {
            return Err(PsionPluginContaminationError::SplitKindItemKindMismatch {
                split_kind: String::from("held_out"),
                expected_item_kind: String::from("held_out_eval_record"),
            });
        }

        let expected_groups =
            expected_trace_disjoint_groups(&self.parent_lineage_rows.iter().collect::<Vec<_>>());
        if self.trace_disjoint_groups != expected_groups {
            return Err(PsionPluginContaminationError::TraceDisjointGroupDrift);
        }
        ensure_trace_disjoint(&train_rows, &held_out_rows)?;
        self.exclusion_manifest
            .validate_against_rows(&train_rows, &held_out_rows, self)?;
        Ok(())
    }
}

impl PsionPluginExclusionManifest {
    fn validate_against_rows(
        &self,
        train_rows: &[&PsionPluginParentLineageRow],
        held_out_rows: &[&PsionPluginParentLineageRow],
        bundle: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginContaminationError> {
        if self.schema_version != PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION {
            return Err(PsionPluginContaminationError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        if self.source_derivation_bundle_ref != bundle.source_derivation_bundle_ref {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.source_derivation_bundle_ref"),
                expected: bundle.source_derivation_bundle_ref.clone(),
                actual: self.source_derivation_bundle_ref.clone(),
            });
        }
        if self.source_derivation_bundle_digest != bundle.source_derivation_bundle_digest {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.source_derivation_bundle_digest"),
                expected: bundle.source_derivation_bundle_digest.clone(),
                actual: self.source_derivation_bundle_digest.clone(),
            });
        }
        if self.dataset_identity != bundle.dataset_identity {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.dataset_identity"),
                expected: bundle.dataset_identity.clone(),
                actual: self.dataset_identity.clone(),
            });
        }

        let expected_training_sources = unique_trace_sources(held_out_rows);
        let expected_benchmark_sources = unique_trace_sources(train_rows);
        if self.training_excluded_trace_sources != expected_training_sources {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.training_excluded_trace_sources"),
                expected: String::from("held-out trace source set"),
                actual: String::from("drifted from held-out trace source set"),
            });
        }
        if self.benchmark_excluded_trace_sources != expected_benchmark_sources {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.benchmark_excluded_trace_sources"),
                expected: String::from("train trace source set"),
                actual: String::from("drifted from train trace source set"),
            });
        }

        let expected_training_receipts = unique_receipt_refs(held_out_rows);
        let expected_benchmark_receipts = unique_receipt_refs(train_rows);
        if self.training_excluded_receipt_refs != expected_training_receipts {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.training_excluded_receipt_refs"),
                expected: String::from("held-out receipt ref set"),
                actual: String::from("drifted from held-out receipt ref set"),
            });
        }
        if self.benchmark_excluded_receipt_refs != expected_benchmark_receipts {
            return Err(PsionPluginContaminationError::FieldMismatch {
                field: String::from("exclusion_manifest.benchmark_excluded_receipt_refs"),
                expected: String::from("train receipt ref set"),
                actual: String::from("drifted from train receipt ref set"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginContaminationError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate lineage id `{lineage_id}`")]
    DuplicateLineageId { lineage_id: String },
    #[error("duplicate item ref `{item_ref}`")]
    DuplicateItemRef { item_ref: String },
    #[error("split `{split_kind}` must use item kind `{expected_item_kind}`")]
    SplitKindItemKindMismatch {
        split_kind: String,
        expected_item_kind: String,
    },
    #[error("train and held-out trace groups drifted from the parent-lineage rows")]
    TraceDisjointGroupDrift,
    #[error("trace disjointness is broken: {detail}")]
    TraceDisjointnessBroken { detail: String },
    #[error(transparent)]
    TrainingDerivation(#[from] PsionPluginTrainingDerivationError),
    #[error(transparent)]
    ConditionedDataset(#[from] PsionPluginConditionedDatasetError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn psion_plugin_contamination_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_CONTAMINATION_BUNDLE_REF)
}

pub fn build_psion_plugin_contamination_bundle()
-> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    let derivation = build_psion_plugin_training_derivation_bundle()?;
    let dataset = build_psion_plugin_conditioned_dataset_bundle_from_derivation(&derivation)?;
    build_psion_plugin_contamination_bundle_from_inputs(&derivation, &dataset)
}

pub fn write_psion_plugin_contamination_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    let bundle = build_psion_plugin_contamination_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_contamination_bundle_from_inputs(
    derivation: &PsionPluginTrainingDerivationBundle,
    dataset: &PsionPluginConditionedDatasetBundle,
) -> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    build_psion_plugin_contamination_bundle_from_inputs_with_ref(
        derivation,
        dataset,
        crate::PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_REF,
        "this bundle projects the committed plugin-conditioned derivation and dataset artifacts into one smaller contamination-review surface. It preserves parent training-record lineage, plugin-trace source identity, and plugin runtime receipt ancestry for the bounded train and held-out splits. It does not claim broader benchmark-family closure, mixed guest-artifact coverage, or universal plugin contamination rules by itself.",
    )
}

#[must_use]
pub fn psion_plugin_mixed_contamination_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_MIXED_CONTAMINATION_BUNDLE_REF)
}

pub fn build_psion_plugin_mixed_contamination_bundle()
-> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    let derivation = build_psion_plugin_training_derivation_bundle()?;
    let dataset =
        crate::build_psion_plugin_mixed_conditioned_dataset_bundle_from_derivation(&derivation)?;
    build_psion_plugin_mixed_contamination_bundle_from_inputs(&derivation, &dataset)
}

pub fn write_psion_plugin_mixed_contamination_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    let bundle = build_psion_plugin_mixed_contamination_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_mixed_contamination_bundle_from_inputs(
    derivation: &PsionPluginTrainingDerivationBundle,
    dataset: &PsionPluginConditionedDatasetBundle,
) -> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    build_psion_plugin_contamination_bundle_from_inputs_with_ref(
        derivation,
        dataset,
        crate::PSION_PLUGIN_MIXED_CONDITIONED_DATASET_BUNDLE_REF,
        "this bundle projects the committed plugin-conditioned derivation and the first mixed dataset artifact into one smaller contamination-review surface. It preserves the added guest-artifact train lineage, keeps the held-out split host-native, and records the plugin-trace plus plugin-receipt exclusions the mixed training lane must preserve. It does not claim held-out guest-artifact benchmarks, publication widening, or universal plugin contamination rules by itself.",
    )
}

fn build_psion_plugin_contamination_bundle_from_inputs_with_ref(
    derivation: &PsionPluginTrainingDerivationBundle,
    dataset: &PsionPluginConditionedDatasetBundle,
    dataset_bundle_ref: &str,
    claim_boundary: &str,
) -> Result<PsionPluginContaminationBundle, PsionPluginContaminationError> {
    let record_map = derivation
        .records
        .iter()
        .map(|record| (record.record_id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let mut parent_lineage_rows = Vec::new();
    for split in &dataset.split_rows {
        for record in &split.records {
            let Some(parent_record) = record_map.get(record.record_id.as_str()) else {
                return Err(PsionPluginContaminationError::FieldMismatch {
                    field: format!(
                        "dataset.split_rows.{:?}.records[].record_id",
                        split.split_kind
                    ),
                    expected: String::from("record present in derivation bundle"),
                    actual: format!("missing `{}`", record.record_id),
                });
            };
            if parent_record.record_digest != record.record_digest {
                return Err(PsionPluginContaminationError::FieldMismatch {
                    field: format!(
                        "dataset.split_rows.{:?}.records[].record_digest",
                        split.split_kind
                    ),
                    expected: parent_record.record_digest.clone(),
                    actual: record.record_digest.clone(),
                });
            }
            let workflow_case_id = record
                .controller_context
                .workflow_case_id
                .clone()
                .ok_or_else(|| PsionPluginContaminationError::MissingField {
                    field: format!("lineage_row.workflow_case_id.{}", record.record_id),
                })?;
            let item_kind = match split.split_kind {
                DatasetSplitKind::Train => PsionPluginContaminationItemKind::SftTrainRecord,
                DatasetSplitKind::HeldOut => PsionPluginContaminationItemKind::HeldOutEvalRecord,
                _ => {
                    return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
                        detail: format!(
                            "unexpected split kind `{}` in plugin-conditioned dataset",
                            split_kind_key(split.split_kind)
                        ),
                    });
                }
            };
            parent_lineage_rows.push(PsionPluginParentLineageRow {
                lineage_id: format!("{}.{}", split_kind_key(split.split_kind), record.record_id),
                item_ref: format!(
                    "{}#{}:{}",
                    dataset.stable_dataset_identity,
                    split_kind_key(split.split_kind),
                    record.record_id
                ),
                item_kind,
                split_kind: split.split_kind,
                training_record_id: record.record_id.clone(),
                training_record_digest: record.record_digest.clone(),
                workflow_case_id,
                controller_surface: record.controller_context.controller_surface,
                source_trace: PsionPluginTraceSourceRef {
                    source_bundle_ref: record.controller_context.source_bundle_ref.clone(),
                    source_bundle_id: record.controller_context.source_bundle_id.clone(),
                    source_bundle_digest: record.controller_context.source_bundle_digest.clone(),
                    source_case_id: record.controller_context.source_case_id.clone(),
                },
                receipt_refs: record
                    .plugin_invocations
                    .iter()
                    .map(|invocation| invocation.receipt_ref.clone())
                    .collect(),
                receipt_digests: record
                    .plugin_invocations
                    .iter()
                    .map(|invocation| invocation.receipt_digest.clone())
                    .collect(),
            });
        }
    }
    parent_lineage_rows.sort_by(|left, right| left.item_ref.cmp(&right.item_ref));

    let train_rows = parent_lineage_rows
        .iter()
        .filter(|row| row.split_kind == DatasetSplitKind::Train)
        .collect::<Vec<_>>();
    let held_out_rows = parent_lineage_rows
        .iter()
        .filter(|row| row.split_kind == DatasetSplitKind::HeldOut)
        .collect::<Vec<_>>();
    let trace_disjoint_groups =
        expected_trace_disjoint_groups(&parent_lineage_rows.iter().collect::<Vec<_>>());
    let exclusion_manifest = PsionPluginExclusionManifest {
        schema_version: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION),
        source_derivation_bundle_ref: String::from(
            crate::PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_REF,
        ),
        source_derivation_bundle_digest: derivation.bundle_digest.clone(),
        dataset_identity: dataset.stable_dataset_identity.clone(),
        training_excluded_trace_sources: unique_trace_sources(&held_out_rows),
        benchmark_excluded_trace_sources: unique_trace_sources(&train_rows),
        training_excluded_receipt_refs: unique_receipt_refs(&held_out_rows),
        benchmark_excluded_receipt_refs: unique_receipt_refs(&train_rows),
    };

    let train_count = train_rows.len();
    let held_out_count = held_out_rows.len();
    let mut bundle = PsionPluginContaminationBundle {
        schema_version: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION),
        source_derivation_bundle_ref: String::from(
            crate::PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_REF,
        ),
        source_derivation_bundle_digest: derivation.bundle_digest.clone(),
        dataset_bundle_ref: String::from(dataset_bundle_ref),
        dataset_bundle_digest: dataset.bundle_digest.clone(),
        dataset_identity: dataset.stable_dataset_identity.clone(),
        parent_lineage_rows,
        trace_disjoint_groups,
        exclusion_manifest,
        claim_boundary: String::from(claim_boundary),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "plugin contamination bundle keeps parent_lineage_rows={} with train_rows={} held_out_rows={} and trace_disjoint_groups={}.",
        bundle.parent_lineage_rows.len(),
        train_count,
        held_out_count,
        bundle.trace_disjoint_groups.len(),
    );
    bundle.bundle_digest = stable_digest(b"psionic_psion_plugin_contamination_bundle|", &bundle);
    bundle.validate()?;
    Ok(bundle)
}

fn validate_lineage_row(
    row: &PsionPluginParentLineageRow,
) -> Result<(), PsionPluginContaminationError> {
    ensure_nonempty(row.lineage_id.as_str(), "lineage_row.lineage_id")?;
    ensure_nonempty(row.item_ref.as_str(), "lineage_row.item_ref")?;
    ensure_nonempty(
        row.training_record_id.as_str(),
        "lineage_row.training_record_id",
    )?;
    ensure_nonempty(
        row.training_record_digest.as_str(),
        "lineage_row.training_record_digest",
    )?;
    ensure_nonempty(
        row.workflow_case_id.as_str(),
        "lineage_row.workflow_case_id",
    )?;
    ensure_nonempty(
        row.source_trace.source_bundle_ref.as_str(),
        "lineage_row.source_trace.source_bundle_ref",
    )?;
    ensure_nonempty(
        row.source_trace.source_bundle_id.as_str(),
        "lineage_row.source_trace.source_bundle_id",
    )?;
    ensure_nonempty(
        row.source_trace.source_bundle_digest.as_str(),
        "lineage_row.source_trace.source_bundle_digest",
    )?;
    ensure_nonempty(
        row.source_trace.source_case_id.as_str(),
        "lineage_row.source_trace.source_case_id",
    )?;
    let mut seen_receipts = BTreeSet::new();
    if row.receipt_refs.len() != row.receipt_digests.len() {
        return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
            detail: format!(
                "receipt ref and digest counts drift for `{}`",
                row.lineage_id
            ),
        });
    }
    for (receipt_ref, receipt_digest) in row.receipt_refs.iter().zip(&row.receipt_digests) {
        ensure_nonempty(receipt_ref.as_str(), "lineage_row.receipt_refs[]")?;
        ensure_nonempty(receipt_digest.as_str(), "lineage_row.receipt_digests[]")?;
        if !seen_receipts.insert(receipt_ref.as_str()) {
            return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
                detail: format!(
                    "duplicate receipt ref `{receipt_ref}` on `{}`",
                    row.lineage_id
                ),
            });
        }
    }
    Ok(())
}

fn ensure_trace_disjoint(
    train_rows: &[&PsionPluginParentLineageRow],
    held_out_rows: &[&PsionPluginParentLineageRow],
) -> Result<(), PsionPluginContaminationError> {
    let train_workflows = train_rows
        .iter()
        .map(|row| row.workflow_case_id.as_str())
        .collect::<BTreeSet<_>>();
    let held_out_workflows = held_out_rows
        .iter()
        .map(|row| row.workflow_case_id.as_str())
        .collect::<BTreeSet<_>>();
    if !train_workflows.is_disjoint(&held_out_workflows) {
        return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
            detail: String::from("train and held-out workflow case ids overlap"),
        });
    }

    let train_sources = train_rows
        .iter()
        .map(|row| row.source_trace.source_case_id.as_str())
        .collect::<BTreeSet<_>>();
    let held_out_sources = held_out_rows
        .iter()
        .map(|row| row.source_trace.source_case_id.as_str())
        .collect::<BTreeSet<_>>();
    if !train_sources.is_disjoint(&held_out_sources) {
        return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
            detail: String::from("train and held-out source case ids overlap"),
        });
    }

    let train_receipts = train_rows
        .iter()
        .flat_map(|row| row.receipt_refs.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let held_out_receipts = held_out_rows
        .iter()
        .flat_map(|row| row.receipt_refs.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    if !train_receipts.is_disjoint(&held_out_receipts) {
        return Err(PsionPluginContaminationError::TraceDisjointnessBroken {
            detail: String::from("train and held-out receipt refs overlap"),
        });
    }
    Ok(())
}

fn expected_trace_disjoint_groups(
    rows: &[&PsionPluginParentLineageRow],
) -> Vec<PsionPluginTraceDisjointGroup> {
    let mut groups = [DatasetSplitKind::HeldOut, DatasetSplitKind::Train]
        .into_iter()
        .filter_map(|split_kind| {
            let split_rows = rows
                .iter()
                .copied()
                .filter(|row| row.split_kind == split_kind)
                .collect::<Vec<_>>();
            if split_rows.is_empty() {
                return None;
            }
            Some(PsionPluginTraceDisjointGroup {
                split_kind,
                workflow_case_ids: split_rows
                    .iter()
                    .map(|row| row.workflow_case_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                source_case_ids: split_rows
                    .iter()
                    .map(|row| row.source_trace.source_case_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                receipt_refs: split_rows
                    .iter()
                    .flat_map(|row| row.receipt_refs.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                detail: format!(
                    "{} plugin-conditioned rows stay disjoint from the opposite split by workflow case id, source case id, and plugin receipt ref.",
                    split_kind_key(split_kind)
                ),
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        split_kind_key(left.split_kind).cmp(split_kind_key(right.split_kind))
    });
    groups
}

fn unique_trace_sources(rows: &[&PsionPluginParentLineageRow]) -> Vec<PsionPluginTraceSourceRef> {
    rows.iter()
        .map(|row| row.source_trace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unique_receipt_refs(rows: &[&PsionPluginParentLineageRow]) -> Vec<String> {
    rows.iter()
        .flat_map(|row| row.receipt_refs.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginContaminationError> {
    if value.trim().is_empty() {
        return Err(PsionPluginContaminationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn split_kind_key(split_kind: DatasetSplitKind) -> &'static str {
    match split_kind {
        DatasetSplitKind::Train => "train",
        DatasetSplitKind::Validation => "validation",
        DatasetSplitKind::Test => "test",
        DatasetSplitKind::HeldOut => "held_out",
        DatasetSplitKind::Preference => "preference",
        DatasetSplitKind::Replay => "replay",
        DatasetSplitKind::Custom => "custom",
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-data crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("contamination bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION, PsionPluginContaminationError,
        build_psion_plugin_contamination_bundle, build_psion_plugin_mixed_contamination_bundle,
    };
    use crate::{DatasetSplitKind, PSION_PLUGIN_MIXED_CONDITIONED_DATASET_BUNDLE_REF};

    #[test]
    fn contamination_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_contamination_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_CONTAMINATION_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.trace_disjoint_groups.len(), 2);
        assert!(!bundle.parent_lineage_rows.is_empty());
        assert!(!bundle.bundle_digest.is_empty());
        Ok(())
    }

    #[test]
    fn mixed_contamination_bundle_keeps_guest_lineage_in_train_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_mixed_contamination_bundle()?;
        assert_eq!(
            bundle.dataset_bundle_ref,
            PSION_PLUGIN_MIXED_CONDITIONED_DATASET_BUNDLE_REF
        );
        let guest_rows = bundle
            .parent_lineage_rows
            .iter()
            .filter(|row| {
                row.receipt_refs
                    .iter()
                    .any(|receipt_ref| receipt_ref.contains("plugin.example.echo_guest"))
            })
            .collect::<Vec<_>>();
        assert_eq!(guest_rows.len(), 1);
        assert!(
            guest_rows
                .iter()
                .all(|row| row.split_kind == DatasetSplitKind::Train)
        );
        assert!(
            bundle
                .exclusion_manifest
                .benchmark_excluded_receipt_refs
                .iter()
                .any(|receipt_ref| receipt_ref.contains("plugin.example.echo_guest"))
        );
        Ok(())
    }

    #[test]
    fn contamination_bundle_rejects_overlapping_source_case_sets()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut bundle = build_psion_plugin_contamination_bundle()?;
        let train_source_case_id = bundle
            .parent_lineage_rows
            .iter()
            .find(|row| row.split_kind == DatasetSplitKind::Train)
            .expect("train row should exist")
            .source_trace
            .source_case_id
            .clone();
        for row in &mut bundle.parent_lineage_rows {
            if row.split_kind == DatasetSplitKind::HeldOut {
                row.source_trace.source_case_id = train_source_case_id.clone();
            }
        }
        bundle.trace_disjoint_groups = super::expected_trace_disjoint_groups(
            &bundle.parent_lineage_rows.iter().collect::<Vec<_>>(),
        );
        bundle.exclusion_manifest.training_excluded_trace_sources = bundle
            .parent_lineage_rows
            .iter()
            .filter(|row| row.split_kind == DatasetSplitKind::HeldOut)
            .map(|row| row.source_trace.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let error = bundle
            .validate()
            .expect_err("overlapping trace source ids should fail");
        assert!(matches!(
            error,
            PsionPluginContaminationError::TraceDisjointnessBroken { .. }
        ));
        Ok(())
    }
}
