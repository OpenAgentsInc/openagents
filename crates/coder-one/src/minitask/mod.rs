//! Mini-tasks: small local tasks with their own graders, for composing
//! components end to end in seconds.
//!
//! A mini-task is a scratch directory the host sets up, a public
//! instruction, and a grader the episode never sees, as Harbor's verifier
//! is to a Terminal-Bench task. Four cover the failure families the v3
//! Luna trials showed and one recovery:
//!
//! | ID | Family | What the grader checks |
//! | --- | --- | --- |
//! | `log-severity` | Field meaning in data | `summary.csv` counts each line by its severity field |
//! | `interactive-terminal` | Interactive behavior | An interactive program run through `HeadlessTerminal` reads typed input |
//! | `cancel-cleanup` | Cancellation lifecycle | After a real interrupt, started tasks finish cleanup |
//! | `git-recovery` | Recovery | A commit lost to a reset is back on the branch |
//!
//! Each task has a known-good and a known-bad script for the scripted
//! executor ([`crate::scripted`]), so an episode on it runs without a
//! model: [`run::run`] sets the task up, runs the same explore-then-
//! delegate path an episode runs, grades the result, and records it all
//! as a mini-task run the Gym shows beside Terminal-Bench attempts. The
//! same task runs with a real executor inside `coder-boundary`.

pub mod cli;
pub mod process;
pub mod run;
pub mod sources;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use crate::scripted::{Act, SCRIPT_SCHEMA, Script, Timed};
use crate::session::Capabilities;
use crate::stream::Format;

/// The schema of a mini-task run's manifest.
pub const RUN_SCHEMA: &str = "openagents.coder-one.minitask-run.v1";

/// One mini-task.
#[derive(Clone, Copy, Debug)]
pub struct MiniTask {
    pub id: &'static str,
    /// The failure family it covers.
    pub family: &'static str,
    pub instruction: &'static str,
}

/// Every mini-task.
pub const CATALOG: &[MiniTask] = &[
    MiniTask {
        id: "log-severity",
        family: "field meaning in data",
        instruction: LOG_INSTRUCTION,
    },
    MiniTask {
        id: "interactive-terminal",
        family: "interactive behavior",
        instruction: TERMINAL_INSTRUCTION,
    },
    MiniTask {
        id: "cancel-cleanup",
        family: "cancellation lifecycle",
        instruction: CANCEL_INSTRUCTION,
    },
    MiniTask {
        id: "git-recovery",
        family: "recovery",
        instruction: GIT_INSTRUCTION,
    },
];

const LOG_INSTRUCTION: &str = "The directory logs/ holds log files named YYYY-MM-DD_<source>.log, \
each holding the logs of that date. Each log line contains an event with a severity level. \
Count how many times each severity appears in the following date ranges:
Today (the current date)
Last 7 days (including today)
Total (all log files combined, regardless of date)

The severity levels to count are exactly: ERROR, WARNING, and INFO.
Write a CSV file summary.csv with the following structure (including the header):
period,severity,count
today,ERROR,<count>
today,WARNING,<count>
today,INFO,<count>
last_7_days,ERROR,<count>
last_7_days,WARNING,<count>
last_7_days,INFO,<count>
total,ERROR,<count>
total,WARNING,<count>
total,INFO,<count>

The current date is 2025-08-12. Use this as the reference date for all calculations.
Keep the program that writes the summary as summarize.py, runnable as \
`python3 summarize.py LOG_DIR OUTPUT_CSV`.";

const TERMINAL_INSTRUCTION: &str = "Implement the provided `BaseTerminal` interface in \
base_terminal.py. It provides a simple python interface to send keys to a headless terminal.

Make sure it supports the following functionality.

- Mimics a terminal, where the process starts as an interactive bash shell and commands are \
typically executed by typing characters and hitting Enter.
- Supports interactive programs.
- Has support for modifier keys like `\"\\x03\"` for control C, etc.

Call your implementation `HeadlessTerminal(BaseTerminal)` and put it in a file called \
headless_terminal.py so that it can be imported as `from headless_terminal import HeadlessTerminal`.";

const CANCEL_INSTRUCTION: &str = "Create a Python function called \
`async run_tasks(tasks: list[Callable[[], Awaitable[None]]], max_concurrent: int) -> None` \
where each task is an async job to be run and `max_concurrent` is the max number of tasks \
that can be run concurrently. Put the function in a file called run.py so that it can be \
imported using `from run import run_tasks`.

