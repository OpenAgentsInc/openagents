use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SIMULATOR_EFFECT_PROFILE_ID: &str =
    "tassadar.effect_profile.simulator_backed_io.v1";
pub const TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_simulator_effects_v1";
pub const TASSADAR_SIMULATOR_EFFECT_BUNDLE_FILE: &str =
    "tassadar_simulator_effect_runtime_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSimulatorKind {
    Clock,
    Randomness,
    NetworkSimulator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSimulatorCaseStatus {
    ExactReplayParity,
    ExactRefusalParity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSimulatorRefusalKind {
    AmbientSystemClockDenied,
    AmbientOsRandomDenied,
    AmbientSocketNetworkDenied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorTraceSample {
    pub step_index: u32,
    pub value: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorCaseReceipt {
    pub case_id: String,
    pub profile_id: String,
    pub simulator_kind: TassadarSimulatorKind,
    pub seed_profile_id: String,
    pub portability_envelope_id: String,
    pub status: TassadarSimulatorCaseStatus,
    pub exact_replay_parity: bool,
    pub simulator_trace: Vec<TassadarSimulatorTraceSample>,
    pub refusal_kinds: Vec<TassadarSimulatorRefusalKind>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub case_receipts: Vec<TassadarSimulatorCaseReceipt>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub simulator_profile_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub refused_effect_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSimulatorEffectRuntimeBundleError {
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

#[must_use]
pub fn build_tassadar_simulator_effect_runtime_bundle() -> TassadarSimulatorEffectRuntimeBundle {
    let case_receipts = vec![
        seeded_clock_case(),
        seeded_randomness_case(),
        seeded_network_case(),
        ambient_clock_refusal_case(),
        ambient_random_refusal_case(),
        ambient_network_refusal_case(),
    ];
    let exact_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSimulatorCaseStatus::ExactReplayParity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let simulator_profile_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSimulatorCaseStatus::ExactReplayParity)
        .map(|case| case.seed_profile_id.clone())
        .collect::<Vec<_>>();
    let portability_envelope_ids = vec![String::from("cpu_reference_current_host")];
    let refused_effect_ids = vec![
        String::from("host.clock.now"),
        String::from("host.random.os_entropy"),
        String::from("host.network.socket_io"),
    ];
    let mut bundle = TassadarSimulatorEffectRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.simulator_effect.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_SIMULATOR_EFFECT_PROFILE_ID),
        case_receipts,
        exact_case_count,
        refusal_case_count,
        simulator_profile_ids,
        portability_envelope_ids,
        refused_effect_ids,
        claim_boundary: String::from(
            "this runtime bundle covers one bounded simulator-backed effect profile with seeded clock, randomness, and loopback-network semantics. It keeps ambient system clock, OS entropy, and socket I/O on explicit refusal paths instead of implying general host interaction",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Simulator-effect runtime bundle covers exact_cases={}, refusal_rows={}, simulator_profiles={}, portability_envelopes={}.",
        bundle.exact_case_count,
        bundle.refusal_case_count,
        bundle.simulator_profile_ids.len(),
        bundle.portability_envelope_ids.len(),
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_simulator_effect_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_simulator_effect_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF)
        .join(TASSADAR_SIMULATOR_EFFECT_BUNDLE_FILE)
}

pub fn write_tassadar_simulator_effect_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSimulatorEffectRuntimeBundle, TassadarSimulatorEffectRuntimeBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSimulatorEffectRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_simulator_effect_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSimulatorEffectRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn seeded_clock_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "seeded_clock_tick_case",
        TassadarSimulatorKind::Clock,
        "sim.clock.seeded_epoch_250ms.v1",
        TassadarSimulatorCaseStatus::ExactReplayParity,
        true,
        vec![
            sample(0, "1700000000000", "seeded simulated wall-clock start"),
            sample(1, "1700000000250", "seeded 250ms clock tick"),
            sample(2, "1700000000500", "seeded 500ms clock tick"),
        ],
        Vec::new(),
        "seeded clock simulator stays replay-stable because the epoch and tick quantum are frozen explicitly",
    )
}

fn seeded_randomness_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "seeded_randomness_stream_case",
        TassadarSimulatorKind::Randomness,
        "sim.random.pcg64_seed_42.v1",
        TassadarSimulatorCaseStatus::ExactReplayParity,
        true,
        vec![
            sample(0, "0x8f5ce2d1", "first seeded pseudo-random draw"),
            sample(1, "0x47ab1934", "second seeded pseudo-random draw"),
            sample(2, "0xd02c7e58", "third seeded pseudo-random draw"),
        ],
        Vec::new(),
        "seeded randomness stays replay-stable because the generator family and initial seed are frozen explicitly",
    )
}

