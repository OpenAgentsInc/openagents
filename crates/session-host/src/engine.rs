//! Engine adapters. An adapter states its exact feature matrix and steering
//! facts; the host never upgrades an emulated or unsupported claim.
use crate::Result;
use serde::{Deserialize, Serialize};
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
    /// Effects of the actual host and engine binding, without adding authority.
    pub effects: Value,
    /// One support level per NIP-SESS feature.
    pub features: BTreeMap<&'static str, Support>,
    pub steering: Steering,
    /// The closed settings schema this adapter accepts.
    pub settings_schema: Value,
}

/// How much of a native protocol message was retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureCoverage {
    Complete,
    Redacted,
    Unavailable,
    Unsupported,
}
impl CaptureCoverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Redacted => "redacted",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One captured native protocol message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub direction: &'static str,
    pub method: String,
    pub native_id: Option<String>,
    pub payload: Option<Vec<u8>>,
    pub coverage: CaptureCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub native_id: String,
    pub method: String,
    pub input: Vec<u8>,
    pub capture: Capture,
}

#[derive(Clone, Debug)]
pub struct ActionProfile {
    pub method: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
    pub effects: Value,
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
    Approval(ApprovalRequest),
    /// Authoritative terminal engine evidence.
    Finished {
        outcome: &'static str,
        cause: &'static str,
        output: Vec<u8>,
    },
    /// A required native message the adapter cannot handle. Keep its actual
    /// capture; the host blocks input and never fabricates a successful reply.
    ProtocolFault {
        request: Capture,
    },
    /// The engine cannot report this turn (for example after a restart).
    Lost,
}

/// Exact values delivered to and acknowledged by a native adapter. Artifact
/// references stay in the host; adapters receive only admitted content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    pub settings: Value,
    pub model: Vec<u8>,
    pub recipient: Vec<u8>,
    pub policy: Vec<u8>,
}

