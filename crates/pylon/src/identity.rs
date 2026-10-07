//! Keys a pylon or a buyer holds: one secp256k1 secret per role, stored as
//! 64 hex characters in a `0600` file that the process creates on first use.

use std::fs;
use std::io::Write;
use std::path::Path;

use nostr::domain::RelaySigner;
use nostr::nip19;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

/// A loaded key and its signer.
#[derive(Clone)]
pub struct Identity {
    secret: SecretKey,
    signer: RelaySigner,
}

impl Identity {
    /// The identity for `secret`.
    ///
    /// # Errors
    ///
    /// When the secret is not a valid key.
    pub fn from_secret(secret: SecretKey) -> Result<Self, String> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        Ok(Self { secret, signer })
    }

    /// A fresh random identity.
    #[must_use]
    pub fn generate() -> Self {
        let keypair = Keypair::new(&Secp256k1::new(), &mut secp256k1::rand::rng());
        Self::from_secret(keypair.secret_key()).unwrap_or_else(|_| unreachable!())
    }

    /// Load the key in `path`, creating it with mode `0600` when absent.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or written, or holds no valid key.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let text = text.trim();
                let bytes = if text.starts_with("nsec1") {
                    nip19::decode_nsec(text).map_err(|e| e.to_string())?
                } else {
                    let mut out = [0_u8; 32];
                    if text.len() != 64 {
                        return Err(format!("{} does not hold a 64-hex key", path.display()));
                    }
                    for (i, byte) in out.iter_mut().enumerate() {
                        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                            .map_err(|_| format!("{} does not hold a hex key", path.display()))?;
                    }
                    out
                };
                Self::from_secret(SecretKey::from_byte_array(bytes).map_err(|e| e.to_string())?)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let identity = Self::generate();
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(path).map_err(|e| e.to_string())?;
                writeln!(file, "{}", identity.secret.display_secret())
                    .map_err(|e| e.to_string())?;
                Ok(identity)
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// The secret key, for NIP-44 conversation keys.
    #[must_use]
    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    /// The event signer.
    #[must_use]
    pub fn signer(&self) -> &RelaySigner {
        &self.signer
    }

    /// The lowercase hex public key.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }

    /// The `npub` form of the public key.
    #[must_use]
    pub fn npub(&self) -> String {
        npub(self.pubkey())
    }
}

/// The `npub` for a hex public key, or the hex itself when it does not parse.
#[must_use]
pub fn npub(hex: &str) -> String {
    parse_pubkey(hex).map_or_else(
        || hex.to_string(),
        |key| nip19::encode_npub(&key.serialize()),
    )
}

/// Parse an `npub` or 64-hex public key.
#[must_use]
pub fn parse_pubkey(text: &str) -> Option<XOnlyPublicKey> {
    let bytes = if text.starts_with("npub1") {
        nip19::decode_npub(text).ok()?
    } else {
        if text.len() != 64 {
            return None;
        }
        let mut out = [0_u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
        }
        out
    };
    XOnlyPublicKey::from_byte_array(bytes).ok()
}

/// The lowercase hex form of an `npub` or hex public key.
#[must_use]
pub fn hex_pubkey(text: &str) -> Option<String> {
    parse_pubkey(text).map(|key| key.to_string())
}
