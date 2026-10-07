use psionic_ir::TASSADAR_FROZEN_CORE_WASM_WINDOW_ID;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wasmparser::{Validator, WasmFeatures};

/// Overall closure status for the frozen core-Wasm target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFrozenCoreWasmClosureGateStatus {
    /// The declared window is closed at the current claim boundary.
    Closed,
    /// The declared window is not closed yet.
    NotClosed,
}

/// Row status inside the frozen core-Wasm closure gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFrozenCoreWasmClosureGateRowStatus {
    /// The gate row is satisfied.
    Green,
    /// The gate row is not satisfied.
    Red,
}

/// One machine-readable row inside the frozen core-Wasm closure gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmClosureGateRow {
    /// Stable row id.
    pub row_id: String,
    /// Human-readable description.
    pub description: String,
    /// Green or red row status.
    pub status: TassadarFrozenCoreWasmClosureGateRowStatus,
    /// Machine-readable detail.
    pub detail: String,
}

/// Typed refusal surface for the frozen core-Wasm validation harness.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarFrozenCoreWasmValidationError {
    /// One module used an out-of-window proposal or semantic family.
    #[error(
        "frozen core-Wasm window `{window_id}` refuses proposal family `{proposal_family_id}`: {detail}"
    )]
    ProposalFamilyUnsupported {
        /// Frozen semantic window id.
        window_id: String,
        /// Refused proposal family.
        proposal_family_id: String,
        /// Validator detail.
        detail: String,
    },
    /// One module was malformed or otherwise failed validation without a typed proposal family.
    #[error(
        "frozen core-Wasm window `{window_id}` rejected malformed or unsupported binary: {detail}"
    )]
    InvalidBinary {
        /// Frozen semantic window id.
        window_id: String,
        /// Validator detail.
        detail: String,
    },
}

/// Returns the validation feature set for the frozen int-first core-Wasm window.
#[must_use]
pub fn tassadar_frozen_core_wasm_validation_features() -> WasmFeatures {
    let mut features = WasmFeatures::empty();
    features.set(WasmFeatures::MUTABLE_GLOBAL, true);
    features.set(WasmFeatures::SIGN_EXTENSION, true);
    features.set(WasmFeatures::REFERENCE_TYPES, true);
    features.set(WasmFeatures::BULK_MEMORY, true);
    features
}

/// Validates one Wasm binary against the frozen core-Wasm window.
pub fn validate_tassadar_frozen_core_wasm_binary(
    bytes: &[u8],
) -> Result<(), TassadarFrozenCoreWasmValidationError> {
    let mut validator =
        Validator::new_with_features(tassadar_frozen_core_wasm_validation_features());
    validator.validate_all(bytes).map(|_| ()).map_err(|error| {
        let detail = error.to_string();
        if let Some(proposal_family_id) = classify_proposal_family(detail.as_str()) {
            TassadarFrozenCoreWasmValidationError::ProposalFamilyUnsupported {
                window_id: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_ID),
                proposal_family_id: String::from(proposal_family_id),
                detail,
            }
        } else {
            TassadarFrozenCoreWasmValidationError::InvalidBinary {
                window_id: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_ID),
                detail,
            }
        }
    })
}

fn classify_proposal_family(detail: &str) -> Option<&'static str> {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("float") {
        Some("floating_point")
    } else if lower.contains("multi-memory") || lower.contains("multiple memories") {
        Some("multi_memory")
    } else if lower.contains("simd") || lower.contains("v128") {
        Some("simd")
    } else if lower.contains("memory64") {
        Some("memory64")
    } else if lower.contains("exception") || lower.contains("try_table") || lower.contains("tag") {
        Some("exceptions")
    } else if lower.contains("tail call") || lower.contains("return_call") {
        Some("tail_call")
    } else if lower.contains("thread") || lower.contains("shared memory") {
        Some("threads")
    } else if lower.contains("component") {
        Some("component_model")
    } else if lower.contains("gc") || lower.contains("externref") || lower.contains("anyref") {
        Some("gc")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use psionic_ir::tassadar_frozen_core_wasm_window_v1;

    use super::{
        TassadarFrozenCoreWasmValidationError, tassadar_frozen_core_wasm_validation_features,
        validate_tassadar_frozen_core_wasm_binary,
    };

    #[test]
    fn frozen_core_wasm_validation_features_stay_int_first() {
        let features = tassadar_frozen_core_wasm_validation_features();
        assert!(features.contains(wasmparser::WasmFeatures::MUTABLE_GLOBAL));
        assert!(features.contains(wasmparser::WasmFeatures::REFERENCE_TYPES));
        assert!(features.contains(wasmparser::WasmFeatures::BULK_MEMORY));
        assert!(!features.contains(wasmparser::WasmFeatures::FLOATS));
        assert!(!features.contains(wasmparser::WasmFeatures::MULTI_MEMORY));
    }

    #[test]
    fn frozen_core_wasm_validation_refuses_seeded_out_of_window_proposals() {
        let fixtures = [
            (
                "floating_point",
                include_str!("../../../fixtures/tassadar/sources/tassadar_float_kernel.wat"),
            ),
            (
                "multi_memory",
                include_str!("../../../fixtures/tassadar/sources/tassadar_multi_memory_kernel.wat"),
            ),
        ];
        for (expected_proposal_family_id, source_text) in fixtures {
            let bytes = wat::parse_str(source_text).expect("seeded refusal fixture should encode");
            let error = validate_tassadar_frozen_core_wasm_binary(&bytes)
                .expect_err("seeded out-of-window proposal should refuse");
            assert!(matches!(
                error,
                TassadarFrozenCoreWasmValidationError::ProposalFamilyUnsupported {
                    proposal_family_id,
                    ..
                } if proposal_family_id == expected_proposal_family_id
            ));
        }
    }

    #[test]
    fn frozen_core_wasm_validation_accepts_seeded_positive_fixture() {
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("fixtures")
                .join("tassadar")
                .join("wasm")
                .join("tassadar_multi_export_kernel.wasm"),
        )
        .expect("positive wasm fixture should exist");
        validate_tassadar_frozen_core_wasm_binary(&bytes)
            .expect("positive wasm fixture should validate");
        let window = tassadar_frozen_core_wasm_window_v1();
        assert_eq!(window.window_id, "tassadar.frozen_core_wasm.window.v1");
    }
}
