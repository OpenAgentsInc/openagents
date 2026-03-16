//! Public array import/export surface above `psionic-array`.
//!
//! This crate keeps general array IO separate from model-family and checkpoint
//! ownership while still exposing the format families expected by an MLX-class
//! framework surface.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read, Write},
    path::Path,
};

use half::{bf16, f16};
use psionic_array::{Array, ArrayContext, ArrayError, HostArrayData};
use psionic_core::{DType, QuantizationMode, Shape};
use psionic_models::{GgufContent, ModelLoadError, WeightTensorStorage};
use safetensors::{
    Dtype as SafeTensorsDType, SafeTensorError, SafeTensors, serialize, tensor::TensorView,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "public array import/export surface above psionic-array";

/// Stable named-array bundle used by multi-array import surfaces.
pub type NamedArrayBundle = BTreeMap<String, Array>;

/// Artifact family supported by the public array-IO layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayArtifactFormat {
    /// One single-array NumPy artifact.
    Npy,
    /// One multi-array NumPy zip artifact.
    Npz,
    /// One multi-array safetensors artifact.
    Safetensors,
    /// One multi-array GGUF artifact.
    Gguf,
}

impl ArrayArtifactFormat {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Npy => "npy",
            Self::Npz => "npz",
            Self::Safetensors => "safetensors",
            Self::Gguf => "gguf",
        }
    }
}

/// Direction of one artifact boundary crossing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayArtifactDirection {
    /// Psionic arrays were exported into one artifact.
    Export,
    /// Psionic arrays were imported from one artifact.
    Import,
}

/// Stable metadata for one array entry in a public array artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayArtifactEntry {
    /// Stable entry name.
    pub name: String,
    /// Logical tensor shape.
    pub shape: Shape,
    /// Logical dtype surfaced to the public array layer.
    pub dtype: DType,
    /// Storage quantization posture observed at the artifact boundary.
    pub quantization: QuantizationMode,
    /// Plain-language storage detail such as `dense`, `dense:f16`, or
    /// `gguf_quantized:ggml_q4_0_dequantized_to_f32`.
    pub storage_detail: String,
}

/// Stable receipt emitted for one public array import/export artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayArtifactReceipt {
    /// Format family that crossed the boundary.
    pub format: ArrayArtifactFormat,
    /// Whether the boundary was import or export.
    pub direction: ArrayArtifactDirection,
    /// Stable SHA-256 digest over the artifact bytes.
    pub artifact_sha256: String,
    /// Exact artifact length in bytes.
    pub artifact_bytes: usize,
    /// Entry inventory in stable name order.
    pub entries: Vec<ArrayArtifactEntry>,
}

impl ArrayArtifactReceipt {
    fn new(
        format: ArrayArtifactFormat,
        direction: ArrayArtifactDirection,
        bytes: &[u8],
        mut entries: Vec<ArrayArtifactEntry>,
    ) -> Self {
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            format,
            direction,
            artifact_sha256: hex::encode(Sha256::digest(bytes)),
            artifact_bytes: bytes.len(),
            entries,
        }
    }
}

/// Error returned by the public array-IO layer.
#[derive(Debug, Error)]
pub enum ArrayIoError {
    /// One multi-array export or import expected at least one array entry.
    #[error("array IO bundle requires at least one named array")]
    EmptyBundle,
    /// One named-array entry is invalid for the selected artifact family.
    #[error("array IO entry name `{name}` is invalid: {detail}")]
    InvalidEntryName {
        /// Invalid entry name.
        name: String,
        /// Plain-language reason.
        detail: String,
    },
    /// One named-array entry appeared more than once.
    #[error("array IO entry `{name}` was declared more than once")]
    DuplicateEntryName {
        /// Duplicate entry name.
        name: String,
    },
    /// One artifact family cannot represent the requested dtype honestly.
    #[error("array IO cannot represent dtype {dtype:?} in `{format}`: {detail}")]
    UnsupportedDType {
        /// Artifact family that refused the dtype.
        format: &'static str,
        /// Logical dtype that was requested.
        dtype: DType,
        /// Plain-language refusal detail.
        detail: String,
    },
    /// One artifact payload used a shape, stride, or layout the current bounded
    /// importer or exporter does not support.
    #[error("invalid `{format}` artifact: {detail}")]
    InvalidArtifact {
        /// Artifact family being parsed or encoded.
        format: &'static str,
        /// Plain-language detail.
        detail: String,
    },
    /// One artifact payload length mismatched the declared dtype and shape.
    #[error(
        "artifact entry `{name}` payload length mismatch: expected {expected_len} bytes, found {actual_len}"
    )]
    PayloadLengthMismatch {
        /// Stable entry name.
        name: String,
        /// Expected payload length.
        expected_len: usize,
        /// Actual payload length.
        actual_len: usize,
    },
    /// One filesystem operation failed.
    #[error("path `{path}` failed during {operation}: {message}")]
    Io {
        /// Path that failed.
        path: String,
        /// Operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
    /// One lower array operation failed.
    #[error(transparent)]
    Array(#[from] ArrayError),
    /// One safetensors operation failed.
    #[error("safetensors {operation} failed: {message}")]
    Safetensors {
        /// Safetensors operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
    /// One zip archive operation failed.
    #[error("zip {operation} failed: {message}")]
    Zip {
        /// Zip operation label.
        operation: &'static str,
        /// Plain-language error.
        message: String,
    },
    /// One GGUF parsing or decode operation failed.
    #[error(transparent)]
    ModelLoad(#[from] ModelLoadError),
}

/// Stable borrowed named-array input used by multi-array export APIs.
#[derive(Clone, Copy, Debug)]
pub struct ArrayExportRef<'a> {
    /// Stable entry name in the artifact.
    pub name: &'a str,
    /// Array exported under the entry name.
    pub array: &'a Array,
}

impl<'a> ArrayExportRef<'a> {
    /// Creates one named borrowed array reference.
    #[must_use]
    pub const fn new(name: &'a str, array: &'a Array) -> Self {
        Self { name, array }
    }
}

#[derive(Clone, Debug)]
struct MaterializedArray {
    name: String,
    shape: Shape,
    dtype: DType,
    host: HostArrayData,
}

#[derive(Clone, Debug)]
enum DecodedArrayValues {
    F32(Vec<f32>),
    I8(Vec<i8>),
}

/// Encodes one array into a `.npy` artifact and returns the artifact bytes plus
/// a stable receipt.
pub fn encode_npy(
    array: &Array,
    name: impl Into<String>,
) -> Result<(Vec<u8>, ArrayArtifactReceipt), ArrayIoError> {
    let materialized = materialize_single_array(name.into(), array)?;
    let bytes = encode_npy_entry(materialized.name.as_str(), &materialized)?;
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Npy,
        ArrayArtifactDirection::Export,
        bytes.as_slice(),
        vec![dense_entry_from_materialized(
            &materialized,
            dense_storage_detail(materialized.dtype),
        )],
    );
    Ok((bytes, receipt))
}

/// Decodes one `.npy` artifact into a public array plus a stable receipt.
pub fn decode_npy(
    context: &ArrayContext,
    bytes: &[u8],
    name: impl Into<String>,
) -> Result<(Array, ArrayArtifactReceipt), ArrayIoError> {
    let name = name.into();
    validate_entry_name(name.as_str())?;
    let (shape, dtype, values) = decode_npy_payload(name.as_str(), bytes)?;
    let array = create_array(context, &shape, dtype, values)?;
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Npy,
        ArrayArtifactDirection::Import,
        bytes,
        vec![ArrayArtifactEntry {
            name,
            shape,
            dtype,
            quantization: QuantizationMode::None,
            storage_detail: dense_storage_detail(dtype),
        }],
    );
    Ok((array, receipt))
}

