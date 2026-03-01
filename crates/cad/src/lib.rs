//! Reusable CAD domain foundation for OpenAgents.
//!
//! This crate intentionally starts with a minimal, product-agnostic API surface.
//! Higher-level CAD workflows are introduced incrementally via the CAD backlog.

use thiserror::Error;

pub mod eval;
pub mod kernel;
pub mod primitives;

/// Result type for CAD domain operations.
pub type CadResult<T> = Result<T, CadError>;

/// Minimal CAD domain error contract.
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum CadError {
    /// Placeholder error returned for unimplemented CAD operations.
    #[error("cad operation is not implemented")]
    NotImplemented,
    /// Primitive values failed CAD domain validation.
    #[error("invalid primitive: {reason}")]
    InvalidPrimitive { reason: String },
}

#[cfg(test)]
mod tests {
    use super::{CadError, CadResult};

    #[test]
    fn placeholder_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::NotImplemented);
        assert_eq!(result, Err(CadError::NotImplemented));
    }

    #[test]
    fn invalid_primitive_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::InvalidPrimitive {
            reason: "width must be positive".to_string(),
        });
        assert_eq!(
            result,
            Err(CadError::InvalidPrimitive {
                reason: "width must be positive".to_string(),
            })
        );
    }
}
