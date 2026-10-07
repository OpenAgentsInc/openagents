//! A workshop agent's engram store (`docs/verse/agent-identity-and-engrams.md`,
//! "Engrams"): `agents/NAME/engrams/` under the host root.
//!
//! Every engram is a NIP-AE `kind:30174` event that she signs with her own
//! key and encrypts under `K_c`, the NIP-44 conversation key between her
//! key and her owner's, so she and her owner both decrypt every record. The
//! directory (mode `0700`) holds one file per head, `D.json`, named by the
//! event's blinded `d` tag and holding the signed event JSON (mode `0600`,
//! written atomically), plus `index.json`, a cache that maps each `d` to its
//! slug, `created_at`, and event ID. [`EngramStore::open`] rebuilds the
//! index from the events with her key whenever it is missing or stale.
//!
//! **Owner.** Her owner's public key is the `owner` of the NIP-OA
//! attestation in her record. Its signature must verify; its expiry does
//! not matter here, because an expired attestation still names who owns
//! her memory. An agent without a key or without an attestation keeps no
//! engrams: [`Opened::Skipped`], journaled once, and her memory writes go
//! on without them.
//!
//! **Slugs.** `core` is seeded from her record (name and charter) when it
//! is absent and is never rewritten here; `mem/persona` is a snapshot of
//! her definition without secrets, rewritten when it changes;
//! `mem/entry/ID` is one `agent_memory::MemoryEntry` as JSON text;
//! `mem/score/journal/POS` and `mem/score/memory/ID` are the newest
//! `scores.jsonl` row for `journal:POS` and `memory:ID`. A memory body
//! carries the working row's `schema` and `v` as extra fields. Forgetting
//! an entry, or trimming it past `agent_memory::MAX_ENTRIES`, writes a
//! tombstone.
//!
//! **Layering.** `memory.jsonl` and `scores.jsonl` stay the working files.
//! [`write_through`] and [`write_through_scores`] follow each write to
//! them; [`reconcile`] runs when the host first opens an agent, takes an
//! engram head newer than its working row (how an edit from another device
//! arrives), and writes through every working row the store lacks, which
//! migrates an agent made before the store existed.
//!
//! **Fail closed.** When any head cannot be read, verified, or decrypted,
//! the whole store is [`Opened::Unreadable`]: a reader carries nothing from
//! it and nothing writes to it, so `core` is never rewritten in that state.
//! The secret screen runs on every body before it is signed.
//!
//! Engram store design follows Buzz's NIP-AE client, reimplemented here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::engram::{self, Body, Engram, EventParams, HeadEntry, Pair, Slug};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use super::agent::{self, Entry, Kind, Record, Store};
use super::agent_memory::{Memory, MemoryEntry};
use super::agent_recall::{SCORE_SCHEMA, ScoreRow, Scores};

/// The store's directory under the agent's.
pub const DIR: &str = "engrams";
/// The index file's name.
pub const INDEX: &str = "index.json";
/// The index's schema.
pub const INDEX_SCHEMA: &str = "openagents.agent-engram-index.v1";
/// The `core` body's schema.
pub const CORE_SCHEMA: &str = "openagents.agent-core.v1";
/// The `mem/persona` value's schema.
pub const PERSONA_SCHEMA: &str = "openagents.agent-persona.v1";
/// The persona snapshot's slug.
pub const PERSONA_SLUG: &str = "mem/persona";
/// The most bytes a `core` body holds.
pub const CORE_MAX: usize = 10 * 1024;
/// What every journal line about the store starts with.
const NOTE_PREFIX: &str = "engrams: ";

/// One head in the index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexHead {
    pub slug: String,
    pub created_at: u64,
    /// The head event's ID.
    pub id: String,
}

/// `index.json` (`openagents.agent-engram-index.v1`): each head by `d`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema: String,
    pub v: u32,
    pub heads: BTreeMap<String, IndexHead>,
}

