use std::io::{Cursor, Read};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use rustc_hash::FxHashMap as HashMap;

#[derive(Debug)]
pub(crate) enum ParseError {
    Incomplete,
    Invalid(String),
}

#[derive(Clone, Debug)]
pub(crate) struct GgufTensor {
    pub(crate) name: String,
    pub(crate) ggml_type: u32,
    pub(crate) ggml_type_name: String,
    pub(crate) dims: Vec<u64>,
    pub(crate) offset: u64,
    pub(crate) absolute_offset: u64,
    pub(crate) nbytes: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct GgufIndex {
    pub(crate) version: u32,
    pub(crate) tensor_data_offset: u64,
    pub(crate) tensors: Vec<GgufTensor>,
    pub(crate) metadata: GgufMetadata,
}

#[derive(Clone, Debug)]
pub(crate) struct GgufTokenizer {
    pub(crate) tokens: Vec<String>,
    pub(crate) token_types: Vec<i32>,
    pub(crate) merges: Vec<String>,
    pub(crate) model: Option<String>,
    pub(crate) pre: Option<String>,
    pub(crate) chat_template: Option<String>,
    pub(crate) bos_token_id: Option<u32>,
    pub(crate) eos_token_id: Option<u32>,
    pub(crate) pad_token_id: Option<u32>,
    pub(crate) pattern: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum GgufScalar {
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

#[derive(Clone, Debug, Default)]
pub(crate) struct GgufMetadata {
    pub(crate) tokenizer: Option<Rc<GgufTokenizer>>,
    pub(crate) values: HashMap<String, GgufScalar>,
}

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

pub(crate) async fn fetch_and_parse_index(
    url: &str,
    mut fetch_len: u64,
    max_attempts: usize,
) -> Result<GgufIndex, String> {
    for _ in 0..max_attempts {
        let bytes = fetch_range(url, 0, fetch_len).await?;
        match parse_gguf_index(&bytes) {
            Ok(index) => return Ok(index),
            Err(ParseError::Incomplete) => {
                fetch_len = fetch_len.saturating_mul(2);
            }
            Err(ParseError::Invalid(msg)) => return Err(msg),
        }
    }
    Err("GGUF metadata parse incomplete after retries".to_string())
}

pub(crate) async fn fetch_range(url: &str, offset: u64, len: u64) -> Result<Vec<u8>, String> {
    if len == 0 {
        return Err("range length is zero".to_string());
    }

    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let end = offset.saturating_add(len.saturating_sub(1));
    let range_header = format!("bytes={offset}-{end}");

    let init = web_sys::RequestInit::new();
    init.set_method("GET");

    let headers = web_sys::Headers::new().map_err(js_err)?;
    headers.set("Range", &range_header).map_err(js_err)?;
    init.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(js_err)?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_err)?;
    let resp: web_sys::Response = resp_value.dyn_into().map_err(js_err)?;

    if !resp.ok() {
        return Err(format!("fetch failed: {}", resp.status()));
    }

    let buffer = JsFuture::from(resp.array_buffer().map_err(js_err)?)
        .await
        .map_err(js_err)?;
    let array = js_sys::Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    if bytes.len() > len as usize
        && resp.headers().get("Content-Range").map_err(js_err)?.is_none()
    {
        return Err("range request not honored by server".to_string());
    }
    Ok(bytes)
}

pub(crate) async fn fetch_range_with_total(
    url: &str,
    offset: u64,
    len: u64,
) -> Result<(Vec<u8>, Option<u64>), String> {
    if len == 0 {
        return Err("range length is zero".to_string());
    }

    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let end = offset.saturating_add(len.saturating_sub(1));
    let range_header = format!("bytes={offset}-{end}");

    let init = web_sys::RequestInit::new();
    init.set_method("GET");

    let headers = web_sys::Headers::new().map_err(js_err)?;
    headers.set("Range", &range_header).map_err(js_err)?;
    init.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(js_err)?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_err)?;
    let resp: web_sys::Response = resp_value.dyn_into().map_err(js_err)?;

    if !resp.ok() {
        return Err(format!("fetch failed: {}", resp.status()));
    }

    let content_range = resp.headers().get("Content-Range").map_err(js_err)?;
    let total = content_range
        .as_deref()
        .and_then(parse_content_range_total);

