//! The ACP backend (#11508): the session's hooks on an agent Coder speaks
//! the Agent Client Protocol to (`acp-client`), whichever agent it is.
//!
//! Before a tool, an agent reaches the hooks one of two ways, and the tier
//! table (`coder_delegate::session::acp_backend`) says which agent does
//! what, from live runs only:
//!
//! - **Its asks.** An agent started so that it asks before every tool
//!   (OpenCode with its `permission` set to ask) sends
//!   `session/request_permission`; [`call`] reads the ask as the hooks'
//!   `PreToolUse` call, in Claude Code's shapes, and [`answer`] picks the
//!   agent's own option for what the hooks came to. An `Ask` goes to the
//!   host's asker (the approval card) while the agent waits. Any other
//!   agent's asks (Cursor's, before a command outside its allowlist) are
//!   answered the same way, but they are not every tool, so a floor can't
//!   count on them.
//! - **Its own hooks.** An agent that runs a hook program before every tool
//!   (Grok Build's plugin hooks, Devin's config hooks) runs Coder's bridge
//!   (`coder coder-hook --format acp`), which asks the session's runner on
//!   the host's socket ([`super::bridge::Host`], shared through
//!   [`Hooks::runner`]); [`claude_tool`] gives the agent's tool names and
//!   inputs Claude Code's.
//!
//! The host drives the rest of the turn itself: `SessionStart` before the
//! first prompt and `UserPromptSubmit` before each (their context goes
//! ahead of the prompt; a denial refuses the turn), `Stop` when a turn ends
//! (a block sends the reason as the next prompt in the same session), and
//! `SessionEnd` when it closes ([`Turns`]).
//!
//! A hook that must deny before a tool, on an agent that can't be stopped
//! there, is reported by [`hooks`] in plain words for the person; it stays
//! registered, so the asks that agent does send are still checked.

use acp_client::wire::{PermissionAnswer, PermissionRequest};
use coder_delegate::session::{HookBackend, acp_backend, hook_tier};
use serde_json::{Map, Value, json};

use super::compose::{Applied, Budget, Refused, check_start};
use super::events::{Call, Event};
use super::hooks::Hooks;
use super::outcome::{Permission, Stop};
use super::registry::Registry;

/// The tool a command reaches the hooks under.
pub const COMMAND_TOOL: &str = "Bash";

/// `registry`'s hooks for the ACP agent with catalog ID `agent` (named
/// `name` for people), and the hooks it can't keep before a tool, each
/// stated plainly. The hooks it can't keep stay registered: the asks the
/// agent does send are still answered by them.
#[must_use]
pub fn hooks(registry: Registry, budget: Budget, agent: &str, name: &str) -> (Hooks, Vec<Refused>) {
    let backend = acp_backend(agent);
    let refused = check_start(&registry, name, |event| hook_tier(backend, event.name()))
        .err()
        .unwrap_or_default();
    (Hooks::keeping(registry, budget, backend), refused)
}

/// Whether the agent's tools all reach the hooks before they run: asked
/// about, or through its own hooks.
#[must_use]
pub fn gated(agent: &str) -> bool {
    hook_tier(acp_backend(agent), Event::PreToolUse.name()).can_deny()
}

/// Whether the agent reaches the hooks through its own hook program (the
/// bridge), so the host must serve the session's runner on a socket.
#[must_use]
pub fn hooked(agent: &str) -> bool {
    acp_backend(agent) == HookBackend::AcpHooked
}

/// The first string among `fields` of `input`.
fn field<'a>(input: &'a Value, fields: &[&str]) -> Option<&'a str> {
    fields
        .iter()
        .find_map(|name| input.get(*name).and_then(Value::as_str))
        .filter(|text| !text.is_empty())
}

/// The fields agents name a file in.
const FILE_FIELDS: [&str; 6] = [
    "file_path",
    "filePath",
    "filepath",
    "target_file",
    "path",
    "notebook_path",
];

