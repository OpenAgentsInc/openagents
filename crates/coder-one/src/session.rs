//! Session control: what a host may do to a running executor session, and
//! the host loop that does it.
//!
//! An executor adapter demonstrates a **capability matrix**: start,
//! observe incrementally, stop with a cleanup acknowledgement, resume, and
//! steer. A policy may use only what its adapter has shown. The scripted
//! executor in [`crate::scripted`] demonstrates all five, which lets the
//! host's handling of each be tested without a model. The Claude Code
//! adapter in [`crate::adapter`] demonstrates all five too; the Codex
//! adapter demonstrates every capability but steering.
//!
//! [`drive`] is that handling. It starts a session, observes its
//! normalized events as they arrive, steers or stops it when a rule fires,
//! and resumes a stopped session when told to. Each control action is an
//! `exec.control` invocation in the record, and an action the adapter
//! hasn't demonstrated is recorded as refused rather than attempted. A
//! fresh session with a new brief is a different thing from a resume, and
//! the record keeps them apart: a resume names the session it continues.
//!
//! One [`Controller`] owns the session's state. Each normalized event
//! becomes an [`Observation`] stamped with a [`Version`]: the session ID,
//! the process generation, the controller's sequence number, and the
//! workspace revision. A rule or a monitor that wants to steer, stop, or
//! accept the workspace submits a [`Proposal`] carrying the version it
//! saw, and the controller refuses one that no longer describes the
//! session.

use std::future::Future;

use atif::document::{Source, Step};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::delegate::{Agent, Briefing, Report};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::stream::{self, Event, Kind};

/// The schema of a capability matrix record.
pub const MATRIX_SCHEMA: &str = "openagents.coder-one.capability-matrix.v1";

/// One tick of the host clock's pace: a wait, or nothing.
pub type Tick = std::pin::Pin<Box<dyn Future<Output = ()>>>;

/// The step extension that holds one normalized executor event.
pub const EVENT_KEY: &str = "executor_event";

/// One thing a host may do to a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Start,
    Observe,
    Stop,
    Resume,
    Steer,
}

impl Capability {
    /// Every capability, in the matrix's order.
    pub const ALL: [Capability; 5] = [
        Capability::Start,
        Capability::Observe,
        Capability::Stop,
        Capability::Resume,
        Capability::Steer,
    ];

    /// The capability's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Capability::Start => "start",
            Capability::Observe => "observe",
            Capability::Stop => "stop",
            Capability::Resume => "resume",
            Capability::Steer => "steer",
        }
    }
}

/// What an adapter has demonstrated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub start: bool,
    /// Normalized events arrive while the session runs.
    pub observe: bool,
    /// The host can end the session and gets a cleanup acknowledgement.
    pub stop: bool,
    /// A stopped or ended session can continue with a new message.
    pub resume: bool,
    /// A running session accepts a message.
    pub steer: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::all()
    }
}

impl Capabilities {
    /// Every capability.
    #[must_use]
    pub fn all() -> Self {
        Capabilities {
            start: true,
            observe: true,
            stop: true,
            resume: true,
            steer: true,
        }
    }

    /// Start only: the session runs to its end and reports once.
    #[must_use]
    pub fn start_only() -> Self {
        Capabilities {
            start: true,
            observe: false,
            stop: false,
            resume: false,
            steer: false,
        }
    }

    /// Whether `capability` is demonstrated.
    #[must_use]
    pub fn has(&self, capability: Capability) -> bool {
        match capability {
            Capability::Start => self.start,
            Capability::Observe => self.observe,
            Capability::Stop => self.stop,
            Capability::Resume => self.resume,
            Capability::Steer => self.steer,
        }
    }

    /// The matrix as the record holds it, with a note on where it comes
    /// from.
    #[must_use]
    pub fn record(&self, adapter: &str, note: &str) -> Value {
        json!({
            "schema": MATRIX_SCHEMA,
            "adapter": adapter,
            "capabilities": Capability::ALL
                .iter()
                .map(|c| (c.word().to_string(), json!(self.has(*c))))
                .collect::<serde_json::Map<String, Value>>(),
            "note": note,
        })
    }
}

