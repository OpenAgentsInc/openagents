//! Core tensor, shape, dtype, device, and layout types for Psionic.
//!
//! This crate intentionally stays small and product-agnostic. It owns public
//! engine-facing metadata, not backend execution logic.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "tensor facade and foundational engine types";

/// Canonical cross-library refusal family for explicitly unsupported behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicRefusalCode {
    /// One op family is intentionally unsupported.
    UnsupportedOp,
    /// One reverse-mode gradient path is intentionally unsupported.
    UnsupportedGradient,
    /// One requested layout or view family is intentionally unsupported.
    UnsupportedLayout,
    /// One backend or kernel capability is intentionally unsupported.
    UnsupportedBackendCapability,
    /// One serialized or replayed artifact is incompatible with the expected contract.
    SerializationIncompatibility,
    /// Sandbox policy denied the request before execution.
    SandboxPolicyDenied,
    /// One runtime or sharding request did not satisfy topology requirements.
    TopologyMismatch,
}

/// Owning surface for one canonical refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicRefusalScope {
    /// Graph construction or operator planning surface.
    Graph,
    /// Autodiff or backward-planning surface.
    Autodiff,
    /// General runtime or serving runtime surface.
    Runtime,
    /// Sandbox execution-policy surface.
    Sandbox,
    /// Local or clustered topology and sharding surface.
    Topology,
}

/// Canonical typed refusal record shared across Psionic crates.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PsionicRefusal {
    /// Stable refusal family.
    pub code: PsionicRefusalCode,
    /// Owning surface that emitted the refusal.
    pub scope: PsionicRefusalScope,
    /// Stable subject such as an op label, backend name, or product id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Plain-language detail suitable for logs, fixtures, or receipts.
    pub detail: String,
}

impl PsionicRefusal {
    /// Creates a canonical refusal.
    #[must_use]
    pub fn new(
        code: PsionicRefusalCode,
        scope: PsionicRefusalScope,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            scope,
            subject: None,
            detail: detail.into(),
        }
    }

    /// Attaches a stable subject when one exists.
    #[must_use]
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }
}

/// Stable tensor identifier used across the Psionic crates.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct TensorId(pub u32);

impl fmt::Display for TensorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// Supported scalar data types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DType {
    /// 32-bit floating point values.
    F32,
    /// 16-bit IEEE 754 half-precision floating point values.
    F16,
    /// 16-bit bfloat values.
    BF16,
    /// 8-bit signed integer values.
    I8,
}

impl DType {
    /// Returns the size of a single element in bytes.
    #[must_use]
    pub const fn element_size_bytes(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 => 1,
        }
    }

    /// Returns the promoted dtype for a binary op when Psionic has an explicit
    /// rule for the pair.
    #[must_use]
    pub fn promote_binary(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::F32, Self::F32) => Some(Self::F32),
            (Self::F16, Self::F16) => Some(Self::F16),
            (Self::BF16, Self::BF16) => Some(Self::BF16),
            (Self::I8, Self::I8) => Some(Self::I8),
            (Self::F32, _) | (_, Self::F32) => Some(Self::F32),
            (Self::BF16, Self::F16) | (Self::F16, Self::BF16) => Some(Self::F32),
            (Self::BF16, Self::I8) | (Self::I8, Self::BF16) => Some(Self::BF16),
            (Self::F16, Self::I8) | (Self::I8, Self::F16) => Some(Self::F16),
        }
    }

    /// Returns the coarse dtype family used by current framework-core
    /// contracts.
    #[must_use]
    pub const fn class(self) -> DTypeClass {
        match self {
            Self::F32 | Self::F16 | Self::BF16 => DTypeClass::FloatingPoint,
            Self::I8 => DTypeClass::SignedInteger,
        }
    }

    /// Returns whether the dtype can act as the logical view over quantized
    /// GGML/GGUF block storage in the current surface.
    #[must_use]
    pub const fn supports_quantized_logical_storage(self) -> bool {
        matches!(self, Self::F32)
    }
}

/// Coarse dtype family exposed by the compact Psionic tensor surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypeClass {
    /// Floating-point arithmetic dtype.
    FloatingPoint,
    /// Signed integer arithmetic dtype.
    SignedInteger,
}

/// Extended dtype vocabulary used by the bounded semantics layer above the
/// compact runtime-core `DType` subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtendedDType {
    /// Boolean truth values.
    Bool,
    /// 8-bit unsigned integer values.
    U8,
    /// 8-bit signed integer values.
    I8,
    /// 16-bit signed integer values.
    I16,
    /// 32-bit signed integer values.
    I32,
    /// 64-bit signed integer values.
    I64,
    /// Float8 E4M3FN values.
    F8E4M3Fn,
    /// Float8 E5M2 values.
    F8E5M2,
    /// 16-bit IEEE 754 half-precision floating point values.
    F16,
    /// 16-bit bfloat values.
    BF16,
    /// 32-bit floating point values.
    F32,
    /// 64-bit floating point values.
    F64,
    /// 64-bit complex values with `f32` real and imaginary parts.
    Complex64,
    /// 128-bit complex values with `f64` real and imaginary parts.
    Complex128,
}

impl ExtendedDType {
    /// Returns a stable lowercase label for diagnostics and receipts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U8 => "u8",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F8E4M3Fn => "f8_e4m3fn",
            Self::F8E5M2 => "f8_e5m2",
            Self::F16 => "f16",
            Self::BF16 => "bf16",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Complex64 => "complex64",
            Self::Complex128 => "complex128",
        }
    }

    /// Returns the coarse dtype class.
    #[must_use]
    pub const fn class(self) -> ExtendedDTypeClass {
        match self {
            Self::Bool => ExtendedDTypeClass::Boolean,
            Self::U8 => ExtendedDTypeClass::UnsignedInteger,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => ExtendedDTypeClass::SignedInteger,
            Self::F8E4M3Fn | Self::F8E5M2 | Self::F16 | Self::BF16 | Self::F32 | Self::F64 => {
                ExtendedDTypeClass::FloatingPoint
            }
            Self::Complex64 | Self::Complex128 => ExtendedDTypeClass::ComplexFloatingPoint,
        }
    }

    /// Returns whether this dtype belongs to the current low-precision family.
    #[must_use]
    pub const fn is_low_precision(self) -> bool {
        matches!(
            self,
            Self::F8E4M3Fn | Self::F8E5M2 | Self::F16 | Self::BF16 | Self::I8 | Self::U8
        )
    }

    /// Tries to lower the extended dtype into the compact runtime-core
    /// `DType` subset used by current graph and runtime execution surfaces.
    pub fn try_into_core_dtype(self) -> Result<DType, PsionicRefusal> {
        match self {
            Self::I8 => Ok(DType::I8),
            Self::F16 => Ok(DType::F16),
            Self::BF16 => Ok(DType::BF16),
            Self::F32 => Ok(DType::F32),
            _ => Err(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    format!(
                        "extended dtype `{}` is not part of the current compact runtime-core dtype subset",
                        self.label()
                    ),
                )
                .with_subject(self.label()),
            ),
        }
    }
}

impl From<DType> for ExtendedDType {
    fn from(value: DType) -> Self {
        match value {
            DType::F32 => Self::F32,
            DType::F16 => Self::F16,
            DType::BF16 => Self::BF16,
            DType::I8 => Self::I8,
        }
    }
}

/// Coarse dtype class exposed by the extended semantics layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtendedDTypeClass {
    /// Boolean predicate dtype.
    Boolean,
    /// Unsigned integer arithmetic dtype.
    UnsignedInteger,
    /// Signed integer arithmetic dtype.
    SignedInteger,
    /// Real-valued floating point dtype.
    FloatingPoint,
    /// Complex floating point dtype.
    ComplexFloatingPoint,
}

/// Status for one bounded promotion case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypePromotionStatus {
    /// The promotion result is explicitly supported.
    Supported,
    /// The promotion pair is explicitly refused.
    Refused,
}

/// One bounded binary-promotion case in the current semantics window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DTypePromotionCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Left-hand dtype.
    pub left: ExtendedDType,
    /// Right-hand dtype.
    pub right: ExtendedDType,
    /// Current status for the pair.
    pub status: DTypePromotionStatus,
    /// Result dtype when the pair is currently supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_dtype: Option<ExtendedDType>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the pair is intentionally unsupported today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Safety posture for one explicit cast path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypeCastKind {
    /// No conversion; source and target are identical.
    Identity,
    /// The cast widens or otherwise preserves all represented values.
    Lossless,
    /// The cast may lose precision but remains a supported current path.
    PrecisionLoss,
    /// The cast lifts values into a richer domain such as real to complex.
    DomainLift,
}

/// Status for one explicit cast rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypeCastStatus {
    /// The cast rule is explicitly supported.
    Supported,
    /// The cast rule is explicitly refused.
    Refused,
}

/// One bounded cast rule in the current semantics window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DTypeCastCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Source dtype.
    pub source: ExtendedDType,
    /// Target dtype.
    pub target: ExtendedDType,
    /// Current status for the cast.
    pub status: DTypeCastStatus,
    /// Cast behavior when the path is supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cast_kind: Option<DTypeCastKind>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the cast is intentionally unsupported today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Backend-family view used by the bounded advanced-dtype semantics report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypeBackendFamily {
    /// Shape-only or meta-only execution where advanced dtypes remain legal
    /// contracts even without runtime buffers.
    MetaExecution,
    /// Current runtime backends that only materialize the compact `DType`
    /// subset today.
    CurrentRuntimeBackends,
}

impl DTypeBackendFamily {
    fn label(self) -> &'static str {
        match self {
            Self::MetaExecution => "meta_execution",
            Self::CurrentRuntimeBackends => "current_runtime_backends",
        }
    }
}

/// Status for one backend-family dtype capability case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DTypeBackendCapabilityStatus {
    /// The dtype is explicitly supported by the backend family.
    Supported,
    /// The dtype is explicitly refused by the backend family.
    Refused,
}

/// One backend-family dtype capability case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DTypeBackendCapabilityCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Backend family under test.
    pub backend: DTypeBackendFamily,
    /// Dtype under test.
    pub dtype: ExtendedDType,
    /// Current status for the backend/dtype pair.
    pub status: DTypeBackendCapabilityStatus,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the backend family intentionally refuses the dtype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable bounded report for advanced dtype semantics above the
