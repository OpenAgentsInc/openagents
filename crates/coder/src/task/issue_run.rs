//! A GitHub issue handed to Coder from a chat, worked on this computer
//! from claim to close.
//!
//! When a person asks a chat to work an issue ("work on #10034", "take
//! OpenAgentsInc/openagents#10034"), the chat router judges the message is
//! coding work and Jev chooses the issue among the references the message
//! names ([`asked`]); the reference itself is a bounded field read only
//! after that, as `AGENTS.md` requires. `openagents chat work --issues`
//! hands several issues over the same way, one flow per issue. A flow:
//!
//! 1. reads the issue, its comments, and the issues it links, and posts a
//!    claim comment ([`CLAIM_MARK`]);
//! 2. starts a local run ([`super::local`]) in Coder's own worktree of the
//!    fetched default branch (`origin/main` here), with the issue as the
//!    prompt, so the engine, provider failover, the dev-tools boundary
//!    (#10045), and the event stream are the chat's own;
//! 3. runs the repository's checks for what the change touches
//!    ([`Checks`]): the tests of each touched Rust package inside a write
//!    boundary, and, as the repository's policy asks, `cargo fmt --check`
//!    and Clippy, plus the issue flow's diff checks; when they find
//!    problems, a fix turn continues the same task, up to
//!    [`Policy::fix_rounds`] times. A turn has no step or time limit
//!    (#10103): it ends when Coder finishes or asks, when the person stops
//!    it, or when the loop's stuck guard finds it repeating a failed
//!    approach without progress, so no turn is continued for running out
//!    of a budget;
//! 4. commits, and lands as the repository's [`Policy`] says: onto the
//!    default branch after a rebase, running the checks again whenever the
//!    rebase moved the base (this repository's policy), as a pull
//!    request, or into the landing queue ([`super::land_queue`]), whose
//!    integrator lands it and closes the issue;
//! 5. comments the commit and the evidence on the issue and closes it
//!    (a queued change's issue stays open until the integrator lands it).
//!
//! A flow never lands a change whose checks failed: a red check, a run
//! that the stuck guard ended, a question instead of a
//! result, or a stop leaves an honest comment with what was tried and the
//! failing output, and the issue open. A committed change that could not
//! land (red after the rebase, a conflict, refused pushes) is pushed to
//! `coder/stranded-<task8>` instead, so the work survives its worktree.
//!
//! Every step shows in the task's event stream: the flow writes its notes
//! and outcome beside the task (`<store>/local/<task>.issue.json`), and
//! [`super::local::Follow`] interleaves them with the turns' events and
//! holds the last turn's ending until the flow ends, so that ending
//! carries the issue link ([`openagents_chat::coder_events::IssueLink`]).
//! The CLI, the desktop, and the phone read the same stream.
//!
//! GitHub is reached through the `gh` CLI the person is signed in to
//! ([`Gh`]); nothing here reads, stores, or prints a token.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openagents_chat::coder_events::{self, CoderEvent, FileChange, IssueLink, Mapper};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use coder_delegate::issue::{Asked, Reference};

use super::Status;
use super::landing;
use super::local::{self, Local, Record};

/// The version of the flow file a follower reads.
pub const FLOW_SCHEMA: &str = "openagents.coder.issue-run.v1";
/// The repository's issue-flow policy, relative to its top level.
pub const POLICY_FILE: &str = ".openagents/coder-issues.json";
/// The claim marks every path writes and reads ([`crate::claim`]).
pub use crate::claim::{CLAIM_MARK, Comment, Gh, RELEASE_MARK};
/// How often a flow reads its task while a turn runs.
const POLL: Duration = Duration::from_millis(1000);
/// The most bytes of the issue's comments the prompt carries.
const COMMENTS_MAX: usize = 6_000;
/// The most issues the body links whose text the prompt carries.
const LINKED_MAX: usize = 3;
/// The most bytes of one linked issue the prompt carries.
const LINKED_BYTES: usize = 1_500;
/// The most bytes of failing output a failure comment quotes.
const FAILING_MAX: usize = 6_000;
/// The most bytes of a diff stat a comment quotes.
const STAT_MAX: usize = 3_000;

// ---------------------------------------------------------------------------
// The flow file: what followers read.

/// One line the flow says between turns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// The turn it follows; 0 is before the first.
    pub after_turn: usize,
    pub text: String,
}

/// An issue flow as its followers read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    pub schema: String,
    pub task: String,
    /// The issue, and, once the flow ends, what it did with it.
    pub link: IssueLink,
    /// What the flow said, in order.
    pub notes: Vec<Note>,
    /// Whether the flow ended; the last turn's ending waits for it.
    pub finished: bool,
    /// The process running the flow, including checks and landing between turns.
    #[serde(default)]
    pub process_id: Option<u32>,
    /// What the flow did, in a sentence, once it ended.
    #[serde(default)]
    pub closing: String,
    /// What the landed change touched, when it landed: the last turn's
    /// ending names these instead of the worktree's difference from its
    /// first base, which a rebase moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileChange>>,
}

impl Flow {
    /// The last turn's ending once the flow ended: the engine's ending
    /// with the issue and the flow's closing sentence.
    #[must_use]
    pub fn ending(&self, end: CoderEvent) -> CoderEvent {
        let link = self.link.clone();
        let closing = self.closing.trim().to_owned();
        let landed = matches!(
            link.outcome.as_str(),
            "landed" | "pull_request" | "queued" | "unchanged"
        );
        let failure = |turn: usize, message: String, ending: &str| {
            CoderEvent::Failure(coder_events::Failure {
                turn,
                message,
                ending: Some(ending.to_owned()),
                resets_at: None,
                issue: Some(link.clone()),
            })
        };
        match end {
            CoderEvent::Result(mut result) if landed => {
                if !closing.is_empty() {
                    result.summary = if result.summary.trim().is_empty() {
                        closing
                    } else {
                        format!("{}\n\n{closing}", result.summary.trim_end())
                    };
                }
                result.insertions = result.files_changed.iter().filter_map(|c| c.added).sum();
                result.deletions = result.files_changed.iter().filter_map(|c| c.removed).sum();
                result.issue = Some(link);
                CoderEvent::Result(result)
            }
            CoderEvent::Result(result) => failure(
                result.turn,
                closing,
                &format!("issue_{}", self.link.outcome),
            ),
            CoderEvent::Failure(mut failed) => {
                if !closing.is_empty() {
                    failed.message = format!("{} {closing}", failed.message.trim_end());
                }
                failed.issue = Some(link);
                CoderEvent::Failure(failed)
            }
            CoderEvent::Question(asked) | CoderEvent::Approval(asked) => failure(
                asked.turn,
                format!(
                    "Coder asked instead of finishing: {} {closing}",
                    asked.text.trim()
                ),
                "issue_asked",
            ),
            CoderEvent::Stopped(mut stopped) => {
                if !closing.is_empty() {
                    stopped.message = format!("{} {closing}", stopped.message.trim_end());
                }
                CoderEvent::Stopped(stopped)
            }
            other => other,
        }
    }
}

fn flow_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.issue.json"))
}

fn stop_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.issue-stop"))
}

/// The issue flow of `task` in `store`, when one works it.
#[must_use]
pub fn load(store: &Path, task: &str) -> Option<Flow> {
    let bytes = std::fs::read(flow_path(store, task)).ok()?;
    serde_json::from_slice::<Flow>(&bytes)
        .ok()
        .filter(|flow| flow.schema == FLOW_SCHEMA && flow.task == task)
}

pub(crate) fn save(store: &Path, flow: &Flow) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(flow).map_err(|e| e.to_string())?;
    super::autostart::write_private(&flow_path(store, &flow.task), &bytes)
}

/// Asks `task`'s issue flow to stop at its next step.
///
/// # Errors
/// The request cannot be written.
pub fn request_stop(store: &Path, task: &str) -> Result<(), String> {
    super::autostart::write_private(&stop_path(store, task), b"stop\n")
}

fn stop_requested(store: &Path, task: &str) -> bool {
    stop_path(store, task).exists()
}

// ---------------------------------------------------------------------------
// The repository's policy.

/// How a green change lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Land {
    /// Rebase onto the default branch and push it.
    Main,
    /// Push a branch and open a pull request.
    PullRequest,
    /// Push a branch and hand it to the landing queue
    /// ([`super::land_queue`]), whose integrator lands it and closes the
    /// issue (#11242).
    Queue,
}

impl Land {
    /// Reads `main`, `pr` (`pull_request`), or `queue`.
    ///
    /// # Errors
    /// Any other word.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word.trim() {
            "main" => Ok(Land::Main),
            "pr" | "pull_request" | "pull-request" => Ok(Land::PullRequest),
            "queue" => Ok(Land::Queue),
            other => Err(format!("--land is `main`, `pr`, or `queue`, not `{other}`")),
        }
    }
}

/// A repository's issue-flow policy, from [`POLICY_FILE`]. A repository
/// without one lands as a pull request and runs only the tests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default = "Policy::pull_request")]
    pub land: Land,
    /// The branch changes start from and land on; `origin/HEAD`'s by
    /// default.
    #[serde(default)]
    pub branch: Option<String>,
    /// How long another's claim comment keeps a queue off an issue.
    #[serde(default = "Policy::six")]
    pub claim_hours: u64,
    /// Fix turns after the checks find problems.
    #[serde(default = "Policy::three")]
    pub fix_rounds: usize,
    /// Run `cargo fmt --check` on each touched package.
    #[serde(default)]
    pub fmt: bool,
    /// Run Clippy with warnings denied on each touched package.
    #[serde(default)]
    pub clippy: bool,
    /// A step limit older policies set for each turn. Turns have no step
    /// or time limit (#10103), so it is read without error and ignored; a
    /// policy need not name it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    /// Continuation turns older policies allowed after a turn ran out of
    /// its step or time limit. With no limits there is nothing to continue
    /// from, so it is read and ignored too. The flow's turns are at most
    /// `1 + fix_rounds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_turns: Option<usize>,
    /// A line appended to each commit message, such as a trailer.
    #[serde(default)]
    pub trailer: Option<String>,
    /// The GitHub Project names a claim moves ([`crate::claim::Project`]).
    #[serde(default)]
    pub project: crate::claim::Project,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            land: Land::PullRequest,
            branch: None,
            claim_hours: Policy::six(),
            fix_rounds: Policy::three(),
            fmt: false,
            clippy: false,
            max_steps: None,
            continue_turns: None,
            trailer: None,
            project: crate::claim::Project::default(),
        }
    }
}

impl Policy {
    fn pull_request() -> Land {
        Land::PullRequest
    }
    fn six() -> u64 {
        6
    }
    fn three() -> usize {
        3
    }