/// The matrix each CLI adapter has demonstrated, with where it was shown.
#[must_use]
pub fn cli_capabilities(agent: Agent) -> (Capabilities, &'static str) {
    crate::adapter::capabilities(agent)
}

/// The schema of a versioned observation in the record.
pub const OBSERVATION_SCHEMA: &str = "openagents.coder-one.executor-event.v1";

/// Where an observation, or a proposal made from one, stands: which
/// session, which process of it, which event, and which workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub session_id: Option<String>,
    /// Which process of the session: 1 for the start, one more for each
    /// resume.
    pub generation: u32,
    /// The controller's own sequence over every observation, across
    /// generations. An event's `seq` counts within one native stream.
    pub seq: u64,
    /// The workspace revision: how many artifact changes the controller
    /// had observed. An observation that changes an artifact carries the
    /// revision it made.
    pub revision: u64,
}

/// One normalized event, as the controller accepted it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    #[serde(flatten)]
    pub version: Version,
    pub at_ms: u64,
    pub event: Event,
}

impl Observation {
    /// The observation as a step extension.
    #[must_use]
    pub fn record(&self, adapter: &str) -> Value {
        json!({
            "schema": OBSERVATION_SCHEMA,
            "adapter": adapter,
            "session_id": self.version.session_id,
            "generation": self.version.generation,
            "seq": self.version.seq,
            "revision": self.version.revision,
            "at_ms": self.at_ms,
            "event": self.event,
        })
    }
}

/// Where a session is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Running,
    /// The host stopped it.
    Stopped,
    /// It ended on its own, or failed to start.
    Ended,
}

/// What an observer asks the controller to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Steer,
    Stop,
    /// Accept the workspace as the candidate the observation saw.
    Certify,
}

/// A proposal and the observation it was made from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub intent: Intent,
    pub basis: Version,
    /// Who proposed it, such as `steer rule` or a monitor's name.
    pub by: String,
}

/// One state change the controller made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub at_ms: u64,
    pub from: Phase,
    pub to: Phase,
    pub generation: u32,
    pub why: String,
}

/// A proposal the controller refused, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub proposal: Proposal,
    pub why: String,
}

/// The one owner of a session's state. Observers, such as rules and
/// monitors, read [`Observation`]s and submit [`Proposal`]s; only the
/// controller changes the phase, and it refuses a proposal made from an
/// observation that no longer describes the session. A monitor's late
/// answer about the first process can't steer the resumed one, and a
/// check that saw revision 3 can't certify revision 4.
#[derive(Clone, Debug)]
pub struct Controller {
    phase: Phase,
    generation: u32,
    seq: u64,
    revision: u64,
    session_id: Option<String>,
    pub transitions: Vec<Transition>,
    pub refusals: Vec<Refusal>,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            phase: Phase::Idle,
            generation: 0,
            seq: 0,
            revision: 0,
            session_id: None,
            transitions: Vec::new(),
            refusals: Vec::new(),
        }
    }
}

impl Controller {
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The version a proposal made now would carry.
    #[must_use]
    pub fn version(&self) -> Version {
        Version {
            session_id: self.session_id.clone(),
            generation: self.generation,
            seq: self.seq,
            revision: self.revision,
        }
    }

    fn move_to(&mut self, at_ms: u64, to: Phase, why: &str) {
        self.transitions.push(Transition {
            at_ms,
            from: self.phase,
            to,
            generation: self.generation,
            why: why.to_string(),
        });
        self.phase = to;
    }

    /// A process of the session started: the first, or a resume.
    pub fn begin(&mut self, at_ms: u64, why: &str) {
        self.generation += 1;
        self.move_to(at_ms, Phase::Running, why);
    }

    /// The session ended on its own.
    pub fn ended(&mut self, at_ms: u64, why: &str) {
        if self.phase == Phase::Running {
            self.move_to(at_ms, Phase::Ended, why);
        }
    }

