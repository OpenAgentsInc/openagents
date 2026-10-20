//! Native safetensors reader for DeepSeek-V4.1-Flash.
//!
//! The checkpoint ships as 48 safetensors shards with FP8 E4M3 dense weights,
//! packed FP4 E2M1 routed experts, and UE8M0 (`F8_E8M0`) scale tensors. This
//! module reads them as stored, without converting to GGUF, so the bytes and
//! the checkpoint identity stay the same as the pinned Hugging Face revision.
//!
//! What it provides:
//!
//! - [`SafetensorsIndex`] and [`SafetensorsShardHeader`]: the index and each
//!   shard's header, with unknown dtypes, overlapping ranges, ranges past the
//!   file end, and tensors missing from their shard refused.
//! - [`CheckpointPin`] and [`CheckpointIdentity`]: the revision, the index
//!   SHA-256, and each shard's SHA-256. Hashing every shard is a separate,
//!   explicit step ([`DeepSeekV41Checkpoint::verify_shard_hashes`]).
//! - [`DeepSeekV41Checkpoint`]: ranged reads (`pread`), so nothing loads a
//!   whole shard into memory.
//! - Typed dequantization: [`decode_f8_e4m3fn`], [`decode_f8_e8m0fnu`],
//!   [`decode_e2m1`], [`decode_bf16`], and the block and row helpers the CPU
//!   reference and the CUDA parity tests use.
//! - [`V41StageSelection`]: the tensors and bytes one stage Pylon needs for a
//!   layer range.
//! - [`V41ModelMap`]: every tensor name mapped to a typed [`V41TensorRole`],
//!   so a missing or extra tensor is an error.
//!
//! # Formats, from the reference
//!
//! The reference is `inference/model.py`, `inference/kernel.py`, and
//! `inference/convert.py` at revision
//! `1bae0e1088993b9b82de08f59de5be5a34e42170`.
//!
//! - `model.py` lines 26–30: `fp8_block_size = 32`, `fp4_block_size = 32`,
//!   `scale_fmt = "ue8m0"`, `scale_dtype = torch.float8_e8m0fnu`.
//! - `model.py` lines 219–231: an FP4 weight is `[out, in // 2]` of
//!   `float4_e2m1fn_x2` with a scale of `[out, in // 32]`; an FP8 weight is
//!   `[out, in]` with a scale of `[ceil(out / 32), ceil(in / 32)]`.
//! - `model.py` lines 317–320: an Engram table row is dequantized as
//!   `values.unflatten(-1, (-1, 32)) * scales.unsqueeze(-1)`, one scale per 32
//!   values along the row.
//! - `convert.py` lines 13–15 and 31–34: the E2M1 table is
//!   `[0, 0.5, 1, 1.5, 2, 3, 4, 6]` with bit 3 as the sign, and a packed byte
//!   holds element `2i` in its **low** nibble (`x & 0x0F`) and element
//!   `2i + 1` in its **high** nibble (`(x >> 4) & 0x0F`), stacked as
//!   `[low, high]` along K.
//! - `kernel.py` lines 22–37 (`fast_round_scale`): UE8M0 scales are powers
//!   of two, `2^(e - 127)`.
//!
//! # MXFP4
//!
//! Psionic's `QuantizationMode::GgmlMxfp4` uses the same E2M1 values, the same
//! 32-element block, and the same E8M0 exponent, but its bytes differ in two
//! ways, so V4.1 doesn't reuse it:
//!
//! 1. GGML stores each 32-element block as 17 bytes, one E8M0 scale followed
//!    by 16 packed bytes. V4.1 keeps the scales in a separate `.scale` tensor.
//! 2. GGML puts element `j` in the low nibble and element `j + 16` in the high
//!    nibble of byte `j`. V4.1 (`float4_e2m1fn_x2`) puts elements `2i` and
//!    `2i + 1` in byte `i`.
//!
//! Also, GGML's decoder maps scale byte `0xFF` to infinity, while
//! `float8_e8m0fnu` defines it as NaN. Repacking into GGML blocks would change
//! the stored bytes and the checkpoint identity, so V4.1 has its own
//! [`V41BlockFormat`] instead of a new `QuantizationMode`, whose modes all
//! describe GGUF blocks with inline scales.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::File,
    io::Read,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Hugging Face repository of the pinned checkpoint.
pub const DEEPSEEK_V41_FLASH_REPO: &str = "deepseek-ai/DeepSeek-V4.1-Flash-1017";
/// Pinned Hugging Face revision.
pub const DEEPSEEK_V41_FLASH_REVISION: &str = "1bae0e1088993b9b82de08f59de5be5a34e42170";
/// SHA-256 of `model.safetensors.index.json` at the pinned revision.
pub const DEEPSEEK_V41_FLASH_INDEX_SHA256: &str =
    "74b0686a3d2891980d5e303251b075a3bccae2c2ff650747db2620a649b98fa8";
/// Total tensor bytes the pinned index declares.
pub const DEEPSEEK_V41_FLASH_TOTAL_BYTES: u64 = 510_286_023_000;
/// Elements per FP8 scale block along each axis, and per FP4 or Engram scale.
pub const V41_SCALE_BLOCK: usize = 32;
/// Index file name inside a checkpoint directory.
pub const SAFETENSORS_INDEX_FILE: &str = "model.safetensors.index.json";

/// Largest safetensors header this reader accepts. The largest V4.1 header is
/// about 270 KB.
const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;

const PIN_JSON: &str = include_str!("testdata/deepseek_v41_weights/checkpoint_pin.json");

/// Errors from reading or checking V4.1 safetensors weights.
#[derive(Debug, Error)]
pub enum V41WeightsError {
    /// Reading a file failed.
    #[error("failed to read `{path}`: {message}")]
    Io {
        /// Path that failed.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// The index file isn't a valid safetensors index.
    #[error("invalid safetensors index: {0}")]
    InvalidIndex(String),
    /// A shard header isn't valid.
    #[error("invalid header in `{shard}`: {message}")]
    InvalidHeader {
        /// Shard file name.
        shard: String,
        /// Failure summary.
        message: String,
    },
    /// A tensor uses a dtype this reader doesn't know.
    #[error("tensor `{tensor}` in `{shard}` has unsupported dtype `{dtype}`")]
    UnknownDtype {
        /// Shard file name.
        shard: String,
        /// Tensor name.
        tensor: String,
        /// Dtype string from the header.
        dtype: String,
    },
    /// Two tensors in one shard share bytes.
    #[error("tensors `{first}` and `{second}` overlap in `{shard}`")]
    OverlappingTensors {
        /// Shard file name.
        shard: String,
        /// Earlier tensor.
        first: String,
        /// Later tensor.
        second: String,
    },
    /// A tensor's byte range ends past the end of its file.
    #[error("tensor `{tensor}` ends at byte {end} of `{shard}`, past the file end {file_len}")]
    PastFileEnd {
        /// Shard file name.
        shard: String,
        /// Tensor name.
        tensor: String,
        /// Absolute end offset.
        end: u64,
        /// File length.
        file_len: u64,
    },
    /// The index names a tensor that its shard doesn't hold.
    #[error("index maps `{tensor}` to `{shard}`, but that shard doesn't hold it")]
    MissingTensor {
        /// Shard file name.
        shard: String,
        /// Tensor name.
        tensor: String,
    },
    /// A shard holds a tensor that the index doesn't map to it.
    #[error("`{shard}` holds `{tensor}`, which the index doesn't map to it")]
    UnindexedTensor {
        /// Shard file name.
        shard: String,
        /// Tensor name.
        tensor: String,
    },
    /// The index's declared total doesn't match the shard headers.
    #[error("index declares {declared} tensor bytes, headers hold {actual}")]
    TotalSizeMismatch {
        /// `metadata.total_size` from the index.
        declared: u64,
        /// Sum of all tensor byte lengths.
        actual: u64,
    },
    /// The checkpoint doesn't match the pin.
    #[error("checkpoint doesn't match the pin: {0}")]
    PinMismatch(String),
    /// A tensor name isn't in the checkpoint.
    #[error("no tensor named `{0}`")]
    NoSuchTensor(String),
    /// A tensor has the wrong dtype or shape for the requested view.
    #[error("tensor `{tensor}` can't be read as {wanted}: {message}")]
    WrongLayout {
        /// Tensor name.
        tensor: String,
        /// Requested view.
        wanted: &'static str,
        /// What didn't match.
        message: String,
    },
    /// A row, block, or byte range lies outside the tensor.
    #[error("{what} is outside tensor `{tensor}`")]
    OutOfRange {
        /// Tensor name.
        tensor: String,
        /// Requested location.
        what: String,
    },
    /// Tensor data was requested from a checkpoint opened with headers only.
    #[error("the checkpoint was opened with headers only, so tensor data can't be read")]
    HeadersOnly,
    /// The tensor names don't match the V4.1 model map.
    #[error(
        "tensor names don't match the V4.1 model map: {missing_count} missing (first: {first_missing:?}), {extra_count} extra (first: {first_extra:?})"
    )]
    ModelMap {
        /// Number of expected tensors that are absent.
        missing_count: usize,
        /// First absent tensor.
        first_missing: Option<String>,
        /// Number of tensors the map doesn't know.
        extra_count: usize,
        /// First unknown tensor.
        first_extra: Option<String>,
    },
    /// A tensor has the wrong dtype for its role.
    #[error("tensor `{tensor}` is {actual}, but its role needs {expected}")]
    RoleDtype {
        /// Tensor name.
        tensor: String,
        /// Dtype the role needs.
        expected: SafetensorsDtype,
        /// Dtype in the header.
        actual: SafetensorsDtype,
    },
    /// An oracle manifest is invalid.
    #[error("invalid oracle parity manifest: {0}")]
    InvalidOracle(String),
}

