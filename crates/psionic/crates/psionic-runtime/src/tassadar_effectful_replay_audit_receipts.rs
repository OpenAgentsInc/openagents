use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_EFFECTFUL_REPLAY_AUDIT_PROFILE_ID: &str =
    "tassadar.effect_profile.replay_challenge_receipts.v1";
pub const TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_effectful_replay_audit_v1";
pub const TASSADAR_EFFECTFUL_REPLAY_AUDIT_BUNDLE_FILE: &str =
    "tassadar_effectful_replay_audit_bundle.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectReceiptArtifact {
    pub receipt_id: String,
    pub effect_family_id: String,
    pub authority_ref: String,
    pub replay_window_steps: u32,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarChallengeReceiptArtifact {
    pub challenge_receipt_id: String,
    pub challenge_surface_id: String,
    pub replay_artifact_digest: String,
    pub verdict: String,
    pub receipt_digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectfulReplayCaseStatus {
    ExactReplayAndChallengeParity,
    ExactRefusalParity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectfulReplayCaseReceipt {
    pub case_id: String,
    pub process_id: String,
    pub effect_surface_id: String,
    pub status: TassadarEffectfulReplayCaseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_receipt: Option<TassadarEffectReceiptArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_receipt: Option<TassadarChallengeReceiptArtifact>,
    pub replay_artifact_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectfulReplayAuditBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub case_receipts: Vec<TassadarEffectfulReplayCaseReceipt>,
    pub challengeable_case_count: u32,
    pub refusal_case_count: u32,
    pub replay_safe_effect_family_ids: Vec<String>,
    pub refused_effect_family_ids: Vec<String>,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectfulReplayAuditBundleError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

#[must_use]
pub fn build_tassadar_effectful_replay_audit_bundle() -> TassadarEffectfulReplayAuditBundle {
    let case_receipts = vec![
        exact_case(
            "seeded_simulator_effect_replay",
            "tassadar.process.search_frontier_kernel.v1",
            "seeded_network_loopback_case",
            "sim.network.loopback_fifo_seed_7.v1",
            "nexus://tassadar/challenge/simulator_loopback",
            256,
            "seeded simulator effects stay replayable and challengeable because both the effect receipt and the challenge verdict are frozen explicitly",
        ),
        exact_case(
            "virtual_fs_proof_replay",
            "tassadar.process.long_loop_kernel.v1",
            "dictionary_scan_read_only_mount",
            "tassadar.effect_profile.virtual_fs_mounts.v1",
            "nexus://tassadar/challenge/virtual_fs_read",
            96,
            "artifact-mounted reads stay replayable and challengeable because the read proof and challenge receipt both bind the same artifact digest",
        ),
        exact_case(
            "async_safe_cancel_replay",
            "tassadar.process.state_machine_accumulator.v1",
            "safe_boundary_cancellation_job",
            "tassadar.internal_compute.async_lifecycle.v1",
            "nexus://tassadar/challenge/async_safe_cancel",
            144,
            "safe-boundary cancellation stays replayable and challengeable because the replay digest and challenge verdict both bind the safe checkpoint boundary",
        ),
        refusal_case(
            "missing_effect_receipt_refusal",
            "tassadar.process.search_frontier_kernel.v1",
            "effect_receipt_missing",
            "missing_effect_receipt",
            "missing effect receipts remain refused because effectful replay truth cannot be challenged without an explicit effect receipt",
        ),
        refusal_case(
            "missing_challenge_receipt_refusal",
            "tassadar.process.long_loop_kernel.v1",
            "challenge_receipt_missing",
            "missing_challenge_receipt",
            "challenge-free effect replay remains refused because effectful replay truth must stay challengeable and audit-ready",
        ),
        refusal_case(
            "unsafe_effect_family_refusal",
            "tassadar.process.external_callback.v1",
            "unsafe_effect_family",
            "unsafe_effect_family_out_of_envelope",
            "unsafe or ambient effect families remain refused because this bundle does not imply arbitrary effectful execution truth",
        ),
    ];
    let challengeable_case_count = case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarEffectfulReplayCaseStatus::ExactReplayAndChallengeParity
        })
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarEffectfulReplayCaseStatus::ExactRefusalParity)
        .count() as u32;
    let replay_safe_effect_family_ids = case_receipts
        .iter()
        .filter_map(|case| {
            case.effect_receipt
                .as_ref()
                .map(|receipt| receipt.effect_family_id.clone())
        })
        .collect::<Vec<_>>();
    let refused_effect_family_ids = vec![
        String::from("challenge_receipt_missing"),
        String::from("effect_receipt_missing"),
        String::from("unsafe_effect_family"),
    ];
    let mut bundle = TassadarEffectfulReplayAuditBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.effectful_replay_audit.bundle.v1"),
        profile_id: String::from(TASSADAR_EFFECTFUL_REPLAY_AUDIT_PROFILE_ID),
        case_receipts,
        challengeable_case_count,
        refusal_case_count,
        replay_safe_effect_family_ids,
        refused_effect_family_ids,
        kernel_policy_dependency_marker: String::from(
            "kernel-policy owns authority on which mediated effects remain admissible for accepted-outcome use",
        ),
        nexus_dependency_marker: String::from(
            "nexus owns challenge and audit coordination for effect-aware replay disputes outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this bundle covers one bounded effectful replay-and-challenge lane with explicit effect receipts, replay digests, and challenge receipts. It keeps missing-effect evidence, missing challenge evidence, and unsafe effect families on explicit refusal paths instead of implying ambient authority closure or market-grade settlement readiness inside standalone psionic",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Effectful replay audit bundle covers challengeable_cases={}, refusal_cases={}, replay_safe_families={}, refused_families={}.",
        bundle.challengeable_case_count,
        bundle.refusal_case_count,
        bundle.replay_safe_effect_family_ids.len(),
        bundle.refused_effect_family_ids.len(),
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_effectful_replay_audit_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_effectful_replay_audit_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF)
        .join(TASSADAR_EFFECTFUL_REPLAY_AUDIT_BUNDLE_FILE)
}