    /// The host stopped the session.
    pub fn stopped(&mut self, at_ms: u64, why: &str) {
        if self.phase == Phase::Running {
            self.move_to(at_ms, Phase::Stopped, why);
        }
    }

    /// Names the session once the adapter or its stream reports an ID.
    pub fn identify(&mut self, session_id: Option<String>) {
        if session_id.is_some() {
            self.session_id = session_id;
        }
    }

    /// Stamps one event with the version it arrived at.
    pub fn observe(&mut self, at_ms: u64, event: Event) -> Observation {
        match &event.kind {
            Kind::SessionStarted { session_id } => self.identify(session_id.clone()),
            Kind::ArtifactChanged { .. } => self.revision += 1,
            _ => {}
        }
        self.seq += 1;
        Observation {
            version: self.version(),
            at_ms,
            event,
        }
    }

    /// Admits a proposal that still describes the session, or refuses it
    /// and records why.
    ///
    /// # Errors
    ///
    /// Returns why the proposal is stale or out of place.
    pub fn admit(&mut self, proposal: &Proposal) -> Result<(), String> {
        let basis = &proposal.basis;
        let why = if basis.generation != self.generation {
            Some(format!(
                "it was made from generation {} and the session is at generation {}",
                basis.generation, self.generation
            ))
        } else if basis.session_id.is_some()
            && self.session_id.is_some()
            && basis.session_id != self.session_id
        {
            Some("it was made from another session".to_string())
        } else if matches!(proposal.intent, Intent::Steer | Intent::Stop)
            && self.phase != Phase::Running
        {
            Some(format!("the session is {:?}, not running", self.phase).to_lowercase())
        } else if proposal.intent == Intent::Certify && basis.revision != self.revision {
            Some(format!(
                "it saw workspace revision {} and the workspace is at revision {}",
                basis.revision, self.revision
            ))
        } else {
            None
        };
        match why {
            None => Ok(()),
            Some(why) => {
                self.refusals.push(Refusal {
                    proposal: proposal.clone(),
                    why: why.clone(),
                });
                Err(why)
            }
        }
    }
}

/// What a stop reports: when, why, and what was still pending.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StopAck {
    pub at_ms: u64,
    pub reason: String,
    /// Scripted events or processes the stop cancelled.
    pub pending: usize,
    /// What cleanup the adapter did before acknowledging.
    pub cleanup: String,
}

/// A session the host can control. Times are milliseconds since start.
pub trait Session {
    /// The adapter's name in the record, such as `scripted`.
    fn adapter(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    /// The native session ID, once the session has reported one.
    fn session_id(&self) -> Option<String>;
    /// Starts the session on `briefing`.
    fn start(&mut self, briefing: &Briefing) -> impl Future<Output = Result<(), String>>;
    /// Lets the session run to `now_ms`. Returns whether it is still
    /// running.
    fn advance(&mut self, now_ms: u64) -> impl Future<Output = bool>;
    /// The normalized events since the last call.
    fn observe(&mut self) -> Vec<Event>;
    /// Ends the session and waits for its cleanup.
    fn stop(&mut self, now_ms: u64, reason: &str) -> impl Future<Output = Result<StopAck, String>>;
    /// Continues a stopped or ended session with `message`.
    fn resume(&mut self, now_ms: u64, message: &str) -> impl Future<Output = Result<(), String>>;
    /// Sends `message` into the running session.
    fn steer(&mut self, now_ms: u64, message: &str) -> impl Future<Output = Result<(), String>>;
    /// The report once the session is over.
    fn report(&mut self) -> Report;
}

/// When a control rule fires.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum Trigger {
    /// An assistant claim containing this text.
    Claim { contains: String },
    /// A command that completed with a non-zero exit code.
    CommandFailed,
    /// A change to a path ending with this text.
    Artifact { path: String },
    /// This many milliseconds after start.
    After { ms: u64 },
}

