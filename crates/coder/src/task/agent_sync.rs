//! Engram relay sync for a workshop agent
//! (`docs/verse/agent-identity-and-engrams.md`, "Engrams", Sync).
//!
//! **Off by default.** The owner turns it on per agent with
//! `openagents agent memory NAME sync on --relay URL`, which writes
//! `agents/NAME/sync.json` (`openagents.agent-sync.v1`): `memory_relays`, the
//! relay URLs she syncs with, and `requested_at`, when the owner last asked
//! for a pass now. An empty list is off.
//!
//! **A pass.** [`sync`] runs one pass when her record names a key that her
//! key store holds (`Store::custody`) and the owner attested it. She
//! connects to each configured relay as herself and presents the owner's
//! NIP-OA `auth` tag in the NIP-42 `AUTH` event (NIP-AA), then:
//!
//! 1. Reads every head she wrote for her owner (`kinds: [30174]`, her key as
//!    author, `#p` her owner), verifies each, and takes a head that wins
//!    NIP-AE head selection over the local one into the engram store.
//!    `agent_engrams::reconcile` then updates the working files, which is
//!    how an edit from another device arrives. A head more than an hour
//!    ahead of the clock is clock-poisoned: it's journaled as a conflict
//!    and never taken.
//! 2. Publishes her `kind:0` profile and her NIP-65 `kind:10002` relay list,
//!    when a relay lacks the current ones.
//! 3. Publishes each local head that a relay lacks or holds an older
//!    version of. After the relay's `OK`, she reads the head again; when it
//!    isn't the event she sent, the pass journals a conflict and never
//!    retries it.
//!
//! The pass writes what it found to `agents/NAME/sync-status.json`
//! (`openagents.agent-sync-status.v1`), including each relay that answered
//! a query with as many events as the limit allows, which may have more
//! (`truncated`). The host runs a pass every [`INTERVAL`] seconds while sync
//! is on, and at its next sweep after the owner asks for one.
//!
//! **Owner reads.** [`owner_read`] reads her heads from relays with the
//! owner's key alone: it finds her `kind:10002` on the relays the owner
//! names, queries her write relays (or the named relays when she has no
//! list), and decrypts each head with the owner's side of the pair.
//!
//! **What a relay sees.** Her key, her owner's key, how many heads she has,
//! their sizes, and when they change; never a slug or a value.
//!
//! The relay client follows Buzz's NIP-AE and NIP-AA client design,
//! reimplemented here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::engram::{self, CLOCK_POISON_SECS, ENGRAM_KIND, Engram, Pair};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::agent::{self, Entry, Kind, State, Store};
use super::agent_engrams::{self, EngramStore, Opened};
use super::agent_memory::Memory;

/// Her sync settings, beside her record.
pub const SETTINGS_FILE: &str = "sync.json";
/// The settings' schema.
pub const SETTINGS_SCHEMA: &str = "openagents.agent-sync.v1";
/// The last pass's status, beside her record.
pub const STATUS_FILE: &str = "sync-status.json";
/// The status's schema.
pub const STATUS_SCHEMA: &str = "openagents.agent-sync-status.v1";
/// Seconds between two passes the host runs on its own.
pub const INTERVAL: u64 = 5 * 60;
/// The most events one query asks a relay for. A relay that answers with
/// this many may hold more, so the status marks it truncated.
pub const QUERY_LIMIT: usize = 1000;
/// NIP-65's relay list.
pub const RELAY_LIST_KIND: u16 = 10_002;
/// The relay a `sync on` without `--relay` names: the owner's relay.
pub const DEFAULT_RELAY: &str = "wss://relay.openagents.com";
/// The most relays one agent syncs with.
pub const MAX_RELAYS: usize = 8;
/// How long one relay connection may last.
const LIFETIME: Duration = Duration::from_secs(90);
/// What every journal line about sync starts with.
const NOTE_PREFIX: &str = "relay sync: ";

