# NIP-06 Implementation Log

**Date:** 2025-12-10
**Crate:** `crates/nostr`

## Summary

Implemented NIP-06 (Basic key derivation from mnemonic seed phrase) for the nostr crate.

## NIP-06 Specification

- BIP39 is used to generate mnemonic seed words and derive a binary seed
- BIP32 is used to derive the path `m/44'/1237'/<account>'/0/0`
- Coin type 1237 is registered in SLIP-0044 for Nostr

## Implementation

### Dependencies (matching spark-sdk)

- `bip39 = "2.2.0"` - BIP39 mnemonic handling
- `bitcoin = "0.32.6"` - BIP32 derivation and secp256k1
- `bech32 = "0.11.0"` - nsec/npub encoding

### Public API

```rust
// Core derivation functions
pub fn derive_keypair(mnemonic: &str) -> Result<Keypair, Nip06Error>;
pub fn derive_keypair_with_account(mnemonic: &str, account: u32) -> Result<Keypair, Nip06Error>;
pub fn derive_keypair_full(mnemonic: &str, passphrase: &str, account: u32) -> Result<Keypair, Nip06Error>;
pub fn mnemonic_to_seed(mnemonic: &str, passphrase: &str) -> Result<[u8; 64], Nip06Error>;

// Bech32 encoding/decoding
pub fn private_key_to_nsec(private_key: &[u8; 32]) -> Result<String, Nip06Error>;
pub fn public_key_to_npub(public_key: &[u8; 32]) -> Result<String, Nip06Error>;
pub fn nsec_to_private_key(nsec: &str) -> Result<[u8; 32], Nip06Error>;
pub fn npub_to_public_key(npub: &str) -> Result<[u8; 32], Nip06Error>;

// Keypair struct
pub struct Keypair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}
impl Keypair {
    pub fn private_key_hex(&self) -> String;
    pub fn public_key_hex(&self) -> String;
    pub fn nsec(&self) -> Result<String, Nip06Error>;
    pub fn npub(&self) -> Result<String, Nip06Error>;
}
```

### Test Coverage (25 tests)

#### Official NIP-06 Test Vectors (2 tests)
- 12-word mnemonic: `leader monkey parrot ring guide accident before fence cannon height naive bean`
- 24-word mnemonic: `what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade`

#### nostr-tools Test Vectors (6 tests)
From https://github.com/nbd-wtf/nostr-tools/blob/master/nip06.test.ts

| Test | Mnemonic | Passphrase | Account | Verified |
|------|----------|------------|---------|----------|
| `zoo_mnemonic_account_0` | zoo×11 wrong | none | 0 | private key |
| `zoo_mnemonic_account_1` | zoo×11 wrong | none | 1 | private key |
| `zoo_mnemonic_with_passphrase` | zoo×11 wrong | "123" | 0 | private key |
| `zoo_mnemonic_account_1_with_passphrase` | zoo×11 wrong | "123" | 1 | private key |
| `zoo_mnemonic_account_1_with_passphrase_full` | zoo×11 wrong | "123" | 1 | private + public |
| `abandon_mnemonic_extended_key_derivation` | abandon×11 about | none | 0 | private + public |

#### Additional Tests (17 tests)
- Mnemonic parsing and seed generation
- Passphrase handling
- Account index derivation (0, 1, 2, high indices)
- nsec/npub roundtrip encoding
- Error handling (invalid mnemonic, wrong HRP, invalid bech32)
- Security (Debug trait redacts private key)
- Determinism verification
- Different mnemonics produce different keys

## Files

- `crates/nostr/Cargo.toml` - Dependencies
- `crates/nostr/src/lib.rs` - Public exports
- `crates/nostr/src/nip06.rs` - Implementation and tests
