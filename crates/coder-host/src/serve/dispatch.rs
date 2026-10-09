//! Effects of admitted NIP-HOST operations.
//!
//! `coder-access` has already checked the grant, the epoch, and the one right
//! each operation requires, and has recorded the admitted request before it
//! calls here. The request ID is the idempotency key for every effect.

use std::sync::Arc;

use coder_access::Code;
use coder_access::host::Dispatch;
use coder_access::protocol::{
    CommandAction, Operation, QueueEdit, Receipt, TaskCommand, TaskQueue,
};
use coder_access::studio::{MergeDecision, Merged, Snapshot, Stream, Update, Verdict};
use coder_pty::wire::{Launch, Open, Reason, Size, Value};
use nostr::activity_summary::Phase;

use super::Shared;
use crate::tasks::TaskRef;

/// Connects task operations to the task owner and `terminal.open` to the
/// terminal host. Changed tasks are collected so their summaries can be
/// published after the reply is committed.
pub(crate) struct Dispatcher {
    shared: Arc<Shared>,
    standing: Option<(String, Vec<coder_access::host::OperatePrincipal>)>,
    pub(crate) changed: Vec<TaskRef>,
    /// Agent spend requests, beside the access store.
    spends: crate::spend::Book,
    /// Asks for the owner's wallet, beside the access store.
    links: crate::wallet_link::Book,
    /// The last studio refusal and the sentence the coordinator gave for
    /// it, for a transport that carries one (the control socket).
    pub(crate) refusal: Option<Refusal>,
}

/// A refusal's code and, when the task owner or the host gave one, the
/// plain sentence that says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub(crate) code: Code,
    pub(crate) reason: Option<String>,
}

impl Refusal {
    fn because(code: Code, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: Some(reason.into()),
        }
    }
}

/// A task owner's refusal, with the sentence it noted on this thread.
impl From<Code> for Refusal {
    fn from(code: Code) -> Self {
        Self {
            code,
            reason: crate::tasks::take_reason(code),
        }
    }
}

impl Dispatcher {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        let spends = crate::spend::Book::open(&shared.config.access);
        let links = crate::wallet_link::Book::open(&shared.config.access);
        Self {
            shared,
            standing: None,
            changed: Vec::new(),
            spends,
            links,
            refusal: None,
        }
    }

    /// Use admission already checked under the access lock during synchronous dispatch.
    fn current_standing(&self) -> impl Fn(&crate::tasks::Principal) -> bool + Sync + use<> {
        let snapshot = self.standing.clone();
        let authority = self.shared.authority.clone();
        move |principal| match &snapshot {
            Some((owner, admitted)) => match (&principal.grant, principal.epoch) {
                (None, None) => principal.device == *owner,
                (Some(grant), Some(epoch)) => coder_access::unix_time().is_ok_and(|now| {
                    admitted.iter().any(|current| {
                        current.device == principal.device
                            && current.grant == *grant
                            && current.epoch == epoch
                            && current.expires_at > now
                    })
                }),
                _ => false,
            },
            None => super::standing(&authority, principal),
        }
    }

    /// Keep `result`'s refusal, with its sentence, and answer its code.
    fn noted<T>(&mut self, result: Result<T, Refusal>) -> Result<T, Code> {
        result.map_err(|refusal| {
            let code = refusal.code;
            self.refusal = Some(refusal);
            code
        })
    }

    fn task(&mut self, op: &Operation, result: Result<TaskRef, Code>) -> Result<Receipt, Code> {
        let task = result?;
        let receipt = Receipt {
            operation: op.name().into(),
            reference: task.task.clone(),
        };
        self.changed.push(task);
        Ok(receipt)
    }
}

/// The sender of a device operation, admitted under `grant` (`None` for the
/// owner).
fn principal(device: &str, grant: Option<(&str, u64)>) -> crate::tasks::Principal {
    crate::tasks::Principal {
        device: device.into(),
        grant: grant.map(|(id, _)| id.to_owned()),
        epoch: grant.map(|(_, epoch)| epoch),
    }
}

/// A new Agent Studio stream, named from this process's start time and
/// identity, so a client's sequence from an earlier process reads as stale.
pub(crate) fn studio_stream() -> Stream {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    Stream::new(format!("{nanos:x}{:08x}", std::process::id()))
}