/// Encodes a named array bundle into a `.npz` artifact and returns the bytes
/// plus a stable receipt.
pub fn encode_npz(
    arrays: &[ArrayExportRef<'_>],
) -> Result<(Vec<u8>, ArrayArtifactReceipt), ArrayIoError> {
    let materialized = materialize_named_arrays(arrays)?;
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut archive = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for array in &materialized {
            let entry_name = format!("{}.npy", array.name);
            let bytes = encode_npy_entry(array.name.as_str(), array)?;
            archive
                .start_file(entry_name, options)
                .map_err(zip_error_start_file)?;
            archive
                .write_all(bytes.as_slice())
                .map_err(|error| ArrayIoError::Zip {
                    operation: "write entry",
                    message: error.to_string(),
                })?;
        }
        archive.finish().map_err(zip_error_finish)?;
    }
    let bytes = cursor.into_inner();
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Npz,
        ArrayArtifactDirection::Export,
        bytes.as_slice(),
        materialized
            .iter()
            .map(|array| dense_entry_from_materialized(array, dense_storage_detail(array.dtype)))
            .collect(),
    );
    Ok((bytes, receipt))
}

/// Decodes a `.npz` artifact into a named array bundle plus a stable receipt.
pub fn decode_npz(
    context: &ArrayContext,
    bytes: &[u8],
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error_open_archive)?;
    let mut arrays = BTreeMap::new();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(zip_error_read_entry)?;
        if file.is_dir() {
            continue;
        }
        let raw_name = file.name().to_string();
        let name = raw_name
            .strip_suffix(".npy")
            .ok_or_else(|| ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Npz.label(),
                detail: format!("zip entry `{raw_name}` is not a `.npy` payload"),
            })?
            .to_string();
        validate_entry_name(name.as_str())?;
        let mut entry_bytes = Vec::new();
        file.read_to_end(&mut entry_bytes)
            .map_err(|error| ArrayIoError::Zip {
                operation: "read entry",
                message: error.to_string(),
            })?;
        let (shape, dtype, values) = decode_npy_payload(name.as_str(), entry_bytes.as_slice())?;
        if arrays.contains_key(name.as_str()) {
            return Err(ArrayIoError::DuplicateEntryName { name });
        }
        let array = create_array(context, &shape, dtype, values)?;
        arrays.insert(name.clone(), array);
        entries.push(ArrayArtifactEntry {
            name,
            shape,
            dtype,
            quantization: QuantizationMode::None,
            storage_detail: dense_storage_detail(dtype),
        });
    }
    if arrays.is_empty() {
        return Err(ArrayIoError::EmptyBundle);
    }
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Npz,
        ArrayArtifactDirection::Import,
        bytes,
        entries,
    );
    Ok((arrays, receipt))
}

/// Encodes a named array bundle into a safetensors artifact and returns the
/// bytes plus a stable receipt.
pub fn encode_safetensors(
    arrays: &[ArrayExportRef<'_>],
) -> Result<(Vec<u8>, ArrayArtifactReceipt), ArrayIoError> {
    let materialized = materialize_named_arrays(arrays)?;
    let mut raw_buffers = Vec::with_capacity(materialized.len());
    for array in &materialized {
        raw_buffers.push((
            array.name.clone(),
            encode_dense_payload(ArrayArtifactFormat::Safetensors, array)?,
            array.shape.dims().to_vec(),
            safetensors_dtype(array.dtype)?,
            array.shape.clone(),
            array.dtype,
        ));
    }

    let mut views = Vec::with_capacity(raw_buffers.len());
    for (name, raw_bytes, shape, dtype, _, _) in &raw_buffers {
        let view = TensorView::new(*dtype, shape.clone(), raw_bytes.as_slice())
            .map_err(safetensors_error("create tensor view"))?;
        views.push((name.clone(), view));
    }

    let bytes = serialize(
        views
            .iter()
            .map(|(name, view)| (name.as_str(), view.clone())),
        None,
    )
    .map_err(safetensors_error("serialize artifact"))?;
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Safetensors,
        ArrayArtifactDirection::Export,
        bytes.as_slice(),
        raw_buffers
            .iter()
            .map(|(name, _, _, _, shape, dtype)| ArrayArtifactEntry {
                name: name.clone(),
                shape: shape.clone(),
                dtype: *dtype,
                quantization: QuantizationMode::None,
                storage_detail: dense_storage_detail(*dtype),
            })
            .collect(),
    );
    Ok((bytes, receipt))
}

