//! The setup agent: a model turn loop that drives one environment's setup
//! session through the setup owner's tools, then the clean build and the
//! fresh-machine verification, until a candidate is ready to save.
//!
//! The setup owner ([`coder_environment_setup::service::Setup`]) is a tool
//! surface; this module is who plays the agent. Each turn sends the
//! conversation to the model ([`codex_transport::Transport`], normally the
//! operator's Codex login) with six function tools:
//!
//! | Tool | Setup owner call |
//! | --- | --- |
//! | `run_command` | `run_command` (discovery), waited to its end |
//! | `write_recipe` | `update_recipe` on the current draft revision |
//! | `run_install` | `run_install` of the current draft, waited to its end |
//! | `set_checks` | seals check scripts and the check plan, then `update_recipe` with the frozen plan |
//! | `ask_user` | `pause`: the turn ends until the person answers |
//! | `finish` | `end`, then a clean build and a fresh verification |
//!
//! The person's steering is retained by the setup owner (`steer`); the loop
//! reads new steering before every model request and hands it to the model
//! as the person's message. A reply with no tool call is a question for the
//! person: the session pauses until they answer.
//!
//! Everything the loop needs to continue is retained after every step
//! (`agent.json` beside the activity log): the model conversation, the
//! phase, the request counter, and the build and verify jobs. Tool request
//! identities come from that counter, so a step repeated after a crash is a
//! replay the owners answer from their records, never a second command.

use crate::activity::{CheckLine, Entry, Logs, Stage, tail_of};
use crate::{Owners, now_ms};
use coder_cloud::operator::Profile;
use coder_environment::{Qualification, valid_id};
use coder_environment_build::service::BuildRequest;
use coder_environment_setup::service::{CommandInput, CommandView, RecipeEdit};
use coder_environment_setup::{Run, SetupRequest, SetupState, ToolResult};
use coder_environment_verify::plan::{
    Assertions, Check, CheckKind, CheckPlan, Idempotence, PLAN_SCHEMA, SourceStep, Startup,
};
use coder_environment_verify::service::VerifyRequest;
use coder_working_computer::provider::{Commands, Images};
use coder_working_computer::{HEARTBEAT_EVERY_MS, Principal};
use codex_transport::{Reply, Request, Transport, TransportError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// The qualification profile every plan this agent writes declares.
pub const PROFILE: &str = "repository";
/// Bytes of each output stream the model sees per command.
pub const MODEL_OUTPUT_BYTES: usize = 12 * 1024;
/// The default and longest install timeout.
pub const INSTALL_SECONDS: u64 = 1800;
pub const COMMAND_SECONDS: u64 = 300;
pub const CHECK_SECONDS: u64 = 1800;
/// The name a failed install rerun goes by in the conversation.
const RERUN: &str = "install-rerun";
/// Transient model failures retried before the setup stops.
pub const MODEL_RETRIES: u32 = 4;

const INSTRUCTIONS: &str = "You set up a software repository's environment on a fresh Linux x86-64 cloud computer, the way an engineer would on their first day.

The repository is already checked out at the exact commit in your working directory. Your goal is an install recipe: a shell script, run from the repository root, that installs every system package, toolchain, and dependency the project needs to build and run its tests. A clean machine will later run your script from scratch, so everything must be in the script; nothing you do by hand survives.

How to work:
1. Explore first with run_command: read the README, contributing notes, manifests, lock files, toolchain files, CI workflows, and any setup scripts. Keep commands short and read-only while exploring.
2. Write the recipe with write_recipe. It runs under `sh` (dash on Ubuntu), not bash, and a `#!` line is ignored: write POSIX shell that starts with `set -eu`, or put the whole script inside `bash -euo pipefail <<'SETUP'` ... `SETUP` when you need bash. Use sudo for system packages when it is available. Prefer the project's own lock files and pinned versions. Never put tokens or passwords in the script.
3. Run it with run_install. When it fails, read the error, fix the recipe with write_recipe, and run it again. Keep going until it passes.
4. Choose one to four quick checks that prove the environment works, such as building the project or running a fast part of its tests, and declare them with set_checks. They run on a fresh machine made from your recipe. Set offline to true only when every dependency is installed by the recipe. The fresh machine also runs your recipe a second time on top of the saved image to show it changes nothing; when offline is true that second run has no network, so make every step skip work that is already done (for example, check a toolchain is installed before installing it, and use `--offline` or the package manager's cache when dependencies are present). Declaring checks makes a new recipe revision, so run the install once more after it.
5. When the install passes on the current revision and the checks are declared, call finish with a short summary.

Talk to the person like a colleague: short, plain sentences about what you found and what you are doing. Do not narrate tool mechanics. If you need a decision only the person can make, call ask_user with one clear question. If the person sends a message, follow it.";

/// What one setup needs that never changes during it.
#[derive(Clone, Debug)]
pub struct Brief {
    pub environment: String,
    pub owner: Principal,
    /// The operator profile alias and the admitted profile itself.
    pub profile_alias: String,
    pub profile: Profile,
    /// Credentials the setup may use, by name.
    pub credential_names: BTreeSet<String>,
    /// One of them, for ephemeral Git auth.
    pub git_credential: Option<String>,
    pub deadline_seconds: u64,
    /// The machine size for setup, build, and verify.
    pub size: String,
    /// What the person asked for, in their words.
    pub objective: String,
    /// Plain facts for the model: repository, branch, commit.
    pub context: String,
}

/// Where the setup is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Phase {
    /// Starting the setup computer and putting the source on it.
    Starting,
    /// The agent is working.
    Working,
    /// The agent asked the person something.
    Waiting { question: String },
    /// The clean build is running.
    Building,
    /// The fresh-machine check is running.
    Verifying,
    /// A verified candidate is ready to save.
    Review { verification: String },
    /// Saved as version `number`.
    Saved { number: u64 },
    /// Stopped; the person can retry.
    Failed { reason: String },
}

/// The frozen checks the agent declared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checks {
    pub plan_digest: String,
    pub offline: bool,
    pub lines: Vec<CheckLine>,
}

