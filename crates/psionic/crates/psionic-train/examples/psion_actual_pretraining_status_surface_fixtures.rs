use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_START_SURFACE_ID, PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID,
    PsionActualPretrainingCurrentRunStatus, PsionActualPretrainingLauncherSurfaces,
    PsionActualPretrainingRetainedSummary,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let run_id = "run-psion-actual-20260402t000000z";
    let surfaces = PsionActualPretrainingLauncherSurfaces {
        start_surface_id: String::from(PSION_ACTUAL_PRETRAINING_START_SURFACE_ID),
        dry_run_surface_id: String::from(PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID),
        resume_surface_id: String::from(PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID),
        status_surface_id: String::from(PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID),
    };

    let current_status = PsionActualPretrainingCurrentRunStatus {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        phase: String::from("running_pretrain"),
        current_status_path: String::from("status/current_run_status.json"),
        retained_summary_path: String::from("status/retained_summary.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        latest_checkpoint_label: String::from("actual-pretrain-step-4096"),
        last_completed_step: 4096,
        launcher_surfaces: surfaces.clone(),
        updated_at_utc: String::from("2026-04-02T13:30:00Z"),
        detail: String::from(
            "Current run status freezes the last known phase, latest accepted checkpoint pointer path, and the reserved launcher surface ids for the actual lane.",
        ),
    };
    current_status.validate()?;

    let retained_summary = PsionActualPretrainingRetainedSummary {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        last_known_phase: String::from("running_pretrain"),
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from("8da3940157d70db8a6ea170fe7a5b598d8933c77"),
        dirty_tree_admission: String::from("refuse_by_default"),
        current_status_path: String::from("status/current_run_status.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        launcher_surfaces: surfaces,
        claim_boundary: String::from(
            "The retained summary records actual-lane start, dry-run, resume, and status surfaces plus the last known operator state. It does not claim that cluster execution, automatic eval, or durable backup are finished.",
        ),
        detail: String::from(
            "Retained summary freezes the launcher contract names and the minimal last-known operator state the actual launcher writes.",
        ),
    };
    retained_summary.validate()?;

    fs::write(
        fixtures_dir.join("psion_actual_pretraining_current_run_status_v1.json"),
        serde_json::to_string_pretty(&current_status)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_actual_pretraining_retained_summary_v1.json"),
        serde_json::to_string_pretty(&retained_summary)?,
    )?;

    let example_root = fixtures_dir
        .join("psion_actual_pretraining_status_surface_example")
        .join(run_id)
        .join("status");
    fs::create_dir_all(&example_root)?;
    fs::write(
        example_root.join("current_run_status.json"),
        serde_json::to_string_pretty(&current_status)?,
    )?;
    fs::write(
        example_root.join("retained_summary.json"),
        serde_json::to_string_pretty(&retained_summary)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
