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
    format!(
        "Coder's own tools are under the `{SERVER}` server. Use them to hand work to Coder rather than doing all of it in this turn: independent tasks to background agents (`agent`, several at once, each in its own worktree), an issue that should end in a pull request to `solver`, and cloud work to the cloud tools. Your own Agent tool stays for quick research inside this turn.\n{}",
        execution.instructions()
    )
}

/// Whether `name` is one of this server's tools as the engine names it.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    name.starts_with(&format!("mcp__{SERVER}__"))
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
    let execution = delegation_only(tools.execution.clone()?);
    let tools = execution.defs();
    // Coder's tool calls hold values that stay on one thread, so each call
    // runs on a thread of its own and the server waits for its answer.
    let execution = Arc::new(std::sync::Mutex::new(execution));
    if tools.is_empty() {
        return None;
    }
    let mut server = claude_agent_sdk::SdkMcpServer::new(SERVER, "1.0.0").timeout_ms(TIMEOUT_MS);
    for tool in tools {
        let function = &tool["function"];
        let Some(name) = function["name"].as_str().map(str::to_owned) else {
            continue;
        };
        let description = function["description"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let schema = function
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| json!({"type":"object"}));
        let execution = execution.clone();
        let events = events.clone();
        let stop = stop.clone();
        let tool_name = name.clone();
        server = server.tool(name, description, schema, move |input: Value| {
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
        });
    }
    Some(server)
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
    let id = format!(
        "coder-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let task = task(name, &input);
    let mut emit = |event: RuntimeEvent| {
        // A tool that draws its own delegation (Solver) keeps it.
        let event = match event {
            event @ RuntimeEvent::Delegation { .. } => event,
            event => RuntimeEvent::Delegation {
                id: id.clone(),
                name: name.to_owned(),
                task: task.clone(),
                event: Box::new(event),
            },
        };
        let _ = events.send(event);
    };
    emit(RuntimeEvent::Tool {
        name: name.to_owned(),
        input: input.clone(),
        output: Value::Null,
        running: true,
    });
    let result = execution.execute(name, input, None, stop, &mut emit).await;
    emit(RuntimeEvent::Tool {
        name: name.to_owned(),
        input: Value::Null,
        output: match &result {
            Ok(value) => value.clone(),
            Err(error) => json!({"error": error}),
        },
        running: false,
    });
    result
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
    }

    #[test]
    fn a_delegation_names_its_task() {
        assert_eq!(task("agent", &json!({"task":"fix login"})), "fix login");
        assert_eq!(task("solver", &json!({"issue": 42})), "42");
        assert_eq!(task("agent_list", &json!({})), "agent_list");
    }
}
