//! Admitting work on an explicitly granted remote computer (#10699).
//!
//! The person selects a paired computer; nothing here picks one. [`place`]
//! turns that selection and the host grant the client holds for it into
//! the admission snapshot's placement: the computer, the host-scoped
//! workspace label (never a path), the grant and its revocation epoch, and
//! the source revision the work starts from. [`binding`] names the host
//! generation and the recipient the dispatch is sealed to
//! ([`route_contract::binding`]). The viewing surface, the computer, the
//! executor, and the payer stay independent fields: a remote placement
//! changes only the placement.
//!
//! [`dispatch`] sends an admitted route to that one host, once:
//!
//! - A record that already names a task is followed; nothing is sent.
//! - The binding is rechecked against the host's current generation and
//!   grant first ([`WorkbenchBinding::recheck`]): a restarted host, a later
//!   revocation epoch, a revoked grant, or a different host refuses before
//!   anything is sent.
//! - The recipient and the idempotency key are journaled
//!   ([`route_contract::record::Sent`]) before the host is asked. The key
//!   is the request ID, so the host's own create deduplicates a resend.
//! - A lost acknowledgment leaves the record admitted with its send kept.
//!   The next call asks the same host what that key created and follows
//!   that task. There is no local fallback and no other host: this module
//!   has no path that runs work anywhere but the bound recipient.
//!
//! Remote artifacts stay on the host and are read back by task through the
//! existing artifact readers; the record names the task, so a reconnect
//! resumes the original execution identity.

use route_contract::binding::{Current, GrantNow, HostPlacement, Refusal, WorkbenchBinding};
use route_contract::lifecycle::Lifecycle;
use route_contract::record::Sent;
use route_contract::route::RefusalReason;
use route_contract::snapshot::{AdmissionSnapshot, GrantRef, GrantSource, WorkspaceBinding};
use route_contract::{BINDING_SCHEMA, RouteRecord};

use crate::route::Journal;

/// The computer the person selected for this route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    /// The host's key, as the pairing record names it.
    pub host: String,
    /// The host-scoped workspace label the work runs in.
    pub workspace: String,
}

/// The grant this client holds for a paired host, as the client reads it
/// now (`coder-access` device grant: rights and revocation epoch).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostGrant {
    pub host: String,
    /// The host's generation: a restart is a new one.
    pub generation: String,
    pub grant: String,
    pub epoch: u64,
    pub revoked: bool,
    /// The grant carries the `operate` right (create, steer, cancel).
    pub operate: bool,
    /// The workspace labels the host admits for this device; `None` when
    /// the host does not restrict them.
    pub workspaces: Option<Vec<String>>,
}

/// Why a remote placement is not admitted or not dispatched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The client holds no grant for the selected host.
    Unauthorized,
    /// The grant lacks the `operate` right.
    NoOperate,
    /// The grant is revoked.
    Revoked,
    /// The host does not admit that workspace for this device.
    Workspace,
    /// The snapshot pins no source revision; a remote run must start from
    /// a named one.
    SourceUnbound,
    /// The binding no longer holds: another host, a restart, a later
    /// epoch, or a revocation.
    Binding(Refusal),
    /// The record left the admitted state without a task.
    NotAdmitted,
}

/// Places `snapshot` on the selected host under `grant`.
///
/// # Errors
///
/// A [`Refused`] before anything is sent.
pub fn place(
    snapshot: &mut AdmissionSnapshot,
    selected: &Selected,
    grant: Option<&HostGrant>,
) -> Result<(), Refused> {
    let grant = grant
        .filter(|grant| grant.host == selected.host)
        .ok_or(Refused::Unauthorized)?;
    if grant.revoked {
        return Err(Refused::Revoked);
    }
    if !grant.operate {
        return Err(Refused::NoOperate);
    }
    if grant
        .workspaces
        .as_ref()
        .is_some_and(|admitted| !admitted.contains(&selected.workspace))
    {
        return Err(Refused::Workspace);
    }
    if snapshot
        .input
        .source
        .as_ref()
        .and_then(|source| source.revision.as_ref())
        .is_none()
    {
        return Err(Refused::SourceUnbound);
    }
    snapshot.placement.computer = Some(selected.host.clone());
    snapshot.placement.workspace = Some(WorkspaceBinding {
        project: selected.workspace.clone(),
        path: None,
    });
    snapshot.placement.grant = Some(GrantRef {
        id: grant.grant.clone(),
        epoch: grant.epoch,
        source: GrantSource::Operator,
    });
    Ok(())
}

/// The workbench binding for a placed snapshot: the host generation and
/// the recipient the dispatch is sealed to.
#[must_use]
pub fn binding(snapshot: &AdmissionSnapshot, grant: &HostGrant) -> WorkbenchBinding {
    WorkbenchBinding {
        schema: BINDING_SCHEMA.into(),
        snapshot: snapshot.digest(),
        parent: None,
        placement: HostPlacement {
            computer: grant.host.clone(),
            generation: grant.generation.clone(),
            recipient: grant.host.clone(),
        },
        run: None,
        terminal: None,
        resources: Vec::new(),
    }
}

