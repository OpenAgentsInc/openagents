//! NIP-13 proof of work.
//!
//! An event's difficulty is the count of leading zero bits in its id.
//! A `nonce` tag carries the mining nonce and SHOULD commit a target
//! difficulty as its third entry: an honest miner declares the work
//! the id was mined against, so a reader can refuse a low-target id
//! that got lucky. NIP-13 is a draft and the relay does not mine or
//! require work, so nothing changes at admission and the NIP is not
//! added to the NIP-11 list.

use super::Event;
use super::hex::decode_lower_hex;

/// The number of leading zero bits in a 32-byte hex id — the
/// difficulty a mined event achieved.
#[must_use]
pub fn pow_difficulty(id: &str) -> u32 {
    let Ok(bytes) = decode_lower_hex::<32>(id, "id") else {
        return 0;
    };
    let mut total = 0_u32;
    for byte in bytes {
        let bits = byte.leading_zeros();
        total += bits;
        if bits != 8 {
            break;
        }
    }
    total
}

/// The target difficulty a `nonce` tag commits to, when present.
#[must_use]
pub fn nonce_commitment(event: &Event) -> Option<u32> {
    let tag = event.tags.iter().find(|tag| tag.name() == Some("nonce"))?;
    tag.0.get(2)?.parse::<u32>().ok()
}

/// Whether an event meets its committed target: the id's leading zero
/// bits reach the `nonce` tag's declared difficulty. An event with no
/// commitment is a MAY-reject — this reports `None` so the caller can
/// apply its own policy.
#[must_use]
pub fn meets_committed_target(event: &Event) -> Option<bool> {
    nonce_commitment(event).map(|target| pow_difficulty(&event.id) >= target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, "mining".to_string())
    }

    #[test]
    fn difficulty_counts_leading_zero_bits_and_the_commitment_gates() {
        // The pinned example: 36 leading zero bits.
        assert_eq!(
            pow_difficulty("000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d"),
            36
        );
        // 002f: 8 + 2 + 0 = 10 leading zeroes.
        assert_eq!(pow_difficulty(&format!("002f{}", "ab".repeat(30))), 10);
        assert_eq!(pow_difficulty(&"ff".repeat(32)), 0);
        assert_eq!(pow_difficulty("not hex"), 0);

        let mined = sign(vec![Tag::new(vec![
            "nonce".into(),
            "776797".into(),
            "0".into(),
        ])]);
        assert_eq!(nonce_commitment(&mined), Some(0));
        assert_eq!(meets_committed_target(&mined), Some(true));

        let uncommitted = sign(vec![Tag::new(vec!["nonce".into(), "1".into()])]);
        assert_eq!(nonce_commitment(&uncommitted), None);
        assert_eq!(meets_committed_target(&uncommitted), None);

        // Commit to an unreachable target and the event misses.
        let overcommitted = sign(vec![Tag::new(vec![
            "nonce".into(),
            "1".into(),
            "250".into(),
        ])]);
        assert_eq!(meets_committed_target(&overcommitted), Some(false));
    }
}
