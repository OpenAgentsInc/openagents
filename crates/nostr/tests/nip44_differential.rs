//! Differential tests: the hand-written NIP-44 primitives against the
//! RustCrypto implementations (`hmac`, `hkdf`, `sha2`, `chacha20`, `subtle`).
//!
//! The RustCrypto crates are dev-dependencies only. Each test checks a fixed
//! set of boundary sizes and then a run of `proptest` cases over random
//! inputs. Read `docs/nostr/crypto-primitives.md` for what these tests do and
//! do not establish.

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use nostr::nip44::primitives;
use proptest::prelude::*;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Sizes around the SHA-256 block (64), the SHA-256 digest (32), the
/// ChaCha20 block (64), and the empty input.
const BOUNDARY_SIZES: &[usize] = &[0, 1, 31, 32, 33, 55, 56, 63, 64, 65, 127, 128, 129, 1000];

fn pattern(length: usize, seed: u8) -> Vec<u8> {
    (0..length)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn oracle_hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts any key");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

fn oracle_hkdf_expand(prk: &[u8; 32], info: &[u8]) -> [u8; 76] {
    let hkdf = Hkdf::<Sha256>::from_prk(prk).expect("32-byte PRK");
    let mut okm = [0_u8; 76];
    hkdf.expand(info, &mut okm)
        .expect("76 bytes is within HKDF's limit");
    okm
}

fn oracle_chacha20(key: &[u8; 32], nonce: &[u8; 12], data: &mut [u8]) {
    let mut cipher = ChaCha20::new(key.into(), nonce.into());
    cipher.seek(0_u32);
    cipher.apply_keystream(data);
}

#[test]
fn hmac_sha256_matches_rustcrypto_at_boundary_sizes() {
    for &key_length in BOUNDARY_SIZES {
        for &message_length in BOUNDARY_SIZES {
            let key = pattern(key_length, 7);
            let message = pattern(message_length, 13);
            assert_eq!(
                primitives::hmac_sha256(&key, &message),
                oracle_hmac(&key, &message),
                "key {key_length} bytes, message {message_length} bytes"
            );
        }
    }
}

#[test]
fn hkdf_expand_matches_rustcrypto_at_boundary_sizes() {
    for &info_length in BOUNDARY_SIZES {
        let key: [u8; 32] = pattern(32, 3).try_into().unwrap();
        let info = pattern(info_length, 5);
        assert_eq!(
            primitives::hkdf_expand(&key, &info),
            oracle_hkdf_expand(&key, &info),
            "info {info_length} bytes"
        );
    }
}

#[test]
fn hkdf_expand_over_a_long_key_matches_rustcrypto_extract_then_expand() {
    // NIP-44 never does this, but the hand-written expand accepts any key
    // length, so pin what it computes: HKDF-Expand with the key used as the
    // PRK directly, which HMAC normalizes the same way the oracle does.
    let key = pattern(100, 9);
    let info = pattern(32, 11);
    let mut okm = [0_u8; 76];
    Hkdf::<Sha256>::from_prk(&key)
        .expect("HKDF accepts a PRK at least one digest long")
        .expand(&info, &mut okm)
        .unwrap();
    assert_eq!(primitives::hkdf_expand(&key, &info), okm);
}

#[test]
fn chacha20_matches_rustcrypto_at_boundary_sizes() {
    let key: [u8; 32] = pattern(32, 17).try_into().unwrap();
    let nonce: [u8; 12] = pattern(12, 19).try_into().unwrap();
    for &length in BOUNDARY_SIZES.iter().chain(&[191, 192, 193, 65_536]) {
        let plaintext = pattern(length, 23);
        let mut ours = plaintext.clone();
        primitives::chacha20_xor(&key, &nonce, &mut ours);
        let mut theirs = plaintext.clone();
        oracle_chacha20(&key, &nonce, &mut theirs);
        assert_eq!(ours, theirs, "{length} bytes");
    }
}

#[test]
fn chacha20_rfc8439_section_2_4_2_vector() {
    // RFC 8439 section 2.4.2 encrypts from counter 1. The hand-written
    // cipher always starts at counter 0, as NIP-44 requires, so prepend one
    // block of zeros and discard it to reach the RFC's ciphertext.
    let key: [u8; 32] = (0_u8..32).collect::<Vec<_>>().try_into().unwrap();
    let nonce = [0, 0, 0, 0, 0, 0, 0, 0x4a, 0, 0, 0, 0];
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let mut padded = vec![0_u8; 64];
    padded.extend_from_slice(plaintext);
    primitives::chacha20_xor(&key, &nonce, &mut padded);
    let ciphertext = &padded[64..];
    assert_eq!(
        &ciphertext[..8],
        &[0x6e, 0x2e, 0x35, 0x9a, 0x25, 0x68, 0xf9, 0x80]
    );
    assert_eq!(
        &ciphertext[ciphertext.len() - 4..],
        &[0x5e, 0x42, 0x87, 0x4d]
    );
}

#[test]
fn constant_time_equal_matches_subtle_at_boundary_sizes() {
    for &length in BOUNDARY_SIZES {
        let left = pattern(length, 29);
        let mut right = left.clone();
        assert!(primitives::constant_time_equal(&left, &right));
        assert_eq!(
            primitives::constant_time_equal(&left, &right),
            bool::from(left.ct_eq(&right))
        );
        if length > 0 {
            for position in [0, length / 2, length - 1] {
                right[position] ^= 0x80;
                assert_eq!(
                    primitives::constant_time_equal(&left, &right),
                    bool::from(left.ct_eq(&right)),
                    "{length} bytes differing at {position}"
                );
                right[position] ^= 0x80;
            }
        }
        let longer = pattern(length + 1, 29);
        assert!(!primitives::constant_time_equal(&left, &longer));
        assert!(!bool::from(left.ct_eq(&longer)));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn hmac_sha256_matches_rustcrypto(
        key in prop::collection::vec(any::<u8>(), 0..200),
        message in prop::collection::vec(any::<u8>(), 0..2048),
    ) {
        prop_assert_eq!(primitives::hmac_sha256(&key, &message), oracle_hmac(&key, &message));
    }

    #[test]
    fn hkdf_expand_matches_rustcrypto(
        key in any::<[u8; 32]>(),
        info in prop::collection::vec(any::<u8>(), 0..200),
    ) {
        prop_assert_eq!(primitives::hkdf_expand(&key, &info), oracle_hkdf_expand(&key, &info));
    }

    #[test]
    fn message_keys_match_rustcrypto_hkdf(
        conversation_key in any::<[u8; 32]>(),
        nonce in any::<[u8; 32]>(),
    ) {
        prop_assert_eq!(
            primitives::message_keys(&conversation_key, &nonce),
            oracle_hkdf_expand(&conversation_key, &nonce)
        );
    }

    #[test]
    fn chacha20_matches_rustcrypto(
        key in any::<[u8; 32]>(),
        nonce in any::<[u8; 12]>(),
        plaintext in prop::collection::vec(any::<u8>(), 0..4096),
    ) {
        let mut ours = plaintext.clone();
        primitives::chacha20_xor(&key, &nonce, &mut ours);
        let mut theirs = plaintext;
        oracle_chacha20(&key, &nonce, &mut theirs);
        prop_assert_eq!(ours, theirs);
    }

    #[test]
    fn constant_time_equal_matches_subtle(
        left in prop::collection::vec(any::<u8>(), 0..128),
        right in prop::collection::vec(any::<u8>(), 0..128),
    ) {
        prop_assert_eq!(
            primitives::constant_time_equal(&left, &right),
            bool::from(left.ct_eq(&right))
        );
        prop_assert_eq!(primitives::constant_time_equal(&left, &right), left == right);
    }

    #[test]
    fn constant_time_equal_detects_a_single_flipped_bit(
        bytes in prop::collection::vec(any::<u8>(), 1..128),
        position in any::<prop::sample::Index>(),
        bit in 0_u8..8,
    ) {
        let mut flipped = bytes.clone();
        flipped[position.index(bytes.len())] ^= 1 << bit;
        prop_assert!(!primitives::constant_time_equal(&bytes, &flipped));
        prop_assert!(!bool::from(bytes.ct_eq(&flipped)));
    }
}
