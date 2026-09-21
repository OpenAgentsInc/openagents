//! Agent keys: one deterministic Nostr identity per enrolled username.
//!
//! An arena agent signs as the name it joined under. The manifest binds
//! `username → pubkey`; the secret is `sha256("voyager-agent-key:" +
//! username)`, re-derived at run time so no private key ever sits in a
//! manifest, a run directory, or this repository. Deterministic
//! derivation is the enrollment: two runs name the same identities, and
//! the trace can verify a signature without a keystore.
//!
//! This is the arena's key story, not the production one — a world that
//! admits outside operators needs real key custody, and that is a later
//! NIP-CAP question rather than a manifest field.

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The derivation tag. Changing it changes every agent's identity.
const DOMAIN: &str = "voyager-agent-key:";

/// The 32-byte secret an agent derives from its enrolled username.
#[must_use]
pub fn agent_secret(username: &str) -> [u8; 32] {
    let mut input = Vec::with_capacity(DOMAIN.len() + username.len());
    input.extend_from_slice(DOMAIN.as_bytes());
    input.extend_from_slice(username.as_bytes());
    Sha256::digest(&input).into()
}

/// The secret hex, for the signer.
#[must_use]
pub fn agent_secret_hex(username: &str) -> String {
    agent_secret(username)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The Nostr pubkey an agent signs under — the value a manifest's
/// `agents[].pubkey` records.
///
/// # Errors
///
/// Fails only if the derived secret cannot form a keypair, which a
/// sha256 output practically never does.
pub fn agent_pubkey(username: &str) -> Result<String> {
    let signer = nostr::domain::RelaySigner::from_secret_hex(&agent_secret_hex(username))
        .map_err(|error| Error::world(format!("agent key for {username:?}: {error}")))?;
    Ok(signer.pubkey().to_string())
}

/// A signer for one agent — its own events, its own guild channel.
///
/// # Errors
///
/// Fails only if the derived secret cannot form a keypair.
pub fn agent_signer(username: &str) -> Result<nostr::domain::RelaySigner> {
    nostr::domain::RelaySigner::from_secret_hex(&agent_secret_hex(username))
        .map_err(|error| Error::world(format!("agent key for {username:?}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_stable_and_distinct() {
        let first = agent_pubkey("ferro_1").unwrap();
        let again = agent_pubkey("ferro_1").unwrap();
        let other = agent_pubkey("lumen_1").unwrap();
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
