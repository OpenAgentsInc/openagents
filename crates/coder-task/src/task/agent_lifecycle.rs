//! A workshop agent's lifecycle past her making
//! (`docs/verse/agent-identity-and-engrams.md`, "Lifecycle"): rotate her
//! key, retire her, move her to another of the owner's computers, and
//! export or import a snapshot of her.
//!
//! **Rotate** ([`rotate`]). A new key replaces hers in the same key store,
//! and every engram head is signed again with it and encrypted under the
//! new conversation key with her owner, so the owner reads all of it as
//! before. The new key waits in a slot of its own (`Store::store_next_key`)
//! and the new heads in `engrams.next/` until both read back and every head
//! verifies for her and for her owner; only then does the new key replace
//! the old one, which is gone from that moment. The owner signs a lineage
//! record ([`Lineage`]) that links the old key to the new one, kept in
//! `agents/NAME/lineage.jsonl`, and attests the new key afresh. A NIP-SOV
//! identity key change is a new identity with lineage, so nothing she was
//! granted carries over: her journal says the owner re-delegates. When
//! relay sync is on, the owner asks her relays to archive the old key with
//! a NIP-IA `kind:9035` request that names the new one in `replaced-by`
//! ([`archive_request`]), and the next sync pass publishes her heads, her
//! profile, and her relay list under the new key.
//!
//! **Retire** ([`retire`]). Her jobs go off, her key is deleted from the
//! store, and her record says retired. Her journal, memory, and engram
//! files stay, and the owner still decrypts every engram with the owner key
//! (`agent_engrams::owner_read`), because her record keeps her public key.
//! When relay sync is on and the owner key is at hand, the owner asks her
//! relays to archive her key with a NIP-IA `kind:9035` request.
//!
//! **Move** ([`mark_moved`]). The owner's other computer receives her
//! record, key, and engrams by the owner's own transfer, never a relay;
//! this one then marks her moved, names the other computer as her
//! controller and custodian, and refuses her requests, jobs, and sync, so
//! one computer runs her at a time.
//!
//! **Export and import** ([`export`], [`import`]). A snapshot holds her
//! definition and, on the owner's choice, her `core` profile or all of her
//! memory as plaintext; never her key. Importing one makes a new agent
//! with a new key, and her memory is encrypted under it.
//!
//! NIP-IA archive requests with owner authority and the snapshot semantics
//! (no key in a snapshot; an import mints a new key and encrypts memory
//! again) follow Buzz's identity archive and agent snapshot designs,
//! reimplemented here.

use std::path::Path;

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::engram::{self, Body, EventParams, Pair};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use super::agent::{self, Entry, Kind, Record, State, Store};
use super::agent_engrams::{self, EngramStore, Opened};
use super::agent_memory::{Memory, MemoryEntry};
use super::agent_sync::{self, Connector, Settings};

/// Her lineage records, beside her record.
pub const LINEAGE_FILE: &str = "lineage.jsonl";
/// A lineage record's schema.
pub const LINEAGE_SCHEMA: &str = "openagents.agent-lineage.v1";
/// What a lineage signature's preimage starts with, so it never means
/// anything else.
pub const LINEAGE_DOMAIN: &str = "openagents:agent-lineage:v1:";
/// A snapshot's schema.
pub const SNAPSHOT_SCHEMA: &str = "openagents.agent-snapshot.v1";
/// How long the owner's credential in an archive request lasts, in
/// seconds: a NIP-IA request is fresh for two minutes, so ten covers a
/// slow relay.
pub const ARCHIVE_CREDENTIAL_SECS: u64 = 600;
/// The longest reason a rotation records.
pub const REASON_MAX: usize = 256;
/// Where the new heads wait during a rotation.
const NEXT_DIR: &str = "engrams.next";
/// Where the old heads wait while the new ones take their place.
const OLD_DIR: &str = "engrams.old";

