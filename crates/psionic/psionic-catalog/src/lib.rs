//! Local blob and catalog substrate for Psionic.

mod ollama;
mod registry;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use ollama::*;
pub use registry::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "local blob access and catalog substrate";

/// Local blob family known to the catalog layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalBlobKind {
    /// Standalone GGUF file discovered directly on disk.
    GgufFile,
    /// Ollama-managed blob resolved by digest inside the models directory.
    OllamaBlob,
}

/// Preferred local read strategy for a blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobReadPreference {
    /// Prefer memory mapping but fall back to buffered reads when mmap fails.
    PreferMemoryMap,
    /// Require memory mapping and fail instead of falling back.
    RequireMemoryMap,
    /// Prefer a buffered in-memory read instead of mmap.
    PreferBuffered,
}

/// Actual local read path used after opening a blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobReadPath {
    /// The blob bytes are exposed through a memory map.
    MemoryMapped,
    /// The blob bytes are exposed from a buffered in-memory copy.
    Buffered,
}

/// Integrity policy applied when opening a local blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobIntegrityPolicy {
    /// Compute a stable SHA-256 over the full blob bytes.
    Sha256,
    /// Skip the full blob hash and emit a stable local-path metadata label instead.
    LocalUnverifiedLabel,
}

/// Open options for local blobs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalBlobOpenOptions {
    /// Preferred local read strategy.
    pub read_preference: BlobReadPreference,
    /// Logical page size to use for paged range views.
    pub page_size: usize,
    /// Integrity policy for the opened blob metadata.
    pub integrity_policy: BlobIntegrityPolicy,
    /// Optional digest expected by the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
}

impl Default for LocalBlobOpenOptions {
    fn default() -> Self {
        Self {
            read_preference: BlobReadPreference::PreferMemoryMap,
            page_size: 4096,
            integrity_policy: BlobIntegrityPolicy::Sha256,
            expected_sha256: None,
        }
    }
}

impl LocalBlobOpenOptions {
    /// Returns a copy with a different read preference.
    #[must_use]
    pub fn with_read_preference(mut self, read_preference: BlobReadPreference) -> Self {
        self.read_preference = read_preference;
        self
    }

    /// Returns a copy with a different logical page size.
    #[must_use]
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    /// Returns a copy with a different integrity policy.
    #[must_use]
    pub fn with_integrity_policy(mut self, integrity_policy: BlobIntegrityPolicy) -> Self {
        self.integrity_policy = integrity_policy;
        self
    }

    /// Returns a copy with an expected digest.
    #[must_use]
    pub fn with_expected_sha256(mut self, expected_sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(expected_sha256.into());
        self
    }
}

/// Stable metadata for an opened local blob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalBlobMetadata {
    /// Logical blob kind.
    pub kind: LocalBlobKind,
    /// Local file name.
    pub name: String,
    /// Absolute or caller-provided path used to open the blob.
    pub path: PathBuf,
    /// Blob length in bytes.
    pub byte_length: u64,
    /// Stable SHA-256 digest over the blob bytes.
    pub sha256: String,
    /// Actual local read path used for the bytes.
    pub read_path: BlobReadPath,
    /// Logical page size for paged range views.
    pub page_size: usize,
    /// Explicit fallback reason when a preferred mmap path was not used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

/// Blob access failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlobError {
    /// Opening or statting a blob file failed because it does not exist.
    #[error("blob `{path}` does not exist")]
    MissingFile {
        /// Path that could not be opened.
        path: String,
    },
    /// Opening or reading a blob failed for another reason.
    #[error("failed to read blob `{path}`: {message}")]
    Read {
        /// Path that failed.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// Memory mapping was required but failed.
    #[error("failed to memory map blob `{path}`: {message}")]
    MemoryMap {
        /// Path that failed.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// The requested page size is invalid.
    #[error("invalid blob page size `{page_size}`")]
    InvalidPageSize {
        /// Invalid page size requested by the caller.
        page_size: usize,
    },
    /// The caller supplied an invalid digest string.
    #[error("invalid ollama blob digest `{digest}`")]
    InvalidDigestFormat {
        /// Invalid digest string.
        digest: String,
    },
    /// The opened blob digest does not match the expected identity.
    #[error("blob digest mismatch for `{path}`: expected `{expected}`, actual `{actual}`")]
    DigestMismatch {
        /// Path that was opened.
        path: String,
        /// Expected digest.
        expected: String,
        /// Actual digest over the blob bytes.
        actual: String,
    },
    /// The requested byte range is outside the blob bounds.
    #[error("blob range [{offset}, {end}) is out of bounds for `{path}` with length {byte_length}")]
    RangeOutOfBounds {
        /// Blob path.
        path: String,
        /// Requested range start.
        offset: usize,
        /// Requested range length.
        len: usize,
        /// Blob length in bytes.
        byte_length: usize,
        /// Range end, cached for the error message.
        end: usize,
    },
    /// The requested page index is outside the paged range.
    #[error("page index {page_index} is out of bounds for page count {page_count}")]
    PageOutOfBounds {
        /// Requested page index.
        page_index: usize,
        /// Total available page count.
        page_count: usize,
    },
}

#[derive(Debug)]
enum BlobBacking {
    MemoryMapped(memmap2::Mmap),
    Buffered(Vec<u8>),
}

impl BlobBacking {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::MemoryMapped(bytes) => bytes,
            Self::Buffered(bytes) => bytes.as_slice(),
        }
    }
}

