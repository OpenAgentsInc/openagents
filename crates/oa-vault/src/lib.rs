//! NIP-VAULT client core (#11240): everything a client needs to keep a
//! person's files so that the service holding them stores ciphertext only.
//!
//! - [`keys`]: the vault master key (`VMK`) and what is derived from it.
//! - [`object`]: the `OAVAULT1` object, chunked AES-256-GCM content, and
//!   the `user` wrap of each object's data key.
//! - [`slot`]: key slots, each holding the `VMK` under one unlock method
//!   (passkey PRF, device key, Nostr key, recovery code, pairing link).
//! - [`index`]: the key index, sealed under a per-epoch index key. A file's
//!   data key lives only here, so rewriting the index without it, and
//!   deleting the old epoch, shreds the file.
//! - [`recovery`]: the 24-word recovery code.
//!
//! Formats are normative in `nips/openagents/NIP-VAULT.md`; vectors are in
//! `fixtures/nips/vault/`. Nothing here talks to a network, and no type
//! that holds key material or plaintext implements `Debug` or `Display`.
//! The crate builds for wasm32 (the browser vault, `oa-vault-web`), where
//! randomness comes from `crypto.getRandomValues`.

pub mod index;
pub mod jcs;
pub mod keys;
pub mod object;
pub mod recovery;
pub mod slot;

use std::fmt;

pub use index::{Entry, Index, Kind};
pub use keys::{Dek, Vmk};
pub use object::{Header, Wrap};
pub use slot::{Method, Slot};

/// Why a vault operation failed. The words never carry key material,
/// plaintext, or file names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not a vault object, slot, or index of this version.
    Format(&'static str),
    /// Authenticated decryption failed: wrong key, or the bytes changed.
    Decrypt(&'static str),
    /// The input is well formed but not allowed.
    Refused(&'static str),
    /// The platform's random source failed.
    Random,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(why) | Self::Decrypt(why) | Self::Refused(why) => f.write_str(why),
            Self::Random => f.write_str("The random number source failed."),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// `n` bytes from the operating system's (or browser's) random source.
pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|_| Error::Random)?;
    Ok(bytes)
}

/// A new 32-byte id as 64 lowercase hex characters.
pub fn new_id() -> Result<String> {
    Ok(hex(&random::<32>()?))
}

/// Lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

/// Bytes from lowercase hex of exactly `N` bytes.
pub fn unhex<const N: usize>(text: &str) -> Result<[u8; N]> {
    let raw = text.as_bytes();
    if raw.len() != N * 2 {
        return Err(Error::Format("A hex value has the wrong length."));
    }
    let mut out = [0u8; N];
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let nibble = |c: u8| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            _ => Err(Error::Format("A hex value must be lowercase hex.")),
        };
        out[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(out)
}

/// Whether `id` is a vault id: 64 lowercase hex characters.
pub fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Standard base64 with padding.
pub fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Bytes from standard base64 with padding.
pub fn unb64(text: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| Error::Format("A base64 value is invalid."))
}

/// Exactly `N` bytes from standard base64.
pub fn unb64_n<const N: usize>(text: &str) -> Result<[u8; N]> {
    unb64(text)?
        .try_into()
        .map_err(|_| Error::Format("A base64 value has the wrong length."))
}

/// SHA-256.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests;
