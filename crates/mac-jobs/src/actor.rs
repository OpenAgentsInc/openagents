//! The job as an actor (#11253, step 1): one `mac.job` per run, owned by
//! the account that sent it.
//!
//! The website creates it ([`Submit`], host only), which offers one work
//! item on [`QUEUE`] for the chosen Mac ([`target`]). The Mac claims it (a
//! long-poll claim), then reports through calls fenced by that claim
//! ([`Report`], [`ArtifactPart`]): only the claim's executor at its epoch,
//! before its lease ends, can report, and each report renews the lease. It
//! ends the claim with the work queue's finish (or release, when
//! cancelled), which arrives here as `WorkDone` or `WorkExpired`.
//!
//! **Approvals.** An outward recipe asks; the question is bound to the
//! claim's epoch. Only the host's own pages answer ([`Approve`], [`Deny`]
//! need the host's service authority, which no HTTP caller gets), and the
//! answer is handed to that epoch's claim once. A denial sticks: the job
//! never asks again, and a later answer is refused.
//!
//! **Recovery.** A Mac that stops answering loses its claim when the lease
//! ends (the runtime fences it). A test or build goes back to waiting and
//! the Mac takes it again at a new epoch; anything it reports from the old
//! one is refused. An upload is never repeated on its own: it becomes
//! uncertain until an operator records what happened (`actors-admin
//! resolve-work`).
//!
//! The view has the shape the website's job pages already read
//! (`openagents-web` `mac_jobs::Job`).

use actors::{
    core::{Actor, Ctx, Definition, Handles, Message, Registry},
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Kind, Spec, redact_line, valid_job_id};

/// The actor type.
pub const TYPE: &str = "mac.job";
/// The work queue linked Macs claim from.
pub const QUEUE: &str = "mac-jobs";
/// How long a claim lives without a report. The Mac reports every few
/// seconds while it runs.
pub const LEASE_MS: i64 = 60_000;
/// A job no Mac took in this long is cancelled.
pub const WAIT_MS: i64 = 6 * 3600 * 1000;
/// The view's schema tag (the website's job record).
pub const VIEW_SCHEMA: &str = "openagents.web.mac-jobs.job.v1";

/// Log text kept on the job, in bytes; the whole log is one of its files.
const MAX_LINE_BYTES: usize = 32 * 1024;
/// The most log lines kept.
const MAX_LINES: usize = 400;
/// The most lines one report carries.
const MAX_REPORT_LINES: usize = 500;
/// The largest part of a file, in bytes.
pub const PART_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PARTS: u32 = 64;
const MAX_ARTIFACTS: usize = 64;
const MAX_JOB_BYTES: u64 = 1024 * 1024 * 1024;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The work target, and the executor id, of the Mac named `computer` (any
/// characters; the actor runtime's names are tokens).
#[must_use]
pub fn target(computer: &str) -> String {
    let digest = Sha256::digest(format!("openagents.mac-jobs.computer.v1\0{computer}"));
    format!("mac:{}", &hex(&digest)[..32])
}

/// The job id for the request `key` of `account`: the same request, sent
/// again, names the same job, so a retry never makes a second run.
#[must_use]
pub fn job_id(account: &str, key: &str) -> String {
    let digest = Sha256::digest(format!("openagents.mac-jobs.job.v1\0{account}\0{key}"));
    format!("mjob{}", &hex(&digest)[..32])
}

/// Where a job is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Waiting for its Mac to take it.
    Waiting,
    Running,
    /// Waiting for the owner's answer.
    Asking,
    Done,
    Failed,
    Cancelled,
    /// The Mac stopped answering during work that must not run twice; an
    /// operator records what happened.
    Uncertain,
}

impl Phase {
    #[must_use]
    pub fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// The owner's question, bound to the claim that asked it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub subject: String,
    pub epoch: u64,
}

/// The owner's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub question: String,
    /// `approved` or `denied`.
    pub decision: String,
    /// `web` or `phone`.
    pub via: String,
    pub at_ms: i64,
    /// The claim it was given for; no other claim receives it.
    pub epoch: u64,
    /// The Mac has it (an answer is used once).
    pub taken: bool,
}

