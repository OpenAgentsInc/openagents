//! The Gym in chat, on the phone (wireframe revision 3; epic #9931).
//!
//! Everything a person does with the Gym goes through the one routed
//! OpenAgents chat: the worker sends typed cards and offers beside its
//! reply, and this module turns them into the phone's own cards
//! ([`crate::eval_cards`]), keeps what only the phone may hold, and does
//! what a tap asks, never more:
//!
//! - **The draft** of a test set made in chat is the conversation's newest
//!   `draft` card. The phone resends it with every turn (`draft`), with the
//!   result of its last try (`tried`); nothing about it is public until
//!   **Add to the Gym**.
//! - **Runs.** A `start_eval` offer is a card's button. Its tap starts a
//!   run where the phone decides, from typed state: the hosted runner when
//!   this build has one ([`Hosted`]), else Coder on a ready computer as a
//!   NIP-HOST task that runs `openagents ext eval`, else a plain line that
//!   says why it can't run yet. Runs are kept in the encrypted store, so a
//!   run's card survives a relaunch.
//! - **Results.** A hosted run's result is the report its runner sealed to
//!   this trainer, read with NIP-EVAL's parser; every number on a result
//!   card or sheet is that report's. A computer run's result stays in its
//!   Coder chat, and its card says so rather than guess.
//! - **Add to the Gym** opens `SCR-20`, which lists what becomes public,
//!   and publishes only on its button.
//! - **Credit** (`CARD-07`, `SCR-11.E10`) comes from the phone's own XP
//!   ledger ([`Standing`]), since the worker doesn't know the trainer.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use coder_computers::cache::Cache;
use nostr::cj_conversation::{self as cj, Card, SubjectSource, SuiteSource};
use nostr::eval_ext::{self, CaseKind};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::basic_coder::{Role, Turn};
use crate::eval_cards::{
    self as ui, Action, Actions, Bar, Button, CardView, Claim, Item, Line, Progress, Purpose,
    Section, SheetView, TestSetSource, Tone, Verdict3,
};
use crate::router::{Offer, Screen};

/// The interview's fixed gate lines in chat (`ext_eval::author::stage`,
/// `Surface::Chat`): a reply that ends with one waits for a tap on
/// **Looks good**. An exact comparison against the lines we wrote, never
/// a reading of the person's words. A test checks them against the
/// interview's own source.
pub const GATE_LINES: &[&str] = &[
    "Is that the plugin? Tap Looks good, or tell us what to change.",
    "Are these the right tests? Tap Looks good, or tell us what to change.",
    "Are these the right checks? Tap Looks good, or tell us what to change.",
    "Tap Try it once to run each test one time with and without the plugin, then tell us what to fix, or tap Looks good.",
    "Is that size right? Tap Looks good, or tell us what to change.",
];

/// What a tap on **Looks good** sends: the words the interview reads as an
/// approval.
pub const LOOKS_GOOD: &str = "Looks good";

/// What **Change it** puts in the composer.
pub const CHANGE: &str = "Change: ";

/// The most runs the phone keeps; the oldest go first.
const MAX_RUNS_KEPT: usize = 40;
/// The most bytes of one report the phone keeps, NIP-EVAL's bound.
const MAX_REPORT_BYTES: usize = eval_ext::MAX_REPORT_BYTES;

/// Where the Gym intro stands (`FLOW-01`), once a person opts into the
/// Gym with **Train Coder**. The phone records the furthest step and
/// reopens there. Before the opt-in, the Chat tab is the chat and none of
/// these steps shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstRun {
    /// Step 1 of 3: Choose your agent (`SCR-02`).
    #[default]
    Choose,
    /// The intro's end card, with **LET'S GO**.
    EndCard,
    /// Steps 2 and 3: the intro's chat, with its capability card and run.
    Chat,
    /// The intro is over; the Gym menu is reachable from the chat.
    Done,
}

/// Where a run happens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum Place {
    /// The hosted runner, by the signed request's event ID once sent, and
    /// the signed request itself, which the phone sends again to follow
    /// the run after a relaunch.
    Hosted {
        request: Option<String>,
        #[serde(default)]
        event: Option<Value>,
    },
    /// Coder on the person's computer, as a NIP-HOST task.
    Computer {
        host: String,
        label: String,
        task: String,
    },
}

/// Where a run stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunState {
    /// The tap was taken; the request is on its way.
    Starting,
    /// Admitted and waiting its turn.
    Queued,
    /// Running: tests done of planned, per side, when the runner says.
    Running {
        done: Option<u64>,
        planned: Option<u64>,
    },
    /// Finished: the outcome is on the run, or, for a computer run, in its
    /// Coder chat.
    Done,
    /// It didn't finish. `ours` when the failure was on our side.
    Failed { why: String, ours: bool },
    /// The person stopped it.
    Stopped,
    /// It couldn't start here: why, in plain words.
    Refused { why: String, connect: bool },
}

/// One test's result on each side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseOutcome {
    pub id: String,
    /// `should-fire` or `should-not-fire`.
    pub kind: String,
    pub with: Option<bool>,
    pub without: Option<bool>,
}

/// What a finished run found, from its report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub claim: Claim,
    pub cases: Vec<CaseOutcome>,
    /// The report's ArtifactRef.
    pub report: Option<Value>,
    /// The report's exact bytes, when the phone holds them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_json: Option<String>,
    /// Changes in time and cost, in plain words, apart from the verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl Outcome {
    /// Reads a report NIP-EVAL's parser accepts: its headline and verdict,
    /// and each test's pass on each side from its `case.<id>.runs_passed`
    /// measurements by the engine's majority rule.
    ///
    /// # Errors
    ///
    /// The parser's refusal, as a sentence.
    pub fn from_report(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_REPORT_BYTES {
            return Err("the report is too large".into());
        }
        let report = eval_ext::parse_report(bytes).map_err(|e| e.to_string())?;
        let raw: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let side = |arm: &str, id: &str| -> Option<bool> {
            let metric = format!("case.{id}.runs_passed");
            let found = raw["measurements"].as_array()?.iter().find(|m| {
                m["arm"].as_str() == Some(arm) && m["metric"].as_str() == Some(metric.as_str())
            })?;
            let passed = found["value"].as_f64()? as u64;
            let planned = found["denominator"].as_u64()?;
            let unknown = found["unknown_count"].as_u64().unwrap_or(0);
            let scored = planned.saturating_sub(unknown);
            if scored == 0 {
                return None;
            }
            let failed = scored.saturating_sub(passed);
            if passed * 2 > planned {
                Some(true)
            } else if (planned - failed) * 2 <= planned {
                Some(false)
            } else {
                None
            }
        };
        let cases = report
            .profile
            .cases
            .iter()
            .map(|(id, kind)| CaseOutcome {
                id: id.clone(),
                kind: kind.word().to_owned(),
                with: side("subject", id),
                without: side("baseline", id),
            })
            .collect();
        Ok(Self {
            claim: Claim::of(&report.profile.headline, report.verdict),
            cases,
            report: Some(json!({
                "digest": nostr::contracts::digest_bytes(bytes),
                "size": bytes.len(),
                "media_type": "application/json",
                "schema": nostr::kb::REPORT_SCHEMA,
            })),
            report_json: std::str::from_utf8(bytes).ok().map(str::to_owned),
            notes: Vec::new(),
        })
    }

    /// What a hosted run's result states when its sealed report can't be
    /// opened: the headline and verdict, and no test-by-test marks.
    pub fn from_output(output: &eval_ext::hosted::RunOutput) -> Self {
        Self {
            claim: Claim::of(&output.headline, output.verdict),
            cases: Vec::new(),
            report: Some(json!({
                "digest": output.report.digest,
                "size": output.report.size,
                "media_type": output.report.media_type,
                "schema": nostr::kb::REPORT_SCHEMA,
            })),
            report_json: None,
            notes: output.notes.clone(),
        }
    }

    /// The `tried` a request carries for the interview to read.
    pub fn tried(&self, runs: u64) -> Value {
        json!({
            "runs": runs,
            "with": self.claim.with,
            "without": self.claim.without,
            "total": self.claim.total,
            "verdict": self.claim.verdict.word(),
            "report": self.report,
            "cases": self.cases.iter().map(|case| json!({
                "id": case.id,
                "kind": case.kind,
                "with": case.with,
                "without": case.without,
                "failing": [],
            })).collect::<Vec<_>>(),
        })
    }
}

/// Where adding a run's result to the Gym stands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PublishState {
    #[default]
    None,
    /// On its way.
    Publishing,
    /// Public: the `3189` event, when the phone knows it.
    Published { event: Option<String> },
    /// It failed; the result stays on the phone.
    Failed { why: String },
}

/// The most result IDs a request's `skip` carries.
pub const MAX_SKIP: usize = 32;

/// One run the person started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    /// The conversation it started in, and the reply whose card started it.
    pub talk: String,
    pub turn: usize,
    /// The tool's name, as its card said it.
    pub tool: String,
    pub purpose: Purpose,
    /// The `start_eval` offer body it ran.
    pub offer: Value,
    /// The draft it ran, when its test set is the chat's draft.
    #[serde(default)]
    pub draft: Option<Value>,
    pub cases: u64,
    pub runs: u64,
    pub arms: u64,
    #[serde(default)]
    pub place: Option<Place>,
    pub state: RunState,
    pub started_at: u64,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub publish: PublishState,
    /// Started from the first-run chat.
    #[serde(default)]
    pub first: bool,
}

impl Run {
    fn running(&self) -> bool {
        matches!(
            self.state,
            RunState::Starting | RunState::Queued | RunState::Running { .. }
        )
    }

    fn pilot(&self) -> bool {
        matches!(self.purpose, Purpose::Try) || (self.runs <= 1 && self.draft.is_some())
    }

    fn check(&self) -> Option<Verdict3> {
        match &self.purpose {
            Purpose::Check { claim, .. } => Some(claim.verdict),
            _ => None,
        }
    }
}

/// What the XP ledger says about this trainer, from the trainer reader.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    /// The trainer's name, as "Trainer 7KQ".
    pub name: String,
    pub public_hex: String,
    /// Whether the ledger has been read at least once.
    pub read: bool,
    pub xp: u64,
    pub level: u32,
    /// XP where this level started and where the next starts.
    pub level_at: u64,
    pub next_at: u64,
    pub titles: Vec<String>,
    /// Test sets this trainer published: results run on each, XP.
    pub suites: Vec<(String, usize, u64)>,
    /// Results and checks: `(publication, verdict, confirmed_by, standing,
    /// xp, is_check)`.
    pub results: Vec<MadeRow>,
    /// Tools Coder adopted: `(title, xp)`.
    pub adoptions: Vec<(String, u64)>,
    /// Results that meet the rule and wait for an award.
    pub pending: usize,
    /// XP from `eval-check` and `eval-adopt`.
    pub eval_xp: u64,
    /// The quest shares, when a trusted quest record states them:
    /// `eval-check`'s checker and evaluator.
    pub checker_xp: Option<u64>,
    pub evaluator_xp: Option<u64>,
}

/// One result or check on "What you made".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MadeRow {
    pub id: String,
    pub verdict: String,
    pub confirmed_by: usize,
    /// `awarded`, `pending`, `waiting`, `disputed`, or `no-credit`.
    pub standing: String,
    pub xp: u64,
    pub check: bool,
    /// The test set's NIP-EXT release it ran on.
    pub suite: String,
}

