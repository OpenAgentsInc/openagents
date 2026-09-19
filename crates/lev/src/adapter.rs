//! Reading and checking a `.fmadapter` package.
//!
//! An adapter is the only way to teach Apple's model a task rather than
//! describe one, and the package it arrives in is strict. A package with the
//! right tensor values and the wrong container layout is rejected by the
//! runtime, so this module checks the layout before the runtime is asked to,
//! and says which rule failed.
//!
//! The format is reimplemented here from its published shape rather than
//! carried over from a sibling repository:
//!
//! - The package is a directory whose name ends in `.fmadapter`.
//! - It holds `metadata.json` and `adapter_weights.bin`.
//! - It may hold a draft-model pair, `draft.mil` and `draft_weights.bin`.
//!   Both or neither: a half pair is not a package state.
//! - `adapter_weights.bin` is a Core ML blob-storage container, not raw
//!   tensor bytes. One 64-byte file header carries a little-endian `u32`
//!   record count and a little-endian `u32` version. Each record starts at a
//!   64-byte-aligned offset with a 64-byte header: the magic `0xdeadbeef`, a
//!   blob kind, the payload length, and the absolute payload offset, all
//!   little-endian. Payloads are padded to the next 64-byte boundary.
//! - `metadata.json` uses camelCase keys and must carry
//!   `adapterIdentifier`, a 40-character lowercase hex `baseModelSignature`,
//!   and a positive `loraRank`.
//!
//! The signature is the part that matters operationally. It pins a package to
//! one base model, and the base ships with the operating system, so an OS
//! update invalidates every adapter trained against the old one. A door
//! refuses on mismatch rather than serving an incompatible pairing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Refusal, RefusalCode, Result};

/// The file every package carries.
pub const METADATA_FILE: &str = "metadata.json";
/// The weights every package carries.
pub const WEIGHTS_FILE: &str = "adapter_weights.bin";
/// The draft model's program, optional but paired.
pub const DRAFT_PROGRAM_FILE: &str = "draft.mil";
/// The draft model's weights, optional but paired.
pub const DRAFT_WEIGHTS_FILE: &str = "draft_weights.bin";

/// The package directory's required suffix.
pub const PACKAGE_SUFFIX: &str = ".fmadapter";

/// The blob-storage record magic.
pub const RECORD_MAGIC: u32 = 0xdead_beef;
/// The blob-storage alignment, for the file header and every record.
pub const ALIGNMENT: usize = 64;
/// The blob-storage version this reader understands.
pub const BLOB_VERSION: u32 = 2;

/// A package's metadata, with unknown keys preserved so a round trip does not
/// silently drop what a newer toolkit wrote.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    /// The stable artifact label, conventionally
    /// `fmadapter-<name>-<signature prefix>`.
    #[serde(rename = "adapterIdentifier")]
    pub adapter_identifier: String,
    /// The base model this package is pinned to: 40 lowercase hex characters.
    #[serde(rename = "baseModelSignature")]
    pub base_model_signature: String,
    /// The LoRA rank it was trained at.
    #[serde(rename = "loraRank")]
    pub lora_rank: u32,
    /// Who made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// What it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Its license.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Draft tokens, when a draft model rides along.
    #[serde(
        rename = "speculativeDecodingDraftTokenCount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub draft_token_count: Option<u32>,
    /// Producer-defined lineage values, round-tripped whatever they hold.
    #[serde(rename = "creatorDefined", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub creator_defined: BTreeMap<String, Value>,
}

/// One tensor record in the blob container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// What kind of payload it holds. `1` is fp16 in the current lane.
    pub kind: u32,
    /// Payload length in bytes.
    pub length: u64,
    /// Absolute offset of the payload in the file.
    pub offset: u64,
}

/// A validated package on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    /// Where it lives.
    pub path: PathBuf,
    /// What it declares.
    pub metadata: Metadata,
    /// The tensor records its weights file holds.
    pub records: Vec<Record>,
    /// Whether a complete draft-model pair rides along.
    pub has_draft: bool,
}