/// `input` as an object, with `name` set to `value` when it has none.
fn with(input: &Value, name: &str, value: Option<&str>) -> Value {
    let mut object = input.as_object().cloned().unwrap_or_else(Map::new);
    if let Some(value) = value
        && !object.get(name).is_some_and(Value::is_string)
    {
        object.insert(name.to_owned(), json!(value));
    }
    Value::Object(object)
}

/// A command as text: a string, or words joined.
fn command_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Array(words) => {
            let words: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
            (!words.is_empty()).then(|| words.join(" "))
        }
        _ => None,
    }
}

/// An agent's own tool name and input, in Claude Code's: a command is
/// `Bash` with its `command`, a read `Read`, an edit `Edit` and a new file
/// `Write`, each with its `file_path`. Other tools keep their names.
/// The agent's own fields are kept beside the ones added.
#[must_use]
pub fn claude_tool(tool: &str, input: &Value) -> (String, Value) {
    let named = tool.to_ascii_lowercase();
    match named.as_str() {
        "run_terminal_command"
        | "exec"
        | "bash"
        | "shell"
        | "execute"
        | "terminal"
        | "run_shell_command"
        | "run_command"
        | "shell_command" => {
            let command = input.get("command").and_then(command_text);
            let mut input = with(input, "command", None);
            if let Some(command) = command {
                input["command"] = json!(command);
            }
            (COMMAND_TOOL.to_owned(), input)
        }
        "read_file" | "read" | "readfile" | "view" | "view_file" => {
            let path = field(input, &FILE_FIELDS).map(str::to_owned);
            ("Read".to_owned(), with(input, "file_path", path.as_deref()))
        }
        "search_replace" | "edit_file" | "edit" | "multi_edit" | "multiedit" | "str_replace"
        | "str_replace_editor" | "apply_diff" | "notebook_edit" => {
            let path = field(input, &FILE_FIELDS).map(str::to_owned);
            ("Edit".to_owned(), with(input, "file_path", path.as_deref()))
        }
        "write" | "write_file" | "create_file" | "write_to_file" => {
            let path = field(input, &FILE_FIELDS).map(str::to_owned);
            (
                "Write".to_owned(),
                with(input, "file_path", path.as_deref()),
            )
        }
        _ => (tool.to_owned(), input.clone()),
    }
}

/// A permission ask as the hooks' `PreToolUse` call, in Claude Code's
/// shapes, by the ask's ACP kind. `seen` is the input the agent's own
/// `tool_call` update gave the call, for an ask that carries none
/// (Cursor's); `cwd` is the session's folder.
#[must_use]
pub fn call(request: &PermissionRequest, seen: Option<&Value>, cwd: &std::path::Path) -> Call {
    let tool = &request.tool_call;
    let raw = tool
        .raw_input
        .as_ref()
        .filter(|raw| raw.as_object().is_some_and(|fields| !fields.is_empty()))
        .or(seen)
        .cloned()
        .unwrap_or_else(|| json!({}));
    let location = tool
        .locations
        .first()
        .map(|location| location.path.clone())
        .filter(|path| !path.is_empty());
    let path = location.or_else(|| field(&raw, &FILE_FIELDS).map(str::to_owned));
    let title = tool.title.clone().unwrap_or_default();
    let (name, input) = match tool.kind.as_deref() {
        Some("execute") => {
            // An ask that names no command is judged by its title, which
            // agents set to the command (OpenCode, Cursor's `` `cmd` ``).
            let command = raw
                .get("command")
                .and_then(command_text)
                .unwrap_or_else(|| title.trim().trim_matches('`').to_owned());
            let mut input = with(&raw, "command", None);
            input["command"] = json!(command);
            if input.get("cwd").and_then(Value::as_str).is_none() {
                input["cwd"] = json!(cwd);
            }
            (COMMAND_TOOL.to_owned(), input)
        }
        Some("edit" | "delete" | "move") => {
            ("Edit".to_owned(), with(&raw, "file_path", path.as_deref()))
        }
        Some("read") => ("Read".to_owned(), with(&raw, "file_path", path.as_deref())),
        Some("fetch") => ("WebFetch".to_owned(), raw),
        Some("search") => ("Grep".to_owned(), raw),
        _ => {
            let named = raw
                .get("variant")
                .and_then(Value::as_str)
                .map_or_else(|| title.clone(), str::to_owned);
            claude_tool(&named, &raw)
        }
    };
    Call::tool(Event::PreToolUse, &name, input)
}

