//! The Gym hall's relay reader: the EVALS board and the agents' notes.
//!
//! While the player stands in the Gym, a background thread reads the relay
//! for extension eval results ([`crate::gym_evals`]), the releases that name
//! their tools and test sets, and the Gym's notes ([`crate::gym_notes`]). It
//! verifies each record, fetches what the records cite, derives the board,
//! renders every grounded note, and hands the game thread a finished
//! [`Snapshot`]. When the player has switched on **Compare notes** and
//! another player stands in the Gym, the same thread lets the player's
//! agent speak under [`gym_notes::Policy`], signing with the player's world
//! key. Leaving the Gym stops the thread; nothing is read or sent outside
//! it.
//!
//! The game thread only passes what it knows (whether the player opted in,
//! who else is in the Gym, and each trainer's eval credit) and drains
//! snapshots, so signature checks and the network never stall a frame.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::gym_evals::{self, Board, Names};
use crate::gym_notes::{self, Context, Note, Plan, Policy, Shown};
use crate::net::{In, Link, Out};

/// The subscription for results and notes.
const SUB: &str = "verse-gym-hall";
/// How long the worker waits for events to settle before deriving.
const SETTLE: Duration = Duration::from_millis(250);
/// How often the agent considers speaking.
const SPEAK_EVERY: Duration = Duration::from_secs(1);
/// The most records fetched by ID in one visit.
const MAX_FETCH: usize = 1_000;

/// What the notes section says about itself.
pub const NOTES_NOTE: &str = "With Compare notes on, our agent trades short notes with other \
trainers' agents here about results you both published: the tool, the test set, and what \
changed. It shares only published results, never your chats or files, and it speaks at \
most four times an hour.";

/// Where the hall reads and who speaks.
#[derive(Clone)]
pub struct Config {
    /// The relay, `wss://` or a `ws://` relay on this machine.
    pub relay: String,
    /// The NIP-MV world the notes belong to.
    pub world: String,
    /// The player's world key, which signs the agent's notes.
    pub signer: RelaySigner,
}

/// A choice the player made on the board.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "do", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Switch **Compare notes** on or off.
    Notes { on: bool },
}

/// What the reader thread derived at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub board: Board,
    /// Grounded notes, newest first.
    pub notes: Vec<Shown>,
    /// Notes read but not shown: ungrounded, or still waiting on a source.
    pub held: usize,
    /// The relay has sent its stored results and notes.
    pub synced: bool,
}

enum Command {
    OptIn(bool),
    Peers(BTreeSet<String>),
    Credit(BTreeMap<String, u64>),
}

enum Update {
    Connected(bool),
    Snapshot(Box<Snapshot>),
}

struct Worker {
    tx: Sender<Command>,
    rx: Receiver<Update>,
    _stop: Stop,
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// The board's screen for the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub schema: &'static str,
    pub revision: u64,
    /// `offline`, `connecting`, `reading`, or `ready`.
    pub state: &'static str,
    /// The relay's host.
    pub relay: String,
    pub board: Board,
    /// What the board says when it has no results.
    pub empty: &'static str,
    pub note: &'static str,
    pub notes_on: bool,
    pub notes: Vec<Shown>,
    pub notes_note: &'static str,
    /// What the notes section says when it has none.
    pub notes_empty: String,
    /// Other players in the Gym now.
    pub here: usize,
}

/// The hall's state on the game thread.
pub struct Hall {
    config: Config,
    worker: Option<Worker>,
    opted_in: bool,
    peers: BTreeSet<String>,
    credit: BTreeMap<String, u64>,
    connected: bool,
    snapshot: Option<Snapshot>,
    revision: u64,
}

impl Hall {
    /// A hall that reads nothing until [`Self::set_active`].
    #[must_use]
    pub fn new(config: Config, opted_in: bool) -> Self {
        Self {
            config,
            worker: None,
            opted_in,
            peers: BTreeSet::new(),
            credit: BTreeMap::new(),
            connected: false,
            snapshot: None,
            revision: 1,
        }
    }

    /// The player's public key.
    #[must_use]
    pub fn me(&self) -> &str {
        self.config.signer.pubkey()
    }

    /// Starts reading when the player enters the Gym and stops when they
    /// leave. The last snapshot stays for the next visit.
    pub fn set_active(&mut self, active: bool) {
        match (active, self.worker.is_some()) {
            (true, false) => {
                self.worker = Some(self.start());
                self.bump();
            }
            (false, true) => {
                self.worker = None;
                self.connected = false;
                self.bump();
            }
            _ => {}
        }
    }