fn io_error(path: &Path, error: impl fmt::Display) -> V41WeightsError {
    V41WeightsError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Dtypes and scalar decoding
// ---------------------------------------------------------------------------

/// The safetensors dtypes V4.1 uses. Any other dtype is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SafetensorsDtype {
    /// `F8_E4M3`: FP8 with 4 exponent and 3 mantissa bits, no infinities
    /// (`torch.float8_e4m3fn`).
    #[serde(rename = "F8_E4M3")]
    F8E4M3,
    /// `F8_E8M0`: an unsigned power-of-two scale (`torch.float8_e8m0fnu`).
    #[serde(rename = "F8_E8M0")]
    F8E8M0,
    /// `I8`: in V4.1, two packed E2M1 values per byte (`float4_e2m1fn_x2`).
    #[serde(rename = "I8")]
    I8,
    /// `BF16`.
    #[serde(rename = "BF16")]
    Bf16,
    /// `F32`.
    #[serde(rename = "F32")]
    F32,
}

impl SafetensorsDtype {
    /// Parses a header dtype string, or returns `None` for an unsupported one.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "F8_E4M3" => Some(Self::F8E4M3),
            "F8_E8M0" => Some(Self::F8E8M0),
            "I8" => Some(Self::I8),
            "BF16" => Some(Self::Bf16),
            "F32" => Some(Self::F32),
            _ => None,
        }
    }

    /// Returns the header dtype string.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::F8E4M3 => "F8_E4M3",
            Self::F8E8M0 => "F8_E8M0",
            Self::I8 => "I8",
            Self::Bf16 => "BF16",
            Self::F32 => "F32",
        }
    }

    /// Bytes per stored element.
    #[must_use]
    pub const fn element_bytes(self) -> u64 {
        match self {
            Self::F8E4M3 | Self::F8E8M0 | Self::I8 => 1,
            Self::Bf16 => 2,
            Self::F32 => 4,
        }
    }
}

impl fmt::Display for SafetensorsDtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Decodes one `float8_e4m3fn` byte: bias 7, subnormals, no infinities, and
/// NaN only for `0x7F` and `0xFF`. The largest finite value is 448.
#[must_use]
pub fn decode_f8_e4m3fn(byte: u8) -> f32 {
    let sign = if byte & 0x80 == 0 { 1.0_f32 } else { -1.0_f32 };
    let exponent = i32::from((byte >> 3) & 0x0F);
    let mantissa = f32::from(byte & 0x07);
    if exponent == 0x0F && byte & 0x07 == 0x07 {
        return f32::NAN;
    }
    if exponent == 0 {
        // Subnormal: m / 8 * 2^-6, exact in f32.
        return sign * mantissa * (2.0_f32).powi(-9);
    }
    sign * (1.0 + mantissa / 8.0) * (2.0_f32).powi(exponent - 7)
}

/// Decodes one `float8_e8m0fnu` byte: `2^(e - 127)`, with `0xFF` as NaN. Byte
/// `0x00` is `2^-127`, a subnormal f32 that is still exact.
#[must_use]
pub fn decode_f8_e8m0fnu(byte: u8) -> f32 {
    match byte {
        0xFF => f32::NAN,
        0 => f32::from_bits(0x0040_0000),
        exponent => f32::from_bits(u32::from(exponent) << 23),
    }
}

/// The E2M1 magnitudes for codes 0 through 7 (`convert.py` line 13).
pub const E2M1_MAGNITUDES: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

/// Decodes one E2M1 code (the low four bits of `code`). Bit 3 is the sign, so
/// code 8 is negative zero, as `float4_e2m1fn` defines it.
#[must_use]
pub fn decode_e2m1(code: u8) -> f32 {
    let magnitude = E2M1_MAGNITUDES[usize::from(code & 0x07)];
    if code & 0x08 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Splits a packed `float4_e2m1fn_x2` byte into `(element 2i, element 2i + 1)`.
/// The low nibble holds the even element (`convert.py` lines 32–34).
#[must_use]
pub const fn unpack_e2m1_pair(byte: u8) -> (u8, u8) {
    (byte & 0x0F, byte >> 4)
}

/// Decodes one little-endian BF16 value.
#[must_use]
pub fn decode_bf16(bytes: [u8; 2]) -> f32 {
    f32::from_bits(u32::from(u16::from_le_bytes(bytes)) << 16)
}

/// Decodes one little-endian F32 value.
#[must_use]
pub fn decode_f32(bytes: [u8; 4]) -> f32 {
    f32::from_le_bytes(bytes)
}

/// Dequantizes FP8 E4M3 values with one UE8M0 scale per 32 values:
/// `out[i] = e4m3(values[i]) * e8m0(scales[i / 32])`. This is the Engram row
/// rule (`model.py` lines 317–320), and also one row of a 32×32-block FP8
/// weight when `scales` is that row's block-row of the scale tensor.
///
/// # Errors
///
/// Returns an error when `scales` is too short for `values`.
pub fn dequantize_fp8_per32(values: &[u8], scales: &[u8]) -> Result<Vec<f32>, V41WeightsError> {
    let needed = values.len().div_ceil(V41_SCALE_BLOCK);
    if scales.len() < needed {
        return Err(V41WeightsError::WrongLayout {
            tensor: String::from("<slice>"),
            wanted: "FP8 values with per-32 scales",
            message: format!(
                "{} values need {needed} scales, got {}",
                values.len(),
                scales.len()
            ),
        });
    }
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, &byte)| {
            decode_f8_e4m3fn(byte) * decode_f8_e8m0fnu(scales[index / V41_SCALE_BLOCK])
        })
        .collect())
}

/// Dequantizes one packed FP4 row with one UE8M0 scale per 32 logical values
/// along K. `packed` holds `K / 2` bytes; the result holds `K` values, with
/// element `2i` from the low nibble of byte `i`.
///
/// # Errors
///
/// Returns an error when `scales` is too short for the row.
pub fn dequantize_fp4_row(packed: &[u8], scales: &[u8]) -> Result<Vec<f32>, V41WeightsError> {
    let k = packed.len() * 2;
    let needed = k.div_ceil(V41_SCALE_BLOCK);
    if scales.len() < needed {
        return Err(V41WeightsError::WrongLayout {
            tensor: String::from("<slice>"),
            wanted: "packed FP4 with per-32 scales",
            message: format!("{k} values need {needed} scales, got {}", scales.len()),
        });
    }
    let mut out = Vec::with_capacity(k);
    for (index, &byte) in packed.iter().enumerate() {
        let (even, odd) = unpack_e2m1_pair(byte);
        // Both elements of one byte share a 32-element group, since 32 is even.
        let scale = decode_f8_e8m0fnu(scales[(2 * index) / V41_SCALE_BLOCK]);
        out.push(decode_e2m1(even) * scale);
        out.push(decode_e2m1(odd) * scale);
    }
    Ok(out)
}

/// How a V4.1 weight is quantized. This is separate from
/// `psionic_core::QuantizationMode`; see the module documentation's MXFP4
/// section for why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V41BlockFormat {
    /// `F8_E4M3` `[rows, cols]` with an `F8_E8M0` scale of
    /// `[ceil(rows / 32), ceil(cols / 32)]`: one scale per 32×32 block.
    Fp8Block32x32,
    /// `F8_E4M3` `[rows, cols]` with an `F8_E8M0` scale of `[rows, cols / 32]`:
    /// one scale per 32 values of a row (the Engram tables).
    Fp8Row32,
    /// `I8` `[rows, K / 2]` of packed E2M1 pairs with an `F8_E8M0` scale of
    /// `[rows, K / 32]` (the routed experts).
    Fp4E2M1Row32,
    /// Plain `BF16`.
    Bf16,
    /// Plain `F32`.
    F32,
}

impl V41BlockFormat {
    /// Whether the stored bytes match GGML MXFP4 blocks. Always false: the
    /// nibble order and the scale placement differ.
    #[must_use]
    pub const fn ggml_mxfp4_byte_compatible(self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Index and headers
// ---------------------------------------------------------------------------

/// A parsed `model.safetensors.index.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafetensorsIndex {
    /// `metadata.total_size`, when present.
    pub total_size: Option<u64>,
    /// Tensor name to shard file name.
    pub weight_map: BTreeMap<String, String>,
    /// SHA-256 of the index bytes, lowercase hex.
    pub sha256: String,
}

#[derive(Deserialize)]
struct RawIndex {
    #[serde(default)]
    metadata: Option<RawIndexMetadata>,
    weight_map: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawIndexMetadata {
    #[serde(default)]
    total_size: Option<u64>,
}

impl SafetensorsIndex {
    /// Parses index bytes.
    ///
    /// # Errors
    ///
    /// Refuses invalid JSON, an empty map, and shard names that aren't plain
    /// file names.
    pub fn parse(bytes: &[u8]) -> Result<Self, V41WeightsError> {
        let raw: RawIndex = serde_json::from_slice(bytes)
            .map_err(|error| V41WeightsError::InvalidIndex(error.to_string()))?;
        if raw.weight_map.is_empty() {
            return Err(V41WeightsError::InvalidIndex(String::from(
                "weight_map is empty",
            )));
        }
        for shard in raw.weight_map.values() {
            if !is_plain_file_name(shard) {
                return Err(V41WeightsError::InvalidIndex(format!(
                    "shard name `{shard}` isn't a plain file name"
                )));
            }
        }
        Ok(Self {
            total_size: raw.metadata.and_then(|metadata| metadata.total_size),
            weight_map: raw.weight_map,
            sha256: hex::encode(Sha256::digest(bytes)),
        })
    }

    /// Returns the distinct shard file names, sorted.
    #[must_use]
    pub fn shard_names(&self) -> BTreeSet<&str> {
        self.weight_map.values().map(String::as_str).collect()
    }
}

fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

/// One tensor's entry in a shard header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShardTensorEntry {
    /// Tensor name.
    pub name: String,
    /// Stored dtype.
    pub dtype: SafetensorsDtype,
    /// Stored shape. Packed FP4 tensors keep their stored `[rows, K / 2]`.
    pub shape: Vec<u64>,
    /// Absolute byte offset of the first byte in the shard file.
    pub file_offset: u64,
    /// Byte length.
    pub byte_len: u64,
}

impl ShardTensorEntry {
    /// Absolute byte range in the shard file.
    #[must_use]
    pub fn file_range(&self) -> Range<u64> {
        self.file_offset..self.file_offset + self.byte_len
    }

