use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH, TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH,
    TassadarDefaultTrainLaneContractError, builtin_tassadar_default_train_lane_contract,
};

/// Stable schema version for the canonical Tassadar launch manifest.
pub const TASSADAR_TRAIN_LAUNCH_MANIFEST_SCHEMA_VERSION: &str = "tassadar.train_launch_manifest.v1";

/// Stable schema version for the canonical Tassadar current-run status surface.
pub const TASSADAR_TRAIN_CURRENT_RUN_STATUS_SCHEMA_VERSION: &str =
    "tassadar.train_current_run_status.v1";

/// Stable schema version for the canonical Tassadar retained summary surface.
pub const TASSADAR_TRAIN_RETAINED_SUMMARY_SCHEMA_VERSION: &str =
    "tassadar.train_retained_summary.v1";

/// Stable fixture path for the canonical Tassadar launch manifest.
pub const TASSADAR_TRAIN_LAUNCH_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_train_launch_manifest_v1.json";

/// Stable fixture path for the canonical Tassadar current-run status surface.
pub const TASSADAR_TRAIN_CURRENT_RUN_STATUS_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_train_current_run_status_v1.json";

/// Stable fixture path for the canonical Tassadar retained summary surface.
pub const TASSADAR_TRAIN_RETAINED_SUMMARY_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_train_retained_summary_v1.json";

const DEFAULT_RUN_ROOT_FAMILY: &str = "tassadar_operator_runs";
const DEFAULT_RUN_ID: &str = "run-tassadar-20260402t200000z";
const FIXTURE_EXAMPLE_ROOT: &str = "fixtures/tassadar/operator/tassadar_train_launcher_example";
const STATUS_SURFACE_ID: &str = "tassadar_train_status_surface_v1";

/// One launcher-supported Tassadar lane with explicit checker and output truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainLauncherLaneSpec {
    /// Stable lane identifier.
    pub lane_id: String,
    /// Short operator-facing lane label.
    pub lane_label: String,
    /// Whether this lane is the canonical default.
    pub is_default: bool,
    /// Launcher command that executes the lane.
    pub launch_command: String,
    /// Frozen checker command for the lane.
    pub checker_command: String,
    /// Retained lane output root already owned by the repo.
    pub lane_output_root_family: String,
    /// Stable evidence family for the lane.
    pub evidence_family: String,
    /// Promotion target named by the lane.
    pub promotion_target: String,
}

/// Current launcher phase for one retained Tassadar operator run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTrainLauncherPhase {
    /// Dry-run contract was staged without execution.
    DryRunPlanned,
    /// Start contract was staged with an admitted launch command.
    LaunchStaged,
}

impl TassadarTrainLauncherPhase {
    fn launcher_surface_id(self) -> &'static str {
        match self {
            Self::DryRunPlanned => "tassadar_train_dry_run",
            Self::LaunchStaged => "tassadar_train_start",
        }
    }
}

/// Retained launch manifest for the canonical Tassadar operator launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainLaunchManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Launcher surface id.
    pub launcher_surface_id: String,
    /// Launcher path.
    pub launcher_path: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Repo-relative run root.
    pub run_root: String,
    /// Selected lane specification.
    pub selected_lane: TassadarTrainLauncherLaneSpec,
    /// Relative current-status ref under the run root.
    pub current_status_ref: String,
    /// Relative retained-summary ref under the run root.
    pub retained_summary_ref: String,
    /// Claim boundary for the launcher output.
    pub claim_boundary: String,
}

/// Current run status for the canonical Tassadar operator launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainCurrentRunStatus {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable status surface id.
    pub status_surface_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Repo-relative run root.
    pub run_root: String,
    /// Selected lane identifier.
    pub lane_id: String,
    /// Current launcher phase.
    pub phase: TassadarTrainLauncherPhase,
    /// Frozen checker command for the run.
    pub checker_command: String,
    /// Relative retained-summary ref under the run root.
    pub retained_summary_ref: String,
    /// Claim boundary for the status surface.
    pub claim_boundary: String,
}

/// Retained summary for the canonical Tassadar operator launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainRetainedSummary {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Repo-relative run root.
    pub run_root: String,
    /// Selected lane specification.
    pub selected_lane: TassadarTrainLauncherLaneSpec,
    /// Last launcher surface used to materialize this summary.
    pub last_launcher_surface_id: String,
    /// Relative launch-manifest ref under the run root.
    pub launch_manifest_ref: String,
    /// Relative current-status ref under the run root.
    pub current_status_ref: String,
    /// Claim boundary for the retained summary.
    pub claim_boundary: String,
}

