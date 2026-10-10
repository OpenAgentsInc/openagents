//! Key slots (NIP-VAULT "Key slots"): the vault master key wrapped under
//! one unlock method. The service stores slots; it holds no method secret,
//! so it opens none of them.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use zeroize::Zeroizing;

use crate::keys::{Vmk, hkdf, open, seal};
use crate::{Error, Result, b64, jcs, random, unb64, unb64_n, unhex, valid_id};

pub const SLOT_V: &str = "openagents.vault-slot.v1";
pub const SLOT_INFO: &[u8] = b"openagents.vault.v1/slot\0";
/// The longest label.
pub const MAX_LABEL: usize = 64;
/// A pairing slot lives at most this long.
pub const MAX_PAIRING_SECS: u64 = 15 * 60;

/// How a slot is unlocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    /// A WebAuthn passkey's PRF output.
    #[serde(rename = "passkey-prf")]
    PasskeyPrf,
    /// 32 random bytes in this device's keychain or keystore.
    #[serde(rename = "device")]
    Device,
    /// 32 random bytes sealed to the person's own Nostr key with NIP-44 v2.
    #[serde(rename = "nostr")]
    Nostr,
    /// The 24-word recovery code, stretched with scrypt.
    #[serde(rename = "recovery")]
    Recovery,
    /// A short-lived link from one of the person's devices to a new one.
    #[serde(rename = "pairing")]
    Pairing,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PasskeyPrf => "passkey-prf",
            Self::Device => "device",
            Self::Nostr => "nostr",
            Self::Recovery => "recovery",
            Self::Pairing => "pairing",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub vault: String,
    pub slot: String,
    pub method: Method,
    pub params: Map<String, Value>,
    /// 12 bytes, base64.
    pub nonce: String,
    /// 48 bytes, base64: AES-256-GCM of the master key and its tag.
    pub vmk: String,
    pub label: String,
    pub created_at: u64,
}

/// A slot before it is sealed: the method and its public parameters.
#[derive(Clone, Debug)]
pub struct Draft {
    pub vault: String,
    pub slot: String,
    pub method: Method,
    pub params: Map<String, Value>,
    pub label: String,
    pub created_at: u64,
}

impl Draft {
    /// A draft with a fresh slot id.
    pub fn new(
        vault: &str,
        method: Method,
        params: Map<String, Value>,
        label: &str,
        created_at: u64,
    ) -> Result<Self> {
        Ok(Self {
            vault: vault.to_owned(),
            slot: crate::new_id()?,
            method,
            params,
            label: label.to_owned(),
            created_at,
        })
    }
}

impl Slot {
    /// The associated data: `JCS(slot without nonce and vmk)`.
    fn aad(&self) -> Result<Vec<u8>> {
        let mut value =
            serde_json::to_value(self).map_err(|_| Error::Format("The slot can't be written."))?;
        if let Value::Object(map) = &mut value {
            map.remove("nonce");
            map.remove("vmk");
        }
        jcs::to_vec(&value)
    }

    /// The wrapping key: `HKDF-SHA256(ikm=method secret, salt=the 32 bytes
    /// of the slot id, info="openagents.vault.v1/slot\0" ‖ method)`.
    fn key(&self, secret: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        let salt = unhex::<32>(&self.slot)?;
        let mut info = SLOT_INFO.to_vec();
        info.extend_from_slice(self.method.as_str().as_bytes());
        Ok(hkdf(secret, &salt, &info))
    }

    /// Check the slot's shape and the parameters of its method.
    pub fn check(&self) -> Result<()> {
        if self.v != SLOT_V {
            return Err(Error::Format("This is not a key slot of a known version."));
        }
        if !self.requires.is_empty() {
            return Err(Error::Refused(
                "The slot needs a feature this client lacks.",
            ));
        }
        if !valid_id(&self.vault) || !valid_id(&self.slot) {
            return Err(Error::Format("The slot or vault id is invalid."));
        }
        if self.label.chars().count() > MAX_LABEL || self.label.chars().any(char::is_control) {
            return Err(Error::Format("The slot's name is invalid."));
        }
        unb64_n::<12>(&self.nonce)?;
        unb64_n::<48>(&self.vmk)?;
        check_params(self.method, &self.params, self.created_at)
    }

    /// Seal `vmk` into a slot unlocked by `secret`.
    pub fn seal(draft: Draft, secret: &[u8], vmk: &Vmk) -> Result<Self> {
        Self::seal_with(draft, secret, vmk, random::<12>()?)
    }

    /// [`Slot::seal`] with the nonce given (test vectors).
    pub fn seal_with(draft: Draft, secret: &[u8], vmk: &Vmk, nonce: [u8; 12]) -> Result<Self> {
        if secret.len() < 32 {
            return Err(Error::Refused("A slot secret must be at least 32 bytes."));
        }
        let mut slot = Self {
            v: SLOT_V.to_owned(),
            requires: Vec::new(),
            meta: None,
            vault: draft.vault,
            slot: draft.slot,
            method: draft.method,
            params: draft.params,
            nonce: b64(&nonce),
            vmk: b64(&[0u8; 48]),
            label: draft.label,
            created_at: draft.created_at,
        };
        slot.check()?;
        let key = slot.key(secret)?;
        slot.vmk = b64(&seal(&key, &nonce, &slot.aad()?, vmk.bytes())?);
        Ok(slot)
    }