/// Decodes a safetensors artifact into a named array bundle plus a stable
/// receipt.
pub fn decode_safetensors(
    context: &ArrayContext,
    bytes: &[u8],
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let tensors =
        SafeTensors::deserialize(bytes).map_err(safetensors_error("deserialize artifact"))?;
    let mut names = tensors.names();
    names.sort_unstable();
    if names.is_empty() {
        return Err(ArrayIoError::EmptyBundle);
    }

    let mut arrays = BTreeMap::new();
    let mut entries = Vec::new();
    for name in names {
        validate_entry_name(name)?;
        let tensor = tensors
            .tensor(name)
            .map_err(safetensors_error("read tensor view"))?;
        let shape = Shape::new(tensor.shape().to_vec());
        let dtype = dtype_from_safetensors(name, tensor.dtype())?;
        let values = decode_dense_values_from_bytes(name, dtype, tensor.data())?;
        let array = create_array(context, &shape, dtype, values)?;
        arrays.insert(String::from(name), array);
        entries.push(ArrayArtifactEntry {
            name: String::from(name),
            shape,
            dtype,
            quantization: QuantizationMode::None,
            storage_detail: dense_storage_detail(dtype),
        });
    }
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Safetensors,
        ArrayArtifactDirection::Import,
        bytes,
        entries,
    );
    Ok((arrays, receipt))
}

/// Encodes a named array bundle into one dense GGUF artifact and returns the
/// bytes plus a stable receipt.
pub fn encode_gguf(
    arrays: &[ArrayExportRef<'_>],
) -> Result<(Vec<u8>, ArrayArtifactReceipt), ArrayIoError> {
    const GGUF_ALIGNMENT: usize = 32;

    let materialized = materialize_named_arrays(arrays)?;
    let mut tensor_specs = Vec::with_capacity(materialized.len());
    for array in &materialized {
        let tensor_type = gguf_tensor_type(array.dtype)?;
        let bytes = encode_dense_payload(ArrayArtifactFormat::Gguf, array)?;
        tensor_specs.push((array.clone(), tensor_type, bytes));
    }

    let mut buffer = Vec::new();
    buffer.extend(b"GGUF");
    push_u32(&mut buffer, 3);
    push_u64(
        &mut buffer,
        u64::try_from(tensor_specs.len()).map_err(|_| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Gguf.label(),
            detail: String::from("tensor count does not fit into u64"),
        })?,
    );
    push_u64(&mut buffer, 0);

    let mut next_offset = 0_usize;
    let mut offsets = Vec::with_capacity(tensor_specs.len());
    for (_, _, bytes) in &tensor_specs {
        offsets.push(next_offset);
        next_offset = align_usize(next_offset + bytes.len(), GGUF_ALIGNMENT);
    }

    for ((array, tensor_type, _), offset) in tensor_specs.iter().zip(offsets.iter()) {
        push_gguf_string(&mut buffer, array.name.as_str())?;
        push_u32(
            &mut buffer,
            u32::try_from(array.shape.rank()).map_err(|_| ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Gguf.label(),
                detail: format!("tensor `{}` rank does not fit into u32", array.name),
            })?,
        );
        for dimension in array.shape.dims().iter().rev() {
            push_u64(
                &mut buffer,
                u64::try_from(*dimension).map_err(|_| ArrayIoError::InvalidArtifact {
                    format: ArrayArtifactFormat::Gguf.label(),
                    detail: format!(
                        "tensor `{}` dimension `{dimension}` does not fit into u64",
                        array.name
                    ),
                })?,
            );
        }
        push_u32(&mut buffer, *tensor_type);
        push_u64(
            &mut buffer,
            u64::try_from(*offset).map_err(|_| ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Gguf.label(),
                detail: format!("tensor `{}` offset does not fit into u64", array.name),
            })?,
        );
    }

    let tensor_data_offset = align_usize(buffer.len(), GGUF_ALIGNMENT);
    buffer.resize(tensor_data_offset, 0);

    for ((_, _, bytes), offset) in tensor_specs.iter().zip(offsets.iter()) {
        let start = tensor_data_offset + offset;
        if buffer.len() < start {
            buffer.resize(start, 0);
        }
        buffer.extend_from_slice(bytes.as_slice());
        buffer.resize(align_usize(buffer.len(), GGUF_ALIGNMENT), 0);
    }

    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Gguf,
        ArrayArtifactDirection::Export,
        buffer.as_slice(),
        materialized
            .iter()
            .map(|array| {
                dense_entry_from_materialized(
                    array,
                    format!("dense:{}", gguf_dtype_label(array.dtype)),
                )
            })
            .collect(),
    );
    Ok((buffer, receipt))
}

/// Decodes one GGUF artifact into a named array bundle plus a stable receipt.
pub fn decode_gguf(
    context: &ArrayContext,
    bytes: &[u8],
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let content = GgufContent::read(bytes)?;
    let mut arrays = BTreeMap::new();
    let mut entries = Vec::new();
    for tensor_info in content.tensor_infos() {
        let name = tensor_info.name.clone();
        validate_entry_name(name.as_str())?;
        let tensor = content.load_tensor(bytes, name.as_str())?;
        let metadata = tensor.metadata().clone();
        let values = tensor.values()?;
        let array = create_array(
            context,
            &metadata.shape,
            metadata.dtype,
            DecodedArrayValues::F32(values.into_owned()),
        )?;
        arrays.insert(name.clone(), array);
        let storage_detail = match tensor.storage() {
            WeightTensorStorage::DequantizedF32(_) => {
                format!("dense:{}", gguf_dtype_label(metadata.dtype))
            }
            WeightTensorStorage::QuantizedBlocks(_) => {
                format!(
                    "gguf_quantized:{}_dequantized_to_f32",
                    metadata.quantization.label()
                )
            }
        };
        entries.push(ArrayArtifactEntry {
            name,
            shape: metadata.shape,
            dtype: metadata.dtype,
            quantization: metadata.quantization,
            storage_detail,
        });
    }
    if arrays.is_empty() {
        return Err(ArrayIoError::EmptyBundle);
    }
    let receipt = ArrayArtifactReceipt::new(
        ArrayArtifactFormat::Gguf,
        ArrayArtifactDirection::Import,
        bytes,
        entries,
    );
    Ok((arrays, receipt))
}

