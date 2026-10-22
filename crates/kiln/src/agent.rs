//! The agent loop, Microcoder-style and portable: each step the model sees
//! the task, the workspace's files, and what the last commands printed, and
//! returns one JSON object with its next commands. The host runs each
//! command as a WASI tool in the sandbox and records what it printed.
//!
//! This holds no I/O. The page drives it: [`Agent::prompt`], then the
//! model, then [`Agent::plan`], then each [`Call`] through a worker, then
//! [`Agent::observe`].

use serde::Deserialize;
use serde_json::Value;

/// The tools a command may name.
pub const TOOLS: &[&str] = &[
    "ls", "cat", "view", "grep", "write", "patch", "rhai", "test", "cargo",
];

/// The system turn for every step.
pub const INSTRUCTIONS: &str = r#"You are Kiln, a coding agent working in a sandboxed workspace inside a web browser. You change files and run programs only through commands, and you see only what they print.

Tools (each runs as a WebAssembly program):
- ls [-R] [PATH]          list files
- cat PATH...             print files
- grep [-i] TEXT [PATH]   search lines
- rhai FILE               run a Rhai script (Rhai is a small Rust-like scripting language)
- test [DIR]              run every *_test.rhai file; each fn test_*() is one test

Rhai notes: `fn fib(n) { if n < 2 { return n; } fib(n - 1) + fib(n - 2) }`, `let x = 5;`, `print(x);`, `for i in 0..10 { }`, arrays `[1, 2]`, strings "a" + "b". Functions can't read variables outside them. A test file imports code with `import "fib" as f;` (no .rhai extension, path relative to the test file) and calls `f::fib(10)`. Assertions: assert(cond), assert(cond, "msg"), assert_eq(left, right).

Reply with ONLY one JSON object, no prose and no code fences:
{"thought": "one short sentence", "commands": [ ... ]}

Each command is one of:
{"run": "test"}                                   run a tool command line
{"write": "PATH", "content": "full file text"}     create or replace a file
{"patch": "PATH", "search": "exact old text", "replace": "new text"}
{"done": "what you did, in one sentence"}

Work in small steps: write the code and a test, run the test, read the output, fix what failed. Say done only after the tests pass."#;

/// The system turn when the workspace is a crate from this repository and
/// `cargo` runs on a remote runner. `{crates}` names the crates.
pub const RUST_INSTRUCTIONS: &str = r#"You are Kiln, a coding agent working on a Rust crate from the OpenAgents repository. The crate's files are in a sandboxed workspace inside a web browser, at their repository paths. You change files and run programs only through commands, and you see only what they print.

Tools:
- ls [-R] [PATH]          list files
- view PATH START END     print lines START to END with their numbers (at most 80)
- grep [-i] TEXT [PATH]   find lines; prints path:line:text (TEXT may be a\|b)
- cargo test -p CRATE [-- FILTER]   build and run the crate's tests on a remote runner (takes 10 s to a few minutes)
- cargo check -p CRATE              type-check the crate on the remote runner
The crates you can test: {crates}. The runner applies your changed files to the repository at the loaded commit, with no network. You can't change Cargo.toml or build.rs, and you can only change files inside the crate. Other repository paths (another crate, docs) load the first time ls, view, or grep names them, so you can read them for context.

Reply with ONLY one JSON object, no prose and no code fences:
{"thought": "one short sentence", "commands": [ ... ]}

Each command is one of:
{"run": "cargo test -p CRATE"}                     run a tool command line
{"write": "PATH", "content": "full file text"}     create or replace a file (use the full repository path)
{"patch": "PATH", "search": "exact old text", "replace": "new text"}
{"done": "what you did, in one sentence"}

You see only a small window: never cat a whole source file. Find places with grep, then read around them with view. Prefer patch for edits to existing files (don't include view's line numbers in the search text): copy the search text exactly from view's output, including indentation, and make it long enough to be unique. Read the code you change first. Keep the crate's style and documentation comments. Work in small steps, run the tests after each change, read the failures, and fix them. When `cargo test` passes, publish the change, then say done."#;

