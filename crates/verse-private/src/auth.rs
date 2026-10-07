//! The grant request: a reader asks the broker for one pack with a NIP-98
//! `Authorization: Nostr ...` header, a kind 27235 event its Verse key signs
//! over the exact URL, the method, and the body's SHA-256.

use base64::Engine as _;
use nostr::domain::{RelaySigner, Tag};
use serde::{Deserialize, Serialize};

use crate::sha256_hex;

/// The broker's grant route.
pub const ROUTE: &str = "/v1/private/url";
/// NIP-98's HTTP authorization kind.
pub const HTTP_AUTH_KIND: u16 = 27_235;
/// How long a signed URL stays valid, s.
pub const URL_SECONDS: u32 = 300;
/// The largest grant request body, bytes.
pub const MAX_REQUEST_BYTES: usize = 1024;

/// What a reader asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRequest {
    /// The registry name.
    pub name: String,
    /// The pack the reader expects.
    pub sha256: String,
}

/// What the broker returns: a URL for the pack that expires at `expires`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// The V4 signed URL.
    pub url: String,
    /// The pack's SHA-256.
    pub sha256: String,
    /// The pack's length.
    pub bytes: u64,
    /// When the URL stops working, Unix seconds.
    pub expires: u64,
}

impl GrantRequest {
    /// The exact body a reader signs and sends.
    #[must_use]
    pub fn body(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// The grant URL on `broker`, an `https://` origin.
#[must_use]
pub fn grant_url(broker: &str) -> String {
    format!("{}{ROUTE}", broker.trim_end_matches('/'))
}

/// A NIP-98 `Nostr ...` header value: `signer` over a `POST` of `body` to
/// `url` at `now`.
#[must_use]
pub fn authorization(signer: &RelaySigner, url: &str, body: &[u8], now: u64) -> String {
    let tags = vec![
        Tag::new(vec!["u".into(), url.into()]),
        Tag::new(vec!["method".into(), "POST".into()]),
        Tag::new(vec!["payload".into(), sha256_hex(body)]),
    ];
    let event = signer.sign(now, HTTP_AUTH_KIND, tags, String::new());
    let json = serde_json::to_vec(&event).unwrap_or_default();
    format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD.encode(json)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_request_verifies_only_for_its_url_body_and_time() {
        let signer = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let url = grant_url("https://broker.example/");
        assert_eq!(url, "https://broker.example/v1/private/url");
        let body = GrantRequest {
            name: "sample-guest".into(),
            sha256: "cd".repeat(32),
        }
        .body();
        let header = authorization(&signer, &url, &body, 1_000);
        let auth =
            nostr::domain::parse_http_authorization(&header, "POST", &url, &body, 1_010).unwrap();
        assert_eq!(auth.pubkey, signer.pubkey());
        assert!(
            nostr::domain::parse_http_authorization(&header, "POST", &url, b"{}", 1_010).is_err()
        );
        assert!(
            nostr::domain::parse_http_authorization(
                &header,
                "POST",
                "https://other.example/v1/private/url",
                &body,
                1_010
            )
            .is_err()
        );
        assert!(
            nostr::domain::parse_http_authorization(&header, "POST", &url, &body, 1_100).is_err()
        );
    }
}