    /// The policy the checkout at `top` commits, or the default.
    ///
    /// # Errors
    /// The file exists and is not a policy.
    pub fn load(top: &Path) -> Result<Self, String> {
        let path = top.join(POLICY_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                format!("{} is not an issue-flow policy: {error}", path.display())
            }),
            Err(_) => Ok(Policy::default()),
        }
    }
}

// ---------------------------------------------------------------------------
// GitHub.

/// An issue as the flow reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub url: String,
    pub open: bool,
    pub comments: Vec<Comment>,
}

/// What the flow asks of GitHub, beyond the claim record ([`crate::claim::Hub`]).
pub trait Tracker: crate::claim::Hub {
    /// `owner/name` of the GitHub repository the checkout at `dir` is.
    ///
    /// # Errors
    /// Why it cannot tell.
    fn repository(&self, dir: &Path) -> Result<String, String>;
    /// # Errors
    /// Why the issue cannot be read.
    fn issue(&self, repository: &str, number: u64) -> Result<Issue, String>;
    /// # Errors
    /// Why the issue was not closed.
    fn close(&self, repository: &str, number: u64) -> Result<(), String>;
    /// Opens a pull request from `branch` onto `base`; returns its URL.
    ///
    /// # Errors
    /// Why it was not opened.
    fn pull_request(
        &self,
        dir: &Path,
        repository: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String>;
    /// The repository's open issues with what a pickup weighs
    /// ([`super::issue_pick`]).
    ///
    /// # Errors
    /// Why they cannot be listed.
    fn open_issues(&self, _repository: &str) -> Result<Vec<super::issue_pick::Open>, String> {
        Err("this tracker cannot list open issues".into())
    }
    /// The repository's open pull requests, for which issues they work.
    ///
    /// # Errors
    /// Why they cannot be listed.
    fn open_pulls(&self, _repository: &str) -> Result<Vec<super::issue_pick::Pull>, String> {
        Err("this tracker cannot list open pull requests".into())
    }
}

use crate::claim::gh;

impl Tracker for Gh {
    fn repository(&self, dir: &Path) -> Result<String, String> {
        gh(
            Some(dir),
            &[
                "repo",
                "view",
                "--json",
                "nameWithOwner",
                "-q",
                ".nameWithOwner",
            ],
        )
        .map(|name| name.trim().to_owned())
    }

    fn issue(&self, repository: &str, number: u64) -> Result<Issue, String> {
        let text = gh(
            None,
            &[
                "issue",
                "view",
                &number.to_string(),
                "-R",
                repository,
                "--json",
                "number,title,body,url,state,comments",
            ],
        )?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("unexpected gh output: {error}"))?;
        Ok(Issue {
            number,
            title: value["title"].as_str().unwrap_or_default().to_owned(),
            body: value["body"].as_str().unwrap_or_default().to_owned(),
            url: value["url"].as_str().unwrap_or_default().to_owned(),
            open: value["state"].as_str() != Some("CLOSED"),
            comments: crate::claim::comments_of(&value),
        })
    }

    fn close(&self, repository: &str, number: u64) -> Result<(), String> {
        gh(
            None,
            &[
                "issue",
                "close",
                &number.to_string(),
                "-R",
                repository,
                "--reason",
                "completed",
            ],
        )
        .map(|_| ())
    }

    fn pull_request(
        &self,
        dir: &Path,
        repository: &str,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        gh(
            Some(dir),
            &[
                "pr", "create", "-R", repository, "--head", branch, "--base", base, "--title",
                title, "--body", body,
            ],
        )
        .map(|url| url.trim().to_owned())
    }

    fn open_issues(&self, repository: &str) -> Result<Vec<super::issue_pick::Open>, String> {
        let text = gh(
            None,
            &[
                "issue",
                "list",
                "-R",
                repository,
                "--state",
                "open",
                "--limit",
                "200",
                "--json",
                "number,title,body,labels,assignees,comments",
            ],
        )?;
        super::issue_pick::parse_issues(&text)
    }

    fn open_pulls(&self, repository: &str) -> Result<Vec<super::issue_pick::Pull>, String> {
        let text = gh(
            None,
            &[
                "pr",
                "list",
                "-R",
                repository,
                "--state",
                "open",
                "--limit",
                "200",
                "--json",
                "title,body,headRefName,closingIssuesReferences",
            ],
        )?;
        super::issue_pick::parse_pulls(&text)
    }
}

/// Why a queue leaves `issue` alone: a claim comment within `hours` that
/// no later release comment answered. A claim is a comment that starts
/// with "Claimed", the workspace's convention for agents, or carries
/// [`CLAIM_MARK`].
///
/// The comments alone; [`crate::claim::held`] also reads the issue's
/// project status, as [`Runner::begin`] does.
#[must_use]
pub fn claimed(issue: &Issue, now: u64, hours: u64) -> Option<String> {
    crate::claim::held(
        issue.number,
        &issue.comments,
        &[],
        now,
        hours,
        &crate::claim::Project::default(),
    )
}

/// The latest claim that has not been released.
fn active_claim(issue: &Issue) -> Option<&Comment> {
    crate::claim::active(&issue.comments)
}

/// The lease root that holds this computer's issue claims (#10764):
/// `OPENAGENTS_LEASE_ROOT`, else beside the task store, as build leases
/// use. A test's root is inside its scratch store, whose parent is the
/// shared temporary directory.
fn claims_root(store: &Path) -> PathBuf {
    if cfg!(test) {
        return store.join("leases");
    }
    std::env::var_os(coder_lease::ROOT_VAR)
        .filter(|root| !root.is_empty())
        .map_or_else(|| super::targets::lease_root(store), PathBuf::from)
}

/// Only a marker backed by this store's local task can be this computer's claim.
fn inactive_own_claim(store: &Path, repository: &str, issue: &Issue) -> Option<String> {
    let body = &active_claim(issue)?.body;
    let task = crate::claim::marker_field(body, "task")?;
    let record = local::record(store, task)?;
    let flow = load(store, task)?;
    if flow.link.number != issue.number
        || !flow.link.repository.eq_ignore_ascii_case(repository)
        || (!flow.finished && flow.process_id.is_some_and(crate::activity::alive))
    {
        return None;
    }
    let mut inbox = super::Store::open(store).ok()?;
    inbox.settle(task).ok()?;
    let current = inbox.show(task).ok()?;
    if !matches!(current.status, Status::Finished | Status::Cancelled)
        || current.intent.workspace.path != record.worktree
    {
        return None;
    }
    Some(format!(
        "Coder is taking #{} again: this computer's previous task is no longer running.",
        issue.number
    ))
}

/// This computer's issue claims that nothing works any more (the
/// background rule `claims`, docs/background): the latest claim on the
/// issue is this store's task's, the issue is open, the flow neither
/// landed nor opened a pull request, and either the task ended or no
/// process has run it for `idle_hours`. A claim another computer or agent
/// made is never one.
#[must_use]
pub fn stale_claims(
    store: &Path,
    tracker: &dyn Tracker,
    now: u64,
    idle_hours: u64,
) -> Vec<background::services::Claim> {
    let Ok(entries) = std::fs::read_dir(store.join("local")) else {
        return Vec::new();
    };
    let mut tasks: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".issue.json").map(str::to_owned)
        })
        .collect();
    tasks.sort();
    let inbox = super::Store::open(store).ok();
    let mut stale = Vec::new();
    for task in tasks {
        let Some(flow) = load(store, &task) else {
            continue;
        };
        if flow.link.number == 0
            || matches!(
                flow.link.outcome.as_str(),
                "landed" | "pull_request" | "queued" | "unchanged"
            )
        {
            continue;
        }
        let running = flow.process_id.is_some_and(crate::activity::alive);
        let ended = inbox
            .as_ref()
            .and_then(|inbox| inbox.show(&task).ok())
            .is_some_and(|task| matches!(task.status, Status::Finished | Status::Cancelled));
        let idle = std::fs::metadata(flow_path(store, &task))
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| now.saturating_sub(since.as_secs()));
        let why = if running {
            continue;
        } else if ended {
            "its task ended".to_owned()
        } else if idle >= idle_hours * 3600 {
            format!("no run for {} hours", idle / 3600)
        } else {
            continue;
        };
        let Ok(issue) = tracker.issue(&flow.link.repository, flow.link.number) else {
            continue;
        };
        let ours = active_claim(&issue)
            .is_some_and(|claim| crate::claim::marker_field(&claim.body, "task") == Some(&*task));
        if issue.open && ours {
            stale.push(background::services::Claim {
                repository: flow.link.repository.clone(),
                number: flow.link.number,
                task,
                why,
            });
        }
    }
    stale
}

/// Release a stale claim with a comment saying why.
///
/// # Errors
/// What GitHub refused.
pub fn release_stale(
    tracker: &dyn Tracker,
    claim: &background::services::Claim,
) -> Result<(), String> {
    let body = format!(
        "Released by this computer's background rule: Coder task {} {}, so #{} is free to take again.\n\n{RELEASE_MARK}",
        claim.task, claim.why, claim.number
    );
    let said = crate::claim::release(
        tracker,
        &claim.repository,
        claim.number,
        Some(&body),
        &crate::claim::Project::default(),
    );
    // The comment is the release; the assignee and project status follow
    // it when they can.
    let failed: Vec<String> = said
        .into_iter()
        .filter(|line| line.starts_with("Could not post"))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join(" "))
    }
}

/// The issues `spec` names: numbers (`10050,10051`, `#10050 #10051`) or,
/// when it names none, a label.
///
/// # Errors
/// The label cannot be listed.
pub fn select(tracker: &dyn Tracker, repository: &str, spec: &str) -> Result<Vec<u64>, String> {
    select_in(tracker, repository, spec, &crate::claim::Project::default())
}

/// [`select`] with the repository's project names: a label is ordered by
/// the repository's project when it has one ([`crate::claim::pickup`]).
///
/// # Errors
/// The label cannot be listed.
pub fn select_in(
    tracker: &dyn Tracker,
    repository: &str,
    spec: &str,
    project: &crate::claim::Project,
) -> Result<Vec<u64>, String> {
    let words: Vec<&str> = spec
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|word| word.trim().trim_start_matches('#'))
        .filter(|word| !word.is_empty())
        .collect();
    if !words.is_empty()
        && words
            .iter()
            .all(|word| word.chars().all(|c| c.is_ascii_digit()))
    {
        let mut numbers: Vec<u64> = Vec::new();
        for word in words {
            let number = word
                .parse::<u64>()
                .map_err(|_| format!("`{word}` is not an issue number"))?;
            if !numbers.contains(&number) {
                numbers.push(number);
            }
        }
        return Ok(numbers);
    }
    let label = spec.trim().strip_prefix("label:").unwrap_or(spec.trim());
    crate::claim::pickup(tracker, repository, project, Some(label)).map(|(numbers, _)| numbers)
}

// ---------------------------------------------------------------------------
// Choosing the issue a chat message asks for.

