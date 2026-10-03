//! `openagents mcp serve`: the command as an MCP server over stdio, and
//! `openagents completions SHELL`. Both are projections of the same help
//! table `openagents --help` prints: one tool per command group, whose
//! `args` are the words that would follow the group on the command line
//! and whose result is the document `openagents --json GROUP ARGS...`
//! prints. Each call runs this same program in a child process with stdin
//! closed, so a call can never wait on a prompt, and the exit code is
//! reported with the document: `0` is a result, anything else is an error
//! result carrying whatever the command printed.
//!
//! The wire is newline-delimited JSON-RPC 2.0 with the MCP lifecycle:
//! `initialize`, `notifications/initialized`, then `tools/list` and
//! `tools/call`. Reimplemented from `oak::mcp`'s shape without sharing its
//! code, which is bound to the decision API.
//!
//! A server may carry a [`Toll`]: `openagents x402 mcp-serve` sets one so
//! every `tools/call` is challenged, settled, and run over the upstream x402
//! MCP transport (`mcp:1`). A tolled call is still validated first, so a
//! buyer never pays for a call the server would refuse as invalid.

use std::io::{BufRead, Read, Write};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use openagents_x402::mcp::{Gate, with_settlement};
use serde_json::{Value, json};

use crate::{Args, Output};

mod completion_scripts;
use completion_scripts::{bash, fish, zsh};

pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
pub const SERVER_NAME: &str = "openagents";
const MAX_MESSAGE_BYTES: u64 = 1024 * 1024;
const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 16 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub const USAGE: &str = "usage: openagents mcp serve [--timeout SECONDS]
  serve    Serve every command group as an MCP tool over stdio. A tool
           call `GROUP {\"args\": [...]}` runs `openagents --json GROUP ARGS...`
           with no stdin and returns its document; a nonzero exit is an
           error result. --timeout bounds one call (default 120).";

pub const COMPLETIONS_USAGE: &str = "usage: openagents completions SHELL
  SHELL is bash, zsh, or fish. Prints a completion script for the command
  groups, subcommands, and flags to stdout; source it or save it where the shell reads.";

/// One row of the top-level help table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub summary: String,
}

/// The groups `usage` lists, in order: every two-space-indented row whose
/// first word is a command name, with continuation lines joined.
pub fn groups(usage: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for line in usage.lines() {
        let Some(row) = line.strip_prefix("  ") else {
            continue;
        };
        if row.starts_with(' ') {
            if let Some(last) = groups.last_mut() {
                last.summary.push(' ');
                last.summary.push_str(row.trim());
            }
            continue;
        }
        let Some((name, summary)) = row.split_once(char::is_whitespace) else {
            continue;
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase()) {
            continue;
        }
        groups.push(Group {
            name: name.to_owned(),
            summary: summary.trim().to_owned(),
        });
    }
    groups
}

/// The groups a tool call may run: everything in the table except the
/// resident `host`, which is a long-running server, the full-screen
/// `terminal`, and `mcp` itself, narrowed to `only` when that names any
/// group.
fn callable(usage: &str, only: &[String]) -> Vec<Group> {
    groups(usage)
        .into_iter()
        .filter(|group| {
            !matches!(
                group.name.as_str(),
                "host" | "terminal" | "mcp" | "completions"
            )
        })
        .filter(|group| only.is_empty() || only.contains(&group.name))
        .collect()
}

/// Decides whether one validated `tools/call` may run, and on what terms.
pub trait Toll: Send + Sync {
    /// `params` is the call's `params` object as received, `_meta` included.
    fn gate(&self, params: &Value) -> Result<Gate, &'static str>;
}

pub fn run(output: &Output, words: &[String], usage: &str) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("mcp", "COMMAND is required", USAGE);
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "serve" => {
            let args = match Args::parse(rest, &[]) {
                Ok(args) => args,
                Err(error) => return output.usage("mcp serve", &error, USAGE),
            };
            let timeout: u64 = match args.number("timeout", DEFAULT_TIMEOUT.as_secs()) {
                Ok(timeout) if timeout > 0 => timeout,
                _ => {
                    return output.usage(
                        "mcp serve",
                        "--timeout is a positive number of seconds",
                        USAGE,
                    );
                }
            };
            let server = Server {
                usage: usage.to_owned(),
                timeout: Duration::from_secs(timeout),
                tools: Vec::new(),
                toll: None,
            };
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            server.serve(stdin.lock(), stdout.lock(), std::io::stderr())
        }
        other => output.usage("mcp", &format!("unknown command `{other}`"), USAGE),
    }
}

