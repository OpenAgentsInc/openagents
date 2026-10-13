//! Coder's own tools in the `claude` mode (#11437): what Coder can hand
//! work to, offered to Claude Code as one in-process tool server.
//!
//! Claude Code brings its own shell, file, web, plan and memory tools, so
//! those stay out. What it gets from Coder is delegation: background agents
//! on Coder's engines in their own worktrees (`agent`, `agent_list`,
//! `agent_message`, `agent_stop`), Solver for an issue to a pull request,
//! the Solver swarm, cloud jobs on GCE or Boat, local ACP subagents, the
//! bundled coding loop, the agent channel, and the GitHub and OpenAgents
//! tools, each only when its plugin is on, exactly as a live chat offers
//! them ([`ExecutionSettings::defs`]). A call runs through the same
//! [`ExecutionSettings::execute`] a live chat uses, and its progress shows
//! as a delegation in the chat.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::plugin_tools::ExecutionSettings;

/// The server's name: a tool `agent` is `mcp__coder__agent` to the engine.
pub const SERVER: &str = crate::solver_plugin::MCP_SERVER;

/// A delegation can run for hours; the tool call must not time out first.
const TIMEOUT_MS: u64 = 6 * 60 * 60 * 1000;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The rule the model gets while the Solver plugin is on: an issue the
/// person asks to have done goes to Solver, every time.
pub const SOLVER_RULE: &str = "When the person asks you to do, solve, fix, implement, build, or work on a GitHub issue (a number like #123, owner/repo#123, or an issue link), call the `solver` tool with that issue right away instead of doing the work yourself in this turn. Turn on `no_pr` only when they ask for no pull request. Do not read the issue, plan, or search the code first: Solver does all of that. Afterwards, report its outcome, pull request link, and cost in a sentence or two. When they ask for several issues at once, use `solver_swarm` if it is offered.\n";

/// The Coder tools one claude chat offers, from the chat's plugin
/// settings. Two are equal when they offer the same tools, so a chat's
/// process restarts only when that changes.
#[derive(Clone, Default)]
pub struct Tools {
    execution: Option<ExecutionSettings>,
    names: Vec<String>,
}

impl Tools {
    /// The tools `execution` turns on, less Claude Code's own.
    #[must_use]
    pub fn new(execution: ExecutionSettings) -> Self {
        let names = names(&execution);
        Self {
            execution: Some(execution),
            names,
        }
    }

    /// The tool names offered, as Coder names them.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

impl PartialEq for Tools {
    fn eq(&self, other: &Self) -> bool {
        self.names == other.names
    }
}

impl std::fmt::Debug for Tools {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Tools").field(&self.names).finish()
    }
}

/// `execution` as the claude mode offers it: without the tools Claude Code
/// has itself.
#[must_use]
pub fn delegation_only(mut execution: ExecutionSettings) -> ExecutionSettings {
    execution.shell = false;
    execution.memory = None;
    execution
}

/// The tools a claude turn gets from `execution`, by name.
#[must_use]
pub fn names(execution: &ExecutionSettings) -> Vec<String> {
    delegation_only(execution.clone())
        .defs()
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_owned))
        .collect()
}

/// What the model is told about Coder's tools, or nothing when none is on.
#[must_use]
pub fn instructions(tools: &Tools) -> String {
    let Some(execution) = tools.execution.as_ref().filter(|_| !tools.names.is_empty()) else {
        return String::new();
    };
    let execution = delegation_only(execution.clone());
    let solver = if tools
        .names
        .iter()
        .any(|name| name == crate::solver_plugin::TOOL)
    {
        SOLVER_RULE
    } else {
        ""
    };
    format!(
        "{solver}Coder's own tools are under the `{SERVER}` server. Use them to hand work to Coder rather than doing all of it in this turn: independent tasks to background agents (`agent`, several at once, each in its own worktree), an issue that should end in a pull request to `solver`, and cloud work to the cloud tools. Your own Agent tool stays for quick research inside this turn.\n{}",
        execution.instructions()
    )
}

