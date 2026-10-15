//! A model's identity from its weights, not its file (#11652).
//!
//! Two GGUF files with the same tensors can hash differently: a converter
//! or a re-upload adds or reorders a metadata tag, and the header (and
//! with it every tensor's offset and the file's sha256) changes while no
//! weight does. The decision model on coderos-4080 is such a file: sha256
//! `fd3e9060…` where the pinned download is `36261166…`, one extra
//! metadata key, every tensor byte the same.
//!
//! [`tensor_digest`] reads a GGUF's tensor table and hashes, in name
//! order, each tensor's name, shape, type, and data bytes. Header metadata
//! (keys, values, their order, the alignment padding) is not hashed. The
//! result, `gguf-tensors:sha256:<hex>`, is the model id the self-check's
//! reference set and the last self-check key on; the file's sha256 stays
//! beside it for display and for the download check.

use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

/// The model id's prefix; the version is the canonical encoding below.
pub const PREFIX: &str = "gguf-tensors:sha256:";

/// The domain separator hashed first, naming the encoding.
const DOMAIN: &[u8] = b"openagents.gguf-tensors.v1\0";

/// One tensor's entry in the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tensor {
    pub name: String,
    pub dims: Vec<u64>,
    pub ggml_type: u32,
    /// Offset from the start of the data section.
    pub offset: u64,
    /// Its data length in bytes, from its shape and type.
    pub bytes: u64,
}

/// `(block size, bytes per block)` of a ggml tensor type.
fn block(ggml_type: u32) -> Option<(u64, u64)> {
    Some(match ggml_type {
        0 => (1, 4),     // F32
        1 => (1, 2),     // F16
        2 => (32, 18),   // Q4_0
        3 => (32, 20),   // Q4_1
        6 => (32, 22),   // Q5_0
        7 => (32, 24),   // Q5_1
        8 => (32, 34),   // Q8_0
        9 => (32, 36),   // Q8_1
        10 => (256, 84), // Q2_K
        11 => (256, 110),
        12 => (256, 144), // Q4_K
        13 => (256, 176), // Q5_K
        14 => (256, 210), // Q6_K
        15 => (256, 292), // Q8_K
        16 => (256, 66),  // IQ2_XXS
        17 => (256, 74),  // IQ2_XS
        18 => (256, 98),  // IQ3_XXS
        19 => (256, 50),  // IQ1_S
        20 => (32, 18),   // IQ4_NL
        21 => (256, 110), // IQ3_S
        22 => (256, 82),  // IQ2_S
        23 => (256, 136), // IQ4_XS
        24 => (1, 1),     // I8
        25 => (1, 2),     // I16
        26 => (1, 4),     // I32
        27 => (1, 8),     // I64
        28 => (1, 8),     // F64
        29 => (256, 56),  // IQ1_M
        30 => (1, 2),     // BF16
        34 => (256, 54),  // TQ1_0
        35 => (256, 66),  // TQ2_0
        39 => (32, 17),   // MXFP4
        _ => return None,
    })
}

struct Reader<R> {
    inner: R,
}

impl<R: Read> Reader<R> {
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; n];
        self.inner
            .read_exact(&mut buf)
            .map_err(|e| format!("the GGUF header ends early: {e}"))?;
        Ok(buf)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let b = self.bytes(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&b);
        Ok(u64::from_le_bytes(a))
    }

    fn string(&mut self) -> Result<String, String> {
        let n = self.u64()?;
        if n > 1 << 24 {
            return Err(format!("a GGUF string of {n} bytes; not a GGUF header"));
        }
        String::from_utf8(self.bytes(n as usize)?)
            .map_err(|_| "a GGUF string is not UTF-8".to_string())
    }

    /// Skip a metadata value of `kind`, returning it as a u64 when it is
    /// an unsigned integer (for `general.alignment`).
    fn value(&mut self, kind: u32) -> Result<Option<u64>, String> {
        let fixed = match kind {
            0 | 1 | 7 => 1,
            2 | 3 => 2,
            4..=6 => 4,
            10..=12 => 8,
            8 => {
                self.string()?;
                return Ok(None);
            }
            9 => {
                let inner = self.u32()?;
                let count = self.u64()?;
                if count > 1 << 32 {
                    return Err(format!("a GGUF array of {count} items"));
                }
                for _ in 0..count {
                    self.value(inner)?;
                }
                return Ok(None);
            }
            other => return Err(format!("unknown GGUF metadata type {other}")),
        };
        let b = self.bytes(fixed)?;
        Ok(match kind {
            0 => Some(u64::from(b[0])),
            2 => Some(u64::from(u16::from_le_bytes([b[0], b[1]]))),
            4 => Some(u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
            10 => {
                let mut a = [0u8; 8];
                a.copy_from_slice(&b);
                Some(u64::from_le_bytes(a))
            }
            _ => None,
        })
    }
}