/// One file the job made; its bytes live in the host's store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub size: u64,
    pub parts: u32,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub id: String,
    pub computer: String,
    pub spec: Spec,
    pub phase: Phase,
    pub created_ms: i64,
    pub updated_ms: i64,
    #[serde(default)]
    pub finished_ms: Option<i64>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub dropped: usize,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub question: Option<Question>,
    #[serde(default)]
    pub approval: Option<Approval>,
    /// The owner denied this job's upload: it never asks again.
    #[serde(default)]
    pub denied: Option<Approval>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub cancel: bool,
    /// The work item, once offered.
    #[serde(default)]
    pub item: Option<String>,
    /// The newest claim epoch that reported.
    #[serde(default)]
    pub epoch: u64,
    /// How long a claim lives without a report.
    pub lease_ms: i64,
}

impl State {
    fn touch(&mut self, ctx: &Ctx) {
        self.updated_ms = ctx.now();
    }

    fn end(&mut self, ctx: &Ctx, phase: Phase, why: Option<String>) {
        self.phase = phase;
        self.why = why;
        self.finished_ms = Some(ctx.now());
        self.touch(ctx);
    }

    fn say(&mut self, text: &str) {
        let line = redact_line(text);
        if line.trim().is_empty() {
            return;
        }
        self.lines.push(line);
        let mut bytes: usize = self.lines.iter().map(String::len).sum();
        while self.lines.len() > MAX_LINES || bytes > MAX_LINE_BYTES {
            let gone = self.lines.remove(0);
            bytes -= gone.len();
            self.dropped += 1;
        }
    }

    /// The claim's fence, checked against this job's own item.
    fn claim<'c>(&self, ctx: &'c Ctx) -> Result<&'c Fenced> {
        let fence = ctx
            .fence()
            .filter(|fence| self.item.as_deref() == Some(fence.item_id.as_str()))
            .ok_or_else(|| {
                ActorError::new("forbidden", "Only the Mac running this job reports on it.")
            })?;
        if fence.epoch < self.epoch {
            return Err(ActorError::new(
                "stale_claim",
                "This job was taken again since.",
            ));
        }
        Ok(fence)
    }
}

/// What creates a job (the host's submit route).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub id: String,
    pub computer: String,
    pub spec: Spec,
    /// The claim's lease; [`LEASE_MS`] unless set (tests shorten it).
    #[serde(default)]
    pub lease_ms: Option<i64>,
}

pub struct MacJob;

fn line(text: &str, chars: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

impl Actor for MacJob {
    const TYPE: &'static str = TYPE;
    const STATE_VERSION: u32 = 1;
    const PRIVATE: bool = true;
    const CREATE_ACCESS: Access = Access::Service;
    const VIEW_ACCESS: Access = Access::AccountOwner;
    type State = State;
    type Input = Input;

    fn create(input: Input, ctx: &mut Ctx) -> Result<State> {
        input
            .spec
            .check()
            .map_err(|why| ActorError::new("bad_args", why))?;
        if !valid_job_id(&input.id) || ctx.id().key != input.id {
            return Err(ActorError::new("bad_args", "The job id is invalid."));
        }
        let computer = line(&input.computer, 64);
        if computer.is_empty() {
            return Err(ActorError::new("bad_args", "Name the Mac."));
        }
        let lease_ms = input.lease_ms.unwrap_or(LEASE_MS);
        if !(1_000..=600_000).contains(&lease_ms) {
            return Err(ActorError::new("bad_args", "The lease is out of range."));
        }
        Ok(State {
            id: input.id,
            computer,
            spec: input.spec,
            phase: Phase::Waiting,
            created_ms: ctx.now(),
            updated_ms: ctx.now(),
            finished_ms: None,
            lines: Vec::new(),
            dropped: 0,
            commit: None,
            question: None,
            approval: None,
            denied: None,
            artifacts: Vec::new(),
            summary: None,
            why: None,
            cancel: false,
            item: None,
            epoch: 0,
            lease_ms,
        })
    }

    fn wake(_state: &State) -> Result<Self> {
        Ok(Self)
    }

    fn view(state: &State, _caller: &Caller) -> Result<Value> {
        let unix = |ms: i64| u64::try_from(ms / 1000).unwrap_or(0);
        Ok(json!({
            "schema": VIEW_SCHEMA,
            "engine": "actor",
            "id": state.id,
            "computer": state.computer,
            "spec": state.spec,
            "created_unix": unix(state.created_ms),
            "updated_unix": unix(state.updated_ms),
            "finished_unix": state.finished_ms.map(unix),
            "state": state.phase,
            "lines": state.lines,
            "dropped": state.dropped,
            "commit": state.commit,
            "question": state.question.as_ref().map(|q| json!({"id": q.id, "text": q.text, "subject": q.subject})),
            "approval": state.approval.as_ref().or(state.denied.as_ref()).map(|a| json!({
                "question": a.question, "decision": a.decision, "via": a.via, "at_unix": unix(a.at_ms),
            })),
            "approval_taken": state.approval.as_ref().is_some_and(|a| a.taken),
            "artifacts": state.artifacts,
            "summary": state.summary,
            "why": state.why,
            "cancel": state.cancel,
            "item": state.item,
            "epoch": state.epoch,
        }))
    }

    fn description() -> &'static str {
        "A job a linked Mac runs for its account: a recipe at a ref, its log, its files, \
         and the owner's answer when it uploads (#11223)."
    }
}

