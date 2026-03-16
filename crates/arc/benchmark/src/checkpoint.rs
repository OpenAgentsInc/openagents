use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_core::{
    ArcBenchmark, ArcGameState, ArcGrid, ArcOperationMode, ArcRecording, ArcScorePolicyId,
    ArcScorecard, ArcTaskId,
};
use serde::{Deserialize, Serialize};

use crate::ArcBenchmarkError;
use crate::interactive::{ArcInteractiveRunReport, ArcInteractiveStepSummary};

const TASK_CHECKPOINT_SCHEMA_VERSION: u16 = 1;
const RUN_MANIFEST_SCHEMA_VERSION: u16 = 1;
const INTERACTIVE_CHECKPOINT_SCHEMA_VERSION: u16 = 1;

const METADATA_FILE: &str = "metadata.json";
const COSTS_FILE: &str = "costs.json";
const RECORDING_FILE: &str = "recording.json";
const SCORECARD_FILE: &str = "scorecard.json";
const STEP_SUMMARIES_FILE: &str = "step_summaries.json";
const ERROR_FILE: &str = "error.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArcBenchmarkUsageTotals {
    pub total_cost_usd: f64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcTaskAttemptCheckpoint {
    pub attempt_index: u16,
    pub test_pair_index: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction: Option<ArcGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "f64_is_zero")]
    pub cost_usd: f64,
    #[serde(default, skip_serializing_if = "u32_is_zero")]
    pub tokens_input: u32,
    #[serde(default, skip_serializing_if = "u32_is_zero")]
    pub tokens_output: u32,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub duration_millis: u64,
    pub recorded_at_unix_s: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcTaskCheckpoint {
    pub schema_version: u16,
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed_attempts: Vec<ArcTaskAttemptCheckpoint>,
    #[serde(default, skip_serializing_if = "f64_is_zero")]
    pub total_cost_usd: f64,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub total_tokens_input: u64,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub total_tokens_output: u64,
    pub started_at_unix_s: u64,
    pub updated_at_unix_s: u64,
}

impl ArcTaskCheckpoint {
    fn new(benchmark: ArcBenchmark, task_id: ArcTaskId, now_unix_s: u64) -> Self {
        Self {
            schema_version: TASK_CHECKPOINT_SCHEMA_VERSION,
            benchmark,
            task_id,
            completed_attempts: Vec::new(),
            total_cost_usd: 0.0,
            total_tokens_input: 0,
            total_tokens_output: 0,
            started_at_unix_s: now_unix_s,
            updated_at_unix_s: now_unix_s,
        }
    }
}

#[derive(Debug)]
pub struct ArcTaskCheckpointManager {
    checkpoint_path: PathBuf,
    checkpoint: ArcTaskCheckpoint,
}

impl ArcTaskCheckpointManager {
    pub fn open(
        task_id: ArcTaskId,
        benchmark: ArcBenchmark,
        checkpoint_dir: impl AsRef<Path>,
    ) -> Result<Self, ArcBenchmarkError> {
        let checkpoint_path = checkpoint_dir
            .as_ref()
            .join(format!("{}.json", task_id.as_str()));
        let checkpoint = if checkpoint_path.exists() {
            let checkpoint: ArcTaskCheckpoint =
                serde_json::from_slice(&fs::read(&checkpoint_path)?)?;
            if checkpoint.schema_version != TASK_CHECKPOINT_SCHEMA_VERSION {
                return Err(ArcBenchmarkError::UnsupportedTaskCheckpointSchemaVersion {
                    version: checkpoint.schema_version,
                });
            }
            if checkpoint.task_id != task_id {
                return Err(ArcBenchmarkError::TaskCheckpointTaskMismatch {
                    expected: task_id,
                    actual: checkpoint.task_id,
                });
            }
            if checkpoint.benchmark != benchmark {
                return Err(ArcBenchmarkError::TaskCheckpointBenchmarkMismatch {
                    task_id: checkpoint.task_id.clone(),
                    expected: benchmark,
                    actual: checkpoint.benchmark,
                });
            }
            checkpoint
        } else {
            ArcTaskCheckpoint::new(benchmark, task_id, unix_timestamp_seconds())
        };

        Ok(Self {
            checkpoint_path,
            checkpoint,
        })
    }