impl Standing {
    /// Whether the ledger already pays these keys as checker on the test
    /// set `suite` for a check other than `except`. NIP-XP pays a key once
    /// per role per test set version
    /// (`eval-check:<season>:<suite release>:checker:<pubkey>`), so another
    /// check of that test set earns its checker no XP (#9948).
    pub fn checked_suite(&self, suite: &str, except: Option<&str>) -> bool {
        self.results
            .iter()
            .any(|r| r.check && r.xp > 0 && r.suite == suite && Some(r.id.as_str()) != except)
    }
}

/// A sheet over the chat or the menu.
#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    Result { run: String },
    Publish { run: String },
    TestSet(TestSetSource),
    LevelUp { level: u32 },
    Profile,
    Stop { run: String },
}

/// The hosted runner, as the phone reaches it: a NIP-CJ execution worker
/// (`25920` → `27020` → `26920`) whose target is the `ext-eval` program.
/// Every request is signed by the trainer's world key, so the requester
/// who earns credit is this trainer.
pub trait Hosted: Send + Sync {
    /// Send the signed request for `run` and follow it into `live`.
    fn start(
        &self,
        world: SecretKey,
        run: HostedRun,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    /// Follow a run requested before a relaunch: `event` is the signed
    /// request, sent again.
    fn resume(
        &self,
        world: SecretKey,
        event: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    /// Ask the runner to stop the run `event` requested.
    fn stop(&self, world: SecretKey, event: Value) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    /// The publish control: the runner publishes the test set and the
    /// signed result, naming this trainer.
    fn publish(
        &self,
        world: SecretKey,
        request: String,
        report: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// A hosted run's request, as the phone asks for it.
#[derive(Clone, Debug, PartialEq)]
pub struct HostedRun {
    /// The `start_eval` offer body: the suite and subject it names.
    pub offer: Value,
    /// The chat's draft, when the suite is the draft.
    pub draft: Option<Value>,
    pub runs: u64,
    /// For a check, the result it checks.
    pub check: Option<Value>,
}

/// What a hosted run's follower has seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    /// The signed request's event ID, once sent, and the request itself.
    pub request: Option<String>,
    pub event: Option<Value>,
    pub queued: bool,
    pub done: Option<u64>,
    pub planned: Option<u64>,
    /// The result, or why it failed and whether on our side.
    pub outcome: Option<Result<Outcome, (String, bool)>>,
    /// A publish control's answer: the `3189` ID, or why not.
    pub published: Option<Result<Option<String>, String>>,
}

/// What a tap asks the Coder tab to do beyond the Gym's own state.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Nothing more.
    None,
    /// Send `text` as the person's message in `talk`.
    Say { talk: String, text: String },
    /// Start a new chat with `text`.
    Fresh { text: String },
    /// Put `text` in the open chat's composer.
    Compose { text: String },
    /// Start a computer run: create a Coder task with `prompt` on a ready
    /// computer, then call [`Gym::on_computer`].
    Computer { run: String, prompt: String },
    /// Send `text` to a run's Coder task on its computer: a follow-up, or,
    /// with `stop`, an interrupt.
    Command {
        host: String,
        task: String,
        text: String,
        stop: bool,
    },
    /// Open a run's Coder chat.
    OpenCoder { host: String, task: String },
    /// Account > Computers.
    ConnectComputer,
    /// Open the chat, from the menu.
    OpenChat { talk: Option<String> },
    /// The main menu.
    Menu,
    /// The Gym in the Verse.
    VerseGym,
}

/// What the Gym keeps across launches.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Saved {
    /// The person opted into the Gym (**Train Coder**, from the Verse's
    /// Gym board or Account). Until then the chat volunteers no Gym
    /// starter, card, or intro.
    #[serde(default)]
    gym: bool,
    #[serde(default)]
    first_run: FirstRun,
    /// The intro's chat.
    #[serde(default)]
    first_talk: Option<String>,
    #[serde(default)]
    runs: Vec<Run>,
    /// The highest level this phone has shown, for `SCR-06`.
    #[serde(default)]
    level_seen: Option<u32>,
    /// The eval XP this phone has shown, for the menu's credit line.
    #[serde(default)]
    eval_xp_seen: Option<u64>,
}

/// Facts the tab passes when it draws a chat's cards.
#[derive(Clone, Debug, Default)]
pub struct Here {
    /// The chat's reply is streaming.
    pub busy: bool,
}

/// The phone's Gym state.
pub struct Gym {
    store: Option<Cache>,
    saved: Saved,
    pub actions: Actions,
    /// This pass's cards, by ID.
    cards: BTreeMap<String, CardView>,
    pub sheet: Option<Sheet>,
    /// A share sheet for the host to open, once.
    share: Option<String>,
    hosted: Option<Arc<dyn Hosted>>,
    runtime: Option<tokio::runtime::Handle>,
    lives: BTreeMap<String, Arc<Mutex<Live>>>,
    /// The trainer's world key, for signing hosted requests.
    world: Option<SecretKey>,
    pub standing: Standing,
    /// A plain line under a card whose button couldn't act.
    pub notice: Option<String>,
    /// The Gym menu is on screen instead of the chat. Never at launch: the
    /// Chat tab opens on the chat.
    pub on_menu: bool,
    /// The Gym is hidden in this build ([`Gym::hide`]): the phone's
    /// release gate (`docs/mobile/1.0-audit.md`).
    hidden: bool,
    /// Cards already shown, so the playtest log records each once.
    seen: std::collections::BTreeSet<String>,
    /// Structural events for the playtest log, taken by the app.
    logged: Vec<playtest::session::Code>,
    /// The trainer key is read only when a hosted run needs it
    /// ([`Gym::wait_for_world`], the desktop): the runs waiting for it.
    lazy_world: bool,
    awaiting_world: Vec<String>,
    /// Why the trainer key can't be had this session, once the app said
    /// so ([`Gym::world_unavailable`]): hosted runs refuse with it at once.
    world_denied: Option<String>,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

impl Gym {
    /// A Gym kept in `store`, with runs on `hosted` when this build has
    /// one.
    pub fn new(
        store: Option<Cache>,
        hosted: Option<Arc<dyn Hosted>>,
        runtime: Option<tokio::runtime::Handle>,
    ) -> Self {
        let saved: Saved = store
            .as_ref()
            .and_then(|store| store.read("gym").ok().flatten())
            .unwrap_or_default();
        Self {
            store,
            saved,
            actions: Actions::default(),
            cards: BTreeMap::new(),
            sheet: None,
            share: None,
            hosted,
            runtime,
            lives: BTreeMap::new(),
            world: None,
            standing: Standing::default(),
            notice: None,
            on_menu: false,
            hidden: false,
            seen: std::collections::BTreeSet::new(),
            logged: Vec::new(),
            lazy_world: false,
            awaiting_world: Vec::new(),
            world_denied: None,
        }
    }

    /// Read the trainer key only when a hosted run needs it: a hosted
    /// start without it waits (the run shows Starting) and
    /// [`Gym::wants_world`] asks the app for it. The desktop does this so
    /// opening the app or a chat never asks the keychain (#10096).
    pub fn wait_for_world(&mut self) {
        self.lazy_world = true;
    }

    /// A hosted run is waiting for the trainer key: the app should read it
    /// now, then call [`Gym::set_world`] or [`Gym::world_unavailable`].
    pub fn wants_world(&self) -> bool {
        self.world.is_none() && self.world_denied.is_none() && !self.awaiting_world.is_empty()
    }

    /// The trainer key couldn't be read this session (denied, cancelled,
    /// or broken): the waiting runs, and every hosted start after, refuse
    /// with `why`, a plain line. Nothing asks for it again.
    pub fn world_unavailable(&mut self, why: &str) {
        self.world_denied = Some(why.to_owned());
        for id in std::mem::take(&mut self.awaiting_world) {
            if let Some(run) = self.run_mut(&id) {
                run.state = RunState::Refused {
                    why: why.to_owned(),
                    connect: false,
                };
            }
        }
        self.save();
    }

    /// Keep this Gym in `store` from now on, once its key is known: what
    /// was saved there comes back, with this session's runs and steps on
    /// top, and it's all saved.
    pub fn attach_store(&mut self, store: Cache) {
        let mut saved: Saved = store.read("gym").ok().flatten().unwrap_or_default();
        let session = std::mem::take(&mut self.saved);
        saved.gym |= session.gym;
        saved.first_run = saved.first_run.max(session.first_run);
        if session.first_talk.is_some() {
            saved.first_talk = session.first_talk;
        }
        saved
            .runs
            .retain(|run| session.runs.iter().all(|new| new.id != run.id));
        saved.runs.extend(session.runs);
        while saved.runs.len() > MAX_RUNS_KEPT {
            saved.runs.remove(0);
        }
        saved.level_seen = saved.level_seen.max(session.level_seen);
        saved.eval_xp_seen = saved.eval_xp_seen.max(session.eval_xp_seen);
        self.saved = saved;
        self.store = Some(store);
        self.save();
    }

    /// The playtest log's events since the last call.
    pub fn take_logged(&mut self) -> Vec<playtest::session::Code> {
        std::mem::take(&mut self.logged)
    }

    /// A card is on screen: log its kind the first time.
    fn shown(&mut self, id: &str, kind: &str) {
        use playtest::session::Code;
        if !self.seen.insert(id.to_owned()) {
            return;
        }
        self.logged.push(match kind {
            "tool" => Code::ToolCard,
            "draft" => Code::DraftCard,
            "run" => Code::RunCard,
            "result" => Code::ResultCard,
            "news" => Code::NewsCard,
            "check" => Code::CheckCard,
            "capability" => Code::CapabilityCard,
            _ => Code::CreditCard,
        });
    }

    /// No store, no runner: tests of other surfaces.
    pub fn empty() -> Self {
        Self::new(None, None, None)
    }

    fn save(&self) {
        if let Some(store) = &self.store {
            let _ = store.write("gym", &self.saved);
        }
    }

    pub fn first_run(&self) -> FirstRun {
        self.saved.first_run
    }

    pub fn first_talk(&self) -> Option<&str> {
        self.saved.first_talk.as_deref()
    }

    pub fn set_first_run(&mut self, step: FirstRun) {
        if step > self.saved.first_run || step == FirstRun::Done {
            self.saved.first_run = step;
            self.save();
        }
    }

    /// The person opted into the Gym with **Train Coder**.
    pub fn opted_in(&self) -> bool {
        self.saved.gym && !self.hidden
    }

    /// Hide the Gym in this build, as the phone's release gate does
    /// (`docs/mobile/1.0-audit.md`): no intro, menu, sheet, or card shows
    /// and nothing opts in, while a person's earlier opt-in stays saved for
    /// a build that shows the Gym again.
    pub fn hide(&mut self) {
        self.hidden = true;
        self.on_menu = false;
        self.sheet = None;
    }

    /// The Gym is hidden in this build ([`Gym::hide`]).
    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// **Train Coder**: opt into the Gym. The intro opens at its furthest
    /// step (step 1 on the first opt-in); the menu waits behind the chat.
    pub fn opt_in(&mut self) {
        if self.hidden {
            return;
        }
        self.saved.gym = true;
        self.on_menu = false;
        self.save();
    }

    /// **Not now** on the intro: back to the chat, with nothing of the
    /// Gym volunteered until the next **Train Coder**.
    pub fn opt_out(&mut self) {
        self.saved.gym = false;
        self.on_menu = false;
        self.save();
    }

