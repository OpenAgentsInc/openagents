//! The 24-word recovery code (NIP-VAULT, method `recovery`).
//!
//! The code is 256 random bits written as 24 BIP-39 English words with
//! their checksum. Its method secret is
//! `scrypt(NFKC(words), salt, N=2^log_n, r=8, p=1, 32)`, where `words` is
//! the 24 words in lowercase, separated by single spaces.

use serde_json::{Map, Value, json};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::{Error, Result, b64, random, unb64_n};

/// The default scrypt work factor: 2^17 (128 MiB, about a second in a
/// browser). The code carries 256 bits, so stretching guards only against
/// a weak random source; it can stay small enough for a phone's browser.
pub const LOG_N: u8 = 17;
/// The least work factor a client accepts.
pub const MIN_LOG_N: u8 = 16;

/// A recovery code. Never stored by the service; shown once.
pub struct Code(Zeroizing<String>);

impl Code {
    /// A new code from 32 random bytes.
    pub fn generate() -> Result<Self> {
        Self::from_entropy(&random::<32>()?)
    }

    /// The code for these 32 bytes (test vectors).
    pub fn from_entropy(entropy: &[u8; 32]) -> Result<Self> {
        let mnemonic = bip39::Mnemonic::from_entropy(entropy)
            .map_err(|_| Error::Format("The recovery code couldn't be made."))?;
        Ok(Self(Zeroizing::new(mnemonic.to_string())))
    }

    /// A code the person typed: 24 English words with a valid checksum.
    /// Case and spacing don't matter.
    pub fn parse(typed: &str) -> Result<Self> {
        let words = Zeroizing::new(
            typed
                .nfkc()
                .collect::<String>()
                .to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        if words.split(' ').count() != 24 {
            return Err(Error::Refused("A recovery code is 24 words."));
        }
        bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &words)
            .map_err(|_| Error::Refused("Those words aren't a recovery code. Check each word."))?;
        Ok(Self(words))
    }

    /// The 24 words, separated by single spaces.
    pub fn words(&self) -> &str {
        &self.0
    }

    /// The method secret for a slot with these parameters.
    pub fn secret(&self, log_n: u8, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>> {
        if !(MIN_LOG_N..=22).contains(&log_n) {
            return Err(Error::Refused(
                "The recovery slot's work factor is out of range.",
            ));
        }
        let params = scrypt::Params::new(log_n, 8, 1, 32)
            .map_err(|_| Error::Refused("The recovery slot's work factor is out of range."))?;
        let normalized = Zeroizing::new(self.0.nfkc().collect::<String>());
        let mut out = Zeroizing::new([0u8; 32]);
        scrypt::scrypt(normalized.as_bytes(), salt, &params, out.as_mut())
            .map_err(|_| Error::Refused("The recovery code couldn't be stretched."))?;
        Ok(out)
    }

    /// The method secret for a slot's parameters (`log_n`, `salt`).
    pub fn secret_for(&self, params: &Map<String, Value>) -> Result<Zeroizing<[u8; 32]>> {
        let log_n = params
            .get("log_n")
            .and_then(Value::as_u64)
            .and_then(|n| u8::try_from(n).ok())
            .ok_or(Error::Format("The recovery slot has no work factor."))?;
        let salt = unb64_n::<16>(
            params
                .get("salt")
                .and_then(Value::as_str)
                .ok_or(Error::Format("The recovery slot has no salt."))?,
        )?;
        self.secret(log_n, &salt)
    }
}

/// Parameters for a new recovery slot: the default work factor and a
/// random salt.
pub fn params() -> Result<Map<String, Value>> {
    params_with(LOG_N, &random::<16>()?)
}

/// Recovery slot parameters with both values given (test vectors).
pub fn params_with(log_n: u8, salt: &[u8; 16]) -> Result<Map<String, Value>> {
    match json!({ "log_n": log_n, "salt": b64(salt) }) {
        Value::Object(map) => Ok(map),
        _ => Err(Error::Format("The recovery slot can't be written.")),
    }
}
