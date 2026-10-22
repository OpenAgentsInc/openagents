//! Livehost deploy pointers (kind `30440`, `crates/livehost/src/pointer.rs`)
//! read as content deploys. Only the channels listed in [`TRUSTED`], signed
//! by the key named there, count.

use nostr::deploy::Env;
use nostr::domain::Event;

use crate::state::Pointer;

/// The pointer kind.
pub const KIND: u16 = 30_440;

/// The pointers the board shows: `d` tag, the key that signs it, and the
/// lane it belongs to. The staging key is the one the staging server
/// follows (`deploy/staging/render.py`, `LIVEHOST_STAGING_PUBKEY`). No
/// production server follows a livehost pointer yet.
pub const TRUSTED: &[(&str, &str, Env)] = &[(
    "openagents/staging",
    "9db9a1549ef4af943371415ee6c388e2b4a225b1f046cdcbeb8ceb2875ccd014",
    Env::Staging,
)];

/// The filter that fetches every trusted pointer.
#[must_use]
pub fn filter() -> serde_json::Value {
    let d: Vec<&str> = TRUSTED.iter().map(|(d, _, _)| *d).collect();
    let authors: Vec<&str> = TRUSTED.iter().map(|(_, key, _)| *key).collect();
    serde_json::json!({"kinds": [KIND], "#d": d, "authors": authors})
}

/// `event` as a content deploy, when it is a trusted pointer whose ID and
/// signature verify, dated no more than five minutes ahead of `now`.
#[must_use]
pub fn read(event: &Event, now: u64) -> Option<Pointer> {
    if event.kind != KIND || event.created_at > now + 300 {
        return None;
    }
    let d = event.tag_values("d").next()?;
    let (_, _, env) = TRUSTED
        .iter()
        .find(|(name, key, _)| *name == d && *key == event.pubkey)?;
    event.validate_crypto().ok()?;
    let manifest = event.tag_values("m").next()?;
    if manifest.len() != 64 || !manifest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(Pointer {
        env: *env,
        manifest: manifest.to_ascii_lowercase(),
        at: event.created_at,
    })
}
