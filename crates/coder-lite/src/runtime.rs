//! coder-lite's turn: its own voice over the capable runtime next door.
//!
//! The session underneath is [`openagents_cli::runtime::CoderRuntimeSession`]
//! — threads, grants, the inference proxy, the live model catalog, lanes,
//! metering, and revocation — and it is used rather than copied, so there is
//! one implementation of each of those and this crate cannot drift from it.
//!
//! What this file owns is the part that is coder-lite's:
//!
//! - The system prompt, composed from the staged `system-prompt` surface
//!   (`surfaces/coder/system-prompt.v1.json`) and carried verbatim. It is
//!   the reason the session
//!   answers as a terminal rather than as an assistant, and a merge that
//!   reworded it would have changed the product.
//! - [`Control`], the one-way channel the TUI loop reads. Text, tool calls,
//!   tool results, the model that answered, what the turn spent, and the
//!   failures — each as its own message, so the frame shows what happened and
//!   not a summary written afterwards.
//!
//! ## Nothing here invents an answer
//!
//! Every path is the model's own words or a [`Control::Failed`] naming what
//! refused. There is no fallback model, no synthesized reply, and no invented
//! grant: the session below refuses out loud and this file carries the refusal
//! to the screen instead of painting over it.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use openagents_cli::surfaces::system_prompt as prompt;
use openagents_cli::runtime::{
    ChatMessage, CoderRuntimeSession, Lane, ToolEvent, TurnUsage,
};
use openagents_cli::tools::{DelegationGate, HarnessToolRegistry, ToolDefinition};


type Failure = Box<dyn std::error::Error + Send + Sync>;

/// What the runtime tells the frame, in the order it happened.
#[derive(Debug, Clone)]
pub enum Control {
    /// A piece of the reply, as the model wrote it.
    Chunk(String),
    /// A tool call started. The header goes up now; the box fills in later.
    Tool {
        call_id: String,
        name: String,
        /// The raw JSON string from the wire.
        arguments: String,
    },
    /// More of what that call has printed, appended to its box.
    ToolOutput { call_id: String, chunk: String },
    /// That call finished, and whether it worked.
    ToolDone { call_id: String, is_error: bool },
    /// The model that actually answered, as its grant pins it.
    Model(String),
    /// What the turn spent, as the server reported it.
    Usage(TurnUsage),
    /// What one read of `GET /api/v1/credit` found, and `None` when it found
    /// nothing. Read rather than subtracted from `Usage`: spend is the
    /// server's, and this session is not the only thing spending it. The
    /// failure travels too, because the frame has to stop showing the previous
    /// answer rather than leave it up.
    Credit(Option<crate::credit::Credit>),
    /// Something worth saying that is not the model talking.
    Notice(String),
    /// What one of the session's own commands printed. Markdown, rendered the
    /// way an answer is, and exported as a notice rather than a model step.
    Output(String),
    /// The turn did not answer, and this is why. Never an answer.
    Failed(String),
    /// The turn is over, one way or the other.
    Done,
}

/// A `Sender` an observer can hold: `Fn` observers are shared, and the frame
/// loop's receiver is on the other end of exactly one channel.
pub type Sink = Arc<Mutex<Sender<Control>>>;

/// Put a message on the frame's channel, or drop it if the frame is gone.
pub fn send(sink: &Sink, message: Control) {
    if let Ok(tx) = sink.lock() {
        let _ = tx.send(message);
    }
}

/// The system message this session opens with.
///
/// The staged instructions first and unchanged, then the tools — because a
/// model that is told it has no tools when it has some will not use them, and
/// one told it has tools it does not have will claim to have run them. The
/// list is generated from what was actually declared, so the two cannot
/// disagree.
pub fn system_prompt(tools: &[ToolDefinition]) -> String {
    let mut lines = vec![prompt::CODER_LITE_INSTRUCTIONS.to_string(), String::new()];
    if tools.is_empty() {
        lines.push(prompt::CODER_LITE_NO_TOOLS.to_string());
    } else {
        lines.push(
            prompt::CODER_LITE_TOOL_LIST_HEADER.replace("{count}", &tools.len().to_string()),
        );
        for tool in tools {
            lines.push(format!("- `{}`", tool.name));
        }
        lines.push(String::new());
        lines.push(prompt::CODER_LITE_TOOL_LIST_CLOSING.to_string());
    }
    lines.join("\n")
}