/// Full launcher output set written under one retained run root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarTrainLauncherOutputSet {
    /// Persisted launch manifest.
    pub launch_manifest: TassadarTrainLaunchManifest,
    /// Persisted current run status.
    pub current_status: TassadarTrainCurrentRunStatus,
    /// Persisted retained summary.
    pub retained_summary: TassadarTrainRetainedSummary,
}

/// Errors returned by the canonical Tassadar operator launcher.
#[derive(Debug, Error)]
pub enum TassadarTrainLauncherError {
    #[error("unsupported Tassadar launcher command `{command}`")]
    UnsupportedCommand { command: String },
    #[error("unsupported Tassadar lane `{lane_id}`")]
    UnsupportedLane { lane_id: String },
    #[error("missing required `--run-root` for status command")]
    MissingRunRootForStatus,
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    DefaultLane(#[from] TassadarDefaultTrainLaneContractError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Returns the launcher-supported Tassadar lanes grounded in retained repo truth.
pub fn builtin_tassadar_train_launcher_lane_specs(
    workspace_root: &Path,
) -> Result<Vec<TassadarTrainLauncherLaneSpec>, TassadarTrainLauncherError> {
    let default_contract = builtin_tassadar_default_train_lane_contract(workspace_root)?;
    Ok(vec![
        TassadarTrainLauncherLaneSpec {
            lane_id: default_contract.lane_id,
            lane_label: String::from("trace-bound trained-v0 default"),
            is_default: true,
            launch_command: String::from(
                "cargo run -q -p psionic-train --example tassadar_article_transformer_weight_production",
            ),
            checker_command: String::from(TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH),
            lane_output_root_family: default_contract.run_root_family,
            evidence_family: default_contract.evidence_family,
            promotion_target: default_contract.promotion_target_model_id,
        },
        TassadarTrainLauncherLaneSpec {
            lane_id: String::from("tassadar_hungarian_10x10_article_learned_v0"),
            lane_label: String::from("Hungarian-10x10 learned article"),
            is_default: false,
            launch_command: String::from(
                "cargo run -q -p psionic-train --example tassadar_hungarian_10x10_article_learned_run",
            ),
            checker_command: String::from("scripts/check-tassadar-acceptance.sh"),
            lane_output_root_family: String::from(
                "fixtures/tassadar/runs/hungarian_10x10_v0_learned_article_executor_v0",
            ),
            evidence_family: String::from("train.tassadar.executor_transformer"),
            promotion_target: String::from("tassadar-executor-transformer-hungarian-10x10-v0"),
        },
        TassadarTrainLauncherLaneSpec {
            lane_id: String::from("tassadar_sudoku_v0_promotion_v3"),
            lane_label: String::from("Sudoku 4x4 learned promotion"),
            is_default: false,
            launch_command: String::from(
                "cargo run -q -p psionic-research --example tassadar_executor_attention_promotion_run",
            ),
            checker_command: String::from(
                "scripts/check-tassadar-4x4-promotion-gate.sh fixtures/tassadar/runs/sudoku_v0_promotion_v3",
            ),
            lane_output_root_family: String::from("fixtures/tassadar/runs/sudoku_v0_promotion_v3"),
            evidence_family: String::from("train.tassadar.executor_transformer"),
            promotion_target: String::from("tassadar-executor-transformer-sudoku-v0"),
        },
    ])
}

/// Materializes retained launcher outputs for one start or dry-run command.
pub fn write_tassadar_train_launcher_outputs(
    workspace_root: &Path,
    phase: TassadarTrainLauncherPhase,
    lane_id: Option<&str>,
    run_root: &Path,
) -> Result<TassadarTrainLauncherOutputSet, TassadarTrainLauncherError> {
    let output = build_tassadar_train_launcher_outputs(workspace_root, phase, lane_id, run_root)?;
    write_json(
        run_root.join("manifests/launch_manifest.json"),
        &output.launch_manifest,
    )?;
    write_json(
        run_root.join("status/current_run_status.json"),
        &output.current_status,
    )?;
    write_json(
        run_root.join("status/retained_summary.json"),
        &output.retained_summary,
    )?;
    Ok(output)
}

fn build_tassadar_train_launcher_outputs(
    workspace_root: &Path,
    phase: TassadarTrainLauncherPhase,
    lane_id: Option<&str>,
    run_root: &Path,
) -> Result<TassadarTrainLauncherOutputSet, TassadarTrainLauncherError> {
    let specs = builtin_tassadar_train_launcher_lane_specs(workspace_root)?;
    let selected_lane = select_lane(specs.as_slice(), lane_id)?.clone();
    let run_root_relative = repo_relative_path(workspace_root, run_root);
    let launch_manifest = TassadarTrainLaunchManifest {
        schema_version: String::from(TASSADAR_TRAIN_LAUNCH_MANIFEST_SCHEMA_VERSION),
        launcher_surface_id: String::from(phase.launcher_surface_id()),
        launcher_path: String::from(TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH),
        run_id: run_root
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from(DEFAULT_RUN_ID)),
        run_root: run_root_relative.clone(),
        selected_lane: selected_lane.clone(),
        current_status_ref: String::from("status/current_run_status.json"),
        retained_summary_ref: String::from("status/retained_summary.json"),
        claim_boundary: String::from(
            "The Tassadar launcher stages one explicit lane selection, checker path, and retained status surface under the operator run root. It does not by itself prove that the selected lane already executed or replaced other historical Tassadar lanes.",
        ),
    };
    let current_status = TassadarTrainCurrentRunStatus {
        schema_version: String::from(TASSADAR_TRAIN_CURRENT_RUN_STATUS_SCHEMA_VERSION),
        status_surface_id: String::from(STATUS_SURFACE_ID),
        run_id: launch_manifest.run_id.clone(),
        run_root: run_root_relative.clone(),
        lane_id: selected_lane.lane_id.clone(),
        phase,
        checker_command: selected_lane.checker_command.clone(),
        retained_summary_ref: String::from("status/retained_summary.json"),
        claim_boundary: String::from(
            "The current status records the chosen Tassadar lane, checker path, and operator phase. It does not claim that the benchmark, promotion, or article-parity surfaces already changed.",
        ),
    };
    let retained_summary = TassadarTrainRetainedSummary {
        schema_version: String::from(TASSADAR_TRAIN_RETAINED_SUMMARY_SCHEMA_VERSION),
        run_id: launch_manifest.run_id.clone(),
        run_root: run_root_relative,
        selected_lane,
        last_launcher_surface_id: String::from(phase.launcher_surface_id()),
        launch_manifest_ref: String::from("manifests/launch_manifest.json"),
        current_status_ref: String::from("status/current_run_status.json"),
        claim_boundary: String::from(
            "The retained summary records which Tassadar lane the operator selected, where its retained repo-owned output family lives, and which checker command governs it. It does not collapse every historical Tassadar lane into one implementation path.",
        ),
    };
    Ok(TassadarTrainLauncherOutputSet {
        launch_manifest,
        current_status,
        retained_summary,
    })
}

/// Reads the retained summary for one existing launcher run root.
pub fn read_tassadar_train_retained_summary(
    run_root: &Path,
) -> Result<TassadarTrainRetainedSummary, TassadarTrainLauncherError> {
    read_json(run_root.join("status/retained_summary.json"))
}

/// Writes the committed launcher fixtures plus one start and one dry-run example.
pub fn write_tassadar_train_launcher_fixtures(
    workspace_root: &Path,
) -> Result<TassadarTrainLauncherOutputSet, TassadarTrainLauncherError> {
    let fixture_run_root = workspace_root
        .join(DEFAULT_RUN_ROOT_FAMILY)
        .join(DEFAULT_RUN_ID);
    let fixture_output = build_tassadar_train_launcher_outputs(
        workspace_root,
        TassadarTrainLauncherPhase::LaunchStaged,
        None,
        fixture_run_root.as_path(),
    )?;

    write_json(
        workspace_root.join(TASSADAR_TRAIN_LAUNCH_MANIFEST_FIXTURE_PATH),
        &fixture_output.launch_manifest,
    )?;
    write_json(
        workspace_root.join(TASSADAR_TRAIN_CURRENT_RUN_STATUS_FIXTURE_PATH),
        &fixture_output.current_status,
    )?;
    write_json(
        workspace_root.join(TASSADAR_TRAIN_RETAINED_SUMMARY_FIXTURE_PATH),
        &fixture_output.retained_summary,
    )?;

    let start_example_root = workspace_root
        .join(FIXTURE_EXAMPLE_ROOT)
        .join("start")
        .join(DEFAULT_RUN_ID);
    write_tassadar_train_launcher_outputs(
        workspace_root,
        TassadarTrainLauncherPhase::LaunchStaged,
        None,
        start_example_root.as_path(),
    )?;

    let dry_run_example_root = workspace_root
        .join(FIXTURE_EXAMPLE_ROOT)
        .join("dry-run")
        .join(DEFAULT_RUN_ID);
    write_tassadar_train_launcher_outputs(
        workspace_root,
        TassadarTrainLauncherPhase::DryRunPlanned,
        Some("tassadar_sudoku_v0_promotion_v3"),
        dry_run_example_root.as_path(),
    )?;

    Ok(fixture_output)
}

fn select_lane<'a>(
    specs: &'a [TassadarTrainLauncherLaneSpec],
    lane_id: Option<&str>,
) -> Result<&'a TassadarTrainLauncherLaneSpec, TassadarTrainLauncherError> {
    if let Some(lane_id) = lane_id {
        return specs
            .iter()
            .find(|spec| spec.lane_id == lane_id)
            .ok_or_else(|| TassadarTrainLauncherError::UnsupportedLane {
                lane_id: String::from(lane_id),
            });
    }
    specs.iter().find(|spec| spec.is_default).ok_or_else(|| {
        TassadarTrainLauncherError::UnsupportedLane {
            lane_id: String::from("<default>"),
        }
    })
}

