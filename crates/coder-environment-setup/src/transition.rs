//! Setup session records and their pure transitions.
//!
//! [`apply`] performs no I/O: it returns the next record (or `None` when
//! the operation is already true) or a typed [`Refusal`]. The service
//! retains the record before calling the provider, so a crash or a lost
//! reply leaves the intent and the command identity on disk.

use crate::*;
use coder_environment::evidence::Sealed;
use coder_working_computer::provider::{CommandCursor, CommandProgress, CommandSpec, Outcome};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandPurpose {
    /// Inspect manifests, inventory, and entry points.
    Discover,
    /// Run the install script of one exact recipe revision.
    Install {
        recipe_revision: u64,
        recipe_digest: String,
        attempt: u32,
    },
    /// A read-only check the setup owner runs (for example that no
    /// credential reached `.git/config`).
    Audit,
}

/// One command's lifecycle. `Unknown` blocks every new command until a
/// read of the same identity reconciles it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Run {
    /// Intent and identity retained; the provider was not asked yet, or
    /// a read proved it never started.
    Requested,
    Running {
        operation: String,
    },
    Unknown {
        reason: String,
    },
    Exited {
        code: i64,
    },
    TimedOut,
    Stopped {
        reason: String,
    },
    /// The process disappeared without a recorded exit.
    Lost,
    /// The provider definitely did not start it.
    NotStarted {
        reason: String,
    },
}
impl Run {
    pub fn active(&self) -> bool {
        matches!(
            self,
            Self::Requested | Self::Running { .. } | Self::Unknown { .. }
        )
    }
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Exited { code: 0 })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Deadline,
    Owner,
    SessionEnded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStep {
    pub run: Run,
    pub at_ms: u64,
}

