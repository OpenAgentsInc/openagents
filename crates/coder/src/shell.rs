//! The shell side of a turn: the model proposes commands, the host
//! decides whether this turn runs any, and the outcomes go back for the
//! next judgment.
//!
//! A reply is an answer or a plan, and [`Reply::read`] says which under
//! the turn's [`Permit`]. A plan is the whole reply as one JSON object —
//! `{"v":1,"commands":[{"command","why"}]}` — declaring the one schema
//! version this host runs. Everything else is an answer: prose that
//! quotes a plan is prose, and a plan that arrives on a turn permitted to
//! run nothing is refused and stands as the answer. Each [`Proposal`]
//! then runs through [`run`], which reads the permit again before it
//! spawns anything: a bounded `sh -c` with a timeout, an output cap, and
//! a short deny list for the commands that end a machine, not a
//! conversation. [`Outcome`]s fold into the transcript and into the state
//! the shell questions in [`crate::classify`] read.

use std::fmt;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::timeout;

use crate::permit::Permit;

/// The most commands one plan may carry. A plan that carries more is not
/// a plan this host runs, because a truncated plan is a plan nobody
/// wrote.
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// The turn's permit does not run commands, so the host refused it
    /// before reading the command at all.
    Refused(&'static str),
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
            Status::Refused(why) => write!(f, "refused: {why}"),
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

/// The plan schema this host runs. A reply that declares another version
/// is a reply this host cannot read, and a reply it cannot read is not
/// one it runs.
pub const PLAN_VERSION: u64 = 1;

/// A supported plan: the version it declared and the commands it carries,
/// each one already read and complete.
///
/// A value of this type exists only where [`Plan::read`] made one, so a
/// caller holding one is holding a plan the host understands whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The schema version the reply declared, which is [`PLAN_VERSION`].
    pub version: u64,
    /// The commands, in the order the reply asked for them.
    pub proposals: Vec<Proposal>,
}

/// Why a reply is not a plan this host runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotAPlan {
    /// The reply is prose. It may describe a plan, quote one, or wrap one
    /// in a fence with a sentence beside it; none of those is a reply
    /// asking for commands to run.
    Prose,
    /// The reply asks for commands under a version this host does not
    /// run, or under none.
    Version(String),
    /// The reply declares the version this host runs and its commands are
    /// unusable.
    Commands(String),
}

impl NotAPlan {
    /// The sentence the host gives for not running this reply.
    #[must_use]
    pub fn sentence(&self) -> String {
        match self {
            NotAPlan::Prose => "the reply is prose".to_string(),
            NotAPlan::Version(why) | NotAPlan::Commands(why) => why.clone(),
        }
    }
}

impl Plan {
    /// Reads `text` as a plan.
    ///
    /// The whole reply is the plan: one JSON object, or one fenced block
    /// holding one JSON object and nothing else. A reply with a sentence
    /// before or after it is prose whatever the fence contains, which is
    /// what makes an example an example.
    ///
    /// # Errors
    ///
    /// Returns [`NotAPlan::Prose`] when the reply does not ask for
    /// commands at all, and the other variants when it asks under a
    /// version this host does not run or with commands it cannot read.
    /// Only the first is an ordinary answer; the rest are refusals, and a
    /// caller should say so.
    pub fn read(text: &str) -> Result<Self, NotAPlan> {
        let object = whole_object(text).ok_or(NotAPlan::Prose)?;
        let Ok(Value::Object(plan)) = serde_json::from_str::<Value>(object) else {
            return Err(NotAPlan::Prose);
        };
        // A reply that never mentions commands is an answer that happens
        // to be JSON, not a plan the host refused.
        let Some(commands) = plan.get("commands") else {
            return Err(NotAPlan::Prose);
        };
        match plan.get("v").and_then(Value::as_u64) {
            Some(PLAN_VERSION) => {}
            Some(other) => {
                return Err(NotAPlan::Version(format!(
                    "the reply asks for commands under plan version {other}, and this host runs \
                     version {PLAN_VERSION}"
                )));
            }
            None => {
                return Err(NotAPlan::Version(format!(
                    "the reply asks for commands without declaring plan version {PLAN_VERSION}"
                )));
            }
        }
        let Some(commands) = commands.as_array() else {
            return Err(NotAPlan::Commands("commands is not a list".to_string()));
        };
        if commands.is_empty() {
            return Err(NotAPlan::Commands(
                "a plan carries at least one command".to_string(),
            ));
        }
        if commands.len() > COMMANDS_MAX {
            return Err(NotAPlan::Commands(format!(
                "a plan carries at most {COMMANDS_MAX} commands, and this one carries {}",
                commands.len()
            )));
        }
        let mut proposals = Vec::with_capacity(commands.len());
        for (index, command) in commands.iter().enumerate() {
            let proposal = proposal_of(command).map_err(|why| {
                NotAPlan::Commands(format!("command {number}: {why}", number = index + 1))
            })?;
            proposals.push(proposal);
        }
        Ok(Self {
            version: PLAN_VERSION,
            proposals,
        })
    }
}

