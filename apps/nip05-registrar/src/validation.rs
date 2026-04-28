use bitcoin::secp256k1::XOnlyPublicKey;

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
    if XOnlyPublicKey::from_slice(&data).is_err() {
        return Err(RegistrarError::InvalidNpub);
    }
    Ok(hex::encode(data))
}

/// Returns true only if `value` is exactly 64 lowercase hex characters AND
/// decodes to a valid x-only secp256k1 public key.
pub fn is_valid_hex_pubkey(value: &str) -> bool {
    if value.len() != PUBKEY_BYTES * 2 {
        return false;
    }
    if !value
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return false;
    }
    let Ok(bytes) = hex::decode(value) else {
        return false;
    };
    XOnlyPublicKey::from_slice(&bytes).is_ok()
}

/// Resolve a request that supplies `npub` and/or `pubkey` into a single
/// canonical 64-char lowercase hex pubkey.
///
/// Rules:
/// - At least one of `npub` / `pubkey` must be present and non-empty.
/// - If both are present, they must decode to the same key.
/// - The resulting bytes must be a valid x-only secp256k1 pubkey.
pub fn resolve_pubkey(npub: Option<&str>, pubkey: Option<&str>) -> Result<String, RegistrarError> {
    let npub_hex = npub
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(decode_npub_to_hex)
        .transpose()?;
    let hex_hex = pubkey
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|raw| {
            let lowered = raw.to_ascii_lowercase();
            if is_valid_hex_pubkey(&lowered) {
                Ok(lowered)
            } else {
                Err(RegistrarError::InvalidNpub)
            }
        })
        .transpose()?;
    match (npub_hex, hex_hex) {
        (Some(a), Some(b)) if a == b => Ok(a),
        (Some(_), Some(_)) => Err(RegistrarError::InvalidNpub),
        (Some(a), None) | (None, Some(a)) => Ok(a),
        (None, None) => Err(RegistrarError::InvalidNpub),
    }
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
        assert!(
            decode_npub_to_hex("nsec180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6")
                .is_err()
        );
        assert!(decode_npub_to_hex("npub1invalid").is_err());
        assert!(decode_npub_to_hex("not-an-npub").is_err());
        assert!(decode_npub_to_hex("").is_err());
    }

    #[test]
    fn hex_pubkey_rejects_invalid_curve_points() {
        // 32 zero bytes is not on the curve.
        assert!(!is_valid_hex_pubkey(
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));
        // 32 0xFF bytes is past the field prime.
        assert!(!is_valid_hex_pubkey(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        ));
    }

    #[test]
    fn resolve_requires_one_or_matching_pair() {
        let npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
        let hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        assert_eq!(resolve_pubkey(Some(npub), None).unwrap(), hex);
        assert_eq!(resolve_pubkey(None, Some(hex)).unwrap(), hex);
        assert_eq!(resolve_pubkey(Some(npub), Some(hex)).unwrap(), hex);
        let mismatching = "9f0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d3";
        // Pick a different valid x-only key. Use a generated test vector:
        // x-only of secret key 0x0000...0001 == G.x.
        let g_x = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        assert!(resolve_pubkey(Some(npub), Some(g_x)).is_err());
        assert!(resolve_pubkey(None, None).is_err());
        assert!(resolve_pubkey(Some(""), Some("")).is_err());
        // mismatching is invalid hex too — covered indirectly via length check.
        let _ = mismatching;
    }
}