/// Saves one array to a `.npy` path.
pub fn save_npy_path(
    path: impl AsRef<Path>,
    array: &Array,
) -> Result<ArrayArtifactReceipt, ArrayIoError> {
    let path = path.as_ref();
    let name = entry_name_from_path(path, "array");
    let (bytes, receipt) = encode_npy(array, name)?;
    fs::write(path, bytes).map_err(|error| io_error(path, "write npy artifact", error))?;
    Ok(receipt)
}

/// Loads one `.npy` path into a public array.
pub fn load_npy_path(
    context: &ArrayContext,
    path: impl AsRef<Path>,
) -> Result<(Array, ArrayArtifactReceipt), ArrayIoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| io_error(path, "read npy artifact", error))?;
    decode_npy(
        context,
        bytes.as_slice(),
        entry_name_from_path(path, "array"),
    )
}

/// Saves a named array bundle to a `.npz` path.
pub fn save_npz_path(
    path: impl AsRef<Path>,
    arrays: &[ArrayExportRef<'_>],
) -> Result<ArrayArtifactReceipt, ArrayIoError> {
    let path = path.as_ref();
    let (bytes, receipt) = encode_npz(arrays)?;
    fs::write(path, bytes).map_err(|error| io_error(path, "write npz artifact", error))?;
    Ok(receipt)
}

/// Loads a `.npz` path into a named array bundle.
pub fn load_npz_path(
    context: &ArrayContext,
    path: impl AsRef<Path>,
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| io_error(path, "read npz artifact", error))?;
    decode_npz(context, bytes.as_slice())
}

/// Saves a named array bundle to a safetensors path.
pub fn save_safetensors_path(
    path: impl AsRef<Path>,
    arrays: &[ArrayExportRef<'_>],
) -> Result<ArrayArtifactReceipt, ArrayIoError> {
    let path = path.as_ref();
    let (bytes, receipt) = encode_safetensors(arrays)?;
    fs::write(path, bytes).map_err(|error| io_error(path, "write safetensors artifact", error))?;
    Ok(receipt)
}

/// Loads a safetensors path into a named array bundle.
pub fn load_safetensors_path(
    context: &ArrayContext,
    path: impl AsRef<Path>,
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| io_error(path, "read safetensors artifact", error))?;
    decode_safetensors(context, bytes.as_slice())
}

/// Saves a named array bundle to a GGUF path.
pub fn save_gguf_path(
    path: impl AsRef<Path>,
    arrays: &[ArrayExportRef<'_>],
) -> Result<ArrayArtifactReceipt, ArrayIoError> {
    let path = path.as_ref();
    let (bytes, receipt) = encode_gguf(arrays)?;
    fs::write(path, bytes).map_err(|error| io_error(path, "write gguf artifact", error))?;
    Ok(receipt)
}

/// Loads a GGUF path into a named array bundle.
pub fn load_gguf_path(
    context: &ArrayContext,
    path: impl AsRef<Path>,
) -> Result<(NamedArrayBundle, ArrayArtifactReceipt), ArrayIoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| io_error(path, "read gguf artifact", error))?;
    decode_gguf(context, bytes.as_slice())
}

fn materialize_single_array(
    name: String,
    array: &Array,
) -> Result<MaterializedArray, ArrayIoError> {
    validate_entry_name(name.as_str())?;
    Ok(MaterializedArray {
        name,
        shape: array.shape().clone(),
        dtype: array.dtype(),
        host: array.to_host_data()?,
    })
}

fn materialize_named_arrays(
    arrays: &[ArrayExportRef<'_>],
) -> Result<Vec<MaterializedArray>, ArrayIoError> {
    if arrays.is_empty() {
        return Err(ArrayIoError::EmptyBundle);
    }
    let mut seen = BTreeSet::new();
    let mut materialized = Vec::with_capacity(arrays.len());
    for entry in arrays {
        validate_entry_name(entry.name)?;
        if !seen.insert(entry.name.to_string()) {
            return Err(ArrayIoError::DuplicateEntryName {
                name: String::from(entry.name),
            });
        }
        materialized.push(MaterializedArray {
            name: String::from(entry.name),
            shape: entry.array.shape().clone(),
            dtype: entry.array.dtype(),
            host: entry.array.to_host_data()?,
        });
    }
    materialized.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(materialized)
}

fn validate_entry_name(name: &str) -> Result<(), ArrayIoError> {
    if name.is_empty() {
        return Err(ArrayIoError::InvalidEntryName {
            name: String::from(name),
            detail: String::from("entry names must not be empty"),
        });
    }
    if name.contains('\0') {
        return Err(ArrayIoError::InvalidEntryName {
            name: String::from(name),
            detail: String::from("entry names must not contain NUL bytes"),
        });
    }
    if name.contains('/') || name.contains('\\') {
        return Err(ArrayIoError::InvalidEntryName {
            name: String::from(name),
            detail: String::from("entry names must not contain path separators"),
        });
    }
    Ok(())
}

fn dense_entry_from_materialized(
    array: &MaterializedArray,
    storage_detail: String,
) -> ArrayArtifactEntry {
    ArrayArtifactEntry {
        name: array.name.clone(),
        shape: array.shape.clone(),
        dtype: array.dtype,
        quantization: QuantizationMode::None,
        storage_detail,
    }
}

fn dense_storage_detail(dtype: DType) -> String {
    format!("dense:{}", dtype_label(dtype))
}

fn dtype_label(dtype: DType) -> &'static str {
    match dtype {
        DType::F32 => "f32",
        DType::F16 => "f16",
        DType::BF16 => "bf16",
        DType::I8 => "i8",
    }
}

