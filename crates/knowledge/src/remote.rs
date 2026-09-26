//! Entries shared over Nostr (`nips/openagents/NIP-KB.md`): turning an
//! entry file into the parts of a kind-`3190` event, accepting the events a
//! relay returns, the local cache they're kept in, and which of them a
//! reader shows.
//!
//! Signing and the relay connection are the caller's; nothing here opens a
//! socket. The cache is `~/.openagents/knowledge/remote/<author>/`, one
//! `<id>.event.json` per entry holding the signed event, and `<id>.md`
//! beside it for people to read. Loading re-checks every event's signature,
//! so an edited cache file is refused, not trusted.
//!
//! Trust is the reader's. Entries from the reader's own key and from
//! authors it lists keep the status their document states; with trust
//! `all`, every other author's entries are candidates at most.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::kb::{self, EntryVersion, Unsigned};

use crate::lint::{Corpus, lint};
use crate::{Base, Entry, Status};

/// `~/.openagents/knowledge/remote`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/knowledge/remote"))
}

/// `~/.openagents/knowledge/trust.json`.
#[must_use]
pub fn trust_file() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/knowledge/trust.json"))
}

/// `~/.openagents/nostr/knowledge-key`, the secret key entries are signed
/// with. It's created on first use with mode 0600.
#[must_use]
pub fn key_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/nostr/knowledge-key"))
}

/// The public key of the secret key in `path`, without creating one.
#[must_use]
pub fn own_pubkey(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let signer = nostr::domain::RelaySigner::from_secret_hex(text.trim()).ok()?;
    Some(signer.pubkey().to_string())
}

/// A public key written as an `npub` or 64 lowercase hex characters, as hex.
#[must_use]
pub fn parse_author(text: &str) -> Option<String> {
    let text = text.trim();
    if let Ok(bytes) = nostr::nip19::decode_npub(text) {
        return Some(bytes.iter().map(|b| format!("{b:02x}")).collect());
    }
    (text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then(|| text.to_string())
}

/// A hex public key as an `npub`.
#[must_use]
pub fn npub(pubkey: &str) -> String {
    let mut bytes = [0u8; 32];
    for (i, pair) in pubkey.as_bytes().chunks(2).take(32).enumerate() {
        bytes[i] = std::str::from_utf8(pair)
            .ok()
            .and_then(|p| u8::from_str_radix(p, 16).ok())
            .unwrap_or(0);
    }
    nostr::nip19::encode_npub(&bytes)
}

/// Which remote entries a reader shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    /// Only local entries and remote entries signed with the reader's key.
    Own,
    /// Those, plus entries from the authors the trust file lists.
    Listed,
    /// Those, plus every other synced author's entries, as candidates.
    All,
}

impl Trust {
    /// The mode `own`, `listed`, or `all` names.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "own" => Trust::Own,
            "listed" => Trust::Listed,
            "all" => Trust::All,
            _ => return None,
        })
    }
}

impl fmt::Display for Trust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Trust::Own => "own",
            Trust::Listed => "listed",
            Trust::All => "all",
        })
    }
}

/// The trust file: a mode and the authors it lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustConfig {
    pub mode: Trust,
    /// Hex public keys.
    pub authors: Vec<String>,
}

impl Default for TrustConfig {
    fn default() -> Self {
        TrustConfig {
            mode: Trust::Own,
            authors: Vec::new(),
        }
    }
}

impl TrustConfig {
    /// Reads `{"mode": "listed", "authors": ["npub1…"]}` from `path`. A
    /// missing file is the default: mode `own`, no authors.
    ///
    /// # Errors
    ///
    /// A file that exists but doesn't parse, an unknown mode, or an author
    /// that isn't a public key.
    pub fn read(path: &Path) -> Result<Self, String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(TrustConfig::default());
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mode = match value["mode"].as_str() {
            None => Trust::Own,
            Some(mode) => Trust::parse(mode).ok_or(format!(
                "{}: mode must be own, listed, or all, not {mode}",
                path.display()
            ))?,
        };
        let mut authors = Vec::new();
        for author in value["authors"].as_array().cloned().unwrap_or_default() {
            let text = author.as_str().unwrap_or_default();
            authors.push(parse_author(text).ok_or(format!(
                "{}: {text} isn't an npub or a hex public key",
                path.display()
            ))?);
        }
        Ok(TrustConfig { mode, authors })
    }
}

