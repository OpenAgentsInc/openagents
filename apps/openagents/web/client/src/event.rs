//! A NIP-01 event signed with the browser's key (#12264): the hello a
//! computer checks with `coder_access::browser::verify_hello`.

use anyhow::{Result, anyhow};
use k256::schnorr::SigningKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The x-only public key, lowercase hex.
pub fn public(key: &SigningKey) -> String {
    hex(&key.verifying_key().to_bytes())
}

/// Sign `[0, pubkey, created_at, kind, tags, ""]` (NIP-01) with `key`;
/// `aux` is BIP-340's fresh auxiliary randomness.
pub fn sign(
    key: &SigningKey,
    kind: u64,
    tags: Value,
    created_at: u64,
    aux: &[u8; 32],
) -> Result<Value> {
    let pubkey = public(key);
    let canonical = serde_json::to_string(&json!([0, pubkey, created_at, kind, tags, ""]))?;
    let id: [u8; 32] = Sha256::digest(canonical.as_bytes()).into();
    let signature = key
        .sign_raw(&id, aux)
        .map_err(|_| anyhow!("Couldn't sign with this browser's key."))?;
    Ok(json!({
        "id": hex(&id),
        "pubkey": pubkey,
        "created_at": created_at,
        "kind": kind,
        "tags": tags,
        "content": "",
        "sig": hex(&signature.to_bytes()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The computer checks the browser's hello with the `nostr` crate:
    /// same ID, same BIP-340 signature.
    #[test]
    fn a_browser_hello_verifies_as_a_nostr_event() {
        let key = SigningKey::from_bytes(&[7u8; 32]).unwrap();
        let signed = sign(
            &key,
            27_237,
            json!([["host", "mac-1"], ["conn", "web-0a1b"]]),
            1_800_000_000,
            &[9u8; 32],
        )
        .unwrap();
        let event: nostr::domain::Event = serde_json::from_value(signed.clone()).unwrap();
        event.validate_crypto().unwrap();
        assert_eq!(event.pubkey, public(&key));
        let mut changed = event.clone();
        changed.created_at += 1;
        assert!(changed.validate_crypto().is_err());
    }
}
