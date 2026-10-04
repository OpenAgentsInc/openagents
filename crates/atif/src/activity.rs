//! What a step shows an agent doing, and where a studio draws it.
//!
//! Agent Studio (`docs/verse/agent-studio.md`) animates every engine the
//! same way by reading one normalized stream, the session's ATIF steps,
//! instead of engine-specific logs. [`classify`] turns one [`Step`] into an
//! [`Activity`] and the [`Station`] it happens at. A Verse replay and a live
//! studio both call it, so the two never place a step differently.
//!
//! | Evidence | Activity | Station |
//! | --- | --- | --- |
//! | A file read, a search, a glob, a web read, knowledge retrieval | `reading` | library |
//! | A file write, an edit, a patch | `editing` | desk |
//! | A shell command that is not a check | `running` | workbench |
//! | A shell command that is a check, acceptance tests, a verifier | `testing` | proving ground |
//! | A Jev, Kev, or Lev decision call | `judging` | oracle |
//! | A plan or to-do update | `thinking` | Task Wall |
//! | A question or approval for a person | `waiting` | podium |
//! | A call something refused before it ran | `blocked` | lounge |
//! | The finish call | `done`, or `failed` when it failed | Task Wall |
//! | A model step with no call, an MCP tool, or anything else | `thinking` | desk |
//!
//! The tool names cover the closed `ToolCall` set Zeron normalizes engines
//! into (`Exec`, `ReadFile`, `WriteFile`, `EditFile`, `ApplyPatch`,
//! `Search`, `Glob`, `WebFetch`, `WebSearch`, `Todo`, `Mcp`, and `Unknown`)
//! under the names the engines Coder runs give them. An unknown call
//! classifies as `thinking` at the desk, never as a guess at another
//! station.
//!
//! A task's summary phase can also say `waiting`, `blocked`, `done`, or
//! `failed` without any step; the host reads that from the task, not here.

use serde_json::Value;

use crate::document::{Outcome, Step};

/// The key in [`crate::Call::extra`] that says whether a command is a
/// check. When it is present it decides; when it is absent, the command's
/// text does.
pub const CHECK: &str = "check";

/// What an agent is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Activity {
    /// Reading files, searching, or retrieving knowledge.
    Reading,
    /// Writing, editing, or patching files.
    Editing,
    /// Running a shell command that is not a check.
    Running,
    /// Running tests, a verifier, or another check.
    Testing,
    /// Asking a decision model.
    Judging,
    /// A model step with no tool call, or a call no rule recognizes.
    Thinking,
    /// Waiting on a person's answer or approval.
    Waiting,
    /// Refused: a missing grant, or no provider with capacity.
    Blocked,
    /// Finished.
    Done,
    /// Finished and failed.
    Failed,
}

impl Activity {
    /// The word a snapshot and a nameplate spell the activity with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Activity::Reading => "reading",
            Activity::Editing => "editing",
            Activity::Running => "running",
            Activity::Testing => "testing",
            Activity::Judging => "judging",
            Activity::Thinking => "thinking",
            Activity::Waiting => "waiting",
            Activity::Blocked => "blocked",
            Activity::Done => "done",
            Activity::Failed => "failed",
        }
    }

    /// The station the activity happens at, when nothing about the step
    /// says otherwise.
    #[must_use]
    pub fn station(self) -> Station {
        match self {
            Activity::Reading => Station::Library,
            Activity::Editing | Activity::Thinking => Station::Desk,
            Activity::Running => Station::Workbench,
            Activity::Testing => Station::ProvingGround,
            Activity::Judging => Station::Oracle,
            Activity::Waiting => Station::Podium,
            Activity::Blocked => Station::Lounge,
            Activity::Done | Activity::Failed => Station::TaskWall,
        }
    }
}

/// Where in the studio an activity is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Station {
    /// Reading and searching.
    Library,
    /// The seat's own desk: editing and thinking.
    Desk,
    /// Shell commands.
    Workbench,
    /// Checks.
    ProvingGround,
    /// Decision models.
    Oracle,
    /// Waiting on a person.
    Podium,
    /// Paused, stopped, and blocked seats.
    Lounge,
    /// Plans and finished tasks.
    TaskWall,
}

impl Station {
    /// The station's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Station::Library => "library",
            Station::Desk => "desk",
            Station::Workbench => "workbench",
            Station::ProvingGround => "proving ground",
            Station::Oracle => "oracle",
            Station::Podium => "podium",
            Station::Lounge => "lounge",
            Station::TaskWall => "Task Wall",
        }
    }
}

/// One step's activity and station.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Classified {
    pub activity: Activity,
    pub station: Station,
}

impl From<Activity> for Classified {
    fn from(activity: Activity) -> Self {
        Classified {
            activity,
            station: activity.station(),
        }
    }
}

