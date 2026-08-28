//! The tools a session declares to the model, and what running them does.
//!
//! Nine tools: `read`, `write`, `edit`, `bash`, `shell`, `skill`,
//! `openagents`, `capability`, and — only where a delegation gate exists —
//! `delegate`. Each is declared to the model and
//! each has an implementation in [`HarnessToolRegistry::execute_tool`]; the
//! list and the match arms carry the same names, which is the only property
//! that keeps a declared tool from being a promise nothing keeps. This
//! module's header once claimed `capability` while `list_tools` did not
//! declare it and no arm implemented it; the rule the mistake bought is that
//! a name is written here only after something answers it.
//!
//! `read`, `write`, `edit` and `bash` are the four a coding agent needs to
//! touch a file without spelling the intent as a shell command. They are pi's
//! four, deliberately kept to pi's size: `read` returns a file, `write`
//! replaces one, `edit` replaces one exact run of text inside one, and `bash`
//! is `shell` under the name that set uses — the same arm answers both, so
//! they cannot drift apart. What the originals did not have is here because
//! this repository has shipped and fixed each of them: a bounded cut steps
//! back to a character boundary, a failure reports `is_error: true`, a refusal
//! is output the model can read and retry from, and `write` and `edit` stage
//! and rename rather than truncating in place (#114). No file tool carries a
//! working-directory jail (#151): a path may name anything on this machine,
//! because `shell` always could, and constraining the three tools that cannot
//! run a single command constrained nothing.
//!
//! `capability` searches the local catalog of digest-pinned WebAssembly
//! plugins and loads one into the session, at which point the plugin's own
//! manifest declares a second tool under its own name. The sandbox that runs
//! it — digest verification, import inspection, confined read-only mounts,
//! the memory ceiling, the timeout — is [`crate::plugins`]. A plugin whose
//! limits cannot be enforced does not run.
//!
//! The tool runtime is the client's. The inference proxy forwards the
//! declarations and returns the calls the model asks for; nothing runs
//! server-side.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::watch;

use crate::acp::{AcpEvent, acp_event_subagent_line};
use crate::coder::acp::Agent;
use crate::coder::acp_harness::{AcpFailure, AcpHarness, PermissionMode};
use crate::coder::agents::{self, AgentDefinition, ToolPool};
use crate::coder::runtime::Control;
use crate::delegate_result::{DelegateAgentResult, DelegateStatus, WorktreeOutcome, WorktreeRef};
use crate::plugins::{
    self, Approval, CatalogEntry, LoadedPlugin, answer_capability, capability_tool_definition,
    plugin_tool_definition,
};
use crate::runtime::{Lane, ModelStreamEvent, ToolEvent};
use crate::surfaces::tool_descriptions as text;
use crate::workspace;

/// The `check` tool's description, with `{scopes}` filled from the repo's
/// declared `.openagents/checks.json`. Absent when the repository declares
/// none — a tool that always refuses is noise in every declaration list.
pub fn check_tool_description(
    scopes: &std::collections::BTreeMap<String, crate::checks::CheckScope>,
) -> String {
    let mut listed: Vec<String> = scopes
        .iter()
        .map(|(name, scope)| match &scope.description {
            Some(description) => format!("- `{name}`: {description}"),
            None => format!("- `{name}`"),
        })
        .collect();
    listed.sort();
    format!(
        "Run a named check scope this repository declares in `.openagents/checks.json`, \
         cheapest-first: `diff` for the files you changed, wider scopes only when the narrow \
         one is green and the question needs it. Each command runs through the same `shell` \
         rules. A failure is checked against this session's baseline of known failures \
         (taken from clean-tree runs) and labelled inherited or new, so attribution never \
         costs another sweep. Declared scopes:\n{}\nUse `shell` directly for anything else.",
        listed.join("\n")
    )
}

pub const OUTPUT_LIMIT: usize = 30_000;

/// A command that ran at least this long keeps its whole transcript on disk
/// even when the bounded excerpt looked complete, because a summary of a
/// two-minute build is exactly the thing a follow-up question wants to grep.
pub const PERSIST_AFTER_SECS: u64 = 30;

/// How to recover text from a persisted `cmd-N.log` without re-running (#244).
const HISTORY_RECALL_GREP: &str = r#"Call `history_recall` with {"_tag":"Grep","pattern":"..."} to answer questions about that file without re-running."#;

/// The most an edit reply's region echo may add (#190).
///
/// The echo exists so the model does not spend another call confirming the
/// splice; a cap an order of magnitude under [`OUTPUT_LIMIT`] keeps the
/// reply a reply. A change larger than the cap still edits fine -- only the
/// echo is dropped, and the model reads the file as it always did.
const ECHO_LIMIT: usize = 2_000;

/// The names [`HarnessToolRegistry::execute_tool`] answers itself.
///
/// A loaded plugin is dispatched from the fallthrough arm, *below* all five of
/// these, so a plugin carrying one of these names would be declared to the
/// model and never reached: the builtin answers first, every time. That is why
/// [`crate::plugins::validate_manifest`] refuses such a manifest by name
/// rather than installing a capability nothing can call.
///
/// `delegate` is on the list even though it is declared only where a
/// delegation gate exists, because its match arm is unconditional — a gateless
/// session answers it with a refusal, which shadows a plugin just as
/// completely. `every_declared_tool_has_an_arm_that_answers_it` keeps this
/// list and the arms in step.
pub const BUILTIN_TOOL_NAMES: [&str; 10] = [
    "read",
    "write",
    "edit",
    "bash",
    "shell",
    "skill",
    "checkpoint",
    "openagents",
    "capability",
    "delegate",
];

/// A tool the front-end driving the session answers itself.
///
/// The nine tools above are every session's, and they stay here. A front-end
/// can have a capability no other caller has — coder-lite's ACP path, which
/// hands a task to a coding agent installed on this machine, is the one this
/// exists for — and it belongs in the same declaration the other five are in,
/// because the model reads one list and the system prompt counts it.
///
/// The handler is given the whole [`ToolCall`] rather than just its arguments
/// so it can stream what it is doing against the call's own id while it runs.
/// A name that collides with a built-in is refused at registration: a host
/// tool that shadowed `shell` would be a tool the model believes it knows the
/// behaviour of and does not.
pub struct HostTool {
    pub definition: ToolDefinition,
    pub run: HostToolFn,
}

