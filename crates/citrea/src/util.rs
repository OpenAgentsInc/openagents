use crate::CitreaError;

pub fn strip_0x(input: &str) -> &str {
    if let Some(stripped) = input.strip_prefix("0x") {
        stripped
    } else if let Some(stripped) = input.strip_prefix("0X") {
        stripped
    } else {
        input
    }
}

pub fn parse_hex_bytes<const N: usize>(input: &str) -> Result<[u8; N], CitreaError> {
    let raw = strip_0x(input);
    let bytes = hex::decode(raw)?;
    if bytes.len() != N {
        return Err(CitreaError::InvalidLength {
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn parse_hex_vec(input: &str) -> Result<Vec<u8>, CitreaError> {
    let raw = strip_0x(input);
    let bytes = hex::decode(raw)?;
    Ok(bytes)
}

pub fn format_hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

pub fn parse_u64_hex_or_dec(input: &str) -> Result<u64, CitreaError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CitreaError::InvalidChainId("empty value".to_string()));
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|e| CitreaError::InvalidChainId(e.to_string()))
    } else {
        trimmed
            .parse::<u64>()
            .map_err(|e| CitreaError::InvalidChainId(e.to_string()))
    }
}

pub fn parse_hex_u64(input: &str) -> Result<u64, CitreaError> {
    let raw = strip_0x(input);
    if raw.is_empty() {
        return Err(CitreaError::InvalidChainId("empty hex value".to_string()));
    }
    u64::from_str_radix(raw, 16).map_err(|e| CitreaError::InvalidChainId(e.to_string()))
}

pub fn parse_hex_u128(input: &str) -> Result<u128, CitreaError> {
    let raw = strip_0x(input);
    if raw.is_empty() {
        return Err(CitreaError::InvalidHex("empty hex value".to_string()));
    }
    u128::from_str_radix(raw, 16).map_err(|e| CitreaError::InvalidHex(e.to_string()))
}
