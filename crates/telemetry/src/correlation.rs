//! Correlation ID for distributed tracing.

use crate::error::{Result, TelemetryError};
use std::fmt;
use uuid::Uuid;

/// Maximum length for external correlation IDs.
const MAX_CORRELATION_ID_LENGTH: usize = 128;

/// A correlation ID for tracking requests across services.
///
/// Correlation IDs can be:
/// - Generated as UUIDs for new requests
/// - Accepted from external sources (e.g., HTTP headers)
///
/// # Example
///
/// ```
/// use telemetry::CorrelationId;
///
/// // Generate a new correlation ID
/// let id = CorrelationId::generate();
///
/// // Accept an external ID
/// let external_id = CorrelationId::from_external("req-123").unwrap();
///
/// // Use in tracing
/// tracing::info!(correlation_id = %id, "Processing request");
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID using UUID v4.
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a correlation ID from an external source.
    ///
    /// Validates that the ID is not empty and not too long.
    ///
    /// # Errors
    ///
    /// Returns `TelemetryError::InvalidCorrelationId` if:
    /// - The ID is empty
    /// - The ID exceeds 128 characters
    pub fn from_external(id: impl Into<String>) -> Result<Self> {
        let id = id.into();

        if id.is_empty() {
            return Err(TelemetryError::InvalidCorrelationId(
                "correlation ID cannot be empty".to_string(),
            ));
        }

        if id.len() > MAX_CORRELATION_ID_LENGTH {
            return Err(TelemetryError::InvalidCorrelationId(format!(
                "correlation ID exceeds maximum length of {} characters",
                MAX_CORRELATION_ID_LENGTH
            )));
        }

        Ok(Self(id))
    }

    /// Create a correlation ID from an external source, or generate one if invalid.
    ///
    /// This is useful when you want to accept an external ID but fall back to
    /// generating one if the external ID is invalid.
    pub fn from_external_or_generate(id: Option<impl Into<String>>) -> Self {
        match id {
            Some(id) => Self::from_external(id).unwrap_or_else(|_| Self::generate()),
            None => Self::generate(),
        }
    }

    /// Get the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CorrelationId").field(&self.0).finish()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for CorrelationId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Create a tracing span with a correlation ID.
///
/// This macro creates a span with the `correlation_id` field automatically set.
///
/// # Example
///
/// ```
/// use telemetry::{CorrelationId, correlation_span};
///
/// let id = CorrelationId::generate();
///
/// // Basic usage
/// let _span = correlation_span!("process_request", &id).entered();
///
/// // With additional fields
/// let _span = correlation_span!("process_request", &id, user_id = 123).entered();
/// ```
#[macro_export]
macro_rules! correlation_span {
    ($name:expr, $id:expr) => {
        tracing::info_span!($name, correlation_id = %$id)
    };
    ($name:expr, $id:expr, $($field:tt)*) => {
        tracing::info_span!($name, correlation_id = %$id, $($field)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unique() {
        let id1 = CorrelationId::generate();
        let id2 = CorrelationId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_from_external_valid() {
        let id = CorrelationId::from_external("req-123").unwrap();
        assert_eq!(id.as_str(), "req-123");
    }

    #[test]
    fn test_from_external_empty() {
        let result = CorrelationId::from_external("");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_external_too_long() {
        let long_id = "x".repeat(129);
        let result = CorrelationId::from_external(long_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_external_max_length() {
        let max_id = "x".repeat(128);
        let result = CorrelationId::from_external(max_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_from_external_or_generate_some() {
        let id = CorrelationId::from_external_or_generate(Some("req-123"));
        assert_eq!(id.as_str(), "req-123");
    }

    #[test]
    fn test_from_external_or_generate_none() {
        let id = CorrelationId::from_external_or_generate(None::<String>);
        // Should be a valid UUID
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_from_external_or_generate_invalid() {
        let id = CorrelationId::from_external_or_generate(Some(""));
        // Should fall back to generated ID
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_display() {
        let id = CorrelationId::from_external("req-123").unwrap();
        assert_eq!(format!("{}", id), "req-123");
    }

    #[test]
    fn test_into_inner() {
        let id = CorrelationId::from_external("req-123").unwrap();
        assert_eq!(id.into_inner(), "req-123");
    }
}