/// compact runtime-core `DType` subset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvancedDTypeSemanticsReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Versioned current-scope window.
    pub current_scope_window: String,
    /// Explicit binary-promotion rules carried by the report.
    pub promotion_cases: Vec<DTypePromotionCaseResult>,
    /// Explicit cast rules carried by the report.
    pub cast_cases: Vec<DTypeCastCaseResult>,
    /// Explicit backend-family capability rules carried by the report.
    pub backend_cases: Vec<DTypeBackendCapabilityCaseResult>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl AdvancedDTypeSemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        promotion_cases: Vec<DTypePromotionCaseResult>,
        cast_cases: Vec<DTypeCastCaseResult>,
        backend_cases: Vec<DTypeBackendCapabilityCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = stable_advanced_dtype_semantics_digest(
            current_scope_window.as_str(),
            promotion_cases.as_slice(),
            cast_cases.as_slice(),
            backend_cases.as_slice(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            promotion_cases,
            cast_cases,
            backend_cases,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.promotion_cases {
            lines.push(format!(
                "promotion|{}|{}|{}|{:?}",
                case.case_id,
                case.left.label(),
                case.right.label(),
                case.status
            ));
        }
        for case in &self.cast_cases {
            lines.push(format!(
                "cast|{}|{}|{}|{:?}",
                case.case_id,
                case.source.label(),
                case.target.label(),
                case.status
            ));
        }
        for case in &self.backend_cases {
            lines.push(format!(
                "backend|{}|{}|{}|{:?}",
                case.case_id,
                case.backend.label(),
                case.dtype.label(),
                case.status
            ));
        }
        lines
    }

    fn promotion_case(
        &self,
        left: ExtendedDType,
        right: ExtendedDType,
    ) -> Option<&DTypePromotionCaseResult> {
        self.promotion_cases.iter().find(|case| {
            (case.left == left && case.right == right) || (case.left == right && case.right == left)
        })
    }

    /// Resolves one bounded binary-promotion rule.
    pub fn resolve_binary_promotion(
        &self,
        left: ExtendedDType,
        right: ExtendedDType,
    ) -> Result<ExtendedDType, PsionicRefusal> {
        let Some(case) = self.promotion_case(left, right) else {
            return Err(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "advanced dtype semantics do not declare a bounded promotion rule for `{}` + `{}`",
                        left.label(),
                        right.label()
                    ),
                )
                .with_subject(format!("{}+{}", left.label(), right.label())),
            );
        };
        match case.status {
            DTypePromotionStatus::Supported => case.result_dtype.ok_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "promotion case `{}` is missing a result dtype",
                        case.case_id
                    ),
                )
                .with_subject(case.case_id.clone())
            }),
            DTypePromotionStatus::Refused => Err(case.refusal.clone().unwrap_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "advanced dtype semantics intentionally refuse `{}` + `{}` in the current scope",
                        left.label(),
                        right.label()
                    ),
                )
                .with_subject(format!("{}+{}", left.label(), right.label()))
            })),
        }
    }

    /// Resolves one bounded cast rule.
    pub fn resolve_cast(
        &self,
        source: ExtendedDType,
        target: ExtendedDType,
    ) -> Result<DTypeCastKind, PsionicRefusal> {
        let Some(case) = self
            .cast_cases
            .iter()
            .find(|case| case.source == source && case.target == target)
        else {
            return Err(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedOp,
                PsionicRefusalScope::Graph,
                format!(
                    "advanced dtype semantics do not declare a bounded cast rule for `{}` -> `{}`",
                    source.label(),
                    target.label()
                ),
            )
            .with_subject(format!("{}->{}", source.label(), target.label())));
        };
        match case.status {
            DTypeCastStatus::Supported => case.cast_kind.ok_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!("cast case `{}` is missing a cast kind", case.case_id),
                )
                .with_subject(case.case_id.clone())
            }),
            DTypeCastStatus::Refused => Err(case.refusal.clone().unwrap_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "advanced dtype semantics intentionally refuse `{}` -> `{}` in the current scope",
                        source.label(),
                        target.label()
                    ),
                )
                .with_subject(format!("{}->{}", source.label(), target.label()))
            })),
        }
    }

    /// Validates whether one backend family currently supports the requested
    /// dtype.
    pub fn validate_backend_support(
        &self,
        backend: DTypeBackendFamily,
        dtype: ExtendedDType,
    ) -> Result<(), PsionicRefusal> {
        let Some(case) = self
            .backend_cases
            .iter()
            .find(|case| case.backend == backend && case.dtype == dtype)
        else {
            return Err(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "advanced dtype semantics do not declare backend capability for `{}` on `{}`",
                    dtype.label(),
                    backend.label()
                ),
            )
            .with_subject(format!("{}@{}", dtype.label(), backend.label())));
        };
        match case.status {
            DTypeBackendCapabilityStatus::Supported => Ok(()),
            DTypeBackendCapabilityStatus::Refused => Err(case.refusal.clone().unwrap_or_else(
                || {
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "backend family `{}` intentionally refuses dtype `{}` in the current scope",
                            backend.label(),
                            dtype.label()
                        ),
                    )
                    .with_subject(format!("{}@{}", dtype.label(), backend.label()))
                },
            )),
        }
    }
}

/// Builds the canonical bounded advanced-dtype semantics report.
#[must_use]
pub fn builtin_advanced_dtype_semantics_report() -> AdvancedDTypeSemanticsReport {
    AdvancedDTypeSemanticsReport::new(
        String::from("psionic_advanced_dtype_v1"),
        vec![
            supported_promotion_case(
                "pytorch.bool_plus_u8",
                ExtendedDType::Bool,
                ExtendedDType::U8,
                ExtendedDType::U8,
                "Boolean-plus-unsigned promotion is bounded to a seeded predicate-plus-byte rule.",
            ),
            supported_promotion_case(
                "pytorch.i8_plus_i16",
                ExtendedDType::I8,
                ExtendedDType::I16,
                ExtendedDType::I16,
                "Signed integer widening is bounded to a seeded small-integer promotion rule.",
            ),
            supported_promotion_case(
                "pytorch.f16_plus_bf16",
                ExtendedDType::F16,
                ExtendedDType::BF16,
                ExtendedDType::F32,
                "Mixed half and bfloat promotion remains explicitly widened to `f32` in the current bounded semantics.",
            ),
            supported_promotion_case(
                "pytorch.f8_e4m3fn_plus_f16",
                ExtendedDType::F8E4M3Fn,
                ExtendedDType::F16,
                ExtendedDType::F16,
                "Float8 plus half promotion is bounded to widening into `f16` rather than claiming float8 arithmetic closure.",
            ),
            supported_promotion_case(
                "pytorch.f32_plus_complex64",
                ExtendedDType::F32,
                ExtendedDType::Complex64,
                ExtendedDType::Complex64,
                "Real-to-complex promotion is bounded to a seeded `f32` plus `complex64` rule.",
            ),
            supported_promotion_case(
                "pytorch.f64_plus_complex64",
                ExtendedDType::F64,
                ExtendedDType::Complex64,
                ExtendedDType::Complex128,
                "Higher-precision real-to-complex promotion is bounded to widening into `complex128`.",
            ),
            refused_promotion_case(
                "pytorch.f8_e5m2_plus_bf16.seed_scope_missing",
                ExtendedDType::F8E5M2,
                ExtendedDType::BF16,
                "Mixed `f8_e5m2` and `bf16` promotion stays explicitly unclaimed until wider low-precision numerics coverage lands.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    "mixed `f8_e5m2` and `bf16` promotion is intentionally outside the current bounded semantics window",
                )
                .with_subject("f8_e5m2+bf16"),
            ),
        ],
        vec![
            supported_cast_case(
                "pytorch.f32_to_f32",
                ExtendedDType::F32,
                ExtendedDType::F32,
                DTypeCastKind::Identity,
                "Identity casts stay explicit so downstream code can distinguish them from widening or lossy conversions.",
            ),
            supported_cast_case(
                "pytorch.f32_to_f64",
                ExtendedDType::F32,
                ExtendedDType::F64,
                DTypeCastKind::Lossless,
                "Real widening from `f32` to `f64` is supported as a lossless bounded cast.",
            ),
            supported_cast_case(
                "pytorch.f64_to_f32",
                ExtendedDType::F64,
                ExtendedDType::F32,
                DTypeCastKind::PrecisionLoss,
                "Real narrowing from `f64` to `f32` is supported but explicitly marked as precision-losing.",
            ),
            supported_cast_case(
                "pytorch.f32_to_complex64",
                ExtendedDType::F32,
                ExtendedDType::Complex64,
                DTypeCastKind::DomainLift,
                "Real-to-complex casts are bounded to domain-lift behavior with zero imaginary component.",
            ),
            supported_cast_case(
                "pytorch.f8_e4m3fn_to_f16",
                ExtendedDType::F8E4M3Fn,
                ExtendedDType::F16,
                DTypeCastKind::Lossless,
                "Float8 widening into `f16` is supported as a bounded low-precision cast path.",
            ),
            refused_cast_case(
                "pytorch.complex128_to_f32.imaginary_drop_refused",
                ExtendedDType::Complex128,
                ExtendedDType::F32,
                "Complex-to-real casts stay explicitly refused until the semantics layer owns a stable imaginary-component drop policy.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    "complex-to-real cast is intentionally unsupported because the current bounded semantics do not define imaginary-component drop behavior",
                )
                .with_subject("complex128->f32"),
            ),
        ],
        vec![
            supported_backend_case(
                "meta_execution.complex64",
                DTypeBackendFamily::MetaExecution,
                ExtendedDType::Complex64,
                "Meta execution may carry `complex64` contracts even though current runtime backends do not materialize them.",
            ),
            supported_backend_case(
                "meta_execution.f8_e5m2",
                DTypeBackendFamily::MetaExecution,
                ExtendedDType::F8E5M2,
                "Meta execution may carry `f8_e5m2` contracts as first-class semantics records.",
            ),
            supported_backend_case(
                "meta_execution.f64",
                DTypeBackendFamily::MetaExecution,
                ExtendedDType::F64,
                "Meta execution may carry `f64` contracts even before runtime closure exists.",
            ),
            supported_backend_case(
                "current_runtime_backends.f32",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::F32,
                "Current runtime backends materially support `f32` through the compact core dtype surface.",
            ),
            supported_backend_case(
                "current_runtime_backends.f16",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::F16,
                "Current runtime backends materially support `f16` through the compact core dtype surface.",
            ),
            supported_backend_case(
                "current_runtime_backends.bf16",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::BF16,
                "Current runtime backends materially support `bf16` through the compact core dtype surface.",
            ),
            supported_backend_case(
                "current_runtime_backends.i8",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::I8,
                "Current runtime backends materially support `i8` through the compact core dtype surface.",
            ),
            refused_backend_case(
                "current_runtime_backends.complex64",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::Complex64,
                "Current runtime backends do not yet materialize complex buffers; `complex64` remains a semantics-only contract today.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not materialize `complex64` buffers",
                )
                .with_subject("complex64@current_runtime_backends"),
            ),
            refused_backend_case(
                "current_runtime_backends.f8_e5m2",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::F8E5M2,
                "Current runtime backends do not yet materialize float8 buffers; `f8_e5m2` remains a semantics-only contract today.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not materialize `f8_e5m2` buffers",
                )
                .with_subject("f8_e5m2@current_runtime_backends"),
            ),
            refused_backend_case(
                "current_runtime_backends.f64",
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::F64,
                "Current runtime backends stay bounded to the compact dtype subset and do not yet claim `f64` execution.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends stay bounded to the compact dtype subset and do not execute `f64` today",
                )
                .with_subject("f64@current_runtime_backends"),
            ),
        ],
    )
}