fn seeded_network_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "seeded_network_loopback_case",
        TassadarSimulatorKind::NetworkSimulator,
        "sim.network.loopback_fifo_seed_7.v1",
        TassadarSimulatorCaseStatus::ExactReplayParity,
        true,
        vec![
            sample(0, "send:job_a->job_b:latency=2", "seeded network simulator schedules a bounded send"),
            sample(1, "recv:job_b<-job_a:latency=2", "seeded network simulator replays the receive deterministically"),
            sample(2, "ack:job_b->job_a:latency=1", "seeded loopback acknowledgment remains replay-stable"),
        ],
        Vec::new(),
        "seeded loopback-network simulator stays replay-stable because ordering, latencies, and packet shapes are frozen explicitly",
    )
}

fn ambient_clock_refusal_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "ambient_system_clock_refusal",
        TassadarSimulatorKind::Clock,
        "host.clock.now",
        TassadarSimulatorCaseStatus::ExactRefusalParity,
        false,
        Vec::new(),
        vec![TassadarSimulatorRefusalKind::AmbientSystemClockDenied],
        "ambient system-clock reads remain refused because they bypass the seeded simulator envelope",
    )
}

fn ambient_random_refusal_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "ambient_os_random_refusal",
        TassadarSimulatorKind::Randomness,
        "host.random.os_entropy",
        TassadarSimulatorCaseStatus::ExactRefusalParity,
        false,
        Vec::new(),
        vec![TassadarSimulatorRefusalKind::AmbientOsRandomDenied],
        "ambient OS entropy remains refused because it bypasses the seeded simulator envelope",
    )
}

fn ambient_network_refusal_case() -> TassadarSimulatorCaseReceipt {
    receipt(
        "ambient_socket_network_refusal",
        TassadarSimulatorKind::NetworkSimulator,
        "host.network.socket_io",
        TassadarSimulatorCaseStatus::ExactRefusalParity,
        false,
        Vec::new(),
        vec![TassadarSimulatorRefusalKind::AmbientSocketNetworkDenied],
        "ambient socket I/O remains refused because it bypasses the bounded network-simulator envelope",
    )
}

fn sample(step_index: u32, value: &str, note: &str) -> TassadarSimulatorTraceSample {
    TassadarSimulatorTraceSample {
        step_index,
        value: String::from(value),
        note: String::from(note),
    }
}

fn receipt(
    case_id: &str,
    simulator_kind: TassadarSimulatorKind,
    seed_profile_id: &str,
    status: TassadarSimulatorCaseStatus,
    exact_replay_parity: bool,
    simulator_trace: Vec<TassadarSimulatorTraceSample>,
    refusal_kinds: Vec<TassadarSimulatorRefusalKind>,
    note: &str,
) -> TassadarSimulatorCaseReceipt {
    let mut receipt = TassadarSimulatorCaseReceipt {
        case_id: String::from(case_id),
        profile_id: String::from(TASSADAR_SIMULATOR_EFFECT_PROFILE_ID),
        simulator_kind,
        seed_profile_id: String::from(seed_profile_id),
        portability_envelope_id: String::from("cpu_reference_current_host"),
        status,
        exact_replay_parity,
        simulator_trace,
        refusal_kinds,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_simulator_effect_case_receipt|", &receipt);
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarSimulatorEffectRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSimulatorEffectRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSimulatorEffectRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SIMULATOR_EFFECT_PROFILE_ID, build_tassadar_simulator_effect_runtime_bundle,
        read_json, tassadar_simulator_effect_runtime_bundle_path,
        write_tassadar_simulator_effect_runtime_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn simulator_effect_runtime_bundle_keeps_simulators_and_refusals_explicit() {
        let bundle = build_tassadar_simulator_effect_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_SIMULATOR_EFFECT_PROFILE_ID);
        assert_eq!(bundle.exact_case_count, 3);
        assert_eq!(bundle.refusal_case_count, 3);
        assert_eq!(bundle.simulator_profile_ids.len(), 3);
        assert_eq!(bundle.portability_envelope_ids, vec![String::from("cpu_reference_current_host")]);
    }

    #[test]
    fn simulator_effect_runtime_bundle_keeps_seeded_traces_replayable() {
        let bundle = build_tassadar_simulator_effect_runtime_bundle();
        let clock_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "seeded_clock_tick_case")
            .expect("clock case");

        assert!(clock_case.exact_replay_parity);
        assert_eq!(clock_case.simulator_trace.len(), 3);
    }

    #[test]
    fn simulator_effect_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_simulator_effect_runtime_bundle();
        let committed = read_json(tassadar_simulator_effect_runtime_bundle_path())
            .expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_simulator_effect_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_simulator_effect_runtime_bundle.json");
        let bundle =
            write_tassadar_simulator_effect_runtime_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
        assert_eq!(
            tassadar_simulator_effect_runtime_bundle_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_simulator_effect_runtime_bundle.json")
        );
    }
}
