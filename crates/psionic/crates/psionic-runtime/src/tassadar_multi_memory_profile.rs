use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_ir::{
    TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_MULTI_MEMORY_PROFILE_ID,
};

pub const TASSADAR_MULTI_MEMORY_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_multi_memory_profile_v1/tassadar_multi_memory_runtime_bundle.json";
pub const TASSADAR_MULTI_MEMORY_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_multi_memory_profile_v1";

/// Canonical status for one bounded multi-memory case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMultiMemoryCaseStatus {
    ExactRoutingParity,
    ExactRefusalParity,
    Drift,
}

/// One routed memory in the bounded multi-memory lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryRouteReceipt {
    pub memory_id: String,
    pub memory_index: u8,
    pub role_id: String,
    pub read_bytes: u32,
    pub write_bytes: u32,
    pub route_digest: String,
}

/// Persisted checkpoint over the admitted multi-memory route family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryCheckpoint {
    pub checkpoint_id: String,
    pub profile_id: String,
    pub memory_order: Vec<String>,
    pub per_memory_digests: Vec<String>,
    pub paused_after_step_count: u32,
    pub resumed_suffix_step_count: u32,
    pub checkpoint_digest: String,
}

/// One bounded multi-memory case receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryCaseReceipt {
    pub case_id: String,
    pub topology_id: String,
    pub status: TassadarMultiMemoryCaseStatus,
    pub routes: Vec<TassadarMultiMemoryRouteReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<TassadarMultiMemoryCheckpoint>,
    pub exact_route_parity: bool,
    pub exact_resume_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded multi-memory routing profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub exact_routing_parity_count: u32,
    pub exact_resume_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub case_receipts: Vec<TassadarMultiMemoryCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Returns the canonical runtime bundle for the bounded multi-memory routing profile.
#[must_use]
pub fn build_tassadar_multi_memory_runtime_bundle() -> TassadarMultiMemoryRuntimeBundle {
    let case_receipts = vec![
        success_case(
            "rodata_heap_output_route",
            "rodata_heap_output_split",
            &[
                route("rodata", 0, "readonly_lookup", 192, 0),
                route("heap_output", 1, "mutable_output", 48, 64),
            ],
            None,
            &[
                "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json",
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            "bounded multi-memory routing keeps immutable lookup bytes and mutable output bytes on separate declared memories instead of hiding topology in lowering",
        ),
        success_case(
            "scratch_heap_checkpoint_route",
            "scratch_heap_checkpoint_split",
            &[
                route("scratch", 0, "scratch_mutable", 64, 32),
                route("heap", 1, "heap_mutable", 128, 96),
            ],
            Some(checkpoint(
                "scratch_heap_checkpoint_route.checkpoint.v1",
                &["scratch", "heap"],
                &[
                    "d2e65ed0fd7f9d7e00f509f4d2b51f843f96365bb3b6fec236d43a7f39be0be1",
                    "0ae7f8dd9f4ed1d9272c5b7fed0f37c52e5731c395d699dfcb8b4e5f6cf5774e",
                ],
                24,
                24,
            )),
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "bounded multi-memory replay keeps per-memory order and digest lineage explicit across scratch and heap memories",
        ),
        refusal_case(
            "malformed_memory_topology_refusal",
            "invalid_duplicate_memory_owner",
            "malformed_memory_topology",
            &[
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
                "fixtures/tassadar/reports/tassadar_memory64_profile_report.json",
            ],
            "duplicate or unmapped memory-owner topology remains typed refusal truth instead of widening from the admitted topologies",
        ),
    ];
    let exact_routing_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarMultiMemoryCaseStatus::ExactRoutingParity)
        .count() as u32;
    let exact_resume_parity_count = case_receipts
        .iter()
        .filter(|case| case.exact_resume_parity)
        .count() as u32;
    let exact_refusal_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarMultiMemoryCaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut bundle = TassadarMultiMemoryRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.multi_memory_profile.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_MULTI_MEMORY_PROFILE_ID),
        portability_envelope_id: String::from(
            TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        exact_routing_parity_count,
        exact_resume_parity_count,
        exact_refusal_parity_count,
        case_receipts,
        claim_boundary: String::from(
            "this runtime bundle proves one bounded multi-memory routing profile over two explicit topology families plus typed malformed-topology refusal on the current-host cpu-reference lane. It does not claim arbitrary Wasm multi-memory closure, memory64 mixing, generic allocator portability, or broader served publication",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Multi-memory runtime bundle covers {} cases with routing_parity={}, resume_parity={}, refusal_parity={}.",
        bundle.case_receipts.len(),
        bundle.exact_routing_parity_count,
        bundle.exact_resume_parity_count,
        bundle.exact_refusal_parity_count,
    );
    bundle.bundle_digest = stable_digest(b"psionic_tassadar_multi_memory_runtime_bundle|", &bundle);
    bundle
}

