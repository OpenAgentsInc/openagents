//! NIP-AGENT owner-selected public gateway receipts (kind 3204).
//!
//! Publication is explicit: the owner signs one chosen receipt. Its seal
//! checks the receipt's bytes; the signature attributes the claim to the
//! owner, without proving remote execution or hardware attestation.

use nostr::agent_public::{self, Definition};
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};

use receipts::ExecutionReceipt;

pub const KIND: u16 = nostr::kinds::AGENT_GATEWAY_RECEIPT;
pub const SCHEMA: &str = "openagents.agent-gateway-receipt.v1";
pub const MAX_BYTES: usize = 262_144;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub v: String,
    pub requires: Vec<String>,
    pub receipt: ExecutionReceipt,
}

fn address(definition: &Definition) -> String {
    format!(
        "{}:{}:{}",
        agent_public::DEFINITION_KIND,
        definition.agent,
        definition.id
    )
}

/// Sign one selected, sealed receipt as the current agent owner. Review
/// its operational metadata before calling this function: all of it becomes
/// public, including account references when the receipt contains them.
pub fn event(
    owner: &RelaySigner,
    definition: &Event,
    receipt: &ExecutionReceipt,
    at: u64,
) -> Result<Event, String> {
    let definition = agent_public::read_definition(definition)?;
    if definition.revoked || owner.pubkey() != definition.owner {
        return Err("Only the agent's current owner can publish its receipts.".into());
    }
    receipt.verify().map_err(|e| e.to_string())?;
    let content = serde_json::to_string(&Publication {
        v: SCHEMA.into(),
        requires: vec![],
        receipt: receipt.clone(),
    })
    .map_err(|e| e.to_string())?;
    if content.len() > MAX_BYTES {
        return Err("The receipt is too large to publish.".into());
    }
    let tags = vec![
        Tag::new(vec!["p".into(), definition.agent.clone()]),
        Tag::new(vec!["a".into(), address(&definition)]),
        Tag::new(vec!["x".into(), receipt.digest.clone()]),
        Tag::new(vec!["alt".into(), "Owner-published gateway receipt".into()]),
    ];
    Ok(owner.sign(at, KIND, tags, content))
}

/// Verify the signature, agent and current owner, and the execution
/// receipt's schema and seal. A retired definition authorizes nothing.
pub fn parse(event: &Event, definition: &Definition) -> Result<Publication, String> {
    event.validate_structure().map_err(|e| e.to_string())?;
    event.validate_crypto().map_err(|e| e.to_string())?;
    if event.kind != KIND
        || event.content.len() > MAX_BYTES
        || definition.revoked
        || event.pubkey != definition.owner
    {
        return Err("That receipt was not published by this agent's current owner.".into());
    }
    let publication: Publication =
        serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    if publication.v != SCHEMA || !publication.requires.is_empty() {
        return Err("Unknown receipt publication version or feature.".into());
    }
    let one = |name: &str, value: &str| {
        let mut tags = event
            .tags
            .iter()
            .filter(|t| t.as_slice().first().is_some_and(|n| n == name));
        tags.next().is_some_and(|t| t.as_slice() == [name, value]) && tags.next().is_none()
    };
    if !one("p", &definition.agent)
        || !one("a", &address(definition))
        || !one("x", &publication.receipt.digest)
    {
        return Err("The receipt names another agent or receipt.".into());
    }
    publication.receipt.verify().map_err(|e| e.to_string())?;
    Ok(publication)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::mint_owner_attestation;

    fn signer(byte: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
    }

    #[test]
    fn selected_receipt_verifies_only_for_its_agent_and_current_owner() {
        let (owner, agent, stranger) = (signer(1), signer(2), signer(3));
        let tag = mint_owner_attestation(&secp_secret(), agent.pubkey(), "created_at<1800003600")
            .unwrap()
            .tag();
        let definition = agent_public::definition_event(
            &agent,
            &tag,
            owner.pubkey(),
            &"ab".repeat(32),
            "https://example.org",
            "{}",
            1_800_000_000,
        )
        .unwrap();
        let read = agent_public::read_definition(&definition).unwrap();
        let mut receipt = ExecutionReceipt::for_attempt("http", "request-1", 1, "sha256:request");
        receipt.attempt_id = "attempt-1".into();
        receipt.seal();
        let published = event(&owner, &definition, &receipt, 1_800_000_001).unwrap();
        assert_eq!(
            parse(&published, &read).unwrap().receipt.digest,
            receipt.digest
        );
        assert!(event(&stranger, &definition, &receipt, 1_800_000_001).is_err());
        let mut other = read.clone();
        other.id = "cd".repeat(32);
        assert!(parse(&published, &other).is_err());
        other = read.clone();
        other.owner = stranger.pubkey().into();
        assert!(parse(&published, &other).is_err());
        other = read.clone();
        other.revoked = true;
        assert!(parse(&published, &other).is_err());
        let mut tampered = receipt.clone();
        tampered.outcome = receipts::Outcome::Answered;
        assert!(event(&owner, &definition, &tampered, 1_800_000_001).is_err());
        let mut bad = published.clone();
        bad.content = bad.content.replace("attempt-1", "attempt-2");
        // Even re-signing a changed receipt cannot repair its seal.
        bad = owner.sign(bad.created_at, bad.kind, bad.tags, bad.content);
        assert!(parse(&bad, &read).is_err());
        for (field, value) in [
            ("v", serde_json::json!("unknown")),
            ("requires", serde_json::json!(["future-feature"])),
        ] {
            let mut content: serde_json::Value = serde_json::from_str(&published.content).unwrap();
            content[field] = value;
            let changed = owner.sign(
                published.created_at,
                KIND,
                published.tags.clone(),
                content.to_string(),
            );
            assert!(parse(&changed, &read).is_err());
        }
        let mut duplicate = published.clone();
        duplicate
            .tags
            .push(Tag::new(vec!["p".into(), agent.pubkey().into()]));
        duplicate = owner.sign(
            duplicate.created_at,
            duplicate.kind,
            duplicate.tags,
            duplicate.content,
        );
        assert!(parse(&duplicate, &read).is_err());
    }

    fn secp_secret() -> secp256k1::SecretKey {
        secp256k1::SecretKey::from_byte_array([1; 32]).unwrap()
    }
}
