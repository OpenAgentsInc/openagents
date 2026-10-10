//! The browser vault (#11240): [`oa_vault`] behind a session that keeps the
//! vault master key inside this module. The page's script (served as
//! `/vault/vault.js`) does the network, WebAuthn and Nostr calls and hands
//! this module only method secrets, ciphertext and the bytes of files the
//! person picked. What comes back out is slots, sealed objects, a sealed
//! index, the file list, and a file the person asked to open. The master
//! key never does.
//!
//! [`Session`] is plain Rust, tested natively; `wasm` wraps it for the page.

use base64::Engine;
use oa_vault::index::{Add, Index, Kind, Route};
use oa_vault::recovery::{self, Code};
use oa_vault::slot::{self, Draft, Method, Slot};
use oa_vault::{Error, Vmk, hex, unhex};
use serde::Serialize;
use zeroize::Zeroizing;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub type Result<T> = std::result::Result<T, Error>;

fn url64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn unurl64(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.trim_end_matches('='))
        .map_err(|_| Error::Format("That link is incomplete. Copy all of it."))
}

fn parse_slot(json: &str, vault: &str) -> Result<Slot> {
    let slot: Slot =
        serde_json::from_str(json).map_err(|_| Error::Format("A key slot is invalid."))?;
    slot.check()?;
    if slot.vault != vault {
        return Err(Error::Refused("That key belongs to another vault."));
    }
    Ok(slot)
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| Error::Format("A value can't be written."))
}

/// An unlocked vault on this page.
pub struct Session {
    vmk: Vmk,
    vault: String,
    index: Option<Index>,
    pending: Option<Index>,
}

/// One row of the file list the page shows.
#[derive(Serialize)]
pub struct Row<'a> {
    pub object: &'a str,
    pub kind: Kind,
    pub name: &'a str,
    pub media: Option<&'a str>,
    pub size: u64,
    pub project: Option<&'a str>,
    pub about: &'a [String],
    pub route: Option<Route>,
    pub created_at: u64,
}

impl Session {
    /// A new vault: a fresh master key and id, and an empty index at epoch 1.
    pub fn create() -> Result<Self> {
        let vault = oa_vault::new_id()?;
        let index = Index::new(&vault)?;
        Ok(Self {
            vmk: Vmk::generate()?,
            vault,
            index: Some(index),
            pending: None,
        })
    }

    /// Unlock with a slot's method secret.
    pub fn unlock(vault: &str, slot_json: &str, secret: &[u8]) -> Result<Self> {
        let slot = parse_slot(slot_json, vault)?;
        Ok(Self {
            vmk: slot.open(secret)?,
            vault: vault.to_owned(),
            index: None,
            pending: None,
        })
    }

    /// Unlock with the 24-word recovery code.
    pub fn unlock_recovery(vault: &str, slot_json: &str, words: &str) -> Result<Self> {
        let slot = parse_slot(slot_json, vault)?;
        if slot.method != Method::Recovery {
            return Err(Error::Refused("That isn't the recovery code slot."));
        }
        let code = Code::parse(words)?;
        let secret = code.secret_for(&slot.params)?;
        Self::unlock(vault, slot_json, secret.as_ref())
    }

    /// Unlock with the secret a Nostr signer decrypted (64 hex characters).
    pub fn unlock_nostr(vault: &str, slot_json: &str, secret_hex: &str) -> Result<Self> {
        let secret = Zeroizing::new(unhex::<32>(secret_hex.trim())?);
        Self::unlock(vault, slot_json, secret.as_ref())
    }

    /// Unlock with a pairing link's fragment, `<slot id>.<secret>`. Returns
    /// the slot id too, so the page can delete the slot once this device has
    /// its own.
    pub fn unlock_pairing(vault: &str, slots_json: &str, fragment: &str) -> Result<(Self, String)> {
        let (id, secret) = fragment
            .split_once('.')
            .ok_or(Error::Format("That link is incomplete. Copy all of it."))?;
        let slots: Vec<serde_json::Value> = serde_json::from_str(slots_json)
            .map_err(|_| Error::Format("The vault's keys are invalid."))?;
        let slot = slots
            .iter()
            .find(|slot| slot["slot"] == id)
            .ok_or(Error::Refused("That link has expired or was already used."))?;
        let secret = Zeroizing::new(unurl64(secret)?);
        let session = Self::unlock(vault, &slot.to_string(), &secret)?;
        Ok((session, id.to_owned()))
    }

