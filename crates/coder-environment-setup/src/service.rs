//! The setup owner: admission, the dedicated computer, recipe and command
//! tools, deadlines, cancellation, and evidence.
//!
//! Each tool call holds the session's lease for its whole duration, so two
//! callers cannot race one session. Intent is retained before every
//! provider effect; every observation is retained before the next step.
//! The machine runs through the CMP-01 [`Driver`] (create, restore,
//! per-boot credentials by name, creator-login refusal, checkpoint, stop,
//! delete); commands run through [`Commands`] on that same provider.

use crate::store::{Lease, Store as SessionStore, StoreError};
use crate::transition::{CommandRecord, End, StopReason, spec_digest};
use crate::*;
use coder_environment::evidence::{
    CallIdentity, CallResult, EvidenceError, Recorder, Redactor, StreamName,
};
use coder_environment::store::{Store as EnvStore, StoreError as EnvStoreError};
use coder_environment::{
    Applied as EnvApplied, Command as EnvCommand, Effect, Inputs, Limits, Qualification, Recipe,
    RunLink, Script, Start,
};
use coder_working_computer::driver::{Driver, Settled};
use coder_working_computer::provider::{CommandCursor, CommandProgress, CommandSpec, Commands};
use coder_working_computer::{Bounds, Computer, Spec};
use serde_json::{Value, json};
use std::{
    fmt,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::Mutex,
};

/// Bytes of each stream one provider read takes.
pub const MAX_READ_BYTES: u64 = 128 * 1024;
pub const MAX_READS_PER_POLL: usize = 64;
/// Model-visible bytes of each stream a poll returns; the evidence keeps
/// everything.
pub const EXCERPT_BYTES: u64 = 16 * 1024;

#[derive(Debug)]
pub enum SetupError {
    Refused(Refusal),
    Session(StoreError),
    Environment(EnvStoreError),
    Computer(coder_working_computer::store::StoreError),
    Evidence(EvidenceError),
    Custody(String),
    Blob(&'static str),
    /// The setup computer could not be made ready now.
    NotReady(String),
}
impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(r) => r.fmt(f),
            Self::Session(e) => e.fmt(f),
            Self::Environment(e) => e.fmt(f),
            Self::Computer(e) => e.fmt(f),
            Self::Evidence(e) => e.fmt(f),
            Self::Custody(m) | Self::NotReady(m) => f.write_str(m),
            Self::Blob(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for SetupError {}
impl From<Refusal> for SetupError {
    fn from(r: Refusal) -> Self {
        Self::Refused(r)
    }
}
impl From<StoreError> for SetupError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Refused(r) => Self::Refused(r),
            other => Self::Session(other),
        }
    }
}
impl From<EnvStoreError> for SetupError {
    fn from(e: EnvStoreError) -> Self {
        match e {
            EnvStoreError::Refused(coder_environment::Refusal::StaleDraft {
                expected,
                current,
            }) => Self::Refused(Refusal::StaleDraft { expected, current }),
            other => Self::Environment(other),
        }
    }
}
impl From<coder_working_computer::store::StoreError> for SetupError {
    fn from(e: coder_working_computer::store::StoreError) -> Self {
        Self::Computer(e)
    }
}
impl From<EvidenceError> for SetupError {
    fn from(e: EvidenceError) -> Self {
        Self::Evidence(e)
    }
}
pub type Result<T> = std::result::Result<T, SetupError>;

/// Builds the redactor for a session's named credentials from operator
/// custody (their exact values plus every engine login). Values never
/// enter a session record.
pub type Custody =
    Box<dyn Fn(&BTreeSet<String>) -> std::result::Result<Redactor, String> + Send + Sync>;

/// One recipe revision the agent proposes. The base, runtime, and
/// platform are not editable here: they are the session's pins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeEdit {
    pub install_script: String,
    pub install_cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Start>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Inputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_names: Option<BTreeSet<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification: Option<Qualification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,
    /// What the clean build may capture (ENV-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<coder_environment::capture::Capture>,
}

/// A discovery command the agent asks to run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandInput {
    pub command: String,
    pub cwd: String,
    /// Credentials this command may see; nothing else is in its env.
    #[serde(default)]
    pub credential_names: BTreeSet<String>,
    /// Use the session's ephemeral Git auth (its credential must be named).
    #[serde(default)]
    pub git_auth: bool,
    pub timeout_seconds: u64,
}

/// What `environment.inspect` returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Inspection {
    pub session: SetupSession,
    pub draft_revision: u64,
    pub recipe: Recipe,
    pub install_script: Option<String>,
}

/// What a command poll returns: the retained record plus the newly
/// retained (already redacted) output, bounded for the model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommandView {
    pub command: CommandRecord,
    pub stdout: String,
    pub stderr: String,
    /// A provider read that could not be completed; the command keeps its
    /// last known state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertain: Option<String>,
}

pub struct Setup<P> {
    pub sessions: SessionStore,
    pub environments: EnvStore,
    pub computers: Driver<P>,
    root: PathBuf,
    custody: Custody,
    live: Mutex<BTreeMap<String, Recorder>>,
    /// Sessions this owner process has visited: the first visit after a
    /// start counts as a heartbeat, so a restart is not a silent turn.
    visited: Mutex<BTreeSet<String>>,
}

