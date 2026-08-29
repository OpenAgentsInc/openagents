//! Fixture-sized F32 embed → norm → lm-head, plus Q8_0 row/matvec so a
//! 27B-class file can finish `gen.done` without `psionic-serve`.

use crate::mmap::MappedWeights;
use crate::tokenizer::TokenizerTables;

const Q8_BLOCK: usize = 34;
const Q8_K: usize = 32;

pub fn embed_token(mapped: &MappedWeights, token: u32, width: usize) -> Option<Vec<f32>> {
    decode_row(mapped, "token_embd.weight", token as usize, width)
        .or_else(|| decode_row(mapped, "token_embd.weight", 0, width))
}

pub(crate) fn lookup<'a>(
    mapped: &'a MappedWeights,
    name: &str,
) -> Option<&'a crate::mmap::TensorView> {
    mapped.tensors.get(name)
}

pub fn prefill_hidden(mapped: &MappedWeights, tokens: &[u32], width: usize) -> Option<Vec<f32>> {
    let last = *tokens.last()?;
    embed_token(mapped, last, width)
}

pub fn greedy_from_hidden(
    mapped: &MappedWeights,
    hidden: &[f32],
    tok: &TokenizerTables,
) -> Option<(u32, String)> {
    let w = decode_row(mapped, "output_norm.weight", 0, hidden.len())
        .unwrap_or_else(|| vec![1f32; hidden.len()]);
    let normed = rmsnorm(hidden, &w);
    let logits = matvec(mapped, "output.weight", &normed)?;
    let (id, _) = logits
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;
    let id = id as u32;
    Some((id, tok.token_piece(id)))
}

pub(crate) fn decode_row(
    mapped: &MappedWeights,
    name: &str,
    row: usize,
    width: usize,
) -> Option<Vec<f32>> {
    let view = mapped.tensors.get(name)?;
    let src = unsafe { std::slice::from_raw_parts(view.data, view.len) };
    match view.info.ggml_type {
        0 => decode_f32_row(src, row, width),
        8 => decode_q8_row(src, row, width),
        _ => None,
    }
}

fn decode_f32_row(src: &[u8], row: usize, width: usize) -> Option<Vec<f32>> {
    let row_bytes = width.checked_mul(4)?;
    let start = row.checked_mul(row_bytes)?;
    if start + row_bytes > src.len() {
        return None;
    }
    let mut out = vec![0f32; width];
    for (i, slot) in out.iter_mut().enumerate() {
        let off = start + i * 4;
        *slot = f32::from_le_bytes(src[off..off + 4].try_into().ok()?);
    }
    Some(out)
}

fn decode_q8_row(src: &[u8], row: usize, width: usize) -> Option<Vec<f32>> {
    let blocks = width.div_ceil(Q8_K);
    let row_bytes = blocks * Q8_BLOCK;
    let start = row.checked_mul(row_bytes)?;
    if start + row_bytes > src.len() {
        return None;
    }
    let mut out = vec![0f32; width];
    for b in 0..blocks {
        let off = start + b * Q8_BLOCK;
        let d = f16_to_f32(src[off..off + 2].try_into().ok()?);
        for j in 0..Q8_K {
            let i = b * Q8_K + j;
            if i >= width {
                break;
            }
            out[i] = d * (src[off + 2 + j] as i8 as f32);
        }
    }
    Some(out)
}

pub(crate) fn matvec(mapped: &MappedWeights, name: &str, x: &[f32]) -> Option<Vec<f32>> {
    let view = mapped.tensors.get(name)?;
    let src = unsafe { std::slice::from_raw_parts(view.data, view.len) };
    match view.info.ggml_type {
        0 => matvec_f32(src, x),
        8 => matvec_q8(src, x),
        _ => None,
    }
}

