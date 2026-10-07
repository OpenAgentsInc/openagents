use psionic_models::TassadarWorkloadClass;
use psionic_runtime::{
    TassadarDirectModelWeightRouteBinding, TassadarDirectModelWeightRoutePosture,
    TassadarExecutorDecodeMode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable routed product identifier for planner-owned executor delegation.
pub const TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID: &str = "psionic.planner_executor_route";
/// Stable delegated executor product identifier.
pub const TASSADAR_EXECUTOR_TRACE_PRODUCT_ID: &str = "psionic.executor_trace";

/// Route posture advertised for one requested decode mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerExecutorRoutePosture {
    /// Route can stay on a direct executor path for this decode mode.
    DirectGuaranteed,
    /// Route can accept the decode mode but may fall back at runtime.
    FallbackCapable,
}

/// Typed refusal reason surfaced during route negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerExecutorRouteRefusalReason {
    UnsupportedProduct,
    UnknownModel,
    ProviderNotReady,
    BenchmarkGateMissing,
    DecodeModeUnsupported,
    RuntimeFallbackDisallowed,
    DirectDecodeRequired,
    WasmModuleClassUnsupported,
    WasmOpcodeFamilyUnsupported,
    WasmImportPostureUnsupported,
}

/// Public per-decode capability row for one planner / executor route surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorDecodeCapability {
    /// Requested decode mode this row describes.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Guaranteed-direct or explicit-fallback-capable posture.
    pub route_posture: TassadarPlannerExecutorRoutePosture,
    /// Stable benchmark report backing this route posture.
    pub benchmark_report_ref: String,
    /// Plain-language note describing the support boundary.
    pub note: String,
}

/// Routeable Wasm opcode families published for one planner / executor route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerExecutorWasmOpcodeFamily {
    CoreI32Arithmetic,
    StructuredControl,
    LinearMemoryV2,
    DirectCallFrames,
}

impl TassadarPlannerExecutorWasmOpcodeFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoreI32Arithmetic => "core_i32_arithmetic",
            Self::StructuredControl => "structured_control",
            Self::LinearMemoryV2 => "linear_memory_v2",
            Self::DirectCallFrames => "direct_call_frames",
        }
    }
}

/// Import posture published for one routeable Wasm module class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerExecutorWasmImportPosture {
    NoImportsOnly,
    DeterministicStubImportsOnly,
}

impl TassadarPlannerExecutorWasmImportPosture {
    /// Returns the stable import-posture label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoImportsOnly => "no_imports_only",
            Self::DeterministicStubImportsOnly => "deterministic_stub_imports_only",
        }
    }
}

/// One routeable Wasm module-class row for planner / executor negotiation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorWasmCapabilityRow {
    /// Stable Wasm module class.
    pub module_class: TassadarWorkloadClass,
    /// Requested decode modes the route publishes for this module class.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Requested decode modes that stay direct for this module class.
    pub direct_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Exact fallback decode mode when this module class stays served only by an explicit slower path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Routeable opcode families surfaced for this module class.
    pub opcode_families: Vec<TassadarPlannerExecutorWasmOpcodeFamily>,
    /// Import posture for the published module class.
    pub import_posture: TassadarPlannerExecutorWasmImportPosture,
    /// Stable benchmark report backing this row when it is a served claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_report_ref: Option<String>,
    /// Typed refusal reasons that remain explicit for this row.
    pub refusal_reasons: Vec<TassadarPlannerExecutorRouteRefusalReason>,
    /// Plain-language support boundary for the row.
    pub note: String,
}

impl TassadarPlannerExecutorWasmCapabilityRow {
    /// Creates one routeable Wasm module-class row.
    #[must_use]
    pub fn new(
        module_class: TassadarWorkloadClass,
        mut supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
        mut direct_decode_modes: Vec<TassadarExecutorDecodeMode>,
        exact_fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
        mut opcode_families: Vec<TassadarPlannerExecutorWasmOpcodeFamily>,
        import_posture: TassadarPlannerExecutorWasmImportPosture,
        benchmark_report_ref: Option<String>,
        mut refusal_reasons: Vec<TassadarPlannerExecutorRouteRefusalReason>,
        note: impl Into<String>,
    ) -> Self {
        supported_decode_modes.sort_by_key(|mode| mode.as_str());
        supported_decode_modes.dedup();
        direct_decode_modes.sort_by_key(|mode| mode.as_str());
        direct_decode_modes.dedup();
        direct_decode_modes.retain(|mode| supported_decode_modes.contains(mode));
        opcode_families.sort_by_key(|family| family.as_str());
        opcode_families.dedup();
        refusal_reasons.sort_by_key(|reason| serde_json::to_string(reason).unwrap_or_default());
        refusal_reasons.dedup();
        Self {
            module_class,
            supported_decode_modes,
            direct_decode_modes,
            exact_fallback_decode_mode,
            opcode_families,
            import_posture,
            benchmark_report_ref,
            refusal_reasons,
            note: note.into(),
        }
    }

    fn supports_required_opcode_families(
        &self,
        required_opcode_families: &[TassadarPlannerExecutorWasmOpcodeFamily],
    ) -> bool {
        required_opcode_families
            .iter()
            .all(|family| self.opcode_families.contains(family))
    }