/// `openagents completions SHELL`.
pub fn completions(output: &Output, words: &[String], _usage: &str) -> u8 {
    let Some(shell) = words.first() else {
        return output.usage("completions", "SHELL is required", COMPLETIONS_USAGE);
    };
    let script = match shell.as_str() {
        "bash" => bash(),
        "zsh" => zsh(),
        "fish" => fish(),
        "--help" | "-h" | "help" => {
            println!("{COMPLETIONS_USAGE}");
            return 0;
        }
        other => {
            return output.usage(
                "completions",
                &format!("unknown shell `{other}`; bash, zsh, or fish"),
                COMPLETIONS_USAGE,
            );
        }
    };
    if output.json() {
        output.emit(&json!({ "shell": shell, "script": script }), |_| {
            String::new()
        });
    } else {
        print!("{script}");
    }
    0
}

/// The lifecycle phase of one stdio session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Negotiated,
    Ready,
}

/// One stdio MCP server over the top-level help table.
pub struct Server {
    pub usage: String,
    pub timeout: Duration,
    /// The groups served, or every callable group when empty.
    pub tools: Vec<String>,
    /// The payment gate on every call, when the server is paid.
    pub toll: Option<Arc<dyn Toll>>,
}

impl Server {
    pub fn serve(
        &self,
        mut input: impl BufRead,
        mut output: impl Write,
        mut log: impl Write,
    ) -> u8 {
        let mut phase = Phase::Start;
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = match std::io::Read::take(&mut input, MAX_MESSAGE_BYTES + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(read) => read,
                Err(error) => {
                    let _ = writeln!(log, "openagents mcp: cannot read stdin: {error}");
                    return crate::EXIT_FAILURE;
                }
            };
            if read == 0 {
                return 0;
            }
            if read as u64 > MAX_MESSAGE_BYTES {
                if !emit(
                    &mut output,
                    &error(
                        Value::Null,
                        INVALID_REQUEST,
                        "the message is larger than the size limit",
                    ),
                ) {
                    return 0;
                }
                continue;
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let message: Value = match serde_json::from_slice(&line) {
                Ok(message) => message,
                Err(_) => {
                    if !emit(&mut output, &error(Value::Null, PARSE_ERROR, "parse error")) {
                        return 0;
                    }
                    continue;
                }
            };
            if let Some(response) = self.handle(&mut phase, &message)
                && !emit(&mut output, &response)
            {
                return 0;
            }
        }
    }