fn supported_promotion_case(
    case_id: &str,
    left: ExtendedDType,
    right: ExtendedDType,
    result_dtype: ExtendedDType,
    bounded_scope: &str,
) -> DTypePromotionCaseResult {
    DTypePromotionCaseResult {
        case_id: String::from(case_id),
        left,
        right,
        status: DTypePromotionStatus::Supported,
        result_dtype: Some(result_dtype),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_promotion_case(
    case_id: &str,
    left: ExtendedDType,
    right: ExtendedDType,
    bounded_scope: &str,
    refusal: PsionicRefusal,
) -> DTypePromotionCaseResult {
    DTypePromotionCaseResult {
        case_id: String::from(case_id),
        left,
        right,
        status: DTypePromotionStatus::Refused,
        result_dtype: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn supported_cast_case(
    case_id: &str,
    source: ExtendedDType,
    target: ExtendedDType,
    cast_kind: DTypeCastKind,
    bounded_scope: &str,
) -> DTypeCastCaseResult {
    DTypeCastCaseResult {
        case_id: String::from(case_id),
        source,
        target,
        status: DTypeCastStatus::Supported,
        cast_kind: Some(cast_kind),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_cast_case(
    case_id: &str,
    source: ExtendedDType,
    target: ExtendedDType,
    bounded_scope: &str,
    refusal: PsionicRefusal,
) -> DTypeCastCaseResult {
    DTypeCastCaseResult {
        case_id: String::from(case_id),
        source,
        target,
        status: DTypeCastStatus::Refused,
        cast_kind: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn supported_backend_case(
    case_id: &str,
    backend: DTypeBackendFamily,
    dtype: ExtendedDType,
    bounded_scope: &str,
) -> DTypeBackendCapabilityCaseResult {
    DTypeBackendCapabilityCaseResult {
        case_id: String::from(case_id),
        backend,
        dtype,
        status: DTypeBackendCapabilityStatus::Supported,
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_backend_case(
    case_id: &str,
    backend: DTypeBackendFamily,
    dtype: ExtendedDType,
    bounded_scope: &str,
    refusal: PsionicRefusal,
) -> DTypeBackendCapabilityCaseResult {
    DTypeBackendCapabilityCaseResult {
        case_id: String::from(case_id),
        backend,
        dtype,
        status: DTypeBackendCapabilityStatus::Refused,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn stable_advanced_dtype_semantics_digest(
    current_scope_window: &str,
    promotion_cases: &[DTypePromotionCaseResult],
    cast_cases: &[DTypeCastCaseResult],
    backend_cases: &[DTypeBackendCapabilityCaseResult],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in promotion_cases {
        lines.push(format!("promotion_case_id={}", case.case_id));
        lines.push(format!("promotion_left={}", case.left.label()));
        lines.push(format!("promotion_right={}", case.right.label()));
        lines.push(format!("promotion_status={:?}", case.status));
        lines.push(format!("promotion_scope={}", case.bounded_scope));
        if let Some(result_dtype) = case.result_dtype {
            lines.push(format!("promotion_result={}", result_dtype.label()));
        }
        if let Some(refusal) = &case.refusal {
            lines.push(format!("promotion_refusal_code={:?}", refusal.code));
            lines.push(format!("promotion_refusal_scope={:?}", refusal.scope));
            lines.push(format!("promotion_refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("promotion_refusal_subject={subject}"));
            }
        }
    }
    for case in cast_cases {
        lines.push(format!("cast_case_id={}", case.case_id));
        lines.push(format!("cast_source={}", case.source.label()));
        lines.push(format!("cast_target={}", case.target.label()));
        lines.push(format!("cast_status={:?}", case.status));
        lines.push(format!("cast_scope={}", case.bounded_scope));
        if let Some(cast_kind) = case.cast_kind {
            lines.push(format!("cast_kind={cast_kind:?}"));
        }
        if let Some(refusal) = &case.refusal {
            lines.push(format!("cast_refusal_code={:?}", refusal.code));
            lines.push(format!("cast_refusal_scope={:?}", refusal.scope));
            lines.push(format!("cast_refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("cast_refusal_subject={subject}"));
            }
        }
    }
    for case in backend_cases {
        lines.push(format!("backend_case_id={}", case.case_id));
        lines.push(format!("backend_family={}", case.backend.label()));
        lines.push(format!("backend_dtype={}", case.dtype.label()));
        lines.push(format!("backend_status={:?}", case.status));
        lines.push(format!("backend_scope={}", case.bounded_scope));
        if let Some(refusal) = &case.refusal {
            lines.push(format!("backend_refusal_code={:?}", refusal.code));
            lines.push(format!("backend_refusal_scope={:?}", refusal.scope));
            lines.push(format!("backend_refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("backend_refusal_subject={subject}"));
            }
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// Operation family used by the bounded autocast-policy surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutocastOperationFamily {
    /// Matrix multiply or GEMM-style kernels.
    Matmul,
    /// Pointwise arithmetic kernels.
    Pointwise,
    /// Reductions that may require a wider accumulator.
    Reduction,
    /// Attention-family kernels.
    Attention,
}

impl AutocastOperationFamily {
    fn label(self) -> &'static str {
        match self {
            Self::Matmul => "matmul",
            Self::Pointwise => "pointwise",
            Self::Reduction => "reduction",
            Self::Attention => "attention",
        }
    }
}

/// One bounded autocast precision policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutocastPrecisionPolicy {
    /// Backend family against which the policy is evaluated.
    pub backend: DTypeBackendFamily,
    /// Preferred low-precision compute dtype.
    pub preferred_low_precision: ExtendedDType,
}

impl AutocastPrecisionPolicy {
    /// Creates a bounded autocast precision policy.
    #[must_use]
    pub const fn new(backend: DTypeBackendFamily, preferred_low_precision: ExtendedDType) -> Self {
        Self {
            backend,
            preferred_low_precision,
        }
    }

    /// Resolves one operation/input pair through the canonical bounded
    /// autocast matrix.
    pub fn resolve(
        &self,
        operation: AutocastOperationFamily,
        input_dtype: ExtendedDType,
    ) -> Result<AutocastPolicyResolution, PsionicRefusal> {
        builtin_autocast_policy_matrix_report().resolve(self, operation, input_dtype)
    }
}

/// Numerics diagnostic emitted by one bounded autocast rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutocastNumericsDiagnostic {
    /// Mantissa width is reduced relative to the input dtype.
    ReducedMantissa,
    /// Dynamic range is reduced relative to the input dtype.
    ReducedDynamicRange,
    /// The rule keeps an FP32 accumulator even while lowering compute dtype.
    Fp32Accumulator,
    /// The rule preserves the original dtype for numerical stability.
    PreservedForStability,
    /// The rule relies on an explicitly experimental low-precision posture.
    ExperimentalLowPrecision,
}

/// Status for one bounded autocast rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutocastPolicyStatus {
    /// The policy lowers compute dtype on the bounded surface.
    Applied,
    /// The policy preserves the higher-precision input dtype.
    Preserved,
    /// The policy is explicitly refused.
    Refused,
}

/// One bounded autocast policy resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutocastPolicyResolution {
    /// Stable case identifier.
    pub case_id: String,
    /// Policy under test.
    pub policy: AutocastPrecisionPolicy,
    /// Operation family under test.
    pub operation: AutocastOperationFamily,
    /// Input dtype observed by the policy.
    pub input_dtype: ExtendedDType,
    /// Current status for the resolution.
    pub status: AutocastPolicyStatus,
    /// Compute dtype selected by the policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_dtype: Option<ExtendedDType>,
    /// Accumulator dtype selected by the policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accumulator_dtype: Option<ExtendedDType>,
    /// Output dtype selected by the policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dtype: Option<ExtendedDType>,
    /// Numerics diagnostics attached to the rule.
    pub diagnostics: Vec<AutocastNumericsDiagnostic>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the policy is intentionally unsupported today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable bounded autocast policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutocastPolicyMatrixReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Versioned current-scope window.
    pub current_scope_window: String,
    /// Resolution cases carried by the report.
    pub cases: Vec<AutocastPolicyResolution>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl AutocastPolicyMatrixReport {
    fn new(current_scope_window: impl Into<String>, cases: Vec<AutocastPolicyResolution>) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest =
            stable_autocast_policy_matrix_digest(current_scope_window.as_str(), cases.as_slice());
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{}|{}|{:?}",
                case.case_id,
                case.policy.backend.label(),
                case.policy.preferred_low_precision.label(),
                case.operation.label(),
                case.status
            ));
        }
        lines
    }

    /// Resolves one policy request against the bounded matrix.
    pub fn resolve(
        &self,
        policy: &AutocastPrecisionPolicy,
        operation: AutocastOperationFamily,
        input_dtype: ExtendedDType,
    ) -> Result<AutocastPolicyResolution, PsionicRefusal> {
        let Some(case) = self.cases.iter().find(|case| {
            case.policy == *policy && case.operation == operation && case.input_dtype == input_dtype
        }) else {
            return Err(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "autocast policy matrix does not declare a bounded rule for backend `{}`, preferred `{}`, op `{}`, input `{}`",
                        policy.backend.label(),
                        policy.preferred_low_precision.label(),
                        operation.label(),
                        input_dtype.label()
                    ),
                )
                .with_subject(format!(
                    "{}:{}:{}:{}",
                    policy.backend.label(),
                    policy.preferred_low_precision.label(),
                    operation.label(),
                    input_dtype.label()
                )),
            );
        };
        match case.status {
            AutocastPolicyStatus::Applied | AutocastPolicyStatus::Preserved => Ok(case.clone()),
            AutocastPolicyStatus::Refused => Err(case.refusal.clone().unwrap_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    format!(
                        "autocast policy `{}` / `{}` / `{}` / `{}` is intentionally refused in the current scope",
                        policy.backend.label(),
                        policy.preferred_low_precision.label(),
                        operation.label(),
                        input_dtype.label()
                    ),
                )
                .with_subject(format!(
                    "{}:{}:{}:{}",
                    policy.backend.label(),
                    policy.preferred_low_precision.label(),
                    operation.label(),
                    input_dtype.label()
                ))
            })),
        }
    }
}

/// Builds the canonical bounded autocast policy matrix.
#[must_use]
pub fn builtin_autocast_policy_matrix_report() -> AutocastPolicyMatrixReport {
    AutocastPolicyMatrixReport::new(
        String::from("psionic_autocast_v1"),
        vec![
            supported_autocast_case(SupportedAutocastCaseSeed {
                case_id: "current_runtime_backends.bf16.matmul.f32",
                policy: AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    ExtendedDType::BF16,
                ),
                operation: AutocastOperationFamily::Matmul,
                input_dtype: ExtendedDType::F32,
                status: AutocastPolicyStatus::Applied,
                compute_dtype: ExtendedDType::BF16,
                accumulator_dtype: ExtendedDType::F32,
                output_dtype: ExtendedDType::BF16,
                diagnostics: vec![
                    AutocastNumericsDiagnostic::ReducedMantissa,
                    AutocastNumericsDiagnostic::Fp32Accumulator,
                ],
                bounded_scope: "Current runtime backends may lower `f32` matmul inputs into `bf16` compute while keeping an `f32` accumulator.",
            }),
            supported_autocast_case(SupportedAutocastCaseSeed {
                case_id: "current_runtime_backends.f16.pointwise.f32",
                policy: AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    ExtendedDType::F16,
                ),
                operation: AutocastOperationFamily::Pointwise,
                input_dtype: ExtendedDType::F32,
                status: AutocastPolicyStatus::Applied,
                compute_dtype: ExtendedDType::F16,
                accumulator_dtype: ExtendedDType::F16,
                output_dtype: ExtendedDType::F16,
                diagnostics: vec![AutocastNumericsDiagnostic::ReducedMantissa],
                bounded_scope: "Current runtime backends may lower seeded pointwise `f32` work into `f16` when the bounded policy explicitly prefers it.",
            }),
            supported_autocast_case(SupportedAutocastCaseSeed {
                case_id: "current_runtime_backends.f16.reduction.f32",
                policy: AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    ExtendedDType::F16,
                ),
                operation: AutocastOperationFamily::Reduction,
                input_dtype: ExtendedDType::F32,
                status: AutocastPolicyStatus::Preserved,
                compute_dtype: ExtendedDType::F32,
                accumulator_dtype: ExtendedDType::F32,
                output_dtype: ExtendedDType::F32,
                diagnostics: vec![AutocastNumericsDiagnostic::PreservedForStability],
                bounded_scope: "Reductions stay at `f32` in the current bounded policy instead of silently downcasting numerically sensitive accumulations.",
            }),
            supported_autocast_case(SupportedAutocastCaseSeed {
                case_id: "meta_execution.f8_e4m3fn.matmul.f32",
                policy: AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::MetaExecution,
                    ExtendedDType::F8E4M3Fn,
                ),
                operation: AutocastOperationFamily::Matmul,
                input_dtype: ExtendedDType::F32,
                status: AutocastPolicyStatus::Applied,
                compute_dtype: ExtendedDType::F8E4M3Fn,
                accumulator_dtype: ExtendedDType::F16,
                output_dtype: ExtendedDType::F16,
                diagnostics: vec![
                    AutocastNumericsDiagnostic::ExperimentalLowPrecision,
                    AutocastNumericsDiagnostic::ReducedDynamicRange,
                ],
                bounded_scope: "Meta execution may carry an experimental float8 autocast rule even though current runtime backends do not materialize that path yet.",
            }),
            refused_autocast_case(
                "current_runtime_backends.bf16.pointwise.complex64",
                AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    ExtendedDType::BF16,
                ),
                AutocastOperationFamily::Pointwise,
                ExtendedDType::Complex64,
                "Current runtime backends do not yet autocast complex inputs through the bounded precision-policy surface.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not autocast `complex64` inputs",
                )
                .with_subject("current_runtime_backends:bf16:pointwise:complex64"),
            ),
            refused_autocast_case(
                "current_runtime_backends.f8_e5m2.matmul.f32",
                AutocastPrecisionPolicy::new(
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    ExtendedDType::F8E5M2,
                ),
                AutocastOperationFamily::Matmul,
                ExtendedDType::F32,
                "Current runtime backends do not yet materialize `f8_e5m2` autocast compute, so that preference is explicitly refused instead of silently ignored.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not materialize `f8_e5m2` autocast compute",
                )
                .with_subject("current_runtime_backends:f8_e5m2:matmul:f32"),
            ),
        ],
    )
}

