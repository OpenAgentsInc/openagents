use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    derive_psion_actual_pretraining_hardware_qualification, stable_hardware_observation_digest,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingHardwareObservation,
    PsionActualPretrainingObservedCredentialSource, PsionActualPretrainingObservedWorker,
    PsionActualPretrainingSystemsBundle, PsionActualPretrainingTopologyStorageBundle,
    PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_LANE_ID,
};
use sha2::{Digest, Sha256};

const FIXTURE_RUN_ID: &str = "run-psion-actual-20260402t010000z";
const FIXTURE_GIT_REF: &str = "refs/heads/main";
const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let pretrain_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&pretrain_dir)?;

    let topology_path = pretrain_dir.join("psion_actual_pretraining_topology_storage_bundle_v1.json");
    let systems_path = pretrain_dir.join("psion_actual_pretraining_systems_bundle_v1.json");
    let evidence_path = pretrain_dir.join("psion_actual_pretraining_evidence_contract_v1.json");

    let topology: PsionActualPretrainingTopologyStorageBundle = load_json(&topology_path)?;
    topology.validate()?;
    let systems_bundle: PsionActualPretrainingSystemsBundle = load_json(&systems_path)?;
    systems_bundle.validate()?;
    let evidence_contract: PsionActualPretrainingEvidenceContract = load_json(&evidence_path)?;
    evidence_contract.validate()?;

    let mut observation = PsionActualPretrainingHardwareObservation {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_SCHEMA_VERSION),
        observation_id: String::from("psion_actual_pretraining_hardware_observation_admitted_v1"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        observation_kind: String::from("operator_snapshot_fixture"),
        observed_at_utc: String::from("2026-04-02T14:55:00Z"),
        backend: String::from("cuda"),
        workers: vec![
            worker("h100-node-1", 73_014_444_032, 54),
            worker("h100-node-2", 72_477_573_120, 55),
            worker("h100-node-3", 71_618_830_336, 56),
            worker("h100-node-4", 70_759_911_424, 57),
        ],
        credential_sources: vec![
            credential(
                "PSION_ACTUAL_PRETRAINING_GCP_PROJECT_ID",
                "environment_variable",
                "psion-pretrain-prod-project",
            ),
            credential(
                "PSION_ACTUAL_PRETRAINING_BUCKET_URL",
                "environment_variable",
                "gs://psion-actual-pretraining",
            ),
            credential(
                "GOOGLE_APPLICATION_CREDENTIALS",
                "secret_file_env",
                "{\"client_email\":\"psion-pretrain@project.iam.gserviceaccount.com\"}",
            ),
        ],
        checkpoint_restore_ready: true,
        summary: String::from(
            "Admitted four-node H100 operator snapshot for the canonical actual-pretraining lane.",
        ),
        observation_digest: String::new(),
    };
    observation.observation_digest = stable_hardware_observation_digest(&observation)?;
    observation.validate()?;

    let observation_path = root.join(PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_FIXTURE_PATH);
    fs::write(&observation_path, serde_json::to_string_pretty(&observation)?)?;

    let qualification = derive_psion_actual_pretraining_hardware_qualification(
        FIXTURE_RUN_ID,
        FIXTURE_GIT_REF,
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        &observation,
        Some(artifact_ref(&root, &observation_path)?),
        artifact_ref(&root, &topology_path)?,
        artifact_ref(&root, &systems_path)?,
        artifact_ref(&root, &evidence_path)?,
        &topology,
        &systems_bundle,
        &evidence_contract,
    )?;

    fs::write(
        root.join(PSION_ACTUAL_PRETRAINING_HARDWARE_QUALIFICATION_FIXTURE_PATH),
        serde_json::to_string_pretty(&qualification)?,
    )?;
    Ok(())
}

fn worker(
    worker_label: &str,
    free_memory_bytes: u64,
    temperature_celsius: u64,
) -> PsionActualPretrainingObservedWorker {
    PsionActualPretrainingObservedWorker {
        worker_label: String::from(worker_label),
        backend: String::from("cuda"),
        device_name: String::from("NVIDIA H100 80GB HBM3"),
        total_memory_bytes: 85_899_345_920,
        free_memory_bytes,
        temperature_celsius: Some(temperature_celsius),
        ecc_uncorrected_error_count: Some(0),
        throttling_observed: Some(false),
        resident_compute_process_count: Some(0),
        mig_partitioned: false,
        detail: String::from(
            "Admitted H100 worker snapshot retained for actual-lane launch qualification.",
        ),
    }
}

fn credential(
    source_name: &str,
    kind: &str,
    redacted_value: &str,
) -> PsionActualPretrainingObservedCredentialSource {
    PsionActualPretrainingObservedCredentialSource {
        source_name: String::from(source_name),
        kind: String::from(kind),
        present: true,
        redacted_digest: Some(sha256_hex(redacted_value.as_bytes())),
        detail: String::from(
            "Credential source is retained only by declared name plus redacted digest.",
        ),
    }
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
