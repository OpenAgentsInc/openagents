use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::{GgufIndex, GgufMetadata, GgufScalar, GgufTensorDump, MlError, Result};

const Q8_0_BLOCK_BYTES: usize = 34;
const Q8_0_BLOCK_VALUES: usize = 32;
const MXFP4_BLOCK_BYTES: usize = 17;
const MXFP4_BLOCK_VALUES: usize = 32;
const MXFP4_VALUES: [f32; 16] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, -0.0, -0.5, -1.0, -1.5, -2.0, -3.0, -4.0,
    -6.0,
];
const ROPE_NTK_ALPHA: f32 = 1.0;
const ROPE_NTK_BETA: f32 = 32.0;

#[derive(Clone, Debug)]
pub struct LayerKvCache {
    pub k: Vec<f32>,
    pub v: Vec<f32>,
}

impl LayerKvCache {
    pub fn new() -> Self {
        Self { k: Vec::new(), v: Vec::new() }
    }

    pub fn token_count(&self, kv_heads: usize, head_dim: usize) -> usize {
        let stride = kv_heads.saturating_mul(head_dim);
        if stride == 0 {
            return 0;
        }
        self.k.len() / stride
    }

    pub fn append(
        &mut self,
        k: &[f32],
        v: &[f32],
        kv_heads: usize,
        head_dim: usize,
    ) -> Result<()> {
        let expected = kv_heads
            .checked_mul(head_dim)
            .ok_or_else(|| MlError::Model("kv append overflow".to_string()))?;
        if k.len() != expected || v.len() != expected {
            return Err(MlError::Model(format!(
                "kv append shape mismatch k={} v={} expected={expected}",
                k.len(),
                v.len()
            )));
        }
        self.k.extend_from_slice(k);
        self.v.extend_from_slice(v);
        Ok(())
    }

    pub fn memory_bytes(&self) -> usize {
        (self.k.len() + self.v.len()) * std::mem::size_of::<f32>()
    }
}

#[derive(Clone, Debug)]
pub struct KvCache {
    pub layers: Vec<LayerKvCache>,
    pub seq_len: usize,
    pub max_len: usize,
}

impl KvCache {
    pub fn new(layer_count: usize, max_len: usize) -> Self {
        let mut layers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            layers.push(LayerKvCache::new());
        }
        Self {
            layers,
            seq_len: 0,
            max_len,
        }
    }

    pub fn layer_mut(&mut self, layer: usize) -> Result<&mut LayerKvCache> {
        self.layers
            .get_mut(layer)
            .ok_or_else(|| MlError::Model(format!("kv cache missing layer {layer}")))
    }

    pub fn total_bytes(&self) -> usize {
        self.layers.iter().map(LayerKvCache::memory_bytes).sum()
    }
}

pub fn find_tensor<'a>(index: &'a GgufIndex, name: &str) -> Result<&'a GgufTensorDump> {
    index
        .tensors
        .iter()
        .find(|tensor| tensor.name == name)
        .ok_or_else(|| MlError::Model(format!("tensor not found: {name}")))
}

pub fn read_meta_u32(metadata: &GgufMetadata, key: &str) -> Result<u32> {
    let value = lookup_meta(metadata, key);
    if value.is_none() && key.ends_with("rope.dimension_count") {
        if let Ok(key_len) = read_meta_u32(metadata, "gpt-oss.attention.key_length") {
            return Ok(key_len);
        }
        if let Ok(value_len) = read_meta_u32(metadata, "gpt-oss.attention.value_length") {
            return Ok(value_len);
        }
        let embedding = read_meta_u32(metadata, "llama.embedding_length")?;
        let heads = read_meta_u32(metadata, "llama.attention.head_count")?;
        if heads > 0 {
            return Ok(embedding / heads);
        }
    }
    let value = value.ok_or_else(|| {
        MlError::Model(format!("missing gguf metadata key: {key}"))
    })?;
    match value {
        GgufScalar::U32(v) => Ok(*v),
        GgufScalar::I32(v) => Ok(*v as u32),
        GgufScalar::U64(v) => Ok(*v as u32),
        GgufScalar::I64(v) => Ok(*v as u32),
        GgufScalar::F32(v) => Ok(*v as u32),
        GgufScalar::F64(v) => Ok(*v as u32),
        _ => Err(MlError::Model(format!(
            "metadata {key} is not numeric"
        ))),
    }
}

