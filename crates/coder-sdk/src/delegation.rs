//! Delegation across harnesses (#11511): one `delegate` tool, the same on
//! every harness, that hands a task to another harness (Claude Code,
//! Codex, OpenCode, an ACP agent, or Coder's own loop) and gets a short
//! structured result back.
//!
//! The tool is built so the parent's prompt cache survives it:
//!
//! - **Fixed.** Its name, description and schema are constants
//!   ([`DESCRIPTION`], [`schema`]). No harness list, model list or plugin
//!   state is embedded; the target and its model are checked when the
//!   call arrives ([`Host::target`]), so turning a plugin on or off never
//!   changes the parent's tool list, and nothing is added to the parent's
//!   system prompt.
//! - **Short answers.** The child's reply comes back as a [`Report`]:
//!   outcome, a clipped summary, changed files, checks, and the path of
//!   the full transcript on disk. Never the child's transcript.
//! - **Background children** answer at once with their id; their progress
//!   reaches the agent tray, not the parent's context.
//!
//! The child is briefed for its task ([`briefing`]), not handed the
//! parent's history, and runs under its own harness's hooks and floors in
//! its own worktree (the [`Host`] does that). Authority never widens
//! ([`Authority::child`]): a read-only parent can't start a writing child,
//! and a child carries no disclosure its parent lacks. Nesting is bounded
//! and recorded ([`Lineage`]): each child knows its depth, its parent's
//! run id (its decisions log under it) and the chain of harnesses above it.
//!
//! [`Delegator::delegate`] is the one code path: the tool on every harness,
//! `coder delegate --to`, and the headless JSON path all call it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::admission::Admission;
use super::tools::Tool;

/// The tool's name on every harness (`mcp__coder__delegate` to Claude
/// Code, `coder.delegate` in Codex's namespace).
pub const TOOL: &str = "delegate";

/// The tool's description: fixed text, the same for the whole chat on
/// every harness.
pub const DESCRIPTION: &str = "Hand a task to another coding agent and get a short result back: outcome, summary, changed files, checks, and the path of its full transcript. `to` names the agent: claude-code, codex, opencode, coder (Coder's own loop), the id of another installed ACP agent, or auto to let Coder choose. An agent that isn't available here, or a model it doesn't offer, is refused with the ones that are. The agent does not see this chat: give it the whole task with the context it needs. It works in a fresh worktree on its own branch, or in this folder without changing anything when read_only is set. With background set it starts and returns at once; its progress and result show in the agent tray.";

/// The tool's argument schema: fixed, like [`DESCRIPTION`].
#[must_use]
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "to": {"type": "string", "minLength": 1, "maxLength": 64, "description": "The agent: claude-code, codex, opencode, coder, an ACP agent id, or auto."},
            "model": {"type": "string", "maxLength": 200, "description": "A model that agent offers; left out, it runs its own default."},
            "task": {"type": "string", "minLength": 1, "maxLength": TASK_MAX, "description": "The whole task, with the context it needs."},
            "read_only": {"type": "boolean", "description": "Work in this folder without changing any file (an audit, review or question)."},
            "background": {"type": "boolean", "description": "Start it and return at once; it reports in the agent tray."}
        },
        "required": ["to", "task"],
        "additionalProperties": false
    })
}

/// The longest task a call takes.
pub const TASK_MAX: usize = 65_536;
/// The longest summary a [`Report`] carries; the rest stays in the
/// transcript.
pub const SUMMARY_MAX: usize = 800;
/// How many changed files a [`Report`] names; the rest are counted.
pub const FILES_MAX: usize = 40;
/// How many checks a [`Report`] names.
pub const CHECKS_MAX: usize = 20;
/// How deep delegation nests unless the host sets less: a parent (0), its
/// child (1) and that child's child (2).
pub const MAX_DEPTH: u32 = 2;