struct SupportedAutocastCaseSeed<'a> {
    case_id: &'a str,
    policy: AutocastPrecisionPolicy,
    operation: AutocastOperationFamily,
    input_dtype: ExtendedDType,
    status: AutocastPolicyStatus,
    compute_dtype: ExtendedDType,
    accumulator_dtype: ExtendedDType,
    output_dtype: ExtendedDType,
    diagnostics: Vec<AutocastNumericsDiagnostic>,
    bounded_scope: &'a str,
}

fn supported_autocast_case(seed: SupportedAutocastCaseSeed<'_>) -> AutocastPolicyResolution {
    AutocastPolicyResolution {
        case_id: String::from(seed.case_id),
        policy: seed.policy,
        operation: seed.operation,
        input_dtype: seed.input_dtype,
        status: seed.status,
        compute_dtype: Some(seed.compute_dtype),
        accumulator_dtype: Some(seed.accumulator_dtype),
        output_dtype: Some(seed.output_dtype),
        diagnostics: seed.diagnostics,
        bounded_scope: String::from(seed.bounded_scope),
        refusal: None,
    }
}

fn refused_autocast_case(
    case_id: &str,
    policy: AutocastPrecisionPolicy,
    operation: AutocastOperationFamily,
    input_dtype: ExtendedDType,
    bounded_scope: &str,
    refusal: PsionicRefusal,
) -> AutocastPolicyResolution {
    AutocastPolicyResolution {
        case_id: String::from(case_id),
        policy,
        operation,
        input_dtype,
        status: AutocastPolicyStatus::Refused,
        compute_dtype: None,
        accumulator_dtype: None,
        output_dtype: None,
        diagnostics: Vec::new(),
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn stable_autocast_policy_matrix_digest(
    current_scope_window: &str,
    cases: &[AutocastPolicyResolution],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in cases {
        lines.push(format!("case_id={}", case.case_id));
        lines.push(format!("backend={}", case.policy.backend.label()));
        lines.push(format!(
            "preferred_low_precision={}",
            case.policy.preferred_low_precision.label()
        ));
        lines.push(format!("operation={}", case.operation.label()));
        lines.push(format!("input_dtype={}", case.input_dtype.label()));
        lines.push(format!("status={:?}", case.status));
        lines.push(format!("bounded_scope={}", case.bounded_scope));
        if let Some(compute_dtype) = case.compute_dtype {
            lines.push(format!("compute_dtype={}", compute_dtype.label()));
        }
        if let Some(accumulator_dtype) = case.accumulator_dtype {
            lines.push(format!("accumulator_dtype={}", accumulator_dtype.label()));
        }
        if let Some(output_dtype) = case.output_dtype {
            lines.push(format!("output_dtype={}", output_dtype.label()));
        }
        for diagnostic in &case.diagnostics {
            lines.push(format!("diagnostic={diagnostic:?}"));
        }
        if let Some(refusal) = &case.refusal {
            lines.push(format!("refusal_code={:?}", refusal.code));
            lines.push(format!("refusal_scope={:?}", refusal.scope));
            lines.push(format!("refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("refusal_subject={subject}"));
            }
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// Quantization mode for stored model weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationMode {
    /// Unquantized weights.
    None,
    /// Symmetric int8 quantization with explicit scale tensors.
    Int8Symmetric,
    /// GGML/GGUF MXFP4 block quantization.
    GgmlMxfp4,
    /// GGML/GGUF Q4_0 block quantization.
    GgmlQ4_0,
    /// GGML/GGUF Q4_1 block quantization.
    GgmlQ4_1,
    /// GGML/GGUF Q8_0 block quantization.
    GgmlQ8_0,
}

impl QuantizationMode {
    /// Returns one stable quantization-mode label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Int8Symmetric => "int8_symmetric",
            Self::GgmlMxfp4 => "ggml_mxfp4",
            Self::GgmlQ4_0 => "ggml_q4_0",
            Self::GgmlQ4_1 => "ggml_q4_1",
            Self::GgmlQ8_0 => "ggml_q8_0",
        }
    }

    /// Returns the GGML block shape for the quantization mode when one exists.
    #[must_use]
    pub const fn ggml_block_spec(self) -> Option<(usize, usize)> {
        match self {
            Self::GgmlMxfp4 => Some((32, 17)),
            Self::GgmlQ4_0 => Some((32, 18)),
            Self::GgmlQ4_1 => Some((32, 20)),
            Self::GgmlQ8_0 => Some((32, 34)),
            Self::None | Self::Int8Symmetric => None,
        }
    }

    /// Returns the block layout for a tensor with the provided logical shape.
    #[must_use]
    pub fn ggml_block_layout(self, shape: &Shape) -> Option<QuantizedBlockLayout> {
        let (elements_per_block, bytes_per_block) = self.ggml_block_spec()?;
        let dims = shape.dims();
        let last_dim = *dims.last()?;
        if last_dim == 0 || last_dim % elements_per_block != 0 {
            return None;
        }
        let element_count = shape.element_count();
        if element_count == 0 || !element_count.is_multiple_of(elements_per_block) {
            return None;
        }
        Some(QuantizedBlockLayout::new(
            elements_per_block,
            bytes_per_block,
            element_count / elements_per_block,
        ))
    }
}

/// Quantization workflow stage covered by the bounded semantics surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationCapabilityStage {
    /// Post-training quantization after dense optimization.
    Ptq,
    /// Quantization-aware training.
    Qat,
    /// Quantized runtime execution semantics above raw file-format decode.
    RuntimeExecution,
    /// Compiler-lowering semantics for quantized programs.
    CompilerLowering,
    /// Export-aware quantization intent on graph handoff surfaces.
    ExportAware,
}

impl QuantizationCapabilityStage {
    fn label(self) -> &'static str {
        match self {
            Self::Ptq => "ptq",
            Self::Qat => "qat",
            Self::RuntimeExecution => "runtime_execution",
            Self::CompilerLowering => "compiler_lowering",
            Self::ExportAware => "export_aware",
        }
    }
}

/// Quantization calibration or observer strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationCalibrationMode {
    /// No calibration is required on the bounded path.
    None,
    /// Min/max calibration statistics are required.
    MinMax,
    /// Histogram or observer-driven calibration statistics are required.
    Histogram,
}

/// Granularity used by a quantization configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationGranularity {
    /// One scale covers the whole tensor.
    PerTensor,
    /// One scale per logical output channel.
    PerChannel,
    /// One scale per block-quantized storage region.
    BlockWise,
}

/// Reusable quantization configuration above loader-only decode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationConfig {
    /// Quantized weight representation under discussion.
    pub weight_mode: QuantizationMode,
    /// Activation dtype exposed to the quantized program.
    pub activation_dtype: ExtendedDType,
    /// Scale granularity used by the configuration.
    pub granularity: QuantizationGranularity,
    /// Calibration or observer strategy required by the flow.
    pub calibration: QuantizationCalibrationMode,
    /// Whether the flow requires explicit fake-quant or observer modules.
    pub requires_observers: bool,
}

impl QuantizationConfig {
    /// Creates one bounded quantization configuration.
    #[must_use]
    pub const fn new(
        weight_mode: QuantizationMode,
        activation_dtype: ExtendedDType,
        granularity: QuantizationGranularity,
        calibration: QuantizationCalibrationMode,
        requires_observers: bool,
    ) -> Self {
        Self {
            weight_mode,
            activation_dtype,
            granularity,
            calibration,
            requires_observers,
        }
    }
}

