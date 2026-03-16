use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::TrainingPrecisionMode;

/// Gradient-scaling mode used by the bounded train-class mixed-precision surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientScalingMode {
    /// No loss scaling is active.
    Disabled,
    /// A fixed loss scale is applied on every step.
    Static,
    /// Loss scale adapts to overflow or underflow signals.
    Dynamic,
}

/// Runtime-visible signal produced by one scaled training step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientScalingSignal {
    /// Gradients remained finite and did not exhibit underflow symptoms.
    Clean,
    /// Gradients overflowed or became non-finite and the step must be skipped.
    Overflow,
    /// Gradients remained finite but collapsed toward zero under the current scale.
    Underflow,
}

/// Current status for one bounded gradient-scaling decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientScalingStatus {
    /// The step may apply gradients at the current scale.
    Applied,
    /// The scale grew to counteract underflow.
    GrewScale,
    /// The scale backed off and the optimizer step is skipped.
    BackedOffAndSkipped,
    /// The current precision path intentionally does not require scaling.
    NoScalingRequired,
    /// The request is intentionally refused in the current scope.
    Refused,
}

/// Diagnostic emitted by one bounded gradient-scaling decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientScalingDiagnostic {
    /// One overflow or non-finite gradient signal was observed.
    OverflowDetected,
    /// One underflow or near-zero gradient signal was observed.
    UnderflowDetected,
    /// The optimizer step must be skipped for numerical safety.
    StepSkipped,
    /// The loss scale was increased.
    ScaleIncreased,
    /// The loss scale was reduced.
    ScaleReduced,
    /// BF16 remains a no-scaling path in the bounded current scope.
    Bf16NoScalingRequired,
}

/// Reusable train-owned gradient-scaling policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingGradientScalingPolicy {
    /// Precision used by gradient buffers.
    pub gradient_precision: TrainingPrecisionMode,
    /// Precision used by master weights when one exists.
    pub master_weight_precision: TrainingPrecisionMode,
    /// Scaling mode for the policy.
    pub scaling_mode: GradientScalingMode,
    /// Minimum legal loss scale.
    pub min_loss_scale: u64,
    /// Maximum legal loss scale.
    pub max_loss_scale: u64,
}

impl TrainingGradientScalingPolicy {
    /// Returns the bounded current-scope dynamic FP16 policy.
    #[must_use]
    pub const fn dynamic_fp16() -> Self {
        Self {
            gradient_precision: TrainingPrecisionMode::Fp16,
            master_weight_precision: TrainingPrecisionMode::Fp32,
            scaling_mode: GradientScalingMode::Dynamic,
            min_loss_scale: 1,
            max_loss_scale: 65_536,
        }
    }

    /// Returns the bounded BF16 no-scaling posture.
    #[must_use]
    pub const fn bf16_passthrough() -> Self {
        Self {
            gradient_precision: TrainingPrecisionMode::Bf16,
            master_weight_precision: TrainingPrecisionMode::Fp32,
            scaling_mode: GradientScalingMode::Disabled,
            min_loss_scale: 1,
            max_loss_scale: 1,
        }
    }

    /// Evaluates one step signal against the bounded current-scope policy.
    pub fn evaluate(
        &self,
        signal: GradientScalingSignal,
        current_loss_scale: u64,
    ) -> Result<GradientScalingDecision, PsionicRefusal> {
        if current_loss_scale == 0 {
            return Err(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Runtime,
                "loss scale must be greater than zero",
            )
            .with_subject("loss_scale=0"));
        }

