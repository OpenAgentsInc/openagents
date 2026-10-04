use bip32::{ChildNumber, XPrv};
use k256::ecdsa::SigningKey;
use sha2::{Digest, Sha256};
use tiny_keccak::{Hasher, Keccak};
use zeroize::Zeroizing;

use crate::error::BoltzError;

/// Manages EVM key derivation from a BIP-32 master seed.
///
/// Key identity model:
/// - **Gas signer** (`m/44/{chainId}/1/0`): Signs all EVM transactions, used as `claimAddress`.
/// - **Per-swap preimage key** (`m/44/{chainId}/0/0/{index}`): Deterministic preimage source.
///
/// All derivation levels are NON-HARDENED so that the same mnemonic
/// produces the same keys across any client using the same derivation
/// scheme — including the Boltz backend's restore endpoint
/// (`POST /v2/swap/restore`), which derives child public keys from the
/// master xpub without access to private keys. Hardened derivation would
/// make that restore path impossible.
///
/// As of 2026-04, restore only works for BTC/L-BTC swaps (the backend
/// doesn't store `claimPublicKey` for EVM swaps), but there are no
/// technical blockers preventing future support.
pub struct EvmKeyManager {
    master_key: XPrv,
}

impl EvmKeyManager {
    /// Create from raw seed bytes.
    /// Uses BIP-32 over secp256k1 (matching `@scure/bip32`).
    pub fn from_seed(seed: &[u8]) -> Result<Self, BoltzError> {
        let master_key =
            XPrv::new(seed).map_err(|e| BoltzError::Signing(format!("BIP-32 seed error: {e}")))?;
        Ok(Self { master_key })
    }

    /// Derive gas abstraction signer at `m/44/{chain_id}/1/0`.
    /// This key signs all EVM transactions and is used as `claimAddress` with Boltz.
    /// All levels are NON-HARDENED.
    pub fn derive_gas_signer(&self, chain_id: u32) -> Result<EvmKeyPair, BoltzError> {
        let path = [
            child_number(44)?,
            child_number(chain_id)?,
            child_number(1)?,
            child_number(0)?,
        ];
        self.derive_key_pair(&path)
    }

    /// Derive per-swap preimage key at `m/44/{chain_id}/0/0/{index}`.
    /// Used ONLY for preimage derivation and sending the public key to Boltz — NOT for signing.
    pub fn derive_preimage_key(&self, chain_id: u32, index: u32) -> Result<EvmKeyPair, BoltzError> {
        let path = [
            child_number(44)?,
            child_number(chain_id)?,
            child_number(0)?,
            child_number(0)?,
            child_number(index)?,
        ];
        self.derive_key_pair(&path)
    }

    /// Derive preimage for a swap: `SHA256(private_key_at_preimage_path)`.
    /// Deterministic — same seed + index always produces same preimage.
    pub fn derive_preimage(&self, chain_id: u32, index: u32) -> Result<[u8; 32], BoltzError> {
        let key_pair = self.derive_preimage_key(chain_id, index)?;
        let private_key_bytes = key_pair.private_key_bytes();
        let preimage = Sha256::digest(private_key_bytes);
        Ok(preimage.into())
    }

    /// Derive preimage hash: `SHA256(preimage) = SHA256(SHA256(private_key))`.
    pub fn derive_preimage_hash(&self, chain_id: u32, index: u32) -> Result<[u8; 32], BoltzError> {
        let preimage = self.derive_preimage(chain_id, index)?;
        let hash = Sha256::digest(preimage);
        Ok(hash.into())
    }

    fn derive_key_pair(&self, path: &[ChildNumber]) -> Result<EvmKeyPair, BoltzError> {
        let mut derived = self.master_key.clone();
        for &child in path {
            derived = derived
                .derive_child(child)
                .map_err(|e| BoltzError::Signing(format!("BIP-32 derivation error: {e}")))?;
        }

        let key_bytes = Zeroizing::new(derived.to_bytes());
        let signing_key = SigningKey::from_bytes((&*key_bytes).into())
            .map_err(|e| BoltzError::Signing(format!("Invalid derived key: {e}")))?;

        Ok(EvmKeyPair::from_signing_key(signing_key))
    }
}

