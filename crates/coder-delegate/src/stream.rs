//! Executor event streams, normalized, and the files they wrote.
//!
//! Claude Code and Codex each write their own event stream. A host that
//! watches a session, a scripted executor that replays one, and a checker
//! that recovers a candidate from one all need the same few facts, so this
//! module reads both formats into [`Event`]s: session started, command
//! started, command completed, artifact changed, assistant claim, usage
//! update, and session ended. Each event keeps its sequence number and the
//! line of the native stream it came from, so a reader can always go back
//! to the source.
//!
//! [`writes`] recovers the files a stream wrote in full: a Claude Code
//! `Write` tool call, or a shell here-document redirected into a file, such
//! as `cat > /app/run.py <<'PY'`. [`programs`] recovers the here-documents
//! fed to an interpreter, such as `python3 - <<'PY'`. A Claude Code `Edit`
//! replays onto content the stream wrote in full. A file changed by a
//! patch, a program, or an edit to content the stream doesn't hold is named
//! but not recovered: its last write is marked [`UNKNOWN`].

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Which executor wrote a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// `codex exec --json`.
    Codex,
    /// Claude Code's `--output-format stream-json`.
    Claude,
}

impl Format {
    /// The format's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Format::Codex => "codex",
            Format::Claude => "claude",
        }
    }

    /// Guesses the format from a stream's first event.
    #[must_use]
    pub fn detect(stream: &str) -> Option<Self> {
        for line in stream.lines() {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            return match event.get("type").and_then(Value::as_str) {
                Some(kind) if kind.contains('.') => Some(Format::Codex),
                Some("system" | "assistant" | "user" | "result") => Some(Format::Claude),
                _ => None,
            };
        }
        None
    }
}

/// What a normalized event says happened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    SessionStarted {
        session_id: Option<String>,
    },
    CommandStarted {
        command: String,
    },
    CommandCompleted {
        command: String,
        exit_code: Option<i64>,
        /// The output, clipped to [`OUTPUT_CHARS`].
        output: String,
    },
    ArtifactChanged {
        path: String,
        /// `add`, `update`, `delete`, or `write`.
        change: String,
    },
    AssistantClaim {
        text: String,
    },
    UsageUpdate {
        usage: Value,
    },
    SessionEnded {
        /// Whether the executor said it failed.
        error: bool,
        result: Option<String>,
    },
}

impl Kind {
    /// The kind's word, as the record spells it.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Kind::SessionStarted { .. } => "session_started",
            Kind::CommandStarted { .. } => "command_started",
            Kind::CommandCompleted { .. } => "command_completed",
            Kind::ArtifactChanged { .. } => "artifact_changed",
            Kind::AssistantClaim { .. } => "assistant_claim",
            Kind::UsageUpdate { .. } => "usage_update",
            Kind::SessionEnded { .. } => "session_ended",
        }
    }
}

/// One normalized event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Its place among the stream's normalized events, from 1.
    pub seq: u64,
    /// The native stream's line it came from, from 1.
    pub line: usize,
    /// The byte offset of that line in the native stream, when the reader
    /// knew it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(flatten)]
    pub kind: Kind,
}

/// The most characters of a command's output a normalized event keeps.
pub const OUTPUT_CHARS: usize = 2_000;

/// The most characters of an assistant claim a normalized event keeps.
pub const CLAIM_CHARS: usize = 4_000;

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}…")
}

/// Normalizes one native line, `line` being its number from 1. `seq` is
/// the next sequence number; it advances by the events returned.
#[must_use]
pub fn normalize_line(format: Format, text: &str, line: usize, seq: &mut u64) -> Vec<Event> {
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let kinds = match format {
        Format::Codex => codex(&event),
        Format::Claude => claude(&event),
    };
    kinds
        .into_iter()
        .map(|kind| {
            *seq += 1;
            Event {
                seq: *seq,
                line,
                offset: None,
                kind,
            }
        })
        .collect()
}