    #[must_use]
    pub fn checkpoint(&self) -> &ArcTaskCheckpoint {
        &self.checkpoint
    }

    pub fn get_completed_attempts(&self) -> &[ArcTaskAttemptCheckpoint] {
        &self.checkpoint.completed_attempts
    }

    pub fn get_next_attempt_index(&self, test_pair_index: u16, max_attempts: u16) -> Option<u16> {
        for attempt_index in 0..max_attempts {
            let exists = self.checkpoint.completed_attempts.iter().any(|attempt| {
                attempt.test_pair_index == test_pair_index && attempt.attempt_index == attempt_index
            });
            if !exists {
                return Some(attempt_index);
            }
        }
        None
    }

    pub fn record_attempt(
        &mut self,
        attempt: ArcTaskAttemptCheckpoint,
    ) -> Result<(), ArcBenchmarkError> {
        let recorded_at_unix_s = attempt.recorded_at_unix_s;
        self.record_attempt_at(attempt, recorded_at_unix_s)
    }

    pub fn record_attempt_at(
        &mut self,
        attempt: ArcTaskAttemptCheckpoint,
        now_unix_s: u64,
    ) -> Result<(), ArcBenchmarkError> {
        let duplicate = self.checkpoint.completed_attempts.iter().any(|existing| {
            existing.test_pair_index == attempt.test_pair_index
                && existing.attempt_index == attempt.attempt_index
        });
        if duplicate {
            return Err(ArcBenchmarkError::DuplicateCheckpointAttempt {
                task_id: self.checkpoint.task_id.clone(),
                test_pair_index: attempt.test_pair_index,
                attempt_index: attempt.attempt_index,
            });
        }

        self.checkpoint.total_cost_usd += attempt.cost_usd;
        self.checkpoint.total_tokens_input += u64::from(attempt.tokens_input);
        self.checkpoint.total_tokens_output += u64::from(attempt.tokens_output);
        self.checkpoint.updated_at_unix_s = now_unix_s;
        self.checkpoint.completed_attempts.push(attempt);
        self.save()
    }

    pub fn delete_checkpoint(&mut self) -> Result<(), ArcBenchmarkError> {
        if self.checkpoint_path.exists() {
            fs::remove_file(&self.checkpoint_path)?;
        }
        self.checkpoint = ArcTaskCheckpoint::new(
            self.checkpoint.benchmark,
            self.checkpoint.task_id.clone(),
            unix_timestamp_seconds(),
        );
        Ok(())
    }

    fn save(&self) -> Result<(), ArcBenchmarkError> {
        ensure_parent_dir(&self.checkpoint_path)?;
        fs::write(
            &self.checkpoint_path,
            serde_json::to_vec_pretty(&self.checkpoint)?,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcRunTaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRunTaskProgress {
    pub task_id: ArcTaskId,
    pub status: ArcRunTaskStatus,
    #[serde(default, skip_serializing_if = "u32_is_zero")]
    pub attempts_completed: u32,
    #[serde(default, skip_serializing_if = "u32_is_zero")]
    pub attempts_total: u32,
    #[serde(default, skip_serializing_if = "f64_is_zero")]
    pub cost_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_unix_s: Option<u64>,
}

impl ArcRunTaskProgress {
    fn new(task_id: ArcTaskId, attempts_total: u32) -> Self {
        Self {
            task_id,
            status: ArcRunTaskStatus::Pending,
            attempts_completed: 0,
            attempts_total,
            cost_usd: 0.0,
            error: None,
            worker_id: None,
            started_at_unix_s: None,
            completed_at_unix_s: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRunManifest {
    pub schema_version: u16,
    pub run_id: String,
    pub benchmark: ArcBenchmark,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_mode: Option<ArcOperationMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_policy_id: Option<ArcScorePolicyId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tasks: BTreeMap<String, ArcRunTaskProgress>,
    #[serde(default, skip_serializing_if = "f64_is_zero")]
    pub total_cost_usd: f64,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub total_tokens_input: u64,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub total_tokens_output: u64,
    pub started_at_unix_s: u64,
    pub updated_at_unix_s: u64,
}

impl ArcRunManifest {
    fn new(run_id: String, benchmark: ArcBenchmark, now_unix_s: u64) -> Self {
        Self {
            schema_version: RUN_MANIFEST_SCHEMA_VERSION,
            run_id,
            benchmark,
            operation_mode: None,
            score_policy_id: None,
            tasks: BTreeMap::new(),
            total_cost_usd: 0.0,
            total_tokens_input: 0,
            total_tokens_output: 0,
            started_at_unix_s: now_unix_s,
            updated_at_unix_s: now_unix_s,
        }
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.status == ArcRunTaskStatus::Pending)
            .count()
    }

    #[must_use]
    pub fn in_progress_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.status == ArcRunTaskStatus::InProgress)
            .count()
    }

    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.status == ArcRunTaskStatus::Completed)
            .count()
    }

    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.status == ArcRunTaskStatus::Failed)
            .count()
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.tasks.values().all(|task| {
            matches!(
                task.status,
                ArcRunTaskStatus::Completed | ArcRunTaskStatus::Failed
            )
        })
    }
}