    fn decode_disposition(
        &self,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Option<TassadarPlannerExecutorWasmDecodeDisposition> {
        if self.direct_decode_modes.contains(&requested_decode_mode) {
            Some(TassadarPlannerExecutorWasmDecodeDisposition::Direct)
        } else if self.supported_decode_modes.contains(&requested_decode_mode) {
            self.exact_fallback_decode_mode
                .map(|effective_decode_mode| {
                    TassadarPlannerExecutorWasmDecodeDisposition::Fallback {
                        effective_decode_mode,
                    }
                })
        } else {
            None
        }
    }
}

/// Routeable Wasm capability matrix for one benchmark-gated Tassadar lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorWasmCapabilityMatrix {
    /// Stable matrix identifier.
    pub matrix_id: String,
    /// Stable model identifier owning the matrix.
    pub model_id: String,
    /// Published Wasm module-class rows.
    pub rows: Vec<TassadarPlannerExecutorWasmCapabilityRow>,
    /// Plain-language matrix boundary.
    pub claim_boundary: String,
    /// Stable digest over the full matrix.
    pub matrix_digest: String,
}

impl TassadarPlannerExecutorWasmCapabilityMatrix {
    /// Creates one routeable Wasm capability matrix.
    #[must_use]
    pub fn new(
        matrix_id: impl Into<String>,
        model_id: impl Into<String>,
        mut rows: Vec<TassadarPlannerExecutorWasmCapabilityRow>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        rows.sort_by_key(|row| row.module_class.as_str());
        let mut matrix = Self {
            matrix_id: matrix_id.into(),
            model_id: model_id.into(),
            rows,
            claim_boundary: claim_boundary.into(),
            matrix_digest: String::new(),
        };
        matrix.matrix_digest = stable_digest(
            b"psionic_tassadar_planner_executor_wasm_capability_matrix|",
            &matrix,
        );
        matrix
    }

    /// Returns the row for one requested module class.
    #[must_use]
    pub fn row(
        &self,
        module_class: TassadarWorkloadClass,
    ) -> Option<&TassadarPlannerExecutorWasmCapabilityRow> {
        self.rows
            .iter()
            .find(|row| row.module_class == module_class)
    }
}

/// Routeable served capability descriptor for one benchmark-gated Tassadar lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorRouteDescriptor {
    /// Stable route identifier.
    pub route_id: String,
    /// Planner-owned routed product identifier.
    pub product_id: String,
    /// Delegated exact executor product identifier.
    pub executor_product_id: String,
    /// Stable executor model identifier exposed by the route.
    pub model_id: String,
    /// Stable benchmark report backing the served route.
    pub benchmark_report_ref: String,
    /// Named internal-compute profile exported by the route.
    pub internal_compute_profile_id: String,
    /// Stable claim digest for the named internal-compute profile exported by the route.
    pub internal_compute_profile_claim_digest: String,
    /// Stable digest of the served workload capability matrix.
    pub workload_capability_digest: String,
    /// Routeable Wasm capability matrix for explicit module-class routing.
    pub wasm_capability_matrix: TassadarPlannerExecutorWasmCapabilityMatrix,
    /// Explicit decode posture published for route negotiation.
    pub decode_capabilities: Vec<TassadarPlannerExecutorDecodeCapability>,
    /// Typed refusal reasons this route may emit during negotiation.
    pub refusal_reasons: Vec<TassadarPlannerExecutorRouteRefusalReason>,
    /// Plain-language note for the route surface.
    pub note: String,
    /// Stable digest over the full route descriptor.
    pub descriptor_digest: String,
}

impl TassadarPlannerExecutorRouteDescriptor {
    /// Creates one routeable served descriptor.
    #[must_use]
    pub fn new(
        route_id: impl Into<String>,
        model_id: impl Into<String>,
        benchmark_report_ref: impl Into<String>,
        internal_compute_profile_id: impl Into<String>,
        internal_compute_profile_claim_digest: impl Into<String>,
        workload_capability_digest: impl Into<String>,
        wasm_capability_matrix: TassadarPlannerExecutorWasmCapabilityMatrix,
        mut decode_capabilities: Vec<TassadarPlannerExecutorDecodeCapability>,
        mut refusal_reasons: Vec<TassadarPlannerExecutorRouteRefusalReason>,
        note: impl Into<String>,
    ) -> Self {
        decode_capabilities.sort_by_key(|capability| capability.requested_decode_mode.as_str());
        refusal_reasons.sort_by_key(|reason| serde_json::to_string(reason).unwrap_or_default());
        refusal_reasons.dedup();
        let mut descriptor = Self {
            route_id: route_id.into(),
            product_id: String::from(TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID),
            executor_product_id: String::from(TASSADAR_EXECUTOR_TRACE_PRODUCT_ID),
            model_id: model_id.into(),
            benchmark_report_ref: benchmark_report_ref.into(),
            internal_compute_profile_id: internal_compute_profile_id.into(),
            internal_compute_profile_claim_digest: internal_compute_profile_claim_digest.into(),
            workload_capability_digest: workload_capability_digest.into(),
            wasm_capability_matrix,
            decode_capabilities,
            refusal_reasons,
            note: note.into(),
            descriptor_digest: String::new(),
        };
        descriptor.descriptor_digest = stable_digest(
            b"psionic_tassadar_planner_executor_route_descriptor|",
            &descriptor,
        );
        descriptor
    }
}

/// Route-binding failure for the direct model-weight execution proof.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarDirectModelWeightRouteBindingError {
    /// The requested decode mode is absent from the published route descriptor.
    #[error(
        "route `{route_descriptor_digest}` does not publish decode `{requested_decode_mode:?}` for direct model-weight proof"
    )]
    DecodeModeMissing {
        route_descriptor_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
    },
    /// The route publishes the decode mode only with fallback-capable posture.
    #[error(
        "route `{route_descriptor_digest}` publishes decode `{requested_decode_mode:?}` as `{route_posture:?}` and cannot close direct model-weight proof"
    )]
    RouteNotDirectGuaranteed {
        route_descriptor_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        route_posture: TassadarPlannerExecutorRoutePosture,
    },
}