    /// Start the Gym intro at a named step, for simulator screenshots;
    /// every step opts in.
    pub fn set_start(&mut self, step: &str) {
        if self.hidden {
            return;
        }
        let step = match step {
            "choose" => FirstRun::Choose,
            "end_card" => FirstRun::EndCard,
            "chat" => FirstRun::Chat,
            "done" => FirstRun::Done,
            _ => return,
        };
        self.saved.gym = true;
        self.saved.first_run = step;
        self.on_menu = false;
        self.save();
    }

    pub fn set_first_talk(&mut self, talk: &str) {
        self.saved.first_talk = Some(talk.to_owned());
        self.save();
    }

    /// The trainer's world key, which signs hosted requests and names the
    /// trainer. Kept in memory only.
    pub fn set_world(&mut self, world: SecretKey) {
        let first = self.world.is_none();
        self.world = Some(world);
        if self.standing.public_hex.is_empty() {
            let (key, _) = world.x_only_public_key(&secp256k1::Secp256k1::new());
            self.standing.public_hex = key.to_string();
            self.standing.name = ui::trainer_name(&self.standing.public_hex);
        }
        if first {
            self.resume();
            // The hosted runs that waited for the key start now.
            for id in std::mem::take(&mut self.awaiting_world) {
                let Some(run) = self.run(&id).cloned() else {
                    continue;
                };
                self.saved.runs.retain(|kept| kept.id != id);
                let _ = self.start(
                    &run.talk,
                    run.turn,
                    &run.offer,
                    &run.tool,
                    run.purpose,
                    run.draft,
                    None,
                    Some(run.runs),
                );
            }
        }
    }

    /// Every run kept, oldest first.
    pub fn runs(&self) -> &[Run] {
        &self.saved.runs
    }

    pub fn run(&self, id: &str) -> Option<&Run> {
        self.saved.runs.iter().find(|run| run.id == id)
    }

    fn run_mut(&mut self, id: &str) -> Option<&mut Run> {
        self.saved.runs.iter_mut().find(|run| run.id == id)
    }

    /// The newest run of `talk`.
    pub fn talk_run(&self, talk: &str) -> Option<&Run> {
        self.saved.runs.iter().rev().find(|run| run.talk == talk)
    }

    /// The newest run with a result: what `See your result` shows.
    pub fn latest_result(&self) -> Option<&Run> {
        self.saved
            .runs
            .iter()
            .rev()
            .find(|run| run.state == RunState::Done)
    }

    /// A run still going, anywhere.
    pub fn active(&self) -> Option<&Run> {
        self.saved.runs.iter().rev().find(|run| run.running())
    }

    /// The run the menu's next step is about: one still running, or a
    /// full result not yet added to the Gym. CHAT reopens its chat.
    pub fn waiting(&self) -> Option<&Run> {
        self.active().or_else(|| {
            self.latest_result()
                .filter(|run| !run.pilot() && run.publish == PublishState::None)
        })
    }

    /// The share sheet to open, once.
    pub fn take_share(&mut self) -> Option<String> {
        self.share.take()
    }

    /// Follow hosted runs requested before a relaunch.
    fn resume(&mut self) {
        let (Some(hosted), Some(runtime), Some(world)) =
            (self.hosted.clone(), self.runtime.clone(), self.world)
        else {
            return;
        };
        for run in &self.saved.runs {
            if !run.running() {
                continue;
            }
            if let Some(Place::Hosted {
                request: Some(request),
                event: Some(event),
            }) = &run.place
            {
                let live = Arc::new(Mutex::new(Live {
                    request: Some(request.clone()),
                    event: Some(event.clone()),
                    ..Live::default()
                }));
                self.lives.insert(run.id.clone(), live.clone());
                runtime.spawn(rung(hosted.resume(world, event.clone(), live)));
            }
        }
    }

    /// The draft of `talk`: the newest `draft` card in its replies.
    pub fn draft_of(turns: &[Turn]) -> Option<Value> {
        turns
            .iter()
            .rev()
            .filter_map(|turn| turn.meta.as_ref())
            .flat_map(|meta| meta.cards.iter().rev())
            .find(|card| card["card"].as_str() == Some("draft"))
            .map(|card| card["draft"].clone())
            .filter(|draft| cj::parse_draft(draft).is_ok())
    }

    /// The results a check must not be offered: the ones this trainer
    /// published (from this phone, or read from the ledger after a
    /// reinstall) and the ones it already checked, newest first, at most
    /// [`MAX_SKIP`]. Public event IDs only; the request's `skip`.
    pub fn skip(&self) -> Vec<String> {
        let hex = |id: &str| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit());
        let mut skip: Vec<String> = vec![];
        let mut add = |id: &str| {
            let id = id.to_ascii_lowercase();
            if hex(&id) && !skip.contains(&id) && skip.len() < MAX_SKIP {
                skip.push(id);
            }
        };
        for run in self.saved.runs.iter().rev() {
            match (&run.purpose, &run.publish) {
                (Purpose::Check { publication, .. }, _) => {
                    if let Some(id) = publication["id"].as_str() {
                        add(id);
                    }
                }
                (_, PublishState::Published { event: Some(id) }) => add(id),
                _ => {}
            }
        }
        for row in self.standing.results.iter().filter(|row| !row.check) {
            add(&row.id);
        }
        skip
    }

    /// What the next turn in `talk` carries: its draft, and the result of
    /// the newest finished run of that same draft.
    pub fn context_for(&self, talk: &str, turns: &[Turn]) -> (Option<Value>, Option<Value>) {
        let Some(draft) = Self::draft_of(turns) else {
            return (None, None);
        };
        let tried = self
            .saved
            .runs
            .iter()
            .rev()
            .filter(|run| run.talk == talk && run.draft.as_ref() == Some(&draft))
            .find_map(|run| run.outcome.as_ref().map(|o| o.tried(run.runs)));
        (Some(draft), tried)
    }

    /// Take what hosted followers saw, and computer tasks' phases from
    /// `phase` (`(host, task)` → the task's phase). Returns whether a run
    /// changed.
    pub fn settle(
        &mut self,
        phase: &dyn Fn(&str, &str) -> Option<nostr::activity_summary::Phase>,
    ) -> bool {
        use nostr::activity_summary::Phase;
        let mut changed = false;
        let lives: Vec<(String, Live)> = self
            .lives
            .iter()
            .map(|(id, live)| (id.clone(), crate::basic_coder::lock(live).clone()))
            .collect();
        for (id, live) in lives {
            let Some(run) = self.run_mut(&id) else {
                continue;
            };
            let before = (run.state.clone(), run.publish.clone());
            if let Some(request) = &live.request
                && let Some(Place::Hosted {
                    request: slot,
                    event,
                }) = run.place.as_mut()
                && slot.as_deref() != Some(request)
            {
                *slot = Some(request.clone());
                event.clone_from(&live.event);
                changed = true;
            }
            if run.running() {
                match &live.outcome {
                    Some(Ok(outcome)) => {
                        run.outcome = Some(outcome.clone());
                        run.state = RunState::Done;
                    }
                    Some(Err((why, ours))) => {
                        run.state = RunState::Failed {
                            why: why.clone(),
                            ours: *ours,
                        };
                    }
                    None if live.done.is_some() || live.planned.is_some() => {
                        run.state = RunState::Running {
                            done: live.done,
                            planned: live.planned,
                        };
                    }
                    None if live.queued => run.state = RunState::Queued,
                    None => {}
                }
            }
            if run.publish == PublishState::Publishing {
                match &live.published {
                    Some(Ok(event)) => {
                        run.publish = PublishState::Published {
                            event: event.clone(),
                        };
                    }
                    Some(Err(why)) => {
                        run.publish = PublishState::Failed { why: why.clone() };
                    }
                    None => {}
                }
            }
            changed |= before != (run.state.clone(), run.publish.clone());
        }
        for run in &mut self.saved.runs {
            let Some(Place::Computer { host, task, .. }) = &run.place else {
                continue;
            };
            if !run.running() {
                continue;
            }
            let next = match phase(host, task) {
                Some(Phase::Queued) => RunState::Queued,
                Some(Phase::Running | Phase::Waiting) => RunState::Running {
                    done: None,
                    planned: None,
                },
                Some(Phase::Completed) => RunState::Done,
                Some(Phase::Failed) => RunState::Failed {
                    why: "Coder couldn't finish the tests on your computer.".into(),
                    ours: false,
                },
                Some(Phase::Cancelled) => RunState::Stopped,
                Some(Phase::Unknown) | None => continue,
            };
            if next != run.state {
                run.state = next;
                changed = true;
            }
        }
        // A run that ended is done with its follower.
        let ended: Vec<String> = self
            .lives
            .keys()
            .filter(|id| {
                self.run(id)
                    .is_none_or(|run| !run.running() && run.publish != PublishState::Publishing)
            })
            .cloned()
            .collect();
        for id in ended {
            self.lives.remove(&id);
        }
        if changed {
            self.save();
        }
        changed
    }

    /// Start a run for a `start_eval` offer, deciding where from typed
    /// state: hosted when wired and the offer fits it, else a ready
    /// computer for a published test set, else refuse in plain words.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        talk: &str,
        turn: usize,
        offer: &Value,
        tool: &str,
        purpose: Purpose,
        draft: Option<Value>,
        computer: Option<&str>,
        runs_override: Option<u64>,
    ) -> Effect {
        let Some(start) = Offer::StartEval {
            body: offer.clone(),
        }
        .start_eval() else {
            return Effect::None;
        };
        // One run at a time: a second tap on a running card does nothing.
        if self
            .saved
            .runs
            .iter()
            .any(|run| run.running() && run.talk == talk && run.turn == turn)
        {
            return Effect::None;
        }
        let draft = match start.suite {
            SuiteSource::Draft => draft,
            SuiteSource::Published(_) => None,
        };
        let runs = runs_override.unwrap_or(start.size.runs);
        let id = uuid::Uuid::new_v4().simple().to_string();
        let first = self.saved.first_run == FirstRun::Chat
            && self.saved.first_talk.as_deref() == Some(talk);
        let mut run = Run {
            id: id.clone(),
            talk: talk.to_owned(),
            turn,
            tool: tool.to_owned(),
            purpose: purpose.clone(),
            offer: offer.clone(),
            draft: draft.clone(),
            cases: start.size.cases,
            runs,
            arms: start.size.arms,
            place: None,
            state: RunState::Starting,
            started_at: unix_now(),
            outcome: None,
            publish: PublishState::None,
            first,
        };
        let fits_size =
            eval_ext::check_hosted_size(start.size.cases, runs, start.size.arms).is_ok();
        let hosted_fits = fits_size && (start.suite != SuiteSource::Draft || draft.is_some());
        // Why our computers won't take it, in plain words: its size, a
        // draft this phone no longer has, or no runner in this build.
        let limit = format!(
            "at most {} tests and {} runs",
            eval_ext::HOSTED_MAX_CASES,
            eval_ext::HOSTED_MAX_RUNS
        );
        let effect = match (&self.hosted, hosted_fits) {
            (Some(hosted), true) => match (self.world, &self.runtime) {
                (Some(world), Some(runtime)) => {
                    run.place = Some(Place::Hosted {
                        request: None,
                        event: None,
                    });
                    let live = Arc::new(Mutex::new(Live::default()));
                    self.lives.insert(id.clone(), live.clone());
                    let check = match &purpose {
                        Purpose::Check { publication, .. } => Some(publication.clone()),
                        _ => None,
                    };
                    runtime.spawn(rung(hosted.start(
                        world,
                        HostedRun {
                            offer: offer.clone(),
                            draft,
                            runs,
                            check,
                        },
                        live,
                    )));
                    Effect::None
                }
                (None, Some(_)) if self.lazy_world && self.world_denied.is_none() => {
                    // The key is read now, not at launch; the run starts
                    // once it is.
                    self.awaiting_world.push(id.clone());
                    Effect::None
                }
                _ => {
                    run.state = RunState::Refused {
                        why: self.world_denied.clone().unwrap_or_else(|| {
                            "We couldn't read your trainer name on this device. Try again in a moment.".into()
                        }),
                        connect: false,
                    };
                    Effect::None
                }
            },
            _ => match (&start.suite, computer, &purpose) {
                // A check runs where it can wait for Add to the Gym: on our
                // computers. `openagents ext eval check` publishes as it
                // ends, which would skip the confirmation.
                (_, _, Purpose::Check { .. }) => {
                    run.state = RunState::Refused {
                        why: if fits_size {
                            "Our computers can't take this check right now. Try again in a moment."
                                .into()
                        } else {
                            format!("This check is more than our computers run: {limit}.")
                        },
                        connect: false,
                    };
                    Effect::None
                }
                (SuiteSource::Published(_), Some(_), _) => Effect::Computer {
                    run: id.clone(),
                    prompt: computer_prompt(&id, tool, &start.subject, runs),
                },
                (SuiteSource::Draft, _, _) => {
                    run.state = RunState::Refused {
                        why: if !fits_size {
                            format!(
                                "These tests are more than our computers run: {limit}. We'll keep your draft here."
                            )
                        } else if draft.is_none() {
                            "This phone no longer has these tests' draft. Ask in the chat and we'll make them again.".into()
                        } else {
                            "Our computers can't take these tests right now. We'll keep your draft here.".into()
                        },
                        connect: false,
                    };
                    Effect::None
                }
                (SuiteSource::Published(_), None, _) => {
                    run.state = RunState::Refused {
                        why: if fits_size {
                            "Our computers can't take these tests right now. Connect a computer and we'll run them there with Coder.".into()
                        } else {
                            format!(
                                "These tests are more than our computers run: {limit}. Connect a computer and we'll run them there with Coder."
                            )
                        },
                        connect: true,
                    };
                    Effect::None
                }
            },
        };
        // A run waiting for the trainer key is logged when it starts.
        let waiting = self.awaiting_world.last() == Some(&id);
        if !matches!(run.state, RunState::Refused { .. }) && !waiting {
            self.logged.push(playtest::session::Code::RunStarted);
        }
        self.saved.runs.push(run);
        while self.saved.runs.len() > MAX_RUNS_KEPT {
            self.saved.runs.remove(0);
        }
        self.save();
        effect
    }