/// Status for one bounded quantization capability case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationCapabilityStatus {
    /// The flow is explicitly supported in the current bounded scope.
    Supported,
    /// The flow is explicitly refused in the current bounded scope.
    Refused,
}

/// One machine-readable bounded quantization capability case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationCapabilityCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Quantization workflow stage under test.
    pub stage: QuantizationCapabilityStage,
    /// Backend family under test.
    pub backend: DTypeBackendFamily,
    /// Quantization configuration under test.
    pub config: QuantizationConfig,
    /// Current status for the case.
    pub status: QuantizationCapabilityStatus,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the stage/config/backend tuple is unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable bounded report for quantization capability semantics above
/// loader-only decode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationCapabilitySemanticsReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Versioned current-scope window.
    pub current_scope_window: String,
    /// Seeded capability cases that define the current scope.
    pub cases: Vec<QuantizationCapabilityCaseResult>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl QuantizationCapabilitySemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<QuantizationCapabilityCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest =
            stable_quantization_capability_digest(current_scope_window.as_str(), cases.as_slice());
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{}|{}|{:?}",
                case.case_id,
                case.stage.label(),
                case.backend.label(),
                case.config.weight_mode.label(),
                case.status
            ));
        }
        lines
    }

    /// Validates one stage/backend/config tuple against the bounded report.
    pub fn validate_support(
        &self,
        stage: QuantizationCapabilityStage,
        backend: DTypeBackendFamily,
        config: &QuantizationConfig,
    ) -> Result<(), PsionicRefusal> {
        let Some(case) = self
            .cases
            .iter()
            .find(|case| case.stage == stage && case.backend == backend && case.config == *config)
        else {
            return Err(
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedOp,
                    PsionicRefusalScope::Graph,
                    format!(
                        "quantization capability matrix does not declare `{}` / `{}` / `{}` / `{}` in the bounded scope",
                        stage.label(),
                        backend.label(),
                        config.weight_mode.label(),
                        config.activation_dtype.label()
                    ),
                )
                .with_subject(format!(
                    "{}:{}:{}:{}",
                    stage.label(),
                    backend.label(),
                    config.weight_mode.label(),
                    config.activation_dtype.label()
                )),
            );
        };
        match case.status {
            QuantizationCapabilityStatus::Supported => Ok(()),
            QuantizationCapabilityStatus::Refused => Err(case.refusal.clone().unwrap_or_else(|| {
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    format!(
                        "quantization capability `{}` / `{}` / `{}` / `{}` is intentionally refused in the current scope",
                        stage.label(),
                        backend.label(),
                        config.weight_mode.label(),
                        config.activation_dtype.label()
                    ),
                )
                .with_subject(format!(
                    "{}:{}:{}:{}",
                    stage.label(),
                    backend.label(),
                    config.weight_mode.label(),
                    config.activation_dtype.label()
                ))
            })),
        }
    }
}

/// Builds the canonical bounded quantization capability report above
/// loader-only decode.
#[must_use]
pub fn builtin_quantization_capability_semantics_report() -> QuantizationCapabilitySemanticsReport {
    QuantizationCapabilitySemanticsReport::new(
        String::from("psionic_quantization_v1"),
        vec![
            supported_quantization_case(
                "current_runtime_backends.ptq.int8_symmetric_per_channel",
                QuantizationCapabilityStage::Ptq,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::Int8Symmetric,
                    ExtendedDType::F32,
                    QuantizationGranularity::PerChannel,
                    QuantizationCalibrationMode::MinMax,
                    false,
                ),
                "Current runtime backends support a bounded PTQ surface for symmetric int8 weights with per-channel scales and explicit calibration metadata.",
            ),
            supported_quantization_case(
                "current_runtime_backends.qat.int8_symmetric_observer",
                QuantizationCapabilityStage::Qat,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::Int8Symmetric,
                    ExtendedDType::F32,
                    QuantizationGranularity::PerTensor,
                    QuantizationCalibrationMode::Histogram,
                    true,
                ),
                "Current runtime backends support a bounded QAT surface for symmetric int8 weights with observer-driven fake-quant metadata instead of implying arbitrary quantizer families.",
            ),
            supported_quantization_case(
                "current_runtime_backends.runtime_execution.ggml_q4_0_f32_activation",
                QuantizationCapabilityStage::RuntimeExecution,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::GgmlQ4_0,
                    ExtendedDType::F32,
                    QuantizationGranularity::BlockWise,
                    QuantizationCalibrationMode::None,
                    false,
                ),
                "Current runtime backends support bounded quantized-matmul execution semantics for `ggml_q4_0` weights with `f32` activations above raw file-format decode.",
            ),
            supported_quantization_case(
                "current_runtime_backends.compiler_lowering.ggml_q4_0_f32_activation",
                QuantizationCapabilityStage::CompilerLowering,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::GgmlQ4_0,
                    ExtendedDType::F32,
                    QuantizationGranularity::BlockWise,
                    QuantizationCalibrationMode::None,
                    false,
                ),
                "Current compiler lowering explicitly preserves bounded quantized-matmul intent for `ggml_q4_0` instead of treating quantization as loader-private trivia.",
            ),
            supported_quantization_case(
                "meta_execution.export_aware.int8_symmetric",
                QuantizationCapabilityStage::ExportAware,
                DTypeBackendFamily::MetaExecution,
                QuantizationConfig::new(
                    QuantizationMode::Int8Symmetric,
                    ExtendedDType::F32,
                    QuantizationGranularity::PerChannel,
                    QuantizationCalibrationMode::MinMax,
                    false,
                ),
                "Meta execution may carry export-aware int8 quantization intent through graph-level semantics even though deployment artifact contracts remain a later issue.",
            ),
            refused_quantization_case(
                "current_runtime_backends.qat.ggml_q4_0",
                QuantizationCapabilityStage::Qat,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::GgmlQ4_0,
                    ExtendedDType::F32,
                    QuantizationGranularity::BlockWise,
                    QuantizationCalibrationMode::Histogram,
                    true,
                ),
                "Current runtime backends do not yet claim block-quant QAT; that path remains explicitly refused instead of being smuggled in through GGUF decode support.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not yet support block-quant QAT",
                )
                .with_subject("qat:current_runtime_backends:ggml_q4_0:f32"),
            ),
            refused_quantization_case(
                "current_runtime_backends.runtime_execution.int8_symmetric_bf16_activation",
                QuantizationCapabilityStage::RuntimeExecution,
                DTypeBackendFamily::CurrentRuntimeBackends,
                QuantizationConfig::new(
                    QuantizationMode::Int8Symmetric,
                    ExtendedDType::BF16,
                    QuantizationGranularity::PerChannel,
                    QuantizationCalibrationMode::None,
                    false,
                ),
                "Current runtime backends keep quantized execution semantics bounded to seeded `f32` activation posture instead of claiming broad activation-dtype closure.",
                PsionicRefusal::new(
                    PsionicRefusalCode::UnsupportedBackendCapability,
                    PsionicRefusalScope::Runtime,
                    "current runtime backends do not yet execute seeded int8 quantization with bf16 activations",
                )
                .with_subject("runtime_execution:current_runtime_backends:int8_symmetric:bf16"),
            ),
        ],
    )
}

fn supported_quantization_case(
    case_id: &str,
    stage: QuantizationCapabilityStage,
    backend: DTypeBackendFamily,
    config: QuantizationConfig,
    bounded_scope: &str,
) -> QuantizationCapabilityCaseResult {
    QuantizationCapabilityCaseResult {
        case_id: String::from(case_id),
        stage,
        backend,
        config,
        status: QuantizationCapabilityStatus::Supported,
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_quantization_case(
    case_id: &str,
    stage: QuantizationCapabilityStage,
    backend: DTypeBackendFamily,
    config: QuantizationConfig,
    bounded_scope: &str,
    refusal: PsionicRefusal,
) -> QuantizationCapabilityCaseResult {
    QuantizationCapabilityCaseResult {
        case_id: String::from(case_id),
        stage,
        backend,
        config,
        status: QuantizationCapabilityStatus::Refused,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn stable_quantization_capability_digest(
    current_scope_window: &str,
    cases: &[QuantizationCapabilityCaseResult],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in cases {
        lines.push(format!("case_id={}", case.case_id));
        lines.push(format!("stage={}", case.stage.label()));
        lines.push(format!("backend={}", case.backend.label()));
        lines.push(format!("weight_mode={}", case.config.weight_mode.label()));
        lines.push(format!(
            "activation_dtype={}",
            case.config.activation_dtype.label()
        ));
        lines.push(format!("granularity={:?}", case.config.granularity));
        lines.push(format!("calibration={:?}", case.config.calibration));
        lines.push(format!(
            "requires_observers={}",
            case.config.requires_observers
        ));
        lines.push(format!("status={:?}", case.status));
        lines.push(format!("bounded_scope={}", case.bounded_scope));
        if let Some(refusal) = &case.refusal {
            lines.push(format!("refusal_code={:?}", refusal.code));
            lines.push(format!("refusal_scope={:?}", refusal.scope));
            lines.push(format!("refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("refusal_subject={subject}"));
            }
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// Stable floating-point parameter encoded via raw `f32` bit representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableF32(pub u32);

impl StableF32 {
    /// Creates a stable floating-point parameter from an `f32`.
    #[must_use]
    pub const fn from_f32(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// Decodes the stored value as an `f32`.
    #[must_use]
    pub const fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Backend-extension family kept separate from the small visible primitive surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendExtensionKind {
    /// Root-mean-square normalization over the last dimension.
    RmsNorm,
    /// Layer normalization over the last dimension.
    LayerNorm,
    /// Rotary position embedding application.
    RotaryEmbedding,
    /// Scaled dot-product attention over query/key/value tensors.
    ScaledDotProductAttention,
    /// Matmul that is eligible for a quantized-GEMM specialization.
    QuantizedMatmul,
}

impl BackendExtensionKind {
    /// Returns a stable extension label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RmsNorm => "rms_norm",
            Self::LayerNorm => "layer_norm",
            Self::RotaryEmbedding => "rotary_embedding",
            Self::ScaledDotProductAttention => "scaled_dot_product_attention",
            Self::QuantizedMatmul => "quantized_matmul",
        }
    }
}

/// Typed backend-extension operation carried through graph and plan surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendExtensionOp {
    /// Root-mean-square normalization over the last dimension.
    RmsNorm {
        /// Epsilon added before square root for numeric stability.
        epsilon: StableF32,
    },
    /// Layer normalization over the last dimension.
    LayerNorm {
        /// Epsilon added before square root for numeric stability.
        epsilon: StableF32,
    },
    /// Rotary position embedding application.
    RotaryEmbedding {
        /// Whether pairs are interleaved on the last dimension.
        interleaved: bool,
    },
    /// Scaled dot-product attention over query/key/value tensors.
    ScaledDotProductAttention {
        /// Multiplicative scale applied to query-key dot products.
        scale: StableF32,
        /// Whether causal masking is applied.
        causal: bool,
    },
    /// Matmul that is eligible for a quantized-GEMM specialization.
    QuantizedMatmul {
        /// Quantized family of the right-hand-side weights.
        rhs_mode: QuantizationMode,
    },
}

impl BackendExtensionOp {
    /// Returns the extension family.
    #[must_use]
    pub const fn kind(&self) -> BackendExtensionKind {
        match self {
            Self::RmsNorm { .. } => BackendExtensionKind::RmsNorm,
            Self::LayerNorm { .. } => BackendExtensionKind::LayerNorm,
            Self::RotaryEmbedding { .. } => BackendExtensionKind::RotaryEmbedding,
            Self::ScaledDotProductAttention { .. } => {
                BackendExtensionKind::ScaledDotProductAttention
            }
            Self::QuantizedMatmul { .. } => BackendExtensionKind::QuantizedMatmul,
        }
    }

    /// Returns a stable extension label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.kind().label()
    }
}

/// Stable block layout for GGML/GGUF quantized tensor storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuantizedBlockLayout {
    /// Logical scalar elements covered by a single quantized block.
    pub elements_per_block: usize,
    /// Serialized byte width of a single quantized block.
    pub bytes_per_block: usize,
    /// Number of quantized blocks in the tensor.
    pub block_count: usize,
}

impl QuantizedBlockLayout {
    /// Creates an explicit block layout.
    #[must_use]
    pub const fn new(
        elements_per_block: usize,
        bytes_per_block: usize,
        block_count: usize,
    ) -> Self {
        Self {
            elements_per_block,
            bytes_per_block,
            block_count,
        }
    }

    /// Returns the logical scalar element count represented by the layout.
    #[must_use]
    pub const fn element_count(self) -> usize {
        self.elements_per_block * self.block_count
    }

    /// Returns the serialized byte length represented by the layout.
    #[must_use]
    pub const fn byte_len(self) -> usize {
        self.bytes_per_block * self.block_count
    }
}

/// Runtime backend family for a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Host CPU execution.
    Cpu,
    /// NVIDIA CUDA execution.
    Cuda,
    /// Apple Metal execution.
    Metal,
    /// AMD KFD execution.
    AmdKfd,
    /// AMD userspace execution.
    AmdUserspace,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
            Self::AmdKfd => "amd_kfd",
            Self::AmdUserspace => "amd_userspace",
        };
        f.write_str(label)
    }
}

/// Logical device descriptor used by graph, runtime, and provider layers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Device {
    kind: DeviceKind,
    ordinal: u16,
    label: Option<String>,
}

impl Device {
    /// Creates a new device descriptor.
    #[must_use]
    pub fn new(kind: DeviceKind, ordinal: u16, label: Option<String>) -> Self {
        Self {
            kind,
            ordinal,
            label,
        }
    }

    /// Returns a default CPU device.
    #[must_use]
    pub fn cpu() -> Self {
        Self::new(DeviceKind::Cpu, 0, Some(String::from("cpu:0")))
    }

    /// Returns the device kind.
    #[must_use]
    pub const fn kind(&self) -> DeviceKind {
        self.kind
    }

    /// Returns the device ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    /// Returns an optional friendly label.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = self.label() {
            write!(f, "{label}")
        } else {
            write!(f, "{}:{}", self.kind, self.ordinal)
        }
    }
}