/// `sync.json`: which relays she syncs her engrams with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema: String,
    pub v: u32,
    /// Her write relays, as the owner wrote them. Empty is off.
    #[serde(default)]
    pub memory_relays: Vec<String>,
    /// When the owner last asked for a pass now, Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at: Option<u64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SETTINGS_SCHEMA.into(),
            v: 1,
            memory_relays: Vec::new(),
            requested_at: None,
        }
    }
}

impl Settings {
    /// Whether sync is on.
    #[must_use]
    pub fn on(&self) -> bool {
        !configured(&self.memory_relays).is_empty()
    }

    /// `store`'s settings; the default (off) when there is no file.
    ///
    /// # Errors
    /// When the file exists and isn't v1 settings.
    pub fn load(store: &Store) -> Result<Self, String> {
        let path = settings_path(store);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("can't read {}: {e}", path.display())),
        };
        let settings: Self = serde_json::from_str(&text)
            .map_err(|e| format!("{} isn't sync settings: {e}", path.display()))?;
        if settings.schema != SETTINGS_SCHEMA || settings.v != 1 {
            return Err(format!(
                "{} is a version this host doesn't read",
                path.display()
            ));
        }
        Ok(settings)
    }

    /// Writes `store`'s settings.
    ///
    /// # Errors
    /// When the file can't be written.
    pub fn save(&self, store: &Store) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write(store, SETTINGS_FILE, &body)
    }
}

fn settings_path(store: &Store) -> PathBuf {
    store.dir().join(SETTINGS_FILE)
}

fn write(store: &Store, name: &str, body: &[u8]) -> Result<(), String> {
    agent::private_dir(store.dir())?;
    let temp = store.dir().join(format!(".{name}.tmp"));
    agent::write_private(&temp, body)?;
    std::fs::rename(&temp, store.dir().join(name))
        .map_err(|e| format!("can't write {}: {e}", store.dir().join(name).display()))
}

/// Turns sync on with `relays`, or off when `relays` is empty, clears the
/// last pass's status so the host's next sweep runs one, and journals that. Each relay must be a `ws://` or `wss://` URL; duplicates
/// by NIP-AE's comparison are dropped.
///
/// # Errors
/// When a URL isn't a relay URL, there are more than [`MAX_RELAYS`], or the
/// settings can't be written.
pub fn set_relays(store: &Store, relays: &[String], now: u64) -> Result<Settings, String> {
    for url in relays {
        canonical(url)?;
    }
    let relays = configured(relays);
    if relays.len() > MAX_RELAYS {
        return Err(format!("she syncs with at most {MAX_RELAYS} relays"));
    }
    let mut settings = Settings::load(store)?;
    settings.memory_relays = relays;
    settings.save(store)?;
    // The last pass was for other relays; the next sweep runs one.
    match std::fs::remove_file(store.dir().join(STATUS_FILE)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("can't reset the sync status: {e}")),
    }
    let text = if settings.memory_relays.is_empty() {
        "off".to_string()
    } else {
        format!("on, with {}", settings.memory_relays.join(", "))
    };
    store.append(&Entry::new(
        now,
        Kind::Memory,
        &format!("{NOTE_PREFIX}{text}"),
    ))?;
    Ok(settings)
}

/// Asks for a pass at the host's next sweep.
///
/// # Errors
/// When sync is off or the settings can't be written.
pub fn request(store: &Store, now: u64) -> Result<(), String> {
    let mut settings = Settings::load(store)?;
    if !settings.on() {
        return Err(format!("relay sync is off for {}", store.name()));
    }
    settings.requested_at = Some(now);
    settings.save(store)
}