    let buffer = JsFuture::from(resp.array_buffer().map_err(js_err)?)
        .await
        .map_err(js_err)?;
    let array = js_sys::Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    if bytes.len() > len as usize && content_range.is_none() {
        return Err("range request not honored by server".to_string());
    }
    Ok((bytes, total))
}

fn parse_gguf_index(bytes: &[u8]) -> Result<GgufIndex, ParseError> {
    let mut cursor = Cursor::new(bytes);

    let magic = read_u32(&mut cursor)?;
    if magic != 0x4655_4747 && magic != 0x4747_5546 {
        return Err(ParseError::Invalid(format!(
            "invalid gguf magic: 0x{magic:08x}"
        )));
    }

    let version_raw = read_u32(&mut cursor)?;
    let version = match version_raw {
        1 | 2 | 3 => version_raw,
        _ => {
            return Err(ParseError::Invalid(format!(
                "unsupported gguf version: {version_raw}"
            )))
        }
    };

    let tensor_count = if version == 1 {
        read_u32(&mut cursor)? as u64
    } else {
        read_u64(&mut cursor)?
    };
    let kv_count = if version == 1 {
        read_u32(&mut cursor)? as u64
    } else {
        read_u64(&mut cursor)?
    };

    let metadata = parse_kv_entries(&mut cursor, kv_count, version)?;

    let tensor_count_usize = usize::try_from(tensor_count)
        .map_err(|_| ParseError::Invalid("tensor count too large".to_string()))?;
    let mut tensors = Vec::with_capacity(tensor_count_usize);
    for _ in 0..tensor_count_usize {
        let name = read_string(&mut cursor, version)?;
        let n_dims = read_u32(&mut cursor)?;
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            let dim = if version == 1 {
                read_u32(&mut cursor)? as u64
            } else {
                read_u64(&mut cursor)?
            };
            dims.push(dim);
        }
        dims.reverse();
        let ggml_type = read_u32(&mut cursor)?;
        let offset = read_u64(&mut cursor)?;
        tensors.push(GgufTensor {
            name,
            ggml_type,
            ggml_type_name: ggml_type_name(ggml_type).to_string(),
            dims,
            offset,
            absolute_offset: 0,
            nbytes: 0,
        });
    }

    let tensor_data_offset = align_offset(cursor.position(), 32);
    tensors.sort_by_key(|tensor| tensor.offset);
    for idx in 0..tensors.len() {
        let start = tensor_data_offset + tensors[idx].offset;
        let end = if idx + 1 < tensors.len() {
            tensor_data_offset + tensors[idx + 1].offset
        } else {
            bytes.len() as u64
        };
        tensors[idx].absolute_offset = start;
        tensors[idx].nbytes = end.saturating_sub(start);
    }

    Ok(GgufIndex {
        version,
        tensor_data_offset,
        tensors,
        metadata,
    })
}

fn parse_kv_entries<R: Read>(
    reader: &mut R,
    kv_count: u64,
    version: u32,
) -> Result<GgufMetadata, ParseError> {
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
        metadata.tokenizer = Some(Rc::new(tokenizer));
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
    version: u32,
) -> Result<String, ParseError> {
    if value_type != GGUF_VALUE_STRING {
        return Err(ParseError::Invalid(format!(
            "expected string value type, got {value_type}"
        )));
    }
    read_string(reader, version)
}

fn read_string_array<R: Read>(
    reader: &mut R,
    value_type: u32,
    version: u32,
) -> Result<Vec<String>, ParseError> {
    let len = read_array_header(reader, value_type, GGUF_VALUE_STRING)?;
    let len_usize = usize::try_from(len)
        .map_err(|_| ParseError::Invalid("string array too large".to_string()))?;
    let mut out = Vec::with_capacity(len_usize);
    for _ in 0..len_usize {
        out.push(read_string(reader, version)?);
    }
    Ok(out)
}

fn read_token_types<R: Read>(
    reader: &mut R,
    value_type: u32,
) -> Result<Vec<i32>, ParseError> {
    let (elem_type, len) = read_array_header_raw(reader, value_type)?;
    let len_usize = usize::try_from(len)
        .map_err(|_| ParseError::Invalid("token_types array too large".to_string()))?;
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
            return Err(ParseError::Invalid(format!(
                "unexpected token_type array element type: {elem_type}"
            )))
        }
    }
    Ok(out)
}

fn read_token_id<R: Read>(reader: &mut R, value_type: u32) -> Result<u32, ParseError> {
    match value_type {
        GGUF_VALUE_UINT32 => Ok(read_u32(reader)?),
        GGUF_VALUE_INT32 => {
            let val = read_i32(reader)?;
            if val < 0 {
                return Err(ParseError::Invalid("negative token id".to_string()));
            }
            Ok(val as u32)
        }
        GGUF_VALUE_UINT64 => {
            let val = read_u64(reader)?;
            if val > u64::from(u32::MAX) {
                return Err(ParseError::Invalid("token id too large".to_string()));
            }
            Ok(val as u32)
        }
        GGUF_VALUE_INT64 => {
            let val = read_i64(reader)?;
            if val < 0 || val > i64::from(u32::MAX) {
                return Err(ParseError::Invalid("token id out of range".to_string()));
            }
            Ok(val as u32)
        }
        _ => Err(ParseError::Invalid(format!(
            "unexpected token id type: {value_type}"
        ))),
    }
}