/// The evidence call that holds a command's bytes in one segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCall {
    pub segment: String,
    pub call: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRecord {
    pub id: String,
    pub request_id: String,
    pub purpose: CommandPurpose,
    /// Exact command, cwd, named credentials, non-secret environment,
    /// timeout, and digest, retained before the provider is called.
    pub spec: CommandSpec,
    pub deadline_ms: u64,
    pub run: Run,
    /// Output bytes already read from the provider and recorded.
    pub cursor: CommandCursor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopReason>,
    pub evidence: Vec<EvidenceCall>,
    pub history: Vec<RunStep>,
    pub requested_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupState {
    Provisioning,
    Discovering,
    Installing,
    /// The last install failed; the agent revises the recipe and reruns.
    Repairing,
    Installed {
        recipe_revision: u64,
    },
    /// The turn ended for user input; the computer is checkpointed.
    AwaitingInput {
        question: String,
    },
    Ended,
    Failed {
        reason: String,
    },
    Cancelled {
        reason: String,
    },
}
impl SetupState {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Ended | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<Sealed>,
    /// The owner restarted before sealing it; its open calls are gaps.
    #[serde(default)]
    pub interrupted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolResult {
    RecipeRevised { revision: u64, digest: String },
    CommandStarted { command: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEntry {
    pub fingerprint: String,
    pub result: ToolResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steer {
    pub at_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupSession {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub environment: String,
    /// The dedicated setup computer.
    pub computer: String,
    pub owner: Principal,
    pub objective: String,
    pub admission: Admission,
    pub state: SetupState,
    /// The setup computer's current turn generation, once dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    pub commands: Vec<CommandRecord>,
    pub steering: Vec<Steer>,
    /// Recipe revisions this session produced, in order.
    pub recipe_revisions: Vec<u64>,
    pub requests: BTreeMap<String, RequestEntry>,
    pub segments: Vec<Segment>,
    pub created_ms: u64,
    pub updated_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_ms: Option<u64>,
}

impl SetupSession {
    pub fn new(request: &SetupRequest, computer: &str, admission: Admission, now_ms: u64) -> Self {
        Self {
            schema: SCHEMA.into(),
            id: request.session.clone(),
            revision: 1,
            environment: request.environment.clone(),
            computer: computer.into(),
            owner: request.owner.clone(),
            objective: request.objective.clone(),
            admission,
            state: SetupState::Provisioning,
            generation: None,
            commands: vec![],
            steering: vec![],
            recipe_revisions: vec![],
            requests: BTreeMap::new(),
            segments: vec![],
            created_ms: now_ms,
            updated_ms: now_ms,
            ended_ms: None,
        }
    }
    pub fn command(&self, id: &str) -> Option<&CommandRecord> {
        self.commands.iter().find(|c| c.id == id)
    }
    fn command_mut(&mut self, id: &str) -> Result<&mut CommandRecord, Refusal> {
        self.commands
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| Refusal::UnknownCommand(id.into()))
    }
    pub fn active_command(&self) -> Option<&CommandRecord> {
        self.commands.iter().find(|c| c.run.active())
    }
    pub fn installs(&self) -> impl Iterator<Item = &CommandRecord> {
        self.commands
            .iter()
            .filter(|c| matches!(c.purpose, CommandPurpose::Install { .. }))
    }
    pub fn next_command_id(&self) -> String {
        format!("cmd-{}", self.commands.len() + 1)
    }
    pub fn segment(&self) -> Option<&Segment> {
        self.segments.last().filter(|s| s.sealed.is_none())
    }
    /// The replayed result of a request, or a conflict.
    pub fn replay(
        &self,
        request_id: &str,
        fingerprint: &str,
    ) -> Option<Result<ToolResult, Refusal>> {
        self.requests.get(request_id).map(|e| {
            if e.fingerprint == fingerprint {
                Ok(e.result.clone())
            } else {
                Err(Refusal::RequestConflict(request_id.into()))
            }
        })
    }
    /// Evidence is complete only when every segment sealed complete and
    /// none was interrupted.
    pub fn evidence_complete(&self) -> bool {
        !self.segments.is_empty()
            && self
                .segments
                .iter()
                .all(|s| !s.interrupted && s.sealed.as_ref().is_some_and(|x| x.status.complete()))
    }
    /// The working state implied by the commands so far.
    fn work_state(&self) -> SetupState {
        match self.installs().last() {
            Some(c) if c.run.succeeded() => match &c.purpose {
                CommandPurpose::Install {
                    recipe_revision, ..
                } => SetupState::Installed {
                    recipe_revision: *recipe_revision,
                },
                _ => unreachable!(),
            },
            Some(c) if !c.run.active() => SetupState::Repairing,
            Some(_) => SetupState::Installing,
            None => SetupState::Discovering,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA
            || !valid_id(&self.id)
            || !valid_id(&self.environment)
            || !valid_id(&self.computer)
        {
            return Err("The setup session schema or identity is invalid.");
        }
        if self.commands.len() > MAX_COMMANDS
            || self.steering.len() > MAX_STEERING
            || self.requests.len() > MAX_REQUESTS
            || self.segments.len() > MAX_SEGMENTS
        {
            return Err("The setup session exceeds its bounds.");
        }
        for (i, c) in self.commands.iter().enumerate() {
            if c.id != format!("cmd-{}", i + 1) || c.spec.id != c.id {
                return Err("The command sequence is broken.");
            }
        }
        if self.commands.iter().filter(|c| c.run.active()).count() > 1 {
            return Err("At most one command is active.");
        }
        Ok(())
    }

    /// Commands, steering, revisions, and request results only grow.
    pub fn preserves_history_of(&self, next: &SetupSession) -> bool {
        next.id == self.id
            && next.computer == self.computer
            && next.admission == self.admission
            && next.created_ms == self.created_ms
            && next.revision > self.revision
            && next.steering.starts_with(&self.steering)
            && next.recipe_revisions.starts_with(&self.recipe_revisions)
            && next.commands.len() >= self.commands.len()
            && self.commands.iter().zip(&next.commands).all(|(a, b)| {
                a.id == b.id
                    && a.spec == b.spec
                    && a.purpose == b.purpose
                    && b.history.starts_with(&a.history)
            })
            && self
                .requests
                .iter()
                .all(|(k, v)| next.requests.get(k) == Some(v))
            && (!self.state.terminal() || next.state == self.state)
    }
}

/// Digest of a spec with its own digest field empty.
pub fn spec_digest(spec: &CommandSpec) -> String {
    let mut bare = spec.clone();
    bare.digest = String::new();
    digest(&serde_json::to_vec(&bare).expect("spec encodes"))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "end", rename_all = "snake_case", deny_unknown_fields)]
pub enum End {
    Ended,
    Failed { reason: String },
    Cancelled { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// Open an evidence segment; an unsealed earlier one was interrupted.
    OpenSegment {
        id: String,
    },
    /// A command still active from an interrupted segment continues in a
    /// new call.
    ContinueEvidence {
        command: String,
        call: String,
    },
    SealSegment {
        id: String,
        sealed: Sealed,
    },
    /// The setup computer is awake for this turn generation.
    Dispatched {
        generation: u64,
    },
    /// End the turn for user input; no command may be active.
    Paused {
        question: String,
    },
    Steered {
        text: String,
    },
    RecipeRevised {
        request_id: String,
        fingerprint: String,
        revision: u64,
        digest: String,
    },
    RequestCommand {
        request_id: String,
        fingerprint: String,
        purpose: CommandPurpose,
        spec: CommandSpec,
    },
    ObserveStart {
        command: String,
        outcome: Outcome<String>,
    },
    ObserveRead {
        command: String,
        progress: CommandProgress,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        digest: Option<String>,
        cursor: CommandCursor,
    },
    RequestStop {
        command: String,
        reason: StopReason,
    },
    Finish {
        end: End,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Invalid(&'static str),
    Admission(&'static str),
    CredentialNotAdmitted(String),
    /// A URL carries a credential; use the session's Git auth instead.
    EmbeddedCredential,
    /// A recipe revision would change the session's pinned base, runtime,
    /// or platform.
    PinChanged,
    Ended,
    AwaitingInput,
    NotProvisioned,
    Busy(String),
    /// A command's outcome is unknown; reconcile it first.
    Unresolved(String),
    Deadline,
    RequestConflict(String),
    UnknownCommand(String),
    /// The provider's record for this identity is a different command.
    Drift(String),
    StaleDraft {
        expected: u64,
        current: u64,
    },
    Limit(&'static str),
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Admission(m) | Self::Limit(m) => f.write_str(m),
            Self::CredentialNotAdmitted(n) => {
                write!(f, "Credential {n} is not admitted for this setup.")
            }
            Self::EmbeddedCredential => f.write_str(
                "A URL carries a credential; name the Git credential and use the session's Git auth.",
            ),
            Self::PinChanged => f.write_str(
                "This session is pinned to its base, runtime, and platform; start a new setup to change them.",
            ),
            Self::Ended => f.write_str("The setup session has ended."),
            Self::AwaitingInput => f.write_str("The setup is waiting for input; steer it first."),
            Self::NotProvisioned => f.write_str("The setup computer is not awake yet."),
            Self::Busy(id) => write!(f, "Command {id} is still running."),
            Self::Unresolved(id) => {
                write!(f, "The outcome of {id} is unknown; reconcile it first.")
            }
            Self::Deadline => f.write_str("The setup deadline has passed."),
            Self::RequestConflict(id) => {
                write!(f, "Request {id} was already used with different arguments.")
            }
            Self::UnknownCommand(id) => write!(f, "No command {id}."),
            Self::Drift(id) => write!(f, "The provider ran a different command as {id}."),
            Self::StaleDraft { expected, current } => write!(
                f,
                "The draft is at revision {current}, not the expected {expected}."
            ),
        }
    }
}
impl std::error::Error for Refusal {}

fn live(s: &SetupSession) -> Result<(), Refusal> {
    if s.state.terminal() {
        Err(Refusal::Ended)
    } else {
        Ok(())
    }
}
fn set_run(c: &mut CommandRecord, run: Run, now_ms: u64) {
    if c.run != run {
        c.run = run.clone();
        c.history.push(RunStep { run, at_ms: now_ms });
    }
}

/// Apply one operation at `now_ms`. `Ok(None)` means it is already true.
pub fn apply(s: &SetupSession, op: &Op, now_ms: u64) -> Result<Option<SetupSession>, Refusal> {
    let mut next = s.clone();
    match op {
        Op::OpenSegment { id } => {
            if !valid_id(id) || s.segments.iter().any(|x| &x.id == id) {
                return Err(Refusal::Invalid("The segment identity is invalid."));
            }
            if s.segments.len() >= MAX_SEGMENTS {
                return Err(Refusal::Limit("The session holds too many segments."));
            }
            for seg in next.segments.iter_mut().filter(|x| x.sealed.is_none()) {
                seg.interrupted = true;
            }
            next.segments.push(Segment {
                id: id.clone(),
                sealed: None,
                interrupted: false,
            });
        }
        Op::ContinueEvidence { command, call } => {
            let segment = next
                .segment()
                .map(|x| x.id.clone())
                .ok_or(Refusal::Invalid("No evidence segment is open."))?;
            let c = next.command_mut(command)?;
            if c.evidence.last().is_some_and(|e| e.segment == segment) {
                return Ok(None);
            }
            c.evidence.push(EvidenceCall {
                segment,
                call: call.clone(),
            });
        }
        Op::SealSegment { id, sealed } => {
            let seg = next
                .segments
                .iter_mut()
                .find(|x| &x.id == id)
                .ok_or(Refusal::Invalid("No such segment."))?;
            match &seg.sealed {
                Some(existing) if existing == sealed => return Ok(None),
                Some(_) => return Err(Refusal::Invalid("The segment was sealed differently.")),
                None => seg.sealed = Some(sealed.clone()),
            }
        }
        Op::Dispatched { generation } => {
            live(s)?;
            if s.generation == Some(*generation) {
                return Ok(None);
            }
            if s.generation.is_some_and(|g| *generation <= g) {
                return Err(Refusal::Invalid("Turn generations only increase."));
            }
            next.generation = Some(*generation);
            next.state = next.work_state();
        }
        Op::Paused { question } => {
            live(s)?;
            if !valid_text(question, MAX_TEXT_BYTES) {
                return Err(Refusal::Invalid("The question needs 1 to 4096 bytes."));
            }
            if let Some(c) = s.active_command() {
                return Err(match c.run {
                    Run::Unknown { .. } => Refusal::Unresolved(c.id.clone()),
                    _ => Refusal::Busy(c.id.clone()),
                });
            }
            if let SetupState::AwaitingInput { question: q } = &s.state
                && q == question
            {
                return Ok(None);
            }
            next.state = SetupState::AwaitingInput {
                question: question.clone(),
            };
        }
        Op::Steered { text } => {
            live(s)?;
            if !valid_text(text, MAX_TEXT_BYTES) {
                return Err(Refusal::Invalid("Steering needs 1 to 4096 bytes."));
            }
            if s.steering.len() >= MAX_STEERING {
                return Err(Refusal::Limit("The session holds too much steering."));
            }
            next.steering.push(Steer {
                at_ms: now_ms,
                text: text.clone(),
            });
        }
        Op::RecipeRevised {
            request_id,
            fingerprint,
            revision,
            digest,
        } => {
            if let Some(r) = s.replay(request_id, fingerprint) {
                r?;
                return Ok(None);
            }
            live(s)?;
            if !valid_id(request_id) || s.requests.len() >= MAX_REQUESTS {
                return Err(Refusal::Invalid("The request identity is invalid."));
            }
            if s.recipe_revisions.last().is_some_and(|r| r >= revision) {
                return Err(Refusal::Invalid("Recipe revisions only increase."));
            }
            next.recipe_revisions.push(*revision);
            next.requests.insert(
                request_id.clone(),
                RequestEntry {
                    fingerprint: fingerprint.clone(),
                    result: ToolResult::RecipeRevised {
                        revision: *revision,
                        digest: digest.clone(),
                    },
                },
            );
        }
        Op::RequestCommand {
            request_id,
            fingerprint,
            purpose,
            spec,
        } => {
            if let Some(r) = s.replay(request_id, fingerprint) {
                r?;
                return Ok(None);
            }
            request_command(&mut next, request_id, fingerprint, purpose, spec, now_ms)?;
        }
        Op::ObserveStart { command, outcome } => {
            let c = next.command_mut(command)?;
            let run = match (&c.run, outcome) {
                (Run::Requested | Run::Unknown { .. }, Outcome::Done { value }) => Run::Running {
                    operation: value.clone(),
                },
                (Run::Requested | Run::Unknown { .. }, Outcome::Failed { reason }) => {
                    Run::NotStarted {
                        reason: reason.clone(),
                    }
                }
                (Run::Requested | Run::Unknown { .. }, Outcome::Unknown { reason }) => {
                    Run::Unknown {
                        reason: reason.clone(),
                    }
                }
                _ => return Ok(None),
            };
            set_run(c, run, now_ms);
        }
        Op::ObserveRead {
            command,
            progress,
            digest,
            cursor,
        } => {
            let c = next.command_mut(command)?;
            if let Some(d) = digest
                && d != &c.spec.digest
            {
                return Err(Refusal::Drift(command.clone()));
            }
            if cursor.stdout < c.cursor.stdout || cursor.stderr < c.cursor.stderr {
                return Err(Refusal::Invalid("A command cursor cannot move backward."));
            }
            let was_active = c.run.active();
            c.cursor = *cursor;
            let run = match progress {
                CommandProgress::Absent => match &c.run {
                    // Provably never started: a start with the same
                    // identity is safe.
                    Run::Requested | Run::Unknown { .. } => Run::Requested,
                    Run::Running { .. } => Run::Lost,
                    other => other.clone(),
                },
                CommandProgress::Running => match &c.run {
                    Run::Requested | Run::Unknown { .. } => Run::Running {
                        operation: "reconciled".into(),
                    },
                    other => other.clone(),
                },
                CommandProgress::Exited { code } if was_active => match c.stop {
                    Some(StopReason::Deadline) => Run::TimedOut,
                    Some(StopReason::Owner) => Run::Stopped {
                        reason: "owner".into(),
                    },
                    Some(StopReason::SessionEnded) => Run::Stopped {
                        reason: "session_ended".into(),
                    },
                    None => Run::Exited { code: *code },
                },
                CommandProgress::Lost if was_active => Run::Lost,
                _ => c.run.clone(),
            };
            set_run(c, run, now_ms);
            let settled_install = was_active
                && !c.run.active()
                && matches!(c.purpose, CommandPurpose::Install { .. });
            if settled_install
                && !next.state.terminal()
                && !matches!(next.state, SetupState::AwaitingInput { .. })
            {
                next.state = next.work_state();
            }
        }
        Op::RequestStop { command, reason } => {
            let c = next.command_mut(command)?;
            if !c.run.active() || c.stop.is_some() {
                return Ok(None);
            }
            c.stop = Some(*reason);
        }
        Op::Finish { end } => {
            let state = match end {
                End::Ended => SetupState::Ended,
                End::Failed { reason } => SetupState::Failed {
                    reason: reason.clone(),
                },
                End::Cancelled { reason } => SetupState::Cancelled {
                    reason: reason.clone(),
                },
            };
            if s.state.terminal() {
                return if s.state == state {
                    Ok(None)
                } else {
                    Err(Refusal::Ended)
                };
            }
            for c in next.commands.iter_mut().filter(|c| c.run.active()) {
                let run = match c.run {
                    Run::Requested => Run::NotStarted {
                        reason: "the session ended first".into(),
                    },
                    _ => Run::Unknown {
                        reason: "the session ended before the command settled".into(),
                    },
                };
                set_run(c, run, now_ms);
            }
            next.state = state;
            next.ended_ms = Some(now_ms);
        }
    }
    next.revision += 1;
    next.updated_ms = now_ms;
    Ok(Some(next))
}

fn request_command(
    s: &mut SetupSession,
    request_id: &str,
    fingerprint: &str,
    purpose: &CommandPurpose,
    spec: &CommandSpec,
    now_ms: u64,
) -> Result<(), Refusal> {
    live(s)?;
    if matches!(s.state, SetupState::AwaitingInput { .. }) {
        return Err(Refusal::AwaitingInput);
    }
    if s.generation.is_none() {
        return Err(Refusal::NotProvisioned);
    }
    if now_ms >= s.admission.deadline_ms {
        return Err(Refusal::Deadline);
    }
    if let Some(c) = s.active_command() {
        return Err(match c.run {
            Run::Unknown { .. } => Refusal::Unresolved(c.id.clone()),
            _ => Refusal::Busy(c.id.clone()),
        });
    }
    if !valid_id(request_id) || s.requests.len() >= MAX_REQUESTS {
        return Err(Refusal::Invalid("The request identity is invalid."));
    }
    if s.commands.len() >= MAX_COMMANDS {
        return Err(Refusal::Limit("The session holds too many commands."));
    }
    if spec.id != s.next_command_id() || spec.digest != spec_digest(spec) {
        return Err(Refusal::Invalid("The command identity or digest is wrong."));
    }
    if !valid_text(&spec.command, MAX_COMMAND_BYTES)
        || !valid_relative(&spec.cwd)
        || spec.timeout_seconds == 0
        || spec.timeout_seconds > MAX_DEADLINE_SECONDS
    {
        return Err(Refusal::Invalid("The command, cwd, or timeout is invalid."));
    }
    if let Some(name) = spec
        .credential_names
        .iter()
        .find(|n| !s.admission.credential_names.contains(*n))
    {
        return Err(Refusal::CredentialNotAdmitted(name.clone()));
    }
    if embeds_url_credential(&spec.command) {
        return Err(Refusal::EmbeddedCredential);
    }
    if !spec.env.is_empty() {
        let git = s.admission.git_credential.as_deref();
        let ok =
            git.is_some_and(|g| spec.env == git_auth_env(g) && spec.credential_names.contains(g));
        if !ok {
            return Err(Refusal::Invalid(
                "A command's extra environment is only the session's Git auth.",
            ));
        }
    }
    let segment = s
        .segment()
        .map(|x| x.id.clone())
        .ok_or(Refusal::Invalid("No evidence segment is open."))?;
    if let CommandPurpose::Install {
        recipe_revision,
        recipe_digest,
        attempt,
    } = purpose
    {
        if *recipe_revision == 0
            || !coder_environment::valid_digest(recipe_digest)
            || *attempt as usize != s.installs().count() + 1
        {
            return Err(Refusal::Invalid("The install attempt is invalid."));
        }
        s.state = if s.installs().any(|c| !c.run.succeeded()) {
            SetupState::Repairing
        } else {
            SetupState::Installing
        };
    } else if s.state == SetupState::Provisioning {
        s.state = SetupState::Discovering;
    }
    let deadline_ms = (now_ms + spec.timeout_seconds * 1000).min(s.admission.deadline_ms);
    s.commands.push(CommandRecord {
        id: spec.id.clone(),
        request_id: request_id.into(),
        purpose: purpose.clone(),
        spec: spec.clone(),
        deadline_ms,
        run: Run::Requested,
        cursor: CommandCursor::default(),
        stop: None,
        evidence: vec![EvidenceCall {
            segment,
            call: spec.id.clone(),
        }],
        history: vec![RunStep {
            run: Run::Requested,
            at_ms: now_ms,
        }],
        requested_ms: now_ms,
    });
    s.requests.insert(
        request_id.into(),
        RequestEntry {
            fingerprint: fingerprint.into(),
            result: ToolResult::CommandStarted {
                command: spec.id.clone(),
            },
        },
    );
    Ok(())
}