impl Trigger {
    fn fires(&self, event: Option<&Event>, now_ms: u64) -> bool {
        match (self, event.map(|e| &e.kind)) {
            (Trigger::Claim { contains }, Some(Kind::AssistantClaim { text })) => {
                text.contains(contains.as_str())
            }
            (Trigger::CommandFailed, Some(Kind::CommandCompleted { exit_code, .. })) => {
                exit_code.is_some_and(|code| code != 0)
            }
            (Trigger::Artifact { path }, Some(Kind::ArtifactChanged { path: changed, .. })) => {
                changed.ends_with(path.as_str())
            }
            (Trigger::After { ms }, None) => now_ms >= *ms,
            _ => false,
        }
    }
}

/// A steering rule: when `when` fires, send `message`, once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Steer {
    pub when: Trigger,
    pub message: String,
}

/// What the host does to a session besides starting it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    /// The host's deadline; the session is stopped when it passes.
    pub deadline_ms: u64,
    /// How far the host's clock moves between looks.
    pub tick_ms: u64,
    #[serde(default)]
    pub steer: Option<Steer>,
    /// Stop the session when this fires, before the deadline.
    #[serde(default)]
    pub stop_when: Option<Trigger>,
    /// After a stop, resume with this message, once.
    #[serde(default)]
    pub resume: Option<String>,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            deadline_ms: 600_000,
            tick_ms: 10,
            steer: None,
            stop_when: None,
            resume: None,
        }
    }
}

/// One control action the host took or was refused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub at_ms: u64,
    pub capability: Capability,
    /// `done`, `refused`, or `failed`.
    pub outcome: String,
    pub detail: String,
}

/// What the host loop saw and did.
#[derive(Clone, Debug)]
pub struct Driven {
    pub session_id: Option<String>,
    pub events: Vec<Event>,
    pub actions: Vec<Action>,
    pub stops: Vec<StopAck>,
    /// Every phase change the controller made.
    pub transitions: Vec<Transition>,
    /// Proposals the controller refused as stale.
    pub refusals: Vec<Refusal>,
    pub report: Report,
    /// The host clock when the loop ended.
    pub elapsed_ms: u64,
}

impl Driven {
    /// The loop's record: events by kind and every control action.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "session_id": self.session_id,
            "events": stream::tally(&self.events),
            "actions": self.actions,
            "stops": self.stops,
            "transitions": self.transitions,
            "refusals": self.refusals,
            "status": self.report.status.word(),
            "elapsed_ms": self.elapsed_ms,
        })
    }
}

fn control_implementation() -> Implementation {
    Implementation::new(
        "exec.control",
        "host session control",
        &json!({ "version": 1, "capabilities": Capability::ALL.iter().map(|c| c.word()).collect::<Vec<_>>() }),
    )
}

/// Records one control action as an `exec.control` invocation.
fn act(
    recorder: &Recorder,
    actions: &mut Vec<Action>,
    at_ms: u64,
    capability: Capability,
    result: Result<String, (String, String)>,
) {
    let id = recorder.begin(
        Start::new("exec.control", control_implementation())
            .named(capability.word())
            .with_effects(),
    );
    let (outcome, detail, record_outcome) = match result {
        Ok(detail) => ("done".to_string(), detail, Outcome::Completed),
        Err((outcome, detail)) => {
            let record = if outcome == "refused" {
                Outcome::Skipped
            } else {
                Outcome::Failed
            };
            (outcome, detail, record)
        }
    };
    recorder.end(
        &id,
        Finish::new(record_outcome)
            .output(json!({ "capability": capability.word(), "outcome": outcome, "detail": detail, "at_ms": at_ms }))
            .cost(crate::record::Cost::none()),
    );
    actions.push(Action {
        at_ms,
        capability,
        outcome,
        detail,
    });
}

fn refusal(capability: Capability, adapter: &str) -> (String, String) {
    (
        "refused".to_string(),
        format!(
            "the {adapter} adapter has not demonstrated {}",
            capability.word()
        ),
    )
}

/// The most host-loop iterations, so a script that never ends and an
/// adapter that can't stop still finish.
const MAX_TICKS: u64 = 1_000_000;

