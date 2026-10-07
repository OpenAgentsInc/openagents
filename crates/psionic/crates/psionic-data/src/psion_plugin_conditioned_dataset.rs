use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetKey, DatasetSplitKind, PsionPluginClass, PsionPluginControllerSurface,
    PsionPluginOutcomeLabel, PsionPluginRouteLabel, PsionPluginTrainingDerivationBundle,
    PsionPluginTrainingDerivationError, PsionPluginTrainingRecord,
    build_psion_plugin_training_derivation_bundle,
};

/// Stable schema version for the first plugin-conditioned dataset bundle.
pub const PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_conditioned_dataset_bundle.v1";
/// Stable dataset ref for the first host-native plugin-conditioned dataset bundle.
pub const PSION_PLUGIN_CONDITIONED_DATASET_REF: &str =
    "dataset://openagents/psion/plugin_conditioned_host_native_reference";
/// Stable committed output ref for the first dataset bundle.
pub const PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_REF: &str = "fixtures/psion/plugins/datasets/psion_plugin_conditioned_dataset_v1/psion_plugin_conditioned_dataset_bundle.json";
/// Stable dataset ref for the first mixed host-native plus guest-artifact dataset bundle.
pub const PSION_PLUGIN_MIXED_CONDITIONED_DATASET_REF: &str =
    "dataset://openagents/psion/plugin_conditioned_mixed_reference";
/// Stable committed output ref for the first mixed dataset bundle.
pub const PSION_PLUGIN_MIXED_CONDITIONED_DATASET_BUNDLE_REF: &str = "fixtures/psion/plugins/datasets/psion_plugin_mixed_conditioned_dataset_v1/psion_plugin_mixed_conditioned_dataset_bundle.json";
const TRAIN_WORKFLOW_CASE_ID: &str = "starter_plugin.web_content_success.v1";
const HELD_OUT_WORKFLOW_CASE_ID: &str = "starter_plugin.fetch_refusal.v1";
const EXCLUDED_GUEST_ARTIFACT_WORKFLOW_CASE_ID: &str = "starter_plugin.guest_artifact_success.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PsionPluginConditionedDatasetProfile {
    HostNativeReference,
    MixedHostNativeGuestArtifact,
}

impl PsionPluginConditionedDatasetProfile {
    fn dataset_key(self) -> DatasetKey {
        match self {
            Self::HostNativeReference => {
                DatasetKey::new(PSION_PLUGIN_CONDITIONED_DATASET_REF, "2026.03.22.v1")
            }
            Self::MixedHostNativeGuestArtifact => {
                DatasetKey::new(PSION_PLUGIN_MIXED_CONDITIONED_DATASET_REF, "2026.03.22.v1")
            }
        }
    }