/// Everything the loop retains to continue after a restart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub environment: String,
    /// Setup attempt: each retry opens a new setup session.
    pub attempt: u32,
    pub phase: Phase,
    /// The model conversation (Responses API input items).
    pub input: Vec<Value>,
    /// Steering messages of the current session already handed over.
    pub seen_steering: usize,
    /// The next tool request number.
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<Checks>,
    /// The draft revision whose install last passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_job: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_job: Option<String>,
}

impl State {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.into(),
            attempt: 1,
            phase: Phase::Starting,
            input: vec![],
            seen_steering: 0,
            seq: 0,
            checks: None,
            installed: None,
            summary: None,
            build_job: None,
            verify_job: None,
        }
    }
    /// The current setup session's identity.
    pub fn session(&self) -> String {
        format!("{}-s{}", self.environment, self.attempt)
    }
    fn next(&mut self, tool: &str) -> String {
        self.seq += 1;
        format!("{tool}-{}-{}", self.attempt, self.seq)
    }
    /// Start over after a failure: a new setup session that keeps the
    /// conversation, so the agent knows what went wrong.
    pub fn retry(&mut self) {
        let reason = match &self.phase {
            Phase::Failed { reason } => reason.clone(),
            _ => return,
        };
        self.attempt += 1;
        self.phase = Phase::Starting;
        self.seen_steering = 0;
        self.installed = None;
        self.build_job = None;
        self.verify_job = None;
        self.summary = None;
        self.input.push(user(&format!(
            "The last attempt stopped: {reason}\nYou are on a new setup computer with the same commit checked out. Any recipe draft and declared checks are kept. Fix what failed, run the install again, and finish."
        )));
    }
}

fn user(text: &str) -> Value {
    json!({"role":"user","content":[{"type":"input_text","text":text}]})
}

/// The tool declarations.
pub fn tools() -> Vec<Value> {
    let object = |properties: Value, required: &[&str]| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    vec![
        json!({"type":"function","name":"run_command","strict":false,
            "description":"Run one shell command in the repository root on the setup computer and get its exit code and output. Use it to explore.",
            "parameters":object(json!({"command":{"type":"string"},"timeout_seconds":{"type":"integer"}}),&["command"])}),
        json!({"type":"function","name":"write_recipe","strict":false,
            "description":"Replace the install recipe with this shell script. It runs from the repository root.",
            "parameters":object(json!({"install_script":{"type":"string"}}),&["install_script"])}),
        json!({"type":"function","name":"run_install","strict":false,
            "description":"Run the current install recipe on the setup computer and get its exit code and output.",
            "parameters":object(json!({"timeout_seconds":{"type":"integer"}}),&[])}),
        json!({"type":"function","name":"set_checks","strict":false,
            "description":"Declare the checks a fresh machine built from the recipe must pass. Each check is a shell command run from the repository root that exits 0 on success.",
            "parameters":object(json!({
                "checks":{"type":"array","items":object(json!({"name":{"type":"string"},"command":{"type":"string"}}),&["name","command"])},
                "offline":{"type":"boolean"}
            }),&["checks","offline"])}),
        json!({"type":"function","name":"ask_user","strict":false,
            "description":"Ask the person one question and wait for the answer.",
            "parameters":object(json!({"question":{"type":"string"}}),&["question"])}),
        json!({"type":"function","name":"finish","strict":false,
            "description":"The install passes on the current recipe and checks are declared: build a clean image and check it on a fresh machine.",
            "parameters":object(json!({"summary":{"type":"string"}}),&["summary"])}),
    ]
}

/// The shell text of one check: the command, then the marker line the
/// verifier counts.
pub fn check_script(command: &str) -> String {
    format!(
        "sh -c {}\nstatus=$?\nif [ \"$status\" -eq 0 ]; then echo 'OA-CHECK passed=1 failed=0'; else echo 'OA-CHECK passed=0 failed=1'; fi\nexit \"$status\"\n",
        boat::shell_quote(command)
    )
}

