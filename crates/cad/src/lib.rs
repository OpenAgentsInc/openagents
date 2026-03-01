//! Reusable CAD domain foundation for OpenAgents.
//!
//! This crate intentionally starts with a minimal, product-agnostic API surface.
//! Higher-level CAD workflows are introduced incrementally via the CAD backlog.

use thiserror::Error;

/// Result type for CAD domain operations.
pub type CadResult<T> = Result<T, CadError>;

/// Minimal CAD domain error contract.
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum CadError {
    /// Placeholder error returned for unimplemented CAD operations.
    #[error("cad operation is not implemented")]
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::{CadError, CadResult};

    #[test]
    fn placeholder_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::NotImplemented);
        assert_eq!(result, Err(CadError::NotImplemented));
    }
}
