//! Child agent delegation: a fan-out of real coding agents, each in its own
//! working directory, streaming as they go.
//!
//! What was here reported a fan-out it had not performed. `worktree_path` was
//! hardcoded `None` and nothing read it, so every child ran in the parent's
//! directory and two children told to edit the same file edited the same file.
//! There was no concurrency limit, no way to stop a running fan-out, and no
//! streaming: all four CLI harnesses called `Command::output()`, which returns
//! when the child is finished, so a reader watching a four-minute child saw
//! nothing for four minutes and then everything at once.
//!
//! Now:
//!
//! - Each child gets its own directory ([`crate::workspace`]), a detached git
//!   worktree of `HEAD` inside a checkout.
//! - `--count` children run at once, under a cap (`--max-parallel`), bounded
//!   by [`MAX_DELEGATE_COUNT`].
//! - Every child's output is forwarded line by line as it arrives, prefixed
//!   with the child it came from.
//! - `ctrl+c` stops the fan-out: children are signalled `SIGTERM` by process
//!   group, then `SIGKILL` after a grace period, so a child's own subprocesses
//!   go with it.
//! - A child that fails is reported as failed, and a fan-out with any failed
//!   child is a failed command.
//!
//! The lanes follow `coder-delegate.ts`: `openagents` runs in this process on
//! the OpenAgents inference proxy with this session's tools; `claude`,
//! `codex`, and `gemini`/`opencode/*` run the corresponding CLI on this
//! machine; `devin` runs the Devin CLI as an ACP server ([`crate::acp`]).

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Semaphore, mpsc, watch};

use crate::acp::{AcpEvent, AcpFailure, AcpHarness, PermissionMode};
use crate::cli::{CoderArgs, DelegateArgs, fail};
use crate::runtime::{CoderRuntimeSession, Lane};
use crate::signals::stop_tree;
use crate::tools::HarnessToolRegistry;
use crate::workspace::{ChildWorkspace, Isolation, WorkspacePlan};

/// The most children one fan-out may run. Matches `MAX_DELEGATE_COUNT` in
/// `coder-delegate.ts`.
pub const MAX_DELEGATE_COUNT: usize = 32;

/// How much of a child's answer is kept for the report.
pub const CHILD_RESULT_LIMIT: usize = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerTask {
    pub id: usize,
    pub prompt: String,
    pub lane: String,
    /// Where this child works. Populated now, and read.
    pub worktree_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerResult {
    pub id: usize,
    pub success: bool,
    /// The child's answer when it succeeded, or why it did not.
    pub output: String,
    pub duration_ms: u128,
    /// The operating system process, for a child that is one. `openagents` runs
    /// in this process and reports `None`; what it starts are its `shell` tool
    /// subprocesses.
    pub pid: Option<u32>,
    pub workspace: Option<PathBuf>,
    /// Set when the child did not answer, so a caller does not have to read
    /// `output` to find out.
    pub failure: Option<String>,
    /// The swarm session id the child registered under, when it did. The
    /// parent can address a running child through it, and the report shows
    /// the exchange that travelled over it.
    pub swarm_id: Option<String>,
    /// Messages the child received while it worked, as (from, kind, body)
    /// within [`CHILD_RESULT_LIMIT`], read before its workspace went away.
    pub swarm_messages: Vec<(String, String, String)>,
}

/// What a child reports while it works.
#[derive(Debug, Clone)]
pub enum ChildEvent {
    Started {
        id: usize,
        lane: String,
        workspace: String,
        pid: Option<u32>,
    },
    /// A piece of what the child wrote, exactly as it arrived.
    Output {
        id: usize,
        text: String,
    },
    /// Something the child did that is not its answer: a tool call, a token
    /// count, a session id.
    Activity {
        id: usize,
        text: String,
    },
    Finished(Box<ChildWorkerResult>),
}

/// The line a parent `delegate` box shows for one child event.
///
/// `Finished` is omitted: the parent tool result already carries the trailer.
pub fn child_event_subagent_line(event: &ChildEvent) -> Option<String> {
    match event {
        ChildEvent::Started {
            lane, workspace, ..
        } => Some(format!("started on {lane} in {workspace}")),
        ChildEvent::Output { text, .. } | ChildEvent::Activity { text, .. } => {
            let text = text.trim_end();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        ChildEvent::Finished(_) => None,
    }
}

/// The lane a fan-out runs on when `--lane` names none.
///
/// It is not a neutral choice: `openagents` is this process on the OpenAgents
/// proxy, so every child on it opens a thread and spends this account's grant.
/// The other lanes shell out to a harness the reader installed and pays for
/// themselves. That is why omitting `--lane` is reported at the point of
/// spending rather than left to be inferred from the header — see
/// [`run_delegation`].
pub const DEFAULT_CHILD_LANE: &str = "openagents";

/// Which harness and model a child runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildLane {
    /// This process, on the OpenAgents inference proxy, with this session's
    /// tools.
    OpenAgents,
    /// The `opencode` CLI on this machine, with its own tools.
    Opencode { model: String },
    /// The Devin CLI as an ACP server.
    Devin,
    /// The Claude Code CLI in print mode.
    Claude,
    /// The OpenAI Codex CLI in exec mode.
    Codex,
}

impl ChildLane {
    pub fn parse(name: &str) -> Result<Self, String> {
        let lowered = name.trim().to_lowercase();
        match lowered.as_str() {
            "gemini" | "gemini-flash" => Ok(ChildLane::Opencode {
                model: "gemini-3.7-flash".to_string(),
            }),
            "devin" => Ok(ChildLane::Devin),
            "claude" => Ok(ChildLane::Claude),
            "codex" => Ok(ChildLane::Codex),
            "openagents" | "ox-alpha" | "ox" => Ok(ChildLane::OpenAgents),
            other if other.starts_with("opencode/") && other.len() > "opencode/".len() => {
                Ok(ChildLane::Opencode {
                    model: other.trim_start_matches("opencode/").to_string(),
                })
            }
            _ => Err(format!(
                "there is no `{name}` lane. This command runs children on: openagents, gemini, opencode/<model>, devin, claude, codex."
            )),
        }
    }

    /// Whether [`ChildLane::parse`] recognised the name it was given.
    pub fn known(name: &str) -> bool {
        Self::parse(name).is_ok()
    }

    /// The current lane name when `name` is a retired compatibility alias.
    pub fn renamed_alias(name: &str) -> Option<&'static str> {
        matches!(name.trim().to_lowercase().as_str(), "ox-alpha" | "ox").then_some("openagents")
    }

    /// Resolve a session's lane into the child lane children run on.
    ///
    /// A session's lane is an inference lane, not a child lane. If it names a
    /// recognised child lane, use it. If it is a recognised inference tier
    /// (`flash`, `free`, `local`) it has no equivalent child lane, so children
    /// run on the default. Any other unparseable name is refused.
    pub fn resolve_for_session(name: &str) -> Result<Self, String> {
        match Self::parse(name) {
            Ok(lane) => Ok(lane),
            Err(why) => {
                // The session's lane may be an inference tier rather than a
                // child lane. Those are not parseable as child lanes, but they
                // are not an error either: the fan-out falls back to the
                // default child lane.
                if Lane::from_str(name).tier().is_some() {
                    Self::parse(DEFAULT_CHILD_LANE)
                } else {
                    Err(why)
                }
            }
        }
    }

    /// The canonical child-lane name, usable with [`ChildLane::parse`].
    pub fn name(&self) -> String {
        match self {
            ChildLane::OpenAgents => "openagents".to_string(),
            ChildLane::Opencode { model } => format!("opencode/{model}"),
            ChildLane::Devin => "devin".to_string(),
            ChildLane::Claude => "claude".to_string(),
            ChildLane::Codex => "codex".to_string(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            ChildLane::OpenAgents => "openagents (this process, the OpenAgents proxy)".to_string(),
            ChildLane::Opencode { model } => format!("opencode ({model})"),
            ChildLane::Devin => "devin (ACP over the Devin CLI)".to_string(),
            ChildLane::Claude => "claude (Claude Code print mode)".to_string(),
            ChildLane::Codex => "codex (Codex exec)".to_string(),
        }
    }

    /// The binary this lane needs on `PATH`, if it needs one.
    pub fn binary(&self) -> Option<&'static str> {
        match self {
            ChildLane::OpenAgents => None,
            ChildLane::Opencode { .. } => Some("opencode"),
            ChildLane::Devin => Some("devin"),
            ChildLane::Claude => Some("claude"),
            ChildLane::Codex => Some("codex"),
        }
    }
}