/// The `/api/v1` base this session talks to.
///
/// `OPENAGENTS_BASE_URL` first, because that is what `--dev` sets and a reader
/// who pointed the session at a server on this machine meant it. Then the
/// endpoint `oa auth` selected, so coder-lite and `oa` agree about where they
/// are without a second configuration file.
pub fn api_base() -> String {
    for name in ["OPENAGENTS_BASE_URL", "OPENAGENTS_API_BASE"] {
        if let Ok(value) = env::var(name) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }
    match openagents_cli::auth::resolve_endpoint(None, None) {
        Ok(endpoint) => format!("{}/api/v1", endpoint.origin),
        Err(_) => "https://openagents.com/api/v1".to_string(),
    }
}

/// The credential this session spends, or `None`.
///
/// `None` is carried rather than papered over: a thread request without one is
/// refused by the server, and that refusal is what the reader should see.
pub fn user_token() -> Option<String> {
    if let Ok(value) = env::var("OPENAGENTS_API_KEY") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    let endpoint = openagents_cli::auth::resolve_endpoint(None, None).ok()?;
    openagents_cli::auth::CredentialStore::for_origin(&endpoint.origin).get_token()
}

/// The session a coder-lite frame drives.
pub struct Session {
    inner: CoderRuntimeSession,
    lane: Lane,
    /// Whether this user turn has already handed work to an ACP agent.
    ///
    /// Cleared at the top of every turn and set by the `acp` tool itself; see
    /// [`crate::acp_tool`] for why one is the limit.
    acp_spent: Arc<AtomicBool>,
}

impl Session {
    /// Open a session on `lane`, reporting everything it does to `tx`.
    ///
    /// The tools are the full set — `read`, `write`, `edit`, `bash`, `shell`,
    /// `skill`, `openagents`, `capability`, and `delegate` — on the same terms
    /// `oa coder` gets them: children run on this lane, on this credential, and
    /// cannot delegate again.
    pub fn open(
        lane: Lane,
        lane_name: &str,
        reasoning: Option<String>,
        agents: Vec<crate::acp::Agent>,
        dev: bool,
        tx: Sender<Control>,
    ) -> Self {
        Self::open_at(
            lane,
            lane_name,
            reasoning,
            agents,
            api_base(),
            user_token(),
            dev,
            tx,
        )
    }

