//! The open lane: a sealed decision to an ordinary Pylon that has no
//! hardware evidence (NIP-ATT level `open`).
//!
//! The job is still NIP-44 sealed to the Pylon's key, so the relay and the
//! gateway carry only ciphertext. But nothing proves what runs on that
//! machine, and its owner can read every job. What the client can check:
//!
//! 1. [`parse_beacon`]: the Pylon's `30200` beacon verifies, is signed by
//!    the key the client expects, is online and current, and offers
//!    decisions from the weights the client expects (the served identity
//!    `clef-flash@sha256:…` is the Pylon's own claim).
//! 2. [`check_answer`], after the job: the result is signed by that key,
//!    answers this request, its receipt seals and covers the response, and
//!    the receipt names the same weights. Again a claim, signed.

use nostr::att::Level;
use nostr::domain::Event;
use serde::Serialize;
use serde_json::Value;

use crate::{Refused, Tamper};

/// The Pylon's beacon, after [`parse_beacon`].
#[derive(Debug, Clone, Serialize)]
pub struct Beacon {
    /// The Pylon's key (hex), which answers and receives sealed jobs.
    pub key: String,
    /// `30200:<key>:<slug>`.
    pub address: String,
    pub label: String,
    /// The served identity the beacon claims, `clef-flash@sha256:…`.
    pub served: String,
    /// The served model's name, `clef-flash`.
    pub model: String,
    /// The weights digest the client expects (changed when tampered).
    pub expected: String,
    pub free_slots: u32,
    pub level: Level,
}

/// What [`check_answer`] found.
#[derive(Debug, Clone, Serialize)]
pub struct Checked {
    pub receipt_digest: String,
    pub request_ciphertext_digest: String,
    pub model: String,
    pub model_digest: String,
    pub level: String,
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(reason.into()))
}

/// Step 1: the beacon of the Pylon at `key` offers decisions from the
/// weights `artifact` (`sha256:…`).
///
/// # Errors
///
/// When the beacon does not verify or offers something else.
pub fn parse_beacon(
    event: &Event,
    key: &str,
    artifact: &str,
    now: u64,
    tamper: Tamper,
) -> Result<Beacon, Refused> {
    event
        .validate_crypto()
        .map_err(|_| Refused("the Pylon's beacon signature does not verify".into()))?;
    if event.pubkey != key {
        return refuse("the beacon is signed by another key than the Pylon this page uses");
    }
    let beacon = nostr::pylon::parse_beacon(event)
        .map_err(|e| Refused(format!("the Pylon's beacon is invalid: {e}")))?;
    if beacon.status != nostr::pylon::Status::Online {
        return refuse("the Pylon says it is not taking work right now");
    }
    if beacon.valid_until <= now {
        return refuse("the Pylon's beacon has expired; it may be offline");
    }
    let mut expected = artifact.to_string();
    if tamper == Tamper::Measurement {
        expected = crate::flip_last_hex(&expected);
    }
    let capability = format!("{key}:pylon/decision");
    let service = beacon
        .services
        .iter()
        .find(|s| s.capability == capability)
        .ok_or_else(|| Refused("the Pylon offers no decisions".into()))?;
    let (model, digest) = service
        .model
        .split_once('@')
        .ok_or_else(|| Refused("the Pylon names no weights digest".into()))?;
    let found = Beacon {
        key: key.to_string(),
        address: beacon.address(),
        label: beacon.label.clone(),
        served: service.model.clone(),
        model: model.to_string(),
        expected: expected.clone(),
        free_slots: beacon.slots.free,
        level: Level::Open,
    };
    if digest != expected {
        return refuse(format!(
            "the Pylon serves weights {digest}, not the expected {expected}"
        ));
    }
    Ok(found)
}

/// Step 2: check a decrypted result from the open Pylon. Arguments as in
/// [`crate::check_answer`].
///
/// # Errors
///
/// When the answer does not bind to the request or names other weights.
pub fn check_answer(
    beacon: &Beacon,
    result: &Event,
    payload: &Value,
    request: &Event,
    request_digest: &str,
) -> Result<Checked, Refused> {
    result
        .validate_crypto()
        .map_err(|_| Refused("the answer's signature does not verify".into()))?;
    if result.pubkey != beacon.key {
        return refuse("the answer is not signed by the Pylon's key");
    }
    if !result.tag_values("e").any(|id| id == request.id) {
        return refuse("the answer names another request");
    }
    let receipt: receipts::ExecutionReceipt = serde_json::from_value(payload["receipt"].clone())
        .map_err(|e| Refused(format!("the receipt does not parse: {e}")))?;
    receipt
        .verify()
        .map_err(|e| Refused(format!("the receipt's seal is broken: {e}")))?;
    if receipt.attempt_id != request.id || receipt.request_digest != request_digest {
        return refuse("the receipt is for another request");
    }
    let response = &payload["response"];
    // An ordinary Pylon digests the response's compact JSON (keys in
    // order); an attested one its JCS bytes. Either covers the answer.
    let compact = nostr::att::sha256_hex(response.to_string().as_bytes());
    let digest = receipt.result_digest.as_deref();
    if digest != Some(compact.as_str()) && digest != Some(crate::response_digest(response).as_str())
    {
        return refuse("the receipt does not cover this answer");
    }
    if receipt.served.artifact_signature != beacon.expected {
        return refuse("the receipt names other weights than the Pylon advertised");
    }
    Ok(Checked {
        receipt_digest: receipt.digest.clone(),
        request_ciphertext_digest: nostr::att::sha256_hex(request.content.as_bytes()),
        model: beacon.model.clone(),
        model_digest: receipt.served.artifact_signature.clone(),
        level: Level::Open.as_str().to_string(),
    })
}
