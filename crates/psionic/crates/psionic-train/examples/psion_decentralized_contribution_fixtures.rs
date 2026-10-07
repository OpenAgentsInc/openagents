use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    DecentralizedAdapterReferenceProgramSpec, PsionAcceptanceMatrix,
    PsionContributionAcceptanceBinding, PsionContributionArtifactReference,
    PsionContributionCapabilityBinding, PsionDecentralizedContributionBundle, PsionPhaseGate,
    PsionPromotionDecisionReceipt, psion_contributor_receipt_summaries,
    record_psion_decentralized_contribution_bundle,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_dir = repo_root.join("fixtures/psion/decentralized");
    fs::create_dir_all(&fixtures_dir)?;

    let reference_program = psionic_train::run_decentralized_adapter_reference_program(
        &DecentralizedAdapterReferenceProgramSpec::open_default(),
    )?;
    let acceptance_matrix: PsionAcceptanceMatrix =
        load_json(repo_root.join("fixtures/psion/acceptance/psion_acceptance_matrix_v1.json"))?;
    let promotion_decision: PsionPromotionDecisionReceipt = load_json(
        repo_root.join("fixtures/psion/acceptance/psion_promotion_decision_receipt_v1.json"),
    )?;
    let capability_matrix: Value =
        load_json(repo_root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"))?;

    let trusted_cluster_run_bytes = fs::read(
        repo_root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json"),
    )?;
    let reasoning_sft_run_bytes =
        fs::read(repo_root.join("fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json"))?;
    let rollback_receipt_bytes = fs::read(
        repo_root
            .join("fixtures/psion/withdrawal/psion_capability_withdrawal_route_regression_v1.json"),
    )?;

    let bundle = record_psion_decentralized_contribution_bundle(
        "psion-decentralized-contribution-bundle-v1",
        artifact_ref(
            "fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json",
            &trusted_cluster_run_bytes,
        ),
        artifact_ref(
            "fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json",
            &reasoning_sft_run_bytes,
        ),
        PsionContributionAcceptanceBinding {
            acceptance_matrix_id: acceptance_matrix.matrix_id.clone(),
            acceptance_matrix_version: acceptance_matrix.matrix_version.clone(),
            current_promoted_decision_ref: promotion_decision.decision_id.clone(),
            current_promoted_phase: promotion_decision.phase,
            required_publication_phase: PsionPhaseGate::InternalServing,
            detail: String::from(
                "Contributed adapter-window outputs remain bounded local policy revisions until they clear the same Psion acceptance discipline required for internal serving or later publication.",
            ),
        },
        PsionContributionCapabilityBinding {
            capability_matrix_id: string_field(&capability_matrix, "matrix_id")?,
            capability_matrix_version: string_field(&capability_matrix, "matrix_version")?,
            rollback_receipt_schema_version: String::from("psion.capability_withdrawal_receipt.v1"),
            rollback_reference_receipt: artifact_ref(
                "fixtures/psion/withdrawal/psion_capability_withdrawal_route_regression_v1.json",
                &rollback_receipt_bytes,
            ),
            detail: String::from(
                "Any contributed output that reaches service still inherits the same capability-matrix and rollback discipline as the main Psion lane instead of bypassing downgrade history.",
            ),
        },
        reference_program.clone(),
        psion_contributor_receipt_summaries(&reference_program),
        "This bundle freezes one bounded decentralized adapter-delta contribution lane over the existing cluster-backed window substrate. It keeps contributor receipts, security receipts, replay-checked windows, and local policy promotion explicit. It does not claim arbitrary public full-model all-reduce, arbitrary public synchronous training, or automatic served promotion of contributed outputs.",
        "First bounded Psion decentralized contribution bundle tying the adapter-window reference substrate back to trusted-cluster, reasoning-SFT, acceptance, capability, and rollback discipline.",
    )?;

    write_bundle(
        fixtures_dir.join("psion_decentralized_contribution_bundle_v1.json"),
        &bundle,
    )?;
    Ok(())
}

fn artifact_ref(artifact_id: &str, bytes: &[u8]) -> PsionContributionArtifactReference {
    PsionContributionArtifactReference {
        artifact_id: String::from(artifact_id),
        artifact_digest: stable_digest(bytes),
    }
}

fn stable_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_decentralized_contribution_fixture|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn string_field(value: &Value, key: &str) -> Result<String, Box<dyn Error>> {
    value[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("missing string field `{key}`").into())
}

fn load_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_bundle(
    path: PathBuf,
    bundle: &PsionDecentralizedContributionBundle,
) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string_pretty(bundle)?)?;
    Ok(())
}