/// What opening an agent's engram store found.
#[derive(Debug)]
pub enum Opened {
    /// Every head read and verified.
    Ready(EngramStore),
    /// She keeps no engrams: no key, or no owner attestation. Says why.
    Skipped(String),
    /// The store cannot be read; carry nothing from it and write nothing
    /// to it. Says why.
    Unreadable(String),
}

impl Opened {
    /// The `core` profile a briefing carries: `Ok(None)` when she keeps no
    /// engrams or has no `core` yet, and `Err` with the reason when the
    /// store cannot be read, which a reader reports and never papers over.
    ///
    /// # Errors
    /// When the store is [`Opened::Unreadable`].
    pub fn carried_core(&self) -> Result<Option<&str>, &str> {
        match self {
            Self::Ready(store) => Ok(store.core()),
            Self::Skipped(_) => Ok(None),
            Self::Unreadable(why) => Err(why),
        }
    }
}

/// An agent's verified engram heads and the key to write more.
pub struct EngramStore {
    dir: PathBuf,
    secret: SecretKey,
    pair: Pair,
    screen: secret_screen::Screen,
    /// Each head by its `d` tag.
    heads: BTreeMap<String, Engram>,
}

impl std::fmt::Debug for EngramStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngramStore")
            .field("dir", &self.dir)
            .field("pair", &self.pair)
            .field("heads", &self.heads.len())
            .finish_non_exhaustive()
    }
}

/// The engram directory of `store`'s agent.
#[must_use]
pub fn dir_of(store: &Store) -> PathBuf {
    store.dir().join(DIR)
}

/// The slug of memory entry `id`: `mem/entry/ID`.
#[must_use]
pub fn entry_slug(id: u64) -> Slug {
    Slug::parse(&format!("mem/entry/{id}")).expect("a number is a slug segment")
}

/// The slug of the score row for `reference` (`journal:POS` or
/// `memory:ID`): `mem/score/journal/POS` or `mem/score/memory/ID`.
#[must_use]
pub fn score_slug(reference: &str) -> Option<Slug> {
    let reference = super::agent_recall::Ref::parse(reference)?;
    let text = match reference {
        super::agent_recall::Ref::Journal(pos) => format!("mem/score/journal/{pos}"),
        super::agent_recall::Ref::Memory(id) => format!("mem/score/memory/{id}"),
    };
    Slug::parse(&text).ok()
}

/// Her owner's public key: the owner of the attestation in `record`,
/// whose signature verifies. `Ok(None)` when she has no attestation.
///
/// # Errors
/// When the record has an attestation but no key, or the attestation does
/// not verify.
pub fn owner_of(record: &Record) -> Result<Option<XOnlyPublicKey>, String> {
    let Some(attestation) = &record.attestation else {
        return Ok(None);
    };
    let pubkey = record
        .pubkey
        .as_deref()
        .ok_or("her record has an attestation but no key")?;
    // Time 0 checks the signature and the clause grammar without the
    // expiry: an expired attestation still names her owner.
    agent::verify_attestation(pubkey, attestation, 0)
        .map_err(|why| format!("her owner attestation does not hold: {why}"))?;
    attestation
        .owner
        .parse::<XOnlyPublicKey>()
        .map(Some)
        .map_err(|_| "her owner attestation names no key".to_string())
}

/// Reads every `D.json` event in `dir`, as `(file stem, event)`. A missing
/// directory holds none.
fn events(dir: &Path) -> Result<Vec<(String, Event)>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {}: {e}", dir.display())),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name == INDEX || name.starts_with('.') {
            continue;
        }
        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };
        let path = entry.path();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let event: Event = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not an event: {e}", path.display()))?;
        out.push((stem.to_string(), event));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