Sometimes I cancel runs via keyboard interrupt but I want the tasks' cleanup code to still run.";

const GIT_INSTRUCTION: &str = "I committed feature.txt on master and then ran a hard reset \
that lost the commit. Recover the lost commit so that master contains feature.txt with its \
original content, and leave the working tree clean.";

/// The mini-task with `id`.
///
/// # Errors
///
/// Returns a message naming the known tasks when none matches.
pub fn find(id: &str) -> Result<MiniTask, String> {
    CATALOG
        .iter()
        .copied()
        .find(|task| task.id == id)
        .ok_or_else(|| {
            format!(
                "no mini-task {id}; known: {}",
                CATALOG.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
            )
        })
}

/// The log-severity task's files: name, days before the reference date,
/// and lines. Some messages carry another severity's word.
pub const LOG_FILES: &[(&str, i64, &[&str])] = &[
    (
        "2025-08-12_app.log",
        0,
        &[
            "2025-08-12 09:00:01 [INFO] Service started",
            "2025-08-12 09:05:12 [ERROR] Payment failed: timeout",
            "2025-08-12 09:06:40 [INFO] Recovered after ERROR in payment worker",
            "2025-08-12 09:10:03 [WARNING] Disk usage at 81%",
            "2025-08-12 09:12:44 [DEBUG] INFO cache warmed",
        ],
    ),
    (
        "2025-08-06_db.log",
        6,
        &[
            "2025-08-06 01:00:00 [WARNING] Slow query; ERROR budget at 40%",
            "2025-08-06 01:02:10 [INFO] Vacuum finished",
            "2025-08-06 01:03:00 [ERROR] Replica lag",
        ],
    ),
    (
        "2025-08-05_db.log",
        7,
        &[
            "2025-08-05 23:59:59 [ERROR] Connection reset",
            "2025-08-05 23:59:59 [INFO] Retry scheduled",
        ],
    ),
    (
        "2025-07-20_web.log",
        23,
        &[
            "2025-07-20 12:00:00 [INFO] Deploy finished with no WARNING",
            "2025-07-20 12:01:00 [WARNING] Certificate expires soon",
        ],
    ),
];

const LEVELS: [&str; 3] = ["ERROR", "WARNING", "INFO"];
const PERIODS: [(&str, i64); 3] = [("today", 0), ("last_7_days", 6), ("total", i64::MAX)];

/// The line's severity field: the bracketed word after the timestamp.
fn field(line: &str) -> Option<&str> {
    let rest = line.splitn(3, ' ').nth(2)?;
    let level = rest.strip_prefix('[')?.split(']').next()?;
    LEVELS.contains(&level).then_some(level)
}

/// Whether `word` appears in `line` as a whole word.
#[must_use]
pub fn has_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + word.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        boundary(before) && boundary(after)
    })
}

/// The summary CSV for the log files, counting by the severity field or,
/// with `whole_line`, by any severity word on the line.
#[must_use]
pub fn log_summary(whole_line: bool) -> String {
    let mut rows = vec!["period,severity,count".to_string()];
    for (period, max_age) in PERIODS {
        for level in LEVELS {
            let count: usize = LOG_FILES
                .iter()
                .filter(|(_, age, _)| *age <= max_age)
                .map(|(_, _, lines)| {
                    lines
                        .iter()
                        .filter(|line| {
                            if whole_line {
                                has_word(line, level)
                            } else {
                                field(line) == Some(level)
                            }
                        })
                        .count()
                })
                .sum();
            rows.push(format!("{period},{level},{count}"));
        }
    }
    format!("{}\n", rows.join("\n"))
}