/// What a host tool does, and whether it worked.
pub type HostToolFn = Arc<
    dyn Fn(
            &ToolCall,
            watch::Receiver<bool>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = (String, bool)> + Send>>
        + Send
        + Sync,
>;

/// The stable tool result emitted when a turn stops an in-flight tool.
///
/// JSON makes the outcome machine-readable in the transcript and ATIF export.
pub const CANCELLED_TOOL_RESULT: &str = r#"{"status":"cancelled","reason":"turn_cancelled"}"#;

/// The largest index at or below `max` that is a character boundary in `text`.
///
/// Slicing a `String` by a byte index panics when the index lands inside a
/// multi-byte character, and truncating tool output at a fixed byte count does
/// exactly that the first time a command prints an accent or an emoji past the
/// limit. The panic took the whole agent process with it, before the thread
/// could even be revoked. `str::floor_char_boundary` is unstable, so this is
/// the same thing spelled out.
fn floor_char_boundary(text: &str, max: usize) -> usize {
    if max >= text.len() {
        return text.len();
    }
    let mut index = max;
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const MAXIMUM_TIMEOUT_SECS: u64 = 600;

/// A plan upsell is not work done.
///
/// An external agent that answers "upgrade your plan to continue" has
/// returned a string rather than performed the task, and reporting that as a
/// successful tool result is how a session comes to believe a file was edited
/// when nothing touched it.
fn is_refusal(answer: &str) -> bool {
    answer
        .to_lowercase()
        .contains("upgrade your plan to continue")
}

/// Child output with its terminal escape sequences removed.
///
/// A child process does not know it is being captured, and one with a pty or
/// an env that forces it colors and animates regardless: a test runner styles
/// `FAIL` red, a spinner rewrites its line with `\r`. Those bytes are written
/// for a terminal to *execute*. Inside the full-screen coder TUI, ratatui
/// paints the frame cell by cell and positions the cursor before each span —
/// an escape sequence embedded in span content runs after that positioning,
/// so the child's colors overwrite the palette and its cursor movements make
/// every later cell land where the child moved the cursor to. Overlapping
/// garbage and a dirty terminal: issue #193. Stripping at the capture
/// boundary covers every render path at once.
///
/// Hand-rolled rather than a dependency; the grammar consumed here is the
/// small part of ECMA-48 that captured output actually meets —
///
/// - CSI sequences (`ESC [` … one final byte in `@`..=`~`)
/// - string sequences: OSC, DCS, PM, APC (`ESC ]`, `ESC P`, `ESC ^`, `ESC _`
///   … terminated by BEL or by ST, `ESC \`)
/// - any other two-byte `ESC` sequence
/// - bare `\r`, which a progress bar uses to rewrite its own line; keeping
///   it would replay the rewrite against cells the TUI did not draw
///
/// Newlines and every ordinary byte survive untouched.
pub fn strip_terminal_escapes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            0x1b => match bytes.get(index + 1) {
                Some(b'[') => {
                    // CSI: parameter and intermediate bytes, then one final
                    // byte in `@`..=`~`. Unterminated at end of buffer, the
                    // partial sequence is dropped.
                    let mut cursor = index + 2;
                    while cursor < bytes.len() && !(0x40..=0x7e).contains(&bytes[cursor]) {
                        cursor += 1;
                    }
                    index = cursor + 1;
                }
                Some(b']') | Some(b'P') | Some(b'^') | Some(b'_') => {
                    // String sequence: scan for BEL or ST.
                    let mut cursor = index + 2;
                    loop {
                        match bytes.get(cursor) {
                            None => {
                                cursor = bytes.len();
                                break;
                            }
                            Some(0x07) => {
                                cursor += 1;
                                break;
                            }
                            Some(0x1b) if bytes.get(cursor + 1) == Some(&b'\\') => {
                                cursor += 2;
                                break;
                            }
                            Some(_) => cursor += 1,
                        }
                    }
                    index = cursor;
                }
                Some(_) => {
                    // `ESC x`, possibly with intermediate bytes before the
                    // final one: `ESC ( B` is three bytes, not two.
                    index += 2;
                    while index - 1 < bytes.len() && (0x20..=0x2f).contains(&bytes[index - 1]) {
                        index += 1;
                    }
                }
                None => index += 1,
            },
            b'\r' => {
                // `\r\n` is still a line ending; a bare `\r` is a rewrite.
                if bytes.get(index + 1) == Some(&b'\n') {
                    out.push(b'\n');
                    index += 2;
                } else {
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
    /// Whole milliseconds the call held the session. Set once per execution
    /// in [`HarnessToolRegistry::execute_tool_cancellable`], around
    /// everything the arm does, so a record says what a run cost without
    /// anyone reconstructing it from timestamps. Zero on the early-cancelled
    /// path: no work ran.
    #[serde(default)]
    pub duration_ms: u64,
}

/// One skill: a directory holding a `SKILL.md` with YAML front matter naming
/// it and saying when it applies, then a body of instructions.
///
/// The format is shared with the other agents that read this repository, so
/// the same file serves all of them and none of them owns it.
#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// The name the model asks for, from the front matter.
    pub name: String,
    /// When to use it, from the front matter. The sentence the model chooses
    /// on, and the reason a body is never in the catalog.
    pub description: String,
    /// The instructions, front matter removed.
    pub body: String,
    /// Whether the body is put in front of the model without being asked for.
    ///
    /// Set with `auto: true` in the front matter. The catalog exists so a body
    /// is read only when it is wanted; a skill that says how to approach the
    /// work is the exception, because a session needs the method before its
    /// first decision and will not think to ask for it. Every auto-loaded body
    /// is paid for on every turn, so it is used sparingly.
    pub auto: bool,
    /// Where it was read from, so a reader can open it.
    pub path: PathBuf,
}

/// How much of one skill body is handed back.
pub const SKILL_BODY_LIMIT: usize = 32_000;

/// What the `delegate` tool is allowed to start.
///
/// Present on the session the reader is talking to and absent on the children
/// it starts. A fan-out whose children fan out has no ceiling: three children
/// each starting three is nine agents on one grant, and none of them told the
/// reader.
#[derive(Debug, Clone)]
pub struct DelegationGate {
    /// The lane children run on.
    pub lane: String,
    /// The credential children spend against.
    pub user_token: Option<String>,
    /// The inference API the parent uses. A Coder Mini child uses the same
    /// endpoint instead of rediscovering process-global configuration.
    pub api_base: Option<String>,
    /// The most children one call may start.
    pub max_count: usize,
    /// The `--child-*` flags the session was started with.
    ///
    /// Carried here so `/delegate` and the `delegate` tool start children the
    /// same way `--delegate` does. Without it `oa coder --child-config f`
    /// parsed, said nothing, and started a child that never saw the file —
    /// which is the only route a provider credential has to one, since this
    /// CLI stores none.
    pub child: crate::delegate::ChildOptions,
    /// The ACP agents installed on this machine, from
    /// [`crate::coder::acp::find_agents`]. Naming one of these in a `delegate`
    /// call hands the whole task to that program over the Agent Client
    /// Protocol — on its own credentials and its own bill, not the session's —
    /// so one such call is the per-turn limit. Empty on a machine with no ACP
    /// agent, which is also when the external-agent language leaves the
    /// declaration: a capability that does not exist here is not advertised.
    pub acp_agents: Vec<Agent>,
    /// Whether this user turn has already handed work to an external ACP
    /// agent. Claimed before a call's arguments are read, so a malformed
    /// second call cannot spend the turn's one external delegation on an
    /// error message; cleared at the top of every turn by
    /// [`crate::coder::runtime::Session::execute_turn`].
    pub acp_spent: Arc<AtomicBool>,
}

/// How a tool arm reaches the turn's Control sink mid-run.
///
/// Attached by [`crate::coder::runtime::Session::open_at`] so `delegate` can
/// stream child activity into the parent box. Absent in headless and child
/// registries: those callers only need the final [`ToolOutput`].
pub type ToolEventSink = Arc<dyn Fn(Control) + Send + Sync>;

/// Branch and directory slug for a Coder Mini isolation worktree.
fn agent_worktree_slug(call_id: &str) -> String {
    let compact: String = call_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let compact = if compact.is_empty() {
        "agent".to_string()
    } else {
        compact.to_ascii_lowercase()
    };
    let ordinal = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("agent-{compact}-{ordinal:08x}")
}

/// The `· ` prefix the TUI draws on a delegated agent's activity line.
fn prefix_subagent_line(line: &str) -> String {
    let line = line.trim_end();
    if line.is_empty() {
        String::new()
    } else if line.starts_with("· ") {
        line.to_string()
    } else {
        format!("· {line}")
    }
}

/// Split streamed chunks into complete lines, flushing the remainder on demand.
struct LineBuffer {
    pending: String,
}

impl LineBuffer {
    fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    fn push_lines(&mut self, chunk: &str, mut emit: impl FnMut(String)) {
        self.pending.push_str(chunk);
        while let Some(idx) = self.pending.find('\n') {
            let line = self.pending[..idx].trim_end().to_string();
            self.pending.drain(..=idx);
            if !line.is_empty() {
                emit(line);
            }
        }
    }

    fn flush(&mut self, mut emit: impl FnMut(String)) {
        let line = std::mem::take(&mut self.pending);
        let line = line.trim_end();
        if !line.is_empty() {
            emit(line.to_string());
        }
    }

    fn discard(&mut self) {
        self.pending.clear();
    }
}

fn installed_agent_ids(gate: &DelegationGate) -> Vec<String> {
    let mut ids = agents::BUILTIN_AGENTS
        .iter()
        .map(|agent| agent.id.to_string())
        .collect::<Vec<_>>();
    for agent in &gate.acp_agents {
        if !ids.iter().any(|id| id == &agent.id) {
            ids.push(agent.id.clone());
        }
    }
    ids
}

pub struct HarnessToolRegistry {
    pub cwd: PathBuf,
    /// Where this session's command logs land, when a local session store
    /// exists. `None` keeps `shell` writing nothing to disk; a path makes
    /// every run's full output addressable for the rest of the session (#152).
    pub session_log_dir: Option<PathBuf>,
    /// Discovered skills by name, in catalog order.
    pub skills: BTreeMap<String, SkillInfo>,
    /// `None` on a delegated child, so it cannot delegate further.
    pub delegation: Option<DelegationGate>,
    /// The digest-pinned WebAssembly plugins installed at or above `cwd`.
    pub catalog: Vec<CatalogEntry>,
    /// Which capability tiers may load in this session. Pure compute always
    /// may; read-only mounts need an operator and refuse without one.
    pub plugin_approval: Approval,
    /// Plugins the model loaded through `capability`, in load order. Each one
    /// declares a further tool under its own manifest name.
    loaded: Mutex<Vec<Arc<LoadedPlugin>>>,
    /// Tools the front-end driving this session answers itself.
    host: Vec<HostTool>,
    /// Where long-running commands keep whole output: the session record's
    /// own directory, when the session has one. `None` keeps the old
    /// truncate-and-drop behaviour — delegated children, one-shot calls.
    session_dir: Option<PathBuf>,
    /// The repository's declared check scopes, from
    /// `.openagents/checks.json`. `None` means the repository has not opted
    /// in and the `check` tool is not declared.
    check_scopes: Option<crate::checks::ChecksConfig>,
    /// The declarations an in-process delegated agent receives.
    tool_pool: ToolPool,
    /// The turn's Control sink, when this registry belongs to an interactive
    /// Coder session. `delegate` uses it to stream into the parent box.
    event_sink: Option<ToolEventSink>,
    /// Last ACP session id per agent id, so a later call to the same agent
    /// on this registry can resume.
    acp_sessions: Mutex<BTreeMap<String, String>>,
}

impl HarnessToolRegistry {
    pub fn new(cwd: Option<PathBuf>) -> Self {
        Self::build(cwd, None, ToolPool::All)
    }

    /// The registry a session gets when it may start children.
    pub fn with_delegation(cwd: Option<PathBuf>, gate: DelegationGate) -> Self {
        Self::build(cwd, Some(gate), ToolPool::All)
    }

    /// The registry a delegated child gets: rooted at the child's own
    /// directory, and with no `delegate` tool.
    pub fn child(cwd: Option<PathBuf>) -> Self {
        Self::build(cwd, None, ToolPool::All)
    }

    /// A fresh in-process delegated agent with a constructor-selected pool.
    pub fn with_tool_pool(cwd: Option<PathBuf>, pool: ToolPool) -> Self {
        Self::build(cwd, None, pool)
    }

    /// Give tool arms the turn's Control sink so they can stream mid-run.
    pub fn with_event_sink(mut self, sink: ToolEventSink) -> Self {
        self.event_sink = Some(sink);
        self
    }

    fn emit_control(&self, event: Control) {
        if let Some(sink) = &self.event_sink {
            sink(event);
        }
    }

    fn emit_subagent_line(&self, call_id: &str, line: impl AsRef<str>) {
        let line = prefix_subagent_line(line.as_ref());
        if line.is_empty() {
            return;
        }
        self.emit_control(Control::SubagentOutput {
            call_id: call_id.to_string(),
            line,
        });
    }

    /// Point this session's shell logs at one directory, created on first
    /// run rather than here: a session that never shells out never makes a
    /// directory.
    pub fn with_session_log_dir(mut self, dir: PathBuf) -> Self {
        self.session_log_dir = Some(dir);
        self
    }

    /// The directory this session's shell logs land in, when one is set.
    pub fn session_log_dir(&self) -> Option<&Path> {
        self.session_log_dir.as_deref()
    }

    /// Grant the read-only mount tier, for a caller with an operator behind
    /// it. Without this a plugin that declares mounts refuses to load, which
    /// is the safe default for an unattended session.
    pub fn allowing_plugin_mounts(mut self) -> Self {
        self.plugin_approval.mounts_allowed = true;
        self
    }

    /// Keep whole output of long commands under this directory.
    ///
    /// The session hands over its own record directory, so the logs live with
    /// everything else that survives a resume rather than in a scratch space
    /// nothing can find later.
    pub fn keeping_session_logs(mut self, dir: PathBuf) -> Self {
        self.session_dir = Some(dir);
        self
    }

    /// The in-place form of [`Self::keeping_session_logs`], for a registry
    /// already held behind a constructed session.
    pub fn keeping_session_logs_in_place(&mut self, dir: PathBuf) {
        self.session_dir = Some(dir);
    }

    fn build(
        cwd: Option<PathBuf>,
        delegation: Option<DelegationGate>,
        tool_pool: ToolPool,
    ) -> Self {
        let root =
            cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let catalog = plugins::discover_catalog(&root);
        // Declared scopes load here: discovery is a read of the repository
        // the session is rooted in, no cheaper moment exists, and a registry
        // without them simply declares no `check` tool.
        let check_scopes = crate::checks::ChecksConfig::load(&root).unwrap_or(None);
        let mut registry = Self {
            cwd: root,
            session_log_dir: None,
            skills: BTreeMap::new(),
            delegation,
            catalog,
            plugin_approval: Approval::default(),
            loaded: Mutex::new(Vec::new()),
            host: Vec::new(),
            session_dir: None,
            check_scopes,
            tool_pool,
            event_sink: None,
            acp_sessions: Mutex::new(BTreeMap::new()),
        };
        registry.load_local_skills();
        registry
    }

    /// Where skills live, nearest first. The first source to claim a name
    /// keeps it, so a repository or a person replaces a shipped skill by
    /// writing one of the same name and nothing has to be uninstalled.
    fn skill_directories(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.cwd.join(".agents").join("skills")];
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join(".agents").join("skills"));
        }
        // Shipped skills live outside any retired implementation package.
        dirs.push(self.cwd.join("skills"));
        dirs
    }

    /// Read every skill this workspace offers.
    ///
    /// A directory that is missing, unreadable, or holds no `SKILL.md`
    /// contributes nothing: a skills directory is optional, and a session in a
    /// repository without one is a session with no skills rather than a
    /// session that failed to start. A `SKILL.md` with no `name` cannot be
    /// asked for and one with no `description` gives the model nothing to
    /// choose on, so both are required and a file missing either is skipped.
    pub fn load_local_skills(&mut self) {
        for dir in self.skill_directories() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            let mut names: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            names.sort();
            for path in names {
                let skill_md = path.join("SKILL.md");
                if !skill_md.is_file() {
                    continue;
                }
                let Ok(content) = fs::read_to_string(&skill_md) else {
                    continue;
                };
                let Some((name, description, auto)) = parse_skill_front_matter(&content) else {
                    continue;
                };
                if self.skills.contains_key(&name) {
                    continue;
                }
                self.skills.insert(
                    name.clone(),
                    SkillInfo {
                        name,
                        description,
                        body: skill_body(&content),
                        auto,
                        path: skill_md,
                    },
                );
            }
        }
    }

    /// The standing context a session starts with: every auto-loaded skill
    /// body, in one block, plus what this workspace is when it is one of the
    /// two OpenAgents repositories.
    ///
    /// `None` when there is none, so a caller adds nothing rather than an
    /// empty heading.
    pub fn standing_context(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(workspace) = openagents_workspace_note(&self.cwd) {
            parts.push(workspace);
        }
        for skill in self.skills.values() {
            if !skill.auto {
                continue;
            }
            parts.push(format!(
                "The `{}` skill, which applies to this session:\n\n{}",
                skill.name, skill.body
            ));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    /// Declare a tool the caller answers itself.
    ///
    /// Three names are refused rather than accepted-and-shadowed, because in
    /// each case one of the two answers could never be reached and the model
    /// would be told a capability exists that does something else:
    ///
    /// - one of [`BUILTIN_TOOL_NAMES`], which this module answers first;
    /// - a name another host tool already holds;
    /// - a name a plugin in this session's catalog claims. A plugin is
    ///   dispatched from the fallthrough arm *below* the host tools, so a host
    ///   tool of the same name would shadow it exactly as a builtin would.
    ///   [`crate::plugins::validate_manifest`] cannot see this collision — it
    ///   validates a manifest on disk, long before a front end decides what to
    ///   declare — so it is caught here, where the catalog and the host tool
    ///   are both known.
    ///
    /// The caller is expected to say so rather than swallow the refusal: a
    /// capability that quietly failed to register is one the reader believes
    /// they have.
    pub fn add_host_tool(&mut self, tool: HostTool) -> Result<(), String> {
        let name = tool.definition.name.clone();
        if BUILTIN_TOOL_NAMES.contains(&name.as_str()) {
            return Err(format!(
                "`{name}` is one of this session's own tools and cannot be replaced by a host \
                 tool. The reserved names are {}.",
                BUILTIN_TOOL_NAMES.join(", ")
            ));
        }
        if self.host.iter().any(|held| held.definition.name == name) {
            return Err(format!("a host tool named `{name}` is already declared"));
        }
        if let Some(entry) = self.catalog.iter().find(|entry| entry.name == name) {
            return Err(format!(
                "a plugin named `{name}` is installed at {}, and a host tool of that name would \
                 shadow it. Rename one of them.",
                entry.manifest_path.display()
            ));
        }
        self.host.push(tool);
        Ok(())
    }

    /// Remove a front-end tool when the state that made it available is gone.
    pub fn remove_host_tool(&mut self, name: &str) -> bool {
        let before = self.host.len();
        self.host.retain(|tool| tool.definition.name != name);
        self.host.len() != before
    }

    /// The plugins loaded into this session so far.
    pub fn loaded_plugins(&self) -> Vec<Arc<LoadedPlugin>> {
        self.loaded
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut skill_list = String::new();
        for (name, info) in &self.skills {
            skill_list.push_str(&format!("\n- `{}`: {}", name, info.description));
        }

        let mut tools = vec![
            ToolDefinition {
                name: "read".to_string(),
                description: text::RUST_READ.to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "The file to read, relative to the working directory or an absolute path."}
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write".to_string(),
                description: text::RUST_WRITE.to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "The file to write, relative to the working directory or an absolute path."},
                        "content": {"type": "string", "description": "The complete new contents of the file."}
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "edit".to_string(),
                description: text::RUST_EDIT.to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "The file to edit, relative to the working directory or an absolute path."},
                        "oldText": {"type": "string", "description": "The exact text to replace. It must appear in the file exactly once."},
                        "newText": {"type": "string", "description": "What to put in its place. Empty deletes the old text."},
                        "edits": {"type": "array", "description": "Several edits to one file, applied in order; all land or none do, and one call prices one against the turn budget. Each element is an object with `oldText` and `newText`; `oldText`/`newText` are not used when this is present.", "items": {"type": "object", "properties": {"oldText": {"type": "string"}, "newText": {"type": "string"}}, "required": ["oldText", "newText"]}}
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "bash".to_string(),
                description: text::RUST_BASH.replace("{cwd}", &self.cwd.display().to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "The command line to run through /bin/sh -c."},
                        "timeout_seconds": {"type": "integer", "description": "How long to wait. Defaults to 120; raise for a build or test run."}
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "shell".to_string(),
                description: text::RUST_SHELL.replace("{cwd}", &self.cwd.display().to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "The command line to run through /bin/sh -c."},
                        "timeout_seconds": {"type": "integer", "description": "How long to wait. Defaults to 120; raise for a build or test run."}
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "skill".to_string(),
                description: text::RUST_SKILL.replace("{skills}", &skill_list),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "The skill to read."}
                    },
                    "required": ["name"]
                }),
            },
            ToolDefinition {
                name: "checkpoint".to_string(),
                description: text::RUST_CHECKPOINT.to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "The note: which issue or task, what landed (files, commits, test results), what is unfinished or broken, and the exact next step. A few sentences."}
                    },
                    "required": ["text"]
                }),
            },
            ToolDefinition {
                name: "openagents".to_string(),
                description: text::RUST_OPENAGENTS.to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "args": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "The arguments after openagents as a list."
                        }
                    },
                    "required": ["args"]
                }),
            },
        ];

        // The standing capability tool. Constant-size: it names no installed
        // plugin, so the declaration does not grow as the catalog does.
        tools.push(capability_tool_definition());

        // Declared only where the repository opted in: a repo without
        // `.openagents/checks.json` has no scopes to name, and a `check`
        // tool that always refuses is friction, not a guardrail.
        if let Some(config) = &self.check_scopes {
            tools.push(ToolDefinition {
                name: "check".to_string(),
                description: check_tool_description(&config.scopes),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Which declared scope to run. Defaults to the narrowest one when omitted."
                        }
                    }
                }),
            });
        }

        // Declared only where it can be run. A child's registry has no gate,
        // so a child neither sees the tool nor can call it.
        if let Some(gate) = &self.delegation {
            let resolved_child = crate::delegate::ChildLane::resolve_for_session(&gate.lane)
                .unwrap_or(crate::delegate::ChildLane::OpenAgents);
            // The external agents are named through the surface's
            // `{external}` placeholder, filled here from what is installed.
            // A dynamic schema cannot go in a builtin's constant parameter
            // block, so the description is where the ids live; an empty
            // installed list fills the placeholder with nothing, so a machine
            // with no ACP agent is not told it has one.
            let external = if gate.acp_agents.is_empty() {
                String::new()
            } else {
                let listed = gate
                    .acp_agents
                    .iter()
                    .map(|agent| format!("`{}` ({})", agent.id, agent.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    " One call may instead hand the whole task to one coding agent installed on \
                     this machine, over the Agent Client Protocol: pass `agent` with one of \
                     {listed} and the task runs in that program with its own tools, its own \
                     credentials, and its own bill — count is then forced to 1, and one \
                     external agent is the per-turn limit. Prefer `shell` for a single command \
                     — an ACP agent is for work worth a whole agent."
                )
            };
            tools.push(ToolDefinition {
                name: "delegate".to_string(),
                description: text::RUST_DELEGATE
                    .replace("{lane}", &resolved_child.label())
                    .replace("{max_count}", &gate.max_count.to_string())
                    .replace("{external}", &external),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The complete, self-contained instruction every child performs. Name the files, the command, and what to report back."
                        },
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": gate.max_count,
                            "description": "How many children run this prompt. Defaults to 1. Ignored when `agent` is set."
                        },
                        "agent": {
                            "type": "string",
                            "description": "The built-in `coder-mini`, `explore`, or `coder` agent, or one installed ACP agent. Omit this field to keep the existing fan-out behavior."
                        },
                        "description": {
                            "type": "string",
                            "description": "An optional 3–5 word label for the tool-call header."
                        },
                        "tools": {
                            "type": "string",
                            "enum": ["read-only", "read-write", "all"],
                            "description": "The Coder Mini tool pool. Defaults to `read-only`; ACP agents ignore it."
                        },
                        "model": {
                            "type": "string",
                            "description": "Optional catalog id for Coder Mini only, resolved the same way as `--model`. ACP agents and fan-out ignore it. An id this deployment does not serve is refused before any child starts."
                        },
                        "isolation": {
                            "type": "string",
                            "enum": ["worktree"],
                            "description": "On Coder Mini with a read-write tool pool, `worktree` runs the agent in a temporary git worktree of this checkout on an `agent-*` branch. Unchanged worktrees are removed; changed ones are kept and named in the result. Read-only runs ignore this. Mutually exclusive with a cwd override, which is not accepted yet."
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["read-only", "prompt", "dangerous"],
                            "description": "With `agent`: how much it may do unattended. Omit to leave the agent's own default; `read-only` for a look that changes nothing."
                        }
                    },
                    "required": ["prompt"]
                }),
            });
        }

        // Whatever the front-end answers itself.
        for tool in &self.host {
            tools.push(tool.definition.clone());
        }

        // A plugin the model loaded through `capability` declares a tool of
        // its own, under its manifest name and over its manifest's input
        // schema. Nothing appears here that has not been digest-verified,
        // import-inspected, and instantiated at least once at load.
        for plugin in self.loaded_plugins() {
            tools.push(plugin_tool_definition(&plugin));
        }

        tools
            .into_iter()
            .filter(|tool| self.tool_pool.allows(&tool.name))
            .collect()
    }

    pub async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        let (_keep_open, cancel) = watch::channel(false);
        self.execute_tool_cancellable(call, cancel).await
    }

    /// Execute one tool under the turn's cancellation signal.
    pub async fn execute_tool_cancellable(
        &self,
        call: &ToolCall,
        cancel: watch::Receiver<bool>,
    ) -> ToolOutput {
        let started = std::time::Instant::now();
        let mut output = self.execute_tool_inner(call, cancel).await;
        output.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        output
    }

    async fn execute_tool_inner(
        &self,
        call: &ToolCall,
        mut cancel: watch::Receiver<bool>,
    ) -> ToolOutput {
        if *cancel.borrow() {
            return ToolOutput {
                call_id: call.id.clone(),
                output: CANCELLED_TOOL_RESULT.to_string(),
                is_error: true,
                duration_ms: 0,
            };
        }
        if !self.tool_pool.allows(&call.name) {
            return ToolOutput {
                call_id: call.id.clone(),
                output: format!(
                    "Tool `{}` is not available in the {} delegated tool pool.",
                    call.name,
                    self.tool_pool.name()
                ),
                is_error: true,
                duration_ms: 0,
            };
        }
        match call.name.as_str() {
            "read" => {
                let (output, is_error) = answer_read(&self.cwd, &call.arguments);
                ToolOutput {
                    call_id: call.id.clone(),
                    output,
                    is_error,
                    duration_ms: 0,
                }
            }
            "write" => {
                let (output, is_error) = answer_write(&self.cwd, &call.arguments);
                ToolOutput {
                    call_id: call.id.clone(),
                    output,
                    is_error,
                    duration_ms: 0,
                }
            }
            "edit" => {
                let (output, is_error) = answer_edit(&self.cwd, &call.arguments);
                ToolOutput {
                    call_id: call.id.clone(),
                    output,
                    is_error,
                    duration_ms: 0,
                }
            }
            // One arm for both names: `bash` is the name pi's tool set gives
            // this, `shell` is the name this session has always given it, and
            // two implementations would be two behaviours the moment either
            // one was touched.
            "shell" | "bash" => {
                let cmd = call
                    .arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let timeout_secs = call
                    .arguments
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(DEFAULT_TIMEOUT_SECS)
                    .min(MAXIMUM_TIMEOUT_SECS);

                if let Some(refusal) = check_shell_refusal(cmd) {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: refusal,
                        is_error: true,
                        duration_ms: 0,
                    };
                }

                if let Some(refusal) = check_duplicate_execution(cmd) {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: refusal,
                        is_error: true,
                        duration_ms: 0,
                    };
                }

                let (output_str, failed) = run_real_shell_logged(
                    cmd,
                    &self.cwd,
                    timeout_secs,
                    &mut cancel,
                    self.session_dir.as_deref(),
                )
                .await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: failed,
                    duration_ms: 0,
                }
            }
            "skill" => {
                let name = call
                    .arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if let Some(skill_info) = self.skills.get(name) {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: render_skill(skill_info),
                        is_error: false,
                        duration_ms: 0,
                    }
                } else {
                    ToolOutput {
                        call_id: call.id.clone(),
                        output: format!("Skill '{}' not found.", name),
                        is_error: true,
                        duration_ms: 0,
                    }
                }
            }
            "checkpoint" => {
                let (output, is_error) = answer_checkpoint(&call.arguments);
                ToolOutput {
                    call_id: call.id.clone(),
                    output,
                    is_error,
                    duration_ms: 0,
                }
            }
            "openagents" => {
                // Every element must be a string. Silent coercion (dropping a
                // non-string) once turned `["issue","comment","178",
                // "--body-file","-",-999]` into a body-less comment call whose
                // stdin read hung the turn (#180) — so refuse the whole call
                // instead of running a command the model did not write.
                let args_result = call
                    .arguments
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|v| v.as_str().map(String::from).ok_or_else(|| v.to_string()))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()));

                let args_array = match args_result {
                    Ok(args) => args,
                    Err(value) => {
                        return ToolOutput {
                            call_id: call.id.clone(),
                            output: format!(
                                "Bad tool call: every element of `args` must be a string, \
                                 but one element was {value}. Nothing was run."
                            ),
                            is_error: true,
                            duration_ms: 0,
                        };
                    }
                };

                let (output_str, failed) = run_openagents_cli(&args_array, &mut cancel).await;
                ToolOutput {
                    call_id: call.id.clone(),
                    output: output_str,
                    is_error: failed,
                    duration_ms: 0,
                }
            }
            "delegate" => {
                let Some(gate) = &self.delegation else {
                    // Reachable only if a model invents the name, since the
                    // tool is not declared without a gate.
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: "This session cannot start child agents.".to_string(),
                        is_error: true,
                        duration_ms: 0,
                    };
                };

                // Claimed before anything about the call is read, so a
                // malformed second call cannot spend the turn's one external
                // delegation on an error message. The cap is scoped to
                // external agents: it exists because one turn once carried
                // twenty-four consecutive hand-offs to agents on somebody's
                // bill, and plain fan-out on the session's own grant is
                // already bounded by `max_count`.
                if let Some(wanted) = call
                    .arguments
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    if let Some(agent) = agents::find(wanted) {
                        return self
                            .delegate_to_builtin_agent(call, agent, gate, cancel)
                            .await;
                    }
                    if !gate.acp_agents.iter().any(|agent| agent.id == wanted) {
                        return ToolOutput {
                            call_id: call.id.clone(),
                            output: format!(
                                "No agent named `{wanted}` is installed here. Installed: {}.",
                                installed_agent_ids(gate).join(", ")
                            ),
                            is_error: true,
                            duration_ms: 0,
                        };
                    }
                    if gate.acp_spent.swap(true, Ordering::SeqCst) {
                        return ToolOutput {
                            call_id: call.id.clone(),
                            output: "This turn has already handed work to an agent, and one is the limit: a \
                                     second agent is a second bill for the same request. Answer with what the \
                                     first one returned, or ask for another turn."
                                .to_string(),
                            is_error: true,
                            duration_ms: 0,
                        };
                    }
                    return self.delegate_to_acp_agent(call, wanted, gate, cancel).await;
                }

                let prompt = call
                    .arguments
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if prompt.is_empty() {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: "No children were started: `prompt` is required and must say what the child does.".to_string(),
                        is_error: true,
                        duration_ms: 0,
                    };
                }

                let count = call
                    .arguments
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, gate.max_count as u64) as usize;

                let report = crate::delegate::fanout_for_tool_cancellable(
                    &prompt,
                    count,
                    &gate.lane,
                    gate.user_token.clone(),
                    gate.child.clone(),
                    Some(self.cwd.clone()),
                    cancel,
                )
                .await;

                ToolOutput {
                    call_id: call.id.clone(),
                    output: report,
                    is_error: false,
                    duration_ms: 0,
                }
            }
            "capability" => {
                let (text, loaded) = answer_capability(
                    &self.catalog,
                    self.plugin_approval,
                    &self.cwd,
                    &call.arguments,
                );
                // A refusal is text the model can act on, not an error the
                // turn dies of; the only `is_error` here is the absence of a
                // plugin where one was named.
                let is_error = loaded.is_none()
                    && call
                        .arguments
                        .get("name")
                        .and_then(|v| v.as_str())
                        .is_some();
                if let Some(plugin) = loaded {
                    if let Ok(mut held) = self.loaded.lock() {
                        held.retain(|existing| existing.manifest.name != plugin.manifest.name);
                        held.push(Arc::new(plugin));
                    }
                }
                ToolOutput {
                    call_id: call.id.clone(),
                    output: text,
                    is_error,
                    duration_ms: 0,
                }
            }
            "check" => {
                let Some(config) = &self.check_scopes else {
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output: "This repository declares no check scopes (no \
                                 `.openagents/checks.json`). Use `shell` directly."
                            .to_string(),
                        is_error: true,
                        duration_ms: 0,
                    };
                };
                let requested = call
                    .arguments
                    .get("scope")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(crate::checks::DEFAULT_SCOPE);
                let scope = match config.scope(requested) {
                    Ok(scope) => scope,
                    Err(error) => {
                        return ToolOutput {
                            call_id: call.id.clone(),
                            output: error,
                            is_error: true,
                            duration_ms: 0,
                        };
                    }
                };
                let mut report = String::new();
                let mut failed = false;
                for (index, command) in scope.run.iter().enumerate() {
                    if let Some(refusal) = check_shell_refusal(command) {
                        report.push_str(&format!("[{index}] refused: {refusal}\n"));
                        failed = true;
                        break;
                    }
                    if let Some(refusal) = check_duplicate_execution(command) {
                        report.push_str(&format!("[{index}] refused: {refusal}\n"));
                        failed = true;
                        break;
                    }
                    let (output, is_error) = run_real_shell_logged(
                        command,
                        &self.cwd,
                        MAXIMUM_TIMEOUT_SECS,
                        &mut cancel,
                        self.session_dir.as_deref(),
                    )
                    .await;
                    report.push_str(&format!(
                        "[{index}] {} → {}\n{}\n",
                        command,
                        if is_error { "FAILED" } else { "ok" },
                        output
                    ));
                    if is_error {
                        failed = true;
                        // Attribution against the baseline: inherited or
                        // new, answered from the record rather than by
                        // re-running anything.
                        let baseline = self
                            .session_dir
                            .as_deref()
                            .map(crate::checks::FailureBaseline::load)
                            .unwrap_or_default();
                        let known = baseline.is_known(requested, command);
                        report.push_str(&format!(
                            "This failure is {} (baseline: {} failures recorded for this \
                             scope).\n",
                            if known {
                                "inherited — it already failed on a clean tree"
                            } else {
                                "new — no clean-tree baseline failure for this command"
                            },
                            baseline.failures.len(),
                        ));
                        break;
                    }
                }
                ToolOutput {
                    call_id: call.id.clone(),
                    output: report,
                    is_error: failed,
                    duration_ms: 0,
                }
            }
            other => {
                // The front-end's own tools first: they were declared before
                // any plugin was loaded, and `add_host_tool` has already
                // refused a name that collides with a built-in.
                if let Some(tool) = self.host.iter().find(|t| t.definition.name == other) {
                    let (output, is_error) = (tool.run)(call, cancel).await;
                    return ToolOutput {
                        call_id: call.id.clone(),
                        output,
                        is_error,
                        duration_ms: 0,
                    };
                }
                // A loaded plugin answers under its own manifest name.
                let plugin = self
                    .loaded_plugins()
                    .into_iter()
                    .find(|plugin| plugin.manifest.name == other);
                match plugin {
                    Some(plugin) => {
                        let arguments = call.arguments.clone();
                        let output = tokio::select! {
                            output = plugins::run_plugin_text(plugin, &arguments) => output,
                            changed = cancel.changed() => {
                                if changed.is_ok() && *cancel.borrow() {
                                    CANCELLED_TOOL_RESULT.to_string()
                                } else {
                                    "The capability cancellation channel closed.".to_string()
                                }
                            }
                        };
                        ToolOutput {
                            call_id: call.id.clone(),
                            is_error: output == CANCELLED_TOOL_RESULT,
                            output,
                            duration_ms: 0,
                        }
                    }
                    None => ToolOutput {
                        call_id: call.id.clone(),
                        output: format!("Unknown tool: {}", call.name),
                        is_error: true,
                        duration_ms: 0,
                    },
                }
            }
        }
    }

    fn delegate_to_builtin_agent<'a>(
        &'a self,
        call: &'a ToolCall,
        agent: &'static AgentDefinition,
        gate: &'a DelegationGate,
        cancel: watch::Receiver<bool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolOutput> + Send + 'a>> {
        Box::pin(async move {
            let make = |output: String, is_error: bool, duration_ms: u64| ToolOutput {
                call_id: call.id.clone(),
                output,
                is_error,
                duration_ms,
            };
            let Some(prompt) = call
                .arguments
                .get("prompt")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return make(
                    "No task was given. `prompt` is required and must say what the agent does."
                        .to_string(),
                    true,
                    0,
                );
            };

            let pool = if agent.id == "coder-mini" {
                match call
                    .arguments
                    .get("tools")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    None => agent.pool,
                    Some(value) => match ToolPool::parse(value) {
                        Some(pool) => pool,
                        None => {
                            return make(
                                format!(
                                    "`{value}` is not a Coder Mini tool pool. Use `read-only`, \
                                     `read-write`, or `all`."
                                ),
                                true,
                                0,
                            );
                        }
                    },
                }
            } else {
                agent.pool
            };

            if let Some(value) = call
                .arguments
                .get("isolation")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if value != "worktree" {
                    return make(
                        format!(
                            "`{value}` is not an isolation mode. Use `worktree`, or omit it. \
                             A cwd override is not accepted yet and cannot be combined with isolation."
                        ),
                        true,
                        0,
                    );
                }
            }

            let lane = if agent.id == "coder-mini" {
                match call
                    .arguments
                    .get("model")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    Some(model) => Lane::from_str(model),
                    None => Lane::from_str(&gate.lane),
                }
            } else {
                Lane::from_str(&gate.lane)
            };

            let probe_tools = HarnessToolRegistry::with_tool_pool(Some(self.cwd.clone()), pool);
            let probe = crate::runtime::CoderRuntimeSession::new(
                lane.clone(),
                gate.api_base.clone(),
                gate.user_token.clone(),
                probe_tools,
            );
            if let Err(error) = probe.ensure_named_served().await {
                return make(error.to_string(), true, 0);
            }

            let wants_worktree = matches!(pool, ToolPool::ReadWrite | ToolPool::All)
                && call
                    .arguments
                    .get("isolation")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    == Some("worktree");
            let workspace = if wants_worktree {
                let slug = agent_worktree_slug(&call.id);
                match workspace::create_agent_worktree(&self.cwd, &slug).await {
                    Ok(workspace) => Some(workspace),
                    Err(error) => return make(error, true, 0),
                }
            } else {
                None
            };
            let child_cwd = workspace
                .as_ref()
                .map(|ws| ws.path.clone())
                .unwrap_or_else(|| self.cwd.clone());

            let tools = HarnessToolRegistry::with_tool_pool(Some(child_cwd), pool);
            let tool_names = tools
                .list_tools()
                .into_iter()
                .map(|tool| tool.name)
                .collect::<Vec<_>>();
            let tool_sink = self.event_sink.clone();
            let stream_sink = self.event_sink.clone();
            let tool_parent = call.id.clone();
            let stream_parent = call.id.clone();
            let content_buf = Arc::new(Mutex::new(LineBuffer::new()));
            let tool_uses = Arc::new(AtomicUsize::new(0));
            let counted_uses = Arc::clone(&tool_uses);
            let mut runtime = crate::runtime::CoderRuntimeSession::new(
                lane,
                gate.api_base.clone(),
                gate.user_token.clone(),
                tools,
            )
            .observing_tools(Arc::new(move |event: ToolEvent| {
                match event {
                    ToolEvent::Started {
                        name, arguments, ..
                    } => {
                        counted_uses.fetch_add(1, Ordering::SeqCst);
                        let Some(sink) = &tool_sink else {
                            return;
                        };
                        sink(Control::Tool {
                            call_id: tool_parent.clone(),
                            name,
                            arguments,
                        });
                    }
                    ToolEvent::Finished { .. } => {
                        // Do not emit ToolDone with the parent id: that would
                        // settle the delegate box before the agent finished.
                    }
                }
            }))
            .observing_stream(Arc::new(move |event| {
                let Some(sink) = &stream_sink else {
                    return;
                };
                let emit_line = |line: String| {
                    let line = prefix_subagent_line(&line);
                    if line.is_empty() {
                        return;
                    }
                    sink(Control::SubagentOutput {
                        call_id: stream_parent.clone(),
                        line,
                    });
                };
                match event {
                    ModelStreamEvent::ContentDelta(chunk) => {
                        if let Ok(mut buf) = content_buf.lock() {
                            buf.push_lines(&chunk, emit_line);
                        }
                    }
                    ModelStreamEvent::ReasoningDelta(_) => {
                        // A live #245 session filled the parent box and the
                        // ATIF export with the child's inner monologue, which
                        // clipped the tool lines the box exists to show.
                    }
                    ModelStreamEvent::ContentCommitted => {
                        if let Ok(mut buf) = content_buf.lock() {
                            buf.flush(emit_line);
                        }
                    }
                    ModelStreamEvent::ContentDiscarded => {
                        if let Ok(mut buf) = content_buf.lock() {
                            buf.discard();
                        }
                    }
                }
            }));
            let cancel_watch = cancel.clone();
            runtime.set_tool_cancellation(cancel);
            runtime.messages.push(crate::runtime::ChatMessage {
                role: "system".to_string(),
                content: Some(agents::system_prompt(agent, pool, &tool_names)),
                tool_calls: None,
                tool_call_id: None,
                images: Vec::new(),
            });

            let prompt = match &workspace {
                Some(ws) => format!(
                    "{prompt}\n\nYou are operating in an isolated git worktree at {}. Your paths are inside the worktree; re-read files you may have seen.",
                    ws.path.display()
                ),
                None => prompt.to_string(),
            };

            let started = Instant::now();
            let result = runtime.execute_turn(&prompt, |_| {}).await;
            let duration = started.elapsed();
            let counted = tool_uses.load(Ordering::SeqCst);
            let tool_uses = u64::try_from(counted.max(runtime.last_calls)).unwrap_or(u64::MAX);
            let answered_model = runtime.last_model.clone();
            let session_id = runtime.thread_id().map(str::to_string);
            let total_tokens = runtime.last_usage.total_tokens;
            let cancelled = *cancel_watch.borrow();
            let _ = runtime.finish().await;
            let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            let worktree = match workspace {
                Some(ws) => {
                    let line = ws.close_if_unchanged().await;
                    if line.starts_with("worktree kept: ") {
                        WorktreeOutcome::Kept(WorktreeRef {
                            path: ws.path.display().to_string(),
                            branch: ws.branch.clone(),
                        })
                    } else {
                        WorktreeOutcome::Removed
                    }
                }
                None => WorktreeOutcome::Unused,
            };

            let (status, report, is_error) = match result {
                Ok(report) if cancelled => {
                    (DelegateStatus::Cancelled, report.trim().to_string(), true)
                }
                Ok(report) => (DelegateStatus::Done, report.trim().to_string(), false),
                Err(error) if cancelled => (DelegateStatus::Cancelled, error.to_string(), true),
                Err(error) => (DelegateStatus::Failed, error.to_string(), true),
            };
            let record = DelegateAgentResult {
                status,
                agent: agent.id.to_string(),
                total_tool_uses: tool_uses,
                duration_ms,
                total_tokens,
                model: answered_model,
                session_id,
                report,
                worktree,
            };
            make(record.to_json(), is_error, duration_ms)
        })
    }

    /// Hand the whole task to one installed ACP agent: the `agent` parameter
    /// of `delegate`, which was the `acp` tool before the two surfaces
    /// merged. Every refusal here is one the old tool's tests pin, kept
    /// verbatim so the consolidation is a move and not a rewrite.
    ///
    /// Child activity streams into the parent box through [`Self::event_sink`].
    async fn delegate_to_acp_agent(
        &self,
        call: &ToolCall,
        wanted: &str,
        gate: &DelegationGate,
        mut cancel: watch::Receiver<bool>,
    ) -> ToolOutput {
        let agents = &gate.acp_agents;
        let make = |output: String, is_error: bool| ToolOutput {
            call_id: call.id.clone(),
            output,
            is_error,
            duration_ms: 0,
        };

        let Some(agent) = agents.iter().find(|a| a.id == wanted).cloned() else {
            return make(
                format!(
                    "No agent named `{wanted}` is installed here. Installed: {}.",
                    agents
                        .iter()
                        .map(|a| a.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                true,
            );
        };
        let Some(prompt) = call
            .arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
        else {
            return make(
                "No task was given. `prompt` is required and must say what the agent does."
                    .to_string(),
                true,
            );
        };
        // A mode this build does not know is refused by name rather than
        // quietly dropped: the reader asked for read-only and would
        // otherwise get whatever the agent's default is.
        let mode = match call
            .arguments
            .get("mode")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            None => None,
            Some(named) => match PermissionMode::parse(named) {
                Some(mode) => Some(mode),
                None => {
                    return make(
                        format!(
                            "`{named}` is not a mode. Use `read-only`, `prompt`, or \
                             `dangerous`, or omit it for the agent's own default."
                        ),
                        true,
                    );
                }
            },
        };

        self.emit_subagent_line(
            &call.id,
            format!("started on {wanted} in {}", self.cwd.display()),
        );
        let parent_call = call.id.clone();
        let event_sink = self.event_sink.clone();
        let text_buf = Arc::new(Mutex::new(LineBuffer::new()));
        let stream_buf = Arc::clone(&text_buf);
        let resume_session_id = self
            .acp_sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(wanted).cloned());
        let started = Instant::now();
        let result = AcpHarness {
            command: agent.command,
            args: agent.args,
            mode,
            resume_session_id,
            ..AcpHarness::default()
        }
        .run_detailed(
            &prompt,
            &self.cwd,
            move |event| {
                let Some(sink) = &event_sink else {
                    return;
                };
                let emit_line = |line: String| {
                    let line = prefix_subagent_line(&line);
                    if line.is_empty() {
                        return;
                    }
                    sink(Control::SubagentOutput {
                        call_id: parent_call.clone(),
                        line,
                    });
                };
                match event {
                    AcpEvent::Text { chunk } => {
                        if let Ok(mut buf) = stream_buf.lock() {
                            buf.push_lines(&chunk, emit_line);
                        }
                    }
                    other => {
                        if let Some(line) = acp_event_subagent_line(&other) {
                            emit_line(line);
                        }
                    }
                }
            },
            &mut cancel,
        )
        .await;
        if let Some(sink) = &self.event_sink {
            let parent_call = call.id.clone();
            if let Ok(mut buf) = text_buf.lock() {
                buf.flush(|line| {
                    let line = prefix_subagent_line(&line);
                    if !line.is_empty() {
                        sink(Control::SubagentOutput {
                            call_id: parent_call.clone(),
                            line,
                        });
                    }
                });
            }
        }

        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (status, report, is_error, session_id) = match result {
            Ok(outcome) if is_refusal(&outcome.answer) => (
                DelegateStatus::Failed,
                format!(
                    "`{wanted}` refused the task rather than doing it: {}",
                    outcome.answer
                ),
                true,
                Some(outcome.session_id).filter(|id| !id.is_empty()),
            ),
            Ok(outcome) if outcome.answer.trim().is_empty() => (
                DelegateStatus::Done,
                format!("`{wanted}` finished and said nothing."),
                false,
                Some(outcome.session_id).filter(|id| !id.is_empty()),
            ),
            Ok(outcome) => (
                DelegateStatus::Done,
                outcome.answer,
                false,
                Some(outcome.session_id).filter(|id| !id.is_empty()),
            ),
            Err(AcpFailure::Unstartable(why)) => (
                DelegateStatus::Failed,
                format!("`{wanted}` could not be started: {why}"),
                true,
                None,
            ),
            Err(AcpFailure::Refused(why)) => (
                DelegateStatus::Failed,
                format!("`{wanted}` did not finish the task: {why}"),
                true,
                None,
            ),
            Err(AcpFailure::Cancelled) => (
                DelegateStatus::Cancelled,
                format!("`{wanted}` was stopped before it finished."),
                true,
                None,
            ),
        };
        if let Some(session_id) = &session_id {
            if let Ok(mut sessions) = self.acp_sessions.lock() {
                sessions.insert(wanted.to_string(), session_id.clone());
            }
        }
        let record = DelegateAgentResult {
            status,
            agent: wanted.to_string(),
            total_tool_uses: 0,
            duration_ms,
            total_tokens: 0,
            model: None,
            session_id,
            report,
            worktree: WorktreeOutcome::Unused,
        };
        ToolOutput {
            call_id: call.id.clone(),
            output: record.to_json(),
            is_error,
            duration_ms,
        }
    }
}

