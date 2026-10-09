//! Durable local task requests, explicit execution, and independent evidence.

use std::io::{Read, Write};
use std::path::PathBuf;

use super::{self as task, Action, MAX_COMMAND_BYTES, Store};
use serde_json::{Value, json};

const EXIT_USAGE: u8 = 64;

pub const USAGE: &str = "\
Usage:
  openagents task submit --file COMMAND.json [--store DIRECTORY]
  openagents task cancel --file COMMAND.json [--store DIRECTORY]
  openagents task correct --file COMMAND.json [--store DIRECTORY]
  openagents task check TASK_ID [--store DIRECTORY]
  openagents task list [--json] [--store DIRECTORY]
  openagents task show TASK_ID [--store DIRECTORY]
  openagents task start --grant GRANT.json [--store DIRECTORY]
  openagents task execute --grant GRANT.json [--store DIRECTORY]
  openagents task recover TASK_ID [--store DIRECTORY]
  openagents task view TASK_ID [--limit 100] [--cursor JSON] [--store DIRECTORY]
  openagents task artifact TASK_ID --path RELATIVE_PATH [--store DIRECTORY]
  openagents task archive TASK_ID --reason TEXT [--store DIRECTORY]
  openagents task restore TASK_ID [--store DIRECTORY]
  openagents task resume TASK_ID [--store DIRECTORY]

Submit and cancel read the exact versioned command bytes from a file (or -
for stdin). Keep the same command ID and file bytes when retrying. Cancellation
requires the task's current expected_revision. Results are JSON, except list,
which prints one line per task unless --json is set.

The default store is ~/.openagents/tasks. Submission is inert. An explicit
execution grant admits the bounded-command adapter through start (detached host)
or execute (foreground host). Cancellation acknowledges a request; inspect the
execution result for confirmed stop. Recover records owner loss as unknown and
never reruns an effect. Model adapters can't be started this way.
Archive takes a finished or cancelled task off every device's task and chat
lists and deletes nothing; restore shows it again. Resume continues a task a
usage limit stopped from its resume point now, without waiting for the reset;
the auto-start policy otherwise resumes it once a provider has capacity.
See docs/coder/guides/tasks.md for command fixtures and recovery behavior.

Exit codes: 0 success, 1 store/command refusal, 64 invalid CLI usage.";

#[derive(Debug, PartialEq, Eq)]
enum Operation {
    Submit(String),
    Cancel(String),
    Correct(String),
    Check(String),
    List,
    Show(String),
    Execute(String),
    Start(String),
    Recover(String),
    View(String, Option<task::view::Cursor>, usize),
    Artifact(String, PathBuf),
    Archive(String, String),
    Restore(String),
    Resume(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    operation: Operation,
    store: Option<PathBuf>,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let (verb, rest) = arguments.split_first().ok_or("give a task subcommand")?;
    let mut file = None;
    let mut store = None;
    let mut grant = None;
    let mut cursor = None;
    let mut limit = None;
    let mut artifact_path = None;
    let mut reason = None;
    let mut id = None;
    let mut rest = rest.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--path" => {
                if artifact_path.is_some() {
                    return Err("give --path once".into());
                }
                artifact_path = Some(PathBuf::from(rest.next().ok_or("--path needs a value")?));
            }
            "--reason" => {
                if reason.is_some() {
                    return Err("give --reason once".into());
                }
                reason = Some(rest.next().ok_or("--reason needs a value")?.clone());
            }
            "--cursor" | "--limit" => {
                let value = rest
                    .next()
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                if argument == "--cursor" {
                    if cursor.is_some() || value.len() > 4096 {
                        return Err("invalid or repeated cursor".into());
                    }
                    cursor = Some(serde_json::from_str(value).map_err(|_| "invalid cursor JSON")?);
                } else {
                    if limit.is_some() {
                        return Err("give --limit only once".into());
                    }
                    limit = Some(value.parse::<usize>().map_err(|_| "invalid view limit")?);
                }
            }
            "--store" | "--file" | "--grant" => {
                let value = rest
                    .next()
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                let previous = if argument == "--store" {
                    store.replace(value.clone())
                } else if argument == "--grant" {
                    grant.replace(value.clone())
                } else {
                    file.replace(value.clone())
                };
                if previous.is_some() {
                    return Err(format!("give {argument} only once"));
                }
            }
            _ if (verb == "check"
                || verb == "show"
                || verb == "recover"
                || verb == "view"
                || verb == "artifact"
                || verb == "archive"
                || verb == "restore"
                || verb == "resume")
                && !argument.starts_with('-')
                && id.is_none() =>
            {
                id = Some(argument.clone());
            }
            _ => return Err(format!("unexpected task argument {argument}")),
        }
    }
    let operation = match verb.as_str() {
        "execute" if file.is_none() && id.is_none() => {
            Operation::Execute(grant.take().ok_or("execute needs --grant")?)
        }
        "start" if file.is_none() && id.is_none() => {
            Operation::Start(grant.take().ok_or("start needs --grant")?)
        }
        "view" if file.is_none() => Operation::View(
            id.ok_or("view needs a task ID")?,
            cursor.take(),
            limit.take().unwrap_or(100),
        ),
        "artifact" if file.is_none() => Operation::Artifact(
            id.ok_or("artifact needs a task ID")?,
            artifact_path.take().ok_or("artifact needs --path")?,
        ),
        "recover" if file.is_none() => Operation::Recover(id.ok_or("recover needs a task ID")?),
        "archive" if file.is_none() => Operation::Archive(
            id.ok_or("archive needs a task ID")?,
            reason.take().ok_or("archive needs --reason")?,
        ),
        "restore" if file.is_none() => Operation::Restore(id.ok_or("restore needs a task ID")?),
        "resume" if file.is_none() => Operation::Resume(id.ok_or("resume needs a task ID")?),
        "submit" => Operation::Submit(file.ok_or("submit needs --file")?),
        "cancel" => Operation::Cancel(file.ok_or("cancel needs --file")?),
        "correct" => Operation::Correct(file.ok_or("correct needs --file")?),
        "check" if file.is_none() => Operation::Check(id.ok_or("check needs a task ID")?),
        "list" if file.is_none() => Operation::List,
        "show" if file.is_none() => Operation::Show(id.ok_or("show needs a task ID")?),
        "list" | "show" => return Err("--file applies only to submit or cancel".into()),
        _ => return Err(format!("unknown task subcommand {verb}")),
    };
    if artifact_path.is_some() {
        return Err("--path applies only to artifact".into());
    }
    if reason.is_some() {
        return Err("--reason applies only to archive".into());
    }
    if cursor.is_some() || limit.is_some() {
        return Err("--cursor and --limit apply only to view".into());
    }
    if grant.is_some() {
        return Err("--grant applies only to start or execute".into());
    }
    Ok(Options {
        operation,
        store: store.map(PathBuf::from),
    })
}