/// The owner's signed record that her key `old` became `new`
/// (`openagents.agent-lineage.v1`). The owner signs
/// `SHA-256(LINEAGE_DOMAIN || agent || ":" || old || ":" || new || ":" ||
/// at || ":" || reason)` with BIP-340; every field before the reason is
/// free of colons, so the preimage reads one way only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub schema: String,
    pub v: u32,
    /// Her name.
    pub agent: String,
    /// Her key before, 64 lowercase hex characters.
    pub old: String,
    /// Her key after.
    pub new: String,
    /// Why, in the owner's words, screened.
    pub reason: String,
    /// When, Unix seconds.
    pub at: u64,
    /// The owner's public key.
    pub owner: String,
    /// The owner's signature, 128 lowercase hex characters.
    pub signature: String,
}

fn lineage_digest(agent: &str, old: &str, new: &str, at: u64, reason: &str) -> [u8; 32] {
    sha2::Sha256::digest(format!("{LINEAGE_DOMAIN}{agent}:{old}:{new}:{at}:{reason}").as_bytes())
        .into()
}

impl Lineage {
    /// The owner's lineage record of `agent`'s key `old` becoming `new`.
    ///
    /// # Errors
    /// When the agent name or a key is malformed.
    pub fn sign(
        owner: &SecretKey,
        agent: &str,
        old: &str,
        new: &str,
        reason: &str,
        at: u64,
    ) -> Result<Self, String> {
        if !agent::valid_name(agent) {
            return Err(format!("`{agent}` is not an agent name"));
        }
        for key in [old, new] {
            agent::decode_hex32(key)
                .filter(|_| key.bytes().all(|b| !b.is_ascii_uppercase()))
                .ok_or("a lineage key is 64 lowercase hex characters")?;
        }
        if old == new {
            return Err("a lineage record links two different keys".into());
        }
        let secp = secp256k1::Secp256k1::signing_only();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, owner);
        let digest = lineage_digest(agent, old, new, at, reason);
        let signature = secp.sign_schnorr_no_aux_rand(&digest, &keypair);
        Ok(Self {
            schema: LINEAGE_SCHEMA.into(),
            v: 1,
            agent: agent.into(),
            old: old.into(),
            new: new.into(),
            reason: reason.into(),
            at,
            owner: agent::public_hex(owner),
            signature: signature.to_string(),
        })
    }

    /// Checks the schema and the owner's signature.
    ///
    /// # Errors
    /// Says what does not hold.
    pub fn verify(&self) -> Result<(), String> {
        if self.schema != LINEAGE_SCHEMA || self.v != 1 {
            return Err("a lineage record this host doesn't read".into());
        }
        let owner = agent::decode_hex32(&self.owner)
            .and_then(|b| XOnlyPublicKey::from_byte_array(b).ok())
            .ok_or("the lineage record's owner is not a key")?;
        let text = &self.signature;
        if text.len() != 128 {
            return Err("the lineage signature is malformed".into());
        }
        let mut bytes = [0u8; 64];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                .map_err(|_| "the lineage signature is malformed")?;
        }
        let digest = lineage_digest(&self.agent, &self.old, &self.new, self.at, &self.reason);
        secp256k1::Secp256k1::verification_only()
            .verify_schnorr(
                &secp256k1::schnorr::Signature::from_byte_array(bytes),
                &digest,
                &owner,
            )
            .map_err(|_| "the owner's lineage signature does not verify".to_string())
    }
}

/// Every lineage record of `store`'s agent, oldest first. A line that
/// does not read is skipped.
///
/// # Errors
/// When the file exists and cannot be read.
pub fn lineage(store: &Store) -> Result<Vec<Lineage>, String> {
    let path = store.dir().join(LINEAGE_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("can't read {}: {e}", path.display())),
    };
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn append_lineage(store: &Store, record: &Lineage) -> Result<(), String> {
    use std::io::Write;
    let path = store.dir().join(LINEAGE_FILE);
    let mut line = serde_json::to_vec(record).map_err(|e| e.to_string())?;
    line.push(b'\n');
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&path)
        .and_then(|mut file| file.write_all(&line).and_then(|()| file.flush()))
        .map_err(|e| format!("can't append to {}: {e}", path.display()))
}

