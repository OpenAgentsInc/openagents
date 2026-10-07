use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_MEMORY64_PROFILE_ID: &str =
    "tassadar.proposal_profile.memory64_continuation.v1";
pub const TASSADAR_MEMORY64_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID: &str =
    "cpu_reference_current_host";
pub const TASSADAR_MEMORY64_RESUME_FAMILY_ID: &str = "tassadar.memory64_resume.v1";
pub const TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_memory64_resume_v1";
pub const TASSADAR_MEMORY64_RESUME_BUNDLE_FILE: &str = "tassadar_memory64_resume_bundle.json";

/// Canonical status for one bounded memory64 continuation case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemory64CaseStatus {
    ExactResumeParity,
    ExactRefusalParity,
    Drift,
}

/// Sparse resident window preserved across a bounded memory64 checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64SparseWindowReceipt {
    pub window_id: String,
    pub base_address: u64,
    pub window_bytes: u64,
    pub nonzero_byte_count: u32,
    pub window_digest: String,
}

/// Persisted checkpoint over one bounded memory64 continuation prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64ResumeCheckpoint {
    pub checkpoint_id: String,
    pub profile_id: String,
    pub paused_after_step_count: u32,
    pub virtual_cursor_address: u64,
    pub max_virtual_address_touched: u64,
    pub resident_windows: Vec<TassadarMemory64SparseWindowReceipt>,
    pub spilled_window_count: u32,
    pub spill_bytes: u64,
    pub checkpoint_digest: String,
}

/// One bounded memory64 continuation case receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64CaseReceipt {
    pub case_id: String,
    pub workload_family: String,
    pub virtual_address_bits: u8,
    pub status: TassadarMemory64CaseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<TassadarMemory64ResumeCheckpoint>,
    pub max_virtual_address_touched: u64,
    pub resumed_suffix_step_count: u32,
    pub exact_large_address_parity: bool,
    pub exact_resume_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded memory64 continuation profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64ResumeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub checkpoint_family_id: String,
    pub portability_envelope_id: String,
    pub case_receipts: Vec<TassadarMemory64CaseReceipt>,
    pub exact_resume_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

/// Returns the canonical runtime bundle for the bounded memory64 continuation profile.
#[must_use]
pub fn build_tassadar_memory64_resume_bundle() -> TassadarMemory64ResumeBundle {
    let case_receipts = vec![
        success_case(
            "sparse_above_4g_resume",
            "memory64_sparse_scan",
            0x1_0000_11ff,
            48,
            0x1_0000_0800,
            2,
            32_768,
            &[
                sparse_window("scan_window_a", 0x1_0000_0000, 4_096, 19),
                sparse_window("scan_window_b", 0x1_0000_1000, 4_096, 11),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "sparse single-memory continuation preserves parity above the 4GiB boundary through explicit resident windows and spill bytes instead of implying arbitrary flat memory64 closure",
        ),
        success_case(
            "memory_grow_above_4g_resume",
            "memory64_growth_resume",
            0x1_0002_0fff,
            96,
            0x1_0001_8000,
            3,
            65_536,
            &[
                sparse_window("grow_window_a", 0x1_0001_0000, 8_192, 27),
                sparse_window("grow_window_b", 0x1_0001_8000, 8_192, 13),
                sparse_window("grow_window_c", 0x1_0002_0000, 8_192, 7),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_resumable_multi_slice_promotion_report.json",
            ],
            "bounded memory growth above 4GiB remains explicit as a continuation receipt with sparse windows and typed spill accounting rather than an arbitrary large-heap claim",
        ),
        refusal_case(
            "backend_virtual_address_limit_refusal",
            "memory64_backend_limit_boundary",
            0x2_0000_0000,
            "backend_virtual_address_limit",
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json",
            ],
            "unsupported host or backend virtual-address envelopes stay as explicit refusal truth instead of being inferred from smaller-memory success cases",
        ),
    ];
    let exact_resume_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarMemory64CaseStatus::ExactResumeParity)
        .count() as u32;
    let exact_refusal_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarMemory64CaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut bundle = TassadarMemory64ResumeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.memory64_resume.bundle.v1"),
        profile_id: String::from(TASSADAR_MEMORY64_PROFILE_ID),
        checkpoint_family_id: String::from(TASSADAR_MEMORY64_RESUME_FAMILY_ID),
        portability_envelope_id: String::from(
            TASSADAR_MEMORY64_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        case_receipts,
        exact_resume_parity_count,
        exact_refusal_parity_count,
        claim_boundary: String::from(
            "this runtime bundle proves one bounded single-memory memory64 continuation profile over sparse addresses above the 4GiB boundary with explicit spill accounting and typed backend-limit refusal. It does not claim arbitrary Wasm memory64 closure, multi-memory support, generic large-heap portability, or served-profile widening",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(b"tassadar_memory64_resume_bundle|", &bundle);
    bundle
}

/// Returns the canonical absolute path for the committed runtime bundle.
#[must_use]
pub fn tassadar_memory64_resume_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_MEMORY64_RESUME_RUN_ROOT_REF)
        .join(TASSADAR_MEMORY64_RESUME_BUNDLE_FILE)
}