/// Where the front matter ends, as a byte offset of the closing `\n---`.
fn front_matter_end(content: &str) -> Option<usize> {
    if !content.starts_with("---") {
        return None;
    }
    content[3..].find("\n---").map(|at| at + 3)
}

/// Read `name`, `description`, and `auto` out of YAML front matter.
///
/// Deliberately not a YAML parser. These are three bounded scalar fields at
/// the top of a known file, and a dependency that can parse anchors and merge
/// keys is a dependency that can also do something surprising with a file
/// anyone may drop in a skills directory.
///
/// `>` and `|` say the value is the indented block beneath, which is how a
/// description longer than a line is written. Taking the marker as the value
/// is how a skill came to describe itself as ">-".
fn parse_skill_front_matter(content: &str) -> Option<(String, String, bool)> {
    let end = front_matter_end(content)?;
    let lines: Vec<&str> = content[3..end].split('\n').collect();

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut auto = false;

    for (at, line) in lines.iter().enumerate() {
        if let Some(value) = line.strip_prefix("auto:") {
            let value = value.trim();
            if value == "true" || value == "false" {
                auto = value == "true";
                continue;
            }
        }
        let (key, inline) = if let Some(rest) = line.strip_prefix("name:") {
            ("name", rest.trim())
        } else if let Some(rest) = line.strip_prefix("description:") {
            ("description", rest.trim())
        } else {
            continue;
        };

        let value = if inline.is_empty() || is_block_marker(inline) {
            let mut block = Vec::new();
            for next in &lines[at + 1..] {
                if next.trim().is_empty() || !next.starts_with([' ', '\t']) {
                    break;
                }
                block.push(next.trim());
            }
            if block.is_empty() {
                continue;
            }
            // A folded block is one paragraph; a literal one keeps its breaks.
            block.join(if inline.starts_with('|') { "\n" } else { " " })
        } else {
            unquote(inline)
        };

        match key {
            "name" => name = Some(value),
            _ => description = Some(value),
        }
    }

    // A skill with no name cannot be asked for, and one with no description
    // gives the model nothing to choose on. Both are required.
    Some((name?, description?, auto))
}

