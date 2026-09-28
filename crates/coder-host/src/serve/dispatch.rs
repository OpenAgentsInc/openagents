//! Effects of admitted NIP-HOST operations.
//!
//! `coder-access` has already checked the grant, the epoch, and the one right
//! each operation requires, and has recorded the admitted request before it
//! calls here. The request ID is the idempotency key for every effect.

use std::sync::Arc;

use coder_access::Code;
use coder_access::host::Dispatch;
use coder_access::protocol::{Operation, Receipt};
use coder_pty::wire::{Launch, Open, Reason, Size, Value};

use super::Shared;
use crate::tasks::TaskRef;

/// Connects task operations to the task owner and `terminal.open` to the
/// terminal host. Changed tasks are collected so their summaries can be
/// published after the reply is committed.
pub(crate) struct Dispatcher {
    shared: Arc<Shared>,
    pub(crate) changed: Vec<TaskRef>,
}

impl Dispatcher {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            changed: Vec::new(),
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

impl Dispatch for Dispatcher {
    /// The configured workspace labels. The roots they name stay on the host.
    fn workspaces(&mut self) -> Result<Vec<String>, Code> {
        Ok(self.shared.config.workspaces.keys().cloned().collect())
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
                let principal = crate::tasks::Principal {
                    device: device.into(),
                    grant: grant.map(|(id, _)| id.to_owned()),
                    epoch: grant.map(|(_, epoch)| epoch),
                };
                let result = self.shared.tasks.clone().command(&principal, command);
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