/// The work item's payload: what the Mac runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offered {
    pub id: String,
    pub spec: Spec,
}

/// Offer the job to its Mac (the host's submit route, with creation).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submit {}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submitted {
    pub id: String,
    pub computer: String,
    pub kind: Kind,
    pub approval: bool,
}
impl Message for Submit {
    const NAME: &'static str = "submit@1";
    const ACCESS: Access = Access::Service;
    type Reply = Submitted;
}
impl Handles<Submit> for MacJob {
    fn handle(&mut self, state: &mut State, _: Submit, ctx: &mut Ctx) -> Result<Submitted> {
        if state.item.is_none() && !state.phase.finished() {
            let outward = state.spec.outward();
            let item = ctx.work(WorkSpec {
                item_id: String::new(),
                queue: QUEUE.into(),
                target: Some(target(&state.computer)),
                payload: json!(Offered {
                    id: state.id.clone(),
                    spec: state.spec.clone()
                }),
                lease_ms: state.lease_ms,
                max_attempts: if outward { 1 } else { 3 },
                retry: if outward {
                    RetryPolicy::Reconcile
                } else {
                    RetryPolicy::Idempotent
                },
            })?;
            state.item = Some(item);
            ctx.schedule(AlarmSpec {
                name: "wait".into(),
                due_at: ctx.now().saturating_add(WAIT_MS),
                message: Envelope {
                    name: ExpireWait::NAME.into(),
                    args: json!({}),
                    origin: Origin::Inbox,
                },
                interval_ms: None,
            })?;
            state.touch(ctx);
        }
        Ok(Submitted {
            id: state.id.clone(),
            computer: state.computer.clone(),
            kind: state.spec.kind(),
            approval: state.spec.outward(),
        })
    }
}

/// The owner's question, as the Mac asks it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    pub id: String,
    pub text: String,
    pub subject: String,
}

/// The Mac's report: log lines, the commit, the owner's question. Fenced
/// by its claim, and renews it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub ask: Option<Ask>,
}
/// The owner's answer, as the Mac receives it once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub question: String,
    pub decision: String,
    pub via: String,
    pub at_unix: u64,
}
/// What a report is answered with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heard {
    pub cancel: bool,
    #[serde(default)]
    pub approval: Option<Answer>,
}
impl Message for Report {
    const NAME: &'static str = "report@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = Heard;
}

fn answer_of(approval: &Approval) -> Answer {
    Answer {
        question: approval.question.clone(),
        decision: approval.decision.clone(),
        via: approval.via.clone(),
        at_unix: u64::try_from(approval.at_ms / 1000).unwrap_or(0),
    }
}