    /// [`Self::open`] against a named server with a named credential.
    ///
    /// The environment is process-global and a test that set it would race
    /// every other test in the same binary, so the two values the environment
    /// supplies are parameters here and read from the environment exactly once,
    /// in `open`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_at(
        lane: Lane,
        lane_name: &str,
        reasoning: Option<String>,
        agents: Vec<crate::acp::Agent>,
        api_base: String,
        token: Option<String>,
        dev: bool,
        tx: Sender<Control>,
    ) -> Self {
        let mut tools = HarnessToolRegistry::with_delegation(
            None,
            DelegationGate {
                lane: lane_name.to_string(),
                user_token: token.clone(),
                max_count: openagents_cli::delegate::MAX_DELEGATE_COUNT,
                // coder-lite takes no `--child-*` flags yet, so children start
                // on the defaults. Set rather than defaulted at the struct so
                // adding those flags here is a visible edit, not a silent
                // inheritance of whatever the field grows into.
                child: openagents_cli::delegate::ChildOptions::default(),
            },
        );

        let sink: Sink = Arc::new(Mutex::new(tx));
        let observed = Arc::clone(&sink);

        // coder-lite's own capability, declared only where there is one to
        // declare: `find_agents` reports installed agents, so a machine with
        // none does not see the tool.
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let found = !agents.is_empty();
        let acp_spent = Arc::new(AtomicBool::new(false));
        if let Some(tool) = crate::acp_tool::acp_host_tool(
            agents,
            cwd,
            Arc::clone(&sink),
            Arc::clone(&acp_spent),
        ) {
            if let Err(refusal) = tools.add_host_tool(tool) {
                send(&sink, Control::Notice(refusal));
            }
        } else if found {
            // Unreachable while `acp_host_tool` refuses only an empty list,
            // and here so that stops being silently true if it stops being.
            send(
                &sink,
                Control::Notice("ACP agents were found but no `acp` tool was declared".to_string()),
            );
        }

        let mut inner = CoderRuntimeSession::new(lane.clone(), Some(api_base), token, tools)
        .observing_tools(Arc::new(move |event: ToolEvent| match event {
            ToolEvent::Started {
                call_id,
                name,
                arguments,
            } => send(
                &observed,
                Control::Tool {
                    call_id,
                    name,
                    arguments,
                },
            ),
            ToolEvent::Finished {
                call_id,
                output,
                is_error,
                ..
            } => {
                send(
                    &observed,
                    Control::ToolOutput {
                        call_id: call_id.clone(),
                        chunk: output,
                    },
                );
                send(&observed, Control::ToolDone { call_id, is_error });
            }
        }))
        .use_openresponses(dev);
        inner.reasoning = reasoning;
        inner.repository = repository();

        // Seeded here so the session below leaves it alone: `execute_turn`
        // writes its own system prompt only into an empty message list, and
        // this one is coder-lite's.
        let prompt = system_prompt(&inner.tools.list_tools());
        inner.messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(prompt),
            tool_calls: None,
            tool_call_id: None,
        });

        Self {
            inner,
            lane,
            acp_spent,
        }
    }

    /// The lane this session was opened on. What was asked for, not what
    /// answered — [`Control::Model`] carries that.
    pub fn lane(&self) -> &Lane {
        &self.lane
    }

    /// Run one turn, streaming everything it does down `tx`.
    ///
    /// Always ends with exactly one [`Control::Done`], so a frame cannot be
    /// left spinning over a turn that has finished.
    pub async fn execute_turn(&mut self, prompt: &str, tx: Sender<Control>) {
        // A fresh turn may hand work to an agent again. The limit is per user
        // turn, not per session.
        self.acp_spent.store(false, Ordering::SeqCst);
        let sink: Sink = Arc::new(Mutex::new(tx));
        let chunks = Arc::clone(&sink);
        // Whether the reply reached the frame as it was written. The answer
        // `execute_turn` returns has normally already streamed, so repeating
        // it would print it twice; this is what tells the one case from the
        // other.
        let streamed = Arc::new(AtomicBool::new(false));
        let saw = Arc::clone(&streamed);
        let result = self
            .inner
            .execute_turn(prompt, move |chunk| {
                if !chunk.is_empty() {
                    saw.store(true, Ordering::Relaxed);
                    send(&chunks, Control::Chunk(chunk.to_string()));
                }
            })
            .await;

        if let Some(model) = &self.inner.last_model {
            send(&sink, Control::Model(model.clone()));
        }
        if self.inner.last_usage.reported() {
            send(&sink, Control::Usage(self.inner.last_usage));
        }
        match result {
            Ok(answer) => {
                // The fallback for a path that answered without streaming, and
                // nothing else: an empty answer stays empty rather than
                // becoming a sentence somebody could read as a reply.
                if !answer.is_empty() && !streamed.load(Ordering::Relaxed) {
                    send(&sink, Control::Chunk(answer));
                }
            }
            Err(error) => send(&sink, Control::Failed(error.to_string())),
        }
        for failure in self.inner.record_failures.drain(..) {
            send(&sink, Control::Notice(failure));
        }
        send(&sink, Control::Done);
    }

    /// End this session's thread by saying what it did, and say what the
    /// server billed.
    ///
    /// A report rather than a `DELETE`: a session that answered and left is not
    /// a cancellation, and filing it as one is what made every thread in the
    /// account's history read as cancelled (issue #106). It also leaves the
    /// thread resumable, which `DELETE` does not.
    ///
    /// Awaited by the caller rather than left to `Drop`: a thread left open
    /// holds its grant's remaining budget, and the `Drop` backstop can only
    /// spawn an ending this process may exit before polling.
    pub async fn finish(&mut self) -> Result<Option<String>, Failure> {
        let spent = self.inner.finish().await?;
        Ok(self.inner.spend_line(spent))
    }

    /// The failures the runtime recorded and nothing has shown yet.
    ///
    /// [`Self::execute_turn`] drains these into notices; [`Self::finish`]
    /// cannot, because its caller is not a turn. `/logout` reads them so an
    /// ending that fell back to a cancellation says so on screen rather than
    /// only in the runtime's field.
    pub fn take_record_failures(&mut self) -> Vec<String> {
        self.inner.record_failures.drain(..).collect()
    }
}