/// A GGUF's tensor table and where its data section starts.
///
/// # Errors
///
/// When `source` is not a GGUF (v2 or v3) or names a tensor type this
/// crate does not know the size of.
pub fn tensors<R: Read + Seek>(source: &mut R) -> Result<(Vec<Tensor>, u64), String> {
    source.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut r = Reader {
        inner: BufReader::new(&mut *source),
    };
    if r.bytes(4)? != b"GGUF" {
        return Err("not a GGUF file".into());
    }
    let version = r.u32()?;
    if !(2..=3).contains(&version) {
        return Err(format!("GGUF version {version} is not supported"));
    }
    let count = r.u64()?;
    let kv = r.u64()?;
    if count > 1 << 20 || kv > 1 << 20 {
        return Err("the GGUF header's counts are not plausible".into());
    }
    let mut alignment = 32u64;
    for _ in 0..kv {
        let key = r.string()?;
        let kind = r.u32()?;
        let value = r.value(kind)?;
        if key == "general.alignment"
            && let Some(a) = value
        {
            alignment = a;
        }
    }
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(format!("GGUF alignment {alignment} is not a power of two"));
    }
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name = r.string()?;
        let n_dims = r.u32()?;
        if n_dims > 8 {
            return Err(format!("tensor {name} has {n_dims} dimensions"));
        }
        let dims = (0..n_dims)
            .map(|_| r.u64())
            .collect::<Result<Vec<_>, _>>()?;
        let ggml_type = r.u32()?;
        let offset = r.u64()?;
        let (block_size, block_bytes) = block(ggml_type)
            .ok_or_else(|| format!("tensor {name} has ggml type {ggml_type}, unknown here"))?;
        let elements = dims
            .iter()
            .try_fold(1u64, |n, d| n.checked_mul(*d))
            .ok_or_else(|| format!("tensor {name}'s shape overflows"))?;
        if elements % block_size != 0 {
            return Err(format!("tensor {name}'s shape does not fill its blocks"));
        }
        out.push(Tensor {
            name,
            dims,
            ggml_type,
            offset,
            bytes: elements / block_size * block_bytes,
        });
    }
    let end = r.inner.stream_position().map_err(|e| e.to_string())?;
    let data = end.div_ceil(alignment) * alignment;
    Ok((out, data))
}

/// The model id of the GGUF read from `source`: `gguf-tensors:sha256:…`
/// over every tensor's name, shape, type, and bytes, in name order.
///
/// # Errors
///
/// When it is not a GGUF this module reads, two tensors share a name, or
/// a tensor's data runs past the end.
pub fn digest<R: Read + Seek>(source: &mut R) -> Result<String, String> {
    let (mut list, data) = tensors(source)?;
    let len = source.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    list.sort_by(|a, b| a.name.cmp(&b.name));
    if list.windows(2).any(|w| w[0].name == w[1].name) {
        return Err("two tensors share a name".into());
    }
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update((list.len() as u64).to_le_bytes());
    let mut buffer = vec![0u8; 1 << 20];
    for t in &list {
        let start = data
            .checked_add(t.offset)
            .filter(|s| s.checked_add(t.bytes).is_some_and(|e| e <= len))
            .ok_or_else(|| format!("tensor {}'s data runs past the end of the file", t.name))?;
        hasher.update((t.name.len() as u64).to_le_bytes());
        hasher.update(t.name.as_bytes());
        hasher.update((t.dims.len() as u32).to_le_bytes());
        for d in &t.dims {
            hasher.update(d.to_le_bytes());
        }
        hasher.update(t.ggml_type.to_le_bytes());
        hasher.update(t.bytes.to_le_bytes());
        source
            .seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        let mut left = t.bytes;
        while left > 0 {
            let n = left.min(buffer.len() as u64) as usize;
            source
                .read_exact(&mut buffer[..n])
                .map_err(|e| format!("cannot read tensor {}: {e}", t.name))?;
            hasher.update(&buffer[..n]);
            left -= n as u64;
        }
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        });
    Ok(format!("{PREFIX}{hex}"))
}