fn repo_relative_path(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<(), TassadarTrainLauncherError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarTrainLauncherError::Write {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contents = serde_json::to_vec_pretty(value)?;
    fs::write(&path, contents).map_err(|error| TassadarTrainLauncherError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, TassadarTrainLauncherError> {
    let contents = fs::read_to_string(&path).map_err(|error| TassadarTrainLauncherError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_str(&contents).map_err(|error| TassadarTrainLauncherError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        TASSADAR_TRAIN_CURRENT_RUN_STATUS_FIXTURE_PATH,
        TASSADAR_TRAIN_LAUNCH_MANIFEST_FIXTURE_PATH, TASSADAR_TRAIN_RETAINED_SUMMARY_FIXTURE_PATH,
        TassadarTrainLauncherPhase, builtin_tassadar_train_launcher_lane_specs,
        read_tassadar_train_retained_summary, write_tassadar_train_launcher_outputs,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn tassadar_launcher_supports_expected_lane_ids() {
        let specs = builtin_tassadar_train_launcher_lane_specs(workspace_root().as_path())
            .expect("lane specs");
        let lane_ids = specs
            .into_iter()
            .map(|spec| spec.lane_id)
            .collect::<Vec<_>>();
        assert_eq!(
            lane_ids,
            vec![
                "tassadar_article_transformer_trace_bound_trained_v0",
                "tassadar_hungarian_10x10_article_learned_v0",
                "tassadar_sudoku_v0_promotion_v3",
            ]
        );
    }

    #[test]
    fn tassadar_launcher_writes_retained_summary() {
        let root = tempfile::tempdir().expect("tempdir");
        let output = write_tassadar_train_launcher_outputs(
            workspace_root().as_path(),
            TassadarTrainLauncherPhase::LaunchStaged,
            Some("tassadar_sudoku_v0_promotion_v3"),
            root.path().join("run-tassadar-test").as_path(),
        )
        .expect("launcher outputs");
        let summary =
            read_tassadar_train_retained_summary(root.path().join("run-tassadar-test").as_path())
                .expect("retained summary");
        assert_eq!(summary, output.retained_summary);
        assert_eq!(
            summary.selected_lane.lane_id,
            "tassadar_sudoku_v0_promotion_v3"
        );
    }

    #[test]
    fn tassadar_launcher_fixture_paths_stay_stable() {
        assert_eq!(
            TASSADAR_TRAIN_LAUNCH_MANIFEST_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_train_launch_manifest_v1.json"
        );
        assert_eq!(
            TASSADAR_TRAIN_CURRENT_RUN_STATUS_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_train_current_run_status_v1.json"
        );
        assert_eq!(
            TASSADAR_TRAIN_RETAINED_SUMMARY_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_train_retained_summary_v1.json"
        );
    }
}