/// The repository this session was opened in, as `owner/name`, when it is one.
///
/// Recorded on the thread so `oa coder --resume` has something to filter on. A
/// directory that is not an OpenAgents checkout has none, which is not an
/// error — the thread is simply not attributable to a repository.
fn repository() -> Option<String> {
    let endpoint = openagents_cli::auth::resolve_endpoint(None, None).ok()?;
    openagents_cli::repo::infer_repository(&endpoint.origin, None).ok()
}

/// The one-line header a tool call shows above its output box.
///
/// Built from the call's own arguments, so it says what was actually asked
/// for. A tool this does not know by name still gets a header rather than a
/// blank one, because a call with no header is a call the reader cannot see.
pub fn tool_title(name: &str, arguments: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let string = |key: &str| {
        parsed
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let detail = match name {
        "shell" | "bash" => string("command"),
        // The path, never the content: `write` carries a whole file in its
        // arguments, and the fallthrough below would put it in the header.
        "read" | "write" | "edit" => string("path"),
        "skill" => string("name"),
        "openagents" => parsed.get("args").and_then(|v| v.as_array()).map(|args| {
            args.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        }),
        "capability" => string("name").or_else(|| string("query")),
        "delegate" => {
            let count = parsed
                .get("count")
                .and_then(|v| v.as_u64())
                .filter(|n| *n > 1)
                .map(|n| format!("×{n} "))
                .unwrap_or_default();
            string("prompt").map(|prompt| format!("{count}{prompt}"))
        }
        // A plugin loaded through `capability` declares a tool under its own
        // name and over its own schema, so there is nothing general to read
        // out of it but the arguments themselves.
        _ => (parsed != serde_json::Value::Null).then(|| parsed.to_string()),
    };

    match detail {
        Some(detail) => format!("{name} {}", one_line(&detail)),
        None => name.to_string(),
    }
}

/// The first line of `text`, marked when there was more.
fn one_line(text: &str) -> String {
    let first = text.lines().next().unwrap_or_default().trim();
    if text.lines().nth(1).is_some() {
        format!("{first} …")
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt is the product. A merge that reworded it would pass every
    /// other test in this crate.
    #[test]
    fn the_system_prompt_opens_with_the_terse_instructions_unchanged() {
        let prompt = system_prompt(&[]);
        assert!(
            prompt.starts_with(prompt::CODER_LITE_INSTRUCTIONS),
            "the instructions were not carried verbatim: {prompt}"
        );
        assert!(prompt::CODER_LITE_INSTRUCTIONS.contains("no greetings"));
        assert!(prompt::CODER_LITE_INSTRUCTIONS.contains("no unnecessary padding"));
    }

    /// A model told it has tools it does not have will claim to have run them.
    #[test]
    fn the_prompt_names_every_declared_tool_and_claims_no_others() {
        let tools = vec![
            ToolDefinition {
                name: "shell".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "skill".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
        ];
        let prompt = system_prompt(&tools);
        assert!(prompt.contains("You have 2 tools, and no others:"), "{prompt}");
        assert!(prompt.contains("- `shell`"), "{prompt}");
        assert!(prompt.contains("- `skill`"), "{prompt}");

        let none = system_prompt(&[]);
        assert!(none.contains("You have no tools in this session"), "{none}");
    }

    #[test]
    fn a_tool_header_says_what_the_call_asked_for() {
        assert_eq!(
            tool_title("shell", r#"{"command":"cargo test -p coder-lite"}"#),
            "shell cargo test -p coder-lite"
        );
        assert_eq!(tool_title("skill", r#"{"name":"effect"}"#), "skill effect");
        assert_eq!(
            tool_title("openagents", r#"{"args":["issue","list"]}"#),
            "openagents issue list"
        );
        assert_eq!(
            tool_title("delegate", r#"{"prompt":"read it","count":3}"#),
            "delegate ×3 read it"
        );
        // A multi-line command is one line on the header, and says so.
        assert_eq!(
            tool_title("shell", "{\"command\":\"one\\ntwo\"}"),
            "shell one …"
        );
        // Arguments that will not parse are not a reason to draw no header.
        assert_eq!(tool_title("shell", "not json"), "shell");
    }
}
