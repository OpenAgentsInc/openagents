use std::{collections::BTreeMap, fs, path::Path};

use psionic_eval::EvalArtifact;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE, TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE,
    TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE, TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE,
    TassadarExecutorReferenceRunBundle, TassadarExecutorRunError,
    TassadarExecutorTraceDivergenceReport, TassadarExecutorTrainingTelemetryReport,
};

/// Canonical machine-readable postmortem artifact file.
pub const TASSADAR_EXECUTOR_POSTMORTEM_FILE: &str = "postmortem.json";
/// Canonical machine-readable next-run plan artifact file.
pub const TASSADAR_EXECUTOR_NEXT_RUN_PLAN_FILE: &str = "next_run_plan.json";

const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// Whether one finding is directly observed in artifacts or inferred from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorFindingPosture {
    /// Directly observed in the persisted artifacts.
    Observed,
    /// Inferred from the persisted artifacts plus the current lane design.
    Inferred,
}

/// Typed finding class for the first-run postmortem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorFindingKind {
    /// Prompt-to-trace transition fails immediately.
    PromptTraceBoundaryCollapse,
    /// Training budget is too small for the target regime.
    OptimizationBudgetTooSmall,
    /// Long traces need staged curriculum rather than one flat regime.
    TraceLengthCurriculumMismatch,
}

/// Severity for one postmortem finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorFindingSeverity {
    /// Highest-priority blocking finding.
    Critical,
    /// Important but downstream of a higher-priority blocker.
    High,
    /// Material but not yet the top blocker.
    Medium,
}

/// One machine-readable postmortem finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorPostmortemFinding {
    /// Stable finding identifier.
    pub finding_id: String,
    /// Typed finding class.
    pub kind: TassadarExecutorFindingKind,
    /// Observed vs inferred posture.
    pub posture: TassadarExecutorFindingPosture,
    /// Severity.
    pub severity: TassadarExecutorFindingSeverity,
    /// Human-readable summary.
    pub summary: String,
    /// Supporting metrics.
    pub metrics: Value,
    /// Supporting artifact refs.
    pub supporting_artifacts: Vec<String>,
}