/// Merge, request changes to, or reject a studio task's change at the
/// revisions a device reviewed. The task owner reads the review again
/// first: a worktree that moved refuses as `stale`, unless a merge of
/// exactly these revisions already published, which a retry answers
/// again. **Merge** and **Request changes** need a task whose turn ended
/// with its change waiting for review, or done; any other task refuses as
/// `conflict`, with a sentence that says where it is. **Merge** goes to
/// the landing path (`task.publish`), which for a studio task merges into
/// its checkout's branch and pushes nothing, **Request changes** is the
/// task's next turn through the durable command journal under the
/// device's command ID, and **Reject** is the owner's record. Returns the
/// record and the task a follow-up changed.
pub(crate) fn studio_merge(
    tasks: &dyn crate::tasks::Tasks,
    principal: &crate::tasks::Principal,
    decision: &MergeDecision,
    standing: crate::tasks::Standing<'_>,
) -> Result<(Merged, Option<TaskRef>), Refusal> {
    // Forget a sentence an earlier call left on this thread.
    let _ = crate::tasks::take_reason(Code::Unavailable);
    let review = tasks.review(&decision.task)?;
    let same = |base: &str, head_commit: &str, head: &str| {
        base == decision.base && head_commit == decision.head_commit && head == decision.head
    };
    let published = decision.verdict == Verdict::Merge
        && review
            .publication
            .as_ref()
            .is_some_and(|p| same(&p.base, &p.head_commit, &p.head));
    if !published && !same(&review.base, &review.head_commit, &review.head) {
        return Err(Refusal::because(
            Code::Stale,
            "The task's worktree changed since this review. Read the review again before you \
             decide.",
        ));
    }
    let current = tasks
        .current()
        .into_iter()
        .find(|task| task.task == decision.task);
    if !published
        && decision.verdict != Verdict::Reject
        && let Some(why) = not_ready(current.as_ref().map(|task| task.phase))
    {
        return Err(Refusal::because(Code::Conflict, why));
    }
    let reviewed = crate::tasks::Reviewed {
        base: decision.base.clone(),
        head_commit: decision.head_commit.clone(),
        head: decision.head.clone(),
    };
    let mut merged = Merged {
        task: decision.task.clone(),
        base: decision.base.clone(),
        head_commit: decision.head_commit.clone(),
        head: decision.head.clone(),
        verdict: decision.verdict,
        publication: None,
    };
    let mut changed = None;
    match decision.verdict {
        Verdict::Merge => {
            merged.publication = Some(tasks.publish(principal, &decision.task, &reviewed)?);
        }
        Verdict::RequestChanges => {
            let command = TaskCommand {
                command: decision.command.clone(),
                task: decision.task.clone(),
                action: CommandAction::Send,
                based_on: current.map_or(0, |task| task.revision),
                text: decision.text.clone(),
                emulate: false,
                issued_at: decision.issued_at,
            };
            changed = Some(tasks.command(principal, &command, standing)?);
        }
        Verdict::Reject => {
            tasks.studio_reject(principal, &decision.task, &reviewed, &decision.text)?;
        }
    }
    Ok((merged, changed))
}

/// Why a task in `phase` cannot be merged or sent back for changes, as a
/// sentence; `None` when its turn ended and its change waits for review,
/// or it is done. A task the owner does not list is left to the owner.
fn not_ready(phase: Option<Phase>) -> Option<String> {
    let place = match phase? {
        Phase::Completed => return None,
        Phase::Queued => "is queued and has not run yet",
        Phase::Running => "is still running",
        Phase::Waiting => "is waiting for an answer to its question or approval",
        Phase::Failed => "failed",
        Phase::Cancelled => "was cancelled",
        Phase::Unknown => "has not finished",
    };
    Some(format!(
        "This task {place}, so it has no finished change to merge or send back. Merge and \
         Request changes need a task whose change is waiting for review or done."
    ))
}