/// The issue a chat message asks Coder to work, when it asks for one:
/// code finds the references in `request` and `earlier` (bounded fields),
/// and Jev chooses among them or answers none
/// ([`coder_delegate::issue::asked`]). Without Jev, or with no
/// reference, `None`: the message runs as ordinary coding work.
pub async fn asked(
    request: &str,
    earlier: &str,
    jev: Option<jev::Client>,
    workdir: &Path,
) -> Option<Reference> {
    match asked_with(request, earlier, jev, workdir, false).await? {
        Asked::Issue(reference) => Some(reference),
        Asked::Pick => None,
    }
}

async fn asked_with(
    request: &str,
    earlier: &str,
    jev: Option<jev::Client>,
    workdir: &Path,
    pick: bool,
) -> Option<Asked> {
    let request = coder_delegate::terminal::Request {
        workdir: workdir.to_path_buf(),
        request: request.to_owned(),
        earlier: earlier.to_owned(),
        resume: None,
        read_only: false,
        clarify: false,
        agent: coder_delegate::delegate::Agent::Codex,
        model: None,
        binary: None,
        credential: coder_delegate::delegate::Credential::Missing,
        jev: Some(jev?),
        artifacts: std::env::temp_dir(),
        issues: true,
        issue: false,
        review: false,
        extra: (),
    };
    let recorder = coder_delegate::record::Recorder::default();
    let _quiet = coder_delegate::say::capture(Box::new(|_| {}));
    coder_delegate::issue::asked_work(&request, &recorder, pick).await
}

/// [`asked`] on a thread of its own, for a caller that may run inside an
/// async runtime.
#[must_use]
pub fn asked_blocking(request: &str, earlier: &str, workdir: &Path) -> Option<Reference> {
    // No reference, no question: Jev is not reached.
    if coder_delegate::issue::references(request).is_empty()
        && coder_delegate::issue::references(earlier).is_empty()
    {
        return None;
    }
    match on_thread(request, earlier, workdir, false)? {
        Asked::Issue(reference) => Some(reference),
        Asked::Pick => None,
    }
}

/// What a chat message routed as coding work asks of the repository's
/// issues, as Jev judges it: an issue it names, or to pick an open issue
/// nobody is working on ([`super::issue_pick`]). Jev is asked even when
/// the message names no issue, since a pickup names none. On a thread of
/// its own, for a caller that may run inside an async runtime.
#[must_use]
pub fn asked_work_blocking(request: &str, earlier: &str, workdir: &Path) -> Option<Asked> {
    on_thread(request, earlier, workdir, true)
}

fn on_thread(request: &str, earlier: &str, workdir: &Path, pick: bool) -> Option<Asked> {
    let (request, earlier, workdir) = (
        request.to_owned(),
        earlier.to_owned(),
        workdir.to_path_buf(),
    );
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        let (jev, _) = crate::delegate_door::jev_from(&crate::delegate_door::env_value);
        runtime.block_on(asked_with(&request, &earlier, jev, &workdir, pick))
    })
    .join()
    .ok()
    .flatten()
}

/// Pick an open issue of the repository the checkout at `dir` is in,
/// under its claim window ([`super::issue_pick`]).
///
/// # Errors
/// GitHub cannot be read, or no open issue is free.
/// Whether `dir`'s checkout has an `origin` on GitHub, the only place
/// its issues can live. A missing remote, a local path, or another forge
/// is not (#10398).
#[must_use]
pub fn on_github(dir: &Path) -> bool {
    local::git_out(dir, &["remote", "get-url", "origin"])
        .ok()
        .and_then(|url| super::publish::github_repository(url.trim()))
        .is_some()
}

pub fn pick_here(
    tracker: &dyn Tracker,
    dir: &Path,
) -> Result<(String, super::issue_pick::Picked), String> {
    let checkout = local::checkout(dir)?;
    let repository = tracker.repository(&checkout.top)?;
    let policy = Policy::load(&checkout.top)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let picked = super::issue_pick::pick(tracker, &repository, now, &policy)
        .map_err(|why| format!("No open issue of {repository} is free to pick up: {why}."))?;
    Ok((repository, picked))
}

// ---------------------------------------------------------------------------
// The checks.

/// What the checks found and how they ran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checked {
    /// Each problem, with its failing output; empty when green.
    pub problems: Vec<String>,
    /// What ran, in sentences, for the evidence comment.
    pub ran: Vec<String>,
}

/// Runs the repository's checks on the change in a worktree.
pub trait Checks: Send + Sync {
    /// Checks the staged change in `worktree` (the flow stages it).
    fn check(&self, worktree: &Path, policy: &Policy) -> Checked;
}

/// The issue flow's gate ([`coder_delegate::issue::gate`]): the touched
/// packages' tests in a write boundary, and the diff checks; then
/// `cargo fmt --check` and Clippy when the policy asks.
pub struct Gate {
    pub jev: Option<jev::Client>,
    /// The task store, whose build slots the checks build in (#10293);
    /// `None` builds where [`coder_delegate::issue::confined`] says.
    pub store: Option<PathBuf>,
}

/// How long the checks wait for a free build slot before building
/// outside the slots.
const SLOT_WAIT: std::time::Duration = std::time::Duration::from_secs(600);

impl Gate {
    /// A build slot for checking `worktree`'s change, waiting up to
    /// [`SLOT_WAIT`] while every slot is taken; `None` without a store, or
    /// when no slot frees up in time.
    fn slot(&self, worktree: &Path) -> Result<Option<super::targets::Lease>, String> {
        let Some(store) = self.store.as_ref() else {
            return Ok(None);
        };
        let common = std::process::Command::new("git")
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .current_dir(worktree)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if !common.status.success() {
            return Err("cannot find the checks' Git directory".into());
        }
        let common = PathBuf::from(String::from_utf8_lossy(&common.stdout).trim());
        let started = std::time::Instant::now();
        loop {
            match super::targets::Lease::acquire(store, &common) {
                Ok(mut lease) => {
                    // The checks build, so they also hold a counted
                    // `build` lease from the host broker (#10756) while
                    // they run. They stand before a push, so they wait at
                    // `push` or the flow's own priority when more urgent
                    // (#10757).
                    let inherited = coder_lease::Priority::from_env().ok().flatten();
                    lease.hold_build(
                        "coder",
                        SLOT_WAIT,
                        super::targets::check_priority(inherited),
                    );
                    return Ok(Some(lease));
                }
                Err(super::Error::Busy | super::Error::BuildDiskLow { .. })
                    if started.elapsed() < SLOT_WAIT =>
                {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                Err(super::Error::Busy) => return Ok(None),
                Err(error) => return Err(error.to_string()),
            }
        }
    }
}

impl Checks for Gate {
    fn check(&self, worktree: &Path, policy: &Policy) -> Checked {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                return Checked {
                    problems: vec![format!("the checks could not start: {error}")],
                    ran: Vec::new(),
                };
            }
        };
        let said = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let heard = said.clone();
        let _captured = coder_delegate::say::capture(Box::new(move |line| {
            heard.borrow_mut().push(line.trim().to_owned());
        }));
        // The checks build in one of the task store's build slots, held
        // until they end, so they stay in the slot budget (#10293).
        let slot = match self.slot(worktree) {
            Ok(slot) => slot,
            Err(error) => {
                return Checked {
                    problems: vec![error],
                    ran: Vec::new(),
                };
            }
        };
        let target = slot.as_ref().map(|lease| lease.path.as_path());
        runtime.block_on(async {
            let recorder = coder_delegate::record::Recorder::default();
            let (mut problems, tested) = coder_delegate::issue::gate_in(
                worktree,
                self.jev.as_ref(),
                &recorder,
                None,
                target,
            )
            .await;
            let mut ran = Vec::new();
            let packages = tested
                .as_ref()
                .map(|tested| tested.packages.clone())
                .unwrap_or_default();
            if let Some(tested) = &tested {
                ran.push(tested.describe());
            }
            ran.push(
                "The diff checks ran: style, figures with no source, broken links, code that \
                 depends on what changed, and plain wording."
                    .to_owned(),
            );
            if policy.fmt && !packages.is_empty() {
                let mut unformatted = Vec::new();
                let mut left_alone = 0usize;
                let staged = staged_paths(worktree);
                for package in &packages {
                    let output = std::process::Command::new("cargo")
                        .args(["fmt", "-p", package, "--", "--check"])
                        .current_dir(worktree)
                        .stdin(std::process::Stdio::null())
                        .output();
                    match output {
                        Ok(output) if output.status.success() => {}
                        Ok(output) => {
                            let (in_change, untouched) =
                                fmt_drift_in_change(&String::from_utf8_lossy(&output.stdout), &staged);
                            left_alone += untouched;
                            if !in_change.is_empty() {
                                unformatted.push(format!(
                                    "`cargo fmt -p {package} -- --check` finds unformatted code: {}",
                                    clip(&in_change, FAILING_MAX / 2)
                                ));
                            }
                        }
                        Err(error) => {
                            unformatted.push(format!("cargo fmt could not run: {error}"));
                        }
                    }
                }
                let mut line = format!(
                    "`cargo fmt --check` ran on {}: {}",
                    names(&packages),
                    if unformatted.is_empty() {
                        "formatted"
                    } else {
                        "unformatted code"
                    }
                );
                if left_alone > 0 {
                    line.push_str(&format!(
                        "; {left_alone} unformatted file(s) the change does not touch were left \
                         as the base branch has them"
                    ));
                }
                line.push('.');
                ran.push(line);
                problems.extend(unformatted);
            }
            if policy.clippy && !packages.is_empty() {
                use coder_delegate::issue::confined;
                match confined::Setup::for_run_in(worktree, None, target) {
                    Ok(setup) => {
                        let (lints, _) =
                            confined::run_suite(&setup, &packages, confined::Suite::Clippy).await;
                        ran.push(format!(
                            "Clippy with warnings denied ran on {} in the same boundary: {}.",
                            names(&packages),
                            if lints.is_empty() {
                                "no findings"
                            } else {
                                "findings"
                            }
                        ));
                        problems.extend(lints);
                    }
                    Err(why) => problems.push(format!("Clippy could not run: {why}")),
                }
            }
            Checked { problems, ran }
        })
    }
}

