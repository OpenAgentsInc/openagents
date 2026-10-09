//! The `computer` tool: the owner's own linked computers, the way the
//! owner would use them from a terminal. It lists them, takes a screenshot
//! of one's screen, lists its open apps, copies files to and from it, and
//! runs a command on it.
//!
//! Every action is `openagents computer ...` with `--json`, over the host
//! connection that command already uses (NIP-HOST and NIP-TERM, never raw
//! SSH), so the host checks this device's grant on each one. On top of
//! that, this tool asks the owner first for anything that changes the
//! other computer:
//!
//! - `run` classifies the command with Coder's effect classes
//!   ([`coder::task::agent::effect`]): a read-only command runs, a
//!   deny-listed one is refused, and every other one waits for the owner's
//!   answer on the approval desk.
//! - `push` that replaces a file (`overwrite`) waits for the owner too; a
//!   push to a new path cannot replace anything, because the host refuses
//!   an existing file without `overwrite`.
//!
//! Without a desk to ask on (a chat nobody is watching), an action that
//! needs the owner's answer is refused, with the command the owner can run
//! themselves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;

/// The largest screenshot the tool shows the model, in bytes.
pub const LOOK_MAX_BYTES: u64 = 4 * 1024 * 1024;
/// The longest a `run` command may take, in seconds.
const RUN_TIMEOUT: u64 = 600;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    action: Action,
    computer: Option<String>,
    command: Option<String>,
    remote: Option<String>,
    local: Option<String>,
    #[serde(default)]
    overwrite: bool,
    screen: Option<String>,
    #[serde(default)]
    android: bool,
    serial: Option<String>,
    #[serde(default)]
    look: bool,
    timeout_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Action {
    List,
    Screenshot,
    Apps,
    Pull,
    Push,
    Run,
}

/// The tool's declaration.
pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"computer",
        "description":"Work with the user's own linked computers over their OpenAgents host connection. Actions: list (the computers and whether each is connected), screenshot (a PNG of a computer's screen, or of an Android device attached to it; set look to see it yourself), apps (the windows open on its screen), pull (copy a file from it to here), push (copy a file from here to it; it never replaces a file unless overwrite is set, which asks the user), run (one shell command on it; read-only commands run, anything that changes the computer asks the user first). Paths on the computer are absolute or start with ~/. Files are at most 256 MiB and are checked by SHA-256 both ways. Use only for what the user asked.",
        "parameters":{"type":"object","properties":{
            "action":{"type":"string","enum":["list","screenshot","apps","pull","push","run"]},
            "computer":{"type":"string","maxLength":200,"description":"The computer: its name from list, an alias, or its key. Every action but list needs it."},
            "command":{"type":"string","minLength":1,"maxLength":8192,"description":"run: the shell command."},
            "remote":{"type":"string","maxLength":4096,"description":"pull/push: the path on the computer."},
            "local":{"type":"string","maxLength":4096,"description":"pull/push: the path here, relative to the working directory."},
            "overwrite":{"type":"boolean","description":"push/pull: replace an existing file. A push with it asks the user."},
            "screen":{"type":"string","maxLength":128,"description":"screenshot: a screen name (a Wayland output such as DP-1, an X11 display such as :0, or a macOS display number)."},
            "android":{"type":"boolean","description":"screenshot: capture an Android device attached to the computer over adb."},
            "serial":{"type":"string","maxLength":128,"description":"screenshot: the Android device's serial, when more than one is attached."},
            "look":{"type":"boolean","description":"screenshot: also send the image to you so you can see it (needs a model that reads images; at most 4 MiB)."},
            "timeout_seconds":{"type":"integer","minimum":1,"maximum":600,"description":"run: how long the command may take."}
        },"required":["action"],"additionalProperties":false}
    }})
}

/// What the model reads about the tool each turn.
pub fn instructions() -> &'static str {
    "The computer tool reaches the user's own linked computers through their OpenAgents host: list them, take a screenshot, list open apps, copy files with pull and push, and run a command. The host checks this device's rights on every action. Commands that change the other computer, and pushes that replace a file, wait for the user's approval; a rejected action stays rejected. Use list first when you do not know the computer's name.\n"
}

/// The command a person would type for the same action, for the
/// approval question and for a refusal that hands it to them.
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

/// What the owner must approve before `arguments` runs, or `None`.
/// `Err` is a refusal that no answer changes.
fn needs_approval(arguments: &ArgumentsView<'_>) -> Result<Option<String>, String> {
    match arguments.action {
        Action::Run => {
            use coder::task::agent::{Effect, effect};
            match effect(arguments.command.unwrap_or_default()) {
                Effect::ReadOnly => Ok(None),
                Effect::Approval(why) => Ok(Some(why)),
                Effect::Denied(why) => Err(format!(
                    "The host refuses this command ({why}). Do not try it another way."
                )),
            }
        }
        Action::Push if arguments.overwrite => {
            Ok(Some("it replaces a file on that computer".to_owned()))
        }
        _ => Ok(None),
    }
}

/// The parts of the arguments the approval policy reads.
struct ArgumentsView<'a> {
    action: Action,
    command: Option<&'a str>,
    overwrite: bool,
}