    pub fn vault(&self) -> &str {
        &self.vault
    }

    pub fn epoch(&self) -> u32 {
        self.index.as_ref().map_or(0, |index| index.epoch)
    }

    /// Open the index the service sent, refusing one older than `min_epoch`
    /// (the newest this browser has seen).
    pub fn load_index(&mut self, blob: &[u8], min_epoch: u32) -> Result<()> {
        if Index::epoch_of(blob)? < min_epoch {
            return Err(Error::Refused(
                "This vault's file list is older than one this browser has seen. Reload the page.",
            ));
        }
        self.index = Some(Index::open(&self.vmk, &self.vault, blob)?);
        self.pending = None;
        Ok(())
    }

    fn index(&self) -> Result<&Index> {
        self.index
            .as_ref()
            .ok_or(Error::Refused("Unlock your vault first."))
    }

    /// The current index, sealed.
    pub fn index_blob(&self) -> Result<Vec<u8>> {
        self.index()?.seal(&self.vmk)
    }

    /// The pending index (after [`Session::add`] or [`Session::remove`]), sealed.
    pub fn pending_blob(&self) -> Result<Vec<u8>> {
        self.pending
            .as_ref()
            .ok_or(Error::Refused("Nothing is waiting to be saved."))?
            .seal(&self.vmk)
    }

    /// The service took the pending index: it is now current.
    pub fn commit(&mut self) {
        if let Some(next) = self.pending.take() {
            self.index = Some(next);
        }
    }

    pub fn discard(&mut self) {
        self.pending = None;
    }

    /// The file list as JSON rows, newest first, limited to `project` when
    /// one is given.
    pub fn rows(&self, project: Option<&str>) -> Result<String> {
        let index = self.index()?;
        let mut rows: Vec<Row<'_>> = index
            .entries
            .iter()
            .filter(|entry| project.is_none_or(|p| entry.project.as_deref() == Some(p)))
            .map(|entry| Row {
                object: &entry.object,
                kind: entry.kind,
                name: &entry.name,
                media: entry.media.as_deref(),
                size: entry.size,
                project: entry.project.as_deref(),
                about: &entry.about,
                route: entry.route,
                created_at: entry.created_at,
            })
            .collect();
        rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        to_json(&rows)
    }

    /// Seal a file (or an answer) and stage the next index. Returns the new
    /// object's id and bytes.
    pub fn add(&mut self, add: Add<'_>, plain: &[u8]) -> Result<(String, Vec<u8>)> {
        let base = self.pending.as_ref().map_or_else(|| self.index(), Ok)?;
        let (bytes, next) = base.add(&self.vmk, add, plain)?;
        let id = next
            .entries
            .last()
            .map(|entry| entry.object.clone())
            .ok_or(Error::Format("The new file wasn't listed."))?;
        self.pending = Some(next);
        Ok((id, bytes))
    }

    /// Stage the next index without `object`.
    pub fn remove(&mut self, object: &str) -> Result<()> {
        self.pending = Some(self.index()?.remove(object)?);
        Ok(())
    }

    /// Open a stored object listed in the index.
    pub fn open(&self, object: &str, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        self.index()?.open_object(&self.vmk, object, bytes)
    }

    fn slot(
        &self,
        method: Method,
        params: serde_json::Map<String, serde_json::Value>,
        label: &str,
        now: u64,
        secret: &[u8],
    ) -> Result<Slot> {
        let label = label.trim();
        let label = if label.is_empty() {
            "This device"
        } else {
            label
        };
        let label: String = label
            .chars()
            .filter(|c| !c.is_control())
            .take(slot::MAX_LABEL)
            .collect();
        Slot::seal(
            Draft::new(&self.vault, method, params, &label, now)?,
            secret,
            &self.vmk,
        )
    }