/// The variable naming a session's depth in the delegation tree.
pub const DEPTH_VAR: &str = "CODER_DELEGATION_DEPTH";
/// The variable naming the deepest a tree may go.
pub const MAX_VAR: &str = "CODER_DELEGATION_MAX";
/// The variable naming this child's id.
pub const ID_VAR: &str = "CODER_DELEGATION_ID";
/// The variable naming the harnesses above this session, `a>b>c`.
pub const CHAIN_VAR: &str = "CODER_DELEGATION_CHAIN";
/// The variable naming the harness this session runs on.
pub const HARNESS_VAR: &str = "CODER_DELEGATION_HARNESS";
/// The variable holding the child's [`Authority`], as JSON.
pub const AUTHORITY_VAR: &str = "CODER_DELEGATION_AUTHORITY";

/// One `delegate` call's arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub task: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub background: bool,
}

impl Request {
    /// The call's arguments, checked against [`schema`].
    ///
    /// # Errors
    ///
    /// Unknown or missing fields, an empty or oversized task.
    pub fn parse(arguments: Value) -> Result<Self, String> {
        let mut request: Request = serde_json::from_value(arguments).map_err(|error| {
            format!("delegate takes to and task, and optionally model, read_only and background ({error}).")
        })?;
        request.to = request.to.trim().to_owned();
        request.model = request
            .model
            .map(|model| model.trim().to_owned())
            .filter(|model| !model.is_empty());
        if request.to.is_empty() || request.to.len() > 64 {
            return Err("Name the agent in `to`.".into());
        }
        if request.task.trim().is_empty() {
            return Err("Give the agent a task.".into());
        }
        if request.task.len() > TASK_MAX {
            return Err(format!("The task is longer than {TASK_MAX} bytes."));
        }
        Ok(request)
    }
}

/// Where a session stands in the delegation tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineage {
    /// The run every decision in the tree logs under: the root's.
    pub run: String,
    /// This session's id; the root's is the run id.
    pub id: String,
    /// The harness this session runs on.
    pub harness: String,
    /// The harnesses above it, root first.
    pub chain: Vec<String>,
    pub depth: u32,
    pub max_depth: u32,
}

impl Lineage {
    /// The root of a tree: a session nobody delegated to.
    #[must_use]
    pub fn root(run: &str, harness: &str) -> Self {
        Lineage {
            run: run.to_owned(),
            id: run.to_owned(),
            harness: harness.to_owned(),
            chain: Vec::new(),
            depth: 0,
            max_depth: MAX_DEPTH,
        }
    }

    /// This session's place, read from its environment: a child started by
    /// a delegation carries [`Lineage::env`]; anything else is a root with
    /// `run` (the session's own run id when none is set).
    #[must_use]
    pub fn from_env(get: &dyn Fn(&str) -> Option<String>, harness: &str, run: &str) -> Self {
        let number = |name: &str| get(name).and_then(|value| value.trim().parse::<u32>().ok());
        let run = get(hook_log::RUN_ID_VAR)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| run.to_owned());
        let depth = number(DEPTH_VAR).unwrap_or(0);
        Lineage {
            harness: get(HARNESS_VAR)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| harness.to_owned()),
            id: get(ID_VAR)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| run.clone()),
            run,
            chain: get(CHAIN_VAR)
                .map(|chain| {
                    chain
                        .split('>')
                        .filter(|part| !part.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            depth,
            // A child can lower the bound for its own children, never raise
            // it past the default.
            max_depth: number(MAX_VAR).unwrap_or(MAX_DEPTH).min(MAX_DEPTH),
        }
    }

    /// A deeper bound than the default is refused; a shallower one holds
    /// for the whole tree below.
    #[must_use]
    pub fn bounded(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth.min(self.max_depth);
        self
    }

    /// The lineage of this session's `n`th child, on `harness`.
    ///
    /// # Errors
    ///
    /// The child would be deeper than the tree may go.
    pub fn child(&self, harness: &str, n: u64) -> Result<Lineage, String> {
        if self.depth >= self.max_depth {
            return Err(format!(
                "Delegation is {} deep here, the most it may nest; do this part yourself.",
                self.depth
            ));
        }
        let mut chain = self.chain.clone();
        chain.push(self.harness.clone());
        Ok(Lineage {
            run: self.run.clone(),
            id: format!("{}.d{n}", self.id),
            harness: harness.to_owned(),
            chain,
            depth: self.depth + 1,
            max_depth: self.max_depth,
        })
    }

    /// The variables a child's process gets, so its own tools and hooks
    /// know where it stands and log under the root's run.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            (hook_log::RUN_ID_VAR.to_owned(), self.run.clone()),
            (ID_VAR.to_owned(), self.id.clone()),
            (DEPTH_VAR.to_owned(), self.depth.to_string()),
            (MAX_VAR.to_owned(), self.max_depth.to_string()),
            (CHAIN_VAR.to_owned(), self.chain.join(">")),
            (HARNESS_VAR.to_owned(), self.harness.clone()),
        ]
    }

    /// The chain with this session at the end: `claude-code>codex`.
    #[must_use]
    pub fn path(&self) -> String {
        let mut chain = self.chain.clone();
        chain.push(self.harness.clone());
        chain.join(">")
    }
}

