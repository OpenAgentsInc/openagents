//! `microcoder kb publish`, `kb sync`, and `kb publish-evidence`: the
//! knowledge base over a Nostr relay (`nips/openagents/NIP-KB.md`).
//!
//! The connection and its NIP-42 authentication are Coder's relay client
//! (`coder::relay`). Events are built and checked by `nostr::kb`, and
//! `knowledge::remote` decides what a sync accepts and caches. Entries are
//! signed with the key in `~/.openagents/nostr/knowledge-key`, created on
//! first use with mode 0600; the key is never printed.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coder::relay::{Identity, Socket, connect, send};
use futures_util::StreamExt;
use knowledge::cli::Options;
use knowledge::evidence::{self, Evaluator};
use knowledge::lint::{Corpus, default_corpora};
use knowledge::remote::{self, npub};
use knowledge::transfer;
use knowledge::{Base, Entry, Status};
use nostr::domain::Event;
use nostr::kb;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

/// How long one query or one publish waits for the relay.
pub const WAIT: Duration = Duration::from_secs(20);

/// Events one page of a query asks for. A relay may send fewer: the
/// OpenAgents relay caps one `REQ` at 127 by default, and
/// [`Relay::query`] pages past whatever cap it meets.
pub const LIMIT: usize = 1_000;

/// Pages one filter may take before a query gives up and warns.
const MAX_PAGES: usize = 10_000;

/// An authenticated relay connection.
pub struct Relay {
    socket: Socket,
    next: u32,
    incomplete: usize,
}

/// What a relay's EOSE said about the stored events it matched (NIP-67).
#[derive(Clone, Copy)]
enum End {
    /// It sent them all.
    Finish,
    /// It holds more it didn't send.
    More,
    /// It didn't say.
    Unknown,
}

