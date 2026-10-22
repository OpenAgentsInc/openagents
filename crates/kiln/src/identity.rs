//! Who wrote a Kiln event (#12451): a person's own key, or an agent key the
//! person authorized.
//!
//! An agent proves its owner with two tags on each event it signs:
//!
//! - the NIP-OA `auth` tag, `["auth", owner, "created_at<EXPIRY", sig]`;
//! - a `mandate` tag, `["mandate", JSON, sig]`: an owner-signed
//!   [`Mandate`] that names the agent key, the closed set of actions it may
//!   take, and the same expiry. The signature is BIP-340 over
//!   `SHA-256("openagents:agent-mandate:" || JSON)`.
//!
//! [`action_for_kind`] maps event kinds to actions. `git.propose` covers
//! patches, pull requests and their updates, comments, and issues; no action
//! covers NIP-34 status kinds, so an agent can never mark anything merged.
//!
//! The owner signs both with `kiln-git own`, which prints a [`Credential`]
//! the person pastes into the Kiln page. Read `docs/kiln/workstation.md`.

use std::collections::BTreeSet;

use nostr::domain::{Event, Tag};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The mandate schema.
pub const MANDATE_V: &str = "openagents.agent-mandate.v1";
/// The signing domain of a mandate.
const MANDATE_DOMAIN: &str = "openagents:agent-mandate:";
/// Propose code: patches, pull requests, comments, and issues.
pub const GIT_PROPOSE: &str = "git.propose";
/// Run `cargo` on a Kiln runner.
pub const KILN_TEST: &str = "kiln.test";
/// Every action a mandate may name.
pub const ACTIONS: &[&str] = &[GIT_PROPOSE, KILN_TEST];

/// The action an event of `kind` needs, or `None` when no mandate covers it.
#[must_use]
pub fn action_for_kind(kind: u16) -> Option<&'static str> {
    match kind {
        1_617 | 1_618 | 1_619 | 1_111 | 1_621 => Some(GIT_PROPOSE),
        25_900 => Some(KILN_TEST),
        _ => None,
    }
}

/// What an owner lets an agent key do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    pub v: String,
    /// The agent's key, 64 lowercase hex characters.
    pub agent: String,
    /// The owner's key, 64 lowercase hex characters.
    pub owner: String,
    /// Closed action words from [`ACTIONS`].
    pub actions: Vec<String>,
    /// Unix time after which nothing the agent signs counts.
    pub expires: u64,
}

fn digest(json: &str) -> [u8; 32] {
    Sha256::digest(format!("{MANDATE_DOMAIN}{json}").as_bytes()).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// What an owner hands an agent: the two tags it carries on every event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub auth: Vec<String>,
    pub mandate: Vec<String>,
}

impl Credential {
    /// The tags to add to an event.
    #[must_use]
    pub fn tags(&self) -> Vec<Tag> {
        vec![Tag::new(self.auth.clone()), Tag::new(self.mandate.clone())]
    }

    /// The mandate it carries, unchecked.
    #[must_use]
    pub fn mandate(&self) -> Option<Mandate> {
        serde_json::from_str(self.mandate.get(1)?).ok()
    }
}

/// Sign a mandate and the matching NIP-OA tag for `agent` with the owner's
/// secret, valid until `expires`.
///
/// # Errors
///
/// A plain reason for a bad agent key or action.
pub fn mint(
    owner: &SecretKey,
    agent: &str,
    actions: &[&str],
    expires: u64,
) -> Result<Credential, String> {
    if let Some(bad) = actions.iter().find(|a| !ACTIONS.contains(a)) {
        return Err(format!(
            "{bad} is not an action; use {}",
            ACTIONS.join(", ")
        ));
    }
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, owner);
    let owner_hex = keypair.x_only_public_key().0.to_string();
    let auth = nostr::domain::mint_owner_attestation(
        owner,
        agent,
        &nostr::agent_public::conditions_until(expires),
    )?;
    let mandate = Mandate {
        v: MANDATE_V.into(),
        agent: agent.to_string(),
        owner: owner_hex,
        actions: actions.iter().map(|a| (*a).to_string()).collect(),
        expires,
    };
    let json = serde_json::to_string(&mandate).map_err(|e| e.to_string())?;
    let signature = secp.sign_schnorr_with_aux_rand(&digest(&json), &keypair, &[0; 32]);
    Ok(Credential {
        auth: auth.tag().as_slice().to_vec(),
        mandate: vec!["mandate".into(), json, hex(&signature.to_byte_array())],
    })
}