/// What a setup whose turn went silent says while it waits (#11059).
pub const STALLED: &str = "The setup stopped responding, so its computer was stopped. \
Its files are kept. Send a message to pick up where it left off.";

fn fingerprint(value: &Value) -> String {
    digest(&serde_json::to_vec(value).expect("request encodes"))
}

impl<P: Commands> Setup<P> {
    /// `root` holds evidence (`evidence/<session>/<segment>`) and script
    /// blobs (`blobs/<digest>`).
    pub fn new(
        root: impl Into<PathBuf>,
        sessions: SessionStore,
        environments: EnvStore,
        computers: Driver<P>,
        custody: Custody,
    ) -> Self {
        Self {
            sessions,
            environments,
            computers,
            root: root.into(),
            custody,
            live: Mutex::new(BTreeMap::new()),
            visited: Mutex::new(BTreeSet::new()),
        }
    }

    /// Forget live recorders, as an owner restart would.
    #[cfg(test)]
    pub(crate) fn restart(&self) {
        self.live.lock().expect("recorders").clear();
    }

    pub fn evidence_dir(&self, session: &str, segment: &str) -> PathBuf {
        self.root.join("evidence").join(session).join(segment)
    }

    /// Admit and start a setup session on its own computer. Repeating the
    /// same request continues the same session.
    pub async fn open(
        &self,
        request: &SetupRequest,
        profile: &coder_cloud::operator::Profile,
        now_ms: u64,
    ) -> Result<SetupSession> {
        let lease = self.sessions.lease(&request.session)?;
        if lease.exists() {
            let s = lease.read()?;
            let same = s.environment == request.environment
                && s.owner == request.owner
                && s.objective == request.objective
                && s.admission.profile == request.profile
                && s.admission.credential_names == request.credential_names
                && s.admission.git_credential == request.git_credential;
            if !same {
                return Err(Refusal::RequestConflict(request.session.clone()).into());
            }
            return self.wake(&lease, now_ms).await;
        }
        let env = self.environments.read(&request.environment)?;
        let admission = admit(request, profile, &env, now_ms)?;
        let computer_id = format!("setup-{}", request.session);
        let window = admission.deadline_ms - now_ms;
        let spec = Spec {
            id: computer_id.clone(),
            owner: request.owner.clone(),
            chat: request.session.clone(),
            project: env.project.clone(),
            source: env.source.clone(),
            base: None,
            size: admission.size.clone(),
            credential_names: admission.credential_names.clone(),
            services: vec![],
            bounds: Bounds {
                idle_ms: window,
                observed_extension_ms: 0,
                absolute_ms: window,
            },
        };
        let mut computer = Computer::for_setup(spec, &env.id, now_ms)
            .map_err(|m| SetupError::Refused(Refusal::Invalid(m)))?;
        computer.provider = self.computers.provider.kind();
        match self.computers.store.read(&computer_id) {
            Ok(existing)
                if existing.purpose == computer.purpose && existing.chat == computer.chat => {}
            Ok(_) => {
                return Err(Refusal::Admission(
                    "That computer is not this session's dedicated setup computer.",
                )
                .into());
            }
            Err(coder_working_computer::store::StoreError::NotFound) => {
                self.computers.store.create(&computer)?
            }
            Err(e) => return Err(e.into()),
        }
        lease.create(&SetupSession::new(request, &computer_id, admission, now_ms))?;
        self.wake(&lease, now_ms).await
    }

    /// Make the setup computer awake for a turn (fresh or restored, with
    /// its named credentials re-applied).
    async fn wake(&self, lease: &Lease, now_ms: u64) -> Result<SetupSession> {
        let s = lease.read()?;
        if s.state.terminal() {
            return Ok(s);
        }
        let s = self.ensure_segment(lease, s, now_ms)?;
        match self.computers.prompt(&s.computer, now_ms).await? {
            Settled::Dispatch { generation, .. } => {
                Ok(lease.apply(&Op::Dispatched { generation }, now_ms)?)
            }
            Settled::Waiting(_, _) if s.generation.is_some() => Ok(s),
            other => Err(SetupError::NotReady(describe(&other))),
        }
    }

    /// Open a new evidence segment when this owner holds no live recorder
    /// for the session (first use, or after a restart).
    fn ensure_segment(&self, lease: &Lease, s: SetupSession, now_ms: u64) -> Result<SetupSession> {
        let mut live = self.live.lock().expect("recorders");
        if live.contains_key(&s.id) && s.segment().is_some() {
            return Ok(s);
        }
        let id = format!("seg-{}", s.segments.len() + 1);
        let redactor =
            (self.custody)(&s.admission.credential_names).map_err(SetupError::Custody)?;
        let mut recorder = Recorder::create(
            self.evidence_dir(&s.id, &id),
            &id,
            Some(run_link(&s)),
            redactor,
            s.admission.evidence_budget,
        )?;
        let mut s = lease.apply(&Op::OpenSegment { id: id.clone() }, now_ms)?;
        // A command still active continues in a new call that says where
        // it resumes; the interrupted segment discloses the gap.
        if let Some(c) = s.active_command().cloned() {
            let call = format!("{}-{id}", c.id);
            recorder.start_call(
                identity(&s, &call, tool_name(&c.purpose), Some(&c.request_id)),
                &json!({"continues": c.id, "cursor": c.cursor}),
                now_ms,
            )?;
            s = lease.apply(
                &Op::ContinueEvidence {
                    command: c.id.clone(),
                    call,
                },
                now_ms,
            )?;
        }
        live.insert(s.id.clone(), recorder);
        Ok(s)
    }