    /// Start `id` again from its own offer: after a failure, a refusal, or
    /// a stop. `runs` changes its size, as the full run after a try does.
    pub fn again(&mut self, id: &str, computer: Option<&str>, full: bool) -> Effect {
        let Some(run) = self.run(id).cloned() else {
            return Effect::None;
        };
        if run.running() {
            return Effect::None;
        }
        let (purpose, runs) = if full {
            (Purpose::Test, eval_ext::DEFAULT_RUNS)
        } else {
            (run.purpose.clone(), run.runs)
        };
        if !full {
            self.saved.runs.retain(|kept| kept.id != id);
        }
        self.start(
            &run.talk,
            run.turn,
            &run.offer,
            &run.tool,
            purpose,
            run.draft.clone(),
            computer,
            Some(runs),
        )
    }

    /// Open the system share sheet with `text`.
    pub fn share(&mut self, text: String) {
        self.share = Some(text);
    }

    /// A computer run's Coder task started, or why it didn't.
    pub fn on_computer(&mut self, run: &str, started: Result<(String, String, String), String>) {
        let Some(entry) = self.run_mut(run) else {
            return;
        };
        match started {
            Ok((host, label, task)) => {
                entry.place = Some(Place::Computer { host, label, task });
                entry.state = RunState::Queued;
            }
            Err(why) => {
                entry.state = RunState::Refused {
                    why,
                    connect: false,
                };
            }
        }
        self.save();
    }

    /// Stop a run, after the person confirmed.
    pub fn stop(&mut self, id: &str) -> Effect {
        let (hosted, runtime, world) = (self.hosted.clone(), self.runtime.clone(), self.world);
        let Some(run) = self.run_mut(id) else {
            return Effect::None;
        };
        if !run.running() {
            return Effect::None;
        }
        let effect = match &run.place {
            Some(Place::Hosted {
                event: Some(event), ..
            }) => {
                if let (Some(hosted), Some(runtime), Some(world)) = (hosted, runtime, world) {
                    runtime.spawn(hosted.stop(world, event.clone()));
                }
                Effect::None
            }
            Some(Place::Computer { host, task, .. }) => Effect::Command {
                host: host.clone(),
                task: task.clone(),
                text: "Stopped from this device.".into(),
                stop: true,
            },
            _ => Effect::None,
        };
        run.state = RunState::Stopped;
        self.lives.remove(id);
        self.save();
        effect
    }

    /// Publish a run's result, after `SCR-20`'s button.
    pub fn publish(&mut self, id: &str) -> Effect {
        let (hosted, runtime, world) = (self.hosted.clone(), self.runtime.clone(), self.world);
        let Some(run) = self.run_mut(id) else {
            return Effect::None;
        };
        if run.state != RunState::Done
            || matches!(
                run.publish,
                PublishState::Publishing | PublishState::Published { .. }
            )
        {
            return Effect::None;
        }
        match run.place.clone() {
            Some(Place::Hosted {
                request: Some(request),
                ..
            }) => {
                let Some(report) = run.outcome.as_ref().and_then(|o| o.report.clone()) else {
                    run.publish = PublishState::Failed {
                        why: "We couldn't find the result's report on this device.".into(),
                    };
                    self.save();
                    return Effect::None;
                };
                let (Some(hosted), Some(runtime), Some(world)) = (hosted, runtime, world) else {
                    run.publish = PublishState::Failed {
                        why: "We can't reach our test computers from this build.".into(),
                    };
                    self.save();
                    return Effect::None;
                };
                run.publish = PublishState::Publishing;
                self.logged.push(playtest::session::Code::Published);
                let live = self
                    .lives
                    .entry(id.to_owned())
                    .or_insert_with(|| Arc::new(Mutex::new(Live::default())))
                    .clone();
                crate::basic_coder::lock(&live).published = None;
                runtime.spawn(rung(hosted.publish(world, request, report, live)));
                self.save();
                Effect::None
            }
            Some(Place::Computer { host, task, .. }) => {
                run.publish = PublishState::Published { event: None };
                self.logged.push(playtest::session::Code::Published);
                let text = publish_prompt();
                self.save();
                Effect::Command {
                    host,
                    task,
                    text,
                    stop: false,
                }
            }
            _ => Effect::None,
        }
    }

    /// Mark a first-run step reached by the run's result.
    pub fn first_result(&self) -> Option<&Run> {
        self.saved
            .runs
            .iter()
            .rev()
            .find(|run| run.first && run.state == RunState::Done)
    }

    /// The level-up overlay, when the ledger shows a level this phone
    /// hasn't shown yet. The first reading only records the level.
    pub fn level_up(&mut self) {
        if !self.standing.read {
            return;
        }
        let level = self.standing.level;
        match self.saved.level_seen {
            None => {
                self.saved.level_seen = Some(level);
                self.save();
            }
            Some(seen) if level > seen => {
                self.saved.level_seen = Some(level);
                self.sheet = Some(Sheet::LevelUp { level });
                self.save();
            }
            Some(_) => {}
        }
    }

    /// New eval XP since the menu last showed it.
    pub fn new_credit(&self) -> Option<u64> {
        let seen = self.saved.eval_xp_seen.unwrap_or(0);
        (self.standing.read && self.standing.eval_xp > seen).then(|| self.standing.eval_xp - seen)
    }

    /// The menu showed the credit line and the person opened it.
    pub fn credit_seen(&mut self) {
        if self.standing.read {
            self.saved.eval_xp_seen = Some(self.standing.eval_xp);
            self.save();
        }
    }

    // ------------------------------------------------------------------
    // Taps.

    /// What a tap on the Gym button `id` does to the Gym's own state, and
    /// what it asks the chat around it to do. The phone's Coder tab and
    /// the desktop's chat both call this, so a button does the same thing
    /// on each; they differ only in how they carry out the [`Effect`].
    /// `None` when the last view didn't mint `id`.
    ///
    /// `draft_of` reads a conversation's draft ([`Gym::draft_of`] of its
    /// turns); `open` is the conversation on screen; `computer` names the
    /// ready computer a run may go to.
    pub fn tap(
        &mut self,
        id: &str,
        draft_of: impl FnOnce(&str) -> Option<Value>,
        open: Option<&str>,
        computer: Option<&str>,
    ) -> Option<Effect> {
        let action = self.actions.get(id).cloned()?;
        Some(match action {
            Action::Start {
                talk,
                turn,
                offer,
                tool,
                purpose,
            } => {
                let draft = draft_of(&talk);
                self.start(&talk, turn, &offer, &tool, purpose, draft, computer, None)
            }
            Action::Stop { run } => {
                self.sheet = Some(Sheet::Stop { run });
                Effect::None
            }
            Action::ConfirmStop { run } => {
                self.sheet = None;
                self.stop(&run)
            }
            Action::Retry { run } => self.again(&run, computer, false),
            Action::FullRun { run } => {
                self.sheet = None;
                self.again(&run, computer, true)
            }
            Action::Details { run } => {
                self.sheet = Some(Sheet::Result { run });
                Effect::None
            }
            Action::TestSet { source } => {
                self.sheet = Some(Sheet::TestSet(source));
                Effect::None
            }
            Action::Publish { run } => {
                self.sheet = Some(Sheet::Publish { run });
                Effect::None
            }
            Action::ConfirmPublish { run } => self.publish(&run),
            Action::CloseSheet | Action::Nice => {
                let closing = self.sheet.take();
                // FLOW-01: after the first result, Add to the Gym or Not
                // now ends the guided path at the menu.
                if matches!(closing, Some(Sheet::Publish { .. }))
                    && self.first_run() == FirstRun::Chat
                    && self.first_result().is_some()
                {
                    Effect::Menu
                } else {
                    Effect::None
                }
            }
            Action::LooksGood { talk } => {
                self.sheet = None;
                Effect::Say {
                    talk,
                    text: LOOKS_GOOD.into(),
                }
            }
            Action::ChangeIt { .. } => Effect::Compose {
                text: CHANGE.into(),
            },
            Action::Say { text, fresh } => {
                self.sheet = None;
                match (open, fresh) {
                    (Some(talk), false) => Effect::Say {
                        talk: talk.to_owned(),
                        text,
                    },
                    _ => Effect::Fresh { text },
                }
            }
            Action::OpenCoder { host, task } => Effect::OpenCoder { host, task },
            Action::ConnectComputer => Effect::ConnectComputer,
            Action::Share { text } => {
                self.share(text);
                Effect::None
            }
            Action::Chat => {
                self.sheet = None;
                self.credit_seen();
                Effect::OpenChat {
                    talk: self.waiting().map(|run| run.talk.clone()),
                }
            }
            Action::Profile => {
                self.sheet = Some(Sheet::Profile);
                Effect::None
            }
            Action::VerseGym => Effect::VerseGym,
            Action::ChooseCoder => {
                self.set_first_run(FirstRun::EndCard);
                Effect::None
            }
            Action::LetsGo => {
                self.set_first_run(FirstRun::Chat);
                Effect::Fresh {
                    text: crate::first_run::FIRST_MESSAGE.into(),
                }
            }
            Action::NotNow => {
                self.opt_out();
                Effect::None
            }
            Action::SkipFirstRun => {
                self.set_first_run(FirstRun::Done);
                Effect::Menu
            }
        })
    }

