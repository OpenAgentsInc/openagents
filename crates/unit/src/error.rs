//! Error types for the Unit runtime
//!
//! Provides structured error types for pin operations, connections,
//! and unit computations.

use std::fmt;

/// Top-level error type for Unit operations
#[derive(Debug, Clone)]
pub enum UnitError {
    /// Pin-related errors
    Pin(PinError),
    /// Connection-related errors
    Connection(ConnectionError),
    /// Computation errors during unit execution
    Computation { unit_id: String, message: String },
    /// Graph structure errors
    Graph(GraphError),
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pin(e) => write!(f, "Pin error: {}", e),
            Self::Connection(e) => write!(f, "Connection error: {}", e),
            Self::Computation { unit_id, message } => {
                write!(f, "Computation error in unit '{}': {}", unit_id, message)
            }
            Self::Graph(e) => write!(f, "Graph error: {}", e),
        }
    }
}

impl std::error::Error for UnitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pin(e) => Some(e),
            Self::Connection(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::Computation { .. } => None,
        }
    }
}

/// Pin-specific errors
#[derive(Debug, Clone)]
pub enum PinError {
    /// Pin with given name not found
    NotFound { unit_id: String, pin_name: String },
    /// Type mismatch when pushing data
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    /// Pin already exists with this name
    AlreadyExists { unit_id: String, pin_name: String },
    /// Operation not allowed on constant pin
    ConstantViolation { pin_name: String },
    /// Pin is in invalid state for operation
    InvalidState {
        pin_name: String,
        state: String,
        operation: &'static str,
    },
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { unit_id, pin_name } => {
                write!(f, "Pin '{}' not found in unit '{}'", pin_name, unit_id)
            }
            Self::TypeMismatch { expected, got } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, got)
            }
            Self::AlreadyExists { unit_id, pin_name } => {
                write!(f, "Pin '{}' already exists in unit '{}'", pin_name, unit_id)
            }
            Self::ConstantViolation { pin_name } => {
                write!(f, "Cannot modify constant pin '{}'", pin_name)
            }
            Self::InvalidState {
                pin_name,
                state,
                operation,
            } => {
                write!(
                    f,
                    "Cannot {} pin '{}' in state '{}'",
                    operation, pin_name, state
                )
            }
        }
    }
}

impl std::error::Error for PinError {}

/// Connection-specific errors
#[derive(Debug, Clone)]
pub enum ConnectionError {
    /// Source unit or pin not found
    SourceNotFound { unit_id: String, pin_name: String },
    /// Target unit or pin not found
    TargetNotFound { unit_id: String, pin_name: String },
    /// Connection would create a cycle
    CycleDetected { path: Vec<String> },
    /// Type mismatch between connected pins
    TypeMismatch {
        source_type: String,
        target_type: String,
    },
    /// Connection already exists
    AlreadyConnected { source: String, target: String },
    /// Invalid connection direction
    InvalidDirection { message: String },
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceNotFound { unit_id, pin_name } => {
                write!(f, "Source not found: {}/{}", unit_id, pin_name)
            }
            Self::TargetNotFound { unit_id, pin_name } => {
                write!(f, "Target not found: {}/{}", unit_id, pin_name)
            }
            Self::CycleDetected { path } => {
                write!(f, "Cycle detected: {}", path.join(" -> "))
            }
            Self::TypeMismatch {
                source_type,
                target_type,
            } => {
                write!(
                    f,
                    "Type mismatch: cannot connect {} to {}",
                    source_type, target_type
                )
            }
            Self::AlreadyConnected { source, target } => {
                write!(f, "Already connected: {} -> {}", source, target)
            }
            Self::InvalidDirection { message } => {
                write!(f, "Invalid connection direction: {}", message)
            }
        }
    }
}

impl std::error::Error for ConnectionError {}

/// Graph structure errors
#[derive(Debug, Clone)]
pub enum GraphError {
    /// Unit not found in graph
    UnitNotFound { unit_id: String },
    /// Unit already exists with this ID
    UnitAlreadyExists { unit_id: String },
    /// Merge not found
    MergeNotFound { merge_id: String },
    /// Merge already exists with this ID
    MergeAlreadyExists { merge_id: String },
    /// Invalid graph structure
    InvalidStructure { message: String },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnitNotFound { unit_id } => {
                write!(f, "Unit '{}' not found in graph", unit_id)
            }
            Self::UnitAlreadyExists { unit_id } => {
                write!(f, "Unit '{}' already exists in graph", unit_id)
            }
            Self::MergeNotFound { merge_id } => {
                write!(f, "Merge '{}' not found in graph", merge_id)
            }
            Self::MergeAlreadyExists { merge_id } => {
                write!(f, "Merge '{}' already exists in graph", merge_id)
            }
            Self::InvalidStructure { message } => {
                write!(f, "Invalid graph structure: {}", message)
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// Result type alias for Unit operations
pub type UnitResult<T> = Result<T, UnitError>;

// Convenient From implementations
impl From<PinError> for UnitError {
    fn from(e: PinError) -> Self {
        Self::Pin(e)
    }
}

impl From<ConnectionError> for UnitError {
    fn from(e: ConnectionError) -> Self {
        Self::Connection(e)
    }
}

impl From<GraphError> for UnitError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_pin_error_display() {
        let err = PinError::NotFound {
            unit_id: "add".to_string(),
            pin_name: "x".to_string(),
        };
        assert!(err.to_string().contains("add"));
        assert!(err.to_string().contains("x"));
    }

    #[test]
    fn test_connection_error_cycle() {
        let err = ConnectionError::CycleDetected {
            path: vec!["a".to_string(), "b".to_string(), "a".to_string()],
        };
        assert!(err.to_string().contains("a -> b -> a"));
    }

    #[test]
    fn test_unit_error_from_pin() {
        let pin_err = PinError::TypeMismatch {
            expected: "i32",
            got: "String",
        };
        let unit_err: UnitError = pin_err.into();
        assert!(matches!(unit_err, UnitError::Pin(_)));
    }

    #[test]
    fn test_unit_error_source() {
        let err = UnitError::Pin(PinError::NotFound {
            unit_id: "test".to_string(),
            pin_name: "input".to_string(),
        });
        assert!(err.source().is_some());
    }
}
