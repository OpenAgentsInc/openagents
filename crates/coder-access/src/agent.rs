//! The workshop agent over NIP-HOST: the `studio.agent.*` operations'
//! payloads and the views a device reads (`docs/verse/workshop-agent.md`,
//! "New records and operations").
//!
//! The host is the only authority. It plans each request, gives each
//! command its effect class, journals it, and asks the person before
//! anything that is not read-only; a device sends requests, answers, and,
//! when it drives the agent's terminal pane, what a command printed. Every
//! `studio.agent.*` answer is one [`crate::Outcome::Agent`], whose JSON is
//! one of the types here, bounded at [`MAX_AGENT_BYTES`], as the
//! `background.*` answers are: a write answers [`Dispatched`], `list`
//! answers [`Agents`], `memory.list` [`Memory`], `jobs.list` [`Jobs`], and
//! `log` [`Journal`], `workspaces` [`Places`], and `new` [`Made`].
//!
//! Pause and resume are `studio.seat.pause` and `studio.seat.resume` with
//! the agent's name as the seat.

use serde::{Deserialize, Serialize};

use crate::{Code, Result, fail};

/// The largest `studio.agent.*` answer.
pub const MAX_AGENT_BYTES: usize = 256 * 1024;
/// The longest request text.
pub const MAX_TEXT: usize = 16 * 1024;
/// The longest context a request carries, such as a terminal's working
/// directory and its last block.
pub const MAX_CONTEXT: usize = 8 * 1024;
/// The most output a typist reports for one command.
pub const MAX_OUTPUT: usize = 64 * 1024;
/// The most transcript lines a view carries.
pub const MAX_LINES: usize = 64;
/// The most journal entries one `log` answer carries.
pub const MAX_JOURNAL: usize = 128;

/// Which kind of work a request asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The host chooses.
    #[default]
    Auto,
    /// A change in the agent's own worktree, merged at the Merge station.
    Task,
    /// Commands in a terminal the agent drives.
    Terminal,
}

/// How a command the agent typed into a device's pane ended, as that
/// device reports it (`studio.agent.ran`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ran {
    /// Its exit status, when it finished.
    #[serde(default)]
    pub status: Option<i32>,
    /// Its output block, at most [`MAX_OUTPUT`] bytes.
    #[serde(default)]
    pub output: String,
    /// The person pressed a key in the pane and took it back.
    #[serde(default)]
    pub taken_back: bool,
    /// Why it did not finish, such as the pane closing.
    #[serde(default)]
    pub lost: Option<String>,
}

/// A change to the agent's memory (`studio.agent.memory.edit`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryEdit {
    /// Add a note from the person.
    Note { text: String },
    /// Remove an entry; the journal records that it was forgotten, not
    /// what it said.
    Forget { id: u64 },
    /// Accept a preference the agent proposed, so briefings carry it.
    Accept { id: u64 },
    /// Reject a proposed preference.
    Reject { id: u64 },
}

/// A change to the agent's standing jobs (`studio.agent.jobs.edit`).
/// Creating or renewing one needs the host's own confirmation, at the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobEdit {
    Pause { job: String },
    Resume { job: String },
    Delete { job: String },
}

/// A write's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispatched {
    /// What the write names: the agent, a step, an entry, or a job.
    pub dispatched: String,
}

/// A proposal waiting for the person's CONFIRM or REJECT.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub step: u64,
    pub command: String,
    /// Why it is not read-only.
    pub why: String,
}

/// A command the host checked and wants typed now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub step: u64,
    pub command: String,
    /// Whether a device's pane types it (the request asked for a typist);
    /// otherwise the host runs it itself.
    pub typist: bool,
    /// The directory it runs in.
    pub cwd: String,
    /// A Coder V1 turn rather than a command (#10753): the pane runs
    /// Coder's own terminal following the agent's session, and types
    /// nothing. `command` is then a short description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coder: Option<CoderPane>,
}

/// What an agent's pane runs while Coder V1 works for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoderPane {
    /// The agent's Coder session.
    pub session: String,
    /// The program and arguments: Coder's terminal in follow mode on
    /// `session`. Empty when this computer has no Coder terminal, and the
    /// pane shows nothing new.
    pub argv: Vec<String>,
}

/// A task-mode change and where it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// The studio task that holds it.
    pub task: String,
    /// `working`, `checks`, `merge` (waiting at the Merge station),
    /// `merged`, `rejected`, or `failed`.
    pub stage: String,
}

/// What the agent did that the person accepted: a local count, not XP.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub requests: u64,
    /// Requests it finished without a failure.
    pub finished: u64,
    /// Task-mode changes the person merged.
    pub merged: u64,
}

/// One agent as a device sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentView {
    pub name: String,
    pub look: String,
    /// The route it plans with, or the model that answered last.
    pub route: String,
    /// `active`, `paused`, `stopped`, or `retired`.
    pub state: String,
    /// What it is doing now, as a studio seat's activity.
    pub activity: crate::studio::Activity,
    /// The last report's headline, from host state, never model text.
    pub headline: String,
    pub desk: u32,
    /// Its own key, when it has one.
    #[serde(default)]
    pub pubkey: Option<String>,
    /// When the owner's attestation of that key expires.
    #[serde(default)]
    pub attested_until: Option<u64>,
    /// Its transcript's newest lines, oldest first, ASCII.
    pub lines: Vec<String>,
    #[serde(default)]
    pub pending: Option<Proposal>,
    #[serde(default)]
    pub run: Option<Step>,
    /// A stop asked every pane it drives to be released, with `Ctrl+C` to
    /// the command it started.
    #[serde(default)]
    pub release: u64,
    #[serde(default)]
    pub change: Option<Change>,
    pub service: Service,
    /// Whether a request is under way.
    pub busy: bool,
    /// Standing jobs on, of all.
    pub jobs: [u32; 2],
    /// Preferences it proposed that wait for the person.
    pub candidates: u32,
}