pub fn write_tassadar_effectful_replay_audit_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEffectfulReplayAuditBundle, TassadarEffectfulReplayAuditBundleError> {
    let output_path = output_path.as_ref();
    let (bundle, write_plans) = build_tassadar_effectful_replay_audit_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarEffectfulReplayAuditBundleError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarEffectfulReplayAuditBundleError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectfulReplayAuditBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectfulReplayAuditBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_tassadar_effectful_replay_audit_materialization() -> Result<
    (TassadarEffectfulReplayAuditBundle, Vec<WritePlan>),
    TassadarEffectfulReplayAuditBundleError,
> {
    let bundle = build_tassadar_effectful_replay_audit_bundle();
    let bundle_ref = format!(
        "{}/{}",
        TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF, TASSADAR_EFFECTFUL_REPLAY_AUDIT_BUNDLE_FILE
    );
    let mut write_plans = vec![WritePlan {
        relative_path: bundle_ref,
        bytes: json_bytes(&bundle)?,
    }];
    for case in &bundle.case_receipts {
        if let Some(effect_receipt) = &case.effect_receipt {
            write_plans.push(WritePlan {
                relative_path: format!(
                    "{}/effect_receipts/{}.json",
                    TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF, effect_receipt.receipt_id
                ),
                bytes: json_bytes(effect_receipt)?,
            });
        }
        if let Some(challenge_receipt) = &case.challenge_receipt {
            write_plans.push(WritePlan {
                relative_path: format!(
                    "{}/challenge_receipts/{}.json",
                    TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF,
                    challenge_receipt.challenge_receipt_id
                ),
                bytes: json_bytes(challenge_receipt)?,
            });
        }
    }
    Ok((bundle, write_plans))
}

fn exact_case(
    case_id: &str,
    process_id: &str,
    effect_surface_id: &str,
    effect_family_id: &str,
    challenge_surface_id: &str,
    replay_window_steps: u32,
    note: &str,
) -> TassadarEffectfulReplayCaseReceipt {
    let replay_artifact_digest = stable_digest(
        b"psionic_tassadar_effectful_replay_artifact|",
        &(case_id, process_id, effect_surface_id, replay_window_steps),
    );
    let effect_receipt = TassadarEffectReceiptArtifact {
        receipt_id: format!("{case_id}.effect_receipt"),
        effect_family_id: String::from(effect_family_id),
        authority_ref: String::from("kernel-policy://tassadar/effect_receipt"),
        replay_window_steps,
        receipt_digest: stable_digest(
            b"psionic_tassadar_effect_receipt_artifact|",
            &(case_id, effect_family_id, replay_window_steps),
        ),
    };
    let challenge_receipt = TassadarChallengeReceiptArtifact {
        challenge_receipt_id: format!("{case_id}.challenge_receipt"),
        challenge_surface_id: String::from(challenge_surface_id),
        replay_artifact_digest: replay_artifact_digest.clone(),
        verdict: String::from("challenge_confirmed"),
        receipt_digest: stable_digest(
            b"psionic_tassadar_challenge_receipt_artifact|",
            &(case_id, challenge_surface_id, &replay_artifact_digest),
        ),
    };
    TassadarEffectfulReplayCaseReceipt {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        effect_surface_id: String::from(effect_surface_id),
        status: TassadarEffectfulReplayCaseStatus::ExactReplayAndChallengeParity,
        effect_receipt: Some(effect_receipt),
        challenge_receipt: Some(challenge_receipt),
        replay_artifact_digest,
        refusal_reason_id: None,
        note: String::from(note),
    }
}

fn refusal_case(
    case_id: &str,
    process_id: &str,
    effect_surface_id: &str,
    refusal_reason_id: &str,
    note: &str,
) -> TassadarEffectfulReplayCaseReceipt {
    TassadarEffectfulReplayCaseReceipt {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        effect_surface_id: String::from(effect_surface_id),
        status: TassadarEffectfulReplayCaseStatus::ExactRefusalParity,
        effect_receipt: None,
        challenge_receipt: None,
        replay_artifact_digest: stable_digest(
            b"psionic_tassadar_effectful_replay_refusal_digest|",
            &(case_id, effect_surface_id, refusal_reason_id),
        ),
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TassadarEffectfulReplayAuditBundleError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarEffectfulReplayAuditBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarEffectfulReplayAuditBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEffectfulReplayAuditBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_effectful_replay_audit_bundle, read_json,
        tassadar_effectful_replay_audit_bundle_path, write_tassadar_effectful_replay_audit_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn effectful_replay_audit_bundle_keeps_replay_and_refusal_truth_explicit() {
        let bundle = build_tassadar_effectful_replay_audit_bundle();

        assert_eq!(bundle.challengeable_case_count, 3);
        assert_eq!(bundle.refusal_case_count, 3);
        assert_eq!(bundle.replay_safe_effect_family_ids.len(), 3);
        assert_eq!(bundle.refused_effect_family_ids.len(), 3);
        assert!(bundle
            .kernel_policy_dependency_marker
            .contains("kernel-policy"));
        assert!(bundle.nexus_dependency_marker.contains("nexus"));
    }

    #[test]
    fn effectful_replay_audit_bundle_matches_committed_truth() {
        let generated = build_tassadar_effectful_replay_audit_bundle();
        let committed =
            read_json(tassadar_effectful_replay_audit_bundle_path()).expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_effectful_replay_audit_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_effectful_replay_audit_bundle.json");
        let bundle =
            write_tassadar_effectful_replay_audit_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
    }
}
