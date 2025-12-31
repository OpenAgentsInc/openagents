//! Identity decryption for wallet worker.
//!
//! Minimal copy of the main worker's identity module - only includes
//! what's needed to decrypt user wallet seeds.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use worker::{Error, Result};

const ENCRYPTION_VERSION: &str = "v1";
const MASTER_KEY_DOMAIN: &[u8] = b"openagents-web:identity:master:v1";

pub struct IdentityMaterial {
    #[allow(dead_code)]
    pub nostr_private_key: [u8; 32],
    pub bitcoin_xpriv: String,
}

pub fn decrypt_identity(
    session_secret: &str,
    nostr_private_key_encrypted: &str,
    bitcoin_xpriv_encrypted: &str,
) -> Result<IdentityMaterial> {
    let master_key = derive_master_key(session_secret);
    let nostr_private_key_bytes = decrypt_with_key(&master_key, nostr_private_key_encrypted)?;
    let bitcoin_xpriv_bytes = decrypt_with_key(&master_key, bitcoin_xpriv_encrypted)?;

    if nostr_private_key_bytes.len() != 32 {
        return Err(Error::RustError(format!(
            "Invalid nostr private key length: {}",
            nostr_private_key_bytes.len()
        )));
    }

    let mut nostr_private_key = [0u8; 32];
    nostr_private_key.copy_from_slice(&nostr_private_key_bytes);

    let bitcoin_xpriv = String::from_utf8(bitcoin_xpriv_bytes)
        .map_err(|e| Error::RustError(format!("Invalid xpriv encoding: {}", e)))?;

    Ok(IdentityMaterial {
        nostr_private_key,
        bitcoin_xpriv,
    })
}

fn derive_master_key(session_secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MASTER_KEY_DOMAIN);
    hasher.update(session_secret.as_bytes());
    let digest = hasher.finalize();

    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

fn decrypt_with_key(key: &[u8; 32], encoded: &str) -> Result<Vec<u8>> {
    let mut parts = encoded.splitn(3, ':');
    let version = parts.next().unwrap_or_default();
    if version != ENCRYPTION_VERSION {
        return Err(Error::RustError(format!(
            "Unsupported encryption version: {}",
            version
        )));
    }

    let nonce_b64 = parts
        .next()
        .ok_or_else(|| Error::RustError("Missing nonce".to_string()))?;
    let ciphertext_b64 = parts
        .next()
        .ok_or_else(|| Error::RustError("Missing ciphertext".to_string()))?;

    let nonce_bytes = URL_SAFE_NO_PAD
        .decode(nonce_b64)
        .map_err(|e| Error::RustError(format!("Invalid nonce encoding: {}", e)))?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(ciphertext_b64)
        .map_err(|e| Error::RustError(format!("Invalid ciphertext encoding: {}", e)))?;

    if nonce_bytes.len() != 12 {
        return Err(Error::RustError(format!(
            "Invalid nonce length: {}",
            nonce_bytes.len()
        )));
    }

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| Error::RustError(format!("Decrypt failed: {}", e)))
}
