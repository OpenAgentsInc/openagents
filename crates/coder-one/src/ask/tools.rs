//! The ask executor's two tools, served over MCP on standard input and
//! output: `read` runs one allowlisted command, and `answer` ends the
//! episode with the answer and its citations.
//!
//! Claude Code and Codex both start this server as a child, inside the
//! same filesystem boundary as themselves, and see no other tool: their
//! own shells and editors are turned off. So the allowlist is code, not an
//! instruction, and the answer is a native tool call rather than JSON
//! written into text.
//!
//! The server writes each call to `calls.jsonl` and the answer to
//! `answer.json` in the ask's scratch directory, the one path the boundary
//! leaves writable, where the host reads them.
//!
//! The wire format is JSON-RPC 2.0, one message per line: `initialize`,
//! `tools/list`, `tools/call`, and `ping`. Notifications are read and
//! ignored.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::allow::Allowlist;

/// The file each tool call is appended to.
pub const CALLS_FILE: &str = "calls.jsonl";

/// The file the answer is written to.
pub const ANSWER_FILE: &str = "answer.json";

/// The most characters one read returns to the executor.
pub const READ_CAP: usize = 40_000;

/// How long one read may run.
pub const READ_DEADLINE: Duration = Duration::from_secs(90);

/// The most reads one ask runs.
pub const MAX_READS: usize = 24;

/// The MCP protocol version the server speaks when the client names none.
const PROTOCOL: &str = "2025-06-18";

/// The server's configuration: `coder-one ask tools` arguments.
#[derive(Clone, Debug)]
pub struct Server {
    pub allow: Allowlist,
    /// Where calls and the answer are written.
    pub scratch: PathBuf,
    /// The directory commands run in.
    pub cwd: PathBuf,
    /// Reads run so far.
    pub reads: usize,
}

/// The tools as `tools/list` describes them.
#[must_use]
pub fn tools() -> Value {
    json!([
        {
            "name": "read",
            "description": "Run one read-only command and get what it printed. Allowed: \
                `gym runs …` (the list with --reason, --agent, --outcome, --search, and \
                --order learning; `group --by …`; `show RUN --json`, `--evidence`, or \
                `--transcript`), `gym terminal-bench overview|compare|attempt|evidence|history \
                … --json`, `gym coder … --json` (matrix, study, composition, and the other \
                reads), `rg`, `cat`, `ls`, and `sed -n 'N,Mp' FILE`. One command with no \
                shell: no pipes, redirects, or second commands.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command, such as `gym runs show JOB/TRIAL --json`." },
                    "reason": { "type": "string", "description": "One sentence: what this read should show." }
                },
                "required": ["command", "reason"],
                "additionalProperties": false
            }
        },
        {
            "name": "answer",
            "description": "Finish with the answer. Call it once, last. Break the answer into \
                claims, and give each claim the runs it rests on as `job/trial`, the \
                transcript step numbers it rests on where it helps, the judgment IDs it \
                cites, such as `unearned_success`, and any repository files. Code checks every citation before the \
                answer is shown; a claim whose citation doesn't check is marked unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "answer": { "type": "string", "description": "The answer in a few short paragraphs of plain text, patterns first." },
                    "claims": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "claim": { "type": "string", "description": "One claim, in a sentence or two." },
                                "runs": { "type": "array", "items": { "type": "string" }, "description": "The runs it rests on, as job/trial." },
                                "steps": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "run": { "type": "string" },
                                            "step": { "type": "integer" }
                                        },
                                        "required": ["run", "step"],
                                        "additionalProperties": false
                                    },
                                    "description": "Transcript steps it rests on: the run and the step number from `gym runs show RUN --json`."
                                },
                                "judgments": { "type": "array", "items": { "type": "string" }, "description": "Jev judgment IDs it cites, such as unearned_success; each must hold for every cited run." },
                                "files": { "type": "array", "items": { "type": "string" }, "description": "Repository files it rests on, as a path from the repository root, optionally with `:LINE`." }
                            },
                            "required": ["claim", "runs", "steps", "judgments", "files"],
                            "additionalProperties": false
                        }
                    },
                    "proposed_change": { "type": "string", "description": "An optional change the findings suggest, for a person to decide on; empty when there is none." }
                },
                "required": ["answer", "claims", "proposed_change"],
                "additionalProperties": false
            }
        }
    ])
}