fn commit_id(commit: &str) -> bool {
    (7..=64).contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Handles<Report> for MacJob {
    fn handle(&mut self, state: &mut State, sent: Report, ctx: &mut Ctx) -> Result<Heard> {
        let fence = state.claim(ctx)?.clone();
        if state.phase.finished() {
            return Ok(Heard {
                cancel: true,
                approval: None,
            });
        }
        // What readers see before, so a report with nothing new (the Mac's
        // ping) leaves the job's time alone.
        let before = serde_json::to_string(&(
            &state.phase,
            &state.lines,
            &state.commit,
            &state.question,
            &state.approval,
        ))
        .unwrap_or_default();
        state.epoch = fence.epoch;
        if matches!(state.phase, Phase::Waiting | Phase::Uncertain) {
            state.phase = Phase::Running;
            state.why = None;
            ctx.cancel_alarm("wait")?;
        }
        for text in sent.lines.iter().take(MAX_REPORT_LINES) {
            state.say(text);
        }
        if state.commit.is_none()
            && let Some(commit) = sent.commit.filter(|c| commit_id(c))
        {
            state.commit = Some(commit);
        }
        let mut heard = Heard {
            cancel: fence.cancel || state.cancel,
            approval: None,
        };
        if let Some(ask) = sent.ask.filter(|ask| !line(&ask.id, 64).is_empty()) {
            let id = line(&ask.id, 64);
            if let Some(denied) = &state.denied {
                // A denial sticks: every later question is answered with it.
                heard.approval = Some(Answer {
                    question: id,
                    ..answer_of(denied)
                });
            } else if !state
                .question
                .as_ref()
                .is_some_and(|q| q.id == id && q.epoch == fence.epoch)
            {
                state.question = Some(Question {
                    id,
                    text: ask.text.chars().take(2_000).collect(),
                    subject: line(&ask.subject, 500),
                    epoch: fence.epoch,
                });
                state.approval = None;
                state.phase = Phase::Asking;
            }
        }
        if let (Some(question), Some(approval)) = (&state.question, &mut state.approval)
            && !approval.taken
            && approval.epoch == fence.epoch
            && approval.question == question.id
        {
            approval.taken = true;
            heard.approval = Some(answer_of(approval));
            if state.phase == Phase::Asking {
                state.phase = Phase::Running;
            }
        }
        let after = serde_json::to_string(&(
            &state.phase,
            &state.lines,
            &state.commit,
            &state.question,
            &state.approval,
        ))
        .unwrap_or_default();
        if after != before {
            state.touch(ctx);
        }
        Ok(heard)
    }
}

/// One part of a file the job made, recorded after the host stored its
/// bytes. Parts come in order; the same part again replaces it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPart {
    pub name: String,
    pub part: u32,
    pub size: u64,
    pub last: bool,
}
impl Message for ArtifactPart {
    const NAME: &'static str = "artifact@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = bool;
}

/// A file name a job may keep: letters, digits, `.`, `_`, `-`, not starting
/// with `.` or `-`, at most 96 characters.
#[must_use]
pub fn valid_artifact(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 96
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Whether part `part` of `name`, `size` bytes, may be kept for `state`;
/// the refusal in plain words.
///
/// # Errors
/// The part is out of order, too large, or past the job's limits.
pub fn part_fits(
    state: &State,
    name: &str,
    part: u32,
    size: u64,
) -> std::result::Result<(), &'static str> {
    if !valid_artifact(name) {
        return Err("Name the file with letters, digits, . _ -.");
    }
    if size > PART_BYTES {
        return Err("A part is at most 8 MiB.");
    }
    if part >= MAX_PARTS {
        return Err("A file is at most 512 MiB.");
    }
    let fits = match state.artifacts.iter().find(|a| a.name == name) {
        Some(found) => !found.done && (part == found.parts || part + 1 == found.parts),
        None => part == 0 && state.artifacts.len() < MAX_ARTIFACTS,
    };
    if !fits {
        return Err("Send a file's parts in order.");
    }
    let total: u64 = state.artifacts.iter().map(|a| a.size).sum();
    if total + size > MAX_JOB_BYTES {
        return Err("This job's files are over 1 GiB.");
    }
    Ok(())
}