/// A URL in NIP-AE's comparison form: lowercase scheme and host, no
/// default port (443 for `wss`, 80 for `ws`), and no trailing slash on an
/// otherwise empty path. The rest of the path is kept as written.
///
/// # Errors
/// When `url` isn't a `ws://` or `wss://` URL with a host.
pub fn canonical(url: &str) -> Result<String, String> {
    let refuse = || format!("{url} isn't a ws:// or wss:// relay URL");
    let (scheme, rest) = url.split_once("://").ok_or_else(refuse)?;
    let scheme = scheme.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "wss" => "443",
        "ws" => "80",
        _ => return Err(refuse()),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    if authority.is_empty()
        || authority.contains('@')
        || authority
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        || path.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(refuse());
    }
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']').ok_or_else(refuse)?;
        let (host, after) = authority.split_at(close + 1);
        match after.strip_prefix(':') {
            Some(port) => (host, Some(port)),
            None if after.is_empty() => (host, None),
            None => return Err(refuse()),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if host.is_empty() || host.trim_start_matches('[').contains('[') {
        return Err(refuse());
    }
    let port = match port {
        Some(port) => {
            port.parse::<u16>().map_err(|_| refuse())?;
            (port != default_port).then(|| format!(":{port}"))
        }
        None => None,
    };
    let path = if path == "/" { "" } else { path };
    Ok(format!(
        "{scheme}://{}{}{path}",
        host.to_ascii_lowercase(),
        port.unwrap_or_default()
    ))
}

/// `urls` without the ones that aren't relay URLs and without later
/// duplicates by [`canonical`] form, each kept as written so a connection
/// goes to the URL as advertised.
#[must_use]
pub fn configured(urls: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    urls.iter()
        .filter(|url| canonical(url).is_ok_and(|c| seen.insert(c)))
        .cloned()
        .collect()
}

/// Her write relays from a NIP-65 relay list: `r` tags marked `write` or
/// unmarked, valid relay URLs, deduplicated.
#[must_use]
pub fn write_relays(list: &Event) -> Vec<String> {
    let urls: Vec<String> = list
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("r"))
        .filter(|tag| matches!(tag.0.get(2).map(String::as_str), None | Some("write")))
        .filter_map(|tag| tag.value().map(str::to_string))
        .collect();
    configured(&urls)
}

fn same_relays(a: &[String], b: &[String]) -> bool {
    let set = |urls: &[String]| -> BTreeSet<String> {
        urls.iter().filter_map(|u| canonical(u).ok()).collect()
    };
    set(a) == set(b)
}

fn signer(key: &SecretKey) -> Result<RelaySigner, String> {
    let secret: String = key
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&secret).map_err(|e| e.to_string())
}

/// Her NIP-65 relay list, signed with her key: each relay as a `write`
/// relay.
///
/// # Errors
/// When the key can't sign.
pub fn relay_list(key: &SecretKey, relays: &[String], created_at: u64) -> Result<Event, String> {
    let tags = configured(relays)
        .into_iter()
        .map(|url| Tag::new(vec!["r".into(), url, "write".into()]))
        .collect();
    Ok(signer(key)?.sign(created_at, RELAY_LIST_KIND, tags, String::new()))
}

/// One relay connection, already authenticated.
pub trait Relay {
    /// Publishes `event` and returns the relay's `OK`: accepted, and its
    /// message.
    ///
    /// # Errors
    /// When the connection fails before the relay answers.
    fn publish(&mut self, event: &Event) -> Result<(bool, String), String>;

    /// The events the relay holds for `filter`, up to its end-of-stored
    /// events.
    ///
    /// # Errors
    /// When the connection fails or the relay closes the query.
    fn query(&mut self, filter: &Value) -> Result<Vec<Event>, String>;
}

/// Opens relay connections.
pub trait Connector: Send + Sync {
    /// Connects to `url` and authenticates with NIP-42 as `key`, with the
    /// NIP-OA `auth` tag in the `AUTH` event when `auth` is set (NIP-AA).
    ///
    /// # Errors
    /// When the relay can't be reached or refuses the authentication.
    fn connect(
        &self,
        url: &str,
        key: &SecretKey,
        auth: Option<&Tag>,
    ) -> Result<Box<dyn Relay>, String>;
}

/// The real relays, through `nostr-transport`. Each connection runs on a
/// runtime of its own, so call it from a plain thread, never from inside
/// an async task.
#[derive(Clone, Copy, Debug, Default)]
pub struct Live;

struct LiveRelay {
    runtime: tokio::runtime::Runtime,
    connection: Option<nostr_transport::Connection>,
    queries: u64,
}