/// The paths of the staged change, relative to the worktree.
fn staged_paths(worktree: &Path) -> Vec<String> {
    local::git_out(worktree, &["diff", "--cached", "--name-only"])
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Splits `rustfmt --check` output into the hunks of files in the staged
/// change and the count of unformatted files the change does not touch.
/// Drift the base branch carries in other files is the base branch's, not
/// the change's, so it must not keep the change from landing.
fn fmt_drift_in_change(output: &str, staged: &[String]) -> (String, usize) {
    let mut kept = String::new();
    let mut untouched = std::collections::BTreeSet::new();
    let mut keep = false;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Diff in ") {
            let path = rest.rsplitn(3, ':').nth(2).unwrap_or(rest);
            let path = path.replace('\\', "/");
            keep = staged
                .iter()
                .any(|s| path == *s || path.ends_with(&format!("/{s}")));
            if !keep {
                untouched.insert(path);
            }
        }
        if keep {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    (kept, untouched.len())
}

fn names(packages: &[String]) -> String {
    packages
        .iter()
        .map(|package| format!("`{package}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// The flow.

/// Only one flow lands at a time in this process, so parallel flows
/// rebase onto each other instead of racing the push; [`landing_lock`]
/// does the same across the processes flows are handed to.
static LANDING: Mutex<()> = Mutex::new(());

/// The branch on origin that keeps a change which could not land.
pub(crate) fn stranded_branch(task: &str) -> String {
    format!("coder/stranded-{}", &task[..8.min(task.len())])
}

/// Fetches only branches tied to the lost task. An unavailable remote is an
/// error, not evidence that there was no pushed work.
fn recovery_commit(top: &Path, task: &str) -> Result<Option<(String, String)>, String> {
    if task.is_empty() || !task.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err("The lost task has an invalid identifier.".into());
    }
    let short = &task[..8.min(task.len())];
    let branches = [
        format!("coder/progress-{short}"),
        stranded_branch(task),
        format!("coder/progress-{task}"),
    ];
    let mut args = vec!["ls-remote", "--heads", "origin"];
    let refs: Vec<String> = branches.iter().map(|b| format!("refs/heads/{b}")).collect();
    args.extend(refs.iter().map(String::as_str));
    let listed = local::git_out(top, &args)?;
    for (branch, reference) in branches.iter().zip(&refs) {
        if listed
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(reference))
        {
            local::git_out(top, &["fetch", "--no-tags", "origin", reference])?;
            let commit = local::git_out(top, &["rev-parse", "FETCH_HEAD^{commit}"])?;
            return Ok(Some((branch.clone(), commit.trim().to_owned())));
        }
    }
    Ok(None)
}

/// A replacement may take only the claim of the task on the confirmed lost host.
fn recovery_claim(issue: &Issue, task: &str) -> bool {
    active_claim(issue)
        .is_none_or(|claim| crate::claim::marker_field(&claim.body, "task") == Some(task))
}

/// The task store's landing lock file, held while a flow fetches, rebases
/// and pushes (never while its checks run, #10391), and whether another run
/// holds it now (then the caller says so and waits with `File::lock`).
/// `None` when the file cannot be opened or locked.
fn landing_lock(store: &Path) -> Option<(std::fs::File, bool)> {
    let file = crate::private::file(
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true),
    )
    .open(store.join("local").join("issue-landing.lock"))
    .ok()?;
    match file.try_lock() {
        Ok(()) => Some((file, false)),
        Err(std::fs::TryLockError::WouldBlock) => Some((file, true)),
        Err(std::fs::TryLockError::Error(_)) => None,
    }
}

/// This machine's landing lock as one guard: the in-process mutex and the
/// task store's lock file.
struct LandingGuard {
    _shared: Option<std::fs::File>,
    _local: std::sync::MutexGuard<'static, ()>,
}

/// Why a flow did not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Another claim holds the issue (only a queue checks).
    Claimed(String),
    /// The issue is closed.
    Closed(String),
    /// Anything else, in a sentence.
    Failed(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Claimed(why) | Refused::Closed(why) | Refused::Failed(why) => f.write_str(why),
        }
    }
}

/// A flow that started: its local run, working.
pub struct Started {
    pub record: Record,
    pub issue: Issue,
    pub repository: String,
    work: Work,
}

/// What a started flow carries into [`Started::finish`].
struct Work {
    store: PathBuf,
    local: Arc<Local>,
    tracker: Arc<dyn Tracker>,
    checks: Arc<dyn Checks>,
    policy: Policy,
    branch: String,
    top: PathBuf,
    now: fn() -> u64,
    /// Where the run's artifacts go, when uploads are on (#10227).
    artifacts: Option<Arc<dyn super::run_artifacts::Uploader>>,
    /// The landing queue `Land::Queue` submits to; this machine's
    /// ([`super::land_queue::open_default`]) when `None`.
    queue: Option<Arc<dyn super::land_queue::Store>>,
}

/// Starts issue flows on this computer.
pub struct Runner {
    pub local: Arc<Local>,
    pub tracker: Arc<dyn Tracker>,
    pub checks: Arc<dyn Checks>,
    /// Overrides the repository's landing policy.
    pub land: Option<Land>,
    /// A queue skips claimed issues; a person naming one issue does not.
    pub skip_claimed: bool,
    pub now: fn() -> u64,
    /// Where each run's artifacts go; `None` uploads nothing (#10227).
    pub artifacts: Option<Arc<dyn super::run_artifacts::Uploader>>,
}

impl Runner {
    /// The runner over `store` with `gh`, the gate, and Jev as this
    /// computer reaches it.
    #[must_use]
    pub fn new(store: PathBuf) -> Self {
        let (jev, _) = crate::delegate_door::jev_from(&crate::delegate_door::env_value);
        Runner {
            local: Arc::new(Local::here(store.clone())),
            tracker: Arc::new(Gh),
            checks: Arc::new(Gate {
                jev,
                store: Some(store),
            }),
            land: None,
            skip_claimed: false,
            now: super::autostart::unix_now,
            artifacts: super::run_artifacts::from_env(),
        }
    }

    /// Claims `number` and starts its first turn in a worktree of the
    /// fetched default branch of the checkout `dir` is in, for the chat
    /// `thread`.
    ///
    /// # Errors
    /// Why the flow did not start; nothing was claimed unless the start
    /// itself failed after the claim, which then says so on the issue.
    pub fn begin(
        &self,
        dir: &Path,
        reference: &Reference,
        thread: Option<&str>,
    ) -> Result<Started, Refused> {
        self.begin_recovering(dir, reference, thread, None)
    }

    /// Starts a replacement for a task whose cloud host the orchestrator
    /// confirmed was lost. Other tasks' claims remain protected.
    ///
    /// # Errors
    /// The claim changed, pushed work cannot be fetched, or the flow cannot start.
    pub fn begin_recovering(
        &self,
        dir: &Path,
        reference: &Reference,
        thread: Option<&str>,
        lost_task: Option<&str>,
    ) -> Result<Started, Refused> {
        let checkout = local::checkout(dir).map_err(Refused::Failed)?;
        let mut policy = Policy::load(&checkout.top).map_err(Refused::Failed)?;
        if let Some(land) = self.land {
            policy.land = land;
        }
        let here = self.tracker.repository(&checkout.top).map_err(|why| {
            Refused::Failed(format!(
                "Coder cannot tell which GitHub repository {} is: {why}",
                checkout.top.display()
            ))
        })?;
        let repository = match &reference.repository {
            Some(named) if !named.eq_ignore_ascii_case(&here) => {
                return Err(Refused::Failed(format!(
                    "Coder works the issues of the checkout it runs in ({here}); \
                     {named}#{} is in another repository. Run this from a checkout of {named}.",
                    reference.number
                )));
            }
            _ => here,
        };
        let number = reference.number;
        let issue = self.tracker.issue(&repository, number).map_err(|why| {
            Refused::Failed(format!("Coder could not read {repository}#{number}: {why}"))
        })?;
        if !issue.open {
            return Err(Refused::Closed(format!("{repository}#{number} is closed.")));
        }
        if lost_task.is_some_and(|task| !recovery_claim(&issue, task)) {
            return Err(Refused::Claimed(
                "Another task claimed the issue after the host was lost.".into(),
            ));
        }
        let mut notes = Vec::new();
        let note = |notes: &mut Vec<Note>, text: String| {
            notes.push(Note {
                after_turn: 0,
                text,
            })
        };
        note(
            &mut notes,
            format!("Issue #{number}: {} ({})", issue.title, issue.url),
        );
        // The claim record: the comments, and the issue's project status.
        let items = match self
            .tracker
            .items(&repository, number, &policy.project.field)
        {
            Ok(items) => items,
            Err(why) => {
                if let Some(line) = crate::claim::unreadable_projects(&repository, &why) {
                    note(&mut notes, line);
                }
                Vec::new()
            }
        };
        if let Some(why) = crate::claim::held(
            number,
            &issue.comments,
            &items,
            (self.now)(),
            policy.claim_hours,
            &policy.project,
        ) {
            if lost_task.is_some() {
                note(
                    &mut notes,
                    "Resuming after the previous cloud host was lost.".into(),
                );
            } else if let Some(recovery) =
                inactive_own_claim(self.local.store(), &repository, &issue)
            {
                note(&mut notes, recovery);
            } else if self.skip_claimed {
                return Err(Refused::Claimed(why));
            } else {
                note(
                    &mut notes,
                    format!("{why}. You asked for this issue by name, so Coder works it anyway."),
                );
            }
        }
        // This computer's hold for this process's session (#10764): another
        // live session's hold refuses a queue, as a claim comment does.
        let session = coder_lease::scratch::delegate_session();
        let claimant = coder_lease::claims::Claimant {
            session: session.clone(),
            agent: "coder".into(),
            pid: Some(std::process::id()),
        };
        let hold = |force: bool| {
            coder_lease::claims::claim(
                &claims_root(self.local.store()),
                &repository,
                number,
                &claimant,
                (self.now)().saturating_mul(1_000),
                Duration::from_secs(policy.claim_hours * 3_600),
                force,
                &|_| None,
            )
        };
        match hold(false) {
            Ok(_) => {}
            Err(coder_lease::claims::Refused::Held(held)) if self.skip_claimed => {
                return Err(Refused::Claimed(held.sentence()));
            }
            Err(coder_lease::claims::Refused::Held(held)) => {
                note(
                    &mut notes,
                    format!(
                        "{}. You asked for this issue by name, so Coder takes it over.",
                        held.sentence()
                    ),
                );
                if let Err(why) = hold(true) {
                    note(
                        &mut notes,
                        format!("Coder could not hold #{number}: {why}."),
                    );
                }
            }
            Err(why) => note(
                &mut notes,
                format!("Coder could not hold #{number}: {why}."),
            ),
        }
        let branch = match &policy.branch {
            Some(branch) => branch.clone(),
            None => local::default_branch(&checkout.top),
        };
        landing::fetch(&checkout.top, &branch).map_err(|why| {
            Refused::Failed(format!("Git could not fetch origin/{branch}: {why}"))
        })?;
        let base = format!("origin/{branch}");
        let saved = lost_task
            .map(|task| recovery_commit(&checkout.top, task))
            .transpose()
            .map_err(Refused::Failed)?
            .flatten();
        let mut prompt = prompt(&issue, &self.linked(&repository, &issue));
        if let Some((branch, _)) = &saved {
            let text = format!(
                "Recovered pushed work from `{branch}` after the cloud host was lost. Review it and complete the issue; the full recovered change must pass checks."
            );
            note(&mut notes, text.clone());
            prompt.push_str(&format!("\n\n{text}\n"));
        }

        let title = format!("#{number}: {}", issue.title);
        let local = Arc::clone(&self.local);
        let record = match &saved {
            Some((_, commit)) => local.start_recovered(dir, &base, commit, &title, &prompt, thread),
            None => local.start_from(dir, Some(&base), &title, &prompt, thread),
        }
        .map_err(Refused::Failed)?;
        let land = match policy.land {
            Land::Main => format!("lands it on `{branch}` when the checks pass"),
            Land::PullRequest => "opens a pull request when the checks pass".to_owned(),
            Land::Queue => {
                format!("hands it to the landing queue for `{branch}` when the checks pass")
            }
        };
        let claim = format!(
            "Claimed: Coder is working on this from an OpenAgents chat {} (task `{}`), in its \
             own worktree of `{base}`. It runs the repository's checks, {land}, and comments \
             the evidence here.\n\n{}",
            placement().claim,
            &record.task[..12],
            crate::claim::marker(&format!("task={}", record.task), &session)
        );
        let mut flow = Flow {
            schema: FLOW_SCHEMA.into(),
            task: record.task.clone(),
            link: IssueLink {
                repository: repository.clone(),
                number,
                url: issue.url.clone(),
                title: issue.title.clone(),
                outcome: "working".into(),
                commits: Vec::new(),
                pull_request: None,
                closed: false,
                not_landed: None,
            },
            notes,
            finished: false,
            process_id: Some(std::process::id()),
            closing: String::new(),
            files: None,
        };
        for said in
            crate::claim::claim(&*self.tracker, &repository, number, &claim, &policy.project)
        {
            flow.notes.push(Note {
                after_turn: 0,
                text: said,
            });
        }
        flow.notes.push(Note {
            after_turn: 0,
            text: format!(
                "Working in {} from {base} ({}); Coder {land}.",
                record.worktree,
                &record.base[..record.base.len().min(10)]
            ),
        });
        save(local.store(), &flow).map_err(Refused::Failed)?;
        Ok(Started {
            record,
            issue,
            repository,
            work: Work {
                store: local.store().to_path_buf(),
                local,
                tracker: Arc::clone(&self.tracker),
                checks: Arc::clone(&self.checks),
                policy,
                branch,
                top: checkout.top,
                now: self.now,
                artifacts: self.artifacts.clone(),
                queue: None,
            },
        })
    }

