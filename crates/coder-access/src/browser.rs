//! A browser as a client of a computer (#12264).
//!
//! A browser on openagents.com reaches a computer through the computer's
//! device room on `coder-sync`, admitted there by a room token
//! ([`crate::room_token`]) and named by a connection ID that starts with
//! `web-`. The computer serves such a connection nothing until the browser
//! proves it holds a key the computer granted ([`crate::Rights::browser`],
//! issued by [`crate::host::Host::approve_browser`] after the person at the
//! computer allowed it). The proof is a hello: a signed Nostr event of kind
//! [`HELLO_KIND`] with exactly the tags `["host", HOST_DEVICE]` and
//! `["conn", CONNECTION]`, empty content, signed within [`HELLO_WINDOW`]
//! seconds. Naming the connection makes a hello worthless on any other
//! socket.

use crate::{Code, Error, Result};
pub use nostr::domain::Event;
use nostr::domain::{RelaySigner, Tag};
use secp256k1::SecretKey;

/// The hello's kind (ephemeral range, never published).
pub const HELLO_KIND: u16 = 27_237;
/// How far a hello's time may be from the computer's clock, in seconds.
pub const HELLO_WINDOW: u64 = 60;
/// The connection-ID prefix `coder-sync` gives a room-token client.
pub const CONN_PREFIX: &str = "web-";

fn refused() -> Error {
    Error::new(
        Code::Forbidden,
        "This browser isn't allowed on this computer.",
    )
}

/// Sign a hello for `host` on connection `conn` (the browser does the same
/// in its own code; this is for native clients and tests).
pub fn sign_hello(secret: &SecretKey, host: &str, conn: &str, now: u64) -> Result<Event> {
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| refused())?;
    Ok(signer.sign(
        now,
        HELLO_KIND,
        vec![
            Tag::new(vec!["host".into(), host.into()]),
            Tag::new(vec!["conn".into(), conn.into()]),
        ],
        String::new(),
    ))
}

/// Check a hello for `host` on connection `conn` at `now`, and answer the
/// browser's key (lowercase hex). The caller then checks that key's grant.
pub fn verify_hello(event: &Event, host: &str, conn: &str, now: u64) -> Result<String> {
    if event.kind != HELLO_KIND
        || !event.content.is_empty()
        || event.tags.len() != 2
        || event.created_at > now.saturating_add(HELLO_WINDOW)
        || event.created_at.saturating_add(HELLO_WINDOW) < now
    {
        return Err(refused());
    }
    let tag = |name: &str| {
        event
            .tags
            .iter()
            .find(|tag| tag.name() == Some(name))
            .filter(|tag| tag.as_slice().len() == 2)
            .and_then(|tag| tag.value())
    };
    if tag("host") != Some(host) || tag("conn") != Some(conn) {
        return Err(refused());
    }
    event.validate_crypto().map_err(|_| refused())?;
    Ok(event.pubkey.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_proves_the_key_on_one_connection_for_a_minute() {
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        let hello = sign_hello(&key, "mac-1", "web-a", 1_000).unwrap();
        let public = verify_hello(&hello, "mac-1", "web-a", 1_030).unwrap();
        assert_eq!(public, hello.pubkey);
        assert!(verify_hello(&hello, "mac-1", "web-b", 1_030).is_err());
        assert!(verify_hello(&hello, "mac-2", "web-a", 1_030).is_err());
        assert!(verify_hello(&hello, "mac-1", "web-a", 1_061).is_err());
        assert!(verify_hello(&hello, "mac-1", "web-a", 939).is_err());
        let mut forged = hello.clone();
        forged.tags[1] = Tag::new(vec!["conn".into(), "web-b".into()]);
        assert!(verify_hello(&forged, "mac-1", "web-b", 1_030).is_err());
    }
}
