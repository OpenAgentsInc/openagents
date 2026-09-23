//! Session control: what a host may do to a running executor session, and
//! the host loop that does it.
//!
//! An executor adapter demonstrates a **capability matrix**: start,
//! observe incrementally, stop with a cleanup acknowledgement, resume, and
//! steer. A policy may use only what its adapter has shown. The CLI
//! adapters in [`crate::delegate`] start a session and report once it ends,
//! so their matrix is start only; the scripted executor in
//! [`crate::scripted`] demonstrates all five, which is what lets the host's
//! handling of each be tested without a model.
//!
//! [`drive`] is that handling. It starts a session, observes its
//! normalized events as they arrive, steers or stops it when a rule fires,
//! and resumes a stopped session when told to. Each control action is an
//! `exec.control` invocation in the record, and an action the adapter
//! hasn't demonstrated is recorded as refused rather than attempted. A
//! fresh session with a new brief is a different thing from a resume, and
//! the record keeps them apart: a resume names the session it continues.

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

/// The matrix the CLI adapters have demonstrated, with why.
#[must_use]
pub fn cli_capabilities(agent: Agent) -> (Capabilities, &'static str) {
    (
        Capabilities::start_only(),
        match agent {
            Agent::ClaudeCode => {
                "Claude Code's CLI offers --resume and stream-json input, but the host redirects the briefing from a file and parses the stream after the process ends; the supervisor's deadline ends the process group without a cleanup acknowledgement."
            }
            Agent::Codex => {
                "Codex offers `codex exec resume`, but the host parses the stream after the process ends; the supervisor's deadline ends the process group without a cleanup acknowledgement."
            }
        },
    )
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
    fn resume(&mut self, now_ms: u64, message: &str) -> Result<(), String>;
    /// Sends `message` into the running session.
    fn steer(&mut self, now_ms: u64, message: &str) -> Result<(), String>;
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
pub async fn drive<S: Session>(
    session: &mut S,
    briefing: &Briefing,
    controls: &Controls,
    recorder: &Recorder,
    pace: &mut dyn FnMut(u64) -> Tick,
) -> Driven {
    let capabilities = session.capabilities();
    let adapter = session.adapter().to_string();
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
        return Driven {
            session_id: None,
            events,
            actions,
            stops,
            report: session.report(),
            elapsed_ms: 0,
        };
    }
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
    let mut resumed = false;
    let mut observe_refused = false;
    let mut stop_refused = false;
    let tick = controls.tick_ms.max(1);
    for _ in 0..MAX_TICKS {
        now += tick;
        let running = session.advance(now).await;
        pace(now).await;
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
        for event in &fresh {
            let mut step = Step::said(
                Source::System,
                &format!("executor event {}: {}", event.seq, event.kind.word()),
            );
            step = step.noting(
                EVENT_KEY,
                json!({ "session_id": session.session_id(), "at_ms": now, "event": event }),
            );
            recorder.push(step);
        }
        // A rule looks at each new event, and at the clock.
        let mut fire_steer = false;
        let mut fire_stop = None;
        let looks: Vec<Option<&Event>> = fresh.iter().map(Some).chain([None]).collect();
        for event in looks {
            if let Some(rule) = &controls.steer
                && !steered
                && rule.when.fires(event, now)
            {
                fire_steer = true;
            }
            if let Some(rule) = &controls.stop_when
                && rule.fires(event, now)
            {
                fire_stop = Some("a stop rule fired".to_string());
            }
        }
        events.extend(fresh);
        if running && now >= controls.deadline_ms {
            fire_stop = Some(format!(
                "the host deadline of {} ms passed",
                controls.deadline_ms
            ));
        }
        if fire_steer && running {
            steered = true;
            let message = controls
                .steer
                .as_ref()
                .map(|s| s.message.clone())
                .unwrap_or_default();
            let result = if capabilities.steer {
                session
                    .steer(now, &message)
                    .map(|()| format!("sent a {}-character message", message.chars().count()))
                    .map_err(|error| ("failed".to_string(), error))
            } else {
                Err(refusal(Capability::Steer, &adapter))
            };
            act(recorder, &mut actions, now, Capability::Steer, result);
        }
        let mut running = running;
        if let Some(reason) = fire_stop.filter(|_| running) {
            if capabilities.stop {
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
            } else if !stop_refused {
                stop_refused = true;
                act(
                    recorder,
                    &mut actions,
                    now,
                    Capability::Stop,
                    Err(refusal(Capability::Stop, &adapter)),
                );
            }
            if !running && let Some(message) = controls.resume.as_ref().filter(|_| !resumed) {
                resumed = true;
                let result = if capabilities.resume {
                    session
                        .resume(now, message)
                        .map(|()| {
                            format!(
                                "resumed session {}",
                                session
                                    .session_id()
                                    .unwrap_or_else(|| "unknown".to_string())
                            )
                        })
                        .map_err(|error| ("failed".to_string(), error))
                } else {
                    Err(refusal(Capability::Resume, &adapter))
                };
                running = result.is_ok();
                act(recorder, &mut actions, now, Capability::Resume, result);
            }
        }
        if !running {
            break;
        }
    }
    // Events the last tick produced after a stop still belong to the record.
    if capabilities.observe {
        events.extend(session.observe());
    }
    Driven {
        session_id: session.session_id(),
        events,
        actions,
        stops,
        report: session.report(),
        elapsed_ms: now,
    }
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
    fn the_cli_adapters_demonstrate_start_only() {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let (capabilities, note) = cli_capabilities(agent);
            assert!(capabilities.start);
            assert!(!capabilities.observe && !capabilities.stop);
            assert!(!capabilities.resume && !capabilities.steer);
            assert!(!note.is_empty());
        }
        let record = Capabilities::all().record("scripted", "every capability");
        assert_eq!(record["capabilities"]["steer"], json!(true));
    }

    #[test]
    fn a_trigger_fires_on_its_own_event_kind() {
        let claim = Event {
            seq: 1,
            line: 1,
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
}