/// `>`, `|`, and their chomping variants: a marker, never a value.
fn is_block_marker(inline: &str) -> bool {
    let mut chars = inline.chars();
    matches!(chars.next(), Some('>') | Some('|'))
        && matches!(chars.next(), None | Some('-') | Some('+'))
        && chars.next().is_none()
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if value.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && (bytes[value.len() - 1] == b'"' || bytes[value.len() - 1] == b'\'')
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

/// The body after the front matter, or the whole file when there is none.
fn skill_body(content: &str) -> String {
    match front_matter_end(content) {
        None => content.trim().to_string(),
        Some(end) => match content[end + 1..].find('\n') {
            Some(at) => content[end + 1 + at + 1..].trim().to_string(),
            None => String::new(),
        },
    }
}

/// What a skill hands back when it is read, bounded so one file cannot spend
/// a whole context on itself.
pub fn render_skill(skill: &SkillInfo) -> String {
    let body = if skill.body.len() > SKILL_BODY_LIMIT {
        let mut cut = SKILL_BODY_LIMIT;
        while cut > 0 && !skill.body.is_char_boundary(cut) {
            cut -= 1;
        }
        format!(
            "{}\n\n[truncated; the rest is in {}]",
            &skill.body[..cut],
            skill.path.display()
        )
    } else {
        skill.body.clone()
    };
    format!(
        "Skill `{}` ({}):\n\n{}",
        skill.name,
        skill.path.display(),
        body
    )
}

/// What the two OpenAgents repositories are, when the session is in one.
///
/// A session in `openagents.com` spent turns working out that it was in the
/// Phoenix application, and one in `openagents` that the CLI lives under
/// `crates/`. Both are facts about the workspace rather than about the work,
/// and neither is discoverable without reading around.
fn openagents_workspace_note(cwd: &Path) -> Option<String> {
    let shown = cwd.to_string_lossy();
    if !shown.to_lowercase().contains("openagents") {
        return None;
    }
    let mut lines = vec![
        format!(
            "This session is working in {shown}, which is part of OpenAgents. Two repositories \
             carry most of the work, and they are easy to confuse:"
        ),
        String::new(),
        "- **`openagents.com`** is the web application: a Phoenix and Elixir codebase serving the"
            .to_string(),
        "  site, the forge, and the `/api/v1` API. Its issues are the site's issues.".to_string(),
        "- **`openagents`** is the monorepo: the native `openagents` CLI lives in".to_string(),
        "  `crates/openagents-cli`, alongside the other packages. Its issues are the CLI's and \
         the monorepo's."
            .to_string(),
        String::new(),
        "They are separate repositories with separate issue lists, so name the one you mean when \
         you read or write issues, and do not assume the current directory is the one being asked \
         about."
            .to_string(),
    ];

    // Where the other one is, when it is checked out beside this one. Naming
    // the two without saying where the other lives sent a session grepping the
    // whole workspace root, which holds every read-only reference clone, and
    // it spent the tool's whole budget before being stopped.
    if let Some(sibling) = sibling_checkout(cwd) {
        let parent = sibling
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        lines.push(String::new());
        lines.push(format!(
            "The other one is checked out at `{}`. To search or read it, change directory first — \
             `cd {} && git grep …`. `git grep` refuses a path outside the repository it is run in, \
             and it is the one command most likely to be reached for here.",
            sibling.display(),
            sibling.display()
        ));
        lines.push(String::new());
        lines.push(format!(
            "Do not search `{parent}` itself. It is the workspace root, and it holds large \
             read-only clones of other people's repositories; a recursive grep there does not \
             finish. Search one repository at a time."
        ));
    }

    Some(lines.join("\n"))
}

/// The other OpenAgents repository, if it is checked out beside this one.
///
/// Checked rather than assumed: a machine with only one of the two would
/// otherwise be told to `cd` somewhere that does not exist, which is a worse
/// instruction than none.
fn sibling_checkout(cwd: &Path) -> Option<PathBuf> {
    let here = cwd.file_name()?.to_str()?;
    let other = match here {
        "openagents.com" => "openagents",
        "openagents" => "openagents.com",
        _ => return None,
    };
    let path = cwd.parent()?.join(other);
    path.join(".git").exists().then_some(path)
}

/// Commands that cannot be undone, and are never what was meant.
///
/// ## What this list is, and is not
///
/// It stops a small number of irreversible mistakes: erasing a home directory
/// or a disk, reformatting, halting the machine. It is not a security boundary
/// and cannot be one — a command can be assembled from variables, decoded, or
/// written to a file and run, and no list of patterns sees that. It catches
/// the accident, not the intent.
///
/// So it is kept short and aimed only at what cannot be undone. `rm -rf` on a
/// build directory is ordinary work and is allowed; `rm -rf` on `/` or `~` is
/// not, because no one means it.
///
/// Each pattern is paired with what to say, because a bare refusal reads as
/// the tool being broken rather than as the command being the problem.
const REFUSED: &[(&str, &str)] = &[
    // A recursive `rm` aimed at a root, a home, or everything in one. Aimed at
    // a build directory it is ordinary work, so the target is what decides:
    // `rm -rf target/debug` runs and `rm -rf ~/` does not. The flag run has to
    // carry an `r`, long options are allowed between the flags and the target
    // (`--no-preserve-root` is exactly the phrase that precedes the worst
    // version of this), and a trailing `/` or `/*` on the target still names
    // the same thing.
    (
        r"(?i)\brm\s+(--?[a-zA-Z][a-zA-Z-]*\s+)*-[a-zA-Z]*r[a-zA-Z]*\s+(--?[a-zA-Z][a-zA-Z-]*\s+)*(/|~|\$HOME|\$\{HOME\})(/\*|/)?(\s|$)",
        "That would erase a root or a home directory.",
    ),
    (r"(?i)\bmkfs(\.\w+)?\b", "That would reformat a filesystem."),
    (
        r"(?i)\bdd\b[^\n]*\bof=/dev/(disk|rdisk|sd|nvme|hd)",
        "That would write over a raw device.",
    ),
    (
        r"(?i)\bdiskutil\s+(erase|reformat|partition)",
        "That would erase or repartition a disk.",
    ),
    // Anchored to command position — the start of the line, after a `;`, `&&`,
    // `||`, or a pipe, or behind `sudo`. A bare word match refused
    // `echo 'shutdown the server' >> notes.md`, and a gate that refuses
    // ordinary work is one an agent learns to route around.
    (
        r"(?i)(^|[;&|]\s*|\bsudo\s+)(shutdown|reboot|halt|poweroff)\b",
        "That would stop this machine.",
    ),
    (r":\(\)\s*\{\s*:\|:&\s*\}\s*;\s*:", "That is a fork bomb."),
    (
        r"(?i)\bchmod\s+(-[a-zA-Z]+\s+)*(-R|--recursive)\s+[0-7]{3,4}\s+(/|~|\$HOME)(\s|$)",
        "That would change the permissions of a whole root or home directory.",
    ),
    (
        r"(?i)>\s*/dev/(disk|rdisk|sd|nvme|hd)",
        "That would write over a raw device.",
    ),
];

/// Why this command will not be run, or `None` when it will.
pub fn check_shell_refusal(cmd: &str) -> Option<String> {
    for (pattern, reason) in REFUSED {
        // A pattern that does not compile is a bug in this table, not a
        // reason to let the command through, so it is skipped loudly in debug
        // and treated as no-match otherwise.
        let Ok(re) = regex::Regex::new(pattern) else {
            debug_assert!(false, "the refusal pattern `{pattern}` does not compile");
            continue;
        };
        if re.is_match(cmd) {
            return Some(format!(
                "{reason} This session refuses it. If you meant something narrower, name the \
                 directory."
            ));
        }
    }
    None
}

/// The command heads one line would execute, in order, or `None` when the
/// line is too tangled to split cheaply.
///
/// A head is an executable plus its first argument when there is one — enough
/// to tell a rerun from legitimate work. Splitting is on top-level `;`, `&&`,
/// `||`, and `|` only: quotes are respected, `$( )` and backticks are treated
/// as opaque, and anything this misses stays missing because a conservative
/// `None` just means no lint, never a wrong refusal. Redirections after a
/// head are ignored; they do not change what executes.
///
/// This is also what #157's exporter re-execution audit clusters on, so the
/// definition of "the same command" lives in exactly one place.
pub fn command_heads(cmd: &str) -> Option<Vec<String>> {
    // Top-level separators. Byte-wise scanning rather than regex, because the
    // state to track is small — in single quotes, in double quotes, in a
    // command substitution — and depth is bounded by the input, not by a
    // grammar worth a crate.
    #[derive(PartialEq)]
    enum In {
        Nothing,
        Single,
        Double,
        Subst,
    }
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut state = In::Nothing;
    let bytes = cmd.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        match state {
            In::Single => {
                current.push(byte as char);
                if byte == b'\'' {
                    state = In::Nothing;
                }
            }
            In::Double => {
                current.push(byte as char);
                if byte == b'"' && bytes[index.saturating_sub(1)] != b'\\' {
                    state = In::Nothing;
                }
            }
            In::Subst => {
                current.push(byte as char);
                if byte == b')' {
                    state = In::Nothing;
                }
            }
            In::Nothing => match byte {
                b'\'' => {
                    current.push('\'');
                    state = In::Single;
                }
                b'"' => {
                    current.push('"');
                    state = In::Double;
                }
                b'$' if index + 1 < bytes.len() && bytes[index + 1] == b'(' => {
                    current.push_str("$(");
                    state = In::Subst;
                }
                b';' | b'|' => {
                    segments.push(std::mem::take(&mut current));
                }
                b'&' if index + 1 < bytes.len() && bytes[index + 1] == b'&'
                    || index > 0 && bytes[index - 1] == b'&' =>
                {
                    // Push only once per `&&`: on the first ampersand, unless
                    // a lone `&` (background) follows — that separates too,
                    // but a bare `&` is rare enough to treat conservatively
                    // as a separator as well.
                    segments.push(std::mem::take(&mut current));
                }
                _ => current.push(byte as char),
            },
        }
    }
    segments.push(current);

    let mut heads = Vec::new();
    for segment in segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        // Env-prefix assignments (`FOO=bar cmd …`) are skipped off the front
        // first: the environment does not change what executes.
        let mut words = segment.split_whitespace().map(str::to_string);
        let mut executable = loop {
            match words.next() {
                Some(word) if word.contains('=') && !word.starts_with('-') => continue,
                Some(word) => break word,
                None => return None,
            }
        };
        // Strip surrounding quotes, which would otherwise make `'cargo'` and
        // `cargo` different commands.
        executable = executable.trim_matches('\'').trim_matches('"').to_string();
        let mut next_word = move || {
            words
                .next()
                .map(|word| word.trim_matches('\'').trim_matches('"').to_string())
        };
        let mut head = match next_word() {
            Some(argument) if !argument.starts_with('-') => {
                format!("{executable} {argument}")
            }
            _ => {
                heads.push(executable.clone());
                continue;
            }
        };
        // `pnpm run test:rust` and `pnpm run lint` share nothing but the
        // binary — one more word separates them (the table is shared with
        // the repeat-cost gate below).
        // Package runners: the tool is the third word, and `pnpm run test`
        // against `pnpm run lint` are different work. `npx`/`pnpx` shape the
        // same way — the package is the second word and the command the
        // third, so `npx vp test` and `npx vp lint` must not collapse.
        const GIT_VERBS: &[&str] = &["stash", "worktree", "remote"];
        let parts: Vec<&str> = head.split(' ').collect();
        if parts.len() == 2 {
            let (first, second) = (parts[0], parts[1]);
            let wants_third = RUNNERS.contains(&first) && second == "run"
                || first == "git" && GIT_VERBS.contains(&second)
                || matches!(first, "npx" | "pnpx");
            if wants_third && let Some(third) = next_word() {
                head = format!("{head} {third}");
            }
        }
        heads.push(head);
    }
    Some(heads)
}

/// Why this one-liner runs the same work twice, or `None` when it does not.
///
/// A repeat is only worth refusing when repeating it **costs** something.
/// The origin (#153) was a five-minute suite executed three times for three
/// greps — the refusal paid for itself there. But the gate first shipped
/// refusing every repeated head, and a repeated `grep` or `sed` over a file
/// costs milliseconds while the refusal costs a whole turn: the model stops,
/// reads the hint, rewrites the line, and the session loses more to the
/// rewrite than the second grep would have spent. Worse, the tool's own
/// guidance — run once, keep the output, grep the file — *produces* lines
/// with several greps in them.
///
/// So the gate classifies the head before refusing:
///
/// - **Expensive** heads (suite runners, compilers, build tools) refuse with
///   the keep-the-output hint — the #153 case, unchanged.
/// - **Unsafe** heads (mutating commands) refuse because a second run is a
///   different action, not a repeat of the first — `git stash pop` twice
///   pops two different stashes. This is the caveat #153 itself carried;
///   the first implementation flattened it into "refuse everything".
/// - Everything else — the greps, the seds, the cat-and-ls reads — runs
///   free, silently. A refusal that saves milliseconds while costing a turn
///   is the bug, not the fix.
pub fn check_duplicate_execution(cmd: &str) -> Option<String> {
    let heads = command_heads(cmd)?;
    let mut seen = std::collections::HashSet::new();
    for head in &heads {
        if seen.insert(head.clone()) {
            continue;
        }
        match repeat_cost(head) {
            RepeatCost::Free => {}
            RepeatCost::Expensive => {
                return Some(format!(
                    "This one-liner executes `{head}` more than once. A second run costs the \
                     first run's time again. Run it once with the output kept — append \
                     `2>&1 | tee /tmp/last-run.log`, or rely on the session log this session \
                     writes for long commands — then answer every follow-up question by \
                     grepping that file instead of executing again. Call `history_recall` \
                     to read what the earlier run printed."
                ));
            }
            RepeatCost::Unsafe => {
                return Some(format!(
                    "This one-liner executes `{head}` more than once. A second run is not the \
                     same action again — it acts on whatever the first run changed. Say what \
                     each run is for and give each its own tool call."
                ));
            }
        }
    }
    None
}

/// What running one head a second time inside the same line costs.
#[derive(PartialEq, Debug)]
enum RepeatCost {
    /// Pure and quick: grep it twice, the second grep is milliseconds.
    Free,
    /// A second run spends the first run's wall time again: suites, builds.
    Expensive,
    /// A second run is a different action: it mutates state the first run
    /// already changed. Refused on danger, not on time.
    Unsafe,
}

/// The package runners whose `run <script>` heads are build-and-test work.
/// The same table [`command_heads`] shapes third words with; both gates must
/// agree on what a head is before they can disagree on what a repeat costs.
const RUNNERS: &[&str] = &["pnpm", "npm", "yarn", "bun", "npx", "pnpx"];

/// The cargo subcommands that compile something.
const CARGO_HEAVY: &[&str] = &[
    "test", "build", "bench", "check", "clippy", "doc", "install", "run",
];

/// The git verbs that change state, for the unsafe classification. (`git
/// stash pop` twice pops two different stashes; `git status` twice is free.)
const GIT_MUTATIONS: &[&str] = &[
    "commit",
    "push",
    "pull",
    "merge",
    "rebase",
    "reset",
    "revert",
    "cherry-pick",
    "checkout",
    "switch",
    "restore",
    "stash",
    "rm",
    "clean",
    "apply",
    "tag",
    "am",
    "bisect",
];

/// The other mutating commands whose repeat is a different action.
const BARE_MUTATIONS: &[&str] = &[
    "rm", "rmdir", "mv", "dd", "shred", "truncate", "kill", "pkill",
];

fn repeat_cost(head: &str) -> RepeatCost {
    let mut words = head.split_whitespace();
    let Some(first) = words.next() else {
        return RepeatCost::Free;
    };
    let second = words.next();

    if BARE_MUTATIONS.contains(&first) {
        return RepeatCost::Unsafe;
    }
    if first == "git" {
        return match second {
            // `git stash pop` carries the pop in the third word, which
            // `command_heads` already kept; the verb pair is what repeats.
            Some("stash" | "worktree" | "remote") => RepeatCost::Unsafe,
            Some(verb) if GIT_MUTATIONS.contains(&verb) => RepeatCost::Unsafe,
            _ => RepeatCost::Free,
        };
    }
    if first == "cargo" {
        return match second {
            Some(sub) if CARGO_HEAVY.contains(&sub) => RepeatCost::Expensive,
            _ => RepeatCost::Free,
        };
    }
    if RUNNERS.contains(&first) {
        return match second {
            Some("run") => RepeatCost::Expensive,
            Some("install" | "add" | "i" | "update" | "remove") => RepeatCost::Unsafe,
            _ => RepeatCost::Free,
        };
    }
    const OTHER_HEAVY: &[&str] = &["make", "bazel", "gradle", "mvn", "cmake"];
    if OTHER_HEAVY.contains(&first) {
        return RepeatCost::Expensive;
    }
    if first == "go" && matches!(second, Some("test" | "build" | "generate")) {
        return RepeatCost::Expensive;
    }
    if first == "dotnet" && matches!(second, Some("build" | "test" | "publish")) {
        return RepeatCost::Expensive;
    }
    if first == "docker" && second == Some("build") {
        return RepeatCost::Expensive;
    }
    RepeatCost::Free
}

/// The text a tool result carries, and whether the command actually worked.
///
/// The outcome used to be dropped here and every shell result was reported to
/// the model as `is_error: false`, so a failing build read like a passing one
/// and the model carried on as though the step had succeeded.
///
/// Whole output that outgrew [`OUTPUT_LIMIT`] used to vanish past the
/// boundary — printed `N` characters and gone. Any question about what the
/// other side of that cut held could only be answered by running the command
/// again, which is minutes against suites. When a command runs long, its full
/// transcript lands beside the session record instead (`#152`), and the
/// excerpt names the file so a later question greps rather than re-executes.
#[cfg(test)]
async fn run_real_shell(
    cmd: &str,
    cwd: &Path,
    timeout_secs: u64,
    cancel: &mut watch::Receiver<bool>,
) -> (String, bool) {
    run_real_shell_logged(cmd, cwd, timeout_secs, cancel, None).await
}

/// The real runner, with an optional place to keep whole output.
///
/// `session_dir` comes from the session record's own directory when there is
/// one; tests pass a scratch directory. A spawn failure before any output
/// means nothing worth keeping, so no file is written for it.
async fn run_real_shell_logged(
    cmd: &str,
    cwd: &Path,
    timeout_secs: u64,
    cancel: &mut watch::Receiver<bool>,
    session_dir: Option<&Path>,
) -> (String, bool) {
    let started = std::time::Instant::now();
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The shell spawns children of its own, and without a group of their
        // own they survive a cancelled fan-out and reparent to init. Every
        // other spawn site in this crate — `delegate.rs`, `computer.rs`,
        // `acp.rs` — already puts its child in one; this was the exception.
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return (format!("Failed to spawn shell command: {}", e), true),
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stdout {
            let _ = stream.read_to_end(&mut bytes).await;
        }
        bytes
    });
    let stderr = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stderr {
            let _ = stream.read_to_end(&mut bytes).await;
        }
        bytes
    });

    enum End {
        Status(std::io::Result<std::process::ExitStatus>),
        Cancelled,
        TimedOut,
    }
    let end = tokio::select! {
        status = child.wait() => End::Status(status),
        changed = cancel.changed() => {
            if changed.is_ok() && *cancel.borrow() {
                crate::signals::stop_tree(&mut child).await;
                End::Cancelled
            } else {
                End::Status(child.wait().await)
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {
            crate::signals::stop_tree(&mut child).await;
            End::TimedOut
        }
    };
    let stdout = stdout.await.unwrap_or_default();
    let stderr = stderr.await.unwrap_or_default();
    let elapsed_secs = started.elapsed().as_secs();

    // One file per call for the session: `cmd-N.log` beside `updates.jsonl`.
    // Written whenever the run was long enough to be worth keeping —
    // truncated or not — because the truncation notice and the log pointer
    // together are what make the next question answerable without a rerun.
    let mut persisted: Option<PathBuf> = None;
    if elapsed_secs >= PERSIST_AFTER_SECS
        && let Some(dir) = session_dir
    {
        match write_command_log(dir, cmd, &stdout, &stderr) {
            Ok(path) => persisted = Some(path),
            Err(error) => {
                tracing::warn!("could not persist shell output: {error}");
            }
        }
    }

    match end {
        End::Status(Ok(status)) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&stdout));
            combined.push_str(&String::from_utf8_lossy(&stderr));

            let combined = strip_terminal_escapes(&combined);
            let total_len = combined.len();
            let bounded = if total_len > OUTPUT_LIMIT {
                let cut_at = floor_char_boundary(&combined, OUTPUT_LIMIT);
                let head = &combined[..cut_at];
                match &persisted {
                    Some(path) => format!(
                        "{}\n\n[Output truncated: printed {} characters, limit is {}. Full output kept at `{}`. {HISTORY_RECALL_GREP}]",
                        head,
                        total_len,
                        OUTPUT_LIMIT,
                        path.display()
                    ),
                    None => format!(
                        "{}\n\n[Output truncated: printed {} characters, limit is {}]",
                        head, total_len, OUTPUT_LIMIT
                    ),
                }
            } else {
                combined
            };

            if status.success() {
                if bounded.trim().is_empty() {
                    ("Success".to_string(), false)
                } else {
                    (bounded.trim().to_string(), false)
                }
            } else {
                let code = status.code().unwrap_or(1);
                (
                    format!(
                        "The command exited with code {}.\n\n{}",
                        code,
                        bounded.trim()
                    ),
                    true,
                )
            }
        }
        End::Status(Err(e)) => (format!("Shell execution error: {}", e), true),
        End::Cancelled => (CANCELLED_TOOL_RESULT.to_string(), true),
        End::TimedOut => {
            // A timed-out command is precisely one whose later output nobody
            // has seen, so the kept transcript earns its file here too.
            let tail_note = match &persisted {
                Some(path) => format!(
                    " Partial output kept at `{}`. {HISTORY_RECALL_GREP}",
                    path.display()
                ),
                None => String::new(),
            };
            (
                format!(
                    "The command timed out after {} seconds and was stopped.{}",
                    timeout_secs, tail_note
                ),
                true,
            )
        }
    }
}

/// Whole stdout and stderr of one command, under the session's directory.
///
/// Numbered by what is already there, so files sort in execution order and a
/// resumed session keeps counting rather than overwriting. The command itself
/// goes in as a header, because a log that does not say what ran is a riddle.
fn write_command_log(
    dir: &Path,
    cmd: &str,
    stdout_bytes: &[u8],
    stderr_bytes: &[u8],
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let next = (0..)
        .map(|n| dir.join(format!("cmd-{n}.log")))
        .find(|path| !path.exists())
        .ok_or_else(|| std::io::Error::other("no free cmd-N.log name in the session directory"))?;
    let mut contents =
        String::with_capacity(cmd.len() + stdout_bytes.len() + stderr_bytes.len() + 32);
    contents.push_str("$ ");
    contents.push_str(cmd);
    contents.push_str("\n\n");
    contents.push_str(&String::from_utf8_lossy(stdout_bytes));
    contents.push_str(&String::from_utf8_lossy(stderr_bytes));
    std::fs::write(&next, contents)?;
    Ok(next)
}

/// The `checkpoint` tool: one milestone note, stored and replayed at resume.
///
/// The value is durability, not conversation: a session that dies mid-turn —
/// cap, crash, cancel — leaves its last checkpoint on disk for the next
/// session, which is exactly the reader the note is for (#189).
fn answer_checkpoint(arguments: &serde_json::Value) -> (String, bool) {
    let Some(text) = arguments.get("text").and_then(|v| v.as_str()) else {
        return (
            "Nothing was recorded: `text` is required and must be the note itself.".to_string(),
            true,
        );
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (
            "Nothing was recorded: the note was empty.".to_string(),
            true,
        );
    }
    (
        format!(
            "Checkpoint recorded ({} bytes). It will be shown when this session resumes.",
            trimmed.len()
        ),
        false,
    )
}

// ─────────────────────────────────────────────────────────── the file tools

/// The absolute path `raw` names, or why the attempt failed.
///
/// A relative path is taken against the session's working directory. `..` is
/// collapsed lexically so a path that does not exist yet resolves the same
/// way one that does: `write` creates files, and a resolution that depended
/// on what already existed would resolve a create differently from a read of
/// the same name. Symlinks are not followed here — the tool that opens the
/// path lets the operating system do that, and lives with what it says.
///
/// There is no working-directory jail here, deliberately (#151): a jail on
/// these tools withheld nothing from the session — `shell` reaches every
/// file on the machine unimpeded, including the ones that own the others
/// (`cat`, `tee`, `rm`) — so the restriction was pure friction, and it fell
/// hardest on `read`, the one tool built to avoid quoting through a shell.
/// What constrains a file tool after this is the operating system's own
/// permissions, which is also all that has ever constrained `shell`.
fn resolve_path(cwd: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("No `path` was given.".to_string());
    }
    let joined = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        cwd.canonicalize()
            .unwrap_or_else(|_| cwd.to_path_buf())
            .join(raw)
    };

    let mut lexical = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::ParentDir => {
                lexical.pop();
            }
            std::path::Component::CurDir => {}
            other => lexical.push(other.as_os_str()),
        }
    }

    Ok(lexical)
}

