use crate::kernel::CadKernelAdapter;
use crate::policy;
use crate::{CadError, CadResult};

/// Minimal boolean operation kinds for adapter-boundary stabilization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooleanOp {
    Difference,
    Union,
    Intersection,
}

/// Resolve and validate tolerance used by boolean operations.
pub fn boolean_tolerance_mm(override_tolerance_mm: Option<f64>) -> CadResult<f64> {
    let tolerance_mm = policy::resolve_tolerance_mm(override_tolerance_mm);
    if tolerance_mm <= 0.0 {
        return Err(CadError::InvalidPolicy {
            reason: "boolean tolerance must be positive".to_string(),
        });
    }
    Ok(tolerance_mm)
}

/// Placeholder boolean adapter entrypoint.
///
/// This method currently validates policy-level tolerance rules and explicitly
/// returns `NotImplemented` until kernel boolean wiring lands.
pub fn boolean_op<K: CadKernelAdapter>(
    _kernel: &mut K,
    _operation: BooleanOp,
    _left: &K::Solid,
    _right: &K::Solid,
    override_tolerance_mm: Option<f64>,
) -> CadResult<K::Solid> {
    let _effective_tolerance_mm = boolean_tolerance_mm(override_tolerance_mm)?;
    Err(CadError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::boolean_tolerance_mm;
    use crate::policy::BASE_TOLERANCE_MM;

    #[test]
    fn boolean_path_uses_policy_default_tolerance() {
        let resolved = boolean_tolerance_mm(None);
        assert_eq!(resolved, Ok(BASE_TOLERANCE_MM));
    }

    #[test]
    fn boolean_path_rejects_non_positive_tolerance() {
        let resolved = boolean_tolerance_mm(Some(0.0));
        assert!(resolved.is_err(), "non-positive tolerance must error");
    }
}