        match self.gradient_precision {
            TrainingPrecisionMode::Bf16 => {
                if self.scaling_mode != GradientScalingMode::Disabled {
                    return Err(PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        "bounded bf16 train path keeps gradient scaling disabled",
                    )
                    .with_subject("bf16_gradient_scaling"));
                }
                Ok(GradientScalingDecision {
                    status: GradientScalingStatus::NoScalingRequired,
                    next_loss_scale: current_loss_scale.max(1),
                    diagnostics: vec![GradientScalingDiagnostic::Bf16NoScalingRequired],
                    bounded_scope: String::from(
                        "Current runtime backends keep the bounded bf16 train path on an explicit no-scaling posture rather than pretending dynamic scaling is required everywhere.",
                    ),
                })
            }
            TrainingPrecisionMode::Fp16 => {
                if self.master_weight_precision != TrainingPrecisionMode::Fp32 {
                    return Err(PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        "bounded fp16 gradient scaling requires fp32 master weights",
                    )
                    .with_subject("fp16_without_fp32_master_weights"));
                }
                if self.scaling_mode != GradientScalingMode::Dynamic {
                    return Err(PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        "bounded fp16 train path only supports dynamic gradient scaling",
                    )
                    .with_subject("fp16_without_dynamic_scaling"));
                }
                let current_loss_scale = current_loss_scale.clamp(
                    self.min_loss_scale.max(1),
                    self.max_loss_scale.max(self.min_loss_scale.max(1)),
                );
                let (status, next_loss_scale, diagnostics, bounded_scope) = match signal {
                    GradientScalingSignal::Clean => (
                        GradientScalingStatus::Applied,
                        current_loss_scale,
                        Vec::new(),
                        "Current runtime backends keep the loss scale unchanged after one finite fp16 step in the bounded dynamic-scaling window.",
                    ),
                    GradientScalingSignal::Overflow => (
                        GradientScalingStatus::BackedOffAndSkipped,
                        (current_loss_scale / 2).max(self.min_loss_scale.max(1)),
                        vec![
                            GradientScalingDiagnostic::OverflowDetected,
                            GradientScalingDiagnostic::StepSkipped,
                            GradientScalingDiagnostic::ScaleReduced,
                        ],
                        "Current runtime backends back off the loss scale and skip the optimizer step when fp16 gradients overflow in the bounded dynamic-scaling window.",
                    ),
                    GradientScalingSignal::Underflow => (
                        GradientScalingStatus::GrewScale,
                        current_loss_scale
                            .saturating_mul(2)
                            .min(self.max_loss_scale.max(current_loss_scale)),
                        vec![
                            GradientScalingDiagnostic::UnderflowDetected,
                            GradientScalingDiagnostic::ScaleIncreased,
                        ],
                        "Current runtime backends grow the loss scale when bounded fp16 training detects underflow pressure instead of silently accepting vanishing gradients.",
                    ),
                };
                Ok(GradientScalingDecision {
                    status,
                    next_loss_scale,
                    diagnostics,
                    bounded_scope: String::from(bounded_scope),
                })
            }
            other => Err(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "bounded gradient scaling does not yet support gradient precision `{other:?}`"
                ),
            )
            .with_subject(format!("{other:?}").to_ascii_lowercase())),
        }
    }
}

/// Decision returned by one bounded gradient-scaling evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradientScalingDecision {
    /// Current status for the step.
    pub status: GradientScalingStatus,
    /// Loss scale to carry into the next step.
    pub next_loss_scale: u64,
    /// Diagnostics attached to the step.
    pub diagnostics: Vec<GradientScalingDiagnostic>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
}

/// One machine-readable seeded gradient-scaling case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradientScalingCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Policy under test.
    pub policy: TrainingGradientScalingPolicy,
    /// Runtime-visible signal under test.
    pub signal: GradientScalingSignal,
    /// Loss scale entering the step.
    pub current_loss_scale: u64,
    /// Current status for the case.
    pub status: GradientScalingStatus,
    /// Loss scale selected for the next step when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_loss_scale: Option<u64>,
    /// Diagnostics emitted by the case.
    pub diagnostics: Vec<GradientScalingDiagnostic>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the policy is intentionally unsupported today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable bounded report for train-class gradient scaling semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradientScalingSemanticsReport {
    /// Stable schema version for the report.
    pub schema_version: u32,
    /// Versioned current-scope window.
    pub current_scope_window: String,
    /// Seeded cases that describe the current scope boundary.
    pub cases: Vec<GradientScalingCaseResult>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl GradientScalingSemanticsReport {
    fn new(current_scope_window: impl Into<String>, cases: Vec<GradientScalingCaseResult>) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest =
            stable_gradient_scaling_semantics_digest(current_scope_window.as_str(), &cases);
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
                "{}|{:?}|{:?}|{:?}",
                case.case_id, case.policy.gradient_precision, case.signal, case.status
            ));
        }
        lines
    }
}