/// Write `content` to `path` by staging it beside the destination and renaming.
///
/// Truncating in place opens a window in which every other reader on the
/// machine — a compiler, a test run, a second agent — sees a half-written file
/// (#114). `rename` within a directory is atomic, so no reader sees anything
/// but the old file or the new one. The staging name carries the process id
/// and a counter so two writers cannot collide on it, and a failed rename
/// takes the staged file with it rather than leaving litter behind.
fn write_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;

    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let staging = dir.join(format!(
        ".{}.{}.{}.staged",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));

    fs::write(&staging, content)?;
    if let Err(error) = fs::rename(&staging, path) {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    Ok(())
}

/// The `read` tool: a file as text, or a refusal saying why not.
fn answer_read(cwd: &Path, arguments: &serde_json::Value) -> (String, bool) {
    let raw = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let path = match resolve_path(cwd, raw) {
        Ok(path) => path,
        Err(refusal) => return (refusal, true),
    };
    match fs::read_to_string(&path) {
        Ok(content) if content.len() > OUTPUT_LIMIT => {
            // The cut steps back to a character boundary; slicing a `String`
            // at a fixed byte count panics the first time a file carries a
            // non-ASCII character across the limit.
            let head = &content[..floor_char_boundary(&content, OUTPUT_LIMIT)];
            (
                format!(
                    "{head}\n\n[Output truncated: the file is {} characters, limit is {}. Read \
                     the rest with `bash`.]",
                    content.len(),
                    OUTPUT_LIMIT
                ),
                false,
            )
        }
        Ok(content) => (content, false),
        Err(error) => (format!("Could not read `{raw}`: {error}."), true),
    }
}

/// The `write` tool: replace a file's contents, or say why nothing was written.
fn answer_write(cwd: &Path, arguments: &serde_json::Value) -> (String, bool) {
    let raw = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let Some(content) = arguments.get("content").and_then(|v| v.as_str()) else {
        return (
            "Nothing was written: `content` is required and must be the file's complete new \
             contents."
                .to_string(),
            true,
        );
    };
    let path = match resolve_path(cwd, raw) {
        Ok(path) => path,
        Err(refusal) => return (refusal, true),
    };
    match write_atomically(&path, content) {
        Ok(()) => (
            format!("Wrote {} bytes to {}.", content.len(), path.display()),
            false,
        ),
        Err(error) => (format!("Could not write `{raw}`: {error}."), true),
    }
}

/// The `edit` tool: replace one exact run of text, or refuse and change nothing.
///
/// The two refusals are the design, and they are pi's. Text that is not there
/// cannot be replaced, and text that is there more than once does not say
/// which one was meant — so the model is told the count and asked to add
/// context until the match is unique. That single rule is what makes a
/// surgical edit safe without a diff format. Both refusals return before the
/// write, so a refused edit leaves the file exactly as it was.
fn answer_edit(cwd: &Path, arguments: &serde_json::Value) -> (String, bool) {
    // The batch form: several edits over one file, applied in order, all or
    // nothing. A turn's budget prices every call the same, so three separate
    // edit calls cost three of the cap while one batched call costs one
    // (#190) -- the 2026-08-27 session spent roughly forty percent of its
    // calls on edit work that batching collapses.
    if let Some(edits) = arguments.get("edits").and_then(|v| v.as_array()) {
        return answer_edit_batch(cwd, arguments, edits);
    }
    let raw = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let old = arguments
        .get("oldText")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if old.is_empty() {
        return (
            "Nothing was changed: `oldText` is required and must be the exact text to replace."
                .to_string(),
            true,
        );
    }
    let path = match resolve_path(cwd, raw) {
        Ok(path) => path,
        Err(refusal) => return (refusal, true),
    };
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => return (format!("Could not read `{raw}` to edit it: {error}."), true),
    };

    match answer_edit_inner(&content, arguments) {
        Ok((body, note)) => match write_atomically(&path, &body) {
            Ok(()) => {
                let new = arguments
                    .get("newText")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let reply = format!(
                    "Replaced {} bytes with {} in {}.",
                    content.len() - body.len(),
                    new.len(),
                    path.display()
                );
                (with_region_echo(reply, &note, &body, new), false)
            }
            Err(error) => (format!("Could not write `{raw}`: {error}."), true),
        },
        Err(refusal) => (format!("Nothing was changed: {refusal}"), true),
    }
}

/// Several edits over one file, applied in order, all or nothing (#190).
///
/// The per-edit results are computed against the running text as each lands,
/// and one write at the end is the only durable change: a miss at any edit
/// refuses the whole batch with the file untouched, which keeps the
/// unique-match rule's guarantee -- a refused call changes nothing -- true
/// for the batch the same way it is true for one edit.
fn answer_edit_batch(
    cwd: &Path,
    arguments: &serde_json::Value,
    edits: &[serde_json::Value],
) -> (String, bool) {
    let raw = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let path = match resolve_path(cwd, raw) {
        Ok(path) => path,
        Err(refusal) => return (refusal, true),
    };
    let mut content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => return (format!("Could not read `{raw}` to edit it: {error}."), true),
    };
    let mut notes: Vec<String> = Vec::new();
    for (index, one) in edits.iter().enumerate() {
        let one_args = serde_json::json!({
            "oldText": one.get("oldText").and_then(|v| v.as_str()).unwrap_or(""),
            "newText": one.get("newText").and_then(|v| v.as_str()).unwrap_or(""),
        });
        match answer_edit_inner(&content, &one_args) {
            Ok((next, note)) => {
                content = next;
                if !note.is_empty() {
                    notes.push(format!("edit {}: {note}", index + 1));
                }
            }
            Err(refusal) => {
                return (
                    format!(
                        "Nothing was changed: edit {} of {} missed, so the whole batch was \
                         refused and the file is untouched.\n\n{refusal}",
                        index + 1,
                        edits.len()
                    ),
                    true,
                );
            }
        }
    }
    match write_atomically(&path, &content) {
        Ok(()) => {
            let mut reply = format!("Applied {} edits to {}.", edits.len(), path.display());
            if !notes.is_empty() {
                reply.push_str(" (");
                reply.push_str(&notes.join("; "));
                reply.push_str(".)");
            }
            (reply, false)
        }
        Err(error) => (format!("Could not write `{raw}`: {error}."), true),
    }
}

/// One edit over `content` held in memory: the file's own bytes for a single
/// edit, the running text for an edit inside a batch. `Ok` carries the new
/// text and any loose-tier note; `Err` is the refusal, with #160's
/// diagnostics, and means the caller must write nothing.
fn answer_edit_inner(
    content: &str,
    arguments: &serde_json::Value,
) -> Result<(String, String), String> {
    let old = arguments
        .get("oldText")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new = arguments
        .get("newText")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if old.is_empty() {
        return Err("`oldText` is required and must be the exact text to replace.".to_string());
    }

    // Exact first, always: the common case pays nothing, and what follows
    // only runs after the byte-for-byte search has already failed.
    match content.matches(old).count() {
        1 => {
            let site = content.find(old).expect("counted");
            return Ok(finish_edit_in_memory(
                content,
                site..site + old.len(),
                new,
                "",
            ));
        }
        hits if hits > 1 => {
            return Err(format!(
                "That `oldText` appears {hits} times and it has to appear exactly once. Add the \
                 lines above and below the one you mean until the match is unique, then call again."
            ));
        }
        _ => {}
    }

    // Zero exact hits. Before refusing, rematch with progressively looser
    // comparisons -- the ladder codex's apply_patch walks when a hunk fails
    // to land. Normalization chooses where a replacement goes, never what is
    // written: a tier locates a span of the file's own bytes, and `newText`
    // is spliced over it verbatim. Serialization damage -- two backslashes
    // sent where the file holds one (#160, steps 40 and 43), or trailing
    // whitespace the model did not reproduce -- becomes one clean edit
    // instead of three blind retries.
    for tier in MATCH_TIERS {
        match locate(content, old, tier.normalize) {
            Locate::Unique(span) => {
                return Ok(finish_edit_in_memory(content, span, new, tier.note));
            }
            Locate::Ambiguous(count) => {
                return Err(format!(
                    "`oldText` matches {count} places once whitespace or escape differences \
                     are ignored. Add the lines above and below the one you mean until one \
                     place is left, then call again."
                ));
            }
            Locate::Miss => {}
        }
    }

    // Every tier missed. Show the nearest real text beside what was sent so
    // the next call repairs the needle instead of resending it.
    let mut message = format!(
        "That `oldText` does not appear here, exactly or up to whitespace or \
         backslash-escape differences."
    );
    if let Some(report) = diagnose_miss(content, old) {
        message.push_str("\n\n");
        message.push_str(&report);
    }
    Err(message)
}

/// The changed region of a finished edit, appended to the reply (#190).
///
/// The next-best thing to the confirmatory read the model was going to pay a
/// call for: the new text in place with a file line on each side, so the
/// model can see the splice landed where it meant. Bounded at
/// [`ECHO_LIMIT`], cut on a character boundary like every other bounded
/// reply here.
fn with_region_echo(mut reply: String, note: &str, body: &str, new: &str) -> String {
    if !note.is_empty() {
        reply.push_str(" (");
        reply.push_str(note);
        reply.push_str(".)");
    }
    if new.is_empty() {
        return reply;
    }
    if let Some(site) = body.find(new) {
        let before = &body[..site];
        let after = &body[(site + new.len()).min(body.len())..];
        let before_line = before
            .rfind('\n')
            .map(|at| &before[at + 1..])
            .unwrap_or(before);
        let after_line = after.split('\n').next().unwrap_or("");
        let context = |line: &str, marker: &str| -> String {
            let mut line = line;
            let mut ellipsis = "";
            if line.len() > 80 {
                let at = floor_char_boundary(line, 80);
                line = &line[..at];
                ellipsis = "\u{2026}";
            }
            format!("{marker} {line}{ellipsis}\n")
        };
        let mut echo = String::from("\n\nThe file now reads, at the edit:\n");
        echo.push_str(&context(before_line, "\u{2026}"));
        echo.push_str(&context(new, "\u{b7}"));
        echo.push_str(&context(after_line, "\u{2026}"));
        if reply.len() + echo.len() <= ECHO_LIMIT {
            reply.push_str(&echo);
        }
    }
    reply
}

/// One rung of the edit rematch ladder.
struct MatchTier {
    /// Applied to both the file and `oldText` line by line; matching compares
    /// the results.
    normalize: fn(&str) -> String,
    /// Said in the success reply when this rung found the site.
    note: &'static str,
}

/// The ladder, loosest last. Byte equality ran before the loop. The order is
/// codex's `seek_sequence`, plus one rung for JSON string-escape damage on
/// Rust line continuations: a `\` before a newline is exactly the pair
/// models miscount, and no whitespace rule touches it. Every rung after byte
/// equality works at line granularity -- physical lines compared after
/// normalization, replaced as whole lines -- because trimming inside
/// character-by-character offset arithmetic hides phantom newlines; codex
/// made the same call, hunks land on line boundaries there too.
const MATCH_TIERS: [MatchTier; 3] = [
    MatchTier {
        normalize: trim_end_lines,
        note: "matched after ignoring trailing whitespace; whole lines were \
               replaced, so check them",
    },
    MatchTier {
        normalize: trim_lines,
        note: "matched after ignoring leading and trailing whitespace; whole \
               lines were replaced, so check them",
    },
    MatchTier {
        normalize: collapse_escaped_backslashes,
        note: "matched after collapsing doubled backslashes before newlines \
               (escape-sequence mismatch); check the result",
    },
];

fn trim_end_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split('\n') {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.pop();
    out
}

fn trim_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split('\n') {
        out.push_str(line.trim());
        out.push('\n');
    }
    out.pop();
    out
}

/// Collapse every run of two or more backslashes to one, anywhere. Applied
/// only after the trim rungs failed, and only to decide where an edit lands:
/// the replaced bytes stay the file's own.
fn collapse_escaped_backslashes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut slashes = 0usize;
    for ch in text.chars() {
        if ch == '\\' {
            slashes += 1;
        } else {
            if slashes > 0 {
                out.push('\\');
                slashes = 0;
            }
            out.push(ch);
        }
    }
    if slashes > 0 {
        out.push('\\');
    }
    out
}

/// What one rematch rung concluded.
enum Locate {
    /// Exactly one site, as a byte span into the original text.
    Unique(std::ops::Range<usize>),
    /// More than one site under this rung's comparison, with the count.
    Ambiguous(usize),
    /// No site under this rung's comparison.
    Miss,
}

/// Where `needle` sits among `haystack`'s physical lines under one
/// normalization, if it sits there exactly once. Lines are compared after
/// normalization and reported as a span of the *original* bytes covering the
/// whole first-through-last matched line, so what gets replaced is the
/// file's own text and the arithmetic never crosses a line boundary.
///
/// A `needle` ending in a newline loses its empty final segment, so `a\nb\n`
/// names the same two lines as `a\nb`; a needle naming fewer lines than it
/// spans cannot arise. A needle whose first line names only part of a file
/// line finds nothing here -- partial-line work belongs to the exact search,
/// and refusing it loose keeps a trim from swallowing half a statement.
fn locate(haystack: &str, needle: &str, normalize: fn(&str) -> String) -> Locate {
    let hay_lines: Vec<&str> = haystack.split('\n').collect();
    let mut wanted: Vec<&str> = needle.split('\n').collect();
    if wanted.last() == Some(&"") {
        wanted.pop();
    }
    if wanted.is_empty() || wanted.len() > hay_lines.len() {
        return Locate::Miss;
    }

    // Byte offset where each physical line starts; the last entry is one past
    // the end, kept for measuring the final line of a window.
    let mut starts = Vec::with_capacity(hay_lines.len() + 1);
    let mut at = 0usize;
    for line in &hay_lines {
        starts.push(at);
        at += line.len() + 1;
    }
    starts.push(at);

    let mut count = 0usize;
    let mut hit = 0usize;
    for begin in 0..=hay_lines.len() - wanted.len() {
        if hay_lines[begin..begin + wanted.len()]
            .iter()
            .zip(&wanted)
            .all(|(have, sent)| normalize(have) == normalize(sent))
        {
            count += 1;
            if count > 1 {
                return Locate::Ambiguous(count);
            }
            hit = begin;
        }
    }
    if count != 1 {
        return Locate::Miss;
    }
    let last = hit + wanted.len() - 1;
    Locate::Unique(starts[hit]..starts[last] + hay_lines[last].len())
}

/// Write the replacement and say whether a fallback rung did the finding.
/// Splice one edit into text held in memory and say what happened.
///
/// No write happens here: the caller owns durability, because a batch's
/// edits must land as one write and a single edit writes once. Returns the
/// new text and the loose-tier note, if a tier found the site.
fn finish_edit_in_memory(
    content: &str,
    span: std::ops::Range<usize>,
    new: &str,
    note: &str,
) -> (String, String) {
    // Removing whole lines leaves their terminator behind; when `newText` is
    // empty the model asked for deletion, so the line's newline goes with it.
    let mut span = span;
    if new.is_empty() && content.as_bytes().get(span.end) == Some(&b'\n') {
        span.end += 1;
    }
    let mut body = String::with_capacity(content.len() - (span.end - span.start) + new.len());
    body.push_str(&content[..span.start]);
    body.push_str(new);
    body.push_str(&content[span.end..]);
    (body, note.to_string())
}

/// Turn a total miss into something the next call can fix: print the file's
/// real lines beside the ones sent, control characters shown, count named
/// where they differ.
///
/// Multi-line `oldText` anchors on its most distinctive line (rarest in the
/// file, then longest). Early boilerplate (`let dir = ...`, closing braces)
/// matches the wrong region so often that the side-by-side becomes a trap:
/// the model retries from a bogus snippet. Single-line misses keep the
/// historical first-line window so existing diagnostics stay byte-stable.
fn diagnose_miss(content: &str, old: &str) -> Option<String> {
    let sent_lines: Vec<&str> = old.split('\n').collect();
    let file_lines: Vec<&str> = content.split('\n').collect();
    if sent_lines.len() <= 1 {
        return diagnose_miss_first_line(&file_lines, &sent_lines);
    }
    let (sent_index, file_anchor) = distinctive_anchor(&file_lines, &sent_lines)?;
    format_miss_report(&file_lines, &sent_lines, sent_index, file_anchor)
}

/// Historical first-line window. Used for a one-line miss, where there is no
/// later line to prefer.
fn diagnose_miss_first_line(file_lines: &[&str], sent_lines: &[&str]) -> Option<String> {
    let first_line = *sent_lines.first()?;
    let trimmed_first = first_line.trim();
    if trimmed_first.is_empty() {
        return None;
    }
    let wanted = collapse_escaped_backslashes(trimmed_first);
    let mut anchor = None;
    for (index, line) in file_lines.iter().enumerate() {
        let candidate = collapse_escaped_backslashes(line.trim());
        if candidate.contains(&wanted) || wanted.contains(&candidate) && !candidate.is_empty() {
            anchor = Some(index);
            break;
        }
    }
    let anchor = anchor?;

    let window_start = anchor.saturating_sub(2);
    let shown = sent_lines.len().max(5);
    let window_end = (anchor + shown).min(file_lines.len());

    let mut report = format!(
        "Closest region for the first submitted line (file line {}):\n",
        anchor + 1
    );
    report.push_str("file shows vs you sent (backslashes doubled so the count is visible):\n");
    for (step, file_index) in (window_start..window_end).enumerate() {
        let file_side = reveal(file_lines[file_index]);
        let sent_side = reveal(sent_lines.get(step).copied().unwrap_or(""));
        let marker = if file_side == sent_side { " " } else { "!" };
        report.push_str(&format!(
            "{marker} {:>4} | {:<60} | {}\n",
            file_index + 1,
            cut(&file_side, 60),
            cut(&sent_side, 40)
        ));
    }
    for line in sent_lines.iter().skip(window_end - window_start) {
        report.push_str(&format!(
            "      . | {:<60} | {}\n",
            " ".repeat(0),
            cut(&reveal(line), 40)
        ));
    }
    report.push_str(&format!(
        "Your line 1 differs from file line {} first at column {}; repair `oldText` from what the file shows.",
        anchor + 1,
        first_diff_column(file_lines[anchor], first_line)
    ));
    Some(report)
}

/// Pick the submitted line that is rarest in the file, then longest, and the
/// file line it matches. Lines that never appear cannot anchor a region.
fn distinctive_anchor(file_lines: &[&str], sent_lines: &[&str]) -> Option<(usize, usize)> {
    let mut best: Option<(u64, usize, usize)> = None;
    for (sent_index, sent) in sent_lines.iter().enumerate() {
        let wanted = collapse_escaped_backslashes(sent.trim());
        if wanted.is_empty() {
            continue;
        }
        // Count only file lines that contain this submitted line. The reverse
        // (`wanted.contains(candidate)`) makes a long unique line look common
        // because it contains every short brace and identifier in the file.
        let matches: Vec<usize> = file_lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let candidate = collapse_escaped_backslashes(line.trim());
                if candidate.is_empty() {
                    return None;
                }
                if candidate.contains(&wanted) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        if matches.is_empty() {
            continue;
        }
        // Rarity first (unique beats common), then length. The later of two
        // equal scores wins so a fixture tail beats an equally-rare opener.
        let rarity = u64::MAX / matches.len() as u64;
        let score = rarity.saturating_add(wanted.len() as u64);
        let file_index = matches[0];
        match best {
            Some((best_score, _, _)) if score < best_score => {}
            _ => best = Some((score, sent_index, file_index)),
        }
    }
    best.map(|(_, sent_index, file_index)| (sent_index, file_index))
}

fn format_miss_report(
    file_lines: &[&str],
    sent_lines: &[&str],
    sent_index: usize,
    file_anchor: usize,
) -> Option<String> {
    let sent_line = *sent_lines.get(sent_index)?;
    let window_start = file_anchor.saturating_sub(2);
    let after = sent_lines.len() - sent_index;
    let window_end = (file_anchor + after.max(3)).min(file_lines.len());

    let mut report = format!(
        "Closest region for submitted line {} (file line {}):\n",
        sent_index + 1,
        file_anchor + 1
    );
    report.push_str("file shows vs you sent (backslashes doubled so the count is visible):\n");
    for file_index in window_start..window_end {
        let file_side = reveal(file_lines[file_index]);
        let sent_row = (file_index + sent_index).checked_sub(file_anchor);
        let sent_side = sent_row
            .and_then(|index| sent_lines.get(index).copied())
            .unwrap_or("");
        let marker = if file_side == reveal(sent_side) {
            " "
        } else {
            "!"
        };
        report.push_str(&format!(
            "{marker} {:>4} | {:<60} | {}\n",
            file_index + 1,
            cut(&file_side, 60),
            cut(&reveal(sent_side), 40)
        ));
    }
    let last_paired_sent = sent_index + (window_end - file_anchor);
    for line in sent_lines.iter().skip(last_paired_sent) {
        report.push_str(&format!(
            "      . | {:<60} | {}\n",
            " ".repeat(0),
            cut(&reveal(line), 40)
        ));
    }
    report.push_str(&format!(
        "Your line {} differs from file line {} first at column {}; repair `oldText` from what the file shows.",
        sent_index + 1,
        file_anchor + 1,
        first_diff_column(file_lines[file_anchor], sent_line)
    ));
    Some(report)
}

