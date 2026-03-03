/// Canonical CAD unit label used by `crates/cad`.
pub const CANONICAL_UNIT: &str = "mm";

/// Baseline linear modeling tolerance in millimeters.
///
/// Aligned to vcad kernel defaults (`1e-6 mm`).
pub const BASE_LINEAR_TOLERANCE_MM: f64 = 1e-6;

/// Backward-compatible alias used by earlier parity lanes.
pub const BASE_TOLERANCE_MM: f64 = BASE_LINEAR_TOLERANCE_MM;

/// Baseline angular modeling tolerance in radians.
///
/// Aligned to vcad kernel defaults (`1e-9 rad`).
pub const BASE_ANGULAR_TOLERANCE_RAD: f64 = 1e-9;

/// Smallest accepted positive primitive dimension in millimeters.
pub const MIN_POSITIVE_DIMENSION_MM: f64 = BASE_LINEAR_TOLERANCE_MM;

/// Modeling policy mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelingPolicy {
    /// Tolerance-aware modeling defaults.
    Tolerant,
    /// Exact modeling mode (future extension point).
    Exact,
}

/// Predicate strategy for geometric classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateStrategy {
    /// Adaptive-precision exact predicates (vcad parity default).
    AdaptiveExact,
    /// Legacy floating-point tolerance comparisons.
    FloatingTolerance,
}

/// Wave 1 default modeling policy.
pub const DEFAULT_MODELING_POLICY: ModelingPolicy = ModelingPolicy::Tolerant;

/// Default predicate strategy aligned to vcad behavior.
pub const DEFAULT_PREDICATE_STRATEGY: PredicateStrategy = PredicateStrategy::AdaptiveExact;

/// Effective precision policy snapshot used by kernel lanes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrecisionPolicy {
    pub modeling_policy: ModelingPolicy,
    pub predicate_strategy: PredicateStrategy,
    pub linear_tolerance_mm: f64,
    pub angular_tolerance_rad: f64,
}

/// Default precision policy for parity lanes.
pub const DEFAULT_PRECISION_POLICY: PrecisionPolicy = PrecisionPolicy {
    modeling_policy: DEFAULT_MODELING_POLICY,
    predicate_strategy: DEFAULT_PREDICATE_STRATEGY,
    linear_tolerance_mm: BASE_LINEAR_TOLERANCE_MM,
    angular_tolerance_rad: BASE_ANGULAR_TOLERANCE_RAD,
};

/// Validate that a dimension is positive under current policy thresholds.
pub fn is_dimension_positive(dimension_mm: f64) -> bool {
    dimension_mm > MIN_POSITIVE_DIMENSION_MM
}

/// Resolve the effective linear tolerance for an operation.
pub fn resolve_tolerance_mm(override_tolerance_mm: Option<f64>) -> f64 {
    override_tolerance_mm.unwrap_or(BASE_LINEAR_TOLERANCE_MM)
}

/// Resolve the effective angular tolerance for an operation.
pub fn resolve_angular_tolerance_rad(override_tolerance_rad: Option<f64>) -> f64 {
    override_tolerance_rad.unwrap_or(BASE_ANGULAR_TOLERANCE_RAD)
}

/// Resolve predicate strategy, defaulting to adaptive exact predicates.
pub fn resolve_predicate_strategy(
    override_strategy: Option<PredicateStrategy>,
) -> PredicateStrategy {
    override_strategy.unwrap_or(DEFAULT_PREDICATE_STRATEGY)
}

/// Check if a linear delta is effectively zero under policy tolerance.
pub fn is_within_linear_tolerance(delta_mm: f64, tolerance_mm: Option<f64>) -> bool {
    delta_mm.abs() <= resolve_tolerance_mm(tolerance_mm)
}

/// Check if an angular delta is effectively zero under policy tolerance.
pub fn is_within_angular_tolerance(delta_rad: f64, tolerance_rad: Option<f64>) -> bool {
    delta_rad.abs() <= resolve_angular_tolerance_rad(tolerance_rad)
}

#[cfg(test)]
mod tests {
    use super::{
        BASE_ANGULAR_TOLERANCE_RAD, BASE_LINEAR_TOLERANCE_MM, BASE_TOLERANCE_MM, CANONICAL_UNIT,
        DEFAULT_MODELING_POLICY, DEFAULT_PRECISION_POLICY, DEFAULT_PREDICATE_STRATEGY,
        MIN_POSITIVE_DIMENSION_MM, ModelingPolicy, PredicateStrategy, is_dimension_positive,
        is_within_angular_tolerance, is_within_linear_tolerance, resolve_angular_tolerance_rad,
        resolve_predicate_strategy, resolve_tolerance_mm,
    };

    #[test]
    fn defaults_are_stable() {
        assert_eq!(CANONICAL_UNIT, "mm");
        assert_eq!(DEFAULT_MODELING_POLICY, ModelingPolicy::Tolerant);
        assert_eq!(DEFAULT_PREDICATE_STRATEGY, PredicateStrategy::AdaptiveExact);
        assert_eq!(BASE_LINEAR_TOLERANCE_MM, 1e-6);
        assert_eq!(BASE_TOLERANCE_MM, BASE_LINEAR_TOLERANCE_MM);
        assert_eq!(BASE_ANGULAR_TOLERANCE_RAD, 1e-9);
        assert_eq!(MIN_POSITIVE_DIMENSION_MM, BASE_LINEAR_TOLERANCE_MM);
        assert_eq!(DEFAULT_PRECISION_POLICY.linear_tolerance_mm, 1e-6);
        assert_eq!(DEFAULT_PRECISION_POLICY.angular_tolerance_rad, 1e-9);
    }

    #[test]
    fn dimension_threshold_obeys_policy_constant() {
        assert!(!is_dimension_positive(BASE_LINEAR_TOLERANCE_MM));
        assert!(is_dimension_positive(
            BASE_LINEAR_TOLERANCE_MM + f64::EPSILON
        ));
    }

    #[test]
    fn tolerance_resolution_prefers_override() {
        assert_eq!(resolve_tolerance_mm(None), BASE_LINEAR_TOLERANCE_MM);
        assert_eq!(resolve_tolerance_mm(Some(0.25)), 0.25);
        assert_eq!(
            resolve_angular_tolerance_rad(None),
            BASE_ANGULAR_TOLERANCE_RAD
        );
        assert_eq!(resolve_angular_tolerance_rad(Some(0.02)), 0.02);
    }

    #[test]
    fn predicate_strategy_resolution_prefers_override() {
        assert_eq!(
            resolve_predicate_strategy(None),
            PredicateStrategy::AdaptiveExact
        );
        assert_eq!(
            resolve_predicate_strategy(Some(PredicateStrategy::FloatingTolerance)),
            PredicateStrategy::FloatingTolerance
        );
    }

    #[test]
    fn tolerance_helpers_apply_resolved_thresholds() {
        assert!(is_within_linear_tolerance(1e-7, None));
        assert!(!is_within_linear_tolerance(1e-4, None));
        assert!(is_within_linear_tolerance(0.01, Some(0.1)));

        assert!(is_within_angular_tolerance(1e-10, None));
        assert!(!is_within_angular_tolerance(1e-6, None));
        assert!(is_within_angular_tolerance(0.01, Some(0.1)));
    }
}