    /// The issues `issue` links, as the prompt quotes them.
    fn linked(&self, repository: &str, issue: &Issue) -> Vec<Issue> {
        coder_delegate::issue::references(&issue.body)
            .into_iter()
            .filter(|reference| {
                reference.number != issue.number
                    && reference
                        .repository
                        .as_deref()
                        .is_none_or(|named| named.eq_ignore_ascii_case(repository))
            })
            .take(LINKED_MAX)
            .filter_map(|reference| self.tracker.issue(repository, reference.number).ok())
            .collect()
    }
}

/// The first turn's prompt: the issue, its comments, the issues it
/// links, and the issue flow's directions.
fn prompt(issue: &Issue, linked: &[Issue]) -> String {
    let mut text = format!(
        "# Issue #{}: {}\n\n{}\n\n{}\n",
        issue.number,
        issue.title,
        issue.body.trim(),
        issue.url
    );
    let comments: Vec<&Comment> = issue
        .comments
        .iter()
        .filter(|comment| {
            !comment.body.contains(CLAIM_MARK)
                && !comment
                    .body
                    .trim_start()
                    .get(..7)
                    .is_some_and(|head| head.eq_ignore_ascii_case("claimed"))
        })
        .collect();
    if !comments.is_empty() {
        let mut quoted = String::new();
        for comment in comments.iter().rev() {
            if quoted.len() > COMMENTS_MAX {
                break;
            }
            quoted = format!("---\n{}\n{quoted}", comment.body.trim());
        }
        text.push_str(&format!(
            "\n## Comments on the issue\n\n{}\n",
            clip(&quoted, COMMENTS_MAX)
        ));
    }
    if !linked.is_empty() {
        text.push_str("\n## Issues it links\n");
        for other in linked {
            text.push_str(&format!(
                "\n### #{}: {}\n\n{}\n",
                other.number,
                other.title,
                clip(other.body.trim(), LINKED_BYTES)
            ));
        }
    }
    text.push_str(&format!(
        "\n{}\n",
        coder_delegate::terminal::ISSUE_DIRECTIONS
    ));
    text
}

/// How one turn ended, as the flow reads it.
enum Turn {
    Finished { summary: String },
    Asked(String),
    Stopped(String),
    Failed(String),
}

impl Started {
    /// Works the flow to its end: waits for each turn, checks, fixes,
    /// lands, comments, and closes. Blocks; run it on a thread of its
    /// own. Returns the flow as its followers read it.
    #[must_use]
    pub fn finish(self) -> Flow {
        let Started {
            record,
            issue,
            repository,
            work,
        } = self;
        let mut flow = load(&work.store, &record.task).unwrap_or_else(|| Flow {
            schema: FLOW_SCHEMA.into(),
            task: record.task.clone(),
            link: IssueLink {
                repository: repository.clone(),
                number: issue.number,
                url: issue.url.clone(),
                title: issue.title.clone(),
                outcome: "working".into(),
                commits: Vec::new(),
                pull_request: None,
                closed: false,
                not_landed: None,
            },
            notes: Vec::new(),
            finished: false,
            process_id: Some(std::process::id()),
            closing: String::new(),
            files: None,
        });
        let worktree = PathBuf::from(&record.worktree);
        let mut run = Run {
            work: &work,
            flow: &mut flow,
            record: &record,
            issue: &issue,
            repository: &repository,
            worktree: &worktree,
            turn: 1,
            summaries: Vec::new(),
            checked: Checked::default(),
            rounds: 0,
            stranded: None,
        };
        run.drive();
        flow
    }
}

// ---------------------------------------------------------------------------
// A flow in a process of its own.

/// The version of the file a flow's own process reads to take it over.
pub const JOB_SCHEMA: &str = "openagents.coder.issue-job.v1";

/// What [`drive`] needs to work a started flow in another process: the
/// issue as it was read, the policy, and where to land.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Job {
    schema: String,
    task: String,
    repository: String,
    issue: Issue,
    policy: Policy,
    branch: String,
    top: PathBuf,
}

fn job_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.issue-job.json"))
}

fn log_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.issue.log"))
}

/// Where a started flow went ([`Started::hand_off`]).
#[derive(Debug)]
pub enum Handed {
    /// A process of its own works it, as any run's engine does: closing
    /// the screen or the shell that started it does not end it.
    Detached { process: u32 },
    /// It was worked here, to its end, because no driver could take it.
    Here(Box<Flow>),
}

impl Started {
    /// Hands the flow to a process of its own, `driver issue-flow --store
    /// STORE --task TASK` (the `microcoder` engine, [`local::controller`]),
    /// and returns once that process has taken it over. Without a driver,
    /// or when it cannot take the flow over (such as an engine older than
    /// this program), the flow is worked here as [`Started::finish`] does.
    #[must_use]
    pub fn hand_off(self, driver: Option<&Path>) -> Handed {
        let Some(driver) = driver else {
            return Handed::Here(Box::new(self.finish()));
        };
        match self.detach(driver) {
            Ok(process) => Handed::Detached { process },
            Err(started) => Handed::Here(Box::new(started.finish())),
        }
    }

    fn detach(self, driver: &Path) -> Result<u32, Self> {
        let store = self.work.store.clone();
        let task = self.record.task.clone();
        let job = Job {
            schema: JOB_SCHEMA.into(),
            task: task.clone(),
            repository: self.repository.clone(),
            issue: self.issue.clone(),
            policy: self.work.policy.clone(),
            branch: self.work.branch.clone(),
            top: self.work.top.clone(),
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&job) else {
            return Err(self);
        };
        if super::autostart::write_private(&job_path(&store, &task), &bytes).is_err() {
            return Err(self);
        }
        let Ok(child) = spawn_driver(driver, &store, &task) else {
            return Err(self);
        };
        let mut child = child;
        let process = child.id();
        // The driver records itself as the flow's process once it has
        // read the job; one that exits first could not take it.
        loop {
            if load(&store, &task).is_some_and(|flow| flow.process_id == Some(process)) {
                return Ok(process);
            }
            if !matches!(child.try_wait(), Ok(None)) {
                return Err(self);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Starts the driver detached: a session of its own on Unix, a process
/// group with no console window on Windows, its output in the flow's log.
fn spawn_driver(driver: &Path, store: &Path, task: &str) -> std::io::Result<std::process::Child> {
    let log = crate::private::file(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true),
    )
    .open(log_path(store, task))?;
    let mut command = std::process::Command::new(driver);
    command
        .arg("issue-flow")
        .arg("--store")
        .arg(store)
        .arg("--task")
        .arg(task)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log.try_clone()?))
        .stderr(std::process::Stdio::from(log));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and uses no parent-memory state.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0200 | 0x0800_0000);
    }
    command.spawn()
}

/// Takes over the started flow of `task` in `store` and works it to its
/// end, in this process: what `microcoder issue-flow` runs. A flow that
/// already ended is returned as it is.
///
/// # Errors
/// The flow, its job, or its run's record is missing or unreadable.
pub fn drive(store: &Path, task: &str) -> Result<Flow, String> {
    drive_with(&Runner::new(store.to_path_buf()), store, task)
}

fn drive_with(runner: &Runner, store: &Path, task: &str) -> Result<Flow, String> {
    let mut flow = load(store, task).ok_or("This task has no issue flow.")?;
    if flow.finished {
        return Ok(flow);
    }
    let bytes = std::fs::read(job_path(store, task))
        .map_err(|_| "This issue flow was not handed to another process.".to_owned())?;
    let job: Job = serde_json::from_slice(&bytes)
        .ok()
        .filter(|job: &Job| job.schema == JOB_SCHEMA && job.task == task)
        .ok_or("This issue flow's job cannot be read.")?;
    let record = local::record(store, task).ok_or("This issue flow's run has no record.")?;
    flow.process_id = Some(std::process::id());
    save(store, &flow)?;
    let started = Started {
        record,
        issue: job.issue,
        repository: job.repository,
        work: Work {
            store: store.to_path_buf(),
            local: Arc::clone(&runner.local),
            tracker: Arc::clone(&runner.tracker),
            checks: Arc::clone(&runner.checks),
            policy: job.policy,
            branch: job.branch,
            top: job.top,
            now: runner.now,
            artifacts: runner.artifacts.clone(),
            queue: None,
        },
    };
    Ok(started.finish())
}