/// Binds one served route descriptor into the runtime-owned direct-proof route surface.
pub fn bind_tassadar_direct_model_weight_route(
    route_descriptor: &TassadarPlannerExecutorRouteDescriptor,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<TassadarDirectModelWeightRouteBinding, TassadarDirectModelWeightRouteBindingError> {
    let capability = route_descriptor
        .decode_capabilities
        .iter()
        .find(|capability| capability.requested_decode_mode == requested_decode_mode)
        .ok_or_else(
            || TassadarDirectModelWeightRouteBindingError::DecodeModeMissing {
                route_descriptor_digest: route_descriptor.descriptor_digest.clone(),
                requested_decode_mode,
            },
        )?;
    if capability.route_posture != TassadarPlannerExecutorRoutePosture::DirectGuaranteed {
        return Err(
            TassadarDirectModelWeightRouteBindingError::RouteNotDirectGuaranteed {
                route_descriptor_digest: route_descriptor.descriptor_digest.clone(),
                requested_decode_mode,
                route_posture: capability.route_posture,
            },
        );
    }
    Ok(TassadarDirectModelWeightRouteBinding {
        route_product_id: route_descriptor.product_id.clone(),
        route_id: route_descriptor.route_id.clone(),
        route_descriptor_digest: route_descriptor.descriptor_digest.clone(),
        route_model_id: route_descriptor.model_id.clone(),
        benchmark_report_ref: capability.benchmark_report_ref.clone(),
        requested_decode_mode: capability.requested_decode_mode,
        route_posture: TassadarDirectModelWeightRoutePosture::DirectGuaranteed,
        note: capability.note.clone(),
    })
}

/// Rebinds one served route descriptor onto a Transformer-backed
/// reference-linear direct-proof surface.
#[must_use]
pub fn rebind_tassadar_reference_linear_direct_proof_route(
    route_descriptor: &TassadarPlannerExecutorRouteDescriptor,
    model_id: impl Into<String>,
    note: impl Into<String>,
) -> TassadarPlannerExecutorRouteDescriptor {
    let model_id = model_id.into();
    let note = note.into();
    TassadarPlannerExecutorRouteDescriptor::new(
        format!("tassadar.planner_executor_route.{model_id}.v0"),
        model_id,
        route_descriptor.benchmark_report_ref.clone(),
        route_descriptor.internal_compute_profile_id.clone(),
        route_descriptor
            .internal_compute_profile_claim_digest
            .clone(),
        route_descriptor.workload_capability_digest.clone(),
        route_descriptor.wasm_capability_matrix.clone(),
        vec![TassadarPlannerExecutorDecodeCapability {
            requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
            benchmark_report_ref: route_descriptor.benchmark_report_ref.clone(),
            note: note.clone(),
        }],
        route_descriptor.refusal_reasons.clone(),
        note,
    )
}

/// Route candidate contributed by one provider / worker pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorRouteCandidate {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Provider backend family label.
    pub backend_family: String,
    /// Whether the provider is ready for routing.
    pub ready: bool,
    /// Public route descriptor exported by the served lane.
    pub route_descriptor: TassadarPlannerExecutorRouteDescriptor,
}

impl TassadarPlannerExecutorRouteCandidate {
    /// Creates one route candidate.
    #[must_use]
    pub fn new(
        provider_id: impl Into<String>,
        worker_id: impl Into<String>,
        backend_family: impl Into<String>,
        ready: bool,
        route_descriptor: TassadarPlannerExecutorRouteDescriptor,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            worker_id: worker_id.into(),
            backend_family: backend_family.into(),
            ready,
            route_descriptor,
        }
    }
}

/// One route-negotiation request coming from a planner-facing surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorRouteNegotiationRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Routed product identifier.
    pub product_id: String,
    /// Optional explicit executor model request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Optional explicit Wasm module class requested for route negotiation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_wasm_module_class: Option<TassadarWorkloadClass>,
    /// Required Wasm opcode families for the requested module class.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_wasm_opcode_families: Vec<TassadarPlannerExecutorWasmOpcodeFamily>,
    /// Optional required import posture for the requested module class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_wasm_import_posture: Option<TassadarPlannerExecutorWasmImportPosture>,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Whether runtime decode fallback is allowed.
    pub allow_runtime_decode_fallback: bool,
    /// Whether a guaranteed direct path is required.
    pub require_direct_decode: bool,
    /// Whether the route must be benchmark-gated.
    pub require_benchmark_gate: bool,
}

impl TassadarPlannerExecutorRouteNegotiationRequest {
    /// Creates a new route-negotiation request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID),
            requested_model_id: None,
            requested_wasm_module_class: None,
            required_wasm_opcode_families: Vec::new(),
            requested_wasm_import_posture: None,
            requested_decode_mode,
            allow_runtime_decode_fallback: true,
            require_direct_decode: false,
            require_benchmark_gate: true,
        }
    }

    /// Pins negotiation to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Pins negotiation to one explicit Wasm module class.
    #[must_use]
    pub fn with_requested_wasm_module_class(
        mut self,
        requested_wasm_module_class: TassadarWorkloadClass,
    ) -> Self {
        self.requested_wasm_module_class = Some(requested_wasm_module_class);
        self
    }

    /// Requires explicit Wasm opcode families for the selected route.
    #[must_use]
    pub fn with_required_wasm_opcode_families(
        mut self,
        mut required_wasm_opcode_families: Vec<TassadarPlannerExecutorWasmOpcodeFamily>,
    ) -> Self {
        required_wasm_opcode_families.sort_by_key(|family| family.as_str());
        required_wasm_opcode_families.dedup();
        self.required_wasm_opcode_families = required_wasm_opcode_families;
        self
    }

    /// Requires one explicit Wasm import posture for the selected route.
    #[must_use]
    pub fn with_requested_wasm_import_posture(
        mut self,
        requested_wasm_import_posture: TassadarPlannerExecutorWasmImportPosture,
    ) -> Self {
        self.requested_wasm_import_posture = Some(requested_wasm_import_posture);
        self
    }

    /// Disallows runtime decode fallback.
    #[must_use]
    pub fn disallow_runtime_decode_fallback(mut self) -> Self {
        self.allow_runtime_decode_fallback = false;
        self
    }

    /// Requires a guaranteed direct route posture.
    #[must_use]
    pub fn require_direct_decode(mut self) -> Self {
        self.require_direct_decode = true;
        self
    }
}

