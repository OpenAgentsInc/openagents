use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID: &str = "tassadar.numeric_profile.f32_only.v1";
pub const TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID: &str =
    "tassadar.numeric_profile.mixed_i32_f32.v1";
pub const TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID: &str =
    "tassadar.numeric_profile.bounded_f64_conversion.v1";

/// Support posture for one mixed-numeric profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedNumericSupportPosture {
    Exact,
    BoundedApproximate,
}

/// One declared profile in the staged mixed-numeric ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericProfileSpec {
    pub profile_id: String,
    pub support_posture: TassadarMixedNumericSupportPosture,
    pub admitted_conversion_ids: Vec<String>,
    pub refused_reason_ids: Vec<String>,
    pub detail: String,
}

/// Public contract for the staged mixed-numeric ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedNumericProfileLadderContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profiles: Vec<TassadarMixedNumericProfileSpec>,
    pub refused_numeric_family_ids: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarMixedNumericProfileLadderContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.mixed_numeric_profile_ladder.contract.v1"),
            profiles: vec![
                TassadarMixedNumericProfileSpec {
                    profile_id: String::from(TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID),
                    support_posture: TassadarMixedNumericSupportPosture::Exact,
                    admitted_conversion_ids: vec![
                        String::from("f32_identity"),
                        String::from("f32_ordered_comparison"),
                    ],
                    refused_reason_ids: vec![
                        String::from("f64_out_of_scope"),
                        String::from("mixed_numeric_out_of_scope"),
                    ],
                    detail: String::from(
                        "scalar-f32 arithmetic and ordered comparisons remain exact inside the bounded float lane",
                    ),
                },
                TassadarMixedNumericProfileSpec {
                    profile_id: String::from(TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID),
                    support_posture: TassadarMixedNumericSupportPosture::Exact,
                    admitted_conversion_ids: vec![
                        String::from("i32_to_f32_exact_range"),
                        String::from("f32_to_i32_trunc_checked"),
                        String::from("mixed_i32_f32_scale_add_exact"),
                    ],
                    refused_reason_ids: vec![
                        String::from("i32_to_f32_non_exact"),
                        String::from("f32_to_i32_invalid"),
                    ],
                    detail: String::from(
                        "mixed i32/f32 programs are exact only inside the declared exact-conversion and checked-truncation envelope",
                    ),
                },
                TassadarMixedNumericProfileSpec {
                    profile_id: String::from(TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID),
                    support_posture: TassadarMixedNumericSupportPosture::BoundedApproximate,
                    admitted_conversion_ids: vec![String::from("f64_to_f32_bounded_rounding")],
                    refused_reason_ids: vec![
                        String::from("f64_out_of_range"),
                        String::from("f64_nan_invalid"),
                    ],
                    detail: String::from(
                        "bounded f64 conversion is admitted only as an explicit approximation envelope into f32, not as full f64 exactness",
                    ),
                },
            ],
            refused_numeric_family_ids: vec![
                String::from("arbitrary_mixed_numeric_programs"),
                String::from("full_f64_exactness"),
                String::from("generic_wasm_numeric_closure"),
            ],
            claim_boundary: String::from(
                "this contract stages mixed numeric widening into exact scalar-f32, exact mixed i32/f32, and bounded-approximate f64-to-f32 profiles. It does not claim arbitrary Wasm numeric closure, generic mixed-numeric exactness, or full f64 exactness",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_mixed_numeric_profile_ladder_contract|",
            &contract,
        );
        contract
    }
}

/// Returns the canonical mixed-numeric ladder contract.
#[must_use]
pub fn tassadar_mixed_numeric_profile_ladder_contract(
) -> TassadarMixedNumericProfileLadderContract {
    TassadarMixedNumericProfileLadderContract::new()
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
        tassadar_mixed_numeric_profile_ladder_contract, TassadarMixedNumericSupportPosture,
        TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
    };

    #[test]
    fn mixed_numeric_profile_ladder_is_machine_legible() {
        let contract = tassadar_mixed_numeric_profile_ladder_contract();

        assert_eq!(contract.profiles.len(), 3);
        assert!(contract.profiles.iter().any(|profile| {
            profile.profile_id == TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID
                && profile.support_posture == TassadarMixedNumericSupportPosture::Exact
        }));
        assert!(contract.profiles.iter().any(|profile| {
            profile.profile_id == TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID
                && profile.support_posture
                    == TassadarMixedNumericSupportPosture::BoundedApproximate
        }));
    }
}