/// The owner's NIP-IA `kind:9035` request that a relay archive `target`,
/// an agent key the owner owns, for `reason` (`rotated` or `retired`),
/// naming `replaced_by` when a rotation made one. The owner signs it and
/// carries a fresh request-borne credential: the owner's NIP-OA signature
/// of `target` with `created_at<` [`ARCHIVE_CREDENTIAL_SECS`] from `now`.
///
/// # Errors
/// When a key is malformed or the request doesn't parse as NIP-IA.
pub fn archive_request(
    owner: &SecretKey,
    target: &str,
    reason: &str,
    replaced_by: Option<&str>,
    now: u64,
) -> Result<Event, String> {
    let credential = agent::sign_attestation(
        owner,
        target,
        &format!("created_at<{}", now + ARCHIVE_CREDENTIAL_SECS),
    )?;
    let mut tags = vec![
        Tag::new(vec!["-".into()]),
        Tag::new(vec!["p".into(), target.into()]),
        Tag::new(vec!["reason".into(), reason.into()]),
    ];
    if let Some(new) = replaced_by {
        tags.push(Tag::new(vec!["replaced-by".into(), new.into()]));
    }
    tags.push(Tag::new(vec![
        "auth".into(),
        credential.owner,
        credential.conditions,
        credential.signature,
    ]));
    let content = match replaced_by {
        Some(_) => "The owner rotated this workshop agent's key.",
        None => "The owner retired this workshop agent.",
    };
    let secret: String = owner
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let event = RelaySigner::from_secret_hex(&secret)
        .map_err(|e| e.to_string())?
        .sign(now, 9_035, tags, content.into());
    nostr::domain::parse_identity_archive_request(&event, now)?;
    Ok(event)
}

/// What one relay said to an archive request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sent {
    pub url: String,
    pub accepted: bool,
    pub message: String,
}

/// Sends `request` to each of `relays`, connected and authenticated as
/// the owner, since NIP-70 lets only its author publish it, and journals
/// each answer.
pub fn send_archive(
    store: &Store,
    owner: &SecretKey,
    request: &Event,
    relays: &[String],
    connector: &dyn Connector,
    now: u64,
) -> Vec<Sent> {
    let mut sent = Vec::new();
    for url in relays {
        let answer = connector
            .connect(url, owner, None)
            .and_then(|mut relay| relay.publish(request));
        let one = match answer {
            Ok((accepted, message)) => Sent {
                url: url.clone(),
                accepted,
                message,
            },
            Err(why) => Sent {
                url: url.clone(),
                accepted: false,
                message: why,
            },
        };
        let text = if one.accepted {
            format!(
                "{url} took the owner's NIP-IA archive request for {} old key",
                store.refer().their()
            )
        } else {
            format!(
                "{url} didn't take the owner's NIP-IA archive request: {}",
                one.message
            )
        };
        let _ = store.append(&Entry::new(now, Kind::Control, &text));
        sent.push(one);
    }
    sent
}

/// Her relays, when relay sync is on.
fn relays(store: &Store) -> Vec<String> {
    Settings::load(store)
        .map(|s| agent_sync::configured(&s.memory_relays))
        .unwrap_or_default()
}

fn owner_matches(record: &Record, owner: &SecretKey) -> Result<(), String> {
    match &record.attestation {
        Some(attestation) if attestation.owner != agent::public_hex(owner) => Err(format!(
            "that owner key isn't the one that attested {}",
            record.refer().them()
        )),
        _ => Ok(()),
    }
}

/// What a rotation did.
#[derive(Clone, Debug)]
pub struct Rotated {
    pub old: String,
    pub new: String,
    /// Engram heads signed and encrypted again.
    pub engrams: usize,
    pub lineage: Lineage,
    /// The owner's NIP-IA request to archive the old key, when sync is on.
    pub archive: Option<Event>,
    /// Her relays, when sync is on.
    pub relays: Vec<String>,
}