/// Final negotiated route state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerExecutorNegotiatedRouteState {
    Direct,
    Fallback,
}

/// Successful route-selection result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorRouteSelection {
    /// Stable request identifier.
    pub request_id: String,
    /// Selected provider identifier.
    pub provider_id: String,
    /// Selected worker identifier.
    pub worker_id: String,
    /// Selected backend family.
    pub backend_family: String,
    /// Selected served route descriptor.
    pub route_descriptor: TassadarPlannerExecutorRouteDescriptor,
    /// Decode capability row used by negotiation.
    pub decode_capability: TassadarPlannerExecutorDecodeCapability,
    /// Effective decode mode after applying explicit module-class fallback.
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    /// Matched Wasm capability row when negotiation was module-class aware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_capability: Option<TassadarPlannerExecutorWasmCapabilityRow>,
    /// Direct or fallback-capable route state.
    pub route_state: TassadarPlannerExecutorNegotiatedRouteState,
    /// Plain-language negotiation detail.
    pub detail: String,
}

/// Typed refusal result for route negotiation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorRouteRefusal {
    /// Stable request identifier.
    pub request_id: String,
    /// Typed refusal reason.
    pub refusal_reason: TassadarPlannerExecutorRouteRefusalReason,
    /// Considered route identifiers in stable order.
    pub considered_route_ids: Vec<String>,
    /// Plain-language refusal detail.
    pub detail: String,
}

/// Route-negotiation result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarPlannerExecutorRouteNegotiationOutcome {
    Selected {
        selection: TassadarPlannerExecutorRouteSelection,
    },
    Refused {
        refusal: TassadarPlannerExecutorRouteRefusal,
    },
}