    /// A new recovery code and its slot: `{"words", "slot"}`. The words are
    /// shown once and never sent.
    pub fn recovery_slot(&self, now: u64) -> Result<String> {
        let code = Code::generate()?;
        let params = recovery::params()?;
        let secret = code.secret_for(&params)?;
        let slot = self.slot(
            Method::Recovery,
            params,
            "Recovery code",
            now,
            secret.as_ref(),
        )?;
        to_json(&serde_json::json!({ "words": code.words(), "slot": slot }))
    }

    /// A passkey slot from the passkey's PRF output.
    pub fn passkey_slot(
        &self,
        label: &str,
        now: u64,
        rp_id: &str,
        credential_id: &[u8],
        prf_salt: &[u8],
        prf_output: &[u8],
    ) -> Result<String> {
        let salt: [u8; 32] = prf_salt
            .try_into()
            .map_err(|_| Error::Format("The passkey salt has the wrong length."))?;
        if prf_output.len() != 32 {
            return Err(Error::Refused(
                "This passkey didn't give a key. Try another way.",
            ));
        }
        let params = slot::passkey_params(rp_id, credential_id, &salt);
        to_json(&self.slot(Method::PasskeyPrf, params, label, now, prf_output)?)
    }

    /// A Nostr slot: `secret_hex` is what the page sealed to the person's
    /// key as `sealed`.
    pub fn nostr_slot(
        &self,
        label: &str,
        now: u64,
        pubkey: &str,
        sealed: &str,
        secret_hex: &str,
    ) -> Result<String> {
        let secret = Zeroizing::new(unhex::<32>(secret_hex)?);
        let params = slot::nostr_params(pubkey, sealed);
        to_json(&self.slot(Method::Nostr, params, label, now, secret.as_ref())?)
    }

    /// A pairing slot for a new device: `{"slot", "fragment"}`, where the
    /// fragment `<slot id>.<secret>` goes after `#pair=` in the link.
    pub fn pairing_slot(&self, now: u64, lifetime: u64) -> Result<String> {
        let secret = Zeroizing::new(oa_vault::random::<32>()?);
        let lifetime = lifetime.clamp(60, slot::MAX_PAIRING_SECS);
        let params = slot::pairing_params(now + lifetime);
        let slot = self.slot(
            Method::Pairing,
            params,
            "Pairing link",
            now,
            secret.as_ref(),
        )?;
        let fragment = format!("{}.{}", slot.slot, url64(secret.as_ref()));
        to_json(&serde_json::json!({ "slot": slot, "fragment": fragment }))
    }
}

/// 32 random bytes as hex, for a Nostr slot's secret.
pub fn random_hex() -> Result<String> {
    Ok(hex(&oa_vault::random::<32>()?))
}

/// Whether the slots may hold data (a recovery code and one more).
pub fn enough(slots_json: &str) -> bool {
    serde_json::from_str::<Vec<Slot>>(slots_json).is_ok_and(|slots| slot::enough(&slots))
}

/// A QR code of `text` as an SVG path over a square of `size` modules:
/// `{"size", "path"}`. The page draws it without HTML parsing.
pub fn qr(text: &str) -> Result<String> {
    let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium)
        .map_err(|_| Error::Refused("That link is too long for a QR code."))?;
    let size = code.size();
    let mut path = String::new();
    for y in 0..size {
        for x in 0..size {
            if code.get_module(x, y) {
                path.push_str(&format!("M{x} {y}h1v1h-1z"));
            }
        }
    }
    to_json(&serde_json::json!({ "size": size, "path": path }))
}

/// The plain words of an error, for the page.
pub fn words(error: &Error) -> String {
    error.to_string()
}

pub fn kind(name: &str) -> Kind {
    if name == "answer" {
        Kind::Answer
    } else {
        Kind::File
    }
}

pub fn route(name: &str) -> Option<Route> {
    match name {
        "device" => Some(Route::Device),
        "fast" => Some(Route::Fast),
        _ => None,
    }
}

pub fn about(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|id| oa_vault::valid_id(id))
        .collect()
}

#[cfg(test)]
mod tests;