/// Builds the canonical bounded gradient-scaling semantics report.
#[must_use]
pub fn builtin_gradient_scaling_semantics_report() -> GradientScalingSemanticsReport {
    let fp16_dynamic = TrainingGradientScalingPolicy::dynamic_fp16();
    let bf16_passthrough = TrainingGradientScalingPolicy::bf16_passthrough();
    let fp16_missing_master = TrainingGradientScalingPolicy {
        gradient_precision: TrainingPrecisionMode::Fp16,
        master_weight_precision: TrainingPrecisionMode::Fp16,
        scaling_mode: GradientScalingMode::Dynamic,
        min_loss_scale: 1,
        max_loss_scale: 65_536,
    };

    GradientScalingSemanticsReport::new(
        String::from("psionic_gradient_scaling_v1"),
        vec![
            supported_gradient_scaling_case(
                "current_runtime_backends.fp16.dynamic.clean_4096",
                fp16_dynamic.clone(),
                GradientScalingSignal::Clean,
                4_096,
            ),
            supported_gradient_scaling_case(
                "current_runtime_backends.fp16.dynamic.overflow_4096",
                fp16_dynamic.clone(),
                GradientScalingSignal::Overflow,
                4_096,
            ),
            supported_gradient_scaling_case(
                "current_runtime_backends.fp16.dynamic.underflow_4096",
                fp16_dynamic,
                GradientScalingSignal::Underflow,
                4_096,
            ),
            supported_gradient_scaling_case(
                "current_runtime_backends.bf16.disabled.clean_1",
                bf16_passthrough,
                GradientScalingSignal::Clean,
                1,
            ),
            refused_gradient_scaling_case(
                "current_runtime_backends.fp16.dynamic.fp16_master_weights",
                fp16_missing_master,
                GradientScalingSignal::Clean,
                4_096,
            ),
        ],
    )
}

fn supported_gradient_scaling_case(
    case_id: &str,
    policy: TrainingGradientScalingPolicy,
    signal: GradientScalingSignal,
    current_loss_scale: u64,
) -> GradientScalingCaseResult {
    let decision = policy
        .evaluate(signal, current_loss_scale)
        .expect("seeded gradient-scaling case should be supported");
    GradientScalingCaseResult {
        case_id: String::from(case_id),
        policy,
        signal,
        current_loss_scale,
        status: decision.status,
        next_loss_scale: Some(decision.next_loss_scale),
        diagnostics: decision.diagnostics,
        bounded_scope: decision.bounded_scope,
        refusal: None,
    }
}

fn refused_gradient_scaling_case(
    case_id: &str,
    policy: TrainingGradientScalingPolicy,
    signal: GradientScalingSignal,
    current_loss_scale: u64,
) -> GradientScalingCaseResult {
    let refusal = policy
        .evaluate(signal, current_loss_scale)
        .expect_err("seeded gradient-scaling case should refuse");
    GradientScalingCaseResult {
        case_id: String::from(case_id),
        policy,
        signal,
        current_loss_scale,
        status: GradientScalingStatus::Refused,
        next_loss_scale: None,
        diagnostics: Vec::new(),
        bounded_scope: String::from(
            "Bounded current scope only supports dynamic fp16 gradient scaling with fp32 master weights plus bf16 no-scaling posture.",
        ),
        refusal: Some(refusal),
    }
}