/// Negotiates one planner / executor route across served candidates.
#[must_use]
pub fn negotiate_tassadar_planner_executor_route(
    candidates: &[TassadarPlannerExecutorRouteCandidate],
    request: &TassadarPlannerExecutorRouteNegotiationRequest,
) -> TassadarPlannerExecutorRouteNegotiationOutcome {
    if request.product_id != TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID {
        return TassadarPlannerExecutorRouteNegotiationOutcome::Refused {
            refusal: TassadarPlannerExecutorRouteRefusal {
                request_id: request.request_id.clone(),
                refusal_reason: TassadarPlannerExecutorRouteRefusalReason::UnsupportedProduct,
                considered_route_ids: route_ids(candidates),
                detail: format!(
                    "unsupported Tassadar planner route product `{}`",
                    request.product_id
                ),
            },
        };
    }

    let mut matching_model_exists = false;
    let mut ready_model_exists = false;
    let mut benchmark_gated_model_exists = false;
    let mut fallback_capable_exists = false;
    let mut requested_wasm_module_class_exists = request.requested_wasm_module_class.is_none();
    let mut requested_wasm_module_class_supported = request.requested_wasm_module_class.is_none();
    let mut requested_wasm_opcode_families_supported =
        request.required_wasm_opcode_families.is_empty();
    let mut requested_wasm_import_posture_supported =
        request.requested_wasm_import_posture.is_none();
    let mut decode_mode_exists = false;

    let mut direct_candidates = Vec::new();
    let mut fallback_candidates = Vec::new();

    for candidate in sorted_candidates(candidates) {
        if let Some(requested_model_id) = request.requested_model_id.as_deref()
            && candidate.route_descriptor.model_id != requested_model_id
        {
            continue;
        }
        matching_model_exists = true;

        if !candidate.ready {
            continue;
        }
        ready_model_exists = true;

        if request.require_benchmark_gate
            && candidate
                .route_descriptor
                .benchmark_report_ref
                .trim()
                .is_empty()
        {
            continue;
        }
        benchmark_gated_model_exists = true;

        if let Some(requested_wasm_module_class) = request.requested_wasm_module_class {
            let Some(wasm_capability) = candidate
                .route_descriptor
                .wasm_capability_matrix
                .row(requested_wasm_module_class)
                .cloned()
            else {
                continue;
            };
            requested_wasm_module_class_exists = true;

            if wasm_capability.supported_decode_modes.is_empty() {
                continue;
            }
            requested_wasm_module_class_supported = true;

            if let Some(requested_wasm_import_posture) = request.requested_wasm_import_posture {
                if wasm_capability.import_posture != requested_wasm_import_posture {
                    continue;
                }
                requested_wasm_import_posture_supported = true;
            }

            if !wasm_capability
                .supports_required_opcode_families(&request.required_wasm_opcode_families)
            {
                continue;
            }
            requested_wasm_opcode_families_supported = true;

            let Some((decode_capability, effective_decode_mode, route_state)) =
                decode_capability_for_wasm_row(
                    &wasm_capability,
                    request.requested_decode_mode,
                    &candidate.route_descriptor.benchmark_report_ref,
                )
            else {
                continue;
            };
            decode_mode_exists = true;

            match route_state {
                TassadarPlannerExecutorNegotiatedRouteState::Direct => {
                    direct_candidates.push((
                        candidate,
                        decode_capability,
                        effective_decode_mode,
                        Some(wasm_capability),
                    ));
                }
                TassadarPlannerExecutorNegotiatedRouteState::Fallback => {
                    fallback_capable_exists = true;
                    fallback_candidates.push((
                        candidate,
                        decode_capability,
                        effective_decode_mode,
                        Some(wasm_capability),
                    ));
                }
            }
            continue;
        }

        let Some(decode_capability) = candidate
            .route_descriptor
            .decode_capabilities
            .iter()
            .find(|capability| capability.requested_decode_mode == request.requested_decode_mode)
            .cloned()
        else {
            continue;
        };
        decode_mode_exists = true;

        match decode_capability.route_posture {
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed => {
                direct_candidates.push((
                    candidate,
                    decode_capability,
                    request.requested_decode_mode,
                    None,
                ));
            }
            TassadarPlannerExecutorRoutePosture::FallbackCapable => {
                fallback_capable_exists = true;
                fallback_candidates.push((
                    candidate,
                    decode_capability,
                    request.requested_decode_mode,
                    None,
                ));
            }
        }
    }

    if let Some((candidate, decode_capability, effective_decode_mode, wasm_capability)) =
        direct_candidates.into_iter().next()
    {
        return TassadarPlannerExecutorRouteNegotiationOutcome::Selected {
            selection: TassadarPlannerExecutorRouteSelection {
                request_id: request.request_id.clone(),
                provider_id: candidate.provider_id.clone(),
                worker_id: candidate.worker_id.clone(),
                backend_family: candidate.backend_family.clone(),
                route_descriptor: candidate.route_descriptor.clone(),
                decode_capability,
                effective_decode_mode,
                wasm_capability,
                route_state: TassadarPlannerExecutorNegotiatedRouteState::Direct,
                detail: format!(
                    "selected direct Tassadar planner / executor route `{}` on provider `{}` worker `{}`",
                    candidate.route_descriptor.route_id, candidate.provider_id, candidate.worker_id
                ),
            },
        };
    }

    if fallback_capable_exists {
        if request.require_direct_decode {
            return refusal(
                request,
                candidates,
                TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                format!(
                    "requested decode `{}` is only published as fallback-capable on the available Tassadar planner routes",
                    request.requested_decode_mode.as_str()
                ),
            );
        }
        if !request.allow_runtime_decode_fallback {
            return refusal(
                request,
                candidates,
                TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                fallback_disallowed_detail(request),
            );
        }
        if let Some((candidate, decode_capability, effective_decode_mode, wasm_capability)) =
            fallback_candidates.into_iter().next()
        {
            return TassadarPlannerExecutorRouteNegotiationOutcome::Selected {
                selection: TassadarPlannerExecutorRouteSelection {
                    request_id: request.request_id.clone(),
                    provider_id: candidate.provider_id.clone(),
                    worker_id: candidate.worker_id.clone(),
                    backend_family: candidate.backend_family.clone(),
                    route_descriptor: candidate.route_descriptor.clone(),
                    decode_capability,
                    effective_decode_mode,
                    wasm_capability,
                    route_state: TassadarPlannerExecutorNegotiatedRouteState::Fallback,
                    detail: fallback_selection_detail(request, candidate, effective_decode_mode),
                },
            };
        }
    }

    let (reason, detail) = if request.requested_model_id.is_some() && !matching_model_exists {
        (
            TassadarPlannerExecutorRouteRefusalReason::UnknownModel,
            format!(
                "no served Tassadar planner route matched requested model `{}`",
                request
                    .requested_model_id
                    .as_deref()
                    .unwrap_or("unknown_model")
            ),
        )
    } else if matching_model_exists && !ready_model_exists {
        (
            TassadarPlannerExecutorRouteRefusalReason::ProviderNotReady,
            String::from(
                "matching Tassadar planner routes exist but all providers are currently not ready",
            ),
        )
    } else if request.require_benchmark_gate && ready_model_exists && !benchmark_gated_model_exists
    {
        (
            TassadarPlannerExecutorRouteRefusalReason::BenchmarkGateMissing,
            String::from(
                "matching ready Tassadar planner routes were missing benchmark-gated evidence",
            ),
        )
    } else if request.requested_wasm_module_class.is_some() && !requested_wasm_module_class_exists {
        (
            TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
            format!(
                "no matching Tassadar planner route published module class `{}`",
                request
                    .requested_wasm_module_class
                    .map(TassadarWorkloadClass::as_str)
                    .unwrap_or("unknown_module_class")
            ),
        )
    } else if request.requested_wasm_module_class.is_some()
        && requested_wasm_module_class_exists
        && !requested_wasm_module_class_supported
    {
        (
            TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
            format!(
                "module class `{}` remains outside the published served boundary on the matching Tassadar planner routes",
                request
                    .requested_wasm_module_class
                    .map(TassadarWorkloadClass::as_str)
                    .unwrap_or("unknown_module_class")
            ),
        )
    } else if request.requested_wasm_import_posture.is_some()
        && requested_wasm_module_class_supported
        && !requested_wasm_import_posture_supported
    {
        (
            TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            format!(
                "module class `{}` does not publish import posture `{}` on the matching Tassadar planner routes",
                request
                    .requested_wasm_module_class
                    .map(TassadarWorkloadClass::as_str)
                    .unwrap_or("unknown_module_class"),
                request
                    .requested_wasm_import_posture
                    .map(TassadarPlannerExecutorWasmImportPosture::as_str)
                    .unwrap_or("unknown_import_posture")
            ),
        )
    } else if !request.required_wasm_opcode_families.is_empty()
        && requested_wasm_module_class_supported
        && !requested_wasm_opcode_families_supported
    {
        (
            TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
            format!(
                "module class `{}` does not publish opcode families {:?} on the matching Tassadar planner routes",
                request
                    .requested_wasm_module_class
                    .map(TassadarWorkloadClass::as_str)
                    .unwrap_or("unknown_module_class"),
                request
                    .required_wasm_opcode_families
                    .iter()
                    .map(|family| family.as_str())
                    .collect::<Vec<_>>()
            ),
        )
    } else if matching_model_exists && benchmark_gated_model_exists && !decode_mode_exists {
        (
            TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
            decode_mode_unsupported_detail(request),
        )
    } else {
        (
            TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
            String::from("no eligible Tassadar planner route candidates remained after filtering"),
        )
    };
    refusal(request, candidates, reason, detail)
}

