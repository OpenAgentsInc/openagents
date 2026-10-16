//! Managed Codex sessions over an explicitly installed, admitted transport.
//!
//! The connector is trusted host code, like `Engine` and ENV's runtime binder.
//! It must enforce its declared effects and zero monetary cost, isolate native
//! tools and credentials, and honor the I/O deadline before returning a port.
//! This module neither launches a process nor imports a user's Codex account.
use crate::{Result, engine::*};
use coder_sdk::codex::managed::{
    self, Direction, Event, Managed, Operation, ReverseRequest, Written,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

/// Exact installed runtime identity and native thread options. These values
/// are host configuration, never values supplied by a session request.
#[derive(Clone, PartialEq)]
pub struct Installation {
    pub engine: Vec<u8>,
    pub protocol: Vec<u8>,
    pub effects: Value,
    pub model: String,
    pub provider: String,
    pub cwd: String,
}

/// Creates a fresh, exclusive app-server connection inside an admitted effect.
/// A connector must refuse if it cannot enforce zero cost, the installed
/// effects, and the supplied deadline. An app-server option is not enforcement.
pub trait Connector {
    fn installation(&self) -> Installation;
    fn connect(&mut self, configuration: &Configuration, budget: IoBudget) -> Result<Managed>;
}

struct Session {
    port: Managed,
    logical_turn: Option<String>,
    native_turn: Option<String>,
    terminal: Option<Observation>,
    reverse: Option<ReverseRequest>,
    output: Vec<u8>,
    lost: bool,
}

/// NIP-SESS mapping for the SDK's explicit request/acknowledgment port.
pub struct CodexEngine {
    connector: Box<dyn Connector>,
    installation: Installation,
    runtime: Runtime,
    budget: Option<IoBudget>,
    sessions: BTreeMap<String, Session>,
    unknown_openings: Vec<Managed>,
    captures: Vec<Capture>,
    started: usize,
}

fn failure(error: managed::Error) -> String {
    format!("unknown: native Codex transport: {error}")
}
fn capture(source: managed::Capture) -> Capture {
    let body: Value = serde_json::from_slice(&source.bytes).unwrap_or(Value::Null);
    Capture {
        direction: if source.direction == Direction::Sent {
            "to_engine"
        } else {
            "from_engine"
        },
        method: body["method"].as_str().unwrap_or("response").into(),
        native_id: body.get("id").map(Value::to_string),
        payload: Some(source.bytes),
        coverage: if source.complete {
            CaptureCoverage::Complete
        } else {
            CaptureCoverage::Unavailable
        },
    }
}
fn text(bytes: &[u8]) -> Result<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| "malformed: Codex input must be UTF-8".into())
}
fn items(input: &TurnInput) -> Result<Vec<Value>> {
    Ok(vec![
        json!({"type":"text","text":serde_json::to_string(&json!({
        "input":text(&input.input)?, "context":text(&input.context)?,
        "task_frame":text(&input.task_frame)?})).map_err(|e|e.to_string())?}),
    ])
}