/// The name a person reads for the engine's tool `name`: Coder's own tools
/// by their plain names ("GitHub", "Agent list", "Solver"), another
/// server's as "server: tool name", and every other tool as it is.
#[must_use]
pub fn display_name(name: &str) -> String {
    let Some((server, tool)) = name
        .strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__"))
    else {
        return name.to_owned();
    };
    let plain = match tool {
        "github" => "GitHub".to_owned(),
        "openagents_cli" => "OpenAgents CLI".to_owned(),
        "acp_subagent" => "Local agent".to_owned(),
        "gce_delegate" => "GCE job".to_owned(),
        "boat_delegate" => "Boat job".to_owned(),
        _ => {
            let spaced = tool.replace(['_', '-'], " ");
            let mut chars = spaced.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        }
    };
    if server == SERVER {
        plain
    } else {
        format!("{}: {}", server.replace('_', " "), plain.to_lowercase())
    }
}

/// Whether `name` is one of this server's tools as the engine names it.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    name.starts_with(&format!("mcp__{SERVER}__"))
}

/// The name a delegation's closing step carries, for a tool that runs work
/// of its own and shows as a delegation until it ends; `None` for a quick
/// tool, which shows as an ordinary tool call. A background agent is a
/// quick call: `agent` only starts it, and the agent list shows it.
fn delegates(name: &str) -> Option<&'static str> {
    match name {
        "microcoder" => Some("microcoder"),
        "acp_subagent" => Some("acp_subagent"),
        "boat_delegate" | "gce_delegate" => Some("remote_delegate"),
        crate::solver_plugin::TOOL => Some(crate::solver_plugin::TOOL),
        _ => None,
    }
}

/// Whether the engine's call `name` is drawn by its own delegation, so the
/// turn leaves it out of the transcript.
#[must_use]
pub fn draws_itself(name: &str) -> bool {
    name.strip_prefix(&format!("mcp__{SERVER}__"))
        .is_some_and(|tool| delegates(tool).is_some())
}

/// The in-process tool server for `execution`, or `None` when no tool is
/// on. Each call's progress reaches `events` as a delegation, and setting
/// `stop` stops the calls still running.
#[must_use]
pub fn server(
    tools: &Tools,
    events: tokio::sync::mpsc::UnboundedSender<RuntimeEvent>,
    stop: Arc<AtomicBool>,
) -> Option<claude_agent_sdk::SdkMcpServer> {
    tool_server(tools, events, stop).map(|server| server.in_process())
}

/// Coder's tools as the one tool server the SDK offers every harness that
/// takes MCP (#11501): in process for Claude Code ([`server`]), over stdio
/// for the others ([`stdio_main`]).
#[must_use]
pub fn tool_server(
    tools: &Tools,
    events: tokio::sync::mpsc::UnboundedSender<RuntimeEvent>,
    stop: Arc<AtomicBool>,
) -> Option<coder_sdk::ToolServer> {
    let execution = delegation_only(tools.execution.clone()?);
    let tools = execution.defs();
    // Coder's tool calls hold values that stay on one thread, so each call
    // runs on a thread of its own and the server waits for its answer.
    let execution = Arc::new(std::sync::Mutex::new(execution));
    if tools.is_empty() {
        return None;
    }
    let mut server = coder_sdk::ToolServer::new(SERVER, "1.0.0").timeout_ms(TIMEOUT_MS);
    if tools
        .iter()
        .any(|tool| tool["function"]["name"] == crate::solver_plugin::TOOL)
    {
        server = server.instructions(SOLVER_RULE);
    }
    for tool in tools {
        let function = &tool["function"];
        let Some(name) = function["name"].as_str().map(str::to_owned) else {
            continue;
        };
        let description = function["description"].as_str().unwrap_or_default();
        let schema = function
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| json!({"type":"object"}));
        let execution = execution.clone();
        let events = events.clone();
        let stop = stop.clone();
        let tool_name = name.clone();
        // Solver and the agent tools are always loaded, never behind tool
        // search, so the model reaches for them without looking first.
        let always = matches!(
            name.as_str(),
            crate::solver_plugin::TOOL | "solver_swarm" | "agent" | "agent_list"
        );
        let tool = coder_sdk::Tool::new(&name, description, schema, move |input: Value| {
            let execution = execution.clone();
            let events = events.clone();
            let stop = stop.clone();
            let name = tool_name.clone();
            async move {
                let execution = execution
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                let (answer, result) = tokio::sync::oneshot::channel();
                std::thread::spawn(move || {
                    let outcome = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => {
                            runtime.block_on(call(&execution, &name, input, &events, &stop))
                        }
                        Err(error) => Err(format!("Coder's tool could not start: {error}")),
                    };
                    let _ = answer.send(outcome);
                });
                let result = result
                    .await
                    .unwrap_or_else(|_| Err("Coder's tool stopped without an answer.".into()));
                Ok(match result {
                    Ok(value) => claude_agent_sdk::ToolResult::text(value.to_string()),
                    Err(error) => claude_agent_sdk::ToolResult::error(error),
                })
            }
        })
        .always_load(always);
        server = server.add(tool);
    }
    Some(server)
}

