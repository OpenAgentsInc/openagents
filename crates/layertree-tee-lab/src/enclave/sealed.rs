//! Image-bound restart recovery for the signet experiment.
//!
//! The enclave key is encrypted locally. Cloud KMS wraps only a random
//! envelope key, released through Confidential Space workload federation.
//! IAM must bind that release to the exact approved image digest.
use std::{io::Read, time::Duration};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bitcoin::{
    key::XOnlyPublicKey,
    secp256k1::{SecretKey, rand::RngCore},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{Config, Enclave};
use crate::attest::report_data;

const SCHEMA: &str = "openagents.layertree.sealed-key.v1";
const KMS_KEY: &str = "projects/openagentsgemini/locations/us-central1/keyRings/layertree-signet/cryptoKeys/enclave-envelope";
const PROVIDER: &str = "//iam.googleapis.com/projects/157437760789/locations/global/workloadIdentityPools/layertree-signet/providers/attestation-verifier";
const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
type Result<T> = std::result::Result<T, &'static str>;

/// Public encrypted state; it contains no plaintext private key.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedKey {
    pub v: String,
    pub public_key: String,
    pub radix: u32,
    pub binding: String,
    pub nonce: String,
    pub ciphertext: String,
    pub wrapped_envelope_key: String,
}

/// A short-lived session obtained from the launcher's private OIDC token.
/// The VM service account is not given direct KMS decrypt permission.
pub struct GcpKms {
    client: reqwest::blocking::Client,
    access: Zeroizing<String>,
}