pub fn read_meta_u32_optional(metadata: &GgufMetadata, key: &str) -> Option<u32> {
    let value = lookup_meta(metadata, key)?;
    let out = match value {
        GgufScalar::U32(v) => *v,
        GgufScalar::I32(v) => *v as u32,
        GgufScalar::U64(v) => *v as u32,
        GgufScalar::I64(v) => *v as u32,
        GgufScalar::F32(v) => *v as u32,
        GgufScalar::F64(v) => *v as u32,
        _ => return None,
    };
    Some(out)
}

pub fn read_meta_f32(metadata: &GgufMetadata, key: &str) -> Result<f32> {
    let value = lookup_meta(metadata, key).ok_or_else(|| {
        MlError::Model(format!("missing gguf metadata key: {key}"))
    })?;
    match value {
        GgufScalar::F32(v) => Ok(*v),
        GgufScalar::F64(v) => Ok(*v as f32),
        GgufScalar::U32(v) => Ok(*v as f32),
        GgufScalar::I32(v) => Ok(*v as f32),
        GgufScalar::U64(v) => Ok(*v as f32),
        GgufScalar::I64(v) => Ok(*v as f32),
        _ => Err(MlError::Model(format!(
            "metadata {key} is not numeric"
        ))),
    }
}

pub fn read_meta_f32_optional(metadata: &GgufMetadata, key: &str) -> Option<f32> {
    let value = lookup_meta(metadata, key)?;
    let out = match value {
        GgufScalar::F32(v) => *v,
        GgufScalar::F64(v) => *v as f32,
        GgufScalar::U32(v) => *v as f32,
        GgufScalar::I32(v) => *v as f32,
        GgufScalar::U64(v) => *v as f32,
        GgufScalar::I64(v) => *v as f32,
        _ => return None,
    };
    Some(out)
}

fn lookup_meta<'a>(metadata: &'a GgufMetadata, key: &str) -> Option<&'a GgufScalar> {
    if let Some(value) = metadata.values.get(key) {
        return Some(value);
    }
    if key == "llama.sliding_window" {
        return metadata.values.get("gpt-oss.attention.sliding_window");
    }
    if key == "gpt-oss.attention.sliding_window" {
        return metadata.values.get("llama.sliding_window");
    }
    let fallback = key
        .strip_prefix("llama.")
        .map(|rest| format!("gpt-oss.{rest}"))
        .or_else(|| key.strip_prefix("gpt-oss.").map(|rest| format!("llama.{rest}")))?;
    metadata.values.get(&fallback)
}