fn matvec_f32(src: &[u8], x: &[f32]) -> Option<Vec<f32>> {
    let width = x.len();
    if width == 0 || src.len() < width * 4 {
        return None;
    }
    let rows = src.len() / (width * 4);
    let mut out = vec![0f32; rows];
    for r in 0..rows {
        let mut acc = 0f32;
        let base = r * width * 4;
        for (i, xv) in x.iter().enumerate() {
            let w = f32::from_le_bytes(src[base + i * 4..base + i * 4 + 4].try_into().ok()?);
            acc += w * xv;
        }
        out[r] = acc;
    }
    Some(out)
}

fn matvec_q8(src: &[u8], x: &[f32]) -> Option<Vec<f32>> {
    let width = x.len();
    if width == 0 {
        return None;
    }
    let blocks = width.div_ceil(Q8_K);
    let row_bytes = blocks * Q8_BLOCK;
    if row_bytes == 0 || src.len() < row_bytes {
        return None;
    }
    let rows = src.len() / row_bytes;
    let mut out = vec![0f32; rows];
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8);
    if rows < 64 || threads == 1 {
        for (r, slot) in out.iter_mut().enumerate() {
            *slot = q8_row_dot(src, r, row_bytes, blocks, width, x);
        }
        return Some(out);
    }
    let chunk = rows.div_ceil(threads);
    std::thread::scope(|scope| {
        for (t, slot) in out.chunks_mut(chunk).enumerate() {
            let start = t * chunk;
            scope.spawn(move || {
                for (i, dest) in slot.iter_mut().enumerate() {
                    *dest = q8_row_dot(src, start + i, row_bytes, blocks, width, x);
                }
            });
        }
    });
    Some(out)
}

fn q8_row_dot(
    src: &[u8],
    row: usize,
    row_bytes: usize,
    blocks: usize,
    width: usize,
    x: &[f32],
) -> f32 {
    let start = row * row_bytes;
    if start + row_bytes > src.len() {
        return 0.0;
    }
    let mut acc = 0f32;
    for b in 0..blocks {
        let off = start + b * Q8_BLOCK;
        let d = f16_to_f32([src[off], src[off + 1]]);
        for j in 0..Q8_K {
            let i = b * Q8_K + j;
            if i >= width {
                break;
            }
            acc += d * (src[off + 2 + j] as i8 as f32) * x[i];
        }
    }
    acc
}

pub(crate) fn rmsnorm(x: &[f32], w: &[f32]) -> Vec<f32> {
    let mut ss = 0f32;
    for v in x {
        ss += *v * *v;
    }
    let scale = (ss / x.len() as f32 + 1e-6).sqrt().recip();
    x.iter()
        .zip(w.iter().chain(std::iter::repeat(&1f32)))
        .map(|(v, g)| v * scale * g)
        .collect()
}

pub(crate) fn f16_to_f32(bytes: [u8; 2]) -> f32 {
    let h = u16::from_le_bytes(bytes);
    let sign = u32::from(h >> 15);
    let exp = u32::from((h >> 10) & 0x1f);
    let frac = u32::from(h & 0x3ff);
    let bits = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            let mut e = -14i32;
            let mut m = frac;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3ff;
            (sign << 31) | (((e + 127) as u32) << 23) | (m << 13)
        }
    } else if exp == 31 {
        (sign << 31) | (0xff << 23) | (frac << 13)
    } else {
        (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13)
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::write_qwen35_fixture;
    use crate::mmap::map_file;
    use crate::parse_path;
    use crate::tokenizer::load_tokenizer;

    #[test]
    fn fixture_greedy_is_deterministic() {
        let dir = std::env::temp_dir().join("psionic-gguf-gen");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("qwen35.gguf");
        write_qwen35_fixture(&path).unwrap();
        let meta = parse_path(&path).unwrap();
        let mapped = map_file(&path, &meta).unwrap();
        let tok = load_tokenizer(&meta).unwrap();
        let hidden = embed_token(&mapped, 0, 8).unwrap();
        let a = greedy_from_hidden(&mapped, &hidden, &tok).unwrap();
        let b = greedy_from_hidden(&mapped, &hidden, &tok).unwrap();
        assert_eq!(a, b);
    }
}