/// Rotates `store`'s agent to a new key: see the module's documentation.
/// `owner` is the owner key that attested her; the new attestation lasts
/// until `expires_at`. Nothing reaches a relay here: send
/// [`Rotated::archive`] with [`send_archive`], and the next sync pass
/// publishes under the new key.
///
/// # Errors
/// When she has no key or attestation, is retired or moved, her key or
/// engram store can't be read, the owner key is not hers, or a step
/// fails; until the new key replaces the old one, a failure leaves her as
/// she was.
pub fn rotate(
    store: &Store,
    screen: &secret_screen::Screen,
    owner: &SecretKey,
    reason: &str,
    expires_at: u64,
    now: u64,
) -> Result<Rotated, String> {
    super::sales::privacy::check_agent_copy(store, reason)?;
    let mut record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    let p = record.refer();
    let (they, them, their) = p.words();
    match record.state {
        State::Retired => {
            return Err(format!("{they} is retired, so {they} has no key to rotate"));
        }
        State::Moved => {
            return Err(format!(
                "{they} moved to another computer; rotate {them} there"
            ));
        }
        _ => {}
    }
    let old_pub = record
        .pubkey
        .clone()
        .ok_or_else(|| format!("{they} has no key to rotate"))?;
    if record.attestation.is_none() {
        return Err(format!(
            "{their} key has no owner attestation; attest it before rotating"
        ));
    }
    owner_matches(&record, owner)?;
    if expires_at <= now || expires_at - now > agent::ATTESTATION_MAX {
        return Err("an attestation expires within a year".into());
    }
    let reason = agent::ascii(&agent::screen(reason.trim()));
    let reason: String = reason
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(REASON_MAX)
        .collect();
    store.custody(&record)?;
    let old_key = store
        .key()?
        .ok_or_else(|| format!("{their} key is missing"))?;
    let old_heads: Vec<engram::Engram> = match EngramStore::read(store, screen) {
        Opened::Ready(engrams) => engrams.heads().into_iter().cloned().collect(),
        Opened::Skipped(_) => Vec::new(),
        Opened::Unreadable(why) => {
            return Err(format!(
                "{their} engram store can't be read, so {they} isn't rotated: {why}"
            ));
        }
    };
    let owner_pub: XOnlyPublicKey = agent::public_hex(owner)
        .parse()
        .map_err(|_| "the owner key is not a key".to_string())?;

    // 1. Her next key, in a slot of its own, read back.
    let fresh = SecretKey::new(&mut secp256k1::rand::rng());
    let undo = |why: String| -> String {
        store.delete_next_key();
        let _ = std::fs::remove_dir_all(store.dir().join(NEXT_DIR));
        why
    };
    store.store_next_key(&fresh).map_err(undo)?;
    let new_pub = agent::public_hex(&fresh);
    let new_x: XOnlyPublicKey = new_pub
        .parse()
        .map_err(|_| undo(format!("{their} new key is not a key")))?;

    // 2. Every head signed again and encrypted for the new pair, each
    //    verified for her and for her owner before anything moves.
    let next = store.dir().join(NEXT_DIR);
    let _ = std::fs::remove_dir_all(&next);
    agent::private_dir(&next).map_err(undo)?;
    let hers = Pair::for_agent(&fresh, &owner_pub);
    let owners = Pair::for_owner(owner, &new_x);
    for head in &old_heads {
        let params = EventParams {
            created_at: head.created_at,
            nonce: secp256k1::rand::random::<[u8; 32]>(),
            aux: secp256k1::rand::random::<[u8; 32]>(),
            alt: true,
        };
        let event = engram::build_event(&fresh, &owner_pub, &head.body, &params)
            .map_err(|e| undo(format!("{}: {e}", head.slug().as_str())))?;
        let mine = engram::validate_and_decrypt(&event, &hers)
            .map_err(|e| undo(format!("{}: {e}", head.slug().as_str())))?;
        let theirs = engram::validate_and_decrypt(&event, &owners)
            .map_err(|e| undo(format!("{}: {e}", head.slug().as_str())))?;
        if mine.body != head.body || theirs.body != head.body {
            return Err(undo(format!(
                "{} doesn't read back the same",
                head.slug().as_str()
            )));
        }
        let json = serde_json::to_vec(&event).map_err(|e| undo(e.to_string()))?;
        let name = format!("{}.json", mine.d);
        let temp = next.join(format!(".{name}.tmp"));
        agent::write_private(&temp, &json).map_err(undo)?;
        std::fs::rename(&temp, next.join(&name))
            .map_err(|e| undo(format!("can't write {name}: {e}")))?;
    }

    // 3. The owner's lineage record and a fresh attestation, both checked.
    let lineage =
        Lineage::sign(owner, store.name(), &old_pub, &new_pub, &reason, now).map_err(undo)?;
    lineage.verify().map_err(undo)?;
    let attestation = agent::sign_attestation(owner, &new_pub, &format!("created_at<{expires_at}"))
        .map_err(undo)?;
    agent::verify_attestation(&new_pub, &attestation, now).map_err(undo)?;
    let relays = relays(store);
    let archive = if relays.is_empty() {
        None
    } else {
        Some(archive_request(owner, &old_pub, "rotated", Some(&new_pub), now).map_err(undo)?)
    };

    // 4. The switch: the new heads take the old ones' place, the new key
    //    replaces the old one, and her record names it.
    let current = agent_engrams::dir_of(store);
    let old_dir = store.dir().join(OLD_DIR);
    let _ = std::fs::remove_dir_all(&old_dir);
    let had_dir = current.exists();
    if had_dir {
        std::fs::rename(&current, &old_dir)
            .map_err(|e| undo(format!("can't set {their} old engrams aside: {e}")))?;
    }
    let put_back = |why: String| -> String {
        let _ = std::fs::remove_dir_all(&current);
        if had_dir {
            let _ = std::fs::rename(&old_dir, &current);
        }
        undo(why)
    };
    std::fs::rename(&next, &current)
        .map_err(|e| put_back(format!("can't put {their} new engrams in place: {e}")))?;
    store.replace_key(&fresh).map_err(|why| {
        let _ = store.replace_key(&old_key);
        put_back(why)
    })?;
    record.pubkey = Some(new_pub.clone());
    record.attestation = Some(attestation);
    record.fill_identity();
    if let Err(why) = store.save(&record) {
        let _ = store.replace_key(&old_key);
        return Err(put_back(why));
    }
    // From here the old key is gone.
    store.delete_next_key();
    let _ = std::fs::remove_dir_all(&old_dir);
    append_lineage(store, &lineage)?;
    // The last pass was for the old key; the next sweep runs one.
    let _ = std::fs::remove_file(store.dir().join(agent_sync::STATUS_FILE));

    // 5. The new store, as the host opens it.
    let opened = EngramStore::open(store, screen, now);
    let Opened::Ready(_) = &opened else {
        return Err(format!(
            "{their} new engram store doesn't open after the rotation: {opened:?}"
        ));
    };
    let view = agent_engrams::owner_read(store, owner)?;
    if !view.problems.is_empty() {
        return Err(format!(
            "the owner can't read every new engram: {}",
            view.problems.join("; ")
        ));
    }
    let _ = super::agent_profile::refresh(store, &record, now);
    let _ = store.append(&Entry::new(
        now,
        Kind::Keyed,
        &format!(
            "the owner rotated {their} key: {old_pub} is now {new_pub}, kept in the {}; {} engrams \
             signed and encrypted again under the new key; reason: {}",
            store.custody_kind(),
            old_heads.len(),
            if reason.is_empty() {
                "none given"
            } else {
                reason.as_str()
            }
        ),
    ));
    let _ = store.append(&Entry::new(
        now,
        Kind::Control,
        &format!(
            "grants made to {their} old key don't carry over; the owner delegates again any \
             {they} needs"
        ),
    ));
    Ok(Rotated {
        old: old_pub,
        new: new_pub,
        engrams: old_heads.len(),
        lineage,
        archive,
        relays,
    })
}

