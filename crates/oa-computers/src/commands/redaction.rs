//! Secret values are discarded after dispatch. Salted fingerprints let another
//! worker remove those exact values after a restart without storing the values
//! or an unfinished output tail. Uncommitted tails are reread from guest tmpfs.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Pattern {
    length: usize,
    hash: String,
}
fn hash(salt: &str, value: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"openagents.command.redaction.v1\0");
    digest.update(salt.as_bytes());
    digest.update([0]);
    digest.update(value);
    format!("{:x}", digest.finalize())
}
pub(super) fn patterns<'a>(salt: &str, values: impl Iterator<Item = &'a str>) -> Vec<Pattern> {
    values
        .filter(|v| !v.is_empty())
        .map(|value| Pattern {
            length: value.len(),
            hash: hash(salt, value.as_bytes()),
        })
        .collect()
}
pub(super) fn scrub(
    bytes: &[u8],
    salt: &str,
    patterns: &[Pattern],
    terminal: bool,
) -> (Vec<u8>, usize) {
    let max = patterns.iter().map(|p| p.length).max().unwrap_or(1);
    let end = if terminal {
        bytes.len()
    } else {
        bytes.len().saturating_sub(max - 1)
    };
    let mut clean = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < end {
        if let Some(pattern) = patterns.iter().find(|p| {
            at.checked_add(p.length)
                .is_some_and(|end| end <= bytes.len())
                && hash(salt, &bytes[at..at + p.length]) == p.hash
        }) {
            clean.extend_from_slice(b"[credential removed]");
            at += pattern.length;
        } else {
            clean.push(bytes[at]);
            at += 1;
        }
    }
    (clean, at)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_restart_needs_only_fingerprints_and_rereads_the_uncommitted_tail() {
        let value = "ghp_fixture-private-test-value-0123456789";
        let patterns = patterns("one-command", std::iter::once(value));
        let record = serde_json::to_vec(&patterns).unwrap();
        assert!(!String::from_utf8_lossy(&record).contains(value));
        let mut source = b"prefix ".to_vec();
        source.extend(value.as_bytes());
        source.extend(b" suffix");
        for split in 0..source.len() {
            let (mut clean, used) = scrub(&source[..split], "one-command", &patterns, false);
            let restored: Vec<Pattern> = serde_json::from_slice(&record).unwrap();
            clean.extend(scrub(&source[used..], "one-command", &restored, true).0);
            assert_eq!(clean, b"prefix [credential removed] suffix");
        }
    }
    #[test]
    fn binary_output_and_an_uncredentialed_command_keep_their_bytes() {
        let bytes = b"\0\xffbuild\n";
        assert_eq!(
            scrub(bytes, "id", &[], false),
            (bytes.to_vec(), bytes.len())
        );
    }
}
