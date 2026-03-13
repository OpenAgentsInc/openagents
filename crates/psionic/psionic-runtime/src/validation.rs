use serde::{Deserialize, Serialize};

use crate::{BackendSelection, BackendSelectionState};

/// Stable identifier for the minimum hardware validation profile that backs
/// current Psionic support claims.
pub const MINIMUM_HARDWARE_VALIDATION_MATRIX_ID: &str = "psionic.minimum_hardware_validation.v1";

/// Canonical documentation path for the minimum hardware validation profile.
pub const MINIMUM_HARDWARE_VALIDATION_DOC_PATH: &str =
    "crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md";

/// Coverage posture for one validation claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCoverage {
    /// The claim is backed by a positive execution path that must stay green.
    PositiveExecution,
    /// The claim is backed by an explicit fallback/refusal path that must stay green.
    ExplicitRefusal,
    /// The claim is currently limited to backend discovery/readiness truth.
    DiscoveryReadiness,
    /// The claim is outside the current minimum matrix and must not be treated as shipped.
    NotYetValidated,
}

/// Provider-facing pointer back to the minimum validation profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationMatrixReference {
    /// Stable validation profile identifier.
    pub matrix_id: String,
    /// Repo path for the human-readable validation matrix.
    pub documentation_path: String,
    /// Stable claim identifier within the validation matrix.
    pub claim_id: String,
    /// Coverage posture for the claim.
    pub coverage: ValidationCoverage,
}

impl ValidationMatrixReference {
    /// Creates a reference to a claim in the minimum hardware validation profile.
    #[must_use]
    pub fn minimum(claim_id: impl Into<String>, coverage: ValidationCoverage) -> Self {
        Self {
            matrix_id: String::from(MINIMUM_HARDWARE_VALIDATION_MATRIX_ID),
            documentation_path: String::from(MINIMUM_HARDWARE_VALIDATION_DOC_PATH),
            claim_id: claim_id.into(),
            coverage,
        }
    }

    /// Creates an explicit reference for a backend/product pair that is not yet
    /// covered by the minimum shipped validation matrix.
    #[must_use]
    pub fn not_yet_validated(claim_id: impl Into<String>) -> Self {
        Self::minimum(claim_id, ValidationCoverage::NotYetValidated)
    }
}

/// One machine-checkable claim in the minimum hardware validation profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HardwareValidationClaim {
    /// Stable claim identifier carried in provider capability and receipt surfaces.
    pub claim_id: &'static str,
    /// Stable backend label for the claim.
    pub backend: &'static str,
    /// Stable product identifier when the claim belongs to a served product path.
    pub product_id: Option<&'static str>,
    /// Coverage posture for the claim.
    pub coverage: ValidationCoverage,
    /// Human-readable hardware lane bounded by the claim.
    pub hardware_lane: &'static str,
    /// Test anchors that currently keep the claim green.
    pub required_tests: &'static [&'static str],
    /// Short note describing the support claim.
    pub notes: &'static str,
}

