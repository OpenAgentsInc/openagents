//! Command hooks (#11500): shell hooks a project or the person configures,
//! registered into the SDK and composed under the same rules as every
//! other hook, so none of them gets past a floor.
//!
//! **Config.** Claude Code's `hooks` shape, which Codex's `hooks` table
//! mirrors:
//!
//! ```json
//! {"hooks": {"PreToolUse": [
//!   {"matcher": "Bash", "hooks": [{"type": "command", "command": "./check.sh", "timeout": 30}]}
//! ]}}
//! ```
//!
//! A project's file is `.openagents/hooks.json` in the checkout; the
//! person's is `~/.openagents/hooks.json`. (Not `.claude/settings.json`:
//! Claude Code runs those itself, and they would fire twice.)
//!
//! **Running.** The command runs under `sh -c` in the session's directory
//! with Claude Code's hook input on standard input, and is answered as
//! Claude Code answers it: exit 0 reads standard output (the JSON reply,
//! or plain text as context on `UserPromptSubmit` and `SessionStart`);
//! exit 2 blocks with standard error as the reason; any other exit, or no
//! answer in its timeout, is a failure, which counts as no opinion.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::events::{Call, Event};
use super::outcome::{Context, HookOutcome, Permission, Stop};
use super::registry::{Hook, Registry, Source};

/// Where a project's command hooks live, under its checkout.
pub const PROJECT_FILE: &str = ".openagents/hooks.json";
/// Where the person's command hooks live, under their home.
pub const PERSON_FILE: &str = ".openagents/hooks.json";
/// A command's timeout unless it names one, as in Claude Code.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// One configured shell hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandHook {
    pub event: Event,
    pub matcher: Option<String>,
    pub command: String,
    pub timeout: Duration,
}

/// The command hooks in a config (`{"hooks": {...}}`, or the `hooks`
/// object itself). Entries other than `type: "command"` are skipped.
///
/// # Errors
///
/// An event name Coder doesn't know, or a malformed entry.
pub fn parse(config: &Value) -> Result<Vec<CommandHook>, String> {
    let hooks = config.get("hooks").unwrap_or(config);
    let Some(events) = hooks.as_object() else {
        return Err("The hooks config should be an object of events.".into());
    };
    let mut parsed = Vec::new();
    for (name, groups) in events {
        let event = Event::from_name(name)
            .ok_or_else(|| format!("{name} isn't a hook event Coder knows."))?;
        for group in groups
            .as_array()
            .ok_or_else(|| format!("The {name} hooks should be a list."))?
        {
            let matcher = group["matcher"]
                .as_str()
                .filter(|matcher| !matcher.is_empty() && *matcher != "*")
                .map(str::to_owned);
            for entry in group["hooks"].as_array().into_iter().flatten() {
                if entry["type"].as_str().unwrap_or("command") != "command" {
                    continue;
                }
                let command = entry["command"]
                    .as_str()
                    .filter(|command| !command.trim().is_empty())
                    .ok_or_else(|| format!("A {name} hook has no command."))?;
                parsed.push(CommandHook {
                    event,
                    matcher: matcher.clone(),
                    command: command.to_owned(),
                    timeout: entry["timeout"]
                        .as_u64()
                        .map_or(DEFAULT_TIMEOUT, Duration::from_secs),
                });
            }
        }
    }
    Ok(parsed)
}

/// Whose hooks a file holds, for the hook's ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Project,
    Person,
}

impl Origin {
    fn word(self) -> &'static str {
        match self {
            Origin::Project => "project",
            Origin::Person => "person",
        }
    }
}