/// An EVM-compatible secp256k1 key pair.
pub struct EvmKeyPair {
    signing_key: SigningKey,
    /// Compressed secp256k1 public key (33 bytes).
    pub public_key: Vec<u8>,
    /// Ethereum address: `keccak256(uncompressed_pubkey[1..])[12..]`.
    pub address: [u8; 20],
}

impl EvmKeyPair {
    /// Build a key pair from raw private-key bytes (e.g. a stored seedless gas
    /// key). Errors if the bytes aren't a valid secp256k1 scalar.
    pub(crate) fn from_private_key_bytes(bytes: &[u8; 32]) -> Result<Self, BoltzError> {
        let signing_key = SigningKey::from_bytes(bytes.into())
            .map_err(|e| BoltzError::Signing(format!("Invalid private key: {e}")))?;
        Ok(Self::from_signing_key(signing_key))
    }

    /// Generate a fresh random key pair. Used for throwaway keys (e.g. the
    /// probe invoice's claim address, which is never claimed).
    pub(crate) fn generate() -> Result<Self, BoltzError> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        getrandom::getrandom(&mut *bytes)
            .map_err(|e| BoltzError::Signing(format!("Failed to generate key: {e}")))?;
        Self::from_private_key_bytes(&bytes)
    }

    fn from_signing_key(signing_key: SigningKey) -> Self {
        let verifying_key = signing_key.verifying_key();

        // Compressed public key (33 bytes)
        let public_key = verifying_key.to_sec1_bytes().to_vec();

        // Ethereum address: keccak256 of uncompressed point (without 0x04 prefix), take last 20 bytes
        let uncompressed = verifying_key.to_encoded_point(false);
        let uncompressed_bytes = uncompressed.as_bytes();
        // Skip the 0x04 prefix byte
        let address = keccak256_to_address(&uncompressed_bytes[1..]);

        Self {
            signing_key,
            public_key,
            address,
        }
    }

    /// Returns the signing key reference.
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Returns the raw private key bytes (32 bytes), wrapped in `Zeroizing` so
    /// the copy is wiped when the caller drops it (the `SigningKey` self-zeroizes
    /// its own storage, but this standalone copy would otherwise linger).
    pub(crate) fn private_key_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing_key.to_bytes().into())
    }

    /// Returns the Ethereum address as a hex string with `0x` prefix (checksummed).
    pub fn address_hex(&self) -> String {
        checksum_address(&self.address)
    }
}

