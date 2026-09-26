//! Authenticated private artifacts. Opening verifies the declaration and bytes,
//! never the enclosed application's authority or an external locator.

use std::{collections::HashSet, str::FromStr};

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::Value;

use crate::contracts::{self, ArtifactEnvelope, ArtifactRef, ContractError, RefusalCode};
use crate::domain::{Event, RelaySigner, Tag, validate_nip44_v2_content};

/// An original, signature-checked declaration decrypted by its author or recipient.
/// Private fields prevent callers from fabricating provenance.
#[derive(Clone)]
pub struct OpenEnvelope {
    event: Event,
    recipient: String,
    body: ArtifactEnvelope,
    inline: Option<Vec<u8>>,
}

impl OpenEnvelope {
    /// Original declaring signer, not the author of a forwarded reference.
    #[must_use]
    pub fn signer(&self) -> &str {
        &self.event.pubkey
    }
    /// Exact encrypted recipient.
    #[must_use]
    pub fn recipient(&self) -> &str {
        &self.recipient
    }
    /// Verified original event identity.
    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event.id
    }
    /// Original declaration retained for forwarding under separate disclosure authority.
    #[must_use]
    pub fn event(&self) -> &Event {
        &self.event
    }
    /// Reference checked against any inline bytes.
    #[must_use]
    pub fn artifact(&self) -> &ArtifactRef {
        &self.body.artifact
    }
    /// Canonical checked inline bytes, absent for externally stored content.
    #[must_use]
    pub fn inline_bytes(&self) -> Option<&[u8]> {
        self.inline.as_deref()
    }
    /// Requested retention and observational issuance; neither grants freshness.
    #[must_use]
    pub fn body(&self) -> &ArtifactEnvelope {
        &self.body
    }
    /// Verify caller-resolved bytes without fetching or widening disclosure.
    ///
    /// # Errors
    /// Returns an identity mismatch for bytes not named by this declaration.
    pub fn check_external(&self, bytes: &[u8]) -> Result<(), ContractError> {
        contracts::check_artifact_bytes(&self.body.artifact, bytes)
    }
}

/// Validate the signed outer envelope without decrypting its body.
///
/// # Errors
/// Refuses invalid signatures, ambiguous routes, extra tag fields, or non-v2 ciphertext.
pub fn admit(event: &Event) -> Result<(), ContractError> {
    event
        .validate_nip01_structure()
        .map_err(|_| malformed("artifact event"))?;
    event
        .validate_crypto()
        .map_err(|_| ContractError::new(RefusalCode::IdentityMismatch, "artifact signature"))?;
    let route = contracts::envelope_route(event)?;
    if event.tags.len() != 3 || event.tags.iter().any(|tag| tag.as_slice().len() != 2) {
        return Err(malformed("artifact envelope tags"));
    }
    XOnlyPublicKey::from_str(&route.recipient).map_err(|_| malformed("artifact recipient"))?;
    validate_nip44_v2_content(&event.content, "artifact")
        .map_err(|_| malformed("artifact ciphertext"))
}

/// Apply the same exact author/recipient ACL to stored and live declarations.
#[must_use]
pub fn visible(event: &Event, readers: &HashSet<String>) -> bool {
    admit(event).is_ok()
        && contracts::envelope_route(event).is_ok_and(|route| {
            readers.contains(&event.pubkey) || readers.contains(&route.recipient)
        })
}

/// Open and verify one original declaration with the author's or recipient's key.
///
/// # Errors
/// Refuses unrelated keys, invalid encryption, and mismatched artifact bytes.
pub fn open(event: &Event, secret: &SecretKey) -> Result<OpenEnvelope, ContractError> {
    admit(event)?;
    let route = contracts::envelope_route(event)?;
    let own = Keypair::from_secret_key(&Secp256k1::new(), secret)
        .x_only_public_key()
        .0
        .to_string();
    let peer = if own == event.pubkey {
        &route.recipient
    } else if own == route.recipient {
        &event.pubkey
    } else {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "artifact reader",
        ));
    };
    let peer = XOnlyPublicKey::from_str(peer).map_err(|_| malformed("artifact peer"))?;
    let key = crate::nip44::conversation_key(secret, &peer);
    let plaintext = crate::nip44::decrypt(&event.content, &key)
        .map_err(|_| ContractError::new(RefusalCode::IdentityMismatch, "artifact encryption"))?;
    let body = contracts::parse_envelope_body(plaintext.as_bytes())?;
    let value = contracts::parse_strict(plaintext.as_bytes())?;
    let inline = value
        .get("inline")
        .filter(|value| !value.is_null())
        .map(contracts::jcs)
        .transpose()?;
    Ok(OpenEnvelope {
        event: event.clone(),
        recipient: route.recipient,
        body,
        inline,
    })
}

