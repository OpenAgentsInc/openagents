use super::DomainError;

pub(crate) fn decode_lower_hex<const N: usize>(
    input: &str,
    field: &'static str,
) -> Result<[u8; N], DomainError> {
    if input.len() != N * 2 || !input.is_ascii() {
        return Err(DomainError::InvalidHex {
            field,
            expected_bytes: N,
        });
    }

    let mut output = [0_u8; N];
    for (index, pair) in input.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or(DomainError::InvalidHex {
            field,
            expected_bytes: N,
        })?;
        let low = hex_nibble(pair[1]).ok_or(DomainError::InvalidHex {
            field,
            expected_bytes: N,
        })?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(crate) fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
