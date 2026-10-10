//! Nostr identity on the API (#11148): `Authorization: Nostr <event>`
//! (NIP-98) on a paid-per-request call, so the payer is one known buyer.
//!
//! - **What is signed.** A kind `27235` event with exactly one `u` (the
//!   absolute URL the request was sent to), one `method` (`POST`), and one
//!   `payload` (the lowercase SHA-256 of the exact body bytes). The body
//!   hash is required here: every paid call has a body, and a signature
//!   that does not bind it could be lifted onto other bytes.
//! - **Freshness and replay.** `created_at` within 60 seconds of our clock
//!   (NIP-98's window), and each event id admitted once: it is held while
//!   its payment settles, kept for twice the window once a payment is
//!   taken, and given back if no payment is taken (the `402`, a refused
//!   proof) or the paid request is never answered, so the retry may carry
//!   the same signature.
//! - **What it is not.** An identity, never a payment: a valid signature
//!   with no payment still gets the `402`. A bad signature is refused with
//!   `401` before any invoice is made or any proof is consumed, so a payer
//!   never pays for a call whose identity we would not accept.
//! - **Where it goes.** The payer's x-only pubkey lands in the receipt's
//!   `payer.nostr` (`openagents.payment-receipt.v1`). NIP-X402 keeps it out
//!   of the protocol's own settlement response: Lightning has no payer.
//!
//! The same header carries owner-backed budgets later (`BuyerAttestation`,
//! PR #11088): the attestation names this key.
//!
//! An `Authorization` field holds one credential, so a payer that uses the
//! `Payment` scheme sends two `Authorization` fields, one `Payment …` and
//! one `Nostr …`; both are read.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::http::HeaderMap;
use nostr::domain::{HttpAuth, parse_http_authorization};

/// NIP-98's freshness window, in seconds.
pub const WINDOW_SECONDS: u64 = 60;
/// How long an admitted event id is remembered: past any `created_at` the
/// window can still accept.
pub const REMEMBER_SECONDS: u64 = 2 * WINDOW_SECONDS + 1;
/// The most event ids held at once; past this, new ones are refused
/// until old ones age out, rather than forgetting live ones.
pub const MAX_HELD: usize = 100_000;
const MAX_HEADER_BYTES: usize = 16 * 1024;

/// The `Authorization` value with the `Nostr` scheme, if any. More than
/// one is an error: a request names one buyer.
///
/// # Errors
///
/// Two or more `Nostr` credentials, or one that is not text.
pub fn find(headers: &HeaderMap) -> Result<Option<String>, &'static str> {
    let mut found = None;
    for value in headers.get_all("authorization") {
        let Ok(text) = value.to_str() else {
            if is_nostr_value(value.as_bytes()) {
                return Err("The Nostr authorization isn't text.");
            }
            continue;
        };
        if !is_nostr(text) {
            continue;
        }
        if found.is_some() {
            return Err("Send one Nostr authorization, not several.");
        }
        found = Some(normalize(text));
    }
    Ok(found)
}

/// Whether an `Authorization` value uses the `Nostr` scheme (any case).
#[must_use]
pub fn is_nostr(value: &str) -> bool {
    is_nostr_bytes(value.trim_start().as_bytes())
}

/// [`is_nostr`] for a raw header value, text or not.
#[must_use]
pub fn is_nostr_value(value: &[u8]) -> bool {
    is_nostr_bytes(value.trim_ascii_start())
}

fn is_nostr_bytes(value: &[u8]) -> bool {
    value
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"nostr "))
}

/// `Nostr <token>` with the scheme in NIP-98's spelling.
fn normalize(value: &str) -> String {
    let token = value.trim_start()[6..].trim();
    format!("Nostr {token}")
}