/// Machine-readable first-run postmortem.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorPostmortemReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Dataset storage key used for the run.
    pub dataset_storage_key: String,
    /// Dataset digest used for the run.
    pub dataset_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Findings extracted from the first run.
    pub findings: Vec<TassadarExecutorPostmortemFinding>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarExecutorPostmortemReport {
    fn new(
        run_bundle: &TassadarExecutorReferenceRunBundle,
        training_telemetry: &TassadarExecutorTrainingTelemetryReport,
        divergence: &TassadarExecutorTraceDivergenceReport,
    ) -> Self {
        let case_count = divergence.case_reports.len() as u32;
        let first_divergence_zero_cases = divergence
            .case_reports
            .iter()
            .filter(|case| case.first_divergence_index == Some(0))
            .count() as u32;
        let exactness_min = divergence
            .case_reports
            .iter()
            .map(|case| case.target_token_exactness_bps)
            .min()
            .unwrap_or(0);
        let exactness_max = divergence
            .case_reports
            .iter()
            .map(|case| case.target_token_exactness_bps)
            .max()
            .unwrap_or(0);
        let min_target_tokens = divergence
            .case_reports
            .iter()
            .map(|case| case.target_token_count)
            .min()
            .unwrap_or(0);
        let max_target_tokens = divergence
            .case_reports
            .iter()
            .map(|case| case.target_token_count)
            .max()
            .unwrap_or(0);
        let total_supervised_tokens = training_telemetry
            .epoch_reports
            .iter()
            .map(|epoch| epoch.target_token_count)
            .sum::<u32>();
        let epoch_count = training_telemetry.epoch_reports.len() as u32;
        let step_count = training_telemetry.step_reports.len() as u32;

        let findings = vec![
            TassadarExecutorPostmortemFinding {
                finding_id: String::from("prompt_trace_boundary_collapse"),
                kind: TassadarExecutorFindingKind::PromptTraceBoundaryCollapse,
                posture: TassadarExecutorFindingPosture::Observed,
                severity: TassadarExecutorFindingSeverity::Critical,
                summary: String::from(
                    "Every analyzed case diverges at the first target token, so the model is not yet making the prompt-to-trace transition at all.",
                ),
                metrics: json!({
                    "case_count": case_count,
                    "first_divergence_zero_cases": first_divergence_zero_cases,
                    "exactness_bps_min": exactness_min,
                    "exactness_bps_max": exactness_max,
                }),
                supporting_artifacts: vec![
                    String::from(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE),
                    String::from(TASSADAR_EXECUTOR_FAILURE_SAMPLES_FILE),
                ],
            },
            TassadarExecutorPostmortemFinding {
                finding_id: String::from("optimization_budget_too_small"),
                kind: TassadarExecutorFindingKind::OptimizationBudgetTooSmall,
                posture: TassadarExecutorFindingPosture::Observed,
                severity: TassadarExecutorFindingSeverity::High,
                summary: String::from(
                    "The first run only executes one training step over one epoch and 1,024 supervised target tokens, which is too little budget for a serious executor claim.",
                ),
                metrics: json!({
                    "step_count": step_count,
                    "epoch_count": epoch_count,
                    "total_supervised_target_tokens": total_supervised_tokens,
                    "mean_loss_milli": training_telemetry
                        .epoch_reports
                        .first()
                        .map(|epoch| epoch.mean_loss_milli)
                        .unwrap_or(0),
                }),
                supporting_artifacts: vec![
                    String::from(TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE),
                    String::from("training_report.json"),
                ],
            },
            TassadarExecutorPostmortemFinding {
                finding_id: String::from("trace_length_curriculum_mismatch"),
                kind: TassadarExecutorFindingKind::TraceLengthCurriculumMismatch,
                posture: TassadarExecutorFindingPosture::Observed,
                severity: TassadarExecutorFindingSeverity::High,
                summary: String::from(
                    "The run is targeting very long executor traces without a staged short-trace curriculum, which leaves later phases underconstrained even if the boundary issue is fixed.",
                ),
                metrics: json!({
                    "min_target_token_count": min_target_tokens,
                    "max_target_token_count": max_target_tokens,
                    "exactness_curve_bucket_count": 256,
                }),
                supporting_artifacts: vec![
                    String::from(TASSADAR_EXECUTOR_EXACTNESS_CURVE_FILE),
                    String::from(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE),
                ],
            },
        ];

        let mut report = Self {
            run_id: run_bundle.run_id.clone(),
            dataset_storage_key: run_bundle.dataset_storage_key.clone(),
            dataset_digest: run_bundle.dataset_digest.clone(),
            trained_model_descriptor_digest: run_bundle.trained_model_descriptor_digest.clone(),
            findings,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_executor_postmortem_report|", &report);
        report
    }
}

/// Action class for the next run plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorNextRunActionKind {
    /// Adjust the curriculum or dataset presentation.
    CurriculumChange,
    /// Adjust the training budget or schedule.
    TrainingConfigChange,
    /// Adjust the trainable model surface.
    ModelChange,
    /// Gate later roadmap work on stronger evidence.
    PhaseGate,
}

/// One explicit next-run action derived from the postmortem.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorNextRunAction {
    /// Stable action identifier.
    pub action_id: String,
    /// Priority, lowest number first.
    pub priority: u32,
    /// Typed action kind.
    pub kind: TassadarExecutorNextRunActionKind,
    /// Human-readable summary.
    pub summary: String,
    /// Why this action exists.
    pub rationale: String,
    /// Concrete changes to make before the next run.
    pub planned_changes: Vec<String>,
    /// Concrete success criteria for the next run.
    pub success_criteria: Vec<String>,
}

/// Machine-readable next-run plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorNextRunPlan {
    /// Stable run identifier this plan is based on.
    pub run_id: String,
    /// Postmortem digest that grounded the plan.
    pub basis_postmortem_digest: String,
    /// Ordered next-run actions.
    pub actions: Vec<TassadarExecutorNextRunAction>,
    /// Stable plan digest.
    pub plan_digest: String,
}

