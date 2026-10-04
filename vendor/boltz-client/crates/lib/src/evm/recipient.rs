//! Cross-transport recipient encoding for OFT cross-chain sends.
//!
//! `LayerZero` OFT `SendParam.to` is a `bytes32`. EVM addresses are
//! left-padded; Solana pubkeys are 32 bytes already; Tron addresses are
//! base58check-encoded 21-byte payloads (1-byte `0x41` prefix + 20-byte
//! address) plus a 4-byte double-SHA256 checksum, of which only the trailing
//! 20 bytes are kept and left-padded.

use alloy_primitives::FixedBytes;
use sha2::{Digest, Sha256};

use crate::error::BoltzError;
use crate::evm::contracts::{address_to_bytes32, parse_address};
use crate::models::NetworkTransport;

const TRON_BASE58_LEN: usize = 25;
const TRON_PAYLOAD_LEN: usize = 21;
const TRON_PREFIX: u8 = 0x41;
const SOLANA_PUBKEY_LEN: usize = 32;

/// Encode a destination address into the 32-byte form expected by
/// `OftSendParam.to` and `SendData.to`.
pub fn encode_oft_recipient(
    transport: NetworkTransport,
    addr: &str,
) -> Result<FixedBytes<32>, BoltzError> {
    match transport {
        NetworkTransport::Evm => encode_evm(addr),
        NetworkTransport::Solana => encode_solana(addr),
        NetworkTransport::Tron => encode_tron(addr),
    }
}

/// Whether `addr` is a valid destination address for the given transport.
/// Cheap to call from input-validation paths in callers.
#[must_use]
pub fn is_valid_destination_address(transport: NetworkTransport, addr: &str) -> bool {
    encode_oft_recipient(transport, addr).is_ok()
}

/// Normalize a token/recipient address for known-token comparison. EVM
/// addresses are lowercased (they are case-insensitive / EIP-55 checksummed,
/// so a paste may carry any casing); Solana and Tron addresses are left as-is
/// because base58 is case-sensitive. Surrounding whitespace is trimmed.
#[must_use]
pub fn normalize_token_address(transport: NetworkTransport, addr: &str) -> String {
    let trimmed = addr.trim();
    match transport {
        NetworkTransport::Evm => trimmed.to_lowercase(),
        NetworkTransport::Solana | NetworkTransport::Tron => trimmed.to_string(),
    }
}

fn encode_evm(addr: &str) -> Result<FixedBytes<32>, BoltzError> {
    let parsed = parse_address(addr)?;
    Ok(address_to_bytes32(parsed))
}

fn encode_solana(addr: &str) -> Result<FixedBytes<32>, BoltzError> {
    let decoded = bs58::decode(addr).into_vec().map_err(|e| BoltzError::Evm {
        reason: format!("Invalid Solana address '{addr}': {e}"),
        tx_hash: None,
    })?;
    if decoded.len() != SOLANA_PUBKEY_LEN {
        return Err(BoltzError::Evm {
            reason: format!(
                "Solana address must decode to {SOLANA_PUBKEY_LEN} bytes, got {}",
                decoded.len()
            ),
            tx_hash: None,
        });
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&decoded);
    Ok(FixedBytes::from(out))
}

