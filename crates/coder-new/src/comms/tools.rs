//! How each kind of agent reaches the channel.
//!
//! - The live model: the `channel` tool ([`tool_definition`], [`execute`]),
//!   speaking as the member `Coder`.
//! - Microclaude: the same tool, and the delegation tools, on its
//!   in-process tool server ([`mcp_server`]).
//! - Engines that run as their own process (Codex, OpenCode, Cursor, Devin,
//!   Claude in print mode, Microcoder): a preamble on their task naming the
//!   channel, their member name and the `coder comms` command
//!   ([`Member::preamble`]).
//! - Solvers and the swarm: [`super::Member`] directly.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicBool};

use serde_json::{Value, json};

use super::{Channel, Kind, Member, now_ms};
use crate::bundled_runtime::RuntimeEvent;

/// The tool the live model and microclaude call.
pub const TOOL: &str = "channel";

/// The member name the main Coder speaks as.
pub const LEAD: &str = "Coder";

const DESCRIPTION: &str = "The session's agent channel, shared by every agent you delegate to. `read` returns what they posted: messages, the files each means to change, overlaps between them, how those were settled, and leases on shared things such as the build folder or a port. `post` sends `text` to everyone, or to one member with `to`. `members` lists them and how each ended; `leases` lists what is held and until when.";

/// The tool's definition, in the chat-completions function shape.
#[must_use]
pub fn tool_definition() -> Value {
    json!({"type":"function","function":{
        "name": TOOL,
        "description": DESCRIPTION,
        "parameters": schema(),
    }})
}

/// The tool's input schema.
#[must_use]
pub fn schema() -> Value {
    json!({
        "type":"object",
        "properties":{
            "action":{"type":"string","enum":["read","post","members","leases"]},
            "text":{"type":"string","description":"What to post."},
            "to":{"type":"string","description":"One member to post to; everyone when left out."}
        },
        "required":["action"],
        "additionalProperties":false
    })
}

/// What the model is told about the channel.
#[must_use]
pub const fn instructions() -> &'static str {
    "Agents you delegate to share a channel in this session: they say which files they will change, are told when they overlap, agree who goes first, and lease shared things (the build folder, a deploy slot, a migration number, a port, a build host). Use the channel tool to read it when several delegations run, and to pass a note to one of them.\n"
}

/// Runs a `channel` call as `LEAD`.
///
/// # Errors
/// The arguments are not the schema's, or the channel can't be used.
pub fn execute(channel: &Channel, arguments: &Value) -> Result<Value, String> {
    const USAGE: &str =
        "channel takes action (read, post, members or leases), and text and to for post.";
    let object = arguments.as_object().ok_or(USAGE)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "action" | "text" | "to"))
    {
        return Err(USAGE.into());
    }
    match object.get("action").and_then(Value::as_str).ok_or(USAGE)? {
        "read" => {
            let (records, _) = channel.records(0);
            let shown: Vec<Value> = records
                .iter()
                .filter(|record| record.kind != Kind::Join)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .take(60)
                .rev()
                .map(|record| json!({"from": record.from, "to": record.to, "kind": record.kind.as_str(), "text": record.text}))
                .collect();
            Ok(json!({"records": shown}))
        }
        "post" => {
            let text = object.get("text").and_then(Value::as_str).ok_or(USAGE)?;
            let to = object.get("to").and_then(Value::as_str);
            let record = channel.member(LEAD)?.post(to, text)?;
            Ok(json!({"posted": record.text, "to": record.to}))
        }
        "members" => {
            let state = channel.read_state()?;
            Ok(
                json!({"members": state.members.iter().map(|(name, member)| json!({
                "name": name, "kind": member.kind, "finished": member.finished,
                "intent": state.intents.get(name).map(super::Intent::sentence),
            })).collect::<Vec<_>>()}),
            )
        }
        "leases" => {
            let state = channel.read_state()?;
            let now = now_ms();
            Ok(
                json!({"leases": state.leases.values().filter(|lease| lease.expires_ms > now).map(|lease| json!({
                "resource": lease.resource, "holder": lease.holder,
                "seconds_left": (lease.expires_ms - now) / 1000,
            })).collect::<Vec<_>>()}),
            )
        }
        _ => Err(USAGE.into()),
    }
}

