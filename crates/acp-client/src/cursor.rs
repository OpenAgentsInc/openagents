//! The Cursor CLI as an ACP agent: `cursor-agent acp`.
//!
//! Cursor's agent is an ACP server on standard input and output. It
//! advertises one sign-in method, `cursor_login`, which reuses the login
//! that `cursor-agent login` stored (or `CURSOR_API_KEY`); a client
//! authenticates with it after `initialize`, and an agent that is not
//! signed in refuses or, in some builds, never answers. [`signed_in`] asks
//! `cursor-agent status --format json` first, so a missing login is named
//! at once. Sessions offer the `agent`, `plan`, and `ask` modes.
//!
//! Cursor also sends its own `cursor/*` methods. Two are requests the agent
//! waits on: `cursor/ask_question` and `cursor/create_plan`. A delegated
//! session has no one to ask, so [`answer`] skips the question and accepts
//! the plan, and the turn goes on. The others are notifications that
//! [`Notice::parse`] types for display. The design follows Cursor's
//! published ACP contract (<https://cursor.com/docs/cli/acp>) and the
//! `cursor-agent` 2026.06.24 recordings in `fixtures/`, reimplemented here.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::process::{first_executable, on_path};

/// The variable that names the binary.
pub const BIN_VAR: &str = "CURSOR_AGENT_BIN";
/// The sign-in method Cursor advertises.
pub const AUTH_METHOD: &str = "cursor_login";
/// The credentials Cursor reads instead of a stored login. They reach only
/// the Cursor agent.
pub const CREDENTIAL_VARS: [&str; 2] = ["CURSOR_API_KEY", "CURSOR_AUTH_TOKEN"];
/// Where the Cursor installer keeps its builds, relative to the home
/// directory. `~/.local/bin/agent` and `~/.local/bin/cursor-agent` link
/// into it.
pub const INSTALL_TREE: &str = ".local/share/cursor-agent";
/// What a person does when Cursor refuses to authenticate.
pub const SIGN_IN: &str =
    "Cursor is not signed in. Run `cursor-agent login`, or set CURSOR_API_KEY, then try again.";

/// What a person does when Cursor's service refuses this `cursor-agent`
/// build (`Update Required`, error code `cursor_stream_error`).
pub const UPDATE: &str = "Cursor says this version of cursor-agent is no longer supported. Run `cursor-agent update`, then try again.";

/// Cursor's extension method names.
pub mod method {
    /// A request: multiple-choice questions for the person.
    pub const ASK_QUESTION: &str = "cursor/ask_question";
    /// A request: a plan for the person to accept.
    pub const CREATE_PLAN: &str = "cursor/create_plan";
    /// A notification: the agent's todo list.
    pub const UPDATE_TODOS: &str = "cursor/update_todos";
    /// A notification: a subagent task.
    pub const TASK: &str = "cursor/task";
    /// A notification: a generated image.
    pub const GENERATE_IMAGE: &str = "cursor/generate_image";
}

/// A Cursor session mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Full tool access.
    Agent,
    /// Read-only planning.
    Plan,
    /// Questions and answers, with no edits or commands.
    Ask,
}

impl Mode {
    /// The mode's id for `session/set_mode`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Mode::Agent => "agent",
            Mode::Plan => "plan",
            Mode::Ask => "ask",
        }
    }
}

/// The arguments that start Cursor's agent as an ACP server.
#[must_use]
pub fn arguments() -> Vec<String> {
    vec!["acp".into()]
}

/// What a person does when Cursor goes silent on `authenticate`. A
/// signed-out `cursor-agent` 2026.06.24 sometimes never answers instead of
/// refusing.
pub const SILENT_SIGN_IN: &str = "Cursor did not answer its sign-in check. If it is not signed in, run `cursor-agent login`, or set CURSOR_API_KEY, then try again.";