    /// Dispatch one message; `None` means nothing answers.
    pub fn handle(&self, phase: &mut Phase, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        if message.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "the message must include `jsonrpc: \"2.0\"`",
            ));
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            if message.get("result").is_some() || message.get("error").is_some() {
                return None;
            }
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "`method` must be a string",
            ));
        };
        let Some(id) = id else {
            if method == "notifications/initialized" && *phase == Phase::Negotiated {
                *phase = Phase::Ready;
            }
            return None;
        };
        match method {
            "initialize" => Some(self.initialize(phase, &id, message.get("params"))),
            "ping" => Some(result(&id, json!({}))),
            "tools/list" if *phase == Phase::Ready => Some(result(&id, self.tool_list())),
            "tools/call" if *phase == Phase::Ready => Some(self.call(&id, message.get("params"))),
            "tools/list" | "tools/call" => Some(error(
                id,
                INVALID_PARAMS,
                "the session is not initialized yet; send `initialize`, then `notifications/initialized`",
            )),
            _ => Some(error(id, METHOD_NOT_FOUND, "method not found")),
        }
    }

    fn initialize(&self, phase: &mut Phase, id: &Value, params: Option<&Value>) -> Value {
        if *phase != Phase::Start {
            return error(
                id.clone(),
                INVALID_PARAMS,
                "the session is already initialized",
            );
        }
        let Some(requested) = params
            .and_then(|p| p.get("protocolVersion"))
            .and_then(Value::as_str)
        else {
            return error(
                id.clone(),
                INVALID_PARAMS,
                "`initialize` params must include `protocolVersion`",
            );
        };
        let version = if PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            PROTOCOL_VERSIONS[0]
        };
        *phase = Phase::Negotiated;
        result(
            id,
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "title": "openagents, the OpenAgents command",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "Each tool is one `openagents` command group. Pass the words that would follow the group on the command line as `args`; the result is the `--json` document the command prints, with its exit code (0 success, 1 refused or failed, 64 invalid usage). Pass [\"--help\"] to read a group's syntax. Commands never prompt: stdin is closed.",
            }),
        )
    }

    /// One tool per callable group. The description is the row from the
    /// help table; the syntax is one `--help` call away.
    pub fn tool_list(&self) -> Value {
        let tools: Vec<Value> = callable(&self.usage, &self.tools)
            .into_iter()
            .map(|group| {
                json!({
                    "name": group.name,
                    "title": format!("openagents {}", group.name),
                    "description": format!("{} Runs `openagents --json {} ARGS...`; pass [\"--help\"] for the syntax.", group.summary, group.name),
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "args": {
                                "type": "array",
                                "items": { "type": "string", "maxLength": MAX_ARG_BYTES },
                                "maxItems": MAX_ARGS,
                                "description": format!("The words after `openagents {}` on the command line.", group.name)
                            }
                        },
                        "additionalProperties": false
                    },
                    "outputSchema": {
                        "type": "object",
                        "properties": {
                            "exit_code": { "type": "integer" },
                            "document": { "description": "The parsed --json output, when the command printed one." },
                            "stdout": { "type": "string", "description": "The raw output when it is not one JSON document." },
                            "stderr": { "type": "string" }
                        },
                        "required": ["exit_code"]
                    }
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    fn call(&self, id: &Value, params: Option<&Value>) -> Value {
        let Some(name) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
            return error(
                id.clone(),
                INVALID_PARAMS,
                "`tools/call` params must include a tool `name`",
            );
        };
        if !callable(&self.usage, &self.tools)
            .iter()
            .any(|g| g.name == name)
        {
            return error(
                id.clone(),
                INVALID_PARAMS,
                &format!("unknown tool `{name}`"),
            );
        }
        let arguments = params.and_then(|p| p.get("arguments"));
        let args: Vec<String> = match arguments.and_then(|a| a.get("args")) {
            None => Vec::new(),
            Some(Value::Array(items)) => {
                if items.len() > MAX_ARGS {
                    return error(
                        id.clone(),
                        INVALID_PARAMS,
                        &format!("`args` has more than {MAX_ARGS} words"),
                    );
                }
                let mut args = Vec::with_capacity(items.len());
                for item in items {
                    match item.as_str() {
                        Some(word) if word.len() <= MAX_ARG_BYTES && !word.contains('\0') => {
                            args.push(word.to_owned())
                        }
                        _ => {
                            return error(
                                id.clone(),
                                INVALID_PARAMS,
                                "each `args` item is a string without NUL, at most 16 KiB",
                            );
                        }
                    }
                }
                args
            }
            Some(_) => {
                return error(
                    id.clone(),
                    INVALID_PARAMS,
                    "`args` must be an array of strings",
                );
            }
        };
        let settlement = match &self.toll {
            None => None,
            Some(toll) => match toll.gate(params.unwrap_or(&Value::Null)) {
                Ok(Gate::Challenge(answer)) | Ok(Gate::Refused(answer)) => {
                    return result(id, answer);
                }
                Ok(Gate::Admitted { settlement, .. }) => Some(settlement),
                Err(message) => return error(id.clone(), INVALID_PARAMS, message),
            },
        };
        let outcome = match self.exec(name, &args) {
            Ok(outcome) => tool_outcome(&outcome),
            // After a settlement the claim is consumed; the buyer holds a
            // settlement that bought a failed run, and the result says so.
            Err(message) => json!({
                "content": [{ "type": "text", "text": message }],
                "structuredContent": { "exit_code": crate::EXIT_FAILURE, "stderr": message },
                "isError": true,
            }),
        };
        match settlement {
            Some(settlement) => result(id, with_settlement(outcome, settlement)),
            None => result(id, outcome),
        }
    }

    /// Run `openagents --json GROUP ARGS...` with no stdin under the timeout.
    fn exec(&self, group: &str, args: &[String]) -> Result<Outcome, String> {
        let exe = std::env::current_exe().map_err(|e| format!("cannot find this program: {e}"))?;
        let mut child = std::process::Command::new(exe)
            .arg("--json")
            .arg(group)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start openagents {group}: {e}"))?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = child.stderr.take().ok_or("no stderr")?;
        let out = std::thread::spawn(move || read_bounded(stdout));
        let err = std::thread::spawn(move || read_bounded(stderr));
        let started = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "openagents {group} did not finish within {} s and was stopped",
                        self.timeout.as_secs()
                    ));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("cannot wait for openagents {group}: {e}")),
            }
        };
        let stdout = out.join().unwrap_or_default();
        let stderr = err.join().unwrap_or_default();
        Ok(Outcome {
            exit_code: status.code().unwrap_or(i32::from(crate::EXIT_FAILURE)),
            stdout,
            stderr,
        })
    }
}