/// What a session may do that a child could inherit: write files, and
/// send what its admission lets leave.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    pub write: bool,
    pub admission: Admission,
}

impl Default for Authority {
    fn default() -> Self {
        Authority {
            write: true,
            admission: Admission::none(),
        }
    }
}

impl Authority {
    /// The authority a delegated session was given, from its environment;
    /// a root session has the host's.
    #[must_use]
    pub fn from_env(get: &dyn Fn(&str) -> Option<String>, root: Authority) -> Self {
        get(AUTHORITY_VAR)
            .and_then(|text| serde_json::from_str::<Authority>(&text).ok())
            .map_or(root.clone(), |given| {
                // What the environment says can only narrow the host's.
                Authority {
                    write: given.write && root.write,
                    admission: Admission {
                        disclosures: given
                            .admission
                            .disclosures
                            .into_iter()
                            .filter(|disclosure| root.admission.covers(disclosure))
                            .collect(),
                    },
                }
            })
    }

    /// What a child asked to work `read_only` (or not) gets: never more
    /// than this session has.
    ///
    /// # Errors
    ///
    /// A writing child under a session that may not write.
    pub fn child(&self, read_only: bool) -> Result<Authority, String> {
        if !read_only && !self.write {
            return Err("This session may not change files, so neither may an agent it starts. Ask again with read_only.".into());
        }
        Ok(Authority {
            write: self.write && !read_only,
            admission: self.admission.clone(),
        })
    }

    /// Whether `other` is within this authority.
    #[must_use]
    pub fn covers(&self, other: &Authority) -> bool {
        (self.write || !other.write)
            && other
                .admission
                .disclosures
                .iter()
                .all(|disclosure| self.admission.covers(disclosure))
    }

    /// The variable a child's process carries.
    #[must_use]
    pub fn env(&self) -> (String, String) {
        (
            AUTHORITY_VAR.to_owned(),
            serde_json::to_string(self).unwrap_or_default(),
        )
    }
}

/// The harness a call resolved to, checked at call time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// The harness id: `claude-code`, `codex`, `opencode`, `coder`, or an
    /// ACP agent's id.
    pub harness: String,
    /// The name a person reads.
    pub name: String,
    /// The model, when the call named one the harness offers.
    pub model: Option<String>,
}

/// One child, ready for the host to run.
#[derive(Clone, Debug)]
pub struct Child {
    pub request: Request,
    pub target: Target,
    pub lineage: Lineage,
    pub authority: Authority,
    /// What the child reads instead of the parent's history.
    pub briefing: String,
    /// Where its transcript and records go.
    pub run_dir: PathBuf,
}

impl Child {
    /// The variables its process gets.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = self.lineage.env();
        env.push(self.authority.env());
        env
    }

    /// Where its full transcript is written.
    #[must_use]
    pub fn transcript(&self) -> PathBuf {
        self.run_dir.join("transcript.jsonl")
    }
}

/// How a child's run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// It finished its task.
    Done,
    /// It started in the background and is running.
    Running,
    /// It ran and failed.
    Failed,
    /// It was stopped.
    Stopped,
}

/// A check the child ran: a build, a test, a lint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub command: String,
    pub passed: bool,
}