/// The work a host creates, without the person's message: the host reads
/// the prompt from the order the client seals to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub title: String,
    pub prompt: String,
    pub workspace: String,
    pub engine: Option<String>,
}

/// A host's answer that is not a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostError {
    /// The host refused, with its NIP-HOST code.
    Refused(String),
    /// The transport failed: whether the host acted is unknown.
    Lost,
}

/// One paired host as the client reaches it.
pub trait RemoteHost {
    /// The host's key.
    fn key(&self) -> &str;
    /// The grant and generation as the client reads them now.
    fn grant(&self) -> Option<HostGrant>;
    /// `task.create` with idempotency key `key`; the host returns the same
    /// task for the same key.
    ///
    /// # Errors
    ///
    /// A [`HostError`].
    fn create(&mut self, key: &str, order: &Order) -> Result<String, HostError>;
    /// The task an earlier create with `key` made, if any.
    ///
    /// # Errors
    ///
    /// A [`HostError`].
    fn created(&mut self, key: &str) -> Result<Option<String>, HostError>;
}

/// What [`dispatch`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatched {
    /// The host created the task now.
    Started(String),
    /// The record already named the task, or the host had created it for
    /// this key: follow it.
    Followed(String),
    /// The send's outcome is unknown; the record keeps it, and the next
    /// call reconciles with the same host.
    Unknown,
    Refused(Refused),
    /// The host refused the order with its code.
    HostRefused(String),
}

fn current(grant: Option<HostGrant>, binding: &WorkbenchBinding) -> Current {
    match grant {
        Some(grant) => Current {
            computer: grant.host.clone(),
            generation: grant.generation,
            recipient: grant.host,
            grant: Some(GrantNow {
                id: grant.grant,
                epoch: grant.epoch,
                revoked: grant.revoked,
            }),
            terminal_generation: binding
                .terminal
                .as_ref()
                .and_then(|terminal| terminal.generation.clone()),
        },
        None => Current {
            computer: binding.placement.computer.clone(),
            generation: binding.placement.generation.clone(),
            recipient: binding.placement.recipient.clone(),
            grant: None,
            terminal_generation: None,
        },
    }
}

/// Sends the admitted `record` to `host`, once, under `binding`.
pub fn dispatch(
    journal: &Journal,
    record: &mut RouteRecord,
    binding: &WorkbenchBinding,
    host: &mut dyn RemoteHost,
    order: &Order,
    now_ms: u64,
) -> Dispatched {
    if let Some(task) = record.tasks().first() {
        return Dispatched::Followed((*task).to_owned());
    }
    if record.state != Lifecycle::Admitted {
        return Dispatched::Refused(Refused::NotAdmitted);
    }
    if host.key() != binding.placement.recipient {
        return Dispatched::Refused(Refused::Binding(Refusal::PlacementChanged));
    }
    if let Err(refusal) = binding.recheck(&record.snapshot, &current(host.grant(), binding)) {
        // Nothing was sent before: the route ends refused. A send whose
        // outcome is unknown stays for reconciliation with the same host
        // once its rights are back; it is never resent elsewhere.
        if record.sent.is_none() {
            let reason = match refusal {
                Refusal::Revoked => RefusalReason::MissingGrant,
                _ => RefusalReason::RouteNotAllowed,
            };
            let _ = record.refuse(Some(reason), "remote_admission", now_ms);
            let _ = journal.write(record);
        }
        return Dispatched::Refused(Refused::Binding(refusal));
    }
    let key = record.request.clone();
    let engine = order.engine.as_deref();
    if record.sent.is_some() {
        match host.created(&key) {
            Ok(Some(task)) => {
                let _ = record.dispatched(&task, engine);
                let _ = journal.write(record);
                return Dispatched::Followed(task);
            }
            Ok(None) => {}
            Err(HostError::Lost) => return Dispatched::Unknown,
            Err(HostError::Refused(code)) => return refused_by_host(journal, record, code, now_ms),
        }
    } else {
        record.sent = Some(Sent {
            recipient: binding.placement.recipient.clone(),
            key: key.clone(),
        });
        if journal.write(record).is_err() {
            // No send without the intent on disk.
            record.sent = None;
            return Dispatched::Unknown;
        }
    }
    match host.create(&key, order) {
        Ok(task) => {
            let _ = record.dispatched(&task, engine);
            let _ = journal.write(record);
            Dispatched::Started(task)
        }
        Err(HostError::Lost) => Dispatched::Unknown,
        Err(HostError::Refused(code)) => refused_by_host(journal, record, code, now_ms),
    }
}

/// The host refused the order outright, so it created nothing: the route
/// ends refused.
fn refused_by_host(
    journal: &Journal,
    record: &mut RouteRecord,
    code: String,
    now_ms: u64,
) -> Dispatched {
    let _ = record.refuse(
        Some(RefusalReason::RouteNotAllowed),
        "remote_host_refused",
        now_ms,
    );
    let _ = journal.write(record);
    Dispatched::HostRefused(code)
}

#[cfg(test)]
mod tests;