fn git(workdir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Mini Task",
            "-c",
            "user.email=minitask@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=master",
        ])
        .args(args)
        .current_dir(workdir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The content of the git task's lost file.
pub const FEATURE: &str = "The feature this repository exists for.\n";

/// Sets `task` up in the empty directory `workdir`.
///
/// # Errors
///
/// Returns a message when a file or a Git command fails.
pub fn setup(task: &MiniTask, workdir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(workdir)
        .map_err(|error| format!("cannot create {}: {error}", workdir.display()))?;
    let write = |relative: &str, text: &str| {
        let path = workdir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, text).map_err(|error| format!("cannot write {relative}: {error}"))
    };
    match task.id {
        "log-severity" => {
            for (name, _, lines) in LOG_FILES {
                write(&format!("logs/{name}"), &format!("{}\n", lines.join("\n")))?;
            }
        }
        "interactive-terminal" => write("base_terminal.py", sources::BASE_TERMINAL)?,
        "cancel-cleanup" => {}
        "git-recovery" => {
            git(workdir, &["init", "-q"])?;
            write("README.md", "A repository with one feature.\n")?;
            git(workdir, &["add", "README.md"])?;
            git(workdir, &["commit", "-q", "-m", "Add the README"])?;
            write("feature.txt", FEATURE)?;
            git(workdir, &["add", "feature.txt"])?;
            git(workdir, &["commit", "-q", "-m", "Add the feature"])?;
            git(workdir, &["reset", "-q", "--hard", "HEAD~1"])?;
        }
        other => return Err(format!("no setup for {other}")),
    }
    Ok(())
}

/// A grader's verdict.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Grade {
    /// `passed`, `failed`, or `unavailable` when the grader couldn't run.
    pub verdict: String,
    pub detail: String,
}

impl Grade {
    fn passed(detail: impl Into<String>) -> Self {
        Grade {
            verdict: "passed".to_string(),
            detail: detail.into(),
        }
    }
    fn failed(detail: impl Into<String>) -> Self {
        Grade {
            verdict: "failed".to_string(),
            detail: detail.into(),
        }
    }
    fn unavailable(detail: impl Into<String>) -> Self {
        Grade {
            verdict: "unavailable".to_string(),
            detail: detail.into(),
        }
    }

    /// The reward the grade stands for: 1, 0, or none.
    #[must_use]
    pub fn reward(&self) -> Option<f64> {
        match self.verdict.as_str() {
            "passed" => Some(1.0),
            "failed" => Some(0.0),
            _ => None,
        }
    }
}

/// Grades `task` in `workdir`, using `scratch` for the grader's own files.
pub async fn grade(task: &MiniTask, workdir: &Path, scratch: &Path) -> Grade {
    let _ = std::fs::create_dir_all(scratch);
    match task.id {
        "log-severity" => match std::fs::read_to_string(workdir.join("summary.csv")) {
            Err(_) => Grade::failed("summary.csv is missing"),
            Ok(text) => {
                let want = log_summary(false);
                if text.trim_end() == want.trim_end() {
                    Grade::passed("summary.csv matches the counts by severity field")
                } else {
                    let wrong: Vec<String> = text
                        .lines()
                        .zip(want.lines())
                        .filter(|(got, want)| got != want)
                        .map(|(got, want)| format!("{got} (want {want})"))
                        .collect();
                    Grade::failed(format!("summary.csv differs: {}", wrong.join("; ")))
                }
            }
        },
        "interactive-terminal" => grade_terminal(workdir, scratch).await,
        "cancel-cleanup" => grade_cancel(workdir, scratch).await,
        "git-recovery" => {
            let head = git(workdir, &["show", "HEAD:feature.txt"]);
            let status = git(workdir, &["status", "--porcelain"]);
            match (head, status) {
                (Ok(content), Ok(status)) if content == FEATURE && status.trim().is_empty() => {
                    Grade::passed("master holds feature.txt and the tree is clean")
                }
                (Ok(content), Ok(status)) if content == FEATURE => {
                    Grade::failed(format!("the tree is not clean: {}", status.trim()))
                }
                (Ok(_), _) => Grade::failed("feature.txt on master differs from the original"),
                (Err(error), _) => Grade::failed(format!("master has no feature.txt: {error}")),
            }
        }
        other => Grade::unavailable(format!("no grader for {other}")),
    }
}

const TERMINAL_GRADER: &str = r#"import os, sys, threading
sys.path.insert(0, os.getcwd())
from headless_terminal import HeadlessTerminal
target = sys.argv[1]
terminal = HeadlessTerminal()
def send(keys, wait):
    thread = threading.Thread(target=terminal.send_keystrokes, args=(keys, wait), daemon=True)
    thread.start()
    thread.join(5)
    return not thread.is_alive()
ok = send("cat > " + target + "\n", 0.5) and send("hello from the grader\n", 0.3) and send("\x04", 0.5)
print("responsive" if ok else "blocked", flush=True)
os._exit(0)
"#;

