//! The `acp` tool: hand a task to a coding agent installed on this machine.
//!
//! This is Coder's own capability rather than one of the five every
//! session has, so it is registered as a [`HostTool`] and answered here. It is
//! declared only when [`crate::coder::acp::find_agents`] found at least one agent
//! installed, and its `agent` parameter enumerates exactly those — a session
//! on a machine with no ACP agent does not see the tool at all, which is the
//! difference between a capability and a claim.
//!
//! It is not `delegate`, and the two are kept apart on purpose. `delegate`
//! starts child `openagents` coder agents on this session's own lane and
//! budget; `acp` speaks the Agent Client Protocol to a different program — a
//! Devin, a Claude Code, whatever the registry found — which brings its own
//! credentials and its own bill. A model that could not tell them apart would
//! reach for whichever it saw first.
//!
//! ## One agent per user turn
//!
//! An exported trajectory showed twenty-four consecutive delegations for a
//! single "do a test delegation" message: the model handed the task off, read
//! the answer, and handed it off again. Each one is a whole second agent on
//! somebody's bill, so the second call in a turn is refused and told why,
//! which leaves the model with nothing to do but answer.
//!
//! This replaces the `tool_choice: none` clamp that did the same job while the
//! ACP path was the openresponses runtime's only tool (commit `afea5551fa`).
//! The mechanism changed because the turn loop did: the runtime beneath this
//! sends no `tool_choice`, and a refusal is the stronger of the two anyway —
//! it says what the limit is rather than silently withdrawing the tool, and it
//! holds whether or not a deployment honours the field.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::tools::{HostTool, ToolCall, ToolDefinition};

use crate::coder::acp::Agent;
use crate::coder::acp_harness::{AcpEvent, AcpFailure, AcpHarness, PermissionMode};
use crate::coder::runtime::{Control, Sink, send};

/// The name the model calls it by.
pub const ACP_TOOL: &str = "acp";

/// A plan upsell is not work done.
///
/// An agent that answers "upgrade your plan to continue" has returned a string
/// rather than performed the task, and reporting that as a successful tool
/// result is how a session comes to believe a file was edited when nothing
/// touched it.
fn is_refusal(answer: &str) -> bool {
    answer
        .to_lowercase()
        .contains("upgrade your plan to continue")
}