const CPU_EMBEDDINGS_TESTS: &[&str] = &[
    "psionic-serve/tests/model_backed_embeddings.rs::model_backed_embeddings_flow_returns_response_capability_and_receipt",
];
const CPU_TEXT_GENERATION_TESTS: &[&str] = &[
    "psionic-serve/tests/model_backed_text_generation.rs::model_backed_text_generation_flow_returns_response_capability_and_receipt",
];
const METAL_EMBEDDINGS_TESTS: &[&str] = &[
    "psionic-serve/tests/metal_model_backed_embeddings.rs::metal_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability",
    "psionic-serve/tests/metal_embeddings_parity.rs::metal_model_backed_embeddings_match_cpu_baseline_within_tolerance_on_ready_hardware",
];
const METAL_TEXT_GENERATION_TESTS: &[&str] = &[
    "psionic-serve/tests/metal_model_backed_text_generation.rs::metal_model_backed_text_generation_returns_response_capability_and_receipt_or_explicit_unavailability",
    "psionic-serve/tests/metal_text_generation_parity.rs::metal_text_generation_matches_cpu_baseline_within_budget_and_seeded_sampling",
];
const METAL_GPT_OSS_TEXT_GENERATION_TESTS: &[&str] = &[
    "psionic-serve/src/gpt_oss.rs::tests::metal_gpt_oss_service_matches_cpu_reference_on_synthetic_fixture",
    "psionic-provider/src/lib.rs::metal_gpt_oss_text_generation_capability_reports_explicit_validation",
    "psionic-provider/src/lib.rs::metal_gpt_oss_text_generation_receipt_reports_explicit_validation",
];
const METAL_REFUSAL_TESTS: &[&str] = &[
    "psionic-serve/tests/metal_embeddings_parity.rs::metal_model_backed_embeddings_parity_reports_explicit_offline_state",
    "psionic-serve/tests/metal_text_generation_parity.rs::metal_text_generation_parity_reports_explicit_offline_state",
    "psionic-serve/src/gpt_oss.rs::tests::metal_gpt_oss_service_reports_backend_unavailable_off_platform",
    "psionic-provider/src/lib.rs::metal_gpt_oss_text_generation_fallback_capability_reports_explicit_refusal_validation",
    "psionic-provider/src/lib.rs::metal_gpt_oss_text_generation_failed_receipt_reports_explicit_refusal_validation",
];
const CUDA_EMBEDDINGS_TESTS: &[&str] = &[
    "psionic-serve/tests/cuda_model_backed_embeddings.rs::cuda_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability",
    "psionic-serve/tests/cuda_embeddings_parity.rs::cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback",
];
const CUDA_REFUSAL_TESTS: &[&str] = &[
    "psionic-serve/tests/cuda_embeddings_parity.rs::cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback",
];
const AMD_KFD_DISCOVERY_TESTS: &[&str] =
    &["psionic-backend-amd-kfd/src/lib.rs::amd_kfd_report_is_self_consistent_on_linux"];
const AMD_USERSPACE_REFUSAL_TESTS: &[&str] = &[
    "psionic-backend-amd-userspace/src/lib.rs::userspace_health_is_offline_when_disabled",
    "psionic-provider/src/lib.rs::amd_userspace_capability_reports_disabled_risk_posture",
];

