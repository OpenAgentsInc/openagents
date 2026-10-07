use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    derive_psion_actual_pretraining_run_shape_qualification, stable_run_shape_observation_digest,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingBaselineToolsBundle,
    PsionActualPretrainingDataBundle, PsionActualPretrainingDataloaderProbe,
    PsionActualPretrainingEvidenceContract, PsionActualPretrainingRunShapeObservation,
    PsionActualPretrainingStorageProbe, PsionActualPretrainingSystemsBundle,
    PsionActualPretrainingThroughputProbe, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_FIXTURE_PATH,
};
use sha2::{Digest, Sha256};

const FIXTURE_RUN_ID: &str = "run-psion-actual-20260402t010000z";
const FIXTURE_GIT_REF: &str = "refs/heads/main";
const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let pretrain_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&pretrain_dir)?;

    let baseline_tools_path =
        pretrain_dir.join("psion_actual_pretraining_baseline_tools_bundle_v1.json");
    let data_path = pretrain_dir.join("psion_actual_pretraining_data_bundle_v1.json");
    let systems_path = pretrain_dir.join("psion_actual_pretraining_systems_bundle_v1.json");
    let evidence_path = pretrain_dir.join("psion_actual_pretraining_evidence_contract_v1.json");

    let baseline_tools_bundle: PsionActualPretrainingBaselineToolsBundle =
        load_json(&baseline_tools_path)?;
    baseline_tools_bundle.validate()?;
    let data_bundle: PsionActualPretrainingDataBundle = load_json(&data_path)?;
    data_bundle.validate()?;
    let systems_bundle: PsionActualPretrainingSystemsBundle = load_json(&systems_path)?;
    systems_bundle.validate()?;
    let evidence_contract: PsionActualPretrainingEvidenceContract = load_json(&evidence_path)?;
    evidence_contract.validate()?;

    let actual_lane_accounting = baseline_tools_bundle
        .resource_accounting_rows
        .iter()
        .find(|row| row.scope_kind == "actual_lane")
        .ok_or_else(|| std::io::Error::other("missing actual_lane accounting row"))?;
    let throughput_anchor = systems_bundle
        .throughput_baselines
        .iter()
        .find(|baseline| baseline.baseline_kind == "trusted_cluster_anchor")
        .ok_or_else(|| {
            std::io::Error::other("missing trusted_cluster_anchor throughput baseline")
        })?;

    let mut observation = PsionActualPretrainingRunShapeObservation {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_SCHEMA_VERSION),
        observation_id: String::from("psion_actual_pretraining_run_shape_observation_admitted_v1"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        observation_kind: String::from("operator_snapshot_fixture"),
        observed_at_utc: String::from("2026-04-02T14:58:00Z"),
        observed_run_root: String::from("/srv/psion_actual_pretraining_runs/run-psion-actual-20260402t010000z"),
        throughput_probe: PsionActualPretrainingThroughputProbe {
            source_receipt_id: throughput_anchor.source_receipt_id.clone(),
            source_receipt_digest: throughput_anchor.source_receipt_digest.clone(),
            observed_tokens_per_second: 301_442,
            observed_step_latency_ms: 224,
            observed_checkpoint_write_throughput_bytes_per_second: 1_596_375_040,
            detail: String::from(
                "Admitted run-shape fixture retains measured throughput above the frozen trusted-cluster anchor floor for the canonical actual lane.",
            ),
        },
        storage_probe: PsionActualPretrainingStorageProbe {
            storage_path: String::from("/srv/psion_actual_pretraining_runs/run-psion-actual-20260402t010000z"),
            available_bytes: 19_790_175_879_168,
            observed_read_bytes_per_second: 3_758_096_384,
            observed_write_bytes_per_second: 2_426_134_528,
            writable: true,
            detail: String::from(
                "Admitted run-shape fixture retains one storage read/write band above the checkpoint and manifest retention floor.",
            ),
        },
        dataloader_probe: PsionActualPretrainingDataloaderProbe {
            dataset_identity: data_bundle.replay_authority.dataset_identity.clone(),
            max_sequence_tokens: data_bundle.replay_authority.max_sequence_tokens,
            planned_optimizer_steps: actual_lane_accounting.optimizer_steps,
            planned_tokens_per_step: actual_lane_accounting.tokens_per_step,
            observed_horizon_steps: actual_lane_accounting.optimizer_steps,
            observed_horizon_tokens: actual_lane_accounting.train_token_budget,
            observed_batches_per_second: 5,
            observed_stall_count: 0,
            deterministic_replay_observed: true,
            detail: String::from(
                "Admitted run-shape fixture retains exact replay and full planned-horizon dataloader coverage for the canonical actual lane.",
            ),
        },
        summary: String::from(
            "Admitted run-shape operator snapshot for the canonical actual-pretraining lane.",
        ),
        observation_digest: String::new(),
    };
    observation.observation_digest = stable_run_shape_observation_digest(&observation)?;
    observation.validate()?;

    let observation_path = root.join(PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_FIXTURE_PATH);
    fs::write(
        &observation_path,
        serde_json::to_string_pretty(&observation)?,
    )?;

    let qualification = derive_psion_actual_pretraining_run_shape_qualification(
        FIXTURE_RUN_ID,
        FIXTURE_GIT_REF,
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        &observation,
        Some(artifact_ref(&root, &observation_path)?),
        artifact_ref(&root, &baseline_tools_path)?,
        artifact_ref(&root, &data_path)?,
        artifact_ref(&root, &systems_path)?,
        artifact_ref(&root, &evidence_path)?,
        &baseline_tools_bundle,
        &data_bundle,
        &systems_bundle,
        &evidence_contract,
    )?;

    fs::write(
        root.join(PSION_ACTUAL_PRETRAINING_RUN_SHAPE_QUALIFICATION_FIXTURE_PATH),
        serde_json::to_string_pretty(&qualification)?,
    )?;
    Ok(())
}

fn load_json<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    Ok(PsionActualPretrainingArtifactRef {
        path: path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/"),
        sha256: file_sha256(path)?,
    })
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(sha256_hex(&fs::read(path)?))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