/// The command hooks of the person (`home`) and the project (`checkout`),
/// person first. A file that is missing adds none; one that can't be read
/// is reported and adds none.
#[must_use]
pub fn load(checkout: &Path, home: Option<&Path>) -> (Vec<(Origin, CommandHook)>, Vec<String>) {
    let mut hooks = Vec::new();
    let mut problems = Vec::new();
    let files = home
        .map(|home| (Origin::Person, home.join(PERSON_FILE)))
        .into_iter()
        .chain(std::iter::once((
            Origin::Project,
            checkout.join(PROJECT_FILE),
        )));
    for (origin, path) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str::<Value>(&text)
            .map_err(|error| error.to_string())
            .and_then(|config| parse(&config))
        {
            Ok(parsed) => hooks.extend(parsed.into_iter().map(|hook| (origin, hook))),
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    (hooks, problems)
}

/// Registers `hooks` after whatever `registry` holds, as command hooks
/// running in `cwd` for `session_id`. Returns the problems, if any.
pub fn register(
    registry: &mut Registry,
    hooks: &[(Origin, CommandHook)],
    cwd: &Path,
    session_id: &str,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (index, (origin, hook)) in hooks.iter().enumerate() {
        let id = format!("{} hook {}", origin.word(), index + 1);
        if let Err(error) = registry.register(command_hook(&id, hook, cwd, session_id)) {
            problems.push(error);
        }
    }
    problems
}

/// One configured shell hook as an SDK hook.
#[must_use]
pub fn command_hook(id: &str, hook: &CommandHook, cwd: &Path, session_id: &str) -> Hook {
    let command = hook.command.clone();
    let cwd = cwd.to_path_buf();
    let session_id = session_id.to_owned();
    let timeout = hook.timeout;
    let source = id.to_owned();
    let mut built = Hook::new(id, hook.event, Source::Command, move |call: &Call| {
        let input = hook_input(call, &cwd, &session_id);
        let ran = run(&command, &input, &cwd, timeout)?;
        outcome(call.event, &source, &ran)
    })
    .timeout(hook.timeout + Duration::from_millis(500));
    built.matcher.clone_from(&hook.matcher);
    built
}

/// Claude Code's hook input for `call`.
#[must_use]
pub fn hook_input(call: &Call, cwd: &Path, session_id: &str) -> Value {
    let mut input = Map::new();
    input.insert("session_id".into(), json!(session_id));
    input.insert("cwd".into(), json!(cwd.display().to_string()));
    input.insert("hook_event_name".into(), json!(call.event.name()));
    if let Some(tool) = &call.tool {
        input.insert("tool_name".into(), json!(tool));
    }
    match call.event {
        Event::PreToolUse => {
            input.insert("tool_input".into(), call.input.clone());
        }
        Event::PostToolUse => {
            input.insert("tool_input".into(), call.input["tool_input"].clone());
            input.insert("tool_response".into(), call.input["tool_response"].clone());
        }
        _ => {
            if let Some(fields) = call.input.as_object() {
                for (key, value) in fields {
                    input.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }
    }
    Value::Object(input)
}

/// What a command printed, and how it exited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `command` under `sh -c` in `cwd` with `input` on standard input,
/// killed at `timeout`.
///
/// # Errors
///
/// It couldn't start, or ran past its timeout.
pub fn run(command: &str, input: &Value, cwd: &Path, timeout: Duration) -> Result<Ran, String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("The hook {command} couldn't start: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let text = input.to_string();
        // A hook that doesn't read its input closes the pipe early.
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = reader(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = reader(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "The hook {command} didn't answer in {} seconds.",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(error.to_string()),
        }
    };
    Ok(Ran {
        code: status.code(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// A command's answer, read as Claude Code reads it.
///
/// # Errors
///
/// A failed command (an exit other than 0 or 2), which counts as no
/// opinion.
pub fn outcome(event: Event, source: &str, ran: &Ran) -> Result<HookOutcome, String> {
    match ran.code {
        Some(0) => Ok(reply(event, source, ran.stdout.trim())),
        Some(2) => {
            let reason = ran.stderr.trim();
            let reason = if reason.is_empty() {
                "A project hook stopped this."
            } else {
                reason
            };
            Ok(match event {
                Event::PreToolUse | Event::UserPromptSubmit => HookOutcome::deny(reason),
                Event::Stop | Event::SubagentStop => HookOutcome::stopping(Stop::Block {
                    reason: reason.to_owned(),
                }),
                Event::PostToolUse | Event::PostToolBatch => {
                    HookOutcome::context(Context::new(source, reason, 0))
                }
                _ => HookOutcome::none(),
            })
        }
        code => Err(format!(
            "The hook failed{}: {}",
            code.map(|code| format!(" with exit code {code}"))
                .unwrap_or_default(),
            ran.stderr.trim()
        )),
    }
}

/// An exit-0 reply: Claude Code's JSON output, or plain text.
fn reply(event: Event, source: &str, stdout: &str) -> HookOutcome {
    let Ok(Value::Object(output)) = serde_json::from_str::<Value>(stdout) else {
        // Plain text is context where Claude Code reads it as context.
        return if !stdout.is_empty()
            && matches!(event, Event::UserPromptSubmit | Event::SessionStart)
        {
            HookOutcome::context(Context::new(source, stdout, 0))
        } else {
            HookOutcome::none()
        };
    };
    let mut outcome = HookOutcome::none();
    let specific = output
        .get("hookSpecificOutput")
        .cloned()
        .unwrap_or(Value::Null);
    let reason = |key: &str, from: &Value| from[key].as_str().unwrap_or_default().to_owned();
    if event == Event::PreToolUse {
        let why = reason("permissionDecisionReason", &specific);
        outcome.permission = match specific["permissionDecision"].as_str() {
            Some("allow") => Permission::Allow,
            Some("deny") => Permission::deny(&why),
            Some("ask") => Permission::ask(&why),
            _ => match output.get("decision").and_then(Value::as_str) {
                // Claude Code's older words.
                Some("approve") => Permission::Allow,
                Some("block") => {
                    Permission::deny(&reason("reason", &Value::Object(output.clone())))
                }
                _ => Permission::NoOpinion,
            },
        };
        outcome.rewrite = specific
            .get("updatedInput")
            .filter(|v| !v.is_null())
            .cloned();
    }
    let decision = output.get("decision").and_then(Value::as_str);
    let why = output
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    match (event, decision) {
        (Event::Stop | Event::SubagentStop, Some("block")) => {
            outcome.stop = Stop::Block { reason: why };
        }
        (Event::UserPromptSubmit, Some("block")) => outcome.permission = Permission::deny(&why),
        // A blocked tool result is fed back to the agent.
        (Event::PostToolUse | Event::PostToolBatch, Some("block")) if !why.is_empty() => {
            outcome.context = Some(Context::new(source, &why, 0));
        }
        _ => {}
    }
    if let Some(text) = specific["additionalContext"]
        .as_str()
        .filter(|t| !t.is_empty())
    {
        let text = match outcome.context.take() {
            Some(earlier) => format!("{}\n{text}", earlier.text),
            None => text.to_owned(),
        };
        outcome.context = Some(Context::new(source, &text, 0));
    }
    if output.get("continue") == Some(&Value::Bool(false)) {
        outcome.stop = Stop::End;
    }
    outcome
}

/// The person's home, for [`load`].
#[must_use]
pub fn home() -> Option<PathBuf> {
    user_home::home_os().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::super::compose::{Budget, Runner};
    use super::super::scripted::{RECORDED_SESSION_SCHEMA, ScriptedSession};
    use super::*;

    #[test]
    fn the_claude_and_codex_config_shape_parses() {
        let config = json!({"hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "./check.sh", "timeout": 5},
                {"type": "prompt", "prompt": "skipped"}]}],
            "Stop": [{"hooks": [{"type": "command", "command": "./stop.sh"}]}],
        }});
        let hooks = parse(&config).unwrap();
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].event, Event::PreToolUse);
        assert_eq!(hooks[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(hooks[0].timeout, Duration::from_secs(5));
        assert_eq!(hooks[1].event, Event::Stop);
        assert_eq!(hooks[1].timeout, DEFAULT_TIMEOUT);
        assert!(parse(&json!({"hooks": {"Mystery": []}})).is_err());
    }

    #[test]
    fn replies_are_read_as_claude_code_reads_them() {
        let ran = |code, stdout: &str, stderr: &str| Ran {
            code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
        };
        let deny = outcome(Event::PreToolUse, "p", &ran(2, "", "no force pushes")).unwrap();
        assert_eq!(deny.permission, Permission::deny("no force pushes"));
        let json_reply = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"ls -1"},"additionalContext":"listed"}}"#;
        let rewrite = outcome(Event::PreToolUse, "p", &ran(0, json_reply, "")).unwrap();
        assert_eq!(rewrite.permission, Permission::Allow);
        assert_eq!(rewrite.rewrite, Some(json!({"command": "ls -1"})));
        assert_eq!(rewrite.context.unwrap().text, "listed");
        let stop = outcome(
            Event::Stop,
            "p",
            &ran(0, r#"{"decision":"block","reason":"run the tests"}"#, ""),
        )
        .unwrap();
        assert_eq!(
            stop.stop,
            Stop::Block {
                reason: "run the tests".into()
            }
        );
        let text = outcome(Event::SessionStart, "p", &ran(0, "branch: main", "")).unwrap();
        assert_eq!(text.context.unwrap().text, "branch: main");
        let end = outcome(
            Event::PostToolUse,
            "p",
            &ran(0, r#"{"continue":false}"#, ""),
        )
        .unwrap();
        assert_eq!(end.stop, Stop::End);
        assert!(outcome(Event::PreToolUse, "p", &ran(1, "", "boom")).is_err());
    }

    /// A project's shell hook and a built-in floor on one `PreToolUse`, in
    /// a scripted session: the project hook allows and rewrites, the floor
    /// judges the rewritten command, and a denial the hook can't lift.
    #[test]
    fn a_project_command_hook_and_a_floor_compose_in_a_scripted_session() {
        let checkout = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join(".openagents")).unwrap();
        // The project hook: allow everything, and add --dry-run to deploys.
        let script = r#"input=$(cat)
case "$input" in
  *deploy*) printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"deploy --dry-run --prod"}}}' ;;
  *) printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"}}' ;;
