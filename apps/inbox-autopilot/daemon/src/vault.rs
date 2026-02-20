use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const KEYCHAIN_SERVICE: &str = "com.openagents.inbox-autopilot.daemon";
const KEYCHAIN_ACCOUNT: &str = "master-key-v1";

#[derive(Clone)]
pub struct Vault {
    cipher: Aes256Gcm,
}

impl Vault {
    pub fn load_or_create(data_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create data dir {}", data_dir.display()))?;

        let mut key_path = data_dir;
        key_path.push("master.key");

        let key_material = if let Some(bytes) = Self::load_keychain_key()? {
            bytes
        } else if key_path.exists() {
            let bytes = fs::read(&key_path)
                .with_context(|| format!("failed to read key {}", key_path.display()))?;
            let _ = Self::store_keychain_key(&bytes);
            bytes
        } else {
            let mut bytes = vec![0_u8; 64];
            rand::rng().fill_bytes(&mut bytes);

            if Self::store_keychain_key(&bytes).is_err() {
                let mut file = fs::File::create(&key_path)
                    .with_context(|| format!("failed to create key {}", key_path.display()))?;
                file.write_all(&bytes)
                    .with_context(|| format!("failed to write key {}", key_path.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perm = fs::Permissions::from_mode(0o600);
                    fs::set_permissions(&key_path, perm).with_context(|| {
                        format!("failed to set permissions on {}", key_path.display())
                    })?;
                }
            }

            bytes
        };

        let digest = Sha256::digest(&key_material);
        let cipher = Aes256Gcm::new_from_slice(&digest)
            .context("failed to initialize cipher from key material")?;

        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let mut nonce_bytes = [0_u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("failed to encrypt secret"))?;
        let mut out = nonce_bytes.to_vec();
        out.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(out))
    }

    pub fn decrypt(&self, encoded: &str) -> Result<String> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .context("failed to decode secret")?;
        if bytes.len() < 12 {
            anyhow::bail!("ciphertext too short");
        }
        let (nonce_bytes, ciphertext) = bytes.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("failed to decrypt secret"))?;
        String::from_utf8(plaintext).context("decrypted secret not valid utf8")
    }

    fn load_keychain_key() -> Result<Option<Vec<u8>>> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .context("failed to access keychain entry")?;

        match entry.get_password() {
            Ok(encoded) => {
                let bytes = URL_SAFE_NO_PAD
                    .decode(encoded.as_bytes())
                    .context("failed to decode keychain key")?;
                Ok(Some(bytes))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(anyhow::anyhow!(err)).context("failed reading keychain key"),
        }
    }

    fn store_keychain_key(bytes: &[u8]) -> Result<()> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .context("failed to access keychain entry")?;
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        entry
            .set_password(&encoded)
            .map_err(|err| anyhow::anyhow!(err))
            .context("failed storing keychain key")
    }
}