/// One command, as the model wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(String),
    Write {
        path: String,
        content: String,
    },
    Patch {
        path: String,
        search: String,
        replace: String,
    },
    Done(String),
    /// Publish the workspace's change as a NIP-34 patch.
    Publish {
        subject: String,
        body: String,
    },
}

/// A tool run: the argument vector (tool first) and standard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub argv: Vec<String>,
    pub stdin: Vec<u8>,
}

/// What the model returned for one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub thought: String,
    pub commands: Vec<Command>,
}

/// One finished command and what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub command: String,
    pub code: i32,
    pub output: String,
}

/// One step of the loop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Step {
    pub thought: String,
    pub observations: Vec<Observation>,
    /// Why the reply could not be used, when it couldn't.
    pub problem: Option<String>,
}

/// The loop's state.
#[derive(Debug, Clone)]
pub struct Agent {
    pub task: String,
    pub steps: Vec<Step>,
    pub done: Option<String>,
    /// The most request text the model's Pylon accepts, in bytes.
    pub budget: usize,
    /// The system turn.
    pub instructions: String,
}

/// Split a command line into words, honoring single and double quotes.
///
/// # Errors
///
/// When a quote is not closed.
pub fn split(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            (None, c) => {
                word.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("a quote is not closed".into());
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

/// The JSON object in a model reply: the whole text, or the text inside a
/// code fence, or the span from the first `{` to the last `}`.
fn json_object(text: &str) -> Option<Value> {
    let text = text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return value.is_object().then_some(value);
    }
    // The first complete object; a model sometimes writes its next step's
    // object after this one.
    let start = text.find('{')?;
    serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()?
        .ok()
        .filter(Value::is_object)
}

#[derive(Deserialize)]
struct RawPlan {
    #[serde(default)]
    thought: String,
    #[serde(default)]
    commands: Vec<Value>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A command written as a tool object, such as
/// `{"view": "src/lib.rs", "start": 10, "end": 40}` or
/// `{"grep": "fn x", "path": "src"}`: the command line it means.
fn tool_object(c: &Value) -> Option<String> {
    let object = c.as_object()?;
    let tool = TOOLS
        .iter()
        .find(|t| object.contains_key(**t) && !matches!(**t, "write" | "patch"))?;
    let quote = |s: &str| {
        if s.is_empty() || s.contains(char::is_whitespace) || s.contains('"') {
            format!("'{}'", s.replace('\'', ""))
        } else {
            s.to_string()
        }
    };
    let word = |v: &Value| match v {
        Value::String(s) => Some(quote(s)),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    let mut line = vec![(*tool).to_string()];
    // A cargo or test value is a whole argument string.
    match (&object[*tool], *tool) {
        (Value::String(s), "cargo") => line.push(s.clone()),
        (v, _) => line.extend(word(v)),
    }
    for key in ["pattern", "path", "file", "start", "end", "dir"] {
        if let Some(v) = object.get(key) {
            line.extend(word(v));
        }
    }
    Some(line.join(" "))
}

/// Commands written in DeepSeek's own tool-call markup instead of JSON:
/// `<|DSML|invoke name="view"><|DSML|parameter name="path" ...>x</...>`
/// blocks (the bars may be full width). Each invoke becomes a command
/// object: `{"view": PATH, "start": .., "end": ..}`, `{"patch": PATH,
/// "search": .., "replace": ..}`, `{"run": COMMAND}`, and so on.
fn dsml_commands(text: &str) -> Option<Vec<Value>> {
    // DeepSeek writes full-width bars, sometimes with a space after the tag
    // name's bar (`<｜DSML｜ invoke`).
    let text = text.replace('\u{ff5c}', "|").replace("|DSML| ", "|DSML|");
    if !text.contains("|DSML|invoke") {
        return None;
    }
    let mut commands = Vec::new();
    for block in text.split("|DSML|invoke name=\"").skip(1) {
        let name = block.split('"').next()?.to_string();
        let mut params = serde_json::Map::new();
        for param in block.split("|DSML|parameter name=\"").skip(1) {
            let key = param.split('"').next()?.to_string();
            let after = param.find('>')? + 1;
            let rest = &param[after..];
            let end = rest.find("</|DSML|parameter").or_else(|| rest.find("</"))?;
            params.insert(key, Value::String(rest[..end].to_string()));
        }
        let mut object = serde_json::Map::new();
        let main = ["path", "command", "line", "summary", "subject", "text"]
            .iter()
            .find_map(|k| params.remove(*k));
        object.insert(name, main.unwrap_or(Value::String(String::new())));
        object.extend(params);
        commands.push(Value::Object(object));
    }
    (!commands.is_empty()).then_some(commands)
}

/// Parse a model reply into a [`Plan`].
///
/// # Errors
///
/// When the reply holds no JSON object, or a command has no known shape.
pub fn parse(reply: &str) -> Result<Plan, String> {
    let value = match json_object(reply) {
        Some(value) => value,
        None => match dsml_commands(reply) {
            Some(commands) => serde_json::json!({"thought": "", "commands": commands}),
            None => return Err("the reply is not a JSON object".into()),
        },
    };
    let raw: RawPlan =
        serde_json::from_value(value).map_err(|e| format!("the reply's shape: {e}"))?;
    let mut commands = Vec::new();
    for c in raw.commands {
        let command = if let Some(line) = text(&c, "run") {
            Command::Run(line)
        } else if let Some(path) = text(&c, "write") {
            Command::Write {
                path,
                content: text(&c, "content").unwrap_or_default(),
            }
        } else if let Some(path) = text(&c, "patch") {
            Command::Patch {
                path,
                search: text(&c, "search").ok_or("a patch has no \"search\"")?,
                replace: text(&c, "replace").unwrap_or_default(),
            }
        } else if let Some(subject) = text(&c, "publish") {
            Command::Publish {
                subject,
                body: text(&c, "body").unwrap_or_default(),
            }
        } else if let Some(summary) = text(&c, "done") {
            Command::Done(summary)
        } else if let Some(line) = tool_object(&c) {
            Command::Run(line)
        } else {
            return Err(format!("unknown command {c}"));
        };
        commands.push(command);
    }
    if commands.is_empty() {
        return Err("the reply has no commands".into());
    }
    Ok(Plan {
        thought: raw.thought,
        commands,
    })
}

impl Command {
    /// The command as one line for the log.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Run(line) => line.clone(),
            Self::Write { path, content } => format!("write {path} ({} bytes)", content.len()),
            Self::Patch { path, .. } => format!("patch {path}"),
            Self::Done(summary) => format!("done: {summary}"),
            Self::Publish { subject, .. } => format!("publish patch: {subject}"),
        }
    }

    /// The tool run this command is, or `None` for `done`.
    ///
    /// # Errors
    ///
    /// When a command line doesn't parse or names an unknown tool.
    pub fn call(&self) -> Result<Option<Call>, String> {
        match self {
            Self::Run(line) => {
                let argv = split(line)?;
                let tool = argv.first().ok_or("an empty command")?;
                if !TOOLS.contains(&tool.as_str()) {
                    return Err(format!(
                        "no tool named {tool}; the tools are {}",
                        TOOLS.join(", ")
                    ));
                }
                Ok(Some(Call {
                    argv,
                    stdin: Vec::new(),
                }))
            }
            Self::Write { path, content } => Ok(Some(Call {
                argv: vec!["write".into(), path.clone()],
                stdin: content.clone().into_bytes(),
            })),
            Self::Patch {
                path,
                search,
                replace,
            } => Ok(Some(Call {
                argv: vec!["patch".into(), path.clone()],
                stdin: format!("<<<<<<< SEARCH\n{search}\n=======\n{replace}\n>>>>>>> REPLACE\n")
                    .into_bytes(),
            })),
            Self::Done(_) | Self::Publish { .. } => Ok(None),
        }
    }
}

/// Keep the head and tail of `text` within `max` bytes.
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut head = max * 2 / 3;
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - (max - head);
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n[... {} bytes cut ...]\n{}",
        &text[..head],
        tail - head,
        &text[tail..]
    )
}