fn encode_tron(addr: &str) -> Result<FixedBytes<32>, BoltzError> {
    let decoded = bs58::decode(addr).into_vec().map_err(|e| BoltzError::Evm {
        reason: format!("Invalid Tron address '{addr}': {e}"),
        tx_hash: None,
    })?;
    if decoded.len() != TRON_BASE58_LEN {
        return Err(BoltzError::Evm {
            reason: format!(
                "Tron address must decode to {TRON_BASE58_LEN} bytes, got {}",
                decoded.len()
            ),
            tx_hash: None,
        });
    }
    let (payload, checksum) = decoded.split_at(TRON_PAYLOAD_LEN);

    let first = Sha256::digest(payload);
    let second = Sha256::digest(first);
    if checksum != &second[..4] {
        return Err(BoltzError::Evm {
            reason: format!("Tron address '{addr}' has invalid checksum"),
            tx_hash: None,
        });
    }

    if payload[0] != TRON_PREFIX {
        return Err(BoltzError::Evm {
            reason: format!(
                "Tron address '{addr}' has unexpected prefix 0x{:02x}, expected 0x41",
                payload[0]
            ),
            tx_hash: None,
        });
    }

    let mut out = [0u8; 32];
    out[12..32].copy_from_slice(&payload[1..]);
    Ok(FixedBytes::from(out))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn evm_recipient_left_pads() {
        let encoded = encode_oft_recipient(
            NetworkTransport::Evm,
            "0x1234567890AbCdEf1234567890aBcDeF12345678",
        )
        .unwrap();
        assert_eq!(&encoded[..12], &[0u8; 12]);
        assert_eq!(
            &encoded[12..],
            &hex::decode("1234567890abcdef1234567890abcdef12345678").unwrap()[..]
        );
    }

    #[macros::test_all]
    fn evm_recipient_rejects_garbage() {
        assert!(encode_oft_recipient(NetworkTransport::Evm, "not-an-address").is_err());
        assert!(encode_oft_recipient(NetworkTransport::Evm, "0x1234").is_err());
    }

    #[macros::test_all]
    fn solana_recipient_decodes_all_zeros() {
        // Base58 '1' is the zero digit; 32 chars of '1' -> 32 zero bytes.
        let encoded =
            encode_oft_recipient(NetworkTransport::Solana, "11111111111111111111111111111111")
                .unwrap();
        assert_eq!(encoded.as_slice(), &[0u8; 32]);
    }

    #[macros::test_all]
    fn solana_recipient_roundtrips_non_trivial_pubkey() {
        // An arbitrary valid Solana pubkey that exercises the full base58
        // alphabet (not just '1'-as-zero). Encoding the decoded bytes back
        // and asserting a bit-exact match catches any bs58 encode/decode
        // asymmetry that zero-only vectors would miss.
        let addr = "BZkwksSEeHrCVS3HeewBJKEBTEEuwnEqpkHqEg1dRpuE";
        let encoded = encode_oft_recipient(NetworkTransport::Solana, addr).unwrap();
        assert_eq!(encoded.len(), 32);
        assert_ne!(encoded.as_slice(), &[0u8; 32]);
        let reencoded = bs58::encode(encoded.as_slice()).into_string();
        assert_eq!(reencoded, addr);
    }

    #[macros::test_all]
    fn solana_recipient_rejects_invalid_inputs() {
        // Empty
        assert!(encode_oft_recipient(NetworkTransport::Solana, "").is_err());
        // 31 chars of '1' -> 31 zero bytes (wrong length)
        assert!(
            encode_oft_recipient(NetworkTransport::Solana, "1111111111111111111111111111111")
                .is_err()
        );
        // Hex string contains '0' which is not a valid base58 alphabet char
        assert!(
            encode_oft_recipient(
                NetworkTransport::Solana,
                "0x0000000000000000000000000000000000000000000000000000000000000000",
            )
            .is_err()
        );
        // Contains 'O', '0', 'I', 'l' — none are in the base58 alphabet
        assert!(
            encode_oft_recipient(NetworkTransport::Solana, "O0Il1111111111111111111111111111")
                .is_err()
        );
    }

    #[macros::test_all]
    fn tron_recipient_decodes_known_address() {
        // TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t is the Tron USDT contract.
        // After stripping the 0x41 prefix, the 20-byte payload is
        // a614f803b6fd780986a42c78ec9c7f77e6ded13c.
        let encoded =
            encode_oft_recipient(NetworkTransport::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t")
                .unwrap();
        assert_eq!(&encoded[..12], &[0u8; 12]);
        assert_eq!(
            &encoded[12..],
            &hex::decode("a614f803b6fd780986a42c78ec9c7f77e6ded13c").unwrap()[..]
        );
    }

    #[macros::test_all]
    fn tron_recipient_rejects_invalid_inputs() {
        // Truncated (decodes to fewer than 25 bytes)
        assert!(
            encode_oft_recipient(NetworkTransport::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6")
                .is_err()
        );
        // Last char tweaked -> bad checksum
        assert!(
            encode_oft_recipient(NetworkTransport::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6u")
                .is_err()
        );
        // Garbage
        assert!(encode_oft_recipient(NetworkTransport::Tron, "TInvalidAddress").is_err());
    }

    #[macros::test_all]
    fn tron_recipient_rejects_valid_base58check_with_wrong_prefix() {
        // The Bitcoin Genesis block coinbase address: a real base58check
        // payload that decodes to 25 bytes with a *valid* double-SHA256
        // checksum, but starts with 0x00 (Bitcoin P2PKH) rather than 0x41
        // (Tron). Exercises the prefix-check branch explicitly.
        let err =
            encode_oft_recipient(NetworkTransport::Tron, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
                .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("prefix"), "expected prefix error, got: {msg}");
    }

    #[macros::test_all]
    fn validator_dispatches_per_transport() {
        assert!(is_valid_destination_address(
            NetworkTransport::Evm,
            "0x1234567890AbCdEf1234567890aBcDeF12345678"
        ));
        assert!(!is_valid_destination_address(
            NetworkTransport::Evm,
            "TInvalid"
        ));

        assert!(is_valid_destination_address(
            NetworkTransport::Solana,
            "11111111111111111111111111111111"
        ));
        assert!(!is_valid_destination_address(
            NetworkTransport::Solana,
            "0x1234567890AbCdEf1234567890aBcDeF12345678"
        ));

        assert!(is_valid_destination_address(
            NetworkTransport::Tron,
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"
        ));
        assert!(!is_valid_destination_address(
            NetworkTransport::Tron,
            "TInvalid"
        ));
    }

    #[macros::test_all]
    fn normalize_token_address_per_transport() {
        // EVM: lowercased and trimmed so mixed-case / EIP-55 pastes compare equal.
        assert_eq!(
            normalize_token_address(
                NetworkTransport::Evm,
                "  0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9 "
            ),
            "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"
        );
        // Solana: case preserved (base58 is case-sensitive), only trimmed.
        assert_eq!(
            normalize_token_address(NetworkTransport::Solana, " Es9vMFrz "),
            "Es9vMFrz"
        );
        // Tron: case preserved, only trimmed.
        assert_eq!(
            normalize_token_address(NetworkTransport::Tron, " TR7NHqje "),
            "TR7NHqje"
        );
    }
}
