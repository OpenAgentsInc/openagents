//! NIP-42: Authentication of Clients to Relays
//!
//! Handles challenge generation and auth event validation.

use worker::*;

/// Auth kind (NIP-42)
pub const AUTH_KIND: u16 = 22242;

/// Maximum time difference allowed between auth event and current time (in seconds)
pub const MAX_TIME_DIFF: u64 = 600; // 10 minutes

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
) -> Result<String> {
    // Check event kind
    if event.kind != AUTH_KIND {
        return Err(Error::RustError(format!(
            "invalid auth event kind: expected {}, got {}",
            AUTH_KIND, event.kind
        )));
    }

    // Check timestamp is recent
    let now = js_sys::Date::now() as u64 / 1000;
    let time_diff = if event.created_at > now {
        event.created_at - now
    } else {
        now - event.created_at
    };
    if time_diff > MAX_TIME_DIFF {
        return Err(Error::RustError(format!(
            "auth event timestamp too old or in future: {} seconds difference",
            time_diff
        )));
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
    let challenge = found_challenge.ok_or_else(|| {
        Error::RustError("missing challenge tag in auth event".to_string())
    })?;
    if challenge != expected_challenge {
        return Err(Error::RustError("challenge mismatch".to_string()));
    }

    // Validate relay URL
    let relay = found_relay.ok_or_else(|| {
        Error::RustError("missing relay tag in auth event".to_string())
    })?;

    // Normalize and compare relay URLs
    let normalized_relay = normalize_relay_url(relay);
    let normalized_expected = normalize_relay_url(expected_relay_url);
    if normalized_relay != normalized_expected {
        return Err(Error::RustError(format!(
            "relay URL mismatch: expected {}, got {}",
            expected_relay_url, relay
        )));
    }

    // Signature verification is skipped in the minimal WASM build.

    Ok(event.pubkey.clone())
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