async fn grade_terminal(workdir: &Path, scratch: &Path) -> Grade {
    let Some(python) = process::python() else {
        return Grade::unavailable("python3 is not on PATH");
    };
    let target = scratch.join("grader-note.txt");
    let _ = std::fs::remove_file(&target);
    let mut command = Command::new(python);
    command
        .arg("-c")
        .arg(TERMINAL_GRADER)
        .arg(&target)
        .current_dir(workdir)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let ran = process::run(command, Duration::from_secs(20)).await;
    let note = process::wait_for_file(&target, Duration::from_secs(2), &|text| {
        text.contains("hello from the grader")
    })
    .await;
    if note.as_deref() == Some("hello from the grader\n") {
        Grade::passed("an interactive program read the typed line")
    } else {
        Grade::failed(format!(
            "the interactive program's file holds {:?}; the grader said {:?}{}",
            note.unwrap_or_default(),
            ran.stdout.trim(),
            if ran.stderr.trim().is_empty() {
                String::new()
            } else {
                format!(" and {}", clip(ran.stderr.trim(), 300))
            }
        ))
    }
}

const CANCEL_GRADER: &str = r#"import asyncio, os, sys
sys.path.insert(0, os.getcwd())
from run import run_tasks
log = sys.argv[1]
def note(text):
    with open(log, "a") as f:
        f.write(text + "\n")
def job(i):
    async def task():
        note(f"started {i}")
        try:
            await asyncio.sleep(60)
        finally:
            await asyncio.sleep(0.1)
            note(f"cleaned {i}")
    return task
try:
    asyncio.run(run_tasks([job(i) for i in range(3)], 2))
except KeyboardInterrupt:
    pass
note("exited")
"#;

async fn grade_cancel(workdir: &Path, scratch: &Path) -> Grade {
    let Some(python) = process::python() else {
        return Grade::unavailable("python3 is not on PATH");
    };
    let log = scratch.join("grader-cancel.log");
    let _ = std::fs::remove_file(&log);
    let mut command = Command::new(python);
    command
        .arg("-c")
        .arg(CANCEL_GRADER)
        .arg(&log)
        .current_dir(workdir)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let log_ready = log.clone();
    let ready = move || {
        std::fs::read_to_string(&log_ready)
            .map(|text| text.matches("started").count() >= 2)
            .unwrap_or(false)
    };
    let interrupted = process::interrupt_when(
        command,
        scratch,
        &ready,
        Duration::from_secs(10),
        Duration::from_secs(5),
    )
    .await;
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let cleaned = text.matches("cleaned").count();
    if !interrupted.ready {
        return Grade::failed(format!(
            "two tasks never started: {}",
            clip(interrupted.ran.stderr.trim(), 300)
        ));
    }
    if cleaned == 2 && text.contains("exited") {
        Grade::passed("both started tasks cleaned up after the interrupt")
    } else {
        Grade::failed(format!(
            "{cleaned} of 2 started tasks cleaned up{}",
            if interrupted.ran.killed {
                "; the process did not exit"
            } else {
                ""
            }
        ))
    }
}

fn clip(text: &str, max: usize) -> String {
    crate::judge::clip(text, max)
}

fn timed(at_ms: u64, act: Act) -> Timed {
    Timed { at_ms, act }
}

fn write(path: &str, content: &str) -> Act {
    Act::Write {
        path: path.to_string(),
        content: content.to_string(),
        announce: true,
    }
}

fn claim(text: &str) -> Act {
    Act::Claim {
        text: text.to_string(),
    }
}