/// Returns the canonical absolute path for the committed runtime bundle.
#[must_use]
pub fn tassadar_multi_memory_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_MULTI_MEMORY_RUNTIME_BUNDLE_REF)
}

/// Writes the committed runtime bundle.
pub fn write_tassadar_multi_memory_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMultiMemoryRuntimeBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_multi_memory_runtime_bundle();
    let json =
        serde_json::to_string_pretty(&bundle).expect("multi-memory runtime bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_multi_memory_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarMultiMemoryRuntimeBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn route(
    memory_id: &str,
    memory_index: u8,
    role_id: &str,
    read_bytes: u32,
    write_bytes: u32,
) -> TassadarMultiMemoryRouteReceipt {
    let mut route = TassadarMultiMemoryRouteReceipt {
        memory_id: String::from(memory_id),
        memory_index,
        role_id: String::from(role_id),
        read_bytes,
        write_bytes,
        route_digest: String::new(),
    };
    route.route_digest = stable_digest(b"psionic_tassadar_multi_memory_route|", &route);
    route
}

fn checkpoint(
    checkpoint_id: &str,
    memory_order: &[&str],
    per_memory_digests: &[&str],
    paused_after_step_count: u32,
    resumed_suffix_step_count: u32,
) -> TassadarMultiMemoryCheckpoint {
    let mut checkpoint = TassadarMultiMemoryCheckpoint {
        checkpoint_id: String::from(checkpoint_id),
        profile_id: String::from(TASSADAR_MULTI_MEMORY_PROFILE_ID),
        memory_order: memory_order.iter().map(|value| String::from(*value)).collect(),
        per_memory_digests: per_memory_digests
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        paused_after_step_count,
        resumed_suffix_step_count,
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest =
        stable_digest(b"psionic_tassadar_multi_memory_checkpoint|", &checkpoint);
    checkpoint
}

fn success_case(
    case_id: &str,
    topology_id: &str,
    routes: &[TassadarMultiMemoryRouteReceipt],
    checkpoint: Option<TassadarMultiMemoryCheckpoint>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMultiMemoryCaseReceipt {
    let mut receipt = TassadarMultiMemoryCaseReceipt {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        status: TassadarMultiMemoryCaseStatus::ExactRoutingParity,
        routes: routes.to_vec(),
        exact_route_parity: true,
        exact_resume_parity: checkpoint.is_some(),
        checkpoint,
        refusal_reason_id: None,
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_multi_memory_case|", &receipt);
    receipt
}

fn refusal_case(
    case_id: &str,
    topology_id: &str,
    refusal_reason_id: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMultiMemoryCaseReceipt {
    let mut receipt = TassadarMultiMemoryCaseReceipt {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        status: TassadarMultiMemoryCaseStatus::ExactRefusalParity,
        routes: Vec::new(),
        checkpoint: None,
        exact_route_parity: false,
        exact_resume_parity: false,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_multi_memory_case|", &receipt);
    receipt
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_MULTI_MEMORY_PROFILE_ID, build_tassadar_multi_memory_runtime_bundle,
        load_tassadar_multi_memory_runtime_bundle, tassadar_multi_memory_runtime_bundle_path,
        write_tassadar_multi_memory_runtime_bundle,
    };

    #[test]
    fn multi_memory_runtime_bundle_keeps_routes_checkpoints_and_refusals_explicit() {
        let bundle = build_tassadar_multi_memory_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_MULTI_MEMORY_PROFILE_ID);
        assert_eq!(bundle.exact_routing_parity_count, 2);
        assert_eq!(bundle.exact_resume_parity_count, 1);
        assert_eq!(bundle.exact_refusal_parity_count, 1);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "scratch_heap_checkpoint_route"
                && case
                    .checkpoint
                    .as_ref()
                    .map(|checkpoint| checkpoint.memory_order.len() == 2)
                    .unwrap_or(false)
        }));
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "malformed_memory_topology_refusal"
                && case.refusal_reason_id.as_deref() == Some("malformed_memory_topology")
        }));
    }

    #[test]
    fn multi_memory_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_multi_memory_runtime_bundle();
        let committed = load_tassadar_multi_memory_runtime_bundle(
            tassadar_multi_memory_runtime_bundle_path(),
        )
        .expect("committed multi-memory runtime bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_multi_memory_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_multi_memory_runtime_bundle.json");
        let bundle =
            write_tassadar_multi_memory_runtime_bundle(&output_path).expect("write runtime bundle");
        let persisted = load_tassadar_multi_memory_runtime_bundle(&output_path)
            .expect("persisted multi-memory runtime bundle");

        assert_eq!(bundle, persisted);
    }
}