/// Seal checked envelope bytes. The caller supplies fresh mailbox and nonce
/// entropy and separately authorizes disclosure; this function cannot grant it.
///
/// # Errors
/// Refuses malformed bodies, invalid routes, and unsupported encryption lengths.
pub fn seal(
    body: &Value,
    secret: &SecretKey,
    recipient: &XOnlyPublicKey,
    mailbox: &str,
    created_at: u64,
    nonce: [u8; 32],
) -> Result<Event, ContractError> {
    let bytes = contracts::jcs(body)?;
    contracts::parse_envelope_body(&bytes)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| malformed("artifact UTF-8"))?;
    let key = crate::nip44::conversation_key(secret, recipient);
    let ciphertext = crate::nip44::encrypt(text, &key, nonce)
        .map_err(|_| malformed("artifact encryption length"))?;
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| malformed("artifact signing key"))?;
    let event = signer.sign(
        created_at,
        contracts::ARTIFACT_ENVELOPE_KIND,
        vec![
            Tag::new(vec!["p".into(), recipient.to_string()]),
            Tag::new(vec!["h".into(), mailbox.into()]),
            Tag::new(vec!["t".into(), contracts::ARTIFACT_MARKER.into()]),
        ],
        ciphertext,
    );
    admit(&event)?;
    Ok(event)
}

fn malformed(field: &str) -> ContractError {
    ContractError::new(RefusalCode::Malformed, field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn secret(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }
    fn public(n: u8) -> XOnlyPublicKey {
        Keypair::from_secret_key(&Secp256k1::new(), &secret(n))
            .x_only_public_key()
            .0
    }
    fn body() -> Value {
        let inline = json!({"task": "private data"});
        let bytes = contracts::jcs(&inline).unwrap();
        json!({"v":"openagents.artifact-envelope.v1", "requires":[],
            "artifact":{"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),
                "media_type":"application/json","schema":"openagents.example.v1"},
            "inline":inline,"issued_at":100,"retain_until":200})
    }
    #[test]
    fn authenticated_roundtrip_and_private_provenance() {
        let event = seal(
            &body(),
            &secret(1),
            &public(2),
            &"aa".repeat(32),
            101,
            [3; 32],
        )
        .unwrap();
        for n in [1, 2] {
            let opened = open(&event, &secret(n)).unwrap();
            assert_eq!(opened.signer(), public(1).to_string());
            assert_eq!(opened.recipient(), public(2).to_string());
            assert_eq!(opened.event_id(), event.id);
            opened
                .check_external(opened.inline_bytes().unwrap())
                .unwrap();
            assert!(opened.check_external(b"changed").is_err());
            assert!(visible(&event, &HashSet::from([public(n).to_string()])));
        }
        assert!(open(&event, &secret(3)).is_err());
        assert!(!visible(&event, &HashSet::from([public(3).to_string()])));
        assert!(!visible(&event, &HashSet::new()));
        let mut changed = event.clone();
        changed.content.push('A');
        assert!(open(&changed, &secret(2)).is_err());
        let mut wrong = body();
        wrong["inline"]["task"] = json!("changed");
        assert!(
            seal(
                &wrong,
                &secret(1),
                &public(2),
                &"aa".repeat(32),
                101,
                [3; 32]
            )
            .is_err()
        );
    }
    #[test]
    fn signed_malformed_routes_refuse_before_decryption() {
        let event = seal(
            &body(),
            &secret(1),
            &public(2),
            &"aa".repeat(32),
            101,
            [3; 32],
        )
        .unwrap();
        let signer = RelaySigner::from_secret_hex(&secret(1).display_secret().to_string()).unwrap();
        let mut tags = event.tags.clone();
        tags.push(Tag::new(vec!["p".into(), public(3).to_string()]));
        let duplicate = signer.sign(101, event.kind, tags, event.content.clone());
        assert!(admit(&duplicate).is_err());
        assert!(!visible(&duplicate, &HashSet::from([event.pubkey.clone()])));
        let mut tags = event.tags.clone();
        tags[0] = Tag::new(vec!["p".into(), public(2).to_string(), "extra".into()]);
        assert!(admit(&signer.sign(101, event.kind, tags, event.content.clone())).is_err());
        assert!(admit(&signer.sign(101, event.kind, event.tags, body().to_string())).is_err());
    }
}