/// The `acp` tool for `agents`, or `None` when none are installed.
///
/// `spent` is cleared at the top of every user turn by
/// [`crate::coder::runtime::Session::execute_turn`]; see the module header.
pub fn acp_host_tool(
    agents: Vec<Agent>,
    cwd: PathBuf,
    sink: Sink,
    spent: Arc<AtomicBool>,
) -> Option<HostTool> {
    if agents.is_empty() {
        return None;
    }

    let ids: Vec<String> = agents.iter().map(|agent| agent.id.clone()).collect();
    let listed = agents
        .iter()
        .map(|agent| format!("`{}` ({})", agent.id, agent.name))
        .collect::<Vec<_>>()
        .join(", ");

    let definition = ToolDefinition {
        name: ACP_TOOL.to_string(),
        description: format!(
            "Hand one self-contained task to a coding agent installed on this machine, over the \
             Agent Client Protocol, and return what it answered. The agent runs in {} with its \
             own tools, its own credentials, and no context from this conversation, so the prompt \
             has to say everything it needs: the files, the command, and what to report back. It \
             cannot ask a question. Installed here: {}. Prefer `shell` for a single command — an \
             ACP agent is for work worth a whole agent.",
            cwd.display(),
            listed
        ),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "enum": ids,
                    "description": "Which installed agent performs the task."
                },
                "prompt": {
                    "type": "string",
                    "description": "The complete, self-contained instruction the agent performs."
                },
                "mode": {
                    "type": "string",
                    "enum": ["read-only", "prompt", "dangerous"],
                    "description": "How much the agent may do unattended. Omit to leave the agent's own default; `read-only` for a look that changes nothing."
                }
            },
            "required": ["agent", "prompt"]
        }),
    };

    let run = Arc::new(move |call: &ToolCall| {
        let agents = agents.clone();
        let cwd = cwd.clone();
        let sink = Arc::clone(&sink);
        let spent = Arc::clone(&spent);
        let call_id = call.id.clone();
        let arguments = call.arguments.clone();
        let future = async move {
            // Claimed before anything is checked, so a malformed second call
            // cannot spend the turn's one delegation on an error message.
            if spent.swap(true, Ordering::SeqCst) {
                return (
                    "This turn has already handed work to an agent, and one is the limit: a \
                     second agent is a second bill for the same request. Answer with what the \
                     first one returned, or ask for another turn."
                        .to_string(),
                    true,
                );
            }
            let string = |key: &str| {
                arguments
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
            };

            let Some(wanted) = string("agent") else {
                return (
                    format!(
                        "No agent was named. `agent` is required and must be one of: {}.",
                        agents
                            .iter()
                            .map(|a| a.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    true,
                );
            };
            let Some(agent) = agents.iter().find(|a| a.id == wanted).cloned() else {
                return (
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
            let Some(prompt) = string("prompt") else {
                return (
                    "No task was given. `prompt` is required and must say what the agent does."
                        .to_string(),
                    true,
                );
            };
            // A mode this build does not know is refused by name rather than
            // quietly dropped: the reader asked for read-only and would
            // otherwise get whatever the agent's default is.
            let mode = match string("mode") {
                None => None,
                Some(named) => match PermissionMode::parse(&named) {
                    Some(mode) => Some(mode),
                    None => {
                        return (
                            format!(
                                "`{named}` is not a mode. Use `read-only`, `prompt`, or \
                                 `dangerous`, or omit it for the agent's own default."
                            ),
                            true,
                        );
                    }
                },
            };

            let streaming = Arc::clone(&sink);
            let id = call_id.clone();
            // Nothing here stops the child early, so the cancellation channel
            // is held open for the length of the run: dropping the sender
            // would signal a cancel the reader never asked for.
            let (_stop, mut cancel) = tokio::sync::watch::channel(false);
            let result = AcpHarness {
                command: agent.command,
                args: agent.args,
                mode,
                ..AcpHarness::default()
            }
            .run(
                &prompt,
                &cwd,
                move |event| {
                    // What the child is doing, into the box under its header,
                    // while it is still doing it.
                    let chunk = match event {
                        AcpEvent::Text { chunk } => chunk,
                        AcpEvent::Tool { kind, title } => format!("[{kind}] {title}\n"),
                        AcpEvent::Tokens { input, output } => {
                            format!("[{input} in / {output} out tokens]\n")
                        }
                        AcpEvent::Session { .. } => return,
                    };
                    send(
                        &streaming,
                        Control::ToolOutput {
                            call_id: id.clone(),
                            chunk,
                        },
                    );
                },
                &mut cancel,
            )
            .await;

            match result {
                Ok(answer) if is_refusal(&answer) => (
                    format!("`{wanted}` refused the task rather than doing it: {answer}"),
                    true,
                ),
                Ok(answer) if answer.trim().is_empty() => {
                    (format!("`{wanted}` finished and said nothing."), false)
                }
                Ok(answer) => (answer, false),
                Err(AcpFailure::Unstartable(why)) => {
                    (format!("`{wanted}` could not be started: {why}"), true)
                }
                Err(AcpFailure::Refused(why)) => {
                    (format!("`{wanted}` did not finish the task: {why}"), true)
                }
                Err(AcpFailure::Cancelled) => {
                    (format!("`{wanted}` was stopped before it finished."), true)
                }
            }
        };
        Box::pin(future)
            as std::pin::Pin<Box<dyn std::future::Future<Output = (String, bool)> + Send>>
    });

    Some(HostTool { definition, run })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::mpsc;

    fn agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            command: "definitely-not-a-real-binary-xyz".to_string(),
            args: vec!["acp".to_string()],
        }
    }

    fn sink() -> (Sink, mpsc::Receiver<Control>) {
        let (tx, rx) = mpsc::channel();
        (Arc::new(Mutex::new(tx)), rx)
    }

    /// A tool whose one delegation has not been spent yet.
    fn fresh(agents: Vec<Agent>, sink: Sink) -> Option<HostTool> {
        acp_host_tool(
            agents,
            std::env::temp_dir(),
            sink,
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn call(arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "1".to_string(),
            name: "acp".to_string(),
            arguments,
        }
    }

    /// A machine with no ACP agent installed does not get the tool. The
    /// alternative is a tool whose `agent` enum is empty, which a model can
    /// call and nothing can answer.
    #[test]
    fn no_installed_agent_means_no_tool() {
        let (sink, _rx) = sink();
        assert!(fresh(Vec::new(), sink).is_none());
    }

    #[test]
    fn the_declaration_enumerates_exactly_the_installed_agents() {
        let (sink, _rx) = sink();
        let tool = fresh(vec![agent("devin"), agent("grok-build")], sink).expect("declared");
        assert_eq!(tool.definition.name, "acp");
        let ids = &tool.definition.parameters["properties"]["agent"]["enum"];
        assert_eq!(ids, &serde_json::json!(["devin", "grok-build"]));
        assert!(tool.definition.description.contains("`devin`"));
    }

    #[tokio::test]
    async fn an_agent_that_is_not_installed_is_refused_by_name() {
        let (sink, _rx) = sink();
        let tool = fresh(vec![agent("devin")], sink).expect("declared");
        let (output, is_error) = (tool.run)(&ToolCall {
            id: "1".to_string(),
            name: "acp".to_string(),
            arguments: serde_json::json!({"agent": "nobody", "prompt": "do it"}),
        })
        .await;
        assert!(is_error);
        assert!(output.contains("nobody"), "{output}");
        assert!(output.contains("devin"), "{output}");
    }

    #[tokio::test]
    async fn a_mode_this_build_does_not_know_is_refused_rather_than_dropped() {
        let (sink, _rx) = sink();
        let tool = fresh(vec![agent("devin")], sink).expect("declared");
        let (output, is_error) = (tool.run)(&ToolCall {
            id: "1".to_string(),
            name: "acp".to_string(),
            arguments: serde_json::json!({"agent": "devin", "prompt": "look", "mode": "whatever"}),
        })
        .await;
        assert!(is_error);
        assert!(output.contains("read-only"), "{output}");
    }

    /// An agent that is not on PATH is said to be missing. It used to be
    /// reported to the model as the tool's answer.
    #[tokio::test]
    async fn an_agent_that_will_not_start_is_an_error_and_says_why() {
        let (sink, _rx) = sink();
        let tool = fresh(vec![agent("devin")], sink).expect("declared");
        let (output, is_error) = (tool.run)(&ToolCall {
            id: "1".to_string(),
            name: "acp".to_string(),
            arguments: serde_json::json!({"agent": "devin", "prompt": "do it"}),
        })
        .await;
        assert!(is_error, "{output}");
        assert!(output.contains("could not be started"), "{output}");
    }

    /// Twenty-four consecutive delegations for one message is what this
    /// exists to stop; the mechanism it replaces was `tool_choice: none`
    /// (commit `afea5551fa`).
    #[tokio::test]
    async fn only_one_agent_is_handed_work_per_turn() {
        let (sink, _rx) = sink();
        let spent = Arc::new(AtomicBool::new(false));
        let tool = acp_host_tool(
            vec![agent("devin")],
            std::env::temp_dir(),
            sink,
            Arc::clone(&spent),
        )
        .expect("declared");

        // The first call is let through — it fails only because the stand-in
        // binary does not exist, which is a different refusal.
        let (first, _) = (tool.run)(&call(
            serde_json::json!({"agent": "devin", "prompt": "do it"}),
        ))
        .await;
        assert!(first.contains("could not be started"), "{first}");

        let (second, is_error) = (tool.run)(&call(
            serde_json::json!({"agent": "devin", "prompt": "again"}),
        ))
        .await;
        assert!(is_error, "{second}");
        assert!(second.contains("already handed work"), "{second}");
        assert!(second.contains("one is the limit"), "{second}");

        // A new turn clears it.
        spent.store(false, Ordering::SeqCst);
        let (third, _) = (tool.run)(&call(
            serde_json::json!({"agent": "devin", "prompt": "next turn"}),
        ))
        .await;
        assert!(third.contains("could not be started"), "{third}");
    }

    /// A malformed second call must not be the one that gets through.
    #[tokio::test]
    async fn a_refused_second_call_is_refused_before_its_arguments_are_read() {
        let (sink, _rx) = sink();
        let spent = Arc::new(AtomicBool::new(true));
        let tool = acp_host_tool(vec![agent("devin")], std::env::temp_dir(), sink, spent)
            .expect("declared");
        let (output, is_error) = (tool.run)(&call(serde_json::json!({}))).await;
        assert!(is_error);
        assert!(output.contains("already handed work"), "{output}");
    }

    #[test]
    fn a_plan_upsell_is_not_a_completed_task() {
        assert!(is_refusal("Please upgrade your plan to continue."));
        assert!(!is_refusal("Done. Edited src/main.rs."));
    }
}