/// The subcommand that serves Coder's tools over stdio: `coder
/// coder-tools-mcp [--decide] [--cwd DIR]`.
pub const STDIO_COMMAND: &str = "coder-tools-mcp";

/// `coder coder-tools-mcp`: the person's Coder tools (the plugins they
/// turned on) over MCP stdio for a harness that takes MCP, gated by the
/// floors, with `decide` when `--decide` admits sending to the Decisions
/// API. Returns the exit code.
pub fn stdio_main(args: &[String]) -> i32 {
    let mut decide = false;
    let mut cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--decide" => decide = true,
            "--cwd" => {
                if let Some(dir) = rest.next() {
                    cwd = std::path::PathBuf::from(dir);
                }
            }
            other => {
                eprintln!("Unknown option {other}. Use --decide or --cwd DIR.");
                return 2;
            }
        }
    }
    let mut app = crate::App::default();
    app.cwd = Some(cwd.clone());
    if let Some(store) = crate::home::data_dir()
        && let Err(error) = app.load_plugin_settings(crate::plugin_store::Store::under(&store))
    {
        eprintln!("{error}");
    }
    let admission = if decide {
        coder_sdk::Admission::none().admitting(coder_sdk::decide::disclosure())
    } else {
        coder_sdk::Admission::none()
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Coder's tools could not start: {error}");
            return 1;
        }
    };
    let execution = app.agent_execution();
    runtime.block_on(async move {
        let (events, _drawn) = tokio::sync::mpsc::unbounded_channel();
        let stop = Arc::new(AtomicBool::new(false));
        let mut server = tool_server(&Tools::new(execution), events, stop)
            .unwrap_or_else(|| coder_sdk::ToolServer::new(SERVER, "1.0.0"));
        match coder_sdk::decide::tool(&admission, Arc::new(coder_sdk::decide::Api)) {
            Ok(tool) => server = server.add(tool),
            Err(refused) if decide => eprintln!("{}", refused.message),
            Err(_) => {}
        }
        let mut registry = coder_sdk::Registry::new();
        if let Err(error) = registry.register(crate::sdk_hooks::held_floors(cwd)) {
            eprintln!("{error}");
            return 1;
        }
        let runner = coder_sdk::Runner::new(registry, coder_sdk::Budget::default());
        let server = server.gated(Arc::new(std::sync::Mutex::new(runner)));
        match coder_sdk::serve_stdio(&server).await {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        }
    })
}