/// How a delegated child is configured.
///
/// The four `--child-*` flags, resolved once with their environment fallbacks
/// so the values a child is started with are decided in one place instead of
/// re-read at each spawn site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChildOptions {
    /// Run children on this model instead of the lane's own.
    pub model: Option<String>,
    /// The harness binary that runs a child.
    pub command: Option<String>,
    /// A harness config file, passed as `OPENCODE_CONFIG`. This is how a
    /// provider credential reaches a child without the CLI storing it.
    pub config: Option<String>,
    /// Make children ask before using a tool.
    pub ask: bool,
}

impl ChildOptions {
    /// Flags first, then the environment, then nothing.
    pub fn resolve(
        model: Option<String>,
        command: Option<String>,
        config: Option<String>,
        ask: bool,
    ) -> Self {
        let from_env = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        Self {
            model: model.or_else(|| from_env("OPENAGENTS_DELEGATE_MODEL")),
            command: command.or_else(|| from_env("OPENAGENTS_DELEGATE_COMMAND")),
            config,
            ask,
        }
    }

    /// The environment additions a child is started with.
    ///
    /// `OPENCODE_CONFIG` is the whole point of `--child-config`: the harness
    /// reads its provider credential from that file, so the credential reaches
    /// the child without ever passing through this CLI's own storage.
    pub fn child_env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if let Some(config) = &self.config {
            env.push(("OPENCODE_CONFIG".to_string(), config.clone()));
        }
        env
    }

    /// Refuse a flag the chosen lane cannot honour.
    ///
    /// A lane that quietly ignored `--child-model` or `--child-ask` would be
    /// the same lie as a flag that is never read: the reader asked for a model
    /// or for a dry run and got neither, with nothing said.
    pub fn check(&self, lane: &ChildLane) -> Result<(), String> {
        if self.model.is_some() {
            match lane {
                ChildLane::Claude | ChildLane::Codex | ChildLane::Opencode { .. } => {}
                other => {
                    return Err(format!(
                        "--child-model cannot be honoured on the {} lane: its model is pinned by \
                         the grant the server issues, not chosen here. Use a claude, codex, or \
                         opencode/<model> lane.",
                        other.label()
                    ));
                }
            }
        }
        if self.ask {
            match lane {
                ChildLane::Claude | ChildLane::Codex => {}
                other => {
                    return Err(format!(
                        "--child-ask cannot be honoured on the {} lane: it has no \
                         ask-before-a-tool mode this command can select. Use a claude or codex \
                         lane.",
                        other.label()
                    ));
                }
            }
        }
        if self.config.is_some() {
            match lane {
                ChildLane::Opencode { .. } | ChildLane::Claude | ChildLane::Codex => {}
                other => {
                    return Err(format!(
                        "--child-config cannot be honoured on the {} lane: it runs in this \
                         process and reads no harness config file.",
                        other.label()
                    ));
                }
            }
        }
        Ok(())
    }
}

pub struct DelegationSupervisor {
    pub count: usize,
    pub lane: String,
    pub user_token: Option<String>,
    /// How much of a directory each child gets to itself.
    pub isolation: Isolation,
    /// How many children run at once. Defaults to all of them.
    pub max_parallel: usize,
    /// Leave the children's worktrees on disk when the fan-out is over, so
    /// what they wrote can be read or merged.
    pub keep_workspaces: bool,
    /// Where children work. `None` means the current directory.
    pub directory: Option<PathBuf>,
    /// How each child is configured.
    pub child: ChildOptions,
}

impl DelegationSupervisor {
    pub fn new(count: usize, lane: &str, user_token: Option<String>) -> Self {
        let count = count.clamp(1, MAX_DELEGATE_COUNT);
        Self {
            count,
            lane: lane.to_string(),
            user_token,
            isolation: Isolation::Worktree,
            max_parallel: count,
            keep_workspaces: false,
            directory: None,
            child: ChildOptions::default(),
        }
    }