    fn rows_cols(&self) -> Option<(u64, u64)> {
        match self.shape.as_slice() {
            [rows, cols] => Some((*rows, *cols)),
            _ => None,
        }
    }
}

/// A parsed shard header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafetensorsShardHeader {
    /// Shard file name.
    pub shard: String,
    /// Length of the JSON header, from the first eight bytes.
    pub header_len: u64,
    /// SHA-256 of the eight length bytes and the JSON header.
    pub header_sha256: String,
    /// The `__metadata__` map, when present.
    pub metadata: BTreeMap<String, String>,
    /// Tensors by name.
    pub tensors: BTreeMap<String, ShardTensorEntry>,
}

#[derive(Deserialize)]
struct RawTensorEntry {
    dtype: String,
    shape: Vec<u64>,
    data_offsets: [u64; 2],
}

impl SafetensorsShardHeader {
    /// Parses a header from the first bytes of a shard. `prefix` must hold at
    /// least the eight length bytes and the JSON header; later bytes are
    /// ignored. When `file_len` is known, every tensor must end inside it.
    ///
    /// # Errors
    ///
    /// Refuses a truncated or invalid header, unsupported dtypes, a byte
    /// length that doesn't match the shape, overlapping tensors, and ranges
    /// past the file end.
    pub fn parse(
        shard: &str,
        prefix: &[u8],
        file_len: Option<u64>,
    ) -> Result<Self, V41WeightsError> {
        let invalid = |message: String| V41WeightsError::InvalidHeader {
            shard: shard.to_owned(),
            message,
        };
        let Some(length_bytes) = prefix.get(..8) else {
            return Err(invalid(String::from(
                "file is shorter than the 8-byte header length",
            )));
        };
        let mut length = [0_u8; 8];
        length.copy_from_slice(length_bytes);
        let header_len = u64::from_le_bytes(length);
        if header_len == 0 || header_len > MAX_HEADER_BYTES {
            return Err(invalid(format!(
                "header length {header_len} is out of bounds"
            )));
        }
        let header_end = 8 + header_len;
        if let Some(file_len) = file_len {
            if header_end > file_len {
                return Err(invalid(format!(
                    "header ends at {header_end}, past the file end {file_len}"
                )));
            }
        }
        let Some(json) = usize::try_from(header_end)
            .ok()
            .and_then(|end| prefix.get(8..end))
        else {
            return Err(invalid(format!(
                "header needs {header_end} bytes, only {} available",
                prefix.len()
            )));
        };
        let raw: BTreeMap<String, serde_json::Value> = serde_json::from_slice(json)
            .map_err(|error| invalid(format!("header JSON: {error}")))?;
        let mut metadata = BTreeMap::new();
        let mut tensors = BTreeMap::new();
        for (name, value) in raw {
            if name == "__metadata__" {
                metadata = serde_json::from_value(value)
                    .map_err(|error| invalid(format!("__metadata__: {error}")))?;
                continue;
            }
            let entry: RawTensorEntry = serde_json::from_value(value)
                .map_err(|error| invalid(format!("tensor `{name}`: {error}")))?;
            let Some(dtype) = SafetensorsDtype::parse(&entry.dtype) else {
                return Err(V41WeightsError::UnknownDtype {
                    shard: shard.to_owned(),
                    tensor: name,
                    dtype: entry.dtype,
                });
            };
            let [start, end] = entry.data_offsets;
            if start > end {
                return Err(invalid(format!(
                    "tensor `{name}` has start {start} > end {end}"
                )));
            }
            let elements = entry
                .shape
                .iter()
                .try_fold(1_u64, |acc, &dim| acc.checked_mul(dim));
            let expected = elements.and_then(|count| count.checked_mul(dtype.element_bytes()));
            if expected != Some(end - start) {
                return Err(invalid(format!(
                    "tensor `{name}` holds {} bytes, but {dtype} {:?} needs {expected:?}",
                    end - start,
                    entry.shape
                )));
            }
            let file_offset = header_end
                .checked_add(start)
                .ok_or_else(|| invalid(format!("tensor `{name}` offset overflows")))?;
            tensors.insert(
                name.clone(),
                ShardTensorEntry {
                    name,
                    dtype,
                    shape: entry.shape,
                    file_offset,
                    byte_len: end - start,
                },
            );
        }
        let mut by_offset: Vec<&ShardTensorEntry> = tensors.values().collect();
        by_offset.sort_by_key(|entry| (entry.file_offset, entry.byte_len));
        for pair in by_offset.windows(2) {
            if pair[0].file_range().end > pair[1].file_offset {
                return Err(V41WeightsError::OverlappingTensors {
                    shard: shard.to_owned(),
                    first: pair[0].name.clone(),
                    second: pair[1].name.clone(),
                });
            }
        }
        if let Some(file_len) = file_len {
            if let Some(last) = by_offset.last() {
                let end = last.file_range().end;
                if end > file_len {
                    return Err(V41WeightsError::PastFileEnd {
                        shard: shard.to_owned(),
                        tensor: last.name.clone(),
                        end,
                        file_len,
                    });
                }
            }
        }
        Ok(Self {
            shard: shard.to_owned(),
            header_len,
            header_sha256: hex::encode(Sha256::digest(
                &prefix[..usize::try_from(header_end).unwrap_or(prefix.len())],
            )),
            metadata,
            tensors,
        })
    }

    /// Reads and parses the header of a shard on disk, reading only the header
    /// bytes. When `check_file_len` is false (a header-only copy of a shard),
    /// ranges are checked against `expected_len` instead, when given.
    ///
    /// # Errors
    ///
    /// Returns read errors and every refusal [`Self::parse`] makes.
    pub fn read_path(
        path: &Path,
        check_file_len: bool,
        expected_len: Option<u64>,
    ) -> Result<Self, V41WeightsError> {
        let shard = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut file = File::open(path).map_err(|error| io_error(path, error))?;
        let actual_len = file
            .metadata()
            .map_err(|error| io_error(path, error))?
            .len();
        let mut length = [0_u8; 8];
        file.read_exact(&mut length)
            .map_err(|_| V41WeightsError::InvalidHeader {
                shard: shard.clone(),
                message: String::from("file is shorter than the 8-byte header length"),
            })?;
        let header_len = u64::from_le_bytes(length);
        if header_len == 0 || header_len > MAX_HEADER_BYTES {
            return Err(V41WeightsError::InvalidHeader {
                shard,
                message: format!("header length {header_len} is out of bounds"),
            });
        }
        let mut prefix = vec![0_u8; 8 + header_len as usize];
        prefix[..8].copy_from_slice(&length);
        file.read_exact(&mut prefix[8..])
            .map_err(|_| V41WeightsError::InvalidHeader {
                shard: shard.clone(),
                message: format!("header of {header_len} bytes is truncated"),
            })?;
        let file_len = if check_file_len {
            Some(actual_len)
        } else {
            expected_len
        };
        Self::parse(&shard, &prefix, file_len)
    }

    /// Sum of all tensor byte lengths.
    #[must_use]
    pub fn tensor_bytes(&self) -> u64 {
        self.tensors.values().map(|entry| entry.byte_len).sum()
    }
}

// ---------------------------------------------------------------------------
// Pin and identity
// ---------------------------------------------------------------------------

/// The pinned revision: the index hash and every shard's size and SHA-256, as
/// the Hugging Face tree API lists them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPin {
    /// Schema label.
    pub schema: String,
    /// Hugging Face repository.
    pub repo: String,
    /// Hugging Face revision.
    pub revision: String,
    /// Index file name.
    pub index_file: String,
    /// Index byte length.
    pub index_size: u64,
    /// Index SHA-256, lowercase hex.
    pub index_sha256: String,
    /// Shard file name to size and SHA-256.
    pub shards: BTreeMap<String, PinnedShard>,
}

/// One pinned shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedShard {
    /// File size in bytes.
    pub size: u64,
    /// File SHA-256, lowercase hex.
    pub sha256: String,
}

impl CheckpointPin {
    /// The pin for `deepseek-ai/DeepSeek-V4.1-Flash-1017` at
    /// [`DEEPSEEK_V41_FLASH_REVISION`].
    ///
    /// # Errors
    ///
    /// Fails only if the embedded pin file is corrupt.
    pub fn deepseek_v41_flash() -> Result<Self, V41WeightsError> {
        serde_json::from_str(PIN_JSON)
            .map_err(|error| V41WeightsError::PinMismatch(format!("embedded pin: {error}")))
    }
}

/// A checkpoint's identity: revision, index hash, and each shard's hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointIdentity {
    /// Hugging Face repository.
    pub repo: String,
    /// Hugging Face revision.
    pub revision: String,
    /// Index SHA-256.
    pub index_sha256: String,
    /// Shard file name to its identity.
    pub shards: BTreeMap<String, ShardIdentity>,
}

/// One shard's identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardIdentity {
    /// File size in bytes.
    pub size: u64,
    /// Expected SHA-256 from the pin.
    pub sha256: String,
    /// Whether the file's SHA-256 was computed and matched in this process.
    pub verified: bool,
}

