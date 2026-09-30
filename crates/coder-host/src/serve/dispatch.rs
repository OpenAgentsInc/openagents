//! Effects of admitted NIP-HOST operations.
//!
//! `coder-access` has already checked the grant, the epoch, and the one right
//! each operation requires, and has recorded the admitted request before it
//! calls here. The request ID is the idempotency key for every effect.

use std::sync::Arc;

use coder_access::Code;
use coder_access::host::Dispatch;
use coder_access::protocol::{Operation, QueueEdit, Receipt, TaskQueue};
use coder_pty::wire::{Launch, Open, Reason, Size, Value};

use super::Shared;
use crate::tasks::TaskRef;

/// Connects task operations to the task owner and `terminal.open` to the
/// terminal host. Changed tasks are collected so their summaries can be
/// published after the reply is committed.
pub(crate) struct Dispatcher {
    shared: Arc<Shared>,
    pub(crate) changed: Vec<TaskRef>,
    /// Agent spend requests, beside the access store.
    spends: crate::spend::Book,
}

impl Dispatcher {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        let spends = crate::spend::Book::open(&shared.config.access);
        Self {
            shared,
            changed: Vec::new(),
            spends,
        }
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
        let authority = self.shared.authority.clone();
        let standing = move |other: &crate::tasks::Principal| super::standing(&authority, other);
        let (queue, changed) = self
            .shared
            .tasks
            .clone()
            .queue(&principal, task, edit, &standing)?;
        self.changed.extend(changed);
        Ok(queue)
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
                let authority = self.shared.authority.clone();
                let standing =
                    move |other: &crate::tasks::Principal| super::standing(&authority, other);
                let result = self
                    .shared
                    .tasks
                    .clone()
                    .command(&principal, command, &standing);
                self.task(op, result)
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
            // An archived task leaves the lists, so it publishes no summary.
            Operation::ArchiveTask { task } => {
                tasks.archive(request, device, task)?;
                Ok(Receipt {
                    operation: op.name().into(),
                    reference: task.clone(),
                })
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