fn failure(code: &str, message: impl std::fmt::Display) -> u8 {
    eprintln!(
        "{}",
        json!({"error": {"code": code, "message": message.to_string()}})
    );
    1
}

fn read_command(path: &str) -> std::io::Result<Vec<u8>> {
    let reader: Box<dyn Read> = if path == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path)?)
    };
    let mut bytes = Vec::new();
    reader
        .take(MAX_COMMAND_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Runs a task command; checking loads only the operator capability trust store.
pub async fn run(arguments: &[String]) -> u8 {
    let json = arguments.iter().any(|argument| argument == "--json");
    let arguments: Vec<String> = arguments
        .iter()
        .filter(|argument| *argument != "--json")
        .cloned()
        .collect();
    run_with_json(&arguments, json).await
}

/// Runs a task command with an explicit output format from the calling CLI.
pub async fn run_with_json(arguments: &[String], json: bool) -> u8 {
    if matches!(arguments, [argument] if argument == "--help" || argument == "-h") {
        println!("{USAGE}");
        return 0;
    }
    let options = match parse(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("coder task: {message}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    // Validate before opening a store. A wrong subcommand must never apply the
    // other action, and malformed input must not create an empty inbox.
    let bytes = match &options.operation {
        Operation::Submit(path) | Operation::Cancel(path) | Operation::Correct(path) => {
            let bytes = match read_command(path) {
                Ok(bytes) => bytes,
                Err(error) => return failure("read_command", error),
            };
            let command = match task::parse_command(&bytes) {
                Ok(command) => command,
                Err(error) => return failure(error.code(), error),
            };
            if !matches!(
                (&options.operation, &command.action),
                (Operation::Submit(_), Action::Submit { .. })
                    | (Operation::Cancel(_), Action::Cancel { .. })
                    | (Operation::Correct(_), Action::Correct { .. })
            ) {
                return failure(
                    "action_mismatch",
                    "command action differs from the subcommand",
                );
            }
            Some(bytes)
        }
        Operation::List
        | Operation::Show(_)
        | Operation::Recover(_)
        | Operation::Execute(_)
        | Operation::Start(_)
        | Operation::View(..)
        | Operation::Artifact(..)
        | Operation::Archive(..)
        | Operation::Restore(_)
        | Operation::Resume(_)
        | Operation::Check(_) => None,
    };
    let directory = match options.store {
        Some(path) => path,
        None => match std::env::var_os("HOME").filter(|home| !home.is_empty()) {
            Some(home) => PathBuf::from(home).join(".openagents/tasks"),
            None => return failure("configuration", "HOME is unset; give --store explicitly"),
        },
    };
    match &options.operation {
        Operation::Check(id) => {
            return match task::owner::check(&directory, id, &crate::capability::Trust::operator())
                .await
            {
                Ok(task) => output(&json!(task)),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::Execute(path) => {
            let bytes = match read_command(path) {
                Ok(bytes) => bytes,
                Err(error) => return failure("read_grant", error),
            };
            return match task::owner::execute(&directory, &bytes).await {
                Ok(task) => output(&json!(task)),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::Start(path) => return start_owner(&directory, path),
        Operation::Artifact(id, path) => {
            return match task::artifact::read(&directory, id, path) {
                Ok(bytes) => output(
                    &json!({"path": path, "digest": nostr::contracts::digest_bytes(&bytes), "bytes": bytes}),
                ),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::View(id, cursor, limit) => {
            return match task::view::read(&directory, id, cursor.as_ref(), *limit) {
                Ok(view) => output(&with_route(json!(view), &directory, id)),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::Archive(id, reason) => {
            return match task::archive::archive(
                &directory,
                id,
                reason,
                task::archive::By::Owner,
                task::autostart::unix_now(),
            ) {
                Ok(archived) => output(&json!({"task": id, "archived": archived})),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::Resume(id) => {
            return match task::resume::resume_now(&directory, id, task::autostart::unix_now()) {
                Ok((point, turn)) => {
                    output(&json!({"task": id, "turn": turn, "resume_point": point}))
                }
                Err(message) => failure("no_resume", message),
            };
        }
        Operation::Restore(id) => {
            return match task::archive::restore(&directory, id) {
                Ok(restored) => output(&json!({"task": id, "restored": restored})),
                Err(error) => failure(error.code(), error),
            };
        }
        Operation::Recover(id) => {
            return match task::owner::recover(&directory, id) {
                Ok(task) => output(&json!(task)),
                Err(error) => failure(error.code(), error),
            };
        }
        _ => (),
    }
    let mut store = match Store::open(&directory) {
        Ok(store) => store,
        Err(error) => return failure(error.code(), error),
    };
    let listing = matches!(options.operation, Operation::List);
    let result = match options.operation {
        Operation::Submit(_) | Operation::Cancel(_) | Operation::Correct(_) => store
            .apply(bytes.as_deref().expect("mutation input was validated"))
            .map(|receipt| json!(receipt)),
        Operation::List => store.list().map(|tasks| json!(tasks)),
        Operation::Show(id) => store.show(&id).map(|task| json!(task)),
        _ => unreachable!("owner operation dispatched above"),
    };
    // Lists stay compact on terminals and in pipes; full records require --json.
    if listing
        && !json
        && let Ok(Value::Array(tasks)) = &result
    {
        println!("{}", task_table(tasks));
        return 0;
    }
    match result {
        Ok(value) => output(&value),
        Err(error) => failure(error.code(), error),
    }
}

/// Detach the host, not the executor. The host itself owns supervision and its
/// retained owner lock. Starting a process is not an execution admission receipt.
fn start_owner(directory: &std::path::Path, grant: &str) -> u8 {
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    if grant == "-" {
        return failure(
            "configuration",
            "start needs a grant file, not standard input",
        );
    }
    let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
        let bytes = read_command(grant)?;
        let parsed = task::owner::Grant::parse(&bytes)?;
        let store = Store::open(directory)?;
        let task = store.show(&parsed.task_id)?;
        if task.status != task::Status::Queued || task.run.is_some() {
            return Err(task::Error::InvalidTransition.into());
        }
        if parsed.intent_digest != task.intent_digest || parsed.expected_revision != task.revision {
            return Err(task::Error::RevisionMismatch.into());
        }
        let directory = directory.canonicalize()?;
        // Retain the exact authority bytes before a detached process reads them.
        let launch = format!(
            "launch-{}-{}-{}",
            task.task_id,
            std::process::id(),
            atif::now_ms()
        );
        let grant = directory.join(format!("{launch}.grant.json"));
        let mut saved =
            crate::private::file(std::fs::OpenOptions::new().write(true).create_new(true))
                .open(&grant)?;
        saved.write_all(&bytes)?;
        saved.sync_all()?;
        let diagnostic_path = directory.join(format!("{launch}.jsonl"));
        let diagnostic =
            crate::private::file(std::fs::OpenOptions::new().write(true).create_new(true))
                .open(&diagnostic_path)?;
        // Windows cannot flush a directory; NTFS journals the new names.
        #[cfg(unix)]
        std::fs::File::open(&directory)?.sync_all()?;
        let mut command = Command::new(std::env::current_exe()?);
        #[cfg(unix)]
        command.env_clear().env("PATH", "/usr/bin:/bin");
        #[cfg(windows)]
        {
            let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
            let mut path = std::path::PathBuf::from(&root);
            path.push("System32");
            command
                .env_clear()
                .env("SystemRoot", &root)
                .env("PATH", path);
        }
        command
            .args(["task", "execute", "--store"])
            .arg(&directory)
            .arg("--grant")
            .arg(grant)
            .stdin(Stdio::null())
            .stdout(Stdio::from(diagnostic.try_clone()?))
            .stderr(Stdio::from(diagnostic));
        // SAFETY: setsid is async-signal-safe and uses no parent-memory state.
        #[cfg(unix)]
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        // A process group of its own, with no console window.
        #[cfg(windows)]
        command.creation_flags(0x0000_0200 | 0x0800_0000);
        let child = command.spawn()?;
        Ok(
            json!({"task_id": task.task_id, "owner_process": child.id(), "admission": "pending", "diagnostic_path": diagnostic_path,
            "message": "The task runs in the background. Check the task for its progress and results."}),
        )
    })();
    match result {
        Ok(value) => output(&value),
        Err(error) => failure("start_owner", error),
    }
}

/// `task list` for a person: one line per task, newest last.
fn task_table(tasks: &[Value]) -> String {
    if tasks.is_empty() {
        return "No tasks.".to_owned();
    }
    let text = |value: &Value| value.as_str().unwrap_or("-").to_owned();
    let mut lines = vec![format!(
        "{} task{}. Full record: openagents task show ID.",
        tasks.len(),
        if tasks.len() == 1 { "" } else { "s" }
    )];
    for task in tasks {
        let id: String = text(&task["task_id"]).chars().take(12).collect();
        let title = task["intent"]["title"]
            .as_str()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| task["intent"]["prompt"].as_str())
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let title: String = if title.chars().count() > 60 {
            format!("{}…", title.chars().take(59).collect::<String>())
        } else {
            title
        };
        lines.push(format!(
            "{id}  {:<9} {:<10} {title}",
            text(&task["status"]),
            text(&task["execution"]),
        ));
    }
    lines.join("\n")
}

/// Adds the route the task belongs to (#10698), read from the route
/// journal beside the store, so a workbench pane shows the route's
/// identities, placement, payers, and cost state beside the run. Reading
/// never writes the journal; a task no route started has no `route`.
pub(crate) fn with_route(mut view: Value, store: &std::path::Path, task: &str) -> Value {
    if let Some(record) = openagents_chat::route::Journal::beside(store).find_task(task)
        && let Value::Object(map) = &mut view
    {
        map.insert(
            "route".into(),
            json!(route_contract::view::RouteView::of(&record)),
        );
    }
    view
}

fn output(value: &Value) -> u8 {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match serde_json::to_writer(&mut out, value)
        .map_err(std::io::Error::other)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
    {
        Ok(()) => 0,
        Err(error) => failure(
            "output",
            format!("output failed; retry the identical command: {error}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_reads_task_list_as_a_short_table() {
        assert_eq!(task_table(&[]), "No tasks.");
        let task = serde_json::json!({
            "task_id": "0123456789abcdef",
            "status": "finished",
            "execution": "succeeded",
            "intent": {"title": "", "prompt": "Fix the   failing\ntest in parser.rs"},
        });
        let text = task_table(&[task]);
        assert!(
            text.starts_with("1 task. Full record: openagents task show ID."),
            "{text}"
        );
        assert!(
            text.contains("0123456789ab  finished  succeeded  Fix the failing test in parser.rs"),
            "{text}"
        );
        assert!(!text.contains("intent"), "{text}");
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn rejects_ambiguous_or_unsupported_operations() {
        for values in [
            vec![],
            vec!["run", "task-1"],
            vec!["cancel", "task-1"],
            vec!["show"],
            vec!["show", "a", "b"],
            vec!["list", "--file", "x"],
            vec!["submit", "--file", "a", "--file", "b"],
            vec!["list", "--store", "a", "--store", "b"],
            vec!["archive", "task-1"],
            vec!["archive", "--reason", "Test chat"],
            vec!["show", "task-1", "--reason", "Test chat"],
        ] {
            assert!(parse(&args(&values)).is_err(), "{values:?}");
        }
    }

    #[test]
    fn parses_stdin_and_explicit_store() {
        assert_eq!(
            parse(&args(&["submit", "--store", "/tmp/tasks", "--file", "-"])).unwrap(),
            Options {
                operation: Operation::Submit("-".into()),
                store: Some("/tmp/tasks".into())
            }
        );
    }

    #[test]
    fn parses_archive_and_restore() {
        assert_eq!(
            parse(&args(&["archive", "task-1", "--reason", "Test chat"])).unwrap(),
            Options {
                operation: Operation::Archive("task-1".into(), "Test chat".into()),
                store: None
            }
        );
        assert_eq!(
            parse(&args(&["restore", "task-1"])).unwrap().operation,
            Operation::Restore("task-1".into())
        );
    }
}
