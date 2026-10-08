//! The task owner a host hands admitted task operations to.
//!
//! The host checks the device's grant and the `operate` right first; the
//! owner then records the effect durably. Every call carries the NIP-HOST
//! request ID as its idempotency key, so a retry after an uncertain save
//! repeats the same logical operation rather than minting a new one.
//!
//! Creating a task records intent only. It grants no execution authority:
//! the local task owner still needs its own explicit execution grant before
//! anything runs. Steering and cancelling follow the CTRL semantics of the
//! local owner: a steer records a replacement instruction and supersedes a
//! running context, and a cancel requests a stop.

use coder_access::Code;
use coder_access::protocol::{Operation, QueueEdit, TaskCommand, TaskCreate, TaskQueue};
use nostr::activity_summary::{Attention, Phase};

/// A task after an accepted operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRef {
    /// The host-issued task ID: 64 lowercase hexadecimal characters.
    pub task: String,
    /// The task's revision after the operation.
    pub revision: u64,
    /// The phase an activity summary reports.
    pub phase: Phase,
}

/// Something typed the host can say about a task in its summary headline.
/// The host builds the text from this state alone, never from a prompt or
/// engine output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// No admitted model provider had capacity, so the task ended without
    /// running, or stopped when the last one refused. `until` is the
    /// earliest reset, in Unix seconds, when a provider reported one.
    NoCapacity { until: Option<u64> },
    /// The task never started: the process the host launched to own it
    /// ended, or waited too long, before admitting it, and the host ended
    /// the task instead of leaving it queued.
    NotStarted { cause: StartCause },
    /// The task's run ended because the process that owned it ended
    /// without finishing it, killed or crashed or with the computer
    /// restarted (#10124); the host ended the run and kept its record.
    OwnerEnded,
    /// The engine ended its turn with a question and waits for an answer.
    Question,
    /// The engine ended its turn asking to approve a step and waits for the
    /// answer.
    Approval,
    /// The person asked for the engine `asked` (#10076, #10081), and the
    /// running turn started on the provider named `runs` instead, for the
    /// typed reason `why`. `runs` is the host's own provider name, such as
    /// `Codex`, never text from a device or an engine.
    Requested {
        asked: nostr::cj_conversation::Engine,
        runs: &'static str,
        why: Passed,
    },
}

/// Why the engine a person asked for did not start the turn (#10081). The
/// host decides it from the owner's policy, the sign-in probe, and its
/// capacity and usage books at the start, never from text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Passed {
    /// The owner's policy admits no route for it.
    NotAllowed,
    /// It is not signed in on this computer.
    NotSignedIn,
    /// It refused for a usage or rate limit, until `until` when known.
    Refused { until: Option<u64> },
    /// A fresh usage reading put it at or above the policy's threshold.
    NearLimit,
}

impl Passed {
    /// Why, without its name; provider usage windows stay in the record.
    #[must_use]
    pub fn clause(self) -> String {
        match self {
            Passed::NotAllowed => {
                "is not one of the engines this computer's Coder policy allows".into()
            }
            Passed::NotSignedIn => "is not signed in on this computer".into(),
            Passed::Refused { .. } | Passed::NearLimit => "isn't available right now".into(),
        }
    }
}

impl Note {
    /// The summary headline, such as
    /// `No model capacity until 2026-10-03 18:07 UTC`.
    #[must_use]
    pub fn headline(self) -> String {
        match self {
            Note::NoCapacity { until: Some(until) } => {
                format!("No model capacity until {}", utc(until))
            }
            Note::NoCapacity { until: None } => "No model capacity".to_owned(),
            Note::NotStarted { cause } => cause.headline().to_owned(),
            Note::OwnerEnded => "Coder's process ended unexpectedly".to_owned(),
            Note::Question => "Coder asked a question".to_owned(),
            Note::Approval => "Coder asked for approval".to_owned(),
            Note::Requested {
                runs,
                why: Passed::Refused { .. } | Passed::NearLimit,
                ..
            } => {
                format!("{runs} is running.")
            }
            Note::Requested { asked, runs, why } => format!(
                "You asked for {}; it {}, so {} is running.",
                asked.name(),
                why.clause(),
                runs
            ),
        }
    }