/// The loop over one environment's setup.
pub struct Agent<P, T> {
    pub owners: Arc<Owners<P>>,
    pub transport: T,
    pub model: String,
    pub logs: Arc<Logs>,
    /// The studio directory (`<root>/<environment>/agent.json`).
    pub root: PathBuf,
    /// How long to wait between reads of a running command or job.
    pub poll: Duration,
}

/// What one tool call did to the loop.
enum Next {
    Continue,
    /// Stop this turn's remaining calls; the phase changed.
    Stop,
}

impl<P: Commands + Images, T: Transport> Agent<P, T> {
    fn state_path(&self, environment: &str) -> PathBuf {
        self.root.join(environment).join("agent.json")
    }

    /// The retained state of `environment`'s setup, if one started.
    pub fn load(&self, environment: &str) -> Option<State> {
        let bytes = fs::read(self.state_path(environment)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Retain `state` atomically.
    pub fn save(&self, state: &State) -> Result<(), String> {
        let path = self.state_path(&state.environment);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("Cannot keep the setup state: {e}"))?;
        }
        let temp = path.with_extension("json.writing");
        let bytes = serde_json::to_vec(state).map_err(|_| "Cannot encode the setup state.")?;
        fs::write(&temp, bytes)
            .and_then(|_| fs::rename(&temp, &path))
            .map_err(|e| format!("Cannot keep the setup state: {e}"))
    }

    fn log(&self, environment: &str, entry: Entry) {
        let _ = self.logs.push(environment, entry, now_ms());
    }

    fn fail(&self, state: &mut State, reason: impl Into<String>) {
        let reason = reason.into();
        self.log(
            &state.environment,
            Entry::Failed {
                reason: reason.clone(),
            },
        );
        state.phase = Phase::Failed { reason };
    }