pub fn read_tensor_slice(path: &Path, offset: u64, len: usize) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_f32_tensor(path: &Path, tensor: &GgufTensorDump) -> Result<Vec<f32>> {
    if tensor.ggml_type != 0 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let bytes = read_tensor_slice(path, tensor.absolute_offset, tensor.nbytes as usize)?;
    if bytes.len() % 4 != 0 {
        return Err(MlError::Model(format!(
            "tensor {} f32 byte len mismatch",
            tensor.name
        )));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

/// Memory-mapped version of read_f32_tensor
pub fn read_f32_tensor_mmap(mmap: &[u8], tensor: &GgufTensorDump) -> Result<Vec<f32>> {
    if tensor.ggml_type != 0 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let offset = tensor.absolute_offset as usize;
    let nbytes = tensor.nbytes as usize;
    if offset + nbytes > mmap.len() {
        return Err(MlError::Model("tensor data out of bounds".to_string()));
    }
    let bytes = &mmap[offset..offset + nbytes];
    if bytes.len() % 4 != 0 {
        return Err(MlError::Model(format!(
            "tensor {} f32 byte len mismatch",
            tensor.name
        )));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

pub fn read_f32_row(path: &Path, tensor: &GgufTensorDump, row: usize) -> Result<Vec<f32>> {
    if tensor.ggml_type != 0 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(MlError::Model(format!(
            "row {row} out of range for {}",
            tensor.name
        )));
    }
    let row_bytes = cols * 4;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = read_tensor_slice(path, offset, row_bytes)?;
    let mut out = Vec::with_capacity(cols);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

/// Memory-mapped version of read_f32_row
pub fn read_f32_row_mmap(mmap: &[u8], tensor: &GgufTensorDump, row: usize) -> Result<Vec<f32>> {
    if tensor.ggml_type != 0 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(MlError::Model(format!(
            "row {row} out of range for {}",
            tensor.name
        )));
    }
    let row_bytes = cols * 4;
    let offset = tensor.absolute_offset as usize + row_bytes * row;
    if offset + row_bytes > mmap.len() {
        return Err(MlError::Model("tensor data out of bounds".to_string()));
    }
    let bytes = &mmap[offset..offset + row_bytes];
    let mut out = Vec::with_capacity(cols);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

pub fn read_q8_0_row(path: &Path, tensor: &GgufTensorDump, row: usize) -> Result<Vec<f32>> {
    if tensor.ggml_type != 8 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(MlError::Model(format!(
            "row {row} out of range for {}",
            tensor.name
        )));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model("q8_0 row cols not divisible by block size".to_string()));
    }
    let blocks_per_row = cols / Q8_0_BLOCK_VALUES;
    let row_bytes = blocks_per_row * Q8_0_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = read_tensor_slice(path, offset, row_bytes)?;
    dequant_q8_0(&bytes, cols)
}

/// Memory-mapped version of read_q8_0_row
pub fn read_q8_0_row_mmap(mmap: &[u8], tensor: &GgufTensorDump, row: usize) -> Result<Vec<f32>> {
    if tensor.ggml_type != 8 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(MlError::Model(format!(
            "row {row} out of range for {}",
            tensor.name
        )));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model("q8_0 row cols not divisible by block size".to_string()));
    }
    let blocks_per_row = cols / Q8_0_BLOCK_VALUES;
    let row_bytes = blocks_per_row * Q8_0_BLOCK_BYTES;
    let offset = tensor.absolute_offset as usize + row_bytes * row;
    if offset + row_bytes > mmap.len() {
        return Err(MlError::Model("tensor data out of bounds".to_string()));
    }
    dequant_q8_0(&mmap[offset..offset + row_bytes], cols)
}

pub fn matmul_q8_0(path: &Path, tensor: &GgufTensorDump, input: &[f32]) -> Result<Vec<f32>> {
    if tensor.ggml_type != 8 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if cols != input.len() || rows == 0 {
        return Err(MlError::Model(format!(
            "matmul shape mismatch rows={rows} cols={cols} input={}",
            input.len()
        )));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "q8_0 cols not divisible by block size".to_string(),
        ));
    }
    let row_bytes = (cols / Q8_0_BLOCK_VALUES) * Q8_0_BLOCK_BYTES;
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(tensor.absolute_offset))?;
    let mut out = vec![0.0f32; rows];
    let mut buf = vec![0u8; row_bytes];
    for row in 0..rows {
        file.read_exact(&mut buf)?;
        out[row] = dot_q8_0_row(&buf, input)?;
    }
    Ok(out)
}

/// Memory-mapped version of matmul_q8_0 - much faster than file I/O version
pub fn matmul_q8_0_mmap(mmap: &[u8], tensor: &GgufTensorDump, input: &[f32]) -> Result<Vec<f32>> {
    if tensor.ggml_type != 8 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let rows = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    if cols != input.len() || rows == 0 {
        return Err(MlError::Model(format!(
            "matmul shape mismatch rows={rows} cols={cols} input={}",
            input.len()
        )));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "q8_0 cols not divisible by block size".to_string(),
        ));
    }
    let row_bytes = (cols / Q8_0_BLOCK_VALUES) * Q8_0_BLOCK_BYTES;
    let offset = tensor.absolute_offset as usize;
    let total_bytes = rows * row_bytes;
    if offset + total_bytes > mmap.len() {
        return Err(MlError::Model("tensor data out of bounds".to_string()));
    }
    let mut out = vec![0.0f32; rows];
    for row in 0..rows {
        let start = offset + row * row_bytes;
        let end = start + row_bytes;
        out[row] = dot_q8_0_row(&mmap[start..end], input)?;
    }
    Ok(out)
}