    /// Open the slot with its method secret.
    pub fn open(&self, secret: &[u8]) -> Result<Vmk> {
        self.check()?;
        let key = self.key(secret)?;
        let opened = open(
            &key,
            &unb64_n::<12>(&self.nonce)?,
            &self.aad()?,
            &unb64(&self.vmk)?,
            "That doesn't unlock this vault.",
        )?;
        let bytes: [u8; 32] = opened
            .as_slice()
            .try_into()
            .map_err(|_| Error::Format("The slot holds a key of the wrong length."))?;
        Ok(Vmk::from_bytes(bytes))
    }

    /// Whether a pairing slot has expired at `now`.
    pub fn expired(&self, now: u64) -> bool {
        self.method == Method::Pairing
            && self
                .params
                .get("expires_at")
                .and_then(Value::as_u64)
                .is_none_or(|at| now >= at)
    }

    /// A string parameter.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).and_then(Value::as_str)
    }
}

fn exact(params: &Map<String, Value>, names: &[&str]) -> Result<()> {
    if params.len() != names.len() || names.iter().any(|name| !params.contains_key(*name)) {
        return Err(Error::Format(
            "The slot's parameters don't match its method.",
        ));
    }
    Ok(())
}

fn text<'a>(params: &'a Map<String, Value>, name: &str, max: usize) -> Result<&'a str> {
    params
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max)
        .ok_or(Error::Format("A slot parameter is invalid."))
}

fn check_params(method: Method, params: &Map<String, Value>, created_at: u64) -> Result<()> {
    match method {
        Method::PasskeyPrf => {
            exact(params, &["rp_id", "credential_id", "prf_salt"])?;
            text(params, "rp_id", 253)?;
            let id = unb64(text(params, "credential_id", 1400)?)?;
            if id.is_empty() || id.len() > 1023 {
                return Err(Error::Format("The passkey id is invalid."));
            }
            unb64_n::<32>(text(params, "prf_salt", 64)?)?;
        }
        Method::Device => {
            exact(params, &["platform", "key_ref"])?;
            let platform = text(params, "platform", 16)?;
            if !["ios", "android", "macos", "linux", "windows"].contains(&platform) {
                return Err(Error::Format("The device platform is unknown."));
            }
            text(params, "key_ref", 128)?;
        }
        Method::Nostr => {
            exact(params, &["pubkey", "sealed"])?;
            unhex::<32>(text(params, "pubkey", 64)?)?;
            // A NIP-44 v2 payload of a 64-character hex secret.
            text(params, "sealed", 512)?;
        }
        Method::Recovery => {
            exact(params, &["log_n", "salt"])?;
            let log_n = params.get("log_n").and_then(Value::as_u64).unwrap_or(0);
            if !(crate::recovery::MIN_LOG_N as u64..=22).contains(&log_n) {
                return Err(Error::Format(
                    "The recovery slot's work factor is out of range.",
                ));
            }
            unb64_n::<16>(text(params, "salt", 32)?)?;
        }
        Method::Pairing => {
            exact(params, &["expires_at"])?;
            let at = params
                .get("expires_at")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if at <= created_at || at - created_at > MAX_PAIRING_SECS {
                return Err(Error::Format(
                    "A pairing link must expire within 15 minutes.",
                ));
            }
        }
    }
    Ok(())
}

/// Parameters for a passkey slot.
pub fn passkey_params(
    rp_id: &str,
    credential_id: &[u8],
    prf_salt: &[u8; 32],
) -> Map<String, Value> {
    object(
        json!({ "rp_id": rp_id, "credential_id": b64(credential_id), "prf_salt": b64(prf_salt) }),
    )
}

/// Parameters for a device slot.
pub fn device_params(platform: &str, key_ref: &str) -> Map<String, Value> {
    object(json!({ "platform": platform, "key_ref": key_ref }))
}

/// Parameters for a Nostr slot: the person's public key (hex) and the
/// NIP-44 v2 payload that holds the slot secret (as 64 hex characters)
/// encrypted to that same key.
pub fn nostr_params(pubkey: &str, sealed: &str) -> Map<String, Value> {
    object(json!({ "pubkey": pubkey, "sealed": sealed }))
}

/// Parameters for a pairing slot.
pub fn pairing_params(expires_at: u64) -> Map<String, Value> {
    object(json!({ "expires_at": expires_at }))
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Whether a set of slots may hold data: at least two slots that last
/// (pairing slots don't count), one of them the recovery code. A vault
/// never rests on a passkey alone.
pub fn enough(slots: &[Slot]) -> bool {
    let lasting: Vec<&Slot> = slots
        .iter()
        .filter(|s| s.method != Method::Pairing)
        .collect();
    lasting.len() >= 2 && lasting.iter().any(|s| s.method == Method::Recovery)
}
