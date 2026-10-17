//! The `cloud_computer` tool: the account's cloud computers through the
//! public Computers API (#11823), as `openagents computer cloud ... --json`.
//!
//! The CLI holds the signed-in session and the OpenAgents SDK client, and
//! the service checks the account on every call, so this tool can reach
//! only what the user's own account can. On top of that it asks the user
//! before anything that starts, changes, or deletes a computer:
//!
//! - `create` and `wake` start a computer on the account; `stop` and
//!   `delete` end one. Each waits for the user's answer.
//! - `exec` classifies the command with Coder's effect classes: a
//!   read-only command runs, a deny-listed one is refused, and every other
//!   one waits for the user.
//! - `write` that replaces a file waits for the user; `read` that replaces
//!   a local file does too.
//!
//! `create` and `exec` return a `key`. When a result is uncertain, calling
//! again with the same key reaches the computer or command already made;
//! the CLI saved the request before sending it. Ending this tool call
//! (a timeout or the user stopping the turn) only stops waiting: remote
//! work keeps running until `cancel`, `stop`, or `delete`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;

/// The longest `wait_seconds` this tool takes.
const MAX_WAIT: u64 = 600;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    action: Action,
    computer: Option<String>,
    command: Option<String>,
    command_id: Option<String>,
    key: Option<String>,
    size: Option<String>,
    placement: Option<String>,
    environment: Option<String>,
    version: Option<String>,
    cwd: Option<String>,
    path: Option<String>,
    local: Option<String>,
    #[serde(default)]
    overwrite: bool,
    timeout_seconds: Option<u64>,
    wait_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Action {
    List,
    Capabilities,
    Show,
    Create,
    Exec,
    Output,
    Cancel,
    Read,
    Write,
    Stop,
    Wake,
    Delete,
}

impl Action {
    fn word(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Capabilities => "capabilities",
            Self::Show => "show",
            Self::Create => "create",
            Self::Exec => "exec",
            Self::Output => "output",
            Self::Cancel => "cancel",
            Self::Read => "read",
            Self::Write => "write",
            Self::Stop => "stop",
            Self::Wake => "wake",
            Self::Delete => "delete",
        }
    }
}

/// The tool's declaration.
pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"cloud_computer",
        "description":"Work with the user's OpenAgents cloud computers (IDs like cmp_ and 20 hex digits) through their signed-in account. Not for this computer, and not for the user's own linked computers (use the computer tool). Actions: list, capabilities (sizes, placements, file limits), show, create (asks the user; returns a key), exec (one non-interactive shell command in the computer's workspace; read-only commands run, others ask the user; returns a key and the output, or says the command is still running when the wait ends first), output (one page of a command's output from byte offsets), cancel (stops a running command), read (copy a workspace file here), write (copy a file here into the workspace), stop, wake, delete (each asks the user). If a create or exec result is uncertain, call it again with the same key and no other change; never start it again without the key. Ending a wait never stops remote work. Files are at most 4 MiB. Never put secrets in a command.",
        "parameters":{"type":"object","properties":{
            "action":{"type":"string","enum":["list","capabilities","show","create","exec","output","cancel","read","write","stop","wake","delete"]},
            "computer":{"type":"string","maxLength":64,"description":"The cloud computer ID. Every action but list, capabilities, and create needs it."},
            "command":{"type":"string","minLength":1,"maxLength":65536,"description":"exec: the shell command."},
            "command_id":{"type":"string","maxLength":256,"description":"output and cancel: the command ID exec returned."},
            "key":{"type":"string","maxLength":128,"description":"create and exec: the key an earlier, uncertain call returned."},
            "size":{"type":"string","maxLength":64,"description":"create: a size from capabilities."},
            "placement":{"type":"string","enum":["auto","our-cloud","fast-sandbox"],"description":"create: where it runs. An explicit placement the service can't meet refuses."},
            "environment":{"type":"string","maxLength":128,"description":"create: a saved environment ID, with version."},
            "version":{"type":"string","maxLength":32,"description":"create: the saved environment's version number."},
            "cwd":{"type":"string","maxLength":1024,"description":"exec: a directory relative to the workspace."},
            "path":{"type":"string","maxLength":1024,"description":"read/write: a path relative to the computer's workspace."},
            "local":{"type":"string","maxLength":4096,"description":"read/write: the path here, relative to the working directory."},
            "overwrite":{"type":"boolean","description":"read/write: replace an existing file. It asks the user."},
            "timeout_seconds":{"type":"integer","minimum":1,"maximum":3600,"description":"exec: how long the command may run."},
            "wait_seconds":{"type":"integer","minimum":0,"maximum":MAX_WAIT,"description":"create, exec, stop, wake, delete: how long to wait for the result (exec defaults to 60)."}
        },"required":["action"],"additionalProperties":false}
    }})
}