impl Connector for Live {
    fn connect(
        &self,
        url: &str,
        key: &SecretKey,
        auth: Option<&Tag>,
    ) -> Result<Box<dyn Relay>, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("no runtime for the relay connection: {e}"))?;
        let connection = runtime
            .block_on(nostr_transport::Connection::connect_as(
                url, key, auth, LIFETIME,
            ))?
            .with_frame_budget(4096);
        Ok(Box::new(LiveRelay {
            runtime,
            connection: Some(connection),
            queries: 0,
        }))
    }
}

impl LiveRelay {
    fn connection(&mut self) -> Result<&mut nostr_transport::Connection, String> {
        self.connection
            .as_mut()
            .ok_or_else(|| "the relay connection is closed".to_string())
    }
}

impl Relay for LiveRelay {
    fn publish(&mut self, event: &Event) -> Result<(bool, String), String> {
        let id = event.id.clone();
        let message = json!(["EVENT", event]);
        let runtime = &self.runtime;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| "the relay connection is closed".to_string())?;
        runtime.block_on(async {
            connection.send(message).await?;
            loop {
                let frame = connection.next().await?;
                if frame[0] == "OK" && frame[1] == id.as_str() {
                    return Ok((
                        frame[2] == true,
                        frame[3].as_str().unwrap_or_default().to_string(),
                    ));
                }
            }
        })
    }

    fn query(&mut self, filter: &Value) -> Result<Vec<Event>, String> {
        self.queries += 1;
        let sub = format!("engrams-{}", self.queries);
        let request = json!(["REQ", sub, filter]);
        self.connection()?;
        let runtime = &self.runtime;
        let connection = self.connection.as_mut().expect("checked above");
        runtime.block_on(async {
            connection.send(request).await?;
            let mut events = Vec::new();
            loop {
                let frame = connection.next().await?;
                if frame[1] != sub.as_str() {
                    continue;
                }
                match frame[0].as_str() {
                    Some("EVENT") => {
                        if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                            events.push(event);
                        }
                    }
                    Some("EOSE") => break,
                    Some("CLOSED") => {
                        return Err(format!(
                            "the relay closed the query: {}",
                            frame[2].as_str().unwrap_or_default()
                        ));
                    }
                    _ => {}
                }
            }
            let _ = connection.send(json!(["CLOSE", sub])).await;
            Ok(events)
        })
    }
}

impl Drop for LiveRelay {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = self.runtime.block_on(connection.close());
        }
    }
}

/// What one relay held and took in a pass.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayStatus {
    /// The relay, as configured.
    pub url: String,
    /// Why the relay couldn't be reached or answered with an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Her heads the relay held that verified.
    pub heads: usize,
    /// Events the relay returned that didn't verify for her and her owner.
    pub invalid: usize,
    /// The relay answered with [`QUERY_LIMIT`] events and may hold more.
    pub truncated: bool,
    /// Heads published to it that it kept.
    pub published: usize,
    /// Events it refused, each with its message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<String>,
    /// Her profile went to it in this pass.
    pub profile: bool,
    /// Her relay list went to it in this pass.
    pub relay_list: bool,
}

/// `sync-status.json`: the last pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub schema: String,
    pub v: u32,
    /// When the pass ran, Unix seconds.
    pub at: u64,
    /// Why the pass didn't run or stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub relays: Vec<RelayStatus>,
    /// Heads taken from relays.
    pub pulled: usize,
    /// Heads published and verified.
    pub pushed: usize,
    /// Each conflict, journaled too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
}

impl Status {
    fn new(at: u64) -> Self {
        Self {
            schema: STATUS_SCHEMA.into(),
            v: 1,
            at,
            error: None,
            relays: Vec::new(),
            pulled: 0,
            pushed: 0,
            conflicts: Vec::new(),
        }
    }

    /// Whether any relay may hold more heads than it returned.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.relays.iter().any(|r| r.truncated)
    }

    /// `store`'s last pass, when one ran.
    ///
    /// # Errors
    /// When the file exists and isn't a v1 status.
    pub fn load(store: &Store) -> Result<Option<Self>, String> {
        let path = store.dir().join(STATUS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("can't read {}: {e}", path.display())),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{} isn't a sync status: {e}", path.display()))
    }

    fn save(&self, store: &Store) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write(store, STATUS_FILE, &body)
    }
}

