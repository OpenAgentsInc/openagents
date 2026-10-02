//! Verify Boat webhook signatures against the exact HTTP request body.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{Error, Result, models::WebhookEvent};

/// Boat kept the Ascii names for its webhook headers.
pub const DELIVERY_HEADER: &str = "X-Ascii-Delivery";
pub const TIMESTAMP_HEADER: &str = "X-Ascii-Timestamp";
pub const SIGNATURE_HEADER: &str = "X-Ascii-Signature";

/// The signature headers from an incoming Boat delivery.
pub struct Headers<'a> {
    pub delivery: &'a str,
    pub timestamp: &'a str,
    pub signature: &'a str,
}

/// Verify freshness and HMAC before decoding a webhook.
///
/// Pass `X-Ascii-Delivery`, `X-Ascii-Timestamp`, and `X-Ascii-Signature` unchanged.
/// The secret is the complete `whsec_...` string returned by Boat. Retain delivery
/// IDs in your application to deduplicate retries after successful verification.
pub fn verify(
    secret: &str,
    headers: Headers<'_>,
    body: &[u8],
    now: SystemTime,
    tolerance: Duration,
) -> Result<WebhookEvent> {
    let Headers {
        delivery,
        timestamp,
        signature,
    } = headers;
    if !delivery.starts_with("evt_")
        || delivery.len() != 36
        || !delivery[4..].bytes().all(|c| c.is_ascii_hexdigit())
        || timestamp.is_empty()
        || !timestamp.bytes().all(|c| c.is_ascii_digit())
        || secret.is_empty()
    {
        return Err(Error::InvalidSignature);
    }
    let seconds: u64 = timestamp.parse().map_err(|_| Error::InvalidSignature)?;
    let current = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::InvalidSignature)?
        .as_secs();
    if current.abs_diff(seconds) > tolerance.as_secs() {
        return Err(Error::InvalidSignature);
    }
    let hex = signature
        .strip_prefix("v1=")
        .ok_or(Error::InvalidSignature)?;
    if hex.len() != 64 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::InvalidSignature);
    }
    let mut expected = [0u8; 32];
    for (slot, pair) in expected.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let digits = std::str::from_utf8(pair).map_err(|_| Error::InvalidSignature)?;
        *slot = u8::from_str_radix(digits, 16).map_err(|_| Error::InvalidSignature)?;
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| Error::InvalidSignature)?;
    mac.update(delivery.as_bytes());
    mac.update(b".");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.verify_slice(&expected)
        .map_err(|_| Error::InvalidSignature)?;
    let event: WebhookEvent = serde_json::from_slice(body).map_err(|_| Error::Decode)?;
    if event.id != delivery {
        return Err(Error::InvalidSignature);
    }
    Ok(event)
}
