//! Content digests over canonical JSON.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// `sha256:` and 64 lowercase hex digits.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);

impl Digest {
    /// The digest of these bytes.
    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let hash = Sha256::digest(bytes);
        let mut out = String::with_capacity(71);
        out.push_str("sha256:");
        for byte in hash {
            out.push_str(&format!("{byte:02x}"));
        }
        Self(out)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Digest {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("digest without sha256: prefix: {value}"))?;
        if hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err(format!("digest is not 64 lowercase hex digits: {value}"))
        }
    }
}

impl From<Digest> for String {
    fn from(value: Digest) -> Self {
        value.0
    }
}

/// The canonical bytes of a JSON value: object keys sorted by their UTF-8
/// bytes, no insignificant whitespace, serde_json's number and string
/// encoding.
#[must_use]
pub fn canonical(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend(serde_json::to_vec(key).unwrap_or_default());
                out.push(b':');
                write(&map[key], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write(item, out);
            }
            out.push(b']');
        }
        scalar => out.extend(serde_json::to_vec(scalar).unwrap_or_default()),
    }
}

/// The digest of a value's canonical JSON.
///
/// # Panics
///
/// Never for the types in this crate: each serializes to JSON without
/// non-string map keys.
#[must_use]
pub fn digest_of<T: Serialize>(value: &T) -> Digest {
    let value = serde_json::to_value(value).expect("contract types serialize to JSON");
    Digest::of_bytes(&canonical(&value))
}