/// `studio.agent.list`'s answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    pub agents: Vec<AgentView>,
}

/// One memory entry as a device reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRow {
    pub id: u64,
    /// `project`, `preference`, `outcome`, or `note`.
    pub kind: String,
    /// `active`, `candidate`, or `rejected`.
    pub state: String,
    pub text: String,
    pub at: u64,
}

/// `studio.agent.memory.list`'s answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub memory: Vec<MemoryRow>,
}

/// One standing job as a device reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRow {
    pub job: String,
    pub title: String,
    /// `schedule`, `issues`, or `checks`.
    pub trigger: String,
    pub enabled: bool,
    pub occurrences: u32,
    pub max_occurrences: u32,
    pub expires_at: u64,
    #[serde(default)]
    pub last: Option<String>,
}

/// `studio.agent.jobs.list`'s answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jobs {
    pub jobs: Vec<JobRow>,
}

/// One journal entry as a device reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalRow {
    /// Its position in the journal, from one.
    pub seq: u64,
    pub at: u64,
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub status: Option<i32>,
}

/// `studio.agent.log`'s answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub journal: Vec<JournalRow>,
}

/// A checkout the host offers a new agent as her workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    /// Its absolute path on the host.
    pub path: String,
    /// Why the host offers it, such as `the studio's repository`.
    pub from: String,
}

/// `studio.agent.workspaces`'s answer, most likely first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Places {
    pub places: Vec<Place>,
}

/// `studio.agent.new`'s answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Made {
    pub agent: String,
    /// The checkout her terminal opens in.
    pub workspace: String,
    /// Her own key's public half.
    pub pubkey: String,
    /// When the owner's attestation of her key expires; `None` when the
    /// host holds no owner key to attest it with.
    #[serde(default)]
    pub attested_until: Option<u64>,
    /// She existed already, and nothing was made.
    #[serde(default)]
    pub existed: bool,
}

/// The longest workspace path `studio.agent.new` carries.
pub const MAX_PATH: usize = 4096;

/// A workspace path: absolute, one line, at most [`MAX_PATH`] bytes.
///
/// # Errors
/// `bounds` for a long path or one with a control character, `malformed`
/// for a relative one.
pub fn workspace_path(value: &str) -> Result<()> {
    if value.len() > MAX_PATH || value.chars().any(char::is_control) {
        return fail(Code::Bounds, "the workspace path exceeds its bound");
    }
    if !value.starts_with('/') {
        return fail(Code::Malformed, "the workspace path is not absolute");
    }
    Ok(())
}

/// An agent name, a studio seat name.
///
/// # Errors
/// `malformed` for anything else.
pub fn name(value: &str) -> Result<()> {
    crate::studio::seat_name(value)
}

/// A request's text.
///
/// # Errors
/// `bounds` for blank or oversized text.
pub fn request_text(value: &str) -> Result<()> {
    crate::studio::text(value, MAX_TEXT)
}

/// A request's context, which may be empty.
///
/// # Errors
/// `bounds` for oversized text.
pub fn context(value: &str) -> Result<()> {
    if value.len() > MAX_CONTEXT {
        return fail(Code::Bounds, "the request's context exceeds its bound");
    }
    Ok(())
}

impl Ran {
    /// # Errors
    /// `bounds` for oversized output.
    pub fn validate(&self) -> Result<()> {
        if self.output.len() > MAX_OUTPUT || self.lost.as_ref().is_some_and(|why| why.len() > 512) {
            return fail(Code::Bounds, "the command's report exceeds its bound");
        }
        Ok(())
    }
}

impl MemoryEdit {
    /// # Errors
    /// `bounds` for a blank or oversized note.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Note { text } => crate::studio::text(text, 2048),
            Self::Forget { .. } | Self::Accept { .. } | Self::Reject { .. } => Ok(()),
        }
    }
}

impl JobEdit {
    /// # Errors
    /// `malformed` for a job ID that is not a studio identity.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Pause { job } | Self::Resume { job } | Self::Delete { job } => {
                crate::studio::id(job)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_and_reports_round_trip_and_bound() {
        let edit = MemoryEdit::Accept { id: 3 };
        let json = serde_json::to_string(&edit).unwrap();
        assert_eq!(json, r#"{"action":"accept","id":3}"#);
        assert_eq!(serde_json::from_str::<MemoryEdit>(&json).unwrap(), edit);
        assert!(MemoryEdit::Note { text: " ".into() }.validate().is_err());
        assert!(
            JobEdit::Pause {
                job: "nightly".into()
            }
            .validate()
            .is_ok()
        );
        assert!(JobEdit::Delete { job: "../x".into() }.validate().is_err());
        let ran = Ran {
            output: "x".repeat(MAX_OUTPUT + 1),
            ..Ran::default()
        };
        assert!(ran.validate().is_err());
        assert!(name("alice").is_ok() && name("Alice").is_err());
        assert!(request_text("run the atif tests").is_ok());
        assert!(context(&"x".repeat(MAX_CONTEXT + 1)).is_err());
        assert!(workspace_path("/Users/me/code/app").is_ok());
        assert!(workspace_path("code/app").is_err());
        assert!(workspace_path("/a\nb").is_err());
        assert!(workspace_path(&format!("/{}", "a".repeat(MAX_PATH))).is_err());
    }
}
