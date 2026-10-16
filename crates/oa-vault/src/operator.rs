//! The `operator` tier's wrap (NIP-VAULT, "Wraps"): the data key encrypted
//! by a named KMS key version with additional authenticated data
//! `core_digest`. Anyone holding decrypt on that key can open it, and every
//! use is logged by the key service. Clients label this tier as readable by
//! the operator; it is not the `user` or `sealed` guarantee.
use crate::keys::Dek;
use crate::object::Wrap;
use crate::{Error, Result, b64, unb64};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// A key service holding the operator key. Implementations log every use.
pub trait OperatorKms {
    /// The key version resource name new wraps use.
    fn key_version(&self) -> String;
    fn encrypt(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(
        &mut self,
        key_version: &str,
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>>;
}

/// The `operator` wrap of `dek` under the service's current key version.
pub fn wrap_operator(kms: &mut dyn OperatorKms, digest: &[u8; 32], dek: &Dek) -> Result<Wrap> {
    let sealed = kms.encrypt(digest, dek.bytes())?;
    Ok(Wrap::Operator {
        kms_key: kms.key_version(),
        dek: b64(&sealed),
    })
}

/// Open an `operator` wrap through the key service (a logged use).
pub fn open_operator(kms: &mut dyn OperatorKms, digest: &[u8; 32], wrap: &Wrap) -> Result<Dek> {
    let Wrap::Operator { kms_key, dek } = wrap else {
        return Err(Error::Refused(
            "Only an operator wrap opens through the key service.",
        ));
    };
    let opened = kms.decrypt(kms_key, digest, &unb64(dek)?)?;
    let bytes: [u8; 32] = opened
        .as_slice()
        .try_into()
        .map_err(|_| Error::Format("A wrapped data key has the wrong length."))?;
    Ok(Dek::from_bytes(bytes))
}

/// A local key service with synthetic key versions, for fixtures and
/// self-hosted use. It records every encrypt and decrypt by key version.
pub struct LocalOperatorKms {
    versions: BTreeMap<String, Zeroizing<[u8; 32]>>,
    current: String,
    nonce: Option<[u8; 12]>,
    /// Every use: `(operation, key version)`.
    pub uses: Vec<(String, String)>,
}

impl LocalOperatorKms {
    pub fn new(version: &str, key: [u8; 32]) -> Self {
        Self {
            versions: BTreeMap::from([(version.to_string(), Zeroizing::new(key))]),
            current: version.into(),
            nonce: None,
            uses: Vec::new(),
        }
    }
    /// A new key version; older versions still decrypt.
    pub fn rotate(&mut self, version: &str, key: [u8; 32]) {
        self.versions.insert(version.into(), Zeroizing::new(key));
        self.current = version.into();
    }
    /// Destroy a key version: its wraps no longer open.
    pub fn destroy(&mut self, version: &str) {
        self.versions.remove(version);
    }
    /// Use this nonce for the next encryption (test vectors).
    pub fn with_nonce(mut self, nonce: [u8; 12]) -> Self {
        self.nonce = Some(nonce);
        self
    }
}

impl OperatorKms for LocalOperatorKms {
    fn key_version(&self) -> String {
        self.current.clone()
    }
    fn encrypt(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self
            .versions
            .get(&self.current)
            .ok_or(Error::Refused("The key version is gone."))?;
        let nonce = match self.nonce.take() {
            Some(nonce) => nonce,
            None => crate::random::<12>()?,
        };
        let cipher =
            Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| Error::Format("bad key"))?;
        let mut out = nonce.to_vec();
        out.extend(
            cipher
                .encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: plaintext,
                        aad,
                    },
                )
                .map_err(|_| Error::Refused("The key service could not encrypt."))?,
        );
        self.uses.push(("encrypt".into(), self.current.clone()));
        Ok(out)
    }
    fn decrypt(
        &mut self,
        key_version: &str,
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        self.uses.push(("decrypt".into(), key_version.into()));
        let key = self
            .versions
            .get(key_version)
            .ok_or(Error::Decrypt("No such key version."))?;
        if ciphertext.len() < 12 {
            return Err(Error::Format("The wrapped key is too short."));
        }
        let cipher =
            Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| Error::Format("bad key"))?;
        cipher
            .decrypt(
                Nonce::from_slice(&ciphertext[..12]),
                Payload {
                    msg: &ciphertext[12..],
                    aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| {
                Error::Decrypt("The operator wrap did not open: wrong key or another file.")
            })
    }
}
