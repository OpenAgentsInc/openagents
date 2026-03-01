use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for CAD domain operations.
pub type CadResult<T> = Result<T, CadError>;

/// Stable CAD error codes for UI/event mapping and telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadErrorCode {
    NotImplemented,
    ParseFailed,
    EvalFailed,
    QueryFailed,
    ExportFailed,
    InvalidPrimitive,
    InvalidPolicy,
    Serialization,
    InvalidFeatureGraph,
}

/// Minimal structured event payload emitted when CAD operations fail.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadErrorEvent {
    pub code: CadErrorCode,
    pub operation: String,
    pub message: String,
    pub remediation_hint: String,
    pub retryable: bool,
}

/// CAD domain error contract.
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum CadError {
    /// Placeholder error returned for unimplemented CAD operations.
    #[error("cad operation is not implemented")]
    NotImplemented,
    /// Request payload failed CAD domain parsing.
    #[error("parse failed: {reason}")]
    ParseFailed { reason: String },
    /// CAD model evaluation failed for the requested operation.
    #[error("evaluation failed: {reason}")]
    EvalFailed { reason: String },
    /// CAD query operation failed.
    #[error("query failed: {reason}")]
    QueryFailed { reason: String },
    /// CAD export operation failed.
    #[error("export failed ({format}): {reason}")]
    ExportFailed { format: String, reason: String },
    /// Primitive values failed CAD domain validation.
    #[error("invalid primitive: {reason}")]
    InvalidPrimitive { reason: String },
    /// Invalid CAD policy or tolerance configuration.
    #[error("invalid policy: {reason}")]
    InvalidPolicy { reason: String },
    /// Serialization or deserialization failed.
    #[error("serialization error: {reason}")]
    Serialization { reason: String },
    /// Feature graph configuration is invalid.
    #[error("invalid feature graph: {reason}")]
    InvalidFeatureGraph { reason: String },
}

impl CadError {
    /// Stable code for downstream UI/telemetry mapping.
    pub const fn code(&self) -> CadErrorCode {
        match self {
            Self::NotImplemented => CadErrorCode::NotImplemented,
            Self::ParseFailed { .. } => CadErrorCode::ParseFailed,
            Self::EvalFailed { .. } => CadErrorCode::EvalFailed,
            Self::QueryFailed { .. } => CadErrorCode::QueryFailed,
            Self::ExportFailed { .. } => CadErrorCode::ExportFailed,
            Self::InvalidPrimitive { .. } => CadErrorCode::InvalidPrimitive,
            Self::InvalidPolicy { .. } => CadErrorCode::InvalidPolicy,
            Self::Serialization { .. } => CadErrorCode::Serialization,
            Self::InvalidFeatureGraph { .. } => CadErrorCode::InvalidFeatureGraph,
        }
    }

    /// Human-actionable hint intended for UI error panes and logs.
    pub const fn remediation_hint(&self) -> &'static str {
        match self {
            Self::NotImplemented => "This operation is planned but not implemented yet.",
            Self::ParseFailed { .. } => "Check payload schema and required fields.",
            Self::EvalFailed { .. } => {
                "Inspect feature graph dependencies and parameter values, then retry."
            }
            Self::QueryFailed { .. } => "Verify the selected entity exists in the current model.",
            Self::ExportFailed { .. } => "Retry export or change export format settings.",
            Self::InvalidPrimitive { .. } => "Use positive dimensions above CAD tolerance.",
            Self::InvalidPolicy { .. } => "Provide a positive modeling tolerance.",
            Self::Serialization { .. } => "Inspect document format/version compatibility.",
            Self::InvalidFeatureGraph { .. } => {
                "Fix graph cycles, duplicate IDs, or missing dependencies."
            }
        }
    }

    /// Whether callers can safely retry without changing inputs.
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NotImplemented | Self::EvalFailed { .. } | Self::QueryFailed { .. }
        )
    }

    /// Convert to structured event payload for UI and activity feeds.
    pub fn to_event(&self, operation: impl Into<String>) -> CadErrorEvent {
        CadErrorEvent {
            code: self.code(),
            operation: operation.into(),
            message: self.to_string(),
            remediation_hint: self.remediation_hint().to_string(),
            retryable: self.is_retryable(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CadError, CadErrorCode, CadResult};

    #[test]
    fn stable_error_code_mapping() {
        let code = CadError::ParseFailed {
            reason: "bad json".to_string(),
        }
        .code();
        assert_eq!(code, CadErrorCode::ParseFailed);
    }

    #[test]
    fn stable_event_mapping() {
        let event = CadError::ExportFailed {
            format: "step".to_string(),
            reason: "writer unavailable".to_string(),
        }
        .to_event("export.step");
        assert_eq!(event.code, CadErrorCode::ExportFailed);
        assert_eq!(event.operation, "export.step");
        assert!(event.message.contains("export failed"));
        assert!(!event.retryable);
    }

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

    #[test]
    fn invalid_policy_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::InvalidPolicy {
            reason: "tolerance must be positive".to_string(),
        });
        assert_eq!(
            result,
            Err(CadError::InvalidPolicy {
                reason: "tolerance must be positive".to_string(),
            })
        );
    }

    #[test]
    fn serialization_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::Serialization {
            reason: "failed to encode json".to_string(),
        });
        assert_eq!(
            result,
            Err(CadError::Serialization {
                reason: "failed to encode json".to_string(),
            })
        );
    }

    #[test]
    fn invalid_feature_graph_error_contract_is_stable() {
        let result: CadResult<()> = Err(CadError::InvalidFeatureGraph {
            reason: "cycle detected".to_string(),
        });
        assert_eq!(
            result,
            Err(CadError::InvalidFeatureGraph {
                reason: "cycle detected".to_string(),
            })
        );
    }
}