/// `filter` split into one filter per kind, or else one per author, for a
/// page that can't advance by `until`; `None` when it names at most one
/// of each.
fn split(filter: &Value) -> Option<Vec<Value>> {
    for field in ["kinds", "authors"] {
        if let Some(values) = filter[field].as_array().filter(|v| v.len() > 1) {
            return Some(
                values
                    .iter()
                    .map(|value| {
                        let mut part = filter.clone();
                        part[field] = json!([value]);
                        part
                    })
                    .collect(),
            );
        }
    }
    None
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

impl Relay {
    /// Connects to `url` and answers its NIP-42 challenge as `identity`.
    ///
    /// # Errors
    ///
    /// When the connection or authentication fails.
    pub async fn open(url: &str, identity: &Identity) -> Result<Self, String> {
        let socket = connect(url, identity)
            .await
            .map_err(|e| format!("{url}: {e}"))?;
        Ok(Relay {
            socket,
            next: 0,
            incomplete: 0,
        })
    }

    async fn frame(&mut self, deadline: tokio::time::Instant) -> Result<Value, String> {
        loop {
            let message = match tokio::time::timeout_at(deadline, self.socket.next()).await {
                Ok(Some(Ok(message))) => message,
                Ok(Some(Err(error))) => return Err(format!("the relay connection broke: {error}")),
                Ok(None) => return Err("the relay closed the connection".to_string()),
                Err(_) => {
                    return Err(format!(
                        "the relay didn't answer within {} seconds",
                        WAIT.as_secs()
                    ));
                }
            };
            if let Message::Text(text) = message
                && let Ok(value) = serde_json::from_str::<Value>(text.as_str())
            {
                return Ok(value);
            }
        }
    }

    /// Every stored event matching `filter`, however many pages the relay
    /// splits them into. A relay caps one `REQ` below the filter's `limit`
    /// (the OpenAgents relay at 127 by default), so this pages back with
    /// `until` (NIP-01) and drops the events it has already seen. It stops
    /// on a NIP-67 `finish` EOSE, on an empty page, or on a page with
    /// nothing new. When a page can't advance, because more events than
    /// one page share the oldest second, it asks again one kind at a
    /// time, then one author at a time. What it still can't reach is
    /// warned about on stderr, and [`Relay::incomplete`] counts it.
    ///
    /// # Errors
    ///
    /// When the relay closes the subscription or doesn't answer.
    pub async fn query(&mut self, filter: Value) -> Result<Vec<Event>, String> {
        let mut seen = std::collections::HashSet::new();
        let mut events = Vec::new();
        let mut pending = vec![filter];
        while let Some(filter) = pending.pop() {
            let mut until = filter.get("until").and_then(Value::as_u64);
            let mut done = false;
            for _ in 0..MAX_PAGES {
                let mut paged = filter.clone();
                if let Some(until) = until {
                    paged["until"] = json!(until);
                }
                let (page, end) = self.page(&paged).await?;
                let oldest = page.iter().map(|e| e.created_at).min();
                let mut fresh = 0;
                for event in page {
                    if seen.insert(event.id.clone()) {
                        events.push(event);
                        fresh += 1;
                    }
                }
                match (end, oldest) {
                    (End::Finish, _) | (End::Unknown, None) => {}
                    // A relay without NIP-67: a page with nothing new is
                    // the end, since it doesn't say whether it cut one.
                    (End::Unknown, Some(_)) if fresh == 0 => {}
                    (End::More, Some(_)) if fresh == 0 => {
                        if let Some(parts) = split(&filter) {
                            pending.extend(parts);
                        } else {
                            self.warn(&filter, "more of them share one second than one page holds");
                        }
                    }
                    (End::More, None) => {
                        self.warn(&filter, "it said there were more, then sent none")
                    }
                    (_, Some(oldest)) => {
                        until = Some(oldest);
                        continue;
                    }
                }
                done = true;
                break;
            }
            if !done {
                self.warn(&filter, &format!("{MAX_PAGES} pages weren't enough"));
            }
        }
        Ok(events)
    }

    fn warn(&mut self, filter: &Value, why: &str) {
        self.incomplete += 1;
        eprintln!(
            "warning: the relay didn't return every event matching {filter}: {why}; this answer \
is incomplete"
        );
    }

    /// How many queries on this connection ended without every matching
    /// event, each already warned about on stderr.
    #[must_use]
    pub fn incomplete(&self) -> usize {
        self.incomplete
    }

    /// One `REQ`: the events up to its EOSE, and what that EOSE said.
    async fn page(&mut self, filter: &Value) -> Result<(Vec<Event>, End), String> {
        self.next += 1;
        let id = format!("kb-{}-{}", std::process::id(), self.next);
        send(&mut self.socket, json!(["REQ", id, filter]))
            .await
            .map_err(|e| e.to_string())?;
        let deadline = tokio::time::Instant::now() + WAIT;
        let mut events = Vec::new();
        let end = loop {
            let frame = self.frame(deadline).await?;
            if frame[1].as_str() != Some(id.as_str()) {
                continue;
            }
            match frame[0].as_str().unwrap_or_default() {
                "EVENT" => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                        events.push(event);
                    }
                }
                "EOSE" => {
                    let marker = frame[2].as_array().and_then(|m| m.first());
                    break match marker.and_then(Value::as_str) {
                        Some("finish") => End::Finish,
                        Some("more") => End::More,
                        _ => End::Unknown,
                    };
                }
                "CLOSED" => {
                    return Err(format!(
                        "the relay closed the query: {}",
                        frame[2].as_str().unwrap_or_default()
                    ));
                }
                _ => {}
            }
        };
        let _ = send(&mut self.socket, json!(["CLOSE", id])).await;
        Ok((events, end))
    }

    /// Publishes `event` and waits for the relay to accept it. A duplicate
    /// counts as accepted.
    ///
    /// # Errors
    ///
    /// When the relay refuses the event or doesn't answer.
    pub async fn publish(&mut self, event: &Event) -> Result<(), String> {
        send(&mut self.socket, json!(["EVENT", event]))
            .await
            .map_err(|e| e.to_string())?;
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let frame = self.frame(deadline).await?;
            if frame[0] != "OK" || frame[1].as_str() != Some(event.id.as_str()) {
                continue;
            }
            let reason = frame[3].as_str().unwrap_or_default();
            if frame[2].as_bool() == Some(true) || reason.starts_with("duplicate:") {
                return Ok(());
            }
            return Err(format!("the relay refused it: {reason}"));
        }
    }
}

fn sign(identity: &Identity, parts: kb::Unsigned) -> Event {
    identity
        .signer()
        .sign(now(), parts.kind, parts.tags, parts.content)
}

/// `~/.openagents/nostr/knowledge-key`.
fn default_key() -> Result<PathBuf, String> {
    remote::key_file().ok_or("HOME isn't set, so there's no key file".to_string())
}

