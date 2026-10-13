//! How one tool call reads on one row, wherever a conversation is shown
//! (#11543): Coder's terminal transcript and the account copy of the same
//! chat on openagents.com say the same thing about each call, so both take
//! it from here.
//!
//! A call reads as its name and [`summary`]: why it failed, or [`params`],
//! its input on one line: the
//! path it works on first and bare, a shell command bare, a command-line
//! tool's argument list as the command line it runs, and anything else as
//! `key: value` joined with ` · `. The model's thinking ([`THINKING`]) and a
//! short notice ([`NOTICE`]) are not calls a person reads as calls: their
//! words are the row ([`Row`]).
//!
//! Nothing here cuts a line to a width; each surface fits it to its own.

use serde_json::Value;

/// The entry that holds a thinking block: its words are in the output's
/// `text`.
pub const THINKING: &str = "claude:thinking";

/// The entry that holds one short notice (a retry, something that failed
/// to load): its words are in the output's `text`.
pub const NOTICE: &str = "claude:notice";

/// The keys a call names the file or folder it works on with, in the order
/// they are preferred. `filePath` is OpenCode's name for it.
pub const PATH_KEYS: [&str; 6] = [
    "path",
    "file_path",
    "filePath",
    "notebook_path",
    "folder",
    "directory",
];

/// Whether a flattened key is a top-level path key.
#[must_use]
pub fn is_path_key(key: &str) -> bool {
    PATH_KEYS.contains(&key)
}

/// A value's `key: value` pairs, nested keys joined with a dot, at most two
/// levels deep, with a lone value named `value`.
#[must_use]
pub fn fields(value: &Value) -> Vec<(String, String)> {
    fn walk(value: &Value, prefix: &str, depth: usize, result: &mut Vec<(String, String)>) {
        match value {
            Value::Object(object) if depth < 2 => {
                for (key, value) in object {
                    let key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(value, &key, depth + 1, result);
                }
            }
            Value::Null if prefix.is_empty() => {}
            value => {
                let text = match value {
                    Value::String(text) => text.replace('\n', " ↵ "),
                    _ => value.to_string(),
                };
                result.push((
                    if prefix.is_empty() {
                        "value".into()
                    } else {
                        prefix.into()
                    },
                    text,
                ));
            }
        }
    }
    let mut values = Vec::new();
    walk(value, "", 0, &mut values);
    values
}

/// A call's parameters, its path first: a nested `questions.refund` key is
/// never a path, so only a top-level key moves.
#[must_use]
pub fn path_first(value: &Value) -> Vec<(String, String)> {
    let mut values = fields(value);
    if let Some(index) = values.iter().position(|(key, _)| is_path_key(key)) {
        let path = values.remove(index);
        values.insert(0, path);
    }
    values
}