impl CodexEngine {
    pub fn new(connector: Box<dyn Connector>) -> Result<Self> {
        let installation = connector.installation();
        if installation.engine.is_empty()
            || installation.engine.len() > 1_048_576
            || installation.protocol.is_empty()
            || installation.protocol.len() > 1_048_576
            || [
                &installation.model,
                &installation.provider,
                &installation.cwd,
            ]
            .iter()
            .any(|s| s.is_empty() || s.len() > 4096 || s.contains('\0'))
            || installation.effects["spend"] != false
            || installation.effects["delegates"] != false
        {
            return Err("unsupported_feature: invalid or spending Codex installation".into());
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            connector,
            installation,
            runtime,
            budget: None,
            sessions: BTreeMap::new(),
            unknown_openings: Vec::new(),
            captures: Vec::new(),
            started: 0,
        })
    }
    fn allowance(&self, input: bool) -> Result<(IoBudget, tokio::time::Instant)> {
        let budget = self
            .budget
            .ok_or("not_admitted: Codex I/O needs an admission")?;
        if budget.deadline() <= Instant::now() || input && budget.purpose() != IoPurpose::Input {
            return Err("not_admitted: expired or cleanup-only Codex allowance".into());
        }
        // A slow native peer cannot hold the resident host for the full job.
        Ok((
            budget,
            tokio::time::Instant::from_std(
                budget
                    .deadline()
                    .min(Instant::now() + Duration::from_secs(2)),
            ),
        ))
    }
    fn collect(&mut self, native: &str) {
        if let Some(session) = self.sessions.get_mut(native) {
            self.captures
                .extend(session.port.take_captures().into_iter().map(capture));
        }
    }
    fn event(session: &mut Session, event: Event) -> Result<Option<(Written, Value)>> {
        match event {
            Event::Waiting => {}
            Event::Answer { request, result } => {
                let result = result.map_err(|_| "unknown: native Codex rejected the request")?;
                if request.operation == Operation::Start {
                    session.native_turn = session.port.turn().map(str::to_owned);
                }
                return Ok(Some((request, result)));
            }
            Event::Reverse(request) => {
                session.reverse = Some(request);
                return Err("unsupported_feature: required Codex method".into());
            }
            Event::Notification { method, params } => {
                if method == "item/agentMessage/delta" {
                    if params["threadId"].as_str() != session.port.thread()
                        || params["turnId"].as_str() != session.native_turn.as_deref()
                    {
                        return Err("identity_mismatch: Codex output has no current turn".into());
                    }
                    let delta = params["delta"]
                        .as_str()
                        .ok_or("malformed: Codex message delta")?;
                    if session.output.len() + delta.len() > 65_536 {
                        return Err("limit_exceeded: Codex output".into());
                    }
                    session.output.extend_from_slice(delta.as_bytes());
                }
            }
            Event::Terminal {
                turn, status, body, ..
            } => {
                if session.native_turn.as_deref() != Some(&turn) {
                    return Err("identity_mismatch: Codex terminal turn".into());
                }
                let (outcome, cause) = match status.as_str() {
                    "completed" => ("completed", "normal"),
                    "interrupted" => ("cancelled", "user_interrupt"),
                    "failed" => ("failed", "provider_failure"),
                    _ => return Err("malformed: Codex terminal status".into()),
                };
                session.terminal = Some(Observation::Finished {
                    outcome,
                    cause,
                    output: serde_json::to_vec(
                        &json!({"text":text(&session.output)?,"native_terminal":body}),
                    )
                    .map_err(|e| e.to_string())?,
                });
                session.native_turn = None;
            }
            Event::Fault { .. } => return Err("unknown: Codex protocol fault".into()),
        }
        Ok(None)
    }
    fn ack(
        runtime: &Runtime,
        session: &mut Session,
        request: &Written,
        deadline: tokio::time::Instant,
    ) -> Result<Value> {
        for _ in 0..256 {
            let event = runtime
                .block_on(session.port.next(deadline))
                .map_err(failure)?;
            if let Some((actual, result)) = Self::event(session, event)? {
                if actual == *request {
                    return Ok(result);
                }
                return Err("identity_mismatch: unexpected Codex acknowledgment".into());
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
        }
        Err("unknown: Codex acknowledgment was not observed".into())
    }
}