    fn with_recorder<T>(
        &self,
        session: &str,
        f: impl FnOnce(&mut Recorder) -> std::result::Result<T, EvidenceError>,
    ) -> Result<T> {
        let mut live = self.live.lock().expect("recorders");
        let recorder = live
            .get_mut(session)
            .ok_or(SetupError::NotReady("No evidence segment is open.".into()))?;
        Ok(f(recorder)?)
    }

    /// Record one non-command tool call whole: arguments, result as
    /// stdout (or refusal as stderr), and its outcome.
    fn record_tool(
        &self,
        s: &SetupSession,
        tool: &str,
        request: Option<&str>,
        arguments: &Value,
        result: &std::result::Result<Value, String>,
        now_ms: u64,
    ) -> Result<()> {
        let call = format!("tool-{}", s.revision);
        self.with_recorder(&s.id, |r| {
            r.start_call(identity(s, &call, tool, request), arguments, now_ms)?;
            let (stream, bytes, ok) = match result {
                Ok(v) => (
                    StreamName::Stdout,
                    serde_json::to_vec(v).expect("encodes"),
                    true,
                ),
                Err(m) => (StreamName::Stderr, m.clone().into_bytes(), false),
            };
            r.output(&call, stream, &bytes)?;
            r.close_stream(&call, StreamName::Stdout)?;
            r.close_stream(&call, StreamName::Stderr)?;
            r.result(
                &call,
                CallResult::Exited {
                    code: Some(if ok { 0 } else { 1 }),
                    success: ok,
                },
                now_ms,
            )?;
            Ok(())
        })
    }

    fn live_session(&self, lease: &Lease, now_ms: u64) -> Result<SetupSession> {
        let s = lease.read()?;
        if s.state.terminal() {
            return Err(Refusal::Ended.into());
        }
        if now_ms >= s.admission.deadline_ms {
            return Err(Refusal::Deadline.into());
        }
        self.ensure_segment(lease, s, now_ms)
    }

    /// `environment.inspect`: the session, the current draft, and its
    /// install script.
    pub fn inspect(&self, id: &str) -> Result<Inspection> {
        let session = self.sessions.read(id)?;
        let env = self.environments.read(&session.environment)?;
        let draft = env.draft();
        Ok(Inspection {
            draft_revision: draft.revision,
            recipe: draft.recipe.clone(),
            install_script: self.read_blob(&draft.recipe.install.digest).ok(),
            session,
        })
    }

    /// `environment.recipe.update`: revise the draft through the
    /// environment owner's revision fence.
    pub async fn update_recipe(
        &self,
        id: &str,
        request_id: &str,
        expected_draft_revision: u64,
        edit: &RecipeEdit,
        now_ms: u64,
    ) -> Result<ToolResult> {
        let lease = self.sessions.lease(id)?;
        let fp = fingerprint(&json!({
            "tool": "recipe.update",
            "expected": expected_draft_revision,
            "edit": edit,
        }));
        if let Some(r) = lease.read()?.replay(request_id, &fp) {
            return Ok(r?);
        }
        let s = self.live_session(&lease, now_ms)?;
        let arguments = json!({"expected_draft_revision": expected_draft_revision, "edit": edit});
        let outcome = self.revise(&s, expected_draft_revision, edit, now_ms);
        let recorded = outcome
            .as_ref()
            .map(|(revision, digest)| json!({"revision": revision, "digest": digest}))
            .map_err(ToString::to_string);
        self.record_tool(
            &s,
            "environment.recipe.update",
            Some(request_id),
            &arguments,
            &recorded,
            now_ms,
        )?;
        let (revision, digest) = outcome?;
        lease.apply(
            &Op::RecipeRevised {
                request_id: request_id.into(),
                fingerprint: fp,
                revision,
                digest: digest.clone(),
            },
            now_ms,
        )?;
        Ok(ToolResult::RecipeRevised { revision, digest })
    }

