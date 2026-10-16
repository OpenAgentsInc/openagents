//! NIP-ATT sealed-job carriage over the endpoint's HPKE key
//! ([NIP-ATT](../../nips/openagents/NIP-ATT.md#hpke-carriage)).
//!
//! - [`keys`]: admit an endpoint's HPKE key only from a verified endpoint
//!   record whose binding covers it, refuse rollbacks and expired keys, and
//!   let a worker rotate keys with a bounded overlap.
//! - [`request`]: seal a request to the admitted key with HPKE `mode_base`,
//!   bound to the release, endpoint, recipient, request ID and expiry; carry
//!   it signed or gift-wrapped (NIP-59); refuse replays on the worker.
//! - [`stream`]: per-chunk sealed output under a key exported from the
//!   request's HPKE context, bound to the stream position, with output
//!   limits, finality and a closed set of stop outcomes.
//!
//! Nothing here logs, prints or formats a plaintext or a private key.
pub mod keys;
pub mod request;
pub mod stream;

#[cfg(test)]
mod tests;

use base64::Engine as _;
use hpke::{Kem as _, Serializable as _, kem::X25519HkdfSha256};
use zeroize::Zeroizing;

/// The HPKE suite (RFC 9180): DHKEM(X25519, HKDF-SHA256), HKDF-SHA256,
/// AES-256-GCM, the same suite NIP-VAULT's sealed tier uses.
pub const SUITE: &str = "x25519-hkdf-sha256/hkdf-sha256/aes-256-gcm";

pub type Result<T> = std::result::Result<T, String>;

pub(crate) type Kem = X25519HkdfSha256;

pub(crate) fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub(crate) fn unb64(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| "a field is not base64".into())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex32(text: &str) -> Result<[u8; 32]> {
    if text.len() != 64 {
        return Err("a 32-byte field is not 64 hex characters".into());
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| "a field is not hex")?;
    }
    Ok(out)
}

/// An X25519 key pair. The private half is zeroed on drop and never
/// formatted.
pub struct KeyPair {
    private: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyPair({}, private redacted)", hex(&self.public))
    }
}

impl KeyPair {
    /// RFC 9180 DeriveKeyPair from input keying material.
    #[must_use]
    pub fn derive(ikm: &[u8]) -> Self {
        let (private, public) = Kem::derive_keypair(ikm);
        let mut p = [0u8; 32];
        p.copy_from_slice(&private.to_bytes());
        let mut q = [0u8; 32];
        q.copy_from_slice(&public.to_bytes());
        Self {
            private: Zeroizing::new(p),
            public: q,
        }
    }

    /// A fresh key pair from the operating system's random source.
    ///
    /// # Errors
    ///
    /// When the random source fails.
    pub fn generate() -> Result<Self> {
        let mut ikm = Zeroizing::new([0u8; 32]);
        getrandom::fill(ikm.as_mut()).map_err(|_| "the random source failed")?;
        Ok(Self::derive(ikm.as_ref()))
    }

    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        self.public
    }

    pub(crate) fn private(&self) -> &[u8; 32] {
        &self.private
    }
}

/// The random source HPKE draws its ephemeral key from.
pub struct OsRandom;

impl hpke::rand_core::RngCore for OsRandom {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        getrandom::fill(dest).expect("the random source failed");
    }
    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> std::result::Result<(), hpke::rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl hpke::rand_core::CryptoRng for OsRandom {}
