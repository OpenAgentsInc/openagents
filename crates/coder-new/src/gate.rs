//! The three-band gate (#11489): System One hook 4, a `PreToolUse` hook that
//! runs after the floors and can only add a `deny`.
//!
//! The first question: does `Bash` run a script file in the working
//! directory that this session never read or wrote? The rule part is
//! deterministic (parse the command, track `Read`/`Edit`/`Write` paths per
//! session, skip well-known entrypoints). Only when the rule cannot tell does
//! one Decisions API predicate ask whether running it unread risks an
//! unintended effect. A failed or slow call does nothing. Each decision is
//! one JSON line in `~/.openagents/decisions/gate.jsonl` (`CODER_GATE_LOG`
//! overrides), the same fields as the squeezer's log.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// The decision model.
pub const MODEL: &str = "openagents/decide";
/// The hook's name in the log.
const HOOK: &str = "gate";
/// The question's name in the log.
const QUESTION: &str = "unread_script_unintended_effect";
/// How long the gate waits for a decision.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Lines of the script the decision sees.
const SCRIPT_LINES: usize = 60;
/// The log override.
pub const LOG_VAR: &str = "CODER_GATE_LOG";

/// Written costs, in minutes of the person's attention.
/// Asking the agent to read the script first costs one extra tool call.
const COST_ASK: f64 = 1.0;
/// A wrong run (a script that deletes, deploys or overwrites) costs a
/// recovery.
const COST_WRONG_RUN: f64 = 20.0;

/// Deny only when `p > 1 - C(ask) / C(wrong run)`: asking is worth it once
/// the chance of a wrong run exceeds the ask's share of its cost (0.95).
#[must_use]
pub fn threshold() -> f64 {
    1.0 - COST_ASK / COST_WRONG_RUN
}

/// A decision: `(model, door, latency_ms, p)`.
pub type Rated = (String, Option<String>, u64, f64);

/// Asks the one predicate.
pub trait Judge {
    /// # Errors
    ///
    /// The call failed or timed out; the gate does nothing.
    fn rate(&self, input: &str, question: &str) -> Result<Rated, String>;
}

/// The Decisions API at `jev::decisions::OPENAGENTS` or
/// `OPENAGENTS_DECISIONS_URL`.
pub struct Decisions;

impl Judge for Decisions {
    fn rate(&self, input: &str, question: &str) -> Result<Rated, String> {
        let url = std::env::var("OPENAGENTS_DECISIONS_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .map_or_else(
                || jev::decisions::OPENAGENTS.to_owned(),
                |url| url.trim_end_matches('/').to_owned(),
            );
        let client = jev::decisions::DecisionsClient::new(url)
            .map_err(|error| error.to_string())?
            .timeout(TIMEOUT);
        let request = jev::decisions::DecisionsRequest::new(MODEL, input)
            .predicate(QUESTION.to_owned(), question.to_owned());
        let started = Instant::now();
        let response = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())?
                        .block_on(client.create(&request))
                        .map_err(|error| error.to_string())
                })
                .join()
                .unwrap_or_else(|_| Err("the decision call panicked".to_owned()))
        })?;
        let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let p = response
            .probability(QUESTION)
            .ok_or_else(|| "no probability returned".to_owned())?;
        Ok((response.model, response.openagents.door, latency, p))
    }
}

/// The question text.
const QUESTION_TEXT: &str = "An agent is about to run a script it has not read. Would running it \
     risk an unintended effect, such as deleting or overwriting files, deploying, pushing, \
     changing configuration, or sending data out, beyond what the command line suggests? A \
     script that only builds, tests, formats or prints is safe.";

/// Files this session has read or written, by session id.
fn touched() -> &'static Mutex<HashMap<String, HashSet<PathBuf>>> {
    static TOUCHED: OnceLock<Mutex<HashMap<String, HashSet<PathBuf>>>> = OnceLock::new();
    TOUCHED.get_or_init(Mutex::default)
}