/// Opened local blob that can expose mmap-backed or buffered bytes.
#[derive(Clone, Debug)]
pub struct LocalBlob {
    metadata: LocalBlobMetadata,
    backing: Arc<BlobBacking>,
}

impl LocalBlob {
    /// Opens a local blob directly from a path.
    pub fn open_path(
        path: impl AsRef<Path>,
        kind: LocalBlobKind,
        options: LocalBlobOpenOptions,
    ) -> Result<Self, BlobError> {
        if options.page_size == 0 {
            return Err(BlobError::InvalidPageSize {
                page_size: options.page_size,
            });
        }

        let path = path.as_ref().to_path_buf();
        let file = fs::File::open(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => BlobError::MissingFile {
                path: path.display().to_string(),
            },
            _ => BlobError::Read {
                path: path.display().to_string(),
                message: error.to_string(),
            },
        })?;
        let file_metadata = file.metadata().map_err(|error| BlobError::Read {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let byte_length_u64 = file_metadata.len();
        let byte_length = usize::try_from(byte_length_u64).map_err(|_| BlobError::Read {
            path: path.display().to_string(),
            message: String::from("blob length does not fit usize on this platform"),
        })?;

        let (backing, read_path, fallback_reason) =
            open_blob_backing(&path, &file, byte_length, options.read_preference)?;
        let sha256 = if options.expected_sha256.is_some()
            || matches!(options.integrity_policy, BlobIntegrityPolicy::Sha256)
        {
            format!("sha256:{}", hex::encode(Sha256::digest(backing.bytes())))
        } else {
            local_unverified_integrity_label(&path, byte_length_u64, &file_metadata)
        };
        if let Some(expected) = options.expected_sha256.as_deref() {
            let expected = canonical_ollama_digest(expected)?;
            if sha256 != expected {
                return Err(BlobError::DigestMismatch {
                    path: path.display().to_string(),
                    expected,
                    actual: sha256,
                });
            }
        }

        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        Ok(Self {
            metadata: LocalBlobMetadata {
                kind,
                name,
                path,
                byte_length: byte_length_u64,
                sha256,
                read_path,
                page_size: options.page_size,
                fallback_reason,
            },
            backing: Arc::new(backing),
        })
    }

    /// Opens an Ollama-managed blob by digest from the provided models root.
    pub fn open_ollama_blob(
        models_root: impl AsRef<Path>,
        digest: &str,
        mut options: LocalBlobOpenOptions,
    ) -> Result<Self, BlobError> {
        let expected = canonical_ollama_digest(digest)?;
        if options.expected_sha256.is_none() {
            options.expected_sha256 = Some(expected.clone());
        }
        let path = ollama_blob_path(models_root, digest)?;
        Self::open_path(path, LocalBlobKind::OllamaBlob, options)
    }

    /// Returns stable metadata for the opened blob.
    #[must_use]
    pub fn metadata(&self) -> &LocalBlobMetadata {
        &self.metadata
    }

    /// Returns the full blob bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.backing.bytes()
    }

    /// Returns a validated byte slice inside the blob.
    pub fn read_range(&self, offset: usize, len: usize) -> Result<&[u8], BlobError> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| BlobError::RangeOutOfBounds {
                path: self.metadata.path.display().to_string(),
                offset,
                len,
                byte_length: self.bytes().len(),
                end: usize::MAX,
            })?;
        self.bytes()
            .get(offset..end)
            .ok_or_else(|| BlobError::RangeOutOfBounds {
                path: self.metadata.path.display().to_string(),
                offset,
                len,
                byte_length: self.bytes().len(),
                end,
            })
    }

    /// Returns a paged range view inside the blob.
    pub fn paged_range(&self, offset: usize, len: usize) -> Result<PagedBlobRange, BlobError> {
        self.read_range(offset, len)?;
        Ok(PagedBlobRange {
            blob: self.clone(),
            offset,
            len,
        })
    }
}

