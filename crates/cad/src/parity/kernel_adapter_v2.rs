use serde::{Deserialize, Serialize};

use crate::kernel::{
    KernelAdapterRegistry, KernelAdapterV2Descriptor, KernelCapability, KernelEngineFamily,
    openagents_kernel_adapter_v2_descriptor,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_ADAPTER_V2_ISSUE_ID: &str = "VCAD-PARITY-011";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelAdapterV2ParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub adapter_descriptor: KernelAdapterV2Descriptor,
    pub pluggability: KernelPluggabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelPluggabilitySummary {
    pub active_engine_id: String,
    pub registered_engine_count: usize,
    pub available_engine_ids: Vec<String>,
    pub required_capabilities: Vec<KernelCapability>,
}

pub fn build_kernel_adapter_v2_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelAdapterV2ParityManifest {
    let adapter_descriptor = openagents_kernel_adapter_v2_descriptor();
    let mut registry = KernelAdapterRegistry::new(adapter_descriptor.clone());
    let fallback = fallback_descriptor();
    registry
        .register(fallback)
        .expect("fallback engine descriptor should be registerable");
    let mut available_engine_ids = registry.engine_ids();
    available_engine_ids.sort();
    let mut required_capabilities = vec![
        KernelCapability::PrimitiveBox,
        KernelCapability::PrimitiveCylinder,
        KernelCapability::PrimitiveSphere,
        KernelCapability::PrimitiveCone,
    ];
    required_capabilities.sort();

    KernelAdapterV2ParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_ADAPTER_V2_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        adapter_descriptor,
        pluggability: KernelPluggabilitySummary {
            active_engine_id: registry.active_engine_id().to_string(),
            registered_engine_count: available_engine_ids.len(),
            available_engine_ids,
            required_capabilities,
        },
    }
}

fn fallback_descriptor() -> KernelAdapterV2Descriptor {
    KernelAdapterV2Descriptor {
        schema_version: 1,
        issue_id: PARITY_KERNEL_ADAPTER_V2_ISSUE_ID.to_string(),
        adapter_version: "2.0.0".to_string(),
        engine_id: "opencascade-kernel-v2".to_string(),
        engine_family: KernelEngineFamily::OpenCascade,
        supports_hot_swap: true,
        capabilities: vec![
            KernelCapability::PrimitiveBox,
            KernelCapability::PrimitiveCylinder,
            KernelCapability::PrimitiveSphere,
            KernelCapability::PrimitiveCone,
        ],
        diagnostics_contract: "cad.error.v1 + kernel.receipt.v2".to_string(),
    }
    .normalized()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_KERNEL_ADAPTER_V2_ISSUE_ID, build_kernel_adapter_v2_manifest, fallback_descriptor,
    };
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    #[test]
    fn fallback_descriptor_is_opencascade_family() {
        let descriptor = fallback_descriptor();
        assert_eq!(descriptor.engine_id, "opencascade-kernel-v2");
        assert_eq!(
            format!("{:?}", descriptor.engine_family),
            "OpenCascade".to_string()
        );
    }

    #[test]
    fn build_manifest_carries_commits_and_registry_metadata() {
        let scorecard = ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad-sha".to_string(),
            openagents_commit: "openagents-sha".to_string(),
            generated_from_gap_matrix: "gap.json".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.1,
                crates_match_rate: 0.1,
                commands_match_rate: 0.1,
                overall_match_rate: 0.1,
                docs_reference_count: 1,
                crates_reference_count: 1,
                commands_reference_count: 1,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        };
        let manifest = build_kernel_adapter_v2_manifest(&scorecard, "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_ADAPTER_V2_ISSUE_ID);
        assert_eq!(manifest.vcad_commit, "vcad-sha");
        assert_eq!(manifest.openagents_commit, "openagents-sha");
        assert_eq!(manifest.pluggability.registered_engine_count, 2);
    }
}