    /// Whether the reader runs.
    #[must_use]
    pub fn active(&self) -> bool {
        self.worker.is_some()
    }

    fn start(&self) -> Worker {
        let (tx, commands) = mpsc::channel();
        let (updates, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let config = self.config.clone();
        let state = (self.opted_in, self.peers.clone(), self.credit.clone());
        std::thread::Builder::new()
            .name("verse-gym-hall".into())
            .spawn(move || run(&config, state, &commands, &updates, &worker_stop))
            .expect("the Gym hall thread starts");
        Worker {
            tx,
            rx,
            _stop: Stop(stop),
        }
    }

    fn send(&self, command: Command) {
        if let Some(worker) = &self.worker {
            let _ = worker.tx.send(command);
        }
    }

    fn bump(&mut self) {
        self.revision += 1;
    }

    /// Whether the player switched on **Compare notes**.
    #[must_use]
    pub fn opted_in(&self) -> bool {
        self.opted_in
    }

    /// Switches **Compare notes** on or off. Off, the agent never speaks;
    /// other agents' notes still show.
    pub fn set_opt_in(&mut self, on: bool) {
        if self.opted_in != on {
            self.opted_in = on;
            self.send(Command::OptIn(on));
            self.bump();
        }
    }

    /// The other players standing in the Gym now, by public key.
    pub fn set_peers(&mut self, peers: BTreeSet<String>) {
        let peers: BTreeSet<String> = peers.into_iter().filter(|p| p != self.me()).collect();
        if self.peers != peers {
            self.peers = peers.clone();
            self.send(Command::Peers(peers));
            self.bump();
        }
    }

    /// Each trainer's XP from `eval-check` and `eval-adopt` awards.
    pub fn set_credit(&mut self, credit: BTreeMap<String, u64>) {
        if self.credit != credit {
            self.credit = credit.clone();
            self.send(Command::Credit(credit));
        }
    }

    /// Applies a choice from the board.
    pub fn act(&mut self, action: &Action) {
        match action {
            Action::Notes { on } => self.set_opt_in(*on),
        }
    }

    /// Takes whatever the reader thread sent since the last call.
    pub fn poll(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        let mut changed = false;
        for update in worker.rx.try_iter() {
            match update {
                Update::Connected(up) => {
                    changed |= self.connected != up;
                    self.connected = up;
                }
                Update::Snapshot(snapshot) => {
                    changed |= self.snapshot.as_ref() != Some(&*snapshot);
                    self.snapshot = Some(*snapshot);
                }
            }
        }
        if changed {
            self.bump();
        }
    }

    /// Blocks until a synced snapshot satisfies `done`, or `timeout`
    /// passes. For tests and headless peers, never for a frame.
    pub fn wait(&mut self, timeout: Duration, mut done: impl FnMut(&Snapshot) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            self.poll();
            if self.snapshot.as_ref().is_some_and(|s| s.synced && done(s)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// The last snapshot, once one exists.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// Changes whenever [`Self::view`] would.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The board's screen.
    #[must_use]
    pub fn view(&self) -> View {
        let state = match (&self.worker, self.connected, &self.snapshot) {
            (None, _, _) => "offline",
            (Some(_), false, _) => "connecting",
            (Some(_), true, Some(s)) if s.synced => "ready",
            (Some(_), true, _) => "reading",
        };
        let snapshot = self.snapshot.clone().unwrap_or_default();
        View {
            schema: "openagents.verse.gym-hall.v1",
            revision: self.revision,
            state,
            relay: host(&self.config.relay).to_owned(),
            board: snapshot.board,
            empty: "No results are published yet. Test a tool in chat and add the result to the Gym; it shows here.",
            note: gym_evals::NOTE,
            notes_on: self.opted_in,
            notes: snapshot.notes,
            notes_note: NOTES_NOTE,
            notes_empty: match (self.opted_in, self.peers.len()) {
                (false, _) => "Switch on Compare notes to let our agent talk with other trainers' agents here.".into(),
                (true, 0) => "No other trainers are in the Gym. Our agent speaks when one arrives.".into(),
                (true, 1) => "1 other trainer is here. Notes appear as the agents talk.".into(),
                (true, n) => format!("{n} other trainers are here. Notes appear as the agents talk."),
            },
            here: self.peers.len(),
        }
    }
}

/// The players whose live avatars stand inside the Gym at `site`, by
/// public key, from a crowd's [`crate::crowd::Crowd::shown`].
#[must_use]
pub fn peers_inside(
    site: crate::world::GymSite,
    shown: &[crate::crowd::Shown],
) -> BTreeSet<String> {
    shown
        .iter()
        .filter(|e| e.role == "avatar" && e.online && site.inside(e.pos))
        .map(|e| e.pubkey.clone())
        .collect()
}

/// A relay URL's host, for the heading.
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split('/').next().unwrap_or(rest)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn filters(world: &str, now: u64) -> Vec<serde_json::Value> {
    vec![
        json!({"kinds": [gym_evals::RESULT_KIND], "#t": [gym_evals::MARKER], "limit": gym_evals::RESULT_LIMIT}),
        json!({"kinds": [9], "#w": [world], "#z": [gym_notes::ZONE],
               "since": now.saturating_sub(gym_notes::HISTORY), "limit": gym_notes::MAX_NOTES}),
    ]
}

/// Everything the worker holds between derivations.
#[derive(Default)]
struct Store {
    results: BTreeMap<String, Event>,
    releases: BTreeMap<String, Event>,
    notes: BTreeMap<String, Note>,
    publications: BTreeMap<String, nostr::eval_ext::Publication>,
    names: Names,
    shown: BTreeSet<String>,
}

impl Store {
    /// Takes one event from the relay; `true` when it changes anything.
    fn take(&mut self, event: Event, world: &str) -> bool {
        if event.validate_id().is_err() {
            return false;
        }
        match event.kind {
            gym_evals::RESULT_KIND if !self.results.contains_key(&event.id) => {
                self.results.insert(event.id.clone(), event);
                true
            }
            gym_evals::RELEASE_KIND if !self.releases.contains_key(&event.id) => {
                self.releases.insert(event.id.clone(), event);
                true
            }
            9 if !self.notes.contains_key(&event.id) => {
                if event.validate_crypto().is_err() {
                    return false;
                }
                let Ok(note) = gym_notes::parse(&event, world) else {
                    return false;
                };
                self.notes.insert(note.id.clone(), note);
                while self.notes.len() > gym_notes::MAX_NOTES {
                    let oldest = self
                        .notes
                        .values()
                        .min_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)))
                        .map(|n| n.id.clone());
                    if let Some(id) = oldest {
                        self.notes.remove(&id);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Re-derives everything; returns the snapshot and the IDs still
    /// missing.
    fn derive(
        &mut self,
        me: &str,
        credit: &BTreeMap<String, u64>,
        synced: bool,
    ) -> (Snapshot, Vec<String>) {
        self.publications = gym_evals::verified(self.results.values());
        self.names = Names::from_events(self.releases.values());
        let mut missing: BTreeSet<String> = BTreeSet::new();
        for p in self.publications.values() {
            missing.insert(p.suite_release.id.clone());
            if let Some(r) = &p.subject_release {
                missing.insert(r.id.clone());
            }
        }
        self.shown.clear();
        let mut notes = Vec::new();
        let mut held = 0;
        for note in self.notes.values() {
            match gym_notes::check(note, &self.notes, &self.publications, &self.names, me) {
                Ok(shown) => {
                    self.shown.insert(shown.id.clone());
                    notes.push(shown);
                }
                Err(refusal) => {
                    held += 1;
                    if refusal == gym_notes::Refusal::Waiting {
                        missing.extend(gym_notes::wanted(note));
                    }
                }
            }
        }
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
        missing.retain(|id| {
            !self.results.contains_key(id)
                && !self.releases.contains_key(id)
                && !self.notes.contains_key(id)
        });
        let board = gym_evals::board(&self.publications, &self.names, credit, me);
        (
            Snapshot {
                board,
                notes,
                held,
                synced,
            },
            missing.into_iter().collect(),
        )
    }
}

fn auth(signer: &RelaySigner, url: &str, challenge: String) -> Event {
    signer.sign(
        unix_now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), url.to_owned()]),
            Tag::new(vec!["challenge".into(), challenge]),
        ],
        String::new(),
    )
}

fn run(
    config: &Config,
    (mut opted_in, mut peers, mut credit): (bool, BTreeSet<String>, BTreeMap<String, u64>),
    commands: &Receiver<Command>,
    tx: &Sender<Update>,
    stop: &AtomicBool,
) {
    let me = config.signer.pubkey().to_owned();
    let link = Link::start(&config.relay);
    link.send(Out::Subscribe {
        id: SUB.into(),
        filters: filters(&config.world, unix_now()),
        live: true,
    });
    let mut store = Store::default();
    let mut policy = Policy::default();
    let mut asked: BTreeSet<String> = BTreeSet::new();
    let mut auth_id: Option<String> = None;
    let (mut synced, mut dirty, mut changed) = (false, true, Instant::now());
    let mut fetches = 0u32;
    // Fetches still open. The agent speaks only once what it cites has
    // arrived, so a plain chat client reads the same names Verse shows.
    let mut outstanding: BTreeSet<String> = BTreeSet::new();
    // Our notes the relay hasn't answered yet.
    let mut sent: BTreeSet<String> = BTreeSet::new();
    let mut spoke = Instant::now();
    while !stop.load(Ordering::Acquire) {
        for command in commands.try_iter() {
            match command {
                Command::OptIn(on) => opted_in = on,
                Command::Peers(set) => peers = set,
                Command::Credit(map) => {
                    credit = map;
                    dirty = true;
                }
            }
        }
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
                    if store.take(*event, &config.world) {
                        dirty = true;
                        changed = Instant::now();
                    }
                }
                In::Eose(sub) => {
                    if sub == SUB {
                        synced = true;
                    } else {
                        outstanding.remove(&sub);
                        link.send(Out::Close(sub));
                    }
                    dirty = true;
                }
                In::Closed(sub, _) => {
                    outstanding.remove(&sub);
                }
                In::Auth(challenge) => {
                    let event = auth(&config.signer, &config.relay, challenge);
                    auth_id = Some(event.id.clone());
                    link.send(Out::Auth(event));
                }
                // A note the relay refused was never said: drop it.
                In::Ok { id, accepted, .. } if sent.remove(&id) => {
                    if !accepted && store.notes.remove(&id).is_some() {
                        dirty = true;
                    }
                }
                In::Ok {
                    id, accepted: true, ..
                } if auth_id.as_deref() == Some(id.as_str()) => {
                    link.send(Out::Subscribe {
                        id: SUB.into(),
                        filters: filters(&config.world, unix_now()),
                        live: true,
                    });
                    asked.clear();
                }
                _ => {}
            }
        }
        if synced && dirty && changed.elapsed() >= SETTLE {
            let (snapshot, missing) = store.derive(&me, &credit, synced);
            let want: Vec<String> = missing
                .into_iter()
                .filter(|id| asked.len() < MAX_FETCH && asked.insert(id.clone()))
                .collect();
            for chunk in want.chunks(100) {
                fetches += 1;
                let id = format!("{SUB}-refs-{fetches}");
                outstanding.insert(id.clone());
                link.send(Out::Subscribe {
                    id,
                    filters: vec![json!({"ids": chunk, "limit": chunk.len()})],
                    live: false,
                });
            }
            if tx.send(Update::Snapshot(Box::new(snapshot))).is_err() {
                return;
            }
            dirty = false;
        }
        if synced && !dirty && outstanding.is_empty() && spoke.elapsed() >= SPEAK_EVERY {
            spoke = Instant::now();
            let now = unix_now();
            let context = Context {
                me: &me,
                opted_in,
                here: true,
                peers: &peers,
                notes: &store.notes,
                shown: &store.shown,
                publications: &store.publications,
            };
            if let Some(plan) = policy.next(&context, now)
                && let Some(event) = speak(config, &store, &plan, now)
            {
                policy.record(&plan, now);
                sent.insert(event.id.clone());
                link.send(Out::Publish(event.clone()));
                if store.take(event, &config.world) {
                    dirty = true;
                    changed = Instant::now() - SETTLE;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Signs the note `plan` makes, with the text a plain NIP-C7 client shows.
fn speak(config: &Config, store: &Store, plan: &Plan, now: u64) -> Option<Event> {
    let text = match plan {
        Plan::Open { ours } => gym_notes::render_open(store.publications.get(ours)?, &store.names),
        Plan::Answer { note, ours, .. } => {
            let opener = store.notes.get(note)?;
            let theirs = store.publications.get(opener.sources.first()?)?;
            let ours = match ours {
                Some(id) => Some(store.publications.get(id)?),
                None => None,
            };
            gym_notes::render_answer(ours, theirs, &store.names)
        }
    };
    Some(config.signer.sign(
        now,
        9,
        gym_notes::tags(&config.world, &config.relay, plan),
        text,
    ))
}