/// How long [`signed_in`] waits for `cursor-agent status`.
pub const STATUS_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Whether `environment` names one of [`CREDENTIAL_VARS`] with a value, so
/// Cursor signs in without a stored login.
#[must_use]
pub fn has_credential(environment: &[(String, String)]) -> bool {
    environment
        .iter()
        .any(|(name, value)| CREDENTIAL_VARS.contains(&name.as_str()) && !value.is_empty())
}

/// The `isAuthenticated` member of `cursor-agent status --format json`'s
/// output, or `None` when the output does not say. Nothing else is read.
#[must_use]
pub fn parse_status(stdout: &[u8]) -> Option<bool> {
    serde_json::from_slice::<Value>(stdout)
        .ok()?
        .get("isAuthenticated")?
        .as_bool()
}

/// Whether `program` has a stored login, from `cursor-agent status --format
/// json` run in `cwd` with exactly `environment`. `None` when the check
/// fails, times out after [`STATUS_LIMIT`], or prints something else; the
/// caller then lets `authenticate` decide.
pub async fn signed_in(
    program: &Path,
    environment: &[(String, String)],
    cwd: &Path,
) -> Option<bool> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(["status", "--format", "json"])
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        // A status check never opens Cursor's sign-in page.
        .envs(crate::process::NON_INTERACTIVE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(STATUS_LIMIT, command.output())
        .await
        .ok()?
        .ok()?;
    parse_status(&output.stdout)
}

/// The command that updates Cursor's agent in place.
pub const UPDATE_COMMAND: &str = "cursor-agent update";
/// The command that installs Cursor's agent fresh, which also repairs an
/// install whose `cursor-agent` is something else.
pub const INSTALL_COMMAND: &str = "curl https://cursor.com/install -fsS | bash";
/// Cursor's install script. It names the newest build in its download URL.
pub const INSTALLER_URL: &str = "https://cursor.com/install";
/// How long [`update`] waits for `cursor-agent update`.
pub const UPDATE_LIMIT: std::time::Duration = std::time::Duration::from_secs(300);

/// A `cursor-agent` build name such as `2026.10.01-e373342`: a date
/// (`YYYY.MM.DD`), then letters, digits, `-` and `.` only. Anything else,
/// such as the Cursor editor's `1.7.44`, is not Cursor's agent.
#[must_use]
pub fn valid_version(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 10
        && bytes.len() <= 64
        && bytes[..10]
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                4 | 7 => *byte == b'.',
                _ => byte.is_ascii_digit(),
            })
        && bytes[10..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'.')
}

/// The build `cursor-agent --version` printed, or `None` when its first line
/// is not a [`valid_version`].
#[must_use]
pub fn parse_version(stdout: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(stdout).ok()?;
    let first = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    valid_version(first).then(|| first.to_owned())
}

/// The newest build Cursor's install script downloads
/// (`downloads.cursor.com/lab/BUILD/...`), or `None` when it names none.
#[must_use]
pub fn latest_in_installer(script: &str) -> Option<String> {
    const MARK: &str = "downloads.cursor.com/lab/";
    let start = script.find(MARK)? + MARK.len();
    let build = script[start..].split('/').next()?;
    valid_version(build).then(|| build.to_owned())
}

/// Whether build `installed` is from an earlier day than build `latest`.
/// Only the dates are compared, since the rest of a build name is a hash.
#[must_use]
pub fn older(installed: &str, latest: &str) -> bool {
    valid_version(installed) && valid_version(latest) && installed[..10] < latest[..10]
}

/// The build `program --version` reports, run in `cwd` with exactly
/// `environment`. `None` when it fails, times out after [`STATUS_LIMIT`], or
/// prints something that is not a Cursor agent build.
pub async fn version(
    program: &Path,
    environment: &[(String, String)],
    cwd: &Path,
) -> Option<String> {
    let mut command = tokio::process::Command::new(program);
    command
        .arg("--version")
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        // Never Cursor's sign-in page (#11549).
        .envs(crate::process::NON_INTERACTIVE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(STATUS_LIMIT, command.output())
        .await
        .ok()?
        .ok()?;
    parse_version(&output.stdout)
}

/// How [`update`] ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Updated {
    /// `cursor-agent` answers with a Cursor agent build: the one it had
    /// before when nothing newer was out, else the new one.
    Now {
        before: Option<String>,
        after: String,
    },
    /// `cursor-agent update` failed or ran past [`UPDATE_LIMIT`]; the text
    /// is the last line it printed, if any.
    Failed(String),
    /// After the update, `cursor-agent` no longer answers as Cursor's
    /// agent. `cursor-agent update` has been seen to put the Cursor editor's
    /// launcher in its place (#11463). The text is what it printed for
    /// `--version`, if anything.
    Replaced(String),
}

