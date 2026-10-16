//! Engine adapters. An adapter states its exact feature matrix and steering
//! facts; the host never upgrades an emulated or unsupported claim.
use crate::Result;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// How the adapter supports one NIP-SESS feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Native,
    Emulated,
    Unsupported,
}

impl Support {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Emulated => "emulated",
            Self::Unsupported => "unsupported",
        }
    }
}

/// The steer feature's three facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steering {
    /// `mid_turn`, `turn_boundary`, or `unsupported`.
    pub native: &'static str,
    /// `cancel_and_continue` or none.
    pub emulation: Option<&'static str>,
    /// Evidence that the engine consumed a steer.
    pub acknowledgment: &'static str,
}

/// An adapter's static description.
#[derive(Clone, Debug)]
pub struct Profile {
    /// Slug used in the adapter's DefinitionRef path.
    pub name: &'static str,
    /// Exact engine build identity bytes.
    pub engine: Vec<u8>,
    /// Native protocol schema bytes.
    pub protocol: Vec<u8>,
    /// One support level per NIP-SESS feature.
    pub features: BTreeMap<&'static str, Support>,
    pub steering: Steering,
    /// The closed settings schema this adapter accepts.
    pub settings_schema: Value,
}

/// One captured native protocol message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub direction: &'static str,
    pub method: String,
    pub native_id: Option<String>,
    pub payload: Option<Vec<u8>>,
}

/// What stepping the active turn observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    Running,
    /// The engine asks a question and waits.
    Question {
        native_id: String,
        prompt: Vec<u8>,
    },
    /// Authoritative terminal engine evidence.
    Finished {
        outcome: &'static str,
        cause: &'static str,
        output: Vec<u8>,
    },
    /// The engine cannot report this turn (for example after a restart).
    Lost,
}

pub trait Engine {
    fn profile(&self) -> Profile;
    /// Open or resume a native session. Resume state is opaque bytes.
    fn open(&mut self, resume: Option<&[u8]>) -> Result<String>;
    fn start(&mut self, native: &str, turn: &str, input: &[u8]) -> Result<()>;
    fn step(&mut self, native: &str) -> Observation;
    /// Deliver a mid-turn steer. Returns whether the engine replayed it.
    fn steer(&mut self, native: &str, turn: &str, correction: &[u8]) -> Result<bool>;
    fn interrupt(&mut self, native: &str, turn: &str) -> Result<()>;
    fn answer(&mut self, native: &str, native_id: &str, response: &[u8]) -> Result<()>;
    fn resume_state(&mut self, native: &str) -> Option<Vec<u8>>;
    fn captures(&mut self) -> Vec<Capture>;
    /// How many turns this engine instance actually started.
    fn started(&self) -> usize;
}

#[derive(Default)]
struct Run {
    turn: String,
    prompt: String,
    steps: u64,
    ask: Option<String>,
    asked: Option<String>,
    answer: Option<Value>,
    corrections: Vec<Value>,
    interrupted: bool,
}

#[derive(Default)]
struct Native {
    turns: u64,
    run: Option<Run>,
}

/// A deterministic in-process engine. Its input is JSON
/// `{prompt, steps, ask?}`; each step advances one unit of work.
pub struct LoopEngine {
    profile: Profile,
    sessions: BTreeMap<String, Native>,
    log: Vec<Capture>,
    counter: u64,
    started: usize,
    /// Fault: lose the answer delivery after it is sent.
    pub fail_answer: bool,
    /// Fault: lose the start acknowledgment after the turn begins.
    pub fail_start: bool,
}