    /// Open the Gym sheet a router offer names (`open_screen` for
    /// `GymResult`, `GymPublish`, or `GymTestSet`): the latest result, or
    /// the open conversation's draft (`open`, whose draft is `draft`) else
    /// the latest result's test set. Whether an offer named it is the
    /// caller's check. `false` for any other screen, or with nothing to
    /// show.
    pub fn open_screen(&mut self, screen: Screen, open: Option<&str>, draft: bool) -> bool {
        if self.hidden {
            return false;
        }
        let latest = self.latest_result().map(|run| run.id.clone());
        let sheet = match screen {
            Screen::GymPublish => latest.map(|run| Sheet::Publish { run }),
            Screen::GymResult => latest.map(|run| Sheet::Result { run }),
            Screen::GymTestSet => match (draft, open) {
                (true, Some(talk)) => Some(Sheet::TestSet(TestSetSource::Draft {
                    talk: talk.to_owned(),
                })),
                _ => latest.map(|run| Sheet::TestSet(TestSetSource::Run { run })),
            },
            _ => None,
        };
        let opened = sheet.is_some();
        if opened {
            self.sheet = sheet;
        }
        opened
    }

    // ------------------------------------------------------------------
    // Cards in a chat.

    /// The cards `talk` shows under its newest reply, drawn now; each is a
    /// surface the tab places in the page, by ID.
    pub fn cards_for(&mut self, talk: &str, turns: &[Turn], here: &Here) -> Vec<String> {
        let mut ids = vec![];
        if self.hidden {
            return ids;
        }
        let Some((turn, meta)) = turns
            .iter()
            .enumerate()
            .rev()
            .find(|(_, turn)| turn.role == Role::Assistant)
            .and_then(|(at, turn)| turn.meta.clone().map(|meta| (at, meta)))
        else {
            // No reply yet: only a run's card.
            if let Some(id) = self.run_card(talk, here) {
                ids.push(id);
            }
            return ids;
        };
        let text = &turns[turn].text;
        let started_here = self.talk_run(talk).filter(|run| run.turn == turn).is_some();
        let starts: Vec<Value> = meta
            .offers
            .iter()
            .filter_map(|offer| match offer {
                Offer::StartEval { body } => Some(body.clone()),
                _ => None,
            })
            .collect();
        let step = (self.saved.first_run == FirstRun::Chat
            && self.saved.first_talk.as_deref() == Some(talk))
        .then(|| "STEP 2 OF 3".to_owned());
        let gate = GATE_LINES
            .iter()
            .any(|line| text.trim_end().ends_with(line));
        let draft = Self::draft_of(turns);
        let base = format!("t{}-{turn}", short(talk));
        let mut used_start = false;
        for (n, card) in meta.parsed_cards().into_iter().enumerate() {
            let id = format!("{base}-{n}");
            let view = match card {
                // A run started from this card replaces it.
                Card::Tool { .. } | Card::Check { .. } | Card::Draft(_) if started_here => continue,
                Card::Tool {
                    name,
                    summary,
                    latest,
                    ..
                } => {
                    let start = starts.first().map(|body| {
                        used_start = true;
                        (
                            Action::Start {
                                talk: talk.to_owned(),
                                turn,
                                offer: body.clone(),
                                tool: name.clone(),
                                purpose: Purpose::Test,
                            },
                            body,
                        )
                    });
                    let size = start.as_ref().and_then(|(_, body)| size_of(body));
                    let first = step.is_some() && start.is_none();
                    let mut view = ui::tool_card(
                        &mut self.actions,
                        &id,
                        &name,
                        &summary,
                        latest.as_ref(),
                        start.map(|(action, _)| action).zip(size.as_ref()),
                        self.hosted.is_some(),
                        step.clone(),
                    );
                    // The first run never ends at a card it can't start.
                    if first {
                        view.chips.push(self.actions.button(
                            format!("{id}.skip"),
                            "Skip for now",
                            None,
                            Action::SkipFirstRun,
                        ));
                    }
                    view
                }
                Card::Draft(parsed) => {
                    let start = starts.first().map(|body| {
                        used_start = true;
                        let size = size_of(body);
                        let purpose = if size.is_some_and(|s| s.runs <= 1) {
                            Purpose::Try
                        } else {
                            Purpose::Test
                        };
                        (
                            Action::Start {
                                talk: talk.to_owned(),
                                turn,
                                offer: body.clone(),
                                tool: parsed.tool.name.clone(),
                                purpose,
                            },
                            size,
                        )
                    });
                    let size = start.as_ref().and_then(|(_, size)| *size);
                    let mut view = ui::draft_card(
                        &mut self.actions,
                        &id,
                        talk,
                        &parsed,
                        gate,
                        start.map(|(action, _)| action).zip(size.as_ref()),
                    );
                    // At the try, Looks good also moves on.
                    if gate
                        && view
                            .primary
                            .as_ref()
                            .is_some_and(|p| p.label == "TRY IT ONCE")
                    {
                        view.secondary.insert(
                            0,
                            self.actions.button(
                                format!("{id}.good"),
                                "Looks good",
                                None,
                                Action::LooksGood {
                                    talk: talk.to_owned(),
                                },
                            ),
                        );
                    }
                    if here.busy {
                        view.primary = None;
                    }
                    view
                }
                Card::Check {
                    tool,
                    line,
                    confirms,
                    ..
                } => {
                    // The card names the result's signer, which for a
                    // hosted run is the runner, not the trainer who asked:
                    // the card says "a trainer" rather than a wrong name.
                    let trainer = "A trainer".to_owned();
                    let claim = Claim::of(&line.headline, line.verdict);
                    let mine = line.publication.pubkey == self.standing.public_hex;
                    // The referee pays a checker once per test set version:
                    // no XP to promise when the ledger already paid this
                    // trainer for checking this one (#9948).
                    let credited = starts
                        .first()
                        .and_then(suite_of)
                        .is_some_and(|suite| self.standing.checked_suite(&suite, None));
                    let start = starts.first().map(|body| {
                        used_start = true;
                        Action::Start {
                            talk: talk.to_owned(),
                            turn,
                            offer: body.clone(),
                            tool: ui::humane(&tool),
                            purpose: Purpose::Check {
                                publication: line.publication.to_value(),
                                trainer: trainer.clone(),
                                claim,
                            },
                        }
                    });
                    ui::check_card(
                        &mut self.actions,
                        &id,
                        &tool,
                        &trainer,
                        &claim,
                        confirms,
                        start,
                        self.standing.checker_xp,
                        mine,
                        credited,
                    )
                }
                Card::Result {
                    headline, verdict, ..
                } => ui::published_result_card(&id, &Claim::of(&headline, verdict), None),
                Card::News(items) => {
                    let offer = meta.offers.iter().find_map(|offer| match offer {
                        Offer::OpenScreen {
                            screen: Screen::VerseGym,
                        } => Some(("SEE THE BOARD".to_owned(), Action::VerseGym)),
                        _ => None,
                    });
                    ui::news_card(&mut self.actions, &id, &items, offer)
                }
                // The worker doesn't know the trainer: the phone draws
                // credit from its own ledger below.
                Card::Credit { .. } => continue,
                Card::Run {
                    completed, planned, ..
                } => run_card_plain(&id, completed, planned),
                // No capability for what was asked (#9960): the closest
                // admitted one, and the way to add one the worker named.
                Card::Capability { closest, add } => {
                    let gym = meta.offers.iter().find_map(|offer| match offer {
                        Offer::OpenScreen {
                            screen: Screen::VerseGym,
                        } => Some(Action::VerseGym),
                        _ => None,
                    });
                    ui::capability_card(&mut self.actions, &id, closest.as_ref(), add, gym)
                }
            };
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        }
        // A start offer no card carried: its own card.
        if !used_start
            && !started_here
            && let Some(body) = starts.first()
        {
            let id = format!("{base}-start");
            let size = size_of(body);
            let view = ui::tool_card(
                &mut self.actions,
                &id,
                "Coder's test",
                "Run this test set on Coder.",
                None,
                Some((
                    Action::Start {
                        talk: talk.to_owned(),
                        turn,
                        offer: body.clone(),
                        tool: "the plugin".into(),
                        purpose: Purpose::Test,
                    },
                    &size.unwrap_or(cj::Size {
                        cases: 0,
                        runs: 0,
                        arms: 0,
                    }),
                )),
                self.hosted.is_some(),
                step.clone(),
            );
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        }
        // Where the draft is, the interview's step reads it: when a draft
        // card isn't in this reply but the step is a gate, a card for it.
        if gate
            && !meta.cards.iter().any(|c| c["card"] == "draft")
            && let Some(draft) = draft.as_ref().and_then(|d| cj::parse_draft(d).ok())
        {
            let id = format!("{base}-draft");
            let view = ui::draft_card(&mut self.actions, &id, talk, &draft, true, None);
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        }
        // The person's own credit and result, which only the phone holds.
        if meta.route.as_deref() == Some("eval.credit") {
            let id = format!("{base}-credit");
            let view = self.credit_card(&id);
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        }
        let own_result = meta.offers.contains(&Offer::OpenScreen {
            screen: Screen::GymResult,
        });
        if let Some(id) = self.run_card(talk, here) {
            ids.push(id);
        } else if own_result && let Some(run) = self.latest_result().map(|run| run.id.clone()) {
            let id = format!("{base}-mine");
            let view = self.result_card(&id, &run);
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        } else if own_result {
            let id = format!("{base}-mine");
            let view = CardView {
                id: id.clone(),
                kind: "result",
                step: None,
                icon: None,
                title: "NO RESULT YET".into(),
                badge: None,
                compare: None,
                lines: vec![Line {
                    text: "You haven't run a test yet.".into(),
                    tone: Tone::Body,
                }],
                items: vec![],
                progress: vec![],
                primary: None,
                secondary: vec![],
                chips: vec![],
                source: None,
                busy: false,
            };
            self.shown(&id, view.kind);
            self.cards.insert(id.clone(), view);
            ids.push(id);
        }
        ids
    }

    /// The card of `talk`'s newest run: running (`CARD-03`), its result
    /// (`CARD-04`), or why it didn't run.
    fn run_card(&mut self, talk: &str, here: &Here) -> Option<String> {
        let run = self.talk_run(talk)?.id.clone();
        let id = format!("r{}", short(&run));
        let view = self.result_card_or_run(&id, &run, here);
        self.shown(&format!("{id}-{}", view.kind), view.kind);
        self.cards.insert(id.clone(), view);
        Some(id)
    }

    fn result_card_or_run(&mut self, id: &str, run: &str, here: &Here) -> CardView {
        let Some(entry) = self.run(run).cloned() else {
            return run_card_plain(id, 0, 0);
        };
        match &entry.state {
            RunState::Done => self.result_card(id, run),
            _ => self.progress_card(id, &entry, here),
        }
    }