pub fn read_mxfp4_expert(
    path: &Path,
    tensor: &GgufTensorDump,
    expert_idx: usize,
) -> Result<Vec<u8>> {
    if tensor.ggml_type != 39 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let experts = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let rows = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(2).copied().unwrap_or(0) as usize;
    if expert_idx >= experts || rows == 0 || cols == 0 {
        return Err(MlError::Model(format!(
            "expert {expert_idx} out of range for {}",
            tensor.name
        )));
    }
    let values = rows
        .checked_mul(cols)
        .ok_or_else(|| MlError::Model("mxfp4 value overflow".to_string()))?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "mxfp4 values not divisible by block size".to_string(),
        ));
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((expert_idx * bytes_needed) as u64);
    read_tensor_slice(path, offset, bytes_needed)
}

pub fn matmul_mxfp4_expert(
    path: &Path,
    tensor: &GgufTensorDump,
    expert_idx: usize,
    input: &[f32],
) -> Result<Vec<f32>> {
    let rows = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(2).copied().unwrap_or(0) as usize;
    if cols != input.len() || rows == 0 {
        return Err(MlError::Model(format!(
            "mxfp4 matmul shape mismatch rows={rows} cols={cols} input={}",
            input.len()
        )));
    }
    let quant = read_mxfp4_expert(path, tensor, expert_idx)?;
    let weights = dequant_mxfp4(&quant, rows * cols)?;
    Ok(matmul_f32(&weights, input, cols, rows))
}

/// Memory-mapped version of read_mxfp4_expert
pub fn read_mxfp4_expert_mmap(
    mmap: &[u8],
    tensor: &GgufTensorDump,
    expert_idx: usize,
) -> Result<Vec<u8>> {
    if tensor.ggml_type != 39 {
        return Err(MlError::Model(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        )));
    }
    let experts = tensor.dims.get(0).copied().unwrap_or(0) as usize;
    let rows = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(2).copied().unwrap_or(0) as usize;
    if expert_idx >= experts || rows == 0 || cols == 0 {
        return Err(MlError::Model(format!(
            "expert {expert_idx} out of range for {}",
            tensor.name
        )));
    }
    let values = rows
        .checked_mul(cols)
        .ok_or_else(|| MlError::Model("mxfp4 value overflow".to_string()))?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "mxfp4 values not divisible by block size".to_string(),
        ));
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    let offset = tensor.absolute_offset as usize + expert_idx * bytes_needed;
    if offset + bytes_needed > mmap.len() {
        return Err(MlError::Model("tensor data out of bounds".to_string()));
    }
    Ok(mmap[offset..offset + bytes_needed].to_vec())
}

/// Memory-mapped version of matmul_mxfp4_expert
pub fn matmul_mxfp4_expert_mmap(
    mmap: &[u8],
    tensor: &GgufTensorDump,
    expert_idx: usize,
    input: &[f32],
) -> Result<Vec<f32>> {
    let rows = tensor.dims.get(1).copied().unwrap_or(0) as usize;
    let cols = tensor.dims.get(2).copied().unwrap_or(0) as usize;
    if cols != input.len() || rows == 0 {
        return Err(MlError::Model(format!(
            "mxfp4 matmul shape mismatch rows={rows} cols={cols} input={}",
            input.len()
        )));
    }
    let quant = read_mxfp4_expert_mmap(mmap, tensor, expert_idx)?;
    let weights = dequant_mxfp4(&quant, rows * cols)?;
    Ok(matmul_f32(&weights, input, cols, rows))
}