impl Handles<ArtifactPart> for MacJob {
    fn handle(&mut self, state: &mut State, sent: ArtifactPart, ctx: &mut Ctx) -> Result<bool> {
        let fence = state.claim(ctx)?.clone();
        if state.phase.finished() || fence.cancel || state.cancel {
            return Err(ActorError::new("cancelled", "This job was stopped."));
        }
        state.epoch = fence.epoch;
        part_fits(state, &sent.name, sent.part, sent.size)
            .map_err(|why| ActorError::new("bad_args", why))?;
        match state.artifacts.iter_mut().find(|a| a.name == sent.name) {
            Some(found) if sent.part == found.parts => {
                found.parts += 1;
                found.size += sent.size;
                found.done = sent.last;
            }
            Some(found) => found.done = sent.last,
            None => state.artifacts.push(Artifact {
                name: sent.name,
                size: sent.size,
                parts: 1,
                done: sent.last,
            }),
        }
        state.touch(ctx);
        Ok(true)
    }
}

/// The owner's answer to the job's question, from the host's own pages
/// (the web form or the phone's board): service authority only.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answering {
    pub question: String,
    /// `web` or `phone`.
    pub via: String,
}
/// What became of an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answered {
    Recorded,
    /// The job isn't waiting on that question (answered, denied, or gone).
    NotAsking,
}

/// Approve the job's question.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Approve(pub Answering);
impl Message for Approve {
    const NAME: &'static str = "approve@1";
    const ACCESS: Access = Access::Service;
    type Reply = Answered;
}
/// Deny the job's question; the denial sticks.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Deny(pub Answering);
impl Message for Deny {
    const NAME: &'static str = "deny@1";
    const ACCESS: Access = Access::Service;
    type Reply = Answered;
}

fn answer(state: &mut State, sent: &Answering, approve: bool, ctx: &Ctx) -> Answered {
    let Some(question) = state.question.as_ref().filter(|q| {
        state.phase == Phase::Asking
            && !state.cancel
            && state.approval.is_none()
            && state.denied.is_none()
            && q.id == sent.question
    }) else {
        return Answered::NotAsking;
    };
    let approval = Approval {
        question: question.id.clone(),
        decision: if approve { "approved" } else { "denied" }.into(),
        via: if sent.via == "phone" { "phone" } else { "web" }.into(),
        at_ms: ctx.now(),
        epoch: question.epoch,
        taken: false,
    };
    if !approve {
        state.denied = Some(Approval {
            taken: true,
            ..approval.clone()
        });
    }
    state.approval = Some(approval);
    state.touch(ctx);
    Answered::Recorded
}

impl Handles<Approve> for MacJob {
    fn handle(&mut self, state: &mut State, sent: Approve, ctx: &mut Ctx) -> Result<Answered> {
        Ok(answer(state, &sent.0, true, ctx))
    }
}
impl Handles<Deny> for MacJob {
    fn handle(&mut self, state: &mut State, sent: Deny, ctx: &mut Ctx) -> Result<Answered> {
        Ok(answer(state, &sent.0, false, ctx))
    }
}

/// Stop the job: a waiting one at once, a running one when its Mac learns
/// of it at its next report and releases the claim.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {}
impl Message for Cancel {
    const NAME: &'static str = "cancel@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = bool;
}
impl Handles<Cancel> for MacJob {
    fn handle(&mut self, state: &mut State, _: Cancel, ctx: &mut Ctx) -> Result<bool> {
        if state.phase.finished() || state.cancel {
            return Ok(false);
        }
        state.cancel = true;
        if let Some(item) = state.item.clone() {
            ctx.cancel_work(item)?;
        }
        ctx.cancel_alarm("wait")?;
        if state.phase == Phase::Waiting {
            state.end(
                ctx,
                Phase::Cancelled,
                Some("The job was cancelled before it started.".into()),
            );
        } else {
            state.touch(ctx);
        }
        Ok(true)
    }
}

/// The waiting time ran out (the job's own alarm).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpireWait {}
impl Message for ExpireWait {
    const NAME: &'static str = "expire_wait@1";
    const ACCESS: Access = Access::Service;
    type Reply = ();
}
impl Handles<ExpireWait> for MacJob {
    fn handle(&mut self, state: &mut State, _: ExpireWait, ctx: &mut Ctx) -> Result<()> {
        if state.phase == Phase::Waiting && !state.cancel {
            state.cancel = true;
            if let Some(item) = state.item.clone() {
                ctx.cancel_work(item)?;
            }
            state.end(
                ctx,
                Phase::Cancelled,
                Some("No Mac took this job in time.".into()),
            );
        }
        Ok(())
    }
}

