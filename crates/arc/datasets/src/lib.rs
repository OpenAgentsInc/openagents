#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC dataset loading and split metadata export.
//!
//! `arc-datasets` owns ARC-specific dataset file loading and split bookkeeping.
//! It can export split metadata through Psionic-owned dataset contracts, but it
//! must not absorb generic dataset infrastructure from `psionic-data`.

use std::fs;
use std::path::{Path, PathBuf};

use arc_core::{ArcTask, ArcTaskError, ArcTaskId, ArcTaskIdError, canonical_sha256_hex};
use psionic_data::{
    DatasetContractError, DatasetKey, DatasetManifest, DatasetRecordEncoding, DatasetShardManifest,
    DatasetSplitDeclaration, DatasetSplitKind, TokenizerDigest, TokenizerFamily,
};
use psionic_datastream::{
    DatastreamDatasetBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

/// Human-readable ownership summary for this crate.
pub const CRATE_ROLE: &str = "ARC-AGI dataset loading, split metadata, and Psionic manifest export";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcDatasetFamily {
    ArcAgi1,
    ArcAgi2,
}

impl ArcDatasetFamily {
    #[must_use]
    pub const fn dataset_ref(self) -> &'static str {
        match self {
            Self::ArcAgi1 => "dataset://arc/agi1",
            Self::ArcAgi2 => "dataset://arc/agi2",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ArcAgi1 => "ARC-AGI-1",
            Self::ArcAgi2 => "ARC-AGI-2",
        }
    }

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ArcAgi1 => "arc_agi1",
            Self::ArcAgi2 => "arc_agi2",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcDatasetSplit {
    Train,
    Evaluation,
}

impl ArcDatasetSplit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Evaluation => "evaluation",
        }
    }

    #[must_use]
    pub const fn psionic_kind(self) -> DatasetSplitKind {
        match self {
            Self::Train => DatasetSplitKind::Train,
            Self::Evaluation => DatasetSplitKind::HeldOut,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcLoadedTask {
    pub source_path: PathBuf,
    pub task: ArcTask,
    pub source_bytes: u64,
    pub sequence_tokens: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcDatasetSplitBundle {
    pub family: ArcDatasetFamily,
    pub split: ArcDatasetSplit,
    pub tasks: Vec<ArcLoadedTask>,
}

impl ArcDatasetSplitBundle {
    #[must_use]
    pub fn task_count(&self) -> u64 {
        self.tasks.len() as u64
    }

    #[must_use]
    pub fn total_sequence_tokens(&self) -> u64 {
        self.tasks
            .iter()
            .map(|task| u64::from(task.sequence_tokens))
            .sum()
    }

    #[must_use]
    pub fn total_source_bytes(&self) -> u64 {
        self.tasks.iter().map(|task| task.source_bytes).sum()
    }

    fn sequence_bounds(&self) -> (u32, u32) {
        let min_tokens = self
            .tasks
            .iter()
            .map(|task| task.sequence_tokens)
            .min()
            .unwrap_or(1);
        let max_tokens = self
            .tasks
            .iter()
            .map(|task| task.sequence_tokens)
            .max()
            .unwrap_or(1);
        (min_tokens, max_tokens)
    }

    fn stable_digest(&self) -> Result<String, ArcDatasetError> {
        let task_digests = self
            .tasks
            .iter()
            .map(|task| task.task.contract_digest())
            .collect::<Result<Vec<_>, _>>()?;
        canonical_sha256_hex(&task_digests).map_err(Into::into)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcDatasetCollection {
    pub family: ArcDatasetFamily,
    pub version: String,
    pub splits: Vec<ArcDatasetSplitBundle>,
}

impl ArcDatasetCollection {
    pub fn to_psionic_manifest(&self) -> Result<DatasetManifest, ArcDatasetError> {
        let dataset_key = DatasetKey::new(self.family.dataset_ref(), self.version.clone());
        let tokenizer = TokenizerDigest::new(TokenizerFamily::Custom, "arc-grid-json-v1", 10);
        let mut manifest = DatasetManifest::new(
            dataset_key.clone(),
            self.family.display_name(),
            DatasetRecordEncoding::JsonlText,
            tokenizer,
        );

        let split_declarations = self
            .splits
            .iter()
            .map(|split| split.to_psionic_split(&dataset_key))
            .collect::<Result<Vec<_>, ArcDatasetError>>()?;
        let provenance_digest = canonical_sha256_hex(
            &split_declarations
                .iter()
                .map(|split| (&split.split_name, split.sequence_count, split.token_count))
                .collect::<Vec<_>>(),
        )?;

        manifest = manifest
            .with_provenance_digest(provenance_digest)
            .with_splits(split_declarations);
        manifest.metadata.insert(
            String::from("arc_family"),
            Value::String(String::from(self.family.slug())),
        );
        manifest.metadata.insert(
            String::from("split_names"),
            Value::Array(
                self.splits
                    .iter()
                    .map(|split| Value::String(String::from(split.split.as_str())))
                    .collect(),
            ),
        );
        manifest.validate()?;
        Ok(manifest)
    }
}

impl ArcDatasetSplitBundle {
    fn to_psionic_split(
        &self,
        dataset_key: &DatasetKey,
    ) -> Result<DatasetSplitDeclaration, ArcDatasetError> {
        let (min_sequence_tokens, max_sequence_tokens) = self.sequence_bounds();
        let split_name = self.split.as_str();
        let shard_key = "tasks";
        let split_digest = self.stable_digest()?;
        let manifest_ref = DatastreamManifestRef {
            stream_id: format!(
                "arc://{}/{}/{}",
                self.family.slug(),
                split_name,
                dataset_key.version
            ),
            manifest_digest: split_digest.clone(),
            subject: DatastreamSubjectKind::TokenizedCorpus,
            object_digest: split_digest.clone(),
            total_bytes: self.total_source_bytes().max(1),
            chunk_count: 1,
            chunk_bytes: self.total_source_bytes().max(1) as usize,
            encoding: DatastreamEncoding::Jsonl,
            compression: None,
            provenance_digest: Some(split_digest),
            dataset_binding: Some(
                DatastreamDatasetBinding::new(dataset_key.storage_key())
                    .with_split(split_name)
                    .with_shard_key(shard_key),
            ),
            checkpoint_binding: None,
            policy_weight_binding: None,
            mirrors: Vec::new(),
        };
        let shard = DatasetShardManifest::new(
            dataset_key,
            split_name,
            shard_key,
            manifest_ref,
            self.task_count(),
            self.total_sequence_tokens(),
            min_sequence_tokens,
            max_sequence_tokens,
        )?;
        DatasetSplitDeclaration::new(
            dataset_key,
            split_name,
            self.split.psionic_kind(),
            vec![shard],
        )
        .map_err(Into::into)
    }
}

pub fn load_split_from_dir(
    family: ArcDatasetFamily,
    split: ArcDatasetSplit,
    dir: impl AsRef<Path>,
) -> Result<ArcDatasetSplitBundle, ArcDatasetError> {
    let mut entries = fs::read_dir(dir.as_ref())?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    entries.sort();

    let tasks = entries
        .into_iter()
        .map(load_task_file)
        .collect::<Result<Vec<_>, ArcDatasetError>>()?;
    if tasks.is_empty() {
        return Err(ArcDatasetError::SplitHasNoTasks {
            split_name: String::from(split.as_str()),
        });
    }

    Ok(ArcDatasetSplitBundle {
        family,
        split,
        tasks,
    })
}

pub fn load_collection(
    family: ArcDatasetFamily,
    version: impl Into<String>,
    train_dir: impl AsRef<Path>,
    evaluation_dir: impl AsRef<Path>,
) -> Result<ArcDatasetCollection, ArcDatasetError> {
    Ok(ArcDatasetCollection {
        family,
        version: version.into(),
        splits: vec![
            load_split_from_dir(family, ArcDatasetSplit::Train, train_dir)?,
            load_split_from_dir(family, ArcDatasetSplit::Evaluation, evaluation_dir)?,
        ],
    })
}

fn load_task_file(path: PathBuf) -> Result<ArcLoadedTask, ArcDatasetError> {
    let raw = fs::read_to_string(&path)?;
    let source_bytes = raw.len() as u64;
    let parsed: RawArcTaskFile = serde_json::from_str(&raw)?;
    let task_id = match parsed.id {
        Some(task_id) => task_id,
        None => ArcTaskId::new(
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| ArcDatasetError::InvalidTaskFilename(path.clone()))?,
        )?,
    };
    let task = ArcTask::new(task_id, parsed.train, parsed.test)?;

    Ok(ArcLoadedTask {
        source_path: path,
        sequence_tokens: task_sequence_tokens(&task),
        source_bytes,
        task,
    })
}

fn task_sequence_tokens(task: &ArcTask) -> u32 {
    let train_tokens = task
        .train
        .iter()
        .map(|example| example.input.cell_count() + example.output.cell_count())
        .sum::<usize>();
    let test_tokens = task
        .test
        .iter()
        .map(arc_core::ArcGrid::cell_count)
        .sum::<usize>();
    (train_tokens + test_tokens) as u32
}

#[derive(Deserialize)]
struct RawArcTaskFile {
    id: Option<ArcTaskId>,
    train: Vec<arc_core::ArcExample>,
    test: Vec<arc_core::ArcGrid>,
}

#[derive(Debug, Error)]
pub enum ArcDatasetError {
    #[error("failed to read ARC dataset file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse ARC dataset file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid ARC task filename for deriving a task id: {0}")]
    InvalidTaskFilename(PathBuf),
    #[error(transparent)]
    TaskId(#[from] ArcTaskIdError),
    #[error(transparent)]
    Task(#[from] ArcTaskError),
    #[error(transparent)]
    Serialization(#[from] arc_core::ContractSerializationError),
    #[error(transparent)]
    DatasetContract(#[from] DatasetContractError),
    #[error("ARC dataset split `{split_name}` contained no task files")]
    SplitHasNoTasks { split_name: String },
}

#[cfg(test)]
mod tests {
    use super::{ArcDatasetFamily, load_collection, load_split_from_dir};

    fn fixture_dir(path: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
    }

    #[test]
    fn split_loader_derives_task_ids_from_filenames() {
        let train = load_split_from_dir(
            ArcDatasetFamily::ArcAgi1,
            super::ArcDatasetSplit::Train,
            fixture_dir("fixtures/arc_agi1/train"),
        )
        .expect("train split should load");

        assert_eq!(train.tasks.len(), 1);
        assert_eq!(train.tasks[0].task.id.as_str(), "demo_square");
        assert!(train.tasks[0].sequence_tokens > 0);
    }

    #[test]
    fn collection_exports_valid_psionic_manifest() {
        let collection = load_collection(
            ArcDatasetFamily::ArcAgi1,
            "2026.03.15",
            fixture_dir("fixtures/arc_agi1/train"),
            fixture_dir("fixtures/arc_agi1/evaluation"),
        )
        .expect("collection should load");

        let manifest = collection
            .to_psionic_manifest()
            .expect("manifest should export");
        manifest.validate().expect("manifest should validate");
        assert_eq!(manifest.key.dataset_ref, "dataset://arc/agi1");
        assert_eq!(
            manifest.split("train").map(|split| split.sequence_count),
            Some(1)
        );
        assert_eq!(
            manifest
                .split("evaluation")
                .map(|split| split.sequence_count),
            Some(1)
        );
    }

    #[test]
    fn arc_agi2_loader_uses_same_public_schema() {
        let collection = load_collection(
            ArcDatasetFamily::ArcAgi2,
            "2026.03.15",
            fixture_dir("fixtures/arc_agi2/train"),
            fixture_dir("fixtures/arc_agi2/evaluation"),
        )
        .expect("arc agi2 collection should load");

        assert_eq!(collection.splits.len(), 2);
        assert_eq!(collection.splits[0].tasks[0].task.id.as_str(), "demo_ring");
        assert_eq!(collection.splits[1].tasks[0].task.id.as_str(), "demo_shift");
    }
}