/// Tensor shape descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shape {
    dims: Vec<usize>,
}

impl Shape {
    /// Creates a new shape.
    #[must_use]
    pub fn new(dims: impl Into<Vec<usize>>) -> Self {
        Self { dims: dims.into() }
    }

    /// Creates a scalar shape.
    #[must_use]
    pub fn scalar() -> Self {
        Self { dims: Vec::new() }
    }

    /// Returns the shape dimensions.
    #[must_use]
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    /// Returns the dimension at the provided axis.
    #[must_use]
    pub fn dim(&self, axis: usize) -> Option<usize> {
        self.dims.get(axis).copied()
    }

    /// Returns the rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Returns the number of addressable elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        if self.dims.is_empty() {
            1
        } else {
            self.dims.iter().product()
        }
    }

    /// Returns a new shape with axes permuted according to `order`.
    #[must_use]
    pub fn permuted(&self, order: &[usize]) -> Option<Self> {
        if order.len() != self.rank() || !is_permutation(order) {
            return None;
        }
        let dims = order
            .iter()
            .map(|&axis| self.dims[axis])
            .collect::<Vec<_>>();
        Some(Self::new(dims))
    }

    /// Returns a new shape with the given axis removed.
    #[must_use]
    pub fn without_axis(&self, axis: usize) -> Option<Self> {
        if axis >= self.rank() {
            return None;
        }
        let mut dims = self.dims.clone();
        dims.remove(axis);
        Some(Self::new(dims))
    }

    /// Returns the broadcast-compatible shape for two tensors when one exists.
    #[must_use]
    pub fn broadcast_with(&self, other: &Self) -> Option<Self> {
        let rank = self.rank().max(other.rank());
        let left_padding = rank.saturating_sub(self.rank());
        let right_padding = rank.saturating_sub(other.rank());
        let mut dims = Vec::with_capacity(rank);

        for axis in 0..rank {
            let left = if axis < left_padding {
                1
            } else {
                self.dims[axis - left_padding]
            };
            let right = if axis < right_padding {
                1
            } else {
                other.dims[axis - right_padding]
            };
            if left == right || left == 1 {
                dims.push(right);
            } else if right == 1 {
                dims.push(left);
            } else {
                return None;
            }
        }

        Some(Self::new(dims))
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.dims)
    }
}

/// Reachable storage span for one tensor layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayoutStorageSpan {
    /// Inclusive starting storage index.
    pub start: usize,
    /// Exclusive ending storage index.
    pub end_exclusive: usize,
}

/// Logical view posture for one tensor layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSemantics {
    /// Dense row-major tensor layout.
    Dense,
    /// Alias-preserving non-broadcast view into an existing storage span.
    AliasView,
    /// Zero-stride broadcast view into an existing storage span.
    BroadcastView,
}

/// Typed alias relation between a derived layout and one source layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayoutAliasRelation {
    /// Source storage span that the derived layout must stay within.
    pub source_span: LayoutStorageSpan,
    /// Derived reachable storage span.
    pub derived_span: LayoutStorageSpan,
    /// View posture of the derived layout.
    pub semantics: ViewSemantics,
}

/// Layout metadata for a logical tensor view.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layout {
    shape: Shape,
    strides: Vec<usize>,
    offset: usize,
}

impl Layout {
    /// Creates a layout from explicit fields.
    #[must_use]
    pub fn new(shape: Shape, strides: Vec<usize>, offset: usize) -> Self {
        Self {
            shape,
            strides,
            offset,
        }
    }

    /// Creates a standard row-major contiguous layout.
    #[must_use]
    pub fn contiguous(shape: Shape) -> Self {
        let mut strides = vec![0; shape.rank()];
        let mut running = 1;
        for (index, dim) in shape.dims().iter().enumerate().rev() {
            strides[index] = running;
            running *= *dim;
        }
        Self::new(shape, strides, 0)
    }

    /// Returns the layout shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Returns the logical strides.
    #[must_use]
    pub fn strides(&self) -> &[usize] {
        &self.strides
    }

    /// Returns the storage offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the minimum backing storage length required by the layout.
    #[must_use]
    pub fn storage_size(&self) -> usize {
        if self.shape.rank() == 0 {
            return self.offset + 1;
        }

        let span = self
            .shape
            .dims()
            .iter()
            .zip(self.strides.iter())
            .map(|(&dim, &stride)| dim.saturating_sub(1) * stride)
            .sum::<usize>();
        self.offset + span + 1
    }

    /// Returns whether the layout is row-major contiguous.
    #[must_use]
    pub fn is_contiguous(&self) -> bool {
        *self == Self::contiguous(self.shape.clone())
    }

    /// Returns the reachable storage span for the layout.
    #[must_use]
    pub fn storage_span(&self) -> LayoutStorageSpan {
        LayoutStorageSpan {
            start: self.offset,
            end_exclusive: self.storage_size(),
        }
    }

    /// Returns the view posture for the layout.
    #[must_use]
    pub fn view_semantics(&self) -> ViewSemantics {
        if self.is_broadcast_view() {
            ViewSemantics::BroadcastView
        } else if self.is_contiguous() && self.offset == 0 {
            ViewSemantics::Dense
        } else {
            ViewSemantics::AliasView
        }
    }

    /// Returns whether the layout is a zero-stride broadcast view over a
    /// smaller source span.
    #[must_use]
    pub fn is_broadcast_view(&self) -> bool {
        self.shape
            .dims()
            .iter()
            .zip(self.strides.iter())
            .any(|(&dim, &stride)| dim > 1 && stride == 0)
    }

    /// Returns whether this layout can be realized as an alias-preserving view
    /// over `source` storage.
    #[must_use]
    pub fn is_alias_preserving_transform_of(&self, source: &Self) -> bool {
        self.offset >= source.offset && self.storage_size() <= source.storage_size()
    }

    /// Returns the typed alias relation to `source` when the derived layout
    /// stays within the source storage span.
    #[must_use]
    pub fn alias_relation_to_source(&self, source: &Self) -> Option<LayoutAliasRelation> {
        self.view_semantics_relative_to(source)
            .map(|semantics| LayoutAliasRelation {
                source_span: source.storage_span(),
                derived_span: self.storage_span(),
                semantics,
            })
    }

    /// Returns the derived view posture relative to `source` when the layout
    /// stays within the source storage span.
    #[must_use]
    pub fn view_semantics_relative_to(&self, source: &Self) -> Option<ViewSemantics> {
        self.is_alias_preserving_transform_of(source).then(|| {
            if self.is_broadcast_view() {
                ViewSemantics::BroadcastView
            } else if self == source {
                ViewSemantics::Dense
            } else {
                ViewSemantics::AliasView
            }
        })
    }

    /// Returns a permuted layout if `order` is valid.
    #[must_use]
    pub fn permuted(&self, order: &[usize]) -> Option<Self> {
        let shape = self.shape.permuted(order)?;
        let strides = order.iter().map(|&axis| self.strides[axis]).collect();
        Some(Self::new(shape, strides, self.offset))
    }

    /// Returns a sliced layout if the requested bounds are valid.
    #[must_use]
    pub fn sliced(&self, axis: usize, start: usize, end: usize) -> Option<Self> {
        let dim = self.shape.dim(axis)?;
        if start > end || end > dim {
            return None;
        }
        let mut dims = self.shape.dims.clone();
        dims[axis] = end - start;
        let offset = self.offset + (start * self.strides[axis]);
        Some(Self::new(Shape::new(dims), self.strides.clone(), offset))
    }

    /// Returns a selected layout if the requested index is valid.
    #[must_use]
    pub fn selected(&self, axis: usize, index: usize) -> Option<Self> {
        let dim = self.shape.dim(axis)?;
        if index >= dim {
            return None;
        }
        let shape = self.shape.without_axis(axis)?;
        let mut strides = self.strides.clone();
        strides.remove(axis);
        let offset = self.offset + (index * self.strides[axis]);
        Some(Self::new(shape, strides, offset))
    }