/// A call whose only parameter is its argument list (`arguments` or
/// `args`, all strings), as one command line: words joined by spaces, a
/// word with a space or a quote in it single-quoted.
#[must_use]
pub fn argument_line(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let (key, list) = object.iter().next()?;
    if key != "arguments" && key != "args" {
        return None;
    }
    let words = list
        .as_array()?
        .iter()
        .map(|word| word.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(
        words
            .iter()
            .map(|word| {
                if word.is_empty()
                    || word.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"')
                {
                    format!("'{}'", word.replace('\'', "'\\''"))
                } else {
                    word.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// A call's input on one line, uncut: a command-line tool's arguments as
/// the command line it runs ("projects show finances"), else the path it
/// works on first and bare, then the rest as `key: value` joined with
/// ` · `, a `command` bare. Empty when the call has none.
#[must_use]
pub fn params(value: &Value) -> String {
    if let Some(line) = argument_line(value) {
        return line;
    }
    let values = path_first(value);
    let path = values
        .first()
        .filter(|(key, _)| is_path_key(key))
        .map(|(_, value)| value.as_str());
    let rest = values
        .iter()
        .filter(|(key, _)| !is_path_key(key))
        .map(|(key, value)| {
            if key == "command" {
                value.clone()
            } else {
                format!("{key}: {value}")
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    match (path, rest.is_empty()) {
        (Some(path), true) => path.to_owned(),
        (Some(path), false) => format!("{path} · {rest}"),
        (None, _) => rest,
    }
}

/// What one entry of a conversation shows, apart from the person's
/// messages and the replies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The model's thinking: its words, shown dim and slanted.
    Thinking(String),
    /// One short notice: its words, shown quietly.
    Notice(String),
    /// A tool call: its name, then [`params`] when it has any.
    Call(String),
}

impl Row {
    /// The entry `name` with `input` that answered `output`, as it reads.
    /// A thinking or notice entry with no words is `None`.
    #[must_use]
    pub fn of(name: &str, input: &Value, output: &Value) -> Option<Self> {
        let words = || {
            output["text"]
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        match name {
            THINKING => words().map(Self::Thinking),
            NOTICE => words().map(Self::Notice),
            _ => Some(Self::Call(join(name, &summary(input, output)))),
        }
    }

    /// The row's words.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Thinking(text) | Self::Notice(text) | Self::Call(text) => text,
        }
    }
}

/// What a call's row says after its name: why it failed, in the error's
/// first line, when it failed with one (#11482), else [`params`].
#[must_use]
pub fn summary(input: &Value, output: &Value) -> String {
    match output.get("error").and_then(Value::as_str) {
        Some(error) => error.lines().next().unwrap_or_default().to_owned(),
        None => params(input),
    }
}

/// Whether a call's output says it failed: an error, a nonzero exit, a
/// timeout, or a cancel. The mark beside the row is red when it did.
#[must_use]
pub fn failed(output: &Value) -> bool {
    output.get("error").is_some()
        || output
            .get("exit")
            .and_then(Value::as_i64)
            .is_some_and(|exit| exit != 0)
        || output.get("timed_out").and_then(Value::as_bool) == Some(true)
        || output.get("canceled").and_then(Value::as_bool) == Some(true)
}

/// A call's whole input, for a reader who opens the row: a command-line
/// tool's command line, a shell call's command, else its parameters as
/// indented JSON.
#[must_use]
pub fn input_text(input: &Value) -> String {
    if let Some(line) = argument_line(input) {
        return line;
    }
    if let Some(object) = input.as_object()
        && object.len() == 1
        && let Some(command) = object.get("command").and_then(Value::as_str)
    {
        return command.to_owned();
    }
    match input {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// A call's whole output, for a reader who opens the row: a command's
/// standard output and error, a text result's text, an error's words,
/// else the output as indented JSON.
#[must_use]
pub fn output_text(output: &Value) -> String {
    match output {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Object(object) => {
            if let Some(error) = object.get("error").and_then(Value::as_str) {
                return error.to_owned();
            }
            let stdout = object.get("stdout").and_then(Value::as_str);
            let stderr = object.get("stderr").and_then(Value::as_str);
            if stdout.is_some() || stderr.is_some() {
                let mut text = stdout.unwrap_or_default().trim_end().to_owned();
                let stderr = stderr.unwrap_or_default().trim_end();
                if !stderr.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(stderr);
                }
                return text;
            }
            for key in ["output", "text", "content"] {
                if object.len() == 1
                    && let Some(text) = object.get(key).and_then(Value::as_str)
                {
                    return text.to_owned();
                }
            }
            serde_json::to_string_pretty(output).unwrap_or_default()
        }
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// A tool call on one line: its name, then [`params`] when it has any.
#[must_use]
pub fn call_line(name: &str, input: &Value) -> String {
    join(name, &params(input))
}

fn join(name: &str, rest: &str) -> String {
    if rest.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {rest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_command_line_tools_arguments_read_as_its_command_line() {
        let input = json!({"arguments": ["projects", "show", "finances"]});
        assert_eq!(params(&input), "projects show finances");
        assert_eq!(
            call_line("OpenAgents CLI", &input),
            "OpenAgents CLI projects show finances"
        );
        let quoted = json!({"args": ["issue", "create", "--title", "Fix the rail"]});
        assert_eq!(params(&quoted), "issue create --title 'Fix the rail'");
        // Anything else keeps the key: value form.
        assert!(params(&json!({"arguments": ["a"], "cwd": "x"})).contains("cwd: x"));
    }

    #[test]
    fn a_path_comes_first_and_a_command_reads_bare() {
        assert_eq!(
            params(&json!({"limit": 5, "file_path": "src/main.rs"})),
            "src/main.rs · limit: 5"
        );
        assert_eq!(params(&json!({"command": "cargo test"})), "cargo test");
        assert_eq!(call_line("Glob", &Value::Null), "Glob");
    }

    #[test]
    fn an_opened_row_shows_the_whole_input_and_output() {
        assert_eq!(
            input_text(&json!({"arguments": ["projects", "show", "finances"]})),
            "projects show finances"
        );
        assert_eq!(input_text(&json!({"command": "ls -la"})), "ls -la");
        assert!(input_text(&json!({"file_path": "a.rs", "limit": 4})).contains("\"limit\": 4"));
        assert_eq!(
            output_text(&json!({"exit": 1, "stdout": "out\n", "stderr": "err"})),
            "out\nerr"
        );
        assert_eq!(
            output_text(&json!({"error": "No such file"})),
            "No such file"
        );
        assert_eq!(output_text(&json!({"text": "fn main"})), "fn main");
        assert!(failed(&json!({"exit": 2})) && !failed(&json!({"exit": 0})));
    }

    #[test]
    fn thinking_and_notices_read_as_their_words() {
        let output = json!({"text": " I'll check the help. "});
        assert_eq!(
            Row::of(THINKING, &Value::Null, &output),
            Some(Row::Thinking("I'll check the help.".into()))
        );
        assert_eq!(
            Row::of(NOTICE, &Value::Null, &json!({"text": "Retrying"})),
            Some(Row::Notice("Retrying".into()))
        );
        assert_eq!(Row::of(THINKING, &Value::Null, &json!({})), None);
        assert_eq!(
            Row::of("Bash", &json!({"command": "ls"}), &Value::Null),
            Some(Row::Call("Bash ls".into()))
        );
        // A failed call says why, not what it was sent.
        assert_eq!(
            Row::of(
                "Bash",
                &json!({"command": "ls"}),
                &json!({"error": "No such folder\nmore"})
            ),
            Some(Row::Call("Bash No such folder".into()))
        );
    }
}