/// What the parent gets back: short, structured, and pointing at the rest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub outcome: Outcome,
    /// The child's id: `<parent>.d<n>`.
    pub child: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// What it did, clipped to [`SUMMARY_MAX`].
    pub summary: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    /// Changed files past [`FILES_MAX`], counted only.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more_files: usize,
    #[serde(default)]
    pub checks: Vec<Check>,
    /// The full transcript, on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub read_only: bool,
    pub depth: u32,
    /// The run its decisions log under.
    pub run: String,
    /// `claude-code>codex`: the harnesses from the root to this child.
    pub chain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(count: &usize) -> bool {
    *count == 0
}

impl Report {
    /// A report for `child` with nothing in it yet.
    #[must_use]
    pub fn for_child(child: &Child, outcome: Outcome) -> Self {
        Report {
            outcome,
            child: child.lineage.id.clone(),
            harness: child.target.harness.clone(),
            model: child.target.model.clone(),
            summary: String::new(),
            changed_files: Vec::new(),
            more_files: 0,
            checks: Vec::new(),
            transcript: Some(child.transcript()),
            worktree: None,
            branch: None,
            read_only: child.request.read_only,
            depth: child.lineage.depth,
            run: child.lineage.run.clone(),
            chain: child.lineage.path(),
            error: None,
        }
    }

    /// The summary, clipped on a character boundary with a pointer to the
    /// transcript.
    #[must_use]
    pub fn summarized(mut self, text: &str) -> Self {
        self.summary = clip(text.trim(), SUMMARY_MAX);
        self
    }

    /// The changed files, the first [`FILES_MAX`] named.
    #[must_use]
    pub fn with_files(mut self, mut files: Vec<String>) -> Self {
        files.sort();
        files.dedup();
        self.more_files = files.len().saturating_sub(FILES_MAX);
        files.truncate(FILES_MAX);
        self.changed_files = files;
        self
    }

    /// The checks, the first [`CHECKS_MAX`].
    #[must_use]
    pub fn with_checks(mut self, mut checks: Vec<Check>) -> Self {
        checks.truncate(CHECKS_MAX);
        for check in &mut checks {
            check.command = clip(&check.command, 200);
        }
        self.checks = checks;
        self
    }

    /// A failed report saying why.
    #[must_use]
    pub fn failed(mut self, error: &str) -> Self {
        self.outcome = Outcome::Failed;
        self.error = Some(clip(error, SUMMARY_MAX));
        self
    }

    /// The report as the tool returns it.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// `text` cut to `max` bytes on a character boundary, with an ellipsis.
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub(3);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// The closing a child was asked for in its [`briefing`], read from the
/// end of its reply: a fenced JSON object with `summary` and `checks`.
/// A reply without one gives its last paragraph as the summary.
#[must_use]
pub fn closing(reply: &str) -> (String, Vec<Check>, Option<bool>) {
    #[derive(Deserialize)]
    struct Closing {
        #[serde(default)]
        summary: String,
        #[serde(default)]
        checks: Vec<Check>,
        #[serde(default)]
        done: Option<bool>,
    }
    let fenced = reply.rfind("```json").and_then(|start| {
        let body = &reply[start + 7..];
        body.find("```").map(|end| &body[..end])
    });
    if let Some(closing) = fenced.and_then(|body| serde_json::from_str::<Closing>(body.trim()).ok())
    {
        return (closing.summary, closing.checks, closing.done);
    }
    let last = reply
        .trim()
        .rsplit("\n\n")
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    (last, Vec::new(), None)
}

/// What a child reads first: its task and how to close, not the parent's
/// history.
#[must_use]
pub fn briefing(request: &Request, lineage: &Lineage) -> String {
    let place = if request.read_only {
        "Work in the current folder and change no file: this is a read-only task."
    } else {
        "You are in a fresh git worktree on your own branch. Make your changes here, run the checks that prove them, and leave them uncommitted or committed on this branch; do not push."
    };
    format!(
        "You are a coding agent another agent ({parent}) handed one task. You do not see its conversation; everything you need is below.\n\n{place}\n\nTask:\n{task}\n\nWhen you are done, end your reply with a fenced json block: {{\"summary\": \"what you did, in two or three sentences\", \"done\": true or false, \"checks\": [{{\"command\": \"the command\", \"passed\": true or false}}]}}.",
        parent = lineage.harness,
        task = request.task.trim(),
    )
}

/// The host side of delegation: which harnesses exist here, and how a
/// child runs on one.
pub trait Host: Send + Sync {
    /// Checks the call's harness and model now, when it arrives: `auto`
    /// picks one; a harness that isn't available, or a model it doesn't
    /// offer, is refused with the ones that are.
    ///
    /// # Errors
    ///
    /// What the parent reads.
    fn target(&self, request: &Request) -> Result<Target, String>;