esac"#;
        std::fs::write(checkout.path().join("hook.sh"), script).unwrap();
        std::fs::write(
            checkout.path().join(PROJECT_FILE),
            json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "sh hook.sh", "timeout": 10}]}]}})
            .to_string(),
        )
        .unwrap();
        let (hooks, problems) = load(checkout.path(), None);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(hooks.len(), 1);

        let mut registry = Registry::new();
        registry
            .register(
                Hook::floor(
                    "floors",
                    Event::PreToolUse,
                    Permission::ask("confirm"),
                    |call: &Call| {
                        let command = call.input["command"].as_str().unwrap_or_default();
                        Ok(if command.contains("--prod") {
                            HookOutcome::deny("A production deploy is the person's.")
                        } else {
                            HookOutcome::none()
                        })
                    },
                )
                .matching("Bash"),
            )
            .unwrap();
        assert!(register(&mut registry, &hooks, checkout.path(), "s-1").is_empty());

        let session = ScriptedSession::from_json(
            &json!({
                "schema": RECORDED_SESSION_SCHEMA,
                "events": [
                    {"event": "PreToolUse", "tool": "Bash", "input": {"command": "ls"}},
                    {"event": "PreToolUse", "tool": "Bash", "input": {"command": "deploy"}},
                ],
            })
            .to_string(),
        )
        .unwrap();
        let mut runner = Runner::new(registry, Budget::default());
        let replay = session.replay(&mut runner).unwrap();
        // The project hook's allow stands where no floor objects.
        assert_eq!(replay.applied[0].permission, Permission::Allow);
        assert_eq!(
            replay.applied[0].decided_by.as_deref(),
            Some("project hook 1")
        );
        // It rewrote the deploy; the floor judged the rewrite and denied,
        // over the hook's allow.
        assert_eq!(replay.applied[1].rewritten_by, ["project hook 1"]);
        assert_eq!(
            replay.applied[1].input["command"],
            "deploy --dry-run --prod"
        );
        assert_eq!(
            replay.applied[1].permission,
            Permission::deny("A production deploy is the person's.")
        );
        assert_eq!(replay.applied[1].decided_by.as_deref(), Some("floors"));
    }

    #[test]
    fn a_slow_command_is_killed_and_counts_as_no_opinion() {
        let dir = tempfile::tempdir().unwrap();
        let slow = CommandHook {
            event: Event::PreToolUse,
            matcher: None,
            command: "sleep 5".into(),
            timeout: Duration::from_millis(200),
        };
        let mut registry = Registry::new();
        registry
            .register(command_hook("slow", &slow, dir.path(), "s"))
            .unwrap();
        let mut runner = Runner::new(registry, Budget::default());
        let started = Instant::now();
        let applied = runner.fire(&Call::tool(
            Event::PreToolUse,
            "Bash",
            json!({"command": "ls"}),
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(applied.permission, Permission::NoOpinion);
        assert_eq!(applied.failures.len(), 1);
    }
}