    /// The attention a summary with this note carries: a waiting question
    /// asks for input, a waiting approval for an approval.
    #[must_use]
    pub fn attention(self) -> Option<Attention> {
        match self {
            Note::NoCapacity { .. }
            | Note::NotStarted { .. }
            | Note::OwnerEnded
            | Note::Requested { .. } => None,
            Note::Question => Some(Attention::Input),
            Note::Approval => Some(Attention::Approval),
        }
    }
}

/// A studio goal's open decision, which asks the person for an answer
/// through an activity summary of its own (NIP-WS attention `input`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalDecision {
    /// The goal's summary subject: 64 lowercase hex characters the task
    /// owner derives from the goal, never a task's identity.
    pub subject: String,
    /// The coordinator sequence that opened the decision. A later
    /// decision of the same goal has a greater one.
    pub sequence: u64,
    /// The headline, from host state only: the lead seat and the kind of
    /// decision, never engine text.
    pub headline: String,
}

/// Why a task's owner process never admitted it. The owner reports the
/// cause in its launch diagnostic; the host words it from this type alone,
/// never from the owner's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartCause {
    /// The task's first route needs Codex, which is not set up here.
    Codex,
    /// The task's first route needs Claude Code, which is not set up here.
    Claude,
    /// The task's first route needs Devin, which is not set up here.
    Devin,
    /// The task's first route needs OpenCode, which is not set up here.
    OpenCode,
    /// The task's first route needs Grok Build, which is not set up here.
    Grok,
    /// The execution grant does not fit this computer's settings.
    Configuration,
    /// The task owner refused the task, such as when another owner held it.
    Admission,
    /// Another task's run still holds the task's project on this computer
    /// (#10124): its process is alive. A run whose process is gone never
    /// holds it.
    Busy,
    /// The owner process ended without saying why.
    Stopped,
    /// The owner process was still running but had not admitted the task
    /// after the host's admission grace.
    Timeout,
}

impl StartCause {
    /// The sentence a device shows, such as
    /// `Couldn't start: Claude Code isn't set up on this computer`.
    #[must_use]
    pub const fn headline(self) -> &'static str {
        match self {
            StartCause::Codex => "Couldn't start: Codex isn't set up on this computer",
            StartCause::Claude => "Couldn't start: Claude Code isn't set up on this computer",
            StartCause::Devin => "Couldn't start: Devin isn't set up on this computer",
            StartCause::OpenCode => "Couldn't start: OpenCode isn't set up on this computer",
            StartCause::Grok => "Couldn't start: Grok Build isn't set up on this computer",
            StartCause::Configuration => {
                "Couldn't start: the task's settings don't fit this computer"
            }
            StartCause::Admission => {
                "Couldn't start: Coder couldn't take the task on this computer"
            }
            StartCause::Busy => {
                "Couldn't start: another Coder task is still running in this project"
            }
            StartCause::Stopped => "Couldn't start: Coder stopped before starting the task",
            StartCause::Timeout => "Couldn't start: Coder didn't start the task within 2 minutes",
        }
    }

    /// Whether starting the task again may work: an unexplained stop or a
    /// refused admission can be transient, while a missing key or app, or a
    /// settings mismatch, fails the same way until the owner fixes it.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            StartCause::Stopped | StartCause::Admission | StartCause::Busy
        )
    }
}

/// `YYYY-MM-DD HH:MM UTC` for Unix seconds.
#[must_use]
pub fn utc(seconds: u64) -> String {
    format!(
        "{} {:02}:{:02} UTC",
        nostr::git_sign::utc_date(seconds),
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60
    )
}

/// Who sent a durable task command: the device key and, for a device, the
/// grant and epoch the host admitted it under. A deferred effect rechecks
/// these before it runs; the owner has no grant.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub device: String,
    pub grant: Option<String>,
    pub epoch: Option<u64>,
}

/// Whether a principal still holds the `operate` right under the same
/// grant and epoch, now.
pub type Standing<'a> = &'a (dyn Fn(&Principal) -> bool + Sync);

/// A studio worktree the task owner explicitly admits for a separate shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalBinding {
    pub directory: std::path::PathBuf,
    /// Read-only bindings may be inspected by task readers but cannot launch a shell.
    pub interactive: bool,
}

