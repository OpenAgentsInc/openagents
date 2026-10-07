use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_ir::{
    TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_COMPONENT_LINKING_PROFILE_ID,
};

pub const TASSADAR_COMPONENT_LINKING_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_component_linking_profile_v1/tassadar_component_linking_runtime_bundle.json";
pub const TASSADAR_COMPONENT_LINKING_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_component_linking_profile_v1";

/// Canonical status for one bounded component-linking case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarComponentLinkingCaseStatus {
    ExactComponentParity,
    ExactRefusalParity,
    Drift,
}

/// One linked component preserved in the bounded runtime bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingLineageEntry {
    pub component_ref: String,
    pub interface_type_ids: Vec<String>,
    pub lineage_digest: String,
}

/// One bounded component-linking case receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingCaseReceipt {
    pub case_id: String,
    pub topology_id: String,
    pub status: TassadarComponentLinkingCaseStatus,
    pub lineage_entries: Vec<TassadarComponentLinkingLineageEntry>,
    pub exact_interface_lowering_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded component/linking proposal profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub exact_component_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub case_receipts: Vec<TassadarComponentLinkingCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Returns the canonical runtime bundle for the bounded component/linking profile.
#[must_use]
pub fn build_tassadar_component_linking_runtime_bundle(
) -> TassadarComponentLinkingRuntimeBundle {
    let case_receipts = vec![
        success_case(
            "utf8_decode_writer_component_pair",
            "utf8_decode_writer_component_pair",
            &[
                lineage(
                    "utf8_decode_component@1.0.0",
                    &["list_u8", "result_i32"],
                ),
                lineage(
                    "heap_writer_component@1.0.0",
                    &["result_i32"],
                ),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json",
            ],
            "bounded component linking keeps one explicit utf8-decode plus heap-writer pair with stable interface-lowering lineage instead of widening to arbitrary component graphs",
        ),
        success_case(
            "checkpoint_resume_component_pair",
            "checkpoint_resume_component_pair",
            &[
                lineage(
                    "checkpoint_codec_component@1.0.0",
                    &["record_i32_i32", "result_refusal_code"],
                ),
                lineage(
                    "resume_runtime_component@1.0.0",
                    &["record_i32_i32", "result_refusal_code"],
                ),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json",
            ],
            "bounded checkpoint/resume component linking keeps interface records and result-coded refusals explicit across the paired lineage",
        ),
        refusal_case(
            "incompatible_component_interface_refusal",
            "utf8_decode_writer_component_pair",
            "incompatible_component_interface",
            &[
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
            ],
            "incompatible component interfaces stay as typed refusal truth instead of widening from the two admitted component pairs",
        ),
    ];
    let exact_component_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarComponentLinkingCaseStatus::ExactComponentParity)
        .count() as u32;
    let exact_refusal_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarComponentLinkingCaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut bundle = TassadarComponentLinkingRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.component_linking_profile.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_COMPONENT_LINKING_PROFILE_ID),
        portability_envelope_id: String::from(
            TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        exact_component_parity_count,
        exact_refusal_parity_count,
        case_receipts,
        claim_boundary: String::from(
            "this runtime bundle proves one bounded component/linking proposal profile with explicit interface-type lineage and typed incompatible-interface refusal truth on the current-host cpu-reference lane. It does not claim arbitrary component-model closure, unrestricted interface-type lowering, or broader served publication",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Component-linking runtime bundle covers {} cases with exact_component_parity={} and exact_refusal_parity={}.",
        bundle.case_receipts.len(),
        bundle.exact_component_parity_count,
        bundle.exact_refusal_parity_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_component_linking_runtime_bundle|",
        &bundle,
    );
    bundle
}

/// Returns the canonical absolute path for the committed runtime bundle.
#[must_use]
pub fn tassadar_component_linking_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPONENT_LINKING_RUNTIME_BUNDLE_REF)
}

/// Writes the committed runtime bundle.
pub fn write_tassadar_component_linking_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarComponentLinkingRuntimeBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_component_linking_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)
        .expect("component-linking runtime bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_component_linking_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarComponentLinkingRuntimeBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn lineage(
    component_ref: &str,
    interface_type_ids: &[&str],
) -> TassadarComponentLinkingLineageEntry {
    let mut entry = TassadarComponentLinkingLineageEntry {
        component_ref: String::from(component_ref),
        interface_type_ids: interface_type_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        lineage_digest: String::new(),
    };
    entry.lineage_digest = stable_digest(b"psionic_tassadar_component_lineage|", &entry);
    entry
}

fn success_case(
    case_id: &str,
    topology_id: &str,
    lineage_entries: &[TassadarComponentLinkingLineageEntry],
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarComponentLinkingCaseReceipt {
    let mut receipt = TassadarComponentLinkingCaseReceipt {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        status: TassadarComponentLinkingCaseStatus::ExactComponentParity,
        lineage_entries: lineage_entries.to_vec(),
        exact_interface_lowering_parity: true,
        refusal_reason_id: None,
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_component_linking_case|", &receipt);
    receipt
}

fn refusal_case(
    case_id: &str,
    topology_id: &str,
    refusal_reason_id: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarComponentLinkingCaseReceipt {
    let mut receipt = TassadarComponentLinkingCaseReceipt {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        status: TassadarComponentLinkingCaseStatus::ExactRefusalParity,
        lineage_entries: Vec::new(),
        exact_interface_lowering_parity: false,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_component_linking_case|", &receipt);
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
        TASSADAR_COMPONENT_LINKING_PROFILE_ID,
        build_tassadar_component_linking_runtime_bundle,
        load_tassadar_component_linking_runtime_bundle,
        tassadar_component_linking_runtime_bundle_path,
        write_tassadar_component_linking_runtime_bundle,
    };

    #[test]
    fn component_linking_runtime_bundle_keeps_lineage_and_refusals_explicit() {
        let bundle = build_tassadar_component_linking_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_COMPONENT_LINKING_PROFILE_ID);
        assert_eq!(bundle.exact_component_parity_count, 2);
        assert_eq!(bundle.exact_refusal_parity_count, 1);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "checkpoint_resume_component_pair"
                && case.lineage_entries.len() == 2
        }));
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "incompatible_component_interface_refusal"
                && case.refusal_reason_id.as_deref() == Some("incompatible_component_interface")
        }));
    }

    #[test]
    fn component_linking_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_component_linking_runtime_bundle();
        let committed = load_tassadar_component_linking_runtime_bundle(
            tassadar_component_linking_runtime_bundle_path(),
        )
        .expect("committed component-linking bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_component_linking_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_component_linking_runtime_bundle.json");
        let bundle = write_tassadar_component_linking_runtime_bundle(&output_path)
            .expect("write runtime bundle");
        let persisted = load_tassadar_component_linking_runtime_bundle(&output_path)
            .expect("persisted component-linking bundle");

        assert_eq!(bundle, persisted);
    }
}