/// What a step shows the agent doing, and where.
#[must_use]
pub fn classify(step: &Step) -> Classified {
    let Some(call) = &step.call else {
        return Activity::Thinking.into();
    };
    if call.outcome == Outcome::Cancelled {
        return Activity::Blocked.into();
    }
    if call.is_decision() {
        return Activity::Judging.into();
    }
    let name = normalized(&call.name);
    if name.starts_with("mcp__") || name.starts_with("mcp.") || name.starts_with("mcp:") {
        return Activity::Thinking.into();
    }
    match name.as_str() {
        "shell" | "bash" | "exec" | "exec_command" | "local_shell" | "run_command"
        | "run_terminal_cmd" | "terminal" | "command" => {
            if is_check(&call.arguments, call.extra.get(CHECK)) {
                Activity::Testing.into()
            } else {
                Activity::Running.into()
            }
        }
        "acceptance_tests" | "tested" | "verifier" | "verified" | "check" | "run_tests" => {
            Activity::Testing.into()
        }
        "read" | "read_file" | "view" | "open_file" | "cat" | "notebookread" | "grep"
        | "search" | "search_files" | "codebase_search" | "grep_search" | "glob" | "find_files"
        | "file_search" | "ls" | "list" | "list_dir" | "list_files" | "webfetch" | "web_fetch"
        | "fetch" | "websearch" | "web_search" | "retrieve" | "retrieved" | "knowledge" => {
            Activity::Reading.into()
        }
        "write"
        | "write_file"
        | "create_file"
        | "edit"
        | "edit_file"
        | "multiedit"
        | "multi_edit"
        | "str_replace"
        | "str_replace_editor"
        | "str_replace_based_edit_tool"
        | "notebookedit"
        | "apply_patch"
        | "patch" => Activity::Editing.into(),
        "todo" | "todowrite" | "todo_write" | "todoread" | "update_plan" | "plan" => Classified {
            activity: Activity::Thinking,
            station: Station::TaskWall,
        },
        "askuserquestion" | "ask_user" | "request_permission" | "request_user_input"
        | "question" | "approval" => Activity::Waiting.into(),
        "finish" | "submit" | "task_complete" | "complete" | "done" | "ended" => {
            if call.outcome == Outcome::Failed {
                Activity::Failed.into()
            } else {
                Activity::Done.into()
            }
        }
        _ => Activity::Thinking.into(),
    }
}

