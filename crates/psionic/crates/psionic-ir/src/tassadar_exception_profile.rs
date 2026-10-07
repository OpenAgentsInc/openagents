use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID: &str =
    "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1";
pub const TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID: &str =
    "cpu_reference_current_host";

/// Support posture for one exception-handling proposal profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExceptionProfileSupportPosture {
    Exact,
}

/// One declared profile in the bounded exception-handling ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileSpec {
    pub profile_id: String,
    pub support_posture: TassadarExceptionProfileSupportPosture,
    pub admitted_semantic_ids: Vec<String>,
    pub refused_reason_ids: Vec<String>,
    pub portability_envelope_id: String,
    pub detail: String,
}

/// Public contract for the bounded exception-handling profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profiles: Vec<TassadarExceptionProfileSpec>,
    pub refused_exception_family_ids: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarExceptionProfileContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.exception_profile.contract.v1"),
            profiles: vec![TassadarExceptionProfileSpec {
                profile_id: String::from(TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID),
                support_posture: TassadarExceptionProfileSupportPosture::Exact,
                admitted_semantic_ids: vec![
                    String::from("throw_catch_success"),
                    String::from("nested_rethrow_trap_stack"),
                    String::from("handler_tag_mismatch_trap"),
                ],
                refused_reason_ids: vec![
                    String::from("malformed_exception_handler"),
                    String::from("exception_delegate_out_of_scope"),
                    String::from("exception_try_table_out_of_scope"),
                ],
                portability_envelope_id: String::from(
                    TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
                ),
                detail: String::from(
                    "the current exceptions profile admits typed throw/catch/rethrow semantics only on the current-host cpu-reference lane and keeps delegate, try_table, and broader proposal-family widening on explicit refusal paths",
                ),
            }],
            refused_exception_family_ids: vec![
                String::from("arbitrary_exception_handling"),
                String::from("generic_wasm_exception_closure"),
                String::from("backend_invariant_exception_portability"),
            ],
            claim_boundary: String::from(
                "this contract names one bounded exception-handling profile over typed throw, catch, and rethrow parity on the current-host cpu-reference lane. It does not claim arbitrary Wasm exception closure, delegate or try_table support, backend-invariant portability, or broader served publication",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_exception_profile_contract|", &contract);
        contract
    }
}

/// Returns the canonical exception-handling profile contract.
#[must_use]
pub fn tassadar_exception_profile_contract() -> TassadarExceptionProfileContract {
    TassadarExceptionProfileContract::new()
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
        TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID, TassadarExceptionProfileSupportPosture,
        tassadar_exception_profile_contract,
    };

    #[test]
    fn exception_profile_contract_is_machine_legible() {
        let contract = tassadar_exception_profile_contract();

        assert_eq!(contract.profiles.len(), 1);
        assert!(contract.profiles.iter().any(|profile| {
            profile.profile_id == TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID
                && profile.support_posture == TassadarExceptionProfileSupportPosture::Exact
                && profile.portability_envelope_id
                    == TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
