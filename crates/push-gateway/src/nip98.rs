//! NIP-98 authorization bound to method, URL path, and body hash.
//!
//! The gateway follows the NIP-PL relay-delivery rule: it checks the signed
//! method and the signed URL's path, not its scheme, host, or port, so a
//! gateway behind a proxy or on a private address verifies the same event.
//! The signed URL must have no query, fragment, or credentials, and the
//! signed path must equal the path the request arrived on.

use nostr::domain::{Event, RelaySigner, Tag};
use secp256k1::SecretKey;

/// NIP-98 HTTP authorization kind.
pub const HTTP_AUTH_KIND: u16 = 27_235;
/// Allowed distance between `created_at` and the gateway clock, in seconds.
pub const WINDOW_SECONDS: u64 = 60;
const MAX_HEADER_BYTES: usize = 8_192;

/// A verified authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The signer: the relay for deliveries, the owner for registration.
    pub pubkey: String,
    /// The event ID, burned once admitted.
    pub event_id: String,
    /// The event's `created_at`.
    pub created_at: u64,
}

/// Sign a NIP-98 `Authorization` header value for `POST url` with `body`.
///
/// # Errors
///
/// Returns a reason when the key cannot sign.
pub fn sign(secret: &SecretKey, url: &str, body: &[u8], now: u64) -> Result<String, String> {
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|error| error.to_string())?;
    let event = signer.sign(
        now,
        HTTP_AUTH_KIND,
        vec![
            Tag::new(vec!["u".into(), url.to_owned()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["payload".into(), crate::sha256_hex(body)]),
        ],
        String::new(),
    );
    let json = serde_json::to_vec(&event).map_err(|error| error.to_string())?;
    Ok(format!(
        "Nostr {}",
        nostr::nip44::primitives::base64_encode(&json)
    ))
}

/// Verify `header` for a `POST` that arrived on `path` with `body`.
///
/// # Errors
///
/// Returns a short cause for any failure. Callers answer every failure with
/// the same `401 invalid_auth`.
pub fn verify(header: &str, path: &str, body: &[u8], now: u64) -> Result<Verified, &'static str> {
    if header.len() > MAX_HEADER_BYTES {
        return Err("header too large");
    }
    let encoded = header.strip_prefix("Nostr ").ok_or("scheme")?;
    let decoded = nostr::nip44::primitives::base64_decode(encoded.trim()).map_err(|_| "base64")?;
    let event: Event = serde_json::from_slice(&decoded).map_err(|_| "not an event")?;
    if event.kind != HTTP_AUTH_KIND {
        return Err("kind");
    }
    event.validate_structure().map_err(|_| "structure")?;
    event.validate_crypto().map_err(|_| "signature")?;
    if event.created_at.abs_diff(now) > WINDOW_SECONDS {
        return Err("timestamp");
    }
    let single = |name: &'static str| -> Result<String, &'static str> {
        let values = event.tag_values(name).collect::<Vec<_>>();
        match values.as_slice() {
            [value] => Ok((*value).to_owned()),
            _ => Err("tag count"),
        }
    };
    if single("method")? != "POST" {
        return Err("method");
    }
    if signed_path(&single("u")?)? != path {
        return Err("path");
    }
    if single("payload")? != crate::sha256_hex(body) {
        return Err("payload");
    }
    Ok(Verified {
        pubkey: event.pubkey.clone(),
        event_id: event.id.clone(),
        created_at: event.created_at,
    })
}

/// The path of an absolute `http://` or `https://` URL with no query,
/// fragment, or credentials.
fn signed_path(url: &str) -> Result<&str, &'static str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("url scheme")?;
    if url.contains(['?', '#']) || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("url shape");
    }
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('@') {
        return Err("url authority");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).unwrap()
    }

    #[test]
    fn a_signed_request_verifies_on_its_path_only() {
        let now = 1_800_000_000;
        let body = br#"{"v":1}"#;
        let header = sign(
            &secret(3),
            "http://127.0.0.1:9/v1/deliveries/apns",
            body,
            now,
        )
        .unwrap();
        let verified = verify(&header, "/v1/deliveries/apns", body, now + 30).unwrap();
        assert_eq!(verified.created_at, now);
        assert_eq!(verified.event_id.len(), 64);
        assert_eq!(
            verify(&header, "/v1/deliveries/fcm", body, now),
            Err("path")
        );
        assert_eq!(
            verify(&header, "/v1/deliveries/apns", b"{}", now),
            Err("payload")
        );
        assert_eq!(
            verify(&header, "/v1/deliveries/apns", body, now + 61),
            Err("timestamp")
        );
        assert!(verify("Bearer x", "/", body, now).is_err());
        let with_query = sign(&secret(3), "http://h/v1/deliveries/apns?x=1", body, now).unwrap();
        assert!(verify(&with_query, "/v1/deliveries/apns", body, now).is_err());
        let with_user = sign(&secret(3), "http://u@h/v1/deliveries/apns", body, now).unwrap();
        assert!(verify(&with_user, "/v1/deliveries/apns", body, now).is_err());
    }

    #[test]
    fn the_relay_adapter_header_verifies() {
        // The relay signs the full configured URL; the gateway binds the path.
        let now = 1_800_000_000;
        let body = br#"{"v":1,"endpoint_grant":"g","request_id":"r","expires_at":1}"#;
        let header = sign(
            &secret(9),
            "http://gateway.internal:8090/v1/deliveries/fcm",
            body,
            now,
        )
        .unwrap();
        let verified = verify(&header, "/v1/deliveries/fcm", body, now).unwrap();
        assert_eq!(
            verified.pubkey,
            RelaySigner::from_secret_hex(&"09".repeat(32))
                .unwrap()
                .pubkey()
        );
    }
}