impl Package {
    /// Reads and checks a package.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.to_string_lossy().ends_with(PACKAGE_SUFFIX) {
            return Err(invalid(format!(
                "a package directory ends in `{PACKAGE_SUFFIX}`, and this one is {}",
                path.display()
            )));
        }
        if !path.is_dir() {
            return Err(invalid(format!("{} is not a directory", path.display())));
        }

        let metadata_path = path.join(METADATA_FILE);
        let text = std::fs::read_to_string(&metadata_path)
            .map_err(|error| invalid(format!("{} did not read: {error}", metadata_path.display())))?;
        let metadata: Metadata = serde_json::from_str(&text)
            .map_err(|error| invalid(format!("{METADATA_FILE} did not parse: {error}")))?;
        check_metadata(&metadata)?;

        let weights_path = path.join(WEIGHTS_FILE);
        let bytes = std::fs::read(&weights_path)
            .map_err(|error| invalid(format!("{} did not read: {error}", weights_path.display())))?;
        let records = read_records(&bytes)?;

        let program = path.join(DRAFT_PROGRAM_FILE).exists();
        let draft_weights = path.join(DRAFT_WEIGHTS_FILE).exists();
        if program != draft_weights {
            return Err(invalid(format!(
                "a draft model is both files or neither; this package has {} and {}",
                if program { DRAFT_PROGRAM_FILE } else { "no program" },
                if draft_weights { DRAFT_WEIGHTS_FILE } else { "no weights" }
            )));
        }

        Ok(Self { path, metadata, records, has_draft: program })
    }

    /// Whether this package may be attached against a running base.
    ///
    /// A mismatch is a refusal rather than a warning. Serving an adapter
    /// trained against a base the device no longer runs produces confident
    /// nonsense.
    pub fn check_signature(&self, running: &str) -> Result<()> {
        if self.metadata.base_model_signature == running {
            return Ok(());
        }
        Err(Refusal::new(
            RefusalCode::AdapterIncompatible,
            format!(
                "the package is pinned to base `{}` and the device runs `{running}`. \
                 An operating system update changes the base and invalidates every adapter \
                 trained against the old one.",
                self.metadata.base_model_signature
            ),
        ))
    }
}

fn invalid(message: String) -> Refusal {
    Refusal::new(RefusalCode::InvalidRequest, message)
}

fn check_metadata(metadata: &Metadata) -> Result<()> {
    if metadata.adapter_identifier.trim().is_empty() {
        return Err(invalid("`adapterIdentifier` is blank".to_string()));
    }
    if metadata.lora_rank == 0 {
        return Err(invalid("`loraRank` is not positive".to_string()));
    }
    let signature = &metadata.base_model_signature;
    if signature.len() != 40 || !signature.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(invalid(format!(
            "`baseModelSignature` is 40 lowercase hex characters, and this one is `{signature}`"
        )));
    }
    Ok(())
}

/// Parses the blob-storage container.
pub fn read_records(bytes: &[u8]) -> Result<Vec<Record>> {
    if bytes.len() < ALIGNMENT {
        return Err(invalid(format!(
            "{WEIGHTS_FILE} is {} bytes, shorter than its {ALIGNMENT}-byte header",
            bytes.len()
        )));
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().expect("four bytes"));
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
    if version != BLOB_VERSION {
        return Err(invalid(format!(
            "{WEIGHTS_FILE} declares blob-storage version {version}; this reader understands {BLOB_VERSION}"
        )));
    }

    let mut records = Vec::with_capacity(count as usize);
    let mut cursor = ALIGNMENT;
    for index in 0..count {
        if cursor % ALIGNMENT != 0 {
            return Err(invalid(format!(
                "record {index} starts at {cursor}, which is not {ALIGNMENT}-byte aligned"
            )));
        }
        let end = cursor + ALIGNMENT;
        if end > bytes.len() {
            return Err(invalid(format!(
                "record {index}'s header runs past the end of {WEIGHTS_FILE}"
            )));
        }
        let header = &bytes[cursor..end];
        let magic = u32::from_le_bytes(header[0..4].try_into().expect("four bytes"));
        if magic != RECORD_MAGIC {
            return Err(invalid(format!(
                "record {index} starts with {magic:#010x} rather than {RECORD_MAGIC:#010x}"
            )));
        }
        let kind = u32::from_le_bytes(header[4..8].try_into().expect("four bytes"));
        let length = u64::from_le_bytes(header[8..16].try_into().expect("eight bytes"));
        let offset = u64::from_le_bytes(header[16..24].try_into().expect("eight bytes"));
        let payload_end = offset.checked_add(length).ok_or_else(|| {
            invalid(format!("record {index} declares an offset and length that overflow"))
        })?;
        if payload_end > bytes.len() as u64 {
            return Err(invalid(format!(
                "record {index}'s payload ends at {payload_end}, past the {} byte file",
                bytes.len()
            )));
        }
        records.push(Record { kind, length, offset });
        cursor = align_up(payload_end as usize);
    }
    Ok(records)
}