/// What a retirement did.
#[derive(Clone, Debug)]
pub struct Retired {
    pub key_deleted: bool,
    /// The owner's NIP-IA request to archive her key, when sync is on and
    /// the owner key was at hand.
    pub archive: Option<Event>,
    /// Her relays, when sync is on.
    pub relays: Vec<String>,
}

/// Retires `store`'s agent: her jobs off, her key deleted from her key
/// store, her record retired, and her journal, memory, and engrams kept.
/// With `owner`, the owner key that attested her, and relay sync on, it
/// also builds the owner's NIP-IA archive request for her key; send it
/// with [`send_archive`]. Retiring her again deletes nothing more.
///
/// # Errors
/// When she doesn't exist, `owner` is not her owner, her key store
/// refuses, or her record can't be written.
pub fn retire(store: &Store, owner: Option<&SecretKey>, now: u64) -> Result<Retired, String> {
    let mut record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    if let Some(owner) = owner {
        owner_matches(&record, owner)?;
    }
    let _ = super::agent_jobs::Jobs::new(store.clone()).disable_all();
    let relays = relays(store);
    let archive = match (owner, &record.pubkey) {
        (Some(owner), Some(pubkey)) if !relays.is_empty() && record.state != State::Retired => {
            Some(archive_request(owner, pubkey, "retired", None, now)?)
        }
        _ => None,
    };
    let key_deleted = store.delete_key()?;
    let p = record.refer();
    let (they, _, their) = p.words();
    record.state = State::Retired;
    record.attestation = None;
    store.save(&record)?;
    store.append(&Entry::new(
        now,
        Kind::Control,
        &format!(
            "retired by the owner; {}; {their} journal and engrams stay, and the owner key \
             reads them",
            if key_deleted {
                format!("{their} key is deleted")
            } else {
                format!("{they} had no key here")
            }
        ),
    ))?;
    if !relays.is_empty() && archive.is_none() && key_deleted {
        let _ = store.append(&Entry::new(
            now,
            Kind::Control,
            &format!(
                "relay sync was on, but no owner key was at hand to ask {their} relays to \
                 archive {their} key with NIP-IA"
            ),
        ));
    }
    Ok(Retired {
        key_deleted,
        archive,
        relays,
    })
}