/// The model id of the GGUF at `path`.
///
/// # Errors
///
/// As [`digest`], or when the file cannot be opened.
pub fn tensor_digest(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    digest(&mut file).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Cursor;

    use super::*;

    /// A GGUF v3 file with `meta` (string key/values) and `tensors`
    /// (name, dims, F32 values), written in the given order.
    pub(crate) fn gguf(meta: &[(&str, &str)], tensors: &[(&str, Vec<u64>, Vec<f32>)]) -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend(3u32.to_le_bytes());
        out.extend((tensors.len() as u64).to_le_bytes());
        out.extend((meta.len() as u64).to_le_bytes());
        let string = |out: &mut Vec<u8>, s: &str| {
            out.extend((s.len() as u64).to_le_bytes());
            out.extend(s.as_bytes());
        };
        for (k, v) in meta {
            string(&mut out, k);
            out.extend(8u32.to_le_bytes());
            string(&mut out, v);
        }
        let mut offset = 0u64;
        let mut offsets = Vec::new();
        for (name, dims, values) in tensors {
            string(&mut out, name);
            out.extend((dims.len() as u32).to_le_bytes());
            for d in dims {
                out.extend(d.to_le_bytes());
            }
            out.extend(0u32.to_le_bytes());
            out.extend(offset.to_le_bytes());
            offsets.push(offset);
            offset += (values.len() as u64 * 4).div_ceil(32) * 32;
        }
        while out.len() % 32 != 0 {
            out.push(0);
        }
        let data = out.len();
        for ((_, _, values), at) in tensors.iter().zip(offsets) {
            out.resize(data + at as usize, 0);
            for v in values {
                out.extend(v.to_le_bytes());
            }
        }
        out
    }

    fn id(bytes: &[u8]) -> String {
        digest(&mut Cursor::new(bytes)).unwrap()
    }

    fn weights() -> Vec<(&'static str, Vec<u64>, Vec<f32>)> {
        vec![
            (
                "blk.0.attn_q.weight",
                vec![4, 2],
                (0..8).map(|i| i as f32).collect(),
            ),
            ("token_embd.weight", vec![3], vec![0.5, -1.0, 2.0]),
        ]
    }

    #[test]
    fn a_metadata_key_changes_the_file_hash_but_not_the_model_id() {
        let pinned = gguf(&[("general.name", "Clef-Flash")], &weights());
        let tagged = gguf(
            &[
                ("general.name", "Clef-Flash"),
                ("general.quantized_by", "someone else"),
            ],
            &weights(),
        );
        assert_ne!(Sha256::digest(&pinned), Sha256::digest(&tagged));
        assert_eq!(id(&pinned), id(&tagged));
        assert!(id(&pinned).starts_with(PREFIX));
        // The tensor table's order is not part of the identity either.
        let mut reordered = weights();
        reordered.reverse();
        assert_eq!(id(&gguf(&[], &reordered)), id(&pinned));
    }

    #[test]
    fn any_weight_name_or_shape_change_changes_the_model_id() {
        let base = id(&gguf(&[], &weights()));
        let mut w = weights();
        w[1].2[2] = 2.5;
        assert_ne!(id(&gguf(&[], &w)), base);
        let mut w = weights();
        w[0].1 = vec![2, 4];
        assert_ne!(id(&gguf(&[], &w)), base);
        let mut w = weights();
        w[1].0 = "output.weight";
        assert_ne!(id(&gguf(&[], &w)), base);
    }

    #[test]
    fn a_file_that_is_not_a_whole_gguf_has_no_model_id() {
        assert!(digest(&mut Cursor::new(b"weights".to_vec())).is_err());
        let mut cut = gguf(&[], &weights());
        cut.truncate(cut.len() - 4);
        assert!(digest(&mut Cursor::new(cut)).is_err());
    }

    /// A real Clef-Flash Q4_K_M file (`OPENAGENTS_CLEF_GGUF=PATH cargo
    /// test -p pylon -- --ignored real_clef`) has the pinned model id,
    /// whatever its header says.
    #[test]
    #[ignore = "reads a 6.5 GB model file"]
    fn a_real_clef_flash_file_has_the_pinned_model_id() {
        let path = std::env::var("OPENAGENTS_CLEF_GGUF").expect("set OPENAGENTS_CLEF_GGUF");
        assert_eq!(
            tensor_digest(Path::new(&path)).unwrap(),
            crate::setup::CLEF_FLASH_MODEL_ID
        );
    }
}