/// Where admitted task operations go.
///
/// Implementations return promptly and never call back into the host.
pub trait Tasks: Send + Sync {
    /// Read the canonical task list under the owner's admitted workspace label.
    /// Observe admission does not replace the task owner's disclosure checks.
    fn task_list(
        &self,
        _query: &coder_access::task_read::ListQuery,
    ) -> Result<coder_access::task_read::List, Code> {
        Err(Code::Unsupported)
    }

    /// Read one exact task revision and its original evidence without effects.
    fn task_read(
        &self,
        _query: &coder_access::task_read::PageQuery,
    ) -> Result<coder_access::task_read::Page, Code> {
        Err(Code::Unsupported)
    }

    /// Read original bytes only under the task, turn, source, and digest pin.
    fn task_original(
        &self,
        _query: &coder_access::task_read::OriginalQuery,
    ) -> Result<coder_access::task_read::OriginalChunk, Code> {
        Err(Code::Unsupported)
    }

    /// Resolve a non-archived studio task's current admitted worktree.
    /// The default grants no shell access.
    fn terminal_binding(&self, _task: &str) -> Result<TerminalBinding, Code> {
        Err(Code::Unsupported)
    }

    /// Record a new task. `key` is the idempotency key.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code>;

    /// Say, before creating it, that the task the create request `key`
    /// makes is for a person who asked for `engine` (#10076). Only the host
    /// calls it, from its own chat's typed `run_coder` offer. A device's
    /// own request names its engine in [`TaskCreate::engine`] (#10081),
    /// which the task store reads the same way; this preference wins when
    /// both are present. The task store puts that engine first when the
    /// owner's policy admits it. The default ignores it.
    fn prefer(&self, _key: &str, _engine: nostr::cj_conversation::Engine) {}

    /// Replace a task's instructions at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn steer(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code>;

