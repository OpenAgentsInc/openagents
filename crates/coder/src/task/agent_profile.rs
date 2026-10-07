//! A workshop agent's `kind:0` profile
//! (`docs/verse/agent-identity-and-engrams.md`, "Identity").
//!
//! The host signs her profile with her own key and carries the owner's
//! NIP-OA `auth` tag from her record, so any reader can check who owns
//! her with `nostr::domain::verify_owner_attestation`. The host prepares
//! it when the owner makes or renews her attestation, and again when the
//! host opens her and the stored one is stale, and keeps it in
//! `agents/NAME/profile.json`. Nothing here publishes it; a later phase
//! sends it to the relays she is configured for.
//!
//! The profile follows Buzz's managed agent profile (a `kind:0` the agent
//! signs, with the `auth` tag), reimplemented here.

use std::path::PathBuf;

use nostr::domain::{Event, RelaySigner, Tag};

use super::agent::{self, Entry, Kind, Record, Store};

/// The file her signed profile is kept in, beside her record.
pub const PROFILE_FILE: &str = "profile.json";
/// The profile's kind.
pub const PROFILE_KIND: u16 = 0;

/// Where `store`'s profile is kept.
#[must_use]
pub fn path(store: &Store) -> PathBuf {
    store.dir().join(PROFILE_FILE)
}

/// Her profile's content: her display name, a fixed line about her, and
/// the NIP-24 `bot` flag. It carries no picture: her look names a
/// character, not an image.
#[must_use]
pub fn content(record: &Record) -> String {
    let name = record.display_name();
    let p = record.refer();
    serde_json::json!({
        "name": name,
        "display_name": name,
        "about": format!(
            "{name} is a {}. {} answers only {} owner.",
            record.role(),
            p.they_cap(),
            p.their()
        ),
        "bot": true,
    })
    .to_string()
}

/// Her profile signed with `key` at `now`, with the owner's `auth` tag.
///
/// # Errors
/// When her record has no attestation, `key` is not the key her record
/// names, or the signed event doesn't verify.
pub fn sign(record: &Record, key: &secp256k1::SecretKey, now: u64) -> Result<Event, String> {
    let pubkey = agent::public_hex(key);
    if record.pubkey.as_deref() != Some(pubkey.as_str()) {
        return Err(format!(
            "the key isn't the one {} record names",
            record.refer().their()
        ));
    }
    let attestation = record.attestation.as_ref().ok_or_else(|| {
        format!(
            "{} has no owner attestation to carry",
            record.refer().they()
        )
    })?;
    let secret: String = key
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let signer = RelaySigner::from_secret_hex(&secret).map_err(|e| e.to_string())?;
    let auth = Tag::new(vec![
        "auth".to_owned(),
        attestation.owner.clone(),
        attestation.conditions.clone(),
        attestation.signature.clone(),
    ]);
    let event = signer.sign(now, PROFILE_KIND, vec![auth], content(record));
    match nostr::domain::verify_owner_attestation(&event)? {
        Some(owner) if owner.owner_pubkey == attestation.owner => Ok(event),
        _ => Err(format!(
            "{} profile doesn't carry the owner's attestation",
            record.refer().their()
        )),
    }
}

/// Her stored profile, when the host prepared one.
///
/// # Errors
/// When the file exists and is not an event.
pub fn load(store: &Store) -> Result<Option<Event>, String> {
    let path = path(store);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("can't read {}: {e}", path.display())),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("{} is not an event: {e}", path.display()))
}

/// Whether `event` is her current profile: her key, her content, and the
/// attestation her record carries.
pub fn current(record: &Record, event: &Event) -> bool {
    let Some(attestation) = &record.attestation else {
        return false;
    };
    let auth = [
        "auth",
        attestation.owner.as_str(),
        attestation.conditions.as_str(),
        attestation.signature.as_str(),
    ];
    record.pubkey.as_deref() == Some(event.pubkey.as_str())
        && event.kind == PROFILE_KIND
        && event.content == content(record)
        && event
            .tags
            .iter()
            .any(|tag| tag.0.iter().map(String::as_str).eq(auth.iter().copied()))
}

/// Signs and keeps her profile when the stored one is missing or stale,
/// and journals that. Returns the new profile, or `None` when the stored
/// one is current or she has no key or attestation to sign with yet.
///
/// # Errors
/// When her key can't be read, the profile doesn't verify, or the file
/// can't be written.
pub fn refresh(store: &Store, record: &Record, now: u64) -> Result<Option<Event>, String> {
    super::sales::privacy::check_agent_copy(
        store,
        &serde_json::to_string(record).map_err(|_| "agent profile source serialization failed")?,
    )?;
    if record.pubkey.is_none() || record.attestation.is_none() {
        return Ok(None);
    }
    if load(store)
        .ok()
        .flatten()
        .is_some_and(|e| current(record, &e))
    {
        return Ok(None);
    }
    let Some(key) = store.key()? else {
        return Ok(None);
    };
    let event = sign(record, &key, now)?;
    let body = serde_json::to_vec_pretty(&event).map_err(|e| e.to_string())?;
    agent::private_dir(store.dir())?;
    let temp = store.dir().join(".profile.json.tmp");
    agent::write_private(&temp, &body)?;
    std::fs::rename(&temp, path(store))
        .map_err(|e| format!("can't write {}: {e}", path(store).display()))?;
    store.append(&Entry::new(
        now,
        Kind::Keyed,
        &format!(
            "{their} profile is signed with the owner's attestation, ready for {their} relays",
            their = store.refer().their()
        ),
    ))?;
    Ok(Some(event))
}

/// What linking an agent's key to its owner's XP trainer profile takes:
/// the agent's side, signed, and the key the owner's side lists.
#[derive(Clone, Debug)]
pub struct XpLink {
    /// The agent's NIP-XP key link (`13195`), signed with its key, naming
    /// the owner as its trainer.
    pub link: Event,
    /// The agent's public key, hex: the entry the owner's trainer profile
    /// (`13193`) adds to its `keys`, so the link holds both ways.
    pub key: String,
    /// The owner's public key, hex.
    pub trainer: String,
}

/// The agent's side of an XP key link to `owner` (hex), signed with its
/// key at `now`. Nothing is published: the owner adds [`XpLink::key`] to
/// their trainer profile and publishes both, and until both sides exist
/// the key counts toward no one.
///
/// # Errors
/// When the agent has no key here, its key isn't the one its record
/// names, `owner` isn't the owner that attested it, or the link doesn't
/// verify.
pub fn xp_link(store: &Store, owner: &str, now: u64) -> Result<XpLink, String> {
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    let p = record.refer();
    let key = store
        .key()?
        .ok_or_else(|| format!("{} key isn't on this computer", p.their()))?;
    let pubkey = agent::public_hex(&key);
    if record.pubkey.as_deref() != Some(pubkey.as_str()) {
        return Err(format!("the key isn't the one {} record names", p.their()));
    }
    if let Some(attestation) = &record.attestation
        && attestation.owner != owner
    {
        return Err(format!(
            "that owner key isn't the one that attested {}",
            p.them()
        ));
    }
    let unsigned = nostr::xp::link(&pubkey, Some(owner)).map_err(|e| e.to_string())?;
    let secret: String = key
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let signer = RelaySigner::from_secret_hex(&secret).map_err(|e| e.to_string())?;
    let link = signer.sign(now, unsigned.kind, unsigned.tags, unsigned.content);
    nostr::xp::parse_link(&link).map_err(|e| e.to_string())?;
    Ok(XpLink {
        link,
        key: pubkey,
        trainer: owner.to_string(),
    })
}