impl EngramStore {
    /// Reads `store`'s engrams without writing anything.
    #[must_use]
    pub fn read(store: &Store, screen: &secret_screen::Screen) -> Opened {
        let record = match store.load() {
            Ok(Some(record)) => record,
            Ok(None) => return Opened::Skipped(format!("{} has no record", store.name())),
            Err(why) => return Opened::Unreadable(why),
        };
        let secret = match store.key() {
            Ok(Some(secret)) => secret,
            Ok(None) => return Opened::Skipped(format!("{} has no key", store.name())),
            Err(why) => return Opened::Unreadable(why),
        };
        if record.pubkey.as_deref() != Some(agent::public_hex(&secret).as_str()) {
            return Opened::Unreadable(format!("{}'s key does not match her record", store.name()));
        }
        let owner = match owner_of(&record) {
            Ok(Some(owner)) => owner,
            Ok(None) => {
                return Opened::Skipped(format!("{}'s key has no owner attestation", store.name()));
            }
            Err(why) => return Opened::Unreadable(why),
        };
        let pair = Pair::for_agent(&secret, &owner);
        let dir = dir_of(store);
        let events = match events(&dir) {
            Ok(events) => events,
            Err(why) => return Opened::Unreadable(why),
        };
        let mut heads = BTreeMap::new();
        for (stem, event) in events {
            let engram = match engram::validate_and_decrypt(&event, &pair) {
                Ok(engram) => engram,
                Err(e) => return Opened::Unreadable(format!("{stem}.json does not verify: {e}")),
            };
            if engram.d != stem {
                return Opened::Unreadable(format!("{stem}.json holds another head"));
            }
            heads.insert(stem, engram);
        }
        Opened::Ready(Self {
            dir,
            secret,
            pair,
            screen: screen.clone(),
            heads,
        })
    }

    /// Reads `store`'s engrams, then makes the directory, rebuilds a
    /// missing or stale index, seeds `core` from her record when it is
    /// absent, and writes `mem/persona` when her definition changed.
    #[must_use]
    pub fn open(store: &Store, screen: &secret_screen::Screen, now: u64) -> Opened {
        let mut engrams = match Self::read(store, screen) {
            Opened::Ready(engrams) => engrams,
            other => return other,
        };
        let Ok(Some(record)) = store.load() else {
            return Opened::Ready(engrams);
        };
        if let Err(why) = agent::private_dir(&engrams.dir).and_then(|()| engrams.fix_index()) {
            note(store, now, &why);
        }
        if engrams.core().is_none() {
            match engrams.put(seed_core(&record), now) {
                Ok(_) => note(
                    store,
                    now,
                    "seeded her core record from her name and charter",
                ),
                Err(why) => note(store, now, &format!("cannot seed her core record: {why}")),
            }
        }
        let persona = persona(&record);
        let slug = Slug::parse(PERSONA_SLUG).expect("the persona slug is a slug");
        if engrams.value(&slug) != Some(persona.as_str())
            && let Err(why) = Body::memory(slug, persona)
                .and_then(|b| b.with_extra("schema", PERSONA_SCHEMA.into()))
                .and_then(|b| b.with_extra("v", 1.into()))
                .map_err(|e| e.to_string())
                .and_then(|body| engrams.put(body, now))
        {
            note(store, now, &format!("cannot write her persona: {why}"));
        }
        Opened::Ready(engrams)
    }

