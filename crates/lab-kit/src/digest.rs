//! Stable content digests: SHA-256 of canonical JSON.
//!
//! Object keys are written in byte order whatever map type serde_json was
//! built with (a crate enabling `preserve_order` elsewhere in a workspace
//! changes it), no whitespace, and numbers as serde_json writes them. Equal
//! content has an equal digest on every platform.

use serde_json::Value;
use sha2::{Digest as _, Sha256};

pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap_or_default()),
    }
}

/// Lowercase hex SHA-256 of [`canonical_json`].
pub fn digest(value: &Value) -> String {
    Sha256::digest(canonical_json(value).as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_and_spacing_do_not_change_the_digest() {
        let a: Value = serde_json::from_str(r#"{"b": 1, "a": {"y": [1, 2], "x": "é"}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":{"x":"é","y":[1,2]},"b":1}"#).unwrap();
        assert_eq!(digest(&a), digest(&b));
        assert_eq!(canonical_json(&a), r#"{"a":{"x":"é","y":[1,2]},"b":1}"#);
        assert_ne!(
            digest(&a),
            digest(&json!({"a": {"x": "é", "y": [2, 1]}, "b": 1}))
        );
    }

    #[test]
    fn a_known_value_has_a_fixed_digest() {
        // Pinned so a change to the canonical form is noticed: every stored
        // version's digest depends on it.
        assert_eq!(
            digest(&json!({"shots": [{"dur": 12.5, "name": "s4"}]})),
            "3e0142870207e052f9a0e10f3bfe94f08cc93c77140f1725b2875b5a65d92244"
        );
    }
}