fn local_unverified_integrity_label(
    path: &Path,
    byte_length: u64,
    metadata: &fs::Metadata,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(byte_length.to_le_bytes());
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            hasher.update(duration.as_secs().to_le_bytes());
            hasher.update(duration.subsec_nanos().to_le_bytes());
        }
    }
    format!("local-unverified:{}", hex::encode(hasher.finalize()))
}

/// Paged view over a byte range inside a local blob.
#[derive(Clone, Debug)]
pub struct PagedBlobRange {
    blob: LocalBlob,
    offset: usize,
    len: usize,
}

impl PagedBlobRange {
    /// Returns the underlying blob metadata.
    #[must_use]
    pub fn blob_metadata(&self) -> &LocalBlobMetadata {
        self.blob.metadata()
    }

    /// Returns the range start offset inside the blob.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the range byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the paged range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the logical page size.
    #[must_use]
    pub fn page_size(&self) -> usize {
        self.blob.metadata().page_size
    }

    /// Returns the total page count for the range.
    #[must_use]
    pub fn page_count(&self) -> usize {
        if self.len == 0 {
            0
        } else {
            self.len.div_ceil(self.page_size())
        }
    }

    /// Returns the full range bytes.
    pub fn bytes(&self) -> Result<&[u8], BlobError> {
        self.blob.read_range(self.offset, self.len)
    }

    /// Returns a single page slice.
    pub fn page(&self, page_index: usize) -> Result<&[u8], BlobError> {
        if page_index >= self.page_count() {
            return Err(BlobError::PageOutOfBounds {
                page_index,
                page_count: self.page_count(),
            });
        }
        let page_offset = page_index * self.page_size();
        let page_len = self.page_size().min(self.len - page_offset);
        self.read_range(page_offset, page_len)
    }

    /// Returns a validated slice inside the paged range.
    pub fn read_range(&self, offset: usize, len: usize) -> Result<&[u8], BlobError> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| BlobError::RangeOutOfBounds {
                path: self.blob.metadata().path.display().to_string(),
                offset: self.offset.saturating_add(offset),
                len,
                byte_length: self.blob.bytes().len(),
                end: usize::MAX,
            })?;
        if end > self.len {
            return Err(BlobError::RangeOutOfBounds {
                path: self.blob.metadata().path.display().to_string(),
                offset: self.offset.saturating_add(offset),
                len,
                byte_length: self.blob.bytes().len(),
                end: self.offset.saturating_add(end),
            });
        }
        self.blob.read_range(self.offset + offset, len)
    }
}

/// Resolves an Ollama blob file path from a models root and digest.
pub fn ollama_blob_path(models_root: impl AsRef<Path>, digest: &str) -> Result<PathBuf, BlobError> {
    let digest = canonical_ollama_digest(digest)?;
    let digest = digest.replace(':', "-");
    Ok(models_root.as_ref().join("blobs").join(digest))
}

