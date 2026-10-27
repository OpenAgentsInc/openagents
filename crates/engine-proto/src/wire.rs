//! Bounded admission check before Prost decoding. Unknown intent fields refuse.
type Field = (u32, i32, bool, usize, usize, &'static str);
include!("../generated/rust/wire.rs");
fn varint(bytes: &mut &[u8]) -> Result<u64, &'static str> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = bytes.split_first().ok_or("truncated protobuf")?;
        *bytes = rest;
        if shift == 63 && byte > 1 {
            return Err("protobuf integer overflow");
        }
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(value);
        }
    }
    Err("protobuf integer overflow")
}
fn take<'a>(input: &mut &'a [u8], size: usize) -> Result<&'a [u8], &'static str> {
    if size > input.len() {
        return Err("truncated protobuf");
    }
    let (head, tail) = input.split_at(size);
    *input = tail;
    Ok(head)
}
pub fn validate(name: &str, bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() > 1_048_576 {
        return Err("protobuf byte limit");
    }
    inspect(name, bytes, 0, &mut 0)
}
fn inspect(
    name: &str,
    mut bytes: &[u8],
    depth: u32,
    nodes: &mut usize,
) -> Result<(), &'static str> {
    if depth > 64 {
        return Err("protobuf depth limit");
    }
    let fields = MESSAGES
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or("unknown contract")?
        .1;
    let mut counts = std::collections::BTreeMap::new();
    while !bytes.is_empty() {
        *nodes += 1;
        if *nodes > 250_000 {
            return Err("protobuf value limit");
        }
        let tag = varint(&mut bytes)?;
        let field = fields
            .iter()
            .find(|f| u64::from(f.0) == tag >> 3)
            .ok_or("unknown semantic field")?;
        let kind = tag & 7;
        let expected = match field.1 {
            1 | 6 | 16 => 1,
            2 | 7 | 15 => 5,
            9 | 11 | 12 => 2,
            _ => 0,
        };
        if kind != expected && !(field.2 && expected != 2 && kind == 2) {
            return Err("protobuf wire type");
        }
        let count = counts.entry(field.0).or_insert(0usize);
        if kind == 2 {
            let size = usize::try_from(varint(&mut bytes)?).map_err(|_| "protobuf length")?;
            let body = take(&mut bytes, size)?;
            if field.1 == 11 {
                inspect(field.5, body, depth + 1, nodes)?;
                *count += 1;
            } else if field.1 == 9 || field.1 == 12 {
                if body.len() > field.3 {
                    return Err("protobuf string limit");
                }
                if field.1 == 9 {
                    let text = std::str::from_utf8(body).map_err(|_| "invalid utf8")?;
                    if text.contains('\0') {
                        return Err("embedded nul");
                    }
                }
                *count += 1;
            } else {
                let mut packed = body;
                while !packed.is_empty() {
                    scalar(&mut packed, expected, field.1)?;
                    *nodes += 1;
                    if *nodes > 250_000 {
                        return Err("protobuf value limit");
                    }
                    *count += 1;
                    if *count > field.4 {
                        return Err("protobuf count limit");
                    }
                }
            }
        } else {
            scalar(&mut bytes, kind, field.1)?;
            *count += 1;
        }
        if field.2 && *count > field.4 {
            return Err("protobuf count limit");
        }
    }
    Ok(())
}
fn scalar(input: &mut &[u8], wire: u64, ty: i32) -> Result<(), &'static str> {
    const SAFE: u64 = 9_007_199_254_740_991;
    match wire {
        0 => {
            let n = varint(input)?;
            let valid = match ty {
                3 => (n as i64).unsigned_abs() <= SAFE,
                4 => n <= SAFE,
                5 | 14 => (n as i64) >= i32::MIN as i64 && (n as i64) <= i32::MAX as i64,
                13 | 17 => n <= u32::MAX as u64,
                18 => ((n >> 1) as i64 ^ -((n & 1) as i64)).unsigned_abs() <= SAFE,
                _ => true,
            };
            if !valid {
                return Err("protobuf integer range");
            }
        }
        1 => {
            let b: [u8; 8] = take(input, 8)?.try_into().unwrap();
            let n = u64::from_le_bytes(b);
            if (ty == 1 && !f64::from_le_bytes(b).is_finite())
                || (ty == 6 && n > SAFE)
                || (ty == 16 && (n as i64).unsigned_abs() > SAFE)
            {
                return Err("protobuf number range");
            }
        }
        5 => {
            let b: [u8; 4] = take(input, 4)?.try_into().unwrap();
            if ty == 2 && !f32::from_le_bytes(b).is_finite() {
                return Err("protobuf finite number");
            }
        }
        _ => return Err("protobuf scalar"),
    };
    Ok(())
}
