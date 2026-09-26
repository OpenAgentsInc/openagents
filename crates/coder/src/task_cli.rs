//! The local task inbox. Its commands record requests without starting an agent.

use std::io::{Read, Write};
use std::path::PathBuf;

use coder::task::{self, Action, MAX_COMMAND_BYTES, Store};
use serde_json::{Value, json};

const USAGE: &str = "\
Usage:
  coder task submit --file COMMAND.json [--store DIRECTORY]
  coder task cancel --file COMMAND.json [--store DIRECTORY]
  coder task list [--store DIRECTORY]
  coder task show TASK_ID [--store DIRECTORY]

Submit and cancel read the exact versioned command bytes from a file (or -
for stdin). Keep the same command ID and file bytes when retrying. Cancellation
requires the queued task's current expected_revision. Results are JSON.

The default store is ~/.openagents/tasks. This inbox records requests only:
no model, executor, shell, network, or paid operation runs. Requested adapter
and model settings are inert intent, not validated execution configuration.
See docs/coder/guides/tasks.md for command fixtures and recovery behavior.

Exit codes: 0 success, 1 store/command refusal, 64 invalid CLI usage.";

#[derive(Debug, PartialEq, Eq)]
enum Operation {
    Submit(String),
    Cancel(String),
    List,
    Show(String),
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
    let mut id = None;
    let mut rest = rest.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--store" | "--file" => {
                let value = rest
                    .next()
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                let previous = if argument == "--store" {
                    store.replace(value.clone())
                } else {
                    file.replace(value.clone())
                };
                if previous.is_some() {
                    return Err(format!("give {argument} only once"));
                }
            }
            _ if verb == "show" && !argument.starts_with('-') && id.is_none() => {
                id = Some(argument.clone());
            }
            _ => return Err(format!("unexpected task argument {argument}")),
        }
    }
    let operation = match verb.as_str() {
        "submit" => Operation::Submit(file.ok_or("submit needs --file")?),
        "cancel" => Operation::Cancel(file.ok_or("cancel needs --file")?),
        "list" if file.is_none() => Operation::List,
        "show" if file.is_none() => Operation::Show(id.ok_or("show needs a task ID")?),
        "list" | "show" => return Err("--file applies only to submit or cancel".into()),
        _ => return Err(format!("unknown task subcommand {verb}")),
    };
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

/// Runs an inbox command without constructing an agent or reading its credentials.
pub fn run(arguments: &[String]) -> u8 {
    if matches!(arguments, [argument] if argument == "--help" || argument == "-h") {
        println!("{USAGE}");
        return 0;
    }
    let options = match parse(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("coder task: {message}\n\n{USAGE}");
            return crate::cli::EXIT_USAGE;
        }
    };
    // Validate before opening a store. A wrong subcommand must never apply the
    // other action, and malformed input must not create an empty inbox.
    let bytes = match &options.operation {
        Operation::Submit(path) | Operation::Cancel(path) => {
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
            ) {
                return failure(
                    "action_mismatch",
                    "command action differs from the subcommand",
                );
            }
            Some(bytes)
        }
        Operation::List | Operation::Show(_) => None,
    };
    let directory = match options.store {
        Some(path) => path,
        None => match std::env::var_os("HOME").filter(|home| !home.is_empty()) {
            Some(home) => PathBuf::from(home).join(".openagents/tasks"),
            None => return failure("configuration", "HOME is unset; give --store explicitly"),
        },
    };
    let mut store = match Store::open(&directory) {
        Ok(store) => store,
        Err(error) => return failure(error.code(), error),
    };
    let result = match options.operation {
        Operation::Submit(_) | Operation::Cancel(_) => store
            .apply(bytes.as_deref().expect("mutation input was validated"))
            .map(|receipt| json!(receipt)),
        Operation::List => store.list().map(|tasks| json!(tasks)),
        Operation::Show(id) => store.show(&id).map(|task| json!(task)),
    };
    match result {
        Ok(value) => output(&value),
        Err(error) => failure(error.code(), error),
    }
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
}
