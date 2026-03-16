use std::collections::BTreeSet;

use arc_core::{ArcGrid, ArcTask, ArcTaskId, TraceLocator, canonical_sha256_hex};
use psionic_data::{
    DatasetKey, DatasetManifest, DatasetRecordEncoding, DatasetShardManifest,
    DatasetSplitDeclaration,
};
use psionic_datastream::{
    DatastreamDatasetBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ArcDatasetError, ArcDatasetFamily, ArcDatasetSplit, arc_tokenizer_digest, task_sequence_tokens,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcAugmentationKind {
    Identity,
    FlipHorizontal,
    FlipVertical,
    RotateClockwise,
}

impl ArcAugmentationKind {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::FlipHorizontal => "flip_horizontal",
            Self::FlipVertical => "flip_vertical",
            Self::RotateClockwise => "rotate_clockwise",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSyntheticTaskLineage {
    pub source_task_id: ArcTaskId,
    pub source_task_digest: String,
    pub derived_task_id: ArcTaskId,
    pub derived_task_digest: String,
    pub augmentation: ArcAugmentationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_locator: Option<TraceLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSyntheticTask {
    pub task: ArcTask,
    pub lineage: ArcSyntheticTaskLineage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArcAugmentationBuilder {
    kinds: Vec<ArcAugmentationKind>,
}

impl ArcAugmentationBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self { kinds: Vec::new() }
    }

    #[must_use]
    pub fn identity(mut self) -> Self {
        self.push_unique(ArcAugmentationKind::Identity);
        self
    }

    #[must_use]
    pub fn flip_horizontal(mut self) -> Self {
        self.push_unique(ArcAugmentationKind::FlipHorizontal);
        self
    }

    #[must_use]
    pub fn flip_vertical(mut self) -> Self {
        self.push_unique(ArcAugmentationKind::FlipVertical);
        self
    }

    #[must_use]
    pub fn rotate_clockwise(mut self) -> Self {
        self.push_unique(ArcAugmentationKind::RotateClockwise);
        self
    }

    pub fn build(
        &self,
        source_task: &ArcTask,
        trace_locator: Option<TraceLocator>,
    ) -> Result<Vec<ArcSyntheticTask>, ArcDatasetError> {
        let source_task_digest = source_task.contract_digest()?;

        self.kinds
            .iter()
            .copied()
            .map(|augmentation| {
                let derived_task_id = ArcTaskId::new(format!(
                    "{}--{}",
                    source_task.id.as_str(),
                    augmentation.slug()
                ))?;
                let task = apply_augmentation(source_task, derived_task_id.clone(), augmentation)?;
                let derived_task_digest = task.contract_digest()?;

                Ok(ArcSyntheticTask {
                    task,
                    lineage: ArcSyntheticTaskLineage {
                        source_task_id: source_task.id.clone(),
                        source_task_digest: source_task_digest.clone(),
                        derived_task_id,
                        derived_task_digest,
                        augmentation,
                        trace_locator: trace_locator.clone(),
                    },
                })
            })
            .collect()
    }

    fn push_unique(&mut self, augmentation: ArcAugmentationKind) {
        if !self.kinds.contains(&augmentation) {
            self.kinds.push(augmentation);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcSyntheticDatasetPackage {
    pub family: ArcDatasetFamily,
    pub version: String,
    pub split: ArcDatasetSplit,
    pub tasks: Vec<ArcSyntheticTask>,
}

impl ArcSyntheticDatasetPackage {
    pub fn new(
        family: ArcDatasetFamily,
        version: impl Into<String>,
        split: ArcDatasetSplit,
        tasks: Vec<ArcSyntheticTask>,
    ) -> Result<Self, ArcDatasetError> {
        if tasks.is_empty() {
            return Err(ArcDatasetError::SplitHasNoTasks {
                split_name: String::from(split.as_str()),
            });
        }
        Ok(Self {
            family,
            version: version.into(),
            split,
            tasks,
        })
    }

    pub fn to_psionic_manifest(&self) -> Result<DatasetManifest, ArcDatasetError> {
        let dataset_key = DatasetKey::new(
            format!("dataset://arc/synthetic/{}", self.family.slug()),
            self.version.clone(),
        );
        let split_name = self.split.as_str();
        let shard_key = "synthetic_tasks";
        let task_digests = self
            .tasks
            .iter()
            .map(|task| task.lineage.derived_task_digest.clone())
            .collect::<Vec<_>>();
        let split_digest = canonical_sha256_hex(&task_digests)?;
        let total_bytes = self
            .tasks
            .iter()
            .map(|task| task.task.canonical_json().map(|json| json.len() as u64))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<u64>()
            .max(1);
        let token_counts = self
            .tasks
            .iter()
            .map(|task| task_sequence_tokens(&task.task))
            .collect::<Vec<_>>();
        let manifest_ref = DatastreamManifestRef {
            stream_id: format!(
                "arc://synthetic/{}/{}/{}",
                self.family.slug(),
                split_name,
                self.version
            ),
            manifest_digest: split_digest.clone(),
            subject: DatastreamSubjectKind::TokenizedCorpus,
            object_digest: split_digest.clone(),
            total_bytes,
            chunk_count: 1,
            chunk_bytes: total_bytes as usize,
            encoding: DatastreamEncoding::Jsonl,
            compression: None,
            provenance_digest: Some(split_digest.clone()),
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
            &dataset_key,
            split_name,
            shard_key,
            manifest_ref,
            self.tasks.len() as u64,
            token_counts.iter().map(|count| u64::from(*count)).sum(),
            token_counts.iter().copied().min().unwrap_or(1),
            token_counts.iter().copied().max().unwrap_or(1),
        )?;
        let split = DatasetSplitDeclaration::new(
            &dataset_key,
            split_name,
            self.split.psionic_kind(),
            vec![shard],
        )?;

        let source_task_ids = self
            .tasks
            .iter()
            .map(|task| task.lineage.source_task_id.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let augmentation_kinds = self
            .tasks
            .iter()
            .map(|task| task.lineage.augmentation.slug().to_owned())
            .collect::<BTreeSet<_>>();
        let trace_locators = self
            .tasks
            .iter()
            .filter_map(|task| task.lineage.trace_locator.as_ref())
            .map(|locator| locator.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let lineage_digest = canonical_sha256_hex(
            &self
                .tasks
                .iter()
                .map(|task| &task.lineage.derived_task_digest)
                .collect::<Vec<_>>(),
        )?;

        let mut manifest = DatasetManifest::new(
            dataset_key,
            format!("Synthetic {}", self.family.display_name()),
            DatasetRecordEncoding::JsonlText,
            arc_tokenizer_digest(),
        )
        .with_provenance_digest(lineage_digest)
        .with_splits(vec![split]);
        manifest.metadata.insert(
            String::from("arc_family"),
            Value::String(String::from(self.family.slug())),
        );
        manifest.metadata.insert(
            String::from("lineage_kind"),
            Value::String(String::from("synthetic_augmentation")),
        );
        manifest.metadata.insert(
            String::from("source_task_ids"),
            Value::Array(source_task_ids.into_iter().map(Value::String).collect()),
        );
        manifest.metadata.insert(
            String::from("augmentation_kinds"),
            Value::Array(augmentation_kinds.into_iter().map(Value::String).collect()),
        );
        manifest.metadata.insert(
            String::from("trace_locators"),
            Value::Array(trace_locators.into_iter().map(Value::String).collect()),
        );
        manifest.metadata.insert(
            String::from("synthetic_task_ids"),
            Value::Array(
                self.tasks
                    .iter()
                    .map(|task| Value::String(task.lineage.derived_task_id.as_str().to_owned()))
                    .collect(),
            ),
        );
        manifest.validate()?;
        Ok(manifest)
    }
}

fn apply_augmentation(
    source_task: &ArcTask,
    derived_task_id: ArcTaskId,
    augmentation: ArcAugmentationKind,
) -> Result<ArcTask, ArcDatasetError> {
    let train = source_task
        .train
        .iter()
        .map(|example| {
            Ok(arc_core::ArcExample {
                input: transform_grid(&example.input, augmentation)?,
                output: transform_grid(&example.output, augmentation)?,
            })
        })
        .collect::<Result<Vec<_>, ArcDatasetError>>()?;
    let test = source_task
        .test
        .iter()
        .map(|grid| transform_grid(grid, augmentation))
        .collect::<Result<Vec<_>, ArcDatasetError>>()?;
    ArcTask::new(derived_task_id, train, test).map_err(Into::into)
}

fn transform_grid(
    grid: &ArcGrid,
    augmentation: ArcAugmentationKind,
) -> Result<ArcGrid, ArcDatasetError> {
    let (width, height, cells) = match augmentation {
        ArcAugmentationKind::Identity => (grid.width(), grid.height(), grid.cells().to_vec()),
        ArcAugmentationKind::FlipHorizontal => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(read_cell(grid, grid.width() - 1 - x, y, augmentation)?);
                }
            }
            (grid.width(), grid.height(), cells)
        }
        ArcAugmentationKind::FlipVertical => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(read_cell(grid, x, grid.height() - 1 - y, augmentation)?);
                }
            }
            (grid.width(), grid.height(), cells)
        }
        ArcAugmentationKind::RotateClockwise => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.width() {
                for x in 0..grid.height() {
                    cells.push(read_cell(grid, y, grid.height() - 1 - x, augmentation)?);
                }
            }
            (grid.height(), grid.width(), cells)
        }
    };

    ArcGrid::new(width, height, cells).map_err(Into::into)
}

fn read_cell(
    grid: &ArcGrid,
    x: u8,
    y: u8,
    augmentation: ArcAugmentationKind,
) -> Result<u8, ArcDatasetError> {
    grid.cell(x, y)
        .ok_or_else(|| ArcDatasetError::AugmentationOutOfRange {
            augmentation: augmentation.slug(),
            width: grid.width(),
            height: grid.height(),
            x,
            y,
        })
}
