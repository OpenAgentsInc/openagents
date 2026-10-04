//! Questions and approvals an engine raises to the person who sent the task.
//!
//! An engine that cannot go on without the person ends its turn with a
//! question, or with a request to approve a step before it takes it. The
//! turn's reply is the question; the run's result ending names the kind.
//! The task then waits: its summary reports [`nostr::activity_summary::Phase::Waiting`]
//! with input or approval attention, and a device's `answer` command
//! (NIP-HOST `task.command`) starts the next turn with the answer.
//!
//! An answer is data for the engine. An approval answer never widens the
//! task's grant, boundary, routes, or spend: the next turn runs under a
//! fresh grant with every usual check, exactly like any follow-up. It is
//! not a POL approval.
//!
//! An approval's reply may name its step exactly, in a fenced JSON block
//! with the schema [`STEP_SCHEMA`]: the tool, the command, the absolute
//! working directory, and the reason ([`Step::in_reply`]). The host
//! classifies the step's [`Risk`] itself ([`Step::risk`]); an engine
//! cannot lower it. A studio seat's standing rule matches only such a
//! named step, exactly (`studio::rules`).

use super::{Status, Task};
use coder_host::access::studio::{
    MAX_STEP_COMMAND, MAX_STEP_CWD, MAX_STEP_REASON, MAX_STEP_TOOL, Risk,
};
use serde::{Deserialize, Serialize};

/// The result ending of a turn that asked a question.
pub const QUESTION_ENDING: &str = "asked_question";
/// The result ending of a turn that asked for an approval.
pub const APPROVAL_ENDING: &str = "asked_approval";

/// What a waiting turn asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An open question; the answer is free text.
    Question,
    /// Approval of a step the engine named; the answer approves or denies.
    Approval,
}

impl Kind {
    /// The result ending that records this kind.
    #[must_use]
    pub const fn ending(self) -> &'static str {
        match self {
            Self::Question => QUESTION_ENDING,
            Self::Approval => APPROVAL_ENDING,
        }
    }

    /// The kind a result ending records, if it records one.
    #[must_use]
    pub fn of_ending(ending: &str) -> Option<Self> {
        [Self::Question, Self::Approval]
            .into_iter()
            .find(|kind| kind.ending() == ending)
    }
}

/// What the task's ended turn asked, while nothing has answered it: the
/// task is finished and its current run's result ending names a kind. A
/// follow-up of any kind starts a new turn and so ends the wait.
#[must_use]
pub fn pending(task: &Task) -> Option<Kind> {
    if task.status != Status::Finished {
        return None;
    }
    let result = task.run.as_ref()?.result.as_ref()?;
    Kind::of_ending(&result.ending)
}

/// The schema of the fenced JSON block that names an approval's step.
pub const STEP_SCHEMA: &str = "openagents.coder.approval-step.v1";

/// The step an approval asks to take, as its engine named it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub schema: String,
    /// The tool, such as `shell`.
    pub tool: String,
    /// The exact command.
    pub command: String,
    /// The absolute directory the command runs in.
    pub cwd: String,
    /// Why the engine asks; may be empty.
    #[serde(default)]
    pub reason: String,
}

/// Destructive, irreversible, publishing, or privileged: approved once at
/// a time. Matched against the lowercased tool, command, and reason.
const HIGH: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "rm -r ",
    "rmdir",
    "reset --hard",
    "git clean",
    "git push",
    "--force",
    "publish",
    "sudo ",
    "chmod",
    "chown",
    "mkfs",
    "dd if=",
    "drop table",
    "drop database",
    "credential",
    "secret",
    "password",
    "api key",
    "api_key",
    "private key",
    "keychain",
    ".git/",
    "deletes",
    "delete ",
    "rewrites history",
];

/// Reaches the network, brings in code, or changes shared repository
/// state.
const MEDIUM: &[&str] = &[
    "curl ",
    "wget ",
    "http://",
    "https://",
    "ssh ",
    "scp ",
    "rsync ",
    "install",
    "download",
    "dependenc",
    "npx ",
    "npm ",
    "pip ",
    "cargo add",
    "brew ",
    "apt ",
    "apt-get",
    "docker ",
    "git checkout",
    "git switch",
    "git branch",
    "git tag",
    "git rebase",
    "git merge",
    "git fetch",
    "git pull",
    "network",
];

impl Step {
    /// The step a reply names: the last fenced block holding
    /// [`STEP_SCHEMA`] that reads as a step within its bounds. `None`
    /// when the reply names none, which leaves the approval as text.
    #[must_use]
    pub fn in_reply(reply: &str) -> Option<Self> {
        let mut found = None;
        let mut rest = reply;
        while let Some(start) = rest.find("```") {
            let after = &rest[start + 3..];
            let body_start = after.find('\n').map_or(after.len(), |line| line + 1);
            let body = &after[body_start..];
            let Some(end) = body.find("```") else {
                break;
            };
            let block = body[..end].trim();
            if block.contains(STEP_SCHEMA)
                && let Ok(step) = serde_json::from_str::<Self>(block)
                && step.valid()
            {
                found = Some(step);
            }
            rest = &body[end + 3..];
        }
        found
    }