    /// `CARD-03` Run card, and its failure states.
    fn progress_card(&mut self, id: &str, run: &Run, here: &Here) -> CardView {
        let tool = run.tool.to_uppercase();
        let title = match &run.purpose {
            Purpose::Check { .. } => format!("CHECKING {tool}"),
            Purpose::Try => format!("TRYING {tool}"),
            Purpose::Test => format!("TESTING {tool}"),
        };
        let step = run.first.then(|| "STEP 3 OF 3".to_owned());
        let mut lines = vec![];
        let mut progress = vec![];
        let mut primary = None;
        let mut secondary = vec![];
        let mut chips = vec![];
        let mut busy = false;
        match &run.state {
            RunState::Starting => {
                busy = true;
                lines.push(line("Starting…", Tone::Body));
            }
            RunState::Queued => {
                busy = true;
                lines.push(line("Waiting for a computer to start it.", Tone::Body));
            }
            RunState::Running { done, planned } => {
                busy = true;
                match (done, planned) {
                    (Some(done), Some(planned)) if *planned > 0 => {
                        // The runner counts runs across both sides, so one
                        // row of blocks, a block per test, shows how far
                        // it is; it never splits them by side.
                        let tests = run.cases.max(1);
                        progress.push(Progress {
                            label: "with and without the plugin".into(),
                            done: (*done).min(*planned) * tests / planned,
                            total: tests,
                        });
                        lines.push(line(&format!("{done} of {planned} runs done"), Tone::Quiet));
                    }
                    _ => lines.push(line("Working", Tone::Quiet)),
                }
            }
            RunState::Failed { why, ours } => {
                lines.push(line(why, Tone::Body));
                if *ours {
                    lines.push(line("Something went wrong on our side.", Tone::Quiet));
                }
                primary = Some(self.actions.button(
                    format!("{id}.retry"),
                    "TRY AGAIN",
                    None,
                    Action::Retry {
                        run: run.id.clone(),
                    },
                ));
            }
            RunState::Stopped => {
                lines.push(line("You stopped this test.", Tone::Body));
                primary = Some(self.actions.button(
                    format!("{id}.retry"),
                    "START IT AGAIN",
                    None,
                    Action::Retry {
                        run: run.id.clone(),
                    },
                ));
            }
            RunState::Refused { why, connect } => {
                lines.push(line(why, Tone::Body));
                if *connect {
                    chips.push(self.actions.button(
                        format!("{id}.connect"),
                        "Connect a computer",
                        Some("add"),
                        Action::ConnectComputer,
                    ));
                }
                primary = Some(self.actions.button(
                    format!("{id}.retry"),
                    "TRY AGAIN",
                    None,
                    Action::Retry {
                        run: run.id.clone(),
                    },
                ));
                if run.first {
                    chips.push(self.actions.button(
                        format!("{id}.skip"),
                        "Skip for now",
                        None,
                        Action::SkipFirstRun,
                    ));
                }
            }
            RunState::Done => {}
        }
        if run.running() {
            match &run.place {
                Some(Place::Computer {
                    label, host, task, ..
                }) => {
                    lines.push(line(
                        &format!("Coder is running the tests on {label}. You can leave; we'll post the result here."),
                        Tone::Quiet,
                    ));
                    secondary.push(self.actions.button(
                        format!("{id}.open"),
                        &format!("Open Coder on {label}"),
                        Some("computer"),
                        Action::OpenCoder {
                            host: host.clone(),
                            task: task.clone(),
                        },
                    ));
                }
                _ => lines.push(line(
                    "You can leave. We'll post the result here and on the menu.",
                    Tone::Quiet,
                )),
            }
            if !matches!(run.state, RunState::Starting) {
                secondary.push(self.actions.button(
                    format!("{id}.stop"),
                    "Stop",
                    Some("stop"),
                    Action::Stop {
                        run: run.id.clone(),
                    },
                ));
            }
        }
        if here.busy {
            primary = None;
        }
        CardView {
            id: id.to_owned(),
            kind: "run",
            step,
            icon: Some(ui::tool_glyph(&run.tool)),
            title,
            badge: None,
            compare: None,
            lines,
            items: vec![],
            progress,
            primary,
            secondary,
            chips,
            source: None,
            busy,
        }
    }

    /// `CARD-04` Result card for a run this phone started.
    pub fn result_card(&mut self, id: &str, run: &str) -> CardView {
        let Some(run) = self.run(run).cloned() else {
            return run_card_plain(id, 0, 0);
        };
        let mut lines = vec![];
        let mut secondary = vec![];
        let mut chips = vec![];
        let (title, compare) = match &run.outcome {
            Some(outcome) => (
                ui::verdict_headline(outcome.claim.verdict, run.check(), run.pilot()),
                Some(ui::compare(&outcome.claim, &run.tool)),
            ),
            None => (
                match &run.place {
                    Some(Place::Computer { label, .. }) => {
                        format!("DONE ON {}", label.to_uppercase())
                    }
                    _ => "DONE".into(),
                },
                None,
            ),
        };
        if run.outcome.is_none() {
            lines.push(line(
                "Coder ran the tests on your computer. Open its chat to see how they went.",
                Tone::Body,
            ));
            if let Some(Place::Computer { host, task, label }) = &run.place {
                secondary.push(self.actions.button(
                    format!("{id}.open"),
                    &format!("Open Coder on {label}"),
                    Some("computer"),
                    Action::OpenCoder {
                        host: host.clone(),
                        task: task.clone(),
                    },
                ));
            }
        }
        let primary = match (&run.publish, run.pilot()) {
            (_, true) => Some(self.actions.button(
                format!("{id}.full"),
                "RUN THE FULL TEST SET",
                None,
                Action::FullRun {
                    run: run.id.clone(),
                },
            )),
            (PublishState::Published { .. }, false) => {
                lines.push(line("✓ Added to the Gym", Tone::Strong));
                None
            }
            (PublishState::Publishing, false) => {
                lines.push(line("Adding it to the Gym…", Tone::Quiet));
                None
            }
            (PublishState::None | PublishState::Failed { .. }, false) => {
                if let PublishState::Failed { why } = &run.publish {
                    lines.push(line(
                        &format!("We couldn't add your result: {why} It's saved on this device."),
                        Tone::Quiet,
                    ));
                }
                Some(self.actions.button(
                    format!("{id}.add"),
                    "ADD TO THE GYM",
                    None,
                    Action::Publish {
                        run: run.id.clone(),
                    },
                ))
            }
        };
        // Time and cost changes, as the runner stated them.
        if let Some(outcome) = &run.outcome {
            for note in &outcome.notes {
                lines.push(line(note, Tone::Quiet));
            }
        }
        if !run.pilot() {
            lines.push(line(&self.xp_line(&run), Tone::Quiet));
        }
        if run.outcome.is_some() {
            secondary.push(self.actions.button(
                format!("{id}.details"),
                "See details",
                Some("info"),
                Action::Details {
                    run: run.id.clone(),
                },
            ));
        }
        secondary.push(self.actions.button(
            format!("{id}.tests"),
            "See the tests",
            Some("list"),
            Action::TestSet {
                source: TestSetSource::Run {
                    run: run.id.clone(),
                },
            },
        ));
        if run.pilot() {
            chips.push(self.actions.button(
                format!("{id}.ask"),
                "What should we fix?",
                Some("ask"),
                Action::Say {
                    text: "How did the try go? What should we fix?".into(),
                    fresh: false,
                },
            ));
        }
        CardView {
            id: id.to_owned(),
            kind: "result",
            step: None,
            icon: None,
            title,
            badge: None,
            compare,
            lines,
            items: vec![],
            progress: vec![],
            primary,
            secondary,
            chips,
            source: run
                .outcome
                .as_ref()
                .map(|_| "From the report of your run.".to_owned()),
            busy: false,
        }
    }

    /// Whether this run is a check of a test set whose checker award the
    /// trainer already holds, from another check.
    fn check_credited(&self, run: &Run) -> bool {
        if run.check().is_none() {
            return false;
        }
        let own = match &run.publish {
            PublishState::Published { event } => event.as_deref(),
            _ => None,
        };
        suite_of(&run.offer).is_some_and(|suite| self.standing.checked_suite(&suite, own))
    }

    /// When XP comes for a result, in one line, with a number only when a
    /// trusted quest record states it.
    fn xp_line(&self, run: &Run) -> String {
        if self.check_credited(run) {
            return CHECKED_ALREADY.into();
        }
        match (
            &run.purpose,
            self.standing.checker_xp,
            self.standing.evaluator_xp,
        ) {
            (Purpose::Check { .. }, Some(xp), _) => {
                format!("+{xp} XP once our referee signs your check, whichever way it went.")
            }
            (Purpose::Check { .. }, None, _) => {
                "You earn XP once our referee signs your check, whichever way it went.".into()
            }
            (_, _, Some(xp)) => format!("+{xp} XP when another trainer checks it."),
            (_, _, None) => "You earn XP when another trainer checks it.".into(),
        }
    }

    /// `CARD-07` Credit card, from the phone's XP ledger.
    pub fn credit_card(&mut self, id: &str) -> CardView {
        let standing = self.standing.clone();
        let mut items = made_items(&standing);
        let mut lines = vec![];
        let chips = vec![];
        let primary = if !standing.read {
            lines.push(line("Reading your XP…", Tone::Quiet));
            None
        } else if items.is_empty() {
            lines.push(line(
                "Nothing yet. When another trainer checks a result you added, you earn XP here.",
                Tone::Body,
            ));
            None
        } else {
            Some(self.actions.button(
                format!("{id}.share"),
                "SHARE WHAT YOU MADE",
                Some("share"),
                Action::Share {
                    text: share_text(&standing),
                },
            ))
        };
        if standing.read {
            items.push(Item {
                marks: vec![],
                text: level_line(&standing),
                detail: None,
                trailing: None,
            });
        }
        lines.push(line(
            "XP can't be spent. It shows what you did, with your name on it.",
            Tone::Quiet,
        ));
        CardView {
            id: id.to_owned(),
            kind: "credit",
            step: None,
            icon: Some("credit"),
            title: "YOUR CREDIT".into(),
            badge: None,
            compare: None,
            lines,
            items,
            progress: vec![],
            primary,
            secondary: vec![],
            chips,
            source: standing
                .read
                .then(|| "From your XP, recomputed on this device.".to_owned()),
            busy: false,
        }
    }

    /// The cards drawn this pass.
    pub fn cards(&self) -> &BTreeMap<String, CardView> {
        &self.cards
    }

    /// Start a new pass: forget the last pass's cards and buttons.
    pub fn begin(&mut self) {
        self.cards.clear();
        self.actions.clear();
    }

    // ------------------------------------------------------------------
    // Sheets.

    /// The conversation whose draft the sheet on screen lists, if any: the
    /// tab reads its turns for [`Gym::sheet_view`].
    pub fn sheet_talk(&self) -> Option<&str> {
        match &self.sheet {
            Some(Sheet::TestSet(TestSetSource::Draft { talk })) => Some(talk),
            _ => None,
        }
    }

    /// The sheet on screen, drawn. `draft` is the draft of
    /// [`Gym::sheet_talk`]'s conversation.
    pub fn sheet_view(&mut self, draft: Option<Value>) -> Option<SheetView> {
        let sheet = self.sheet.clone()?;
        let close = Some(
            self.actions
                .button("sheet.close", "Close", None, Action::CloseSheet),
        );
        let view = match sheet {
            Sheet::Result { run } => self.result_sheet(&run, close),
            Sheet::Publish { run } => self.publish_sheet(&run, close),
            Sheet::TestSet(source) => self.test_set_sheet(&source, draft, close),
            Sheet::LevelUp { level } => self.level_sheet(level),
            Sheet::Profile => self.profile_sheet(close),
            Sheet::Stop { run } => self.stop_sheet(&run),
        };
        if view.is_none() {
            self.sheet = None;
        }
        view
    }

