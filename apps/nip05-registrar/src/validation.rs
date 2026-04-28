use crate::error::RegistrarError;

const HANDLE_MAX_LEN: usize = 32;
const NPUB_HRP: &str = "npub";
const PUBKEY_BYTES: usize = 32;

pub fn validate_handle(raw: &str) -> Result<String, RegistrarError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > HANDLE_MAX_LEN {
        return Err(RegistrarError::InvalidHandle);
    }
    let valid = trimmed
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'));
    if !valid {
        return Err(RegistrarError::InvalidHandle);
    }
    Ok(trimmed.to_string())
}

pub fn decode_npub_to_hex(raw: &str) -> Result<String, RegistrarError> {
    let trimmed = raw.trim();
    if !trimmed.to_ascii_lowercase().starts_with("npub1") {
        return Err(RegistrarError::InvalidNpub);
    }
    let (hrp, data) = bech32::decode(trimmed).map_err(|_| RegistrarError::InvalidNpub)?;
    if hrp.as_str() != NPUB_HRP || data.len() != PUBKEY_BYTES {
        return Err(RegistrarError::InvalidNpub);
    }
    Ok(hex::encode(data))
}

pub fn is_valid_hex_pubkey(value: &str) -> bool {
    value.len() == PUBKEY_BYTES * 2
        && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn handle_accepts_lowercase_alnum_and_symbols() {
        assert_eq!(validate_handle("chris").unwrap(), "chris");
        assert_eq!(validate_handle("a.b-c_1").unwrap(), "a.b-c_1");
        assert_eq!(validate_handle("agent01").unwrap(), "agent01");
    }

    #[test]
    fn handle_rejects_empty_and_too_long() {
        assert!(validate_handle("").is_err());
        assert!(validate_handle("   ").is_err());
        let long = "a".repeat(33);
        assert!(validate_handle(&long).is_err());
    }

    #[test]
    fn handle_rejects_uppercase_and_invalid_chars() {
        assert!(validate_handle("Chris").is_err());
        assert!(validate_handle("chris@home").is_err());
        assert!(validate_handle("chris space").is_err());
        assert!(validate_handle("chris/slash").is_err());
    }

    #[test]
    fn handle_trims_whitespace() {
        assert_eq!(validate_handle("  bob  ").unwrap(), "bob");
    }

    #[test]
    fn npub_decodes_to_64_hex() {
        let npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
        let hex = decode_npub_to_hex(npub).expect("decode");
        assert_eq!(hex.len(), 64);
        assert!(is_valid_hex_pubkey(&hex));
    }

    #[test]
    fn npub_rejects_wrong_hrp_or_garbage() {
        assert!(decode_npub_to_hex("nsec180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6").is_err());
        assert!(decode_npub_to_hex("npub1invalid").is_err());
        assert!(decode_npub_to_hex("not-an-npub").is_err());
        assert!(decode_npub_to_hex("").is_err());
    }
}
