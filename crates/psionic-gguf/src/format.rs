//! GGUF v2/v3 header and tensor-index parser (no weight copy).

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

pub const MAGIC: [u8; 4] = *b"GGUF";
pub const DEFAULT_ALIGNMENT: u64 = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum GgufValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    String(String),
    Array(Vec<GgufValue>),
    U64(u64),
    I64(i64),
    F64(f64),
}

impl GgufValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U8(v) => Some(*v as u64),
            Self::U16(v) => Some(*v as u64),
            Self::U32(v) => Some(*v as u64),
            Self::U64(v) => Some(*v),
            Self::I8(v) if *v >= 0 => Some(*v as u64),
            Self::I16(v) if *v >= 0 => Some(*v as u64),
            Self::I32(v) if *v >= 0 => Some(*v as u64),
            Self::I64(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }

    pub fn as_string_array(&self) -> Option<Vec<&str>> {
        match self {
            Self::Array(items) => items.iter().map(|v| v.as_str()).collect::<Option<Vec<_>>>(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TensorInfo {
    pub name: String,
    pub dims: Vec<u64>,
    pub ggml_type: u32,
    pub offset: u64,
}

impl TensorInfo {
    pub fn nbytes(&self) -> Option<u64> {
        let n_elem = self.dims.iter().try_fold(1u64, |a, d| a.checked_mul(*d))?;
        match self.ggml_type {
            8 => {
                // Q8_0: 32 elements per 34-byte block (f16 scale + 32 i8).
                let blocks = n_elem.div_ceil(32);
                Some(blocks.saturating_mul(34))
            }
            _ => element_size(self.ggml_type).map(|sz| n_elem.saturating_mul(sz)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GgufMeta {
    pub version: u32,
    pub n_tensors: u64,
    pub n_kv: u64,
    pub kv: Vec<(String, GgufValue)>,
    pub tensors: Vec<TensorInfo>,
    pub data_offset: u64,
    pub alignment: u64,
    pub file_size: u64,
}

impl GgufMeta {
    pub fn get(&self, key: &str) -> Option<&GgufValue> {
        self.kv.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn architecture(&self) -> Option<&str> {
        self.get("general.architecture").and_then(GgufValue::as_str)
    }

    pub fn kv_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(GgufValue::as_u64)
    }
}

#[derive(Debug)]
pub enum ParseError {
    Magic { got: Vec<u8> },
    Version(u32),
    Truncated(&'static str),
    Trailing,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Magic { got } => write!(f, "magic {:?}", got),
            Self::Version(v) => write!(f, "version {v}"),
            Self::Truncated(what) => write!(f, "truncated {what}"),
            Self::Trailing => write!(f, "trailing bytes in header"),
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse_path(path: &Path) -> Result<GgufMeta, ParseError> {
    let mut file = File::open(path).map_err(|_| ParseError::Truncated("file"))?;
    let file_size = file
        .metadata()
        .map_err(|_| ParseError::Truncated("stat"))?
        .len();
    const HEADER_CAP: u64 = 64 * 1024 * 1024;
    let to_read = file_size.min(HEADER_CAP) as usize;
    let mut buf = vec![0u8; to_read];
    file.read_exact(&mut buf)
        .map_err(|_| ParseError::Truncated("header"))?;
    let mut meta = parse_bytes(&buf)?;
    if meta.data_offset > buf.len() as u64 {
        return Err(ParseError::Truncated("header cap"));
    }
    meta.file_size = file_size;
    Ok(meta)
}

pub fn parse_bytes(bytes: &[u8]) -> Result<GgufMeta, ParseError> {
    if bytes.len() < 4 {
        return Err(ParseError::Magic {
            got: bytes.to_vec(),
        });
    }
    if bytes[0..4] != MAGIC {
        return Err(ParseError::Magic {
            got: bytes[0..4].to_vec(),
        });
    }
    let mut cur = Cursor::new(bytes);
    cur.set_position(4);
    let version = read_u32(&mut cur)?;
    if version < 2 || version > 3 {
        return Err(ParseError::Version(version));
    }
    let n_tensors = read_u64(&mut cur)?;
    let n_kv = read_u64(&mut cur)?;
    let mut kv = Vec::with_capacity(n_kv as usize);
    for _ in 0..n_kv {
        let key = read_string(&mut cur)?;
        let value = read_value(&mut cur)?;
        kv.push((key, value));
    }
    let mut tensors = Vec::with_capacity(n_tensors as usize);
    for _ in 0..n_tensors {
        let name = read_string(&mut cur)?;
        let n_dims = read_u32(&mut cur)?;
        if n_dims > 4 {
            return Err(ParseError::Truncated("n_dims"));
        }
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            dims.push(read_u64(&mut cur)?);
        }
        let ggml_type = read_u32(&mut cur)?;
        let offset = read_u64(&mut cur)?;
        tensors.push(TensorInfo {
            name,
            dims,
            ggml_type,
            offset,
        });
    }
    let alignment = kv
        .iter()
        .find(|(k, _)| k == "general.alignment")
        .and_then(|(_, v)| v.as_u64())
        .unwrap_or(DEFAULT_ALIGNMENT);
    if alignment == 0 {
        return Err(ParseError::Truncated("alignment"));
    }
    let pos = cur.position();
    let data_offset = pos.div_ceil(alignment) * alignment;
    Ok(GgufMeta {
        version,
        n_tensors,
        n_kv,
        kv,
        tensors,
        data_offset,
        alignment,
        file_size: bytes.len() as u64,
    })
}

fn read_u32(cur: &mut Cursor<&[u8]>) -> Result<u32, ParseError> {
    let mut buf = [0u8; 4];
    cur.read_exact(&mut buf)
        .map_err(|_| ParseError::Truncated("u32"))?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(cur: &mut Cursor<&[u8]>) -> Result<u64, ParseError> {
    let mut buf = [0u8; 8];
    cur.read_exact(&mut buf)
        .map_err(|_| ParseError::Truncated("u64"))?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i32(cur: &mut Cursor<&[u8]>) -> Result<i32, ParseError> {
    Ok(read_u32(cur)? as i32)
}

fn read_string(cur: &mut Cursor<&[u8]>) -> Result<String, ParseError> {
    let len = read_u64(cur)? as usize;
    let mut buf = vec![0u8; len];
    cur.read_exact(&mut buf)
        .map_err(|_| ParseError::Truncated("string"))?;
    String::from_utf8(buf).map_err(|_| ParseError::Truncated("utf8"))
}

fn read_value(cur: &mut Cursor<&[u8]>) -> Result<GgufValue, ParseError> {
    let ty = read_i32(cur)?;
    read_typed(cur, ty)
}

fn read_typed(cur: &mut Cursor<&[u8]>, ty: i32) -> Result<GgufValue, ParseError> {
    match ty {
        0 => Ok(GgufValue::U8(read_exact::<1>(cur)?[0])),
        1 => Ok(GgufValue::I8(read_exact::<1>(cur)?[0] as i8)),
        2 => {
            let b = read_exact::<2>(cur)?;
            Ok(GgufValue::U16(u16::from_le_bytes(b)))
        }
        3 => {
            let b = read_exact::<2>(cur)?;
            Ok(GgufValue::I16(i16::from_le_bytes(b)))
        }
        4 => Ok(GgufValue::U32(read_u32(cur)?)),
        5 => Ok(GgufValue::I32(read_i32(cur)?)),
        6 => {
            let b = read_exact::<4>(cur)?;
            Ok(GgufValue::F32(f32::from_le_bytes(b)))
        }
        7 => Ok(GgufValue::Bool(read_exact::<1>(cur)?[0] != 0)),
        8 => Ok(GgufValue::String(read_string(cur)?)),
        9 => {
            let elem = read_i32(cur)?;
            let n = read_u64(cur)?;
            let mut items = Vec::with_capacity(n as usize);
            for _ in 0..n {
                items.push(read_typed(cur, elem)?);
            }
            Ok(GgufValue::Array(items))
        }
        10 => Ok(GgufValue::U64(read_u64(cur)?)),
        11 => {
            let b = read_exact::<8>(cur)?;
            Ok(GgufValue::I64(i64::from_le_bytes(b)))
        }
        12 => {
            let b = read_exact::<8>(cur)?;
            Ok(GgufValue::F64(f64::from_le_bytes(b)))
        }
        _ => Err(ParseError::Truncated("value type")),
    }
}

fn read_exact<const N: usize>(cur: &mut Cursor<&[u8]>) -> Result<[u8; N], ParseError> {
    let mut buf = [0u8; N];
    cur.read_exact(&mut buf)
        .map_err(|_| ParseError::Truncated("bytes"))?;
    Ok(buf)
}

pub fn ggml_type_name(ty: u32) -> &'static str {
    match ty {
        0 => "f32",
        1 => "f16",
        2 => "q4_0",
        3 => "q4_1",
        6 => "q5_0",
        7 => "q5_1",
        8 => "q8_0",
        12 => "q6_k",
        14 => "q4_k",
        15 => "q5_k",
        16 => "q3_k",
        _ => "other",
    }
}

fn element_size(ty: u32) -> Option<u64> {
    match ty {
        0 => Some(4),
        1 => Some(2),
        _ => None,
    }
}

/// Write a tiny admitted Qwen 3.8-family GGUF for tests. Padded to 16 KiB so
/// Metal `newBufferWithBytesNoCopy` can wrap a page-sized mapping.
pub fn write_qwen35_fixture(path: &Path) -> std::io::Result<u64> {
    let mut body = Vec::new();
    body.extend_from_slice(&MAGIC);
    body.extend_from_slice(&3u32.to_le_bytes());

    let tokens = ["a", "b", "c", "d"];
    let merges = ["a b"];
    let tensors: &[(&str, &[u64])] = &[
        ("token_embd.weight", &[8, 4]),
        ("output.weight", &[8, 4]),
        ("output_norm.weight", &[8]),
        ("blk.0.attn_norm.weight", &[8]),
    ];

    let kv: Vec<(&str, GgufValue)> = vec![
        ("general.architecture", GgufValue::String("qwen35".into())),
        ("general.file_type", GgufValue::U32(0)),
        ("qwen35.block_count", GgufValue::U32(1)),
        ("qwen35.embedding_length", GgufValue::U32(8)),
        ("qwen35.feed_forward_length", GgufValue::U32(16)),
        ("qwen35.context_length", GgufValue::U32(32)),
        ("qwen35.full_attention_interval", GgufValue::U32(4)),
        ("qwen35.nextn.predict_layers", GgufValue::U32(0)),
        ("tokenizer.ggml.model", GgufValue::String("gpt2".into())),
        (
            "tokenizer.ggml.tokens",
            GgufValue::Array(
                tokens
                    .iter()
                    .map(|t| GgufValue::String((*t).into()))
                    .collect(),
            ),
        ),
        (
            "tokenizer.ggml.merges",
            GgufValue::Array(
                merges
                    .iter()
                    .map(|t| GgufValue::String((*t).into()))
                    .collect(),
            ),
        ),
        ("tokenizer.ggml.bos_token_id", GgufValue::U32(0)),
        ("tokenizer.ggml.eos_token_id", GgufValue::U32(1)),
    ];

    body.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
    body.extend_from_slice(&(kv.len() as u64).to_le_bytes());
    for (key, value) in &kv {
        write_string(&mut body, key);
        write_value(&mut body, value);
    }

    let mut payloads: Vec<Vec<u8>> = Vec::new();
    let mut offset = 0u64;
    let mut infos = Vec::new();
    for (name, dims) in tensors {
        let n: u64 = dims.iter().product();
        let bytes = vec![0u8; (n * 4) as usize];
        infos.push((name.to_string(), dims.to_vec(), offset));
        offset += bytes.len() as u64;
        payloads.push(bytes);
    }
    for (name, dims, off) in &infos {
        write_string(&mut body, name);
        body.extend_from_slice(&(dims.len() as u32).to_le_bytes());
        for d in dims {
            body.extend_from_slice(&d.to_le_bytes());
        }
        body.extend_from_slice(&0u32.to_le_bytes()); // F32
        body.extend_from_slice(&off.to_le_bytes());
    }

    while (body.len() as u64) % DEFAULT_ALIGNMENT != 0 {
        body.push(0);
    }
    for payload in payloads {
        body.extend_from_slice(&payload);
    }
    const PAGE: usize = 16384;
    if body.len() < PAGE {
        body.resize(PAGE, 0);
    }
    std::fs::write(path, &body)?;
    Ok(body.len() as u64)
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

fn write_value(buf: &mut Vec<u8>, value: &GgufValue) {
    match value {
        GgufValue::U32(v) => {
            buf.extend_from_slice(&4i32.to_le_bytes());
            buf.extend_from_slice(&v.to_le_bytes());
        }
        GgufValue::String(s) => {
            buf.extend_from_slice(&8i32.to_le_bytes());
            write_string(buf, s);
        }
        GgufValue::Array(items) => {
            buf.extend_from_slice(&9i32.to_le_bytes());
            buf.extend_from_slice(&8i32.to_le_bytes()); // string elements
            buf.extend_from_slice(&(items.len() as u64).to_le_bytes());
            for item in items {
                match item {
                    GgufValue::String(s) => write_string(buf, s),
                    _ => panic!("fixture arrays are strings"),
                }
            }
        }
        _ => panic!("unsupported fixture value"),
    }
}