/// The parts of a `3190` for the entry file `text`.
///
/// # Errors
///
/// When the file isn't a valid entry, or NIP-KB refuses its ID, kind, or
/// tags.
pub fn entry_event(text: &str) -> Result<Unsigned, String> {
    let entry = Entry::parse(text)?;
    let topics: Vec<String> = entry
        .tags
        .iter()
        .map(|t| t.to_lowercase().replace(char::is_whitespace, "-"))
        .filter(|t| !t.is_empty() && !t.starts_with("oa:"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    kb::entry(
        &entry.id,
        u64::from(entry.version),
        &entry.kind.to_string(),
        &topics,
        text,
    )
    .map_err(|e| format!("{}: {e}", entry.id))
}

/// One entry version a sync accepted.
#[derive(Clone, Debug)]
pub struct Accepted {
    pub author: String,
    pub version: EntryVersion,
    /// The signed `3190`.
    pub event: Event,
}

/// What a sync made of the events a relay returned.
#[derive(Clone, Debug, Default)]
pub struct Sync {
    /// The version each author's entry is at, one per author and ID.
    pub accepted: Vec<Accepted>,
    /// Author and ID of each entry whose current version is withdrawn.
    pub withdrawn: Vec<(String, String)>,
    /// What was refused and why.
    pub refused: Vec<String>,
}

/// Checks every event and picks each author's current version of each
/// entry: the version its head names, else the highest one. A withdrawn
/// version, a version with two different documents, or a document that
/// fails the lint is never picked.
#[must_use]
pub fn accept(events: &[Event], corpus: &Corpus) -> Sync {
    let mut sync = Sync::default();
    let mut versions: BTreeMap<(String, String), Vec<(EntryVersion, Event)>> = BTreeMap::new();
    let mut by_event: BTreeMap<String, Event> = BTreeMap::new();
    for event in events.iter().filter(|e| e.kind == kb::ENTRY_KIND) {
        match kb::parse_entry(event) {
            Ok(version) => {
                by_event.insert(event.id.clone(), event.clone());
                let slot = versions
                    .entry((event.pubkey.clone(), version.id.clone()))
                    .or_default();
                if !slot.iter().any(|(v, _)| v.digest == version.digest) {
                    slot.push((version, event.clone()));
                }
            }
            Err(error) => sync
                .refused
                .push(format!("event {}: {error}", short(&event.id))),
        }
    }
    let mut withdrawn: BTreeSet<String> = BTreeSet::new();
    let mut heads: BTreeMap<(String, String), (u64, u64, String)> = BTreeMap::new();
    for event in events {
        let pointer = match event.kind {
            kb::WITHDRAWAL_KIND => kb::parse_withdrawal(event).map(|w| (w.id, w.version, w.entry)),
            kb::HEAD_KIND => kb::parse_head(event).map(|h| (h.id, h.version, h.entry)),
            _ => continue,
        };
        let (id, version, pointer) = match pointer {
            Ok(parts) => parts,
            Err(error) => {
                sync.refused
                    .push(format!("event {}: {error}", short(&event.id)));
                continue;
            }
        };
        let Some(entry) = by_event.get(&pointer.id) else {
            continue;
        };
        if let Err(error) = kb::bind(&pointer, &id, version, entry) {
            sync.refused
                .push(format!("event {}: {error}", short(&event.id)));
            continue;
        }
        if event.kind == kb::WITHDRAWAL_KIND {
            withdrawn.insert(pointer.id);
        } else {
            let key = (event.pubkey.clone(), id);
            if heads
                .get(&key)
                .is_none_or(|(at, _, _)| event.created_at > *at)
            {
                heads.insert(key, (event.created_at, version, pointer.id));
            }
        }
    }
    for ((author, id), list) in versions {
        let mut conflicted: BTreeSet<u64> = BTreeSet::new();
        for (i, (a, _)) in list.iter().enumerate() {
            for (b, _) in &list[i + 1..] {
                if let Err(error) = kb::equivocation(a, b) {
                    sync.refused
                        .push(format!("{} by {}: {error}", id, short(&author)));
                    conflicted.insert(a.version);
                }
            }
        }
        let usable: Vec<&(EntryVersion, Event)> = list
            .iter()
            .filter(|(v, e)| !conflicted.contains(&v.version) && !withdrawn.contains(&e.id))
            .collect();
        let head = heads.get(&(author.clone(), id.clone()));
        let chosen = head
            .and_then(|(_, _, event_id)| usable.iter().find(|(_, e)| &e.id == event_id))
            .or_else(|| usable.iter().max_by_key(|(v, _)| v.version));
        let Some((version, event)) = chosen else {
            if list.iter().any(|(_, e)| withdrawn.contains(&e.id)) {
                sync.withdrawn.push((author, id));
            }
            continue;
        };
        match check_document(version, corpus) {
            Ok(()) => sync.accepted.push(Accepted {
                author,
                version: version.clone(),
                event: (*event).clone(),
            }),
            Err(error) => sync
                .refused
                .push(format!("{} by {}: {error}", id, short(&author))),
        }
    }
    sync
}

/// Checks that a version's document is a valid entry agreeing with the
/// event, and passes the lint.
fn check_document(version: &EntryVersion, corpus: &Corpus) -> Result<(), String> {
    let entry = Entry::parse(&version.document)?;
    if entry.id != version.id
        || u64::from(entry.version) != version.version
        || entry.kind.to_string() != version.kind
    {
        return Err("the document's id, version, or kind disagrees with the event".to_string());
    }
    if let Some(problem) = lint(&[entry], corpus).first() {
        return Err(format!("lint: {}", problem.message));
    }
    Ok(())
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

/// Writes a sync's result under `dir`: for each author it returned, the
/// author's directory is replaced by the accepted entries.
///
/// # Errors
///
/// When a file can't be written.
pub fn write_cache(dir: &Path, sync: &Sync) -> Result<(), String> {
    let authors: BTreeSet<&str> = sync
        .accepted
        .iter()
        .map(|a| a.author.as_str())
        .chain(sync.withdrawn.iter().map(|(a, _)| a.as_str()))
        .collect();
    for author in authors {
        let at = dir.join(author);
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).map_err(|e| format!("can't make {}: {e}", at.display()))?;
        for accepted in sync.accepted.iter().filter(|a| a.author == author) {
            let id = &accepted.version.id;
            let event = serde_json::to_string_pretty(&accepted.event).map_err(|e| e.to_string())?;
            std::fs::write(at.join(format!("{id}.event.json")), event)
                .and_then(|()| {
                    std::fs::write(at.join(format!("{id}.md")), &accepted.version.document)
                })
                .map_err(|e| format!("can't write {id} under {}: {e}", at.display()))?;
        }
    }
    Ok(())
}

/// Every cached entry, re-checked: author, parsed entry, and the event.
/// A file that no longer verifies is reported, not loaded.
#[must_use]
pub fn read_cache(dir: &Path) -> (Vec<(String, Entry, Event)>, Vec<String>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let Ok(listing) = std::fs::read_dir(dir) else {
        return (out, problems);
    };
    let mut authors: Vec<PathBuf> = listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    authors.sort();
    for author_dir in authors {
        let author = author_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if parse_author(&author).is_none() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&author_dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = files
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".event.json"))
            .collect();
        files.sort();
        for file in files {
            let loaded = std::fs::read_to_string(&file)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Event>(&t).map_err(|e| e.to_string()))
                .and_then(|event| {
                    if event.pubkey != author {
                        return Err("the event's author isn't the directory's".to_string());
                    }
                    let version = kb::parse_entry(&event).map_err(|e| e.to_string())?;
                    let entry = Entry::parse(&version.document)?;
                    Ok((entry, event))
                });
            match loaded {
                Ok((entry, event)) => out.push((author.clone(), entry, event)),
                Err(error) => problems.push(format!("{}: {error}", file.display())),
            }
        }
    }
    (out, problems)
}