fn cut(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        line.to_string()
    } else {
        line.chars().take(width).collect()
    }
}

/// First differing column between two strings; the number the reply points
/// at is the repair the model needs to make.
fn first_diff_column(file_line: &str, sent_line: &str) -> usize {
    let file_chars: Vec<char> = file_line.chars().collect();
    let sent_chars: Vec<char> = sent_line.trim_end().chars().collect();
    let last = file_chars.len().max(sent_chars.len());
    for column in 0..last {
        match (file_chars.get(column), sent_chars.get(column)) {
            (Some(a), Some(b)) if a == b => {}
            _ => return column + 1,
        }
    }
    last
}

/// Rendering for the two characters that matter here: backslash shown
/// doubled so its count survives anything, tab and carriage return named.
fn reveal(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// As [`run_real_shell`]: the text, and whether it worked. A CLI that could
/// not even be spawned was previously reported to the model as a success.
/// Where the program behind the `openagents` tool came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAgentsCliSource {
    /// A program named `openagents` found on `PATH`.
    Path,
    /// Nothing on `PATH`, so this binary answers for it.
    ThisBinary,
}

/// The program the `openagents` tool runs, and where it was found.
///
/// `PATH` comes first so an installed `openagents` entry point keeps the same
/// update and launch behavior as a shell command. The current binary is the
/// fallback for development runs and renamed entry points.
pub fn resolve_openagents_cli() -> Result<(PathBuf, OpenAgentsCliSource), String> {
    let name = format!("openagents{}", std::env::consts::EXE_SUFFIX);
    let on_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|dir| dir.join(&name))
        .find(|candidate| is_executable_file(candidate));
    if let Some(found) = on_path {
        return Ok((found, OpenAgentsCliSource::Path));
    }
    match std::env::current_exe() {
        Ok(exe) => Ok((exe, OpenAgentsCliSource::ThisBinary)),
        // Neither resolves. This is an error rather than an empty success: a
        // tool result that says nothing reads to a model as a command that ran
        // and printed nothing.
        Err(error) => Err(format!(
            "No `openagents` CLI is available: nothing named `openagents` is on PATH, \
             and this binary's own path could not be read: {error}"
        )),
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

async fn run_openagents_cli(args: &[String], cancel: &mut watch::Receiver<bool>) -> (String, bool) {
    /// How long the CLI may run unanswerable before the watchdog kills it.
    ///
    /// Generous: a real forge call answers in seconds; the occasional local
    /// report takes a few. Ten minutes is far past any legitimate answer and
    /// far short of a turn budget burned on a hang.
    const CLI_WATCHDOG: std::time::Duration = std::time::Duration::from_secs(600);

    /// Quote argv the way a shell would, for the `[argv: ...]` header.
    fn shell_words_quote(args: &[String]) -> String {
        args.iter()
            .map(|arg| {
                if !arg.is_empty()
                    && arg
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_./:=@+".contains(&(c as u8)))
                {
                    arg.clone()
                } else {
                    format!("'{}'", arg.replace('\'', "'\\''"))
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    let (program, source) = match resolve_openagents_cli() {
        Ok(resolved) => resolved,
        Err(error) => return (error, true),
    };

    // Which program answered is part of the result. The two differ in what
    // they support, so a model reading `unknown command` needs to know which
    // CLI said it rather than guessing. The effective argv follows: a coerced
    // or truncated arg list (#180) must be visible in the transcript, not
    // reconstructible only from a JSON export.
    let note = match source {
        OpenAgentsCliSource::Path => {
            format!(
                "[ran the `openagents` CLI on PATH: {}]\n[argv: openagents {}]",
                program.display(),
                shell_words_quote(args)
            )
        }
        OpenAgentsCliSource::ThisBinary => format!(
            "[no `openagents` on PATH; ran this binary instead: {}]\n[argv: openagents {}]",
            program.display(),
            shell_words_quote(args)
        ),
    };

    let mut cmd = Command::new(&program);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Stdin is closed, never inherited. The coder runtime has no stdin worth
    // reading, so an inherited pipe hands any stdin-reading flag (`--body-file
    // -`, `--token-stdin`) a read that blocks forever — the #178/#180 hang.
    // Closed stdin is an immediate EOF the CLI can refuse with an error.
    cmd.stdin(Stdio::null());
    cmd.kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            return (
                format!("{note}\n\nFailed to run the openagents CLI: {error}"),
                true,
            );
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stdout {
            let _ = stream.read_to_end(&mut bytes).await;
        }
        bytes
    });
    let stderr = tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stderr {
            let _ = stream.read_to_end(&mut bytes).await;
        }
        bytes
    });
    let status = tokio::select! {
        status = child.wait() => status,
        changed = cancel.changed() => {
            if changed.is_ok() && *cancel.borrow() {
                crate::signals::stop_tree(&mut child).await;
                let _ = stdout.await;
                let _ = stderr.await;
                return (CANCELLED_TOOL_RESULT.to_string(), true);
            }
            child.wait().await
        }
        // The watchdog. A CLI that produces nothing for this long is hung —
        // most plausibly on a stdin read the harness can never satisfy
        // (#178/#180) — and a hang must cost the bound, not the turn.
        _ = tokio::time::sleep(CLI_WATCHDOG) => {
            crate::signals::stop_tree(&mut child).await;
            let _ = stdout.await;
            let _ = stderr.await;
            return (
                format!(
                    "{note}\n\nTimed out: the openagents CLI produced no exit within {:?} \
                     and was killed. If it was reading stdin, no stdin will ever arrive — \
                     pass a file path instead of `-` (see issues #178/#180).",
                    CLI_WATCHDOG
                ),
                true,
            );
        }
    };
    let stdout = stdout.await.unwrap_or_default();
    let stderr = stderr.await.unwrap_or_default();
    match status {
        Ok(status) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&stdout));
            combined.push_str(&String::from_utf8_lossy(&stderr));
            (
                format!("{note}\n\n{}", strip_terminal_escapes(&combined).trim()),
                !status.success(),
            )
        }
        Err(error) => (
            format!("{note}\n\nFailed to wait for the openagents CLI: {error}"),
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn write_skill(root: &Path, dir: &str, source: &str) {
        let at = root.join(".agents").join("skills").join(dir);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join("SKILL.md"), source).unwrap();
    }

    // ───────────────────────────────────────────── the destructive-command gate

    #[test]
    fn the_gate_refuses_a_destructive_command_that_is_no_literal_it_knows() {
        // Every one of these passed the three-string check this replaced.
        for command in [
            "rm -fr ~/",
            "rm -rf --no-preserve-root /",
            "sudo rm -Rf $HOME/*",
            "mkfs.ext4 /dev/sda1",
            "dd if=/dev/zero of=/dev/disk2 bs=1m",
            "diskutil eraseDisk JHFS+ Blank /dev/disk3",
            "sudo shutdown -h now",
            ":(){ :|:& };:",
            "chmod -R 777 /",
            "cat payload > /dev/rdisk0",
        ] {
            assert!(
                check_shell_refusal(command).is_some(),
                "`{command}` should be refused"
            );
        }
    }

    #[test]
    fn the_gate_leaves_ordinary_work_alone() {
        // A refusal list that catches ordinary commands is one an agent learns
        // to work around, which is worse than not having it.
        for command in [
            "rm -rf target/debug",
            "rm -rf ./node_modules",
            "cargo test -p openagents-cli",
            "git rm -r --cached .",
            "mkdir -p /tmp/build && dd if=in of=out",
            "echo 'shutdown the server' >> notes.md",
        ] {
            assert_eq!(
                check_shell_refusal(command),
                None,
                "`{command}` is ordinary work and should run"
            );
        }
    }

    #[test]
    fn a_refusal_says_what_the_command_would_have_done() {
        let refusal = check_shell_refusal("rm -rf ~/").expect("refused");
        assert!(
            refusal.contains("erase a root or a home directory"),
            "{refusal}"
        );
        assert!(refusal.contains("name the directory"), "{refusal}");
    }

    // ─────────────────────────────────────────────────── skills, as they are read

    #[test]
    fn a_block_scalar_description_is_the_block_and_not_its_marker() {
        // A skill written with `description: |` was catalogued as describing
        // itself as ">-" by a parser that took the marker for the value.
        let source = "---\nname: effect\ndescription: |\n  Opinionated guide for Effect v4.\n  Use when implementing workflows.\nlicense: MIT\n---\n\n# Effect\n\nBody.\n";
        let (name, description, auto) = parse_skill_front_matter(source).expect("parsed");
        assert_eq!(name, "effect");
        assert_eq!(
            description,
            "Opinionated guide for Effect v4.\nUse when implementing workflows."
        );
        assert!(!auto);
        assert_eq!(skill_body(source), "# Effect\n\nBody.");
    }

    #[test]
    fn a_folded_block_description_is_one_paragraph() {
        let source =
            "---\nname: folded\ndescription: >-\n  First line\n  second line.\n---\nBody.\n";
        let (_, description, _) = parse_skill_front_matter(source).expect("parsed");
        assert_eq!(description, "First line second line.");
    }

    #[test]
    fn a_skill_missing_a_name_or_a_description_is_not_a_skill() {
        // It could not be asked for, or gives the model nothing to choose on.
        assert!(parse_skill_front_matter("---\ndescription: no name here\n---\nBody").is_none());
        assert!(parse_skill_front_matter("---\nname: nameless\n---\nBody").is_none());
        assert!(parse_skill_front_matter("# No front matter at all\n").is_none());
    }

    #[test]
    fn the_nearest_skills_directory_keeps_a_contested_name() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "shared",
            "---\nname: shared\ndescription: The repository's.\n---\nRepo body.\n",
        );
        let shipped = root.path().join("skills").join("shared");
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::write(
            shipped.join("SKILL.md"),
            "---\nname: shared\ndescription: The CLI's.\n---\nShipped body.\n",
        )
        .unwrap();

        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let skill = registry.skills.get("shared").expect("found");
        assert_eq!(skill.description, "The repository's.");
        assert_eq!(skill.body, "Repo body.");
    }

    #[test]
    fn the_skill_tool_offers_names_and_descriptions_and_never_a_body() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "brewing",
            "---\nname: brewing\ndescription: How to make tea.\n---\nSTEEP_FOR_FOUR_MINUTES\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let tools = registry.list_tools();
        let skill_tool = tools.iter().find(|t| t.name == "skill").expect("declared");

        assert!(
            skill_tool
                .description
                .contains("`brewing`: How to make tea.")
        );
        // The catalog is what a session pays for on every turn; a body in it
        // is 46 KB of instructions the model may never use.
        assert!(
            !skill_tool.description.contains("STEEP_FOR_FOUR_MINUTES"),
            "the catalog carried a body"
        );
    }

    #[tokio::test]
    async fn reading_a_skill_returns_its_body_and_says_where_it_came_from() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "brewing",
            "---\nname: brewing\ndescription: How to make tea.\n---\nSTEEP_FOR_FOUR_MINUTES\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));

        let out = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "skill".to_string(),
                arguments: serde_json::json!({"name": "brewing"}),
            })
            .await;
        assert!(!out.is_error);
        assert!(
            out.output.contains("STEEP_FOR_FOUR_MINUTES"),
            "{}",
            out.output
        );
        assert!(out.output.contains("SKILL.md"), "{}", out.output);

        let missing = registry
            .execute_tool(&ToolCall {
                id: "2".to_string(),
                name: "skill".to_string(),
                arguments: serde_json::json!({"name": "smelting"}),
            })
            .await;
        assert!(missing.is_error);
    }

    #[test]
    fn only_an_auto_skill_reaches_the_standing_context() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "method",
            "---\nname: method\ndescription: How to approach the work.\nauto: true\n---\nWORK_THIS_WAY\n",
        );
        write_skill(
            root.path(),
            "asked-for",
            "---\nname: asked-for\ndescription: Read on request.\n---\nONLY_ON_REQUEST\n",
        );
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));

        let context = registry
            .standing_context()
            .expect("an auto skill is injected");
        assert!(context.contains("WORK_THIS_WAY"), "{context}");
        assert!(
            !context.contains("ONLY_ON_REQUEST"),
            "a skill nobody asked for was injected: {context}"
        );
    }

    #[test]
    fn a_workspace_with_no_auto_skill_injects_nothing() {
        let root = tempfile::tempdir().unwrap();
        write_skill(
            root.path(),
            "plain",
            "---\nname: plain\ndescription: Read on request.\n---\nBody.\n",
        );
        // A temporary directory is not named for either OpenAgents repository,
        // so the workspace note does not apply either.
        assert!(
            HarnessToolRegistry::new(Some(root.path().to_path_buf()))
                .standing_context()
                .is_none()
        );
    }

    #[test]
    fn the_shipped_repository_skills_are_discovered_and_the_auto_one_is_injected() {
        // End to end against the real `.agents/skills` tree rather than a
        // fixture, because the fixture is what a broken discovery path still
        // passes.
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo.join(".agents").join("skills").is_dir() {
            return;
        }
        let registry = HarnessToolRegistry::new(Some(repo.clone()));
        assert!(
            registry.skills.contains_key("fast-follow"),
            "discovered: {:?}",
            registry.skills.keys().collect::<Vec<_>>()
        );
        // The `effect` skill's description is a `|` block; a parser that took
        // the marker would catalogue it as "|".
        let effect = registry.skills.get("effect").expect("the effect skill");
        assert!(effect.description.len() > 20, "{:?}", effect.description);

        // `superdelegate` ships with `auto: true`, so its body is the standing
        // context this session starts with.
        let context = registry.standing_context().expect("something is injected");
        assert!(
            registry
                .skills
                .get("superdelegate")
                .is_some_and(|skill| skill.auto),
            "superdelegate is the repository's auto skill"
        );
        let body = &registry.skills["superdelegate"].body;
        assert!(
            context.contains(&body[..80.min(body.len())]),
            "the auto body was not injected"
        );
    }

    // ───────────────────────────────────────────── the capability tool wiring

    #[test]
    fn every_declared_tool_has_an_arm_that_answers_it() {
        // The property the missing `capability` implementation broke: a name
        // in `list_tools` that no arm answers is a promise nothing keeps.
        let root = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::with_delegation(
            Some(root.path().to_path_buf()),
            DelegationGate {
                lane: "test".to_string(),
                user_token: None,
                api_base: None,
                max_count: 2,
                child: Default::default(),
                acp_agents: Vec::new(),
                acp_spent: Arc::new(AtomicBool::new(false)),
            },
        );
        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec![
                "read",
                "write",
                "edit",
                "bash",
                "shell",
                "skill",
                "checkpoint",
                "openagents",
                "capability",
                "delegate"
            ]
        );
        // The same nine names `plugins::validate_manifest` refuses a plugin
        // for taking. An arm added here and not there would leave a name a
        // plugin can claim and never be called under.
        assert_eq!(
            names,
            BUILTIN_TOOL_NAMES.to_vec(),
            "the reserved list and the arms disagree"
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();
        for name in &names {
            if name == "delegate" {
                continue; // Starting real children is not this test's business.
            }
            let out = runtime.block_on(registry.execute_tool(&ToolCall {
                id: "1".to_string(),
                name: name.clone(),
                arguments: serde_json::json!({}),
            }));
            assert!(
                !out.output.starts_with("Unknown tool:"),
                "`{name}` is declared and unanswered"
            );
        }
    }

    #[tokio::test]
    async fn the_read_only_pool_refuses_write_and_edit() {
        let root = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::with_tool_pool(
            Some(root.path().to_path_buf()),
            ToolPool::ReadOnly,
        );
        let names = registry
            .list_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["read", "bash", "skill"]);

        for name in [
            "write",
            "edit",
            "shell",
            "delegate",
            "openagents",
            "capability",
        ] {
            let output = registry
                .execute_tool(&ToolCall {
                    id: name.to_string(),
                    name: name.to_string(),
                    arguments: serde_json::json!({}),
                })
                .await;
            assert!(output.is_error, "`{name}` ran: {}", output.output);
            assert!(output.output.contains("read-only"), "{}", output.output);
        }
    }

    #[test]
    fn the_read_write_and_all_pools_declare_their_exact_tools() {
        let root = tempfile::tempdir().unwrap();
        let names = |pool| {
            HarnessToolRegistry::with_tool_pool(Some(root.path().to_path_buf()), pool)
                .list_tools()
                .into_iter()
                .map(|tool| tool.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(ToolPool::ReadWrite),
            vec!["read", "write", "edit", "bash", "shell", "skill"]
        );
        let all = names(ToolPool::All);
        assert!(all.contains(&"capability".to_string()), "{all:?}");
        assert!(!all.contains(&"delegate".to_string()), "{all:?}");
    }

    // ───────────────────────────────────────────────── tools the host answers

    fn host_tool(name: &str, answer: &'static str) -> HostTool {
        HostTool {
            definition: ToolDefinition {
                name: name.to_string(),
                description: "A tool the front-end answers.".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
            run: Arc::new(move |call: &ToolCall, _cancel| {
                let id = call.id.clone();
                Box::pin(async move { (format!("{answer} for {id}"), false) })
            }),
        }
    }

    #[tokio::test]
    async fn a_host_tool_is_declared_alongside_the_built_ins_and_answers() {
        let root = tempfile::tempdir().unwrap();
        let mut registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        registry
            .add_host_tool(host_tool("acp", "handed off"))
            .unwrap();

        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec![
                "read",
                "write",
                "edit",
                "bash",
                "shell",
                "skill",
                "checkpoint",
                "openagents",
                "capability",
                "acp"
            ]
        );

        let out = registry
            .execute_tool(&ToolCall {
                id: "call_1".to_string(),
                name: "acp".to_string(),
                arguments: serde_json::json!({}),
            })
            .await;
        assert!(!out.is_error);
        assert_eq!(out.output, "handed off for call_1");
    }

    /// Two answers to one name is a model told a tool does one thing while
    /// something else does another.
    #[test]
    fn a_host_tool_cannot_take_a_built_in_name_or_its_own_twice() {
        let root = tempfile::tempdir().unwrap();
        let mut registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        for reserved in BUILTIN_TOOL_NAMES {
            let refusal = registry
                .add_host_tool(host_tool(reserved, "x"))
                .expect_err("a reserved name was accepted");
            assert!(refusal.contains(reserved), "{refusal}");
        }
        registry.add_host_tool(host_tool("acp", "x")).unwrap();
        assert!(registry.add_host_tool(host_tool("acp", "y")).is_err());
        // And the refused names declared nothing.
        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(names.iter().filter(|n| *n == "acp").count(), 1);
    }

    /// The collision `validate_manifest` cannot see. A plugin is installed
    /// and valid; a front end then declares a host tool of the same name. The
    /// host tool is dispatched first, so the plugin would be declared to the
    /// model and never reached — the same defect `name_reserved` exists to
    /// prevent, one layer up.
    #[test]
    fn a_host_tool_cannot_shadow_an_installed_plugin() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo
            .join("plugins")
            .join("word-stats")
            .join("manifest.json")
            .is_file()
        {
            return;
        }
        let mut registry = HarnessToolRegistry::new(Some(repo));
        assert!(
            registry.catalog.iter().any(|e| e.name == "word_stats"),
            "the checked-in catalog was not discovered"
        );

        let refusal = registry
            .add_host_tool(host_tool("word_stats", "the host's answer"))
            .expect_err("a host tool shadowed an installed plugin");
        assert!(refusal.contains("word_stats"), "{refusal}");
        assert!(refusal.contains("shadow"), "{refusal}");
        // And nothing was declared under the contested name twice.
        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(names.iter().filter(|n| *n == "word_stats").count(), 0);
    }

    #[tokio::test]
    async fn a_capability_search_names_a_plugin_and_loading_it_declares_its_tool() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo
            .join("plugins")
            .join("word-stats")
            .join("manifest.json")
            .is_file()
        {
            return;
        }
        let registry = HarnessToolRegistry::new(Some(repo));
        assert!(
            !registry.catalog.is_empty(),
            "the checked-in catalog was not discovered"
        );

        let search = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"query": "statistics about the longest word in a piece of text"}),
            })
            .await;
        assert!(search.output.contains("word_stats"), "{}", search.output);
        // Nothing is loaded by searching, so no plugin tool is declared yet.
        assert!(registry.list_tools().iter().all(|t| t.name != "word_stats"));

        let load = registry
            .execute_tool(&ToolCall {
                id: "2".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "word_stats"}),
            })
            .await;
        assert!(load.output.contains("digest verified"), "{}", load.output);
        let word_stats = registry
            .list_tools()
            .into_iter()
            .find(|t| t.name == "word_stats")
            .expect("the loaded plugin declares its tool");
        assert_eq!(
            word_stats.parameters["properties"]["text"]["type"],
            "string"
        );

        // And the tool the plugin declared runs the plugin.
        let ran = registry
            .execute_tool(&ToolCall {
                id: "3".to_string(),
                name: "word_stats".to_string(),
                arguments: serde_json::json!({"text": "alpha beta beta"}),
            })
            .await;
        let value: serde_json::Value = serde_json::from_str(&ran.output).expect(&ran.output);
        assert_eq!(value["ok"]["words"], 3);
        assert_eq!(value["ok"]["top_word"]["word"], "beta");
    }

    #[tokio::test]
    async fn a_mounted_capability_refuses_to_load_without_an_operator() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        if !repo
            .join("plugins")
            .join("file-stats")
            .join("manifest.json")
            .is_file()
        {
            return;
        }
        let unattended = HarnessToolRegistry::new(Some(repo.clone()));
        let refused = unattended
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "file_stats"}),
            })
            .await;
        assert!(
            refused.output.contains("approval_unavailable"),
            "{}",
            refused.output
        );
        assert!(
            unattended
                .list_tools()
                .iter()
                .all(|t| t.name != "file_stats")
        );

        let attended = HarnessToolRegistry::new(Some(repo)).allowing_plugin_mounts();
        let loaded = attended
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "file_stats"}),
            })
            .await;
        assert!(
            loaded.output.contains("digest verified"),
            "{}",
            loaded.output
        );
        assert!(attended.list_tools().iter().any(|t| t.name == "file_stats"));
    }

    #[tokio::test]
    async fn a_capability_that_is_not_installed_is_said_to_be_missing() {
        let root = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(root.path().to_path_buf()));
        let out = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "capability".to_string(),
                arguments: serde_json::json!({"name": "does_not_exist"}),
            })
            .await;
        assert!(out.is_error);
        assert!(out.output.contains("does_not_exist"), "{}", out.output);
    }
}

