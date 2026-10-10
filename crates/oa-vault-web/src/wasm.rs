//! The page's view of [`Session`]: `wasm_bindgen` classes and functions.
//! Errors cross as plain-word strings.

use oa_vault::index::Add;
use wasm_bindgen::prelude::*;

use crate::{Session, about, kind, route, words};

fn fail(error: oa_vault::Error) -> JsValue {
    JsValue::from_str(&words(&error))
}

fn now(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    }
}

/// An unlocked vault. The master key stays inside.
#[wasm_bindgen]
pub struct Vault(Session);

/// A sealed object waiting to be uploaded.
#[wasm_bindgen]
pub struct Sealed {
    id: String,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl Sealed {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// An unlock through a pairing link: the vault, and the pairing slot to
/// delete once this device has its own key.
#[wasm_bindgen]
pub struct Paired {
    vault: Option<Session>,
    slot: String,
}

#[wasm_bindgen]
impl Paired {
    #[wasm_bindgen(getter)]
    pub fn slot(&self) -> String {
        self.slot.clone()
    }

    /// The unlocked vault (once).
    pub fn take(&mut self) -> Option<Vault> {
        self.vault.take().map(Vault)
    }
}

#[wasm_bindgen]
impl Vault {
    /// A brand-new vault.
    pub fn create() -> Result<Vault, JsValue> {
        Session::create().map(Vault).map_err(fail)
    }

    pub fn unlock(vault: &str, slot: &str, secret: &[u8]) -> Result<Vault, JsValue> {
        Session::unlock(vault, slot, secret)
            .map(Vault)
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = unlockRecovery)]
    pub fn unlock_recovery(vault: &str, slot: &str, words: &str) -> Result<Vault, JsValue> {
        Session::unlock_recovery(vault, slot, words)
            .map(Vault)
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = unlockNostr)]
    pub fn unlock_nostr(vault: &str, slot: &str, secret_hex: &str) -> Result<Vault, JsValue> {
        Session::unlock_nostr(vault, slot, secret_hex)
            .map(Vault)
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = unlockPairing)]
    pub fn unlock_pairing(vault: &str, slots: &str, fragment: &str) -> Result<Paired, JsValue> {
        Session::unlock_pairing(vault, slots, fragment)
            .map(|(session, slot)| Paired {
                vault: Some(session),
                slot,
            })
            .map_err(fail)
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.0.vault().to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u32 {
        self.0.epoch()
    }

    #[wasm_bindgen(js_name = loadIndex)]
    pub fn load_index(&mut self, blob: &[u8], min_epoch: u32) -> Result<(), JsValue> {
        self.0.load_index(blob, min_epoch).map_err(fail)
    }

    #[wasm_bindgen(js_name = indexBlob)]
    pub fn index_blob(&self) -> Result<Vec<u8>, JsValue> {
        self.0.index_blob().map_err(fail)
    }

    #[wasm_bindgen(js_name = pendingBlob)]
    pub fn pending_blob(&self) -> Result<Vec<u8>, JsValue> {
        self.0.pending_blob().map_err(fail)
    }

    pub fn commit(&mut self) {
        self.0.commit();
    }

    pub fn discard(&mut self) {
        self.0.discard();
    }

    /// The file list as JSON, for `project` or every file when it's empty.
    pub fn rows(&self, project: &str) -> Result<String, JsValue> {
        let project = (!project.is_empty()).then_some(project);
        self.0.rows(project).map_err(fail)
    }

    /// Seal `bytes` and stage the next index.
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        kind_name: &str,
        name: &str,
        media: &str,
        project: &str,
        about_json: &str,
        route_name: &str,
        bytes: &[u8],
        now_seconds: f64,
    ) -> Result<Sealed, JsValue> {
        let add = Add {
            kind: kind(kind_name),
            name,
            media: (!media.is_empty()).then_some(media),
            project: (!project.is_empty()).then_some(project),
            about: about(about_json),
            route: route(route_name),
            created_at: now(now_seconds),
        };
        self.0
            .add(add, bytes)
            .map(|(id, bytes)| Sealed { id, bytes })
            .map_err(fail)
    }

    pub fn remove(&mut self, object: &str) -> Result<(), JsValue> {
        self.0.remove(object).map_err(fail)
    }

    pub fn open(&self, object: &str, bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.0
            .open(object, bytes)
            .map(|plain| plain.to_vec())
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = recoverySlot)]
    pub fn recovery_slot(&self, now_seconds: f64) -> Result<String, JsValue> {
        self.0.recovery_slot(now(now_seconds)).map_err(fail)
    }

    #[wasm_bindgen(js_name = passkeySlot)]
    pub fn passkey_slot(
        &self,
        label: &str,
        now_seconds: f64,
        rp_id: &str,
        credential_id: &[u8],
        prf_salt: &[u8],
        prf_output: &[u8],
    ) -> Result<String, JsValue> {
        self.0
            .passkey_slot(
                label,
                now(now_seconds),
                rp_id,
                credential_id,
                prf_salt,
                prf_output,
            )
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = nostrSlot)]
    pub fn nostr_slot(
        &self,
        label: &str,
        now_seconds: f64,
        pubkey: &str,
        sealed: &str,
        secret_hex: &str,
    ) -> Result<String, JsValue> {
        self.0
            .nostr_slot(label, now(now_seconds), pubkey, sealed, secret_hex)
            .map_err(fail)
    }

    #[wasm_bindgen(js_name = pairingSlot)]
    pub fn pairing_slot(&self, now_seconds: f64, lifetime: f64) -> Result<String, JsValue> {
        self.0
            .pairing_slot(now(now_seconds), now(lifetime))
            .map_err(fail)
    }
}

#[wasm_bindgen(js_name = randomHex)]
pub fn random_hex() -> Result<String, JsValue> {
    crate::random_hex().map_err(fail)
}

#[wasm_bindgen]
pub fn enough(slots: &str) -> bool {
    crate::enough(slots)
}

#[wasm_bindgen]
pub fn qr(text: &str) -> Result<String, JsValue> {
    crate::qr(text).map_err(fail)
}