    pub fn with_isolation(mut self, isolation: Isolation) -> Self {
        self.isolation = isolation;
        self
    }

    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel.clamp(1, self.count);
        self
    }

    pub fn keeping_workspaces(mut self, keep: bool) -> Self {
        self.keep_workspaces = keep;
        self
    }

    /// Where children work. `--dir`.
    pub fn in_directory(mut self, directory: Option<PathBuf>) -> Self {
        self.directory = directory;
        self
    }

    pub fn with_child_options(mut self, child: ChildOptions) -> Self {
        self.child = child;
        self
    }

    /// Run the fan-out and return every child's outcome.
    ///
    /// Convenience over [`DelegationSupervisor::dispatch_streaming`] for a
    /// caller that has nowhere to stream to.
    pub async fn dispatch(&self, prompt: &str) -> Vec<ChildWorkerResult> {
        let (events, mut drain) = mpsc::unbounded_channel();
        let sink = tokio::spawn(async move { while drain.recv().await.is_some() {} });
        let (_stop, cancel) = watch::channel(false);
        let results = self.dispatch_streaming(prompt, events, cancel).await;
        let _ = sink.await;
        results.unwrap_or_default()
    }

    /// Where children will work, or why there is nowhere for them to.
    ///
    /// `--dir` names it. It has to exist before a worktree can be prepared
    /// under it, so a path that is not a directory is a refusal rather than a
    /// fan-out that silently ran somewhere else.
    fn workdir(&self) -> Result<PathBuf, String> {
        match &self.directory {
            Some(directory) => {
                if !directory.is_dir() {
                    return Err(format!(
                        "{} is not a directory, so there is nowhere for the children to work.",
                        directory.display()
                    ));
                }
                Ok(directory.clone())
            }
            None => Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        }
    }

    /// Work out where every child will go, without building anything yet.
    ///
    /// Separate from the dispatch so a caller can read
    /// [`WorkspacePlan::isolation`] — the isolation children will *actually*
    /// get — before it announces one. `oa delegate` printed the isolation it
    /// was asked for, which outside a git checkout was `worktree` over a run
    /// that gave each child a plain empty directory.
    ///
    /// Nothing is written to disk here; the plan only names a base directory.
    /// [`WorkspacePlan::prepare`] is what creates anything.
    pub async fn plan(&self) -> Result<WorkspacePlan, String> {
        Ok(WorkspacePlan::resolve(self.workdir()?, self.isolation).await)
    }

    /// Run the fan-out, reporting each child as it goes.
    ///
    /// Returns `Err` only when no child could be started at all — a workspace
    /// that could not be prepared. A child that fails is a result with
    /// `success: false`, because the other children's answers are still worth
    /// having.
    pub async fn dispatch_streaming(
        &self,
        prompt: &str,
        events: mpsc::UnboundedSender<ChildEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<ChildWorkerResult>, String> {
        let plan = self.plan().await?;
        self.dispatch_with_plan(plan, prompt, events, cancel).await
    }

    /// The same fan-out, over a plan the caller already resolved.
    ///
    /// A caller that reported the isolation runs the plan it reported, rather
    /// than resolving a second one that could answer differently.
    pub async fn dispatch_with_plan(
        &self,
        plan: WorkspacePlan,
        prompt: &str,
        events: mpsc::UnboundedSender<ChildEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<ChildWorkerResult>, String> {
        let lane = ChildLane::parse(&self.lane)?;
        let workspaces = plan.prepare(self.count).await?;

        let gate = Arc::new(Semaphore::new(self.max_parallel.max(1)));
        let mut handles = Vec::with_capacity(self.count);

        for workspace in workspaces.clone() {
            let task = ChildWorkerTask {
                id: workspace.id,
                prompt: identify(prompt, workspace.id, self.count),
                lane: self.lane.clone(),
                worktree_path: Some(workspace.path.clone()),
            };
            let lane = lane.clone();
            let token = self.user_token.clone();
            let events = events.clone();
            let cancel = cancel.clone();
            let gate = Arc::clone(&gate);
            let child_options = self.child.clone();

            handles.push(tokio::spawn(async move {
                // The cap is here rather than around the spawn so a child that
                // is waiting for a slot still exists and still reports.
                let _slot = gate.acquire().await;
                let result = run_child(
                    task,
                    lane,
                    workspace,
                    token,
                    &child_options,
                    &events,
                    cancel,
                )
                .await;
                let _ = events.send(ChildEvent::Finished(Box::new(result.clone())));
                result
            }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                // A panic in a child's task is a failed child, not a lost one.
                Err(error) => results.push(ChildWorkerResult {
                    id: 0,
                    success: false,
                    output: format!("the child's task ended abnormally: {error}"),
                    duration_ms: 0,
                    pid: None,
                    workspace: None,
                    failure: Some(format!("the child's task ended abnormally: {error}")),
                    swarm_id: None,
                    swarm_messages: Vec::new(),
                }),
            }
        }
        results.sort_by_key(|result| result.id);

        if !self.keep_workspaces {
            for workspace in &workspaces {
                if let Some(problem) = workspace.release().await {
                    let _ = events.send(ChildEvent::Activity {
                        id: workspace.id,
                        text: problem,
                    });
                }
            }
        }

        Ok(results)
    }
}

/// Tell a child which of the fan-out it is.
///
/// Every child gets the same prompt, so a prompt that says "your own file"
/// otherwise has no way to mean anything and the whole fleet writes the same
/// one. A single child is told nothing, because there is nothing to
/// distinguish.
pub fn identify(prompt: &str, index: usize, count: usize) -> String {
    if count == 1 {
        return prompt.to_string();
    }
    format!("You are child {index} of {count}.\n\n{prompt}")
}

async fn run_child(
    task: ChildWorkerTask,
    lane: ChildLane,
    workspace: ChildWorkspace,
    user_token: Option<String>,
    options: &ChildOptions,
    events: &mpsc::UnboundedSender<ChildEvent>,
    cancel: watch::Receiver<bool>,
) -> ChildWorkerResult {
    let start = Instant::now();
    let id = task.id;

    // The child joins the local swarm for the fan-out's duration, so the
    // parent (and any other tab) can address it while it works, and `swarm
    // tree` shows the fan-out as what it is. The registration is best-effort:
    // a swarm that cannot see one child is degraded, not broken.
    let swarm_home = crate::session_store::default_root();
    let swarm_id = format!(
        "delegate-{id}-{}",
        workspace.path.to_string_lossy().replace('/', "_")
    );
    let swarm_registration = crate::swarm::Registration {
        schema: crate::swarm::REGISTRATION_SCHEMA.to_string(),
        session_id: swarm_id.clone(),
        pid: std::process::id(),
        cwd: workspace.path.display().to_string(),
        lane: lane.label().to_string(),
        model: None,
        role: "child".to_string(),
        parent: None,
        worktree: Some(workspace.path.display().to_string()),
        inbox: workspace.path.join("inbox.jsonl").display().to_string(),
        alive_after_ms: crate::swarm::DEFAULT_ALIVE_AFTER_MS,
        started_at_ms: crate::swarm::now_ms(),
        heartbeat_at_ms: crate::swarm::now_ms(),
    };
    if let Err(why) = crate::swarm::register(&swarm_home, &swarm_registration) {
        let _ = events.send(ChildEvent::Activity {
            id,
            text: format!("swarm registration failed: {why}"),
        });
    }

    let outcome = match &lane {
        ChildLane::OpenAgents => {
            let _ = events.send(ChildEvent::Started {
                id,
                lane: lane.label(),
                workspace: workspace.describe(),
                pid: None,
            });
            run_proxy_child(&task, &workspace, user_token, events, cancel).await
        }
        ChildLane::Devin => {
            run_devin_child(&task, &lane, &workspace, options, events, cancel).await
        }
        ChildLane::Claude | ChildLane::Codex | ChildLane::Opencode { .. } => {
            run_cli_child(&task, &lane, &workspace, options, events, cancel).await
        }
    };

    let duration_ms = start.elapsed().as_millis();

    // The child's registration is removed by its own exit, so read the mail
    // it received *now*, before the workspace (and its inbox) can go away.
    // This is the swarm exchange the report will show: what reached the
    // child while it worked, which is otherwise invisible.
    let swarm_id = format!(
        "delegate-{id}-{}",
        workspace.path.to_string_lossy().replace('/', "_")
    );
    let child_directory = workspace.path.clone();
    let swarm_messages = crate::swarm::Mailbox::at(&child_directory)
        .messages()
        .unwrap_or_default()
        .into_iter()
        .map(|message| (message.from, message.kind, message.body))
        .collect::<Vec<_>>();
    let _ = crate::swarm::unregister(&swarm_home, &swarm_id);

    match outcome {
        Ok(ChildAnswer { text, pid }) => ChildWorkerResult {
            id,
            success: true,
            output: clip(&text),
            duration_ms,
            pid,
            workspace: Some(workspace.path.clone()),
            failure: None,
            swarm_id: Some(swarm_id),
            swarm_messages,
        },
        Err(ChildFailure { why, pid }) => ChildWorkerResult {
            id,
            success: false,
            output: why.clone(),
            duration_ms,
            pid,
            workspace: Some(workspace.path.clone()),
            failure: Some(why),
            swarm_id: Some(swarm_id),
            swarm_messages,
        },
    }
}

struct ChildAnswer {
    text: String,
    pid: Option<u32>,
}

struct ChildFailure {
    why: String,
    pid: Option<u32>,
}

/// A child on this process's own runtime, over the inference proxy.
///
/// It gets a tool registry rooted at its own directory, so its `shell` tool
/// runs there and what it writes lands there. It does not get the `delegate`
/// tool: a fan-out whose children fan out is a fan-out with no ceiling.
async fn run_proxy_child(
    task: &ChildWorkerTask,
    workspace: &ChildWorkspace,
    user_token: Option<String>,
    events: &mpsc::UnboundedSender<ChildEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<ChildAnswer, ChildFailure> {
    let tools = HarnessToolRegistry::child(Some(workspace.path.clone()));
    // The default lane, which resolves its model from the catalog. This used
    // to name `ox-alpha` directly, which is exactly the shape that goes stale:
    // `ChildLane::OpenAgents` is a harness name meaning "in this process on the
    // OpenAgents proxy", and spending it as a model id tied every delegated
    // child to one model's continued existence. The OpenAgents harness now
    // asks the deployment for its default model instead of assuming one.
    let mut runtime = CoderRuntimeSession::new(Lane::default(), None, user_token, tools);

    let id = task.id;
    let sink = events.clone();

    let outcome = {
        let turn = runtime.execute_turn(&task.prompt, move |chunk| {
            let _ = sink.send(ChildEvent::Output {
                id,
                text: chunk.to_string(),
            });
        });

        tokio::select! {
            biased;
            _ = cancel.changed() => Err("stopped before finishing".to_string()),
            answered = turn => answered.map_err(|error| error.to_string()),
        }
    };

    // A cancelled child drops its turn mid-flight, so the turn's own failure
    // path never runs and the transcript would simply stop. Say what happened
    // before the thread is revoked, so a stopped child is not left looking
    // like one that finished quietly.
    if let Err(why) = &outcome {
        runtime.note_interruption(why).await;
    }
    // Awaited, on every path. A child used to leave its thread to the `Drop`
    // impl, which spawns a best-effort ending the process may never poll — and
    // a thread left open holds its grant's remaining budget. It ends by
    // reporting what it did: a child that answered is not a cancellation, and a
    // child that was stopped reports `cancelled` with `interrupted` because
    // `note_interruption` above settled that. The failure is reported to the
    // parent's event stream rather than to a screen this child does not own.
    if let Err(error) = runtime.finish().await {
        let _ = events.send(ChildEvent::Activity {
            id,
            text: format!("the thread was not ended: {error}"),
        });
    }
    for failure in &runtime.record_failures {
        let _ = events.send(ChildEvent::Activity {
            id,
            text: failure.clone(),
        });
    }

    match outcome {
        Ok(text) => Ok(ChildAnswer { text, pid: None }),
        Err(why) => Err(ChildFailure { why, pid: None }),
    }
}

/// A child on the Devin CLI, over the Agent Client Protocol.
async fn run_devin_child(
    task: &ChildWorkerTask,
    lane: &ChildLane,
    workspace: &ChildWorkspace,
    options: &ChildOptions,
    events: &mpsc::UnboundedSender<ChildEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<ChildAnswer, ChildFailure> {
    let id = task.id;
    let _ = events.send(ChildEvent::Started {
        id,
        lane: lane.label(),
        workspace: workspace.describe(),
        pid: None,
    });

    let harness = AcpHarness {
        command: options
            .command
            .clone()
            .unwrap_or_else(|| harness_binary(lane)),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let sink = events.clone();
    let answered = harness
        .run(
            &task.prompt,
            &workspace.path,
            move |event| {
                let text = match event {
                    AcpEvent::Session { id: session } => format!("session {session}"),
                    AcpEvent::Tool { kind, title } => {
                        if title.is_empty() {
                            kind
                        } else {
                            title
                        }
                    }
                    AcpEvent::Tokens { input, output } => {
                        format!("{input} in / {output} out tokens")
                    }
                    AcpEvent::Text { chunk } => {
                        let _ = sink.send(ChildEvent::Output { id, text: chunk });
                        return;
                    }
                };
                let _ = sink.send(ChildEvent::Activity { id, text });
            },
            &mut cancel,
        )
        .await;

    match answered {
        Ok(text) => Ok(ChildAnswer { text, pid: None }),
        Err(AcpFailure::Cancelled) => Err(ChildFailure {
            why: "stopped before finishing".to_string(),
            pid: None,
        }),
        Err(other) => Err(ChildFailure {
            why: other.to_string(),
            pid: None,
        }),
    }
}

/// A child on another coding CLI: `claude`, `codex`, or `opencode`.
///
/// The child is spawned into a process group of its own, its two output
/// streams are read as they are written rather than at the end, and both are
/// forwarded upward line by line. The answer is pulled out of the harness's
/// own event stream where the harness has one, and is the tail of what it
/// printed where it does not.
async fn run_cli_child(
    task: &ChildWorkerTask,
    lane: &ChildLane,
    workspace: &ChildWorkspace,
    options: &ChildOptions,
    events: &mpsc::UnboundedSender<ChildEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<ChildAnswer, ChildFailure> {
    let id = task.id;
    let (command, args) = harness_command(lane, &task.prompt, &workspace.path, options);

    let mut spawn = Command::new(&command);
    spawn
        .args(&args)
        .envs(options.child_env())
        .current_dir(&workspace.path)
        // No terminal, so a harness that would prompt gets end-of-file rather
        // than a wait nobody can see.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Its own process group, so cancelling a fan-out stops what the children
    // started. `process_group` is a Unix extension and is absent on Windows.
    #[cfg(unix)]
    {
        spawn.process_group(0);
    }
    let mut child = match spawn.spawn() {
        Ok(child) => child,
        Err(error) => {
            let why = if error.kind() == std::io::ErrorKind::NotFound {
                format!("the `{command}` command is not on PATH")
            } else {
                format!("the `{command}` command would not start: {error}")
            };
            let _ = events.send(ChildEvent::Started {
                id,
                lane: lane.label(),
                workspace: workspace.describe(),
                pid: None,
            });
            return Err(ChildFailure { why, pid: None });
        }
    };

    let pid = child.id();
    let _ = events.send(ChildEvent::Started {
        id,
        lane: lane.label(),
        workspace: workspace.describe(),
        pid,
    });

    // Both streams are drained by their own task and merged here, so neither
    // can fill its pipe and stall the child while the other is being read.
    let (lines_tx, mut lines_rx) = mpsc::unbounded_channel::<(bool, String)>();
    if let Some(stdout) = child.stdout.take() {
        let tx = lines_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send((true, line)).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = lines_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send((false, line)).is_err() {
                    break;
                }
            }
        });
    }
    drop(lines_tx);

    let mut harvest = Harvest::new(lane);
    let mut stopped = false;

    loop {
        tokio::select! {
            biased;
            _ = cancel.changed() => {
                stopped = true;
                stop_tree(&mut child).await;
                break;
            }
            line = lines_rx.recv() => {
                match line {
                    Some((from_stdout, line)) => {
                        if from_stdout {
                            // A harness with an event stream is forwarded as
                            // what it said, not as its wire format: five lines
                            // of `{"type":"assistant","message":{"content":…`
                            // is not a reader watching a child work.
                            for rendered in harvest.take(&line) {
                                let _ = events.send(match rendered {
                                    Rendered::Text(text) => ChildEvent::Output { id, text },
                                    Rendered::Note(text) => ChildEvent::Activity { id, text },
                                });
                            }
                        } else {
                            let _ = events.send(ChildEvent::Activity { id, text: line.clone() });
                            harvest.note_stderr(&line);
                        }
                    }
                    // Both streams are closed, so the child has finished
                    // writing even if it has not yet been reaped.
                    None => break,
                }
            }
        }
    }

    if stopped {
        return Err(ChildFailure {
            why: "stopped before finishing".to_string(),
            pid,
        });
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            return Err(ChildFailure {
                why: format!("the `{command}` child could not be reaped: {error}"),
                pid,
            });
        }
    };

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(ChildFailure {
            why: format!(
                "the `{command}` child exited with code {code}.\n\n{}",
                harvest.tail()
            ),
            pid,
        });
    }

    if let Some(reported) = harvest.reported_error() {
        return Err(ChildFailure {
            why: format!("the `{command}` child reported an error: {reported}"),
            pid,
        });
    }

    Ok(ChildAnswer {
        text: harvest.answer(),
        pid,
    })
}

/// The binary a lane runs, or the stand-in a test points it at.
///
/// The TypeScript harnesses each take a `command` for the same reason: a test
/// that cannot substitute the agent can only assert against a real one, which
/// means it either costs money or does not run.
pub fn harness_binary(lane: &ChildLane) -> String {
    let (variable, default) = match lane {
        ChildLane::Claude => ("OA_CHILD_CLAUDE", "claude"),
        ChildLane::Codex => ("OA_CHILD_CODEX", "codex"),
        ChildLane::Opencode { .. } => ("OA_CHILD_OPENCODE", "opencode"),
        ChildLane::Devin => ("OA_CHILD_DEVIN", "devin"),
        ChildLane::OpenAgents => ("", ""),
    };
    std::env::var(variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// The binary and arguments each CLI lane runs, following `coder-delegate.ts`.
///
/// `options` is where the four `--child-*` flags land: the binary comes from
/// `--child-command` when one was given, `--child-model` replaces the lane's
/// model, and `--child-ask` selects the harness's own ask-before-a-tool mode
/// instead of the mode a child with nobody to ask normally runs in.
pub fn harness_command(
    lane: &ChildLane,
    prompt: &str,
    cwd: &std::path::Path,
    options: &ChildOptions,
) -> (String, Vec<String>) {
    let binary = options
        .command
        .clone()
        .unwrap_or_else(|| harness_binary(lane));
    match lane {
        ChildLane::Claude => {
            let mut args = vec![
                "-p".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
                // `stream-json` requires it.
                "--verbose".to_string(),
                "--permission-mode".to_string(),
                // A delegated child normally has nobody to ask; `--child-ask`
                // is the dry run that stops it at its first edit instead.
                if options.ask {
                    "default".to_string()
                } else {
                    "acceptEdits".to_string()
                },
            ];
            if let Some(model) = &options.model {
                args.push("--model".to_string());
                args.push(model.clone());
            }
            (binary, args)
        }
        ChildLane::Codex => {
            let mut args = vec![
                "exec".to_string(),
                "--json".to_string(),
                // A child's worktree is a checkout but not one Codex has been
                // told to trust, and without this it refuses before it starts.
                "--skip-git-repo-check".to_string(),
                "--sandbox".to_string(),
                // The child may edit the checkout it was pointed at and
                // nothing outside it — unless it was asked to touch nothing.
                if options.ask {
                    "read-only".to_string()
                } else {
                    "workspace-write".to_string()
                },
            ];
            if let Some(model) = &options.model {
                args.push("--model".to_string());
                args.push(model.clone());
            }
            args.push(prompt.to_string());
            (binary, args)
        }
        ChildLane::Opencode { model } => (
            binary,
            vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--model".to_string(),
                options.model.clone().unwrap_or_else(|| model.clone()),
                "--dir".to_string(),
                cwd.to_string_lossy().to_string(),
                prompt.to_string(),
            ],
        ),
        // Handled by their own runners; unreachable through this function.
        ChildLane::OpenAgents => (String::new(), Vec::new()),
        ChildLane::Devin => (binary, vec!["acp".to_string()]),
    }
}

/// Pulls a child's answer out of whatever its harness prints.
///
/// A harness with a JSON event stream is read as one; anything that is not
/// JSON, or is JSON of a shape this does not know, is kept as text. So a
/// harness that changes its schema degrades to "the tail of what it printed"
/// rather than to an empty answer that reads like a child with nothing to say.
/// What one line of a child's output is: its answer, or what it is doing.
enum Rendered {
    Text(String),
    Note(String),
}

struct Harvest {
    lane: ChildLane,
    assistant: String,
    result: Option<String>,
    error: Option<String>,
    plain: String,
    stderr_tail: String,
}

impl Harvest {
    fn new(lane: &ChildLane) -> Self {
        Self {
            lane: lane.clone(),
            assistant: String::new(),
            result: None,
            error: None,
            plain: String::new(),
            stderr_tail: String::new(),
        }
    }

    /// Read one line and say what the reader should see of it.
    fn take(&mut self, line: &str) -> Vec<Rendered> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        // Not the harness's event stream. Whatever it is, it is what the child
        // printed, so it is passed through as written.
        let Ok(event) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            push_bounded(&mut self.plain, line);
            return vec![Rendered::Text(format!("{line}\n"))];
        };

        let mut shown: Vec<Rendered> = Vec::new();

        // Claude Code print mode: `assistant` messages carry the text and the
        // last event carries the whole result.
        if let Some(text) = event
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for part in text {
                if let Some(said) = part.get("text").and_then(|v| v.as_str()) {
                    self.assistant.push_str(said);
                    shown.push(Rendered::Text(said.to_string()));
                }
                // A tool call is what the child is doing, not what it said.
                if part.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    shown.push(Rendered::Note(format!(
                        "tool {}",
                        part.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                    )));
                }
            }
        }
        if let Some(result) = event.get("result").and_then(|v| v.as_str()) {
            self.result = Some(result.to_string());
        }
        if event.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
            self.error = Some(
                event
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("the harness reported an error")
                    .to_string(),
            );
        }

        // Codex exec: the final assistant message arrives as an item.
        if let Some(item) = event.get("item") {
            match item.get("type").and_then(|v| v.as_str()) {
                Some("agent_message") => {
                    if let Some(said) = item.get("text").and_then(|v| v.as_str()) {
                        self.assistant.push_str(said);
                        shown.push(Rendered::Text(format!("{said}\n")));
                    }
                }
                Some("command_execution") => {
                    if let Some(command) = item.get("command").and_then(|v| v.as_str()) {
                        shown.push(Rendered::Note(format!("ran {command}")));
                    }
                }
                _ => {}
            }
        }
        // Codex's older wire shape, and opencode's.
        if let Some(msg) = event.get("msg") {
            if let Some(said) = msg.get("message").and_then(|v| v.as_str()) {
                self.assistant.push_str(said);
                shown.push(Rendered::Text(format!("{said}\n")));
            }
            if msg.get("type").and_then(|v| v.as_str()) == Some("error") {
                if let Some(said) = msg.get("message").and_then(|v| v.as_str()) {
                    self.error = Some(said.to_string());
                }
            }
        }
        if let Some(said) = event
            .get("parts")
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .filter(|said| !said.is_empty())
        {
            self.assistant.push_str(&said);
            shown.push(Rendered::Text(said));
        }

        if shown.is_empty() {
            // A known wire format, an event this does not render. Named rather
            // than dropped, so a silent stretch is a silent child and not a
            // parser looking the other way.
            if let Some(kind) = event
                .get("type")
                .or_else(|| event.get("msg").and_then(|m| m.get("type")))
                .and_then(|v| v.as_str())
            {
                if kind != "result" {
                    shown.push(Rendered::Note(kind.to_string()));
                }
            }
        }
        shown
    }

    fn note_stderr(&mut self, line: &str) {
        push_bounded(&mut self.stderr_tail, line);
    }

    fn reported_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn answer(&self) -> String {
        let said = self
            .result
            .clone()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| Some(self.assistant.clone()).filter(|text| !text.trim().is_empty()))
            .or_else(|| Some(self.plain.clone()).filter(|text| !text.trim().is_empty()));
        match said {
            Some(text) => text.trim().to_string(),
            None => format!(
                "The {} child finished and printed no answer this harness could read.",
                match self.lane {
                    ChildLane::Claude => "claude",
                    ChildLane::Codex => "codex",
                    ChildLane::Opencode { .. } => "opencode",
                    ChildLane::Devin => "devin",
                    ChildLane::OpenAgents => "openagents",
                }
            ),
        }
    }

    fn tail(&self) -> String {
        let mut both = String::new();
        if !self.plain.trim().is_empty() {
            both.push_str(self.plain.trim());
        }
        if !self.stderr_tail.trim().is_empty() {
            if !both.is_empty() {
                both.push('\n');
            }
            both.push_str(self.stderr_tail.trim());
        }
        if both.is_empty() {
            "The child printed nothing.".to_string()
        } else {
            both
        }
    }
}

