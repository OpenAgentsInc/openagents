use psionic_runtime::{
    TassadarDeterministicImportSideEffectPolicy, TassadarHostImportStubKind,
    TassadarModuleExecutionRefusalKind, tassadar_module_execution_capability_report,
};
use serde::{Deserialize, Serialize};

/// Sandbox-facing projection of the bounded Tassadar host-import boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSandboxImportBoundary {
    /// Stable boundary identifier.
    pub boundary_id: String,
    /// Stub kinds admitted by the sandbox boundary.
    pub supported_stub_kinds: Vec<TassadarHostImportStubKind>,
    /// Side-effect posture for admitted deterministic stubs.
    pub side_effect_policy: TassadarDeterministicImportSideEffectPolicy,
    /// Typed refusal surfaced for unsupported host calls.
    pub unsupported_host_call_refusal: TassadarModuleExecutionRefusalKind,
    /// Plain-language capability summary.
    pub capability_summary: String,
}

/// Returns the sandbox-facing host-import boundary for the bounded module lane.
#[must_use]
pub fn tassadar_sandbox_import_boundary() -> TassadarSandboxImportBoundary {
    let capability = tassadar_module_execution_capability_report();
    TassadarSandboxImportBoundary {
        boundary_id: String::from("tassadar.sandbox_import_boundary.v1"),
        supported_stub_kinds: capability.host_import_boundary.supported_stub_kinds,
        side_effect_policy: capability.host_import_boundary.side_effect_policy,
        unsupported_host_call_refusal: capability
            .host_import_boundary
            .unsupported_host_call_refusal,
        capability_summary: capability.host_import_boundary.claim_boundary,
    }
}

#[cfg(test)]
mod tests {
    use super::{TassadarSandboxImportBoundary, tassadar_sandbox_import_boundary};
    use psionic_runtime::{
        TassadarDeterministicImportSideEffectPolicy, TassadarHostImportStubKind,
        TassadarModuleExecutionRefusalKind,
    };

    #[test]
    fn sandbox_import_boundary_is_machine_legible() {
        let boundary = tassadar_sandbox_import_boundary();
        assert_eq!(boundary.boundary_id, "tassadar.sandbox_import_boundary.v1");
        assert_eq!(
            boundary.supported_stub_kinds,
            vec![TassadarHostImportStubKind::DeterministicI32Const]
        );
        assert_eq!(
            boundary.side_effect_policy,
            TassadarDeterministicImportSideEffectPolicy::NoSideEffects
        );
        assert_eq!(
            boundary.unsupported_host_call_refusal,
            TassadarModuleExecutionRefusalKind::UnsupportedHostImport
        );
        let encoded = serde_json::to_value(&boundary).expect("boundary should serialize");
        assert_eq!(
            encoded["unsupported_host_call_refusal"],
            serde_json::json!("unsupported_host_import")
        );
    }

    #[test]
    fn sandbox_import_boundary_round_trips() {
        let boundary = tassadar_sandbox_import_boundary();
        let encoded = serde_json::to_vec(&boundary).expect("encode");
        let decoded: TassadarSandboxImportBoundary =
            serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(decoded, boundary);
    }
}