fn stable_gradient_scaling_semantics_digest(
    current_scope_window: &str,
    cases: &[GradientScalingCaseResult],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in cases {
        lines.push(format!("case_id={}", case.case_id));
        lines.push(format!(
            "gradient_precision={:?}",
            case.policy.gradient_precision
        ));
        lines.push(format!(
            "master_weight_precision={:?}",
            case.policy.master_weight_precision
        ));
        lines.push(format!("scaling_mode={:?}", case.policy.scaling_mode));
        lines.push(format!("signal={:?}", case.signal));
        lines.push(format!("current_loss_scale={}", case.current_loss_scale));
        lines.push(format!("status={:?}", case.status));
        lines.push(format!("bounded_scope={}", case.bounded_scope));
        if let Some(next_loss_scale) = case.next_loss_scale {
            lines.push(format!("next_loss_scale={next_loss_scale}"));
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

#[cfg(test)]
mod tests {
    use psionic_core::{PsionicRefusalCode, PsionicRefusalScope};

    use super::{
        builtin_gradient_scaling_semantics_report, GradientScalingDiagnostic, GradientScalingMode,
        GradientScalingSignal, GradientScalingStatus, TrainingGradientScalingPolicy,
    };
    use crate::TrainingPrecisionMode;

    #[test]
    fn gradient_scaling_semantics_report_tracks_loss_scale_overflow_and_underflow_cases() {
        let report = builtin_gradient_scaling_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_gradient_scaling_v1");
        assert!(report
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("report_digest=")));

        let fp16_dynamic = TrainingGradientScalingPolicy::dynamic_fp16();
        let clean = fp16_dynamic
            .evaluate(GradientScalingSignal::Clean, 4_096)
            .expect("seeded clean fp16 case should resolve");
        assert_eq!(clean.status, GradientScalingStatus::Applied);
        assert_eq!(clean.next_loss_scale, 4_096);

        let overflow = TrainingGradientScalingPolicy::dynamic_fp16()
            .evaluate(GradientScalingSignal::Overflow, 4_096)
            .expect("seeded overflow fp16 case should resolve");
        assert_eq!(overflow.status, GradientScalingStatus::BackedOffAndSkipped);
        assert_eq!(overflow.next_loss_scale, 2_048);
        assert!(overflow
            .diagnostics
            .contains(&GradientScalingDiagnostic::OverflowDetected));
        assert!(overflow
            .diagnostics
            .contains(&GradientScalingDiagnostic::StepSkipped));

        let underflow = TrainingGradientScalingPolicy::dynamic_fp16()
            .evaluate(GradientScalingSignal::Underflow, 4_096)
            .expect("seeded underflow fp16 case should resolve");
        assert_eq!(underflow.status, GradientScalingStatus::GrewScale);
        assert_eq!(underflow.next_loss_scale, 8_192);
        assert!(underflow
            .diagnostics
            .contains(&GradientScalingDiagnostic::UnderflowDetected));
        assert!(underflow
            .diagnostics
            .contains(&GradientScalingDiagnostic::ScaleIncreased));

        let bf16_passthrough = TrainingGradientScalingPolicy::bf16_passthrough()
            .evaluate(GradientScalingSignal::Clean, 1)
            .expect("seeded bf16 passthrough case should resolve");
        assert_eq!(
            bf16_passthrough.status,
            GradientScalingStatus::NoScalingRequired
        );
        assert!(bf16_passthrough
            .diagnostics
            .contains(&GradientScalingDiagnostic::Bf16NoScalingRequired));

        let refused_master = TrainingGradientScalingPolicy {
            gradient_precision: TrainingPrecisionMode::Fp16,
            master_weight_precision: TrainingPrecisionMode::Fp16,
            scaling_mode: GradientScalingMode::Dynamic,
            min_loss_scale: 1,
            max_loss_scale: 65_536,
        }
        .evaluate(GradientScalingSignal::Clean, 4_096)
        .expect_err("fp16 without fp32 master weights should refuse");
        assert_eq!(
            refused_master.code,
            PsionicRefusalCode::UnsupportedBackendCapability
        );
        assert_eq!(refused_master.scope, PsionicRefusalScope::Runtime);
        assert_eq!(
            refused_master.subject.as_deref(),
            Some("fp16_without_fp32_master_weights")
        );
    }
}