    /// Run the setup as far as it can go now: until it waits for the
    /// person, is ready to save, is saved, or failed. Returns the state it
    /// stopped in (also retained).
    ///
    /// While it runs, the setup turn's computer hears a heartbeat every
    /// [`HEARTBEAT_EVERY_MS`] however long a step takes, so only a loop
    /// that is gone leaves the turn silent (#11059).
    pub async fn run(&self, brief: &Brief, state: State) -> State {
        let session = state.session();
        let beat = async {
            let mut every = tokio::time::interval(Duration::from_millis(HEARTBEAT_EVERY_MS));
            every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                every.tick().await;
                let _ = self.owners.setup.heartbeat(&session, now_ms()).await;
            }
        };
        tokio::select! {
            state = self.steps(brief, state) => state,
            _ = beat => unreachable!("the heartbeat never ends"),
        }
    }

    async fn steps(&self, brief: &Brief, mut state: State) -> State {
        loop {
            match state.phase.clone() {
                Phase::Starting => self.start(brief, &mut state).await,
                Phase::Working => self.turn(brief, &mut state).await,
                Phase::Waiting { .. } => {
                    if !self.take_steering(&mut state) {
                        let _ = self.save(&state);
                        return state;
                    }
                    state.phase = Phase::Working;
                }
                Phase::Building => self.build(brief, &mut state).await,
                Phase::Verifying => self.verify(brief, &mut state).await,
                Phase::Failed { reason } => {
                    // A stopped setup keeps no machine running.
                    self.release(&state.session(), &reason).await;
                    let _ = self.save(&state);
                    return state;
                }
                Phase::Review { .. } | Phase::Saved { .. } => {
                    let _ = self.save(&state);
                    return state;
                }
            }
            if let Err(e) = self.save(&state) {
                self.fail(&mut state, e);
                let _ = self.save(&state);
                return state;
            }
        }
    }

    /// Cancel a setup session that is still open, which deletes its
    /// computer. Nothing to do when it never opened or already ended.
    async fn release(&self, session: &str, reason: &str) {
        match self.owners.setup.sessions().read(session) {
            Ok(s) if !s.state.terminal() => {
                let _ = self
                    .owners
                    .setup
                    .cancel(session, &plain_reason(reason), now_ms())
                    .await;
            }
            _ => {}
        }
    }

    async fn start(&self, brief: &Brief, state: &mut State) {
        // Earlier attempts that stopped without ending keep no machine.
        for attempt in 1..state.attempt {
            self.release(
                &format!("{}-s{attempt}", state.environment),
                "A new setup attempt started.",
            )
            .await;
        }
        let session = state.session();
        let request = SetupRequest {
            session: session.clone(),
            environment: brief.environment.clone(),
            owner: brief.owner.clone(),
            profile: brief.profile_alias.clone(),
            objective: brief.objective.clone(),
            credential_names: brief.credential_names.clone(),
            git_credential: brief.git_credential.clone(),
            deadline_seconds: brief.deadline_seconds,
        };
        if state.input.is_empty() {
            state.input.push(user(&format!(
                "{}\n\n{}\nThe repository is checked out in your working directory.",
                brief.objective, brief.context
            )));
        }
        let mut opened = None;
        let mut last = String::new();
        for _ in 0..60 {
            match self
                .owners
                .setup
                .open(&request, &brief.profile, now_ms())
                .await
            {
                Ok(s) => {
                    opened = Some(s);
                    break;
                }
                Err(coder_environment_setup::service::SetupError::NotReady(m)) => {
                    last = m;
                    tokio::time::sleep(self.poll * 5).await;
                }
                Err(e) => {
                    last = e.to_string();
                    break;
                }
            }
        }
        if opened.is_none() {
            return self.fail(
                state,
                format!("The setup computer didn't start. {}", plain_reason(&last)),
            );
        }
        let source = match self
            .owners
            .setup
            .materialize_source(&session, "source", 900, now_ms())
            .await
        {
            Ok(ToolResult::CommandStarted { command }) => command,
            Ok(_) => return self.fail(state, "The repository couldn't be checked out."),
            Err(e) => {
                return self.fail(
                    state,
                    format!(
                        "The repository couldn't be checked out. {}",
                        plain_reason(&e.to_string())
                    ),
                );
            }
        };
        let (run, out, err) = match self.wait(&session, &source).await {
            Ok(v) => v,
            Err(e) => return self.fail(state, e),
        };
        let ok = run.succeeded();
        let revision = self
            .owners
            .setup
            .environments
            .read(&brief.environment)
            .map(|e| e.source.revision)
            .unwrap_or_default();
        self.log(
            &state.environment,
            Entry::Source {
                ok,
                revision,
                output: shown(&run, &out, &err),
            },
        );
        if !ok {
            return self.fail(
                state,
                match unfinished(&run) {
                    Some(why) => format!("The repository couldn't be checked out. {why}"),
                    None => match exit_code(&run) {
                        Some(code) if out.trim().is_empty() && err.trim().is_empty() => format!(
                            "The repository couldn't be checked out at that commit (exit {code}, no output)."
                        ),
                        _ => "The repository couldn't be checked out at that commit.".into(),
                    },
                },
            );
        }
        state.phase = Phase::Working;
    }

    /// Wait for a setup command to end; returns its end and all the
    /// output read while waiting.
    async fn wait(&self, session: &str, command: &str) -> Result<(Run, String, String), String> {
        let (mut out, mut err) = (String::new(), String::new());
        let mut failures = 0;
        loop {
            match self.owners.setup.poll(session, command, now_ms()).await {
                Ok(CommandView {
                    command: record,
                    stdout,
                    stderr,
                    ..
                }) => {
                    out.push_str(&stdout);
                    err.push_str(&stderr);
                    if out.len() > 4 * MODEL_OUTPUT_BYTES {
                        out = tail_of(&out, 2 * MODEL_OUTPUT_BYTES);
                    }
                    if err.len() > 4 * MODEL_OUTPUT_BYTES {
                        err = tail_of(&err, 2 * MODEL_OUTPUT_BYTES);
                    }
                    if !record.run.active() {
                        return Ok((record.run, out, err));
                    }
                }
                Err(e) => {
                    failures += 1;
                    if failures > 30 {
                        return Err(format!(
                            "The setup computer stopped answering. {}",
                            plain_reason(&e.to_string())
                        ));
                    }
                }
            }
            tokio::time::sleep(self.poll).await;
        }
    }

    /// Hand new steering to the model. Returns whether there was any.
    fn take_steering(&self, state: &mut State) -> bool {
        let Ok(s) = self.owners.setup.sessions().read(&state.session()) else {
            return false;
        };
        let new: Vec<String> = s
            .steering
            .iter()
            .skip(state.seen_steering)
            .map(|t| t.text.clone())
            .collect();
        state.seen_steering = s.steering.len();
        for text in &new {
            state.input.push(user(text));
        }
        !new.is_empty()
    }

    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        let mut tries = 0;
        loop {
            match self.transport.respond(request).await {
                Err(e) if e.transient() && tries < MODEL_RETRIES => {
                    tries += 1;
                    tokio::time::sleep(self.poll * (5 * tries)).await;
                }
                other => return other,
            }
        }
    }

    async fn turn(&self, brief: &Brief, state: &mut State) {
        let session = state.session();
        match self.owners.setup.sessions().read(&session) {
            Ok(s) if s.state.terminal() => {
                let reason = match s.state {
                    SetupState::Cancelled { reason } | SetupState::Failed { reason } => reason,
                    _ => "The setup session ended.".into(),
                };
                return self.fail(state, reason);
            }
            Ok(_) => {}
            Err(e) => return self.fail(state, e.to_string()),
        }
        self.take_steering(state);
        let request = Request {
            model: self.model.clone(),
            instructions: INSTRUCTIONS.into(),
            input: state.input.clone(),
            tools: tools(),
            effort: Some("medium".into()),
            cache_key: format!("environment-setup-{}", state.environment),
            parallel_tools: false,
            text_format: None,
        };
        let reply = match self.respond(&request).await {
            Ok(r) => r,
            Err(e) => {
                return self.fail(
                    state,
                    format!("The setup agent couldn't reach its model: {e}"),
                );
            }
        };
        state.input.extend(reply.items.clone());
        let text = reply.text();
        if !text.trim().is_empty() {
            self.log(
                &state.environment,
                Entry::Agent {
                    text: text.trim().into(),
                },
            );
        }
        let calls = reply.calls();
        if calls.is_empty() {
            // A turn that ends in words is a question for the person.
            let question = if text.trim().is_empty() {
                "What should I do next?".to_owned()
            } else {
                text.trim().to_owned()
            };
            return self.pause(state, &question).await;
        }
        let mut stopped = false;
        for call in calls {
            let output = if stopped {
                json!({"error":"Not run: the turn ended before this call."})
            } else {
                let arguments: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
                let (output, next) = self.tool(brief, state, &call.name, &arguments).await;
                stopped = matches!(next, Next::Stop);
                output
            };
            state.input.push(json!({"type":"function_call_output","call_id":call.call_id,"output":output.to_string()}));
            if let Err(e) = self.save(state) {
                return self.fail(state, e);
            }
        }
    }

    async fn pause(&self, state: &mut State, question: &str) {
        match self
            .owners
            .setup
            .pause(&state.session(), question, now_ms())
            .await
        {
            Ok(_) => {
                state.phase = Phase::Waiting {
                    question: question.into(),
                }
            }
            Err(e) => self.fail(state, plain_reason(&e.to_string())),
        }
    }

    async fn tool(
        &self,
        brief: &Brief,
        state: &mut State,
        name: &str,
        args: &Value,
    ) -> (Value, Next) {
        let session = state.session();
        let text = |field: &str| args[field].as_str().unwrap_or_default().to_owned();
        let seconds = |default: u64| {
            args["timeout_seconds"]
                .as_u64()
                .filter(|s| *s > 0)
                .unwrap_or(default)
                .min(INSTALL_SECONDS)
        };
        match name {
            "run_command" => {
                let command = text("command");
                if command.trim().is_empty() {
                    return (json!({"error":"The command is empty."}), Next::Continue);
                }
                let request = state.next("cmd");
                let started = self
                    .owners
                    .setup
                    .run_command(
                        &session,
                        &request,
                        &CommandInput {
                            command: command.clone(),
                            cwd: ".".into(),
                            credential_names: BTreeSet::new(),
                            git_auth: false,
                            timeout_seconds: seconds(COMMAND_SECONDS),
                        },
                        now_ms(),
                    )
                    .await;
                let id = match started {
                    Ok(ToolResult::CommandStarted { command }) => command,
                    Ok(_) => return (json!({"error":"The command didn't start."}), Next::Continue),
                    Err(e) => return (json!({"error":e.to_string()}), Next::Continue),
                };
                match self.wait(&session, &id).await {
                    Ok((run, out, err)) => {
                        let exit = exit_code(&run);
                        self.log(
                            &state.environment,
                            Entry::Explored {
                                command,
                                exit,
                                output: shown(&run, &out, &err),
                            },
                        );
                        (result(&run, &out, &err), Next::Continue)
                    }
                    Err(e) => {
                        self.fail(state, e);
                        (json!({"error":"The setup stopped."}), Next::Stop)
                    }
                }
            }
            "write_recipe" => {
                let script = text("install_script");
                match self.write(state, &session, &script, None).await {
                    Ok(revision) => (json!({"revision":revision}), Next::Continue),
                    Err(e) => (json!({"error":e}), Next::Continue),
                }
            }
            "run_install" => {
                let draft = match self.owners.setup.inspect(&session) {
                    Ok(i) => i,
                    Err(e) => return (json!({"error":e.to_string()}), Next::Continue),
                };
                if draft.install_script.is_none() {
                    return (
                        json!({"error":"There is no recipe yet. Write one with write_recipe."}),
                        Next::Continue,
                    );
                }
                let revision = draft.draft_revision;
                let request = state.next("install");
                let id = match self
                    .owners
                    .setup
                    .run_install(
                        &session,
                        &request,
                        revision,
                        seconds(INSTALL_SECONDS),
                        now_ms(),
                    )
                    .await
                {
                    Ok(ToolResult::CommandStarted { command }) => command,
                    Ok(_) => return (json!({"error":"The install didn't start."}), Next::Continue),
                    Err(e) => return (json!({"error":e.to_string()}), Next::Continue),
                };
                match self.wait(&session, &id).await {
                    Ok((run, out, err)) => {
                        self.log(
                            &state.environment,
                            Entry::Install {
                                revision,
                                exit: exit_code(&run),
                                output: shown(&run, &out, &err),
                            },
                        );
                        state.installed = run.succeeded().then_some(revision);
                        let mut r = result(&run, &out, &err);
                        r["revision"] = json!(revision);
                        (r, Next::Continue)
                    }
                    Err(e) => {
                        self.fail(state, e);
                        (json!({"error":"The setup stopped."}), Next::Stop)
                    }
                }
            }
            "set_checks" => match self.set_checks(state, &session, args).await {
                Ok(revision) => (
                    json!({"revision":revision,"note":"Run the install again on this revision before finishing."}),
                    Next::Continue,
                ),
                Err(e) => (json!({"error":e}), Next::Continue),
            },
            "ask_user" => {
                let question = text("question");
                let question = if question.trim().is_empty() {
                    "What should I do next?".to_owned()
                } else {
                    question
                };
                self.log(
                    &state.environment,
                    Entry::Question {
                        text: question.clone(),
                    },
                );
                self.pause(state, &question).await;
                (
                    json!({"asked":true,"note":"The person's answer arrives as their next message."}),
                    Next::Stop,
                )
            }
            "finish" => {
                let current = self
                    .owners
                    .setup
                    .inspect(&session)
                    .map(|i| i.draft_revision)
                    .ok();
                if state.checks.is_none() {
                    return (
                        json!({"error":"Declare the checks with set_checks first."}),
                        Next::Continue,
                    );
                }
                if current.is_none() || state.installed != current {
                    return (
                        json!({"error":"Run the install on the current recipe revision first; it must pass."}),
                        Next::Continue,
                    );
                }
                if let Err(e) = self.owners.setup.end(&session, now_ms()).await {
                    return (json!({"error":e.to_string()}), Next::Continue);
                }
                state.summary = Some(text("summary"));
                state.phase = Phase::Building;
                let _ = brief;
                (json!({"building":true}), Next::Stop)
            }
            other => (
                json!({"error":format!("There is no tool named {other}.")}),
                Next::Continue,
            ),
        }
    }

    /// Write a new recipe revision; returns it.
    async fn write(
        &self,
        state: &mut State,
        session: &str,
        script: &str,
        qualification: Option<Qualification>,
    ) -> Result<u64, String> {
        if script.trim().is_empty() {
            return Err("The install script is empty.".into());
        }
        let current = self
            .owners
            .setup
            .inspect(session)
            .map_err(|e| e.to_string())?;
        let request = state.next("recipe");
        let edit = RecipeEdit {
            install_script: script.into(),
            install_cwd: ".".into(),
            start: None,
            inputs: None,
            credential_names: None,
            qualification,
            limits: None,
            capture: None,
        };
        match self
            .owners
            .setup
            .update_recipe(session, &request, current.draft_revision, &edit, now_ms())
            .await
            .map_err(|e| e.to_string())?
        {
            ToolResult::RecipeRevised { revision, .. } => {
                state.installed = None;
                if current.install_script.as_deref() != Some(script) {
                    self.log(
                        &state.environment,
                        Entry::Recipe {
                            revision,
                            script: script.into(),
                            previous: current.install_script,
                        },
                    );
                }
                Ok(revision)
            }
            ToolResult::CommandStarted { .. } => Err("The recipe wasn't saved.".into()),
        }
    }

    async fn set_checks(
        &self,
        state: &mut State,
        session: &str,
        args: &Value,
    ) -> Result<u64, String> {
        let offline = args["offline"].as_bool().unwrap_or(false);
        let mut lines = vec![];
        let mut checks = vec![];
        let mut names = BTreeSet::new();
        for c in args["checks"].as_array().into_iter().flatten() {
            let name = c["name"].as_str().unwrap_or_default().trim().to_owned();
            let command = c["command"].as_str().unwrap_or_default().trim().to_owned();
            let name = slug(&name);
            if !valid_id(&name) || name.len() > 48 || !names.insert(name.clone()) {
                return Err("Each check needs a short, unique name.".into());
            }
            if command.is_empty() {
                return Err(format!("Check {name} has no command."));
            }
            let script = self
                .owners
                .seal_artifact(check_script(&command).as_bytes())?;
            checks.push(Check {
                name: name.clone(),
                kind: CheckKind::Behavior,
                script,
                cwd: ".".into(),
                timeout_seconds: CHECK_SECONDS,
                assertions: Assertions::Marker { min_passed: 1 },
            });
            lines.push(CheckLine { name, command });
        }
        if checks.is_empty() || checks.len() > 8 {
            return Err("Declare one to eight checks.".into());
        }
        let plan = CheckPlan {
            schema: PLAN_SCHEMA.into(),
            profile: PROFILE.into(),
            source: SourceStep::Contained,
            offline,
            startup: Startup::NotApplicable {
                reason: "No services are declared.".into(),
            },
            checks,
            idempotence: Idempotence::default(),
        };
        plan.validate()?;
        let bytes = serde_json::to_vec_pretty(&plan).map_err(|_| "Cannot encode the plan.")?;
        let plan_digest = self.owners.seal_artifact(&bytes)?;
        let script = self
            .owners
            .setup
            .inspect(session)
            .map_err(|e| e.to_string())?
            .install_script
            .ok_or("Write the recipe with write_recipe first.")?;
        let revision = self
            .write(
                state,
                session,
                &script,
                Some(Qualification {
                    profile: PROFILE.into(),
                    plan_digest: plan_digest.clone(),
                }),
            )
            .await?;
        self.log(
            &state.environment,
            Entry::Checks {
                checks: lines.clone(),
            },
        );
        state.checks = Some(Checks {
            plan_digest,
            offline,
            lines,
        });
        Ok(revision)
    }

    async fn build(&self, brief: &Brief, state: &mut State) {
        let owners = &self.owners;
        let mut job = match &state.build_job {
            Some(id) => match owners.builder.advance(id, now_ms()).await {
                Ok(j) => j,
                Err(e) => return self.fail(state, format!("The clean build stopped: {e}")),
            },
            None => {
                let draft = match owners.setup.environments.read(&brief.environment) {
                    Ok(e) => e.draft_revision,
                    Err(e) => return self.fail(state, e.to_string()),
                };
                self.log(
                    &state.environment,
                    Entry::Build {
                        stage: Stage::Started,
                        detail: format!("Recipe revision {draft} on a fresh machine"),
                    },
                );
                let request = BuildRequest {
                    request_id: format!("build-{}-{draft}", state.attempt),
                    environment: brief.environment.clone(),
                    owner: brief.owner.clone(),
                    expected_draft_revision: draft,
                    size: brief.size.clone(),
                };
                match owners.build(&request, now_ms()).await {
                    Ok(j) => {
                        state.build_job = Some(j.id.clone());
                        let _ = self.save(state);
                        j
                    }
                    Err(e) => {
                        return self.fail(state, format!("The clean build didn't start: {e}"));
                    }
                }
            }
        };
        let mut failures = 0;
        while !job.phase.terminal() {
            tokio::time::sleep(self.poll * 5).await;
            match owners.builder.advance(&job.id, now_ms()).await {
                Ok(j) => job = j,
                Err(e) => {
                    failures += 1;
                    if failures > 30 {
                        return self.fail(state, format!("The clean build stopped: {e}"));
                    }
                }
            }
        }
        if job.phase == coder_environment_build::Phase::Ready {
            self.log(
                &state.environment,
                Entry::Build {
                    stage: Stage::Passed,
                    detail: "Image saved".into(),
                },
            );
            state.phase = Phase::Verifying;
        } else {
            let reason = job
                .reason
                .clone()
                .unwrap_or_else(|| "The clean build didn't finish.".into());
            self.log(
                &state.environment,
                Entry::Build {
                    stage: Stage::Failed,
                    detail: reason.clone(),
                },
            );
            crate::images::sweep(&self.owners, &self.root, &state.environment).await;
            self.fail(state, format!("The clean build failed: {reason}"));
        }
    }

    async fn verify(&self, brief: &Brief, state: &mut State) {
        let owners = &self.owners;
        let Some(build_id) = state
            .build_job
            .as_ref()
            .and_then(|id| owners.builder.jobs.read(id).ok())
            .map(|j| j.inputs.build_id)
        else {
            return self.fail(state, "The clean build is missing.");
        };
        let Some(checks) = state.checks.clone() else {
            return self.fail(state, "No checks were declared.");
        };
        let mut job = match &state.verify_job {
            Some(id) => match owners.verifier.advance(id, now_ms()).await {
                Ok(j) => j,
                Err(e) => return self.fail(state, format!("The fresh-machine check stopped: {e}")),
            },
            None => {
                self.log(
                    &state.environment,
                    Entry::Verify {
                        stage: Stage::Started,
                        detail: format!(
                            "{} on a fresh machine from the image",
                            plural(checks.lines.len(), "check")
                        ),
                    },
                );
                let request = VerifyRequest {
                    request_id: format!("verify-{build_id}"),
                    environment: brief.environment.clone(),
                    build_id: build_id.clone(),
                    owner: brief.owner.clone(),
                    plan_digest: checks.plan_digest.clone(),
                    size: brief.size.clone(),
                };
                match owners.verify(&request, now_ms()).await {
                    Ok(j) => {
                        state.verify_job = Some(j.id.clone());
                        let _ = self.save(state);
                        j
                    }
                    Err(e) => {
                        return self
                            .fail(state, format!("The fresh-machine check didn't start: {e}"));
                    }
                }
            }
        };
        let mut failures = 0;
        while job.phase != coder_environment_verify::Phase::Done {
            tokio::time::sleep(self.poll * 5).await;
            match owners.verifier.advance(&job.id, now_ms()).await {
                Ok(j) => job = j,
                Err(e) => {
                    failures += 1;
                    if failures > 30 {
                        return self.fail(state, format!("The fresh-machine check stopped: {e}"));
                    }
                }
            }
        }
        match &job.verdict {
            Some(coder_environment_verify::Verdict::Passed) => {
                let env = match owners.setup.environments.read(&brief.environment) {
                    Ok(e) => e,
                    Err(e) => return self.fail(state, e.to_string()),
                };
                let Some(v) = env
                    .verifications
                    .iter()
                    .rev()
                    .find(|v| v.build_id == build_id)
                else {
                    return self.fail(state, "The check result is missing.");
                };
                if let Err(e) = env.propose(&v.id) {
                    return self.fail(state, format!("This result can't be saved: {e}"));
                }
                self.log(
                    &state.environment,
                    Entry::Verify {
                        stage: Stage::Passed,
                        detail: format!("{} passed", plural(checks.lines.len(), "check")),
                    },
                );
                self.log(
                    &state.environment,
                    Entry::Ready {
                        summary: state.summary.clone().unwrap_or_default(),
                    },
                );
                state.phase = Phase::Review {
                    verification: v.id.clone(),
                };
            }
            other => {
                let reason = match other {
                    Some(coder_environment_verify::Verdict::Failed { reason })
                    | Some(coder_environment_verify::Verdict::Incomplete { reason })
                    | Some(coder_environment_verify::Verdict::Cancelled { reason }) => {
                        reason.clone()
                    }
                    _ => "The fresh machine didn't finish its checks.".into(),
                };
                self.log(
                    &state.environment,
                    Entry::Verify {
                        stage: Stage::Failed,
                        detail: reason.clone(),
                    },
                );
                if let Some((name, exit, output)) = self.failed_check(&job) {
                    let command = if name == RERUN {
                        "the install recipe, run again on the image".to_owned()
                    } else {
                        checks
                            .lines
                            .iter()
                            .find(|l| l.name == name)
                            .map(|l| l.command.clone())
                            .unwrap_or_default()
                    };
                    // The model sees the check's output on the next attempt.
                    state.input.push(user(&format!(
                        "On the fresh machine built from the recipe, check {name} (`{command}`) failed. Its output ends with:\n{}",
                        tail_of(&output, MODEL_OUTPUT_BYTES)
                    )));
                    self.log(
                        &state.environment,
                        Entry::CheckFailed {
                            name,
                            command,
                            exit,
                            output,
                        },
                    );
                }
                // The image that failed its check is never saved; free
                // its place in the account's image allowance.
                crate::images::sweep(&self.owners, &self.root, &state.environment).await;
                self.fail(state, format!("The fresh-machine check failed: {reason}"));
            }
        }
    }
}

