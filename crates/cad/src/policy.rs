/// Canonical CAD unit label used by `crates/cad`.
pub const CANONICAL_UNIT: &str = "mm";

/// Baseline modeling tolerance in millimeters.
///
/// This corresponds to `1e-6 m` in the broader plan, represented in mm.
pub const BASE_TOLERANCE_MM: f64 = 1e-3;

/// Smallest accepted positive primitive dimension in millimeters.
pub const MIN_POSITIVE_DIMENSION_MM: f64 = BASE_TOLERANCE_MM;

/// Modeling policy mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelingPolicy {
    /// Tolerance-aware modeling defaults.
    Tolerant,
    /// Exact modeling mode (future extension point).
    Exact,
}

/// Wave 1 default modeling policy.
pub const DEFAULT_MODELING_POLICY: ModelingPolicy = ModelingPolicy::Tolerant;

/// Validate that a dimension is positive under current policy thresholds.
pub fn is_dimension_positive(dimension_mm: f64) -> bool {
    dimension_mm > MIN_POSITIVE_DIMENSION_MM
}

/// Resolve the effective tolerance for an operation.
pub fn resolve_tolerance_mm(override_tolerance_mm: Option<f64>) -> f64 {
    override_tolerance_mm.unwrap_or(BASE_TOLERANCE_MM)
}

#[cfg(test)]
mod tests {
    use super::{
        BASE_TOLERANCE_MM, CANONICAL_UNIT, DEFAULT_MODELING_POLICY, MIN_POSITIVE_DIMENSION_MM,
        ModelingPolicy, is_dimension_positive, resolve_tolerance_mm,
    };

    #[test]
    fn defaults_are_stable() {
        assert_eq!(CANONICAL_UNIT, "mm");
        assert_eq!(DEFAULT_MODELING_POLICY, ModelingPolicy::Tolerant);
        assert_eq!(BASE_TOLERANCE_MM, 1e-3);
        assert_eq!(MIN_POSITIVE_DIMENSION_MM, BASE_TOLERANCE_MM);
    }

    #[test]
    fn dimension_threshold_obeys_policy_constant() {
        assert!(!is_dimension_positive(BASE_TOLERANCE_MM));
        assert!(is_dimension_positive(BASE_TOLERANCE_MM + f64::EPSILON));
    }

    #[test]
    fn tolerance_resolution_prefers_override() {
        assert_eq!(resolve_tolerance_mm(None), BASE_TOLERANCE_MM);
        assert_eq!(resolve_tolerance_mm(Some(0.25)), 0.25);
    }
}