impl CheckpointIdentity {
    /// One digest over the revision, the index hash, and each shard's hash.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.repo.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.revision.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.index_sha256.as_bytes());
        hasher.update(b"\n");
        for (name, shard) in &self.shards {
            hasher.update(format!("{name} {} {}\n", shard.size, shard.sha256).as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Streams a file through SHA-256 in 8 MiB chunks.
///
/// # Errors
///
/// Returns read errors.
pub fn sha256_file(path: &Path) -> Result<String, V41WeightsError> {
    let mut file = File::open(path).map_err(|error| io_error(path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 8 * 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

// ---------------------------------------------------------------------------
// Checkpoint
// ---------------------------------------------------------------------------

/// How to open a checkpoint directory.
#[derive(Clone, Debug)]
pub struct CheckpointOpenOptions {
    /// Read only the headers. Shard files may be header-only copies; tensor
    /// data reads are refused. Ranges are checked against the pinned sizes.
    pub headers_only: bool,
    /// The pin the checkpoint must match. `None` accepts any revision and
    /// checks only internal consistency.
    pub pin: Option<CheckpointPin>,
    /// Check every tensor against the V4.1 model map.
    pub check_model_map: bool,
}

impl CheckpointOpenOptions {
    /// Full shards, the pinned Flash revision, and the model map check.
    ///
    /// # Errors
    ///
    /// Fails only if the embedded pin is corrupt.
    pub fn pinned_flash() -> Result<Self, V41WeightsError> {
        Ok(Self {
            headers_only: false,
            pin: Some(CheckpointPin::deepseek_v41_flash()?),
            check_model_map: true,
        })
    }

    /// Same as [`Self::pinned_flash`], with headers only.
    ///
    /// # Errors
    ///
    /// Fails only if the embedded pin is corrupt.
    pub fn pinned_flash_headers_only() -> Result<Self, V41WeightsError> {
        Ok(Self {
            headers_only: true,
            ..Self::pinned_flash()?
        })
    }
}

#[derive(Debug)]
struct ShardFile {
    header: SafetensorsShardHeader,
    path: PathBuf,
    file: Option<Arc<File>>,
}

/// An opened V4.1 checkpoint directory.
#[derive(Debug)]
pub struct DeepSeekV41Checkpoint {
    root: PathBuf,
    index: SafetensorsIndex,
    shards: BTreeMap<String, ShardFile>,
    pin: Option<CheckpointPin>,
    headers_only: bool,
    roles: Option<V41ModelMap>,
}

impl DeepSeekV41Checkpoint {
    /// Opens a checkpoint directory: parses the index and every shard header
    /// and cross-checks them. Reads no tensor data.
    ///
    /// # Errors
    ///
    /// Refuses everything [`SafetensorsShardHeader::parse`] refuses, a tensor
    /// the index maps to a shard that doesn't hold it, a shard tensor the index
    /// doesn't map, a total that doesn't match, a pin mismatch, and, when
    /// requested, a model map mismatch.
    pub fn open(root: &Path, options: CheckpointOpenOptions) -> Result<Self, V41WeightsError> {
        let index_path = root.join(SAFETENSORS_INDEX_FILE);
        let index_bytes =
            std::fs::read(&index_path).map_err(|error| io_error(&index_path, error))?;
        let index = SafetensorsIndex::parse(&index_bytes)?;
        if let Some(pin) = &options.pin {
            if index.sha256 != pin.index_sha256 {
                return Err(V41WeightsError::PinMismatch(format!(
                    "index SHA-256 is {}, pin wants {}",
                    index.sha256, pin.index_sha256
                )));
            }
            let named: BTreeSet<&str> = pin.shards.keys().map(String::as_str).collect();
            if named != index.shard_names() {
                return Err(V41WeightsError::PinMismatch(String::from(
                    "the index names different shards than the pin",
                )));
            }
        }
        let mut shards = BTreeMap::new();
        for shard in index.shard_names() {
            let path = root.join(shard);
            let pinned = options
                .pin
                .as_ref()
                .and_then(|pin| pin.shards.get(shard))
                .map(|pinned| pinned.size);
            let header = SafetensorsShardHeader::read_path(&path, !options.headers_only, pinned)?;
            let file = if options.headers_only {
                None
            } else {
                let file = File::open(&path).map_err(|error| io_error(&path, error))?;
                let len = file
                    .metadata()
                    .map_err(|error| io_error(&path, error))?
                    .len();
                if let Some(pinned) = pinned {
                    if len != pinned {
                        return Err(V41WeightsError::PinMismatch(format!(
                            "`{shard}` is {len} bytes, pin wants {pinned}"
                        )));
                    }
                }
                Some(Arc::new(file))
            };
            shards.insert(shard.to_owned(), ShardFile { header, path, file });
        }
        for (tensor, shard) in &index.weight_map {
            let holds = shards
                .get(shard)
                .is_some_and(|file| file.header.tensors.contains_key(tensor));
            if !holds {
                return Err(V41WeightsError::MissingTensor {
                    shard: shard.clone(),
                    tensor: tensor.clone(),
                });
            }
        }
        let mut actual = 0_u64;
        for (shard, file) in &shards {
            for tensor in file.header.tensors.keys() {
                if index.weight_map.get(tensor) != Some(shard) {
                    return Err(V41WeightsError::UnindexedTensor {
                        shard: shard.clone(),
                        tensor: tensor.clone(),
                    });
                }
            }
            actual += file.header.tensor_bytes();
        }
        if let Some(declared) = index.total_size {
            if declared != actual {
                return Err(V41WeightsError::TotalSizeMismatch { declared, actual });
            }
        }
        let mut checkpoint = Self {
            root: root.to_path_buf(),
            index,
            shards,
            pin: options.pin,
            headers_only: options.headers_only,
            roles: None,
        };
        if options.check_model_map {
            let map = V41ModelMap::new(&V41WeightConfig::flash());
            map.check_names(checkpoint.index.weight_map.keys().map(String::as_str))?;
            for name in checkpoint.index.weight_map.keys() {
                let entry = checkpoint.entry(name)?;
                let role = map.role(name)?;
                if entry.dtype != role.dtype {
                    return Err(V41WeightsError::RoleDtype {
                        tensor: name.clone(),
                        expected: role.dtype,
                        actual: entry.dtype,
                    });
                }
            }
            checkpoint.roles = Some(map);
        }
        Ok(checkpoint)
    }

    /// The checkpoint directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The parsed index.
    #[must_use]
    pub fn index(&self) -> &SafetensorsIndex {
        &self.index
    }

    /// Whether the checkpoint was opened with headers only.
    #[must_use]
    pub const fn headers_only(&self) -> bool {
        self.headers_only
    }

    /// The parsed header of one shard.
    #[must_use]
    pub fn shard_header(&self, shard: &str) -> Option<&SafetensorsShardHeader> {
        self.shards.get(shard).map(|file| &file.header)
    }

    /// The model map, when the checkpoint was opened with the map check.
    #[must_use]
    pub fn model_map(&self) -> Option<&V41ModelMap> {
        self.roles.as_ref()
    }

    /// The identity from the pin, with no shard verified yet. Without a pin,
    /// the shard hashes are empty strings.
    #[must_use]
    pub fn identity(&self) -> CheckpointIdentity {
        let shards = self
            .shards
            .keys()
            .map(|name| {
                let pinned = self.pin.as_ref().and_then(|pin| pin.shards.get(name));
                (
                    name.clone(),
                    ShardIdentity {
                        size: pinned.map_or(0, |pinned| pinned.size),
                        sha256: pinned
                            .map(|pinned| pinned.sha256.clone())
                            .unwrap_or_default(),
                        verified: false,
                    },
                )
            })
            .collect();
        CheckpointIdentity {
            repo: self
                .pin
                .as_ref()
                .map_or_else(String::new, |pin| pin.repo.clone()),
            revision: self
                .pin
                .as_ref()
                .map_or_else(String::new, |pin| pin.revision.clone()),
            index_sha256: self.index.sha256.clone(),
            shards,
        }
    }

    /// Hashes every shard file (or only `only`, when given) and checks it
    /// against the pin. This reads every byte, about 510 GB for the full
    /// checkpoint, so it is a separate step from [`Self::open`].
    ///
    /// # Errors
    ///
    /// Refuses headers-only checkpoints, read errors, and hash mismatches.
    pub fn verify_shard_hashes(
        &self,
        only: Option<&BTreeSet<String>>,
    ) -> Result<CheckpointIdentity, V41WeightsError> {
        if self.headers_only {
            return Err(V41WeightsError::HeadersOnly);
        }
        let mut identity = self.identity();
        for (name, file) in &self.shards {
            if only.is_some_and(|only| !only.contains(name)) {
                continue;
            }
            let actual = sha256_file(&file.path)?;
            let shard = identity
                .shards
                .get_mut(name)
                .ok_or_else(|| V41WeightsError::NoSuchTensor(name.clone()))?;
            if self.pin.is_some() && actual != shard.sha256 {
                return Err(V41WeightsError::PinMismatch(format!(
                    "`{name}` hashes to {actual}, pin wants {}",
                    shard.sha256
                )));
            }
            shard.sha256 = actual;
            shard.verified = true;
        }
        Ok(identity)
    }

    /// The header entry for one tensor.
    ///
    /// # Errors
    ///
    /// Returns [`V41WeightsError::NoSuchTensor`] for an unknown name.
    pub fn entry(&self, name: &str) -> Result<&ShardTensorEntry, V41WeightsError> {
        self.index
            .weight_map
            .get(name)
            .and_then(|shard| self.shards.get(shard))
            .and_then(|file| file.header.tensors.get(name))
            .ok_or_else(|| V41WeightsError::NoSuchTensor(name.to_owned()))
    }

    /// The shard file name that holds a tensor.
    #[must_use]
    pub fn shard_of(&self, name: &str) -> Option<&str> {
        self.index.weight_map.get(name).map(String::as_str)
    }

    /// Reads `out.len()` bytes of a tensor, starting `offset` bytes into it,
    /// with one positioned read.
    ///
    /// # Errors
    ///
    /// Refuses headers-only checkpoints, ranges outside the tensor, and read
    /// errors.
    pub fn read_tensor_bytes(
        &self,
        name: &str,
        offset: u64,
        out: &mut [u8],
    ) -> Result<(), V41WeightsError> {
        let entry = self.entry(name)?;
        let end = offset.checked_add(out.len() as u64);
        if end.is_none_or(|end| end > entry.byte_len) {
            return Err(V41WeightsError::OutOfRange {
                tensor: name.to_owned(),
                what: format!("bytes {offset}..+{}", out.len()),
            });
        }
        let shard = self
            .index
            .weight_map
            .get(name)
            .and_then(|shard| self.shards.get(shard))
            .ok_or_else(|| V41WeightsError::NoSuchTensor(name.to_owned()))?;
        let Some(file) = &shard.file else {
            return Err(V41WeightsError::HeadersOnly);
        };
        read_at(file, &shard.path, entry.file_offset + offset, out)
    }

    /// Reads a whole tensor. Use the row and block readers for large tensors.
    ///
    /// # Errors
    ///
    /// Same as [`Self::read_tensor_bytes`].
    pub fn read_tensor(&self, name: &str) -> Result<Vec<u8>, V41WeightsError> {
        let len = usize::try_from(self.entry(name)?.byte_len).map_err(|_| {
            V41WeightsError::OutOfRange {
                tensor: name.to_owned(),
                what: String::from("the whole tensor"),
            }
        })?;
        let mut out = vec![0_u8; len];
        self.read_tensor_bytes(name, 0, &mut out)?;
        Ok(out)
    }

    /// The block format of a weight, from its dtype and its scale's shape.
    ///
    /// # Errors
    ///
    /// Refuses a quantized weight whose scale is missing or has the wrong
    /// shape.
    pub fn block_format(&self, weight: &str) -> Result<V41BlockFormat, V41WeightsError> {
        let entry = self.entry(weight)?;
        match entry.dtype {
            SafetensorsDtype::Bf16 => return Ok(V41BlockFormat::Bf16),
            SafetensorsDtype::F32 => return Ok(V41BlockFormat::F32),
            SafetensorsDtype::F8E8M0 => {
                return Err(layout_error(weight, "a weight", "it is a scale tensor"));
            }
            SafetensorsDtype::F8E4M3 | SafetensorsDtype::I8 => {}
        }
        let (rows, cols) = entry
            .rows_cols()
            .ok_or_else(|| layout_error(weight, "a weight", "it isn't two-dimensional"))?;
        let scale_name = scale_name_for(weight)
            .ok_or_else(|| layout_error(weight, "a weight", "its name doesn't end in `weight`"))?;
        let scale = self.entry(&scale_name)?;
        if scale.dtype != SafetensorsDtype::F8E8M0 {
            return Err(layout_error(weight, "a weight", "its scale isn't F8_E8M0"));
        }
        let scale_shape = scale
            .rows_cols()
            .ok_or_else(|| layout_error(weight, "a weight", "its scale isn't two-dimensional"))?;
        let block = V41_SCALE_BLOCK as u64;
        match entry.dtype {
            SafetensorsDtype::I8 => {
                let k = cols * 2;
                if k % block == 0 && scale_shape == (rows, k / block) {
                    Ok(V41BlockFormat::Fp4E2M1Row32)
                } else {
                    Err(layout_error(
                        weight,
                        "packed FP4",
                        &format!("scale is {scale_shape:?}, wants ({rows}, {})", k / block),
                    ))
                }
            }
            _ => {
                let is_table = weight.ends_with(".engram.embed.weight");
                if is_table && cols % block == 0 && scale_shape == (rows, cols / block) {
                    Ok(V41BlockFormat::Fp8Row32)
                } else if !is_table && scale_shape == (rows.div_ceil(block), cols.div_ceil(block)) {
                    Ok(V41BlockFormat::Fp8Block32x32)
                } else {
                    Err(layout_error(
                        weight,
                        "FP8",
                        &format!("scale shape {scale_shape:?} doesn't fit ({rows}, {cols})"),
                    ))
                }
            }
        }
    }

    /// Dequantizes one 32×32 block (smaller at the bottom and right edges) of
    /// an FP8 block-scaled weight, row-major. Reads only the block's bytes and
    /// its one scale.
    ///
    /// # Errors
    ///
    /// Refuses other formats, blocks outside the weight, and read errors.
    pub fn fp8_weight_block(
        &self,
        weight: &str,
        block_row: u64,
        block_col: u64,
    ) -> Result<DequantizedBlock, V41WeightsError> {
        if self.block_format(weight)? != V41BlockFormat::Fp8Block32x32 {
            return Err(layout_error(weight, "an FP8 32x32 block", "other format"));
        }
        let entry = self.entry(weight)?;
        let (rows, cols) = entry.rows_cols().unwrap_or_default();
        let block = V41_SCALE_BLOCK as u64;
        let row_start = block_row * block;
        let col_start = block_col * block;
        if row_start >= rows || col_start >= cols {
            return Err(V41WeightsError::OutOfRange {
                tensor: weight.to_owned(),
                what: format!("block ({block_row}, {block_col})"),
            });
        }
        let height = block.min(rows - row_start);
        let width = block.min(cols - col_start);
        let scale_name = scale_name_for(weight).unwrap_or_default();
        let scale_cols = cols.div_ceil(block);
        let mut scale = [0_u8; 1];
        self.read_tensor_bytes(&scale_name, block_row * scale_cols + block_col, &mut scale)?;
        let factor = decode_f8_e8m0fnu(scale[0]);
        let mut values = Vec::with_capacity((height * width) as usize);
        let mut row_bytes = vec![0_u8; width as usize];
        for row in row_start..row_start + height {
            self.read_tensor_bytes(weight, row * cols + col_start, &mut row_bytes)?;
            values.extend(
                row_bytes
                    .iter()
                    .map(|&byte| decode_f8_e4m3fn(byte) * factor),
            );
        }
        Ok(DequantizedBlock {
            rows: height as usize,
            cols: width as usize,
            values,
        })
    }

    /// Dequantizes one full row of any V4.1 weight: an FP8 block-scaled row
    /// with its block-row of scales, a packed FP4 row with its per-32 scales,
    /// an Engram table row with its per-32 scales, or a BF16 or F32 row.
    ///
    /// # Errors
    ///
    /// Refuses rows outside the weight, layouts that don't fit, and read
    /// errors.
    pub fn dequantize_row(&self, weight: &str, row: u64) -> Result<Vec<f32>, V41WeightsError> {
        let format = self.block_format(weight)?;
        let entry = self.entry(weight)?;
        let (rows, cols) = match entry.shape.as_slice() {
            [len] => (1, *len),
            [rows, cols] => (*rows, *cols),
            _ => return Err(layout_error(weight, "rows", "more than two dimensions")),
        };
        if row >= rows {
            return Err(V41WeightsError::OutOfRange {
                tensor: weight.to_owned(),
                what: format!("row {row}"),
            });
        }
        let row_bytes = cols * entry.dtype.element_bytes();
        let mut bytes = vec![0_u8; row_bytes as usize];
        self.read_tensor_bytes(weight, row * row_bytes, &mut bytes)?;
        let block = V41_SCALE_BLOCK as u64;
        let scale_name = scale_name_for(weight).unwrap_or_default();
        match format {
            V41BlockFormat::Bf16 => Ok(bytes
                .chunks_exact(2)
                .map(|pair| decode_bf16([pair[0], pair[1]]))
                .collect()),
            V41BlockFormat::F32 => Ok(bytes
                .chunks_exact(4)
                .map(|quad| decode_f32([quad[0], quad[1], quad[2], quad[3]]))
                .collect()),
            V41BlockFormat::Fp8Block32x32 => {
                let scale_cols = cols.div_ceil(block);
                let mut scales = vec![0_u8; scale_cols as usize];
                self.read_tensor_bytes(&scale_name, (row / block) * scale_cols, &mut scales)?;
                dequantize_fp8_per32(&bytes, &scales)
            }
            V41BlockFormat::Fp8Row32 => {
                let scale_cols = cols / block;
                let mut scales = vec![0_u8; scale_cols as usize];
                self.read_tensor_bytes(&scale_name, row * scale_cols, &mut scales)?;
                dequantize_fp8_per32(&bytes, &scales)
            }
            V41BlockFormat::Fp4E2M1Row32 => {
                let scale_cols = cols * 2 / block;
                let mut scales = vec![0_u8; scale_cols as usize];
                self.read_tensor_bytes(&scale_name, row * scale_cols, &mut scales)?;
                dequantize_fp4_row(&bytes, &scales)
            }
        }
    }

    /// Dequantizes one row of the Engram table at `layer` (1 or 14).
    ///
    /// # Errors
    ///
    /// Same as [`Self::dequantize_row`].
    pub fn engram_row(&self, layer: u32, row: u64) -> Result<Vec<f32>, V41WeightsError> {
        self.dequantize_row(&format!("layers.{layer}.engram.embed.weight"), row)
    }

    /// Lists the tensors one stage needs, with byte totals.
    ///
    /// # Errors
    ///
    /// Refuses a tensor name the model map doesn't know.
    pub fn select(
        &self,
        selection: &V41StageSelection,
    ) -> Result<V41StageTensors, V41WeightsError> {
        let fallback;
        let map = if let Some(map) = &self.roles {
            map
        } else {
            fallback = V41ModelMap::new(&V41WeightConfig::flash());
            &fallback
        };
        let mut stage = V41StageTensors::default();
        for (name, shard) in &self.index.weight_map {
            let role = map.role(name)?;
            if !selection.includes(role) {
                continue;
            }
            let entry = self.entry(name)?;
            stage.add(name, shard, entry, role);
        }
        Ok(stage)
    }
}

fn layout_error(tensor: &str, wanted: &'static str, message: &str) -> V41WeightsError {
    V41WeightsError::WrongLayout {
        tensor: tensor.to_owned(),
        wanted,
        message: message.to_owned(),
    }
}

/// The scale tensor name for a quantized weight: `X.weight` becomes `X.scale`.
#[must_use]
pub fn scale_name_for(weight: &str) -> Option<String> {
    weight
        .strip_suffix("weight")
        .map(|stem| format!("{stem}scale"))
}

#[cfg(unix)]
fn read_at(file: &File, path: &Path, offset: u64, out: &mut [u8]) -> Result<(), V41WeightsError> {
    use std::os::unix::fs::FileExt as _;
    file.read_exact_at(out, offset)
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn read_at(_file: &File, path: &Path, offset: u64, out: &mut [u8]) -> Result<(), V41WeightsError> {
    use std::io::{Seek as _, SeekFrom};
    let mut file = File::open(path).map_err(|error| io_error(path, error))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| io_error(path, error))?;
    file.read_exact(out).map_err(|error| io_error(path, error))
}

/// A dequantized block, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct DequantizedBlock {
    /// Rows in the block (32, or fewer at the bottom edge).
    pub rows: usize,
    /// Columns in the block (32, or fewer at the right edge).
    pub cols: usize,
    /// `rows * cols` values.
    pub values: Vec<f32>,
}

// ---------------------------------------------------------------------------
// Model map
// ---------------------------------------------------------------------------

/// The V4.1 shape facts that decide which tensors exist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41WeightConfig {
    /// Decoder layers.
    pub n_layers: u32,
    /// MTP (DSpark) layers.
    pub n_mtp_layers: u32,
    /// Routed experts per decoder layer.
    pub n_routed_experts: u32,
    /// Routed experts per MTP layer.
    pub n_mtp_routed_experts: u32,
    /// Compression ratio per layer, decoder layers first.
    pub compress_ratios: Vec<u32>,
    /// Layers that own a compressor and indexer keys.
    pub kv_source_layers: Vec<u32>,
    /// Layers that own indexer queries.
    pub index_source_layers: Vec<u32>,
    /// Layers with an Engram table.
    pub engram_layers: Vec<u32>,
    /// Vision encoder blocks.
    pub vision_layers: u32,
}

impl V41WeightConfig {
    /// The Flash checkpoint's values (`inference/config.json`).
    #[must_use]
    pub fn flash() -> Self {
        let mut compress_ratios = vec![0, 0];
        compress_ratios.extend(std::iter::repeat_n(2, 18));
        compress_ratios.extend(std::iter::repeat_n(1, 20));
        compress_ratios.extend([0, 0, 0]);
        Self {
            n_layers: 40,
            n_mtp_layers: 3,
            n_routed_experts: 384,
            n_mtp_routed_experts: 128,
            compress_ratios,
            kv_source_layers: vec![2, 8, 14, 20],
            index_source_layers: vec![2, 8, 14, 20, 24, 28, 32, 36],
            engram_layers: vec![1, 14],
            vision_layers: 32,
        }
    }
}

/// Where a tensor sits in the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V41Scope {
    /// Embedding, final norm, and head.
    Global,
    /// Decoder layer `n`.
    Layer(u32),
    /// MTP (DSpark) layer `n`.
    Mtp(u32),
    /// Vision encoder, aligner, and image marker embeddings.
    Vision,
}

/// What a tensor does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V41Component {
    /// `embed.weight`.
    Embedding,
    /// `head.weight`.
    Head,
    /// `norm.weight`.
    FinalNorm,
    /// `attn_norm` and `ffn_norm`.
    LayerNorm,
    /// Attention projections, norms, and sink.
    Attention,
    /// The KV compressor.
    Compressor,
    /// The sparse indexer.
    Indexer,
    /// The MoE gate.
    Gate,
    /// One routed expert.
    RoutedExpert,
    /// The shared expert.
    SharedExpert,
    /// The Engram lookup table (`engram.embed`).
    EngramTable,
    /// Engram projections other than the table.
    Engram,
    /// Hyper-connection mixing parameters.
    HyperConnection,
    /// MTP-only projections and heads.
    MtpHead,
    /// Vision encoder, aligner, and image markers.
    Vision,
}

/// One tensor's typed role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41TensorRole {
    /// Where it sits.
    pub scope: V41Scope,
    /// What it does.
    pub component: V41Component,
    /// The routed expert's number, for [`V41Component::RoutedExpert`].
    pub expert: Option<u32>,
    /// The dtype it must have.
    pub dtype: SafetensorsDtype,
}

/// Every V4.1 tensor name, with its role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V41ModelMap {
    roles: BTreeMap<String, V41TensorRole>,
}

