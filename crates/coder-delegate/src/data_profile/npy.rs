//! NumPy `.npy` and `.npz` files, read in Rust with no dependency.
//!
//! An `.npy` file is a magic string, a version, a header that is a Python
//! dictionary literal (`descr`, `fortran_order`, `shape`), and the raw
//! array. This reads the numeric and Boolean dtypes: `f2`, `f4`, `f8`,
//! `i1` to `i8`, `u1` to `u8`, and `b1`, in either byte order. Anything
//! else, such as strings or Python objects, reads as its header only.
//!
//! An `.npz` file is a ZIP archive of `.npy` members. This reads the
//! members stored without compression, which `numpy.savez` writes. A member
//! `numpy.savez_compressed` deflated is named with its size and not read:
//! reading it would need a decompressor this crate doesn't depend on.

use regex::Regex;

/// The `.npy` magic string.
pub const MAGIC: &[u8] = b"\x93NUMPY";

/// An `.npy` header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The dtype as NumPy spells it, such as `<f8`.
    pub descr: String,
    pub fortran: bool,
    pub shape: Vec<usize>,
    /// Where the array's bytes start.
    pub offset: usize,
}

impl Header {
    /// The count of elements the shape holds.
    #[must_use]
    pub fn elements(&self) -> usize {
        self.shape.iter().product()
    }

