//! Web identity and credential encryption helpers.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use bech32::{Bech32, Hrp};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use rand::RngCore;
use sha2::{Digest, Sha256};
use worker::{Error, Result};

const ENCRYPTION_VERSION: &str = "v1";
const ENCRYPTED_PREFIX: &str = "v1:";
const MASTER_KEY_DOMAIN: &[u8] = b"openagents-web:identity:master:v1";
const CREDENTIAL_KEY_DOMAIN: &[u8] = b"openagents-web:credentials:v1";

pub struct StoredIdentity {
    pub nostr_public_key: String,
    pub nostr_npub: String,
    pub nostr_private_key_encrypted: String,
    pub bitcoin_xpriv_encrypted: String,
}

pub struct IdentityMaterial {
    pub nostr_private_key: [u8; 32],
    pub bitcoin_xpriv: String,
}

struct GeneratedIdentity {
    nostr_private_key: [u8; 32],
    nostr_public_key: [u8; 32],
    nostr_npub: String,
    bitcoin_xpriv: String,
}

pub fn generate_identity_bundle(session_secret: &str) -> Result<(StoredIdentity, IdentityMaterial)> {
    let generated = generate_identity()?;
    let master_key = derive_master_key(session_secret);

    let nostr_private_key_encrypted = encrypt_with_key(&master_key, &generated.nostr_private_key)?;
    let bitcoin_xpriv_encrypted =
        encrypt_with_key(&master_key, generated.bitcoin_xpriv.as_bytes())?;

    let stored = StoredIdentity {
        nostr_public_key: hex::encode(generated.nostr_public_key),
        nostr_npub: generated.nostr_npub.clone(),
        nostr_private_key_encrypted,
        bitcoin_xpriv_encrypted,
    };

    let material = IdentityMaterial {
        nostr_private_key: generated.nostr_private_key,
        bitcoin_xpriv: generated.bitcoin_xpriv,
    };

    Ok((stored, material))
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

pub fn derive_credentials_key(identity: &IdentityMaterial) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CREDENTIAL_KEY_DOMAIN);
    hasher.update(identity.nostr_private_key);
    hasher.update(identity.bitcoin_xpriv.as_bytes());
    let digest = hasher.finalize();

    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

pub fn nostr_public_key_from_private(private_key: &[u8; 32]) -> Result<[u8; 32]> {
    let secret_key = SecretKey::from_slice(private_key)
        .map_err(|e| Error::RustError(format!("Invalid nostr private key: {}", e)))?;
    let public_key = secret_key.public_key();
    let public_key_bytes = public_key.to_encoded_point(true);
    let public_key_bytes = public_key_bytes.as_bytes();
    if public_key_bytes.len() != 33 {
        return Err(Error::RustError("Invalid public key encoding".to_string()));
    }
    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&public_key_bytes[1..33]);
    Ok(public_key)
}

pub fn nostr_npub_from_private(private_key: &[u8; 32]) -> Result<String> {
    let public_key = nostr_public_key_from_private(private_key)?;
    encode_npub(&public_key)
}

pub fn encrypt_with_key(key: &[u8; 32], plaintext: &[u8]) -> Result<String> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| Error::RustError(format!("Encrypt failed: {}", e)))?;

    let nonce_b64 = URL_SAFE_NO_PAD.encode(nonce_bytes);
    let ciphertext_b64 = URL_SAFE_NO_PAD.encode(ciphertext);

    Ok(format!(
        "{}:{}:{}",
        ENCRYPTION_VERSION, nonce_b64, ciphertext_b64
    ))
}

pub fn decrypt_with_key(key: &[u8; 32], encoded: &str) -> Result<Vec<u8>> {
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

pub fn decrypt_optional_with_key(key: &[u8; 32], encoded: &str) -> Result<Vec<u8>> {
    if encoded.starts_with(ENCRYPTED_PREFIX) {
        decrypt_with_key(key, encoded)
    } else {
        Ok(encoded.as_bytes().to_vec())
    }
}

fn generate_identity() -> Result<GeneratedIdentity> {
    let mut rng = rand::rngs::OsRng;

    let nostr_secret = SecretKey::random(&mut rng);
    let nostr_public = nostr_secret.public_key();
    let nostr_public_encoded = nostr_public.to_encoded_point(true);
    let nostr_public_bytes = nostr_public_encoded.as_bytes();
    if nostr_public_bytes.len() != 33 {
        return Err(Error::RustError("Invalid Nostr public key encoding".to_string()));
    }

    let mut nostr_public_key = [0u8; 32];
    nostr_public_key.copy_from_slice(&nostr_public_bytes[1..33]);

    let nostr_private_bytes = nostr_secret.to_bytes();
    let mut nostr_private_key = [0u8; 32];
    nostr_private_key.copy_from_slice(nostr_private_bytes.as_slice());

    let nostr_npub = encode_npub(&nostr_public_key)?;

    let bitcoin_secret = SecretKey::random(&mut rng);
    let bitcoin_secret_bytes = bitcoin_secret.to_bytes();
    let bitcoin_xpriv = hex::encode(bitcoin_secret_bytes.as_slice());

    Ok(GeneratedIdentity {
        nostr_private_key,
        nostr_public_key,
        nostr_npub,
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

fn encode_npub(public_key: &[u8; 32]) -> Result<String> {
    let hrp = Hrp::parse("npub")
        .map_err(|e| Error::RustError(format!("Invalid npub HRP: {}", e)))?;
    bech32::encode::<Bech32>(hrp, public_key)
        .map_err(|e| Error::RustError(format!("Failed to encode npub: {}", e)))
}
