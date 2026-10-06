//! The workbench binding (#10669): what an admission snapshot does not
//! say about where a workbench route runs and what it belongs to.
//!
//! A new document beside the frozen ones. It names one admission snapshot
//! by digest and adds the host generation and dispatch recipient, the run
//! (task and engine session) the route started or continues, the terminal
//! it came from with its generation, and the workbench resources it reads
//! ([`workbench::ResourceRef`]). The snapshot keeps the source revision,
//! adapter identity, disclosure, and payer; [`WorkbenchBinding::check`]
//! keeps the two documents in agreement.
//!
//! Three checks use it:
//!
//! - [`WorkbenchBinding::check`]: the binding is about its snapshot.
//! - [`WorkbenchBinding::continues`]: a continuation runs on the same
//!   computer, generation, recipient, task, engine session, and terminal
//!   generation, and its snapshot widens nothing
//!   ([`crate::AdmissionSnapshot::widens`]). Anything else needs a new
//!   offer.
//! - [`WorkbenchBinding::recheck`]: at dispatch and at every supported
//!   control operation, the current rights still match what was admitted.
//!   An admission never outlives a host restart, a revoked grant, or a
//!   later revocation epoch.

use serde::{Deserialize, Serialize};
use workbench::{Kind, ResourceRef};

use crate::digest::{Digest, digest_of};
use crate::snapshot::{AdmissionSnapshot, Widening};

/// The most workbench resources one binding names.
pub const RESOURCES_MAX: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchBinding {
    /// [`crate::BINDING_SCHEMA`].
    pub schema: String,
    /// The admission snapshot this binding extends.
    pub snapshot: Digest,
    /// The parent's binding, exactly when the snapshot inherits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Digest>,
    pub placement: HostPlacement,
    /// The run this route started or continues; absent before dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunBinding>,
    /// The terminal the request came from: a `terminal` reference with
    /// its generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<ResourceRef>,
    /// Workbench resources the route reads or acts on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRef>,
}

/// Where the work runs, in which host generation, and who receives the
/// dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPlacement {
    /// The snapshot's `placement.computer`.
    pub computer: String,
    /// The host's generation when admitted. A restart is a new one.
    pub generation: String,
    /// The key the dispatch is sealed to: the host itself, or the host a
    /// grant names for a remote computer.
    pub recipient: String,
}

/// One run: the task owner's task and the engine session it resumes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBinding {
    pub task: String,
    pub engine: String,
    /// The engine's own session, once it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// The current rights and state a host reads at dispatch or at a control
/// operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Current {
    pub computer: String,
    pub generation: String,
    pub recipient: String,
    /// The grant the snapshot names, as the host holds it now; `None` when
    /// it no longer exists.
    pub grant: Option<GrantNow>,
    /// The bound terminal's generation now, when the route has one.
    pub terminal_generation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantNow {
    pub id: String,
    pub epoch: u64,
    pub revoked: bool,
}

/// Why a binding does not admit, continue, or dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Refusal {
    Malformed {
        detail: String,
    },
    /// The binding is about another snapshot, computer, or task.
    SnapshotMismatch,
    /// The snapshot or binding does not inherit from the named parent.
    NotAContinuation,
    /// The continuation asks for more than its parent granted.
    Widened {
        widenings: Vec<Widening>,
    },
    /// Another computer, host generation, or recipient.
    PlacementChanged,
    /// Another task, engine, or engine session.
    RunChanged,
    /// The terminal belongs to another host or generation.
    TerminalChanged,
    /// The host restarted, the grant's epoch moved on, or the terminal's
    /// generation ended since admission.
    Stale,
    /// The grant was revoked or no longer exists.
    Revoked,
}

fn malformed(detail: &str) -> Refusal {
    Refusal::Malformed {
        detail: detail.into(),
    }
}

