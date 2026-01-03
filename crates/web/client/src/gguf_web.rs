use std::io::{Cursor, Read};

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

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
}

#[derive(Clone, Debug)]
pub(crate) struct GgufIndex {
    pub(crate) version: u32,
    pub(crate) tensor_data_offset: u64,
    pub(crate) tensors: Vec<GgufTensor>,
}

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

    skip_kv_entries(&mut cursor, kv_count, version)?;

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
        });
    }

    let tensor_data_offset = align_offset(cursor.position(), 32);
    for tensor in &mut tensors {
        tensor.absolute_offset = tensor_data_offset + tensor.offset;
    }

    Ok(GgufIndex {
        version,
        tensor_data_offset,
        tensors,
    })
}

fn skip_kv_entries<R: Read>(
    reader: &mut R,
    kv_count: u64,
    version: u32,
) -> Result<(), ParseError> {
    for _ in 0..kv_count {
        let _key = read_string(reader, version)?;
        let value_type = read_u32(reader)?;
        skip_value(reader, value_type, version)?;
    }
    Ok(())
}

fn skip_value<R: Read>(reader: &mut R, value_type: u32, version: u32) -> Result<(), ParseError> {
    match value_type {
        0 | 1 | 7 => skip_bytes(reader, 1),
        2 | 3 => skip_bytes(reader, 2),
        4 | 5 | 6 => skip_bytes(reader, 4),
        10 | 11 | 12 => skip_bytes(reader, 8),
        8 => {
            let len = read_string_len(reader, version)?;
            skip_bytes(reader, len)?;
            Ok(())
        }
        9 => {
            let elem_type = read_u32(reader)?;
            let len = read_u64(reader)?;
            skip_array(reader, elem_type, len, version)
        }
        _ => Err(ParseError::Invalid(format!(
            "unsupported gguf value type: {value_type}"
        ))),
    }
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

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, ParseError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, ParseError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(map_incomplete)?;
    Ok(u64::from_le_bytes(buf))
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
        39 => "Q4_K_M",
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