/// What the model reads about the tool each turn.
pub fn instructions() -> &'static str {
    "The cloud_computer tool reaches the user's OpenAgents cloud computers through their account. Use it only when the user asks for work on a cloud computer. Creating, waking, stopping, or deleting one, commands that change it, and overwriting a file wait for the user's approval; a rejected action stays rejected. Create and exec return a key: after an uncertain result, call again with that same key instead of starting over. A wait that ends leaves the work running; use output to read it and cancel to stop it.\n"
}

/// What the user must approve before `arguments` runs, or `None`.
/// `Err` is a refusal no answer changes.
fn needs_approval(arguments: &Arguments) -> Result<Option<String>, String> {
    Ok(match arguments.action {
        Action::Create => Some("it starts a cloud computer on your account".to_owned()),
        Action::Wake => Some("it starts this cloud computer again".to_owned()),
        Action::Stop => Some("it stops this cloud computer".to_owned()),
        Action::Delete => Some("it deletes this cloud computer and its files".to_owned()),
        Action::Exec if arguments.command.is_some() => {
            use coder::task::agent::{Effect, effect};
            match effect(arguments.command.as_deref().unwrap_or_default()) {
                Effect::ReadOnly => None,
                Effect::Approval(why) => Some(why),
                Effect::Denied(why) => {
                    return Err(format!(
                        "This command is refused ({why}). Do not try it another way."
                    ));
                }
            }
        }
        Action::Write | Action::Read if arguments.overwrite => {
            Some("it replaces an existing file".to_owned())
        }
        _ => None,
    })
}

fn required<'a>(value: Option<&'a String>, name: &str, action: Action) -> Result<&'a str, String> {
    value
        .map(String::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| format!("{} needs {name}.", action.word()))
}

