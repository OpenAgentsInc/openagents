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
//! The lanes follow `coder-delegate.ts`: `ox-alpha` runs in this process on
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
use tokio::sync::{mpsc, watch, Semaphore};

use crate::acp::{AcpEvent, AcpFailure, AcpHarness, PermissionMode};
use crate::cli::{fail, CoderArgs};
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
    /// The operating system process, for a child that is one. `ox-alpha` runs
    /// in this process and reports `None`; what it starts are its `shell` tool
    /// subprocesses.
    pub pid: Option<u32>,
    pub workspace: Option<PathBuf>,
    /// Set when the child did not answer, so a caller does not have to read
    /// `output` to find out.
    pub failure: Option<String>,
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
    Output { id: usize, text: String },
    /// Something the child did that is not its answer: a tool call, a token
    /// count, a session id.
    Activity { id: usize, text: String },
    Finished(Box<ChildWorkerResult>),
}

/// Which harness and model a child runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildLane {
    /// This process, on the OpenAgents inference proxy, with this session's
    /// tools.
    OxAlpha,
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
    pub fn parse(name: &str) -> Self {
        let lowered = name.trim().to_lowercase();
        match lowered.as_str() {
            "gemini" | "gemini-flash" => ChildLane::Opencode {
                model: "gemini-3.7-flash".to_string(),
            },
            "devin" => ChildLane::Devin,
            "claude" => ChildLane::Claude,
            "codex" => ChildLane::Codex,
            "ox-alpha" | "ox" | "openagents" => ChildLane::OxAlpha,
            other if other.starts_with("opencode/") => ChildLane::Opencode {
                model: other.trim_start_matches("opencode/").to_string(),
            },
            // An unknown name used to fall through to `ox-alpha` in silence, so
            // a typo spent this account's budget on a lane the caller did not
            // ask for. It still runs there, but the caller is told.
            _ => ChildLane::OxAlpha,
        }
    }

    /// Whether [`ChildLane::parse`] recognised the name it was given.
    pub fn known(name: &str) -> bool {
        let lowered = name.trim().to_lowercase();
        matches!(
            lowered.as_str(),
            "gemini" | "gemini-flash" | "devin" | "claude" | "codex" | "ox-alpha" | "ox" | "openagents"
        ) || lowered.starts_with("opencode/")
    }

    pub fn label(&self) -> String {
        match self {
            ChildLane::OxAlpha => "ox-alpha (this process, the OpenAgents proxy)".to_string(),
            ChildLane::Opencode { model } => format!("opencode ({model})"),
            ChildLane::Devin => "devin (ACP over the Devin CLI)".to_string(),
            ChildLane::Claude => "claude (Claude Code print mode)".to_string(),
            ChildLane::Codex => "codex (Codex exec)".to_string(),
        }
    }

    /// The binary this lane needs on `PATH`, if it needs one.
    pub fn binary(&self) -> Option<&'static str> {
        match self {
            ChildLane::OxAlpha => None,
            ChildLane::Opencode { .. } => Some("opencode"),
            ChildLane::Devin => Some("devin"),
            ChildLane::Claude => Some("claude"),
            ChildLane::Codex => Some("codex"),
        }
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
        let lane = ChildLane::parse(&self.lane);
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let plan = WorkspacePlan::resolve(cwd, self.isolation).await;
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

            handles.push(tokio::spawn(async move {
                // The cap is here rather than around the spawn so a child that
                // is waiting for a slot still exists and still reports.
                let _slot = gate.acquire().await;
                let result = run_child(task, lane, workspace, token, &events, cancel).await;
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
    events: &mpsc::UnboundedSender<ChildEvent>,
    cancel: watch::Receiver<bool>,
) -> ChildWorkerResult {
    let start = Instant::now();
    let id = task.id;

    let outcome = match &lane {
        ChildLane::OxAlpha => {
            let _ = events.send(ChildEvent::Started {
                id,
                lane: lane.label(),
                workspace: workspace.describe(),
                pid: None,
            });
            run_proxy_child(&task, &workspace, user_token, events, cancel).await
        }
        ChildLane::Devin => {
            run_devin_child(&task, &lane, &workspace, events, cancel).await
        }
        ChildLane::Claude | ChildLane::Codex | ChildLane::Opencode { .. } => {
            run_cli_child(&task, &lane, &workspace, events, cancel).await
        }
    };

    let duration_ms = start.elapsed().as_millis();
    match outcome {
        Ok(ChildAnswer { text, pid }) => ChildWorkerResult {
            id,
            success: true,
            output: clip(&text),
            duration_ms,
            pid,
            workspace: Some(workspace.path.clone()),
            failure: None,
        },
        Err(ChildFailure { why, pid }) => ChildWorkerResult {
            id,
            success: false,
            output: why.clone(),
            duration_ms,
            pid,
            workspace: Some(workspace.path.clone()),
            failure: Some(why),
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
    let mut runtime = CoderRuntimeSession::new(Lane::OxAlpha, None, user_token, tools);

    let id = task.id;
    let sink = events.clone();
    let turn = runtime.execute_turn(&task.prompt, move |chunk| {
        let _ = sink.send(ChildEvent::Output {
            id,
            text: chunk.to_string(),
        });
    });

    tokio::select! {
        biased;
        _ = cancel.changed() => Err(ChildFailure {
            why: "stopped before finishing".to_string(),
            pid: None,
        }),
        answered = turn => match answered {
            Ok(text) => Ok(ChildAnswer { text, pid: None }),
            Err(error) => Err(ChildFailure { why: error.to_string(), pid: None }),
        },
    }
}

/// A child on the Devin CLI, over the Agent Client Protocol.
async fn run_devin_child(
    task: &ChildWorkerTask,
    lane: &ChildLane,
    workspace: &ChildWorkspace,
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
        command: harness_binary(lane),
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
    events: &mpsc::UnboundedSender<ChildEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<ChildAnswer, ChildFailure> {
    let id = task.id;
    let (command, args) = harness_command(lane, &task.prompt, &workspace.path);

    let mut child = match Command::new(&command)
        .args(&args)
        .current_dir(&workspace.path)
        // No terminal, so a harness that would prompt gets end-of-file rather
        // than a wait nobody can see.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
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
            })
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
        ChildLane::OxAlpha => ("", ""),
    };
    std::env::var(variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// The binary and arguments each CLI lane runs, following `coder-delegate.ts`.
fn harness_command(lane: &ChildLane, prompt: &str, cwd: &std::path::Path) -> (String, Vec<String>) {
    match lane {
        ChildLane::Claude => (
            harness_binary(lane),
            vec![
                "-p".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
                // `stream-json` requires it.
                "--verbose".to_string(),
                // A delegated child has nobody to ask.
                "--permission-mode".to_string(),
                "acceptEdits".to_string(),
            ],
        ),
        ChildLane::Codex => (
            harness_binary(lane),
            vec![
                "exec".to_string(),
                "--json".to_string(),
                // A child's worktree is a checkout but not one Codex has been
                // told to trust, and without this it refuses before it starts.
                "--skip-git-repo-check".to_string(),
                // The child may edit the checkout it was pointed at and
                // nothing outside it.
                "--sandbox".to_string(),
                "workspace-write".to_string(),
                prompt.to_string(),
            ],
        ),
        ChildLane::Opencode { model } => (
            harness_binary(lane),
            vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--model".to_string(),
                model.clone(),
                "--dir".to_string(),
                cwd.to_string_lossy().to_string(),
                prompt.to_string(),
            ],
        ),
        // Handled by their own runners; unreachable through this function.
        ChildLane::OxAlpha => ("".to_string(), Vec::new()),
        ChildLane::Devin => (harness_binary(lane), vec!["acp".to_string()]),
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
                    ChildLane::OxAlpha => "ox-alpha",
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
pub async fn run_delegation(
    args: CoderArgs,
    user_token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let requested = args.count.max(1);
    if requested > MAX_DELEGATE_COUNT {
        fail(&format!(
            "{requested} children were asked for and this command runs at most {MAX_DELEGATE_COUNT}."
        ));
    }
    let lane_name = args.lane.clone().unwrap_or_else(|| "ox-alpha".to_string());
    let prompt = args
        .prompt
        .clone()
        .unwrap_or_else(|| "Analyze workspace and run tests".to_string());

    if !ChildLane::known(&lane_name) {
        fail(&format!(
            "there is no `{lane_name}` lane. This command runs children on: ox-alpha, gemini, opencode/<model>, devin, claude, codex."
        ));
    }
    let lane = ChildLane::parse(&lane_name);

    let isolation = match args.isolation.as_deref() {
        None => Isolation::Worktree,
        Some(named) => match Isolation::parse(named) {
            Some(isolation) => isolation,
            None => fail(&format!(
                "`{named}` is not an isolation this command knows. Use worktree, directory, or none."
            )),
        },
    };

    let supervisor = DelegationSupervisor::new(requested, &lane_name, user_token)
        .with_isolation(isolation)
        .with_max_parallel(args.max_parallel.unwrap_or(requested))
        .keeping_workspaces(args.keep_workspaces);

    println!(
        "Delegating to {} {} on {}, {} at a time, isolation: {}.",
        supervisor.count,
        if supervisor.count == 1 { "child" } else { "children" },
        lane.label(),
        supervisor.max_parallel,
        isolation.name(),
    );

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
                    println!(
                        "[child {id}] started on {lane} in {workspace}{}",
                        match pid {
                            Some(pid) => format!(" as pid {pid}"),
                            None => " in this process".to_string(),
                        }
                    );
                }
                ChildEvent::Output { id, text } => printer.feed(id, &text),
                ChildEvent::Activity { id, text } => {
                    printer.flush(id);
                    println!("[child {id}] · {text}");
                }
                ChildEvent::Finished(result) => {
                    printer.flush(result.id);
                    println!(
                        "[child {}] {} after {}ms",
                        result.id,
                        if result.success { "finished" } else { "FAILED" },
                        result.duration_ms
                    );
                }
            }
        }
    });

    let results = match supervisor.dispatch_streaming(&prompt, events, cancel).await {
        Ok(results) => results,
        // No child ran at all, so there is nothing to report but the reason.
        Err(error) => fail(&format!("no children were started: {error}")),
    };
    let _ = printing.await;
    interrupt.abort();

    println!();
    let succeeded = results.iter().filter(|result| result.success).count();
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
        if results.len() == 1 { "child" } else { "children" },
        lane.label()
    );

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
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    let prompt = prompt.to_string();
    let lane = lane.to_string();
    Box::pin(async move {
    let prompt = prompt.as_str();
    let lane = lane.as_str();
    let count = count.clamp(1, MAX_DELEGATE_COUNT);
    let supervisor = DelegationSupervisor::new(count, lane, user_token);
    let results = supervisor.dispatch(prompt).await;

    let succeeded = results.iter().filter(|result| result.success).count();
    let mut lines = vec![format!(
        "{succeeded} of {} {} completed on {}.",
        results.len(),
        if results.len() == 1 { "child" } else { "children" },
        ChildLane::parse(lane).label()
    )];
    lines.push(String::new());
    for result in &results {
        if result.success {
            lines.push(format!(
                "child {} completed in {}ms:\n{}",
                result.id,
                result.duration_ms,
                if result.output.trim().is_empty() {
                    "(no output)"
                } else {
                    result.output.trim()
                }
            ));
        } else {
            lines.push(format!("child {} failed: {}", result.id, result.output));
        }
    }
    lines.join("\n")
    })
}
