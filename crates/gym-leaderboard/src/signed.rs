//! Who published a leaderboard: the signed Gym results publication.
//!
//! A digest proves the bytes are the ones the index names; it doesn't say
//! who wrote the index. A publisher signs a NIP-EVAL `3195` event
//! (`nips/openagents/NIP-EVAL.md`, "Gym results publication") stating the
//! leaderboard's digest, the commit its evidence was read at, and its
//! boards, and commits the event beside the leaderboard at
//! [`signature_path`]. A reader checks it with [`check`] after the
//! leaderboard itself verified:
//!
//! - no event is [`Signature::Unsigned`], not a failure: the digest checks
//!   still hold, and the viewer says "not signed";
//! - an event that fails its signature, names another digest, commit, or
//!   board list, or is signed by a key the reader hasn't pinned is
//!   [`Signature::Refused`], with the reason shown beside the numbers;
//! - otherwise it's [`Signature::Verified`], naming the publisher.
//!
//! The publishers a reader trusts are pinned in the reader's build
//! ([`PINNED`]), never read from the host that serves the files.

use serde::{Deserialize, Serialize};

use nostr::domain::Event;
use nostr::gym_results;

/// The largest signed event a reader accepts.
pub const MAX_SIGNATURE_BYTES: usize = 64 * 1024;

/// Publishers every reader trusts by default, as `(name, hex public key)`.
///
/// Empty until the OpenAgents publisher key is generated and its public
/// key is pinned here (see `docs/verse/gym-leaderboard.md`, "Signed
/// publication"). While it's empty, a signed event is refused as signed by
/// an unpinned key, and an unsigned publication reads "not signed".
pub const PINNED: &[(&str, &str)] = &[];

/// A publisher a reader trusts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publisher {
    /// The name shown: "signed by OpenAgents".
    pub name: String,
    /// The x-only public key, 64 lowercase hex.
    pub pubkey: String,
}

/// The pinned publishers, owned.
#[must_use]
pub fn pinned() -> Vec<Publisher> {
    PINNED
        .iter()
        .map(|(name, pubkey)| Publisher {
            name: (*name).to_owned(),
            pubkey: (*pubkey).to_owned(),
        })
        .collect()
}

/// Where the signed event for the leaderboard with content digest
/// `digest` is published, relative to the publication's base.
#[must_use]
pub fn signature_path(digest: &str) -> String {
    format!("signatures/{digest}.json")
}

/// What a reader knows about who published the leaderboard on screen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Signature {
    /// Not checked: the reader doesn't look, or hasn't yet.
    #[default]
    Unchecked,
    /// No signed event is published for this leaderboard.
    Unsigned,
    /// A pinned publisher signed exactly this leaderboard.
    Verified {
        /// The pinned publisher's name.
        publisher: String,
        /// The signer's key as `npub1…`.
        npub: String,
        /// The signed event's ID.
        event: String,
        /// The event's `created_at`, Unix seconds.
        signed_at: u64,
    },
    /// A signed event is present and doesn't hold for this leaderboard.
    Refused { reason: String },
}

impl Signature {
    /// The words a viewer shows: "signed by OpenAgents (npub1abcd…wxyz)",
    /// "not signed", or "signature refused: …"; `None` when unchecked.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Unchecked => None,
            Self::Unsigned => Some("not signed".to_owned()),
            Self::Verified {
                publisher, npub, ..
            } => Some(format!("signed by {publisher} ({})", short_npub(npub))),
            Self::Refused { reason } => Some(format!("signature refused: {reason}")),
        }
    }
}

fn short_npub(npub: &str) -> String {
    if npub.len() <= 20 {
        return npub.to_owned();
    }
    format!("{}…{}", &npub[..10], &npub[npub.len() - 6..])
}

/// Checks a signed event's bytes against the leaderboard the reader
/// already verified: its content `digest`, the index entry's `commit`, and
/// its board IDs in order, and the signer against `publishers`.
#[must_use]
pub fn check(
    bytes: &[u8],
    digest: &str,
    commit: Option<&str>,
    boards: &[String],
    publishers: &[Publisher],
) -> Signature {
    match verify(bytes, digest, commit, boards, publishers) {
        Ok(signature) => signature,
        Err(reason) => Signature::Refused { reason },
    }
}

fn verify(
    bytes: &[u8],
    digest: &str,
    commit: Option<&str>,
    boards: &[String],
    publishers: &[Publisher],
) -> Result<Signature, String> {
    if bytes.len() > MAX_SIGNATURE_BYTES {
        return Err(format!(
            "{} bytes, over the {MAX_SIGNATURE_BYTES}-byte bound",
            bytes.len()
        ));
    }
    let event: Event =
        serde_json::from_slice(bytes).map_err(|e| format!("not a Nostr event: {e}"))?;
    let publication = gym_results::parse_publication(&event).map_err(|e| e.to_string())?;
    if publication.digest != digest {
        return Err(format!(
            "it signs digest {}, not this leaderboard's",
            &publication.digest[..8]
        ));
    }
    let Some(commit) = commit else {
        return Err("the index names no commit to match".to_owned());
    };
    if publication.commit != commit {
        return Err(format!(
            "it signs commit {}, not the index's {}",
            &publication.commit[..7],
            commit.get(..7).unwrap_or(commit)
        ));
    }
    if publication.boards != boards {
        return Err("its board list differs from the leaderboard's".to_owned());
    }
    let npub = npub(&publication.publisher)?;
    let Some(publisher) = publishers
        .iter()
        .find(|p| p.pubkey == publication.publisher)
    else {
        return Err(format!(
            "signed by {}, which isn't a pinned publisher",
            short_npub(&npub)
        ));
    };
    Ok(Signature::Verified {
        publisher: publisher.name.clone(),
        npub,
        event: publication.id,
        signed_at: publication.created_at,
    })
}