/// The owner's NIP-IA archive request for a retired agent's key and her
/// relays, when relay sync is on, for a host that retired her without
/// the owner key. `None` when sync is off or she never had a key.
///
/// # Errors
/// When she isn't retired or `owner` is not the owner her roles name.
pub fn retired_archive(
    store: &Store,
    owner: &SecretKey,
    now: u64,
) -> Result<Option<(Event, Vec<String>)>, String> {
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    if record.state != State::Retired {
        return Err(format!("{} isn't retired", record.refer().they()));
    }
    let authority = record
        .roles
        .as_ref()
        .map(|roles| roles.authority.clone())
        .unwrap_or_default();
    if !authority.is_empty() && authority != agent::public_hex(owner) {
        return Err(format!(
            "that owner key isn't the one that attested {}",
            record.refer().them()
        ));
    }
    let relays = relays(store);
    match &record.pubkey {
        Some(pubkey) if !relays.is_empty() => Ok(Some((
            archive_request(owner, pubkey, "retired", None, now)?,
            relays,
        ))),
        _ => Ok(None),
    }
}

/// Marks `store`'s agent moved to the owner's computer whose host key is
/// `to` (64 lowercase hex characters): it becomes her controller and
/// custodian, her jobs go off, and this computer refuses her requests and
/// sync from now on. Her key and files stay here; the owner copies them to
/// the other computer first.
///
/// # Errors
/// When `to` is not a key, she doesn't exist or is retired, or her record
/// can't be written.
pub fn mark_moved(store: &Store, to: &str, now: u64) -> Result<Record, String> {
    if to.len() != 64
        || agent::decode_hex32(to).is_none()
        || to.bytes().any(|b| b.is_ascii_uppercase())
    {
        return Err("name the other computer by its host key, 64 lowercase hex characters".into());
    }
    let mut record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    let p = record.refer();
    if record.state == State::Retired {
        return Err(format!("{} is retired", p.they()));
    }
    let _ = super::agent_jobs::Jobs::new(store.clone()).disable_all();
    record.fill_identity();
    if let Some(roles) = &mut record.roles {
        roles.controller = to.into();
        roles.custodian = to.into();
    }
    record.state = State::Moved;
    store.save(&record)?;
    store.append(&Entry::new(
        now,
        Kind::Control,
        &format!(
            "moved by the owner to the computer {to}, which runs {} now; this one runs nothing \
             of {}",
            p.them(),
            p.theirs()
        ),
    ))?;
    Ok(record)
}

