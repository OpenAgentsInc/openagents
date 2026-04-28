//! Proof-of-control challenges for existing Nostr keys.
//!
//! Flow:
//! 1. Caller posts `name` + `pubkey` (or `npub`) to `/claim/challenge`.
//!    The registrar issues a short-lived random `otp` plus a canonical
//!    `message` string the caller must include verbatim in the `content`
//!    of a Nostr event signed by that pubkey.
//! 2. Caller posts the signed event to `/claim/complete` along with the
//!    `challengeId`. The registrar verifies (a) signature is valid for the
//!    bound pubkey, (b) the event content includes the OTP and the canonical
//!    message fields, (c) the challenge is not expired, and (d) the
//!    challenge has not been used before. On success the claim lands.
//!
//! The OTP is high-entropy (>= 60 bits) so it can't be brute-forced inside
//! the 10-minute lifetime. The message embeds domain, action, handle,
//! pubkey, nonce, and expiry so a signature for one challenge can never be
//! replayed against another (different handle, different domain, different
//! day, etc).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;
use rand::rng;
use serde::Serialize;
use subtle::ConstantTimeEq;

use crate::error::RegistrarError;

/// Domain that the challenge is bound to. Public so callers can show it.
pub const CHALLENGE_DOMAIN: &str = "openagents.com";
/// Default TTL for an issued challenge (10 minutes).
pub const CHALLENGE_TTL_SECS: u64 = 600;
/// Required event kind for a claim proof event. We use kind 27235 because
/// it's already in the "ephemeral, signed message" range used by NIP-98.
/// We're not implementing NIP-98 itself, but borrowing the range keeps us
/// out of conflict with regular notes / DMs.
pub const PROOF_EVENT_KIND: u16 = 27235;

#[derive(Debug, Clone)]
pub struct ChallengeRecord {
    pub id: String,
    pub name: String,
    pub pubkey_hex: String,
    pub otp: String,
    pub nonce: String,
    pub created_at: u64,
    pub expires_at: u64,
}

impl ChallengeRecord {
    /// Render the canonical, line-by-line message a caller must embed in
    /// their signed event content. Both sides reconstruct this identically.
    pub fn canonical_message(&self) -> String {
        format!(
            "OpenAgents NIP-05 claim proof\n\
             domain: {domain}\n\
             action: claim\n\
             handle: {handle}\n\
             pubkey: {pubkey}\n\
             challenge_id: {cid}\n\
             nonce: {nonce}\n\
             otp: {otp}\n\
             expires_at: {exp}",
            domain = CHALLENGE_DOMAIN,
            handle = self.name,
            pubkey = self.pubkey_hex,
            cid = self.id,
            nonce = self.nonce,
            otp = self.otp,
            exp = self.expires_at,
        )
    }
}

/// API-shape view of a challenge — what the server hands back to the
/// caller. Includes the OTP (plain) and the rendered message so clients
/// can sign it without recomputing the format.
#[derive(Debug, Serialize)]
pub struct ChallengeView {
    pub challenge_id: String,
    pub otp: String,
    pub nonce: String,
    pub message: String,
    pub expires_at: u64,
    pub domain: &'static str,
    pub kind: u16,
}

#[derive(Debug, Default)]
pub struct ChallengeStore {
    inner: Mutex<HashMap<String, ChallengeRecord>>,
}

impl ChallengeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issue(
        &self,
        name: &str,
        pubkey_hex: &str,
        ttl_secs: u64,
    ) -> Result<ChallengeRecord, RegistrarError> {
        let now = unix_now()?;
        let id = format!("oa-{}", uuid_like());
        let otp = generate_otp();
        let nonce = generate_nonce();
        let record = ChallengeRecord {
            id: id.clone(),
            name: name.to_string(),
            pubkey_hex: pubkey_hex.to_string(),
            otp,
            nonce,
            created_at: now,
            expires_at: now.saturating_add(ttl_secs),
        };
        let mut guard = self.inner.lock().map_err(|err| {
            tracing::error!(error = %err, "challenge store mutex poisoned");
            RegistrarError::Internal
        })?;
        // Drop expired entries opportunistically every issuance to keep
        // memory bounded without a dedicated GC task.
        guard.retain(|_, rec| rec.expires_at > now);
        guard.insert(id, record.clone());
        Ok(record)
    }

    /// Take (consume) the challenge by id. Returns the record if it
    /// existed and was not expired. The record is removed regardless to
    /// prevent replays of the same challenge.
    pub fn take(&self, id: &str) -> Result<ChallengeRecord, RegistrarError> {
        let now = unix_now()?;
        let mut guard = self.inner.lock().map_err(|err| {
            tracing::error!(error = %err, "challenge store mutex poisoned");
            RegistrarError::Internal
        })?;
        // Locate via constant-time comparison so an attacker can't time
        // their way to a known-prefix challenge id.
        let mut found_key: Option<String> = None;
        for key in guard.keys() {
            if key.as_bytes().ct_eq(id.as_bytes()).unwrap_u8() == 1 {
                found_key = Some(key.clone());
                break;
            }
        }
        let Some(key) = found_key else {
            return Err(RegistrarError::ChallengeNotFound);
        };
        let record = guard
            .remove(&key)
            .ok_or(RegistrarError::ChallengeNotFound)?;
        if record.expires_at <= now {
            return Err(RegistrarError::ChallengeExpired);
        }
        Ok(record)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }
}