impl V41ModelMap {
    /// Builds the map from the shape facts. The rule for which layers hold a
    /// compressor gate (`compress_ratios[layer] > 1`) and the MTP extras
    /// (`main_*` on the first MTP layer, the heads on the last) follow the
    /// shipped index.
    #[must_use]
    pub fn new(config: &V41WeightConfig) -> Self {
        use SafetensorsDtype::{Bf16, F8E4M3, F8E8M0, F32, I8};
        use V41Component as C;
        let mut roles = BTreeMap::new();
        let mut put = |name: String,
                       scope: V41Scope,
                       component: C,
                       expert: Option<u32>,
                       dtype: SafetensorsDtype| {
            roles.insert(
                name,
                V41TensorRole {
                    scope,
                    component,
                    expert,
                    dtype,
                },
            );
        };
        let fp8 = |put: &mut dyn FnMut(String, V41Scope, C, Option<u32>, SafetensorsDtype),
                   stem: String,
                   scope: V41Scope,
                   component: C| {
            put(format!("{stem}.weight"), scope, component, None, F8E4M3);
            put(format!("{stem}.scale"), scope, component, None, F8E8M0);
        };

        put(
            "embed.weight".into(),
            V41Scope::Global,
            C::Embedding,
            None,
            Bf16,
        );
        put("head.weight".into(), V41Scope::Global, C::Head, None, Bf16);
        put(
            "norm.weight".into(),
            V41Scope::Global,
            C::FinalNorm,
            None,
            Bf16,
        );

        let v = V41Scope::Vision;
        for name in ["image_start", "image_end", "image_newline"] {
            put(name.into(), v, C::Vision, None, Bf16);
        }
        for name in [
            "vision.patch_embed.proj.weight",
            "vision.patch_embed.proj.bias",
            "vision.norm.weight",
            "aligner.w1.weight",
            "aligner.w1.bias",
            "aligner.w2.weight",
            "aligner.w2.bias",
        ] {
            put(name.into(), v, C::Vision, None, Bf16);
        }
        for block in 0..config.vision_layers {
            for part in [
                "norm1.weight",
                "norm2.weight",
                "attn.wqkv.weight",
                "attn.wqkv.bias",
                "attn.wo.weight",
                "attn.wo.bias",
                "mlp.w1.weight",
                "mlp.w2.weight",
            ] {
                put(
                    format!("vision.blocks.{block}.{part}"),
                    v,
                    C::Vision,
                    None,
                    Bf16,
                );
            }
        }

        let block =
            |prefix: &str,
             scope: V41Scope,
             experts: u32,
             put: &mut dyn FnMut(String, V41Scope, C, Option<u32>, SafetensorsDtype)| {
                put(
                    format!("{prefix}.attn.attn_sink"),
                    scope,
                    C::Attention,
                    None,
                    F32,
                );
                put(
                    format!("{prefix}.attn.kv_norm.weight"),
                    scope,
                    C::Attention,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.attn.q_norm.weight"),
                    scope,
                    C::Attention,
                    None,
                    Bf16,
                );
                for projection in ["wq_a", "wq_b", "wkv", "wo_a", "wo_b"] {
                    fp8(
                        put,
                        format!("{prefix}.attn.{projection}"),
                        scope,
                        C::Attention,
                    );
                }
                put(
                    format!("{prefix}.attn_norm.weight"),
                    scope,
                    C::LayerNorm,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.ffn_norm.weight"),
                    scope,
                    C::LayerNorm,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.ffn.gate.weight"),
                    scope,
                    C::Gate,
                    None,
                    Bf16,
                );
                put(format!("{prefix}.ffn.gate.bias"), scope, C::Gate, None, F32);
                put(
                    format!("{prefix}.ffn.gate.bias_vl"),
                    scope,
                    C::Gate,
                    None,
                    F32,
                );
                for projection in ["w1", "w2", "w3"] {
                    fp8(
                        put,
                        format!("{prefix}.ffn.shared_experts.{projection}"),
                        scope,
                        C::SharedExpert,
                    );
                }
                for expert in 0..experts {
                    for projection in ["w1", "w2", "w3"] {
                        let stem = format!("{prefix}.ffn.experts.{expert}.{projection}");
                        put(
                            format!("{stem}.weight"),
                            scope,
                            C::RoutedExpert,
                            Some(expert),
                            I8,
                        );
                        put(
                            format!("{stem}.scale"),
                            scope,
                            C::RoutedExpert,
                            Some(expert),
                            F8E8M0,
                        );
                    }
                }
                for part in ["attn", "ffn"] {
                    for parameter in ["base", "fn", "scale"] {
                        put(
                            format!("{prefix}.hc_{part}_{parameter}"),
                            scope,
                            C::HyperConnection,
                            None,
                            F32,
                        );
                    }
                }
            };

        for layer in 0..config.n_layers {
            let prefix = format!("layers.{layer}");
            let scope = V41Scope::Layer(layer);
            block(&prefix, scope, config.n_routed_experts, &mut put);
            if config.kv_source_layers.contains(&layer) {
                let c = C::Compressor;
                put(
                    format!("{prefix}.attn.compressor.norm.weight"),
                    scope,
                    c,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.attn.compressor.wkv.weight"),
                    scope,
                    c,
                    None,
                    Bf16,
                );
                if config
                    .compress_ratios
                    .get(layer as usize)
                    .copied()
                    .unwrap_or(0)
                    > 1
                {
                    put(
                        format!("{prefix}.attn.compressor.wgate.weight"),
                        scope,
                        c,
                        None,
                        Bf16,
                    );
                }
                put(
                    format!("{prefix}.attn.indexer.k_norm.weight"),
                    scope,
                    C::Indexer,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.attn.indexer.wk.weight"),
                    scope,
                    C::Indexer,
                    None,
                    Bf16,
                );
            }
            if config.index_source_layers.contains(&layer) {
                fp8(
                    &mut put,
                    format!("{prefix}.attn.indexer.wq_b"),
                    scope,
                    C::Indexer,
                );
                put(
                    format!("{prefix}.attn.indexer.weights_proj.weight"),
                    scope,
                    C::Indexer,
                    None,
                    Bf16,
                );
            }
            if config.engram_layers.contains(&layer) {
                put(
                    format!("{prefix}.engram.embed.weight"),
                    scope,
                    C::EngramTable,
                    None,
                    F8E4M3,
                );
                put(
                    format!("{prefix}.engram.embed.scale"),
                    scope,
                    C::EngramTable,
                    None,
                    F8E8M0,
                );
                put(
                    format!("{prefix}.engram.q_weight"),
                    scope,
                    C::Engram,
                    None,
                    Bf16,
                );
                put(
                    format!("{prefix}.engram.k_weight"),
                    scope,
                    C::Engram,
                    None,
                    Bf16,
                );
                fp8(&mut put, format!("{prefix}.engram.wkv"), scope, C::Engram);
            }
        }
        for mtp in 0..config.n_mtp_layers {
            let prefix = format!("mtp.{mtp}");
            let scope = V41Scope::Mtp(mtp);
            block(&prefix, scope, config.n_mtp_routed_experts, &mut put);
            if mtp == 0 {
                put(
                    format!("{prefix}.main_norm.weight"),
                    scope,
                    C::MtpHead,
                    None,
                    Bf16,
                );
                fp8(&mut put, format!("{prefix}.main_proj"), scope, C::MtpHead);
            }
            if mtp + 1 == config.n_mtp_layers {
                for part in [
                    "norm.weight",
                    "markov_head.embed.weight",
                    "markov_head.head.weight",
                    "confidence_head.proj.weight",
                ] {
                    put(format!("{prefix}.{part}"), scope, C::MtpHead, None, Bf16);
                }
            }
        }
        Self { roles }
    }