    fn revise(
        &self,
        s: &SetupSession,
        expected: u64,
        edit: &RecipeEdit,
        now_ms: u64,
    ) -> Result<(u64, String)> {
        let env = self.environments.read(&s.environment)?;
        let mut recipe = env.draft().recipe.clone();
        if !s.admission.pins(&recipe) {
            return Err(Refusal::PinChanged.into());
        }
        if !valid_text(&edit.install_script, MAX_COMMAND_BYTES) {
            return Err(Refusal::Invalid("The install script needs 1 byte to 64 KiB.").into());
        }
        if let Some(names) = &edit.credential_names
            && let Some(name) = names
                .iter()
                .find(|n| !s.admission.credential_names.contains(*n))
        {
            return Err(Refusal::CredentialNotAdmitted(name.clone()).into());
        }
        if embeds_url_credential(&edit.install_script) {
            return Err(Refusal::EmbeddedCredential.into());
        }
        let script_digest = self.write_blob(edit.install_script.as_bytes())?;
        recipe.install = Script {
            cwd: edit.install_cwd.clone(),
            digest: script_digest,
        };
        if let Some(v) = &edit.start {
            recipe.start = v.clone();
        }
        if let Some(v) = &edit.inputs {
            recipe.inputs = v.clone();
        }
        if let Some(v) = &edit.credential_names {
            recipe.credential_names = v.clone();
        }
        if let Some(v) = &edit.qualification {
            recipe.qualification = v.clone();
        }
        if let Some(v) = &edit.limits {
            recipe.limits = v.clone();
        }
        if let Some(v) = &edit.capture {
            recipe.capture = v.clone();
        }
        let command = EnvCommand::UpdateRecipe {
            expected_draft_revision: expected,
            recipe,
        };
        let effect = match self.environments.apply(&env.id, &command, now_ms)? {
            EnvApplied::Changed(_, effect) | EnvApplied::Replayed(effect) => effect,
        };
        match effect {
            Effect::RecipeRevised { revision, digest } => Ok((revision, digest)),
            _ => Err(SetupError::NotReady(
                "Unexpected environment effect.".into(),
            )),
        }
    }