/// Runs a session under `controls`, recording each normalized event and
/// each control action. `pace` is called with the host clock after every
/// tick; the scripted executor uses it to sleep in real time, and a test
/// passes a no-op to run on virtual time.
///
/// A [`Controller`] owns the session's state. Every event is stamped with
/// the version it arrived at, and a rule's steer or stop is a proposal the
/// controller admits only while it still describes the session.
pub async fn drive<S: Session>(
    session: &mut S,
    briefing: &Briefing,
    controls: &Controls,
    recorder: &Recorder,
    pace: &mut dyn FnMut(u64) -> Tick,
) -> Driven {
    let capabilities = session.capabilities();
    let adapter = session.adapter().to_string();
    let mut controller = Controller::default();
    let mut actions = Vec::new();
    let mut events = Vec::new();
    let mut stops = Vec::new();
    let mut now = 0u64;
    let started = if capabilities.start {
        session.start(briefing).await
    } else {
        Err(refusal(Capability::Start, &adapter).1)
    };
    if let Err(error) = started {
        act(
            recorder,
            &mut actions,
            0,
            Capability::Start,
            Err(("failed".to_string(), error)),
        );
        controller.ended(0, "the session did not start");
        return Driven {
            session_id: None,
            events,
            actions,
            stops,
            transitions: controller.transitions,
            refusals: controller.refusals,
            report: session.report(),
            elapsed_ms: 0,
        };
    }
    controller.begin(0, "started");
    controller.identify(session.session_id());
    act(
        recorder,
        &mut actions,
        0,
        Capability::Start,
        Ok(format!(
            "started on a {}-character briefing",
            briefing.chars()
        )),
    );
    let mut steered = false;
    // A stop rule acts once: a resumed session isn't stopped again by the
    // rule that stopped it.
    let mut stop_ruled = false;
    let mut resumed = false;
    let mut observe_refused = false;
    let mut stop_refused = false;
    let tick = controls.tick_ms.max(1);
    for _ in 0..MAX_TICKS {
        now += tick;
        let running = session.advance(now).await;
        pace(now).await;
        controller.identify(session.session_id());
        let fresh = if capabilities.observe {
            session.observe()
        } else {
            if !observe_refused {
                observe_refused = true;
                act(
                    recorder,
                    &mut actions,
                    now,
                    Capability::Observe,
                    Err(refusal(Capability::Observe, &adapter)),
                );
            }
            Vec::new()
        };
        let observed: Vec<Observation> = fresh
            .into_iter()
            .map(|event| controller.observe(now, event))
            .collect();
        for observation in &observed {
            recorder.push(observation_step(&adapter, observation));
        }
        // A rule looks at each new observation, and at the clock. What it
        // proposes carries the version it saw.
        let mut fire_steer = None;
        let mut fire_stop = None;
        let looks: Vec<(Option<&Event>, Version)> = observed
            .iter()
            .map(|o| (Some(&o.event), o.version.clone()))
            .chain([(None, controller.version())])
            .collect();
        for (event, version) in looks {
            if let Some(rule) = &controls.steer
                && !steered
                && fire_steer.is_none()
                && rule.when.fires(event, now)
            {
                fire_steer = Some(version.clone());
            }
            if let Some(rule) = &controls.stop_when
                && !stop_ruled
                && fire_stop.is_none()
                && rule.fires(event, now)
            {
                fire_stop = Some(("a stop rule fired".to_string(), version));
            }
        }
        events.extend(observed.into_iter().map(|o| o.event));
        if !running {
            controller.ended(now, "the session ended");
        }
        if running && now >= controls.deadline_ms {
            fire_stop = Some((
                format!("the host deadline of {} ms passed", controls.deadline_ms),
                controller.version(),
            ));
        }
        if let Some(basis) = fire_steer.filter(|_| running) {
            steered = true;
            let message = controls
                .steer
                .as_ref()
                .map(|s| s.message.clone())
                .unwrap_or_default();
            let proposal = Proposal {
                intent: Intent::Steer,
                basis,
                by: "steer rule".to_string(),
            };
            let result = if !capabilities.steer {
                Err(refusal(Capability::Steer, &adapter))
            } else if let Err(why) = controller.admit(&proposal) {
                Err(("refused".to_string(), format!("stale: {why}")))
            } else {
                session
                    .steer(now, &message)
                    .await
                    .map(|()| format!("sent a {}-character message", message.chars().count()))
                    .map_err(|error| ("failed".to_string(), error))
            };
            act(recorder, &mut actions, now, Capability::Steer, result);
        }
        let mut running = running;
        if let Some((reason, basis)) = fire_stop.filter(|_| running) {
            let proposal = Proposal {
                intent: Intent::Stop,
                basis,
                by: if reason.contains("deadline") {
                    "host deadline"
                } else {
                    "stop rule"
                }
                .to_string(),
            };
            if !capabilities.stop {
                if !stop_refused {
                    stop_refused = true;
                    act(
                        recorder,
                        &mut actions,
                        now,
                        Capability::Stop,
                        Err(refusal(Capability::Stop, &adapter)),
                    );
                }
            } else if let Err(why) = controller.admit(&proposal) {
                act(
                    recorder,
                    &mut actions,
                    now,
                    Capability::Stop,
                    Err(("refused".to_string(), format!("stale: {why}"))),
                );
            } else {
                match session.stop(now, &reason).await {
                    Ok(ack) => {
                        act(
                            recorder,
                            &mut actions,
                            now,
                            Capability::Stop,
                            Ok(format!(
                                "{reason}; {} pending cancelled; {}",
                                ack.pending, ack.cleanup
                            )),
                        );
                        controller.stopped(now, &reason);
                        stop_ruled |= proposal.by == "stop rule";
                        stops.push(ack);
                        running = false;
                    }
                    Err(error) => act(
                        recorder,
                        &mut actions,
                        now,
                        Capability::Stop,
                        Err(("failed".to_string(), error)),
                    ),
                }
            }
            if !running && let Some(message) = controls.resume.as_ref().filter(|_| !resumed) {
                resumed = true;
                let continues = session.session_id();
                let result = if capabilities.resume {
                    session
                        .resume(now, message)
                        .await
                        .map(|()| {
                            format!(
                                "resumed session {}",
                                continues.clone().unwrap_or_else(|| "unknown".to_string())
                            )
                        })
                        .map_err(|error| ("failed".to_string(), error))
                } else {
                    Err(refusal(Capability::Resume, &adapter))
                };
                running = result.is_ok();
                if running {
                    controller.begin(now, "resumed");
                    controller.identify(session.session_id());
                }
                act(recorder, &mut actions, now, Capability::Resume, result);
            }
        }
        if !running {
            break;
        }
    }
    // Events the last tick produced after a stop still belong to the record.
    if capabilities.observe {
        for event in session.observe() {
            let observation = controller.observe(now, event);
            recorder.push(observation_step(&adapter, &observation));
            events.push(observation.event);
        }
    }
    controller.ended(now, "the host loop ended");
    Driven {
        session_id: session.session_id(),
        events,
        actions,
        stops,
        transitions: controller.transitions,
        refusals: controller.refusals,
        report: session.report(),
        elapsed_ms: now,
    }
}