/// Verify `header` for a `POST` of `body` sent to one of `urls` (the
/// request's absolute URL; behind a proxy that ends TLS, also its
/// `https://` spelling).
///
/// # Errors
///
/// A sentence for the caller: what did not match.
pub fn verify(header: &str, urls: &[String], body: &[u8], now: u64) -> Result<HttpAuth, String> {
    if header.len() > MAX_HEADER_BYTES {
        return Err("The Nostr authorization is too large.".into());
    }
    let mut last = None;
    for url in urls {
        match parse_http_authorization(header, "POST", url, body, now) {
            Ok(auth) => return Ok(auth),
            Err(error) => last = Some(error.to_string()),
        }
    }
    Err(format!(
        "The Nostr authorization wasn't accepted ({}). Sign a kind 27235 event with `u` {}, `method` POST, and `payload` the SHA-256 of the body, at most {WINDOW_SECONDS} seconds old.",
        last.unwrap_or_else(|| "no URL to check".into()),
        urls.first().map_or("the request URL", String::as_str),
    ))
}

/// The URLs a signed `u` may name for a request that arrived at `url`.
/// `guessed` is true when the origin came from the `Host` header (no
/// configured origin): then a TLS-ending proxy may have turned the
/// client's `https://` into our `http://`.
#[must_use]
pub fn candidates(url: &str, guessed: bool) -> Vec<String> {
    let mut out = vec![url.to_owned()];
    if guessed && let Some(rest) = url.strip_prefix("http://") {
        out.push(format!("https://{rest}"));
    }
    out
}

/// Event ids admitted recently, each with when it may be forgotten.
#[derive(Default)]
pub struct Seen {
    held: Mutex<HashMap<String, u64>>,
}

impl Seen {
    /// Hold `event_id` (until `now + REMEMBER_SECONDS`). `false` when it
    /// is already held: a replay.
    pub fn hold(&self, event_id: &str, now: u64) -> bool {
        let Ok(mut held) = self.held.lock() else {
            return false;
        };
        held.retain(|_, until| *until > now);
        if held.contains_key(event_id) || held.len() >= MAX_HELD {
            return false;
        }
        held.insert(event_id.to_owned(), now + REMEMBER_SECONDS);
        true
    }

    /// Forget `event_id`: its request took no payment, or was never
    /// answered, so the same signature may come again.
    pub fn release(&self, event_id: &str) {
        if let Ok(mut held) = self.held.lock() {
            held.remove(event_id);
        }
    }

    /// Whether `event_id` is held now.
    #[must_use]
    pub fn holds(&self, event_id: &str, now: u64) -> bool {
        self.held
            .lock()
            .is_ok_and(|held| held.get(event_id).is_some_and(|until| *until > now))
    }
}