    /// Number of tensors in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.roles.len()
    }

    /// The role of one tensor.
    ///
    /// # Errors
    ///
    /// Returns [`V41WeightsError::NoSuchTensor`] for a name the map doesn't
    /// know.
    pub fn role(&self, name: &str) -> Result<&V41TensorRole, V41WeightsError> {
        self.roles
            .get(name)
            .ok_or_else(|| V41WeightsError::NoSuchTensor(name.to_owned()))
    }

    /// Iterates names and roles in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V41TensorRole)> {
        self.roles.iter().map(|(name, role)| (name.as_str(), role))
    }

    /// Checks that `names` is exactly the map's set.
    ///
    /// # Errors
    ///
    /// Returns [`V41WeightsError::ModelMap`] with the missing and extra names.
    pub fn check_names<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), V41WeightsError> {
        let names: BTreeSet<&str> = names.into_iter().collect();
        let missing: Vec<&str> = self
            .roles
            .keys()
            .map(String::as_str)
            .filter(|name| !names.contains(name))
            .collect();
        let extra: Vec<&str> = names
            .iter()
            .copied()
            .filter(|name| !self.roles.contains_key(*name))
            .collect();
        if missing.is_empty() && extra.is_empty() {
            return Ok(());
        }
        Err(V41WeightsError::ModelMap {
            missing_count: missing.len(),
            first_missing: missing.first().map(|name| (*name).to_owned()),
            extra_count: extra.len(),
            first_extra: extra.first().map(|name| (*name).to_owned()),
        })
    }
}