    /// Request a task's cancellation at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn cancel(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        reason: &str,
    ) -> Result<TaskRef, Code>;

    /// Take a finished or cancelled task off every device's lists, deleting
    /// nothing. Archiving an archived task succeeds again. The default
    /// refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn archive(&self, _key: &str, _device: &str, _task: &str) -> Result<(), Code> {
        Err(Code::Unsupported)
    }

    /// Keep one chunk of an image `device` will name in `task.create`
    /// (`artifact.put`, [`coder_access::media`]), for that device only,
    /// and answer what is held. The default refuses as `unsupported`, so a
    /// host whose task owner keeps no images refuses before any task.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn put_artifact(
        &self,
        _device: &str,
        _put: &coder_access::media::ArtifactPut,
    ) -> Result<coder_access::media::ArtifactState, Code> {
        Err(Code::Unsupported)
    }

    /// Record and evaluate a durable task command. The device's command ID
    /// is its idempotency key across NIP-HOST requests: a replay returns
    /// the recorded disposition and never runs the command twice. The
    /// default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn command(
        &self,
        _principal: &Principal,
        _command: &TaskCommand,
        _standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        Err(Code::Unsupported)
    }

    /// List or edit a task's held messages for `principal`: the edit lease,
    /// and changes to this device's own messages under it. `standing`
    /// rechecks each sender whose held message the edit lets run. Returns
    /// the queue, and the task when the edit changed it, such as a message
    /// sent now. The default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn queue(
        &self,
        _principal: &Principal,
        _task: &str,
        _edit: &QueueEdit,
        _standing: Standing<'_>,
    ) -> Result<(TaskQueue, Option<TaskRef>), Code> {
        Err(Code::Unsupported)
    }

    /// Evaluate held commands again, such as a queued message after its
    /// task's turn ends. `standing` rechecks each sender's grant before a
    /// deferred command runs. The host calls this periodically, off its
    /// async runtime. The default does nothing.
    fn tick(&self, _standing: Standing<'_>) {}

    /// Every listed task's current revision and phase, including changes
    /// made outside a device operation, such as an auto-started run
    /// finishing. Archived tasks are not listed.
    /// The host publishes a summary when a revision changes. The default
    /// reports none.
    fn current(&self) -> Vec<TaskRef> {
        Vec::new()
    }

    /// A typed note for a task's summary headline, such as why it ended
    /// without running. The default has none.
    fn note(&self, _task: &str) -> Option<Note> {
        None
    }

    /// The headline a waiting task's summary carries while its question or
    /// approval asks for the person, built from host state alone: for a
    /// studio task, its seat and plan entry title. The default has none,
    /// and the note's generic headline stands.
    fn decision_headline(&self, _task: &str) -> Option<String> {
        None
    }

    /// The studio goals whose decision waits on the person. The host
    /// raises each one's own activity summary. The default has none.
    fn goal_decisions(&self) -> Vec<GoalDecision> {
        Vec::new()
    }

    /// Presence capabilities this task owner adds to the host's own
    /// ([`crate::CAPABILITIES`]), read each time presence is published, so a
    /// change of the owner's settings reaches devices with the next
    /// presence. Such as [`coder_access::protocol::CODER_START_AT_ONCE`]
    /// (#10101). The default adds none.
    fn capabilities(&self) -> Vec<String> {
        Vec::new()
    }

    /// A cheap fingerprint of the task store, such as its files' lengths and
    /// modification times, that changes whenever a task or a held command
    /// may have. The host reads it often and runs [`Tasks::tick`] and
    /// [`Tasks::current`] as soon as it moves, so a device hears of a run
    /// starting or ending at once. The default, `None`, leaves the host to
    /// its periodic sweep.
    fn stamp(&self) -> Option<Vec<u8>> {
        None
    }

    /// Whether this owner's task store holds `task` as a local run on this
    /// computer (`openagents chat`, a thread's binding with host `local`)
    /// for the chat thread `thread`. When it does, the host names itself as
    /// the task's host on that thread, so a device opens, follows, and
    /// stops it as any task here. The default, `false`, leaves such a run
    /// outside the host.
    fn local_run(&self, _task: &str, _thread: &str) -> bool {
        false
    }

    /// What `task` changed between its base and its worktree's content now
    /// (`task.review`). The default, and a task with no worktree of its
    /// own, refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn review(&self, _task: &str) -> Result<coder_access::review::TaskReview, Code> {
        Err(Code::Unsupported)
    }

    /// Publish the reviewed change of `task` for `principal`, whose
    /// `operate` right the host checked (`task.publish`). The operation is
    /// keyed by the task and the reviewed revisions. The default refuses as
    /// `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn publish(
        &self,
        _principal: &Principal,
        _task: &str,
        _reviewed: &Reviewed,
    ) -> Result<coder_access::review::Publication, Code> {
        Err(Code::Unsupported)
    }

    /// The Agent Studio as this owner's coordinator holds it, joined with
    /// its tasks, for `studio.snapshot` and `studio.update`
    /// (`docs/verse/agent-studio.md`). Log lines follow the disclosure
    /// rule in [`coder_access::studio`]. The default, an owner without a
    /// studio, refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn studio(&self) -> Result<coder_access::studio::View, Code> {
        Err(Code::Unsupported)
    }

    /// Apply a `studio.*` intent ([`Operation::studio_intent`]) for
    /// `principal`, whose `operate` right the host checked. `key`, the
    /// NIP-HOST request ID, is the idempotency key; `standing` rechecks a
    /// sender before a deferred answer runs. Returns the reference the
    /// receipt names: the goal, seat, or task. The default refuses as
    /// `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn studio_intent(
        &self,
        _key: &str,
        _principal: &Principal,
        _op: &Operation,
        _standing: Standing<'_>,
    ) -> Result<String, Code> {
        Err(Code::Unsupported)
    }

    /// Answer a `studio.agent.*` operation ([`Operation::agent`]) for
    /// `principal`, whose right the host checked. `key`, the NIP-HOST
    /// request ID, keys an ask so a retry asks once. The answer is one of
    /// `coder_access::agent`'s answers as JSON. The default, an owner
    /// without workshop agents, refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn agent(
        &self,
        _key: &str,
        _principal: &Principal,
        _op: &Operation,
    ) -> Result<serde_json::Value, Code> {
        Err(Code::Unsupported)
    }

    /// Make, retire, or rotate a workshop agent for `studio.agent.new`,
    /// `studio.agent.retire`, or `studio.agent.rotate`, which only the
    /// owner's own key sends. `owner` is the owner key this host holds,
    /// which attests her key; `None` when the key lives elsewhere. The
    /// answer to `studio.agent.new` is `coder_access::agent::Made` as
    /// JSON. The default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn new_agent(
        &self,
        _key: &str,
        _principal: &Principal,
        _op: &Operation,
        _owner: Option<&secp256k1::SecretKey>,
    ) -> Result<serde_json::Value, Code> {
        Err(Code::Unsupported)
    }

    /// The workshop agents' reports since the last call, oldest first: the
    /// host notes each into the agent's own chat thread and publishes its
    /// activity summary to every device that holds `observe`. The default
    /// has none.
    fn agent_reports(&self) -> Vec<AgentReport> {
        Vec::new()
    }

    /// Record that `principal`, whose `review` right the host checked,
    /// rejected the studio task `task` at the `reviewed` revisions, with
    /// an optional `reason`. The task's worktree stays for inspection
    /// until it is archived. The default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn studio_reject(
        &self,
        _principal: &Principal,
        _task: &str,
        _reviewed: &Reviewed,
        _reason: &str,
    ) -> Result<(), Code> {
        Err(Code::Unsupported)
    }
}

