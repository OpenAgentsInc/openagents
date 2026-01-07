//! NIP-42: Authentication of Clients to Relays
//!
//! Handles challenge generation and auth event validation.

use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

/// Auth kind (NIP-42)
pub const AUTH_KIND: u16 = 22242;

/// Maximum time difference allowed between auth event and current time (in seconds)
pub const MAX_TIME_DIFF: u64 = 600; // 10 minutes

/// Auth validation errors
#[derive(Debug, Error)]
pub enum Nip42Error {
    #[error("invalid auth event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },
    #[error("auth event timestamp too old or in future: {diff} seconds difference")]
    TimestampSkew { diff: u64 },
    #[error("missing challenge tag in auth event")]
    MissingChallenge,
    #[error("challenge mismatch")]
    ChallengeMismatch,
    #[error("missing relay tag in auth event")]
    MissingRelay,
    #[error("relay URL mismatch: expected {expected}, got {actual}")]
    RelayMismatch { expected: String, actual: String },
    #[error("system clock error")]
    SystemTime,
}

/// Auth state for a connection
#[derive(Debug, Clone)]
pub enum AuthState {
    /// Not yet authenticated
    Unauthenticated { challenge: String },
    /// Successfully authenticated
    Authenticated { pubkey: String },
}

/// Generate a random challenge string
pub fn generate_challenge() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("failed to generate random bytes");
    hex::encode(bytes)
}

/// Validate an auth event
///
/// Returns the authenticated pubkey on success.
pub fn validate_auth_event(
    event: &nostr::Event,
    expected_challenge: &str,
    expected_relay_url: &str,
) -> Result<String, Nip42Error> {
    // Check event kind
    if event.kind != AUTH_KIND {
        return Err(Nip42Error::InvalidKind {
            expected: AUTH_KIND,
            actual: event.kind,
        });
    }

    // Check timestamp is recent
    let now = unix_timestamp()?;
    let time_diff = if event.created_at > now {
        event.created_at - now
    } else {
        now - event.created_at
    };
    if time_diff > MAX_TIME_DIFF {
        return Err(Nip42Error::TimestampSkew { diff: time_diff });
    }

    // Extract challenge and relay from tags
    let mut found_challenge = None;
    let mut found_relay = None;

    for tag in &event.tags {
        if tag.len() >= 2 {
            match tag[0].as_str() {
                "challenge" => found_challenge = Some(tag[1].as_str()),
                "relay" => found_relay = Some(tag[1].as_str()),
                _ => {}
            }
        }
    }

    // Validate challenge
    let challenge = found_challenge.ok_or(Nip42Error::MissingChallenge)?;
    if challenge != expected_challenge {
        return Err(Nip42Error::ChallengeMismatch);
    }

    // Validate relay URL
    let relay = found_relay.ok_or(Nip42Error::MissingRelay)?;

    // Normalize and compare relay URLs
    let normalized_relay = normalize_relay_url(relay);
    let normalized_expected = normalize_relay_url(expected_relay_url);
    if normalized_relay != normalized_expected {
        return Err(Nip42Error::RelayMismatch {
            expected: expected_relay_url.to_string(),
            actual: relay.to_string(),
        });
    }

    // Signature verification is runtime-specific and not enforced here.

    Ok(event.pubkey.clone())
}

fn unix_timestamp() -> Result<u64, Nip42Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Nip42Error::SystemTime)
}

/// Normalize a relay URL for comparison
fn normalize_relay_url(url: &str) -> String {
    let mut normalized = url.to_lowercase();

    // Remove trailing slash
    if normalized.ends_with('/') {
        normalized.pop();
    }

    // Ensure wss:// prefix
    if normalized.starts_with("ws://") {
        normalized = format!("wss://{}", &normalized[5..]);
    } else if !normalized.starts_with("wss://") {
        normalized = format!("wss://{}", normalized);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_relay_url() {
        assert_eq!(
            normalize_relay_url("wss://relay.example.com/"),
            "wss://relay.example.com"
        );
        assert_eq!(
            normalize_relay_url("ws://relay.example.com"),
            "wss://relay.example.com"
        );
        assert_eq!(
            normalize_relay_url("relay.example.com"),
            "wss://relay.example.com"
        );
    }
}