// ---------------------------------------------------------------------------
// Stage selection
// ---------------------------------------------------------------------------

/// What one stage Pylon loads: a range of decoder layers plus optional parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41StageSelection {
    /// Decoder layers, end exclusive. `10..15` is layers 10 through 14.
    pub layers: Range<u32>,
    /// Include `embed.weight`.
    pub embeddings: bool,
    /// Include `norm.weight` and `head.weight`.
    pub head: bool,
    /// Include the MTP (DSpark) layers.
    pub mtp: bool,
    /// Include the vision encoder, aligner, and image markers.
    pub vision: bool,
}

impl V41StageSelection {
    /// Only the given decoder layers.
    #[must_use]
    pub const fn layers(layers: Range<u32>) -> Self {
        Self {
            layers,
            embeddings: false,
            head: false,
            mtp: false,
            vision: false,
        }
    }

    /// Whether the selection includes a tensor with this role.
    #[must_use]
    pub fn includes(&self, role: &V41TensorRole) -> bool {
        match role.scope {
            V41Scope::Layer(layer) => self.layers.contains(&layer),
            V41Scope::Mtp(_) => self.mtp,
            V41Scope::Vision => self.vision,
            V41Scope::Global => match role.component {
                V41Component::Embedding => self.embeddings,
                _ => self.head,
            },
        }
    }
}

/// The tensors and bytes of one stage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct V41StageTensors {
    /// Tensor names in name order.
    pub tensors: Vec<String>,
    /// Sum of all tensor bytes.
    pub total_bytes: u64,
    /// Bytes of routed experts (packed FP4 plus scales).
    pub routed_expert_bytes: u64,
    /// Bytes of Engram tables (FP8 plus scales), which can live in host memory.
    pub engram_table_bytes: u64,
    /// Bytes by shard file, which is what a stage downloads.
    pub bytes_by_shard: BTreeMap<String, u64>,
    /// Bytes by dtype label.
    pub bytes_by_dtype: BTreeMap<String, u64>,
}

impl V41StageTensors {
    fn add(&mut self, name: &str, shard: &str, entry: &ShardTensorEntry, role: &V41TensorRole) {
        self.tensors.push(name.to_owned());
        self.total_bytes += entry.byte_len;
        match role.component {
            V41Component::RoutedExpert => self.routed_expert_bytes += entry.byte_len,
            V41Component::EngramTable => self.engram_table_bytes += entry.byte_len,
            _ => {}
        }
        *self.bytes_by_shard.entry(shard.to_owned()).or_default() += entry.byte_len;
        *self
            .bytes_by_dtype
            .entry(entry.dtype.label().to_owned())
            .or_default() += entry.byte_len;
    }
}

// ---------------------------------------------------------------------------
// Oracle parity
// ---------------------------------------------------------------------------

/// Schema label for [`DequantParityManifest`].
pub const DEQUANT_PARITY_SCHEMA: &str = "psionic.deepseek_v41.dequant_parity.v1";

/// Dequantized values the reference produced, for exact comparison. The
/// oracle run (#12371) writes this file next to its activation manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DequantParityManifest {
    /// Must be [`DEQUANT_PARITY_SCHEMA`].
    pub schema: String,
    /// The checkpoint revision the values came from.
    pub revision: String,
    /// The cases.
    pub cases: Vec<DequantParityCase>,
}

/// One reference dequantization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DequantParityCase {
    /// What was dequantized.
    #[serde(flatten)]
    pub target: DequantParityTarget,
    /// The reference's f32 values as IEEE-754 bit patterns, in row-major
    /// order. `weight.float() * scale.float()` in the reference.
    pub f32_bits: Vec<u32>,
}

/// The part of the checkpoint one case covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DequantParityTarget {
    /// One 32×32 block of an FP8 weight.
    Fp8Block {
        /// Weight tensor name.
        tensor: String,
        /// Block row.
        block_row: u64,
        /// Block column.
        block_col: u64,
    },
    /// One row of any weight (FP8, packed FP4, BF16, or F32).
    Row {
        /// Weight tensor name.
        tensor: String,
        /// Row.
        row: u64,
    },
    /// One row of an Engram table.
    EngramRow {
        /// Layer (1 or 14).
        layer: u32,
        /// Row.
        row: u64,
    },
}

/// The result of one parity case.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DequantParityResult {
    /// What was compared.
    pub target: DequantParityTarget,
    /// Values compared.
    pub values: usize,
    /// Values that differ. NaN equals NaN, and +0 equals -0.
    pub mismatches: usize,
    /// First differing index with Psionic's and the reference's values.
    pub first_mismatch: Option<(usize, f32, f32)>,
}

impl DequantParityManifest {
    /// Parses and checks a manifest.
    ///
    /// # Errors
    ///
    /// Refuses invalid JSON and the wrong schema.
    pub fn parse(bytes: &[u8]) -> Result<Self, V41WeightsError> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| V41WeightsError::InvalidOracle(error.to_string()))?;
        if manifest.schema != DEQUANT_PARITY_SCHEMA {
            return Err(V41WeightsError::InvalidOracle(format!(
                "schema is `{}`, wants `{DEQUANT_PARITY_SCHEMA}`",
                manifest.schema
            )));
        }
        Ok(manifest)
    }

    /// Runs every case against a checkpoint with tensor data.
    ///
    /// # Errors
    ///
    /// Returns read and layout errors. A value mismatch is reported in the
    /// result, not as an error.
    pub fn check(
        &self,
        checkpoint: &DeepSeekV41Checkpoint,
    ) -> Result<Vec<DequantParityResult>, V41WeightsError> {
        self.cases
            .iter()
            .map(|case| {
                let ours = match &case.target {
                    DequantParityTarget::Fp8Block {
                        tensor,
                        block_row,
                        block_col,
                    } => {
                        checkpoint
                            .fp8_weight_block(tensor, *block_row, *block_col)?
                            .values
                    }
                    DequantParityTarget::Row { tensor, row } => {
                        checkpoint.dequantize_row(tensor, *row)?
                    }
                    DequantParityTarget::EngramRow { layer, row } => {
                        checkpoint.engram_row(*layer, *row)?
                    }
                };
                Ok(compare_values(case.target.clone(), &ours, &case.f32_bits))
            })
            .collect()
    }
}