impl<P: Commands + Images, T: Transport> Agent<P, T> {
    /// The first declared check that failed in `job`, with the output the
    /// verifier kept for it.
    fn failed_check(
        &self,
        job: &coder_environment_verify::VerifyJob,
    ) -> Option<(String, Option<i64>, String)> {
        use coder_environment_verify::plan::Action;
        let step = job.steps.iter().find(|s| {
            matches!(s.action, Action::Check { .. } | Action::Install)
                && matches!(
                    s.outcome,
                    Some(coder_environment_verify::StepOutcome::Failed { .. })
                )
        })?;
        let name = match &step.action {
            Action::Check { name } => name.clone(),
            Action::Install => RERUN.to_owned(),
            _ => return None,
        };
        let role = serde_json::to_value(step.role).ok()?;
        let streams = self
            .owners
            .verifier
            .evidence_dir(&job.id)
            .join("children")
            .join(role.as_str()?)
            .join("streams");
        let read = |ext: &str| {
            fs::read(streams.join(format!("{}.{ext}", step.id)))
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default()
        };
        let output = format!("{}{}", read("stdout"), read("stderr"));
        Some((
            name,
            match step.run {
                coder_environment_verify::Run::Exited { code } => Some(code),
                _ => None,
            },
            tail_of(&output, MODEL_OUTPUT_BYTES),
        ))
    }
}