impl Agent {
    #[must_use]
    pub fn new(task: &str, budget: usize) -> Self {
        Self {
            task: task.trim().to_string(),
            steps: Vec::new(),
            done: None,
            budget,
            instructions: INSTRUCTIONS.to_string(),
        }
    }

    /// The user turn for the next step: the task, the files, and the
    /// recent steps, within the budget less the instructions.
    #[must_use]
    pub fn prompt(&self, files: &str) -> String {
        let room = self
            .budget
            .saturating_sub(self.instructions.len() + self.task.len() + 600);
        let files = clip(files, room / 4);
        let output_cap = if self.budget > 32 * 1024 { 6000 } else { 1500 };
        let mut history = Vec::new();
        let mut used = files.len();
        for (n, step) in self.steps.iter().enumerate().rev() {
            let mut block = format!("Step {}: {}\n", n + 1, step.thought);
            if let Some(problem) = &step.problem {
                block.push_str(&format!("Your reply could not be used: {problem}\n"));
            }
            for o in &step.observations {
                block.push_str(&format!(
                    "$ {} (exit {})\n{}\n",
                    o.command,
                    o.code,
                    clip(o.output.trim_end(), output_cap)
                ));
            }
            if used + block.len() > room {
                history.push(format!("[{} earlier steps omitted]\n", n + 1));
                break;
            }
            used += block.len();
            history.push(block);
        }
        history.reverse();
        let history = if history.is_empty() {
            "No steps yet.".to_string()
        } else {
            history.concat()
        };
        format!(
            "Task: {}\n\nFiles in the workspace:\n{}\n\nWhat happened so far:\n{}\nReply with the JSON object for your next step.",
            self.task,
            if files.trim().is_empty() {
                "(none)"
            } else {
                files.trim_end()
            },
            history
        )
    }

