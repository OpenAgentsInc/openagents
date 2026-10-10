//! The small part of CBOR (RFC 8949) Cashu's V4 tokens (NUT-00) and
//! payment requests (NUT-18) use: unsigned and negative integers, byte
//! and text strings, arrays, maps, booleans, and null, all with definite
//! lengths. Floats, tags, and indefinite lengths are refused.

/// One CBOR data item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cbor {
    Uint(u64),
    /// `-1 - n`.
    Neg(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    Map(Vec<(Cbor, Cbor)>),
    Bool(bool),
    Null,
}

/// The deepest nesting read.
const MAX_DEPTH: usize = 16;

impl Cbor {
    /// The value under text key `key` of a map.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Cbor> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .find(|(name, _)| matches!(name, Self::Text(text) if text == key))
                .map(|(_, value)| value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Uint(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Cbor]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The canonical-length encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Uint(value) => head(out, 0, *value),
            Self::Neg(value) => head(out, 1, *value),
            Self::Bytes(bytes) => {
                head(out, 2, bytes.len() as u64);
                out.extend_from_slice(bytes);
            }
            Self::Text(text) => {
                head(out, 3, text.len() as u64);
                out.extend_from_slice(text.as_bytes());
            }
            Self::Array(items) => {
                head(out, 4, items.len() as u64);
                for item in items {
                    item.write(out);
                }
            }
            Self::Map(entries) => {
                head(out, 5, entries.len() as u64);
                for (key, value) in entries {
                    key.write(out);
                    value.write(out);
                }
            }
            Self::Bool(false) => out.push(0xf4),
            Self::Bool(true) => out.push(0xf5),
            Self::Null => out.push(0xf6),
        }
    }

    /// Read exactly one item filling `bytes`.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut at = 0;
        let value = read(bytes, &mut at, 0)?;
        (at == bytes.len()).then_some(value)
    }
}

fn head(out: &mut Vec<u8>, major: u8, value: u64) {
    let major = major << 5;
    if value < 24 {
        out.push(major | value as u8);
    } else if value <= 0xff {
        out.push(major | 24);
        out.push(value as u8);
    } else if value <= 0xffff {
        out.push(major | 25);
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= 0xffff_ffff {
        out.push(major | 26);
        out.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        out.push(major | 27);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn take<'a>(bytes: &'a [u8], at: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = at.checked_add(len)?;
    let slice = bytes.get(*at..end)?;
    *at = end;
    Some(slice)
}

/// The argument of an item whose initial byte had `info` as its low bits.
fn argument(bytes: &[u8], at: &mut usize, info: u8) -> Option<u64> {
    Some(match info {
        0..=23 => u64::from(info),
        24 => u64::from(take(bytes, at, 1)?[0]),
        25 => u64::from(u16::from_be_bytes(take(bytes, at, 2)?.try_into().ok()?)),
        26 => u64::from(u32::from_be_bytes(take(bytes, at, 4)?.try_into().ok()?)),
        27 => u64::from_be_bytes(take(bytes, at, 8)?.try_into().ok()?),
        _ => return None,
    })
}

fn read(bytes: &[u8], at: &mut usize, depth: usize) -> Option<Cbor> {
    if depth > MAX_DEPTH {
        return None;
    }
    let initial = *take(bytes, at, 1)?.first()?;
    let major = initial >> 5;
    let info = initial & 0x1f;
    if major == 7 {
        return match info {
            20 => Some(Cbor::Bool(false)),
            21 => Some(Cbor::Bool(true)),
            22 => Some(Cbor::Null),
            _ => None,
        };
    }
    let value = argument(bytes, at, info)?;
    // Every item takes at least one byte, so a count can't exceed what's
    // left; this keeps a hostile length from allocating.
    let left = (bytes.len() - *at) as u64;
    match major {
        0 => Some(Cbor::Uint(value)),
        1 => Some(Cbor::Neg(value)),
        2 => Some(Cbor::Bytes(
            take(bytes, at, usize::try_from(value).ok()?)?.to_vec(),
        )),
        3 => String::from_utf8(take(bytes, at, usize::try_from(value).ok()?)?.to_vec())
            .ok()
            .map(Cbor::Text),
        4 => {
            if value > left {
                return None;
            }
            let mut items = Vec::with_capacity(value as usize);
            for _ in 0..value {
                items.push(read(bytes, at, depth + 1)?);
            }
            Some(Cbor::Array(items))
        }
        5 => {
            if value.checked_mul(2)? > left {
                return None;
            }
            let mut entries = Vec::with_capacity(value as usize);
            for _ in 0..value {
                let key = read(bytes, at, depth + 1)?;
                let item = read(bytes, at, depth + 1)?;
                entries.push((key, item));
            }
            Some(Cbor::Map(entries))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_8949_examples_read_and_write() {
        for (hex, value) in [
            ("00", Cbor::Uint(0)),
            ("17", Cbor::Uint(23)),
            ("1818", Cbor::Uint(24)),
            ("1903e8", Cbor::Uint(1000)),
            ("1a000f4240", Cbor::Uint(1_000_000)),
            ("1b000000e8d4a51000", Cbor::Uint(1_000_000_000_000)),
            ("20", Cbor::Neg(0)),
            ("6161", Cbor::Text("a".into())),
            ("4401020304", Cbor::Bytes(vec![1, 2, 3, 4])),
            ("f4", Cbor::Bool(false)),
            ("f5", Cbor::Bool(true)),
            ("f6", Cbor::Null),
            (
                "83010203",
                Cbor::Array(vec![Cbor::Uint(1), Cbor::Uint(2), Cbor::Uint(3)]),
            ),
            (
                "a26161016162820203",
                Cbor::Map(vec![
                    (Cbor::Text("a".into()), Cbor::Uint(1)),
                    (
                        Cbor::Text("b".into()),
                        Cbor::Array(vec![Cbor::Uint(2), Cbor::Uint(3)]),
                    ),
                ]),
            ),
        ] {
            let bytes = hex::decode(hex).unwrap();
            assert_eq!(Cbor::decode(&bytes), Some(value.clone()), "{hex}");
            assert_eq!(value.encode(), bytes, "{hex}");
        }
    }

    #[test]
    fn hostile_input_is_refused() {
        // A float, a tag, an indefinite array, a huge count, trailing bytes.
        for hex in ["f93c00", "c11a514b67b0", "9f01ff", "9b00000000ffffffff", "0000"] {
            assert_eq!(Cbor::decode(&hex::decode(hex).unwrap()), None, "{hex}");
        }
        // Too deep.
        let mut deep = vec![0x81; MAX_DEPTH + 2];
        deep.push(0x00);
        assert_eq!(Cbor::decode(&deep), None);
    }
}