/// Runs `kb publish`, `kb sync`, or `kb publish-evidence` with `args`, the
/// command first. Returns the exit code: 0 on success, 1 when something
/// was refused, 2 on bad usage or a relay that can't be reached.
pub async fn main(args: &[String]) -> u8 {
    let result = async {
        let command = args.first().ok_or(knowledge::cli::USAGE)?;
        let o = knowledge::cli::parse(&args[1..])?;
        let key = default_key()?;
        match command.as_str() {
            "publish" => publish(&o, &key).await,
            "sync" => {
                let dir = o
                    .remote
                    .clone()
                    .or_else(remote::default_dir)
                    .ok_or("no cache directory: pass --remote")?;
                sync(&o, &key, &dir).await
            }
            "publish-evidence" => publish_evidence(&o, &key).await,
            other => Err(format!("unknown command {other}")),
        }
    }
    .await;
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

fn relay_url(o: &Options) -> Result<&str, String> {
    o.relay.as_deref().ok_or(
        "name the relay with --relay, such as --relay ws://127.0.0.1:7447; publishing sends \
entries to other people, so there's no default relay"
            .to_string(),
    )
}

/// Loads the key, creating it on first use. `role` says what it's for:
/// signing, or answering the relay's authentication challenge.
fn load_key(key: &Path, role: &str) -> Result<Identity, String> {
    let identity = Identity::load_from(key)?;
    println!(
        "{role} {} (key in {})",
        npub(identity.pubkey()),
        key.display()
    );
    Ok(identity)
}

/// The entries in `--dir`, or the ones the words name.
fn entries(o: &Options) -> Result<Vec<(Entry, String)>, String> {
    let (all, problems) = Base::read(&o.dir);
    if let Some(problem) = problems.first() {
        return Err(format!("{}: {problem}", o.dir.display()));
    }
    for id in &o.words {
        if !all.iter().any(|e| &e.id == id) {
            return Err(format!("no entry {id} in {}", o.dir.display()));
        }
    }
    all.into_iter()
        .filter(|e| o.words.is_empty() || o.words.contains(&e.id))
        .map(|e| {
            let path = o.dir.join(format!("{}.md", e.id));
            let text = std::fs::read_to_string(&path)
                .map_err(|err| format!("can't read {}: {err}", path.display()))?;
            Ok((e, text))
        })
        .collect()
}

/// One author's entry versions, withdrawals, and heads on the relay.
struct Mine {
    versions: Vec<(kb::EntryVersion, Event)>,
    withdrawn: Vec<String>,
    heads: Vec<(String, String)>,
}

/// The entry versions, withdrawals, and heads `author` signed for `ids`.
/// Events another key signed are never counted.
async fn mine(relay: &mut Relay, author: &str, ids: &[String]) -> Result<Mine, String> {
    let events = relay
        .query(json!({
            "kinds": [kb::ENTRY_KIND, kb::WITHDRAWAL_KIND, kb::HEAD_KIND],
            "authors": [author], "#d": ids, "limit": LIMIT,
        }))
        .await?;
    let events: Vec<Event> = events.into_iter().filter(|e| e.pubkey == author).collect();
    let mut found = Mine {
        versions: Vec::new(),
        withdrawn: Vec::new(),
        heads: Vec::new(),
    };
    for event in &events {
        match event.kind {
            kb::ENTRY_KIND => {
                if let Ok(version) = kb::parse_entry(event) {
                    found.versions.push((version, event.clone()));
                }
            }
            kb::WITHDRAWAL_KIND => {
                if let Ok(w) = kb::parse_withdrawal(event) {
                    found.withdrawn.push(w.entry.id);
                }
            }
            kb::HEAD_KIND => {
                if let Ok(h) = kb::parse_head(event) {
                    found.heads.push((h.id, h.entry.id));
                }
            }
            _ => {}
        }
    }
    Ok(found)
}

fn hex_digest(text: &str) -> String {
    knowledge::digest(text.as_bytes())
        .trim_start_matches("sha256:")
        .to_string()
}

/// `kb publish`: signs each entry as a `3190` with its `30190` head, and
/// publishes a `3191` for each published version of a withdrawn entry.
/// A version already on the relay with the same document is left alone;
/// one with a different document is refused.
///
/// # Errors
///
/// A bad usage, key, or relay.
pub async fn publish(o: &Options, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let identity = load_key(key, "signing as")?;
    let chosen = entries(o)?;
    let ids: Vec<String> = chosen.iter().map(|(e, _)| e.id.clone()).collect();
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let found = mine(&mut relay, identity.pubkey(), &ids).await?;
    let (mut published, mut present, mut withdrawals, mut refused) = (0, 0, 0, 0);
    for (entry, text) in &chosen {
        let id = &entry.id;
        if entry.status == Status::Withdrawn {
            let reason = entry
                .evidence
                .iter()
                .rev()
                .find_map(|l| l.strip_prefix("withdrawn "))
                .and_then(|l| l.split_once(": ").map(|(_, r)| r.to_string()))
                .unwrap_or_else(|| "withdrawn by its author".to_string());
            let targets: Vec<&(kb::EntryVersion, Event)> = found
                .versions
                .iter()
                .filter(|(v, e)| &v.id == id && !found.withdrawn.contains(&e.id))
                .collect();
            if targets.is_empty() {
                println!("{id}: withdrawn here, with no published version to withdraw");
            }
            for (version, event) in targets {
                let parts = kb::withdrawal(event, &reason).map_err(|e| e.to_string())?;
                let withdrawal = sign(&identity, parts);
                match relay.publish(&withdrawal).await {
                    Ok(()) => {
                        withdrawals += 1;
                        println!(
                            "{id} v{}: withdrawal {} published",
                            version.version,
                            short(&withdrawal.id)
                        );
                    }
                    Err(error) => {
                        refused += 1;
                        println!("{id} v{}: {error}", version.version);
                    }
                }
            }
            continue;
        }
        let digest = hex_digest(text);
        let same_version = found
            .versions
            .iter()
            .find(|(v, _)| &v.id == id && v.version == u64::from(entry.version));
        let event = match same_version {
            Some((v, _)) if v.digest != digest => {
                refused += 1;
                println!(
                    "{id} v{}: this version is already published with other content; raise its \
version and publish again",
                    entry.version
                );
                continue;
            }
            Some((_, event)) => {
                present += 1;
                if found
                    .heads
                    .iter()
                    .any(|(head_id, at)| head_id == id && at == &event.id)
                {
                    println!("{id} v{}: already published", entry.version);
                    continue;
                }
                event.clone()
            }
            None => {
                let parts = match remote::entry_event(text) {
                    Ok(parts) => parts,
                    Err(error) => {
                        refused += 1;
                        println!("{id}: {error}");
                        continue;
                    }
                };
                let event = sign(&identity, parts);
                if let Err(error) = relay.publish(&event).await {
                    refused += 1;
                    println!("{id} v{}: {error}", entry.version);
                    continue;
                }
                published += 1;
                event
            }
        };
        let head = sign(&identity, kb::head(&event).map_err(|e| e.to_string())?);
        match relay.publish(&head).await {
            Ok(()) => println!(
                "{id} v{} [{}]: entry {} and head {} published",
                entry.version,
                entry.status,
                short(&event.id),
                short(&head.id)
            ),
            Err(error) => {
                refused += 1;
                println!("{id} v{}: the head: {error}", entry.version);
            }
        }
    }
    println!(
        "{} published, {present} already there, {}, {refused} refused",
        evidence::count(published, "entry"),
        evidence::count(withdrawals, "withdrawal")
    );
    Ok(u8::from(refused > 0))
}

/// `kb sync`: fetches entry versions, heads, and withdrawals from the
/// relay, keeps each author's current version of each entry that passes
/// every check, caches them in `dir`, and caches the evidence events that
/// cite them.
///
/// # Errors
///
/// A bad usage, key, author, or relay.
pub async fn sync(o: &Options, key: &Path, dir: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let mut authors = Vec::new();
    for author in &o.authors {
        authors.push(
            remote::parse_author(author)
                .ok_or(format!("{author} isn't an npub or a hex public key"))?,
        );
    }
    let identity = load_key(key, "authenticating to the relay as")?;
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}");
    let mut filter = json!({
        "kinds": [kb::ENTRY_KIND, kb::WITHDRAWAL_KIND, kb::HEAD_KIND], "limit": LIMIT,
    });
    if !authors.is_empty() {
        filter["authors"] = json!(authors);
    }
    let events = relay.query(filter).await?;
    let corpus = Corpus::read(&if o.corpora.is_empty() {
        default_corpora()
    } else {
        o.corpora.clone()
    });
    let result = remote::accept(&events, &corpus);
    remote::write_cache(dir, &result)?;
    let authors = result
        .accepted
        .iter()
        .map(|a| &a.author)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    println!(
        "{} events; {} entries accepted from {authors} {}",
        events.len(),
        result.accepted.len(),
        if authors == 1 { "author" } else { "authors" }
    );
    for accepted in &result.accepted {
        let status = Entry::parse(&accepted.version.document)
            .map(|e| e.status.to_string())
            .unwrap_or_default();
        println!(
            "- {} v{} by {} (its author says {status})",
            accepted.version.id,
            accepted.version.version,
            npub(&accepted.author)
        );
    }
    for (author, id) in &result.withdrawn {
        println!("- {id} by {}: withdrawn", npub(author));
    }
    for refusal in &result.refused {
        println!("- refused: {refusal}");
    }
    let cited: Vec<&str> = result
        .accepted
        .iter()
        .map(|a| a.event.id.as_str())
        .collect();
    if !cited.is_empty() {
        let events = relay
            .query(json!({"kinds": [kb::EVIDENCE_KIND], "#e": cited, "limit": LIMIT}))
            .await?;
        let store = dir.join("evidence");
        let mut kept = 0;
        for event in &events {
            match kb::parse_evidence(event) {
                Ok(_) => {
                    std::fs::create_dir_all(&store).map_err(|e| e.to_string())?;
                    let text = serde_json::to_string_pretty(event).map_err(|e| e.to_string())?;
                    std::fs::write(store.join(format!("{}.json", event.id)), text)
                        .map_err(|e| e.to_string())?;
                    kept += 1;
                }
                Err(error) => println!("- refused evidence {}: {error}", short(&event.id)),
            }
        }
        println!("{kept} evidence reports cite these entries");
    }
    println!(
        "cached in {}. Runs include other authors' entries with --kb-trust listed (for the \
authors in the trust file) or --kb-trust all (as candidates).",
        dir.display()
    );
    let incomplete = relay.incomplete();
    if incomplete > 0 {
        println!(
            "incomplete: the relay didn't return everything for {incomplete} {}; see the \
warnings above",
            if incomplete == 1 { "query" } else { "queries" }
        );
    }
    Ok(u8::from(!result.refused.is_empty() || incomplete > 0))
}

