//! The key-release boundary. A system key's private half is stored only as
//! KMS ciphertext; the KMS releases it only to the attested release it was
//! bound to (in production, Cloud KMS under a Confidential Space attestation
//! condition). Every release is counted.
use crate::Result;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use zeroize::Zeroizing;

pub trait Kms {
    /// Encrypt key material for the named release.
    fn seal(&mut self, release: &str, plain: &[u8]) -> Result<Vec<u8>>;
    /// Release key material only to the attested release it was sealed for.
    fn unseal(&mut self, release: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>>;
}

/// An in-process KMS bound to one attested release, for fixtures.
pub struct LocalKms {
    key: Zeroizing<[u8; 32]>,
    bound: String,
    pub releases: usize,
}

impl LocalKms {
    pub fn new(key: [u8; 32], bound_release: &str) -> Self {
        Self {
            key: Zeroizing::new(key),
            bound: bound_release.into(),
            releases: 0,
        }
    }
}

impl Kms for LocalKms {
    fn seal(&mut self, release: &str, plain: &[u8]) -> Result<Vec<u8>> {
        if release != self.bound {
            return Err("not_admitted: the KMS key is bound to another release".into());
        }
        let nonce: [u8; 12] = oa_vault::random::<12>().map_err(|e| e.to_string())?;
        let cipher =
            Aes256Gcm::new_from_slice(self.key.as_ref()).map_err(|_| "malformed: kms key")?;
        let mut out = nonce.to_vec();
        out.extend(
            cipher
                .encrypt(Nonce::from_slice(&nonce), plain)
                .map_err(|_| "unavailable: kms seal")?,
        );
        Ok(out)
    }
    fn unseal(&mut self, release: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        if release != self.bound {
            return Err("not_admitted: only the attested release may release this key".into());
        }
        if sealed.len() < 12 {
            return Err("malformed: kms ciphertext".into());
        }
        let cipher =
            Aes256Gcm::new_from_slice(self.key.as_ref()).map_err(|_| "malformed: kms key")?;
        let plain = cipher
            .decrypt(Nonce::from_slice(&sealed[..12]), &sealed[12..])
            .map_err(|_| "identity_mismatch: kms ciphertext")?;
        self.releases += 1;
        Ok(Zeroizing::new(plain))
    }
}