fn gguf_dtype_label(dtype: DType) -> &'static str {
    match dtype {
        DType::F32 => "f32",
        DType::F16 => "f16",
        DType::BF16 => "bf16",
        DType::I8 => "i8",
    }
}

fn encode_npy_entry(name: &str, array: &MaterializedArray) -> Result<Vec<u8>, ArrayIoError> {
    let descr = npy_descr(array.dtype)?;
    let payload = encode_dense_payload(ArrayArtifactFormat::Npy, array)?;
    let mut bytes = Vec::new();
    bytes.extend(b"\x93NUMPY");
    bytes.push(1);
    bytes.push(0);
    let shape_repr = npy_shape_repr(&array.shape);
    let mut header =
        format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {shape_repr}, }}");
    let preamble_len = 10;
    let padding = (16 - ((preamble_len + header.len() + 1) % 16)) % 16;
    header.push_str(&" ".repeat(padding));
    header.push('\n');
    let header_len = u16::try_from(header.len()).map_err(|_| ArrayIoError::InvalidArtifact {
        format: ArrayArtifactFormat::Npy.label(),
        detail: format!("entry `{name}` header length does not fit into v1 npy"),
    })?;
    bytes.extend(header_len.to_le_bytes());
    bytes.extend(header.as_bytes());
    bytes.extend(payload);
    Ok(bytes)
}

fn decode_npy_payload(
    name: &str,
    bytes: &[u8],
) -> Result<(Shape, DType, DecodedArrayValues), ArrayIoError> {
    if bytes.len() < 10 {
        return Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` is shorter than the npy header preamble"),
        });
    }
    if &bytes[..6] != b"\x93NUMPY" {
        return Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` is missing the npy magic header"),
        });
    }
    let version = (bytes[6], bytes[7]);
    let (header_len, header_offset) = match version {
        (1, 0) => {
            let header_len = u16::from_le_bytes([bytes[8], bytes[9]]);
            (usize::from(header_len), 10_usize)
        }
        (2, 0) | (3, 0) => {
            if bytes.len() < 12 {
                return Err(ArrayIoError::InvalidArtifact {
                    format: ArrayArtifactFormat::Npy.label(),
                    detail: format!(
                        "entry `{name}` is shorter than the version {version:?} npy preamble"
                    ),
                });
            }
            let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
            (
                usize::try_from(header_len).map_err(|_| ArrayIoError::InvalidArtifact {
                    format: ArrayArtifactFormat::Npy.label(),
                    detail: format!("entry `{name}` header length does not fit into usize"),
                })?,
                12_usize,
            )
        }
        _ => {
            return Err(ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Npy.label(),
                detail: format!("entry `{name}` uses unsupported npy version {version:?}"),
            });
        }
    };
    let data_offset =
        header_offset
            .checked_add(header_len)
            .ok_or_else(|| ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Npy.label(),
                detail: format!("entry `{name}` header length overflowed usize"),
            })?;
    let header =
        bytes
            .get(header_offset..data_offset)
            .ok_or_else(|| ArrayIoError::InvalidArtifact {
                format: ArrayArtifactFormat::Npy.label(),
                detail: format!("entry `{name}` header length is out of bounds"),
            })?;
    let header = std::str::from_utf8(header).map_err(|error| ArrayIoError::InvalidArtifact {
        format: ArrayArtifactFormat::Npy.label(),
        detail: format!("entry `{name}` header is not utf-8: {error}"),
    })?;
    let fortran_order = parse_npy_fortran_order(name, header)?;
    if fortran_order {
        return Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!(
                "entry `{name}` uses Fortran order, which the bounded array-IO layer does not support"
            ),
        });
    }
    let dtype = parse_npy_dtype(name, header)?;
    let shape = parse_npy_shape(name, header)?;
    let payload = bytes
        .get(data_offset..)
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` payload is out of bounds"),
        })?;
    let values = decode_dense_values_from_bytes(name, dtype, payload)?;
    validate_payload_length(name, dtype, &shape, payload.len())?;
    Ok((shape, dtype, values))
}

fn parse_npy_dtype(name: &str, header: &str) -> Result<DType, ArrayIoError> {
    let descr = parse_npy_quoted_field(name, header, "descr")?;
    match descr {
        "<f4" | "|f4" => Ok(DType::F32),
        "<f2" | "|f2" => Ok(DType::F16),
        "|i1" | "<i1" => Ok(DType::I8),
        other => Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` uses unsupported dtype descriptor `{other}`"),
        }),
    }
}

fn parse_npy_fortran_order(name: &str, header: &str) -> Result<bool, ArrayIoError> {
    let value = parse_npy_plain_field(name, header, "fortran_order")?;
    match value {
        "False" => Ok(false),
        "True" => Ok(true),
        other => Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` uses invalid fortran_order `{other}`"),
        }),
    }
}

fn parse_npy_shape(name: &str, header: &str) -> Result<Shape, ArrayIoError> {
    let marker = "'shape':";
    let start = header
        .find(marker)
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` header is missing `shape`"),
        })?
        + marker.len();
    let remainder = header[start..].trim_start();
    let open = remainder
        .find('(')
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` shape is missing `(`"),
        })?;
    let close = remainder[open..]
        .find(')')
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` shape is missing `)`"),
        })?
        + open;
    let inner = remainder[open + 1..close].trim();
    if inner.is_empty() {
        return Ok(Shape::scalar());
    }
    let mut dims = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        dims.push(
            part.parse::<usize>()
                .map_err(|error| ArrayIoError::InvalidArtifact {
                    format: ArrayArtifactFormat::Npy.label(),
                    detail: format!("entry `{name}` has invalid shape dimension `{part}`: {error}"),
                })?,
        );
    }
    Ok(Shape::new(dims))
}