fn required<'a>(value: Option<&'a String>, name: &str, action: &str) -> Result<&'a str, String> {
    value
        .map(String::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| format!("{action} needs {name}."))
}

/// The `openagents` words for one action.
fn words(arguments: &Arguments, cwd: &Path, captures: &Path) -> Result<Vec<String>, String> {
    let host = || required(arguments.computer.as_ref(), "computer", "This action");
    let mut words = vec!["computer".to_owned()];
    match arguments.action {
        Action::List => words.push("list".into()),
        Action::Screenshot => {
            words.extend(["screenshot".into(), host()?.to_owned()]);
            if arguments.android {
                words.push("--android".into());
                if let Some(serial) = &arguments.serial {
                    words.extend(["--serial".into(), serial.clone()]);
                }
            } else if let Some(screen) = &arguments.screen {
                words.extend(["--screen".into(), screen.clone()]);
            }
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let out = captures.join(format!("screenshot-{nanos}.png"));
            words.extend(["--out".into(), out.display().to_string()]);
        }
        Action::Apps => words.extend(["apps".into(), host()?.to_owned()]),
        Action::Pull | Action::Push => {
            let remote = required(arguments.remote.as_ref(), "remote", "pull and push")?;
            let local = required(arguments.local.as_ref(), "local", "pull and push")?;
            let local = cwd.join(local).display().to_string();
            if arguments.action == Action::Pull {
                words.extend(["pull".into(), host()?.to_owned(), remote.into(), local]);
            } else {
                words.extend(["push".into(), host()?.to_owned(), local, remote.into()]);
            }
            if arguments.overwrite {
                words.push("--overwrite".into());
            }
        }
        Action::Run => {
            let command = required(arguments.command.as_ref(), "command", "run")?;
            words.extend([
                "exec".into(),
                host()?.to_owned(),
                "--timeout".into(),
                arguments
                    .timeout_seconds
                    .unwrap_or(RUN_TIMEOUT)
                    .clamp(1, RUN_TIMEOUT)
                    .to_string(),
                "--".into(),
                "sh".into(),
                "-c".into(),
                command.into(),
            ]);
        }
    }
    Ok(words)
}

/// Where screenshots taken for the model go on this computer.
fn captures_dir() -> PathBuf {
    std::env::temp_dir().join("openagents-computer-captures")
}

/// Run one `computer` call.
///
/// # Errors
/// Invalid arguments, a refusal (by policy or by the owner), or the CLI's
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
    execute_with(&program, arguments, cwd, desk, cancel, emit).await
}

pub(crate) async fn execute_with(
    program: &Path,
    arguments: Value,
    cwd: &Path,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let arguments: Arguments = serde_json::from_value(arguments)
        .map_err(|error| format!("computer arguments do not match the declared schema: {error}"))?;
    let captures = captures_dir();
    if arguments.action == Action::Screenshot {
        std::fs::create_dir_all(&captures)
            .map_err(|error| format!("{}: {error}", captures.display()))?;
    }
    let words = words(&arguments, cwd, &captures)?;
    let view = ArgumentsView {
        action: arguments.action,
        command: arguments.command.as_deref(),
        overwrite: arguments.overwrite,
    };
    if let Some(why) = needs_approval(&view)? {
        let host = arguments.computer.clone().unwrap_or_default();
        let action = match arguments.action {
            Action::Run => arguments.command.clone().unwrap_or_default(),
            _ => shown(&words),
        };
        let confirmed = match desk.or(crate::approval::desk().as_deref()) {
            Some(desk) => desk.confirm_computer(&host, &action, &why, cancel).await,
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
                "The user rejected this action on their computer. Do not run it or \
                        work around it; continue without it or say what you would have done."
                    .into(),
            );
        }
    }
    let result =
        crate::bundled_runtime::cli_checked(program, &words, cwd, cancel, &mut |event| emit(event))
            .await?;
    Ok(shape(arguments.action, arguments.look, result))
}

/// The tool's answer from the CLI's run: its JSON when it printed one, and
/// for a screenshot the file to look at.
fn shape(action: Action, look: bool, result: Value) -> Value {
    let stdout = result["stdout"].as_str().unwrap_or_default();
    let parsed: Option<Value> = serde_json::from_str(stdout.trim()).ok();
    let exit = result["exit"].as_i64();
    let mut answer = match parsed {
        Some(value) => json!({"exit": exit, "result": value}),
        None => json!({"exit": exit, "stdout": stdout}),
    };
    if let Some(stderr) = result.get("stderr").filter(|value| !value.is_null()) {
        let text = stderr
            .as_str()
            .map_or_else(|| stderr.to_string(), str::to_owned);
        if !text.trim().is_empty() {
            answer["stderr"] = json!(text);
        }
    }
    if action == Action::Screenshot
        && exit == Some(0)
        && let Some(path) = answer["result"]["path"].as_str()
    {
        let size = answer["result"]["size"].as_u64().unwrap_or(u64::MAX);
        if look && size <= LOOK_MAX_BYTES {
            answer["look"] = json!({"path": path, "media_type": "image/png"});
        } else if look {
            answer["note"] = json!(format!(
                "The screenshot is {size} bytes, over the {LOOK_MAX_BYTES} byte limit for viewing; it is saved at {path}."
            ));
        }
    }
    answer
}