/// How much of her memory a snapshot carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryChoice {
    /// Her definition only.
    None,
    /// Her definition and `core` profile.
    Core,
    /// Her definition, `core`, and every memory entry, as plaintext.
    All,
}

impl MemoryChoice {
    /// `none`, `core`, or `all`.
    ///
    /// # Errors
    /// For any other word.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word {
            "none" => Ok(Self::None),
            "core" => Ok(Self::Core),
            "all" => Ok(Self::All),
            _ => Err("--memory is none, core, or all".into()),
        }
    }
}

/// A snapshot of an agent without her key (`openagents.agent-snapshot.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub v: u32,
    pub exported_at: u64,
    /// Her name when exported.
    pub name: String,
    pub definition: agent::Definition,
    pub charter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_role: Option<coder_host::access::crew::JobRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_charter: Option<coder_host::access::crew::Charter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sales_model_scope: Option<super::agent::SalesModelScope>,
    pub look: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub route: String,
    pub desk: u32,
    /// Her key's public half when exported, for lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    /// Her owner's public key when exported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub memory: MemoryChoice,
    /// Her `core` profile, with [`MemoryChoice::Core`] or `All`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core: Option<String>,
    /// Every memory entry as plaintext, with [`MemoryChoice::All`] only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<MemoryEntry>,
}

/// A snapshot of `store`'s agent, with `memory` of her memory. Her `core`
/// comes from her engram store, read with her key here or, with `owner`,
/// with the owner key; her entries come from her working memory. Her key
/// is never in it.
///
/// # Errors
/// When she doesn't exist, or `core` was asked for and neither key reads
/// her engrams.
pub fn export(
    store: &Store,
    screen: &secret_screen::Screen,
    memory: MemoryChoice,
    owner: Option<&SecretKey>,
    now: u64,
) -> Result<Snapshot, String> {
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    let core = if memory == MemoryChoice::None {
        None
    } else {
        read_core(store, screen, owner)?
    };
    let entries = if memory == MemoryChoice::All {
        Memory::new(store.clone(), screen.clone()).entries()?
    } else {
        Vec::new()
    };
    let snapshot = Snapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        v: 1,
        exported_at: now,
        name: record.name.clone(),
        definition: record.definition(),
        charter: record.charter.clone(),
        job_role: record.job_role,
        crew_charter: record.crew_charter.clone(),
        sales_model_scope: record.sales_model_scope.clone(),
        look: record.look.clone(),
        route: record.route.clone(),
        desk: record.desk,
        pubkey: record.pubkey.clone(),
        owner: record.attestation.as_ref().map(|a| a.owner.clone()),
        memory,
        core,
        entries,
    };
    super::sales::privacy::check_agent_copy(
        store,
        &serde_json::to_string(&snapshot).map_err(|_| "agent snapshot serialization failed")?,
    )?;
    Ok(snapshot)
}

fn read_core(
    store: &Store,
    screen: &secret_screen::Screen,
    owner: Option<&SecretKey>,
) -> Result<Option<String>, String> {
    if let Some(owner) = owner {
        let view = agent_engrams::owner_read(store, owner)?;
        return Ok(view.heads.iter().find_map(|head| match &head.body {
            Body::Core { profile, .. } => Some(profile.clone()),
            Body::Memory { .. } => None,
        }));
    }
    match EngramStore::read(store, screen) {
        Opened::Ready(engrams) => Ok(engrams.core().map(str::to_string)),
        Opened::Skipped(why) | Opened::Unreadable(why) => Err(format!(
            "{their} core is encrypted and {their} key can't read it here ({why}); export \
             with --owner-key FILE",
            their = store.refer().their()
        )),
    }
}