/// Whether the host should run a pass for `store` now: sync is on, and no
/// pass ran yet, the last one is [`INTERVAL`] old, or the owner asked for
/// one since.
#[must_use]
pub fn due(store: &Store, now: u64) -> bool {
    let Ok(settings) = Settings::load(store) else {
        return false;
    };
    if !settings.on() {
        return false;
    }
    match Status::load(store) {
        Ok(Some(last)) => {
            last.at.saturating_add(INTERVAL) <= now
                || settings.requested_at.is_some_and(|at| at > last.at)
        }
        _ => true,
    }
}

fn note(store: &Store, now: u64, text: &str) {
    let _ = store.append(&Entry::new(
        now,
        Kind::Memory,
        &format!("{NOTE_PREFIX}{text}"),
    ));
}

/// Whether `candidate` wins NIP-AE head selection over `head`.
fn wins(candidate: &Engram, head: &Engram) -> bool {
    candidate.id != head.id
        && engram::select_head([head, candidate]).is_some_and(|h| h.id == candidate.id)
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// Each `d`'s head among `events`, verified for `pair`, with its event.
/// Returns the heads and how many events didn't verify.
fn heads_of(events: Vec<Event>, pair: &Pair) -> (BTreeMap<String, (Engram, Event)>, usize) {
    let mut heads: BTreeMap<String, (Engram, Event)> = BTreeMap::new();
    let mut invalid = 0;
    for event in events {
        let Ok(engram) = engram::validate_and_decrypt(&event, pair) else {
            invalid += 1;
            continue;
        };
        let newer = heads
            .get(&engram.d)
            .is_none_or(|(head, _)| wins(&engram, head));
        if newer {
            heads.insert(engram.d.clone(), (engram, event));
        }
    }
    (heads, invalid)
}

fn heads_filter(pair: &Pair) -> Value {
    json!({
        "kinds": [ENGRAM_KIND],
        "authors": [pair.agent().to_string()],
        "#p": [pair.owner().to_string()],
        "limit": QUERY_LIMIT,
    })
}

/// Runs one pass for `store` through `connector`, writes its status, and
/// returns it. A pass that can't run says why in `error`, which is
/// journaled.
pub fn sync(
    store: &Store,
    screen: &secret_screen::Screen,
    connector: &dyn Connector,
    now: u64,
) -> Status {
    let mut status = Status::new(now);
    if let Err(why) = pass(store, screen, connector, now, &mut status) {
        note(store, now, &format!("didn't finish: {why}"));
        status.error = Some(why);
    }
    for conflict in &status.conflicts {
        note(store, now, &format!("conflict: {conflict}"));
    }
    if status.pulled + status.pushed > 0 {
        note(
            store,
            now,
            &format!(
                "{} heads taken from relays, {} published",
                status.pulled, status.pushed
            ),
        );
    }
    if let Err(why) = status.save(store) {
        note(store, now, &format!("can't keep the status: {why}"));
    }
    status
}

struct Link {
    url: String,
    relay: Box<dyn Relay>,
    heads: BTreeMap<String, (Engram, Event)>,
}

fn pass(
    store: &Store,
    screen: &secret_screen::Screen,
    connector: &dyn Connector,
    now: u64,
    status: &mut Status,
) -> Result<(), String> {
    let relays = configured(&Settings::load(store)?.memory_relays);
    if relays.is_empty() {
        return Err("relay sync is off".into());
    }
    let record = store
        .load()?
        .ok_or_else(|| format!("there is no agent named {}", store.name()))?;
    if record.state == State::Retired {
        return Err("she is retired".into());
    }
    if record.pubkey.is_none() {
        return Err("she has no key".into());
    }
    store.custody(&record)?;
    let key = store.key()?.ok_or("she has no key")?;
    let attestation = record
        .attestation
        .clone()
        .ok_or("she has no owner attestation to present")?;
    let auth = Tag::new(vec![
        "auth".into(),
        attestation.owner,
        attestation.conditions,
        attestation.signature,
    ]);
    let _ = super::agent_profile::refresh(store, &record, now);
    let profile = super::agent_profile::load(store)
        .ok()
        .flatten()
        .filter(|event| super::agent_profile::current(&record, event));
    // Read without writing, so a computer that has none yet takes `core`
    // from the relays instead of seeding a rival one first.
    let mut engrams = match EngramStore::read(store, screen) {
        Opened::Ready(engrams) => engrams,
        Opened::Skipped(why) => return Err(format!("she keeps no engrams: {why}")),
        Opened::Unreadable(why) => {
            return Err(format!(
                "her engram store can't be read, so nothing syncs: {why}"
            ));
        }
    };
    let pair = engrams.pair().clone();

    // Read every relay's heads.
    let mut links = Vec::new();
    for url in &relays {
        let mut relay_status = RelayStatus {
            url: url.clone(),
            ..RelayStatus::default()
        };
        let read = connector
            .connect(url, &key, Some(&auth))
            .and_then(|mut relay| {
                let events = relay.query(&heads_filter(&pair))?;
                Ok((relay, events))
            });
        match read {
            Ok((relay, events)) => {
                relay_status.truncated = events.len() >= QUERY_LIMIT;
                let (heads, invalid) = heads_of(events, &pair);
                relay_status.heads = heads.len();
                relay_status.invalid = invalid;
                links.push(Link {
                    url: url.clone(),
                    relay,
                    heads,
                });
            }
            Err(why) => relay_status.error = Some(why),
        }
        status.relays.push(relay_status);
    }
    if links.is_empty() {
        return Err("no relay could be read".into());
    }

    // Take each head that is newer than hers.
    let mut union: BTreeMap<String, (Engram, Event)> = BTreeMap::new();
    for link in &links {
        for (d, (head, event)) in &link.heads {
            if union.get(d).is_none_or(|(have, _)| wins(head, have)) {
                union.insert(d.clone(), (head.clone(), event.clone()));
            }
        }
    }
    for (d, (head, event)) in &union {
        if engrams.head_at(d).is_some_and(|local| !wins(head, local)) {
            continue;
        }
        if head.created_at > now.saturating_add(CLOCK_POISON_SECS) {
            status.conflicts.push(format!(
                "{} on the relays is dated {} seconds ahead of this clock; not taken",
                head.slug().as_str(),
                head.created_at - now
            ));
            continue;
        }
        match engrams.adopt(event) {
            Ok(true) => status.pulled += 1,
            Ok(false) => {}
            Err(why) => status
                .conflicts
                .push(format!("{} can't be stored: {why}", head.slug().as_str())),
        }
    }
    drop(engrams);
    if status.pulled > 0 {
        let memory = Memory::new(store.clone(), screen.clone());
        if let Err(why) = agent_engrams::reconcile(&memory, now) {
            note(
                store,
                now,
                &format!("the working files didn't take the new heads: {why}"),
            );
        }
    }
    // Opening seeds `core` and the persona when the relays had neither.
    let engrams = match EngramStore::open(store, screen, now) {
        Opened::Ready(engrams) => engrams,
        Opened::Skipped(why) | Opened::Unreadable(why) => {
            return Err(format!(
                "her engram store can't be read after the merge: {why}"
            ));
        }
    };

    // Publish her identity and every head a relay lacks, and verify each.
    for link in &mut links {
        let index = status
            .relays
            .iter()
            .position(|r| r.url == link.url)
            .expect("every link has a status");
        let mut conflicts = Vec::new();
        let result = publish_to(
            link,
            &engrams,
            &key,
            profile.as_ref(),
            &relays,
            now,
            &mut status.relays[index],
            &mut conflicts,
        );
        if let Err(why) = result {
            status.relays[index].error = Some(why);
        }
        status.pushed += status.relays[index].published;
        status.conflicts.extend(conflicts);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn publish_to(
    link: &mut Link,
    engrams: &EngramStore,
    key: &SecretKey,
    profile: Option<&Event>,
    relays: &[String],
    now: u64,
    relay_status: &mut RelayStatus,
    conflicts: &mut Vec<String>,
) -> Result<(), String> {
    let agent_hex = engrams.pair().agent().to_string();
    let identity = link.relay.query(&json!({
        "kinds": [super::agent_profile::PROFILE_KIND, RELAY_LIST_KIND],
        "authors": [agent_hex],
    }))?;
    let newest = |kind: u16| {
        identity
            .iter()
            .filter(|e| e.kind == kind && e.pubkey == agent_hex && e.validate_crypto().is_ok())
            .max_by(|a, b| a.created_at.cmp(&b.created_at).then(b.id.cmp(&a.id)))
    };
    if let Some(profile) = profile {
        let stale = newest(super::agent_profile::PROFILE_KIND)
            .is_none_or(|have| have.id != profile.id && have.created_at <= profile.created_at);
        if stale {
            match link.relay.publish(profile)? {
                (true, _) => relay_status.profile = true,
                (false, why) => relay_status.refused.push(format!("profile: {why}")),
            }
        }
    }
    let list = newest(RELAY_LIST_KIND);
    if list.is_none_or(|have| !same_relays(&write_relays(have), relays)) {
        let created_at = list.map_or(now, |have| now.max(have.created_at + 1));
        let event = relay_list(key, relays, created_at)?;
        match link.relay.publish(&event)? {
            (true, _) => relay_status.relay_list = true,
            (false, why) => relay_status.refused.push(format!("relay list: {why}")),
        }
    }
    for head in engrams.heads() {
        let theirs = link.heads.get(&head.d).map(|(engram, _)| engram);
        if theirs.is_some_and(|theirs| theirs.id == head.id || wins(theirs, head)) {
            continue;
        }
        let slug = head.slug();
        let Some(event) = engrams.event(&head.d)? else {
            continue;
        };
        let (accepted, message) = link.relay.publish(&event)?;
        if !accepted {
            relay_status
                .refused
                .push(format!("{}: {message}", slug.as_str()));
            continue;
        }
        // Verify: the relay's head must now be this event.
        let found = link.relay.query(&json!({
            "kinds": [ENGRAM_KIND],
            "authors": [agent_hex],
            "#d": [head.d],
        }))?;
        let (now_held, _) = heads_of(found, engrams.pair());
        match now_held.get(&head.d) {
            Some((held, _)) if held.id == head.id => relay_status.published += 1,
            Some((held, _)) => conflicts.push(format!(
                "{} on {}: the relay's head is {} at {}, not this write {} at {}; not retried",
                slug.as_str(),
                link.url,
                short(&held.id),
                held.created_at,
                short(&head.id),
                head.created_at
            )),
            None => conflicts.push(format!(
                "{} on {}: the relay said OK but holds no head; not retried",
                slug.as_str(),
                link.url
            )),
        }
    }
    Ok(())
}

/// Runs passes on the host's sweep, one at a time per agent, each on a
/// thread of its own.
pub struct Sweeper {
    connector: Arc<dyn Connector>,
    running: Mutex<BTreeSet<String>>,
}

impl std::fmt::Debug for Sweeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sweeper").finish_non_exhaustive()
    }
}

impl Sweeper {
    /// A sweeper that connects through `connector`.
    #[must_use]
    pub fn new(connector: Arc<dyn Connector>) -> Arc<Self> {
        Arc::new(Self {
            connector,
            running: Mutex::new(BTreeSet::new()),
        })
    }

    /// Starts a pass for `store` when one is [`due`] and none runs for her,
    /// and returns its thread.
    pub fn sweep(
        self: &Arc<Self>,
        store: &Store,
        screen: &secret_screen::Screen,
        now: u64,
    ) -> Option<std::thread::JoinHandle<()>> {
        if !due(store, now) {
            return None;
        }
        let name = store.name().to_string();
        if !self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.clone())
        {
            return None;
        }
        let this = Arc::clone(self);
        let store = store.clone();
        let screen = screen.clone();
        Some(std::thread::spawn(move || {
            let _ = sync(&store, &screen, this.connector.as_ref(), now);
            this.running
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&name);
        }))
    }
}