/// The owner of `event`'s signer when it carries a valid `auth` tag and a
/// valid mandate that allows `event`'s kind at its time.
///
/// # Errors
///
/// A plain reason when either tag is missing or doesn't hold.
pub fn authorized_owner(event: &Event) -> Result<String, String> {
    let attestation =
        nostr::domain::verify_owner_attestation(event)?.ok_or("the event carries no auth tag")?;
    let tag = event
        .tags
        .iter()
        .find(|t| t.name() == Some("mandate"))
        .ok_or("the event carries no mandate")?;
    let [_, json, signature] = tag.as_slice() else {
        return Err("the mandate tag is malformed".into());
    };
    let mandate: Mandate =
        serde_json::from_str(json).map_err(|_| "the mandate is not readable".to_string())?;
    if mandate.v != MANDATE_V {
        return Err("the mandate has an unknown schema".into());
    }
    if mandate.agent != event.pubkey || mandate.owner != attestation.owner_pubkey {
        return Err("the mandate names another agent or owner".into());
    }
    if event.created_at >= mandate.expires {
        return Err("the mandate had expired".into());
    }
    let action = action_for_kind(event.kind).ok_or("no mandate covers this kind")?;
    if !mandate.actions.iter().any(|a| a == action) {
        return Err(format!("the mandate doesn't allow {action}"));
    }
    let owner = XOnlyPublicKey::from_byte_array(
        unhex::<32>(&mandate.owner).ok_or("the mandate's owner is not a key")?,
    )
    .map_err(|_| "the mandate's owner is not a key".to_string())?;
    let signature = Signature::from_byte_array(
        unhex::<64>(signature).ok_or("the mandate's signature is malformed")?,
    );
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest(json), &owner)
        .map_err(|_| "the mandate's signature does not verify".to_string())?;
    Ok(mandate.owner)
}

/// Check a credential pasted for `agent` at `now`: both signatures, the
/// agent key, and the expiry. Returns the mandate.
///
/// # Errors
///
/// A plain reason the person can act on.
pub fn verify_credential(
    credential: &Credential,
    agent: &str,
    now: u64,
) -> Result<Mandate, String> {
    let mandate = credential
        .mandate()
        .ok_or("this isn't an identity from kiln-git own")?;
    if mandate.agent != agent {
        return Err("this identity is for another key; make one for this page's key".into());
    }
    if now >= mandate.expires {
        return Err("this identity has expired; make a new one".into());
    }
    let [_, owner, conditions, auth_sig] = credential.auth.as_slice() else {
        return Err("this isn't an identity from kiln-git own".into());
    };
    let [_, json, mandate_sig] = credential.mandate.as_slice() else {
        return Err("this isn't an identity from kiln-git own".into());
    };
    let key = XOnlyPublicKey::from_byte_array(unhex::<32>(owner).ok_or("bad owner key")?)
        .map_err(|_| "bad owner key".to_string())?;
    let secp = Secp256k1::verification_only();
    let auth_digest: [u8; 32] =
        Sha256::digest(format!("nostr:agent-auth:{agent}:{conditions}").as_bytes()).into();
    let check = |sig: &str, digest: &[u8; 32]| {
        unhex::<64>(sig)
            .map(Signature::from_byte_array)
            .is_some_and(|sig| secp.verify_schnorr(&sig, digest, &key).is_ok())
    };
    if *owner != mandate.owner
        || !check(auth_sig, &auth_digest)
        || !check(mandate_sig, &digest(json))
    {
        return Err("this identity's signatures don't check out".into());
    }
    Ok(mandate)
}

/// Who wrote an event, as a reader shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Author {
    /// A key the repository lists as a maintainer.
    Maintainer(String),
    /// An agent key with a valid mandate from `owner`.
    Agent {
        agent: String,
        owner: String,
        owner_is_maintainer: bool,
    },
    /// Any other key: shown, but not vouched for.
    Unverified(String),
}

/// Read `event`'s author against the repository's maintainers.
#[must_use]
pub fn author(event: &Event, maintainers: &BTreeSet<String>) -> Author {
    if maintainers.contains(&event.pubkey) {
        return Author::Maintainer(event.pubkey.clone());
    }
    match authorized_owner(event) {
        Ok(owner) => Author::Agent {
            agent: event.pubkey.clone(),
            owner_is_maintainer: maintainers.contains(&owner),
            owner,
        },
        Err(_) => Author::Unverified(event.pubkey.clone()),
    }
}

/// `npub1...` for a hex key.
#[must_use]
pub fn npub(hex_key: &str) -> String {
    unhex::<32>(hex_key).map_or_else(|| hex_key.to_string(), |b| nostr::nip19::encode_npub(&b))
}

/// `npub1abcdefgh…uvwxyz`.
#[must_use]
pub fn short_npub(hex_key: &str) -> String {
    let n = npub(hex_key);
    if n.len() > 20 {
        format!("{}…{}", &n[..12], &n[n.len() - 6..])
    } else {
        n
    }
}