fn read_scalar_value<R: Read>(
    reader: &mut R,
    value_type: u32,
    version: u32,
) -> Result<Option<GgufScalar>, ParseError> {
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
            return Err(ParseError::Invalid(format!(
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
) -> Result<u64, ParseError> {
    let (elem_type, len) = read_array_header_raw(reader, value_type)?;
    if elem_type != expected_elem_type {
        return Err(ParseError::Invalid(format!(
            "unexpected array element type: {elem_type}"
        )));
    }
    Ok(len)
}

fn read_array_header_raw<R: Read>(
    reader: &mut R,
    value_type: u32,
) -> Result<(u32, u64), ParseError> {
    if value_type != GGUF_VALUE_ARRAY {
        return Err(ParseError::Invalid(format!(
            "expected array value type, got {value_type}"
        )));
    }
    let elem_type = read_u32(reader)?;
    let len = read_u64(reader)?;
    Ok((elem_type, len))
}

fn skip_array<R: Read>(
    reader: &mut R,
    elem_type: u32,
    len: u64,
    version: u32,
) -> Result<(), ParseError> {
    match elem_type {
        0 | 1 | 7 => skip_bytes(reader, len),
        2 | 3 => skip_bytes(reader, len * 2),
        4 | 5 | 6 => skip_bytes(reader, len * 4),
        10 | 11 | 12 => skip_bytes(reader, len * 8),
        8 => {
            for _ in 0..len {
                let slen = read_string_len(reader, version)?;
                skip_bytes(reader, slen)?;
            }
            Ok(())
        }
        _ => Err(ParseError::Invalid(format!(
            "unsupported gguf array type: {elem_type}"
        ))),
    }
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8, ParseError> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(buf[0])
}

fn read_i8<R: Read>(reader: &mut R) -> Result<i8, ParseError> {
    Ok(read_u8(reader)? as i8)
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16, ParseError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16<R: Read>(reader: &mut R) -> Result<i16, ParseError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, ParseError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i32<R: Read>(reader: &mut R) -> Result<i32, ParseError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, ParseError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64<R: Read>(reader: &mut R) -> Result<i64, ParseError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32<R: Read>(reader: &mut R) -> Result<f32, ParseError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64<R: Read>(reader: &mut R) -> Result<f64, ParseError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_string_len<R: Read>(reader: &mut R, version: u32) -> Result<u64, ParseError> {
    if version == 1 {
        Ok(read_u32(reader)? as u64)
    } else {
        read_u64(reader)
    }
}

fn read_string<R: Read>(reader: &mut R, version: u32) -> Result<String, ParseError> {
    let len = read_string_len(reader, version)?;
    if len > (usize::MAX as u64) {
        return Err(ParseError::Invalid("gguf string too long".to_string()));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    let value = String::from_utf8(buf)
        .map_err(|_| ParseError::Invalid("gguf string invalid utf8".to_string()))?;
    Ok(value)
}

fn skip_bytes<R: Read>(reader: &mut R, len: u64) -> Result<(), ParseError> {
    let mut remaining = len;
    let mut buf = [0u8; 1024];
    while remaining > 0 {
        let chunk = buf.len().min(remaining as usize);
        reader
            .read_exact(&mut buf[..chunk])
            .map_err(map_incomplete)?;
        remaining -= chunk as u64;
    }
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
        4 => "Q5_0",
        5 => "Q5_1",
        6 => "Q8_0",
        7 => "Q8_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "I8",
        17 => "I16",
        18 => "I32",
        19 => "I64",
        20 => "F64",
        21 => "Q4_0_4_4",
        22 => "Q4_0_4_8",
        23 => "Q4_0_8_8",
        24 => "Q4_1_4_4",
        25 => "Q4_1_4_8",
        26 => "Q4_1_8_8",
        27 => "Q4_2_4_4",
        28 => "Q4_2_4_8",
        29 => "Q4_2_8_8",
        30 => "Q8_0_2_8",
        31 => "Q2_K_S",
        32 => "Q3_K_S",
        33 => "Q4_K_S",
        34 => "Q5_K_S",
        35 => "Q6_K_S",
        36 => "Q8_K_S",
        37 => "Q2_K_M",
        38 => "Q3_K_M",
        39 => "MXFP4",
        40 => "Q5_K_M",
        41 => "Q6_K_M",
        42 => "Q8_K_M",
        43 => "IQ1_S",
        44 => "IQ2_XS",
        45 => "IQ2_XXS",
        46 => "IQ2_S",
        47 => "IQ3_XXS",
        48 => "IQ3_S",
        49 => "IQ4_XS",
        50 => "IQ4_NL",
        51 => "IQ5_S",
        52 => "IQ5_M",
        53 => "IQ6_XS",
        54 => "IQ6_S",
        55 => "IQ8_S",
        56 => "IQ1_M",
        57 => "BF16",
        _ => "UNKNOWN",
    }
}

fn map_incomplete(err: std::io::Error) -> ParseError {
    if err.kind() == std::io::ErrorKind::UnexpectedEof {
        ParseError::Incomplete
    } else {
        ParseError::Invalid(err.to_string())
    }
}

fn js_err(err: impl std::fmt::Debug) -> String {
    format!("{err:?}")
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    let mut parts = value.split('/');
    let _range = parts.next()?;
    let total = parts.next()?;
    if total == "*" {
        None
    } else {
        total.parse::<u64>().ok()
    }
}
