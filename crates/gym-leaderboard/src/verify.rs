//! Checking published bytes against their digests, with no generator.
//!
//! A reader trusts only digests, never the host that served the bytes:
//!
//! - a leaderboard's `digest` is the ATIF-rule digest (object keys sorted
//!   at every depth, then SHA-256) of its `boards`, computed here over the
//!   JSON exactly as served, so fields and variants this reader doesn't
//!   know still count; it must also equal the digest the index names;
//! - a trace bundle's SHA-256 over its bytes must equal the leaderboard's
//!   [`TraceRef`].

use std::fmt;

use serde_json::Value;

use crate::contract::{
    LEADERBOARD_SCHEMA, Leaderboard, TRACE_BUNDLE_SCHEMA, TraceBundle, TraceRef,
};
use crate::evidence::sha256_hex;

/// Why published bytes weren't accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not JSON, or not the schema it claims.
    Malformed(String),
    /// Over the file's bound.
    TooLarge { bytes: usize, bound: usize },
    /// The content doesn't hash to what it must.
    DigestMismatch { expected: String, computed: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "malformed: {why}"),
            Self::TooLarge { bytes, bound } => {
                write!(f, "{bytes} bytes, over the {bound}-byte bound")
            }
            Self::DigestMismatch { expected, computed } => {
                write!(f, "digest {computed} isn't the expected {expected}")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// A leaderboard's bytes, checked: its schema, its own digest over the
/// served `boards`, and, when given, the digest the index names.
pub fn leaderboard(bytes: &[u8], expected: Option<&str>) -> Result<Leaderboard, Refusal> {
    if bytes.len() > crate::MAX_LEADERBOARD_BYTES {
        return Err(Refusal::TooLarge {
            bytes: bytes.len(),
            bound: crate::MAX_LEADERBOARD_BYTES,
        });
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| Refusal::Malformed(e.to_string()))?;
    if value.get("schema").and_then(Value::as_str) != Some(LEADERBOARD_SCHEMA) {
        return Err(Refusal::Malformed(format!("not {LEADERBOARD_SCHEMA}")));
    }
    let boards = value
        .get("boards")
        .ok_or_else(|| Refusal::Malformed("no boards".into()))?;
    let computed = atif::digest(boards);
    let recorded = value
        .get("digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    for want in std::iter::once(recorded).chain(expected) {
        if want != computed {
            return Err(Refusal::DigestMismatch {
                expected: want.to_owned(),
                computed,
            });
        }
    }
    serde_json::from_value(value).map_err(|e| Refusal::Malformed(e.to_string()))
}

/// A trace bundle's bytes, checked against the leaderboard's reference.
pub fn bundle(bytes: &[u8], trace: &TraceRef) -> Result<TraceBundle, Refusal> {
    if bytes.len() > crate::MAX_BUNDLE_BYTES {
        return Err(Refusal::TooLarge {
            bytes: bytes.len(),
            bound: crate::MAX_BUNDLE_BYTES,
        });
    }
    let computed = sha256_hex(bytes);
    if computed != trace.sha256 {
        return Err(Refusal::DigestMismatch {
            expected: trace.sha256.clone(),
            computed,
        });
    }
    let bundle: TraceBundle =
        serde_json::from_slice(bytes).map_err(|e| Refusal::Malformed(e.to_string()))?;
    if bundle.schema != TRACE_BUNDLE_SCHEMA {
        return Err(Refusal::Malformed(format!("not {TRACE_BUNDLE_SCHEMA}")));
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn published(rel: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read(root.join(crate::PUBLISHED).join(rel)).unwrap()
    }

    #[test]
    fn the_committed_leaderboard_and_a_bundle_verify() {
        let bytes = published(crate::LEADERBOARD_FILE);
        let lb = leaderboard(&bytes, None).unwrap();
        assert!(leaderboard(&bytes, Some(&lb.digest)).is_ok());
        let trace = lb
            .boards
            .iter()
            .flat_map(|b| &b.attempts)
            .find_map(|a| a.trace.clone())
            .unwrap();
        assert!(bundle(&published(&trace.path), &trace).is_ok());
    }

    #[test]
    fn a_changed_byte_or_an_unexpected_digest_refuses() {
        let bytes = published(crate::LEADERBOARD_FILE);
        assert!(matches!(
            leaderboard(&bytes, Some(&"0".repeat(64))),
            Err(Refusal::DigestMismatch { .. })
        ));
        let text = String::from_utf8(bytes).unwrap();
        let edited = text.replacen("\"passes\":13", "\"passes\":14", 1);
        assert_ne!(edited, text);
        assert!(matches!(
            leaderboard(edited.as_bytes(), None),
            Err(Refusal::DigestMismatch { .. })
        ));
        // A field this reader doesn't know still counts toward the digest.
        let mut value: Value = serde_json::from_str(&text).unwrap();
        value["boards"][0]["new_field"] = Value::from(1);
        let digest = atif::digest(&value["boards"]);
        value["digest"] = Value::from(digest.clone());
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(leaderboard(&bytes, Some(&digest)).is_ok());
    }
}