impl TassadarExecutorNextRunPlan {
    fn new(run_id: &str, postmortem: &TassadarExecutorPostmortemReport) -> Self {
        let actions = vec![
            TassadarExecutorNextRunAction {
                action_id: String::from("boundary_curriculum_v1"),
                priority: 1,
                kind: TassadarExecutorNextRunActionKind::CurriculumChange,
                summary: String::from(
                    "Introduce a short-trace boundary curriculum before any broader decode claims.",
                ),
                rationale: String::from(
                    "All 8 cases fail at target token 0, so the next run must learn the prompt-to-trace transition before long-trace behavior matters.",
                ),
                planned_changes: vec![
                    String::from(
                        "Add an explicit first-target-token eval metric and report it every epoch.",
                    ),
                    String::from(
                        "Train on 1-, 8-, 32-, and 128-target-token slices before reusing the current 256-target-token cap.",
                    ),
                    String::from(
                        "Oversample trace starts whose first target token is `<step>` to make the boundary objective explicit.",
                    ),
                ],
                success_criteria: vec![
                    String::from(
                        "Validation first-target exactness reaches 100% (2/2 cases) before broader decode evaluation is treated as meaningful.",
                    ),
                    String::from("Validation first-32-target-token exactness exceeds 8,000 bps."),
                ],
            },
            TassadarExecutorNextRunAction {
                action_id: String::from("optimizer_budget_v1"),
                priority: 2,
                kind: TassadarExecutorNextRunActionKind::TrainingConfigChange,
                summary: String::from(
                    "Increase training budget and checkpoint cadence enough to observe a learning curve.",
                ),
                rationale: String::from(
                    "The first run only used one step, one epoch, and 1,024 supervised target tokens, so there is no meaningful optimization sweep yet.",
                ),
                planned_changes: vec![
                    String::from(
                        "Increase the next reference run from 1 epoch to at least 8 epochs.",
                    ),
                    String::from(
                        "Emit validation telemetry after every epoch instead of only at the terminal run boundary.",
                    ),
                    String::from(
                        "Persist one checkpoint per epoch so failure analysis can compare early vs later learning.",
                    ),
                ],
                success_criteria: vec![
                    String::from(
                        "Aggregate validation target exactness materially exceeds the current 13 bps baseline.",
                    ),
                    String::from(
                        "Per-epoch telemetry shows monotonic improvement on the first-target and first-32-token metrics.",
                    ),
                ],
            },
            TassadarExecutorNextRunAction {
                action_id: String::from("trainable_surface_v1"),
                priority: 3,
                kind: TassadarExecutorNextRunActionKind::ModelChange,
                summary: String::from(
                    "Expand the trainable surface if the boundary curriculum still fails.",
                ),
                rationale: String::from(
                    "If a longer run with a short-trace curriculum still cannot learn the first target token, output-head-only training is too weak for this lane.",
                ),
                planned_changes: vec![
                    String::from(
                        "Unfreeze token embeddings for the next run if boundary_curriculum_v1 and optimizer_budget_v1 do not clear the first-target gate.",
                    ),
                    String::from(
                        "If needed after that, add one small trainable mixer above the deterministic lookup state while keeping the claim boundary next-token-only.",
                    ),
                ],
                success_criteria: vec![
                    String::from(
                        "At least one validation case becomes exact over the full predicted suffix.",
                    ),
                    String::from("Validation first-32-target-token exactness exceeds 9,000 bps."),
                ],
            },
            TassadarExecutorNextRunAction {
                action_id: String::from("phase_gate_v1"),
                priority: 4,
                kind: TassadarExecutorNextRunActionKind::PhaseGate,
                summary: String::from("Hold Phase 10 and Phase 11 behind stronger 4x4 evidence."),
                rationale: String::from(
                    "There is no value in neural hull-cache or 9x9 scale-out while the 4x4 model still fails at target token 0.",
                ),
                planned_changes: vec![
                    String::from(
                        "Do not advance the trained-model hull-cache issue until validation first-target exactness is 100%.",
                    ),
                    String::from(
                        "Do not advance the 9x9 issue until validation first-32-target-token exactness exceeds 9,000 bps and at least one 4x4 validation case is fully exact.",
                    ),
                ],
                success_criteria: vec![
                    String::from(
                        "Phase 10 begins only after the 4x4 lane has passed the boundary gate.",
                    ),
                    String::from(
                        "Phase 11 begins only after the 4x4 lane has passed the short-trace exactness gate.",
                    ),
                ],
            },
        ];
        let mut plan = Self {
            run_id: run_id.to_string(),
            basis_postmortem_digest: postmortem.report_digest.clone(),
            actions,
            plan_digest: String::new(),
        };
        plan.plan_digest = stable_digest(b"psionic_tassadar_executor_next_run_plan|", &plan);
        plan
    }
}

