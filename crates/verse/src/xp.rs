//! Quests, XP, levels, and achievement titles, read from NIP-XP events
//! (`nips/openagents/NIP-XP.md`).
//!
//! A background thread subscribes to a relay for quests (`30193`), awards
//! (`3193`), revocations (`3194`), and `openagents.xp` achievement labels
//! (`1985`). It fetches the entries and evidence the trusted referees'
//! awards name, derives the reader's ledger with [`xp_ledger::derive`]
//! under the reader's trust list, and hands the game thread a finished
//! [`Snapshot`]. The game thread only drains snapshots, so the network and
//! the signature checks never stall a frame.
//!
//! Everything here is read-only: Verse never publishes quests, awards, or
//! labels. XP is a record of accepted work. It can't be spent, traded, or
//! converted, and a level unlocks nothing. Levels are this client's
//! reading of the ledger ([`level_of`]), not part of the protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder_ui::theme::Intensity;
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::xp;
use serde_json::json;
use xp_ledger::eval::Documents;
use xp_ledger::{
    Credit, Ledger, derive_with, own_pubkey, parse_key as parse_author, referee_key_file,
    trust_file,
};
pub use xp_ledger::{Trainers, XpTrust};

use crate::net::{In, Link, Out};

/// How close to the quest board the player must be for the prompt.
pub const BOARD_REACH: f32 = 8.0;
/// The subscription for quests, awards, revocations, and labels.
const SUB: &str = "verse-xp";
/// Most events asked for per kind group.
const LIMIT: usize = 500;
/// How long the worker waits for events to settle before deriving.
const SETTLE: Duration = Duration::from_millis(250);

/// The OpenAgents referee's public key, hex: the one referee the
/// OpenAgents app trusts. Its key lives on OpenAgents' execution host.
pub const OPENAGENTS_REFEREE: &str =
    "650a2a22df20b1567b07b7ee069e7c4a7f5569b187844ff394870c424a75b5c7";

/// A trust list with the OpenAgents referee alone and no runner list: what
/// the OpenAgents app counts. Early boards trust one referee, and say so.
#[must_use]
pub fn openagents_trust() -> XpTrust {
    XpTrust {
        referees: BTreeSet::from([OPENAGENTS_REFEREE.to_owned()]),
        runners: BTreeSet::new(),
    }
}

/// The OpenAgents *playtest* referee's public key, hex, once the owner has
/// created it (`microcoder xp playtest-keygen`, on the owner's machine).
/// Until then it is `None`, and no playtest award counts anywhere. It is a
/// separate key from [`OPENAGENTS_REFEREE`] so a reader that trusts only
/// the trainer referee sees a trainer level playtesting never touched.
pub const PLAYTEST_REFEREE: Option<&str> = None;

/// A trust list with the playtest referee alone: what the OpenAgents app
/// counts for its playtest card and Grid titles. `None` until the key
/// exists.
#[must_use]
pub fn playtest_trust() -> Option<XpTrust> {
    PLAYTEST_REFEREE.map(|referee| XpTrust {
        referees: BTreeSet::from([referee.to_owned()]),
        runners: BTreeSet::new(),
    })
}

/// The name of the level curve [`xp_to_reach`] and [`level_of`] compute.
/// Every display of a level names it, so two clients never show different
/// numbers under one name; a new curve gets a new name.
pub const CURVE: &str = "trainer-curve-v1";

/// Cumulative XP needed to reach `level`. Level 1 needs nothing; level
/// `n + 1` needs `100 · n^1.5`, rounded up: 100 XP for level 2, 283 for
/// level 3, 520 for level 4, and 800 for level 5.
#[must_use]
pub fn xp_to_reach(level: u32) -> u64 {
    if level <= 1 {
        return 0;
    }
    let n = f64::from(level - 1);
    (100.0 * n.powf(1.5)).ceil() as u64
}

/// The level `xp` reaches under [`xp_to_reach`]. Everyone starts at 1.
#[must_use]
pub fn level_of(xp: u64) -> u32 {
    let mut level = 1;
    while level < 10_000 && xp_to_reach(level + 1) <= xp {
        level += 1;
    }
    level
}

/// One quest version as the board shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct QuestRow {
    /// `<id>@<version>`.
    pub address: String,
    /// The referee's hex public key.
    pub referee: String,
    /// Whether the reader trusts the referee, so its awards count.
    pub trusted: bool,
    /// Two different events at this address: a rewritten frozen version.
    pub conflict: bool,
    pub title: String,
    /// `kb-transfer` or `reproduce`.
    pub rule: String,
    pub task: String,
    /// `reproduce`: the recipe's digest, lowercase hex.
    pub recipe: Option<String>,
    /// `reproduce`: the claim's event ID.
    pub claim: Option<String>,
    pub min_pass_rate: f64,
    pub max_usd_per_run: Option<f64>,
    pub reference: Option<xp::Reference>,
    /// XP per role, in the quest's order.
    pub split: Vec<(String, u64)>,
    pub season: xp::Season,
    /// Awards for this version on the relay, counted or not.
    pub awards: usize,
    /// Awards that count in the reader's ledger.
    pub counted: usize,
    /// Achievement labels on counted awards.
    pub titles: Vec<String>,
    /// The uniqueness policy: `first` or `per-awardee`.
    pub completions: String,
    /// The most awards the version pays, when it states one
    /// (`per-awardee` and `playtest` quests).
    pub max_awards: Option<u64>,
}

impl QuestRow {
    /// The quest's fixed award, all roles together.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.split.iter().map(|(_, xp)| xp).sum()
    }
}