    fn write_blob(&self, bytes: &[u8]) -> Result<String> {
        let d = digest(bytes);
        let dir = self.root.join("blobs");
        fs::create_dir_all(&dir).map_err(|_| SetupError::Blob("Cannot create the blob store."))?;
        let path = dir.join(&d);
        if self.read_blob(&d).is_ok() {
            return Ok(d);
        }
        let temp = dir.join(format!("{d}.writing"));
        let mut file =
            File::create(&temp).map_err(|_| SetupError::Blob("Cannot write a script blob."))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::rename(&temp, &path))
            .map_err(|_| SetupError::Blob("Cannot retain a script blob."))?;
        Ok(d)
    }

    fn read_blob(&self, d: &str) -> Result<String> {
        if !coder_environment::valid_digest(d) {
            return Err(SetupError::Blob("Invalid blob digest."));
        }
        let bytes = fs::read(self.root.join("blobs").join(d))
            .map_err(|_| SetupError::Blob("The script blob is missing."))?;
        if digest(&bytes) != d {
            return Err(SetupError::Blob(
                "The script blob does not match its digest.",
            ));
        }
        String::from_utf8(bytes).map_err(|_| SetupError::Blob("The script is not UTF-8."))
    }

    /// `environment.command.start` for discovery.
    pub async fn run_command(
        &self,
        id: &str,
        request_id: &str,
        input: &CommandInput,
        now_ms: u64,
    ) -> Result<ToolResult> {
        let lease = self.sessions.lease(id)?;
        let fp = fingerprint(&json!({"tool": "command", "input": input}));
        if let Some(r) = lease.read()?.replay(request_id, &fp) {
            return Ok(r?);
        }
        let s = self.live_session(&lease, now_ms)?;
        let env = self.git_env(&s, input.git_auth, &input.credential_names)?;
        self.start(
            &lease,
            s,
            request_id,
            fp,
            CommandPurpose::Discover,
            CommandSpec {
                id: String::new(),
                command: input.command.clone(),
                cwd: input.cwd.clone(),
                credential_names: input.credential_names.clone(),
                env,
                timeout_seconds: input.timeout_seconds,
                digest: String::new(),
            },
            now_ms,
        )
        .await
    }

    /// `environment.install.run`: run the exact install script of the
    /// expected draft revision, with the credentials that recipe names.
    pub async fn run_install(
        &self,
        id: &str,
        request_id: &str,
        expected_draft_revision: u64,
        timeout_seconds: u64,
        now_ms: u64,
    ) -> Result<ToolResult> {
        let lease = self.sessions.lease(id)?;
        let fp = fingerprint(&json!({
            "tool": "install",
            "expected": expected_draft_revision,
            "timeout_seconds": timeout_seconds,
        }));
        if let Some(r) = lease.read()?.replay(request_id, &fp) {
            return Ok(r?);
        }
        let s = self.live_session(&lease, now_ms)?;
        let env = self.environments.read(&s.environment)?;
        let draft = env.draft();
        if draft.revision != expected_draft_revision {
            return Err(Refusal::StaleDraft {
                expected: expected_draft_revision,
                current: draft.revision,
            }
            .into());
        }
        if !s.admission.pins(&draft.recipe) {
            return Err(Refusal::PinChanged.into());
        }
        if !s.source_ready() {
            return Err(Refusal::SourceNotReady.into());
        }
        let script = self.read_blob(&draft.recipe.install.digest)?;
        let names = draft.recipe.credential_names.clone();
        let git = s
            .admission
            .git_credential
            .as_ref()
            .is_some_and(|g| names.contains(g));
        let env_vars = self.git_env(&s, git, &names)?;
        let purpose = CommandPurpose::Install {
            recipe_revision: draft.revision,
            recipe_digest: draft.digest.clone(),
            attempt: s.installs().count() as u32 + 1,
        };
        self.start(
            &lease,
            s,
            request_id,
            fp,
            purpose,
            CommandSpec {
                id: String::new(),
                command: script,
                cwd: draft.recipe.install.cwd.clone(),
                credential_names: names,
                env: env_vars,
                timeout_seconds,
                digest: String::new(),
            },
            now_ms,
        )
        .await
    }

    /// `environment.source.materialize`: fetch the session's exact pinned
    /// commit into the setup computer's checkout and prove `HEAD` and a
    /// clean tree ([`crate::source`]). The session's Git credential, when
    /// admitted, is used only as ephemeral auth for the fetch.
    pub async fn materialize_source(
        &self,
        id: &str,
        request_id: &str,
        timeout_seconds: u64,
        now_ms: u64,
    ) -> Result<ToolResult> {
        let lease = self.sessions.lease(id)?;
        let fp = fingerprint(&json!({
            "tool": "source",
            "timeout_seconds": timeout_seconds,
        }));
        if let Some(r) = lease.read()?.replay(request_id, &fp) {
            return Ok(r?);
        }
        let s = self.live_session(&lease, now_ms)?;
        let pin = s.admission.source.clone();
        let (command, credential_names, env) = crate::source::command(
            &pin,
            crate::source::Mode::Materialize,
            s.admission.git_credential.as_deref(),
        )
        .map_err(|m| SetupError::Refused(Refusal::Invalid(m)))?;
        self.start(
            &lease,
            s,
            request_id,
            fp,
            CommandPurpose::Source {
                revision: pin.revision.clone(),
            },
            CommandSpec {
                id: String::new(),
                command,
                cwd: ".".into(),
                credential_names,
                env,
                timeout_seconds,
                digest: String::new(),
            },
            now_ms,
        )
        .await
    }

    fn git_env(
        &self,
        s: &SetupSession,
        git_auth: bool,
        names: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, String>> {
        if !git_auth {
            return Ok(BTreeMap::new());
        }
        match &s.admission.git_credential {
            Some(g) if names.contains(g) => Ok(git_auth_env(g)),
            Some(g) => Err(Refusal::CredentialNotAdmitted(format!(
                "{g} (Git auth needs the credential named on the command)"
            ))
            .into()),
            None => {
                Err(Refusal::Admission("No Git credential was admitted for this setup.").into())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn start(
        &self,
        lease: &Lease,
        s: SetupSession,
        request_id: &str,
        fp: String,
        purpose: CommandPurpose,
        mut spec: CommandSpec,
        now_ms: u64,
    ) -> Result<ToolResult> {
        spec.id = s.next_command_id();
        spec.digest = spec_digest(&spec);
        let s = lease.apply(
            &Op::RequestCommand {
                request_id: request_id.into(),
                fingerprint: fp,
                purpose: purpose.clone(),
                spec: spec.clone(),
            },
            now_ms,
        )?;
        self.with_recorder(&s.id, |r| {
            r.start_call(
                identity(&s, &spec.id, tool_name(&purpose), Some(request_id)),
                &json!({
                    "purpose": purpose,
                    "command": spec.command,
                    "cwd": spec.cwd,
                    "credential_names": spec.credential_names,
                    "env": spec.env,
                    "timeout_seconds": spec.timeout_seconds,
                    "digest": spec.digest,
                }),
                now_ms,
            )
        })?;
        self.issue_start(lease, &spec.id, now_ms).await?;
        Ok(ToolResult::CommandStarted { command: spec.id })
    }

    /// Call the provider for a command whose intent is retained. Safe to
    /// repeat: the provider runs one identity at most once.
    async fn issue_start(&self, lease: &Lease, command: &str, now_ms: u64) -> Result<SetupSession> {
        let s = lease.read()?;
        let c = s
            .command(command)
            .ok_or_else(|| Refusal::UnknownCommand(command.into()))?;
        let computer = self.computers.store.read(&s.computer)?;
        let resource = computer.resource().ok_or(SetupError::NotReady(
            "The setup computer has no machine.".into(),
        ))?;
        let outcome = self
            .computers
            .provider
            .start_command(&computer, resource, &c.spec)
            .await;
        Ok(lease.apply(
            &Op::ObserveStart {
                command: command.into(),
                outcome,
            },
            now_ms,
        )?)
    }

    /// `environment.command.status/output`: read new output into the
    /// evidence, settle the command if it ended, enforce its deadline, and
    /// reconcile an unknown start by identity.
    pub async fn poll(&self, id: &str, command: &str, now_ms: u64) -> Result<CommandView> {
        let lease = self.sessions.lease(id)?;
        let s = lease.read()?;
        let s = if s.state.terminal() {
            s
        } else {
            self.ensure_segment(&lease, s, now_ms)?
        };
        let c = s
            .command(command)
            .ok_or_else(|| Refusal::UnknownCommand(command.into()))?;
        if c.run.active() && now_ms >= c.deadline_ms && !s.state.terminal() {
            lease.apply(
                &Op::RequestStop {
                    command: command.into(),
                    reason: StopReason::Deadline,
                },
                now_ms,
            )?;
            self.signal(&lease, command).await?;
        }
        self.drive(&lease, command, now_ms).await
    }

    /// `environment.command.reconcile`: the same as a poll; named for the
    /// agent's intent after an unknown outcome.
    pub async fn reconcile(&self, id: &str, command: &str, now_ms: u64) -> Result<CommandView> {
        self.poll(id, command, now_ms).await
    }

    /// `environment.command.stop`.
    pub async fn stop(&self, id: &str, command: &str, now_ms: u64) -> Result<CommandView> {
        let lease = self.sessions.lease(id)?;
        let s = lease.read()?;
        if !s.state.terminal() {
            self.ensure_segment(&lease, s, now_ms)?;
        }
        lease.apply(
            &Op::RequestStop {
                command: command.into(),
                reason: StopReason::Owner,
            },
            now_ms,
        )?;
        self.signal(&lease, command).await?;
        self.drive(&lease, command, now_ms).await
    }

    async fn signal(&self, lease: &Lease, command: &str) -> Result<Option<String>> {
        let s = lease.read()?;
        let Some(c) = s.command(command) else {
            return Ok(None);
        };
        if !matches!(c.run, Run::Running { .. } | Run::Unknown { .. }) {
            return Ok(None);
        }
        let computer = self.computers.store.read(&s.computer)?;
        let Some(resource) = computer.resource() else {
            return Ok(None);
        };
        Ok(
            match self
                .computers
                .provider
                .stop_command(&computer, resource, command)
                .await
            {
                coder_working_computer::provider::Outcome::Done { .. } => None,
                coder_working_computer::provider::Outcome::Failed { reason }
                | coder_working_computer::provider::Outcome::Unknown { reason } => Some(reason),
            },
        )
    }

    async fn drive(&self, lease: &Lease, command: &str, now_ms: u64) -> Result<CommandView> {
        use coder_working_computer::provider::Outcome;
        let mut uncertain = None;
        let before = lease.read()?;
        let first = before
            .command(command)
            .ok_or_else(|| Refusal::UnknownCommand(command.into()))?
            .clone();
        let call = first.evidence.last().map(|e| e.call.clone());
        let lengths = |this: &Self, sid: &str| -> (u64, u64) {
            let Some(call) = &call else { return (0, 0) };
            this.with_recorder(sid, |r| {
                Ok(r.call(call)
                    .map_or((0, 0), |c| (c.stdout.length, c.stderr.length)))
            })
            .unwrap_or((0, 0))
        };
        let start_lengths = lengths(self, &before.id);
        for _ in 0..MAX_READS_PER_POLL {
            let s = lease.read()?;
            let c = s.command(command).expect("command exists").clone();
            if c.run == Run::Requested && !s.state.terminal() && c.stop.is_none() {
                self.issue_start(lease, command, now_ms).await?;
                continue;
            }
            if !matches!(c.run, Run::Running { .. } | Run::Unknown { .. }) {
                break;
            }
            let computer = self.computers.store.read(&s.computer)?;
            let Some(resource) = computer.resource() else {
                uncertain = Some("The setup computer has no machine.".into());
                break;
            };
            let read = match self
                .computers
                .provider
                .read_command(&computer, resource, command, c.cursor, MAX_READ_BYTES)
                .await
            {
                Outcome::Done { value } => value,
                Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                    uncertain = Some(reason);
                    break;
                }
            };
            if let Some(d) = &read.digest
                && d != &c.spec.digest
            {
                return Err(Refusal::Drift(command.into()).into());
            }
            let full = read.stdout.len() as u64 >= MAX_READ_BYTES
                || read.stderr.len() as u64 >= MAX_READ_BYTES;
            // An exit is final only once the remaining output is read.
            let progress = match read.progress {
                CommandProgress::Exited { .. } if full => CommandProgress::Running,
                other => other,
            };
            let call = c.evidence.last().expect("command has a call").call.clone();
            let live = !s.state.terminal();
            if live {
                let stop = c.stop;
                let progress = progress.clone();
                self.with_recorder(&s.id, |r| {
                    r.output(&call, StreamName::Stdout, &read.stdout)?;
                    r.output(&call, StreamName::Stderr, &read.stderr)?;
                    match progress {
                        CommandProgress::Exited { code } => {
                            r.close_stream(&call, StreamName::Stdout)?;
                            r.close_stream(&call, StreamName::Stderr)?;
                            let result = if stop == Some(StopReason::Deadline) {
                                CallResult::TimedOut { code: Some(code) }
                            } else {
                                CallResult::Exited {
                                    code: Some(code),
                                    success: code == 0,
                                }
                            };
                            r.result(&call, result, now_ms)?;
                        }
                        CommandProgress::Lost => {
                            r.result(
                                &call,
                                CallResult::EngineError {
                                    message: "The process ended without a recorded exit.".into(),
                                },
                                now_ms,
                            )?;
                        }
                        CommandProgress::Absent | CommandProgress::Running => {}
                    }
                    Ok(())
                })?;
            }
            let cursor = CommandCursor {
                stdout: c.cursor.stdout + read.stdout.len() as u64,
                stderr: c.cursor.stderr + read.stderr.len() as u64,
            };
            lease.apply(
                &Op::ObserveRead {
                    command: command.into(),
                    progress: progress.clone(),
                    digest: read.digest.clone(),
                    cursor,
                },
                now_ms,
            )?;
            if matches!(progress, CommandProgress::Absent) {
                continue;
            }
            if !full {
                break;
            }
        }
        let s = lease.read()?;
        let record = s.command(command).expect("command exists").clone();
        let (stdout, stderr) = match &call {
            Some(call) if self.live.lock().expect("recorders").contains_key(&s.id) => {
                let end = lengths(self, &s.id);
                let dir = self.live.lock().expect("recorders")[&s.id].dir().to_owned();
                (
                    excerpt(&dir, call, "stdout", start_lengths.0, end.0),
                    excerpt(&dir, call, "stderr", start_lengths.1, end.1),
                )
            }
            _ => (String::new(), String::new()),
        };
        Ok(CommandView {
            command: record,
            stdout,
            stderr,
            uncertain,
        })
    }

    /// `environment.steer`: retain user steering; when the setup awaits
    /// input, wake the computer for the next turn.
    pub async fn steer(&self, id: &str, text: &str, now_ms: u64) -> Result<SetupSession> {
        let s = self.retain_steering(id, text, now_ms)?;
        if matches!(s.state, SetupState::AwaitingInput { .. }) {
            return self.resume(id, now_ms).await;
        }
        Ok(s)
    }

    /// The session store this owner retains sessions in.
    pub fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    /// Wake a session that awaits input and holds steering for its next
    /// turn; any other session is returned unchanged.
    pub async fn resume(&self, id: &str, now_ms: u64) -> Result<SetupSession> {
        let lease = self.sessions.lease(id)?;
        let s = self.live_session(&lease, now_ms)?;
        if matches!(s.state, SetupState::AwaitingInput { .. }) {
            return self.wake(&lease, now_ms).await;
        }
        Ok(s)
    }

    /// The retention half of `environment.steer`, with no provider call:
    /// the steering and its evidence are retained before this returns.
    pub fn retain_steering(&self, id: &str, text: &str, now_ms: u64) -> Result<SetupSession> {
        let lease = self.sessions.lease(id)?;
        let s = self.live_session(&lease, now_ms)?;
        let applied = lease.apply(&Op::Steered { text: text.into() }, now_ms);
        let recorded = applied
            .as_ref()
            .map(|_| json!({"steering": s.steering.len() + 1}))
            .map_err(ToString::to_string);
        self.record_tool(
            &s,
            "environment.steer",
            None,
            &json!({"text": text}),
            &recorded,
            now_ms,
        )?;
        Ok(applied?)
    }

    /// `environment.await_input`: end this turn for the user. The computer
    /// checkpoints (Boat stops it); the next steer restores it.
    pub async fn pause(&self, id: &str, question: &str, now_ms: u64) -> Result<SetupSession> {
        let lease = self.sessions.lease(id)?;
        let s = self.live_session(&lease, now_ms)?;
        let applied = lease.apply(
            &Op::Paused {
                question: question.into(),
            },
            now_ms,
        );
        let recorded = applied
            .as_ref()
            .map(|_| json!({"awaiting_input": true}))
            .map_err(ToString::to_string);
        self.record_tool(
            &s,
            "environment.await_input",
            None,
            &json!({"question": question}),
            &recorded,
            now_ms,
        )?;
        let s = applied?;
        if let Some(generation) = s.generation {
            match self
                .computers
                .turn_finished(&s.computer, generation, now_ms)
                .await?
            {
                Settled::Idle(_) | Settled::Waiting(_, _) => {}
                other => return Err(SetupError::NotReady(describe(&other))),
            }
        }
        Ok(s)
    }

    /// The turn's owner (the setup agent) is alive. Nothing to do when no
    /// turn runs; a busy computer record is skipped until the next beat.
    pub async fn heartbeat(&self, id: &str, now_ms: u64) -> Result<()> {
        let s = self.sessions.read(id)?;
        if s.state.terminal() {
            return Ok(());
        }
        let Some(generation) = s.generation else {
            return Ok(());
        };
        match self
            .computers
            .heartbeat(&s.computer, generation, now_ms)
            .await
        {
            Ok(())
            | Err(coder_working_computer::store::StoreError::Refused(_))
            | Err(coder_working_computer::store::StoreError::Busy) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// A turn whose owner went silent: stop its command, stop the computer
    /// (its files stay with it), and wait for the person with a plain
    /// message. Nothing is announced as done.
    async fn stalled(&self, id: &str, now_ms: u64) -> Result<SetupSession> {
        let s = self.sessions.read(id)?;
        if let Some(c) = s.active_command() {
            let command = c.id.clone();
            let _ = self.stop(id, &command, now_ms).await;
        }
        self.computers.tick(&s.computer, false, now_ms).await?;
        let lease = self.sessions.lease(id)?;
        Ok(lease.apply(
            &Op::Paused {
                question: STALLED.into(),
            },
            now_ms,
        )?)
    }

    /// A timer visit: past the session deadline the setup is cancelled;
    /// a turn whose owner went silent ends ([`STALLED`]); a command past
    /// its own deadline is stopped.
    pub async fn tick(&self, id: &str, now_ms: u64) -> Result<SetupSession> {
        let s = self.sessions.read(id)?;
        if s.state.terminal() {
            return Ok(s);
        }
        if now_ms >= s.admission.deadline_ms {
            return self
                .finish(
                    id,
                    End::Cancelled {
                        reason: "The setup deadline passed.".into(),
                    },
                    StopReason::Deadline,
                    now_ms,
                )
                .await;
        }
        let first = self.visited.lock().expect("visited").insert(id.to_owned());
        if first {
            self.heartbeat(id, now_ms).await?;
        } else if self.computers.store.read(&s.computer)?.turn_silent(now_ms) {
            return self.stalled(id, now_ms).await;
        }
        if let Some(c) = s.active_command()
            && now_ms >= c.deadline_ms
        {
            let command = c.id.clone();
            self.poll(id, &command, now_ms).await?;
        }
        Ok(self.sessions.read(id)?)
    }

    /// `environment.setup.cancel`.
    pub async fn cancel(&self, id: &str, reason: &str, now_ms: u64) -> Result<SetupSession> {
        self.finish(
            id,
            End::Cancelled {
                reason: reason.into(),
            },
            StopReason::SessionEnded,
            now_ms,
        )
        .await
    }

    /// `environment.setup.end`: the setup is done. Ending does not build,
    /// verify, or save anything.
    pub async fn end(&self, id: &str, now_ms: u64) -> Result<SetupSession> {
        self.finish(id, End::Ended, StopReason::SessionEnded, now_ms)
            .await
    }

    async fn finish(
        &self,
        id: &str,
        end: End,
        stop: StopReason,
        now_ms: u64,
    ) -> Result<SetupSession> {
        let lease = self.sessions.lease(id)?;
        let s = lease.read()?;
        if s.state.terminal() {
            return Ok(s);
        }
        let s = self.ensure_segment(&lease, s, now_ms)?;
        if let Some(c) = s.active_command().cloned() {
            lease.apply(
                &Op::RequestStop {
                    command: c.id.clone(),
                    reason: stop,
                },
                now_ms,
            )?;
            self.signal(&lease, &c.id).await?;
            self.drive(&lease, &c.id, now_ms).await?;
        }
        let s = lease.read()?;
        let recorder = self.live.lock().expect("recorders").remove(&s.id);
        if let (Some(mut recorder), Some(segment)) = (recorder, s.segment().cloned()) {
            let sealed = recorder.finish(now_ms)?;
            lease.apply(
                &Op::SealSegment {
                    id: segment.id,
                    sealed,
                },
                now_ms,
            )?;
        }
        let s = lease.apply(&Op::Finish { end }, now_ms)?;
        // Cleanup is a separate fact on the computer record; an unknown
        // deletion stays visible there and [`Setup::cleanup`] retries it.
        let _ = self.computers.delete(&s.computer, now_ms).await?;
        Ok(s)
    }

    /// Retry the setup computer's cleanup after the session ended.
    pub async fn cleanup(&self, id: &str, now_ms: u64) -> Result<Computer> {
        let s = self.sessions.read(id)?;
        if !s.state.terminal() {
            return Err(Refusal::Invalid("End or cancel the setup before cleanup.").into());
        }
        Ok(self
            .computers
            .delete(&s.computer, now_ms)
            .await?
            .computer()
            .clone())
    }
}

fn run_link(s: &SetupSession) -> RunLink {
    RunLink {
        cloud_job: s.computer.clone(),
        task: Some(s.id.clone()),
    }
}
fn identity(s: &SetupSession, call: &str, tool: &str, request: Option<&str>) -> CallIdentity {
    CallIdentity {
        id: call.into(),
        parent: None,
        run: run_link(s),
        tool: tool.into(),
        request: request.map(Into::into),
        operation: None,
    }
}
fn tool_name(purpose: &CommandPurpose) -> &'static str {
    match purpose {
        CommandPurpose::Discover => "environment.command",
        CommandPurpose::Install { .. } => "environment.install",
        CommandPurpose::Audit => "environment.audit",
        CommandPurpose::Source { .. } => "environment.source.materialize",
    }
}
fn describe(settled: &Settled) -> String {
    let c = settled.computer();
    match settled {
        Settled::Dispatch { generation, .. } => format!("Dispatched turn {generation}."),
        Settled::Waiting(..) => "The setup computer is busy with a turn.".into(),
        Settled::Idle(_) => format!("The setup computer is idle ({:?}).", c.phase),
        Settled::Refused(m, _) => format!("The setup computer refused: {m}"),
        Settled::Stuck(d, _) => format!("The setup computer is stuck at {d:?} ({:?}).", c.phase),
        Settled::BootFailed(_) => "The setup computer failed to boot.".into(),
    }
}
/// Retained (already redacted) stream bytes `[from, to)`, at most
/// [`EXCERPT_BYTES`].
fn excerpt(dir: &std::path::Path, call: &str, stream: &str, from: u64, to: u64) -> String {
    if to <= from {
        return String::new();
    }
    let mut bytes = Vec::new();
    let read =
        File::open(dir.join("streams").join(format!("{call}.{stream}"))).and_then(|mut f| {
            f.seek(SeekFrom::Start(from))?;
            f.take((to - from).min(EXCERPT_BYTES))
                .read_to_end(&mut bytes)
        });
    match read {
        Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}