impl WorkbenchBinding {
    /// The binding's content digest.
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }

    /// Checks that the binding is well formed and about `snapshot`.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] naming the first disagreement.
    pub fn check(&self, snapshot: &AdmissionSnapshot) -> Result<(), Refusal> {
        if self.schema != crate::BINDING_SCHEMA {
            return Err(malformed("expected openagents.route.workbench-binding.v1"));
        }
        if self.snapshot != snapshot.digest()
            || snapshot.placement.computer.as_deref() != Some(self.placement.computer.as_str())
        {
            return Err(Refusal::SnapshotMismatch);
        }
        if let Some(task) = &snapshot.identity.task
            && self.run.as_ref().map(|run| &run.task) != Some(task)
        {
            return Err(Refusal::SnapshotMismatch);
        }
        if self.parent.is_some() != snapshot.inherits.is_some() {
            return Err(Refusal::NotAContinuation);
        }
        for text in [
            &self.placement.computer,
            &self.placement.generation,
            &self.placement.recipient,
        ] {
            if text.is_empty() {
                return Err(malformed(
                    "a placement names its computer, generation, and recipient",
                ));
            }
        }
        if let Some(terminal) = &self.terminal {
            terminal
                .check()
                .map_err(|refusal| malformed(&refusal.to_string()))?;
            if terminal.kind != Kind::Terminal {
                return Err(malformed("the terminal binding names a terminal"));
            }
        }
        if self.resources.len() > RESOURCES_MAX {
            return Err(malformed("a binding names at most 32 resources"));
        }
        for resource in &self.resources {
            resource
                .check()
                .map_err(|refusal| malformed(&refusal.to_string()))?;
        }
        Ok(())
    }

    /// Admits this binding and `snapshot` as a continuation of `parent`
    /// and `parent_snapshot`: the same computer, generation, recipient,
    /// task, engine session, and terminal, and no widening.
    ///
    /// # Errors
    ///
    /// A [`Refusal`]; the caller then issues a new offer rather than
    /// continuing.
    pub fn continues(
        &self,
        snapshot: &AdmissionSnapshot,
        parent: &WorkbenchBinding,
        parent_snapshot: &AdmissionSnapshot,
    ) -> Result<(), Refusal> {
        self.check(snapshot)?;
        parent.check(parent_snapshot)?;
        if snapshot.inherits.as_ref() != Some(&parent_snapshot.digest())
            || self.parent.as_ref() != Some(&parent.digest())
        {
            return Err(Refusal::NotAContinuation);
        }
        let widenings = snapshot.widens(parent_snapshot);
        if !widenings.is_empty() {
            return Err(Refusal::Widened { widenings });
        }
        if self.placement != parent.placement {
            return Err(Refusal::PlacementChanged);
        }
        match (&self.run, &parent.run) {
            (Some(run), Some(before)) if run == before && run.session.is_some() => {}
            _ => return Err(Refusal::RunChanged),
        }
        if let (Some(terminal), Some(before)) = (&self.terminal, &parent.terminal)
            && !terminal.same_resource(before)
        {
            return Err(Refusal::TerminalChanged);
        }
        Ok(())
    }

    /// Rechecks the admission against `current`, at dispatch and at every
    /// supported control operation (steer, cancel, observe).
    ///
    /// # Errors
    ///
    /// [`Refusal::PlacementChanged`] for another computer or recipient,
    /// [`Refusal::Stale`] for a restarted host, a later grant epoch, or an
    /// ended terminal generation, and [`Refusal::Revoked`] for a grant
    /// that is gone or revoked.
    pub fn recheck(&self, snapshot: &AdmissionSnapshot, current: &Current) -> Result<(), Refusal> {
        self.check(snapshot)?;
        if current.computer != self.placement.computer
            || current.recipient != self.placement.recipient
        {
            return Err(Refusal::PlacementChanged);
        }
        if current.generation != self.placement.generation {
            return Err(Refusal::Stale);
        }
        if let Some(admitted) = &snapshot.placement.grant {
            match &current.grant {
                Some(now) if now.id == admitted.id && !now.revoked => {
                    if now.epoch != admitted.epoch {
                        return Err(Refusal::Stale);
                    }
                }
                _ => return Err(Refusal::Revoked),
            }
        }
        if let Some(terminal) = &self.terminal
            && current.terminal_generation != terminal.generation
        {
            return Err(Refusal::Stale);
        }
        Ok(())
    }
}