#[derive(Debug)]
pub struct ArcRunManifestManager {
    manifest_path: PathBuf,
    worker_id: String,
    manifest: ArcRunManifest,
}

impl ArcRunManifestManager {
    pub fn open(
        run_id: impl Into<String>,
        benchmark: ArcBenchmark,
        manifest_path: impl AsRef<Path>,
    ) -> Result<Self, ArcBenchmarkError> {
        Self::open_with_worker_id(run_id, benchmark, manifest_path, default_worker_id())
    }

    pub fn open_with_worker_id(
        run_id: impl Into<String>,
        benchmark: ArcBenchmark,
        manifest_path: impl AsRef<Path>,
        worker_id: impl Into<String>,
    ) -> Result<Self, ArcBenchmarkError> {
        let run_id = run_id.into();
        let manifest_path = manifest_path.as_ref().to_path_buf();
        let manifest = if manifest_path.exists() {
            let manifest: ArcRunManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
            if manifest.schema_version != RUN_MANIFEST_SCHEMA_VERSION {
                return Err(ArcBenchmarkError::UnsupportedRunManifestSchemaVersion {
                    version: manifest.schema_version,
                });
            }
            if manifest.run_id != run_id {
                return Err(ArcBenchmarkError::RunManifestRunIdMismatch {
                    expected: run_id,
                    actual: manifest.run_id,
                });
            }
            if manifest.benchmark != benchmark {
                return Err(ArcBenchmarkError::RunManifestBenchmarkMismatch {
                    expected: benchmark,
                    actual: manifest.benchmark,
                });
            }
            manifest
        } else {
            ArcRunManifest::new(run_id, benchmark, unix_timestamp_seconds())
        };

        Ok(Self {
            manifest_path,
            worker_id: worker_id.into(),
            manifest,
        })
    }

    #[must_use]
    pub fn manifest(&self) -> &ArcRunManifest {
        &self.manifest
    }

    pub fn initialize_tasks(
        &mut self,
        task_ids: &[ArcTaskId],
        attempts_per_task: u32,
    ) -> Result<(), ArcBenchmarkError> {
        for task_id in task_ids {
            self.manifest
                .tasks
                .entry(task_id.as_str().to_owned())
                .or_insert_with(|| ArcRunTaskProgress::new(task_id.clone(), attempts_per_task));
        }
        self.touch(unix_timestamp_seconds())?;
        Ok(())
    }

    pub fn claim_task(&mut self, task_id: &ArcTaskId) -> Result<bool, ArcBenchmarkError> {
        self.claim_task_at(task_id, unix_timestamp_seconds())
    }

    pub fn claim_task_at(
        &mut self,
        task_id: &ArcTaskId,
        now_unix_s: u64,
    ) -> Result<bool, ArcBenchmarkError> {
        let Some(task) = self.manifest.tasks.get_mut(task_id.as_str()) else {
            return Err(ArcBenchmarkError::UnknownRunTask {
                task_id: task_id.clone(),
            });
        };
        if task.status != ArcRunTaskStatus::Pending {
            return Ok(false);
        }

        task.status = ArcRunTaskStatus::InProgress;
        task.worker_id = Some(self.worker_id.clone());
        task.started_at_unix_s = Some(now_unix_s);
        task.completed_at_unix_s = None;
        self.touch(now_unix_s)?;
        Ok(true)
    }

    pub fn claim_next_task(&mut self) -> Result<Option<ArcTaskId>, ArcBenchmarkError> {
        self.claim_next_task_at(unix_timestamp_seconds())
    }