fn parse_npy_quoted_field<'a>(
    name: &str,
    header: &'a str,
    key: &str,
) -> Result<&'a str, ArrayIoError> {
    let marker = format!("'{key}':");
    let start = header
        .find(marker.as_str())
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` header is missing `{key}`"),
        })?
        + marker.len();
    let remainder = header[start..].trim_start();
    let remainder = remainder
        .strip_prefix('\'')
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` field `{key}` is not quoted"),
        })?;
    let end = remainder
        .find('\'')
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` field `{key}` is missing a closing quote"),
        })?;
    Ok(&remainder[..end])
}

fn parse_npy_plain_field<'a>(
    name: &str,
    header: &'a str,
    key: &str,
) -> Result<&'a str, ArrayIoError> {
    let marker = format!("'{key}':");
    let start = header
        .find(marker.as_str())
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Npy.label(),
            detail: format!("entry `{name}` header is missing `{key}`"),
        })?
        + marker.len();
    let remainder = header[start..].trim_start();
    let end = remainder.find(',').unwrap_or(remainder.len());
    Ok(remainder[..end].trim())
}

fn encode_dense_payload(
    format: ArrayArtifactFormat,
    array: &MaterializedArray,
) -> Result<Vec<u8>, ArrayIoError> {
    match array.dtype {
        DType::F32 => {
            let values =
                array
                    .host
                    .as_f32_slice()
                    .ok_or_else(|| ArrayIoError::InvalidArtifact {
                        format: format.label(),
                        detail: format!(
                            "entry `{}` could not expose dense f32 host values",
                            array.name
                        ),
                    })?;
            Ok(values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect())
        }
        DType::F16 => {
            let values =
                array
                    .host
                    .as_f32_slice()
                    .ok_or_else(|| ArrayIoError::InvalidArtifact {
                        format: format.label(),
                        detail: format!(
                            "entry `{}` could not expose dense f16 host values",
                            array.name
                        ),
                    })?;
            Ok(values
                .iter()
                .flat_map(|value| f16::from_f32(*value).to_bits().to_le_bytes())
                .collect())
        }
        DType::BF16 => {
            if matches!(format, ArrayArtifactFormat::Npy | ArrayArtifactFormat::Npz) {
                return Err(ArrayIoError::UnsupportedDType {
                    format: format.label(),
                    dtype: DType::BF16,
                    detail: String::from(
                        "the bounded NumPy path preserves only f32, f16, and i8 today; use safetensors or gguf for bf16",
                    ),
                });
            }
            let values =
                array
                    .host
                    .as_f32_slice()
                    .ok_or_else(|| ArrayIoError::InvalidArtifact {
                        format: format.label(),
                        detail: format!(
                            "entry `{}` could not expose dense bf16 host values",
                            array.name
                        ),
                    })?;
            Ok(values
                .iter()
                .flat_map(|value| bf16::from_f32(*value).to_bits().to_le_bytes())
                .collect())
        }
        DType::I8 => {
            if matches!(format, ArrayArtifactFormat::Gguf) {
                return Err(ArrayIoError::UnsupportedDType {
                    format: format.label(),
                    dtype: DType::I8,
                    detail: String::from(
                        "the bounded GGUF export path currently supports only dense f32, f16, and bf16 tensors",
                    ),
                });
            }
            let values = array
                .host
                .as_i8_slice()
                .ok_or_else(|| ArrayIoError::InvalidArtifact {
                    format: format.label(),
                    detail: format!(
                        "entry `{}` could not expose dense i8 host values",
                        array.name
                    ),
                })?;
            Ok(values
                .iter()
                .map(|value| value.to_le_bytes()[0])
                .collect::<Vec<_>>())
        }
    }
}

fn decode_dense_values_from_bytes(
    name: &str,
    dtype: DType,
    bytes: &[u8],
) -> Result<DecodedArrayValues, ArrayIoError> {
    match dtype {
        DType::F32 => {
            let chunks = bytes.chunks_exact(4);
            if !chunks.remainder().is_empty() {
                return Err(ArrayIoError::InvalidArtifact {
                    format: "dense",
                    detail: format!("entry `{name}` f32 payload is not aligned to 4 bytes"),
                });
            }
            let values = chunks
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok(DecodedArrayValues::F32(values))
        }
        DType::F16 => {
            let chunks = bytes.chunks_exact(2);
            if !chunks.remainder().is_empty() {
                return Err(ArrayIoError::InvalidArtifact {
                    format: "dense",
                    detail: format!("entry `{name}` f16 payload is not aligned to 2 bytes"),
                });
            }
            let values = chunks
                .map(|chunk| f16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f32())
                .collect();
            Ok(DecodedArrayValues::F32(values))
        }
        DType::BF16 => {
            let chunks = bytes.chunks_exact(2);
            if !chunks.remainder().is_empty() {
                return Err(ArrayIoError::InvalidArtifact {
                    format: "dense",
                    detail: format!("entry `{name}` bf16 payload is not aligned to 2 bytes"),
                });
            }
            let values = chunks
                .map(|chunk| bf16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f32())
                .collect();
            Ok(DecodedArrayValues::F32(values))
        }
        DType::I8 => Ok(DecodedArrayValues::I8(
            bytes
                .iter()
                .map(|byte| i8::from_le_bytes([*byte]))
                .collect(),
        )),
    }
}

fn validate_payload_length(
    name: &str,
    dtype: DType,
    shape: &Shape,
    actual_len: usize,
) -> Result<(), ArrayIoError> {
    let expected_len = shape
        .element_count()
        .checked_mul(dtype.element_size_bytes())
        .ok_or_else(|| ArrayIoError::InvalidArtifact {
            format: "dense",
            detail: format!("entry `{name}` expected byte length overflowed usize"),
        })?;
    if expected_len != actual_len {
        return Err(ArrayIoError::PayloadLengthMismatch {
            name: String::from(name),
            expected_len,
            actual_len,
        });
    }
    Ok(())
}

fn create_array(
    context: &ArrayContext,
    shape: &Shape,
    dtype: DType,
    values: DecodedArrayValues,
) -> Result<Array, ArrayIoError> {
    match (dtype, values) {
        (DType::F32, DecodedArrayValues::F32(values)) => {
            Ok(context.constant_f32(shape.clone(), values)?)
        }
        (DType::F16, DecodedArrayValues::F32(values)) => Ok(context
            .constant_f32(shape.clone(), values)?
            .cast(DType::F16)?),
        (DType::BF16, DecodedArrayValues::F32(values)) => Ok(context
            .constant_f32(shape.clone(), values)?
            .cast(DType::BF16)?),
        (DType::I8, DecodedArrayValues::I8(values)) => {
            let values = values.into_iter().map(f32::from).collect::<Vec<_>>();
            Ok(context
                .constant_f32(shape.clone(), values)?
                .cast(DType::I8)?)
        }
        (dtype, _) => Err(ArrayIoError::InvalidArtifact {
            format: "dense",
            detail: format!("decoded payload family did not match dtype {dtype:?}"),
        }),
    }
}

fn safetensors_dtype(dtype: DType) -> Result<SafeTensorsDType, ArrayIoError> {
    match dtype {
        DType::F32 => Ok(SafeTensorsDType::F32),
        DType::F16 => Ok(SafeTensorsDType::F16),
        DType::BF16 => Ok(SafeTensorsDType::BF16),
        DType::I8 => Ok(SafeTensorsDType::I8),
    }
}

fn dtype_from_safetensors(name: &str, dtype: SafeTensorsDType) -> Result<DType, ArrayIoError> {
    match dtype {
        SafeTensorsDType::F32 => Ok(DType::F32),
        SafeTensorsDType::F16 => Ok(DType::F16),
        SafeTensorsDType::BF16 => Ok(DType::BF16),
        SafeTensorsDType::I8 => Ok(DType::I8),
        other => Err(ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Safetensors.label(),
            detail: format!("tensor `{name}` uses unsupported safetensors dtype `{other}`"),
        }),
    }
}

fn npy_descr(dtype: DType) -> Result<&'static str, ArrayIoError> {
    match dtype {
        DType::F32 => Ok("<f4"),
        DType::F16 => Ok("<f2"),
        DType::I8 => Ok("|i1"),
        DType::BF16 => Err(ArrayIoError::UnsupportedDType {
            format: ArrayArtifactFormat::Npy.label(),
            dtype,
            detail: String::from(
                "the bounded NumPy path does not claim a portable bf16 descriptor",
            ),
        }),
    }
}

fn npy_shape_repr(shape: &Shape) -> String {
    match shape.dims() {
        [] => String::from("()"),
        [only] => format!("({only},)"),
        dims => format!(
            "({})",
            dims.iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn gguf_tensor_type(dtype: DType) -> Result<u32, ArrayIoError> {
    match dtype {
        DType::F32 => Ok(0),
        DType::F16 => Ok(1),
        DType::BF16 => Ok(30),
        DType::I8 => Err(ArrayIoError::UnsupportedDType {
            format: ArrayArtifactFormat::Gguf.label(),
            dtype,
            detail: String::from(
                "the bounded GGUF export path currently supports only dense f32, f16, and bf16 tensors",
            ),
        }),
    }
}

fn push_gguf_string(buffer: &mut Vec<u8>, value: &str) -> Result<(), ArrayIoError> {
    push_u64(
        buffer,
        u64::try_from(value.len()).map_err(|_| ArrayIoError::InvalidArtifact {
            format: ArrayArtifactFormat::Gguf.label(),
            detail: String::from("gguf string length does not fit into u64"),
        })?,
    );
    buffer.extend(value.as_bytes());
    Ok(())
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend(value.to_le_bytes());
}

fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend(value.to_le_bytes());
}

fn align_usize(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        value
    } else {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + (alignment - remainder)
        }
    }
}

fn entry_name_from_path(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(String::from)
        .unwrap_or_else(|| String::from(fallback))
}

fn io_error(path: &Path, operation: &'static str, error: std::io::Error) -> ArrayIoError {
    ArrayIoError::Io {
        path: path.display().to_string(),
        operation,
        message: error.to_string(),
    }
}

fn safetensors_error(operation: &'static str) -> impl Fn(SafeTensorError) -> ArrayIoError {
    move |error| ArrayIoError::Safetensors {
        operation,
        message: error.to_string(),
    }
}

fn zip_error_open_archive(error: zip::result::ZipError) -> ArrayIoError {
    ArrayIoError::Zip {
        operation: "open archive",
        message: error.to_string(),
    }
}

fn zip_error_read_entry(error: zip::result::ZipError) -> ArrayIoError {
    ArrayIoError::Zip {
        operation: "read entry",
        message: error.to_string(),
    }
}

fn zip_error_start_file(error: zip::result::ZipError) -> ArrayIoError {
    ArrayIoError::Zip {
        operation: "start file",
        message: error.to_string(),
    }
}

fn zip_error_finish(error: zip::result::ZipError) -> ArrayIoError {
    ArrayIoError::Zip {
        operation: "finish archive",
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::{
        ArrayArtifactFormat, ArrayExportRef, ArrayIoError, decode_gguf, load_gguf_path,
        load_npy_path, load_npz_path, load_safetensors_path, save_gguf_path, save_npy_path,
        save_npz_path, save_safetensors_path,
    };
    use psionic_array::ArrayContext;
    use psionic_core::{DType, Shape};
    use tempfile::tempdir;

    #[test]
    fn npy_roundtrip_preserves_shape_dtype_and_values() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArrayContext::cpu();
        let array = context.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let temp = tempdir()?;
        let path = temp.path().join("matrix.npy");

        let receipt = save_npy_path(&path, &array)?;
        assert_eq!(receipt.format, ArrayArtifactFormat::Npy);
        assert_eq!(receipt.entries[0].name, "matrix");
        assert_eq!(receipt.entries[0].dtype, DType::F32);

        let (loaded, loaded_receipt) = load_npy_path(&context, &path)?;
        assert_eq!(loaded_receipt.entries[0].shape, Shape::new(vec![2, 2]));
        assert_eq!(
            loaded.to_host_data()?.as_f32_slice(),
            Some(&[1.0, 2.0, 3.0, 4.0][..])
        );
        Ok(())
    }

    #[test]
    fn npz_roundtrip_preserves_named_bundle_and_i8_dtype() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = ArrayContext::cpu();
        let weights = context.constant_f32(Shape::new(vec![2]), vec![1.0, -3.0])?;
        let bias = context.scalar_f32(7.0)?.cast(DType::I8)?;
        let temp = tempdir()?;
        let path = temp.path().join("bundle.npz");

        save_npz_path(
            &path,
            &[
                ArrayExportRef::new("weights", &weights),
                ArrayExportRef::new("bias", &bias),
            ],
        )?;

        let (loaded, receipt) = load_npz_path(&context, &path)?;
        assert_eq!(receipt.entries.len(), 2);
        assert_eq!(
            loaded["weights"].to_host_data()?.as_f32_slice(),
            Some(&[1.0, -3.0][..])
        );
        assert_eq!(loaded["bias"].dtype(), DType::I8);
        assert_eq!(loaded["bias"].item()?.as_i8(), Some(7));
        Ok(())
    }

    #[test]
    fn safetensors_roundtrip_preserves_f16_bf16_and_i8_arrays()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = ArrayContext::cpu();
        let f16 = context
            .constant_f32(Shape::new(vec![2]), vec![1.5, -2.25])?
            .cast(DType::F16)?;
        let bf16 = context
            .constant_f32(Shape::new(vec![2]), vec![3.0, -4.0])?
            .cast(DType::BF16)?;
        let i8 = context
            .constant_f32(Shape::new(vec![2]), vec![5.0, -6.0])?
            .cast(DType::I8)?;
        let temp = tempdir()?;
        let path = temp.path().join("bundle.safetensors");

        save_safetensors_path(
            &path,
            &[
                ArrayExportRef::new("f16", &f16),
                ArrayExportRef::new("bf16", &bf16),
                ArrayExportRef::new("i8", &i8),
            ],
        )?;

        let (loaded, _) = load_safetensors_path(&context, &path)?;
        assert_eq!(loaded["f16"].dtype(), DType::F16);
        assert_eq!(loaded["bf16"].dtype(), DType::BF16);
        assert_eq!(loaded["i8"].dtype(), DType::I8);
        assert_eq!(
            loaded["i8"].to_host_data()?.as_i8_slice(),
            Some(&[5, -6][..])
        );
        Ok(())
    }

    #[test]
    fn gguf_roundtrip_preserves_dense_arrays_and_receipts() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = ArrayContext::cpu();
        let f32 = context.constant_f32(Shape::new(vec![2]), vec![0.25, -0.5])?;
        let f16 = context
            .constant_f32(Shape::new(vec![2]), vec![1.0, -2.0])?
            .cast(DType::F16)?;
        let temp = tempdir()?;
        let path = temp.path().join("bundle.gguf");

        let export_receipt = save_gguf_path(
            &path,
            &[
                ArrayExportRef::new("f32_tensor", &f32),
                ArrayExportRef::new("f16_tensor", &f16),
            ],
        )?;
        assert_eq!(export_receipt.entries.len(), 2);

        let (loaded, import_receipt) = load_gguf_path(&context, &path)?;
        assert_eq!(import_receipt.entries.len(), 2);
        assert_eq!(
            loaded["f32_tensor"].to_host_data()?.as_f32_slice(),
            Some(&[0.25, -0.5][..])
        );
        assert_eq!(loaded["f16_tensor"].dtype(), DType::F16);
        Ok(())
    }

    #[test]
    fn gguf_export_refuses_i8_arrays() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArrayContext::cpu();
        let array = context
            .constant_f32(Shape::new(vec![2]), vec![1.0, 2.0])?
            .cast(DType::I8)?;
        let temp = tempdir()?;
        let path = temp.path().join("bad.gguf");

        let error = match save_gguf_path(&path, &[ArrayExportRef::new("bad", &array)]) {
            Ok(_) => return Err("i8 gguf export should refuse".into()),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ArrayIoError::UnsupportedDType {
                format,
                dtype: DType::I8,
                ..
            } if format == "gguf"
        ));
        Ok(())
    }

    #[test]
    fn npy_load_refuses_fortran_order() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArrayContext::cpu();
        let temp = tempdir()?;
        let path = temp.path().join("fortran.npy");
        let bytes = b"\x93NUMPY\x01\x00F\x00{'descr': '<f4', 'fortran_order': True, 'shape': (2,), }            \n\x00\x00\x80?\x00\x00\x00@";
        std::fs::write(&path, bytes)?;

        let error = match load_npy_path(&context, &path) {
            Ok(_) => return Err("fortran order should refuse".into()),
            Err(error) => error,
        };
        assert!(matches!(error, ArrayIoError::InvalidArtifact { format, .. } if format == "npy"));
        Ok(())
    }

    #[test]
    fn gguf_decode_supports_in_memory_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let context = ArrayContext::cpu();
        let array = context.constant_f32(Shape::new(vec![2]), vec![10.0, 20.0])?;
        let (bytes, _) = super::encode_gguf(&[ArrayExportRef::new("weights", &array)])?;
        let (loaded, receipt) = decode_gguf(&context, bytes.as_slice())?;

        assert_eq!(receipt.entries[0].name, "weights");
        assert_eq!(
            loaded["weights"].to_host_data()?.as_f32_slice(),
            Some(&[10.0, 20.0][..])
        );
        Ok(())
    }
}
