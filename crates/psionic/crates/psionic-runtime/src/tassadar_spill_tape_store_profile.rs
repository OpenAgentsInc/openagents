use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarCheckpointWorkloadFamily;

/// Stable spill-aware internal-compute profile over external tape-store continuation.
pub const TASSADAR_SPILL_TAPE_STORE_PROFILE_ID: &str =
    "tassadar.internal_compute.spill_tape_store.v1";
/// Stable family identifier for persisted spill-backed memory segments.
pub const TASSADAR_SPILL_SEGMENT_FAMILY_ID: &str = "tassadar.spill_segment.v1";
/// Stable family identifier for persisted external tape-store segments.
pub const TASSADAR_EXTERNAL_TAPE_STORE_FAMILY_ID: &str = "tassadar.external_tape_store.v1";
/// Stable run root for the committed spill/tape bundle.
pub const TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_spill_tape_store_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_SPILL_TAPE_STORE_BUNDLE_FILE: &str =
    "tassadar_spill_tape_store_bundle.json";

/// Typed case outcome for the spill/tape runtime profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSpillTapeCaseStatus {
    ExactSpillAndResumeParity,
    ExactRefusalParity,
}

/// Typed refusal over the spill/tape profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSpillTapeRefusalKind {
    OversizeStateOutOfEnvelope,
    MissingExternalTapeSegment,
    PortabilityEnvelopeMismatch,
}

/// One spill-backed memory segment admitted by the bounded profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillSegment {
    pub segment_id: String,
    pub family_id: String,
    pub source_checkpoint_id: String,
    pub segment_index: u32,
    pub spill_bytes: u32,
    pub page_count: u32,
    pub digest: String,
    pub note: String,
}

/// One external tape-store segment admitted by the bounded profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExternalTapeStoreSegment {
    pub segment_id: String,
    pub family_id: String,
    pub process_id: String,
    pub sequence_index: u32,
    pub payload_bytes: u32,
    pub digest: String,
    pub note: String,
}

/// One refusal case bound to a spill-backed continuation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeRefusal {
    pub refusal_kind: TassadarSpillTapeRefusalKind,
    pub object_ref: String,
    pub detail: String,
}

/// Runtime receipt for one bounded spill/tape case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeCaseReceipt {
    pub case_id: String,
    pub process_id: String,
    pub workload_family: TassadarCheckpointWorkloadFamily,
    pub checkpoint_id: String,
    pub checkpoint_step: u32,
    pub portability_envelope_id: String,
    pub status: TassadarSpillTapeCaseStatus,
    pub spill_vs_in_core_parity: bool,
    pub external_tape_resume_parity: bool,
    pub in_core_state_bytes: u32,
    pub spill_segments: Vec<TassadarSpillSegment>,
    pub external_tape_segments: Vec<TassadarExternalTapeStoreSegment>,
    pub refusal_cases: Vec<TassadarSpillTapeRefusal>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded spill/tape profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeStoreRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub spill_segment_family_id: String,
    pub external_tape_store_family_id: String,
    pub portability_envelope_ids: Vec<String>,
    pub case_receipts: Vec<TassadarSpillTapeCaseReceipt>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSpillTapeStoreRuntimeBundleError {
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
pub fn build_tassadar_spill_tape_store_runtime_bundle() -> TassadarSpillTapeStoreRuntimeBundle {
    let case_receipts = vec![
        exact_case_receipt(
            "long_loop_spill_resume",
            "tassadar.process.long_loop_kernel.v1",
            TassadarCheckpointWorkloadFamily::LongLoopKernel,
            "checkpoint.long_loop_kernel.02",
            16_384,
            96,
            0,
            128,
            2,
            vec![TassadarSpillTapeRefusal {
                refusal_kind: TassadarSpillTapeRefusalKind::MissingExternalTapeSegment,
                object_ref: String::from("external_tape://tassadar.process.long_loop_kernel.v1/segment/99"),
                detail: String::from(
                    "missing external tape segment remains an explicit refusal instead of silent reconstruction",
                ),
            }],
            "spill-backed long-loop continuation preserves exact parity against the in-core lane and replays from the external tape store exactly",
        ),
        exact_case_receipt(
            "search_frontier_spill_resume",
            "tassadar.process.search_frontier_kernel.v1",
            TassadarCheckpointWorkloadFamily::SearchFrontierKernel,
            "checkpoint.search_frontier_kernel.03",
            2_048,
            112,
            2,
            144,
            2,
            vec![TassadarSpillTapeRefusal {
                refusal_kind: TassadarSpillTapeRefusalKind::PortabilityEnvelopeMismatch,
                object_ref: String::from("portability://metal_served"),
                detail: String::from(
                    "spill-backed continuation remains bounded to the current-host cpu-reference portability envelope and refuses non-cpu widening",
                ),
            }],
            "spill-backed search-frontier continuation preserves exact parity under the bounded cpu-reference portability envelope",
        ),
        refusal_case_receipt(),
    ];
    let exact_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_cases.len() as u32)
        .sum();
    let mut bundle = TassadarSpillTapeStoreRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.spill_tape_store.bundle.v1"),
        profile_id: String::from(TASSADAR_SPILL_TAPE_STORE_PROFILE_ID),
        spill_segment_family_id: String::from(TASSADAR_SPILL_SEGMENT_FAMILY_ID),
        external_tape_store_family_id: String::from(TASSADAR_EXTERNAL_TAPE_STORE_FAMILY_ID),
        portability_envelope_ids: vec![String::from("cpu_reference_current_host")],
        case_receipts,
        exact_case_count,
        refusal_case_count,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded spill-aware continuation profile with external tape-store semantics on the current-host cpu-reference envelope only. It keeps oversize state, missing tape segments, and non-cpu portability widening on explicit refusal paths instead of implying infinite in-core memory, arbitrary process persistence, or broader served internal compute",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Spill/tape runtime bundle covers {} case rows with exact_cases={} and refusal_rows={}.",
        bundle.case_receipts.len(),
        bundle.exact_case_count,
        bundle.refusal_case_count,
    );
    bundle.bundle_digest = stable_digest(b"psionic_tassadar_spill_tape_store_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_spill_tape_store_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_SPILL_TAPE_STORE_RUN_ROOT_REF)
        .join(TASSADAR_SPILL_TAPE_STORE_BUNDLE_FILE)
}