/// Actual admitted delivery. Context and frame bytes stay distinct from the
/// input, and their presence does not grant tool or spending authority.
#[derive(Clone, Debug)]
pub struct TurnInput {
    pub input: Vec<u8>,
    pub context: Vec<u8>,
    pub task_frame: Vec<u8>,
}
impl TurnInput {
    pub fn check_size(&self) -> Result<()> {
        if [&self.input, &self.context, &self.task_frame]
            .iter()
            .any(|bytes| bytes.len() > 1_048_576)
        {
            return Err("limit_exceeded: native turn input, context, or frame".into());
        }
        Ok(())
    }
    fn digests(&self) -> Value {
        json!({"input":nostr::contracts::digest_bytes(&self.input),
            "context":nostr::contracts::digest_bytes(&self.context),
            "task_frame":nostr::contracts::digest_bytes(&self.task_frame)})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoPurpose {
    Input,
    Observation,
    Cleanup,
}

/// A local I/O ceiling supplied inside the host's current admission. Cleanup
/// can observe or stop existing work; it cannot start work or approve effects.
#[derive(Clone, Copy, Debug)]
pub struct IoBudget {
    pub(crate) deadline: std::time::Instant,
    pub(crate) purpose: IoPurpose,
}
impl IoBudget {
    pub fn deadline(self) -> std::time::Instant {
        self.deadline
    }
    pub fn purpose(self) -> IoPurpose {
        self.purpose
    }
}

pub trait Engine {
    fn profile(&self) -> Profile;
    /// I/O engines honor this absolute ceiling at each write and wait. Pure
    /// in-process engines need no transport timer. A budget grants no operation.
    fn io_budget(&mut self, _budget: IoBudget) -> Result<()> {
        Ok(())
    }
    fn approval_actions(&self) -> Vec<ActionProfile> {
        Vec::new()
    }
    fn check_approval(&self, _native: &str, _request: &ApprovalRequest) -> Result<()> {
        Err("unsupported_feature: native approval checking is not implemented".into())
    }
    fn approve(&mut self, _native: &str, _request: &ApprovalRequest, _allow: bool) -> Result<()> {
        Err("unsupported_feature: native approval delivery is not implemented".into())
    }
    /// Validate without starting a process, sending input, or changing state.
    fn check_configuration(&self, _configuration: &Configuration) -> Result<()> {
        Err("unsupported_feature: native configuration validation is not implemented".into())
    }
    /// Whether this instance still owns this exact native session.
    fn owns(&self, _native: &str) -> bool {
        false
    }
    /// Open or resume a native session. Resume state is opaque bytes.
    fn open(&mut self, resume: Option<&[u8]>) -> Result<String>;
    fn open_configured(
        &mut self,
        _resume: Option<&[u8]>,
        _configuration: &Configuration,
    ) -> Result<(String, Configuration)> {
        Err("unsupported_feature: native configured opening is not implemented".into())
    }
    fn configure(
        &mut self,
        _native: &str,
        _configuration: &Configuration,
    ) -> Result<Configuration> {
        Err("unsupported_feature: native configuration is not implemented".into())
    }
    /// Stop this exact idle native session. Success acknowledges its closure.
    fn close(&mut self, _native: &str) -> Result<()> {
        Err("unsupported_feature: native close is not implemented".into())
    }
    fn start(&mut self, native: &str, turn: &str, input: &[u8]) -> Result<()>;
    /// Validate admitted delivery without native effects.
    fn check_turn_input(&self, _input: &TurnInput) -> Result<()> {
        Err("unsupported_feature: admitted frame and context delivery is not implemented".into())
    }
    fn start_admitted(&mut self, _native: &str, _turn: &str, _input: &TurnInput) -> Result<()> {
        Err("unsupported_feature: admitted frame and context delivery is not implemented".into())
    }
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
    effort: String,
    admitted: Option<Value>,
    approval: Option<Value>,
    approval_asked: bool,
    approval_answer: Option<bool>,
    action_digest: Option<String>,
}

#[derive(Default)]
struct Native {
    turns: u64,
    run: Option<Run>,
    configuration: Option<Configuration>,
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
    /// Fault: lose the opening acknowledgment after creating the native session.
    pub fail_open: bool,
    /// Fault: emit a required reverse request the adapter does not support.
    pub protocol_fault: bool,
    /// Fault: lose the reply after the native session has closed.
    pub fail_close: bool,
    /// Fault: lose the acknowledgment after consuming a steering correction.
    pub fail_steer: bool,
    /// Fault: lose the acknowledgment after applying native settings.
    pub fail_configure: bool,
    pub fail_approval: bool,
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

fn check_loop_configuration(configuration: &Configuration) -> Result<()> {
    if configuration
        .settings
        .as_object()
        .is_none_or(|o| o.len() != 1)
        || !matches!(
            configuration.settings["effort"].as_str(),
            Some("low" | "high")
        )
    {
        return Err("unsupported_feature: unsupported loop settings".into());
    }
    let model: Value = serde_json::from_slice(&configuration.model)
        .map_err(|_| "malformed: loop model descriptor")?;
    if model != json!({"model":"loop"}) {
        return Err("unsupported_feature: loop model descriptor differs".into());
    }
    Ok(())
}

fn loop_approval(run: &Run) -> Result<ApprovalRequest> {
    let approval = run.approval.as_ref().ok_or("stale: no native approval")?;
    let native_id = format!("{}-approval", run.turn);
    let input = nostr::contracts::jcs(&approval["value"]).map_err(|e| e.to_string())?;
    Ok(ApprovalRequest {
        native_id: native_id.clone(),
        method: "hash".into(),
        input,
        capture: Capture {
            coverage: CaptureCoverage::Complete,
            direction: "from_engine",
            method: "approval/request".into(),
            native_id: Some(native_id.clone()),
            payload: Some(
                serde_json::to_vec(
                    &json!({"request":native_id,"method":"hash","input":approval["value"]}),
                )
                .map_err(|e| e.to_string())?,
            ),
        },
    })
}

impl LoopEngine {
    /// An explicit pure-action approval profile; no external tool execution.
    pub fn approval() -> Self {
        let mut engine = Self::mid_turn();
        engine.profile.name = "loop-approval";
        engine.profile.engine = b"loop-engine 4 single-use hash approval".to_vec();
        engine.profile.features.insert("approvals", Support::Native);
        engine
    }
    /// Mid-turn steering with engine replay; the host emulates the queue.
    pub fn mid_turn() -> Self {
        use Support::*;
        Self::new(Profile {
            name: "loop-mid-turn",
            engine: b"loop-engine 3 mid-turn admitted context".to_vec(),
            protocol: br#"{"type":"object"}"#.to_vec(),
            effects: json!({"reads":["artifact"],
                "writes":[],"network":[],
                "process":false,"delegates":false,"spend":false}),
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
            engine: b"loop-engine 3 boundary admitted context".to_vec(),
            protocol: br#"{"type":"object"}"#.to_vec(),
            effects: json!({"reads":["artifact"],
                "writes":[],"network":[],
                "process":false,"delegates":false,"spend":false}),
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
            fail_open: false,
            protocol_fault: false,
            fail_close: false,
            fail_steer: false,
            fail_configure: false,
            fail_approval: false,
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
            coverage: CaptureCoverage::Complete,
            direction,
            method: method.into(),
            native_id: id,
            payload: Some(serde_json::to_vec(payload).unwrap_or_default()),
        });
    }
}

impl Engine for LoopEngine {
    fn approval_actions(&self) -> Vec<ActionProfile> {
        if self.profile.features.get("approvals") != Some(&Support::Native) {
            return Vec::new();
        }
        vec![ActionProfile {
            method: "hash",
            input_schema: json!({}),
            output_schema: json!({"type":"object","additionalProperties":false,"required":["digest"],
                "properties":{"digest":{"type":"string","minLength":71,"maxLength":71}}}),
            effects: self.profile.effects.clone(),
        }]
    }
    fn check_approval(&self, native: &str, request: &ApprovalRequest) -> Result<()> {
        if self.approval_actions().is_empty() {
            return Err("unsupported_feature: this engine has no native approval".into());
        }
        let run = self
            .sessions
            .get(native)
            .and_then(|s| s.run.as_ref())
            .ok_or("stale: native turn ended")?;
        if !run.approval_asked
            || run.approval_answer.is_some()
            || run.interrupted
            || loop_approval(run)? != *request
        {
            return Err("identity_mismatch: native approval request changed or ended".into());
        }
        Ok(())
    }
    fn approve(&mut self, native: &str, request: &ApprovalRequest, allow: bool) -> Result<()> {
        self.check_approval(native, request)?;
        self.sessions
            .get_mut(native)
            .and_then(|s| s.run.as_mut())
            .ok_or("native turn")?
            .approval_answer = Some(allow);
        self.capture(
            "to_engine",
            "approval/response",
            Some(request.native_id.clone()),
            &json!({"allow_once":allow}),
        );
        if self.fail_approval {
            return Err("unknown: native approval acknowledgment lost".into());
        }
        Ok(())
    }
    fn check_configuration(&self, configuration: &Configuration) -> Result<()> {
        check_loop_configuration(configuration)
    }
    fn open_configured(
        &mut self,
        resume: Option<&[u8]>,
        configuration: &Configuration,
    ) -> Result<(String, Configuration)> {
        check_loop_configuration(configuration)?;
        let native = self.open(resume)?;
        self.sessions
            .get_mut(&native)
            .ok_or("native session")?
            .configuration = Some(configuration.clone());
        self.capture(
            "from_engine",
            "session/configured",
            Some(native.clone()),
            &serde_json::to_value(configuration).map_err(|e| e.to_string())?,
        );
        Ok((native, configuration.clone()))
    }
    fn configure(&mut self, native: &str, configuration: &Configuration) -> Result<Configuration> {
        check_loop_configuration(configuration)?;
        let session = self
            .sessions
            .get_mut(native)
            .ok_or("unknown: native session unavailable")?;
        if session.run.is_some() {
            return Err("conflict: configuration needs an idle engine".into());
        }
        if session.configuration.as_ref().is_none_or(|old| {
            old.model != configuration.model
                || old.recipient != configuration.recipient
                || old.policy != configuration.policy
        }) {
            return Err("unsupported_feature: model, recipient, or policy changes require runtime replacement".into());
        }
        session.configuration = Some(configuration.clone());
        self.capture(
            "from_engine",
            "session/configured",
            Some(native.into()),
            &serde_json::to_value(configuration).map_err(|e| e.to_string())?,
        );
        if self.fail_configure {
            return Err("unknown: native configuration acknowledgment lost".into());
        }
        Ok(configuration.clone())
    }
    fn owns(&self, native: &str) -> bool {
        self.sessions.contains_key(native)
    }
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
        if self.fail_open {
            return Err("unknown: native opening acknowledgment lost".into());
        }
        Ok(id)
    }
    fn close(&mut self, native: &str) -> Result<()> {
        let session = self
            .sessions
            .get(native)
            .ok_or("unknown: native session unavailable")?;
        if session.run.is_some() {
            return Err("conflict: native session is still running".into());
        }
        self.capture(
            "to_engine",
            "session/close",
            Some(native.into()),
            &json!({}),
        );
        self.sessions.remove(native);
        self.capture(
            "from_engine",
            "session/closed",
            Some(native.into()),
            &json!({}),
        );
        if self.fail_close {
            return Err("unknown: native close acknowledgment lost".into());
        }
        Ok(())
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
        let effort = session
            .configuration
            .as_ref()
            .and_then(|c| c.settings["effort"].as_str())
            .unwrap_or("low")
            .to_string();
        let step_limit = if effort == "high" { 64 } else { 32 };
        session.run = Some(Run {
            turn: turn.into(),
            prompt: input["prompt"].as_str().unwrap_or_default().into(),
            steps: input["steps"].as_u64().unwrap_or(1).clamp(1, step_limit),
            ask: input["ask"].as_str().map(str::to_string),
            effort,
            corrections: input["corrections"].as_array().cloned().unwrap_or_default(),
            approval: input.get("approval").cloned(),
            ..Run::default()
        });
        self.started += 1;
        self.capture("to_engine", "turn/start", Some(turn.into()), &input);
        if self.fail_start {
            return Err("unknown: native start reply lost".into());
        }
        Ok(())
    }
    fn check_turn_input(&self, input: &TurnInput) -> Result<()> {
        input.check_size()?;
        let body: Value = nostr::contracts::parse_strict_bounded(&input.input, 1_048_576)
            .map_err(|e| e.to_string())?;
        if !body.is_object() || !body["prompt"].is_string() {
            return Err("malformed: loop input requires a prompt".into());
        }
        if let Some(approval) = body.get("approval") {
            if self.approval_actions().is_empty() {
                return Err("unsupported_feature: native approvals are unavailable".into());
            }
            if approval
                .as_object()
                .is_none_or(|o| o.len() != 2 || !o.contains_key("value"))
                || approval["method"] != "hash"
                || nostr::contracts::jcs(&approval["value"])
                    .map_err(|e| e.to_string())?
                    .len()
                    > 32_768
            {
                return Err("malformed: native hash approval input".into());
            }
        }
        Ok(())
    }
    fn start_admitted(&mut self, native: &str, turn: &str, input: &TurnInput) -> Result<()> {
        self.check_turn_input(input)?;
        let digests = input.digests();
        let result = self.start(native, turn, &input.input);
        if let Some(run) = self
            .sessions
            .get_mut(native)
            .and_then(|s| s.run.as_mut())
            .filter(|r| r.turn == turn)
        {
            run.admitted = Some(digests.clone());
            self.capture("to_engine", "turn/context", Some(turn.into()), &digests);
        }
        result
    }
    fn step(&mut self, native: &str) -> Observation {
        let Some(session) = self.sessions.get_mut(native) else {
            return Observation::Lost;
        };
        let Some(run) = session.run.as_mut() else {
            return Observation::Lost;
        };
        if run.interrupted {
            let output = json!({"prompt":run.prompt,"interrupted":true,"corrections":run.corrections,"effort":run.effort,"admitted":run.admitted,"action_digest":run.action_digest});
            let turn = run.turn.clone();
            session.run = None;
            self.capture("from_engine", "turn/completed", Some(turn), &output);
            return Observation::Finished {
                outcome: "cancelled",
                cause: "user_interrupt",
                output: serde_json::to_vec(&output).unwrap_or_default(),
            };
        }
        if run.approval.is_some() {
            if !run.approval_asked {
                run.approval_asked = true;
                return match loop_approval(run) {
                    Ok(request) => Observation::Approval(request),
                    Err(_) => Observation::Lost,
                };
            }
            match run.approval_answer {
                None => return Observation::Running,
                Some(false) => {
                    let output =
                        json!({"approved":false,"prompt":run.prompt,"admitted":run.admitted});
                    let turn = run.turn.clone();
                    session.run = None;
                    self.capture("from_engine", "turn/completed", Some(turn), &output);
                    return Observation::Finished {
                        outcome: "cancelled",
                        cause: "policy",
                        output: serde_json::to_vec(&output).unwrap_or_default(),
                    };
                }
                Some(true) if run.action_digest.is_none() => {
                    let request = match loop_approval(run) {
                        Ok(r) => r,
                        Err(_) => return Observation::Lost,
                    };
                    let digest = nostr::contracts::digest_bytes(&request.input);
                    run.action_digest = Some(digest.clone());
                    self.capture(
                        "from_engine",
                        "action/completed",
                        Some(request.native_id),
                        &json!({"digest":digest}),
                    );
                    return Observation::Running;
                }
                Some(true) => {}
            }
        }
        if self.protocol_fault {
            self.protocol_fault = false;
            return Observation::ProtocolFault {
                request: Capture {
                    coverage: CaptureCoverage::Complete,
                    direction: "from_engine",
                    method: "unsupported/required".into(),
                    native_id: Some(format!("{}-unknown", run.turn)),
                    payload: Some(br#"{"required":true,"operation":"unknown"}"#.to_vec()),
                },
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
        let output = json!({"prompt":run.prompt,"corrections":run.corrections,"answer":run.answer,"effort":run.effort,
            "turn_number":session.turns,"admitted":run.admitted,"action_digest":run.action_digest});
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
        if self.fail_steer {
            return Err("unknown: steering acknowledgment lost".into());
        }
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