/// Resolves `path` against `cwd`, dropping `.` and `..` without touching disk.
fn normalize(cwd: &Path, path: &str) -> PathBuf {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Notes a `Read`, `Edit`, `Write` or `MultiEdit` of a file.
fn note_touch(touched: &mut HashSet<PathBuf>, tool: &str, tool_input: &Value, cwd: &Path) {
    if matches!(tool, "Read" | "Edit" | "Write" | "MultiEdit")
        && let Some(path) = tool_input["file_path"].as_str()
    {
        touched.insert(normalize(cwd, path));
    }
}

const INTERPRETERS: &[&str] = &[
    "bash", "sh", "zsh", "python", "python3", "node", "ruby", "perl", "deno", "bun",
];

/// Script files inside `cwd` that `command` runs: `bash x.sh`,
/// `python x.py`, `./x`. Only paths that are existing files.
#[must_use]
pub fn script_paths(command: &str, cwd: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for segment in command.split(['\n', ';', '|', '&']) {
        let mut words = segment
            .split_whitespace()
            .map(|w| w.trim_matches(['"', '\'']));
        let Some(head) = words.next() else { continue };
        let program = head.rsplit('/').next().unwrap_or(head);
        let candidate = if INTERPRETERS.contains(&program) {
            words.find(|w| !w.starts_with('-'))
        } else {
            (head.starts_with("./") || head.starts_with("../") || head.starts_with('/'))
                .then_some(head)
        };
        let Some(candidate) = candidate else { continue };
        let path = normalize(cwd, candidate);
        if path.starts_with(cwd) && path.is_file() && !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// A project entrypoint: under `scripts/` and named in the README.
fn is_known_entrypoint(script: &Path, cwd: &Path) -> bool {
    let Ok(relative) = script.strip_prefix(cwd) else {
        return false;
    };
    if !relative.starts_with("scripts") {
        return false;
    }
    let Ok(readme) = std::fs::read_to_string(cwd.join("README.md")) else {
        return false;
    };
    let name = relative.file_name().and_then(|n| n.to_str()).unwrap_or("");
    readme.contains(relative.to_string_lossy().as_ref())
        || (!name.is_empty() && readme.contains(name))
}

/// The gate's verdict on one tool call. `None` means do nothing.
fn evaluate(
    touched: &HashSet<PathBuf>,
    tool: &str,
    tool_input: &Value,
    cwd: &Path,
    judge: &dyn Judge,
    log: &dyn Fn(&Value),
) -> Option<String> {
    if tool != "Bash" {
        return None;
    }
    let command = tool_input["command"].as_str()?;
    for script in script_paths(command, cwd) {
        if touched.contains(&script) || is_known_entrypoint(&script, cwd) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&script) else {
            continue;
        };
        let head: String = text
            .lines()
            .take(SCRIPT_LINES)
            .collect::<Vec<_>>()
            .join("\n");
        let input = format!(
            "Command: {command}\n\nScript {} (first {SCRIPT_LINES} lines):\n{head}",
            script.display()
        );
        let limit = threshold();
        match judge.rate(&input, QUESTION_TEXT) {
            Ok((model, door, latency_ms, p)) => {
                let deny = p >= limit;
                let action = if deny { "deny" } else { "allow" };
                log(&record(
                    Some(p),
                    limit,
                    action,
                    &model,
                    door.as_deref(),
                    latency_ms,
                    command,
                ));
                if deny {
                    return Some(format!(
                        "{} has not been read in this session and may have an unintended effect. Read it first, then run the command again.",
                        script.display()
                    ));
                }
            }
            Err(error) => {
                let mut line = record(None, limit, "none", MODEL, None, 0, command);
                line["error"] = json!(error);
                log(&line);
            }
        }
    }
    None
}

fn record(
    p: Option<f64>,
    threshold: f64,
    action: &str,
    model: &str,
    door: Option<&str>,
    latency_ms: u64,
    command: &str,
) -> Value {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    json!({
        "hook": HOOK, "question": QUESTION, "p": p, "threshold": threshold,
        "action": action, "model": model, "door": door, "latency_ms": latency_ms,
        "ts": ts, "command": command,
    })
}

fn log_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(LOG_VAR)
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }
    Some(PathBuf::from(std::env::var_os("HOME")?).join(".openagents/decisions/gate.jsonl"))
}