/// A tool name in lower case, with the separators engines differ on
/// (`read-file`, `ReadFile`) folded where the folding is unambiguous.
fn normalized(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

/// Whether a command is a check: the call says so, or its text runs a test
/// runner.
fn is_check(arguments: &Value, marked: Option<&Value>) -> bool {
    if let Some(marked) = marked.and_then(Value::as_bool) {
        return marked;
    }
    let command = match arguments {
        Value::String(text) => text.as_str(),
        _ => arguments
            .get("command")
            .or_else(|| arguments.get("cmd"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    };
    is_check_command(command)
}

/// Whether a command's text runs a test runner.
#[must_use]
pub fn is_check_command(command: &str) -> bool {
    let words: Vec<&str> = command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')'))
        .filter(|w| !w.is_empty())
        .collect();
    words.iter().enumerate().any(|(i, word)| {
        let program = word.rsplit('/').next().unwrap_or(word);
        let next = words.get(i + 1).copied().unwrap_or_default();
        match program {
            "pytest" | "py.test" | "jest" | "vitest" | "mocha" | "rspec" | "phpunit" | "ctest"
            | "nextest" => true,
            "cargo" | "go" | "npm" | "pnpm" | "yarn" | "bun" | "make" | "dotnet" | "mvn"
            | "gradle" | "swift" | "mix" => matches!(next, "test" | "nextest" | "check"),
            _ => program.starts_with("test_") || program.starts_with("run_tests"),
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::*;
    use crate::document::{Call, Decision, Source};

    fn call(name: &str, arguments: Value) -> Step {
        Step::called(Call {
            id: "c1".into(),
            name: name.into(),
            arguments,
            output: String::new(),
            outcome: Outcome::Completed,
            milliseconds: 1,
            purpose: None,
            extra: Map::new(),
        })
    }

    fn at(step: &Step) -> (Activity, Station) {
        let c = classify(step);
        (c.activity, c.station)
    }

    // One fixture per Zeron `ToolCall` kind, under a name an engine Coder
    // runs gives it.

    #[test]
    fn exec_is_the_workbench_or_the_proving_ground_for_a_check() {
        assert_eq!(
            at(&call("shell", json!({"command": "ls -la"}))),
            (Activity::Running, Station::Workbench)
        );
        assert_eq!(
            at(&call(
                "Bash",
                json!({"command": "cd app && cargo test -p x"})
            )),
            (Activity::Testing, Station::ProvingGround)
        );
        let mut marked = call("shell", json!({"command": "./grade.sh"}));
        marked
            .call
            .as_mut()
            .unwrap()
            .extra
            .insert(CHECK.into(), json!(true));
        assert_eq!(at(&marked).1, Station::ProvingGround);
        let mut unmarked = call("shell", json!({"command": "pytest -q"}));
        unmarked
            .call
            .as_mut()
            .unwrap()
            .extra
            .insert(CHECK.into(), json!(false));
        assert_eq!(at(&unmarked).1, Station::Workbench, "the mark decides");
    }

    #[test]
    fn read_file_is_the_library() {
        assert_eq!(
            at(&call("Read", json!({"file_path": "a.rs"}))),
            (Activity::Reading, Station::Library)
        );
    }

    #[test]
    fn write_file_is_the_desk() {
        assert_eq!(
            at(&call("Write", json!({"file_path": "a.rs"}))),
            (Activity::Editing, Station::Desk)
        );
    }

    #[test]
    fn edit_file_is_the_desk() {
        assert_eq!(
            at(&call("Edit", json!({"file_path": "a.rs"}))),
            (Activity::Editing, Station::Desk)
        );
    }

    #[test]
    fn apply_patch_is_the_desk() {
        assert_eq!(
            at(&call("apply_patch", json!({"input": "*** Begin Patch"}))),
            (Activity::Editing, Station::Desk)
        );
    }

    #[test]
    fn search_is_the_library() {
        assert_eq!(
            at(&call("Grep", json!({"pattern": "fn main"}))),
            (Activity::Reading, Station::Library)
        );
    }

    #[test]
    fn glob_is_the_library() {
        assert_eq!(
            at(&call("Glob", json!({"pattern": "**/*.rs"}))),
            (Activity::Reading, Station::Library)
        );
    }

    #[test]
    fn web_fetch_is_the_library() {
        assert_eq!(
            at(&call("WebFetch", json!({"url": "https://example.com"}))),
            (Activity::Reading, Station::Library)
        );
    }

    #[test]
    fn web_search_is_the_library() {
        assert_eq!(
            at(&call("web_search", json!({"query": "atif"}))),
            (Activity::Reading, Station::Library)
        );
    }

    #[test]
    fn todo_is_the_task_wall() {
        assert_eq!(
            at(&call("TodoWrite", json!({"todos": []}))),
            (Activity::Thinking, Station::TaskWall)
        );
        assert_eq!(at(&call("update_plan", json!({}))).1, Station::TaskWall);
    }

    #[test]
    fn mcp_is_the_desk() {
        assert_eq!(
            at(&call("mcp__github__create_issue", json!({}))),
            (Activity::Thinking, Station::Desk)
        );
    }

    #[test]
    fn unknown_is_thinking_at_the_desk() {
        assert_eq!(
            at(&call("frobnicate", json!({}))),
            (Activity::Thinking, Station::Desk)
        );
        assert_eq!(
            at(&Step::said(Source::Agent, "Looking at the layout.")),
            (Activity::Thinking, Station::Desk),
            "a model step with no call"
        );
    }

    #[test]
    fn a_decision_call_is_the_oracle() {
        let step = Step::called(
            Decision {
                id: "d1".into(),
                name: "classify".into(),
                door: "jev".into(),
                model: "jev".into(),
                ..Decision::default()
            }
            .call(),
        );
        assert_eq!(at(&step), (Activity::Judging, Station::Oracle));
    }

    #[test]
    fn a_refused_call_waits_in_the_lounge_and_a_question_at_the_podium() {
        let mut refused = call("shell", json!({"command": "rm -rf /"}));
        refused.call.as_mut().unwrap().outcome = Outcome::Cancelled;
        assert_eq!(at(&refused), (Activity::Blocked, Station::Lounge));
        assert_eq!(
            at(&call("AskUserQuestion", json!({}))),
            (Activity::Waiting, Station::Podium)
        );
    }

    #[test]
    fn the_finish_is_done_or_failed_at_the_task_wall() {
        assert_eq!(
            at(&call("finish", json!({}))),
            (Activity::Done, Station::TaskWall)
        );
        let mut failed = call("finish", json!({}));
        failed.call.as_mut().unwrap().outcome = Outcome::Failed;
        assert_eq!(at(&failed).0, Activity::Failed);
    }

    #[test]
    fn check_commands_are_recognized_by_their_runner() {
        for command in [
            "pytest -q",
            "python -m pytest tests/",
            "cargo test -p atif",
            "npm test",
            "go test ./...",
            "bash tests/test_outputs.sh",
            "/usr/bin/pytest",
        ] {
            assert!(is_check_command(command), "{command}");
        }
        for command in ["ls -la", "cargo build", "cat test.txt", "npm install"] {
            assert!(!is_check_command(command), "{command}");
        }
    }
}
