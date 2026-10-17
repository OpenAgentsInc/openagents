//! Spent Nostr sign-in proofs (#11993).
//!
//! A signed NIP-98 event and the challenge it carries each work once. The
//! account service burns both in the same write that issues the session,
//! so a replayed event is refused even after a restart, and on every
//! instance that shares the store. An entry is kept until its challenge
//! expires; after that the challenge itself is refused, so the entry can
//! be forgotten.

use super::{Refusal, SessionBook, is_digest};

/// The most unexpired spent proofs the book holds at once. A challenge
/// stands minutes, so this bounds sign-ins in that window, not in total.
pub const NOSTR_PROOFS_LIVE_MAX: usize = 20_000;

impl SessionBook {
    /// Burn `ids` (event ids and challenge nonces, each 64 hex
    /// characters) until `until`. Refused with [`Refusal::ProofUsed`],
    /// burning nothing, when any of them is already spent.
    pub fn burn_nostr_proofs(&mut self, ids: &[&str], until: u64, now: u64) -> Result<(), Refusal> {
        self.nostr_proofs.retain(|_, expires| *expires >= now);
        if ids.iter().any(|id| !is_digest(id)) {
            return Err(Refusal::MalformedDigest("nostr-proof"));
        }
        if ids.iter().any(|id| self.nostr_proofs.contains_key(*id)) {
            return Err(Refusal::ProofUsed);
        }
        if self.nostr_proofs.len() + ids.len() > NOSTR_PROOFS_LIVE_MAX {
            return Err(Refusal::Unavailable);
        }
        for id in ids {
            self.nostr_proofs.insert((*id).to_string(), until.max(now));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proof_burns_once_and_is_forgotten_after_it_expires() {
        let mut book = SessionBook::new(3600, 3600);
        let event = "e".repeat(64);
        let nonce = "a".repeat(64);
        book.burn_nostr_proofs(&[&event, &nonce], 1_300, 1_000)
            .unwrap();
        assert_eq!(
            book.burn_nostr_proofs(&[&event], 1_300, 1_100),
            Err(Refusal::ProofUsed)
        );
        assert_eq!(
            book.burn_nostr_proofs(&["f".repeat(64).as_str(), &nonce], 1_300, 1_100),
            Err(Refusal::ProofUsed)
        );
        assert!(!book.nostr_proofs.contains_key(&"f".repeat(64)));
        assert!(book.burn_nostr_proofs(&["short"], 1_300, 1_100).is_err());
        book.burn_nostr_proofs(&["f".repeat(64).as_str()], 2_000, 1_301)
            .unwrap();
        assert_eq!(book.nostr_proofs.len(), 1);
    }

    #[test]
    fn stores_without_proofs_keep_their_shape() {
        let text = serde_json::to_string(&SessionBook::new(1, 1)).unwrap();
        assert!(!text.contains("nostr_proofs"));
    }
}