fn unix_now() -> Result<u64, RegistrarError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|err| {
            tracing::error!(error = %err, "system time before epoch");
            RegistrarError::Internal
        })
}

fn uuid_like() -> String {
    // 16 random bytes hex-encoded (128 bits). uuid v4 gives the same
    // entropy but with hyphens. This avoids the extra dep dance for an
    // identifier that doesn't need RFC4122 layout. We still use uuid in
    // `Cargo.toml` for completeness in case future endpoints want it.
    let mut buf = [0u8; 16];
    rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

fn generate_nonce() -> String {
    let mut buf = [0u8; 16];
    rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Generate a 12-character base32-style OTP from 60 bits of entropy.
/// Uses Crockford-style alphabet (no I/L/O/U) for human readability when
/// the user has to type or read the OTP out loud at a booth. Search space
/// is ~1.15e18, far beyond brute-forcing inside the 10-minute TTL.
fn generate_otp() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    const LEN: usize = 12;
    let mut out = String::with_capacity(LEN + 2);
    let mut buf = [0u8; LEN];
    rng().fill_bytes(&mut buf);
    for (i, b) in buf.iter().enumerate() {
        if i > 0 && i % 4 == 0 {
            out.push('-');
        }
        out.push(ALPHABET[(*b as usize) & 0x1f] as char);
    }
    out
}

/// Verify a Nostr event signature, kind, age, and embedded canonical
/// message. The event must:
/// - Verify against `expected_pubkey`.
/// - Have `kind == PROOF_EVENT_KIND`.
/// - Have a `created_at` no older than the challenge `created_at - 5min`
///   and no newer than the challenge `expires_at + 5min`. This loose band
///   tolerates clock skew while still binding the event to roughly the
///   same time window as the challenge.
/// - Contain the canonical message in `content`. We do NOT require the
///   content to be exactly the message — the canonical message must
///   appear as a substring so callers may add their own preamble.
pub fn verify_proof_event(
    record: &ChallengeRecord,
    event_json: &serde_json::Value,
) -> Result<(), RegistrarError> {
    let event: nostr::nip01::Event = serde_json::from_value(event_json.clone()).map_err(|err| {
        tracing::warn!(error = %err, "claim proof event failed to parse");
        RegistrarError::ChallengeInvalid("malformed event")
    })?;

    if event.pubkey.to_ascii_lowercase() != record.pubkey_hex {
        return Err(RegistrarError::ChallengeInvalid("pubkey mismatch"));
    }
    if event.kind != PROOF_EVENT_KIND {
        return Err(RegistrarError::ChallengeInvalid("unexpected kind"));
    }
    let lower = record.created_at.saturating_sub(300);
    let upper = record.expires_at.saturating_add(300);
    if event.created_at < lower || event.created_at > upper {
        return Err(RegistrarError::ChallengeInvalid("created_at out of range"));
    }
    let canonical = record.canonical_message();
    if !event.content.contains(&canonical) {
        return Err(RegistrarError::ChallengeInvalid(
            "canonical message missing from content",
        ));
    }

    let verified = nostr::nip01::verify_event(&event).map_err(|err| {
        tracing::warn!(error = %err, "schnorr verification errored");
        RegistrarError::ChallengeInvalid("signature verify failed")
    })?;
    if !verified {
        return Err(RegistrarError::ChallengeInvalid("invalid signature"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn issue_then_take_consumes_record() {
        let store = ChallengeStore::new();
        let rec = store
            .issue(
                "alice",
                "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
                CHALLENGE_TTL_SECS,
            )
            .unwrap();
        let id = rec.id.clone();
        let taken = store.take(&id).unwrap();
        assert_eq!(taken.name, "alice");
        // Second take is rejected — replay protection.
        assert!(matches!(
            store.take(&id),
            Err(RegistrarError::ChallengeNotFound)
        ));
    }

    #[test]
    fn take_unknown_returns_not_found() {
        let store = ChallengeStore::new();
        assert!(matches!(
            store.take("oa-doesnotexist"),
            Err(RegistrarError::ChallengeNotFound)
        ));
    }

    #[test]
    fn expired_record_returns_expired_error() {
        let store = ChallengeStore::new();
        let rec = store
            .issue(
                "alice",
                "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
                0,
            )
            .unwrap();
        // Sleep a beat so unix_now() advances past created_at.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(matches!(
            store.take(&rec.id),
            Err(RegistrarError::ChallengeExpired)
        ));
    }

    #[test]
    fn otp_format_has_separators_and_alphabet() {
        let otp = generate_otp();
        assert!(otp.contains('-'));
        for c in otp.chars().filter(|c| *c != '-') {
            assert!("0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c));
        }
    }
}