/// What the reader derived from the relay at one moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// Trusted referees' quests first, then everyone else's.
    pub quests: Vec<QuestRow>,
    /// Trainer XP per hex public key: every counted award except
    /// `playtest` awards, which never feed the trainer level.
    pub totals: BTreeMap<String, u64>,
    /// Achievement titles per hex public key, on trainer awards.
    pub titles: BTreeMap<String, BTreeSet<String>>,
    /// Playtest XP per hex public key, kept apart from [`Self::totals`].
    pub playtest_totals: BTreeMap<String, u64>,
    /// Titles on counted `playtest` awards, such as `playtester`.
    pub playtest_titles: BTreeMap<String, BTreeSet<String>>,
    /// Every awardee's share of every counted award.
    pub credits: Vec<Credit>,
    /// Awards that count.
    pub counted: usize,
    pub revoked: usize,
    pub refused: usize,
    pub conflicts: usize,
    /// Trusted referees.
    pub referees: usize,
    /// Who published a trainer profile, and whether it asks to be shown.
    pub trainers: Trainers,
    /// The ledger the snapshot was derived from.
    pub ledger: Ledger,
    /// The extension evaluation publications (`3189`, `oa:ext-eval:v1`)
    /// and hosted requests the reader holds, for "what you made"
    /// ([`made`]).
    pub evals: Vec<Event>,
}

impl Snapshot {
    /// XP across `keys`, each key once.
    #[must_use]
    pub fn xp_of(&self, keys: &[String]) -> u64 {
        let keys: BTreeSet<&String> = keys.iter().collect();
        keys.iter().filter_map(|k| self.totals.get(*k)).sum()
    }

    /// Titles across `keys`.
    #[must_use]
    pub fn titles_of(&self, keys: &[String]) -> BTreeSet<String> {
        keys.iter()
            .filter_map(|k| self.titles.get(k))
            .flatten()
            .cloned()
            .collect()
    }
}

/// Derives a [`Snapshot`] from relay events under `trust`. Events may
/// repeat. Quests from untrusted referees are listed but their awards
/// never count.
#[must_use]
pub fn snapshot(events: &[Event], trust: &XpTrust) -> Snapshot {
    snapshot_with(events, &Documents::new(), trust)
}

/// [`snapshot`], with the `coder-defaults` documents `eval-adopt` awards
/// are checked against ([`xp_ledger::derive_with`]).
#[must_use]
pub fn snapshot_with(events: &[Event], documents: &Documents, trust: &XpTrust) -> Snapshot {
    let mut unique: BTreeMap<&str, &Event> = BTreeMap::new();
    for event in events {
        unique.entry(event.id.as_str()).or_insert(event);
    }
    let ledger = derive_with(events, documents, trust);

    // Counted awards: their referee, awardees, and quest.
    let mut counted: BTreeMap<&str, (&str, &str, Vec<&str>)> = BTreeMap::new();
    let playtest_awards: BTreeSet<&str> = ledger
        .credits
        .iter()
        .filter(|c| c.rule == xp::PLAYTEST)
        .map(|c| c.award.as_str())
        .collect();
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut playtest_totals: BTreeMap<String, u64> = BTreeMap::new();
    for credit in &ledger.credits {
        let bucket = if credit.rule == xp::PLAYTEST {
            &mut playtest_totals
        } else {
            &mut totals
        };
        *bucket.entry(credit.pubkey.clone()).or_default() += credit.xp;
    }
    let mut playtest_titles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for credit in &ledger.credits {
        counted
            .entry(credit.award.as_str())
            .or_insert((credit.referee.as_str(), credit.quest.as_str(), Vec::new()))
            .2
            .push(credit.pubkey.as_str());
    }

    // Achievement labels: shown only when signed by the award's referee
    // and the award counts.
    let mut titles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut quest_titles: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for event in unique.values().filter(|e| e.kind == xp::LABEL_KIND) {
        let Ok(label) = xp::parse_achievement(event) else {
            continue;
        };
        let Some((referee, quest, people)) = counted.get(label.award.as_str()) else {
            continue;
        };
        if *referee != event.pubkey {
            continue;
        }
        let bucket = if playtest_awards.contains(label.award.as_str()) {
            &mut playtest_titles
        } else {
            &mut titles
        };
        for pubkey in people {
            bucket
                .entry((*pubkey).to_owned())
                .or_default()
                .insert(label.value.clone());
        }
        quest_titles
            .entry(((*referee).to_owned(), (*quest).to_owned()))
            .or_default()
            .insert(label.value.clone());
    }

    // Awards on the relay per quest coordinate, signed by the quest's
    // referee, whether or not they count.
    let mut on_relay: BTreeMap<String, usize> = BTreeMap::new();
    for event in unique.values().filter(|e| e.kind == xp::AWARD_KIND) {
        if let Ok(award) = xp::parse_award(event)
            && award.quest.pubkey == event.pubkey
        {
            *on_relay.entry(award.coordinate).or_default() += 1;
        }
    }
    let mut counted_per: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (referee, quest, _) in counted.values() {
        *counted_per.entry((referee, quest)).or_default() += 1;
    }

    let mut rows: BTreeMap<(String, String), (QuestRow, BTreeSet<&str>)> = BTreeMap::new();
    for event in unique.values().filter(|e| e.kind == xp::QUEST_KIND) {
        let Ok(quest) = xp::parse_quest(event) else {
            continue;
        };
        let key = (event.pubkey.clone(), quest.address.clone());
        let coordinate = xp::coordinate(&event.pubkey, &quest.address);
        let entry = rows.entry(key).or_insert_with(|| {
            let row = QuestRow {
                trusted: trust.referees.contains(&event.pubkey),
                conflict: false,
                title: quest.title.clone(),
                rule: quest.acceptance.rule.clone(),
                task: quest.acceptance.task.clone(),
                recipe: quest.acceptance.recipe.clone(),
                claim: quest.acceptance.claim.as_ref().map(|c| c.id.clone()),
                min_pass_rate: quest.acceptance.min_pass_rate,
                max_usd_per_run: quest.acceptance.max_usd_per_run,
                reference: quest.reference.clone(),
                split: xp::roles(&quest.acceptance.rule)
                    .iter()
                    .filter_map(|r| quest.award.get(*r).map(|xp| ((*r).to_owned(), *xp)))
                    .collect(),
                season: quest.season.clone(),
                awards: on_relay.get(&coordinate).copied().unwrap_or(0),
                counted: counted_per
                    .get(&(event.pubkey.as_str(), quest.address.as_str()))
                    .copied()
                    .unwrap_or(0),
                titles: quest_titles
                    .get(&(event.pubkey.clone(), quest.address.clone()))
                    .map(|t| t.iter().cloned().collect())
                    .unwrap_or_default(),
                address: quest.address.clone(),
                referee: event.pubkey.clone(),
                completions: quest.completions.clone(),
                max_awards: quest.award_limit(),
            };
            (row, BTreeSet::new())
        });
        entry.1.insert(event.id.as_str());
    }
    let mut quests: Vec<QuestRow> = rows
        .into_values()
        .map(|(mut row, ids)| {
            row.conflict = ids.len() > 1;
            row
        })
        .collect();
    quests.sort_by(|a, b| {
        b.trusted
            .cmp(&a.trusted)
            .then(b.season.closes_at.cmp(&a.season.closes_at))
            .then(a.address.cmp(&b.address))
    });

    Snapshot {
        quests,
        totals,
        titles,
        playtest_totals,
        playtest_titles,
        credits: ledger.credits.clone(),
        counted: counted.len(),
        revoked: ledger.revoked.len(),
        refused: ledger.refused.len(),
        conflicts: ledger.conflicts.len(),
        referees: trust.referees.len(),
        trainers: Trainers::read(events),
        evals: unique
            .values()
            .filter(|e| {
                (e.kind == nostr::kb::EVIDENCE_KIND
                    && nostr::eval_ext::parse_publication(e).is_ok())
                    || e.kind == nostr::kinds::CJ_EXECUTION_REQUEST
            })
            .map(|e| (*e).clone())
            .collect(),
        ledger,
    }
}