    /// Runs `child` to its end, in its own worktree (or read-only in the
    /// folder), under its harness's hooks and floors, and reports.
    fn run(&self, child: Child) -> BoxFuture<'static, Report>;

    /// Starts `child` in the background, reporting in the agent tray, and
    /// returns at once.
    ///
    /// # Errors
    ///
    /// No agent tray here, or it could not start.
    fn start(&self, child: Child) -> Result<Report, String>;
}

/// One record per delegation: parent to child, with how it ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub run: String,
    pub parent: String,
    pub parent_harness: String,
    pub child: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub depth: u32,
    pub chain: String,
    pub read_only: bool,
    pub background: bool,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The schema of a [`Record`] line.
pub const RECORD_SCHEMA: &str = "openagents.coder.delegation.v1";

/// One session's delegations: its place in the tree, its authority, and
/// the host that runs children.
pub struct Delegator {
    host: Arc<dyn Host>,
    lineage: Lineage,
    authority: Authority,
    /// The folder each child's run folder goes under.
    runs: PathBuf,
    next: AtomicU64,
}

impl std::fmt::Debug for Delegator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Delegator")
            .field("lineage", &self.lineage)
            .field("authority", &self.authority)
            .field("runs", &self.runs)
            .finish_non_exhaustive()
    }
}

impl Delegator {
    #[must_use]
    pub fn new(host: Arc<dyn Host>, lineage: Lineage, authority: Authority, runs: &Path) -> Self {
        Delegator {
            host,
            lineage,
            authority,
            runs: runs.to_owned(),
            next: AtomicU64::new(1),
        }
    }

    #[must_use]
    pub fn lineage(&self) -> &Lineage {
        &self.lineage
    }

    #[must_use]
    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    /// Where the delegation records go: `<runs>/delegations.jsonl`.
    #[must_use]
    pub fn records(&self) -> PathBuf {
        self.runs.join("delegations.jsonl")
    }

    /// The child a call makes, checked: arguments, nesting, authority, and
    /// the target harness and model, all at call time.
    ///
    /// # Errors
    ///
    /// What the parent reads.
    pub fn prepare(&self, arguments: Value) -> Result<Child, String> {
        let request = Request::parse(arguments)?;
        // Nesting first: a session at the bottom of the tree starts nothing.
        self.lineage.child("", 0)?;
        let authority = self.authority.child(request.read_only)?;
        let target = self.host.target(&request)?;
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let lineage = self.lineage.child(&target.harness, n)?;
        let run_dir = self.runs.join(&lineage.id);
        Ok(Child {
            briefing: briefing(&request, &self.lineage),
            request,
            target,
            lineage,
            authority,
            run_dir,
        })
    }

    /// Runs one delegation: the tool on every harness, `coder delegate`,
    /// and the headless path all come here.
    ///
    /// # Errors
    ///
    /// The call was refused before a child started: its arguments, the
    /// nesting bound, authority, or the target.
    pub async fn delegate(&self, arguments: Value) -> Result<Report, String> {
        let child = match self.prepare(arguments.clone()) {
            Ok(child) => child,
            Err(error) => {
                self.record_refused(&arguments, &error);
                return Err(error);
            }
        };
        let _ = std::fs::create_dir_all(&child.run_dir);
        let background = child.request.background;
        let report = if background {
            match self.host.start(child.clone()) {
                Ok(report) => report,
                Err(error) => {
                    self.record_refused(&arguments, &error);
                    return Err(error);
                }
            }
        } else {
            self.host.run(child.clone()).await
        };
        self.record(&child, &report);
        Ok(report)
    }