fn open_blob_backing(
    path: &Path,
    file: &fs::File,
    byte_length: usize,
    read_preference: BlobReadPreference,
) -> Result<(BlobBacking, BlobReadPath, Option<String>), BlobError> {
    match read_preference {
        BlobReadPreference::PreferBuffered => {
            let bytes = fs::read(path).map_err(|error| BlobError::Read {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            Ok((BlobBacking::Buffered(bytes), BlobReadPath::Buffered, None))
        }
        BlobReadPreference::PreferMemoryMap | BlobReadPreference::RequireMemoryMap => {
            if byte_length == 0 {
                return match read_preference {
                    BlobReadPreference::RequireMemoryMap => Err(BlobError::MemoryMap {
                        path: path.display().to_string(),
                        message: String::from("zero-length blobs cannot be memory mapped"),
                    }),
                    BlobReadPreference::PreferMemoryMap => Ok((
                        BlobBacking::Buffered(Vec::new()),
                        BlobReadPath::Buffered,
                        Some(String::from("zero-length blobs cannot be memory mapped")),
                    )),
                    BlobReadPreference::PreferBuffered => unreachable!(),
                };
            }

            match unsafe { memmap2::MmapOptions::new().map(file) } {
                Ok(bytes) => Ok((
                    BlobBacking::MemoryMapped(bytes),
                    BlobReadPath::MemoryMapped,
                    None,
                )),
                Err(error) => match read_preference {
                    BlobReadPreference::RequireMemoryMap => Err(BlobError::MemoryMap {
                        path: path.display().to_string(),
                        message: error.to_string(),
                    }),
                    BlobReadPreference::PreferMemoryMap => {
                        let bytes = fs::read(path).map_err(|read_error| BlobError::Read {
                            path: path.display().to_string(),
                            message: read_error.to_string(),
                        })?;
                        Ok((
                            BlobBacking::Buffered(bytes),
                            BlobReadPath::Buffered,
                            Some(format!("mmap failed: {error}")),
                        ))
                    }
                    BlobReadPreference::PreferBuffered => unreachable!(),
                },
            }
        }
    }
}

pub(crate) fn canonical_ollama_digest(digest: &str) -> Result<String, BlobError> {
    let normalized = digest.trim();
    let (prefix, value) = if let Some(value) = normalized.strip_prefix("sha256:") {
        ("sha256", value)
    } else if let Some(value) = normalized.strip_prefix("sha256-") {
        ("sha256", value)
    } else {
        return Err(BlobError::InvalidDigestFormat {
            digest: normalized.to_string(),
        });
    };
    if prefix != "sha256"
        || value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BlobError::InvalidDigestFormat {
            digest: normalized.to_string(),
        });
    }
    Ok(format!("sha256:{}", value.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
    };

    use tempfile::tempdir;

    use super::{
        BlobError, BlobIntegrityPolicy, BlobReadPath, BlobReadPreference, LocalBlob, LocalBlobKind,
        LocalBlobOpenOptions, ollama_blob_path,
    };

    fn write_blob(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(bytes)?;
        Ok(())
    }

    #[test]
    fn ollama_blob_path_normalizes_digest() -> Result<(), Box<dyn std::error::Error>> {
        let path = ollama_blob_path(
            "/tmp/ollama-models",
            "sha256:abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000",
        )?;
        assert_eq!(
            path,
            PathBuf::from(
                "/tmp/ollama-models/blobs/sha256-abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000abcd0000"
            )
        );
        Ok(())
    }

    #[test]
    fn local_blob_reads_buffered_and_reports_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("buffered.gguf");
        let bytes = (0_u8..32).collect::<Vec<_>>();
        write_blob(&path, &bytes)?;

        let blob = LocalBlob::open_path(
            &path,
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered)
                .with_page_size(8),
        )?;

        assert_eq!(blob.metadata().read_path, BlobReadPath::Buffered);
        assert_eq!(blob.metadata().page_size, 8);
        assert_eq!(blob.bytes(), bytes.as_slice());
        assert_eq!(blob.read_range(4, 4)?, &[4, 5, 6, 7]);
        Ok(())
    }

    #[test]
    fn local_blob_can_require_memory_mapping() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("mapped.gguf");
        let bytes = (0_u8..64).collect::<Vec<_>>();
        write_blob(&path, &bytes)?;

        let blob = LocalBlob::open_path(
            &path,
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::RequireMemoryMap),
        )?;

        assert_eq!(blob.metadata().read_path, BlobReadPath::MemoryMapped);
        assert_eq!(blob.read_range(8, 8)?, &[8, 9, 10, 11, 12, 13, 14, 15]);
        Ok(())
    }

    #[test]
    fn paged_blob_range_supports_partial_and_repeated_reads()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("paged.gguf");
        let bytes = (0_u8..48).collect::<Vec<_>>();
        write_blob(&path, &bytes)?;

        let blob = LocalBlob::open_path(
            &path,
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered)
                .with_page_size(8),
        )?;
        let paged = blob.paged_range(5, 20)?;

        assert_eq!(paged.page_count(), 3);
        assert_eq!(paged.page(0)?, &[5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(paged.page(1)?, &[13, 14, 15, 16, 17, 18, 19, 20]);
        assert_eq!(paged.page(2)?, &[21, 22, 23, 24]);
        assert_eq!(paged.read_range(3, 6)?, &[8, 9, 10, 11, 12, 13]);
        assert_eq!(paged.read_range(3, 6)?, &[8, 9, 10, 11, 12, 13]);
        Ok(())
    }

    #[test]
    fn local_blob_detects_digest_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("digest.gguf");
        write_blob(&path, b"psionic")?;

        let error = LocalBlob::open_path(
            &path,
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default().with_expected_sha256(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ),
        )
        .expect_err("digest mismatch should fail");
        assert!(matches!(error, BlobError::DigestMismatch { .. }));
        Ok(())
    }

    #[test]
    fn local_blob_can_skip_full_sha256_for_unverified_local_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("fast-local.gguf");
        write_blob(&path, b"psionic")?;

        let blob = LocalBlob::open_path(
            &path,
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default()
                .with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel),
        )?;

        assert!(blob.metadata().sha256.starts_with("local-unverified:"));
        Ok(())
    }

    #[test]
    fn local_blob_reports_missing_file() {
        let error = LocalBlob::open_path(
            "/tmp/does-not-exist.gguf",
            LocalBlobKind::GgufFile,
            LocalBlobOpenOptions::default(),
        )
        .expect_err("missing file should fail");
        assert!(matches!(error, BlobError::MissingFile { .. }));
    }
}