/// Normalizes a whole stream.
#[must_use]
pub fn normalize(format: Format, stream: &str) -> Vec<Event> {
    let mut seq = 0;
    stream
        .lines()
        .enumerate()
        .flat_map(|(i, text)| normalize_line(format, text, i + 1, &mut seq))
        .collect()
}

fn text_of(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn codex(event: &Value) -> Vec<Kind> {
    let item = &event["item"];
    let item_type = item.get("type").and_then(Value::as_str);
    match (event.get("type").and_then(Value::as_str), item_type) {
        (Some("thread.started"), _) => vec![Kind::SessionStarted {
            session_id: text_of(event, "thread_id"),
        }],
        (Some("item.started"), Some("command_execution")) => vec![Kind::CommandStarted {
            command: text_of(item, "command").unwrap_or_default(),
        }],
        (Some("item.completed"), Some("command_execution")) => {
            let command = text_of(item, "command").unwrap_or_default();
            let mut kinds = vec![Kind::CommandCompleted {
                exit_code: item.get("exit_code").and_then(Value::as_i64),
                output: clip(
                    item.get("aggregated_output")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    OUTPUT_CHARS,
                ),
                command: command.clone(),
            }];
            kinds.extend(
                shell_writes(&command)
                    .into_iter()
                    .map(|write| Kind::ArtifactChanged {
                        path: write.path,
                        change: "write".to_string(),
                    }),
            );
            kinds
        }
        (Some("item.completed"), Some("file_change")) => item
            .get("changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|change| Kind::ArtifactChanged {
                path: text_of(change, "path").unwrap_or_default(),
                change: text_of(change, "kind").unwrap_or_else(|| "update".to_string()),
            })
            .collect(),
        (Some("item.completed"), Some("agent_message")) => vec![Kind::AssistantClaim {
            text: clip(&text_of(item, "text").unwrap_or_default(), CLAIM_CHARS),
        }],
        (Some("turn.completed"), _) => vec![Kind::UsageUpdate {
            usage: event.get("usage").cloned().unwrap_or(Value::Null),
        }],
        (Some("turn.failed"), _) => vec![Kind::SessionEnded {
            error: true,
            result: event
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string),
        }],
        _ => Vec::new(),
    }
}

