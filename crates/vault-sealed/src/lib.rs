//! NIP-VAULT tier `sealed`: a person's client and an attested workload.
//!
//! - [`client`]: verify the attested `vault` endpoint through `oa-att`,
//!   accept a system key certificate only from that endpoint, seal objects
//!   with the `sealed` wrap (and an optional device copy), sign leases and
//!   revocations, and open the replies.
//! - [`workload`]: the attested workload's side: one system key per person
//!   per epoch whose private half exists only as KMS ciphertext, session
//!   requests opened with the endpoint's HPKE key, exact lease admission,
//!   single use, revocation, and an audit log with no content or key bytes.
//! - [`kms`]: the key-release boundary that only the attested release may
//!   pass.
//!
//! Tier `user` (device-only) is unchanged in `oa-vault`.
pub mod client;
pub mod kms;
pub mod workload;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub type Result<T> = std::result::Result<T, String>;

/// The ATT workload slug a vault endpoint runs.
pub const WORKLOAD: &str = "vault";
/// The longest a lease may live.
pub const MAX_LEASE_SECONDS: u64 = 86_400;

/// A body and the Schnorr signature of its signer over SHA-256(JCS(body)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed {
    pub body: Value,
    pub signer: String,
    pub sig: String,
}

fn digest(body: &Value) -> Result<[u8; 32]> {
    let bytes = nostr::contracts::jcs(body).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(&bytes).into())
}

impl Signed {
    pub fn sign(body: Value, secret: &secp256k1::SecretKey) -> Result<Self> {
        let secp = secp256k1::Secp256k1::new();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, secret);
        let sig = secp.sign_schnorr_no_aux_rand(&digest(&body)?, &keypair);
        Ok(Self {
            signer: keypair.x_only_public_key().0.to_string(),
            sig: sig.to_string(),
            body,
        })
    }
    /// Verify against an expected signer.
    pub fn verify(&self, signer: &str) -> Result<()> {
        if self.signer != signer {
            return Err("identity_mismatch: signed by another key".into());
        }
        let key: secp256k1::XOnlyPublicKey = signer.parse().map_err(|_| "malformed: signer key")?;
        let sig: secp256k1::schnorr::Signature =
            self.sig.parse().map_err(|_| "malformed: signature")?;
        secp256k1::Secp256k1::verification_only()
            .verify_schnorr(&sig, &digest(&self.body)?, &key)
            .map_err(|_| "identity_mismatch: the signature does not verify".into())
    }
}

/// The 32 raw bytes of a 64-hex id.
pub fn id_bytes(id: &str) -> Result<[u8; 32]> {
    oa_vault::unhex::<32>(id).map_err(|_| "malformed: id".to_string())
}

#[cfg(test)]
mod tests;