impl Author {
    /// The words a reader shows.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Maintainer(key) => format!("by {} (maintainer)", short_npub(key)),
            Self::Agent {
                owner,
                owner_is_maintainer,
                ..
            } => format!(
                "agent, authorized by {}{}",
                short_npub(owner),
                if *owner_is_maintainer {
                    " (maintainer)"
                } else {
                    ""
                }
            ),
            Self::Unverified(key) => format!("by {}, unverified author", short_npub(key)),
        }
    }

    /// Whether a maintainer wrote it or authorized the agent that did.
    #[must_use]
    pub fn vouched(&self) -> bool {
        matches!(
            self,
            Self::Maintainer(_)
                | Self::Agent {
                    owner_is_maintainer: true,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cj::Key;

    const NOW: u64 = 1_792_540_800;

    fn owner() -> SecretKey {
        SecretKey::from_byte_array([7; 32]).unwrap()
    }

    fn patch_tags(credential: &Credential) -> Vec<Tag> {
        let mut tags = crate::patch::patch_tags(
            &format!("30617:{}:openagents", "a".repeat(64)),
            &"a".repeat(64),
            &"b".repeat(40),
            &["atif".to_string()],
        );
        tags.extend(credential.tags());
        tags
    }

    #[test]
    fn an_agent_patch_names_its_owner() {
        let agent = Key::from_bytes([9; 32]).unwrap();
        let credential = mint(&owner(), agent.pubkey(), &[GIT_PROPOSE], NOW + 3_600).unwrap();
        let mandate = credential.mandate().unwrap();
        let patch = agent.sign(1_617, patch_tags(&credential), "diff".into(), NOW);
        assert_eq!(authorized_owner(&patch).unwrap(), mandate.owner);
        let none = BTreeSet::new();
        let read = author(&patch, &none);
        assert!(
            read.label().starts_with("agent, authorized by npub1"),
            "{}",
            read.label()
        );
        assert!(!read.vouched());
        let listed = BTreeSet::from([mandate.owner.clone()]);
        assert!(author(&patch, &listed).vouched());
        assert!(author(&patch, &listed).label().ends_with("(maintainer)"));
    }

    #[test]
    fn a_mandate_never_covers_status_or_late_events() {
        let agent = Key::from_bytes([9; 32]).unwrap();
        let credential = mint(&owner(), agent.pubkey(), &[GIT_PROPOSE], NOW + 3_600).unwrap();
        let merged = agent.sign(1_631, credential.tags(), String::new(), NOW);
        assert!(authorized_owner(&merged).is_err());
        let late = agent.sign(1_617, patch_tags(&credential), String::new(), NOW + 7_200);
        assert!(authorized_owner(&late).is_err());
        let job = agent.sign(25_900, credential.tags(), String::new(), NOW);
        assert!(authorized_owner(&job).unwrap_err().contains("kiln.test"));
        assert!(mint(&owner(), agent.pubkey(), &["git.merge"], NOW).is_err());
    }

    #[test]
    fn a_pasted_credential_checks_for_its_key() {
        let agent = Key::from_bytes([9; 32]).unwrap();
        let credential = mint(
            &owner(),
            agent.pubkey(),
            &[GIT_PROPOSE, KILN_TEST],
            NOW + 60,
        )
        .unwrap();
        let text = serde_json::to_string(&credential).unwrap();
        let back: Credential = serde_json::from_str(&text).unwrap();
        assert!(verify_credential(&back, agent.pubkey(), NOW).is_ok());
        assert!(verify_credential(&back, agent.pubkey(), NOW + 60).is_err());
        assert!(verify_credential(&back, &"c".repeat(64), NOW).is_err());
        let mut forged = back.clone();
        forged.mandate[1] =
            forged.mandate[1].replace(&format!("{}", NOW + 60), &format!("{}", NOW + 600));
        assert!(verify_credential(&forged, agent.pubkey(), NOW).is_err());
    }

    #[test]
    fn a_borrowed_or_altered_mandate_fails() {
        let agent = Key::from_bytes([9; 32]).unwrap();
        let thief = Key::from_bytes([10; 32]).unwrap();
        let credential = mint(&owner(), agent.pubkey(), &[GIT_PROPOSE], NOW + 3_600).unwrap();
        let stolen = thief.sign(1_617, patch_tags(&credential), String::new(), NOW);
        assert!(authorized_owner(&stolen).is_err());
        let mut altered = credential.clone();
        altered.mandate[1] = altered.mandate[1].replace("git.propose", "kiln.test");
        let patch = agent.sign(25_900, altered.tags(), String::new(), NOW);
        assert!(authorized_owner(&patch).is_err());
        let plain = agent.sign(1_617, vec![], String::new(), NOW);
        assert!(matches!(
            author(&plain, &BTreeSet::new()),
            Author::Unverified(_)
        ));
        assert!(
            author(&plain, &BTreeSet::new())
                .label()
                .ends_with("unverified author")
        );
    }
}
