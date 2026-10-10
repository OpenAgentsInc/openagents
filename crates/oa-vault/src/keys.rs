//! The vault master key and the keys derived from it (NIP-VAULT "Keys").

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use crate::{Error, Result, hex, random, sha256};

pub const INFO_USER_WRAP: &[u8] = b"openagents.vault.v1/user-wrap";
pub const INFO_USER_SHARE: &[u8] = b"openagents.vault.v1/user-share";
pub const INFO_INDEX: &[u8] = b"openagents.vault.v1/index\0";
pub const KEY_ID_PREFIX: &[u8] = b"openagents.vault.v1/key-id\0";

/// HKDF-SHA256 to 32 bytes.
pub(crate) fn hkdf(ikm: &[u8], salt: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, out.as_mut())
        .unwrap_or_else(|_| unreachable!("32 bytes is a valid HKDF-SHA256 length"));
    out
}

/// AES-256-GCM seal: ciphertext followed by the 16-byte tag.
pub(crate) fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    Aes256Gcm::new(key.into())
        .encrypt(Nonce::from_slice(nonce), Payload { msg: plain, aad })
        .map_err(|_| Error::Refused("The data is too large to seal."))
}

/// AES-256-GCM open.
pub(crate) fn open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    sealed: &[u8],
    what: &'static str,
) -> Result<Zeroizing<Vec<u8>>> {
    Aes256Gcm::new(key.into())
        .decrypt(Nonce::from_slice(nonce), Payload { msg: sealed, aad })
        .map(Zeroizing::new)
        .map_err(|_| Error::Decrypt(what))
}

/// The vault master key: 32 random bytes made on the person's first device.
/// It leaves a device only inside a key slot.
pub struct Vmk(Zeroizing<[u8; 32]>);

impl Vmk {
    /// A new random master key.
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<32>()?)))
    }

    /// The key from its 32 bytes (a slot was opened, or a test vector).
    pub fn from_bytes(mut bytes: [u8; 32]) -> Self {
        let key = Self(Zeroizing::new(bytes));
        bytes.zeroize();
        key
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// `K_user_wrap = HKDF-SHA256(ikm=VMK, salt=empty, info="openagents.vault.v1/user-wrap")`.
    pub fn user_wrap_key(&self) -> Zeroizing<[u8; 32]> {
        hkdf(self.0.as_ref(), &[], INFO_USER_WRAP)
    }

    /// `S_user`, the person's share for the `sealed` tier.
    pub fn user_share(&self) -> Zeroizing<[u8; 32]> {
        hkdf(self.0.as_ref(), &[], INFO_USER_SHARE)
    }

    /// `user_key_id`: the first 16 bytes of
    /// `SHA-256("openagents.vault.v1/key-id\0" ‖ K_user_wrap)`, as 32 hex
    /// characters. Public.
    pub fn user_key_id(&self) -> String {
        let wrap = self.user_wrap_key();
        let mut input = Zeroizing::new(KEY_ID_PREFIX.to_vec());
        input.extend_from_slice(wrap.as_ref());
        hex(&sha256(&input)[..16])
    }

    /// The key index key of `epoch`:
    /// `HKDF-SHA256(ikm=VMK, salt=empty, info="openagents.vault.v1/index\0" ‖ be32(epoch))`.
    pub fn index_key(&self, epoch: u32) -> Zeroizing<[u8; 32]> {
        let mut info = INFO_INDEX.to_vec();
        info.extend_from_slice(&epoch.to_be_bytes());
        hkdf(self.0.as_ref(), &[], &info)
    }

    /// Whether two handles hold the same key (constant time is not needed:
    /// both sides are the caller's own key).
    pub fn same(&self, other: &Self) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

/// An object's data key: 32 random bytes, only ever stored wrapped.
pub struct Dek(Zeroizing<[u8; 32]>);

impl Dek {
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<32>()?)))
    }

    pub fn from_bytes(mut bytes: [u8; 32]) -> Self {
        let key = Self(Zeroizing::new(bytes));
        bytes.zeroize();
        key
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
