//! Canonical JSON serialization and deterministic hashing.
//!
//! Ensures identical inputs produce identical hashes across all implementations.
//! This follows RFC 8785 (JCS - JSON Canonicalization Scheme) principles.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during hashing operations.
#[derive(Debug, Error)]
pub enum HashError {
    /// Failed to serialize value to JSON.
    #[error("serialization failed: {0}")]
    Serialization(String),
}

/// Serialize a value to canonical JSON (sorted keys, no whitespace).
///
/// This follows RFC 8785 (JCS) principles:
/// - Object keys sorted lexicographically
/// - No whitespace between tokens
/// - Numbers in shortest form
///
/// # Example
///
/// ```
/// use protocol::hash::canonical_json;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Example {
///     zebra: String,
///     apple: i32,
/// }
///
/// let value = Example { zebra: "z".into(), apple: 1 };
/// let json = canonical_json(&value).unwrap();
/// // Keys are sorted: {"apple":1,"zebra":"z"}
/// assert!(json.contains("\"apple\":1"));
/// ```
pub fn canonical_json<T: Serialize + ?Sized>(value: &T) -> Result<String, HashError> {
    let json_value =
        serde_json::to_value(value).map_err(|e| HashError::Serialization(e.to_string()))?;

    // Recursively sort all object keys
    let canonical = canonicalize_value(json_value);

    // Serialize without whitespace
    serde_json::to_string(&canonical).map_err(|e| HashError::Serialization(e.to_string()))
}

/// Compute SHA-256 hash of canonical JSON representation.
///
/// Returns a lowercase hex-encoded 64-character string.
///
/// # Example
///
/// ```
/// use protocol::hash::canonical_hash;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { value: i32 }
///
/// let hash = canonical_hash(&Data { value: 42 }).unwrap();
/// assert_eq!(hash.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars
/// ```
pub fn canonical_hash<T: Serialize + ?Sized>(value: &T) -> Result<String, HashError> {
    let json = canonical_json(value)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Recursively canonicalize a JSON value by sorting object keys.
fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            // Create a new map with sorted keys
            let mut sorted: Vec<(String, Value)> = map
                .into_iter()
                .map(|(k, v)| (k, canonicalize_value(v)))
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}

/// Trait for types that can compute their canonical hash.
pub trait Hashable: Serialize {
    /// Compute the canonical SHA-256 hash of this value.
    fn compute_hash(&self) -> Result<String, HashError> {
        canonical_hash(self)
    }
}

// Blanket implementation for all serializable types
impl<T: Serialize> Hashable for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestStruct {
        zebra: String,
        apple: i32,
        mango: bool,
    }

    #[test]
    fn test_canonical_json_sorts_keys() {
        let value = TestStruct {
            zebra: "z".into(),
            apple: 1,
            mango: true,
        };
        let json = canonical_json(&value).unwrap();
        // Keys should be sorted alphabetically
        assert_eq!(json, r#"{"apple":1,"mango":true,"zebra":"z"}"#);
    }

    #[test]
    fn test_canonical_json_no_whitespace() {
        let value = TestStruct {
            zebra: "z".into(),
            apple: 1,
            mango: true,
        };
        let json = canonical_json(&value).unwrap();
        assert!(!json.contains(' '));
        assert!(!json.contains('\n'));
    }

    #[test]
    fn test_hash_determinism() {
        let value = TestStruct {
            zebra: "z".into(),
            apple: 1,
            mango: true,
        };
        let hash1 = canonical_hash(&value).unwrap();
        let hash2 = canonical_hash(&value).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_nested_object_sorting() {
        #[derive(Serialize)]
        struct Nested {
            outer_z: Inner,
            outer_a: i32,
        }
        #[derive(Serialize)]
        struct Inner {
            inner_z: String,
            inner_a: i32,
        }

        let value = Nested {
            outer_z: Inner {
                inner_z: "z".into(),
                inner_a: 1,
            },
            outer_a: 2,
        };
        let json = canonical_json(&value).unwrap();
        // Both outer and inner keys should be sorted
        assert!(json.starts_with(r#"{"outer_a":2,"outer_z":{"inner_a":1,"inner_z":"z"}}"#));
    }

    #[test]
    fn test_array_ordering_preserved() {
        #[derive(Serialize)]
        struct WithArray {
            items: Vec<i32>,
        }

        let value = WithArray {
            items: vec![3, 1, 2],
        };
        let json = canonical_json(&value).unwrap();
        // Array order should be preserved
        assert_eq!(json, r#"{"items":[3,1,2]}"#);
    }

    #[test]
    fn test_hashable_trait() {
        let value = TestStruct {
            zebra: "z".into(),
            apple: 1,
            mango: true,
        };
        let hash = value.compute_hash().unwrap();
        assert_eq!(hash.len(), 64);
    }
}
