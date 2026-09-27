//! The NIP-PL push gateway and its device client.
//!
//! The relay's PL executor never holds an APNs or FCM credential. It posts
//! the closed relay-delivery request `{v, endpoint_grant, request_id,
//! expires_at}` with NIP-98 authorization to this gateway, which resolves the
//! opaque grant to a native device token it holds and sends the platform's
//! fixed wake constant. See `docs/deployment/push-gateway.md`.
//!
//! - [`wire`] holds the closed request and response bodies both sides share.
//! - [`nip98`] signs and verifies the NIP-98 authorization, bound to the
//!   method, the URL path, and the body hash.
//! - `server` (feature `server`) is the gateway: two listeners, encrypted
//!   token custody, idempotent delivery, and the APNs and FCM senders.
//! - `client` (feature `client`) is the device side: register a native
//!   token, obtain a delivery capability for one relay, and publish, renew,
//!   or revoke the kind 30350 lease encrypted to the relay's executor key.

pub mod nip98;
pub mod wire;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

/// Current Unix time in seconds.
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Lowercase hexadecimal encoding.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// SHA-256 as lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}