    fn record(&self, child: &Child, report: &Report) {
        let record = Record {
            schema: RECORD_SCHEMA.to_owned(),
            run: self.lineage.run.clone(),
            parent: self.lineage.id.clone(),
            parent_harness: self.lineage.harness.clone(),
            child: child.lineage.id.clone(),
            harness: child.target.harness.clone(),
            model: child.target.model.clone(),
            depth: child.lineage.depth,
            chain: child.lineage.path(),
            read_only: child.request.read_only,
            background: child.request.background,
            outcome: serde_json::to_value(report.outcome)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default(),
            error: report.error.clone(),
        };
        append(&self.records(), &record);
    }

    fn record_refused(&self, arguments: &Value, error: &str) {
        let record = Record {
            schema: RECORD_SCHEMA.to_owned(),
            run: self.lineage.run.clone(),
            parent: self.lineage.id.clone(),
            parent_harness: self.lineage.harness.clone(),
            child: String::new(),
            harness: arguments["to"].as_str().unwrap_or_default().to_owned(),
            model: arguments["model"].as_str().map(str::to_owned),
            depth: self.lineage.depth + 1,
            chain: self.lineage.path(),
            read_only: arguments["read_only"].as_bool().unwrap_or(false),
            background: arguments["background"].as_bool().unwrap_or(false),
            outcome: "refused".to_owned(),
            error: Some(clip(error, SUMMARY_MAX)),
        };
        append(&self.records(), &record);
    }

    /// The `delegate` tool for this session: [`DESCRIPTION`] and
    /// [`schema`], always loaded, its answer the [`Report`] as JSON.
    #[must_use]
    pub fn tool(self: &Arc<Self>) -> Tool {
        let delegator = self.clone();
        Tool::new(TOOL, DESCRIPTION, schema(), move |input: Value| {
            let delegator = delegator.clone();
            async move {
                Ok(match delegator.delegate(input).await {
                    Ok(report) => claude_agent_sdk::ToolResult::text(report.to_value().to_string()),
                    Err(error) => claude_agent_sdk::ToolResult::error(error),
                })
            }
        })
        .always_load(true)
    }
}

fn append(path: &Path, record: &Record) {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let (Ok(line), Ok(mut file)) = (
        serde_json::to_string(record),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path),
    ) {
        let _ = writeln!(file, "{line}");
    }
}

/// The records in `path`, oldest first.
#[must_use]
pub fn read_records(path: &Path) -> Vec<Record> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The `delegate` tool's definition as the Coder loop offers tools
/// natively (an OpenAI-style function): the same name, description and
/// schema.
#[must_use]
pub fn function_definition() -> Value {
    json!({"type": "function", "function": {
        "name": TOOL,
        "description": DESCRIPTION,
        "parameters": schema(),
    }})
}

/// Files a child changed in `worktree` since `base`: committed on its
/// branch, staged, unstaged and untracked.
#[must_use]
pub fn changed_files(worktree: &Path, base: Option<&str>) -> Vec<String> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    let mut files: Vec<String> = git(&["status", "--porcelain", "--untracked-files=all"])
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|path| {
            path.rsplit(" -> ")
                .next()
                .unwrap_or(path)
                .trim_matches('"')
                .to_owned()
        })
        .collect();
    if let Some(base) = base.filter(|base| !base.is_empty()) {
        files.extend(
            git(&["diff", "--name-only", &format!("{base}..HEAD")])
                .lines()
                .map(str::to_owned),
        );
    }
    files.sort();
    files.dedup();
    files
}

/// The prompt-cache read ratio of a Claude Code turn's usage: the share of
/// its input read from the cache.
#[must_use]
pub fn cache_read_ratio(usage: &Value) -> Option<f64> {
    let read = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
    let created = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
    let fresh = usage["input_tokens"].as_u64().unwrap_or(0);
    let total = read + created + fresh;
    #[allow(clippy::cast_precision_loss)]
    (total > 0).then(|| read as f64 / total as f64)
}

pub mod scripted;

#[cfg(test)]
#[path = "delegation_tests.rs"]
mod tests;