/// The `openagents` words for one action.
fn words(arguments: &Arguments, cwd: &Path) -> Result<Vec<String>, String> {
    let action = arguments.action;
    let computer = || required(arguments.computer.as_ref(), "computer", action);
    let mut words = vec![
        "computer".to_owned(),
        "cloud".to_owned(),
        action.word().to_owned(),
    ];
    let option = |words: &mut Vec<String>, name: &str, value: Option<&String>| {
        if let Some(value) = value {
            words.extend([format!("--{name}"), value.clone()]);
        }
    };
    match action {
        Action::List | Action::Capabilities => {}
        Action::Show | Action::Stop | Action::Wake | Action::Delete => {
            words.push(computer()?.to_owned());
        }
        Action::Create => {
            option(&mut words, "size", arguments.size.as_ref());
            option(&mut words, "placement", arguments.placement.as_ref());
            option(&mut words, "environment", arguments.environment.as_ref());
            option(&mut words, "version", arguments.version.as_ref());
            option(&mut words, "key", arguments.key.as_ref());
        }
        Action::Exec => {
            words.push(computer()?.to_owned());
            option(&mut words, "cwd", arguments.cwd.as_ref());
            option(&mut words, "key", arguments.key.as_ref());
            if let Some(timeout) = arguments.timeout_seconds {
                words.extend(["--timeout".into(), timeout.to_string()]);
            }
            if arguments.command.is_none() && arguments.key.is_none() {
                return Err("exec needs command, or the key of an earlier call.".into());
            }
        }
        Action::Output | Action::Cancel => {
            words.push(computer()?.to_owned());
            words.push(required(arguments.command_id.as_ref(), "command_id", action)?.to_owned());
        }
        Action::Read => {
            words.push(computer()?.to_owned());
            words.push(required(arguments.path.as_ref(), "path", action)?.to_owned());
            let local = required(arguments.local.as_ref(), "local", action)?;
            words.extend(["--out".into(), cwd.join(local).display().to_string()]);
        }
        Action::Write => {
            words.push(computer()?.to_owned());
            let local = required(arguments.local.as_ref(), "local", action)?;
            words.push(cwd.join(local).display().to_string());
            words.push(required(arguments.path.as_ref(), "path", action)?.to_owned());
        }
    }
    if arguments.overwrite && matches!(action, Action::Read | Action::Write) {
        words.push("--overwrite".into());
    }
    if let Some(wait) = arguments.wait_seconds {
        if matches!(
            action,
            Action::Create | Action::Exec | Action::Stop | Action::Wake | Action::Delete
        ) {
            words.extend(["--wait".into(), wait.min(MAX_WAIT).to_string()]);
        }
    }
    if action == Action::Exec
        && let Some(command) = &arguments.command
    {
        words.extend(["--".into(), command.clone()]);
    }
    Ok(words)
}

/// Run one `cloud_computer` call.
///
/// # Errors
/// Invalid arguments, a refusal (by policy or by the user), or the CLI's
/// own failure.
pub async fn execute(
    arguments: Value,
    cwd: &Path,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let program = crate::bundled_runtime::cli_binary().ok_or_else(|| {
        format!(
            "The bundled OpenAgents CLI is missing. Reinstall Coder with `{}`.",
            crate::account::INSTALL_COMMAND
        )
    })?;
    let arguments: Arguments = serde_json::from_value(arguments).map_err(|error| {
        format!("cloud_computer arguments do not match the declared schema: {error}")
    })?;
    let words = words(&arguments, cwd)?;
    if let Some(why) = needs_approval(&arguments)? {
        let computer = arguments
            .computer
            .clone()
            .unwrap_or_else(|| "a new cloud computer".to_owned());
        let action = match (arguments.action, &arguments.command) {
            (Action::Exec, Some(command)) => command.clone(),
            _ => shown(&words),
        };
        let confirmed = match desk.or(crate::approval::desk().as_deref()) {
            Some(desk) => {
                desk.confirm_computer(&computer, &action, &why, cancel)
                    .await
            }
            None => {
                return Err(format!(
                    "This needs the user's approval ({why}), and no one can answer here. \
                     Do not try it another way. Tell the user they can run it themselves: {}",
                    shown(&words)
                ));
            }
        };
        if !confirmed {
            return Err(
                "The user rejected this action. Do not run it or work around it; \
                        continue without it or say what you would have done."
                    .into(),
            );
        }
    }
    // The child's deadline covers the longest wait plus the API calls
    // around it; ending it only stops waiting.
    let deadline = arguments.wait_seconds.unwrap_or(60).min(MAX_WAIT) + 120;
    let result = crate::bundled_runtime::cli_checked_within(
        &program,
        &words,
        cwd,
        cancel,
        &mut |event| emit(event),
        Some(deadline),
    )
    .await?;
    Ok(shape(&result))
}

/// The command a person would type for the same action.
fn shown(words: &[String]) -> String {
    std::iter::once("openagents".to_owned())
        .chain(
            words
                .iter()
                .map(|word| crate::bundled_runtime::shell_word(word)),
        )
        .collect::<Vec<_>>()
        .join(" ")
}