fn claude(event: &Value) -> Vec<Kind> {
    match event.get("type").and_then(Value::as_str) {
        Some("system") if text_of(event, "subtype").as_deref() == Some("init") => {
            vec![Kind::SessionStarted {
                session_id: text_of(event, "session_id"),
            }]
        }
        Some("assistant") => {
            let message = &event["message"];
            let mut kinds: Vec<Kind> = message
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|block| match block.get("type").and_then(Value::as_str) {
                    Some("text") => Some(Kind::AssistantClaim {
                        text: clip(&text_of(block, "text").unwrap_or_default(), CLAIM_CHARS),
                    }),
                    Some("tool_use") => {
                        let input = &block["input"];
                        match block.get("name").and_then(Value::as_str) {
                            Some("Bash") => Some(Kind::CommandStarted {
                                command: text_of(input, "command").unwrap_or_default(),
                            }),
                            Some("Write" | "Edit" | "MultiEdit" | "NotebookEdit") => {
                                Some(Kind::ArtifactChanged {
                                    path: text_of(input, "file_path")
                                        .or_else(|| text_of(input, "notebook_path"))
                                        .unwrap_or_default(),
                                    change: if block.get("name").and_then(Value::as_str)
                                        == Some("Write")
                                    {
                                        "write"
                                    } else {
                                        "update"
                                    }
                                    .to_string(),
                                })
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect();
            if let Some(usage) = message.get("usage").filter(|usage| usage.is_object()) {
                kinds.push(Kind::UsageUpdate {
                    usage: usage.clone(),
                });
            }
            kinds
        }
        Some("user") => event
            .pointer("/message/content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
            .map(|block| {
                let output = match block.get("content") {
                    Some(Value::String(text)) => text.clone(),
                    Some(Value::Array(parts)) => parts
                        .iter()
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                Kind::CommandCompleted {
                    command: text_of(block, "tool_use_id").unwrap_or_default(),
                    exit_code: match block.get("is_error").and_then(Value::as_bool) {
                        Some(true) => Some(1),
                        Some(false) => Some(0),
                        None => None,
                    },
                    output: clip(&output, OUTPUT_CHARS),
                }
            })
            .collect(),
        Some("result") => vec![Kind::SessionEnded {
            error: event.get("is_error").and_then(Value::as_bool) == Some(true),
            result: text_of(event, "result").map(|text| clip(&text, CLAIM_CHARS)),
        }],
        _ => Vec::new(),
    }
}

/// A file a stream wrote in full.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Write {
    /// The native stream's line it came from, from 1.
    pub line: usize,
    /// The path as the stream spelled it, absolute or relative to the
    /// session's working directory.
    pub path: String,
    pub content: String,
    /// How it was recovered: `heredoc` or `write_tool`.
    pub how: String,
}

/// A program a stream fed to an interpreter through a here-document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub line: usize,
    /// The interpreter's command line, such as `python3 -`.
    pub interpreter: String,
    pub source: String,
    /// The command's output, when the stream holds it.
    pub output: Option<String>,
    pub exit_code: Option<i64>,
}

/// Every file `stream` wrote in full, in order. A later write to the same
/// path replaces an earlier one; [`final_files`] applies that.
#[must_use]
pub fn writes(format: Format, stream: &str) -> Vec<Write> {
    let mut out = Vec::new();
    for (i, text) in stream.lines().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        match format {
            Format::Codex => {
                if event.get("type").and_then(Value::as_str) == Some("item.completed")
                    && event.pointer("/item/type").and_then(Value::as_str)
                        == Some("command_execution")
                {
                    let command = text_of(&event["item"], "command").unwrap_or_default();
                    out.extend(shell_writes(&command).into_iter().map(|mut write| {
                        write.line = i + 1;
                        write
                    }));
                }
            }
            Format::Claude => {
                for block in event
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let input = &block["input"];
                    match block.get("name").and_then(Value::as_str) {
                        Some("Write") => {
                            if let (Some(path), Some(content)) =
                                (text_of(input, "file_path"), text_of(input, "content"))
                            {
                                out.push(Write {
                                    line: i + 1,
                                    path,
                                    content,
                                    how: "write_tool".to_string(),
                                });
                            }
                        }
                        Some("Bash") => {
                            let command = text_of(input, "command").unwrap_or_default();
                            out.extend(shell_writes(&command).into_iter().map(|mut write| {
                                write.line = i + 1;
                                write
                            }));
                        }
                        Some(name @ ("Edit" | "MultiEdit")) => {
                            let Some(path) = text_of(input, "file_path") else {
                                continue;
                            };
                            let edits: Vec<Value> = if name == "Edit" {
                                vec![input.clone()]
                            } else {
                                input
                                    .get("edits")
                                    .and_then(Value::as_array)
                                    .cloned()
                                    .unwrap_or_default()
                            };
                            let before = out.iter().rev().find(|w| w.path == path);
                            let edited = before
                                .filter(|w| w.how != UNKNOWN)
                                .map(|w| w.content.clone())
                                .and_then(|mut content| {
                                    for edit in &edits {
                                        let old = text_of(edit, "old_string")?;
                                        let new = text_of(edit, "new_string").unwrap_or_default();
                                        if old.is_empty() || !content.contains(&old) {
                                            return None;
                                        }
                                        content =
                                            if edit.get("replace_all").and_then(Value::as_bool)
                                                == Some(true)
                                            {
                                                content.replace(&old, &new)
                                            } else {
                                                content.replacen(&old, &new, 1)
                                            };
                                    }
                                    Some(content)
                                });
                            out.push(match edited {
                                Some(content) => Write {
                                    line: i + 1,
                                    path,
                                    content,
                                    how: "edit_replayed".to_string(),
                                },
                                // An edit to content the stream doesn't
                                // hold leaves the file unknown from here on.
                                None => Write {
                                    line: i + 1,
                                    path,
                                    content: String::new(),
                                    how: UNKNOWN.to_string(),
                                },
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

/// How a [`Write`] marks a file an edit changed in a way the stream can't
/// replay: its content is unknown from that point.
pub const UNKNOWN: &str = "unknown";

/// The last full write to each path, in first-write order.
#[must_use]
pub fn final_files(writes: &[Write]) -> Vec<Write> {
    let mut out: Vec<Write> = Vec::new();
    for write in writes {
        if let Some(existing) = out.iter_mut().find(|w| w.path == write.path) {
            *existing = write.clone();
        } else {
            out.push(write.clone());
        }
    }
    out
}

/// Every here-document a stream fed to an interpreter: a Codex command
/// with its output, or a Claude Code `Bash` call.
#[must_use]
pub fn programs(stream: &str) -> Vec<Program> {
    let mut out = Vec::new();
    for (i, text) in stream.lines().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        // Each command the line carries, with its output when known.
        let mut commands: Vec<(String, Option<String>, Option<i64>)> = Vec::new();
        if event.get("type").and_then(Value::as_str) == Some("item.completed")
            && event.pointer("/item/type").and_then(Value::as_str) == Some("command_execution")
        {
            let item = &event["item"];
            commands.push((
                text_of(item, "command").unwrap_or_default(),
                text_of(item, "aggregated_output"),
                item.get("exit_code").and_then(Value::as_i64),
            ));
        }
        if event.get("type").and_then(Value::as_str) == Some("assistant") {
            for block in event
                .pointer("/message/content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if block.get("type").and_then(Value::as_str) == Some("tool_use")
                    && block.get("name").and_then(Value::as_str) == Some("Bash")
                {
                    commands.push((
                        text_of(&block["input"], "command").unwrap_or_default(),
                        None,
                        None,
                    ));
                }
            }
        }
        for (command, output, exit_code) in commands {
            let script = unwrap_shell(&command);
            for doc in heredocs(&script) {
                let head = doc.head.trim();
                let interpreter = head.split("<<").next().unwrap_or_default().trim();
                let program = interpreter.split_whitespace().next().unwrap_or_default();
                if ["python", "python3", "bash", "sh", "node", "ruby", "perl"]
                    .iter()
                    .any(|name| program == *name || program.ends_with(&format!("/{name}")))
                {
                    out.push(Program {
                        line: i + 1,
                        interpreter: interpreter.to_string(),
                        source: doc.body,
                        output: output.clone(),
                        exit_code,
                    });
                }
            }
        }
    }
    out
}

/// The script inside `bash -lc SCRIPT` or `sh -c SCRIPT`, or the command
/// itself when it isn't wrapped.
#[must_use]
pub fn unwrap_shell(command: &str) -> String {
    let words = shell_words(command);
    match words.as_slice() {
        [shell, flag, script, ..]
            if (shell.ends_with("bash") || shell.ends_with("sh"))
                && flag.starts_with('-')
                && flag.ends_with('c') =>
        {
            script.clone()
        }
        _ => command.to_string(),
    }
}

/// Splits `text` into words the way a POSIX shell would: single quotes
/// are literal, double quotes honor `\"`, `\\`, `\$`, and `` \` ``, and
/// an unquoted backslash escapes the next character. Newlines separate
/// words like spaces. Expansions are left as written.
#[must_use]
pub fn shell_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut open = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                open = true;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    word.push(c);
                }
            }
            '"' => {
                open = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.peek() {
                            Some(&next @ ('"' | '\\' | '$' | '`')) => {
                                word.push(next);
                                chars.next();
                            }
                            Some('\n') => {
                                chars.next();
                            }
                            _ => word.push('\\'),
                        },
                        other => word.push(other),
                    }
                }
            }
            '\\' => {
                open = true;
                if let Some(next) = chars.next()
                    && next != '\n'
                {
                    word.push(next);
                }
            }
            c if c.is_whitespace() => {
                if open {
                    words.push(std::mem::take(&mut word));
                    open = false;
                }
            }
            other => {
                open = true;
                word.push(other);
            }
        }
    }
    if open {
        words.push(word);
    }
    words
}

/// One here-document in a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heredoc {
    /// The line that opened it, such as `cat > run.py <<'PY'`.
    pub head: String,
    pub tag: String,
    pub body: String,
}

/// Every here-document in `script`. A document whose closing tag never
/// appears is left out: its content isn't known to be whole.
#[must_use]
pub fn heredocs(script: &str) -> Vec<Heredoc> {
    let lines: Vec<&str> = script.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let Some(at) = line.find("<<") else {
            i += 1;
            continue;
        };
        let rest = line[at + 2..].trim_start_matches('-').trim_start();
        let tag: String = if let Some(stripped) = rest.strip_prefix('\'') {
            stripped.split('\'').next().unwrap_or_default().to_string()
        } else if let Some(stripped) = rest.strip_prefix('"') {
            stripped.split('"').next().unwrap_or_default().to_string()
        } else {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect()
        };
        if tag.is_empty() || rest.starts_with('<') {
            i += 1;
            continue;
        }
        let strip_tabs = line[at + 2..].starts_with('-');
        let close = lines[i + 1..].iter().position(|body| {
            let body = if strip_tabs {
                body.trim_start_matches('\t')
            } else {
                body
            };
            body == tag
        });
        let Some(close) = close else {
            i += 1;
            continue;
        };
        let body_lines = &lines[i + 1..i + 1 + close];
        let mut body = body_lines
            .iter()
            .map(|body| {
                if strip_tabs {
                    body.trim_start_matches('\t')
                } else {
                    body
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        body.push('\n');
        out.push(Heredoc {
            head: line.to_string(),
            tag,
            body,
        });
        i += close + 2;
    }
    out
}

/// The files a shell command writes in full through a here-document
/// redirected to a file: `cat > PATH <<'X'`, `cat <<'X' > PATH`, or
/// `tee PATH <<'X'`.
#[must_use]
pub fn shell_writes(command: &str) -> Vec<Write> {
    let script = unwrap_shell(command);
    heredocs(&script)
        .into_iter()
        .filter_map(|doc| {
            let words = shell_words(&doc.head.replace("<<", " <<"));
            let program = words.first()?;
            if program != "cat" && program != "tee" {
                return None;
            }
            let mut path = None;
            let mut iter = words.iter().skip(1).peekable();
            while let Some(word) = iter.next() {
                if word.starts_with("<<") {
                    // The tag, possibly as the next word.
                    if word.len() == 2 {
                        iter.next();
                    }
                    continue;
                }
                if word == ">" {
                    path = iter.next().cloned();
                } else if let Some(target) = word.strip_prefix('>')
                    && !target.starts_with('>')
                    && !target.is_empty()
                {
                    path = Some(target.to_string());
                } else if program == "tee" && !word.starts_with('-') && path.is_none() {
                    path = Some(word.clone());
                }
            }
            path.map(|path| Write {
                line: 0,
                path,
                content: doc.body,
                how: "heredoc".to_string(),
            })
        })
        .collect()
}

/// A short record of normalized events: counts by kind.
#[must_use]
pub fn tally(events: &[Event]) -> Value {
    let mut counts = serde_json::Map::new();
    for event in events {
        let n = counts
            .get(event.kind.word())
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        counts.insert(event.kind.word().to_string(), json!(n));
    }
    Value::Object(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_words_follow_posix_quoting() {
        assert_eq!(
            shell_words(r#"/bin/bash -lc "echo \"hi\" 'x'"'"'y'"#),
            vec!["/bin/bash", "-lc", "echo \"hi\" 'x'\"y"]
        );
        assert_eq!(shell_words("a'b c'\"d\""), vec!["ab cd"]);
        assert_eq!(shell_words(r"a\ b"), vec!["a b"]);
    }

    #[test]
    fn a_codex_heredoc_write_is_recovered_whole() {
        let command = "/bin/bash -lc \"cat > /app/run.py <<'PY'\nx = \\\"\\\"\\\"doc\\\"\\\"\\\"\nPY\npython3 - <<'PY'\nprint(1)\nPY\"";
        let writes = shell_writes(command);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].path, "/app/run.py");
        assert_eq!(writes[0].content, "x = \"\"\"doc\"\"\"\n");
        let line = json!({ "type": "item.completed", "item": { "type": "command_execution", "command": command, "aggregated_output": "1\n", "exit_code": 0 } });
        let programs = programs(&line.to_string());
        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0].interpreter, "python3 -");
        assert_eq!(programs[0].source, "print(1)\n");
    }

    #[test]
    fn both_formats_normalize_to_the_same_kinds() {
        let codex = [
            json!({"type":"thread.started","thread_id":"t1"}),
            json!({"type":"item.started","item":{"type":"command_execution","command":"ls"}}),
            json!({"type":"item.completed","item":{"type":"command_execution","command":"ls","aggregated_output":"a\n","exit_code":0}}),
            json!({"type":"item.completed","item":{"type":"file_change","changes":[{"path":"a.py","kind":"add"}]}}),
            json!({"type":"item.completed","item":{"type":"agent_message","text":"done"}}),
            json!({"type":"turn.completed","usage":{"input_tokens":3}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let events = normalize(Format::Codex, &codex);
        let kinds: Vec<&str> = events.iter().map(|e| e.kind.word()).collect();
        assert_eq!(
            kinds,
            [
                "session_started",
                "command_started",
                "command_completed",
                "artifact_changed",
                "assistant_claim",
                "usage_update"
            ]
        );
        assert_eq!(events[3].line, 4);
        assert_eq!(Format::detect(&codex), Some(Format::Codex));

        let claude = [
            json!({"type":"system","subtype":"init","session_id":"s1"}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"/w/a.py","content":"print(2)\n"}}],"usage":{"input_tokens":5}}}),
            json!({"type":"result","is_error":false,"result":"ok"}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let events = normalize(Format::Claude, &claude);
        let kinds: Vec<&str> = events.iter().map(|e| e.kind.word()).collect();
        assert_eq!(
            kinds,
            [
                "session_started",
                "artifact_changed",
                "usage_update",
                "session_ended"
            ]
        );
        let written = writes(Format::Claude, &claude);
        assert_eq!(written[0].content, "print(2)\n");
    }

    #[test]
    fn an_edit_replays_on_content_the_stream_holds_and_otherwise_is_unknown() {
        let call = |name: &str, input: Value| {
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":name,"input":input}]}}).to_string()
        };
        let stream = [
            call(
                "Write",
                json!({"file_path":"/app/a.py","content":"x = 1\ny = 2\n"}),
            ),
            call(
                "Edit",
                json!({"file_path":"/app/a.py","old_string":"y = 2","new_string":"y = 3"}),
            ),
            call(
                "Edit",
                json!({"file_path":"/app/b.py","old_string":"z","new_string":"w"}),
            ),
        ]
        .join("\n");
        let files = final_files(&writes(Format::Claude, &stream));
        assert_eq!(files[0].content, "x = 1\ny = 3\n");
        assert_eq!(files[0].how, "edit_replayed");
        assert_eq!(files[1].path, "/app/b.py");
        assert_eq!(files[1].how, UNKNOWN);
    }

    #[test]
    fn a_retained_luna_stream_yields_its_candidate() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../bench/terminal-bench/traces/extended--coder-one-jevprobe3-luna--cancel-async-tasks-2/cancel-async-tasks__8MSemsU.episode/artifacts/delegate-1.stream.jsonl",
        );
        let Ok(stream) = std::fs::read_to_string(path) else {
            return;
        };
        let files = final_files(&writes(Format::Codex, &stream));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/app/run.py");
        assert!(files[0].content.contains("async def run_tasks("));
        assert!(files[0].content.contains("\"\"\"Run async jobs"));
        assert!(files[0].content.contains("``finally`` cleanup"));
    }
}