    /// Whether the step is within its bounds: a single-line tool and
    /// directory, an absolute directory, a command that is not blank, and
    /// no control characters but line breaks and tabs in the command and
    /// the reason.
    #[must_use]
    pub fn valid(&self) -> bool {
        let one_line = |value: &str, max: usize| {
            !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
        };
        let lines = |value: &str, max: usize| {
            value.len() <= max
                && !value
                    .chars()
                    .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        };
        self.schema == STEP_SCHEMA
            && one_line(&self.tool, MAX_STEP_TOOL)
            && !self.command.trim().is_empty()
            && lines(&self.command, MAX_STEP_COMMAND)
            && one_line(&self.cwd, MAX_STEP_CWD)
            && self.cwd.starts_with('/')
            && lines(&self.reason, MAX_STEP_REASON)
    }

    /// The host's risk for the step, from its tool, command, and reason.
    /// A keyword classification: a word can only raise a step's risk, and
    /// the risk grants nothing.
    #[must_use]
    pub fn risk(&self) -> Risk {
        let text = format!("{} {} {}", self.tool, self.command, self.reason).to_lowercase();
        if HIGH.iter().any(|word| text.contains(word)) {
            Risk::High
        } else if MEDIUM.iter().any(|word| text.contains(word)) {
            Risk::Medium
        } else {
            Risk::Low
        }
    }
}

/// `reply` without the fenced blocks that name a step, which a prompt
/// shows as fields instead.
#[must_use]
pub fn without_step(reply: &str) -> String {
    let mut out = String::new();
    let mut rest = reply;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map_or(after.len(), |line| line + 1);
        let body = &after[body_start..];
        let Some(end) = body.find("```") else {
            break;
        };
        let block_end = start + 3 + body_start + end + 3;
        if body[..end].contains(STEP_SCHEMA) {
            out.push_str(&rest[..start]);
        } else {
            out.push_str(&rest[..block_end]);
        }
        rest = &rest[block_end..];
    }
    out.push_str(rest);
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_shows_the_reply_without_its_step_block() {
        let text = reply(
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"shell","command":"ls","cwd":"/repo"}"#,
        );
        assert_eq!(without_step(&text), "May I run the tests?");
        let other = "Look:\n\n```sh\nls\n```\n\nMay I?";
        assert_eq!(without_step(other), other);
    }

    fn reply(json: &str) -> String {
        format!("May I run the tests?\n\n```json\n{json}\n```\n")
    }

    #[test]
    fn a_reply_names_its_step_in_a_fenced_block() {
        let text = reply(
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"shell","command":"cargo test -p coder","cwd":"/work/repo","reason":"checks the change"}"#,
        );
        let step = Step::in_reply(&text).unwrap();
        assert_eq!(step.tool, "shell");
        assert_eq!(step.command, "cargo test -p coder");
        assert_eq!(step.cwd, "/work/repo");
        assert_eq!(step.reason, "checks the change");
        assert_eq!(step.risk(), Risk::Low);
        assert_eq!(Step::in_reply("May I go ahead?"), None);
    }

    #[test]
    fn a_step_outside_its_bounds_is_no_step() {
        for json in [
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"shell","command":"ls","cwd":"repo"}"#,
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"","command":"ls","cwd":"/repo"}"#,
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"shell","command":"  ","cwd":"/repo"}"#,
            r#"{"schema":"openagents.coder.approval-step.v2","tool":"shell","command":"ls","cwd":"/repo"}"#,
            r#"{"schema":"openagents.coder.approval-step.v1","tool":"shell","command":"ls","cwd":"/repo","grant":"all"}"#,
        ] {
            assert_eq!(Step::in_reply(&reply(json)), None, "{json}");
        }
    }

    #[test]
    fn the_host_classifies_risk_from_the_step() {
        let step = |command: &str, reason: &str| Step {
            schema: STEP_SCHEMA.into(),
            tool: "shell".into(),
            command: command.into(),
            cwd: "/work/repo".into(),
            reason: reason.into(),
        };
        assert_eq!(step("cargo fmt", "").risk(), Risk::Low);
        assert_eq!(step("curl https://example.com", "").risk(), Risk::Medium);
        assert_eq!(
            step("cargo build", "downloads dependencies").risk(),
            Risk::Medium
        );
        assert_eq!(step("rm -rf target", "").risk(), Risk::High);
        assert_eq!(step("git push origin main", "").risk(), Risk::High);
        assert_eq!(step("cargo publish", "").risk(), Risk::High);
    }
}