static SESSION: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Names the session (`chat --session ID`); its channel is then
/// `comms/<ID>` under Coder's store.
pub fn set_session(name: &str) {
    *SESSION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(name.to_owned());
}

/// This process's session channel: the parent's when [`super::CHANNEL_ENV`]
/// is set, else `comms/<session>` under `root` (Coder's store). The
/// folder is made only when something is first written.
#[must_use]
pub fn session_channel(root: Option<&Path>) -> Option<Channel> {
    if cfg!(test) {
        // Tests never write under this computer's home.
        let dir = std::env::temp_dir()
            .join("coder-comms-tests")
            .join(format!("{}", std::process::id()));
        return Some(Channel::at(dir));
    }
    if let Some(dir) = std::env::var_os(super::CHANNEL_ENV).filter(|dir| !dir.is_empty()) {
        return Some(Channel::at(PathBuf::from(dir)));
    }
    let root = root
        .map(Path::to_path_buf)
        .or_else(|| model_access::store::openagents_dir().map(|dir| dir.join("coder-new")))?;
    let name = SESSION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_or_insert_with(|| format!("{}-{}", now_ms(), std::process::id()))
        .clone();
    Channel::for_session(&root, &name).ok()
}

/// The member name a delegation joins as.
#[must_use]
pub fn delegation_label(
    tool: &str,
    arguments: &Value,
    agents: &[crate::bundled_runtime::AcpAgent],
) -> Option<(String, String)> {
    match tool {
        "acp_subagent" => {
            let id = arguments["agent"].as_str()?;
            let name = agents
                .iter()
                .find(|agent| agent.id == id)
                .map_or_else(|| id.to_owned(), |agent| agent.name.clone());
            Some((id.to_owned(), name))
        }
        "microcoder" => Some(("microcoder".into(), "Microcoder".into())),
        crate::solver_plugin::TOOL => {
            let (_, issue, _) = crate::solver_plugin::arguments(arguments).ok()?;
            Some(("solver".into(), format!("Solver #{issue}")))
        }
        _ => None,
    }
}

impl Channel {
    /// The channel in `dir`, made when first written.
    #[must_use]
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }
}

/// The command a process runs to reach the channel: this binary's `comms`
/// (`openagents coder comms` when this is the OpenAgents CLI).
#[must_use]
pub fn command_prefix() -> String {
    let exe = std::env::current_exe().ok();
    let word = |path: &Path| crate::bundled_runtime::shell_word(&path.to_string_lossy());
    match exe {
        Some(exe)
            if exe
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openagents")) =>
        {
            format!("{} coder comms", word(&exe))
        }
        Some(exe)
            if exe
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("coder")) =>
        {
            format!("{} comms", word(&exe))
        }
        _ => crate::bundled_runtime::cli_binary().map_or_else(
            || "coder comms".to_owned(),
            |cli| format!("{} coder comms", word(&cli)),
        ),
    }
}

impl Member {
    /// What an engine running as its own process is told before its task:
    /// who it is, that others share the session, and the commands.
    #[must_use]
    pub fn preamble(&self) -> String {
        let cmd = format!(
            "{} --channel {} --as {}",
            command_prefix(),
            crate::bundled_runtime::shell_word(&self.channel().dir().to_string_lossy()),
            crate::bundled_runtime::shell_word(self.name()),
        );
        format!(
            "You are {name}, one of several agents working in this session. They share a channel; use it when your work may touch theirs. Run these as shell commands:\n\
- Before you edit, say what you will change: `{cmd} intent --file PATH [--file PATH] [--crate NAME] [--resource NAME]`. It tells you whom you overlap with.\n\
- Read what the others posted, overlaps and agreements included: `{cmd} read`\n\
- Post to everyone, or to one agent: `{cmd} post [--to NAME] \"TEXT\"`\n\
- Settle an overlap: `{cmd} agree --with NAME --outcome first|split|merge [--first NAME] [--on PATH] --note \"TEXT\"`\n\
- Before you use a shared thing, lease it, and release it when done: `{cmd} lease take build-folder|build-host|deploy-slot|migration|port [NAME]`, `{cmd} lease renew RESOURCE`, `{cmd} lease release RESOURCE`. A held lease names its holder; ask them, wait, or do something else.\n\
If those commands are unavailable, append a JSON line such as {{\"from\":\"{name}\",\"text\":\"...\"}} to {log}.\n\nYour task:\n",
            name = self.name(),
            log = self.channel().log_path().display(),
        )
    }
}