fn compare_values(
    target: DequantParityTarget,
    ours: &[f32],
    reference_bits: &[u32],
) -> DequantParityResult {
    let mut mismatches = ours.len().abs_diff(reference_bits.len());
    let mut first_mismatch = None;
    for (index, (&value, &bits)) in ours.iter().zip(reference_bits).enumerate() {
        let reference = f32::from_bits(bits);
        let equal = value == reference || (value.is_nan() && reference.is_nan());
        if !equal {
            mismatches += 1;
            first_mismatch.get_or_insert((index, value, reference));
        }
    }
    DequantParityResult {
        target,
        values: reference_bits.len(),
        mismatches,
        first_mismatch,
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Schema label for [`V41WeightsReport`].
pub const WEIGHTS_REPORT_SCHEMA: &str = "psionic.deepseek_v41.weights_report.v1";

/// What a checkpoint holds per layer and per stage range.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct V41WeightsReport {
    /// [`WEIGHTS_REPORT_SCHEMA`].
    pub schema: &'static str,
    /// `headers_only` or `full`.
    pub mode: &'static str,
    /// Checkpoint identity (shards marked verified only after hashing).
    pub identity: CheckpointIdentity,
    /// Tensors in the index.
    pub tensor_count: usize,
    /// Sum of all tensor bytes.
    pub total_bytes: u64,
    /// Tensor count by dtype label.
    pub dtype_counts: BTreeMap<String, usize>,
    /// Each decoder layer alone.
    pub layers: Vec<V41RangeReport>,
    /// Embedding, head, MTP, and vision parts.
    pub parts: BTreeMap<String, V41RangeReport>,
    /// Requested stage ranges.
    pub ranges: Vec<V41RangeReport>,
}

/// Totals for one selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct V41RangeReport {
    /// A label such as `layers 10-14` or `head`.
    pub label: String,
    /// Tensors selected.
    pub tensors: usize,
    /// Sum of bytes.
    pub bytes: u64,
    /// Routed-expert bytes.
    pub routed_expert_bytes: u64,
    /// Engram-table bytes.
    pub engram_table_bytes: u64,
    /// Shards touched.
    pub shards: Vec<String>,
}

impl V41RangeReport {
    fn from_stage(label: String, stage: &V41StageTensors) -> Self {
        Self {
            label,
            tensors: stage.tensors.len(),
            bytes: stage.total_bytes,
            routed_expert_bytes: stage.routed_expert_bytes,
            engram_table_bytes: stage.engram_table_bytes,
            shards: stage.bytes_by_shard.keys().cloned().collect(),
        }
    }
}

impl DeepSeekV41Checkpoint {
    /// Builds a report with each decoder layer, each optional part, and the
    /// given inclusive layer ranges.
    ///
    /// # Errors
    ///
    /// Refuses a tensor the model map doesn't know.
    pub fn report(&self, ranges: &[(u32, u32)]) -> Result<V41WeightsReport, V41WeightsError> {
        let mut dtype_counts = BTreeMap::new();
        let mut total_bytes = 0;
        for name in self.index.weight_map.keys() {
            let entry = self.entry(name)?;
            *dtype_counts
                .entry(entry.dtype.label().to_owned())
                .or_default() += 1;
            total_bytes += entry.byte_len;
        }
        let n_layers = V41WeightConfig::flash().n_layers;
        let mut layers = Vec::new();
        for layer in 0..n_layers {
            let stage = self.select(&V41StageSelection::layers(layer..layer + 1))?;
            layers.push(V41RangeReport::from_stage(format!("layer {layer}"), &stage));
        }
        let none = V41StageSelection::layers(0..0);
        let mut parts = BTreeMap::new();
        for (label, selection) in [
            (
                "embeddings",
                V41StageSelection {
                    embeddings: true,
                    ..none.clone()
                },
            ),
            (
                "head",
                V41StageSelection {
                    head: true,
                    ..none.clone()
                },
            ),
            (
                "mtp",
                V41StageSelection {
                    mtp: true,
                    ..none.clone()
                },
            ),
            (
                "vision",
                V41StageSelection {
                    vision: true,
                    ..none.clone()
                },
            ),
        ] {
            let stage = self.select(&selection)?;
            parts.insert(
                label.to_owned(),
                V41RangeReport::from_stage(label.to_owned(), &stage),
            );
        }
        let mut range_reports = Vec::new();
        for &(first, last) in ranges {
            let stage = self.select(&V41StageSelection::layers(first..last + 1))?;
            range_reports.push(V41RangeReport::from_stage(
                format!("layers {first}-{last}"),
                &stage,
            ));
        }
        Ok(V41WeightsReport {
            schema: WEIGHTS_REPORT_SCHEMA,
            mode: if self.headers_only {
                "headers_only"
            } else {
                "full"
            },
            identity: self.identity(),
            tensor_count: self.index.weight_map.len(),
            total_bytes,
            dtype_counts,
            layers,
            parts,
            ranges: range_reports,
        })
    }
}

// ---------------------------------------------------------------------------
// Weights check
// ---------------------------------------------------------------------------

/// The stage ranges the plan tracks: layers 0–4, 10–14, and 35–39.
pub const V41_REPORT_RANGES: [(u32, u32); 3] = [(0, 4), (10, 14), (35, 39)];

/// Options for [`check_checkpoint`].
#[derive(Clone, Debug)]
pub struct WeightsCheckOptions {
    /// Read headers only (no tensor data).
    pub headers_only: bool,
    /// Inclusive layer ranges to total.
    pub ranges: Vec<(u32, u32)>,
    /// Hash every shard against the pin. Reads every byte.
    pub verify_sha256: bool,
    /// A [`DequantParityManifest`] to check.
    pub oracle: Option<PathBuf>,
}

impl Default for WeightsCheckOptions {
    fn default() -> Self {
        Self {
            headers_only: false,
            ranges: V41_REPORT_RANGES.to_vec(),
            verify_sha256: false,
            oracle: None,
        }
    }
}

/// What [`check_checkpoint`] found.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WeightsCheckOutcome {
    /// Per-layer and per-range totals.
    pub report: V41WeightsReport,
    /// Tensors whose first and last bytes were read, and, for every weight,
    /// whose first and last rows were dequantized.
    pub tensors_opened: usize,
    /// Rows dequantized while opening.
    pub rows_dequantized: usize,
    /// NaN values found in the dequantized rows (a scale of `0xFF` or an FP8
    /// NaN code).
    pub nan_values: usize,
    /// Tensors that failed to open, with the reason.
    pub failures: Vec<String>,
    /// Shard hashes, when requested.
    pub verified_identity: Option<CheckpointIdentity>,
    /// Oracle parity results, when requested.
    pub parity: Option<Vec<DequantParityResult>>,
}

impl WeightsCheckOutcome {
    /// Whether every check passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
            && self.nan_values == 0
            && self
                .parity
                .as_ref()
                .is_none_or(|results| results.iter().all(|result| result.mismatches == 0))
    }
}

/// Opens a checkpoint directory against the pinned Flash revision and the
/// model map, opens every tensor (unless `headers_only`), totals the layer
/// ranges, and optionally verifies shard hashes and oracle parity. This is
/// what `cargo run -p psionic-models --example deepseek_v41_weights_check`
/// runs.
///
/// # Errors
///
/// Returns errors from opening the checkpoint or reading the oracle file.
/// Per-tensor failures are collected in the outcome instead.
pub fn check_checkpoint(
    root: &Path,
    options: &WeightsCheckOptions,
) -> Result<WeightsCheckOutcome, V41WeightsError> {
    let open = if options.headers_only {
        CheckpointOpenOptions::pinned_flash_headers_only()?
    } else {
        CheckpointOpenOptions::pinned_flash()?
    };
    let checkpoint = DeepSeekV41Checkpoint::open(root, open)?;
    check_opened_checkpoint(&checkpoint, options)
}

/// The checks of [`check_checkpoint`] on an already opened checkpoint, with
/// whatever pin and model-map options it was opened with.
///
/// # Errors
///
/// Same as [`check_checkpoint`], after opening.
pub fn check_opened_checkpoint(
    checkpoint: &DeepSeekV41Checkpoint,
    options: &WeightsCheckOptions,
) -> Result<WeightsCheckOutcome, V41WeightsError> {
    let report = checkpoint.report(&options.ranges)?;
    let mut outcome = WeightsCheckOutcome {
        report,
        tensors_opened: 0,
        rows_dequantized: 0,
        nan_values: 0,
        failures: Vec::new(),
        verified_identity: None,
        parity: None,
    };
    if checkpoint.headers_only() {
        return Ok(outcome);
    }
    for name in checkpoint.index().weight_map.keys() {
        match open_tensor(checkpoint, name) {
            Ok((rows, nans)) => {
                outcome.tensors_opened += 1;
                outcome.rows_dequantized += rows;
                outcome.nan_values += nans;
            }
            Err(error) => outcome.failures.push(format!("{name}: {error}")),
        }
    }
    if options.verify_sha256 {
        outcome.verified_identity = Some(checkpoint.verify_shard_hashes(None)?);
    }
    if let Some(path) = &options.oracle {
        let bytes = std::fs::read(path).map_err(|error| io_error(path, error))?;
        let manifest = DequantParityManifest::parse(&bytes)?;
        outcome.parity = Some(manifest.check(checkpoint)?);
    }
    Ok(outcome)
}

fn open_tensor(
    checkpoint: &DeepSeekV41Checkpoint,
    name: &str,
) -> Result<(usize, usize), V41WeightsError> {
    let entry = checkpoint.entry(name)?;
    let probe = entry.byte_len.min(4096);
    let mut bytes = vec![0_u8; probe as usize];
    checkpoint.read_tensor_bytes(name, 0, &mut bytes)?;
    checkpoint.read_tensor_bytes(name, entry.byte_len - probe, &mut bytes)?;
    if entry.dtype == SafetensorsDtype::F8E8M0 {
        // Scales are checked through their weights.
        return Ok((0, 0));
    }
    let rows = match entry.shape.as_slice() {
        [_] => 1,
        [rows, _] => *rows,
        _ => return Ok((0, 0)),
    };
    if rows == 0 {
        return Ok((0, 0));
    }
    let mut dequantized = 0;
    let mut nans = 0;
    for row in [0, rows - 1] {
        let values = checkpoint.dequantize_row(name, row)?;
        nans += values.iter().filter(|value| value.is_nan()).count();
        dequantized += 1;
        if rows == 1 {
            break;
        }
    }
    Ok((dequantized, nans))
}

#[cfg(test)]
#[path = "deepseek_v41_weights_tests.rs"]
mod tests;
