//! Private knowledge delivery through an authenticated kind-3188 declaration
//! and separately encrypted exact document bytes. The file bundle is admitted
//! local artifact storage, not public NIP-KB publication or automatic fetching.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use nostr::domain::Event;
use nostr::{nip44, private_artifact};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Entry, Status, snapshot};

const MAX_BYTES: u64 = 3 * 1024 * 1024;

/// A signed encrypted declaration plus separately encrypted artifact storage.
/// The outer file contains no plaintext document or plaintext artifact digest.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub v: String,
    pub declaration: Event,
    pub ciphertext: String,
}

/// Verified original author and exact document; never automatic model admission.
pub struct Opened {
    pub entry: Entry,
    pub document: String,
    pub author: String,
    pub recipient: String,
    pub event: String,
}

/// An explicit local operator permission, bound to one declaration and digest.
/// It is not sender authority, a network grant, or permission to republish.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disclosure {
    pub v: String,
    pub event: String,
    pub author: String,
    pub digest: String,
    pub model_recipients: BTreeSet<String>,
}

/// Encrypt one entry for one recipient, using fresh independent entropy.
///
/// # Errors
/// Refuses malformed/withdrawn/oversized documents, invalid retention, or failed
/// cryptography. Public publication is not performed.
pub fn seal(
    document: &str,
    secret: &SecretKey,
    recipient: &XOnlyPublicKey,
    issued_at: u64,
    retain_until: u64,
) -> Result<Bundle, String> {
    let entry = Entry::parse(document)?;
    if document.len() > 1024 * 1024 || entry.version == 0 || entry.status == Status::Withdrawn {
        return Err("private entry has invalid status, version or size".into());
    }
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],
        "artifact":{"digest":entry.digest,"size":document.len(),"media_type":"text/markdown","schema":"openagents.kb-entry.v1"},
        "inline":null,"issued_at":issued_at,"retain_until":retain_until});
    let declaration = private_artifact::seal(
        &body,
        secret,
        recipient,
        &snapshot::hex(&snapshot::entropy()?),
        issued_at,
        snapshot::entropy()?,
    )
    .map_err(|e| e.to_string())?;
    let key = nip44::conversation_key(secret, recipient);
    let ciphertext =
        nip44::encrypt(document, &key, snapshot::entropy()?).map_err(|e| e.to_string())?;
    Ok(Bundle {
        v: "openagents.kb-private-bundle.v1".into(),
        declaration,
        ciphertext,
    })
}

/// Authenticate and decrypt an explicitly selected local delivery bundle.
/// Retention is a request, not an access expiry; no model or relay is contacted.
///
/// # Errors
/// Refuses unrelated readers, tampering, wrong schemas and mismatched document
/// bytes. Neither an event signature nor decryptability admits the entry.
pub fn open(bundle: &Bundle, secret: &SecretKey) -> Result<Opened, String> {
    if bundle.v != "openagents.kb-private-bundle.v1" || bundle.ciphertext.len() as u64 > MAX_BYTES {
        return Err("unsupported private bundle or ciphertext limit".into());
    }
    let opened = private_artifact::open(&bundle.declaration, secret).map_err(|e| e.to_string())?;
    if opened.artifact().schema.as_deref() != Some("openagents.kb-entry.v1")
        || opened.artifact().media_type != "text/markdown"
        || opened.inline_bytes().is_some()
        || opened.artifact().size > 1024 * 1024
    {
        return Err("private entry requires separately stored exact Markdown".into());
    }
    let own = Keypair::from_secret_key(&Secp256k1::new(), secret)
        .x_only_public_key()
        .0
        .to_string();
    let peer = if own == opened.signer() {
        opened.recipient()
    } else {
        opened.signer()
    };
    let key = nip44::conversation_key(
        secret,
        &XOnlyPublicKey::from_str(peer).map_err(|e| e.to_string())?,
    );
    let document = nip44::decrypt(&bundle.ciphertext, &key)
        .map_err(|_| "private document authentication failed".to_string())?;
    opened
        .check_external(document.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut entry = Entry::parse(&document)?;
    if entry.version == 0 || entry.status == Status::Withdrawn {
        return Err("private entry is withdrawn or unversioned".into());
    }
    entry.status = Status::Candidate;
    Ok(Opened {
        entry,
        document,
        author: opened.signer().into(),
        recipient: opened.recipient().into(),
        event: opened.event_id().into(),
    })
}

/// Read a bounded bundle without enabling inference or logging its contents.
///
/// # Errors
/// Refuses invalid JSON, duplicate fields, excess bytes or unverified delivery.
pub fn read(path: &Path, secret: &SecretKey) -> Result<Opened, String> {
    let bundle: Bundle = snapshot::read_json(path, MAX_BYTES)?;
    open(&bundle, secret)
}

impl Disclosure {
    /// Record a local operator's exact model-recipient selection.
    ///
    /// # Errors
    /// Refuses an empty or malformed recipient list. No broad wildcard exists.
    pub fn new(entry: &Opened, model_recipients: BTreeSet<String>) -> Result<Self, String> {
        let grant = Self {
            v: "openagents.kb-model-disclosure.v1".into(),
            event: entry.event.clone(),
            author: entry.author.clone(),
            digest: entry.entry.digest.clone(),
            model_recipients,
        };
        grant.check(entry, &grant.model_recipients)?;
        Ok(grant)
    }
    /// Check exact artifact identity and every model that will receive its text.
    /// This includes judges and stronger generators, not only the main generator.
    ///
    /// # Errors
    /// Refuses mismatched provenance, unlisted models or ambiguous recipient names.
    pub fn check(&self, entry: &Opened, required: &BTreeSet<String>) -> Result<(), String> {
        if self.v != "openagents.kb-model-disclosure.v1"
            || self.event != entry.event
            || self.author != entry.author
            || self.digest != entry.entry.digest
            || required.is_empty()
            || self.model_recipients.is_empty()
            || self.model_recipients.len() > 32
            || self.model_recipients.iter().any(|s| {
                s.is_empty()
                    || s.len() > 256
                    || s.contains('*')
                    || s.chars().any(char::is_whitespace)
            })
            || !required.is_subset(&self.model_recipients)
        {
            return Err("private knowledge lacks an exact model disclosure permission".into());
        }
        Ok(())
    }
    /// Read a local operator permission. This file grants no network authority.
    ///
    /// # Errors
    /// Refuses malformed or oversized permission files.
    pub fn read(path: &Path) -> Result<Self, String> {
        snapshot::read_json(path, 16 * 1024)
    }
}

/// The canonical document identity for retaining provenance outside model text.
#[must_use]
pub fn reference(entry: &Opened) -> serde_json::Value {
    json!({"event":entry.event,"author":entry.author,"recipient":entry.recipient,
        "digest":entry.entry.digest,"id":entry.entry.id,"version":entry.entry.version})
}

#[cfg(test)]
mod tests;