impl Updated {
    /// What to tell the person, with the one command to run when something
    /// is wrong.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Updated::Now { before, after } if before.as_deref() == Some(after.as_str()) => {
                format!("cursor-agent is already the newest build ({after}).")
            }
            Updated::Now { after, .. } => format!("cursor-agent is updated to {after}."),
            Updated::Failed(said) if said.is_empty() => format!(
                "cursor-agent could not update itself. Install it fresh: `{INSTALL_COMMAND}`"
            ),
            Updated::Failed(said) => format!(
                "cursor-agent could not update itself ({said}). Install it fresh: `{INSTALL_COMMAND}`"
            ),
            Updated::Replaced(said) => {
                let answered = if said.is_empty() {
                    String::new()
                } else {
                    format!(" (it answers `{said}`)")
                };
                format!(
                    "The update put something other than Cursor's agent at cursor-agent{answered}, so Cursor delegations will fail. Install it fresh: `{INSTALL_COMMAND}`"
                )
            }
        }
    }
}

/// Runs `program update`, then checks that `program` still answers
/// `--version` as Cursor's agent, since an update has been seen to leave
/// the Cursor editor's launcher in its place.
pub async fn update(program: &Path, environment: &[(String, String)], cwd: &Path) -> Updated {
    let before = version(program, environment, cwd).await;
    let mut command = tokio::process::Command::new(program);
    command
        .arg("update")
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        // Never Cursor's sign-in page (#11549).
        .envs(crate::process::NON_INTERACTIVE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(UPDATE_LIMIT, command.output()).await {
        Err(_) => return Updated::Failed("it ran for five minutes without finishing".into()),
        Ok(Err(error)) => return Updated::Failed(error.to_string()),
        Ok(Ok(output)) => output,
    };
    if !output.status.success() {
        return Updated::Failed(
            last_line(&output.stderr)
                .or_else(|| last_line(&output.stdout))
                .unwrap_or_default(),
        );
    }
    let mut check = tokio::process::Command::new(program);
    check
        .arg("--version")
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().map(|(name, value)| (name, value)))
        // Never Cursor's sign-in page (#11549).
        .envs(crate::process::NON_INTERACTIVE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let printed = match tokio::time::timeout(STATUS_LIMIT, check.output()).await {
        Ok(Ok(output)) => output.stdout,
        _ => Vec::new(),
    };
    match parse_version(&printed) {
        Some(after) => Updated::Now { before, after },
        None => Updated::Replaced(last_line(&printed).unwrap_or_default()),
    }
}

fn last_line(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .last()?;
    Some(line.chars().filter(|c| !c.is_control()).take(200).collect())
}

/// The Cursor agent binary: `CURSOR_AGENT_BIN`, else `cursor-agent` on
/// `PATH` or in `~/.local/bin`, else `agent` on `PATH` or in `~/.local/bin`
/// when it resolves into [`INSTALL_TREE`]. Another tool may also install a
/// command named `agent`, so a bare `agent` outside Cursor's tree is never
/// taken. `variable` reads the environment.
#[must_use]
pub fn binary(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(named) = variable(BIN_VAR).filter(|value| !value.is_empty()) {
        return first_executable([PathBuf::from(named)]);
    }
    let home = user_home::home_in(variable);
    let path = variable("PATH");
    let local = |name: &str| home.as_ref().map(|home| home.join(".local/bin").join(name));
    if let Some(found) = first_executable(
        on_path("cursor-agent", path.as_deref())
            .into_iter()
            .chain(local("cursor-agent")),
    ) {
        return Some(found);
    }
    let tree = home.as_ref()?.join(INSTALL_TREE);
    on_path("agent", path.as_deref())
        .into_iter()
        .chain(local("agent"))
        .filter(|candidate| in_tree(candidate, &tree))
        .find_map(|candidate| first_executable([candidate]))
}

fn in_tree(candidate: &Path, tree: &Path) -> bool {
    let (Ok(candidate), Ok(tree)) = (candidate.canonicalize(), tree.canonicalize()) else {
        return false;
    };
    candidate.starts_with(tree)
}

/// What an unattended client answers to Cursor's request `method`, or
/// `None` for a method that is not Cursor's. A question is skipped, so the
/// agent decides for itself; a plan is accepted; a notification that
/// arrives as a request is acknowledged; an image is refused, because a
/// delegated session cannot show one.
#[must_use]
pub fn answer(method: &str, params: &Value) -> Option<Value> {
    let outcome = match method {
        method::ASK_QUESTION => json!({
            "outcome": "skipped",
            "reason": "No one can answer during a delegated task. Choose the most reasonable option and continue."
        }),
        method::CREATE_PLAN => json!({"outcome": "accepted"}),
        method::UPDATE_TODOS => {
            json!({"outcome": "accepted", "todos": params.get("todos").cloned().unwrap_or(json!([]))})
        }
        method::TASK => {
            let mut outcome = json!({"outcome": "completed"});
            if let Some(agent) = params.get("agentId") {
                outcome["agentId"] = agent.clone();
            }
            outcome
        }
        method::GENERATE_IMAGE => json!({
            "outcome": "rejected",
            "reason": "A delegated task cannot show generated images."
        }),
        _ => return None,
    };
    Some(json!({ "outcome": outcome }))
}

/// One entry of Cursor's todo list.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Todo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub status: String,
}