    /// Returns an expanded layout if the requested target shape is valid.
    #[must_use]
    pub fn expanded(&self, target_shape: &Shape) -> Option<Self> {
        if target_shape.rank() < self.shape.rank() {
            return None;
        }

        let rank_padding = target_shape.rank() - self.shape.rank();
        let storage_stride = self.storage_size();
        let mut current_dims = vec![1; rank_padding];
        current_dims.extend_from_slice(self.shape.dims());

        let mut strides = vec![storage_stride; rank_padding];
        strides.extend_from_slice(&self.strides);

        for (axis, (&current, &target)) in current_dims.iter().zip(target_shape.dims()).enumerate()
        {
            if current == target {
                continue;
            }
            if current != 1 {
                return None;
            }
            strides[axis] = 0;
        }

        Some(Self::new(target_shape.clone(), strides, self.offset))
    }
}

/// Static tensor metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TensorSpec {
    layout: Layout,
    dtype: DType,
    device: Device,
}

impl TensorSpec {
    /// Creates a new contiguous tensor specification.
    #[must_use]
    pub fn new(shape: Shape, dtype: DType, device: Device) -> Self {
        Self {
            layout: Layout::contiguous(shape),
            dtype,
            device,
        }
    }

    /// Creates a tensor specification from an explicit layout.
    #[must_use]
    pub fn from_layout(layout: Layout, dtype: DType, device: Device) -> Self {
        Self {
            layout,
            dtype,
            device,
        }
    }

    /// Returns the tensor layout.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns the tensor shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        self.layout.shape()
    }

    /// Returns the tensor dtype.
    #[must_use]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Returns the target device.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Returns a copy with a different contiguous shape.
    #[must_use]
    pub fn with_shape(&self, shape: Shape) -> Self {
        Self::new(shape, self.dtype, self.device.clone())
    }

    /// Returns a copy with a different layout.
    #[must_use]
    pub fn with_layout(&self, layout: Layout) -> Self {
        Self::from_layout(layout, self.dtype, self.device.clone())
    }

    /// Returns the number of addressable elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.shape().element_count()
    }

    /// Returns the minimum backing storage length required by the tensor
    /// layout.
    #[must_use]
    pub fn storage_size(&self) -> usize {
        self.layout.storage_size()
    }
}

/// Small data container used for constants and host-visible results.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TensorData {
    /// 32-bit floating point tensor payload.
    F32(Vec<f32>),
    /// Quantized GGML/GGUF block payload.
    QuantizedBlocks(QuantizedTensorData),
}

/// Quantized GGML/GGUF block payload kept in graph/runtime constants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedTensorData {
    /// Quantization family for the blocks.
    pub mode: QuantizationMode,
    /// Stable GGML block layout for the logical tensor.
    pub layout: QuantizedBlockLayout,
    /// Serialized quantized block bytes.
    pub bytes: Vec<u8>,
}

impl QuantizedTensorData {
    /// Creates a quantized block payload.
    #[must_use]
    pub fn new(
        mode: QuantizationMode,
        layout: QuantizedBlockLayout,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            mode,
            layout,
            bytes: bytes.into(),
        }
    }
}

impl TensorData {
    /// Returns the element count of the payload.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::F32(values) => values.len(),
            Self::QuantizedBlocks(data) => data.layout.element_count(),
        }
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the payload as an `f32` slice when the storage is dense.
    #[must_use]
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            Self::QuantizedBlocks(_) => None,
        }
    }

    /// Returns the quantized payload when the storage is GGML/GGUF blocks.
    #[must_use]
    pub fn as_quantized_blocks(&self) -> Option<&QuantizedTensorData> {
        match self {
            Self::F32(_) => None,
            Self::QuantizedBlocks(data) => Some(data),
        }
    }
}

/// High-level operation provenance for a lazy tensor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LazyOp {
    /// Graph input.
    Input { name: String },
    /// Constant tensor.
    Constant,
    /// Gradient-stopping identity.
    Detach,
    /// Binary add.
    Add,
    /// Binary multiply.
    Mul,
    /// Matrix multiplication.
    Matmul,
    /// Tensor reshape.
    Reshape,
    /// Tensor permute.
    Permute { axes: Vec<usize> },
    /// Tensor slice.
    Slice {
        axis: usize,
        start: usize,
        end: usize,
    },
    /// Tensor select.
    Select { axis: usize, index: usize },
    /// Tensor concat.
    Concat { axis: usize },
    /// Tensor expand/broadcast.
    Expand { shape: Shape },
    /// Tensor dtype cast.
    Cast { dtype: DType },
    /// Full or axis-specific reduction.
    ReduceSum { axis: Option<usize> },
    /// Typed backend-extension operation kept separate from primitive ops.
    BackendExtension { op: BackendExtensionOp },
}

/// Public tensor handle produced by graph construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tensor {
    id: TensorId,
    spec: TensorSpec,
    op: LazyOp,
}

impl Tensor {
    /// Creates a new tensor handle.
    #[must_use]
    pub fn new(id: TensorId, spec: TensorSpec, op: LazyOp) -> Self {
        Self { id, spec, op }
    }

    /// Returns the tensor identifier.
    #[must_use]
    pub const fn id(&self) -> TensorId {
        self.id
    }

    /// Returns the tensor specification.
    #[must_use]
    pub fn spec(&self) -> &TensorSpec {
        &self.spec
    }

    /// Returns the lazy operation provenance.
    #[must_use]
    pub fn op(&self) -> &LazyOp {
        &self.op
    }
}