/// What the trainer holding `keys` made, and where its credit stands
/// (`xp_ledger::eval::made`): the phone's `CARD-07` and Profile.
#[must_use]
pub fn made(snapshot: &Snapshot, keys: &[String]) -> xp_ledger::eval::Made {
    xp_ledger::eval::made(&snapshot.evals, &snapshot.ledger, keys)
}

/// The `sha256:` digests of the documents `release` pins that `documents`
/// lacks: its manifest, then each admission the manifest cites.
#[must_use]
pub fn wanted_documents(release: &Event, documents: &Documents) -> Vec<String> {
    let Ok(body) = nostr::ext::parse_record(release) else {
        return Vec::new();
    };
    let Ok(manifest) = nostr::contracts::parse_artifact(&body["manifest"]) else {
        return Vec::new();
    };
    let mut out = vec![manifest.digest.clone()];
    if let Some(bytes) = documents.get(&manifest.digest) {
        out.extend(
            xp_ledger::adopt::receipts_of(bytes)
                .unwrap_or_default()
                .iter()
                .filter_map(|r| nostr::contracts::parse_artifact(r).ok())
                .map(|a| a.digest),
        );
    }
    out.retain(|digest| !documents.contains_key(digest));
    out
}

/// The releases trusted `eval-adopt` awards name, among `have`.
fn adopted_releases<'a>(have: &'a BTreeMap<String, Event>, trust: &XpTrust) -> Vec<&'a Event> {
    let mut out = Vec::new();
    for event in have.values().filter(|e| e.kind == xp::AWARD_KIND) {
        if !trust.referees.contains(&event.pubkey) {
            continue;
        }
        let Ok(award) = xp::parse_award(event) else {
            continue;
        };
        for evidence in award.evidence {
            if let Some(release) = have.get(&evidence.id)
                && release.kind == nostr::ext::RELEASE_KIND
            {
                out.push(release);
            }
        }
    }
    out
}

/// Fetches one document a NIP-94 locator names over HTTPS, checking its
/// digest and size. Blocking; the reader thread calls it.
fn fetch_document(url: &str, digest: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .ok()?;
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    (bytes.len() <= xp_ledger::eval::MAX_DOCUMENT_BYTES
        && nostr::contracts::digest_bytes(&bytes) == digest)
        .then(|| bytes.to_vec())
}

/// Event IDs the trusted referees' awards name that `have` lacks: their
/// exact quest, entry, and evidence events (a reproduction's claim and
/// reproduction are its evidence).
#[must_use]
pub fn missing(have: &BTreeMap<String, Event>, trust: &XpTrust) -> BTreeSet<String> {
    let mut want = BTreeSet::new();
    for event in have.values().filter(|e| e.kind == xp::AWARD_KIND) {
        if !trust.referees.contains(&event.pubkey) {
            continue;
        }
        if let Ok(award) = xp::parse_award(event) {
            want.insert(award.quest.id);
            want.extend(award.entry.map(|e| e.id));
            want.extend(award.evidence.into_iter().map(|e| e.id));
        }
    }
    want.retain(|id| !have.contains_key(id));
    want
}

/// The reader's trust: shipped defaults when no trust file exists, otherwise
/// the explicit list in `~/.openagents/knowledge/xp-trust.json`.
/// `--referee` adds keys for this reading only.
#[must_use]
pub fn load_trust(referees: &[String]) -> (XpTrust, Option<String>) {
    let own = referee_key_file().and_then(|k| own_pubkey(&k));
    load_trust_at(trust_file().as_deref(), own.as_deref(), referees)
}

/// Loads trust from an explicit path without accessing the reader's home.
#[must_use]
pub fn load_trust_at(
    path: Option<&std::path::Path>,
    own: Option<&str>,
    referees: &[String],
) -> (XpTrust, Option<String>) {
    let mut defaults = openagents_trust();
    if let Some(own) = own {
        defaults.referees.insert(own.to_owned());
    }
    let (mut trust, mut problem) = match path.filter(|p| p.exists()) {
        Some(path) => match XpTrust::read(path) {
            Ok(trust) => (trust, None),
            Err(e) => (defaults, Some(e)),
        },
        None => (defaults, None),
    };
    for key in referees {
        match parse_author(key) {
            Some(hex) => {
                trust.referees.insert(hex);
            }
            None => problem = Some(format!("--referee {key} isn't an npub or a hex key")),
        }
    }
    (trust, problem)
}

