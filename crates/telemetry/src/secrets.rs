//! Secret string wrapper that redacts sensitive data in logs.

use serde::{Serialize, Serializer};
use std::fmt;

/// A string wrapper that redacts its contents in Debug, Display, and Serialize.
///
/// Use this to wrap sensitive data like API keys, passwords, tokens, etc.
/// The actual value is only accessible via `expose_secret()`.
///
/// # Example
///
/// ```
/// use telemetry::SecretString;
///
/// let api_key = SecretString::new("sk-1234567890".to_string());
///
/// // Debug and Display show [REDACTED]
/// assert_eq!(format!("{:?}", api_key), "[REDACTED]");
/// assert_eq!(format!("{}", api_key), "[REDACTED]");
///
/// // Access the actual value when needed
/// assert_eq!(api_key.expose_secret(), "sk-1234567890");
/// ```
#[derive(Clone)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Create a new SecretString.
    pub fn new(secret: String) -> Self {
        Self { inner: secret }
    }

    /// Expose the secret value.
    ///
    /// Use this only when you actually need the secret (e.g., for API calls).
    /// Never log the result of this method.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Consume self and return the inner secret.
    pub fn into_inner(self) -> String {
        self.inner
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for SecretString {}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_redacts() {
        let secret = SecretString::new("my-secret-key".to_string());
        assert_eq!(format!("{:?}", secret), "[REDACTED]");
    }

    #[test]
    fn test_display_redacts() {
        let secret = SecretString::new("my-secret-key".to_string());
        assert_eq!(format!("{}", secret), "[REDACTED]");
    }

    #[test]
    fn test_serialize_redacts() {
        let secret = SecretString::new("my-secret-key".to_string());
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, "\"[REDACTED]\"");
    }

    #[test]
    fn test_expose_secret() {
        let secret = SecretString::new("my-secret-key".to_string());
        assert_eq!(secret.expose_secret(), "my-secret-key");
    }

    #[test]
    fn test_into_inner() {
        let secret = SecretString::new("my-secret-key".to_string());
        assert_eq!(secret.into_inner(), "my-secret-key");
    }

    #[test]
    fn test_equality() {
        let s1 = SecretString::new("secret".to_string());
        let s2 = SecretString::new("secret".to_string());
        let s3 = SecretString::new("different".to_string());

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_from_string() {
        let secret: SecretString = "my-secret".into();
        assert_eq!(secret.expose_secret(), "my-secret");

        let secret: SecretString = String::from("my-secret").into();
        assert_eq!(secret.expose_secret(), "my-secret");
    }
}