/// One complete entry of a plan's `commands` list: shell text to run and
/// the reason the terminal displays and the judge reads. A caller gets
/// the whole entry or the sentence saying what is missing from it.
fn proposal_of(command: &Value) -> Result<Proposal, String> {
    let Some(text) = command.get("command").and_then(Value::as_str) else {
        return Err("no command text".to_string());
    };
    let text = text.trim();
    if text.is_empty() {
        return Err("the command is empty".to_string());
    }
    let Some(why) = command.get("why").and_then(Value::as_str) else {
        return Err("no reason for running it".to_string());
    };
    let why = why.trim();
    if why.is_empty() {
        return Err("the reason is empty".to_string());
    }
    Ok(Proposal {
        command: text.to_string(),
        why: why.to_string(),
    })
}

/// The one JSON object a reply is, when the reply is one: the whole text,
/// or the whole text inside one fence. A word before or after it means
/// the reply is prose, whatever it quotes.
fn whole_object(text: &str) -> Option<&str> {
    let text = text.trim();
    if text.starts_with('{') {
        return Some(text);
    }
    let body = text.strip_prefix("```")?;
    let body = body.strip_prefix("json").unwrap_or(body);
    let body = body.strip_suffix("```")?.trim();
    match body.contains("```") {
        true => None,
        false => Some(body),
    }
}

/// What the host made of one reply.
///
/// An answer and a plan are different outcomes, and the difference is the
/// host's to draw: the same text is a plan on a turn permitted to run one
/// and an answer on a turn that is not.
#[derive(Clone, Debug)]
pub enum Reply {
    /// Prose: the turn's answer, as the model wrote it.
    Answer(String),
    /// A supported plan, on a turn permitted to run one.
    Plan(Plan),
    /// A reply that asked for commands the host will not run. The text is
    /// still the answer, and `why` is the sentence for the trace.
    Refused {
        /// What the model wrote.
        text: String,
        /// Why none of it ran.
        why: String,
    },
}

impl Reply {
    /// Reads one reply under the permit the host gave this turn.
    ///
    /// Nothing in `text` can widen `permit`. A valid plan on a turn that
    /// runs nothing comes back [`Reply::Refused`], and so does a plan the
    /// host cannot read on a turn that does.
    #[must_use]
    pub fn read(text: &str, permit: Permit) -> Self {
        let refused = |why: String| Self::Refused {
            text: text.to_string(),
            why,
        };
        match Plan::read(text) {
            Ok(plan) => match permit.refusal() {
                Some(why) => refused(why.to_string()),
                None if plan.proposals.len() > permit.commands() => refused(format!(
                    "this turn runs at most {} commands, and the plan carries {}",
                    permit.commands(),
                    plan.proposals.len()
                )),
                None => Self::Plan(plan),
            },
            Err(NotAPlan::Prose) => Self::Answer(text.to_string()),
            Err(why) => refused(why.sentence()),
        }
    }

