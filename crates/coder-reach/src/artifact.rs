//! Seal and open NIP-REACH bodies as private `3188` artifacts.
//!
//! Opening checks the original signature, the exact signer and recipient, the
//! schema, and the inline bytes. It does not check the body's authority; each
//! record type does that after opening.

use std::str::FromStr;

use nostr::{contracts, domain::Event, private_artifact};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{Error, Refusal, Result, fail, random_bytes};

/// Largest canonical body any NIP-REACH artifact may carry.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Largest encrypted content accepted before decryption.
pub const MAX_CIPHERTEXT_BYTES: usize = 128 * 1024;

/// Envelope facts that callers compare with the body they opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealed {
    pub event_id: String,
    pub signer: String,
    pub recipient: String,
    pub issued_at: u64,
    pub retain_until: u64,
}

/// Canonical JCS bytes of a body, bounded.
///
/// # Errors
/// Refuses bodies that do not serialize or exceed [`MAX_BODY_BYTES`].
pub fn canonical(value: &impl Serialize) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)
        .map_err(|_| Error::new(Refusal::Malformed, "body serialization"))?;
    let bytes =
        contracts::jcs(&value).map_err(|_| Error::new(Refusal::Malformed, "canonical body"))?;
    if bytes.len() > MAX_BODY_BYTES {
        return fail(Refusal::LimitExceeded, "body exceeds its byte bound");
    }
    Ok(bytes)
}

/// Seal `value` for one recipient under a caller-held mailbox.
///
/// # Errors
/// Refuses oversized or unserializable bodies and invalid keys.
pub fn seal(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    recipient: &str,
    mailbox: &str,
    issued_at: u64,
    retain_until: u64,
) -> Result<Event> {
    crate::random_id(mailbox)?;
    if retain_until <= issued_at {
        return fail(Refusal::Malformed, "retention must follow issue time");
    }
    let bytes = canonical(value)?;
    let inline: Value = serde_json::from_slice(&bytes)
        .map_err(|_| Error::new(Refusal::Malformed, "canonical body"))?;
    let body = json!({
        "v": "openagents.artifact-envelope.v1",
        "requires": [],
        "artifact": {
            "digest": contracts::digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": "application/json",
            "schema": schema,
        },
        "inline": inline,
        "issued_at": issued_at,
        "retain_until": retain_until,
    });
    let recipient = XOnlyPublicKey::from_str(recipient)
        .map_err(|_| Error::new(Refusal::Malformed, "recipient"))?;
    private_artifact::seal(
        &body,
        secret,
        &recipient,
        mailbox,
        issued_at,
        random_bytes(),
    )
    .map_err(|_| Error::new(Refusal::Malformed, "envelope cannot be sealed"))
}

/// Open an artifact with the reader's key and decode its closed body.
///
/// `signer` is the expected original author; `recipient` the exact encrypted
/// recipient. A copy signed by anyone else refuses as `identity_mismatch`.
///
/// # Errors
/// Refuses wrong signers, recipients, schemas, sizes, or malformed bodies.
pub fn open<T: DeserializeOwned>(
    event: &Event,
    secret: &SecretKey,
    signer: &str,
    recipient: &str,
    schema: &str,
) -> Result<(T, Sealed)> {
    if event.content.len() > MAX_CIPHERTEXT_BYTES {
        return fail(
            Refusal::LimitExceeded,
            "encrypted content exceeds its bound",
        );
    }
    let opened = private_artifact::open(event, secret).map_err(|_| {
        Error::new(
            Refusal::IdentityMismatch,
            "signature, encryption, or reader",
        )
    })?;
    if opened.signer() != signer || opened.recipient() != recipient {
        return fail(Refusal::IdentityMismatch, "unexpected signer or recipient");
    }
    if opened.artifact().schema.as_deref() != Some(schema)
        || opened.artifact().media_type != "application/json"
    {
        return fail(Refusal::UnsupportedVersion, "unexpected artifact schema");
    }
    if event.created_at != opened.body().issued_at {
        return fail(Refusal::Malformed, "event and envelope issue times differ");
    }
    let bytes = opened
        .inline_bytes()
        .ok_or_else(|| Error::new(Refusal::Unavailable, "inline body unavailable"))?;
    let value = contracts::parse_strict_bounded(bytes, MAX_BODY_BYTES)
        .map_err(|_| Error::new(Refusal::Malformed, "body JSON"))?;
    let body =
        serde_json::from_value(value).map_err(|_| Error::new(Refusal::Malformed, "body fields"))?;
    Ok((
        body,
        Sealed {
            event_id: opened.event_id().to_owned(),
            signer: opened.signer().to_owned(),
            recipient: opened.recipient().to_owned(),
            issued_at: opened.body().issued_at,
            retain_until: opened.body().retain_until,
        },
    ))
}