/// Her heads as the owner reads them from relays.
#[derive(Debug, Default)]
pub struct RelayView {
    /// The relays read: her write relays when she publishes a list, else
    /// the ones named.
    pub relays: Vec<String>,
    /// Each live head, `core` included, by slug.
    pub heads: Vec<Engram>,
    /// Forgotten entries: tombstones at the head.
    pub forgotten: usize,
    /// Each relay that couldn't be read, and events that didn't verify.
    pub problems: Vec<String>,
    /// Relays that answered with [`QUERY_LIMIT`] events and may hold more.
    pub truncated: Vec<String>,
}

/// Reads and decrypts `agent`'s heads from relays with the owner's key
/// `owner` alone. `named` are the relays to look for her NIP-65 list on;
/// her write relays from it are read, or `named` when she has none.
///
/// # Errors
/// When no relay is named or none can be read.
pub fn owner_read(
    agent: &XOnlyPublicKey,
    owner: &SecretKey,
    named: &[String],
    connector: &dyn Connector,
) -> Result<RelayView, String> {
    let named = configured(named);
    if named.is_empty() {
        return Err("name a ws:// or wss:// relay to read her memory from".into());
    }
    let pair = Pair::for_owner(owner, agent);
    let mut view = RelayView::default();
    let mut open: BTreeMap<String, Box<dyn Relay>> = BTreeMap::new();
    let mut list: Option<Event> = None;
    for url in &named {
        let found = connector.connect(url, owner, None).and_then(|mut relay| {
            let events = relay.query(&json!({
                "kinds": [RELAY_LIST_KIND],
                "authors": [agent.to_string()],
            }))?;
            Ok((relay, events))
        });
        match found {
            Ok((relay, events)) => {
                for event in events {
                    let valid = event.kind == RELAY_LIST_KIND
                        && event.pubkey == agent.to_string()
                        && event.validate_crypto().is_ok();
                    if valid
                        && list
                            .as_ref()
                            .is_none_or(|l| event.created_at > l.created_at)
                    {
                        list = Some(event);
                    }
                }
                open.insert(canonical(url)?, relay);
            }
            Err(why) => view.problems.push(format!("{url}: {why}")),
        }
    }
    let advertised = list.as_ref().map(write_relays).unwrap_or_default();
    view.relays = if advertised.is_empty() {
        named
    } else {
        advertised
    };
    let mut union: BTreeMap<String, Engram> = BTreeMap::new();
    let mut read = 0;
    for url in &view.relays.clone() {
        let key = canonical(url)?;
        let relay = match open.remove(&key) {
            Some(relay) => Ok(relay),
            None => connector.connect(url, owner, None),
        };
        let events = relay.and_then(|mut relay| relay.query(&heads_filter(&pair)));
        let events = match events {
            Ok(events) => events,
            Err(why) => {
                view.problems.push(format!("{url}: {why}"));
                continue;
            }
        };
        read += 1;
        if events.len() >= QUERY_LIMIT {
            view.truncated.push(url.clone());
        }
        let (heads, invalid) = heads_of(events, &pair);
        if invalid > 0 {
            view.problems
                .push(format!("{url}: {invalid} events didn't verify"));
        }
        for (d, (head, _)) in heads {
            if union.get(&d).is_none_or(|have| wins(&head, have)) {
                union.insert(d, head);
            }
        }
    }
    if read == 0 {
        return Err(format!(
            "no relay could be read: {}",
            view.problems.join("; ")
        ));
    }
    let (forgotten, live): (Vec<Engram>, Vec<Engram>) =
        union.into_values().partition(Engram::is_tombstone);
    view.forgotten = forgotten.len();
    view.heads = live;
    view.heads.sort_by_key(Engram::slug);
    Ok(view)
}

#[cfg(test)]
#[path = "agent_sync_tests.rs"]
mod tests;