    /// What the user reads, which is what the model wrote either way.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Reply::Answer(text) | Reply::Refused { text, .. } => Some(text),
            Reply::Plan(_) => None,
        }
    }
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

/// Runs one proposal: the turn's permit first, then the deny list, then a
/// bounded `sh -c`.
///
/// The permit is read here as well as where the plan was, because this is
/// the function that spawns a process. A caller cannot reach a shell by
/// holding a [`Proposal`]; it has to hold a permit that runs one.
pub async fn run(proposal: &Proposal, permit: Permit) -> Outcome {
    if let Some(why) = permit.refusal() {
        return Outcome {
            proposal: proposal.clone(),
            status: Status::Refused(why),
            output: String::new(),
            elapsed: Duration::ZERO,
        };
    }
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

    /// The plan the instructions describe, read whole.
    const PLAN: &str = r#"{"v":1,"commands":[{"command":"ls crates","why":"list members"},{"command":"git log -1","why":"see the head"}]}"#;

    #[test]
    fn a_json_plan_parses() {
        let plan = Plan::read(PLAN).unwrap();
        assert_eq!(plan.version, PLAN_VERSION);
        assert_eq!(plan.proposals.len(), 2);
        assert_eq!(plan.proposals[0].command, "ls crates");
        assert_eq!(plan.proposals[0].why, "list members");
    }

    #[test]
    fn a_fenced_plan_parses() {
        let text =
            "```json\n{\"v\":1,\"commands\":[{\"command\":\"pwd\",\"why\":\"where am I\"}]}\n```";
        let plan = Plan::read(text).unwrap();
        assert_eq!(plan.proposals[0].command, "pwd");
    }

    #[test]
    fn prose_is_not_a_plan() {
        assert_eq!(
            Plan::read("jev is the classify crate"),
            Err(NotAPlan::Prose)
        );
        assert_eq!(Plan::read(r#"{"answer":"two"}"#), Err(NotAPlan::Prose));
    }

    /// The audit's case: a reply that introduces a plan as an example and
    /// says not to run it. The words are the model's and carry no weight;
    /// what decides it is that the reply is prose with a fence in it.
    #[test]
    fn an_example_in_prose_is_not_a_plan() {
        let text = "Here is an example; do not run it.\n```json\n{\"commands\":[{\"command\":\"printf harmless\"}]}\n```\nThat is the format.";
        assert_eq!(Plan::read(text), Err(NotAPlan::Prose));
    }

    /// The same example, correct in every other way. Prose around the
    /// fence is the whole difference.
    #[test]
    fn a_fence_surrounded_by_prose_is_not_a_plan() {
        let text = format!("I would run this.\n\n```json\n{PLAN}\n```\n\nSay the word.");
        assert_eq!(Plan::read(&text), Err(NotAPlan::Prose));
        let trailing = format!("```json\n{PLAN}\n```\nShall I?");
        assert_eq!(Plan::read(&trailing), Err(NotAPlan::Prose));
        let after_object = format!("{PLAN}\nShall I?");
        assert_eq!(Plan::read(&after_object), Err(NotAPlan::Prose));
    }

    /// A reply that asks for commands under a version this host does not
    /// run is refused rather than read as well as it can be.
    #[test]
    fn an_unsupported_version_is_refused() {
        let missing = r#"{"commands":[{"command":"ls","why":"look"}]}"#;
        assert!(matches!(Plan::read(missing), Err(NotAPlan::Version(_))));
        let later = r#"{"v":2,"commands":[{"command":"ls","why":"look"}]}"#;
        assert!(matches!(Plan::read(later), Err(NotAPlan::Version(_))));
        let word = r#"{"v":"1","commands":[{"command":"ls","why":"look"}]}"#;
        assert!(matches!(Plan::read(word), Err(NotAPlan::Version(_))));
    }

    /// An entry the host cannot read whole refuses the plan it is in.
    /// Running the rest would run a plan nobody wrote.
    #[test]
    fn a_malformed_command_refuses_the_plan() {
        for text in [
            r#"{"v":1,"commands":[]}"#,
            r#"{"v":1,"commands":{"command":"ls","why":"look"}}"#,
            r#"{"v":1,"commands":[{"why":"no command"}]}"#,
            r#"{"v":1,"commands":[{"command":"   ","why":"empty"}]}"#,
            r#"{"v":1,"commands":[{"command":["ls"],"why":"a list"}]}"#,
            r#"{"v":1,"commands":[{"command":"ls"}]}"#,
            r#"{"v":1,"commands":[{"command":"ls","why":" "}]}"#,
            r#"{"v":1,"commands":[{"command":"ls","why":"look"},{"command":""}]}"#,
        ] {
            assert!(
                matches!(Plan::read(text), Err(NotAPlan::Commands(_))),
                "{text}"
            );
        }
    }

    /// More commands than the host runs is a plan it refuses, not a plan
    /// it truncates.
    #[test]
    fn too_many_commands_refuse_the_plan() {
        let commands: Vec<String> = (0..=COMMANDS_MAX)
            .map(|index| format!(r#"{{"command":"echo {index}","why":"count"}}"#))
            .collect();
        let text = format!(r#"{{"v":1,"commands":[{}]}}"#, commands.join(","));
        assert!(matches!(Plan::read(&text), Err(NotAPlan::Commands(_))));
    }

    /// The permit decides, not the text. The same supported plan is a
    /// plan on a turn that runs commands and an answer on one that does
    /// not.
    #[test]
    fn a_plan_is_refused_without_a_permit_to_run_it() {
        assert!(matches!(
            Reply::read(PLAN, Permit::executing()),
            Reply::Plan(_)
        ));
        let Reply::Refused { text, why } = Reply::read(PLAN, Permit::answering()) else {
            panic!("a turn that runs nothing ran a plan");
        };
        assert_eq!(text, PLAN);
        assert_eq!(why, crate::permit::REFUSAL);
    }

    /// A reply the host cannot read is the answer, and the trace says why
    /// nothing ran.
    #[test]
    fn an_unreadable_plan_becomes_the_answer() {
        let text = r#"{"v":9,"commands":[{"command":"ls","why":"look"}]}"#;
        let Reply::Refused { why, .. } = Reply::read(text, Permit::executing()) else {
            panic!("an unsupported version reached execution");
        };
        assert!(why.contains("version 9"), "{why}");
        assert!(matches!(
            Reply::read("jev is the classify crate", Permit::executing()),
            Reply::Answer(_)
        ));
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
        let outcome = run(
            &Proposal {
                command: "printf hello".to_string(),
                why: "check the pipe".to_string(),
            },
            Permit::executing(),
        )
        .await;
        assert!(matches!(outcome.status, Status::Exit(0)));
        assert_eq!(outcome.output, "hello");
    }

    #[tokio::test]
    async fn a_denied_command_never_spawns() {
        let outcome = run(
            &Proposal {
                command: "sudo rm -rf /".to_string(),
                why: "harm".to_string(),
            },
            Permit::executing(),
        )
        .await;
        assert!(matches!(outcome.status, Status::Denied(_)));
    }

    /// The runner asks the permit before it asks the deny list, so a
    /// proposal that reaches it on an answering turn never spawns —
    /// whatever the command is and however it got there.
    #[tokio::test]
    async fn a_command_without_a_permit_never_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let outcome = run(
            &Proposal {
                command: format!("printf harmless > '{}'", marker.display()),
                why: "write a marker".to_string(),
            },
            Permit::answering(),
        )
        .await;
        assert!(matches!(outcome.status, Status::Refused(_)));
        assert!(!marker.exists(), "an unpermitted command ran");
        assert_eq!(outcome.elapsed, Duration::ZERO);
    }
}