pub fn dequant_mxfp4(data: &[u8], values: usize) -> Result<Vec<f32>> {
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "value count not divisible by MXFP4 block size".to_string(),
        ));
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let needed = blocks * MXFP4_BLOCK_BYTES;
    if data.len() < needed {
        return Err(MlError::Model(format!(
            "insufficient MXFP4 data: need {needed}, have {}",
            data.len()
        )));
    }
    let mut out = vec![0.0f32; values];
    for block in 0..blocks {
        let base = block * MXFP4_BLOCK_BYTES;
        let scale_byte = data[base];
        let scale = (2.0f32).powi(scale_byte as i32 - 127);
        for i in 0..MXFP4_BLOCK_VALUES {
            let byte = data[base + 1 + (i / 2)];
            let nibble = if i % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            let value = MXFP4_VALUES[nibble as usize] * scale;
            out[block * MXFP4_BLOCK_VALUES + i] = value;
        }
    }
    Ok(out)
}

pub fn matmul_f32(weights: &[f32], input: &[f32], k: usize, n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    for row in 0..n {
        let mut acc = 0.0f32;
        let row_base = row * k;
        for col in 0..k {
            acc += input[col] * weights[row_base + col];
        }
        out[row] = acc;
    }
    out
}

pub fn apply_bias(values: &mut [f32], bias: &[f32]) {
    if bias.len() != values.len() {
        return;
    }
    for (v, b) in values.iter_mut().zip(bias.iter()) {
        *v += *b;
    }
}

pub fn rms_norm(input: &[f32], weight: &[f32], eps: f32) -> Result<Vec<f32>> {
    if input.len() != weight.len() {
        return Err(MlError::Model("rms_norm shape mismatch".to_string()));
    }
    let mut sum_sq = 0.0f32;
    for v in input {
        sum_sq += v * v;
    }
    let mean = sum_sq / input.len().max(1) as f32;
    let inv = (mean + eps).sqrt().recip();
    let mut out = Vec::with_capacity(input.len());
    for (v, w) in input.iter().zip(weight.iter()) {
        out.push(v * inv * w);
    }
    Ok(out)
}

pub fn swiglu(gate: &[f32], up: &[f32]) -> Result<Vec<f32>> {
    if gate.len() != up.len() {
        return Err(MlError::Model("swiglu shape mismatch".to_string()));
    }
    let mut out = Vec::with_capacity(gate.len());
    for (&g, &u) in gate.iter().zip(up.iter()) {
        let g_clamped = g.min(7.0);
        let u_clamped = u.max(-7.0).min(7.0);
        let sigmoid = 1.0 / (1.0 + (-1.702 * g_clamped).exp());
        let glu = g_clamped * sigmoid;
        out.push(glu * (u_clamped + 1.0));
    }
    Ok(out)
}