fn is_permutation(order: &[usize]) -> bool {
    let mut seen = vec![false; order.len()];
    for &axis in order {
        if axis >= order.len() || seen[axis] {
            return false;
        }
        seen[axis] = true;
    }
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        AutocastNumericsDiagnostic, AutocastOperationFamily, AutocastPolicyStatus,
        AutocastPrecisionPolicy, DType, DTypeBackendFamily, DTypeCastKind, DTypeClass, Device,
        DeviceKind, ExtendedDType, ExtendedDTypeClass, Layout, PsionicRefusal, PsionicRefusalCode,
        PsionicRefusalScope, QuantizationCalibrationMode, QuantizationCapabilityStage,
        QuantizationConfig, QuantizationGranularity, QuantizationMode, Shape, TensorSpec,
        ViewSemantics, builtin_advanced_dtype_semantics_report,
        builtin_autocast_policy_matrix_report, builtin_quantization_capability_semantics_report,
    };

    #[test]
    fn scalar_shape_counts_as_one_element() {
        assert_eq!(Shape::scalar().element_count(), 1);
    }

    #[test]
    fn dense_shape_reports_element_count() {
        assert_eq!(Shape::new(vec![2, 3, 4]).element_count(), 24);
    }

    #[test]
    fn tensor_spec_retains_device_and_dtype() {
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());

        assert_eq!(spec.dtype(), DType::F32);
        assert_eq!(spec.device().kind(), super::DeviceKind::Cpu);
        assert!(spec.layout().is_contiguous());
    }

    #[test]
    fn cuda_device_kind_formats_stably() {
        let device = Device::new(DeviceKind::Cuda, 0, None);
        assert_eq!(device.kind(), DeviceKind::Cuda);
        assert_eq!(device.to_string(), "cuda:0");
    }

    #[test]
    fn layout_permute_updates_shape_and_strides() {
        let layout = Layout::contiguous(Shape::new(vec![2, 3, 4]));
        let permuted = layout.permuted(&[1, 0, 2]);
        assert!(permuted.is_some());
        let Some(permuted) = permuted else {
            return;
        };

        assert_eq!(permuted.shape().dims(), &[3, 2, 4]);
        assert_eq!(permuted.strides(), &[4, 12, 1]);
        assert!(!permuted.is_contiguous());
    }

    #[test]
    fn layout_expand_uses_zero_strides() {
        let layout = Layout::contiguous(Shape::new(vec![1, 3]));
        let expanded = layout.expanded(&Shape::new(vec![4, 3]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };

        assert_eq!(expanded.shape().dims(), &[4, 3]);
        assert_eq!(expanded.strides(), &[0, 1]);
        assert!(expanded.is_broadcast_view());
    }

    #[test]
    fn layout_expand_can_increase_rank() {
        let layout = Layout::contiguous(Shape::new(vec![2]));
        let expanded = layout.expanded(&Shape::new(vec![3, 2]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };

        assert_eq!(expanded.shape().dims(), &[3, 2]);
        assert_eq!(expanded.strides(), &[0, 1]);
    }

    #[test]
    fn expanded_layout_storage_size_matches_source_span() {
        let expanded = Layout::contiguous(Shape::new(vec![1, 3])).expanded(&Shape::new(vec![4, 3]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };
        let spec = TensorSpec::from_layout(expanded, DType::F32, Device::cpu());

        assert_eq!(spec.element_count(), 12);
        assert_eq!(spec.storage_size(), 3);
    }

    #[test]
    fn shape_broadcast_merges_trailing_singleton_axes() {
        let shape = Shape::new(vec![2, 1, 3]).broadcast_with(&Shape::new(vec![1, 4, 3]));
        assert_eq!(shape, Some(Shape::new(vec![2, 4, 3])));
    }

    #[test]
    fn shape_broadcast_refuses_incompatible_axes() {
        let shape = Shape::new(vec![2, 3]).broadcast_with(&Shape::new(vec![2, 2]));
        assert!(shape.is_none());
    }

    #[test]
    fn dtype_promotion_prefers_widest_supported_representation() {
        assert_eq!(DType::I8.promote_binary(DType::I8), Some(DType::I8));
        assert_eq!(DType::I8.promote_binary(DType::F16), Some(DType::F16));
        assert_eq!(DType::I8.promote_binary(DType::BF16), Some(DType::BF16));
        assert_eq!(DType::F16.promote_binary(DType::BF16), Some(DType::F32));
        assert_eq!(DType::BF16.promote_binary(DType::F32), Some(DType::F32));
    }

    #[test]
    fn derived_views_remain_alias_preserving_transforms() {
        let source = Layout::contiguous(Shape::new(vec![2, 3]));
        let permuted = source.permuted(&[1, 0]);
        assert!(permuted.is_some());
        let Some(permuted) = permuted else {
            return;
        };
        let sliced = permuted.sliced(0, 1, 3);
        assert!(sliced.is_some());
        let Some(sliced) = sliced else {
            return;
        };
        let selected = sliced.selected(1, 0);
        assert!(selected.is_some());
        let Some(selected) = selected else {
            return;
        };
        let expanded = selected.expanded(&Shape::new(vec![2, 2]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };

        assert!(permuted.is_alias_preserving_transform_of(&source));
        assert!(sliced.is_alias_preserving_transform_of(&source));
        assert!(selected.is_alias_preserving_transform_of(&source));
        assert!(expanded.is_alias_preserving_transform_of(&source));
    }

    #[test]
    fn layout_alias_relation_tracks_dense_and_broadcast_views() {
        let source = Layout::contiguous(Shape::new(vec![2, 3]));
        let sliced = source.sliced(0, 1, 2);
        assert!(sliced.is_some());
        let Some(sliced) = sliced else {
            return;
        };
        let broadcast =
            Layout::contiguous(Shape::new(vec![1, 3])).expanded(&Shape::new(vec![4, 3]));
        assert!(broadcast.is_some());
        let Some(broadcast) = broadcast else {
            return;
        };

        let sliced_relation = sliced.alias_relation_to_source(&source);
        assert!(sliced_relation.is_some());
        let Some(sliced_relation) = sliced_relation else {
            return;
        };
        assert_eq!(sliced_relation.semantics, ViewSemantics::AliasView);
        assert_eq!(sliced_relation.source_span.start, 0);

        let broadcast_source = Layout::contiguous(Shape::new(vec![1, 3]));
        let broadcast_relation = broadcast.alias_relation_to_source(&broadcast_source);
        assert!(broadcast_relation.is_some());
        let Some(broadcast_relation) = broadcast_relation else {
            return;
        };
        assert_eq!(broadcast_relation.semantics, ViewSemantics::BroadcastView);
        assert_eq!(broadcast_relation.derived_span.end_exclusive, 3);
    }

    #[test]
    fn dtype_contracts_mark_current_quantized_and_dense_surface() {
        assert_eq!(DType::F32.class(), DTypeClass::FloatingPoint);
        assert_eq!(DType::BF16.class(), DTypeClass::FloatingPoint);
        assert_eq!(DType::I8.class(), DTypeClass::SignedInteger);
        assert!(DType::F32.supports_quantized_logical_storage());
        assert!(!DType::F16.supports_quantized_logical_storage());
        assert!(!DType::I8.supports_quantized_logical_storage());
    }

    #[test]
    fn advanced_dtype_semantics_report_tracks_seeded_promotion_cast_and_backend_cases() {
        let report = builtin_advanced_dtype_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_advanced_dtype_v1");
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        assert_eq!(ExtendedDType::Bool.class(), ExtendedDTypeClass::Boolean);
        assert_eq!(
            ExtendedDType::Complex64.class(),
            ExtendedDTypeClass::ComplexFloatingPoint
        );
        assert!(ExtendedDType::F8E4M3Fn.is_low_precision());
        assert!(!ExtendedDType::Complex128.is_low_precision());

        let promotion = report
            .resolve_binary_promotion(ExtendedDType::F16, ExtendedDType::BF16)
            .expect("missing seeded f16/bf16 promotion");
        assert_eq!(promotion, ExtendedDType::F32);

        let complex_promotion = report
            .resolve_binary_promotion(ExtendedDType::F64, ExtendedDType::Complex64)
            .expect("missing seeded f64/complex64 promotion");
        assert_eq!(complex_promotion, ExtendedDType::Complex128);

        let refused_promotion = report
            .resolve_binary_promotion(ExtendedDType::F8E5M2, ExtendedDType::BF16)
            .expect_err("seeded float8/bfloat16 promotion should refuse");
        assert_eq!(refused_promotion.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(refused_promotion.scope, PsionicRefusalScope::Graph);
        assert_eq!(refused_promotion.subject.as_deref(), Some("f8_e5m2+bf16"));

        let cast = report
            .resolve_cast(ExtendedDType::F64, ExtendedDType::F32)
            .expect("missing seeded f64->f32 cast");
        assert_eq!(cast, DTypeCastKind::PrecisionLoss);

        let refused_cast = report
            .resolve_cast(ExtendedDType::Complex128, ExtendedDType::F32)
            .expect_err("complex-to-real cast should refuse");
        assert_eq!(refused_cast.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(refused_cast.scope, PsionicRefusalScope::Graph);
        assert_eq!(refused_cast.subject.as_deref(), Some("complex128->f32"));

        let backend = report.validate_backend_support(
            DTypeBackendFamily::CurrentRuntimeBackends,
            ExtendedDType::F32,
        );
        assert!(backend.is_ok());

        let refused_backend = report
            .validate_backend_support(
                DTypeBackendFamily::CurrentRuntimeBackends,
                ExtendedDType::Complex64,
            )
            .expect_err("current runtime backends should refuse complex64");
        assert_eq!(
            refused_backend.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(refused_backend.scope, PsionicRefusalScope::Runtime);
        assert_eq!(
            refused_backend.subject.as_deref(),
            Some("complex64@current_runtime_backends")
        );

        let lowered = ExtendedDType::F32
            .try_into_core_dtype()
            .expect("f32 should lower into compact core dtype");
        assert_eq!(lowered, DType::F32);

        let refused_core = ExtendedDType::Complex64
            .try_into_core_dtype()
            .expect_err("complex64 should not lower into the compact core dtype");
        assert_eq!(
            refused_core.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(refused_core.scope, PsionicRefusalScope::Runtime);
        assert_eq!(refused_core.subject.as_deref(), Some("complex64"));
    }

    #[test]
    fn autocast_policy_matrix_tracks_seeded_backend_rules_and_diagnostics() {
        let report = builtin_autocast_policy_matrix_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_autocast_v1");
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let bf16_matmul = AutocastPrecisionPolicy::new(
            DTypeBackendFamily::CurrentRuntimeBackends,
            ExtendedDType::BF16,
        )
        .resolve(AutocastOperationFamily::Matmul, ExtendedDType::F32)
        .expect("missing seeded bf16 matmul rule");
        assert_eq!(bf16_matmul.status, AutocastPolicyStatus::Applied);
        assert_eq!(bf16_matmul.compute_dtype, Some(ExtendedDType::BF16));
        assert_eq!(bf16_matmul.accumulator_dtype, Some(ExtendedDType::F32));
        assert!(
            bf16_matmul
                .diagnostics
                .contains(&AutocastNumericsDiagnostic::Fp32Accumulator)
        );

        let preserved_reduction = AutocastPrecisionPolicy::new(
            DTypeBackendFamily::CurrentRuntimeBackends,
            ExtendedDType::F16,
        )
        .resolve(AutocastOperationFamily::Reduction, ExtendedDType::F32)
        .expect("missing seeded preserved reduction rule");
        assert_eq!(preserved_reduction.status, AutocastPolicyStatus::Preserved);
        assert_eq!(preserved_reduction.compute_dtype, Some(ExtendedDType::F32));
        assert!(
            preserved_reduction
                .diagnostics
                .contains(&AutocastNumericsDiagnostic::PreservedForStability)
        );

        let experimental_float8 = AutocastPrecisionPolicy::new(
            DTypeBackendFamily::MetaExecution,
            ExtendedDType::F8E4M3Fn,
        )
        .resolve(AutocastOperationFamily::Matmul, ExtendedDType::F32)
        .expect("missing seeded meta float8 rule");
        assert_eq!(experimental_float8.status, AutocastPolicyStatus::Applied);
        assert!(
            experimental_float8
                .diagnostics
                .contains(&AutocastNumericsDiagnostic::ExperimentalLowPrecision)
        );

        let complex_refusal = AutocastPrecisionPolicy::new(
            DTypeBackendFamily::CurrentRuntimeBackends,
            ExtendedDType::BF16,
        )
        .resolve(AutocastOperationFamily::Pointwise, ExtendedDType::Complex64)
        .expect_err("complex autocast should refuse");
        assert_eq!(
            complex_refusal.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(complex_refusal.scope, PsionicRefusalScope::Runtime);

        let missing_rule = AutocastPrecisionPolicy::new(
            DTypeBackendFamily::CurrentRuntimeBackends,
            ExtendedDType::BF16,
        )
        .resolve(AutocastOperationFamily::Attention, ExtendedDType::F32)
        .expect_err("missing attention rule should refuse");
        assert_eq!(missing_rule.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(missing_rule.scope, PsionicRefusalScope::Graph);
    }

    #[test]
    fn quantization_capability_report_tracks_ptq_qat_runtime_and_export_cases() {
        let report = builtin_quantization_capability_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_quantization_v1");
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let ptq_config = QuantizationConfig::new(
            QuantizationMode::Int8Symmetric,
            ExtendedDType::F32,
            QuantizationGranularity::PerChannel,
            QuantizationCalibrationMode::MinMax,
            false,
        );
        assert!(
            report
                .validate_support(
                    QuantizationCapabilityStage::Ptq,
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    &ptq_config
                )
                .is_ok()
        );

        let runtime_config = QuantizationConfig::new(
            QuantizationMode::GgmlQ4_0,
            ExtendedDType::F32,
            QuantizationGranularity::BlockWise,
            QuantizationCalibrationMode::None,
            false,
        );
        assert!(
            report
                .validate_support(
                    QuantizationCapabilityStage::RuntimeExecution,
                    DTypeBackendFamily::CurrentRuntimeBackends,
                    &runtime_config
                )
                .is_ok()
        );

        let export_config = QuantizationConfig::new(
            QuantizationMode::Int8Symmetric,
            ExtendedDType::F32,
            QuantizationGranularity::PerChannel,
            QuantizationCalibrationMode::MinMax,
            false,
        );
        assert!(
            report
                .validate_support(
                    QuantizationCapabilityStage::ExportAware,
                    DTypeBackendFamily::MetaExecution,
                    &export_config
                )
                .is_ok()
        );

        let refused_qat = report
            .validate_support(
                QuantizationCapabilityStage::Qat,
                DTypeBackendFamily::CurrentRuntimeBackends,
                &QuantizationConfig::new(
                    QuantizationMode::GgmlQ4_0,
                    ExtendedDType::F32,
                    QuantizationGranularity::BlockWise,
                    QuantizationCalibrationMode::Histogram,
                    true,
                ),
            )
            .expect_err("block-quant qat should refuse");
        assert_eq!(
            refused_qat.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(refused_qat.scope, PsionicRefusalScope::Runtime);

        let missing_case = report
            .validate_support(
                QuantizationCapabilityStage::RuntimeExecution,
                DTypeBackendFamily::CurrentRuntimeBackends,
                &QuantizationConfig::new(
                    QuantizationMode::GgmlQ8_0,
                    ExtendedDType::F16,
                    QuantizationGranularity::BlockWise,
                    QuantizationCalibrationMode::None,
                    false,
                ),
            )
            .expect_err("missing q8 f16 runtime case should refuse");
        assert_eq!(missing_case.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(missing_case.scope, PsionicRefusalScope::Graph);
    }

    #[test]
    fn psionic_refusal_builder_keeps_code_scope_and_subject() {
        let refusal = PsionicRefusal::new(
            PsionicRefusalCode::UnsupportedOp,
            PsionicRefusalScope::Graph,
            "operator registry refused the op",
        )
        .with_subject("rope");

        assert_eq!(refusal.code, PsionicRefusalCode::UnsupportedOp);
        assert_eq!(refusal.scope, PsionicRefusalScope::Graph);
        assert_eq!(refusal.subject.as_deref(), Some("rope"));
        assert_eq!(refusal.detail, "operator registry refused the op");
    }
}