/// The delegation tools and the channel on microclaude's in-process tool
/// server.
#[derive(Clone, Debug, Default)]
pub struct Delegates {
    pub solver: bool,
    pub swarm: bool,
    pub comms: Option<Channel>,
}

impl Delegates {
    #[must_use]
    pub fn any(&self) -> bool {
        self.solver || self.swarm || self.comms.is_some()
    }

    /// What the model is told about them.
    #[must_use]
    pub fn instructions(&self) -> String {
        let mut text = String::new();
        if self.solver {
            text.push_str(crate::solver_plugin::instructions());
        }
        if self.swarm {
            text.push_str(super::swarm::instructions());
        }
        if self.comms.is_some() {
            text.push_str(instructions());
        }
        text
    }
}

/// The tool server's name: its tools are `mcp__coder__<tool>` to the engine.
pub const MCP_SERVER: &str = "coder";

/// Whether `name` is one of the server's delegation tools as the engine
/// names it (each delegation shows as its own entry).
#[must_use]
pub fn is_delegation_mcp_tool(name: &str) -> bool {
    [crate::solver_plugin::TOOL, super::swarm::TOOL]
        .iter()
        .any(|tool| name == format!("mcp__{MCP_SERVER}__{tool}"))
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// microclaude's in-process tool server: the delegations that are on and
/// the channel. A delegation's progress reaches `events`; `stop` stops it.
#[must_use]
pub fn mcp_server(
    cwd: PathBuf,
    events: tokio::sync::mpsc::UnboundedSender<RuntimeEvent>,
    stop: Arc<AtomicBool>,
    delegates: &Delegates,
) -> claude_agent_sdk::SdkMcpServer {
    let mut server = claude_agent_sdk::SdkMcpServer::new(MCP_SERVER, "1.0.0")
        // A run takes many minutes; the tool call must not time out first.
        .timeout_ms(24 * 60 * 60 * 1000);
    for (enabled, tool) in [
        (delegates.solver, crate::solver_plugin::TOOL),
        (delegates.swarm, super::swarm::TOOL),
    ] {
        if !enabled {
            continue;
        }
        let (description, schema) = if tool == crate::solver_plugin::TOOL {
            let definition = crate::solver_plugin::tool_definition();
            (
                definition["function"]["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                crate::solver_plugin::schema(),
            )
        } else {
            let definition = super::swarm::tool_definition();
            (
                definition["function"]["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                super::swarm::schema(),
            )
        };
        let cwd = cwd.clone();
        let events = events.clone();
        let stop = stop.clone();
        let channel = delegates.comms.clone();
        server = server.tool(tool, description, schema, move |input: Value| {
            let cwd = cwd.clone();
            let events = events.clone();
            let stop = stop.clone();
            let channel = channel.clone();
            async move {
                let id = format!(
                    "{tool}-{}-{}",
                    std::process::id(),
                    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                );
                let (name, task) = if tool == crate::solver_plugin::TOOL {
                    (
                        crate::solver_plugin::NAME,
                        crate::solver_plugin::task(&input)
                            .unwrap_or_else(|| "Solve an issue.".into()),
                    )
                } else {
                    (
                        super::swarm::NAME,
                        super::swarm::task(&input)
                            .unwrap_or_else(|| "Solve several issues.".into()),
                    )
                };
                let mut emit = |event: RuntimeEvent| {
                    let _ = events.send(RuntimeEvent::Delegation {
                        id: id.clone(),
                        name: name.into(),
                        task: task.clone(),
                        event: Box::new(event),
                    });
                };
                emit(RuntimeEvent::Tool {
                    name: tool.into(),
                    input: input.clone(),
                    output: Value::Null,
                    running: true,
                });
                let result = if tool == crate::solver_plugin::TOOL {
                    let member = channel.as_ref().and_then(|channel| {
                        let (kind, label) = delegation_label(tool, &input, &[])?;
                        channel.join(&kind, &label).ok()
                    });
                    let result = match &member {
                        Some(member) => {
                            let shared = Shared::new(&mut emit);
                            let mut forward = |event: RuntimeEvent| shared.send(event);
                            attend(
                                member,
                                &shared,
                                crate::solver_plugin::execute_with(
                                    &input,
                                    &cwd,
                                    Some(member.clone()),
                                    &stop,
                                    &mut forward,
                                ),
                            )
                            .await
                        }
                        None => {
                            crate::solver_plugin::execute_with(&input, &cwd, None, &stop, &mut emit)
                                .await
                        }
                    };
                    if let Some(member) = &member {
                        let _ = member.finish(&finished_words(&result));
                    }
                    result
                } else {
                    super::swarm::execute(&input, &cwd, channel.as_ref(), &stop, &mut emit).await
                };
                let output = match &result {
                    Ok(value) => value.clone(),
                    Err(error) => json!({"error": error}),
                };
                emit(RuntimeEvent::Tool {
                    name: tool.into(),
                    input: Value::Null,
                    output,
                    running: false,
                });
                Ok(match result {
                    Ok(value) => claude_agent_sdk::ToolResult::text(value.to_string()),
                    Err(error) => claude_agent_sdk::ToolResult::error(error),
                })
            }
        });
    }
    if let Some(channel) = &delegates.comms {
        let channel = channel.clone();
        server = server.tool(TOOL, DESCRIPTION, schema(), move |input: Value| {
            let channel = channel.clone();
            async move {
                Ok(match execute(&channel, &input) {
                    Ok(value) => claude_agent_sdk::ToolResult::text(value.to_string()),
                    Err(error) => claude_agent_sdk::ToolResult::error(error),
                })
            }
        });
    }
    server
}

/// How a delegation ended, for its `finish` record.
#[must_use]
pub fn finished_words(result: &Result<Value, String>) -> String {
    match result {
        Ok(value) => value["outcome"]
            .as_str()
            .map(super::swarm::outcome_words)
            .unwrap_or_else(|| "done".into()),
        Err(error) if error.contains("stopped") || error.contains("canceled") => {
            "was stopped".into()
        }
        Err(_) => "failed".into(),
    }
}

/// A delegation's emit, shared between its work and the channel shown
/// beside it ([`attend`]).
pub struct Shared<'e>(Mutex<&'e mut (dyn FnMut(RuntimeEvent) + Send)>);

impl<'e> Shared<'e> {
    pub fn new(emit: &'e mut (dyn FnMut(RuntimeEvent) + Send)) -> Self {
        Self(Mutex::new(emit))
    }

    pub fn send(&self, event: RuntimeEvent) {
        let mut emit = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (*emit)(event);
    }
}

/// Runs `work`, a delegation sending its progress through `shared`, while
/// what the channel says to `member` is shown beside it in the same chat.
pub async fn attend<T>(
    member: &Member,
    shared: &Shared<'_>,
    work: impl std::future::Future<Output = T>,
) -> T {
    let mut cursor = member.channel().records(0).1;
    let work = std::pin::pin!(work);
    let mut work = work;
    let result = loop {
        tokio::select! {
            result = &mut work => break result,
            () = tokio::time::sleep(std::time::Duration::from_millis(700)) => {
                for record in member.inbox(&mut cursor) {
                    shared.send(super::view::event(&record));
                }
            }
        }
    };
    for record in member.inbox(&mut cursor) {
        shared.send(super::view::event(&record));
    }
    result
}