    pub fn claim_next_task_at(
        &mut self,
        now_unix_s: u64,
    ) -> Result<Option<ArcTaskId>, ArcBenchmarkError> {
        let task_id = self
            .manifest
            .tasks
            .values()
            .find(|task| task.status == ArcRunTaskStatus::Pending)
            .map(|task| task.task_id.clone());
        if let Some(task_id) = task_id {
            if self.claim_task_at(&task_id, now_unix_s)? {
                return Ok(Some(task_id));
            }
        }
        Ok(None)
    }

    pub fn update_task_progress(
        &mut self,
        task_id: &ArcTaskId,
        attempts_completed: u32,
        cost_usd: f64,
    ) -> Result<(), ArcBenchmarkError> {
        let Some(task) = self.manifest.tasks.get_mut(task_id.as_str()) else {
            return Err(ArcBenchmarkError::UnknownRunTask {
                task_id: task_id.clone(),
            });
        };
        task.attempts_completed = attempts_completed;
        task.cost_usd = cost_usd;
        self.touch(unix_timestamp_seconds())
    }

    pub fn mark_completed(
        &mut self,
        task_id: &ArcTaskId,
        totals: ArcBenchmarkUsageTotals,
    ) -> Result<(), ArcBenchmarkError> {
        self.mark_completed_at(task_id, totals, unix_timestamp_seconds())
    }

    pub fn mark_completed_at(
        &mut self,
        task_id: &ArcTaskId,
        totals: ArcBenchmarkUsageTotals,
        now_unix_s: u64,
    ) -> Result<(), ArcBenchmarkError> {
        let Some(task) = self.manifest.tasks.get_mut(task_id.as_str()) else {
            return Err(ArcBenchmarkError::UnknownRunTask {
                task_id: task_id.clone(),
            });
        };

        task.status = ArcRunTaskStatus::Completed;
        task.completed_at_unix_s = Some(now_unix_s);
        task.cost_usd = totals.total_cost_usd;
        task.attempts_completed = task.attempts_total;
        self.manifest.total_cost_usd += totals.total_cost_usd;
        self.manifest.total_tokens_input += totals.total_tokens_input;
        self.manifest.total_tokens_output += totals.total_tokens_output;
        self.touch(now_unix_s)
    }

    pub fn mark_failed(
        &mut self,
        task_id: &ArcTaskId,
        error: impl Into<String>,
        totals: ArcBenchmarkUsageTotals,
    ) -> Result<(), ArcBenchmarkError> {
        self.mark_failed_at(task_id, error, totals, unix_timestamp_seconds())
    }

    pub fn mark_failed_at(
        &mut self,
        task_id: &ArcTaskId,
        error: impl Into<String>,
        totals: ArcBenchmarkUsageTotals,
        now_unix_s: u64,
    ) -> Result<(), ArcBenchmarkError> {
        let Some(task) = self.manifest.tasks.get_mut(task_id.as_str()) else {
            return Err(ArcBenchmarkError::UnknownRunTask {
                task_id: task_id.clone(),
            });
        };

        task.status = ArcRunTaskStatus::Failed;
        task.error = Some(error.into());
        task.completed_at_unix_s = Some(now_unix_s);
        task.cost_usd = totals.total_cost_usd;
        self.manifest.total_cost_usd += totals.total_cost_usd;
        self.manifest.total_tokens_input += totals.total_tokens_input;
        self.manifest.total_tokens_output += totals.total_tokens_output;
        self.touch(now_unix_s)
    }

    pub fn reset_stale_tasks(&mut self, max_age_secs: u64) -> Result<u32, ArcBenchmarkError> {
        self.reset_stale_tasks_at(unix_timestamp_seconds(), max_age_secs)
    }

    pub fn reset_stale_tasks_at(
        &mut self,
        now_unix_s: u64,
        max_age_secs: u64,
    ) -> Result<u32, ArcBenchmarkError> {
        let mut reset_count = 0u32;
        for task in self.manifest.tasks.values_mut() {
            if task.status != ArcRunTaskStatus::InProgress {
                continue;
            }
            let Some(started_at_unix_s) = task.started_at_unix_s else {
                continue;
            };
            if now_unix_s.saturating_sub(started_at_unix_s) > max_age_secs {
                task.status = ArcRunTaskStatus::Pending;
                task.worker_id = None;
                task.started_at_unix_s = None;
                task.completed_at_unix_s = None;
                reset_count = reset_count.saturating_add(1);
            }
        }

        if reset_count > 0 {
            self.touch(now_unix_s)?;
        }
        Ok(reset_count)
    }