/// `kb publish-evidence`: for each published entry with paired runs, a
/// NIP-EVAL report signed by this key, as a `3189` citing the entry's
/// `3190`. The report is also kept under the evidence directory's
/// `published/`. With `--author`, the entries are that author's synced
/// ones: see [`publish_transfer`].
///
/// # Errors
///
/// A bad usage, key, or relay.
pub async fn publish_evidence(o: &Options, key: &Path) -> Result<u8, String> {
    if !o.authors.is_empty() {
        return publish_transfer(o, key).await;
    }
    let url = relay_url(o)?;
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    let chosen = entries(o)?;
    let ids: Vec<String> = chosen.iter().map(|(e, _)| e.id.clone()).collect();
    let runs = evidence::scan(&o.runs()?);
    let store = o.evidence_dir()?.join("published");
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}; {} runs recorded", runs.len());
    let found = mine(&mut relay, &me, &ids).await?;
    let evaluator = Evaluator {
        id: me.clone(),
        namespace: me.clone(),
    };
    let (mut published, mut refused) = (0, 0);
    for (entry, text) in &chosen {
        let id = &entry.id;
        if entry.status == Status::Withdrawn {
            continue;
        }
        let digest = hex_digest(text);
        let Some((_, event)) = found
            .versions
            .iter()
            .find(|(v, _)| &v.id == id && v.digest == digest)
        else {
            println!(
                "{id}: this version isn't published; run kb publish first, so the evidence can cite it"
            );
            continue;
        };
        let measured = evidence::measure(entry, &runs);
        if measured.pairs.is_empty() {
            println!("{id}: no paired tasks yet, so there's nothing to publish");
            continue;
        }
        let reference = json!({"id": event.id, "pubkey": event.pubkey, "kind": kb::ENTRY_KIND});
        let (report, artifacts) =
            evidence::report(&measured, text, &runs, &evaluator, Some(reference));
        let report_text = serde_json::to_string(&report).map_err(|e| e.to_string())?;
        let parts = kb::evidence(&report_text, std::slice::from_ref(&event.id))
            .map_err(|e| e.to_string())?;
        let signed = sign(&identity, parts);
        match relay.publish(&signed).await {
            Ok(()) => {
                published += 1;
                let path = store.join(format!("{}.json", signed.id));
                evidence::write(&path, &report, &artifacts)?;
                std::fs::write(&path, &report_text).map_err(|e| e.to_string())?;
                println!(
                    "{id} v{}: evidence {} published; {}",
                    entry.version,
                    short(&signed.id),
                    evidence::tally(&measured)
                );
            }
            Err(error) => {
                refused += 1;
                println!("{id}: {error}");
            }
        }
    }
    println!(
        "{} published, {refused} refused",
        evidence::count(published, "evidence report")
    );
    Ok(u8::from(refused > 0))
}