/// Runs one call through the live chat's tool path, drawing it as a
/// delegation named for the tool.
async fn call(
    execution: &ExecutionSettings,
    name: &str,
    input: Value,
    events: &tokio::sync::mpsc::UnboundedSender<RuntimeEvent>,
    stop: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let Some(closing) = delegates(name) else {
        // A quick call shows as the engine's own tool call; its progress,
        // if any, is not drawn twice.
        return execution
            .execute(name, input, None, stop, &mut |_| {})
            .await;
    };
    let id = format!(
        "coder-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let task = task(name, &input);
    let label = delegate_label(execution, name, &input);
    let mut emit = |event: RuntimeEvent| {
        // A tool that draws its own delegation (Solver) keeps it.
        let event = match event {
            event @ RuntimeEvent::Delegation { .. } => event,
            event => RuntimeEvent::Delegation {
                id: id.clone(),
                name: label.clone(),
                task: task.clone(),
                event: Box::new(event),
            },
        };
        let _ = events.send(event);
    };
    emit(RuntimeEvent::Tool {
        name: closing.to_owned(),
        input: input.clone(),
        output: Value::Null,
        running: true,
    });
    let result = execution.execute(name, input, None, stop, &mut emit).await;
    emit(RuntimeEvent::Tool {
        name: closing.to_owned(),
        input: Value::Null,
        output: match &result {
            Ok(value) => value.clone(),
            Err(error) => json!({"error": error}),
        },
        running: false,
    });
    result
}

/// The name a delegation shows: the agent that runs it, never the tool
/// ("Codex", "Claude Code", "Microcoder"), with the model it was asked to
/// run in parentheses when the call names one. The rail adds the model the
/// agent reports once it does.
fn delegate_label(execution: &ExecutionSettings, name: &str, input: &Value) -> String {
    let agent = match name {
        "acp_subagent" => {
            let id = input["agent"].as_str().unwrap_or_default();
            execution
                .agents
                .iter()
                .find(|agent| agent.id == id)
                .map_or_else(|| id.to_owned(), |agent| agent.name.clone())
        }
        "microcoder" => "Microcoder".to_owned(),
        crate::solver_plugin::TOOL => crate::solver_plugin::NAME.to_owned(),
        _ => display_name(&format!("mcp__{SERVER}__{name}")),
    };
    let agent = if agent.is_empty() {
        display_name(&format!("mcp__{SERVER}__{name}"))
    } else {
        agent
    };
    match input["model"].as_str().filter(|model| !model.is_empty()) {
        Some(model) => format!("{agent} ({model})"),
        None => agent,
    }
}

/// The line a delegation shows for a call: its task, issue, or message when
/// it names one, else the tool's name.
fn task(name: &str, input: &Value) -> String {
    ["task", "issue", "message", "agent"]
        .iter()
        .find_map(|field| match &input[*field] {
            Value::String(text) if !text.is_empty() => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_codes_own_tools_stay_out() {
        let mut execution = crate::App::default().agent_execution();
        execution.shell = true;
        let offered = names(&execution);
        for native in [
            "Run",
            "Read",
            "Write",
            "Edit",
            "web_fetch",
            "web_search",
            "plan",
        ] {
            assert!(!offered.iter().any(|name| name == native), "{native}");
        }
    }

    #[test]
    fn no_tool_on_means_no_server() {
        let (sender, _) = tokio::sync::mpsc::unbounded_channel();
        let execution = crate::App::default().agent_execution();
        let tools = Tools::new(execution);
        if tools.names().is_empty() {
            assert!(server(&tools, sender, Arc::new(AtomicBool::new(false))).is_none());
            assert!(instructions(&tools).is_empty());
        }
        assert!(
            server(
                &Tools::default(),
                tokio::sync::mpsc::unbounded_channel().0,
                Arc::new(AtomicBool::new(false))
            )
            .is_none()
        );
    }

    #[test]
    fn the_servers_tools_are_known_by_their_prefix() {
        assert!(is_tool("mcp__coder__agent"));
        assert!(is_tool("mcp__coder__solver"));
        assert!(!is_tool("Bash"));
        assert_eq!(display_name("mcp__coder__github"), "GitHub");
        assert_eq!(display_name("mcp__coder__agent_list"), "Agent list");
        assert_eq!(
            display_name("mcp__stripe__create_customer"),
            "stripe: create customer"
        );
        assert_eq!(display_name("Bash"), "Bash");
        // `agent` only starts a background agent: an ordinary tool call.
        assert!(!draws_itself("mcp__coder__agent"));
        assert!(draws_itself("mcp__coder__solver"));
        assert!(draws_itself("mcp__coder__gce_delegate"));
    }

    #[test]
    fn a_delegation_is_named_for_its_agent_not_the_tool() {
        let mut execution = crate::App::default().agent_execution();
        execution.agents = vec![crate::bundled_runtime::AcpAgent {
            id: "codex".into(),
            name: "Codex".into(),
            ..serde_json::from_value(json!({"id":"x","name":"x","program":"x"})).unwrap()
        }];
        assert_eq!(
            delegate_label(&execution, "acp_subagent", &json!({"agent":"codex"})),
            "Codex"
        );
        assert_eq!(
            delegate_label(
                &execution,
                "acp_subagent",
                &json!({"agent":"codex","model":"gpt-6.1-sol"})
            ),
            "Codex (gpt-6.1-sol)"
        );
        assert_eq!(
            delegate_label(&execution, "microcoder", &json!({})),
            "Microcoder"
        );
    }

    #[test]
    fn a_delegation_names_its_task() {
        assert_eq!(task("agent", &json!({"task":"fix login"})), "fix login");
        assert_eq!(task("solver", &json!({"issue": 42})), "42");
        assert_eq!(task("agent_list", &json!({})), "agent_list");
    }
}