/// Keep the last [`CHILD_RESULT_LIMIT`] characters, so an hour of build output
/// cannot grow without bound in memory.
fn push_bounded(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push('\n');
    if buffer.len() > CHILD_RESULT_LIMIT * 2 {
        let keep = buffer.len() - CHILD_RESULT_LIMIT;
        let at = buffer
            .char_indices()
            .map(|(at, _)| at)
            .find(|at| *at >= keep)
            .unwrap_or(0);
        buffer.drain(..at);
    }
}

/// A child's answer, cut to what the reader is shown.
///
/// The cut says its own size. A child that reported ten findings and was shown
/// as three reads exactly like a child that found three.
fn clip(text: &str) -> String {
    if text.len() <= CHILD_RESULT_LIMIT {
        return text.to_string();
    }
    let mut at = CHILD_RESULT_LIMIT;
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    format!(
        "{}\n…[{} of {} characters cut from the end of this child's answer]",
        &text[..at],
        text.len() - at,
        text.len()
    )
}

/// Turns the event stream into prefixed lines on standard output.
///
/// A child's output arrives in whatever pieces the harness or the model
/// produced it in, which for a streamed model is a few characters at a time.
/// Each child gets its own buffer so a prefix is printed once per line rather
/// than once per chunk, and two children writing at once do not interleave
/// mid-word.
struct Printer {
    pending: std::collections::BTreeMap<usize, String>,
}