#[cfg(test)]
mod defect_tests {
    use super::*;

    /// `&combined[..OUTPUT_LIMIT]` is a *byte* index into a `String`. The first
    /// time a command printed an accent or an emoji straddling the limit, the
    /// slice panicked and took the whole agent process with it — before the
    /// thread could even be revoked. Any `git log`, test run, or file with a
    /// non-ASCII character past 30 kB did it.
    #[test]
    fn truncation_survives_a_multibyte_character_on_the_boundary() {
        // 29,999 ASCII bytes, then a 3-byte character straddling byte 30,000.
        let mut text = "a".repeat(OUTPUT_LIMIT - 1);
        text.push_str("€€€");
        assert!(
            !text.is_char_boundary(OUTPUT_LIMIT),
            "the probe must straddle"
        );

        let cut = floor_char_boundary(&text, OUTPUT_LIMIT);
        // The old code did `&text[..OUTPUT_LIMIT]`, which panics here.
        let head = &text[..cut];

        assert_eq!(cut, OUTPUT_LIMIT - 1, "it must step back to the boundary");
        assert!(head.ends_with('a'));
        assert!(text.len() > cut, "there was something to truncate");
    }

    #[test]
    fn a_boundary_that_is_already_clean_is_left_alone() {
        let text = "a".repeat(OUTPUT_LIMIT + 10);
        assert_eq!(floor_char_boundary(&text, OUTPUT_LIMIT), OUTPUT_LIMIT);
    }

    #[test]
    fn a_short_string_is_never_cut() {
        let text = "€€€";
        assert_eq!(floor_char_boundary(text, OUTPUT_LIMIT), text.len());
    }

    /// Every shell result was reported to the model as `is_error: false`, so a
    /// failing build read exactly like a passing one.
    #[tokio::test]
    async fn a_failing_command_is_reported_as_a_failure() {
        let dir = std::env::temp_dir();
        let (_stop, mut cancel) = watch::channel(false);
        let (text, failed) = run_real_shell("exit 7", &dir, 30, &mut cancel).await;
        assert!(failed, "exit 7 must be reported as an error, got: {text}");
        assert!(text.contains('7'), "the code belongs in the text: {text}");

        let (text, failed) = run_real_shell("true", &dir, 30, &mut cancel).await;
        assert!(!failed, "a successful command must not be an error: {text}");
    }

    /// A timeout is not a successful result either.
    #[tokio::test]
    async fn a_timed_out_command_is_reported_as_a_failure() {
        let dir = std::env::temp_dir();
        let (_stop, mut cancel) = watch::channel(false);
        let (text, failed) = run_real_shell("sleep 5", &dir, 1, &mut cancel).await;
        assert!(failed, "a timeout must be an error, got: {text}");
        assert!(text.contains("timed out"), "{text}");
    }

    // ───────────────────────────────────────── keeping long output on disk

    /// The point of the log: a truncated result names where the rest went,
    /// so the follow-up question is a `grep` and not a second two-minute
    /// suite run. This is the exact shape of trajectory 2026-08-27 step 48.
    #[tokio::test]
    async fn a_long_command_keeps_its_whole_output_and_names_the_file() {
        let session = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let command = "echo line-0; echo line-1; sleep 31; echo tail-marker";

        let (_stop, mut cancel) = watch::channel(false);
        // The wait itself buys the 30-second persistence threshold, so the
        // command is real work plus real waiting rather than a mocked clock.
        let (text, failed) =
            run_real_shell_logged(command, work.path(), 60, &mut cancel, Some(session.path()))
                .await;
        assert!(!failed);
        assert!(text.contains("tail-marker"));

        let logs: Vec<_> = std::fs::read_dir(session.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("cmd-"))
            .collect();
        assert_eq!(logs.len(), 1, "one command, one log");
        let logged = std::fs::read_to_string(logs[0].path()).unwrap();
        assert!(logged.starts_with("$ "), "the log says what ran: {logged}");
        assert!(logged.contains("line-0"), "{logged}");
        assert!(logged.contains("tail-marker"), "{logged}");
    }

    #[tokio::test]
    async fn a_truncated_result_points_at_the_persisted_log() {
        let session = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        // 300 lines of ~55 bytes plus the marker clears the 30_000-character
        // excerpt limit once both streams are counted; the marker line after
        // the sleep proves the log holds what the excerpt cut off. Real
        // `sleep` rather than a mocked clock, because the threshold is
        // defined in wall time.
        let command = "for i in $(seq 1 1200); do echo padding-line-$i xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; done; sleep 31; echo END-MARKER";

        let (_stop, mut cancel) = watch::channel(false);
        let (text, failed) =
            run_real_shell_logged(command, work.path(), 90, &mut cancel, Some(session.path()))
                .await;
        assert!(!failed);
        assert!(
            text.contains("Full output kept at"),
            "the excerpt names the file: {text}"
        );
        assert!(
            text.contains("history_recall"),
            "the excerpt names recall as the way to read the log: {text}"
        );
        assert!(
            text.contains(r#"{"_tag":"Grep","pattern":"..."}"#),
            "the excerpt shows the Grep question shape: {text}"
        );

        let log_path = text
            .split("Full output kept at `")
            .nth(1)
            .and_then(|rest| rest.split('`').next())
            .expect("a path in the notice")
            .to_string();
        let logged = std::fs::read_to_string(&log_path).unwrap();
        assert!(logged.contains("END-MARKER"), "whole output survived");
        assert!(logged.len() > OUTPUT_LIMIT);
    }

    /// The truncation notice's recovery clause is the wording a follow-up
    /// turn has to see; pin it without waiting on a 30-second persist.
    #[test]
    fn the_truncation_notice_points_at_history_recall() {
        assert!(HISTORY_RECALL_GREP.contains("history_recall"));
        assert!(
            HISTORY_RECALL_GREP.contains(r#"{"_tag":"Grep","pattern":"..."}"#),
            "{HISTORY_RECALL_GREP}"
        );
        let notice = format!("Full output kept at `cmd-3.log`. {HISTORY_RECALL_GREP}");
        assert!(notice.contains("without re-running"), "{notice}");
    }

    #[tokio::test]
    async fn a_short_command_writes_no_log() {
        let session = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let (_stop, mut cancel) = watch::channel(false);
        let _ = run_real_shell_logged(
            "echo hi",
            work.path(),
            30,
            &mut cancel,
            Some(session.path()),
        )
        .await;
        let entries = std::fs::read_dir(session.path()).unwrap().count();
        assert_eq!(entries, 0, "fast quiet work leaves no file behind");
    }

    // ─────────────────────────────── the repeated-execution gate

    /// The shape that cost five minutes in trajectory 2026-08-27 step 55:
    /// three executions of one suite because each carried a different grep.
    /// This — and only this shape of thing — is what the gate is for.
    #[test]
    fn the_gate_refuses_a_one_liner_that_executes_a_command_twice() {
        let refusal = check_duplicate_execution(
            "pnpm run test:rust 2>&1 | grep -E 'test result' | head; \
             pnpm run test:rust 2>&1 | grep -c 'test result: ok'; \
             pnpm run test:rust 2>&1 | grep FAILED",
        )
        .expect("three runs of one suite are two too many");
        assert!(refusal.contains("pnpm run test:rust"), "{refusal}");
        assert!(refusal.contains("tee"), "the refusal names the way out");
        assert!(
            refusal.contains("history_recall"),
            "the refusal names recall as the way to read the earlier run: {refusal}"
        );
    }

    /// The regression this gate shipped with: it refused *every* repeated
    /// head, so two greps over one file — milliseconds each — bought a
    /// refusal that costs a turn. Repeats are only work when repeating
    /// them costs something.
    #[test]
    fn cheap_reads_repeat_free() {
        // Different searches over the same evidence: exactly the shape the
        // old gate refused three of in one session.
        assert!(
            check_duplicate_execution(
                "grep -n 'fn answer_edit' src/tools.rs; grep -n 'resolve_path' src/tools.rs"
            )
            .is_none()
        );
        assert!(check_duplicate_execution("rg -n 'head' a.rs; rg -n 'head' b.rs").is_none());
        assert!(check_duplicate_execution("sed -n '1,10p' f.txt; sed -n '11,20p' f.txt").is_none());
        assert!(check_duplicate_execution("cat a; cat b").is_none());
        assert!(check_duplicate_execution("ls; ls -la").is_none());
    }

    /// A mutating repeat is refused on danger, not on time: the second run
    /// acts on whatever the first run changed, so it is a different action.
    #[test]
    fn mutating_heads_refuse_even_when_quick() {
        let refusal = check_duplicate_execution("git stash pop; git stash pop")
            .expect("two pops are two different stashes");
        assert!(refusal.contains("not the same action"), "{refusal}");
        assert!(check_duplicate_execution("rm -rf tmp/a; rm -rf tmp/b").is_some());
    }

    #[test]
    fn a_single_run_of_a_command_is_never_the_gate() {
        // One execution with three post-processing stages is one run. Piping
        // through several consumers is exactly what the tool description
        // already encourages.
        assert!(
            check_duplicate_execution(
                "vp test --run --reporter=json --outputFile=/tmp/v.json 2>&1 | tee /tmp/raw.log | grep FAIL | head"
            )
            .is_none()
        );
        // Distinct commands to distinct heads are unrelated work.
        assert!(check_duplicate_execution("git status && git log --oneline -5 && ls").is_none());
    }

    #[test]
    fn the_head_check_distinguishes_commands_by_first_argument() {
        // `cargo build` and `cargo test` share an executable but not work;
        // refusing those would break ordinary compound lines.
        assert!(check_duplicate_execution("cargo build --release && cargo test").is_none());
        // But `cargo test` twice re-pays a build-and-suite each time.
        assert!(check_duplicate_execution("cargo test lib && cargo test doc").is_some());
    }

    #[test]
    fn quoted_and_substituted_segments_are_not_split() {
        // The separators inside quotes and substitutions do not count as
        // boundaries, so this is one echo followed by a different head.
        assert!(
            check_duplicate_execution(r#"echo "a; b | c && d" && echo "$(git status)" "#).is_none(),
            "one echo plus one echo is two different heads at worst"
        );
        // A repeat behind quotes is still a repeat: the quote is on the word,
        // and the identity survives the strip. Two suites are one suite twice.
        assert!(check_duplicate_execution(r#"cargo test; 'cargo' test"#).is_some());
        // But two distinct commands, however quoted, run free.
        assert!(check_duplicate_execution(r#"echo hi; 'echo' again"#).is_none());
    }

    #[test]
    fn environment_prefixes_do_not_hide_a_repeat() {
        assert!(
            check_duplicate_execution(
                "RUST_BACKTRACE=1 cargo test && RUST_BACKTRACE=full cargo test"
            )
            .is_some(),
            "same command under different env assignments still re-pays the suite"
        );
    }

    #[cfg(unix)]
    async fn wait_for_pid(path: &Path) -> u32 {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(text) = tokio::fs::read_to_string(path).await {
                    if let Ok(pid) = text.trim().parse() {
                        return pid;
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the shell wrote its child pid")
    }

    #[cfg(unix)]
    fn process_exists(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancel_stops_the_shell_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("child.pid");
        let cwd = dir.path().to_path_buf();
        let (stop, mut cancel) = watch::channel(false);
        let task = tokio::spawn(async move {
            run_real_shell(
                "sleep 30 & echo $! > child.pid; wait",
                &cwd,
                60,
                &mut cancel,
            )
            .await
        });

        let child_pid = wait_for_pid(&pid_file).await;
        stop.send(true).unwrap();
        let (output, failed) = task.await.unwrap();

        assert!(failed);
        assert_eq!(output, CANCELLED_TOOL_RESULT);
        assert!(
            !process_exists(child_pid),
            "child {child_pid} survived cancellation"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn one_cancel_stops_multiple_shell_tools() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_path_buf();
        let registry = Arc::new(HarnessToolRegistry::new(Some(cwd.clone())));
        let (stop, cancel) = watch::channel(false);
        let first_call = ToolCall {
            id: "one".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({
                "command": "sleep 30 & echo $! > one.pid; wait"
            }),
        };
        let second_call = ToolCall {
            id: "two".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({
                "command": "sleep 30 & echo $! > two.pid; wait"
            }),
        };
        let first_registry = Arc::clone(&registry);
        let first_cancel = cancel.clone();
        let first = tokio::spawn(async move {
            first_registry
                .execute_tool_cancellable(&first_call, first_cancel)
                .await
        });
        let second_registry = Arc::clone(&registry);
        let second = tokio::spawn(async move {
            second_registry
                .execute_tool_cancellable(&second_call, cancel)
                .await
        });

        let one = wait_for_pid(&cwd.join("one.pid")).await;
        let two = wait_for_pid(&cwd.join("two.pid")).await;
        stop.send(true).unwrap();
        let (first, second) = tokio::join!(first, second);

        for output in [first.unwrap(), second.unwrap()] {
            assert!(output.is_error);
            assert_eq!(output.output, CANCELLED_TOOL_RESULT);
        }
        assert!(!process_exists(one));
        assert!(!process_exists(two));
    }

    // ─────────────────────────────────────────── read, write, edit and bash

    fn file_call(name: &str, arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "1".to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    /// The rule that makes a surgical edit safe without a diff format: text
    /// appearing more than once does not say which one was meant, so the edit
    /// is refused — and, the half that matters, the file is left alone. An
    /// `edit` that replaced the first hit would silently change the wrong line
    /// and report success.
    #[tokio::test]
    async fn an_edit_whose_text_appears_twice_is_refused_and_the_file_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        let before = "let x = 1;\nlet y = 2;\nlet x = 1;\n";
        std::fs::write(dir.path().join("a.rs"), before).unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "a.rs", "oldText": "let x = 1;", "newText": "let x = 9;"}),
            ))
            .await;

        assert!(out.is_error, "a refused edit is a failure: {}", out.output);
        assert!(
            out.output.contains("appears 2 times"),
            "the refusal must say how many: {}",
            out.output
        );
        assert!(
            out.output.contains("unique"),
            "the refusal must say what to do about it: {}",
            out.output
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            before,
            "a refused edit wrote to the file"
        );
    }

    /// Multi-line misses used to key on the first submitted line. Early lines
    /// are boilerplate (`let dir = ...`) that appear all over the file, so the
    /// side-by-side landed on a struct doc instead of the fixture the model
    /// was aiming at. Distinctive-line alignment reports the rare line.
    #[test]
    fn a_multiline_miss_aligns_on_the_distinctive_line_not_the_first() {
        let file = "\
/// Recalled history.
pub excerpt: String
let dir = tempfile::tempdir().unwrap();
let dir = tempfile::tempdir().unwrap();
fn the_step_48_49_shape_resolves_by_recall_not_reexecution() {
    let needle = r#\"cap)}]}\"#;
}
";
        let old = "\
let dir = tempfile::tempdir().unwrap();
    let needle = r#\"cap)}]}\"#;
";
        let report = diagnose_miss(file, old).expect("a miss has a region");
        assert!(
            report.contains("the_step_48_49_shape_resolves_by_recall_not_reexecution")
                || report.contains("cap)}]}\"#"),
            "the region must be the fixture, not the struct docs: {report}"
        );
        assert!(
            !report.contains("pub excerpt: String"),
            "boilerplate first-line alignment leaked the struct docs: {report}"
        );
        assert!(
            report.contains("submitted line 2") || report.contains("Your line 2"),
            "the distinctive line is the second submitted line: {report}"
        );
    }

    /// One-line misses keep the historical first-line window so existing
    /// diagnostics do not change shape.
    #[test]
    fn a_single_line_miss_still_names_line_1() {
        let file = "alpha\nbeta\ngamma\n";
        let report = diagnose_miss(file, "bet").expect("substring match");
        assert!(
            report.contains("Closest region for the first submitted line"),
            "{report}"
        );
        assert!(report.contains("Your line 1"), "{report}");
    }

    /// The other refusal. Text that is not there cannot be replaced, and the
    /// model needs to be told that rather than handed an empty result.
    #[tokio::test]
    async fn an_edit_whose_text_is_not_there_is_refused_and_the_file_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        let before = "one\ntwo\n";
        std::fs::write(dir.path().join("a.txt"), before).unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "a.txt", "oldText": "three", "newText": "four"}),
            ))
            .await;

        assert!(out.is_error, "{}", out.output);
        assert!(!out.output.trim().is_empty(), "a refusal is never silent");
        assert!(out.output.contains("does not appear"), "{}", out.output);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            before
        );
    }

