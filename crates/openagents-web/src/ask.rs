//! The web chat worker and its server-held visitor identity.
//!
//! Conversation commands run through `/chat/{uuid}` and its durable request
//! records. The former `/ask` streaming route is retired so it cannot bypass
//! those records or the visitor's shared answer lease. The worker still uses
//! the web policy and never starts Coder or controls a computer.

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::post;
use openagents_chat::basic_coder::{self, Door};
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};

/// The cookie that names a visitor: 32 random hex characters.
pub const COOKIE: &str = "oa_visitor";
/// The newest turns that one worker request includes.
pub const MAX_TURNS: usize = 8;
/// The longest context turn, in characters.
pub const MAX_TURN_CHARS: usize = 4_000;

/// Where questions are answered: the relay to the production chat worker.
pub trait Chat: Send + Sync {
    /// Return a door that signs with `secret`.
    fn door(&self, secret: SecretKey) -> Result<Box<dyn Door>, String>;
}

/// The OpenAgents chat worker, reached through `relay.openagents.com`.
pub struct Worker;

impl Chat for Worker {
    fn door(&self, secret: SecretKey) -> Result<Box<dyn Door>, String> {
        Ok(Box::new(basic_coder::Relay::new(
            basic_coder::RELAY,
            basic_coder::WORKER,
            secret,
        )?))
    }
}

pub(crate) fn routes() -> Router<crate::App> {
    Router::new().route("/ask", post(retired))
}

async fn retired() -> Response {
    let mut response = crate::chat_html::protect(crate::layout::problem(
        StatusCode::GONE,
        "This page has moved",
        "Start a chat from the homepage.",
        ("/", "Start a chat"),
    ));
    response.headers_mut().insert(
        header::LINK,
        HeaderValue::from_static("</>; rel=\"alternate\""),
    );
    response
}

/// Return a visitor cookie only when its shape matches a site-issued value.
pub(crate) fn visitor(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
        .filter(|value| value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Create a visitor identity without exposing a signing key.
pub(crate) fn new_visitor() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Derive a visitor's worker key from a shared server secret and its cookie.
pub(crate) fn key(salt: &[u8; 32], visitor: &str) -> SecretKey {
    let mut seed: [u8; 32] = Sha256::new()
        .chain_update(b"openagents-web-visitor-v1")
        .chain_update(salt)
        .chain_update(visitor.as_bytes())
        .finalize()
        .into();
    loop {
        if let Ok(key) = SecretKey::from_byte_array(seed) {
            return key;
        }
        seed = Sha256::digest(seed).into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_visitor_signs_with_a_key_of_its_own_that_the_salt_hides() {
        let salt = [7; 32];
        assert_eq!(key(&salt, "a"), key(&salt, "a"));
        assert_ne!(key(&salt, "a"), key(&salt, "b"));
        assert_ne!(key(&salt, "a"), key(&[8; 32], "a"));
    }

    #[test]
    fn only_a_cookie_this_site_could_set_names_a_visitor() {
        let mut headers = HeaderMap::new();
        let id = new_visitor();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("x=1; {COOKIE}={id}")).unwrap(),
        );
        assert_eq!(visitor(&headers), Some(id));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_visitor=../../etc"),
        );
        assert_eq!(visitor(&headers), None);
    }
}
