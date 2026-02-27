use super::error::{Nip77Error, Result};

/// Encode a varint (variable-length unsigned integer)
///
/// Varints are represented as base-128 digits, most significant digit first.
/// Bit 7 (high bit) is set on each byte except the last.
pub fn encode_varint(mut value: u64) -> Result<Vec<u8>> {
    if value == 0 {
        return Ok(vec![0]);
    }

    let mut bytes = Vec::new();
    let mut temp = Vec::new();

    // Extract base-128 digits
    while value > 0 {
        temp.push((value & 0x7F) as u8);
        value >>= 7;
    }

    // Reverse and set high bit on all but last
    for (i, &byte) in temp.iter().rev().enumerate() {
        if i < temp.len() - 1 {
            bytes.push(byte | 0x80);
        } else {
            bytes.push(byte);
        }
    }

    Ok(bytes)
}

/// Decode a varint from a byte slice
///
/// Returns (value, bytes_consumed)
pub fn decode_varint(data: &[u8]) -> Result<(u64, usize)> {
    if data.is_empty() {
        return Err(Nip77Error::VarintDecode("empty data".to_string()));
    }

    let mut value: u64 = 0;
    let mut bytes_read = 0;

    for &byte in data.iter() {
        bytes_read += 1;

        // Check for overflow before shifting
        if value > (u64::MAX >> 7) {
            return Err(Nip77Error::VarintDecode("varint overflow".to_string()));
        }

        value = (value << 7) | ((byte & 0x7F) as u64);

        // If high bit is not set, this is the last byte
        if (byte & 0x80) == 0 {
            return Ok((value, bytes_read));
        }

        if bytes_read > 10 {
            return Err(Nip77Error::VarintDecode(
                "varint too long (max 10 bytes for u64)".to_string(),
            ));
        }
    }

    Err(Nip77Error::VarintDecode("incomplete varint".to_string()))
}