fn exit_code(run: &Run) -> Option<i64> {
    match run {
        Run::Exited { code } => Some(*code),
        _ => None,
    }
}

/// Why a command ended without an exit code, for a person; `None` when it
/// exited.
fn unfinished(run: &Run) -> Option<String> {
    match run {
        Run::Exited { .. } => None,
        Run::TimedOut => Some("It ran past its time limit.".into()),
        Run::Stopped { reason } => Some(format!("It was stopped: {}", plain_reason(reason))),
        Run::Lost => Some("It ended without reporting an exit code.".into()),
        Run::NotStarted { reason } => Some(format!("It didn't start: {}", plain_reason(reason))),
        Run::Unknown { reason } => Some(format!("Its state is unknown: {}", plain_reason(reason))),
        Run::Requested | Run::Running { .. } => None,
    }
}

/// A command's output as the conversation shows it, with the reason it
/// ended early when it did.
fn shown(run: &Run, out: &str, err: &str) -> String {
    let mut text = format!("{out}{err}");
    if let Some(why) = unfinished(run) {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&why);
    }
    text
}

fn result(run: &Run, out: &str, err: &str) -> Value {
    let status = match run {
        Run::Exited { code } => json!(code),
        Run::TimedOut => json!("timed out"),
        Run::Stopped { reason } => json!(format!("stopped: {reason}")),
        Run::Lost => json!("lost"),
        Run::NotStarted { reason } => json!(format!("not started: {reason}")),
        _ => json!("unknown"),
    };
    json!({
        "exit": status,
        "stdout": tail_of(out, MODEL_OUTPUT_BYTES),
        "stderr": tail_of(err, MODEL_OUTPUT_BYTES),
    })
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches('-').to_owned()
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// The first sentence of an owner error, for a person.
pub fn plain_reason(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        return String::new();
    }
    tail_of(reason, 300)
}

#[cfg(test)]
mod tests;