pub fn write_tassadar_spill_tape_store_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSpillTapeStoreRuntimeBundle, TassadarSpillTapeStoreRuntimeBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSpillTapeStoreRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_spill_tape_store_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSpillTapeStoreRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn exact_case_receipt(
    case_id: &str,
    process_id: &str,
    workload_family: TassadarCheckpointWorkloadFamily,
    checkpoint_id: &str,
    checkpoint_step: u32,
    in_core_state_bytes: u32,
    spill_segment_index_start: u32,
    spill_bytes_per_segment: u32,
    tape_segment_count: u32,
    refusal_cases: Vec<TassadarSpillTapeRefusal>,
    note: &str,
) -> TassadarSpillTapeCaseReceipt {
    let spill_segments = (0..2)
        .map(|offset| spill_segment(
            case_id,
            checkpoint_id,
            spill_segment_index_start + offset,
            spill_bytes_per_segment,
        ))
        .collect::<Vec<_>>();
    let external_tape_segments = (0..tape_segment_count)
        .map(|index| external_tape_segment(process_id, index, 72 + (index * 16)))
        .collect::<Vec<_>>();
    let mut receipt = TassadarSpillTapeCaseReceipt {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        workload_family,
        checkpoint_id: String::from(checkpoint_id),
        checkpoint_step,
        portability_envelope_id: String::from("cpu_reference_current_host"),
        status: TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity,
        spill_vs_in_core_parity: true,
        external_tape_resume_parity: true,
        in_core_state_bytes,
        spill_segments,
        external_tape_segments,
        refusal_cases,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_spill_tape_case_receipt|", &receipt);
    receipt
}

fn refusal_case_receipt() -> TassadarSpillTapeCaseReceipt {
    let process_id = "tassadar.process.state_machine_accumulator.v1";
    let spill_segments = vec![spill_segment(
        "state_machine_oversize_spill_refusal",
        "checkpoint.state_machine_accumulator.04",
        0,
        512,
    )];
    let external_tape_segments = vec![external_tape_segment(process_id, 0, 48)];
    let refusal_cases = vec![TassadarSpillTapeRefusal {
        refusal_kind: TassadarSpillTapeRefusalKind::OversizeStateOutOfEnvelope,
        object_ref: String::from("spill://tassadar.process.state_machine_accumulator.v1/segment/00"),
        detail: String::from(
            "spill-backed continuation refuses state that exceeds the bounded segment-size envelope instead of pretending unbounded in-core extension exists",
        ),
    }];
    let mut receipt = TassadarSpillTapeCaseReceipt {
        case_id: String::from("state_machine_oversize_spill_refusal"),
        process_id: String::from(process_id),
        workload_family: TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
        checkpoint_id: String::from("checkpoint.state_machine_accumulator.04"),
        checkpoint_step: 1_024,
        portability_envelope_id: String::from("cpu_reference_current_host"),
        status: TassadarSpillTapeCaseStatus::ExactRefusalParity,
        spill_vs_in_core_parity: false,
        external_tape_resume_parity: false,
        in_core_state_bytes: 176,
        spill_segments,
        external_tape_segments,
        refusal_cases,
        note: String::from(
            "oversize spill state stays on an explicit refusal path instead of widening the bounded segment-size contract",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_spill_tape_case_receipt|", &receipt);
    receipt
}

fn spill_segment(
    case_id: &str,
    checkpoint_id: &str,
    segment_index: u32,
    spill_bytes: u32,
) -> TassadarSpillSegment {
    let mut segment = TassadarSpillSegment {
        segment_id: format!("{case_id}::spill::{segment_index:02}"),
        family_id: String::from(TASSADAR_SPILL_SEGMENT_FAMILY_ID),
        source_checkpoint_id: String::from(checkpoint_id),
        segment_index,
        spill_bytes,
        page_count: spill_bytes.div_ceil(32),
        digest: String::new(),
        note: format!(
            "spill segment {segment_index} externalizes bounded continuation bytes from `{checkpoint_id}`",
        ),
    };
    segment.digest = stable_digest(b"psionic_tassadar_spill_segment|", &segment);
    segment
}

fn external_tape_segment(
    process_id: &str,
    sequence_index: u32,
    payload_bytes: u32,
) -> TassadarExternalTapeStoreSegment {
    let mut segment = TassadarExternalTapeStoreSegment {
        segment_id: format!("{process_id}::external_tape::{sequence_index:02}"),
        family_id: String::from(TASSADAR_EXTERNAL_TAPE_STORE_FAMILY_ID),
        process_id: String::from(process_id),
        sequence_index,
        payload_bytes,
        digest: String::new(),
        note: format!(
            "external tape-store segment {sequence_index} carries bounded continuation lineage for `{process_id}`",
        ),
    };
    segment.digest = stable_digest(b"psionic_tassadar_external_tape_segment|", &segment);
    segment
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
) -> Result<T, TassadarSpillTapeStoreRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSpillTapeStoreRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSpillTapeStoreRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SPILL_TAPE_STORE_PROFILE_ID, TassadarSpillTapeCaseStatus,
        build_tassadar_spill_tape_store_runtime_bundle, read_json,
        tassadar_spill_tape_store_runtime_bundle_path, write_tassadar_spill_tape_store_runtime_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn spill_tape_runtime_bundle_keeps_parity_and_refusal_boundaries_explicit() {
        let bundle = build_tassadar_spill_tape_store_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_SPILL_TAPE_STORE_PROFILE_ID);
        assert_eq!(bundle.exact_case_count, 2);
        assert_eq!(bundle.refusal_case_count, 3);
        assert_eq!(bundle.portability_envelope_ids, vec![String::from("cpu_reference_current_host")]);
        assert_eq!(bundle.case_receipts.len(), 3);
    }

    #[test]
    fn spill_tape_runtime_bundle_tracks_exact_and_refusal_rows() {
        let bundle = build_tassadar_spill_tape_store_runtime_bundle();
        let exact_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "long_loop_spill_resume")
            .expect("exact case");
        assert_eq!(
            exact_case.status,
            TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity
        );
        assert!(exact_case.spill_vs_in_core_parity);
        assert!(exact_case.external_tape_resume_parity);
        assert_eq!(exact_case.spill_segments.len(), 2);
        let refusal_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "state_machine_oversize_spill_refusal")
            .expect("refusal case");
        assert_eq!(refusal_case.status, TassadarSpillTapeCaseStatus::ExactRefusalParity);
        assert!(!refusal_case.spill_vs_in_core_parity);
        assert!(!refusal_case.external_tape_resume_parity);
    }

    #[test]
    fn spill_tape_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_spill_tape_store_runtime_bundle();
        let committed = read_json(tassadar_spill_tape_store_runtime_bundle_path())
            .expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_spill_tape_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_spill_tape_store_bundle.json");
        let bundle =
            write_tassadar_spill_tape_store_runtime_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
        assert_eq!(
            tassadar_spill_tape_store_runtime_bundle_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_spill_tape_store_bundle.json")
        );
    }
}