/// The step that records one observation.
#[must_use]
pub fn observation_step(adapter: &str, observation: &Observation) -> Step {
    Step::said(
        Source::System,
        &format!(
            "executor event {}: {}",
            observation.version.seq,
            observation.event.kind.word()
        ),
    )
    .noting(EVENT_KEY, observation.record(adapter))
}

/// A pace that doesn't wait: the host clock is virtual.
pub fn virtual_time() -> impl FnMut(u64) -> Tick {
    |_| Box::pin(async {})
}

/// A pace that sleeps `scale` real milliseconds per host millisecond.
pub fn real_time(scale: f64) -> impl FnMut(u64) -> Tick {
    let mut last = 0u64;
    move |now| {
        let step = now.saturating_sub(last);
        last = now;
        let wait = std::time::Duration::from_secs_f64((step as f64 * scale / 1000.0).max(0.0));
        Box::pin(async move {
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cli_adapters_demonstrate_their_own_matrices() {
        let (claude, note) = cli_capabilities(Agent::ClaudeCode);
        assert_eq!(claude, Capabilities::all());
        assert!(!note.is_empty());
        let (codex, _) = cli_capabilities(Agent::Codex);
        assert!(codex.resume && !codex.steer);
        let record = Capabilities::all().record("scripted", "every capability");
        assert_eq!(record["capabilities"]["steer"], json!(true));
    }

    #[test]
    fn a_trigger_fires_on_its_own_event_kind() {
        let claim = Event {
            seq: 1,
            line: 1,
            offset: None,
            kind: Kind::AssistantClaim {
                text: "All tests pass".to_string(),
            },
        };
        assert!(
            Trigger::Claim {
                contains: "tests pass".to_string()
            }
            .fires(Some(&claim), 0)
        );
        assert!(!Trigger::CommandFailed.fires(Some(&claim), 0));
        assert!(Trigger::After { ms: 5 }.fires(None, 5));
        assert!(!Trigger::After { ms: 5 }.fires(Some(&claim), 5));
    }

    fn event(seq: u64, kind: Kind) -> Event {
        Event {
            seq,
            line: seq as usize,
            offset: None,
            kind,
        }
    }

    #[test]
    fn a_late_answer_about_the_first_process_cannot_steer_the_resumed_one() {
        let mut controller = Controller::default();
        controller.begin(0, "started");
        let seen = controller.observe(
            10,
            event(
                1,
                Kind::SessionStarted {
                    session_id: Some("s-1".to_string()),
                },
            ),
        );
        assert_eq!(seen.version.generation, 1);
        assert_eq!(seen.version.session_id.as_deref(), Some("s-1"));
        // A monitor is still thinking about `seen` when the host stops and
        // resumes the session.
        controller.stopped(20, "a stop rule fired");
        controller.begin(30, "resumed");
        let late = Proposal {
            intent: Intent::Steer,
            basis: seen.version.clone(),
            by: "monitor".to_string(),
        };
        let why = controller.admit(&late).unwrap_err();
        assert!(why.contains("generation 1"), "{why}");
        assert_eq!(controller.refusals.len(), 1);
        // The same proposal made from the resumed process is admitted.
        let fresh = Proposal {
            basis: controller.version(),
            ..late
        };
        assert!(controller.admit(&fresh).is_ok());
    }

    #[test]
    fn a_check_that_saw_an_older_revision_cannot_certify_a_changed_workspace() {
        let mut controller = Controller::default();
        controller.begin(0, "started");
        let written = controller.observe(
            5,
            event(
                1,
                Kind::ArtifactChanged {
                    path: "run.py".to_string(),
                    change: "write".to_string(),
                },
            ),
        );
        assert_eq!(written.version.revision, 1);
        let certify = Proposal {
            intent: Intent::Certify,
            basis: written.version.clone(),
            by: "check".to_string(),
        };
        assert!(controller.admit(&certify).is_ok());
        let _ = controller.observe(
            9,
            event(
                2,
                Kind::ArtifactChanged {
                    path: "run.py".to_string(),
                    change: "update".to_string(),
                },
            ),
        );
        let why = controller.admit(&certify).unwrap_err();
        assert!(why.contains("revision 1"), "{why}");
        // Stopping an ended session is refused as out of place, not stale.
        controller.ended(12, "the session ended");
        let stop = Proposal {
            intent: Intent::Stop,
            basis: controller.version(),
            by: "stop rule".to_string(),
        };
        assert!(controller.admit(&stop).unwrap_err().contains("not running"));
        assert_eq!(
            controller
                .transitions
                .iter()
                .map(|t| t.to)
                .collect::<Vec<_>>(),
            vec![Phase::Running, Phase::Ended]
        );
    }
}