fn refusal(
    request: &TassadarPlannerExecutorRouteNegotiationRequest,
    candidates: &[TassadarPlannerExecutorRouteCandidate],
    refusal_reason: TassadarPlannerExecutorRouteRefusalReason,
    detail: String,
) -> TassadarPlannerExecutorRouteNegotiationOutcome {
    TassadarPlannerExecutorRouteNegotiationOutcome::Refused {
        refusal: TassadarPlannerExecutorRouteRefusal {
            request_id: request.request_id.clone(),
            refusal_reason,
            considered_route_ids: route_ids(candidates),
            detail,
        },
    }
}

fn sorted_candidates(
    candidates: &[TassadarPlannerExecutorRouteCandidate],
) -> Vec<&TassadarPlannerExecutorRouteCandidate> {
    let mut candidates = candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (
            left.provider_id.as_str(),
            left.worker_id.as_str(),
            left.route_descriptor.model_id.as_str(),
            left.route_descriptor.route_id.as_str(),
        )
            .cmp(&(
                right.provider_id.as_str(),
                right.worker_id.as_str(),
                right.route_descriptor.model_id.as_str(),
                right.route_descriptor.route_id.as_str(),
            ))
    });
    candidates
}

fn route_ids(candidates: &[TassadarPlannerExecutorRouteCandidate]) -> Vec<String> {
    let mut route_ids = candidates
        .iter()
        .map(|candidate| candidate.route_descriptor.route_id.clone())
        .collect::<Vec<_>>();
    route_ids.sort();
    route_ids.dedup();
    route_ids
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TassadarPlannerExecutorWasmDecodeDisposition {
    Direct,
    Fallback {
        effective_decode_mode: TassadarExecutorDecodeMode,
    },
}

fn decode_capability_for_wasm_row(
    wasm_capability: &TassadarPlannerExecutorWasmCapabilityRow,
    requested_decode_mode: TassadarExecutorDecodeMode,
    route_benchmark_report_ref: &str,
) -> Option<(
    TassadarPlannerExecutorDecodeCapability,
    TassadarExecutorDecodeMode,
    TassadarPlannerExecutorNegotiatedRouteState,
)> {
    let benchmark_report_ref = wasm_capability
        .benchmark_report_ref
        .clone()
        .unwrap_or_else(|| String::from(route_benchmark_report_ref));
    match wasm_capability.decode_disposition(requested_decode_mode)? {
        TassadarPlannerExecutorWasmDecodeDisposition::Direct => Some((
            TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode,
                route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                benchmark_report_ref,
                note: format!(
                    "module class `{}` stays direct on `{}`; {}",
                    wasm_capability.module_class.as_str(),
                    requested_decode_mode.as_str(),
                    wasm_capability.note
                ),
            },
            requested_decode_mode,
            TassadarPlannerExecutorNegotiatedRouteState::Direct,
        )),
        TassadarPlannerExecutorWasmDecodeDisposition::Fallback {
            effective_decode_mode,
        } => Some((
            TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode,
                route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                benchmark_report_ref,
                note: format!(
                    "module class `{}` requests `{}` via explicit fallback to `{}`; {}",
                    wasm_capability.module_class.as_str(),
                    requested_decode_mode.as_str(),
                    effective_decode_mode.as_str(),
                    wasm_capability.note
                ),
            },
            effective_decode_mode,
            TassadarPlannerExecutorNegotiatedRouteState::Fallback,
        )),
    }
}

fn decode_mode_unsupported_detail(
    request: &TassadarPlannerExecutorRouteNegotiationRequest,
) -> String {
    if let Some(requested_wasm_module_class) = request.requested_wasm_module_class {
        format!(
            "module class `{}` does not publish decode support for `{}` on the matching Tassadar planner routes",
            requested_wasm_module_class.as_str(),
            request.requested_decode_mode.as_str()
        )
    } else {
        format!(
            "no matching Tassadar planner route published decode support for `{}`",
            request.requested_decode_mode.as_str()
        )
    }
}

fn fallback_disallowed_detail(request: &TassadarPlannerExecutorRouteNegotiationRequest) -> String {
    if let Some(requested_wasm_module_class) = request.requested_wasm_module_class {
        format!(
            "planner policy disallowed runtime decode fallback for module class `{}` on requested decode `{}`",
            requested_wasm_module_class.as_str(),
            request.requested_decode_mode.as_str()
        )
    } else {
        format!(
            "planner policy disallowed runtime decode fallback for `{}`",
            request.requested_decode_mode.as_str()
        )
    }
}