/// A single-use `coder-pair:` invitation to this host's read-only Coder
/// chats, naming the host's primary relay, and when its chat grant ends.
/// Every pairing path reaches the same invitation: the iroh enroll reply
/// carries one, and `chats.invite` issues one to a device holding
/// `observe`, however it paired. `unavailable` when the host serves no
/// chats.
pub(crate) fn chat_invitation(config: &crate::config::Config) -> Result<(String, u64), Code> {
    let chats = config.chats.clone().ok_or(Code::Unavailable)?;
    let relay = config.primary().map_err(|_| Code::Unavailable)?.to_owned();
    let now = crate::unix_time().map_err(|_| Code::Unavailable)?;
    let expires_at = now.saturating_add(crate::tailnet::CHAT_GRANT_SECS);
    coder_connect::host::Host::new(&chats.observer, config.policy)
        .invite(&relay, chats.sources, now, expires_at)
        .map(|invitation| (invitation, expires_at))
        .map_err(|_| Code::Unavailable)
}

impl Dispatch for Dispatcher {
    fn cloud(
        &mut self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        op: &Operation,
    ) -> Result<coder_access::Outcome, Code> {
        let workspace = match op {
            Operation::CloudProjects { workspace } => workspace,
            Operation::CloudCatalog { query } => &query.workspace,
            Operation::CloudList { query } => &query.workspace,
            Operation::CloudRead { query } => &query.workspace,
            Operation::CloudOriginal { query } => &query.scope.workspace,
            Operation::CloudSubmit { intent } => &intent.workspace,
            Operation::CloudContinue { intent } => &intent.scope.workspace,
            Operation::CloudCancel { intent } => &intent.scope.workspace,
            Operation::CloudFollow { intent } => &intent.scope.workspace,
            Operation::CloudRelease { intent } => &intent.workspace,
            Operation::EnvironmentRead { query } => &query.workspace,
            Operation::EnvironmentEvidence { query } => &query.workspace,
            Operation::EnvironmentPromote { intent } => &intent.workspace,
            Operation::EnvironmentSelect { intent } => &intent.workspace,
            Operation::EnvironmentSteer { intent } => &intent.workspace,
            _ => return Err(Code::Unsupported),
        };
        if !self.shared.config.workspaces.contains_key(workspace) {
            return Err(Code::Forbidden);
        }
        self.shared
            .tasks
            .cloud(request, &principal(device, grant), op)
    }
    fn cloud_admit_recovery(
        &mut self,
        device: &str,
        admission: &coder_access::cloud::Admission,
    ) -> Result<(), Code> {
        if !self
            .shared
            .config
            .workspaces
            .contains_key(&admission.workspace)
        {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.cloud_admit_recovery(device, admission)
    }
    fn project_list(
        &mut self,
        device: &str,
        workspace: &str,
    ) -> Result<coder_access::project::List, Code> {
        if !self.shared.config.workspaces.contains_key(workspace) {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.project_list(device, workspace)
    }
    fn project_read(
        &mut self,
        device: &str,
        query: &coder_access::project::Query,
    ) -> Result<coder_access::project::Page, Code> {
        if !self.shared.config.workspaces.contains_key(&query.workspace) {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.project_read(device, query)
    }
    fn project_original(
        &mut self,
        device: &str,
        query: &coder_access::project::OriginalQuery,
    ) -> Result<coder_access::project::Chunk, Code> {
        if !self.shared.config.workspaces.contains_key(&query.workspace) {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.project_original(device, query)
    }
    fn operate_snapshot(
        &mut self,
        owner: &str,
        principals: Vec<coder_access::host::OperatePrincipal>,
    ) {
        self.standing = Some((owner.to_owned(), principals));
    }

    fn task_list(
        &mut self,
        _device: &str,
        query: &coder_access::task_read::ListQuery,
    ) -> Result<coder_access::task_read::List, Code> {
        if !self.shared.config.workspaces.contains_key(&query.workspace) {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.task_list(query)
    }
    fn task_read(
        &mut self,
        _device: &str,
        query: &coder_access::task_read::PageQuery,
    ) -> Result<coder_access::task_read::Page, Code> {
        if !self.shared.config.workspaces.contains_key(&query.workspace) {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.task_read(query)
    }
    fn task_original(
        &mut self,
        _device: &str,
        query: &coder_access::task_read::OriginalQuery,
    ) -> Result<coder_access::task_read::OriginalChunk, Code> {
        if !self
            .shared
            .config
            .workspaces
            .contains_key(&query.scope.workspace)
        {
            return Err(Code::Forbidden);
        }
        self.shared.tasks.task_original(query)
    }
    /// A chat invitation for a device the access layer admitted with
    /// `observe`.
    fn chats(&mut self, _device: &str, _now: u64) -> Result<(String, u64), Code> {
        chat_invitation(&self.shared.config)
    }

    /// The host's chat threads, for a device holding `observe`.
    fn threads(&mut self, _device: &str) -> Result<Vec<coder_access::thread::ThreadRow>, Code> {
        super::threads::list(&self.shared)
    }

    /// One page of a host thread, for a device holding `observe`.
    fn thread(
        &mut self,
        _device: &str,
        thread: &str,
        before: Option<u64>,
    ) -> Result<coder_access::thread::ThreadPage, Code> {
        super::threads::read(&self.shared, thread, before)
    }

    /// The book of agent spend requests the phone answers.
    fn spends(&mut self) -> Option<&mut dyn coder_access::host::Spends> {
        Some(&mut self.spends)
    }

    /// The book of asks for the owner's wallet the phone answers.
    fn links(&mut self) -> Option<&mut dyn coder_access::host::Links> {
        Some(&mut self.links)
    }

    /// The owner's private Verse placements, from Verse's home on this
    /// computer, for a paired phone that named its Verse world key.
    #[cfg(feature = "verse-assets")]
    fn verse_private(
        &mut self,
        device: &str,
        world_key: &str,
        now: u64,
    ) -> Result<Option<String>, Code> {
        let home = self
            .shared
            .config
            .verse_home
            .as_deref()
            .ok_or(Code::Unsupported)?;
        crate::verse_private::answer(home, device, world_key, now)
    }

    /// One image chunk, kept by the task owner for this device only.
    fn put_artifact(
        &mut self,
        device: &str,
        put: &coder_access::media::ArtifactPut,
    ) -> Result<coder_access::media::ArtifactState, Code> {
        self.shared.tasks.clone().put_artifact(device, put)
    }

    /// A `background.*` operation, answered by this host's background
    /// runner.
    fn background(
        &mut self,
        _device: &str,
        op: &coder_access::protocol::Operation,
    ) -> Result<serde_json::Value, Code> {
        #[cfg(unix)]
        {
            crate::background::answer(op)
        }
        #[cfg(not(unix))]
        {
            let _ = op;
            Err(Code::Unsupported)
        }
    }

    /// The task owner's workshop agents answer `studio.agent.*`.
    fn agent(
        &mut self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        op: &coder_access::protocol::Operation,
    ) -> Result<serde_json::Value, Code> {
        // Only the owner talks to the workshop agent: the owner's own key,
        // or a device the owner granted. The access layer has checked the
        // right the operation needs.
        if !agent_admits(&self.shared.owner, device, grant) {
            return self.noted(Err(Refusal::because(
                Code::Forbidden,
                "The workshop agent answers only her owner.",
            )));
        }
        let principal = principal(device, grant);
        let _ = crate::tasks::take_reason(Code::Unavailable);
        if op.owner_agent() {
            // Making, retiring, and rotating her are the owner's own acts:
            // her key is attested with the owner key, so a granted device
            // may not ask for them.
            if !agent_setup_admits(&self.shared.owner, device, grant) {
                return self.noted(Err(Refusal::because(
                    Code::Forbidden,
                    "Only the owner's own key changes crew identity, charters, verdicts, or cohort controls.",
                )));
            }
            // Load only: an existing host never mints another owner.
            let owner = crate::control::owner_key(&self.shared).ok().flatten();
            let result = self
                .shared
                .tasks
                .new_agent(request, &principal, op, owner.as_ref());
            return self.noted(result.map_err(Refusal::from));
        }
        let result = self.shared.tasks.agent(request, &principal, op);
        self.noted(result.map_err(Refusal::from))
    }

    /// The configured workspace labels. The roots they name stay on the host.
    fn workspaces(&mut self) -> Result<Vec<String>, Code> {
        Ok(self.shared.config.workspaces.keys().cloned().collect())
    }

    /// The task owner lists or edits the queue. An edit that lets a held
    /// message run changes the task, whose summary follows the reply.
    fn queue(
        &mut self,
        device: &str,
        grant: Option<(&str, u64)>,
        task: &str,
        edit: &QueueEdit,
    ) -> Result<TaskQueue, Code> {
        let principal = principal(device, grant);
        let standing = self.current_standing();
        let (queue, changed) = self
            .shared
            .tasks
            .clone()
            .queue(&principal, task, edit, &standing)?;
        self.changed.extend(changed);
        Ok(queue)
    }

    fn queue_at_revision(
        &mut self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        task: &str,
        revision: u64,
        edit: &QueueEdit,
        queue_digest: Option<&str>,
    ) -> Result<(TaskQueue, String), Code> {
        let principal = principal(device, grant);
        let standing = self.current_standing();
        let (queue, digest, changed) = self.shared.tasks.clone().queue_at_revision(
            request,
            &principal,
            task,
            revision,
            edit,
            queue_digest,
            &standing,
        )?;
        self.changed.extend(changed);
        Ok((queue, digest))
    }

    /// The studio now, from the task owner's coordinator, at the next
    /// point of this process's stream.
    fn studio_snapshot(&mut self, _device: &str) -> Result<Snapshot, Code> {
        let view = self.shared.tasks.studio()?;
        Ok(self
            .shared
            .studio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot(view))
    }

    /// What changed in the studio since `since`; `stale` when this
    /// process's stream does not hold that point.
    fn studio_update(&mut self, _device: &str, stream: &str, since: u64) -> Result<Update, Code> {
        let view = self.shared.tasks.studio()?;
        self.shared
            .studio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .update(view, stream, since)
    }

    /// A merge decision for a device holding `review`.
    fn studio_merge(
        &mut self,
        _request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        decision: &MergeDecision,
    ) -> Result<Merged, Code> {
        let principal = principal(device, grant);
        let standing = self.current_standing();
        let result = studio_merge(self.shared.tasks.as_ref(), &principal, decision, &standing);
        let (merged, changed) = self.noted(result)?;
        self.changed.extend(changed);
        Ok(merged)
    }

    /// What a task changed, for a device holding `observe`.
    fn review(
        &mut self,
        _device: &str,
        task: &str,
    ) -> Result<coder_access::review::TaskReview, Code> {
        self.shared.tasks.review(task)
    }

    /// Publish a reviewed change for a device holding `operate`. The task
    /// owner keys it by its review identity, so the request ID's retry and
    /// a new request for the same review are the same operation.
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        _request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        task: &str,
        base: &str,
        head_commit: &str,
        head: &str,
    ) -> Result<coder_access::review::Publication, Code> {
        let principal = principal(device, grant);
        let reviewed = crate::tasks::Reviewed {
            base: base.into(),
            head_commit: head_commit.into(),
            head: head.into(),
        };
        self.shared.tasks.publish(&principal, task, &reviewed)
    }

    fn dispatch_as(
        &mut self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        op: &Operation,
    ) -> Result<Receipt, Code> {
        match op {
            Operation::CommandTask { command } => {
                let principal = principal(device, grant);
                let standing = self.current_standing();
                let result = self
                    .shared
                    .tasks
                    .clone()
                    .command(&principal, command, &standing);
                self.task(op, result)
            }
            Operation::CommandTaskAtRevision { command, revision } => {
                let principal = principal(device, grant);
                let standing = self.current_standing();
                let result = self
                    .shared
                    .tasks
                    .clone()
                    .command_at_revision(&principal, command, *revision, &standing);
                self.task(op, result)
            }
            // A studio intent goes to the task owner's coordinator; the
            // studio's next update shows what it changed.
            op if op.studio_intent() => {
                let principal = principal(device, grant);
                let standing = self.current_standing();
                // Forget a sentence an earlier call left on this thread.
                let _ = crate::tasks::take_reason(Code::Unavailable);
                let result = self
                    .shared
                    .tasks
                    .clone()
                    .studio_intent(request, &principal, op, &standing)
                    .map_err(Refusal::from);
                let reference = self.noted(result)?;
                Ok(Receipt {
                    operation: op.name().into(),
                    reference,
                })
            }
            _ => self.dispatch(request, device, op),
        }
    }

    fn dispatch(&mut self, request: &str, device: &str, op: &Operation) -> Result<Receipt, Code> {
        let tasks = self.shared.tasks.clone();
        match op {
            Operation::CreateTask { task } => {
                let result = tasks.create(request, device, task);
                self.task(op, result)
            }
            Operation::SteerTask {
                task,
                revision,
                prompt,
            } => {
                let result = tasks.steer(request, device, task, *revision, prompt);
                self.task(op, result)
            }
            Operation::CancelTask {
                task,
                revision,
                reason,
            } => {
                let result = tasks.cancel(request, device, task, *revision, reason);
                self.task(op, result)
            }
            // A follow-up appended to a host thread; its send ID keeps a
            // retry from appending it twice.
            Operation::SendThread {
                thread,
                request: send,
                text,
            } => {
                super::threads::send(&self.shared, thread, send, text)?;
                Ok(Receipt {
                    operation: op.name().into(),
                    reference: thread.clone(),
                })
            }
            // A stop of the reply to one message: repeated, or after the
            // reply ended, it changes nothing.
            Operation::StopThread {
                thread,
                request: send,
            } => {
                super::threads::stop(&self.shared, thread, send.as_deref())?;
                Ok(Receipt {
                    operation: op.name().into(),
                    reference: thread.clone(),
                })
            }
            // Start Coder for a host thread. The handoff runs here, on the
            // access lock, so it must not take that lock again or the
            // desktop handoff lock.
            Operation::RunThread { thread } => {
                let started = crate::control::run_thread(&self.shared, thread)?;
                if let Some(task) = started.changed {
                    self.changed.push(task);
                }
                Ok(Receipt {
                    operation: op.name().into(),
                    reference: started.task,
                })
            }
            // An archived task leaves the lists, so it publishes no summary.
            Operation::ArchiveTask { task } => {
                tasks.archive(request, device, task)?;
                Ok(Receipt {
                    operation: op.name().into(),
                    reference: task.clone(),
                })
            }
            Operation::OpenTaskTerminal { task, cols, rows } => {
                let mut binding = tasks.terminal_binding(task)?;
                binding.directory = binding
                    .directory
                    .canonicalize()
                    .map_err(|_| Code::Unavailable)?;
                let mut terminals = self
                    .shared
                    .task_terminals
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                terminals.retain(|terminal, _| {
                    self.shared
                        .pty
                        .head(&coder_pty::wire::TerminalRef {
                            generation: self.shared.pty.generation().into(),
                            terminal: terminal.clone(),
                        })
                        .is_ok()
                });
                if terminals.len() >= 1024 {
                    return Err(Code::Bounds);
                }
                if !binding.interactive {
                    return Err(Code::Forbidden);
                }
                let open = Open::new(
                    request,
                    crate::mailbox::workspace_id("studio-task"),
                    "",
                    Launch::Shell,
                    Size::new(*rows, *cols),
                );
                match self
                    .shared
                    .pty
                    .open_bound(device, &open, &binding.directory)
                {
                    Ok((_, Value::Opened { terminal, .. })) => {
                        terminals.insert(terminal.terminal.clone(), (task.clone(), binding));
                        Ok(Receipt {
                            operation: op.name().into(),
                            reference: terminal.terminal,
                        })
                    }
                    Ok(_) => Err(Code::Unavailable),
                    Err(refusal) => Err(match refusal.reason {
                        Reason::Revoked => Code::Revoked,
                        Reason::NotAdmitted => Code::Forbidden,
                        Reason::LimitExceeded => Code::Bounds,
                        Reason::IdempotencyConflict => Code::Conflict,
                        Reason::Malformed => Code::Malformed,
                        _ => Code::Unavailable,
                    }),
                }
            }
            Operation::OpenTerminal { cols, rows } => {
                // No workspace at all is a configuration the host cannot
                // serve (`unsupported`); a configured root that is not a
                // directory is a passing condition (`unavailable`).
                let workspace = self
                    .shared
                    .default_workspace
                    .clone()
                    .ok_or(Code::Unsupported)?;
                if !self
                    .shared
                    .config
                    .workspaces
                    .values()
                    .next()
                    .is_some_and(|root| root.is_dir())
                {
                    return Err(Code::Unavailable);
                }
                let open = Open::new(
                    request,
                    workspace,
                    "",
                    Launch::Shell,
                    Size::new(*rows, *cols),
                );
                match self.shared.pty.open(device, &open) {
                    Ok((_, Value::Opened { terminal, .. })) => Ok(Receipt {
                        operation: op.name().into(),
                        reference: terminal.terminal,
                    }),
                    Ok(_) => Err(Code::Unavailable),
                    Err(refusal) => Err(match refusal.reason {
                        Reason::Revoked => Code::Revoked,
                        Reason::NotAdmitted => Code::Forbidden,
                        Reason::LimitExceeded => Code::Bounds,
                        Reason::IdempotencyConflict => Code::Conflict,
                        Reason::Malformed => Code::Malformed,
                        _ => Code::Unavailable,
                    }),
                }
            }
            _ => Err(Code::Unsupported),
        }
    }
}

#[cfg(test)]
#[path = "studio_tests.rs"]
mod studio_tests;

/// Whether `device`, under `grant`, may talk to the workshop agent: the
/// host's owner, or a device holding a grant, which only the owner issues.
pub(crate) fn agent_admits(owner: &str, device: &str, grant: Option<(&str, u64)>) -> bool {
    device == owner || grant.is_some()
}

/// Whether `device`, under `grant`, may set up a workshop agent: only the
/// host's owner, with the owner's own key and no grant.
pub(crate) fn agent_setup_admits(owner: &str, device: &str, grant: Option<(&str, u64)>) -> bool {
    device == owner && grant.is_none()
}

#[cfg(test)]
mod agent_tests {
    #[test]
    fn the_workshop_agent_answers_only_her_owner_and_granted_devices() {
        let owner = "a".repeat(64);
        assert!(super::agent_admits(&owner, &owner, None));
        assert!(super::agent_admits(
            &owner,
            &"b".repeat(64),
            Some(("grant", 1))
        ));
        assert!(!super::agent_admits(&owner, &"c".repeat(64), None));
    }

    #[test]
    fn only_the_owners_own_key_sets_up_the_workshop_agent() {
        let owner = "a".repeat(64);
        assert!(super::agent_setup_admits(&owner, &owner, None));
        // A device the owner granted `operate` asks her for work, but
        // cannot make her.
        assert!(!super::agent_setup_admits(
            &owner,
            &"b".repeat(64),
            Some(("grant", 1))
        ));
        assert!(!super::agent_setup_admits(&owner, &"c".repeat(64), None));
    }

    #[test]
    fn crew_mutations_take_the_owner_only_dispatch_branch() {
        use coder_access::{Operation, crew::JobRole};
        let operations = [
            Operation::NewCrewAgent {
                agent: "paul".into(),
                workspace: "/work/repo".into(),
                job_role: JobRole::SalesLead,
            },
            Operation::ControlCrew {
                control: coder_access::crew::Control {
                    cohort: "floor".into(),
                    selection: coder_access::crew::Selection::AllSales,
                    action: coder_access::crew::ControlAction::Stop,
                    expected: None,
                    reason: "Owner stop.".into(),
                },
            },
            Operation::SetAgentCharter {
                agent: "paul".into(),
                job_role: JobRole::SalesLead,
                expected: 1,
                drafting: false,
                purpose: "Wait for owner review.".into(),
            },
            Operation::RecordAgentVerdict {
                agent: "paul".into(),
                verdict: coder_access::crew::VerdictInput {
                    id: "review-1".into(),
                    subject: coder_access::crew::Subject {
                        kind: "issue".into(),
                        reference: "github:issue/1".into(),
                        revision: 1,
                        sha256: "a".repeat(64),
                    },
                    evidence: vec![coder_access::crew::Evidence {
                        reference: "host:receipt/1".into(),
                        sha256: "b".repeat(64),
                    }],
                    result: coder_access::crew::ResultKind::NeedsEvidence,
                    reason: "Independent acceptance is missing.".into(),
                    question_set_sha256: None,
                },
            },
        ];
        for op in operations {
            assert!(op.owner_agent());
            assert!(!op.reads_only());
            assert!(op.validate().is_ok());
            assert!(!super::agent_setup_admits(
                &"a".repeat(64),
                &"b".repeat(64),
                Some(("grant", 1))
            ));
        }
        assert!(Operation::CrewStatus {}.owner_agent());
        assert!(Operation::CrewStatus {}.reads_only());
        assert!(Operation::CrewStatus {}.agent());
        assert!(
            !Operation::ListAgentVerdicts {
                agent: "paul".into()
            }
            .owner_agent()
        );
    }
}