/// Compute `keccak256(data)[12..]` to get an Ethereum address.
fn keccak256_to_address(data: &[u8]) -> [u8; 20] {
    let hash = keccak256(data);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

/// Compute keccak256 hash of data.
pub(crate) fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Produce an EIP-55 checksummed Ethereum address.
fn checksum_address(address: &[u8; 20]) -> String {
    let hex_addr = hex::encode(address);
    let hash = keccak256(hex_addr.as_bytes());

    let mut checksummed = String::with_capacity(42);
    checksummed.push_str("0x");
    for (i, c) in hex_addr.chars().enumerate() {
        if c.is_ascii_alphabetic() {
            // Each hex char of the hash corresponds to 4 bits.
            // Use the high nibble of the corresponding hash byte for even indices,
            // low nibble for odd indices.
            let hash_nibble = if i % 2 == 0 {
                hash[i / 2] >> 4
            } else {
                hash[i / 2] & 0x0f
            };
            if hash_nibble >= 8 {
                checksummed.push(c.to_ascii_uppercase());
            } else {
                checksummed.push(c.to_ascii_lowercase());
            }
        } else {
            checksummed.push(c);
        }
    }
    checksummed
}

fn child_number(index: u32) -> Result<ChildNumber, BoltzError> {
    ChildNumber::new(index, false)
        .map_err(|e| BoltzError::Signing(format!("Invalid BIP-32 child number: {e}")))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    // Test vector: known mnemonic -> known derived keys.
    // We use the BIP-39 mnemonic "abandon" x 11 + "about" to derive a seed,
    // then verify key derivation matches expected results.
    //
    // This mnemonic is a well-known test vector. The seed (with empty passphrase) is:
    // 5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4
    const TEST_SEED_HEX: &str = "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4";

    fn test_seed() -> Vec<u8> {
        hex::decode(TEST_SEED_HEX).unwrap()
    }

    #[macros::test_all]
    fn test_from_seed() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed);
        assert!(manager.is_ok());
    }

    #[macros::test_all]
    fn test_from_seed_too_short() {
        let short_seed = vec![0u8; 15];
        let result = EvmKeyManager::from_seed(&short_seed);
        assert!(result.is_err());
    }

    #[macros::test_all]
    fn test_derive_gas_signer_deterministic() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key1 = manager.derive_gas_signer(42161).unwrap();
        let key2 = manager.derive_gas_signer(42161).unwrap();

        assert_eq!(key1.address, key2.address);
        assert_eq!(key1.public_key, key2.public_key);
    }

    #[macros::test_all]
    fn test_derive_preimage_key_deterministic() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key1 = manager.derive_preimage_key(42161, 0).unwrap();
        let key2 = manager.derive_preimage_key(42161, 0).unwrap();

        assert_eq!(key1.address, key2.address);
        assert_eq!(key1.public_key, key2.public_key);
    }

    #[macros::test_all]
    fn test_different_indices_produce_different_keys() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key0 = manager.derive_preimage_key(42161, 0).unwrap();
        let key1 = manager.derive_preimage_key(42161, 1).unwrap();

        assert_ne!(key0.address, key1.address);
        assert_ne!(key0.public_key, key1.public_key);
    }

    #[macros::test_all]
    fn test_different_chains_produce_different_keys() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let arb_key = manager.derive_gas_signer(42161).unwrap();
        let eth_key = manager.derive_gas_signer(1).unwrap();

        assert_ne!(arb_key.address, eth_key.address);
    }

    #[macros::test_all]
    fn test_gas_signer_vs_preimage_key_different() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let gas = manager.derive_gas_signer(42161).unwrap();
        let preimage = manager.derive_preimage_key(42161, 0).unwrap();

        assert_ne!(gas.address, preimage.address);
    }

    #[macros::test_all]
    fn test_preimage_deterministic() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let preimage1 = manager.derive_preimage(42161, 0).unwrap();
        let preimage2 = manager.derive_preimage(42161, 0).unwrap();

        assert_eq!(preimage1, preimage2);
    }

    #[macros::test_all]
    fn test_preimage_is_sha256_of_private_key() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key = manager.derive_preimage_key(42161, 0).unwrap();
        let preimage = manager.derive_preimage(42161, 0).unwrap();

        // Verify: preimage = SHA256(private_key)
        let expected = Sha256::digest(key.private_key_bytes());
        assert_eq!(preimage, &expected[..]);
    }

    #[macros::test_all]
    fn test_preimage_hash_is_double_sha256() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let preimage = manager.derive_preimage(42161, 0).unwrap();
        let preimage_hash = manager.derive_preimage_hash(42161, 0).unwrap();

        // Verify: preimage_hash = SHA256(preimage)
        let expected = Sha256::digest(preimage);
        assert_eq!(preimage_hash, &expected[..]);
    }

    #[macros::test_all]
    fn test_compressed_public_key_length() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key = manager.derive_gas_signer(42161).unwrap();
        assert_eq!(key.public_key.len(), 33);
        // Compressed public key starts with 0x02 or 0x03
        assert!(key.public_key[0] == 0x02 || key.public_key[0] == 0x03);
    }

    #[macros::test_all]
    fn test_address_hex_format() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();

        let key = manager.derive_gas_signer(42161).unwrap();
        let addr = key.address_hex();

        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42); // "0x" + 40 hex chars
    }

    #[macros::test_all]
    fn test_checksum_address_eip55() {
        // EIP-55 test vector
        let addr_bytes: [u8; 20] = hex::decode("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed")
            .unwrap()
            .try_into()
            .unwrap();
        let checksummed = checksum_address(&addr_bytes);
        assert_eq!(checksummed, "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");

        let addr_bytes2: [u8; 20] = hex::decode("fB6916095ca1df60bB79Ce92cE3Ea74c37c5d359")
            .unwrap()
            .try_into()
            .unwrap();
        let checksummed2 = checksum_address(&addr_bytes2);
        assert_eq!(checksummed2, "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359");
    }

    #[macros::test_all]
    fn test_keccak256_known_vector() {
        // keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let hash = keccak256(b"");
        assert_eq!(
            hex::encode(hash),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    // Cross-referenced test vectors: values verified against `@scure/bip32`
    // using the same seed. See test_vectors/generate.mjs for the JS script.
    //
    // Seed: "abandon" x11 + "about" mnemonic (empty passphrase)
    // All paths use NON-HARDENED derivation.

    #[macros::test_all]
    fn test_known_derivation_gas_signer() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();
        let gas_signer = manager.derive_gas_signer(42161).unwrap();

        // Verified against @scure/bip32 output (m/44/42161/1/0)
        assert_eq!(
            hex::encode(gas_signer.private_key_bytes()),
            "0a7bd36b28aacbc176c44ce15840c40d2fa279a77566aad130dbd77016379c2b"
        );
        assert_eq!(
            hex::encode(&gas_signer.public_key),
            "03c323462632d86451a45b6cdf14a63501a9924ae26c8410c4156f6a368c6e6441"
        );
        // Address comparison is case-insensitive (EIP-55 checksum may differ)
        assert_eq!(
            hex::encode(gas_signer.address),
            "323f3d3cd440ad067a8d6ceb8c9bf2252c5779da"
        );
    }

    #[macros::test_all]
    fn test_known_derivation_preimage_key_0() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();
        let key = manager.derive_preimage_key(42161, 0).unwrap();
        let preimage = manager.derive_preimage(42161, 0).unwrap();
        let preimage_hash = manager.derive_preimage_hash(42161, 0).unwrap();

        // Verified against @scure/bip32 + @noble/hashes output (m/44/42161/0/0/0)
        assert_eq!(
            hex::encode(key.private_key_bytes()),
            "c392850d58fd4d6a191840736acdf2f742a5317ac44c88b9473ae4442e2470c0"
        );
        assert_eq!(
            hex::encode(&key.public_key),
            "03fb98906a07bce0298a88d351c789df1b3dbdab15ee6d495c2bbccece0487dfda"
        );
        assert_eq!(
            hex::encode(key.address),
            "a1fa6a313c1d7593f31cf39ec2d8923cc3bd147d"
        );
        assert_eq!(
            hex::encode(preimage),
            "d8cbe170f3ba98b39bb7580a562c867515af94d560c989df701d70a6816d5095"
        );
        assert_eq!(
            hex::encode(preimage_hash),
            "1e3884aa33eba2cee5711d9191e6429d60013d7a27d45642ecd216a1a1fd3226"
        );
    }

    #[macros::test_all]
    fn test_known_derivation_preimage_key_1() {
        let seed = test_seed();
        let manager = EvmKeyManager::from_seed(&seed).unwrap();
        let key = manager.derive_preimage_key(42161, 1).unwrap();
        let preimage = manager.derive_preimage(42161, 1).unwrap();
        let preimage_hash = manager.derive_preimage_hash(42161, 1).unwrap();

        // Verified against @scure/bip32 + @noble/hashes output (m/44/42161/0/0/1)
        assert_eq!(
            hex::encode(key.private_key_bytes()),
            "70dd2064898b13ada4102f0bf4790973123239e8f0bda6727316593eb0acbcac"
        );
        assert_eq!(
            hex::encode(&key.public_key),
            "02cac2a960b7f77caf7d237f23375af4cdd6ca5d44f55d7a38a92987c5bca0e962"
        );
        assert_eq!(
            hex::encode(key.address),
            "077de11fdc539df4004308a262c4d8be35a2a927"
        );
        assert_eq!(
            hex::encode(preimage),
            "affbee9d714f148d1560df3436566ace8bc29e11b3f2cb351c399b6ac40040ec"
        );
        assert_eq!(
            hex::encode(preimage_hash),
            "31b7a2db5911ce028a01063bfd176b6cf609fd4d31c3be26005d97028800dadf"
        );
    }
}
