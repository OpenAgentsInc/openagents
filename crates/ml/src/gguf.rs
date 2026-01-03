use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::collections::HashMap;

use serde::Serialize;

use crate::error::{MlError, Result};

const GGUF_MAGIC_LE: u32 = 0x4655_4747;
const GGUF_MAGIC_BE: u32 = 0x4747_5546;
const GGUF_ALIGN: u64 = 32;

const GGUF_VALUE_UINT8: u32 = 0;
const GGUF_VALUE_INT8: u32 = 1;
const GGUF_VALUE_UINT16: u32 = 2;
const GGUF_VALUE_INT16: u32 = 3;
const GGUF_VALUE_UINT32: u32 = 4;
const GGUF_VALUE_INT32: u32 = 5;
const GGUF_VALUE_FLOAT32: u32 = 6;
const GGUF_VALUE_BOOL: u32 = 7;
const GGUF_VALUE_STRING: u32 = 8;
const GGUF_VALUE_ARRAY: u32 = 9;
const GGUF_VALUE_UINT64: u32 = 10;
const GGUF_VALUE_INT64: u32 = 11;
const GGUF_VALUE_FLOAT64: u32 = 12;

const O200K_PATTERN: &str = "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]*[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]+[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)|\\p{N}{1,3}| ?[^\\s\\p{L}\\p{N}]+[\\r\\n/]*|\\s*[\\r\\n]+|\\s+(?!\\S)|\\s+";