impl Printer {
    fn new() -> Self {
        Self {
            pending: std::collections::BTreeMap::new(),
        }
    }

    fn feed(&mut self, id: usize, text: &str) {
        let buffer = self.pending.entry(id).or_default();
        buffer.push_str(text);
        while let Some(at) = buffer.find('\n') {
            let line: String = buffer.drain(..=at).collect();
            println!("[child {id}] {}", line.trim_end_matches('\n'));
        }
    }

    fn flush(&mut self, id: usize) {
        if let Some(buffer) = self.pending.get_mut(&id) {
            if !buffer.trim().is_empty() {
                println!("[child {id}] {}", buffer.trim_end());
            }
            buffer.clear();
        }
    }
}

/// `oa coder --delegate`.
/// What a fan-out was asked for, whichever command asked.
///
/// `oa delegate` and `oa coder --delegate` run the same engine, so they resolve
/// to the same request rather than each carrying its own copy of the argument
/// handling. Both commands now declare the `--child-*` flags, so a fan-out
/// started from either can be given a harness, a model, and a config file;
/// `--dir` and `--description` remain `oa delegate`'s alone, which is why they
/// are `None` on the coder side rather than silently defaulted.
#[derive(Debug, Clone)]
pub struct DelegationRequest {
    pub prompt: Option<String>,
    pub count: usize,
    pub max_parallel: Option<usize>,
    pub lane: Option<String>,
    pub isolation: Option<String>,
    pub keep_workspaces: bool,
    pub directory: Option<String>,
    pub description: Option<String>,
    pub child_model: Option<String>,
    pub child_command: Option<String>,
    pub child_config: Option<String>,
    pub child_ask: bool,
}

