use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    PsionActualPretrainingLaneSpec, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_LANE_SPEC_SCHEMA_VERSION,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let trusted_cluster_run: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json"),
    )?)?;

    let lane_spec = PsionActualPretrainingLaneSpec {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_LANE_SPEC_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        stage_program_id: String::from("psion_actual_pretraining_program_v1"),
        training_run_profile: String::from("broader_pretraining"),
        run_root_family: String::from("psion_actual_pretraining_runs/<run_id>"),
        evidence_family: String::from("psion.actual_pretraining.evidence.v1"),
        bounded_reference_lane_id: String::from("psion_accelerated_reference_pilot"),
        anchor_run_bundle_ref: String::from(
            "fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json",
        ),
        anchor_run_bundle_digest: trusted_cluster_run["bundle_digest"]
            .as_str()
            .ok_or("trusted cluster bundle digest missing")?
            .to_string(),
        summary: String::from(
            "The canonical actual Psion pretraining lane is the broader_pretraining lane anchored to the bounded trusted-cluster run bundle and kept explicitly distinct from the accelerator-backed reference pilot.",
        ),
    };
    lane_spec.validate()?;

    fs::write(
        fixtures_dir.join("psion_actual_pretraining_lane_spec_v1.json"),
        serde_json::to_string_pretty(&lane_spec)?,
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