    /// `SCR-05` Result (detail).
    fn result_sheet(&mut self, run: &str, close: Option<Button>) -> Option<SheetView> {
        let run = self.run(run)?.clone();
        let outcome = run.outcome.clone()?;
        let mut sections = vec![];
        let items = outcome
            .cases
            .iter()
            .map(|case| Item {
                marks: vec![mark(case.without), mark(case.with)],
                text: ui::humane(&case.id),
                detail: (case.kind == CaseKind::ShouldNotFire.word())
                    .then(|| "The plugin should stay out of the way.".to_owned()),
                trailing: None,
            })
            .collect();
        sections.push(Section {
            heading: Some("TESTS".into()),
            lines: vec![line("Without the plugin, then with it.", Tone::Quiet)],
            items,
        });
        let why = match (run.pilot(), outcome.claim.verdict, run.check()) {
            (true, _, _) => {
                "One run is a first look, not a result. Run the full test set to add it to the Gym."
            }
            (false, _, Some(_)) => {
                "A check shows whether a result holds up when someone else runs the same tests."
            }
            (false, Verdict3::Pass, None) => {
                "When other trainers confirm it and it holds up on a test set someone else wrote, Coder can use this plugin for everyone."
            }
            (false, Verdict3::Inconclusive, None) => {
                "That's useful too. Now everyone knows this plugin doesn't help on these tests."
            }
            (false, Verdict3::Fail, None) => "That's useful too. We won't give Coder this plugin.",
        };
        sections.push(Section {
            heading: None,
            lines: vec![
                line(&self.xp_line(&run), Tone::Body),
                line(why, Tone::Quiet),
            ],
            items: vec![],
        });
        let (next, primary) = match (&run.publish, run.pilot()) {
            (_, true) => (
                "Next: run the full test set.",
                Some(self.actions.button(
                    "sheet.full",
                    "RUN THE FULL TEST SET",
                    None,
                    Action::FullRun {
                        run: run.id.clone(),
                    },
                )),
            ),
            (PublishState::Published { .. }, false) => (
                "Next: check someone else's result for more XP.",
                Some(
                    self.actions
                        .button("sheet.back", "BACK TO CHAT", None, Action::CloseSheet),
                ),
            ),
            _ => (
                "Next: add your result to the Gym.",
                Some(self.actions.button(
                    "sheet.add",
                    "ADD TO THE GYM",
                    None,
                    Action::Publish {
                        run: run.id.clone(),
                    },
                )),
            ),
        };
        let mut secondary =
            vec![
            self.actions.button(
                "sheet.tests",
                "See the whole test set",
                Some("list"),
                Action::TestSet {
                    source: TestSetSource::Run { run: run.id.clone() },
                },
            ),
            self.actions.button(
                "sheet.share",
                "Share outside the app",
                Some("share"),
                Action::Share {
                    text: format!(
                        "Coder passed {} tests with {} on OpenAgents ({}). https://openagents.com",
                        outcome.claim.arrow(),
                        run.tool,
                        outcome.claim.verdict.plain()
                    ),
                },
            ),
        ];
        secondary.push(self.actions.button(
            "sheet.ask",
            "Ask about this result",
            Some("ask"),
            Action::Say {
                text: format!(
                    "Why did Coder do this with {}? It passed {} tests.",
                    run.tool,
                    outcome.claim.arrow()
                ),
                fresh: false,
            },
        ));
        Some(SheetView {
            id: format!("result-{}", short(&run.id)),
            kind: "result",
            title: "YOUR RESULT".into(),
            headline: Some(ui::verdict_headline(
                outcome.claim.verdict,
                run.check(),
                run.pilot(),
            )),
            big: None,
            compare: Some(ui::compare(&outcome.claim, &run.tool)),
            sections,
            bar: self.bar(),
            next: Some(next.into()),
            primary,
            secondary,
            close,
            busy: false,
        })
    }

    /// `SCR-20` Add to the Gym: exactly what becomes public.
    fn publish_sheet(&mut self, run: &str, close: Option<Button>) -> Option<SheetView> {
        let run = self.run(run)?.clone();
        let credited = self.check_credited(&run);
        let mut public = vec![];
        let tests = if run.cases == 1 {
            "the test you ran, and how it's checked".to_owned()
        } else {
            format!("the {} tests you ran, and how they're checked", run.cases)
        };
        match &run.purpose {
            Purpose::Check { trainer, .. } => {
                // "A trainer" starts a sentence elsewhere; here it's inside one.
                let trainer = match trainer.strip_prefix("A trainer") {
                    Some(rest) => format!("a trainer{rest}"),
                    None => trainer.clone(),
                };
                public.push(format!("your check of {trainer}'s result"));
            }
            _ => public.push(tests),
        }
        if let Some(outcome) = &run.outcome {
            public.push(format!("the result: {}", outcome.claim.arrow()));
        } else {
            public.push("the result Coder got on your computer".into());
        }
        let signer = match &run.place {
            Some(Place::Computer { label, .. }) => format!("{label}'s trainer name"),
            _ => match self.standing.name.as_str() {
                "" => "your trainer name".to_owned(),
                name => format!("your trainer name, {name}"),
            },
        };
        public.push(signer);
        let items = public
            .into_iter()
            .map(|text| Item {
                marks: vec!["dot"],
                text,
                detail: None,
                trailing: None,
            })
            .collect();
        let mut sections = vec![
            Section {
                heading: Some("Everyone will see:".into()),
                lines: vec![],
                items,
            },
            Section {
                heading: None,
                lines: vec![
                    line("Coder's full work on each test stays private.", Tone::Quiet),
                    line(
                        if credited {
                            "You already earned XP for checking this test set, so this check earns no more. The trainer who added the result earns XP whether you confirm it or not."
                        } else if run.check().is_some() {
                            "You and the trainer who added the result earn XP whether your check confirms it or not."
                        } else {
                            "Other trainers can run these tests to check the result. You earn XP when they do, whether they confirm it or not."
                        },
                        Tone::Quiet,
                    ),
                ],
                items: vec![],
            },
        ];
        let busy = run.publish == PublishState::Publishing;
        let primary = match &run.publish {
            PublishState::Published { .. } => {
                sections.push(Section {
                    heading: None,
                    lines: vec![line(
                        if credited {
                            "Added to the Gym. You already earned XP for checking this test set."
                        } else if run.check().is_some() {
                            "Added to the Gym. XP comes once our referee signs your check, whichever way it went."
                        } else {
                            "Added to the Gym. You'll earn XP when another trainer checks it."
                        },
                        Tone::Strong,
                    )],
                    items: vec![],
                });
                // FLOW-01: closing this sheet after the first result ends
                // the guided path at the menu, so the button says so.
                let label = if self.first_run() == FirstRun::Chat && self.first_result().is_some() {
                    "TO THE MENU"
                } else {
                    "BACK TO CHAT"
                };
                Some(
                    self.actions
                        .button("sheet.done", label, None, Action::CloseSheet),
                )
            }
            PublishState::Failed { why } => {
                sections.push(Section {
                    heading: None,
                    lines: vec![line(
                        &format!("We couldn't add your result: {why} It's saved on this device."),
                        Tone::Body,
                    )],
                    items: vec![],
                });
                Some(self.actions.button(
                    "sheet.publish",
                    "TRY AGAIN",
                    None,
                    Action::ConfirmPublish {
                        run: run.id.clone(),
                    },
                ))
            }
            PublishState::Publishing => Some(Actions::inert("sheet.publishing", "ADDING…")),
            PublishState::None => Some(self.actions.button(
                "sheet.publish",
                "ADD TO THE GYM",
                None,
                Action::ConfirmPublish {
                    run: run.id.clone(),
                },
            )),
        };
        let secondary = match &run.publish {
            PublishState::Published { .. } | PublishState::Publishing => vec![],
            _ => vec![
                self.actions
                    .button("sheet.later", "Not now", None, Action::CloseSheet),
            ],
        };
        Some(SheetView {
            id: format!("publish-{}", short(&run.id)),
            kind: "publish",
            title: "ADD TO THE GYM".into(),
            headline: None,
            big: None,
            compare: None,
            sections,
            bar: None,
            next: None,
            primary,
            secondary,
            close,
            busy,
        })
    }

    /// `SCR-21` Test set: a draft's tests, or the tests a run ran.
    fn test_set_sheet(
        &mut self,
        source: &TestSetSource,
        draft: Option<Value>,
        close: Option<Button>,
    ) -> Option<SheetView> {
        let (title, items, author, draft_talk) = match source {
            TestSetSource::Draft { talk } => {
                let draft = draft.and_then(|d| cj::parse_draft(&d).ok())?;
                (
                    format!(
                        "{} · {} TESTS",
                        draft.tool.name.to_uppercase(),
                        draft.cases.len()
                    ),
                    ui::draft_items(&draft, true),
                    "Your draft. Only on this device until you add it to the Gym.".to_owned(),
                    Some(talk.clone()),
                )
            }
            TestSetSource::Run { run } => {
                let run = self.run(run)?.clone();
                if let Some(draft) = run.draft.as_ref().and_then(|d| cj::parse_draft(d).ok()) {
                    (
                        format!("{} · {} TESTS", run.tool.to_uppercase(), draft.cases.len()),
                        ui::draft_items(&draft, true),
                        "Made by you in chat.".to_owned(),
                        None,
                    )
                } else if let Some(outcome) = &run.outcome {
                    (
                        format!(
                            "{} · {} TESTS",
                            run.tool.to_uppercase(),
                            outcome.cases.len()
                        ),
                        outcome
                            .cases
                            .iter()
                            .enumerate()
                            .map(|(n, case)| Item {
                                marks: vec![],
                                text: format!("{} {}", n + 1, ui::humane(&case.id)),
                                detail: (case.kind == CaseKind::ShouldNotFire.word())
                                    .then(|| "The plugin should stay out of the way.".to_owned()),
                                trailing: None,
                            })
                            .collect(),
                        "A published test set in the Gym.".to_owned(),
                        None,
                    )
                } else {
                    (
                        format!("{} · {} TESTS", run.tool.to_uppercase(), run.cases),
                        vec![],
                        "A published test set in the Gym. Coder's chat on your computer lists each test.".to_owned(),
                        None,
                    )
                }
            }
        };
        let primary = match &draft_talk {
            Some(talk) => self.actions.button(
                "sheet.good",
                "LOOKS GOOD",
                None,
                Action::LooksGood { talk: talk.clone() },
            ),
            None => self
                .actions
                .button("sheet.done", "DONE", None, Action::CloseSheet),
        };
        Some(SheetView {
            id: "test-set".into(),
            kind: "test_set",
            title,
            headline: None,
            big: None,
            compare: None,
            sections: vec![
                Section {
                    heading: None,
                    lines: vec![],
                    items,
                },
                Section {
                    heading: None,
                    lines: vec![line(&author, Tone::Quiet)],
                    items: vec![],
                },
            ],
            bar: None,
            next: None,
            primary: Some(primary),
            secondary: vec![],
            close,
            busy: false,
        })
    }

    /// `SCR-06` Level up.
    fn level_sheet(&mut self, level: u32) -> Option<SheetView> {
        let nice = self
            .actions
            .button("sheet.nice", "NICE", None, Action::Nice);
        let title = self.standing.titles.first().cloned();
        Some(SheetView {
            id: format!("level-{level}"),
            kind: "level_up",
            title: "LEVEL UP".into(),
            headline: None,
            big: Some(level.to_string()),
            compare: None,
            sections: title
                .map(|title| Section {
                    heading: None,
                    lines: vec![line(
                        &format!("Your title: {}", title.to_uppercase()),
                        Tone::Strong,
                    )],
                    items: vec![],
                })
                .into_iter()
                .collect(),
            bar: self.bar(),
            next: Some("Next: check someone's result for more XP.".into()),
            primary: Some(nice),
            secondary: vec![],
            close: None,
            busy: false,
        })
    }