#[derive(Debug, Clone, Copy)]
enum GgufVersion {
    V1,
    V2,
    V3,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufTensorDump {
    pub name: String,
    pub ggml_type: u32,
    pub ggml_type_name: String,
    pub dims: Vec<u64>,
    pub offset: u64,
    pub absolute_offset: u64,
    pub nbytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufIndex {
    pub version: u32,
    pub tensor_data_offset: u64,
    pub tensor_count: usize,
    pub tensors: Vec<GgufTensorDump>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufTokenizer {
    pub tokens: Vec<String>,
    pub token_types: Vec<i32>,
    pub merges: Vec<String>,
    pub model: Option<String>,
    pub pre: Option<String>,
    pub chat_template: Option<String>,
    pub bos_token_id: Option<u32>,
    pub eos_token_id: Option<u32>,
    pub pad_token_id: Option<u32>,
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum GgufScalar {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GgufMetadata {
    pub tokenizer: Option<GgufTokenizer>,
    pub values: HashMap<String, GgufScalar>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufModel {
    pub index: GgufIndex,
    pub metadata: GgufMetadata,
}

#[derive(Debug, Clone)]
struct GgufTensorInfo {
    name: String,
    ggml_type: u32,
    dims: Vec<u64>,
    offset: u64,
}

pub fn load_gguf_index(path: impl AsRef<Path>) -> Result<GgufIndex> {
    let path = path.as_ref();
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let magic = read_u32(&mut file)?;
    if magic != GGUF_MAGIC_LE && magic != GGUF_MAGIC_BE {
        return Err(MlError::Model(format!("invalid gguf magic: 0x{magic:08x}")));
    }

    let version_raw = read_u32(&mut file)?;
    let version = match version_raw {
        1 => GgufVersion::V1,
        2 => GgufVersion::V2,
        3 => GgufVersion::V3,
        _ => {
            return Err(MlError::Model(format!(
                "unsupported gguf version: {version_raw}"
            )))
        }
    };

    let tensor_count = match version {
        GgufVersion::V1 => read_u32(&mut file)? as u64,
        GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
    };
    let kv_count = match version {
        GgufVersion::V1 => read_u32(&mut file)? as u64,
        GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
    };

    skip_kv_entries(&mut file, kv_count, version)?;

    let tensor_count_usize = usize::try_from(tensor_count).map_err(|_| {
        MlError::Model(format!("tensor count too large: {tensor_count}"))
    })?;

    let mut tensors = Vec::with_capacity(tensor_count_usize);
    for _ in 0..tensor_count_usize {
        let name = read_string(&mut file, version)?;
        let n_dims = read_u32(&mut file)?;
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            let dim = match version {
                GgufVersion::V1 => read_u32(&mut file)? as u64,
                GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
            };
            dims.push(dim);
        }
        dims.reverse();
        let ggml_type = read_u32(&mut file)?;
        let offset = read_u64(&mut file)?;
        tensors.push(GgufTensorInfo {
            name,
            ggml_type,
            dims,
            offset,
        });
    }

    let tensor_data_offset = align_offset(file.seek(SeekFrom::Current(0))?, GGUF_ALIGN);

    let mut tensors_sorted = tensors;
    tensors_sorted.sort_by_key(|t| t.offset);

    let mut dumps = Vec::with_capacity(tensors_sorted.len());
    for (idx, tensor) in tensors_sorted.iter().enumerate() {
        let start = tensor_data_offset + tensor.offset;
        if start > file_size {
            return Err(MlError::Model(format!(
                "tensor {} offset beyond file size",
                tensor.name
            )));
        }
        let end = if idx + 1 < tensors_sorted.len() {
            tensor_data_offset + tensors_sorted[idx + 1].offset
        } else {
            file_size
        };
        let nbytes = end.saturating_sub(start);

        dumps.push(GgufTensorDump {
            name: tensor.name.clone(),
            ggml_type: tensor.ggml_type,
            ggml_type_name: ggml_type_name(tensor.ggml_type).to_string(),
            dims: tensor.dims.clone(),
            offset: tensor.offset,
            absolute_offset: start,
            nbytes,
        });
    }

    Ok(GgufIndex {
        version: version_raw,
        tensor_data_offset,
        tensor_count: dumps.len(),
        tensors: dumps,
    })
}

pub fn load_gguf_model(path: impl AsRef<Path>) -> Result<GgufModel> {
    let path = path.as_ref();
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let magic = read_u32(&mut file)?;
    if magic != GGUF_MAGIC_LE && magic != GGUF_MAGIC_BE {
        return Err(MlError::Model(format!("invalid gguf magic: 0x{magic:08x}")));
    }

    let version_raw = read_u32(&mut file)?;
    let version = match version_raw {
        1 => GgufVersion::V1,
        2 => GgufVersion::V2,
        3 => GgufVersion::V3,
        _ => {
            return Err(MlError::Model(format!(
                "unsupported gguf version: {version_raw}"
            )))
        }
    };

    let tensor_count = match version {
        GgufVersion::V1 => read_u32(&mut file)? as u64,
        GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
    };
    let kv_count = match version {
        GgufVersion::V1 => read_u32(&mut file)? as u64,
        GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
    };

    let metadata = parse_kv_entries(&mut file, kv_count, version)?;

    let tensor_count_usize = usize::try_from(tensor_count).map_err(|_| {
        MlError::Model(format!("tensor count too large: {tensor_count}"))
    })?;

    let mut tensors = Vec::with_capacity(tensor_count_usize);
    for _ in 0..tensor_count_usize {
        let name = read_string(&mut file, version)?;
        let n_dims = read_u32(&mut file)?;
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            let dim = match version {
                GgufVersion::V1 => read_u32(&mut file)? as u64,
                GgufVersion::V2 | GgufVersion::V3 => read_u64(&mut file)?,
            };
            dims.push(dim);
        }
        dims.reverse();
        let ggml_type = read_u32(&mut file)?;
        let offset = read_u64(&mut file)?;
        tensors.push(GgufTensorInfo {
            name,
            ggml_type,
            dims,
            offset,
        });
    }

    let tensor_data_offset = align_offset(file.seek(SeekFrom::Current(0))?, GGUF_ALIGN);

    let mut tensors_sorted = tensors;
    tensors_sorted.sort_by_key(|t| t.offset);

    let mut dumps = Vec::with_capacity(tensors_sorted.len());
    for (idx, tensor) in tensors_sorted.iter().enumerate() {
        let start = tensor_data_offset + tensor.offset;
        if start > file_size {
            return Err(MlError::Model(format!(
                "tensor {} offset beyond file size",
                tensor.name
            )));
        }
        let end = if idx + 1 < tensors_sorted.len() {
            tensor_data_offset + tensors_sorted[idx + 1].offset
        } else {
            file_size
        };
        let nbytes = end.saturating_sub(start);

        dumps.push(GgufTensorDump {
            name: tensor.name.clone(),
            ggml_type: tensor.ggml_type,
            ggml_type_name: ggml_type_name(tensor.ggml_type).to_string(),
            dims: tensor.dims.clone(),
            offset: tensor.offset,
            absolute_offset: start,
            nbytes,
        });
    }

    Ok(GgufModel {
        index: GgufIndex {
            version: version_raw,
            tensor_data_offset,
            tensor_count: dumps.len(),
            tensors: dumps,
        },
        metadata,
    })
}

fn parse_kv_entries<R: Read + Seek>(
    reader: &mut R,
    kv_count: u64,
    version: GgufVersion,
) -> Result<GgufMetadata> {
    let mut metadata = GgufMetadata::default();
    let mut tokenizer = TokenizerBuilder::default();
    for _ in 0..kv_count {
        let key = read_string(reader, version)?;
        let value_type = read_u32(reader)?;
        match key.as_str() {
            "tokenizer.ggml.tokens" => {
                tokenizer.tokens = Some(read_string_array(reader, value_type, version)?);
            }
            "tokenizer.ggml.token_type" => {
                tokenizer.token_types = Some(read_token_types(reader, value_type)?);
            }
            "tokenizer.ggml.merges" => {
                tokenizer.merges = Some(read_string_array(reader, value_type, version)?);
            }
            "tokenizer.ggml.model" => {
                tokenizer.model = Some(read_string_value(reader, value_type, version)?);
            }
            "tokenizer.ggml.pre" => {
                tokenizer.pre = Some(read_string_value(reader, value_type, version)?);
            }
            "tokenizer.chat_template" => {
                tokenizer.chat_template = Some(read_string_value(reader, value_type, version)?);
            }
            "tokenizer.ggml.bos_token_id" => {
                tokenizer.bos_token_id = Some(read_token_id(reader, value_type)?);
            }
            "tokenizer.ggml.eos_token_id" => {
                tokenizer.eos_token_id = Some(read_token_id(reader, value_type)?);
            }
            "tokenizer.ggml.pad_token_id" => {
                tokenizer.pad_token_id = Some(read_token_id(reader, value_type)?);
            }
            _ => {
                if let Some(value) = read_scalar_value(reader, value_type, version)? {
                    metadata.values.insert(key, value);
                }
            }
        }
    }

    if let Some(tokenizer) = tokenizer.build() {
        metadata.tokenizer = Some(tokenizer);
    }

    Ok(metadata)
}

#[derive(Default)]
struct TokenizerBuilder {
    tokens: Option<Vec<String>>,
    token_types: Option<Vec<i32>>,
    merges: Option<Vec<String>>,
    model: Option<String>,
    pre: Option<String>,
    chat_template: Option<String>,
    bos_token_id: Option<u32>,
    eos_token_id: Option<u32>,
    pad_token_id: Option<u32>,
}

impl TokenizerBuilder {
    fn build(self) -> Option<GgufTokenizer> {
        let tokens = self.tokens?;
        let token_types = self.token_types.unwrap_or_default();
        let merges = self.merges.unwrap_or_default();
        Some(GgufTokenizer {
            tokens,
            token_types,
            merges,
            model: self.model,
            pre: self.pre,
            chat_template: self.chat_template,
            bos_token_id: self.bos_token_id,
            eos_token_id: self.eos_token_id,
            pad_token_id: self.pad_token_id,
            pattern: O200K_PATTERN.to_string(),
        })
    }
}

fn read_string_value<R: Read>(
    reader: &mut R,
    value_type: u32,
    version: GgufVersion,
) -> Result<String> {
    if value_type != GGUF_VALUE_STRING {
        return Err(MlError::Model(format!(
            "expected string value type, got {value_type}"
        )));
    }
    read_string(reader, version)
}

fn read_string_array<R: Read>(
    reader: &mut R,
    value_type: u32,
    version: GgufVersion,
) -> Result<Vec<String>> {
    let len = read_array_header(reader, value_type, GGUF_VALUE_STRING)?;
    let len_usize = usize::try_from(len)
        .map_err(|_| MlError::Model("string array too large".to_string()))?;
    let mut out = Vec::with_capacity(len_usize);
    for _ in 0..len_usize {
        out.push(read_string(reader, version)?);
    }
    Ok(out)
}

fn read_token_types<R: Read>(reader: &mut R, value_type: u32) -> Result<Vec<i32>> {
    let (elem_type, len) = read_array_header_raw(reader, value_type)?;
    let len_usize = usize::try_from(len)
        .map_err(|_| MlError::Model("token_types array too large".to_string()))?;
    let mut out = Vec::with_capacity(len_usize);
    match elem_type {
        GGUF_VALUE_INT32 => {
            for _ in 0..len_usize {
                out.push(read_i32(reader)?);
            }
        }
        GGUF_VALUE_UINT32 => {
            for _ in 0..len_usize {
                let val = read_u32(reader)?;
                out.push(val as i32);
            }
        }
        _ => {
            return Err(MlError::Model(format!(
                "unexpected token_type array element type: {elem_type}"
            )))
        }
    }
    Ok(out)
}

fn read_token_id<R: Read>(reader: &mut R, value_type: u32) -> Result<u32> {
    match value_type {
        GGUF_VALUE_UINT32 => Ok(read_u32(reader)?),
        GGUF_VALUE_INT32 => {
            let val = read_i32(reader)?;
            if val < 0 {
                return Err(MlError::Model("negative token id".to_string()));
            }
            Ok(val as u32)
        }
        GGUF_VALUE_UINT64 => {
            let val = read_u64(reader)?;
            if val > u64::from(u32::MAX) {
                return Err(MlError::Model("token id too large".to_string()));
            }
            Ok(val as u32)
        }
        GGUF_VALUE_INT64 => {
            let val = read_i64(reader)?;
            if val < 0 || val > i64::from(u32::MAX) {
                return Err(MlError::Model("token id out of range".to_string()));
            }
            Ok(val as u32)
        }
        _ => Err(MlError::Model(format!(
            "unexpected token id type: {value_type}"
        ))),
    }
}

fn read_scalar_value<R: Read + Seek>(
    reader: &mut R,
    value_type: u32,
    version: GgufVersion,
) -> Result<Option<GgufScalar>> {
    let value = match value_type {
        GGUF_VALUE_UINT8 => Some(GgufScalar::U8(read_u8(reader)?)),
        GGUF_VALUE_INT8 => Some(GgufScalar::I8(read_i8(reader)?)),
        GGUF_VALUE_UINT16 => Some(GgufScalar::U16(read_u16(reader)?)),
        GGUF_VALUE_INT16 => Some(GgufScalar::I16(read_i16(reader)?)),
        GGUF_VALUE_UINT32 => Some(GgufScalar::U32(read_u32(reader)?)),
        GGUF_VALUE_INT32 => Some(GgufScalar::I32(read_i32(reader)?)),
        GGUF_VALUE_UINT64 => Some(GgufScalar::U64(read_u64(reader)?)),
        GGUF_VALUE_INT64 => Some(GgufScalar::I64(read_i64(reader)?)),
        GGUF_VALUE_FLOAT32 => Some(GgufScalar::F32(read_f32(reader)?)),
        GGUF_VALUE_FLOAT64 => Some(GgufScalar::F64(read_f64(reader)?)),
        GGUF_VALUE_BOOL => Some(GgufScalar::Bool(read_u8(reader)? != 0)),
        GGUF_VALUE_STRING => Some(GgufScalar::String(read_string(reader, version)?)),
        GGUF_VALUE_ARRAY => {
            let (elem_type, len) = read_array_header_raw(reader, value_type)?;
            skip_array(reader, elem_type, len, version)?;
            None
        }
        _ => {
            return Err(MlError::Model(format!(
                "unsupported gguf value type: {value_type}"
            )))
        }
    };
    Ok(value)
}

fn read_array_header<R: Read>(
    reader: &mut R,
    value_type: u32,
    expected_elem_type: u32,
) -> Result<u64> {
    let (elem_type, len) = read_array_header_raw(reader, value_type)?;
    if elem_type != expected_elem_type {
        return Err(MlError::Model(format!(
            "unexpected array element type: {elem_type}"
        )));
    }
    Ok(len)
}

fn read_array_header_raw<R: Read>(reader: &mut R, value_type: u32) -> Result<(u32, u64)> {
    if value_type != GGUF_VALUE_ARRAY {
        return Err(MlError::Model(format!(
            "expected array value type, got {value_type}"
        )));
    }
    let elem_type = read_u32(reader)?;
    let len = read_u64(reader)?;
    Ok((elem_type, len))
}

fn skip_kv_entries<R: Read + Seek>(
    reader: &mut R,
    kv_count: u64,
    version: GgufVersion,
) -> Result<()> {
    for _ in 0..kv_count {
        let _key = read_string(reader, version)?;
        let value_type = read_u32(reader)?;
        skip_value(reader, value_type, version)?;
    }
    Ok(())
}

fn skip_value<R: Read + Seek>(
    reader: &mut R,
    value_type: u32,
    version: GgufVersion,
) -> Result<()> {
    match value_type {
        GGUF_VALUE_UINT8 | GGUF_VALUE_INT8 | GGUF_VALUE_BOOL => skip_bytes(reader, 1),
        GGUF_VALUE_UINT16 | GGUF_VALUE_INT16 => skip_bytes(reader, 2),
        GGUF_VALUE_UINT32 | GGUF_VALUE_INT32 | GGUF_VALUE_FLOAT32 => skip_bytes(reader, 4),
        GGUF_VALUE_UINT64 | GGUF_VALUE_INT64 | GGUF_VALUE_FLOAT64 => skip_bytes(reader, 8),
        GGUF_VALUE_STRING => {
            let len = read_string_len(reader, version)?;
            skip_bytes(reader, len)?;
            Ok(())
        }
        GGUF_VALUE_ARRAY => {
            let elem_type = read_u32(reader)?;
            let len = read_u64(reader)?;
            skip_array(reader, elem_type, len, version)
        }
        _ => Err(MlError::Model(format!(
            "unsupported gguf value type: {value_type}"
        ))),
    }
}

fn skip_array<R: Read + Seek>(
    reader: &mut R,
    elem_type: u32,
    len: u64,
    version: GgufVersion,
) -> Result<()> {
    match elem_type {
        GGUF_VALUE_UINT8 | GGUF_VALUE_INT8 | GGUF_VALUE_BOOL => skip_bytes(reader, len),
        GGUF_VALUE_UINT16 | GGUF_VALUE_INT16 => skip_bytes(reader, len * 2),
        GGUF_VALUE_UINT32 | GGUF_VALUE_INT32 | GGUF_VALUE_FLOAT32 => skip_bytes(reader, len * 4),
        GGUF_VALUE_UINT64 | GGUF_VALUE_INT64 | GGUF_VALUE_FLOAT64 => skip_bytes(reader, len * 8),
        GGUF_VALUE_STRING => {
            for _ in 0..len {
                let str_len = read_string_len(reader, version)?;
                skip_bytes(reader, str_len)?;
            }
            Ok(())
        }
        GGUF_VALUE_ARRAY => {
            for _ in 0..len {
                let nested_type = read_u32(reader)?;
                let nested_len = read_u64(reader)?;
                skip_array(reader, nested_type, nested_len, version)?;
            }
            Ok(())
        }
        _ => Err(MlError::Model(format!(
            "unsupported gguf array element type: {elem_type}"
        ))),
    }
}

fn read_string<R: Read>(reader: &mut R, version: GgufVersion) -> Result<String> {
    let len = read_string_len(reader, version)?;
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    while let Some(0) = buf.last() {
        buf.pop();
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn read_string_len<R: Read>(reader: &mut R, version: GgufVersion) -> Result<u64> {
    match version {
        GgufVersion::V1 => Ok(read_u32(reader)? as u64),
        GgufVersion::V2 | GgufVersion::V3 => Ok(read_u64(reader)?),
    }
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_i8<R: Read>(reader: &mut R) -> Result<i8> {
    Ok(read_u8(reader)? as i8)
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16<R: Read>(reader: &mut R) -> Result<i16> {
    Ok(read_u16(reader)? as i16)
}

fn read_i32<R: Read>(reader: &mut R) -> Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_i64<R: Read>(reader: &mut R) -> Result<i64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32<R: Read>(reader: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64<R: Read>(reader: &mut R) -> Result<f64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn skip_bytes<R: Read + Seek>(reader: &mut R, len: u64) -> Result<()> {
    reader.seek(SeekFrom::Current(len as i64))?;
    Ok(())
}

fn align_offset(offset: u64, align: u64) -> u64 {
    if align == 0 {
        return offset;
    }
    let rem = offset % align;
    if rem == 0 {
        offset
    } else {
        offset + (align - rem)
    }
}

fn ggml_type_name(type_id: u32) -> &'static str {
    match type_id {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        4 => "Q4_2",
        5 => "Q4_3",
        6 => "Q5_0",
        7 => "Q5_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "IQ2_XXS",
        17 => "IQ2_XS",
        18 => "IQ3_XXS",
        19 => "IQ1_S",
        20 => "IQ4_NL",
        21 => "IQ3_S",
        22 => "IQ2_S",
        23 => "IQ4_XS",
        24 => "I8",
        25 => "I16",
        26 => "I32",
        27 => "I64",
        28 => "F64",
        29 => "IQ1_M",
        30 => "BF16",
        _ => "unknown",
    }
}