fn read_bounded(mut reader: impl std::io::Read) -> String {
    let mut bytes = Vec::new();
    let _ = std::io::Read::take(&mut reader, MAX_OUTPUT_BYTES as u64).read_to_end(&mut bytes);
    // Drain the rest so the child is never blocked on a full pipe.
    let _ = std::io::copy(&mut reader, &mut std::io::sink());
    String::from_utf8_lossy(&bytes).into_owned()
}

/// What one command run produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The tool result for a finished command: the parsed document when the
/// output is one JSON value, `{"events": [...]}` when it is NDJSON (as
/// `chat` streams), else the raw text, and the exit code either way.
pub fn tool_outcome(outcome: &Outcome) -> Value {
    let trimmed = outcome.stdout.trim();
    let document: Option<Value> = serde_json::from_str(trimmed).ok().or_else(|| {
        trimmed
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()
            .ok()
            .filter(|events| events.len() > 1)
            .map(|events| json!({ "events": events }))
    });
    let mut structured = json!({ "exit_code": outcome.exit_code });
    match &document {
        Some(document) => structured["document"] = document.clone(),
        None if !trimmed.is_empty() => structured["stdout"] = Value::String(outcome.stdout.clone()),
        None => {}
    }
    if !outcome.stderr.trim().is_empty() {
        structured["stderr"] = Value::String(outcome.stderr.clone());
    }
    let text = match &document {
        Some(document) if outcome.exit_code == 0 => document.to_string(),
        Some(document) => format!(
            "exit {}: {document}\n{}",
            outcome.exit_code,
            outcome.stderr.trim()
        ),
        None => format!(
            "exit {}: {}{}",
            outcome.exit_code, outcome.stdout, outcome.stderr
        )
        .trim()
        .to_owned(),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": outcome.exit_code != 0,
    })
}