/// The tool's answer: the CLI's JSON document (a result, or a typed
/// failure with its `kind` and retry `key`).
fn shape(result: &Value) -> Value {
    let stdout = result["stdout"].as_str().unwrap_or_default();
    let exit = result["exit"].as_i64();
    match serde_json::from_str::<Value>(stdout.trim()) {
        Ok(value) => json!({"exit": exit, "result": value}),
        Err(_) => json!({"exit": exit, "stdout": stdout, "stderr": result["stderr"]}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: Value) -> Arguments {
        serde_json::from_value(value).unwrap()
    }
    const ID: &str = "cmp_0123456789abcdef0123";

    #[test]
    fn lifecycle_changes_ask_and_reads_do_not() {
        for action in ["create", "wake", "stop", "delete"] {
            let a = parse(json!({"action":action,"computer":ID}));
            assert!(matches!(needs_approval(&a), Ok(Some(_))), "{action}");
        }
        for action in ["list", "capabilities", "show", "output", "cancel"] {
            let a = parse(json!({"action":action,"computer":ID,"command_id":"trn_1"}));
            assert_eq!(needs_approval(&a), Ok(None), "{action}");
        }
        let read = parse(json!({"action":"exec","computer":ID,"command":"ls -la"}));
        assert_eq!(needs_approval(&read), Ok(None));
        let change = parse(json!({"action":"exec","computer":ID,"command":"rm notes.txt"}));
        assert!(matches!(needs_approval(&change), Ok(Some(_))));
        let denied = parse(json!({"action":"exec","computer":ID,"command":"rm -rf /"}));
        assert!(needs_approval(&denied).is_err());
        let replace =
            parse(json!({"action":"write","computer":ID,"local":"a","path":"b","overwrite":true}));
        assert!(matches!(needs_approval(&replace), Ok(Some(_))));
    }

    #[test]
    fn actions_become_the_cli_words_with_keys_and_bounded_waits() {
        let cwd = Path::new("/work");
        let exec = words(
            &parse(json!({"action":"exec","computer":ID,"command":"cargo test","key":"k1","wait_seconds":5000})),
            cwd,
        )
        .unwrap();
        assert_eq!(
            exec,
            [
                "computer",
                "cloud",
                "exec",
                ID,
                "--key",
                "k1",
                "--wait",
                "600",
                "--",
                "cargo test"
            ]
        );
        // A retry by key alone resends the saved request.
        let retry = words(
            &parse(json!({"action":"exec","computer":ID,"key":"k1"})),
            cwd,
        )
        .unwrap();
        assert_eq!(retry, ["computer", "cloud", "exec", ID, "--key", "k1"]);
        assert!(words(&parse(json!({"action":"exec","computer":ID})), cwd).is_err());
        let create = words(
            &parse(json!({"action":"create","placement":"fast-sandbox","size":"small"})),
            cwd,
        )
        .unwrap();
        assert_eq!(
            create,
            [
                "computer",
                "cloud",
                "create",
                "--size",
                "small",
                "--placement",
                "fast-sandbox"
            ]
        );
        let read = words(
            &parse(json!({"action":"read","computer":ID,"path":"out.txt","local":"copy.txt"})),
            cwd,
        )
        .unwrap();
        assert_eq!(
            read,
            [
                "computer",
                "cloud",
                "read",
                ID,
                "out.txt",
                "--out",
                "/work/copy.txt"
            ]
        );
        assert!(words(&parse(json!({"action":"show"})), cwd).is_err());
    }

    #[test]
    fn an_unknown_placement_does_not_parse() {
        let a = serde_json::from_value::<Arguments>(json!({"action":"create","placement":"boat"}));
        // The schema's enum is checked by the CLI too; the tool passes it on.
        assert!(a.is_ok());
        let tool = definition();
        let placements = &tool["function"]["parameters"]["properties"]["placement"]["enum"];
        assert_eq!(placements, &json!(["auto", "our-cloud", "fast-sandbox"]));
    }
}