fn response(response: reqwest::blocking::Response) -> Result<Value> {
    if !response.status().is_success() {
        return Err("attested key access refused");
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(131_073)
        .read_to_end(&mut bytes)
        .map_err(|_| "key service response failed")?;
    if bytes.len() > 131_072 {
        return Err("key service response is too large");
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid key service response")
}

fn decode(text: &str, maximum: usize) -> Result<Vec<u8>> {
    if text.len() > maximum * 2 {
        return Err("sealed field is too large");
    }
    let bytes = STANDARD
        .decode(text)
        .map_err(|_| "invalid sealed encoding")?;
    if bytes.len() > maximum {
        return Err("sealed field is too large");
    }
    Ok(bytes)
}

impl GcpKms {
    pub fn connect() -> Result<Self> {
        let path = "/run/container_launcher/attestation_verifier_claims_token";
        if std::fs::metadata(path)
            .map_err(|_| "no Confidential Space identity")?
            .len()
            > 65_536
        {
            return Err("invalid Confidential Space identity");
        }
        let identity = Zeroizing::new(
            std::fs::read_to_string(path).map_err(|_| "no Confidential Space identity")?,
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "key transport initialization failed")?;
        let value = response(
            client
                .post("https://sts.googleapis.com/v1/token")
                .json(&json!({
                    "audience":PROVIDER,
                    "grantType":"urn:ietf:params:oauth:grant-type:token-exchange",
                    "requestedTokenType":"urn:ietf:params:oauth:token-type:access_token",
                    "scope":"https://www.googleapis.com/auth/cloud-platform",
                    "subjectTokenType":"urn:ietf:params:oauth:token-type:jwt",
                    "subjectToken":identity.trim(),
                }))
                .send()
                .map_err(|_| "workload identity exchange failed")?,
        )?;
        let access = Zeroizing::new(
            value["access_token"]
                .as_str()
                .ok_or("no federated identity")?
                .to_owned(),
        );
        Ok(Self { client, access })
    }

    fn wrap(&self, key: &[u8], aad: &[u8]) -> Result<String> {
        let result = response(self.client.post(format!("https://cloudkms.googleapis.com/v1/{KMS_KEY}:encrypt"))
            .bearer_auth(self.access.as_str()).json(&json!({
                "plaintext":STANDARD.encode(key),"additionalAuthenticatedData":STANDARD.encode(aad),
                "plaintextCrc32c":CRC.checksum(key).to_string(),"additionalAuthenticatedDataCrc32c":CRC.checksum(aad).to_string(),
            })).send().map_err(|_| "envelope wrapping failed")?)?;
        if result["verifiedPlaintextCrc32c"] != true
            || result["verifiedAdditionalAuthenticatedDataCrc32c"] != true
            || !result["name"]
                .as_str()
                .is_some_and(|n| n.starts_with(&format!("{KMS_KEY}/cryptoKeyVersions/")))
        {
            return Err("envelope integrity check failed");
        }
        let encoded = result["ciphertext"].as_str().ok_or("no wrapped envelope")?;
        let bytes = decode(encoded, 4096)?;
        if result["ciphertextCrc32c"].as_str() != Some(CRC.checksum(&bytes).to_string().as_str()) {
            return Err("envelope integrity check failed");
        }
        Ok(encoded.to_owned())
    }

    fn unwrap(&self, wrapped: &str, aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let bytes = decode(wrapped, 4096)?;
        let result = response(self.client.post(format!("https://cloudkms.googleapis.com/v1/{KMS_KEY}:decrypt"))
            .bearer_auth(self.access.as_str()).json(&json!({
                "ciphertext":wrapped,"additionalAuthenticatedData":STANDARD.encode(aad),
                "ciphertextCrc32c":CRC.checksum(&bytes).to_string(),"additionalAuthenticatedDataCrc32c":CRC.checksum(aad).to_string(),
            })).send().map_err(|_| "envelope release failed")?)?;
        let plain = Zeroizing::new(decode(
            result["plaintext"].as_str().ok_or("no released envelope")?,
            32,
        )?);
        if plain.len() != 32
            || result["plaintextCrc32c"].as_str() != Some(CRC.checksum(&plain).to_string().as_str())
        {
            return Err("released envelope integrity check failed");
        }
        Ok(plain)
    }
}

fn aad(binding: &[u8; 64], radix: u32) -> Vec<u8> {
    let mut bytes = SCHEMA.as_bytes().to_vec();
    bytes.push(0);
    bytes.extend_from_slice(binding);
    bytes.extend_from_slice(&radix.to_le_bytes());
    bytes
}

impl Enclave {
    /// Encrypt the private key locally; only its random envelope key reaches KMS.
    pub fn seal_for_restart(&self, kms: &GcpKms) -> Result<SealedKey> {
        if self.pending.is_some() || self.next_unroll.is_some() {
            return Err("finish signing before sealing");
        }
        let mut key = Zeroizing::new([0u8; 32]);
        bitcoin::secp256k1::rand::thread_rng().fill_bytes(key.as_mut());
        let mut sealed = self.seal_with_envelope(&key)?;
        sealed.wrapped_envelope_key =
            kms.wrap(key.as_ref(), &aad(&report_data(&self.params), sealed.radix))?;
        Ok(sealed)
    }

    fn seal_with_envelope(&self, key: &[u8; 32]) -> Result<SealedKey> {
        let binding = report_data(&self.params);
        let radix = u32::try_from(self.radix).map_err(|_| "invalid unroll radix")?;
        let mut nonce = [0u8; 12];
        bitcoin::secp256k1::rand::thread_rng().fill_bytes(&mut nonce);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid envelope key")?;
        let secret = Zeroizing::new(self.key.secret_bytes());
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: secret.as_ref(),
                    aad: &aad(&binding, radix),
                },
            )
            .map_err(|_| "local key sealing failed")?;
        Ok(SealedKey {
            v: SCHEMA.into(),
            public_key: self.params.enclave_key.to_string(),
            radix,
            binding: STANDARD.encode(binding),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
            wrapped_envelope_key: String::new(),
        })
    }

    /// Recover only the key and configuration. The service must reload chain
    /// state, mirrors, exit requests, and HALT before it can accept requests.
    pub fn restore_in_guest(cfg: Config, sealed: &SealedKey, kms: &GcpKms) -> Result<Self> {
        let binding = Self::sealed_binding(&cfg, sealed)?;
        let envelope = kms.unwrap(&sealed.wrapped_envelope_key, &aad(&binding, sealed.radix))?;
        Self::restore_with_envelope(cfg, sealed, envelope.as_slice())
    }

    fn sealed_binding(cfg: &Config, sealed: &SealedKey) -> Result<[u8; 64]> {
        if sealed.radix as usize != cfg.radix || !(2..=128).contains(&sealed.radix) {
            return Err("sealed unroll radix differs");
        }
        if sealed.v != SCHEMA {
            return Err("unknown sealed key format");
        }
        let public = sealed
            .public_key
            .parse::<XOnlyPublicKey>()
            .map_err(|_| "invalid sealed public key")?;
        let params = crate::rules::TreeParams {
            enclave_key: public,
            operator_key: cfg.operator_key,
            tree_id: cfg.tree_id,
            depth: cfg.depth,
            dust_min: cfg.dust_min,
            unroll_delay: cfg.unroll_delay,
            migration_m: cfg.migration_m,
        };
        let binding = report_data(&params);
        if decode(&sealed.binding, 64)? != binding {
            return Err("sealed tree parameters differ");
        }
        Ok(binding)
    }

    fn restore_with_envelope(cfg: Config, sealed: &SealedKey, key: &[u8]) -> Result<Self> {
        let binding = Self::sealed_binding(&cfg, sealed)?;
        let nonce = decode(&sealed.nonce, 12)?;
        if nonce.len() != 12 {
            return Err("invalid sealed nonce");
        }
        let bytes = decode(&sealed.ciphertext, 48)?;
        if bytes.len() != 48 {
            return Err("invalid sealed key length");
        }
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid envelope key")?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &bytes,
                        aad: &aad(&binding, sealed.radix),
                    },
                )
                .map_err(|_| "sealed key authentication failed")?,
        );
        let key = SecretKey::from_slice(&plain).map_err(|_| "invalid recovered key")?;
        let enclave = Self::from_guest_key(cfg, key);
        if enclave.params.enclave_key.to_string() != sealed.public_key {
            return Err("recovered public key differs");
        }
        Ok(enclave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            operator_key: [2; 32],
            tree_id: [3; 32],
            depth: 8,
            dust_min: 330,
            unroll_delay: 144,
            migration_m: None,
            radix: 2,
        }
    }
    #[test]
    fn key_round_trip_preserves_q_and_refuses_changes() {
        let enclave = Enclave::launch_in_guest(config());
        let sealed = enclave.seal_with_envelope(&[7; 32]).unwrap();
        let restored = Enclave::restore_with_envelope(config(), &sealed, &[7; 32]).unwrap();
        assert_eq!(enclave.params(), restored.params());
        assert_eq!(restored.ephemeral_keys_held(), 0);
        assert!(Enclave::restore_with_envelope(config(), &sealed, &[8; 32]).is_err());
        let mut changed = Vec::new();
        let mut cfg = config();
        cfg.operator_key[0] ^= 1;
        changed.push(cfg);
        let mut cfg = config();
        cfg.tree_id[0] ^= 1;
        changed.push(cfg);
        let mut cfg = config();
        cfg.depth += 1;
        changed.push(cfg);
        let mut cfg = config();
        cfg.dust_min += 1;
        changed.push(cfg);
        let mut cfg = config();
        cfg.unroll_delay += 1;
        changed.push(cfg);
        let mut cfg = config();
        cfg.migration_m = Some([4; 32]);
        changed.push(cfg);
        let mut cfg = config();
        cfg.radix = 4;
        changed.push(cfg);
        for cfg in changed {
            assert!(Enclave::restore_with_envelope(cfg, &sealed, &[7; 32]).is_err());
        }
        let mut bad = sealed.clone();
        let mut bytes = STANDARD.decode(&bad.ciphertext).unwrap();
        bytes[0] ^= 1;
        bad.ciphertext = STANDARD.encode(bytes);
        assert!(Enclave::restore_with_envelope(config(), &bad, &[7; 32]).is_err());
    }
}