/// Whether `flow` has not ended and the process working it is gone, so
/// nothing will end it.
#[must_use]
pub fn orphaned(flow: &Flow) -> bool {
    !flow.finished
        && flow
            .process_id
            .is_some_and(|process| !crate::activity::alive(process))
}

/// A flow while it works.
struct Run<'a> {
    work: &'a Work,
    flow: &'a mut Flow,
    record: &'a Record,
    issue: &'a Issue,
    repository: &'a str,
    worktree: &'a Path,
    turn: usize,
    summaries: Vec<String>,
    checked: Checked,
    rounds: usize,
    /// The branch a change that failed to land was pushed to, so its
    /// worktree holds nothing that exists only there.
    stranded: Option<String>,
}

impl Run<'_> {
    fn note(&mut self, text: impl Into<String>) {
        self.flow.notes.push(Note {
            after_turn: self.turn,
            text: text.into(),
        });
        let _ = save(&self.work.store, self.flow);
    }

    fn stopping(&self) -> bool {
        stop_requested(&self.work.store, &self.record.task)
    }

    fn drive(&mut self) {
        loop {
            match self.wait() {
                Turn::Finished { summary } => {
                    if !summary.trim().is_empty() {
                        self.summaries.push(summary);
                    }
                }
                Turn::Stopped(why) => return self.stopped(&why),
                Turn::Asked(text) => {
                    return self.failed(
                        &format!(
                            "Coder asked a question instead of finishing: {}",
                            clip(text.trim(), 600)
                        ),
                        None,
                    );
                }
                Turn::Failed(why) => return self.failed(&why, None),
            }
            if self.stopping() {
                return self.stopped("Stopped by the person who started it.");
            }
            let _ = local::git_out(self.worktree, &["add", "-A"]);
            let staged = local::git_out(self.worktree, &["diff", "--cached", "--name-only"])
                .unwrap_or_default();
            if staged.trim().is_empty() {
                return self.unchanged();
            }
            self.note("Running the repository's checks on the change.");
            self.checked = self.work.checks.check(self.worktree, &self.work.policy);
            if self.stopping() {
                return self.stopped("Stopped by the person who started it.");
            }
            if self.checked.problems.is_empty() {
                self.note("The checks pass.");
                break;
            }
            let count = self.checked.problems.len();
            let listed = clip(&self.checked.problems.join("; "), 600);
            if self.rounds >= self.work.policy.fix_rounds {
                self.note(format!(
                    "The checks still find {count} problem(s) after {} fix turn(s): {listed}",
                    self.rounds
                ));
                let problems = self.checked.problems.clone();
                return self.failed(
                    &format!(
                        "The repository's checks still fail after {} fix turn(s), so Coder \
                         pushed nothing.",
                        self.rounds
                    ),
                    Some(&problems),
                );
            }
            self.rounds += 1;
            self.note(format!(
                "The checks find {count} problem(s): {listed}. Coder fixes them (fix turn {} of {}).",
                self.rounds, self.work.policy.fix_rounds
            ));
            let request = coder_delegate::issue::fix_request(
                self.worktree,
                self.issue.number,
                &self.checked.problems,
            );
            // The turn's own checks may still run on the host; the task
            // continues only once they end (#10273).
            let store = self.work.store.clone();
            let task = self.record.task.clone();
            let stopping = || stop_requested(&store, &task);
            match local::await_checks(&store, &task, &stopping, POLL) {
                Ok(true) => {}
                Ok(false) => return self.stopped("Stopped by the person who started it."),
                Err(why) => {
                    let problems = self.checked.problems.clone();
                    return self.failed(
                        &format!("The fix turn could not start: {why}"),
                        Some(&problems),
                    );
                }
            }
            match self.work.local.answer(&self.record.task, &request) {
                Ok(_) => self.turn += 1,
                Err(why) => {
                    let problems = self.checked.problems.clone();
                    return self.failed(
                        &format!("The fix turn could not start: {why}"),
                        Some(&problems),
                    );
                }
            }
        }
        match self.work.policy.land {
            Land::Main => self.land_main(),
            Land::PullRequest => self.land_pull_request(),
            Land::Queue => self.land_queue(),
        }
    }

    /// Waits for the current turn to end.
    fn wait(&mut self) -> Turn {
        let store = &self.work.store;
        let task = &self.record.task;
        let since = local::record(store, task)
            .and_then(|r| r.turns.iter().find(|t| t.turn == self.turn).map(|t| t.at))
            .unwrap_or_else(|| (self.work.now)());
        // A store another process holds is waited out, not the turn's end:
        // the flow fails only once it stays busy past `READER_BUSY_WAIT`.
        let mut reading = super::Reading::default();
        loop {
            if self.stopping() {
                let _ = self.work.local.stop(task);
            }
            // A dead owner leaves no result until the store settles it.
            if let Ok(mut inbox) = super::Store::open(store) {
                let _ = inbox.settle(task);
            }
            let current = match reading.show(store, task) {
                Ok(Some(current)) => current,
                Ok(None) => {
                    std::thread::sleep(POLL);
                    continue;
                }
                Err(error) => {
                    return Turn::Failed(format!("Coder's task could not be read: {error}"));
                }
            };
            let runs: Vec<&super::owner::Run> =
                current.earlier.iter().chain(current.run.iter()).collect();
            match runs.get(self.turn - 1) {
                Some(run) => {
                    if let Some(result) = &run.result {
                        let trace = store.join(&run.admission.trace_file);
                        let mut mapper = Mapper::new(self.turn, None);
                        let steps: Vec<Value> = atif::log::read(&trace)
                            .map(|recording| {
                                recording.document()["steps"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default()
                            })
                            .unwrap_or_default();
                        for step in &steps {
                            let _ = mapper.step(step);
                        }
                        return match mapper.end(&result.ending, Vec::new(), "", "", None) {
                            CoderEvent::Result(finished) => Turn::Finished {
                                summary: finished.summary,
                            },
                            CoderEvent::Question(asked) | CoderEvent::Approval(asked) => {
                                Turn::Asked(asked.text)
                            }
                            CoderEvent::Stopped(stopped) => Turn::Stopped(stopped.message),
                            CoderEvent::Failure(failed) => Turn::Failed(failed.message),
                            _ => Turn::Failed(format!("Coder ended as {}.", result.ending)),
                        };
                    }
                }
                None => {
                    if current.status == Status::Cancelled {
                        return Turn::Stopped("Coder stopped before the turn started.".into());
                    }
                    if let Some(why) = local::unadmitted(store, task, since, (self.work.now)()) {
                        return Turn::Failed(why);
                    }
                }
            }
            std::thread::sleep(POLL);
        }
    }

    /// Commits the staged change; returns the commit.
    fn commit(&mut self) -> Result<String, String> {
        let _ = local::git_out(self.worktree, &["add", "-A"]);
        let what = if self.summaries.is_empty() {
            "Worked by Coder.".to_owned()
        } else {
            self.summaries
                .iter()
                .map(|summary| summary.trim().to_owned())
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        let started = self.record.turns.first();
        let by = started.map_or_else(String::new, |start| {
            format!(" ({} {})", start.provider, start.model)
        });
        let mut body = format!(
            "{}\n\nWorked by Coder from an OpenAgents chat{by} for issue #{}.\nIssue: {}",
            clip(&what, 4_000),
            self.issue.number,
            self.issue.url
        );
        if let Some(trailer) = &self.work.policy.trailer {
            body.push_str(&format!("\n\n{}", trailer.trim()));
        }
        local::git_out(
            self.worktree,
            &["commit", "-q", "-m", &self.issue.title, "-m", &body],
        )
        .map_err(|why| format!("Git could not commit: {why}"))?;
        local::git_out(self.worktree, &["rev-parse", "HEAD"]).map(|head| head.trim().to_owned())
    }

    fn land_main(&mut self) {
        let branch = self.work.branch.clone();
        if self.stopping() {
            return self.stopped("Stopped by the person who started it, before landing.");
        }
        if let Err(why) = self.commit() {
            return self.failed(&why, None);
        }
        let worktree = self.worktree;
        let plan = landing::Plan {
            worktree,
            branch: &branch,
            attempts: landing::Plan::ATTEMPTS,
            backoff: landing::Backoff::LANDING,
        };
        let outcome = landing::land(&plan, self);
        let attempts = match &outcome {
            Ok(landed) => &landed.attempts,
            Err(not) => &not.attempts,
        };
        let tries = landing::summary(attempts, &branch);
        // Whether the change that landed skipped the checks after its last
        // rebase.
        let attempts_skipped = attempts
            .iter()
            .rev()
            .find(|attempt| attempt.recheck != landing::Recheck::NotMoved)
            .is_some_and(|attempt| matches!(attempt.recheck, landing::Recheck::Skipped(_)));
        let landed = match outcome {
            Ok(landed) => landed,
            Err(not) => {
                // A change that could not land is kept on its own branch on
                // origin, so it can be picked up from any computer and its
                // worktree can be removed. A stopped run, or a push that
                // fails, leaves the change staged on its latest base, as the
                // flow leaves every change it did not land.
                if matches!(not.failure, landing::Failure::Stopped) || !self.strand() {
                    let _ = local::git_out(self.worktree, &["reset", "-q", "--soft", "HEAD~1"]);
                }
                let tried = format!("\n\n**Landing**: {tries}");
                self.flow.link.not_landed = match &not.failure {
                    landing::Failure::Conflict(_) => Some("conflict".into()),
                    landing::Failure::GaveUp(_) => Some("push_refused".into()),
                    landing::Failure::Red(_) => Some("checks_failed".into()),
                    landing::Failure::Stopped | landing::Failure::Unreadable(_) => None,
                };
                return match not.failure {
                    landing::Failure::Stopped => {
                        self.stopped("Stopped by the person who started it, before landing.")
                    }
                    landing::Failure::Red(problems) => self.failed(
                        &format!(
                            "After the rebase onto {branch}, the repository's checks fail, so \
                             Coder pushed nothing.{tried}"
                        ),
                        Some(&problems),
                    ),
                    landing::Failure::Conflict(why)
                    | landing::Failure::Unreadable(why)
                    | landing::Failure::GaveUp(why) => self.failed(&format!("{why}{tried}"), None),
                };
            }
        };
        let commit = landed.commit;
        let files = changed_by(self.worktree, &format!("{commit}~1"), &commit);
        self.flow.files = Some(files.clone());
        self.flow.link.commits = vec![commit.clone()];
        self.flow.link.outcome = "landed".into();
        let url = format!("https://github.com/{}/commit/{commit}", self.repository);
        let mut comment = self.evidence(
            &format!(
                "Coder landed this on `{branch}` in [{}]({url}) and closed the issue.",
                &commit[..10]
            ),
            &files,
        );
        if attempts_skipped {
            comment = comment.replace(
                "- The checks passed on the exact change that landed.\n",
                "- The checks passed on the change before its last rebase; the commits it was \
                 rebased over cannot affect what it touches (see Landing).\n",
            );
        }
        comment.push_str(&format!("\n\n**Landing**: {tries}"));
        comment.push_str(&self.artifacts(Some(&format!("{commit}~1"))));
        let commented = self
            .work
            .tracker
            .comment(self.repository, self.issue.number, &comment);
        let closed = self.work.tracker.close(self.repository, self.issue.number);
        self.flow.link.closed = closed.is_ok();
        for said in crate::claim::done(
            &*self.work.tracker,
            self.repository,
            self.issue.number,
            &self.work.policy.project,
        ) {
            self.note(said);
        }
        let mut closing = format!("Landed {} on {branch}", &commit[..10]);
        match (&commented, &closed) {
            (Ok(()), Ok(())) => closing.push_str(&format!(
                ", commented the evidence, and closed #{}.",
                self.issue.number
            )),
            _ => closing.push_str(&format!(
                "; {}.",
                [commented.err(), closed.err()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ")
            )),
        }
        self.end(closing)
    }

    /// Push the committed change that failed to land to
    /// `coder/stranded-<task8>` on origin. Whether it was pushed.
    fn strand(&mut self) -> bool {
        let branch = stranded_branch(&self.record.task);
        match local::git_out(
            self.worktree,
            &[
                "push",
                "-q",
                "-f",
                "origin",
                &format!("HEAD:refs/heads/{branch}"),
            ],
        ) {
            Ok(_) => {
                self.note(format!("Kept the change on `{branch}` on origin."));
                self.stranded = Some(branch);
                true
            }
            Err(why) => {
                self.note(format!(
                    "Could not keep the change on `{branch}`: {}",
                    clip(&why, 300)
                ));
                false
            }
        }
    }

    fn land_pull_request(&mut self) {
        let commit = match self.commit() {
            Ok(commit) => commit,
            Err(why) => return self.failed(&why, None),
        };
        let branch = format!(
            "coder/issue-{}-{}",
            self.issue.number,
            &self.record.task[..8]
        );
        if let Err(why) = local::git_out(
            self.worktree,
            &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
        ) {
            self.flow.link.not_landed = Some("push_refused".into());
            return self.failed(&format!("Git could not push {branch}: {why}"), None);
        }
        let files = changed_by(self.worktree, &format!("{commit}~1"), &commit);
        self.flow.files = Some(files.clone());
        self.flow.link.commits = vec![commit.clone()];
        let body = self.evidence(&format!("Coder's change for {}.", self.issue.url), &files);
        let body = format!(
            "{body}{}\n\nCloses #{}",
            self.artifacts(Some(&format!("{commit}~1"))),
            self.issue.number
        );
        match self.work.tracker.pull_request(
            &self.work.top,
            self.repository,
            &branch,
            &self.work.branch,
            &self.issue.title,
            &body,
        ) {
            Ok(url) => {
                self.flow.link.outcome = "pull_request".into();
                self.flow.link.pull_request = Some(url.clone());
                let _ = self.work.tracker.comment(
                    self.repository,
                    self.issue.number,
                    &format!("Coder opened {url} for this issue; the checks pass on it."),
                );
                self.end(format!("Opened pull request {url}."))
            }
            Err(why) => self.failed(&format!("The pull request was not opened: {why}"), None),
        }
    }

    /// Pushes the committed change to `land/<entry id>` on origin and
    /// submits it to the landing queue (#11242). The integrator rebases,
    /// checks, and lands it, then closes the issue and moves the board, or
    /// comments why it bounced; the flow closes nothing itself.
    fn land_queue(&mut self) {
        use super::land_queue;
        if self.stopping() {
            return self.stopped("Stopped by the person who started it, before landing.");
        }
        let commit = match self.commit() {
            Ok(commit) => commit,
            Err(why) => return self.failed(&why, None),
        };
        let at = land_queue::now();
        let machine = land_queue::machine();
        let id = land_queue::new_id(at, &machine);
        let branch = format!("land/{id}");
        if let Err(why) = local::git_out(
            self.worktree,
            &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
        ) {
            let _ = local::git_out(self.worktree, &["reset", "-q", "--soft", "HEAD~1"]);
            self.flow.link.not_landed = Some("push_refused".into());
            return self.failed(&format!("Git could not push {branch}: {why}"), None);
        }
        self.stranded = Some(branch.clone());
        let store = match &self.work.queue {
            Some(store) => Ok(Arc::clone(store)),
            None => land_queue::open_default(None).map(Arc::from),
        };
        let entry = land_queue::Entry {
            id: id.clone(),
            branch: branch.clone(),
            target: self.work.branch.clone(),
            issue: Some(self.issue.number),
            close: true,
            author: local::git_out(self.worktree, &["config", "user.name"])
                .map(|name| name.trim().to_owned())
                .unwrap_or_default(),
            machine,
            summary: self.issue.title.clone(),
            head: commit.clone(),
            submitted_at: at,
            state: land_queue::State::Queued,
            updated_at: at,
            tries: 0,
            commit: None,
            reason: None,
            worker: None,
        };
        let submitted = store.and_then(|store| {
            land_queue::Queue { store: &*store }
                .submit(&entry)
                .map(|()| store.location())
        });
        let location = match submitted {
            Ok(location) => location,
            Err(why) => {
                return self.failed(
                    &format!("The landing queue did not take the change: {why}"),
                    None,
                );
            }
        };
        self.note(format!("Queued {id} in {location}."));
        let files = changed_by(self.worktree, &format!("{commit}~1"), &commit);
        self.flow.files = Some(files.clone());
        self.flow.link.commits = vec![commit.clone()];
        self.flow.link.outcome = "queued".into();
        let tree = format!("https://github.com/{}/tree/{branch}", self.repository);
        let mut comment = self.evidence(
            &format!(
                "Coder queued this to land on `{}`: entry `{id}`, branch [`{branch}`]({tree}). \
                 The integrator lands it and closes the issue, or comments here if it bounces.",
                self.work.branch
            ),
            &files,
        );
        comment = comment.replace(
            "- The checks passed on the exact change that landed.\n",
            "- The checks passed on the queued change; the integrator runs them again after \
             its rebase.\n",
        );
        comment.push_str(&self.artifacts(Some(&format!("{commit}~1"))));
        let commented = self
            .work
            .tracker
            .comment(self.repository, self.issue.number, &comment);
        let mut closing = format!(
            "Queued {} to land on {} as entry {id} on branch {branch}",
            &commit[..10],
            self.work.branch
        );
        match commented {
            Ok(()) => closing.push_str(&format!("; #{} closes when it lands.", self.issue.number)),
            Err(why) => closing.push_str(&format!("; {why}.")),
        }
        self.end(closing)
    }

    /// The evidence comment: `headline`, what changed, the checks, and
    /// the run.
    fn evidence(&self, headline: &str, files: &[FileChange]) -> String {
        let mut text = format!("{headline}\n\n");
        if !self.summaries.is_empty() {
            text.push_str("**What Coder did**\n\n");
            for summary in &self.summaries {
                text.push_str(&format!("{}\n\n", clip(summary.trim(), 2_000)));
            }
        }
        if !files.is_empty() {
            text.push_str("**Files**\n\n");
            for file in files {
                text.push_str(&format!(
                    "- {} `{}` (+{} -{})\n",
                    file.status,
                    file.path,
                    file.added.map_or("?".into(), |n| n.to_string()),
                    file.removed.map_or("?".into(), |n| n.to_string())
                ));
            }
            text.push('\n');
        }
        text.push_str("**Checks**\n\n");
        for ran in &self.checked.ran {
            text.push_str(&format!("- {ran}\n"));
        }
        text.push_str("- The checks passed on the exact change that landed.\n\n");
        text.push_str(&self.run_line());
        text
    }

    fn run_line(&self) -> String {
        let record = local::record(&self.work.store, &self.record.task);
        let turns = record.as_ref().map_or(1, |r| r.turns.len());
        let providers: Vec<String> = record
            .iter()
            .flat_map(|r| r.turns.iter())
            .map(|turn| format!("{} {}", turn.provider, turn.model))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        format!(
            "**Run**: task `{}`, {turns} turn(s) ({} fix turn(s)) on {}, from an OpenAgents chat \
             {}.",
            &self.record.task[..12],
            self.rounds,
            if providers.is_empty() {
                "a local provider".to_owned()
            } else {
                providers.join(", ")
            },
            placement().run
        )
    }

    /// What the worktree changed from the branch it started on, as
    /// `git diff --stat` shows it; empty when nothing changed.
    fn diff_stat(&self) -> String {
        let _ = local::git_out(self.worktree, &["add", "-A"]);
        let base = self.started_on();
        local::git_out(self.worktree, &["diff", "--cached", "--stat", &base])
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    /// The commit the run's change starts from: where `HEAD` meets the
    /// branch, or the run's first base.
    fn started_on(&self) -> String {
        let upstream = format!("origin/{}", self.work.branch);
        local::git_out(self.worktree, &["merge-base", "HEAD", &upstream])
            .map(|base| base.trim().to_owned())
            .ok()
            .filter(|base| !base.is_empty())
            .unwrap_or_else(|| self.record.base.clone())
    }

    /// The comment's section linking this run's uploaded artifacts, with
    /// the change diffed from `base` (where the run started when `None`);
    /// empty when uploads are off (#10227).
    fn artifacts(&self, base: Option<&str>) -> String {
        let Some(uploader) = &self.work.artifacts else {
            return String::new();
        };
        let base = base.map_or_else(|| self.started_on(), str::to_owned);
        let record = local::record(&self.work.store, &self.record.task)
            .unwrap_or_else(|| self.record.clone());
        let files = super::run_artifacts::Files::collect(
            &self.work.store,
            &record,
            self.worktree,
            &base,
            &self.checked,
        );
        let prefix = super::run_artifacts::prefix(
            self.repository,
            self.issue.number,
            &self.record.task,
            &self.flow.link.outcome,
        );
        format!(
            "\n\n{}",
            super::run_artifacts::publish(&**uploader, &prefix, &files).trim_end()
        )
    }

    /// How far the run got, for a comment that leaves the issue open.
    fn how_far(&self) -> String {
        let stat = self.diff_stat();
        if stat.trim().is_empty() {
            return "**How far it got**: the worktree has no change.\n\n".to_owned();
        }
        format!(
            "**How far it got** (`git diff --stat` against the branch it started on)\n\n```text\n{}\n```\n\n",
            clip(&stat, STAT_MAX).replace("```", "'''")
        )
    }

    fn unchanged(&mut self) {
        self.flow.link.outcome = "unchanged".into();
        let what = self.summaries.join("\n\n");
        let comment = format!(
            "Coder worked this issue and changed nothing, so nothing landed and the issue stays \
             open.\n\n{}\n\n{}{}\n\n{RELEASE_MARK}",
            if what.trim().is_empty() {
                "It gave no summary.".to_owned()
            } else {
                format!("**What Coder said**\n\n{}", clip(what.trim(), 3_000))
            },
            self.run_line(),
            self.artifacts(None)
        );
        let _ = self
            .work
            .tracker
            .comment(self.repository, self.issue.number, &comment);
        self.end(format!(
            "Coder changed nothing, so nothing landed; #{} stays open with a comment.",
            self.issue.number
        ));
    }

    /// Why the task was asked to stop, as its store recorded it, when the
    /// stop came from a command rather than from the turn's own end.
    fn stop_reason(&self) -> Option<String> {
        super::Store::open(&self.work.store)
            .ok()?
            .show(&self.record.task)
            .ok()?
            .cancellation_reason
            .filter(|reason| !reason.trim().is_empty())
    }

    /// Commit whatever the turn left uncommitted in the worktree and push it
    /// to `coder/stranded-<task8>`, so a stop or failure between turns does
    /// not lose the work with its computer. Whether a branch was pushed.
    fn keep_unfinished(&mut self) -> bool {
        if self.stranded.is_some() {
            return true;
        }
        let dirty = local::git_out(self.worktree, &["status", "--porcelain"])
            .map(|out| !out.trim().is_empty())
            .unwrap_or(false);
        let ahead = local::git_out(
            self.worktree,
            &[
                "rev-list",
                "--count",
                &format!("{}..HEAD", self.record.base),
            ],
        )
        .ok()
        .and_then(|out| out.trim().parse::<u64>().ok())
        .unwrap_or(0)
            > 0;
        if !dirty && !ahead {
            return false;
        }
        if dirty {
            let message = format!(
                "WIP: Coder's unfinished work on #{} (task {})",
                self.issue.number,
                &self.record.task[..8.min(self.record.task.len())]
            );
            if local::git_out(self.worktree, &["add", "-A"]).is_err()
                || local::git_out(
                    self.worktree,
                    &[
                        "-c",
                        "user.name=Coder",
                        "-c",
                        "user.email=coder@openagents.com",
                        "commit",
                        "-q",
                        "--no-verify",
                        "-m",
                        &message,
                    ],
                )
                .is_err()
            {
                self.note("Could not commit the unfinished work before keeping it.");
                return false;
            }
        }
        self.strand()
    }

    fn stopped(&mut self, why: &str) {
        self.flow.link.outcome = "stopped".into();
        let reason = self
            .stop_reason()
            .map(|reason| format!(" The stop request said: {reason}"))
            .unwrap_or_default();
        let kept = if self.keep_unfinished() {
            match &self.stranded {
                Some(branch) => format!(
                    "The unfinished change is kept on the branch [`{branch}`](https://github.com/{}/tree/{branch}).",
                    self.repository
                ),
                None => String::new(),
            }
        } else {
            format!(
                "Any partial change is in Coder's worktree `{}` on the computer that ran it.",
                self.worktree.display()
            )
        };
        let comment = format!(
            "Coder stopped working on this before it landed anything: {why}{reason} The issue stays \
             open. {kept}\n\n{}{}{}\n\n{RELEASE_MARK}",
            self.how_far(),
            self.run_line(),
            self.artifacts(None)
        );
        let _ = self
            .work
            .tracker
            .comment(self.repository, self.issue.number, &comment);
        self.end(format!(
            "Nothing landed; #{} stays open with a comment.",
            self.issue.number
        ));
    }

    fn failed(&mut self, why: &str, problems: Option<&[String]>) {
        self.flow.link.outcome = "failed".into();
        let mut comment = format!("Coder tried this issue and did not land a change. {why}\n\n");
        if !self.summaries.is_empty() {
            comment.push_str("**What Coder tried**\n\n");
            for summary in &self.summaries {
                comment.push_str(&format!("{}\n\n", clip(summary.trim(), 1_500)));
            }
        }
        if let Some(problems) = problems {
            comment.push_str("**What the checks found**\n\n```text\n");
            comment.push_str(&clip(&problems.join("\n\n"), FAILING_MAX).replace("```", "'''"));
            comment.push_str("\n```\n\n");
        }
        comment.push_str(&self.how_far());
        self.keep_unfinished();
        let kept = match &self.stranded {
            Some(branch) => format!(
                "Nothing landed on the default branch, and the issue stays open. The change is \
                 kept on the branch [`{branch}`](https://github.com/{}/tree/{branch}).",
                self.repository
            ),
            None => format!(
                "Nothing was pushed, and the issue stays open. The change is in Coder's \
                 worktree `{}` on the computer that ran it.",
                self.worktree.display()
            ),
        };
        comment.push_str(&format!(
            "{kept}\n\n{}{}\n\n{RELEASE_MARK}",
            self.run_line(),
            self.artifacts(None)
        ));
        let _ = self
            .work
            .tracker
            .comment(self.repository, self.issue.number, &comment);
        let pushed = match &self.stranded {
            Some(branch) => format!("The change is kept on `{branch}`"),
            None => "Nothing was pushed".to_owned(),
        };
        self.end(format!(
            "{why} {pushed}; #{} stays open with a comment.",
            self.issue.number
        ));
    }

    fn end(&mut self, closing: String) {
        // A queued change keeps its claim until the integrator lands it.
        if !matches!(
            self.flow.link.outcome.as_str(),
            "landed" | "pull_request" | "queued"
        ) {
            let release = format!("Coder released its claim; nothing landed. {RELEASE_MARK}");
            for said in crate::claim::release(
                &*self.work.tracker,
                self.repository,
                self.issue.number,
                Some(&release),
                &self.work.policy.project,
            ) {
                if !said.starts_with("Released ") {
                    self.note(said);
                }
            }
        }
        // The run is over either way, so this computer's hold ends with it.
        let _ = coder_lease::claims::release(
            &claims_root(&self.work.store),
            self.repository,
            self.issue.number,
            &coder_lease::scratch::delegate_session(),
            0,
            Duration::ZERO,
            true,
        );
        self.note(closing.clone());
        self.flow.closing = closing;
        self.flow.finished = true;
        let _ = save(&self.work.store, self.flow);
        let _ = std::fs::remove_file(stop_path(&self.work.store, &self.record.task));
        // Landed, or nothing to keep: the worktree goes once the task has
        // ended (#10291). Unsaved work keeps it.
        let _ = super::retire::retire(&self.work.store, &self.record.task);
    }
}

/// The landing's checks are the flow's own: the rebased change goes back
/// to staged (the checks read the staged diff), the checks run, and the
/// change is committed again with its message.
impl landing::Hooks for Run<'_> {
    fn fix_conflict(&mut self, request: &str) -> Result<(), String> {
        let store = self.work.store.clone();
        let task = self.record.task.clone();
        let stopping = || stop_requested(&store, &task);
        if !local::await_checks(&store, &task, &stopping, POLL)? {
            return Err("Stopped before the conflict fix turn.".into());
        }
        self.work.local.answer(&task, request)?;
        self.turn += 1;
        self.rounds += 1;
        match self.wait() {
            Turn::Finished { summary } => {
                self.summaries.push(summary);
                Ok(())
            }
            Turn::Stopped(why) | Turn::Failed(why) => Err(why),
            Turn::Asked(text) => Err(format!("The conflict fix turn asked a question: {text}")),
        }
    }

    fn check(&mut self) -> Vec<String> {
        let held = local::git_out(self.worktree, &["rev-parse", "HEAD"])
            .map(|head| head.trim().to_owned())
            .unwrap_or_default();
        if let Err(why) = local::git_out(self.worktree, &["reset", "-q", "--soft", "HEAD~1"]) {
            return vec![format!("Git could not stage the rebased change: {why}")];
        }
        self.checked = self.work.checks.check(self.worktree, &self.work.policy);
        let _ = local::git_out(self.worktree, &["add", "-A"]);
        if let Err(why) =
            local::git_out(self.worktree, &["commit", "-q", "--no-verify", "-C", &held])
        {
            return vec![format!(
                "Git could not commit the rebased change again: {why}"
            )];
        }
        self.checked.problems.clone()
    }

    fn note(&mut self, text: &str) {
        Run::note(self, text);
    }

    fn stopping(&self) -> bool {
        Run::stopping(self)
    }

    fn enter(&mut self) -> Option<Box<dyn std::any::Any>> {
        const WAITING: &str = "Waiting to land behind another run…";
        let mut said = false;
        let local = match LANDING.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poison)) => poison.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                Run::note(self, WAITING);
                said = true;
                LANDING.lock().unwrap_or_else(|poison| poison.into_inner())
            }
        };
        let shared = match landing_lock(&self.work.store) {
            Some((file, false)) => Some(file),
            Some((file, true)) => {
                if !said {
                    Run::note(self, WAITING);
                }
                file.lock().ok().map(|()| file)
            }
            None => None,
        };
        Some(Box::new(LandingGuard {
            _shared: shared,
            _local: local,
        }))
    }
}