    fn train_workflow_case_ids(self) -> &'static [&'static str] {
        match self {
            Self::HostNativeReference => &[TRAIN_WORKFLOW_CASE_ID],
            Self::MixedHostNativeGuestArtifact => &[
                TRAIN_WORKFLOW_CASE_ID,
                EXCLUDED_GUEST_ARTIFACT_WORKFLOW_CASE_ID,
            ],
        }
    }

    fn held_out_workflow_case_ids(self) -> &'static [&'static str] {
        match self {
            Self::HostNativeReference | Self::MixedHostNativeGuestArtifact => {
                &[HELD_OUT_WORKFLOW_CASE_ID]
            }
        }
    }

    fn ignored_workflow_case_ids(self) -> &'static [&'static str] {
        match self {
            Self::HostNativeReference => &[EXCLUDED_GUEST_ARTIFACT_WORKFLOW_CASE_ID],
            Self::MixedHostNativeGuestArtifact => &[],
        }
    }

    fn held_out_isolation_detail(self) -> &'static str {
        match self {
            Self::HostNativeReference => {
                "train and held-out splits stay disjoint by normalized workflow case id across all controller surfaces while the out-of-scope guest-artifact workflow remains excluded from this host-native v1 dataset."
            }
            Self::MixedHostNativeGuestArtifact => {
                "train and held-out splits stay disjoint by normalized workflow case id across all controller surfaces while the single admitted guest-artifact workflow remains confined to the train split and the held-out split stays host-native for the first mixed v1 review surface."
            }
        }
    }

    fn claim_boundary(self) -> &'static str {
        match self {
            Self::HostNativeReference => {
                "this bundle freezes one small host-native plugin-conditioned dataset built from the committed derivation bundle with workflow-case-disjoint train and held-out splits. It preserves controller-surface, plugin-class, route-label, and outcome-label counts instead of collapsing the records into anonymous prompt/response rows. It does not claim scale, generalization, or mixed guest-artifact coverage by itself."
            }
            Self::MixedHostNativeGuestArtifact => {
                "this bundle freezes the first mixed plugin-conditioned dataset built from the committed derivation bundle with workflow-case-disjoint train and held-out splits. It preserves host-native and bounded guest-artifact class membership, controller-surface provenance, route labels, and outcome labels instead of collapsing the records into anonymous prompt/response rows. It does not claim publication widening, benchmark-family closure, or broad guest-artifact generalization by itself."
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSplitStats {
    /// Split kind represented by the stats.
    pub split_kind: DatasetSplitKind,
    /// Number of records in the split.
    pub record_count: u32,
    /// Workflow cases represented in the split.
    pub workflow_case_ids: Vec<String>,
    /// Controller-surface counts preserved in the split.
    pub controller_surface_counts: BTreeMap<PsionPluginControllerSurface, u32>,
    /// Plugin-class counts preserved in the split.
    pub plugin_class_counts: BTreeMap<PsionPluginClass, u32>,
    /// Route-label counts preserved in the split.
    pub route_label_counts: BTreeMap<PsionPluginRouteLabel, u32>,
    /// Outcome-label counts preserved in the split.
    pub outcome_label_counts: BTreeMap<PsionPluginOutcomeLabel, u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSplit {
    /// Split kind for the record group.
    pub split_kind: DatasetSplitKind,
    /// Stable records assigned to the split.
    pub records: Vec<PsionPluginTrainingRecord>,
    /// Summary stats preserved for the split.
    pub stats: PsionPluginConditionedSplitStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHeldOutIsolationContract {
    /// Stable policy identifier.
    pub policy_id: String,
    /// Whether train and held-out splits are disjoint by workflow case id.
    pub workflow_case_disjoint: bool,
    /// Train workflow case ids.
    pub train_workflow_case_ids: Vec<String>,
    /// Held-out workflow case ids.
    pub held_out_workflow_case_ids: Vec<String>,
    /// Short explanation of the isolation policy.
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginConditionedDatasetBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable dataset key.
    pub dataset_key: DatasetKey,
    /// Stable dataset identity string.
    pub stable_dataset_identity: String,
    /// Source derivation bundle ref.
    pub source_derivation_bundle_ref: String,
    /// Source derivation bundle digest.
    pub source_derivation_bundle_digest: String,
    /// Held-out isolation contract.
    pub held_out_isolation: PsionPluginHeldOutIsolationContract,
    /// Split rows for the dataset.
    pub split_rows: Vec<PsionPluginConditionedSplit>,
    /// Plain-language claim boundary for the bundle.
    pub claim_boundary: String,
    /// Short machine-readable summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPluginConditionedDatasetBundle {
    /// Writes the dataset bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginConditionedDatasetError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginConditionedDatasetError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginConditionedDatasetError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the dataset bundle.
    pub fn validate(&self) -> Result<(), PsionPluginConditionedDatasetError> {
        if self.schema_version != PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION {
            return Err(PsionPluginConditionedDatasetError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        if self.split_rows.is_empty() {
            return Err(PsionPluginConditionedDatasetError::MissingField {
                field: String::from("dataset_bundle.split_rows"),
            });
        }
        let mut seen_record_ids = BTreeSet::new();
        let mut seen_split_kinds = BTreeSet::new();
        let mut train_workflow_case_ids = None;
        let mut held_out_workflow_case_ids = None;
        for split in &self.split_rows {
            if !seen_split_kinds.insert(split_kind_key(split.split_kind)) {
                return Err(PsionPluginConditionedDatasetError::DuplicateSplitKind {
                    split_kind: format!("{:?}", split.split_kind),
                });
            }
            if split.records.is_empty() {
                return Err(PsionPluginConditionedDatasetError::MissingField {
                    field: format!("dataset_bundle.split_rows[{:#?}].records", split.split_kind),
                });
            }
            for record in &split.records {
                record.validate()?;
                if !seen_record_ids.insert(record.record_id.as_str()) {
                    return Err(PsionPluginConditionedDatasetError::DuplicateRecordId {
                        record_id: record.record_id.clone(),
                    });
                }
            }
            let actual_stats = build_split_stats(split.split_kind, split.records.clone())?;
            if split.stats != actual_stats {
                return Err(PsionPluginConditionedDatasetError::SplitStatsMismatch {
                    split_kind: format!("{:?}", split.split_kind),
                });
            }
            match split.split_kind {
                DatasetSplitKind::Train => {
                    train_workflow_case_ids = Some(split.stats.workflow_case_ids.clone());
                }
                DatasetSplitKind::HeldOut => {
                    held_out_workflow_case_ids = Some(split.stats.workflow_case_ids.clone());
                }
                _ => {}
            }
        }
        let train_workflow_case_ids = train_workflow_case_ids.ok_or_else(|| {
            PsionPluginConditionedDatasetError::MissingField {
                field: String::from("dataset_bundle.train_split"),
            }
        })?;
        let held_out_workflow_case_ids = held_out_workflow_case_ids.ok_or_else(|| {
            PsionPluginConditionedDatasetError::MissingField {
                field: String::from("dataset_bundle.held_out_split"),
            }
        })?;
        let train_set = train_workflow_case_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let held_out_set = held_out_workflow_case_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !train_set.is_disjoint(&held_out_set) {
            return Err(PsionPluginConditionedDatasetError::HeldOutIsolationBroken {
                detail: String::from("train and held-out workflow case ids overlap"),
            });
        }
        if !self.held_out_isolation.workflow_case_disjoint {
            return Err(PsionPluginConditionedDatasetError::HeldOutIsolationBroken {
                detail: String::from("workflow_case_disjoint must stay true"),
            });
        }
        if self.held_out_isolation.train_workflow_case_ids != train_workflow_case_ids {
            return Err(PsionPluginConditionedDatasetError::HeldOutIsolationBroken {
                detail: String::from("train workflow case ids drift from split stats"),
            });
        }
        if self.held_out_isolation.held_out_workflow_case_ids != held_out_workflow_case_ids {
            return Err(PsionPluginConditionedDatasetError::HeldOutIsolationBroken {
                detail: String::from("held-out workflow case ids drift from split stats"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginConditionedDatasetError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("duplicate split kind `{split_kind}`")]
    DuplicateSplitKind { split_kind: String },
    #[error("duplicate record id `{record_id}`")]
    DuplicateRecordId { record_id: String },
    #[error("split stats drift for `{split_kind}`")]
    SplitStatsMismatch { split_kind: String },
    #[error("held-out isolation is broken: {detail}")]
    HeldOutIsolationBroken { detail: String },
    #[error(transparent)]
    Derivation(#[from] PsionPluginTrainingDerivationError),
    #[error(transparent)]
    TrainingRecord(#[from] crate::PsionPluginTrainingRecordError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn psion_plugin_conditioned_dataset_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_REF)
}

pub fn build_psion_plugin_conditioned_dataset_bundle()
-> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    let derivation = build_psion_plugin_training_derivation_bundle()?;
    build_psion_plugin_conditioned_dataset_bundle_from_derivation(&derivation)
}

pub fn write_psion_plugin_conditioned_dataset_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    let bundle = build_psion_plugin_conditioned_dataset_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_conditioned_dataset_bundle_from_derivation(
    derivation: &PsionPluginTrainingDerivationBundle,
) -> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    build_psion_plugin_conditioned_dataset_bundle_for_profile(
        derivation,
        PsionPluginConditionedDatasetProfile::HostNativeReference,
    )
}

#[must_use]
pub fn psion_plugin_mixed_conditioned_dataset_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_MIXED_CONDITIONED_DATASET_BUNDLE_REF)
}

pub fn build_psion_plugin_mixed_conditioned_dataset_bundle()
-> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    let derivation = build_psion_plugin_training_derivation_bundle()?;
    build_psion_plugin_mixed_conditioned_dataset_bundle_from_derivation(&derivation)
}

pub fn write_psion_plugin_mixed_conditioned_dataset_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    let bundle = build_psion_plugin_mixed_conditioned_dataset_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_mixed_conditioned_dataset_bundle_from_derivation(
    derivation: &PsionPluginTrainingDerivationBundle,
) -> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    build_psion_plugin_conditioned_dataset_bundle_for_profile(
        derivation,
        PsionPluginConditionedDatasetProfile::MixedHostNativeGuestArtifact,
    )
}

fn build_psion_plugin_conditioned_dataset_bundle_for_profile(
    derivation: &PsionPluginTrainingDerivationBundle,
    profile: PsionPluginConditionedDatasetProfile,
) -> Result<PsionPluginConditionedDatasetBundle, PsionPluginConditionedDatasetError> {
    let mut train_records = Vec::new();
    let mut held_out_records = Vec::new();
    for record in &derivation.records {
        let workflow_case_id = record
            .controller_context
            .workflow_case_id
            .as_deref()
            .ok_or_else(|| PsionPluginConditionedDatasetError::MissingField {
                field: format!(
                    "dataset_bundle.record_workflow_case_id.{}",
                    record.record_id
                ),
            })?;
        if profile
            .train_workflow_case_ids()
            .contains(&workflow_case_id)
        {
            train_records.push(record.clone());
            continue;
        }
        if profile
            .held_out_workflow_case_ids()
            .contains(&workflow_case_id)
        {
            held_out_records.push(record.clone());
            continue;
        }
        if profile
            .ignored_workflow_case_ids()
            .contains(&workflow_case_id)
        {
            continue;
        }
        return Err(PsionPluginConditionedDatasetError::HeldOutIsolationBroken {
            detail: format!("unexpected workflow case id `{workflow_case_id}`"),
        });
    }
    let train_split = PsionPluginConditionedSplit {
        split_kind: DatasetSplitKind::Train,
        stats: build_split_stats(DatasetSplitKind::Train, train_records.clone())?,
        records: train_records,
    };
    let held_out_split = PsionPluginConditionedSplit {
        split_kind: DatasetSplitKind::HeldOut,
        stats: build_split_stats(DatasetSplitKind::HeldOut, held_out_records.clone())?,
        records: held_out_records,
    };
    let dataset_key = profile.dataset_key();
    let stable_dataset_identity = dataset_key.storage_key();
    let mut bundle = PsionPluginConditionedDatasetBundle {
        schema_version: String::from(PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION),
        dataset_key,
        stable_dataset_identity,
        source_derivation_bundle_ref: String::from(
            crate::PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_REF,
        ),
        source_derivation_bundle_digest: derivation.bundle_digest.clone(),
        held_out_isolation: PsionPluginHeldOutIsolationContract {
            policy_id: String::from("workflow_case_disjoint.v1"),
            workflow_case_disjoint: true,
            train_workflow_case_ids: train_split.stats.workflow_case_ids.clone(),
            held_out_workflow_case_ids: held_out_split.stats.workflow_case_ids.clone(),
            detail: String::from(profile.held_out_isolation_detail()),
        },
        split_rows: vec![train_split, held_out_split],
        claim_boundary: String::from(profile.claim_boundary()),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    let train_guest_count = bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == DatasetSplitKind::Train)
        .and_then(|split| {
            split
                .stats
                .plugin_class_counts
                .get(&PsionPluginClass::GuestArtifactDigestBound)
                .copied()
        })
        .unwrap_or(0);
    let held_out_guest_count = bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == DatasetSplitKind::HeldOut)
        .and_then(|split| {
            split
                .stats
                .plugin_class_counts
                .get(&PsionPluginClass::GuestArtifactDigestBound)
                .copied()
        })
        .unwrap_or(0);
    bundle.summary = format!(
        "plugin-conditioned dataset bundle keeps train_records={} held_out_records={} train_guest_artifact_records={} held_out_guest_artifact_records={} under workflow_case_disjoint=true.",
        bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == DatasetSplitKind::Train)
            .map(|split| split.records.len())
            .unwrap_or(0),
        bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == DatasetSplitKind::HeldOut)
            .map(|split| split.records.len())
            .unwrap_or(0),
        train_guest_count,
        held_out_guest_count,
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_psion_plugin_conditioned_dataset_bundle|", &bundle);
    bundle.validate()?;
    Ok(bundle)
}

fn build_split_stats(
    split_kind: DatasetSplitKind,
    records: Vec<PsionPluginTrainingRecord>,
) -> Result<PsionPluginConditionedSplitStats, PsionPluginConditionedDatasetError> {
    let mut workflow_case_ids = BTreeSet::new();
    let mut controller_surface_counts = BTreeMap::new();
    let mut plugin_class_counts = BTreeMap::new();
    let mut route_label_counts = BTreeMap::new();
    let mut outcome_label_counts = BTreeMap::new();
    for record in &records {
        let workflow_case_id = record
            .controller_context
            .workflow_case_id
            .as_deref()
            .ok_or_else(|| PsionPluginConditionedDatasetError::MissingField {
                field: format!("split_stats.workflow_case_id.{}", record.record_id),
            })?;
        workflow_case_ids.insert(String::from(workflow_case_id));
        *controller_surface_counts
            .entry(record.controller_context.controller_surface)
            .or_insert(0_u32) += 1;
        *route_label_counts
            .entry(record.route_label)
            .or_insert(0_u32) += 1;
        *outcome_label_counts
            .entry(record.outcome_label)
            .or_insert(0_u32) += 1;
        for plugin in &record.admitted_plugins {
            *plugin_class_counts
                .entry(plugin.plugin_class)
                .or_insert(0_u32) += 1;
        }
    }
    Ok(PsionPluginConditionedSplitStats {
        split_kind,
        record_count: records.len() as u32,
        workflow_case_ids: workflow_case_ids.into_iter().collect(),
        controller_surface_counts,
        plugin_class_counts,
        route_label_counts,
        outcome_label_counts,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-data crate dir")
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("dataset bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION,
        PSION_PLUGIN_MIXED_CONDITIONED_DATASET_REF, PsionPluginConditionedDatasetError,
        build_psion_plugin_conditioned_dataset_bundle,
        build_psion_plugin_mixed_conditioned_dataset_bundle,
    };
    use crate::PsionPluginClass;

    #[test]
    fn dataset_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_conditioned_dataset_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_CONDITIONED_DATASET_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.split_rows.len(), 2);
        assert!(bundle.held_out_isolation.workflow_case_disjoint);
        assert!(bundle.split_rows.iter().all(|split| {
            !split
                .stats
                .plugin_class_counts
                .contains_key(&PsionPluginClass::GuestArtifactDigestBound)
        }));
        assert!(!bundle.bundle_digest.is_empty());
        Ok(())
    }

    #[test]
    fn mixed_dataset_bundle_builds_and_reports_guest_class_balance()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_mixed_conditioned_dataset_bundle()?;
        assert_eq!(
            bundle.dataset_key.dataset_ref,
            PSION_PLUGIN_MIXED_CONDITIONED_DATASET_REF
        );
        let train_split = bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == crate::DatasetSplitKind::Train)
            .expect("train split should exist");
        let held_out_split = bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == crate::DatasetSplitKind::HeldOut)
            .expect("held-out split should exist");
        assert_eq!(
            train_split
                .stats
                .plugin_class_counts
                .get(&PsionPluginClass::GuestArtifactDigestBound)
                .copied(),
            Some(1)
        );
        assert!(
            !held_out_split
                .stats
                .plugin_class_counts
                .contains_key(&PsionPluginClass::GuestArtifactDigestBound)
        );
        assert!(bundle.summary.contains("train_guest_artifact_records=1"));
        Ok(())
    }

    #[test]
    fn dataset_bundle_rejects_overlapping_workflow_cases() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut bundle = build_psion_plugin_conditioned_dataset_bundle()?;
        let held_out_split = bundle
            .split_rows
            .iter_mut()
            .find(|split| split.split_kind == crate::DatasetSplitKind::HeldOut)
            .expect("held-out split should exist");
        assert!(!held_out_split.records.is_empty());
        for held_out_record in &mut held_out_split.records {
            held_out_record.controller_context.workflow_case_id =
                Some(String::from("starter_plugin.web_content_success.v1"));
        }
        held_out_split.stats.workflow_case_ids =
            vec![String::from("starter_plugin.web_content_success.v1")];
        bundle.held_out_isolation.held_out_workflow_case_ids =
            vec![String::from("starter_plugin.web_content_success.v1")];
        let error = bundle
            .validate()
            .expect_err("overlapping workflow cases should fail");
        assert!(matches!(
            error,
            PsionPluginConditionedDatasetError::HeldOutIsolationBroken { .. }
        ));
        Ok(())
    }
}
