use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable frozen semantic window id for the current core-Wasm closure target.
pub const TASSADAR_FROZEN_CORE_WASM_WINDOW_ID: &str = "tassadar.frozen_core_wasm.window.v1";
/// Stable authority id for the official text harness.
pub const TASSADAR_FROZEN_CORE_WASM_TEXT_AUTHORITY_ID: &str = "wat.reference.v1";
/// Stable authority id for the official binary decode harness.
pub const TASSADAR_FROZEN_CORE_WASM_BINARY_DECODE_AUTHORITY_ID: &str = "wasmparser.decode.0.244.0";
/// Stable authority id for the official binary encode harness.
pub const TASSADAR_FROZEN_CORE_WASM_BINARY_ENCODE_AUTHORITY_ID: &str =
    "wasm_encoder.encode.0.244.0";
/// Stable authority id for the official validation harness.
pub const TASSADAR_FROZEN_CORE_WASM_VALIDATION_AUTHORITY_ID: &str =
    "wasmparser.validate.core_int_first.v1";
/// Stable authority id for the official reference execution harness.
pub const TASSADAR_FROZEN_CORE_WASM_REFERENCE_EXECUTION_AUTHORITY_ID: &str = "wasmi.reference.v1";
/// Stable harness id binding text, binary, validation, and execution surfaces together.
pub const TASSADAR_FROZEN_CORE_WASM_OFFICIAL_HARNESS_ID: &str =
    "tassadar.frozen_core_wasm.harness.v1";

/// Shared declaration for the frozen core-Wasm window Tassadar is widening against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmWindow {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable semantic window id.
    pub window_id: String,
    /// Human-readable label for the target.
    pub spec_window_label: String,
    /// Stable official harness id.
    pub official_harness_id: String,
    /// Stable text authority id.
    pub text_authority_id: String,
    /// Stable binary decode authority id.
    pub binary_decode_authority_id: String,
    /// Stable binary encode authority id.
    pub binary_encode_authority_id: String,
    /// Stable validation authority id.
    pub validation_authority_id: String,
    /// Stable reference execution authority id.
    pub reference_execution_authority_id: String,
    /// Target feature families for the frozen window.
    pub target_feature_family_ids: Vec<String>,
    /// Explicit proposal or semantic families outside the frozen window.
    pub unsupported_proposal_family_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the window.
    pub window_digest: String,
}

impl TassadarFrozenCoreWasmWindow {
    fn new() -> Self {
        let mut window = Self {
            schema_version: 1,
            window_id: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_ID),
            spec_window_label: String::from("frozen core-Wasm int-first window v1"),
            official_harness_id: String::from(TASSADAR_FROZEN_CORE_WASM_OFFICIAL_HARNESS_ID),
            text_authority_id: String::from(TASSADAR_FROZEN_CORE_WASM_TEXT_AUTHORITY_ID),
            binary_decode_authority_id: String::from(
                TASSADAR_FROZEN_CORE_WASM_BINARY_DECODE_AUTHORITY_ID,
            ),
            binary_encode_authority_id: String::from(
                TASSADAR_FROZEN_CORE_WASM_BINARY_ENCODE_AUTHORITY_ID,
            ),
            validation_authority_id: String::from(
                TASSADAR_FROZEN_CORE_WASM_VALIDATION_AUTHORITY_ID,
            ),
            reference_execution_authority_id: String::from(
                TASSADAR_FROZEN_CORE_WASM_REFERENCE_EXECUTION_AUTHORITY_ID,
            ),
            target_feature_family_ids: vec![
                String::from("core_types.i32_i64"),
                String::from("control.structured_direct_call"),
                String::from("memory.single_linear_memory"),
                String::from("memory.bulk_copy_fill"),
                String::from("globals.mutable"),
                String::from("tables.funcref_call_indirect"),
                String::from("segments.active_data_and_element"),
                String::from("module.import_export_start"),
                String::from("numeric.sign_extension"),
            ],
            unsupported_proposal_family_ids: vec![
                String::from("floating_point"),
                String::from("saturating_float_to_int"),
                String::from("multi_value"),
                String::from("simd"),
                String::from("relaxed_simd"),
                String::from("exceptions"),
                String::from("tail_call"),
                String::from("function_references"),
                String::from("gc"),
                String::from("memory64"),
                String::from("multi_memory"),
                String::from("threads"),
                String::from("component_model"),
            ],
            claim_boundary: String::from(
                "declares the frozen core-Wasm semantic window and official harness ids Tassadar will use for closure work; it names one bounded int-first core target and explicit out-of-window proposal families, not current arbitrary Wasm support or full closure by itself",
            ),
            window_digest: String::new(),
        };
        window.target_feature_family_ids.sort();
        window.target_feature_family_ids.dedup();
        window.unsupported_proposal_family_ids.sort();
        window.unsupported_proposal_family_ids.dedup();
        window.window_digest = stable_digest(b"tassadar_frozen_core_wasm_window|", &window);
        window
    }
}

/// Returns the canonical frozen core-Wasm window declaration.
#[must_use]
pub fn tassadar_frozen_core_wasm_window_v1() -> TassadarFrozenCoreWasmWindow {
    TassadarFrozenCoreWasmWindow::new()
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
        TASSADAR_FROZEN_CORE_WASM_BINARY_DECODE_AUTHORITY_ID,
        TASSADAR_FROZEN_CORE_WASM_OFFICIAL_HARNESS_ID, TASSADAR_FROZEN_CORE_WASM_WINDOW_ID,
        tassadar_frozen_core_wasm_window_v1,
    };

    #[test]
    fn frozen_core_wasm_window_is_machine_legible() {
        let window = tassadar_frozen_core_wasm_window_v1();
        assert_eq!(window.window_id, TASSADAR_FROZEN_CORE_WASM_WINDOW_ID);
        assert_eq!(
            window.binary_decode_authority_id,
            TASSADAR_FROZEN_CORE_WASM_BINARY_DECODE_AUTHORITY_ID
        );
        assert_eq!(
            window.official_harness_id,
            TASSADAR_FROZEN_CORE_WASM_OFFICIAL_HARNESS_ID
        );
        assert!(
            window
                .target_feature_family_ids
                .contains(&String::from("memory.single_linear_memory"))
        );
        assert!(
            window
                .unsupported_proposal_family_ids
                .contains(&String::from("floating_point"))
        );
        assert!(!window.window_digest.is_empty());
    }
}
