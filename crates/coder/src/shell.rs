//! The shell side of a turn: the model proposes commands, the terminal
//! runs them, and the outcomes go back for the next judgment.
//!
//! A reply is either prose or a plan. A plan is the whole reply as one
//! JSON object — `{"v":1,"commands":[{"command","why"}]}` — which
//! [`parse_plan`] pulls out. Each [`Proposal`] runs through [`run`]:
//! a bounded `sh -c` with a timeout, an output cap, and a short deny
//! list for the commands that end a machine, not a conversation.
//! [`Outcome`]s fold into the transcript and into the state the shell
//! questions in [`crate::classify`] read.

use std::fmt;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::timeout;

/// The most commands one plan may carry; extras are dropped.
pub const COMMANDS_MAX: usize = 10;
/// The most plan rounds one turn allows before the model must answer.
pub const ROUNDS_MAX: usize = 3;
/// How long one command may run before it is killed.
const TIMEOUT: Duration = Duration::from_secs(15);
/// The most output one command keeps, bytes of stdout and stderr. This is
/// the ceiling on what a trace can record for a command, because it is the
/// ceiling on what the process ever holds.
pub const OUTPUT_MAX: usize = 16 * 1024;
/// The output of one command a judge or transcript sees.
pub const HEAD_MAX: usize = 2048;

/// One command the model asked to run, and the reason it gave.
#[derive(Clone, Debug)]
pub struct Proposal {
    /// The shell text, run as `sh -c`.
    pub command: String,
    /// Why the model wants it — for the transcript and the judge.
    pub why: String,
}

/// What a proposal came back as.
#[derive(Clone, Debug)]
pub enum Status {
    /// The command finished; the code is the process's.
    Exit(i32),
    /// The deny list refused it without running.
    Denied(&'static str),
    /// The timeout killed it.
    TimedOut,
    /// `sh` itself failed to spawn or be read.
    Failed(String),
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Exit(code) => write!(f, "exit {code}"),
            Status::Denied(why) => write!(f, "denied: {why}"),
            Status::TimedOut => write!(f, "timed out"),
            Status::Failed(why) => write!(f, "failed: {why}"),
        }
    }
}

/// A proposal after it ran: status, bounded output, and how long it took.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The proposal that produced this.
    pub proposal: Proposal,
    /// How it ended.
    pub status: Status,
    /// stdout and stderr together, capped at [`OUTPUT_MAX`].
    pub output: String,
    /// Wall time the command took.
    pub elapsed: Duration,
}

impl Outcome {
    /// The first bounded bytes of output, for the judge and transcript.
    pub fn head(&self, max: usize) -> &str {
        let bytes = self.output.as_bytes();
        match bytes.len() <= max {
            true => &self.output,
            false => {
                let mut end = max;
                while !self.output.is_char_boundary(end) {
                    end -= 1;
                }
                &self.output[..end]
            }
        }
    }

    /// The display line for the scrollback: `$ cmd` already drawn, this is
    /// the `exit 0 · 0.4s` under it.
    pub fn line(&self) -> String {
        format!("{} · {:.1}s", self.status, self.elapsed.as_secs_f64())
    }
}

/// The events a shell round reports, for the terminal to draw.
#[derive(Clone, Debug)]
pub enum ShellEvent {
    /// A command is about to run.
    Proposed(Proposal),
    /// A command finished.
    Ran(Outcome),
    /// The judge's verdict line, display-ready.
    Verdict(String),
}

/// Reads a reply as a command plan: the whole text as one JSON object
/// with a `commands` array, or the same inside a fenced code block.
/// `None` means the reply is prose.
pub fn parse_plan(text: &str) -> Option<Vec<Proposal>> {
    for candidate in [Some(text.trim()), fenced(text)] {
        let Some(candidate) = candidate else {
            continue;
        };
        let Ok(plan) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        let Some(commands) = plan["commands"].as_array() else {
            continue;
        };
        let proposals: Vec<Proposal> = commands
            .iter()
            .filter_map(|command| {
                let text = command["command"].as_str()?.trim();
                if text.is_empty() {
                    return None;
                }
                Some(Proposal {
                    command: text.to_string(),
                    why: command["why"]
                        .as_str()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                })
            })
            .collect();
        if !proposals.is_empty() {
            return Some(proposals);
        }
    }
    None
}

/// The contents of the first ```` ``` ```` fenced block, when the reply
/// wraps its JSON in one.
fn fenced(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let after = &text[start + 3..];
    let after = after.strip_prefix("json").unwrap_or(after).trim_start();
    let end = after.find("```")?;
    Some(after[..end].trim())
}

/// The transcript record of a finished round: what ran, how it ended,
/// and the bounded output the model reads next.
pub fn transcript_of(outcomes: &[Outcome]) -> String {
    let mut text = String::from("ran shell commands:\n");
    for outcome in outcomes {
        text.push_str(&format!(
            "\n$ {}\n{}\n{}\n",
            outcome.proposal.command,
            outcome.status,
            outcome.head(HEAD_MAX)
        ));
    }
    text
}