/// Errors while generating the first-run postmortem artifacts.
#[derive(Debug, Error)]
pub enum TassadarExecutorPostmortemError {
    /// Reusing persisted run surfaces failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Reading one persisted artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one persisted artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Serializing one postmortem artifact failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one postmortem artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Generates the canonical first-run postmortem and next-run plan for the reference run.
pub fn augment_tassadar_reference_run_with_postmortem(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorPostmortemError> {
    let run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    let training_telemetry: TassadarExecutorTrainingTelemetryReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_TRAINING_TELEMETRY_FILE),
        "tassadar_training_telemetry",
    )?;
    let divergence: TassadarExecutorTraceDivergenceReport = read_json(
        output_dir.join(TASSADAR_EXECUTOR_TRACE_DIVERGENCE_FILE),
        "tassadar_trace_divergence_report",
    )?;

    let postmortem =
        TassadarExecutorPostmortemReport::new(&run_bundle, &training_telemetry, &divergence);
    let next_run_plan = TassadarExecutorNextRunPlan::new(run_bundle.run_id.as_str(), &postmortem);

    let mut artifact_map = run_bundle
        .artifacts
        .iter()
        .cloned()
        .map(|artifact| (artifact.artifact_ref.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    for artifact in [
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_POSTMORTEM_FILE,
            "tassadar_postmortem",
            &postmortem,
        )?,
        write_json_artifact(
            output_dir,
            TASSADAR_EXECUTOR_NEXT_RUN_PLAN_FILE,
            "tassadar_next_run_plan",
            &next_run_plan,
        )?,
    ] {
        artifact_map.insert(artifact.artifact_ref.clone(), artifact);
    }

    let updated_bundle =
        refresh_bundle_with_artifacts(run_bundle, artifact_map.into_values().collect());
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &updated_bundle,
    )?;
    Ok(updated_bundle)
}

fn refresh_bundle_with_artifacts(
    mut bundle: TassadarExecutorReferenceRunBundle,
    artifacts: Vec<EvalArtifact>,
) -> TassadarExecutorReferenceRunBundle {
    bundle.artifacts = artifacts;
    bundle.bundle_digest.clear();
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_executor_reference_run_bundle|", &bundle);
    bundle
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorPostmortemError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarExecutorPostmortemError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarExecutorPostmortemError::Deserialize {
        artifact_kind: artifact_kind.to_string(),
        path: path.display().to_string(),
        error,
    })
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarExecutorPostmortemError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorPostmortemError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarExecutorPostmortemError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(EvalArtifact::new(
        artifact_kind,
        relative_path,
        bytes.as_slice(),
    ))
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorPostmortemError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorPostmortemError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarExecutorPostmortemError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar executor postmortem value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR, augment_tassadar_reference_run_with_telemetry,
        execute_tassadar_training_run, tassadar_executor_reference_run_config,
    };

    use super::{
        TASSADAR_EXECUTOR_NEXT_RUN_PLAN_FILE, TASSADAR_EXECUTOR_POSTMORTEM_FILE,
        augment_tassadar_reference_run_with_postmortem,
    };

    #[test]
    fn postmortem_augmentation_writes_machine_readable_review_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        execute_tassadar_training_run(
            temp.path(),
            &tassadar_executor_reference_run_config(),
            Some(psionic_data::TassadarSequenceSplit::Validation),
        )?;
        augment_tassadar_reference_run_with_telemetry(temp.path())?;
        let bundle = augment_tassadar_reference_run_with_postmortem(temp.path())?;

        assert!(temp.path().join(TASSADAR_EXECUTOR_POSTMORTEM_FILE).exists());
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_NEXT_RUN_PLAN_FILE)
                .exists()
        );
        assert!(
            bundle
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_ref == TASSADAR_EXECUTOR_NEXT_RUN_PLAN_FILE)
        );
        Ok(())
    }

    #[test]
    fn postmortem_reuses_the_repo_relative_reference_run_constant() {
        assert_eq!(
            TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR,
            "crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0"
        );
    }
}