fn emit(output: &mut impl Write, message: &Value) -> bool {
    let mut bytes = serde_json::to_vec(message).unwrap_or_default();
    bytes.push(b'\n');
    output
        .write_all(&bytes)
        .and_then(|()| output.flush())
        .is_ok()
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn result(id: &Value, value: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": value })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "usage: openagents [--json] COMMAND [ARGS]

Verse (NIP-MV):
  verse        See who is around, listen, speak, move, gesture, drive owned
               entities, and read quests, XP, and the board.
  zone         Drive the Lagrange 1 construction zone.

  host         Run and administer the resident host on this machine.
  version      Show the repository, commit, and tree state.

Run `openagents COMMAND --help` for each group's syntax.
Exit codes: 0 success, 1 refused or failed, 64 invalid usage.";

    #[test]
    fn groups_come_from_the_help_table() {
        let groups = groups(TABLE);
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["verse", "zone", "host", "version"]);
        assert_eq!(
            groups[0].summary,
            "See who is around, listen, speak, move, gesture, drive owned entities, and read quests, XP, and the board."
        );
        let real = super::groups(crate::USAGE);
        assert!(real.iter().any(|g| g.name == "sov"));
        assert!(real.iter().any(|g| g.name == "mcp"));
        assert!(real.iter().any(|g| g.name == "completions"));
    }

    #[test]
    fn lifecycle_gates_tools_and_lists_callable_groups() {
        let server = Server {
            usage: TABLE.to_owned(),
            timeout: Duration::from_secs(1),
            tools: Vec::new(),
            toll: None,
        };
        let mut phase = Phase::Start;
        let early = server
            .handle(
                &mut phase,
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
            )
            .unwrap();
        assert_eq!(early["error"]["code"], INVALID_PARAMS);
        let init = server
            .handle(
                &mut phase,
                &json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } }),
            )
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(
            server
                .handle(
                    &mut phase,
                    &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
                )
                .is_none()
        );
        let list = server
            .handle(
                &mut phase,
                &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
            )
            .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["verse", "zone", "version"], "host is not callable");
        let unknown = server
            .handle(&mut phase, &json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "host" } }))
            .unwrap();
        assert_eq!(unknown["error"]["code"], INVALID_PARAMS);
        let bad_args = server
            .handle(&mut phase, &json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "zone", "arguments": { "args": "fly" } } }))
            .unwrap();
        assert_eq!(bad_args["error"]["code"], INVALID_PARAMS);
    }

    struct FixedToll(Gate);

    impl Toll for FixedToll {
        fn gate(&self, _: &Value) -> Result<Gate, &'static str> {
            Ok(self.0.clone())
        }
    }

    fn ready(server: &Server) -> Phase {
        let mut phase = Phase::Start;
        server.handle(
            &mut phase,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
        );
        server.handle(
            &mut phase,
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        );
        phase
    }

    #[test]
    fn a_toll_answers_before_the_tool_runs_and_narrows_the_list() {
        let challenge =
            json!({ "isError": true, "structuredContent": { "x402Version": 2 }, "content": [] });
        let server = Server {
            usage: TABLE.to_owned(),
            timeout: Duration::from_secs(1),
            tools: vec!["version".into()],
            toll: Some(Arc::new(FixedToll(Gate::Challenge(challenge.clone())))),
        };
        let mut phase = ready(&server);
        let list = server
            .handle(
                &mut phase,
                &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
            )
            .unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 1);
        let zone = server
            .handle(&mut phase, &json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "zone" } }))
            .unwrap();
        assert_eq!(
            zone["error"]["code"], INVALID_PARAMS,
            "not served, so not sold"
        );
        let unpaid = server
            .handle(&mut phase, &json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "version" } }))
            .unwrap();
        assert_eq!(unpaid["result"], challenge);

        let server = Server {
            usage: TABLE.to_owned(),
            timeout: Duration::from_secs(20),
            tools: Vec::new(),
            toll: Some(Arc::new(FixedToll(Gate::Admitted {
                request_hash: "00".repeat(32),
                payment_hash: "11".repeat(32),
                settlement: json!({ "success": true, "transaction": "11".repeat(32) }),
            }))),
        };
        let mut phase = ready(&server);
        let paid = server
            .handle(&mut phase, &json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "version", "arguments": { "args": [] } } }))
            .unwrap();
        assert_eq!(
            paid["result"]["_meta"][openagents_x402::mcp::PAYMENT_RESPONSE_META]["success"],
            json!(true)
        );
        assert!(paid["result"]["structuredContent"]["exit_code"].is_number());
    }

    #[test]
    fn outcomes_keep_text_when_not_json() {
        let outcome = tool_outcome(&Outcome {
            exit_code: 1,
            stdout: "plain\n".into(),
            stderr: "why\n".into(),
        });
        assert_eq!(outcome["isError"], true);
        assert_eq!(outcome["structuredContent"]["stdout"], "plain\n");
        assert_eq!(outcome["structuredContent"]["stderr"], "why\n");
        assert!(outcome["structuredContent"].get("document").is_none());
    }

    /// `chat --json` streams NDJSON events; a tool call returns them all.
    #[test]
    fn ndjson_outcomes_are_one_list_of_events() {
        let outcome = tool_outcome(&Outcome {
            exit_code: 0,
            stdout: "{\"event\":\"accepted\"}\n{\"event\":\"result\",\"text\":\"hi\"}\n".into(),
            stderr: String::new(),
        });
        let events = &outcome["structuredContent"]["document"]["events"];
        assert_eq!(events[1]["text"], "hi");
        assert_eq!(outcome["isError"], false);
    }

    #[test]
    fn completion_scripts_name_every_group() {
        let names: Vec<String> = coder::cli_route::tree::bundled()
            .groups
            .iter()
            .map(|g| g.name.clone())
            .collect();
        for script in [bash(), zsh(), fish()] {
            for name in &names {
                assert!(script.contains(name.as_str()), "{script}");
            }
        }
    }
}