/// Adds or removes a referee in the explicit local trust list.
///
/// # Errors
///
/// Returns an error for invalid keys, invalid existing trust, or failed writes.
pub fn edit_trust_at(
    path: &std::path::Path,
    own: Option<&str>,
    key: &str,
    add: bool,
) -> Result<XpTrust, String> {
    let key = parse_author(key).ok_or("referee must be an npub or a hex public key")?;
    let (mut trust, problem) = load_trust_at(Some(path), own, &[]);
    if let Some(problem) = problem {
        return Err(problem);
    }
    if add {
        trust.referees.insert(key);
    } else {
        trust.referees.remove(&key);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes =
        serde_json::to_vec_pretty(&json!({"referees": trust.referees, "runners": trust.runners}))
            .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
    Ok(trust)
}

/// Actionable guidance when a reader has opted out of all referees.
pub fn no_referees_message() -> String {
    format!(
        "No referee is trusted; awards do not count. Trust one with: openagents verse trust add {OPENAGENTS_REFEREE}"
    )
}

/// The public keys whose XP is shown as the player's: the Verse profile
/// key, the knowledge key that signs entries and evidence
/// (`~/.openagents/nostr/knowledge-key`), and any `--xp-key`. Only public
/// keys are read; nothing is created.
#[must_use]
pub fn my_keys(profile: Option<&str>, extra: &[String]) -> Vec<String> {
    let mut keys: Vec<String> = profile.map(str::to_owned).into_iter().collect();
    if let Some(own) = xp_ledger::key_file().and_then(|k| own_pubkey(&k)) {
        keys.push(own);
    }
    keys.extend(extra.iter().filter_map(|k| parse_author(k)));
    let mut seen = BTreeSet::new();
    keys.retain(|k| seen.insert(k.clone()));
    keys
}

enum Update {
    Connected(bool),
    Snapshot(Box<Snapshot>),
}

/// The game's handle on the XP reader thread.
pub struct Board {
    rx: Receiver<Update>,
    _stop: Stop,
    /// The relay it reads.
    pub relay: String,
    /// Whether the relay is connected.
    pub connected: bool,
    /// The latest derived snapshot, once one exists.
    pub snapshot: Option<Snapshot>,
    /// A trust-file problem to show, if any.
    pub problem: Option<String>,
}

impl Board {
    /// Starts reading `relay` under the reader's trust (see
    /// [`load_trust`]), also trusting `referees`. `signer` answers a NIP-42
    /// challenge when the relay asks for one.
    #[must_use]
    pub fn start(relay: &str, referees: &[String], signer: Option<RelaySigner>) -> Self {
        let (trust, problem) = load_trust(referees);
        Self::start_with(relay, trust, problem, signer)
    }

    /// Starts reading `relay` under `trust`.
    #[must_use]
    pub fn start_with(
        relay: &str,
        trust: XpTrust,
        problem: Option<String>,
        signer: Option<RelaySigner>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let url = relay.to_owned();
        std::thread::Builder::new()
            .name("verse-xp".into())
            .spawn(move || run(&url, &trust, signer.as_ref(), &tx, &worker_stop))
            .expect("the XP thread starts");
        Self {
            rx,
            _stop: Stop(stop),
            relay: relay.to_owned(),
            connected: false,
            snapshot: None,
            problem,
        }
    }

    /// A board that shows `snapshot` and reads nothing, for captures and
    /// tests.
    #[must_use]
    pub fn fixed(relay: &str, snapshot: Snapshot) -> Self {
        let (_, rx) = mpsc::channel();
        Self {
            rx,
            _stop: Stop(Arc::new(AtomicBool::new(false))),
            relay: relay.to_owned(),
            connected: true,
            snapshot: Some(snapshot),
            problem: None,
        }
    }

    /// Takes whatever the reader thread has sent since the last call.
    pub fn tick(&mut self) {
        for update in self.rx.try_iter() {
            match update {
                Update::Connected(up) => self.connected = up,
                Update::Snapshot(s) => self.snapshot = Some(*s),
            }
        }
    }

    /// Blocks until the snapshot has been quiet for `quiet`, or `timeout`
    /// passes. For headless captures and tests, never for a frame.
    pub fn settle(&mut self, quiet: Duration, timeout: Duration) {
        let start = Instant::now();
        let mut last = Instant::now();
        while start.elapsed() < timeout {
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Update::Connected(up)) => self.connected = up,
                Ok(Update::Snapshot(s)) => {
                    self.snapshot = Some(*s);
                    last = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.snapshot.is_some() && last.elapsed() >= quiet {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn filters() -> Vec<serde_json::Value> {
    vec![
        json!({"kinds": [xp::QUEST_KIND, xp::AWARD_KIND, xp::REVOCATION_KIND], "limit": LIMIT}),
        json!({"kinds": [xp::LABEL_KIND], "#L": [xp::LABEL_NAMESPACE], "limit": LIMIT}),
        json!({"kinds": [xp::PROFILE_KIND, xp::LINK_KIND], "limit": LIMIT}),
        // Extension eval results and checks: "what you made".
        json!({"kinds": [nostr::kb::EVIDENCE_KIND], "#t": [nostr::eval_ext::PROFILE_MARKER],
            "limit": LIMIT}),
    ]
}

/// The reader thread: gathers events, fetches what awards name, and sends
/// a snapshot whenever the events settle. Exits when the game drops its
/// [`Board`].
fn run(
    url: &str,
    trust: &XpTrust,
    signer: Option<&RelaySigner>,
    tx: &Sender<Update>,
    stop: &AtomicBool,
) {
    let link = Link::start(url);
    link.send(Out::Subscribe {
        id: SUB.into(),
        filters: filters(),
        live: true,
    });
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    let mut asked: BTreeSet<String> = BTreeSet::new();
    let mut auth_id: Option<String> = None;
    let mut synced = false;
    let mut dirty = false;
    let mut changed = Instant::now();
    let mut fetches = 0u32;
    // `coder-defaults` documents `eval-adopt` awards need, by digest, and
    // the digests already asked about.
    let mut documents = Documents::new();
    let mut asked_documents: BTreeSet<String> = BTreeSet::new();
    let mut asked_reports: BTreeSet<String> = BTreeSet::new();
    while !stop.load(Ordering::Acquire) {
        for message in link.drain() {
            match message {
                In::Connected => {
                    if tx.send(Update::Connected(true)).is_err() {
                        return;
                    }
                }
                In::Disconnected(_) => {
                    if tx.send(Update::Connected(false)).is_err() {
                        return;
                    }
                }
                In::Event { event, .. } => {
                    // A locator for a document an adoption needs: fetch
                    // and check it, and keep only the bytes.
                    if event.kind == nostr::ext::LOCATOR_KIND {
                        if let (Some(url), Some(hex)) =
                            (event.tag_values("url").next(), event.tag_values("x").next())
                        {
                            let digest = format!("sha256:{hex}");
                            if asked_documents.contains(&digest)
                                && !documents.contains_key(&digest)
                                && let Some(bytes) = fetch_document(url, &digest)
                            {
                                documents.insert(digest, bytes);
                                dirty = true;
                                changed = Instant::now();
                            }
                        }
                        continue;
                    }
                    if event.validate_id().is_ok() && !events.contains_key(&event.id) {
                        events.insert(event.id.clone(), *event);
                        dirty = true;
                        changed = Instant::now();
                    }
                }
                In::Eose(sub) => {
                    if sub == SUB {
                        synced = true;
                    } else {
                        link.send(Out::Close(sub));
                    }
                    dirty = true;
                }
                In::Auth(challenge) => {
                    if let Some(signer) = signer {
                        let event = signer.sign(
                            unix_now(),
                            22_242,
                            vec![
                                Tag::new(vec!["relay".into(), url.to_owned()]),
                                Tag::new(vec!["challenge".into(), challenge]),
                            ],
                            String::new(),
                        );
                        auth_id = Some(event.id.clone());
                        link.send(Out::Auth(event));
                    }
                }
                In::Ok {
                    id, accepted: true, ..
                } if auth_id.as_deref() == Some(id.as_str()) => {
                    // Ask again now that the relay knows who is reading.
                    link.send(Out::Subscribe {
                        id: SUB.into(),
                        filters: filters(),
                        live: true,
                    });
                    asked.clear();
                }
                _ => {}
            }
        }
        if synced && dirty && changed.elapsed() >= SETTLE {
            let want: Vec<String> = missing(&events, trust)
                .into_iter()
                .filter(|id| asked.insert(id.clone()))
                .collect();
            for chunk in want.chunks(100) {
                fetches += 1;
                link.send(Out::Subscribe {
                    id: format!("{SUB}-refs-{fetches}"),
                    filters: vec![json!({"ids": chunk, "limit": chunk.len()})],
                    live: false,
                });
            }
            let wanted: Vec<String> = adopted_releases(&events, trust)
                .into_iter()
                .flat_map(|release| wanted_documents(release, &documents))
                .filter(|digest| asked_documents.insert(digest.clone()))
                .collect();
            for chunk in wanted.chunks(50) {
                fetches += 1;
                let hexes: Vec<&str> = chunk
                    .iter()
                    .map(|d| d.trim_start_matches("sha256:"))
                    .collect();
                link.send(Out::Subscribe {
                    id: format!("{SUB}-docs-{fetches}"),
                    filters: vec![json!({"kinds": [nostr::ext::LOCATOR_KIND], "#x": hexes,
                        "limit": LIMIT})],
                    live: false,
                });
            }
            // An `eval-adopt` award's evidence names the release, the
            // result, and one check; the validation an admission cites is
            // named only by its report digest, so the publications the held
            // admissions cite are fetched by `#x` too.
            let reports: Vec<String> = xp_ledger::eval::cited_report_hexes(&documents)
                .into_iter()
                .filter(|hex| asked_reports.insert(hex.clone()))
                .collect();
            for chunk in reports.chunks(50) {
                fetches += 1;
                link.send(Out::Subscribe {
                    id: format!("{SUB}-cited-{fetches}"),
                    filters: vec![json!({"kinds": [nostr::kb::EVIDENCE_KIND], "#x": chunk,
                        "limit": LIMIT})],
                    live: false,
                });
            }
            let all: Vec<Event> = events.values().cloned().collect();
            if tx
                .send(Update::Snapshot(Box::new(snapshot_with(
                    &all, &documents, trust,
                ))))
                .is_err()
            {
                return;
            }
            dirty = false;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// A relay URL's host, for headings.
#[must_use]
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split('/').next().unwrap_or(rest)
}

fn money(usd: f64) -> String {
    if usd > 0.0 && usd < 0.01 {
        format!("${usd:.4}")
    } else {
        format!("${usd:.2}")
    }
}

fn duration(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m {:02}s", s / 60, s % 60),
        s => format!("{}h {:02}m", s / 3_600, (s % 3_600) / 60),
    }
}

fn season(s: &xp::Season, now: u64) -> String {
    let days = |t: u64| t.div_ceil(86_400);
    if now < s.opens_at {
        format!("season {}, opens in {} days", s.id, days(s.opens_at - now))
    } else if now <= s.closes_at {
        format!(
            "season {}, open {} more days",
            s.id,
            days(s.closes_at - now)
        )
    } else {
        format!("season {}, closed", s.id)
    }
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(8)]
}

/// A styled line of HUD text.
pub type Styled = (String, Intensity);

/// The XP lines at the top left of the screen.
#[must_use]
pub fn strip(board: Option<&Board>, mine: &[String]) -> Vec<Styled> {
    let Some(board) = board else {
        return vec![(
            "XP · offline: start Verse with a relay to read quests and awards".into(),
            Intensity::Quarter,
        )];
    };
    let Some(snap) = &board.snapshot else {
        let state = if board.connected {
            "reading"
        } else {
            "connecting to"
        };
        return vec![(
            format!("XP · {state} {} …", host(&board.relay)),
            Intensity::Quarter,
        )];
    };
    let xp = snap.xp_of(mine);
    let level = level_of(xp);
    let next = xp_to_reach(level + 1);
    let mut out = vec![(
        format!(
            "XP {xp} · level {level} ({CURVE}) · {} XP to level {}",
            next - xp,
            level + 1
        ),
        Intensity::Full,
    )];
    let titles = snap.titles_of(mine);
    if !titles.is_empty() {
        out.push((
            format!(
                "titles: {}",
                titles.into_iter().collect::<Vec<_>>().join(", ")
            ),
            Intensity::ThreeQuarters,
        ));
    }
    let referees = if snap.referees == 0 {
        no_referees_message()
    } else {
        format!(
            "trusting {} referee{}",
            snap.referees,
            if snap.referees == 1 { "" } else { "s" }
        )
    };
    out.push((
        format!(
            "B: quest board · {} quests · {referees} · {}",
            snap.quests.len(),
            host(&board.relay)
        ),
        Intensity::Quarter,
    ));
    out
}

/// The quest board panel's lines.
#[must_use]
pub fn board_lines(board: Option<&Board>, now: u64) -> Vec<Styled> {
    let Some(board) = board else {
        return vec![
            (
                "Verse is offline, so the board has no quests to show.".into(),
                Intensity::Half,
            ),
            (
                "Start Verse with a relay, or pass --xp-relay URL.".into(),
                Intensity::Quarter,
            ),
        ];
    };
    let mut out = Vec::new();
    if let Some(problem) = &board.problem {
        out.push((format!("trust file: {problem}"), Intensity::Full));
    }
    let Some(snap) = &board.snapshot else {
        out.push((
            format!("Reading quests from {} …", host(&board.relay)),
            Intensity::Half,
        ));
        return out;
    };
    if snap.quests.is_empty() {
        out.push((
            format!("No quests on {} yet.", host(&board.relay)),
            Intensity::Half,
        ));
    }
    for (i, q) in snap.quests.iter().enumerate() {
        if i > 0 {
            out.push((String::new(), Intensity::Quarter));
        }
        let mut title = q.title.clone();
        if !q.trusted {
            title.push_str("  [referee not trusted: its awards count for nothing here]");
        }
        if q.conflict {
            title.push_str("  [conflict: this version was published twice]");
        }
        out.push((title, Intensity::Full));
        let bar = match (&q.recipe, q.max_usd_per_run) {
            _ if q.rule == xp::PLAYTEST => format!(
                "an accepted {} contribution on a season build, by the playtest referee",
                q.task
            ),
            (Some(recipe), _) => format!(
                "reproduce the published pass from recipe {} with a run of your own",
                short(recipe)
            ),
            (None, Some(usd)) => format!(
                "pass at least {:.0}% of runs, under {} a run",
                q.min_pass_rate * 100.0,
                money(usd)
            ),
            (None, None) => format!("pass at least {:.0}% of runs", q.min_pass_rate * 100.0),
        };
        out.push((
            format!("  task {} · bar: {bar}", q.task),
            Intensity::ThreeQuarters,
        ));
        if let Some(r) = &q.reference {
            let mut parts = vec![r.label.clone()];
            if let Some(usd) = r.usd {
                parts.push(money(usd));
            }
            if let Some(s) = r.seconds {
                parts.push(duration(s));
            }
            out.push((
                format!("  reference: {}", parts.join(" · ")),
                Intensity::Half,
            ));
        }
        let split: Vec<String> = q.split.iter().map(|(r, x)| format!("{r} {x}")).collect();
        let mut awards = match (q.awards, q.counted) {
            (0, _) => "no awards yet".to_owned(),
            (n, c) => format!("{n} award{} ({c} counted)", if n == 1 { "" } else { "s" }),
        };
        if let (Some(max), true) = (q.max_awards, q.completions == xp::PER_AWARDEE) {
            awards.push_str(&format!(" · pays each trainer once, up to {max}"));
        }
        out.push((
            format!(
                "  award {} XP ({}) · {} · {awards}",
                q.total(),
                split.join(", "),
                season(&q.season, now)
            ),
            Intensity::Half,
        ));
        if !q.titles.is_empty() {
            out.push((
                format!("  achievements: {}", q.titles.join(", ")),
                Intensity::Half,
            ));
        }
        out.push((
            format!("  {} · referee {}", q.address, short(&q.referee)),
            Intensity::Quarter,
        ));
    }
    if snap.referees == 0 {
        out.push((no_referees_message(), Intensity::Half));
    }
    out.push((String::new(), Intensity::Quarter));
    out.push((
        format!(
            "ledger: {} counted · {} revoked · {} refused · {} conflicts",
            snap.counted, snap.revoked, snap.refused, snap.conflicts
        ),
        Intensity::Quarter,
    ));
    out.push((
        "XP records accepted work. It can't be spent, traded, or converted.".into(),
        Intensity::Quarter,
    ));
    out
}

/// How many quests `keys` can still earn under `snapshot` at `now`: trusted,
/// unconflicted, in season, and not yet paid out to them. A `first`
/// quest is taken by its first counted award; a `per-awardee` quest stays
/// open until it pays its `max_awards` or pays one of `keys`.
#[must_use]
pub fn open_quests(snapshot: &Snapshot, keys: &[String], now: u64) -> usize {
    let mine: BTreeSet<&String> = keys.iter().collect();
    snapshot
        .quests
        .iter()
        .filter(|q| {
            let paid_me = snapshot.credits.iter().any(|c| {
                c.quest == q.address && c.referee == q.referee && mine.contains(&c.pubkey)
            });
            let full = match q.max_awards {
                Some(max) => q.counted as u64 >= max,
                None => q.counted > 0,
            };
            q.trusted
                && !q.conflict
                && q.season.opens_at <= now
                && now <= q.season.closes_at
                && !paid_me
                && !full
        })
        .count()
}

/// A player's level for a name tag, when they have XP. The tag is short;
/// the HUD strip and the trainer card name the curve.
#[must_use]
pub fn level_tag(snapshot: Option<&Snapshot>, keys: &[String]) -> Option<String> {
    let xp = snapshot?.xp_of(keys);
    (xp > 0).then(|| format!("lv {}", level_of(xp)))
}

/// The keys whose XP counts as `key`'s trainer's: the trainer's key and
/// the keys linked to it both ways (NIP-XP `13195`), or `key` alone when
/// it belongs to no trainer.
#[must_use]
pub fn trainer_keys(snapshot: &Snapshot, key: &str) -> Vec<String> {
    snapshot
        .trainers
        .trainer_of(key)
        .map_or_else(|| vec![key.to_owned()], |t| snapshot.trainers.keys_of(t))
}

/// Another player's level for a name tag: only when `pubkey`'s trainer
/// published a profile that asks to be shown, and has XP. The level sums
/// the trainer's keys. A key that never opted in gets no level, though
/// its ledger stays computable.
#[must_use]
pub fn trainer_level_tag(snapshot: Option<&Snapshot>, pubkey: &str) -> Option<String> {
    let snapshot = snapshot?;
    if !snapshot.trainers.shown(pubkey) {
        return None;
    }
    let trainer = snapshot.trainers.trainer_of(pubkey)?;
    level_tag(Some(snapshot), &snapshot.trainers.keys_of(trainer))
}

/// A name tag: the first eight hex characters of `pubkey`, then ` · lv n`
/// when its trainer opted in with a shown profile and has XP under
/// `snapshot`, such as `650a2a22 · lv 3`.
#[must_use]
pub fn name_tag(snapshot: Option<&Snapshot>, pubkey: &str) -> String {
    let prefix = &pubkey[..pubkey.len().min(8)];
    match trainer_level_tag(snapshot, pubkey) {
        Some(level) => format!("{prefix} · {level}"),
        None => prefix.to_owned(),
    }
}

/// One counted award on a trainer card.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CardAward {
    /// The `3193` event ID.
    pub award: String,
    pub referee: String,
    /// `<id>@<version>`.
    pub quest: String,
    pub title: String,
    pub season: String,
    pub rule: String,
    pub role: String,
    pub xp: u64,
}

/// A trainer card: a key's level under the reader's trust, the curve it
/// uses, and the counted awards behind it. The awards are the credential;
/// the level is their summary.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Card {
    pub keys: Vec<String>,
    pub curve: &'static str,
    pub xp: u64,
    pub level: u32,
    /// Cumulative XP at which the next level starts.
    pub next_level_at: u64,
    /// XP still needed for the next level.
    pub to_next: u64,
    pub titles: Vec<String>,
    pub awards: Vec<CardAward>,
    /// How many referees the reader trusts.
    pub referees: usize,
}

/// The trainer card for `keys` under `snapshot`.
#[must_use]
pub fn card(snapshot: &Snapshot, keys: &[String]) -> Card {
    let xp = snapshot.xp_of(keys);
    let level = level_of(xp);
    let next_level_at = xp_to_reach(level + 1);
    let mine: BTreeSet<&String> = keys.iter().collect();
    let mut awards: Vec<CardAward> = snapshot
        .credits
        .iter()
        .filter(|c| c.xp > 0 && c.rule != xp::PLAYTEST && mine.contains(&c.pubkey))
        .map(|c| CardAward {
            award: c.award.clone(),
            referee: c.referee.clone(),
            quest: c.quest.clone(),
            title: c.title.clone(),
            season: c.season.clone(),
            rule: c.rule.clone(),
            role: c.role.clone(),
            xp: c.xp,
        })
        .collect();
    awards.sort_by(|a, b| a.quest.cmp(&b.quest).then(a.award.cmp(&b.award)));
    awards.dedup();
    Card {
        keys: keys.to_vec(),
        curve: CURVE,
        xp,
        level,
        next_level_at,
        to_next: next_level_at.saturating_sub(xp),
        titles: snapshot.titles_of(keys).into_iter().collect(),
        awards,
        referees: snapshot.referees,
    }
}

/// A playtest card: playtest XP and titles for `keys`, and what they came
/// from. It is shown next to the trainer card and never summed into the
/// trainer level: playtest XP has no curve.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlaytestCard {
    pub keys: Vec<String>,
    pub xp: u64,
    /// Such as `playtester`, `founding-playtester`, `bug-hunter`.
    pub titles: Vec<String>,
    pub awards: Vec<CardAward>,
    /// Counted feedback, bug, and design awards.
    pub accepted_reports: usize,
    /// Counted verified-fix awards.
    pub fixes_verified: usize,
    /// Counted session-script awards.
    pub sessions: usize,
    /// Counted diary awards.
    pub diaries: usize,
}

/// The playtest card for `keys` under `snapshot`, which should come from a
/// reader trusting the playtest referee ([`playtest_trust`]).
#[must_use]
pub fn playtest_card(snapshot: &Snapshot, keys: &[String]) -> PlaytestCard {
    let mine: BTreeSet<&String> = keys.iter().collect();
    let contribution = |referee: &str, quest: &str| {
        snapshot
            .quests
            .iter()
            .find(|q| q.referee == referee && q.address == quest)
            .map_or("", |q| q.task.as_str())
    };
    let mut awards: Vec<CardAward> = snapshot
        .credits
        .iter()
        .filter(|c| c.xp > 0 && c.rule == xp::PLAYTEST && mine.contains(&c.pubkey))
        .map(|c| CardAward {
            award: c.award.clone(),
            referee: c.referee.clone(),
            quest: c.quest.clone(),
            title: c.title.clone(),
            season: c.season.clone(),
            rule: c.rule.clone(),
            role: c.role.clone(),
            xp: c.xp,
        })
        .collect();
    awards.sort_by(|a, b| a.quest.cmp(&b.quest).then(a.award.cmp(&b.award)));
    awards.dedup_by(|a, b| a.award == b.award);
    let count = |kinds: &[&str]| {
        awards
            .iter()
            .filter(|a| kinds.contains(&contribution(&a.referee, &a.quest)))
            .count()
    };
    let titles: BTreeSet<String> = keys
        .iter()
        .filter_map(|k| snapshot.playtest_titles.get(k))
        .flatten()
        .cloned()
        .collect();
    PlaytestCard {
        keys: keys.to_vec(),
        xp: keys
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|k| snapshot.playtest_totals.get(k))
            .sum(),
        titles: titles.into_iter().collect(),
        accepted_reports: count(&["feedback", "bug", "design"]),
        fixes_verified: count(&["verified-fix"]),
        sessions: count(&["session"]),
        diaries: count(&["diary"]),
        awards,
    }
}

/// The playtest titles `pubkey` holds under `snapshot`, which the Grid
/// draws as shapes: `playtester`, `founding-playtester`, `bug-hunter`,
/// `fix-verifier`, and `raider`.
#[must_use]
pub fn playtest_titles(snapshot: Option<&Snapshot>, pubkey: &str) -> BTreeSet<String> {
    snapshot
        .and_then(|s| s.playtest_titles.get(pubkey))
        .cloned()
        .unwrap_or_default()
}

/// The keys a card for `key` sums: `key` and the keys linked to it both
/// ways when it is its own trainer, else `key` alone. The signer is first.
#[must_use]
pub fn card_keys(snapshot: &Snapshot, key: &str) -> Vec<String> {
    if snapshot.trainers.trainer_of(key) == Some(key) {
        snapshot.trainers.keys_of(key)
    } else {
        vec![key.to_owned()]
    }
}

/// The counted awards behind `keys`, as a card lists them, sorted.
fn card_awards(snapshot: &Snapshot, keys: &[String]) -> Vec<xp::CardAward> {
    let mine: BTreeSet<&String> = keys.iter().collect();
    let mut awards: Vec<xp::CardAward> = snapshot
        .credits
        .iter()
        .filter(|c| c.xp > 0 && mine.contains(&c.pubkey))
        .map(|c| xp::CardAward {
            id: c.award.clone(),
            pubkey: c.pubkey.clone(),
            role: c.role.clone(),
            xp: c.xp,
            quest: c.quest.clone(),
        })
        .collect();
    awards.sort_by(|a, b| (&a.quest, &a.id, &a.pubkey).cmp(&(&b.quest, &b.id, &b.pubkey)));
    awards.dedup();
    awards
}

/// The trainer card `key` would sign: its keys, XP, level under
/// [`CURVE`], and counted awards under `snapshot`, derived with `trust`
/// from `relays`.
#[must_use]
pub fn trainer_card(
    snapshot: &Snapshot,
    key: &str,
    trust: &XpTrust,
    relays: &[String],
    issued_at: u64,
) -> xp::TrainerCard {
    let keys = card_keys(snapshot, key);
    let awards = card_awards(snapshot, &keys);
    let total = snapshot.xp_of(&keys);
    xp::TrainerCard {
        curve: CURVE.to_owned(),
        relays: relays.to_vec(),
        referees: trust.referees.iter().cloned().collect(),
        runners: trust.runners.iter().cloned().collect(),
        keys,
        xp: total,
        level: u64::from(level_of(total)),
        awards,
        issued_at,
    }
}

/// What a reader derived for a card, and every way the card differs.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CardCheck {
    pub keys: Vec<String>,
    pub xp: u64,
    /// The level under the card's curve, when this client implements it.
    pub level: Option<u64>,
    pub awards: usize,
    /// Empty when the card matches.
    pub differences: Vec<String>,
}