impl DelegationRequest {
    pub fn from_coder(args: CoderArgs) -> Self {
        Self {
            prompt: args.prompt,
            count: args.count,
            max_parallel: args.max_parallel,
            lane: args.lane,
            isolation: args.isolation,
            keep_workspaces: args.keep_workspaces,
            directory: None,
            description: None,
            child_model: args.child_model,
            child_command: args.child_command,
            child_config: args.child_config,
            child_ask: args.child_ask,
        }
    }

    pub fn from_delegate(args: DelegateArgs) -> Self {
        Self {
            prompt: args.prompt,
            count: args.agents,
            max_parallel: args.concurrency,
            lane: args.lane,
            isolation: args.isolation,
            keep_workspaces: args.keep_workspaces,
            directory: args.dir,
            description: args.description,
            child_model: args.child_model,
            child_command: args.child_command,
            child_config: args.child_config,
            child_ask: args.child_ask,
        }
    }
}

/// Three to five words naming the task, from `--description` or the prompt.
///
/// Mirrors `describePrompt` in `coder-delegate.ts`: the first words of the
/// prompt, so a fan-out with no description is still named by what it does.
pub fn describe(description: Option<&str>, prompt: &str) -> String {
    if let Some(given) = description {
        let trimmed = given.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let words: Vec<&str> = prompt.split_whitespace().take(5).collect();
    if words.is_empty() {
        return "delegated task".to_string();
    }
    words.join(" ")
}

pub async fn run_delegation(
    args: DelegationRequest,
    user_token: Option<String>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Per-child progress is a running commentary, not the answer. Under
    // `--json` the answer is the one document at the end, so the commentary
    // moves to stderr — which is what the TypeScript CLI does with it too
    // (`cli.ts:3025`, where the `--json` path unsubscribes the printer).
    let say = move |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    let requested = args.count.max(1);
    if requested > MAX_DELEGATE_COUNT {
        fail(&format!(
            "{requested} children were asked for and this command runs at most {MAX_DELEGATE_COUNT}."
        ));
    }
    // Refused before anything is prepared, spawned, or billed. A missing
    // prompt used to become the literal `Analyze workspace and run tests`,
    // which every child then ran: N `git worktree add`s on disk and, on the
    // default lane, N threads and N grants spent on an instruction nobody
    // gave. `describe()` even named the run "delegated task", so the header
    // read plausibly while it happened.
    //
    // The same operation exposed as a model tool already refuses this:
    // `tools.rs` answers `No children were started: \`prompt\` is required and
    // must say what the child does.`, and trims before it decides. A CLI more
    // permissive than the model-facing surface is backwards, so both now
    // answer the omission the same way — whitespace included, which is the
    // same omission with a space in it.
    let Some(prompt) = args
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|given| !given.is_empty())
        .map(str::to_string)
    else {
        fail(
            "a delegation runs one prompt in every child, and there is nothing to run \
             without one. Give it one: `oa delegate \"<prompt>\"`, or \
             `oa coder --delegate \"<prompt>\"`",
        );
    };
    // The lane decides who pays. Naming none is not refused — `oa coder`
    // defaults its lane too, and a fan-out with no lane is a reasonable
    // thing to ask for — but it is said out loud, here, before a child
    // exists. The header below names the lane either way and so cannot
    // distinguish a lane that was chosen from one that was assumed.
    let lane_name = match args.lane.clone() {
        Some(named) => named,
        None => {
            say(format!(
                "No --lane given, so children run on {DEFAULT_CHILD_LANE}, which opens a \
                 thread per child and spends this account's grant. Name another with --lane."
            ));
            DEFAULT_CHILD_LANE.to_string()
        }
    };

    let lane = ChildLane::parse(&lane_name).unwrap_or_else(|problem| fail(&problem));
    if let Some(current) = ChildLane::renamed_alias(&lane_name) {
        say(format!(
            "The {lane_name} delegation lane was renamed to {current}. Using {current}."
        ));
    }

    let asked_isolation = match args.isolation.as_deref() {
        None => Isolation::Worktree,
        Some(named) => match Isolation::parse(named) {
            Some(isolation) => isolation,
            None => fail(&format!(
                "`{named}` is not an isolation this command knows. Use worktree, directory, or none."
            )),
        },
    };

    let child = ChildOptions::resolve(
        args.child_model.clone(),
        args.child_command.clone(),
        args.child_config.clone(),
        args.child_ask,
    );
    // A flag the chosen lane cannot honour ends the command. Running anyway
    // would give the reader the fan-out they asked for without the model, the
    // config, or the dry run they asked for it with.
    if let Err(why) = child.check(&lane) {
        fail(&why);
    }

    let description = describe(args.description.as_deref(), &prompt);

    let supervisor = DelegationSupervisor::new(requested, &lane_name, user_token)
        .with_isolation(asked_isolation)
        .with_max_parallel(args.max_parallel.unwrap_or(requested))
        .keeping_workspaces(args.keep_workspaces)
        .in_directory(args.directory.as_deref().map(PathBuf::from))
        .with_child_options(child);

    // Resolved before the header, and the header reports the resolution.
    // `worktree` outside a git checkout is a plain empty directory per child;
    // this used to print the value that was asked for, so a run announced an
    // isolation it did not have and nothing later said otherwise. The plan is
    // then handed to the dispatch, so what was reported is what runs.
    let plan = match supervisor.plan().await {
        Ok(plan) => plan,
        Err(error) => fail(&format!("no children were started: {error}")),
    };
    let isolation = plan.isolation();
    if isolation != asked_isolation {
        say(format!(
            "`{}` isolation is not available here — this is not a git checkout — so children get \
             `{}` instead.",
            asked_isolation.name(),
            isolation.name(),
        ));
    }

    say(format!(
        "Delegating {}: {} {} on {}, {} at a time, isolation: {}.",
        description,
        supervisor.count,
        if supervisor.count == 1 {
            "child"
        } else {
            "children"
        },
        lane.label(),
        supervisor.max_parallel,
        isolation.name(),
    ));
    if let Some(directory) = &supervisor.directory {
        say(format!("Children work under {}.", directory.display()));
    }

    // `ctrl+c` is the only stop signal a running fan-out has. Without it a
    // reader who changed their mind had to kill the terminal, and the
    // children — which are their own process groups — carried on spending.
    let (stop, cancel) = watch::channel(false);
    let interrupt = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\nStopping the fan-out; children are being signalled.");
            let _ = stop.send(true);
        }
    });

    let (events, mut incoming) = mpsc::unbounded_channel();
    let printing = tokio::spawn(async move {
        let mut printer = Printer::new();
        while let Some(event) = incoming.recv().await {
            match event {
                ChildEvent::Started {
                    id,
                    lane,
                    workspace,
                    pid,
                } => {
                    say(format!(
                        "[child {id}] started on {lane} in {workspace}{}",
                        match pid {
                            Some(pid) => format!(" as pid {pid}"),
                            None => " in this process".to_string(),
                        }
                    ));
                }
                ChildEvent::Output { id, text } => printer.feed(id, &text),
                ChildEvent::Activity { id, text } => {
                    printer.flush(id);
                    say(format!("[child {id}] · {text}"));
                }
                ChildEvent::Finished(result) => {
                    printer.flush(result.id);
                    say(format!(
                        "[child {}] {} after {}ms",
                        result.id,
                        if result.success { "finished" } else { "FAILED" },
                        result.duration_ms
                    ));
                }
            }
        }
    });

    let results = match supervisor
        .dispatch_with_plan(plan, &prompt, events, cancel)
        .await
    {
        Ok(results) => results,
        // No child ran at all, so there is nothing to report but the reason.
        Err(error) => fail(&format!("no children were started: {error}")),
    };
    let _ = printing.await;
    interrupt.abort();

    let succeeded = results.iter().filter(|result| result.success).count();
    if json {
        // The field names the TypeScript CLI publishes for this command
        // (`cli.ts:3070`): `agent`, `cwd`, and one entry per child. It
        // pretty-prints this one document — `JSON.stringify(…, null, 2)` —
        // where every other `--json` output it writes is compact. This matches
        // it rather than tidying it, because a consumer already parsing that
        // shape is what the flag is for.
        let document = serde_json::json!({
            "agent": lane.label(),
            "cwd": supervisor
                .directory
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            "lane": lane_name,
            "outcomes": results
                .iter()
                .map(|result| serde_json::json!({
                    "id": result.id,
                    "success": result.success,
                    "duration_ms": result.duration_ms,
                    "pid": result.pid,
                    "workspace": result
                        .workspace
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned()),
                    "failure": result.failure,
                    "swarm_id": result.swarm_id,
                    "swarm_messages": result.swarm_messages
                        .iter()
                        .map(|(from, kind, body)| serde_json::json!({
                            "from": from,
                            "kind": kind,
                            "body": body,
                        }))
                        .collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
            "succeeded": succeeded,
            "requested": results.len(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&document).unwrap_or_else(|_| document.to_string())
        );
    } else {
        println!();
        for result in &results {
            println!(
                "child {}: {} in {}ms{}{}",
                result.id,
                if result.success { "ok" } else { "failed" },
                result.duration_ms,
                match result.pid {
                    Some(pid) => format!(", pid {pid}"),
                    None => String::new(),
                },
                match &result.workspace {
                    Some(path) => format!(", in {}", path.display()),
                    None => String::new(),
                }
            );
            if let Some(why) = &result.failure {
                println!("  {why}");
            }
        }
        println!(
            "{succeeded} of {} {} completed on {}.",
            results.len(),
            if results.len() == 1 {
                "child"
            } else {
                "children"
            },
            lane.label()
        );
        let exchanged: Vec<_> = results
            .iter()
            .filter(|result| !result.swarm_messages.is_empty())
            .collect();
        if !exchanged.is_empty() {
            println!();
            println!("Messages exchanged while the fan-out ran:");
            for result in exchanged {
                for (from, kind, body) in &result.swarm_messages {
                    println!("  [child {}] {} from {}: {body}", result.id, kind, from);
                }
            }
        }
    }

    if succeeded < results.len() {
        // A fan-out that lost a child is not a command that worked. This used
        // to print `2/2 children succeeded` and exit zero whatever happened.
        // Exit 1 rather than the 2 an input error gets: the command was asked
        // for correctly and the work is what did not finish.
        eprintln!(
            "oa: {} of {} children did not finish.",
            results.len() - succeeded,
            results.len()
        );
        std::process::exit(1);
    }
    Ok(())
}

/// The `delegate` tool's fan-out, rendered for a model to read.
///
/// Awaited rather than launched and forgotten: a model told "three children
/// are running" has nothing to say next and will either invent their findings
/// or ask the reader to wait.
///
/// Returned as a boxed `Send` future on purpose. The call graph is a cycle —
/// a session runs a tool, the `delegate` tool starts a child, and the child is
/// a session that runs tools — and the compiler cannot infer `Send` around a
/// cycle: it asks whether this future is `Send` in order to answer whether it
/// is `Send`. Naming the bound here is what breaks it.
pub fn fanout_for_tool(
    prompt: &str,
    count: usize,
    lane: &str,
    user_token: Option<String>,
    child: ChildOptions,
    directory: Option<PathBuf>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    let (keep_open, cancel) = watch::channel(false);
    let future =
        fanout_for_tool_cancellable(prompt, count, lane, user_token, child, directory, cancel);
    Box::pin(async move {
        let _keep_open = keep_open;
        future.await
    })
}

/// Run the `delegate` tool under the parent turn's cancellation signal.
pub fn fanout_for_tool_cancellable(
    prompt: &str,
    count: usize,
    lane: &str,
    user_token: Option<String>,
    child: ChildOptions,
    directory: Option<PathBuf>,
    cancel: watch::Receiver<bool>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    let prompt = prompt.to_string();
    let lane = lane.to_string();
    Box::pin(async move {
        let prompt = prompt.as_str();
        let lane = lane.as_str();
        let count = count.clamp(1, MAX_DELEGATE_COUNT);
        // A flag the lane cannot honour is reported to the caller rather than
        // dropped: the session asked for a model or a dry run and would otherwise
        // get a fan-out without either, with nothing said.
        let resolved_lane = match ChildLane::resolve_for_session(lane) {
            Ok(resolved) => resolved,
            Err(why) => return format!("No children were started: {why}"),
        };
        if let Err(why) = child.check(&resolved_lane) {
            return format!("No children were started: {why}");
        }
        // Children work where the session works. The version this replaces took
        // whatever the process's own directory happened to be, which is the same
        // thing only until something changes it.
        let resolved_name = resolved_lane.name();
        let supervisor = DelegationSupervisor::new(count, &resolved_name, user_token)
            .with_child_options(child)
            .in_directory(directory);
        let (events, mut drain) = mpsc::unbounded_channel();
        let sink = tokio::spawn(async move { while drain.recv().await.is_some() {} });
        let results = supervisor
            .dispatch_streaming(prompt, events, cancel)
            .await
            .unwrap_or_default();
        let _ = sink.await;

        let succeeded = results.iter().filter(|result| result.success).count();
        let header = format!(
            "{succeeded} of {} {} completed on {}.",
            results.len(),
            if results.len() == 1 {
                "child"
            } else {
                "children"
            },
            resolved_lane.label()
        );
        let records = results
            .into_iter()
            .map(|result| crate::delegate_result::DelegateAgentResult {
                status: if result.success {
                    crate::delegate_result::DelegateStatus::Done
                } else {
                    crate::delegate_result::DelegateStatus::Failed
                },
                agent: resolved_lane.name(),
                total_tool_uses: 0,
                duration_ms: u64::try_from(result.duration_ms).unwrap_or(u64::MAX),
                total_tokens: 0,
                model: None,
                session_id: None,
                report: if result.output.trim().is_empty() {
                    format!("child {} completed with no output", result.id)
                } else {
                    result.output
                },
                worktree: result
                    .workspace
                    .map(|path| crate::delegate_result::WorktreeRef {
                        path: path.display().to_string(),
                        branch: None,
                    }),
            })
            .collect();
        crate::delegate_result::DelegateFanoutResult {
            header,
            results: records,
        }
        .to_json()
    })
}

#[cfg(test)]
mod child_option_tests {
    use super::*;
    use std::path::Path;

    fn argv(lane: &ChildLane, options: &ChildOptions) -> (String, Vec<String>) {
        harness_command(lane, "do the thing", Path::new("/tmp/work"), options)
    }

    #[test]
    fn child_events_map_onto_subagent_lines() {
        assert_eq!(
            child_event_subagent_line(&ChildEvent::Started {
                id: 1,
                lane: "devin".to_string(),
                workspace: "/tmp/work".to_string(),
                pid: Some(9),
            })
            .as_deref(),
            Some("started on devin in /tmp/work")
        );
        assert_eq!(
            child_event_subagent_line(&ChildEvent::Output {
                id: 1,
                text: "hello\n".to_string(),
            })
            .as_deref(),
            Some("hello")
        );
        assert_eq!(
            child_event_subagent_line(&ChildEvent::Activity {
                id: 1,
                text: "Read src/a.ts".to_string(),
            })
            .as_deref(),
            Some("Read src/a.ts")
        );
        assert_eq!(
            child_event_subagent_line(&ChildEvent::Finished(Box::new(ChildWorkerResult {
                id: 1,
                success: true,
                output: "done".to_string(),
                duration_ms: 10,
                pid: None,
                workspace: None,
                failure: None,
                swarm_id: None,
                swarm_messages: Vec::new(),
            }))),
            None
        );
    }

    /// `--child-command` has to change which binary a child is started as.
    #[test]
    fn child_command_replaces_the_harness_binary() {
        let default = argv(&ChildLane::Claude, &ChildOptions::default());
        let overridden = argv(
            &ChildLane::Claude,
            &ChildOptions {
                command: Some("/opt/stub-claude".to_string()),
                ..ChildOptions::default()
            },
        );
        assert_ne!(default.0, overridden.0);
        assert_eq!(overridden.0, "/opt/stub-claude");
        // Only the binary moves; the arguments are the lane's own.
        assert_eq!(default.1, overridden.1);
    }

    /// `--child-model` has to reach the harness's own model argument.
    #[test]
    fn child_model_reaches_the_harness_argument() {
        for lane in [ChildLane::Claude, ChildLane::Codex] {
            let plain = argv(&lane, &ChildOptions::default());
            assert!(
                !plain.1.iter().any(|a| a == "--model"),
                "{lane:?} named a model with none asked for"
            );
            let chosen = argv(
                &lane,
                &ChildOptions {
                    model: Some("anthropic/opus".to_string()),
                    ..ChildOptions::default()
                },
            );
            let at = chosen
                .1
                .iter()
                .position(|a| a == "--model")
                .unwrap_or_else(|| panic!("{lane:?} did not pass --model: {:?}", chosen.1));
            assert_eq!(chosen.1[at + 1], "anthropic/opus");
        }

        // The opencode lane already carries a model; the flag replaces it
        // rather than adding a second one.
        let lane = ChildLane::Opencode {
            model: "gemini-3.7-flash".to_string(),
        };
        let chosen = argv(
            &lane,
            &ChildOptions {
                model: Some("openai/gpt-5".to_string()),
                ..ChildOptions::default()
            },
        );
        assert_eq!(chosen.1.iter().filter(|a| *a == "--model").count(), 1);
        assert!(chosen.1.contains(&"openai/gpt-5".to_string()));
        assert!(!chosen.1.contains(&"gemini-3.7-flash".to_string()));
    }

    /// `--child-ask` has to select the harness's ask-before-a-tool mode.
    ///
    /// The whole point of the flag is a dry run over a directory the reader
    /// does not want touched, so the argument that lets a child edit has to be
    /// the one that changes.
    #[test]
    fn child_ask_selects_the_harness_ask_mode() {
        let asking = ChildOptions {
            ask: true,
            ..ChildOptions::default()
        };

        let claude_default = argv(&ChildLane::Claude, &ChildOptions::default()).1;
        let claude_asking = argv(&ChildLane::Claude, &asking).1;
        assert!(claude_default.contains(&"acceptEdits".to_string()));
        assert!(!claude_asking.contains(&"acceptEdits".to_string()));
        assert!(claude_asking.contains(&"default".to_string()));

        let codex_default = argv(&ChildLane::Codex, &ChildOptions::default()).1;
        let codex_asking = argv(&ChildLane::Codex, &asking).1;
        assert!(codex_default.contains(&"workspace-write".to_string()));
        assert!(!codex_asking.contains(&"workspace-write".to_string()));
        assert!(codex_asking.contains(&"read-only".to_string()));
    }

    /// `--child-config` reaches the child as `OPENCODE_CONFIG`, which is how a
    /// provider credential gets there without this CLI storing it.
    #[test]
    fn child_config_reaches_the_child_environment() {
        assert!(ChildOptions::default().child_env().is_empty());
        let options = ChildOptions {
            config: Some("/tmp/opencode.json".to_string()),
            ..ChildOptions::default()
        };
        assert_eq!(
            options.child_env(),
            vec![(
                "OPENCODE_CONFIG".to_string(),
                "/tmp/opencode.json".to_string()
            )]
        );
    }

    /// A lane that cannot honour a flag says so instead of ignoring it.
    #[test]
    fn a_lane_that_cannot_honour_a_flag_refuses_it() {
        let model = ChildOptions {
            model: Some("anything".to_string()),
            ..ChildOptions::default()
        };
        assert!(model.check(&ChildLane::OpenAgents).is_err());
        assert!(model.check(&ChildLane::Devin).is_err());
        assert!(model.check(&ChildLane::Claude).is_ok());

        let ask = ChildOptions {
            ask: true,
            ..ChildOptions::default()
        };
        assert!(ask.check(&ChildLane::OpenAgents).is_err());
        assert!(ask.check(&ChildLane::Codex).is_ok());

        let config = ChildOptions {
            config: Some("/tmp/c.json".to_string()),
            ..ChildOptions::default()
        };
        assert!(config.check(&ChildLane::OpenAgents).is_err());
        assert!(
            config
                .check(&ChildLane::Opencode {
                    model: "m".to_string()
                })
                .is_ok()
        );

        // Nothing asked for, nothing refused, on any lane.
        for lane in [
            ChildLane::OpenAgents,
            ChildLane::Devin,
            ChildLane::Claude,
            ChildLane::Codex,
        ] {
            assert!(ChildOptions::default().check(&lane).is_ok());
        }
    }

    /// `--description` names the run; without one the prompt does.
    #[test]
    fn a_run_is_named_by_its_description_or_its_prompt() {
        assert_eq!(
            describe(Some("port the flags"), "anything"),
            "port the flags"
        );
        assert_eq!(
            describe(Some("   "), "one two three four five six"),
            "one two three four five"
        );
        assert_eq!(
            describe(None, "one two three four five six"),
            "one two three four five"
        );
        assert_eq!(describe(None, "   "), "delegated task");
    }

    /// The two commands that run a fan-out resolve to the same request.
    ///
    /// Including the `--child-*` flags: `oa coder --delegate --child-config f`
    /// used to drop the config on the floor, which is the only route a provider
    /// credential has to a child.
    #[test]
    fn both_entry_points_resolve_to_one_request() {
        let request = DelegationRequest::from_delegate(DelegateArgs {
            prompt: Some("go".to_string()),
            agents: 4,
            dir: Some("/tmp/here".to_string()),
            description: Some("a run".to_string()),
            concurrency: Some(2),
            lane: Some("claude".to_string()),
            isolation: None,
            keep_workspaces: true,
            child_model: Some("m".to_string()),
            child_command: Some("c".to_string()),
            child_config: Some("f".to_string()),
            child_ask: true,
        });
        assert_eq!(request.count, 4);
        assert_eq!(request.max_parallel, Some(2));
        assert_eq!(request.directory.as_deref(), Some("/tmp/here"));
        assert!(request.child_ask);

        let request = DelegationRequest::from_coder(CoderArgs {
            prompt: Some("go".to_string()),
            delegate: true,
            count: 3,
            max_parallel: Some(2),
            isolation: None,
            keep_workspaces: false,
            child_model: Some("anthropic/claude".to_string()),
            child_command: Some("opencode".to_string()),
            child_config: Some("/tmp/harness.json".to_string()),
            child_ask: true,
            lane: None,
            model: None,
            local: false,
            offline: false,
            reasoning: None,
            resume: None,
            last: false,
            all: false,
            headless: false,
            export: None,
            plain: false,
            dev: false,
            dev_port: 4000,
        });
        assert_eq!(request.count, 3);
        assert_eq!(request.max_parallel, Some(2));
        assert_eq!(request.child_model.as_deref(), Some("anthropic/claude"));
        assert_eq!(request.child_command.as_deref(), Some("opencode"));
        assert_eq!(request.child_config.as_deref(), Some("/tmp/harness.json"));
        assert!(request.child_ask);
        // `--dir` and `--description` are still `oa delegate`'s alone.
        assert!(request.directory.is_none());
        assert!(request.description.is_none());
    }

    /// A session's inference lane is not a child lane, but it is not an error.
    ///
    /// Known inference tiers (`flash`, `free`, `local`) fall back to the
    /// default child lane. Recognised child lanes pass through unchanged. A
    /// name that is neither is refused.
    #[test]
    fn resolve_for_session_maps_inference_tiers_to_the_default_child_lane() {
        assert_eq!(
            ChildLane::resolve_for_session("flash").unwrap(),
            ChildLane::OpenAgents
        );
        assert_eq!(
            ChildLane::resolve_for_session("free").unwrap(),
            ChildLane::OpenAgents
        );
        assert_eq!(
            ChildLane::resolve_for_session("local").unwrap(),
            ChildLane::OpenAgents
        );
        assert_eq!(
            ChildLane::resolve_for_session("devin").unwrap(),
            ChildLane::Devin
        );
        assert!(ChildLane::resolve_for_session("nonsense-lane-xyz").is_err());
    }
}