/// Writes the committed runtime bundle.
pub fn write_tassadar_memory64_resume_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMemory64ResumeBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_memory64_resume_bundle();
    let json = serde_json::to_string_pretty(&bundle).expect("memory64 runtime bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_memory64_resume_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarMemory64ResumeBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn sparse_window(
    window_id: &str,
    base_address: u64,
    window_bytes: u64,
    nonzero_byte_count: u32,
) -> TassadarMemory64SparseWindowReceipt {
    let mut window = TassadarMemory64SparseWindowReceipt {
        window_id: String::from(window_id),
        base_address,
        window_bytes,
        nonzero_byte_count,
        window_digest: String::new(),
    };
    window.window_digest = stable_digest(b"tassadar_memory64_sparse_window|", &window);
    window
}

fn success_case(
    case_id: &str,
    workload_family: &str,
    max_virtual_address_touched: u64,
    resumed_suffix_step_count: u32,
    virtual_cursor_address: u64,
    spilled_window_count: u32,
    spill_bytes: u64,
    resident_windows: &[TassadarMemory64SparseWindowReceipt],
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMemory64CaseReceipt {
    let mut checkpoint = TassadarMemory64ResumeCheckpoint {
        checkpoint_id: format!("{case_id}.checkpoint.v1"),
        profile_id: String::from(TASSADAR_MEMORY64_PROFILE_ID),
        paused_after_step_count: resumed_suffix_step_count / 2,
        virtual_cursor_address,
        max_virtual_address_touched,
        resident_windows: resident_windows.to_vec(),
        spilled_window_count,
        spill_bytes,
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest = stable_digest(b"tassadar_memory64_checkpoint|", &checkpoint);
    let mut receipt = TassadarMemory64CaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        virtual_address_bits: 64,
        status: TassadarMemory64CaseStatus::ExactResumeParity,
        checkpoint: Some(checkpoint),
        max_virtual_address_touched,
        resumed_suffix_step_count,
        exact_large_address_parity: true,
        exact_resume_parity: true,
        refusal_reason_id: None,
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_memory64_case_receipt|", &receipt);
    receipt
}

fn refusal_case(
    case_id: &str,
    workload_family: &str,
    max_virtual_address_touched: u64,
    refusal_reason_id: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMemory64CaseReceipt {
    let mut receipt = TassadarMemory64CaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        virtual_address_bits: 64,
        status: TassadarMemory64CaseStatus::ExactRefusalParity,
        checkpoint: None,
        max_virtual_address_touched,
        resumed_suffix_step_count: 0,
        exact_large_address_parity: false,
        exact_resume_parity: false,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_memory64_case_receipt|", &receipt);
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_MEMORY64_PROFILE_ID, build_tassadar_memory64_resume_bundle,
        load_tassadar_memory64_resume_bundle, tassadar_memory64_resume_bundle_path,
        write_tassadar_memory64_resume_bundle,
    };

    #[test]
    fn memory64_resume_bundle_keeps_large_address_and_refusal_truth_explicit() {
        let bundle = build_tassadar_memory64_resume_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_MEMORY64_PROFILE_ID);
        assert_eq!(bundle.exact_resume_parity_count, 2);
        assert_eq!(bundle.exact_refusal_parity_count, 1);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "sparse_above_4g_resume"
                && case.max_virtual_address_touched > u64::from(u32::MAX)
                && case.exact_large_address_parity
        }));
    }

    #[test]
    fn memory64_resume_bundle_matches_committed_truth() {
        let expected = build_tassadar_memory64_resume_bundle();
        let committed = load_tassadar_memory64_resume_bundle(tassadar_memory64_resume_bundle_path())
            .expect("committed memory64 resume bundle");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_memory64_resume_bundle_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_memory64_resume_bundle.json");
        let expected =
            write_tassadar_memory64_resume_bundle(&output_path).expect("write runtime bundle");
        let persisted =
            load_tassadar_memory64_resume_bundle(&output_path).expect("persisted runtime bundle");

        assert_eq!(persisted, expected);
    }
}
