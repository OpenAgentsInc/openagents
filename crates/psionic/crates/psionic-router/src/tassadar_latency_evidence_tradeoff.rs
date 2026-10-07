use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_VALIDATOR_HEAVY_WORKLOAD_PACK_ID: &str =
    "psionic.tassadar_validator_heavy_workload_pack.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTradeoffRouteFamily {
    Compiled,
    Learned,
    External,
    Hybrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarValidatorHeavyWorkloadCase {
    pub case_id: String,
    pub workload_family: String,
    pub validator_heavy: bool,
    pub challenge_rate_bps: u32,
    pub minimum_evidence_completeness_bps: u32,
    pub minimum_correctness_bps: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLatencyEvidenceRouteThreshold {
    pub threshold_id: String,
    pub max_latency_ms_for_light_evidence: u32,
    pub min_evidence_completeness_bps_when_validator_heavy: u32,
    pub min_correctness_bps_when_challenge_rate_high: u32,
    pub preferred_route_family_when_threshold_crossed: TassadarTradeoffRouteFamily,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarValidatorHeavyWorkloadPack {
    pub pack_id: String,
    pub cases: Vec<TassadarValidatorHeavyWorkloadCase>,
    pub thresholds: Vec<TassadarLatencyEvidenceRouteThreshold>,
    pub pack_digest: String,
}

#[must_use]
pub fn tassadar_validator_heavy_workload_pack() -> TassadarValidatorHeavyWorkloadPack {
    let mut pack = TassadarValidatorHeavyWorkloadPack {
        pack_id: String::from(TASSADAR_VALIDATOR_HEAVY_WORKLOAD_PACK_ID),
        cases: vec![
            TassadarValidatorHeavyWorkloadCase {
                case_id: String::from("validator_patch_fast"),
                workload_family: String::from("patch_apply_internal_exact"),
                validator_heavy: true,
                challenge_rate_bps: 500,
                minimum_evidence_completeness_bps: 9_200,
                minimum_correctness_bps: 9_700,
                note: String::from(
                    "patch case where validator attachment matters but latency still needs to remain reasonable",
                ),
            },
            TassadarValidatorHeavyWorkloadCase {
                case_id: String::from("challenge_search"),
                workload_family: String::from("served_search_validator_mount"),
                validator_heavy: true,
                challenge_rate_bps: 3_500,
                minimum_evidence_completeness_bps: 9_500,
                minimum_correctness_bps: 9_800,
                note: String::from(
                    "search case where challenge rate is high enough that low-evidence wins must be ignored",
                ),
            },
            TassadarValidatorHeavyWorkloadCase {
                case_id: String::from("learned_trial_error"),
                workload_family: String::from("verifier_guided_search"),
                validator_heavy: false,
                challenge_rate_bps: 1_200,
                minimum_evidence_completeness_bps: 8_800,
                minimum_correctness_bps: 9_300,
                note: String::from(
                    "trial-and-error case where learned and hybrid lanes can stay on the Pareto frontier when evidence remains acceptable",
                ),
            },
            TassadarValidatorHeavyWorkloadCase {
                case_id: String::from("long_loop_validator"),
                workload_family: String::from("long_loop_validator_heavy"),
                validator_heavy: true,
                challenge_rate_bps: 2_800,
                minimum_evidence_completeness_bps: 9_300,
                minimum_correctness_bps: 9_700,
                note: String::from(
                    "long-loop case where external or hybrid routes deserve explicit threshold treatment",
                ),
            },
        ],
        thresholds: vec![
            TassadarLatencyEvidenceRouteThreshold {
                threshold_id: String::from("threshold.validator_heavy.min_evidence"),
                max_latency_ms_for_light_evidence: 120,
                min_evidence_completeness_bps_when_validator_heavy: 9_300,
                min_correctness_bps_when_challenge_rate_high: 9_700,
                preferred_route_family_when_threshold_crossed: TassadarTradeoffRouteFamily::Hybrid,
                note: String::from(
                    "validator-heavy routes should escalate to hybrid or external paths when evidence posture falls below the current floor",
                ),
            },
            TassadarLatencyEvidenceRouteThreshold {
                threshold_id: String::from("threshold.challenge_rate.externalize"),
                max_latency_ms_for_light_evidence: 160,
                min_evidence_completeness_bps_when_validator_heavy: 9_500,
                min_correctness_bps_when_challenge_rate_high: 9_850,
                preferred_route_family_when_threshold_crossed:
                    TassadarTradeoffRouteFamily::External,
                note: String::from(
                    "high challenge-rate workloads should externalize once correctness or evidence posture slips under the high-stakes floor",
                ),
            },
        ],
        pack_digest: String::new(),
    };
    pack.pack_digest = stable_digest(b"psionic_tassadar_validator_heavy_workload_pack|", &pack);
    pack
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarTradeoffRouteFamily, tassadar_validator_heavy_workload_pack};

    #[test]
    fn validator_heavy_workload_pack_is_machine_legible() {
        let pack = tassadar_validator_heavy_workload_pack();

        assert_eq!(pack.cases.len(), 4);
        assert_eq!(pack.thresholds.len(), 2);
        assert!(pack.thresholds.iter().any(|threshold| {
            threshold.preferred_route_family_when_threshold_crossed
                == TassadarTradeoffRouteFamily::External
        }));
    }
}