/// The user message that shows a screenshot the `computer` tool took to
/// the model, when its output asks for one; `None` otherwise. Read at the
/// end of a tool batch, after every tool message.
pub fn look_message(output: &Value) -> Option<Value> {
    let path = output.get("look")?.get("path")?.as_str()?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() as u64 > LOOK_MAX_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    Some(json!({"role":"user","content":[
        {"type":"text","text":format!("The screenshot the computer tool took ({path}).")},
        {"type":"image_url","image_url":{"url":url}}
    ]}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(action: Action, command: Option<&str>, overwrite: bool) -> ArgumentsView<'_> {
        ArgumentsView {
            action,
            command,
            overwrite,
        }
    }

    #[test]
    fn reads_run_changes_ask_and_denied_commands_are_refused() {
        assert_eq!(
            needs_approval(&view(Action::Run, Some("ls -la ~"), false)),
            Ok(None)
        );
        assert_eq!(
            needs_approval(&view(Action::Run, Some("df -h"), false)),
            Ok(None)
        );
        assert!(matches!(
            needs_approval(&view(Action::Run, Some("rm notes.txt"), false)),
            Ok(Some(_))
        ));
        assert!(needs_approval(&view(Action::Run, Some("rm -rf /"), false)).is_err());
        assert_eq!(needs_approval(&view(Action::Push, None, false)), Ok(None));
        assert!(matches!(
            needs_approval(&view(Action::Push, None, true)),
            Ok(Some(_))
        ));
        assert_eq!(
            needs_approval(&view(Action::Screenshot, None, false)),
            Ok(None)
        );
        assert_eq!(needs_approval(&view(Action::Pull, None, true)), Ok(None));
    }

    #[test]
    fn actions_become_the_cli_words_a_person_would_type() {
        let parse = |value: Value| serde_json::from_value::<Arguments>(value).unwrap();
        let cwd = Path::new("/work");
        let captures = Path::new("/tmp/c");
        let run = words(
            &parse(json!({"action":"run","computer":"coderos","command":"uptime"})),
            cwd,
            captures,
        )
        .unwrap();
        assert_eq!(
            run,
            [
                "computer",
                "exec",
                "coderos",
                "--timeout",
                "600",
                "--",
                "sh",
                "-c",
                "uptime"
            ]
        );
        let push = words(
            &parse(json!({"action":"push","computer":"c","local":"a.txt","remote":"~/a.txt"})),
            cwd,
            captures,
        )
        .unwrap();
        assert_eq!(push, ["computer", "push", "c", "/work/a.txt", "~/a.txt"]);
        let shot = words(
            &parse(json!({"action":"screenshot","computer":"c","android":true})),
            cwd,
            captures,
        )
        .unwrap();
        assert_eq!(&shot[..4], ["computer", "screenshot", "c", "--android"]);
        assert!(
            words(&parse(json!({"action":"apps"})), cwd, captures)
                .unwrap_err()
                .contains("computer")
        );
        assert!(serde_json::from_value::<Arguments>(json!({"action":"format"})).is_err());
    }

    #[tokio::test]
    async fn a_change_without_anyone_to_ask_is_refused_with_the_command_to_run() {
        let _lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cancel = Arc::new(AtomicBool::new(false));
        let error = execute_with(
            Path::new("/nonexistent/openagents"),
            json!({"action":"run","computer":"coderos","command":"touch x"}),
            Path::new("/"),
            None,
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(error.contains("approval"), "{error}");
        assert!(
            error.contains("openagents computer exec coderos"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_rejected_change_does_not_run() {
        let desk = crate::approval::Desk::new();
        let answering = desk.clone();
        let answer = tokio::spawn(async move {
            loop {
                if let Some(event) = answering
                    .drain()
                    .into_iter()
                    .find(|event| event["event"] == "approval")
                {
                    assert_eq!(event["kind"], "computer");
                    assert_eq!(event["host"], "coderos");
                    assert_eq!(event["command"], "touch x");
                    answering
                        .answer(&format!("reject {}", event["id"]))
                        .unwrap();
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let error = execute_with(
            Path::new("/nonexistent/openagents"),
            json!({"action":"run","computer":"coderos","command":"touch x"}),
            Path::new("/"),
            Some(&desk),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        answer.await.unwrap();
        assert!(error.contains("rejected"), "{error}");
    }

    #[test]
    fn a_screenshot_to_look_at_is_shown_only_when_small_and_a_png() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.png");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let result = json!({"exit":0,"stdout":json!({"path":path.display().to_string(),"size":12}).to_string()});
        let shaped = shape(Action::Screenshot, true, result.clone());
        let message = look_message(&shaped).unwrap();
        assert!(
            message["content"][1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(look_message(&shape(Action::Screenshot, false, result)).is_none());
        std::fs::write(&path, b"not a png").unwrap();
        assert!(look_message(&shaped).is_none());
    }
}
