//! NIP-AGENT private export and explicit, pinned selection. No execution authority.
use crate::{
    contracts::{self, ContractError, RefusalCode},
    domain::Event,
    private_artifact,
};
use agent_contract::{Profile, SCHEMA};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::json;
fn invalid() -> ContractError {
    ContractError::new(RefusalCode::IdentityMismatch, "agent profile")
}
/// Publish an inert definition using an explicitly chosen recipient and signing key.
pub fn publish(
    profile: &Profile,
    key: &SecretKey,
    recipient: &XOnlyPublicKey,
    mailbox: &str,
    created_at: u64,
    retain_until: u64,
    nonce: [u8; 32],
) -> Result<Event, ContractError> {
    profile.validate().map_err(|_| invalid())?;
    let inline = serde_json::to_value(profile).map_err(|_| invalid())?;
    let bytes = contracts::jcs(&inline)?;
    if bytes.len() > 24 * 1024 {
        return Err(invalid());
    }
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":SCHEMA},"inline":inline,"issued_at":created_at,"retain_until":retain_until});
    private_artifact::seal(&body, key, recipient, mailbox, created_at, nonce)
}
/// Select only the original signer, definition, revision, and exact artifact the caller requested.
pub fn consume(
    event: &Event,
    key: &SecretKey,
    signer: &str,
    id: &str,
    revision: u64,
    digest: &str,
) -> Result<Profile, ContractError> {
    let open = private_artifact::open(event, key)?;
    if open.signer() != signer
        || open.artifact().schema.as_deref() != Some(SCHEMA)
        || open.artifact().digest != digest
    {
        return Err(invalid());
    }
    let bytes = open.inline_bytes().ok_or_else(invalid)?;
    if bytes.len() > 24 * 1024 {
        return Err(invalid());
    }
    let profile: Profile = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    profile.validate().map_err(|_| invalid())?;
    if profile.definition.id != id || profile.definition.revision != revision {
        return Err(invalid());
    }
    Ok(profile)
}
#[cfg(test)]
mod tests {
    use super::*;
    use agent_contract::{Access, AgentDefinition, AgentSpec};
    use secp256k1::{Keypair, Secp256k1};
    fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }
    fn public(n: u8) -> XOnlyPublicKey {
        Keypair::from_secret_key(&Secp256k1::new(), &key(n))
            .x_only_public_key()
            .0
    }
    #[test]
    fn private_producer_and_pinned_consumer_refuse_wrong_authority_and_bytes() {
        let p = Profile::new(AgentDefinition {
            id: "a".repeat(64),
            revision: 1,
            deleted: false,
            spec: AgentSpec {
                name: "Review".into(),
                description: String::new(),
                instructions: "Find defects".into(),
                runtime: "codex".into(),
                model: None,
                access: Access::ReadOnly,
                fallbacks: vec![],
                autonomy: None,
            },
        })
        .unwrap();
        let event = publish(&p, &key(1), &public(2), &"b".repeat(64), 100, 200, [3; 32]).unwrap();
        assert_eq!(event.kind, 3188);
        let opened = private_artifact::open(&event, &key(2)).unwrap();
        let digest = &opened.artifact().digest;
        assert_eq!(
            consume(
                &event,
                &key(2),
                &public(1).to_string(),
                &p.definition.id,
                1,
                digest
            )
            .unwrap(),
            p
        );
        assert!(
            consume(
                &event,
                &key(3),
                &public(1).to_string(),
                &p.definition.id,
                1,
                digest
            )
            .is_err()
        );
        assert!(
            consume(
                &event,
                &key(2),
                &public(3).to_string(),
                &p.definition.id,
                1,
                digest
            )
            .is_err()
        );
        assert!(
            consume(
                &event,
                &key(2),
                &public(1).to_string(),
                &p.definition.id,
                2,
                digest
            )
            .is_err()
        );
        assert!(
            consume(
                &event,
                &key(2),
                &public(1).to_string(),
                &p.definition.id,
                1,
                &"c".repeat(64)
            )
            .is_err()
        );
        let mut changed = event;
        changed.content.push('A');
        assert!(
            consume(
                &changed,
                &key(2),
                &public(1).to_string(),
                &p.definition.id,
                1,
                digest
            )
            .is_err()
        );
    }
}