/// The scripts for `task`: `good` and `bad`.
#[must_use]
pub fn scripts(task: &MiniTask) -> Vec<(&'static str, Script)> {
    let script = |name: &str, events: Vec<Timed>| Script {
        schema: SCRIPT_SCHEMA.to_string(),
        name: format!("{}-{name}", task.id),
        format: Format::Codex,
        model: "scripted".to_string(),
        capabilities: Capabilities::all(),
        events,
        on_steer: Vec::new(),
        on_resume: Vec::new(),
        rebind: Vec::new(),
        opening: true,
    };
    let solve = |files: Vec<(&str, &str)>, done: &str| {
        let mut events = vec![timed(0, claim("Reading the task and the files it names."))];
        for (i, (path, content)) in files.iter().enumerate() {
            events.push(timed(20 + 20 * i as u64, write(path, content)));
        }
        events.push(timed(200, claim(done)));
        events.push(timed(200, Act::End { error: false }));
        events
    };
    let good_summary = log_summary(false);
    let bad_summary = log_summary(true);
    match task.id {
        "log-severity" => vec![
            (
                "good",
                script(
                    "good",
                    solve(
                        vec![
                            ("summarize.py", sources::SUMMARIZE_FIELD),
                            ("summary.csv", &good_summary),
                        ],
                        "Wrote summarize.py, which reads each line's severity field, and summary.csv.",
                    ),
                ),
            ),
            (
                "bad",
                script(
                    "bad",
                    solve(
                        vec![
                            ("summarize.py", sources::SUMMARIZE_WHOLE_LINE),
                            ("summary.csv", &bad_summary),
                        ],
                        "Wrote summarize.py and summary.csv; the CSV has the header and 9 integer rows.",
                    ),
                ),
            ),
        ],
        "interactive-terminal" => vec![
            (
                "good",
                script(
                    "good",
                    solve(
                        vec![("headless_terminal.py", sources::TERMINAL_PTY)],
                        "HeadlessTerminal runs an interactive bash on a pseudo-terminal.",
                    ),
                ),
            ),
            (
                "bad",
                script(
                    "bad",
                    solve(
                        vec![("headless_terminal.py", sources::TERMINAL_BUILTINS)],
                        "HeadlessTerminal runs each command line and handles control C.",
                    ),
                ),
            ),
        ],
        "cancel-cleanup" => vec![
            (
                "good",
                script(
                    "good",
                    solve(
                        vec![("run.py", sources::RUN_AWAITED)],
                        "run_tasks cancels and awaits started tasks, so their cleanup runs.",
                    ),
                ),
            ),
            (
                "bad",
                script(
                    "bad",
                    solve(
                        vec![("run.py", sources::RUN_EARLY_RETURN)],
                        "run_tasks cancels its workers on interrupt, so cleanup runs.",
                    ),
                ),
            ),
        ],
        "git-recovery" => vec![
            (
                "good",
                script(
                    "good",
                    vec![
                        timed(
                            0,
                            Act::Run {
                                command: "git reflog -n 3".to_string(),
                            },
                        ),
                        timed(
                            50,
                            Act::Run {
                                command: "git reset -q --hard 'HEAD@{1}'".to_string(),
                            },
                        ),
                        timed(100, claim("Restored the lost commit from the reflog.")),
                        timed(100, Act::End { error: false }),
                    ],
                ),
            ),
            (
                "bad",
                script(
                    "bad",
                    solve(
                        vec![("feature.txt", "The feature this repository exists for\n")],
                        "Recreated feature.txt on master.",
                    ),
                ),
            ),
        ],
        _ => Vec::new(),
    }
}

/// The catalog as JSON.
#[must_use]
pub fn catalog_json() -> Value {
    json!(CATALOG.iter().map(|task| json!({
        "id": task.id,
        "family": task.family,
        "instruction_digest": atif::digest(&json!(task.instruction)),
        "scripts": scripts(task).iter().map(|(variant, script)| json!({ "variant": variant, "digest": script.digest() })).collect::<Vec<_>>(),
    })).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-minitask-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn whole_line_counting_differs_from_the_field() {
        assert_ne!(log_summary(false), log_summary(true));
        assert!(log_summary(false).contains("today,ERROR,1\n"));
        assert!(log_summary(true).contains("today,ERROR,2\n"));
        assert!(has_word("x ERROR y", "ERROR"));
        assert!(!has_word("ERROR_CODE", "ERROR"));
    }

    #[tokio::test]
    async fn the_git_task_grades_its_recovery() {
        let task = find("git-recovery").unwrap();
        let dir = scratch("git");
        let work = dir.join("work");
        setup(&task, &work).unwrap();
        assert_eq!(grade(&task, &work, &dir).await.verdict, "failed");
        git(&work, &["reset", "-q", "--hard", "HEAD@{1}"]).unwrap();
        assert_eq!(grade(&task, &work, &dir).await.verdict, "passed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_task_has_a_good_and_a_bad_script() {
        for task in CATALOG {
            let variants: Vec<&str> = scripts(task).iter().map(|(v, _)| *v).collect();
            assert_eq!(variants, ["good", "bad"], "{}", task.id);
        }
    }
}