/// Rounds up to the next alignment boundary.
#[must_use]
pub fn align_up(value: usize) -> usize {
    value.div_ceil(ALIGNMENT) * ALIGNMENT
}

/// Writes a blob-storage container. Used to build fixtures and to check the
/// reader against a writer that follows the same rules.
#[must_use]
pub fn write_records(payloads: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0_u8; ALIGNMENT];
    out[0..4].copy_from_slice(&(payloads.len() as u32).to_le_bytes());
    out[4..8].copy_from_slice(&BLOB_VERSION.to_le_bytes());

    for (kind, payload) in payloads {
        let header_at = out.len();
        out.resize(header_at + ALIGNMENT, 0);
        let payload_at = out.len();
        out[header_at..header_at + 4].copy_from_slice(&RECORD_MAGIC.to_le_bytes());
        out[header_at + 4..header_at + 8].copy_from_slice(&kind.to_le_bytes());
        out[header_at + 8..header_at + 16]
            .copy_from_slice(&(payload.len() as u64).to_le_bytes());
        out[header_at + 16..header_at + 24]
            .copy_from_slice(&(payload_at as u64).to_le_bytes());
        out.extend_from_slice(payload);
        out.resize(align_up(out.len()), 0);
    }
    out
}

/// Writes a package to disk. Used by tests and by the export check, so the
/// reader is always exercised against a writer that follows the same rules.
pub fn write_package(
    path: &Path,
    metadata: &Metadata,
    payloads: &[(u32, Vec<u8>)],
) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    let text = serde_json::to_vec_pretty(metadata)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path.join(METADATA_FILE), text)?;
    std::fs::write(path.join(WEIGHTS_FILE), write_records(payloads))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNATURE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";

    fn metadata() -> Metadata {
        Metadata {
            adapter_identifier: format!("fmadapter-lev-{}", &SIGNATURE[..8]),
            base_model_signature: SIGNATURE.to_string(),
            lora_rank: 32,
            author: Some("openagents".to_string()),
            description: Some("support-desk judgments".to_string()),
            license: None,
            draft_token_count: None,
            creator_defined: BTreeMap::new(),
        }
    }

    fn write_package(dir: &Path, metadata: &Metadata, weights: &[u8]) -> PathBuf {
        let path = dir.join("lev.fmadapter");
        std::fs::create_dir_all(&path).expect("the package directory");
        std::fs::write(path.join(METADATA_FILE), serde_json::to_vec_pretty(metadata).unwrap())
            .expect("metadata");
        std::fs::write(path.join(WEIGHTS_FILE), weights).expect("weights");
        path
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lev-adapter-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn a_container_round_trips_through_the_writer_and_the_reader() {
        let payloads = vec![(1_u32, vec![0xAB_u8; 130]), (1, vec![0xCD; 64]), (1, vec![0xEF; 3])];
        let bytes = write_records(&payloads);
        let records = read_records(&bytes).expect("the container parses");
        assert_eq!(records.len(), 3);
        for (record, (kind, payload)) in records.iter().zip(&payloads) {
            assert_eq!(record.kind, *kind);
            assert_eq!(record.length, payload.len() as u64);
            assert_eq!(record.offset % ALIGNMENT as u64, 0, "payloads start aligned");
            let start = record.offset as usize;
            assert_eq!(&bytes[start..start + payload.len()], payload.as_slice());
        }
    }

    #[test]
    fn a_good_package_opens() {
        let dir = scratch("good");
        let path = write_package(&dir, &metadata(), &write_records(&[(1, vec![7; 32])]));
        let package = Package::open(&path).expect("the package opens");
        assert_eq!(package.metadata.lora_rank, 32);
        assert_eq!(package.records.len(), 1);
        assert!(!package.has_draft);
        package.check_signature(SIGNATURE).expect("the signature matches");
    }

    #[test]
    fn raw_tensor_bytes_are_refused_even_when_the_values_are_right() {
        // The whole point of checking the container: correct metadata and
        // correct values still fail if the layout is raw bytes.
        let dir = scratch("raw");
        let raw = vec![0x11_u8; 256];
        let path = write_package(&dir, &metadata(), &raw);
        let refusal = Package::open(&path).expect_err("raw bytes are refused");
        assert_eq!(refusal.code, RefusalCode::InvalidRequest);
    }

    #[test]
    fn a_bad_signature_is_refused_at_open() {
        let dir = scratch("sig");
        let mut bad = metadata();
        bad.base_model_signature = "NOTHEX".to_string();
        let path = write_package(&dir, &bad, &write_records(&[(1, vec![7; 32])]));
        let refusal = Package::open(&path).expect_err("a malformed signature is refused");
        assert!(refusal.message.contains("baseModelSignature"), "{}", refusal.message);
    }

    #[test]
    fn a_zero_rank_is_refused() {
        let dir = scratch("rank");
        let mut bad = metadata();
        bad.lora_rank = 0;
        let path = write_package(&dir, &bad, &write_records(&[(1, vec![7; 32])]));
        assert!(Package::open(&path).is_err());
    }

    #[test]
    fn a_half_draft_pair_is_not_a_package_state() {
        let dir = scratch("draft");
        let path = write_package(&dir, &metadata(), &write_records(&[(1, vec![7; 32])]));
        std::fs::write(path.join(DRAFT_PROGRAM_FILE), b"program").expect("the draft program");
        let refusal = Package::open(&path).expect_err("a half pair is refused");
        assert!(refusal.message.contains("both files or neither"), "{}", refusal.message);

        std::fs::write(path.join(DRAFT_WEIGHTS_FILE), b"weights").expect("the draft weights");
        let package = Package::open(&path).expect("a complete pair opens");
        assert!(package.has_draft);
    }

    #[test]
    fn a_directory_without_the_suffix_is_refused() {
        let dir = scratch("suffix");
        let path = dir.join("plain-directory");
        std::fs::create_dir_all(&path).expect("a directory");
        assert!(Package::open(&path).is_err());
    }

    #[test]
    fn a_signature_mismatch_refuses_with_its_own_code() {
        let dir = scratch("mismatch");
        let path = write_package(&dir, &metadata(), &write_records(&[(1, vec![7; 32])]));
        let package = Package::open(&path).expect("the package opens");
        let refusal = package
            .check_signature("0000000000000000000000000000000000000000")
            .expect_err("a different base is refused");
        assert_eq!(refusal.code, RefusalCode::AdapterIncompatible);
        assert!(refusal.message.contains("operating system update"));
    }

    #[test]
    fn unknown_producer_metadata_round_trips() {
        let mut extended = metadata();
        extended
            .creator_defined
            .insert("suiteDigest".to_string(), serde_json::json!("35dfdf43"));
        let text = serde_json::to_string(&extended).expect("it encodes");
        let back: Metadata = serde_json::from_str(&text).expect("it decodes");
        assert_eq!(back.creator_defined["suiteDigest"], serde_json::json!("35dfdf43"));
    }
}