fn append_log(line: &Value) {
    use std::io::Write;
    let Some(path) = log_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// The `PreToolUse` deny output.
fn deny_output(reason: &str) -> claude_agent_sdk::HookJSONOutput {
    claude_agent_sdk::SyncHookJSONOutput {
        hook_specific_output: Some(json!({
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        })),
        ..claude_agent_sdk::SyncHookJSONOutput::default()
    }
    .into()
}

/// The hook: tracks reads and writes, and denies an unread script the
/// decision rates risky.
#[must_use]
pub fn claude_hook() -> claude_agent_sdk::HookMatcher {
    let hook = claude_agent_sdk::hook_fn(move |input: Value, _tool_use_id| async move {
        let tool = input["tool_name"].as_str().unwrap_or_default().to_owned();
        let tool_input = input["tool_input"].clone();
        let session = input["session_id"].as_str().unwrap_or_default().to_owned();
        let cwd = input["cwd"].as_str().map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            PathBuf::from,
        );
        let seen = {
            let mut all = touched()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let set = all.entry(session).or_default();
            note_touch(set, &tool, &tool_input, &cwd);
            set.clone()
        };
        let reason = tokio::task::spawn_blocking(move || {
            evaluate(&seen, &tool, &tool_input, &cwd, &Decisions, &append_log)
        })
        .await
        .ok()
        .flatten();
        Ok(match reason {
            Some(reason) => deny_output(&reason),
            None => claude_agent_sdk::SyncHookJSONOutput::continue_execution().into(),
        })
    });
    claude_agent_sdk::HookMatcher::new(Some("Read|Edit|Write|MultiEdit|Bash"), hook)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake(Result<f64, String>, RefCell<u32>);
    impl Judge for Fake {
        fn rate(&self, _: &str, _: &str) -> Result<Rated, String> {
            *self.1.borrow_mut() += 1;
            self.0.clone().map(|p| ("fake".into(), None, 5, p))
        }
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("scripts")).unwrap();
        std::fs::write(dir.path().join("x.sh"), "rm -rf build\n").unwrap();
        std::fs::write(dir.path().join("scripts/ok.sh"), "echo hi\n").unwrap();
        dir
    }

    #[test]
    fn threshold_follows_the_written_costs() {
        assert!((threshold() - 0.95).abs() < 1e-9);
    }

    #[test]
    fn parses_script_paths_inside_the_cwd() {
        let dir = repo();
        let cwd = dir.path();
        assert_eq!(script_paths("bash x.sh", cwd), vec![cwd.join("x.sh")]);
        assert_eq!(
            script_paths("cd a && ./x.sh --flag", cwd),
            vec![cwd.join("x.sh")]
        );
        assert_eq!(script_paths("bash -e x.sh", cwd), vec![cwd.join("x.sh")]);
        assert!(script_paths("bash ../x.sh", cwd).is_empty());
        assert!(script_paths("bash missing.sh", cwd).is_empty());
        assert!(script_paths("cargo test", cwd).is_empty());
    }

    #[test]
    fn tracks_reads_and_writes() {
        let dir = repo();
        let mut set = HashSet::new();
        note_touch(&mut set, "Read", &json!({"file_path": "x.sh"}), dir.path());
        note_touch(&mut set, "Bash", &json!({"file_path": "y.sh"}), dir.path());
        assert!(set.contains(&dir.path().join("x.sh")));
        assert_eq!(set.len(), 1);
    }

    fn run(set: &HashSet<PathBuf>, cmd: &str, dir: &Path, judge: &Fake) -> Option<String> {
        evaluate(set, "Bash", &json!({"command": cmd}), dir, judge, &|_| {})
    }

    #[test]
    fn denies_only_an_unread_script_rated_above_threshold() {
        let dir = repo();
        let empty = HashSet::new();
        let high = Fake(Ok(0.99), RefCell::default());
        let reason = run(&empty, "bash x.sh", dir.path(), &high).unwrap();
        assert!(reason.contains("Read it first"));
        let low = Fake(Ok(0.5), RefCell::default());
        assert!(run(&empty, "bash x.sh", dir.path(), &low).is_none());
        assert_eq!(*low.1.borrow(), 1);
    }

    #[test]
    fn rules_skip_the_api() {
        let dir = repo();
        let judge = Fake(Ok(0.99), RefCell::default());
        let mut read = HashSet::new();
        read.insert(dir.path().join("x.sh"));
        assert!(run(&read, "bash x.sh", dir.path(), &judge).is_none());
        std::fs::write(dir.path().join("README.md"), "run scripts/ok.sh\n").unwrap();
        assert!(run(&HashSet::new(), "bash scripts/ok.sh", dir.path(), &judge).is_none());
        assert!(run(&HashSet::new(), "ls", dir.path(), &judge).is_none());
        assert_eq!(*judge.1.borrow(), 0);
    }

    #[test]
    fn a_failed_call_does_nothing_and_is_logged() {
        let dir = repo();
        let judge = Fake(Err("timeout".into()), RefCell::default());
        let lines = RefCell::new(Vec::new());
        let out = evaluate(
            &HashSet::new(),
            "Bash",
            &json!({"command": "./x.sh"}),
            dir.path(),
            &judge,
            &|l| lines.borrow_mut().push(l.clone()),
        );
        assert!(out.is_none());
        assert_eq!(lines.borrow()[0]["error"], "timeout");
        assert_eq!(lines.borrow()[0]["action"], "none");
    }

    #[test]
    fn deny_output_carries_the_reason() {
        let value = serde_json::to_value(deny_output("why")).unwrap();
        assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            value["hookSpecificOutput"]["permissionDecisionReason"],
            "why"
        );
    }
}