/// Where the entries in a loaded base came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    pub local: usize,
    /// Remote entries shown, by author (hex).
    pub remote: BTreeMap<String, usize>,
    /// Remote entries left out because a local or earlier entry has the ID.
    pub shadowed: usize,
    /// Cache files that didn't verify.
    pub problems: Vec<String>,
}

/// The base a run searches: the local entries in `dir`, then the cached
/// remote entries `trust` admits, each shown only if its status is (given
/// whether candidates are). A remote entry's author becomes its signer's
/// `npub`, and an entry from an author the reader doesn't trust is a
/// candidate at most. When two entries share an ID, the local one wins,
/// then the reader's own, then listed authors in order.
///
/// # Errors
///
/// Any problem with the local entries, as [`Base::load`] reports it.
pub fn load(
    dir: &Path,
    remote: Option<&Path>,
    config: &TrustConfig,
    own: Option<&str>,
    candidates: bool,
) -> Result<(Base, Loaded), String> {
    let mut base = Base::load(dir, candidates)?;
    let mut loaded = Loaded {
        local: base.entries.len(),
        ..Loaded::default()
    };
    let Some(remote) = remote else {
        return Ok((base, loaded));
    };
    let (cached, problems) = read_cache(remote);
    loaded.problems = problems;
    let rank = |author: &str| -> Option<usize> {
        if Some(author) == own {
            Some(0)
        } else if config.mode != Trust::Own {
            match config.authors.iter().position(|a| a == author) {
                Some(i) => Some(1 + i),
                None if config.mode == Trust::All => Some(usize::MAX),
                None => None,
            }
        } else {
            None
        }
    };
    let mut ranked: Vec<(usize, String, Entry)> = cached
        .into_iter()
        .filter_map(|(author, entry, _)| rank(&author).map(|r| (r, author, entry)))
        .collect();
    ranked.sort_by(|a, b| (a.0, &a.1, &a.2.id).cmp(&(b.0, &b.1, &b.2.id)));
    let local: BTreeSet<String> = Base::read(dir).0.into_iter().map(|e| e.id).collect();
    for (rank, author, mut entry) in ranked {
        if rank == usize::MAX && entry.status == Status::Admitted {
            entry.status = Status::Candidate;
        }
        if !entry.status.shown(candidates) {
            continue;
        }
        if local.contains(&entry.id) || base.get(&entry.id).is_some() {
            loaded.shadowed += 1;
            continue;
        }
        entry.author = npub(&author);
        *loaded.remote.entry(author).or_default() += 1;
        base.entries.push(entry);
    }
    Ok((base, loaded))
}

#[cfg(test)]
mod tests;