fn fallback_selection_detail(
    request: &TassadarPlannerExecutorRouteNegotiationRequest,
    candidate: &TassadarPlannerExecutorRouteCandidate,
    effective_decode_mode: TassadarExecutorDecodeMode,
) -> String {
    if let Some(requested_wasm_module_class) = request.requested_wasm_module_class {
        format!(
            "selected fallback-capable Tassadar planner / executor route `{}` on provider `{}` worker `{}`; module class `{}` will execute requested decode `{}` via explicit fallback to `{}`",
            candidate.route_descriptor.route_id,
            candidate.provider_id,
            candidate.worker_id,
            requested_wasm_module_class.as_str(),
            request.requested_decode_mode.as_str(),
            effective_decode_mode.as_str()
        )
    } else {
        format!(
            "selected fallback-capable Tassadar planner / executor route `{}` on provider `{}` worker `{}`",
            candidate.route_descriptor.route_id, candidate.provider_id, candidate.worker_id
        )
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("tassadar planner executor route should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        bind_tassadar_direct_model_weight_route, negotiate_tassadar_planner_executor_route,
        rebind_tassadar_reference_linear_direct_proof_route,
        TassadarDirectModelWeightRouteBindingError, TassadarPlannerExecutorDecodeCapability,
        TassadarPlannerExecutorNegotiatedRouteState, TassadarPlannerExecutorRouteCandidate,
        TassadarPlannerExecutorRouteDescriptor, TassadarPlannerExecutorRouteNegotiationOutcome,
        TassadarPlannerExecutorRouteNegotiationRequest, TassadarPlannerExecutorRoutePosture,
        TassadarPlannerExecutorRouteRefusalReason, TassadarPlannerExecutorWasmCapabilityMatrix,
        TassadarPlannerExecutorWasmCapabilityRow, TassadarPlannerExecutorWasmImportPosture,
        TassadarPlannerExecutorWasmOpcodeFamily,
    };
    use psionic_models::TassadarWorkloadClass;
    use psionic_runtime::TassadarExecutorDecodeMode;

    fn wasm_capability_matrix() -> TassadarPlannerExecutorWasmCapabilityMatrix {
        TassadarPlannerExecutorWasmCapabilityMatrix::new(
            "matrix.article-model.v0",
            "article-model",
            vec![
                TassadarPlannerExecutorWasmCapabilityRow::new(
                    TassadarWorkloadClass::MicroWasmKernel,
                    vec![
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        TassadarExecutorDecodeMode::HullCache,
                        TassadarExecutorDecodeMode::SparseTopK,
                    ],
                    vec![
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        TassadarExecutorDecodeMode::HullCache,
                        TassadarExecutorDecodeMode::SparseTopK,
                    ],
                    None,
                    vec![
                        TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
                        TassadarPlannerExecutorWasmOpcodeFamily::StructuredControl,
                        TassadarPlannerExecutorWasmOpcodeFamily::LinearMemoryV2,
                        TassadarPlannerExecutorWasmOpcodeFamily::DirectCallFrames,
                    ],
                    TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                    Some(String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    )),
                    vec![
                        TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                        TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
                    ],
                    "micro kernels stay direct on every published decode mode",
                ),
                TassadarPlannerExecutorWasmCapabilityRow::new(
                    TassadarWorkloadClass::LongLoopKernel,
                    vec![
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        TassadarExecutorDecodeMode::HullCache,
                        TassadarExecutorDecodeMode::SparseTopK,
                    ],
                    vec![TassadarExecutorDecodeMode::ReferenceLinear],
                    Some(TassadarExecutorDecodeMode::ReferenceLinear),
                    vec![
                        TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
                        TassadarPlannerExecutorWasmOpcodeFamily::StructuredControl,
                    ],
                    TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                    Some(String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    )),
                    vec![
                        TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                        TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                    ],
                    "long-loop kernels remain exact only with explicit fallback to reference_linear",
                ),
                TassadarPlannerExecutorWasmCapabilityRow::new(
                    TassadarWorkloadClass::ArithmeticMicroprogram,
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic],
                    TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                    None,
                    vec![TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported],
                    "article route does not publish validation microprogram classes",
                ),
            ],
            "test matrix claim boundary",
        )
    }

    fn route_descriptor(
        route_id: &str,
        model_id: &str,
        decode_capabilities: Vec<TassadarPlannerExecutorDecodeCapability>,
    ) -> TassadarPlannerExecutorRouteDescriptor {
        TassadarPlannerExecutorRouteDescriptor::new(
            route_id,
            model_id,
            "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
            "tassadar.internal_compute.article_closeout.v1",
            "claim-digest",
            "matrix-digest",
            wasm_capability_matrix(),
            decode_capabilities,
            vec![
                TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            ],
            "test route descriptor",
        )
    }

    #[test]
    fn direct_model_weight_route_binding_projects_direct_route_descriptor() {
        let descriptor = route_descriptor(
            "route-direct-proof",
            "article-model",
            vec![TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                benchmark_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                ),
                note: String::from("direct dense floor"),
            }],
        );

        let binding = bind_tassadar_direct_model_weight_route(
            &descriptor,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
        .expect("direct route binding");

        assert_eq!(
            binding.route_descriptor_digest,
            descriptor.descriptor_digest
        );
        assert_eq!(
            binding.route_posture,
            psionic_runtime::TassadarDirectModelWeightRoutePosture::DirectGuaranteed
        );
    }

    #[test]
    fn direct_model_weight_route_binding_refuses_fallback_capable_route() {
        let descriptor = route_descriptor(
            "route-fallback-proof",
            "article-model",
            vec![TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
                route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                benchmark_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                ),
                note: String::from("fallback capable"),
            }],
        );

        assert_eq!(
            bind_tassadar_direct_model_weight_route(
                &descriptor,
                TassadarExecutorDecodeMode::HullCache,
            )
            .unwrap_err(),
            TassadarDirectModelWeightRouteBindingError::RouteNotDirectGuaranteed {
                route_descriptor_digest: descriptor.descriptor_digest,
                requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
                route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
            }
        );
    }

    #[test]
    fn reference_linear_direct_proof_route_rebinds_model_identity() {
        let descriptor = route_descriptor(
            "route-direct-proof",
            "fixture-model",
            vec![TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                benchmark_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                ),
                note: String::from("fixture direct dense floor"),
            }],
        );

        let rebound = rebind_tassadar_reference_linear_direct_proof_route(
            &descriptor,
            "tassadar-article-transformer-trace-bound-trained-v0",
            "Transformer-backed direct-proof route",
        );

        assert_eq!(
            rebound.model_id,
            "tassadar-article-transformer-trace-bound-trained-v0"
        );
        assert_eq!(rebound.decode_capabilities.len(), 1);
        assert_eq!(
            rebound.decode_capabilities[0].requested_decode_mode,
            TassadarExecutorDecodeMode::ReferenceLinear
        );
        assert_eq!(
            rebound.decode_capabilities[0].route_posture,
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed
        );
        assert_ne!(rebound.descriptor_digest, descriptor.descriptor_digest);
    }

    #[test]
    fn tassadar_route_negotiation_prefers_direct_route() {
        let candidates = vec![
            TassadarPlannerExecutorRouteCandidate::new(
                "provider-a",
                "worker-a",
                "psionic",
                true,
                route_descriptor(
                    "route-a",
                    "article-model",
                    vec![TassadarPlannerExecutorDecodeCapability {
                        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                        route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                        benchmark_report_ref: String::from(
                            "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                        ),
                        note: String::from("direct dense floor"),
                    }],
                ),
            ),
            TassadarPlannerExecutorRouteCandidate::new(
                "provider-b",
                "worker-b",
                "psionic",
                true,
                route_descriptor(
                    "route-b",
                    "article-model",
                    vec![TassadarPlannerExecutorDecodeCapability {
                        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                        route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                        benchmark_report_ref: String::from(
                            "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                        ),
                        note: String::from("fallback capable"),
                    }],
                ),
            ),
        ];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-direct",
                TassadarExecutorDecodeMode::ReferenceLinear,
            )
            .with_requested_model_id("article-model"),
        );
        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                assert_eq!(selection.provider_id, "provider-a");
                assert_eq!(
                    selection.route_state,
                    TassadarPlannerExecutorNegotiatedRouteState::Direct
                );
            }
            other => panic!("expected direct route selection, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_surfaces_fallback_when_allowed() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
                    route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("hull route may fall back"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-fallback",
                TassadarExecutorDecodeMode::HullCache,
            )
            .with_requested_model_id("article-model"),
        );
        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                assert_eq!(
                    selection.route_state,
                    TassadarPlannerExecutorNegotiatedRouteState::Fallback
                );
            }
            other => panic!("expected fallback-capable route selection, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_refuses_when_direct_path_is_required() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::SparseTopK,
                    route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("sparse route may fall back"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-direct-required",
                TassadarExecutorDecodeMode::SparseTopK,
            )
            .with_requested_model_id("article-model")
            .require_direct_decode(),
        );
        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.refusal_reason,
                    TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired
                );
            }
            other => panic!("expected direct-path refusal, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_uses_module_class_matrix_for_direct_selection() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
                    route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("coarse route posture"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-module-class-direct",
                TassadarExecutorDecodeMode::HullCache,
            )
            .with_requested_model_id("article-model")
            .with_requested_wasm_module_class(TassadarWorkloadClass::MicroWasmKernel),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                assert_eq!(
                    selection.route_state,
                    TassadarPlannerExecutorNegotiatedRouteState::Direct
                );
                assert_eq!(
                    selection.decode_capability.route_posture,
                    TassadarPlannerExecutorRoutePosture::DirectGuaranteed
                );
                assert_eq!(
                    selection.effective_decode_mode,
                    TassadarExecutorDecodeMode::HullCache
                );
                assert_eq!(
                    selection
                        .wasm_capability
                        .expect("module-class-aware selection")
                        .module_class,
                    TassadarWorkloadClass::MicroWasmKernel
                );
            }
            other => panic!("expected module-class direct selection, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_surfaces_fallback_by_module_class() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
                    route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("coarse route posture"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-module-class-fallback",
                TassadarExecutorDecodeMode::HullCache,
            )
            .with_requested_model_id("article-model")
            .with_requested_wasm_module_class(TassadarWorkloadClass::LongLoopKernel),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                assert_eq!(
                    selection.route_state,
                    TassadarPlannerExecutorNegotiatedRouteState::Fallback
                );
                assert_eq!(
                    selection.effective_decode_mode,
                    TassadarExecutorDecodeMode::ReferenceLinear
                );
                assert_eq!(
                    selection
                        .wasm_capability
                        .expect("module-class-aware selection")
                        .module_class,
                    TassadarWorkloadClass::LongLoopKernel
                );
            }
            other => panic!("expected module-class fallback selection, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_refuses_unsupported_module_class() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                    route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("direct"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-module-class-refused",
                TassadarExecutorDecodeMode::ReferenceLinear,
            )
            .with_requested_model_id("article-model")
            .with_requested_wasm_module_class(TassadarWorkloadClass::ArithmeticMicroprogram),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.refusal_reason,
                    TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported
                );
            }
            other => panic!("expected unsupported module-class refusal, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_route_negotiation_refuses_unsupported_import_posture() {
        let candidates = vec![TassadarPlannerExecutorRouteCandidate::new(
            "provider-a",
            "worker-a",
            "psionic",
            true,
            route_descriptor(
                "route-a",
                "article-model",
                vec![TassadarPlannerExecutorDecodeCapability {
                    requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                    route_posture: TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
                    benchmark_report_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                    ),
                    note: String::from("direct"),
                }],
            ),
        )];

        let outcome = negotiate_tassadar_planner_executor_route(
            candidates.as_slice(),
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "request-import-posture-refused",
                TassadarExecutorDecodeMode::ReferenceLinear,
            )
            .with_requested_model_id("article-model")
            .with_requested_wasm_module_class(TassadarWorkloadClass::MicroWasmKernel)
            .with_requested_wasm_import_posture(
                TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
            ),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.refusal_reason,
                    TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported
                );
            }
            other => panic!("expected unsupported import-posture refusal, got {other:?}"),
        }
    }
}