/// Compares `card`, signed by `signer`, with what `snapshot` derives. The
/// snapshot must have been read under the card's own trust list.
#[must_use]
pub fn check_card(card: &xp::TrainerCard, signer: &str, snapshot: &Snapshot) -> CardCheck {
    let keys = card_keys(snapshot, signer);
    let total = snapshot.xp_of(&keys);
    let awards = card_awards(snapshot, &keys);
    let mut differences = Vec::new();
    let claimed: BTreeSet<&String> = card.keys.iter().collect();
    let derived: BTreeSet<&String> = keys.iter().collect();
    for key in claimed.difference(&derived) {
        differences.push(format!(
            "key {} isn't linked to the trainer both ways",
            short(key)
        ));
    }
    for key in derived.difference(&claimed) {
        differences.push(format!(
            "key {} is linked to the trainer but the card leaves it out",
            short(key)
        ));
    }
    let claimed: BTreeSet<(&str, &str, u64)> = card
        .awards
        .iter()
        .map(|a| (a.id.as_str(), a.pubkey.as_str(), a.xp))
        .collect();
    let counted: BTreeSet<(&str, &str, u64)> = awards
        .iter()
        .map(|a| (a.id.as_str(), a.pubkey.as_str(), a.xp))
        .collect();
    for (id, _, xp) in claimed.difference(&counted) {
        differences.push(format!(
            "award {} ({xp} XP) doesn't count under the card's trust list",
            short(id)
        ));
    }
    for (id, _, xp) in counted.difference(&claimed) {
        differences.push(format!(
            "award {} ({xp} XP) counts but the card leaves it out",
            short(id)
        ));
    }
    if card.xp != total {
        differences.push(format!(
            "the card claims {} XP; the relays give {total}",
            card.xp
        ));
    }
    let level = (card.curve == CURVE).then(|| u64::from(level_of(total)));
    match level {
        Some(level) if level != card.level => differences.push(format!(
            "the card claims level {}; {CURVE} gives {level}",
            card.level
        )),
        Some(_) => {}
        None => differences.push(format!(
            "the card's curve {} isn't one this reader implements, so its level wasn't checked",
            card.curve
        )),
    }
    CardCheck {
        keys,
        xp: total,
        level,
        awards: awards.len(),
        differences,
    }
}

pub mod fixture;

#[cfg(test)]
mod tests;