/// What `to` changed since `from` in `worktree`.
fn changed_by(worktree: &Path, from: &str, to: &str) -> Vec<FileChange> {
    let mut out = Vec::new();
    let statuses =
        local::git_out(worktree, &["diff", "--name-status", from, to]).unwrap_or_default();
    let numstat = local::git_out(worktree, &["diff", "--numstat", from, to]).unwrap_or_default();
    for line in numstat.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let status = statuses
            .lines()
            .find(|line| line.ends_with(&format!("\t{path}")))
            .and_then(|line| line.chars().next())
            .map_or("modified", |code| match code {
                'A' => "added",
                'D' => "deleted",
                'R' => "renamed",
                _ => "modified",
            });
        out.push(FileChange {
            path: path.to_owned(),
            status: status.to_owned(),
            added: added.parse().ok(),
            removed: removed.parse().ok(),
            ..FileChange::default()
        });
    }
    coder_events::attach_patches(&mut out, &super::review::patch(worktree, from, Some(to)));
    out
}

fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
#[path = "issue_run_tests.rs"]
mod tests;

/// The variable `chat work --on boat|gce` sets on the machine it runs an
/// issue's flow on, naming that placement for the issue's comments.
pub const PLACEMENT_ENV: &str = "OPENAGENTS_CODER_PLACEMENT";

/// Where the issue's comments say the run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    /// For the claim, said by the machine itself.
    pub claim: &'static str,
    /// For the evidence's run line.
    pub run: &'static str,
}

fn placement() -> Placement {
    placement_named(std::env::var(PLACEMENT_ENV).ok().as_deref())
}

pub(crate) fn placement_named(name: Option<&str>) -> Placement {
    match name {
        Some("boat") => Placement {
            claim: "on a Boat sandbox",
            run: "on a Boat sandbox",
        },
        Some("gce") => Placement {
            claim: "on a GCE pool host",
            run: "on a GCE pool host",
        },
        _ => Placement {
            claim: "on this computer",
            run: "on the owner's computer",
        },
    }
}