/// The agent's option for what the hooks came to: a denial is the agent's
/// own one-time refusal, anything else allows this call only, so the agent
/// keeps asking. An ask with no matching option is cancelled.
#[must_use]
pub fn answer(permission: &Permission, request: &PermissionRequest) -> PermissionAnswer {
    let option = match permission {
        Permission::Deny { .. } | Permission::Ask { .. } => request
            .options
            .iter()
            .find(|option| option.kind.as_deref() == Some("reject_once"))
            .map(|option| option.option_id.as_str())
            .or_else(|| request.reject()),
        Permission::Allow | Permission::NoOpinion => request.allow_once(),
    };
    option.map_or(PermissionAnswer::Cancelled, |option| {
        PermissionAnswer::Selected(option.to_owned())
    })
}

/// Why the hooks refused a call, for the agent's next turn: ACP's refusal
/// carries no reason, so the host says it.
#[must_use]
pub fn refusal(applied: &Applied) -> Option<String> {
    match &applied.permission {
        Permission::Deny { reason } => Some(reason.clone()),
        _ => None,
    }
}

/// The host's side of an ACP session's turns: `SessionStart` once,
/// `UserPromptSubmit` before each prompt, `Stop` after each turn, and
/// `SessionEnd` at the close.
#[derive(Debug)]
pub struct Turns {
    hooks: Hooks,
    started: bool,
    /// Turns a `Stop` block has sent on since the last prompt.
    continued: usize,
}

/// The most turns `Stop` blocks may add after one prompt, so a hook that
/// always blocks can't keep a session going on its own.
pub const MAX_CONTINUED: usize = 3;

impl Turns {
    #[must_use]
    pub fn new(hooks: Hooks) -> Self {
        Turns {
            hooks,
            started: false,
            continued: 0,
        }
    }

    /// The prompt as the agent gets it: `SessionStart`'s context (on the
    /// first) and `UserPromptSubmit`'s ahead of it.
    ///
    /// # Errors
    ///
    /// The reason a hook refused the turn.
    pub async fn prompt(&mut self, cwd: &std::path::Path, prompt: &str) -> Result<String, String> {
        let mut context = Vec::new();
        if !self.started {
            self.started = true;
            let started = self
                .hooks
                .fire(Call::new(
                    Event::SessionStart,
                    json!({"cwd": cwd, "source": "startup"}),
                ))
                .await;
            context.extend(started.context);
        }
        let submitted = self
            .hooks
            .fire(Call::new(
                Event::UserPromptSubmit,
                json!({"prompt": prompt}),
            ))
            .await;
        if let Permission::Deny { reason } = &submitted.permission {
            return Err(reason.clone());
        }
        context.extend(submitted.context);
        self.continued = 0;
        Ok(match super::claude::context_text(&context) {
            Some(context) => format!("{context}\n\n{prompt}"),
            None => prompt.to_owned(),
        })
    }

    /// A turn ended with `reply`: the next prompt a `Stop` block asks for,
    /// if any.
    pub async fn stopped(&mut self, reply: &str) -> Option<String> {
        let stopped = self
            .hooks
            .fire(Call::new(
                Event::Stop,
                json!({"last_assistant_message": reply, "stop_hook_active": self.continued > 0}),
            ))
            .await;
        match stopped.stop {
            Stop::Block { reason } if self.continued < MAX_CONTINUED => {
                self.continued += 1;
                Some(reason)
            }
            _ => None,
        }
    }

    /// The session is over.
    pub async fn end(&self, reason: &str) {
        let _ = self
            .hooks
            .fire(Call::new(Event::SessionEnd, json!({"reason": reason})))
            .await;
    }
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod tests;