/// `kb publish-evidence --author KEY [ids]`: evidence about another
/// author's synced entries, the runner's half of a NIP-XP `kb-transfer`
/// completion. Each cached version is measured by its exact digest, and
/// its report cites the author's `3190` on the relay that carries that
/// digest; a version the relay doesn't have, or has withdrawn, is refused.
/// The report is signed by this key, which must not be the author's.
///
/// # Errors
///
/// A bad usage, key, author, or relay, or an entry that isn't synced.
pub async fn publish_transfer(o: &Options, key: &Path) -> Result<u8, String> {
    let url = relay_url(o)?;
    let [author] = o.authors.as_slice() else {
        return Err("name one author with --author KEY".to_string());
    };
    let author = remote::parse_author(author)
        .ok_or(format!("{author} isn't an npub or a hex public key"))?;
    let remote_dir = o
        .remote
        .clone()
        .or_else(remote::default_dir)
        .ok_or("no synced entries directory: pass --remote")?;
    let identity = load_key(key, "signing as")?;
    let me = identity.pubkey().to_string();
    if author == me {
        return Err(format!(
            "{} is your own key: evidence you sign about your own entry earns no XP under \
NIP-XP. Publish it without --author, from --dir",
            npub(&author)
        ));
    }
    let chosen = transfer::synced(&remote_dir, &author, &o.words)?;
    let ids: Vec<String> = chosen.iter().map(|s| s.entry.id.clone()).collect();
    let runs_dir = o.runs()?;
    let runs = evidence::scan(&runs_dir);
    let store = o.evidence_dir()?.join("published");
    let mut relay = Relay::open(url, &identity).await?;
    println!("connected to {url}; {} runs recorded", runs.len());
    let found = mine(&mut relay, &author, &ids).await?;
    let evaluator = Evaluator {
        id: me.clone(),
        namespace: me.clone(),
    };
    let (mut published, mut refused) = (0, 0);
    for synced in &chosen {
        let entry = &synced.entry;
        let label = format!("{} v{} by {}", entry.id, entry.version, npub(&author));
        let Some((_, event)) = found
            .versions
            .iter()
            .find(|(v, _)| v.id == entry.id && v.digest == synced.hex_digest())
        else {
            refused += 1;
            println!(
                "{label}: this relay has no version with digest {}; publish to the relay you \
synced it from, or sync again",
                entry.short_digest()
            );
            continue;
        };
        if found.withdrawn.contains(&event.id) {
            refused += 1;
            println!("{label}: its author withdrew this version, so there's nothing to cite");
            continue;
        }
        let (measured, selection) = transfer::measure(synced, &runs, &runs_dir);
        if measured.pairs.is_empty() {
            println!(
                "{label}: no paired tasks yet, so there's nothing to publish{}",
                transfer::left_out(&selection)
            );
            continue;
        }
        let cited = transfer::Synced {
            event: event.clone(),
            ..synced.clone()
        };
        let (report, artifacts) = transfer::report(&cited, &measured, &selection, &evaluator);
        let report_text = serde_json::to_string(&report).map_err(|e| e.to_string())?;
        let parts = kb::evidence(&report_text, std::slice::from_ref(&event.id))
            .map_err(|e| e.to_string())?;
        let signed = sign(&identity, parts);
        match relay.publish(&signed).await {
            Ok(()) => {
                published += 1;
                let path = store.join(format!("{}.json", signed.id));
                evidence::write(&path, &report, &artifacts)?;
                std::fs::write(&path, &report_text).map_err(|e| e.to_string())?;
                println!(
                    "{label}: evidence {} published, citing entry {}; {}: {}{}",
                    signed.id,
                    short(&event.id),
                    evidence::tally(&measured),
                    report["verdict"].as_str().unwrap_or("unknown"),
                    transfer::left_out(&selection)
                );
            }
            Err(error) => {
                refused += 1;
                println!("{label}: {error}");
            }
        }
    }
    println!(
        "{} published, {refused} refused",
        evidence::count(published, "evidence report")
    );
    Ok(u8::from(refused > 0))
}

#[cfg(test)]
pub(crate) mod tests;
