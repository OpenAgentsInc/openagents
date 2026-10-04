//! Solana Associated Token Account (ATA) derivation.
//!
//! An ATA is a Program Derived Address (PDA) deterministically computed from
//! `(owner_pubkey, spl_token_program_id, mint_pubkey)` under the Associated
//! Token Program. The caller supplies the raw 32-byte owner and mint pubkeys;
//! `derive_ata` returns the 32-byte ATA pubkey or an error if no viable bump
//! seed exists (cryptographically near-impossible for valid inputs).
//!
//! Algorithm mirrors Solana's `Pubkey::find_program_address` /
//! `Pubkey::create_program_address`: loop a 1-byte bump seed from 255 down to
//! 0; for each, compute `SHA256(seeds || [bump] || program_id || "ProgramDerivedAddress")`
//! and accept the first candidate whose value is **off the ed25519 curve**
//! (PDAs must be off-curve so they can't be controlled by a private key).
//!
//! Off-curve check uses `curve25519-dalek::edwards::CompressedEdwardsY`: the
//! hash bytes are interpreted as a compressed Edwards Y coordinate; if
//! decompression fails, the point is not on the curve and the candidate is
//! a valid PDA.

use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest, Sha256};

use crate::error::BoltzError;

/// SPL Token program ID (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`).
const SPL_TOKEN_PROGRAM_ID: [u8; 32] = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// Associated Token program ID (`ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`).
const ASSOCIATED_TOKEN_PROGRAM_ID: [u8; 32] = [
    0x8c, 0x97, 0x25, 0x8f, 0x4e, 0x24, 0x89, 0xf1, 0xbb, 0x3d, 0x10, 0x29, 0x14, 0x8e, 0x0d, 0x83,
    0x0b, 0x5a, 0x13, 0x99, 0xda, 0xff, 0x10, 0x84, 0x04, 0x8e, 0x7b, 0xd8, 0xdb, 0xe9, 0xf8, 0x59,
];

/// Suffix appended to PDA seed input before hashing. From Solana runtime.
const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

/// Derive the Associated Token Account for `(owner, mint)` under the SPL
/// Token program. Returns the 32-byte ATA pubkey.
pub fn derive_ata(owner: &[u8; 32], mint: &[u8; 32]) -> Result<[u8; 32], BoltzError> {
    let seeds: [&[u8]; 3] = [owner, &SPL_TOKEN_PROGRAM_ID, mint];
    find_program_address(&seeds, &ASSOCIATED_TOKEN_PROGRAM_ID)
}

/// Derive the CCTP `used_nonce` PDA for `nonce` under the Solana
/// `MessageTransmitter` program. Its existence proves `receiveMessage` ran for
/// this message (replay-protection record) — a forwarder-agnostic delivery
/// signal used to complete a swap whose Circle forward stalled. Seeds match
/// Circle's Solana program: `[b"used_nonce", nonce]`.
pub fn derive_used_nonce_pda(
    nonce: &[u8; 32],
    message_transmitter: &[u8; 32],
) -> Result<[u8; 32], BoltzError> {
    let seeds: [&[u8]; 2] = [b"used_nonce", nonce];
    find_program_address(&seeds, message_transmitter)
}

/// Solana `Pubkey::find_program_address` — loop a 1-byte bump seed 255→0
/// and return the first candidate that is off-curve (valid PDA).
fn find_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> Result<[u8; 32], BoltzError> {
    for bump in (0u8..=255).rev() {
        if let Some(candidate) = create_program_address(seeds, bump, program_id) {
            return Ok(candidate);
        }
    }
    Err(BoltzError::Generic(
        "Could not find a viable bump seed for Solana PDA".into(),
    ))
}

/// Hash the seeds (plus bump) with the program id and PDA marker. Returns
/// `Some(pubkey)` if the hash is off-curve, `None` if the hash would land on
/// the ed25519 curve (which would mean the seeds don't produce a valid PDA).
fn create_program_address(seeds: &[&[u8]], bump: u8, program_id: &[u8; 32]) -> Option<[u8; 32]> {
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update([bump]);
    hasher.update(program_id);
    hasher.update(PDA_MARKER);
    let hash: [u8; 32] = hasher.finalize().into();

    if is_on_curve(&hash) { None } else { Some(hash) }
}