    /// The shape as NumPy prints it, such as `(500, 64)`.
    #[must_use]
    pub fn shape_text(&self) -> String {
        match self.shape.as_slice() {
            [one] => format!("({one},)"),
            dims => format!(
                "({})",
                dims.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// A numeric dtype this module reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dtype {
    F16,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Bool,
}

impl Dtype {
    /// The bytes of one element.
    #[must_use]
    pub fn size(self) -> usize {
        match self {
            Dtype::I8 | Dtype::U8 | Dtype::Bool => 1,
            Dtype::F16 | Dtype::I16 | Dtype::U16 => 2,
            Dtype::F32 | Dtype::I32 | Dtype::U32 => 4,
            Dtype::F64 | Dtype::I64 | Dtype::U64 => 8,
        }
    }

    /// The name NumPy prints, such as `float64`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Dtype::F16 => "float16",
            Dtype::F32 => "float32",
            Dtype::F64 => "float64",
            Dtype::I8 => "int8",
            Dtype::I16 => "int16",
            Dtype::I32 => "int32",
            Dtype::I64 => "int64",
            Dtype::U8 => "uint8",
            Dtype::U16 => "uint16",
            Dtype::U32 => "uint32",
            Dtype::U64 => "uint64",
            Dtype::Bool => "bool",
        }
    }
}

/// The dtype and whether it is big-endian, for a `descr` this module
/// reads.
#[must_use]
pub fn dtype(descr: &str) -> Option<(Dtype, bool)> {
    let (order, code) = match descr.chars().next()? {
        c @ ('<' | '>' | '|' | '=') => (c, &descr[1..]),
        _ => ('=', descr),
    };
    let big = order == '>';
    let kind = match code {
        "f2" | "e" => Dtype::F16,
        "f4" | "f" => Dtype::F32,
        "f8" | "d" => Dtype::F64,
        "i1" | "b" => Dtype::I8,
        "i2" | "h" => Dtype::I16,
        "i4" | "i" => Dtype::I32,
        "i8" | "q" | "l" => Dtype::I64,
        "u1" | "B" => Dtype::U8,
        "u2" | "H" => Dtype::U16,
        "u4" | "I" => Dtype::U32,
        "u8" | "Q" | "L" => Dtype::U64,
        "b1" | "?" => Dtype::Bool,
        _ => return None,
    };
    Some((kind, big))
}

/// Reads an `.npy` header from the file's first bytes.
///
/// # Errors
///
/// Returns why the bytes aren't an `.npy` header.
pub fn header(bytes: &[u8]) -> Result<Header, String> {
    if !bytes.starts_with(MAGIC) || bytes.len() < 10 {
        return Err("not an .npy file: no NumPy magic string".to_string());
    }
    let major = bytes[6];
    let (len, start) = match major {
        1 => (usize::from(u16::from_le_bytes([bytes[8], bytes[9]])), 10),
        2 | 3 if bytes.len() >= 12 => (
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize,
            12,
        ),
        _ => return Err(format!("an .npy version this reader doesn't know: {major}")),
    };
    let end = start + len;
    let text = bytes
        .get(start..end)
        .ok_or("the .npy header is cut short")?;
    let text = String::from_utf8_lossy(text);
    let descr = Regex::new(r#"['"]descr['"]\s*:\s*['"]([^'"]+)['"]"#)
        .expect("a valid pattern")
        .captures(&text)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "a structured dtype".to_string());
    let fortran = Regex::new(r#"['"]fortran_order['"]\s*:\s*True"#)
        .expect("a valid pattern")
        .is_match(&text);
    let shape_text = Regex::new(r#"['"]shape['"]\s*:\s*\(([^)]*)\)"#)
        .expect("a valid pattern")
        .captures(&text)
        .map(|c| c[1].to_string())
        .ok_or("the .npy header has no shape")?;
    let shape = shape_text
        .split(',')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| d.trim_end_matches('L').parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("the .npy shape doesn't read: {e}"))?;
    Ok(Header {
        descr,
        fortran,
        shape,
        offset: end,
    })
}

fn half(bits: u16) -> f64 {
    let sign = if bits >> 15 == 1 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f64::from(bits & 0x3ff);
    match exponent {
        0 => sign * fraction * 2f64.powi(-24),
        31 if fraction == 0.0 => sign * f64::INFINITY,
        31 => f64::NAN,
        e => sign * (1.0 + fraction / 1024.0) * 2f64.powi(e - 15),
    }
}

/// Decodes one element.
fn element(bytes: &[u8], kind: Dtype, big: bool) -> f64 {
    macro_rules! read {
        ($t:ty) => {{
            let array = bytes.try_into().expect("an element's bytes");
            if big {
                <$t>::from_be_bytes(array)
            } else {
                <$t>::from_le_bytes(array)
            }
        }};
    }
    match kind {
        Dtype::F16 => half(read!(u16)),
        Dtype::F32 => f64::from(read!(f32)),
        Dtype::F64 => read!(f64),
        Dtype::I8 => f64::from(read!(i8)),
        Dtype::I16 => f64::from(read!(i16)),
        Dtype::I32 => f64::from(read!(i32)),
        Dtype::I64 => read!(i64) as f64,
        Dtype::U8 => f64::from(read!(u8)),
        Dtype::U16 => f64::from(read!(u16)),
        Dtype::U32 => f64::from(read!(u32)),
        Dtype::U64 => read!(u64) as f64,
        Dtype::Bool => f64::from(u8::from(bytes[0] != 0)),
    }
}

/// An array's values in row-major order, as many whole rows as `bytes`
/// holds, and the count of rows read. A Fortran-order array is read only
/// whole.
///
/// # Errors
///
/// Returns why the values can't be read.
pub fn values(bytes: &[u8], header: &Header) -> Result<(Vec<f64>, usize), String> {
    let (kind, big) =
        dtype(&header.descr).ok_or_else(|| format!("the dtype {} isn't numeric", header.descr))?;
    let data = bytes.get(header.offset..).unwrap_or_default();
    let size = kind.size();
    let rows = header.shape.first().copied().unwrap_or(1);
    let per_row: usize = if header.shape.is_empty() {
        1
    } else {
        header.shape[1..].iter().product()
    };
    let whole = header.elements();
    let available = data.len() / size;
    if header.fortran && available < whole {
        return Err("a Fortran-order array larger than the read bound".to_string());
    }
    let read_rows = available
        .checked_div(per_row)
        .map_or(rows, |fit| rows.min(fit));
    let count = read_rows * per_row;
    let mut out: Vec<f64> = data[..count * size]
        .chunks_exact(size)
        .map(|b| element(b, kind, big))
        .collect();
    if header.fortran && header.shape.len() >= 2 {
        // Column-major to row-major.
        let mut row_major = vec![0.0; out.len()];
        for (i, value) in out.iter().enumerate() {
            let r = i % rows;
            let c = i / rows;
            row_major[r * per_row + c] = *value;
        }
        out = row_major;
    }
    Ok((out, read_rows))
}

/// One member of an `.npz` archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// The member's bytes, when it is stored without compression.
    pub stored: Option<std::ops::Range<usize>>,
    pub size: u64,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    bytes
        .get(at..at + 2)
        .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u64> {
    bytes
        .get(at..at + 4)
        .map(|b| u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
}

/// The members of an `.npz` archive, read from its central directory.
///
/// # Errors
///
/// Returns why the bytes aren't a ZIP archive this reads.
pub fn members(bytes: &[u8]) -> Result<Vec<Member>, String> {
    let end = bytes
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or("not a ZIP archive: no end of central directory")?;
    let count = u16_at(bytes, end + 10).ok_or("a cut-short ZIP directory")?;
    let mut at = usize::try_from(u32_at(bytes, end + 16).ok_or("a cut-short ZIP directory")?)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for _ in 0..count {
        if bytes.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err("a ZIP directory entry doesn't read".to_string());
        }
        let method = u16_at(bytes, at + 10).unwrap_or(0);
        let compressed = u32_at(bytes, at + 20).unwrap_or(0);
        let size = u32_at(bytes, at + 24).unwrap_or(0);
        let name_len = u16_at(bytes, at + 28).unwrap_or(0);
        let extra_len = u16_at(bytes, at + 30).unwrap_or(0);
        let comment_len = u16_at(bytes, at + 32).unwrap_or(0);
        let local = usize::try_from(u32_at(bytes, at + 42).unwrap_or(0)).unwrap_or(usize::MAX);
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + name_len).unwrap_or(&[]))
            .into_owned();
        let stored = (method == 0 && compressed != u64::from(u32::MAX))
            .then(|| {
                let local_name = u16_at(bytes, local + 26)?;
                let local_extra = u16_at(bytes, local + 28)?;
                let start = local + 30 + local_name + local_extra;
                let end = start + usize::try_from(compressed).ok()?;
                (end <= bytes.len()).then_some(start..end)
            })
            .flatten();
        out.push(Member { name, stored, size });
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}