    /// `SCR-11` Profile, minimal: name, level, XP, titles, your results, and
    /// what you made.
    fn profile_sheet(&mut self, close: Option<Button>) -> Option<SheetView> {
        let standing = self.standing.clone();
        let mut sections = vec![];
        if !standing.titles.is_empty() {
            sections.push(Section {
                heading: None,
                lines: vec![line(
                    &format!("Titles: {}", standing.titles.join(", ").to_uppercase()),
                    Tone::Body,
                )],
                items: vec![],
            });
        }
        let results: Vec<Item> = self
            .saved
            .runs
            .iter()
            .rev()
            // Full runs only: a try can't be added to the Gym, and a check
            // shows under What you made as "Your check" (#9949).
            .filter(|run| !run.pilot() && run.check().is_none())
            .filter_map(|run| {
                let outcome = run.outcome.as_ref()?;
                Some(Item {
                    marks: vec![],
                    text: run.tool.clone(),
                    detail: Some(format!(
                        "{} tests · {}",
                        outcome.claim.arrow(),
                        outcome.claim.verdict.plain()
                    )),
                    trailing: None,
                })
            })
            .take(6)
            .collect();
        // Runs this phone keeps; after a reinstall only the ledger has them,
        // and "No results yet" over a result it shows would contradict it.
        if !(results.is_empty() && standing.results.iter().any(|r| !r.check)) {
            sections.push(Section {
                heading: Some("YOUR RESULTS".into()),
                lines: if results.is_empty() {
                    vec![line(
                        "No results yet. Your first test takes a few minutes.",
                        Tone::Quiet,
                    )]
                } else {
                    vec![]
                },
                items: results,
            });
        }
        let made = made_items(&standing);
        sections.push(Section {
            heading: Some("WHAT YOU MADE".into()),
            lines: if !standing.read {
                vec![line("Reading your XP…", Tone::Quiet)]
            } else if made.is_empty() {
                vec![line(
                    "Nothing yet. When another trainer checks a result you added, you earn XP here.",
                    Tone::Quiet,
                )]
            } else {
                vec![]
            },
            items: made,
        });
        sections.push(Section {
            heading: None,
            lines: vec![line(
                "XP can't be spent. It shows what you did, with your name on it.",
                Tone::Quiet,
            )],
            items: vec![],
        });
        let chat = self
            .actions
            .button("sheet.chat", "CHAT WITH OPENAGENTS", None, Action::Chat);
        Some(SheetView {
            id: "profile".into(),
            kind: "profile",
            title: if standing.name.is_empty() {
                "PROFILE".into()
            } else {
                standing.name.to_uppercase()
            },
            headline: standing.read.then(|| level_line(&standing)),
            big: None,
            compare: None,
            sections,
            bar: self.bar(),
            next: Some(next_step(self).to_owned()),
            primary: Some(chat),
            secondary: vec![],
            close,
            busy: false,
        })
    }

    /// Confirm before stopping a run.
    fn stop_sheet(&mut self, run: &str) -> Option<SheetView> {
        let run = self.run(run)?.clone();
        if !run.running() {
            return None;
        }
        let stop = self.actions.button(
            "sheet.stop",
            "STOP THE TEST",
            None,
            Action::ConfirmStop {
                run: run.id.clone(),
            },
        );
        let keep = self
            .actions
            .button("sheet.keep", "Keep going", None, Action::CloseSheet);
        Some(SheetView {
            id: format!("stop-{}", short(&run.id)),
            kind: "stop",
            title: "STOP THE TEST?".into(),
            headline: None,
            big: None,
            compare: None,
            sections: vec![Section {
                heading: None,
                lines: vec![line(
                    "Nothing is added to the Gym. You can start it again from the card.",
                    Tone::Body,
                )],
                items: vec![],
            }],
            bar: None,
            next: None,
            primary: Some(stop),
            secondary: vec![keep],
            close: None,
            busy: false,
        })
    }

    /// The level bar, once the ledger is read.
    pub fn bar(&self) -> Option<Bar> {
        let s = &self.standing;
        s.read.then(|| Bar {
            label: format!("Level {} · {}/{} XP", s.level, s.xp, s.next_at),
            value: s.xp.saturating_sub(s.level_at),
            max: s.next_at.saturating_sub(s.level_at).max(1),
        })
    }
}

/// The next step on the menu and the profile, from real state
/// (`SCR-01.E11`).
pub fn next_step(gym: &Gym) -> &'static str {
    if let Some(run) = gym.waiting() {
        return if run.running() {
            "Next: your test is running. We'll post the result in chat."
        } else {
            "Next: add your result to the Gym."
        };
    }
    if gym.standing.pending > 0 {
        return "Next: someone checked your result. XP is on its way.";
    }
    if gym.latest_result().is_some() {
        return "Next: check someone else's result for more XP.";
    }
    "Next: ask what's new in the Gym."
}

fn line(text: &str, tone: Tone) -> Line {
    Line {
        text: text.to_owned(),
        tone,
    }
}

fn mark(passed: Option<bool>) -> &'static str {
    match passed {
        Some(true) => "check",
        Some(false) => "cross",
        None => "none",
    }
}

/// The size an offer body names.
/// What a check whose checker award the trainer already holds says
/// instead of an XP promise.
const CHECKED_ALREADY: &str =
    "You already earned XP for checking this test set. This check earns no more.";

/// The published test set a `start_eval` offer runs, as its release ID.
fn suite_of(body: &Value) -> Option<String> {
    match (Offer::StartEval { body: body.clone() })
        .start_eval()?
        .suite
    {
        SuiteSource::Published(suite) => Some(suite.id),
        SuiteSource::Draft => None,
    }
}

fn size_of(body: &Value) -> Option<cj::Size> {
    Offer::StartEval { body: body.clone() }
        .start_eval()
        .map(|start| start.size)
}

/// A worker's `run` card, which the phone draws plainly.
fn run_card_plain(id: &str, completed: u64, planned: u64) -> CardView {
    CardView {
        id: id.to_owned(),
        kind: "run",
        step: None,
        icon: None,
        title: "TESTING".into(),
        badge: None,
        compare: None,
        lines: vec![line(
            &if planned > 0 {
                format!("{completed} of {planned} runs done")
            } else {
                "Working".into()
            },
            Tone::Quiet,
        )],
        items: vec![],
        progress: vec![],
        primary: None,
        secondary: vec![],
        chips: vec![],
        source: None,
        busy: planned == 0 || completed < planned,
    }
}

/// "Level 3 · 40 XP to level 4".
pub fn level_line(s: &Standing) -> String {
    format!(
        "Level {} · {} XP to level {}",
        s.level,
        s.next_at.saturating_sub(s.xp),
        s.level + 1
    )
}

/// What the person made and the XP it earned, as rows.
fn made_items(s: &Standing) -> Vec<Item> {
    let mut items = vec![];
    for (_, results, xp) in &s.suites {
        items.push(Item {
            marks: vec![],
            text: "A test set you made".into(),
            detail: Some(if *results == 1 {
                "1 result on it".into()
            } else {
                format!("{results} results on it")
            }),
            trailing: (*xp > 0).then(|| format!("+{xp} XP")),
        });
    }
    for row in &s.results {
        let (mark, text) = match (row.check, row.standing.as_str()) {
            (true, "awarded") => ("check", "Your check earned XP".to_owned()),
            (true, "pending") => (
                "wait",
                "Your check followed the rules. XP is on its way".to_owned(),
            ),
            (true, "disputed") => ("cross", "Your check disagreed with the result".to_owned()),
            (true, _) => ("dot", "A check you added".to_owned()),
            (false, "awarded" | "pending") => (
                if row.standing == "awarded" {
                    "check"
                } else {
                    "wait"
                },
                if row.confirmed_by == 1 {
                    "Your result: checked by 1 trainer".to_owned()
                } else {
                    format!("Your result: checked by {} trainers", row.confirmed_by)
                },
            ),
            (false, "waiting") => ("wait", "Your result: waiting for a check".to_owned()),
            (false, "disputed") => ("cross", "Your result: a check disagreed".to_owned()),
            (false, _) => ("dot", "A result you added".to_owned()),
        };
        items.push(Item {
            marks: vec![mark],
            text,
            detail: Some(
                Verdict3::from(
                    nostr::eval_ext::Verdict::parse(&row.verdict)
                        .unwrap_or(nostr::eval_ext::Verdict::Inconclusive),
                )
                .plain()
                .to_owned(),
            ),
            trailing: (row.xp > 0).then(|| format!("+{} XP", row.xp)),
        });
    }
    for (title, xp) in &s.adoptions {
        items.push(Item {
            marks: vec!["check"],
            text: format!("Coder uses it now: {title}"),
            detail: None,
            trailing: Some(format!("+{xp} XP")),
        });
    }
    items
}

/// What **Share what you made** shares: the trainer's level and XP from
/// the ledger.
fn share_text(s: &Standing) -> String {
    format!(
        "I'm {} on OpenAgents: level {}, {} XP from testing plugins for Coder. https://openagents.com",
        s.name, s.level, s.xp
    )
}

/// What a computer run asks Coder to do: run the plugin's test set with
/// `openagents ext eval run`, and publish nothing. It names `ext eval`,
/// the older name for `plugin test`, so a computer with an older
/// `openagents` runs it too.
fn computer_prompt(run: &str, tool: &str, subject: &SubjectSource, runs: u64) -> String {
    let path = format!(".openagents/phone-tests/{run}");
    let id = match subject {
        SubjectSource::Definition(definition) => definition.id.clone(),
        SubjectSource::Draft => String::new(),
    };
    // Each catalog plugin and its directory, from the hosted runner's
    // catalog (#10090).
    let where_they_are = crate::eval_cards::CATALOG
        .iter()
        .zip(crate::eval_cards::catalog_dirs())
        .map(|(name, dir)| format!("{name} is {dir}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Run the test set for the plugin {tool} ({id}) with the OpenAgents command line, and tell \
         us how it went. The person started this test from the OpenAgents app on their phone.\n\n\
         Find the plugin's directory ({where_they_are} in the OpenAgents repository), then \
         run:\n\n\
         openagents ext eval run DIR --trust --grant write --runs {runs} --output-dir {path} \
         --json {path}/result.json\n\n\
         Don't publish anything: the person adds the result to the Gym from their phone. When it \
         finishes, say how many tests passed with and without the plugin, and the verdict, from the \
         report it wrote."
    )
}

/// What a computer run asks Coder to do when the person confirmed Add to
/// the Gym.
fn publish_prompt() -> String {
    "The person confirmed Add to the Gym on their phone. Publish the report.json the test run \
     you just did wrote, with `openagents ext eval publish REPORT`."
        .into()
}

/// A future that rings the app once it ends, so the change shows at once.
async fn rung(future: Pin<Box<dyn Future<Output = ()> + Send>>) {
    future.await;
    crate::wake::ring();
}

/// The view packet's Gym section.
#[derive(Serialize)]
pub struct View {
    /// `first_run`, `menu`, or `chat`.
    pub screen: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_run: Option<crate::first_run::FirstRunView>,
    pub menu: crate::first_run::MenuView,
    pub cards: BTreeMap<String, CardView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<SheetView>,
    /// Text for the system share sheet, once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share: Option<String>,
    /// A run is going: ask for packets every few seconds.
    pub live: bool,
}

#[cfg(test)]
mod tests;