/// Returns `true` if the 32-byte value is a valid compressed Edwards Y point
/// on the ed25519 curve — i.e., decodes to a real curve point.
fn is_on_curve(bytes: &[u8; 32]) -> bool {
    CompressedEdwardsY::from_slice(bytes)
        .ok()
        .and_then(|c| c.decompress())
        .is_some()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    fn decode_pubkey(s: &str) -> [u8; 32] {
        let v = bs58::decode(s).into_vec().expect("valid base58");
        assert_eq!(v.len(), 32, "pubkey must decode to 32 bytes");
        let mut out = [0u8; 32];
        out.copy_from_slice(&v);
        out
    }

    #[macros::test_all]
    fn spl_token_program_constant_matches_base58() {
        let expected = decode_pubkey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        assert_eq!(SPL_TOKEN_PROGRAM_ID, expected);
    }

    #[macros::test_all]
    fn associated_token_program_constant_matches_base58() {
        let expected = decode_pubkey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
        assert_eq!(ASSOCIATED_TOKEN_PROGRAM_ID, expected);
    }

    #[macros::test_all]
    fn derive_ata_is_off_curve() {
        let owner = decode_pubkey("DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy");
        let mint = decode_pubkey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        let ata = derive_ata(&owner, &mint).expect("derive");
        assert!(!is_on_curve(&ata), "PDA must be off-curve by construction");
    }

    #[macros::test_all]
    fn derive_ata_is_deterministic() {
        let owner = decode_pubkey("DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy");
        let mint = decode_pubkey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        let a = derive_ata(&owner, &mint).expect("derive 1");
        let b = derive_ata(&owner, &mint).expect("derive 2");
        assert_eq!(a, b);
    }

    #[macros::test_all]
    fn different_owners_produce_different_atas() {
        let mint = decode_pubkey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        let owner_a = decode_pubkey("DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy");
        let owner_b = decode_pubkey("11111111111111111111111111111112");
        let ata_a = derive_ata(&owner_a, &mint).expect("derive a");
        let ata_b = derive_ata(&owner_b, &mint).expect("derive b");
        assert_ne!(ata_a, ata_b);
    }

    #[macros::test_all]
    fn different_mints_produce_different_atas() {
        let owner = decode_pubkey("DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy");
        let mint_a = decode_pubkey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        let mint_b = decode_pubkey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let ata_a = derive_ata(&owner, &mint_a).expect("derive a");
        let ata_b = derive_ata(&owner, &mint_b).expect("derive b");
        assert_ne!(ata_a, ata_b);
    }

    /// Cross-check `derive_used_nonce_pda` against a real mainnet vector: a CCTP
    /// nonce received on Solana, whose derived `used_nonce` PDA exists on-chain
    /// owned by the `MessageTransmitterV2` program (re-verify via
    /// `getAccountInfo`). PDA existence is the signal the probe treats as "mint
    /// landed"; if this derivation drifts, the probe would miss a real delivery
    /// or invent one.
    #[macros::test_all]
    fn derive_used_nonce_pda_matches_mainnet_vector() {
        let transmitter = decode_pubkey("CCTPV2Sm4AdWt5296sk4P66VBZ7bEhcARwFaaS9YPbeC");
        let nonce_vec =
            hex::decode("d7c7c073ec476983e3f222924974a48a7f61a7045df31dcf3ed83172bf0bb478")
                .expect("valid hex");
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&nonce_vec);

        let pda = derive_used_nonce_pda(&nonce, &transmitter).expect("derive");
        assert_eq!(
            bs58::encode(pda).into_string(),
            "28W8xrmyCAKPnWyRKju5GCLp9QQeMu7YPQ7epY6KueiG"
        );
    }

    #[macros::test_all]
    fn curve_check_recognises_known_on_curve_point() {
        // Base58 "11...1" -> all zero bytes, which is a valid ed25519 point
        // (the identity). A PDA must not land here — proves is_on_curve works.
        let zeros = [0u8; 32];
        assert!(is_on_curve(&zeros));
    }

    /// Cross-check `derive_ata` against a real mainnet vector. Pairs a
    /// well-known Solana wallet with the USDT (Tether) SPL mint; the expected
    /// ATA is the only token account for that `(owner, mint)` pair returned
    /// by `api.mainnet.solana.com`'s `getTokenAccountsByOwner` that carries
    /// no `closeAuthority` — i.e., the canonical Associated Token Account as
    /// opposed to ad-hoc token accounts the owner may have created manually.
    /// If this assertion fires, `derive_ata` disagrees with the Solana
    /// runtime and no derived Solana address the library produces can be
    /// trusted.
    #[macros::test_all]
    fn derive_ata_matches_mainnet_vector() {
        // Binance Solana hot wallet.
        let owner = decode_pubkey("5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9");
        // Tether USDT SPL mint (`USDT0-SOL` tokenAddress in the OFT route).
        let mint = decode_pubkey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        let expected = decode_pubkey("CyBjGpte4Npi5zNkdtWumPxVW4kpMR8BuFSbA587xZES");

        let ata = derive_ata(&owner, &mint).expect("derive");
        assert_eq!(ata, expected);
    }
}
