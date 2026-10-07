use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_WEDGE_TAXONOMY_SUITE_ID: &str = "psionic.tassadar_wedge_taxonomy_suite.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDeterministicStructureClass {
    FullyDeterministic,
    MostlyDeterministicWithFallbacks,
    ChallengeProneStructured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVerifierCostClass {
    Cheap,
    Moderate,
    Expensive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarChallengeCostClass {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEvidenceValueClass {
    Operational,
    ValidatorFacing,
    SettlementFacing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarIntermediateComputeBenefitClass {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTaskPropertyProfile {
    pub profile_id: String,
    pub deterministic_structure: TassadarDeterministicStructureClass,
    pub verifier_cost: TassadarVerifierCostClass,
    pub challenge_cost: TassadarChallengeCostClass,
    pub evidence_value: TassadarEvidenceValueClass,
    pub exact_intermediate_compute_benefit: TassadarIntermediateComputeBenefitClass,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDerivedDomainSuite {
    pub suite_id: String,
    pub property_profile_id: String,
    pub workload_family_refs: Vec<String>,
    pub illustrative_domain_labels: Vec<String>,
    pub validator_attachment_expected: bool,
    pub fallback_comparison_required: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWedgeTaxonomySuite {
    pub suite_id: String,
    pub profiles: Vec<TassadarTaskPropertyProfile>,
    pub derived_domain_suites: Vec<TassadarDerivedDomainSuite>,
    pub suite_digest: String,
}

#[must_use]
pub fn tassadar_wedge_taxonomy_suite() -> TassadarWedgeTaxonomySuite {
    let profiles = vec![
        TassadarTaskPropertyProfile {
            profile_id: String::from("profile.patch_validator_low_challenge"),
            deterministic_structure: TassadarDeterministicStructureClass::FullyDeterministic,
            verifier_cost: TassadarVerifierCostClass::Cheap,
            challenge_cost: TassadarChallengeCostClass::Low,
            evidence_value: TassadarEvidenceValueClass::ValidatorFacing,
            exact_intermediate_compute_benefit: TassadarIntermediateComputeBenefitClass::High,
            note: String::from(
                "patch-style exact compute where the verifier is cheap and exact intermediate state matters a lot",
            ),
        },
        TassadarTaskPropertyProfile {
            profile_id: String::from("profile.long_loop_high_challenge"),
            deterministic_structure:
                TassadarDeterministicStructureClass::MostlyDeterministicWithFallbacks,
            verifier_cost: TassadarVerifierCostClass::Moderate,
            challenge_cost: TassadarChallengeCostClass::High,
            evidence_value: TassadarEvidenceValueClass::ValidatorFacing,
            exact_intermediate_compute_benefit: TassadarIntermediateComputeBenefitClass::High,
            note: String::from(
                "long-loop workloads where internal exact paths help but fallback and challenge costs stay material",
            ),
        },
        TassadarTaskPropertyProfile {
            profile_id: String::from("profile.search_settlement_heavy"),
            deterministic_structure: TassadarDeterministicStructureClass::ChallengeProneStructured,
            verifier_cost: TassadarVerifierCostClass::Expensive,
            challenge_cost: TassadarChallengeCostClass::High,
            evidence_value: TassadarEvidenceValueClass::SettlementFacing,
            exact_intermediate_compute_benefit: TassadarIntermediateComputeBenefitClass::Medium,
            note: String::from(
                "search workloads where validator and challenge posture dominate any raw internal compute savings",
            ),
        },
        TassadarTaskPropertyProfile {
            profile_id: String::from("profile.short_parity_operational"),
            deterministic_structure: TassadarDeterministicStructureClass::FullyDeterministic,
            verifier_cost: TassadarVerifierCostClass::Cheap,
            challenge_cost: TassadarChallengeCostClass::Low,
            evidence_value: TassadarEvidenceValueClass::Operational,
            exact_intermediate_compute_benefit: TassadarIntermediateComputeBenefitClass::Medium,
            note: String::from(
                "short bounded parity workloads where operational evidence matters but settlement-grade overhead is unnecessary",
            ),
        },
    ];
    let mut suite = TassadarWedgeTaxonomySuite {
        suite_id: String::from(TASSADAR_WEDGE_TAXONOMY_SUITE_ID),
        derived_domain_suites: vec![
            TassadarDerivedDomainSuite {
                suite_id: String::from("suite.patch_workflows"),
                property_profile_id: String::from("profile.patch_validator_low_challenge"),
                workload_family_refs: vec![String::from("patch_apply_internal_exact")],
                illustrative_domain_labels: vec![
                    String::from("patch verification"),
                    String::from("config transform"),
                ],
                validator_attachment_expected: true,
                fallback_comparison_required: false,
                note: String::from(
                    "suite is derived from cheap verification and high exact-intermediate benefit rather than from one vertical label",
                ),
            },
            TassadarDerivedDomainSuite {
                suite_id: String::from("suite.long_loop_workflows"),
                property_profile_id: String::from("profile.long_loop_high_challenge"),
                workload_family_refs: vec![String::from("long_loop_validator_heavy")],
                illustrative_domain_labels: vec![
                    String::from("workflow orchestration"),
                    String::from("branch-heavy validation"),
                ],
                validator_attachment_expected: true,
                fallback_comparison_required: true,
                note: String::from(
                    "suite is derived from high challenge cost and fallback-sensitive structure, not from one industry demo",
                ),
            },
            TassadarDerivedDomainSuite {
                suite_id: String::from("suite.search_with_settlement_value"),
                property_profile_id: String::from("profile.search_settlement_heavy"),
                workload_family_refs: vec![String::from("served_search_validator_mount")],
                illustrative_domain_labels: vec![
                    String::from("route search"),
                    String::from("validator-heavy planning"),
                ],
                validator_attachment_expected: true,
                fallback_comparison_required: true,
                note: String::from(
                    "suite is derived from expensive verification and settlement-facing evidence value rather than search hype alone",
                ),
            },
            TassadarDerivedDomainSuite {
                suite_id: String::from("suite.short_operational_parity"),
                property_profile_id: String::from("profile.short_parity_operational"),
                workload_family_refs: vec![String::from("parity_short_bounded")],
                illustrative_domain_labels: vec![
                    String::from("bounded admin automation"),
                    String::from("small exact checks"),
                ],
                validator_attachment_expected: false,
                fallback_comparison_required: false,
                note: String::from(
                    "suite is derived from low challenge cost and operational evidence needs instead of broad admin vertical branding",
                ),
            },
        ],
        profiles,
        suite_digest: String::new(),
    };
    suite.suite_digest = stable_digest(b"psionic_tassadar_wedge_taxonomy_suite|", &suite);
    suite
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
        TassadarChallengeCostClass, TassadarEvidenceValueClass, TassadarVerifierCostClass,
        tassadar_wedge_taxonomy_suite,
    };

    #[test]
    fn wedge_taxonomy_suite_is_machine_legible() {
        let suite = tassadar_wedge_taxonomy_suite();

        assert_eq!(suite.profiles.len(), 4);
        assert_eq!(suite.derived_domain_suites.len(), 4);
        assert!(suite.profiles.iter().any(|profile| {
            profile.verifier_cost == TassadarVerifierCostClass::Expensive
                && profile.evidence_value == TassadarEvidenceValueClass::SettlementFacing
                && profile.challenge_cost == TassadarChallengeCostClass::High
        }));
        assert!(suite.derived_domain_suites.iter().all(|domain| {
            suite
                .profiles
                .iter()
                .any(|profile| profile.profile_id == domain.property_profile_id)
        }));
    }
}