fn features(pairs: &[(&'static str, Support)]) -> BTreeMap<&'static str, Support> {
    nostr::session::FEATURES
        .iter()
        .map(|f| {
            (
                *f,
                pairs
                    .iter()
                    .find(|(name, _)| name == f)
                    .map_or(Support::Unsupported, |(_, s)| *s),
            )
        })
        .collect()
}

fn settings_schema() -> Value {
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object",
        "additionalProperties":false,"required":["effort"],
        "properties":{"effort":{"enum":["low","high"]}}})
}

impl LoopEngine {
    /// Mid-turn steering with engine replay; the host emulates the queue.
    pub fn mid_turn() -> Self {
        use Support::*;
        Self::new(Profile {
            name: "loop-mid-turn",
            engine: b"loop-engine 1 mid-turn".to_vec(),
            protocol: br#"{"type":"object"}"#.to_vec(),
            features: features(&[
                ("create", Native),
                ("resume", Native),
                ("import", Emulated),
                ("close", Native),
                ("configure", Native),
                ("submit", Native),
                ("queue", Emulated),
                ("reorder", Emulated),
                ("steer", Native),
                ("interrupt", Native),
                ("history", Native),
                ("questions", Native),
            ]),
            steering: Steering {
                native: "mid_turn",
                emulation: None,
                acknowledgment: "engine_replay",
            },
            settings_schema: settings_schema(),
        })
    }
    /// Turn-boundary input only; steering is emulated by cancel-and-continue.
    pub fn boundary() -> Self {
        use Support::*;
        Self::new(Profile {
            name: "loop-boundary",
            engine: b"loop-engine 1 boundary".to_vec(),
            protocol: br#"{"type":"object"}"#.to_vec(),
            features: features(&[
                ("create", Native),
                ("close", Native),
                ("submit", Native),
                ("queue", Emulated),
                ("steer", Emulated),
                ("interrupt", Native),
                ("history", Native),
            ]),
            steering: Steering {
                native: "turn_boundary",
                emulation: Some("cancel_and_continue"),
                acknowledgment: "next_turn_start",
            },
            settings_schema: settings_schema(),
        })
    }
    fn new(profile: Profile) -> Self {
        Self {
            profile,
            sessions: BTreeMap::new(),
            log: Vec::new(),
            counter: 0,
            started: 0,
            fail_answer: false,
            fail_start: false,
        }
    }
    fn capture(
        &mut self,
        direction: &'static str,
        method: &str,
        id: Option<String>,
        payload: &Value,
    ) {
        self.log.push(Capture {
            direction,
            method: method.into(),
            native_id: id,
            payload: Some(serde_json::to_vec(payload).unwrap_or_default()),
        });
    }
}

impl Engine for LoopEngine {
    fn profile(&self) -> Profile {
        self.profile.clone()
    }
    fn open(&mut self, resume: Option<&[u8]>) -> Result<String> {
        self.counter += 1;
        let id = match resume {
            Some(bytes) => {
                let state: Value =
                    serde_json::from_slice(bytes).map_err(|_| "malformed: resume")?;
                let id = state["native"]
                    .as_str()
                    .ok_or("malformed: resume")?
                    .to_string();
                let turns = state["turns"].as_u64().unwrap_or_default();
                self.sessions.entry(id.clone()).or_default().turns = turns;
                id
            }
            None => {
                let id = format!("loop-{}-{}", self.profile.name, self.counter);
                self.sessions.insert(id.clone(), Native::default());
                id
            }
        };
        self.capture(
            "to_engine",
            "session/open",
            Some(id.clone()),
            &json!({"resume":resume.is_some()}),
        );
        Ok(id)
    }
    fn start(&mut self, native: &str, turn: &str, input: &[u8]) -> Result<()> {
        let input: Value = serde_json::from_slice(input).map_err(|_| "malformed: turn input")?;
        let session = self
            .sessions
            .get_mut(native)
            .ok_or("unavailable: native session")?;
        if session.run.is_some() {
            return Err("conflict: a turn is already running".into());
        }
        session.turns += 1;
        session.run = Some(Run {
            turn: turn.into(),
            prompt: input["prompt"].as_str().unwrap_or_default().into(),
            steps: input["steps"].as_u64().unwrap_or(1).clamp(1, 64),
            ask: input["ask"].as_str().map(str::to_string),
            corrections: input["corrections"].as_array().cloned().unwrap_or_default(),
            ..Run::default()
        });
        self.started += 1;
        self.capture("to_engine", "turn/start", Some(turn.into()), &input);
        if self.fail_start {
            return Err("unknown: native start reply lost".into());
        }
        Ok(())
    }
    fn step(&mut self, native: &str) -> Observation {
        let Some(session) = self.sessions.get_mut(native) else {
            return Observation::Lost;
        };
        let Some(run) = session.run.as_mut() else {
            return Observation::Lost;
        };
        if run.interrupted {
            let output =
                json!({"prompt":run.prompt,"interrupted":true,"corrections":run.corrections});
            let turn = run.turn.clone();
            session.run = None;
            self.capture("from_engine", "turn/completed", Some(turn), &output);
            return Observation::Finished {
                outcome: "cancelled",
                cause: "user_interrupt",
                output: serde_json::to_vec(&output).unwrap_or_default(),
            };
        }
        if let Some(question) = run.ask.clone()
            && run.answer.is_none()
        {
            let id = format!("{}-q", run.turn);
            if run.asked.is_none() {
                run.asked = Some(id.clone());
                let prompt = json!({"question":question});
                self.capture("from_engine", "question/request", Some(id.clone()), &prompt);
                return Observation::Question {
                    native_id: id,
                    prompt: serde_json::to_vec(&prompt).unwrap_or_default(),
                };
            }
            return Observation::Running;
        }
        run.steps -= 1;
        if run.steps > 0 {
            return Observation::Running;
        }
        let output = json!({"prompt":run.prompt,"corrections":run.corrections,"answer":run.answer,
            "turn_number":session.turns});
        let turn = run.turn.clone();
        session.run = None;
        self.capture("from_engine", "turn/completed", Some(turn), &output);
        Observation::Finished {
            outcome: "completed",
            cause: "normal",
            output: serde_json::to_vec(&output).unwrap_or_default(),
        }
    }
    fn steer(&mut self, native: &str, turn: &str, correction: &[u8]) -> Result<bool> {
        if self.profile.steering.native != "mid_turn" {
            return Err(
                "unsupported_feature: the engine takes input only at turn boundaries".into(),
            );
        }
        let correction: Value =
            serde_json::from_slice(correction).map_err(|_| "malformed: correction")?;
        let run = self
            .sessions
            .get_mut(native)
            .and_then(|s| s.run.as_mut())
            .filter(|r| r.turn == turn)
            .ok_or("stale: turn is not running")?;
        run.corrections.push(correction.clone());
        self.capture("to_engine", "turn/steer", Some(turn.into()), &correction);
        self.capture("from_engine", "user/replay", Some(turn.into()), &correction);
        Ok(true)
    }
    fn interrupt(&mut self, native: &str, turn: &str) -> Result<()> {
        let run = self
            .sessions
            .get_mut(native)
            .and_then(|s| s.run.as_mut())
            .filter(|r| r.turn == turn)
            .ok_or("stale: turn is not running")?;
        run.interrupted = true;
        self.capture("to_engine", "turn/interrupt", Some(turn.into()), &json!({}));
        Ok(())
    }
    fn answer(&mut self, native: &str, native_id: &str, response: &[u8]) -> Result<()> {
        let response: Value = serde_json::from_slice(response).map_err(|_| "malformed: answer")?;
        let run = self
            .sessions
            .get_mut(native)
            .and_then(|s| s.run.as_mut())
            .filter(|r| r.asked.as_deref() == Some(native_id))
            .ok_or("stale: no such pending question")?;
        if run.answer.is_some() {
            return Err("conflict: already answered".into());
        }
        run.answer = Some(response.clone());
        self.capture(
            "to_engine",
            "question/answer",
            Some(native_id.into()),
            &response,
        );
        if self.fail_answer {
            return Err("unavailable: delivery acknowledgment lost".into());
        }
        Ok(())
    }
    fn resume_state(&mut self, native: &str) -> Option<Vec<u8>> {
        let session = self.sessions.get(native)?;
        serde_json::to_vec(&json!({"native":native,"turns":session.turns})).ok()
    }
    fn captures(&mut self) -> Vec<Capture> {
        std::mem::take(&mut self.log)
    }
    fn started(&self) -> usize {
        self.started
    }
}