const MINIMUM_HARDWARE_VALIDATION_CLAIMS: &[HardwareValidationClaim] = &[
    HardwareValidationClaim {
        claim_id: "cpu.embeddings.reference",
        backend: "cpu",
        product_id: Some("psionic.embeddings"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "host_cpu_x86_64_or_aarch64",
        required_tests: CPU_EMBEDDINGS_TESTS,
        notes: "Reference embeddings path that must stay green on ordinary host CPUs.",
    },
    HardwareValidationClaim {
        claim_id: "cpu.text_generation.reference",
        backend: "cpu",
        product_id: Some("psionic.text_generation"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "host_cpu_x86_64_or_aarch64",
        required_tests: CPU_TEXT_GENERATION_TESTS,
        notes: "Reference text-generation path that anchors parity and fallback work.",
    },
    HardwareValidationClaim {
        claim_id: "metal.embeddings.apple_silicon",
        backend: "metal",
        product_id: Some("psionic.embeddings"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "apple_silicon_metal_gpu_family_apple1_to_apple9_or_common3_plus",
        required_tests: METAL_EMBEDDINGS_TESTS,
        notes: "Apple Silicon embeddings path validated against the CPU baseline.",
    },
    HardwareValidationClaim {
        claim_id: "metal.text_generation.apple_silicon",
        backend: "metal",
        product_id: Some("psionic.text_generation"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "apple_silicon_metal_gpu_family_apple1_to_apple9_or_common3_plus",
        required_tests: METAL_TEXT_GENERATION_TESTS,
        notes: "Apple Silicon text-generation path validated against the CPU baseline for the dense artifact-backed decoder lane, not for GPT-OSS/OpenAI-MoE.",
    },
    HardwareValidationClaim {
        claim_id: "metal.gpt_oss.text_generation.apple_silicon",
        backend: "metal",
        product_id: Some("psionic.text_generation"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "apple_silicon_metal_gpu_family_apple1_to_apple9_or_common3_plus",
        required_tests: METAL_GPT_OSS_TEXT_GENERATION_TESTS,
        notes: "Apple Silicon Metal GGUF GPT-OSS lane validated against the CPU reference path, with benchmark evidence tracked separately for cold, warm, and prompt-cache-hit cases.",
    },
    HardwareValidationClaim {
        claim_id: "metal.refusal.off_platform",
        backend: "metal",
        product_id: None,
        coverage: ValidationCoverage::ExplicitRefusal,
        hardware_lane: "non_macos_hosts_or_legacy_only_metal_devices",
        required_tests: METAL_REFUSAL_TESTS,
        notes: "Metal support claims must fall back or refuse explicitly when the platform is not ready.",
    },
    HardwareValidationClaim {
        claim_id: "cuda.embeddings.nvidia",
        backend: "cuda",
        product_id: Some("psionic.embeddings"),
        coverage: ValidationCoverage::PositiveExecution,
        hardware_lane: "linux_nvidia_cuda_device",
        required_tests: CUDA_EMBEDDINGS_TESTS,
        notes: "First NVIDIA-backed served product path validated against the CPU baseline.",
    },
    HardwareValidationClaim {
        claim_id: "cuda.refusal.unavailable",
        backend: "cuda",
        product_id: None,
        coverage: ValidationCoverage::ExplicitRefusal,
        hardware_lane: "hosts_without_usable_cuda_runtime_or_nvidia_driver",
        required_tests: CUDA_REFUSAL_TESTS,
        notes: "CUDA support claims must degrade or fall back explicitly when NVIDIA execution is unavailable.",
    },
    HardwareValidationClaim {
        claim_id: "amd_kfd.discovery",
        backend: "amd_kfd",
        product_id: None,
        coverage: ValidationCoverage::DiscoveryReadiness,
        hardware_lane: "linux_amd_amdgpu_plus_kfd",
        required_tests: AMD_KFD_DISCOVERY_TESTS,
        notes: "AMD KFD is currently validated as a discovery/readiness lane, not a served-product execution lane.",
    },
    HardwareValidationClaim {
        claim_id: "amd_userspace.refusal",
        backend: "amd_userspace",
        product_id: None,
        coverage: ValidationCoverage::ExplicitRefusal,
        hardware_lane: "linux_amd_userspace_opt_in_disabled_or_kernel_driver_still_bound",
        required_tests: AMD_USERSPACE_REFUSAL_TESTS,
        notes: "AMD userspace remains explicitly gated behind elevated-risk opt-in and refusal/degraded posture.",
    },
];

/// Returns the full minimum hardware validation profile.
#[must_use]
pub const fn minimum_hardware_validation_claims() -> &'static [HardwareValidationClaim] {
    MINIMUM_HARDWARE_VALIDATION_CLAIMS
}

/// Returns one claim from the minimum hardware validation profile.
#[must_use]
pub fn minimum_hardware_validation_claim(
    claim_id: &str,
) -> Option<&'static HardwareValidationClaim> {
    MINIMUM_HARDWARE_VALIDATION_CLAIMS
        .iter()
        .find(|claim| claim.claim_id == claim_id)
}

/// Returns the validation reference for one current served-product claim.
#[must_use]
pub fn validation_reference_for_served_product(
    backend_selection: &BackendSelection,
    product_id: &str,
) -> ValidationMatrixReference {
    let refusal_state = matches!(
        backend_selection.selection_state,
        BackendSelectionState::CrossBackendFallback | BackendSelectionState::Refused
    );
    match (
        backend_selection.requested_backend.as_str(),
        product_label(product_id),
        refusal_state,
    ) {
        ("cpu", "embeddings", _) => ValidationMatrixReference::minimum(
            "cpu.embeddings.reference",
            ValidationCoverage::PositiveExecution,
        ),
        ("cpu", "text_generation", _) => ValidationMatrixReference::minimum(
            "cpu.text_generation.reference",
            ValidationCoverage::PositiveExecution,
        ),
        ("metal", "embeddings", true) => ValidationMatrixReference::minimum(
            "metal.refusal.off_platform",
            ValidationCoverage::ExplicitRefusal,
        ),
        ("metal", "embeddings", false) => ValidationMatrixReference::minimum(
            "metal.embeddings.apple_silicon",
            ValidationCoverage::PositiveExecution,
        ),
        ("metal", "text_generation", true) => ValidationMatrixReference::minimum(
            "metal.refusal.off_platform",
            ValidationCoverage::ExplicitRefusal,
        ),
        ("metal", "text_generation", false) => ValidationMatrixReference::minimum(
            "metal.text_generation.apple_silicon",
            ValidationCoverage::PositiveExecution,
        ),
        ("cuda", "embeddings", true) => ValidationMatrixReference::minimum(
            "cuda.refusal.unavailable",
            ValidationCoverage::ExplicitRefusal,
        ),
        ("cuda", "embeddings", false) => ValidationMatrixReference::minimum(
            "cuda.embeddings.nvidia",
            ValidationCoverage::PositiveExecution,
        ),
        _ => ValidationMatrixReference::not_yet_validated(format!(
            "{}.{}.not_yet_validated",
            backend_selection.requested_backend,
            product_label(product_id),
        )),
    }
}

/// Returns the validation reference for one text-generation model family.
#[must_use]
pub fn validation_reference_for_text_generation_model(
    backend_selection: &BackendSelection,
    model_family: &str,
) -> ValidationMatrixReference {
    let normalized_family = model_family.to_ascii_lowercase();
    let refusal_state = matches!(
        backend_selection.selection_state,
        BackendSelectionState::CrossBackendFallback | BackendSelectionState::Refused
    );

    match (
        backend_selection.requested_backend.as_str(),
        refusal_state,
        normalized_family.as_str(),
    ) {
        ("metal", true, _) => ValidationMatrixReference::minimum(
            "metal.refusal.off_platform",
            ValidationCoverage::ExplicitRefusal,
        ),
        ("metal", false, "gpt-oss" | "gptoss") => ValidationMatrixReference::minimum(
            "metal.gpt_oss.text_generation.apple_silicon",
            ValidationCoverage::PositiveExecution,
        ),
        _ => validation_reference_for_served_product(backend_selection, "psionic.text_generation"),
    }
}

/// Returns the validation reference for one backend probe claim.
#[must_use]
pub fn validation_reference_for_backend_probe(backend: &str) -> ValidationMatrixReference {
    match backend {
        "amd_kfd" => ValidationMatrixReference::minimum(
            "amd_kfd.discovery",
            ValidationCoverage::DiscoveryReadiness,
        ),
        "amd_userspace" => ValidationMatrixReference::minimum(
            "amd_userspace.refusal",
            ValidationCoverage::ExplicitRefusal,
        ),
        _ => ValidationMatrixReference::not_yet_validated(format!(
            "{backend}.probe.not_yet_validated"
        )),
    }
}

fn product_label(product_id: &str) -> &str {
    match product_id {
        "psionic.embeddings" => "embeddings",
        "psionic.text_generation" => "text_generation",
        _ => "unknown_product",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        MINIMUM_HARDWARE_VALIDATION_MATRIX_ID, ValidationCoverage,
        minimum_hardware_validation_claim, minimum_hardware_validation_claims,
        validation_reference_for_backend_probe, validation_reference_for_served_product,
        validation_reference_for_text_generation_model,
    };
    use crate::{BackendSelection, DeviceDescriptor};

    fn direct_selection(backend: &str) -> BackendSelection {
        BackendSelection::direct(backend, Option::<DeviceDescriptor>::None, Vec::new())
    }

    fn fallback_selection(requested_backend: &str) -> BackendSelection {
        BackendSelection::fallback(
            requested_backend,
            "cpu",
            Option::<DeviceDescriptor>::None,
            Vec::new(),
            format!("{requested_backend} unavailable"),
        )
    }

    #[test]
    fn minimum_hardware_validation_matrix_covers_required_lanes() {
        let claims = minimum_hardware_validation_claims();
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "cpu.embeddings.reference")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "cpu.text_generation.reference")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "metal.embeddings.apple_silicon")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "metal.text_generation.apple_silicon")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "metal.gpt_oss.text_generation.apple_silicon")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "cuda.embeddings.nvidia")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.claim_id == "amd_kfd.discovery")
        );
        assert!(
            claims
                .iter()
                .any(|claim| claim.coverage == ValidationCoverage::ExplicitRefusal)
        );
    }

    #[test]
    fn shipped_served_product_claims_map_to_the_minimum_matrix() {
        let cpu_embeddings =
            validation_reference_for_served_product(&direct_selection("cpu"), "psionic.embeddings");
        assert_eq!(
            cpu_embeddings.matrix_id,
            MINIMUM_HARDWARE_VALIDATION_MATRIX_ID
        );
        assert_eq!(cpu_embeddings.claim_id, "cpu.embeddings.reference");
        assert_eq!(
            cpu_embeddings.coverage,
            ValidationCoverage::PositiveExecution
        );

        let cpu_text = validation_reference_for_served_product(
            &direct_selection("cpu"),
            "psionic.text_generation",
        );
        assert_eq!(cpu_text.claim_id, "cpu.text_generation.reference");

        let metal_embeddings = validation_reference_for_served_product(
            &direct_selection("metal"),
            "psionic.embeddings",
        );
        assert_eq!(metal_embeddings.claim_id, "metal.embeddings.apple_silicon");

        let metal_refusal = validation_reference_for_served_product(
            &fallback_selection("metal"),
            "psionic.text_generation",
        );
        assert_eq!(metal_refusal.claim_id, "metal.refusal.off_platform");
        assert_eq!(metal_refusal.coverage, ValidationCoverage::ExplicitRefusal);

        let cuda_embeddings = validation_reference_for_served_product(
            &direct_selection("cuda"),
            "psionic.embeddings",
        );
        assert_eq!(cuda_embeddings.claim_id, "cuda.embeddings.nvidia");

        let cuda_refusal = validation_reference_for_served_product(
            &fallback_selection("cuda"),
            "psionic.embeddings",
        );
        assert_eq!(cuda_refusal.claim_id, "cuda.refusal.unavailable");
        assert_eq!(cuda_refusal.coverage, ValidationCoverage::ExplicitRefusal);
    }

    #[test]
    fn metal_gpt_oss_text_generation_maps_to_explicit_matrix_claim() {
        let reference =
            validation_reference_for_text_generation_model(&direct_selection("metal"), "gpt-oss");
        assert_eq!(reference.coverage, ValidationCoverage::PositiveExecution);
        assert_eq!(
            reference.claim_id,
            "metal.gpt_oss.text_generation.apple_silicon"
        );
        assert_eq!(
            minimum_hardware_validation_claim(&reference.claim_id)
                .expect("matrix claim")
                .coverage,
            ValidationCoverage::PositiveExecution
        );
    }

    #[test]
    fn metal_gpt_oss_fallback_maps_to_explicit_refusal_claim() {
        let reference =
            validation_reference_for_text_generation_model(&fallback_selection("metal"), "gpt-oss");
        assert_eq!(reference.coverage, ValidationCoverage::ExplicitRefusal);
        assert_eq!(reference.claim_id, "metal.refusal.off_platform");
        assert_eq!(
            minimum_hardware_validation_claim(&reference.claim_id)
                .expect("matrix claim")
                .coverage,
            ValidationCoverage::ExplicitRefusal
        );
    }

    #[test]
    fn non_gpt_oss_metal_text_generation_keeps_dense_claim() {
        let reference = validation_reference_for_text_generation_model(
            &direct_selection("metal"),
            "wordpiece_decoder",
        );
        assert_eq!(reference.coverage, ValidationCoverage::PositiveExecution);
        assert_eq!(reference.claim_id, "metal.text_generation.apple_silicon");
    }

    #[test]
    fn non_matrix_backend_product_pairs_are_marked_not_yet_validated() {
        let reference = validation_reference_for_served_product(
            &direct_selection("cuda"),
            "psionic.text_generation",
        );
        assert_eq!(reference.coverage, ValidationCoverage::NotYetValidated);
        assert_eq!(reference.claim_id, "cuda.text_generation.not_yet_validated");
        assert!(minimum_hardware_validation_claim(&reference.claim_id).is_none());
    }

    #[test]
    fn backend_probe_references_expose_current_amd_claims() {
        let amd_kfd = validation_reference_for_backend_probe("amd_kfd");
        assert_eq!(amd_kfd.claim_id, "amd_kfd.discovery");
        assert_eq!(amd_kfd.coverage, ValidationCoverage::DiscoveryReadiness);

        let amd_userspace = validation_reference_for_backend_probe("amd_userspace");
        assert_eq!(amd_userspace.claim_id, "amd_userspace.refusal");
        assert_eq!(amd_userspace.coverage, ValidationCoverage::ExplicitRefusal);
    }
}