pub fn apply_rope(
    values: &mut [f32],
    heads: usize,
    head_dim: usize,
    position: usize,
    theta: f32,
    rope_dim: u32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
) -> Result<()> {
    if head_dim == 0 || heads == 0 {
        return Err(MlError::Model("rope invalid head dims".to_string()));
    }
    let expected = heads
        .checked_mul(head_dim)
        .ok_or_else(|| MlError::Model("rope shape overflow".to_string()))?;
    if values.len() != expected {
        return Err(MlError::Model(format!(
            "rope shape mismatch values={} heads={} head_dim={}",
            values.len(),
            heads,
            head_dim
        )));
    }
    let rope_dim = rope_dim.min(head_dim as u32) as usize;
    if rope_dim == 0 {
        return Ok(());
    }
    if rope_dim % 2 != 0 {
        return Err(MlError::Model("rope_dim must be even".to_string()));
    }

    let theta = if theta <= 0.0 { 10000.0 } else { theta };
    let scaling_factor = rope_scaling_factor.max(1.0);
    let original_context = rope_scaling_original_context as f32;
    let use_yarn = scaling_factor > 1.0 && original_context > 0.0;
    let concentration = if use_yarn {
        0.1 * scaling_factor.ln() + 1.0
    } else {
        1.0
    };
    let theta_log = theta.ln();
    let d_half = rope_dim as f32 / 2.0;
    let mut low = 0.0f32;
    let mut high = 0.0f32;
    if use_yarn {
        let denom = theta_log.max(1e-6);
        low = d_half
            * (original_context / (ROPE_NTK_BETA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        high = d_half
            * (original_context / (ROPE_NTK_ALPHA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        if !(low > 0.0 && high > low && high < d_half - 1.0) {
            low = 0.0;
            high = 0.0;
        }
    }
    for h in 0..heads {
        let base = h * head_dim;
        for i in (0..rope_dim).step_by(2) {
            let idx = base + i;
            let idx2 = idx + 1;
            let freq = theta.powf(i as f32 / rope_dim as f32);
            let mut inv_freq = 1.0 / freq;
            if use_yarn && high > low {
                let t = (i / 2) as f32;
                let ramp = (t - low) / (high - low);
                let mask = 1.0 - ramp.clamp(0.0, 1.0);
                let interp = 1.0 / (scaling_factor * freq);
                let extrap = 1.0 / freq;
                inv_freq = interp * (1.0 - mask) + extrap * mask;
            }
            let angle = position as f32 * inv_freq;
            let (sin, cos) = angle.sin_cos();
            let sin = sin * concentration;
            let cos = cos * concentration;
            let v0 = values[idx];
            let v1 = values[idx2];
            values[idx] = v0 * cos - v1 * sin;
            values[idx2] = v0 * sin + v1 * cos;
        }
    }
    Ok(())
}

pub fn attention_with_cache(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err(MlError::Model("attention invalid dims".to_string()));
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| MlError::Model("attention stride overflow".to_string()))?;
    if cache.k.len() != cache.v.len() || cache.k.len() % stride != 0 {
        return Err(MlError::Model("attention cache shape mismatch".to_string()));
    }
    if q.len() != heads * head_dim {
        return Err(MlError::Model("attention q shape mismatch".to_string()));
    }

    let token_count = cache.k.len() / stride;
    if token_count == 0 {
        return Err(MlError::Model("attention cache empty".to_string()));
    }
    let window = window.max(1).min(token_count);
    let start = token_count.saturating_sub(window);
    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    let mut out = vec![0.0f32; heads * head_dim];

    for h in 0..heads {
        let q_base = h * head_dim;
        let kv = h % kv_heads;
        let sink = sinks.get(h).copied().unwrap_or(0.0);
        let mut max_score = sink;

        for t in start..token_count {
            let k_base = (t * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            if score > max_score {
                max_score = score;
            }
        }

        let mut weights = Vec::with_capacity(window);
        let mut denom = (sink - max_score).exp();
        for t in start..token_count {
            let k_base = (t * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            let w = (score - max_score).exp();
            weights.push(w);
            denom += w;
        }

        if denom <= 0.0 {
            return Err(MlError::Model("attention softmax denom is zero".to_string()));
        }

        for (idx, w) in weights.iter().enumerate() {
            let weight = *w / denom;
            let token_idx = start + idx;
            let v_base = (token_idx * kv_heads + kv) * head_dim;
            for i in 0..head_dim {
                out[q_base + i] += cache.v[v_base + i] * weight;
            }
        }
    }

    Ok(out)
}

pub fn attention_head_weights(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    head_index: usize,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err(MlError::Model("attention invalid dims".to_string()));
    }
    if head_index >= heads {
        return Err(MlError::Model("attention head index out of range".to_string()));
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| MlError::Model("attention stride overflow".to_string()))?;
    if cache.k.len() != cache.v.len() || cache.k.len() % stride != 0 {
        return Err(MlError::Model("attention cache shape mismatch".to_string()));
    }
    if q.len() != heads * head_dim {
        return Err(MlError::Model("attention q shape mismatch".to_string()));
    }

    let token_count = cache.k.len() / stride;
    if token_count == 0 {
        return Err(MlError::Model("attention cache empty".to_string()));
    }
    let window = window.max(1).min(token_count);
    let start = token_count.saturating_sub(window);
    let sm_scale = 1.0 / (head_dim as f32).sqrt();

    let q_base = head_index * head_dim;
    let kv = head_index % kv_heads;
    let sink = sinks.get(head_index).copied().unwrap_or(0.0);
    let mut max_score = sink;
    for t in start..token_count {
        let k_base = (t * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        if score > max_score {
            max_score = score;
        }
    }

    let mut weights = Vec::with_capacity(window);
    let mut denom = (sink - max_score).exp();
    for t in start..token_count {
        let k_base = (t * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        let w = (score - max_score).exp();
        weights.push(w);
        denom += w;
    }

    if denom <= 0.0 {
        return Err(MlError::Model("attention softmax denom is zero".to_string()));
    }
    for weight in &mut weights {
        *weight /= denom;
    }
    Ok(weights)
}

pub fn top_k_softmax(scores: &[f32], k: usize) -> Result<(Vec<usize>, Vec<f32>)> {
    if scores.is_empty() {
        return Err(MlError::Model("empty score list".to_string()));
    }
    let mut pairs: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let k = k.min(pairs.len());
    let mut top = pairs[..k].to_vec();
    let max = top[0].1;
    let mut sum = 0.0f32;
    for (_, v) in &top {
        sum += (*v - max).exp();
    }
    if sum <= 0.0 {
        return Err(MlError::Model("softmax sum is zero".to_string()));
    }
    let mut indices = Vec::with_capacity(k);
    let mut weights = Vec::with_capacity(k);
    for (idx, v) in top.drain(..) {
        indices.push(idx);
        weights.push((v - max).exp() / sum);
    }
    Ok((indices, weights))
}

pub fn dot_q8_0_row(data: &[u8], input: &[f32]) -> Result<f32> {
    if input.len() % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "input length not divisible by Q8_0 block size".to_string(),
        ));
    }
    let blocks = input.len() / Q8_0_BLOCK_VALUES;
    let needed = blocks * Q8_0_BLOCK_BYTES;
    if data.len() < needed {
        return Err(MlError::Model(format!(
            "insufficient Q8_0 data: need {needed}, have {}",
            data.len()
        )));
    }
    let mut acc = 0.0f32;
    for block in 0..blocks {
        let base = block * Q8_0_BLOCK_BYTES;
        let scale_bits = u16::from_le_bytes([data[base], data[base + 1]]);
        let scale = f16_to_f32(scale_bits);
        let input_base = block * Q8_0_BLOCK_VALUES;
        for i in 0..Q8_0_BLOCK_VALUES {
            let q = data[base + 2 + i] as i8;
            acc += input[input_base + i] * (scale * q as f32);
        }
    }
    Ok(acc)
}

pub fn dequant_q8_0(data: &[u8], values: usize) -> Result<Vec<f32>> {
    if values % Q8_0_BLOCK_VALUES != 0 {
        return Err(MlError::Model(
            "value count not divisible by Q8_0 block size".to_string(),
        ));
    }
    let blocks = values / Q8_0_BLOCK_VALUES;
    let needed = blocks * Q8_0_BLOCK_BYTES;
    if data.len() < needed {
        return Err(MlError::Model(format!(
            "insufficient Q8_0 data: need {needed}, have {}",
            data.len()
        )));
    }
    let mut out = vec![0.0f32; values];
    for block in 0..blocks {
        let base = block * Q8_0_BLOCK_BYTES;
        let scale_bits = u16::from_le_bytes([data[base], data[base + 1]]);
        let scale = f16_to_f32(scale_bits);
        for i in 0..Q8_0_BLOCK_VALUES {
            let q = data[base + 2 + i] as i8;
            out[block * Q8_0_BLOCK_VALUES + i] = scale * q as f32;
        }
    }
    Ok(out)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x03ff) as u32;
    let mut val = if exp == 0 {
        if frac == 0 {
            0.0
        } else {
            (frac as f32) * 2f32.powi(-24)
        }
    } else if exp == 31 {
        f32::INFINITY
    } else {
        (1.0 + (frac as f32) * 0.000_976_562_5) * 2f32.powi(exp - 15)
    };
    if sign == 1 {
        val = -val;
    }
    val
}
