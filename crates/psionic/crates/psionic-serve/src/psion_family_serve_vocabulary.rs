use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_FAMILY_SERVE_VOCABULARY_SCHEMA_VERSION: &str =
    "psion.family_serve_vocabulary.v1";
pub const PSION_FAMILY_SERVE_VOCABULARY_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_family_serve_vocabulary_v1.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionFamilyServeVocabularyPacket {
    pub schema_version: String,
    pub vocabulary_id: String,
    pub family_name: String,
    pub summary: String,
    pub lanes: Vec<PsionFamilyServeLane>,
    pub shared_rules: Vec<PsionFamilyServeRule>,
    pub packet_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionFamilyServeLane {
    pub lane_id: String,
    pub family_role: String,
    pub served_surface_kind: String,
    pub evidence_class: String,
    pub claim_posture: String,
    pub execution_posture: String,
    pub publication_posture: String,
    pub canonical_authorities: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionFamilyServeRule {
    pub rule_id: String,
    pub rule: String,
}

impl PsionFamilyServeVocabularyPacket {
    pub fn write_fixture(&self, repo_root: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let path = repo_root
            .as_ref()
            .join(PSION_FAMILY_SERVE_VOCABULARY_FIXTURE_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self).expect("serialize packet"))?;
        Ok(())
    }
}

#[must_use]
pub fn builtin_psion_family_serve_vocabulary_packet() -> PsionFamilyServeVocabularyPacket {
    let mut packet = PsionFamilyServeVocabularyPacket {
        schema_version: PSION_FAMILY_SERVE_VOCABULARY_SCHEMA_VERSION.to_string(),
        vocabulary_id: "psion-family-serve-vocabulary-v1".to_string(),
        family_name: "Psion".to_string(),
        summary: "Family-level serve and claim vocabulary for generic learned, plugin-conditioned, and executor-capable Psion lanes.".to_string(),
        lanes: vec![
            PsionFamilyServeLane {
                lane_id: "generic_compact_decoder".to_string(),
                family_role: "generic_learned_lane".to_string(),
                served_surface_kind: "direct_artifact_backed_generation".to_string(),
                evidence_class: "learned_judgment_only".to_string(),
                claim_posture: "served_evidence_plus_served_output_claim_posture".to_string(),
                execution_posture: "no_hidden_execution_surface".to_string(),
                publication_posture: "bounded_learned_lane_publication".to_string(),
                canonical_authorities: vec![
                    "docs/PSION_PROGRAM_MAP.md".to_string(),
                    "docs/PSION_GENERIC_LOAD_AND_GENERATE.md".to_string(),
                    "docs/PSION_SERVED_EVIDENCE.md".to_string(),
                    "docs/PSION_SERVED_OUTPUT_CLAIMS.md".to_string(),
                ],
                detail: "Generic learned Psion may load, generate, and serve with explicit learned-judgment claim posture, but it may not imply executor backing, source grounding, or verification by default.".to_string(),
            },
            PsionFamilyServeLane {
                lane_id: "plugin_conditioned".to_string(),
                family_role: "plugin_conditioned_learned_lane".to_string(),
                served_surface_kind: "lane_specific_served_posture_over_shared_claim_contract".to_string(),
                evidence_class: "learned_plugin_reasoning_with_runtime_receipt_gate".to_string(),
                claim_posture: "plugin_capability_matrix_and_served_posture_reusing_shared_claim_contract".to_string(),
                execution_posture: "runtime_owned_plugin_execution_with_receipts".to_string(),
                publication_posture: "bounded_operator_internal_or_lane_specific_publication".to_string(),
                canonical_authorities: vec![
                    "docs/PSION_PLUGIN_CLAIM_BOUNDARY_AND_CAPABILITY_POSTURE.md".to_string(),
                    "docs/PSION_PLUGIN_HOST_NATIVE_CAPABILITY_MATRIX_V1.md".to_string(),
                    "docs/PSION_PLUGIN_MIXED_CAPABILITY_MATRIX_V2.md".to_string(),
                    "docs/PSION_SERVED_EVIDENCE.md".to_string(),
                    "docs/PSION_SERVED_OUTPUT_CLAIMS.md".to_string(),
                ],
                detail: "Plugin-conditioned Psion may reason about admitted plugins and surface lane-specific served posture, but runtime execution remains receipt-gated and the lane may not flatten into generic plugin-platform or arbitrary software claims.".to_string(),
            },
            PsionFamilyServeLane {
                lane_id: "executor_capable_tassadar_profile".to_string(),
                family_role: "executor_capable_psion_profile".to_string(),
                served_surface_kind: "bounded_executor_profile_and_fast_route_projection".to_string(),
                evidence_class: "bounded_executor_replacement_and_admitted_fast_route_truth".to_string(),
                claim_posture: "promotion_and_replacement_packets_with_consumer_seam_validation".to_string(),
                execution_posture: "explicit_executor_profile_only".to_string(),
                publication_posture: "bounded_admitted_executor_profile".to_string(),
                canonical_authorities: vec![
                    "docs/PSION_EXECUTOR_PROGRAM.md".to_string(),
                    "docs/PSION_EXECUTOR_TRAINED_V1_PROMOTION.md".to_string(),
                    "docs/PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT.md".to_string(),
                    "docs/ROADMAP_TASSADAR.md".to_string(),
                ],
                detail: "Executor-capable Psion remains the bounded Tassadar profile. Its serve vocabulary is narrower than the family umbrella and must stay tied to admitted-workload replacement truth, explicit executor posture, and the reference_linear versus hull_cache claim boundary.".to_string(),
            },
        ],
        shared_rules: vec![
            PsionFamilyServeRule {
                rule_id: "family_name_does_not_flatten_lane_truth".to_string(),
                rule: "Do not use the umbrella family name Psion to erase lane-specific evidence classes, publication posture, or execution boundaries.".to_string(),
            },
            PsionFamilyServeRule {
                rule_id: "shared_claim_contract_reuse".to_string(),
                rule: "All family lanes should reuse the shared served-evidence and served-output-claim posture contract where applicable instead of inventing lane-private claim surfaces without explicit reason.".to_string(),
            },
            PsionFamilyServeRule {
                rule_id: "no_hidden_execution".to_string(),
                rule: "No lane may imply hidden execution. Plugin-conditioned and executor-capable lanes may only claim execution through their explicit runtime or executor receipt surfaces.".to_string(),
            },
            PsionFamilyServeRule {
                rule_id: "executor_lane_not_generic_default".to_string(),
                rule: "Executor-capable truth belongs to the bounded Tassadar profile and may not be projected onto the generic compact-decoder lane or the plugin-conditioned lane by umbrella shorthand.".to_string(),
            },
        ],
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_digest(&packet);
    packet
}

fn stable_digest(packet: &PsionFamilyServeVocabularyPacket) -> String {
    let mut hasher = Sha256::new();
    hasher.update(packet.schema_version.as_bytes());
    hasher.update(packet.vocabulary_id.as_bytes());
    hasher.update(packet.family_name.as_bytes());
    hasher.update(packet.summary.as_bytes());
    for lane in &packet.lanes {
        hasher.update(lane.lane_id.as_bytes());
        hasher.update(lane.family_role.as_bytes());
        hasher.update(lane.served_surface_kind.as_bytes());
        hasher.update(lane.evidence_class.as_bytes());
        hasher.update(lane.claim_posture.as_bytes());
        hasher.update(lane.execution_posture.as_bytes());
        hasher.update(lane.publication_posture.as_bytes());
        for authority in &lane.canonical_authorities {
            hasher.update(authority.as_bytes());
        }
        hasher.update(lane.detail.as_bytes());
    }
    for rule in &packet.shared_rules {
        hasher.update(rule.rule_id.as_bytes());
        hasher.update(rule.rule.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_serve_vocabulary_fixture_matches_builtin_truth() {
        let expected = builtin_psion_family_serve_vocabulary_packet();
        let fixture = include_str!("../../../fixtures/psion/serve/psion_family_serve_vocabulary_v1.json");
        let actual: PsionFamilyServeVocabularyPacket =
            serde_json::from_str(fixture).expect("parse family serve vocabulary fixture");
        assert_eq!(actual, expected);
    }
}