/// A workshop agent's report (`docs/verse/workshop-agent.md`, "Reporting
/// back"): its activity summary, from host state alone, and the full
/// report its thread keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentReport {
    /// The agent's name, its thread's title.
    pub agent: String,
    /// The summary's subject: 64 lowercase hex characters, the agent's
    /// own, never a task's.
    pub subject: String,
    /// Higher supersedes lower for the subject.
    pub sequence: u64,
    pub phase: nostr::activity_summary::Phase,
    pub attention: nostr::activity_summary::Attention,
    /// The summary's headline, such as `alice: atif tests failed`: host
    /// state, never engine text.
    pub headline: String,
    /// The agent's thread: 32 lowercase hex characters.
    pub thread: String,
    /// The full report for the thread, plain ASCII.
    pub text: String,
}

std::thread_local! {
    /// The sentence a task owner gave for the refusal it is returning on
    /// this thread, with that refusal's code.
    static REASON: std::cell::RefCell<Option<(Code, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// Return `code` after noting `reason`, a plain sentence that says why, so
/// the host can carry it with the refusal where the transport has room for
/// one (the control socket's `message`). A task owner calls it on the
/// thread that answers the operation; the host takes the sentence with
/// [`take_reason`] when the call returns `code`. A NIP-HOST reply over a
/// relay carries only the code.
pub fn refuse(code: Code, reason: impl Into<String>) -> Code {
    let reason = reason.into();
    REASON.with(|slot| *slot.borrow_mut() = Some((code, reason)));
    code
}

/// Take the sentence noted on this thread for a refusal with `code`. A
/// sentence noted for another code is dropped, as is any sentence once
/// taken.
#[must_use]
pub fn take_reason(code: Code) -> Option<String> {
    REASON
        .with(|slot| slot.borrow_mut().take())
        .and_then(|(noted, reason)| (noted == code).then_some(reason))
}

/// The revisions a device reviewed: the identity a publication carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reviewed {
    pub base: String,
    pub head_commit: String,
    pub head: String,
}

/// A host without a task owner. Every task operation refuses as
/// `unavailable`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTasks;

impl Tasks for NoTasks {
    fn create(&self, _: &str, _: &str, _: &TaskCreate) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::activity_summary::{self, SubjectKind, SummaryDraft};

    #[test]
    fn a_no_capacity_note_survives_the_summary_disclosure_rules() {
        let note = Note::NoCapacity {
            until: Some(1_791_050_823),
        };
        assert_eq!(
            note.headline(),
            "No model capacity until 2026-10-03 18:07 UTC"
        );
        let headline = note.headline();
        let summary = activity_summary::encode(&SummaryDraft {
            host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            subject_kind: SubjectKind::Task,
            subject: &"a".repeat(64),
            sequence: 2,
            phase: Phase::Cancelled,
            headline: &headline,
            attention: Attention::None,
            updated_at: 1_790_572_210,
        })
        .unwrap();
        assert_eq!(summary.headline, headline);
        assert_eq!(
            Note::NoCapacity { until: None }.headline(),
            "No model capacity"
        );
    }

    /// The reason a turn did not start on the engine the person asked for
    /// reads plainly and survives the disclosure rules whole, for every
    /// pair and reason (#10081).
    #[test]
    fn a_requested_engine_note_says_plainly_why_another_runs() {
        use nostr::cj_conversation::Engine;
        let note = Note::Requested {
            asked: Engine::ClaudeCode,
            runs: "Codex",
            why: Passed::NotAllowed,
        };
        assert_eq!(
            note.headline(),
            "You asked for Claude Code; it is not one of the engines this computer's Coder policy allows, so Codex is running."
        );
        assert_eq!(note.attention(), None);
        let reasons = [
            Passed::NotAllowed,
            Passed::NotSignedIn,
            Passed::Refused {
                until: Some(1_791_050_823),
            },
            Passed::Refused { until: None },
            Passed::NearLimit,
        ];
        for why in reasons {
            for asked in Engine::ALL {
                for runs in Engine::ALL.map(Engine::name) {
                    let headline = Note::Requested { asked, runs, why }.headline();
                    assert!(!headline.contains("limit"));
                    if matches!(why, Passed::Refused { .. } | Passed::NearLimit) {
                        assert_eq!(headline, format!("{runs} is running."));
                    }
                    let summary = activity_summary::encode(&SummaryDraft {
                        host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                        subject_kind: SubjectKind::Task,
                        subject: &"a".repeat(64),
                        sequence: 2,
                        phase: Phase::Running,
                        headline: &headline,
                        attention: Attention::None,
                        updated_at: 1_790_572_210,
                    })
                    .unwrap();
                    assert_eq!(summary.headline, headline);
                }
            }
            let text = serde_json::to_string(&why).unwrap();
            assert_eq!(serde_json::from_str::<Passed>(&text).unwrap(), why);
        }
        assert_eq!(
            Note::Requested {
                asked: Engine::ClaudeCode,
                runs: "Codex",
                why: Passed::Refused {
                    until: Some(1_791_050_823)
                },
            }
            .headline(),
            "Codex is running."
        );
    }

    #[test]
    fn a_run_whose_owner_ended_says_so_in_plain_words() {
        let note = Note::OwnerEnded;
        assert_eq!(note.attention(), None);
        let headline = note.headline();
        assert_eq!(headline, "Coder's process ended unexpectedly");
        let summary = activity_summary::encode(&SummaryDraft {
            host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            subject_kind: SubjectKind::Task,
            subject: &"a".repeat(64),
            sequence: 3,
            phase: Phase::Cancelled,
            headline: &headline,
            attention: Attention::None,
            updated_at: 1_790_572_210,
        })
        .unwrap();
        assert_eq!(summary.headline, headline);
        assert!(StartCause::Busy.retryable());
        assert!(!StartCause::Busy.headline().contains("couldn't take"));
    }

    #[test]
    fn every_not_started_cause_survives_the_summary_disclosure_rules() {
        for cause in [
            StartCause::Codex,
            StartCause::Claude,
            StartCause::Devin,
            StartCause::OpenCode,
            StartCause::Grok,
            StartCause::Configuration,
            StartCause::Admission,
            StartCause::Busy,
            StartCause::Stopped,
            StartCause::Timeout,
        ] {
            let note = Note::NotStarted { cause };
            assert_eq!(note.attention(), None);
            let headline = note.headline();
            assert!(headline.starts_with("Couldn't start: "), "{headline}");
            let summary = activity_summary::encode(&SummaryDraft {
                host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                subject_kind: SubjectKind::Task,
                subject: &"a".repeat(64),
                sequence: 2,
                phase: Phase::Cancelled,
                headline: &headline,
                attention: Attention::None,
                updated_at: 1_790_572_210,
            })
            .unwrap();
            assert_eq!(summary.headline, headline);
            let text = serde_json::to_string(&cause).unwrap();
            assert_eq!(serde_json::from_str::<StartCause>(&text).unwrap(), cause);
        }
        assert_eq!(
            serde_json::from_str::<StartCause>("\"open_code\"").unwrap(),
            StartCause::OpenCode
        );
    }

    #[test]
    fn a_waiting_question_or_approval_asks_for_attention_without_its_text() {
        for (note, attention) in [
            (Note::Question, Attention::Input),
            (Note::Approval, Attention::Approval),
        ] {
            assert_eq!(note.attention(), Some(attention));
            let headline = note.headline();
            let summary = activity_summary::encode(&SummaryDraft {
                host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                subject_kind: SubjectKind::Task,
                subject: &"a".repeat(64),
                sequence: 3,
                phase: Phase::Waiting,
                headline: &headline,
                attention,
                updated_at: 1_790_572_210,
            })
            .unwrap();
            assert_eq!(summary.attention, attention);
        }
        assert_eq!(Note::NoCapacity { until: None }.attention(), None);
    }
}