/// A NIP-98 `Authorization` value for a `POST` of `body` to `url`, signed
/// by `secret_hex` at `now`. For tests and for clients in this repo.
///
/// # Errors
///
/// A key that is not 32 bytes of lowercase hex.
pub fn sign(secret_hex: &str, url: &str, body: &[u8], now: u64) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let signer =
        nostr::domain::RelaySigner::from_secret_hex(secret_hex).map_err(|e| e.to_string())?;
    let event = signer.sign(
        now,
        27_235,
        vec![
            nostr::domain::Tag::new(vec!["u".into(), url.to_owned()]),
            nostr::domain::Tag::new(vec!["method".into(), "POST".into()]),
            nostr::domain::Tag::new(vec!["payload".into(), hex::encode(Sha256::digest(body))]),
        ],
        String::new(),
    );
    let json = serde_json::to_vec(&event).map_err(|e| e.to_string())?;
    Ok(format!(
        "Nostr {}",
        nostr::nip44::primitives::base64_encode(&json)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const KEY: &str = "0303030303030303030303030303030303030303030303030303030303030303";
    const URL: &str = "https://api.openagents.com/v1/responses";
    const NOW: u64 = 1_800_000_000;

    fn urls() -> Vec<String> {
        vec![URL.to_owned()]
    }

    #[test]
    fn a_signed_request_binds_method_url_and_body() {
        let body = br#"{"model":"openagents/chat","input":"hi"}"#;
        let header = sign(KEY, URL, body, NOW).unwrap();
        let auth = verify(&header, &urls(), body, NOW + 30).unwrap();
        assert_eq!(auth.pubkey.len(), 64);
        assert_eq!(auth.event_id.len(), 64);

        // Other bytes, another URL, or outside the window.
        assert!(verify(&header, &urls(), b"{}", NOW).is_err());
        assert!(
            verify(
                &header,
                &["https://api.openagents.com/v1/chat/completions".to_owned()],
                body,
                NOW
            )
            .is_err()
        );
        assert!(verify(&header, &urls(), body, NOW + WINDOW_SECONDS + 1).is_err());
        assert!(verify(&header, &urls(), body, NOW - WINDOW_SECONDS - 1).is_err());
        // A tampered signature.
        let encoded = header.strip_prefix("Nostr ").unwrap();
        let mut event: serde_json::Value =
            serde_json::from_slice(&nostr::nip44::primitives::base64_decode(encoded).unwrap())
                .unwrap();
        let sig = event["sig"].as_str().unwrap().to_owned();
        let flipped = if sig.starts_with('0') { "1" } else { "0" };
        event["sig"] = serde_json::Value::String(format!("{flipped}{}", &sig[1..]));
        let tampered = format!(
            "Nostr {}",
            nostr::nip44::primitives::base64_encode(&serde_json::to_vec(&event).unwrap())
        );
        assert!(verify(&tampered, &urls(), body, NOW).is_err());
    }

    #[test]
    fn a_signature_without_the_body_hash_is_refused() {
        use nostr::domain::{RelaySigner, Tag};
        let signer = RelaySigner::from_secret_hex(KEY).unwrap();
        let event = signer.sign(
            NOW,
            27_235,
            vec![
                Tag::new(vec!["u".into(), URL.into()]),
                Tag::new(vec!["method".into(), "POST".into()]),
            ],
            String::new(),
        );
        let header = format!(
            "Nostr {}",
            nostr::nip44::primitives::base64_encode(&serde_json::to_vec(&event).unwrap())
        );
        assert!(verify(&header, &urls(), b"{}", NOW).is_err());
    }

    #[test]
    fn a_proxy_may_turn_https_into_http_only_when_the_origin_was_guessed() {
        let body = b"{}";
        let header = sign(KEY, URL, body, NOW).unwrap();
        let ours = "http://api.openagents.com/v1/responses";
        assert!(verify(&header, &candidates(ours, false), body, NOW).is_err());
        assert!(verify(&header, &candidates(ours, true), body, NOW).is_ok());
    }

    #[test]
    fn each_event_is_admitted_once_until_given_back_or_aged_out() {
        let seen = Seen::default();
        assert!(seen.hold("a", NOW));
        assert!(!seen.hold("a", NOW + 1), "a replay");
        assert!(seen.holds("a", NOW + 1));
        seen.release("a");
        assert!(seen.hold("a", NOW + 2), "given back, it may come again");
        assert!(seen.hold("a", NOW + 2 + REMEMBER_SECONDS), "aged out");
    }

    #[test]
    fn the_nostr_value_is_found_among_several_authorization_fields() {
        let mut headers = HeaderMap::new();
        assert_eq!(find(&headers), Ok(None));
        headers.append(
            "authorization",
            HeaderValue::from_static("Payment eyJhIjoxfQ"),
        );
        assert_eq!(find(&headers), Ok(None));
        headers.append("authorization", HeaderValue::from_static("nostr  abc "));
        assert_eq!(find(&headers), Ok(Some("Nostr abc".to_owned())));
        headers.append("authorization", HeaderValue::from_static("Nostr def"));
        assert!(find(&headers).is_err(), "one buyer per request");
        assert!(is_nostr("Nostr x"));
        assert!(!is_nostr("Bearer oak_x"));
        assert!(!is_nostr("Nostr"));
    }
}