    /// The agent-owner pair the store is encrypted for.
    #[must_use]
    pub fn pair(&self) -> &Pair {
        &self.pair
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The head at `slug`, a tombstone included.
    #[must_use]
    pub fn head(&self, slug: &Slug) -> Option<&Engram> {
        self.heads.get(&self.pair.d_tag(slug))
    }

    /// The live value at memory slug `slug`.
    #[must_use]
    pub fn value(&self, slug: &Slug) -> Option<&str> {
        match &self.head(slug)?.body {
            Body::Memory { value, .. } => value.as_deref(),
            Body::Core { .. } => None,
        }
    }

    /// Her `core` profile.
    #[must_use]
    pub fn core(&self) -> Option<&str> {
        match &self.head(&Slug::core())?.body {
            Body::Core { profile, .. } => Some(profile),
            Body::Memory { .. } => None,
        }
    }

    /// Every head, tombstones and `core` included, sorted by slug.
    #[must_use]
    pub fn heads(&self) -> Vec<&Engram> {
        let mut heads: Vec<&Engram> = self.heads.values().collect();
        heads.sort_by_key(|head| head.slug());
        heads
    }

    /// The live memory heads, without `core` or tombstones, by slug.
    #[must_use]
    pub fn live(&self) -> Vec<HeadEntry> {
        let heads: Vec<Engram> = self.heads.values().cloned().collect();
        engram::list_heads(&heads)
    }

    /// The index the heads make.
    #[must_use]
    pub fn index(&self) -> Index {
        Index {
            schema: INDEX_SCHEMA.into(),
            v: 1,
            heads: self
                .heads
                .iter()
                .map(|(d, head)| {
                    (
                        d.clone(),
                        IndexHead {
                            slug: head.slug().as_str().to_string(),
                            created_at: head.created_at,
                            id: head.id.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    /// Writes the index again when the stored one is missing, unreadable,
    /// or not the one the heads make.
    fn fix_index(&self) -> Result<(), String> {
        let index = self.index();
        if read_index(&self.dir).ok().flatten().as_ref() == Some(&index) {
            return Ok(());
        }
        self.write_index(&index)
    }

    fn write_index(&self, index: &Index) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
        write_atomic(&self.dir, INDEX, &body)
    }

    /// Signs and stores `body` as its slug's new head, and returns it.
    /// The secret screen runs first, a `core` body is at most
    /// [`CORE_MAX`] bytes, and `created_at` is `max(now, head + 1)`.
    ///
    /// # Errors
    /// The screen refuses the body, `core` is too long, the clock would
    /// run more than an hour ahead of `now` (a conflict to report), or the
    /// event cannot be written.
    pub fn put(&mut self, body: Body, now: u64) -> Result<Engram, String> {
        let plaintext = body.to_json();
        if let Err(refusal) = self.screen.check(&plaintext) {
            return Err(format!("the secret screen refuses this engram: {refusal}"));
        }
        let slug = body.slug();
        if slug.is_core() && plaintext.len() > CORE_MAX {
            return Err(format!("a core record is at most {CORE_MAX} bytes"));
        }
        let head = self.head(&slug).map(|head| head.created_at);
        let created_at = engram::monotonic_created_at(now, head)
            .map_err(|e| format!("conflict at {}: {e}; nothing was written", slug.as_str()))?;
        let params = EventParams {
            created_at,
            nonce: secp256k1::rand::random::<[u8; 32]>(),
            aux: secp256k1::rand::random::<[u8; 32]>(),
            alt: true,
        };
        let event = engram::build_event(&self.secret, self.pair.owner(), &body, &params)
            .map_err(|e| e.to_string())?;
        let written =
            engram::validate_and_decrypt(&event, &self.pair).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(&event).map_err(|e| e.to_string())?;
        agent::private_dir(&self.dir)?;
        write_atomic(&self.dir, &format!("{}.json", written.d), &json)?;
        self.heads.insert(written.d.clone(), written.clone());
        self.write_index(&self.index())?;
        Ok(written)
    }

    /// Writes memory entry `entry` as `mem/entry/ID`.
    ///
    /// # Errors
    /// As [`EngramStore::put`].
    pub fn put_entry(&mut self, entry: &MemoryEntry, now: u64) -> Result<Engram, String> {
        let value = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        let body = Body::memory(entry_slug(entry.id), value)
            .and_then(|b| b.with_extra("schema", entry.schema.clone().into()))
            .and_then(|b| b.with_extra("v", entry.v.into()))
            .map_err(|e| e.to_string())?;
        self.put(body, now)
    }

    /// Writes the tombstone of memory entry `id`.
    ///
    /// # Errors
    /// As [`EngramStore::put`].
    pub fn forget_entry(&mut self, id: u64, now: u64) -> Result<Engram, String> {
        let body = Body::tombstone(entry_slug(id))
            .and_then(|b| b.with_extra("schema", super::agent_memory::SCHEMA.into()))
            .and_then(|b| b.with_extra("v", 1.into()))
            .map_err(|e| e.to_string())?;
        self.put(body, now)
    }

    /// Writes score row `row` as `mem/score/...`.
    ///
    /// # Errors
    /// The row names no record, or as [`EngramStore::put`].
    pub fn put_score(&mut self, row: &ScoreRow, now: u64) -> Result<Engram, String> {
        let slug = score_slug(&row.record)
            .ok_or_else(|| format!("{} is not a scored record", row.record))?;
        let value = serde_json::to_string(row).map_err(|e| e.to_string())?;
        let body = Body::memory(slug, value)
            .and_then(|b| b.with_extra("schema", row.schema.clone().into()))
            .and_then(|b| b.with_extra("v", row.v.into()))
            .map_err(|e| e.to_string())?;
        self.put(body, now)
    }

    /// Every live `mem/entry/ID` head as `(head, entry)`, and every entry
    /// tombstone as `(head, None)`, by ID. A head whose value is not an
    /// entry is skipped.
    fn entry_heads(&self) -> BTreeMap<u64, (&Engram, Option<MemoryEntry>)> {
        let mut out = BTreeMap::new();
        for head in self.heads.values() {
            let Body::Memory { slug, value, .. } = &head.body else {
                continue;
            };
            let Some(Ok(id)) = slug
                .as_str()
                .strip_prefix("mem/entry/")
                .map(str::parse::<u64>)
            else {
                continue;
            };
            match value {
                None => {
                    out.insert(id, (head, None));
                }
                Some(text) => {
                    if let Ok(entry) = serde_json::from_str::<MemoryEntry>(text)
                        && entry.id == id
                    {
                        out.insert(id, (head, Some(entry)));
                    }
                }
            }
        }
        out
    }

    /// Every live `mem/score/...` head as `(created_at, row)`, by record.
    fn score_heads(&self) -> BTreeMap<String, (u64, ScoreRow)> {
        let mut out = BTreeMap::new();
        for head in self.heads.values() {
            let Body::Memory {
                slug,
                value: Some(text),
                ..
            } = &head.body
            else {
                continue;
            };
            if !slug.as_str().starts_with("mem/score/") {
                continue;
            }
            if let Ok(row) = serde_json::from_str::<ScoreRow>(text)
                && row.schema == SCORE_SCHEMA
                && score_slug(&row.record).as_ref() == Some(slug)
            {
                out.insert(row.record.clone(), (head.created_at, row));
            }
        }
        out
    }
}

/// The index stored in `dir`, when there is one that reads.
///
/// # Errors
/// When the file exists and cannot be read or parsed.
pub fn read_index(dir: &Path) -> Result<Option<Index>, String> {
    let path = dir.join(INDEX);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let index: Index = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not an engram index: {e}", path.display()))?;
    if index.schema != INDEX_SCHEMA || index.v != 1 {
        return Err(format!(
            "{} is an index this host does not read",
            path.display()
        ));
    }
    Ok(Some(index))
}

fn write_atomic(dir: &Path, name: &str, body: &[u8]) -> Result<(), String> {
    let temp = dir.join(format!(".{name}.tmp"));
    agent::write_private(&temp, body)?;
    std::fs::rename(&temp, dir.join(name))
        .map_err(|e| format!("cannot write {}: {e}", dir.join(name).display()))
}

/// The `core` body seeded from her record: her name and charter.
fn seed_core(record: &Record) -> Body {
    let profile = format!(
        "I am {}, my owner's workshop agent.\n\nMy charter: {}",
        record.name, record.charter
    );
    Body::Core {
        profile,
        extra: serde_json::Map::from_iter([
            ("schema".to_string(), CORE_SCHEMA.into()),
            ("v".to_string(), 1.into()),
        ]),
    }
}

/// Her definition without secrets, as the `mem/persona` value: the
/// record's name, charter, look, route, desk, and public keys.
fn persona(record: &Record) -> String {
    serde_json::json!({
        "name": record.name,
        "charter": record.charter,
        "look": record.look,
        "route": record.route,
        "desk": record.desk,
        "pubkey": record.pubkey,
        "owner": record.attestation.as_ref().map(|a| a.owner.clone()),
    })
    .to_string()
}

/// Journals `text` about the store, unless the newest such line already
/// says it, so a store that stays off is journaled once.
fn note(store: &Store, now: u64, text: &str) {
    let line = format!("{NOTE_PREFIX}{text}");
    let last = store.journal(256).ok().and_then(|entries| {
        entries
            .into_iter()
            .rev()
            .find(|e| e.kind == Kind::Memory && e.text.starts_with(NOTE_PREFIX))
    });
    if last.is_some_and(|e| e.text == agent::screen(&line)) {
        return;
    }
    let _ = store.append(&Entry::new(now, Kind::Memory, &line));
}

/// Opens `memory`'s store for a write, journaling once why it cannot.
fn writable(memory: &Memory, now: u64) -> Option<EngramStore> {
    match EngramStore::open(memory.store(), memory.screen(), now) {
        Opened::Ready(engrams) => Some(engrams),
        Opened::Skipped(why) => {
            note(memory.store(), now, &format!("off: {why}"));
            None
        }
        Opened::Unreadable(why) => {
            note(
                memory.store(),
                now,
                &format!("unreadable, nothing written: {why}"),
            );
            None
        }
    }
}

/// Follows a write of `memory.jsonl` from `before` to `after`: each entry
/// that is new or changed becomes its `mem/entry/ID` head, and each entry
/// that is gone becomes a tombstone. A failure is journaled, never what an
/// entry said, and never fails the memory write.
pub fn write_through(memory: &Memory, before: &[MemoryEntry], after: &[MemoryEntry], now: u64) {
    let Some(mut engrams) = writable(memory, now) else {
        return;
    };
    for entry in after {
        if before.iter().any(|b| b == entry) {
            continue;
        }
        if let Err(why) = engrams.put_entry(entry, now) {
            note(memory.store(), now, &format!("entry {}: {why}", entry.id));
        }
    }
    for gone in before.iter().filter(|b| after.iter().all(|a| a.id != b.id)) {
        if let Err(why) = engrams.forget_entry(gone.id, now) {
            note(memory.store(), now, &format!("entry {}: {why}", gone.id));
        }
    }
}

/// Follows an append of `rows` to `scores.jsonl`: each becomes its
/// `mem/score/...` head. A briefing writes no journal line, so this is
/// silent and seeds nothing: a row it could not write goes through at the next
/// [`reconcile`], which journals why the store is off.
pub fn write_through_scores(memory: &Memory, rows: &[ScoreRow], now: u64) {
    if rows.is_empty() {
        return;
    }
    let Opened::Ready(mut engrams) = EngramStore::read(memory.store(), memory.screen()) else {
        return;
    };
    for row in rows {
        let _ = engrams.put_score(row, now);
    }
}

/// What [`reconcile`] changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reconciled {
    /// Working rows an engram head replaced, added, or removed.
    pub pulled: usize,
    /// Working rows written through to the store.
    pub pushed: usize,
}

/// Reconciles `memory`'s working files with its engram store. An engram
/// head newer than its working row (`created_at` after the row's `at`)
/// that differs from it wins: it replaces the row, adds it, or, as a
/// tombstone, removes it. Every other working row the store lacks or
/// holds differently is written through, which migrates an agent whose
/// directory has no engrams yet.
///
/// # Errors
/// When she keeps no engrams or the store cannot be read (both journaled
/// once), or a working file cannot be read or written.
pub fn reconcile(memory: &Memory, now: u64) -> Result<Reconciled, String> {
    let store = memory.store();
    let mut engrams = match EngramStore::open(store, memory.screen(), now) {
        Opened::Ready(engrams) => engrams,
        Opened::Skipped(why) => {
            note(store, now, &format!("off: {why}"));
            return Err(why);
        }
        Opened::Unreadable(why) => {
            note(store, now, &format!("unreadable, nothing written: {why}"));
            return Err(why);
        }
    };
    let mut done = Reconciled::default();

    // Memory entries.
    let mut rows = memory.entries()?;
    let mut changed = false;
    for (id, (head, entry)) in engrams.entry_heads() {
        let position = rows.iter().position(|row| row.id == id);
        match (entry, position) {
            (None, Some(i)) if head.created_at > rows[i].at => {
                rows.remove(i);
                changed = true;
                done.pulled += 1;
            }
            (Some(entry), None) => {
                rows.push(entry);
                changed = true;
                done.pulled += 1;
            }
            (Some(entry), Some(i)) if rows[i] != entry && head.created_at > rows[i].at => {
                rows[i] = entry;
                changed = true;
                done.pulled += 1;
            }
            _ => {}
        }
    }
    if changed {
        rows.sort_by_key(|row| row.id);
        memory.replace_entries(&rows)?;
    }
    let heads = engrams.entry_heads();
    let stale: Vec<MemoryEntry> = rows
        .into_iter()
        .filter(|row| match heads.get(&row.id) {
            Some((_, Some(entry))) => entry != row,
            _ => true,
        })
        .collect();
    for row in &stale {
        engrams.put_entry(row, now)?;
        done.pushed += 1;
    }

    // Score rows.
    let sidecar = Scores::of(store);
    let mut known = sidecar.load()?;
    let mut newer = Vec::new();
    for (record, (created_at, row)) in engrams.score_heads() {
        let take = match known.get(&record) {
            None => true,
            Some(have) => *have != row && created_at > have.at,
        };
        if take {
            known.insert(record, row.clone());
            newer.push(row);
        }
    }
    sidecar.append(&newer)?;
    done.pulled += newer.len();
    let heads = engrams.score_heads();
    let mut stale: Vec<&ScoreRow> = known
        .values()
        .filter(|row| heads.get(&row.record).is_none_or(|(_, have)| have != *row))
        .collect();
    stale.sort_by(|a, b| a.record.cmp(&b.record));
    for row in stale {
        engrams.put_score(row, now)?;
        done.pushed += 1;
    }
    if done != Reconciled::default() {
        note(
            store,
            now,
            &format!(
                "reconciled: {} rows from engrams, {} rows written through",
                done.pulled, done.pushed
            ),
        );
    }
    Ok(done)
}

/// What the owner reads with the owner key.
#[derive(Debug, Default)]
pub struct OwnerView {
    /// Every head that verified and decrypted, by slug.
    pub heads: Vec<Engram>,
    /// Each file that did not, and why.
    pub problems: Vec<String>,
}

/// Decrypts `store`'s engrams with `owner`, the owner's secret key: her
/// public key comes from her record. The owner never needs her key.
///
/// # Errors
/// When her record cannot be read or names no key, or the directory
/// cannot be read.
pub fn owner_read(store: &Store, owner: &SecretKey) -> Result<OwnerView, String> {
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    let agent = record
        .pubkey
        .as_deref()
        .ok_or("she has no key, so she has no engrams")?
        .parse::<XOnlyPublicKey>()
        .map_err(|_| "her record's key is not a key".to_string())?;
    let pair = Pair::for_owner(owner, &agent);
    let mut view = OwnerView::default();
    for (stem, event) in events(&dir_of(store))? {
        match engram::validate_and_decrypt(&event, &pair) {
            Ok(engram) if engram.d == stem => view.heads.push(engram),
            Ok(_) => view
                .problems
                .push(format!("{stem}.json holds another head")),
            Err(e) => view.problems.push(format!("{stem}.json: {e}")),
        }
    }
    view.heads.sort_by_key(Engram::slug);
    Ok(view)
}

#[cfg(test)]
#[path = "agent_engrams_tests.rs"]
mod tests;