    /// The success path, and the one thing about it worth pinning: only the
    /// matched run changes.
    #[tokio::test]
    async fn a_unique_edit_replaces_that_run_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        std::fs::write(dir.path().join("a.txt"), "keep\nchange me\nkeep\n").unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "a.txt", "oldText": "change me", "newText": "changed"}),
            ))
            .await;

        assert!(!out.is_error, "{}", out.output);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "keep\nchanged\nkeep\n"
        );
    }

    /// A run long enough to persist leaves the *command itself* in the log,
    /// so a grep of the file says what produced the output it holds (#152).
    #[tokio::test]
    async fn a_persisted_log_says_what_ran() {
        let session = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let command = "echo header-marker; sleep 31; echo tail-marker";

        let (_stop, mut cancel) = watch::channel(false);
        let (_text, failed) =
            run_real_shell_logged(command, work.path(), 60, &mut cancel, Some(session.path()))
                .await;
        assert!(!failed);

        let logs: Vec<_> = std::fs::read_dir(session.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("cmd-"))
            .collect();
        assert_eq!(logs.len(), 1, "one command, one log");
        let logged = std::fs::read_to_string(logs[0].path()).unwrap();
        assert!(logged.contains("header-marker"), "{logged}");
        assert!(logged.contains("tail-marker"), "{logged}");
    }

    /// A long *failing* run is logged like a passing one: the failure names
    /// are the part a follow-up greps for, and they are exactly what a
    /// bounded reply is most likely to have cut.
    #[tokio::test]
    async fn a_failing_logged_run_keeps_its_output_too() {
        let session = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let command = "for i in $(seq 1 1200); do echo padding-line-$i xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; done; sleep 31; echo boom; exit 3";

        let (_stop, mut cancel) = watch::channel(false);
        let (_text, failed) =
            run_real_shell_logged(command, work.path(), 90, &mut cancel, Some(session.path()))
                .await;
        assert!(failed);

        let logs: Vec<_> = std::fs::read_dir(session.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("cmd-"))
            .collect();
        assert_eq!(logs.len(), 1, "a failing run is logged too");
        let logged = std::fs::read_to_string(logs[0].path()).unwrap();
        assert!(
            logged.contains("boom"),
            "the failure name is on disk: {logged}"
        );
    }

    /// The escape-sequence tier, on the defect that motivated it: the model
    /// sends two backslashes before the newline where the file holds one,
    /// byte-exact fails, and the ladder lands the edit in one call instead of
    /// three refusals (#160 steps 40/43).
    #[tokio::test]
    async fn an_edit_with_a_doubled_line_continuation_backslash_still_lands() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        // What `sed` showed the model: `working \` then newline (one slash).
        let before =
            "let msg = format!(\n    \"outside this session's working \\\n     directory.\",\n);\n";
        std::fs::write(dir.path().join("jail.rs"), before).unwrap();
        assert_eq!(
            before.matches("working \\\n").count(),
            1,
            "fixture: the file's own line holds one backslash before its newline"
        );

        // What the model resends after JSON double-escaping: two backslashes.
        let doubled = "let msg = format!(\n    \"outside this session's working \\\\\n     directory.\",\n);\n";
        assert_ne!(
            doubled, before,
            "the submitted text must differ from the file"
        );
        assert_eq!(
            doubled.matches("working \\\\\n").count(),
            1,
            "fixture: the submitted line holds two backslashes before its newline"
        );

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "jail.rs", "oldText": doubled, "newText": ""}),
            ))
            .await;

        assert!(
            !out.is_error,
            "the escape tier should land it: {}",
            out.output
        );
        assert!(
            out.output.contains("escape-sequence mismatch"),
            "the reply must say which tier fired: {}",
            out.output
        );
        let after = std::fs::read_to_string(dir.path().join("jail.rs")).unwrap();
        assert!(
            !after.contains("format!"),
            "the file's own bytes were replaced"
        );
        assert_eq!(after.matches("\\\\\n").count(), 0);
    }

    /// Trailing whitespace only, so the first rung catches it and says which
    /// rung fired.
    #[tokio::test]
    async fn an_edit_whose_text_misses_only_trailing_whitespace_is_matched_by_the_first_tier() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        std::fs::write(dir.path().join("w.txt"), "value = 42;   \ncount = 7;\n").unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({
                    "path": "w.txt",
                    "oldText": "value = 42;\ncount = 7;",
                    "newText": "value = 43;\ncount = 8;"
                }),
            ))
            .await;

        assert!(!out.is_error, "{}", out.output);
        assert!(out.output.contains("trailing whitespace"), "{}", out.output);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("w.txt")).unwrap(),
            "value = 43;\ncount = 8;\n",
            "normalization located the site; the written text comes from newText alone"
        );
    }

    /// Looseness never buys a second match site: ambiguous even under the
    /// loosest tier stays refused.
    #[tokio::test]
    async fn a_fuzzy_match_that_hits_twice_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        // Both windows carry a trailing space on their first line, so the
        // exact search misses; both agree once trailing whitespace is
        // ignored -- which must refuse rather than pick one.
        std::fs::write(dir.path().join("d.txt"), "ping \npong\nping \npong\n").unwrap();
        let before = std::fs::read_to_string(dir.path().join("d.txt")).unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "d.txt", "oldText": "ping\npong\n", "newText": "x"}),
            ))
            .await;

        assert!(
            out.is_error,
            "an ambiguous fuzzy hit must be refused: {}",
            out.output
        );
        assert!(out.output.contains("matches 2 places"), "{}", out.output);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("d.txt")).unwrap(),
            before,
            "a refused fuzzy edit wrote to the file"
        );
    }

    /// A total miss now shows the file's real text beside what was sent,
    /// backslash counts visible, instead of one bare sentence.
    #[tokio::test]
    async fn a_total_miss_shows_the_file_region_beside_what_was_sent() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        std::fs::write(dir.path().join("m.rs"), "fn main() {}\nlet p = \"a\\b\";\n").unwrap();

        let out = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": "m.rs", "oldText": "let p = \"a\\\\b\"; x", "newText": ""}),
            ))
            .await;

        assert!(out.is_error, "{}", out.output);
        assert!(out.output.contains("file line"), "{}", out.output);
        assert!(
            out.output.contains('|'),
            "a side-by-side table is the point: {}",
            out.output
        );
    }

    /// A missing file is a refusal the model reads and retries from, not an
    /// unwrap that takes the process with it.
    #[tokio::test]
    async fn reading_a_file_that_is_not_there_is_a_refusal_and_not_a_raise() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));

        let out = registry
            .execute_tool(&file_call(
                "read",
                serde_json::json!({"path": "missing.txt"}),
            ))
            .await;

        assert!(out.is_error, "a missing file is a failure: {}", out.output);
        assert!(
            out.output.contains("missing.txt"),
            "the refusal must name the path: {}",
            out.output
        );
        assert!(!out.output.trim().is_empty(), "a refusal is never silent");
    }

    /// `shell` reported every failure as a success until recently, so a
    /// failing build read like a passing one. `bash` is the same arm, and this
    /// holds it to the same answer through the tool interface.
    #[tokio::test]
    async fn a_bash_command_that_exits_non_zero_is_reported_as_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));

        let failed = registry
            .execute_tool(&file_call("bash", serde_json::json!({"command": "exit 3"})))
            .await;
        assert!(
            failed.is_error,
            "a non-zero exit must be an error: {}",
            failed.output
        );
        assert!(failed.output.contains('3'), "{}", failed.output);

        let worked = registry
            .execute_tool(&file_call(
                "bash",
                serde_json::json!({"command": "echo ok"}),
            ))
            .await;
        assert!(!worked.is_error, "{}", worked.output);
        assert!(worked.output.contains("ok"), "{}", worked.output);
    }

    /// No file tool carries a working-directory jail (#151): a path may name
    /// anything on this machine. `shell` could always reach every one of
    /// those files — `cat` reads, `tee` writes, and nothing between them
    /// asks where the session started — so a jail on the three tools that
    /// cannot run a command constrained nothing real. It cost a quoting-free
    /// path on `read`, and on `write` legitimate destinations: a config into
    /// `$HOME`, a fix under `/etc` while the session holds the host. What
    /// limits any file tool now is the operating system's own permissions,
    /// which is also all that has ever limited `shell`.
    #[tokio::test]
    async fn every_file_tool_serves_a_path_outside_the_working_directory() {
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "not yours\n").unwrap();

        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));

        // Two shapes per tool — an absolute path outside, and a `..`
        // traversal out of the working directory — in an order where every
        // call succeeds: `write` establishes, `read` confirms what landed,
        // `edit` changes it in place.
        let escaped = "../escaped.txt";
        registry
            .execute_tool(&file_call(
                "write",
                serde_json::json!({"path": escaped, "content": "written\n"}),
            ))
            .await;
        let through_read = registry
            .execute_tool(&file_call("read", serde_json::json!({"path": escaped})))
            .await;
        assert!(!through_read.is_error, "{}", through_read.output);
        assert_eq!(through_read.output, "written\n");
        let through_edit = registry
            .execute_tool(&file_call(
                "edit",
                serde_json::json!({"path": escaped, "oldText": "written", "newText": "edited"}),
            ))
            .await;
        assert!(!through_edit.is_error, "{}", through_edit.output);

        let absolute = secret.display().to_string();
        for (tool, arguments) in [
            ("read", serde_json::json!({"path": absolute})),
            (
                "write",
                serde_json::json!({"path": absolute, "content": "mine\n"}),
            ),
            (
                "edit",
                serde_json::json!({"path": absolute, "oldText": "mine", "newText": "ours"}),
            ),
        ] {
            let out = registry.execute_tool(&file_call(tool, arguments)).await;
            assert!(
                !out.is_error,
                "`{tool}` refused `{absolute}`: {}",
                out.output
            );
        }

        // The traversal actually landed outside, so the assertions above are
        // not passing because the resolver silently no-op'd.
        let escaped_abs = dir.path().parent().unwrap().join("escaped.txt");
        assert_eq!(
            std::fs::read_to_string(&escaped_abs).unwrap(),
            "edited\n",
            "the ../ traversal did not escape"
        );
        std::fs::remove_file(&escaped_abs).ok();

        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "ours\n");
    }

    /// `write` creates the parents, and it stages and renames — so the only
    /// thing left in the directory afterwards is the file itself. A reader
    /// arriving mid-write sees the old file or the new one, never a truncated
    /// one (#114).
    #[tokio::test]
    async fn a_write_creates_parents_and_leaves_no_staged_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));

        let out = registry
            .execute_tool(&file_call(
                "write",
                serde_json::json!({"path": "deep/er/a.txt", "content": "hello\n"}),
            ))
            .await;
        assert!(!out.is_error, "{}", out.output);

        let home = dir.path().join("deep").join("er");
        assert_eq!(
            std::fs::read_to_string(home.join("a.txt")).unwrap(),
            "hello\n"
        );
        let left: Vec<String> = std::fs::read_dir(&home)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(left, vec!["a.txt"], "a staged file was left behind");
    }

    /// The rename itself, which is the part that closes the window: the
    /// destination is a different file afterwards, so no reader ever held a
    /// descriptor on a truncated one. Writing in place keeps the same inode
    /// and passes every other assertion here (#114).
    #[cfg(unix)]
    #[test]
    fn a_write_replaces_the_file_by_rename_rather_than_truncating_it() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "old\n").unwrap();
        let before = std::fs::metadata(&path).unwrap().ino();

        write_atomically(&path, "new\n").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        assert_ne!(
            before,
            std::fs::metadata(&path).unwrap().ino(),
            "the file was truncated in place instead of staged and renamed"
        );
    }

    /// The bounded cut in `read`, on the same defect the shell one was fixed
    /// for: `&content[..OUTPUT_LIMIT]` panics when a multi-byte character
    /// straddles the limit, and it takes the whole agent process with it.
    #[tokio::test]
    async fn reading_a_file_with_a_multibyte_character_on_the_limit_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
        let mut body = "a".repeat(OUTPUT_LIMIT - 1);
        body.push_str("€€€");
        assert!(
            !body.is_char_boundary(OUTPUT_LIMIT),
            "the probe must straddle"
        );
        std::fs::write(dir.path().join("big.txt"), &body).unwrap();

        let out = registry
            .execute_tool(&file_call("read", serde_json::json!({"path": "big.txt"})))
            .await;

        assert!(!out.is_error, "{}", out.output);
        assert!(out.output.contains("[Output truncated"), "it was not cut");
        assert!(
            out.output.starts_with(&body[..OUTPUT_LIMIT - 1]),
            "the cut did not step back to the boundary"
        );
    }

    // ───────────────────────────────────── stripping terminal escapes from output

    #[test]
    fn sgr_color_sequences_are_removed() {
        // A test runner styling `FAIL` red, bold, then resetting.
        let styled = "\u{1b}[1;31mFAIL\u{1b}[0m node";
        assert_eq!(strip_terminal_escapes(styled), "FAIL node");
    }

    #[test]
    fn cursor_movement_and_erase_sequences_are_removed() {
        // Save, rewrite, erase, restore — the shape of a progress bar.
        let input = "\u{1b}[s\u{1b}[2K\u{1b}[u\u{1b}[1A\u{1b}[G\u{1b}[Jtext";
        assert_eq!(strip_terminal_escapes(input), "text");
    }

    #[test]
    fn osc_sequences_are_removed_through_either_terminator() {
        // BEL-terminated title, ST-terminated hyperlink, with payload intact.
        let input = "\u{1b}]0;title\u{7}a\u{1b}]8;;https://example.com\u{1b}\\b";
        assert_eq!(strip_terminal_escapes(input), "ab");
    }

    #[test]
    fn two_byte_escape_sequences_are_removed() {
        // Charset selection and an `ESC @`..=`ESC _` form.
        let input = "\u{1b}(Ba\u{1b}Mb";
        assert_eq!(strip_terminal_escapes(input), "ab");
    }

    #[test]
    fn carriage_return_rewrites_are_removed() {
        // A spinner: three writes, one line. Keeping the `\r` would make the
        // terminal replay the rewrite against cells the TUI did not draw.
        // The stripped form keeps each write; the last before the newline is
        // the one a terminal would have left showing.
        assert_eq!(
            strip_terminal_escapes("10%\r50%\r100%\ndone"),
            "10%50%100%\ndone"
        );
    }

    #[test]
    fn plain_text_and_newlines_pass_through_untouched() {
        let plain = "Test Files  2 failed | 2 passed (4)\nTests  72 passed (72)\n";
        assert_eq!(strip_terminal_escapes(plain), plain);
    }

    #[test]
    fn an_unterminated_sequence_at_end_of_buffer_is_dropped_without_panicking() {
        assert_eq!(strip_terminal_escapes("ok\u{1b}[31"), "ok");
        assert_eq!(strip_terminal_escapes("ok\u{1b}]0;title"), "ok");
        assert_eq!(strip_terminal_escapes("ok\u{1b}"), "ok");
    }

    #[test]
    fn an_empty_string_stays_empty() {
        assert_eq!(strip_terminal_escapes(""), "");
    }

    #[test]
    fn multibyte_characters_survive_stripping() {
        // The stripper walks bytes, and this only works because 0x1b cannot
        // appear inside a multi-byte UTF-8 sequence — every continuation byte
        // is >= 0x80. So an ESC byte is always a real escape introducer, and
        // non-ASCII text passes through a byte at a time, intact.
        let input = "€ FAIL \u{1b}[31m\u{1e00}가\u{1b}[0m ✓";
        assert_eq!(strip_terminal_escapes(input), "€ FAIL \u{1e00}가 ✓");
    }

    // ─────────────────────────── the delegate tool's external-agent path
    //
    // These are the `acp` tool's tests, re-targeted at the `agent` parameter
    // of `delegate` — the capability was folded into delegate (#228) and the
    // refusals were kept verbatim, so the tests that pinned them moved with
    // it and read one tool's one behaviour.

    fn acp_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            command: "definitely-not-a-real-binary-xyz".to_string(),
            args: vec!["acp".to_string()],
        }
    }

    fn gate_with(agents: Vec<Agent>) -> (HarnessToolRegistry, Arc<AtomicBool>) {
        let spent = Arc::new(AtomicBool::new(false));
        let registry = HarnessToolRegistry::with_delegation(
            Some(std::env::temp_dir()),
            DelegationGate {
                lane: "test".to_string(),
                user_token: None,
                api_base: None,
                max_count: 2,
                child: Default::default(),
                acp_agents: agents,
                acp_spent: Arc::clone(&spent),
            },
        );
        (registry, spent)
    }

    async fn run_delegate(
        registry: &HarnessToolRegistry,
        arguments: serde_json::Value,
    ) -> (String, bool) {
        let out = registry
            .execute_tool(&ToolCall {
                id: "1".to_string(),
                name: "delegate".to_string(),
                arguments,
            })
            .await;
        (out.output, out.is_error)
    }

    /// The declaration enumerates the installed agents, and a machine with
    /// none carries no external-agent language at all. The alternative is a
    /// capability claimed where it does not exist.
    #[test]
    fn the_declaration_names_the_installed_external_agents_and_only_those() {
        let (registry, _spent) = gate_with(vec![acp_agent("devin"), acp_agent("grok-build")]);
        let tool = registry
            .list_tools()
            .into_iter()
            .find(|t| t.name == "delegate")
            .expect("declared");
        assert!(tool.description.contains("`devin`"), "{}", tool.description);
        assert!(
            tool.description.contains("grok-build"),
            "{}",
            tool.description
        );
        assert!(
            tool.description.contains("own bill"),
            "{}",
            tool.description
        );
        for builtin in ["coder-mini", "explore", "coder"] {
            assert!(tool.description.contains(builtin), "{}", tool.description);
        }
        let properties = tool.parameters["properties"]
            .as_object()
            .expect("delegate properties");
        for parameter in ["prompt", "count", "agent", "description", "tools", "mode"] {
            assert!(properties.contains_key(parameter), "missing `{parameter}`");
        }

        let (empty, _spent) = gate_with(Vec::new());
        let tool = empty
            .list_tools()
            .into_iter()
            .find(|t| t.name == "delegate")
            .expect("declared");
        assert!(
            !tool
                .description
                .to_lowercase()
                .contains("agent client protocol"),
            "no external-agent language without an installed agent: {}",
            tool.description
        );
    }

    #[tokio::test]
    async fn an_unknown_agent_is_refused_by_name_with_the_installed_list() {
        let (registry, spent) = gate_with(vec![acp_agent("devin")]);
        let (output, is_error) = run_delegate(
            &registry,
            serde_json::json!({"agent": "nobody", "prompt": "do it"}),
        )
        .await;
        assert!(is_error);
        assert!(output.contains("nobody"), "{output}");
        assert!(output.contains("devin"), "{output}");
        for builtin in ["coder-mini", "explore", "coder"] {
            assert!(output.contains(builtin), "{output}");
        }
        assert!(
            !spent.load(Ordering::SeqCst),
            "an unknown name consumed the ACP allowance"
        );
    }

    #[tokio::test]
    async fn old_callers_without_agent_keep_the_fan_out() {
        let (registry, spent) = gate_with(Vec::new());
        let (output, is_error) = run_delegate(
            &registry,
            serde_json::json!({"prompt": "do it", "count": 2}),
        )
        .await;
        assert!(!is_error, "{output}");
        assert!(output.starts_with("No children were started:"), "{output}");
        assert!(output.contains("there is no `test` lane"), "{output}");
        assert!(!spent.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_builtin_agent_does_not_spend_the_acp_allowance() {
        let (registry, spent) = gate_with(vec![acp_agent("devin")]);
        let (output, is_error) = run_delegate(
            &registry,
            serde_json::json!({
                "agent": "coder-mini",
                "prompt": "inspect it",
                "tools": "not-a-pool"
            }),
        )
        .await;
        assert!(is_error, "{output}");
        assert!(output.contains("not-a-pool"), "{output}");
        assert!(!spent.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_mode_this_build_does_not_know_is_refused_rather_than_dropped() {
        let (registry, _spent) = gate_with(vec![acp_agent("devin")]);
        let (output, is_error) = run_delegate(
            &registry,
            serde_json::json!({"agent": "devin", "prompt": "look", "mode": "whatever"}),
        )
        .await;
        assert!(is_error);
        assert!(output.contains("read-only"), "{output}");
    }

    /// An agent that is not on PATH is said to be missing. It used to be
    /// reported to the model as the tool's answer.
    #[tokio::test]
    async fn an_agent_that_will_not_start_is_an_error_and_says_why() {
        let (registry, _spent) = gate_with(vec![acp_agent("devin")]);
        let (output, is_error) = run_delegate(
            &registry,
            serde_json::json!({"agent": "devin", "prompt": "do it"}),
        )
        .await;
        assert!(is_error, "{output}");
        assert!(output.contains("could not be started"), "{output}");
    }

    /// Twenty-four consecutive delegations for one message is what this
    /// exists to stop; the mechanism it replaced was `tool_choice: none`
    /// (commit `afea5551fa`). Scoped to external agents: a plain fan-out
    /// spends the session's own grant, an `agent` call spends somebody's bill.
    #[tokio::test]
    async fn only_one_external_agent_is_handed_work_per_turn() {
        let (registry, spent) = gate_with(vec![acp_agent("devin")]);

        // The first call is let through — it fails only because the stand-in
        // binary does not exist, which is a different refusal.
        let (first, _) = run_delegate(
            &registry,
            serde_json::json!({"agent": "devin", "prompt": "do it"}),
        )
        .await;
        assert!(first.contains("could not be started"), "{first}");

        let (second, is_error) = run_delegate(
            &registry,
            serde_json::json!({"agent": "devin", "prompt": "again"}),
        )
        .await;
        assert!(is_error, "{second}");
        assert!(second.contains("already handed work"), "{second}");
        assert!(second.contains("one is the limit"), "{second}");

        // A new turn clears it.
        spent.store(false, Ordering::SeqCst);
        let (third, _) = run_delegate(
            &registry,
            serde_json::json!({"agent": "devin", "prompt": "next turn"}),
        )
        .await;
        assert!(third.contains("could not be started"), "{third}");
    }

    /// A malformed second call must not be the one that gets through: the
    /// flag is claimed before the arguments are read.
    #[tokio::test]
    async fn a_refused_second_call_is_refused_before_its_arguments_are_read() {
        let (registry, spent) = gate_with(vec![acp_agent("devin")]);
        spent.store(true, Ordering::SeqCst);
        let (output, is_error) =
            run_delegate(&registry, serde_json::json!({"agent": "devin"})).await;
        assert!(is_error);
        assert!(output.contains("already handed work"), "{output}");
    }

    #[test]
    fn a_plan_upsell_is_not_a_completed_task() {
        assert!(is_refusal("Please upgrade your plan to continue."));
        assert!(!is_refusal("Done. Edited src/main.rs."));
    }
}