fn npub(hex: &str) -> Result<String, String> {
    let mut key = [0_u8; 32];
    if hex.len() != 64 {
        return Err("bad signer key".to_owned());
    }
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| "bad signer key".to_owned())?;
    }
    Ok(nostr::nip19::encode_npub(&key))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use nostr::domain::RelaySigner;
    use sha2::{Digest, Sha256};

    /// A throwaway signer derived from a label; no key is stored.
    pub(crate) fn signer(label: &str) -> RelaySigner {
        let hex: String = Sha256::digest(label.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        RelaySigner::from_secret_hex(&hex).unwrap()
    }

    /// A signed event's bytes for `digest`, `commit`, and `boards`.
    pub(crate) fn signed_bytes(
        signer: &RelaySigner,
        digest: &str,
        commit: &str,
        boards: &[String],
    ) -> Vec<u8> {
        let parts = gym_results::publication(digest, commit, boards).unwrap();
        let event = signer.sign(1_790_000_000, parts.kind, parts.tags, parts.content);
        serde_json::to_vec(&event).unwrap()
    }

    const DIGEST: &str = "bf451beee1b755cf6e655920ac6f3124ae42f62c86e97bbe64677bcd5d8425aa";
    const COMMIT: &str = "6bfc94876de82ec6a8c1a684dc650cb76131c20a";

    fn boards() -> Vec<String> {
        vec!["a-board".into(), "b-board".into()]
    }

    fn trusted(signer: &RelaySigner) -> Vec<Publisher> {
        vec![Publisher {
            name: "OpenAgents".into(),
            pubkey: signer.pubkey().to_owned(),
        }]
    }

    #[test]
    fn a_pinned_publishers_event_verifies_and_names_them() {
        let key = signer("publisher");
        let bytes = signed_bytes(&key, DIGEST, COMMIT, &boards());
        let signature = check(&bytes, DIGEST, Some(COMMIT), &boards(), &trusted(&key));
        let Signature::Verified {
            publisher, npub, ..
        } = &signature
        else {
            panic!("{signature:?}")
        };
        assert_eq!(publisher, "OpenAgents");
        assert!(npub.starts_with("npub1"));
        let text = signature.text().unwrap();
        assert!(text.starts_with("signed by OpenAgents (npub1"), "{text}");
    }

    #[test]
    fn a_wrong_signer_digest_commit_or_board_list_is_refused() {
        let key = signer("publisher");
        let bytes = signed_bytes(&key, DIGEST, COMMIT, &boards());
        let other = signer("someone else");
        let refused = |s: Signature| match s {
            Signature::Refused { reason } => reason,
            other => panic!("{other:?}"),
        };
        assert!(
            refused(check(
                &bytes,
                DIGEST,
                Some(COMMIT),
                &boards(),
                &trusted(&other)
            ))
            .contains("isn't a pinned publisher")
        );
        assert!(
            refused(check(&bytes, DIGEST, Some(COMMIT), &boards(), &[]))
                .contains("isn't a pinned publisher")
        );
        let digest = "0".repeat(64);
        assert!(
            refused(check(
                &bytes,
                &digest,
                Some(COMMIT),
                &boards(),
                &trusted(&key)
            ))
            .contains("digest")
        );
        let commit = "1".repeat(40);
        assert!(
            refused(check(
                &bytes,
                DIGEST,
                Some(&commit),
                &boards(),
                &trusted(&key)
            ))
            .contains("commit")
        );
        assert!(refused(check(&bytes, DIGEST, None, &boards(), &trusted(&key))).contains("commit"));
        let fewer = vec!["a-board".to_string()];
        assert!(
            refused(check(&bytes, DIGEST, Some(COMMIT), &fewer, &trusted(&key)))
                .contains("board list")
        );
    }

    #[test]
    fn a_forged_signature_or_junk_is_refused() {
        let key = signer("publisher");
        let bytes = signed_bytes(&key, DIGEST, COMMIT, &boards());
        let mut event: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // Another key's pubkey over the same signature.
        event["pubkey"] = signer("forger").pubkey().into();
        let forged = serde_json::to_vec(&event).unwrap();
        let s = check(
            &forged,
            DIGEST,
            Some(COMMIT),
            &boards(),
            &trusted(&signer("forger")),
        );
        assert!(
            matches!(&s, Signature::Refused { reason } if reason.contains("signature")),
            "{s:?}"
        );
        let s = check(b"<html>", DIGEST, Some(COMMIT), &boards(), &trusted(&key));
        assert!(matches!(s, Signature::Refused { .. }));
        assert_eq!(Signature::Unsigned.text().unwrap(), "not signed");
        assert!(Signature::Unchecked.text().is_none());
    }
}