impl Engine for CodexEngine {
    fn profile(&self) -> Profile {
        use Support::*;
        let features = nostr::session::FEATURES
            .iter()
            .map(|f| {
                (
                    *f,
                    match *f {
                        "create" | "submit" | "steer" | "interrupt" | "history" => Native,
                        "queue" | "reorder" => Emulated,
                        _ => Unsupported,
                    },
                )
            })
            .collect();
        Profile {
            name: "codex-managed",
            engine: serde_json::to_vec(&json!({"adapter":"codex-managed-v1",
            "engine":nostr::contracts::digest_bytes(&self.installation.engine),
            "model":self.installation.model,"provider":self.installation.provider,
            "cwd":self.installation.cwd,"effects":self.installation.effects}))
            .unwrap_or_default(),
            protocol: self.installation.protocol.clone(),
            effects: self.installation.effects.clone(),
            features,
            steering: Steering {
                native: "mid_turn",
                emulation: None,
                acknowledgment: "engine_replay",
            },
            settings_schema: json!({"type":"object","additionalProperties":false,"properties":{
                "effort":{"const":"low"}},"required":["effort"]}),
        }
    }
    fn io_budget(&mut self, budget: IoBudget) -> Result<()> {
        if budget.deadline() <= Instant::now() {
            return Err("stale: Codex I/O allowance".into());
        }
        self.budget = Some(budget);
        Ok(())
    }
    fn check_configuration(&self, configuration: &Configuration) -> Result<()> {
        let model: Value =
            serde_json::from_slice(&configuration.model).map_err(|_| "malformed: Codex model")?;
        if model != json!({"model":self.installation.model})
            || configuration.settings != json!({"effort":"low"})
        {
            return Err("unsupported_feature: installed Codex configuration differs".into());
        }
        text(&configuration.recipient)?;
        text(&configuration.policy)?;
        Ok(())
    }
    fn open(&mut self, _resume: Option<&[u8]>) -> Result<String> {
        Err("unsupported_feature: Codex opening requires an admitted configuration".into())
    }
    fn open_configured(
        &mut self,
        resume: Option<&[u8]>,
        configuration: &Configuration,
    ) -> Result<(String, Configuration)> {
        if resume.is_some() {
            return Err("unsupported_feature: Codex resume".into());
        }
        self.check_configuration(configuration)?;
        if self.connector.installation() != self.installation {
            return Err("identity_mismatch: installed Codex connector changed".into());
        }
        let (budget, deadline) = self.allowance(true)?;
        if self.sessions.len() + self.unknown_openings.len() >= 64 {
            return Err("limit_exceeded: Codex connection count".into());
        }
        let port = self.connector.connect(configuration, budget)?;
        let mut session = Session {
            port,
            logical_turn: None,
            native_turn: None,
            terminal: None,
            reverse: None,
            output: Vec::new(),
            lost: false,
        };
        let opened = (|| {
            let request = self
                .runtime
                .block_on(
                    session
                        .port
                        .initialize("openagents-session-host", "1", deadline),
                )
                .map_err(failure)?;
            Self::ack(&self.runtime, &mut session, &request, deadline)?;
            self.runtime
                .block_on(session.port.initialized(deadline))
                .map_err(failure)?;
            let params = json!({"model":self.installation.model,"modelProvider":self.installation.provider,
                "cwd":self.installation.cwd,"approvalPolicy":"unlessTrusted","sandbox":"readOnly",
                "ephemeral":true,"config":{"model_reasoning_effort":"low"},
                "developerInstructions":serde_json::to_string(&json!({"recipient":text(&configuration.recipient)?,
                    "policy":text(&configuration.policy)?})).map_err(|e|e.to_string())?});
            let request = self
                .runtime
                .block_on(session.port.open(params, deadline))
                .map_err(failure)?;
            let result = Self::ack(&self.runtime, &mut session, &request, deadline)?;
            if result["model"] != self.installation.model
                || result["modelProvider"] != self.installation.provider
                || result["cwd"] != self.installation.cwd
                || result["approvalPolicy"] != "unlessTrusted"
                || result["reasoningEffort"] != "low"
                || result["sandbox"]["type"] != "readOnly"
            {
                return Err("unknown: Codex effective configuration differs".into());
            }
            session
                .port
                .thread()
                .map(str::to_owned)
                .ok_or("unknown: Codex thread acknowledgment".into())
        })();
        self.captures
            .extend(session.port.take_captures().into_iter().map(capture));
        match opened {
            Ok(id) if !self.sessions.contains_key(&id) => {
                self.sessions.insert(id.clone(), session);
                Ok((id, configuration.clone()))
            }
            result => {
                if let Some(partial) = session.port.partial_capture() {
                    self.captures.push(capture(partial));
                }
                self.unknown_openings.push(session.port);
                Err(result
                    .err()
                    .unwrap_or_else(|| "unknown: Codex reused a native thread".into()))
            }
        }
    }
    fn owns(&self, native: &str) -> bool {
        self.sessions.contains_key(native)
    }
    fn check_turn_input(&self, input: &TurnInput) -> Result<()> {
        input.check_size()?;
        items(input)?;
        Ok(())
    }
    fn start(&mut self, _native: &str, _turn: &str, _input: &[u8]) -> Result<()> {
        Err("unsupported_feature: Codex requires admitted context and frame".into())
    }
    fn start_admitted(&mut self, native: &str, turn: &str, input: &TurnInput) -> Result<()> {
        self.check_turn_input(input)?;
        let (_, deadline) = self.allowance(true)?;
        let session = self.sessions.get_mut(native).ok_or("native")?;
        if session.lost
            || session.reverse.is_some()
            || session.terminal.is_some()
            || session.port.phase() != managed::Phase::Idle
        {
            return Err("conflict: Codex thread is not idle".into());
        }
        session.logical_turn = Some(turn.into());
        session.output.clear();
        // Once written, an uncertain start is never sent again by this binding.
        self.started += 1;
        let result = (|| {
            let request = self
                .runtime
                .block_on(session.port.start(items(input)?, deadline))
                .map_err(failure)?;
            Self::ack(&self.runtime, session, &request, deadline)?;
            Ok(())
        })();
        self.collect(native);
        result
    }
    fn step(&mut self, native: &str) -> Observation {
        let Ok((_, deadline)) = self.allowance(false) else {
            return Observation::Lost;
        };
        let Some(session) = self.sessions.get_mut(native) else {
            return Observation::Lost;
        };
        if let Some(terminal) = session.terminal.take() {
            return terminal;
        }
        if let Some(request) = session.reverse.as_ref() {
            return Observation::ProtocolFault {
                request: capture(managed::Capture {
                    direction: Direction::Received,
                    bytes: request.source().to_vec(),
                    complete: true,
                }),
            };
        }
        if session.lost {
            return Observation::Lost;
        }
        let deadline = deadline.min(tokio::time::Instant::now() + Duration::from_millis(20));
        let event = self.runtime.block_on(session.port.next(deadline));
        let outcome = match event {
            Ok(event) => Self::event(session, event),
            Err(error) => Err(failure(error)),
        };
        if outcome.is_err() && session.reverse.is_none() {
            session.lost = true;
        }
        let observation = if let Some(request) = session.reverse.as_ref() {
            Observation::ProtocolFault {
                request: capture(managed::Capture {
                    direction: Direction::Received,
                    bytes: request.source().to_vec(),
                    complete: true,
                }),
            }
        } else if let Some(terminal) = session.terminal.take() {
            terminal
        } else if session.lost {
            Observation::Lost
        } else {
            Observation::Running
        };
        if session.lost {
            if let Some(partial) = session.port.partial_capture() {
                self.captures.push(capture(partial));
            }
        }
        self.collect(native);
        observation
    }
    fn steer(&mut self, native: &str, turn: &str, correction: &[u8]) -> Result<bool> {
        let (_, deadline) = self.allowance(true)?;
        let session = self.sessions.get_mut(native).ok_or("native")?;
        if session.logical_turn.as_deref() != Some(turn) {
            return Err("stale: Codex logical turn".into());
        }
        let actual = session
            .native_turn
            .clone()
            .ok_or("unknown: Codex native turn")?;
        let result = (|| {
            let request = self
                .runtime
                .block_on(session.port.steer(
                    &actual,
                    vec![json!({"type":"text","text":text(correction)?})],
                    deadline,
                ))
                .map_err(failure)?;
            let ack = Self::ack(&self.runtime, session, &request, deadline)?;
            if ack["turnId"] != actual {
                return Err("identity_mismatch: Codex steer acknowledgment".into());
            }
            Ok(true)
        })();
        self.collect(native);
        result
    }
    fn interrupt(&mut self, native: &str, turn: &str) -> Result<()> {
        let (_, deadline) = self.allowance(false)?;
        let session = self.sessions.get_mut(native).ok_or("native")?;
        if session.logical_turn.as_deref() != Some(turn) {
            return Err("stale: Codex logical turn".into());
        }
        let actual = session
            .native_turn
            .clone()
            .ok_or("unknown: Codex native turn")?;
        let result = (|| {
            let request = self
                .runtime
                .block_on(session.port.interrupt(&actual, deadline))
                .map_err(failure)?;
            Self::ack(&self.runtime, session, &request, deadline)?;
            Ok(())
        })();
        self.collect(native);
        result
    }
    fn reject_required(&mut self, native: &str) -> Result<()> {
        let (budget, deadline) = self.allowance(false)?;
        if budget.purpose() != IoPurpose::Cleanup {
            return Err("not_admitted: Codex denial requires cleanup".into());
        }
        let session = self.sessions.get_mut(native).ok_or("native")?;
        let Some(request) = session.reverse.take() else {
            return Ok(());
        };
        let result = (|| {
            if matches!(
                request.method(),
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) {
                self.runtime
                    .block_on(session.port.permission(&request, false, deadline))
                    .map_err(failure)?;
            } else {
                self.runtime
                    .block_on(session.port.deny_unsupported(&request, deadline))
                    .map_err(failure)?;
            }
            let turn = session
                .native_turn
                .clone()
                .ok_or("unknown: native turn for cleanup")?;
            let stop = self
                .runtime
                .block_on(session.port.interrupt(&turn, deadline))
                .map_err(failure)?;
            Self::ack(&self.runtime, session, &stop, deadline)?;
            Ok(())
        })();
        self.collect(native);
        result
    }
    fn answer(&mut self, _native: &str, _id: &str, _response: &[u8]) -> Result<()> {
        Err("unsupported_feature: Codex questions".into())
    }
    fn resume_state(&mut self, _native: &str) -> Option<Vec<u8>> {
        None
    }
    fn captures(&mut self) -> Vec<Capture> {
        std::mem::take(&mut self.captures)
    }
    fn started(&self) -> usize {
        self.started
    }
}