/// What one of Cursor's extension methods told the client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The agent asked the person something; [`answer`] skips it.
    Question {
        title: Option<String>,
        prompts: Vec<String>,
    },
    /// The agent proposed a plan; [`answer`] accepts it.
    Plan {
        name: Option<String>,
        overview: Option<String>,
        plan: String,
        todos: Vec<Todo>,
    },
    /// The agent's todo list, merged into the last one when `merge`.
    Todos { todos: Vec<Todo>, merge: bool },
    /// A subagent task the agent ran.
    Task {
        description: String,
        subagent: String,
        model: Option<String>,
        duration_ms: Option<u64>,
    },
    /// An image the agent generated.
    Image {
        description: String,
        path: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    questions: Vec<Prompt>,
}

#[derive(Deserialize)]
struct Prompt {
    #[serde(default)]
    prompt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    plan: String,
    #[serde(default)]
    todos: Vec<Todo>,
}

#[derive(Deserialize)]
struct Todos {
    #[serde(default)]
    todos: Vec<Todo>,
    #[serde(default)]
    merge: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    #[serde(default)]
    description: String,
    #[serde(default)]
    subagent_type: Value,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    duration_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Image {
    #[serde(default)]
    description: String,
    #[serde(default)]
    file_path: Option<String>,
}

impl Notice {
    /// The notice Cursor's `method` carries, or `None` for another method
    /// or a payload of the wrong shape.
    #[must_use]
    pub fn parse(method: &str, params: &Value) -> Option<Notice> {
        let params = params.clone();
        Some(match method {
            method::ASK_QUESTION => {
                let question: Question = serde_json::from_value(params).ok()?;
                Notice::Question {
                    title: question.title,
                    prompts: question.questions.into_iter().map(|q| q.prompt).collect(),
                }
            }
            method::CREATE_PLAN => {
                let plan: Plan = serde_json::from_value(params).ok()?;
                Notice::Plan {
                    name: plan.name,
                    overview: plan.overview,
                    plan: plan.plan,
                    todos: plan.todos,
                }
            }
            method::UPDATE_TODOS => {
                let todos: Todos = serde_json::from_value(params).ok()?;
                Notice::Todos {
                    todos: todos.todos,
                    merge: todos.merge,
                }
            }
            method::TASK => {
                let task: Task = serde_json::from_value(params).ok()?;
                let subagent = match &task.subagent_type {
                    Value::String(kind) => kind.clone(),
                    Value::Object(custom) => custom
                        .get("custom")
                        .and_then(Value::as_str)
                        .unwrap_or("custom")
                        .to_owned(),
                    _ => "unspecified".into(),
                };
                Notice::Task {
                    description: task.description,
                    subagent,
                    model: task.model,
                    duration_ms: task.duration_ms,
                }
            }
            method::GENERATE_IMAGE => {
                let image: Image = serde_json::from_value(params).ok()?;
                Notice::Image {
                    description: image.description,
                    path: image.file_path,
                }
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay;
    use crate::{
        Handler, Opening, PermissionAnswer, PermissionRequest, Session, StopReason, Update,
    };
    use std::time::Duration;

    #[cfg(unix)]
    fn executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn the_binary_prefers_cursor_agent_and_takes_agent_only_from_cursors_tree() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let other = dir.path().join("other/bin");
        executable(&other.join("agent"));
        let path = other.as_os_str().to_owned();
        let home_value = home.as_os_str().to_owned();
        let env = |name: &str| match name {
            "HOME" => Some(home_value.clone()),
            "PATH" => Some(path.clone()),
            _ => None,
        };
        // Another tool's `agent` is not Cursor's.
        assert_eq!(binary(&env), None);

        let build = home.join(INSTALL_TREE).join("versions/1/cursor-agent");
        executable(&build);
        let link = home.join(".local/bin/agent");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&build, &link).unwrap();
        assert_eq!(binary(&env), Some(link));

        let named = home.join(".local/bin/cursor-agent");
        std::os::unix::fs::symlink(&build, &named).unwrap();
        assert_eq!(binary(&env), Some(named));

        let explicit = other.join("agent").into_os_string();
        let env = |name: &str| (name == BIN_VAR).then(|| explicit.clone());
        assert_eq!(binary(&env), Some(other.join("agent")));
        let missing = |name: &str| (name == BIN_VAR).then(|| OsString::from("/nonexistent/agent"));
        assert_eq!(binary(&missing), None);
    }

    #[test]
    fn requests_the_agent_waits_on_are_answered_and_others_are_not() {
        let skipped = answer(method::ASK_QUESTION, &json!({})).unwrap();
        assert_eq!(skipped["outcome"]["outcome"], "skipped");
        assert_eq!(
            answer(method::CREATE_PLAN, &json!({"plan": "x"})).unwrap(),
            json!({"outcome": {"outcome": "accepted"}})
        );
        let todos = json!({"todos": [{"id": "1", "content": "a", "status": "pending"}]});
        assert_eq!(
            answer(method::UPDATE_TODOS, &todos).unwrap()["outcome"]["todos"],
            todos["todos"]
        );
        assert_eq!(
            answer(method::TASK, &json!({"agentId": "a1"})).unwrap()["outcome"]["agentId"],
            "a1"
        );
        assert_eq!(
            answer(method::GENERATE_IMAGE, &json!({})).unwrap()["outcome"]["outcome"],
            "rejected"
        );
        assert_eq!(answer("cursor/unknown", &json!({})), None);
        assert_eq!(answer("session/update", &json!({})), None);
        assert_eq!(Mode::Agent.id(), "agent");
        assert_eq!(Mode::Plan.id(), "plan");
        assert_eq!(Mode::Ask.id(), "ask");
        assert_eq!(arguments(), vec!["acp"]);
    }

    #[test]
    fn the_status_check_reads_only_is_authenticated() {
        assert_eq!(
            parse_status(br#"{"status":"unauthenticated","isAuthenticated":false,"message":"Not logged in"}"#),
            Some(false)
        );
        assert_eq!(
            parse_status(br#"{"status":"authenticated","isAuthenticated":true,"userInfo":{"email":"unread"}}"#),
            Some(true)
        );
        assert_eq!(parse_status(b"Not logged in"), None);
        assert_eq!(parse_status(br#"{"isAuthenticated":"yes"}"#), None);
        let key = |value: &str| vec![("CURSOR_API_KEY".to_owned(), value.to_owned())];
        assert!(has_credential(&key("k")));
        assert!(!has_credential(&key("")));
        assert!(!has_credential(&[("XAI_API_KEY".into(), "k".into())]));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_status_check_runs_the_binary_with_the_given_environment() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("cursor-agent");
        std::fs::write(
            &program,
            "#!/bin/sh\n[ \"$1 $2 $3\" = 'status --format json' ] || exit 2\nprintf '{\"isAuthenticated\":%s}' \"$SIGNED\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = |signed: &str| {
            vec![
                ("PATH".to_owned(), "/bin:/usr/bin".to_owned()),
                ("SIGNED".to_owned(), signed.to_owned()),
            ]
        };
        assert_eq!(
            signed_in(&program, &env("false"), dir.path()).await,
            Some(false)
        );
        assert_eq!(
            signed_in(&program, &env("true"), dir.path()).await,
            Some(true)
        );
        assert_eq!(
            signed_in(&dir.path().join("missing"), &env("true"), dir.path()).await,
            None
        );
    }

    #[test]
    fn extension_payloads_are_typed() {
        let task = json!({"toolCallId": "c", "description": "Explore", "prompt": "p",
            "subagentType": {"custom": "reviewer"}, "durationMs": 12});
        assert_eq!(
            Notice::parse(method::TASK, &task),
            Some(Notice::Task {
                description: "Explore".into(),
                subagent: "reviewer".into(),
                model: None,
                duration_ms: Some(12),
            })
        );
        let todos = json!({"toolCallId": "c", "merge": true,
            "todos": [{"id": "1", "content": "Write tests", "status": "in_progress"}]});
        let Some(Notice::Todos { todos, merge }) = Notice::parse(method::UPDATE_TODOS, &todos)
        else {
            panic!("todos");
        };
        assert!(merge);
        assert_eq!(todos[0].status, "in_progress");
        let question = json!({"toolCallId": "c", "title": "Need input",
            "questions": [{"id": "q1", "prompt": "Red or blue?", "options": []}]});
        assert_eq!(
            Notice::parse(method::ASK_QUESTION, &question),
            Some(Notice::Question {
                title: Some("Need input".into()),
                prompts: vec!["Red or blue?".into()],
            })
        );
        assert_eq!(Notice::parse(method::TASK, &json!("not an object")), None);
        assert_eq!(Notice::parse("cursor/other", &json!({})), None);
    }

    /// Answers the way an unattended delegation does.
    #[derive(Default)]
    struct Unattended {
        text: String,
        tools: Vec<String>,
        notices: Vec<Notice>,
        answers: Vec<Value>,
    }

    impl Handler for Unattended {
        fn update(&mut self, update: Update) {
            match update {
                Update::AgentText(text) => self.text.push_str(&text),
                Update::ToolCall { title, .. } => self.tools.push(title),
                _ => {}
            }
        }
        fn permission(&mut self, request: &PermissionRequest) -> PermissionAnswer {
            let answer = request
                .allow()
                .map_or(PermissionAnswer::Cancelled, |option| {
                    PermissionAnswer::Selected(option.into())
                });
            self.answers.push(answer.to_value());
            answer
        }
        fn notification(&mut self, method: &str, params: &Value) {
            self.notices.extend(Notice::parse(method, params));
        }
        fn reverse(
            &mut self,
            method: &str,
            params: &Value,
        ) -> Result<Value, crate::wire::RpcError> {
            self.notices.extend(Notice::parse(method, params));
            let answered = answer(method, params)
                .ok_or_else(|| crate::wire::RpcError::method_not_found(method));
            if let Ok(value) = &answered {
                self.answers.push(value.clone());
            }
            answered
        }
    }

    #[cfg(unix)]
    fn opening(program: PathBuf, cwd: PathBuf, mode: Mode) -> Opening {
        Opening {
            spec: crate::process::Spec {
                program,
                arguments: arguments(),
                cwd,
                environment: vec![("PATH".into(), "/bin:/usr/bin".into())],
            },
            resume: None,
            meta: None,
            mode: Some(mode.id().into()),
            authenticate: Some(AUTH_METHOD.into()),
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_recorded_turn_authenticates_runs_a_command_and_ends() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(replay::CURSOR_TURN));
        let mut session = Session::open(&opening(agent, dir.path().into(), Mode::Agent), &|| false)
            .await
            .unwrap();
        assert_eq!(session.id(), "5ae4d49f-2510-4f4d-83ec-3faac4c1571e");
        assert_eq!(
            session.opened.model(),
            Some("grok-4.5[effort=high,fast=true]")
        );
        let mut handler = Unattended::default();
        let reply = session
            .prompt(
                "Use your shell tool to run `echo hi > probe.txt`, then reply with exactly: done",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut handler,
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        assert!(handler.text.ends_with("done"));
        assert_eq!(handler.tools, vec!["`echo hi > probe.txt`"]);
        assert!(session.close(Duration::from_millis(500)).await);
        assert_eq!(replay::arguments(dir.path()), vec!["acp"]);
        let sent = replay::received(dir.path());
        assert_eq!(sent[1]["method"], "authenticate");
        assert_eq!(sent[1]["params"]["methodId"], AUTH_METHOD);
        assert_eq!(sent[3]["params"]["modeId"], "agent");
        assert_eq!(handler.answers.len(), 1);
        assert_eq!(handler.answers[0]["outcome"]["optionId"], "allow-once");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_recorded_plan_is_accepted_so_the_turn_ends() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(replay::CURSOR_PLAN));
        let mut session = Session::open(&opening(agent, dir.path().into(), Mode::Plan), &|| false)
            .await
            .unwrap();
        let mut handler = Unattended::default();
        let reply = session
            .prompt(
                "Plan a README.",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut handler,
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        assert!(matches!(
            handler.notices.as_slice(),
            [Notice::Plan { name: Some(name), plan, .. }]
                if name == "Add folder README" && plan.starts_with("# Add README.md")
        ));
        session.close(Duration::from_millis(500)).await;
        let sent = replay::received(dir.path());
        assert_eq!(sent[3]["params"]["modeId"], "plan");
        assert_eq!(
            handler.answers,
            vec![json!({"outcome": {"outcome": "accepted"}})]
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_question_is_skipped_so_the_turn_goes_on() {
        let dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::CURSOR_TURN);
        let session = "5ae4d49f-2510-4f4d-83ec-3faac4c1571e";
        blocks[4] = vec![
            json!({"jsonrpc": "2.0", "id": 0, "method": method::ASK_QUESTION, "params": {
                "toolCallId": "call_1", "title": "Need input",
                "questions": [{"id": "q1", "prompt": "Red or blue?",
                    "options": [{"id": "red", "label": "Red"}, {"id": "blue", "label": "Blue"}]}]}}),
            json!({"jsonrpc": "2.0", "method": method::UPDATE_TODOS, "params": {
                "toolCallId": "call_2", "merge": false,
                "todos": [{"id": "1", "content": "Pick a color", "status": "completed"}]}}),
            json!({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": session,
                "update": {"sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": "skipped"}}}}),
            json!({"jsonrpc": "2.0", "id": 5, "result": {"stopReason": "end_turn"}}),
        ];
        let agent = replay::script(dir.path(), &blocks);
        let mut session = Session::open(&opening(agent, dir.path().into(), Mode::Agent), &|| false)
            .await
            .unwrap();
        let mut handler = Unattended::default();
        let reply = session
            .prompt(
                "Ask me.",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut handler,
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        assert_eq!(handler.text, "skipped");
        assert!(matches!(handler.notices[0], Notice::Question { .. }));
        assert!(matches!(
            handler.notices[1],
            Notice::Todos { merge: false, .. }
        ));
        assert_eq!(handler.answers.len(), 1);
        assert_eq!(handler.answers[0]["outcome"]["outcome"], "skipped");
        session.close(Duration::from_millis(500)).await;
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_signed_out_agent_fails_authentication() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(replay::CURSOR_SIGNED_OUT));
        let failure = Session::open(&opening(agent, dir.path().into(), Mode::Agent), &|| false)
            .await
            .err()
            .unwrap();
        assert!(failure.unauthenticated());
        let sent = replay::received(dir.path());
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[1]["method"], "authenticate");
    }

    #[test]
    fn a_build_name_is_a_date_then_a_hash() {
        assert!(valid_version("2026.10.01-e373342"));
        assert!(valid_version("2026.06.24-00-45-58-9f61de7"));
        assert!(!valid_version("1.7.44"));
        assert!(!valid_version("2026.10.01 e373342"));
        assert!(!valid_version("2026-10-01-e373342"));
        assert_eq!(
            parse_version(b"\n2026.10.01-e373342\n"),
            Some("2026.10.01-e373342".into())
        );
        assert_eq!(parse_version(b"1.7.44\nabc123\narm64\n"), None);
        assert!(older("2026.06.24-00-45-58-9f61de7", "2026.10.01-e373342"));
        assert!(!older("2026.10.01-e373342", "2026.10.01-ffffff"));
        assert!(!older("1.7.44", "2026.10.01-e373342"));
    }

    #[test]
    fn the_installer_names_the_newest_build() {
        let script = "FINAL_DIR=x\nDOWNLOAD_URL=\"https://downloads.cursor.com/lab/2026.10.01-e373342/${OS}/${ARCH}/agent-cli-package.tar.gz\"\n";
        assert_eq!(
            latest_in_installer(script),
            Some("2026.10.01-e373342".into())
        );
        assert_eq!(latest_in_installer("echo hi"), None);
        assert_eq!(
            latest_in_installer("downloads.cursor.com/lab/$(rm -rf)/x"),
            None
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn an_update_is_checked_by_asking_the_binary_its_version_again() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("cursor-agent");
        // `update` writes what `--version` prints afterwards.
        std::fs::write(
            &program,
            "#!/bin/sh\ncase \"$1\" in\n--version) cat \"$STATE\" ;;\nupdate) [ \"$AFTER\" = fail ] && { echo 'no network' >&2; exit 1; }; printf '%s\\n' \"$AFTER\" > \"$STATE\" ;;\n*) exit 2 ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let state = dir.path().join("state");
        let run = |after: &'static str| {
            std::fs::write(&state, "2026.06.24-00-45-58-9f61de7\n").unwrap();
            let env = vec![
                ("PATH".to_owned(), "/bin:/usr/bin".to_owned()),
                ("STATE".to_owned(), state.to_string_lossy().into_owned()),
                ("AFTER".to_owned(), after.to_owned()),
            ];
            let program = program.clone();
            let cwd = dir.path().to_path_buf();
            async move { update(&program, &env, &cwd).await }
        };
        assert_eq!(
            run("2026.10.01-e373342").await,
            Updated::Now {
                before: Some("2026.06.24-00-45-58-9f61de7".into()),
                after: "2026.10.01-e373342".into()
            }
        );
        let replaced = run("1.7.44").await;
        assert_eq!(replaced, Updated::Replaced("1.7.44".into()));
        assert!(
            replaced.message().contains(INSTALL_COMMAND),
            "{}",
            replaced.message()
        );
        let failed = run("fail").await;
        assert_eq!(failed, Updated::Failed("no network".into()));
        assert!(failed.message().contains(INSTALL_COMMAND));
        assert_eq!(
            version(
                &program,
                &[("STATE".into(), state.to_string_lossy().into_owned())],
                dir.path()
            )
            .await,
            Some("2026.06.24-00-45-58-9f61de7".into())
        );
    }
}