impl Server {
    /// Serves requests from standard input until it closes.
    ///
    /// # Errors
    ///
    /// Returns a message when standard output can't be written.
    pub async fn serve(mut self) -> Result<(), String> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let Some(reply) = self.handle(&line).await else {
                continue;
            };
            writeln!(stdout, "{reply}").map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Answers one message, or `None` for a notification.
    pub async fn handle(&mut self, line: &str) -> Option<Value> {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": "not JSON" }
            }));
        };
        let id = message.get("id").cloned()?;
        let method = message["method"].as_str().unwrap_or_default();
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": message
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "coder-one-ask", "version": crate::episode::version() },
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(self.call(&message["params"]).await),
            other => Err(format!("unknown method {other}")),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(why) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": why }
            }),
        })
    }

    async fn call(&mut self, params: &Value) -> Value {
        let arguments = &params["arguments"];
        let (text, error) = match params["name"].as_str().unwrap_or_default() {
            "read" => self.read(arguments).await,
            "answer" => self.answer(arguments),
            other => (format!("there is no tool {other}"), true),
        };
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": error,
        })
    }

    async fn read(&mut self, arguments: &Value) -> (String, bool) {
        let command = arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let reason = arguments["reason"].as_str().unwrap_or_default().to_string();
        let mut entry = json!({
            "at": atif::now_ms(),
            "tool": "read",
            "command": command,
            "reason": reason,
        });
        let outcome = if self.reads >= MAX_READS {
            Err(format!(
                "this ask has used its {MAX_READS} reads; answer from what you have"
            ))
        } else {
            self.allow.check(&command)
        };
        let allowed = match outcome {
            Ok(allowed) => allowed,
            Err(why) => {
                entry["refused"] = json!(why);
                self.log(&entry);
                return (format!("refused: {why}"), true);
            }
        };
        self.reads += 1;
        let ended = supervise::Job::new(&allowed.program)
            .args(&allowed.args)
            .in_directory(&self.cwd)
            .bounded(supervise::Limits::within(READ_DEADLINE).keeping(256 * 1024))
            .run()
            .await;
        let mut output = ended.stdout.marked();
        if !ended.ending.success() {
            output.push_str(&format!(
                "\n[{}]\n{}",
                ended.ending,
                super::clip(&ended.stderr.marked(), 2_000)
            ));
        }
        let clipped = super::clip_middle(&output, READ_CAP);
        entry["exit"] = json!(ended.ending.code());
        entry["ending"] = json!(ended.ending.to_string());
        entry["bytes"] = json!(ended.bytes());
        entry["returned_chars"] = json!(clipped.chars().count());
        entry["milliseconds"] = json!(u64::try_from(ended.elapsed.as_millis()).unwrap_or(0));
        self.log(&entry);
        (clipped, !ended.ending.success())
    }

    fn answer(&mut self, arguments: &Value) -> (String, bool) {
        let path = self.scratch.join(ANSWER_FILE);
        let written = serde_json::to_vec_pretty(arguments)
            .map_err(|error| error.to_string())
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()));
        self.log(&json!({
            "at": atif::now_ms(),
            "tool": "answer",
            "claims": arguments["claims"].as_array().map_or(0, Vec::len),
            "error": written.as_ref().err(),
        }));
        match written {
            Ok(()) => (
                "The answer is recorded. Stop now; the host checks the citations.".to_string(),
                false,
            ),
            Err(why) => (format!("the answer could not be recorded: {why}"), true),
        }
    }

    fn log(&self, entry: &Value) {
        let path = self.scratch.join(CALLS_FILE);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{entry}");
        }
    }
}

/// `coder-one ask tools --gym PATH --scratch DIR --cwd DIR [--rank]`.
///
/// # Errors
///
/// Returns a message when an argument is missing.
pub async fn command(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .map(PathBuf::from)
    };
    let server = Server {
        allow: Allowlist {
            gym: flag("--gym").ok_or("ask tools needs --gym")?,
            rank: args.iter().any(|arg| arg == "--rank"),
        },
        scratch: flag("--scratch").ok_or("ask tools needs --scratch")?,
        cwd: flag("--cwd").ok_or("ask tools needs --cwd")?,
        reads: 0,
    };
    server.serve().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn the_server_lists_two_tools_refuses_writes_and_records_the_answer() {
        let scratch = tempfile::tempdir().unwrap();
        let mut server = Server {
            allow: Allowlist {
                gym: crate::ask::allow::which("echo").expect("echo on PATH"),
                rank: false,
            },
            scratch: scratch.path().to_path_buf(),
            cwd: scratch.path().to_path_buf(),
            reads: 0,
        };
        let init = block_on(server.handle(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        ))
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(
            block_on(server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#))
                .is_none()
        );
        let listed =
            block_on(server.handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)).unwrap();
        let names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["read", "answer"]);

        // An allowed read runs the program; `gym` here is `echo`.
        let read = block_on(server.handle(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read","arguments":{"command":"gym runs --json","reason":"list"}}}"#,
        ))
        .unwrap();
        assert_eq!(read["result"]["isError"], false, "{read}");
        assert_eq!(
            read["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .trim(),
            "runs --json"
        );
        // A write is refused, with the reason.
        let refused = block_on(server.handle(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"read","arguments":{"command":"gym runs rank","reason":"rank"}}}"#,
        ))
        .unwrap();
        assert_eq!(refused["result"]["isError"], true);
        assert!(
            refused["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("--rank")
        );
        let answered = block_on(server.handle(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"answer","arguments":{"answer":"A.","claims":[],"proposed_change":""}}}"#,
        ))
        .unwrap();
        assert_eq!(answered["result"]["isError"], false);
        let answer: Value =
            serde_json::from_slice(&std::fs::read(scratch.path().join(ANSWER_FILE)).unwrap())
                .unwrap();
        assert_eq!(answer["answer"], "A.");
        let calls = std::fs::read_to_string(scratch.path().join(CALLS_FILE)).unwrap();
        assert_eq!(calls.lines().count(), 3, "{calls}");
        assert!(calls.contains("\"refused\""), "{calls}");
    }
}
