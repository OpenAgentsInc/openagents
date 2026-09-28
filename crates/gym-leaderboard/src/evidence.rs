//! Reading committed evidence: JSON files, their digests, and typed
//! field access that fails loudly instead of defaulting to zero.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::contract::EvidenceFile;

/// Why a board or bundle couldn't be built.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Builds an [`Error`] from a format string.
macro_rules! fail {
    ($($arg:tt)*) => { $crate::evidence::Error(format!($($arg)*)) };
}
#[cfg_attr(not(feature = "generate"), allow(unused_imports))]
pub(crate) use fail;

/// The repository root and every file read under it, in read order.
#[derive(Debug)]
pub struct Reader {
    root: PathBuf,
}

impl Reader {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The bytes of a repository-relative file and its evidence entry.
    pub fn bytes(&self, rel: &str) -> Result<(Vec<u8>, EvidenceFile)> {
        let bytes = std::fs::read(self.root.join(rel)).map_err(|e| fail!("read {rel}: {e}"))?;
        let entry = EvidenceFile {
            path: rel.to_owned(),
            sha256: sha256_hex(&bytes),
            bytes: bytes.len() as u64,
        };
        Ok((bytes, entry))
    }

    /// A repository-relative JSON file and its evidence entry.
    pub fn json(&self, rel: &str) -> Result<(Value, EvidenceFile)> {
        let (bytes, entry) = self.bytes(rel)?;
        let value = serde_json::from_slice(&bytes).map_err(|e| fail!("parse {rel}: {e}"))?;
        Ok((value, entry))
    }

    /// A repository-relative text file and its evidence entry.
    pub fn text(&self, rel: &str) -> Result<(String, EvidenceFile)> {
        let (bytes, entry) = self.bytes(rel)?;
        Ok((String::from_utf8_lossy(&bytes).into_owned(), entry))
    }

    /// Whether a repository-relative path exists.
    #[must_use]
    pub fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    /// The sorted entries of a repository-relative directory.
    pub fn list(&self, rel: &str) -> Result<Vec<String>> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.join(rel))
            .map_err(|e| fail!("list {rel}: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        Ok(names)
    }
}

/// Lower-case hex SHA-256.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The field at a `/`-separated path.
#[must_use]
pub fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('/')
        .fold(value, |v, key| v.get(key).unwrap_or(&Value::Null))
}

/// A required string.
pub fn string(value: &Value, path: &str) -> Result<String> {
    at(value, path)
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| fail!("missing string {path}"))
}

/// An optional string.
#[must_use]
pub fn opt_string(value: &Value, path: &str) -> Option<String> {
    at(value, path).as_str().map(str::to_owned)
}

/// A required number.
pub fn number(value: &Value, path: &str) -> Result<f64> {
    at(value, path)
        .as_f64()
        .ok_or_else(|| fail!("missing number {path}"))
}

/// An optional number; `null` and absence are both `None`.
#[must_use]
pub fn opt_number(value: &Value, path: &str) -> Option<f64> {
    at(value, path).as_f64()
}

/// An optional unsigned integer.
#[must_use]
pub fn opt_u64(value: &Value, path: &str) -> Option<u64> {
    at(value, path).as_u64()
}

/// A required array.
pub fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>> {
    at(value, path)
        .as_array()
        .ok_or_else(|| fail!("missing array {path}"))
}

/// Whether two money or time figures agree to within rounding.
#[must_use]
pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6_f64.max(1e-9 * a.abs().max(b.abs()))
}

/// Converts a count to `u32`, saturating.
#[must_use]
pub fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