/// The work queue's completion (the Mac's finish, or an operator's record).
#[derive(Deserialize)]
pub struct WorkDone {
    pub item_id: String,
    pub epoch: u64,
    pub outcome: Value,
    #[serde(default)]
    pub reconciled: bool,
}
impl Message for WorkDone {
    const NAME: &'static str = "WorkDone";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}

/// How a run ended, as the Mac finishes its claim.
#[must_use]
pub fn outcome_done(summary: &str) -> Value {
    json!({"done": {"summary": summary}})
}
#[must_use]
pub fn outcome_failed(why: &str) -> Value {
    json!({"failed": {"why": why}})
}

impl Handles<WorkDone> for MacJob {
    fn handle(&mut self, state: &mut State, sent: WorkDone, ctx: &mut Ctx) -> Result<()> {
        if state.item.as_deref() != Some(sent.item_id.as_str()) || state.phase.finished() {
            return Ok(());
        }
        state.epoch = state.epoch.max(sent.epoch);
        let by = if sent.reconciled {
            "Recorded by an operator: "
        } else {
            ""
        };
        if let Some(summary) = sent.outcome["done"]["summary"].as_str() {
            state.summary = Some(format!("{by}{}", line(summary, 500)));
            state.end(ctx, Phase::Done, None);
        } else if let Some(why) = sent.outcome["failed"]["why"].as_str() {
            let why = line(why, 500);
            let why = if why.is_empty() {
                "The job stopped.".to_owned()
            } else {
                why
            };
            state.end(ctx, Phase::Failed, Some(format!("{by}{why}")));
        } else if state.cancel {
            state.end(ctx, Phase::Cancelled, Some("The job was cancelled.".into()));
        } else {
            state.end(
                ctx,
                Phase::Failed,
                Some(format!("{by}the job ended without a result.")),
            );
        }
        Ok(())
    }
}

/// The work queue fenced the claim: its lease ended, or the Mac released
/// it after a cancel.
#[derive(Deserialize)]
pub struct WorkExpired {
    pub item_id: String,
    pub epoch: u64,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub uncertain: bool,
}
impl Message for WorkExpired {
    const NAME: &'static str = "WorkExpired";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<WorkExpired> for MacJob {
    fn handle(&mut self, state: &mut State, sent: WorkExpired, ctx: &mut Ctx) -> Result<()> {
        if state.item.as_deref() != Some(sent.item_id.as_str()) || state.phase.finished() {
            return Ok(());
        }
        // A newer claim has already reported: this is the old one's end.
        if sent.epoch < state.epoch {
            return Ok(());
        }
        // The fenced epoch's answer is void; the next claim asks again.
        state.question = None;
        state.approval = None;
        if sent.cancelled || state.cancel || sent.state.as_deref() == Some("cancelled") {
            state.end(ctx, Phase::Cancelled, Some("The job was cancelled.".into()));
        } else if sent.state.as_deref() == Some("pending") {
            state.phase = Phase::Waiting;
            state.say("The Mac stopped answering, so the job waits for it to take it again.");
            state.touch(ctx);
        } else if sent.uncertain || sent.state.as_deref() == Some("uncertain") {
            state.phase = Phase::Uncertain;
            state.why = Some(
                "The Mac stopped answering during this job. Whether its upload went out isn't \
                 known, so it won't run again on its own; an operator checks and records it."
                    .into(),
            );
            state.touch(ctx);
        } else {
            state.end(
                ctx,
                Phase::Failed,
                Some("The Mac stopped answering.".into()),
            );
        }
        Ok(())
    }
}

/// Register `mac.job` and its messages.
///
/// # Errors
/// The type is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        Definition::<MacJob>::new()
            .message::<Submit>()
            .message::<Report>()
            .message::<ArtifactPart>()
            .message::<Approve>()
            .message::<Deny>()
            .message::<Cancel>()
            .message::<ExpireWait>()
            .message::<WorkDone>()
            .message::<WorkExpired>(),
    )
}

/// A registry with `mac.job` alone.
///
/// # Errors
/// Never in practice; registration is checked.
pub fn registry() -> Result<Registry> {
    let mut registry = Registry::new();
    register(&mut registry)?;
    Ok(registry)
}