/// Makes `store`'s agent, who must not exist yet, from `snapshot`: her
/// definition, a new key of her own, and, with `owner`, the owner's
/// attestation until `expires_at`, her `core`, and her memory entries
/// encrypted under the new key. Without `owner` her entries wait in her
/// working memory until the owner attests her. With `owner` and the
/// snapshot's key, the owner's lineage record links the two.
///
/// # Errors
/// When she exists, the snapshot isn't v1, or a step fails.
pub fn import(
    store: &Store,
    screen: &secret_screen::Screen,
    snapshot: &Snapshot,
    workspace: &Path,
    owner: Option<&SecretKey>,
    expires_at: u64,
    now: u64,
) -> Result<Record, String> {
    {
        let root = store
            .dir()
            .parent()
            .and_then(Path::parent)
            .ok_or("agent host root is unavailable")?;
        super::sales::privacy::check_record_copy(
            root,
            snapshot.job_role.is_some(),
            &serde_json::to_string(snapshot).map_err(|_| "agent snapshot serialization failed")?,
        )?;
    }
    if snapshot.schema != SNAPSHOT_SCHEMA || snapshot.v != 1 {
        return Err("a snapshot this host doesn't read".into());
    }
    match (&snapshot.job_role, &snapshot.crew_charter) {
        (None, None) => {}
        (Some(_), Some(charter)) => charter.validate().map_err(|e| e.message)?,
        _ => {
            return Err(
                "A sales snapshot must preserve its job role and machine charter together.".into(),
            );
        }
    }
    if store.load()?.is_some() {
        return Err(format!(
            "{} exists here already; import under another name",
            store.name()
        ));
    }
    let mut record = store.open(workspace, now)?;
    record.charter = snapshot.charter.clone();
    record.job_role = snapshot.job_role;
    record.crew_charter = snapshot.crew_charter.clone();
    record.sales_model_scope = snapshot.sales_model_scope.clone();
    if record.sales_model_scope.is_some() {
        record.requires.push("sales-model-budget.v1".into());
    }
    record.requires.retain(|r| r != "crew-sales.v1");
    if record.job_role.is_some() {
        record.requires.push("crew-sales.v1".into());
    }
    record.look = snapshot.look.clone();
    record.route = snapshot.route.clone();
    record.desk = snapshot.desk;
    record.definition = Some(snapshot.definition.clone());
    store.save(&record)?;
    let mut record = store.ensure_key(record, now)?;
    if let Some(owner) = owner {
        record = store.attest(record, owner, expires_at, now)?;
    }
    let memory = Memory::new(store.clone(), screen.clone());
    let entries: Vec<MemoryEntry> = snapshot
        .entries
        .iter()
        .filter(|entry| screen.check(&entry.text).is_ok())
        .cloned()
        .collect();
    memory.replace_entries(&entries)?;
    if owner.is_some() {
        if let (Some(core), Opened::Ready(mut engrams)) =
            (&snapshot.core, EngramStore::open(store, screen, now))
        {
            let body = Body::Core {
                profile: core.clone(),
                extra: serde_json::Map::from_iter([
                    ("schema".to_string(), agent_engrams::CORE_SCHEMA.into()),
                    ("v".to_string(), 1.into()),
                ]),
            };
            engrams.put(body, now)?;
        }
        agent_engrams::reconcile(&memory, now)?;
    }
    if let (Some(owner), Some(old), Some(new)) = (owner, &snapshot.pubkey, &record.pubkey) {
        let reason = format!("imported from a snapshot of {}", snapshot.name);
        if old != new {
            append_lineage(
                store,
                &Lineage::sign(owner, store.name(), old, new, &reason, now)?,
            )?;
        }
    }
    store.append(&Entry::new(
        now,
        Kind::Created,
        &format!(
            "imported from a snapshot of {} with a new key: {} definition, {}, and {} memory \
             entries",
            snapshot.name,
            record.refer().their(),
            if snapshot.core.is_some() {
                format!("{} core", record.refer().their())
            } else {
                "no core".into()
            },
            entries.len()
        ),
    ))?;
    Ok(record)
}

#[cfg(test)]
#[path = "agent_lifecycle_tests.rs"]
mod tests;