    pub fn retry_failed_tasks(&mut self) -> Result<u32, ArcBenchmarkError> {
        let mut reset_count = 0u32;
        for task in self.manifest.tasks.values_mut() {
            if task.status == ArcRunTaskStatus::Failed {
                task.status = ArcRunTaskStatus::Pending;
                task.error = None;
                task.worker_id = None;
                task.started_at_unix_s = None;
                task.completed_at_unix_s = None;
                reset_count = reset_count.saturating_add(1);
            }
        }
        if reset_count > 0 {
            self.touch(unix_timestamp_seconds())?;
        }
        Ok(reset_count)
    }

    fn touch(&mut self, now_unix_s: u64) -> Result<(), ArcBenchmarkError> {
        self.manifest.updated_at_unix_s = now_unix_s;
        self.save()
    }

    fn save(&self) -> Result<(), ArcBenchmarkError> {
        ensure_parent_dir(&self.manifest_path)?;
        fs::write(
            &self.manifest_path,
            serde_json::to_vec_pretty(&self.manifest)?,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcCheckpointErrorRecord {
    pub message: String,
    pub timestamp_unix_s: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveCheckpointMetadata {
    pub schema_version: u16,
    pub checkpoint_id: String,
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_mode: Option<ArcOperationMode>,
    pub score_policy_id: ArcScorePolicyId,
    pub recording_digest: String,
    pub total_actions: u32,
    pub resets: u32,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub final_state: ArcGameState,
    pub step_count: u32,
    pub checkpoint_timestamp_unix_s: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveCheckpointBundle {
    pub metadata: ArcInteractiveCheckpointMetadata,
    pub costs: ArcBenchmarkUsageTotals,
    pub recording: ArcRecording,
    pub scorecard: ArcScorecard,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_summaries: Vec<ArcInteractiveStepSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ArcCheckpointErrorRecord>,
}

impl ArcInteractiveCheckpointBundle {
    pub fn from_run_report(
        checkpoint_id: impl Into<String>,
        report: &ArcInteractiveRunReport,
        costs: ArcBenchmarkUsageTotals,
        checkpoint_timestamp_unix_s: u64,
        recording: ArcRecording,
    ) -> Result<Self, ArcBenchmarkError> {
        let bundle = Self {
            metadata: ArcInteractiveCheckpointMetadata {
                schema_version: INTERACTIVE_CHECKPOINT_SCHEMA_VERSION,
                checkpoint_id: checkpoint_id.into(),
                benchmark: report.benchmark,
                task_id: report.task_id.clone(),
                operation_mode: report.operation_mode,
                score_policy_id: report.score_policy_id,
                recording_digest: report.recording_digest.clone(),
                total_actions: report.total_actions,
                resets: report.resets,
                levels_completed: report.levels_completed,
                win_levels: report.win_levels,
                final_state: report.final_state,
                step_count: u32::try_from(report.step_summaries.len()).unwrap_or(u32::MAX),
                checkpoint_timestamp_unix_s,
            },
            costs,
            recording,
            scorecard: report.scorecard.clone(),
            step_summaries: report.step_summaries.clone(),
            error: None,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn save_to_dir(&self, checkpoint_dir: impl AsRef<Path>) -> Result<(), ArcBenchmarkError> {
        let checkpoint_dir = checkpoint_dir.as_ref();
        fs::create_dir_all(checkpoint_dir)?;
        fs::write(
            checkpoint_dir.join(METADATA_FILE),
            serde_json::to_vec_pretty(&self.metadata)?,
        )?;
        fs::write(
            checkpoint_dir.join(COSTS_FILE),
            serde_json::to_vec_pretty(&self.costs)?,
        )?;
        fs::write(
            checkpoint_dir.join(RECORDING_FILE),
            serde_json::to_vec_pretty(&self.recording)?,
        )?;
        fs::write(
            checkpoint_dir.join(SCORECARD_FILE),
            serde_json::to_vec_pretty(&self.scorecard)?,
        )?;
        fs::write(
            checkpoint_dir.join(STEP_SUMMARIES_FILE),
            serde_json::to_vec_pretty(&self.step_summaries)?,
        )?;
        if let Some(error) = &self.error {
            fs::write(
                checkpoint_dir.join(ERROR_FILE),
                serde_json::to_vec_pretty(error)?,
            )?;
        }
        Ok(())
    }

    pub fn load_from_dir(checkpoint_dir: impl AsRef<Path>) -> Result<Self, ArcBenchmarkError> {
        let checkpoint_dir = checkpoint_dir.as_ref();
        let metadata: ArcInteractiveCheckpointMetadata =
            serde_json::from_slice(&fs::read(checkpoint_dir.join(METADATA_FILE))?)?;
        if metadata.schema_version != INTERACTIVE_CHECKPOINT_SCHEMA_VERSION {
            return Err(
                ArcBenchmarkError::UnsupportedInteractiveCheckpointSchemaVersion {
                    version: metadata.schema_version,
                },
            );
        }

        let bundle = Self {
            metadata,
            costs: serde_json::from_slice(&fs::read(checkpoint_dir.join(COSTS_FILE))?)?,
            recording: serde_json::from_slice(&fs::read(checkpoint_dir.join(RECORDING_FILE))?)?,
            scorecard: serde_json::from_slice(&fs::read(checkpoint_dir.join(SCORECARD_FILE))?)?,
            step_summaries: serde_json::from_slice(&fs::read(
                checkpoint_dir.join(STEP_SUMMARIES_FILE),
            )?)?,
            error: read_optional_json(checkpoint_dir.join(ERROR_FILE))?,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    #[must_use]
    pub fn next_step_index(&self) -> u32 {
        self.step_summaries
            .last()
            .map(|step| step.step_index.saturating_add(1))
            .unwrap_or(0)
    }

    pub fn validate(&self) -> Result<(), ArcBenchmarkError> {
        if self.recording.task_id != self.metadata.task_id {
            return Err(ArcBenchmarkError::InteractiveCheckpointTaskMismatch {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                expected: self.metadata.task_id.clone(),
                actual: self.recording.task_id.clone(),
            });
        }
        if self.scorecard.task_id != self.metadata.task_id {
            return Err(ArcBenchmarkError::InteractiveCheckpointTaskMismatch {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                expected: self.metadata.task_id.clone(),
                actual: self.scorecard.task_id.clone(),
            });
        }
        if self.recording.benchmark != self.metadata.benchmark {
            return Err(ArcBenchmarkError::InteractiveCheckpointBenchmarkMismatch {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                expected: self.metadata.benchmark,
                actual: self.recording.benchmark,
            });
        }
        if self.scorecard.benchmark != self.metadata.benchmark {
            return Err(ArcBenchmarkError::InteractiveCheckpointBenchmarkMismatch {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                expected: self.metadata.benchmark,
                actual: self.scorecard.benchmark,
            });
        }
        let actual_digest = self.recording.contract_digest()?;
        if actual_digest != self.metadata.recording_digest {
            return Err(
                ArcBenchmarkError::InteractiveCheckpointRecordingDigestMismatch {
                    checkpoint_id: self.metadata.checkpoint_id.clone(),
                    expected: self.metadata.recording_digest.clone(),
                    actual: actual_digest,
                },
            );
        }
        let actual_step_count = self.step_summaries.len();
        let expected_step_count = usize::try_from(self.metadata.step_count).unwrap_or(usize::MAX);
        if actual_step_count != expected_step_count {
            return Err(ArcBenchmarkError::InteractiveCheckpointStepCountMismatch {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                expected: expected_step_count,
                actual: actual_step_count,
            });
        }
        Ok(())
    }
}

fn read_optional_json<T: for<'de> Deserialize<'de>>(
    path: PathBuf,
) -> Result<Option<T>, ArcBenchmarkError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn ensure_parent_dir(path: &Path) -> Result<(), ArcBenchmarkError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn default_worker_id() -> String {
    format!("pid-{}", std::process::id())
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn f64_is_zero(value: &f64) -> bool {
    *value == 0.0
}

fn u32_is_zero(value: &u32) -> bool {
    *value == 0
}

fn u64_is_zero(value: &u64) -> bool {
    *value == 0
}