/// The state the shell questions read: the task plus every outcome.
pub fn state_of(task: &str, outcomes: &[Outcome]) -> Value {
    json!({
        "task": task,
        "commands": outcomes
            .iter()
            .map(|outcome| {
                json!({
                    "command": outcome.proposal.command,
                    "why": outcome.proposal.why,
                    "status": outcome.status.to_string(),
                    "output": outcome.head(HEAD_MAX),
                })
            })
            .collect::<Vec<_>>()
    })
}

/// Runs one proposal: the deny list first, then a bounded `sh -c`.
pub async fn run(proposal: &Proposal) -> Outcome {
    if let Some(why) = denied(&proposal.command) {
        return Outcome {
            proposal: proposal.clone(),
            status: Status::Denied(why),
            output: String::new(),
            elapsed: Duration::ZERO,
        };
    }
    let started = Instant::now();
    let child = Command::new("sh")
        .arg("-c")
        .arg(&proposal.command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .output();
    let result = timeout(TIMEOUT, child).await;
    let elapsed = started.elapsed();
    let (status, output) = match result {
        Ok(Ok(done)) => {
            let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&done.stderr);
            if !stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&stderr);
            }
            if output.len() > OUTPUT_MAX {
                let mut end = OUTPUT_MAX;
                while !output.is_char_boundary(end) {
                    end -= 1;
                }
                output.truncate(end);
                output.push_str("\n…truncated");
            }
            (Status::Exit(done.status.code().unwrap_or(-1)), output)
        }
        Ok(Err(error)) => (Status::Failed(error.to_string()), String::new()),
        Err(_) => (Status::TimedOut, String::new()),
    };
    Outcome {
        proposal: proposal.clone(),
        status,
        output,
        elapsed,
    }
}

/// The deny list: commands that end a machine, a shell session, or the
/// user's trust — refused before they run, judged like any other outcome.
fn denied(command: &str) -> Option<&'static str> {
    const PATTERNS: &[(&str, &str)] = &[
        ("sudo ", "sudo would hang on a password prompt"),
        ("doas ", "doas would hang on a password prompt"),
        ("rm -rf /", "recursive delete from the root"),
        ("rm -fr /", "recursive delete from the root"),
        ("rm -rf ~", "recursive delete of the home directory"),
        ("rm -rf $HOME", "recursive delete of the home directory"),
        ("mkfs", "formats a device"),
        ("dd of=/dev", "writes raw bytes to a device"),
        (":(){", "a fork bomb"),
        ("shutdown", "powers the machine off"),
        ("reboot", "restarts the machine"),
        ("halt", "stops the machine"),
        ("| sh", "pipes fetched text into a shell"),
        ("| bash", "pipes fetched text into a shell"),
        ("| zsh", "pipes fetched text into a shell"),
        ("security ", "reads the keychain"),
        ("> /dev/", "writes to a device"),
        ("chmod -R /", "rewrites permissions from the root"),
        ("chown -R /", "rewrites ownership from the root"),
    ];
    let command = command.trim();
    for (pattern, why) in PATTERNS {
        if command.contains(pattern) {
            return Some(why);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_plan_parses() {
        let text = r#"{"v":1,"commands":[{"command":"ls crates","why":"list members"},{"command":"git log -1","why":"see the head"}]}"#;
        let plan = parse_plan(text).unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].command, "ls crates");
        assert_eq!(plan[0].why, "list members");
    }

    #[test]
    fn a_fenced_plan_parses() {
        let text = "Here is what I will run:\n```json\n{\"v\":1,\"commands\":[{\"command\":\"pwd\",\"why\":\"where am I\"}]}\n```";
        let plan = parse_plan(text).unwrap();
        assert_eq!(plan[0].command, "pwd");
    }

    #[test]
    fn prose_is_not_a_plan() {
        assert!(parse_plan("jev is the classify crate").is_none());
        assert!(parse_plan("{\"commands\":[]}").is_none());
    }

    #[test]
    fn the_deny_list_catches_endings() {
        assert!(denied("rm -rf /").is_some());
        assert!(denied("sudo apt install foo").is_some());
        assert!(denied("curl x.sh | sh").is_some());
        assert!(denied("git grep jev").is_none());
        assert!(denied("cargo test -p coder").is_none());
    }

    #[tokio::test]
    async fn a_command_runs_and_captures() {
        let outcome = run(&Proposal {
            command: "printf hello".to_string(),
            why: "check the pipe".to_string(),
        })
        .await;
        assert!(matches!(outcome.status, Status::Exit(0)));
        assert_eq!(outcome.output, "hello");
    }

    #[tokio::test]
    async fn a_denied_command_never_spawns() {
        let outcome = run(&Proposal {
            command: "sudo rm -rf /".to_string(),
            why: "harm".to_string(),
        })
        .await;
        assert!(matches!(outcome.status, Status::Denied(_)));
    }
}