    /// Start a step from the model's reply. Returns the plan's commands, or
    /// none when the reply was unusable (recorded for the next prompt).
    pub fn plan(&mut self, reply: &str) -> Vec<Command> {
        match parse(reply) {
            Ok(plan) => {
                self.steps.push(Step {
                    thought: plan.thought,
                    ..Step::default()
                });
                plan.commands
            }
            Err(problem) => {
                self.steps.push(Step {
                    thought: "(unusable reply)".into(),
                    problem: Some(format!("{problem}. Reply with only the JSON object.")),
                    ..Step::default()
                });
                Vec::new()
            }
        }
    }

    /// Record a command's result in the current step.
    pub fn observe(&mut self, command: &Command, code: i32, output: String) {
        if let Command::Done(summary) = command {
            self.done = Some(summary.clone());
            return;
        }
        if let Some(step) = self.steps.last_mut() {
            step.observations.push(Observation {
                command: command.label(),
                code,
                output,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_split_with_quotes() {
        assert_eq!(
            split("grep -i 'fn fib' src").unwrap(),
            ["grep", "-i", "fn fib", "src"]
        );
        assert_eq!(split("ls \"\"").unwrap(), ["ls", ""]);
        assert!(split("cat 'x").is_err());
    }

    #[test]
    fn a_fenced_reply_parses_into_calls() {
        let reply = "Sure!\n```json\n{\"thought\":\"t\",\"commands\":[{\"write\":\"a.rhai\",\"content\":\"print(1);\"},{\"run\":\"rhai a.rhai\"},{\"patch\":\"a.rhai\",\"search\":\"1\",\"replace\":\"2\"},{\"done\":\"ok\"}]}\n```";
        let plan = parse(reply).unwrap();
        assert_eq!(plan.commands.len(), 4);
        let write = plan.commands[0].call().unwrap().unwrap();
        assert_eq!(write.argv, ["write", "a.rhai"]);
        assert_eq!(write.stdin, b"print(1);");
        let patch = plan.commands[2].call().unwrap().unwrap();
        assert!(
            String::from_utf8(patch.stdin)
                .unwrap()
                .starts_with("<<<<<<< SEARCH\n1\n")
        );
        assert_eq!(plan.commands[3].call().unwrap(), None);
        assert!(Command::Run("bash -c x".into()).call().is_err());
        assert!(Command::Run("cargo test -p x".into()).call().is_ok());
        let two = "{\"thought\":\"a\",\"commands\":[{\"run\":\"ls\"}]}\n{\"thought\":\"b\",\"commands\":[{\"run\":\"test\"}]}";
        assert_eq!(parse(two).unwrap().thought, "a");
        let tools = r#"{"thought":"t","commands":[{"view":"src/lib.rs","start":28,"end":45},{"grep":"fn a","path":"src"},{"cargo":"test -p x"}]}"#;
        let plan = parse(tools).unwrap();
        assert_eq!(
            plan.commands[0],
            Command::Run("view src/lib.rs 28 45".into())
        );
        assert_eq!(plan.commands[1], Command::Run("grep 'fn a' src".into()));
        assert_eq!(plan.commands[2], Command::Run("cargo test -p x".into()));
        let dsml = "<\u{ff5c}DSML\u{ff5c}calls>\n<\u{ff5c}DSML\u{ff5c}invoke name=\"view\">\n<\u{ff5c}DSML\u{ff5c}parameter name=\"path\" string=\"true\">src/lib.rs</\u{ff5c}DSML\u{ff5c}parameter>\n<\u{ff5c}DSML\u{ff5c} parameter name=\"start\" string=\"false\">18</\u{ff5c}DSML\u{ff5c} parameter>\n</\u{ff5c}DSML\u{ff5c}invoke>\n<|DSML|invoke name=\"patch\"><|DSML|parameter name=\"path\" string=\"true\">a.rs</|DSML|parameter><|DSML|parameter name=\"search\" string=\"true\">x</|DSML|parameter><|DSML|parameter name=\"replace\" string=\"true\">y</|DSML|parameter></|DSML|invoke>";
        let plan = parse(dsml).unwrap();
        let spaced = "<\u{ff5c}DSML\u{ff5c} invoke name=\"run\">\n<\u{ff5c}DSML\u{ff5c} parameter name=\"command\" string=\"true\">cargo test -p x</\u{ff5c}DSML\u{ff5c} parameter>\n</\u{ff5c}DSML\u{ff5c} invoke>";
        assert_eq!(
            parse(spaced).unwrap().commands[0],
            Command::Run("cargo test -p x".into())
        );
        assert_eq!(plan.commands[0], Command::Run("view src/lib.rs 18".into()));
        assert_eq!(
            plan.commands[1],
            Command::Patch {
                path: "a.rs".into(),
                search: "x".into(),
                replace: "y".into()
            }
        );
    }

    #[test]
    fn the_loop_records_steps_and_bad_replies() {
        let mut agent = Agent::new("make fib", 16 * 1024);
        assert!(agent.plan("no json here").is_empty());
        let commands =
            agent.plan(r#"{"thought":"run","commands":[{"run":"test"},{"done":"fine"}]}"#);
        agent.observe(&commands[0], 1, "FAIL x".into());
        agent.observe(&commands[1], 0, String::new());
        assert_eq!(agent.done.as_deref(), Some("fine"));
        let prompt = agent.prompt("fib.rhai  10\n");
        assert!(prompt.contains("could not be used"));
        assert!(prompt.contains("$ test (exit 1)\nFAIL x"));
    }

    #[test]
    fn the_prompt_stays_within_the_budget() {
        let mut agent = Agent::new("t", 8 * 1024);
        for _ in 0..40 {
            let commands = agent.plan(r#"{"thought":"x","commands":[{"run":"cat big"}]}"#);
            agent.observe(&commands[0], 0, "y".repeat(5000));
        }
        let prompt = agent.prompt(&"f\n".repeat(5000));
        assert!(
            prompt.len() + INSTRUCTIONS.len() < 8 * 1024,
            "{}",
            prompt.len()
        );
        assert!(prompt.contains("earlier steps omitted"));
        assert!(clip("ééééé", 4).len() < 40);
    }
}
