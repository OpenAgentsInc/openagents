//! Identifiers both ends derive without asking each other.
//!
//! Presence, hints, and activity summaries each need a mailbox (`h` tag) a
//! device can filter on. The host and the device derive it from their NIP-44
//! conversation key, which only they can compute, so a relay cannot link a
//! mailbox to a purpose. Terminal generations and workspace IDs are derived
//! from public values, because both are identifiers rather than secrets.

use std::str::FromStr;

use secp256k1::{SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// Which stream a mailbox carries: host to device, except
/// [`Stream::Nudges`], which a device sends its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Presence,
    Hints,
    Summaries,
    Nudges,
}

impl Stream {
    const fn label(self) -> &'static [u8] {
        match self {
            Self::Presence => b"openagents.host-mailbox.presence.v1\0",
            Self::Hints => b"openagents.host-mailbox.hints.v1\0",
            Self::Summaries => b"openagents.host-mailbox.summaries.v1\0",
            Self::Nudges => b"openagents.host-mailbox.nudges.v1\0",
        }
    }
}

/// The mailbox for one stream between `secret`'s key and `peer`. The host
/// passes its key and the device's public key; the device passes the reverse.
///
/// # Errors
/// Refuses a peer that is not an x-only public key.
pub fn mailbox(secret: &SecretKey, peer: &str, stream: Stream) -> Result<String> {
    let peer = XOnlyPublicKey::from_str(peer)
        .map_err(|_| Error::Config("peer is not an x-only public key".into()))?;
    let shared = nostr::nip44::conversation_key(secret, &peer);
    Ok(hex(&labeled(stream.label(), &[&shared])))
}

/// The NIP-TERM generation for a host run: a common ID bound to the host key
/// and the NIP-REACH generation, so a restart changes both together and a
/// client computes it from fresh presence.
#[must_use]
pub fn terminal_generation(host: &str, generation: u64) -> String {
    hex(&labeled(
        b"openagents.host-terminal-generation.v1\0",
        &[host.as_bytes(), &generation.to_be_bytes()],
    ))
}

/// The NIP-TERM workspace ID for a host-scoped workspace label. Labels, not
/// paths, cross the wire; the host maps a label to its root.
#[must_use]
pub fn workspace_id(label: &str) -> String {
    hex(&labeled(
        b"openagents.host-workspace.v1\0",
        &[label.as_bytes()],
    ))
}

fn labeled(label: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(label);
    for part in parts {
        hash.update(part);
    }
    hash.finalize().into()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }

    #[test]
    fn both_ends_derive_the_same_private_mailbox() {
        let (host, device) = (key(3), key(4));
        let host_key = coder_reach::pubkey(&host);
        let device_key = coder_reach::pubkey(&device);
        for stream in [
            Stream::Presence,
            Stream::Hints,
            Stream::Summaries,
            Stream::Nudges,
        ] {
            let from_host = mailbox(&host, &device_key, stream).unwrap();
            assert_eq!(from_host, mailbox(&device, &host_key, stream).unwrap());
            assert!(coder_reach::random_id(&from_host).is_ok());
        }
        assert_ne!(
            mailbox(&host, &device_key, Stream::Presence).unwrap(),
            mailbox(&host, &device_key, Stream::Hints).unwrap()
        );
        // A third key derives another mailbox for the same stream.
        assert_ne!(
            mailbox(&host, &device_key, Stream::Presence).unwrap(),
            mailbox(&key(5), &device_key, Stream::Presence).unwrap()
        );
        assert!(mailbox(&host, "npub", Stream::Presence).is_err());
    }

    #[test]
    fn generations_and_workspaces_are_common_ids() {
        let a = terminal_generation(&coder_reach::pubkey(&key(3)), 7);
        assert!(coder_pty::wire::is_common_id(&a));
        assert_ne!(a, terminal_generation(&coder_reach::pubkey(&key(3)), 8));
        assert!(coder_pty::wire::is_common_id(&workspace_id("checkout")));
        assert_ne!(workspace_id("checkout"), workspace_id("other"));
    }
}
